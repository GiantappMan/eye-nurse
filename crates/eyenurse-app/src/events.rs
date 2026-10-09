//! 事件通道定义（tauri 架构文档 §2）。通道全部 `crossbeam_channel::unbounded`，
//! IPC 应答通道用 `bounded(1)`。
//!
//! 事件路由规则（Tauri 版）：
//! - **CoreLoop 线程**是唯一业务状态写者；托盘/bar/Win32 线程/async 命令一律
//!   只 convert 成 [`CoreMsg`] → `tx.send()`，不在各自线程干活；
//! - Win32 会话锁线程经 main 里的 3 行转发线程把 [`WinEvent`] 转成
//!   `CoreMsg::Win`（会话锁事件即时唤醒 CoreLoop，无轮询延迟）；
//! - 页面（WebView）只经 `commands.rs` 的命令与 CoreLoop 交互。
//!
//! **应答契约**：凡携带 [`Reply`] 的变体，CoreLoop 在**所有**处理路径（含错误路径，
//! 如写盘失败）必须恰好回包一次；命令侧 `recv_timeout(2s)` 兜底，超时映射为
//! Err 返回页面，不挂线程。

use crate::skin_proto::SkinSurfaceDto;
use crate::tray::TrayAction;
use crossbeam_channel::Sender;
use eyenurse_core::config::{LockScreenJson, SettingJson};
use eyenurse_core::skin::{SkinColors, SkinDef, SkinsDto};
use eyenurse_core::stats::StatsDay;
use std::collections::BTreeMap;

/// Win32 线程 → CoreLoop（复用自 egui 版，零改动）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinEvent {
    /// WM_WTSSESSION_CHANGE wParam==WTS_SESSION_LOCK(7)
    SessionLock,
    /// wParam==WTS_SESSION_UNLOCK(8)
    SessionUnlock,
    /// WM_DISPLAYCHANGE：显示器的数量/布局可能变了
    DisplayChanged,
}

/// IPC/查询应答通道（bounded(1)；CoreLoop 侧恰好回包一次，命令侧 recv_timeout 兜底）
pub type Reply<T> = Sender<T>;

/// 所有线程/命令 → CoreLoop 的消息总线（= egui 版 `UiAction + TrayAction + WinEvent`
/// 的并集搬移；归属 B，冻结契约：变体增删归 B/D 协商，他人只消费）。
#[derive(Debug)]
pub enum CoreMsg {
    /// 托盘菜单与 bar 右键菜单共用同一动作入口；「打开设置窗」唯一入口是
    /// `Tray(TrayAction::Settings)`（§2 评审修订：不设 ShowSettings 变体）
    Tray(TrayAction),
    /// 会话锁/显示器变化（session 线程经转发线程转入）
    Win(WinEvent),
    /// 遮罩解锁按钮（等价 v2 CloseCommand）→ 提前结束休息
    RestDone,
    /// 设置窗语言下拉（写 Languages.json + 托盘/标题刷新 + 广播 i18n-changed）
    SetLanguage(String),
    /// 设置保存（写盘 + 自启 + 计时器按新间隔重建）→ 回应用后的值
    SaveSettings(SettingJson, Reply<SettingJson>),
    /// 恢复默认（F11）：`SettingJson::default()` 走同一应用路径 → 回应后的值
    ResetSettings(Reply<SettingJson>),
    /// bar 拖动结束（WM_EXITSIZEMOVE，逻辑 DIP）→ 写 UI.json 并保存（Q9）
    SaveBarPosition { left: f64, top: f64 },
    /// bar 置顶复选点击 → 写 UI.json + WM_APP_TOPMOST 驱动原生置顶
    ToggleTopmost(bool),
    /// 托盘双击 → bar 按 UI.json 持久化 TopMost/位置恢复显示（B11，经 WM_APP_SHOW；
    /// 对齐 v2 ShowCountdownWindow，非无条件置顶）
    ShowBar,
    /// ESP-1：HTML 悬浮条（barweb 皮肤）请求右键菜单（eyenurse.openMenu() 桥）→
    /// CoreLoop 在临时线程弹原生菜单（win::menu），选中动作经 Tray 回流
    ShowBarMenu,
    // —— 只读查询（各携带 Reply，取数路径 = CoreLoop 内存态，§4.1 评审修订）——
    /// 当前 SettingJson（内存态，不直读磁盘——写盘非原子，并发读可能撕裂）
    GetSettings(Reply<SettingJson>),
    /// 当前语言整包文案（settings/mask 页面的启动依赖）
    GetTexts(Reply<BTreeMap<String, String>>),
    /// 可用语言清单（I18n::available）
    GetLanguages(Reply<Vec<String>>),
    /// 当前语言名
    GetCurrentLanguage(Reply<String>),
    // —— v3.1 换肤 ——
    /// 选择皮肤（内置 id / 磁盘 id / "custom"）：写 Skin.json + bar 快照/WM_APP_SKIN
    /// + 广播 skin-changed
    SetSkin(String),
    /// 保存用户自定义配色并启用（Current="custom"）→ 回应生效后的完整配色
    ///（Box：SkinDef 11 个 Option 字段体积远超其他变体，装箱抑制 large_enum_variant）
    SetCustomSkin(Box<SkinDef>, Reply<SkinColors>),
    /// 可用皮肤清单（内置 + 磁盘 + 自定义槽）+ 当前选中 id
    GetSkins(Reply<SkinsDto>),
    /// 当前生效配色（含逐键回退结果）
    GetSkin(Reply<SkinColors>),
    /// ESP-1 表面运行时（配色 + bar/lock 皮肤页 URL + chrome 摆放 + bar 模式）
    GetSkinSurface(Reply<SkinSurfaceDto>),
    // —— v3.2 仿锁屏 / 护眼统计 ——
    /// 键盘钩子：Win+L 被拦截 → 激活仿锁屏（锁屏设置开启才生效）
    LockActivate,
    /// 锁屏遮罩解锁：密码不匹配回 false（CoreLoop 侧验证）
    LockTryUnlock(String, Reply<bool>),
    /// 锁屏设置保存（写 LockScreen.json + 钩子开关 + 注册表策略）→ 回应用后的值
    SaveLockSettings(LockScreenJson, Reply<LockScreenJson>),
    /// 当前锁屏设置（CoreLoop 内存态，同 GetSettings 纪律）
    GetLockSettings(Reply<LockScreenJson>),
    /// 近 n 天护眼统计（含今天，旧→新，缺失天补零）
    GetStats(u32, Reply<Vec<StatsDay>>),
}
