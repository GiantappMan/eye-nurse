using EyeNurse.Client.Helpers;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EyeNurse.Client.Services
{
    public class AppServices
    {
        #region public methods

        public async Task<T> LoadConfigAsync<T>(string path = null) where T : new()
        {
            try
            {
                if (string.IsNullOrEmpty(path))
                    path = GetDefaultPath<T>();

                var config = await JsonHelper.JsonDeserializeFromFileAsync<T>(path);
                if (config == null)
                    config = new T();
                return config;
            }
            catch (Exception ex)
            {
                return default(T);
            }
        }

        public async Task SaveConfigAsync<T>(T data, string path = null) where T : new()
        {
            if (string.IsNullOrEmpty(path))
                path = GetDefaultPath<T>();

            var json = await JsonHelper.JsonSerializeAsync(data, path);
        }

        public string GetDefaultPath<T>() where T : new()
        {
            var rootDir = Environment.CurrentDirectory;
            return $"{rootDir}\\Config\\{typeof(T).Name}.json";
        }

        #endregion
    }
}
