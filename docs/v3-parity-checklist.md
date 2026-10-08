# v3 行为对齐清单（Parity Checklist）

> 用途：v3 实现完成后逐条核对。每条含 **v2 证据**（`文件:行`，均在本仓库 v2 代码中实际读取）、**v2 行为**、**v3 预期**、**验证方法**、**归属**（实现者 A=core / B=壳与平台层 / C=悬浮条+遮罩 / D=设置窗+打包，分工见 `docs/v3-architecture.md` §1）。
> 标记：`[单测]`=自动化断言；`[手测]`=人工步骤；`[字节]`=文件字节级比对。
> 怪癖决议项（Q 系列）实现前必须先给出"保留/修复"结论，并在本文件勾选。

---

## A. 计时器状态机（归属 A，复刻 `src/EyeNurse/Services/GlobalTimerService.cs`）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| A1 | 间隔下限 | GlobalTimerService.cs:27-32 | `TimeSpan < 1s` 抛 `ArgumentOutOfRangeException` | `GlobalTimer::new(<1s)` 返回 `Err(IntervalTooShort)` | [单测] interval=0.9s → Err | A |
| A2 | Start 幂等 | cs:41-44；TestProject/GlobalTimerServiceTest.cs:39-40 | 已启动再 Start → `false` | 同 | [单测] | A |
| A3 | Start 即刷新剩余 | cs:46 `Reset()` | Start 立即把 remaining 置满并立刻发一次 Elapsed | `start()` 后 `poll()` 产出 `Elapsed{remaining=interval}` | [单测] | A |
| A4 | Stop 返回值 | cs:55-66；Test:63-68 | 运行中 Stop→true；未启动 Stop→**false 但仍调一次 CheckTrigger**（cs:64） | `stop()` 返回 false 时调用方须补一次 `poll()`；core 层行为与 v2 一致 | [单测] + [手测] 托盘连点暂停无异常 | A/B |
| A5 | Pause 返回值 | cs:67-76；Test:83-90 | 未启动或已暂停 → false；成功后记 `_pauseTime` | 同；`paused_at=now` | [单测] | A |
| A6 | Pause 冻结倒计时 | Test:92-94 | 暂停 3s 触发计数不增加 | 暂停期间 `poll()` 不产出 `Trigger`，remaining 冻结 | [单测] | A |
| A7 | Resume 补偿暂停时长 | cs:83-84 | `_nextTriggerTime += now - _pauseTime` | `next_trigger += resume_at - paused_at` | [单测] 暂停 2s 后 remaining ≈ 原剩余-2s（±1s） | A |
| A8 | Resume 返回值 | cs:77-80；Test:114-119 | 未启动/未暂停 → false；已暂停 → true | 同 | [单测] | A |
| A9 | Reset 语义 | cs:89-94；Test:138-183 | 恒 true；remaining 立即回满；Elapssed 的 remaining 随时间单调递减（Test:150-153） | `reset()` 恒 true；`Elapsed{remaining}` 单调递减 | [单测] | A |
| A10 | 触发阈值 | cs:122-127 | 剩余 <1s → 发 `Trigger` 并立即 Reset | `poll()` 在 `remaining<1s` 产出 `Trigger` 且内部回满 | [单测] 1s 间隔 6s 内触发次数 4..8（v2 容差 ±2） | A |
| A11 | Elapsed 在 Trigger 之前 | cs:109-128 | 每次检查先 Elapsed 再（可能）Trigger | `Vec` 内顺序 `Elapsed` 先于 `Trigger` | [单测] | A |
| A12 | 1Hz 更新粒度 | cs:47-52（Timer Interval=1000） | 悬浮条每秒跳一格 | v3 心跳 1Hz（`request_repaint_after(1s)`），显示秒粒度 | [手测] 观察 60s | B |
| A13 | Reset 即时刷新 | cs:92（Reset 内立即 CheckTrigger，注释"立即更新"） | 托盘"重置"后悬浮条**立即**回满，不等下一秒 | `AppModel::reset_timer` 在 `reset()` 后立即 `poll()` 并应用 Elapsed（架构 §4.2 怪癖 3） | [手测] 点重置瞬间倒计时回满 | B |
| A14 | 暂停后 Start | cs:43（守卫 `_timer != null && _timer.Enabled`） | 暂停后调用 `Start()` 会**成功**（Enabled=false 通过守卫并新建定时器）；该序列 v2 UI 不可达、NUnit 未锁定 | **Q11 决议：有意偏差**——v3 `start()` 暂停期间返回 false（视为已运行）；架构 §4.2 的"逐条等价"限定为 v2 UI 可达序列 | [单测] 断言 v3 选择的行为，注释标明 Q11 | A |

