# EyeNurse v3.1 架构设计（应用壳从 egui/eframe 迁移到 Tauri 2）

> 本文是 `docs/v3-architecture.md`（egui 版）的**替换稿**，只改应用壳：eyenurse-core 与
> `win/{monitors,session,single_instance}.rs` 原样复用，`crates/eyenurse-app` 原地改造为
> Tauri 2 应用 crate。egui 版在真机上暴露的两个问题（拖动不畅、设置窗打不开，见 §12）
> 是本次迁移的直接动因。
> v2 行为引用沿用 egui 版的 `文件:行号` 口径；行为对齐清单见 `docs/v3-parity-checklist.md`
> （框架相关条目已在清单内标注 Tauri 注记）。
> 本文所有第三方 API 均为 2026-10-08 实际查证：tauri 2.12.1（crates.io `max_stable_version`）
> ，源码取自 github.com/tauri-apps/tauri `dev` 分支；CLI 工作目录语义用 `@tauri-apps/cli`
> 真机实测（§1.4，输出原文在文中）。

---

## 0. 目标与非目标

**目标**
- 行为与 `docs/v3-parity-checklist.md` 完全一致（egui 版已实现的行为逐项保留），只更换承载框架：
  - **设置窗 + 休息遮罩 → WebView 页面**（Tauri `WebviewWindow`，前端 Vite + Vue 3 + TS）；
  - **悬浮条 → 保留原生**（独立线程 + Win32 消息循环 + GDI 双缓冲自绘，§6）；
  - **托盘 → tauri 2 内置 tray**（`tauri::tray`，§7）。
- 配置文件与 v2 仍字节级双向兼容（core 未动，F1-F12 不受影响）。
- `[[bin]] name = "eyenurse"` 不变：产物路径仍是 `target/{profile}/eyenurse.exe`，
  `setup/installer.iss` 与 G2/G3 自启值数据语义不受影响。

**非目标**
- 不引入 tauri 插件（single-instance / autostart 等继续走自有 Win32 实现，§3）。
- 不改用 tauri 的 NSIS/WiX 打包（Inno Setup 流程保留，§10）。
- 不做 v2 没有的新功能。

---

## 1. 总体布局

### 1.1 目录树（Δ = 相对 egui 版的改动）

```
eye-nurse/
├─ Cargo.toml                      # workspace 根保留；[workspace.dependencies] 换 tauri 系（§1.3）
├─ package.json                    # bun 脚本入口：dev = tauri dev（§1.5）
├─ .gitignore                      # Δ 增补 ui/dist/、gen/schemas/；建议改为提交 bun.lock
├─ docs/
│  ├─ v3-architecture.md           # egui 版存档（被本文替换）
│  ├─ v3-tauri-architecture.md     # 本文
│  └─ v3-parity-checklist.md       # 对齐清单（框架条目已注记）
├─ assets/                         # logo.ico / logo.png / logo.{zh,en}.svg 不动
├─ ui/                             # 【新增】前端（bun 管理，Vite + Vue 3 + TypeScript）
│  ├─ package.json                 #   dev: vite（端口 1420 strictPort）；build: vite build
│  ├─ vite.config.ts               #   MPA：index.html（设置窗）+ mask.html（遮罩）
│  ├─ index.html                   # 设置窗页面（Vue 3）
│  ├─ mask.html                    # 遮罩页面（无框架，纯 TS：深底倒计时 + 解锁按钮）
│  └─ src/
│     ├─ settings/                 #   四项设置 + 侧栏 + 页脚 + 语言下拉（复刻 egui 版 ui/settings.rs 的视图与保存管线）
│     └─ mask/                     #   倒计时渲染（B2 分钟分量公式）+ 解锁 invoke
└─ crates/
   ├─ eyenurse-core/               # 【原样复用】唯一增量：I18n::texts()（§9）
   └─ eyenurse-app/                # 【原地改造为 Tauri 2 应用 crate】
      ├─ Cargo.toml                # Δ tauri / tauri-build 依赖 + custom-protocol feature（§1.3）
      ├─ assets/                   # 【删除】crate 内资源副本不再需要（图标/文案均引用仓库根与 core 真源）
      ├─ tauri.conf.json           # 【新增】§1.2（devUrl/beforeDevCommand/frontendDist 已实测）
      ├─ capabilities/default.json # 【新增】最小权限（§4.4）
      ├─ app.manifest              # 保留【D】（PerMonitorV2），改由 tauri-build 嵌入（§10）
      ├─ build.rs                  # Δ【D】tauri_build::try_build + theme.json 校验/代码生成（§3）+ 语言文件拷贝（保留）
      └─ src/
         ├─ main.rs                # Δ【B】启动序列 1-2.6 步不变，eframe 段换成 tauri::Builder
         ├─ core.rs                # 【新增】【B】CoreLoop 线程 = egui 版 app.rs AppModel 的状态机移植；
         │                         #    LOGO_PNG 常量随迁（原 app.rs:16 的 include_bytes! 不变）
         ├─ commands.rs            # 【新增】【D】IPC 命令（§4）
         ├─ events.rs              # Δ【B】WinEvent 不变；UiAction → CoreMsg（含应答通道）
         ├─ tray.rs                # Δ【B】tauri::tray 重写，菜单结构/互换/文案刷新对齐 egui 版 tray.rs
         ├─ ui/
         │  ├─ mod.rs              # Δ【B】pub mod bar; pub mod theme;
         │  ├─ bar.rs              # 【重写】【C】Win32 悬浮条线程（§6；行为对齐 egui 版 bar.rs）
         │  ├─ theme.rs            # 改写【C】（include! 生成常量 + ramp()；egui::Color32 → RGB/COLORREF）
         │  └─ theme.json          # 【新增】【C】配色单一真源（build.rs 校验+代码生成；页面 import 同一文件，§3）
         │  └─ fonts.rs            # 【删除】WebView2 自带 CJK 渲染（H5）
         │  └─ mask.rs             # 【删除】→ ui/mask 页面（§5）
         │  └─ settings.rs         # 【删除】→ ui/settings 页面 + commands.rs（§8）
         └─ win/                   # 【原样复用】【B】monitors.rs / session.rs / single_instance.rs
```

**所有权（沿用 egui 版 A/B/C/D 分工，随迁移重述）**：A = `eyenurse-core/**`（唯一增量
`I18n::texts()`）；B = `main.rs`、`core.rs`、`events.rs`、`tray.rs`、`ui/mod.rs`、`win/**`；
C = `ui/{bar.rs,theme.rs,theme.json}`；D = `commands.rs`、`ui/` 前端页面、`build.rs`、
`app.manifest`、`tauri.conf.json`、`capabilities/`、打包脚本。**无主文件清零**（评审修订）：
`build.rs` 虽同时含壳层构建调用（tauri_build）与 D 侧打包嵌入（图标/manifest/capabilities/
语言与配色拷贝），**整体归 D**——沿用 egui 版「build.rs/资源/打包归 D」的约定，B 需要的构建期
变更（新增 cfg/feature/代码生成）经 D 协商，不得直接改。**冻结契约规则不变**：跨所有者文件
只能由其所有者增删变体/字段（`events.rs` 的 `CoreMsg`、`commands.rs` 的 `SettingsDto`/命令面
改动归 B/D 协商，他人只消费）。

### 1.2 tauri.conf.json（写法已实测可跑通）

