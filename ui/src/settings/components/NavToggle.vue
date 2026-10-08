<script setup lang="ts">
// 折叠态的左缘展开入口（实现者 D 所有）：复刻 v2 Toggle.vue——固定在左缘、垂直位置
// 可拖动（百分比持久化到 localStorage "togglePosition"，v2 同键）、右向 chevron 点击
// 展开侧栏、下方竖排“面板”文案。
import { onMounted, onUnmounted, ref } from "vue";
import { t } from "../i18n";

const emit = defineEmits<{ (e: "open"): void }>();

const topPercent = ref<number>(Number(localStorage.getItem("togglePosition") || 76));
const maxTopPercent = ref<number>(100);
const isDragging = ref(false);
const moveZone = ref<HTMLDivElement | null>(null);

function clampTop(): void {
  if (topPercent.value > maxTopPercent.value) topPercent.value = maxTopPercent.value;
  if (topPercent.value < 0) topPercent.value = 0;
}

/** 高度约束（v2 correctHeight）：窗口缩放后仍落在可视区内 */
function correctHeight(): void {
  if (!moveZone.value) return;
  maxTopPercent.value =
    ((window.innerHeight - moveZone.value.clientHeight) / window.innerHeight) * 100;
  clampTop();
}

function handlePointerDown(): void {
  isDragging.value = true;
  document.body.style.cursor = "move";
}

/** v2 handlePointerMove 的同款公式：光标 y 换算为百分比并收在 [0, maxTopPercent] */
function handlePointerMove(e: PointerEvent): void {
  if (!isDragging.value || !moveZone.value) return;
  const tmp =
    ((e.clientY - moveZone.value.clientHeight + 20) / window.innerHeight) * 100;
  topPercent.value = Math.min(Math.max(tmp, 0), maxTopPercent.value);
}

function handlePointerUp(): void {
  if (!isDragging.value) return;
  isDragging.value = false;
  document.body.style.cursor = "";
  localStorage.setItem("togglePosition", topPercent.value.toString());
}

onMounted(() => {
  correctHeight();
  window.addEventListener("resize", correctHeight);
  window.addEventListener("pointerup", handlePointerUp);
  document.onpointermove = handlePointerMove;
});
onUnmounted(() => {
  window.removeEventListener("resize", correctHeight);
  window.removeEventListener("pointerup", handlePointerUp);
  document.onpointermove = null;
});
</script>

<template>
  <div ref="moveZone" class="nav-toggle select-none" :style="{ top: `${topPercent}%` }">
    <!-- v2 展开按钮：右向 chevron -->
    <svg
      class="chevron"
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
      stroke="#9ba0aa"
      stroke-width="2"
      @click="emit('open')"
    >
      <path stroke-linecap="round" stroke-linejoin="round" d="M13 5l7 7-7 7M5 5l7 7-7 7" />
    </svg>
    <!-- 竖排“面板”（v2 writing-mode:vertical-lr），按住可拖动 -->
    <div
      class="panel-label"
      @pointerdown="handlePointerDown"
      @click.stop
    >
      {{ t("panel") }}
    </div>
  </div>
</template>

<style scoped>
/* v2: cursor-pointer fixed left-0 select-none，top 由百分比内联控制 */
.nav-toggle {
  position: fixed;
  left: 0;
  cursor: pointer;
  z-index: 30;
}
.chevron {
  display: block;
  margin: 0 auto;
  width: 20px; /* v2 h-5 w-5 */
  height: 20px;
}
/* v2: mx-auto cursor-move mt-2 [writing-mode:vertical-lr] text-gray-500 */
.panel-label {
  margin: 8px auto 0;
  cursor: move;
  writing-mode: vertical-lr;
  color: var(--tw-gray-500);
  font-size: 14px;
  user-select: none;
}
</style>
