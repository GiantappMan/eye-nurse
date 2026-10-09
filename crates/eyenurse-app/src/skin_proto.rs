//! ESP-1 开放皮肤协议的 app 侧落点（docs/v3-skin-protocol.md §2–§3）：
//!
//! 1. `eyenurse-skin://` 自定义协议 handler：只服务 exe 旁 `Assets/Skins/<id>/`
//!    包目录内的文件。Windows 上自定义协议的页面源是
//!    `http://eyenurse-skin.localhost/<path>`（tauri 2 文档明示，全平台映射见
//!    `register_uri_scheme_protocol` 的 Warning 段）；路径纪律：皮肤 id 与资源
//!    路径只接受白名单字符集（不做百分号解码——编码名不匹配即 404），规范化后
//!    断言仍落在对应包目录内，越界一律 404；
//! 2. 响应头安全基线：皮肤页是**不可信代码**，统一打严格 CSP（禁远程源、禁
//!    fetch/XHR 外联、禁嵌框/表单），配合宿主壳页的 `sandbox="allow-scripts"`
//!    iframe（opaque origin，拿不到 Tauri IPC 与宿主 DOM）构成双防线；
//! 3. [`SkinSurfaceDto`]：页面消费的表面运行时（当前配色 + 两个 target 的
//!    皮肤页 URL + lock 宿主 chrome 摆放 + bar 运行模式），`get_skin_surface`
//!    命令与 `eyenurse://skin-surface-changed` 广播共用。

use eyenurse_core::skin::{self, ChromePos, LockChromeDef, ResolvedSurface, SkinColors, SkinConfig};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// 自定义协议 scheme（页面源 = `http://eyenurse-skin.localhost/...`）
pub const SKIN_SCHEME_HOST: &str = "http://eyenurse-skin.localhost";

/// lock 宿主 chrome 的缺省摆放：密码框无声明时居中偏下；**解锁按钮无声明时
/// 不改位**（壳页 CSS 默认右下 12px 定角——与 v3.2 之前的 classic 外观像素级
/// 一致，皮肤 manifest 显式声明百分比才移动）
const DEFAULT_PASSWORD_POS: ChromePos = ChromePos { x: 50.0, y: 62.0 };

/// 页面消费的表面运行时快照（get_skin_surface 回包 / skin-surface-changed 载荷）。
/// bar URL 为 None = GDI 原生条；lock URL 由 core 保证恒有（headless：无声明
/// 回退内置 classic 包锁屏）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkinSurfaceDto {
    /// 当前皮肤 id（"classic" / 内置 / 磁盘 id / "custom"）
    pub id: String,
    /// 逐键回退后的完整配色（classic 包锁屏等皮肤消费；壳页 chrome 也用）
    pub colors: SkinColors,
    /// bar 皮肤页 URL（`http://eyenurse-skin.localhost/<id>/<entry>`；None = GDI）
    pub bar_url: Option<String>,
    /// lock 皮肤页 URL（core 侧永不 None——headless 锁屏回退 classic 包）
    pub lock_url: Option<String>,
    /// bar 窗口逻辑尺寸（DIP；manifest 未声明 → 80×20，对齐 GDI 条）
    pub bar_w: f64,
    pub bar_h: f64,
    /// lock 宿主独占控件摆放（密码框百分比含缺省兜底；解锁按钮 None = 壳页
    /// CSS 默认右下角像素定位）
    pub lock_chrome: LockChromeDto,
    /// bar 当前实际运行模式（true = WebView 壳 + 皮肤 iframe）。启动时定死；
    /// 设置页据此对「表面形态变化」的皮肤显示重启提示（换色无需重启）
    pub bar_is_web: bool,
}

/// 宿主独占控件摆放（序列化形态；unlock 为 None = 保持壳页默认右下角）
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LockChromeDto {
    pub password_x: f64,
    pub password_y: f64,
    pub unlock_x: Option<f64>,
    pub unlock_y: Option<f64>,
}

impl LockChromeDto {
    fn of(def: Option<LockChromeDef>) -> Self {
        let def = def.unwrap_or_default();
        let password = def.password.unwrap_or(DEFAULT_PASSWORD_POS);
        let unlock = def.unlock.map(|p| (p.x.clamp(0.0, 100.0), p.y.clamp(0.0, 100.0)));
        Self {
            password_x: password.x.clamp(0.0, 100.0),
            password_y: password.y.clamp(0.0, 100.0),
            unlock_x: unlock.map(|(x, _)| x),
            unlock_y: unlock.map(|(_, y)| y),
        }
    }
}