## B. 悬浮倒计时条 CountdownWindow（归属 C）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| B1 | 尺寸/边框 | CountdownWindow.xaml:11-21 | 80×20 DIP，`WindowStyle=None`，`ResizeMode=NoResize`，允许透明 | root 视口 `with_inner_size([80,20])`、`decorations(false)`、`resizable(false)` | [手测] 与 v2 并排目测 | C |
| B2 | 显示格式 | CountdownWindow.xaml:58（`StringFormat=mm\:ss`） | `mm` 是 TimeSpan **分钟分量（00-59）**，非总分钟：剩余 >59min 时回绕显示（90min 间隔首帧即显示 "30:00"，Q13） | 同公式：`mm = 分钟分量（总分钟 % 60）`，**不做**总分钟换算；Q13 决议：保留 | [单测] interval=90min 首帧显示 "30:00" | C |
| B3 | 进度条语义 | CountdownWindow.xaml:47-52 + Converters/TimeSpanToPercent.cs:21 | `remaining/interval*100`：**满→空**（ProgressBar 值是"剩余"） | bar 内自绘进度条从 100% 缩到 0% | [手测] 45min 档观察 | C |
| B4 | 悬停显示置顶开关 | CountdownWindow.xaml:91-117 | 鼠标进入 0.3s 淡入置顶复选；移出 ~1s 淡出 | hover 时显示置顶 checkbox，移出淡出（0.3s/1s） | [手测] | C |
| B5 | 置顶持久化 | CountdownWindow.xaml.cs:130-140；EyeNurseViewModel.cs:168-173 | 勾选即写 UI.json `TopMost`；`Topmost` 绑定生效 | toggle 即存 UI.json；**启动时按 UiJson.TopMost 条件应用**（`native_options(top_most)`，架构 §5.1——关掉置顶的用户重启后仍不置顶）；运行中经 `ViewportCommand::WindowLevel` 切换 | [手测] 关闭置顶→重启：悬浮条不置顶；开启→重启：置顶 | C/D |
| B6 | 拖动与保存 | CountdownWindow.xaml.cs:33-46 | **评审修订：保存分支疑似死代码**——`DragMove()` 阻塞至释放（MSDN：左键未按下时抛 InvalidOperationException，拖动进入系统 move-size 循环），MouseUp 被该循环吞掉，返回后 `e.LeftButton`（引用源核实：委托活设备状态，但设备大概率未收到被吞的释放）极可能仍为按下态 ⇒ `Released` 分支不执行，**v2 实际从不保存悬浮条位置**（UI.json Left/Top 恒默认 100,100） | **Q9 决议：修复**——v3 拖动结束帧保存逻辑坐标（有意偏差，记录于 Q9） | [手测] v3 拖动→重启位置保持；对 v2 不做此项验收 | C/B |
| B7 | Alt-Tab/任务栏隐藏 | CountdownWindow.xaml.cs:52-57（WS_EX_TOOLWINDOW）+ xaml:18（ShowInTaskbar=False） | 任务栏与 Alt-Tab 均不可见 | `with_taskbar(false)` **加** `GWL_EXSTYLE\|=WS_EX_TOOLWINDOW`（缺一不可，架构 §7.2）；补丁必须**自愈**（每帧检测重打），否则 B5 切换置顶触发 winit 回写 EXSTYLE 后重新出现在 Alt-Tab | [手测] Alt+Tab 查无此项；**切换置顶开关后再查仍查无**；托盘重置/休息各来一轮后复查 | C/B |
| B8 | 默认位置 | Models/UserConfigs/UI.cs:11-12 | Left=Top=100 | UI.json 缺省 (100,100) | [字节] | A |
| B9 | 暂停态展示 | CountdownWindow.xaml:118-123 + EyeNurseViewModel.cs:241-256 | 文字变 `PausedTime`（mm:ss，分钟分量同 Q13 回绕），文字色 `#080808→#F40000`、进度条前景 `#ECC071→#3E0D0D`（分母固定 480s，`percent==0` 时取 0.1），左侧出现时钟图标 | 同色值同渐变公式（含 0.1 下限），图标用 egui 自绘时钟 | [手测] 暂停 5min 比对色 | C |
| B10 | 休息中文案 | CountdownWindow.xaml:59-64 + 键 `resting` | 休息期间文字替换为本地化"休息中..." | 同 | [手测] | C |
| B11 | 双击托盘 | App.xaml.cs:87,162-165 | 双击托盘图标 → 显示悬浮条 | DoubleClick → bar 置顶显示 | [手测] | B |

