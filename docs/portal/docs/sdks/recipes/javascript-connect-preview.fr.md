---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0df3d72cb822e0fef5201d5a5d25b8588378f51e3e3106c73def669d68b1c674
source_last_modified: "2025-11-14T11:46:45.985700+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Recette de prévisualisation Connect en JavaScript
description: Préparez des sessions de prévisualisation Connect, émettez la télémétrie de file et ouvrez le socket `/v2/connect/ws` avec `@iroha/iroha-js`.
slug: /sdks/recipes/javascript-connect-preview
---

import SampleDownload from '@site/src/components/SampleDownload';

Cette recette montre comment combiner `bootstrapConnectPreviewSession` avec le dialer WebSocket exposé par `ToriiClient.openConnectWebSocket()`. Le script reflète la section Connect de la roadmap du SDK JS : il génère des URI de prévisualisation déterministes, enregistre la télémétrie de profondeur de file et ouvre l’endpoint canonique `/v2/connect/ws` via le paquet `ws` pour que les apps Node.js suivent le même flux que les navigateurs.

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="Téléchargez le script exécutable référencé dans cette recette."
/>

## Prérequis

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

Définissez `CONNECT_ROLE` sur `app` lorsque vous devez ouvrir le volet applicatif du handshake. Le rôle par défaut est `wallet`.

## Script d’exemple

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

## Exécuter et surveiller

- Exécutez le script avec `node --env-file=.env connect-preview.mjs` (ou exportez les variables manuellement). Le script enregistre l’URI wallet de prévisualisation, le deeplink et la profondeur de file avant d’ouvrir le WebSocket.
- Alimentez les tableaux de bord de télémétrie en scrutant les métriques de file que le script imprime en cas d’overflow/expiry (`connect.queue_depth`, `connect.queue_overflow_total`, `connect.queue_expired_total`). Les helpers `ConnectQueueError` émettent la taxonomie de la roadmap (`queueOverflow`, `timeout`) pour rester cohérents avec les clients Android/Swift.
- Basculez le rôle sur `app` pour inspecter la branche applicative du handshake. Le dialer choisit automatiquement le bon token (`token_app` vs `token_wallet`) et met à niveau `http→ws`/`https→wss` pour que les deux rôles partagent le même snippet.

Cette recette comble le dernier manque de documentation JS5 pour la prévisualisation Connect mentionné dans `roadmap.md` : le portail fournit désormais un exemple clé en main et un guide de télémétrie de file, conformément à l’exigence de documenter le walkthrough WebSocket avec les helpers de session Connect.

