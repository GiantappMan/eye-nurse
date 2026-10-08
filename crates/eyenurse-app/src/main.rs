//! 入口（实现者 B 所有，架构文档 §5.1）。
//!
//! 托盘常驻 GUI 程序不能带控制台黑窗（v2 为 WPF WinExe 同理；debug 构建保留控制台
//! 方便看日志）。
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod events;
mod tray;
mod ui;
mod win;

use app::AppModel;
use eyenurse_core::config::{ConfigStore, LanguagesJson, UiJson};

fn main() {
    // 1) 单实例（v3 新增，清单 J3）：CreateMutexW("Local\\EyeNurse3")，
    //    ERROR_ALREADY_EXISTS → 直接退出
    if !win::single_instance::acquire() {
        return;
    }

    // 2) 配置读取（I18n 在 AppModel::new 里装配：磁盘优先，内嵌兜底）
    let cfg = ConfigStore::v2_default();
    let setting: eyenurse_core::config::SettingJson = cfg.load();
    let ui_cfg: UiJson = cfg.load();
    let _lang: LanguagesJson = cfg.load(); // 校验 Languages.json 可解析（AppModel::new 会再装配 I18n）

    // 2.5) 自启值名静默迁移（清单 Q6/G2）："Eye Nurse 2" → "Eye Nurse 3"，无 UI 提示
    let startup =
        eyenurse_core::autostart::DesktopStartup::new("Eye Nurse 3", app::exe_path_string());
    startup.migrate_from("Eye Nurse 2");

    // 2.6) startup.set_enabled(setting.RunWhenStarts) —— **每次启动都应用**
    //      （对齐 v2 App.xaml.cs:43 → EyeNurseService.Init → ApplySetting；
    //        清单 G5“首启写自启”；AppModel::new 内也有同调用兜底）
    startup.set_enabled(setting.RunWhenStarts);

    // 3) WTS 会话锁线程（失败降级：仅丢失会话锁暂停/重置能力）
    let session = win::session::SessionThread::spawn().ok();
    let rx = match &session {
        Some(s) => s.rx.clone(),
        None => crossbeam_channel::unbounded().1,
    };

    // 4) run_native：悬浮条 = root 视口；AppModel::new 里启动计时器
    //    （对齐 v2 EyeNurseViewModel.Init cs:98-102）
    let top_most = ui_cfg.TopMost;
    let result = eframe::run_native(
        "EyeNurse3",
        native_options(top_most, &ui_cfg),
        Box::new(move |cc| Ok(Box::new(AppModel::new(cc, rx)) as Box<dyn eframe::App>)),
    );

    // 5) run_native 返回后：request_stop → WTS 注销 → 线程自然退出 → join（§7.3）
    if let Some(session) = session {
        session.request_stop();
        session.join();
    }

    if let Err(e) = result {
        eprintln!("eframe 运行失败: {e}");
    }
}

/// top_most = UiJson.TopMost。
/// 评审修订：`with_always_on_top(self)` 无参（docs.rs 已核实），只能条件调用；
/// 无条件置顶会把用户关掉的置顶在重启后改回去（清单 B5）。
fn native_options(top_most: bool, ui_cfg: &UiJson) -> eframe::NativeOptions {
    let b = egui::ViewportBuilder::default()
        .with_title("CountdownWindow")
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(false)
        .with_inner_size([80.0, 20.0]) // 逻辑点；v2 XAML 80x20 DIP
        .with_position([
            ui_cfg.CoutdownWindowLeft.0 as f32,
            ui_cfg.CoutdownWindowTop.0 as f32,
        ]); // UiJson 两字段（v2 默认 100,100）
    let b = if top_most { b.with_always_on_top() } else { b };
    eframe::NativeOptions {
        viewport: b,
        renderer: eframe::Renderer::Glow,
        run_and_return: true,
        ..Default::default()
    }
}