## C. 休息遮罩 LockScreenWindow（归属 C + B）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| C1 | 每显示器一个 | EyeNurseViewModel.cs:184-196 | 遍历显示器快照，各建一窗，位置/大小 = `rcWork` | 每显示器一个 deferred 视口，rcWork 摆放（物理÷scale，见架构 §7.4） | [手测] 双屏不同 DPI | C/B |
| C2 | 样式 | LockScreenWindow.xaml:16-22 | 背景 `#141418`、前景 `#EEEEF2`、Topmost、无任务栏、无边框 | 同色值；`always_on_top`+`taskbar(false)`+`active(false)` | [手测] 取色器比对 | C |
| C3 | 居中倒计时 | LockScreenWindow.xaml:55-58 | `mm:ss`（LockScreenCountdown） | 同 | [手测] | C |
| C4 | 加载圈 | LockScreenWindow.xaml:51-53 | 70×70 LoadingCircle | egui Spinner，70×70 | [手测] | C |
| C5 | 解锁按钮 | LockScreenWindow.xaml:59-75；CloseCommand → CloseLockScreen（EyeNurseViewModel.cs:52,108-117） | 右下角 25×25 锁开图标，点击提前结束休息 | 右下角图标按钮 → `UiAction::RestDone` 提前结束 | [手测] | C |
| C6 | 休息时长 | EyeNurseViewModel.cs:212-217 | `RestDuration` 起 1Hz 递减，`TotalSeconds<0` 才退出 ⇒ 实际休息 ≈ RestDuration+1s（00:00 保留一秒） | 同：显示到 00:00 后下一 tick 才结束 | [单测] tick 序列 + [手测] 30s 档 | B |
| C7 | 立即休息入口 | App.xaml.cs:63-64,156-159 | 托盘"立刻休息" | 托盘同项触发 `rest_now()` | [手测] | B |
| C8 | 休息期间计时器停止/重启 | EyeNurseViewModel.cs:182,199-201 | RestNow：Stop；结束：Reset→Start | 同 | [单测] 状态序列 | B |
| C9 | 遮罩期间再触发休息 | EyeNurseViewModel.cs:178-180 | `IsResting` 互斥 | `rest_now()` 重入直接返回 | [手测] 托盘连点 | B |
| C10 | 显示器快照时机 | EyeNurseViewModel.cs:46（构造时枚举一次） | 用启动时快照 → 运行中插拔显示器会错位 | **Q4 决议**：默认 v3 改为休息开始时现枚举（记录为有意偏差） | [手测] 拔显示器后立刻休息 | B |
| C11 | 遮罩按钮即时性 | EyeNurseViewModel.cs:108-117（CloseLockScreen 立即关窗） | 点击解锁 → 立即关闭全部遮罩 | UiAction + `request_repaint_after_for(ZERO, ROOT)` 唤醒（架构 §6.1/§10），关闭感知延迟 ≤100ms；**deferred 视口输入不会执行 root logic，不得依赖 1Hz 心跳** | [手测] 点击解锁无明显滞留 | C/B |

