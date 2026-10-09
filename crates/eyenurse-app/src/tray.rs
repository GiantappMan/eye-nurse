//! 托盘构建 + 菜单（tauri 架构文档 §7；实现者 B 所有）。菜单顺序逐项复刻
//! v2 App.xaml.cs:48-91：about / setting / ─ / pause / resume(初始隐藏) / reset /
//! rest_now / ─ / exit。
//!
//! pause/resume 可见性互换的实现限制（与 egui 版同一限制）：tauri 的 `MenuItem`
//! 只有 `set_text`/`set_enabled`（menu/normal.rs:82-105，无 set_visible），沿用
//! `menu.remove(当前) + menu.insert(另一项, pause_index=3)` 实现等效互换。
//!
//! 必须在主线程（setup 钩子）构建——tauri 要求托盘在主线程建（§2）。
//! 构建后 Tray/Menu/MenuItem 句柄均为 Send+Sync（tauri menu/mod.rs:91-92、
//! tray/mod.rs:417-418 已核实），CoreLoop 线程可安全持有并 set_text/互换。

use crate::events::CoreMsg;
use crossbeam_channel::Sender;
use eyenurse_core::i18n::I18n;
use std::cell::Cell;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

/// 界面文案快照（I18n 一次取齐，避免 UI 各处反复查询；与 egui 版字段一致）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I18nTexts {
    pub app_name: String,
    /// 皮肤中心窗口标题（ESP-1 独立窗口；main setup + set_language 同步刷新）
    pub skin_center: String,
    pub about: String,
    pub setting: String,
    pub pause: String,
    pub resume: String,
    pub reset: String,
    pub rest_now: String,
    pub exit: String,
    pub resting: String,
    pub always_on_top: String,
    // 设置窗（页面层消费，随 get_texts 下发）
    pub client_settings: String,
    pub run_when_starts: String,
    pub reset_when_session_unlock: String,
    pub rest_interval: String,
    pub rest_duration: String,
    /// 设置窗“恢复默认”按钮（清单 F11）
    pub restore_defaults: String,
    pub settings: String,
    pub panel: String,
    pub home: String,
    pub copyright: String,
    pub zh: String,
    pub en: String,
    /// v3.2 托盘「立即锁屏」（仿锁屏激活入口；Win+L 被管控软件拦截的机器上的
    /// 兜底路径之一，另有 Ctrl+Alt+L 热键）
    pub lock_now_menu: String,
}

impl I18nTexts {
    pub fn from_i18n(i18n: &I18n) -> Self {
        Self {
            app_name: i18n.get("app_name"),
            skin_center: i18n.get("skin_center"),
            about: i18n.get("about"),
            setting: i18n.get("setting"),
            pause: i18n.get("pause"),
            resume: i18n.get("resume"),
            reset: i18n.get("reset"),
            rest_now: i18n.get("rest_now"),
            exit: i18n.get("exit"),
            resting: i18n.get("resting"),
            always_on_top: i18n.get("always_on_top"),
            client_settings: i18n.get("clientSettings"),
            run_when_starts: i18n.get("runWhenStarts"),
            reset_when_session_unlock: i18n.get("resetWhenSessionUnlock"),
            rest_interval: i18n.get("restInterval"),
            rest_duration: i18n.get("restDuration"),
            restore_defaults: i18n.get("restoreDefaults"),
            settings: i18n.get("settings"),
            panel: i18n.get("panel"),
            home: i18n.get("home"),
            copyright: i18n.get("copyright"),
            zh: i18n.get("zh"),
            en: i18n.get("en"),
            lock_now_menu: i18n.get("lock_now_menu"),
        }
    }
}

/// 托盘菜单 id → 应用动作（on_menu_event 回调里映射后经 CoreMsg::Tray 转入 CoreLoop）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    About,
    Settings,
    Pause,
    Resume,
    Reset,
    RestNow,
    /// v3.2 立即锁屏（仿锁屏激活；未启用时 CoreLoop 侧幂等无事）
    LockNow,
    Exit,
}

/// 菜单 id 字符串（与 v2 语言键同名）
pub fn menu_action(id: &str) -> Option<TrayAction> {
    Some(match id {
        "about" => TrayAction::About,
        "setting" => TrayAction::Settings,
        "pause" => TrayAction::Pause,
        "resume" => TrayAction::Resume,
        "reset" => TrayAction::Reset,
        "rest_now" => TrayAction::RestNow,
        "lock_now_menu" => TrayAction::LockNow,
        "exit" => TrayAction::Exit,
        _ => return None,
    })
}

/// 托盘句柄（CoreLoop 持有：set_language 时 set_text 七项 + tooltip；
/// pause/resume 动作时互换菜单）
pub struct Tray {
    tray: TrayIcon<Wry>,
    menu: Menu<Wry>,
    items: MenuItems,
    /// pause 在菜单中的位置（resume 互换时插回这个位置）
    pause_index: usize,
    /// 菜单当前是否处于“恢复可见”态：set_paused 的幂等守卫。
    /// muda 的 `Menu::insert` 不查重（重复插入会出现两项“暂停”，egui 版
    /// tray.rs:201-213 的教训原样适用）。
    paused_shown: Cell<bool>,
}

struct MenuItems {
    about: MenuItem<Wry>,
    setting: MenuItem<Wry>,
    pause: MenuItem<Wry>,
    resume: MenuItem<Wry>,
    reset: MenuItem<Wry>,
    rest_now: MenuItem<Wry>,
    lock_now_menu: MenuItem<Wry>,
    exit: MenuItem<Wry>,
}

