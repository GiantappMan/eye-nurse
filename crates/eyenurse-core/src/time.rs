//! .NET TimeSpan 兼容的时间段类型（架构文档 §4.1）。
//!
//! v2 磁盘上的 TimeSpan 是 System.Text.Json 的**字符串**（"c" 常量格式），
//! 已实测 .NET 10 `JsonSerializer.Serialize(new TimeSpan(0,45,0))` → `"00:45:00"`。
//! .NET "c" 文法：`[-]d.hh:mm:ss.fffffff`，天/小数部分为 0 时省略。

use std::time::Duration;

const SECS_PER_DAY: u64 = 86_400;

/// .NET TimeSpan 的 Rust 等价物，serde 序列化为 "c" 格式字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct HmsSpan(Duration);

impl HmsSpan {
    /// 由时/分/秒构造（可为任意 i64，负值钳到 0——u64 无法表示负 TimeSpan）。
    pub fn from_hms(h: i64, m: i64, s: i64) -> Self {
        let total = h * 3600 + m * 60 + s;
        Self(Duration::from_secs(total.max(0) as u64))
    }

    pub fn from_duration(d: Duration) -> Self {
        Self(d)
    }

    /// 解析 .NET "c" 格式（`[-]d.hh:mm:ss[.fffffff]`）；
    /// 兼容 v2 UI 输入的短格式 `[-]hh:mm[:ss]`（.NET TimeSpan.TryParse 的 g 格式子集）。
    /// 解析失败返回 None（调用方回退默认值，对齐 v2 `Setting.GetTimeSpan`）。
    ///
    /// 说明：
    /// - 与 .NET 一样接受小时 ≥ 24 并归一化进天；
    /// - 分钟/秒 ≥ 60、小数秒超过 7 位 → None（.NET TryParse 判为非法 → 回退默认值）；
    /// - **负号丢弃、幅值保留**（如 "-00:10:00" → 10 分钟）：v2 的 `GetTimeSpan`
    ///   对解析结果按分量取 `Math.Abs`（Setting.cs:29），设置窗保存路径
    ///   `parse + clamp_like_v2` 后 "-00:10:00" → "00:10:00"（清单 F6）。
    ///   内部表示是 `Duration`（无符号，§4.1 冻结签名），故负号只能在 parse 处丢。
    pub fn parse(s: &str) -> Option<Self> {
        let t = s.trim();
        if t.is_empty() {
            return None;
        }
        // 负号识别后即丢弃（幅值保留，见方法注释），故绑定名为 _negative
        let (_negative, t) = match t.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, t),
        };
        if t.is_empty() {
            return None;
        }

        // 按 ':' 分段：2 段 = hh:mm（g 短格式）；3 段 = [d.]hh:mm:ss[.fffffff]
        let parts: Vec<&str> = t.split(':').collect();
        let (days, hours, mins, secs, nanos) = match parts.len() {
            2 => {
                let h = parse_u64_part(parts[0])?;
                let m = parse_u64_part(parts[1])?;
                if m > 59 {
                    // .NET TryParse：分钟分量越界 → 非法
                    return None;
                }
                (0u64, h, m, 0u64, 0u32)
            }
            3 => {
                // 首段可带天前缀 "d.hh"
                let (d, h) = match parts[0].split_once('.') {
                    Some((d, h)) => (parse_u64_part(d)?, parse_u64_part(h)?),
                    None => (0u64, parse_u64_part(parts[0])?),
                };
                let m = parse_u64_part(parts[1])?;
                // 末段可带小数秒 "ss.fffffff"
                let (sec, frac) = match parts[2].split_once('.') {
                    Some((sec, frac)) => (parse_u64_part(sec)?, parse_fraction_nanos(frac)?),
                    None => (parse_u64_part(parts[2])?, 0u32),
                };
                if m > 59 || sec > 59 {
                    // .NET TryParse：分钟/秒分量越界 → 非法
                    return None;
                }
                (d, h, m, sec, frac)
            }
            _ => return None,
        };

        let total_secs = days
            .checked_mul(SECS_PER_DAY)?
            .checked_add(hours.checked_mul(3600)?)
            .and_then(|v| v.checked_add(mins.checked_mul(60)?))
            .and_then(|v| v.checked_add(secs))?;
        // 负号丢弃、幅值保留（见方法注释；clamp_like_v2 在保存路径补足 v2 的
        // Math.Abs 分量语义）。小数秒 > 7 位已在 parse_fraction_nanos 拒绝。
        Some(Self(Duration::new(total_secs, nanos)))
    }

    /// 输出 .NET "c" 格式：负数在此不可能出现（parse 即丢符号）。
    /// 文法 `[-]d.hh:mm:ss.fffffff`：天为 0 省略 "d."；小数非 0 → 补齐 7 位。
    /// 小数部分按 .NET 100ns（tick）精度截断到 7 位（Duration 是 9 位纳秒）。
    pub fn to_c_string(&self) -> String {
        let total = self.0.as_secs();
        let nanos = self.0.subsec_nanos();
        let days = total / SECS_PER_DAY;
        let rem = total % SECS_PER_DAY;
        let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
        // 1 tick = 100ns，最多 9_999_999（7 位）
        let frac = if nanos > 0 {
            format!(".{:07}", nanos / 100)
        } else {
            String::new()
        };
        if days > 0 {
            format!("{}.{:02}:{:02}:{:02}{}", days, h, m, s, frac)
        } else {
            format!("{:02}:{:02}:{:02}{}", h, m, s, frac)
        }
    }

    pub fn as_duration(self) -> Duration {
        self.0
    }

    /// 复刻 v2 `Setting.GetTimeSpan` 的 clamp（Setting.cs:28-29）：
    /// 判据是 **TotalDays**：`TotalDays >= 1 || TotalDays <= 0` 时取
    /// `(|Hours|, |Minutes|, |Seconds|)`（丢掉天与负号）。Duration 无符号（parse 已丢
    /// 负号），故等价于 `>= 1 天 || == 0`。纯亚秒值（如 0.5s，TotalDays ≈ 5.8e-6）
    /// 落在 (0,1) 区间 → **原样保留**（v2 同输入同输出，不变形）。
    /// 注意取的是 .NET TimeSpan 的**分量**（Hours = 总小时 % 24），不是 TotalHours；
    /// `new TimeSpan(h, m, s)` 无亚秒 → ≥1 天值的亚秒同样丢弃（from_hms 同）。
    ///
    /// clamp 时机（架构文档 §4.3 评审修订）：只在设置窗“文本 → 数值”保存路径调用；
    /// `ConfigStore::load` 读盘路径**从不** clamp。
    pub fn clamp_like_v2(self) -> Self {
        if self.0 >= Duration::from_secs(SECS_PER_DAY) || self.0.is_zero() {
            let total = self.0.as_secs();
            let h = (total / 3600 % 24) as i64;
            let m = (total / 60 % 60) as i64;
            let s = (total % 60) as i64;
            Self::from_hms(h, m, s)
        } else {
            self
        }
    }
}

