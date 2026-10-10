//! 悬浮倒计时条（实现者 C 所有）—— v3 唯一保留的原生 GUI：独立线程 +
//! Win32 消息循环 + GDI 双缓冲自绘（tauri 架构文档 §6；行为对齐 egui 版 bar.rs）。
//!
//! 线程与窗口（§6.1）：`spawn()` 在 setup 钩子里启动独立 std::thread（名
//! `eyenurse-bar`）：RegisterClassW("EyeNurse3BarWnd") → CreateWindowExW
//! （标题 `CountdownWindow`，WS_POPUP | WS_EX_TOOLWINDOW [| WS_EX_TOPMOST]）→
//! GetMessageW 循环。尺寸 80×20 逻辑 × 窗口 DPI；全部内部几何以物理像素工作，
//! 只在读写 UI.json 时换算（教训 1：拖动路径的逻辑/物理混用从机制上消除）。
//!
//! 三条实测教训的落地（§6.3）：
//! 1. **拖动 = 系统拖动**：`WM_NCHITTEST` 命中本窗口且不在复选命中区 →
//!    `HTCAPTION`，移动循环全由系统接管，代码不做任何逐帧 SetWindowPos；
//!    `WM_EXITSIZEMOVE` → GetWindowRect（物理）÷ GetDpiForWindow（逻辑）→
//!    `CoreMsg::SaveBarPosition` → CoreLoop 写 UI.json（Q9：拖动结束即存）。
//!    **由此带来的派生结论**：条身 HTCAPTION 使多数鼠标消息呈**非客户区**形态，
//!    悬停跟踪必须同时收 WM_MOUSEMOVE/WM_NCMOUSEMOVE，并以
//!    TME_LEAVE|TME_NONCLIENT 挂单次离开跟踪；
//! 2. **右键菜单 = `TrackPopupMenuEx(TPM_RETURNCMD)` 顶层弹出**（绝无 80×20
//!    表面内裁剪），id/顺序/pause-resume 互换与托盘同组（v2 ContextMenu =
//!    App.Menu 同一对象），选中动作经 `CoreMsg::Tray` 与托盘共用同一入口；
//!    弹出前置前台、关闭后补投 WM_NULL（KB135788：非前台窗口的弹出菜单
//!    点外部不消失的经典坑）；
//! 3. **悬停浮现置顶复选（B4）**：进入条内浮现（0.7s 宽限后隐藏——v2 退场
//!    故事板为 0.7s 保持 + 0.3s 淡出，淡入淡出为观感增强项，先交付硬显隐 +
//!    宽限定时器，§6.2/§13）；复选命中区挂 tooltip 控件（`tooltips_class32`，
//!    文案 = `always_on_top` 键，切语言热更新）。
//!
//! 与 CoreLoop 的接口（§6.4）：
//! - CoreLoop → bar 数据：`Arc<Mutex<BarSnapshot>>`，`WM_TIMER(1s)` 读快照重绘；
//! - CoreLoop → bar 事件：`WM_APP_TOPMOST` / `WM_APP_LANGUAGE` / `WM_APP_SHOW`；
//! - bar → CoreLoop：`cmd_tx: Sender<CoreMsg>`——`SaveBarPosition`（拖动结束）/
//!   `ToggleTopmost`（复选点击）/ `Tray(TrayAction::…)`（右键菜单，与托盘同源）。

use crate::events::CoreMsg;
use crate::tray::{I18nTexts, TrayAction};
use crate::ui::theme;
use crossbeam_channel::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// bar 窗口类名 / 标题（代码内写死常量；冒烟按类名 FindWindow）
pub const BAR_CLASS_NAME: &str = "EyeNurse3BarWnd";
pub const BAR_TITLE: &str = "CountdownWindow";

/// 逻辑尺寸（v2 XAML 80x20 DIP）
const LOGICAL_WIDTH: f64 = 80.0;
const LOGICAL_HEIGHT: f64 = 20.0;
/// 复选命中区宽（18×18 复选 + 1px 右边距，来自 TopMostCheckBox 样式 / egui 版
/// `rect.right - 18 - 1`，B4/B5）——交互契约（命中/tooltip/拖动分界），不改
const CHECK_LOGICAL: f64 = 19.0;

// —— 外观度量 token（DS3 §5.6-6：绘制不再出现内联魔法数；改外观只动这里。
//    单位 = 逻辑 DIP，经 scale_px 按 DPI 换算；间距 4pt 网吸附，
//    TEXT_ICON_GAP=6 为 DS3 §5.6-5 明定的唯一例外）——
/// 主文字字高（DS3 --fs-body 13px；barweb 壳页 tokens.bar 同基准：20px 条 → 13px）
const FONT_SIZE: f64 = 13.0;
/// 暂停时钟图标半径（⌀13 = 字高：装饰不压过主数据，层级 1:1，DS3 §5.6-5；
/// v2 为 ⌀15 > 文字 13 的层级倒置，已修）
const ICON_RADIUS: f64 = 6.5;
/// 图标左缘留白（= DS3 --sp-1；沿用 v2 XAML Margin 4）
const ICON_MARGIN: f64 = 4.0;
/// 暂停时图标与文字的最小分离（DS3 §5.6-5：文字矩形从图标右缘再让 6 逻辑px，
/// 取代 v2 全宽居中压到图标边上的构图）
const TEXT_ICON_GAP: f64 = 6.0;
/// 图钉符号包围盒边长（v2 12×12 方框自绘的尺寸契约；2026-10 起该盒内画图钉，
/// 命中/拖动分界等交互契约不变）
const CHECK_BOX: f64 = 12.0;
/// 复选框在命中区内的左内缩（v2 3.5 → 4.0 入 4pt 网）
const CHECK_INSET: f64 = 4.0;
/// 复选悬停底色块边长（v2 18 → 16：20px 条身上下各留 2px 米边，DS3 §5.6-6，
/// 消灭「白块贴满条身像补丁」的硬切观感；v2 XAML 原值 18 仅存于注释史）
const CHECK_SIDE: f64 = 16.0;
/// 悬停底色块距窗缘/命中区边缘的发丝内缩（物理 px；v2 硬编码 ±1 的命名化）
const HAIRLINE_PX: i32 = 1;

// —— CoreLoop → bar 的 WM_APP 控制消息（§6.4）——
pub const WM_APP_TOPMOST: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1; // wparam: 1=置顶 0=取消
pub const WM_APP_LANGUAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2; // 文案已在共享快照
/// 托盘双击 → 按持久化 TopMost/位置恢复显示（B11，对齐 v2 ShowCountdownWindow；
/// 值由 CoreLoop 先写入共享快照再投递）
pub const WM_APP_SHOW: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 3;
/// 换肤（v3.1）：配色已写入共享快照，本消息只触发即时重绘（1s 心跳本也会覆盖）
pub const WM_APP_SKIN: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 4;

// —— WM_TIMER id ——
/// 1s 重绘心跳（读共享快照重绘，§2：与 CoreLoop 的 1Hz 时钟相互独立）
#[cfg(target_os = "windows")]
const TIMER_PAINT: usize = 1;
/// 复选隐藏的 0.7s 宽限（B4 退场：宽限内再进入即取消隐藏）
#[cfg(target_os = "windows")]
const TIMER_GRACE: usize = 2;
/// 复选隐藏宽限毫秒（v2 退场故事板：0.7s 保持后才淡出）
#[cfg(target_os = "windows")]
const GRACE_MS: u32 = 700;

