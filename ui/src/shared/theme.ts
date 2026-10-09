// 配色真源注入（实现者 D 所有；tauri 架构文档 §3 评审修订）：
// Rust（build.rs 生成常量）与页面消费同一个 theme.json，改文件两态自动一致。
// settings 与 mask 两个入口各自调用 applyThemeVars()。
// v3.1 换肤：applySkinVars() 在同一命名规则下注入**运行时**皮肤（Rust 解析回退
// 后的完整配色，字段名与 theme.json 键同名），覆盖静态值；两函数可并存——
// theme.json 提供首帧兜底，get_skin/skin-changed 到达后即被皮肤覆盖。
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

/** 运行时皮肤 → :root CSS 变量（SkinColors 字段名与 theme.json 键同名，注入规则相同） */
export function applySkinVars(skin: Record<string, string>): void {
  for (const [key, value] of Object.entries(skin)) {
    if (typeof value === "string") {
      document.documentElement.style.setProperty(
        `--eyenurse-${key.replaceAll("_", "-")}`,
        value,
      );
    }
  }
}
