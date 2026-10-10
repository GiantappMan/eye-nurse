//! 与 v2 字节级兼容的持久化（架构文档 §4.3）。
//!
//! 路径与文件名（外部库 ConfigService.cs，附录 §11）：
//! `%LOCALAPPDATA%\EyeNurse\UserData\<TypeName>.json`。
//! 序列化规则（外部库 JsonHelper.cs：System.Text.Json **默认选项**）：
//! - 属性名原样 PascalCase、按声明顺序输出（serde 结构体字段序 = v2 声明序）；
//! - 紧凑（无缩进、无空格）；
//! - `null` 引用照写（`"CurrentLan":null`）；
//! - 坏 JSON / 缺文件 → 整个对象取默认值（v2 `JsonDeserialize` 吞异常返回 `default(T)`）；
//! - 保存失败不 panic（v2 只记日志），返回 `Err` 由上层记录。
//!
//! clamp 时机：`load` 读盘路径**从不** clamp；`clamp_like_v2` 只在设置窗
//! “文本 → 数值”保存路径调用（架构文档 §4.3 评审修订）。

use std::path::{Path, PathBuf};

use crate::time::HmsSpan;

pub struct ConfigStore {
    user_config_dir: PathBuf,
}

impl ConfigStore {
    /// %LOCALAPPDATA%\EyeNurse\UserData（v2 AppName 固定为 "EyeNurse"，App.xaml.cs:37）
    pub fn v2_default() -> Self {
        Self::with_dir(local_app_data_dir().join("EyeNurse").join("UserData"))
    }

    /// 单测用：指定目录（临时目录）
    pub fn with_dir(dir: PathBuf) -> Self {
        Self { user_config_dir: dir }
    }

    pub fn dir(&self) -> &Path {
        &self.user_config_dir
    }

    /// 读 `<T>::FILE_NAME`；缺失/坏 JSON → `T::default()`。
    /// 容忍 UTF-8 BOM（v2 走 `File.ReadAllText` 会自动剥 BOM，手编带 BOM 的文件
    /// 在 v2 下能正常加载，v3 需同样处理，否则坏 JSON 兜底会悄悄回默认值）。
    pub fn load<T: ConfigFile + Default + serde::de::DeserializeOwned>(&self) -> T {
        let path = self.user_config_dir.join(T::FILE_NAME);
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
                serde_json::from_str(text).ok().unwrap_or_default()
            }
            Err(_) => T::default(),
        }
    }

    /// 写 `<T>::FILE_NAME`；目录不存在则创建；紧凑序列化。
    /// 先序列化成功才落盘（对齐 v2 JsonHelper.Save：序列化异常则原文件不动）。
    pub fn save<T: ConfigFile + serde::Serialize>(&self, value: &T) -> std::io::Result<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        std::fs::create_dir_all(&self.user_config_dir)?;
        std::fs::write(self.user_config_dir.join(T::FILE_NAME), json)
    }
}

/// 配置文件名（v2 文件名 = 类型名 + `.json`）
pub trait ConfigFile {
    const FILE_NAME: &'static str;
}

/// Setting.json —— v2 Setting.cs:35-48，字段顺序/默认值逐项一致。
/// 容器级 #[serde(default)] 要求 T: Default 且默认值不是零值，因此手写 impl Default
/// （derive 出来的零值是错的，架构文档 §4.3 评审修订）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SettingJson {
    pub RestInterval: HmsSpan,             // "00:45:00" = new TimeSpan(0,45,0)（Setting.cs:7）
    pub RestDuration: HmsSpan,             // "00:02:30" = new TimeSpan(0,2,30)（Setting.cs:8）
    /// 开机启动
    pub RunWhenStarts: bool,               // true（Setting.cs:40）
    /// 登录系统重置时间
    pub ResetWhenSessionUnlock: bool,      // false（Setting.cs:44）
    /// 锁定超过时间就重置
    pub ResetTimeout: HmsSpan,             // "00:05:00" = new TimeSpan(0,5,0)（Setting.cs:9）
}