// —— tooltip 相关裸常量与结构 ——
// windows crate 把 TTS_*/TTM_*/TTF_*/TOOLINFOW 收在 Win32::UI::Controls feature
// 后面（本 crate 未启用该 feature，归 B/D 的 Cargo.toml 管，C 不动签名外的依赖）；
// 值是稳定的 Win32 常量/布局，就地定义——与 WM_MOUSELEAVE 的处理同理。
/// TTS_ALWAYSTIP：即使属主窗口非激活也显示
#[cfg(target_os = "windows")]
const TTS_ALWAYSTIP: u32 = 0x0001;
/// TTS_NOPREFIX：不把 & 当加速键前缀
#[cfg(target_os = "windows")]
const TTS_NOPREFIX: u32 = 0x0002;
/// TTF_SUBCLASS：tooltip 控件子类化工具窗口自己收鼠标（免手动 TTM_RELAYEVENT）
#[cfg(target_os = "windows")]
const TTF_SUBCLASS: u32 = 0x0010;
/// TTM_ADDTOOLW = WM_USER + 50
#[cfg(target_os = "windows")]
const TTM_ADDTOOLW: u32 = 1074;
/// TTM_NEWTOOLRECTW = WM_USER + 52（DPI 变化后更新工具区矩形）
#[cfg(target_os = "windows")]
const TTM_NEWTOOLRECTW: u32 = 1076;
/// TTM_UPDATETIPTEXTW = WM_USER + 57（切语言热更新文案）
#[cfg(target_os = "windows")]
const TTM_UPDATETIPTEXTW: u32 = 1081;
/// tooltip 工具 id（rect 工具的任意非零 id，UPDATETIPTEXT/NEWTOOLRECT 靠它定位）
#[cfg(target_os = "windows")]
const TOOLTIP_TOOL_ID: usize = 1;
/// tooltip 文案缓冲长度（UTF-16 单元；`always_on_top` 文案远短于此）
#[cfg(target_os = "windows")]
const TOOLTIP_TEXT_CCH: usize = 64;

/// Win32 TOOLINFOW（commctrl.h tagTOOLINFOW 镜像；x64 自然对齐，与 windows
/// crate Controls::TTTOOLINFOW 布局一致——feature 未启用故本地定义）
#[cfg(target_os = "windows")]
#[repr(C)]
struct ToolInfoW {
    cb_size: u32,
    u_flags: u32,
    hwnd: windows::Win32::Foundation::HWND,
    u_id: usize,
    rect: windows::Win32::Foundation::RECT,
    hinst: *mut core::ffi::c_void,
    lpsz_text: *mut u16,
    l_param: isize,
    lp_reserved: *mut core::ffi::c_void,
}

/// 悬浮条运行时配色（v3.1 换肤；COLORREF 0x00BBGGRR，CoreLoop 从生效 SkinDef
/// 解析写入快照；on_paint 全部读这里——编译期 theme:: 常量退居「经典皮肤真源」）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarSkinColors {
    pub progress_bg: u32,
    pub progress_fill: u32,
    pub progress_from: u32,
    pub progress_to: u32,
    pub text_idle: u32,
    pub text_from: u32,
    pub text_to: u32,
    pub check_glyph: u32,
    pub check_hover_bg: u32,
}

impl BarSkinColors {
    /// SkinColors（#RRGGBB）→ COLORREF。非法值不会到达（解析侧已逐键回退），
    /// 这里仍兜底为黑。
    pub fn of(s: &eyenurse_core::skin::SkinColors) -> Self {
        fn c(hex: &str) -> u32 {
            let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
            let r = (v >> 16) & 0xFF;
            let g = (v >> 8) & 0xFF;
            let b = v & 0xFF;
            (b << 16) | (g << 8) | r
        }
        Self {
            progress_bg: c(&s.bar_progress_bg),
            progress_fill: c(&s.bar_progress_fill),
            progress_from: c(&s.bar_progress_from),
            progress_to: c(&s.bar_progress_to),
            text_idle: c(&s.bar_text_idle),
            text_from: c(&s.bar_text_from),
            text_to: c(&s.bar_text_to),
            check_glyph: c(&s.bar_check_glyph),
            check_hover_bg: c(&s.bar_check_hover_bg),
        }
    }
}

/// 悬浮条展示状态（§6.4：CoreLoop 每拍与状态变化时更新；bar 线程只读）
#[derive(Debug, Clone)]
pub struct BarSnapshot {
    /// 主倒计时剩余（Elapsed）
    pub countdown: Duration,
    /// 主间隔（进度分母）
    pub interval: Duration,
    /// 暂停累计时长（v2 PausedTime；None=非暂停态）
    pub paused_time: Option<Duration>,
    /// 休息中（文字切 `resting` 文案）
    pub resting: bool,
    /// 托盘/bar 菜单当前展示态（true=「恢复」可见；与托盘 set_paused 同步，§6.3-2）
    pub paused_menu_shown: bool,
    /// 置顶态（UI.json TopMost；复选显示值 + WS_EX_TOPMOST）
    pub topmost: bool,
    /// 持久化位置（逻辑 DIP，UI.json CoutdownWindowLeft/Top 的镜像；托盘双击
    /// WM_APP_SHOW 的位置回放源——v2 ShowCountdownWindow 每次按持久化位置恢复）
    pub saved_left: f64,
    pub saved_top: f64,
    /// 界面文案（切语言时 CoreLoop 整包替换，WM_APP_LANGUAGE 触发重绘）
    pub texts: I18nTexts,
    /// 运行时配色（v3.1 换肤；切皮肤时 CoreLoop 替换 + WM_APP_SKIN 触发重绘）
    pub skin: BarSkinColors,
}

/// bar 线程建窗后回填的句柄（复用 win/session.rs 的 OnceLock 模式，§6.4）
#[derive(Clone)]
pub struct BarHandle {
    pub hwnd: Arc<OnceLock<isize>>,
}

/// 初始摆放（UI.json 的逻辑 DIP + 置顶态；由 CoreLoop 装配方传入）
#[derive(Debug, Clone, Copy)]
pub struct BarInit {
    pub left: f64,
    pub top: f64,
    pub topmost: bool,
}

/// 在 setup 钩子里启动独立线程（Win32 消息循环自持；失败仅丢悬浮条，不致命）
pub fn spawn(
    cmd_tx: Sender<CoreMsg>,
    snapshot: Arc<Mutex<BarSnapshot>>,
    init: BarInit,
) -> std::io::Result<BarHandle> {
    let hwnd: Arc<OnceLock<isize>> = Arc::new(OnceLock::new());
    let hwnd_for_thread = hwnd.clone();
    std::thread::Builder::new()
        .name("eyenurse-bar".into())
        .spawn(move || {
            bar_thread_main(cmd_tx, snapshot, init, hwnd_for_thread);
        })?;
    Ok(BarHandle { hwnd })
}

/// 线程内状态（GWLP_USERDATA 持 Box 指针，wndproc 还原；session.rs 同款生命周期）
struct BarState {
    cmd_tx: Sender<CoreMsg>,
    snapshot: Arc<Mutex<BarSnapshot>>,
    dpi: u32,
    /// HFONT（isize 存放；WM_NCDESTROY 时 DeleteObject）
    font: isize,
    /// 复选当前是否浮现（B4：进入条内置位；0.7s 宽限定时器到点且未再进入才清位。
    /// 条身 HTCAPTION → 悬停跟踪同时收客户区/非客户区两种鼠标消息）
    check_shown: bool,
    /// 鼠标悬停在置顶复选命中区（白色悬停底色，v2 CheckBox 模板 MouseOver）
    check_hovered: bool,
    /// TrackMouseEvent 已挂单次离开跟踪（leave 事件单次有效，进入时重挂）
    tracking: bool,
    /// tooltip 控件 hwnd（0=未创建，如系统资源失败——仅观感降级，不致命）
    tooltip: isize,
    /// tooltip 文案缓冲（定长、地址稳定——ToolInfoW.lpszText 长期指向它；
    /// 切语言经 WM_APP_LANGUAGE 原地改写 + TTM_UPDATETIPTEXTW）
    tooltip_text: Box<[u16]>,
}

