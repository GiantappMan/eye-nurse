<script setup lang="ts">
// 开关控件（实现者 D 所有）：复刻 v2 headlessui Switch 的观感——h-6 w-11 圆角轨道 +
// h-5 w-5 白色圆钮（开态 translate-x-5），开 = teal-500 / 关 = gray-200。
// v2 在 Switch 上绑 @blur 保存；v3 按 I3「开关点击即提交」——点击翻转后由父组件
// 走同一 300ms 防抖管线（net 行为一致，关窗前有 settings-closing flush 兜底）。
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
    <span aria-hidden="true" :class="['knob', modelValue ? 'knob-on' : 'knob-off']" />
  </button>
</template>

<style scoped>
/* v2: ml-4 relative inline-flex h-6 w-11 border-2 border-transparent rounded-full
   transition-colors ease-in-out duration-200 focus:ring-2 focus:ring-offset-2 focus:ring-sky-500 */
.switch {
  margin-left: 16px; /* v2 ml-4 */
  position: relative;
  display: inline-flex;
  flex-shrink: 0;
  width: 44px; /* v2 w-11 */
  height: 24px; /* v2 h-6 */
  border: 2px solid transparent;
  border-radius: 9999px;
  cursor: pointer;
  padding: 0;
  transition: background-color 0.2s ease-in-out;
}
.switch-on {
  background-color: var(--tw-teal-500);
}
.switch-off {
  background-color: var(--tw-gray-200);
}
.switch-off:hover {
  background-color: var(--tw-gray-300);
}
.switch:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--eyenurse-page-bg), 0 0 0 4px var(--tw-sky-500); /* v2 focus:ring-sky-500 + offset-2 */
}
/* v2: inline-block h-5 w-5 rounded-full bg-white shadow transform transition ease-in-out duration-200 */
.knob {
  display: inline-block;
  width: 20px; /* v2 w-5 */
  height: 20px; /* v2 h-5 */
  border-radius: 9999px;
  background: #fff;
  box-shadow: 0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1);
  transform: translateX(0);
  transition: transform 0.2s ease-in-out;
}
.knob-on {
  transform: translateX(20px); /* v2 translate-x-5 */
}
</style>
