<template>
  <TransitionRoot
    :show="props.open"
    as="template"
    enter="transition ease-in-out duration-300 transform"
    enterFrom="-translate-x-full"
    enterTo="translate-x-0"
    leave="transition ease-in-out duration-300 transform"
    leaveFrom="translate-x-0"
    leaveTo="-translate-x-full"
  >
    <div
      class="fixed inset-0 z-40 flex w-64 bg-white first-line:shadow-sm ring-1 ring-gray-900 ring-opacity-5"
    >
      <div class="relative max-w-xs w-full pt-5 pb-4 flex-1 flex flex-col">
        <div class="flex-row px-4 flex items-center">
          <a class="flex-1">
            <img
              :src="`/logo.${$i18n.locale}.svg`"
              width="43"
              height="43"
              class="h-8 w-auto"
            />
          </a>
          <svg
            @click="emit('onOpenChanged', false)"
            xmlns="http://www.w3.org/2000/svg"
            class="cursor-pointer rotate-180 mx-auto h-5 w-5"
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
        </div>
        <div class="mt-5 flex-1 h-0 overflow-y-auto">
          <nav class="px-2 space-y-1">
            <RouterLink
              :to="item.href!"
              v-for="item in props.navItems"
              :class="[
                'group border-l-4 px-3 py-2 flex items-center text-sm font-medium',
                item.current
                  ? 'bg-teal-50 border-teal-500 text-teal-700 hover:bg-teal-50 hover:text-teal-700'
                  : 'border-transparent text-gray-900 hover:bg-gray-50 hover:text-gray-900',
              ]"
            >
              <component
                :is="item.icon"
                :class="[
                  item.current
                    ? 'text-teal-500 group-hover:text-teal-500'
                    : 'text-gray-400 group-hover:text-gray-500',
                  'flex-shrink-0 -ml-1 mr-3 h-6 w-6',
                ]"
              />
              <span class="truncate">{{ t(item.textKey) }}</span>
            </RouterLink>
          </nav>
        </div>
        <div class="flex-shrink-0 border-t border-gray-200 p-2">
          <nav>
            <RouterLink
              :to="item.href!"
              v-for="item in props.bottomItems"
              :class="[
                item.current
                  ? 'bg-teal-50 border-teal-500 text-teal-700 hover:bg-teal-50 hover:text-teal-700'
                  : 'border-transparent text-gray-900 hover:bg-gray-50 hover:text-gray-900',
                'group border-l-4 px-3 py-2 flex items-center text-sm font-medium',
              ]"
            >
              <component
                :is="item.icon"
                :class="[
                  item.current
                    ? 'text-teal-500 group-hover:text-teal-500'
                    : 'text-gray-400 group-hover:text-gray-500',
                  'flex-shrink-0 -ml-1 mr-3 h-6 w-6',
                ]"
              />
              <p class="truncate">{{ t(item.textKey) }}</p>
            </RouterLink>
          </nav>
        </div>
      </div>
    </div>
  </TransitionRoot>
  <Toggle :show="!props.open" @click="emit('onOpenChanged', true)"></Toggle>
</template>
<script lang="ts" setup>
import { TransitionRoot } from "@headlessui/vue";
import { useI18n } from "vue-i18n";
import type { NavItemModel } from "../models/NavItemModel";
import { RouterLink } from "vue-router";
import Toggle from "./Toggle.vue";

type PropsType = {
  navItems: NavItemModel[];
  bottomItems: NavItemModel[];
  open: boolean | undefined;
};

type EmitsType = {
  (e: "onOpenChanged", value: boolean): void;
};

const props = defineProps<PropsType>();
const emit = defineEmits<EmitsType>();
const { t } = useI18n();
</script>
