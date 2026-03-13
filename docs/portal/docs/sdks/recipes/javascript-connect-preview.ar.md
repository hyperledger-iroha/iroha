---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0df3d72cb822e0fef5201d5a5d25b8588378f51e3e3106c73def669d68b1c674
source_last_modified: "2025-11-14T11:46:45.985700+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: وصفة معاينة Connect في JavaScript
description: حضّر جلسات معاينة Connect، وأصدر قياسًا عن بعد للطابور، واتصل بمقبس `/v2/connect/ws` باستخدام `@iroha/iroha-js`.
slug: /sdks/recipes/javascript-connect-preview
---

import SampleDownload from '@site/src/components/SampleDownload';

توضح هذه الوصفة كيفية الجمع بين `bootstrapConnectPreviewSession` ومُتصل WebSocket الذي يوفّره `ToriiClient.openConnectWebSocket()`. يعكس السكربت قسم Connect في خارطة طريق SDK الخاصة بـ JS: فهو يولّد روابط معاينة حتمية، ويسجّل قياس عمق الطابور، ويفتح نقطة النهاية القياسية `/v2/connect/ws` باستخدام حزمة `ws` حتى تتمكن تطبيقات Node.js من تنفيذ التدفق نفسه الذي تنفذه المتصفحات.

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="نزّل السكربت القابل للتشغيل المشار إليه في هذه الوصفة."
/>

## المتطلبات المسبقة

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

اضبط `CONNECT_ROLE` على `app` عندما تحتاج لفتح جانب التطبيق من المصافحة. الدور الافتراضي هو `wallet`.

## مثال على السكربت

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

## التشغيل والمراقبة

- شغّل السكربت باستخدام `node --env-file=.env connect-preview.mjs` (أو صدّر المتغيرات يدويًا). يسجّل السكربت رابط wallet للمعاينة وdeeplink وعمق الطابور قبل فتح WebSocket.
- غذّ لوحات القياس عن بعد عبر جمع مقاييس الطابور التي يطبعها السكربت في حالات overflow/expiry (`connect.queue_depth`, `connect.queue_overflow_total`, `connect.queue_expired_total`). تساعد `ConnectQueueError` في إصدار تصنيف خارطة الطريق (`queueOverflow`, `timeout`) للحفاظ على الاتساق مع عملاء Android/Swift.
- بدّل الدور إلى `app` لفحص ساق التطبيق في المصافحة. يختار المتصل تلقائيًا الرمز الصحيح (`token_app` مقابل `token_wallet`) ويحدّث `http→ws`/`https→wss` بحيث يشارك الدوران المقتطف نفسه.

تُغلق هذه الوصفة آخر فجوة توثيقية في JS5 لقصة معاينة Connect المذكورة في `roadmap.md`: يوفّر البوابة الآن مثالًا جاهزًا وإرشادات لقياس الطابور، بما يتماشى مع متطلب خارطة الطريق لتوثيق شرح WebSocket بجانب أدوات جلسة Connect.

