//! 护眼时间统计（v3.2 新增）：按天累计 工作/休息/锁屏 三桶秒数 + 休息次数。
//!
//! 存储 `%LOCALAPPDATA%\EyeNurse\UserData\Stats.json`（与配置同目录）；
//! CoreLoop 1Hz 入账（内存累加），脏标记 + 周期落盘（60s）与关键事件
//! （休息开始/结束、解锁、退出）即时落盘，坏 JSON 走默认值（对齐 config 容错）。
//!
//! 日期键 `YYYY-MM-DD`（本地时区，Windows 走 GetLocalTime）；跨天归属规则：
//! 每一拍计入"当下"的日期，跨天自然切开，不做回溯分摊。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 单日统计（serde PascalCase，与其余配置文件风格一致）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct DayStatJson {
    /// 计时运行中（未休息/未暂停/未锁屏）的秒数
    pub WorkSecs: u64,
    /// 休息遮罩显示中的秒数
    pub RestSecs: u64,
    /// 休息会话次数（会话开始时计，0 秒会话也计）
    pub RestCount: u32,
    /// 仿锁屏激活中（未计休息）的秒数
    pub LockSecs: u64,
}

/// Stats.json 根对象（Days 键有序 = 日期有序，BTreeMap）
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct StatsJson {
    pub Days: BTreeMap<String, DayStatJson>,
}

/// 保留天数上限（写入前裁剪，防无限膨胀）
const KEEP_DAYS: usize = 90;

impl StatsJson {
    fn day_mut(&mut self, day: &str) -> &mut DayStatJson {
        self.Days.entry(day.to_string()).or_default()
    }

    /// 1Hz 入账：三桶各加 n 秒（全零不产生脏数据，调用方自行判断）
    pub fn add_tick(&mut self, day: &str, work: u64, rest: u64, lock: u64) {
        if (work, rest, lock) == (0, 0, 0) {
            return;
        }
        let d = self.day_mut(day);
        d.WorkSecs += work;
        d.RestSecs += rest;
        d.LockSecs += lock;
    }

    pub fn add_rest_session(&mut self, day: &str) {
        self.day_mut(day).RestCount += 1;
    }

    /// 按键（=日期序）保留最近 keep 天
    pub fn prune(&mut self, keep: usize) {
        while self.Days.len() > keep {
            let oldest = self.Days.keys().next().expect("len > keep > 0").clone();
            self.Days.remove(&oldest);
        }
    }

    /// 近 n 天报表（含 today，旧→新；无记录的天补零，页面无需对齐缺失键）。
    /// 天数跨月/跨年由 civil 历法换算（不依赖时区库）。
    pub fn last_days(&self, n: u32, today: &str) -> Vec<StatsDay> {
        let (y, m, d) = parse_ymd(today);
        let today_abs = days_from_civil(y, m, d);
        (0..n as i64)
            .map(|i| {
                let (cy, cm, cd) = civil_from_days(today_abs - (n as i64 - 1 - i));
                let date = format!("{cy:04}-{cm:02}-{cd:02}");
                let stat = self.Days.get(&date).copied().unwrap_or_default();
                StatsDay {
                    date,
                    work_secs: stat.WorkSecs,
                    rest_secs: stat.RestSecs,
                    rest_count: stat.RestCount,
                    lock_secs: stat.LockSecs,
                }
            })
            .collect()
    }
}

/// 报表行（camelCase 序列化，前端字段直读）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StatsDay {
    pub date: String,
    pub work_secs: u64,
    pub rest_secs: u64,
    pub rest_count: u32,
    pub lock_secs: u64,
}

/// 本地今天 `YYYY-MM-DD`（Windows GetLocalTime；其余平台回退 UTC）
pub fn local_today() -> String {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY：无副作用系统调用，返回值按值拷贝
        let st = unsafe { GetLocalTime() };
        format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
        format!("{y:04}-{m:02}-{d:02}")
    }
}

/// Stats.json 存取 + 内存态（CoreLoop 独占写者）
pub struct StatsStore {
    path: PathBuf,
    pub data: StatsJson,
    dirty: bool,
}

impl StatsStore {
    /// dir = ConfigStore::dir()（UserData）；缺文件/坏 JSON → 空统计
    pub fn new(dir: &Path) -> Self {
        let path = dir.join("Stats.json");
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| {
                serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text)).ok()
            })
            .unwrap_or_default();
        Self { path, data, dirty: false }
    }

    pub fn add_tick(&mut self, day: &str, work: u64, rest: u64, lock: u64) {
        self.data.add_tick(day, work, rest, lock);
        self.dirty = true;
    }

    pub fn add_rest_session(&mut self, day: &str) {
        self.data.add_rest_session(day);
        self.dirty = true;
    }

    /// 落盘（不脏则空操作）；目录不存在则创建；失败保脏待下轮重试
    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        self.data.prune(KEEP_DAYS);
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string(&self.data) {
            if std::fs::write(&self.path, json).is_ok() {
                self.dirty = false;
            }
        }
    }
}

/// `YYYY-MM-DD` → (y, m, d)；坏格式回 (1970,1,1)（只影响报表起点，不入账路径）
fn parse_ymd(s: &str) -> (i64, i64, i64) {
    let b = s.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {
        if let (Ok(y), Ok(m), Ok(d)) = (
            s[..4].parse::<i64>(),
            s[5..7].parse::<i64>(),
            s[8..10].parse::<i64>(),
        ) {
            return (y, m, d);
        }
    }
    (1970, 1, 1)
}

/// 民历日 → 天数（1970-01-01 = 0；Howard Hinnant days_from_civil）
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // 3月=0 … 2月=11
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 天数 → 民历日（civil_from_days，days_from_civil 的逆）
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每测试独立的临时目录
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eyenurse-stats-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn civil_roundtrip_and_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 10, 9), 20_735);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_735), (2026, 10, 9));
        // 跨月/闰年往返
        for z in [0i64, 31, 365, 366, 19_723, 20_701] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z, "z={z}");
        }
    }

    #[test]
    fn add_tick_buckets_and_zero_is_noop() {
        let mut s = StatsJson::default();
        s.add_tick("2026-10-09", 60, 0, 0);
        s.add_tick("2026-10-09", 0, 30, 0);
        s.add_tick("2026-10-09", 0, 0, 5);
        assert_eq!(
            s.Days.get("2026-10-09"),
            Some(&DayStatJson {
                WorkSecs: 60,
                RestSecs: 30,
                RestCount: 0,
                LockSecs: 5
            })
        );
        // 全零不入账（不产生空天）
        s.add_tick("2026-10-09", 0, 0, 0);
        s.add_tick("2026-10-10", 0, 0, 0);
        assert_eq!(s.Days.len(), 1);
    }

    #[test]
    fn last_days_fills_gaps_in_order() {
        let mut s = StatsJson::default();
        s.add_tick("2026-10-08", 0, 120, 0);
        s.add_rest_session("2026-10-08");
        let days = s.last_days(3, "2026-10-09");
        assert_eq!(days.len(), 3);
        assert_eq!(days[0].date, "2026-10-07");
        assert_eq!(days[0].rest_secs, 0); // 缺失天补零
        assert_eq!(days[1].date, "2026-10-08");
        assert_eq!(days[1].rest_secs, 120);
        assert_eq!(days[1].rest_count, 1);
        assert_eq!(days[2].date, "2026-10-09"); // 今天在末尾
    }

    #[test]
    fn store_roundtrip_prune_and_dirty() {
        let dir = temp_dir("store");
        {
            let mut st = StatsStore::new(&dir);
            assert!(!st.dirty);
            st.save(); // 不脏 → 不落盘（文件不存在）
            assert!(!dir.join("Stats.json").exists());
            st.add_tick("2026-01-01", 1, 2, 3);
            st.save();
            assert!(dir.join("Stats.json").exists());
        }
        // 重开读回
        let mut st = StatsStore::new(&dir);
        assert_eq!(st.data.Days.get("2026-01-01").map(|d| d.RestSecs), Some(2));
        // 90 天裁剪：塞 100 天 → 保存后剩最近 90 天
        for i in 0..100u64 {
            st.add_tick(&format!("2026-{:02}-01", 1 + i / 28), i, 0, 0);
        }
        st.save();
        let text = std::fs::read_to_string(dir.join("Stats.json")).unwrap();
        let reloaded: StatsJson = serde_json::from_str(&text).unwrap();
        assert!(reloaded.Days.len() <= 90);
        std::fs::remove_dir_all(&dir).ok();
    }
}
