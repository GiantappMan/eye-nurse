<script setup lang="ts">
// 休息模块（方案 C 侧边栏布局）：实时倒计时 Hero + 休息计划卡。
// Hero 进度（2026-10-10 用户拍板）：离开卡片边缘的 4px 独立进度条 +
// 「本次工作周期 32 / 45 分钟」文案，替代旧贴边细线（无语义）。
// 提交时机（清单 I3）：失焦/回车/chip 点击 → 父组件 300ms 防抖保存；
// parse + clamp 在命令侧（清单 I4），页面只做展示与占位校验。
import { computed, reactive } from "vue";
import type { SettingsDto } from "../types";
import { t } from "../i18n";

const props = defineProps<{
  /** 父组件持有的响应式草稿（直接绑定字段，保存回包整体合并） */
  settings: SettingsDto;
  /** 页眉实时状态（eyenurse://tick；首拍到达前为 null → 按满进度展示） */
  tick: { remaining_ms: number; paused: boolean; resting: boolean } | null;
}>();
const emit = defineEmits<{
  /** 任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
  /** 立刻休息（Hero 按钮） */
  (e: "rest-now"): void;
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

/** 周期进度文案（模块②）：已工作 / 周期总长，分钟取整（demo 32 / 45 同款） */
const elapsedMin = computed(() =>
  Math.max(0, Math.round((intervalSecs.value - remainingMs.value / 1000) / 60)),
);
const totalMin = computed(() => Math.max(1, Math.round(intervalSecs.value / 60)));

// —— 状态完备（DS3 §4.2）：用户触碰过的时间输入字段（失焦记入）；
// invalid 红态只对触碰过的字段显示，防首帧草稿空串出现假错误 ——
const touched = reactive(new Set<string>());

/** 时间输入失焦：记触碰 + 沿用「失焦即提交」（清单 I3）；回车经 blur 走同一入口 */
function onHmsBlur(field: string): void {
  touched.add(field);
  emit("commit");
}
</script>

<template>
  <div class="page">
    <!-- Hero：实时倒计时 + 立刻休息 + 显性周期进度（模块②） -->
    <section class="hero">
      <div class="hero-info">
        <p class="hero-label">
          {{ restingState ? t("resting") : pausedState ? t("paused") : t("nextRest") }}
        </p>
        <p class="hero-count">{{ countdownText }}</p>
        <p class="hero-sub">{{ scheduleSummary }}</p>
      </div>
      <button type="button" class="en-btn en-btn-onhero" @click="$emit('rest-now')">
        {{ t("rest_now") }}
      </button>
      <div class="hp" aria-hidden="true">
        <div class="hp-track"><span class="hp-fill" :style="{ width: progressPct + '%' }" /></div>
        <div class="hp-meta">
          <span>{{ t("workCycle") }}</span>
          <span class="num">{{ elapsedMin }} / {{ totalMin }} {{ t("minutes") }}</span>
        </div>
      </div>
    </section>

    <!-- 休息计划 -->
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

/* —— Hero（§5.1.5：padding 20/24/24；圆角收敛 --r-lg）—— */
.hero {
  position: relative;
  display: flex;
  flex-wrap: wrap; /* 进度条（.hp flex-basis:100%）换行到信息行下方，独占整卡宽 */
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2) var(--sp-4);
  padding: var(--sp-5) var(--sp-6) var(--sp-4);
  border-radius: var(--r-lg);
  background: linear-gradient(135deg, var(--hero-from), var(--hero-to));
  color: var(--hero-text);
  overflow: hidden;
}
.hero-info {
  min-width: 0;
}
.hero-label {
  margin: 0 0 2px;
  font-size: var(--fs-hint);
  letter-spacing: 0.5px;
  color: var(--hero-dim);
}
.hero-count {
  margin: 0;
  font-family: var(--font-num);
  font-size: var(--fs-display);
  line-height: 1.1;
  font-weight: var(--fw-semibold);
  font-variant-numeric: tabular-nums;
  /* 变宽数值槽（§3）：min-width 按最宽形态 H:MM:SS = 7 个等宽字符，
     MM:SS↔H:MM:SS 跨格式切换右缘向外生长，不动左侧与右侧按钮 */
  min-width: 7ch;
}
.hero-sub {
  margin: var(--sp-1) 0 0;
  font-size: var(--fs-hint);
  color: var(--hero-dim);
}
/* 渐变深底上绿色 outline 不可读：焦点描边换 Hero 白（token --hero-text） */
.hero .en-btn:focus-visible {
  outline-color: var(--hero-text);
}

/* —— 模块②：显性周期进度（离开边缘的独立进度条 + 文案，DS3 demo 拍板款）—— */
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
  gap: var(--sp-3);
  margin-top: 6px;
  font-size: var(--fs-caption);
  color: var(--hero-dim);
}
.hp-meta .num {
  font-family: var(--font-num);
  font-variant-numeric: tabular-nums;
  color: var(--hero-text);
}

/* —— 卡片与行：骨架走共享 .en-card/.en-card-title/.en-row（design.css L4）—— */
.row-main {
  min-width: 0;
}

/* —— 预设 chips：共享 .en-chip/.is-active —— */
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
  margin-top: var(--sp-2);
}

/* —— 时间输入：共享 .en-input/.en-input-mono/.is-invalid，这里只管宽度 —— */
.hms {
  width: 104px; /* HH:mm:ss 定宽（§4.2） */
}
/* 焦点语言归一：ring 已由 .en-input:focus 提供，键盘焦点不再叠全局 outline */
.hms:focus-visible {
  outline: none;
}
</style>
