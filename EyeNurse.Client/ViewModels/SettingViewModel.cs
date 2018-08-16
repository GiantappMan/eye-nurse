using Caliburn.Micro;
using EyeNurse.Client.Configs;
using EyeNurse.Client.Services;
using JsonConfiger;
using JsonConfiger.Models;
using JsonConfiger.Utils;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.ViewModels
{
    public class SettingViewModel : Screen
    {
        JCrService _jcrService = new JCrService();
        EyeNurseService _appServices;

        public SettingViewModel(EyeNurseService appService)
        {
            _appServices = appService;
        }

        protected async override void OnInitialize()
        {
            var config = await JsonHelper.JsonDeserializeFromFileAsync<dynamic>(_appServices.ConfigFilePath);
            string descPath = Path.Combine(AppDomain.CurrentDomain.SetupInformation.ApplicationBase, "Configs\\setting.desc.json");
            var descConfig = await JsonHelper.JsonDeserializeFromFileAsync<dynamic>(descPath);
            JsonConfierViewModel = _jcrService.GetVM(config, descConfig);
            base.OnInitialize();
        }

        public void Save()
        {

        }
        public void Cancel()
        {

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
