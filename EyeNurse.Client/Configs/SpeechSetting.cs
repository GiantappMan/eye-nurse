using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.Configs
{
    public class SpeechSetting
    {
        public const string DefaultMessage = "老铁你已经连续使用电脑{0}了，注意休息";
        public bool Enable { get; set; } = true;
        public string Message { get; set; } = DefaultMessage;
        /// <summary>
        /// 仅语音提醒，不锁屏
        /// </summary>
        public bool NeverLockScreen { get; set; } = false;
    }
}