fn parse_u64_part(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<u64>().ok()
}

/// 小数秒（恰好 ≤ 7 位十进制小数）→ 纳秒；超过 7 位 → None。
/// .NET "c"/"g" 格式的小数秒最多 7 位（100ns tick），TryParse 遇到第 8 位判非法。
/// ".5" = 0.5s = 5×10⁻¹s：右侧补零到 7 位得 100ns tick 数，再 ×100 转纳秒。
fn parse_fraction_nanos(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 7 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut digits: String = s.to_string();
    while digits.len() < 7 {
        digits.push('0');
    }
    let ticks: u32 = digits.parse().ok()?;
    ticks.checked_mul(100) // 1 tick = 100ns；最大 9_999_999×100 < u32::MAX
}

impl serde::Serialize for HmsSpan {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // 写出 .NET "c" 格式（System.Text.Json 默认序列化格式）
        serializer.serialize_str(&self.to_c_string())
    }
}

impl<'de> serde::Deserialize<'de> for HmsSpan {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // 解析失败 → Err（由容器 #[serde(default)] / load() 的整体兜底成默认值）
        let s = String::deserialize(deserializer)?;
        HmsSpan::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("无效的 TimeSpan 字符串: {s:?}")))
    }
}

impl std::fmt::Display for HmsSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_c_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_c_and_g_formats() {
        assert_eq!(HmsSpan::parse("00:45:00"), Some(HmsSpan::from_hms(0, 45, 0)));
        assert_eq!(HmsSpan::parse("00:02:30"), Some(HmsSpan::from_hms(0, 2, 30)));
        // g 短格式：h:mm 与 h:mm:ss（首段是**小时**，≥24 归一化进天，同 .NET）
        assert_eq!(HmsSpan::parse("45:00"), Some(HmsSpan::from_hms(45, 0, 0)));
        assert_eq!(HmsSpan::parse("1:30"), Some(HmsSpan::from_hms(1, 30, 0)));
        // 带天与小数秒
        assert_eq!(
            HmsSpan::parse("1.02:03:04"),
            Some(HmsSpan::from_duration(Duration::new(93784, 0)))
        );
        assert_eq!(
            HmsSpan::parse("00:00:00.5"),
            Some(HmsSpan::from_duration(Duration::from_millis(500)))
        );
        // 小时 ≥ 24 归一化（.NET TimeSpan.Parse 同样接受；25h = 1 天 1 小时）
        assert_eq!(
            HmsSpan::parse("25:00:00"),
            Some(HmsSpan::from_hms(25, 0, 0))
        );
        assert_eq!(
            HmsSpan::from_hms(25, 0, 0).to_c_string(),
            "1.01:00:00"
        );
        // 非法输入 → None
        assert_eq!(HmsSpan::parse(""), None);
        assert_eq!(HmsSpan::parse("abc"), None);
        assert_eq!(HmsSpan::parse("1:2:3:4"), None);
        assert_eq!(HmsSpan::parse("45"), None);
        // .NET TryParse：分钟/秒分量越界、小数秒超 7 位 → 非法
        assert_eq!(HmsSpan::parse("1:75"), None);
        assert_eq!(HmsSpan::parse("00:75:00"), None);
        assert_eq!(HmsSpan::parse("00:00:60"), None);
        assert_eq!(HmsSpan::parse("00:00:00.12345678"), None);
        // 负值：丢符号、保留幅值（v2 GetTimeSpan 的 Math.Abs 分量语义，清单 F6）
        assert_eq!(HmsSpan::parse("-00:10:00"), Some(HmsSpan::from_hms(0, 10, 0)));
        assert_eq!(HmsSpan::parse("-1.02:03:04"), Some(HmsSpan::from_hms(26, 3, 4)));
    }

    #[test]
    fn f6_save_path_parse_then_clamp() {
        // 清单 F6：设置窗“文本 → 数值”保存路径 = parse + clamp_like_v2
        // "1.02:03:04" → 丢天取分量 → 02:03:04
        let day = HmsSpan::parse("1.02:03:04").unwrap();
        assert_eq!(day.to_c_string(), "1.02:03:04"); // parse 本身不 clamp
        assert_eq!(day.clamp_like_v2().to_c_string(), "02:03:04");
        // "-00:10:00" → 丢符号（幅值保留）→ clamp 不再变形 → 00:10:00
        let neg = HmsSpan::parse("-00:10:00").unwrap();
        assert_eq!(neg.clamp_like_v2().to_c_string(), "00:10:00");
        // "-1.02:03:04" → 同 v2：TotalDays≤0 → (|Hours|,|Minutes|,|Seconds|) = 02:03:04
        let neg_day = HmsSpan::parse("-1.02:03:04").unwrap();
        assert_eq!(neg_day.clamp_like_v2().to_c_string(), "02:03:04");
        // "abc" → None → 调用方回退默认值（v2 GetTimeSpan 的 defaultTs）
        assert_eq!(HmsSpan::parse("abc"), None);
        // 正常区间 parse+clamp 前后不变（保存可往返）
        let normal = HmsSpan::parse("00:45:00").unwrap();
        assert_eq!(normal.clamp_like_v2().to_c_string(), "00:45:00");
    }

    #[test]
    fn to_c_string_rules() {
        assert_eq!(HmsSpan::from_hms(0, 45, 0).to_c_string(), "00:45:00");
        assert_eq!(HmsSpan::from_hms(0, 2, 30).to_c_string(), "00:02:30");
        // 天 > 0 → d.hh:mm:ss
        assert_eq!(
            HmsSpan::from_duration(Duration::new(93784, 0)).to_c_string(),
            "1.02:03:04"
        );
        // 小数非 0 → 补齐 7 位
        assert_eq!(
            HmsSpan::from_duration(Duration::from_millis(500)).to_c_string(),
            "00:00:00.5000000"
        );
        // 天与小数并存（.NET c 文法两个独立省略规则）
        assert_eq!(
            HmsSpan::from_duration(Duration::new(86_400, 500_000_000)).to_c_string(),
            "1.00:00:00.5000000"
        );
        // parse → to_c_string 往返一致
        for s in ["00:45:00", "1.02:03:04", "00:00:00.1234000"] {
            assert_eq!(HmsSpan::parse(s).unwrap().to_c_string(), s);
        }
    }

    #[test]
    fn clamp_like_v2_matches_setting_cs() {
        // ≥ 1 天：丢天取分量（1.05:30:00 → Hours=5,Minutes=30 → 05:30:00）
        let day_plus = HmsSpan::from_duration(Duration::new(SECS_PER_DAY + 5 * 3600 + 30 * 60, 0));
        assert_eq!(day_plus.clamp_like_v2(), HmsSpan::from_hms(5, 30, 0));
        // 0 → 00:00:00（TotalDays <= 0 分支）
        assert_eq!(HmsSpan::from_hms(0, 0, 0).clamp_like_v2(), HmsSpan::from_hms(0, 0, 0));
        // 纯亚秒值：TotalDays ∈ (0,1) → 不变形（v2 同输入输出 "00:00:00.5000000"）
        assert_eq!(
            HmsSpan::parse("00:00:00.5").unwrap().clamp_like_v2().to_c_string(),
            "00:00:00.5000000"
        );
        // 亚秒且 ≥1 天：取分量、丢亚秒（v2 new TimeSpan(h,m,s) 同）
        assert_eq!(
            HmsSpan::parse("1.02:03:04.9").unwrap().clamp_like_v2().to_c_string(),
            "02:03:04"
        );
        // 正常区间不动
        let normal = HmsSpan::from_hms(0, 45, 0);
        assert_eq!(normal.clamp_like_v2(), normal);
    }

    #[test]
    fn serde_bytes_match_system_text_json() {
        // System.Text.Json：TimeSpan → "c" 字符串；紧凑、PascalCase
        assert_eq!(
            serde_json::to_string(&HmsSpan::from_hms(0, 45, 0)).unwrap(),
            "\"00:45:00\""
        );
        let v: HmsSpan = serde_json::from_str("\"00:45:00\"").unwrap();
        assert_eq!(v, HmsSpan::from_hms(0, 45, 0));
        // 坏字符串 → Err
        assert!(serde_json::from_str::<HmsSpan>("\"xx\"").is_err());
    }
}
