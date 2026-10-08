//! AppModel 状态机 + eframe App 实现（实现者 B 所有，架构文档 §5.6 / §6）。

use crate::events::{UiAction, WinEvent};
use crate::tray::{I18nTexts, Tray, TrayAction};
use crate::ui;
use crate::win::monitors::MonitorRects;
use crossbeam_channel::Receiver;
use eyenurse_core::autostart::DesktopStartup;
use eyenurse_core::config::{ConfigStore, LanguagesJson, SettingJson, UiJson};
use eyenurse_core::i18n::I18n;
use eyenurse_core::timer::{GlobalTimer, TimerEvent};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 托盘/窗口图标（单一来源：仓库根 assets/logo.png；D 打包时可改为构建期拷贝）
pub const LOGO_PNG: &[u8] = include_bytes!("../../../assets/logo.png");

pub struct AppModel {
    ctx: egui::Context,
    tray: Tray,
    core: CoreBundle, // 字段对 C/D 不透明（经方法区访问器暴露）
    // —— 共享 UI 快照与动作出口 ——
    shared: Arc<SharedUi>,
    actions_rx: Receiver<UiAction>,
    rx_win: Receiver<WinEvent>,
    // —— B 内部状态 ——
    is_resting: bool,
    lock_countdown: Duration,
    /// 上次休息倒计时递减时刻：logic 会因输入事件以 >1Hz 运行，
    /// 用真实时间把递减门控到 1Hz（v2 Task.Delay(1000) 粒度）
    last_rest_tick: Option<Instant>,
    is_paused: bool,
    paused_since: Option<Instant>,
    locked_at: Option<Instant>,
    settings_open: bool,
    startup: DesktopStartup,
}

/// core 聚合（架构文档 §5.6；C/D 不直接访问）
pub struct CoreBundle {
    pub cfg: ConfigStore,
    pub setting: SettingJson,
    pub ui: UiJson,
    pub lang: LanguagesJson,
    pub i18n: I18n,
    pub timer: GlobalTimer,
}

/// 悬浮条展示状态（bar.rs 消费）
#[derive(Debug, Clone)]
pub struct BarState {
    pub main_countdown: Duration,
    pub main_interval: Duration,
    pub paused_time: Option<Duration>,
    pub is_resting: bool,
}

/// 遮罩展示状态（mask.rs 消费）
#[derive(Debug, Clone)]
pub struct MaskState {
    pub lock_countdown: Duration,
    pub monitors: Vec<MonitorRects>,
    pub open: bool,
}

/// 休息/暂停等跨视口共享的展示状态（deferred 视口回调要求 Send+Sync，
/// 用 Arc<Mutex<..>> 传入；每帧 logic 里更新快照）
pub struct SharedUi {
    pub bar: Mutex<BarState>,
    pub mask: Mutex<MaskState>,
    // 类型定义在 D 的 ui/settings.rs（所有权规则见 §1）
    pub settings: Mutex<ui::settings::SettingsDraft>,
    // 跨视口动作出口：deferred 视口闭包发送后必须唤醒 ROOT（§6.1），否则最坏 1s 才被处理
    pub actions: (
        crossbeam_channel::Sender<UiAction>,
        crossbeam_channel::Receiver<UiAction>,
    ),
}

impl eframe::App for AppModel {
    /// §6.1 每帧流程：每次 ui 之前调用，窗口隐藏时只要 request_repaint 也会被调用
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_win_events();
        self.drain_tray_events();
        self.drain_actions();
        self.tick_timer(Instant::now());
        self.refresh_shared_snapshots();
        // 注册遮罩/设置窗（当帧生效；当帧不注册即销毁）
        ui::show_viewports(ctx, self);
        // §7.2 WS_EX_TOOLWINDOW 自愈（清单 B7）：winit 在任何窗口标志变化时都会无条件
        // 回写 GWL_EXSTYLE（toggle_topmost 的 WindowLevel 即触发），把补丁抹掉。
        // 每帧“检测位不在 → 重打”同时覆盖“样式类命令发出后补一次”的要求——命令由
        // winit 异步应用，下一帧 logic 必然发生在回写之后，缺失窗口期 ≤1s。
        crate::win::heal_toolwindows();
        // 1Hz 心跳（只作用于 ROOT，§6.1 要点）
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    /// root 视口内容 = 悬浮倒计时条
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::bar::bar_ui(ui, self);
    }

    /// 退出兜底：保存 UI.json（位置/置顶）
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_ui_json();
    }
}

