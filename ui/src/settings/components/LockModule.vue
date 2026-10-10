<script setup lang="ts">
// 锁屏模块（功能页，2026-10-10 IA 调整）：立即锁屏大按钮 + 当前锁屏配置摘要。
// 锁屏的全部配置项（启用/Win+L/截图穿透/自动休息/密码/重置时间）已迁入
// 「设置」页；本页是动作入口——与托盘「立即锁屏」同源（CoreMsg::Tray(LockNow)）。
import type { LockSettingsDto } from "../types";
import { t } from "../i18n";

defineProps<{
  /** 锁屏草稿（仅读取用于状态摘要；配置在「设置」页修改） */
  lockSettings: LockSettingsDto;
}>();
const emit = defineEmits<{
  /** 立即锁屏 */
  (e: "lock-now"): void;
}>();
</script>

<template>
  <div class="page">
    <div class="page-head">
      <h2 class="page-title">{{ t("lockTitle") }}</h2>
      <p class="page-hint">{{ t("lockEnableHint") }}</p>
    </div>

    <section class="card">
      <button type="button" class="lock-btn" @click="$emit('lock-now')">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
          <rect x="3.5" y="7" width="9" height="6" rx="1.5" />
          <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
        </svg>
        {{ t("lock_now_menu") }}
      </button>
      <p class="lock-hint">{{ t("lockPasswordHint") }}</p>
    </section>

    <section class="card">
      <h3 class="card-title">{{ t("lockTitle") }} · {{ t("setting") }}</h3>
      <div class="row">
        <span class="k">{{ t("lockEnable") }}</span>
        <span class="v" :class="{ on: lockSettings.Enabled }">
          {{ lockSettings.Enabled ? "✓ " + t("skinActive") : "—" }}
        </span>
      </div>
      <div class="row">
        <span class="k">{{ t("lockWinL") }}</span>
        <span class="v" :class="{ on: lockSettings.InterceptWinL }">
          {{ lockSettings.InterceptWinL ? "✓" : "—" }}
        </span>
      </div>
      <div class="row">
        <span class="k">{{ t("lockPassword") }}</span>
        <span class="v" :class="{ on: Boolean(lockSettings.Password) }">
          {{ lockSettings.Password ? "✓" : "—" }}
        </span>
      </div>
      <p class="more-hint">{{ t("modifyAutosave") }} · {{ t("setting") }} → {{ t("lockTitle") }}</p>
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
.page-head {
  margin-bottom: var(--sp-1);
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
.card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  padding: var(--sp-1) var(--sp-4);
}
.card-title {
  margin: var(--sp-3) 0 var(--sp-1);
  font-size: var(--fs-hint);
  font-weight: var(--fw-semibold);
  letter-spacing: 0.5px;
  color: var(--text-2);
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-4);
  padding: var(--sp-3) 0;
  border-top: 1px solid var(--border);
}
.row:first-of-type {
  border-top: none;
}
.k {
  font-size: var(--fs-body);
  color: var(--text-2);
}
.v {
  font-size: var(--fs-body);
  font-weight: var(--fw-medium);
  color: var(--text-3);
}
.v.on {
  color: var(--accent-strong);
}
/* 大按钮：功能页唯一的动作，给足视觉分量 */
.lock-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--sp-2);
  width: 100%;
  height: 64px;
  margin: var(--sp-3) 0 var(--sp-2);
  border: none;
  border-radius: var(--r-lg);
  background: var(--accent);
  color: #fff;
  font-family: inherit;
  font-size: var(--fs-subtitle);
  font-weight: var(--fw-semibold);
  cursor: pointer;
  transition: background-color var(--dur-fast) var(--ease), transform var(--dur-fast) var(--ease);
}
.lock-btn svg {
  width: 20px;
  height: 20px;
}
.lock-btn:hover {
  background: var(--accent-strong);
}
.lock-btn:active {
  transform: translateY(1px);
}
.lock-btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.lock-hint {
  margin: 0 0 var(--sp-3);
  text-align: center;
  font-size: var(--fs-hint);
  color: var(--text-3);
}
.more-hint {
  margin: var(--sp-2) 0 var(--sp-3);
  font-size: var(--fs-caption);
  color: var(--text-3);
}
</style>
