//! 每显示器休息遮罩（实现者 C 所有）——内容 + 视口注册 `maybe_show`（架构文档 §5.7）。
//!
//! v2 对照（LockScreenWindow.xaml + §6.2）：背景 #141418、文字 #EEEEF2、居中 `mm:ss`
//! 倒计时、70×70 加载圈、右下角 25×25 解锁图标按钮（→ UiAction::RestDone，等价
//! v2 CloseCommand → CloseLockScreen，EyeNurseViewModel.cs:108-117）。
//! 位置/大小 = 每显示器 `rcWork`（v2 EyeNurseViewModel.cs:189-192，不覆盖任务栏，Q5）；
//! v3 现枚举而非启动快照（Q4 决议，枚举发生在 AppModel::rest_now）。

use crate::app::AppModel;
use crate::events::UiAction;
use crate::ui::theme;
use crate::win::monitors::MonitorRects;
use std::sync::Arc;
use std::time::Duration;

/// 遮罩窗口标题（= v2 LockScreenWindow 标题；WS_EX_TOOLWINDOW 自愈按
/// “本进程 + 标题”识别要补丁的窗口，见 win::heal_toolwindows）
pub const MASK_TITLE: &str = "LockScreenWindow";

/// 几何收敛容差（物理像素，§7.4 收敛式校正）
const CORRECTION_EPSILON_PX: f32 = 1.0;

/// 由 `AppModel::logic` 每帧调用（deferred 视口“当帧不注册即销毁”）：
/// 遮罩开着 → 注册 N 个遮罩视口（每显示器 rcWork 一个，v2 EyeNurseViewModel.cs:189-192）。
pub fn maybe_show(ctx: &egui::Context, m: &mut AppModel) {
    let (open, monitors) = {
        let mask = m.shared().mask.lock().unwrap_or_else(|e| e.into_inner());
        (mask.open, mask.monitors.clone())
    };
    if !open {
        return;
    }
    // 初始换算用 root 当前的组合 ppp 估计（egui ppp = zoom × native）：
    // 单一缩放环境一次到位；混合 DPI 下首帧可能偏（egui-winit 按“创建落点屏”的
    // ppp 应用 builder，§7.4），由闭包内的收敛式校正 1-2 帧内贴合 rcWork。
    let guess_ppp = ctx.pixels_per_point();
    for (i, mon) in monitors.iter().enumerate() {
        let id = crate::ui::mask_id(i);
        let shared = m.shared().clone();
        let mon = mon.clone();
        let (origin, size) = logical_rect(&mon.rc_work, guess_ppp);
        ctx.show_viewport_deferred(id, viewport_builder(origin, size), move |ui, _class| {
            mask_viewport_ui(ui, &shared, id, &mon);
        });
    }
}

/// 遮罩视口 builder（每显示器；物理坐标已按 ppp 换算为逻辑点，§5.7/§7.4）
fn viewport_builder(origin: egui::Pos2, size: egui::Vec2) -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(MASK_TITLE)
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top() // v2 Topmost="True"
        .with_active(false) // 出现时不抢焦点（v2 Topmost + ShowInTaskbar=False 的遮罩观感）
        .with_taskbar(false) // v2 ShowInTaskbar="False"（DeleteTab；Alt-Tab 由自愈补丁兜住）
        .with_position(origin)
        .with_inner_size(size)
}

fn mask_viewport_ui(
    ui: &mut egui::Ui,
    shared: &Arc<crate::app::SharedUi>,
    id: egui::ViewportId,
    mon: &MonitorRects,
) {
    let (open, countdown) = {
        let mask = shared.mask.lock().unwrap_or_else(|e| e.into_inner());
        (mask.open, mask.lock_countdown)
    };
    if !open {
        return; // 上一帧残留的闭包：直接退出，本帧后视口销毁
    }

    // 收敛式校正（§7.4）：误差 >1px 时按当前 ppp 反推并重发位置/尺寸命令
    correct_geometry(ui, id, &mon.rc_work);
    // 遮罩自调度 1Hz 重绘，否则 mm:ss 冻结（§6.1 要点）
    ui.ctx().request_repaint_after(Duration::from_secs(1));
    // WS_EX_TOOLWINDOW 自愈（§7.2）：logic 的每帧补丁发生在遮罩窗口创建之前
    // （show_viewports 先注册，窗口随后才建），此处补一次让首帧即从 Alt-Tab 隐藏
    crate::win::heal_toolwindows();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::MASK_BG))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let center = rect.center();

            // 70×70 加载圈（v2 Viewbox 70×70 + hc:LoadingCircle）
            let spinner_rect = egui::Rect::from_center_size(center, egui::vec2(70.0, 70.0));
            ui.put(
                spinner_rect,
                egui::Spinner::new().size(70.0).color(theme::MASK_FG),
            );
            // mm:ss 叠在加载圈正中（v2 Grid 两控件同 Center，文字覆盖在圈上）
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                crate::ui::bar::format_mmss(countdown),
                egui::FontId::monospace(16.0), // v2 TextBlock 默认 12pt ≈ 16px
                theme::MASK_FG,
            );

            // 右下角 25×25 解锁图标按钮（v2 Margin="0,0,12,12"，提前结束休息，C5）
            let btn_rect = egui::Rect::from_min_size(
                rect.max - egui::vec2(25.0 + 12.0, 25.0 + 12.0),
                egui::vec2(25.0, 25.0),
            );
            let btn = ui.interact(
                btn_rect,
                egui::Id::new("eyenurse_mask_unlock"),
                egui::Sense::click(),
            );
            paint_lock_open(&painter, btn_rect.center(), theme::MASK_FG);
            if btn.clicked() {
                // 解锁按钮 → 动作出口 + 立即唤醒 ROOT（§6.1，清单 C11：
                // deferred 视口输入不会执行 root logic，不得依赖 1Hz 心跳）
                shared.actions.0.send(UiAction::RestDone).ok();
                ui.ctx()
                    .request_repaint_after_for(Duration::ZERO, egui::ViewportId::ROOT);
            }
        });
}

