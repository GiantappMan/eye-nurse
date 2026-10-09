//! HTML 悬浮条（barweb 皮肤）的右键菜单（ESP-1 headless 配套）：皮肤 iframe
//! 拿不到窗口 API，`eyenurse.openMenu()` 经壳页 → `CoreMsg::ShowBarMenu` → 本模块
//! 在**临时线程**上弹出原生 `TrackPopupMenuEx` 菜单（与 GDI 条/托盘同一组动作与
//! pause/resume 互换语义，id/顺序对齐 tray.rs）。选中的动作经 `CoreMsg::Tray` 回
//! CoreLoop，与托盘共用同一入口。
//!
//! 为什么临时线程：菜单是模态消息循环，不能阻塞 CoreLoop（休息倒计时在菜单
//! 打开期间照走）；barweb 的 hwnd 归 WebView 线程，跨线程 TrackPopupMenuEx
//! 不可靠。线程自建屏幕外 0×0 宿主窗（不占位渲染，但 IsWindowVisible=true——
//! KB135788 的「点外部不消失」修复要求 SetForegroundWindow 成功，而它只对
//! 可见窗口生效）。

use crate::events::CoreMsg;
use crate::tray::{I18nTexts, TrayAction};
use crossbeam_channel::Sender;

/// 弹出菜单（非阻塞：spawn 临时线程，立即返回）。`paused_menu_shown` = 当前
/// 菜单应展示「恢复」而非「暂停」（与托盘/bar 菜单同一状态位）。
pub fn spawn_popup(texts: I18nTexts, paused_menu_shown: bool, tx: Sender<CoreMsg>) {
    #[cfg(target_os = "windows")]
    {
        std::thread::Builder::new()
            .name("eyenurse-menu".into())
            .spawn(move || {
                if let Some(action) = popup_once(&texts, paused_menu_shown) {
                    let _ = tx.send(CoreMsg::Tray(action));
                }
            })
            .ok();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (texts, paused_menu_shown, tx);
    }
}

/// 单次弹出：建宿主窗 → 菜单 → 模态等待选择 → 销毁。返回选中动作（关闭/ESC = None）。
#[cfg(target_os = "windows")]
fn popup_once(texts: &I18nTexts, paused_menu_shown: bool) -> Option<TrayAction> {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DispatchMessageW, GetCursorPos,
        PeekMessageW, PostMessageW, RegisterClassW, SetForegroundWindow, ShowWindow,
        TrackPopupMenuEx, TranslateMessage, DestroyMenu, DestroyWindow, MF_SEPARATOR,
        PM_REMOVE, SW_SHOWNOACTIVATE, TPM_NONOTIFY, TPM_RIGHTBUTTON, TPM_RETURNCMD, WM_NULL,
        WNDCLASSW, WS_POPUP,
    };

    // SAFETY：标准窗口类注册/建窗；类名进程级注册，二次注册失败（已存在）忽略，
    // 由 CreateWindowExW 的成败做最终门控
    unsafe {
        let hinstance = match GetModuleHandleW(None) {
            Ok(h) => h.into(),
            Err(_) => return None,
        };
        let class_name = HSTRING::from("EyeNurse3MenuHost");
        let class_pcw = PCWSTR(class_name.as_ptr());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(menu_host_wndproc),
            hInstance: hinstance,
            lpszClassName: class_pcw,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc); // 0 = 已注册或失败，CreateWindowExW 兜底判定

        // 屏幕 (-32000,-32000) 0×0：不渲染任何东西，但 ShowWindow 后可见
        let hwnd = CreateWindowExW(
            Default::default(),
            class_pcw,
            w!("EyeNurse Menu Host"),
            WS_POPUP,
            -32000,
            -32000,
            0,
            0,
            None,
            None,
            Some(hinstance),
            None,
        )
        .ok()?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        let hmenu = CreatePopupMenu().ok()?;
        append_item(hmenu, 1, &texts.about);
        append_item(hmenu, 2, &texts.setting);
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        if paused_menu_shown {
            append_item(hmenu, 3, &texts.resume);
        } else {
            append_item(hmenu, 3, &texts.pause);
        }
        append_item(hmenu, 4, &texts.reset);
        append_item(hmenu, 5, &texts.rest_now);
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        append_item(hmenu, 6, &texts.exit);

        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        // KB135788：非前台窗口的 TrackPopupMenu 点外部不消失——先置前台，
        // 菜单关闭后补投 WM_NULL 清队列残留（bar.rs 同款）
        let _ = SetForegroundWindow(hwnd);
        // TPM_RETURNCMD：返回值即所选 id（模态）；uflags 参数是裸 u32（0.62）
        let chosen = TrackPopupMenuEx(
            hmenu,
            TPM_RETURNCMD.0 | TPM_RIGHTBUTTON.0 | TPM_NONOTIFY.0,
            p.x,
            p.y,
            hwnd,
            None,
        )
        .0;
        let _ = DestroyMenu(hmenu);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyWindow(hwnd);

        // 抽干本线程消息（WM_NULL/菜单残留），避免线程退出时悬挂
        let mut msg = std::mem::zeroed();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // id → TrayAction（与托盘共用同一动作入口，v2 ContextMenu = App.Menu）
        Some(match chosen {
            1 => TrayAction::About,
            2 => TrayAction::Settings,
            3 if paused_menu_shown => TrayAction::Resume,
            3 => TrayAction::Pause,
            4 => TrayAction::Reset,
            5 => TrayAction::RestNow,
            6 => TrayAction::Exit,
            _ => return None,
        })
    }
}

/// 菜单宿主窗不需要任何消息处理（DefWindowProc 即可）
#[cfg(target_os = "windows")]
unsafe extern "system" fn menu_host_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// 菜单项追加（文本缓冲只需活到调用返回；与 bar.rs 同实现）
#[cfg(target_os = "windows")]
fn append_item(
    hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    id: usize,
    text: &str,
) {
    use windows::Win32::UI::WindowsAndMessaging::{AppendMenuW, MF_ENABLED, MF_STRING};
    let w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY：自建 HMENU；宽字符缓冲以 NUL 结尾且活到调用返回
    unsafe {
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_ENABLED,
            id,
            windows::core::PCWSTR(w.as_ptr()),
        );
    }
}
