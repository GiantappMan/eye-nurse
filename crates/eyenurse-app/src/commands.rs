//! IPC 命令（实现者 D 所有，tauri 架构文档 §4.1；对齐 v2 `Apis/ClientApi.cs` 语义，
//! 结构化参数取代 JSON 字符串）。
//!
//! 全部 `#[tauri::command] async fn`（async 是硬要求：tauri 明文警告同步 command
//! 里创建窗口会死锁，§2；转发给 CoreLoop 的应答通道阻塞发生在 async runtime
//! 工作线程上，可接受）。
//!
//! **取数路径规则（§4.1 评审修订）**：凡依赖 CoreLoop 独占状态（I18n 实例、当前
//! SettingJson）的命令一律走 `events::CoreMsg::Get*` 应答变体——本模块不持有任何
//! 业务状态句柄；唯一例外是 `get_autostart` 的无状态注册表查询（`DesktopStartup`
//! 无共享状态，命令线程直调不经 CoreLoop）。

use crate::events::CoreMsg;
use crossbeam_channel::Sender;
use eyenurse_core::autostart::DesktopStartup;
use eyenurse_core::config::SettingJson;
use eyenurse_core::HmsSpan;
use std::time::Duration;
use tauri::State;

/// CoreLoop 总线的命令侧句柄（main 里 `.manage(CoreHandle { tx })` 注入）
pub struct CoreHandle {
    pub tx: Sender<CoreMsg>,
}

/// IPC 应答兜底超时（§4.1 骨架）：CoreLoop 存活但漏回包时，2s 后返回 Err 而不是
/// 永久挂住工作线程（CoreLoop panic/退出路径会 drop Sender → Disconnected → 同样 Err）
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// 自启注册表值名（与 main 启动序列一致：v3 用 "Eye Nurse 3"，清单 Q6/G2）
const AUTOSTART_KEY: &str = "Eye Nurse 3";

/// 设置项 DTO（字段名与 SettingJson 一致，零映射；serde 按字段名输出 PascalCase，
/// 与 Setting.json 同形）。`Settings` 键语义对应 v2 ClientApi.GetSettings/SetSettings。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(non_snake_case)]
pub struct SettingsDto {
    /// .NET "c" 格式字符串（HmsSpan::to_c_string）
    pub RestInterval: String,
    pub RestDuration: String,
    pub ResetTimeout: String,
    /// get=注册表实测；set=开关值
    pub RunWhenStarts: bool,
    pub ResetWhenSessionUnlock: bool,
}

impl SettingsDto {
    pub fn of(s: &SettingJson) -> Self {
        Self {
            RestInterval: s.RestInterval.to_c_string(),
            RestDuration: s.RestDuration.to_c_string(),
            ResetTimeout: s.ResetTimeout.to_c_string(),
            RunWhenStarts: s.RunWhenStarts,
            ResetWhenSessionUnlock: s.ResetWhenSessionUnlock,
        }
    }

    /// 文本 → 数值（egui 版 ui/settings.rs `build_setting_json` 的移植，落在命令侧）：
    /// `HmsSpan::parse + clamp_like_v2`，解析失败静默回默认（v2 Setting.GetTimeSpan：
    /// 非法回默认、负值取幅、≥1 天丢天取分量）。页面不重复实现解析。
    pub fn parse_and_clamp(&self) -> SettingJson {
        fn parse_or_default(s: &str, default: HmsSpan) -> HmsSpan {
            HmsSpan::parse(s).map(HmsSpan::clamp_like_v2).unwrap_or(default)
        }
        SettingJson {
            RestInterval: parse_or_default(&self.RestInterval, HmsSpan::from_hms(0, 45, 0)),
            RestDuration: parse_or_default(&self.RestDuration, HmsSpan::from_hms(0, 2, 30)),
            ResetTimeout: parse_or_default(&self.ResetTimeout, HmsSpan::from_hms(0, 5, 0)),
            RunWhenStarts: self.RunWhenStarts,
            ResetWhenSessionUnlock: self.ResetWhenSessionUnlock,
        }
    }
}

