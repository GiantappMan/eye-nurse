// 设置项 DTO（实现者 D 所有）：字段与 crates/eyenurse-app/src/commands.rs 的
// `SettingsDto` 一一对应（serde 按字段名输出 PascalCase，与 Setting.json 同形）。
// 文本字段是 .NET "c" 格式字符串（HH:mm:ss）；parse + clamp 在命令侧（清单 I4），
// 页面不做数值解析。
export interface SettingsDto {
  RestInterval: string;
  RestDuration: string;
  ResetTimeout: string;
  /** get = 注册表实测值（Q10/F10）；set = 开关值 */
  RunWhenStarts: boolean;
  ResetWhenSessionUnlock: boolean;
}

/** 空白 DTO（页面加载完成前的占位，加载后经 Object.assign 合并，v2 同款） */
export function emptySettings(): SettingsDto {
  return {
    RestInterval: "",
    RestDuration: "",
    ResetTimeout: "",
    RunWhenStarts: false,
    ResetWhenSessionUnlock: false,
  };
}

/** 锁屏设置 DTO（v3.2 仿锁屏；字段与 commands.rs 的 LockSettingsDto 一一对应） */
export interface LockSettingsDto {
  Enabled: boolean;
  InterceptWinL: boolean;
  CaptureThrough: boolean;
  AutoRest: boolean;
  Password: string;
}

/** 空白锁屏 DTO（全关 + 无密码） */
export function emptyLockSettings(): LockSettingsDto {
  return {
    Enabled: false,
    InterceptWinL: false,
    CaptureThrough: false,
    AutoRest: false,
    Password: "",
  };
}

/** 护眼统计单日行（get_stats 返回；秒数口径，缺失天由后端补零） */
export interface StatsDay {
  date: string;
  work_secs: number;
  rest_secs: number;
  rest_count: number;
  lock_secs: number;
}

// —— v3.1 换肤 ——

/** 皮肤清单条目（get_skins；id = 内置 id / 磁盘文件名或目录名 / "custom"，
 * 名称由前端按语言二选一，colors = 该皮肤逐键回退后的完整配色，供迷你预览直接
 * 渲染。ESP-1：has_bar/has_lock = 目录包声明的 HTML 表面徽标；has_preview =
 * 包内 preview.png 存在（预览 URL = eyenurse-skin 协议 host + /<id>/preview.png）） */
export interface SkinMeta {
  id: string;
  name: string;
  name_en: string;
  /** 包内皮肤（Tier1 = manifest.author）；内置包由后端填「巨应君」，缺省为空 */
  author?: string;
  colors: SkinColors;
  has_bar: boolean;
  has_lock: boolean;
  has_preview: boolean;
}

/** ESP-1 皮肤包 preview.png 的 URL（eyenurse-skin 协议；仅 has_preview 时有效） */
export function skinPreviewUrl(id: string): string {
  return `http://eyenurse-skin.localhost/${id}/preview.png`;
}

/** 任意皮肤 id 的预览表面（get_skin_preview；解析规则与生效皮肤一致——
 * lock 恒有（回退 classic 包），bar null = GDI 原生（前端以 CSS 仿样预览）） */
export interface SkinPreviewDto {
  bar_url: string | null;
  lock_url: string | null;
  bar_w: number;
  bar_h: number;
}

/** get_skins 回包 */
export interface SkinsDto {
  current: string;
  skins: SkinMeta[];
}

/** 生效配色（get_skin / skin-changed；字段名 = theme.json 键名 = CSS 变量去前缀） */
export interface SkinColors {
  name: string;
  name_en: string;
  bar_progress_bg: string;
  bar_progress_fill: string;
  bar_progress_from: string;
  bar_progress_to: string;
  bar_text_idle: string;
  bar_text_from: string;
  bar_text_to: string;
  bar_check_glyph: string;
  bar_check_hover_bg: string;
  mask_bg: string;
  mask_fg: string;
  mask_error: string;
}

/** 自定义配色载荷（set_custom_skin；嵌套结构与 Rust SkinDef 一致，缺省键回退经典） */
export interface SkinDefPayload {
  name?: string | null;
  name_en?: string | null;
  bar: {
    progress_bg?: string | null;
    progress_fill?: string | null;
    progress_from?: string | null;
    progress_to?: string | null;
    text_idle?: string | null;
    text_from?: string | null;
    text_to?: string | null;
    check_glyph?: string | null;
    check_hover_bg?: string | null;
  };
  mask: {
    bg?: string | null;
    fg?: string | null;
    error?: string | null;
  };
}
