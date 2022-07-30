import type { Setting } from "./models";

export async function getSettings(): Promise<Setting | null> {
  if (!window.chrome.webview) return null;
  const { api } = window.chrome.webview.hostObjects;
  var json = await api.GetSettings();

  return JSON.parse(json);
}
export async function SetSettings(setting: Setting) {
  if (!window.chrome.webview) return null;
  const { api } = window.chrome.webview.hostObjects;
  var json = await api.SetSettings(JSON.stringify(setting));

  return JSON.parse(json);
}
export async function ResetSettings(): Promise<Setting | null> {
  if (!window.chrome.webview) return null;
  const { api } = window.chrome.webview.hostObjects;
  const json = await api.ResetSettings();

  return JSON.parse(json);
}
export async function SetCurrentLanguage(language: string) {
  if (!window.chrome.webview) return null;
  const { api } = window.chrome.webview.hostObjects;
  await api.SetCurrentLanguage(language);
}

export async function GetCurrentLanguage() {
  const { api } = window.chrome.webview.hostObjects;
  var r = await api.GetCurrentLanguage();

  return r;
}
