//! 悬浮倒计时条（实现者 C 所有）——root 视口内容（NativeOptions.viewport，架构文档 §5.1）。
//!
//! v2 对照（CountdownWindow.xaml + .xaml.cs）：
//! - 80×20 无边框小条，进度条底 Beige、满→空（TimeSpanToPercent.cs:21）；
//! - 居中 `mm:ss`（mm 为分钟分量，>59min 回绕，清单 B2/Q13）；
//! - 休息中文字替换为 `resting` 文案（B10）——v2 里它是独立 TextBlock（无
//!   Foreground setter）恒默认黑，暂停渐变不落在它上（一致性差异 #6）；
//! - 暂停中：文字切 PausedTime、左侧时钟图标、文字/进度按 480s 渐变（B9/Q8）；
//! - 右键弹出与托盘相同的菜单（一致性差异 #4：v2 ContextMenu = App.Menu）；
//! - 悬停浮现置顶复选（18×18，右侧；0.3s 淡入、移出 0.7s 后 0.3s 淡出，B4），
//!   tooltip = `always_on_top`；点击即写 UI.json（B5）；
//! - 可拖动；拖动结束把逻辑坐标写回 UiJson 并保存（v2 的保存分支是死代码，
//!   清单 Q9 决议：v3 修复为拖动结束即存，B6）；
//! - WS_EX_TOOLWINDOW 自愈补丁（Alt-Tab 隐藏，§7.2/B7）：接线在 `AppModel::logic`
//!   每帧调用 `win::heal_toolwindows()`（app.rs），本文件不重复调用。

use crate::app::AppModel;
use crate::ui::theme;
use std::time::{Duration, Instant};

