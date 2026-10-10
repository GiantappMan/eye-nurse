<script setup lang="ts">
// 皮肤模块（方案 C 侧边栏布局；由独立「皮肤中心」窗口迁入，2026-10-10 用户拍板
// 单窗口化）。卡片墙 = 浏览/预览/应用皮肤的一站式界面：
// bar 重启提示横幅 → 卡片网格（点卡片 = 打开预览弹窗）→ 自定义颜色编辑器。
//
// **预览弹窗**：加载真实皮肤页 iframe（eyenurse-skin 协议 + sandbox，与生产
// 同管线），握手后灌入**模拟桥状态**（ESP-1 皮肤页是 (state, texts, colors) 的
// 纯函数渲染——预览即真实效果，第三方皮肤零适配）：
// - 锁屏效果：1280×800 视口按比例缩放嵌入（锁屏脸按本机配置取真）；
// - 悬浮条效果：manifest 逻辑尺寸 ×2 放大居中；GDI 配色皮肤无 HTML 表面 →
//   CSS 1:1 仿样（与 GDI 条同构：底色 + 渐变填充 + mm:ss，直角是真源）。
// 预览是**惰性**的：皮肤页的动作消息（drag/setTopmost/endRest…）一律忽略。
// 文案由宿主（App.vue）加载与刷新；本组件只管皮肤数据与预览桥。
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { currentLanguage, t, texts as i18nTexts } from "../i18n";
import {
  skinPreviewUrl,
  type SkinColors,
  type SkinDefPayload,
  type SkinMeta,
  type SkinPreviewDto,
} from "../types";

const skins = ref<SkinMeta[]>([]);
const currentId = ref("");
const effective = ref<SkinColors | null>(null);
/** bar 当前运行模式（true = WebView 壳；与选中皮肤 has_bar 不一致 → 重启提示） */
const barIsWeb = ref(false);

async function refresh(): Promise<void> {
  try {
    const dto = await invoke<{ current: string; skins: SkinMeta[] }>("get_skins");
    skins.value = dto.skins;
    currentId.value = dto.current;
  } catch {
    /* CoreLoop 未就绪时静默，下个广播会再来 */
  }
  try {
    effective.value = await invoke<SkinColors>("get_skin");
  } catch {
    /* 同上 */
  }
  try {
    barIsWeb.value = ((await invoke("get_skin_surface")) as { bar_is_web: boolean })
      .bar_is_web;
  } catch {
    /* 同上 */
  }
}

/** 选中皮肤的 bar 形态与运行模式不一致 → 重启提示 */
const restartHint = computed(() => {
  const meta = skins.value.find((s) => s.id === currentId.value);
  return Boolean(meta && meta.has_bar && meta.has_bar !== barIsWeb.value);
});

async function select(id: string): Promise<void> {
  if (id === currentId.value) return;
  try {
    await invoke("set_skin", { id });
    currentId.value = id; // skin-changed 广播会再整体刷新
  } catch {
    /* CoreLoop 未就绪等瞬态：广播兜底 */
  }
}

function openSkinsFolder(): void {
  invoke("open_skins_folder").catch(() => {});
}

function displayName(meta: SkinMeta): string {
  if (meta.id === "custom") return t("skinCustom");
  return currentLanguage.value === "zh" ? meta.name : meta.name_en;
}
function badges(meta: SkinMeta): string[] {
  const out: string[] = [];
  if (meta.has_bar) out.push(t("skinTargetBar"));
  if (meta.has_lock) out.push(t("skinTargetLock"));
  return out;
}
function authorLine(meta: SkinMeta): string {
  return meta.author ? t("skinByAuthor").replace("{author}", meta.author) : "";
}
/** 无 preview.png 的卡片：配色渐变兜底底图 */
function fallbackStyle(meta: SkinMeta): Record<string, string> {
  return {
    background: `linear-gradient(135deg, ${meta.colors.mask_bg} 0%, ${meta.colors.bar_progress_bg} 62%, ${meta.colors.bar_progress_fill} 100%)`,
  };
}

// —— 预览弹窗（真实皮肤页 iframe + 模拟桥状态；动作消息一律忽略）——
const previewMeta = ref<SkinMeta | null>(null);
const previewDto = ref<SkinPreviewDto | null>(null);
const lockFrame = ref<HTMLIFrameElement | null>(null);
const barFrame = ref<HTMLIFrameElement | null>(null);
/** 锁屏预览视口（真实比例，缩放嵌入）与缩放系数 */
const LOCK_VW = 1280;
const LOCK_VH = 800;
const LOCK_SCALE = 0.4765625; // 610 / 1280（body 内容宽 620 − 10 滚动条槽；槽由
// .modal-body 的 scrollbar-gutter: stable 恒占位，滚动/非滚动两态宽度一致，
// 预览不被裁切——design.css ::-webkit-scrollbar 宽 10px）