路径：`crates/eyenurse-app/tauri.conf.json`。**刻意不用 `src-tauri` 目录名**——CLI 的目录
发现是「从当前目录递归找含 `tauri.conf.json` 的子目录（max_depth=3）」（已核实
`crates/tauri-cli/src/helpers/app_paths.rs` `resolve_tauri_dir()`：先查 `TAURI_APP_PATH`
→ `<cwd>/tauri.conf.json` → `<cwd>/src-tauri/` → 递归查找），`crates/eyenurse-app` 深度为 2，
在仓库根执行 `bun dev` 即可被发现（§1.4 实测）。

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "EyeNurse",
  "mainBinaryName": "eyenurse",
  "version": "3.0.0",
  "identifier": "cn.giantapp.eyenurse3",
  "build": {
    "beforeDevCommand": { "script": "bun run dev", "cwd": "../../ui" },
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": { "script": "bun run build", "cwd": "../../ui" },
    "frontendDist": "../../ui/dist"
  },
  "app": {
    "windows": [
      {
        "label": "settings",
        "url": "index.html",
        "title": "EyeNurse",
        "width": 900,
        "height": 500,
        "resizable": true,
        "visible": false,
        "center": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"
    }
  },
  "bundle": { "active": false }
}
```

要点（全部查证过）：
- **`mainBinaryName: "eyenurse"`**：该字段「Overrides app's main binary filename」（tauri-utils
  `config.rs:4084-4092`）；CLI 主二进制定位默认取 cargo 产物名，显式写死与 `[[bin]] name`
  一致，保证 dev/build 找到的都是 `eyenurse.exe`（实测运行的就是 `target\debug\eyenurse.exe`）。
- **`beforeDevCommand`/`beforeBuildCommand` 用对象形式 `{script, cwd}`**：`cwd` 相对
  **tauri 目录**（`crates/eyenurse-app`）解析——CLI 在派发 hook 前先 `set_current_dir(dirs.tauri)`
  （`dev.rs:148` / `build.rs:166`），`Command::current_dir(相对路径)` 按父进程当下 cwd 展开。
  因此写法**与从哪个目录启动 CLI 无关**，比字符串形式（默认 cwd = frontend 目录，见 §1.4）更确定。
- **`devUrl` 端口 = vite 端口（1420，strictPort）**：CLI 会先 TCP 轮询该端口（90×2s 超时，
  `dev.rs:269-320`）再启动 cargo，vite 必须先起且端口不能漂移。
- **`frontendDist` 相对 tauri 目录（= tauri.conf.json 所在目录）**：`../../ui/dist`
  （`crates/eyenurse-app/../../ui/dist` = `<root>/ui/dist`）。解析基准已核实 tauri-codegen
  `context.rs:186`：`let assets_path = config_parent.join(path)`，且 `:188-190` 目录不存在时
  **generate_context! 直接 panic**（"The `frontendDist` configuration is set to … but this
  path doesn't exist"）——写错层级（如 `../../../` 指到仓库外）在 `bun run build` / `bun run
  smoke`（均带 `custom-protocol` 编译）第一时间失败；`tauri build` 的 CLI 侧检查
  （`build.rs:209-235`，禁止包含 `target`/`node_modules`）是第二道闸。`beforeBuildCommand`
  先行构建保证目录存在。
- **`bundle.active = false`**：不用 tauri 的 NSIS/WiX，`tauri build` 只做「前端构建 + cargo
  release（带 `custom-protocol`）」，Inno 流程继续（§10）。
- **settings 窗静态声明、`visible: false`**：应用启动即创建但隐藏，等价 v2 MainWindow 的
  Show/Hide 生命周期（§8），页面状态（未保存的编辑）天然不丢。
- `identifier` 取独立值 `cn.giantapp.eyenurse3`，避免与 v2 的 WebView2 用户数据目录混用。

### 1.3 Cargo 变更

workspace 根 `[workspace.dependencies]`：

```toml
[workspace.package]
rust-version = "1.95"   # 注释改为「当前工具链基线；tauri 2 MSRV 低于此值」——eframe 1.95 约束已消失

[workspace.dependencies]
tauri = { version = "2.12", features = ["tray-icon", "image-png"] }
tauri-build = { version = "2" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
crossbeam-channel = "0.5"
sys-locale = "0.3"
windows = { version = "0.62.2", features = [
  # ……原有 feature 全部保留（bar/monitors/session 用）……
  "Win32_UI_HiDpi",             # 【新增】GetDpiForWindow（bar 逻辑/物理换算）
] }
# 删除：eframe / egui / tray-icon / image
```

`crates/eyenurse-app/Cargo.toml`：

```toml
[features]
# tauri build 时由 CLI 传入；普通 cargo check/test 不启用（无需 ui/dist 即可编译）
custom-protocol = ["tauri/custom-protocol"]

[dependencies]
eyenurse-core = { path = "../eyenurse-core" }
tauri = { workspace = true }
crossbeam-channel = { workspace = true }
sys-locale = { workspace = true }
windows = { workspace = true }

[build-dependencies]
tauri-build = { workspace = true }
# 删除 embed-resource（图标/manifest 改由 tauri-build 嵌入，§10）
```

**⚠ DX 陷阱（务必写进 Build.md）**：dev 流程由 tauri CLI 驱动时是 `cargo run
--no-default-features`（`interface/rust.rs:425-441`：从 default features 里剔除会启用
`tauri/custom-protocol` 的项，实测命令原文 `cargo run --no-default-features --color always`；
release 则推入 `tauri/custom-protocol`，`interface/rust.rs:481`）。因此：
- `default = []`（不把 custom-protocol 放进 default）→ `cargo check / cargo test` 在没有
  `ui/dist` 的干净机器上照常通过（generate_context! 不嵌资产）；
- `compile.ps1` 的 `cargo build --release` 必须换成 `bun run build`（= `tauri build`，
  自动先建前端、再带 feature 编译），否则 release exe 嵌不到页面（§10）。

### 1.4 tauri CLI before 命令的工作目录语义【实测验证】

用与 §1.1/§1.2 相同的目录形态（根 `package.json` + `crates/eyenurse-app/tauri.conf.json`，
无 `src-tauri`）搭了最小工程，`bunx @tauri-apps/cli dev`（CLI 当前版）在 2026-10-08 实测两轮：

**第一轮：字符串形式 `"beforeDevCommand": "echo HOOK_CWD=%CD%"`，从仓库根执行：**

```
     Running BeforeDevCommand (`echo HOOK_CWD=%CD%`)
     Running DevCommand (`cargo  run --no-default-features --color always --`)
HOOK_CWD=C:\Users\RAZER\AppData\Local\Temp\tauri-cwd-test
        Info Watching ...\tauri-cwd-test\crates\eyenurse-app for changes...
     Running `target\debug\eyenurse.exe`
```

两个结论：① 非 `src-tauri` 布局（`crates/eyenurse-app`，深度 2）从仓库根能被发现；
② **字符串形式的 hook 默认 cwd = frontend 目录**（= 从 cwd 起最近一个含 `package.json` 的
目录，此处即仓库根——已核实 `dev.rs:150-189`：`let cwd = script_cwd.unwrap_or_else(||
dirs.frontend.to_owned())`，Windows 下经 `cmd /S /C` 执行）。

**第二轮：对象形式 `"beforeDevCommand": { "script": "echo HOOK_CWD=%CD%", "cwd": "../../ui" }`：**

```
     Running BeforeDevCommand (`echo HOOK_CWD=%CD%`)
HOOK_CWD=C:\Users\RAZER\AppData\Local\Temp\tauri-cwd-test\ui
```

相对 `cwd` 按 tauri 目录（`crates\eyenurse-app`）展开到 `<root>\ui`，与启动目录无关。
`tauri build` 的 `beforeBuildCommand` 同语义（`build.rs:166` 同样先 `set_current_dir(dirs.tauri)`，
hook 派发在 `helpers/mod.rs:68-108` 的 `run_hook`）。**§1.2 的写法即据此定稿。**

### 1.5 根 package.json（bun 脚本）

```json
{
  "name": "eyenurse",
  "private": true,
  "devDependencies": { "@tauri-apps/cli": "^2" },
  "scripts": {
    "dev": "tauri dev",
    "dev:web": "cd ui && bun run dev",
    "build": "tauri build",
    "build:web": "cd ui && bun run build",
    "smoke": "cd ui && bun run build && cd .. && cargo run -p eyenurse-app --features custom-protocol -- --smoke",
    "check": "cargo check --workspace",
    "test": "cargo test --workspace",
    "clippy": "cargo clippy --workspace --all-targets -- -D warnings"
  }
}
```

- `bun dev` → `tauri dev`：CLI 自身递归找到 tauri 目录 → 以 `{script, cwd}` 在 `ui/` 拉起
  vite（1420）→ 轮询 devUrl 就绪 → `cargo run --no-default-features` 跑 `eyenurse.exe`，
  页面走 devUrl，**vite HMR 生效**；cargo 侧改动由 CLI 的 watcher（观察 tauri 目录）重启。
- `tauri` 命令来自 devDependencies（bun 会把 `node_modules/.bin` 挂进 PATH）。
- 保留原 cargo 任务脚本（check/test/clippy），`dev:release`（`cargo watch` 热重载）删除——
  被 `tauri dev` 的 watcher 取代。

### 1.6 ui/ 前端

- `ui/package.json`：`dependencies: { "vue": "^3", "@tauri-apps/api": "^2" }`；
  `devDependencies: { "vite": "^7", "@vitejs/plugin-vue": "^6", "typescript": "^5" }`；
  `scripts: { "dev": "vite", "build": "vite build" }`（类型检查走 IDE/clippy 之外的
  `vue-tsc` 可选，不阻塞构建）。
- `vite.config.ts`：

```ts
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: { port: 1420, strictPort: true },   // tauri devUrl 硬依赖该端口
  build: {
    rollupOptions: {
      input: { settings: "index.html", mask: "mask.html" },  // MPA：两个窗口两个入口
    },
  },
});
```

- **遮罩页不用 Vue**：一个深底 + `mm:ss` + spinner + 解锁按钮的静态页（`mask.html` + 少量
  TS），减少每显示器一个 WebView 的内存与启动开销；设置窗用 Vue（复刻 egui 版
  ui/settings.rs 的视图结构）。
- 侧栏 logo 可以直接用 `logo.zh.svg` / `logo.en.svg`（WebView 能渲染 SVG；egui 版当时被迫
  用 png 平替的偏差随之消除）。

---

## 2. 线程与事件模型

egui 版的所有逻辑都跑在 winit 主线程的 `logic()` 里（靠 `request_repaint_after(1s)` 心跳）；
Tauri 版主线程让给 tao 事件循环，逻辑搬进**独立 CoreLoop 线程**，1Hz 节拍由
`recv_timeout` 到点驱动，不再依赖任何「重绘心跳」。

```
主线程（tao 事件循环）
├─ setup 钩子（启动一次）：
│   1) 构建 tray（tauri 要求托盘在主线程建）
│   2) spawn CoreLoop（把 AppHandle、启动序列产物移交）
│   3) spawn bar 线程（原生悬浮条，§6）
├─ tray/menu/tray-icon 回调：只 convert 成 CoreMsg → tx.send()，不在回调里干活
└─ on_window_event：settings 关闭请求 → prevent_close + hide（§8）；mask 关闭请求 → prevent_close

