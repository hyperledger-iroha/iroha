import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

function normalizeProxyTarget(envName: string, fallback: string) {
  const value = process.env[envName] ?? fallback;
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function rewriteApiPrefix(path: string) {
  return path.replace(/^\/api/, "");
}

const liveProxyTarget = normalizeProxyTarget(
  "SORACLOUD_LIVE_DEV_PROXY_TARGET",
  "http://127.0.0.1:8787"
);
const vaultProxyTarget = normalizeProxyTarget(
  "SORACLOUD_VAULT_DEV_PROXY_TARGET",
  "http://127.0.0.1:8788"
);

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    proxy: {
      "/api/auth": {
        target: vaultProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      },
      "/api/v1/user": {
        target: vaultProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      },
      "/api/v1/health": {
        target: liveProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      },
      "/api/v1/search": {
        target: liveProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      },
      "/api/v1/airports": {
        target: liveProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      },
      "/api/v1/filters": {
        target: liveProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      },
      "/api/v1/luxury": {
        target: liveProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      },
      "/api/v1/links": {
        target: liveProxyTarget,
        changeOrigin: true,
        rewrite: rewriteApiPrefix
      }
    }
  }
});
