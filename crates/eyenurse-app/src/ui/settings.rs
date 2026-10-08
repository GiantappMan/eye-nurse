//! 设置窗（实现者 D 所有）——内容 + 视口注册 `maybe_show` + `SettingsDraft` 类型定义。
//!
//! `SettingsDraft` 定义在本文件（架构文档 §1 跨所有权规则）：B 的 app.rs 只
//! `use crate::ui::settings::SettingsDraft;`，D 之后增删字段不改 B 的文件。
//! 字段对齐 v2 SettingFrontEnd（Setting.cs:52-66）：文本框里是字符串，保存时
//! `parse + clamp_like_v2` 成 `SettingJson`（clamp 时机见 core/time.rs）。
//!
//! ## 对照来源（git show v2.x）
//! - `src/eyenurse-client-ui/src/views/Settings.vue`：四个设置项 + 失焦 300ms 防抖保存 +
//!   保存中 spinner（标题右侧）+ 保存后用回包刷新显示。
//! - `components/layout/Index.vue` / `components/layout/components/SliderNav.vue`：
//!   可折叠左侧导航（默认展开、折叠后左缘浮钮 + 竖排“面板”、顶部导航 + 底部“设置”项、
//!   按语言换 logo）。
//! - `components/layout/components/Footer.vue`：页脚社交链接（GitHub/Bilibili/YouTube/订阅号）、
//!   中英文下拉（选中项 ✔）、copyright（v-html 渲染 `&copy;` 实体）。
//! - `locales/{zh,en}.json`：键全部消费（clientSettings/home/panel/settings/restInterval/
//!   restDuration/runWhenStarts/resetWhenSessionUnlock/copyright/zh/en）；
//!   `installNow`/`installWebview2Tips` 按架构 §4.5 保留在文案文件里但不再使用（v3 无 WebView2）。
//!
//! ## 关键决定（与 v2 的差异均有注记）
//! 1. 保存管线（清单 I3/I4/F9）：任一控件提交（失焦/回车/开关点击）→ 300ms 防抖 →
//!    `UiAction::SaveSettings` → 根视口 `apply_settings` 写盘并整体重置草稿（等价 v2
//!    `Object.assign(settings, 回包)` 的回写刷新）。`pending_since` 非空即“保存中”，
//!    草稿被根视口重置后自动清零 → 标题旁 spinner 消失。
//! 2. Q10/F10：开机启动勾选态显示**注册表实测值**（打开/保存重置后的首帧用
//!    `DesktopStartup::check` 覆盖草稿，落实 v2 ClientApi.GetSettings 注释声明但未生效的意图）。
//! 3. F6 负值：core `HmsSpan::parse` 丢符号保幅值（"-00:10:00" → 10 分钟，time.rs 注记），
//!    保存路径 parse + `clamp_like_v2` 输出 "00:10:00"，与 v2 `Math.Abs` 分量语义一致；
//!    天/非法输入两条路径同样与 v2 一致。
//! 4. 关窗时若有未到点的防抖修改则立即 flush（对齐 v2 关窗触发 blur → 防抖保存在存活的
//!    前端里仍会执行）；无编辑则不发，避免无谓重启计时器（v2 仅 blur 才保存）。
//! 5. 视图：v2 路由只有 /（Home.vue 静态文本 "EyeNurse2"）与 /settings 两个页面，v3 用
//!    `SettingsView` 复刻侧栏导航切换。默认落在设置页（有意偏差：托盘入口即“设置”，
//!    v2 打开窗口落在首页属路由残留行为）。
//! 6. 侧栏开合状态存 egui 临时内存（v2 存 localStorage；UI.json 字节兼容不允许加字段，
//!    不跨进程重启持久化）。
//! 7. 配色：v2 的 tailwind 类（teal-500/gray-*）在 C 的 theme.rs 里没有对应常量且该文件
//!    归 C 所有，为不改他人文件在此局部定义，值与 v2 class 逐一对应，联调后可上移 theme.rs。
//! 8. 侧栏 logo：v2 用 `logo.zh.svg`/`logo.en.svg`（egui 无 SVG 解码），改用同一份
//!    `assets/logo.png` 纹理，尺寸对齐 v2 h-8。
//! 9. 开关控件：egui 0.36.2 无原生 Switch（已核实 widgets 目录无该类型），用 painter
//!    自绘 44×24 圆角轨道 + 20 圆钮，交互（点击翻转）等价 v2 headlessui Switch。
//! 10. 第 5 行“恢复默认”（清单 F11，一致性差异 #3）：v2 的 ClientApi.ResetSettings
//!     有后端无前端入口；v3 落地入口 → UiAction::ResetSettings → 根视口
//!     apply_settings(SettingJson::default())，与 v2 后端语义逐项等价（写默认值盘 +
//!     自启 + 计时器重启 + 回包刷新=草稿整体重置）。文案键 restoreDefaults 为 v3 新增。

use crate::app::AppModel;
use crate::events::UiAction;
use eyenurse_core::autostart::DesktopStartup;
use eyenurse_core::config::{ConfigStore, LanguagesJson, SettingJson};
use eyenurse_core::HmsSpan;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// 常量
// ============================================================================

