---
lang: ka
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0df3d72cb822e0fef5201d5a5d25b8588378f51e3e3106c73def669d68b1c674
source_last_modified: "2025-12-29T18:16:35.166929+00:00"
translation_last_reviewed: 2026-02-07
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v1/connect/ws` socket with `@iroha/iroha-js`.
slug: /sdks/recipes/javascript-connect-preview
translator: machine-google-reviewed
---

SampleDownload-ის იმპორტი '@site/src/components/SampleDownload'-დან;

ეს რეცეპტი გვიჩვენებს, თუ როგორ უნდა დააკავშიროთ `bootstrapConnectPreviewSession`
WebSocket აკრიფეთ `ToriiClient.openConnectWebSocket()`. სცენარი
ასახავს JS SDK საგზაო რუქის Connect მონაკვეთს: ის დეტერმინისტულია
გადახედეთ URI-ებს, ჩაიწერს რიგის სიღრმის ტელემეტრიას და ხსნის კანონიკურს
`/v1/connect/ws` საბოლოო წერტილი `ws` პაკეტის გამოყენებით, რათა Node.js აპებმა შეძლონ განახორციელონ
იგივე ნაკადი, როგორც ბრაუზერები.

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  ფაილის სახელი = "connect-preview.mjs"
  description="ჩამოტვირთეთ runnable სკრიპტი, რომელიც მითითებულია ამ რეცეპტში."
/>

## წინაპირობები

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

დააყენეთ `CONNECT_ROLE`-ზე `app`, როდესაც გჭირდებათ აკრიფეთ აპლიკაციის მხარე
ხელის ჩამორთმევა. ნაგულისხმევი როლი არის `wallet`.

## მაგალითი სკრიპტი

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

## გაშვება და მონიტორინგი

- შეასრულეთ სკრიპტი `node --env-file=.env connect-preview.mjs`-ით (ან ექსპორტი
  ცვლადები ხელით). სკრიპტი აღრიცხავს გადახედვისას საფულის URI-ს, ღრმა ბმულს და
  რიგის სიღრმე WebSocket-ის გახსნამდე.
- მიაწოდეთ ტელემეტრიის საინფორმაციო დაფები რიგის მეტრიკის ამოკვეთით, რისთვისაც ბეჭდავს სკრიპტს
  გადინება/გადასვლის შემთხვევები (`connect.queue_depth`, `connect.queue_overflow_total`,
  `connect.queue_expired_total`). `ConnectQueueError` დამხმარეები ასხივებენ
  საგზაო რუქის ტაქსონომია (`queueOverflow`, `timeout`), რათა OTEL-ის ექსპორტიორებს შეეძლოთ დარჩენა
  შეესაბამება Android/Swift კლიენტებს.
- შეცვალეთ როლი `app`-ზე, რათა შეამოწმოთ ხელის ჩამორთმევის აპლიკაციის ფეხი. The
  აკრიფეთ ავტომატურად ირჩევს სწორ ჟეტონს (`token_app` vs. `token_wallet`)
  და განაახლებს `http→ws`/`https→wss`, ასე რომ ორივე როლი იზიარებს ერთნაირ სნიპეტს.

ეს რეცეპტი ხურავს დარჩენილ JS5 დოკუმენტაციის ხარვეზს Connect გადახედვისთვის
ამბავი გამოქვეყნებულია `roadmap.md`-ში: პორტალი ახლა აგზავნის ანაზრაურების ნიმუშს პლუს
რიგის ტელემეტრიის მითითება, რომელიც შეესაბამება საგზაო რუქის მოთხოვნას დოკუმენტურად
WebSocket გზამკვლევი Connect სესიის დამხმარეებთან ერთად.