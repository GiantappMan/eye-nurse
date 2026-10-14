//! 应用内更新（家族模式移植：livewallpaper `src-tauri/src/app_update.rs` 同源）：
//! 按通道（stable/preview/off）从发布源拉清单，发现新版本后（可选）自动下载
//! 安装包到数据目录 `update/`；安装始终由用户在设置页确认（「安装并重启」）。
//!
//! 与 livewallpaper 的三点差异：
//! - 清单是 giantapp-release 技能的扩展格式（含 sha256/size）——下载完成后
//!   校验 sha256（清单缺失该字段则跳过，向后兼容壁纸基础格式）；
//! - 安装器是 Inno（静默参数 `/VERYSILENT` 而非 NSIS `/S`），且
//!   PrivilegesRequired=lowest 无 UAC——脚本先等本进程退出并兜底 taskkill，
//!   否则静默安装会撞上还在运行的 exe 文件锁；
//! - 更新源默认指向共用桶产品前缀（[`eyenurse_core::config::DEFAULT_UPDATE_BASE`]），
//!   不要求用户先在设置里填地址（livewallpaper 的 appUrl 为空时不检查）。

use crate::commands::validate_public_http_url;
use crate::events::CoreMsg;
use crossbeam_channel::Sender;
use eyenurse_core::config::{ConfigStore, UpdateJson, DEFAULT_UPDATE_BASE};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// 当前程序版本（workspace version，与 tauri.conf.json 同步）
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 设置窗事件名（设置窗静态存在，emit_to 恒有接收方）
const EVENT: &str = "eyenurse://app-update";

/// 启动自动检查的延迟（避开 WebView2 冷启动高峰，用户机器实测 >8s）
const STARTUP_CHECK_DELAY: Duration = Duration::from_secs(30);

/// 下载进度事件的推送节流
const PROGRESS_THROTTLE: Duration = Duration::from_millis(200);

/// 更新通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Off,
    Stable,
    Preview,
}

impl Channel {
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "preview" => Channel::Preview,
            "stable" => Channel::Stable,
            _ => Channel::Off,
        }
    }

    fn manifest_name(self) -> &'static str {
        match self {
            Channel::Preview => "preview.json",
            _ => "stable.json",
        }
    }
}

/// 更新配置 DTO（Update.json 的 Channel/AutoDownload；AppUrl 不进 UI，
/// 手改配置文件即可指向自建源——界面保持克制）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfigDto {
    pub channel: String,
    pub auto_download: bool,
}

/// 远程更新信息（清单内容 + 所属通道；serde 直读发布清单，未知字段忽略——
/// git/builtAt 不进客户端）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AppUpdateInfo {
    pub version: String,
    /// 安装包地址；相对地址以清单地址为 base 解析。
    pub url: String,
    pub notes: String,
    pub date: String,
    pub channel: String,
    /// 安装包 sha256（十六进制；空 = 清单未提供，跳过校验）
    pub sha256: String,
    /// 安装包字节数（仅展示用，不参与校验）
    pub size: u64,
}

/// 状态快照（设置页渲染用；下载进度经 [`EVENT`] 实时推送）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateState {
    pub channel: String,
    pub auto_download: bool,
    /// idle | available | downloading | downloaded | error
    pub phase: String,
    pub percent: f64,
    pub info: Option<AppUpdateInfo>,
    pub error: Option<String>,
    pub downloaded_version: Option<String>,
}

/// 前端事件载荷（tag = state，便于按分支解构；与 mask 页 rest-tick 同风格）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum AppUpdateEvent {
    Checking,
    UpToDate,
    Available {
        version: String,
        notes: String,
        channel: String,
    },
    Progress {
        percent: f64,
        received_bytes: u64,
        total_bytes: u64,
    },
    Downloaded {
        version: String,
    },
    Error {
        message: String,
    },
}

static DOWNLOADING: AtomicBool = AtomicBool::new(false);
static LAST_INFO: Mutex<Option<AppUpdateInfo>> = Mutex::new(None);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);
static DOWNLOADED: Mutex<Option<(String, std::path::PathBuf)>> = Mutex::new(None);

/// 锁守卫（Mutex 中毒时取回内部数据继续——状态静态量不因单次 panic 而废）
fn lock<'a, T>(cell: &'a Mutex<T>) -> std::sync::MutexGuard<'a, T> {
    cell.lock().unwrap_or_else(|e| e.into_inner())
}

fn publish(app: &AppHandle, event: AppUpdateEvent) {
    let _ = app.emit_to("settings", EVENT, event);
}

