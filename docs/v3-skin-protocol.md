# v3.2+ 开放皮肤协议（EyeNurse Skin Protocol, ESP-1）

> 状态：**v1 已实现**（2026-10-09）。本文定义一套**开放的 UGC 皮肤协议**：
> 用户/第三方可以整体替换**桌面计数器（悬浮条）**与**锁屏/休息遮罩**的界面
> 形态并打包分发，而不只是 v3.1 的颜色键换肤。
>
> 实现落点：core 解析 `crates/eyenurse-core/src/skin.rs`（manifest/扫描/逐
> target 回退）；协议服务与 barweb 壳窗 `crates/eyenurse-app/src/skin_proto.rs`；
> 壳页桥 `ui/src/shared/skinHost.ts`（mask / barweb 共用）；示例皮肤
> `assets/skins-example/sunset-clock/`（build.rs 拷到 exe 旁 Assets/Skins）。

## 0. 问题定义

v3.1 现状（`eyenurse-core/src/skin.rs`）：皮肤 = 11 个颜色键（bar 9 + mask 2），
逐键回退 classic；来源优先级 custom > 磁盘 `Assets/Skins/*.json` > 内置 4 预设。
**结构与行为焊死**：bar 是 Win32+GDI 原生自绘（80×20 进度条，`ui/bar.rs`），
lock 是 WebView（`ui/src/mask/`）。用户能换色，不能换形态。

目标：皮肤可以**替换整个界面**——布局、字体、图形、动画、交互元素均可自定义，
以文件夹/压缩包为单位分发（网盘、社区帖、未来的市场均可），装上即用。

## 1. 总体设计：两级协议 + 表面（target）模型

### 1.1 两级格式（同一 `skin.json`，向后兼容）

| 级别 | 形态 | 载体 | 现状 |
|---|---|---|---|
| **Tier 0 颜色键** | `*.json` 裸文件（v3.1 格式） | `Assets/Skins/<id>.json` | 已实现，原样保留 |
| **Tier 1 HTML 表面** | 目录包（manifest + HTML/JS/CSS + 资源） | `Assets/Skins/<id>/` | 本协议新增 |

Tier 0 是 Tier 1 的子集：目录包可以只带颜色（等价 Tier 0），也可以为
`bar` / `lock` 两个 **target** 各自提供 HTML 入口。**每个 target 独立声明、
独立回退**——皮肤只做了锁屏，悬浮条自动落回 classic；反之亦然。这是 v3.1
"逐键回退"哲学在表面粒度上的延续。

### 1.2 关键架构决策：bar 按需升级为 WebView 承载

锁屏窗已是 WebView，Tier 1 的 `lock` 零架构成本。**bar 是唯一需要动引擎的
地方**，方案：

- 激活的皮肤对 `bar` target **没有** HTML 入口 → 维持现有 GDI 窗口
  （`EyeNurse3BarWnd`），零回归、零额外内存；
- 激活的皮肤对 `bar` **有** HTML 入口 → bar 线程改为创建透明 WebView 窗口
  （与 mask 同栈：tao 窗口 + wry，Tauri 2 多 webview 已有先例），加载
  `eyenurse-skin://<id>/bar/index.html`。

代价：选了 HTML bar 皮肤后常驻一个 WebView2 进程（数十 MB 级）。收益：
UGC 表达力与 lock 完全对齐，一套协议两个表面。Wallpaper Engine / Rainmeter
生态证明该成本用户可接受，且**成本是 opt-in 的**（classic 用户永远不付）。

拖动（HTCAPTION）、右键菜单、置顶复选等 Win32 交互的归属见 §4。

## 2. 包格式 ESP-1

### 2.1 目录结构

```
MySkin/
├─ skin.json          # manifest（唯一必需）
├─ preview.png        # 设置页预览图（建议 320×180；可选）
├─ eyenurse-api.js    # 桥适配脚本（模板自带，皮肤页经 /eyenurse-api.js 引用）
├─ bar/               # 可选：桌面计数器
│  ├─ index.html      # 入口（相对路径，见 §3 加载）
│  ├─ main.js
│  ├─ style.css
│  └─ assets/
└─ lock/              # 可选：锁屏/休息遮罩
   ├─ index.html
   └─ ...
```

分发 = 打 zip 改后缀 `.eyeskin`（导入时解到用户皮肤目录）；网盘直下文件夹
拖进 `Assets/Skins/` 也合法。**裸 `*.json` 继续按 Tier 0 识别**，存量皮肤零迁移。

### 2.2 manifest `skin.json`（formatVersion 1）

```json
{
  "format": "eyenurse-skin",
  "formatVersion": 1,
  "id": "sunset-glass",
  "name": { "zh": "日落玻璃", "en": "Sunset Glass" },
  "author": "someone",
  "version": "1.0.0",
  "targets": {
    "bar":  { "entry": "bar/index.html",  "size": { "w": 120, "h": 32 } },
    "lock": { "entry": "lock/index.html" }
  },
  "colors": { "bar": { "progress_fill": "#FF6B35" }, "mask": { "bg": "#1A0E08" } }
}
```

