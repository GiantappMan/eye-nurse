# EyeNurse 设计语言 v3（DS3）— 实施规范

> 适用面：设置窗口、皮肤中心、mask 壳页、barweb 壳页、内置皮肤包（classic/devtest）、
> Win32 GDI 悬浮条。六个实现者按本文照做，数值不再自由发挥。
> Token 真源：`ui/src/shared/design.css`（CSS）与 `ui/src/shared/theme.ts` 的 `tokens`
> （JS 镜像）；运行时活色真源：`crates/eyenurse-app/src/ui/theme.json`。
> 三处改值必须同步，本文 §2 是对照表。

设计气质对标 macOS 系统设置 / Linear：克制装饰、层级靠字号与留白、色彩纪律
（绿 = 护眼/进行，红 = 警示，其余信息一律中性灰阶）、数字一律等宽。深色护眼场景优先。

---

## 1. 原则（违反任何一条即返工）

1. **单一取色入口**：CSS 一律 `var(--…)`；禁止页面写 `#rrggbb` 字面量（皮肤配色仿样、
   皮肤包内的 `--eyenurse-*` 兜底值除外——那是数据不是样式）。
2. **4pt 网格**：一切间距 ∈ {4,8,12,16,20,24,32,40}px。现存的 9/13/14/18/22/26px 全部吸附。
3. **字号下限 11px**：10px 微文字一律升到 `--fs-caption`；字重只用 400/500/600。
4. **数字等宽**：倒计时/统计/时间输入/时钟一律 `--font-num` + `tabular-nums`，宽度不抖。
5. **布局稳定三禁**：禁条件渲染改变周围元素位置（用占位/禁用）；禁异步首帧跳动
   （默认文案/最小高度占位）；禁状态 chip 插入文档流推动邻居（固定槽位）。
6. **键盘可达**：每个可交互元素都有可见焦点态（全局 `:focus-visible` 绿描边已由
   design.css 提供，自定义 ring 的控件沿用其 ring）；打击区 ≥ 28px，唯一逃生口 ≥ 44px。
7. **动效克制**：时长只允许 `--dur-fast`(120ms)/`--dur-slow`(180ms)；一切动效必须
   有 `prefers-reduced-motion` 归零分支。唯一明文例外：barweb 壳页 `#fill` 的
   `width 1s linear`（§5.4-6）——进度由 1Hz 状态步进驱动，1s linear 把每步收敛为
   恒速连续收缩；120/180ms 会让恒速倒计时每秒抽动一次，任何 easing 都破坏恒速
   语义。reduced-motion 归零分支照常必备。

---

## 2. Token 表

### 2.1 色彩（L2 语义层，页面唯一入口）

| 变量 | 亮色 | 暗色 | 用途 |
|---|---|---|---|
| `--bg` | `#f3f5f7` | `#16181c` | 窗口底 |
| `--surface` | `#ffffff` | `#1f2227` | 卡片 / 顶栏 / 弹窗 |
| `--surface-2` | `#f7f9fa` | `#262a30` | 输入底 / chip 底 / 图表轨道 |
| `--border` | `#e4e8ec` | `#30353c` | 1px 描边（装饰级） |
| `--border-strong` | `#c6cdd4` | `#4a5058` | hover 描边 / 图表轨道描边 |
| `--text` | `#1b2129` | `#e7eaee` | 主文字 |
| `--text-2` | `#67727e` | `#99a1ab` | 次级文字/图标（白底 4.9:1） |
| `--text-3` | `#98a1ac` | `#6b7480` | 版权 / 占位 / 禁用（最低档） |
| `--accent` / `--accent-strong` / `--accent-soft` | `#0e9f6e` / `#0b7f58` / `#e2f6ee` | `#34c48d` / `#2aa877` / `#1d332b` | 强调 / hover / 选中底与焦点 ring |
| `--danger` / `--danger-strong` / `--danger-soft` | `#d23c3c` / `#b52f2f` / `#fdeeee` | `#f07171` / `#f68b8b` / `#3a2426` | 错误 / 危险操作（亮色已按 WCAG 校准 4.72:1） |
| `--chart-work` | `#7d8792` | `#79838e` | 图表"工作"条（对 `--surface-2` ≥3.4:1） |
| `--chart-rest` | = `--accent` | | 图表"休息"段 |
| `--hero-from/-to/-text/-dim` | `#10715a`/`#0e9f6e`/`#f2fbf7`/`#cfe9de` | `#17453a`/`#1d5c47`/`#eafaf3`/`#9fc9b8` | 设置页 Hero 渐变 |

