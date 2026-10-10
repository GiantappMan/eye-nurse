<script setup lang="ts">
// 开关控件（DS3 §4.3）：40×22 圆角轨道 + 18px 圆钮，开 = accent 绿，
// 关 = surface-2 灰；点击翻转后由父组件走 300ms 防抖保存管线（清单 I3）。
// disabled prop（§4.3）：依赖主开关的子开关传入——原生 disabled 挡掉点击
// （不翻转、不发 commit），opacity .45 弱化；行位置保持不动（§1.5 三禁）。
defineProps<{ modelValue: boolean; disabled?: boolean }>();
const emit = defineEmits<{ (e: "update:modelValue", v: boolean): void }>();
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="modelValue"
    :disabled="disabled"
    :class="['switch', modelValue ? 'switch-on' : 'switch-off']"
    @click="emit('update:modelValue', !modelValue)"
  >
    <span aria-hidden="true" class="knob" :class="{ 'knob-on': modelValue }" />
  </button>
</template>

<style scoped>
.switch {
  position: relative;
  display: inline-flex;
  flex-shrink: 0;
  width: var(--switch-w);
  height: var(--switch-h);
  border: none;
  border-radius: var(--r-full);
  cursor: pointer;
  padding: 2px;
  transition: background-color var(--dur-slow) var(--ease);
}
.switch-on {
  background-color: var(--accent);
}
.switch-off {
  background-color: var(--surface-2);
  box-shadow: inset 0 0 0 1px var(--border);
}
.switch-off:not(:disabled):hover {
  box-shadow: inset 0 0 0 1px var(--text-2);
}
.switch:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.switch:disabled {
  opacity: 0.45;
  cursor: default;
}
.knob {
  display: inline-block;
  width: var(--switch-knob);
  height: var(--switch-knob);
  border-radius: var(--r-full);
  /* 恒白圆钮（Mac 开关同款）：控件物理常数，不随暗色反转，故不走语义面 token */
  background: rgb(255 255 255);
  box-shadow: 0 1px 3px 0 rgb(0 0 0 / 0.18);
  transform: translateX(0);
  transition: transform var(--dur-slow) var(--ease);
}
.switch-on .knob {
  /* 行程 = 轨道宽 − 钮径 − 左右内衬 2px×2 */
  transform: translateX(calc(var(--switch-w) - var(--switch-knob) - 4px));
}
</style>
