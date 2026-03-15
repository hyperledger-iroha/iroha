---
lang: am
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

ናሙና አውርድን ከ'@site/src/components/SampleDownload' አስመጣ;

ይህ የምግብ አሰራር I18NI0000002X ከ ጋር እንዴት እንደሚዋሃድ ያሳያል
የዌብሶኬት መደወያ በ`ToriiClient.openConnectWebSocket()` ተጋልጧል። ስክሪፕቱ
የJS ኤስዲኬ ፍኖተ ካርታ የግንኙነት ክፍልን ያንጸባርቃል፡ ቆራጥ ነው።
ዩአርአይዎችን አስቀድመው ይመልከቱ፣ የወረፋ ጥልቀት ቴሌሜትሪ ይመዘግባል፣ እና ቀኖናውን ይከፍታል።
`/v1/connect/ws` የ `ws` ጥቅል በመጠቀም የ Node.js አፕሊኬሽኖች ይህንን መጠቀም ይችላሉ።
እንደ አሳሾች ተመሳሳይ ፍሰት።

<ናሙና አውርድ
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="በዚህ የምግብ አሰራር ውስጥ የተጠቀሰውን ሊሄድ የሚችል ስክሪፕት አውርድ።"
/>

## ቅድመ ሁኔታዎች

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

የመተግበሪያውን ጎን መደወል ሲፈልጉ `CONNECT_ROLE` ወደ I18NI0000007X ያቀናብሩ
መጨባበጥ. ነባሪ ሚና `wallet` ነው።

## ምሳሌ ስክሪፕት።

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

## አሂድ እና ተቆጣጠር

- ስክሪፕቱን በI18NI0000009X (ወይም ወደ ውጭ መላክ) ያስፈጽሙ
  ተለዋዋጭዎቹ በእጅ)። ስክሪፕቱ የቅድመ እይታ የኪስ ቦርሳ ዩአርአይን፣ ጥልቅ ማገናኛን እና ይመዘግባል
  ዌብሶኬትን ከመክፈትዎ በፊት የወረፋ ጥልቀት።
- የስክሪፕት ህትመቶችን ወረፋ መለኪያዎችን በመቧጠጥ የቴሌሜትሪ ዳሽቦርዶችን ይመግቡ
  የትርፍ/የሚያልቅባቸው ጉዳዮች (`connect.queue_depth`፣ `connect.queue_overflow_total`፣
  `connect.queue_expired_total`). የ `ConnectQueueError` ረዳቶች ይለቃሉ
  የመንገድ ካርታ ታክሶኖሚ (`queueOverflow`፣ `timeout`) የኦቲኤል ላኪዎች መቆየት ይችላሉ።
  ከአንድሮይድ/ስዊፍት ደንበኞች ጋር የሚስማማ።
- የእጅ መጨባበጥ የትግበራ እግርን ለመመርመር ሚናውን ወደ `app` ይቀይሩት። የ
  መደወያ በራስ ሰር ትክክለኛውን ቶከን ይመርጣል (`token_app` vs. I18NI0000018X)
  እና ማሻሻያዎች `http→ws`/I18NI0000020X ስለዚህ ሁለቱም ሚናዎች አንድ አይነት ቅንጣቢ ይጋራሉ።

ይህ የምግብ አሰራር ለግንኙነት ቅድመ እይታ የቀረውን የJS5 ሰነድ ክፍተት ይዘጋል።
ታሪክ በ `roadmap.md` ውስጥ ተጠርቷል፡ ፖርታሉ አሁን የማዞሪያ ቁልፍ ናሙና ሲደመር ይልካል።
ወረፋ የቴሌሜትሪ መመሪያ፣ ከሮድ ካርታው መስፈርት ጋር በማዛመድ ሰነዱን
የዌብሶኬት መራመጃ ከConnect ክፍለ አጋሮች ጋር።