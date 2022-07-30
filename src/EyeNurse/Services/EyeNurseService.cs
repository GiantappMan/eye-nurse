using Common.Apps.Helpers;
using Common.Apps.Services;
using MultiLanguageForXAML;
using MultiLanguageForXAML.DB;
using System.IO;
using System.Reflection;
using System.Threading.Tasks;
using UserConfigs = EyeNurse.Models.UserConfigs;


namespace EyeNurse.Services
{
    /// <summary>
    /// 全局服务
    /// </summary>
    public class EyeNurseService
    {
        #region fileds

        readonly DesktopStartupHelper _desktopStartupHelper;
        readonly AppService _appService;
        readonly ConfigService _configService;
        #endregion

        #region construct

        public EyeNurseService(AppService appService, ConfigService configService)
        {
            ApptEntryDir = Path.GetDirectoryName(Assembly.GetEntryAssembly()!.Location)!;
            _appService = appService;
            _configService = configService;
            string exePath = Assembly.GetEntryAssembly()!.Location.Replace(".dll", ".exe");
            _desktopStartupHelper = new("Eye Nurse 2", exePath);
        }

        #endregion

        #region properties
        public string ApptEntryDir { get; private set; }
        #endregion

        #region private

        #endregion

        #region public
        internal bool CheckRunWhenStarts()
        {
            var r = _desktopStartupHelper.Check();
            return r;
        }
        internal void ApplySetting(UserConfigs.Setting setting)
        {
            _desktopStartupHelper.Set(setting.RunWhenStarts);
        }
        internal void Init()
        {
            //全局捕获异常
            _appService.CatchApplicationError();
            var lanSetting = LoadUserConfig<UserConfigs.Languages>();
            //多语言初始化
            string i18nDir = Path.Combine(ApptEntryDir, "Assets\\Languages");
            LanService.Init(new JsonFileDB(i18nDir), true, lanSetting?.CurrentLan, "en");

            var appSetting = LoadUserConfig<UserConfigs.Setting>();
            ApplySetting(appSetting);
        }
        internal T LoadUserConfig<T>() where T : new()
        {
            var res = _configService.LoadUserConfig<T>();
            return res;
        }
        internal Task<T> LoadUserConfigAsync<T>() where T : new()
        {
            return Task.Run(() => LoadUserConfig<T>());
        }
        internal void SaveUserConfig(object config)
        {
            _configService.SaveUserConfig(config);
        }
        internal Task SaveUserConfigAsync(object config)
        {
            return Task.Run(() => SaveUserConfig(config));
        }
        #endregion
    }
}