/// 逻辑 DIP → 物理像素
fn scale_px(logical: f64, dpi: u32) -> i32 {
    (logical * dpi as f64 / 96.0).round() as i32
}

/// 冒烟断言用（§11-3）：给定 DPI 下的期望物理尺寸（80×20 逻辑 × 窗口 DPI）
pub fn expected_size_px(dpi: u32) -> (i32, i32) {
    (scale_px(LOGICAL_WIDTH, dpi), scale_px(LOGICAL_HEIGHT, dpi))
}

/// 保存点所在显示器的 DPI（§6.1：换算基准 = 落点屏；取不到回退系统 DPI）。
/// 探测点先按系统 DPI 近似换算（落点屏 DPI 未知前无法精确换算——跨异 DPI 屏的
/// 「拖动 → 重启」往返可能有一次 scale 漂移，属 §6.1 记录在案的可接受偏差）。
#[cfg(target_os = "windows")]
fn dpi_at_point(left: f64, top: f64) -> u32 {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForSystem, MDT_EFFECTIVE_DPI};
    // SAFETY：纯查询 Win32 调用
    unsafe {
        let system = GetDpiForSystem().max(96);
        let probe = POINT {
            x: (left * system as f64 / 96.0).round() as i32,
            y: (top * system as f64 / 96.0).round() as i32,
        };
        let hmon = MonitorFromPoint(probe, MONITOR_DEFAULTTONEAREST);
        let (mut cx, mut cy) = (0u32, 0u32);
        if GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut cx, &mut cy).is_ok() && cx >= 96 {
            cx
        } else {
            system
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn dpi_at_point(_left: f64, _top: f64) -> u32 {
    96
}

/// DWM 圆角偏好（DS3 §5.6-4）：`DWMWA_WINDOW_CORNER_PREFERENCE`(33) = ROUND(2)，
/// Win11 给 WS_POPUP 直角条上系统圆角 + 标准投影（分离深度线索）；Win10 的旧
/// dwmapi 不认识该属性 → 返回错误码，静默忽略（保持直角回退）。
/// windows crate 未启用 Win32_Graphics_Dwm feature——与文件头 WM_MOUSELEAVE /
/// tooltip 常量同款「值稳定、就地声明」策略，免为一个调用拖入整个 feature。
#[cfg(target_os = "windows")]
fn set_rounded_corners(hwnd: isize) {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const u32, size: u32) -> i32;
    }
    // SAFETY：仅设置本进程窗口的 DWM 圆角偏好；失败（Win10）只丢圆角，不致命
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &DWMWCP_ROUND,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(target_os = "windows")]
fn bar_thread_main(
    cmd_tx: Sender<CoreMsg>,
    snapshot: Arc<Mutex<BarSnapshot>>,
    init: BarInit,
    hwnd_cell: Arc<OnceLock<isize>>,
) {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, LoadCursorW, RegisterClassW, SetTimer,
        SetWindowLongPtrW, TranslateMessage, GWLP_USERDATA, IDC_ARROW, HCURSOR, WINDOW_EX_STYLE,
        WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
    };

    // SAFETY：标准窗口类注册/建窗/消息循环；BarState 装箱指针由 wndproc 消费，
    // WM_NCDESTROY 时回收（与 session.rs 同款模式）
    unsafe {
        let hinstance = match GetModuleHandleW(None) {
            Ok(h) => h.into(),
            Err(_) => return,
        };
        // w! 只收字面量；类名/标题是模块常量（冒烟按名查找），运行时编码
        let class_name = HSTRING::from(BAR_CLASS_NAME);
        let title = HSTRING::from(BAR_TITLE);
        let class_pcw = PCWSTR(class_name.as_ptr());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(bar_wndproc),
            hInstance: hinstance,
            // 类名必须挂到注册结构上——漏写则 RegisterClassW 以 NULL 类名注册失败
            // 返回 0，线程静默 return：进程存活但悬浮条永不出现（启动冒烟教训）
            lpszClassName: class_pcw,
            // 类光标 = 箭头（缺省 NULL 会继承上一窗口的残留光标形态）
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or(HCURSOR(std::ptr::null_mut())),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }

        let dpi = dpi_at_point(init.left, init.top);
        let (w, h) = (scale_px(LOGICAL_WIDTH, dpi), scale_px(LOGICAL_HEIGHT, dpi));
        let (x, y) = (scale_px(init.left, dpi), scale_px(init.top, dpi));
        let ex_style = if init.topmost {
            WINDOW_EX_STYLE(WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0)
        } else {
            WS_EX_TOOLWINDOW
        };
        let hwnd = match CreateWindowExW(
            ex_style,
            class_pcw,
            PCWSTR(title.as_ptr()),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            w.max(1),
            h.max(1),
            None,
            None,
            Some(hinstance),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };
        let _ = hwnd_cell.set(hwnd.0 as isize);

        // Alt-Tab 隐藏补丁（§5.4/B7：skip_taskbar 类机制只删任务栏按钮，这里同函数复用）
        crate::win::ensure_toolwindow(hwnd.0 as isize);

        // Win11 圆角 + DWM 投影（DS3 §5.6-4；Win10 静默忽略保持直角）
        set_rounded_corners(hwnd.0 as isize);

        // tooltip 控件（§6.2/B4：复选命中区显示 `always_on_top` 文案）。
        // 缓冲先于 ToolInfoW 存在且地址稳定，lpszText 才能长期指向它。
        let mut tooltip_text: Box<[u16]> = vec![0u16; TOOLTIP_TEXT_CCH].into_boxed_slice();
        let always_on_top = snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .texts
            .always_on_top
            .clone();
        let tooltip = create_tooltip(hwnd, dpi, hinstance, &always_on_top, &mut tooltip_text);

        let state = Box::new(BarState {
            cmd_tx,
            snapshot,
            dpi,
            font: create_bar_font(dpi),
            check_shown: false,
            check_hovered: false,
            tracking: false,
            tooltip,
            tooltip_text,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        // 1s WM_TIMER：读共享快照重绘（两条 1Hz 时钟独立，互不阻塞，§2）
        SetTimer(Some(HWND(hwnd.0)), TIMER_PAINT, 1000, None);

        let mut msg = std::mem::zeroed();
        // GetMessageW 返回值：0 = WM_QUIT，-1 = 错误，>0 = 收到消息（session.rs 同款判定）
        loop {
            if GetMessageW(&mut msg, None, 0, 0).0 <= 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn bar_thread_main(
    _cmd_tx: Sender<CoreMsg>,
    _snapshot: Arc<Mutex<BarSnapshot>>,
    _init: BarInit,
    _hwnd_cell: Arc<OnceLock<isize>>,
) {
}

// ============================================================================
// 绘制与消息处理（GDI 双缓冲，行为对齐 egui 版 bar.rs:22-226，§6.2）
// ============================================================================

/// 按 DPI 自绘条字体（清单 H5：`CreateFontW("Microsoft YaHei UI")` 按 DPI 缩放，
/// 中文显示为方块=失败——mm:ss 居中由 DrawTextW DT_CENTER 保证；egui 版的等宽
/// 字体是 egui 自带字库的形态，GDI 下改用系统 CJK 字体防豆腐块）
#[cfg(target_os = "windows")]
fn create_bar_font(dpi: u32) -> isize {
    use windows::core::w;
    use windows::Win32::Graphics::Gdi::{
        CreateFontW, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
    };
    // SAFETY：GDI 字体对象，由 WM_NCDESTROY 释放
    unsafe {
        let h = CreateFontW(
            -(FONT_SIZE * dpi as f64 / 96.0).round() as i32, // 负值 = 字符高度
            0,
            0,
            0,
            400, // FW_NORMAL
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0, // DEFAULT_PITCH | FF_DONTCARE
            w!("Microsoft YaHei UI"),
        );
        h.0 as isize
    }
}

/// 把文案写入定长 UTF-16 缓冲（NUL 结尾、超长截断）——ToolInfoW.lpszText 长期
/// 指向该缓冲，缓冲必须定长且地址稳定
#[cfg(target_os = "windows")]
fn write_utf16_z(buf: &mut [u16], text: &str) {
    if buf.is_empty() {
        return;
    }
    let mut i = 0;
    for unit in text.encode_utf16() {
        if i >= buf.len() - 1 {
            break;
        }
        buf[i] = unit;
        i += 1;
    }
    buf[i] = 0;
}

/// 复选命中区矩形（tooltip 工具区；客户区坐标：右缘 CHECK_LOGICAL 全高条带）
#[cfg(target_os = "windows")]
fn check_zone_rect(hwnd: windows::Win32::Foundation::HWND, dpi: u32) -> windows::Win32::Foundation::RECT {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    let mut rc = RECT::default();
    // SAFETY：标准查询调用，rc 为本窗口客户区
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
        rc.left = rc.right - scale_px(CHECK_LOGICAL, dpi);
    }
    rc
}

/// 创建 tooltip 控件并挂复选命中区工具（TTF_SUBCLASS：tooltip 自行子类化收
/// 鼠标，无需手动转发）；失败返回 0（仅观感降级）。文字缓冲由调用方持有。
#[cfg(target_os = "windows")]
fn create_tooltip(
    parent: windows::Win32::Foundation::HWND,
    dpi: u32,
    hinstance: windows::Win32::Foundation::HINSTANCE,
    text: &str,
    text_buf: &mut [u16],
) -> isize {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_POPUP,
    };
    // SAFETY：标准 tooltip 控件创建路径；TOOLINFOW 为本地镜像布局，宽字符缓冲
    // NUL 结尾且活到调用返回（其后由 BarState 长期持有）
    unsafe {
        let class = HSTRING::from("tooltips_class32");
        let tip = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(WS_POPUP.0 | TTS_NOPREFIX | TTS_ALWAYSTIP),
            0,
            0,
            0,
            0,
            Some(parent), // owner：随主窗口销毁（WM_NCDESTROY 仍显式 Destroy 兜底）
            None,
            Some(hinstance),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return 0,
        };
        write_utf16_z(text_buf, text);
        let mut ti = ToolInfoW {
            cb_size: std::mem::size_of::<ToolInfoW>() as u32,
            u_flags: TTF_SUBCLASS,
            hwnd: parent,
            u_id: TOOLTIP_TOOL_ID,
            rect: check_zone_rect(parent, dpi),
            hinst: std::ptr::null_mut(),
            lpsz_text: text_buf.as_mut_ptr(),
            l_param: 0,
            lp_reserved: std::ptr::null_mut(),
        };
        let _ = SendMessageW(
            tip,
            TTM_ADDTOOLW,
            None,
            Some(LPARAM(&mut ti as *mut ToolInfoW as isize)),
        );
        tip.0 as isize
    }
}

