import { defineConfig, loadEnv } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const centerBackendOrigin =
    env.VITE_CENTER_BACKEND_ORIGIN || env.CENTER_BACKEND_ORIGIN || "http://127.0.0.1:8080";

  return {
    plugins: [vue()],
    server: {
      proxy: {
        "/api/center": {
          target: centerBackendOrigin,
          changeOrigin: true,
        },
        "/api/disk": {
          target: centerBackendOrigin,
          changeOrigin: true,
        },
        "/api/edge/auth": {
          target: centerBackendOrigin,
          changeOrigin: true,
        },
        "/ws/center": {
          target: centerBackendOrigin,
          changeOrigin: true,
          ws: true,
        },
        "/healthz": {
          target: centerBackendOrigin,
          changeOrigin: true,
        },
        "/readyz": {
          target: centerBackendOrigin,
          changeOrigin: true,
        },
      },
    },
  };
});