## D. 会话锁/暂停（归属 B，复刻 EyeNurseViewModel.cs:59-83, 229-256）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| D1 | 锁屏暂停 | cs:78-81 | SessionLock：记 `_lockTime`、Pause | `SessionLock`: 记 `locked_at` + `pause()` | [手测] Win+L 后倒计时停 | B |
| D2 | 解锁恢复 | cs:63-64 | SessionUnlock：Resume | 同 | [手测] | B |
| D3 | 长锁重置 | cs:65-76 | 解锁时若 `now-_lockTime > ResetTimeout`：若在休息则提前结束；清 `_lockTime`；Reset | 同（阈值比较用 `>`） | [手测] 锁 >5min（或临时把 ResetTimeout 调 10s） | B |
| D4 | 忽略 ResetWhenSessionUnlock | cs:63-77 | **开关未被读取**：只要锁超时就重置 | **Q3 决议**：默认照抄（忽略开关）；如选修复须在清单勾掉本条并注明 | [手测] 开关=关仍重置 | B |
| D5 | 托盘暂停/恢复互斥展示 | App.xaml.cs:129-143（WPF Visibility，**点击即无条件互换**，先翻菜单再调 vm.Pause()） | 点暂停→暂停项隐藏+恢复项出现；反之亦然；Pause 失败（如休息中 timer 已 stop）**菜单也翻** | **Q12 决议**：菜单互换无条件（对齐 v2）；暂停展示另行门控（D8）；实现为 `Menu::remove`+`insert`（muda 无 set_visible，架构 §5.3） | [手测] 休息中点暂停：菜单翻到"恢复"但无暂停展示态 | B |
| D6 | 暂停计时展示 | EyeNurseViewModel.cs:249-255 | `PausedTime` 每秒 +1s | 1Hz 累加展示（见 B9 颜色） | [手测] | C |
| D7 | Reset 同时退出暂停 | EyeNurseViewModel.cs:229-234 | `Reset()`：若 IsPaused 先 Resume（清 PausedTime）再 timer.Reset | 同 | [单测]+[手测] | B |
| D8 | 休息中锁屏不进暂停展示 | EyeNurseViewModel.cs:246-247（`if (timer.Pause()) IsPaused = true` 门控） | 休息中 timer 已 stop → 锁屏时 Pause 失败 → **不**显示 PausedTime/暂停配色，仅记锁定时刻 | `AppModel::pause()` 按 core `pause()` 返回值门控，不得无条件置 paused 态（架构 §6.3） | [手测] 休息中 Win+L 再解锁：遮罩/悬浮条无暂停态 | B |

## E. 托盘（归属 B，复刻 App.xaml.cs:48-116）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| E1 | 菜单项与顺序 | App.xaml.cs:67-75 | 关于/设置/—/暂停/恢复/重置/立刻休息/—/退出 | 同序同项 | [手测] | B |
| E2 | 恢复项初始隐藏 | App.xaml.cs:59 | resume 初始 Collapsed | 初始只显示"暂停"（构建时即不含 resume 项；muda 无 set_visible，用 remove/insert，架构 §5.3） | [手测] | B |
| E3 | 退出 | App.xaml.cs:65（ControlCommands.ShutdownApp） | 菜单退出 → 进程结束 | 同（root Close → run_and_return） | [手测] 任务管理器查进程 | B |
| E4 | 关于 | EyeNurseViewModel.cs:265-269 | 打开浏览器 `https://www.giantapp.cn/post/products/eyenurse2` | ShellExecute open 同 URL | [手测] | B |
| E5 | 设置入口 | EyeNurseViewModel.cs:258-263 | 打开设置窗（已开则激活/还原，MainViewModel.cs:36-44） | 打开/聚焦设置视口 | [手测] | D/B |
| E6 | 图标 | App.xaml.cs:46-47 | `Assets/Img/logo.png`，300px 解码 | 同一 png（复制进 v3 assets）渲染托盘+视口图标 | [手测] | D |
| E7 | 菜单文案本地化 | App.xaml.cs:101-116 | 切语言即刷新全部菜单标题 | `set_language` 后 `tray.update_texts` | [手测] 中英切换 | B/D |