遮罩/悬浮条活色沿用 `theme.json → --eyenurse-*`，本表不管；但其**变更**见 §6。

### 2.2 字阶 / 字体

| Token | 值 | 用途 |
|---|---|---|
| `--fs-caption` | 11px | 版权、徽章、角标、仿样注释 |
| `--fs-hint` | 12px | 分组标题、行辅助说明、chip、ghost 按钮 |
| `--fs-body` | 13px | **全局基准**：正文、行标题、按钮、输入 |
| `--fs-subtitle` | 15px | 弹窗标题、强调数值 |
| `--fs-title` | 18px | 窗口标题 |
| `--fs-display` | 40px | 设置页 Hero 倒计时 |
| `--fs-clock` | 64px | 锁屏时钟 |
| `--font-ui` | `"Segoe UI","Microsoft YaHei UI","Microsoft YaHei","PingFang SC",sans-serif` | 一切 UI 文字（含会出 CJK 的元素） |
| `--font-num` | `Consolas,"Cascadia Mono",monospace` | 一切数字读数（**栈内两字体必须成对出现**，不允许只写 Consolas） |

### 2.3 间距 / 圆角 / 阴影 / 动效 / 层级

| 类别 | Token → 值 |
|---|---|
| 间距 | `--sp-1…8` → 4 / 8 / 12 / 16 / 20 / 24 / 32 / 40 px |
| 圆角 | `--r-sm` 6px（色板小件）、`--r-md` 8px（按钮/输入/横幅/小面板）、`--r-lg` 12px（卡片/编辑器/弹窗/预览舞台）、`--r-full` 999px（chip/徽章/开关/进度条）；**GDI 条及其仿样 = 0（直角是真源）**。全站不存在 3/4/5/10/14px |
| 阴影 | `--shadow-1`（卡片 hover、下拉）、`--shadow-2`（弹窗）。深色模式靠 border 分离，不叠阴影 |
| 焦点 ring | `--ring-accent` / `--ring-danger` / `--ring-onaccent`（深底白 ring 35%） |
| 动效 | `--dur-fast` 120ms（hover/press）、`--dur-slow` 180ms（滑块/弹层）、`--ease cubic-bezier(0.2,0,0,1)` |
| 层级 | `--z-frame` 10（皮肤 iframe）、`--z-chrome` 20（宿主 chrome）、`--z-modal` 50、`--z-toast` 60 |

### 2.4 控件几何

| 控件 | 规格 |
|---|---|
| 主按钮 | 高 30px（`--control-h-md`）、padding 0 16px、13px/600、radius 8、disabled opacity .45 + not-allowed |
| 幽灵按钮 | 高 28px、padding 0 12px、12px/500、1px `--border`、radius 8 |
| 输入框 | 高 30px、padding 0 12px、13px、`--surface-2` 底、radius 8 |
| 开关 | 40×22、圆钮 18、内衬 2px（SwitchToggle.vue 现值即标准）；disabled opacity .45 |
| 图标 | 24 viewBox 网格、stroke 2、currentColor；渲染尺寸取 24 的整数倍或 1:1（16/24/32）；**禁止文本字形 ▸ ✕ ▲ ✓ 充当图标** |

---

## 3. 布局原则

- **窗口**：settings 900×500、skincenter 900×620（tauri.conf.json 现值不变）。两窗都保留
  原生标题栏；窗口内不再重复品牌名。
- **设置页单列**：内容 `max-width: 640px` 居中，页 padding 20px 20px 16px，块间距
  `--sp-4`(16px)；顶栏高 52px、左右 padding 20px。
