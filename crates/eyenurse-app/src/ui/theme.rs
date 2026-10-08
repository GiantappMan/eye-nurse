//! 颜色/样式（实现者 C 所有）：v2 颜色常量表，值取自 v2 XAML。
//! 骨架期仅 bar/mask 用到 MASK_*；其余常量供 C/D 填充时消费。
#![allow(dead_code)]

use egui::Color32;

// 来源：
// - LockScreenWindow.xaml：Background="#141418"、Foreground="#EEEEF2"；
// - CountdownWindow.xaml：进度条底 "Beige"、两个 TimeSpanToColorBrushConveter
//   （DurationSecond=480，8 分钟走完 From→To）：
//   progreeBGConverter From="#ECC071" To="#3E0D0D"（进度前景），
//   timeBGConverter From="#080808" To="#F40000"（时间文字，暂停态）。
// D 的设置窗配色一律取本文件常量，不自定义第二套色板（架构文档 §1 契约）。

/// 遮罩背景（LockScreenWindow Background）
pub const MASK_BG: Color32 = Color32::from_rgb(0x14, 0x14, 0x18);
/// 遮罩文字（LockScreenWindow Foreground）
pub const MASK_FG: Color32 = Color32::from_rgb(0xEE, 0xEE, 0xF2);

/// 悬浮条进度条底色（ProgressBar Background="Beige"）
pub const BAR_PROGRESS_BG: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xDC);
/// 悬浮条倒计时文字色（v2 txt 无显式 Foreground，取默认黑）
pub const BAR_TEXT_IDLE: Color32 = Color32::BLACK;
/// 悬浮条进度前景（非暂停态）。v2 未设 Foreground，走 WPF Aero2 主题默认的绿色
/// 渐变指示条；Aero2 画刷字节在主题库内、v2 XAML 未携带，无法逐字节复刻，
/// 取常见 Aero2 绿 #3AC32E 作平替（视觉近似，记录为已知偏差）。
pub const BAR_PROGRESS_FILL: Color32 = Color32::from_rgb(0x3A, 0xC3, 0x2E);
/// 悬浮条置顶复选框的字形/边框色（CheckBox.xaml GlyphColor="#FF444444"）
pub const BAR_CHECK_GLYPH: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
/// 悬浮条置顶复选框悬停底色（CheckBox.xaml 模板 MouseOver → Background #FFFFFF）
pub const BAR_CHECK_HOVER_BG: Color32 = Color32::WHITE;
/// 进度前景渐变起点（progreeBGConverter From，暂停时长 0 时）
pub const BAR_PROGRESS_FROM: Color32 = Color32::from_rgb(0xEC, 0xC0, 0x71);
/// 进度前景渐变终点（progreeBGConverter To，暂停 480s 时）
pub const BAR_PROGRESS_TO: Color32 = Color32::from_rgb(0x3E, 0x0D, 0x0D);
/// 时间文字渐变起点（timeBGConverter From，暂停时长 0 时）
pub const BAR_TIME_FROM: Color32 = Color32::from_rgb(0x08, 0x08, 0x08);
/// 时间文字渐变终点（timeBGConverter To，暂停 480s 时）
pub const BAR_TIME_TO: Color32 = Color32::from_rgb(0xF4, 0x00, 0x00);

/// 变色阈值：XAML DurationSecond="480"（8 分钟），即暂停展示从 From 渐变到 To 的时长
pub const COLOR_RAMP_SECONDS: f64 = 480.0;

/// 线性插值（t ∈ [0,1]）
pub fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

/// 按秒数取渐变色，语义逐条对齐 v2 TimeSpanToColorBrushConveter.cs:20-31：
/// `percent = seconds / 480`；**`percent == 0 → 0.1`**（v2 converter 特判，暂停一开局
/// 就有 10% 渐变）；`percent > 1 → 1`（走满封顶）。
pub fn ramp(from: Color32, to: Color32, seconds: f64) -> Color32 {
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

    const A: Color32 = Color32::from_rgb(0x00, 0x80, 0xFF);
    const B: Color32 = Color32::from_rgb(0xFF, 0xFF, 0x00);

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
        assert_eq!(c.r(), 128); // round(0 + 255*0.5) = 127.5 → 128
        assert_eq!(c.g(), 192); // round(128 + 127*0.5) = 191.5 → 192
        assert_eq!(c.b(), 128); // round(255 - 255*0.5) = 127.5 → 128
    }
}
