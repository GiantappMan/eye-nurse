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