- **皮肤中心网格**：容器 padding 20px 24px 24px；卡片 `repeat(auto-fill, minmax(240px,1fr))`
  + `gap: var(--sp-4)` → 900 宽下 3 列；预览图高 110px 不变。
- **弹窗**：宽 660px、radius `--r-lg`；结构 = head(16/20/12) + body(`overflow-y:auto`,
  0 20px 12px) + foot(`flex-shrink:0`, 12/20/16)——**footer 永远不被滚走**，滚动只发生在 body。
- **对齐**：卡片内左右 padding 统一 16px；行内控件右缘与卡片 padding 右缘对齐；
  图表行 = 日期 46px + 轨道 flex + 计数 34px，gap 10px。
- **数字列宽稳定**：等宽字体 + tabular-nums 之外，会变化的数值槽（今日统计、倒计时）
  给 `min-width`，避免秒跳动引发行宽抖动。

---

## 4. 组件规格（含行为）

### 4.1 按钮 `.en-btn` 家族（design.css 已提供，页面直接用）
- 主 CTA：`.en-btn.en-btn-primary`（accent 底白字）；深色渐变底上用 `.en-btn-onhero`。
- 次级：`.en-btn.en-btn-ghost`；危险二次确认态加 `.is-danger`。
- 全站一个文件只有**一个**主按钮实现；皮肤中心现存的 `.apply` 与 `.apply-btn` 合并为此类。
- 悬停只变色（主）/变边（ghost）；只有 Hero 白按钮允许 `translateY(-1px)+--shadow-1`。

### 4.2 输入框 `.en-input`
- 时间类加 `.en-input-mono`（等宽 + 居中）；宽度按内容：HH:mm:ss = 104px、锁屏密码 180px。
- **invalid 红态只在用户触碰后出现**（blur 或输入过；首帧草稿加载完成前不得显示假错误）。
- 依赖主开关的输入用 `:disabled` + `.en-input:disabled`（opacity .45），**禁用 v-if 拆除**。

### 4.3 开关（SwitchToggle.vue，规格即法律）
- 40×22 / 钮 18 / on= `--accent` / off= `--surface-2` + inset 1px `--border`。
- 新增 `disabled` prop：opacity .45 + cursor default + 不发 commit——"依赖主开关的子开关"
  一律传 `:disabled="!主开关"`，保持行位置不动。
- 焦点 ring 3px `--accent-soft`（现状保留）。

### 4.4 卡片与行 `.en-card / .en-card-title / .en-row`
- 卡片：radius 12、`padding: var(--sp-1) var(--sp-4)`；分组标题 12px/600/`letter-spacing:.5px`
  /`--text-2`；行 `padding: var(--sp-3) 0` + 顶部分隔线（首行无线）。
- 行标题 13px/500；辅助说明 12px `--text-2`，`margin-top: 2px`。

### 4.5 chip / 徽章 `.en-chip / .en-badge`
- chip：12px、3px 10px、radius full、active= `--accent-soft` 底 + 1px `--accent` 边。
- 徽章：11px（caption）、3px 8px、radius full、accent-soft 底；**一行内徽章 ≤2 枚**，更多合并。

### 4.6 图表（StatsCard）
- 轨道：`--surface-2` 底 + `inset 0 0 0 1px var(--border)`、高 10px、radius full。
- 工作段 `--chart-work`、休息段 `--chart-rest`；零数据日不画条；图例颜色与段一致。

### 4.7 弹窗
- mask 遮罩 `rgb(0 0 0 / .45)`、radius 12、`--shadow-2`；head/body/foot 节奏见 §3。
- 关闭钮 = 24×24 命中、16px 线性 SVG（两条线）、`--text-2`→hover `--text`，
  tooltip/aria 用「关闭」语义键（新增 i18n `close`），**禁止复制相邻按钮的文案**。
- ESC + 点遮罩关闭（现状保留）；主 CTA 在 foot 右侧恒可见。

### 4.8 焦点与动效
- 全局 `:focus-visible` 绿描边已内置；自定义 ring 的控件（switch、输入）保持 ring 语法。
- reduced-motion 归零规则 design.css 已内置；皮肤包/GDI 条的动效各自加同款分支（§5.5/§6）。

