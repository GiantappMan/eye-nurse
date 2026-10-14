<script setup lang="ts">
// 设置模块（2026-10-10 IA 二次调整）：休息计划/锁屏配置已迁回各自功能页
// （RestModule/LockModule 原位直改），本页只留通用（开机启动/语言/恢复默认/
// 更新）与关于——一页平铺，不再设子级导航。两套保存管线中本页只剩主设置
// （commit），锁屏管线（lock-commit）整体在 LockModule；均 300ms 防抖（清单 I3）。
// 「关于」承载版本信息与作者站点链接（open_url 公网校验）。
// 「更新」卡自成一管（v3.3 应用内更新）：Update.json 即时保存不走防抖，
// 进度经 eyenurse://app-update 事件实时刷新（app_update 家族模式）。
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import type { AppUpdateEvent, AppUpdateState, SettingsDto, UpdateConfigDto } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";

const props = defineProps<{
  /** 主设置草稿 */
  settings: SettingsDto;
  /** 可用语言清单（语言下拉数据源） */
  languages: string[];
  /** 当前语言 */
  currentLanguage: string;
  /** 保存成功提示（页头固定槽） */
  saved: boolean;
}>();
const emit = defineEmits<{
  /** 主设置任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "commit"): void;
  /** 恢复默认（两步确认后触发） */
  (e: "reset"): void;
  /** 切换语言 */
  (e: "change-language", lan: string): void;
}>();

/** 应用版本（tauri.conf.json version；非 Tauri 环境回退清单值） */
const appVersion = ref("3.0.0");
onMounted(async () => {
  try {
    appVersion.value = (await getVersion()) || appVersion.value;
  } catch {
    /* 非 Tauri 环境保持回退值 */
  }
});

// —— 更新卡（v3.3 应用内更新）——

const upd = ref<AppUpdateState>({
  channel: "stable",
  autoDownload: true,
  phase: "idle",
  percent: 0,
  info: null,
  error: null,
  downloadedVersion: null,
});
const updCfg = ref<UpdateConfigDto>({ channel: "stable", autoDownload: true });
/** 刚检查完且无更新时显示「已是最新版本」（事件/重开页后自动消退） */
const justChecked = ref(false);
const unlisteners: Array<() => void> = [];

onMounted(async () => {
  try {
    upd.value = await invoke<AppUpdateState>("get_update_state");
    updCfg.value = await invoke<UpdateConfigDto>("get_update_config");
  } catch (e) {
    console.error("get_update_* 失败", e);
  }
  unlisteners.push(
    await listen<AppUpdateEvent>("eyenurse://app-update", ({ payload }) => {
      applyEvent(payload);
    }),
  );
});
onUnmounted(() => unlisteners.forEach((fn) => fn()));

function applyEvent(ev: AppUpdateEvent): void {
  const s = upd.value;
  switch (ev.state) {
    case "checking":
      s.phase = "checking";
      break;
    case "upToDate":
      s.phase = "idle";
      s.info = null;
      s.error = null;
      justChecked.value = true;
      break;
    case "available":
      s.phase = "available";
      s.error = null;
      s.info = { version: ev.version, notes: ev.notes, date: "", channel: ev.channel };
      break;
    case "progress":
      s.phase = "downloading";
      s.percent = ev.percent;
      break;
    case "downloaded":
      s.phase = "downloaded";
      s.percent = 100;
      s.downloadedVersion = ev.version;
      if (s.info && !s.info.version) s.info.version = ev.version;
      break;
    case "error":
      s.phase = "error";
      s.error = ev.message;
      break;
  }
}

/** 更新通道关闭时检查/下载入口整体禁用 */
const channelOff = computed(() => updCfg.value.channel === "off");
const busy = computed(
  () => upd.value.phase === "checking" || upd.value.phase === "downloading",
);

const actionLabel = computed(() => {
  const p = upd.value.phase;
  if (p === "downloaded") return t("installRestart");
  if (p === "available") return t("downloadUpdate");
  return t("checkUpdate");
});

async function onUpdateAction(): Promise<void> {
  if (channelOff.value || busy.value) return;
  justChecked.value = false;
  try {
    if (upd.value.phase === "downloaded") {
      // 程序随即走托盘退出语义退出并由安装器重启，无需刷新本页
      await invoke("install_update");
      return;
    }
    if (upd.value.phase === "available") {
      upd.value = await invoke<AppUpdateState>("download_update");
      return;
    }
    upd.value = await invoke<AppUpdateState>("check_update");
  } catch (e) {
    upd.value = { ...upd.value, phase: "error", error: String(e) };
  }
}

