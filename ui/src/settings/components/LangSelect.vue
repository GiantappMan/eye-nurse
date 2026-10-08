<script setup lang="ts">
// 语言下拉（实现者 D 所有）：复刻 v2 Footer.vue 的 headlessui Listbox——w-28 圆角
// 按钮显示当前语言名（t(lan)，如 “简体中文”/English）、右侧 chevron；选项浮层里
// 当前项左侧带 teal 对勾（CheckIcon）。无依赖手写，替代 @headlessui/vue。
import { onMounted, onUnmounted, ref } from "vue";
import { t } from "../i18n";

const props = defineProps<{
  modelValue: string;
  options: string[];
}>();
const emit = defineEmits<{ (e: "update:modelValue", v: string): void }>();

const open = ref(false);
const root = ref<HTMLDivElement | null>(null);

function select(lan: string): void {
  open.value = false;
  if (lan !== props.modelValue) emit("update:modelValue", lan);
}

/** 点击下拉外部时收起（v2 Listbox 的默认行为） */
function onDocPointerDown(e: PointerEvent): void {
  if (open.value && root.value && !root.value.contains(e.target as Node)) {
    open.value = false;
  }
}
onMounted(() => document.addEventListener("pointerdown", onDocPointerDown));
onUnmounted(() => document.removeEventListener("pointerdown", onDocPointerDown));
</script>

<template>
  <div ref="root" class="lang-select">
    <button type="button" class="lang-btn" @click="open = !open">
      <span class="lang-current">{{ t(modelValue) }}</span>
      <!-- v2 SelectorIcon（chevron-up-down） -->
      <span class="lang-chevron">
        <svg class="selector" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 15L12 18.75 15.75 15m-7.5-6L12 5.25 15.75 9" />
        </svg>
      </span>
    </button>

    <ul v-if="open" class="lang-options">
      <li
        v-for="lan in options"
        :key="`locale-${lan}`"
        :class="['lang-option', lan === modelValue ? 'option-selected-text' : '']"
        @click="select(lan)"
      >
        <!-- v2 选中项左侧 CheckIcon（teal-600） -->
        <span v-if="lan === modelValue" class="option-check">
          <svg class="check" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
            <path fill-rule="evenodd" d="M4.5 12.75l6 6 9-13.5" clip-rule="evenodd" />
          </svg>
        </span>
        <span :class="lan === modelValue ? 'option-label option-label-selected' : 'option-label'">
          {{ t(lan) }}
        </span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.lang-select {
  position: relative;
  width: 112px; /* v2 w-28 */
  margin-top: -8px; /* v2 -mt-2 */
}
/* v2: relative w-full cursor-default rounded-lg bg-white py-2 pl-3 pr-10 text-left shadow-md */
.lang-btn {
  position: relative;
  width: 100%;
  cursor: default;
  background: #fff;
  border-radius: 8px;
  padding: 8px 40px 8px 12px;
  text-align: left;
  box-shadow: 0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1);
  border: none;
}
.lang-current {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
  line-height: 20px;
  color: var(--tw-gray-600); /* v2 text-gray-600 */
}
.lang-chevron {
  position: absolute;
  inset-block: 0;
  right: 0;
  display: flex;
  align-items: center;
  padding-right: 8px;
  pointer-events: none;
}
.selector {
  width: 20px;
  height: 20px;
  color: var(--tw-gray-400);
}
/* v2: absolute mt-1 max-h-60 w-full overflow-auto rounded-md bg-white py-1 shadow-lg ring-1 ring-black/5 */
.lang-options {
  position: absolute;
  margin-top: 4px;
  width: 100%;
  max-height: 240px;
  overflow: auto;
  background: #fff;
  border-radius: 6px;
  padding: 4px 0;
  box-shadow:
    0 10px 15px -3px rgb(0 0 0 / 0.1),
    0 4px 6px -4px rgb(0 0 0 / 0.1),
    0 0 0 1px rgb(0 0 0 / 0.05);
  z-index: 50;
  list-style: none;
  margin-bottom: 0;
}
.lang-option {
  position: relative;
  cursor: default;
  user-select: none;
  padding: 8px 16px 8px 40px; /* v2 py-2 pl-10 pr-4 */
  font-size: 14px;
  line-height: 20px;
  color: var(--tw-gray-900);
  list-style: none;
}
.lang-option:hover {
  background: #ccfbf1; /* v2 active: bg-teal-100 */
  color: #134e4a; /* v2 text-teal-900 */
}
.option-selected-text {
  font-weight: 500; /* v2 selected: font-medium */
}
.option-check {
  position: absolute;
  inset-block: 0;
  left: 0;
  display: flex;
  align-items: center;
  padding-left: 12px; /* v2 pl-3 */
  color: var(--tw-teal-600);
}
.check {
  width: 20px;
  height: 20px;
}
.option-label {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