#[cfg(target_os = "windows")]
fn snapshot_clone(state: &BarState) -> BarSnapshot {
    state
        .snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// 指针位置是否落在置顶复选命中区（右缘 CHECK_LOGICAL 逻辑宽，B4/B5）
#[cfg(target_os = "windows")]
fn in_check_zone(x: i32, client_w: i32, dpi: u32) -> bool {
    x >= client_w - scale_px(CHECK_LOGICAL, dpi)
}

/// 弹出条身右键菜单（`WM_CONTEXTMENU` 与 `WM_NCRBUTTONUP` 共用，触发条件见
/// wndproc 内注释）：原生 `TrackPopupMenuEx` 顶层弹出，绝无 80×20 表面内裁剪
/// 问题；状态变化时重建，id/顺序对齐 tray.rs（about/setting/—/[pause|resume
/// 互换]/reset/rest_now/—/exit）
#[cfg(target_os = "windows")]
fn show_bar_menu(
    hwnd: windows::Win32::Foundation::HWND,
    state: &mut BarState,
    l: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW,
        SetForegroundWindow, TrackPopupMenuEx, MF_SEPARATOR, TPM_NONOTIFY, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, WM_NULL,
    };

    let bar = snapshot_clone(state);
    let hmenu = match unsafe { CreatePopupMenu() } {
        Ok(m) => m,
        Err(_) => return LRESULT(0),
    };
    let t = &bar.texts;
    append_menu_item(hmenu, 1, &t.about);
    append_menu_item(hmenu, 2, &t.setting);
    let _ = unsafe { AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null()) };
    if bar.paused_menu_shown {
        append_menu_item(hmenu, 3, &t.resume);
    } else {
        append_menu_item(hmenu, 3, &t.pause);
    }
    append_menu_item(hmenu, 4, &t.reset);
    append_menu_item(hmenu, 5, &t.rest_now);
    let _ = unsafe { AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null()) };
    append_menu_item(hmenu, 6, &t.exit);

    // 弹出位置：鼠标（键盘唤起 lParam=-1 → 取当前光标）
    let (mut x, mut y) = ((l.0 as i16) as i32, ((l.0 >> 16) as i16) as i32);
    if (x, y) == (-1, -1) {
        let mut p = POINT::default();
        let _ = unsafe { GetCursorPos(&mut p) };
        (x, y) = (p.x, p.y);
    }
    // KB135788：非前台窗口的 TrackPopupMenu 点外部不消失——先置前台，
    // 菜单关闭后补投 WM_NULL 清队列残留
    let _ = unsafe { SetForegroundWindow(hwnd) };
    // TPM_RETURNCMD：返回值即所选 id（模态）；uflags 参数是裸 u32（0.62）
    let chosen = unsafe {
        TrackPopupMenuEx(
            hmenu,
            TPM_RETURNCMD.0 | TPM_RIGHTBUTTON.0 | TPM_NONOTIFY.0,
            x,
            y,
            hwnd,
            None,
        )
    }
    .0;
    let _ = unsafe { DestroyMenu(hmenu) };
    let _ = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };
    // id → TrayAction（与托盘共用同一动作入口，v2 ContextMenu = App.Menu）
    let paused_shown = bar.paused_menu_shown;
    let action = match chosen {
        1 => Some(TrayAction::About),
        2 => Some(TrayAction::Settings),
        3 => Some(if paused_shown {
            TrayAction::Resume
        } else {
            TrayAction::Pause
        }),
        4 => Some(TrayAction::Reset),
        5 => Some(TrayAction::RestNow),
        6 => Some(TrayAction::Exit),
        _ => None,
    };
    if let Some(a) = action {
        let _ = state.cmd_tx.send(CoreMsg::Tray(a));
    }
    LRESULT(0)
}

/// WM_CONTEXTMENU 菜单项追加（文本缓冲只需活到调用返回）
#[cfg(target_os = "windows")]
fn append_menu_item(
    hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    id: usize,
    text: &str,
) {
    use windows::Win32::UI::WindowsAndMessaging::{AppendMenuW, MF_ENABLED, MF_STRING};
    let w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY：自建 HMENU；宽字符缓冲以 NUL 结尾且活到调用返回
    unsafe {
        let _ = AppendMenuW(hmenu, MF_STRING | MF_ENABLED, id, windows::core::PCWSTR(w.as_ptr()));
    }
}

