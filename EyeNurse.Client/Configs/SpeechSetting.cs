using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.Configs
{
    public class SpeechSetting
    {
        public bool Enable { get; set; } = true;
        public string Message { get; set; } = "老铁你已经玩了{0}了，注意保护眼睛";
    }
}
