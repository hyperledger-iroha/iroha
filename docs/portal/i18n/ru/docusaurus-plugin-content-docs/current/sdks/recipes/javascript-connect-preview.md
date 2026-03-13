---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-connect-preview
title: Рецепт превью Connect на JavaScript
description: Подготовьте сессии превью Connect, выдавайте телеметрию очереди и открывайте сокет `/v2/connect/ws` с `@iroha/iroha-js`.
---

import SampleDownload from '@site/src/components/SampleDownload';

Этот рецепт показывает, как объединить `bootstrapConnectPreviewSession` с WebSocket-дайалером, доступным через `ToriiClient.openConnectWebSocket()`. Скрипт повторяет раздел Connect из roadmap JS SDK: он генерирует детерминированные URI превью, записывает телеметрию глубины очереди и открывает канонический endpoint `/v2/connect/ws` с пакетом `ws`, чтобы Node.js приложения выполняли тот же поток, что и браузеры.

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="Скачайте исполняемый скрипт, упомянутый в этой рецепте."
/>

## Предварительные требования

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

Установите `CONNECT_ROLE` в `app`, когда нужно открыть прикладную сторону рукопожатия. Роль по умолчанию — `wallet`.

## Пример скрипта

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

## Запуск и мониторинг

- Запустите скрипт с `node --env-file=.env connect-preview.mjs` (или экспортируйте переменные вручную). Скрипт логирует preview wallet URI, deeplink и глубину очереди перед открытием WebSocket.
- Питайте дашборды телеметрии, собирая метрики очереди, которые скрипт печатает при overflow/expiry (`connect.queue_depth`, `connect.queue_overflow_total`, `connect.queue_expired_total`). Хелперы `ConnectQueueError` отдают таксономию roadmap (`queueOverflow`, `timeout`), чтобы OTEL-экспортеры оставались согласованными с клиентами Android/Swift.
- Переключите роль на `app`, чтобы проверить прикладную часть рукопожатия. Дайалер автоматически выбирает правильный токен (`token_app` vs. `token_wallet`) и обновляет `http→ws`/`https→wss`, чтобы обе роли использовали один и тот же сниппет.

Этот рецепт закрывает последний пробел в документации JS5 по превью Connect, упомянутый в `roadmap.md`: портал теперь содержит готовый пример и руководство по телеметрии очереди, соответствуя требованию roadmap документировать WebSocket walkthrough рядом с helper-ами сессий Connect.

