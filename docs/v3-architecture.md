# EyeNurse v3 架构设计（Rust Cargo workspace）

> 本文自包含：实现者只看本文 + `docs/v3-parity-checklist.md` + 已给出的模块骨架即可动手。
> 所有 v2 行为引用均标注 `文件:行号`（v2 代码在 `src/EyeNurse/`，当前工作树仍在）。
> 所有第三方 API 签名均为 2026-10-08 在 docs.rs / 源码仓库实际查证的结果（版本见 §2）。

---

## 0. 目标与非目标

**目标**
- 用 Rust 重写 EyeNurse v2（WPF + WebView2）为纯原生桌面应用：托盘 + 悬浮倒计时条 + 全屏休息遮罩 + 设置窗。
- **配置文件与 v2 完全双向兼容**（同路径、同字段、同序列化字节格式），v2 升级 v3 无感迁移。
- 行为与 v2 对齐，对齐清单见 `docs/v3-parity-checklist.md`；文档明确标注的**有意偏差**除外。

**非目标（v3 不做）**
- 不再内嵌 WebView2 / Vue 前端（v2 的 `MainWindow` + `src/eyenurse-client-ui` 被 egui 设置窗取代，`src/EyeNurse/Views/MainWindow.xaml:19` 的 `WebView2` 与 `Apis/ClientApi.cs` 的宿主桥接全部废弃）。
- 不做 v2 中只有 UI 入口但逻辑不存在/未生效的功能，除非 parity 清单明确要求。

---

## 1. Workspace 布局与文件所有权

```
eye-nurse/
├─ Cargo.toml                  # workspace 根
├─ docs/
│  ├─ v3-architecture.md       # 本文
│  └─ v3-parity-checklist.md   # 行为对齐清单（可核对）
├─ assets/
│  └─ logo.png                 # 从 src/EyeNurse/Assets/Img/logo.png 复制（托盘/窗口图标）
└─ crates/
   ├─ eyenurse-core/           # 【实现者 A 独占】纯逻辑，不依赖 egui/窗口
   │  ├─ Cargo.toml
   │  ├─ assets/languages/     # 【A】内嵌文案真源（core include_str!；D 打包直接从此目录拷贝，见 §4.5）
   │  │  ├─ zh.json
   │  │  └─ en.json
   │  └─ src/
   │     ├─ lib.rs             # 模块声明 + 公共 re-export
   │     ├─ time.rs            # HmsSpan（.NET TimeSpan 兼容格式）
   │     ├─ timer.rs           # GlobalTimer 状态机（v2 GlobalTimerService 的语义等价）
   │     ├─ config.rs          # ConfigStore + SettingJson/UIJson/LanguagesJson
   │     ├─ autostart.rs       # DesktopStartup（HKCU Run 自启）
   │     └─ i18n.rs            # I18n（加载 Assets/Languages/*.json）
   └─ eyenurse-app/            # 桌面壳
      ├─ Cargo.toml
      ├─ build.rs              # 【D】Windows 资源（exe 图标 .ico）
      ├─ app.manifest          # 【D】可选；见 §7.5 DPI 说明
      ├─ assets/               # 【D】构建期拷贝 logo.png、Languages/*.json 到 target
      └─ src/
         ├─ main.rs            # 【B】入口：单实例、事件线程、run_native
         ├─ app.rs             # 【B】AppModel 状态机 + eframe App 实现（logic/ui）
         ├─ events.rs          # 【B】WinEvent / UiCommand 通道定义
         ├─ tray.rs            # 【B】tray-icon 构建 + 菜单
         ├─ win/
         │  ├─ mod.rs          # 【B】
         │  ├─ monitors.rs     # 【B】EnumDisplayMonitors 封装
         │  ├─ session.rs      # 【B】WTS 会话锁专用消息窗口线程
         │  └─ single_instance.rs # 【B】CreateMutexW 单实例
         └─ ui/
            ├─ mod.rs          # 【B】纯接线（骨架期定稿后不再改）：调用 mask::maybe_show / settings::maybe_show
            ├─ bar.rs          # 【C】悬浮倒计时条（root 视口内容）
            ├─ mask.rs         # 【C】每显示器休息遮罩（内容 + 视口注册 maybe_show）
            ├─ theme.rs        # 【C】颜色/样式（v2 颜色常量表）
            ├─ fonts.rs        # 【C】CJK 字体加载
            └─ settings.rs     # 【D】设置窗（内容 + 视口注册 maybe_show + SettingsDraft 类型定义）
```

### 所有权规则（四人并行不相交）
| 实现者 | 独占文件 | 消费的冻结契约 |
|---|---|---|
| **A** | `crates/eyenurse-core/**`（含 `assets/languages/` 内嵌文案真源） | 无（core 是契约源头） |
| **B** | `main.rs`、`app.rs`、`events.rs`、`tray.rs`、`win/**`、`ui/mod.rs`（纯接线） | core 公共 API（A 定义） |
| **C** | `ui/{bar.rs,mask.rs,theme.rs,fonts.rs}` | `AppModel` 公共方法 + `SharedUi`（B 定义，§5.6） |
| **D** | `ui/settings.rs`（含 `SettingsDraft` 类型定义）、`build.rs`、`assets/**`、`crates/xtask/**`（可选打包脚本） | `AppModel` + `SharedUi`（B）、`ui::theme` 颜色常量（C）、字体由 B 预载（见下） |

- **冻结契约**：§4（core 全部签名）与 §5.6（AppModel 全部签名）在四人开工前定稿；修改契约只能由契约所有者改，并回写本文。
- 互相只 `use` 契约签名，不读对方实现；A、B、C、D 可同时编译通过（B 先提交骨架 `ui/mod.rs` 与 `app.rs` 的空实现，C/D 在骨架上填充）。
- **跨所有权类型**：`SettingsDraft` 定义在 D 的 `ui/settings.rs`；B 的 `app.rs` 只写一行 `use crate::ui::settings::SettingsDraft;` 并声明字段类型（骨架期一次写死）——D 之后增删字段**不改 B 的文件**。同理设置视口的注册逻辑（含 close_requested 处理）整体住在 D 的 `settings.rs::maybe_show`，C 的遮罩注册住在 C 的 `mask.rs::maybe_show`，`ui/mod.rs` 只保留两行调用。
- **字体（评审修订，补齐冻结契约）**：`ui/fonts.rs::load_cjk_fonts(ctx: &egui::Context)`（C 提供，签名冻结）；**由 B 在 `AppModel::new` 里首帧前调用一次**（§5.6）——D 的设置窗依赖它但**不得**自行调用，避免重复插入字体。D 的界面配色一律取 `ui::theme`（C）的常量，不自定义第二套色板。
- A 完全不依赖 Windows 窗口 API（autostart 例外：只依赖注册表，见 §4.4）；B/C/D 不改 core。

---

## 2. 依赖与版本选择（已查证）

| crate | 版本 | 说明 |
|---|---|---|
| eframe / egui | **0.36.2**（2026-09-08 发布，MSRV 1.95） | 多视口（deferred viewports）内建；Windows 上 `embed_viewports` 自动为 `false`（egui 源码 `context.rs:4001` 注释："eframe sets this to false on supported platforms, but the default value is true"） |
| winit（eframe 0.36.2 的工作区依赖） | 0.30.13 | `skip_taskbar` 用 ITaskbarList::DeleteTab 实现（winit `window.rs:1498`），**不等于** WS_EX_TOOLWINDOW，Alt-Tab 仍可见 → 见 §7.2 手动补 WS_EX_TOOLWINDOW |
| tray-icon | **0.26.1** | 自带 `menu`（muda 0.21.1 re-export）；跨线程事件走全局 crossbeam channel |
| windows | **0.62.2** | 只开下述 feature，最小化编译量 |
| serde / serde_json | 1.x | `serde_json::to_string`（非 pretty）与 System.Text.Json 默认输出同为紧凑格式 |
| image | 0.25 | 解码 logo.png → `tray_icon::Icon` + `egui::IconData` |
| crossbeam-channel | 0.5 | tray 与 Win32 线程 → 主循环事件 |
| sys-locale | 0.3 | `CurrentLan == null` 时探测系统语言 |

根 `Cargo.toml`：

```toml
[workspace]
resolver = "2"
members = ["crates/eyenurse-core", "crates/eyenurse-app"]

[workspace.package]
version = "3.0.0"
edition = "2021"
rust-version = "1.95"        # eframe 0.36.2 要求

[workspace.dependencies]
eframe = { version = "0.36.2", default-features = false, features = ["glow", "default_fonts"] }
tray-icon = "0.26.1"
image = { version = "0.25", default-features = false, features = ["png"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
crossbeam-channel = "0.5"
sys-locale = "0.3"
windows = { version = "0.62.2", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_RemoteDesktop",
    "Win32_System_Registry",
    "Win32_System_LibraryLoader",
    "Win32_System_Threading",       # CreateMutexW 单实例
    "Win32_UI_Shell",               # ShellExecuteW（关于页打开 URL，§7.5）
] }

[profile.release]
lto = "thin"
strip = true
```

