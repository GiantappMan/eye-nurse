using Caliburn.Micro;
using EyeNurse.Client.Services;
using EyeNurse.Client.ViewModels;
using Hardcodet.Wpf.TaskbarNotification;
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace EyeNurse.Client
{
    public class Bootstrapper : BootstrapperBase
    {
        private SimpleContainer container;
        private TaskbarIcon notifyIcon;
        private static NLog.Logger logger = NLog.LogManager.GetCurrentClassLogger();

        public Bootstrapper()
        {
            Initialize();
        }

        protected override void Configure()
        {
            //异常捕获
            Application.Current.DispatcherUnhandledException += Current_DispatcherUnhandledException;
            AppDomain.CurrentDomain.UnhandledException += CurrentDomain_UnhandledException;

            //映射公用库中的vm
            string dzyView = "DZY.DotNetUtil.WPF.Views";
            string dzyViewModel = "DZY.DotNetUtil.WPF.ViewModels";
            ViewLocator.AddSubNamespaceMapping(dzyViewModel, dzyView, "Control");
            ViewModelLocator.AddSubNamespaceMapping(dzyView, dzyViewModel);

            //初始化ioc容器
            container = new SimpleContainer();

            container.
                Singleton<IEventAggregator, EventAggregator>().
                Singleton<IWindowManager, WindowManager>().
                Singleton<CountDownViewModel>().
                Singleton<ContextMenuViewModel>(nameof(ContextMenuViewModel)).
                Singleton<EyeNurseService>().
                PerRequest<LockScreenViewModel>().
                PerRequest<SettingViewModel>().
                Instance(container);
        }

        #region exceptions

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

        #endregion

        #region Icon

        private void NotifyIcon_TrayMouseDoubleClick(object sender, RoutedEventArgs e)
        {
            //重置到最顶层
            Application.Current.MainWindow.Topmost = false;
            Application.Current.MainWindow.Topmost = true;
            Application.Current.MainWindow.Show();
        }

        #endregion

        protected override IEnumerable<Assembly> SelectAssemblies()
        {
            return new[] {
                Assembly.GetExecutingAssembly(),
                Assembly.Load(new AssemblyName("DZY.DotNetUtil.WPF")),
            };
        }

        protected override void OnStartup(object sender, StartupEventArgs e)
        {
            try
            {
                //托盘icon
                notifyIcon = (TaskbarIcon)Application.Current.FindResource("NotifyIcon");
                notifyIcon.TrayMouseDoubleClick += NotifyIcon_TrayMouseDoubleClick;
                notifyIcon.Icon = Icon.ExtractAssociatedIcon(Assembly.GetExecutingAssembly().Location);
                container.Instance(notifyIcon);
            }
            catch (Exception ex)
            {
                throw;
            }
            DisplayRootViewFor<CountDownViewModel>();
        }

        protected override object GetInstance(Type service, string key)
        {
            return container.GetInstance(service, key);
        }

        protected override IEnumerable<object> GetAllInstances(Type service)
        {
            return container.GetAllInstances(service);
        }

        protected override void BuildUp(object instance)
        {
            container.BuildUp(instance);
        }

        protected override void OnExit(object sender, EventArgs e)
        {
            notifyIcon.Dispose();
            NLog.LogManager.Shutdown();
            base.OnExit(sender, e);
        }
    }
}
