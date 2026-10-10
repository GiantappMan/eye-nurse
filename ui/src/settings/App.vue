<script setup lang="ts">
// 设置窗根组件（方案 C · 侧边栏模块导航，2026-10-10 用户拍板；取代 v3 的一页流式布局）：
// 左侧模块列表（休息 / 锁屏 / 统计 / 皮肤 / 通用），右侧只显示当前模块——
// 每个模块独占一页互不掺杂；语言切换与版权收进侧栏底部，不再设页内顶栏
// （原生标题栏已承担品牌名）。独立「皮肤中心」窗口同日废弃，皮肤作为模块页迁入。
//
// 数据管线与事件接线保持不变：
//   控件提交 → 300ms 防抖 → invoke("set_settings") → 回包整体合并；
//   i18n-changed → 重取文案；settings-changed → 非保存中重取设置；
//   settings-shown → 重同步注册表实测勾选态；settings-closing → flush 防抖保存；
//   eyenurse://tick → 休息页 Hero 实时倒计时/状态（core.rs 每拍下发）。
// 皮肤数据管线（清单/预览/自定义色）整体归属 SkinModule 自管。
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { computed, onMounted, onUnmounted, reactive, ref, toRaw } from "vue";
import { currentLanguage, languages, loadLanguageList, loadTexts, t } from "./i18n";
import {
  emptyLockSettings,
  emptySettings,
  type LockSettingsDto,
  type SettingsDto,
} from "./types";
import RestModule from "./components/RestModule.vue";
import LockModule from "./components/LockModule.vue";
import StatsCard from "./components/StatsCard.vue";
import SkinModule from "./components/SkinModule.vue";
import SettingsModule from "./components/SettingsModule.vue";

/** 防抖窗口（v2 Settings.vue：lodash/debounce 300ms，清单 I3） */
const SAVE_DEBOUNCE_MS = 300;

// —— 草稿（页面加载后经 Object.assign 填充；加载失败保持空白占位）——
const settings = reactive<SettingsDto>(emptySettings());
const saving = ref(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

/** 锁屏草稿（v3.2）：独立文件 LockScreen.json，保存管线与主设置同款 300ms 防抖 */
const lockSettings = reactive<LockSettingsDto>(emptyLockSettings());
let lockSaveTimer: ReturnType<typeof setTimeout> | null = null;

/** 保存成功的短暂提示（设置页头部固定槽位，1.6s 后淡出） */
const justSaved = ref(false);
let savedFadeTimer: ReturnType<typeof setTimeout> | null = null;

/** 当前模块（2026-10-10 IA：上部=功能 休息/锁屏/统计/皮肤，左下角=设置；默认休息） */
const activeModule = ref<"rest" | "lock" | "stats" | "skin" | "settings">("rest");

/** 页眉 Hero 实时状态（eyenurse://tick，core.rs 每拍下发） */
interface TickPayload {
  remaining_ms: number;
  paused: boolean;
  resting: boolean;
}
const tick = ref<TickPayload | null>(null);

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

/** 锁屏保存管线（v3.2）：与主设置同款防抖；复用 saving 顶栏提示 */
function scheduleLockSave(): void {
  if (lockSaveTimer !== null) clearTimeout(lockSaveTimer);
  lockSaveTimer = setTimeout(() => {
    lockSaveTimer = null;
    void flushLockSave();
  }, SAVE_DEBOUNCE_MS);
}

async function flushLockSave(): Promise<void> {
  if (lockSaveTimer !== null) {
    clearTimeout(lockSaveTimer);
    lockSaveTimer = null;
  }
  saving.value = true;
  try {
    const applied = await invoke<LockSettingsDto>("set_lock_settings", {
      dto: { ...toRaw(lockSettings) },
    });
    Object.assign(lockSettings, applied);
    flashSaved();
  } catch (e) {
    console.error("set_lock_settings 失败", e);
  } finally {
    saving.value = false;
  }
}

function flashSaved(): void {
  justSaved.value = true;
  if (savedFadeTimer !== null) clearTimeout(savedFadeTimer);
  savedFadeTimer = setTimeout(() => (justSaved.value = false), 1600);
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
    flashSaved();
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
    flashSaved();
  } catch (e) {
    console.error("reset_settings 失败", e);
  } finally {
    saving.value = false;
  }
}

