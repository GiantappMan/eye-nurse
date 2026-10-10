<script setup lang="ts">
// 设置模块（2026-10-10 IA 调整 + 子级导航）：全部配置项归拢于此，按子级导航
// 分四页——休息计划 / 锁屏 / 通用 / 关于。侧栏左下角「设置」进入。
// 两套保存管线并存：主设置（间隔/时长/重置时间/开机启动）→ commit；
// 锁屏（LockScreen.json）→ lock-commit；均为 300ms 防抖（清单 I3）。
// 「关于」页承载版本信息与作者站点链接（原侧栏左下角链接迁入，走 open_url）。
import { computed, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import type { LockSettingsDto, SettingsDto } from "../types";
import { t } from "../i18n";
import SwitchToggle from "./SwitchToggle.vue";

const props = defineProps<{
  /** 主设置草稿 */
  settings: SettingsDto;
  /** 锁屏草稿 */
  lockSettings: LockSettingsDto;
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
  /** 锁屏设置任一控件提交 → 父组件排 300ms 防抖保存 */
  (e: "lock-commit"): void;
  /** 恢复默认（两步确认后触发） */
  (e: "reset"): void;
  /** 切换语言 */
  (e: "change-language", lan: string): void;
}>();

/** 子级导航当前页（默认休息计划） */
const sub = ref<"plan" | "lock" | "general" | "about">("plan");
const SUB_PAGES = computed(() => [
  { id: "plan" as const, label: t("planTitle") },
  { id: "lock" as const, label: t("lockTitle") },
  { id: "general" as const, label: t("navGeneral") },
  { id: "about" as const, label: t("about") },
]);

/** 应用版本（tauri.conf.json version；非 Tauri 环境回退清单值） */
const appVersion = ref("3.0.0");
onMounted(async () => {
  try {
    appVersion.value = (await getVersion()) || appVersion.value;
  } catch {
    /* 非 Tauri 环境保持回退值 */
  }
});

/** 休息间隔预设（分钟） */
const PRESETS = [20, 30, 45, 60, 90];

/** HH:mm:ss 宽松校验：仅控制输入态红框，解析在命令侧（清单 I4） */
function hmsValid(s: string): boolean {
  return /^(\d{1,2}):([0-5]\d):([0-5]\d)$/.test(s.trim());
}

/** 预设命中（当前间隔恰好等于某预设） */
const activePreset = computed(() => {
  const m = /^(\d{1,2}):([0-5]\d):([0-5]\d)$/.exec(props.settings.RestInterval.trim());
  if (!m) return undefined;
  return PRESETS.find((mins) => mins * 60 === Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]));
});

function setPreset(mins: number): void {
  props.settings.RestInterval = `00:${String(mins).padStart(2, "0")}:00`;
  emit("commit");
}

// —— 状态完备（DS3 §4.2）：invalid 红态只在用户触碰（失焦）后出现 ——
const touched = reactive(new Set<string>());

function onHmsBlur(field: string): void {
  touched.add(field);
  emit("commit");
}

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

