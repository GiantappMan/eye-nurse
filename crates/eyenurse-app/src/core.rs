//! CoreLoop 线程（实现者 B 所有，tauri 架构文档 §2）—— egui 版 app.rs `AppModel`
//! 状态机的移植：唯一持有业务状态的写者，1Hz 节拍由 `recv_timeout` 到点驱动。
//!
//! 线程模型（§2）：
//! ```text
//! CoreLoop 线程（本文件）
//! loop {
//!     match rx.recv_timeout(next_tick - now) {   // 1Hz 对齐；消息到达即刻醒来
//!         Ok(msg) => self.apply(msg),            // Tray/Bar/Win/IPC 应答……
//!         Err(Timeout) => {}
//!         Err(Disconnected) => break,            // 全部发送方 drop → 退出循环
//!     }
//!     if now >= next_tick { next_tick += 1s; self.tick(now); }  // tick_timer 原样移植
//! }
//! ```
//! 窗口创建/销毁只发生在 CoreLoop 线程（tauri 明文警告「同步 command/事件回调里建窗
//! 会死锁」，本线程非主线程、非同步回调，是受支持路径，§2）。

use crate::events::{CoreMsg, Reply, WinEvent};
use crate::skin_proto::{self, SkinSurfaceDto};
use crate::tray::{I18nTexts, Tray, TrayAction};
use crate::ui::bar::{self, BarSkinColors, BarSnapshot};
use crate::win::monitors::MonitorRects;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use eyenurse_core::autostart::DesktopStartup;
use eyenurse_core::config::{
    ConfigStore, DEFAULT_LOCK_HOTKEY, HK_WIN, HotkeyCombo, LanguagesJson, LockScreenJson,
    SettingJson, UiJson,
};
use eyenurse_core::i18n::I18n;
use eyenurse_core::skin::{
    self, DiskSkinEntry, SkinColors, SkinConfig, SkinMeta, SkinsDto,
};
use eyenurse_core::stats::{local_today, StatsStore};
use eyenurse_core::timer::{GlobalTimer, TimerEvent};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Wry};

/// 托盘/窗口图标（单一来源：仓库根 assets/logo.png；随 AppModel 迁入，原 app.rs:16 不变）
pub const LOGO_PNG: &[u8] = include_bytes!("../../../assets/logo.png");

/// 休息/锁屏遮罩事件（§4.2 + v3.2 锁屏）：CoreLoop → 每个 mask 标签，1Hz，遮罩
/// 存续期间发。resting/locked/has_password 驱动页面在三态间切换（纯休息 / 纯锁屏 /
/// 锁屏+休息叠加）；页面未及加载也不丢账——下一拍 tick 继续发。
#[derive(serde::Serialize, Clone, Copy, Debug)]
pub struct RestTickPayload {
    pub remaining_ms: u64,
    pub resting: bool,
    pub locked: bool,
    pub has_password: bool,
}

/// 设置窗页眉的实时状态事件（§4.2 增补）：主倒计时 + 暂停/休息态，随 tick 发
/// （设置窗静态存在，emit_to 恒有接收方；页面用它渲染“下次休息”Hero 区）
#[derive(serde::Serialize, Clone, Copy, Debug)]
pub struct SettingsTickPayload {
    pub remaining_ms: u64,
    pub paused: bool,
    pub resting: bool,
}

/// ESP-1 bar 皮肤壳窗（barweb）的 1Hz 状态事件：GDI 条读共享快照的 WebView
/// 等价物（§6.4 数据同源——CoreLoop 仍是唯一写者，barweb 页面只读）。
/// 文案不在此载荷：页面启动取 get_texts + 监听 i18n-changed（低频路径分离）。
#[derive(serde::Serialize, Clone, Copy, Debug)]
pub struct BarStatePayload {
    pub countdown_ms: u64,
    pub interval_ms: u64,
    /// 暂停累计（v2 PausedTime；None = 非暂停态）
    pub paused_ms: Option<u64>,
    pub resting: bool,
    pub topmost: bool,
}

/// core 聚合（egui 版 CoreBundle 原样搬移；v3.2 增锁屏配置与统计；字段对 C/D 不透明）
pub struct CoreBundle {
    pub cfg: ConfigStore,
    pub setting: SettingJson,
    pub ui: UiJson,
    pub lang: LanguagesJson,
    pub i18n: I18n,
    pub timer: GlobalTimer,
    /// 仿锁屏设置（LockScreen.json）
    pub lock: LockScreenJson,
    /// 护眼时间统计（Stats.json；CoreLoop 独占写者）
    pub stats: StatsStore,
    /// 皮肤配置（Skin.json；v3.1 换肤）
    pub skin: SkinConfig,
}

