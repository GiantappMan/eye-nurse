<script setup lang="ts">
// 设置窗根组件（实现者 D 所有；tauri 架构文档 §8.2）。
//
// 布局复刻 v2 components/layout/Index.vue：可折叠左侧导航（默认展开，开合态存
// localStorage，v2 同键 sidebarOpen）+ 白色 64px 页头条 + 中央内容 + 页脚（社交链接 +
// 语言下拉 + copyright）。v2 的 main/Footer 在侧栏展开时整体右移 pl-64。
//
// 保存管线（清单 I3/F9，egui 版 ui/settings.rs 的网页等价）：
//   控件提交（文本框失焦/回车、开关点击）→ 300ms 防抖 → invoke("set_settings")
//   → 用应用后 DTO 回包整体合并刷新（v2 Object.assign(settings, 回包) 等价）；
//   invoke 未决期即“保存中”（设置标题旁 spinner，v2 loading）。
//   parse + clamp 在命令侧（I4），页面不做数值解析。
//
// 事件接线（架构 §4.2/§8.1/§8.2）：
//   i18n-changed   → 重取整包文案（语言切换广播，原生侧托盘/标题由 CoreLoop 刷）
//   settings-changed → 非保存中时重取 get_settings（刷新非编辑态）
//   settings-shown → 只重同步注册表实测勾选态（Q10），不碰文本框草稿
//   settings-closing → 把未到点的防抖保存立即 flush（关窗=X = hide，草稿保留）
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { onMounted, onUnmounted, reactive, ref, toRaw, watch } from "vue";
import { currentLanguage, loadLanguageList, loadTexts } from "./i18n";
import { emptySettings, type SettingsDto } from "./types";
import SliderNav from "./components/SliderNav.vue";
import NavToggle from "./components/NavToggle.vue";
import SettingsView from "./components/SettingsView.vue";
import FooterBar from "./components/FooterBar.vue";

/** 防抖窗口（v2 Settings.vue：lodash/debounce 300ms，清单 I3） */
const SAVE_DEBOUNCE_MS = 300;

// —— 草稿（页面加载后经 Object.assign 填充；加载失败保持空白占位）——
const settings = reactive<SettingsDto>(emptySettings());
const saving = ref(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

// —— 视图与侧栏（v2 路由 / 首页、/settings；默认落在设置页，egui 版关键决定 5）——
type ViewName = "home" | "settings";
const view = ref<ViewName>("settings");
const sidebarOpen = ref<boolean>(loadSidebarOpen());

function loadSidebarOpen(): boolean {
  try {
    return JSON.parse(localStorage.getItem("sidebarOpen") ?? "true") === true;
  } catch {
    return true; // v2 默认 true
  }
}
watch(sidebarOpen, (open) => {
  localStorage.setItem("sidebarOpen", JSON.stringify(open)); // v2 onOpenChanged
});

// —— 保存管线 ——

/** 任一控件提交：排 300ms 防抖保存（重复提交顺延，等价 lodash debounce 尾触发） */
function scheduleSave(): void {
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void flushSave();
  }, SAVE_DEBOUNCE_MS);
}

function discardScheduledSave(): void {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
}

/** 立即保存：防抖到点 / 关窗 flush 共用；回包整体合并刷新（v2 Object.assign 等价） */
async function flushSave(): Promise<void> {
  discardScheduledSave();
  saving.value = true;
  try {
    const applied = await invoke<SettingsDto>("set_settings", {
      dto: { ...toRaw(settings) },
    });
    Object.assign(settings, applied);
  } catch (e) {
    console.error("set_settings 失败", e);
  } finally {
    saving.value = false;
  }
}

/** 恢复默认（F11）：v2 ClientApi.ResetSettings（写默认盘 + 自启 + 计时器重启）+ 回包刷新 */
async function resetDefaults(): Promise<void> {
  discardScheduledSave();
  saving.value = true;
  try {
    Object.assign(settings, await invoke<SettingsDto>("reset_settings"));
  } catch (e) {
    console.error("reset_settings 失败", e);
  } finally {
    saving.value = false;
  }
}

/** 语言切换（I5/H4）：乐观更新 → 写盘 + 托盘/标题刷新 + i18n-changed 广播；重取幂等 */
async function changeLanguage(lan: string): Promise<void> {
  currentLanguage.value = lan;
  try {
    await invoke("set_current_language", { lan });
  } catch (e) {
    console.error("set_current_language 失败", e);
  }
  try {
    await loadTexts();
  } catch (e) {
    console.error("get_texts 失败", e);
  }
}

// —— 事件/数据加载 ——

/** 非编辑态刷新（§4.2）：自身保存的回写走 flushSave 回包，保存中跳过防覆盖草稿 */
async function refreshFromBackend(): Promise<void> {
  if (saving.value) return;
  try {
    Object.assign(settings, await invoke<SettingsDto>("get_settings"));
  } catch (e) {
    console.error("get_settings 失败", e);
  }
}

