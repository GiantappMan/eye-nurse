<script setup lang="ts">
// 左侧可折叠导航（实现者 D 所有）：复刻 v2 SliderNav.vue——白色 256px 定位侧栏、
// 顶部 logo（按语言换 logo.zh/en.svg，I2）+ 折叠按钮、顶部“首页”项、底部“设置”项
// （border-l-4，当前项 teal 高亮）。折叠态由 NavToggle 在左缘提供展开入口。
// v2 的 topNav 数组实际为空（WIP 残留），v3 沿 egui 版先例展示“首页”项并支持视图切换，
// 默认落在设置页（egui 版关键决定 5：托盘入口即“设置”）。
import { computed } from "vue";
import { currentLanguage, t } from "../i18n";
import logoZh from "../../../../assets/logo.zh.svg";
import logoEn from "../../../../assets/logo.en.svg";

const props = defineProps<{
  open: boolean;
  view: "home" | "settings";
}>();
const emit = defineEmits<{
  (e: "close"): void;
  (e: "navigate", view: "home" | "settings"): void;
}>();

/** v2: /logo.${$i18n.locale}.svg——zh 用中文标，其余语言用英文标 */
const logoSrc = computed(() => (currentLanguage.value === "zh" ? logoZh : logoEn));

function navClass(current: boolean): string {
  return current ? "nav-item nav-current" : "nav-item";
}
</script>

<template>
  <!-- v2 TransitionRoot 300ms 滑入滑出：常驻节点用 transform + visibility 等价实现 -->
  <div :class="['slider', props.open ? 'slider-open' : 'slider-closed']">
    <div class="inner">
      <div class="head">
        <a class="logo-link flex-1">
          <img :src="logoSrc" width="43" height="43" class="logo" alt="EyeNurse" />
        </a>
        <!-- v2 折叠按钮：chevron rotate-180（指向左） -->
        <svg
          class="chevron chevron-collapse"
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
          stroke="#9ba0aa"
          stroke-width="2"
          @click="emit('close')"
        >
          <path stroke-linecap="round" stroke-linejoin="round" d="M13 5l7 7-7 7M5 5l7 7-7 7" />
        </svg>
      </div>

      <!-- 顶部导航（首页） -->
      <div class="top">
        <nav class="nav-list">
          <a
            href="#"
            :class="navClass(props.view === 'home')"
            @click.prevent="emit('navigate', 'home')"
          >
            <svg class="nav-icon" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12l8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75" />
            </svg>
            <span class="truncate">{{ t("home") }}</span>
          </a>
        </nav>
      </div>

      <!-- 底部导航（设置）v2: flex-shrink-0 border-t border-gray-200 p-2 -->
      <div class="bottom">
        <nav>
          <a
            href="#"
            :class="navClass(props.view === 'settings')"
            @click.prevent="emit('navigate', 'settings')"
          >
            <svg class="nav-icon" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 010 .442c.008.379.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.43-.612.43-.991a7.723 7.723 0 010-.442c0-.379-.138-.75-.43-.992l-1.004-.827a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.281Z" />
              <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0Z" />
            </svg>
            <span class="truncate">{{ t("settings") }}</span>
          </a>
        </nav>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* v2: fixed inset-0 z-40 flex w-64 bg-white + shadow-sm ring-1 ring-gray-900/5 */
.slider {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: flex;
  width: 256px; /* v2 w-64 */
  background: #fff;
  box-shadow:
    0 1px 2px 0 rgb(0 0 0 / 0.05),
    0 0 0 1px rgb(17 24 39 / 0.05);
  transition:
    transform 0.3s ease-in-out,
    visibility 0s linear 0s;
}
.slider-closed {
  transform: translateX(-100%);
  visibility: hidden;
  transition:
    transform 0.3s ease-in-out,
    visibility 0s linear 0.3s; /* 滑出完成后再隐藏，动画不截断 */
}
/* v2: relative max-w-xs w-full pt-5 pb-4 flex-1 flex flex-col */
.inner {
  position: relative;
  width: 100%;
  max-width: 20rem;
  display: flex;
  flex: 1 1 0%;
  flex-direction: column;
  padding: 20px 0 16px; /* v2 pt-5 pb-4 */
}
.head {
  display: flex;
  flex-direction: row;
  align-items: center;
  padding: 0 16px; /* v2 px-4 */
}
.logo-link {
  flex: 1 1 0%;
}
.logo {
  height: 32px; /* v2 h-8 */
  width: auto;
}
.chevron {
  width: 20px; /* v2 h-5 w-5 */
  height: 20px;
  cursor: pointer;
  margin: 0 auto;
}
.chevron-collapse {
  transform: rotate(180deg); /* v2 rotate-180：指向左=折叠 */
}
/* v2: mt-5 flex-1 h-0 overflow-y-auto > nav px-2 space-y-1 */
.top {
  margin-top: 20px;
  flex: 1 1 0%;
  height: 0;
  overflow-y: auto;
}
.nav-list {
  padding: 0 8px;
}
/* v2: border-l-4 px-3 py-2 flex items-center text-sm font-medium
   current: bg-teal-50 border-teal-500 text-teal-700；否则 border-transparent text-gray-900 hover:bg-gray-50 */
.nav-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-left-width: 4px;
  font-size: 14px;
  font-weight: 500;
  line-height: 20px;
  color: var(--tw-gray-900);
  border-left-color: transparent;
  cursor: pointer;
  text-decoration: none;
  user-select: none;
}
.nav-item:hover {
  background: var(--tw-gray-50);
}
.nav-current {
  background: var(--tw-teal-50);
  border-left-color: var(--tw-teal-500);
  color: var(--tw-teal-700);
}
.nav-current:hover {
  background: var(--tw-teal-50);
}
/* v2: 图标 h-6 w-6 -ml-1 mr-3 flex-shrink-0；current ? text-teal-500 : text-gray-400 */
.nav-icon {
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  margin-left: -4px;
  margin-right: 12px;
  color: var(--tw-gray-400);
}
.nav-current .nav-icon {
  color: var(--tw-teal-500);
}
.bottom {
  flex-shrink: 0;
  border-top: 1px solid var(--tw-gray-200);
  padding: 8px; /* v2 p-2 */
}
</style>