> 渲染后端选 `glow`（OpenGL）：托盘小工具无需 wgpu 的体积/编译时间；eframe 默认 `Renderer::Glow`。

---

## 3. v2 现状速览（实现者必读的 5 分钟版）

- **计时**：`GlobalTimerService`（`src/EyeNurse/Services/GlobalTimerService.cs`）= 1 秒 `System.Timers.Timer` + `DateTime` 比较的状态机；事件 `Elapsed{RemainingTime}`（每次检查都发）与 `Trigger`（剩余 <1s 时发一次并 Reset）。
- **窗口**：
  - `CountdownWindow`（悬浮条，80×20，无边框、可拖动、可置顶、Alt-Tab 隐藏，`CountdownWindow.xaml:11-21`、`.xaml.cs:48-58`）；
  - `LockScreenWindow`（每显示器一个遮罩，Topmost，无任务栏图标，`LockScreenWindow.xaml:16-22`）；
  - `MainWindow`（WebView2 设置窗，900×500）→ v3 用 egui 视口替代。
- **托盘**：`App.xaml.cs:48-91`，菜单顺序：关于/设置/—/暂停/恢复(初始隐藏)/重置/立刻休息/—/退出；双击托盘 → 显示悬浮条。
- **会话锁**：`SystemEvents.SessionSwitch`（`EyeNurseViewModel.cs:59-83`）：Lock → 记时 + Pause；Unlock → Resume，锁超 `ResetTimeout` 则（若在休息则提前结束 +）Reset。
- **配置**：`%LOCALAPPDATA%\EyeNurse\UserData\{Setting,UI,Languages}.json`（外部库 `ConfigService.cs`：`LocalApplicationData + "\\" + AppName + "\\UserData"`，`AppName = nameof(EyeNurse)` 来自 `App.xaml.cs:37`；文件名 = 类型名 + `.json`）。序列化 = System.Text.Json 默认选项（外部库 `JsonHelper.cs`：`JsonSerializer.Serialize(obj, obj.GetType())`，无任何 options）。
- **自启**：`DesktopStartupHelper`（外部库）：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`，值名 `"Eye Nurse 2"`（`EyeNurseService.cs:34`），值数据 = exe 完整路径（不带引号）；`Check()` 对值名做忽略大小写比较。
- **i18n**：`LanService.Init(new JsonFileDB("Assets\\Languages"), true, CurrentLan, "en")`（`EyeNurseService.cs:66`）；`zh.json`/`en.json` 平铺 `{key: value}`（`src/EyeNurse/Assets/Languages/`）。
- **已知怪癖**（v3 必须显式决定保留/修复，逐条见 parity 清单 Q 系列）：`Stop()` 未启动也会发一次 Elapsed；`ResetWhenSessionUnlock` 开关被 UI 保存但解锁逻辑**并未读取**它（`EyeNurseViewModel.cs:63-77`）；休息时长实际为 `RestDuration + 1s`（`EyeNurseViewModel.cs:212-218` 的 `TotalSeconds < 0` 判出）；遮罩用**启动时**的显示器快照（`EyeNurseViewModel.cs:46`）。

---

## 4. crates/eyenurse-core —— 完整类型签名【实现者 A】

原则：core 内**没有线程、没有时钟读取**（`Instant`/系统时间一律由调用方传入参数），全部方法同步、可单测。错误用 `thiserror` 风格手写 enum（不引依赖）。

### 4.1 `time.rs` —— .NET TimeSpan 兼容

v2 磁盘上的 TimeSpan 是 **System.Text.Json 的字符串**。已实测验证：.NET 10 `JsonSerializer.Serialize(new TimeSpan(0,45,0))` → `"00:45:00"`（"c" 常量格式；.NET 6 GA 起行为相同，见 Microsoft Learn "Breaking change: Default serialization format for TimeSpan"）。.NET "c" 文法：`[-]d.hh:mm:ss.fffffff`，天/小数部分为 0 时省略。

```rust
/// .NET TimeSpan 的 Rust 等价物，serde 序列化为 "c" 格式字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct HmsSpan(std::time::Duration);

impl HmsSpan {
    pub fn from_hms(h: i64, m: i64, s: i64) -> Self;
    pub fn from_duration(d: std::time::Duration) -> Self;
    /// 解析 .NET "c" 格式（`[-]d.hh:mm:ss[.fffffff]`）；
    /// 兼容 v2 UI 输入的短格式 `[-]hh:mm[:ss]`（.NET TimeSpan.TryParse 的 g 格式子集）。
    /// 解析失败返回 None（调用方回退默认值，对齐 v2 `Setting.GetTimeSpan`）。
    pub fn parse(s: &str) -> Option<Self>;
    /// 输出 .NET "c" 格式：负数在此不可能出现（parse 后即 clamp）。
    /// 规则：days>0 → `d.hh:mm:ss`；小数非 0 → 补齐 7 位；否则 `hh:mm:ss`。
    pub fn to_c_string(&self) -> String;
    pub fn as_duration(self) -> std::time::Duration;
    /// 复刻 v2 `Setting.GetTimeSpan` 的 clamp（Setting.cs:28-29）：
    /// `TotalDays >= 1 || TotalDays <= 0` 时取 `(|Hours|, |Minutes|, |Seconds|)`（丢掉天与负号）。
    pub fn clamp_like_v2(self) -> Self;
}

impl serde::Serialize for HmsSpan;   // 写出 to_c_string()
impl<'de> serde::Deserialize<'de> for HmsSpan;  // parse 失败 → Err（由 #[serde(default)] 兜底成默认值）
```

> 字节兼容细节：`serde_json` 对 `f64 100.0` 输出 `"100.0"`，而 System.Text.Json 输出 `100`。`UIJson` 的两个坐标字段自定义序列化：`fract()==0` 时按 `i64` 写出（§4.3）。

### 4.2 `timer.rs` —— v2 GlobalTimerService 的语义等价状态机

v2 是 1 秒系统定时器 + `DateTime.Now` 比较（`GlobalTimerService.cs:41-132`）。v3 改为**纯状态机 + 调用方喂时间**（1Hz 喂，见 §6.1），对 **v2 UI 可达的调用序列**逐条等价（唯一例外是 UI 不可达的“暂停后 Start”，见怪癖 6/Q11）：

```rust
pub struct GlobalTimer {
    interval: std::time::Duration,
    next_trigger: Option<std::time::Instant>,
    paused_at: Option<std::time::Instant>,
    started: bool,
}

/// 与 v2 事件顺序一致：先 Elapsed 后 Trigger（CheckTrigger, GlobalTimerService.cs:103-132）
pub enum TimerEvent {
    /// v2 Elapsed：每次 poll 都发（next_trigger 为 Some 时），remaining 已 clamp 到 ≥0
    Elapsed { remaining: std::time::Duration },
    /// v2 Trigger：`next_trigger - now < 1s` 时发一次，随后自动 reset(now)
    Trigger,
}

#[derive(Debug)]
pub struct IntervalTooShort;   // 对应 v2 ArgumentOutOfRangeException

