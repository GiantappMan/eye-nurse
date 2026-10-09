//! Win+L 低级键盘钩子线程 + 系统锁屏策略（v3.2 仿锁屏）。
//!
//! 拦截原理与 AutoHotkey 重映射 Win+L 同路径：`WH_KEYBOARD_LL` 钩子收到
//! vkCode=='L' 的 keydown 且 Win 键按下（`GetAsyncKeyState` 高位）时返回 1 吞掉
//! 按键，并通知 CoreLoop 激活仿锁屏。LL 钩子的硬要求：钩子线程必须有消息循环。
//!
//! 局限（UIPI）：前台是提权窗口时本进程收不到键盘输入，钩子会漏——此时靠
//! `DisableLockWorkstation` 兜底（HKCU Policies，无需管理员）：注册表开着时系统
//! 对 Win+L 直接不响应，杜绝「钩子漏掉 → 退化成系统锁屏」。代价是注册表开着
//! 期间用户彻底无法用 Win+L 系统锁屏，因此**退出路径必须恢复**（见模块尾注释）。
//!
//! 开关是进程级 `AtomicBool`（settings 保存路径热切换）；钩子线程随进程存活，
//! 退出时 `request_stop()`（PostThreadMessageW(WM_QUIT)）→ Unhook。

use crate::events::CoreMsg;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};

/// 仿锁屏拦截开关（CoreLoop 在锁屏设置保存/启动时热切换；false 时钩子直通）
static HOOK_ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK_TX: std::sync::OnceLock<Sender<CoreMsg>> = std::sync::OnceLock::new();
static THREAD_ID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/// 装配 + 启动钩子线程（进程级单次；重复调用静默拒绝）
pub fn spawn(tx: Sender<CoreMsg>) -> std::io::Result<()> {
    if HOOK_TX.set(tx).is_err() {
        return Ok(()); // 已启动
    }
    std::thread::Builder::new()
        .name("eyenurse-hotkey".into())
        .spawn(hotkey_thread_main)?;
    Ok(())
}

/// 仿锁屏拦截开关（Enabled && InterceptWinL 才开；关闭时按键直通系统）
pub fn set_enabled(on: bool) {
    HOOK_ENABLED.store(on, Ordering::Relaxed);
}

/// 退出路径：投递 WM_QUIT 结束消息循环（线程内完成 Unhook）；未回填线程 id
/// （钩子注册失败）则无事发生
pub fn request_stop() {
    if let Some(id) = THREAD_ID.get() {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
            // SAFETY：向自身线程投递消息，合法跨线程
            unsafe {
                let _ = PostThreadMessageW(*id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = id;
    }
}

/// DisableLockWorkstation 策略开关：on=true 时系统 Win+L 不再锁屏（本应用接管）。
/// off=删除注册表值（尽力而为：值不存在也算成功，失败由调用方忽略——
/// 退出路径漏删的残留会在下次启动按当前设置重设/清理）。
pub fn set_disable_lock_workstation(on: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{w, PCWSTR};
        use windows::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY,
            HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE,
        };
        const SUBKEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System");
        const VALUE: PCWSTR = w!("DisableLockWorkstation");
        // SAFETY：标准注册表序列；句柄用完即关（0.62 全系返回 WIN32_ERROR，0 = 成功）
        unsafe {
            let mut hkey = HKEY::default();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                SUBKEY,
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            )
            .0
            != 0
            {
                return false;
            }
            let result = if on {
                let val: u32 = 1;
                RegSetValueExW(hkey, VALUE, Some(0), REG_DWORD, Some(&val.to_ne_bytes())).0 == 0
            } else {
                let _ = RegDeleteValueW(hkey, VALUE); // 值不存在亦视为已恢复
                true
            };
            let _ = RegCloseKey(hkey);
            result
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = on;
        true
    }
}

#[cfg(target_os = "windows")]
fn hotkey_thread_main() {
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LWIN, VK_RWIN};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN, MSG,
    };

    const VK_L: u16 = 0x4C; // 'L'

    // SAFETY：LL 钩子回调——只读钩子结构、发一次通道消息、无重入状态
    unsafe extern "system" fn hook_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            let kb = &*(l.0 as *const KBDLLHOOKSTRUCT);
            let keydown = w.0 == WM_KEYDOWN as usize || w.0 == WM_SYSKEYDOWN as usize;
            // Win 键高位置位（GetAsyncKeyState 返回 SHORT，高位 = 按下）
            let win_down = unsafe {
                (GetAsyncKeyState(VK_LWIN.0 as i32) | GetAsyncKeyState(VK_RWIN.0 as i32)) < 0
            };
            if keydown && kb.vkCode == u32::from(VK_L) && win_down && HOOK_ENABLED.load(Ordering::Relaxed) {
                if let Some(tx) = HOOK_TX.get() {
                    let _ = tx.send(CoreMsg::LockActivate);
                }
                return LRESULT(1); // 吞掉 Win+L，系统不再锁屏
            }
        }
        CallNextHookEx(None, code, w, l)
    }

    // SAFETY：注册线程级 LL 钩子 + 标准消息循环；WM_QUIT 退出时 Unhook
    unsafe {
        let Ok(hinstance) = GetModuleHandleW(None) else {
            return;
        };
        let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(hinstance.into()), 0)
        {
            Ok(h) => h,
            Err(_) => return, // 注册失败：仅失去 Win+L 拦截，主流程照常
        };
        THREAD_ID.set(GetCurrentThreadId()).ok();

        let mut msg = MSG::default();
        // GetMessageW 返回值：0 = WM_QUIT，-1 = 错误（as_bool 会把 -1 当真，空转死循环）
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {}

        let _ = UnhookWindowsHookEx(hook);
    }
}

#[cfg(not(target_os = "windows"))]
fn hotkey_thread_main() {
    // 非 Windows：无钩子；线程空转等待通道关闭即可（保持 spawn 语义统一）
    loop {
        std::thread::park();
    }
}