/// WM_PAINT：离屏 DC 按序画 → BitBlt 一次性上屏（§6.2 双缓冲）
#[cfg(target_os = "windows")]
fn on_paint(hwnd: windows::Win32::Foundation::HWND, state: &BarState) {
    use windows::Win32::Foundation::{COLORREF, RECT};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen,
        CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint, FillRect,
        FrameRect, GetStockObject, LineTo, MoveToEx, SelectObject, SetBkMode,
        SetTextColor, DT_CALCRECT, DT_CENTER, DT_SINGLELINE, DT_VCENTER, HBITMAP, HGDIOBJ,
        NULL_BRUSH, PAINTSTRUCT, PS_SOLID, SRCCOPY, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut ps = PAINTSTRUCT::default();
    // SAFETY：标准 WM_PAINT 序列；全部 GDI 对象在本函数内配对释放
    unsafe {
        let hdc = BeginPaint(hwnd, &mut ps);
        if hdc.is_invalid() {
            return;
        }
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let (cw, ch) = (rc.right - rc.left, rc.bottom - rc.top);

        let mem = CreateCompatibleDC(Some(hdc));
        let bmp: HBITMAP = CreateCompatibleBitmap(hdc, cw, ch);
        let old_bmp = SelectObject(mem, bmp.into());

        let bar = snapshot_clone(state);
        let dpi = state.dpi;
        let paused = bar.paused_time;
        let paused_secs = paused.map(|p| p.as_secs_f64());
        let skin = bar.skin; // v3.1 换肤：色值全部来自共享快照（classic 真源仍是 theme.json）
        let rgb = |v: u32| {
            theme::Rgb::new(
                (v & 0xFF) as u8,
                ((v >> 8) & 0xFF) as u8,
                ((v >> 16) & 0xFF) as u8,
            )
        };
        let colorref = |c: theme::Rgb| COLORREF(c.as_colorref());

        // 1) 底色（经典皮肤 = v2 "Beige"）
        let bg = CreateSolidBrush(COLORREF(skin.progress_bg));
        FillRect(mem, &rc, bg);
        let _ = DeleteObject(bg.into());

        // 文案与字体先行：暂停警示需要按实测文字宽度为填充让位（见 2)）
        let text = if bar.resting {
            bar.texts.resting.clone()
        } else {
            format_mmss(paused.unwrap_or(bar.countdown))
        };
        let mut wtext: Vec<u16> = text.encode_utf16().collect();
        SetBkMode(mem, TRANSPARENT);
        let old_font = SelectObject(mem, HGDIOBJ(state.font as _));

        // 2) 进度前景（B3；interval=0 全宽，同 egui 守卫）：
        //    - 休息帧（含暂停+休息）填充不吃渐变，回 progress_fill 纯色——对齐
        //      DS3 §5.4-1 barweb 语义（渐变是暂停专属表达），同帧只保留一套色彩
        //      系统（差异 #6 的填充侧收口；否则 idle 深字会叠上渐变深棕填充）；
        //    - 暂停警示填充设最小覆盖宽度（复审 (c)）：≥ 文字块 + ICON_MARGIN，
        //      mm:ss 与时钟图标恒落在同一 ramp 场上——文字与填充由同一插值 t 配对
        //      （起点 9.0:1 → 末态 #FF5252/#3E0D0D 5.19:1），杜绝文字落米色底
        //      （#FF5252 对 #F5F5DC 仅 2.88:1，比 v2 #F40000 的 3.92 更差的回退）。
        //      覆盖不足时 meter 让位于警示可读性（暂停中读数冻结，时长由 mm:ss 表达）。
        let warning = paused.is_some() && !bar.resting;
        let percent = if bar.interval.is_zero() {
            1.0
        } else {
            (bar.countdown.as_secs_f64() / bar.interval.as_secs_f64()).clamp(0.0, 1.0)
        };
        let fill = match paused_secs {
            Some(s) if !bar.resting => {
                theme::ramp(rgb(skin.progress_from), rgb(skin.progress_to), s)
            }
            _ => rgb(skin.progress_fill),
        };
        // 暂停文字矩形左缘（图标让位后的文字区起点，与 4) 的 DT_CENTER 同式）
        let paused_text_left = scale_px(ICON_MARGIN, dpi)
            + 2 * scale_px(ICON_RADIUS, dpi)
            + scale_px(TEXT_ICON_GAP, dpi);
        let mut fill_right = (cw as f64 * percent).round() as i32;
        if warning {
            let mut measure = RECT::default();
            DrawTextW(mem, &mut wtext, &mut measure, DT_CALCRECT | DT_SINGLELINE);
            let text_left = paused_text_left + (cw - paused_text_left - measure.right).max(0) / 2;
            fill_right = fill_right
                .max(text_left + measure.right + scale_px(ICON_MARGIN, dpi))
                .min(cw);
        }
        let fill_brush = CreateSolidBrush(colorref(fill));
        let fill_rect = RECT {
            left: 0,
            top: 0,
            right: fill_right,
            bottom: ch,
        };
        FillRect(mem, &fill_rect, fill_brush);
        let _ = DeleteObject(fill_brush.into());

        // 前景色（文字与时钟图标**共用同一守卫**，一帧只有一套色彩系统——
        // 一致性差异 #6 关闭）：暂停时 from→to 渐变（Q8）；休息文案不吃渐变，
        // 连同图标一起回 idle（暂停+休息帧 = 单色 idle，无同帧双色）。
        // 渐变分支的底已由 2) 保证是同一 t 的 ramp 场（文字色与其底配对）
        let fg = match paused_secs {
            Some(s) if !bar.resting => theme::ramp(rgb(skin.text_from), rgb(skin.text_to), s),
            _ => rgb(skin.text_idle),
        };

        // 3) 暂停态时钟图标（左侧 ⌀13 = 字高：表圈 + 两针，等价 egui paint_clock_icon；
        //    Foreground 绑 txt.Foreground = 同一前景守卫，B9。装饰不再大于主数据文字）
        if paused.is_some() {
            let r = scale_px(ICON_RADIUS, dpi);
            let cx = scale_px(ICON_MARGIN, dpi) + r; // XAML Margin="4,0,0,0"
            let cy = ch / 2;
            let old_brush = SelectObject(mem, GetStockObject(NULL_BRUSH));
            // DPI 描边统一（DS3 §5.6-1）：与复选框同式 96dpi→1px、200%→2px，
            // 删除时钟特例 dpi/48（150%→3px、200%→4px 的失衡增粗）
            let pen = CreatePen(PS_SOLID, 1i32.max((dpi / 96) as i32), colorref(fg));
            let old_pen = SelectObject(mem, pen.into());
            let _ = Ellipse(mem, cx - r, cy - r, cx + r, cy + r);
            let _ = MoveToEx(mem, cx, cy, None);
            let _ = LineTo(mem, cx + r / 2, cy + r / 2); // 时针（4 点方向）
            let _ = MoveToEx(mem, cx, cy, None);
            let _ = LineTo(mem, cx, cy - r * 3 / 4); // 分针（12 点方向）
            SelectObject(mem, old_pen);
            SelectObject(mem, old_brush);
            let _ = DeleteObject(pen.into());
        }

        // 4) 文字：休息文案 / PausedTime / mm:ss（B2/B10/D6）。
        //    暂停构图（DS3 §5.6-5）：图标让位——文字矩形 = 图标右缘 + TEXT_ICON_GAP
        //    到右缘，在剩余空间内居中（v2 全宽 DT_CENTER 把 mm:ss 压向图标、重心
        //    偏右的构图废除）；常态/休息无图标，仍全宽居中。
        //    （文案与字体已在 2) 前构建/选入，measure 先行）
        SetTextColor(mem, colorref(fg));
        let mut text_rect = rc;
        if paused.is_some() {
            text_rect.left = paused_text_left;
        }
        DrawTextW(
            mem,
            &mut wtext,
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // 5) 置顶图钉（命中区 CHECK_LOGICAL=19 不变；悬停浮现 + 0.7s 宽限后隐藏，B4。
        //    0.3s 淡入淡出为观感增强项，先交付硬显隐，§6.2/§13）
        if state.check_shown {
            let side = scale_px(CHECK_SIDE, dpi); // 16：20px 条身上下各留 2px 米边
            let zx = cw - scale_px(CHECK_LOGICAL, dpi);
            let cy = ch / 2;
            if state.check_hovered {
                // 悬停底色（经典皮肤 = #FFFFFF，v2 CheckBox.xaml 模板 MouseOver：仅图钉上悬停；
                // 块高 16 不再贴满 20px 条身——上下米边让白块成为「状态」而非「补丁」）
                let check_bg = CreateSolidBrush(COLORREF(skin.check_hover_bg));
                let hover_rect = RECT {
                    left: zx + HAIRLINE_PX,
                    top: cy - side / 2,
                    right: cw - HAIRLINE_PX,
                    bottom: cy + side / 2,
                };
                FillRect(mem, &hover_rect, check_bg);
                let _ = DeleteObject(check_bg.into());
            }
            // 图钉符号（2026-10 用户需求，取代 v2 12×12 方框+勾自绘）：未置顶直立
            // 描边 / 置顶 45° 斜插实心；几何以 CHECK_BOX=12 逻辑盒为基准，命中区/
            // 悬停底色/tooltip 交互契约不变（§5.6-6）。色 = check_glyph，DPI 描边
            // 统一（DS3 §5.6-1）与时钟同式
            let cx = zx + scale_px(CHECK_INSET, dpi) + scale_px(CHECK_BOX, dpi) / 2;
            draw_pin(mem, cx, cy, dpi, bar.topmost, COLORREF(skin.check_glyph));
        }

        // 6) 1px 收边描边（DS3 §5.6-4）：底色逐通道 ×0.9（整除截断；classic 米色
        //    #F5F5DC → #DCDCC6）——相近色系壁纸上给出分离线，近似色皮肤同吃此规则
        //    （按运行时底色推导而非写死 #DCDCC6）。Win11 圆角由 DWM 裁掉方角描边。
        let stroke = rgb(skin.progress_bg);
        // ×0.9 必须在宽整型里算再截回 u8：245u8*9 直接乘会溢出（debug 下 panic，
        // wndproc 是 extern "system" → 不可展开 → abort/0xc0000409，冒烟教训）
        let darken = |c: u8| ((c as u16 * 9 / 10) as u8);
        let stroke_dark = theme::Rgb::new(darken(stroke.r), darken(stroke.g), darken(stroke.b));
        let edge_brush = CreateSolidBrush(colorref(stroke_dark));
        let _ = FrameRect(mem, &rc, edge_brush);
        let _ = DeleteObject(edge_brush.into());

        // 一次性上屏，释放离屏资源
        let _ = BitBlt(hdc, 0, 0, cw, ch, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old_font);
        SelectObject(mem, old_bmp);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        let _ = EndPaint(hwnd, &ps);
    }
}

/// 置顶图钉符号（2026-10，取代 v2「12×12 方框+勾」自绘）：圆头 + 收尖针体的图钉。
/// `pinned`=false 直立描边（未钉），true 45° 斜插实心（钉上的通用表意，针尖左下、
/// 头部右上）。几何以 CHECK_BOX=12 逻辑盒为基准（原点 = 盒中心，y 向下），经 DPI
/// 整数化；颜色由调用方传入（skin.check_glyph），描边宽同 §5.6-1 统一式。
#[cfg(target_os = "windows")]
fn draw_pin(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    cx: i32,
    cy: i32,
    dpi: u32,
    pinned: bool,
    color: windows::Win32::Foundation::COLORREF,
) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        CreatePen, CreateSolidBrush, DeleteObject, Ellipse, GetStockObject, LineTo, MoveToEx,
        Polygon, SelectObject, NULL_BRUSH, PS_SOLID,
    };

    // 局部几何（逻辑 px）：头部圆 ⌀6（= 半个盒高，与暂停时钟同「装饰不过主数据」
    // 的层级克制）、针体基线 y=-0.9 半宽 1.2（基线取头部圆内——半径 3 圆在 x=±1.2
    // 处边缘 y≈-0.25，接缝由后画的头部覆盖）、针尖 y=6。总高 12 = CHECK_BOX。
    const HEAD_R: f64 = 3.0;
    const HEAD_CY: f64 = -3.0;
    const BASE_Y: f64 = -0.9;
    const BASE_HW: f64 = 1.2;
    const TIP_Y: f64 = 6.0;

    // 屏幕坐标 y 向下，正角旋转矩阵视觉为顺时针：45° 时针尖 (0,6) → 左下、
    // 头部 (0,-3) → 右上（未置顶不旋转，直立表示「可钉」）
    let (sn, cs) = if pinned {
        (
            core::f64::consts::FRAC_1_SQRT_2,
            core::f64::consts::FRAC_1_SQRT_2,
        )
    } else {
        (0.0, 1.0)
    };
    let k = dpi as f64 / 96.0;
    let px = |x: f64, y: f64| POINT {
        x: cx + ((x * cs - y * sn) * k).round() as i32,
        y: cy + ((x * sn + y * cs) * k).round() as i32,
    };
    let head = px(0.0, HEAD_CY);
    let tip = px(0.0, TIP_Y);
    let b1 = px(-BASE_HW, BASE_Y);
    let b2 = px(BASE_HW, BASE_Y);

    // SAFETY：GDI 对象在本函数内配对释放；坐标均为调用方离屏 DC 的客户区像素
    unsafe {
        let stroke = 1i32.max((dpi / 96) as i32);
        let pen = CreatePen(PS_SOLID, stroke, color);
        let old_pen = SelectObject(hdc, pen.into());
        let brush = CreateSolidBrush(color); // 仅实心分支真正选入
        let old_brush = if pinned {
            SelectObject(hdc, brush.into())
        } else {
            SelectObject(hdc, GetStockObject(NULL_BRUSH))
        };
        // 针体先画：实心三角 / 两根描边线（描边不画基线，接缝让头部吸收）
        if pinned {
            let tri = [b1, b2, tip];
            let _ = Polygon(hdc, &tri);
        } else {
            let _ = MoveToEx(hdc, b1.x, b1.y, None);
            let _ = LineTo(hdc, tip.x, tip.y);
            let _ = MoveToEx(hdc, b2.x, b2.y, None);
            let _ = LineTo(hdc, tip.x, tip.y);
        }
        // 头部后画：盖住针根接缝（实心圆 / 描边圆环）
        let r = (HEAD_R * k).round() as i32;
        let _ = Ellipse(hdc, head.x - r, head.y - r, head.x + r, head.y + r);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(pen.into());
        let _ = DeleteObject(brush.into());
    }
}