impl ConfigFile for SettingJson {
    const FILE_NAME: &'static str = "Setting.json";
}

impl Default for SettingJson {
    fn default() -> Self {
        Self {
            RestInterval: HmsSpan::from_hms(0, 45, 0),
            RestDuration: HmsSpan::from_hms(0, 2, 30),
            RunWhenStarts: true,
            ResetWhenSessionUnlock: false,
            ResetTimeout: HmsSpan::from_hms(0, 5, 0),
        }
    }
}

/// UI.json —— v2 UI.cs:11-13。注意 v2 的拼写错误 **CoutdownWindow** 必须原样保留！
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UiJson {
    pub CoutdownWindowLeft: StjF64, // 100.0（输出字节 "100"）
    pub CoutdownWindowTop: StjF64,  // 100.0
    pub TopMost: bool,              // true
}

impl ConfigFile for UiJson {
    const FILE_NAME: &'static str = "UI.json";
}

impl Default for UiJson {
    fn default() -> Self {
        Self {
            CoutdownWindowLeft: StjF64(100.0),
            CoutdownWindowTop: StjF64(100.0),
            TopMost: true,
        }
    }
}

/// Languages.json —— v2 Languages.cs:6
///（Default 可 derive：`Option<String>` 的 derive 默认值就是 `None`）
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LanguagesJson {
    pub CurrentLan: Option<String>, // null 时照写 "CurrentLan":null
}

impl ConfigFile for LanguagesJson {
    const FILE_NAME: &'static str = "Languages.json";
}

/// LockScreen.json —— v3.2 仿锁屏新增（无 v2 对应文件；快捷键默认 Alt+Win+L，
/// 见下方 Default 手工实现）。密码明文存本地：软锁屏定位（防顺便瞄屏），不是
/// 安全边界——真安全靠 Win+L 之外的系统锁。
/// （v3.2.1 删除 InterceptWinL / Enabled：接管能力并入 Hotkey——快捷键设为
/// Win+L 即替代；锁屏是自带能力、立即锁屏恒可用，总开关纯属多余（用户拍板
/// 「脱裤子放屁」）。旧配置里的这些键 serde 默认忽略未知字段，读入无害、落盘消失）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LockScreenJson {
    /// 截图穿透：锁屏遮罩不进入截图/录屏（WDA_EXCLUDEFROMCAPTURE），
    /// AI 等截屏工具继续看到真实桌面
    pub CaptureThrough: bool,
    /// 锁屏激活即进入休息（复用休息遮罩倒计时，计入休息统计）
    pub AutoRest: bool,
    /// 解锁密码（空 = 无需密码）
    pub Password: String,
    /// 自定义激活快捷键（`HotkeyCombo` 字符串格式，默认 "Alt+Win+L"；空 =
    /// 禁用快捷键，非法值运行时回退默认并记日志）。设为 Win+L = 接管系统
    /// 锁屏（HKCU DisableLockWorkstation 随之自动开启，防钩子漏键退化成
    /// 系统锁屏）；默认不用裸 Win+L：管控软件常在键盘过滤驱动层把它整个
    /// 吞掉（LL 钩子收不到，2026-10-09 实测），Ctrl+Win+L 本机亦被其他软件
    /// RegisterHotKey 占用（2026-10-10 实测），Win+Alt+L 在两者之外
    pub Hotkey: String,
}

impl Default for LockScreenJson {
    fn default() -> Self {
        Self {
            CaptureThrough: false,
            AutoRest: false,
            Password: String::new(),
            Hotkey: DEFAULT_LOCK_HOTKEY.to_string(),
        }
    }
}

impl ConfigFile for LockScreenJson {
    const FILE_NAME: &'static str = "LockScreen.json";
}

/// 仿锁屏默认激活快捷键（LockScreen.json Hotkey 的缺省值；显示序 Alt 在 Win 前）
pub const DEFAULT_LOCK_HOTKEY: &str = "Alt+Win+L";

