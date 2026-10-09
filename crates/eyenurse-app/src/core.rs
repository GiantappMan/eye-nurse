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
use crate::tray::{I18nTexts, Tray, TrayAction};
use crate::ui::bar::{self, BarSnapshot};
use crate::win::monitors::MonitorRects;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use eyenurse_core::autostart::DesktopStartup;
use eyenurse_core::config::{ConfigStore, LanguagesJson, SettingJson, UiJson};
use eyenurse_core::i18n::I18n;
use eyenurse_core::timer::{GlobalTimer, TimerEvent};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Wry};

/// 托盘/窗口图标（单一来源：仓库根 assets/logo.png；随 AppModel 迁入，原 app.rs:16 不变）
pub const LOGO_PNG: &[u8] = include_bytes!("../../../assets/logo.png");

/// 休息倒计时事件（§4.2）：CoreLoop → 每个 mask 标签，1Hz，仅休息期间发
#[derive(serde::Serialize, Clone, Copy, Debug)]
pub struct RestTickPayload {
    pub remaining_ms: u64,
}

/// 设置窗页眉的实时状态事件（§4.2 增补）：主倒计时 + 暂停/休息态，随 tick 发
/// （设置窗静态存在，emit_to 恒有接收方；页面用它渲染“下次休息”Hero 区）
#[derive(serde::Serialize, Clone, Copy, Debug)]
pub struct SettingsTickPayload {
    pub remaining_ms: u64,
    pub paused: bool,
    pub resting: bool,
}

/// core 聚合（egui 版 CoreBundle 原样搬移；字段对 C/D 不透明）
pub struct CoreBundle {
    pub cfg: ConfigStore,
    pub setting: SettingJson,
    pub ui: UiJson,
    pub lang: LanguagesJson,
    pub i18n: I18n,
    pub timer: GlobalTimer,
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

/// CoreLoop 线程（`run` 消费 Self；由 main 的 setup 钩子 spawn）
pub struct CoreLoop {
    app: AppHandle<Wry>,
    rx: Receiver<CoreMsg>,
    core: CoreBundle,
    startup: DesktopStartup,
    tray: Tray,
    /// 悬浮条共享快照（bar 线程读，本线程写；§6.4）
    bar: Arc<Mutex<BarSnapshot>>,
    /// bar 窗口句柄（bar 线程建窗后回填；本线程 PostMessage 用）
    bar_hwnd: Arc<std::sync::OnceLock<isize>>,
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
}

impl CoreLoop {
    /// 装配（main 的 setup 钩子里调用；托盘已先行在主线程构建完毕）
    pub fn new(
        app: AppHandle<Wry>,
        rx: Receiver<CoreMsg>,
        core: CoreBundle,
        startup: DesktopStartup,
        tray: Tray,
        bar: Arc<Mutex<BarSnapshot>>,
        bar_hwnd: Arc<std::sync::OnceLock<isize>>,
    ) -> Self {
        Self {
            app,
            rx,
            core,
            startup,
            tray,
            bar,
            bar_hwnd,
            is_resting: false,
            lock_countdown: Duration::ZERO,
            last_rest_tick: None,
            is_paused: false,
            paused_since: None,
            locked_at: None,
            mask_labels: Vec::new(),
        }
    }

    /// 主循环（§2）：消息到达即刻处理；1s 空闲到点跑一拍 tick。
    /// 全部发送方 drop（应用退出、命令 State 全部释放）→ Disconnected → 收尾退出。
    pub fn run(mut self) {
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
        // 退出兜底：保存 UI.json（位置/置顶）——egui 版 on_exit 的等价路径
        self.save_ui_json();
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
            CoreMsg::GetSettings(reply) => self.reply(reply, self.core.setting.clone()),
            CoreMsg::GetTexts(reply) => self.reply(reply, self.core.i18n.texts()),
            CoreMsg::GetLanguages(reply) => self.reply(reply, self.core.i18n.available()),
            CoreMsg::GetCurrentLanguage(reply) => {
                self.reply(reply, self.core.i18n.current().to_string())
            }
        }
    }

