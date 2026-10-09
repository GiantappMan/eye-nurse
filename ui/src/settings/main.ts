// 设置窗页面入口（实现者 D 所有；tauri 架构文档 §8.2）：
// 挂载 + 配色真源注入；视图与保存管线在 App.vue 及其子组件。
import { createApp } from "vue";
import { applyThemeVars } from "../shared/theme";
import "../shared/design.css";
import App from "./App.vue";

// theme.json → :root CSS 变量（bar_xxx → --eyenurse-bar-xxx，§3 单一配色真源）
applyThemeVars();

createApp(App).mount("#app");
