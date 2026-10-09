// ESP-1 皮肤宿主壳（mask / barweb 两页共用；docs/v3-skin-protocol.md §3）。
//
// 安全模型：皮肤页经 `eyenurse-skin://` 加载进 `sandbox="allow-scripts"` iframe
// （opaque origin）——皮肤 JS 拿不到 Tauri IPC、摸不到宿主 DOM；宿主与皮肤之间
// 只走 postMessage：
// - 宿主 → 皮肤：{ type: "eyenurse-state" | "eyenurse-texts", ... }（单向推送，
//   皮肤无需也无法请求数据）；
// - 皮肤 → 宿主：{ type: "eyenurse-hello" }（加载成功握手，看门狗用）与
//   { type: "eyenurse-action", action, ...payload }（动作白名单，见 SKIN_ACTIONS）。
// 白名单之外的消息一律丢弃；动作经壳页 invoke 转 CoreLoop（密码等敏感动作的
// 限速在 CoreLoop 侧，皮肤看到的只是 false）。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** 皮肤动作白名单（皮肤 iframe → 宿主；之外的消息静默丢弃） */
const SKIN_ACTIONS: ReadonlySet<string> = new Set([
  "drag", // bar：系统级拖动（GDI 条 HTCAPTION 的等价物）
  "openSettings", // 双击悬浮条 / 锁屏设置入口
  "setTopmost", // bar 置顶复选
  "endRest", // 未锁时结束休息（v2 CloseCommand）
  "tryUnlock", // 锁屏密码验证（宿主限速）
]);

/** get_skin_surface 回包（与 Rust skin_proto::SkinSurfaceDto 字段同名） */
export interface SkinSurfaceDto {
  id: string;
  colors: Record<string, string>;
  bar_url: string | null;
  lock_url: string | null;
  bar_w: number;
  bar_h: number;
  lock_chrome: {
    password_x: number;
    password_y: number;
    unlock_x: number;
    unlock_y: number;
  };
  bar_is_web: boolean;
}

/** 皮肤页握手：onload 后 helloTimeoutMs 内未收到 → 判定皮肤损坏，隐藏 iframe
 * 回退内置界面（永不黑屏/白窗——锁屏回退失败是事故） */
const HELLO_TIMEOUT_MS = 3000;

export interface SkinHostOptions {
  /** 从表面快照取本壳页关心的皮肤页 URL（mask 取 lock_url，barweb 取 bar_url） */
  pickUrl: (surface: SkinSurfaceDto) => string | null;
  /** 表面快照到达（含 chrome 摆放 / 配色；皮肤未启用时 url=null） */
  onSurface: (surface: SkinSurfaceDto, active: boolean) => void;
  /** 白名单动作（已过滤；payload 为原始附加字段） */
  onAction: (action: string, payload: Record<string, unknown>) => void;
  /** 皮肤页可用性变化（true=iframe 已挂且握手通过；false=回退内置界面） */
  onSkinAlive?: (alive: boolean) => void;
}

export interface SkinHost {
  /** 推状态进皮肤页（1Hz 数据：倒计时/三态等；皮肤未启用时静默丢弃） */
  postState: (state: Record<string, unknown>) => void;
  /** 推整包文案（i18n；低频） */
  postTexts: (texts: Record<string, string>) => void;
  /** 当前表面快照（壳页取 chrome 摆放等） */
  surface: SkinSurfaceDto | null;
  dispose: () => void;
}

/**
 * 挂载皮肤宿主：取表面快照 → 有 URL 则给 iframe 换源并启动握手看门狗；
 * 监听 skin-surface-changed 热换源；收发 postMessage。
 */
export function createSkinHost(
  frame: HTMLIFrameElement,
  opts: SkinHostOptions,
): SkinHost {
  let surface: SkinSurfaceDto | null = null;
  let alive = false;
  let helloTimer: ReturnType<typeof setTimeout> | null = null;

  const setAlive = (value: boolean): void => {
    if (alive === value) return;
    alive = value;
    opts.onSkinAlive?.(value);
  };

  const clearHelloTimer = (): void => {
    if (helloTimer !== null) {
      clearTimeout(helloTimer);
      helloTimer = null;
    }
  };

  const applySurface = (dto: SkinSurfaceDto): void => {
    const prevUrl = surface ? opts.pickUrl(surface) : null;
    surface = dto;
    const url = opts.pickUrl(dto);
    opts.onSurface(dto, Boolean(url));
    if (!url) {
      // 皮肤未声明本表面（或皮肤 id 失效回退）→ 撤 iframe，内置界面接管
      clearHelloTimer();
      setAlive(false);
      frame.removeAttribute("src");
      return;
    }
    if (url === prevUrl && alive) {
      // 同一皮肤仅配色/chrome 变化：不重载（重赋 src 会整页刷新、闪一帧内置
      // 界面），只补推新配色
      frame.contentWindow?.postMessage(
        { type: "eyenurse-colors", colors: dto.colors },
        "*",
      );
      return;
    }
    setAlive(false);
    frame.src = url;
  };

  // 皮肤 → 宿主：握手与动作白名单分发
  const onMessage = (ev: MessageEvent): void => {
    const data = ev.data as Record<string, unknown> | null;
    if (!data || typeof data !== "object") return;
    if (data.type === "eyenurse-hello") {
      clearHelloTimer();
      setAlive(true);
      // 握手即补发配色（皮肤页 CSS 变量数据源；表面切换时下方 applySurface 再推）
      frame.contentWindow?.postMessage(
        { type: "eyenurse-colors", colors: surface?.colors ?? {} },
        "*",
      );
      return;
    }
    if (data.type === "eyenurse-action") {
      const action = typeof data.action === "string" ? data.action : "";
      if (!SKIN_ACTIONS.has(action)) return; // 白名单外静默丢弃
      const { type: _t, action: _a, ...payload } = data;
      opts.onAction(action, payload);
    }
  };
  window.addEventListener("message", onMessage);

  // 看门狗：iframe onload 起 3s 内无握手 → 皮肤损坏，撤下回退内置界面
  frame.addEventListener("load", () => {
    clearHelloTimer();
    if (!frame.src) return;
    helloTimer = setTimeout(() => setAlive(false), HELLO_TIMEOUT_MS);
  });

  const unlistenSurface = Promise.resolve(
    listen<SkinSurfaceDto>("eyenurse://skin-surface-changed", (ev) => {
      applySurface(ev.payload);
    }),
  );

  // 启动：首取表面（skin-surface-changed 只在变更时广播，静态启动靠这条）
  void invoke<SkinSurfaceDto>("get_skin_surface")
    .then(applySurface)
    .catch(() => {
      /* 无后端（纯前端 dev）→ 保持内置界面 */
    });

  const post = (message: Record<string, unknown>): void => {
    if (!alive) return; // 皮肤未就绪：丢弃（1Hz 数据下一拍自会补上）
    frame.contentWindow?.postMessage(message, "*");
  };

  return {
    postState: (state) => post({ type: "eyenurse-state", ...state }),
    postTexts: (texts) => post({ type: "eyenurse-texts", texts }),
    get surface() {
      return surface;
    },
    dispose: () => {
      clearHelloTimer();
      window.removeEventListener("message", onMessage);
      void unlistenSurface.then((fn) => fn());
    },
  };
}
