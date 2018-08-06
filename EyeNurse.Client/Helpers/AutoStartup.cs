using EyeNurse.Client.Helpers.StartupManager;
using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace EyeNurse.Client.Helpers
{
    public static class AutoStartup
    {
#if UWP
        public static IStartupManager Instance { get; private set; } = new DesktopBridgeStartupManager();
#else
        //wpf
        public static IStartupManager Instance { get; private set; } = new DesktopStartupManager();
#endif
    }
}