/// 可移植修饰键位掩码（`HotkeyCombo::parse` 产物；到 Win32 `MOD_*` 的映射在
/// app 层钩子线程做，core 保持无窗口依赖）
pub const HK_CTRL: u8 = 1;
/// 见 [`HK_CTRL`]
pub const HK_ALT: u8 = 2;
/// 见 [`HK_CTRL`]
pub const HK_SHIFT: u8 = 4;
/// 见 [`HK_CTRL`]
pub const HK_WIN: u8 = 8;

/// 仿锁屏激活快捷键组合（修饰键 + 主键）。字符串格式 `Ctrl+Alt+Shift+Win+键`：
/// 修饰键大小写不敏感、任意顺序、不可重复（`Control`/`Windows` 别名可解析，
/// 规范输出用短名）；主键 = `A-Z` / `0-9` / `F1-F12`；至少一个修饰键；
/// 空串解析返回 `None`（调用方按「禁用快捷键」区分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyCombo {
    /// [`HK_CTRL`] 等位或组合（非零）
    pub mods: u8,
    /// Win32 虚拟键码（core 侧只做码表换算，不引用 windows crate）
    pub vk: u16,
}

impl HotkeyCombo {
    pub fn parse(s: &str) -> Option<Self> {
        let tokens: Vec<&str> = s.split('+').map(str::trim).collect();
        // 至少「修饰键+主键」两段；单主键（无修饰键）不收——裸键全局吞击太危险
        let (key_tok, mod_toks) = tokens.split_last()?;
        if mod_toks.is_empty() {
            return None;
        }
        let mut mods = 0u8;
        for t in mod_toks {
            let m = match t.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => HK_CTRL,
                "alt" => HK_ALT,
                "shift" => HK_SHIFT,
                "win" | "windows" => HK_WIN,
                _ => return None,
            };
            if mods & m != 0 {
                return None; // 重复修饰键
            }
            mods |= m;
        }
        Some(Self { mods, vk: parse_key(key_tok)? })
    }

    /// 规范写法：修饰键固定 Ctrl→Alt→Shift→Win 顺序 + 短名 + 大写主键
    ///（"win+ctrl+l" → "Ctrl+Win+L"），设置页回包/日志统一用它
    pub fn to_display(&self) -> String {
        let mut out = String::new();
        let mut push = |name: &str| {
            if !out.is_empty() {
                out.push('+');
            }
            out.push_str(name);
        };
        if self.mods & HK_CTRL != 0 {
            push("Ctrl");
        }
        if self.mods & HK_ALT != 0 {
            push("Alt");
        }
        if self.mods & HK_SHIFT != 0 {
            push("Shift");
        }
        if self.mods & HK_WIN != 0 {
            push("Win");
        }
        push(&vk_label(self.vk));
        out
    }
}

/// 主键 token → Win32 虚拟键码：字母/数字取 ASCII（与 VK 0x41+/0x30- 重合），
/// F1-F12 取 0x70 起。其余（OEM 键、方向键等）不收，保证显示与跨语言输入无歧义
fn parse_key(tok: &str) -> Option<u16> {
    let up = tok.to_ascii_uppercase();
    if up.len() == 1 {
        let c = up.as_bytes()[0];
        return match c {
            b'A'..=b'Z' | b'0'..=b'9' => Some(u16::from(c)),
            _ => None,
        };
    }
    up.strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=12).contains(n))
        .map(|n| u16::from(0x6F + n)) // F1=0x70 .. F12=0x7B
}

/// 虚拟键码 → 显示名（`parse_key` 的反变换；范围外值只可能来自手编配置，
/// 显示十六进制兜底）
fn vk_label(vk: u16) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(u32::from(vk)).unwrap().to_string(),
        0x70..=0x7B => format!("F{}", vk - 0x6F),
        _ => format!("VK{vk:02X}"),
    }
}

/// f64 的 System.Text.Json 字节兼容包装（整数值输出 "100" 而非 "100.0"）
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StjF64(pub f64);