/** Q10/F10：注册表实测勾选态（窗口被托盘重新唤起时重同步；不碰文本草稿，§8.2） */
async function syncAutostart(): Promise<void> {
  try {
    settings.RunWhenStarts = await invoke<boolean>("get_autostart");
  } catch (e) {
    console.error("get_autostart 失败", e);
  }
}

const unlisteners: Array<() => void> = [];

onMounted(async () => {
  // 启动依赖一次取全：整包文案 + 语言清单 + 设置（含注册表实测覆盖）
  await Promise.all([
    loadTexts().catch((e) => console.error("get_texts 失败", e)),
    loadLanguageList().catch((e) => console.error("语言清单加载失败", e)),
    refreshFromBackend(),
  ]);
  unlisteners.push(
    await listen("eyenurse://i18n-changed", () => {
      void loadTexts().catch((e) => console.error("get_texts 失败", e));
    }),
    await listen("eyenurse://settings-changed", () => {
      void refreshFromBackend();
    }),
    await listen("eyenurse://settings-shown", () => {
      void syncAutostart();
    }),
    await listen("eyenurse://settings-closing", () => {
      // §8.1：未到点的防抖修改立即 flush；无编辑不发（避免无谓重启计时器）
      if (saveTimer !== null) void flushSave();
    }),
  );
});

onUnmounted(() => {
  for (const off of unlisteners.splice(0)) off();
});
</script>

<template>
  <div class="app-root">
    <SliderNav
      :open="sidebarOpen"
      :view="view"
      @close="sidebarOpen = false"
      @navigate="(v) => (view = v)"
    />
    <NavToggle v-if="!sidebarOpen" @open="sidebarOpen = true" />

    <div class="content">
      <!-- v2 header：h-16 白条 + 底部细阴影（内容为空，纯装饰） -->
      <header class="header" />
      <div class="scroll-area">
        <!-- v2 Index.vue：main 与 Footer 在侧栏展开时整体右移 pl-64 -->
        <main class="main" :class="{ 'content-shift': sidebarOpen }">
          <div v-if="view === 'home'" class="home-page">EyeNurse2</div>
          <SettingsView
            v-else
            :settings="settings"
            :saving="saving"
            @commit="scheduleSave"
            @reset="resetDefaults"
          />
        </main>
        <div :class="{ 'content-shift': sidebarOpen }">
          <FooterBar @change-language="changeLanguage" />
        </div>
      </div>
    </div>
  </div>
</template>

<style>
/* v2 tailwind 色板等价变量（值与 Settings.vue/SliderNav.vue/Footer.vue 的 class 一一对应；
   egui 版 ui/settings.rs 头注第 7 条的同款局部定义——theme.json 归 C 所有，只收原生 bar/mask 色） */
:root {
  --tw-gray-50: #f9fafb;
  --tw-gray-200: #e5e7eb;
  --tw-gray-300: #d1d5db;
  --tw-gray-400: #9ca3af;
  --tw-gray-500: #6b7280;
  --tw-gray-600: #4b5563;
  --tw-gray-900: #111827;
  --tw-teal-50: #f0fdfa;
  --tw-teal-500: #14b8a6;
  --tw-teal-600: #0d9488;
  --tw-teal-700: #0f766e;
  --tw-sky-500: #0ea5e9;
  --eyenurse-page-bg: #ffffff;
  color-scheme: light;
}
* {
  box-sizing: border-box;
}
html,
body {
  margin: 0;
  font-family:
    "Segoe UI",
    "Microsoft YaHei",
    "PingFang SC",
    sans-serif;
  font-size: 16px;
  color: var(--tw-gray-900);
}
</style>

<style scoped>
/* v2: flex flex-col bg-gray-50（页面整体底色） */
.app-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--tw-gray-50);
}
.content {
  flex: 1 1 0%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
/* v2 header: top-0 flex-shrink-0 h-16 bg-white shadow-sm ring-1 ring-gray-900/5 */
.header {
  flex-shrink: 0;
  height: 64px;
  background: #fff;
  box-shadow:
    0 1px 2px 0 rgb(0 0 0 / 0.05),
    0 0 0 1px rgb(17 24 39 / 0.05);
}
/* 窗口定高（900×500）：header 以下可滚动，页脚在文档流底部（v2 整页滚动的等价） */
.scroll-area {
  flex: 1 1 0%;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}
.main {
  flex: 1 1 0%;
}
/* v2 md:pl-64：侧栏展开时内容右移一个侧栏宽 */
.content-shift {
  padding-left: 256px;
}
/* v2 Home.vue：静态文本 */
.home-page {
  padding: 24px 16px;
}
</style>