/// 自启注册表值名（Q6/G2：v3 用 "Eye Nurse 3"；与 app.rs 启动序列一致）
const AUTOSTART_KEY: &str = "Eye Nurse 3";
/// 防抖窗口（v2 Settings.vue：lodash/debounce 300ms）
const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);
/// “保存中”兜底超时（正常路径：根视口重置草稿即清零；通道异常时不永久转圈）
const SAVE_PENDING_TIMEOUT: Duration = Duration::from_secs(2);
/// 侧栏宽度（v2 w-64 = 256）
const SIDEBAR_WIDTH: f32 = 256.0;
/// 侧栏折叠后的窄条宽度（容纳展开按钮 + 竖排文字）
const SIDEBAR_COLLAPSED_WIDTH: f32 = 28.0;
/// 内容区左右留白（v2 sm:px-6 = 24）
const CONTENT_PADDING: f32 = 24.0;
/// 时长输入框宽度（v2 w-28 = 112），占位 `HH:mm:ss`
const INPUT_WIDTH: f32 = 112.0;
/// 顶栏高度（v2 h-16 = 64）
const HEADER_HEIGHT: f32 = 64.0;
/// 开关轨道尺寸（v2 h-6 w-11 = 24×44）
const SWITCH_SIZE: egui::Vec2 = egui::vec2(44.0, 24.0);

// v2 tailwind 色板（见模块注释第 7 条：局部定义，值与 Settings.vue/SliderNav.vue/Footer.vue
// 的 class 一一对应）
const TEAL_500: egui::Color32 = egui::Color32::from_rgb(0x14, 0xb8, 0xa6); // bg-teal-500 / border-teal-500
const TEAL_50: egui::Color32 = egui::Color32::from_rgb(0xf0, 0xfd, 0xfa); // bg-teal-50
const TEAL_700: egui::Color32 = egui::Color32::from_rgb(0x0f, 0x76, 0x6e); // text-teal-700
const GRAY_50: egui::Color32 = egui::Color32::from_rgb(0xf9, 0xfa, 0xfb); // bg-gray-50 / hover:bg-gray-50
const GRAY_200: egui::Color32 = egui::Color32::from_rgb(0xe5, 0xe7, 0xeb); // 开关底色 bg-gray-200 / 分隔线 divide-gray-200
const GRAY_300: egui::Color32 = egui::Color32::from_rgb(0xd1, 0xd5, 0xdb); // 开关关态悬停加深（bg-gray-300）
const GRAY_400: egui::Color32 = egui::Color32::from_rgb(0x9c, 0xa3, 0xaf); // text-gray-400（页脚图标/版权/导航图标）
const GRAY_500: egui::Color32 = egui::Color32::from_rgb(0x6b, 0x72, 0x80); // text-gray-500（折叠竖排文字/悬停加深）
const GRAY_900: egui::Color32 = egui::Color32::from_rgb(0x11, 0x18, 0x27); // text-gray-900（正文标签）

/// 页脚社交链接（v2 Footer.vue navigation 数组：名称/链接/图标字符）。
/// 全部为编译期 https 常量，`open_url` 另做 scheme 校验兜底。
const FOOTER_LINKS: [(&str, &str, &str); 4] = [
    ("GitHub", "https://github.com/DaZiYuan/eye-nurse", "\u{E62E}"), // egui 内置 GitHub 字形
    ("Bilibili", "https://space.bilibili.com/32676827", "\u{1F4FA}"), // 📺
    (
        "YouTube",
        "https://www.youtube.com/channel/UCS-nZhrfAz0osmKm2AchvFA",
        "\u{25B6}", // ▶
    ),
    ("订阅号", "https://giantapp.cn/images/dyh.png", "\u{1F4AC}"), // 💬
];
/// 首页导航图标（🏠；已用 cmap 工具验证 egui 内置 emoji 字体覆盖）
const HOME_ICON: &str = "\u{1F3E0}";
/// 设置导航图标（⚙，对应 v2 CogIcon；同上已验证）
const GEAR_ICON: &str = "\u{2699}";
/// 语言下拉选中标记（v2 Footer.vue CheckIcon）
const CHECK_MARK: &str = "\u{2714}"; // ✔

/// 设置视口的临时状态键（侧栏开合，进程内持久，见模块注释第 6 条）。
/// 注：egui 0.36 的 `Id::new` 非 const fn（已核实 id.rs:67），运行时构造。
const SIDEBAR_OPEN_KEY: &str = "settings_sidebar_open";
/// 侧栏 logo 纹理缓存键（避免每帧解码 png）
const LOGO_TEX_KEY: &str = "settings_logo_tex";
/// 居中一次性标记：只在窗口创建后的首帧居中（保存重置草稿不重居中，避免窗口跳位）
const CENTERED_KEY: &str = "settings_centered";

// ============================================================================
// 草稿类型（本文件所有权；B 只经 from_setting 构造/整体重置）
// ============================================================================

/// 主区视图（v2 路由：/ 首页、/settings 设置页）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsView {
    /// /settings（默认；有意偏差见模块注释第 5 条）
    #[default]
    Settings,
    /// /（v2 Home.vue：静态文本 "EyeNurse2"）
    Home,
}

