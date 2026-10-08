# 开发（v3 · Rust Cargo workspace）

## 环境要求

- Windows 10 / 11
- Rust 1.95+（`rustup` 安装；eframe 0.36 要求 MSRV 1.95）
- MSVC 工具链（Visual Studio 2022 Build Tools，提供 `link.exe`）

## 日常命令（仓库根执行）

```sh
cargo check --workspace     # 快速编译检查
cargo build --workspace     # 构建（debug，带控制台日志，便于看输出）
cargo run -p eyenurse-app   # 直接运行（release 构建无控制台黑窗）
cargo test -p eyenurse-core # core 单元测试（时间格式 / 状态机 / 配置字节兼容）
cargo build --release       # 发布构建（thin LTO + strip）
```

产物：`target/{debug,release}/eyenurse.exe`。

## 运行期数据

- 配置：`%LOCALAPPDATA%\EyeNurse\UserData\{Setting,UI,Languages}.json`，与 v2 **完全双向兼容**（同路径、同字段、同序列化字节），v2 升级 v3 无感迁移。
- 文案：exe 旁 `Assets/Languages/*.json`（可替换即换文案）；缺失时回退到程序内嵌副本（真源：`crates/eyenurse-core/assets/languages/`）。

## 打包

- exe 图标（`assets/logo.ico`）与 PerMonitorV2 manifest 的资源嵌入：`crates/eyenurse-app/build.rs` 经 `embed-resource`（build-dependency，调 MSVC 的 `rc.exe`）在每次 Windows 构建时完成，无需额外步骤。
- exe 旁 `Assets/Languages/{zh,en}.json`：`cargo build` 时由 build.rs 从单一真源 `crates/eyenurse-core/assets/languages/` 拷到 `target/{profile}/Assets/Languages/`（清单 H1）。
- 一键打包（清单 J4）：`setup/compile.ps1` —— release 构建 → 暂存 `setup/publish/` 绿色包（exe + Assets/Languages）→ 检测到 Inno Setup 时调 `setup/installer.iss` 生成安装包（输出 `setup/dist/`）。
