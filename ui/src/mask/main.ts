// 休息/锁屏遮罩页（无框架，纯 TS；tauri 架构文档 §5.1 + v3.2 仿锁屏；实现者 D 所有）。
// - 背景 #141418、文字 #EEEEF2（C2，v2 LockScreenWindow.xaml 值，真源 theme.json）；
// - `listen("eyenurse://rest-tick")` 驱动三态：resting（倒计时）/ locked（时钟+密码）/
//   叠加；mm:ss 渲染公式复刻 B2/Q13：mm = Math.floor(totalSeconds / 60) % 60
//   （分钟分量，>59min 回绕），ss = totalSeconds % 60；
// - 右下角解锁按钮（v2 LockUnLocked 图标 25×25，Margin 0,0,12,12）：
//   未锁 → invoke("rest_done")（v2 CloseCommand，C5/C11）；
//   已锁 → invoke("lock_try_unlock", { password })，false = 密码错误（v3.2）；
// - 文案走 i18n：`get_texts` 整包获取 + `listen("eyenurse://i18n-changed")` 重取；
//   缺键回退 key 原文（与 I18n::get 第三级回退一致）。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applySkinVars, applyThemeVars } from "../shared/theme";
import "./mask.css";

interface RestTickPayload {
  remaining_ms: number;
  resting: boolean;
  locked: boolean;
  has_password: boolean;
}

type TextMap = Record<string, string>;

const spinnerStack = document.querySelector(".spinner-stack") as HTMLDivElement;
const countdownEl = document.getElementById("countdown") as HTMLDivElement;
const unlockEl = document.getElementById("unlock") as HTMLButtonElement;
const lockPanel = document.getElementById("lock-panel") as HTMLDivElement;
const lockedTitle = document.getElementById("locked-title") as HTMLParagraphElement;
const clockEl = document.getElementById("clock") as HTMLDivElement;
const lockDateEl = document.getElementById("lock-date") as HTMLDivElement;
const pwdRow = document.getElementById("pwd-row") as HTMLDivElement;
const pwdEl = document.getElementById("pwd") as HTMLInputElement;
const lockErr = document.getElementById("lock-err") as HTMLParagraphElement;

/** 后端显示态（rest-tick 每拍同步；未收到首拍前三态全关，面板隐藏） */
let resting = false;
let locked = false;
let hasPassword = false;

/** 文案应用：解锁按钮/密码框/错误提示（v2 图标按钮无可见文字，i18n 走 aria/title） */
function applyTexts(map: TextMap): void {
  const label = map["unlock"] ?? "unlock";
  unlockEl.title = label;
  unlockEl.setAttribute("aria-label", label);
  lockedTitle.textContent = map["lockedTitle"] ?? "lockedTitle";
  pwdEl.placeholder = map["lockUnlockPlaceholder"] ?? "lockUnlockPlaceholder";
  lockErr.textContent = map["lockWrongPassword"] ?? "lockWrongPassword";
}

function formatMmss(totalSeconds: number): string {
  // B2/Q13：mm 是分钟分量（00-59）而非总分钟，>59min 回绕
  const mm = Math.floor(totalSeconds / 60) % 60;
  const ss = totalSeconds % 60;
  return `${String(mm).padStart(2, "0")}:${String(ss).padStart(2, "0")}`;
}

function renderTick(payload: RestTickPayload): void {
  resting = payload.resting;
  locked = payload.locked;
  hasPassword = payload.has_password;
  countdownEl.textContent = formatMmss(Math.floor(payload.remaining_ms / 1000));
  spinnerStack.style.display = resting ? "" : "none";
  lockPanel.hidden = !locked;
  pwdRow.hidden = !hasPassword;
  if (!locked) lockErr.hidden = true;
  if (locked && hasPassword) pwdEl.focus();
}

/** 锁屏时钟：本地时间 HH:MM + 日期（1s 刷新；仅 locked 可见，空转成本忽略） */
function renderClock(): void {
  const now = new Date();
  clockEl.textContent = `${String(now.getHours()).padStart(2, "0")}:${String(
    now.getMinutes(),
  ).padStart(2, "0")}`;
  lockDateEl.textContent = now.toLocaleDateString();
}
setInterval(renderClock, 1000);

/** 解锁入口：未锁 = 结束休息；已锁 = 交密码（CoreLoop 验证，false = 密码错误） */
function requestUnlock(): void {
  if (!locked) {
    invoke("rest_done").catch(() => {});
    return;
  }
  invoke<boolean>("lock_try_unlock", { password: pwdEl.value })
    .then((ok) => {
      if (ok) {
        lockErr.hidden = true;
        pwdEl.value = "";
      } else {
        lockErr.hidden = false; // 密码错误
        pwdEl.select();
      }
    })
    .catch(() => {
      /* CoreLoop 未应答：下一拍 tick 重同步状态 */
    });
}

unlockEl.addEventListener("click", requestUnlock);
pwdEl.addEventListener("keydown", (ev) => {
  if (ev.key === "Enter") requestUnlock();
});

applyThemeVars();
renderClock();

// v3.1 换肤：静态 theme.json 只是首帧兜底，启动即取当前皮肤覆盖；
// 运行中切换皮肤 → skin-changed 广播载荷即完整配色（Rust 侧已逐键回退）
async function applySkin(): Promise<void> {
  try {
    applySkinVars(await invoke<Record<string, string>>("get_skin"));
  } catch {
    /* 无后端（纯前端 dev）时保持 theme.json 静态值 */
  }
}
void applySkin();

void (async () => {
  await listen<RestTickPayload>("eyenurse://rest-tick", (ev) => renderTick(ev.payload));
  await listen<Record<string, string>>("eyenurse://skin-changed", (ev) =>
    applySkinVars(ev.payload),
  );
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