- `id`：目录名即 id（同 Tier 0 惯例：放文件即装）。校验 `[a-z0-9-]{1,64}`。
- `targets.bar.size`：bar 窗口逻辑尺寸（DIP），宿主负责 ×DPI 与位置持久化
  （UI.json 仍存逻辑 DIP，与现在同规则）；缺省 = 80×20。
- `colors`：**可选**的颜色键，语义与 v3.1 完全一致——给 Tier 1 皮肤当
  「classic 回退底色」（见 §5），也让纯配色皮肤能从裸 json 无损升级为目录包。
- `formatVersion` 高于宿主支持 → 该皮肤标记为不兼容，设置页置灰 + 原因文案，
  **绝不半加载**。

## 3. 加载与隔离（安全边界）

**UGC 皮肤 = 不可信代码**。现有 mask 页持有完整 Tauri IPC
（`rest_done`、`lock_try_unlock`——后者收**锁屏密码**），皮肤页绝不能同级。

1. **自定义协议加载**：皮肤资源经 `eyenurse-skin://`（Windows 页面源
   `http://eyenurse-skin.localhost/...`）提供，handler 只服务已注册皮肤目录内
   的文件——id 白名单字符集 + 逐段字符校验（**不做百分号解码**，编码名不匹配
   即 404）+ 规范化越界断言，无任意 FS 读（`skin_proto::handle_skin_request`）。
2. **沙箱 iframe**：宿主壳页（`mask.html` / `barweb.html`）以
   `<iframe sandbox="allow-scripts">` 加载皮肤页（**无 `allow-same-origin`**），
   皮肤 JS 处于 opaque origin：拿不到 Tauri IPC、拿不到宿主 DOM、拿不到
   localStorage。宿主壳 ↔ 皮肤之间只走 `postMessage`，消息白名单见 §3.1。
3. **密码输入宿主独占**：锁屏密码 `<input>` 是宿主壳页自有的 chrome 元素
   （`#skin-pwd`）——沙箱 iframe 本就摸不到宿主 DOM，无需 Shadow DOM（设计期
   方案的简化：安全边界由 iframe 提供，不重复设防）。皮肤经 manifest 的
   `targets.lock.chrome` **百分比坐标**声明密码框/解锁按钮摆哪，宿主照声明
   定位；密码值只出现在宿主 DOM 与 CoreLoop 之间，永不过 iframe。
4. **网络隔离**：皮肤响应统一带严格 CSP（`default-src 'none'` + 仅协议 host
   子资源 + `connect-src 'none'`；内联脚本/样式放行——唯一内容源是本地包本身，
   单文件皮肤是重要 authored 体验）。皮肤是纯本地包。
5. **动作限速**：`try_unlock` 在宿主 CoreLoop 侧限速（**1 次/秒 + 连错 5 次
   冷却 60s**，`CoreLoop::try_unlock`），与皮肤无关——防爆破是宿主职责。

### 3.1 桥契约（`window.eyenurse`，包内适配脚本交付）

宿主**无法**向 opaque iframe 注入对象（跨域限制），故桥以**包内适配脚本**
交付：模板/示例包根的 `eyenurse-api.js`（`<script src="/eyenurse-api.js">`），
把 postMessage 协议包成 `window.eyenurse` API。加载即向宿主握手
（`eyenurse-hello`）——3 秒内未握手 = 皮肤损坏，宿主撤下 iframe 回退内置界面
（看门狗，永不白窗）。协议报文：

```text
皮肤 → 宿主：{ type: "eyenurse-hello" }
             { type: "eyenurse-action", action: "drag" | "openSettings" |
                        "setTopmost" | "endRest" | "tryUnlock", ...payload }
宿主 → 皮肤：{ type: "eyenurse-state",   ...1Hz 状态（见下） }
             { type: "eyenurse-texts",  texts: i18n 整包 }
             { type: "eyenurse-colors", colors: 逐键回退完整配色 }
```

`eyenurse-api.js` 暴露的等价 API（皮肤只消费这一个对象）：

```ts
interface SkinState {
  // —— bar 表面（1Hz bar-state）——
  countdownMs: number;  intervalMs: number;  pausedMs: number | null;
  resting: boolean;     topmost: boolean;
  // —— lock 表面（1Hz rest-tick 映射）——
  remainingMs: number;  locked: boolean;     hasPassword: boolean;
}
eyenurse.onState(cb) / onTexts(cb) / onColors(cb);
eyenurse.drag() / openSettings() / setTopmost(v);
eyenurse.endRest() / tryUnlock(password);   // 白名单动作，宿主侧过滤+限速
```

设计原则：**数据契约只收敛现有快照字段**（`BarSnapshot` / `RestTickPayload`），
不为皮肤发明新数据源；v2/v3 行为语义（mm:ss 分钟分量回绕、暂停互换菜单等）
留在宿主壳，皮肤只管"长什么样"。

## 4. bar 表面的交互契约

GDI 版的三条实测教训（系统拖动/非客户区命中/菜单前台坑）**全部留在宿主**，
皮肤不碰窗口管理。**实现形态（v1）**：iframe 事件不出沙箱，交互一律显式经桥
动作声明——

