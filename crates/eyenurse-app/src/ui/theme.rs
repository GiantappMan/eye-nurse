//! 颜色/样式（实现者 C 所有）：v2 颜色常量表，**真源 = 同目录 theme.json**（§3
//! 评审修订）。build.rs 读取并严格校验 theme.json（#RRGGBB + 必备键，失败即 panic
//! =编译期红），生成 RGB 常量到 OUT_DIR，由本文件 `include!`——常量名与 egui 版
//! theme.rs 一致，类型从 `egui::Color32` 改为 [`Rgb`] 三元组（GDI 消费）。
//! `ramp()`/480s 渐变公式原样保留供 bar.rs 用。
//!
//! 页面侧（D 的 ui/src）直接 `import theme from theme.json` 注入 CSS 变量；
//! 唯一允许的改色入口 = 改 theme.json，两态自动一致。
#![allow(dead_code)]

// 来源（theme.json 注释同）：
// - LockScreenWindow.xaml：Background="#141418"、Foreground="#EEEEF2"；
// - CountdownWindow.xaml：进度条底 "Beige"、两个 TimeSpanToColorBrushConveter
//   （DurationSecond=480，8 分钟走完 From→To）：
//   progreeBGConverter From="#ECC071" To="#3E0D0D"（进度前景），
//   timeBGConverter From="#080808" To="#F40000"（时间文字，暂停态）。
// - DS3 校准（docs/design-system-v3.md §5.6/§6）：暂停末态文字 #F40000→#FF5252
//   （对 #3E0D0D 填充 3.82→5.19:1，过 WCAG AA）；idle #000000→#1A1A1A 软黑
//   （18.98→15.73:1，仍远超 AA）；暂停文字渐变键名弃用旧「time」前缀，统一为
//   bar_text_from/to（与 SkinColors 字段/皮肤中心编辑器键同名）。

/// RGB 三元组（egui::Color32 的 GDI 等价物；值恒来自 theme.json）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// GDI COLORREF（0x00BBGGRR）
    pub const fn as_colorref(self) -> u32 {
        (self.b as u32) << 16 | (self.g as u32) << 8 | self.r as u32
    }
}

// —— build.rs 由 theme.json 生成（键缺失/格式错 = 编译失败）——
include!(concat!(env!("OUT_DIR"), "/eyenurse_theme_consts.rs"));

/// 变色阈值：XAML DurationSecond="480"（8 分钟），即暂停展示从 From 渐变到 To 的时长
pub const COLOR_RAMP_SECONDS: f64 = 480.0;

/// 线性插值（t ∈ [0,1]）
pub fn lerp_color(from: Rgb, to: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Rgb::new(
        mix(from.r, to.r),
        mix(from.g, to.g),
        mix(from.b, to.b),
    )
}

/// 按秒数取渐变色，语义逐条对齐 v2 TimeSpanToColorBrushConveter.cs:20-31：
/// `percent = seconds / 480`；**`percent == 0 → 0.1`**（v2 converter 特判，暂停一开局
/// 就有 10% 渐变）；`percent > 1 → 1`（走满封顶）。
pub fn ramp(from: Rgb, to: Rgb, seconds: f64) -> Rgb {
    let mut percent = seconds / COLOR_RAMP_SECONDS;
    if percent == 0.0 {
        percent = 0.1;
    }
    if percent > 1.0 {
        percent = 1.0;
    }
    lerp_color(from, to, percent as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Rgb = Rgb::new(0x00, 0x80, 0xFF);
    const B: Rgb = Rgb::new(0xFF, 0xFF, 0x00);

    #[test]
    fn theme_consts_match_json_values() {
        // 代码生成与 theme.json 的锚点值一致（改 json 必须过本测试；
        // BAR_TEXT_* 锚定 DS3 校准后的暂停文字渐变，见文件头注释）
        assert_eq!(MASK_BG, Rgb::new(0x14, 0x14, 0x18));
        assert_eq!(MASK_FG, Rgb::new(0xEE, 0xEE, 0xF2));
        assert_eq!(BAR_PROGRESS_BG, Rgb::new(0xF5, 0xF5, 0xDC));
        assert_eq!(BAR_PROGRESS_FILL, Rgb::new(0x3A, 0xC3, 0x2E));
        assert_eq!(BAR_PROGRESS_FROM, Rgb::new(0xEC, 0xC0, 0x71));
        assert_eq!(BAR_TEXT_IDLE, Rgb::new(0x1A, 0x1A, 0x1A));
        assert_eq!(BAR_TEXT_FROM, Rgb::new(0x08, 0x08, 0x08));
        assert_eq!(BAR_TEXT_TO, Rgb::new(0xFF, 0x52, 0x52));
        assert_eq!(MASK_ERROR, Rgb::new(0xFF, 0x8B, 0x8B));
    }

    #[test]
    fn colorref_is_bbggrr() {
        assert_eq!(Rgb::new(0x12, 0x34, 0x56).as_colorref(), 0x563412);
    }

    #[test]
    fn ramp_zero_seconds_takes_ten_percent_floor() {
        // v2 converter：percent==0 → 0.1（TimeSpanToColorBrushConveter.cs:23-25），
        // 暂停一开局颜色即离开起点 10%
        let c = ramp(A, B, 0.0);
        assert_eq!(c, lerp_color(A, B, 0.1));
        assert_ne!(c, A);
    }

    #[test]
    fn ramp_full_and_overfull_clamp_to_to() {
        assert_eq!(ramp(A, B, 480.0), B);
        assert_eq!(ramp(A, B, 960.0), B); // >1 封顶
    }

    #[test]
    fn ramp_midpoint_is_linear() {
        let c = ramp(A, B, 240.0); // 恰好 50%（f32 下 .5 半值远离零取整）
        assert_eq!(c.r, 128); // round(0 + 255*0.5) = 127.5 → 128
        assert_eq!(c.g, 192); // round(128 + 127*0.5) = 191.5 → 192
        assert_eq!(c.b, 128); // round(255 - 255*0.5) = 127.5 → 128
    }
}
