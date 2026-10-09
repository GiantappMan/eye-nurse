//! 皮肤系统（v3.1 换肤 → v3.2 开放皮肤协议 ESP-1）：悬浮条 + 锁屏窗的
//! **运行时**配色与**表面（target）替换**。
//!
//! 真源与回退：`classic` 与编译期 `crates/eyenurse-app/src/ui/theme.json` 同值
//! （theme.json 仍是「改经典色」的唯一入口，build.rs 校验不变；本文件字面量由
//! app 侧锚点测试保护，见 eyenurse-app `ui/theme.rs` tests）。其他皮肤以 classic
//! 为底做**逐键回退**——缺键或非法色值 → classic 对应键。
//!
//! ESP-1 两级格式（docs/v3-skin-protocol.md）：
//! - **Tier 0 颜色键**：`Assets/Skins/<id>.json` 裸文件（v3.1 格式，原样兼容）；
//! - **Tier 1 目录包**：`Assets/Skins/<id>/` + `skin.json` manifest，可为
//!   `bar` / `lock` 两个 target 各自提供 HTML 入口（`targets.bar.entry` 等），
//!   亦可只带 `colors`（等价 Tier 0）。**每个 target 独立声明、独立回退**：
//!   皮肤缺某 target 的入口 → 该 target 用内置界面（[`resolve_surfaces`]）。
//!
//! 来源优先级（[`resolve_effective`] / [`resolve_surfaces`]）：
//! 1. 用户自定义：Skin.json 的 `Custom`（`Current == "custom"` 时生效）；
//! 2. 磁盘皮肤（exe 旁 `Assets/Skins`，文件名/目录名 = id——延续 v2
//!    「替换 json 即换文案」的惯例，第三方放文件/文件夹即可分发）；
//! 3. 内置预设（[`BUILTIN_IDS`]）；
//! 4. classic（兜底，任何失效路径最终都落到这里）。

use std::path::{Path, PathBuf};

use crate::config::ConfigFile;

// ============================================================================
// 数据模型
// ============================================================================

/// 解析回退后的**完整**配色（#RRGGBB；字段名 = theme.json 键名 = CSS 变量名去掉
/// `--eyenurse-` 前缀的下划线形态，前端按同一规则注入）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkinColors {
    /// 展示名（中文）
    pub name: String,
    /// 展示名（英文；前端按当前语言二选一）
    pub name_en: String,
    pub bar_progress_bg: String,
    pub bar_progress_fill: String,
    pub bar_progress_from: String,
    pub bar_progress_to: String,
    pub bar_text_idle: String,
    pub bar_text_from: String,
    pub bar_text_to: String,
    pub bar_check_glyph: String,
    pub bar_check_hover_bg: String,
    pub mask_bg: String,
    pub mask_fg: String,
}

/// 悬浮条侧配色子集（可选键；磁盘/自定义的**部分**定义形态）
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BarSkinDef {
    pub progress_bg: Option<String>,
    pub progress_fill: Option<String>,
    pub progress_from: Option<String>,
    pub progress_to: Option<String>,
    pub text_idle: Option<String>,
    pub text_from: Option<String>,
    pub text_to: Option<String>,
    pub check_glyph: Option<String>,
    pub check_hover_bg: Option<String>,
}

/// 锁屏侧配色子集
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MaskSkinDef {
    pub bg: Option<String>,
    pub fg: Option<String>,
}

/// 部分皮肤定义（磁盘 JSON 与用户自定义的存储形态；缺键 = 回退 classic）
/// 扩展位：后续可加 `mask.bg_image`（背景图，需 assetProtocol + 拷贝到用户目录）、
/// 形态参数（bar 宽高/圆角，涉及动态窗口尺寸 + DPI 重算）。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SkinDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_en: Option<String>,
    pub bar: BarSkinDef,
    pub mask: MaskSkinDef,
}

/// Skin.json —— v3 新增（无 v2 对应文件；Current 手写 Default，不可 derive）
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SkinConfig {
    /// 当前选中皮肤 id（内置 id / 磁盘文件名 / "custom"）
    pub Current: String,
    /// 用户自定义配色（仅 `Current == "custom"` 时生效；编辑器保存即写入并启用）
    pub Custom: Option<SkinDef>,
}

impl ConfigFile for SkinConfig {
    const FILE_NAME: &'static str = "Skin.json";
}

impl Default for SkinConfig {
    fn default() -> Self {
        Self {
            Current: "classic".into(),
            Custom: None,
        }
    }
}

/// 皮肤清单条目（皮肤中心卡片数据源；colors = 该皮肤逐键回退后的完整配色，
/// 供页面直接渲染迷你预览）。has_bar/has_lock = 目录包提供的 HTML 表面徽标。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkinMeta {
    pub id: String,
    pub name: String,
    pub name_en: String,
    /// 作者（内置 = 巨应君；Tier 1 = manifest.author；Tier 0/自定义 = None）
    pub author: Option<String>,
    pub colors: SkinColors,
    pub has_bar: bool,
    pub has_lock: bool,
    /// Tier 1 包的 preview.png 是否存在（预览 URL 由 app 侧按协议拼装）
    pub has_preview: bool,
}

/// `get_skins` 回包：清单 + 当前选中 id
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkinsDto {
    pub current: String,
    pub skins: Vec<SkinMeta>,
}

/// 磁盘皮肤条目（id = 文件名去 .json / 目录名）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskSkin {
    pub id: String,
    pub def: SkinDef,
}

// ============================================================================
// ESP-1 Tier 1：目录包 manifest（docs/v3-skin-protocol.md §2）
// ============================================================================