/// 设置窗草稿（文本输入态；保存路径：parse + clamp → SettingJson → UiAction::SaveSettings）
///
/// 前五个字段对齐 v2 SettingFrontEnd；其余为 v3 视图/保存流水线的内存态
/// （不落盘，随 `from_setting` 重置回默认）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SettingsDraft {
    pub rest_interval: String,           // v2 SettingFrontEnd.RestInterval（默认空串）
    pub rest_duration: String,           // v2 SettingFrontEnd.RestDuration
    pub reset_timeout: String,           // v2 SettingFrontEnd.ResetTimeout
    pub run_when_starts: bool,           // v2 默认 true
    pub reset_when_session_unlock: bool, // v2 默认 false

    /// 当前导航视图（侧栏顶部“首页”/底部“设置”）
    pub view: SettingsView,
    /// 防抖保存截止时刻；Some = 有未落盘修改（v2 debounce 300ms）
    pub save_due: Option<Instant>,
    /// 已发送、待根视口回写草稿的保存（非空 = “保存中”，标题旁转圈）
    pub pending_since: Option<Instant>,
    /// close_requested 已 flush 过（防止关窗帧重复发送）
    pub close_flushed: bool,
    /// 打开/保存重置后的首帧快照已完成（注册表实测值 + 当前语言 + 居中）
    pub synced: bool,
    /// 页脚语言下拉的选中值（"zh"/"en"；首帧从 Languages.json 解析）
    pub current_language: String,
}

impl SettingsDraft {
    pub fn from_setting(s: &eyenurse_core::config::SettingJson) -> Self {
        Self {
            rest_interval: s.RestInterval.to_c_string(),
            rest_duration: s.RestDuration.to_c_string(),
            reset_timeout: s.ResetTimeout.to_c_string(),
            run_when_starts: s.RunWhenStarts,
            reset_when_session_unlock: s.ResetWhenSessionUnlock,
            ..Self::default()
        }
    }
}

/// 文本 → 数值（v2 `Setting.GetTimeSpan`：解析失败静默回默认；成功则 clamp）
fn parse_or_default(s: &str, default: HmsSpan) -> HmsSpan {
    HmsSpan::parse(s).map(HmsSpan::clamp_like_v2).unwrap_or(default)
}

/// 草稿 → 磁盘结构（v2 `new Setting(SettingFrontEnd)`；默认值与 Setting.cs:7-9 一致）
fn build_setting_json(d: &SettingsDraft) -> SettingJson {
    SettingJson {
        RestInterval: parse_or_default(&d.rest_interval, HmsSpan::from_hms(0, 45, 0)),
        RestDuration: parse_or_default(&d.rest_duration, HmsSpan::from_hms(0, 2, 30)),
        ResetTimeout: parse_or_default(&d.reset_timeout, HmsSpan::from_hms(0, 5, 0)),
        RunWhenStarts: d.run_when_starts,
        ResetWhenSessionUnlock: d.reset_when_session_unlock,
    }
}

/// 有未落盘的修改：排一个 300ms 防抖保存（重复提交会顺延，等价 debounce 语义）
fn schedule_save(draft: &mut SettingsDraft) {
    draft.save_due = Some(Instant::now() + SAVE_DEBOUNCE);
}

// ============================================================================
// 视口注册（AppModel::logic 每帧调用）
// ============================================================================

/// 由 `AppModel::logic` 每帧调用：设置窗开着 → 注册设置视口，
/// 含 close_requested → UiAction::CloseSettings（§5.7）。
pub fn maybe_show(ctx: &egui::Context, m: &mut AppModel) {
    if !m.settings_open() {
        return;
    }
    let shared = m.shared().clone();
    let texts = m.texts();

    // 打开/保存重置后的首帧：同步注册表实测值（Q10）与当前语言（页脚下拉），
    // 并决定本帧是否做一次居中（I1：v2 CenterScreen）。
    let recenter = {
        let mut draft = shared.settings.lock().unwrap_or_else(|e| e.into_inner());
        let recenter = !draft.synced;
        if recenter {
            sync_draft_snapshots(&mut draft);
        }
        recenter
    };
    let center_phys = if recenter { primary_monitor_center() } else { None };

    // 窗口图标（与托盘同源 logo.png；OnceLock 避免每帧解码）
    static ICON: std::sync::OnceLock<(Vec<u8>, u32, u32)> = std::sync::OnceLock::new();
    let (icon_rgba, iw, ih) = ICON.get_or_init(crate::app::load_logo_rgba);

    let builder = egui::ViewportBuilder::default()
        .with_title(texts.app_name.clone()) // v2 lan:Xaml.Key="app_name"
        .with_inner_size([900.0, 500.0]) // v2 MainWindow 900×500
        .with_resizable(true)
        .with_icon(egui::IconData {
            // 0.36 的 IconData 宽高为 u32（viewport.rs:183）
            width: *iw,
            height: *ih,
            rgba: icon_rgba.clone(),
        });

    ctx.show_viewport_deferred(
        crate::ui::settings_id(),
        builder,
        move |ui, _class| {
            settings_viewport_ui(ui, &shared, &texts, center_phys);
        },
    );
}

