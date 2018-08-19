using Caliburn.Micro;
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

        [ComImport]
        [Guid("3E68D4BD-7135-4D10-8018-9FB6D9F33FA1")]
        [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        public interface IInitializeWithWindow
        {
            void Initialize(IntPtr hwnd);
        }

        public async void Donate()
        {
            await PurchaseAddOn("9PPHPFT7Q3XK");
        }

        private StoreContext context = null;
        public async Task<string> PurchaseAddOn(string storeId)
        {
            string msg = null;
            if (context == null)
            {
                ////UWP
                //context = StoreContext.GetDefault();

                //desktopbridge
                //await Execute.OnUIThreadAsync(() =>
                //{
                context = StoreContext.GetDefault();
                IInitializeWithWindow initWindow = (IInitializeWithWindow)(object)context;
                IntPtr mainHandler = IoC.Get<IntPtr>("MainHandler");
                initWindow.Initialize(mainHandler);
                //});
            }

            StorePurchaseResult result = await context.RequestPurchaseAsync(storeId);

            // Capture the error message for the operation, if any.
            string extendedError = string.Empty;
            if (result.ExtendedError != null)
            {
                extendedError = result.ExtendedError.Message;
            }

            switch (result.Status)
            {
                case StorePurchaseStatus.AlreadyPurchased:
                    msg = "The user has already purchased the product.";
                    break;

                case StorePurchaseStatus.Succeeded:
                    msg = "The purchase was successful.";
                    break;

                case StorePurchaseStatus.NotPurchased:
                    msg = "The purchase did not complete. " +
                        "The user may have cancelled the purchase. ExtendedError: " + extendedError;
                    break;

                case StorePurchaseStatus.NetworkError:
                    msg = "The purchase was unsuccessful due to a network error. " +
                        "ExtendedError: " + extendedError;
                    break;

                case StorePurchaseStatus.ServerError:
                    msg = "The purchase was unsuccessful due to a server error. " +
                        "ExtendedError: " + extendedError;
                    break;

                default:
                    msg = "The purchase was unsuccessful due to an unknown error. " +
                        "ExtendedError: " + extendedError;
                    break;
            }

            return msg;
        }

        #endregion
    }
}
