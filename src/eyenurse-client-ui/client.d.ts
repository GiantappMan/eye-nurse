interface API {
  GetSettings(): Promise<string>;
  SetSettings(settingJson: string);
  ResetSettings(): Promise<string>;
  SetCurrentLanguage(lan: string): Promise;
  GetCurrentLanguage(): Promise<string>;
}
interface Window {
  chrome: {
    webview: {
      hostObjects: {
        sync: {
          api: API;
        };
        api: API;
      };
    };
  };
}