impl GlobalTimer {
    /// v2 构造函数：interval < 1s → Err(IntervalTooShort)（GlobalTimerService.cs:27-32）
    pub fn new(interval: std::time::Duration) -> Result<Self, IntervalTooShort>;
    pub fn interval(&self) -> std::time::Duration;
    /// v2 Start()：已启动 → false；否则 reset(now)（触发一次 poll 语义由调用方 poll 消化）并置 started=true（cs:41-54）
    pub fn start(&mut self, now: std::time::Instant) -> bool;
    /// v2 Stop()：未启动 → 先产出一次 poll 语义（调用方须再 poll 一次！见下）并返回 false；
    /// 已启动 → 清 next_trigger/paused_at/started 并返回 true（cs:55-66）
    pub fn stop(&mut self, now: std::time::Instant) -> bool;
    /// v2 Pause()：未启动或已暂停 → false；否则记 paused_at=now、started 保持“运行中但冻结”语义并返回 true（cs:67-76）
    pub fn pause(&mut self, now: std::time::Instant) -> bool;
    /// v2 Resume()：未启动或未暂停 → false；否则 next_trigger += now - paused_at，清 paused_at，返回 true（cs:77-88）
    pub fn resume(&mut self, now: std::time::Instant) -> bool;
    /// v2 Reset()：恒 true；next_trigger = now + interval（cs:89-94）。
    /// 注意 v2 Reset 内部随后立即 CheckTrigger（cs:92 “立即更新”）——v3 的即时 Elapsed
    /// 由调用方在 reset() 后紧跟一次 poll() 完成（见下方怪癖 3）。
    pub fn reset(&mut self, now: std::time::Instant) -> bool;
    /// v2 CheckTrigger()：每次调用先发 Elapsed（若有 next_trigger）；
    /// 若 `next_trigger - now < 1s` 再发 Trigger 并 reset(now)。可重入保护在 v3 不需要（单线程调用）。
    /// 返回按序事件（长度 0..=2）。
    pub fn poll(&mut self, now: std::time::Instant) -> Vec<TimerEvent>;
    pub fn remaining(&self, now: std::time::Instant) -> Option<std::time::Duration>;
    pub fn is_started(&self) -> bool;
    pub fn is_paused(&self) -> bool;
}
```

**必须移植的 v2 怪癖（parity 由 `src/TestProject/GlobalTimerServiceTest.cs` 锁定）**：
1. `Stop()` 在未启动时返回 `false` **且**调用一次 CheckTrigger（cs:64）→ v3 里 `stop()` 返回 false 时调用方（`AppModel::stop_timer`，§5.6）必须紧接一次 `poll()`（core 只负责状态）。
2. `Pause()`/`Resume()` 也各带一次 CheckTrigger（cs:74, 86）→ `AppModel::pause/resume` 同样“调用后立即 poll 并应用事件”。
3. `Reset()` 内部立即 CheckTrigger（cs:92 注释“立即更新”）→ `AppModel::reset_timer` 在 `reset()` 后**必须立即 `poll()`** 并应用 Elapsed，否则悬浮条要等下一秒心跳才回满，丢失 v2 的即时刷新。
4. 已启动再 `Start()` → false；未启动 `Pause()` → false；未启动/已运行 `Resume()` → false。
5. `Trigger` 发出后**立即** `Reset()`，即 remaining 立刻回满。
6. 【有意偏差，清单 Q11】v2 `Start()` 的守卫是 `_timer != null && _timer.Enabled`（cs:43）：**暂停后**调用 `Start()` 会成功（Enabled=false 通过守卫并新建定时器）。该序列在 v2 UI 上不可达（菜单没有 Start；start 只出现在 Init 与休息结束），NUnit 亦未锁定。v3 选择：`start()` 在暂停期间返回 `false`（视为已运行）。不得写成“逐条等价”。

单测要求（A 负责，对齐 NUnit 测试的断言）：start 两次第二次 false；未启动 stop false；pause 后 poll 不再产出 Trigger；resume 后 remaining 单调递减；reset 后 remaining == interval 且紧接的 poll 产出 `Elapsed{interval}`；interval<1s 报错。

### 4.3 `config.rs` —— 与 v2 字节级兼容的持久化

路径与文件名（外部库 ConfigService.cs，摘录见附录 §11）：`%LOCALAPPDATA%\EyeNurse\UserData\<TypeName>.json`。目录定位用 KnownFolder API（`SHGetKnownFolderPath(FOLDERID_LocalAppData)`，见本节末尾"目录定位"，**不能**直接读 `LOCALAPPDATA` 环境变量）。序列化规则（外部库 JsonHelper.cs：System.Text.Json **默认选项**，摘录见附录 §11）：
- 属性名原样 PascalCase、按声明顺序输出；
- 紧凑（无缩进、无空格）；
- `null` 引用照写（`"CurrentLan":null`）；
- 坏 JSON / 缺文件 → **整个对象取默认值**（v2 `JsonDeserialize` 吞异常返回 `default(T)`，`ConfigService.LoadUserConfig` 再 `?? new T()`）；
- 保存失败不 panic（v2 只记日志），返回 `Err` 由上层记录。

```rust
pub struct ConfigStore { user_config_dir: std::path::PathBuf }

impl ConfigStore {
    /// %LOCALAPPDATA%\EyeNurse\UserData（v2 AppName 固定为 "EyeNurse"）
    pub fn v2_default() -> Self;
    pub fn with_dir(dir: std::path::PathBuf) -> Self;   // 单测用临时目录
    pub fn dir(&self) -> &std::path::Path;
    /// 读 <T>::FILE_NAME；缺失/坏 JSON → T::default()
    pub fn load<T: ConfigFile + serde::de::DeserializeOwned>(&self) -> T;
    /// 写 <T>::FILE_NAME；目录不存在则创建；紧凑序列化
    pub fn save<T: ConfigFile + serde::Serialize>(&self, value: &T) -> std::io::Result<()>;
}