async function saveUpdateConfig(): Promise<void> {
  try {
    await invoke("set_update_config", {
      dto: { channel: updCfg.value.channel, autoDownload: updCfg.value.autoDownload },
    });
  } catch (e) {
    console.error("set_update_config 失败", e);
  }
}

const statusText = computed(() => {
  const s = upd.value;
  if (channelOff.value) return t("updateChannelOffHint");
  if (s.phase === "checking") return t("updateChecking");
  if (s.phase === "downloading") return t("updateDownloading").replace("{percent}", String(Math.round(s.percent)));
  if (s.phase === "downloaded") return t("updateDownloaded").replace("{version}", s.downloadedVersion ?? "");
  if (s.phase === "available") return t("updateAvailable").replace("{version}", s.info?.version ?? "");
  if (s.phase === "error") return s.error ?? t("updateErrorHint");
  if (justChecked.value) return t("updateUpToDate");
  return t("updateHint").replace("{version}", appVersion.value);
});

const notesText = computed(() =>
  upd.value.phase === "available" ? upd.value.info?.notes.trim() : "",
);

// —— 恢复默认：两步确认（3s 内二次点击生效），替代 confirm() 弹窗 ——
const confirmReset = ref(false);
let confirmTimer: ReturnType<typeof setTimeout> | null = null;
function onReset(): void {
  if (!confirmReset.value) {
    confirmReset.value = true;
    if (confirmTimer !== null) clearTimeout(confirmTimer);
    confirmTimer = setTimeout(() => (confirmReset.value = false), 3000);
    return;
  }
  if (confirmTimer !== null) clearTimeout(confirmTimer);
  confirmReset.value = false;
  emit("reset");
}

/** 作者站点（关于；与侧栏原左下角链接同址，走 open_url 公网校验） */
function openAuthorSite(): void {
  invoke("open_url", { url: "https://mscoder.cn" }).catch((e) =>
    console.error("open_url 失败", e),
  );
}
</script>

<template>
  <div class="page">
    <div class="page-head">
      <div>
        <h2 class="page-title">{{ t("setting") }}</h2>
        <p class="page-hint">{{ t("modifyAutosave") }}</p>
      </div>
      <!-- 保存 chip 固定槽位：出现/消失不推动标题（§1.5） -->
      <div class="saved-slot" role="status" aria-live="polite">
        <Transition name="fade">
          <span v-if="saved" class="saved-chip">{{ t("saved") }}</span>
        </Transition>
      </div>
    </div>

    <section class="en-card">
      <h3 class="en-card-title">{{ t("navGeneral") }}</h3>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("runWhenStarts") }}</p>
        </div>
        <SwitchToggle v-model="settings.RunWhenStarts" @update:model-value="$emit('commit')" />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("language") }}</p>
        </div>
        <select
          class="lang"
          :value="currentLanguage"
          :aria-label="t('language')"
          @change="$emit('change-language', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="lan in languages" :key="lan" :value="lan">{{ t(lan) }}</option>
        </select>
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-hint">{{ t("restoreHint") }}</p>
        </div>
        <button
          type="button"
          class="en-btn en-btn-ghost"
          :class="{ 'is-danger': confirmReset }"
          @click="onReset"
        >
          {{ confirmReset ? t("confirmRestore") : t("restoreDefaults") }}
        </button>
      </div>
    </section>

    <section class="en-card">
      <h3 class="en-card-title">{{ t("updateSection") }}</h3>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ statusText }}</p>
          <p v-if="notesText" class="en-row-hint">{{ notesText }}</p>
        </div>
        <button
          type="button"
          class="en-btn"
          :class="upd.phase === 'downloaded' ? 'en-btn-primary' : 'en-btn-ghost'"
          :disabled="channelOff || busy"
          @click="onUpdateAction"
        >
          {{ actionLabel }}
        </button>
      </div>
      <div v-if="upd.phase === 'downloading'" class="en-row">
        <div
          class="upd-progress"
          role="progressbar"
          :aria-valuenow="Math.round(upd.percent)"
          aria-valuemin="0"
          aria-valuemax="100"
        >
          <div class="upd-progress-fill" :style="{ width: upd.percent + '%' }"></div>
        </div>
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("autoDownload") }}</p>
        </div>
        <SwitchToggle v-model="updCfg.autoDownload" :disabled="channelOff" @update:model-value="saveUpdateConfig" />
      </div>
      <div class="en-row">
        <div class="row-main">
          <p class="en-row-title">{{ t("updateChannel") }}</p>
        </div>
        <select
          class="lang"
          v-model="updCfg.channel"
          :aria-label="t('updateChannel')"
          @change="saveUpdateConfig"
        >
          <option value="stable">{{ t("channelStable") }}</option>
          <option value="preview">{{ t("channelPreview") }}</option>
          <option value="off">{{ t("channelOff") }}</option>
        </select>
      </div>
    </section>

    <section class="about">
      <span class="about-logo" aria-hidden="true"></span>
      <h3 class="about-name">{{ t("app_name") }}</h3>
      <p class="about-ver">v{{ appVersion }}</p>
      <p class="about-copy">
        <a
          class="about-link"
          href="https://mscoder.cn"
          title="mscoder.cn"
          @click.prevent="openAuthorSite"
          v-html="t('copyright')"
        ></a>
      </p>
    </section>
  </div>
