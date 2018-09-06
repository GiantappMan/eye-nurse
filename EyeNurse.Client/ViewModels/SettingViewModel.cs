using Caliburn.Micro;
using DZY.DotNetUtil.Helpers;
using EyeNurse.Client.Services;
using JsonConfiger;
using JsonConfiger.Models;
using NLog;
using System;
using System.Diagnostics;
using System.IO;
using Windows.Storage;

namespace EyeNurse.Client.ViewModels
{
    public class SettingViewModel : Screen
    {
        private Logger logger = NLog.LogManager.GetCurrentClassLogger();
        JCrService _jcrService = new JCrService();
        EyeNurseService _appServices;

        public SettingViewModel(EyeNurseService appService)
        {
            _appServices = appService;
        }

        protected async override void OnInitialize()
        {
            var config = await JsonHelper.JsonDeserializeFromFileAsync<dynamic>(_appServices.ConfigFilePath);
            string descPath = Path.Combine(AppDomain.CurrentDomain.SetupInformation.ApplicationBase, "Resources\\Configs\\setting.desc.json");
            var descConfig = await JsonHelper.JsonDeserializeFromFileAsync<dynamic>(descPath);
            JsonConfierViewModel = _jcrService.GetVM(config, descConfig);
            base.OnInitialize();
        }

        public async void Save()
        {
            var data = _jcrService.GetData(JsonConfierViewModel.Nodes);
            var result = await JsonHelper.JsonSerializeAsync(data, _appServices.ConfigFilePath);
            await Execute.OnUIThreadAsync(() =>
            {
                TryClose(true);
            });
            await _appServices.ReloadSetting();
        }
        public void Cancel()
        {
            TryClose(false);
        }
        public void OpenConfigFolder()
        {
            try
            {
#if UWP
                //https://stackoverflow.com/questions/48849076/uwp-app-does-not-copy-file-to-appdata-folder
                var appData = Path.Combine(ApplicationData.Current.LocalCacheFolder.Path, "Roaming\\EyeNurse");
#else
                var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                appData = Path.Combine(appData, "EyeNurse");
#endif
                Process.Start(appData);
            }
            catch (Exception ex)
            {
                logger.Error(ex, "OpenConfigFolder Ex");
            }
        }

        #region JsonConfierViewModel

        /// <summary>
        /// The <see cref="JsonConfierViewModel" /> property's name.
        /// </summary>
        public const string JsonConfierViewModelPropertyName = "JsonConfierViewModel";

        private JsonConfierViewModel _JsonConfierViewModel;

        /// <summary>
        /// JsonConfierViewModel
        /// </summary>
        public JsonConfierViewModel JsonConfierViewModel
        {
            get { return _JsonConfierViewModel; }

            set
            {
                if (_JsonConfierViewModel == value) return;

                _JsonConfierViewModel = value;
                NotifyOfPropertyChange(JsonConfierViewModelPropertyName);
            }
        }

        #endregion
    }
}
