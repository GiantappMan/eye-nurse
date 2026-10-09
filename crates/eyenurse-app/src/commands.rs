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
use crate::skin_proto::SkinSurfaceDto;
use crate::tray::TrayAction;
use crossbeam_channel::Sender;
use eyenurse_core::autostart::DesktopStartup;
use eyenurse_core::config::{LockScreenJson, SettingJson};
use eyenurse_core::skin::{SkinColors, SkinDef, SkinsDto};
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

/// 锁屏设置 DTO（字段名与 LockScreenJson 一致，零映射；v3.2 仿锁屏）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(non_snake_case)]
pub struct LockSettingsDto {
    /// 仿锁屏总开关
    pub Enabled: bool,
    /// Win+L 拦截激活（配套注册表 DisableLockWorkstation）
    pub InterceptWinL: bool,
    /// 截图穿透（WDA_EXCLUDEFROMCAPTURE）
    pub CaptureThrough: bool,
    /// 锁屏激活即进入休息
    pub AutoRest: bool,
    /// 解锁密码（空 = 无需密码）
    pub Password: String,
}

impl LockSettingsDto {
    pub fn of(s: &LockScreenJson) -> Self {
        Self {
            Enabled: s.Enabled,
            InterceptWinL: s.InterceptWinL,
            CaptureThrough: s.CaptureThrough,
            AutoRest: s.AutoRest,
            Password: s.Password.clone(),
        }
    }

