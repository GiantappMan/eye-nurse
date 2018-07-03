using Caliburn.Micro;
using EyeNurse.Client.Configs;
using EyeNurse.Client.Services;
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
        readonly AppServices _services;
        WindowPosition _windowPosition;
        public CountDownViewModel(AppServices servcies)
        {
            _services = servcies;
        }

        #region override

        protected override async void OnInitialize()
        {
            _windowPosition = await Services.LoadConfigAsync<WindowPosition>();
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
            await Services.SaveConfigAsync(_windowPosition);
        }

        public void Test()
        {

        }

        #endregion

        #region properties

        public AppServices Services => _services;

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
