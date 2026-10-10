//! 锁屏激活热键线程（v3.2 仿锁屏）：可自定义快捷键 = LL 钩子拦截 +
//! `RegisterHotKey` 双路（v3.2.1 起不再有独立 Win+L 开关——快捷键设为 Win+L
//! 即等价旧的「拦截 Win+L 替代系统锁屏」，DisableLockWorkstation 由调用方
//! 按此条件自动开关）。
//!
//! 拦截原理：`WH_KEYBOARD_LL` 钩子收到 vkCode==配置键 的 keydown 且修饰键
//! 全部按住（`GetAsyncKeyState` 高位）时返回 1 吞掉按键并通知 CoreLoop 激活
//! 仿锁屏（AutoHotkey 重映射同路径；LL 钩子硬要求：钩子线程必须有消息循环）。
//! 正常前台钩子先吞键，WM_HOTKEY 不再产生，两路不会双触发；钩子因 UIPI 在
//! 提权前台收不到键时，内核侧分发的 RegisterHotKey 仍可达。
//!
//! 默认 Alt+Win+L 的原因：部分安全管控软件在键盘过滤驱动层把裸 Win+L 整个
//! 吞掉（真实按键与注入按键都到不了 LL 钩子，2026-10-09 实测本机即如此；
//! 同批机器上 HKCU Policies\System 往往也锁 ACL）；Ctrl+Win+L 本机亦被其他
//! 软件 RegisterHotKey 占用（2026-10-10 实测），Win+Alt+L 在两者之外。
//! 注意 Win+L 被系统保留、RegisterHotKey 注册不上（返回失败只记日志降级），
//! 接管场景实际靠钩子单路 + 注册表禁用系统锁兜底。
//!
//! 托盘「立即锁屏」（TrayAction::LockNow，不走本模块）是最后入口。
//!
//! DisableLockWorkstation（HKCU，无需管理员）：快捷键为 Win+L 时系统对 Win+L
//! 直接不响应，杜绝「钩子漏掉 → 退化成系统锁屏」。**退出时必须恢复**，否则
//! 应用退出后用户 Win+L 全废；写失败（键被锁 ACL）降级为仅钩子拦截并由调用方
//! 记日志。

use crate::events::CoreMsg;
use crossbeam_channel::Sender;
use eyenurse_core::config::{HK_ALT, HK_CTRL, HK_SHIFT, HK_WIN, HotkeyCombo};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

/// 自定义快捷键开关（跟随锁屏 Enabled；实际注册/注销由钩子线程执行）
static HOTKEY_ON: AtomicBool = AtomicBool::new(false);
/// 自定义快捷键组合（`HotkeyCombo` 字段快照；钩子回调与注册线程只读）
static HK_MODS: AtomicU32 = AtomicU32::new(0);
static HK_VK: AtomicU16 = AtomicU16::new(0);
static HOOK_TX: std::sync::OnceLock<Sender<CoreMsg>> = std::sync::OnceLock::new();
static THREAD_ID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/// 钩子线程私有消息偏移：请求注册/注销快捷键（wParam 1=注册 0=注销），
/// 实际消息 = WM_APP + 本值
#[cfg(target_os = "windows")]
const WM_APP_HOTKEY: u32 = 5;
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

/// 自定义快捷键开关 + 组合（跟随锁屏 Enabled；注册动作由钩子线程异步完成）。
/// `on=false` 时组合值可传 0——注销路径不读组合
pub fn set_hotkey(on: bool, mods: u8, vk: u16) {
    HK_MODS.store(u32::from(mods), Ordering::Relaxed);
    HK_VK.store(vk, Ordering::Relaxed);
    HOTKEY_ON.store(on, Ordering::Relaxed);
    request_hotkey_sync();
}

