<script setup lang="ts">
// 护眼统计卡片（v3.2）：今日三桶摘要 + 近 7 天 堆叠条形报表。
// 数据自取（get_stats 近 30 天，展示取最近 7 天）：挂载 + 30s 轮询 +
// settings-shown（窗口被托盘重新唤起）时刷新；口径见 crates/eyenurse-core/src/stats.rs。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { StatsDay } from "../types";

const REFRESH_MS = 30_000;
const SHOW_DAYS = 7;

const days = ref<StatsDay[]>([]);
let timer: ReturnType<typeof setInterval> | null = null;

async function refresh(): Promise<void> {
  try {
    days.value = await invoke<StatsDay[]>("get_stats");
  } catch (e) {
    console.error("get_stats 失败", e);
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

/** 单日 工作+休息 占比（全 0 天不画条） */
function totalPct(d: StatsDay): number {
  return ((d.work_secs + d.rest_secs) / maxTotal.value) * 100;
}

/** 单日 休息占 工作+休息 的比例（条内绿色分段宽度） */
function restPct(d: StatsDay): number {
  const total = d.work_secs + d.rest_secs;
  if (total <= 0) return 0;
  return (d.rest_secs / total) * 100;
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
  <section class="card">
    <h3 class="card-title">{{ t("statsTitle") }}</h3>

    <!-- 今日摘要 -->
    <div v-if="today" class="today">
      <div class="stat">
        <span class="stat-num">{{ fmtDur(today.work_secs) }}</span>
        <span class="stat-label">{{ t("statsWork") }}</span>
      </div>
      <div class="stat">
        <span class="stat-num accent">{{ fmtDur(today.rest_secs) }}</span>
        <span class="stat-label">{{ t("statsRest") }} · {{ today.rest_count }}</span>
      </div>
      <div class="stat">
        <span class="stat-num">{{ fmtDur(today.lock_secs) }}</span>
        <span class="stat-label">{{ t("statsLock") }}</span>
      </div>
    </div>

    <!-- 近 7 天条形：灰色 = 工作，绿色 = 休息 -->
    <div v-if="recent.length > 0" class="bars">
      <p class="bars-title">{{ t("statsLast7") }}</p>
      <div v-for="d in recent" :key="d.date" class="brow">
        <span class="bdate">{{ shortDate(d.date) }}</span>
        <div class="btrack">
          <div class="bfill" :style="{ width: totalPct(d) + '%' }">
            <div class="brest" :style="{ width: restPct(d) + '%' }" />
          </div>
        </div>
        <span class="bcnt">{{ d.rest_count > 0 ? `×${d.rest_count}` : "" }}</span>
      </div>
    </div>

    <p v-if="recent.length === 0" class="empty">{{ t("statsEmpty") }}</p>
  </section>
</template>

<style scoped>
/* —— 今日摘要 —— */
.today {
  display: flex;
  gap: 28px;
  padding: 12px 0 14px;
  border-top: 1px solid var(--border);
}
.stat {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.stat-num {
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 20px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.stat-num.accent {
  color: var(--accent-strong);
}
.stat-label {
  font-size: 12px;
  color: var(--text-2);
}

/* —— 近 7 天条形 —— */
.bars {
  padding: 2px 0 12px;
  border-top: 1px solid var(--border);
}
.bars-title {
  margin: 10px 0 8px;
  font-size: 12px;
  color: var(--text-2);
}
.brow {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 0;
}
.bdate {
  width: 46px;
  flex-shrink: 0;
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 12px;
  color: var(--text-2);
}
.btrack {
  flex: 1;
  height: 10px;
  border-radius: 5px;
  background: var(--surface-2);
  overflow: hidden;
}
.bfill {
  height: 100%;
  border-radius: 5px;
  background: var(--border);
  min-width: 0;
}
.brest {
  height: 100%;
  border-radius: 5px 0 0 5px;
  background: var(--accent);
}
.bcnt {
  width: 34px;
  flex-shrink: 0;
  text-align: right;
  font-size: 12px;
  color: var(--text-2);
}
.empty {
  margin: 0;
  padding: 12px 0 14px;
  border-top: 1px solid var(--border);
  font-size: 12px;
  color: var(--text-2);
}
</style>
