using Caliburn.Micro;
using EyeNurse.Client.Configs;
using EyeNurse.Client.Events;
using EyeNurse.Client.Services;
using JsonConfiger.Utils;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace EyeNurse.Client.ViewModels
{
    public class CountDownViewModel : Screen
    {
        WindowPosition _windowPosition;

        public CountDownViewModel(EyeNurseService servcies)
        {
            AppService = servcies;
        }

        #region override

        protected override async void OnInitialize()
        {
            var setting = await JsonHelper.JsonDeserializeFromFileAsync<Setting>(AppService.ConfigFilePath);
            _windowPosition = setting == null ? new WindowPosition() : setting.WindowPosition;
            if (_windowPosition == null)
            {
                _windowPosition = new WindowPosition();
                _windowPosition.X = SystemParameters.WorkArea.Width - 120;
                _windowPosition.Y = SystemParameters.WorkArea.Height / 3;
            }
            PositionLeft = _windowPosition.X;
            PositionTop = _windowPosition.Y;

            base.OnInitialize();
        }

        protected override void OnDeactivate(bool close)
        {
            base.OnDeactivate(close);
        }

        #endregion

        #region public methods

        internal async void SavePosition()
        {
            var setting = await JsonHelper.JsonDeserializeFromFileAsync<Setting>(AppService.ConfigFilePath);
            setting.WindowPosition = _windowPosition;
            await JsonHelper.JsonSerializeAsync(setting, AppService.ConfigFilePath);
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
