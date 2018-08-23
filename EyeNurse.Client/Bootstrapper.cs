using Caliburn.Micro;
using EyeNurse.Client.Services;
using EyeNurse.Client.ViewModels;
using System;
using System.Collections.Generic;
using System.Linq;
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
            string view = "EyeNurse.Client.Views";
            string dzyViewModel = "DZY.DotNetUtil.ViewModels";
            ViewLocator.AddSubNamespaceMapping(dzyViewModel, view);

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
