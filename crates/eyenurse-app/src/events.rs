//! 事件通道定义（架构文档 §5.2）。通道全部 `crossbeam_channel::unbounded`。
//!
//! 事件路由规则（评审修订）：
//! - root（`App::ui`/`App::logic`）里对 AppModel 是同线程直接方法调用；
//! - deferred 视口闭包拿不到 `&mut AppModel` → 一切跨视口动作走 `UiAction` 通道
//!   （`SharedUi.actions`），且发送后必须唤醒 ROOT（架构文档 §6.1）；
//! - Win32 线程走 `WinEvent` 通道。

/// Win32 线程 → 主循环
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinEvent {
    /// WM_WTSSESSION_CHANGE wParam==WTS_SESSION_LOCK(7)
    SessionLock,
    /// wParam==WTS_SESSION_UNLOCK(8)
    SessionUnlock,
    /// WM_DISPLAYCHANGE：显示器的数量/布局可能变了
    DisplayChanged,
}

/// 跨视口动作（deferred 视口闭包 → `SharedUi.actions` → `AppModel::logic` 排空）
/// 发送方：遮罩解锁（RestDone，ui/mask.rs）、设置窗（SaveSettings/CloseSettings/
/// SetLanguage/ResetSettings，ui/settings.rs）；ToggleTopmost 由 root 直接调用
/// AppModel::toggle_topmost，该变体预留（枚举级 allow 抑制未构造变体的 dead_code）
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    /// 遮罩上的解锁按钮（等价 v2 CloseCommand）
    RestDone,
    /// 设置窗保存（文本已由设置窗侧 parse + clamp 成结构化值）
    SaveSettings(eyenurse_core::config::SettingJson),
    /// 设置窗“恢复默认”（清单 F11；语义 = v2 ClientApi.ResetSettings）
    ResetSettings,
    ToggleTopmost(bool),
    CloseSettings,
    SetLanguage(String),
}