/// 置顶样式 + z-band 同步切换（WM_APP_TOPMOST 与 WM_APP_SHOW 共用；§6.3-1：
/// 样式位与 SetWindowPos 必须成对改——只改样式不调 SetWindowPos 不即时生效）
#[cfg(target_os = "windows")]
fn apply_topmost(hwnd: windows::Win32::Foundation::HWND, on: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_NOTOPMOST,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_TOPMOST,
    };
    // SAFETY：仅改本进程窗口的扩展样式位与 z 序
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_ex = if on {
            ex | WS_EX_TOPMOST.0 as isize
        } else {
            ex & !(WS_EX_TOPMOST.0 as isize)
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex);
        let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
        let _ = SetWindowPos(
            hwnd,
            Some(after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// 消息过程（§6.3 三条实测教训的落地机制）
#[cfg(target_os = "windows")]
unsafe extern "system" fn bar_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    w: windows::Win32::Foundation::WPARAM,
    l: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, RECT};
    use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, GetClientRect,
        GetWindowLongPtrW, GetWindowRect, KillTimer, PostQuitMessage,
        SendMessageW, SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow,
        GWLP_USERDATA, HTCAPTION, HTCLIENT, SWP_NOACTIVATE, SWP_NOSIZE,
        SWP_NOZORDER, SW_SHOW, WM_CONTEXTMENU,
        WM_DPICHANGED, WM_EXITSIZEMOVE, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCLBUTTONDBLCLK,
        WM_NCMOUSELEAVE,
        WM_NCMOUSEMOVE, WM_NCDESTROY, WM_NCHITTEST, WM_NCRBUTTONUP, WM_PAINT, WM_TIMER,
    };

    // WM_MOUSELEAVE（0x02A3）：windows crate 把它放在 Win32::UI::Controls（会拖入
    // 整个 Controls feature），值是稳定的 Win32 常量，就地定义
    const WM_MOUSELEAVE: u32 = 0x02A3;

    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut BarState;
    // WM_NCCREATE 之前 GWLP_USERDATA 尚未设置
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let state = &mut *state_ptr;

    match msg {
        WM_PAINT => {
            on_paint(hwnd, state);
            LRESULT(0)
        }
        WM_TIMER => {
            match w.0 {
                TIMER_PAINT => {
                    // 1s 重绘（快照由 CoreLoop 更新）
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                TIMER_GRACE => {
                    // 0.7s 宽限到点：宽限内的再进入已在 WM_MOUSEMOVE 里 KillTimer，
                    // 走到这里说明鼠标确实已离开 → 隐藏复选（B4 退场）
                    let _ = KillTimer(Some(hwnd), TIMER_GRACE);
                    state.check_shown = false;
                    state.check_hovered = false;
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE | WM_NCMOUSEMOVE => {
            // 进入条内 / 在条内移动：浮现复选（B4）。条身 HTCAPTION → 多数区域为
            // 非客户区，WM_MOUSEMOVE 只在复选命中区（HTCLIENT）到达，两者都要收；
            // 离开跟踪是单次的，进入即重挂。宽限定时器在任何移动时取消（再进入
            // 即取消隐藏）。
            let new_hovered = if msg == WM_MOUSEMOVE {
                let pt = POINT {
                    x: (l.0 as i16) as i32,
                    y: ((l.0 >> 16) as i16) as i32,
                };
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                in_check_zone(pt.x, rc.right, state.dpi)
            } else {
                false // 非客户区必然不在复选命中区
            };
            let changed = !state.check_shown || state.check_hovered != new_hovered;
            state.check_shown = true;
            state.check_hovered = new_hovered;
            let _ = KillTimer(Some(hwnd), TIMER_GRACE);
            if !state.tracking {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                if TrackMouseEvent(&mut tme).is_ok() {
                    state.tracking = true;
                }
            }
            if changed {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE | WM_NCMOUSELEAVE => {
            // 单次离开跟踪已消费（客户区→非客户区的过渡也会触发其一，0.7s 宽限
            // 吸收该瞬态；再进入会重挂跟踪并取消宽限）
            state.tracking = false;
            SetTimer(Some(hwnd), TIMER_GRACE, GRACE_MS, None);
            LRESULT(0)
        }
        WM_NCHITTEST => {
            // 教训 1：拖动 = 系统拖动——命中本窗口且不在复选命中区 → HTCAPTION，
            // 系统接管移动循环（含贴边/无跳动），代码不做任何逐帧 SetWindowPos
            let def = DefWindowProcW(hwnd, msg, w, l);
            if def.0 as u32 == HTCLIENT {
                let mut pt = POINT {
                    x: (l.0 as i16) as i32,
                    y: ((l.0 >> 16) as i16) as i32,
                };
                let _ = ScreenToClient(hwnd, &mut pt);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                if in_check_zone(pt.x, rc.right, state.dpi) {
                    def
                } else {
                    LRESULT(HTCAPTION as isize)
                }
            } else {
                def
            }
        }
        WM_LBUTTONUP => {
            // 复选点击：勾选即写 UI.json + 切换置顶（对齐 v2 TopMost_Checked/Unchecked，B5）
            let pt = POINT {
                x: (l.0 as i16) as i32,
                y: ((l.0 >> 16) as i16) as i32,
            };
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            if in_check_zone(pt.x, rc.right, state.dpi) {
                let topmost = snapshot_clone(state).topmost;
                let _ = state.cmd_tx.send(CoreMsg::ToggleTopmost(!topmost));
            }
            LRESULT(0)
        }
        WM_NCLBUTTONDBLCLK => {
            // 双击浮窗打开主界面（用户需求 2026-10）：条身 HTCAPTION，双击与真实
            // 标题栏同机制以非客户区双击消息到达。必须拦下 DefWindowProc 的默认
            // 标题栏双击行为（最大化/最小化），转托盘同款动作打开设置窗；
            // 复选命中区是 HTCLIENT，双击不受此影响（仍走两次 WM_LBUTTONUP 翻转）
            let _ = state.cmd_tx.send(CoreMsg::Tray(TrayAction::Settings));
            LRESULT(0)
        }
        WM_EXITSIZEMOVE => {
            // 教训 1 落地：拖动结束 → GetWindowRect（物理）÷ GetDpiForWindow（逻辑）
            // → SaveBarPosition → CoreLoop 写 UI.json 并保存（Q9：拖动结束即存）
            let mut rc = RECT::default();
            if GetWindowRect(hwnd, &mut rc).is_ok() {
                let dpi = GetDpiForWindow(hwnd).max(96);
                let _ = state.cmd_tx.send(CoreMsg::SaveBarPosition {
                    left: rc.left as f64 * 96.0 / dpi as f64,
                    top: rc.top as f64 * 96.0 / dpi as f64,
                });
            }
            LRESULT(0)
        }
        // 右键菜单两路并接：条身 HTCAPTION，右键抬起落在**非客户区**，DefWindowProc
        // 不会为标题栏区域的 WM_NCRBUTTONUP 生成 WM_CONTEXTMENU（菜单“消失”的根因），
        // 故直连 WM_NCRBUTTONUP；WM_CONTEXTMENU 仍保留，覆盖 Shift+F10 / 键盘唤起
        WM_CONTEXTMENU | WM_NCRBUTTONUP => show_bar_menu(hwnd, state, l),
        m if m == WM_APP_TOPMOST => {
            // 置顶切换由 CoreLoop 经消息驱动（§6.3-1）；样式与 SetWindowPos 同步改
            apply_topmost(hwnd, w.0 == 1);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        m if m == WM_APP_SKIN => {
            // 换肤（v3.1）：配色已在共享快照（CoreLoop 先更新再投递），只需即时重绘
            //（1s 心跳本也会覆盖，本消息保证切换零延迟）
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        m if m == WM_APP_LANGUAGE => {
            // 文案已在共享快照（CoreLoop 先更新再投递）：重绘 + tooltip 文案热更新
            let texts = snapshot_clone(state).texts;
            write_utf16_z(&mut state.tooltip_text, &texts.always_on_top);
            if state.tooltip != 0 {
                let mut ti = ToolInfoW {
                    cb_size: std::mem::size_of::<ToolInfoW>() as u32,
                    u_flags: TTF_SUBCLASS,
                    hwnd,
                    u_id: TOOLTIP_TOOL_ID,
                    rect: RECT::default(),
                    hinst: std::ptr::null_mut(),
                    lpsz_text: state.tooltip_text.as_mut_ptr(),
                    l_param: 0,
                    lp_reserved: std::ptr::null_mut(),
                };
                let _ = SendMessageW(
                    windows::Win32::Foundation::HWND(state.tooltip as _),
                    TTM_UPDATETIPTEXTW,
                    None,
                    Some(LPARAM(&mut ti as *mut ToolInfoW as isize)),
                );
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        m if m == WM_APP_SHOW => {
            // 托盘双击（B11）：对齐 v2 ShowCountdownWindow（EyeNurseViewModel.cs:148-160）
            // ——每次 Show 前取持久化 UI 配置、按其 TopMost/位置恢复显示：关掉置顶的
            // 用户双击后**不**置顶（旧实现无条件 HWND_TOPMOST 且不回放位置，评审修订）。
            // 值由 CoreLoop 在转发前写入共享快照（单写者：进程内不直读盘——UI.json
            // 写盘非原子，并发读可能撕裂，§3 评审修订）
            let bar = snapshot_clone(state);
            apply_topmost(hwnd, bar.topmost);
            // 位置回放（v2 Left/Top 赋值）：逻辑 DIP → 物理像素，换算基准 = 保存点
            // 所在显示器 DPI（§6.1，与建窗路径同源）
            let dpi = dpi_at_point(bar.saved_left, bar.saved_top).max(96);
            let _ = SetWindowPos(
                hwnd,
                None,
                scale_px(bar.saved_left, dpi),
                scale_px(bar.saved_top, dpi),
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
            );
            let _ = ShowWindow(hwnd, SW_SHOW);
            // v2 `if (!IsActive) Activate()` 的 Win32 等价：非置顶条被其他窗压住时，
            // 双击后必须浮到前台才算「显示悬浮条」
            let _ = SetForegroundWindow(hwnd);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            // §6.1：按建议矩形重设（含位置——跨屏拖动时建议矩形把窗口锚在光标下）
            // + 重建字体 + tooltip 工具区更新 + Invalidate
            let suggested = l.0 as *const RECT;
            if !suggested.is_null() {
                let r = &*suggested;
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            let old_font = state.font;
            state.dpi = GetDpiForWindow(hwnd).max(96);
            state.font = create_bar_font(state.dpi);
            if old_font != 0 {
                use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
                let _ = DeleteObject(HGDIOBJ(old_font as _));
            }
            if state.tooltip != 0 {
                let mut ti = ToolInfoW {
                    cb_size: std::mem::size_of::<ToolInfoW>() as u32,
                    u_flags: TTF_SUBCLASS,
                    hwnd,
                    u_id: TOOLTIP_TOOL_ID,
                    rect: check_zone_rect(hwnd, state.dpi),
                    hinst: std::ptr::null_mut(),
                    lpsz_text: std::ptr::null_mut(),
                    l_param: 0,
                    lp_reserved: std::ptr::null_mut(),
                };
                let _ = SendMessageW(
                    windows::Win32::Foundation::HWND(state.tooltip as _),
                    TTM_NEWTOOLRECTW,
                    None,
                    Some(LPARAM(&mut ti as *mut ToolInfoW as isize)),
                );
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // 先摘 tooltip（其 TTF_SUBCLASS 对本窗口的子类化随销毁还原），再释放
            // 其余资源，退出消息循环
            if state.tooltip != 0 {
                let _ = DestroyWindow(windows::Win32::Foundation::HWND(state.tooltip as _));
            }
            if state.font != 0 {
                use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
                let _ = DeleteObject(HGDIOBJ(state.font as _));
            }
            let _ = KillTimer(Some(hwnd), TIMER_PAINT);
            let _ = KillTimer(Some(hwnd), TIMER_GRACE);
            drop(Box::from_raw(state_ptr));
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

/// `mm:ss`（对齐 v2 StringFormat=mm\:ss）。
/// `mm` 是 TimeSpan 的**分钟分量（00-59）**而非总分钟：剩余 >59min 时回绕显示
/// （90min 间隔首帧显示 "30:00"，清单 B2/Q13 决议保留）。
pub fn format_mmss(d: Duration) -> String {
    let total = d.as_secs();
    format!("{:02}:{:02}", (total / 60) % 60, total % 60)
}

#[cfg(all(test, target_os = "windows"))]
mod pin_tests {
    use super::*;
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
        HBITMAP, DIB_RGB_COLORS, SRCCOPY,
    };

    /// 离屏 32bpp DIB 上渲染图钉并返回像素（确定性地验证两种状态的几何，
    /// 不依赖活窗口/真实光标——活体探测会被 TrackMouseEvent 的立即离开 +
    /// 0.7s 宽限隐藏搅乱，见 2026-10 图钉验证记录）
    fn render_pin(pinned: bool) -> Vec<u32> {
        const W: i32 = 48;
        const H: i32 = 48;
        unsafe {
            let mem = CreateCompatibleDC(None);
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmi = windows::Win32::Graphics::Gdi::BITMAPINFO {
                bmiHeader: windows::Win32::Graphics::Gdi::BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::BITMAPINFOHEADER>()
                        as u32,
                    biWidth: W,
                    biHeight: -H, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: DIB_RGB_COLORS.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let bmp: HBITMAP = CreateDIBSection(
                None,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )
            .unwrap();
            let old = SelectObject(mem, bmp.into());
            // 底刷白（默认黑，难以区分描边/填充的失败形态）
            let whitewash = windows::Win32::Graphics::Gdi::CreateSolidBrush(COLORREF(0x00FF_FF_FF));
            let rc = windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: W,
                bottom: H,
            };
            let _ = windows::Win32::Graphics::Gdi::FillRect(mem, &rc, whitewash);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(whitewash.into());

            draw_pin(mem, W / 2, H / 2, 192, pinned, COLORREF(0x00444444));

            let _ = BitBlt(mem, 0, 0, W, H, Some(mem), 0, 0, SRCCOPY); // no-op，保持对称
            let mut px = vec![0u32; (W * H) as usize];
            std::ptr::copy_nonoverlapping(bits as *const u32, px.as_mut_ptr(), px.len());
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            px
        }
    }

    fn is_dark(p: u32) -> bool {
        let (r, g, b) = (p & 0xFF, (p >> 8) & 0xFF, (p >> 16) & 0xFF);
        r < 120 && g < 120 && b < 120
    }

    fn grid(px: &[u32]) -> String {
        const W: usize = 48;
        let mut s = String::new();
        for y in 6..42 {
            for x in 12..36 {
                s.push(if is_dark(px[y * W + x]) { '#' } else { '.' });
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn pin_unpinned_is_upright_outline() {
        let px = render_pin(false);
        println!("未置顶（直立描边）:\n{}", grid(&px));
        // 48×48 画布，原点 (24,24)，200% DPI：头部圆心 (24,18) 半径 6、针尖 (24,36)
        let at = |x: i32, y: i32| px[(y * 48 + x) as usize];
        // 头部空芯：圆心 = 白底，描边环 = 墨色（GDI Ellipse 右/下排他：环底在 23）
        assert!(!is_dark(at(24, 18)), "未置顶头部应空芯（描边圆环）");
        assert!(is_dark(at(24, 12)), "头顶描边缺失");
        assert!(is_dark(at(24, 23)), "头底描边缺失");
        // 直立针体：中轴线上应有墨（针身/针尖），两侧让空
        assert!(is_dark(at(24, 32)), "直立针尖缺失");
        assert!(!is_dark(at(20, 32)) && !is_dark(at(28, 32)), "针体过宽");
    }

    #[test]
    fn pin_pinned_is_tilted_solid() {
        let px = render_pin(true);
        println!("置顶（45° 斜插实心）:\n{}", grid(&px));
        let at = |x: i32, y: i32| px[(y * 48 + x) as usize];
        // 头部圆心旋至 (28,20) 附近且实心
        assert!(is_dark(at(28, 20)), "置顶头部应实心（斜 45°）");
        // 针尖旋至 (16,32) 附近（局部 (0,6)×45°×2dpi = (∓8,±8)）
        assert!(is_dark(at(16, 32)), "斜插针尖缺失");
        // 直立路径不再有针：中轴下段 (24,34) 应为底色
        assert!(!is_dark(at(24, 36)), "置顶时中轴不应残留直立针体");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