## F. 配置持久化（归属 A/D；字节兼容最高优先级）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| F1 | 目录 | 外部 ConfigService.cs（摘录见架构附录 §11；App.xaml.cs:37 AppName="EyeNurse"） | `%LOCALAPPDATA%\EyeNurse\UserData\`（.NET KnownFolder API 定位） | 同路径同文件名；用 `SHGetKnownFolderPath(FOLDERID_LocalAppData)` 定位、`LOCALAPPDATA` 环境变量仅作回退（架构 §4.3） | [字节] 路径断言 | A |
| F2 | 文件名 | ConfigService.cs `$"{typeof(T).Name}.json"` | `Setting.json` / `UI.json` / `Languages.json` | 同 | [字节] | A |
| F3 | Setting.json 默认内容 | Models/UserConfigs/Setting.cs:7-9,35-48 | `{"RestInterval":"00:45:00","RestDuration":"00:02:30","RunWhenStarts":true,"ResetWhenSessionUnlock":false,"ResetTimeout":"00:05:00"}`（紧凑、PascalCase、TimeSpan 为 "c" 字符串） | 字节一致（serde 紧凑 + HmsSpan） | [字节] 存默认后与上面串 diff | A |
| F4 | UI.json | Models/UserConfigs/UI.cs | `{"CoutdownWindowLeft":100,"CoutdownWindowTop":100,"TopMost":true}`（**注意 Coutdown 拼写**；整数字不带小数点） | 字节一致（StjF64 整数走 i64 输出） | [字节] | A |
| F5 | Languages.json | Models/UserConfigs/Languages.cs | `{"CurrentLan":null}`（null 照写） | 字节一致 | [字节] | A |
| F6 | TimeSpan 解析与 clamp 时机 | Setting.cs:14-21（构造路径 GetTimeSpan 有 clamp） | clamp **只**发生在“前端字符串 → Setting”路径；读盘路径从不 clamp（v2 手改 ≥1 天的 RestInterval 原样生效、原样回存） | `HmsSpan::parse` 失败→默认（同 v2）；`clamp_like_v2` 只在设置窗“文本→数值”保存路径调用，**`ConfigStore::load` 不 clamp**（否则手改配置行为改变、保存字节无法与 v2 往返，破坏 F12） | [单测] 读入 "1.02:03:04" 不变形、保存往返一致；设置窗输入 "1.02:03:04"→"02:03:04"、"-00:10:00"→"00:10:00"、"abc"→默认 | A/D |
| F7 | 坏文件容错 | 外部 JsonHelper.cs（吞异常返回 default）+ ConfigService.cs（`?? new T()`） | 损坏 JSON → 全默认值，不崩溃 | 同（load 内部 `unwrap_or_default`） | [单测] 写入 `{{{` 后 load | A |
| F8 | 保存失败静默 | 外部 JsonHelper.cs catch 返回 null | 保存异常不崩溃 | `save` 返回 Err，上层记日志不崩溃 | [单测] 只读目录 | A |
| F9 | 改设置即时生效 | ClientApi.cs:57-67 + EyeNurseService.cs:54-58 | SetSettings → 存盘 + 注册表 + `SettingChanged`→重建计时器（间隔生效，当前倒计时重置） | 设置窗保存 → 存盘 + timer 换 interval + reset | [手测] 改 2min 档立即重跳 | D/B |
| F10 | RunWhenStarts 展示值 | ClientApi.cs:51-55 | **评审修订：v2 实际显示磁盘值**——`GetSettings` 先 `Serialize` 再覆盖 `settings.RunWhenStarts`（cs:51-54，覆盖发生在序列化之后，是死代码；注释"显示给UI才调用"的意图未生效），Settings.vue 显示的是 Setting.json 里的值 | **Q10 决议：修复**——v3 设置窗勾选态 = 注册表实测值（落实 v2 注释声明的意图，有意偏差，记录于 Q10） | [手测] 手动删注册表值后打开设置窗：v3 应显示未勾选（v2 显示磁盘值 true） | D |
| F11 | 恢复默认 | ClientApi.cs:69-76 | ResetSettings：写默认 + 应用（注册表/计时器） | 设置窗"恢复默认"按钮同语义 | [手测]+[字节] | D |
| F12 | v2→v3 迁移 | —（兼容性目标） | — | 把 v2 的三个 json 原样放入 v3 读取，字段全部生效 | [手测] 用 v2 遗留文件冷启动 v3 | A |

