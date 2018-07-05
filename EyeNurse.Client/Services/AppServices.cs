using Caliburn.Micro;
using EyeNurse.Client.Configs;
using EyeNurse.Client.Helpers;
using EyeNurse.Client.ViewModels;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Timers;

namespace EyeNurse.Client.Services
{
    public class AppServices : INotifyPropertyChanged
    {
        Timer _timer;
        AppSetting _appSetting;
        IWindowManager _windowManager;
        LockScreenViewModel _lastLockScreenViewModel;

        public AppServices(IWindowManager windowManager)
        {
            _windowManager = windowManager;
            Init();
        }

        private async void Init()
        {
            _timer = new Timer();
            _timer.Interval = 1000;
            _timer.Elapsed += Timer_Elapsed;
            await ResetCountDownAsync();
        }

        private async Task ResetCountDownAsync()
        {
            //休息结束
            if (_lastLockScreenViewModel != null)
            {
                _lastLockScreenViewModel.TryClose();
                _lastLockScreenViewModel = null;
            }

            _timer.Stop();
            IsResting = false;

            _appSetting = await LoadConfigAsync<AppSetting>();
            if (_appSetting == null)
            {
                //默认值
                _appSetting = new AppSetting()
                {
                    //AlarmInterval = new TimeSpan(0, 45, 0)
                    AlarmInterval = new TimeSpan(0, 0, 10),
                    RestTime = new TimeSpan(0, 0, 10)
                };
            }

            Countdown = _appSetting.AlarmInterval;
            CountdownPercent = 100;

            _timer.Start();
        }



        private async void Timer_Elapsed(object sender, ElapsedEventArgs e)
        {
            if (!IsResting)
            {
                Countdown = Countdown.Subtract(new TimeSpan(0, 0, 1));
                CountdownPercent = Countdown.TotalSeconds / _appSetting.AlarmInterval.TotalSeconds * 100;
                if (Countdown.TotalSeconds <= 0)
                {
                    _timer.Stop();
                    _lastLockScreenViewModel = IoC.Get<LockScreenViewModel>();
                    RestTimeCountdown = _appSetting.RestTime;
                    Execute.OnUIThread(() =>
                    {
                        _windowManager.ShowWindow(_lastLockScreenViewModel);
                        _lastLockScreenViewModel.Deactivated += _lastLockScreenViewModel_Deactivated;
                    });
                    IsResting = true;
                    _timer.Start();
                }
            }
            else
            {
                RestTimeCountdown = RestTimeCountdown.Subtract(new TimeSpan(0, 0, 1));
                RestTimeCountdownPercent = RestTimeCountdown.TotalSeconds / _appSetting.RestTime.TotalSeconds * 100;
                if (RestTimeCountdown.TotalSeconds <= 0)
                {
                    await ResetCountDownAsync();
                }
            }
        }

        private void _lastLockScreenViewModel_Deactivated(object sender, DeactivationEventArgs e)
        {
            var temp = sender as LockScreenViewModel;
            temp.Deactivated -= _lastLockScreenViewModel_Deactivated;
            RestTimeCountdown = new TimeSpan();
        }

        #region properties

        #region IsResting

        /// <summary>
        /// The <see cref="IsResting" /> property's name.
        /// </summary>
        public const string IsRestingPropertyName = "IsResting";

        private bool _IsResting;

        /// <summary>
        /// IsResting
        /// </summary>
        public bool IsResting
        {
            get { return _IsResting; }

            set
            {
                if (_IsResting == value) return;

                _IsResting = value;
                NotifyOfPropertyChange(IsRestingPropertyName);
            }
        }

        #endregion

        #region Countdown

        /// <summary>
        /// The <see cref="Countdown" /> property's name.
        /// </summary>
        public const string CountdownPropertyName = "Countdown";

        private TimeSpan _Countdown;

        /// <summary>
        /// Countdown
        /// </summary>
        public TimeSpan Countdown
        {
            get { return _Countdown; }

            set
            {
                if (_Countdown == value) return;

                _Countdown = value;
                NotifyOfPropertyChange(CountdownPropertyName);
            }
        }

        #endregion

        #region CountdownPercent

        /// <summary>
        /// The <see cref="CountdownPercent" /> property's name.
        /// </summary>
        public const string CountdownPercentPropertyName = "CountdownPercent";

        private double _CountdownPercent = 100;

        /// <summary>
        /// CountdownPercent
        /// </summary>
        public double CountdownPercent
        {
            get { return _CountdownPercent; }

            set
            {
                if (_CountdownPercent == value) return;

                _CountdownPercent = value;
                NotifyOfPropertyChange(CountdownPercentPropertyName);
            }
        }

        #endregion

        #region RestTimeCountdown

        /// <summary>
        /// The <see cref="RestTimeCountdown" /> property's name.
        /// </summary>
        public const string RestTimeCountdownPropertyName = "RestTimeCountdown";

        private TimeSpan _RestTimeCountdown;

        /// <summary>
        /// RestTimeCountdown
        /// </summary>
        public TimeSpan RestTimeCountdown
        {
            get { return _RestTimeCountdown; }

            set
            {
                if (_RestTimeCountdown == value) return;

                _RestTimeCountdown = value;
                NotifyOfPropertyChange(RestTimeCountdownPropertyName);
            }
        }

        #endregion

        #region RestTimeCountdownPercent

        /// <summary>
        /// The <see cref="RestTimeCountdownPercent" /> property's name.
        /// </summary>
        public const string RestTimeCountdownPercentPropertyName = "RestTimeCountdownPercent";

        private double _RestTimeCountdownPercent = 100;

        /// <summary>
        /// RestTimeCountdownPercent
        /// </summary>
        public double RestTimeCountdownPercent
        {
            get { return _RestTimeCountdownPercent; }

            set
            {
                if (_RestTimeCountdownPercent == value) return;

                _RestTimeCountdownPercent = value;
                NotifyOfPropertyChange(RestTimeCountdownPercentPropertyName);
            }
        }

        #endregion

        #endregion

        #region public methods

        public void Start()
        {
            _timer.Start();
        }

        public void Pause()
        {
            _timer.Stop();
        }

        #region config

        public async Task<T> LoadConfigAsync<T>(string path = null) where T : new()
        {
            try
            {
                if (string.IsNullOrEmpty(path))
                    path = GetDefaultPath<T>();

                var config = await JsonHelper.JsonDeserializeFromFileAsync<T>(path);
                return config;
            }
            catch (Exception ex)
            {
                return default(T);
            }
        }

        public async Task SaveConfigAsync<T>(T data, string path = null) where T : new()
        {
            if (string.IsNullOrEmpty(path))
                path = GetDefaultPath<T>();

            var json = await JsonHelper.JsonSerializeAsync(data, path);
        }

        public string GetDefaultPath<T>() where T : new()
        {
            var rootDir = Environment.CurrentDirectory;
            return $"{rootDir}\\Configs\\{typeof(T).Name}.json";
        }

        #endregion

        #endregion

        #region INotifyPropertyChanged
        public event PropertyChangedEventHandler PropertyChanged;
        protected void NotifyOfPropertyChange(string propertyName)
        {
            var handle = PropertyChanged;
            if (handle == null)
                return;
            handle(this, new PropertyChangedEventArgs(propertyName));
        }
        #endregion
    }
}