/** 立刻休息：与托盘「立刻休息」同一动作入口（CoreMsg::Tray(RestNow)） */
async function restNow(): Promise<void> {
  try {
    await invoke("rest_now");
  } catch (e) {
    console.error("rest_now 失败", e);
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

/** 立即锁屏（锁屏功能页大按钮，与托盘同源 CoreMsg::Tray(LockNow)） */
async function lockNow(): Promise<void> {
  try {
    await invoke("lock_now");
  } catch (e) {
    console.error("lock_now 失败", e);
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

/** 锁屏设置加载（v3.2）：启动依赖与 settings-shown 重同步共用 */
async function refreshLockSettings(): Promise<void> {
  if (saving.value) return;
  try {
    Object.assign(lockSettings, await invoke<LockSettingsDto>("get_lock_settings"));
  } catch (e) {
    console.error("get_lock_settings 失败", e);
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
  // 启动依赖一次取全：整包文案 + 语言清单 + 设置（含注册表实测覆盖）+ 锁屏设置
  await Promise.all([
    loadTexts().catch((e) => console.error("get_texts 失败", e)),
    loadLanguageList().catch((e) => console.error("语言清单加载失败", e)),
    refreshFromBackend(),
    refreshLockSettings(),
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
      void refreshLockSettings();
    }),
    await listen("eyenurse://settings-closing", () => {
      // §8.1：未到点的防抖修改立即 flush；无编辑不发（避免无谓重启计时器）
      if (saveTimer !== null) void flushSave();
      if (lockSaveTimer !== null) void flushLockSave();
    }),
    await listen<TickPayload>("eyenurse://tick", (e) => {
      tick.value = e.payload;
    }),
  );
});

onUnmounted(() => {
  for (const off of unlisteners.splice(0)) off();
});

/** 侧栏图标（16px 线性，stroke 继承 currentColor；DS3 克制装饰）。
 *  IA（2026-10-10）：上部 = 功能（休息/锁屏/统计/皮肤），左下角 = 设置 */
const navItems = computed(() => [
  { id: "rest" as const, label: t("navRest") },
  { id: "lock" as const, label: t("lockTitle") },
  { id: "stats" as const, label: t("navStats") },
  { id: "skin" as const, label: t("navSkin") },
]);
</script>

<template>
  <div class="app-root">
    <!-- 模块①：侧边栏（方案 C）——模块列表 + 底部（保存提示槽 / 语言 / 版权） -->
    <aside class="sidebar">
      <nav class="nav" :aria-label="t('setting')">
        <button
          v-for="item in navItems"
          :key="item.id"
          type="button"
          class="nav-item"
          :class="{ active: activeModule === item.id }"
          @click="activeModule = item.id"
        >
          <svg
            v-if="item.id === 'rest'"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            aria-hidden="true"
          >
            <circle cx="8" cy="8" r="6.3" />
            <path d="M8 4.5V8l2.4 1.6" />
          </svg>
          <svg
            v-else-if="item.id === 'lock'"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            aria-hidden="true"
          >
            <rect x="3.5" y="7" width="9" height="6" rx="1.5" />
            <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
          </svg>
          <svg
            v-else-if="item.id === 'stats'"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            aria-hidden="true"
          >
            <path d="M2.5 13.5v-5M8 13.5V2.5M13.5 13.5v-8" />
          </svg>
          <svg
            v-else-if="item.id === 'skin'"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            aria-hidden="true"
          >
            <circle cx="8" cy="8" r="6.3" />
            <circle cx="6" cy="6" r=".9" />
            <circle cx="10.2" cy="6.4" r=".9" />
            <circle cx="6.4" cy="10.2" r=".9" />
          </svg>
          {{ item.label }}
        </button>
      </nav>

      <div class="side-foot">
        <!-- 左下角：设置入口（子级导航：休息计划/锁屏/通用/关于；版权链接在关于页） -->
        <button
          type="button"
          class="nav-item"
          :class="{ active: activeModule === 'settings' }"
          @click="activeModule = 'settings'"
        >
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            aria-hidden="true"
          >
            <circle cx="8" cy="8" r="2.2" />
            <path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M12.4 3.6L11 5M5 11l-1.4 1.4" />
          </svg>
          {{ t("setting") }}
        </button>
      </div>
    </aside>

    <!-- 右侧：当前模块页（v-show 保持各页状态与滚动位置，切换零开销） -->
    <main class="pane">
      <div v-show="activeModule === 'rest'" class="module">
        <RestModule :settings="settings" :tick="tick" @rest-now="restNow" @commit="scheduleSave" />
      </div>
      <div v-show="activeModule === 'lock'" class="module">
        <LockModule
          :lock-settings="lockSettings"
          :settings="settings"
          @lock-now="lockNow"
          @lock-commit="scheduleLockSave"
          @commit="scheduleSave"
        />
      </div>
      <div v-show="activeModule === 'stats'" class="module">
        <StatsCard />
      </div>
      <div v-show="activeModule === 'skin'" class="module">
        <SkinModule />
      </div>
      <div v-show="activeModule === 'settings'" class="module">
        <SettingsModule
          :settings="settings"
          :languages="languages"
          :current-language="currentLanguage"
          :saved="justSaved"
          @commit="scheduleSave"
          @reset="resetDefaults"
          @change-language="changeLanguage"
        />
      </div>
    </main>
  </div>
</template>

<style scoped>
.app-root {
  display: flex;
  height: 100vh;
  background: var(--bg);
}

/* —— 模块①：侧边栏 —— */
.sidebar {
  width: 172px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3) var(--sp-2);
  background: var(--surface);
  border-right: 1px solid var(--border);
}
.nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 34px;
  padding: 0 10px;
  border: none;
  border-radius: var(--r-md);
  background: transparent;
  font-family: inherit;
  font-size: var(--fs-body);
  font-weight: var(--fw-medium);
  color: var(--text-2);
  text-align: left;
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease),
    color var(--dur-fast) var(--ease);
}
.nav-item svg {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}
.nav-item:hover {
  background: var(--surface-2);
  color: var(--text);
}
.nav-item.active {
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-weight: var(--fw-semibold);
}
.side-foot {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-2) 10px 2px;
  border-top: 1px solid var(--border);
}

/* —— 右侧模块页容器 —— */
.pane {
  flex: 1 1 0%;
  min-width: 0;
  overflow-y: auto;
}
.module {
  min-height: 100%;
}
</style>
