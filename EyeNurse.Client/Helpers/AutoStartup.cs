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
    //抄写的
    //https://github.com/shadowsocks/shadowsocks-windows/blob/675a5801d84b0945a6755be20e9ea9713009c7a0/shadowsocks-csharp/Controller/System/AutoStartup.cs
    public static class AutoStartup
    {
        // Don't use Application.ExecutablePath
        // see https://stackoverflow.com/questions/12945805/odd-c-sharp-path-issue
        private static NLog.Logger logger = NLog.LogManager.GetCurrentClassLogger();
        private static readonly string ExecutablePath = Assembly.GetEntryAssembly().Location;

        private static string Key = "EyeNurse"; //+ GetMd5(ExecutablePath);

        private static string GetMd5(string filename)
        {
            using (var md5 = MD5.Create())
            {
                using (var stream = File.OpenRead(filename))
                {
                    var hash = md5.ComputeHash(stream);
                    return BitConverter.ToString(hash).Replace("-", "").ToLowerInvariant();
                }
            }
        }

        public static RegistryKey OpenRegKey(string name, bool writable, RegistryHive hive = RegistryHive.CurrentUser)
        {
            // we are building x86 binary for both x86 and x64, which will
            // cause problem when opening registry key
            // detect operating system instead of CPU
            if (string.IsNullOrEmpty(name)) throw new ArgumentException(nameof(name));
            try
            {
                RegistryKey userKey = RegistryKey.OpenBaseKey(hive,
                        Environment.Is64BitOperatingSystem ? RegistryView.Registry64 : RegistryView.Registry32)
                    .OpenSubKey(name, writable);
                return userKey;
            }
            catch (ArgumentException ae)
            {
                MessageBox.Show("OpenRegKey: " + ae.ToString());
                return null;
            }
            catch (Exception e)
            {
                logger.Error(e);
                return null;
            }
        }

        public static bool Set(bool enabled)
        {
            RegistryKey runKey = null;
            try
            {
                runKey = OpenRegKey(@"Software\Microsoft\Windows\CurrentVersion\Run", true);
                if (runKey == null)
                {
                    logger.Error(@"Cannot find HKCU\Software\Microsoft\Windows\CurrentVersion\Run");
                    return false;
                }
                if (enabled)
                {
                    runKey.SetValue(Key, ExecutablePath);
                }
                else
                {
                    runKey.DeleteValue(Key);
                }
                return true;
            }
            catch (Exception e)
            {
                logger.Error(e);
                return false;
            }
            finally
            {
                if (runKey != null)
                {
                    try
                    {
                        runKey.Close();
                        runKey.Dispose();
                    }
                    catch (Exception e)
                    {
                        logger.Error(e);
                    }
                }
            }
        }

        public static bool Check()
        {
            RegistryKey runKey = null;
            try
            {
                runKey = OpenRegKey(@"Software\Microsoft\Windows\CurrentVersion\Run", true);
                if (runKey == null)
                {
                    logger.Error(@"Cannot find HKCU\Software\Microsoft\Windows\CurrentVersion\Run");
                    return false;
                }
                string[] runList = runKey.GetValueNames();
                foreach (string item in runList)
                {
                    if (item.Equals(Key, StringComparison.OrdinalIgnoreCase))
                        return true;
                    //else if (item.Equals("Shadowsocks", StringComparison.OrdinalIgnoreCase)) // Compatibility with older versions
                    //{
                    //    string value = Convert.ToString(runKey.GetValue(item));
                    //    if (ExecutablePath.Equals(value, StringComparison.OrdinalIgnoreCase))
                    //    {
                    //        runKey.DeleteValue(item);
                    //        runKey.SetValue(Key, ExecutablePath);
                    //        return true;
                    //    }
                    //}
                }
                return false;
            }
            catch (Exception e)
            {
                logger.Error(e);
                return false;
            }
            finally
            {
                if (runKey != null)
                {
                    try
                    {
                        runKey.Close();
                        runKey.Dispose();
                    }
                    catch (Exception e)
                    {
                        logger.Error(e);
                    }
                }
            }
        }
    }
}
