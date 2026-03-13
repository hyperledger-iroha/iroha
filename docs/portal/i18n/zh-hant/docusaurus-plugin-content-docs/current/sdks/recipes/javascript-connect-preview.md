---
slug: /sdks/recipes/javascript-connect-preview
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v2/connect/ws` socket with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

從“@site/src/components/SampleDownload”導入 SampleDownload；

這個食譜展示瞭如何將 `bootstrapConnectPreviewSession` 與
`ToriiClient.openConnectWebSocket()` 公開的 WebSocket 撥號器。劇本
反映了 JS SDK 路線圖的 Connect 部分：它具有確定性
預覽 URI、記錄隊列深度遙測並打開規範
使用 `ws` 包的 `/v2/connect/ws` 端點，以便 Node.js 應用程序可以執行
與瀏覽器的流程相同。

<樣本下載
  href="/sdk-recipes/javascript/connect-preview.mjs"
  文件名=“連接預覽.mjs”
  描述=“下載本節中引用的可運行腳本。”
/>

## 先決條件

```bash
npm install @iroha/iroha-js ws
export TORII_URL="https://torii.nexus.example"
export CHAIN_ID="sora-mainnet"
# optional when the node requires auth/API tokens
export IROHA_TORII_AUTH_TOKEN="Bearer …"
export IROHA_TORII_API_TOKEN="sandbox-token"
# optional override when the registration node differs from the WebSocket node
export CONNECT_REGISTRATION_NODE="https://torii.backup.example"
```

當需要撥打應用側時，將`CONNECT_ROLE`設置為`app`
握手。默認角色是 `wallet`。

## 示例腳本

```ts title="connect-preview.mjs"
#!/usr/bin/env node

import WebSocket from "ws";
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const TORII_URL = process.env.TORII_URL ?? process.env.IROHA_TORII_URL ?? "http://127.0.0.1:8080";
const CHAIN_ID =
  process.env.CHAIN_ID ??
  process.env.IROHA_CHAIN_ID ??
  "00000000-0000-0000-0000-000000000000";
const AUTH_TOKEN = process.env.IROHA_TORII_AUTH_TOKEN ?? process.env.AUTH_TOKEN ?? null;
const API_TOKEN = process.env.IROHA_TORII_API_TOKEN ?? process.env.API_TOKEN ?? null;
const REGISTRATION_NODE =
  process.env.CONNECT_REGISTRATION_NODE ?? process.env.REGISTRATION_NODE ?? TORII_URL;
const CONNECT_ROLE = process.env.CONNECT_ROLE ?? "wallet";
const SESSION_METADATA = {
  suite: "js-connect-recipe",
  run_id: new Date().toISOString(),
};

async function main() {
  const client = new ToriiClient(TORII_URL, {
    authToken: AUTH_TOKEN ?? undefined,
    apiToken: API_TOKEN ?? undefined,
  });

  const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
    chainId: CHAIN_ID,
    node: REGISTRATION_NODE,
    metadata: SESSION_METADATA,
  });

  console.log("[connect] sid", session.sid);
  console.log("[connect] wallet URI", preview.walletUri);
  console.log("[connect] deeplink", preview.walletDeeplink);
  console.log("[connect] queue depth", session.queue_depth ?? "unknown");

  const socket = client.openConnectWebSocket({
    sid: session.sid,
    role: CONNECT_ROLE,
    token:
      CONNECT_ROLE === "wallet"
        ? tokens?.wallet ?? session.token_wallet
        : tokens?.app ?? session.token_app,
    WebSocketImpl: WebSocket,
    protocols: ["iroha-connect"],
  });

  socket.addEventListener("open", () => {
    console.log("[ws] connected");
  });

  socket.addEventListener("message", (event) => {
    console.log("[ws] payload", event.data);
  });

  socket.addEventListener("close", (event) => {
    console.log("[ws] closed", { code: event.code, reason: event.reason });
  });

  socket.addEventListener("error", (error) => {
    console.error("[ws] error", error?.message ?? error);
  });

  const shutdownTimer = setTimeout(() => {
    console.log("[ws] closing after 5s demo window");
    socket.close(1000, "connect-preview-demo");
  }, 5_000);

  socket.addEventListener("close", () => {
    clearTimeout(shutdownTimer);
  });
}

main().catch((error) => {
  if (error instanceof ConnectQueueError) {
    if (error.kind === "overflow") {
      console.error("[queue] overflow", { limit: error.limit });
    } else if (error.kind === "expired") {
      console.error("[queue] expired", { ttlMs: error.ttlMs });
    }
  } else {
    console.error("[connect] error", error);
  }
  process.exit(1);
});
```

## 運行和監控

- 使用 `node --env-file=.env connect-preview.mjs` 執行腳本（或導出
  手動更改變量）。該腳本記錄預覽錢包 URI、深度鏈接和
  打開 WebSocket 之前的隊列深度。
- 通過抓取腳本打印的隊列指標來提供遙測儀表板
  溢出/過期情況（`connect.queue_depth`、`connect.queue_overflow_total`、
  `connect.queue_expired_total`）。 `ConnectQueueError` 助手發出
  路線圖分類（`queueOverflow`、`timeout`），以便 OTEL 出口商可以留下來
  與 Android/Swift 客戶端一致。
- 將角色交換為 `app` 以檢查握手的應用程序腿。的
  撥號器自動選擇正確的令牌（`token_app` 與 `token_wallet`）
  併升級 `http→ws`/`https→wss`，以便兩個角色共享相同的代碼片段。

本秘籍彌補了 Connect 預覽版中剩餘的 JS5 文檔空白
`roadmap.md` 中提到的故事：該門戶現在提供交鑰匙樣品以及
隊列遙測指導，匹配路線圖要求以記錄
WebSocket 演練以及 Connect 會話幫助程序。