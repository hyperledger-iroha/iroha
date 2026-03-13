---
slug: /sdks/recipes/javascript-connect-preview
lang: kk
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v2/connect/ws` socket with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload файлын '@site/src/components/SampleDownload' ішінен импорттау;

Бұл рецепт `bootstrapConnectPreviewSession` мен `bootstrapConnectPreviewSession` біріктіру жолын көрсетеді
WebSocket тергіші `ToriiClient.openConnectWebSocket()` арқылы ашылды. Сценарий
JS SDK жол картасының Connect бөлімін бейнелейді: ол детерминирленген
URI алдын ала қарау, кезек тереңдігі телеметриясын жазады және канондықты ашады
`/v2/connect/ws` соңғы нүктесі `ws` бумасын пайдаланып, Node.js қолданбалары
браузерлер сияқты бірдей ағын.

<Үлгі жүктеп алу
  href="/sdk-recipes/javascript/connect-preview.mjs"
  файл атауы = "connect-preview.mjs"
  description="Осы рецептте сілтеме жасалған орындалатын сценарийді жүктеп алыңыз."
/>

## Алғышарттар

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

Қолданба жағын теру қажет болғанда `CONNECT_ROLE` параметрін `app` мәніне орнатыңыз.
қол алысу. Әдепкі рөл - `wallet`.

## Сценарий үлгісі

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

## Іске қосу және бақылау

- `node --env-file=.env connect-preview.mjs` (немесе экспорттау) көмегімен сценарийді орындаңыз
  айнымалы мәндерді қолмен). Сценарий алдын ала қарау әмиянының URI мекенжайын, deeplink және
  WebSocket ашар алдында кезек тереңдігі.
- Сценарий басып шығаратын кезек көрсеткіштерін сызып тастау арқылы телеметрия бақылау тақталарын беріңіз
  толып кету/өткізу жағдайлары (`connect.queue_depth`, `connect.queue_overflow_total`,
  `connect.queue_expired_total`). `ConnectQueueError` көмекшілері мынаны шығарады
  жол картасы таксономиясы (`queueOverflow`, `timeout`), сондықтан OTEL экспорттаушылары қала алады
  Android/Swift клиенттеріне сәйкес келеді.
- Қол алысудың қолданбалы бөлігін тексеру үшін рөлді `app` деп ауыстырыңыз. The
  тергіш автоматты түрде дұрыс таңбалауышты таңдайды (`token_app` және `token_wallet`)
  және `http→ws`/`https→wss` жаңартады, осылайша екі рөл де бірдей үзіндіні бөліседі.

Бұл рецепт Connect алдын ала қарау үшін қалған JS5 құжаттамасының бос орнын жабады
`roadmap.md`-те айтылған оқиға: портал енді кілт тапсыру үлгісін жібереді.
құжаттау үшін жол картасы талабына сәйкес келетін кезек телеметриясының нұсқаулығы
Connect сеансының көмекшілерімен бірге WebSocket шолуы.