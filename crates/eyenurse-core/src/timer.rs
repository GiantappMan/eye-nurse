//! v2 GlobalTimerService 的语义等价状态机（架构文档 §4.2）。
//!
//! v2 是 1 秒系统定时器 + `DateTime.Now` 比较（GlobalTimerService.cs:41-132）。
//! v3 改为纯状态机 + 调用方喂时间（AppModel 以 1Hz 心跳喂，见架构文档 §6.1），
//! 对 v2 UI 可达的调用序列逐条等价（唯一例外是 UI 不可达的“暂停后 Start”，见下
//! start() 的有意偏差 Q11）。
//!
//! **必须由调用方（AppModel）承接的 v2 怪癖**（core 只管状态）：
//! 1. `Stop()` 未启动也会发一次 Elapsed → `stop()` 返回 false 时调用方须再 poll 一次（cs:64）；
//! 2. `Pause()`/`Resume()` 各带一次 CheckTrigger → 调用方调用后立即 poll 并应用（cs:74, 86）；
//! 3. `Reset()` 内部立即 CheckTrigger → 调用方 reset 后必须立即 poll（cs:92）；
//! 5. Trigger 发出后本模块立即 Reset（remaining 立刻回满）。

use std::time::{Duration, Instant};

/// 与 v2 事件顺序一致：先 Elapsed 后 Trigger（CheckTrigger, GlobalTimerService.cs:103-132）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEvent {
    /// v2 Elapsed：每次 poll 都发（next_trigger 为 Some 时），remaining 已 clamp 到 ≥0
    Elapsed { remaining: Duration },
    /// v2 Trigger：`next_trigger - now < 1s` 时发一次，随后自动 reset(now)
    Trigger,
}

/// 对应 v2 ArgumentOutOfRangeException（GlobalTimerService.cs:27-32）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntervalTooShort;

pub struct GlobalTimer {
    interval: Duration,
    next_trigger: Option<Instant>,
    paused_at: Option<Instant>,
    started: bool,
}

