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
            RestInterval = GetTimeSpan(settingFrontEnd.RestInterval, new TimeSpan(0, 45, 0));
            RestDuration = GetTimeSpan(settingFrontEnd.RestDuration, new TimeSpan(0, 2, 30));
            ResetTimeout = GetTimeSpan(settingFrontEnd.ResetTimeout, new TimeSpan(0, 5, 0));
            ResetWhenSessionUnlock = settingFrontEnd.ResetWhenSessionUnlock;
            RunWhenStarts = settingFrontEnd.RunWhenStarts;
        }

        private TimeSpan GetTimeSpan(string tsStr, TimeSpan defaultTs)
        {
            bool ok = TimeSpan.TryParse(tsStr, out TimeSpan ts);
            if (ok)
            {
                if (ts.TotalDays >= 1 || ts.TotalDays <= 0)
                    ts = new TimeSpan(Math.Abs(ts.Hours), Math.Abs(ts.Minutes), Math.Abs(ts.Seconds));
                return ts;
            }
            return defaultTs;
        }

        public TimeSpan RestInterval { get; set; }
        public TimeSpan RestDuration { get; set; }
        /// <summary>
        /// 开机启动
        /// </summary>
        public bool RunWhenStarts { get; set; } = true;
        /// <summary>
        /// 登录系统重置时间
        /// </summary>
        public bool ResetWhenSessionUnlock { get; set; } = false;
        /// <summary>
        /// 锁定超过时间就重置
        /// </summary>
        public TimeSpan ResetTimeout { get; set; }
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
            ResetWhenSessionUnlock = setting.ResetWhenSessionUnlock;
            ResetTimeout = setting.ResetTimeout.ToString();
        }
        public string RestInterval { get; set; } = string.Empty;
        public string RestDuration { get; set; } = string.Empty;
        public bool RunWhenStarts { get; set; } = true;
        public bool ResetWhenSessionUnlock { get; set; } = false;
        public string ResetTimeout { get; set; } = string.Empty;
    }
}