CoreLoop 线程（唯一持有业务状态的写者；= egui 版 AppModel 逐方法移植）
loop {
    match rx.recv_timeout(next_tick - now) {     // 1Hz 对齐；消息到达即刻醒来处理
        Ok(msg)  => apply(msg),                   // Tray/Bar/Win/IPC 应答……
        Err(Timeout) => {}
    }
    if now >= next_tick { next_tick += 1s; tick(now); }   // egui 版 tick_timer 原样移植
}

bar 线程（§6）：RegisterClassW("EyeNurse3BarWnd") → CreateWindowExW → GetMessageW 循环；
   1s WM_TIMER 读共享快照重绘；跨线程只经 Arc<Mutex<BarSnapshot>>（读）与 cmd tx（写方向）

session 线程（win/session.rs 原样）→ 转发线程（main 里 3 行）：
   forwarder: while let Ok(ev) = rx_win.recv() { core_tx.send(CoreMsg::Win(ev)) }
   —— 会话锁事件即时唤醒 CoreLoop（无轮询延迟），session.rs 自身零改动（§3）
```

- **通道**：`crossbeam_channel::unbounded` 一条 `CoreMsg` 总线 + 按需的
  `bounded(1)` 应答通道（IPC 命令需要返回值，v2 ClientApi 回包语义，§4.1）。
- **`CoreMsg`**（events.rs，归属 B）：
  - 动作：`Tray(TrayAction)`（托盘与 bar 右键菜单共用，含 `TrayAction::Settings` → 打开设置窗）
    / `Win(WinEvent)` / `RestDone` / `SetLanguage(String)` /
    `SaveSettings(SettingJson, Reply<AppliedSettings>)` / `ResetSettings(Reply)` /
    `SaveBarPosition { left: f64, top: f64 }` / `ToggleTopmost(bool)` / `ShowBar`；
  - **只读查询（评审修订：各携带 Reply，取数路径=CoreLoop 内存态，见 §4.1）**：
    `GetSettings(Reply<SettingJson>)` / `GetTexts(Reply<BTreeMap<String,String>>)` /
    `GetLanguages(Reply<Vec<String>>)` / `GetCurrentLanguage(Reply<String>)`——
    `get_texts` 是 settings/mask 页面的启动依赖，枚举里必须是头等变体而非隐含约定。
  枚举=egui 版 `UiAction + TrayAction + WinEvent` 的并集搬移。
  **不设 `Bar(BarCmd)` 与 `ShowSettings` 变体**（评审修订）：bar 线程持有 `Sender<CoreMsg>`
  直接发 `SaveBarPosition` / `ToggleTopmost` / `Tray(TrayAction::…)`；「打开设置窗」唯一入口是
  `Tray(TrayAction::Settings)`（托盘与 bar 菜单同源）——避免出现无人消费的死变体。
  **应答契约**：凡携带 `Reply` 的变体，CoreLoop 在**所有**处理路径（含错误路径，如写盘失败）
  必须恰好回包一次（`apply()` 内以包装器保证，漏发即 bug）；命令侧 `recv_timeout(2s)` 兜底
  （§4.1 骨架），超时映射为 Err 返回页面，不挂线程。
- **线程安全边界**：`AppHandle` 是 `Send + Sync`，CoreLoop 在非主线程做 `emit_to`、
  `WebviewWindowBuilder::build`、`destroy()` 都是受支持路径——tauri 明文警告的只有「**同步
  command / 事件回调里建窗口会死锁**」（`webview/webview_window.rs:52-58` 原文 "On Windows,
  this function deadlocks when used in a synchronous command and event handlers … You should
  use `async` commands and separate threads when creating windows"）。设计上把窗口创建/销毁
  **只**放在 CoreLoop 线程与 async command 里，规避该限制（§5）。
- **1Hz 显示驱动分工**：CoreLoop 每拍 `poll()` 计时器并更新共享快照 + `emit_to` 遮罩页；
  悬浮条用自己的 `WM_TIMER(1s)` 重绘（两条 1Hz 时钟独立，互不阻塞——bar 不依赖 tauri 事件循环，
  即使主线程繁忙倒计时也照跳）。

---

## 3. 复用清单（能复用就不重写）

| 模块 | 处置 | 新接法 |
|---|---|---|
| `win/monitors.rs` | **原样复用** | `get_monitor_infos()` 的物理 `rcWork` 在 `begin_rest` 里直接喂给遮罩窗口的 `set_position(PhysicalPosition)`/`set_size(PhysicalSize)`（§5.2）。egui 版「物理÷scale」换算整段废弃——tauri 的 setter 收物理值，混合 DPI 不再有换算收敛问题 |
| `win/session.rs` | **原样复用**（零改动） | 文件不动：`SessionThread` 仍产出 `Receiver<WinEvent>`。接线在 main 侧加一个 **3 行转发线程**：`while let Ok(ev) = rx_win.recv() { core_tx.send(CoreMsg::Win(ev)) }`——把 WinEvent 即时转入 CoreMsg 总线，CoreLoop 的唯一阻塞点就是这条总线，**SessionLock/Unlock/DisplayChanged 即时唤醒**（若按「每拍 try_recv」轮询，会话锁最坏延迟 1s，与 v2 的事件驱动不符，废弃该方案）；`request_stop()/join()` 退出序列原样保留在 main 尾部（转发线程随进程退出终止，无需显式 join） |
| `win/single_instance.rs` | **原样复用** | `CreateMutexW("Local\\EyeNurse3")` 仍在 main 第一行（tauri-plugin-single-instance 不引入：它的能力=跨进程聚焦转发，v3 只需防双开） |
| `eyenurse-core::timer::GlobalTimer` | **原样复用** | 移进 CoreLoop 线程所有；`poll(Instant)` 由 `recv_timeout` 1Hz 节拍喂（原 AppModel::tick_timer 逐行移植，含 Q1/Q2/Q3/怪癖 1/2/3 的调用方义务与 `is_paused` 门控） |
| `eyenurse-core::config::ConfigStore` | **原样复用** | 读路径只发生在 main 启动序列（CoreLoop 启动前）；写路径（UI.json 位置/置顶、Setting.json、Languages.json）全部由 CoreLoop 调用（单写者，消除 egui 版多视口并发写 UI.json 的隐患）。**进程内不存在与写并发的磁盘读**（评审修订）：`ConfigStore::save` 是非原子 `std::fs::write`（core config.rs:54-59），并发读可能撕裂 → serde 失败 → 静默回默认 → 下次保存把默认值落盘（破坏 F3/F12），因此运行期所有读取（get_settings/get_texts 等）一律经 CoreLoop **内存态**应答（§4.1），命令线程不直读磁盘；外部进程（用户用编辑器）读盘遇写瞬间撕裂属 v2 同款行为，不处理 |
| `eyenurse-core::i18n::I18n` | **复用 + 唯一增量** | 装配（磁盘优先/内嵌兜底，`load_i18n`）原样搬进 main。增量仅一个方法：`pub fn texts(&self) -> BTreeMap<String, String>`——当前语言整包文案（current→fallback 合并），供 `get_texts` 命令（§4.1）与 `available()` 语言下拉使用；core 其余文件零改动 |
| `eyenurse-core::autostart::DesktopStartup` | **原样复用** | 启动序列 2.5/2.6（migrate_from + set_enabled）原样；`check()` 的注册表实测值经 `get_settings`/`get_autostart` 命令暴露给设置页（Q10/F10） |
| `ui/theme.rs` → `ui/theme.rs + ui/theme.json` | **保留（真源重构，评审修订）** | **egui 版契约「D 的设置窗配色一律取本文件常量，不自定义第二套色板」（theme.rs:13 头注）随迁移废除**——但废除的不是单一事实源，而是它的载体：配色真源改为 **`crates/eyenurse-app/src/ui/theme.json`**（C 所有，Rust 与页面都能原生消费），机制兜底跨所有者漂移：① `build.rs` 读取并**严格校验**（`#RRGGBB` 正则 + 必备键存在，失败即 panic=编译期红），代码生成 RGB 常量到 OUT_DIR（`include!` 进 theme.rs——app crate 已无 egui，类型从 `egui::Color32` 改为 RGB 三元组/COLORREF，值不变）；② 页面侧 `ui/src` 直接 `import theme from "../../../crates/eyenurse-app/src/ui/theme.json"` 注入 `:root` CSS 变量（vite 默认 `server.fs.allow` = 仓库根，dev/build 均可用，无拷贝、无镜像）；③ 唯一允许的改色入口 = 改 theme.json，两态自动一致。`ramp()`/480s 渐变公式原样保留在 theme.rs 供 bar.rs 用。清单 C2/B9 的取色器验收口径不变 |
| `ui/fonts.rs` | **删除** | WebView2 直接用系统字体渲染中文（H5 的「豆腐块」风险消失）；bar 的 GDI 字体在 §6.2 自建 |
| `app.rs::AppModel` | **改写为 core.rs::CoreLoop** | 状态机方法（rest_now/pause/resume/reset_timer/tick_timer/apply_settings/end_rest_and_restart/会话锁处理）逐行移植；egui 专属部分（SharedUi 快照、viewport 注册、`ctx.request_repaint_after`、`heal_toolwindows` 每帧调用）按 §5/§6/§7 的新机制替换 |
| `tray.rs` | **重写（结构对齐）** | 菜单 id/顺序/初始隐藏/pause-resume 互换/切语言 set_text 全部保留，实现从 tray-icon 换 tauri::tray（§7） |
| `ui/bar.rs` | **重写为 Win32** | 行为逐项对齐 egui 版 bar.rs（尺寸/配色/mm:ss/时钟图标/悬停复选/右键菜单/拖动保存），三条实测教训以 Win32 机制落地（§6） |
| `ui/settings.rs` / `ui/mask.rs` | **删除 → WebView 页面** | 设置窗/遮罩的视图与保存管线分别落在 `ui/src/settings` 与 `ui/src/mask`（§5、§8） |

