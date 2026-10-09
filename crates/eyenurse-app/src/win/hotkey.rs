//! Win+L 低级键盘钩子线程 + Ctrl+Alt+L 系统热键 + 系统锁屏策略（v3.2 仿锁屏）。
//!
//! 拦截原理与 AutoHotkey 重映射 Win+L 同路径：`WH_KEYBOARD_LL` 钩子收到
//! vkCode=='L' 的 keydown 且 Win 键按下（`GetAsyncKeyState` 高位）时返回 1 吞掉
//! 按键，并通知 CoreLoop 激活仿锁屏。LL 钩子的硬要求：钩子线程必须有消息循环。
//!
//! **实测局限（2026-10-09）**：部分安全管控软件在键盘过滤驱动层把 Win+L 整个
//! 吞掉（真实按键与注入按键都到不了 LL 钩子，本机即如此；同一批机器上
//! HKCU Policies\System 往往也锁 ACL）。因此仿锁屏另有两条激活路径：
//! - Ctrl+Alt+L 系统热键（RegisterHotKey 线程热键，管控一般不拦）——开关跟随
//!   锁屏设置 Enabled，经 `set_hotkey` 热切换（线程消息投递到钩子线程注册）；
//! - 托盘「立即锁屏」菜单（TrayAction::LockNow，不走本模块）。
//!
//! DisableLockWorkstation（HKCU，无需管理员）：注册表开着时系统对 Win+L 直接
//! 不响应，杜绝「钩子漏掉 → 退化成系统锁屏」。**退出时必须恢复**，否则应用退出
//! 后用户 Win+L 全废；写失败（键被锁 ACL）降级为仅钩子拦截并由调用方记日志。

use crate::events::CoreMsg;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};

/// 仿锁屏拦截开关（CoreLoop 在锁屏设置保存/启动时热切换；false 时钩子直通）
static HOOK_ENABLED: AtomicBool = AtomicBool::new(false);
/// Ctrl+Alt+L 热键开关（跟随锁屏 Enabled；实际注册/注销由钩子线程执行）
static HOTKEY_ON: AtomicBool = AtomicBool::new(false);
static HOOK_TX: std::sync::OnceLock<Sender<CoreMsg>> = std::sync::OnceLock::new();
static THREAD_ID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/// 钩子线程私有消息：请求注册/注销 Ctrl+Alt+L（wParam 1=注册 0=注销）
#[cfg(target_os = "windows")]
const WM_APP_HOTKEY: u32 = 5; // WM_APP = 0x8000
/// RegisterHotKey 的热键 id（线程热键，hwnd = None）
#[cfg(target_os = "windows")]
const HOTKEY_ID: i32 = 1;

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

/// Ctrl+Alt+L 激活热键开关（跟随锁屏 Enabled；注册动作由钩子线程异步完成）
pub fn set_hotkey(on: bool) {
    HOTKEY_ON.store(on, Ordering::Relaxed);
    request_hotkey_sync();
}

/// 请求钩子线程重读 HOTKEY_ON（线程未起/已退出时丢弃：启动序列在 spawn 前
/// 调用，线程启动时自行按 HOTKEY_ON 初始注册一次）
fn request_hotkey_sync() {
    #[cfg(target_os = "windows")]
    {
        if let Some(id) = THREAD_ID.get() {
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};
            // SAFETY：向自身线程投递线程消息，合法跨线程
            unsafe {
                let _ = PostThreadMessageW(
                    *id,
                    WM_APP + WM_APP_HOTKEY,
                    WPARAM(HOTKEY_ON.load(Ordering::Relaxed) as usize),
                    LPARAM(0),
                );
            }
        }
    }
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
/// off=删除注册表值（尽力而为：值不存在也算成功）；返回 false = 写失败
/// （键被域策略/安全软件锁 ACL），调用方记日志降级。
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
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, MOD_ALT, MOD_CONTROL, VK_LWIN, VK_RWIN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, RegisterHotKey, SetWindowsHookExW, UnhookWindowsHook,
        UnregisterHotKey, KBDLLHOOKSTRUCT, LLKHF_INJECTED, WH_KEYBOARD_LL, WM_HOTKEY,
        WM_KEYDOWN, WM_SYSKEYDOWN, MSG,
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
            if keydown && kb.vkCode == u32::from(VK_L) && win_down {
                if HOOK_ENABLED.load(Ordering::Relaxed) {
                    // debug 构建有控制台；命中才算热点路径，日志量可忽略
                    eprintln!("[eyenurse] Win+L intercepted -> lock");
                    if let Some(tx) = HOOK_TX.get() {
                        let _ = tx.send(CoreMsg::LockActivate);
                    }
                    return LRESULT(1); // 吞掉 Win+L，系统不再锁屏
                }
            }
        }
        CallNextHookEx(None, code, w, l)
    }

    // Ctrl+Alt+L 注册/注销（HOTKEY_ON 为准；WM_HOTKEY → LockActivate）
    fn sync_hotkey_registration() {
        // SAFETY：线程热键（hwnd=None），消息投递到本线程
        unsafe {
            if HOTKEY_ON.load(Ordering::Relaxed) {
                match RegisterHotKey(None, HOTKEY_ID, MOD_CONTROL | MOD_ALT, VK_L as u32) {
                    Ok(()) => eprintln!("[eyenurse] Ctrl+Alt+L hotkey registered"),
                    Err(e) => eprintln!("[eyenurse] RegisterHotKey(Ctrl+Alt+L) failed: {e}"),
                }
            } else {
                let _ = UnregisterHotKey(None, HOTKEY_ID);
            }
        }
    }

    // SAFETY：注册线程级 LL 钩子 + 标准消息循环；WM_QUIT 退出时 Unhook
    unsafe {
        let Ok(hinstance) = GetModuleHandleW(None) else {
            eprintln!("[eyenurse] hotkey thread: GetModuleHandleW failed");
            return;
        };
        let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(hinstance.into()), 0)
        {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[eyenurse] SetWindowsHookExW failed: {e}; Win+L interception off");
                return; // 注册失败：仅失去 Win+L 拦截，主流程照常
            }
        };
        eprintln!("[eyenurse] keyboard LL hook installed");
        THREAD_ID.set(GetCurrentThreadId()).ok();

        // 启动即按开关初始注册一次（spawn 前的 set_hotkey 请求无人接收，此处补偿）
        if HOTKEY_ON.load(Ordering::Relaxed) {
            sync_hotkey_registration();
        }

        let mut msg = MSG::default();
        // GetMessageW 返回值：0 = WM_QUIT，-1 = 错误（as_bool 会把 -1 当真，空转死循环）。
        // 线程消息（WM_APP_HOTKEY / WM_HOTKEY）不经 Dispatch，直接在循环里识别处理。
        loop {
            if GetMessageW(&mut msg, None, 0, 0).0 <= 0 {
                break;
            }
            if msg.message == WM_HOTKEY && msg.wParam.0 == HOTKEY_ID as usize {
                eprintln!("[eyenurse] Ctrl+Alt+L hotkey -> lock");
                if let Some(tx) = HOOK_TX.get() {
                    let _ = tx.send(CoreMsg::LockActivate);
                }
            } else if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_APP + WM_APP_HOTKEY
            {
                sync_hotkey_registration();
            }
        }

        let _ = UnhookWindowsHook(hook);
    }
}

#[cfg(not(target_os = "windows"))]
fn hotkey_thread_main() {
    // 非 Windows：无钩子；线程空转等待通道关闭即可（保持 spawn 语义统一）
    loop {
        std::thread::park();
    }
}
