import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true,
    watch: {
      // Rust 侧改动由 tauri CLI 负责重启；避免 Vite 监听 src-tauri/target 与 cargo 并发写文件产生 EBUSY
      ignored: ["**/src-tauri/**"],
    },
  },
});
