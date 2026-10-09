// 休息/锁屏遮罩壳页（ESP-1 headless；实现者 D 所有）。
//
// 本页没有内置界面：画面全部来自皮肤 iframe（core 保证 lock URL 恒有——激活
// 皮肤无 lock 表面时回退内置 classic 包）。壳页职责只有三件：
// 1. 皮肤宿主（shared/skinHost.ts）：sandbox iframe + postMessage 白名单桥 +
//    3s 握手看门狗；看门狗失败 → 换源内置 classic 包锁屏（一次性，防回退环）；
// 2. 宿主独占 chrome：密码框（#skin-pwd）、错误提示、解锁逃生按钮——皮肤不可
//    达，密码永不过 iframe；三态由 rest-tick 驱动（locked/has_password）；
// 3. 解锁路径：未锁 → invoke("rest_done")（v2 CloseCommand，C5/C11）；已锁 →
//    invoke("lock_try_unlock")，false = 密码错误（CoreLoop 限速：1 次/秒 +
//    连错 5 次冷却 60s）。
// 文案走 i18n：get_texts 整包 + i18n-changed 重取，缺键回退 key 原文。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applySkinVars } from "../shared/theme";
import { createSkinHost, type SkinSurfaceDto } from "../shared/skinHost";
import "./mask.css";

interface RestTickPayload {
  remaining_ms: number;
  resting: boolean;
  locked: boolean;
  has_password: boolean;
}

type TextMap = Record<string, string>;

/** 内置 classic 包锁屏（看门狗回退目标；core 内嵌资产，恒可加载） */
const CLASSIC_LOCK_FALLBACK_URL =
  "http://eyenurse-skin.localhost/classic/lock/index.html";

const unlockEl = document.getElementById("unlock") as HTMLButtonElement;
const skinPwdRow = document.getElementById("skin-pwd-row") as HTMLDivElement;
const skinPwdEl = document.getElementById("skin-pwd") as HTMLInputElement;
const skinLockErr = document.getElementById("skin-lock-err") as HTMLParagraphElement;

/** 后端显示态（rest-tick 每拍同步；未收到首拍前三态全关） */
let locked = false;
let hasPassword = false;
/** 皮肤页已握手并接管画面（body.skin-alive） */
let skinAlive = false;
/** 看门狗已触发过 classic 回退（一次性——classic 自身损坏时不循环换源） */
let fellBack = false;

/** 文案应用：解锁按钮/密码框/错误提示（v2 图标按钮无可见文字，i18n 走 aria/title） */
function applyTexts(map: TextMap): void {
  const label = map["unlock"] ?? "unlock";
  unlockEl.title = label;
  unlockEl.setAttribute("aria-label", label);
  skinPwdEl.placeholder = map["lockUnlockPlaceholder"] ?? "lockUnlockPlaceholder";
  setLockError(false, map["lockWrongPassword"] ?? "lockWrongPassword");
  skinHost.postTexts(map); // 整包转发给皮肤页（皮肤自取所需键）
}

/** 锁屏错误提示（chrome 单元素） */
function setLockError(visible: boolean, text?: string): void {
  if (text !== undefined) skinLockErr.textContent = text;
  skinLockErr.hidden = !visible;
}

function renderTick(payload: RestTickPayload): void {
  locked = payload.locked;
  hasPassword = payload.has_password;
  skinPwdRow.hidden = !locked || !hasPassword;
  if (!locked) setLockError(false);
  if (locked && hasPassword) skinPwdEl.focus(); // 锁屏必须可键入（焦点逐拍兜底）
  // 状态转发（1Hz；皮肤未就绪时 skinHost 丢弃，下一拍自会补上）
  skinHost.postState({
    remainingMs: payload.remaining_ms,
    resting: payload.resting,
    locked: payload.locked,
    hasPassword: payload.has_password,
  });
}

/** 解锁入口：未锁 = 结束休息；已锁 = 交密码（CoreLoop 验证，false = 密码错误）。
 * 皮肤页经桥动作（endRest/tryUnlock）与本按钮/密码框共用同一条提交路径 */
