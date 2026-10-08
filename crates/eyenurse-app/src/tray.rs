//! 托盘构建 + 菜单（架构文档 §5.3）。菜单顺序逐项复刻 v2 App.xaml.cs:48-91：
//! about / setting / ─ / pause / resume(初始隐藏) / reset / rest_now / ─ / exit。
//!
//! pause/resume 可见性互换的实现限制（评审修订）：muda 0.21.1 的 `MenuItem` 没有
//! `set_visible`，用菜单的 remove + insert 实现等效互换（`set_paused`）。

use eyenurse_core::i18n::I18n;
use std::cell::Cell;

/// 界面文案快照（AppModel::texts() 从 I18n 一次取齐，避免 UI 各处反复查询）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I18nTexts {
    pub app_name: String,
    pub about: String,
    pub setting: String,
    pub pause: String,
    pub resume: String,
    pub reset: String,
    pub rest_now: String,
    pub exit: String,
    pub resting: String,
    pub always_on_top: String,
    // 设置窗（实现者 D 消费）
    pub client_settings: String,
    pub run_when_starts: String,
    pub reset_when_session_unlock: String,
    pub rest_interval: String,
    pub rest_duration: String,
    /// 设置窗“恢复默认”按钮（清单 F11；v2 ClientApi.ResetSettings 有后端无入口）
    pub restore_defaults: String,
    pub settings: String,
    pub panel: String,
    pub home: String,
    pub copyright: String,
    pub zh: String,
    pub en: String,
}

impl I18nTexts {
    pub fn from_i18n(i18n: &I18n) -> Self {
        Self {
            app_name: i18n.get("app_name"),
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
        }
    }
}

/// 托盘菜单 id → 应用动作（app.rs logic 里排空 MenuEvent 后调用本映射）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    About,
    Settings,
    Pause,
    Resume,
    Reset,
    RestNow,
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
        "exit" => TrayAction::Exit,
        _ => return None,
    })
}

pub struct Tray {
    _tray: tray_icon::TrayIcon,
    menu: tray_icon::menu::Menu,
    items: MenuItems,
    /// pause 在菜单中的位置（resume 互换时插回这个位置）
    pause_index: usize,
    /// 菜单当前是否处于“恢复可见”态：set_paused 的幂等守卫。
    /// muda 0.21.1 的 `Menu::insert` 不查重（内部 `state.children.insert` 直接插入），
    /// 状态未变时重复调用会把 pause 项插成两条（休息结束路径会无条件调用 set_paused(false)）。
    paused_shown: Cell<bool>,
}

struct MenuItems {
    about: tray_icon::menu::MenuItem,
    setting: tray_icon::menu::MenuItem,
    pause: tray_icon::menu::MenuItem,
    resume: tray_icon::menu::MenuItem,
    reset: tray_icon::menu::MenuItem,
    rest_now: tray_icon::menu::MenuItem,
    exit: tray_icon::menu::MenuItem,
}