---

## 4. IPC 命令、事件与 capabilities

### 4.1 命令（对齐 v2 `Apis/ClientApi.cs` 语义，结构化参数取代 JSON 字符串）

全部 `#[tauri::command] async fn`（async 是硬要求：tauri 明文警告同步 command 里创建窗口
会死锁，见 §2；转发给 CoreLoop 的应答通道阻塞发生在 async runtime 工作线程上，可接受）。

| 命令 | 参数 → 返回 | v2 语义对应 |
|---|---|---|
| `get_settings` | `()` → `SettingsDto` | `GetSettings`：**CoreLoop 内存中的当前 SettingJson**（经 `CoreMsg::GetSettings` 应答——v2 同样取自内存服务态而非磁盘；也避免命令线程与 CoreLoop 的非原子写盘并发撕裂，§3）+ **RunWhenStarts 用注册表实测值覆盖**（Q10/F10，`DesktopStartup::check()` 无共享状态，在命令线程直查）；返回结构化对象而非 JSON 字符串 |
| `set_settings` | `SettingsDto` → `SettingsDto`（应用后的值） | `SetSettings`：字符串字段经 `HmsSpan::parse + clamp_like_v2`（= egui 版 ui/settings.rs `build_setting_json` 的移植，落在 commands.rs）→ `CoreMsg::SaveSettings` → 写盘 + 自启 + 计时器按新间隔重建（怪癖 1）→ **返回应用后 DTO，页面用回包刷新**（等价 v2 `Object.assign(settings, 回包)`） |
| `reset_settings` | `()` → `SettingsDto` | `ResetSettings`（F11）：`SettingJson::default()` 走同一应用路径，回包刷新 |
| `get_autostart` | `()` → `bool` | 新增专用口：`DesktopStartup::check()`——**唯一的例外**：注册表查询无共享状态，命令线程直调不经 CoreLoop（get_settings 内的覆盖值同源） |
| `get_current_language` | `()` → `String` | `GetCurrentLanguage`（CoreLoop 持有的 I18n 实例，Languages.json CurrentLan 解析规则同 I18n::build） |
| `set_current_language` | `lan: String` → `()` | `SetCurrentLanguage`：写 Languages.json + `i18n.set_language` + 托盘 `update_texts` + 设置窗标题 `set_title` + 广播 `i18n-changed` |
| `get_texts` | `()` → `BTreeMap<String,String>` | 新增整包文案：`I18n::texts()`（core 唯一增量，实例方法 → 经 `CoreMsg::GetTexts` 取），页面一次取全、语言切换时重取；**settings/mask 页面的启动依赖** |
| `get_languages` | `()` → `Vec<String>` | 新增：可用语言清单 = `I18n::available()`（经 `CoreMsg::GetLanguages`）——语言下拉（I5）的数据源，独立成命令而非塞进 `get_texts`（后者返回纯文案 map，装不下清单） |
| `rest_done` | `()` → `()` | v2 CloseCommand：遮罩解锁按钮 → `CoreMsg::RestDone` 提前结束休息（§5.3） |

**取数路径规则（评审修订）**：凡依赖 CoreLoop 独占状态（I18n 实例、当前 SettingJson）的命令，
一律走 §2 的 `Get*` 应答变体——commands.rs 不持有任何业务状态句柄，唯一例外是
`get_autostart` 的无状态注册表查询。

`SettingsDto`（commands.rs 定义，字段名与 SettingJson 一致，零映射）：

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct SettingsDto {           // serde 按字段名输出 PascalCase，与 Setting.json 同形
    pub RestInterval: String,      // .NET "c" 格式字符串（HmsSpan::to_c_string）
    pub RestDuration: String,
    pub ResetTimeout: String,
    pub RunWhenStarts: bool,       // get=注册表实测；set=开关值
    pub ResetWhenSessionUnlock: bool,
}
```

命令骨架（模式统一，以 set_settings 为例；应答超时与无条件回包契约见 §2）：

```rust
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