/// §5.2 单个遮罩窗的**生产创建路径本体**（CoreLoop::create_mask_window 与 smoke
/// 共用同一函数——§11 教训 3：冒烟验证的就是生产路径本身，而非其镜像）。
pub(crate) fn build_mask_window(
    app: &AppHandle<Wry>,
    label: &str,
    mon: &MonitorRects,
) -> tauri::Result<tauri::WebviewWindow<Wry>> {
    let win =
        tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("mask.html".into()))
            .title("LockScreenWindow") // 沿用 v2/egui 版标题（冒烟与工具窗口识别）
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .closable(false)
            .always_on_top(true) // v2 Topmost="True"
            .skip_taskbar(true) // 任务栏按钮隐藏（DeleteTab，§5.4）
            .focused(false) // 不抢焦点
            .shadow(false)
            .visible(false) // 先藏后摆，避免 (0,0) 闪现
            .build()?;
    // 物理像素直接摆到**整块显示器矩形**（rc_monitor，含任务栏区域）：
    // 休息 = 全屏遮挡，任务栏不得露出（用户反馈 2026-10）；builder 的
    // position/inner_size 是逻辑像素，tauri 的物理 setter 收物理值，
    // 混合 DPI 不再有换算收敛问题，§5.2
    let _ = win.set_position(tauri::PhysicalPosition::new(mon.rc_monitor[0], mon.rc_monitor[1]));
    let _ = win.set_size(tauri::PhysicalSize::new(
        (mon.rc_monitor[2] - mon.rc_monitor[0]).max(1) as u32,
        (mon.rc_monitor[3] - mon.rc_monitor[1]).max(1) as u32,
    ));
    #[cfg(target_os = "windows")]
    let hwnd_for_style = win.hwnd().ok().map(|h| h.0 as isize);
    let _ = win.show();
    // Alt-Tab 隐藏补丁（§5.4）：同步先打一次；
    // 但 wry 把窗口标志应用代理到主线程异步执行，建窗后的一小段时间内会用 winit
    // 缓存态反复回写 GWL_EXSTYLE、抹掉手工补丁（winit set_window_flags 语义，
    // 即 egui 版 heal_toolwindows 面对的同源问题）。因此这里再派生短周期自愈线程
    // 覆盖创建初段，休息期由 CoreLoop 1Hz tick 兜底。
    #[cfg(target_os = "windows")]
    if let Some(h) = hwnd_for_style {
        crate::win::ensure_toolwindow(h);
        let app = app.clone();
        let label = label.to_string();
        std::thread::spawn(move || {
            use tauri::Manager;
            for _ in 0..12 {
                match app.get_webview_window(&label) {
                    Some(w) => {
                        if let Ok(hwnd) = w.hwnd() {
                            crate::win::ensure_toolwindow(hwnd.0 as isize);
                        }
                    }
                    None => return, // 窗口已销毁（冒烟/正常关闭路径）
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
    }
    Ok(win)
}

/// v3.2 锁屏激活策略热应用：自定义快捷键（空串=禁用；非法串回退默认+记日志；
/// 锁屏本身是自带能力，立即锁屏/托盘不依赖任何开关）；快捷键恰为 Win+L 时
/// 连带 HKCU DisableLockWorkstation——接管系统锁屏，防钩子漏键退化成系统锁屏
/// （启动路径 main 亦调用）。
pub(crate) fn apply_lock_policy(lock: &LockScreenJson) {
    let combo = match HotkeyCombo::parse(&lock.Hotkey) {
        Some(c) => Some(c),
        None if lock.Hotkey.trim().is_empty() => None,
        None => {
            eprintln!(
                "[eyenurse] invalid lock hotkey {:?}; fallback to default {DEFAULT_LOCK_HOTKEY}",
                lock.Hotkey
            );
            HotkeyCombo::parse(DEFAULT_LOCK_HOTKEY)
        }
    };
    // Win+L 接管检测（v3.2.1 起 InterceptWinL/Enabled 并入快捷键；0x4C = 'L'）
    let takeover = matches!(combo, Some(c) if c.mods == HK_WIN && c.vk == u16::from(b'L'));
    if takeover {
        if !crate::win::hotkey::set_disable_lock_workstation(true) {
            // 实测：该键可能被域策略/安全软件锁 ACL（拒绝访问，手动 reg add 同样
            // 失败）。降级为仅钩子拦截：非提权前台照常生效；钩子漏键（提权前台/
            // 驱动层拦截）还有托盘入口兜底。
            eprintln!(
                "[eyenurse] DisableLockWorkstation registry write failed (policies key locked?); \
                 Win+L takeover runs hook-only"
            );
        }
    } else {
        // 恢复默认：尽力删值（值本不存在亦视为已恢复；键被锁死时写不进也意味
        // 着此前没写成过，静默即可，不刷日志噪音）
        crate::win::hotkey::set_disable_lock_workstation(false);
    }
    match combo {
        Some(c) => crate::win::hotkey::set_hotkey(true, c.mods, c.vk),
        None => crate::win::hotkey::set_hotkey(false, 0, 0),
    }
}

/// bar 装配（main setup 产出；ESP-1 双模式——GDI 线程或 barweb 壳窗共用快照，
/// web 模式 hwnd 为空占位、is_web=true）
pub struct BarWiring {
    pub snapshot: Arc<Mutex<BarSnapshot>>,
    pub hwnd: Arc<std::sync::OnceLock<isize>>,
    pub is_web: bool,
}

/// CoreLoop 线程（`run` 消费 Self；由 main 的 setup 钩子 spawn）
pub struct CoreLoop {
    app: AppHandle<Wry>,
    rx: Receiver<CoreMsg>,
    /// 总线发送端克隆（win::menu 菜单线程回投 CoreMsg::Tray 用；ESP-1）
    tx: crossbeam_channel::Sender<CoreMsg>,
    core: CoreBundle,
    startup: DesktopStartup,
    tray: Tray,
    /// 悬浮条共享快照（bar 线程读，本线程写；§6.4）
    bar: Arc<Mutex<BarSnapshot>>,
    /// bar 窗口句柄（bar 线程建窗后回填；本线程 PostMessage 用）
    bar_hwnd: Arc<std::sync::OnceLock<isize>>,
    /// ESP-1：bar 运行模式（true = barweb WebView 壳，启动期定死）。
    /// true 时 bar_hwnd 恒空（bar_post 自然空转），WM_APP 路径由事件广播替代
    bar_is_web: bool,
    /// ESP-1：web bar 位置落盘脏标记（窗口 Moved 逐帧触发 → 攒到 1Hz tick
    /// 落盘；GDI 路径是 WM_EXITSIZEMOVE 一次性写，不经此标记）
    bar_pos_dirty: bool,
    // —— B 内部状态（AppModel 同名字段逐项搬移）——
    is_resting: bool,
    lock_countdown: Duration,
    /// 上次休息倒计时递减时刻：把递减门控到 1Hz（v2 Task.Delay(1000) 粒度）
    last_rest_tick: Option<Instant>,
    is_paused: bool,
    paused_since: Option<Instant>,
    locked_at: Option<Instant>,
    /// 当前打开的遮罩窗口标签（emit_to 与 destroy 都要用，§5.2）
    mask_labels: Vec<String>,
    // —— v3.2 仿锁屏 / 统计 ——
    /// 仿锁屏激活中（与休息态正交：AutoRest 路径两者同真）
    is_locked: bool,
    /// ESP-1：解锁尝试限速（宿主职责，与入口无关）：上次尝试时刻 / 连错计数 /
    /// 冷却截止（连错 5 次 → 60s）
    unlock_last: Option<Instant>,
    unlock_fails: u32,
    unlock_cooldown_until: Option<Instant>,
    /// 统计有未落盘变更（60s 周期 + 关键事件即时落盘）
    stats_dirty: bool,
    /// 上次统计落盘时刻
    last_stats_save: Instant,
}

impl CoreLoop {
    /// 装配（main 的 setup 钩子里调用；托盘已先行在主线程构建完毕）
    pub fn new(
        app: AppHandle<Wry>,
        rx: Receiver<CoreMsg>,
        tx: crossbeam_channel::Sender<CoreMsg>,
        core: CoreBundle,
        startup: DesktopStartup,
        tray: Tray,
        bar: BarWiring,
    ) -> Self {
        Self {
            app,
            rx,
            tx,
            core,
            startup,
            tray,
            bar: bar.snapshot,
            bar_hwnd: bar.hwnd,
            bar_is_web: bar.is_web,
            bar_pos_dirty: false,
            is_resting: false,
            lock_countdown: Duration::ZERO,
            last_rest_tick: None,
            is_paused: false,
            paused_since: None,
            locked_at: None,
            mask_labels: Vec::new(),
            is_locked: false,
            unlock_last: None,
            unlock_fails: 0,
            unlock_cooldown_until: None,
            stats_dirty: false,
            last_stats_save: Instant::now(),
        }
    }

    /// 主循环（§2）：消息到达即刻处理；1s 空闲到点跑一拍 tick。
    /// 全部发送方 drop（应用退出、命令 State 全部释放）→ Disconnected → 收尾退出。
    pub fn run(mut self) {
        // 启动即应用当前皮肤：磁盘/自定义非 classic 时覆盖 bar 快照的 classic 初值，
        // 并广播给已存在的 WebView（页面侧启动另有 get_skin 兜底，双保险）
        let boot_skin = self.effective_skin();
        self.apply_skin(&boot_skin);
        let mut next_tick = Instant::now() + Duration::from_secs(1);
        loop {
            let timeout = next_tick.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(timeout) {
                Ok(msg) => self.apply(msg),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let now = Instant::now();
            if now >= next_tick {
                next_tick += Duration::from_secs(1);
                self.tick(now);
            }
        }
        // 退出兜底：保存 UI.json（位置/置顶）与统计——egui 版 on_exit 的等价路径
        self.save_ui_json();
        self.core.stats.save();
    }

    // —— 消息应用（§2 唯一入口；凡 Reply 变体在所有路径恰好回包一次）——

    fn apply(&mut self, msg: CoreMsg) {
        match msg {
            CoreMsg::Tray(action) => self.apply_tray_action(action),
            CoreMsg::Win(ev) => self.apply_win_event(ev),
            CoreMsg::RestDone => {
                if self.is_resting {
                    self.end_rest_and_restart();
                }
            }
            CoreMsg::SetLanguage(lan) => self.set_language(&lan),
            CoreMsg::SaveSettings(setting, reply) => {
                let applied = self.apply_settings(setting);
                self.reply(reply, applied);
            }
            CoreMsg::ResetSettings(reply) => {
                // 一致性差异 #3（清单 F11）：v2 ResetSettings = 默认值走同一应用路径
                let applied = self.apply_settings(SettingJson::default());
                self.reply(reply, applied);
            }
            CoreMsg::SaveBarPosition { left, top } => self.set_bar_position(left, top),
            CoreMsg::ToggleTopmost(on) => self.toggle_topmost(on),
            CoreMsg::ShowBar => self.show_bar(),
            CoreMsg::ShowBarMenu => {
                // ESP-1：HTML 悬浮条右键菜单——临时线程弹原生菜单（模态不阻塞
                // CoreLoop，休息倒计时照走），选中动作经 Tray 回流共用入口
                let (texts, paused_menu_shown) = {
                    let b = self.bar_lock();
                    (b.texts.clone(), b.paused_menu_shown)
                };
                crate::win::menu::spawn_popup(texts, paused_menu_shown, self.tx.clone());
            }
            CoreMsg::GetSettings(reply) => self.reply(reply, self.core.setting.clone()),
            CoreMsg::GetTexts(reply) => self.reply(reply, self.core.i18n.texts()),
            CoreMsg::GetLanguages(reply) => self.reply(reply, self.core.i18n.available()),
            CoreMsg::GetCurrentLanguage(reply) => {
                self.reply(reply, self.core.i18n.current().to_string())
            }
            // —— v3.1 换肤 / v3.2 ESP-1 开放皮肤协议 ——
            CoreMsg::SetSkin(id) => {
                self.core.skin.Current = id;
                let _ = self.core.cfg.save(&self.core.skin);
                let effective = self.effective_skin();
                self.apply_skin(&effective);
                // ESP-1：表面运行时随配色一并广播（mask/barweb 壳页换 iframe /
                // 调 chrome 摆放；bar 形态变化由设置页对照 bar_is_web 提示重启）
                let _ = self
                    .app
                    .emit("eyenurse://skin-surface-changed", self.surface_dto());
            }
            CoreMsg::SetCustomSkin(def, reply) => {
                self.core.skin.Custom = Some(*def);
                self.core.skin.Current = "custom".into();
                let _ = self.core.cfg.save(&self.core.skin);
                let effective = self.effective_skin();
                self.apply_skin(&effective);
                let _ = self
                    .app
                    .emit("eyenurse://skin-surface-changed", self.surface_dto());
                self.reply(reply, effective);
            }
            CoreMsg::GetSkins(reply) => self.reply(reply, self.skins_dto()),
            CoreMsg::GetSkin(reply) => self.reply(reply, self.effective_skin()),
            CoreMsg::GetSkinSurface(reply) => self.reply(reply, self.surface_dto()),
            // —— v3.2 仿锁屏 / 护眼统计 ——
            CoreMsg::LockActivate => self.lock_now(),
            CoreMsg::LockTryUnlock(password, reply) => {
                let ok = self.try_unlock(&password);
                self.reply(reply, ok);
            }
            CoreMsg::SaveLockSettings(s, reply) => {
                let applied = self.apply_lock_settings(s);
                self.reply(reply, applied);
            }
            CoreMsg::GetLockSettings(reply) => self.reply(reply, self.core.lock.clone()),
            CoreMsg::GetStats(days, reply) => {
                self.reply(reply, self.core.stats.data.last_days(days, &local_today()))
            }
        }
    }

    /// 应答包装器：通道关闭（命令侧已超时放弃）不算错误，静默即可。
    /// §2 应答契约的落点——凡 Reply 变体在所有路径（含错误路径）恰好回包一次。
    fn reply<T>(&self, tx: Reply<T>, value: T) {
        let _ = tx.send(value);
    }

    // —— v3.1 换肤 ——

    /// 磁盘皮肤目录（exe 旁 Assets/Skins；延续 v2「替换 json 即换文案」惯例）
    fn skins_dir() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(std::env::temp_dir)
            .join("Assets")
            .join("Skins")
    }

    /// 当前生效配色（Skin.json 选择 + 磁盘扫描 + 逐键回退 classic）
    fn effective_skin(&self) -> SkinColors {
        let disk = skin::scan_disk(&Self::skins_dir());
        skin::resolve_effective(&self.core.skin, &disk)
    }

    /// ESP-1 表面运行时（配色 + 皮肤页 URL + chrome 摆放 + bar 模式；
    /// GetSkinSurface 命令与 skin-surface-changed 广播共用）
    fn surface_dto(&self) -> SkinSurfaceDto {
        skin_proto::surface_dto(&self.core.skin, self.bar_is_web)
    }

    /// 可用皮肤清单：内置 + 磁盘（Tier 0 + Tier 1）+ 自定义槽（custom 槽仅在
    /// 已有自定义时出现，展示名由前端按语言本地化；colors 供页面直接渲染迷你
    /// 预览；has_bar/has_lock/has_preview = ESP-1 徽标位）
    fn skins_dto(&self) -> SkinsDto {
        let classic = skin::classic();
        let mut skins = skin::builtin_metas();
        for entry in skin::scan_disk(&Self::skins_dir()) {
            let mut colors = skin::resolve(entry.colors_def(), &classic);
            let (zh, en) = entry.display_name();
            if let Some(zh) = zh {
                // Tier 1：manifest.name 为展示真源（覆盖 colors 回退名）
                colors.name = zh;
            }
            if let Some(en) = en {
                colors.name_en = en;
            }
            let (has_bar, has_lock) = entry.has_surface();
            let has_preview = matches!(
                &entry,
                DiskSkinEntry::Package { dir, .. } if dir.join("preview.png").is_file()
            );
            let author = match &entry {
                DiskSkinEntry::Package { manifest, .. } => manifest.author.clone(),
                DiskSkinEntry::Colors { .. } => None,
            };
            skins.push(SkinMeta {
                id: entry.id().to_string(),
                name: colors.name.clone(),
                name_en: colors.name_en.clone(),
                author,
                colors,
                has_bar,
                has_lock,
                has_preview,
            });
        }
        if let Some(def) = &self.core.skin.Custom {
            let colors = skin::resolve(def, &classic);
            skins.push(SkinMeta {
                id: "custom".into(),
                name: "自定义".into(),
                name_en: "Custom".into(),
                author: None,
                colors,
                has_bar: false,
                has_lock: false,
                has_preview: false,
            });
        }
        SkinsDto {
            current: self.core.skin.Current.clone(),
            skins,
        }
    }

    /// 应用生效配色：bar 快照 + WM_APP_SKIN 即时重绘 + WebView 广播
    ///（设置页/遮罩页监听 skin-changed 后重取/重注入 CSS 变量）
    fn apply_skin(&mut self, effective: &SkinColors) {
        self.bar_lock().skin = BarSkinColors::of(effective);
        self.bar_post(bar::WM_APP_SKIN, 0, 0);
        let _ = self.app.emit("eyenurse://skin-changed", effective);
    }

    // —— 托盘/bar 菜单动作（AppModel::apply_tray_action 逐项搬移）——

    fn apply_tray_action(&mut self, action: TrayAction) {
        match action {
            TrayAction::About => self.open_about(),
            TrayAction::Settings => self.show_settings(),
            TrayAction::Pause => {
                // 清单 Q12：菜单可见性**无条件**互换（v2 先翻菜单再调 vm.Pause()），
                // 展示态按 pause() 返回值门控（D8）
                self.tray.set_paused(true);
                self.sync_paused_menu_shown(true);
                self.pause();
            }
            TrayAction::Resume => {
                self.tray.set_paused(false);
                self.sync_paused_menu_shown(false);
                self.resume();
            }
            TrayAction::Reset => self.reset_timer(),
            TrayAction::RestNow => self.rest_now(),
            // v3.2 立即锁屏：已锁时幂等无事（菜单项常驻）
            TrayAction::LockNow => self.lock_now(),
            TrayAction::Exit => {
                // §8.1：退出路径的 UI.json 兜底保存由 CoreLoop 在此完成；
                // v3.2：统计落盘 + 撤销锁屏接管
                self.save_ui_json();
                self.core.stats.save();
                // 注册表 DisableLockWorkstation 必须恢复（否则应用退出后用户
                // Win+L 全废）；快捷键同步注销（线程随进程退出本会自动释放，
                // 显式调用保持与开关路径同一语义）
                crate::win::hotkey::set_disable_lock_workstation(false);
                crate::win::hotkey::set_hotkey(false, 0, 0);
                self.app.exit(0);
            }
        }
    }

    /// 菜单展示态快照（bar 右键菜单与托盘共用同一组 pause/resume 互换，§6.3-2：
    /// 由 CoreLoop 在与托盘 set_paused 同一位置同步）
    fn sync_paused_menu_shown(&self, paused_shown: bool) {
        let mut bar = self.bar_lock();
        bar.paused_menu_shown = paused_shown;
    }

    // —— 会话锁/显示器事件（AppModel::drain_win_events 逐项搬移）——

    fn apply_win_event(&mut self, ev: WinEvent) {
        match ev {
            WinEvent::SessionLock => {
                self.locked_at = Some(Instant::now());
                // 门控（清单 D8）：休息中 pause() 返回 false，不进入暂停展示，只记 locked_at
                self.pause();
            }
            WinEvent::SessionUnlock => {
                // v3.2：仿锁屏未撤下时不恢复计时——期间夹杂的真会话解锁
                // （锁屏界面穿透/快速用户切换）不得绕过仿锁屏的密码门
                if self.is_locked {
                    self.locked_at = None; // 真锁屏时长不计，计时仍由仿锁屏冻结
                    return;
                }
                self.resume(); // 同门控：未处于暂停态时返回 false，无事发生
                if let Some(locked_at) = self.locked_at.take() {
                    if Instant::now().duration_since(locked_at)
                        > self.core.setting.ResetTimeout.as_duration()
                    {
                        // v2 未检查 ResetWhenSessionUnlock 开关（清单 Q3 默认按 v2 原样）
                        if self.is_resting {
                            self.end_rest_and_restart();
                        }
                        self.reset_timer();
                    }
                }
            }
            WinEvent::DisplayChanged => {
                // §5.2 + v3.2：休息或锁屏中显示器布局变化 → 重枚举，按新列表差分——
                // 新显示器补建遮罩、消失显示器的遮罩关闭；枚举空（切换瞬态）保留旧值
                if self.is_resting || self.is_locked {
                    let monitors = crate::win::monitors::get_monitor_infos();
                    if !monitors.is_empty() {
                        self.sync_mask_windows(monitors);
                    }
                }
            }
        }
    }

    // —— 状态机（AppModel 同名方法逐行移植）——

    /// §6.2 rest_now 状态机（对齐 EyeNurseViewModel.cs:177-202；v3.2 增统计入账
    /// 与仿锁屏复用路径）
    pub fn rest_now(&mut self) {
        if self.is_resting {
            return; // v2 同款互斥（cs:178-180，清单 C9）
        }
        let now = Instant::now();
        self.is_resting = true;
        // v3.2 统计：会话次数在开始时记（0 秒会话也计）
        self.core.stats.add_rest_session(&local_today());
        self.stats_dirty = true;
        if !self.core.timer.stop(now) {
            // v2 Stop() 未启动也会发一次 CheckTrigger（怪癖 1）
            let events = self.core.timer.poll(now);
            self.apply_timer_events(events);
        }
        // 【有意偏差 Q4/C10】v2 用启动时快照；v3 现枚举显示器（更正确）。
        // 先定 lock_countdown 再建窗：begin_rest 收尾即发出首个 rest-tick（v2
        // 先显示满值再 Task.Delay(1000)，首个递减发生在 +1s）
        self.lock_countdown = self.core.setting.RestDuration.as_duration();
        if self.is_locked {
            // 仿锁屏（AutoRest 路径 / 锁屏中手动休息）：遮罩已亮着，复用现有窗口，
            // 不重建（label 冲突），仅切换页面显示态
            self.push_mask_state();
        } else {
            let monitors = crate::win::monitors::get_monitor_infos();
            self.begin_rest(monitors);
        }
        // 基准：首个 1s 递减发生在 +1s（v2 先显示满值再 Task.Delay(1000)）
        self.last_rest_tick = Some(now);
    }

    /// v3.2 激活仿锁屏（快捷键钩子/托盘/设置页大按钮共用入口）：替代系统锁屏。
    /// AutoRest → 直接走 rest_now（休息遮罩 + 锁屏 UI 叠加，计入休息统计）；
    /// 否则 pause 冻结计时（对齐真会话锁语义），无遮罩时按显示器补建（纯锁屏
    /// UI）。已锁时幂等无事。
    fn lock_now(&mut self) {
        if self.is_locked {
            return;
        }
        self.is_locked = true;
        self.stats_dirty = true;
        if self.core.lock.AutoRest {
            self.rest_now();
            // rest_now 在锁定态走「复用已有窗口」分支；首次激活时还没有任何
            // 遮罩窗（实测教训：漏掉这条路径 = Win+L 隐形激活，屏幕无任何变化）
            if self.mask_labels.is_empty() {
                let monitors = crate::win::monitors::get_monitor_infos();
                for mon in &monitors {
                    self.create_mask_window(mon);
                }
            }
            self.push_mask_state();
        } else {
            self.pause(); // 离席：计时冻结（resting 中 pause 被门控，无副作用）
            if self.mask_labels.is_empty() {
                let monitors = crate::win::monitors::get_monitor_infos();
                for mon in &monitors {
                    self.create_mask_window(mon);
                }
            }
            self.push_mask_state();
        }
        // 截图穿透：锁屏期对全部遮罩打 WDA_EXCLUDEFROMCAPTURE（遮罩本体在
        // create_mask_window 已打一次，这里兜底；tick 每拍重打）
        if self.core.lock.CaptureThrough {
            for label in &self.mask_labels {
                self.apply_capture_to(label);
            }
        }
        // 锁屏必须可键入：主屏遮罩取焦点，页面首拍再把焦点落进密码框
        if let Some(label) = self.mask_labels.first() {
            if let Some(w) = self.app.get_webview_window(label) {
                let _ = w.set_focus();
            }
        }
    }

    /// v3.2 锁屏遮罩解锁：密码匹配（空密码 = 无需密码）→ 撤锁。休息中（AutoRest）
    /// → 提前收尾休息；纯锁屏 → 关遮罩 + 恢复计时。不匹配回 false（页面提示）。
    /// ESP-1：宿主侧限速——解锁入口开放给皮肤页（经桥），防爆破必须是宿主职责：
    /// 1 次/秒 + 连错 5 次冷却 60s；冷却期内一律 false（页面按密码错误提示）。
    fn try_unlock(&mut self, password: &str) -> bool {
        let now = Instant::now();
        if let Some(until) = self.unlock_cooldown_until {
            if now < until {
                return false;
            }
        }
        if let Some(last) = self.unlock_last {
            if now.duration_since(last) < Duration::from_secs(1) {
                return false;
            }
        }
        self.unlock_last = Some(now);
        if !self.is_locked {
            return true;
        }
        let expected = self.core.lock.Password.trim();
        if !expected.is_empty() && password.trim() != expected {
            self.unlock_fails += 1;
            if self.unlock_fails >= 5 {
                self.unlock_fails = 0;
                self.unlock_cooldown_until = Some(now + Duration::from_secs(60));
            }
            return false;
        }
        self.unlock_fails = 0;
        self.is_locked = false;
        self.stats_dirty = true;
        if self.is_resting {
            // 人回来了：AutoRest 休息提前结束（is_locked 已撤 → 正常关窗收尾）
            self.end_rest_and_restart();
        } else {
            self.destroy_mask_windows();
            // 计时器可能处于停止态（异常序列兜底）：重置为满间隔再恢复
            if !self.core.timer.is_started() {
                let now = Instant::now();
                self.core.timer.reset(now);
                self.core.timer.start(now);
            }
            self.resume();
            self.refresh_bar_snapshot();
        }
        true
    }

    /// 锁屏设置保存：写 LockScreen.json + 热应用钩子/注册表策略（启动路径
    /// main 亦调用同一策略函数）
    fn apply_lock_settings(&mut self, s: LockScreenJson) -> LockScreenJson {
        self.core.lock = s;
        let _ = self.core.cfg.save(&self.core.lock);
        apply_lock_policy(&self.core.lock);
        self.core.lock.clone()
    }

    /// 给单个遮罩窗口打截图穿透（幂等）
    fn apply_capture_to(&self, label: &str) {
        if let Some(w) = self.app.get_webview_window(label) {
            if let Ok(hwnd) = w.hwnd() {
                crate::win::ensure_capture_through(hwnd.0 as isize, true);
            }
        }
    }

    /// timer.pause(now) 成功才置 paused 展示态并 poll（§4.2 怪癖 2、§6.3 门控 D8）
    fn pause(&mut self) -> bool {
        let now = Instant::now();
        if !self.core.timer.pause(now) {
            return false;
        }
        // v2 Pause() 自带一次 CheckTrigger（怪癖 2）→ 立即 poll 并应用
        let events = self.core.timer.poll(now);
        self.apply_timer_events(events);
        self.is_paused = true;
        self.paused_since = Some(now);
        true
    }

    /// 同 pause 门控
    fn resume(&mut self) -> bool {
        let now = Instant::now();
        if !self.core.timer.resume(now) {
            return false;
        }
        let events = self.core.timer.poll(now);
        self.apply_timer_events(events);
        self.is_paused = false;
        self.paused_since = None;
        true
    }

    /// 若暂停展示中先 resume()（清 PausedTime），再 reset + 立即 poll
    /// （v2 Reset cs:229-234；§4.2 怪癖 3，清单 D7/A13）
    fn reset_timer(&mut self) {
        if self.is_paused {
            self.resume();
        }
        let now = Instant::now();
        self.core.timer.reset(now);
        // v2 Reset 内部立即 CheckTrigger（怪癖 3）→ 立即 poll，悬浮条即时回满
        let events = self.core.timer.poll(now);
        self.apply_timer_events(events);
    }

    /// 悬浮条拖动结束写回逻辑坐标并保存 UI.json（清单 B6/Q9）。
    /// ESP-1：web bar 的 Moved 事件逐帧触发本方法（bar_pos_dirty 攒批），
    /// GDI 路径保持拖动结束一次性写。
    fn set_bar_position(&mut self, left: f64, top: f64) {
        self.core.ui.CoutdownWindowLeft = eyenurse_core::config::StjF64(left);
        self.core.ui.CoutdownWindowTop = eyenurse_core::config::StjF64(top);
        // 快照镜像同步（托盘双击 WM_APP_SHOW 的位置回放源，§6.4）
        {
            let mut bar = self.bar_lock();
            bar.saved_left = left;
            bar.saved_top = top;
        }
        if self.bar_is_web {
            self.bar_pos_dirty = true; // 1Hz tick 落盘（见 tick 尾部）
        } else {
            self.save_ui_json();
        }
    }

    /// 写 UI.json + PostMessage 驱动原生置顶（egui 版 WindowLevel 命令的等价路径）。
    /// ESP-1：web bar 同步 set_always_on_top（barweb 窗口归宿主管理）。
    fn toggle_topmost(&mut self, on: bool) {
        self.core.ui.TopMost = on;
        self.save_ui_json();
        self.bar_lock().topmost = on;
        self.bar_post(bar::WM_APP_TOPMOST, usize::from(on), 0);
        if self.bar_is_web {
            if let Some(w) = self.app.get_webview_window("barweb") {
                let _ = w.set_always_on_top(on);
            }
        }
    }

    /// 托盘双击 → bar 按 UI.json 持久化 TopMost/位置恢复显示（B11）。v2
    /// ShowCountdownWindow（EyeNurseViewModel.cs:148-160）每次 Show 前重读 UI 配置：
    /// `TopMost = uiConfig.TopMost` + Left/Top 回放——关掉置顶的用户双击后**不**置顶。
    /// 取数走 CoreLoop 内存态 ui（单写者：与 UI.json 同步演进；进程内不并发读盘——
    /// 写盘非原子可能撕裂，§3 评审修订），经共享快照交给 bar 的 WM_APP_SHOW 应用。
    fn show_bar(&mut self) {
        let (left, top, topmost) = {
            let mut bar = self.bar_lock();
            bar.saved_left = self.core.ui.CoutdownWindowLeft.0;
            bar.saved_top = self.core.ui.CoutdownWindowTop.0;
            // v2 `TopMost = uiConfig.TopMost`：复选显示值跟随持久化值（不回写磁盘）
            bar.topmost = self.core.ui.TopMost;
            (bar.saved_left, bar.saved_top, bar.topmost)
        };
        if self.bar_is_web {
            // web bar 等价路径：按持久化位置回放 + 跟随持久化置顶（不无条件置顶）
            if let Some(w) = self.app.get_webview_window("barweb") {
                let _ = w.set_position(tauri::LogicalPosition::new(left, top));
                let _ = w.set_always_on_top(topmost);
                let _ = w.show();
            }
            return;
        }
        self.bar_post(bar::WM_APP_SHOW, 0, 0);
    }

    /// 写 Languages.json + i18n + tray 文案 + 设置窗标题 + 广播（v2 SetCurrentLanguage）
    fn set_language(&mut self, lan: &str) {
        self.core.i18n.set_language(lan);
        self.core.lang.CurrentLan = Some(lan.to_string());
        let _ = self.core.cfg.save(&self.core.lang);
        let texts = self.texts();
        // 原生层热更新（§9）：托盘七项 + bar 文案（经 WM_APP_LANGUAGE 即时重绘，
        // tooltip 文案同步原地改写；不等 bar 自身 1s WM_TIMER）
        self.tray.update_texts(&texts);
        self.bar_lock().texts = texts.clone();
        self.bar_post(bar::WM_APP_LANGUAGE, 0, 0);
        // 设置窗标题（v2 XAML 绑定 app_name 的等价）
        if let Some(w) = self.app.get_webview_window("settings") {
            let _ = w.set_title(&texts.app_name);
        }
        // 页面层重取 get_texts（§4.2）
        let _ = self.app.emit("eyenurse://i18n-changed", ());
    }

    /// ShellExecute 打开关于页（与 v2 相同 URL）
    fn open_about(&mut self) {
        const ABOUT_URL: &str = "https://www.giantapp.cn/post/products/eyenurse2";
        #[cfg(target_os = "windows")]
        {
            use windows::core::{w, HSTRING};
            use windows::Win32::UI::Shell::ShellExecuteW;
            use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
            // SAFETY：仅打开一个 URL；句柄返回值被忽略（与 v2 Process.Start 同语义）
            unsafe {
                ShellExecuteW(
                    None,
                    w!("open"),
                    &HSTRING::from(ABOUT_URL),
                    None,
                    None,
                    SW_SHOWNORMAL,
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = ABOUT_URL;
    }

    // —— 设置窗（§8.1 WebView 生命周期：静态声明 + hide/show，不复用 egui 视口路径）——

    /// 打开（托盘「设置」/bar 菜单同源入口）：对齐 v2 MainViewModel.ShowWindow——
    /// 已存在则还原+激活，不丢未保存编辑；窗口常驻，页面不重载。
    fn show_settings(&self) {
        if let Some(w) = self.app.get_webview_window("settings") {
            let visible = w.is_visible().unwrap_or(false);
            if !visible {
                let _ = w.show(); // 首次显示
            }
            let _ = w.unminimize(); // 已存在则还原（Minimized → Normal）
            let _ = w.set_focus(); // 激活（!IsActive → Activate）
            // 页面可重同步注册表实测勾选态（Q10）；不清编辑中的草稿（§8.2）
            let _ = self.app.emit_to("settings", "eyenurse://settings-shown", ());
        }
    }

    // —— 休息遮罩（§5：每显示器一个 WebviewWindow）——

    /// begin_rest：休息开始，按显示器列表建窗（CoreLoop 线程执行——非主线程、
    /// 非同步 command，规避 tauri 的 Webview2 死锁场景，§2）
    fn begin_rest(&mut self, monitors: Vec<MonitorRects>) {
        self.bar_lock().resting = true;
        self.mask_labels.clear();
        for mon in &monitors {
            self.create_mask_window(mon);
        }
        self.push_mask_state();
    }

    /// DisplayChanged 差分（§5.2 + v3.2 锁屏）：按新显示器列表补建/关闭遮罩。
    /// 骨架实现：关闭全部旧窗再按新列表重建（差分优化留给实现期，行为等价）。
    /// 展示态（bar.resting）保持原值：休息展示态全程 true；纯锁屏保持 false——
    /// rest-tick 由 is_resting 门控且重建在本同步调用内完成，关窗不改状态位。
    fn sync_mask_windows(&mut self, monitors: Vec<MonitorRects>) {
        let resting = self.is_resting;
        self.destroy_mask_windows();
        self.bar_lock().resting = resting;
        for mon in &monitors {
            self.create_mask_window(mon);
        }
        self.push_mask_state();
    }

    /// 单个遮罩窗（§5.2；创建本体在模块级 `build_mask_window`，与 smoke 共用）。
    /// v3.2：锁屏 + 截图穿透时建窗即打 WDA_EXCLUDEFROMCAPTURE（后续 tick 兜底重打）。
    fn create_mask_window(&mut self, mon: &MonitorRects) {
        let label = format!("mask{}", self.mask_labels.len()); // 全字母数字
        if build_mask_window(&self.app, &label, mon).is_ok() {
            if self.is_locked && self.core.lock.CaptureThrough {
                self.apply_capture_to(&label);
            }
            self.mask_labels.push(label);
        } else {
            eprintln!("[eyenurse] mask window FAILED: {label}");
        }
        // 单屏建窗失败不拖垮其余屏幕
    }

    /// end_rest（§5.3）：**必须用 destroy()**——close() 会先发 CloseRequested、
    /// 被本设计对 mask* 的 prevent_close 拦截器吞掉；destroy() 绕过事件直接强关。
    /// 只负责关窗，不动展示状态位：bar.resting 的清除在 end_rest_and_restart
    /// （休息收尾）完成；sync_mask_windows（显示器变化重建）保持 true。
    fn destroy_mask_windows(&mut self) {
        for label in self.mask_labels.drain(..) {
            if let Some(w) = self.app.get_webview_window(&label) {
                let _ = w.destroy();
            }
        }
    }

    /// 建窗后立即下发当前显示态（v2 先显示满值再开始 1s 递减，等价语义）：
    /// 页面若已挂上监听即拿到起始值；未及加载也不丢账——下一拍 tick 继续发。
    fn push_mask_state(&self) {
        let payload = self.mask_payload();
        for label in &self.mask_labels {
            let _ = self.app.emit_to(label, "eyenurse://rest-tick", payload);
        }
    }

    /// 遮罩显示态快照（倒计时 + 三态标志；页面据此切换 休息/锁屏/叠加 布局）
    fn mask_payload(&self) -> RestTickPayload {
        RestTickPayload {
            remaining_ms: self.lock_countdown.as_millis() as u64,
            resting: self.is_resting,
            locked: self.is_locked,
            has_password: !self.core.lock.Password.trim().is_empty(),
        }
    }

    /// end_rest + 计时器收尾（AppModel::end_rest_and_restart 逐行移植；
    /// v3.2 锁屏守卫：仍在仿锁屏中时不关窗、计时转冻结——AutoRest 倒计时走完
    /// 但人未解锁，遮罩继续亮，解锁后从满间隔恢复）
    fn end_rest_and_restart(&mut self) {
        let now = Instant::now();
        if !self.is_locked {
            self.destroy_mask_windows();
        }
        // 休息展示态随窗关闭同步撤销（rest-tick 由 is_resting 门控，此后不再发）
        self.bar_lock().resting = false;
        self.lock_countdown = Duration::ZERO;
        self.last_rest_tick = None;
        self.is_resting = false;
        // 【有意偏差】休息收尾时干净退出暂停展示，菜单同步翻回“暂停”
        self.is_paused = false;
        self.paused_since = None;
        self.tray.set_paused(false);
        self.sync_paused_menu_shown(false);
        self.core.timer.reset(now);
        // v2：Reset 后立即刷新（怪癖 3）
        let events = self.core.timer.poll(now);
        self.apply_timer_events(events);
        if self.is_locked {
            // 仍在锁屏：重启计时后立即冻结（pause 作用于已停止的计时器会失败，
            // 必须先 start；解锁 resume 后从满间隔恢复），遮罩切换回纯锁屏 UI
            self.core.timer.start(now);
            self.pause();
            self.push_mask_state();
        } else {
            self.core.timer.start(now);
        }
        self.refresh_bar_snapshot();
    }

    // —— 1Hz 节拍（AppModel::tick_timer 逐行移植 + emit rest-tick）——

    fn tick(&mut self, now: Instant) {
        if self.is_resting {
            // 休息推进：每 tick 固定减 1s（v2 Task.Delay(1000) 粒度）
            let due = self
                .last_rest_tick
                .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1));
            if due {
                self.last_rest_tick = Some(now);
                // 已为 0 再过一拍才结束（清单 C6/Q2：显示走到 00:00 并停留一整秒）
                if self.lock_countdown.is_zero() {
                    self.end_rest_and_restart();
                    return;
                }
                self.lock_countdown = self.lock_countdown.saturating_sub(Duration::from_secs(1));
                // §5.4 兜底自愈：DPI 变化等后续窗口标志回写会抹掉 WS_EX_TOOLWINDOW，
                // 休息期每拍重打（ensure_ 幂等，稳态只是一次 GetWindowLongPtrW 读）；
                // v3.2：截图穿透同拍重打（SetWindowDisplayAffinity 同为幂等）
                #[cfg(target_os = "windows")]
                for label in &self.mask_labels {
                    if let Some(w) = self.app.get_webview_window(label) {
                        if let Ok(hwnd) = w.hwnd() {
                            crate::win::ensure_toolwindow(hwnd.0 as isize);
                            if self.is_locked && self.core.lock.CaptureThrough {
                                crate::win::ensure_capture_through(hwnd.0 as isize, true);
                            }
                        }
                    }
                }
            }
            // §4.2：CoreLoop 每拍 emit_to 每个 mask 标签（仅休息期间发）
            let payload = self.mask_payload();
            for label in &self.mask_labels {
                let _ = self.app.emit_to(label, "eyenurse://rest-tick", payload);
            }
        } else if !self.is_paused {
            for ev in self.core.timer.poll(now) {
                match ev {
                    TimerEvent::Elapsed { remaining } => {
                        self.bar_lock().countdown = remaining;
                    }
                    TimerEvent::Trigger => {
                        self.rest_now();
                        return; // rest_now 已切换状态，本拍到此为止
                    }
                }
            }
        }
        // v3.2 纯锁屏（非休息）也要 1Hz 推显示态：页面建窗后异步加载，建窗即发的
        // push_mask_state 可能赶不上挂监听，倒计时/休息路径有逐拍重发兜底，锁屏
        // 路径在这里补齐（叠加态由休息分支覆盖）
        if self.is_locked && !self.is_resting && !self.mask_labels.is_empty() {
            let payload = self.mask_payload();
            for label in &self.mask_labels {
                let _ = self.app.emit_to(label, "eyenurse://rest-tick", payload);
            }
        }
        // v3.2 护眼统计入账（1Hz；休息/锁屏/工作三桶互斥，暂停不记）。
        // 由 Trigger 早退的拍漏记 1s：该拍已转入休息，下一拍起按休息记，可接受。
        if self.is_resting {
            self.core.stats.add_tick(&local_today(), 0, 1, 0);
            self.stats_dirty = true;
        } else if self.is_locked {
            self.core.stats.add_tick(&local_today(), 0, 0, 1);
            self.stats_dirty = true;
        } else if !self.is_paused && self.core.timer.is_started() {
            self.core.stats.add_tick(&local_today(), 1, 0, 0);
            self.stats_dirty = true;
        }
        // 统计周期落盘（60s；关键事件另有即时 save）
        if self.stats_dirty && now.duration_since(self.last_stats_save) >= Duration::from_secs(60)
        {
            self.core.stats.save();
            self.stats_dirty = false;
            self.last_stats_save = now;
        }
        // 暂停时长 1Hz 累加（v2 PausedTime）
        if let Some(since) = self.paused_since {
            self.bar_lock().paused_time = Some(now.duration_since(since));
        }
        // 设置窗页眉“下次休息”实时倒计时（每拍随发；触发 rest_now 的早退分支
        // 会漏掉一拍，下一拍即补上，页眉无感知）
        let tick_payload = SettingsTickPayload {
            remaining_ms: self.bar_lock().countdown.as_millis() as u64,
            paused: self.is_paused,
            resting: self.is_resting,
        };
        let _ = self.app.emit_to("settings", "eyenurse://tick", tick_payload);
        // 暂停 480s 渐变与倒计时数字都依赖快照每拍刷新
        if !self.is_resting {
            self.refresh_bar_snapshot();
        }
        // ESP-1：web bar 的 1Hz 状态广播（GDI 条读快照重绘的 WebView 等价物；
        // 数据同源共享快照，载荷只含数字态——文案走 i18n-changed 低频路径）
        if self.bar_is_web {
            let (countdown, interval, paused_ms, resting, topmost) = {
                let b = self.bar_lock();
                (
                    b.countdown,
                    b.interval,
                    b.paused_time.map(|p| p.as_millis() as u64),
                    b.resting,
                    b.topmost,
                )
            };
            let _ = self.app.emit_to(
                "barweb",
                "eyenurse://bar-state",
                BarStatePayload {
                    countdown_ms: countdown.as_millis() as u64,
                    interval_ms: interval.as_millis() as u64,
                    paused_ms,
                    resting,
                    topmost,
                },
            );
            // web bar 拖动攒批的位置落盘（Moved 逐帧触发 set_bar_position，
            // 这里 1Hz 兜底刷盘；GDI 路径在拖动结束即时写）
            if self.bar_pos_dirty {
                self.bar_pos_dirty = false;
                self.save_ui_json();
            }
        }
    }

    /// 设置保存路径（SaveSettings/ResetSettings）：写盘 + 自启 + 计时器重建，
    /// 返回应用后的值（命令回包 = v2 `Object.assign(settings, 回包)` 的刷新源）
    fn apply_settings(&mut self, s: SettingJson) -> SettingJson {
        self.startup.set_enabled(s.RunWhenStarts);
        // 重启计时器应用新间隔（一致性差异 #7：v2 SettingChanged → Init 的顺序——
        // 先 Stop 旧计时器；间隔 <1s 时 v2 抛异常且旧计时器已停 → 提醒停摆）
        let now = Instant::now();
        if !self.core.timer.stop(now) {
            // v2 Stop() 未启动也会发一次 CheckTrigger（怪癖 1）
            let events = self.core.timer.poll(now);
            self.apply_timer_events(events);
        }
        self.core.setting = s;
        let _ = self.core.cfg.save(&self.core.setting);
        let interval = self.core.setting.RestInterval.as_duration();
        if let Ok(mut t) = GlobalTimer::new(interval) {
            t.start(now);
            self.core.timer = t;
            let mut bar = self.bar_lock();
            bar.interval = interval;
            bar.countdown = interval;
        }
        // §4.2：设置保存/恢复默认后广播（v2 SettingChanged）
        let _ = self.app.emit("eyenurse://settings-changed", ());
        self.core.setting.clone()
    }

    /// 事件应用到展示状态（Elapsed → 悬浮条快照）
    fn apply_timer_events(&mut self, events: Vec<TimerEvent>) {
        for ev in events {
            if let TimerEvent::Elapsed { remaining } = ev {
                self.bar_lock().countdown = remaining;
            }
        }
    }

    /// bar 快照全量刷新（egui 版 refresh_shared_snapshots 的等价；CoreLoop 每拍调用）
    fn refresh_bar_snapshot(&mut self) {
        let now = Instant::now();
        let resting = self.is_resting;
        // 暂停期间冻结剩余值（v2 暂停时无 Elapsed 事件，进度条与剩余不动）
        let remaining = if resting || self.is_paused {
            None
        } else {
            self.core.timer.remaining(now)
        };
        let interval = self.core.timer.interval();
        let paused = self.paused_since.map(|since| now.duration_since(since));
        let mut bar = self.bar_lock();
        if !resting {
            if let Some(r) = remaining {
                bar.countdown = r;
            }
            bar.interval = interval;
        }
        bar.paused_time = paused;
        bar.resting = resting;
    }

    fn texts(&self) -> I18nTexts {
        I18nTexts::from_i18n(&self.core.i18n)
    }

    fn save_ui_json(&self) {
        let _ = self.core.cfg.save(&self.core.ui);
    }

    fn bar_lock(&self) -> std::sync::MutexGuard<'_, BarSnapshot> {
        self.bar.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 向 bar 窗口线程投递 WM_APP 控制消息（§6.4；窗口未建好时丢弃——bar 起来后
    /// 会按快照自行同步一次状态）
    fn bar_post(&self, msg: u32, wparam: usize, lparam: isize) {
        if let Some(hwnd) = self.bar_hwnd.get() {
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
                // SAFETY：跨线程 PostMessage 合法；目标窗口由 bar 线程持有
                unsafe {
                    let _ = PostMessageW(
                        Some(HWND(*hwnd as _)),
                        msg,
                        WPARAM(wparam),
                        LPARAM(lparam),
                    );
                }
            }
            #[cfg(not(target_os = "windows"))]
            let _ = (msg, wparam, lparam);
        }
    }
}