/// manifest 文件名 / 协议标识 / 当前协议版本（高于宿主的包拒载，绝不半加载）
pub const MANIFEST_FILE: &str = "skin.json";
pub const SKIN_FORMAT: &str = "eyenurse-skin";
pub const SKIN_FORMAT_VERSION: u32 = 1;

/// manifest 展示名（`{"zh": "...", "en": "..."}`，双键必备——前端按当前语言取用）
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalizedName {
    pub zh: String,
    pub en: String,
}

/// bar target 声明：HTML 入口 + 窗口逻辑尺寸（DIP；缺省 80×20 由 app 侧兜底）
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TargetBarDef {
    /// 包内相对路径（`bar/index.html`）；解析时做穿越校验 + 存在性校验
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<SizeDef>,
}

/// bar 窗口逻辑尺寸（DIP；窗口管理仍归宿主，皮肤只声明想要多大）
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SizeDef {
    pub w: f64,
    pub h: f64,
}

/// lock target 声明：HTML 入口 + 宿主 chrome（密码框/解锁按钮）的摆放百分比
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TargetLockDef {
    pub entry: String,
    /// 宿主独占控件位置（百分比坐标；皮肤用数字对齐自己的设计，不做 DOM 测量）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chrome: Option<LockChromeDef>,
}

/// 宿主独占控件（密码框/解锁按钮）百分比位置（0-100，解析时 clamp）
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChromePos {
    pub x: f64,
    pub y: f64,
}

/// lock target 的宿主 chrome 摆放声明：密码框与解锁按钮由宿主独占渲染
///（皮肤 iframe 不可达——安全边界），只通过百分比坐标对齐皮肤设计。
/// 缺省位置由 app 侧兜底（密码框居中偏下、解锁按钮右下角）。
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LockChromeDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<ChromePos>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlock: Option<ChromePos>,
}

/// manifest `targets`（两个 target 均可缺省——缺哪个，哪个表面回退内置界面）
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SkinTargets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar: Option<TargetBarDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock: Option<TargetLockDef>,
}

/// ESP-1 manifest（`<包目录>/skin.json`）。`colors` 复用 [`SkinDef`]：
/// 给 Tier 1 皮肤当「classic 回退底色」，也让纯配色包与裸 json 语义一致。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SkinManifest {
    pub format: String,
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    pub id: String,
    pub name: LocalizedName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub targets: SkinTargets,
    #[serde(default)]
    pub colors: SkinDef,
}

impl Default for SkinManifest {
    fn default() -> Self {
        Self {
            format: SKIN_FORMAT.into(),
            format_version: SKIN_FORMAT_VERSION,
            id: String::new(),
            name: LocalizedName {
                zh: String::new(),
                en: String::new(),
            },
            author: None,
            version: None,
            targets: SkinTargets::default(),
            colors: SkinDef::default(),
        }
    }
}

/// 磁盘皮肤条目：Tier 0 裸 json（Colors）或 Tier 1 目录包（Package）。
/// SkinDef/manifest 装箱：抑制 clippy::large_enum_variant（Package 远大于 Colors）
#[derive(Debug, Clone, PartialEq)]
pub enum DiskSkinEntry {
    Colors { id: String, def: Box<SkinDef> },
    Package { id: String, manifest: Box<SkinManifest>, dir: PathBuf },
}

impl DiskSkinEntry {
    pub fn id(&self) -> &str {
        match self {
            DiskSkinEntry::Colors { id, .. } | DiskSkinEntry::Package { id, .. } => id,
        }
    }

    /// 配色定义（Tier 1 = manifest.colors；供逐键回退与清单预览）
    pub fn colors_def(&self) -> &SkinDef {
        match self {
            DiskSkinEntry::Colors { def, .. } => def,
            DiskSkinEntry::Package { manifest, .. } => &manifest.colors,
        }
    }

    /// 展示名（manifest.name 双语；Tier 0 走 def.name/name_en，可缺）
    pub fn display_name(&self) -> (Option<String>, Option<String>) {
        match self {
            DiskSkinEntry::Colors { def, .. } => (def.name.clone(), def.name_en.clone()),
            DiskSkinEntry::Package { manifest, .. } => {
                (Some(manifest.name.zh.clone()), Some(manifest.name.en.clone()))
            }
        }
    }

    /// 是否提供各 target 的 HTML 表面（设置页徽标 + bar 模式判定）
    pub fn has_surface(&self) -> (bool, bool) {
        match self {
            DiskSkinEntry::Colors { .. } => (false, false),
            DiskSkinEntry::Package { manifest, .. } => {
                (manifest.targets.bar.is_some(), manifest.targets.lock.is_some())
            }
        }
    }

