import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// tauri 架构文档 §1.6：MPA——窗口入口（settings 设置窗 / mask 遮罩 /
// barweb ESP-1 bar 皮肤壳）。皮肤中心窗口已废弃（2026-10-10 单窗口化：
// 皮肤作为设置窗的模块页迁入，见 settings/components/SkinModule.vue）。
// 端口 2420 strictPort 是 tauri.conf.json devUrl 的硬依赖，不能漂移；
// 不用 tauri 脚手架默认的 1420——多个 tauri 项目并行开发时都撞它（2026-10-10）。
export default defineConfig({
  plugins: [vue()],
  server: {
    port: 2420,
    strictPort: true,
    // 允许 dev server 供给仓库根下的文件（页面侧 import 配色真源
    // crates/eyenurse-app/src/ui/theme.json，§3）
    fs: { allow: [fileURLToPath(new URL("..", import.meta.url))] },
  },
  build: {
    rollupOptions: {
      input: {
        settings: fileURLToPath(new URL("./index.html", import.meta.url)),
        mask: fileURLToPath(new URL("./mask.html", import.meta.url)),
        barweb: fileURLToPath(new URL("./barweb.html", import.meta.url)),
      },
    },
  },
});
