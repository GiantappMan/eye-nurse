//! HKCU Run 自启（架构文档 §4.4，对齐外部库 DesktopStartupHelper.cs，附录 §11）。
//!
//! v2 外部库行为：
//! - 键：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`；
//! - 写：`RegSetValueExW` REG_SZ，exe 完整路径（不带引号）；
//! - 删：`RegDeleteValueW`；
//! - 查：遍历值名做 `OrdinalIgnoreCase` 比较（注册表值名本身大小写不敏感，
//!   直接 `RegQueryValueExW` 与之等价，见 check() 注释）。

/// 自启句柄：值名 + exe 完整路径
pub struct DesktopStartup {
    key_name: String,
    exe_path: String,
}

impl DesktopStartup {
    /// v2 实例化参数：("Eye Nurse 2", <exe 完整路径>)（EyeNurseService.cs:34）
    /// v3 默认 key_name = "Eye Nurse 3"；v2 的旧键由 `migrate_from` 静默迁移（清单 Q6/G2）
    pub fn new(key_name: impl Into<String>, exe_path: impl Into<String>) -> Self {
        Self {
            key_name: key_name.into(),
            exe_path: exe_path.into(),
        }
    }

    /// HKCU Run 下是否存在同名值（忽略大小写）
    pub fn check(&self) -> bool {
        imp::value_exists(&self.key_name)
    }

    /// true=写值（REG_SZ，exe 完整路径，不带引号——保持 v2 形态），false=删值；
    /// 打开失败返回 false
    pub fn set_enabled(&self, enabled: bool) -> bool {
        if enabled {
            imp::set_value(&self.key_name, &self.exe_path)
        } else {
            imp::delete_value(&self.key_name)
        }
    }

    /// 自启值名升级的**静默迁移**（清单 Q6/G2，无任何 UI 提示）：
    /// 本键缺失且 legacy_key 存在 → 写本键 + 删 legacy_key，返回是否发生迁移。
    /// 落点：main.rs 启动序列第 2.5 步调用 `startup.migrate_from("Eye Nurse 2")`。
    pub fn migrate_from(&self, legacy_key: &str) -> bool {
        if self.check() || !imp::value_exists(legacy_key) {
            return false;
        }
        // 迁移 = 用旧键名对应的新值名写 exe 路径，再删旧键
        if imp::set_value(&self.key_name, &self.exe_path) {
            imp::delete_value(legacy_key);
            true
        } else {
            false
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
    };

    const RUN_SUBKEY: PCWSTR = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

    /// 打开 Run 键；失败返回 None
    fn open_key(writable: bool) -> Option<HKEY> {
        let mut hkey = HKEY::default();
        let access = if writable {
            KEY_SET_VALUE | KEY_QUERY_VALUE
        } else {
            KEY_QUERY_VALUE
        };
        // SAFETY：句柄输出参数由本函数独占；调用方负责 RegCloseKey
        let err = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_SUBKEY, Some(0), access, &mut hkey) };
        if err == ERROR_SUCCESS {
            Some(hkey)
        } else {
            None
        }
    }

    fn close(hkey: HKEY) {
        unsafe {
            let _ = RegCloseKey(hkey);
        }
    }

    /// 值是否存在。v2 是遍历值名做 OrdinalIgnoreCase 比较；注册表值名本身
    /// 大小写不敏感但保留大小写，直接查询与之等价。
    pub fn value_exists(name: &str) -> bool {
        let Some(hkey) = open_key(false) else {
            return false;
        };
        let wide = HSTRING::from(name);
        // SAFETY：wide 活跃于调用期间；输出参数全部 None（只判断存在性）
        let err: WIN32_ERROR = unsafe {
            RegQueryValueExW(hkey, PCWSTR(wide.as_ptr()), None, None, None, None)
        };
        close(hkey);
        err == ERROR_SUCCESS
    }

    /// 写 REG_SZ（exe 完整路径，不带引号；UTF-16 + 终止符）
    pub fn set_value(name: &str, data: &str) -> bool {
        let Some(hkey) = open_key(true) else {
            return false;
        };
        let wide = HSTRING::from(name);
        let mut value: Vec<u16> = data.encode_utf16().collect();
        value.push(0); // REG_SZ 以空终止符结尾
        // UTF-16LE 字节序列（小端机即原生布局），长度按字节数
        let bytes: Vec<u8> = value.iter().flat_map(|c| c.to_le_bytes()).collect();
        // SAFETY：bytes/wide 活跃于调用期间；切片长度即 cbdata
        let err = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(wide.as_ptr()),
                Some(0),
                REG_SZ,
                Some(bytes.as_slice()),
            )
        };
        close(hkey);
        err == ERROR_SUCCESS
    }

    /// 删值；`ERROR_FILE_NOT_FOUND` 视为成功（对齐“删到不存在即达标”）
    pub fn delete_value(name: &str) -> bool {
        let Some(hkey) = open_key(true) else {
            return false;
        };
        let wide = HSTRING::from(name);
        // SAFETY：wide 活跃于调用期间
        let err = unsafe { RegDeleteValueW(hkey, PCWSTR(wide.as_ptr())) };
        close(hkey);
        err == ERROR_SUCCESS || err == ERROR_FILE_NOT_FOUND
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    // 非 Windows 平台（CI/单测环境）：全部返回 false/无操作，保持签名编译通过
    pub fn value_exists(_name: &str) -> bool {
        false
    }
    pub fn set_value(_name: &str, _data: &str) -> bool {
        false
    }
    pub fn delete_value(_name: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_accepts_strings() {
        let s = DesktopStartup::new("Eye Nurse 3", "C:\\path\\eyenurse.exe");
        assert_eq!(std::mem::size_of_val(&s), 2 * std::mem::size_of::<String>());
    }
}
