<script setup lang="ts">
// 皮肤中心（ESP-1 独立窗口）：大卡片墙 = 浏览/预览/应用皮肤的一站式界面。
// 布局自上而下：工具栏（标题 + 打开皮肤文件夹 + 设置）→ bar 重启提示横幅 →
// 卡片网格（点卡片 = 打开预览弹窗）→ 自定义颜色编辑器（折叠）。
//
// **预览弹窗**：加载真实皮肤页 iframe（eyenurse-skin 协议 + sandbox，与生产
// 同管线），握手后灌入**模拟桥状态**（ESP-1 皮肤页是 (state, texts, colors) 的
// 纯函数渲染——预览即真实效果，第三方皮肤零适配）：
// - 锁屏效果：1280×800 视口按比例缩放嵌入（纯锁屏脸：时钟 + 标题）；
// - 悬浮条效果：manifest 逻辑尺寸 ×2 放大居中；GDI 配色皮肤无 HTML 表面 →
//   CSS 1:1 仿样（与 GDI 条同构：底色 + 渐变填充 + mm:ss）。
// 预览是**惰性**的：皮肤页的动作消息（drag/setTopmost/endRest…）一律忽略。
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  currentLanguage,
  loadTexts,
  loadLanguageList,
  t,
  texts as i18nTexts,
} from "../settings/i18n";
import {
  skinPreviewUrl,
  type SkinColors,
  type SkinDefPayload,
  type SkinMeta,
  type SkinPreviewDto,
} from "../settings/types";

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
    /* 窗口早于 CoreLoop 就绪时静默，下个广播会再来 */
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
function openSettings(): void {
  invoke("open_settings").catch(() => {});
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
const LOCK_SCALE = 0.469; // 600 / 1280（弹窗内容宽固定 600px）

/** 预览用模拟显示态：锁屏 = 纯锁屏脸（时钟+标题，带密码提示位）；bar = 45min 间隔剩余 17min */
const LOCK_DEMO = { remainingMs: 95_000, resting: false, locked: true, hasPassword: true };
const BAR_DEMO = {
  countdownMs: 17 * 60 * 1000,
  intervalMs: 45 * 60 * 1000,
  pausedMs: null,
  resting: false,
  topmost: false,
};

async function openPreview(meta: SkinMeta): Promise<void> {
  previewMeta.value = meta;
  previewDto.value = null;
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

/** GDI 配色皮肤的 bar 仿样（与原生条同构：底色 + 三段渐变填充 62% + mm:ss） */
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
  };
});

