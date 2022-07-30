using Common.Apps.WPF.ViewModels;
using CommunityToolkit.Mvvm.ComponentModel;
using EyeNurse.Views;
using Microsoft.Web.WebView2.Core;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.ViewModels
{
    public class MainViewModel : WebView2ShellViewModel
    {
        #region fileds
        MainWindow? _window;
        #endregion

        #region construct
        public MainViewModel() : base()
        {

        }
        #endregion

        #region properties
        #endregion

        #region public

        public void ShowWindow()
        {
            if (_window == null)
            {
                _window = new MainWindow();
                _window.DataContext = this;
                _window.Closed += Window_Closed;
            }
            if (_window.WindowState == System.Windows.WindowState.Minimized)
                _window.WindowState = System.Windows.WindowState.Normal;
            
            if (!_window.IsActive)
                _window.Activate();

            _window.Show();
        }

        #endregion

        #region private

        private void Window_Closed(object? sender, EventArgs e)
        {
            _window!.Closed -= Window_Closed;
            _window = null;
        }

        #endregion
    }
}