    /// manifest 加载 + 全量校验（格式/版本/id/entry 路径穿越与存在性）。
    /// 任何一项不过 → Err，调用方跳过该包（与坏 Tier 0 json 同待遇）。
    pub fn load_package(dir: &Path) -> Result<Self, String> {
        let dir = dir
            .canonicalize()
            .map_err(|e| format!("目录不可达: {e}"))?;
        let Some(id) = dir.file_name().and_then(|s| s.to_str()) else {
            return Err("目录名不可用".into());
        };
        if !is_valid_skin_id(id) {
            return Err(format!("目录名 {id:?} 不符合 id 规范（[a-z0-9-]{{1,64}}）"));
        }
        let text = std::fs::read_to_string(dir.join(MANIFEST_FILE))
            .map_err(|e| format!("读取 {} 失败: {e}", MANIFEST_FILE))?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let manifest: SkinManifest = serde_json::from_str(text)
            .map_err(|e| format!("manifest 解析失败: {e}"))?;
        if manifest.format != SKIN_FORMAT {
            return Err(format!("format {:?} 不是 {:?}", manifest.format, SKIN_FORMAT));
        }
        if manifest.format_version != SKIN_FORMAT_VERSION {
            return Err(format!(
                "formatVersion {} 高于宿主支持（宿主 = {}）",
                manifest.format_version, SKIN_FORMAT_VERSION
            ));
        }
        if manifest.id != id {
            return Err(format!("manifest.id {:?} 与目录名 {:?} 不一致", manifest.id, id));
        }
        if manifest.name.zh.trim().is_empty() || manifest.name.en.trim().is_empty() {
            return Err("name.zh / name.en 不能为空".into());
        }
        // entry 路径纪律：包内相对路径 + 规范化后必须仍在包目录内（穿越即拒）
        let check_entry = |entry: &str, what: &str| -> Result<(), String> {
            if entry.is_empty()
                || entry.starts_with('/')
                || entry.starts_with('\\')
                || entry.contains(':')
                || entry.split(['/', '\\']).any(|seg| seg == "..")
            {
                return Err(format!("{what} entry {entry:?} 不是合法的包内相对路径"));
            }
            if !entry.to_ascii_lowercase().ends_with(".html") {
                return Err(format!("{what} entry {entry:?} 必须是 .html"));
            }
            let full = dir.join(entry);
            let canonical = full
                .canonicalize()
                .map_err(|e| format!("{what} entry {entry:?} 不可达: {e}"))?;
            if !canonical.starts_with(&dir) {
                return Err(format!("{what} entry {entry:?} 越出包目录"));
            }
            Ok(())
        };
        if let Some(bar) = &manifest.targets.bar {
            check_entry(&bar.entry, "bar")?;
        }
        if let Some(lock) = &manifest.targets.lock {
            check_entry(&lock.entry, "lock")?;
        }
        Ok(DiskSkinEntry::Package {
            id: id.to_string(),
            manifest: Box::new(manifest),
            dir,
        })
    }
}

/// 皮肤 id 规范：`[a-z0-9-]{1,64}`（目录名/裸 json 文件名同规则；"custom" 与
/// 内置 id 不会撞——它们不经过磁盘扫描命名空间）
fn is_valid_skin_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// app 侧复用的 id 校验入口（协议 handler 的第一段路径即 id）
pub fn is_valid_skin_id_pub(id: &str) -> bool {
    is_valid_skin_id(id)
}

// ============================================================================
// classic 真源 + 内置预设
// ============================================================================

/// 经典皮肤：与 `crates/eyenurse-app/src/ui/theme.json` 同值（v2 颜色表，
/// app 侧 `theme.rs` 的锚点测试与 build.rs 校验共同防漂移）
pub fn classic() -> SkinColors {
    SkinColors {
        name: "经典".into(),
        name_en: "Classic".into(),
        bar_progress_bg: "#F5F5DC".into(),
        bar_progress_fill: "#3AC32E".into(),
        bar_progress_from: "#ECC071".into(),
        bar_progress_to: "#3E0D0D".into(),
        bar_text_idle: "#000000".into(),
        bar_text_from: "#080808".into(),
        bar_text_to: "#F40000".into(),
        bar_check_glyph: "#444444".into(),
        bar_check_hover_bg: "#FFFFFF".into(),
        mask_bg: "#141418".into(),
        mask_fg: "#EEEEF2".into(),
    }
}

/// 内置皮肤 id（顺序 = 设置页展示顺序）。ESP-1 headless 化后：
/// - `classic`/`devtest` 是**内置目录包**（[`builtin_package`]，HTML 表面编译进
///   二进制，经 eyenurse-skin:// 协议与磁盘皮肤同管线服务）；
/// - `night`/`sakura`/`ocean` 仍是纯配色（bar 走 GDI 原生、lock 经 colors 桥
///   套 classic 包的锁屏形态）。
pub const BUILTIN_IDS: [&str; 5] = ["classic", "night", "sakura", "ocean", "devtest"];

/// 是否内置 id（磁盘扫描跳过这些目录/文件名——内嵌皮肤不可被磁盘包遮蔽）
pub fn is_builtin_id(id: &str) -> bool {
    BUILTIN_IDS.contains(&id)
}

