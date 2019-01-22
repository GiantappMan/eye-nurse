using Caliburn.Micro;
using EyeNurse.Client.Events;
using EyeNurse.Client.Helpers;
using EyeNurse.Client.Services;
using NLog;
using System;
using System.Diagnostics;
using System.Windows;

namespace EyeNurse.Client.ViewModels
{
    public class ContextMenuViewModel : Screen, IHandle<VipEvent>
    {
        private Logger logger = NLog.LogManager.GetCurrentClassLogger();
        private IWindowManager _windowManager = null;
        private SettingViewModel _settingVM;
        readonly IEventAggregator _eventAggregator;
        public ContextMenuViewModel(EyeNurseService servcies, IWindowManager windowManager, IEventAggregator eventAggregator)
        {
            eventAggregator.Subscribe(this);
            _eventAggregator = eventAggregator;
            _windowManager = windowManager;
            Services = servcies;
            IsVIP = Services.AppData.Purchased;
            Init();
        }

        private async void Init()
        {
            IsAutoLaunch = await AutoStartup.Instance.Check();
        }

        #region properties

        public EyeNurseService Services { get; }

        #region IsVIP

        /// <summary>
        /// The <see cref="IsVIP" /> property's name.
        /// </summary>
        public const string IsVIPPropertyName = "IsVIP";

        private bool _IsVIP;

        /// <summary>
        /// IsVIP
        /// </summary>
        public bool IsVIP
        {
            get { return _IsVIP; }

            set
            {
                if (_IsVIP == value) return;

                _IsVIP = value;
                NotifyOfPropertyChange(IsVIPPropertyName);
            }
        }

        #endregion

        #region IsPaused

        /// <summary>
        /// The <see cref="IsPaused" /> property's name.
        /// </summary>
        public const string IsPausedPropertyName = "IsPaused";

        private bool _IsPaused;

        /// <summary>
        /// IsPaused
        /// </summary>
        public bool IsPaused
        {
            get { return _IsPaused; }

            set
            {
                if (_IsPaused == value) return;

                _IsPaused = value;
                NotifyOfPropertyChange(IsPausedPropertyName);
            }
        }


        #endregion

        #region IsAutoLaunch

        /// <summary>
        /// The <see cref="IsAutoLaunch" /> property's name.
        /// </summary>
        public const string IsAutoLaunchPropertyName = "IsAutoLaunch";

        private bool _IsAutoLaunch;

        /// <summary>
        /// IsAutoLaunch
        /// </summary>
        public bool IsAutoLaunch
        {
            get { return _IsAutoLaunch; }

            set
            {
                if (_IsAutoLaunch == value) return;

                _IsAutoLaunch = value;
                NotifyOfPropertyChange(IsAutoLaunchPropertyName);
            }
        }

        #endregion

        #endregion

        #region methods

        public async void StartWithOS()
        {
            IsAutoLaunch = !IsAutoLaunch;
            await AutoStartup.Instance.Set(IsAutoLaunch);
            IsAutoLaunch = await AutoStartup.Instance.Check();
        }

        public void Pause()
        {
            IsPaused = Services.Pause();
        }

        public void Resum()
        {
            IsPaused = false;
            Services.Resum();
        }

        public void Reset()
        {
            Services.Reset();
        }

        public void ExitApp()
        {
            Application.Current.Shutdown();
        }

        public void RestImmediately()
        {
            if (IsPaused)
                Resum();

            Services.RestImmediately();
        }

        public void About()
        {
            try
            {
                Process.Start("https://mscoder.cn/products/EyeNurse.html");
            }
            catch (Exception ex)
            {
                logger.Error(ex, "About Ex");
            }
        }

        public async void OpenConfig()
        {
            if (_settingVM != null)
                return;

            _settingVM = IoC.Get<SettingViewModel>();
            if (!Services.AppData.Purchased)
            {
                await Services.ShowPurchaseTip();
            }

            bool? isOk = _windowManager.ShowDialog(_settingVM);
            if (isOk != null && isOk.Value)
            {
                _eventAggregator.PublishOnUIThread(new AppSettingChangedEvent());
            }
            _settingVM = null;
        }

        public void Purchase()
        {
            Services.Purchase();
        }

        public void QQ()
        {
            Services.OpenQQGroupLink();
        }

        public void QQVIP()
        {
            Services.OpenVIPQQGroupLink();
        }

        public void Handle(VipEvent message)
        {
            IsVIP = message.IsVIP;
        }

        #endregion
    }
}