    /// 应答包装器：通道关闭（命令侧已超时放弃）不算错误，静默即可。
    /// §2 应答契约的落点——凡 Reply 变体在所有路径（含错误路径）恰好回包一次。
    fn reply<T>(&self, tx: Reply<T>, value: T) {
        let _ = tx.send(value);
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
            TrayAction::Exit => {
                // §8.1：退出路径的 UI.json 兜底保存由 CoreLoop 在此完成
                self.save_ui_json();
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
                // §5.2：休息中显示器布局变化 → 重枚举 rcWork，按新列表差分——
                // 新显示器补建遮罩、消失显示器的遮罩关闭；枚举空（切换瞬态）保留旧值
                if self.is_resting {
                    let monitors = crate::win::monitors::get_monitor_infos();
                    if !monitors.is_empty() {
                        self.sync_mask_windows(monitors);
                    }
                }
            }
        }
    }

    // —— 状态机（AppModel 同名方法逐行移植）——

    /// §6.2 rest_now 状态机（对齐 EyeNurseViewModel.cs:177-202）
    pub fn rest_now(&mut self) {
        if self.is_resting {
            return; // v2 同款互斥（cs:178-180，清单 C9）
        }
        let now = Instant::now();
        self.is_resting = true;
        if !self.core.timer.stop(now) {
            // v2 Stop() 未启动也会发一次 CheckTrigger（怪癖 1）
            let events = self.core.timer.poll(now);
            self.apply_timer_events(events);
        }
        // 【有意偏差 Q4/C10】v2 用启动时快照；v3 现枚举显示器（更正确）。
        // 先定 lock_countdown 再建窗：begin_rest 收尾即发出首个 rest-tick（v2
        // 先显示满值再 Task.Delay(1000)，首个递减发生在 +1s）
        self.lock_countdown = self.core.setting.RestDuration.as_duration();
        let monitors = crate::win::monitors::get_monitor_infos();
        self.begin_rest(monitors);
        // 基准：首个 1s 递减发生在 +1s（v2 先显示满值再 Task.Delay(1000)）
        self.last_rest_tick = Some(now);
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

    /// 悬浮条拖动结束写回逻辑坐标并立即保存 UI.json（清单 B6/Q9）
    fn set_bar_position(&mut self, left: f64, top: f64) {
        self.core.ui.CoutdownWindowLeft = eyenurse_core::config::StjF64(left);
        self.core.ui.CoutdownWindowTop = eyenurse_core::config::StjF64(top);
        // 快照镜像同步（托盘双击 WM_APP_SHOW 的位置回放源，§6.4）
        {
            let mut bar = self.bar_lock();
            bar.saved_left = left;
            bar.saved_top = top;
        }
        self.save_ui_json();
    }

    /// 写 UI.json + PostMessage 驱动原生置顶（egui 版 WindowLevel 命令的等价路径）
    fn toggle_topmost(&mut self, on: bool) {
        self.core.ui.TopMost = on;
        self.save_ui_json();
        self.bar_lock().topmost = on;
        self.bar_post(bar::WM_APP_TOPMOST, usize::from(on), 0);
    }

    /// 托盘双击 → bar 按 UI.json 持久化 TopMost/位置恢复显示（B11）。v2
    /// ShowCountdownWindow（EyeNurseViewModel.cs:148-160）每次 Show 前重读 UI 配置：
    /// `TopMost = uiConfig.TopMost` + Left/Top 回放——关掉置顶的用户双击后**不**置顶。
    /// 取数走 CoreLoop 内存态 ui（单写者：与 UI.json 同步演进；进程内不并发读盘——
    /// 写盘非原子可能撕裂，§3 评审修订），经共享快照交给 bar 的 WM_APP_SHOW 应用。
    fn show_bar(&mut self) {
        {
            let mut bar = self.bar_lock();
            bar.saved_left = self.core.ui.CoutdownWindowLeft.0;
            bar.saved_top = self.core.ui.CoutdownWindowTop.0;
            // v2 `TopMost = uiConfig.TopMost`：复选显示值跟随持久化值（不回写磁盘）
            bar.topmost = self.core.ui.TopMost;
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

    /// DisplayChanged 差分（§5.2）：按新显示器列表补建/关闭遮罩。
    /// 骨架实现：关闭全部旧窗再按新列表重建（差分优化留给实现期，行为等价）。
    /// 休息展示态（bar.resting）全程保持 true：rest-tick 由 is_resting 门控且
    /// 重建在本同步调用内完成（无 tick 插入窗口），关窗不改状态位。
    fn sync_mask_windows(&mut self, monitors: Vec<MonitorRects>) {
        self.destroy_mask_windows();
        self.bar_lock().resting = true;
        for mon in &monitors {
            self.create_mask_window(mon);
        }
        self.push_mask_state();
    }

    /// 单个遮罩窗（§5.2；创建本体在模块级 `build_mask_window`，与 smoke 共用）
    fn create_mask_window(&mut self, mon: &MonitorRects) {
        let label = format!("mask{}", self.mask_labels.len()); // 全字母数字
        if build_mask_window(&self.app, &label, mon).is_ok() {
            self.mask_labels.push(label);
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

    /// 建窗后立即下发当前剩余（v2 先显示满值再开始 1s 递减，等价语义）：
    /// 页面若已挂上监听即拿到起始值；未及加载也不丢账——下一拍 tick 继续发。
    fn push_mask_state(&self) {
        let payload = RestTickPayload {
            remaining_ms: self.lock_countdown.as_millis() as u64,
        };
        for label in &self.mask_labels {
            let _ = self.app.emit_to(label, "eyenurse://rest-tick", payload);
        }
    }

    /// end_rest + 计时器收尾（AppModel::end_rest_and_restart 逐行移植）
    fn end_rest_and_restart(&mut self) {
        let now = Instant::now();
        self.destroy_mask_windows();
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
        self.core.timer.start(now);
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
                // 休息期每拍重打（ensure_ 幂等，稳态只是一次 GetWindowLongPtrW 读）
                #[cfg(target_os = "windows")]
                for label in &self.mask_labels {
                    if let Some(w) = self.app.get_webview_window(label) {
                        if let Ok(hwnd) = w.hwnd() {
                            crate::win::ensure_toolwindow(hwnd.0 as isize);
                        }
                    }
                }
            }
            // §4.2：CoreLoop 每拍 emit_to 每个 mask 标签（仅休息期间发）
            let payload = RestTickPayload {
                remaining_ms: self.lock_countdown.as_millis() as u64,
            };
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
