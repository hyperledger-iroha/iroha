---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0df3d72cb822e0fef5201d5a5d25b8588378f51e3e3106c73def669d68b1c674
source_last_modified: "2025-11-14T11:46:45.985700+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: JavaScript Connect پیش نظارہ ترکیب
description: Connect پیش نظارہ سیشنز تیار کریں، قطار کی ٹیلی میٹری خارج کریں، اور `@iroha/iroha-js` کے ساتھ `/v1/connect/ws` ساکٹ ڈائل کریں۔
slug: /sdks/recipes/javascript-connect-preview
---

import SampleDownload from '@site/src/components/SampleDownload';

یہ ترکیب دکھاتی ہے کہ `bootstrapConnectPreviewSession` کو `ToriiClient.openConnectWebSocket()` کے ذریعے فراہم کردہ WebSocket ڈائلر کے ساتھ کیسے ملایا جائے۔ اسکرپٹ JS SDK roadmap کے Connect حصے کی عکاسی کرتا ہے: یہ تعیینی پیش نظارہ URIs بناتا ہے، قطار کی گہرائی کی ٹیلی میٹری ریکارڈ کرتا ہے، اور `ws` پیکج کے ذریعے کینونیکل `/v1/connect/ws` اینڈپوائنٹ کھولتا ہے تاکہ Node.js ایپس براؤزرز والا ہی فلو چلا سکیں۔

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="اس ترکیب میں حوالہ دیا گیا قابلِ اجرا اسکرپٹ ڈاؤن لوڈ کریں۔"
/>

## پیشگی تقاضے

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

جب آپ کو ہینڈ شیک کے ایپ سائیڈ کو ڈائل کرنا ہو تو `CONNECT_ROLE` کو `app` پر سیٹ کریں۔ ڈیفالٹ رول `wallet` ہے۔

## مثالی اسکرپٹ

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

## چلائیں اور مانیٹر کریں

- `node --env-file=.env connect-preview.mjs` سے اسکرپٹ چلائیں (یا متغیرات دستی طور پر ایکسپورٹ کریں)۔ اسکرپٹ WebSocket کھولنے سے پہلے preview wallet URI، deeplink اور قطار کی گہرائی لاگ کرتا ہے۔
- ٹیلی میٹری ڈیش بورڈز کو فیڈ کرنے کے لیے اسکرپٹ کے پرنٹ کردہ queue میٹرکس (overflow/expiry کیسز میں) جمع کریں (`connect.queue_depth`, `connect.queue_overflow_total`, `connect.queue_expired_total`)۔ `ConnectQueueError` ہیلپرز roadmap ٹیکسونومی (`queueOverflow`, `timeout`) خارج کرتے ہیں تاکہ OTEL ایکسپورٹرز Android/Swift کلائنٹس کے مطابق رہیں۔
- ہینڈ شیک کی ایپ لیگ دیکھنے کے لیے رول کو `app` پر سوئچ کریں۔ ڈائلر خودکار طور پر درست ٹوکن (`token_app` بمقابلہ `token_wallet`) منتخب کرتا ہے اور `http→ws`/`https→wss` اپ گریڈ کرتا ہے تاکہ دونوں رول ایک ہی snippet شیئر کریں۔

یہ ترکیب `roadmap.md` میں بیان کردہ Connect پیش نظارہ اسٹوری کے لیے باقی JS5 ڈاکیومنٹیشن خلا کو بند کرتی ہے: پورٹل اب ایک تیار نمونہ اور قطار ٹیلی میٹری گائیڈنس فراہم کرتا ہے، جو Connect سیشن ہیلپرز کے ساتھ WebSocket walkthrough دستاویز کرنے کی roadmap ضرورت کو پورا کرتا ہے۔

