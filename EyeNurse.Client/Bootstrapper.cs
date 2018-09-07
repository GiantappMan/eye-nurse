using Caliburn.Micro;
using EyeNurse.Client.Services;
using EyeNurse.Client.ViewModels;
using System;
using System.Collections.Generic;
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

        public Bootstrapper()
        {
            Initialize();
        }

        protected override void Configure()
        {
            //string view = "EyeNurse.Client.Views";
            string dzyView = "DZY.DotNetUtil.WPF.Views";
            string dzyViewModel = "DZY.DotNetUtil.WPF.ViewModels";
            //ViewLocator.AddNamespaceMapping(dzyViewModel, dzyView);
            ViewLocator.AddSubNamespaceMapping(dzyViewModel, dzyView, "Control");

            ViewModelLocator.AddSubNamespaceMapping(dzyView, dzyViewModel);
            //自定义消息拦截
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

        protected override IEnumerable<Assembly> SelectAssemblies()
        {
            return new[] {
                Assembly.GetExecutingAssembly(),
                Assembly.Load(new AssemblyName("DZY.DotNetUtil.WPF")),
            };
        }

        protected override void OnStartup(object sender, StartupEventArgs e)
        {
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
    }
}
