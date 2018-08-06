using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.Helpers.StartupManager
{
    public interface IStartupManager
    {
        Task<bool> Check();
        Task<bool> Set(bool enable);
    }
}
