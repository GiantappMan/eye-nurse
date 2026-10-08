//! WTS 会话锁专用消息窗口线程（架构文档 §5.4）。
//!
//! 结论（已查证）：`WTSRegisterSessionNotification` 的通知经由窗口过程投递；
//! message-only 窗口（HWND_MESSAGE）收不到系统类消息（Raymond Chen, 2025-11-27），
//! 因此用一个**从不显示的顶层窗口**（WS_OVERLAPPED，不 Show）。

use crate::events::WinEvent;
use crossbeam_channel::Sender;
use std::sync::{Arc, OnceLock};

/// 句柄结构（评审修订：暴露 rx + hwnd + join，退出路径才能 WTS 注销）
pub struct SessionThread {
    pub rx: crossbeam_channel::Receiver<WinEvent>,
    hwnd: Arc<OnceLock<isize>>, // 线程内建窗后回填
    join: Option<std::thread::JoinHandle<()>>,
}

impl SessionThread {
    /// 在独立 std::thread 上执行：
    /// 1) GetModuleHandleW(None) + RegisterClassW("EyeNurse3SessionWnd")
    /// 2) CreateWindowExW(0, "EyeNurse3SessionWnd", "", WS_OVERLAPPED, CW_USEDEFAULT, ..,
    ///    .., HWND_NULL, HMENU::default(), hinstance, None)  // 从不 Show
    /// 3) WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)  // wtsapi32
    /// 4) GetMessageW 循环；过程函数：WM_WTSSESSION_CHANGE(689) wParam 7/8、
    ///    WM_DISPLAYCHANGE、WM_CLOSE（注销 + 销毁 + PostQuitMessage）
    pub fn spawn() -> std::io::Result<Self> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let hwnd: Arc<OnceLock<isize>> = Arc::new(OnceLock::new());
        let hwnd_for_thread = hwnd.clone();
        let join = std::thread::Builder::new()
            .name("eyenurse-session".into())
            .spawn(move || {
                session_thread_main(tx, hwnd_for_thread);
            })?;
        Ok(Self {
            rx,
            hwnd,
            join: Some(join),
        })
    }

    /// 退出路径（架构文档 §7.3）：PostMessageW(hwnd, WM_CLOSE)（跨线程投递合法），
    /// 窗口过程完成注销后线程自然退出
    pub fn request_stop(&self) {
        if let Some(h) = self.hwnd.get() {
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
                // SAFETY：向自身线程创建的窗口投递 WM_CLOSE，销毁流程在窗口线程完成
                unsafe {
                    let _ = PostMessageW(Some(HWND(*h as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
        }
    }

    pub fn join(mut self) {
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[cfg(target_os = "windows")]
fn session_thread_main(tx: Sender<WinEvent>, hwnd_cell: Arc<OnceLock<isize>>) {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::RemoteDesktop::{
        WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        GetWindowLongPtrW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
        GWLP_USERDATA, MSG, WM_CLOSE, WM_DISPLAYCHANGE, WM_WTSSESSION_CHANGE, WNDCLASSW,
        WS_OVERLAPPED,
    };

    // WM_WTSSESSION_CHANGE 的 wParam 值（WTS_SESSION_LOCK=7 / WTS_SESSION_UNLOCK=8）
    const WTS_SESSION_LOCK: usize = 7;
    const WTS_SESSION_UNLOCK: usize = 8;

    // SAFETY：tx 的所有权转移进 GWLP_USERDATA，wndproc 内还原；窗口销毁前不再变动
    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        let tx: Option<Sender<WinEvent>> = {
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if p == 0 {
                None
            } else {
                Some((*(p as *const Sender<WinEvent>)).clone())
            }
        };
        match msg {
            WM_WTSSESSION_CHANGE => {
                if let Some(tx) = tx {
                    let _ = match w.0 {
                        WTS_SESSION_LOCK => tx.send(WinEvent::SessionLock),
                        WTS_SESSION_UNLOCK => tx.send(WinEvent::SessionUnlock),
                        _ => Ok(()),
                    };
                }
                LRESULT(0)
            }
            WM_DISPLAYCHANGE => {
                if let Some(tx) = tx {
                    let _ = tx.send(WinEvent::DisplayChanged);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                // 退出前完成 WTS 注销（架构文档 §7.3）
                let _ = WTSUnRegisterSessionNotification(hwnd);
                let _ = DestroyWindow(hwnd);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }

    // SAFETY：标准窗口类注册/建窗/消息循环；tx 装箱指针由 wndproc 消费，泄漏至窗口销毁
    unsafe {
        let hinstance = match GetModuleHandleW(None) {
            Ok(h) => h.into(),
            Err(_) => return,
        };
        let class_name = windows::core::w!("EyeNurse3SessionWnd");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class_name,
            // 从不显示的窗口：无需背景刷（默认 null）
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return; // 注册失败：无会话通知，主流程照常（rx 永远空）
        }
        let hwnd = match CreateWindowExW(
            Default::default(),
            class_name,
            windows::core::w!(""),
            WS_OVERLAPPED, // 从不 Show 的顶层窗口（见模块注释）
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };
        hwnd_cell.set(hwnd.0 as isize).ok();
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(Box::new(tx)) as isize);

        // 注册失败不致命：仅丢失会话锁通知
        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);

        let mut msg = MSG::default();
        // GetMessageW 返回值：0 = WM_QUIT，-1 = 错误，>0 = 收到消息。
        // 用 `.0 > 0` 而非 as_bool()：-1 在 as_bool() 下为真，错误时会空转死循环。
        loop {
            if GetMessageW(&mut msg, None, 0, 0).0 <= 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn session_thread_main(_tx: Sender<WinEvent>, _hwnd_cell: Arc<OnceLock<isize>>) {}
