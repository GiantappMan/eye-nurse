//! UI 模块编排（tauri 架构文档 §1.1）：`pub mod bar; pub mod theme;`
//! —— egui 版的 fonts/mask/settings 随壳迁移删除（fonts → WebView2 系统字体；
//!    mask/settings → ui/ 前端页面 + commands.rs）。

pub mod bar;
pub mod theme;
