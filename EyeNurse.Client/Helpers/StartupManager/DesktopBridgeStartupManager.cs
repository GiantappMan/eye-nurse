using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Windows.ApplicationModel;

namespace EyeNurse.Client.Helpers.StartupManager
{
    public class DesktopBridgeStartupManager : IStartupManager
    {
        public DesktopBridgeStartupManager()
        {
        }

        public async Task<bool> Set(bool enabled)
        {
#pragma warning disable UWP003 // UWP-only
            var startupTask = await StartupTask.GetAsync("EyeNurse");
#pragma warning restore UWP003 // UWP-only
            if (!enabled && startupTask.State == StartupTaskState.Enabled)
            {
                startupTask.Disable();
                return true;
            }
            else if (enabled)
            {
                var state = await startupTask.RequestEnableAsync();
                switch (state)
                {
                    case StartupTaskState.DisabledByUser:
                        return false;
                    case StartupTaskState.Enabled:
                        return true;
                }
            }
            return false;
        }

        public async Task<bool> Check()
        {
            try
            {
                bool result = false;
#pragma warning disable UWP003 // UWP-only
                var startupTask = await StartupTask.GetAsync("EyeNurse");
#pragma warning restore UWP003 // UWP-only
                switch (startupTask.State)
                {
                    case StartupTaskState.Disabled:
                        result = false;
                        break;
                    case StartupTaskState.DisabledByUser:
                        result = false;
                        break;
                    case StartupTaskState.Enabled:
                        result = true;
                        break;
                }
                return result;
            }
            catch (Exception)
            {
                return false;
            }
        }
    }
}