impl serde::Serialize for StjF64 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        // System.Text.Json 对整数值 double 输出 "100"；serde_json 输出 "100.0"。
        // fract()==0 且绝对值 < 2^53（i64 精确表示范围）时按 i64 路径写出。
        if v.fract() == 0.0 && v.abs() < 9_007_199_254_740_992.0 {
            serializer.serialize_i64(v as i64)
        } else {
            serializer.serialize_f64(v)
        }
    }
}

impl<'de> serde::Deserialize<'de> for StjF64 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // serde_json 对 "100" 与 "100.0" 都能反序列化为 f64，读方向天然兼容
        Ok(StjF64(f64::deserialize(deserializer)?))
    }
}

/// 定位 %LOCALAPPDATA%（架构文档 §4.3 评审修订）：用 KnownFolder API
/// `SHGetKnownFolderPath(&FOLDERID_LocalAppData, ..)`（与 .NET
/// `Environment.GetFolderPath(LocalApplicationData)` 同为 KnownFolder API），
/// **不能**直接读 `LOCALAPPDATA` 环境变量（清单 F1）；KnownFolder 失败才回退环境变量。
fn local_app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Shell::{
            SHGetKnownFolderPath, FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG,
        };
        unsafe {
            // dwflags = 0，htoken = None（默认令牌）；0.62 返回 Result<PWSTR>
            if let Ok(path) =
                SHGetKnownFolderPath(&FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG(0), None)
            {
                let s = path.to_string().unwrap_or_default();
                windows::Win32::System::Com::CoTaskMemFree(Some(path.as_ptr().cast()));
                if !s.is_empty() {
                    return PathBuf::from(s);
                }
            }
        }
    }
    // 回退：环境变量（KnownFolder API 失败或非 Windows 平台/单测环境）
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每测试独立的临时目录（避免并行测试互踩）
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eyenurse-core-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_setting_json_bytes_match_v2() {
        // 架构文档 §4.3 注：保存默认配置后的期望字节
        assert_eq!(
            serde_json::to_string(&SettingJson::default()).unwrap(),
            "{\"RestInterval\":\"00:45:00\",\"RestDuration\":\"00:02:30\",\"RunWhenStarts\":true,\"ResetWhenSessionUnlock\":false,\"ResetTimeout\":\"00:05:00\"}"
        );
    }

    #[test]
    fn default_ui_json_bytes_integers_not_floats() {
        assert_eq!(
            serde_json::to_string(&UiJson::default()).unwrap(),
            "{\"CoutdownWindowLeft\":100,\"CoutdownWindowTop\":100,\"TopMost\":true}"
        );
        // StjF64：非整数值仍按 f64 输出
        assert_eq!(serde_json::to_string(&StjF64(100.5)).unwrap(), "100.5");
        assert_eq!(serde_json::to_string(&StjF64(123.0)).unwrap(), "123");
    }

    #[test]
    fn default_languages_json_writes_null() {
        assert_eq!(
            serde_json::to_string(&LanguagesJson::default()).unwrap(),
            "{\"CurrentLan\":null}"
        );
    }

    #[test]
    fn load_save_roundtrip_and_bad_json_fallback() {
        let dir = temp_dir("roundtrip");
        let store = ConfigStore::with_dir(dir.clone());

        // 缺文件 → 默认值
        assert_eq!(store.load::<SettingJson>(), SettingJson::default());

        // 保存 → 读回一致
        let s = SettingJson {
            RestInterval: HmsSpan::from_hms(0, 30, 0),
            ..SettingJson::default()
        };
        store.save(&s).unwrap();
        assert_eq!(store.load::<SettingJson>(), s);

        // 坏 JSON → 整个对象取默认值（v2 JsonDeserialize 吞异常）
        std::fs::write(dir.join(SettingJson::FILE_NAME), "{broken").unwrap();
        assert_eq!(store.load::<SettingJson>(), SettingJson::default());

        // 字节兼容：v2 生成的文件能原样读入
        std::fs::write(
            dir.join(UiJson::FILE_NAME),
            "{\"CoutdownWindowLeft\":120,\"CoutdownWindowTop\":56.5,\"TopMost\":false}",
        )
        .unwrap();
        let ui = store.load::<UiJson>();
        assert_eq!(ui.CoutdownWindowLeft.0, 120.0);
        assert_eq!(ui.CoutdownWindowTop.0, 56.5);
        assert!(!ui.TopMost);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn f6_disk_value_not_clamped_and_saves_back_identical() {
        // 清单 F6/F12：读盘路径从不 clamp——手改 ≥1 天的值原样生效、保存字节往返一致
        let dir = temp_dir("f6");
        let raw = "{\"RestInterval\":\"1.02:03:04\",\"RestDuration\":\"00:02:30\",\
                   \"RunWhenStarts\":true,\"ResetWhenSessionUnlock\":false,\
                   \"ResetTimeout\":\"00:05:00\"}";
        std::fs::write(dir.join(SettingJson::FILE_NAME), raw).unwrap();
        let store = ConfigStore::with_dir(dir.clone());

        let s = store.load::<SettingJson>();
        assert_eq!(s.RestInterval.to_c_string(), "1.02:03:04"); // 不变形
        store.save(&s).unwrap();
        let saved = std::fs::read_to_string(dir.join(SettingJson::FILE_NAME)).unwrap();
        assert_eq!(saved, raw); // 保存往返一致

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn bom_prefixed_file_loads_like_v2() {
        // v2 File.ReadAllText 自动剥 BOM；带 BOM 的手编文件在 v2 下正常加载
        let dir = temp_dir("bom");
        std::fs::write(
            dir.join(UiJson::FILE_NAME),
            "\u{feff}{\"CoutdownWindowLeft\":120,\"CoutdownWindowTop\":100,\"TopMost\":false}",
        )
        .unwrap();
        let store = ConfigStore::with_dir(dir.clone());
        let ui = store.load::<UiJson>();
        assert_eq!(ui.CoutdownWindowLeft.0, 120.0);
        assert!(!ui.TopMost);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn partial_json_missing_fields_use_defaults_like_stj() {
        // v2 System.Text.Json 默认选项：缺成员不抛异常，属性留构造默认值
        //（不是整体 default(T)）；#[serde(default)] 同语义
        let dir = temp_dir("partial");
        std::fs::write(
            dir.join(SettingJson::FILE_NAME),
            "{\"RestInterval\":\"01:00:00\"}",
        )
        .unwrap();
        let store = ConfigStore::with_dir(dir.clone());
        let s = store.load::<SettingJson>();
        assert_eq!(s.RestInterval, HmsSpan::from_hms(1, 0, 0));
        assert_eq!(s.RestDuration, HmsSpan::from_hms(0, 2, 30)); // 默认
        assert!(s.RunWhenStarts); // 默认
        assert!(!s.ResetWhenSessionUnlock); // 默认
        assert_eq!(s.ResetTimeout, HmsSpan::from_hms(0, 5, 0)); // 默认
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lock_screen_json_roundtrip() {
        // v3.2 仿锁屏：缺文件 → 全关默认；保存 → 读回一致（无 v2 字节兼容负担）
        let dir = temp_dir("lock");
        let store = ConfigStore::with_dir(dir.clone());
        assert_eq!(store.load::<LockScreenJson>(), LockScreenJson::default());
        // 缺文件默认值：快捷键缺省 Alt+Win+L（裸 Win+L 被驱动层吞、Ctrl+Win+L
        // 本机被占的规避，见 Hotkey 字段注释）；默认值必须自身可解析且为规范写法
        // （防手改常量引入不可激活的组合）
        assert_eq!(LockScreenJson::default().Hotkey, DEFAULT_LOCK_HOTKEY);
        assert_eq!(
            HotkeyCombo::parse(DEFAULT_LOCK_HOTKEY).map(|c| c.to_display()),
            Some(DEFAULT_LOCK_HOTKEY.to_string())
        );

        let cfg = LockScreenJson {
            CaptureThrough: true,
            AutoRest: true,
            Password: "1234".into(),
            Hotkey: "Ctrl+Alt+F8".into(),
        };
        store.save(&cfg).unwrap();
        assert_eq!(store.load::<LockScreenJson>(), cfg);
        // 缺字段 → 该字段默认值（serde(default) 容器语义）：旧版本配置文件没有
        // Hotkey 键，读入即得默认快捷键
        std::fs::write(dir.join(LockScreenJson::FILE_NAME), "{}").unwrap();
        let loaded = store.load::<LockScreenJson>();
        assert_eq!(loaded.Password, "");
        assert_eq!(loaded.Hotkey, DEFAULT_LOCK_HOTKEY);
        // v3.2.1 删除 Enabled/InterceptWinL 后，旧配置残留的这些键按未知字段
        // 忽略（读入无害、落盘消失）
        std::fs::write(
            dir.join(LockScreenJson::FILE_NAME),
            "{\"Enabled\":true,\"InterceptWinL\":true}",
        )
        .unwrap();
        let legacy = store.load::<LockScreenJson>();
        assert_eq!(legacy, LockScreenJson::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hotkey_combo_parse_and_display() {
        // 大小写不敏感 + 修饰键任意顺序 → 规范顺序输出
        assert_eq!(
            HotkeyCombo::parse("win+ctrl+l").map(|c| c.to_display()),
            Some("Ctrl+Win+L".into())
        );
        assert_eq!(
            HotkeyCombo::parse("Ctrl+Win+L").map(|c| c.to_display()),
            Some("Ctrl+Win+L".into())
        );
        // 别名
        assert_eq!(
            HotkeyCombo::parse("Windows + Control + L").map(|c| c.to_display()),
            Some("Ctrl+Win+L".into())
        );
        // 主键码表：字母 / 数字 / F 键
        assert_eq!(
            HotkeyCombo::parse("Ctrl+Alt+F12").map(|c| c.to_display()),
            Some("Ctrl+Alt+F12".into())
        );
        assert_eq!(
            HotkeyCombo::parse("Shift+F1").map(|c| c.to_display()),
            Some("Shift+F1".into())
        );
        assert_eq!(
            HotkeyCombo::parse("Alt+9").map(|c| (c.vk, c.mods)),
            Some((0x39, HK_ALT))
        );
    }

    #[test]
    fn hotkey_combo_parse_rejects_garbage() {
        // 空串 = 禁用（调用方语义），不是合法组合
        assert_eq!(HotkeyCombo::parse(""), None);
        assert_eq!(HotkeyCombo::parse("   "), None);
        // 无修饰键 / 修饰键作主键 / 未知段 / 重复修饰键 / 超范围 F 键
        assert_eq!(HotkeyCombo::parse("L"), None);
        assert_eq!(HotkeyCombo::parse("Ctrl+Win"), None);
        assert_eq!(HotkeyCombo::parse("Ctrl+Hyper+L"), None);
        assert_eq!(HotkeyCombo::parse("Ctrl+Ctrl+L"), None);
        assert_eq!(HotkeyCombo::parse("Ctrl+F13"), None);
        assert_eq!(HotkeyCombo::parse("Ctrl+;"), None);
        assert_eq!(HotkeyCombo::parse("Ctrl++L"), None);
    }

    #[test]
    fn languages_json_roundtrip() {
        // 非 null 值读回 + 默认值照写 null（F5）
        let dir = temp_dir("lang");
        std::fs::write(dir.join(LanguagesJson::FILE_NAME), "{\"CurrentLan\":\"zh\"}").unwrap();
        let store = ConfigStore::with_dir(dir.clone());
        assert_eq!(store.load::<LanguagesJson>().CurrentLan.as_deref(), Some("zh"));
        store.save(&LanguagesJson::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(LanguagesJson::FILE_NAME)).unwrap(),
            "{\"CurrentLan\":null}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