impl GlobalTimer {
    /// v2 构造函数：interval < 1s → Err(IntervalTooShort)（GlobalTimerService.cs:27-32）
    pub fn new(interval: Duration) -> Result<Self, IntervalTooShort> {
        if interval < Duration::from_secs(1) {
            return Err(IntervalTooShort);
        }
        Ok(Self {
            interval,
            next_trigger: None,
            paused_at: None,
            started: false,
        })
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// v2 Start()：已启动 → false；否则 reset(now)（触发一次 poll 语义由调用方 poll 消化）
    /// 并置 started=true（cs:41-54）。
    /// 有意偏差（清单 Q11）：v2 的守卫是 `_timer != null && _timer.Enabled`，暂停后调用
    /// Start() 会成功；该序列在 v2 UI 上不可达（NUnit 亦未锁定），v3 选择暂停期间返回 false
    /// （pause 期间 started 保持 true，见 pause()）。
    pub fn start(&mut self, now: Instant) -> bool {
        if self.started {
            return false;
        }
        self.next_trigger = Some(now + self.interval);
        self.started = true;
        true
    }

    /// v2 Stop()：未启动 → 返回 false（**调用方须再 poll 一次**，怪癖 1，core 不代劳）；
    /// 已启动 → 清 next_trigger/paused_at/started 并返回 true（cs:55-66）
    pub fn stop(&mut self, _now: Instant) -> bool {
        if !self.started {
            return false;
        }
        self.next_trigger = None;
        self.paused_at = None;
        self.started = false;
        true
    }

    /// v2 Pause()：未启动或已暂停 → false；否则记 paused_at=now、started 保持
    /// “运行中但冻结”语义并返回 true（cs:67-76）。
    /// 注意：调用方在成功后须立即 poll 一次（怪癖 2）。
    pub fn pause(&mut self, now: Instant) -> bool {
        if !self.started || self.paused_at.is_some() {
            return false;
        }
        self.paused_at = Some(now);
        true
    }

    /// v2 Resume()：未启动或未暂停 → false；否则 next_trigger += now - paused_at，
    /// 清 paused_at，返回 true（cs:77-88）。调用方在成功后须立即 poll 一次（怪癖 2）。
    pub fn resume(&mut self, now: Instant) -> bool {
        if !self.started {
            return false;
        }
        let paused_at = match self.paused_at {
            Some(p) => p,
            None => return false,
        };
        if let Some(next) = self.next_trigger {
            self.next_trigger = Some(next + now.saturating_duration_since(paused_at));
        }
        self.paused_at = None;
        true
    }

    /// v2 Reset()：恒 true；next_trigger = now + interval（cs:89-94）。
    /// 注意 v2 Reset 内部随后立即 CheckTrigger（cs:92 “立即更新”）——v3 的即时 Elapsed
    /// 由调用方在 reset() 后紧跟一次 poll() 完成（怪癖 3）。
    pub fn reset(&mut self, now: Instant) -> bool {
        self.next_trigger = Some(now + self.interval);
        true
    }

    /// v2 CheckTrigger()：每次调用先发 Elapsed（若有 next_trigger）；
    /// 若 `next_trigger - now < 1s`（含已超时）再发 Trigger 并 reset(now)。返回按序事件
    /// （长度 0..=2）。可重入保护在 v3 不需要（单线程调用）。
    pub fn poll(&mut self, now: Instant) -> Vec<TimerEvent> {
        let mut events = Vec::with_capacity(2);
        if let Some(next) = self.next_trigger {
            let remaining = next.saturating_duration_since(now);
            events.push(TimerEvent::Elapsed { remaining });
            if remaining < Duration::from_secs(1) {
                // v2：Trigger 发出后立即 Reset（cs:130-132），remaining 立刻回满
                events.push(TimerEvent::Trigger);
                self.next_trigger = Some(now + self.interval);
            }
        }
        events
    }

    /// 距下次触发的剩余时间（clamp ≥ 0；未启动返回 None）
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.next_trigger
            .map(|next| next.saturating_duration_since(now))
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: Duration = Duration::from_secs(1);
    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn interval_below_one_second_rejected() {
        assert!(GlobalTimer::new(Duration::from_millis(999)).is_err());
        assert!(GlobalTimer::new(SEC).is_ok());
    }

    #[test]
    fn start_twice_second_false() {
        let mut t = GlobalTimer::new(MIN).unwrap();
        assert!(t.start(Instant::now()));
        assert!(!t.start(Instant::now()));
    }

    #[test]
    fn stop_when_not_started_false() {
        let mut t = GlobalTimer::new(MIN).unwrap();
        assert!(!t.stop(Instant::now()));
        assert!(t.remaining(Instant::now()).is_none());
    }

    #[test]
    fn pause_blocks_trigger_and_resume_shifts_next() {
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(MIN).unwrap();
        assert!(!t.pause(t0)); // 未启动 → false
        assert!(t.start(t0));
        assert!(t.pause(t0 + 10 * SEC));
        assert!(t.is_paused());
        // 暂停期间不 poll（AppModel 仅在 pause/resume 转换的当下各 poll 一次）：
        // 暂停瞬间 remaining = 50s
        assert_eq!(t.remaining(t0 + 10 * SEC), Some(50 * SEC));
        // resume 后 next 顺延暂停时长（暂停 10s → next 从 t0+60s 顺延到 t0+70s）
        assert!(t.resume(t0 + 20 * SEC));
        assert!(!t.is_paused());
        let r1 = t.remaining(t0 + 21 * SEC).unwrap();
        let r2 = t.remaining(t0 + 22 * SEC).unwrap();
        assert_eq!(r1, 49 * SEC);
        assert!(r1 > r2); // remaining 单调递减
        // 已在暂停中再 pause → false；未暂停时 resume → false
        assert!(t.pause(t0 + 22 * SEC));
        assert!(!t.pause(t0 + 22 * SEC));
        assert!(t.resume(t0 + 23 * SEC));
        assert!(!t.resume(t0 + 23 * SEC));
    }

    #[test]
    fn reset_restores_and_immediate_poll_yields_full_interval() {
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(MIN).unwrap();
        assert!(t.start(t0));
        assert!(t.reset(t0 + 30 * SEC)); // 恒 true
        assert_eq!(t.remaining(t0 + 30 * SEC), Some(MIN));
        // 怪癖 3：reset 后紧接一次 poll 产出 Elapsed{interval}
        assert_eq!(
            t.poll(t0 + 30 * SEC),
            vec![TimerEvent::Elapsed { remaining: MIN }]
        );
    }

    #[test]
    fn trigger_when_less_than_one_second_then_auto_reset() {
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(MIN).unwrap();
        t.start(t0);
        // 喂到剩余 0.5s：先 Elapsed 后 Trigger，且 Trigger 发出后立即 Reset（以当下为基准）
        let evs = t.poll(t0 + MIN - Duration::from_millis(500));
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0], TimerEvent::Elapsed { remaining: Duration::from_millis(500) });
        assert_eq!(evs[1], TimerEvent::Trigger);
        // reset 以 poll 当下为基准：t0+MIN 时的 remaining = interval - 0.5s
        assert_eq!(t.remaining(t0 + MIN), Some(MIN - Duration::from_millis(500)));
        // 已启动再 start → false（含暂停期间，见 Q11）
        assert!(!t.start(t0 + MIN));
        assert!(t.pause(t0 + MIN));
        assert!(!t.start(t0 + MIN)); // 有意偏差 Q11
    }

    #[test]
    fn paused_start_returns_false_q11() {
        // 清单 A14：v2 Start() 守卫是 `_timer != null && _timer.Enabled`，暂停后调用会
        // 成功；该序列 v2 UI 不可达、NUnit 未锁定 → Q11 决议：v3 暂停期间 start() 返回
        // false（视为已运行）。
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(MIN).unwrap();
        assert!(t.start(t0));
        assert!(t.pause(t0 + SEC));
        assert!(!t.start(t0 + SEC)); // Q11 有意偏差
                                     // resume 后照常，且暂停时长被顺延补偿（A7）：暂停 1s → remaining 少 1s
        assert!(t.resume(t0 + 2 * SEC));
        assert_eq!(t.remaining(t0 + 2 * SEC), Some(MIN - SEC));
    }

    // ---- 以下为 v2 NUnit src/TestProject/GlobalTimerServiceTest.cs 的移植 ----
    // 真实 1Hz 系统 Timer / Task.Delay 由“虚拟时钟逐秒喂 poll()”替代（不真实睡眠）；
    // 断言保留 NUnit 的 ±2s 容差口径（确定性虚拟时钟下实际精确相等）。

    /// NUnit Start：0.1s 构造抛 ArgumentOutOfRangeException；TestTimer(6/8/3)：
    /// Start 幂等、N 秒内 Trigger 计数 N±2、Stop 后重启计数归零（重启后紧接的
    /// CheckTrigger 只发 Elapsed 剩余=interval，不发 Trigger）。
    #[test]
    fn nunit_start_port() {
        // constructor：0.1s < 1s → Err（对齐 ArgumentOutOfRangeException）
        assert!(matches!(
            GlobalTimer::new(Duration::from_millis(100)),
            Err(IntervalTooShort)
        ));

        for seconds in [6usize, 8, 3] {
            let t0 = Instant::now();
            let mut t = GlobalTimer::new(SEC).unwrap();
            let mut count = 0usize;
            assert!(t.start(t0));
            // A3：Start 立即刷新剩余——紧接的 poll 产出 Elapsed{remaining=interval}
            //（对齐 v2 Start() 内 Reset() 的“立即更新”CheckTrigger）
            assert_eq!(t.poll(t0), vec![TimerEvent::Elapsed { remaining: SEC }]);
            assert!(!t.start(t0)); // 二次 Start → false（A2）
            for i in 1..=seconds as u32 {
                for ev in t.poll(t0 + i * SEC) {
                    if ev == TimerEvent::Trigger {
                        count += 1;
                    }
                }
            }
            // NUnit：count ∈ [seconds-2, seconds+2]；虚拟时钟下恰为 seconds
            assert!(count + 2 >= seconds && count <= seconds + 2, "seconds={seconds} count={count}");

            t.stop(t0 + seconds as u32 * SEC);
            // 重启后计数归零：Start 后紧接 poll 只发 Elapsed{interval}（A3），无 Trigger
            count = 0;
            assert!(t.start(t0));
            for ev in t.poll(t0) {
                assert!(
                    matches!(ev, TimerEvent::Elapsed { .. }),
                    "重启后紧接的 poll 不应 Trigger: {ev:?}"
                );
            }
            assert_eq!(count, 0);
            // NUnit 未断言剩余值，但 A3 语义要求它等于满间隔
            assert_eq!(t.remaining(t0), Some(SEC));
        }
    }

    /// NUnit Stop：未启动 Stop → false（且 v2 会补一次 CheckTrigger → 调用方补一次
    /// poll，怪癖 1/A4）；运行中 Stop → true；再 Stop → false。
    #[test]
    fn nunit_stop_port() {
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(MIN).unwrap();
        assert!(!t.stop(t0));
        // 怪癖 1：未启动 stop 后调用方补一次 poll——此刻无 next_trigger，
        // 不发事件、不 panic（v2 的 CheckTrigger 因 _nextTriggerTime==null 同样无事件）
        assert!(t.poll(t0).is_empty());
        assert!(t.start(t0));
        assert!(t.stop(t0 + SEC));
        assert!(!t.stop(t0 + 2 * SEC));
        // 再次补 poll（同上，core 的 stop 已清 next_trigger → 无事件）
        assert!(t.poll(t0 + 2 * SEC).is_empty());
    }

    /// NUnit Pause：未启动 Pause → false；1s 后触发 1±2 次；Pause → true；
    /// 已暂停再 Pause → false；暂停 3s 计数不增（A6）。
    #[test]
    fn nunit_pause_port() {
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(SEC).unwrap();
        let mut count = 0usize;
        assert!(!t.pause(t0)); // 未开始
        assert!(t.start(t0));
        // ~1s 后的一个 tick（interval==1s → remaining 0 → Trigger + 自动 Reset）
        for ev in t.poll(t0 + SEC) {
            if ev == TimerEvent::Trigger {
                count += 1;
            }
        }
        // NUnit 容差 ±2
        assert!(count <= 1 + 2, "count={count}");

        // v2 Pause 自带一次 CheckTrigger：调用方在 pause 后 poll（怪癖 2）——
        // 此刻 remaining 恰为 1s（不 < 1s）→ 只有 Elapsed，无 Trigger
        assert!(t.pause(t0 + SEC));
        for ev in t.poll(t0 + SEC) {
            assert!(matches!(ev, TimerEvent::Elapsed { .. }), "{ev:?}");
        }
        assert!(!t.pause(t0 + SEC)); // 已暂停

        // 暂停 3s：v2 的系统 Timer 已 Stop → 这段时间内 CheckTrigger 一次都不会被
        // 调用（等价于“暂停期间不 poll”，A6）→ count 不增；倒计时冻结在暂停
        // 瞬间的 remaining = 1s（自动 Reset 后截止 = t0+2s，此刻还剩 1s）
        assert_eq!(t.remaining(t0 + SEC), Some(SEC));
        assert_eq!(count, 1); // 判断没有增加
        assert!(t.stop(t0 + 4 * SEC));
        // 补充：暂停期间的 poll() 语义 = v2 Reset-while-paused 等显式 CheckTrigger 路径
        // （截止已过时仍会 Trigger）；AppModel 的 tick_timer 以 is_paused 门控，
        // 暂停窗口内不 poll——见 crates/eyenurse-app/src/app.rs tick_timer。
        // 怪癖 1 演练：stop 后调用方补一次 poll → 无 next_trigger → 无事件、不 panic
        assert!(t.poll(t0 + 4 * SEC).is_empty());
    }

    /// NUnit Resume：Start→Pause→Resume → true；5s 内触发 5±2 次；已恢复再
    /// Resume → false；再 Pause/再 Pause(false)；暂停 3s 计数不增。
    #[test]
    fn nunit_resume_port() {
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(SEC).unwrap();
        let mut count = 0usize;
        assert!(t.start(t0));
        assert!(t.pause(t0));
        assert!(t.resume(t0)); // 暂停 0s → next 不变
        for i in 1..=5u32 {
            for ev in t.poll(t0 + i * SEC) {
                if ev == TimerEvent::Trigger {
                    count += 1;
                }
            }
        }
        assert!(count + 2 >= 5 && count <= 5 + 2, "count={count}");
        assert!(!t.resume(t0 + 5 * SEC)); // 已恢复 → false（A8）
        assert!(t.pause(t0 + 5 * SEC));
        assert!(!t.pause(t0 + 5 * SEC)); // 已暂停 → false（A5）
        // 暂停 3s：不 poll（v2 暂停期间系统 Timer 已停，A6）→ count 不增；
        // 恢复时顺延暂停时长（A7）：暂停瞬间剩余 1s（截止 t0+6s），resume 于
        // t0+8s → 截止顺延 3s 至 t0+9s → remaining 恢复为暂停前的 1s
        assert!(t.resume(t0 + 8 * SEC));
        assert_eq!(t.remaining(t0 + 8 * SEC), Some(SEC));
        assert_eq!(count, 5); // 判断没有增加
        assert!(t.stop(t0 + 8 * SEC));
    }

    /// NUnit Reset：2min 间隔，Elapsed 的 remaining ≥ 0 且严格单调递减；
    /// 10s 后 trigger-lastRemaining < 11s；Reset 后 remaining 回满，
    /// 再 5s 后 < 6s（±1s 容差）。
    #[test]
    fn nunit_reset_port() {
        let trigger_ts = 2 * MIN;
        let t0 = Instant::now();
        let mut t = GlobalTimer::new(trigger_ts).unwrap();
        assert!(t.start(t0));
        let mut last: Option<Duration> = None;
        // 前 10s：每秒一次 tick（v2 1Hz Timer_Elapsed → CheckTrigger）
        for i in 1..=10u32 {
            for ev in t.poll(t0 + i * SEC) {
                match ev {
                    TimerEvent::Elapsed { remaining } => {
                        assert!(remaining >= Duration::ZERO);
                        if let Some(prev) = last {
                            assert!(prev > remaining, "Elapsed 剩余应严格递减");
                        }
                        last = Some(remaining);
                    }
                    TimerEvent::Trigger => panic!("2min 间隔 10s 内不应触发"),
                }
            }
        }
        let elapsed = trigger_ts - last.expect("10s 内应收到 Elapsed");
        assert!(elapsed < 11 * SEC); // NUnit ±1s 容差

        // Reset：remaining 立即回满；随后 5s 内继续单调递减
        assert!(t.reset(t0 + 10 * SEC));
        assert_eq!(t.remaining(t0 + 10 * SEC), Some(trigger_ts));
        last = None;
        for i in 1..=5u32 {
            for ev in t.poll(t0 + 10 * SEC + i * SEC) {
                match ev {
                    TimerEvent::Elapsed { remaining } => {
                        if let Some(prev) = last {
                            assert!(prev > remaining);
                        }
                        last = Some(remaining);
                    }
                    TimerEvent::Trigger => panic!("Reset 后 5s 内不应触发"),
                }
            }
        }
        let elapsed2 = trigger_ts - last.expect("Reset 后应收到 Elapsed");
        assert!(elapsed2 < 6 * SEC); // NUnit ±1s 容差
    }
}