---

## 5. 各界面重做方向

### 5.1 设置窗口（ui/src/settings/*）
1. **去双页头**：原生标题栏保留；`App.vue` 顶栏删除品牌名「EyeNurse」（mark 图标一并删），
   左侧留空或放语境标题「设置」；52px 高、padding 0 20px 不变。
2. **保存 chip 固定槽**：`.top-right` 改为 `[saved-slot][lang]`，slot 恒占位
   `width:72px; display:flex; justify-content:flex-end`，chip 在槽内出现/消失——语言下拉永不被推动。
3. **图表可见性**：`.bfill` 背景 `var(--chart-work)`；`.btrack` 加 inset 描边（§4.6）。
4. **状态完备**：hms 输入加 `touched` 集合，首帧不显示红框（§4.2）；`ResetTimeout` 输入
   从 `v-if` 改恒渲染 + `:disabled`；锁屏三个子开关 `:disabled="!lockSettings.Enabled"`。
5. **节奏收敛**：卡片/行全部换 `.en-*` 类；hero padding 20px 24px 24px、gap 16px；
   页 gap 14→16px；`.copyright` 11px = `--fs-caption`；全局字号基准已降为 13px。

### 5.2 皮肤中心（ui/src/skincenter/SkinCenter.vue）
1. **修错位 tooltip**：`.modal-close` 的 title 从 `t("skinApply")` 改为「关闭」键（新增 i18n `close`，zh=关闭 / en=Close）。
2. **演示数据自洽**：仿样文案 32:15 → `17:00`，与 `BAR_DEMO`(45min 剩 17min) 和 62% 填充同讲一个故事；
   文案由 `BAR_DEMO.countdownMs` 计算生成，不手写字面量。
3. **按钮归一**：`.apply`/`.apply-btn` → `.en-btn.en-btn-primary`；设置页 `.skin-center-btn` 同规格。
4. **圆角 + 高程词汇表收敛**：卡片/编辑器/横幅/两块预览舞台/弹窗统一 `--r-lg`(12px)；
   `.card:hover` 在现位移基础上加 `box-shadow: var(--shadow-1)`；active ring 3px `--accent-soft` 保留。
5. **密度与字号**：10px 三处（active-chip/badge/mock-note）→ `--fs-caption`(11px)；
   group-label/section-label → 12px、letter-spacing 0.5px；meta 区 padding 12px 16px 12px、gap 8px，
   name 13/600、author 12、badges 11px；弹窗改为 body 滚动 + foot 恒可见（§3）。
6. **图标与焦点**：▸ ✕ ✓ 换 16px 内联 SVG（24 网格 stroke 2）；全控件吃全局 `:focus-visible`。

### 5.3 内置遮罩画面 mask 壳页（ui/mask.html、ui/src/mask/*）
1. **字体补齐**：`mask.css` 的 `html,body` 加 `font-family: var(--font-ui)`（本页不 import
   design.css，把同款栈字面写入）；`#skin-pwd` 保持 16px，`#skin-lock-err` 13px。
2. **chrome 全面主题化**（消灭第二份 #EEEFF2 字面值）：所有白 alpha 改
   `color-mix(in srgb, var(--eyenurse-mask-fg) N%, transparent)`——占位符 45%、边框 28%→focus 70%、
   底 8%→12%、解锁钮 hover 底 10%；任何皮肤_fg（含 devtest 绿）下同一个控件只有一种主题语言。
3. **逃生口升级**：`#unlock` 44×44 命中区（`--z-chrome`）、内嵌 24px 图标 1:1 渲染（杀 25px 非整缩放）、
   贴角 8px、radius `--r-md`、常态 opacity 0.6、hover 1 + fg-10% 底；删除重复 `z-index:20`
   与 `#skin-frame` 的 `inset+width/height` 冗余。
4. **错误间距定值**：`main.ts` 错误行 `top` 改 `calc(${password_y}% + 28px)`（28px = 密码框半高 20
   + 间距 8，分辨率无关）；删除 `margin: 2px 0 0` 定位噪声。