/// 打开/保存重置后的首帧同步（maybe_show 主线程侧，调用地已保证 `!synced`）：
/// - Q10/F10：开机启动勾选态 = 注册表实测值（v2 注释声明的意图）
/// - 当前语言：AppModel 未暴露 i18n（B 契约不可改），从 Languages.json 按同一套
///   解析规则推导（`current_language_from_disk`）
fn sync_draft_snapshots(draft: &mut SettingsDraft) {
    draft.run_when_starts = DesktopStartup::new(AUTOSTART_KEY, String::new()).check();
    draft.current_language = current_language_from_disk();
}

/// 当前语言快照（复刻 core `I18n::build` 的解析规则）：
/// Languages.json 的 CurrentLan；空/None → 系统语言 zh 开头 → "zh"，否则 "en"。
fn current_language_from_disk() -> String {
    let cfg = ConfigStore::v2_default();
    let lang: LanguagesJson = cfg.load();
    match lang.CurrentLan.as_deref().map(str::trim) {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => {
            let sys = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
            if sys.to_ascii_lowercase().starts_with("zh") {
                "zh".to_string()
            } else {
                "en".to_string()
            }
        }
    }
}

/// 主显示器物理中心（v2 WindowStartupLocation=CenterScreen）。
/// EnumDisplayMonitors 在 Windows 上首个枚举项即主显示器。
fn primary_monitor_center() -> Option<[i32; 2]> {
    let monitors = crate::win::monitors::get_monitor_infos();
    let m = monitors.first()?;
    Some([
        (m.rc_monitor[0] + m.rc_monitor[2]) / 2,
        (m.rc_monitor[1] + m.rc_monitor[3]) / 2,
    ])
}

/// 跨视口动作发出后唤醒 ROOT（架构 §6.1：deferred 视口输入不会执行根 logic）
fn wake_root(ctx: &egui::Context) {
    ctx.request_repaint_after_for(Duration::ZERO, egui::ViewportId::ROOT);
}

// ============================================================================
// 视口内容
// ============================================================================

/// 视口根：保存管线（防抖/保存中/关窗 flush）+ 三段布局（侧栏 / 页脚 / 中央内容）
fn settings_viewport_ui(
    ui: &mut egui::Ui,
    shared: &Arc<crate::app::SharedUi>,
    texts: &crate::tray::I18nTexts,
    center_phys: Option<[i32; 2]>,
) {
    let mut draft = shared.settings.lock().unwrap_or_else(|e| e.into_inner());

    // —— 防抖到点 → 发送保存（对齐 v2 debounced saveSettings：loading=true → SetSettings）——
    let now = Instant::now();
    if let Some(due) = draft.save_due {
        if now >= due {
            draft.save_due = None;
            draft.pending_since = Some(now);
            let setting = build_setting_json(&draft);
            shared.actions.0.send(UiAction::SaveSettings(setting)).ok();
            wake_root(ui.ctx());
        }
    }
    // “保存中”兜底超时（正常由根视口 apply_settings 整体重置草稿清零）
    if let Some(t) = draft.pending_since {
        if now.duration_since(t) > SAVE_PENDING_TIMEOUT {
            draft.pending_since = None;
        }
    }
    let saving = draft.pending_since.is_some();

    // —— 窗口创建后的首帧：一次居中（I1；命令次帧生效，误差一帧可接受）。
    //     居中标记走 egui 内存而非 draft.synced：保存重置草稿不应让窗口跳回屏幕中央 ——
    let centered_key = egui::Id::new(CENTERED_KEY);
    let already_centered = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(centered_key).unwrap_or(false));
    if !draft.synced {
        draft.synced = true;
        if let (Some([cx, cy]), false) = (center_phys, already_centered) {
            // 视口已创建，native_pixels_per_point 即所在屏的真实缩放（§7.4）
            let ppp = ui
                .ctx()
                .input(|i| i.viewport().native_pixels_per_point)
                .unwrap_or(1.0);
            let x = cx as f32 / ppp - 450.0; // 900/2
            let y = cy as f32 / ppp - 250.0; // 500/2
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
        }
        ui.ctx().data_mut(|d| d.insert_temp(centered_key, true));
    }

    // —— 关窗：未落盘的防抖修改立即 flush（对齐 v2 关窗触发 blur → 防抖保存仍执行）；
    //     无编辑不发，避免无谓重启计时器（v2 仅 blur 才保存）——
    let close_requested = ui.ctx().input(|i| i.viewport().close_requested());
    if close_requested && !draft.close_flushed {
        draft.close_flushed = true;
        if draft.save_due.is_some() {
            draft.save_due = None;
            let setting = build_setting_json(&draft);
            shared.actions.0.send(UiAction::SaveSettings(setting)).ok();
        }
        // 复位居中标记：下次打开=新窗口创建，重新按 CenterScreen 居中
        ui.ctx().data_mut(|d| d.insert_temp(centered_key, false));
        shared.actions.0.send(UiAction::CloseSettings).ok();
        wake_root(ui.ctx());
        return;
    }

    // —— 帧驱动：保存中转圈动画 / 防抖到点前唤醒本视口 ——
    if saving {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    } else if let Some(due) = draft.save_due {
        ui.ctx()
            .request_repaint_after(due.saturating_duration_since(now));
    }

    // —— 布局：左导航（256/28，可折叠）+ 底部页脚 + 中央内容 ——
    // 注：egui 0.36 以统一 Panel 取代 SidePanel/TopBottomPanel（show 直收 &mut Ui）
    let sidebar_open = ui
        .ctx()
        .data(|d| {
            d.get_temp::<bool>(egui::Id::new(SIDEBAR_OPEN_KEY))
                .unwrap_or(true)
        }); // v2 默认 true
    egui::Panel::left("settings_sidebar")
        .resizable(false)
        .exact_size(if sidebar_open {
            SIDEBAR_WIDTH
        } else {
            SIDEBAR_COLLAPSED_WIDTH
        })
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(1.0, GRAY_200)), // v2 ring-gray-900/5 近似
        )
        .show(ui, |ui| sidebar_ui(ui, texts, &mut draft, sidebar_open));

    egui::Panel::bottom("settings_footer")
        .resizable(false)
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::WHITE)
                .inner_margin(egui::Margin::symmetric(16, 16)), // v2 px-4 py-12 的收敛版（窗高有限）
        )
        .show(ui, |ui| footer_ui(ui, shared, texts, &mut draft));

    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(GRAY_50)) // v2 bg-gray-50
        .show(ui, |ui| {
            // v2 header：h-16 白条 + 底部细阴影（Index.vue）
            let (header, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), HEADER_HEIGHT),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(header, 0, egui::Color32::WHITE);
            ui.painter().line_segment(
                [header.left_bottom(), header.right_bottom()],
                egui::Stroke::new(1.0, GRAY_200),
            );
            ui.add_space(8.0);
            match draft.view {
                SettingsView::Settings => {
                    settings_list_ui(ui, shared, texts, &mut draft, saving);
                }
                SettingsView::Home => home_ui(ui),
            }
        });
}

