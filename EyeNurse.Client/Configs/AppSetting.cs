using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.Configs
{
    public class AppSetting
    {
        public TimeSpan AlarmInterval { get; set; } = new TimeSpan(0, 45, 0);
        public TimeSpan RestTime { get; set; } = new TimeSpan(0, 2, 30);
    }
}
