// 配色真源注入（实现者 D 所有；tauri 架构文档 §3 评审修订）：
// Rust（build.rs 生成常量）与页面消费同一个 theme.json，改文件两态自动一致。
// settings 与 mask 两个入口各自调用 applyThemeVars()。
import theme from "../../../crates/eyenurse-app/src/ui/theme.json";

/** theme.json → :root CSS 变量（bar_xxx → --eyenurse-bar-xxx，供页面样式消费） */
export function applyThemeVars(): void {
  for (const [key, value] of Object.entries(theme)) {
    if (typeof value === "string") {
      document.documentElement.style.setProperty(
        `--eyenurse-${key.replaceAll("_", "-")}`,
        value,
      );
    }
  }
}