/// 内置预设（编译进二进制；色值与深底/浅底的自洽由人工评审保证：
/// 深色 progress_bg 必须配浅色 text_idle 与浅色 check_hover_bg 上的深色 glyph）
pub fn builtin(id: &str) -> Option<SkinColors> {
    let (name, name_en) = match id {
        "classic" => ("经典", "Classic"),
        "night" => ("暗夜", "Night"),
        "sakura" => ("樱粉", "Sakura"),
        "ocean" => ("海蓝", "Ocean"),
        "devtest" => ("测试皮肤", "Dev Test"),
        _ => return None,
    };
    let mut c = classic();
    c.name = name.into();
    c.name_en = name_en.into();
    match id {
        "night" => {
            c.bar_progress_bg = "#1E1E24".into();
            c.bar_progress_fill = "#34C48D".into();
            c.bar_progress_from = "#C79A3A".into();
            c.bar_progress_to = "#8B1A1A".into();
            c.bar_text_idle = "#EDEDED".into();
            c.bar_text_from = "#EDEDED".into();
            c.bar_text_to = "#FF6B6B".into();
            c.bar_check_glyph = "#DDDDDD".into();
            c.bar_check_hover_bg = "#33333B".into();
            c.mask_bg = "#101216".into();
            c.mask_fg = "#E8EAEE".into();
        }
        "sakura" => {
            c.bar_progress_bg = "#FFF0F4".into();
            c.bar_progress_fill = "#FF7BAC".into();
            c.bar_progress_from = "#F2A0C0".into();
            c.bar_progress_to = "#B03060".into();
            c.bar_text_idle = "#4A2A35".into();
            c.bar_text_from = "#5A2A3A".into();
            c.bar_text_to = "#E05A8A".into();
            c.bar_check_glyph = "#A05A75".into();
            c.bar_check_hover_bg = "#FFE3EC".into();
            c.mask_bg = "#2A1520".into();
            c.mask_fg = "#FFE8F1".into();
        }
        "ocean" => {
            c.bar_progress_bg = "#EAF4FB".into();
            c.bar_progress_fill = "#2D9CDB".into();
            c.bar_progress_from = "#F2C94C".into();
            c.bar_progress_to = "#C0392B".into();
            c.bar_text_idle = "#10394A".into();
            c.bar_text_from = "#10394A".into();
            c.bar_text_to = "#E74C3C".into();
            c.bar_check_glyph = "#2A6E8F".into();
            c.bar_check_hover_bg = "#FFFFFF".into();
            c.mask_bg = "#0E1F2A".into();
            c.mask_fg = "#E3F2FD".into();
        }
        // devtest：高对比霓虹（协议验证皮肤——每个表面一眼可辨）
        "devtest" => {
            c.bar_progress_bg = "#101014".into();
            c.bar_progress_fill = "#00E676".into();
            c.bar_progress_from = "#FFD600".into();
            c.bar_progress_to = "#FF1744".into();
            c.bar_text_idle = "#00E676".into();
            c.bar_text_from = "#FFD600".into();
            c.bar_text_to = "#FF1744".into();
            c.bar_check_glyph = "#00E676".into();
            c.bar_check_hover_bg = "#1B2E24".into();
            c.mask_bg = "#0D001A".into();
            c.mask_fg = "#76FF03".into();
        }
        _ => {}
    }
    Some(c)
}

/// 内置目录包（ESP-1 headless）：`classic` 的锁屏界面与 `devtest` 全表面。
/// manifest 与 app 侧内嵌资产（skin_proto::embedded）一一对应——改 entry 必须
/// 同步内嵌文件映射。`dir` 语义为空：内嵌内容由协议按 id 直接服务。
///
/// - classic：无 bar entry（bar 走 GDI 原生——WebView2 冷启动可达秒级，默认
///   悬浮条保持零延迟；GDI 是 headless 宿主唯一的原生表面渲染器）；
/// - devtest：bar+lock 全 HTML 表面 + 异常位 chrome（密码框 y=74%、解锁按钮
///   居中）——专用于真机验证协议链路（表面加载/握手/状态推送/chrome 摆放）。
pub fn builtin_package(id: &str) -> Option<SkinManifest> {
    let mut m = SkinManifest {
        id: id.to_string(),
        author: Some("巨应君".into()),
        ..SkinManifest::default()
    };
    match id {
        "classic" => {
            m.name = LocalizedName {
                zh: "经典".into(),
                en: "Classic".into(),
            };
            m.targets.lock = Some(TargetLockDef {
                entry: "lock/index.html".into(),
                chrome: None,
            });
            Some(m)
        }
        "devtest" => {
            m.name = LocalizedName {
                zh: "测试皮肤".into(),
                en: "Dev Test".into(),
            };
            m.targets.bar = Some(TargetBarDef {
                entry: "bar/index.html".into(),
                size: Some(SizeDef { w: 160.0, h: 44.0 }),
            });
            m.targets.lock = Some(TargetLockDef {
                entry: "lock/index.html".into(),
                chrome: Some(LockChromeDef {
                    password: Some(ChromePos { x: 50.0, y: 74.0 }),
                    unlock: Some(ChromePos { x: 50.0, y: 88.0 }),
                }),
            });
            Some(m)
        }
        _ => None,
    }
}

/// 内置皮肤清单（皮肤中心固定置顶；表面徽标来自 [`builtin_package`] 声明）
pub fn builtin_metas() -> Vec<SkinMeta> {
    BUILTIN_IDS
        .iter()
        .filter_map(|id| {
            builtin(id).map(|colors| {
                let pkg = builtin_package(id);
                let (has_bar, has_lock) = pkg
                    .map(|m| (m.targets.bar.is_some(), m.targets.lock.is_some()))
                    .unwrap_or((false, false));
                SkinMeta {
                    id: (*id).into(),
                    name: colors.name.clone(),
                    name_en: colors.name_en.clone(),
                    author: Some("巨应君".into()),
                    colors,
                    has_bar,
                    has_lock,
                    has_preview: false,
                }
            })
        })
        .collect()
}

// ============================================================================
// 解析与回退
// ============================================================================

/// #RRGGBB 校验 + 规范化（大写）；非法返回 None
fn norm_color(s: &str) -> Option<String> {
    let t = s.trim();
    let body = t.strip_prefix('#')?;
    if body.len() != 6 || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", body.to_ascii_uppercase()))
}

