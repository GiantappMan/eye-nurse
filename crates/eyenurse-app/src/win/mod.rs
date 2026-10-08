//! Win32 封装（实现者 B 所有，架构文档 §5.4 / §5.5 / §7.2 / §7.5）。

pub mod monitors;
pub mod session;
pub mod single_instance;

/// 幂等补 WS_EX_TOOLWINDOW（架构文档 §7.2）：v2 在 Loaded 时给悬浮条打此扩展样式
/// 以从 Alt-Tab 隐藏（CountdownWindow.xaml.cs:48-58）。winit 0.30 的 `with_taskbar(false)`
/// 走 ITaskbarList::DeleteTab，只移除任务栏按钮、不会从 Alt-Tab 隐藏，必须补位。
///
/// **必须自愈、不能只打一次**：winit 的 `set_window_flags` 在任何窗口标志变化时都会
/// 用 `to_window_styles()` 的结果无条件回写 GWL_STYLE/GWL_EXSTYLE，一次回写就把手动
/// 补丁抹掉（winit window_state.rs:187-193/400-408）。由 `logic` 每帧经
/// [`heal_toolwindows`] 对 root 与所有打开的遮罩执行“检测位不在 → 重打”。
#[cfg(target_os = "windows")]
pub fn ensure_toolwindow(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };
    let h = HWND(hwnd as _);
    // SAFETY：仅读写本进程窗口的扩展样式位
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        if ex & (WS_EX_TOOLWINDOW.0 as isize) == 0 {
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_toolwindow(_hwnd: isize) {}

/// §7.2 自愈入口：对 root 悬浮条（标题 CountdownWindow）与所有打开的遮罩
/// （标题 LockScreenWindow）重打 WS_EX_TOOLWINDOW。由 `AppModel::logic` 每帧调用
/// （1Hz 下一次 GetWindowLongPtrW 级别的开销，可忽略）。
///
/// 句柄定位用「枚举本进程顶层窗口 + 标题匹配」：root 句柄虽可由
/// `frame.winit_window()` 取得，但遮罩是 deferred 视口、eframe 不暴露其句柄，
/// 统一走标题匹配最简单可靠——两个标题都是代码内写死常量（main.rs / mask.rs 的
/// `with_title`），不随语言变化。设置窗标题是本地化 app_name，天然不命中、保持
/// 正常窗口形态（v2 MainWindow 同理）。
#[cfg(target_os = "windows")]
pub fn heal_toolwindows() {
    // 标题都是代码内写死常量（main.rs root 视口 / ui::mask::MASK_TITLE），引用常量
    // 而非字面量避免两处漂移（遮罩标题单一真源在 mask.rs）
    const TITLES: [&str; 2] = ["CountdownWindow", crate::ui::mask::MASK_TITLE];
    for hwnd in find_own_windows_by_title(&TITLES) {
        ensure_toolwindow(hwnd);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn heal_toolwindows() {}

/// 枚举本进程顶层窗口，返回标题命中 `titles` 的 hwnd 列表。
#[cfg(target_os = "windows")]
fn find_own_windows_by_title(titles: &[&str]) -> Vec<isize> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
    };

    // SAFETY：lparam 指向调用方栈帧存活的对象（见下方 EnumWindows 调用点）
    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam.0 as *mut Vec<(isize, String)>);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() {
            let mut buf = [0u16; 64];
            let n = GetWindowTextW(hwnd, &mut buf);
            if n > 0 {
                let title = String::from_utf16_lossy(&buf[..n as usize]);
                out.push((hwnd.0 as isize, title));
            }
        }
        BOOL(1) // 继续枚举
    }

    let mut all: Vec<(isize, String)> = Vec::new();
    // SAFETY：回调与 lparam 均指向本栈帧存活对象；返回 Err 视为枚举中断，返回已收集项
    unsafe {
        let _ = EnumWindows(
            Some(enum_cb),
            LPARAM(&mut all as *mut Vec<(isize, String)> as isize),
        );
    }
    all.into_iter()
        .filter(|(_, title)| titles.contains(&title.as_str()))
        .map(|(hwnd, _)| hwnd)
        .collect()
}