/// 请求钩子线程重读 HOTKEY_ON/HK_*（线程未起/已退出时丢弃：启动序列在 spawn 前
/// 调用，线程启动时自行按静态量初始注册一次）
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
        GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT,
        MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_LWIN, VK_RWIN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WM_HOTKEY, WM_KEYDOWN, WM_SYSKEYDOWN, MSG,
    };

    // 钩子/热键命中 → CoreLoop 激活仿锁屏（通道关闭 = 退出中，静默）
    fn fire_lock() {
        if let Some(tx) = HOOK_TX.get() {
            let _ = tx.send(CoreMsg::LockActivate);
        }
    }

    // 自定义快捷键的修饰键实况：掩码里每个修饰键当前都必须按下
    //（GetAsyncKeyState 高位 = 按下；VK_CONTROL/VK_MENU/VK_SHIFT 天然含左右键）
    unsafe fn mods_down(mods: u32) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_MENU, VK_SHIFT};
        let down = |vk: i32| unsafe { GetAsyncKeyState(vk) < 0 };
        (mods & u32::from(HK_CTRL) == 0 || down(VK_CONTROL.0 as i32))
            && (mods & u32::from(HK_ALT) == 0 || down(VK_MENU.0 as i32))
            && (mods & u32::from(HK_SHIFT) == 0 || down(VK_SHIFT.0 as i32))
            && (mods & u32::from(HK_WIN) == 0 || down(VK_LWIN.0 as i32) || down(VK_RWIN.0 as i32))
    }

    // SAFETY：LL 钩子回调——只读钩子结构、发一次通道消息、无重入状态
    unsafe extern "system" fn hook_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            let kb = &*(l.0 as *const KBDLLHOOKSTRUCT);
            let keydown = w.0 == WM_KEYDOWN as usize || w.0 == WM_SYSKEYDOWN as usize;
            // 自定义快捷键（组合读静态量快照；设为 Win+L 即旧的「拦截 Win+L」
            // 场景）：命中即吞键 + 激活；吞掉的键不再进系统，同组合的 WM_HOTKEY
            // 不会双触发
            let hot_vk = u32::from(HK_VK.load(Ordering::Relaxed));
            if keydown
                && hot_vk != 0
                && kb.vkCode == hot_vk
                && HOTKEY_ON.load(Ordering::Relaxed)
                && unsafe { mods_down(HK_MODS.load(Ordering::Relaxed)) }
            {
                eprintln!("[eyenurse] lock hotkey intercepted -> lock");
                fire_lock();
                return LRESULT(1);
            }
        }
        CallNextHookEx(None, code, w, l)
    }

    // 可配置快捷键的 RegisterHotKey 路径（HOTKEY_ON + HK_MODS/HK_VK 为准；
    // WM_HOTKEY → LockActivate）。钩子被 UIPI 跳过（提权前台）时由本路兜底。
    // id 恒为 HOTKEY_ID：先撤旧注册再按当前组合注册——换组合热切换旧注册不
    // 残留；未注册时 Unregister 静默失败，无副作用
    fn sync_hotkey_registration() {
        // SAFETY：线程热键（hwnd=None），消息投递到本线程
        unsafe {
            let _ = UnregisterHotKey(None, HOTKEY_ID);
            if !HOTKEY_ON.load(Ordering::Relaxed) {
                return;
            }
            let combo = HotkeyCombo {
                mods: HK_MODS.load(Ordering::Relaxed) as u8,
                vk: HK_VK.load(Ordering::Relaxed),
            };
            if combo.vk == 0 {
                return; // 组合未配置（理论不可达）：仅钩子路 + 托盘入口
            }
            match RegisterHotKey(None, HOTKEY_ID, mods_to_win32(combo.mods), u32::from(combo.vk)) {
                Ok(()) => {
                    eprintln!("[eyenurse] lock hotkey registered: {}", combo.to_display());
                }
                Err(e) => {
                    // 组合被占（如本机 Ctrl+Alt+L）：只记日志降级，钩子路在
                    // 非提权前台照常吞键
                    eprintln!(
                        "[eyenurse] RegisterHotKey({}) taken: {e}; LL-hook intercept still active",
                        combo.to_display()
                    );
                }
            }
        }
    }

    // 可移植修饰键位（HK_*）→ Win32 HOT_KEY_MODIFIERS
    fn mods_to_win32(m: u8) -> HOT_KEY_MODIFIERS {
        let mut f = HOT_KEY_MODIFIERS(0);
        if m & HK_CTRL != 0 {
            f |= MOD_CONTROL;
        }
        if m & HK_ALT != 0 {
            f |= MOD_ALT;
        }
        if m & HK_SHIFT != 0 {
            f |= MOD_SHIFT;
        }
        if m & HK_WIN != 0 {
            f |= MOD_WIN;
        }
        f
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

        // 启动即按静态量初始注册一次（spawn 前的 set_hotkey 请求无人接收，此处补偿）
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
                if let Some(tx) = HOOK_TX.get() {
                    let _ = tx.send(CoreMsg::LockActivate);
                }
            } else if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_APP + WM_APP_HOTKEY
            {
                sync_hotkey_registration();
            }
        }

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
