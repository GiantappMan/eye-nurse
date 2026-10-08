//! 视口编排（实现者 B 独占，纯接线；骨架期定稿后不再改，架构文档 §5.7）。

pub mod bar;
pub mod fonts;
pub mod mask;
pub mod settings;
pub mod theme;

/// 评审修订：ViewportId 没有 _named 之类的命名构造（0.36.2 仅有 ROOT 常量与
/// from_hash_of），from_hash_of 非 const fn，故用函数而非 const。
pub fn settings_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("settings")
}

pub fn mask_id(i: usize) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(format!("mask-{i}"))
}

/// 由 `AppModel::logic` 每帧调用（deferred 视口“当帧不注册即销毁”，egui 文档原文）。
/// mod.rs 只保留两行接线，注册逻辑在各自所有者的文件里：
pub fn show_viewports(ctx: &egui::Context, m: &mut crate::app::AppModel) {
    mask::maybe_show(ctx, m); // 【C】ui/mask.rs：遮罩开着 → 注册 N 个遮罩视口
    settings::maybe_show(ctx, m); // 【D】ui/settings.rs：设置窗开着 → 注册设置视口，
                                  //      含 close_requested → UiAction::CloseSettings
}