/// §7.4 收敛式校正：视口当前物理矩形（outer_rect × 当前组合 ppp）与目标 rcWork
/// 误差 >1px 时，按**当前** ppp 反推逻辑值，同帧发 OuterPosition + InnerSize
/// （两条命令内部同乘“当时的 ppp”，必须成对、同一 ppp 来源）。
/// 无状态：未对齐时每帧重发（窗口移动/跨屏后 ppp 变化也能在 1-2 帧内收敛），
/// 已对齐时零开销；`WinEvent::DisplayChanged` 后同样由本流程自然重排。
fn correct_geometry(ui: &egui::Ui, id: egui::ViewportId, target_rc_work: &[i32; 4]) {
    let ppp = ui.ctx().pixels_per_point();
    if !ppp.is_finite() || ppp <= 0.0 {
        return;
    }
    let Some(outer) = ui.ctx().input(|i| i.viewport().outer_rect) else {
        return; // 首帧尚无窗口几何信息
    };
    let cur_origin_px = outer.min * ppp;
    let cur_size_px = outer.size() * ppp;
    let want_origin = [target_rc_work[0] as f32, target_rc_work[1] as f32];
    let want_size = [
        (target_rc_work[2] - target_rc_work[0]) as f32,
        (target_rc_work[3] - target_rc_work[1]) as f32,
    ];
    let err = (cur_origin_px.x - want_origin[0])
        .abs()
        .max((cur_origin_px.y - want_origin[1]).abs())
        .max((cur_size_px.x - want_size[0]).abs())
        .max((cur_size_px.y - want_size[1]).abs());
    if err > CORRECTION_EPSILON_PX {
        let (origin, size) = logical_rect(target_rc_work, ppp);
        ui.ctx()
            .send_viewport_cmd_to(id, egui::ViewportCommand::OuterPosition(origin));
        ui.ctx()
            .send_viewport_cmd_to(id, egui::ViewportCommand::InnerSize(size));
    }
}

/// 物理 rcWork + ppp → 逻辑 (origin, size)（§7.4：logical = physical / ppp）
fn logical_rect(rc_work: &[i32; 4], ppp: f32) -> (egui::Pos2, egui::Vec2) {
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    (
        egui::pos2(rc_work[0] as f32 / ppp, rc_work[1] as f32 / ppp),
        egui::vec2(
            (rc_work[2] - rc_work[0]) as f32 / ppp,
            (rc_work[3] - rc_work[1]) as f32 / ppp,
        ),
    )
}

/// 解锁图标（v2 PackIconMaterialLight Kind="LockUnLocked"，25×25）：实心锁体 +
/// 脱开上翘的锁梁，自绘近似（v2 图标来自 Material 字体文件，无法逐点复刻）
fn paint_lock_open(painter: &egui::Painter, center: egui::Pos2, color: egui::Color32) {
    // 锁体：14×10 圆角矩形，位于中心偏下
    let body = egui::Rect::from_min_size(
        egui::pos2(center.x - 7.0, center.y - 1.0),
        egui::vec2(14.0, 10.0),
    );
    painter.rect_filled(body, 2.0, color);
    // 锁梁：上半圆弧从左端到右端，右端上翘 = 开启
    let r = 4.5_f32;
    let cx = center.x - 2.5;
    let cy = center.y - 1.0;
    let mut pts = vec![egui::pos2(cx - r, cy)];
    for step in 1..=10 {
        let a = std::f32::consts::PI * (1.0 - step as f32 / 10.0); // π → 0
        pts.push(egui::pos2(cx + r * a.cos(), cy - r * a.sin()));
    }
    pts.push(egui::pos2(cx + r, cy - 5.0)); // 开口端上翘
    painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0, color)));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 浮点比较（几何换算允许极小误差）
    fn assert_close(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn logical_rect_identity_at_scale_one() {
        let rc = [100, 100, 900, 700];
        let (origin, size) = logical_rect(&rc, 1.0);
        assert_close(origin.x, 100.0);
        assert_close(origin.y, 100.0);
        assert_close(size.x, 800.0);
        assert_close(size.y, 600.0);
    }

    #[test]
    fn logical_rect_divides_by_scale_factor() {
        // §7.4：logical = physical / ppp（150% 缩放屏）
        let rc = [150, 150, 1350, 1050];
        let (origin, size) = logical_rect(&rc, 1.5);
        assert_close(origin.x, 100.0);
        assert_close(origin.y, 100.0);
        assert_close(size.x, 800.0);
        assert_close(size.y, 600.0);
    }

    #[test]
    fn logical_rect_falls_back_to_one_on_bad_ppp() {
        let rc = [0, 0, 800, 600];
        let (o1, s1) = logical_rect(&rc, 0.0);
        let (o2, s2) = logical_rect(&rc, f32::NAN);
        assert_close(s1.x, 800.0);
        assert_close(s2.x, 800.0);
        assert_close(o1.x, 0.0);
        assert_close(o2.x, 0.0);
    }
}
