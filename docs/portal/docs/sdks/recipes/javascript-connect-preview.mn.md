---
lang: mn
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

SampleDownload-г '@site/src/components/SampleDownload'-аас импортлох;

Энэ жор нь `bootstrapConnectPreviewSession`-ийг
WebSocket залгагчийг `ToriiClient.openConnectWebSocket()` илрүүлсэн. Скрипт
JS SDK замын газрын зургийн Connect хэсгийг толин тусгал: энэ нь тодорхойлогч шинж чанартай байдаг
URI-г урьдчилан харж, дарааллын гүнийн телеметрийг бүртгэж, каноникийг нээнэ
`/v2/connect/ws` төгсгөлийн цэг нь `ws` багцыг ашиглан Node.js програмууд
хөтөчтэй ижил урсгал.

<Жишээ татаж авах
  href="/sdk-recipes/javascript/connect-preview.mjs"
  файлын нэр = "connect-preview.mjs"
  description="Энэ жоронд дурдсан ажиллуулж болох скриптийг татаж авах."
/>

## Урьдчилсан нөхцөл

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

Програмын тал руу залгах шаардлагатай үед `CONNECT_ROLE`-г `app` болгож тохируулна уу.
гар барих. Анхдагч үүрэг нь `wallet`.

## Жишээ скрипт

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

## Ажиллаж, хянах

- Скриптийг `node --env-file=.env connect-preview.mjs` (эсвэл экспорт
  хувьсагчдыг гараар). Скрипт нь урьдчилан үзэх түрийвч URI, deeplink, болон
  WebSocket-ийг нээхээс өмнө дарааллын гүн.
- Скриптийн хэвлэсэн дарааллын хэмжигдэхүүнийг хусах замаар телеметрийн хяналтын самбарыг оруулна уу
  халих/хугацаа дуусах тохиолдол (`connect.queue_depth`, `connect.queue_overflow_total`,
  `connect.queue_expired_total`). `ConnectQueueError` туслахууд нь ялгаруулдаг
  замын зураглалын ангилал зүй (`queueOverflow`, `timeout`) тул OTEL экспортлогчид үлдэх боломжтой
  Android/Swift үйлчлүүлэгчидтэй нийцдэг.
- Гар барих үеийг шалгахын тулд дүрийг `app` болгон соль. The
  залгагч автоматаар зөв токеныг сонгоно (`token_app` vs. `token_wallet`)
  мөн `http→ws`/`https→wss`-г сайжруулснаар хоёр дүр ижилхэн хэсгийг хуваалцдаг.

Энэхүү жор нь Connect-ыг урьдчилан үзэхэд JS5 баримт бичгийн үлдсэн зайг хаадаг
`roadmap.md`-д дурдсан түүх: портал одоо түлхүүр гардуулах дээжийг илгээдэг.
баримтжуулах замын зураглалын шаардлагад нийцүүлэн дарааллын телеметрийн удирдамж
Холболтын сессийн туслахуудын хажууд WebSocket-н танилцуулга.