/// 逐键回退合成：`def` 缺键/非法 → `classic` 对应键
pub fn resolve(def: &SkinDef, classic: &SkinColors) -> SkinColors {
    fn pick(opt: &Option<String>, fallback: &str) -> String {
        opt.as_deref().and_then(norm_color).unwrap_or_else(|| fallback.to_string())
    }
    SkinColors {
        name: def.name.clone().unwrap_or_else(|| classic.name.clone()),
        name_en: def.name_en.clone().unwrap_or_else(|| classic.name_en.clone()),
        bar_progress_bg: pick(&def.bar.progress_bg, &classic.bar_progress_bg),
        bar_progress_fill: pick(&def.bar.progress_fill, &classic.bar_progress_fill),
        bar_progress_from: pick(&def.bar.progress_from, &classic.bar_progress_from),
        bar_progress_to: pick(&def.bar.progress_to, &classic.bar_progress_to),
        bar_text_idle: pick(&def.bar.text_idle, &classic.bar_text_idle),
        bar_text_from: pick(&def.bar.text_from, &classic.bar_text_from),
        bar_text_to: pick(&def.bar.text_to, &classic.bar_text_to),
        bar_check_glyph: pick(&def.bar.check_glyph, &classic.bar_check_glyph),
        bar_check_hover_bg: pick(&def.bar.check_hover_bg, &classic.bar_check_hover_bg),
        mask_bg: pick(&def.mask.bg, &classic.mask_bg),
        mask_fg: pick(&def.mask.fg, &classic.mask_fg),
    }
}

/// 扫描磁盘皮肤目录（`Assets/Skins`）：`<id>.json` = Tier 0 颜色键，
/// `<id>/skin.json` = Tier 1 目录包。坏 JSON / 校验不过 / 非皮肤项跳过；
/// **内置 id（[`is_builtin_id`]）的文件/目录跳过**——内嵌皮肤不可被磁盘遮蔽；
/// 按 id 排序保证展示稳定。目录不存在 → 空表。
pub fn scan_disk(dir: &Path) -> Vec<DiskSkinEntry> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.extension().and_then(|e| e.to_str()) {
            // Tier 0：裸颜色键 json（v3.1 格式原样兼容）
            Some("json") => {
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if is_builtin_id(stem) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
                if let Ok(def) = serde_json::from_str::<SkinDef>(text) {
                    out.push(DiskSkinEntry::Colors {
                        id: stem.to_string(),
                        def: Box::new(def),
                    });
                }
            }
            // Tier 1：目录包（含 skin.json 才算；目录存在即尝试，校验不过静默跳过）
            None if path.is_dir() => {
                let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if is_builtin_id(name) {
                    continue;
                }
                if let Ok(pkg) = DiskSkinEntry::load_package(&path) {
                    out.push(pkg);
                }
            }
            _ => {}
        }
    }
    out.sort_by(|a, b| a.id().cmp(b.id()));
    out
}

/// 生效配色解析（优先级见模块注释）。`disk` 由调用方扫描后传入
///（app 侧定位 exe 旁 Assets/Skins；core 不感知 exe 路径）。
pub fn resolve_effective(config: &SkinConfig, disk: &[DiskSkinEntry]) -> SkinColors {
    let classic = classic();
    if config.Current == "custom" {
        if let Some(def) = &config.Custom {
            return resolve(def, &classic);
        }
    }
    if let Some(c) = builtin(&config.Current) {
        return c;
    }
    if let Some(e) = disk.iter().find(|e| e.id() == config.Current) {
        let mut colors = resolve(e.colors_def(), &classic);
        // Tier 1 的展示名来自 manifest.name（双语必备键），优先于 colors 回退名
        let (zh, en) = e.display_name();
        if let Some(zh) = zh {
            colors.name = zh;
        }
        if let Some(en) = en {
            colors.name_en = en;
        }
        return colors;
    }
    classic
}

// ============================================================================
// ESP-1 表面（target）解析：每个 target 独立生效、独立回退
// ============================================================================

/// 单个 target 的解析结果（入口相对路径 + 包目录绝对路径；app 侧拼装
/// `eyenurse-skin://` URL，core 不感知 URL 方案）
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSurface {
    pub dir: PathBuf,
    /// 包内 HTML 入口（manifest 校验时已做穿越与存在性检查）
    pub entry: String,
    /// bar：窗口逻辑尺寸（DIP；None = app 侧用默认 80×20）
    pub size: Option<SizeDef>,
    /// lock：宿主独占控件（密码框/解锁按钮）百分比位置
    pub chrome: Option<LockChromeDef>,
}

/// 生效皮肤的全量解析：配色（逐键回退）+ 两个 target 的 HTML 表面（各自独立）。
///
/// **headless 回退规则**：宿主壳页没有内置界面——
/// - lock **永不为 None**：激活皮肤无 lock 表面（纯配色/自定义/内置纯色预设/
///   磁盘 id 失效）→ 回退内置 `classic` 包的锁屏（经 colors 桥吃当前配色，
///   Tier 0 换色照常生效）；
/// - bar 为 None = GDI 原生条（渲染器级回退，也是默认悬浮条——零冷启动延迟）。
///
/// 磁盘 id 失效 → 配色落 classic、lock 落 classic 包。
pub fn resolve_surfaces(
    config: &SkinConfig,
    disk: &[DiskSkinEntry],
) -> (
    SkinColors,
    Option<ResolvedSurface>,
    Option<ResolvedSurface>,
) {
    let colors = resolve_effective(config, disk);
    let (bar, lock) = match config.Current.as_str() {
        "custom" => (None, None),
        id => match builtin_package(id) {
            Some(m) => surfaces_of(&m, &PathBuf::new()),
            None => match disk.iter().find(|e| e.id() == id) {
                Some(DiskSkinEntry::Package { manifest, dir, .. }) => {
                    surfaces_of(manifest, dir)
                }
                _ => (None, None),
            },
        },
    };
    let lock = lock.or_else(|| {
        builtin_package("classic").and_then(|m| m.targets.lock).map(|t| ResolvedSurface {
            dir: PathBuf::new(),
            entry: t.entry,
            size: None,
            chrome: t.chrome,
        })
    });
    (colors, bar, lock)
}

