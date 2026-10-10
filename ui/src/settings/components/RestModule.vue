<script setup lang="ts">
// 休息模块（功能页）：实时倒计时 Hero + 立刻休息 + 周期进度。
// 2026-10-10 IA 调整：休息计划（间隔/时长）等配置整体迁入「设置」页，本页
// 只保留状态与动作——打开即看剩余时间，要休息点一下。
// 实时状态来自 eyenurse://tick（core.rs 每拍下发）。
import { computed } from "vue";
import type { SettingsDto } from "../types";
import { t } from "../i18n";

const props = defineProps<{
  /** 主设置草稿（本页只读 RestInterval 用于进度归一化） */
  settings: SettingsDto;
  /** 实时状态（首拍到达前为 null → 按满进度展示） */
  tick: { remaining_ms: number; paused: boolean; resting: boolean } | null;
}>();
const emit = defineEmits<{
  /** 立刻休息（Hero 按钮） */
  (e: "rest-now"): void;
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
</style>