/// root 视口每帧内容（App::ui 委托到这里）
pub fn bar_ui(ui: &mut egui::Ui, m: &mut AppModel) {
    let bar = m
        .shared()
        .bar
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let texts = m.texts();
    let paused_time = bar.paused_time;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BAR_PROGRESS_BG))
        .show(ui, |ui| {
            let (rect, bar_resp) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            let bar_resp = bar_resp.on_hover_cursor(egui::CursorIcon::Move);
            let painter = ui.painter_at(rect);

            // —— 手写拖动（§6.1 评审修订：StartDrag 有 has_focus 前提会静默失效，
            //      故按 pointer 增量每帧发 OuterPosition，无焦点依赖、行为确定）——
            if bar_resp.dragged() {
                let delta = bar_resp.drag_delta();
                if delta != egui::Vec2::ZERO {
                    let cur = ui.ctx().input(|i| i.viewport().outer_rect).map(|r| r.min);
                    if let Some(p) = cur {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::OuterPosition(p + delta));
                    }
                }
            }
            if bar_resp.drag_stopped() {
                // Q9 决议：拖动结束帧保存逻辑坐标（v2 保存分支死代码，v3 修复）
                if let Some(r) = ui.ctx().input(|i| i.viewport().outer_rect) {
                    m.set_bar_position(r.min.x as f64, r.min.y as f64);
                }
            }

            // —— 进度前景：remaining/interval，满→空（B3；v2 ProgressBar.Value = 剩余）——
            let percent = if bar.main_interval.is_zero() {
                100.0
            } else {
                bar.main_countdown.as_secs_f64() / bar.main_interval.as_secs_f64() * 100.0
            }
            .clamp(0.0, 100.0);
            let fill_color = match paused_time {
                // 暂停：进度前景 #ECC071→#3E0D0D 按 PausedTime 渐变（480s 走满，Q8）
                Some(paused) => theme::ramp(
                    theme::BAR_PROGRESS_FROM,
                    theme::BAR_PROGRESS_TO,
                    paused.as_secs_f64(),
                ),
                None => theme::BAR_PROGRESS_FILL,
            };
            painter.rect_filled(
                egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(rect.width() * (percent / 100.0) as f32, rect.height()),
                ),
                0.0,
                fill_color,
            );

            // —— 文字色：暂停时 #080808→#F40000 按 PausedTime 渐变（Q8）。
            // 一致性差异 #6：休息文案对应 v2 独立的 txtResting（无 Foreground setter，
            // 恒默认黑）；渐变只落在隐藏的 txt 与进度条（及经 txt.Foreground 绑定
            // 的时钟图标）上，休息文案不吃渐变 ——
            let paused_ramp = paused_time
                .map(|paused| theme::ramp(theme::BAR_TIME_FROM, theme::BAR_TIME_TO, paused.as_secs_f64()));
            let text_color = if bar.is_resting {
                theme::BAR_TEXT_IDLE
            } else {
                paused_ramp.unwrap_or(theme::BAR_TEXT_IDLE)
            };

            // —— 左侧时钟图标（暂停态出现，15×15；v2 Visibility 绑 IsPaused 触发器、
            //      Foreground 绑 txt.Foreground = 暂停渐变色，“暂停→立刻休息”序列中
            //      v2 同样可见且带渐变色，B9）——
            if let Some(icon_color) = paused_ramp {
                paint_clock_icon(
                    &painter,
                    rect.left() + 4.0 + 7.5, // XAML Margin="4,0,0,0"，图标 15×15
                    rect.center().y,
                    7.5,
                    icon_color,
                );
            }

            // —— 居中文字：休息文案 / PausedTime / mm:ss（B2/B10/D6）——
            let text = if bar.is_resting {
                texts.resting.clone()
            } else {
                format_mmss(paused_time.unwrap_or(bar.main_countdown))
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::monospace(13.0),
                text_color,
            );

            // —— 右侧置顶复选（18×18 + 1px 右边距，来自 TopMostCheckBox 样式，B4/B5）——
            // v2 复选 Opacity=0 时仍可命中，此处同样始终可点击
            let check_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - 18.0 - 1.0, rect.center().y - 9.0),
                egui::vec2(18.0, 18.0),
            );
            let check_resp = ui.interact(
                check_rect,
                egui::Id::new("eyenurse_bar_topmost"),
                egui::Sense::click(),
            );

            // 悬停跟踪：淡出带 0.7s 宽限（v2 退场故事板：0.7s 保持后 0.3s 淡出）
            let hover_id = egui::Id::new("eyenurse_bar_hover");
            let now = Instant::now();
            if bar_resp.hovered() || check_resp.hovered() {
                ui.ctx().data_mut(|d| d.insert_temp(hover_id, now));
            }
            let last_hover: Option<Instant> = ui.ctx().data_mut(|d| d.get_temp(hover_id));
            let shown = bar_resp.hovered()
                || check_resp.hovered()
                || last_hover.is_some_and(|t| now.duration_since(t) < Duration::from_millis(700));
            let alpha = ui.ctx().animate_value_with_time(
                egui::Id::new("eyenurse_bar_check_alpha"),
                if shown { 1.0 } else { 0.0 },
                0.3, // 淡入 0.3s；淡出在 0.7s 宽限后同样 0.3s
            );

            if alpha > 0.01 {
                if check_resp.hovered() {
                    // 悬停底色 #FFFFFF（CheckBox.xaml 模板 MouseOver）
                    painter.rect_filled(
                        check_rect.shrink(1.0),
                        0.0,
                        fade(theme::BAR_CHECK_HOVER_BG, alpha),
                    );
                }
                // 复选框本体：12×12 方框 + 勾（v2 用 40×40 PNG 缩放为 18 显示，自绘近似）
                let box_rect =
                    egui::Rect::from_center_size(check_rect.center(), egui::vec2(12.0, 12.0));
                painter.rect_stroke(
                    box_rect,
                    1.0,
                    egui::Stroke::new(1.2, fade(theme::BAR_CHECK_GLYPH, alpha)),
                    egui::StrokeKind::Inside,
                );
                if m.topmost() {
                    let c = fade(theme::BAR_CHECK_GLYPH, alpha);
                    let stroke = egui::Stroke::new(1.6, c);
                    let l = box_rect.left() + 2.5;
                    let r = box_rect.right() - 2.5;
                    let bot = box_rect.bottom() - 3.0;
                    let mid = box_rect.center().y + 1.0;
                    let top = box_rect.top() + 3.5;
                    let mx = box_rect.center().x;
                    painter.line_segment([egui::pos2(l, mid), egui::pos2(mx, bot)], stroke);
                    painter.line_segment([egui::pos2(mx, bot), egui::pos2(r, top)], stroke);
                }
            }
            // on_hover_text 消费 self，回绑后继续判 clicked
            let check_resp = check_resp.on_hover_text(texts.always_on_top.clone());
            if check_resp.clicked() {
                // 勾选即写 UI.json + 切换视口层级（对齐 v2 TopMost_Checked/Unchecked，B5）
                m.toggle_topmost(!m.topmost());
            }

            // —— 右键菜单：与托盘完全同一组菜单项/顺序/动作（一致性差异 #4：
            //      v2 CountdownWindow 构造函数 ContextMenu = App.Menu，与托盘共用
            //      同一个 ContextMenu 对象；pause/resume 互换展示读托盘菜单状态）——
            bar_resp.context_menu(|ui| {
                if ui.button(texts.about.clone()).clicked() {
                    m.tray_action(crate::tray::TrayAction::About);
                    ui.close();
                }
                if ui.button(texts.setting.clone()).clicked() {
                    m.tray_action(crate::tray::TrayAction::Settings);
                    ui.close();
                }
                ui.separator();
                if m.tray_paused_shown() {
                    if ui.button(texts.resume.clone()).clicked() {
                        m.tray_action(crate::tray::TrayAction::Resume);
                        ui.close();
                    }
                } else if ui.button(texts.pause.clone()).clicked() {
                    m.tray_action(crate::tray::TrayAction::Pause);
                    ui.close();
                }
                if ui.button(texts.reset.clone()).clicked() {
                    m.tray_action(crate::tray::TrayAction::Reset);
                    ui.close();
                }
                if ui.button(texts.rest_now.clone()).clicked() {
                    m.tray_action(crate::tray::TrayAction::RestNow);
                    ui.close();
                }
                ui.separator();
                if ui.button(texts.exit.clone()).clicked() {
                    m.tray_action(crate::tray::TrayAction::Exit);
                    ui.close();
                }
            });
        });
}