/// manifest targets → 解析表面（dir 由调用方给：磁盘包为真实目录，内置包为空
/// 占位——内嵌内容由协议 handler 按 id 服务，不落盘）
fn surfaces_of(manifest: &SkinManifest, dir: &Path) -> (Option<ResolvedSurface>, Option<ResolvedSurface>) {
    let bar = manifest.targets.bar.as_ref().map(|t| ResolvedSurface {
        dir: dir.to_path_buf(),
        entry: t.entry.clone(),
        size: t.size,
        chrome: None,
    });
    let lock = manifest.targets.lock.as_ref().map(|t| ResolvedSurface {
        dir: dir.to_path_buf(),
        entry: t.entry.clone(),
        size: None,
        chrome: t.chrome,
    });
    (bar, lock)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigStore;

    #[test]
    fn classic_matches_theme_json_values() {
        // 真源锚点：与 app 侧 theme.rs 锚点测试同源（theme.json 改值必须同步这里）
        let c = classic();
        assert_eq!(c.bar_progress_bg, "#F5F5DC");
        assert_eq!(c.bar_progress_fill, "#3AC32E");
        assert_eq!(c.bar_progress_from, "#ECC071");
        assert_eq!(c.bar_progress_to, "#3E0D0D");
        assert_eq!(c.bar_text_to, "#F40000");
        assert_eq!(c.mask_bg, "#141418");
        assert_eq!(c.mask_fg, "#EEEEF2");
    }

    #[test]
    fn resolve_missing_and_invalid_keys_fall_back_to_classic() {
        let def = SkinDef {
            name: Some("日落".into()),
            bar: BarSkinDef {
                progress_fill: Some("#ff6b35".into()), // 小写 → 规范化大写
                progress_bg: Some("red".into()),       // 非法 → 回退
                ..BarSkinDef::default()
            },
            ..SkinDef::default()
        };
        let c = resolve(&def, &classic());
        assert_eq!(c.name, "日落");
        assert_eq!(c.name_en, "Classic"); // name 缺 → classic
        assert_eq!(c.bar_progress_fill, "#FF6B35"); // 规范化大写
        assert_eq!(c.bar_progress_bg, "#F5F5DC"); // 非法回退
        assert_eq!(c.mask_bg, "#141418"); // 缺键回退
    }

    #[test]
    fn builtin_all_ids_resolve_and_differ_from_classic() {
        for id in BUILTIN_IDS {
            let c = builtin(id).unwrap_or_else(|| panic!("{id} 应存在"));
            assert!(c.bar_progress_bg.starts_with('#') && c.bar_progress_bg.len() == 7);
        }
        assert!(builtin("nope").is_none());
        let night = builtin("night").unwrap();
        assert_ne!(night.bar_progress_bg, classic().bar_progress_bg);
    }

    #[test]
    fn effective_priority_custom_over_builtin_over_disk_over_classic() {
        let mut config = SkinConfig::default();
        let disk = vec![DiskSkinEntry::Colors {
            id: "sunset".into(),
            def: Box::new(SkinDef {
                name: Some("日落".into()),
                ..SkinDef::default()
            }),
        }];

        // classic
        assert_eq!(resolve_effective(&config, &disk), classic());

        // 磁盘
        config.Current = "sunset".into();
        assert_eq!(resolve_effective(&config, &disk).name, "日落");

        // 磁盘 id 失效（文件被删）→ classic
        assert_eq!(resolve_effective(&config, &[]).name, "经典");

        // 内置
        config.Current = "night".into();
        assert_eq!(resolve_effective(&config, &disk).name_en, "Night");

        // 自定义最高
        config.Current = "custom".into();
        config.Custom = Some(SkinDef {
            name: Some("我的".into()),
            ..SkinDef::default()
        });
        assert_eq!(resolve_effective(&config, &disk).name, "我的");

        // Current=custom 但 Custom 缺失 → classic 兜底
        config.Custom = None;
        assert_eq!(resolve_effective(&config, &disk), classic());
    }

    #[test]
    fn skin_config_roundtrip_and_partial_json() {
        let dir = std::env::temp_dir().join(format!("eyenurse-skin-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::with_dir(dir.clone());

        // 缺文件 → 默认 classic
        assert_eq!(store.load::<SkinConfig>(), SkinConfig::default());

        let config = SkinConfig {
            Current: "custom".into(),
            Custom: Some(SkinDef {
                name: Some("我的".into()),
                bar: BarSkinDef {
                    progress_fill: Some("#123ABC".into()),
                    ..BarSkinDef::default()
                },
                ..SkinDef::default()
            }),
        };
        store.save(&config).unwrap();
        assert_eq!(store.load::<SkinConfig>(), config);

        // 部分字段磁盘文件：缺成员留默认（serde(default) 容器语义）
        std::fs::write(dir.join(SkinConfig::FILE_NAME), "{\"Current\":\"night\"}").unwrap();
        let loaded = store.load::<SkinConfig>();
        assert_eq!(loaded.Current, "night");
        assert!(loaded.Custom.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_disk_skips_broken_and_sorts_by_id() {
        let dir = std::env::temp_dir().join(format!("eyenurse-skin-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.json"), "{\"name\":\"B\"}").unwrap();
        std::fs::write(dir.join("a.json"), "{\"name\":\"A\"}").unwrap();
        std::fs::write(dir.join("bad.json"), "{broken").unwrap();
        std::fs::write(dir.join("note.txt"), "not a skin").unwrap();

        let skins = scan_disk(&dir);
        let ids: Vec<&str> = skins.iter().map(|s| s.id()).collect();
        assert_eq!(ids, ["a", "b"]); // 坏 JSON 与非 json 跳过，按 id 排序
        std::fs::remove_dir_all(&dir).ok();
    }

    // —— ESP-1 Tier 1：manifest 校验 + 表面解析 ——

    /// 造一个最小合法包目录（含 entry HTML）
    fn make_package(root: &Path, id: &str, manifest_body: &str) -> PathBuf {
        let dir = root.join(id);
        std::fs::create_dir_all(dir.join("bar")).unwrap();
        std::fs::create_dir_all(dir.join("lock")).unwrap();
        std::fs::write(dir.join("bar").join("index.html"), "<html></html>").unwrap();
        std::fs::write(dir.join("lock").join("index.html"), "<html></html>").unwrap();
        let body = manifest_body
            .replace("%ID%", id)
            .replace("%BAR_ENTRY%", "bar/index.html")
            .replace("%LOCK_ENTRY%", "lock/index.html");
        std::fs::write(dir.join(MANIFEST_FILE), body).unwrap();
        dir
    }

    const VALID_MANIFEST: &str = r##"{
        "format": "eyenurse-skin", "formatVersion": 1, "id": "%ID%",
        "name": {"zh": "日落时钟", "en": "Sunset Clock"},
        "author": "giantapp", "version": "1.0.0",
        "targets": {
            "bar":  {"entry": "%BAR_ENTRY%", "size": {"w": 120, "h": 32}},
            "lock": {"entry": "%LOCK_ENTRY%", "chrome": {"password": {"x": 50, "y": 64}}}
        },
        "colors": {"bar": {"progress_fill": "#FF6B35"}, "mask": {"bg": "#1A0E08"}}
    }"##;

    #[test]
    fn package_manifest_validates_and_surfaces_resolve_per_target() {
        let root = std::env::temp_dir().join(format!("eyenurse-skin-pkg-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let dir = make_package(&root, "sunset-clock", VALID_MANIFEST);

        let pkg = DiskSkinEntry::load_package(&dir).expect("合法包应通过校验");
        assert_eq!(pkg.id(), "sunset-clock");
        assert_eq!(pkg.has_surface(), (true, true));

        // 表面解析：两个 target 各自带入口/尺寸/chrome
        let config = SkinConfig {
            Current: "sunset-clock".into(),
            Custom: None,
        };
        let disk = scan_disk(&root);
        let (colors, bar, lock) = resolve_surfaces(&config, &disk);
        assert_eq!(colors.name, "日落时钟"); // manifest.name 优先
        assert_eq!(colors.bar_progress_fill, "#FF6B35"); // colors 逐键回退仍生效
        assert_eq!(colors.bar_progress_bg, "#F5F5DC"); // 缺键回退 classic
        let bar = bar.expect("bar 表面应存在");
        assert_eq!(bar.entry, "bar/index.html");
        assert_eq!(bar.size, Some(SizeDef { w: 120.0, h: 32.0 }));
        let lock = lock.expect("lock 表面应存在");
        assert_eq!(
            lock.chrome.and_then(|c| c.password),
            Some(ChromePos { x: 50.0, y: 64.0 })
        );

        // 只做 lock 的包：bar 回退无表面（内置界面）
        let lock_only = r#"{
            "format": "eyenurse-skin", "formatVersion": 1, "id": "%ID%",
            "name": {"zh": "只锁屏", "en": "LockOnly"},
            "targets": {"lock": {"entry": "%LOCK_ENTRY%"}}
        }"#;
        let dir2 = make_package(&root, "lock-only", lock_only);
        let pkg2 = DiskSkinEntry::load_package(&dir2).unwrap();
        assert_eq!(pkg2.has_surface(), (false, true));
        let config2 = SkinConfig {
            Current: "lock-only".into(),
            Custom: None,
        };
        let disk2 = scan_disk(&root);
        let (_, bar2, lock2) = resolve_surfaces(&config2, &disk2);
        assert!(bar2.is_none());
        assert!(lock2.is_some());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn package_manifest_rejects_bad_inputs() {
        let root = std::env::temp_dir().join(format!("eyenurse-skin-bad-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();

        // formatVersion 高于宿主
        let d = make_package(
            &root,
            "future-skin",
            &VALID_MANIFEST.replace("\"formatVersion\": 1", "\"formatVersion\": 2"),
        );
        assert!(DiskSkinEntry::load_package(&d).is_err());

        // manifest.id 与目录名不一致
        let d = make_package(&root, "actual-name", &VALID_MANIFEST.replace("%ID%", "other-name"));
        assert!(DiskSkinEntry::load_package(&d).is_err());

        // entry 路径穿越
        let d = make_package(
            &root,
            "escapee",
            &VALID_MANIFEST.replace("%BAR_ENTRY%", "../../outside.html"),
        );
        assert!(DiskSkinEntry::load_package(&d).is_err());

        // entry 不存在
        let d = make_package(
            &root,
            "missing-entry",
            &VALID_MANIFEST.replace("%LOCK_ENTRY%", "lock/missing.html"),
        );
        assert!(DiskSkinEntry::load_package(&d).is_err());

        // id 不规范（大写）
        let d = make_package(&root, "BadID", &VALID_MANIFEST.replace("%ID%", "BadID"));
        assert!(DiskSkinEntry::load_package(&d).is_err());

        // 无 manifest 的目录不算皮肤（scan_disk 跳过）
        std::fs::create_dir_all(root.join("empty-dir")).unwrap();
        let scanned = scan_disk(&root);
        assert_eq!(scanned.len(), 0); // 全部坏包 → 空
        assert!(DiskSkinEntry::load_package(&root.join("empty-dir")).is_err());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scan_disk_mixes_tier0_and_tier1_sorted_by_id() {
        let dir = std::env::temp_dir().join(format!("eyenurse-skin-mix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("aaa.json"), "{\"name\":\"A\"}").unwrap();
        make_package(&dir, "zzz-clock", VALID_MANIFEST);

        let skins = scan_disk(&dir);
        let ids: Vec<&str> = skins.iter().map(|s| s.id()).collect();
        assert_eq!(ids, ["aaa", "zzz-clock"]);
        assert_eq!(skins[0].has_surface(), (false, false)); // Tier 0 无表面
        assert_eq!(skins[1].has_surface(), (true, true));

        // Tier 1 也走 resolve_effective（配色 = manifest.colors 逐键回退）
        let config = SkinConfig {
            Current: "zzz-clock".into(),
            Custom: None,
        };
        let colors = resolve_effective(&config, &skins);
        assert_eq!(colors.name, "日落时钟");
        assert_eq!(colors.name_en, "Sunset Clock");

        std::fs::remove_dir_all(&dir).ok();
    }

    // —— ESP-1 headless：内置包 + 永不空锁屏 ——

    #[test]
    fn builtin_packages_declare_surfaces() {
        // classic：仅 lock（bar = GDI 原生，零冷启动）
        let c = builtin_package("classic").expect("classic 包应存在");
        assert!(c.targets.bar.is_none());
        assert_eq!(c.targets.lock.as_ref().unwrap().entry, "lock/index.html");
        // devtest：bar+lock 全表面 + 异常位 chrome（协议验证点）
        let d = builtin_package("devtest").expect("devtest 包应存在");
        assert_eq!(
            d.targets.bar.as_ref().unwrap().size,
            Some(SizeDef { w: 160.0, h: 44.0 })
        );
        let chrome = d.targets.lock.as_ref().unwrap().chrome.unwrap();
        assert_eq!(chrome.password, Some(ChromePos { x: 50.0, y: 74.0 }));
        // 纯色预设无包
        assert!(builtin_package("night").is_none());
        assert!(builtin_package("custom").is_none());
        // 清单徽标
        let metas = builtin_metas();
        let classic = metas.iter().find(|m| m.id == "classic").unwrap();
        assert!(!classic.has_bar && classic.has_lock);
        let devtest = metas.iter().find(|m| m.id == "devtest").unwrap();
        assert!(devtest.has_bar && devtest.has_lock);
        assert_eq!(devtest.colors.bar_progress_fill, "#00E676");
    }

    #[test]
    fn lock_surface_never_none_and_bar_falls_back_to_gdi() {
        // classic：lock = 自己的包，bar = None（GDI）
        let config = SkinConfig {
            Current: "classic".into(),
            Custom: None,
        };
        let (_, bar, lock) = resolve_surfaces(&config, &[]);
        assert!(bar.is_none());
        assert_eq!(lock.unwrap().entry, "lock/index.html");

        // 纯色预设 night：lock 回退 classic 包（colors 桥吃 night 配色），bar = GDI
        let config = SkinConfig {
            Current: "night".into(),
            Custom: None,
        };
        let (colors, bar, lock) = resolve_surfaces(&config, &[]);
        assert_eq!(colors.mask_bg, "#101216");
        assert!(bar.is_none());
        assert_eq!(lock.unwrap().entry, "lock/index.html");

        // 自定义编辑器：同规则
        let config = SkinConfig {
            Current: "custom".into(),
            Custom: Some(SkinDef::default()),
        };
        let (_, _, lock) = resolve_surfaces(&config, &[]);
        assert!(lock.is_some());

        // 磁盘 id 失效：lock 仍不空
        let config = SkinConfig {
            Current: "gone-skin".into(),
            Custom: None,
        };
        let (colors, _, lock) = resolve_surfaces(&config, &[]);
        assert_eq!(colors.name, "经典");
        assert!(lock.is_some());

        // devtest：bar 有表面（WebView 壳）
        let config = SkinConfig {
            Current: "devtest".into(),
            Custom: None,
        };
        let (_, bar, _) = resolve_surfaces(&config, &[]);
        assert_eq!(bar.unwrap().entry, "bar/index.html");
    }

    #[test]
    fn scan_disk_skips_builtin_ids() {
        let dir = std::env::temp_dir().join(format!("eyenurse-skin-shadow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 磁盘伪装内置：两种形态都不得入选（内嵌皮肤不可被遮蔽）
        std::fs::write(dir.join("classic.json"), "{\"name\":\"假经典\"}").unwrap();
        std::fs::create_dir_all(dir.join("devtest")).unwrap();
        std::fs::write(dir.join("devtest").join(MANIFEST_FILE), "{}").unwrap();
        std::fs::write(dir.join("real.json"), "{\"name\":\"R\"}").unwrap();

        let skins = scan_disk(&dir);
        let ids: Vec<&str> = skins.iter().map(|s| s.id()).collect();
        assert_eq!(ids, ["real"]);

        std::fs::remove_dir_all(&dir).ok();
    }
}
