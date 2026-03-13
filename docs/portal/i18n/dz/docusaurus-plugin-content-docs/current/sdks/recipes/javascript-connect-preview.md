---
slug: /sdks/recipes/javascript-connect-preview
lang: dz
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v2/connect/ws` socket with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

filight Sample load འདི་ '@site/src/ཆ་ཤས་/ཆ་ཤས་/དཔེ་ཚད་ཕབ་ལེན་';

བཀོད་སྒྲིག་འདི་གིས་ `bootstrapConnectPreviewSession` དང་གཅིག་ཁར་ མཉམ་སྡེབ་འབད་ཐངས་སྟོནམ་ཨིན།
`ToriiClient.openConnectWebSocket()` གིས་ ཝེབ་སོ་ཀེཊ་ཌའི་ལར་ བཀྲམ་སྟོན་འབད་ཡོདཔ། ༡ ཡིག་གཟུགས་འདི།
ཇེ་ཨེསི་ཨེསི་ཌི་ཀེ་ ལམ་གྱི་ས་ཁྲ་གི་མཐུད་འབྲེལ་གྱི་དབྱེ་ཚན་འདི་ མེ་ལོང་: དེ་གིས་ མིན་ཊི་ཚུ་ གཏན་འབེབས་བཟོ་ནི།
སྔོན་ལྟའི་ཡུ་ཨར་ཨའི་ཚུ་དང་ གྱལ་རིམ་གཏིང་ཚད་ བརྒྱུད་འཕྲིན་ དེ་ལས་ ཀེ་ནོ་ནིག་ཁ་ཕྱེཝ་ཨིན།
`/v2/connect/ws` མཐའ་མཚམས་ `ws` ཐུམ་སྒྲིལ་ལག་ལེན་འཐབ་སྟེ་ Node.js གློག་རིམ་ཚུ་གིས་ ལག་ལེན་འཐབ་བཏུབ།
བརའུ་ཟར་དང་འདྲ།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལན་/ཇ་བ་སི་ཀིརིཀ་ཊི་/མཐུད་ལམ་-སྔོན་ལྟ་.mjs"
  filen name="--preview.mjs""
  secont="ཐབས་ཤེས་འདི་ནང་གཞི་བསྟུན་འབད་ཡོད་པའི་ གཡོག་བཀོལ་བཏུབ་པའི་ཡིག་ཚུགས་ཕབ་ལེན་འབད།"
།/>།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

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

ཁྱོད་ཀྱིས་ གློག་རིམ་གྱི་ཕྱོགས་འདི་ ཌའི་ཨེལ་འདི་ ཌའི་ཨེལ་འདི་ ཌའི་ཨེལ་འདི་ ཌིལ་འབད་དགོ་པའི་སྐབས་ I18NI0000000006X ལུ་ གཞི་སྒྲིག་འབད།
ལག་བཟོ། སྔོན་སྒྲིག་འགན་ཁུར་འདི་ `wallet` ཨིན།

## དཔེར་ཡིག་།

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

## གཡོག་བཀོལ།

- ཡིག་ཚུགས་འདི་ `node --env-file=.env connect-preview.mjs` དང་གཅིག་ཁར་ལག་ལེན་འཐབ།(ཡང་ན་ཕྱིར་འདྲེན་འབད།
  འགྱུར་ཅན་ཚུ་ ལག་ཐོག་ལས་)། ཡིག་ཆ་འདི་གིས་ སྔོན་ལྟའི་དངུལ་ཁུག་འདི་ URI དང་ གཏིང་ཟབ་ལིངཀ་ དེ་ལས་ དང་ དེ་ལས་ དང་ དེ་ལས་ ནང་བསྐྱོད་འབདཝ་ཨིན།
  ཝེབ་སོ་ཀེཊི་ཁ་མ་ཕྱེ་བའི་ཧེ་མ་ གྱལ་གཏིང་ཚད་།
- གྱལ་རིམ་ཚུ་ བཤུད་སྒྲིལ་འབད་ཐོག་ལས་ ཕིཌི་ཊེ་ལི་མི་ཊི་ ཌེཤ་བོརཌི་ཚུ་ ཡིག་ཚུགས་ཀྱི་དཔར་བསྐྲུན་ཚུ་ གི་དོན་ལུ་ བརྡ་སྟོནམ་ཨིན།
  འཕྱུར་/འདོན་གྱི་གནད་དོན་ (`connect.queue_depth`, I18NI000000011X,
  `connect.queue_expired_total`). I18NI000000013X གྲོགས་རམ་འབད་མི་ཚུ་གིས་ བཏོནམ་ཨིན།
  ལམ་གྱི་ས་ཁྲ་ (`queueOverflow`, `timeout`) དེ་འབདཝ་ལས་ OTEL ཕྱིར་ཚོང་འཐབ་མི་ཚུ་ སྡོད་ཚུགས།
  Android/Swift མཁོ་མངགས་འབད་མི་ཚུ་དང་གཅིག་ཁར་ མཐུན་སྒྲིག་ཡོདཔ།
- ལག་བཟོའི་ ཞུ་ཡིག་རྐངམ་བརྟག་དཔྱད་འབད་ནིའི་དོན་ལུ་ I18NI0000016X ལུ་ འགན་ཁུར་བརྗེ་སོར་འབད། ཚིག༌ཕྲད
  ཌའི་ལར་གྱིས་ རང་བཞིན་གྱིས་ ཊོ་ཀེན་ཚུ་ ངེས་བདེན་ (`token_app` དང་ I18NI0000018X) གདམ་ཁ་རྐྱབ་ཨིན།
  དང་ ཡར་འཕར་ `http→ws`/`https→wss` དེ་གཉིས་ཆ་ར་གིས་ འགན་ཁུར་གཅིག་མཚུངས་སྦེ་ བརྗེ་སོར་འབདཝ་ཨིན།

ཐབས་གཞི་འདི་གིས་ མཐུད་ལམ་སྔོན་ལྟ་གི་དོན་ལུ་ ལྷག་ལུས་ཇེ་ཨེསི་༥ ཡིག་ཆ་གི་བར་སྟོང་འདི་ཁ་བསྡམས།
ད་ལྟོ་ཡོད་པའི་ལོ་རྒྱུས་འདི་ `roadmap.md` ནང་ལུ་བཀོད་དེ་ཡོདཔ་ཨིན།
གྱལ་རིམ་གྱི་བརྡ་འཕྲིན་ལམ་སྟོན་དང་ ལམ་སྟོན་གྱི་དགོས་མཁོ་དང་མཐུན་སྒྲིག་འབད་ནི།
WebSocket speed speed speed sostwork གི་མཉམ་དུ་ མཐུད་བྱེད་ཀྱི་ལས་རོགས་པ་ཚུ།