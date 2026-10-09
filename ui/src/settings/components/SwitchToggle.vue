<script setup lang="ts">
// 开关控件（v3.1 重设计）：40×22 圆角轨道 + 18px 圆钮，开 = accent 绿，
// 关 = surface-2 灰；点击翻转后由父组件走 300ms 防抖保存管线（清单 I3）。
defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ (e: "update:modelValue", v: boolean): void }>();
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="modelValue"
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
  width: 40px;
  height: 22px;
  border: none;
  border-radius: 999px;
  cursor: pointer;
  padding: 2px;
  transition: background-color 0.18s ease;
}
.switch-on {
  background-color: var(--accent);
}
.switch-off {
  background-color: var(--surface-2);
  box-shadow: inset 0 0 0 1px var(--border);
}
.switch-off:hover {
  box-shadow: inset 0 0 0 1px var(--text-2);
}
.switch:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.knob {
  display: inline-block;
  width: 18px;
  height: 18px;
  border-radius: 999px;
  background: #fff;
  box-shadow: 0 1px 3px 0 rgb(0 0 0 / 0.18);
  transform: translateX(0);
  transition: transform 0.18s ease;
}
.switch-on .knob {
  transform: translateX(18px);
}
</style>
