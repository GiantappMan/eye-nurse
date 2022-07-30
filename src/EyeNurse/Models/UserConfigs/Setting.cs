using System;

namespace EyeNurse.Models.UserConfigs
{
    public class Setting
    {
        public Setting()
        {

        }
        public Setting(SettingFrontEnd settingFrontEnd)
        {
            bool ok = TimeSpan.TryParse(settingFrontEnd.RestInterval, out TimeSpan restInterval);
            if (ok)
            {
                if (restInterval.TotalDays >= 1 || restInterval.TotalDays <= 0)
                    restInterval = new TimeSpan(Math.Abs(restInterval.Hours), Math.Abs(restInterval.Minutes), Math.Abs(restInterval.Seconds));
                RestInterval = restInterval;
            }
            ok = TimeSpan.TryParse(settingFrontEnd.RestDuration, out TimeSpan restDuration);
            if (ok)
            {
                if (restDuration.TotalDays >= 1 || restDuration.TotalDays <= 0)
                    restDuration = new TimeSpan(Math.Abs(restDuration.Hours), Math.Abs(restDuration.Minutes), Math.Abs(restDuration.Seconds));
                RestDuration = restDuration;
            }
            RunWhenStarts = settingFrontEnd.RunWhenStarts;
        }
        public TimeSpan RestInterval { get; set; } = new TimeSpan(0, 45, 0);
        public TimeSpan RestDuration { get; set; } = new TimeSpan(0, 2, 30);
        //开机启动
        public bool RunWhenStarts { get; set; } = true;
    }

    public class SettingFrontEnd
    {
        public SettingFrontEnd()
        {

        }
        public SettingFrontEnd(Setting setting)
        {
            RestInterval = setting.RestInterval.ToString();
            RestDuration = setting.RestDuration.ToString();
            RunWhenStarts = setting.RunWhenStarts;
        }
        public string RestInterval { get; set; } = string.Empty;
        public string RestDuration { get; set; } = string.Empty;
        //开机启动
        public bool RunWhenStarts { get; set; } = true;
    }
}
