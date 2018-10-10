using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.Events
{
    public class ServiceInitEvent
    {
        public bool Initialized { get; set; }
        public bool IsInitializing { get; set; }
    }
}
