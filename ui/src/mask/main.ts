// 休息遮罩页（无框架，纯 TS；tauri 架构文档 §5.1；实现者 D 所有）。
// - 背景 #141418、文字 #EEEEF2（C2，v2 LockScreenWindow.xaml 值，真源 theme.json）；
// - `listen("eyenurse://rest-tick")` 更新倒计时；mm:ss 渲染公式复刻 B2/Q13：
//   mm = Math.floor(totalSeconds / 60) % 60（分钟分量，>59min 回绕），ss = totalSeconds % 60；
// - 右下角解锁按钮（v2 LockUnLocked 图标 25×25，Margin 0,0,12,12）→ invoke("rest_done")
//   （v2 CloseCommand，C5/C11）；
// - 文案走 i18n：`get_texts` 整包获取 + `listen("eyenurse://i18n-changed")` 重取；
//   缺键回退 key 原文（与 I18n::get 第三级回退一致）。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyThemeVars } from "../shared/theme";
import "./mask.css";

interface RestTickPayload {
  remaining_ms: number;
}

type TextMap = Record<string, string>;

const countdownEl = document.getElementById("countdown") as HTMLDivElement;
const unlockEl = document.getElementById("unlock") as HTMLButtonElement;

/** 文案应用：解锁按钮的悬停提示与读屏标签（v2 图标按钮无可见文字，i18n 走 aria/title） */
function applyTexts(texts: TextMap): void {
  const label = texts["unlock"] ?? "unlock";
  unlockEl.title = label;
  unlockEl.setAttribute("aria-label", label);
}

function formatMmss(totalSeconds: number): string {
  // B2/Q13：mm 是分钟分量（00-59）而非总分钟，>59min 回绕
  const mm = Math.floor(totalSeconds / 60) % 60;
  const ss = totalSeconds % 60;
  return `${String(mm).padStart(2, "0")}:${String(ss).padStart(2, "0")}`;
}

function renderTick(payload: RestTickPayload): void {
  countdownEl.textContent = formatMmss(Math.floor(payload.remaining_ms / 1000));
}

// 解锁（遮罩唯一关闭入口；窗口 close 被 Rust 侧 prevent_close 拦截，§5.3）
unlockEl.addEventListener("click", () => {
  invoke("rest_done").catch(() => {});
});

applyThemeVars();

void (async () => {
  await listen<RestTickPayload>("eyenurse://rest-tick", (ev) => renderTick(ev.payload));
  await listen("eyenurse://i18n-changed", async () => {
    try {
      applyTexts(await invoke<TextMap>("get_texts"));
    } catch {
      /* 文案缺失时保持原值（key 回退已兜底） */
    }
  });
  // 文案启动依赖（§4.1）；极端情况下显示 key 原文，与 I18n::get 第三级回退一致
  try {
    applyTexts(await invoke<TextMap>("get_texts"));
  } catch {
    /* 无后端（如纯前端 dev）时按钮无提示标签，不影响倒计时 */
  }
})();