## G. 开机自启（归属 A）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| G1 | 注册表位置 | 外部 DesktopStartupHelper.cs | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | 同 | [手测] regedit | A |
| G2 | 值名 | EyeNurseService.cs:34 | `"Eye Nurse 2"` | **Q6 决议：静默迁移**——v3 用 `"Eye Nurse 3"`；启动序列第 2.5 步调 `DesktopStartup::migrate_from("Eye Nurse 2")`（本键缺失且旧键存在 → 写新键+删旧键，**无 UI 提示**；落点：core 方法 §4.4 + main.rs 步骤 2.5） | [手测] v2 已装自启的机器装 v3：旧值消失、新值出现、开机自启的是 v3 | A |
| G3 | 值数据形态 | DesktopStartupHelper.cs `SetValue(_Key, _executablePath)` | exe 完整路径，REG_SZ，**不带引号** | 同形态（`current_exe()`） | [手测] | A |
| G4 | Check 忽略大小写 | DesktopStartupHelper.cs `OrdinalIgnoreCase` | 值名比较忽略大小写 | 同 | [单测] | A |
| G5 | 默认值与每次启动应用 | Setting.cs:40；App.xaml.cs:42-43 + EyeNurseService.cs:68-69 | RunWhenStarts 默认 **true**；且 `Init→ApplySetting` **每次启动都执行**（开关关掉时每次启动都会删注册表值） | v3 在 main 启动序列第 2.6 步每次启动 `startup.set_enabled(setting.RunWhenStarts)`（架构 §5.1；漏掉则首启不写自启、关闭开关后残留失效值） | [手测] 首启后查注册表有值；关开关→重启→注册表无值；开开关→重启→有值 | A/B |
| G6 | 失败静默 | DesktopStartupHelper.cs catch → false | 写失败不崩溃、界面无提示 | 返回 false 记日志 | [单测] | A |

## H. i18n（归属 A/D）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| H1 | 文件位置/格式 | EyeNurseService.cs:65-66 + Assets/Languages/*.json | exe 旁 `Assets/Languages/{zh,en}.json`，平铺 key-value | 同位置同格式（v3 文件合并设置窗键） | [字节] 结构比对 | D |
| H2 | 缺失键回退 en | EyeNurseService.cs:66（第 4 参 "en"） | 当前语言无键 → 英文 | `get()`：current→en→key | [单测] | A |
| H3 | 未设置语言 | Languages.cs CurrentLan=null | 由 LanService 决定（未深究）→ **Q7 决议**：v3 取系统 locale，zh 开头→zh 否则 en | 同 | [单测] | A |
| H4 | 切语言持久化 | ClientApi.cs:36-45 | 写 Languages.json + 刷新托盘文案 | 同（Vue 侧还存 localStorage 的行为不迁移） | [手测] | D/B |
| H5 | 中文渲染 | —（WPF 自带） | — | egui 需加载系统字体 msyh.ttc（架构 §7.5），缺字体表现为方块=失败 | [手测] 中文界面无"豆腐块" | C |

## I. 设置窗（归属 D；替代 v2 WebView2，行为对齐 `src/eyenurse-client-ui`）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| I1 | 窗口尺寸 | MainWindow.xaml:11-12 | 900×500，居中启动 | 同（逻辑点） | [手测] | D |
| I2 | 控件集 | Settings.vue:19-78 | 开机启动开关；锁屏重置开关+超时输入；休息间隔输入；休息时长输入，占位 `HH:mm:ss` | 同四项同布局 | [手测] | D |
| I3 | 保存时机 | Settings.vue:94-100 | 输入 blur 后 300ms 防抖保存，回写 UI | 失焦/回车提交，防抖 300ms；保存后用服务端回包刷新显示（等价 SetSettings 返回值语义） | [手测] | D |
| I4 | 非法时长输入 | Setting.cs:23-33 | 解析失败静默回默认并回显 | 同（保存后回读刷新） | [手测] 输入 "abc" | D/A |
| I5 | 语言下拉 | Footer.vue:12-68 | 列出可用语言，选中即 SetCurrentLanguage | 下拉列出 `Assets/Languages/*.json` 文件名 | [手测] | D |

## J. 进程级行为（归属 B/D）

| # | 条目 | v2 证据 | v2 行为 | v3 预期 | 验证 | 归属 |
|---|---|---|---|---|---|---|
| J1 | 启动即显示悬浮条 | App.xaml.cs:94-98 | OnStartup → ShowCountdownWindow | v3 root 视口常驻即悬浮条 | [手测] | B |
| J2 | 全局异常不闪退 | EyeNurseService.cs:62 | CatchApplicationError | v3：主循环错误记日志（E2E 手测无崩溃） | [手测] | B |
| J3 | 单实例 | —（v2 未实现） | 无 | v3 新增：CreateMutexW("Local\\EyeNurse3") 防双开 | [手测] 双击两次仅一个实例 | B |
| J4 | 打包产物 | docs/Build.md + src/setup | Inno Setup 安装包 | D 提供 cargo build 脚本 + 沿用现有 iss（或出 v3 模板） | [手测] 干净机安装 | D |
| J5 | 无控制台黑窗 | EyeNurse.csproj `<OutputType>WinExe</OutputType>` | v2 是 GUI 子系统，无控制台窗口 | v3 main.rs 顶部 `#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]`（架构 §5.1；debug 构建保留控制台） | [手测] 双击 release exe 与 `cargo run --release` 均无控制台 | B |

