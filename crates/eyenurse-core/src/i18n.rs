//! I18n（架构文档 §4.5）：加载 Assets/Languages/*.json 的平铺 `{key: value}` 文案。
//!
//! 加载顺序：优先磁盘（保持 v2 的“替换 json 即换文案”能力），磁盘目录不存在时
//! 由调用方回退 `load_embedded` 内嵌副本。内嵌副本的真源在
//! `crates/eyenurse-core/assets/languages/{zh,en}.json`（§4.5 评审修订：单一真源，
//! D 打包时直接从此目录拷贝，不再维护第二份）。
//!
//! v2 键集（Assets/Languages/*.json）：about, always_on_top, app_name, exit,
//! installNow, installWebview2Tips, pause, reset, rest_now, resting, resume, setting；
//! v3 增加设置窗键（v2 Vue locales/*.json）：clientSettings, copyright, home, panel,
//! restDuration, restInterval, runWhenStarts, resetWhenSessionUnlock, settings, zh, en；
//! v3 新增：restoreDefaults（设置窗“恢复默认”入口，清单 F11；v2 的
//! ClientApi.ResetSettings 有后端无入口）。
//! 占位符：`{year}` 在 get/texts 输出时展开为本地当前年份（版权行随年份滚动）。
//! `installWebview2Tips`/`installNow` 不再使用但保留（v2 文件兼容）。

use std::collections::BTreeMap;
use std::path::Path;

pub struct I18n {
    current: String,
    fallback: String,
    /// 语言名 → key → value（BTreeMap：键有序，available() 天然排序）
    langs: BTreeMap<String, BTreeMap<String, String>>,
}