5. **错误色入真源**：theme.json 新增 `mask_error`（默认 `#FF8B8B`，对 `#141418` 8.15:1），
   build.rs 必备键 + SkinColors/skin 透传同步；CSS 用 `var(--eyenurse-mask-error, #ff8b8b)`。
   `#skin-pwd:focus` 加 `box-shadow: 0 0 0 3px color-mix(fg 25%, transparent)`。

### 5.4 悬浮条 Web 渲染 barweb 壳页（ui/barweb.html、ui/src/barweb/*）
1. **填充语义对齐 GDI**：常态/休息 = 纯 `--eyenurse-bar-progress-fill` 满宽按比例；暂停才出现渐变——
   JS 按秒插值 `progress_from→progress_to`（480s 走满）与 `text_from→text_to`；删除三色空间渐变；
   头注释改为与实现一致。插值公式（JS）：
   `lerp(a,b,t) = a + Math.round((b-a)*t)` 逐通道，`t = clamp(pausedSecs/480, 0, 1)`。
2. **暂停表达**：删 `opacity .5`，文字色走渐变（同 GDI）；休息文案 = `text_idle` 纯色。
3. **字体与缩放**：`#label` 换 UI 栈（resting 中文不再掉进 generic monospace）；字号由 JS 按
   surface 高度设定：`clamp(round(h*0.65), 11, 32)`——20px 条 = 13px 与 GDI 同基准（tokens.bar）。
4. **直角**：`#builtin` radius 3px→0（GDI FillRect 直角真源，注释同步）。
5. **可点击可感知**：`#builtin { cursor: pointer }` + `:active { filter: brightness(0.96) }`；
   hover 不加装饰（回退安全网，克制）。
6. **1Hz 填充平滑（§1.7 动效白名单唯一例外）**：`#fill` 的 `transition: width 1s linear`
   保留——进度每 1s 步进一次，1s linear 使步进呈恒速收缩（对齐倒计时恒速语义；
   短时长 + easing 会每秒抽动）。reduced-motion 归零分支必备（barweb.css 已有）。
   注意本页不 import design.css，`--dur-*` 在该页未定义，勿照抄设置页 Hero 写法。

### 5.5 内置皮肤包（crates/eyenurse-app/src/skins/classic、devtest）
1. **兜底文案不漏键名**：`texts["lockedTitle"] ?? "已锁定"`（键泄漏 = 协议实现事故）。
2. **防首帧跳动**：`#locked-title` 直接预置「已锁定」默认文案，不再空 `<p>`。
3. **微排版统一**：`#locked-title` 与 `#lock-date` 同为 13px、opacity 0.65、都无 letter-spacing
   （删标题 2px 字距）；monospace 栈统一为 `Consolas,"Cascadia Mono",monospace`（#countdown 补 Cascadia）。
4. **层级**：`#countdown` 16→18px + `font-weight:600`；center-stack gap 28px 保持（主/辅分组），
   lock-panel 内三行 gap 8px 保持但标题与日期视觉同权、时钟独大（64px）。
5. **reduced-motion**：`@media (prefers-reduced-motion: reduce){ .spinner{ animation:none; } }`
   （静止双弧仍是加载表达）；日期固定 `toLocaleDateString("zh-CN",{year:"numeric",month:"2-digit",day:"2-digit"})`
   → 「2026/10/09」零填充，配 tabular 时钟宽度稳定。
6. devtest 只守基本对齐（协议验证专用），以上 1–5 同样适用。

### 5.6 Win32 GDI 悬浮条（crates/eyenurse-app/src/ui/bar.rs、theme.json、build.rs）
1. **DPI 描边统一**：时钟与置顶复选 pen 宽统一 `1i32.max((dpi / 96) as i32)`
   （96dpi→1px、200%→2px）；删除时钟特例 `dpi/48`。
2. **暂停末态对比度**：theme.json `bar_time_to` `#F40000`→`#FF5252`（对 `#3E0D0D` 实测 5.19:1，
   达标 4.5；ramp 起点 `#080808` 不变）；`skin.rs classic()` 与皮肤中心 draft 默认值同步。