/** 预览用模拟显示态：锁屏脸**按本机锁屏配置取真**（AutoRest → 休息+锁屏叠加态，
 * 否则纯锁屏脸——预览与实际锁屏所见一致，openPreview 时刷新）；bar = 45min
 * 间隔剩余 17min 的中间态 */
const LOCK_DEMO = { remainingMs: 95_000, resting: false, locked: true, hasPassword: true };
const BAR_DEMO = {
  countdownMs: 17 * 60 * 1000,
  intervalMs: 45 * 60 * 1000,
  pausedMs: null,
  resting: false,
  topmost: false,
};

/** mm:ss 演示读数：一律由状态推导生成，禁止手写字面量（杜绝与填充比例漂移） */
function formatMmss(ms: number): string {
  const total = Math.round(ms / 1000);
  return `${String(Math.floor(total / 60)).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
}

/** GDI 仿样演示三要素同源自洽：45min 间隔剩 17:00 → 已流逝 62%（填充），右侧留 38%。
 *  倒计时文案与填充 inset 都从 BAR_DEMO 推导，同一状态只讲一个故事 */
const BAR_MOCK_TEXT = formatMmss(BAR_DEMO.countdownMs);
const BAR_MOCK_FILL_INSET = `0 ${100 - Math.round((1 - BAR_DEMO.countdownMs / BAR_DEMO.intervalMs) * 100)}% 0 0`;

async function openPreview(meta: SkinMeta): Promise<void> {
  previewMeta.value = meta;
  previewDto.value = null;
  try {
    const lock = await invoke<{ AutoRest: boolean; Password: string }>("get_lock_settings");
    LOCK_DEMO.resting = lock.AutoRest;
    LOCK_DEMO.locked = true;
    LOCK_DEMO.hasPassword = Boolean(lock.Password);
  } catch {
    /* 无后端时保持默认纯锁屏脸 */
  }
  try {
    previewDto.value = await invoke<SkinPreviewDto>("get_skin_preview", { id: meta.id });
  } catch {
    previewMeta.value = null; // 解析失败不弹空壳
  }
}
function closePreview(): void {
  previewMeta.value = null; // v-if 卸载 iframe，消息源随之消失
  previewDto.value = null;
}
async function applyPreview(): Promise<void> {
  if (!previewMeta.value) return;
  await select(previewMeta.value.id);
  closePreview();
}

/** 灌模拟桥状态（顺序与生产 skinHost 一致：colors → texts → state） */
function pushPreviewState(win: Window | null, kind: "lock" | "bar"): void {
  if (!win || !previewMeta.value) return;
  win.postMessage({ type: "eyenurse-colors", colors: previewMeta.value.colors }, "*");
  win.postMessage({ type: "eyenurse-texts", texts: i18nTexts.value }, "*");
  win.postMessage({ type: "eyenurse-state", ...(kind === "lock" ? LOCK_DEMO : BAR_DEMO) }, "*");
}

/** 预览桥宿主：只认握手，按消息源分流到对应 iframe；动作白名单在这里不存在 */
function onPreviewMessage(ev: MessageEvent): void {
  const d = ev.data as Record<string, unknown> | null;
  if (!d || d.type !== "eyenurse-hello" || !previewMeta.value) return;
  if (lockFrame.value && ev.source === lockFrame.value.contentWindow) {
    pushPreviewState(ev.source as Window, "lock");
  } else if (barFrame.value && ev.source === barFrame.value.contentWindow) {
    pushPreviewState(ev.source as Window, "bar");
  }
}
function onKeydown(ev: KeyboardEvent): void {
  if (ev.key === "Escape" && previewMeta.value) closePreview();
}

/** GDI 配色皮肤的 bar 仿样（与原生条同构：底色 + 三段渐变填充 + mm:ss） */
const barMockStyle = computed(() => {
  const c = previewMeta.value?.colors;
  if (!c) return {};
  return {
    background: c.bar_progress_bg,
    color: c.bar_text_idle,
  };
});
const barMockFillStyle = computed(() => {
  const c = previewMeta.value?.colors;
  if (!c) return {};
  return {
    background: `linear-gradient(90deg, ${c.bar_progress_fill}, ${c.bar_progress_from}, ${c.bar_progress_to})`,
    inset: BAR_MOCK_FILL_INSET, // 与 BAR_MOCK_TEXT 同源（BAR_DEMO 推导）
  };
});

// —— 自定义颜色编辑器（展开时从生效配色预填）——
// draft 默认值 = get_skin 失败时（effective 为 null、watch 不回填）用户可保存的
// 兜底配色，必须与 theme.json / skin.rs classic() 同步（DS3 §6 同步点）。
const editorOpen = ref(false);
const draft = reactive({
  bar_progress_bg: "#F5F5DC",
  bar_progress_fill: "#3AC32E",
  bar_progress_from: "#ECC071",
  bar_progress_to: "#3E0D0D",
  bar_text_idle: "#1A1A1A",
  bar_text_from: "#080808",
  bar_text_to: "#FF5252",
  bar_check_glyph: "#444444",
  bar_check_hover_bg: "#FFFFFF",
  mask_bg: "#141418",
  mask_fg: "#EEEEF2",
  mask_error: "#FF8B8B",
});
watch([editorOpen, effective], ([open, eff]) => {
  if (open && eff) Object.assign(draft, eff);
});
const MAIN_COLORS = [
  { key: "bar_progress_bg", label: "colorBarBg" },
  { key: "bar_progress_fill", label: "colorBarProgress" },
  { key: "bar_text_idle", label: "colorBarText" },
  { key: "mask_bg", label: "colorMaskBg" },
  { key: "mask_fg", label: "colorMaskFg" },
] as const;
const PAUSED_COLORS = [
  { key: "bar_progress_from", label: "colorProgressFrom" },
  { key: "bar_progress_to", label: "colorProgressTo" },
  { key: "bar_text_from", label: "colorTextFrom" },
  { key: "bar_text_to", label: "colorTextTo" },
] as const;
const CHECK_COLORS = [
  { key: "bar_check_glyph", label: "colorCheckGlyph" },
  { key: "bar_check_hover_bg", label: "colorCheckHover" },
] as const;
const draftValid = computed(() => Object.values(draft).every((v) => /^#[0-9a-fA-F]{6}$/.test(v)));
async function saveCustom(): Promise<void> {
  try {
    effective.value = await invoke<SkinColors>("set_custom_skin", {
      def: {
        name: null,
        name_en: null,
        bar: {
          progress_bg: draft.bar_progress_bg,
          progress_fill: draft.bar_progress_fill,
          progress_from: draft.bar_progress_from,
          progress_to: draft.bar_progress_to,
          text_idle: draft.bar_text_idle,
          text_from: draft.bar_text_from,
          text_to: draft.bar_text_to,
          check_glyph: draft.bar_check_glyph,
          check_hover_bg: draft.bar_check_hover_bg,
        },
        mask: { bg: draft.mask_bg, fg: draft.mask_fg, error: draft.mask_error },
      } satisfies SkinDefPayload,
    });
    currentId.value = "custom";
    await refresh();
  } catch {
    /* CoreLoop 未就绪等瞬态 */
  }
}

onMounted(async () => {
  window.addEventListener("message", onPreviewMessage);
  window.addEventListener("keydown", onKeydown);
  await refresh();
  await listen("eyenurse://skin-changed", () => void refresh());
  await listen("eyenurse://skin-surface-changed", () => void refresh());
});
onUnmounted(() => {
  window.removeEventListener("message", onPreviewMessage);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div class="page">
    <header class="page-head">
      <div class="head-text">
        <h2 class="page-title">{{ t("navSkin") }}</h2>
        <p class="page-hint">{{ t("skinsFolderHint") }}</p>
      </div>
      <button type="button" class="en-btn en-btn-ghost" @click="openSkinsFolder">
        {{ t("openSkinsFolder") }}
      </button>
    </header>

    <!-- bar 形态变化需要重启（换色即时生效）。固定槽恒占位：出现/消失不推动
         卡片墙（§1.5 布局稳定三禁；隐藏态 visibility:hidden 退出无障碍树） -->
    <p class="banner" :class="{ shown: restartHint }">{{ t("skinRestartHint") }}</p>

    <!-- 卡片墙：点卡片 = 预览（应用在预览弹窗里） -->
    <main class="grid">
      <button
        v-for="meta in skins"
        :key="meta.id"
        type="button"
        class="card"
        :class="{ active: meta.id === currentId }"
        :title="t('skinPreview')"
        @click="openPreview(meta)"
      >
        <img
          v-if="meta.has_preview"
          class="pv"
          :src="skinPreviewUrl(meta.id)"
          :alt="displayName(meta)"
          loading="lazy"
        />
        <span v-else class="pv pv-fallback" :style="fallbackStyle(meta)">
          <span class="fake-bar" :style="{ background: meta.colors.bar_progress_bg }">
            <span
              class="fake-fill"
              :style="{ background: meta.colors.bar_progress_fill, inset: BAR_MOCK_FILL_INSET }"
            />
          </span>
          <span class="fake-ring" :style="{ borderColor: meta.colors.mask_fg }" />
        </span>
        <span class="meta">
          <span class="name-row">
            <span class="name">{{ displayName(meta) }}</span>
            <span v-if="meta.id === currentId" class="active-chip">
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path d="M5 13l4 4L19 7" />
              </svg>
              {{ t("skinActive") }}
            </span>
          </span>
          <span v-if="authorLine(meta)" class="author">{{ authorLine(meta) }}</span>
          <span v-if="badges(meta).length" class="badges">
            <span v-for="b in badges(meta)" :key="b" class="en-badge">{{ b }}</span>
          </span>
        </span>
      </button>
    </main>

    <!-- 自定义颜色编辑器（折叠） -->
    <section class="editor">
      <button
        type="button"
        class="editor-toggle"
        :aria-expanded="editorOpen"
        @click="editorOpen = !editorOpen"
      >
        <svg
          class="chev"
          :class="{ open: editorOpen }"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M9 6l6 6-6 6" />
        </svg>
        {{ t("customColor") }}
      </button>
      <div v-if="editorOpen" class="editor-body">
        <p class="group-label">{{ t("colorMain") }}</p>
        <div class="color-grid">
          <label v-for="c in MAIN_COLORS" :key="c.key" class="color-item">
            <input v-model="draft[c.key]" type="color" />
            <span>{{ t(c.label) }}</span>
          </label>
        </div>
        <p class="group-label">{{ t("colorPaused") }}</p>
        <div class="color-grid">
          <label v-for="c in PAUSED_COLORS" :key="c.key" class="color-item">
            <input v-model="draft[c.key]" type="color" />
            <span>{{ t(c.label) }}</span>
          </label>
        </div>
        <p class="group-label">{{ t("colorCheck") }}</p>
        <div class="color-grid">
          <label v-for="c in CHECK_COLORS" :key="c.key" class="color-item">
            <input v-model="draft[c.key]" type="color" />
            <span>{{ t(c.label) }}</span>
          </label>
        </div>
        <div class="editor-actions">
          <button
            type="button"
            class="en-btn en-btn-primary"
            :disabled="!draftValid"
            :title="draftValid ? '' : t('colorInvalid')"
            @click="saveCustom"
          >
            {{ t("skinSave") }}
          </button>
        </div>
      </div>
    </section>

    <!-- 预览弹窗：head + 可滚 body + 恒可见 foot（ESC / 点遮罩关闭） -->
    <div v-if="previewMeta" class="modal-mask" @click.self="closePreview">
      <div class="modal">
        <header class="modal-head">
          <div class="modal-title-wrap">
            <h3 class="modal-title">{{ displayName(previewMeta) }}</h3>
            <p v-if="authorLine(previewMeta)" class="modal-author">{{ authorLine(previewMeta) }}</p>
          </div>
          <button
            type="button"
            class="modal-close"
            :title="t('close')"
            :aria-label="t('close')"
            @click="closePreview"
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              aria-hidden="true"
            >
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </header>

        <div class="modal-body">
          <p class="section-label">{{ t("skinPreviewLock") }}</p>
          <div v-if="previewDto?.lock_url" class="lock-wrap">
            <iframe
              ref="lockFrame"
              class="lock-frame"
              :src="previewDto.lock_url"
              sandbox="allow-scripts"
              :style="{ transform: `scale(${LOCK_SCALE})`, width: LOCK_VW + 'px', height: LOCK_VH + 'px' }"
            />
          </div>

          <p class="section-label">{{ t("skinPreviewBar") }}</p>
          <div class="bar-stage">
            <template v-if="previewDto?.bar_url">
              <iframe
                ref="barFrame"
                class="bar-frame"
                :src="previewDto.bar_url"
                sandbox="allow-scripts"
                :style="{
                  width: previewDto.bar_w + 'px',
                  height: previewDto.bar_h + 'px',
                  transform: 'scale(2)',
                }"
              />
            </template>
            <template v-else>
              <div class="bar-mock" :style="barMockStyle">
                <span class="bar-mock-fill" :style="barMockFillStyle" />
                <span class="bar-mock-text">{{ BAR_MOCK_TEXT }}</span>
              </div>
              <span class="mock-note">{{ t("skinNativeBar") }}</span>
            </template>
          </div>
        </div>

        <footer class="modal-foot">
          <span class="foot-badges">
            <span v-for="b in badges(previewMeta)" :key="b" class="en-badge">{{ b }}</span>
          </span>
          <button type="button" class="en-btn en-btn-primary" @click="applyPreview">
            {{ t("skinApply") }}
          </button>
        </footer>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 布局按 docs/design-system-v3.md §3/§5.2：间距全走 --sp-*，圆角四档，
   字阶下限 11px，按钮/徽章复用 design.css 的 .en-* 共享类，
   页面不写 #rrggbb（皮肤数据与 --eyenurse-* 兜底除外）。 */
.page {
  max-width: 640px;
  margin: 0 auto;
  padding: var(--sp-5) var(--sp-5) var(--sp-4);
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  min-height: 100%;
}
.page-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  margin-bottom: var(--sp-1);
}
.head-text {
  min-width: 0;
}
.page-title {
  margin: 0;
  font-size: var(--fs-subtitle);
  font-weight: var(--fw-semibold);
}
.page-hint {
  margin: 2px 0 0;
  font-size: var(--fs-hint);
  color: var(--text-2);
}

/* —— 重启提示横幅：固定槽（§1.5）——隐藏时原位占位，卡片墙位置恒定 */
.banner {
  margin: 0;
  padding: var(--sp-2) var(--sp-3);
  font-size: var(--fs-hint);
  border: 1px solid var(--border);
  border-left: 3px solid var(--accent);
  border-radius: var(--r-lg);
  background: var(--surface-2);
  color: var(--text-2);
  visibility: hidden;
}
.banner.shown {
  visibility: visible;
}

/* —— 卡片墙 —— */
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--sp-4);
}
.card {
  display: flex;
  flex-direction: column;
  padding: 0;
  overflow: hidden;
  text-align: left;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  cursor: pointer;
  transition:
    border-color var(--dur-fast) var(--ease),
    box-shadow var(--dur-fast) var(--ease),
    transform var(--dur-fast) var(--ease);
}
.card:hover {
  border-color: var(--accent);
  transform: translateY(-1px);
  box-shadow: var(--shadow-1); /* 悬停高程升级（§5.2.4） */
}
.card.active {
  border-color: var(--accent);
  box-shadow: var(--ring-accent); /* 选中 ring 压过 hover 阴影（后声明优先） */
}
.pv {
  display: block;
  width: 100%;
  height: 110px;
  object-fit: cover;
  border-bottom: 1px solid var(--border);
}
.pv-fallback {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--sp-3);
}
.fake-bar {
  position: relative;
  width: 64px;
  height: 18px;
  border-radius: 0; /* GDI 条直角是真源（§2.3） */
  overflow: hidden;
  box-shadow: inset 0 0 0 1px var(--border-strong);
}
.fake-fill {
  position: absolute; /* inset 由 BAR_MOCK_FILL_INSET 内联提供（与倒计时同源） */
}
.fake-ring {
  width: 26px;
  height: 26px;
  border-radius: var(--r-full);
  border: 3px solid transparent; /* 颜色一律来自内联 mask_fg（皮肤数据），这里只定几何 */
  opacity: 0.9;
}
.meta {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3) var(--sp-4);
}
.name-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2);
}
.name {
  font-size: var(--fs-body);
  font-weight: var(--fw-semibold);
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.active-chip {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
  font-size: var(--fs-caption);
  font-weight: var(--fw-semibold);
  color: var(--accent-strong);
}
.author {
  font-size: var(--fs-hint);
  color: var(--text-2);
}
.badges {
  display: flex;
  gap: var(--sp-1);
}

/* —— 自定义颜色编辑器 —— */
.editor {
  margin-top: var(--sp-1);
}
.editor-toggle {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-body);
  color: var(--text-2);
  background: transparent;
  border: none;
  cursor: pointer;
  padding: var(--sp-2) 0; /* 垂直打击区 ≥28px（§1.6） */
  font-family: inherit;
}
.editor-toggle:hover {
  color: var(--text);
}
.chev {
  flex-shrink: 0;
  transition: transform var(--dur-slow) var(--ease);
}
.chev.open {
  transform: rotate(90deg);
}
.editor-body {
  margin-top: var(--sp-2);
  padding: var(--sp-4);
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
}
.group-label {
  margin: 0 0 var(--sp-2);
  font-size: var(--fs-hint);
  font-weight: var(--fw-semibold);
  letter-spacing: 0.5px;
  color: var(--text-2);
}
.color-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2) var(--sp-4);
  margin-bottom: var(--sp-4);
}
.editor-actions {
  display: flex;
  justify-content: flex-end; /* 保存 CTA 右下，与弹窗 foot 的主 CTA 同构 */
}
.color-item {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-hint);
  color: var(--text);
}
.color-item input[type="color"] {
  width: 28px;
  height: var(--control-h-sm); /* 28px 打击区下限（§1.6） */
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: transparent;
  cursor: pointer;
}

/* —— 预览弹窗：head + 可滚 body + 恒可见 foot（§3） —— */
.modal-mask {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgb(0 0 0 / 0.45);
}
.modal {
  display: flex;
  flex-direction: column;
  width: 660px;
  max-width: calc(100vw - var(--sp-6) * 2);
  max-height: calc(100vh - var(--sp-6) * 2);
  overflow: hidden;
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-2);
}
.modal-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-3);
  flex-shrink: 0;
  padding: var(--sp-4) var(--sp-5) var(--sp-3);
}
.modal-title {
  margin: 0;
  font-size: var(--fs-subtitle);
  font-weight: var(--fw-semibold);
}
.modal-author {
  margin: 2px 0 0;
  font-size: var(--fs-hint);
  color: var(--text-2);
}
.modal-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  padding: 0;
  border: none;
  border-radius: var(--r-sm);
  background: transparent;
  color: var(--text-2);
  cursor: pointer;
  transition:
    color var(--dur-fast) var(--ease),
    background-color var(--dur-fast) var(--ease);
}
.modal-close:hover {
  color: var(--text);
  background: var(--surface-2);
}
.modal-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto; /* 滚动只发生在 body：主 CTA 永不被滚走（§3） */
  /* 滚动条槽恒占位（§3）：LOCK_SCALE 按 610（= 内容宽 620 − 槽 10）调定，
     两态宽一致，锁屏预览永不因滚动条出现/消失被裁切 */
  scrollbar-gutter: stable;
  padding: 0 var(--sp-5) var(--sp-3);
}
.section-label {
  margin: var(--sp-3) 0 var(--sp-2);
  font-size: var(--fs-hint);
  font-weight: var(--fw-semibold);
  letter-spacing: 0.5px;
  color: var(--text-2);
}
.section-label:first-child {
  margin-top: 0;
}
.lock-wrap {
  width: 100%;
  aspect-ratio: 1280 / 800;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  /* 舞台底 = 皮肤遮罩默认底色（theme.json 真源经 applyThemeVars 注入；
     兜底值即其默认数据，非样式字面量） */
  background: var(--eyenurse-mask-bg, #141418);
}
.lock-frame {
  display: block;
  border: 0;
  transform-origin: top left;
  pointer-events: none; /* 预览惰性：不吃交互 */
}
.bar-stage {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-5) 0 var(--sp-4);
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: repeating-conic-gradient(var(--surface-2) 0% 25%, var(--surface) 0% 50%) 50% / 18px 18px;
}
.bar-frame {
  border: 0;
  flex-shrink: 0;
  pointer-events: none; /* 预览惰性：不吃交互 */
}
.bar-mock {
  position: relative;
  width: 160px;
  height: 40px;
  border-radius: 0; /* GDI 仿样直角（§2.3） */
  overflow: hidden;
  display: flex;
  align-items: center;
  box-shadow: inset 0 0 0 1px var(--border-strong);
}
.bar-mock-fill {
  position: absolute; /* inset 由 barMockFillStyle 内联提供（BAR_DEMO 推导） */
}
.bar-mock-text {
  position: relative;
  flex: 1;
  text-align: center;
  font-family: var(--font-num);
  font-size: var(--fs-subtitle);
  font-weight: var(--fw-semibold);
  font-variant-numeric: tabular-nums;
}
.mock-note {
  font-size: var(--fs-caption);
  color: var(--text-2);
}
.modal-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  flex-shrink: 0; /* footer 永不被滚走（§3） */
  padding: var(--sp-3) var(--sp-5) var(--sp-4);
  border-top: 1px solid var(--border);
}
.foot-badges {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
}
</style>