/// 在主线程（setup 钩子）构建托盘；菜单/双击事件一律 convert 成
/// `CoreMsg::Tray(...)` / `CoreMsg::ShowBar` 后经 tx 发出，回调里不干活（§2）。
/// 失败（极少见，如 explorer 未运行）骨架期直接 panic，后续可降级为无托盘模式。
pub fn build(app: &AppHandle<Wry>, tx: Sender<CoreMsg>, texts: &I18nTexts) -> tauri::Result<Tray> {
    // 菜单顺序固定（v2 App.xaml.cs:48-91）
    let menu = Menu::new(app)?;
    let about = MenuItem::with_id(app, "about", &texts.about, true, None::<&str>)?;
    let setting = MenuItem::with_id(app, "setting", &texts.setting, true, None::<&str>)?;
    // 先建好、不插入（初始隐藏；E2：v2 App.xaml.cs:59 Collapsed）
    let pause = MenuItem::with_id(app, "pause", &texts.pause, true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", &texts.resume, true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", &texts.reset, true, None::<&str>)?;
    let rest_now = MenuItem::with_id(app, "rest_now", &texts.rest_now, true, None::<&str>)?;
    let lock_now_menu =
        MenuItem::with_id(app, "lock_now_menu", &texts.lock_now_menu, true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", &texts.exit, true, None::<&str>)?;

    // about(0) setting(1) ─(2) pause(3) reset(4) rest_now(5) lock_now_menu(6) ─(7) exit(8)
    for item in [&about, &setting] {
        menu.append(item)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&pause)?;
    menu.append(&reset)?;
    menu.append(&rest_now)?;
    menu.append(&lock_now_menu)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&exit)?;
    let pause_index = 3;

    let tx_menu = tx.clone();
    let tray = TrayIconBuilder::with_id("eyenurse")
        .icon(tauri::image::Image::from_bytes(crate::core::LOGO_PNG)?) // feature "image-png"
        .tooltip(&texts.app_name)
        .menu(&menu)
        // 2.2 起的正确名（menu_on_left_click 已废弃）
        .show_menu_on_left_click(false)
        // 该回调对任何菜单事件触发（tray/mod.rs:328 注记）；本设计只有托盘一个
        // tauri 菜单（bar 右键菜单是独立 Win32 HMENU），不会误收
        .on_menu_event(move |_, ev| {
            if let Some(a) = menu_action(ev.id().as_ref()) {
                tx_menu.send(CoreMsg::Tray(a)).ok();
            }
        })
        // v2 双击托盘 → ShowCountdownWindow（App.xaml.cs:162-165 →
        // EyeNurseViewModel.cs:148-160：按 UI.json 持久化 TopMost/位置恢复显示）；
        // v3 同语义，经 CoreMsg::ShowBar → WM_APP_SHOW（B11）
        .on_tray_icon_event(move |_, ev| {
            if matches!(ev, TrayIconEvent::DoubleClick { .. }) {
                tx.send(CoreMsg::ShowBar).ok();
            }
        })
        .build(app)?;

    Ok(Tray {
        tray,
        menu,
        items: MenuItems {
            about,
            setting,
            pause,
            resume,
            reset,
            rest_now,
            lock_now_menu,
            exit,
        },
        pause_index,
        // resume 初始未插入 → 菜单处于“未暂停”展示
        paused_shown: Cell::new(false),
    })
}

impl Tray {
    /// 切语言后刷新菜单标题 + tooltip（对齐 v2 UpdateNotifyIconText；清单 E7：
    /// set_text ×7 + tooltip 重设——tooltip 也承载 app_name，随语言一起换）
    pub fn update_texts(&self, texts: &I18nTexts) {
        let _ = self.items.about.set_text(&texts.about);
        let _ = self.items.setting.set_text(&texts.setting);
        let _ = self.items.pause.set_text(&texts.pause);
        let _ = self.items.resume.set_text(&texts.resume);
        let _ = self.items.reset.set_text(&texts.reset);
        let _ = self.items.rest_now.set_text(&texts.rest_now);
        let _ = self.items.lock_now_menu.set_text(&texts.lock_now_menu);
        let _ = self.items.exit.set_text(&texts.exit);
        let _ = self.tray.set_tooltip(Some(&texts.app_name));
    }

    /// 菜单当前是否处于“恢复可见”态（bar 右键菜单与托盘保持同一组 pause/resume
    /// 互换展示；v2 两者共用同一个 ContextMenu 对象，状态天然一致。
    /// v3 bar 读共享快照的 paused_menu_shown，本方法保留给后续冒烟/诊断消费）
    #[allow(dead_code)]
    pub fn is_paused_shown(&self) -> bool {
        self.paused_shown.get()
    }

    /// 菜单互换：**无条件**调用（对齐 v2 App.xaml.cs:129-143：点击处理先翻菜单再调
    /// vm.Pause()，Pause 失败也翻，清单 Q12/D5）。
    /// 无 set_visible：`set_paused(true)` = `remove(pause)` + `insert(resume, 原
    /// pause 位置)`，false 反之。幂等：状态未变时直接返回（insert 不查重）。
    pub fn set_paused(&self, paused: bool) {
        if self.paused_shown.get() == paused {
            return;
        }
        let (to_remove, to_insert) = if paused {
            (&self.items.pause, &self.items.resume)
        } else {
            (&self.items.resume, &self.items.pause)
        };
        let _ = self.menu.remove(to_remove);
        let _ = self.menu.insert(to_insert, self.pause_index);
        self.paused_shown.set(paused);
    }
}
