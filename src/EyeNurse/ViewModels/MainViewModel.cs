using Common.Apps.WPF.ViewModels;
using EyeNurse.Views;
using Microsoft.Web.WebView2.Core;
using System;
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

        public async void ShowWindow()
        {
            if (_window == null)
            {
                _window = new MainWindow();
                _window.webview2.NavigationCompleted += Webview2_NavigationCompleted;
                _window.DataContext = this;
                _window.Closed += Window_Closed;
            }
            if (_window.WindowState == System.Windows.WindowState.Minimized)
                _window.WindowState = System.Windows.WindowState.Normal;

            if (!_window.IsActive)
                _window.Activate();

            Initlizing = true;
            bool ok = await Task.Run(CheckWebView2);
            _window.Show();
            if (!ok)
                Initlizing = false;
        }

        private void Webview2_NavigationCompleted(object? sender, CoreWebView2NavigationCompletedEventArgs e)
        {
            if (_window == null)
                return;
            _window.webview2.NavigationCompleted -= Webview2_NavigationCompleted;
            Initlizing = false;
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
