import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// tauri 架构文档 §1.6：MPA——两个窗口两个入口（settings 设置窗 / mask 遮罩）。
// 端口 1420 strictPort 是 tauri.conf.json devUrl 的硬依赖，不能漂移。
export default defineConfig({
  plugins: [vue()],
  server: {
    port: 1420,
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
      },
    },
  },
});
