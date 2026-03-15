---
slug: /sdks/recipes/javascript-connect-preview
lang: my
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript Connect preview recipe
description: Stage Connect preview sessions, emit queue telemetry, and dial the `/v2/connect/ws` socket with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

'@site/src/components/SampleDownload' မှ SampleDownload ကို တင်သွင်းပါ။

ဤစာရွက်တွင် `bootstrapConnectPreviewSession` ကို မည်သို့ပေါင်းစပ်ရမည်ကို ပြသထားသည်။
`ToriiClient.openConnectWebSocket()` ဖြင့် ဖော်ထုတ်ထားသော WebSocket dialer ဇာတ်ညွှန်း
JS SDK လမ်းပြမြေပုံ၏ ချိတ်ဆက်မှုအပိုင်းကို ထင်ဟပ်စေသည်- ၎င်းသည် အဆုံးအဖြတ်ပိုင်းကို ဦးစားပေးသည်။
URI များကို အစမ်းကြည့်ရှုခြင်း၊ တန်းစီနေသော အတိမ်အနက် တယ်လီမီတာကို မှတ်တမ်းတင်ပြီး canonical ကိုဖွင့်ပါ။
`/v2/connect/ws` အဆုံးမှတ်သည် `ws` ပက်ကေ့ဂျ်ကို အသုံးပြု၍ Node.js အက်ပ်များကို လေ့ကျင့်ခန်းလုပ်နိုင်သည်။
ဘရောက်ဆာများနှင့် တူညီသော စီးဆင်းမှု။

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/javascript/connect-preview.mjs"
  ဖိုင်အမည် = "connect-preview.mjs"
  description="ဤစာရွက်တွင် ကိုးကားထားသော runnable script ကို ဒေါင်းလုဒ်လုပ်ပါ။"
/>

## လိုအပ်ချက်များ

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

သင်သည် အပလီကေးရှင်းဘက်မှ ခေါ်ဆိုရန် လိုအပ်သောအခါ `CONNECT_ROLE` ကို `app` သို့ သတ်မှတ်ပါ။
လက်ဆွဲနှုတ်ဆက်သည်။ မူရင်းအခန်းကဏ္ဍမှာ `wallet` ဖြစ်သည်။

## ဥပမာ ဇာတ်ညွှန်း

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

## ပြေးပြီး စောင့်ကြည့်ပါ။

- ဇာတ်ညွှန်းကို `node --env-file=.env connect-preview.mjs` ဖြင့် လုပ်ဆောင်ပါ (သို့မဟုတ် ထုတ်ယူပါ။
  variable များကို manually)။ ဇာတ်ညွှန်းသည် အကြိုကြည့်ရှုသည့် ပိုက်ဆံအိတ် URI၊ deeplink နှင့် မှတ်တမ်းများကို မှတ်သားထားသည်။
  WebSocket ကိုမဖွင့်မီ တန်းစီခြင်းအတိမ်အနက်။
- script prints များအတွက် တန်းစီမက်ထရစ်များကို ခြစ်ထုတ်ခြင်းဖြင့် telemetry dashboards များကို feed လုပ်ပါ။
  ပြည့်လျှံ/သက်တမ်းလွန်ကိစ္စများ (`connect.queue_depth`၊ `connect.queue_overflow_total`၊
  `connect.queue_expired_total`)။ `ConnectQueueError` အကူအညီပေးသူများသည် ၎င်းကို ထုတ်လွှတ်သည်။
  လမ်းပြမြေပုံအစီအစဥ် (`queueOverflow`၊ `timeout`) ထို့ကြောင့် OTEL တင်ပို့သူများသည် ဆက်လက်နေထိုင်နိုင်သည်
  Android/Swift သုံးစွဲသူများနှင့် ကိုက်ညီသည်။
- လက်ဆွဲခြင်း၏ အပလီကေးရှင်းခြေထောက်ကို စစ်ဆေးရန် အခန်းကဏ္ဍကို `app` သို့ ပြောင်းပါ။ ဟိ
  dialer သည် မှန်ကန်သော တိုကင်ကို အလိုအလျောက် ရွေးချယ်သည် (`token_app` နှင့် `token_wallet`)
  နှင့် `http→ws`/`https→wss` ကို အဆင့်မြှင့်ခြင်းဖြင့် ကဏ္ဍနှစ်ခုလုံးသည် တူညီသော အတိုအထွာကို မျှဝေပါသည်။

ဤစာရွက်သည် Connect အစမ်းကြည့်ရှုမှုအတွက် ကျန်ရှိသော JS5 စာရွက်စာတမ်းကွာဟချက်ကို ပိတ်သည်။
`roadmap.md` တွင် ခေါ်ဝေါ်သော ဇာတ်လမ်း- ပေါ်တယ်သည် ယခု turnkey နမူနာ အပေါင်းကို ပို့ပေးသည်
စီတန်းတယ်လီမီတာလမ်းညွှန်ချက်၊ မှတ်တမ်းပြုစုရန် လမ်းပြမြေပုံလိုအပ်ချက်နှင့် ကိုက်ညီသည်။
Connect session helpers နှင့်အတူ WebSocket လမ်းညွှန်ချက်။