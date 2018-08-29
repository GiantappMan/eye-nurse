using Caliburn.Micro;
using Hardcodet.Wpf.TaskbarNotification;
using System;
using System.Collections.Generic;
using System.Configuration;
using System.Data;
using System.Drawing;
using System.Linq;
using System.Reflection;
using System.Threading.Tasks;
using System.Windows;

namespace EyeNurse.Client
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
        private TaskbarIcon notifyIcon;
        private static NLog.Logger logger = NLog.LogManager.GetCurrentClassLogger();

        protected override void OnStartup(StartupEventArgs e)
        {
            Current.DispatcherUnhandledException += Current_DispatcherUnhandledException;
            AppDomain.CurrentDomain.UnhandledException += CurrentDomain_UnhandledException;

            base.OnStartup(e);

            notifyIcon = (TaskbarIcon)FindResource("NotifyIcon");
            notifyIcon.TrayMouseDoubleClick += NotifyIcon_TrayMouseDoubleClick;
            notifyIcon.Icon = Icon.ExtractAssociatedIcon(Assembly.GetExecutingAssembly().Location);
            var container = IoC.Get<SimpleContainer>();
            container.Instance(notifyIcon);
        }

        private void NotifyIcon_TrayMouseDoubleClick(object sender, RoutedEventArgs e)
        {
            //重置到最顶层
            Current.MainWindow.Topmost = false;
            Current.MainWindow.Topmost = true;
            Current.MainWindow.Show();
        }

        private void CurrentDomain_UnhandledException(object sender, UnhandledExceptionEventArgs e)
        {
            var ex = e.ExceptionObject as Exception;
            logger.Error(ex);
            MessageBox.Show(ex.Message);
        }

        void Current_DispatcherUnhandledException(object sender, System.Windows.Threading.DispatcherUnhandledExceptionEventArgs e)
        {
            var ex = e.Exception;
            logger.Error(ex);
            MessageBox.Show(ex.Message);
        }

        protected override void OnExit(ExitEventArgs e)
        {
            NLog.LogManager.Shutdown();
            notifyIcon.Dispose();
            base.OnExit(e);
        }
    }
}
