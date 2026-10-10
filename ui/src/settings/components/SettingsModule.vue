<script setup lang="ts">
// 设置模块（2026-10-10 IA 调整）：全部配置项归拢于此——休息计划、锁屏、通用
// （开机启动/语言/恢复默认）。侧栏左下角「设置」进入。
// 两套保存管线并存：主设置（间隔/时长/重置时间/开机启动）→ commit；
// 锁屏（LockScreen.json）→ lock-commit；均为 300ms 防抖（清单 I3）。
import { computed, reactive, ref } from "vue";
import type { LockSettingsDto, SettingsDto } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";

const props = defineProps<{
  /** 主设置草稿 */
  settings: SettingsDto;
  /** 锁屏草稿 */
  lockSettings: LockSettingsDto;
  /** 可用语言清单（语言下拉数据源） */
  languages: string[];
  /** 当前语言 */
  currentLanguage: string;
  /** 保存成功提示（侧栏 chip 迁入本页头部固定槽） */
  saved: boolean;
}>();
const emit = defineEmits<{
  /** 主设置任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
  /** 锁屏设置任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "lock-commit"): void;
  /** 恢复默认（两步确认后触发） */
  (e: "reset"): void;
  /** 切换语言 */
  (e: "change-language", lan: string): void;
}>();

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
    <div class="page-head">
      <div>
        <h2 class="page-title">{{ t("setting") }}</h2>
        <p class="page-hint">{{ t("modifyAutosave") }}</p>
      </div>
      <!-- 保存 chip 固定槽位：出现/消失不推动标题（§1.5） -->
      <div class="saved-slot" role="status" aria-live="polite">
        <Transition name="fade">
          <span v-if="saved" class="saved-chip">{{ t("saved") }}</span>
        </Transition>
      </div>
    </div>

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

    <!-- 锁屏配置 -->
    <section class="en-card">
      <h3 class="en-card-title">{{ t("lockTitle") }}</h3>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockEnable") }}</p>
          <p class="en-row-hint">{{ t("lockEnableHint") }}</p>
        </div>
        <SwitchToggle v-model="lockSettings.Enabled" @update:model-value="$emit('lock-commit')" />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockWinL") }}</p>
          <p class="en-row-hint">{{ t("lockWinLHint") }}</p>
        </div>
        <SwitchToggle
          v-model="lockSettings.InterceptWinL"
          :disabled="!lockSettings.Enabled"
          @update:model-value="$emit('lock-commit')"
        />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockCapture") }}</p>
          <p class="en-row-hint">{{ t("lockCaptureHint") }}</p>
        </div>
        <SwitchToggle
          v-model="lockSettings.CaptureThrough"
          :disabled="!lockSettings.Enabled"
          @update:model-value="$emit('lock-commit')"
        />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("lockAutoRest") }}</p>
          <p class="en-row-hint">{{ t("lockAutoRestHint") }}</p>
        </div>
        <SwitchToggle
          v-model="lockSettings.AutoRest"
          :disabled="!lockSettings.Enabled"
          @update:model-value="$emit('lock-commit')"
        />
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

    <!-- 通用 -->
    <section class="en-card">
      <h3 class="en-card-title">{{ t("navGeneral") }}</h3>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("runWhenStarts") }}</p>
        </div>
        <SwitchToggle v-model="settings.RunWhenStarts" @update:model-value="$emit('commit')" />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("language") }}</p>
        </div>
        <select
          class="lang"
          :value="currentLanguage"
          :aria-label="t('language')"
          @change="$emit('change-language', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="lan in languages" :key="lan" :value="lan">{{ t(lan) }}</option>
        </select>
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-hint">{{ t("restoreHint") }}</p>
        </div>
        <button
          type="button"
          class="en-btn en-btn-ghost"
          :class="{ 'is-danger': confirmReset }"
          @click="onReset"
        >
          {{ confirmReset ? t("confirmRestore") : t("restoreDefaults") }}
        </button>
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
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
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
/* 保存 chip 固定槽位（§5.1.2）：恒占高度 */
.saved-slot {
  height: 22px;
  display: flex;
  align-items: center;
}
.saved-chip {
  font-size: var(--fs-hint);
  font-weight: var(--fw-medium);
  color: var(--accent-strong);
  background: var(--accent-soft);
  padding: 3px 10px;
  border-radius: var(--r-full);
  white-space: nowrap;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--dur-slow) var(--ease);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
.row-main {
  min-width: 0;
}
.row-control {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
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
.pwd {
  width: 180px;
  text-align: left;
}
.lang {
  appearance: none;
  height: var(--control-h-sm);
  padding: 0 24px 0 12px;
  font-size: var(--fs-hint);
  font-family: inherit;
  color: var(--text-2);
  background-color: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  cursor: pointer;
  background-image: linear-gradient(45deg, transparent 50%, var(--text-2) 50%),
    linear-gradient(135deg, var(--text-2) 50%, transparent 50%);
  background-position:
    calc(100% - 14px) calc(50% - 1px),
    calc(100% - 9px) calc(50% - 1px);
  background-size: 5px 5px;
  background-repeat: no-repeat;
  transition:
    color var(--dur-fast) var(--ease),
    border-color var(--dur-fast) var(--ease);
}
.lang:hover {
  color: var(--text);
  border-color: var(--accent);
}
</style>