---

## Q. v2 怪癖决议（实现前必须逐条勾选：保留 / 修复）

| # | 怪癖 | v2 证据 | 默认决议（未勾选则按此实现） |
|---|---|---|---|
| Q1 | `Stop()` 未启动也发一次 Elapsed | GlobalTimerService.cs:64 | **保留**（测试锁定 A4） |
| Q2 | 休息实际时长 = RestDuration + 1s | EyeNurseViewModel.cs:212-217 | **保留**（否则与 v2 手感不同） |
| Q3 | `ResetWhenSessionUnlock` 开关保存但不生效 | EyeNurseViewModel.cs:63-77 | **保留**，待产品确认后改 |
| Q4 | 遮罩显示器列表取启动快照 | EyeNurseViewModel.cs:46 | **修复**：休息开始时枚举（C10 已注明有意偏差） |
| Q5 | 遮罩不覆盖任务栏（用 rcWork） | EyeNurseViewModel.cs:189-192 | **保留**（v2 外观） |
| Q6 | 自启值名 "Eye Nurse 2" | EyeNurseService.cs:34 | v3 用 `"Eye Nurse 3"` + 启动时**静默迁移**（写新删旧，无提示；落点与验收见 G2 / 架构 §4.4） |
| Q7 | CurrentLan=null 的语言选择未定义 | EyeNurseService.cs:66 | 按 locale 规则（H3） |
| Q8 | 进度条颜色/文字色的 480s 固定分母 | CountdownWindow.xaml:25-34 | **保留**（视觉 parity） |
| Q9 | 悬浮条位置保存分支疑似死代码（v2 实际不保存位置） | CountdownWindow.xaml.cs:33-46（DragMove 阻塞至释放 + MouseUp 被系统 move 循环吞掉，详见 B6） | **修复**：v3 拖动结束即保存；B6 只验收 v3 新语义 |
| Q10 | GetSettings 覆盖 RunWhenStarts 是死代码（UI 显示磁盘值） | ClientApi.cs:51-55（先 Serialize 后覆盖） | **修复**：v3 显示注册表实测值（落实 v2 注释意图，见 F10） |
| Q11 | 暂停后调用 Start() 在 v2 会成功（守卫只查 Enabled），v2 UI 不可达且无测试锁定 | GlobalTimerService.cs:43 | **有意偏差**：v3 `start()` 暂停期间返回 false；"逐条等价"限定为 v2 UI 可达序列（A14） |
| Q12 | 托盘菜单可见性无条件互换 vs 暂停展示门控，是两条独立轨道 | App.xaml.cs:129-143（先翻菜单再调 Pause()） | **保留**：菜单互换无条件对齐 v2；暂停展示按 core 返回值门控（D5/D8，架构 §6.3） |
| Q13 | `mm:ss` 的 mm 是 TimeSpan 分钟分量（00-59），>59min 回绕显示（90min 间隔显示 "30:00" 起） | CountdownWindow.xaml:58（`StringFormat=mm\:ss`） | **保留**（v2 视觉）；v3 同公式，不做总分钟换算（B2/B9） |

---

## 验收基线（一次性）

1. `[字节]` 用 v2 正式版产生的三个 json 冷启动 v3 → F12 全过。
2. `[手测]` 与 v2 同屏并跑 45min：倒计时视觉一致（B1/B2/B3）。
3. `[手测]` 全休息流程：托盘立刻休息 → 双屏遮罩 → 解锁按钮（即时性 C11）→ 计时器回满（C1-C10）。
4. `[手测]` Win+L 锁屏 10s 解锁：暂停/恢复；锁 >ResetTimeout：重置；休息中锁屏无暂停态（D1-D4、D8）。
5. `[单测]` `cargo test -p eyenurse-core` 全绿（A 系列全项）。
6. `[字节]` v3 保存默认配置后三个文件与 §F3/F4/F5 期望串逐字节一致。
