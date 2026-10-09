<script setup lang="ts">
// 皮肤选择器（v3.1 换肤 → ESP-1 开放皮肤协议）：预设网格（目录包优先显示
// preview.png + 表面徽标，纯配色皮肤保持双迷你色块）+ 可折叠的自定义颜色编辑器
// （11 个取色器，按当前生效配色预填，保存即全量下发 set_custom_skin）。
// 生效配色/清单由父组件经 props 下发（get_skins/get_skin + skin-changed 同步）。
// bar 形态变化需重启（bar 运行模式启动期定死）：选中皮肤 has_bar 与运行模式
// 不一致时显示 skinRestartHint（换色不受影响，即时生效）。
import { computed, reactive, ref, watch } from "vue";
import { currentLanguage, t } from "../i18n";
import { skinPreviewUrl, type SkinColors, type SkinDefPayload, type SkinMeta } from "../types";

const props = defineProps<{
  /** 可用皮肤（内置 + 磁盘 + 自定义槽） */
  skins: SkinMeta[];
  /** 当前选中 id */
  currentId: string;
  /** 当前生效配色（编辑器预填源） */
  effective: SkinColors | null;
  /** bar 当前运行模式（true = WebView 壳；启动期定死） */
  barIsWeb: boolean;
}>();
const emit = defineEmits<{
  (e: "select", id: string): void;
  (e: "save-custom", def: SkinDefPayload): void;
}>();

/** 条目展示名：custom 槽本地化，其余按当前语言二选一 */
function displayName(meta: SkinMeta): string {
  if (meta.id === "custom") return t("skinCustom");
  return currentLanguage.value === "zh" ? meta.name : meta.name_en;
}

/** 表面徽标（目录包声明了哪些 HTML 表面；纯配色皮肤为空数组） */
function surfaceBadges(meta: SkinMeta): string[] {
  const badges: string[] = [];
  if (meta.has_bar) badges.push(t("skinTargetBar"));
  if (meta.has_lock) badges.push(t("skinTargetLock"));
  return badges;
}

/** 选中皮肤的 bar 形态与运行模式不一致 → 重启提示（仅对含 bar 表面的皮肤） */
const restartHint = computed(() => {
  const meta = props.skins.find((s) => s.id === props.currentId);
  if (!meta || !meta.has_bar || meta.has_bar === props.barIsWeb) return false;
  return true;
});

// —— 编辑器（展开时从生效配色预填；颜色即值，不做格式校验——非法值由后端回退） ——
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

watch(editorOpen, (open) => {
  if (open && props.effective) {
    Object.assign(draft, props.effective);
  }
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

function saveCustom(): void {
  emit("save-custom", {
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
  });
}
</script>

<template>
  <div class="picker">
    <!-- 预设网格 -->
    <div class="grid">
      <button
        v-for="meta in skins"
        :key="meta.id"
        type="button"
        class="slot"
        :class="{ active: meta.id === currentId }"
        :title="displayName(meta)"
        @click="emit('select', meta.id)"
      >
        <!-- 目录包预览图（preview.png）优先；纯配色皮肤保持双迷你色块 -->
        <img
          v-if="meta.has_preview"
          class="pv-img"
          :src="skinPreviewUrl(meta.id)"
          :alt="displayName(meta)"
          loading="lazy"
        />
        <span v-else class="preview">
          <span class="pv-bar" :style="{ background: meta.colors.bar_progress_bg }">
            <span class="pv-bar-fill" :style="{ background: meta.colors.bar_progress_fill }" />
          </span>
          <span class="pv-mask" :style="{ background: meta.colors.mask_bg }">
            <span class="pv-ring" :style="{ borderColor: meta.colors.mask_fg }" />
          </span>
        </span>
        <span class="slot-name">{{ displayName(meta) }}</span>
        <!-- ESP-1 表面徽标：该皮肤替换了哪些界面 -->
        <span v-if="surfaceBadges(meta).length" class="badges">
          <span v-for="b in surfaceBadges(meta)" :key="b" class="badge">{{ b }}</span>
        </span>
      </button>
    </div>
    <p v-if="restartHint" class="restart-hint">{{ t("skinRestartHint") }}</p>

    <!-- 自定义编辑器 -->
    <div class="editor">
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
    </div>
  </div>
</template>

<style scoped>
.grid {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.slot {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 8px;
  min-width: 76px;
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: 10px;
  cursor: pointer;
  transition: border-color 0.12s ease, box-shadow 0.12s ease;
}
.slot:hover {
  border-color: var(--accent);
}
.slot.active {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.preview {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.pv-bar {
  position: relative;
  width: 52px;
  height: 14px;
  border-radius: 3px;
  background: var(--bg);
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
}
.pv-bar-fill {
  position: absolute;
  inset: 0 40% 0 0;
  background: var(--accent);
}
.pv-mask {
  width: 52px;
  height: 14px;
  border-radius: 3px;
  background: var(--text);
  display: flex;
  align-items: center;
  justify-content: center;
}
.pv-ring {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  border: 1.5px solid #fff;
}
.slot-name {
  font-size: 11px;
  color: var(--text-2);
  max-width: 76px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.slot.active .slot-name {
  color: var(--accent-strong);
  font-weight: 600;
}
.pv-img {
  width: 76px;
  height: 44px;
  object-fit: cover;
  border-radius: 6px;
  background: var(--bg);
  box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
}
.badges {
  display: flex;
  gap: 3px;
}
.badge {
  font-size: 9px;
  line-height: 1;
  padding: 2px 5px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent-strong);
}
.restart-hint {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--text-2);
}

.editor {
  margin-top: 10px;
}
.editor-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
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
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-2);
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
</style>
