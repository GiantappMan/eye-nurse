// 休息/锁屏遮罩页（无框架，纯 TS；tauri 架构文档 §5.1 + v3.2 仿锁屏；实现者 D 所有）。
// - 背景 #141418、文字 #EEEEF2（C2，v2 LockScreenWindow.xaml 值，真源 theme.json）；
// - `listen("eyenurse://rest-tick")` 驱动三态：resting（倒计时）/ locked（时钟+密码）/
//   叠加；mm:ss 渲染公式复刻 B2/Q13：mm = Math.floor(totalSeconds / 60) % 60
//   （分钟分量，>59min 回绕），ss = totalSeconds % 60；
// - 右下角解锁按钮（v2 LockUnLocked 图标 25×25，Margin 0,0,12,12）：
//   未锁 → invoke("rest_done")（v2 CloseCommand，C5/C11）；
//   已锁 → invoke("lock_try_unlock", { password })，false = 密码错误（v3.2）；
// - 文案走 i18n：`get_texts` 整包获取 + `listen("eyenurse://i18n-changed")` 重取；
//   缺键回退 key 原文（与 I18n::get 第三级回退一致）；
// - ESP-1 皮肤宿主（shared/skinHost.ts）：皮肤声明 lock 表面时以 sandbox iframe
//   接管画面（握手通过才显示，看门狗失败回退内置界面）；密码框/错误提示是宿主
//   独占 chrome（百分比摆放由皮肤 manifest 声明），皮肤只经 postMessage 白名单
//   动作参与交互——密码永远不过 iframe，防爆破限速在 CoreLoop 侧。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applySkinVars, applyThemeVars } from "../shared/theme";
import { createSkinHost, type SkinSurfaceDto } from "../shared/skinHost";
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
// —— ESP-1 皮肤宿主元素 ——
const skinFrame = document.getElementById("skin-frame") as HTMLIFrameElement;
const skinPwdRow = document.getElementById("skin-pwd-row") as HTMLDivElement;
const skinPwdEl = document.getElementById("skin-pwd") as HTMLInputElement;
const skinLockErr = document.getElementById("skin-lock-err") as HTMLParagraphElement;

/** 后端显示态（rest-tick 每拍同步；未收到首拍前三态全关，面板隐藏） */
let resting = false;
let locked = false;
let hasPassword = false;
/** 皮肤页已握手并接管画面（body.skin-alive；失败/未启用 = false → 内置界面） */
let skinAlive = false;

/** 文案应用：解锁按钮/密码框/错误提示（v2 图标按钮无可见文字，i18n 走 aria/title） */
function applyTexts(map: TextMap): void {
  const label = map["unlock"] ?? "unlock";
  unlockEl.title = label;
  unlockEl.setAttribute("aria-label", label);
  lockedTitle.textContent = map["lockedTitle"] ?? "lockedTitle";
  pwdEl.placeholder = map["lockUnlockPlaceholder"] ?? "lockUnlockPlaceholder";
  skinPwdEl.placeholder = pwdEl.placeholder;
  setLockError(false, map["lockWrongPassword"] ?? "lockWrongPassword");
  skinHost?.postTexts(map); // 整包转发给皮肤页（皮肤自取所需键）
}

function formatMmss(totalSeconds: number): string {
  // B2/Q13：mm 是分钟分量（00-59）而非总分钟，>59min 回绕
  const mm = Math.floor(totalSeconds / 60) % 60;
  const ss = totalSeconds % 60;
  return `${String(mm).padStart(2, "0")}:${String(ss).padStart(2, "0")}`;
}

/** 锁屏错误提示双写（内置 + 皮肤 chrome 两套元素同态） */
function setLockError(visible: boolean, text?: string): void {
  if (text !== undefined) {
    lockErr.textContent = text;
    skinLockErr.textContent = text;
  }
  lockErr.hidden = !visible;
  skinLockErr.hidden = !visible;
}