#[tauri::command]
async fn set_settings(state: tauri::State<'_, CoreHandle>, dto: SettingsDto) -> Result<SettingsDto, String> {
    let setting = dto.parse_and_clamp()?;             // HmsSpan::parse + clamp_like_v2（core 复用）
    let (tx, rx) = crossbeam_channel::bounded(1);
    state.send(CoreMsg::SaveSettings(setting, tx))?;  // 单写者：落盘+自启+计时器重建在 CoreLoop
    // recv_timeout 兜底：CoreLoop 存活但漏回包时，2s 后返回 Err 而不是永久挂住工作线程
    //（CoreLoop panic/退出路径会 drop Sender → rx.recv_timeout 返回 Disconnected → 同样 Err）
    let applied = rx.recv_timeout(REPLY_TIMEOUT).map_err(|_| "CoreLoop 未在时限内应答".to_string())?;
    Ok(SettingsDto::of(&applied))
}
```

### 4.2 事件（tauri `Emitter`，已核实 `lib.rs:961-1047`：`emit` / `emit_to(label)` / `emit_filter`）

| 事件 | 方向 | 载荷 | 用途 |
|---|---|---|---|
| `eyenurse://rest-tick` | CoreLoop → **emit_to 每个 mask 标签** | `{ remaining_ms: u64 }` | 休息倒计时 1Hz（仅休息期间发）；页面按 B2 公式渲染 mm:ss |
| `eyenurse://settings-changed` | CoreLoop → `emit` 广播 | `{}` | 设置保存/恢复默认后广播（v2 `SettingChanged`）；设置页重取 get_settings 刷新非编辑态 |
| `eyenurse://i18n-changed` | CoreLoop → `emit` 广播 | `{}` | 语言切换广播：页面重取 `get_texts`，原生侧（托盘/窗标题）由 CoreLoop 直接刷 |
| `eyenurse://settings-shown` | CoreLoop → `emit_to("settings")` | `{}` | 托盘「设置」唤起已隐藏窗口时通知页面（页面可选择重同步注册表实测勾选态，**不清编辑中的草稿**，§8.2） |
| `eyenurse://settings-closing` | CoreLoop → `emit_to("settings")` | `{}` | 用户点 X（prevent_close + hide 前发出）：页面把未到点的防抖保存立即 flush（§8.1） |

事件名合法字符为字母数字与 `- / : _`（已核实 `manager/mod.rs:495` 的 panic 注记），上述命名合规。

### 4.3 capabilities（最小集）

`crates/eyenurse-app/capabilities/default.json`：

```json
{
  "identifier": "default",
  "description": "页面仅监听事件；invoke 的自定义命令不受 ACL 门控（未声明 AppManifest 权限）",
  "windows": ["settings", "mask*"],
  "permissions": ["core:event:default"]
}
```

查证依据：
- `windows` 支持通配（`tauri-utils/src/acl/capability.rs:85,150-164`："`admin-*`" 式 glob），
  `mask*` 覆盖 `mask0/mask1/…`（窗口标签全用字母数字，避开标签字符集歧义）。
- `core:event:default` = `allow-listen / allow-unlisten / allow-emit / allow-emit-to`
  （`crates/tauri/permissions/event/autogenerated/reference.md:1-9`）——页面只需要 `listen`。
- **自定义命令为何不用配权限**：ACL 拦截条件是「插件命令 ‖ 应用声明了 ACL manifest ‖ 远程
  origin」（`webview/mod.rs:2046-2109` 原文注释 + 分支逻辑），而应用 manifest 只有在
  build.rs 里 `Attributes::app_manifest(...).commands(...)` 生成了权限时才存在
  （`tauri-build/src/acl.rs:409-422`）。本设计**不调用** `AppManifest::commands`，本地 origin
  的自定义命令不被 ACL 拦截。若将来要收紧（给命令配 `allow-` 权限），再显式启用即可。
- `core:default` 也可以用（超集，含 window/webview/app 等），这里按最小集只授 event。

---

## 5. 休息遮罩（每显示器一个 WebviewWindow）

### 5.1 页面契约（`ui/mask.html`，无框架）

- 背景 `#141418`、文字 `#EEEEF2`（C2，v2 LockScreenWindow.xaml 值）；内容垂直水平居中：
  70×70 CSS 旋转加载圈 + 叠放其上的 `mm:ss`（C3/C4）；
- 右下角 25×25 解锁图标按钮（CSS 内联 SVG 描绘锁形，C5），点击
  `import { invoke } from "@tauri-apps/api/core"; invoke("rest_done")`；
- 启动时 `invoke("get_texts")` 取文案并 `listen("eyenurse://i18n-changed")` 重取（文案走 i18n）；
- `listen("eyenurse://rest-tick")` 更新倒计时；**mm:ss 渲染公式必须复刻 B2/Q13**：
  `mm = Math.floor(totalSeconds / 60) % 60`（分钟分量，>59min 回绕），`ss = totalSeconds % 60`；
- `window.close` 被 Rust 侧拦截（见 5.3），页面无关闭入口。

### 5.2 创建（CoreLoop::begin_rest，每显示器一个）

```rust
// CoreLoop 线程执行（非主线程、非同步 command —— 避开 §2 的 Webview2 死锁场景）
for (i, mon) in monitors.iter().enumerate() {
    let label = format!("mask{i}");                       // 全字母数字
    let win = WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::App("mask.html".into()))
        .title("LockScreenWindow")                        // 沿用 v2/egui 版标题（冒烟与工具窗口识别）
        .decorations(false).resizable(false)
        .maximizable(false).minimizable(false).closable(false)
        .always_on_top(true)                              // v2 Topmost="True"
        .skip_taskbar(true)                               // 任务栏按钮隐藏（DeleteTab，注意 §5.4）
        .focused(false)                                   // 不抢焦点（v2 遮罩无激活观感）
        .shadow(false).visible(false)                     // 先藏后摆，避免 (0,0) 闪现
        .build()?;
    // 物理像素直接摆到 rcWork：builder 的 position/inner_size 是逻辑像素（已核实
    // webview_window.rs:861-873 "in logical pixels"），多屏混合 DPI 不用；改用物理 setter：
    win.set_position(PhysicalPosition::new(mon.rc_work[0], mon.rc_work[1]))?;
    win.set_size(PhysicalSize::new(mon.rc_work[2]-mon.rc_work[0], mon.rc_work[3]-mon.rc_work[1]))?;
    win::ensure_toolwindow(win.hwnd()?.0 as isize)?;      // Alt-Tab 隐藏补丁，§5.4
    win.show()?;
    self.mask_labels.push(label);
}
```

- **不覆盖任务栏**（rcWork，Q5 决议保留）；显示器列表**休息开始时现枚举**（Q4/C10 决议保留）。
- CoreLoop 持有 `mask_labels: Vec<String>`（`Emitter::emit_to` 与关闭都要用）。
- 休息中 `WM_DISPLAYCHANGE`（WinEvent::DisplayChanged）：重新 `get_monitor_infos()`，
  按新列表**差分**——新显示器补建遮罩、消失显示器的遮罩关闭（枚举空则保留旧值，逻辑与
  egui 版 app.rs:400-413 一致，只是「重注册视口」换成「补建/关闭窗口」）。

### 5.3 关闭（CoreLoop::end_rest，倒计时结束或解锁统一走这里）

```rust
for label in self.mask_labels.drain(..) {
    if let Some(w) = self.app.get_webview_window(&label) { let _ = w.destroy(); }
}
```

- **必须用 `destroy()`，不能用 `close()`**（评审修订）：`close()` 会像用户发起关闭一样**先发
  `WindowEvent::CloseRequested`**（`webview_window.rs:2344` 委托 `window.close()`；
  `window/mod.rs:1890` 文档原文 "It emits WindowEvent::CloseRequested first like a
  user-initiated close request so you can intercept it"），而本设计对 `mask*` 的
  `CloseRequested` 一律 `prevent_close()`（见下）——CoreLoop 自己的 `close()` 会被自己的
  拦截器吞掉，遮罩永远关不掉。`destroy()` 的文档原文："Similar to `Self::close` but
  **does not emit any events and force close** the window instead"
  （`webview_window.rs:2347-2350`），绕过事件、直接强关，正是程序化关闭要的语义。
  §5.2 的 DisplayChanged 差分关闭同样用 `destroy()`。关闭前先 `mask.open=false` 停发 rest-tick。
