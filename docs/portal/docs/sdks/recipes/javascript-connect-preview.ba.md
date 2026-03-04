---
lang: ba
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

импорт SapleDownload '@site/src/компоненттар/SampleDownload';

Был рецепт күрһәтә, нисек берләштереү I18NI000000002X менән
WebSocket диалеры фашланған I18NI000000003X. Сценарий
JS SDK юл картаһының тоташтырыу бүлеген көҙгөләй: ул детерминистик мәтрүшкә
алдан ҡарау URI, яҙмалар сират тәрәнлеге телеметрия, һәм канон аса
I18NI000000004X ос нөктәһе I18NI000000005X пакеты ҡулланып, шулай Node.js ҡушымталар эшләй ала.
браузерҙар менән бер үк ағым.

<СэмплДау-лог
  href="/sdk-рецепттары/жаваскрипт/бәйләнеш-превизор.мжс".
  файл исеме="тоташҡа-превизор.мжс".
  тасуирлама="Был рецептта һылтанма яһалған йүгерә торған сценарийҙы скачать."
/>

## Алдан шарттар

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

I18NI0000000006X комплекты I18NI000000007X тиклем, ҡасан һеҙгә кәрәк, тип йыйырға ҡушымта яғы .
ҡул ҡыҫыу. Дефолт роле — `wallet`.

## Миҫал сценарийы

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

## Йүгерергә & монитор

- I18NI000000009X менән сценарийҙы башҡарыу (йәки экспорт
  үҙгәртеүселәр ҡул менән). Сценарий логин алдан ҡарау янсыҡ URI, dowerlink, һәм
  сират тәрәнлеге WebSocket асыу алдынан.
- Телеметрия приборҙар таҡталары ашау скрипт баҫмаларын сират метрикаларын ҡырҡып, 2012 йыл.
  өҙөү/ваҡыт осраҡтар (`connect.queue_depth`, `connect.queue_overflow_total`,
  `connect.queue_expired_total`). `ConnectQueueError` ярҙамсылары сығарыла.
  юл картаһы таксономияһы (`queueOverflow`, I18NI000000015X) шулай OTEL экспортерҙары ҡала ала
  тура килә Android/Swift клиенттар.
- Ҡул ҡыҫышыуының ҡулланыу аяҡын тикшерергә `app` тиклем ролде алмаштырығыҙ. 1990 й.
  диалер автоматик рәүештә дөрөҫ жетон һайлай (`token_app` vs. `token_wallet`)
  һәм яңыртыу `http→ws`/I18NI000000020X шулай ике ролде лә бер үк өҙөк бүлешә.

Был рецепт ҡалған JS5 документация айырмаһы өсөн ябыла Connect алдан ҡарау .
18NI000000021X-та саҡырылған хикәйә: портал хәҙер ҡабаҡтың төп өлгөһөн плюс ташый
сират телеметрия етәкселеге, тап килтереп юл картаһы талабы документлаштырыу өсөн
WebSocket проходка менән бергә Коннект сессияһы ярҙамсылары.