function renderTick(payload: RestTickPayload): void {
  resting = payload.resting;
  locked = payload.locked;
  hasPassword = payload.has_password;
  countdownEl.textContent = formatMmss(Math.floor(payload.remaining_ms / 1000));
  spinnerStack.style.display = resting ? "" : "none";
  lockPanel.hidden = !locked;
  pwdRow.hidden = !hasPassword;
  if (!locked) setLockError(false);
  if (locked && hasPassword) {
    // 焦点落进可见的密码框（皮肤接管时是宿主 chrome 的那只）
    (skinAlive ? skinPwdEl : pwdEl).focus();
  }
  // 皮肤模式 chrome 三态镜像 + 状态转发（1Hz；皮肤未就绪时 skinHost 丢弃）
  skinPwdRow.hidden = !locked || !hasPassword;
  skinHost?.postState({
    remainingMs: payload.remaining_ms,
    resting: payload.resting,
    locked: payload.locked,
    hasPassword: payload.has_password,
  });
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

/** 解锁入口：未锁 = 结束休息；已锁 = 交密码（CoreLoop 验证，false = 密码错误）。
 * 内置与皮肤 chrome 两只密码框共用同一条提交路径 */
function submitUnlock(password: string): void {
  if (!locked) {
    invoke("rest_done").catch(() => {});
    return;
  }
  invoke<boolean>("lock_try_unlock", { password })
    .then((ok) => {
      if (ok) {
        setLockError(false);
        pwdEl.value = "";
        skinPwdEl.value = "";
      } else {
        setLockError(true); // 密码错误（CoreLoop 限速期同样回 false）
        (skinAlive ? skinPwdEl : pwdEl).select();
      }
    })
    .catch(() => {
      /* CoreLoop 未应答：下一拍 tick 重同步状态 */
    });
}

function requestUnlock(): void {
  submitUnlock(pwdEl.value);
}

unlockEl.addEventListener("click", requestUnlock);
pwdEl.addEventListener("keydown", (ev) => {
  if (ev.key === "Enter") requestUnlock();
});
skinPwdEl.addEventListener("keydown", (ev) => {
  if (ev.key === "Enter") submitUnlock(skinPwdEl.value);
});

/** 皮肤 manifest 声明的 chrome 摆放 → 宿主独占控件内联样式（百分比定位） */
function applyChrome(dto: SkinSurfaceDto): void {
  skinPwdRow.style.left = `${dto.lock_chrome.password_x}%`;
  skinPwdRow.style.top = `${dto.lock_chrome.password_y}%`;
  skinLockErr.style.left = `${dto.lock_chrome.password_x}%`;
  skinLockErr.style.top = `${Math.min(dto.lock_chrome.password_y + 6, 100)}%`;
  unlockEl.style.left = `${dto.lock_chrome.unlock_x}%`;
  unlockEl.style.top = `${dto.lock_chrome.unlock_y}%`;
  unlockEl.style.right = "auto";
  unlockEl.style.bottom = "auto";
}

/** 退出皮肤模式：还原解锁按钮的默认 right/bottom 定位 */
function resetChrome(): void {
  unlockEl.style.left = "";
  unlockEl.style.top = "";
  unlockEl.style.right = "";
  unlockEl.style.bottom = "";
}

// ESP-1 皮肤宿主（握手看门狗 / 表面热切换 / 动作白名单都在 skinHost 内）
const skinHost = createSkinHost(skinFrame, {
  pickUrl: (dto) => dto.lock_url,
  onSurface: (dto) => applyChrome(dto),
  onSkinAlive: (alive) => {
    skinAlive = alive;
    document.body.classList.toggle("skin-alive", alive);
    if (alive) {
      applyChrome(skinHost.surface as SkinSurfaceDto);
      if (locked && hasPassword) skinPwdEl.focus();
    } else {
      resetChrome();
      // 焦点交还内置密码框（锁屏态下保持可键入）
      if (locked && hasPassword) pwdEl.focus();
    }
  },
  onAction: (action, payload) => {
    switch (action) {
      case "endRest":
        submitUnlock("");
        break;
      case "tryUnlock":
        submitUnlock(typeof payload.password === "string" ? payload.password : "");
        break;
      case "openSettings":
        invoke("open_settings").catch(() => {});
        break;
      // lock 表面用不到的动作（drag/setTopmost）静默忽略
    }
  },
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