/// 经 CoreLoop 取数/执行并等应答（bounded(1) + recv_timeout 兜底，§4.1）
fn ask_core<T>(
    state: &State<'_, CoreHandle>,
    make: impl FnOnce(crate::events::Reply<T>) -> CoreMsg,
) -> Result<T, String> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    state
        .tx
        .send(make(tx))
        .map_err(|_| "CoreLoop 已退出".to_string())?;
    rx.recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| "CoreLoop 未在时限内应答".to_string())
}

/// 注册表实测自启态（唯一的命令线程直查例外；get_settings 内的覆盖值同源）
fn autostart_now() -> bool {
    DesktopStartup::new(AUTOSTART_KEY, "").check()
}

/// v2 `GetSettings`：CoreLoop 内存中的当前 SettingJson + RunWhenStarts 用注册表
/// 实测值覆盖（Q10/F10）。返回结构化对象而非 JSON 字符串。
#[tauri::command]
pub async fn get_settings(state: State<'_, CoreHandle>) -> Result<SettingsDto, String> {
    let mut setting: SettingJson = ask_core(&state, CoreMsg::GetSettings)?;
    setting.RunWhenStarts = autostart_now();
    Ok(SettingsDto::of(&setting))
}

/// v2 `SetSettings`：字符串字段经 `HmsSpan::parse + clamp_like_v2` →
/// `CoreMsg::SaveSettings`（写盘 + 自启 + 计时器按新间隔重建）→ 返回应用后的 DTO，
/// 页面用回包刷新（等价 v2 `Object.assign(settings, 回包)`）。
#[tauri::command]
pub async fn set_settings(
    state: State<'_, CoreHandle>,
    dto: SettingsDto,
) -> Result<SettingsDto, String> {
    let setting = dto.parse_and_clamp();
    let applied: SettingJson = ask_core(&state, |tx| CoreMsg::SaveSettings(setting, tx))?;
    Ok(SettingsDto::of(&applied))
}

/// v2 `ResetSettings`（F11）：`SettingJson::default()` 走同一应用路径，回包刷新。
#[tauri::command]
pub async fn reset_settings(state: State<'_, CoreHandle>) -> Result<SettingsDto, String> {
    let applied: SettingJson = ask_core(&state, CoreMsg::ResetSettings)?;
    Ok(SettingsDto::of(&applied))
}

/// 新增专用口：注册表实测自启态（无共享状态，命令线程直查，§4.1）
#[tauri::command]
pub async fn get_autostart() -> bool {
    autostart_now()
}

/// v2 `GetCurrentLanguage`（CoreLoop 持有的 I18n 实例，Languages.json CurrentLan
/// 解析规则同 I18n::build）
#[tauri::command]
pub async fn get_current_language(state: State<'_, CoreHandle>) -> Result<String, String> {
    ask_core(&state, CoreMsg::GetCurrentLanguage)
}

/// v2 `SetCurrentLanguage`：写 Languages.json + i18n.set_language + 托盘
/// `update_texts` + 设置窗标题 `set_title` + 广播 `i18n-changed`（全部在 CoreLoop 侧）。
#[tauri::command]
pub async fn set_current_language(state: State<'_, CoreHandle>, lan: String) -> Result<(), String> {
    state
        .tx
        .send(CoreMsg::SetLanguage(lan))
        .map_err(|_| "CoreLoop 已退出".to_string())
}

/// 新增整包文案：`I18n::texts()`（current→fallback 合并），页面一次取全、
/// 语言切换时重取；settings/mask 页面的启动依赖。
#[tauri::command]
pub async fn get_texts(
    state: State<'_, CoreHandle>,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    ask_core(&state, CoreMsg::GetTexts)
}

/// 新增：可用语言清单 = `I18n::available()`（语言下拉 I5 的数据源）
#[tauri::command]
pub async fn get_languages(state: State<'_, CoreHandle>) -> Result<Vec<String>, String> {
    ask_core(&state, CoreMsg::GetLanguages)
}

/// v2 CloseCommand：遮罩解锁按钮 → `CoreMsg::RestDone` 提前结束休息（§5.3）
#[tauri::command]
pub async fn rest_done(state: State<'_, CoreHandle>) -> Result<(), String> {
    state
        .tx
        .send(CoreMsg::RestDone)
        .map_err(|_| "CoreLoop 已退出".to_string())
}
