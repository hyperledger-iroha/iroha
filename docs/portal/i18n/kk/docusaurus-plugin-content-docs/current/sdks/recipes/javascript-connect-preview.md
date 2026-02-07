---
slug: /sdks/recipes/javascript-connect-preview
lang: kk
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v1/connect/ws` socket with `@iroha/iroha-js`.
---

import SampleDownload from '@site/src/components/SampleDownload';

This recipe shows how to combine `bootstrapConnectPreviewSession` with the
WebSocket dialer exposed by `ToriiClient.openConnectWebSocket()`. The script
mirrors the Connect section of the JS SDK roadmap: it mints deterministic
preview URIs, records queue depth telemetry, and opens the canonical
`/v1/connect/ws` endpoint using the `ws` package so Node.js apps can exercise the
same flow as browsers.

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="Download the runnable script referenced in this recipe."
/>

## Prerequisites

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

Set `CONNECT_ROLE` to `app` when you need to dial the application side of the
handshake. The default role is `wallet`.

## Example script

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

## Run & monitor

- Execute the script with `node --env-file=.env connect-preview.mjs` (or export
  the variables manually). The script logs the preview wallet URI, deeplink, and
  queue depth before opening the WebSocket.
- Feed telemetry dashboards by scraping the queue metrics the script prints for
  overflow/expiry cases (`connect.queue_depth`, `connect.queue_overflow_total`,
  `connect.queue_expired_total`). The `ConnectQueueError` helpers emit the
  roadmap taxonomy (`queueOverflow`, `timeout`) so OTEL exporters can stay
  consistent with Android/Swift clients.
- Swap the role to `app` to inspect the application leg of the handshake. The
  dialer automatically chooses the correct token (`token_app` vs. `token_wallet`)
  and upgrades `http→ws`/`https→wss` so both roles share the same snippet.

This recipe closes the remaining JS5 documentation gap for the Connect preview
story called out in `roadmap.md`: the portal now ships a turnkey sample plus
queue telemetry guidance, matching the roadmap requirement to document the
WebSocket walkthrough alongside the Connect session helpers.
