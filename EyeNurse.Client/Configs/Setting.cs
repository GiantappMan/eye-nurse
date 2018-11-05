using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.Configs
{
    public class Setting
    {
        public AppSetting App { get; set; } = new AppSetting();
        public WindowPosition WindowPosition { get; set; } = new WindowPosition();
        public SpeechSetting Speech { get; set; } = new SpeechSetting();
    }
}
