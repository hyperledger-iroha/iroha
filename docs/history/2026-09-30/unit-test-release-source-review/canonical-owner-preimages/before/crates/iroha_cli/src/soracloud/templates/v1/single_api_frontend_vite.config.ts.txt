import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

function normalizeProxyTarget(envName: string, fallback: string) {
  const value = process.env[envName] ?? fallback;
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

const apiProxyTarget = normalizeProxyTarget(
  "SORACLOUD_SINGLE_API_DEV_PROXY_TARGET",
  "http://127.0.0.1:8787"
);

export default defineConfig({
  plugins: [vue()],
  server: {
    host: "0.0.0.0",
    port: 5173,
    proxy: {
      "/api": {
        target: apiProxyTarget,
        changeOrigin: true
      }
    }
  }
});
