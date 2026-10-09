<script setup lang="ts">
// 设置页主区（v3.1 重设计）：实时倒计时 Hero + 两张分组卡片 + 底部操作栏。
// 提交时机（清单 I3）：文本框失焦/回车、开关点击、预设/恢复点击 → 父组件 300ms
// 防抖保存；parse + clamp 在命令侧（清单 I4），页面只做展示与占位校验。
import { computed, ref } from "vue";
import type { LockSettingsDto, SettingsDto, SkinColors, SkinDefPayload, SkinMeta } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";
import StatsCard from "./StatsCard.vue";
import SkinPicker from "./SkinPicker.vue";

const props = defineProps<{
  /** 父组件持有的响应式草稿（直接绑定字段，保存回包整体合并） */
  settings: SettingsDto;
  /** invoke 未决期 = 保存中 */
  saving: boolean;
  /** 页眉实时状态（eyenurse://tick；首拍到达前为 null → 按满进度展示） */
  tick: { remaining_ms: number; paused: boolean; resting: boolean } | null;
  /** 锁屏草稿（v3.2；独立保存管线，回包整体合并） */
  lockSettings: LockSettingsDto;
  /** 可用皮肤清单（内置 + 磁盘 + 自定义槽；v3.1 换肤） */
  skins: SkinMeta[];
  /** 当前选中皮肤 id */
  currentSkinId: string;
  /** 当前生效配色 */
  effectiveSkin: SkinColors | null;
  /** ESP-1：bar 当前运行模式（true = WebView 壳；bar 形态变化需重启） */
  barIsWeb: boolean;
}>();
const emit = defineEmits<{
  /** 任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
  /** 锁屏设置任一控件提交 → 父组件排 300ms 防抖保存（v3.2） */
  (e: "lock-commit"): void;
  /** 恢复默认（F11），两步确认后触发 */
  (e: "reset"): void;
  /** 立刻休息（Hero 按钮） */
  (e: "rest-now"): void;
  /** 选择皮肤（v3.1 换肤） */
  (e: "select-skin", id: string): void;
  /** 保存自定义配色并启用 */
  (e: "save-custom-skin", def: SkinDefPayload): void;
}>();

/** 休息间隔预设（分钟） */
const PRESETS = [20, 30, 45, 60, 90];

/** HH:mm:ss（.NET "c" 紧凑格式）宽松校验：仅控制输入态红框，解析在命令侧 */
function hmsValid(s: string): boolean {
  return /^(\d{1,2}):([0-5]\d):([0-5]\d)$/.test(s.trim());
}

/** HH:mm:ss → 秒（非法返回 NaN，调用方自行兜底） */
function hmsSecs(s: string): number {
  const m = /^(\d{1,2}):([0-5]\d):([0-5]\d)$/.exec(s.trim());
  if (!m) return NaN;
  return Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]);
}

