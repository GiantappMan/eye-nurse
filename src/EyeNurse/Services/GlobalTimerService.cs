using System;
using System.Timers;

namespace EyeNurse.Services
{
    /// <summary>
    /// 全局时间管理
    /// 职责：定时触发，全屏检测
    /// </summary>
    public class GlobalTimerService
    {
        #region field
        Timer? _timer;
        DateTime? _nextTriggerTime;//下次触发时间
        DateTime? _pauseTime;//暂停时间
        readonly TimeSpan _triggerInterval;//触发间隔
        readonly long _oneSecondsTicks = TimeSpan.FromSeconds(1).Ticks;
        bool _isChecking;
        #endregion

        #region events
        public event EventHandler<TimerTriggerEventArgs>? Trigger;
        public event EventHandler<TimerElapsedEventArgs>? Elapsed;
        #endregion

        #region constructs
        public GlobalTimerService(TimeSpan triggerInterval)
        {
            _triggerInterval = triggerInterval;
            if (triggerInterval.TotalSeconds < 1)
                throw new ArgumentOutOfRangeException(nameof(triggerInterval));
        }
        #endregion

        #region public methods
        /// <summary>
        /// 
        /// </summary>
        /// <param name="triggerInterval">不能小于一秒</param>
        /// <returns></returns>
        public bool Start()
        {
            if (_timer != null && _timer.Enabled)
                return false;

            Reset();
            _timer = new Timer
            {
                Interval = 1000
            };
            _timer.Elapsed += Timer_Elapsed;
            _timer.Start();
            return true;
        }
        public bool Stop()
        {
            if (_timer != null)
            {
                _timer.Stop();
                _timer.Dispose();
                _timer = null;
                return true;
            }
            CheckTrigger();
            return false;
        }
        public bool Pause()
        {
            if (_timer == null || !_timer.Enabled)
                return false;

            _timer?.Stop();
            _pauseTime = DateTime.Now;
            CheckTrigger();
            return true;
        }
        public bool Resume()
        {
            if (_timer == null || _timer.Enabled)
                return false;

            _timer?.Start();
            if (_nextTriggerTime != null && _pauseTime != null)
                _nextTriggerTime = _nextTriggerTime.Value + (DateTime.Now - _pauseTime);//增加暂停时间
            _pauseTime = null;
            CheckTrigger();
            return true;
        }
        public bool Reset()
        {
            _nextTriggerTime = DateTime.Now + _triggerInterval;
            CheckTrigger();//立即更新
            return true;
        }
        #endregion

        #region private
        private void Timer_Elapsed(object? sender, ElapsedEventArgs e)
        {
            CheckTrigger();
        }

        private void CheckTrigger()
        {
            if (_isChecking)
                return;

            _isChecking = true;
            if (_nextTriggerTime != null)
            {
                TimeSpan ts = _nextTriggerTime.Value - DateTime.Now;
                if (ts.TotalMilliseconds < 0)
                    ts = new TimeSpan();
                Elapsed?.Invoke(this, new TimerElapsedEventArgs()
                {
                    RemainingTime = ts
                });
            }
            var now = DateTime.Now;
            if (_nextTriggerTime != null)
            {
                var timeDiff = (_nextTriggerTime - DateTime.Now).Value.Ticks;
                if (timeDiff < _oneSecondsTicks)
                {
                    //todo check isfullscreen
                    Trigger?.Invoke(this, new TimerTriggerEventArgs());
                    Reset();
                }
            }

            _isChecking = false;
        }
        #endregion
    }
}
