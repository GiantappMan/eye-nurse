// bar 皮肤壳页（ESP-1；shared/skinHost.ts 同款 iframe + postMessage 桥）。
// barweb 窗口仅在当前皮肤声明 bar HTML 表面时创建（main.rs 启动期定死），
// 本页职责：
// - 皮肤 iframe 接管画面；握手失败/未就绪 → 回退 #builtin 迷你条（配色 =
//   当前皮肤逐键回退值；绘制语义对齐 GDI 孪生条——常态纯色、暂停按秒
//   from→to 时间渐变、字号随条高缩放），悬浮条永不隐形；
// - 1Hz `eyenurse://bar-state`（CoreLoop 广播，数据源 = 共享快照）转发进皮肤，
//   同时驱动 #builtin；
// - 皮肤动作白名单：drag（系统拖动）/ openSettings / setTopmost——皮肤 iframe
//   拿不到 IPC，全经此壳转 invoke；双击 = 打开设置（GDI 条同款）。
// mm:ss 公式复刻 B2/Q13：mm 为分钟分量（>59min 回绕）。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applySkinVars, tokens } from "../shared/theme";
import { createSkinHost, type SkinSurfaceDto } from "../shared/skinHost";
import "./barweb.css";

interface BarStatePayload {
  countdown_ms: number;
  interval_ms: number;
  paused_ms: number | null;
  resting: boolean;
  topmost: boolean;
}

type TextMap = Record<string, string>;

const builtin = document.getElementById("builtin") as HTMLDivElement;
const fill = document.getElementById("fill") as HTMLDivElement;
const label = document.getElementById("label") as HTMLSpanElement;
const skinFrame = document.getElementById("skin-frame") as HTMLIFrameElement;

let skinAlive = false;
let lastState: BarStatePayload | null = null;
let texts: TextMap = {};

function formatMmss(totalSeconds: number): string {
  const mm = Math.floor(totalSeconds / 60) % 60;
  const ss = totalSeconds % 60;
  return `${String(mm).padStart(2, "0")}:${String(ss).padStart(2, "0")}`;
}

/** 暂停渐变走满时长（秒）——同 GDI theme::COLOR_RAMP_SECONDS；插值公式按
 *  设计规范 §5.4.1（v2 converter 的 percent==0→0.1 特判不复刻：壳页从简） */
const BAR_RAMP_SECONDS = 480;

function clamp01(v: number): number {
  return Math.min(Math.max(v, 0), 1);
}

