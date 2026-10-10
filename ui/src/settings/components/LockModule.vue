<script setup lang="ts">
// 锁屏模块（方案 C 侧边栏布局）：v3.2 仿锁屏全部选项 + 锁屏后重置时间。
// 三个子开关依赖主开关：禁用弱化不拆除（§4.3）；「锁屏后重置时间」的开关与
// 输入恒渲染（§4.2/§1.5）：开关翻转不拆输入框，行宽稳定。
// 两套保存管线在此页并存：锁屏五项 → lock-commit；重置时间 → commit。
import type { LockSettingsDto, SettingsDto } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";

defineProps<{
  /** 主设置草稿（ResetTimeout / ResetWhenSessionUnlock 走主保存管线） */
  settings: SettingsDto;
  /** 锁屏草稿（独立保存管线，回包整体合并） */
  lockSettings: LockSettingsDto;
}>();
const emit = defineEmits<{
  /** 主设置任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
  /** 锁屏设置任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "lock-commit"): void;
}>();
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2 class="page-title">{{ t("lockTitle") }}</h2>
      <p class="page-hint">{{ t("lockEnableHint") }}</p>
    </header>

    <section class="en-card">
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
    </section>

    <section class="en-card">
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("resetWhenSessionUnlock") }}</p>
          <p class="en-row-hint">{{ t("lockResetHint") }}</p>
        </div>
        <div class="row-control">
          <!-- 恒渲染 + :disabled（§4.2/§1.5）：开关翻转不拆输入框，行宽稳定 -->
          <input
            v-model="settings.ResetTimeout"
            type="text"
            class="en-input en-input-mono hms"
            placeholder="HH:mm:ss"
            spellcheck="false"
            :disabled="!settings.ResetWhenSessionUnlock"
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
.row-main {
  min-width: 0;
}
.row-control {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}
.pwd {
  width: 180px; /* 锁屏密码（§4.2） */
  text-align: left;
}
.hms {
  width: 104px; /* HH:mm:ss 定宽（§4.2） */
}
.hms:focus-visible {
  outline: none; /* ring 已由 .en-input:focus 提供 */
}
</style>