3. **token 命名统一**：theme.json/build.rs 的 `bar_time_from/to` 改名 `bar_text_from/to`
   （与 SkinColors 字段、皮肤中心编辑器键同名，删除三处两名漂移）。
4. **圆角 + 收边**：`DwmSetWindowAttribute(hwnd, 33 /*CORNER_PREFERENCE*/, &2 /*ROUND*/, 4)`
   （Win11 圆角、Win10 静默忽略）；`on_paint` 末尾画 1px 内描边 `#DCDCC6`（底色加深 10%）
   在相近壁纸上给出分离线。
5. **暂停构图**：时钟图标 ⌀15→13（=字高，`r=scale_px(6.5,dpi)`）；暂停时文字矩形
   `left = cx + r + scale_px(6,dpi)`（图标让位，重心居中于剩余空间）。暂停警示填充设
   **最小覆盖宽度 ≥ 文字块 + ICON_MARGIN**（实测文字宽，DT_CALCRECT）：mm:ss 与时钟
   图标恒落在渐变场上，文字与填充由同一插值 t 配对（起点 9.0:1、末态 5.19:1），
   **禁止文字落米色底**（`#FF5252` 对 `#F5F5DC` 仅 2.88:1）；覆盖不足时 meter 让位。
   休息帧（含暂停+休息）填充不吃渐变，回 `progress_fill` 纯色（对齐 §5.4-1：
   渐变是暂停专属表达）。已知继承局限：ramp 中段 t≈0.3–0.7 文字-填充同穿中灰度，
   对比度不足（v2 旧对同址同样失效，t≥0.5 时更差），修复需重设计两条 ramp 的
   亮度轨迹，不在 token 篇章内。
6. **度量 token 化**：新增逻辑常量 `ICON_MARGIN=4.0 / ICON_RADIUS=6.5 / FONT_SIZE=13.0 /
   CHECK_BOX=12.0 / CHECK_INSET=4.0（3.5→4.0 入网）/ CHECK_SIDE=16.0`；
   `bar_text_idle` `#000000`→`#1A1A1A`（15.7:1 仍达标，去纯黑刺感）；
   复选悬停白块 18→16px（条高 20 上下各留 2px），色 `#FFFFFF` 保留（v2 对齐）。

---

## 6. theme.json 变更汇总（配色真源，改这里 = 两态一致）

| 键 | 现值 → 新值 | 理由 |
|---|---|---|
| `bar_time_from` → `bar_text_from` | `#080808` 不变 | 与 SkinColors 字段/编辑器键同名 |
| `bar_time_to` → `bar_text_to` | `#F40000` → `#FF5252` | 暂停末态对比 3.82→5.19 |
| `bar_text_idle` | `#000000` → `#1A1A1A` | 软黑，15.7:1 |
| `mask_error`（新增） | `#FF8B8B` | mask chrome 错误色入唯一真源；build.rs 必备键同步 |

同步点：`crates/eyenurse-core/src/skin.rs` 的 `classic()` 字面值、皮肤中心 `draft` 默认值、
`mask.css` 的 `var(--eyenurse-mask-error, …)` 兜底。

---

## 7. 验收清单（合并前自查，命令可直接跑）

```bash
# 1. 出圈圆角清零（应无输出；barweb/皮肤仿样的直角写 0 不写 3px）
grep -rnE "border-radius: *(3|4|5|10|14)px" ui/src
# 2. 字号下限（应无输出）
grep -rn "font-size: *10px" ui/src
# 3. 字面量色（settings/skincenter 应无输出；--eyenurse-* 兜底与皮肤包数据除外）
grep -rnE "#[0-9a-fA-F]{6}" ui/src/settings ui/src/skincenter
# 4. 孤值间距（4pt 网外；应无输出）
grep -rnE "(padding|gap|margin)[^:]*: *[^;]*\b(9|13|14|18|22|26)px" ui/src/settings ui/src/skincenter
# 5. token 命名漂移（应无输出）
grep -rn "bar_time_" crates ui/src
# 6. 前端可编译
bun run build    # 或 package.json 中的 build 脚本
```
