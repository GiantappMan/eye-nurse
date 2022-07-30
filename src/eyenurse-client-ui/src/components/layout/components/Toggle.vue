<template>
  <div
    ref="moveZone"
    class="cursor-pointer fixed left-0 select-none"
    @click=""
    :style="{
      top: `${topPercent}%`,
    }"
  >
    <svg
      ref="iconZone"
      xmlns="http://www.w3.org/2000/svg"
      class="mx-auto h-5 w-5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="#9ba0aa"
      strokeWidth="{2}"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M13 5l7 7-7 7M5 5l7 7-7 7"
      />
    </svg>
    <div
      @click="(e) => e.stopPropagation()"
      @mousedown="handlePointerDown"
      class="mx-auto cursor-move mt-2 [writing-mode:vertical-lr] text-gray-500"
    >
      {{ t("panel") }}
    </div>
  </div>
</template>
<script lang="ts" setup>
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
const { t } = useI18n();
const topPercent = ref<number>(
  Number(localStorage.getItem("togglePosition") || 76)
);
const maxTopPercent = ref<number>();
const isDragging = ref<boolean>(false);
const iconZone = ref<HTMLElement>();
const moveZone = ref<HTMLElement>();

const handlePointerDown = (e: any) => {
  isDragging.value = true;
  document.body.style.cursor = "move";
};

const handlePointerUp = (e: any) => {
  isDragging.value = false;
  document.body.style.cursor = "";
  localStorage.setItem("togglePosition", topPercent.value.toString());
};

const handlePointerMove = (e: any) => {
  if (!isDragging.value) return;

  let y = e.y;
  {
    var tmp =
      ((y - moveZone.value!.clientHeight + iconZone.value!.clientHeight) /
        window.innerHeight) *
      100;
    if (tmp < 0) tmp = 0;
    if (tmp > maxTopPercent.value!) tmp = maxTopPercent.value!;
    topPercent.value = tmp;
  }
};

const correctHeight = () => {
  maxTopPercent.value =
    ((window.innerHeight - moveZone.value!.clientHeight) / window.innerHeight) *
    100;
  if (topPercent.value > maxTopPercent.value)
    topPercent.value = maxTopPercent.value;
  if (topPercent.value < 0) topPercent.value = 0;
};
onMounted(() => {
  maxTopPercent.value =
    ((window.innerHeight - moveZone.value!.clientHeight) / window.innerHeight) *
    100;
  window.addEventListener("resize", correctHeight);
  window.addEventListener("pointerup", handlePointerUp);
  window.document.onpointermove = handlePointerMove;
  correctHeight();
});
onUnmounted(() => {
  window.removeEventListener("resize", correctHeight);
  window.removeEventListener("pointerup", handlePointerUp);
  window.document.onpointermove = null;
});
</script>
