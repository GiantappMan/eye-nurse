// 共享 token 桥（实现者 D 所有；tauri 架构文档 §3 评审修订；v3.2 设计系统落地）：
// - 色彩真源 = theme.json（Rust build.rs 生成常量与本文件 import 同一份，改文件
//   两态自动一致）。applyThemeVars/applySkinVars 把它注入 :root 的 --eyenurse-*
//   变量；settings 与 mask/barweb 各入口各自调用。
// - 静态设计 token（字阶/间距/圆角/控件几何）真源 = ui/src/shared/design.css 的
//   CSS 变量；下方 tokens 是**JS 侧只读镜像**，仅供内联样式与 JS 几何计算使用
//   （如设置窗皮肤预览缩放、mask/barweb 壳页的 JS 定位）。改值必须与 design.css
//   同步修改——对照 docs/design-system-v3.md §2 的 token 表，禁止两边各改一边。
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

/**
 * JS 侧设计 token（design.css 同名变量的只读镜像；数值层，单位 px/ms）。
 * CSS 消费优先走 CSS 变量；只有 JS 计算几何时才 import 本对象。
 */
export const tokens = {
  /** 字阶（px）——与 design.css --fs-* 一致；10px 不存在于本系统 */
  fontSize: {
    caption: 11,
    hint: 12,
    body: 13,
    subtitle: 15,
    title: 18,
    display: 40,
    clock: 64,
  },
  /** 间距刻度（px）——4pt 网，只允许取本表值 */
  space: { 1: 4, 2: 8, 3: 12, 4: 16, 5: 20, 6: 24, 7: 32, 8: 40 },
  /** 圆角（px）——全站只有这 4 档 + GDI/条形仿样的 0 */
  radius: { sm: 6, md: 8, lg: 12, full: 999 },
  /** 控件规格（px） */
  control: {
    btnH: 30,
    btnHSm: 28,
    inputH: 30,
    switchW: 40,
    switchH: 22,
    switchKnob: 18,
  },
  /** 动效（ms） */
  duration: { fast: 120, slow: 180 },
  /** 层级 */
  z: { frame: 10, chrome: 20, modal: 50, toast: 60 },
  /** 悬浮条（barweb 壳页 JS 用）：字号 = 条高 × fontRatio，clamp 到 [min,max]；
   *  20px 条 → 13px，与 GDI 条 CreateFontW(-(13*dpi/96)) 同基准 */
  bar: { fontRatio: 0.65, fontMin: 11, fontMax: 32 },
  /** 遮罩壳页 chrome（mask 壳页 JS 用）：逃生按钮 44px 命中区（桌面最小标准）、
   *  内嵌 24px 图标（24 网格 1:1 整数渲染）、贴角 8px、错误提示与密码框中心
   *  定距 28px（= 密码框半高 20 + 间距 8，calc 百分比+px 实现，分辨率无关） */
  mask: { unlockHit: 44, unlockIcon: 24, unlockInset: 8, errGap: 28 },
} as const;