impl AppModel {
    pub fn new(cc: &eframe::CreationContext, rx: Receiver<WinEvent>) -> Self {
        // 1) 首帧前注册中文字体（C 提供该函数，由 B 在此调用一次；§1 契约）
        ui::fonts::load_cjk_fonts(&cc.egui_ctx);

        // 2) 读配置 → GlobalTimer 启动（对齐 v2 EyeNurseViewModel.Init cs:98-102）
        let cfg = ConfigStore::v2_default();
        let setting: SettingJson = cfg.load();
        let ui_cfg: UiJson = cfg.load();
        let lang: LanguagesJson = cfg.load();
        let i18n = load_i18n(&cfg, &lang);

        let interval = setting.RestInterval.as_duration();
        // interval < 1s 的手改配置：回退默认 45 分钟（v2 构造函数会抛异常，v3 容错）
        let mut timer = GlobalTimer::new(interval)
            .or_else(|_| GlobalTimer::new(Duration::from_secs(45 * 60)))
            .expect("默认间隔必合法");
        timer.start(Instant::now());

        // 托盘：此处已处于 eframe creator 回调（winit 主线程），满足 tray-icon 要求。
        // 失败（极少见，如 explorer 未运行）骨架期直接 panic，后续可降级为无托盘模式。
        let (icon_rgba, w, h) = load_logo_rgba();
        let tray =
            Tray::new(icon_rgba.clone(), w, h, &I18nTexts::from_i18n(&i18n)).expect("托盘创建失败");

        let (actions_tx, actions_rx) = crossbeam_channel::unbounded();
        let shared = Arc::new(SharedUi {
            bar: Mutex::new(BarState {
                main_countdown: interval,
                main_interval: interval,
                paused_time: None,
                is_resting: false,
            }),
            mask: Mutex::new(MaskState {
                lock_countdown: Duration::ZERO,
                monitors: Vec::new(),
                open: false,
            }),
            settings: Mutex::new(ui::settings::SettingsDraft::from_setting(&setting)),
            actions: (actions_tx, actions_rx.clone()),
        });

        // 每次启动都应用自启设置（对齐 v2 App.xaml.cs:43 → EyeNurseService.Init，
        // 清单 G5“首启写自启”的兜底；迁移已在 main.rs 完成）
        let startup = DesktopStartup::new("Eye Nurse 3", exe_path_string());
        startup.set_enabled(setting.RunWhenStarts);

        Self {
            ctx: cc.egui_ctx.clone(),
            tray,
            core: CoreBundle {
                cfg,
                setting,
                ui: ui_cfg,
                lang,
                i18n,
                timer,
            },
            shared,
            actions_rx,
            rx_win: rx,
            is_resting: false,
            lock_countdown: Duration::ZERO,
            last_rest_tick: None,
            is_paused: false,
            paused_since: None,
            locked_at: None,
            settings_open: false,
            startup,
        }
    }

    // —— 供 C（bar/mask）与 D（settings）调用 ——

    pub fn shared(&self) -> &Arc<SharedUi> {
        &self.shared
    }

    /// 快照全部界面文案
    pub fn texts(&self) -> I18nTexts {
        I18nTexts::from_i18n(&self.core.i18n)
    }

    /// 悬浮条当前置顶态（bar.rs 置顶复选的显示值；来源 = UiJson.TopMost）
    pub fn topmost(&self) -> bool {
        self.core.ui.TopMost
    }