/** 作者站点（关于页；与侧栏原左下角链接同址，走 open_url 公网校验） */
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

    <!-- 子级导航（设置内部的模块分页） -->
    <nav class="subnav" :aria-label="t('setting')">
      <button
        v-for="p in SUB_PAGES"
        :key="p.id"
        type="button"
        :class="{ active: sub === p.id }"
        @click="sub = p.id"
      >
        {{ p.label }}
      </button>
    </nav>

    <!-- 休息计划 -->
    <div v-show="sub === 'plan'" class="sub">
      <section class="en-card">
        <h3 class="en-card-title">{{ t("planTitle") }}</h3>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("restInterval") }}</p>
            <div class="chips">
              <button
                v-for="m in PRESETS"
                :key="m"
                type="button"
                class="en-chip"
                :class="{ 'is-active': activePreset === m }"
                @click="setPreset(m)"
              >
                {{ m }} {{ t("minutes") }}
              </button>
            </div>
          </div>
          <input
            v-model="settings.RestInterval"
            type="text"
            class="en-input en-input-mono hms"
            :class="{ 'is-invalid': touched.has('RestInterval') && !hmsValid(settings.RestInterval) }"
            placeholder="HH:mm:ss"
            spellcheck="false"
            @blur="onHmsBlur('RestInterval')"
            @keydown.enter="($event.target as HTMLInputElement).blur()"
          />
        </div>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("restDuration") }}</p>
          </div>
          <input
            v-model="settings.RestDuration"
            type="text"
            class="en-input en-input-mono hms"
            :class="{ 'is-invalid': touched.has('RestDuration') && !hmsValid(settings.RestDuration) }"
            placeholder="HH:mm:ss"
            spellcheck="false"
            @blur="onHmsBlur('RestDuration')"
            @keydown.enter="($event.target as HTMLInputElement).blur()"
          />
        </div>
      </section>
    </div>

    <!-- 锁屏 -->
    <div v-show="sub === 'lock'" class="sub">
      <section class="en-card">
        <h3 class="en-card-title">{{ t("lockTitle") }}</h3>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("lockEnable") }}</p>
            <p class="en-row-hint">{{ t("lockEnableHint") }}</p>
          </div>
          <SwitchToggle v-model="lockSettings.Enabled" @update:model-value="$emit('lock-commit')" />
        </div>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("lockWinL") }}</p>
            <p class="en-row-hint">{{ t("lockWinLHint") }}</p>
          </div>
          <SwitchToggle
            v-model="lockSettings.InterceptWinL"
            :disabled="!lockSettings.Enabled"
            @update:model-value="$emit('lock-commit')"
          />
        </div>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("lockCapture") }}</p>
            <p class="en-row-hint">{{ t("lockCaptureHint") }}</p>
          </div>
          <SwitchToggle
            v-model="lockSettings.CaptureThrough"
            :disabled="!lockSettings.Enabled"
            @update:model-value="$emit('lock-commit')"
          />
        </div>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("lockAutoRest") }}</p>
            <p class="en-row-hint">{{ t("lockAutoRestHint") }}</p>
          </div>
          <SwitchToggle
            v-model="lockSettings.AutoRest"
            :disabled="!lockSettings.Enabled"
            @update:model-value="$emit('lock-commit')"
          />
        </div>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("lockPassword") }}</p>
            <p class="en-row-hint">{{ t("lockPasswordHint") }}</p>
          </div>
          <input
            v-model="lockSettings.Password"
            type="password"
            class="en-input pwd"
            :placeholder="t('lockPasswordHint')"
            autocomplete="new-password"
            spellcheck="false"
            @blur="$emit('lock-commit')"
            @keydown.enter="($event.target as HTMLInputElement).blur()"
          />
        </div>
        <div class="en-row">
          <div class="row-main">
            <p class="en-row-title">{{ t("resetWhenSessionUnlock") }}</p>
            <p class="en-row-hint">{{ t("lockResetHint") }}</p>
          </div>
          <div class="row-control">
            <input
              v-model="settings.ResetTimeout"
              type="text"
              class="en-input en-input-mono hms"
              :class="{ 'is-invalid': touched.has('ResetTimeout') && !hmsValid(settings.ResetTimeout) }"
              placeholder="HH:mm:ss"
              spellcheck="false"
              :disabled="!settings.ResetWhenSessionUnlock"
              @blur="onHmsBlur('ResetTimeout')"
              @keydown.enter="($event.target as HTMLInputElement).blur()"
            />
            <SwitchToggle
              v-model="settings.ResetWhenSessionUnlock"
              @update:model-value="$emit('commit')"
            />
          </div>
        </div>
      </section>
    </div>

    <!-- 通用 -->
    <div v-show="sub === 'general'" class="sub">
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
    </div>

    <!-- 关于 -->
    <div v-show="sub === 'about'" class="sub">
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
/* 子级导航：文字标签 + active 底线（DS3 克制装饰） */
.subnav {
  display: flex;
  gap: var(--sp-1);
  border-bottom: 1px solid var(--border);
}
.subnav button {
  height: 34px;
  padding: 0 var(--sp-3);
  border: none;
  background: transparent;
  font-family: inherit;
  font-size: var(--fs-body);
  font-weight: var(--fw-medium);
  color: var(--text-2);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  transition:
    color var(--dur-fast) var(--ease),
    border-color var(--dur-fast) var(--ease);
}
.subnav button:hover {
  color: var(--text);
}
.subnav button.active {
  color: var(--accent-strong);
  font-weight: var(--fw-semibold);
  border-bottom-color: var(--accent);
}
.sub {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}
.row-main {
  min-width: 0;
}
.row-control {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
  margin-top: var(--sp-2);
}
.hms {
  width: 104px; /* HH:mm:ss 定宽（§4.2） */
}
.hms:focus-visible {
  outline: none; /* ring 已由 .en-input:focus 提供 */
}
.pwd {
  width: 180px;
  text-align: left;
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
/* 关于页 */
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
