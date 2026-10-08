<script setup lang="ts">
// 设置页主区（实现者 D 所有）：复刻 v2 Settings.vue 的四行设置 + 标题旁“保存中”
// spinner；v3 增补第 5 行“恢复默认”入口（清单 F11，v2 的 ResetSettings 有后端无前端入口）。
// 提交时机（清单 I3）：文本框失焦/回车、开关点击 → 父组件 300ms 防抖保存；
// parse + clamp 在命令侧，页面只做 HH:mm:ss 占位（清单 I4）。
import type { SettingsDto } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";

defineProps<{
  /** 父组件持有的响应式草稿（v2 同款：直接绑定字段，保存回包整体合并） */
  settings: SettingsDto;
  /** invoke 未决期 = “保存中”（标题旁 spinner，v2 loading） */
  saving: boolean;
}>();
defineEmits<{
  /** 任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
  /** 恢复默认（F11） */
  (e: "reset"): void;
}>();
</script>

<template>
  <div class="settings-page">
    <div class="px-4 sm:px-6">
      <!-- 标题行：clientSettings + 保存中 spinner（v2 animate-spin，仅保存期间出现） -->
      <div class="flex items-center">
        <h2 class="page-title">{{ t("clientSettings") }}</h2>
        <svg v-if="saving" class="spinner" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
          <path
            class="opacity-75"
            fill="#14b8a6"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
          />
        </svg>
      </div>

      <ul role="list" class="rows">
        <!-- 行 1：开机启动开关（勾选态 = 注册表实测值，Q10/F10 由 get_settings 覆盖） -->
        <li class="row">
          <p class="label">{{ t("runWhenStarts") }}</p>
          <SwitchToggle
            v-model="settings.RunWhenStarts"
            @update:model-value="$emit('commit')"
          />
        </li>

        <!-- 行 2：锁屏后重置开关 +（开关打开时）超时输入（v2 v-if 同款） -->
        <li class="row">
          <p class="label">{{ t("resetWhenSessionUnlock") }}</p>
          <div class="right">
            <input
              v-if="settings.ResetWhenSessionUnlock"
              v-model="settings.ResetTimeout"
              type="text"
              class="hms"
              placeholder="HH:mm:ss"
              @blur="$emit('commit')"
              @keydown.enter="($event.target as HTMLInputElement).blur()"
            />
            <SwitchToggle
              v-model="settings.ResetWhenSessionUnlock"
              @update:model-value="$emit('commit')"
            />
          </div>
        </li>

        <!-- 行 3：休息间隔 -->
        <li class="row">
          <p class="label">{{ t("restInterval") }}</p>
          <input
            v-model="settings.RestInterval"
            type="text"
            class="hms"
            placeholder="HH:mm:ss"
            @blur="$emit('commit')"
            @keydown.enter="($event.target as HTMLInputElement).blur()"
          />
        </li>

        <!-- 行 4：休息时长 -->
        <li class="row">
          <p class="label">{{ t("restDuration") }}</p>
          <input
            v-model="settings.RestDuration"
            type="text"
            class="hms"
            placeholder="HH:mm:ss"
            @blur="$emit('commit')"
            @keydown.enter="($event.target as HTMLInputElement).blur()"
          />
        </li>

        <!-- 行 5：恢复默认（F11）——语义 = v2 ClientApi.ResetSettings（写默认盘 + 自启 +
             计时器重启），由父组件 invoke("reset_settings") 并用回包刷新 -->
        <li class="row">
          <button type="button" class="reset-btn" @click="$emit('reset')">
            {{ t("restoreDefaults") }}
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  max-width: 80rem; /* v2 max-w-7xl */
  margin: 0 auto;
  padding-top: 24px; /* v2 pt-6 */
}
.page-title {
  font-size: 18px; /* v2 text-lg */
  line-height: 24px; /* v2 leading-6 */
  font-weight: 500; /* v2 font-medium */
  color: var(--tw-gray-900);
}
.spinner {
  margin-left: 8px; /* v2 ml-2 */
  width: 24px; /* v2 w-6 */
  height: 24px;
  color: var(--tw-gray-900);
  animation: spin 1s linear infinite; /* v2 animate-spin */
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
.rows {
  margin-top: 8px; /* v2 mt-2 */
  border-top: 1px solid var(--tw-gray-200); /* v2 divide-y divide-gray-200 */
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between; /* v2 justify-between */
  padding: 16px 0; /* v2 py-4 */
  border-bottom: 1px solid var(--tw-gray-200);
}
.label {
  font-size: 14px; /* v2 text-sm */
  font-weight: 500; /* v2 font-medium */
  color: var(--tw-gray-900);
}
.right {
  display: flex;
  align-items: center;
}
/* v2: w-28 text-center px-5 sm:text-sm border-gray-300 rounded-md
   focus:ring-teal-500 focus:border-teal-500 */
.hms {
  width: 112px; /* v2 w-28 */
  text-align: center;
  padding: 6px 20px; /* v2 px-5 + 默认纵向内边距 */
  font-size: 14px;
  line-height: 20px;
  border: 1px solid var(--tw-gray-300);
  border-radius: 6px; /* v2 rounded-md */
  outline: none;
}
.hms:focus {
  border-color: var(--tw-teal-500);
  box-shadow: 0 0 0 1px var(--tw-teal-500); /* v2 focus:ring-teal-500 */
}
.reset-btn {
  font-size: 14px;
  line-height: 20px;
  padding: 6px 16px;
  border: 1px solid var(--tw-gray-300);
  border-radius: 6px;
  color: var(--tw-gray-900);
  background: #fff;
  cursor: pointer;
}
.reset-btn:hover {
  background: var(--tw-gray-50);
}
</style>