    /// 悬浮条拖动结束写回逻辑坐标并立即保存 UI.json
    /// （清单 B6/Q9：v2 保存分支死代码，v3 修复为拖动结束即存）
    pub fn set_bar_position(&mut self, left: f64, top: f64) {
        self.core.ui.CoutdownWindowLeft = eyenurse_core::config::StjF64(left);
        self.core.ui.CoutdownWindowTop = eyenurse_core::config::StjF64(top);
        self.save_ui_json();
    }

    pub fn settings_open(&self) -> bool {
        self.settings_open
    }

    /// 托盘菜单当前展示态（bar 右键菜单与托盘共用同一组 pause/resume 互换；
    /// v2 悬浮条 ContextMenu = App.Menu 是同一个对象，状态天然一致）
    pub fn tray_paused_shown(&self) -> bool {
        self.tray.is_paused_shown()
    }

    /// 托盘/悬浮条右键菜单共用的动作入口（v2 同一个 Menu → 同一组 Click 处理）
    pub fn tray_action(&mut self, action: TrayAction) {
        self.apply_tray_action(action);
    }

    // —— 动作（托盘/遮罩按钮/设置窗共用，全部同线程直接调用）——

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
        // 【有意偏差 Q4/C10】v2 用启动时快照；v3 现枚举显示器（更正确）
        let monitors = crate::win::monitors::get_monitor_infos();
        self.begin_rest(monitors);
        self.lock_countdown = self.core.setting.RestDuration.as_duration();
        // 基准：首个 1s 递减发生在 +1s（v2 先显示满值再 Task.Delay(1000)）
        self.last_rest_tick = Some(now);
    }

    /// timer.stop(now)；返回 false 时补一次 poll（§4.2 怪癖 1）
    /// （§5.6 冻结契约，供后续接线消费；骨架期允许暂未被调用）
    #[allow(dead_code)]
    pub fn stop_timer(&mut self) {
        let now = Instant::now();
        if !self.core.timer.stop(now) {
            // v2 Stop() 未启动也会发一次 Elapsed
            let events = self.core.timer.poll(now);
            self.apply_timer_events(events);
        }
    }

    /// timer.pause(now) 成功才置 paused 展示态并 poll（§4.2 怪癖 2、§6.3 门控 D8）
    pub fn pause(&mut self) -> bool {
        let now = Instant::now();
        // 门控：休息中 timer 已 stop → pause 返回 false → 不进入暂停展示（清单 D8）
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
    pub fn resume(&mut self) -> bool {
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
    /// （v2 EyeNurseViewModel.Reset cs:229-234 = if IsPaused Resume + Reset；
    /// §4.2 怪癖 3，清单 D7/A13。漏掉 resume 会卡在暂停展示）
    pub fn reset_timer(&mut self) {
        if self.is_paused {
            self.resume();
        }
        let now = Instant::now();
        self.core.timer.reset(now);
        // v2 Reset 内部立即 CheckTrigger（怪癖 3）→ 立即 poll，悬浮条即时回满
        let events = self.core.timer.poll(now);
        self.apply_timer_events(events);
    }

    /// 写 UI.json + 视口层级
    pub fn toggle_topmost(&mut self, on: bool) {
        self.core.ui.TopMost = on;
        self.save_ui_json();
        let level = if on {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        };
        self.ctx.send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            egui::ViewportCommand::WindowLevel(level),
        );
    }

    pub fn show_settings_window(&mut self, on: bool) {
        // 一致性差异 #1：窗口已存在时 v2 只还原+激活（MainViewModel.ShowWindow：
        // Minimized → Normal、!IsActive → Activate），WebView2 前端状态原样保留——
        // 不得用磁盘配置整体重置 SettingsDraft 丢弃用户未保存的编辑。
        // 草稿只在窗口真正（重新）打开时刷新（等价 v2 重建窗口后前端重新 GetSettings）。
        if on && self.settings_open {
            self.ctx
                .send_viewport_cmd_to(ui::settings_id(), egui::ViewportCommand::Minimized(false));
            self.ctx
                .send_viewport_cmd_to(ui::settings_id(), egui::ViewportCommand::Focus);
            return;
        }
        self.settings_open = on;
        if on {
            // 打开时从当前配置刷新草稿（D 的编辑基线）
            let draft = ui::settings::SettingsDraft::from_setting(&self.core.setting);
            *self
                .shared
                .settings
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = draft;
        }
    }

    /// 写 Languages.json + i18n + tray 文案（v2 ClientApi.SetCurrentLanguage）
    pub fn set_language(&mut self, lan: &str) {
        self.core.i18n.set_language(lan);
        self.core.lang.CurrentLan = Some(lan.to_string());
        let _ = self.core.cfg.save(&self.core.lang);
        let texts = self.texts();
        self.tray.update_texts(&texts);
    }

    /// ShellExecute 打开关于页（与 v2 相同 URL）
    pub fn open_about(&mut self) {
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

    /// 根视口 Close（run_and_return=true → run_native 返回）
    pub fn exit(&mut self) {
        self.save_ui_json();
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    // —— B 内部 ——

    /// §6.1 第 1 步：WTS/显示器事件 → AppModel 方法（§6.3 会话锁）
    pub fn drain_win_events(&mut self) {
        while let Ok(ev) = self.rx_win.try_recv() {
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
                    // 一致性差异 #5：休息中显示器布局变化 → 立即重枚举 rcWork 并更新遮罩
                    // 目标，本帧 show_viewports 即按新几何注册视口，correct_geometry 的
                    // 校正目标随之更新——否则它按休息开始时的旧 rcWork 反复拉回被系统
                    // 移动的遮罩窗口，来回抖动。（v2 无此处理：窗口摆放后不再校正；
                    // v3 的收敛式校正必须消费该事件才有新鲜 target。）
                    // 枚举结果为空（切换瞬态）时保留旧值，避免遮罩中途消失。
                    if self.is_resting {
                        let monitors = crate::win::monitors::get_monitor_infos();
                        if !monitors.is_empty() {
                            self.mask_mut().monitors = monitors;
                        }
                    }
                }
            }
        }
    }

    /// §6.1 第 2/3 步：排空托盘菜单/托盘事件与跨视口动作
    fn drain_tray_events(&mut self) {
        use tray_icon::TrayIconEvent;
        while let Ok(ev) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if let Some(action) = crate::tray::menu_action(&ev.id.0) {
                self.apply_tray_action(action);
            }
        }
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            // v2 双击托盘 → 显示悬浮条（App.xaml.cs:162-165）；v3 bar 常驻可见，
            // 动作改为聚焦 root（§6.1）
            if matches!(ev, TrayIconEvent::DoubleClick { .. }) {
                self.ctx
                    .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
            }
        }
    }

    fn apply_tray_action(&mut self, action: TrayAction) {
        match action {
            TrayAction::About => self.open_about(),
            TrayAction::Settings => self.show_settings_window(true),
            TrayAction::Pause => {
                // 清单 Q12：菜单可见性**无条件**互换（v2 先翻菜单再调 vm.Pause()），
                // 展示态按 pause() 返回值门控（D8）
                self.tray.set_paused(true);
                self.pause();
            }
            TrayAction::Resume => {
                self.tray.set_paused(false);
                self.resume();
            }
            TrayAction::Reset => self.reset_timer(),
            TrayAction::RestNow => self.rest_now(),
            TrayAction::Exit => self.exit(),
        }
    }

    fn drain_actions(&mut self) {
        while let Ok(action) = self.actions_rx.try_recv() {
            match action {
                UiAction::RestDone => self.end_rest_and_restart(),
                UiAction::SaveSettings(s) => self.apply_settings(s),
                UiAction::ResetSettings => {
                    // 一致性差异 #3（清单 F11）：v2 ClientApi.ResetSettings =
                    // new Setting() 默认值 → SaveUserConfig → ApplySetting（自启 +
                    // SettingChanged→Init 重启计时器）→ 返回 json 刷新前端。
                    // apply_settings 逐项等价（写盘 + 自启 + 重启计时器 + 草稿整体重置）。
                    self.apply_settings(SettingJson::default());
                }
                UiAction::ToggleTopmost(on) => self.toggle_topmost(on),
                UiAction::CloseSettings => self.show_settings_window(false),
                UiAction::SetLanguage(lan) => self.set_language(&lan),
            }
        }
    }

    /// §6.1 第 4 步：1Hz 心跳
    pub fn tick_timer(&mut self, now: Instant) {
        if self.is_resting {
            // 休息推进：每 tick 固定减 1s（对齐 v2 Task.Delay(1000) 粒度）。
            // logic 会因输入事件以 >1Hz 运行（遮罩/悬浮条上的鼠标移动即触发重绘），
            // 必须用真实时间门控到 1Hz，否则倒计时被加速（v2 无此问题）。
            let due = self
                .last_rest_tick
                .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1));
            if due {
                self.last_rest_tick = Some(now);
                // 已为 0 再过一拍才结束（Duration 无负值，等价 v2 `TotalSeconds < 0`，
                // 显示走到 00:00 并停留一整秒，总时长 = RestDuration + 1s，清单 C6/Q2）
                if self.lock_countdown.is_zero() {
                    self.end_rest_and_restart();
                    return;
                }
                self.lock_countdown = self.lock_countdown.saturating_sub(Duration::from_secs(1));
            }
        } else if !self.is_paused {
            for ev in self.core.timer.poll(now) {
                match ev {
                    TimerEvent::Elapsed { remaining } => {
                        let mut bar = self.bar_mut();
                        bar.main_countdown = remaining;
                    }
                    TimerEvent::Trigger => {
                        self.rest_now();
                        return; // rest_now 已切换状态，本帧到此为止
                    }
                }
            }
        }
        // 暂停时长 1Hz 累加（v2 PausedTime）
        if let Some(since) = self.paused_since {
            let mut bar = self.bar_mut();
            bar.paused_time = Some(now.duration_since(since));
        }
    }

    pub fn begin_rest(&mut self, monitors: Vec<MonitorRects>) {
        let countdown = self.lock_countdown;
        {
            let mut mask = self.mask_mut();
            mask.open = true;
            mask.monitors = monitors;
            mask.lock_countdown = countdown;
        }
        self.bar_mut().is_resting = true;
    }

    pub fn end_rest(&mut self) {
        // 仅收尾遮罩展示（遮罩解锁后计时器重启由 end_rest_and_restart 完成）
        {
            let mut mask = self.mask_mut();
            mask.open = false;
        }
        self.bar_mut().is_resting = false;
    }

    /// end_rest + 计时器收尾：mask.open=false; timer.reset(now); 立即 poll;
    /// timer.start(now); resting=false（§6.2 end_rest）
    fn end_rest_and_restart(&mut self) {
        let now = Instant::now();
        self.end_rest();
        self.lock_countdown = Duration::ZERO;
        self.last_rest_tick = None;
        self.is_resting = false;
        // 【有意偏差】若休息开始时处于暂停（“暂停 → 立刻休息”序列），v2 的 Reset 在
        // timer 已 stop 时调 Resume() 会失败，IsPaused 卡在 true（PausedTime 持续累加、
        // 菜单停在“恢复”），需手动点一次恢复才脱困——v2 缺陷。v3 在休息收尾时干净退出
        // 暂停展示，菜单同步翻回“暂停”（set_paused 幂等，未暂停时是无害空操作）。
        self.is_paused = false;
        self.paused_since = None;
        self.tray.set_paused(false);
        self.core.timer.reset(now);
        // v2：Reset 后立即刷新（怪癖 3）
        let events = self.core.timer.poll(now);
        self.apply_timer_events(events);
        self.core.timer.start(now);
        self.refresh_shared_snapshots();
    }

    /// 设置窗保存路径（UiAction::SaveSettings）：写盘 + 应用（对齐 v2 ApplySetting）
    fn apply_settings(&mut self, s: SettingJson) {
        self.startup.set_enabled(s.RunWhenStarts);
        self.core.setting = s;
        let _ = self.core.cfg.save(&self.core.setting);
        *self
            .shared
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner()) =
            ui::settings::SettingsDraft::from_setting(&self.core.setting);
        // 重启计时器应用新间隔（一致性差异 #7：对齐 v2 SettingChanged → Init 的
        // 顺序——先 Stop 旧计时器，再 new GlobalTimerService；间隔 <1s 时 v2 构造
        // 抛 ArgumentOutOfRangeException 且旧计时器已停 → 提醒停摆直到下次合法
        // 保存/重启（v2 真实行为，均无界面提示）。此前 v3 在构造失败时保留旧计时器，
        // 与 v2 不同；现按 v2 逐项对齐。）
        let now = Instant::now();
        if !self.core.timer.stop(now) {
            // v2 Stop() 未启动也会发一次 CheckTrigger（怪癖 1）
            let events = self.core.timer.poll(now);
            self.apply_timer_events(events);
        }
        let interval = self.core.setting.RestInterval.as_duration();
        if let Ok(mut t) = GlobalTimer::new(interval) {
            t.start(now);
            self.core.timer = t;
            let mut bar = self.bar_mut();
            bar.main_interval = interval;
        }
    }

    /// 事件应用到展示状态（Elapsed → 悬浮条；Trigger 由调用方 rest_now 处理）
    fn apply_timer_events(&mut self, events: Vec<TimerEvent>) {
        for ev in events {
            if let TimerEvent::Elapsed { remaining } = ev {
                self.bar_mut().main_countdown = remaining;
            }
        }
    }

    /// §6.1 第 5 步：SharedUi 各 Mutex 更新快照
    fn refresh_shared_snapshots(&mut self) {
        // 先取值再锁（MutexGuard 持有期间不能再次可变借用 self）
        let now = Instant::now();
        let resting = self.is_resting;
        // 暂停期间冻结剩余值（v2 暂停时无 Elapsed 事件，进度条与剩余不动，只切
        // PausedTime 文案）；core 的 next_trigger 在暂停期是固定时刻，直接读
        // remaining 会随真实时间缩水，与 v2 不符 → 暂停/休息时不读
        let remaining = if resting || self.is_paused {
            None
        } else {
            self.core.timer.remaining(now)
        };
        let interval = self.core.timer.interval();
        let paused = self.paused_since.map(|since| now.duration_since(since));
        {
            let mut bar = self.bar_mut();
            if !resting {
                if let Some(r) = remaining {
                    bar.main_countdown = r;
                }
                bar.main_interval = interval;
            }
            bar.paused_time = paused;
            bar.is_resting = resting;
        }
        let countdown = self.lock_countdown;
        let mut mask = self.mask_mut();
        mask.lock_countdown = countdown;
    }

    fn bar_mut(&mut self) -> std::sync::MutexGuard<'_, BarState> {
        self.shared.bar.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn mask_mut(&mut self) -> std::sync::MutexGuard<'_, MaskState> {
        self.shared.mask.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save_ui_json(&self) {
        let _ = self.core.cfg.save(&self.core.ui);
    }
}

/// logo.png → RGBA8（托盘 Icon 与 egui::IconData 共用）
pub fn load_logo_rgba() -> (Vec<u8>, u32, u32) {
    match image::load_from_memory(LOGO_PNG) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            (rgba.into_raw(), w, h)
        }
        Err(_) => {
            // 内嵌资源不可能解码失败；兜底 1×1 透明像素保持签名可用
            (vec![0, 0, 0, 0], 1, 1)
        }
    }
}

/// 配置 + i18n 装配（main.rs 与 AppModel::new 共用）：
/// 磁盘优先（保持 v2 “替换 json 即换文案”能力），exe 旁 Assets/Languages 不存在时
/// 回退内嵌副本（§4.5）。
pub fn load_i18n(_cfg: &ConfigStore, lang: &LanguagesJson) -> I18n {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("Assets").join("Languages")));
    let system = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    let current = lang.CurrentLan.as_deref();
    match dir {
        Some(dir) if dir.is_dir() => I18n::load(&dir, current, "en", &system),
        _ => I18n::load_embedded(current, "en", &system),
    }
}

/// exe 完整路径（自启注册表值数据；失败回退空串，v2 传参同源）
pub fn exe_path_string() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}
