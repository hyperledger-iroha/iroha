---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-connect-preview
title: JavaScript Connect プレビュー レシピ
description: Connect プレビューセッションを準備し、キューのテレメトリを出力し、`@iroha/iroha-js` で `/v2/connect/ws` ソケットに接続します。
---

import SampleDownload from '@site/src/components/SampleDownload';

このレシピは `bootstrapConnectPreviewSession` と `ToriiClient.openConnectWebSocket()` が公開する WebSocket ダイアラを組み合わせる方法を示します。スクリプトは JS SDK の Connect セクションの roadmap を反映し、決定的なプレビュー URI を生成し、キュー深度のテレメトリを記録し、`ws` パッケージを使って `/v2/connect/ws` のカノニカルなエンドポイントを開き、Node.js アプリがブラウザと同じフローを実行できるようにします。

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="このレシピで参照している実行可能スクリプトをダウンロードします。"
/>

## 前提条件

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

アプリ側のハンドシェイクを開く必要がある場合は `CONNECT_ROLE` を `app` に設定します。既定のロールは `wallet` です。

## サンプルスクリプト

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

## 実行と監視

- `node --env-file=.env connect-preview.mjs` でスクリプトを実行します（または手動で環境変数をエクスポートします）。スクリプトは WebSocket を開く前にプレビュー wallet URI、deeplink、キュー深度をログに出力します。
- スクリプトが overflow/expiry 時に出力するキュー指標（`connect.queue_depth`, `connect.queue_overflow_total`, `connect.queue_expired_total`）を収集してテレメトリダッシュボードに取り込みます。`ConnectQueueError` のヘルパーは roadmap のタクソノミ（`queueOverflow`, `timeout`）を出力するため、OTEL エクスポータを Android/Swift クライアントと整合させられます。
- 役割を `app` に切り替えてハンドシェイクのアプリ側を確認します。ダイアラは正しいトークン（`token_app` vs. `token_wallet`）を自動選択し、`http→ws`/`https→wss` を昇格して両ロールが同じスニペットを共有できるようにします。

このレシピは `roadmap.md` で指摘された Connect プレビューの JS5 ドキュメントギャップを埋めます。ポータルは turnkey なサンプルとキューのテレメトリガイダンスを提供し、Connect セッションヘルパーと WebSocket walkthrough を併記するという roadmap 要件を満たします。