/// 默认 bar 逻辑尺寸（= GDI 条 LOGICAL_WIDTH/HEIGHT，ui/bar.rs 同值）
pub const DEFAULT_BAR_W: f64 = 80.0;
pub const DEFAULT_BAR_H: f64 = 20.0;

/// 磁盘皮肤目录（与 core.rs `CoreLoop::skins_dir` 同一定位：exe 旁 Assets/Skins）
pub fn skins_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(std::env::temp_dir)
        .join("Assets")
        .join("Skins")
}

/// 启动期/切换期的全量表面解析（扫描磁盘 → 逐 target 回退）。
/// 返回 (配色, bar 表面, lock 表面)。
pub fn resolve(config: &SkinConfig) -> (SkinColors, Option<ResolvedSurface>, Option<ResolvedSurface>) {
    let disk = skin::scan_disk(&skins_dir());
    skin::resolve_surfaces(config, &disk)
}

/// `ResolvedSurface` → 页面 URL（scheme host + id + entry；entry 已在 manifest
/// 加载期做过穿越校验，此处只拼装）
fn surface_url(id: &str, surf: &ResolvedSurface) -> String {
    format!("{SKIN_SCHEME_HOST}/{id}/{}", surf.entry.replace('\\', "/"))
}

/// 全量 DTO（CoreLoop 侧调用；bar_is_web 由 CoreLoop 传入——运行模式它才知晓）
pub fn surface_dto(
    config: &SkinConfig,
    bar_is_web: bool,
) -> SkinSurfaceDto {
    let (colors, bar, lock) = resolve(config);
    let id = config.Current.clone();
    let (bar_url, bar_size) = match &bar {
        Some(s) => (Some(surface_url(&id, s)), s.size),
        None => (None, None),
    };
    let lock_url = lock.as_ref().map(|s| surface_url(&id, s));
    let chrome = lock.as_ref().and_then(|s| s.chrome);
    SkinSurfaceDto {
        id,
        colors,
        bar_url,
        lock_url,
        bar_w: bar_size.map_or(DEFAULT_BAR_W, |s| s.w),
        bar_h: bar_size.map_or(DEFAULT_BAR_H, |s| s.h),
        lock_chrome: LockChromeDto::of(chrome),
        bar_is_web,
    }
}

// ============================================================================
// 协议 handler：eyenurse-skin:// → 包目录文件
// ============================================================================

/// 资源路径字符白名单（皮肤包内容自控；百分号编码一律不匹配 = 404，
/// 从机制上排除解码类绕过）
fn is_safe_segment(seg: &str) -> bool {
    !seg.is_empty()
        && seg != "."
        && seg != ".."
        && seg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// 扩展名 → Content-Type（皮肤页会消费的全部资源类型；未识别类型按字节流兜底）
fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// 皮肤页统一 CSP：只允许同源（协议 host）子资源 + 内联脚本/样式（单文件皮肤
/// 是重要 authored 体验；唯一内容源是本地包本身，connect-src 'none' 已断外联，
/// 内联不构成额外风险面）。远程源、fetch/XHR 外联、嵌框、表单、对象插件全禁。
/// 与宿主壳页的 sandbox iframe 构成双防线（§3 安全边界）。
const SKIN_CSP: &str = "default-src 'none'; \
     script-src http://eyenurse-skin.localhost 'unsafe-inline'; \
     style-src http://eyenurse-skin.localhost 'unsafe-inline'; \
     img-src http://eyenurse-skin.localhost data: blob:; \
     font-src http://eyenurse-skin.localhost data:; \
     media-src http://eyenurse-skin.localhost data: blob:; \
     connect-src 'none'; form-action 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'";

// ============================================================================
// 内嵌皮肤资产（ESP-1 headless）：classic 锁屏 + devtest 全表面 + 桥脚本，
// 编译进二进制，经与磁盘皮肤完全相同的 eyenurse-skin:// 协议服务——宿主没有
// 皮肤系统之外的界面。与 core 的 builtin_package(id) 声明一一对应。
// ============================================================================

mod embedded {
    pub const BRIDGE_JS: &str = include_str!("skins/eyenurse-api.js");
    pub const CLASSIC_MANIFEST: &str = include_str!("skins/classic/skin.json");
    pub const CLASSIC_LOCK: &str = include_str!("skins/classic/lock/index.html");
    pub const DEVTEST_MANIFEST: &str = include_str!("skins/devtest/skin.json");
    pub const DEVTEST_BAR: &str = include_str!("skins/devtest/bar/index.html");
    pub const DEVTEST_LOCK: &str = include_str!("skins/devtest/lock/index.html");
}

/// 内嵌资产查找（rel 路径以 `/` 分隔；精确匹配，无遍历空间）
fn embedded_lookup(id: &str, rel: &str) -> Option<&'static str> {
    match (id, rel) {
        ("classic", "skin.json") => Some(embedded::CLASSIC_MANIFEST),
        ("classic", "lock/index.html") => Some(embedded::CLASSIC_LOCK),
        ("devtest", "skin.json") => Some(embedded::DEVTEST_MANIFEST),
        ("devtest", "bar/index.html") => Some(embedded::DEVTEST_BAR),
        ("devtest", "lock/index.html") => Some(embedded::DEVTEST_LOCK),
        // 桥脚本对内嵌皮肤恒可用
        ("classic" | "devtest", "eyenurse-api.js") => Some(embedded::BRIDGE_JS),
        _ => None,
    }
}

