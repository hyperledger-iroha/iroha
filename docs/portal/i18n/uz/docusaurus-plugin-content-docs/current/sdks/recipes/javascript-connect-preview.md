---
slug: /sdks/recipes/javascript-connect-preview
lang: uz
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v2/connect/ws` socket with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownloadni '@site/src/components/SampleDownload'dan import qilish;

Ushbu retsept `bootstrapConnectPreviewSession` bilan qanday birlashtirishni ko'rsatadi
WebSocket teruvchisi `ToriiClient.openConnectWebSocket()` tomonidan ochilgan. Ssenariy
JS SDK yo'l xaritasining Connect bo'limini aks ettiradi: u deterministikdir
URI-larni oldindan ko'rish, navbat chuqurligi telemetriyasini yozib olish va kanonikni ochish
`/v2/connect/ws` so'nggi nuqtasi `ws` paketidan foydalangan holda Node.js ilovalari
brauzerlar bilan bir xil oqim.

<Namunani yuklab olish
  href="/sdk-recipes/javascript/connect-preview.mjs"
  fayl nomi = "connect-preview.mjs"
  description="Ushbu retseptda havola qilingan ishga tushiriladigan skriptni yuklab oling."
/>

## Old shartlar

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

Ilova tomonini terish kerak bo'lganda `CONNECT_ROLE` ni `app` ga o'rnating.
qo'l siqish. Odatiy rol - `wallet`.

## Misol skript

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

## Ishga tushirish va kuzatish

- Skriptni `node --env-file=.env connect-preview.mjs` bilan bajaring (yoki eksport
  o'zgaruvchilar qo'lda). Skript oldindan ko'rish hamyon URI, deeplink, va jurnallar
  WebSocketni ochishdan oldin navbat chuqurligi.
- Skript bosib chiqaradigan navbat ko'rsatkichlarini o'chirish orqali telemetriya asboblar panelini ta'minlang
  toshib ketish/muddati tugash holatlari (`connect.queue_depth`, `connect.queue_overflow_total`,
  `connect.queue_expired_total`). `ConnectQueueError` yordamchilari chiqaradi
  yo'l xaritasi taksonomiyasi (`queueOverflow`, `timeout`), shuning uchun OTEL eksportchilari qolishlari mumkin
  Android/Swift mijozlariga mos keladi.
- Qo'l siqishning dastur oyog'ini tekshirish uchun rolni `app` ga almashtiring. The
  teruvchi avtomatik ravishda to'g'ri tokenni tanlaydi (`token_app` va `token_wallet`)
  va `http→ws`/`https→wss` ni yangilaydi, shuning uchun ikkala rol ham bir xil parchani taqsimlaydi.

Ushbu retsept Connectni oldindan ko'rish uchun qolgan JS5 hujjatidagi bo'shliqni yopadi
hikoya `roadmap.md` da aytilgan: portal endi kalit namunani yuboradi va plyus
hujjatlashtirish uchun yo'l xaritasi talabiga mos keladigan telemetriya bo'yicha ko'rsatmalar
Connect seansi yordamchilari bilan bir qatorda WebSocket ko'rsatmalari.