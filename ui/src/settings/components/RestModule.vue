<script setup lang="ts">
// 休息模块（功能页）：实时倒计时 Hero + 立刻休息 + 周期进度 + 休息计划卡。
// 2026-10-10 IA 二次调整：休息计划（间隔/时长）从设置子页迁回本页——作息
// 就该在休息页直接改，Hero 摘要随草稿即时变化即是保存反馈（修改自动保存）。
// 实时状态来自 eyenurse://tick（core.rs 每拍下发）。
import { computed, reactive } from "vue";
import type { SettingsDto } from "../types";
import { t } from "../i18n";

const props = defineProps<{
  /** 主设置草稿（间隔/时长在本页编辑，实时驱动 Hero 进度归一化） */
  settings: SettingsDto;
  /** 实时状态（首拍到达前为 null → 按满进度展示） */
  tick: { remaining_ms: number; paused: boolean; resting: boolean } | null;
}>();
const emit = defineEmits<{
  /** 立刻休息（Hero 按钮） */
  (e: "rest-now"): void;
  /** 休息计划任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
}>();

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

/** 周期进度文案：已工作 / 周期总长，分钟取整 */
const elapsedMin = computed(() =>
  Math.max(0, Math.round((intervalSecs.value - remainingMs.value / 1000) / 60)),
);
const totalMin = computed(() => Math.max(1, Math.round(intervalSecs.value / 60)));

// —— 休息计划卡（自设置页迁回）——
/** 休息间隔预设（分钟） */
const PRESETS = [20, 30, 45, 60, 90];

/** HH:mm:ss 宽松校验：仅控制输入态红框，解析在命令侧（清单 I4） */
function hmsValid(s: string): boolean {
  return /^(\d{1,2}):([0-5]\d):([0-5]\d)$/.test(s.trim());
}

/** 预设命中（当前间隔恰好等于某预设） */
const activePreset = computed(() => {
  const m = /^(\d{1,2}):([0-5]\d):([0-5]\d)$/.exec(props.settings.RestInterval.trim());
  if (!m) return undefined;
  return PRESETS.find((mins) => mins * 60 === Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]));
});

function setPreset(mins: number): void {
  props.settings.RestInterval = `00:${String(mins).padStart(2, "0")}:00`;
  emit("commit");
}

// —— 状态完备（DS3 §4.2）：invalid 红态只在用户触碰（失焦）后出现 ——
const touched = reactive(new Set<string>());

function onHmsBlur(field: string): void {
  touched.add(field);
  emit("commit");
}
</script>

<template>
  <div class="page">
    <section class="hero">
      <div class="hero-top">
        <div>
          <div class="hero-label">
            {{ restingState ? t("resting") : pausedState ? t("paused") : t("nextRest") }}
          </div>
          <div class="hero-time">{{ countdownText }}</div>
          <div class="hero-sub">{{ scheduleSummary }}</div>
        </div>
        <button type="button" class="en-btn en-btn-onhero" @click="$emit('rest-now')">
          {{ t("rest_now") }}
        </button>
      </div>
      <div class="hp">
        <div class="hp-track"><span class="hp-fill" :style="{ width: progressPct + '%' }" /></div>
        <div class="hp-meta">
          <span>{{ t("workCycle") }}</span>
          <span class="num">{{ elapsedMin }} / {{ totalMin }} {{ t("minutes") }}</span>
        </div>
      </div>
    </section>

    <section class="en-card">
      <h3 class="en-card-title">{{ t("planTitle") }}</h3>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("restInterval") }}</p>
          <div class="chips">
            <button
              v-for="m in PRESETS"
              :key="m"
              type="button"
              class="en-chip"
              :class="{ 'is-active': activePreset === m }"
              @click="setPreset(m)"
            >
              {{ m }} {{ t("minutes") }}
            </button>
          </div>
        </div>
        <input
          v-model="settings.RestInterval"
          type="text"
          class="en-input en-input-mono hms"
          :class="{ 'is-invalid': touched.has('RestInterval') && !hmsValid(settings.RestInterval) }"
          placeholder="HH:mm:ss"
          spellcheck="false"
          @blur="onHmsBlur('RestInterval')"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("restDuration") }}</p>
        </div>
        <input
          v-model="settings.RestDuration"
          type="text"
          class="en-input en-input-mono hms"
          :class="{ 'is-invalid': touched.has('RestDuration') && !hmsValid(settings.RestDuration) }"
          placeholder="HH:mm:ss"
          spellcheck="false"
          @blur="onHmsBlur('RestDuration')"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>
    </section>
  </div>
</template>

<style scoped>
.page {
  max-width: 640px;
  margin: 0 auto;
  padding: var(--sp-5) var(--sp-5) var(--sp-4);
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  min-height: 100%;
}

/* —— Hero（渐变绿 + 白色主按钮）—— */
.hero {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2) var(--sp-4);
  padding: var(--sp-5) var(--sp-6) var(--sp-4);
  border-radius: var(--r-lg);
  background: linear-gradient(135deg, var(--hero-from), var(--hero-to));
  color: var(--hero-text);
  overflow: hidden;
}
.hero-info,
.hero-top > div {
  min-width: 0;
}
.hero-label {
  margin: 0 0 2px;
  font-size: var(--fs-hint);
  letter-spacing: 0.5px;
  color: var(--hero-dim);
}
.hero-time {
  margin: 0;
  font-family: var(--font-num);
  font-size: var(--fs-display);
  line-height: 1.1;
  font-weight: var(--fw-semibold);
  font-variant-numeric: tabular-nums;
  min-width: 7ch; /* 最宽形态 H:MM:SS，跨格式切换右缘生长（§3） */
}
.hero-sub {
  margin: var(--sp-1) 0 0;
  font-size: var(--fs-hint);
  color: var(--hero-dim);
}
.hero .en-btn:focus-visible {
  outline-color: var(--hero-text);
}

/* —— 显性周期进度（离开边缘的独立进度条 + 文案）—— */
.hp {
  flex-basis: 100%;
  margin-top: var(--sp-2);
}
.hp-track {
  height: var(--sp-1);
  border-radius: var(--r-full);
  background: rgb(255 255 255 / 0.22);
  overflow: hidden;
}
.hp-fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: rgb(255 255 255 / 0.92);
  transition: width var(--dur-slow) var(--ease);
}
.hp-meta {
  display: flex;
  justify-content: space-between;
  margin-top: 6px;
  font-size: var(--fs-caption);
  color: var(--hero-dim);
}
.hp-meta .num {
  font-family: var(--font-num);
  color: var(--hero-text);
}

/* —— 休息计划卡 —— */
.row-main {
  min-width: 0;
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
  margin-top: var(--sp-2);
}
.hms {
  width: 104px; /* HH:mm:ss 定宽（§4.2） */
}
.hms:focus-visible {
  outline: none; /* ring 已由 .en-input:focus 提供 */
}
</style>