/// 主区：v2 Home.vue（静态文本 "EyeNurse2"）
fn home_ui(ui: &mut egui::Ui) {
    ui.add_space(CONTENT_PADDING);
    ui.label(egui::RichText::new("EyeNurse2").size(14.0).color(GRAY_900));
}

/// 主区：设置列表（v2 Settings.vue：标题 + 保存中 spinner + 四行 divide-y；
/// v3 增加第 5 行“恢复默认”，清单 F11）
fn settings_list_ui(
    ui: &mut egui::Ui,
    shared: &Arc<crate::app::SharedUi>,
    texts: &crate::tray::I18nTexts,
    draft: &mut SettingsDraft,
    saving: bool,
) {
    egui::ScrollArea::vertical()
        .id_salt("settings_rows")
        .show(ui, |ui| {
            // 标题行：clientSettings + 保存中转圈（v2 animate-spin，仅保存期间出现）
            ui.horizontal(|ui| {
                ui.add_space(CONTENT_PADDING);
                ui.label(
                    egui::RichText::new(&texts.client_settings)
                        .size(18.0)
                        .strong()
                        .color(GRAY_900),
                );
                if saving {
                    ui.add(egui::Spinner::new().size(20.0));
                }
            });
            ui.add_space(4.0);

            // 行 1：开机启动开关
            setting_row(ui, &texts.run_when_starts, draft, |ui, draft| {
                if switch_toggle(ui, &mut draft.run_when_starts).changed() {
                    schedule_save(draft); // v2 Switch @blur → saveSettings
                }
            });

            // 行 2：锁屏后重置开关 +（开关打开时）HH:mm:ss 超时输入
            setting_row(ui, &texts.reset_when_session_unlock, draft, |ui, draft| {
                if switch_toggle(ui, &mut draft.reset_when_session_unlock).changed() {
                    schedule_save(draft);
                }
                if draft.reset_when_session_unlock {
                    ui.add_space(16.0); // v2 ml-4
                    if hms_input(ui, "reset_timeout", &mut draft.reset_timeout) {
                        schedule_save(draft);
                    }
                }
            });

            // 行 3：休息间隔
            setting_row(ui, &texts.rest_interval, draft, |ui, draft| {
                if hms_input(ui, "rest_interval", &mut draft.rest_interval) {
                    schedule_save(draft);
                }
            });

            // 行 4：休息时长
            setting_row(ui, &texts.rest_duration, draft, |ui, draft| {
                if hms_input(ui, "rest_duration", &mut draft.rest_duration) {
                    schedule_save(draft);
                }
            });

            // 行 5：恢复默认（清单 F11；一致性差异 #3）。语义 = v2 ClientApi.ResetSettings：
            // new Setting() 默认值 → SaveUserConfig → ApplySetting（自启 + 计时器重启）→
            // 回包刷新显示。发送 ResetSettings 由根视口 apply_settings(默认值) 落实；
            // pending_since 走与保存相同的“保存中”转圈，草稿被整体重置后自动消失。
            setting_row(ui, &texts.restore_defaults, draft, |ui, draft| {
                let btn = ui.add(
                    egui::Button::new(
                        egui::RichText::new(texts.restore_defaults.clone())
                            .size(13.0)
                            .color(GRAY_900),
                    ),
                );
                if btn.clicked() {
                    draft.save_due = None;
                    draft.pending_since = Some(Instant::now());
                    shared.actions
                        .0
                        .send(UiAction::ResetSettings)
                        .ok();
                    wake_root(ui.ctx());
                }
            });
        });
}

