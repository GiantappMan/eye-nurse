// bar 皮肤壳页（ESP-1；shared/skinHost.ts 同款 iframe + postMessage 桥）。
// barweb 窗口仅在当前皮肤声明 bar HTML 表面时创建（main.rs 启动期定死），
// 本页职责：
// - 皮肤 iframe 接管画面；握手失败/未就绪 → 回退 #builtin 迷你条（配色 =
//   当前皮肤逐键回退值），悬浮条永不隐形；
// - 1Hz `eyenurse://bar-state`（CoreLoop 广播，数据源 = 共享快照）转发进皮肤，
//   同时驱动 #builtin；
// - 皮肤动作白名单：drag（系统拖动）/ openSettings / setTopmost——皮肤 iframe
//   拿不到 IPC，全经此壳转 invoke；双击 = 打开设置（GDI 条同款）。
// mm:ss 公式复刻 B2/Q13：mm 为分钟分量（>59min 回绕）。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applySkinVars } from "../shared/theme";
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

/** 回退迷你条：与 GDI 条同一数据语义（resting 文案 / 剩余 mm:ss / 进度比例） */
function renderBuiltin(): void {
  if (!lastState) return;
  if (skinAlive) return; // 皮肤接管中不渲染（display 由 CSS 控制，这里省计算）
  if (lastState.resting) {
    label.textContent = texts["resting"] ?? "resting";
    fill.style.width = "100%";
    return;
  }
  const paused = lastState.paused_ms !== null;
  label.textContent = formatMmss(Math.floor(lastState.countdown_ms / 1000));
  label.style.opacity = paused ? "0.5" : "1";
  const interval = Math.max(lastState.interval_ms, 1);
  const ratio = Math.min(Math.max(lastState.countdown_ms / interval, 0), 1);
  fill.style.width = `${(ratio * 100).toFixed(1)}%`;
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
    if (!alive) renderBuiltin(); // 看门狗失败 → 立即回退
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

// 启动：整包文案（皮肤页由 skinHost 转发）；配色在 skinHost.onSurface 注入
void (async () => {
  try {
    texts = await invoke<TextMap>("get_texts");
    skinHost.postTexts(texts);
  } catch {
    /* 无后端（纯前端 dev）时显示 key 原文 */
  }
})();