function hexToRgb(hex: string): [number, number, number] | null {
  const m = hex.trim().match(/^#([0-9a-f]{6})$/i);
  if (m === null) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** 读 :root 活色（applySkinVars 注入 / CSS 兜底数据）做时间插值：逐通道
 *  lerp(a,b,t) = a + round((b-a)*t)；解析失败返回 null（保持 CSS 兜底色） */
function rampColor(fromVar: string, toVar: string, t: number): string | null {
  const style = getComputedStyle(document.documentElement);
  const from = hexToRgb(style.getPropertyValue(fromVar));
  const to = hexToRgb(style.getPropertyValue(toVar));
  if (!from || !to) return null;
  const ch = (i: number): number => from[i] + Math.round((to[i] - from[i]) * t);
  return `rgb(${ch(0)}, ${ch(1)}, ${ch(2)})`;
}

/** 字号随条高缩放（tokens.bar：ratio 0.65、clamp [11,32]）——20px 条 = 13px
 *  与 GDI CreateFontW(-(13*dpi/96)) 同基准，44px 条（devtest）= 29px */
function applyBarFontScale(): void {
  const h = builtin.clientHeight;
  if (h <= 0) return; // 窗口尚未排版：rAF/resize 再补
  const scaled = Math.round(h * tokens.bar.fontRatio);
  label.style.fontSize = `${Math.min(Math.max(scaled, tokens.bar.fontMin), tokens.bar.fontMax)}px`;
}

/** 回退迷你条：与 GDI 条同一绘制语义（bar.rs on_paint）——休息 = 满宽纯
 *  fill + text_idle 文案；常态 = 纯 fill 按剩余比例 + 剩余 mm:ss；暂停 =
 *  fill 与文字色按暂停秒数 from→to 时间插值（同 GDI ramp，非空间渐变），
 *  数字 = 累计暂停时长（GDI format_mmss(paused.unwrap_or(countdown)) 同款走动） */
function renderBuiltin(): void {
  if (!lastState) return;
  if (skinAlive) return; // 皮肤接管中不渲染（display 由 CSS 控制，这里省计算）
  if (lastState.resting) {
    label.textContent = texts["resting"] ?? "resting";
    label.style.color = "";
    fill.style.backgroundColor = "";
    fill.style.width = "100%";
    return;
  }
  const pausedMs = lastState.paused_ms;
  label.textContent = formatMmss(
    Math.floor((pausedMs ?? lastState.countdown_ms) / 1000),
  );
  const ratio = clamp01(
    lastState.countdown_ms / Math.max(lastState.interval_ms, 1),
  );
  fill.style.width = `${(ratio * 100).toFixed(1)}%`;
  if (pausedMs === null) {
    label.style.color = "";
    fill.style.backgroundColor = "";
    return;
  }
  const t = clamp01(pausedMs / 1000 / BAR_RAMP_SECONDS);
  fill.style.backgroundColor =
    rampColor("--eyenurse-bar-progress-from", "--eyenurse-bar-progress-to", t) ??
    "";
  label.style.color =
    rampColor("--eyenurse-bar-text-from", "--eyenurse-bar-text-to", t) ?? "";
}

const skinHost = createSkinHost(skinFrame, {
  pickUrl: (dto) => dto.bar_url,
  onSurface: (dto: SkinSurfaceDto) => {
    // 回退条配色跟随当前皮肤（逐键回退后的完整配色直接注入 CSS 变量）
    applySkinVars(dto.colors);
  },
  onSkinAlive: (alive) => {
    skinAlive = alive;
    document.body.classList.toggle("skin-alive", alive);
    builtin.hidden = alive;
    if (!alive) {
      applyBarFontScale(); // 隐藏期间条高可能已变（resize 被跳过），回退前重算
      renderBuiltin(); // 看门狗失败 → 立即回退
    }
  },
  onAction: (action, payload) => {
    switch (action) {
      case "drag":
        invoke("bar_drag").catch(() => {});
        break;
      case "openMenu":
        invoke("bar_open_menu").catch(() => {});
        break;
      case "openSettings":
        invoke("open_settings").catch(() => {});
        break;
      case "setTopmost":
        invoke("set_topmost", { on: payload.on === true }).catch(() => {});
        break;
      // lock 专属动作（endRest/tryUnlock）在 bar 表面静默忽略
    }
  },
});

// 壳页自身右键（皮肤未接管时的回退迷你条）也弹托盘同源菜单
window.addEventListener("contextmenu", (ev) => {
  ev.preventDefault();
  invoke("bar_open_menu").catch(() => {});
});

void listen<BarStatePayload>("eyenurse://bar-state", (ev) => {
  lastState = ev.payload;
  skinHost.postState({
    countdownMs: ev.payload.countdown_ms,
    intervalMs: ev.payload.interval_ms,
    pausedMs: ev.payload.paused_ms,
    resting: ev.payload.resting,
    topmost: ev.payload.topmost,
  });
  renderBuiltin();
});

void listen("eyenurse://i18n-changed", async () => {
  try {
    texts = await invoke<TextMap>("get_texts");
    skinHost.postTexts(texts);
    renderBuiltin();
  } catch {
    /* 文案缺失时保持原值（key 回退已兜底） */
  }
});

// 双击 = 打开设置（GDI 条 WM_NCLBUTTONDBLCLK 同款；皮肤页自己经桥转 send）
builtin.addEventListener("dblclick", () => {
  invoke("open_settings").catch(() => {});
});

// 启动：按当前条高定字号（rAF 保证首帧排版后取值）；跨屏 DPI 变化随 resize 重算
requestAnimationFrame(applyBarFontScale);
window.addEventListener("resize", applyBarFontScale);

// 启动：整包文案（皮肤页由 skinHost 转发）；配色在 skinHost.onSurface 注入
void (async () => {
  try {
    texts = await invoke<TextMap>("get_texts");
    skinHost.postTexts(texts);
  } catch {
    /* 无后端（纯前端 dev）时显示 key 原文 */
  }
})();
