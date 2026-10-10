<script setup lang="ts">
// 护眼统计卡片（v3.2 → 2026-10-10 视觉验收改进）：今日三桶摘要 + 近 7 天堆叠条形报表。
// 数据自取（get_stats 近 30 天，展示取最近 7 天）：挂载 + 30s 轮询 +
// settings-shown（窗口被托盘重新唤起）时刷新；口径见 crates/eyenurse-core/src/stats.rs。
// 图表规范（DS3 §4.6 + demo 模块③拍板款）：
// - 轨道 = --surface-2 底 + 1px 描边，工作段 --chart-work（对轨道 ≥3.4:1），
//   休息段 --chart-rest；
// - 图例常驻（工作/休息），"灰条绿段"不再需要猜；
// - 无数据日整行弱化：轨道虚化 + 短横占位 + 日期降档——不再渲染完整空胶囊
//   （看着像坏了而不是零）；
// - 休息次数列带语义（"休息 ×17"），有数据的行悬停浮出当日明细 tooltip；
// - 文案去重：今日摘要「休息 · 0」→「休息（0 次）」（数字在上方，标签不再重复 0）。
// 防异步首帧跳动（§1.5-2）：首拍落定（成功或失败）前渲染与数据态同几何的
// 骨架占位，get_stats 回包后原位填充，卡片高度一次成形。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { StatsDay } from "../types";

const REFRESH_MS = 30_000;
const SHOW_DAYS = 7;

const days = ref<StatsDay[]>([]);
/** 首拍是否已落定（成功或失败都算）：落定前渲染骨架占位 */
const loaded = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

async function refresh(): Promise<void> {
  try {
    days.value = await invoke<StatsDay[]>("get_stats");
  } catch (e) {
    console.error("get_stats 失败", e);
  } finally {
    loaded.value = true;
  }
}

/** 最近 N 天（旧→新；后端返回 30 天，截尾展示） */
const recent = computed(() => days.value.slice(-SHOW_DAYS));

/** 今日 = 报表末位（后端保证含今天、缺失天补零） */
const today = computed<StatsDay | null>(() => recent.value.at(-1) ?? null);

/** 条形归一化基准：近 7 天中最大的 工作+休息 秒数（至少 1 防除零） */
const maxTotal = computed(() =>
  Math.max(1, ...recent.value.map((d) => d.work_secs + d.rest_secs)),
);

/** 单日 工作+休息 占比（全 0 天不画条，整行弱化） */
function totalPct(d: StatsDay): number {
  return ((d.work_secs + d.rest_secs) / maxTotal.value) * 100;
}

/** 单日 休息占 工作+休息 的比例（条内绿色分段宽度） */
function restPct(d: StatsDay): number {
  const total = d.work_secs + d.rest_secs;
  if (total <= 0) return 0;
  return (d.rest_secs / total) * 100;
}

/** 全 0 日（弱化行） */
function isEmpty(d: StatsDay): boolean {
  return d.work_secs + d.rest_secs === 0;
}

/** MM-DD 短日期 */
function shortDate(date: string): string {
  return date.slice(5);
}

/** 秒 → 人话时长：1 小时 5 分钟 / 45 分钟 */
function fmtDur(secs: number): string {
  const mins = Math.round(secs / 60);
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  if (h > 0 && m > 0) return `${h} ${t("hours")} ${m} ${t("minutes")}`;
  if (h > 0) return `${h} ${t("hours")}`;
  return `${m} ${t("minutes")}`;
}

/** 悬停明细（当日）：工作 X · 休息 Y / N 次 */
function dayTip(d: StatsDay): string {
  const rest = `${fmtDur(d.rest_secs)} / ${d.rest_count} ${t("times")}`;
  return `${shortDate(d.date)}　${t("statsWork")} ${fmtDur(d.work_secs)} · ${t("statsRest")} ${rest}`;
}

const unlisteners: Array<() => void> = [];