// —— 自定义颜色编辑器（自设置页迁入；展开时从生效配色预填）——
const editorOpen = ref(false);
const draft = reactive({
  bar_progress_bg: "#F5F5DC",
  bar_progress_fill: "#3AC32E",
  bar_progress_from: "#ECC071",
  bar_progress_to: "#3E0D0D",
  bar_text_idle: "#000000",
  bar_text_from: "#080808",
  bar_text_to: "#F40000",
  bar_check_glyph: "#444444",
  bar_check_hover_bg: "#FFFFFF",
  mask_bg: "#141418",
  mask_fg: "#EEEEF2",
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
        mask: { bg: draft.mask_bg, fg: draft.mask_fg },
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
  await loadTexts();
  void loadLanguageList();
  await refresh();
  await listen("eyenurse://skin-changed", () => void refresh());
  await listen("eyenurse://skin-surface-changed", () => void refresh());
  await listen("eyenurse://i18n-changed", () => void loadTexts());
});
onUnmounted(() => {
  window.removeEventListener("message", onPreviewMessage);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div class="center">
    <!-- 工具栏：标题 + 安装入口 + 返回设置 -->
    <header class="toolbar">
      <div>
        <h2 class="title">{{ t("skin_center") }}</h2>
        <p class="subtitle">{{ t("skinsFolderHint") }}</p>
      </div>
      <div class="actions">
        <button type="button" class="ghost-btn" @click="openSkinsFolder">
          {{ t("openSkinsFolder") }}
        </button>
        <button type="button" class="ghost-btn" @click="openSettings">
          {{ t("settings") }}
        </button>
      </div>
    </header>

    <!-- bar 形态变化需要重启（换色即时生效） -->
    <p v-if="restartHint" class="banner">{{ t("skinRestartHint") }}</p>

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
            <span class="fake-fill" :style="{ background: meta.colors.bar_progress_fill }" />
          </span>
          <span class="fake-ring" :style="{ borderColor: meta.colors.mask_fg }" />
        </span>
        <span class="meta">
          <span class="name-row">
            <span class="name">{{ displayName(meta) }}</span>
            <span v-if="meta.id === currentId" class="active-chip">✓ {{ t("skinActive") }}</span>
          </span>
          <span v-if="authorLine(meta)" class="author">{{ authorLine(meta) }}</span>
          <span v-if="badges(meta).length" class="badges">
            <span v-for="b in badges(meta)" :key="b" class="badge">{{ b }}</span>
          </span>
        </span>
      </button>
    </main>

    <!-- 自定义颜色编辑器（折叠） -->
    <section class="editor">
      <button type="button" class="editor-toggle" @click="editorOpen = !editorOpen">
        <span class="tri" :class="{ open: editorOpen }">▸</span>
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
        <button
          type="button"
          class="apply"
          :disabled="!draftValid"
          :title="draftValid ? '' : t('colorInvalid')"
          @click="saveCustom"
        >
          {{ t("skinSave") }}
        </button>
      </div>
    </section>

    <!-- 预览弹窗：真实皮肤页 iframe + 模拟桥状态（ESC / 点遮罩关闭） -->
    <div v-if="previewMeta" class="modal-mask" @click.self="closePreview">
      <div class="modal">
        <header class="modal-head">
          <div class="modal-title-wrap">
            <h3 class="modal-title">{{ displayName(previewMeta) }}</h3>
            <p v-if="authorLine(previewMeta)" class="modal-author">{{ authorLine(previewMeta) }}</p>
          </div>
          <button type="button" class="modal-close" :title="t('skinApply')" @click="closePreview">✕</button>
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
                <span class="bar-mock-text">32:15</span>
              </div>
              <span class="mock-note">{{ t("skinNativeBar") }}</span>
            </template>
          </div>
        </div>

        <footer class="modal-foot">
          <span class="foot-badges">
            <span v-for="b in badges(previewMeta)" :key="b" class="badge">{{ b }}</span>
          </span>
          <button type="button" class="apply-btn" @click="applyPreview">
            {{ t("skinApply") }}
          </button>
        </footer>
      </div>
    </div>
  </div>
</template>

<style scoped>
.center {
  min-height: 100vh;
  background: var(--bg);
  padding: 18px 24px 26px;
}
.toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}
.title {
  margin: 0;
  font-size: 18px;
}
.subtitle {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--text-2);
}
.actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}
.ghost-btn {
  font-size: 12px;
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  color: var(--text);
  cursor: pointer;
  white-space: nowrap;
}
.ghost-btn:hover {
  border-color: var(--accent);
  color: var(--accent-strong);
}
.banner {
  margin: 0 0 12px;
  padding: 8px 12px;
  font-size: 12px;
  border: 1px solid var(--border);
  border-left: 3px solid var(--accent);
  border-radius: 8px;
  background: var(--surface-2);
  color: var(--text-2);
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 14px;
}
.card {
  display: flex;
  flex-direction: column;
  padding: 0;
  overflow: hidden;
  text-align: left;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  cursor: pointer;
  transition: border-color 0.12s ease, box-shadow 0.12s ease, transform 0.12s ease;
}
.card:hover {
  border-color: var(--accent);
  transform: translateY(-1px);
}
.card.active {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
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
  gap: 12px;
}
.fake-bar {
  position: relative;
  width: 64px;
  height: 18px;
  border-radius: 4px;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.18);
}
.fake-fill {
  position: absolute;
  inset: 0 38% 0 0;
}
.fake-ring {
  width: 26px;
  height: 26px;
  border-radius: 999px;
  border: 3px solid #fff;
  opacity: 0.9;
}
.meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px 12px;
}
.name-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.active-chip {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 700;
  color: var(--accent-strong);
}
.author {
  font-size: 11px;
  color: var(--text-2);
}
.badges {
  display: flex;
  gap: 4px;
  margin-top: 2px;
}
.badge {
  font-size: 10px;
  line-height: 1;
  padding: 3px 7px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent-strong);
}
.editor {
  margin-top: 18px;
}
.editor-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--text-2);
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 2px 0;
}
.editor-toggle:hover {
  color: var(--text);
}
.tri {
  display: inline-block;
  transition: transform 0.15s ease;
}
.tri.open {
  transform: rotate(90deg);
}
.editor-body {
  margin-top: 8px;
  padding: 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
}
.group-label {
  margin: 0 0 6px;
  font-size: 11px;
  letter-spacing: 1px;
  color: var(--text-2);
}
.color-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 16px;
  margin-bottom: 10px;
}
.color-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text);
}
.color-item input[type="color"] {
  width: 28px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
}
.apply {
  font-size: 12px;
  font-weight: 600;
  padding: 6px 14px;
  border: none;
  border-radius: 8px;
  background: var(--accent);
  color: #fff;
  cursor: pointer;
}
.apply:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.apply:hover:not(:disabled) {
  background: var(--accent-strong);
}

/* —— 预览弹窗 —— */
.modal-mask {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgb(0 0 0 / 0.45);
}
.modal {
  display: flex;
  flex-direction: column;
  width: 660px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  overflow-y: auto;
  border-radius: 14px;
  background: var(--surface);
  box-shadow: 0 12px 40px rgb(0 0 0 / 0.3);
}
.modal-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: 16px 20px 10px;
}
.modal-title {
  margin: 0;
  font-size: 16px;
}
.modal-author {
  margin: 3px 0 0;
  font-size: 11px;
  color: var(--text-2);
}
.modal-close {
  border: none;
  background: transparent;
  color: var(--text-2);
  font-size: 15px;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 6px;
}
.modal-close:hover {
  color: var(--text);
  background: var(--surface-2);
}
.modal-body {
  padding: 0 20px;
}
.section-label {
  margin: 10px 0 6px;
  font-size: 11px;
  letter-spacing: 1px;
  color: var(--text-2);
}
.lock-wrap {
  width: 600px;
  max-width: 100%;
  aspect-ratio: 1280 / 800;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: #000;
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
  gap: 6px;
  padding: 22px 0 18px;
  border: 1px solid var(--border);
  border-radius: 10px;
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
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  align-items: center;
  box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.15);
}
.bar-mock-fill {
  position: absolute;
  inset: 0 38% 0 0;
}
.bar-mock-text {
  position: relative;
  flex: 1;
  text-align: center;
  font-family: Consolas, monospace;
  font-size: 15px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.mock-note {
  font-size: 10px;
  color: var(--text-2);
}
.modal-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px 16px;
}
.foot-badges {
  display: flex;
  gap: 4px;
}
.apply-btn {
  font-size: 13px;
  font-weight: 600;
  padding: 8px 22px;
  border: none;
  border-radius: 8px;
  background: var(--accent);
  color: #fff;
  cursor: pointer;
}
.apply-btn:hover {
  background: var(--accent-strong);
}
</style>