impl Tray {
    /// 必须在 eframe creator 回调里（= winit 主线程，之后该线程跑消息泵，满足
    /// tray-icon 文档要求 "On Windows ... an event loop must be running on the thread"）。
    /// i18n 文案由调用方传入，返回 Self 供 drop 时注销。
    pub fn new(icon_rgba: Vec<u8>, w: u32, h: u32, texts: &I18nTexts) -> tray_icon::Result<Self> {
        use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
        use tray_icon::{Icon, TrayIconBuilder};

        // menu::Error / BadIcon → io::Error → tray_icon::Error（From 链）
        fn to_io(e: impl std::fmt::Display) -> std::io::Error {
            std::io::Error::other(e.to_string())
        }
        let icon = Icon::from_rgba(icon_rgba, w, h).map_err(to_io)?;

        // 菜单顺序固定（v2 App.xaml.cs:48-91）
        let menu = Menu::new();
        let about = MenuItem::with_id("about", texts.about.clone(), true, None);
        let setting = MenuItem::with_id("setting", texts.setting.clone(), true, None);
        let pause = MenuItem::with_id("pause", texts.pause.clone(), true, None);
        let resume = MenuItem::with_id("resume", texts.resume.clone(), true, None); // 先建好，不插入（初始隐藏）
        let reset = MenuItem::with_id("reset", texts.reset.clone(), true, None);
        let rest_now = MenuItem::with_id("rest_now", texts.rest_now.clone(), true, None);
        let exit = MenuItem::with_id("exit", texts.exit.clone(), true, None);

        // about(0) setting(1) ─(2) pause(3) reset(4) rest_now(5) ─(6) exit(7)
        for item in [&about, &setting] {
            menu.append(item).map_err(to_io)?;
        }
        menu.append(&PredefinedMenuItem::separator())
            .map_err(to_io)?;
        for item in [&pause, &reset, &rest_now] {
            menu.append(item).map_err(to_io)?;
        }
        menu.append(&PredefinedMenuItem::separator())
            .map_err(to_io)?;
        menu.append(&exit).map_err(to_io)?;
        let pause_index = 3;

        let tray = TrayIconBuilder::new()
            .with_tooltip(texts.app_name.clone())
            .with_icon(icon)
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .build()?;

        Ok(Self {
            _tray: tray,
            menu,
            items: MenuItems {
                about,
                setting,
                pause,
                resume,
                reset,
                rest_now,
                exit,
            },
            pause_index,
            // resume 初始未插入（v2 App.xaml.cs:59 Collapsed）→ 菜单处于“未暂停”展示
            paused_shown: Cell::new(false),
        })
    }

    /// 切语言后刷新菜单标题（对齐 v2 UpdateNotifyIconText）
    pub fn update_texts(&self, texts: &I18nTexts) {
        self.items.about.set_text(texts.about.clone());
        self.items.setting.set_text(texts.setting.clone());
        self.items.pause.set_text(texts.pause.clone());
        self.items.resume.set_text(texts.resume.clone());
        self.items.reset.set_text(texts.reset.clone());
        self.items.rest_now.set_text(texts.rest_now.clone());
        self.items.exit.set_text(texts.exit.clone());
    }

    /// 菜单当前是否处于“恢复可见”态（bar 右键菜单与托盘保持同一组 pause/resume
    /// 互换展示；v2 两者共用同一个 ContextMenu 对象，状态天然一致）
    pub fn is_paused_shown(&self) -> bool {
        self.paused_shown.get()
    }

    /// 菜单互换：**无条件**调用（对齐 v2 App.xaml.cs:129-143：点击处理先翻菜单再调
    /// vm.Pause()，Pause 失败也翻，清单 Q12/D5）。
    /// muda 无 set_visible：`set_paused(true)` = `remove(pause)` + `insert(resume, 原
    /// pause 位置)`，false 反之（架构文档 §5.3 评审修订）。
    /// 幂等：状态未变时直接返回（muda insert 不查重，重复插入会出现两项“暂停”）。
    pub fn set_paused(&self, paused: bool) {
        if self.paused_shown.get() == paused {
            return;
        }
        let (to_remove, to_insert, at) = if paused {
            (&self.items.pause, &self.items.resume, self.pause_index)
        } else {
            (&self.items.resume, &self.items.pause, self.pause_index)
        };
        self.menu.remove(to_remove).ok();
        self.menu.insert(to_insert, at).ok();
        self.paused_shown.set(paused);
    }

    /// D 的设置窗切语言/换图标后消费；骨架期允许暂未被调用
    #[allow(dead_code)]
    pub fn set_icon_rgba(&mut self, rgba: Vec<u8>, w: u32, h: u32) -> tray_icon::Result<()> {
        fn to_io(e: impl std::fmt::Display) -> std::io::Error {
            std::io::Error::other(e.to_string())
        }
        let icon = tray_icon::Icon::from_rgba(rgba, w, h).map_err(to_io)?;
        let _ = self._tray.set_icon(Some(icon));
        Ok(())
    }
}
