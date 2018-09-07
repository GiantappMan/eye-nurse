using Caliburn.Micro;
using DZY.DotNetUtil.Helpers;
using DZY.DotNetUtil.ViewModels;
using EyeNurse.Client.Events;
using EyeNurse.Client.Helpers;
using EyeNurse.Client.Services;
using NLog;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using Windows.Services.Store;

namespace EyeNurse.Client.ViewModels
{
    public class ContextMenuViewModel : Screen
    {
        private Logger logger = NLog.LogManager.GetCurrentClassLogger();
        private IWindowManager _windowManager = null;
        private SettingViewModel _settingVM;
        readonly IEventAggregator _eventAggregator;
        public ContextMenuViewModel(EyeNurseService servcies, IWindowManager windowManager, IEventAggregator eventAggregator)
        {
            _eventAggregator = eventAggregator;
            _windowManager = windowManager;
            Services = servcies;
            Init();
        }

        private async void Init()
        {
            IsAutoLaunch = await AutoStartup.Instance.Check();
        }

        #region properties

        public EyeNurseService Services { get; }

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
            IsPaused = true;
            Services.Pause();
        }

        public void Resum()
        {
            IsPaused = false;
            Services.Resum();
        }

        public void ExitApp()
        {
            Application.Current.Shutdown();
        }

        public void RestImmediately()
        {
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

        public void OpenConfig()
        {
            if (_settingVM != null)
                return;

            _settingVM = IoC.Get<SettingViewModel>();
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
            try
            {
                Process.Start("https://shang.qq.com/wpa/qunwpa?idkey=e8d8e46fa4067c16110376db53d51065bdce6abb943e08f09736317527bfbf45");
            }
            catch (Exception ex)
            {
                logger.Error(ex, "About Ex");
            }
        }

        #endregion
    }
}