function submitUnlock(password: string): void {
  if (!locked) {
    invoke("rest_done").catch(() => {});
    return;
  }
  invoke<boolean>("lock_try_unlock", { password })
    .then((ok) => {
      if (ok) {
        setLockError(false);
        skinPwdEl.value = "";
      } else {
        setLockError(true); // 密码错误（限速期同样回 false）
        skinPwdEl.select();
      }
    })
    .catch(() => {
      /* CoreLoop 未应答：下一拍 tick 重同步状态 */
    });
}

unlockEl.addEventListener("click", () => submitUnlock(""));
skinPwdEl.addEventListener("keydown", (ev) => {
  if (ev.key === "Enter") submitUnlock(skinPwdEl.value);
});

/** 皮肤 manifest 声明的 chrome 摆放 → 宿主独占控件内联样式（百分比定位） */
function applyChrome(dto: SkinSurfaceDto): void {
  skinPwdRow.style.left = `${dto.lock_chrome.password_x}%`;
  skinPwdRow.style.top = `${dto.lock_chrome.password_y}%`;
  skinLockErr.style.left = `${dto.lock_chrome.password_x}%`;
  skinLockErr.style.top = `${Math.min(dto.lock_chrome.password_y + 6, 100)}%`;
  const ux = dto.lock_chrome.unlock_x;
  const uy = dto.lock_chrome.unlock_y;
  if (ux !== null && uy !== null) {
    unlockEl.style.left = `${ux}%`;
    unlockEl.style.top = `${uy}%`;
    unlockEl.style.right = "auto";
    unlockEl.style.bottom = "auto";
  } else {
    resetUnlockPos();
  }
}

/** 退出皮肤模式/未声明位置：还原解锁按钮默认右下 12px 定角（classic 外观） */
function resetUnlockPos(): void {
  unlockEl.style.left = "";
  unlockEl.style.top = "";
  unlockEl.style.right = "";
  unlockEl.style.bottom = "";
}

// ESP-1 皮肤宿主（握手看门狗 / 表面热切换 / 动作白名单都在 skinHost 内）
const skinHost = createSkinHost(skinFrame(), {
  pickUrl: (dto) => dto.lock_url,
  onSurface: (dto) => {
    applyChrome(dto);
    // 配色注入 CSS 变量（chrome 与皮肤同色；classic 包锁屏经 colors 桥另取）
    applySkinVars(dto.colors);
  },
  onSkinAlive: (alive) => {
    skinAlive = alive;
    document.body.classList.toggle("skin-alive", alive);
    if (alive) {
      if (locked && hasPassword) skinPwdEl.focus();
      return;
    }
    resetUnlockPos();
    // 看门狗失败：激活皮肤未握手 → 换源内置 classic 包（一次性）。core 只对
    // 「无 lock 声明」给 classic URL；损坏的是激活皮肤本身时这里二次换源
    const surface = skinHost.surface;
    if (!fellBack && surface?.lock_url && surface.lock_url !== CLASSIC_LOCK_FALLBACK_URL) {
      fellBack = true;
      skinHost.reload(CLASSIC_LOCK_FALLBACK_URL);
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
      // bar 专属动作（drag/setTopmost/openMenu）在 lock 表面静默忽略
    }
  },
});

function skinFrame(): HTMLIFrameElement {
  return document.getElementById("skin-frame") as HTMLIFrameElement;
}

applySkinVarsOnBoot();

/** 首帧兜底配色（theme.json 静态值；get_skin_surface 到达后即被皮肤覆盖） */
async function applySkinVarsOnBoot(): Promise<void> {
  try {
    applySkinVars(await invoke<Record<string, string>>("get_skin"));
  } catch {
    /* 无后端（纯前端 dev）时保持 CSS 静态值 */
  }
}

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
    /* 无后端（如纯前端 dev）时按钮无提示标签 */
  }
})();