/// 配置目录（命令线程独立构建，与 `get_autostart` 无状态例外同模式——
/// Update.json/update/ 均不经 CoreLoop）
fn store() -> ConfigStore {
    ConfigStore::v2_default()
}

fn update_dir() -> std::path::PathBuf {
    store().dir().join("update")
}

fn installer_path(version: &str) -> std::path::PathBuf {
    update_dir().join(format!("EyeNurse_{version}_x64-setup.exe"))
}

fn load_config() -> UpdateJson {
    store().load()
}

/// 通道值规范化：off/stable/preview 原样，其余回 stable（与非法快捷键回默认同约定）
pub fn normalize_channel(value: &str) -> String {
    let v = value.trim();
    if v == "off" || v == "preview" || v == "stable" {
        v.to_string()
    } else {
        "stable".to_string()
    }
}

/// 保存通道与自动下载开关（设置页即时保存；AppUrl 保持不动）
pub fn save_config(channel: &str, auto_download: bool) -> Result<(), String> {
    let mut cfg: UpdateJson = store().load();
    cfg.Channel = normalize_channel(channel);
    cfg.AutoDownload = auto_download;
    store().save(&cfg).map_err(|e| e.to_string())
}

/// 当前配置 DTO（设置页回显；通道规范化后返回）
pub fn current_config() -> UpdateConfigDto {
    let cfg = load_config();
    UpdateConfigDto {
        channel: normalize_channel(&cfg.Channel),
        auto_download: cfg.AutoDownload,
    }
}

/// 当前设置 + 安装状态快照。
pub fn current_state() -> AppUpdateState {
    let cfg = load_config();
    let info = lock(&LAST_INFO).clone();
    let error = lock(&LAST_ERROR).clone();
    let downloaded = lock(&DOWNLOADED).as_ref().map(|(v, _)| v.clone());
    let phase = if DOWNLOADING.load(Ordering::SeqCst) {
        "downloading"
    } else if downloaded.is_some() {
        "downloaded"
    } else if error.is_some() {
        "error"
    } else if info.is_some() {
        "available"
    } else {
        "idle"
    };
    AppUpdateState {
        channel: normalize_channel(&cfg.Channel),
        auto_download: cfg.AutoDownload,
        phase: phase.into(),
        percent: 0.0,
        info,
        error,
        downloaded_version: downloaded,
    }
}

/// 检查用清单地址：`<base>/<stable|preview>.json`。base 为空回默认发布源；
/// 整体按 http/https 公网地址校验（自建源指向内网/本机一律拒绝）。
fn manifest_url_for(base: &str, manifest_name: &str) -> Result<String, String> {
    let base = base.trim();
    let base = if base.is_empty() { DEFAULT_UPDATE_BASE } else { base };
    let base = base.trim_end_matches('/');
    let url = format!("{base}/{manifest_name}");
    validate_public_http_url(&url)?;
    Ok(url)
}

/// 相对地址（清单里的 `dl/xxx.exe`）以清单地址为 base 解析成绝对地址；
/// 绝对地址原样返回（仍过公网校验）。
fn resolve_against(manifest_url: &str, target: &str) -> Result<String, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("清单缺少下载地址".into());
    }
    if target.contains("://") {
        validate_public_http_url(target)?;
        return Ok(target.to_string());
    }
    let base = manifest_url.trim_end_matches('/');
    let base = match base.rsplit_once('/') {
        // 清单地址形如 <base>/stable.json：相对地址落在清单所在目录内
        Some((prefix, _file)) => prefix,
        None => base,
    };
    let url = format!("{base}/{target}");
    validate_public_http_url(&url)?;
    Ok(url)
}