- 用户 Alt+F4 遮罩：`on_window_event` 对 `mask*` 的 `CloseRequested { api }` 一律
  `api.prevent_close()`（已核实 `app.rs:97-121` 的 `CloseRequestApi::prevent_close`）——
  该拦截只作用于用户/系统发起的关闭请求（`destroy()` 不走此路径），关闭唯一入口是解锁按钮
  与倒计时，行为确定（v2 遮罩不可激活，该路径本就不可达）。

### 5.4 WS_EX_TOOLWINDOW（B7 在 Tauri 下的结论）

**已核实 tao（tauri 的窗口层）`skip_taskbar` 用 `ITaskbarList::DeleteTab`，不是
WS_EX_TOOLWINDOW**（`tao/src/platform_impl/windows/window.rs:1543-1552`）——与 winit 同款
局限：只隐藏任务栏按钮，**Alt-Tab 仍可见**。egui 版的结论原样适用：遮罩创建后必须补打
`GWL_EXSTYLE |= WS_EX_TOOLWINDOW`（`win::ensure_toolwindow` 原函数复用，句柄来自
`WebviewWindow::hwnd()`，已核实 `webview_window.rs:1966`）。
与 egui 版的差异：**无需每帧自愈**——tauri 窗口在 v3 里没有任何运行时样式切换（置顶切换
只作用于原生悬浮条，§6.3），不存在 winit「窗口标志回写抹掉补丁」的触发器；创建时打一次即可，
`ensure_toolwindow` 保持幂等（位已在则跳过）。

---

## 6. 悬浮条（唯一保留的原生 GUI：独立线程 + Win32 + GDI）

### 6.1 窗口与线程

- `ui/bar.rs::spawn(cmd_tx, snapshot)` 在 setup 钩子里启动独立 `std::thread`
  （名字 `eyenurse-bar`）：`RegisterClassW("EyeNurse3BarWnd")` → `CreateWindowExW(
  WS_POPUP | WS_VISIBLE, WS_EX_TOOLWINDOW [| WS_EX_TOPMOST 按置顶态])` → `GetMessageW`
  循环（参照 session.rs 的现成实现，`GetMessageW` 返回值 `.0 > 0` 判定）。
- 窗口标题 `CountdownWindow`（与 egui 版一致；本版自持 hwnd，不再需要按标题枚举自愈）。
- 尺寸：**80×20 逻辑 × 窗口 DPI**。创建时按落点显示器 DPI（`GetDpiForWindow`，新 feature
  `Win32_UI_HiDpi`）缩放；`WM_DPICHANGED` → 按建议矩形重设尺寸 + 重建字体 + Invalidate。
- 位置：启动按 `UiJson.CoutdownWindowLeft/Top`（**逻辑 DIP**）摆放，**换算基准 = 保存点所在
  显示器的 DPI**：`MonitorFromPoint(保存点)` → `GetDpiForMonitor` 取 scale（与 v2 WPF 的
  DIP→物理语义一致：落点屏决定缩放），取不到回退 `GetDpiForSystem`。**所有内部几何以物理
  像素工作，只在读/写 UI.json 时换算**——教训 1 的「拖动路径逻辑/物理混用」从机制上消除；
  存取换算基准（存 = `GetWindowRect ÷ GetDpiForWindow`，取 = 落点屏 DPI）在跨异 DPI 屏的
  「拖动 → 重启」往返上仍可能有一次 scale 差异的漂移，属**记录在案的可接受偏差**（与 v2 的
  DIP 存储语义、egui 版清单 B6/Q9 的注记一致），不做跨屏重映射。

### 6.2 绘制（GDI 双缓冲，行为对齐 egui 版 bar.rs:22-226）

`WM_PAINT`：`CreateCompatibleDC/Bitmap`（物理尺寸）→ 按序画 → `BitBlt` 一次性上屏：
1. 底色 Beige `#F5F5DC`（BAR_PROGRESS_BG）；
2. 进度前景：宽度 = `remaining/interval`（满→空，B3；interval=0 时全宽，同 egui 版守卫）；
   颜色：暂停态 `ramp(#ECC071, #3E0D0D, paused_secs)`，否则 `#3AC32E`（Aero2 绿平替，theme.rs 注记的已知偏差原样保留）；
3. 文字：休息中=i18n `resting`（不受渐变影响，一致性差异 #6 原样）；暂停中=`PausedTime`
   mm:ss，颜色 `ramp(#080808, #F40000, paused_secs)`；平时=倒计时 mm:ss，黑色。
   mm:ss 公式 = `(total/60)%60 : total%60`（B2/Q13），与 egui 版 `format_mmss` 一致
   （该纯函数连同单测从 bar.rs 迁入新 bar.rs）；
4. 暂停态时钟图标：GDI 画笔绘表圈+两针（等价 egui 版 `paint_clock_icon`，15×15 逻辑位）；
5. 置顶复选（18×18 逻辑，右缘 1px 边距）：字形 `#444444`，悬停底 `#FFFFFF`。
   显隐规则复刻 egui 版：进入条内（`WM_MOUSEMOVE` + `TrackMouseEvent` 收 `WM_MOUSELEAVE`）
   或复选上悬停即浮现；移出 0.7s 宽限后 0.3s 淡出（`SetTimer(30ms)` 推进 alpha，
   `AlphaBlend` 半透明上屏；实现时可先做硬显隐+宽限定时器，淡入淡出为增强项）。
   tooltip 文案 = `always_on_top` 键（`WM_MOUSEMOVE` 命中区 + 托盘气泡或
   `TTM` tooltip 控件，实现取简）。

### 6.3 行为接线（三条实测教训的落地机制）

1. **拖动 = 系统拖动（教训 1 落地）**：`WM_NCHITTEST` 先 `DefWindowProc`，命中本窗口且不在
   复选命中区 → 返回 `HTCAPTION`。系统接管移动循环（含贴边/无跳动），代码**不做任何**逐帧
   `SetWindowPos`（egui 版每帧发 OuterPosition 的滞后一帧问题不存在了）。`WM_EXITSIZEMOVE`
   → `GetWindowRect`（物理）÷ `GetDpiForWindow`（逻辑）→
   `cmd_tx.send(CoreMsg::SaveBarPosition { left, top })` → CoreLoop 写 UI.json 并保存
   （Q9 决议「拖动结束即存」保留）。窗口样式与 `SetWindowPos(HWND_TOPMOST/NOTOPMOST)`
   全部由 CoreLoop 经 `WM_APP` 消息驱动（§6.4），bar 线程自持。
2. **右键菜单 = 原生弹出菜单（教训 2 落地）**：`WM_CONTEXTMENU` → `CreatePopupMenu`（状态
   变化时重建：`AppendMenuW` 按 egui 版 tray.rs 的同一组 id/顺序：about/setting/—/
   [pause|resume 互换]/reset/rest_now/—/exit，文案取共享快照）→
   `TrackPopupMenuEx(TPM_RETURNCMD)`（模态、返回所选 id，**顶层弹出，绝无 80×20 表面内
   裁剪问题**）→ `cmd_tx.send(CoreMsg::Tray(action))`（与托盘共用同一动作入口，对齐 v2
   「悬浮条 ContextMenu = App.Menu 同一对象」的语义；互换展示态读快照
   `paused_menu_shown`，由 CoreLoop 在与托盘 `set_paused` 同一位置同步）。
3. **创建路径可冒烟（教训 3 落地）**：见 §11。

### 6.4 与 CoreLoop 的接口