/// 按 alpha 淡化颜色（复选框淡入淡出）
fn fade(c: egui::Color32, alpha: f32) -> egui::Color32 {
    c.gamma_multiply(alpha.clamp(0.0, 1.0))
}

/// 暂停态时钟图标（egui 自绘：表圈 + 指针；v2 PackIconMaterialLight Kind="Clock"）
fn paint_clock_icon(painter: &egui::Painter, cx: f32, cy: f32, r: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.4, color);
    painter.circle_stroke(egui::pos2(cx, cy), r, stroke);
    // 时针（4 点方向）+ 分针（12 点方向）——纯装饰形状
    painter.line_segment(
        [egui::pos2(cx, cy), egui::pos2(cx + r * 0.5, cy + r * 0.5)],
        stroke,
    );
    painter.line_segment([egui::pos2(cx, cy), egui::pos2(cx, cy - r * 0.75)], stroke);
}

/// `mm:ss`（对齐 v2 StringFormat=mm\:ss）。
/// `mm` 是 TimeSpan 的**分钟分量（00-59）**而非总分钟：剩余 >59min 时回绕显示
/// （90min 间隔首帧显示 "30:00"，清单 B2/Q13 决议保留）。
pub fn format_mmss(d: std::time::Duration) -> String {
    let total = d.as_secs();
    format!("{:02}:{:02}", (total / 60) % 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn mmss_uses_minute_component_with_wraparound() {
        // 清单 B2/Q13：mm 是分钟分量，90min 显示 "30:00" 而非 "90:00"
        assert_eq!(format_mmss(Duration::from_secs(90 * 60)), "30:00");
        assert_eq!(format_mmss(Duration::from_secs(45 * 60)), "45:00");
        assert_eq!(format_mmss(Duration::from_secs(0)), "00:00");
        assert_eq!(format_mmss(Duration::from_secs(59)), "00:59");
        // 3725s = 1:02:05 → 分量 mm=02 ss=05
        assert_eq!(format_mmss(Duration::from_secs(3725)), "02:05");
        assert_eq!(format_mmss(Duration::from_secs(3600)), "00:00");
    }

    #[test]
    fn mmss_truncates_subsecond() {
        // 倒计时带亚秒剩余时向下取整显示
        assert_eq!(format_mmss(Duration::from_millis(2999)), "00:02");
    }
}
