---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0df3d72cb822e0fef5201d5a5d25b8588378f51e3e3106c73def669d68b1c674
source_last_modified: "2025-12-29T18:16:35.166929+00:00"
translation_last_reviewed: 2026-02-07
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v2/connect/ws` socket with `@iroha/iroha-js`.
slug: /sdks/recipes/javascript-connect-preview
translator: machine-google-reviewed
---

从“@site/src/components/SampleDownload”导入 SampleDownload；

这个食谱展示了如何将 `bootstrapConnectPreviewSession` 与
`ToriiClient.openConnectWebSocket()` 公开的 WebSocket 拨号器。剧本
反映了 JS SDK 路线图的 Connect 部分：它具有确定性
预览 URI、记录队列深度遥测并打开规范
使用 `ws` 包的 `/v2/connect/ws` 端点，以便 Node.js 应用程序可以执行
与浏览器的流程相同。

<样本下载
  href="/sdk-recipes/javascript/connect-preview.mjs"
  文件名=“连接预览.mjs”
  描述=“下载本节中引用的可运行脚本。”
/>

## 先决条件

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

当需要拨打应用侧时，将`CONNECT_ROLE`设置为`app`
握手。默认角色是 `wallet`。

## 示例脚本

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

## 运行和监控

- 使用 `node --env-file=.env connect-preview.mjs` 执行脚本（或导出
  手动更改变量）。该脚本记录预览钱包 URI、深度链接和
  打开 WebSocket 之前的队列深度。
- 通过抓取脚本打印的队列指标来提供遥测仪表板
  溢出/过期情况（`connect.queue_depth`、`connect.queue_overflow_total`、
  `connect.queue_expired_total`）。 `ConnectQueueError` 助手发出
  路线图分类（`queueOverflow`、`timeout`），以便 OTEL 出口商可以留下来
  与 Android/Swift 客户端一致。
- 将角色交换为 `app` 以检查握手的应用程序腿。的
  拨号器自动选择正确的令牌（`token_app` 与 `token_wallet`）
  并升级 `http→ws`/`https→wss`，以便两个角色共享相同的代码片段。

本秘籍弥补了 Connect 预览版中剩余的 JS5 文档空白
`roadmap.md` 中提到的故事：该门户现在提供交钥匙样品以及
队列遥测指导，匹配路线图要求以记录
WebSocket 演练以及 Connect 会话帮助程序。