onMounted(async () => {
  await refresh();
  timer = setInterval(() => void refresh(), REFRESH_MS);
  unlisteners.push(
    await listen("eyenurse://settings-shown", () => {
      void refresh();
    }),
  );
});

onUnmounted(() => {
  if (timer !== null) clearInterval(timer);
  for (const off of unlisteners.splice(0)) off();
});
</script>

<template>
  <section class="en-card">
    <h3 class="en-card-title">{{ t("statsTitle") }}</h3>

    <!-- 首拍骨架：与数据态同行高（1.5em）同列宽（46/track/64），纯占位无文案 -->
    <template v-if="!loaded">
      <div class="today" aria-hidden="true">
        <div v-for="i in 3" :key="i" class="stat">
          <span class="stat-num skel" />
          <span class="stat-label skel" />
        </div>
      </div>
      <div class="bars" aria-hidden="true">
        <p class="bars-title skel" />
        <div v-for="i in 7" :key="i" class="brow">
          <span class="bdate skel" />
          <div class="btrack">
            <div class="bfill skel" />
          </div>
          <span class="bcnt skel" />
        </div>
      </div>
    </template>

    <template v-else>
      <!-- 今日摘要：数字在上，标签不再重复数值（「休息 · 0」→「休息（0 次）」） -->
      <div v-if="today" class="today">
        <div class="stat">
          <span class="stat-num">{{ fmtDur(today.work_secs) }}</span>
          <span class="stat-label">{{ t("statsWork") }}</span>
        </div>
        <div class="stat">
          <span class="stat-num accent">{{ fmtDur(today.rest_secs) }}</span>
          <span class="stat-label">{{ t("statsRest") }}（{{ today.rest_count }} {{ t("times") }}）</span>
        </div>
        <div class="stat">
          <span class="stat-num">{{ fmtDur(today.lock_secs) }}</span>
          <span class="stat-label">{{ t("statsLock") }}</span>
        </div>
      </div>

      <!-- 近 7 天条形：图例常驻；灰 = 工作，绿 = 休息；空数据日整行弱化 -->
      <div v-if="recent.length > 0" class="bars">
        <div class="bars-head">
          <p class="bars-title">{{ t("statsLast7") }}</p>
          <div class="legend" aria-hidden="true">
            <span><i class="lg-work" />{{ t("statsWork") }}</span>
            <span><i class="lg-rest" />{{ t("statsRest") }}</span>
          </div>
        </div>
        <div
          v-for="d in recent"
          :key="d.date"
          class="brow"
          :class="{ empty: isEmpty(d) }"
        >
          <span class="bdate">{{ shortDate(d.date) }}</span>
          <div class="btrack">
            <div class="bfill" :style="{ width: totalPct(d) + '%' }">
              <div class="brest" :style="{ width: restPct(d) + '%' }" />
            </div>
          </div>
          <span class="bcnt">{{ d.rest_count > 0 ? `${t("statsRest")} ×${d.rest_count}` : "" }}</span>
          <span v-if="!isEmpty(d)" class="tipbox">{{ dayTip(d) }}</span>
        </div>
      </div>

      <p v-if="recent.length === 0" class="empty-tip">{{ t("statsEmpty") }}</p>
    </template>
  </section>
</template>

<style scoped>
/* —— 今日摘要（首段无分隔线，与 .en-row 首行语言一致；min-width 防秒跳抖动，§3）—— */
.today {
  display: flex;
  gap: var(--sp-6);
  padding: var(--sp-3) 0;
}
.stat {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 96px;
}
.stat-num {
  font-family: var(--font-num);
  font-size: var(--fs-subtitle);
  font-weight: var(--fw-semibold);
  font-variant-numeric: tabular-nums;
}
.stat-num.accent {
  color: var(--accent-strong);
}
.stat-label {
  font-size: var(--fs-hint);
  color: var(--text-2);
}