- **CoreLoop → bar**（两条路，各司其职）：
  1. **数据**：`Arc<Mutex<BarSnapshot>>`（`countdown/interval/paused/resting/
     paused_menu_shown/topmost/texts`），CoreLoop 每拍与状态变化时更新；bar 的
     `WM_TIMER(1s)` 里读快照 + `InvalidateRect` 重绘；
  2. **事件**：bar 线程启动时把 hwnd 写入 `Arc<OnceLock<isize>>` 回传（**复用
     `win/session.rs` 的现成模式**，bar.rs::spawn 返回该句柄结构），CoreLoop 据此
     `PostMessageW` 投递 `WM_APP_TOPMOST(bool)`、`WM_APP_LANGUAGE`（文案热更新）、
     `WM_APP_SHOW`（托盘双击 → 按 UI.json 持久化 TopMost/位置恢复显示，B11 对齐
     v2 ShowCountdownWindow；值由 CoreLoop 先写入共享快照再投递）；bar 线程只泵
     `GetMessageW`，无通道消费点，所有跨线程动作都经消息或快照。
- **bar → CoreLoop**：`cmd_tx: Sender<CoreMsg>`（`SaveBarPosition` / `ToggleTopmost` /
  `Tray(TrayAction::…)`，直发、无包装变体，见 §2）。
- 常量复用：`crate::core::LOGO_PNG`（随 AppModel 迁移落入 core.rs，原
  `app.rs:16` 的 `include_bytes!("../../../assets/logo.png")` 不变）、`ui::theme`
  （配色/渐变，`ui/mod.rs` 相应保留 `pub mod theme`）、`win::ensure_toolwindow`
  （bar 的 WS_EX_TOOLWINDOW 创建时直写即可，同函数复用）。

---

## 7. 托盘（tauri 2 内置 tray）

`tray.rs` 重写，结构与 egui 版 tray.rs 一一对应（菜单 id/顺序/动作映射 `menu_action` 原样保留）：

```rust
// setup 钩子内（主线程）：
let menu = Menu::new(app)?;
for item in [&about, &setting] { menu.append(item)?; }
menu.append(&PredefinedMenuItem::separator(app)?)?;
let pause = MenuItem::with_id(app, "pause", &texts.pause, true, None::<&str>)?;
let resume = MenuItem::with_id(app, "resume", &texts.resume, true, None::<&str>)?; // 建好不插入（E2）
menu.append(&pause)?; menu.append(&reset)?; menu.append(&rest_now)?;
menu.append(&PredefinedMenuItem::separator(app)?)?;
menu.append(&exit)?;
let tray = TrayIconBuilder::with_id("eyenurse")
    .icon(Image::from_bytes(crate::core::LOGO_PNG)?)  // feature "image-png"（image/mod.rs:214）
    .tooltip(&texts.app_name)
    .menu(&menu)                                      // 签名 menu<M: ContextMenu>(&M)（tray/mod.rs:241）
    .show_menu_on_left_click(false)                   // 2.2 起的正确名（menu_on_left_click 已废弃）
    .on_menu_event(move |_, ev| { if let Some(a) = menu_action(ev.id().as_ref()) { tx.send(CoreMsg::Tray(a)).ok(); } })
    // tx2 = tx.clone()，在两个闭包前取好
    .on_tray_icon_event(move |_, ev| { if matches!(ev, TrayIconEvent::DoubleClick { .. }) { tx2.send(CoreMsg::ShowBar).ok(); } })
    .build(app)?;
```

- **pause/resume 互换**：与 egui 版同一限制——tauri 的 `MenuItem` 只有 `set_text`/
  `set_enabled`（已核实 `menu/normal.rs:82-105`，**无 set_visible**），沿用
  `menu.remove(当前) + menu.insert(另一项, pause_index=3)`（已核实 `menu/menu.rs:320,344`），
  幂等守卫（`paused_shown` 状态标记）原样移植（egui 版 tray.rs:201-213 的教训：muda insert
  不查重，重复插入会出现两项「暂停」）。Q12/D5 语义不变：菜单互换无条件，展示态按 core 门控。
- **on_menu_event 的广播性**：官方文档注明该回调「对任何菜单事件触发，无论来自哪个窗口或
  托盘」（`tray/mod.rs:328` 注释原文）——本设计只有托盘一个 tauri 菜单（bar 的右键菜单是
  独立 Win32 HMENU），不会误收。
- **切语言**：CoreLoop::set_language 里 `set_text` 七项 + tooltip 重设（E7）；设置窗标题
  `get_webview_window("settings").set_title(&texts.app_name)`（v2 XAML 绑定 app_name 的等价）。

---

## 8. 设置窗（WebView 版；不再复用 egui 视口路径）

egui 版「主界面打不开」的根因虽未查明，但本版设置窗改走 WebView（静态声明窗口 + hide/show），
**不复用 egui deferred viewport 的注册/销毁路径**；打开/聚焦/还原语义对齐 v2
`MainViewModel.ShowWindow`（已存在则还原+激活，不丢未保存编辑）。

### 8.1 生命周期

- **创建**：tauri.conf.json `app.windows[0]`（label `settings`，900×500，`visible:false`，
  `center:true`）——应用启动即创建，等价 v2 MainWindow 构造即存在、Show/Hide 切换的形态。
- **打开（`CoreMsg::Tray(TrayAction::Settings)`，托盘菜单与 bar 右键菜单同源；CoreLoop 内部
  走统一的 show_settings 路径）**：

```rust
if let Some(w) = app.get_webview_window("settings") {
    if !w.is_visible()? { w.show()?; }   // 首次显示
    w.unminimize()?;                      // 已存在则还原（MainViewModel.ShowWindow: Minimized→Normal）
    w.set_focus()?;                       // 激活（!IsActive→Activate）
    let _ = app.emit_to("settings", "eyenurse://settings-shown", ());
}
```

  窗口从进程启动起常驻，页面（Vue）只在加载时执行一次 `get_settings`——**重开不重载页面、
  不重置草稿**：未保存的编辑原样保留（Ask 硬性要求）。页面收到 `settings-shown` 后只重同步
  「注册表实测勾选态」（Q10）这类外部易变值，不碰文本框内容。
- **关闭（点 X）**：`on_window_event` 对 `settings` 的 `CloseRequested` →
  `api.prevent_close()` + `emit_to("settings", "eyenurse://settings-closing")`（页面立即
  flush 未到点的防抖保存）+ `w.hide()`。若 WebView 隐藏后定时器被节流导致 flush 丢失，
  与 v2 关窗丢最后编辑的行为等价，可接受。
- **退出（托盘「退出」）**：CoreLoop 收 `Tray(Exit)` → `app.exit(0)`；main 在 run 返回后执行
  与 egui 版相同的 `session.request_stop(); session.join();`（UI.json 的位置/置顶兜底保存由
  CoreLoop 在退出路径写盘）。

### 8.2 页面（`ui/index.html` + `src/settings/`）

- 视图复刻 egui 版 ui/settings.rs 的决定：默认落在设置页、可折叠侧栏（localStorage 换
  内存态即可）、页脚社交链接 + 语言下拉 + copyright、保存中 spinner。
- 保存管线（I3）：控件提交（失焦/回车/开关点击）→ JS 侧 300ms 防抖 → `invoke("set_settings",
  { dto })` → 用回包刷新显示（v2 `Object.assign(settings, 回包)` 等价）；「保存中」以
  invoke 未决期为准。
- 文本 → 数值的 `parse + clamp` 在命令侧（core HmsSpan，I4/F6 语义与 egui 版
  `parse_or_default` 一致：非法回默认、负值取幅、≥1 天丢天取分量）；页面不重复实现解析，
  仅做输入占位 `HH:mm:ss`。
- 语言下拉选项 = `get_current_language`（选中值）+ `get_languages`（清单，`I18n::available`）；
  选中即 `invoke("set_current_language")`。

---

## 9. i18n 与文案

- 文案真源不变：`crates/eyenurse-core/assets/languages/{zh,en}.json`，build.rs 拷贝到
  `target/{profile}/Assets/Languages/`（H1，函数原样保留在 build.rs），运行时磁盘优先、
  内嵌兜底（`load_i18n` 原样搬进 main.rs）。
- 消费方式分三层：
  1. **原生层**（托盘菜单/tooltip、bar 文案、窗口标题）：CoreLoop 持有 `I18n`，
     `set_language` 时 `tray.update_texts`（set_text ×7）、bar 经 `WM_APP_LANGUAGE` 热更、
     设置窗 `set_title`；
  2. **页面层**：`get_texts()` 整包取（含 current→fallback 合并），`i18n-changed` 广播重取；
  3. **兜底**：页面在 devUrl 资产未就绪等极端情况下显示 key 原文，与 I18n::get 的第三级回退一致。
