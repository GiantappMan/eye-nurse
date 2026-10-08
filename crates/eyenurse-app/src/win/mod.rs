//! Win32 封装（实现者 B 所有；tauri 架构文档 §5.4 / §5.5 / §6.1）。
//!
//! egui 版的 `heal_toolwindows` 每帧自愈随壳迁移**删除**（§5.4：tauri 窗口在 v3
//! 里没有任何运行时样式切换——置顶切换只作用于原生悬浮条——不存在 winit
//! 「窗口标志回写抹掉补丁」的触发器；创建时打一次即可）。

pub mod monitors;
pub mod session;
pub mod single_instance;

/// 幂等补 WS_EX_TOOLWINDOW（架构文档 §5.4/B7）：v2 在 Loaded 时给悬浮条/遮罩打此
/// 扩展样式以从 Alt-Tab 隐藏（CountdownWindow.xaml.cs:48-58）。tao 的
/// `skip_taskbar` 走 ITaskbarList::DeleteTab，只移除任务栏按钮、不会从 Alt-Tab
/// 隐藏，必须补位（位已在则跳过）。
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
