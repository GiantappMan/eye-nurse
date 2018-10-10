using Caliburn.Micro;
using DZY.DotNetUtil.Helpers;
using EyeNurse.Client.Configs;
using EyeNurse.Client.Events;
using EyeNurse.Client.Services;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Interop;

namespace EyeNurse.Client.ViewModels
{
    public class CountDownViewModel : Screen, IHandle<AppSettingChangedEvent>
    {
        WindowPosition _windowPosition;

        public CountDownViewModel(EyeNurseService servcies, IEventAggregator eventAggregator)
        {
            eventAggregator.Subscribe(this);
            AppService = servcies;
            ApplyConfig();
        }

        #region public methods

        public async void SourceInitialized()
        {
            var handle = (new WindowInteropHelper(Application.Current.MainWindow)).Handle;
            await AppService.Init(handle);
        }

        public void ApplyConfig()
        {
            AppService.ReloadSetting();
            var setting = AppService.Setting;
            _windowPosition = setting?.WindowPosition;
            if (_windowPosition == null)
            {
                //默认位置
                _windowPosition = new WindowPosition
                {
                    X = SystemParameters.WorkArea.Width - 120,
                    Y = SystemParameters.WorkArea.Height / 3
                };
            }
            PositionLeft = _windowPosition.X;
            PositionTop = _windowPosition.Y;
        }

        internal async void SavePosition()
        {
            var setting = await JsonHelper.JsonDeserializeFromFileAsync<Setting>(AppService.ConfigFilePath);
            setting.WindowPosition = _windowPosition;
            await JsonHelper.JsonSerializeAsync(setting, AppService.ConfigFilePath);
        }

        public void Handle(AppSettingChangedEvent message)
        {
            ApplyConfig();
        }

        #endregion

        #region properties

        public EyeNurseService AppService { get; }

        #region PositionLeft

        /// <summary>
        /// The <see cref="PositionLeft" /> property's name.
        /// </summary>
        public const string PositionLeftPropertyName = "PositionLeft";

        private double _PositionLeft;

        /// <summary>
        /// PositionLeft
        /// </summary>
        public double PositionLeft
        {
            get { return _PositionLeft; }

            set
            {
                if (_PositionLeft == value) return;

                _PositionLeft = value;
                _windowPosition.X = value;
                NotifyOfPropertyChange(PositionLeftPropertyName);
            }
        }

        #endregion

        #region PositionTop

        /// <summary>
        /// The <see cref="PositionTop" /> property's name.
        /// </summary>
        public const string PositionTopPropertyName = "PositionTop";

        private double _PositionTop;

        /// <summary>
        /// PositionTop
        /// </summary>
        public double PositionTop
        {
            get { return _PositionTop; }

            set
            {
                if (_PositionTop == value) return;

                _PositionTop = value;
                _windowPosition.Y = value;
                NotifyOfPropertyChange(PositionTopPropertyName);
            }
        }

        #endregion

        #endregion
    }
}
