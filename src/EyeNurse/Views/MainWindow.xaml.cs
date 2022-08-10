using Common.Apps.Services;
using EyeNurse.Apis;
using EyeNurse.Services;
using Microsoft.Web.WebView2.Core;
using System;
using System.Windows;

namespace EyeNurse.Views
{
    public partial class MainWindow : Window
    {
        readonly EyeNurseService? _eyeNurseService;

        public MainWindow()
        {
            InitializeComponent();

            _eyeNurseService = IocService.GetService<EyeNurseService>()!;
            webview2.CoreWebView2InitializationCompleted += Webview2_CoreWebView2InitializationCompleted;
            Uri domain = new("https://client.eyenurse.giantapp.cn/index.html");
#if DEBUG
            //本地开发
            //domain = new("http://localhost:3000/index.html");
#endif
            Uri source = new($"{domain}#/settings");
            webview2.Source = source;
        }

        #region private

        private void Webview2_CoreWebView2InitializationCompleted(object? sender, CoreWebView2InitializationCompletedEventArgs e)
        {
            webview2.CoreWebView2.AddHostObjectToScript("api", new ClientApi(_eyeNurseService!));
            //webview2.CoreWebView2.DOMContentLoaded += CoreWebView2_DOMContentLoaded; ;

            webview2.CoreWebView2.SetVirtualHostNameToFolderMapping("client.eyenurse.giantapp.cn", "Assets/UI", CoreWebView2HostResourceAccessKind.DenyCors);
        }

        //private async void CoreWebView2_DOMContentLoaded(object? sender, CoreWebView2DOMContentLoadedEventArgs e)
        //{
        //    //https://docs.microsoft.com/zh-cn/microsoft-edge/webview2/how-to/javascript

        //    //屏蔽右键
        //    await webview2.CoreWebView2.ExecuteScriptAsync("window.addEventListener('contextmenu', window => {window.preventDefault();});");

        //    //屏蔽拖放
        //    await webview2.CoreWebView2.AddScriptToExecuteOnDocumentCreatedAsync(
        //       "window.addEventListener('dragover',function(e){e.preventDefault();},false);" +
        //       "window.addEventListener('drop',function(e){" +
        //          "e.preventDefault();" +
        //          "console.log(e.dataTransfer);" +
        //          "console.log(e.dataTransfer.files[0])" +
        //       "}, false);"
        //       );
        //}
        #endregion
    }
}