/** 秒 → 倒计时文本：>1h 显示 H:MM:SS，否则 MM:SS */
function fmtCountdown(totalSecs: number): string {
  const s = Math.max(0, Math.floor(totalSecs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = s % 60;
  const two = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${two(m)}:${two(ss)}` : `${two(m)}:${two(ss)}`;
}

/** HH:mm:ss → 人话时长：45 分钟 / 2 分钟 30 秒；含小时或非法 → 原样回显 */
function friendly(s: string): string {
  const total = hmsSecs(s);
  if (!Number.isFinite(total)) return s;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const ss = total % 60;
  if (h > 0) return s;
  if (m > 0 && ss > 0) return `${m} ${t("minutes")} ${ss} ${t("seconds")}`;
  if (m > 0) return `${m} ${t("minutes")}`;
  return `${ss} ${t("seconds")}`;
}

const intervalSecs = computed(() => hmsSecs(props.settings.RestInterval) || 45 * 60);

/** 预设命中（当前间隔恰好等于某预设） */
const activePreset = computed(() => {
  const total = hmsSecs(props.settings.RestInterval);
  return PRESETS.find((m) => m * 60 === total);
});

function setPreset(mins: number): void {
  props.settings.RestInterval = `00:${String(mins).padStart(2, "0")}:00`;
  emit("commit");
}

/** 作息摘要：休息间隔 45 分钟 · 休息时长 2 分钟 30 秒 */
const scheduleSummary = computed(
  () =>
    `${t("restInterval")} ${friendly(props.settings.RestInterval)} · ${t(
      "restDuration",
    )} ${friendly(props.settings.RestDuration)}`,
);

// —— Hero 实时区 ——
const remainingMs = computed(() => props.tick?.remaining_ms ?? intervalSecs.value * 1000);
const pausedState = computed(() => props.tick?.paused ?? false);
const restingState = computed(() => props.tick?.resting ?? false);
const countdownText = computed(() => fmtCountdown(remainingMs.value / 1000));
const progressPct = computed(() => {
  const total = intervalSecs.value;
  if (total <= 0) return 100;
  return Math.min(100, Math.max(0, (remainingMs.value / 1000 / total) * 100));
});

// —— 恢复默认：两步确认（3s 内二次点击生效），替代 confirm() 弹窗 ——
const confirmReset = ref(false);
let confirmTimer: ReturnType<typeof setTimeout> | null = null;
function onReset(): void {
  if (!confirmReset.value) {
    confirmReset.value = true;
    if (confirmTimer !== null) clearTimeout(confirmTimer);
    confirmTimer = setTimeout(() => (confirmReset.value = false), 3000);
    return;
  }
  if (confirmTimer !== null) clearTimeout(confirmTimer);
  confirmReset.value = false;
  emit("reset");
}
</script>

<template>
  <div class="page">
    <!-- Hero：实时倒计时 + 立刻休息 -->
    <section class="hero">
      <div class="hero-info">
        <p class="hero-label">
          {{ restingState ? t("resting") : pausedState ? t("paused") : t("nextRest") }}
        </p>
        <p class="hero-count">{{ countdownText }}</p>
        <p class="hero-sub">{{ scheduleSummary }}</p>
      </div>
      <button type="button" class="rest-btn" @click="$emit('rest-now')">
        {{ t("rest_now") }}
      </button>
      <div class="hero-track" aria-hidden="true">
        <span class="hero-fill" :style="{ width: progressPct + '%' }" />
      </div>
    </section>

    <!-- 休息计划 -->
    <section class="card">
      <h3 class="card-title">{{ t("planTitle") }}</h3>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("restInterval") }}</p>
          <div class="chips">
            <button
              v-for="m in PRESETS"
              :key="m"
              type="button"
              class="chip"
              :class="{ active: activePreset === m }"
              @click="setPreset(m)"
            >
              {{ m }} {{ t("minutes") }}
            </button>
          </div>
        </div>
        <input
          v-model="settings.RestInterval"
          type="text"
          class="hms"
          :class="{ invalid: !hmsValid(settings.RestInterval) }"
          placeholder="HH:mm:ss"
          spellcheck="false"
          @blur="$emit('commit')"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("restDuration") }}</p>
        </div>
        <input
          v-model="settings.RestDuration"
          type="text"
          class="hms"
          :class="{ invalid: !hmsValid(settings.RestDuration) }"
          placeholder="HH:mm:ss"
          spellcheck="false"
          @blur="$emit('commit')"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>
    </section>

    <!-- 系统 -->
    <section class="card">
      <h3 class="card-title">{{ t("systemTitle") }}</h3>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("runWhenStarts") }}</p>
        </div>
        <SwitchToggle v-model="settings.RunWhenStarts" @update:model-value="$emit('commit')" />
      </div>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("resetWhenSessionUnlock") }}</p>
        </div>
        <div class="row-control">
          <input
            v-if="settings.ResetWhenSessionUnlock"
            v-model="settings.ResetTimeout"
            type="text"
            class="hms"
            :class="{ invalid: !hmsValid(settings.ResetTimeout) }"
            placeholder="HH:mm:ss"
            spellcheck="false"
            @blur="$emit('commit')"
            @keydown.enter="($event.target as HTMLInputElement).blur()"
          />
          <SwitchToggle
            v-model="settings.ResetWhenSessionUnlock"
            @update:model-value="$emit('commit')"
          />
        </div>
      </div>
    </section>

    <!-- 锁屏（v3.2 仿锁屏） -->
    <section class="card">
      <h3 class="card-title">{{ t("lockTitle") }}</h3>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("lockEnable") }}</p>
          <p class="row-hint">{{ t("lockEnableHint") }}</p>
        </div>
        <SwitchToggle v-model="lockSettings.Enabled" @update:model-value="$emit('lock-commit')" />
      </div>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("lockWinL") }}</p>
          <p class="row-hint">{{ t("lockWinLHint") }}</p>
        </div>
        <SwitchToggle
          v-model="lockSettings.InterceptWinL"
          @update:model-value="$emit('lock-commit')"
        />
      </div>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("lockCapture") }}</p>
          <p class="row-hint">{{ t("lockCaptureHint") }}</p>
        </div>
        <SwitchToggle
          v-model="lockSettings.CaptureThrough"
          @update:model-value="$emit('lock-commit')"
        />
      </div>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("lockAutoRest") }}</p>
          <p class="row-hint">{{ t("lockAutoRestHint") }}</p>
        </div>
        <SwitchToggle v-model="lockSettings.AutoRest" @update:model-value="$emit('lock-commit')" />
      </div>

      <div class="row">
        <div class="row-main">
          <p class="row-title">{{ t("lockPassword") }}</p>
          <p class="row-hint">{{ t("lockPasswordHint") }}</p>
        </div>
        <input
          v-model="lockSettings.Password"
          type="password"
          class="hms pwd"
          :placeholder="t('lockPasswordHint')"
          autocomplete="new-password"
          spellcheck="false"
          @blur="$emit('lock-commit')"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>
    </section>

    <!-- 护眼统计（v3.2：今日摘要 + 近 7 天报表） -->
    <StatsCard />

    <!-- 外观（v3.1 换肤：预设网格 + 自定义颜色编辑器） -->
    <section class="card">
      <h3 class="card-title">{{ t("appearance") }}</h3>
      <SkinPicker
        :skins="skins"
        :current-id="currentSkinId"
        :effective="effectiveSkin"
        :bar-is-web="barIsWeb"
        @select="$emit('select-skin', $event)"
        @save-custom="$emit('save-custom-skin', $event)"
      />
    </section>

    <!-- 底部：恢复默认（两步确认） + 版权 -->
    <footer class="page-foot">
      <button type="button" class="ghost" :class="{ danger: confirmReset }" @click="onReset">
        {{ confirmReset ? t("confirmRestore") : t("restoreDefaults") }}
      </button>
      <span class="copyright" v-html="t('copyright')"></span>
    </footer>
  </div>
</template>

<style scoped>
.page {
  max-width: 640px;
  margin: 0 auto;
  padding: 20px 20px 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 100%;
}

/* —— Hero —— */
.hero {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 22px 24px 26px;
  border-radius: 14px;
  background: linear-gradient(135deg, var(--hero-from), var(--hero-to));
  color: var(--hero-text);
  overflow: hidden;
}
.hero-label {
  margin: 0 0 2px;
  font-size: 12px;
  letter-spacing: 1px;
  color: var(--hero-dim);
}
.hero-count {
  margin: 0;
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 40px;
  line-height: 1.1;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.hero-sub {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--hero-dim);
}
.rest-btn {
  flex-shrink: 0;
  padding: 9px 18px;
  font-size: 13px;
  font-weight: 600;
  color: var(--hero-from);
  background: var(--hero-text);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  transition: transform 0.12s ease, box-shadow 0.12s ease;
}
.rest-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 14px rgb(0 0 0 / 0.2);
}
.rest-btn:active {
  transform: translateY(0);
}
.hero-track {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 4px;
  background: rgb(255 255 255 / 0.18);
}
.hero-fill {
  display: block;
  height: 100%;
  background: rgb(255 255 255 / 0.85);
  transition: width 1s linear;
}

/* —— 卡片与行 —— */
.card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 4px 18px;
}
.card-title {
  margin: 12px 0 4px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--text-2);
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 13px 0;
  border-top: 1px solid var(--border);
}
.row:first-of-type {
  border-top: none;
}
.row-main {
  min-width: 0;
}
.row-title {
  margin: 0;
  font-size: 13px;
  font-weight: 500;
}
.row-hint {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--text-2);
}
.row-control {
  display: flex;
  align-items: center;
  gap: 10px;
}

/* —— 预设 chips —— */
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 8px;
}
.chip {
  font-size: 12px;
  padding: 3px 10px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text-2);
  cursor: pointer;
  transition: all 0.12s ease;
}
.chip:hover {
  border-color: var(--accent);
  color: var(--text);
}
.chip.active {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--accent-strong);
  font-weight: 600;
}

/* —— 时间输入 —— */
.hms {
  width: 104px;
  text-align: center;
  padding: 7px 10px;
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: var(--text);
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: 8px;
  outline: none;
  transition: border-color 0.12s ease, box-shadow 0.12s ease;
}
.hms:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.hms.invalid {
  border-color: var(--danger);
  box-shadow: 0 0 0 3px var(--danger-soft);
}
/* 锁屏密码框（v3.2）：同 hms 尺寸语言，加宽放下提示 */
.hms.pwd {
  width: 180px;
  text-align: left;
}

/* —— 底部 —— */
.page-foot {
  margin-top: auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 4px;
}
.ghost {
  font-size: 12px;
  padding: 6px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-2);
  cursor: pointer;
  transition: all 0.12s ease;
}
.ghost:hover {
  color: var(--text);
  border-color: var(--text-2);
}
.ghost.danger {
  color: var(--danger);
  border-color: var(--danger);
  background: var(--danger-soft);
  font-weight: 600;
}
.copyright {
  font-size: 11px;
  color: var(--text-2);
}
</style>