/// 一行设置项：左标签 + 右控件 + 行间分隔线（v2 divide-y + py-4）
fn setting_row(
    ui: &mut egui::Ui,
    label: &str,
    draft: &mut SettingsDraft,
    content: impl FnOnce(&mut egui::Ui, &mut SettingsDraft),
) {
    ui.horizontal(|ui| {
        ui.add_space(CONTENT_PADDING);
        ui.label(egui::RichText::new(label).size(14.0).color(GRAY_900));
        // 右侧控件区（v2 justify-between 的右半）：从右往左排
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(CONTENT_PADDING);
            content(ui, draft);
        });
    });
    ui.add_space(12.0);
    ui.separator(); // v2 divide-gray-200
    ui.add_space(12.0);
}

// ============================================================================
// 侧栏（v2 Index.vue + SliderNav.vue + Toggle.vue）
// ============================================================================

fn sidebar_ui(
    ui: &mut egui::Ui,
    texts: &crate::tray::I18nTexts,
    draft: &mut SettingsDraft,
    open: bool,
) {
    if open {
        // —— 头部：logo（v2 /logo.<lang>.svg → png，见模块注释第 8 条）+ 折叠按钮 ——
        ui.add_space(20.0); // v2 pt-5
        ui.horizontal(|ui| {
            ui.add_space(16.0); // v2 px-4
            let handle = logo_texture(ui);
            let tex_size = handle.size_vec2();
            let h = 32.0; // v2 h-8
            let w = if tex_size.y > 0.0 {
                (tex_size.x / tex_size.y * h).min(96.0)
            } else {
                h
            };
            ui.add(
                egui::Image::from_texture(egui::load::SizedTexture::new(
                    handle.id(),
                    egui::vec2(w, h),
                )),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("\u{00AB}") // «（折叠，v2 双箭头）
                                .size(16.0)
                                .color(GRAY_400),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::new(SIDEBAR_OPEN_KEY), false));
                }
            });
        });
        ui.add_space(20.0); // v2 mt-5

        // —— 顶部导航（v2 topNav；首页为唯一项，点击切到首页视图）——
        if nav_item(ui, HOME_ICON, &texts.home, draft.view == SettingsView::Home).clicked() {
            draft.view = SettingsView::Home;
        }

        // —— 弹性留白 + 底部导航（v2 border-t + p-2）——
        // 底部导航占高：分隔线 1 + 内边距 8*2 + 条目 36
        let bottom_h = 1.0 + 16.0 + 36.0;
        let remain = ui.available_height() - bottom_h;
        ui.add_space(remain.max(0.0));
        ui.separator(); // v2 border-t border-gray-200
        ui.add_space(8.0); // v2 p-2
        if nav_item(ui, GEAR_ICON, &texts.settings, draft.view == SettingsView::Settings)
            .clicked()
        {
            draft.view = SettingsView::Settings;
        }
    } else {
        // —— 折叠态：左缘窄条，76% 高度处展开按钮（v2 Toggle.vue），下方竖排“面板” ——
        let total_h = ui.available_height();
        ui.add_space((total_h * 0.70).max(0.0));
        ui.vertical_centered(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("\u{00BB}") // »（展开）
                            .size(14.0)
                            .color(GRAY_400),
                    )
                    .frame(false),
                )
                .clicked()
            {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new(SIDEBAR_OPEN_KEY), true));
            }
            ui.add_space(8.0);
            // v2 [writing-mode:vertical-lr] 的等价：逐字竖排
            for ch in texts.panel.chars() {
                ui.label(egui::RichText::new(ch).size(12.0).color(GRAY_500));
            }
        });
    }
}

/// 导航条目（v2 RouterLink：border-l-4 + px-3 py-2 + 图标 + 文案）。
/// current：teal 高亮（bg-teal-50/border-teal-500/text-teal-700），否则灰。
fn nav_item(ui: &mut egui::Ui, icon: &str, label: &str, current: bool) -> egui::Response {
    let desired = egui::vec2(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if current {
            painter.rect_filled(rect, 0, TEAL_50);
            // 左侧 4pt 高亮条（v2 border-l-4 border-teal-500）
            painter.rect_filled(
                egui::Rect::from_min_size(rect.left_top(), egui::vec2(4.0, rect.height())),
                0,
                TEAL_500,
            );
        } else if response.hovered() {
            painter.rect_filled(rect, 0, GRAY_50); // v2 hover:bg-gray-50
        }
        let cy = rect.center().y;
        let mut x = rect.left() + 12.0 + 4.0; // px-3 + border-l-4
        painter.text(
            egui::pos2(x, cy),
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(18.0),
            if current { TEAL_500 } else { GRAY_400 }, // v2 图标色 current?teal-500:gray-400
        );
        x += 30.0; // 图标宽 + mr-3
        painter.text(
            egui::pos2(x, cy),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(14.0),
            if current { TEAL_700 } else { GRAY_900 },
        );
    }
    response
}

