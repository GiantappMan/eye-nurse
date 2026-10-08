//! 冒烟验证（实现者 B 所有，tauri 架构文档 §11 / 清单验收基线第 7 条——egui 版
//! 「设置窗真机打不开而纯代码评审没发现」的教训 3 制度化：三类窗口的创建路径
//! 必须可被程序化验证）。由 `--smoke` 启动旗标触发（main 解析），独立线程执行：
//!
//! 1. **settings**：静态声明的设置窗存在（tauri 在 setup 钩子前按 config 建、隐藏）
//!    → 走 §8.1 的 show 路径（show + unminimize + set_focus）→ 断言 `is_visible()`
//!    且 hwnd 非空 → `hide()`；
//! 2. **mask**：取首个显示器 rcWork → 走 §5.2 的**生产创建路径本体**
//!    （`core::build_mask_window`，与 CoreLoop 建遮罩同一函数）建 `mask0` → 断言
//!    窗口存在、`always_on_top` 生效、EXSTYLE 含 WS_EX_TOOLWINDOW → `destroy()`
//!    （与生产关闭路径一致，顺带验证 §5.3 的 destroy 语义）；
//! 3. **bar**：等待 bar 线程回填 hwnd 后，`FindWindowW("EyeNurse3BarWnd")` 非空 →
//!    断言可见、尺寸 = 80×20×DPI（±1px）。
//!
//! 全部通过打印 `SMOKE settings=ok mask=ok bar=ok` 并以退出码 0 退出；任一失败该
//! 项输出 `fail(窗口名: 原因)` 且退出码非 0（`bun run smoke` 的验收形态）。
//! 冒烟进程以 `std::process::exit` 直退（自测进程无需走 run 返回后的收尾）；
//! 不改变用户环境——单实例互斥与自启注册表写入已在 main 里跳过。

use crate::core::build_mask_window;
use crate::ui::bar::{self, BAR_CLASS_NAME};
use crate::win::monitors;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{Manager, Wry};

/// 单项等待上限（webview 建窗 / bar 线程建窗均为异步，轮询至多 10s）
const WINDOW_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 启动冒烟线程（main 的 setup 钩子末尾调用；结果与退出码由线程自行落地）
pub fn run(app: tauri::AppHandle<Wry>, bar_hwnd: Arc<OnceLock<isize>>) {
    std::thread::Builder::new()
        .name("eyenurse-smoke".into())
        .spawn(move || {
            let settings = check_settings(&app);
            let mask = check_mask(&app);
            let bar = check_bar(&bar_hwnd);
            let fmt = |name: &str, r: &Result<(), String>| match r {
                Ok(()) => "ok".to_string(),
                Err(e) => format!("fail({name}: {e})"),
            };
            println!(
                "SMOKE settings={} mask={} bar={}",
                fmt("settings", &settings),
                fmt("mask", &mask),
                fmt("bar", &bar),
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
            // 自测进程直退（§11：结果以退出码表达，不走正常收尾路径）
            std::process::exit(if settings.is_ok() && mask.is_ok() && bar.is_ok() {
                0
            } else {
                1
            });
        })
        .expect("冒烟线程创建失败");
}

/// §11-1：settings 存在 → §8.1 show 路径 → 可见 && hwnd 非空 → hide
fn check_settings(app: &tauri::AppHandle<Wry>) -> Result<(), String> {
    let deadline = Instant::now() + WINDOW_TIMEOUT;
    let w = loop {
        if let Some(w) = app.get_webview_window("settings") {
            break w;
        }
        if Instant::now() >= deadline {
            return Err("窗口不存在（tauri.conf.json 静态声明未生效）".into());
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    // §8.1 show 路径（与 CoreLoop::show_settings 同款三连）
    let _ = w.unminimize();
    w.show().map_err(|e| format!("show 失败: {e}"))?;
    let _ = w.set_focus();
    if !w.is_visible().map_err(|e| format!("is_visible 失败: {e}"))? {
        return Err("show 后 is_visible()=false".into());
    }
    let hwnd = w.hwnd().map_err(|e| format!("hwnd 获取失败: {e}"))?;
    if hwnd.0.is_null() {
        return Err("hwnd 为空".into());
    }
    let _ = w.hide();
    Ok(())
}

/// §11-2：主屏 rcWork → 生产创建路径建 mask0 → 置顶/工具窗口断言 → destroy
fn check_mask(app: &tauri::AppHandle<Wry>) -> Result<(), String> {
    let mon = monitors::get_monitor_infos()
        .into_iter()
        .next()
        .ok_or_else(|| "未枚举到显示器".to_string())?;
    let w = build_mask_window(app, "mask0", &mon).map_err(|e| format!("创建失败: {e}"))?;
    if app.get_webview_window("mask0").is_none() {
        return Err("建窗后按 label 查不到".into());
    }
    if !w
        .is_always_on_top()
        .map_err(|e| format!("is_always_on_top 失败: {e}"))?
    {
        return Err("always_on_top 未生效".into());
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
        };
        let hwnd = w.hwnd().map_err(|e| format!("hwnd 获取失败: {e}"))?;
        // SAFETY：只读本进程窗口的扩展样式位
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        if ex & WS_EX_TOOLWINDOW.0 as isize == 0 {
            return Err("EXSTYLE 缺 WS_EX_TOOLWINDOW（§5.4 补丁未打）".into());
        }
    }
    // 生产关闭路径同为 destroy（§5.3：close 会被 mask* 的 prevent_close 拦截）
    w.destroy().map_err(|e| format!("destroy 失败: {e}"))?;
    Ok(())
}

/// §11-3：FindWindowW(类名) 非空 → 可见 + 尺寸 = 80×20×DPI（±1px）
fn check_bar(bar_hwnd: &Arc<OnceLock<isize>>) -> Result<(), String> {
    // bar 线程建窗是异步的：先等 OnceLock 回填（bar_thread_main 建窗后即写），
    // 再按 §11 的断言形态按类名 FindWindow
    let deadline = Instant::now() + WINDOW_TIMEOUT;
    while bar_hwnd.get().is_none() {
        if Instant::now() >= deadline {
            return Err("bar 线程未在时限内回填窗口句柄".into());
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    #[cfg(target_os = "windows")]
    {
        check_bar_windows()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("bar 为 Win32 原生窗口，仅 Windows 可验证".into())
    }
}

#[cfg(target_os = "windows")]
fn check_bar_windows() -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, IsWindowVisible};

    let class = HSTRING::from(BAR_CLASS_NAME);
    let deadline = Instant::now() + WINDOW_TIMEOUT;
    let hwnd = loop {
        // SAFETY：按类名查询顶层窗口（标题 NULL = 任意）
        if let Ok(h) = unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) } {
            if !h.0.is_null() {
                break h;
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("FindWindowW(\"{BAR_CLASS_NAME}\") 超时为空"));
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    // SAFETY：以下均为本进程窗口的只读查询
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return Err("bar 窗口不可见".into());
        }
        let mut rc = RECT::default();
        GetWindowRect(hwnd, &mut rc).map_err(|e| format!("GetWindowRect 失败: {e}"))?;
        let dpi = GetDpiForWindow(hwnd).max(96);
        let (ew, eh) = bar::expected_size_px(dpi);
        let (aw, ah) = (rc.right - rc.left, rc.bottom - rc.top);
        if (aw - ew).abs() > 1 || (ah - eh).abs() > 1 {
            return Err(format!("尺寸 {aw}x{ah} ≠ 期望 {ew}x{eh}（DPI {dpi}）±1px"));
        }
    }
    Ok(())
}
