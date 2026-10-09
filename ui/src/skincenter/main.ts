// 皮肤中心窗口页面入口（ESP-1）：挂载 + 配色真源注入；视图在 SkinCenter.vue。
import { createApp } from "vue";
import { applyThemeVars } from "../shared/theme";
import "../shared/design.css";
import SkinCenter from "./SkinCenter.vue";

// theme.json → :root CSS 变量（bar_xxx → --eyenurse-bar-xxx，§3 单一配色真源）
applyThemeVars();

createApp(SkinCenter).mount("#app");
