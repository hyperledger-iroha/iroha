---
lang: hy
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

ներմուծել SampleDownload-ից '@site/src/components/SampleDownload';

Այս բաղադրատոմսը ցույց է տալիս, թե ինչպես համատեղել `bootstrapConnectPreviewSession`-ը
WebSocket հավաքիչը ցուցադրվում է `ToriiClient.openConnectWebSocket()`-ի կողմից: Սցենարը
արտացոլում է JS SDK ճանապարհային քարտեզի Connect բաժինը. այն դետերմինիստական է
նախադիտեք URI-ները, գրանցում է հերթի խորության հեռաչափությունը և բացում կանոնականը
`/v1/connect/ws` վերջնակետը՝ օգտագործելով `ws` փաթեթը, որպեսզի Node.js հավելվածները կարողանան իրականացնել
նույն հոսքը, ինչ բրաուզերները:

<Նմուշի ներբեռնում
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename = "connect-preview.mjs"
  description="Ներբեռնեք այս բաղադրատոմսում նշված runnable script-ը։"
/>

## Նախադրյալներ

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

Սահմանեք `CONNECT_ROLE`-ը `app`-ի, երբ անհրաժեշտ է հավաքել հավելվածի կողքին
ձեռքսեղմում. Լռելյայն դերը `wallet` է:

## Օրինակ սցենար

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

## Գործարկել և վերահսկել

- Կատարեք սցենարը `node --env-file=.env connect-preview.mjs`-ով (կամ արտահանում
  փոփոխականները ձեռքով): Սցենարը գրանցում է նախադիտման դրամապանակի URI, deeplink և
  Հերթի խորությունը WebSocket-ը բացելուց առաջ:
- Սնուցեք հեռաչափության վահանակները՝ քերելով հերթի չափումները, որոնց համար տպագրվում է սցենարը
  հորդառատ/ժամկետանց դեպքեր (`connect.queue_depth`, `connect.queue_overflow_total`,
  `connect.queue_expired_total`): `ConnectQueueError` օգնականներն արտանետում են
  ճանապարհային քարտեզի տաքսոնոմիա (`queueOverflow`, `timeout`), որպեսզի OTEL արտահանողները կարողանան մնալ
  համահունչ Android/Swift հաճախորդների հետ:
- Փոխեք դերը `app`-ի հետ՝ ստուգելու ձեռքսեղմման կիրառական ոտքը: Այն
  Dialer ավտոմատ կերպով ընտրում է ճիշտ նշանը (`token_app` ընդդեմ `token_wallet`)
  և թարմացնում է `http→ws`/`https→wss`, այնպես որ երկու դերերն էլ կիսում են նույն հատվածը:

Այս բաղադրատոմսը փակում է JS5 փաստաթղթերի մնացած բացը Connect նախադիտման համար
պատմությունը նկարագրված է `roadmap.md`-ում. պորտալն այժմ ուղարկում է բանտապահ նմուշ գումարած
Հերթի հեռաչափության ուղեցույց, որը համապատասխանում է ճանապարհային քարտեզի պահանջին փաստաթղթավորման համար
WebSocket ուղեցույցը Connect նիստի օգնականների կողքին: