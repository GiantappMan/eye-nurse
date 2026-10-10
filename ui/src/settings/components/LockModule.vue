<script setup lang="ts">
// 锁屏模块（功能页）：立即锁屏大按钮 + 锁屏配置卡。
// 2026-10-10 IA 二次调整：锁屏配置从设置子页迁回本页原位直改（含会话解锁
// 重置——语义归属锁屏，走主设置管线），删掉只读摘要卡。
import { reactive } from "vue";
import type { LockSettingsDto, SettingsDto } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";

const props = defineProps<{
  /** 锁屏草稿（LockScreen.json 管线） */
  lockSettings: LockSettingsDto;
  /** 主设置草稿（本页只编辑会话解锁重置一组） */
  settings: SettingsDto;
}>();
const emit = defineEmits<{
  /** 立即锁屏 */
  (e: "lock-now"): void;
  /** 锁屏配置任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "lock-commit"): void;
  /** 会话解锁重置提交（主设置管线） */
  (e: "commit"): void;
}>();

/** HH:mm:ss 宽松校验：仅控制输入态红框，解析在命令侧（清单 I4） */
function hmsValid(s: string): boolean {
  return /^(\d{1,2}):([0-5]\d):([0-5]\d)$/.test(s.trim());
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
    <div class="page-head">
      <h2 class="page-title">{{ t("lockTitle") }}</h2>
      <p class="page-hint">{{ t("lockSubtitle") }}</p>
    </div>

    <section class="card">
      <button type="button" class="lock-btn" @click="$emit('lock-now')">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
          <rect x="3.5" y="7" width="9" height="6" rx="1.5" />
          <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
        </svg>
        {{ t("lock_now_menu") }}
      </button>
    </section>

    <section class="en-card">
      <h3 class="en-card-title">{{ t("lockTitle") }} · {{ t("setting") }}</h3>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockHotkey") }}</p>
          <p class="en-row-hint">{{ t("lockHotkeyHint") }}</p>
        </div>
        <input
          v-model="lockSettings.Hotkey"
          type="text"
          class="en-input en-input-mono hotkey"
          :placeholder="t('lockHotkeyPlaceholder')"
          spellcheck="false"
          @blur="$emit('lock-commit')"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockCapture") }}</p>
          <p class="en-row-hint">{{ t("lockCaptureHint") }}</p>
        </div>
        <SwitchToggle v-model="lockSettings.CaptureThrough" @update:model-value="$emit('lock-commit')" />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockAutoRest") }}</p>
          <p class="en-row-hint">{{ t("lockAutoRestHint") }}</p>
        </div>
        <SwitchToggle v-model="lockSettings.AutoRest" @update:model-value="$emit('lock-commit')" />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockPassword") }}</p>
          <p class="en-row-hint">{{ t("lockPasswordHint") }}</p>
        </div>
        <input
          v-model="lockSettings.Password"
          type="password"
          class="en-input pwd"
          :placeholder="t('lockPasswordHint')"
          autocomplete="new-password"
          spellcheck="false"
          @blur="$emit('lock-commit')"
          @keydown.enter="($event.target as HTMLInputElement).blur()"
        />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("resetWhenSessionUnlock") }}</p>
          <p class="en-row-hint">{{ t("lockResetHint") }}</p>
        </div>
        <div class="row-control">
          <input
            v-model="settings.ResetTimeout"
            type="text"
            class="en-input en-input-mono hms"
            :class="{ 'is-invalid': touched.has('ResetTimeout') && !hmsValid(settings.ResetTimeout) }"
            placeholder="HH:mm:ss"
            spellcheck="false"
            :disabled="!settings.ResetWhenSessionUnlock"
            @blur="onHmsBlur('ResetTimeout')"
            @keydown.enter="($event.target as HTMLInputElement).blur()"
          />
          <SwitchToggle
            v-model="settings.ResetWhenSessionUnlock"
            @update:model-value="$emit('commit')"
          />
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
.row-main {
  min-width: 0;
}
.row-control {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
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
.hms {
  width: 104px; /* HH:mm:ss 定宽（§4.2） */
}
.hms:focus-visible {
  outline: none; /* ring 已由 .en-input:focus 提供 */
}
.pwd {
  width: 180px;
  text-align: left;
}
.hotkey {
  width: 140px; /* Ctrl+Shift+F12 最长组合约 14 字符（等宽） */
  text-align: left;
}
</style>