/// 侧栏 logo 纹理（解码结果缓存进 egui 临时内存，避免每帧解 png）
fn logo_texture(ui: &egui::Ui) -> egui::TextureHandle {
    let id = egui::Id::new(LOGO_TEX_KEY);
    if let Some(handle) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return handle;
    }
    let image = match image::load_from_memory(crate::app::LOGO_PNG) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width() as usize, rgba.height() as usize);
            egui::ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw())
        }
        Err(_) => egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]), // 内嵌资源不会失败；兜底保签名
    };
    let handle = ui
        .ctx()
        .load_texture("settings_logo", image, egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|d| d.insert_temp(id, handle.clone()));
    handle
}

// ============================================================================
// 页脚（v2 Footer.vue：社交链接 + 语言下拉 + copyright）
// ============================================================================

fn footer_ui(
    ui: &mut egui::Ui,
    shared: &Arc<crate::app::SharedUi>,
    texts: &crate::tray::I18nTexts,
    draft: &mut SettingsDraft,
) {
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            ui.add_space(24.0); // v2 mt-8 的收敛版
            // 社交链接：图标按钮（v2 text-gray-400 hover:text-gray-500），悬停显名称（v2 sr-only）
            for (name, url, icon) in FOOTER_LINKS {
                icon_link(ui, icon, name, url);
                ui.add_space(24.0); // v2 space-x-6
            }
            language_combo(ui, shared, texts, draft);
        });
        ui.add_space(16.0); // v2 mt-8 的收敛版
        // copyright：v2 经 v-html 渲染，`&copy;` 实体显示为 ©；这里做等价替换
        ui.label(
            egui::RichText::new(texts.copyright.replace("&copy;", "\u{00A9}"))
                .size(13.0)
                .color(GRAY_400),
        );
    });
}

/// 页脚社交图标（点击用系统默认浏览器打开；仅允许 https 常量链接）
fn icon_link(ui: &mut egui::Ui, icon: &str, name: &str, url: &str) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    let color = if response.hovered() { GRAY_500 } else { GRAY_400 }; // v2 hover:text-gray-500
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(22.0),
        color,
    );
    let response = response.on_hover_text(name); // v2 sr-only 名称
    if response.clicked() {
        open_url(url);
    }
}

/// 中英文切换下拉（v2 Listbox：列出可用语言，选中项带 ✔，切换即 SetCurrentLanguage）
fn language_combo(
    ui: &mut egui::Ui,
    shared: &Arc<crate::app::SharedUi>,
    texts: &crate::tray::I18nTexts,
    draft: &mut SettingsDraft,
) {
    ui.add_space(12.0);
    // v2 $i18n.availableLocales = 内置 locales 目录 = zh/en 两个；显示名取同键文案
    let options = [("zh", &texts.zh), ("en", &texts.en)];
    let current_display = options
        .iter()
        .find(|(code, _)| *code == draft.current_language)
        .map(|(_, display)| display.to_string())
        .unwrap_or_else(|| draft.current_language.clone());
    egui::ComboBox::from_id_salt("settings_language")
        .width(INPUT_WIDTH) // v2 w-28
        .selected_text(
            egui::RichText::new(current_display)
                .size(14.0)
                .color(GRAY_500), // v2 text-gray-600 近似
        )
        .show_ui(ui, |ui| {
            for (code, display) in options {
                let selected = draft.current_language == code;
                let label = if selected {
                    format!("{CHECK_MARK} {display}") // v2 选中项左侧 CheckIcon
                } else {
                    format!("    {display}")
                };
                if ui
                    .selectable_label(selected, egui::RichText::new(label).size(14.0))
                    .clicked()
                    && !selected
                {
                    draft.current_language = code.to_string(); // 乐观更新（v2 watch(locale)）
                    shared.actions.0.send(UiAction::SetLanguage(code.to_string())).ok();
                    wake_root(ui.ctx()); // v2 即时生效：写 Languages.json + 刷托盘文案
                }
            }
        });
}

// ============================================================================
// 控件
// ============================================================================

/// 开关（v2 headlessui Switch：h-6 w-11 圆角轨道 + h-5 w-5 白色圆钮，
/// 开=teal-500 / 关=gray-200；点击翻转并 mark_changed）
fn switch_toggle(ui: &mut egui::Ui, checked: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(SWITCH_SIZE, egui::Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let bg = if *checked {
            TEAL_500
        } else if response.hovered() {
            GRAY_300
        } else {
            GRAY_200
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 12, bg); // 全圆角轨道
        let knob_r = (SWITCH_SIZE.y - 4.0) / 2.0; // 直径 20（v2 h-5 w-5）
        let knob_x = if *checked {
            rect.right() - 2.0 - knob_r
        } else {
            rect.left() + 2.0 + knob_r
        };
        painter.circle_filled(egui::pos2(knob_x, rect.center().y), knob_r, egui::Color32::WHITE);
    }
    response
}

