// EyeNurse 皮肤桥（ESP-1，docs/v3-skin-protocol.md §3.1）。
// 宿主壳页把本包以 sandbox iframe 加载（opaque origin）——皮肤拿不到 Tauri IPC，
// 与宿主的全部交互只有 postMessage。本文件把它包成一个极小的 API：
//
//   eyenurse.onState(cb)    状态推送（1Hz + 变化即发）：{ remainingMs, resting,
//                           locked, hasPassword } 或 bar 表面的 { countdownMs,
//                           intervalMs, pausedMs, resting, topmost }
//   eyenurse.onTexts(cb)    整包文案（i18n）
//   eyenurse.onColors(cb)   当前配色（逐键回退后的完整键值）
//   eyenurse.drag()         悬浮条系统拖动（等价 GDI 条拖住即走）
//   eyenurse.openSettings() 打开设置窗
//   eyenurse.setTopmost(v)  悬浮条置顶开关
//   eyenurse.endRest()      未锁时结束休息
//   eyenurse.tryUnlock(pwd) 提交锁屏密码（宿主限速：1 次/秒，连错 5 次冷却 60s）
//
// 加载即自动向宿主握手（eyenurse-hello）——3 秒内未握手会被宿主判定为损坏
// 皮肤并回退内置界面，所以本文件必须先于其他脚本加载。
(function () {
  "use strict";
  var listeners = { state: [], texts: [], colors: [] };

  function send(action, payload) {
    parent.postMessage(
      Object.assign({ type: "eyenurse-action", action: action }, payload || {}),
      "*"
    );
  }

  window.eyenurse = {
    onState: function (cb) { listeners.state.push(cb); },
    onTexts: function (cb) { listeners.texts.push(cb); },
    onColors: function (cb) { listeners.colors.push(cb); },
    drag: function () { send("drag"); },
    openSettings: function () { send("openSettings"); },
    setTopmost: function (on) { send("setTopmost", { on: !!on }); },
    endRest: function () { send("endRest"); },
    tryUnlock: function (password) {
      send("tryUnlock", { password: String(password == null ? "" : password) });
    },
  };

  window.addEventListener("message", function (ev) {
    var d = ev.data || {};
    if (d.type === "eyenurse-state") {
      listeners.state.forEach(function (cb) { cb(d); });
    } else if (d.type === "eyenurse-texts") {
      listeners.texts.forEach(function (cb) { cb(d.texts || {}); });
    } else if (d.type === "eyenurse-colors") {
      listeners.colors.forEach(function (cb) { cb(d.colors || {}); });
    }
  });

  // 握手：宿主收到后才显示本 iframe（防损坏皮肤白屏遮脸）
  parent.postMessage({ type: "eyenurse-hello" }, "*");
})();
