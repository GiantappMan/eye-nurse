<script setup lang="ts">
// 通用模块（方案 C 侧边栏布局）：开机启动 + 恢复默认（两步确认）。
// 恢复默认沿用 v2 ClientApi.ResetSettings 语义：3s 内二次点击生效，替代 confirm() 弹窗。
import { ref } from "vue";
import type { SettingsDto } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";

defineProps<{
  /** 主设置草稿（RunWhenStarts = 注册表实测勾选态） */
  settings: SettingsDto;
}>();
const emit = defineEmits<{
  /** 任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
  /** 恢复默认（两步确认后触发） */
  (e: "reset"): void;
}>();

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
    <header class="page-head">
      <h2 class="page-title">{{ t("navGeneral") }}</h2>
      <p class="page-hint">{{ t("generalHint") }}</p>
    </header>

    <section class="en-card">
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("runWhenStarts") }}</p>
        </div>
        <SwitchToggle v-model="settings.RunWhenStarts" @update:model-value="$emit('commit')" />
      </div>
    </section>

    <section class="en-card">
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
</style>