/// HH:mm:ss 时长输入（v2 w-28 + placeholder="HH:mm:ss"）。
/// 返回是否应提交保存：失焦或回车（清单 I3）。
fn hms_input(ui: &mut egui::Ui, id_salt: &str, value: &mut String) -> bool {
    // TextEdit::show 返回 TextEditOutput，交互 Response 在 .response（Deref 到 egui::Response）
    let output = egui::TextEdit::singleline(value)
        .hint_text("HH:mm:ss")
        .desired_width(INPUT_WIDTH)
        .font(egui::FontId::proportional(14.0))
        .id_salt(egui::Id::new(id_salt))
        .show(ui);
    let response = &output.response;
    // 注：egui 0.36 的 input 读取只在 Context 上（Ui 无 input 方法）
    response.lost_focus()
        || (response.has_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)))
}

/// 用系统默认浏览器打开链接（v2 openExternal；对齐 app.rs open_about 的 ShellExecute 用法）。
/// 防御性约束：仅放行 https 且仅限编译期常量表里的链接。
fn open_url(url: &str) {
    if !url.starts_with("https://") || !FOOTER_LINKS.iter().any(|(_, u, _)| *u == url) {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        use windows::core::{w, HSTRING};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        // SAFETY：仅调起系统浏览器打开白名单内 https 链接；返回句柄被忽略（与 v2 Process.Start 同语义）
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &HSTRING::from(url),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {}
}

// ============================================================================
// 单元测试（保存路径纯逻辑，不依赖 UI）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_setting_json_matches_v2_constructor() {
        // 正常路径：原样通过（v2 Setting(SettingFrontEnd) 无 clamp 触发）
        let d = SettingsDraft {
            rest_interval: "00:45:00".into(),
            rest_duration: "00:02:30".into(),
            reset_timeout: "00:05:00".into(),
            run_when_starts: true,
            reset_when_session_unlock: false,
            ..Default::default()
        };
        let s = build_setting_json(&d);
        assert_eq!(s.RestInterval.to_c_string(), "00:45:00");
        assert_eq!(s.RestDuration.to_c_string(), "00:02:30");
        assert_eq!(s.ResetTimeout.to_c_string(), "00:05:00");
        assert!(s.RunWhenStarts);
        assert!(!s.ResetWhenSessionUnlock);
    }

    #[test]
    fn invalid_input_falls_back_to_default_like_v2() {
        // "abc" → 默认（v2 TimeSpan.TryParse 失败走 defaultTs）
        let d = SettingsDraft {
            rest_interval: "abc".into(),
            ..Default::default()
        };
        let s = build_setting_json(&d);
        assert_eq!(s.RestInterval, HmsSpan::from_hms(0, 45, 0));
    }

    #[test]
    fn day_and_short_format_are_clamped_like_v2() {
        // "1.02:03:04" → ≥1 天 → 丢天取分量 02:03:04（v2 Setting.cs:28-29）
        let d = SettingsDraft {
            rest_interval: "1.02:03:04".into(),
            ..Default::default()
        };
        assert_eq!(
            build_setting_json(&d).RestInterval,
            HmsSpan::from_hms(2, 3, 4)
        );
        // g 短格式 "45:00" = 45 小时 ≥1 天 → 分量 21:00:00（Hours=45%24）
        let d = SettingsDraft {
            rest_interval: "45:00".into(),
            ..Default::default()
        };
        assert_eq!(
            build_setting_json(&d).RestInterval,
            HmsSpan::from_hms(21, 0, 0)
        );
    }

    #[test]
    fn negative_input_drops_sign_keeps_magnitude_like_v2() {
        // 清单 F6：v2 "-00:10:00" → `Math.Abs` 分量 → "00:10:00"。
        // core parse 丢符号保幅值（time.rs 冻结签名的注记），clamp 不变形 → 与 v2 一致。
        let d = SettingsDraft {
            rest_interval: "-00:10:00".into(),
            ..Default::default()
        };
        let s = build_setting_json(&d);
        assert_eq!(s.RestInterval.to_c_string(), "00:10:00");
    }

    #[test]
    fn from_setting_roundtrip_and_reset_flags() {
        let s = SettingJson {
            RunWhenStarts: false,
            ..SettingJson::default()
        };
        let d = SettingsDraft::from_setting(&s);
        assert_eq!(d.rest_interval, "00:45:00");
        assert!(!d.run_when_starts);
        // 内存态字段重置回默认（B 在打开/保存时整体重置草稿）
        assert_eq!(d.view, SettingsView::Settings);
        assert_eq!(d.save_due, None);
        assert_eq!(d.pending_since, None);
        assert!(!d.close_flushed);
        assert!(!d.synced);
        assert_eq!(d.current_language, "");
    }

    #[test]
    fn debounce_is_300ms_and_reschedules() {
        let mut d = SettingsDraft::default();
        let before = Instant::now();
        schedule_save(&mut d);
        let due = d.save_due.unwrap();
        // 截止 = 提交时刻 + 300ms（宽松界防调度抖动）
        let window = due - before;
        assert!(window >= Duration::from_millis(250) && window <= Duration::from_millis(350));
        std::thread::sleep(Duration::from_millis(5));
        schedule_save(&mut d); // 再次提交顺延截止（等价 debounce）
        assert!(d.save_due.unwrap() > due);
    }
}