fn respond_ok(mime: &'static str, bytes: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(tauri::http::StatusCode::OK)
        .header(tauri::http::header::CONTENT_TYPE, mime)
        .header(tauri::http::header::CONTENT_SECURITY_POLICY, SKIN_CSP)
        .header(tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(tauri::http::header::CACHE_CONTROL, "no-cache")
        .body(bytes)
        .unwrap()
}

/// `eyenurse-skin://` 请求处理：`/<id>/<entry...>`。
/// - 内置 id（classic/devtest）→ 内嵌资产（磁盘同名目录被扫描层跳过，不可遮蔽）；
/// - 其余 → `Assets/Skins/<id>/<entry...>`；磁盘包未自带 `eyenurse-api.js` 时
///   回退内嵌桥（第三方皮肤零依赖模板）。
///
/// 任何不合法（id 字符集、路径穿越、越出包目录、文件缺失）→ 404，绝不回退到
/// 更宽的目录（皮肤资源只有包目录一个根）。
pub fn handle_skin_request(request: tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let not_found = || {
        tauri::http::Response::builder()
            .status(tauri::http::StatusCode::NOT_FOUND)
            .body(b"skin asset not found".to_vec())
            .unwrap()
    };
    let raw_path = request.uri().path();
    let mut segments = raw_path.split('/').filter(|s| !s.is_empty());
    // 第一段 = 皮肤 id（目录名，[a-z0-9-]{1,64}——与 core 校验同规范）
    let Some(id) = segments.next() else {
        return not_found();
    };
    if !eyenurse_core::skin::is_valid_skin_id_pub(id) {
        return not_found();
    }
    let mut rel = PathBuf::new();
    for seg in segments {
        if !is_safe_segment(seg) {
            return not_found();
        }
        rel.push(seg);
    }
    if rel.as_os_str().is_empty() {
        return not_found();
    }
    let rel_unix = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    // 内置 id → 内嵌资产（精确匹配，无 FS 访问）
    if eyenurse_core::skin::is_builtin_id(id) {
        return match embedded_lookup(id, &rel_unix) {
            Some(content) => respond_ok(mime_of(Path::new(&rel_unix)), content.as_bytes().to_vec()),
            None => not_found(),
        };
    }

    // 磁盘包：规范化双重校验——最终路径必须仍在「 skins_dir/<id> 」内（纵深
    // 防御，manifest 加载期已查过 entry，这里覆盖其余全部资源请求）
    let base = skins_dir().join(id);
    let Ok(base_canon) = base.canonicalize() else {
        return not_found();
    };
    let full = base.join(&rel);
    let Ok(full_canon) = full.canonicalize() else {
        // 磁盘包未自带桥脚本 → 内嵌回退（第三方皮肤零依赖）
        return if rel_unix == "eyenurse-api.js" {
            respond_ok("text/javascript; charset=utf-8", embedded::BRIDGE_JS.as_bytes().to_vec())
        } else {
            not_found()
        };
    };
    if !full_canon.starts_with(&base_canon) || !full_canon.is_file() {
        return not_found();
    }
    match std::fs::read(&full_canon) {
        Ok(bytes) => respond_ok(mime_of(&full_canon), bytes),
        Err(_) => not_found(),
    }
}

// ============================================================================
// bar WebView 壳窗（皮肤声明 bar HTML 时的承载窗口；GDI 路径在 ui/bar.rs 不动）
// ============================================================================

/// 建 bar 的 WebView 壳窗（main setup 钩子调用，主线程——tauri 要求）。
/// 行为对齐 GDI 条的初始摆放：UI.json 逻辑 DIP 位置 + TopMost；透明、无装饰、
/// 不进任务栏、不抢焦点。标签固定 "barweb"（CoreLoop 事件路由与冒烟互不影响）。
pub fn build_barweb_window(
    app: &tauri::AppHandle<tauri::Wry>,
    dto: &SkinSurfaceDto,
    left: f64,
    top: f64,
    topmost: bool,
) -> tauri::Result<tauri::WebviewWindow<tauri::Wry>> {
    let url = dto.bar_url.clone().expect("bar_is_web 时必有皮肤页 URL");
    let win = tauri::WebviewWindowBuilder::new(
        app,
        "barweb",
        tauri::WebviewUrl::External(url.parse().expect("皮肤 bar URL 必合法")),
    )
    .title("CountdownWindow") // 沿用 GDI 条标题（工具窗口识别惯例）
    .decorations(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .transparent(true)
    .always_on_top(topmost)
    .skip_taskbar(true)
    .focused(false)
    .shadow(false)
    .position(left, top)
    .inner_size(dto.bar_w, dto.bar_h)
    .build()?;
    // Alt-Tab 隐藏补丁：与 mask 同一 wry 异步回写问题，同款自愈线程（core.rs §5.4）
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = win.hwnd() {
        crate::win::ensure_toolwindow(hwnd.0 as isize);
        let app = app.clone();
        std::thread::spawn(move || {
            use tauri::Manager;
            for _ in 0..12 {
                match app.get_webview_window("barweb") {
                    Some(w) => {
                        if let Ok(h) = w.hwnd() {
                            crate::win::ensure_toolwindow(h.0 as isize);
                        }
                    }
                    None => return,
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
    }
    Ok(win)
}

// ============================================================================
// 测试：mime 表 / 路径字符白名单 / DTO 兜底（协议 handler 本体依赖 exe 布局，
// 由冒烟 + 真机 dev 覆盖；这里锁纯函数行为）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_segment_rules() {
        assert!(is_safe_segment("index.html"));
        assert!(is_safe_segment("assets"));
        assert!(is_safe_segment("img_1-a.png"));
        assert!(!is_safe_segment(".."));
        assert!(!is_safe_segment("."));
        assert!(!is_safe_segment(""));
        assert!(!is_safe_segment("a/b")); // 含分隔符（split 后不该出现，防御）
        assert!(!is_safe_segment("a%2e%2e")); // 百分号编码不匹配
        assert!(!is_safe_segment("a b")); // 空格不匹配
        assert!(!is_safe_segment("中文.png")); // 非 ASCII 不匹配（包内容自控）
    }

    #[test]
    fn mime_table() {
        assert!(mime_of(Path::new("x/bar.html")).starts_with("text/html"));
        assert!(mime_of(Path::new("x/main.js")).contains("javascript"));
        assert_eq!(mime_of(Path::new("x/logo.PNG")), "image/png");
        assert_eq!(mime_of(Path::new("x/weird.bin")), "application/octet-stream");
    }

    #[test]
    fn chrome_defaults_and_clamp() {
        // 无声明 → 密码框缺省居中偏下；解锁按钮 None = 壳页 CSS 默认右下角
        let d = LockChromeDto::of(None);
        assert_eq!(d.password_x, 50.0);
        assert_eq!(d.password_y, 62.0);
        assert_eq!(d.unlock_x, None);
        assert_eq!(d.unlock_y, None);
        // 声明越界 → clamp
        let d = LockChromeDto::of(Some(LockChromeDef {
            password: Some(ChromePos { x: 120.0, y: -5.0 }),
            unlock: Some(ChromePos { x: 50.0, y: 88.0 }),
        }));
        assert_eq!((d.password_x, d.password_y), (100.0, 0.0));
        assert_eq!(d.unlock_x, Some(50.0));
        assert_eq!(d.unlock_y, Some(88.0));
    }

    /// 内嵌皮肤资产可经协议服务（classic 锁屏 + devtest 全表面 + 桥）——
    /// headless 的「内置皮肤走同一协议管线」契约
    #[test]
    fn embedded_skins_served_via_protocol() {
        let req = |path: &str| {
            tauri::http::Request::builder()
                .uri(path)
                .body(Vec::new())
                .unwrap()
        };
        for path in [
            "http://eyenurse-skin.localhost/classic/lock/index.html",
            "http://eyenurse-skin.localhost/classic/skin.json",
            "http://eyenurse-skin.localhost/classic/eyenurse-api.js",
            "http://eyenurse-skin.localhost/devtest/bar/index.html",
            "http://eyenurse-skin.localhost/devtest/lock/index.html",
            "http://eyenurse-skin.localhost/devtest/skin.json",
        ] {
            let r = handle_skin_request(req(path));
            assert_eq!(r.status(), 200, "{path} 应 200");
            assert!(r.headers().get("Content-Security-Policy").is_some());
            assert!(!r.body().is_empty());
        }
        // 内嵌皮肤未发布的内容 → 404（精确匹配，无遍历）
        assert_eq!(
            handle_skin_request(req("http://eyenurse-skin.localhost/classic/bar/index.html"))
                .status(),
            404
        );
        assert_eq!(
            handle_skin_request(req("http://eyenurse-skin.localhost/classic/..%2fetc")).status(),
            404
        );
    }

    /// 协议 handler 对真实文件系统的行为（测试进程 exe 旁造包——与生产同定位
    /// 规则 exe 旁 Assets/Skins）：合法资源 200+mime+CSP；id 大写/穿越/缺失 404。
    #[test]
    fn skin_protocol_serves_package_and_blocks_traversal() {
        let skins = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("Assets")
            .join("Skins");
        let dir = skins.join("proto-test");
        std::fs::create_dir_all(dir.join("bar")).unwrap();
        std::fs::write(
            dir.join("skin.json"),
            r##"{"format":"eyenurse-skin","formatVersion":1,"id":"proto-test",
                 "name":{"zh":"协议","en":"Proto"},
                 "targets":{"bar":{"entry":"bar/index.html"}}}"##,
        )
        .unwrap();
        std::fs::write(dir.join("bar").join("index.html"), "<html>ok</html>").unwrap();

        let req = |path: &str| {
            tauri::http::Request::builder()
                .uri(path)
                .body(Vec::new())
                .unwrap()
        };
        // 合法资源 → 200 + 正确 mime + CSP 响应头
        let ok = handle_skin_request(req("http://eyenurse-skin.localhost/proto-test/bar/index.html"));
        assert_eq!(ok.status(), 200);
        assert_eq!(
            ok.headers().get("Content-Type").unwrap(),
            "text/html; charset=utf-8"
        );
        assert!(ok.headers().get("Content-Security-Policy").is_some());
        // 包根共享桥文件可服务（示例皮肤 /eyenurse-api.js 的加载形态）
        std::fs::write(dir.join("eyenurse-api.js"), "// bridge").unwrap();
        assert_eq!(
            handle_skin_request(req("http://eyenurse-skin.localhost/proto-test/eyenurse-api.js"))
                .status(),
            200
        );

        // id 大写（越出命名空间）/ 路径穿越（明文与编码）/ 缺失文件 → 404
        assert_eq!(
            handle_skin_request(req("http://eyenurse-skin.localhost/Proto-Test/bar/index.html"))
                .status(),
            404
        );
        assert_eq!(
            handle_skin_request(req("http://eyenurse-skin.localhost/proto-test/../other/x.html"))
                .status(),
            404
        );
        assert_eq!(
            handle_skin_request(req(
                "http://eyenurse-skin.localhost/proto-test/%2e%2e/secret.txt"
            ))
            .status(),
            404
        );
        assert_eq!(
            handle_skin_request(req("http://eyenurse-skin.localhost/proto-test/missing.html"))
                .status(),
            404
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