/// 检查用 HTTP 客户端（UA + 连接/读取超时）。
fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("EyeNurse/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

/// 按当前配置的通道检查更新。
/// 返回 Some(信息) 表示有比当前程序更新的版本（已缓存并广播事件）。
pub async fn check(app: &AppHandle) -> Result<Option<AppUpdateInfo>, String> {
    let cfg = load_config();
    let channel = Channel::parse(&cfg.Channel);
    *lock(&LAST_ERROR) = None;
    if channel == Channel::Off {
        return Ok(None);
    }
    publish(app, AppUpdateEvent::Checking);

    let manifest_url = manifest_url_for(&cfg.AppUrl, channel.manifest_name())?;
    let text = http_client()?
        .get(manifest_url)
        .send()
        .await
        .map_err(|e| format!("请求更新清单失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("更新清单请求失败: {e}"))?
        .text()
        .await
        .map_err(|e| format!("读取更新清单失败: {e}"))?;
    let mut info: AppUpdateInfo = serde_json::from_str(&text)
        .map_err(|e| format!("解析更新清单失败: {e}"))?;
    if info.version.trim().is_empty() || info.url.trim().is_empty() {
        return Err("更新清单缺少 version / url 字段".into());
    }
    // stable 通道忽略预发布版本（预发布走 preview 通道发布）
    if channel == Channel::Stable && is_prerelease(&info.version) {
        publish(app, AppUpdateEvent::UpToDate);
        return Ok(None);
    }
    info.channel = cfg.Channel.clone();
    if compare_versions(&info.version, APP_VERSION) != std::cmp::Ordering::Greater {
        publish(app, AppUpdateEvent::UpToDate);
        return Ok(None);
    }
    *lock(&LAST_INFO) = Some(info.clone());
    // 已下载过同一版本则直接进入 downloaded 状态（重开设置页不重复下载）
    let path = installer_path(&info.version);
    if path.exists() {
        *lock(&DOWNLOADED) = Some((info.version.clone(), path));
        publish(
            app,
            AppUpdateEvent::Downloaded {
                version: info.version.clone(),
            },
        );
    } else {
        publish(
            app,
            AppUpdateEvent::Available {
                version: info.version.clone(),
                notes: info.notes.clone(),
                channel: info.channel.clone(),
            },
        );
    }
    Ok(Some(info))
}

/// 下载更新（默认用最近一次检查结果）到 `update/` 目录，进度经事件广播；
/// 清单带 sha256 时校验，不匹配删包报错。
pub async fn download(app: &AppHandle, info: Option<AppUpdateInfo>) -> Result<(), String> {
    let resolved = match info {
        Some(i) => i,
        None => lock(&LAST_INFO)
            .clone()
            .ok_or_else(|| "请先检查更新".to_string())?,
    };
    if DOWNLOADING.swap(true, Ordering::SeqCst) {
        return Err("正在下载更新".into());
    }
    let result = download_inner(app, resolved).await;
    DOWNLOADING.store(false, Ordering::SeqCst);
    match result {
        Ok((version, path)) => {
            *lock(&DOWNLOADED) = Some((version.clone(), path));
            *lock(&LAST_ERROR) = None;
            publish(app, AppUpdateEvent::Downloaded { version });
            Ok(())
        }
        Err(e) => {
            *lock(&LAST_ERROR) = Some(e.clone());
            publish(app, AppUpdateEvent::Error { message: e.clone() });
            Err(e)
        }
    }
}

async fn download_inner(
    app: &AppHandle,
    info: AppUpdateInfo,
) -> Result<(String, std::path::PathBuf), String> {
    let cfg = load_config();
    let manifest_url = manifest_url_for(&cfg.AppUrl, Channel::parse(&cfg.Channel).manifest_name())?;
    let target = resolve_against(&manifest_url, &info.url)?;
    let dest = installer_path(&info.version);
    std::fs::create_dir_all(update_dir()).map_err(|e| format!("创建目录失败: {e}"))?;

    let client = http_client()?;
    let mut resp = client
        .get(target)
        .send()
        .await
        .map_err(|e| format!("下载安装包失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("下载安装包失败: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("创建文件失败: {e}"))?;
    let mut hasher = sha2::Sha256::new();
    let mut received: u64 = 0;
    let mut last_notify = std::time::Instant::now() - PROGRESS_THROTTLE;
    publish(
        app,
        AppUpdateEvent::Progress {
            percent: 0.0,
            received_bytes: 0,
            total_bytes: total,
        },
    );
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("下载安装包失败: {e}"))?
    {
        use std::io::Write;
        file.write_all(&chunk).map_err(|e| format!("写入文件失败: {e}"))?;
        hasher.update(&chunk);
        received += chunk.len() as u64;
        let now = std::time::Instant::now();
        if now.duration_since(last_notify) >= PROGRESS_THROTTLE {
            last_notify = now;
            let percent = if total > 0 { received as f64 / total as f64 * 100.0 } else { 0.0 };
            publish(
                app,
                AppUpdateEvent::Progress {
                    percent,
                    received_bytes: received,
                    total_bytes: total,
                },
            );
        }
    }
    use std::io::Write as _;
    file.flush().map_err(|e| format!("写入文件失败: {e}"))?;
    drop(file);
    // 完整性：清单带 sha256 才校验（基础格式清单无此字段则跳过，向后兼容）
    let hash = format!("{:x}", hasher.finalize());
    if !info.sha256.trim().is_empty() && !info.sha256.trim().eq_ignore_ascii_case(&hash) {
        let _ = std::fs::remove_file(&tmp);
        return Err("安装包校验失败（sha256 不匹配），已删除下载".into());
    }
    std::fs::rename(&tmp, &dest).map_err(|e| format!("保存安装包失败: {e}"))?;
    Ok((info.version.clone(), dest))
}

/// 安装已下载的更新：写批处理（等本进程退出 → 兜底 taskkill → 静默安装 →
/// 重启 → 自删）→ 分离进程执行 → 走托盘退出语义（UI.json/统计落盘、撤销锁屏
/// 接管）退出应用。per-user 安装无 UAC，等待是必须的：Inno 静默模式下撞上
/// 文件锁不会弹询问，直接失败。
pub fn install(core_tx: &Sender<CoreMsg>) -> Result<(), String> {
    let (_version, path) = lock(&DOWNLOADED)
        .clone()
        .ok_or_else(|| "没有已下载的更新".to_string())?;
    if !path.exists() {
        *lock(&DOWNLOADED) = None;
        return Err("安装包不存在，请重新下载".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("定位程序失败: {e}"))?;
    let script_path = store().dir().join("apply-update.cmd");
    let script = format!(
        "@echo off\r\nping -n 3 127.0.0.1 >nul\r\ntaskkill /im eyenurse.exe /f /t >nul 2>&1\r\n\
         start \"\" /wait \"{}\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART\r\n\
         start \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        path.display(),
        exe.display()
    );
    std::fs::write(&script_path, script).map_err(|e| format!("写入安装脚本失败: {e}"))?;

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    std::process::Command::new("cmd")
        .arg("/C")
        .arg(&script_path)
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("启动安装程序失败: {e}"))?;
    // 走托盘退出语义：与托盘「退出」同一清理路径（CoreLoop TrayAction::Exit）
    core_tx
        .send(CoreMsg::Tray(crate::tray::TrayAction::Exit))
        .map_err(|_| "CoreLoop 已退出".to_string())
}

// ---------- 启动自动检查 ----------

/// 启动后台检查：延迟 [`STARTUP_CHECK_DELAY`]（避开启动期 WebView2 冷启动
/// 高峰），通道开启时拉清单；发现新版本且允许自动下载则直接下载，安装仍需
/// 用户在设置页确认（绝不静默安装）。错误只记 stderr（release 无控制台无害）。
pub fn start_background(app: AppHandle) {
    std::thread::Builder::new()
        .name("app-update-boot".into())
        .spawn(move || {
            std::thread::sleep(STARTUP_CHECK_DELAY);
            tauri::async_runtime::spawn(async move {
                let cfg = load_config();
                if Channel::parse(&cfg.Channel) == Channel::Off {
                    return;
                }
                match check(&app).await {
                    Ok(Some(info)) => {
                        if cfg.AutoDownload {
                            if let Err(e) = download(&app, Some(info)).await {
                                boot_log(&format!("app update auto download failed: {e}"));
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => boot_log(&format!("app update check failed: {e}")),
                }
            });
        })
        .ok();
}

fn boot_log(message: &str) {
    // 英文：Windows 控制台默认 GBK，中文会乱码（实测教训 3）
    #[cfg(debug_assertions)]
    eprintln!("[app-update] {message}");
    #[cfg(not(debug_assertions))]
    let _ = message;
}

// ---------- 版本比较（livewallpaper updater.rs 同源移植）----------

/// 宽松 semver 比较（`3.0.1` / `3.1.0-alpha.3` / `1.2`）。
/// 无预发布段 > 有预发布段；预发布段逐标识符比较（数字段按数值，
/// `alpha.2 < alpha.10`）。
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let (core_a, pre_a) = split_version(a);
    let (core_b, pre_b) = split_version(b);
    let ord = compare_core(&core_a, &core_b);
    if ord != std::cmp::Ordering::Equal {
        return ord;
    }
    match (pre_a, pre_b) {
        (None, None) => std::cmp::Ordering::Equal,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(pa), Some(pb)) => compare_prerelease(pa, pb),
    }
}

fn split_version(v: &str) -> (Vec<u64>, Option<&str>) {
    let v = v.trim().trim_start_matches('v');
    let v = v.split('+').next().unwrap_or(v);
    match v.split_once('-') {
        Some((core, pre)) => (parse_core(core), Some(pre)),
        None => (parse_core(v), None),
    }
}

fn parse_core(core: &str) -> Vec<u64> {
    core.split('.').map(|s| s.parse().unwrap_or(0)).collect()
}

fn compare_core(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    let len = a.len().max(b.len());
    for i in 0..len {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x.cmp(&y);
        }
    }
    std::cmp::Ordering::Equal
}

fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ia = a.split('.');
    let mut ib = b.split('.');
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                    (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

/// 版本是否带预发布段（`-alpha.1` 等）。
pub fn is_prerelease(version: &str) -> bool {
    let v = version.trim().trim_start_matches('v');
    v.split('+').next().unwrap_or(v).contains('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("3.0.1", "3.0.0"), Greater);
        assert_eq!(compare_versions("3.0.0", "3.0.0"), Equal);
        assert_eq!(compare_versions("3.0.1-alpha.1", "3.0.1"), Less);
        assert_eq!(compare_versions("3.0.1-alpha.3", "3.0.1-alpha.2"), Greater);
        assert_eq!(compare_versions("3.0.1-alpha.10", "3.0.1-alpha.9"), Greater);
        assert_eq!(compare_versions("3.1.0", "3.0.9"), Greater);
        assert_eq!(compare_versions("3.0", "3.0.0"), Equal);
        assert_eq!(compare_versions("v3.0.2", "3.0.1"), Greater);
        assert!(is_prerelease("3.0.1-alpha.3"));
        assert!(!is_prerelease("3.0.1"));
    }

    #[test]
    fn stable_channel_ignores_prerelease_but_preview_accepts() {
        // 清单带 - 后缀时 stable 通道在 check() 里跳过（分支语义），此处锚定判定函数
        assert!(is_prerelease("3.1.0-beta.1"));
        assert_eq!(Channel::parse("preview").manifest_name(), "preview.json");
        assert_eq!(Channel::parse("stable").manifest_name(), "stable.json");
    }

    #[test]
    fn manifest_url_keeps_base_path_and_defaults() {
        // 默认源（多产品共用桶 + eyenurse3 前缀）
        assert_eq!(
            manifest_url_for("", "stable.json").unwrap(),
            "https://releases.giantapp.cc/eyenurse3/stable.json"
        );
        // 自定义 base 允许带路径段，尾斜杠有无都必须保住路径段
        for base in ["https://dl.example.com/updates", "https://dl.example.com/updates/"] {
            assert_eq!(
                manifest_url_for(base, "preview.json").unwrap(),
                "https://dl.example.com/updates/preview.json"
            );
        }
        // 本机/内网源一律拒绝（与 open_url 同一黑名单）
        assert!(manifest_url_for("http://localhost:8787/updates", "stable.json").is_err());
        assert!(manifest_url_for("http://192.168.1.10/updates", "stable.json").is_err());
        assert!(manifest_url_for("ftp://dl.example.com", "stable.json").is_err());
    }

    #[test]
    fn resolves_relative_and_rejects_private_absolute() {
        let manifest = "https://releases.giantapp.cc/eyenurse3/stable.json";
        assert_eq!(
            resolve_against(manifest, "https://other.example.com/x.exe").unwrap(),
            "https://other.example.com/x.exe"
        );
        assert_eq!(
            resolve_against(manifest, "dl/EyeNurse_3.0.1_x64-setup.exe").unwrap(),
            "https://releases.giantapp.cc/eyenurse3/dl/EyeNurse_3.0.1_x64-setup.exe"
        );
        assert!(resolve_against(manifest, "").is_err());
        assert!(resolve_against(manifest, "http://127.0.0.1/x.exe").is_err());
    }

    #[test]
    fn parses_extended_manifest_and_tolerates_basic() {
        // 扩展格式（giantapp-release 技能清单）
        let extended: AppUpdateInfo = serde_json::from_str(
            r#"{"version":"3.0.1","url":"https://releases.giantapp.cc/eyenurse3/dl/EyeNurse_3.0.1_x64-setup.exe",
                "notes":"修复若干问题","date":"2026.11.1","sha256":"abc123","size":4193144,
                "git":"9fe6387","builtAt":"2026-11-01T00:00:00Z"}"#,
        )
        .unwrap();
        assert_eq!(extended.sha256, "abc123");
        assert_eq!(extended.size, 4193144);
        assert_eq!(extended.notes, "修复若干问题");
        // 基础格式（壁纸清单）缺 sha256/size 也能读
        let basic: AppUpdateInfo = serde_json::from_str(
            r#"{"version":"3.0.1","url":"https://x/dl.exe","notes":"","date":""}"#,
        )
        .unwrap();
        assert!(basic.sha256.is_empty());
        assert_eq!(basic.size, 0);
    }

    #[test]
    fn channel_normalization() {
        assert_eq!(normalize_channel("preview"), "preview");
        assert_eq!(normalize_channel("off"), "off");
        assert_eq!(normalize_channel(" stable "), "stable");
        assert_eq!(normalize_channel("whatever"), "stable");
        assert_eq!(normalize_channel(""), "stable");
    }
}
