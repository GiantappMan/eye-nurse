using Common.Apps.Helpers;
using Common.Apps.Services;
using EyeNurse.Models.UserConfigs;
using EyeNurse.Services;
using System;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text.Json;
using UserConfigs = EyeNurse.Models.UserConfigs;

namespace EyeNurse.Apis
{
#pragma warning disable CS0618 // 类型或成员已过时
    [ClassInterface(ClassInterfaceType.AutoDual)]
#pragma warning restore CS0618 // 类型或成员已过时
    [ComVisible(true)]
    public class ClientApi
    {
        private readonly EyeNurseService _eyeNurseService;

        public ClientApi(EyeNurseService eyeNurseService)
        {
            _eyeNurseService = eyeNurseService;
        }


        public string? GetCurrentLanguage()
        {
            var settings = _eyeNurseService.LoadUserConfig<UserConfigs.Languages>();
            return settings.CurrentLan;
        }
        /// <summary>
        /// 设置当前语言
        /// </summary>
        /// <param name="lan"></param>
        public void SetCurrentLanguage(string lan)
        {
            _eyeNurseService?.SaveUserConfig(new UserConfigs.Languages()
            {
                CurrentLan = lan
            });

            var app = IocService.GetService<App>();
            app?.UpdateNotifyIconText(lan);
        }

        //系统限制复杂类型返回前端收不到
        //https://github.com/MicrosoftEdge/WebView2Feedback/issues/292
        public string GetSettings()
        {
            var settings = new UserConfigs.SettingFrontEnd(_eyeNurseService.LoadUserConfig<UserConfigs.Setting>());
            var json = JsonSerializer.Serialize(settings);
            //显示给UI才调用，节约不必要的检查
            settings.RunWhenStarts = _eyeNurseService.CheckRunWhenStarts();
            return json;
        }
        public string SetSettings(string json)
        {
            var tmpSetting = JsonSerializer.Deserialize<UserConfigs.SettingFrontEnd>(json);
            if (tmpSetting == null)
                return GetSettings();
            UserConfigs.Setting setting = new UserConfigs.Setting(tmpSetting);
            _eyeNurseService?.SaveUserConfig(setting);
            _eyeNurseService?.ApplySetting(setting);

            return GetSettings();
        }

        public string ResetSettings()
        {
            UserConfigs.Setting defaultConfig = new();
            _eyeNurseService?.SaveUserConfig(defaultConfig);
            var json = JsonSerializer.Serialize(defaultConfig);
            _eyeNurseService?.ApplySetting(defaultConfig);
            return json;
        }
    }
}
