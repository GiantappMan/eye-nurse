//! 入口（实现者 B 所有，tauri 架构文档 §1.1/§2）。
//!
//! 启动序列 1-2.6 与 egui 版完全一致（单实例 → 配置读取 → 自启迁移与应用 →
//! WTS 会话线程）；eframe 段替换为 tauri::Builder：
//! - setup 钩子（主线程，启动一次）：构建托盘 → spawn bar 线程 → spawn CoreLoop；
//! - on_window_event：settings 关闭请求 → prevent_close + hide（§8.1）；
//!   mask 关闭请求 → prevent_close（§5.3，遮罩唯一关闭入口是解锁按钮与倒计时）；
//! - 命令面（§4.1）经 `commands::CoreHandle` 注入 CoreMsg 总线。
//!
//! 托盘常驻 GUI 程序不能带控制台黑窗（v2 为 WPF WinExe 同理；debug 构建保留
//! 控制台方便看日志）。
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod commands;
mod core;
mod events;
mod smoke;
mod tray;
mod ui;
mod win;

use crate::core::{CoreBundle, CoreLoop};
use events::CoreMsg;
use eyenurse_core::config::{ConfigStore, LanguagesJson, SettingJson, UiJson};
use eyenurse_core::i18n::I18n;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

fn main() {
    // 0) --smoke 冒烟旗标（架构 §11 / 清单验收基线 7，教训 3）：程序化验证三类窗口
    //    的创建路径，结果打到 stdout、以退出码表达（smoke.rs）。自测进程不改变用户
    //    环境——跳过 1) 单实例互斥（避免与运行中的实例互踩导致静默假通过）与
    //    2.5/2.6) 的自启注册表写入
    let smoke = std::env::args().any(|arg| arg == "--smoke");

    // 1) 单实例（清单 J3）：CreateMutexW("Local\\EyeNurse3")，
    //    ERROR_ALREADY_EXISTS → 直接退出（tauri-plugin-single-instance 不引入：§0 非目标）。
    //    release 无控制台，静默让位是预期行为；debug 下必须打印原因——实测教训：
    //    残留实例会让 tauri dev 秒退且外层报 255，不打印会被当成应用崩溃去排查
    if !smoke && !win::single_instance::acquire() {
        // 英文提示：Windows 控制台默认 GBK 代码页，中文 eprintln 会乱码（实测）
        eprintln!(
            "An Eye Nurse instance is already running (single-instance mutex Local\\EyeNurse3). \
             This launch yields and exits. To debug, end it first: taskkill /IM eyenurse.exe /F"
        );
        return;
    }

    // 2) 配置读取（I18n 装配：磁盘优先，内嵌兜底）
    let cfg = ConfigStore::v2_default();
    let setting: SettingJson = cfg.load();
    let ui_cfg: UiJson = cfg.load();
    let lang: LanguagesJson = cfg.load();
    let i18n = load_i18n(&cfg, &lang);

    // 2.5) 自启值名静默迁移（清单 Q6/G2）："Eye Nurse 2" → "Eye Nurse 3"，无 UI 提示；
    //      2.6) set_enabled(RunWhenStarts) **每次启动都应用**（对齐 v2 App.xaml.cs:43 →
    //      EyeNurseService.Init → ApplySetting；清单 G5）。--smoke 跳过：验证窗口
    //      创建路径不应写用户注册表
    let startup =
        eyenurse_core::autostart::DesktopStartup::new("Eye Nurse 3", exe_path_string());
    if !smoke {
        startup.migrate_from("Eye Nurse 2");
        startup.set_enabled(setting.RunWhenStarts);
    }

    // 3) WTS 会话锁线程（失败降级：仅丢失会话锁暂停/重置能力）
    let session = win::session::SessionThread::spawn().ok();
    let rx_win = match &session {
        Some(s) => s.rx.clone(),
        None => crossbeam_channel::unbounded().1,
    };

    // 3.5) CoreMsg 总线 + 转发线程（§3）：WinEvent 即时转入 CoreMsg——会话锁事件
    //      即时唤醒 CoreLoop（无轮询延迟），session.rs 自身零改动；
    //      转发线程随进程退出终止，无需显式 join
    let (core_tx, core_rx) = crossbeam_channel::unbounded::<CoreMsg>();
    {
        let core_tx = core_tx.clone();
        std::thread::Builder::new()
            .name("eyenurse-forwarder".into())
            .spawn(move || {
                while let Ok(ev) = rx_win.recv() {
                    let _ = core_tx.send(CoreMsg::Win(ev));
                }
            })
            .expect("转发线程创建失败");
    }

    // 4) tauri 应用（settings 窗由 tauri.conf.json 静态声明，启动即建、visible:false）
    let result = tauri::Builder::default()
        .manage(commands::CoreHandle {
            tx: core_tx.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::set_settings,
            commands::reset_settings,
            commands::rest_now,
            commands::get_autostart,
            commands::get_current_language,
            commands::set_current_language,
            commands::get_texts,
            commands::get_languages,
            commands::rest_done,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // 4.1) 托盘（tauri 要求主线程建，§7）
            let texts = tray::I18nTexts::from_i18n(&i18n);
            let tray = tray::build(&handle, core_tx.clone(), &texts).expect("托盘创建失败");

            // 设置窗初始标题 = 本地化 app_name（v2 MainWindow.xaml
            // `lan:Xaml.Key="app_name"` 绑定的等价；切语言由 CoreLoop
            // set_language 持续刷新，清单 E7）
            if let Some(w) = handle.get_webview_window("settings") {
                let _ = w.set_title(&texts.app_name);
            }

            // 4.2) 悬浮条（原生 Win32，独立线程，§6）
            let interval = setting.RestInterval.as_duration();
            let bar_snapshot = Arc::new(Mutex::new(ui::bar::BarSnapshot {
                countdown: interval,
                interval,
                paused_time: None,
                resting: false,
                paused_menu_shown: false,
                topmost: ui_cfg.TopMost,
                saved_left: ui_cfg.CoutdownWindowLeft.0,
                saved_top: ui_cfg.CoutdownWindowTop.0,
                texts: texts.clone(),
            }));
            let bar_handle = ui::bar::spawn(
                core_tx.clone(),
                bar_snapshot.clone(),
                ui::bar::BarInit {
                    left: ui_cfg.CoutdownWindowLeft.0,
                    top: ui_cfg.CoutdownWindowTop.0,
                    topmost: ui_cfg.TopMost,
                },
            )
            .expect("悬浮条线程创建失败");

            // 4.3) GlobalTimer 启动（对齐 v2 EyeNurseViewModel.Init cs:98-102；
            //      interval < 1s 的手改配置回退默认 45 分钟，v2 构造会抛异常，v3 容错）
            let mut timer = eyenurse_core::timer::GlobalTimer::new(interval)
                .or_else(|_| {
                    eyenurse_core::timer::GlobalTimer::new(Duration::from_secs(45 * 60))
                })
                .expect("默认间隔必合法");
            timer.start(Instant::now());

            // 4.4) CoreLoop 线程（唯一业务状态写者，§2）
            let bundle = CoreBundle {
                cfg,
                setting,
                ui: ui_cfg,
                lang,
                i18n,
                timer,
            };
            // 4.5) --smoke：冒烟线程（三类窗口创建路径验证 → stdout + 退出码，§11）。
            //      smoke::run 只 spawn 线程立即返回，必须放在 CoreLoop::new 之前——
            //      handle / bar_handle.hwnd 在那里被 move，之后无从克隆
            if smoke {
                smoke::run(handle.clone(), bar_handle.hwnd.clone());
            }
            let core_loop = CoreLoop::new(
                handle,
                core_rx,
                bundle,
                startup,
                tray,
                bar_snapshot,
                bar_handle.hwnd,
            );
            std::thread::Builder::new()
                .name("eyenurse-core".into())
                .spawn(move || core_loop.run())?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match window.label() {
                    "settings" => {
                        // §8.1：点 X → 先通知页面 flush 未到点的防抖保存，再
                        // prevent_close + hide（页面状态/未保存编辑天然不丢）
                        let _ = window
                            .app_handle()
                            .emit_to("settings", "eyenurse://settings-closing", ());
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    label if label.starts_with("mask") => {
                        // §5.3：用户 Alt+F4 遮罩一律拦截（destroy() 不走此路径，
                        // 关闭唯一入口是解锁按钮与倒计时）
                        api.prevent_close();
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!());

    // 5) run 返回后：request_stop → WTS 注销 → 线程自然退出 → join（§7.3）
    if let Some(session) = session {
        session.request_stop();
        session.join();
    }

    if let Err(e) = result {
        eprintln!("tauri 运行失败: {e}");
    }
}

/// 配置 + i18n 装配（egui 版 app.rs 原样搬进 main）：磁盘优先（保持 v2
/// “替换 json 即换文案”能力），exe 旁 Assets/Languages 不存在时回退内嵌副本（§9）。
fn load_i18n(_cfg: &ConfigStore, lang: &LanguagesJson) -> I18n {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("Assets").join("Languages")));
    let system = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    let current = lang.CurrentLan.as_deref();
    match dir {
        Some(dir) if dir.is_dir() => I18n::load(&dir, current, "en", &system),
        _ => I18n::load_embedded(current, "en", &system),
    }
}

/// exe 完整路径（自启注册表值数据；失败回退空串，v2 传参同源）
fn exe_path_string() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}
