<script setup lang="ts">
// 设置窗根组件（v3.1 重设计）。
//
// 布局：放弃 v2 的可折叠侧栏 + 空页头条，改为「顶栏（品牌 + 保存态 + 语言切换）
// + 单列内容」；唯一页面即设置页（v2 的 home 占位页无存在价值，一并去除）。
// 数据管线与事件接线保持不变：
//   控件提交 → 300ms 防抖 → invoke("set_settings") → 回包整体合并；
//   i18n-changed → 重取文案；settings-changed → 非保存中重取设置；
//   settings-shown → 重同步注册表实测勾选态；settings-closing → flush 防抖保存；
//   eyenurse://tick → 页眉 Hero 实时倒计时/状态（core.rs 每拍下发）。
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { onMounted, onUnmounted, reactive, ref, toRaw } from "vue";
import { currentLanguage, languages, loadLanguageList, loadTexts, t } from "./i18n";
import { emptyLockSettings, emptySettings, type LockSettingsDto, type SettingsDto } from "./types";
import SettingsView from "./components/SettingsView.vue";

/** 防抖窗口（v2 Settings.vue：lodash/debounce 300ms，清单 I3） */
const SAVE_DEBOUNCE_MS = 300;

// —— 草稿（页面加载后经 Object.assign 填充；加载失败保持空白占位）——
const settings = reactive<SettingsDto>(emptySettings());
const saving = ref(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

/** 锁屏草稿（v3.2）：独立文件 LockScreen.json，保存管线与主设置同款 300ms 防抖 */
const lockSettings = reactive<LockSettingsDto>(emptyLockSettings());
let lockSaveTimer: ReturnType<typeof setTimeout> | null = null;

/** 保存成功的短暂提示（顶栏 ✓，1.6s 后淡出） */
const justSaved = ref(false);
let savedFadeTimer: ReturnType<typeof setTimeout> | null = null;

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
</script>

<template>
  <div class="app-root">
    <!-- 顶栏：品牌 + 保存成功提示 + 语言切换 -->
    <header class="topbar">
      <div class="brand">
        <svg class="mark" viewBox="0 0 24 24" aria-hidden="true">
          <path
            d="M12 5C6.5 5 2.6 9.6 1.5 12c1.1 2.4 5 7 10.5 7s9.4-4.6 10.5-7C21.4 9.6 17.5 5 12 5Z"
            class="mark-eye"
          />
          <circle cx="12" cy="12" r="3.2" class="mark-pupil" />
        </svg>
        <span class="brand-name">{{ t("app_name") }}</span>
      </div>
      <div class="top-right">
        <Transition name="fade">
          <span v-if="justSaved" class="saved-chip">✓ {{ t("saved") }}</span>
        </Transition>
        <select
          class="lang"
          :value="currentLanguage"
          :aria-label="t('setting')"
          @change="changeLanguage(($event.target as HTMLSelectElement).value)"
        >
          <option v-for="lan in languages" :key="lan" :value="lan">{{ t(lan) }}</option>
        </select>
      </div>
    </header>

    <main class="content">
      <SettingsView
        :settings="settings"
        :saving="saving"
        :tick="tick"
        :lock-settings="lockSettings"
        @commit="scheduleSave"
        @reset="resetDefaults"
        @rest-now="restNow"
        @lock-commit="scheduleLockSave"
      />
    </main>
  </div>
</template>

<style>
/* —— 设计系统：护眼绿点缀，浅/深随系统 —— */
:root {
  --bg: #f3f5f7;
  --surface: #ffffff;
  --surface-2: #f7f9fa;
  --border: #e4e8ec;
  --text: #1b2129;
  --text-2: #67727e;
  --accent: #0e9f6e;
  --accent-soft: #e2f6ee;
  --accent-strong: #0b7f58;
  --danger: #dc4b4b;
  --danger-soft: #fdeeee;
  --hero-from: #10715a;
  --hero-to: #0e9f6e;
  --hero-text: #f2fbf7;
  --hero-dim: #cfe9de;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #16181c;
    --surface: #1f2227;
    --surface-2: #262a30;
    --border: #30353c;
    --text: #e7eaee;
    --text-2: #99a1ab;
    --accent: #34c48d;
    --accent-soft: #1d332b;
    --accent-strong: #2aa877;
    --danger: #f07171;
    --danger-soft: #3a2426;
    --hero-from: #17453a;
    --hero-to: #1d5c47;
    --hero-text: #eafaf3;
    --hero-dim: #9fc9b8;
    color-scheme: dark;
  }
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
  font-size: 14px;
  color: var(--text);
}
button {
  font-family: inherit;
}
</style>

<style scoped>
.app-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg);
}
.topbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 52px;
  padding: 0 20px;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
}
.brand {
  display: flex;
  align-items: center;
  gap: 9px;
  min-width: 0;
}
.mark {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
}
.mark-eye {
  fill: none;
  stroke: var(--accent);
  stroke-width: 1.8;
}
.mark-pupil {
  fill: var(--accent);
}
.brand-name {
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 0.2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.top-right {
  display: flex;
  align-items: center;
  gap: 12px;
}
.saved-chip {
  font-size: 12px;
  color: var(--accent-strong);
  background: var(--accent-soft);
  padding: 3px 10px;
  border-radius: 999px;
}
.lang {
  appearance: none;
  font-size: 12px;
  color: var(--text-2);
  background: transparent;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 5px 26px 5px 10px;
  cursor: pointer;
  background-image: linear-gradient(45deg, transparent 50%, var(--text-2) 50%),
    linear-gradient(135deg, var(--text-2) 50%, transparent 50%);
  background-position:
    calc(100% - 14px) calc(50% - 1px),
    calc(100% - 9px) calc(50% - 1px);
  background-size: 5px 5px;
  background-repeat: no-repeat;
}
.lang:hover {
  color: var(--text);
  border-color: var(--accent);
}
.content {
  flex: 1 1 0%;
  min-height: 0;
  overflow-y: auto;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
