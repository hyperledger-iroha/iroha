---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-connect-preview
title: Receita de prévia do Connect em JavaScript
description: Prepare sessões de prévia do Connect, emita telemetria de fila e abra o socket `/v2/connect/ws` com `@iroha/iroha-js`.
---

import SampleDownload from '@site/src/components/SampleDownload';

Esta receita mostra como combinar `bootstrapConnectPreviewSession` com o discador WebSocket exposto por `ToriiClient.openConnectWebSocket()`. O script espelha a seção Connect do roadmap do SDK JS: ele gera URIs de prévia determinísticos, registra telemetria de profundidade de fila e abre o endpoint canônico `/v2/connect/ws` usando o pacote `ws` para que apps Node.js executem o mesmo fluxo que os navegadores.

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="Baixe o script executável referenciado nesta receita."
/>

## Pré-requisitos

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

Defina `CONNECT_ROLE` como `app` quando precisar abrir o lado da aplicação do handshake. O papel padrão é `wallet`.

## Script de exemplo

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

## Executar e monitorar

- Execute o script com `node --env-file=.env connect-preview.mjs` (ou exporte as variáveis manualmente). O script registra a URI da wallet de prévia, o deeplink e a profundidade de fila antes de abrir o WebSocket.
- Alimente os dashboards de telemetria raspando as métricas de fila que o script imprime em casos de overflow/expiry (`connect.queue_depth`, `connect.queue_overflow_total`, `connect.queue_expired_total`). Os helpers `ConnectQueueError` emitem a taxonomia da roadmap (`queueOverflow`, `timeout`) para manter consistência com clientes Android/Swift.
- Mude o papel para `app` para inspecionar a perna de aplicação do handshake. O discador escolhe automaticamente o token correto (`token_app` vs. `token_wallet`) e atualiza `http→ws`/`https→wss` para que ambos os papéis compartilhem o mesmo trecho.

Esta receita fecha a lacuna restante de documentação JS5 para a prévia do Connect mencionada em `roadmap.md`: o portal agora entrega um exemplo pronto e orientação de telemetria de filas, atendendo ao requisito do roadmap de documentar o walkthrough WebSocket junto aos helpers de sessão Connect.