- core 唯一改动：`I18n::texts()`（§3）。

---

## 10. 构建、资源与打包

- **build.rs**：`tauri_build::try_build(Attributes::new().windows_attributes(
    WindowsAttributes::new().app_manifest(include_str!("../app.manifest"))
    .window_icon_path("../../assets/logo.ico")))`——已核实
  `WindowsAttributes::app_manifest<S: AsRef<str>>`（tauri-build `lib.rs:451`，注释明确支持
  `include_str!` 外置文件）与 `window_icon_path`（.ico 嵌入窗口图标，官方示例同款）。
  PerMonitorV2 的 app.manifest 原文件保留（§7.4/清单 B 系列的 DPI 语义不变）。语言拷贝
  函数原样保留；build.rs 另新增 theme.json 的格式校验与 Rust 常量代码生成（§3，归属 D）。
- **capabilities 目录**：`tauri-build` 默认扫 `./capabilities/**/*`（cwd=crate 根，已核实
  `acl.rs:446-458`），即 `crates/eyenurse-app/capabilities/default.json`。
- **`tauri build`（= `bun run build`）**：beforeBuildCommand 建 `ui/dist` → cargo release
  （推入 `tauri/custom-protocol`，资产嵌入 exe）→ bundle.active=false 不出安装器。
  产物仍为 `target/release/eyenurse.exe`。
- **`setup/compile.ps1`**：`cargo build --release -p eyenurse-app` 一行替换为
  `bun run build`（原因见 §1.3 陷阱：裸 cargo release 不带 custom-protocol 时不嵌页面）；
  其余（publish 暂存、Assets/Languages 拷贝、ISCC installer.iss）不动。
- **`.gitignore` 增补**：`ui/dist/`、`crates/eyenurse-app/gen/schemas/`（capability 校验
  schema 为生成物）；`node_modules/` 已有通配可覆盖 `ui/node_modules`。建议把 `bun.lock`
  从忽略表移除并入库（前端依赖锁定，`bun.lockb`/`package-lock.json` 仍忽略）。
- **运行时依赖**：Windows 10/11 + WebView2 Runtime（v2 WPF 版已依赖 WebView2，部署前提不变；
  Tauri 2 官方支持矩阵为 Win10+，与 docs/Build.md 的现状一致）。

---

## 11. 冒烟验证（教训 3：每个窗口的创建路径必须可被冒烟验证）

`--smoke` 启动旗标（main 里解析，仅在 debug/CI 语境使用；release 行为不变），依次验证三类
窗口并把结果打到 stdout、以退出码表达：

1. **settings**：`get_webview_window("settings")` 存在（启动即建，隐藏）→ 走一遍 §8.1 的
   show 路径 → 断言 `is_visible()==true` 且 hwnd 非空 → `hide()`；
2. **mask**：`get_monitor_infos()` 取主屏 rcWork → 走 §5.2 的创建路径建 `mask0` → 断言
   窗口存在、`always_on_top` 生效、EXSTYLE 含 WS_EX_TOOLWINDOW → `destroy()`（与生产关闭
   路径一致，顺带验证 §5.3 的 destroy 语义）；
3. **bar**：`FindWindowW("EyeNurse3BarWnd", NULL)` 非空 → 断言可见与尺寸 = 80×20×DPI
   （±1px）。

运行方式（配 `bun run smoke` 脚本，§1.5）：

```sh
bun run smoke
# = 先 vite build 出 ui/dist，再 cargo run -p eyenurse-app --features custom-protocol -- --smoke
#   （必须带 custom-protocol：不带时 dev 形态运行时指向 devUrl，会依赖 dev server）
```

期望输出（样例）：`SMOKE settings=ok mask=ok bar=ok`，进程退出码 0；任一失败为非 0 并指明
窗口名。这条命令同时验证「带 `custom-protocol` 的资产嵌入形态」（页面真正加载，而非错误页）
与三个创建路径——egui 版「设置窗真机打不开而纯代码评审没发现」的教训制度化：**冒烟脚本进
验收基线**（清单 §验收基线第 7 条）。

---

## 12. 已知实测反馈的处置对照

| 反馈 | egui 版根因 | 本版处置 |
|---|---|---|
| 拖动不畅（每帧 OuterPosition 滞后一帧 + 逻辑/物理像素混用致高 DPI 跳动） | egui 手写拖动 | bar 为自有 Win32 窗口：`WM_NCHITTEST→HTCAPTION` 系统拖动 + `WM_EXITSIZEMOVE` 落盘；拖动路径的混用从机制上根除；UI.json 存取换算基准（存取各自的 DPI 来源）与跨异 DPI 屏漂移口径见 §6.1 |
| 主界面（设置窗）打不开（根因未查明） | egui deferred viewport 注册路径 | 设置窗改为 WebView（静态声明 + hide/show），不复用该路径；ShowWindow 语义（还原+激活、不丢编辑）对齐 v2（§8）；创建路径纳入冒烟（§11） |
| 右键菜单被 80×20 窗口裁剪 | egui `context_menu` 画在视口表面内 | 原生 `TrackPopupMenuEx` 顶层弹出（§6.3-2） |
| （附带）遮罩倒计时冻结风险 | egui 需手动逐视口 `request_repaint_after` | 页面由 `rest-tick` 事件驱动 + CSS 无重绘依赖；bar 由自身 WM_TIMER 驱动，均不依赖 tauri 主循环心跳 |

---

## 13. 风险与备选

- **WebviewWindow 创建时序**：CoreLoop 非主线程建窗依赖「窗口创建派发到事件循环」，tauri
  对非主线程创建是支持路径（死锁警告只针对同步 command/事件回调，§2）。若个别环境出现
  创建卡顿，备选：CoreLoop 把建窗动作 `app.run_on_main_thread` 包一层（API 已核实存在于
  `AppHandle`），语义不变。
- **隐藏 WebView 的定时器节流**：设置窗隐藏期间 300ms 防抖 flush 可能被 WebView2 节流；
  与 v2 行为等价（关窗丢最后一次未 blur 的编辑），不做额外对抗；如需收紧，可在 closing
  通知后同步 `invoke` 一次（页面 flush 改为同步路径）。
- **AlphaBlend 淡入淡出**：bar 复选的渐显若实现成本高，先以「显隐 + 0.7s 宽限定时器」交付
  （B4 验收核心是悬停浮现/移出消失，淡化为观感项）。
- **WebView2 运行时成为部署前提**：相对 egui 版是**新增**依赖（与 v2 WPF 版相同的前提；
  Win11 与较新的 Win10 自带）。**已落地**：`setup/installer.iss` 的 `InitializeSetup`
  检测 Evergreen Runtime 注册表指纹（HKCU / HKLM 原生视图 / WOW6432Node 的
  EdgeUpdate Clients `{F3017226-…}`，pv 非空且非 0.0.0.0），缺失时询问用户并下载官方
  Evergreen Bootstrapper（fwlink 2124703）安装、复检，仍缺失终止安装；tauri 的
  `bundle.windows.webviewInstallMode` 只作用于自带打包器，不适用 Inno 流程，故走 iss
  内 `[Code]` 引导。`installWebview2Tips`/`installNow` 文案键因此继续保留在语言文件里。
- **版本锚点与复验义务**：tauri = "2.12"（2026-10-08 crates.io `max_stable_version` =
  2.12.1，本日复核仍为 2.12.1；`newest_version` 已出现 3.0.0-alpha，**3.x 不在本文范围**）。
  §1.2 的 `{script,cwd}` 相对 cwd 语义、devUrl 轮询、`--no-default-features` 行为与 §1.4 的
  实测输出均为**单机测量**（CLI 当前版，2026-10-08）——属于一次性验证而非长期保证：实现期
  升级 2.x 小版本时，若 dev 流程行为异常，第一优先级是按 §1.4 的最小工程配方重跑两轮
  echo 实验（成本 <5 分钟）再定位，不盲信本文结论。`tauri = "2.12"` 锁次版本位，锁死用
  `=2.12.*`。
