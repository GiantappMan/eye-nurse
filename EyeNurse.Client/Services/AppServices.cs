using EyeNurse.Client.Configs;
using EyeNurse.Client.Helpers;
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
        Timer timer;
        AppSetting appSetting;

        public AppServices()
        {
            Init();
        }

        private async void Init()
        {
            timer = new Timer();
            timer.Interval = 1000;
            timer.Elapsed += Timer_Elapsed;
            appSetting = await LoadConfigAsync<AppSetting>();
            if (appSetting == null)
            {
                //默认值
                appSetting = new AppSetting()
                {
                    AlarmInterval = new TimeSpan(0, 45, 0)
                };
            }

            Countdown = appSetting.AlarmInterval;
            Start();
        }

        private void Timer_Elapsed(object sender, ElapsedEventArgs e)
        {
            Countdown = Countdown.Subtract(new TimeSpan(0, 0, 1));
            CountdownPercent = Countdown.TotalSeconds / appSetting.AlarmInterval.TotalSeconds * 100;
        }

        #region properties

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

        private double _CountdownPercent;

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

        #endregion

        #region public methods

        public void Start()
        {
            timer.Start();
        }

        public void Pause()
        {
            timer.Stop();
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
