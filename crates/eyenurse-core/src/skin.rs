//! 皮肤系统（v3.1 换肤）：悬浮条 + 锁屏窗的**运行时**配色。
//!
//! 真源与回退：`classic` 与编译期 `crates/eyenurse-app/src/ui/theme.json` 同值
//! （theme.json 仍是「改经典色」的唯一入口，build.rs 校验不变；本文件字面量由
//! app 侧锚点测试保护，见 eyenurse-app `ui/theme.rs` tests）。其他皮肤以 classic
//! 为底做**逐键回退**——缺键或非法色值 → classic 对应键。
//!
//! 来源优先级（[`resolve_effective`]）：
//! 1. 用户自定义：Skin.json 的 `Custom`（`Current == "custom"` 时生效）；
//! 2. 磁盘皮肤：exe 旁 `Assets/Skins/<id>.json`，文件名 = id（延续 v2
//!    「替换 json 即换文案」的惯例，第三方放文件即可分发）；
//! 3. 内置预设（[`BUILTIN_IDS`]）；
//! 4. classic（兜底，任何失效路径最终都落到这里）。

use std::path::Path;

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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// 皮肤清单条目（设置页选择器数据源；colors = 该皮肤逐键回退后的完整配色，
/// 供页面直接渲染迷你预览）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkinMeta {
    pub id: String,
    pub name: String,
    pub name_en: String,
    pub colors: SkinColors,
}

/// `get_skins` 回包：清单 + 当前选中 id
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkinsDto {
    pub current: String,
    pub skins: Vec<SkinMeta>,
}

/// 磁盘皮肤条目（id = 文件名去 .json）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskSkin {
    pub id: String,
    pub def: SkinDef,
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

/// 内置预设 id（顺序 = 设置页展示顺序）
pub const BUILTIN_IDS: [&str; 4] = ["classic", "night", "sakura", "ocean"];

/// 内置预设（编译进二进制；色值与深底/浅底的自洽由人工评审保证：
/// 深色 progress_bg 必须配浅色 text_idle 与浅色 check_hover_bg 上的深色 glyph）
pub fn builtin(id: &str) -> Option<SkinColors> {
    let (name, name_en) = match id {
        "classic" => ("经典", "Classic"),
        "night" => ("暗夜", "Night"),
        "sakura" => ("樱粉", "Sakura"),
        "ocean" => ("海蓝", "Ocean"),
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
        _ => {}
    }
    Some(c)
}

/// 内置皮肤清单（设置页固定置顶）
pub fn builtin_metas() -> Vec<SkinMeta> {
    BUILTIN_IDS
        .iter()
        .filter_map(|id| {
            builtin(id).map(|colors| SkinMeta {
                id: (*id).into(),
                name: colors.name.clone(),
                name_en: colors.name_en.clone(),
                colors,
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

/// 扫描磁盘皮肤目录（`Assets/Skins/*.json`）：文件名（去扩展名）= id，
/// 坏 JSON / 非文件项跳过；按 id 排序保证展示稳定。目录不存在 → 空表。
pub fn scan_disk(dir: &Path) -> Vec<DiskSkin> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        if let Ok(def) = serde_json::from_str::<SkinDef>(text) {
            out.push(DiskSkin {
                id: stem.to_string(),
                def,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// 生效配色解析（优先级见模块注释）。`disk` 由调用方扫描后传入
///（app 侧定位 exe 旁 Assets/Skins；core 不感知 exe 路径）。
pub fn resolve_effective(config: &SkinConfig, disk: &[DiskSkin]) -> SkinColors {
    let classic = classic();
    if config.Current == "custom" {
        if let Some(def) = &config.Custom {
            return resolve(def, &classic);
        }
    }
    if let Some(c) = builtin(&config.Current) {
        return c;
    }
    if let Some(e) = disk.iter().find(|e| e.id == config.Current) {
        return resolve(&e.def, &classic);
    }
    classic
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
        let disk = vec![DiskSkin {
            id: "sunset".into(),
            def: SkinDef {
                name: Some("日落".into()),
                ..SkinDef::default()
            },
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
        let ids: Vec<&str> = skins.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]); // 坏 JSON 与非 json 跳过，按 id 排序
        std::fs::remove_dir_all(&dir).ok();
    }
}
