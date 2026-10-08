//! 单实例互斥体（架构文档 §5.1 第 1 步 / §7.5，清单 J3；v3 新增，v2 未见实现）。

/// 尝试持有 `Local\EyeNurse3` 互斥体；已有实例在运行 → false（调用方直接退出）。
#[cfg(target_os = "windows")]
pub fn acquire() -> bool {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    // SAFETY：互斥体由系统管理；句柄有意不关闭（进程退出时自动释放，单实例即常驻语义）
    unsafe {
        match CreateMutexW(None, false, windows::core::w!("Local\\EyeNurse3")) {
            Ok(_handle) => GetLastError() != ERROR_ALREADY_EXISTS,
            Err(_) => {
                // 创建失败（极少见）不阻塞启动，避免互斥体损坏时应用永远起不来
                true
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn acquire() -> bool {
    true
}
