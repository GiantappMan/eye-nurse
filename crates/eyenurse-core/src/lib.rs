//! EyeNurse v3 核心库（实现者 A 所有）。
//!
//! 纯逻辑：不依赖 egui/窗口；没有线程、没有时钟读取（`Instant`/系统时间一律由
//! 调用方作为参数传入）；全部方法同步、可单测。详见 docs/v3-architecture.md §4。
//!
//! 例外：`autostart` 与 `config::ConfigStore::v2_default` 只依赖注册表 / KnownFolder
//! API，不涉及窗口。

#![allow(non_snake_case)] // 配置字段名与 v2 磁盘格式一致（PascalCase），不许改名

pub mod autostart;
pub mod config;
pub mod i18n;
pub mod skin;
pub mod stats;
pub mod time;
pub mod timer;

// 公共 re-export：app 侧最常用的类型
pub use config::{ConfigFile, ConfigStore, LanguagesJson, LockScreenJson, SettingJson, StjF64, UiJson};
pub use i18n::I18n;
pub use skin::{SkinColors, SkinConfig, SkinDef, SkinMeta};
pub use time::HmsSpan;
pub use timer::{GlobalTimer, IntervalTooShort, TimerEvent};
