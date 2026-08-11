import { defineConfig, loadEnv } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const edgeBackendOrigin =
    env.VITE_EDGE_BACKEND_ORIGIN || env.EDGE_BACKEND_ORIGIN || "http://127.0.0.1:8081";

  return {
    plugins: [vue()],
    server: {
      proxy: {
        "/api/edge/dashboard": {
          target: edgeBackendOrigin,
          changeOrigin: true,
        },
        "/ws/edge/progress": {
          target: edgeBackendOrigin,
          changeOrigin: true,
          ws: true,
        },
        "/healthz": {
          target: edgeBackendOrigin,
          changeOrigin: true,
        },
        "/readyz": {
          target: edgeBackendOrigin,
          changeOrigin: true,
        },
      },
    },
  };
});
