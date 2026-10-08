# 眼睛护士 3（EyeNurse v3）

一个小软件提醒你保护视力：定时全屏休息遮罩 + 悬浮倒计时条 + 托盘常驻。**v3 用 Rust 从零重写**（纯原生桌面应用，不再内嵌 WebView2/Vue 前端），配置文件与 v2 **完全双向兼容**（同路径、同字段、同序列化字节），v2 升级 v3 无感迁移。

> 架构设计：[docs/v3-architecture.md](docs/v3-architecture.md) · 行为对齐清单：[docs/v3-parity-checklist.md](docs/v3-parity-checklist.md) · 构建说明：[docs/Build.md](docs/Build.md)

## 功能

- 托盘常驻：关于 / 设置 / 暂停 / 恢复 / 重置 / 立刻休息 / 退出（与 v2 菜单一致）
- 悬浮倒计时条（80×20，无边框，Alt-Tab 隐藏，可拖动、可置顶）
- 全屏休息遮罩（每显示器一个，居中 mm:ss 倒计时 + 解锁按钮）
- 设置窗（egui 视口）：休息间隔/时长、开机启动、锁屏后重置、中英文切换
- 会话锁联动：锁屏暂停计时，解锁超时自动重置
- 单实例运行、WTS 会话通知、HKCU 自启（含 v2 `Eye Nurse 2` 值名静默迁移）

## 从源码构建

环境：Windows 10/11 + Rust 1.95+（MSVC 工具链）。

```sh
cargo check --workspace      # 编译检查
cargo build --workspace      # debug 构建
cargo run -p eyenurse-app    # 运行（debug 带控制台日志）
cargo test -p eyenurse-core  # core 单元测试
cargo build --release        # 发布构建 → target/release/eyenurse.exe
```

## 配置与文案

- 配置：`%LOCALAPPDATA%\EyeNurse\UserData\{Setting,UI,Languages}.json`——与 v2 完全一致，可直接沿用旧文件。
- 文案：exe 旁 `Assets/Languages/*.json`（替换即换文案）；缺失时回退程序内嵌副本（真源：`crates/eyenurse-core/assets/languages/`）。

## 仓库结构

```
crates/
├─ eyenurse-core/   # 纯逻辑：TimeSpan 兼容、计时状态机、字节级兼容配置、自启、i18n
└─ eyenurse-app/    # 桌面壳：eframe/egui、托盘、WTS 会话线程、显示器枚举、悬浮条/遮罩/设置窗
assets/             # logo.png / logo.ico / 设置页 logo.zh.svg / logo.en.svg（取自 v2）
docs/               # v3 架构文档、parity 清单、构建说明
```

# 下载

[微软商店（v2）](https://apps.microsoft.com/store/detail/eye-nurse-2/XP9K4SFKVKX9SS)

# 源码

https://github.com/DaZiYuan/eye-nurse
https://gitee.com/DaZiYuan/eye-nurse

# 欢迎任何语言的贡献，中文/英文/或者任何语言~

- PR / 想法 / Bug
