---
lang: he
direction: rtl
source: docs/portal/docs/sdks/recipes/javascript-connect-preview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-connect-preview
title: מתכון תצוגה מקדימה של Connect ב-JavaScript
description: הכינו סשנים לתצוגה מקדימה של Connect, הפיקו טלמטריית תור וחייגו את `/v2/connect/ws` עם `@iroha/iroha-js`.
---

import SampleDownload from '@site/src/components/SampleDownload';

המתכון הזה מראה כיצד לשלב `bootstrapConnectPreviewSession` עם ה-dialer של WebSocket שמסופק על ידי `ToriiClient.openConnectWebSocket()`. הסקריפט משקף את חלק ה-Connect ב-roadmap של SDK ה-JS: הוא מייצר URI תצוגה מקדימה דטרמיניסטיים, רושם טלמטריית עומק תור ופותח את נקודת הקצה הקנונית `/v2/connect/ws` באמצעות החבילה `ws` כדי שאפליקציות Node.js יפעילו את אותו הזרם כמו הדפדפנים.

<SampleDownload
  href="/sdk-recipes/javascript/connect-preview.mjs"
  filename="connect-preview.mjs"
  description="הורידו את הסקריפט שניתן להרצה שמוזכר במתכון הזה."
/>

## דרישות מקדימות

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

הגדירו `CONNECT_ROLE` ל-`app` כאשר צריך לחייג את צד האפליקציה של ה-handshake. תפקיד ברירת המחדל הוא `wallet`.

## סקריפט לדוגמה

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

## הרצה ומעקב

- הריצו את הסקריפט עם `node --env-file=.env connect-preview.mjs` (או ייצאו את המשתנים ידנית). הסקריפט רושם את preview wallet URI, ה-deeplink ועומק התור לפני פתיחת ה-WebSocket.
- הזינו לוחות טלמטריה על ידי גרידה של מדדי התור שהסקריפט מדפיס במקרי overflow/expiry (`connect.queue_depth`, `connect.queue_overflow_total`, `connect.queue_expired_total`). ה-helpers של `ConnectQueueError` מפיקים את הטקסונומיה של ה-roadmap (`queueOverflow`, `timeout`) כדי לשמור עקביות עם לקוחות Android/Swift.
- החליפו את התפקיד ל-`app` כדי לבדוק את ענף האפליקציה של ה-handshake. ה-dialer בוחר אוטומטית את הטוקן הנכון (`token_app` מול `token_wallet`) ומשדרג `http→ws`/`https→wss` כך ששני התפקידים חולקים אותו snippet.

המתכון הזה סוגר את הפער האחרון בתיעוד JS5 לסיפור תצוגת Connect שמוזכר ב-`roadmap.md`: הפורטל מספק כעת דוגמה מוכנה והנחיות לטלמטריית תור, בהתאם לדרישת ה-roadmap לתעד walkthrough של WebSocket לצד helpers של סשן Connect.

