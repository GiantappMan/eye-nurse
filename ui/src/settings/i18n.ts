// 设置窗文案层（实现者 D 所有；tauri 架构文档 §4.2/§9）：
// - 初始整包文案经 invoke("get_texts") 获取（current→fallback 合并后的 map）；
// - 语言切换广播 `eyenurse://i18n-changed` 后重取（CoreLoop 在 set_current_language
//   里广播；原生侧托盘/窗标题由 Rust 直接刷新）；
// - 缺键回退：显示 key 原文，与 core I18n::get 的第三级回退一致。
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";

export type TextMap = Record<string, string>;

/** 整包文案（响应式；模板经 t() 消费） */
export const texts = ref<TextMap>({});
/** 可用语言清单（I18n::available()，语言下拉数据源，清单 I5） */
export const languages = ref<string[]>([]);
/** 当前语言（Languages.json CurrentLan，解析规则同 I18n::build） */
export const currentLanguage = ref("");

/** 取文案：zh/en 缺键时回退 key 原文 */
export function t(key: string): string {
  return texts.value[key] ?? key;
}

/** 整包取文案并同步窗口标题（v2 XAML 绑定 app_name 的页面侧等价） */
export async function loadTexts(): Promise<void> {
  texts.value = await invoke<TextMap>("get_texts");
  document.title = texts.value["app_name"] ?? "EyeNurse";
}

/** 语言下拉数据源：可用清单 + 选中值（get_languages + get_current_language） */
export async function loadLanguageList(): Promise<void> {
  const [list, current] = await Promise.all([
    invoke<string[]>("get_languages"),
    invoke<string>("get_current_language"),
  ]);
  languages.value = list;
  currentLanguage.value = current;
}