impl I18n {
    /// dir: exe 旁 "Assets/Languages"；current: LanguagesJson.CurrentLan；
    /// fallback: "en"；system: sys_locale::get_locale()（如 "zh-CN"）。
    /// current 为 None/" " 时：system 以 "zh" 开头 → "zh"，否则 fallback。
    /// 每语言文件缺失 → 空表；坏 JSON → 空表（不 panic，对齐 v2 容错风格）。
    pub fn load(dir: &Path, current: Option<&str>, fallback: &str, system: &str) -> Self {
        let mut langs = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_json = path.extension().and_then(|e| e.to_str()) == Some("json");
                if !is_json {
                    continue;
                }
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                // 文件读不到/解析失败 → 该语言空表（仍计入 available，get 走回退）
                let map = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| parse_flat_json(&text))
                    .unwrap_or_default();
                langs.insert(name.to_string(), map);
            }
        }
        Self::build(langs, current, fallback, system)
    }

    /// 内嵌副本加载（磁盘目录不存在时的兜底；单一真源见模块注释）
    pub fn load_embedded(current: Option<&str>, fallback: &str, system: &str) -> Self {
        const ZH: &str = include_str!("../assets/languages/zh.json");
        const EN: &str = include_str!("../assets/languages/en.json");
        let mut langs = BTreeMap::new();
        for (name, text) in [("zh", ZH), ("en", EN)] {
            let map = parse_flat_json(text).unwrap_or_default();
            langs.insert(name.to_string(), map);
        }
        Self::build(langs, current, fallback, system)
    }

    fn build(
        langs: BTreeMap<String, BTreeMap<String, String>>,
        current: Option<&str>,
        fallback: &str,
        system: &str,
    ) -> Self {
        let fallback = fallback.to_string();
        // current 为 None/空白：系统语言以 "zh" 开头 → "zh"，否则 fallback（v2 LanService.Init 语义）
        let chosen = match current.map(str::trim) {
            Some(c) if !c.is_empty() => c.to_string(),
            _ => {
                if system.to_ascii_lowercase().starts_with("zh") {
                    "zh".to_string()
                } else {
                    fallback.clone()
                }
            }
        };
        // 未知语言（目录下没有对应 json）→ 回退 fallback
        let current = if langs.contains_key(&chosen) {
            chosen
        } else {
            fallback.clone()
        };
        Self {
            current,
            fallback,
            langs,
        }
    }

    /// 当前语言整包文案（current → fallback 合并；fallback 缺失的 key 原样保留）。
    /// v3 对 core 的唯一增量（tauri 架构文档 §3/§9）：`get_texts` 命令的数据源，
    /// settings/mask 页面一次取全，语言切换时重取。
    pub fn texts(&self) -> BTreeMap<String, String> {
        let mut merged = self
            .langs
            .get(&self.fallback)
            .cloned()
            .unwrap_or_default();
        if let Some(m) = self.langs.get(&self.current) {
            for (k, v) in m {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged
            .into_iter()
            .map(|(k, v)| (k, expand_placeholders(&v)))
            .collect()
    }

    /// 当前语言 → 回退语言 → 原样返回 key（v2 LanService.Get 语义）
    pub fn get(&self, key: &str) -> String {
        let text = self
            .langs
            .get(&self.current)
            .and_then(|m| m.get(key))
            .or_else(|| self.langs.get(&self.fallback).and_then(|m| m.get(key)))
            .cloned()
            .unwrap_or_else(|| key.to_string());
        expand_placeholders(&text)
    }

    /// 目录下 *.json 文件名（去扩展名）排序
    pub fn available(&self) -> Vec<String> {
        self.langs.keys().cloned().collect()
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    /// 未知语言 → 回退 fallback
    pub fn set_language(&mut self, lan: &str) {
        if self.langs.contains_key(lan) {
            self.current = lan.to_string();
        } else {
            self.current = self.fallback.clone();
        }
    }
}

/// 解析平铺 `{key: value}`；容忍 UTF-8 BOM（v2 资产文件带 BOM，用户直接拷贝 v2
/// 的 json 进 Assets/Languages 也能读）
fn parse_flat_json(text: &str) -> Option<BTreeMap<String, String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    serde_json::from_str(text).ok()
}

/// 文案占位符展开：`{year}` → 本地当前年份（版权行 "2022-{year}" 随年份滚动，
/// 免去每年手改 json）；不含占位符的文案原样返回。
fn expand_placeholders(value: &str) -> String {
    let today = crate::stats::local_today();
    value.replace("{year}", &today[..4])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir_with_files(files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eyenurse-i18n-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, content) in files {
            std::fs::write(dir.join(name), content).unwrap();
        }
        dir
    }

    #[test]
    fn embedded_contains_merged_keyset() {
        let i = I18n::load_embedded(None, "en", "zh-CN");
        assert_eq!(i.current(), "zh");
        assert_eq!(i.get("app_name"), "眼睛护士3");
        assert_eq!(i.get("clientSettings"), "客户端设置");
        assert_eq!(i.available(), vec!["en".to_string(), "zh".to_string()]);
        // 未知 key 原样返回
        assert_eq!(i.get("no_such_key"), "no_such_key");
    }

    #[test]
    fn fallback_and_unknown_language() {
        let i = I18n::load_embedded(None, "en", "en-US");
        assert_eq!(i.current(), "en");
        assert_eq!(i.get("app_name"), "EyeNurse 3");

        let mut i = I18n::load_embedded(None, "en", "zh-CN");
        i.set_language("fr"); // 未知语言 → 回退 fallback
        assert_eq!(i.current(), "en");
        i.set_language("zh");
        assert_eq!(i.current(), "zh");
        assert_eq!(i.get("pause"), "暂停");
    }

    #[test]
    fn empty_current_uses_system_locale() {
        // CurrentLan = Some(" ") → 视为未选择
        let i = I18n::load_embedded(Some(" "), "en", "zh-CN");
        assert_eq!(i.current(), "zh");
        let i = I18n::load_embedded(Some(" "), "en", "ja-JP");
        assert_eq!(i.current(), "en");
    }

    #[test]
    fn year_placeholder_expands_to_local_year() {
        let i = I18n::load_embedded(None, "en", "zh-CN");
        let year = &crate::stats::local_today()[..4];
        assert_eq!(i.get("copyright"), format!("&copy; 2022-{year} 巨应君"));
        assert!(i.texts()["copyright"].contains(year));
        // 无占位符的键不受影响
        assert_eq!(i.get("app_name"), "眼睛护士3");
    }

    #[test]
    fn disk_load_with_bom_and_bad_json() {
        let dir = temp_dir_with_files(&[
            ("zh.json", "\u{feff}{\"pause\": \"暂停(磁盘)\"}"),
            ("en.json", "{broken json"),
        ]);
        let i = I18n::load(&dir, Some("zh"), "en", "en-US");
        assert_eq!(i.get("pause"), "暂停(磁盘)"); // BOM 被容忍
        assert_eq!(i.get("reset"), "reset"); // en 空表且当前为 zh → 原样返回 key
        std::fs::remove_dir_all(&dir).ok();
    }
}