/* —— 近 7 天条形（§4.6：日期 46px + 轨道 flex + 计数 64px，行 gap 10px）—— */
.bars {
  padding: var(--sp-3) 0;
  border-top: 1px solid var(--border);
}
.bars-head {
  display: flex;
  align-items: center;
  margin-bottom: var(--sp-2);
}
.bars-title {
  margin: 0;
  font-size: var(--fs-hint);
  color: var(--text-2);
}
/* 图例（模块③）：11px 色点 + 标签，弱化为辅助信息 */
.legend {
  margin-left: auto;
  display: flex;
  gap: var(--sp-3);
  font-size: var(--fs-caption);
  color: var(--text-2);
}
.legend i {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 2px;
  margin-right: 4px;
}
.lg-work {
  background: var(--chart-work);
}
.lg-rest {
  background: var(--chart-rest);
}
.brow {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: var(--sp-1) 0;
  border-radius: var(--r-sm);
  position: relative;
}
.brow:not(.empty):hover {
  background: var(--surface-2);
}
.bdate {
  width: 46px;
  flex-shrink: 0;
  font-family: var(--font-num);
  font-size: var(--fs-hint);
  font-variant-numeric: tabular-nums;
  color: var(--text-2);
}
.btrack {
  position: relative;
  flex: 1;
  height: 10px;
  border-radius: var(--r-full);
  background: var(--surface-2);
  overflow: hidden;
}
/* 轨道 1px 描边（§2.1 图表轨道描边 = --border-strong）画在覆盖层：
   inset box-shadow 会被静态子元素盖住，必须叠在其上 */
.btrack::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  box-shadow: inset 0 0 0 1px var(--border-strong);
  pointer-events: none;
}
.bfill {
  height: 100%;
  background: var(--chart-work);
}
.brest {
  height: 100%;
  background: var(--chart-rest);
}
/* 空数据日（模块③）：整行弱化——轨道虚化 + 短横占位 + 日期降档，
   不再渲染完整空胶囊（视觉上"像坏了"） */
.brow.empty .bdate {
  color: var(--text-3);
}
.brow.empty .btrack {
  background: transparent;
}
.brow.empty .btrack::after {
  box-shadow: inset 0 0 0 1px var(--border);
}
.brow.empty::before {
  content: "—";
  position: absolute;
  left: 56px;
  top: 50%;
  transform: translateY(-50%);
  font-size: var(--fs-hint);
  color: var(--text-3);
  z-index: 1;
}
.bcnt {
  width: 64px;
  flex-shrink: 0;
  text-align: right;
  font-family: var(--font-num);
  font-size: var(--fs-hint);
  color: var(--text-2);
  white-space: nowrap;
}
/* 当日明细 tooltip（模块③）：悬停有数据的行浮出，右对齐防出窗 */
.tipbox {
  display: none;
  position: absolute;
  right: 0;
  bottom: calc(100% + 4px);
  z-index: var(--z-modal);
  background: var(--text);
  color: var(--surface);
  font-size: var(--fs-hint);
  line-height: 1.6;
  padding: 6px 10px;
  border-radius: var(--r-sm);
  white-space: nowrap;
  box-shadow: var(--shadow-1);
  pointer-events: none;
  font-variant-numeric: tabular-nums;
}
.brow:hover .tipbox {
  display: block;
}
.empty-tip {
  margin: 0;
  padding: var(--sp-3) 0;
  border-top: 1px solid var(--border);
  font-size: var(--fs-hint);
  color: var(--text-2);
}

/* —— 首拍骨架（§1.5-2）：高度 = 同槽位行高（1.5em，随自身字号），数据落定后原位填充 —— */
.skel {
  display: block;
  height: 1.5em;
  border-radius: var(--r-sm);
  background: var(--surface-2);
}
.stat-label.skel {
  width: 56px;
}
.bars-title.skel {
  width: 72px;
}
.bfill.skel {
  background: transparent; /* 轨道底与描边已由 .btrack 提供，条体留空 */
}
</style>