</template>

<style scoped>
.page {
  max-width: 640px;
  margin: 0 auto;
  padding: var(--sp-5) var(--sp-5) var(--sp-4);
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  min-height: 100%;
}
.page-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  margin-bottom: var(--sp-1);
}
.page-title {
  margin: 0;
  font-size: var(--fs-subtitle);
  font-weight: var(--fw-semibold);
}
.page-hint {
  margin: 2px 0 0;
  font-size: var(--fs-hint);
  color: var(--text-2);
}
/* 保存 chip 固定槽位（§5.1.2）：恒占高度 */
.saved-slot {
  height: 22px;
  display: flex;
  align-items: center;
}
.saved-chip {
  font-size: var(--fs-hint);
  font-weight: var(--fw-medium);
  color: var(--accent-strong);
  background: var(--accent-soft);
  padding: 3px 10px;
  border-radius: var(--r-full);
  white-space: nowrap;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--dur-slow) var(--ease);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
.row-main {
  min-width: 0;
}
/* 更新下载进度（§5.3 显性进度）：细条 + 品牌色填充 */
.upd-progress {
  flex: 1;
  height: 4px;
  border-radius: var(--r-full);
  background: var(--surface-2);
  overflow: hidden;
}
.upd-progress-fill {
  height: 100%;
  border-radius: var(--r-full);
  background: var(--accent);
  transition: width var(--dur-fast) var(--ease);
}
.lang {
  appearance: none;
  height: var(--control-h-sm);
  padding: 0 24px 0 12px;
  font-size: var(--fs-hint);
  font-family: inherit;
  color: var(--text-2);
  background-color: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  cursor: pointer;
  background-image: linear-gradient(45deg, transparent 50%, var(--text-2) 50%),
    linear-gradient(135deg, var(--text-2) 50%, transparent 50%);
  background-position:
    calc(100% - 14px) calc(50% - 1px),
    calc(100% - 9px) calc(50% - 1px);
  background-size: 5px 5px;
  background-repeat: no-repeat;
  transition:
    color var(--dur-fast) var(--ease),
    border-color var(--dur-fast) var(--ease);
}
.lang:hover {
  color: var(--text);
  border-color: var(--accent);
}
/* 关于 */
.about {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-6) 0 var(--sp-5);
  text-align: center;
}
.about-logo {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  margin-bottom: var(--sp-2);
  background: radial-gradient(circle at 50% 50%, var(--accent) 0 10px, transparent 10px);
  border: 4px solid var(--accent);
}
.about-name {
  margin: 0;
  font-size: var(--fs-title);
  font-weight: var(--fw-semibold);
}
.about-ver {
  margin: 0;
  font-family: var(--font-num);
  font-size: var(--fs-hint);
  color: var(--text-3);
}
.about-copy {
  margin: var(--sp-2) 0 0;
  font-size: var(--fs-hint);
  color: var(--text-3);
}
.about-link {
  color: var(--text-2);
  text-decoration: none;
  transition:
    color var(--dur-fast) var(--ease),
    text-decoration-color var(--dur-fast) var(--ease);
}
.about-link:hover {
  color: var(--accent-strong);
  text-decoration: underline;
}
.about-link:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
  border-radius: var(--r-sm);
}
</style>