pub trait ConfigFile { const FILE_NAME: &'static str; }

/// Setting.json —— v2 Setting.cs:35-48，字段顺序/默认值逐项一致。
/// 评审修订：容器级 #[serde(default)] 要求 T: Default，且默认值不是 HmsSpan::ZERO，
/// 因此必须手写 impl Default（derive 出来的零值是错的）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SettingJson {
    pub RestInterval:    HmsSpan, // "00:45:00"  = new TimeSpan(0,45,0)（Setting.cs:7）
    pub RestDuration:    HmsSpan, // "00:02:30"  = new TimeSpan(0,2,30)（Setting.cs:8）
    pub RunWhenStarts:   bool,    // true        （Setting.cs:40）
    pub ResetWhenSessionUnlock: bool, // false   （Setting.cs:44）
    pub ResetTimeout:    HmsSpan, // "00:05:00"  = new TimeSpan(0,5,0)（Setting.cs:9）
}
impl ConfigFile for SettingJson { const FILE_NAME: &'static str = "Setting.json"; }

impl Default for SettingJson {
    fn default() -> Self {
        Self {
            RestInterval: HmsSpan::from_hms(0, 45, 0),
            RestDuration: HmsSpan::from_hms(0, 2, 30),
            RunWhenStarts: true,
            ResetWhenSessionUnlock: false,
            ResetTimeout: HmsSpan::from_hms(0, 5, 0),
        }
    }
}

/// UI.json —— v2 UI.cs:11-13。注意 v2 的拼写错误 CoutdownWindow 必须原样保留！
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UiJson {
    pub CoutdownWindowLeft: StjF64, // 100.0（输出字节 "100"）
    pub CoutdownWindowTop:  StjF64, // 100.0
    pub TopMost:            bool,   // true
}
impl ConfigFile for UiJson { const FILE_NAME: &'static str = "UI.json"; }
// 同样手写 impl Default：CoutdownWindowLeft/Top = 100.0、TopMost = true。

/// Languages.json —— v2 Languages.cs:6
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LanguagesJson {
    pub CurrentLan: Option<String>, // null 时照写 "CurrentLan":null
}
impl ConfigFile for LanguagesJson { const FILE_NAME: &'static str = "Languages.json"; }
// 手写 impl Default：CurrentLan = None。

/// f64 的 System.Text.Json 字节兼容包装（整数值输出 "100" 而非 "100.0"）
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StjF64(pub f64);
impl serde::Serialize for StjF64;         // fract()==0 && |v| < 2^53 → i64 路径
impl<'de> serde::Deserialize<'de> for StjF64;
```

> Setting.json 与 v2 的期望字节（保存默认配置后）：
> `{"RestInterval":"00:45:00","RestDuration":"00:02:30","RunWhenStarts":true,"ResetWhenSessionUnlock":false,"ResetTimeout":"00:05:00"}`

clamp 时机（评审修订，原稿有误）：v2 只在“前端字符串 → Setting”构造路径 clamp（Setting.cs:14-21 的 `GetTimeSpan`），**读盘路径从不 clamp**（ConfigService 直接反序列化 TimeSpan）。因此 v3：`ConfigStore::load` 之后**不做**任何 clamp；`clamp_like_v2` 只在设置窗“文本 → 数值”保存路径调用（§5.6 `UiAction::SaveSettings` 内 `parse + clamp`）。这样手改过 ≥1 天/负值 RestInterval 的老配置在 v3 下行为与 v2 一致，且保存字节可往返（清单 F6/F12）。

目录定位（评审修订）：`ConfigStore::v2_default()` 用 `SHGetKnownFolderPath(&FOLDERID_LocalAppData, ..)`（`Win32::UI::Shell`，与 .NET `Environment.GetFolderPath(LocalApplicationData)` 同为 KnownFolder API；两个符号已核实存在于 windows-rs 该模块）拿真实目录，**不能直接读 `LOCALAPPDATA` 环境变量**（改过环境变量的机器上会与 v2 路径分叉，清单 F1）；KnownFolder 调用失败才回退环境变量。

### 4.4 `autostart.rs` —— HKCU 自启

对齐外部库 `DesktopStartupHelper.cs`（已核实源码）：

```rust
pub struct DesktopStartup { key_name: String, exe_path: String }

impl DesktopStartup {
    /// v2 实例化参数：("Eye Nurse 2", <exe 完整路径>)（EyeNurseService.cs:34）
    /// v3 默认 key_name = "Eye Nurse 3"；v2 的旧键由 migrate_from 静默迁移（见下，清单 Q6/G2）
    pub fn new(key_name: impl Into<String>, exe_path: impl Into<String>) -> Self;
    /// HKCU\Software\Microsoft\Windows\CurrentVersion\Run 下是否存在同名值（忽略大小写）
    pub fn check(&self) -> bool;
    /// true=写值（REG_SZ，exe 完整路径，不带引号——保持 v2 形态），false=删值；打开失败返回 false
    pub fn set_enabled(&self, enabled: bool) -> bool;
    /// 自启值名升级的**静默迁移**（清单 Q6/G2，无任何 UI 提示）：
    /// 本键缺失且 legacy_key 存在 → 写本键 + 删 legacy_key，返回是否发生迁移。
    /// 落点：main.rs 启动序列第 2.5 步调用 startup.migrate_from("Eye Nurse 2")。
    pub fn migrate_from(&self, legacy_key: &str) -> bool;
}
```

windows-rs 用法（`Win32::System::Registry`，全部 `unsafe`）：
`RegOpenKeyExW(HKEY_CURRENT_USER, wide!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"), 0, KEY_SET_VALUE | KEY_QUERY_VALUE, &mut hkey)`；
写 `RegSetValueExW(hkey, name, 0, REG_SZ, bytes.as_ptr() as _, byte_len)`；
删 `RegDeleteValueW(hkey, name)`（`ERROR_FILE_NOT_FOUND` 视为成功）；
查 `RegQueryValueExW(hkey, name, ..)`；关 `RegCloseKey(hkey)`。
exe 路径获取：`std::env::current_exe()`。

### 4.5 `i18n.rs`

v2 资源文件平铺 JSON：`src/EyeNurse/Assets/Languages/{zh,en}.json`（键集：about, always_on_top, app_name, exit, installNow, installWebview2Tips, pause, reset, rest_now, resting, resume, setting）。v3 键集 = 上述 + 设置窗键（v2 Vue 侧 `src/eyenurse-client-ui/src/locales/*.json`：clientSettings, copyright, home, panel, restDuration, restInterval, runWhenStarts, resetWhenSessionUnlock, settings, zh, en）。v3 自带的两份 json 合并两套键；`installWebview2Tips`/`installNow` 不再使用但**保留在文件里**（v2 文件兼容）。

```rust
pub struct I18n { /* current, fallback, lang -> key -> value */ }

impl I18n {
    /// dir: exe 旁 "Assets/Languages"；current: LanguagesJson.CurrentLan；
    /// fallback: "en"；system: sys_locale::get_locale()（如 "zh-CN"）。
    /// current 为 None/" " 时：system 以 "zh" 开头 → "zh"，否则 "en"。
    /// 每语言文件缺失 → 空表；坏 JSON → 空表（不 panic，对齐 v2 容错风格）。
    pub fn load(dir: &std::path::Path, current: Option<&str>, fallback: &str, system: &str) -> Self;
    /// 当前语言 → 回退语言 → 原样返回 key（v2 LanService.Get 语义）
    pub fn get(&self, key: &str) -> String;
    pub fn available(&self) -> Vec<String>;      // 目录下 *.json 文件名（去扩展名）排序
    pub fn current(&self) -> &str;
    pub fn set_language(&mut self, lan: &str);   // 未知语言 → 回退 fallback
}
```

加载顺序：优先磁盘（保持 v2 的“替换 json 即换文案”能力），磁盘目录不存在时回退 `include_str!` 内嵌副本（`I18n::load_embedded(current, fallback, system)`）。**单一真源**（评审修订）：内嵌副本放 A 的 `crates/eyenurse-core/assets/languages/{zh,en}.json`——`include_str!` 无法跨 crate 引用 D 的资产，真源必须在 core 内；D 打包时直接从该目录拷贝到产物 `Assets/Languages/`，**不再维护第二份**（改文案只改 core 目录里的文件）。**注意**：切语言即写 `Languages.json`（v2 `ClientApi.SetCurrentLanguage`，ClientApi.cs:36-45），UI 层负责调用。

---

## 5. crates/eyenurse-app —— 结构与完整签名【实现者 B 主笔契约】

### 5.1 入口 `main.rs`（B）

```rust
// main.rs 第一行（评审补充）：托盘常驻 GUI 程序不能带控制台黑窗（v2 为 WPF WinExe 同理；
// debug 构建保留控制台方便看日志）
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    // 1) 单实例（v2 未见实现，v3 新增）：CreateMutexW("Local\\EyeNurse3")，
    //    GetLastError()==ERROR_ALREADY_EXISTS → 直接退出
    // 2) ConfigStore::v2_default() 读 LanguagesJson → I18n（磁盘优先，内嵌兜底）
    // 2.5) DesktopStartup::new("Eye Nurse 3", exe).migrate_from("Eye Nurse 2")（清单 Q6/G2）
    // 2.6) startup.set_enabled(setting.RunWhenStarts) —— **每次启动都应用**
    //      （对齐 v2 App.xaml.cs:43 → EyeNurseService.Init → ApplySetting，EyeNurseService.cs:68-69；
    //        清单 G5“首启写自启”的实现落点；漏掉这步则首次运行注册表无值）
    // 3) let session = SessionThread::spawn()?（§5.4：句柄含 rx + request_stop）
    // 4) eframe::run_native("EyeNurse3", native_options(ui.TopMost),
    //      Box::new(|cc| Ok(Box::new(AppModel::new(cc, session.rx)))))
    //      （计时器的启动在 AppModel::new 里，见 §5.6 —— 对齐 v2 EyeNurseViewModel.Init cs:98-102）
    // 5) run_native 返回后：session.request_stop(); session.join();（§7.3）
}
// top_most = 第 2 步读到的 UiJson.TopMost。
// 评审修订：with_always_on_top(self) 无参（docs.rs 已核实），不能传值，只能条件调用；
// 原稿无条件置顶会把用户关掉的置顶在重启后改回去（清单 B5 验收必挂）。
fn native_options(top_most: bool) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: {
            let b = egui::ViewportBuilder::default()
                .with_title("CountdownWindow")
                .with_decorations(false)
                .with_resizable(false)
                .with_taskbar(false)
                .with_transparent(false)
                .with_inner_size([80.0, 20.0])            // 逻辑点；v2 XAML 80x20 DIP
                .with_position(saved_or([100.0, 100.0])); // UiJson 两字段（v2 默认 100,100）
            if top_most { b.with_always_on_top() } else { b }
        },
        renderer: eframe::Renderer::Glow,
        run_and_return: true,
        ..Default::default()
    }
}
```

> `run_native` 签名（eframe 0.36.2）：`pub fn run_native(app_name: &str, native_options: NativeOptions, app_creator: AppCreator) -> Result<()>`，`AppCreator = Box<dyn FnOnce(&CreationContext) -> Result<Box<dyn App>, Box<dyn std::error::Error + Send + Sync>>>`。

### 5.2 事件通道 `events.rs`（B）

```rust
/// Win32 线程 → 主循环
pub enum WinEvent {
    SessionLock,      // WM_WTSSESSION_CHANGE wParam==WTS_SESSION_LOCK(7)
    SessionUnlock,    // wParam==WTS_SESSION_UNLOCK(8)
    DisplayChanged,   // WM_DISPLAYCHANGE：显示器的数量/布局可能变了
}
/// 事件路由规则（评审修订，原表述有误）：
/// - root（App::ui/logic）里对 AppModel 是同线程直接方法调用；
/// - **deferred 视口闭包拿不到 `&mut AppModel`** → 一切跨视口动作走 `UiAction` 通道
///   （`SharedUi.actions`），且发送后必须唤醒 ROOT（§6.1）；
/// - Win32 线程走 `WinEvent` 通道。通道全部 `crossbeam_channel::unbounded`。
```

### 5.3 托盘 `tray.rs`（B）

v2 菜单（App.xaml.cs:48-91）逐项复刻，id 用 v2 语言键命名：

```rust
pub struct Tray { _tray: tray_icon::TrayIcon }
// 菜单（顺序固定）：
// about / setting / ─ / pause / resume(初始隐藏) / reset / rest_now / ─ / exit
// resume 初始隐藏、pause 可见；Pause 动作后互换（App.xaml.cs:59, 137-143）

impl Tray {
    /// 必须在 eframe creator 回调里（= winit 主线程，之后该线程跑消息泵，
    /// 满足 tray-icon 文档要求 "On Windows ... an event loop must be running on the thread"）。
    /// i18n 文案由调用方传入，返回 Self 供 drop 时注销。
    pub fn new(icon_rgba: Vec<u8>, w: u32, h: u32, texts: &I18nTexts) -> tray_icon::Result<Self>;
    pub fn update_texts(&self, texts: &I18nTexts);      // 切语言后刷新菜单标题（对齐 UpdateNotifyIconText）
    pub fn set_paused(&self, paused: bool);             // 菜单互换：**无条件**调用（对齐 v2 App.xaml.cs:129-143：
                                                        //   点击处理先翻菜单再调 vm.Pause()，Pause 失败也翻，清单 Q12/D5）；
                                                        //   muda 无 set_visible，实现见下
    pub fn set_icon_rgba(&mut self, rgba: Vec<u8>, w: u32, h: u32);
}
```

用到的 tray-icon 0.26.1 API（已核实）：
- 构建：`TrayIconBuilder::new().with_tooltip(..).with_icon(Icon).with_menu(Box::new(menu)).with_menu_on_left_click(false).build()`；
- 菜单：`tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem, MenuId}`，`Menu::new()` + `menu.append(&item)?`，分隔线 `PredefinedMenuItem::separator()`；
- 事件：`tray_icon::menu::MenuEvent::receiver()`（全局 `&Receiver<MenuEvent>`，事件 `MenuEvent { id: MenuId }`）与 `tray_icon::TrayIconEvent::receiver()`（Click/DoubleClick/Enter/Move/Leave）——每帧 `try_recv` 排空（§6.1）；
- `MenuId::new("rest_now")` 等字符串 id 与动作映射写在 `tray.rs` 内部。

**pause/resume 可见性互换的实现限制（评审修订）**：muda 0.21.1 的 `MenuItem` **没有 `set_visible`**（已核实方法全集：`set_enabled`/`set_text`/`set_accelerator` 等，无可见性 API），v2 的 `Visibility.Collapsed`（App.xaml.cs:59, 137-143）不能直接照搬。v3 用菜单的插入/删除实现等效互换——muda 0.21.1 签名已核实：`pub fn Menu::insert(&self, item: &dyn IsMenuItem, position: usize) -> Result<()>`、`pub fn Menu::remove(&self, item: &dyn IsMenuItem) -> Result<()>`。`set_paused(true)` = `remove(pause)` + `insert(resume, 原 pause 位置)`，false 反之。行为与 v2 一致（清单 E2/D5 验收口径不变，只是实现手段不同，不算偏差）。

图标：`image::load_from_memory(logo.png)` → RGBA8 → `tray_icon::Icon::from_rgba(rgba, w, h)`；同一份 RGBA 转 `egui::IconData { width, height, rgba }` 给 `ViewportBuilder::with_icon`。

### 5.4 `win/session.rs` —— WTS 会话锁线程（B）

**结论（已查证）**：`WTSRegisterSessionNotification` 的通知经由窗口过程投递；message-only 窗口（`HWND_MESSAGE`）收不到系统类消息（Raymond Chen, 2025-11-27 "Message-only windows are for messaging, not as a broadcast receiver"），因此用一个**从不显示的顶层窗口**（`WS_OVERLAPPED`，不 Show）。

```rust
/// 句柄结构（评审修订：原 `spawn -> io::Result<()>` 不暴露 hwnd/控制通道，
/// 导致 WTS 永远无法注销）：
pub struct SessionThread {
    pub rx: crossbeam_channel::Receiver<WinEvent>,
    hwnd: std::sync::Arc<std::sync::OnceLock<isize>>,  // 线程内建窗后回填
    join: std::thread::JoinHandle<()>,
}

impl SessionThread {
    /// 在独立 std::thread 上执行：
    /// 1) GetModuleHandleW(None) + RegisterClassW("EyeNurse3SessionWnd")
    /// 2) CreateWindowExW(0, "EyeNurse3SessionWnd", "", WS_OVERLAPPED,
    ///    CW_USEDEFAULT, .., HWND_NULL, HMENU::default(), hinstance, None)   // 从不 Show
    ///    hwnd 写入 OnceLock
    /// 3) WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)   // wtsapi32，动态链接
    /// 4) GetMessageW 循环；过程函数：
    ///    WM_WTSSESSION_CHANGE(0x02B1=689)：wParam 7 → tx.send(SessionLock)，8 → tx.send(SessionUnlock)
    ///    WM_DISPLAYCHANGE(0x007E)        → tx.send(DisplayChanged)
    ///    WM_CLOSE → WTSUnRegisterSessionNotification(hwnd) + DestroyWindow + PostQuitMessage
    ///    其余 → DefWindowProcW
    pub fn spawn() -> std::io::Result<Self>;
    /// 退出路径（§7.3）：PostMessageW(hwnd, WM_CLOSE)（跨线程投递合法），
    /// 窗口过程完成注销后线程自然退出
    pub fn request_stop(&self);
    pub fn join(self);
}
```

windows-rs 名称（已核实 0.62 存在）：
- `Win32::UI::WindowsAndMessaging`：`RegisterClassW`、`CreateWindowExW`、`GetMessageW`、`TranslateMessage`、`DispatchMessageW`、`DefWindowProcW`、`DestroyWindow`、`PostQuitMessage`、`WNDCLASSW`、`WM_DISPLAYCHANGE`、**`WM_WTSSESSION_CHANGE: u32 = 689`**；
- `Win32::System::RemoteDesktop`：`WTSRegisterSessionNotification(hwnd: HWND, dwflags: REMOTE_SESSION_ACTION)`、`WTSUnRegisterSessionNotification`、`NOTIFY_FOR_THIS_SESSION`；
- wtsapi32 为动态链接：`#[link(name = "wtsapi32")]` extern 声明即可（Windows 自带该 DLL）。

`WTS_SESSION_LOCK = 7`、`WTS_SESSION_UNLOCK = 8`（完整表 1..9 见 Microsoft Learn WTSRegisterSessionNotification 页；v3 只关心 7/8）。

### 5.5 `win/monitors.rs` —— 显示器枚举（B）

```rust
/// 物理像素坐标（与 v2 User32Ex.GetMonitorInfos() 等价）
pub struct MonitorRects {
    pub device_name: String,   // MONITORINFOEXW.szDevice，如 "\\.\DISPLAY1"
    pub rc_monitor: [i32; 4],  // 整屏 left,top,right,bottom
    pub rc_work:    [i32; 4],  // 工作区（去掉任务栏）——v2 遮罩用这个（EyeNurseViewModel.cs:189-192）
}
pub fn get_monitor_infos() -> Vec<MonitorRects>;
```

windows-rs 用法（`Win32::Graphics::Gdi`，签名已核实）：
- `EnumDisplayMonitors(hdc: Option<HDC>, lprcclip: Option<*const RECT>, lpfnenum: MONITORENUMPROC, lparam: LPARAM) -> BOOL`；
- 回调 `unsafe extern "system" fn(hmon: HMONITOR, hdc: HDC, rect: *mut RECT, lparam: LPARAM) -> BOOL`，通过 `lparam` 传 `&mut Vec<MonitorRects>` 指针；
- 每个句柄：`let mut info = MONITORINFOEXW::default(); info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;` 然后 `GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut MONITORINFO)`（EXW 以基类指针传入，靠 cbSize 区分），取 `szDevice`、`rcMonitor`、`rcWork`。

### 5.6 `app.rs` —— AppModel（B；C/D 只调用这些签名）

eframe 0.36.2 的 `App` trait（已核实，**注意与旧版不同**）：必需方法是 `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)`（root 视口每帧）；另有
`fn logic(&mut self, ctx: &egui::Context, frame: &mut Frame)`——**每次 `ui` 之前调用，且窗口隐藏时只要 `request_repaint` 也会被调用**（eframe 文档原文："Called once before each call to Self::ui, and additionally also called when the UI is hidden"）。`logic` 正是 1Hz 心跳与事件排空的位置。

```rust
pub struct AppModel {
    ctx: egui::Context,
    tray: Tray,
    core: CoreBundle,          // 见下，字段对 C/D 不透明
    // —— 以下为 C/D 需要感知的只读访问器（见方法区）——
}

pub struct CoreBundle {
    pub cfg: eyenurse_core::config::ConfigStore,
    pub setting: eyenurse_core::config::SettingJson,
    pub ui:      eyenurse_core::config::UiJson,
    pub lang:    eyenurse_core::config::LanguagesJson,
    pub i18n:    eyenurse_core::i18n::I18n,
    pub timer:   eyenurse_core::timer::GlobalTimer,
}

/// 休息/暂停等跨视口共享的展示状态（deferred 视口回调要求 Send+Sync，
/// 用 Arc<Mutex<..>> 传入；每帧 logic 里更新快照）
pub struct SharedUi {
    pub bar: Mutex<BarState>,        // { main_countdown: Duration, main_interval: Duration,
                                     //   paused_time: Option<Duration>, is_resting: bool }
    pub mask: Mutex<MaskState>,      // { lock_countdown: Duration, monitors: Vec<MonitorRects>, open: bool }
    // 类型定义在 D 的 ui/settings.rs（所有权规则见 §1）；app.rs 仅 `use crate::ui::settings::SettingsDraft;`
    pub settings: Mutex<ui::settings::SettingsDraft>,
    // 跨视口动作出口：deferred 视口闭包发送后必须唤醒 ROOT（§6.1），否则最坏 1s 才被处理
    pub actions: (crossbeam_channel::Sender<UiAction>, crossbeam_channel::Receiver<UiAction>),
}

impl eframe::App for AppModel {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame); // §6.1 事件流
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame);      // 委托 ui::bar::bar_ui
    fn on_exit(&mut self, _gl: Option<&glow::Context>);                  // 保存 UI.json（位置/置顶兜底）
}

impl AppModel {
    pub fn new(cc: &eframe::CreationContext, rx: Receiver<WinEvent>) -> Self;
    // new() 必须完成（评审修订，均为 v2 启动必经路径）：
    // 1) ui::fonts::load_cjk_fonts(&cc.egui_ctx) —— 首帧前注册中文字体
    //    （C 提供该函数，由 B 在此调用一次；D 的设置窗依赖它但不得自行调用，见 §1 契约）
    // 2) 读 SettingJson → GlobalTimer::new(interval)?; timer.start(Instant::now())
    //    —— 对齐 v2 EyeNurseViewModel.Init（cs:98-102）；漏掉则悬浮条永远不走表
    // 供 C（bar/mask）与 D（settings）调用：
    pub fn shared(&self) -> &std::sync::Arc<SharedUi>;
    pub fn texts(&self) -> I18nTexts;               // 快照全部界面文案（锁内 clone）
    // —— 动作（托盘/遮罩按钮/设置窗共用，全部同线程直接调用）——
    pub fn rest_now(&mut self);                     // §6.2
    pub fn stop_timer(&mut self);                   // timer.stop(now)；返回 false 时补一次 poll（§4.2 怪癖 1）
    pub fn pause(&mut self) -> bool;                // timer.pause(now) 成功才置 paused 展示态并 poll（§4.2 怪癖 2、§6.3 门控）
    pub fn resume(&mut self) -> bool;               // 同上门控
    pub fn reset_timer(&mut self);                  // 若暂停展示中先 resume()（清 PausedTime），再 reset + 立即 poll
                                                    // （v2 EyeNurseViewModel.Reset cs:229-234 = if IsPaused Resume + Reset；
                                                    //   §4.2 怪癖 3，清单 D7/A13。漏掉 resume 会卡在暂停展示）
    pub fn toggle_topmost(&mut self, on: bool);     // 写 UI.json + 视口层级
    pub fn show_settings_window(&mut self, on: bool);
    pub fn set_language(&mut self, lan: &str);      // 写 Languages.json + i18n + tray 文案
    pub fn open_about(&mut self);                   // ShellExecute "https://www.giantapp.cn/post/products/eyenurse2"
    pub fn exit(&mut self);                         // 根视口 Close（run_and_return=true → run_native 返回）
    // —— B 内部 ——
    pub fn drain_win_events(&mut self);
    pub fn tick_timer(&mut self, now: Instant);     // timer.poll + SharedUi.bar 更新 + Trigger→rest_now
    pub fn begin_rest(&mut self, monitors: Vec<MonitorRects>);
    pub fn end_rest(&mut self);
}
```

### 5.7 视口编排 `ui/mod.rs`（B 提供骨架，C/D 填充）

```rust
// ui/mod.rs（B 独占，纯接线，骨架期定稿后不再改）
// 评审修订：ViewportId 没有 _named 之类的命名构造（已核实 0.36.2 仅有 ROOT 常量与 from_hash_of），
// from_hash_of 非 const fn，故用函数而非 const。
pub fn settings_id() -> egui::ViewportId { egui::ViewportId::from_hash_of("settings") }
pub fn mask_id(i: usize) -> egui::ViewportId { egui::ViewportId::from_hash_of(format!("mask-{i}")) }

/// 由 AppModel::logic 每帧调用（deferred 视口“当帧不注册即销毁”，egui 文档原文）。
/// mod.rs 只保留两行接线，注册逻辑在各自所有者的文件里：
pub fn show_viewports(ctx: &egui::Context, m: &mut AppModel) {
    ui::mask::maybe_show(ctx, m);      // 【C】ui/mask.rs：遮罩开着 → 注册 N 个遮罩视口
    ui::settings::maybe_show(ctx, m);  // 【D】ui/settings.rs：设置窗开着 → 注册设置视口，
}                                      //      含 close_requested → UiAction::CloseSettings
```

**悬浮条 = root 视口**（NativeOptions.viewport，§5.1）。**遮罩与设置窗 = deferred 视口**（注册逻辑分别在 C 的 `mask.rs::maybe_show` / D 的 `settings.rs::maybe_show`）：
`ctx.show_viewport_deferred(id, builder, move |ui, _class| { ... })`（签名已核实：`impl Fn(&mut Ui, ViewportClass) + Send + Sync + 'static`）。闭包内通过 `Arc<SharedUi>` 快照取数据；遮罩解锁按钮、设置窗动作通过 `SharedUi.actions` 发 `UiAction` 并**唤醒 ROOT**（§6.1），`logic` 里排空（`RestDone` / `SaveSettings(SettingJson)` / `ToggleTopmost(bool)` / `CloseSettings` / `SetLanguage(String)`）。

遮罩视口 builder（每显示器，物理坐标 → 逻辑点换算见 §7.4）：

```rust
egui::ViewportBuilder::default()
    .with_title("LockScreenWindow")
    .with_decorations(false).with_resizable(false)
    .with_always_on_top()
    .with_active(false)          // 不抢焦点（v2 Topmost=True + ShowInTaskbar=False）
    .with_taskbar(false)
    .with_position([work.left as f32 / scale, work.top as f32 / scale])
    .with_inner_size([(work.right-work.left) as f32 / scale, (work.bottom-work.top) as f32 / scale])
```

设置窗 builder：`with_title(app_name).with_inner_size([900.0, 500.0]).with_resizable(true)`（v2 MainWindow 900×500）。关闭（用户点 X）：该视口的 `ctx.input(|i| i.viewport().close_requested)` → 发 `UiAction::CloseSettings`（deferred 视口每帧都要注册，正好在这一帧停注册即可）。以上设置窗逻辑全部住在 D 的 `ui/settings.rs::maybe_show`。

---

## 6. 线程与事件流

### 6.1 主循环每帧流程（`App::logic`）

```
winit 主线程（唯一有 UI 状态的线程）
├─ logic(ctx, frame)：                                     ← 每帧 & 隐藏时心跳
│  1. drain  rx_win      （WTS/显示器事件）→ AppModel 方法
│  2. drain  MenuEvent::receiver() + TrayIconEvent::receiver()
│     · menu id → 对应 AppModel 动作；TrayIconEvent::DoubleClick → show bar（v2 双击托盘）
│  3. drain  UiAction    （遮罩按钮 / 设置窗）
│  4. tick_timer(Instant::now())
│     · timer.poll() → Elapsed 更新 SharedUi.bar；Trigger → rest_now()
│     · 休息中：lock_countdown -= 1s（每 tick 固定减 1s，对齐 v2 Task.Delay(1000) 粒度）；
│       **< 0** 才 end_rest()（对齐 v2 EyeNurseViewModel.cs:216 TotalSeconds<0：
│       显示走到 00:00 并停留一整秒，总时长 = RestDuration + 1s，清单 C6/Q2）
│  5. SharedUi 各 Mutex 更新快照
│  6. ui::show_viewports(ctx, self)      ← 注册遮罩/设置窗（当帧生效）
│  7. ctx.request_repaint_after(Duration::from_secs(1))   ← 1Hz 心跳，只作用于 ROOT（见要点 1）
└─ ui(ui, frame)：ui::bar::bar_ui(...)    ← root 悬浮条内容
```

要点：
- **不需要任何自定义 1 秒线程，但心跳是按视口的（评审修订）**：`request_repaint_after` 文档原文 "This repaints the **current** viewport"（已核实 egui 0.36.2 context.rs:1869-1873）——logic 末尾那次只保 root（悬浮条）1Hz。**deferred 视口必须自行驱动重绘**，否则遮罩上的 mm:ss 会冻结：
  - 遮罩 ui 闭包内每帧调用 `ui.ctx().request_repaint_after(Duration::from_secs(1))`（推荐，遮罩自调度）；或由 logic 调 `ctx.request_repaint_after_for(1s, mask_id(i))`（跨视口定向原语，签名已核实 context.rs:1913-1917）。
  - 设置窗是静态内容，跟随输入重绘即可，无需定时。
- **跨视口动作必须显式唤醒 ROOT（评审修订）**：deferred 视口的输入只会唤醒它自己，不会执行 root 的 logic。所有 deferred 视口向 `SharedUi.actions` 发送 `UiAction` 后，必须立刻 `ctx.request_repaint_after_for(Duration::ZERO, ViewportId::ROOT)`——否则遮罩解锁、设置保存、切语言最坏要等 1s 心跳才被处理（v2 均为即时，清单 C11；延迟论证见 §10）。
- **拖动悬浮条（评审修订：不得把 `ViewportCommand::StartDrag` 当唯一手段）**：egui-winit 0.36.2 对 StartDrag 有 `window.has_focus()` 前提（lib.rs:1750-1758，`if window.has_focus() && let Err(err) = window.drag_window()`，不满足时**静默跳过**；源码注释说明 x11 下不检查会永久占用输入）——点击当帧 winit 焦点事件尚未走完，拖动会偶发无效且无处报错。v3 规定**手写拖动为主**：`response.dragged()` 时按 `pointer.latest_pos() - prev` 每帧发 `ViewportCommand::OuterPosition(当前位置 + delta)`（无焦点依赖、行为确定，egui 拖拽状态本身就在本视口内）；拖动结束（`drag_stopped()`）当帧读 `ctx.input(|i| i.viewport().outer_rect)` 的 `min` 写回 `UiJson.CoutdownWindowLeft/Top` 并保存。注意 v2 的保存分支疑似死代码（清单 B6/Q9：`CountdownWindow.xaml.cs:41-44` 的 Released 判断在 `DragMove()` 模态循环返回后）；v3 按 Q9 决议**修复**为拖动结束即保存。
- 托盘双击：`TrayIconEvent::DoubleClick` → `show bar`（v2 `NotifyIcon_MouseDoubleClick`，App.xaml.cs:162-165；v3 bar 常驻可见，动作改为 focus + 置顶开关位置不变）。

### 6.2 `rest_now()` 状态机（对齐 EyeNurseViewModel.cs:177-202）

```
rest_now():
  if resting: return
  resting = true
  timer.stop(now)                       // v2 Stop
  monitors = win::get_monitor_infos()   // 【有意偏差】v2 用启动时快照；v3 现枚举（更正确，见清单 Q4/C10）
  begin_rest(monitors)                  // SharedUi.mask.open=true（本帧注册 N 个遮罩视口）
  lock_countdown = setting.RestDuration
  // —— 以下在 logic 的 1Hz tick 中推进（每 tick 固定减 1s，对齐 v2 Task.Delay(1000) 粒度）——
  loop: lock_countdown -= 1s
        若 unlock 按钮（UiAction::RestDone + 唤醒 ROOT）或 会话解锁且锁时>ResetTimeout（§6.3）→ 提前结束
        lock_countdown **< 0** 才正常结束（对齐 v2 EyeNurseViewModel.cs:216 TotalSeconds<0：
        显示走到 00:00 并停留一整秒，总时长 = RestDuration + 1s，清单 C6/Q2）
  end_rest(): mask.open=false; timer.reset(now); 立即 poll；timer.start(now); resting=false
```
（评审修订：原稿写 `≤ 0` 结束、一处 `-= elapsed` 一处 `-= 1s`，均与 v2 `< 0` 语义和自家清单 C6/Q2 矛盾，已统一为上式。）

遮罩内容（C，`ui/mask.rs`，对齐 LockScreenWindow.xaml）：背景 `#141418`、文字 `#EEEEF2`、居中 `mm:ss` 倒计时、70×70 的加载圈（egui 自绘 `Spinner`，egui 内置 widget）、右下角 25×25 解锁图标按钮（触发 `UiAction::RestDone`，等价 v2 CloseCommand）。

### 6.3 会话锁（对齐 EyeNurseViewModel.cs:59-83）

```
SessionLock:   locked_at = now;
               if pause() { /* 进入暂停展示：bar 切 PausedTime + 暂停配色 */ }
               // 门控（评审修订，对齐 v2 EyeNurseViewModel.cs:246-247 的 if (timer.Pause()) IsPaused = true）：
               // core 的 pause 在计时未运行时返回 false → **休息中锁屏（timer 已 stop）不进入暂停展示**，
               // 只记 locked_at；AppModel::pause() 必须按 core 返回值门控，不得无条件置 paused 态（清单 D8）。
SessionUnlock: resume()   // 同门控：未处于暂停态时 resume() 返回 false，无事发生
               if locked_at 存在 且 now - locked_at > setting.ResetTimeout:
                   if resting { end_rest 提前收尾 }     // v2: if (_isResting) CloseLockScreen()
                   locked_at = None; reset_timer()
```

**v2 怪癖（必须保留到 parity 决议为止）**：此分支**没有**检查 `ResetWhenSessionUnlock`（v2 只在 UI 保存该开关）。v3 是否启用该开关 = parity 清单 Q3 的决议，默认按 v2 原样（忽略开关）实现。

暂停展示：仅当 `pause()` 成功（门控通过）才进入——bar 显示 `PausedTime`（1Hz 累加）+ 时钟图标，恢复清零（v2 EyeNurseViewModel.cs:241-256、CountdownWindow.xaml:118-123）。
**托盘点击路径**（评审修订，清单 Q12）：菜单可见性**无条件**互换——v2 的 `PauseMenuItem_Click`（App.xaml.cs:137-143）先翻 `Visibility` 再调 `vm.Pause()`，Pause 失败（如休息中）菜单照样翻。v3：点击处理先 `tray.set_paused(...)`（无条件），再调 `AppModel::pause()`（展示态按返回值门控，D8）。

---

## 7. Windows API 与视口细节

### 7.1 显示器枚举 → 遮罩摆放
见 §5.5。v2 遮罩 = 每个 `MONITORINFO.rcWork` 一个窗口（`EyeNurseViewModel.cs:184-196`），Loaded 后 Maximized；v3 直接把视口位置/大小设为 rcWork（无需 maximize）。**不覆盖任务栏**（v2 就没盖住，保持一致，清单 Q5）。

### 7.2 WS_EX_TOOLWINDOW（悬浮条 Alt-Tab 隐藏）
v2 在 Loaded 时给 CountdownWindow 打 `WS_EX_TOOLWINDOW`（`CountdownWindow.xaml.cs:48-58`，动机即注释里的 Alt-Tab 隐藏 SO 帖）。**已核实**：winit 0.30 的 `with_taskbar(false)`/`set_skip_taskbar` 用的是 `ITaskbarList::DeleteTab`（winit `window.rs:1498-1530`），只移除任务栏按钮、**不会**从 Alt-Tab 隐藏。因此 v3 需要在 bar 视口创建后补一手：

```rust
// logic 里对目标视口执行（封装成幂等 helper，见下）：
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TOOLWINDOW};
let win = frame.winit_window().unwrap();                 // eframe 0.36: Option<&Arc<winit::window::Window>>（已核实）
let raw = win.window_handle().unwrap().as_raw();         // raw-window-handle 0.6（eframe winit feature "rwh_06"）
let RawWindowHandle::Win32(w) = raw else { return };
let hwnd = HWND(w.hwnd.get());
let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize) };
```

遮罩视口同理（v2 LockScreenWindow `ShowInTaskbar=False`，`LockScreenWindow.xaml:19`；Topmost 由 `with_always_on_top` 覆盖）。

**补丁必须自愈，不能只打一次（评审修订，实锤 winit v0.30.13）**：winit 的 `WindowState::set_window_flags` 在任何窗口标志变化时都会 `old_flags.apply_diff(window, new_flags)`，`apply_diff` 用 `to_window_styles()` 的结果**无条件回写** `SetWindowLongW(window, GWL_STYLE/GWL_EXSTYLE, ..)`（winit `window_state.rs:187-193` 与 `400-408`，本会话已拉源码核实）——winit 自己不管理 TOOLWINDOW 位，一次回写就把手动补丁抹掉。而 v3 恰好会触发标志变化：B5 的置顶切换走 `ViewportCommand::WindowLevel`（→ `set_window_flags`），切一次补丁即失效，悬浮条重新出现在 Alt-Tab。规定：把上面的补丁写成幂等 helper `ensure_toolwindow(hwnd)`，`logic` 每帧（1Hz，开销一次 `GetWindowLongPtrW`）对 root 与所有打开的遮罩执行“检测位不在 → 重打”；任何样式类 `ViewportCommand`（WindowLevel / Resizable / Decorations / Fullscreen / Minimized / Visible / MousePassthrough）发出后也立即执行一次。清单 B7 验收含“切换置顶后仍不可见”。

### 7.3 WTS 会话通知
见 §5.4。注册：`WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)`（只收本会话，等价 v2 `SystemEvents.SessionSwitch` 默认行为）。退出路径（评审修订：原 `spawn -> io::Result<()>` 签名无法注销，已改为句柄结构）：`run_native` 返回后主线程调用 `session_thread.request_stop()`（= `PostMessageW(hwnd, WM_CLOSE)`，跨线程投递合法）→ 窗口过程 `WM_CLOSE` 分支执行 `WTSUnRegisterSessionNotification` + `DestroyWindow` + `PostQuitMessage` → `join()`。

### 7.4 多显示器 DPI 换算（重要！）
egui/winit 的 `ViewportBuilder::with_position/with_inner_size` 接**逻辑点**——egui-winit 内部 `PhysicalPosition::new(pixels_per_point * pos.x)`（已核实 egui-winit `lib.rs:2272-2275`）。而 `GetMonitorInfoW` 给的是**物理像素**。规则：

```
logical = physical / target_monitor_scale_factor
```

- `scale` 获取：遍历 `frame.winit_window().unwrap().available_monitors()`，`monitor.name()` 与 `MonitorRects.device_name` 匹配后取 `monitor.scale_factor()`；找不到按 `1.0` 并记日志。
- 混合 DPI（如 4K@150% + FHD@100%）：**初始换算可能就是错的**（评审修订，实锤 egui-winit 0.36.2）：`apply_viewport_builder_to_window` 在窗口创建后立即执行（lib.rs:1983），其 `pixels_per_point` 取自窗口**创建落点所在显示器**——源码注释自认 "we didn't know on what monitor the window would appear"（lib.rs:2245 一带），即初始 position/inner_size 乘的是落点屏 scale 而非目标屏 scale；且 `resizable(false)` 锁死后系统不会替你重排。因此每个遮罩视口要做**收敛式校正，位置与尺寸两条命令都要发**：
  1. 目标物理矩形 = 目标显示器的 `rcWork`（§5.5）。
  2. 每帧读该视口 `ctx.input(|i| (i.viewport().outer_rect, i.viewport().native_pixels_per_point))`，当前物理矩形 = outer_rect × native_ppp。
  3. 误差 >1px 时按**当前** native_ppp 反推逻辑值，同时发 `ViewportCommand::OuterPosition(目标原点/native_ppp_now)` 与 `ViewportCommand::InnerSize(目标尺寸/native_ppp_now)`（注意 `InnerSize` 命令内部同样乘"当时的 ppp"，lib.rs:1759-1760 已核实——所以必须两条都发、用同一个当时的 ppp）。
  4. 移动到目标屏后 native_ppp 收敛为目标屏 scale，第 2-3 步再跑 1-2 帧即精确贴合 rcWork；`WinEvent::DisplayChanged`（§5.4）到达时对全部遮罩重跑本流程（清单 C1/B1 验收口径）。
- `UiJson` 里的 bar 坐标存**逻辑点**（v2 存 WPF DIP，量纲一致；跨 DPI 屏漂移属可接受偏差，清单 B6/Q9 注明）。
- winit 在创建事件循环时会自行声明 PerMonitorV2 DPI 感知（实现时于真机验证缩放是否生效；如需保险可由 D 提供 `app.manifest` 声明 PerMonitorV2，v2 manifest 即如此，`src/EyeNurse/app.manifest`）。

### 7.5 其余
- **单实例**：`CreateMutexW(None, false, w!("Local\\EyeNurse3"))`，`ERROR_ALREADY_EXISTS` → 退出（v3 新增，清单 J3）。
- **关于页**：`Process.Start(url)` → windows-rs `ShellExecuteW(None, w!("open"), url_wide, None, None, SW_SHOWNORMAL)`（`Win32::UI::Shell`，feature 已含在 §2 的 workspace 依赖清单；评审修订：原稿 §2 缺该项与本文矛盾）。
- **CJK 字体**（C，`ui/fonts.rs`）：egui 默认字体无汉字。启动时读 `C:\Windows\Fonts\msyh.ttc`（失败依次尝试 `simsun.ttc`、`simhei.ttf`），`egui::FontData { font: bytes.into(), index: 0 /*ttc 面号，已核实字段存在*/, tweak: Default::default() }`，`ctx.set_fonts(FontDefinitions{ font_data: 插入 "msyh", families: Proportional/Monospace 尾部追加 "msyh" })`。

---

## 8. i18n 与文案映射

| 键 | v2 来源 | v3 用途 |
|---|---|---|
| app_name / about / setting / pause / resume / reset / rest_now / exit | `Assets/Languages/*.json` + `App.xaml.cs:108-114` | 托盘菜单、窗口标题 |
| resting | 同上（`CountdownWindow.xaml:59-64`） | bar 休息中文案 |
| always_on_top | 同上（`CountdownWindow.xaml:85-88`） | bar 置顶复选 tooltip |
| clientSettings / runWhenStarts / resetWhenSessionUnlock / restInterval / restDuration / settings / panel / home / copyright / zh / en | Vue `locales/*.json` | 设置窗 |

切语言（设置窗下拉）：`UiAction::SetLanguage` → `i18n.set_language` + `Languages.json` 保存 + tray 文案刷新（v2 `ClientApi.SetCurrentLanguage`，ClientApi.cs:36-45）。

---

## 9. 四人并行开发顺序（建议）

1. **第 0 步（半小时，一起）**：B 建好 workspace、空 `AppModel`、空 `ui/*` stub，`cargo check` 全绿后推骨架；契约（§4/§5.6）冻结。
2. A：core 五模块 + 单测（不依赖任何人）。
3. B：tray + session 线程 + monitors + logic 流水线（用 stub UI 即可跑“托盘→倒计时打印”）。
4. C：fonts/theme/bar/mask（对着 stub 数据画，早期可假数据驱动）。
5. D：settings 视口 + 资源/图标/打包脚本 + 按 parity 清单出验收记录。
6. 联调里程碑：M1 托盘+bar 计时跳动；M2 rest_now 遮罩全屏+解锁；M3 会话锁暂停/重置；M4 设置窗读写配置字节级兼容（用 v2 的旧 json 文件冷启动验证）。

## 10. 风险与备选

- **deferred 视口 + `with_active(false)` 遮罩不抢焦点**：若 Windows 下仍抢焦点，备选 `show_viewport_immediate`（签名已核实）或 W 层面再补 `WS_EX_NOACTIVATE`（同 §7.2 手法，`GWL_EXSTYLE |= WS_EX_NOACTIVATE(0x0800_0000)`）。
- **事件延迟与唤醒（评审修订）**：方案 A（每秒轮询 receiver + 1Hz 心跳）只对**托盘菜单**够用（≤1s 感知延迟可接受）。跨视口 UI 动作（遮罩解锁按钮、设置保存、切语言）**不能**依赖心跳——deferred 视口的输入不会执行 root 的 logic（v2 对应路径全是即时的）——必须按 §6.1 在发送 `UiAction` 后用 `request_repaint_after_for(Duration::ZERO, ViewportId::ROOT)` 即时唤醒，目标延迟 ≤100ms（清单 C11 验收）。若该原语在个别环境不生效（风险低，未在本机验证），升级方案 B：`NativeOptions.event_loop_builder` 里拿 `EventLoopProxy`，`MenuEvent::set_event_handler` + `UiAction` 通道改由 proxy 唤醒（tray-icon 官方推荐 winit 用户走此路，已核实文档）。
- **eframe 0.36.2 刚发布**：若遇 glow/驱动回归，降级锚点 `eframe = "=0.33.*"`（同代 API 有 `update(ctx, frame)` 而非 `ui`/`logic` 拆分——届时以该版本文档为准改写 §5.6 的 trait 方法名，其余设计不变）。

## 11. 附录：外部库事实摘录（self-contained，无需访问 dzy-dotnet-tools）

v2 的持久化/自启实现在兄弟仓库 `GiantappMan/dzy-dotnet-tools`（GitHub 公开库；**本地未 checkout，实现者无需也无法在本机复核**——本节即事实快照，由文档作者于 2026-10-08 从 `https://raw.githubusercontent.com/GiantappMan/dzy-dotnet-tools/master/...` 实际拉取核实）：

1. **`src/Common.Apps/Services/ConfigService.cs`**（路径与文件名的全部依据）：
   ```csharp
   AppConfigDir  = $"{Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData)}\\{option.AppName}\\";
   //卸载时不清除
   UserConfigDir = $"{Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData)}\\{option.AppName}\\UserData";
   string configFile = Path.Combine(UserConfigDir, $"{typeof(T).Name}.json");   // Load 与 Save 同式
   ...
   return res ?? new T();   // 缺文件/反序列化失败 → 默认实例
   ```
   `AppName = "EyeNurse"` 来自 v2 `App.xaml.cs:37` 的 `IocService.Init(new InitServiceOption(){ AppName = nameof(EyeNurse) }, ..)`。
2. **`src/Common/Helpers/JsonHelper.cs`**（序列化选项的全部依据）：
   ```csharp
   var json = JsonSerializer.Serialize(obj, obj.GetType());   // 无任何 JsonSerializerOptions
   ```
   默认选项 ⇒ 属性名原样 PascalCase、紧凑无缩进、null 引用照写、TimeSpan 序列化为 "c" 格式字符串。`JsonDeserialize<T>` 捕获异常返回 `default`；Save 是"先序列化成功才 `File.WriteAllText`"，序列化异常则原文件不动。
3. **`src/Common.Apps/Helpers/DesktopStartupHelper.cs`**（HKCU 自启的全部依据）：
   ```csharp
   runKey = OpenRegKey(@"Software\Microsoft\Windows\CurrentVersion\Run", true);  // RegistryHive.CurrentUser
   runKey.SetValue(_Key, _executablePath);      // Set(true)：REG_SZ，路径不带引号
   runKey.DeleteValue(_Key);                    // Set(false)
   // Check()：遍历 GetValueNames()，item.Equals(_Key, StringComparison.OrdinalIgnoreCase)
   ```
   打开键时 `RegistryKey.OpenBaseKey(hive, Environment.Is64BitOperatingSystem ? RegistryView.Registry64 : RegistryView.Registry32)`。v2 传参 `("Eye Nurse 2", exe路径)` 见 `EyeNurseService.cs:34`。
4. **`User32Ex.GetMonitorInfos()`**（Common.WinAPI）：该文件**未逐行拉取核实**，但 v2 调用点语义明确（`EyeNurseViewModel.cs:46` 构造时取列表，cs:189-192 用 `rcWork` 摆窗）。v3 按 §5.5 的 `EnumDisplayMonitors` + `GetMonitorInfoW(MONITORINFOEXW)` 实现即可，对齐的是 v2 的可观察行为（每屏 rcWork 一个窗），无需与内部实现逐行一致。