    pub fn into_core(self) -> LockScreenJson {
        LockScreenJson {
            Enabled: self.Enabled,
            InterceptWinL: self.InterceptWinL,
            CaptureThrough: self.CaptureThrough,
            AutoRest: self.AutoRest,
            Password: self.Password,
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

/// 立刻休息（设置窗 Hero 按钮用）：与托盘「立刻休息」同一动作入口
/// （`CoreMsg::Tray(RestNow)`），火后不理——遮罩窗口由 CoreLoop 统一创建
#[tauri::command]
pub async fn rest_now(state: State<'_, CoreHandle>) -> Result<(), String> {
    state
        .tx
        .send(CoreMsg::Tray(crate::tray::TrayAction::RestNow))
        .map_err(|e| e.to_string())
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

// —— v3.2 仿锁屏 / 护眼统计 ——

/// 当前锁屏设置（CoreLoop 内存态）
#[tauri::command]
pub async fn get_lock_settings(state: State<'_, CoreHandle>) -> Result<LockSettingsDto, String> {
    let lock: LockScreenJson = ask_core(&state, CoreMsg::GetLockSettings)?;
    Ok(LockSettingsDto::of(&lock))
}

/// 锁屏设置保存：写 LockScreen.json + 钩子开关 + 注册表策略 → 回包刷新
#[tauri::command]
pub async fn set_lock_settings(
    state: State<'_, CoreHandle>,
    dto: LockSettingsDto,
) -> Result<LockSettingsDto, String> {
    let applied: LockScreenJson =
        ask_core(&state, |tx| CoreMsg::SaveLockSettings(dto.into_core(), tx))?;
    Ok(LockSettingsDto::of(&applied))
}

/// 锁屏遮罩密码解锁：匹配 true 并撤锁；不匹配 false（页面提示密码错误）
#[tauri::command]
pub async fn lock_try_unlock(
    state: State<'_, CoreHandle>,
    password: String,
) -> Result<bool, String> {
    ask_core(&state, |tx| CoreMsg::LockTryUnlock(password, tx))
}

/// 近 30 天护眼统计（含今天，旧→新；缺失天补零）
#[tauri::command]
pub async fn get_stats(
    state: State<'_, CoreHandle>,
) -> Result<Vec<eyenurse_core::stats::StatsDay>, String> {
    ask_core(&state, |tx| CoreMsg::GetStats(30, tx))
}

// —— v3.1 换肤 ——

/// 可用皮肤清单（内置 + 磁盘 + 自定义槽）+ 当前选中 id
#[tauri::command]
pub async fn get_skins(state: State<'_, CoreHandle>) -> Result<SkinsDto, String> {
    ask_core(&state, CoreMsg::GetSkins)
}

/// 当前生效配色（逐键回退后的完整值；页面据此渲染预览/预填编辑器）
#[tauri::command]
pub async fn get_skin(state: State<'_, CoreHandle>) -> Result<SkinColors, String> {
    ask_core(&state, CoreMsg::GetSkin)
}

/// 选择皮肤（内置 id / 磁盘文件名 / "custom"）：切换即生效并广播
#[tauri::command]
pub async fn set_skin(state: State<'_, CoreHandle>, id: String) -> Result<(), String> {
    state.tx.send(CoreMsg::SetSkin(id)).map_err(|e| e.to_string())
}

/// 保存用户自定义配色并启用（Current="custom"）→ 回应生效后的完整配色
#[tauri::command]
pub async fn set_custom_skin(
    state: State<'_, CoreHandle>,
    def: SkinDef,
) -> Result<SkinColors, String> {
    ask_core(&state, |tx| CoreMsg::SetCustomSkin(Box::new(def), tx))
}

// —— v3.2 ESP-1 开放皮肤协议 ——

/// 表面运行时（配色 + bar/lock 皮肤页 URL + lock chrome 摆放 + bar 运行模式）。
/// mask/barweb 壳页的启动依赖；运行中变化经 `eyenurse://skin-surface-changed`
/// 广播（载荷同型）
#[tauri::command]
pub async fn get_skin_surface(state: State<'_, CoreHandle>) -> Result<SkinSurfaceDto, String> {
    ask_core(&state, CoreMsg::GetSkinSurface)
}

/// 打开设置窗（托盘「设置」同源动作 CoreMsg::Tray(Settings)；
/// bar/lock 皮肤页经 postMessage 桥调用——皮肤 iframe 自身拿不到 IPC）
#[tauri::command]
pub async fn open_settings(state: State<'_, CoreHandle>) -> Result<(), String> {
    state
        .tx
        .send(CoreMsg::Tray(TrayAction::Settings))
        .map_err(|e| e.to_string())
}

/// bar 皮肤壳页的系统级拖动（皮肤 iframe 在 sandbox 内拿不到窗口 API，
/// 经桥转到壳页后 invoke 本命令；语义 = GDI 条的 HTCAPTION 系统拖动）
#[tauri::command]
pub async fn bar_drag(window: tauri::WebviewWindow<tauri::Wry>) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

/// 置顶开关（bar 复选同源 CoreMsg::ToggleTopmost：写 UI.json + GDI WM_APP /
/// barweb set_always_on_top 双路生效）
#[tauri::command]
pub async fn set_topmost(state: State<'_, CoreHandle>, on: bool) -> Result<(), String> {
    state
        .tx
        .send(CoreMsg::ToggleTopmost(on))
        .map_err(|e| e.to_string())
}

/// HTML 悬浮条右键菜单（皮肤 eyenurse.openMenu() / barweb 壳页 contextmenu 桥）：
/// CoreLoop 在临时线程弹原生 TrackPopupMenuEx（托盘同源动作组）
#[tauri::command]
pub async fn bar_open_menu(state: State<'_, CoreHandle>) -> Result<(), String> {
    state
        .tx
        .send(CoreMsg::ShowBarMenu)
        .map_err(|e| e.to_string())
}

/// 打开磁盘皮肤目录（设置页「打开皮肤文件夹」入口：目录不存在则先创建）。
/// 安装方式 = 把皮肤包文件夹/裸 json 丢进该目录，重开设置页即见。
#[tauri::command]
pub async fn open_skins_folder() -> Result<(), String> {
    let dir = crate::skin_proto::skins_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    {
        use windows::core::{w, HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let path = HSTRING::from(dir.as_os_str());
        // SAFETY：仅打开一个本地目录；句柄返回值被忽略（与 open_about 同语义）
        unsafe {
            ShellExecuteW(None, w!("open"), PCWSTR(path.as_ptr()), None, None, SW_SHOWNORMAL);
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = dir;
    Ok(())
}
