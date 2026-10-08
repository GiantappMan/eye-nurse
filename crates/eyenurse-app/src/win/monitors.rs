//! 显示器枚举（架构文档 §5.5，等价 v2 User32Ex.GetMonitorInfos()）。

/// 物理像素坐标
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRects {
    /// MONITORINFOEXW.szDevice，如 "\\.\DISPLAY1"
    pub device_name: String,
    /// 整屏 left,top,right,bottom
    pub rc_monitor: [i32; 4],
    /// 工作区（去掉任务栏）——v2 遮罩用这个（EyeNurseViewModel.cs:189-192）
    pub rc_work: [i32; 4],
}

/// 枚举全部显示器（EnumDisplayMonitors + GetMonitorInfoW(MONITORINFOEXW)）
#[cfg(target_os = "windows")]
pub fn get_monitor_infos() -> Vec<MonitorRects> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };

    // SAFETY：EnumDisplayMonitors 回调契约——lparam 由我方传入并在此还原
    unsafe extern "system" fn enum_cb(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let out = &mut *(lparam.0 as *mut Vec<MonitorRects>);
        // EXW 以基类指针传入，靠 cbSize 区分（架构文档 §5.5）
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        let ok = GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut MONITORINFO);
        if ok.as_bool() {
            let cut = |s: &[u16; 32]| {
                let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                String::from_utf16_lossy(&s[..end])
            };
            out.push(MonitorRects {
                device_name: cut(&info.szDevice),
                rc_monitor: [
                    info.monitorInfo.rcMonitor.left,
                    info.monitorInfo.rcMonitor.top,
                    info.monitorInfo.rcMonitor.right,
                    info.monitorInfo.rcMonitor.bottom,
                ],
                rc_work: [
                    info.monitorInfo.rcWork.left,
                    info.monitorInfo.rcWork.top,
                    info.monitorInfo.rcWork.right,
                    info.monitorInfo.rcWork.bottom,
                ],
            });
        }
        BOOL(1) // 继续枚举
    }

    let mut out: Vec<MonitorRects> = Vec::new();
    // SAFETY：回调与 lparam 指针均指向本栈帧存活对象
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_cb),
            LPARAM(&mut out as *mut Vec<MonitorRects> as isize),
        );
    }
    out
}

#[cfg(not(target_os = "windows"))]
pub fn get_monitor_infos() -> Vec<MonitorRects> {
    Vec::new()
}