- **拖动**：皮肤在条身 `mousedown` 调 `eyenurse.drag()` → 壳页
  `invoke("bar_drag")` → `startDragging()` 系统拖动；Move 逐帧事件在 Rust 侧
  转 `CoreMsg::SaveBarPosition`（物理÷DPI → 逻辑），CoreLoop 攒批 1Hz 落盘
  UI.json（等价 GDI 条 WM_EXITSIZEMOVE→Q9 语义）。
- **右键菜单**：v1 未接 TrackPopupMenuEx（跨 iframe 事件不可达 + 弹窗宿主线
  程约束）；皮肤以 `eyenurse.openSettings()` / `setTopmost()` 自行搭控制件，
  托盘菜单仍可用（暂停/恢复/重置）。菜单桥动作留 P2。
- **置顶/暂停等控制**：建议皮肤显式做按钮调 `setTopmost`/`openSettings`，协议
  不强制——没有任何按钮的皮肤依然完整可用（托盘兜底）。
- **命中透明**：v1 未做（barweb 整窗接收事件）；非矩形皮肤的点击穿透留 P2
  （采样透明度换算分层窗口）。

## 5. 解析与回退（延续逐键哲学）

激活皮肤 `Current` 指向 id，每个 target 独立解析：

```
target(bar)  = manifest.targets.bar  存在且文件有效 ? HTML表面
             : manifest.colors.bar  存在 ? Tier0配色 + GDI经典形态
             : classic
target(lock) = 同规则（经典形态 = 现有 mask 页）
```

- 加载失败（入口 404、JS 崩溃、握手超时）→ 该 target 落回内置界面：
  mask = 壳页 iframe 看门狗（3s 无 `eyenurse-hello` 即撤）；barweb = 回退壳页
  内置迷你条；bar 模式差异（GDI↔HTML）**启动期定死**，换肤后形态变化由设置页
  显示重启提示（配色即时生效）。
- 来源优先级不变：custom（编辑器，Tier 0）> 磁盘（Tier 0 + Tier 1 混扫，
  按 id 排序）> 内置。

## 6. 设置页与工具链

- **SkinPicker（已实现）**：目录包显示 `preview.png`（经协议 host 加载，
  tauri.conf.json CSP 已放行 img-src）+ 「悬浮条/锁屏」表面徽标 + bar 形态
  重启提示；纯配色皮肤维持迷你色卡。
- **导入**：v1 手动放置（`Assets/Skins/<id>/` 拖入即装）；`.eyeskin` 后缀与
  文件关联留 P2。
- **开发体验**：仓库 `assets/skins-example/sunset-clock/` 即模板（build.rs
  拷到 exe 旁 Assets/Skins，dev 直接可试）；包根 `eyenurse-api.js` 是桥适配
  脚本。改包内文件后重进休息/重启应用生效（无热重载，P2）。

## 7. 非目标（v1 明确砍掉）

- 不做皮肤市场/在线源（格式已为其留位：id + version，加个索引 JSON 就是商店）。
- 不做 Tier 2「classic 引擎形态参数」（bar 宽高圆角字体等中间档）——要改形态
  直接上 Tier 1，中间档的协议复杂度不值。
- 皮肤不提供"逻辑插件"（改提醒策略、改倒计时算法）：协议只管**呈现**，
  状态快照是皮肤能拿到的全部输入。

## 8. 落地状态

1. **P0 协议地基 — ✅**：manifest 解析/校验（id/版本/entry 穿越与存在性，
   core 9 测试）、`eyenurse-skin://` 协议 handler（字符白名单 + 越界断言 +
   CSP，handler 单测）、扫描混扫 Tier 0/1（零回归）。
2. **P0 lock 表面 — ✅**：mask 宿主壳 + sandbox iframe + postMessage 白名单桥
   （skinHost）+ 宿主独占密码 chrome（manifest 百分比摆放）+ tryUnlock 限速。
3. **P1 bar 表面 — ✅**：启动期双模式分流（GDI 原样 / barweb WebView 壳窗，
   `skin_proto::build_barweb_window`）；barweb 壳 + 回退迷你条；拖动落位攒批
   落盘；置顶/双击设置同源。
4. **P1 分发 — 部分**：SkinPicker 徽标/预览/重启提示已上；`.eyeskin` 导入与
   文件关联留 P2。
5. **P2**：右键菜单桥、点击穿透、热重载、`.eyeskin` 关联、官方皮肤扩充。

## 9. 决策记录（原"待拍板"项）

1. bar HTML 化 WebView 常驻内存（opt-in）→ **接受**（classic 零回归；实测
   smoke 双模式均过）。
2. `tryUnlock` 限速 → **1 次/秒 + 连错 5 次冷却 60s**（CoreLoop 实现对所有
   入口一致）。
3. `.eyeskin` 后缀 → 未定（P2 引入导入时再拍）。
4. 模板仓库 → 暂以仓库内 `assets/skins-example/` 为模板真源，独立仓库待社区
   起量再拆。
