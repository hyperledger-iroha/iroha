---
lang: he
direction: rtl
source: docs/portal/docs/sdks/javascript.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: התחלה מהירה של JavaScript SDK
תיאור: בנה עסקאות, הזרם אירועים והגבר תצוגות מקדימות של Connect עם `@iroha/iroha-js`.
slug: /sdks/javascript
---

`@iroha/iroha-js` היא חבילת Node.js הקנונית לאינטראקציה עם Torii. זה
חבילות בוני Norito, עוזרי Ed25519, כלי עזר לעימוד וחומר עמיד
לקוח HTTP/WebSocket כך שתוכל לשקף את זרימות ה-CLI מ-TypeScript.

## התקנה

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

שלב הבנייה עוטף את `cargo build -p iroha_js_host`. ודא שרשרת הכלים מ
`rust-toolchain.toml` זמין באופן מקומי לפני הפעלת `npm run build:native`.

## ניהול מפתחות

```ts
import {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
} from "@iroha/iroha-js";

const { publicKey, privateKey } = generateKeyPair();

const message = Buffer.from("hello iroha");
const signature = signEd25519(message, privateKey);

console.assert(verifyEd25519(message, signature, publicKey));

const derived = publicKeyFromPrivate(privateKey);
console.assert(Buffer.compare(derived, publicKey) === 0);
```

## בניית עסקאות

בוני הוראות Norito מנרמל מזהים, מטא נתונים וכמויות כך
עסקאות מקודדות תואמות למטעני Rust/CLI.

```ts
import {
  buildMintAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
} from "@iroha/iroha-js";

const mint = buildMintAssetInstruction({
  assetId: "rose#wonderland#alice",
  quantity: "10",
});

const transfer = buildTransferAssetInstruction({
  sourceAssetId: "rose#wonderland#alice",
  destinationAccountId: "ih58...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "ih58...",
  mint: { assetId: "rose#wonderland#alice", quantity: "10" },
  transfers: [{ destinationAccountId: "ih58...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## תצורת לקוח Torii

`ToriiClient` מקבל כפתורי ניסיון חוזר/פסק זמן המשקפים את `iroha_config`. השתמש
`resolveToriiClientConfig` למיזוג אובייקט תצורה של camelCase (לנרמל
`iroha_config` תחילה), עקיפות סביבה ואפשרויות מוטבעות.

```ts
import { ToriiClient, resolveToriiClientConfig } from "@iroha/iroha-js";
import fs from "node:fs";

const rawConfig = JSON.parse(fs.readFileSync("./iroha_config.json", "utf8"));
const config = rawConfig?.torii
  ? {
      ...rawConfig,
      torii: {
        ...rawConfig.torii,
        apiTokens: rawConfig.torii.api_tokens ?? rawConfig.torii.apiTokens,
      },
    }
  : rawConfig;
const clientConfig = resolveToriiClientConfig({
  config,
  overrides: { timeoutMs: 2_000, maxRetries: 5 },
});

const torii = new ToriiClient(
  config?.torii?.address ?? "http://localhost:8080",
  {
    config,
    timeoutMs: clientConfig.timeoutMs,
    maxRetries: clientConfig.maxRetries,
  },
);
```

משתני סביבה עבור פיתוח מקומי:

| משתנה | מטרה |
|--------|--------|
| `IROHA_TORII_TIMEOUT_MS` | בקש פסק זמן (מילישניות). |
| `IROHA_TORII_MAX_RETRIES` | מקסימום ניסיונות ניסיון חוזר. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | נסיגה ראשונית חוזרת. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | מכפיל גיבוי אקספוננציאלי. |
| `IROHA_TORII_MAX_BACKOFF_MS` | עיכוב מקסימלי של ניסיון חוזר. |
| `IROHA_TORII_RETRY_STATUSES` | קודי מצב HTTP מופרדים בפסיקים כדי לנסות שוב. |
| `IROHA_TORII_RETRY_METHODS` | שיטות HTTP מופרדות בפסיק כדי לנסות שוב. |
| `IROHA_TORII_API_TOKEN` | מוסיף `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | מוסיף כותרת `Authorization: Bearer …`. |

פרופילי ניסיון חוזר משקפים את ברירת המחדל של Android ומיוצאים לבדיקות זוגיות:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. ראה `docs/source/sdk/js/torii_retry_policy.md`
עבור מיפוי נקודת קצה לפרופיל ופרמטרים ביקורת ממשל במהלך
JS4/JS7.

## רשימות ניתנות לחזרה ועימוד

עוזרי עימוד משקפים את ארגונומיה של Python SDK עבור `/v1/accounts`,
`/v1/domains`, `/v1/assets/definitions`, NFTs, יתרות, מחזיקי נכסים, וה
היסטוריית עסקאות בחשבון.

```ts
const { items, total } = await torii.listDomains({
  limit: 25,
  sort: [{ key: "id", order: "asc" }],
});
console.log(`first page out of ${total}`, items);

for await (const account of torii.iterateAccounts({
  pageSize: 50,
  maxItems: 200,
})) {
  console.log(account.id);
}

const defs = await torii.queryAssetDefinitions({
  filter: { Eq: ["metadata.display_name", "Ticket"] },
  sort: [{ key: "metadata.display_name", order: "desc" }],
  fetchSize: 64,
});
console.log("filtered definitions", defs.items);

const assetId = "rose#wonderland#alice@test";
const balances = await torii.listAccountAssets("alice@test", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("alice@test", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("rose#wonderland", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## קצבאות לא מקוונות ומטא נתונים של פסק דין

תגובות קצבאות לא מקוונות חושפות את המטא-נתונים של ספר החשבונות המועשר מראש -
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` ו-`remaining_amount` מוחזרים לצד הגולמי
להקליט כך שמרכזי המחוונים לא יצטרכו לפענח את מטעני Norito המוטבעים. החדש
עוזרי ספירה לאחור (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) מדגישים את המועד האחרון שפג תוקף (רענן → מדיניות
→ אישור) כך שתגי ממשק משתמש יכולים להזהיר מפעילים בכל פעם שיש קצבה
נותרו פחות מ-24 שעות. ה-SDK
משקף את מסנני REST שנחשפו על ידי `/v1/offline/allowances`:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs`, וה-
`requireVerdict` / `onlyMissingVerdict` בוליאני. שילובים לא חוקיים (עבור
דוגמה `onlyMissingVerdict` + `verdictIdHex`) נדחו באופן מקומי לפני Torii
נקרא.

```ts
const { items: allowances } = await torii.listOfflineAllowances({
  limit: 25,
  policyExpiresBeforeMs: Date.now() + 86_400_000,
  requireVerdict: true,
});

for (const entry of allowances) {
  console.log(
    entry.controller_display,
    entry.remaining_amount,
    entry.verdict_id_hex,
    entry.refresh_at_ms,
  );
}
```

## העלאות לא מקוונות (בעיה + הרשמה)השתמש בעוזרי הטעינה כאשר אתה רוצה להנפיק תעודה ומיד
לרשום אותו בפנקס החשבונות. ה-SDK מאמת את התעודה שהונפק והרשומה
המזהים תואמים לפני החזרה, והתגובה כוללת את שני המטענים. יש
אין נקודת קצה ייעודית להעלאה; העוזר משרשר את הנושא + רישום שיחות. אם
כבר יש לך תעודה חתומה, התקשר ל-`registerOfflineAllowance` (או
`renewOfflineAllowance`) ישירות.

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "alice@wonderland",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "alice@wonderland",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii שאילתות וסטרימינג (WebSockets)

עוזרי שאילתות חושפים סטטוס, מדדי Prometheus, צילומי מצב של טלמטריה ואירוע
זרמים באמצעות דקדוק המסנן Norito. סטרימינג משתדרג אוטומטית ל
WebSockets וקורות חיים כאשר תקציב הניסיון מחדש מאפשר.

```ts
const status = await torii.getSumeragiStatus();
console.log(status?.leader_index);

const metrics = await torii.getMetrics({ asText: true });
console.log(metrics.split("\n").slice(0, 5));

const abort = new AbortController();
for await (const event of torii.streamEvents({
  filter: { Pipeline: { Block: {} } },
  signal: abort.signal,
})) {
  console.log(event.id, event.data);
  break;
}
abort.abort(); // closes the underlying WebSocket cleanly
```

השתמש ב-`streamBlocks`, `streamTransactions`, או `streamTelemetry` עבור השני
נקודות קצה של WebSocket. כל עוזרי הסטרימינג מעלים ניסיונות ניסיון חוזר, אז חבר את
`onReconnect` התקשרות חוזרת ללוחות מחוונים של הזנה והתראה.

## צילומי מצב של Explorer ומטעני QR

טלמטריית Explorer מספקת עוזרים מוקלדים עבור `/v1/explorer/metrics` ו
`/v1/explorer/accounts/{account_id}/qr` נקודות קצה כך שמרכזי המחוונים יכולים להפעיל מחדש את
אותן צילומי מצב שמניעים את הפורטל. `getExplorerMetrics()` מנרמל את
עומס ומחזיר `null` כאשר המסלול מושבת. חבר אותו עם
`getExplorerAccountQr()` בכל פעם שתזדקק ל-IH58 (מועדף)/סורה (השני בטובו) ליטרלים בתוספת מובנה
SVG עבור כפתורי שיתוף.

```ts
import { promises as fs } from "node:fs";

const snapshot = await torii.getExplorerMetrics();
if (!snapshot) {
  console.warn("explorer metrics unavailable");
} else {
  console.log("peers:", snapshot.peers);
  console.log("last block:", snapshot.blockHeight, snapshot.blockCreatedAt);
  console.log("avg commit ms:", snapshot.averageCommitTimeMs ?? "n/a");
}

const qr = await torii.getExplorerAccountQr("ih58...", {
  addressFormat: "compressed",
});
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

מעבר `addressFormat: "compressed"` משקף את ברירת המחדל של Explorer דחוסה
בוררים; השמט את העקיפה עבור פלט IH58 המועדף או בקש `ih58_qr`
כאשר אתה צריך את הגרסה בטוחה ל-QR. הליטרלי הדחוס הוא השני הטוב ביותר
אפשרות לסורה בלבד עבור UX. המסייע תמיד מחזיר את המזהה הקנוני,
המילולי שנבחר, והמטא נתונים (קידומת רשת, גרסת QR/מודולים, שגיאה
שכבת תיקון, ו-SVG מוטבע), כך ש-CI/CD יכולים לפרסם את אותם מטענים
האקספלורר צץ מבלי לקרוא לממירים מותאמים אישית.

## חיבור הפעלות ותורים

המראה של Connect helpers `docs/source/connect_architecture_strawman.md`. ה
הנתיב המהיר ביותר להפעלה מוכנה לתצוגה מקדימה הוא `bootstrapConnectPreviewSession`,
שתופר יחד את יצירת SID/URI דטרמיניסטית ואת Torii
שיחת הרשמה.

```ts
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(
  torii,
  {
    chainId: "sora-mainnet",
    node: "https://torii.nexus.example",
    sessionOptions: { node: "https://torii.backup.example" },
  },
);

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- עברו את `register: false` כאשר אתם צריכים רק URI דטרמיניסטיים עבור QR/קישור עמוק
  תצוגות מקדימות.
- `generateConnectSid` נשאר זמין כאשר אתה צריך לגזור מזהי הפעלה
  ללא הטבעת URIs.
- מפתחות הכוונה ומעטפות טקסט צופן מגיעים מהגשר המקומי; מתי
  לא זמין ה-SDK נופל בחזרה ל-codec JSON וזורק
  `ConnectQueueError.bridgeUnavailable`.
- מאגרים לא מקוונים מאוחסנים כ-Norito `.to` בלובים ב-IndexedDB. תור לפקח
  מצב דרך `ConnectQueueError.overflow(limit)` הנפלט /
  שגיאות `.expired(ttlMs)` וטלמטריית הזנה `connect.queue_depth` כמתואר
  במפת הדרכים.

### חבר תמונות מצב של רישום ומדיניותמפעילי פלטפורמה יכולים לבחון פנימה ולעדכן את הרישום של Connect בלי
עוזב את Node.js. `iterateConnectApps()` דפים ברישום, בעוד
`getConnectStatus()` ו-`getConnectAppPolicy()` חושפים את מוני זמן הריצה
מעטפת המדיניות הנוכחית. `updateConnectAppPolicy()` מקבל שדות camelCase,
כך שתוכל לשלב את אותו מטען JSON ש-Torii מצפה לו.

```ts
const status = await torii.getConnectStatus();
console.log("connect enabled:", status?.enabled ?? false);
console.log("active sessions:", status?.sessionsActive ?? 0);
console.log("buffered bytes:", status?.totalBufferBytes ?? 0);

for await (const app of torii.iterateConnectApps({ limit: 100 })) {
  console.log(app.appId, app.namespaces, app.policy?.relayEnabled ? "relay" : "wallet-only");
}

const policy = await torii.getConnectAppPolicy();
if ((policy.wsPerIpMaxSessions ?? 0) < 5) {
  await torii.updateConnectAppPolicy({
    wsPerIpMaxSessions: 5,
    pingIntervalMs: policy.pingIntervalMs ?? 30_000,
    pingMissTolerance: policy.pingMissTolerance ?? 3,
  });
}
```

צלם תמיד את תמונת המצב העדכנית ביותר של `getConnectStatus()` לפני היישום
מוטציות - רשימת המשימות לממשל דורשת הוכחות לכך שמתחילים עדכוני מדיניות
מהמגבלות הנוכחיות של הצי.

### חיבור WebSocket חיוג

`ToriiClient.openConnectWebSocket()` מרכיב את הקנוני
`/v1/connect/ws` כתובת אתר (כולל `sid`, `role` ופרמטרים אסימון), שדרוגים
`http→ws` / `https→wss`, ומעביר את כתובת האתר הסופית לכל WebSocket
יישום שאתה מספק. דפדפנים משתמשים מחדש באופן אוטומטי בגלובלי
`WebSocket`. מתקשרי Node.js צריכים לעבור בנאי כגון `ws`:

```ts
import WebSocket from "ws";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL ?? "https://torii.nexus.example");
const preview = await torii.createConnectSessionPreview({ chainId: "sora-mainnet" });
const session = await torii.createConnectSession({ sid: preview.sidBase64Url });

const socket = torii.openConnectWebSocket({
  sid: session.sid,
  role: "wallet",
  token: session.token_wallet,
  WebSocketImpl: WebSocket,
  protocols: ["iroha-connect"],
});

socket.addEventListener("message", (event) => {
  console.log("Connect payload", event.data);
});
socket.addEventListener("close", () => {
  console.log("Connect socket closed");
});

socket.binaryType = "arraybuffer";
socket.addEventListener("message", (event) => {
  if (typeof event.data === "string") {
    const control = JSON.parse(event.data);
    console.log("[ws] control", control.kind);
    return;
  }
  pendingFrames.enqueue(new Uint8Array(event.data));
});
```

כאשר אתה צריך רק את כתובת האתר, התקשר ל-`torii.buildConnectWebSocketUrl(params)` או ל-
העוזר `buildConnectWebSocketUrl(baseUrl, params)` ברמה העליונה והשתמש מחדש ב
מחרוזת שהתקבלה בהובלה/תור מותאם אישית.

מחפש מדגם מלא מכוון CLI? ה
[מתכון תצוגה מקדימה לחבר](./recipes/javascript-connect-preview.md) כולל א
סקריפט שניתן להרצה בתוספת הנחיית טלמטריה המשקפת את מפת הדרכים שניתן לספק עבור
תיעוד זרימת תור ה- Connect + WebSocket.

### תור טלמטריה והתראה

העבר מדדי תור ישירות למשטחי העזר כדי שלוחות המחוונים יוכלו לשקף
מדדי ה-KPI של מפת הדרכים.

```ts
import { bootstrapConnectPreviewSession, ConnectQueueError } from "@iroha/iroha-js";

async function dialWithTelemetry(client: ToriiClient) {
  try {
    const { session } = await bootstrapConnectPreviewSession(client, { chainId: "sora-mainnet" });
    queueDepthGauge.record(session.queue_depth ?? 0);
    // …open the WebSocket here…
  } catch (error) {
    if (error instanceof ConnectQueueError) {
      if (error.kind === ConnectQueueError.KIND.OVERFLOW) {
        queueOverflowCounter.add(1, { limit: error.limit ?? 0 });
      } else if (error.kind === ConnectQueueError.KIND.EXPIRED) {
        queueExpiryCounter.add(1, { ttlMs: error.ttlMs ?? 0 });
      }
      return;
    }
    throw error;
  }
}
```

`ConnectQueueError#toConnectError()` ממיר כשלים בתור לגנרי
טקסונומיה `ConnectError` כך שמיירטי HTTP/WebSocket משותפים יכולים לפלוט את
תקן `connect.queue_depth`, `connect.queue_overflow_total`, ו
מדדי `connect.queue_expired_total` המוזכרים בכל מפת הדרכים.

## צופי סטרימינג וסמני אירועים

`ToriiClient.streamEvents()` חושף את `/v1/events/sse` כאיטרטור אסינכרון עם אוטומטי
ניסיונות חוזרים, כך ש-Node/Bun CLIs יכולים לעצור את פעילות הצינור באותו אופן שבו Rust CLI עושה.
החזיקו את הסמן `Last-Event-ID` לצד חפצי ה-runbook שלכם כדי שהמפעילים יוכלו
המשך זרם מבלי לדלג על אירועים כאשר תהליך מופעל מחדש.

```ts
import fs from "node:fs/promises";
import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080");
const cursorFile = process.env.STREAM_CURSOR_FILE ?? ".cache/torii.cursor";
const resumeId = await fs
  .readFile(cursorFile, "utf8")
  .then((value) => value.trim())
  .catch(() => null);
const controller = new AbortController();

process.once("SIGINT", () => controller.abort());
process.once("SIGTERM", () => controller.abort());

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Transaction: { status: "Committed" } } },
  lastEventId: resumeId || undefined,
  signal: controller.signal,
})) {
  if (event.id) {
    await fs.writeFile(cursorFile, `${event.id}\n`, "utf8");
  }
  const status = event.data ? extractPipelineStatusKind(event.data) : null;
  console.log(`[${event.event}] id=${event.id ?? "∅"} status=${status ?? "n/a"}`);
}
```

- מתג `PIPELINE_STATUS` (לדוגמה `Pending`, `Applied` או `Approved`) או הגדר
  `STREAM_FILTER_JSON` כדי להפעיל מחדש את אותם מסננים שה-CLI מקבל.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` שומר על האיטרטור בחיים עד א
  אות מתקבל; לעבור את `STREAM_MAX_EVENTS=25` כאשר אתה צריך רק כמה אירועים ראשונים
  לבדיקת עשן.
- `ToriiClient.streamSumeragiStatus()` משקף את אותו ממשק עבור
  `/v1/sumeragi/status/sse` כך שניתן להתאים טלמטריית קונצנזוס בנפרד, וה
  iterator מכבד את `Last-Event-ID` באותו אופן.
- ראה `javascript/iroha_js/recipes/streaming.mjs` עבור CLI סוהר (התמדה של הסמן,
  עקיפות מסנן env-var, ורישום `extractPipelineStatusKind`) בשימוש ב-JS4
  ניתן לספק מפת דרכים סטרימינג/WebSocket.

## תיקי UAID ומדריך החלל

ממשקי ה-API של Space Directory מציגים את מחזור החיים של מזהה חשבון אוניברסלי (UAID). ה
עוזרים מקבלים `uaid:<hex>` ליטרלים או תקצירים גולמיים של 64 hex (LSB=1) ו
הפוך אותם לקנוניז לפני הגשת בקשות:- `getUaidPortfolio(uaid, { assetId })` צובר יתרות לכל מרחב נתונים,
  קיבוץ החזקות נכסים לפי מזהי חשבון קנוני; העבר את `assetId` כדי לסנן את
  תיק עד למקרה של נכס בודד.
- `getUaidBindings(uaid, { addressFormat })` מונה כל חשבון מרחב נתונים ↔
  מחייב (`addressFormat: "compressed"` מחזירה את ה-`sora…` המילולי).
- `getUaidManifests(uaid, { dataspaceId })` מחזיר כל מניפסט יכולת,
  מצב מחזור החיים, וחשבונות קשורים לביקורת.

עבור חבילות ראיות למפעיל, זרימות פרסום/ביטול של מניפסט, והגירת SDK
הנחיות, עקוב אחר מדריך החשבונות האוניברסלי (`docs/source/universal_accounts_guide.md`)
לצד עוזרי לקוחות אלה, כך שהפורטל ותיעוד המקור יישארו מסונכרנים.

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "cash#global::holder@global",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, { addressFormat: "compressed" });
console.log("bindings", bindings.dataspaces);

const manifests = await torii.getUaidManifests(uaid, { dataspaceId: 11 });
console.log("manifests", manifests.manifests[0].manifest.entries.length);
```

מפעילים יכולים גם לסובב מניפסטים או לבצע זרימות חירום של הכחשת זכיות ללא
ירידה ל-CLI. שני העוזרים מקבלים אובייקט `{ signal }` אופציונלי כך
ניתן לבטל הגשות ארוכות עם `AbortController`; ללא אובייקט
אפשרויות או כניסות שאינן `AbortSignal` מעלות `TypeError` סינכרוני לפני
תוצאות הבקשה Torii:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "ih58...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "ih58...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` מקבל את ה-JSON המניפסט הגולמי (תואם את
מתקנים תחת `fixtures/space_directory/`) או כל אובייקט שמגיע בסידרה ל-
אותו מבנה. `privateKey`, `privateKeyHex`, או `privateKeyMultihash` מפה ל
שדה `ExposedPrivateKey` Torii מצפה וברירת המחדל ל-`ed25519`
אלגוריתם כאשר לא מסופקת קידומת. שתי הבקשות חוזרות פעם אחת בתור Torii
ההוראה (`202 Accepted`), שבשלב זה ספר החשבונות יפלוט את
תואם `SpaceDirectoryEvent`.

## ממשל וגשר ISO

`ToriiClient` חושף את ממשקי ה-API של ממשל לבדיקת חוזים, הבמה
הצעות, הגשת פתקי הצבעה (פשוטים או צ"ק), החלפת מועצה וקריאה
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` ללא DTOs בכתב יד. עוזרי ISO 20022
עקוב אחר אותו דפוס דרך `buildPacs008Message`/`buildPacs009Message` וה-
שלישיית `submitIso*`/`waitForIsoMessageStatus`.

ראה [מתכון גשר ניהול ו-ISO](./recipes/javascript-governance-iso.md)
לדוגמאות מוכנות ל-CLI בתוספת מצביעים חזרה למדריך השטח המלא ב
`docs/source/sdk/js/governance_iso_examples.md`.

## דגימת RBC ומסירה

מפת הדרכים של JS דורשת גם דגימה של Roadrunner Block Commitment (RBC) כדי שהמפעילים יוכלו
להוכיח שהגוש שהם הביאו דרך Sumeragi תואם את ההוכחות שהם מאמתים.
השתמש בעוזרים המובנים במקום לבנות מטענים ביד:

1. מראות `getSumeragiRbcSessions()` `/v1/sumeragi/rbc/sessions`, ו
   `findRbcSamplingCandidate()` בוחר אוטומטית את ההפעלה הראשונה שנמסרה עם hash בלוק
   (חבילת האינטגרציה חוזרת אליה בכל פעם
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` לא מוגדר).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` מנרמל את `{blockHash,height,view}`
   בתוספת `{count,seed,apiToken}` אופציונלי עוקף כך ש-hex או מספרים שלמים שליליים לעולם לא
   להגיע ל-Torii.
3. `sampleRbcChunks()` מפרסם את הבקשה ל-`/v1/sumeragi/rbc/sample`, ומחזיר הוכחות נתחים
   ונתיבים של מרקל (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) אתה צריך לאחסן עם
   שאר עדויות האימוץ שלך.
4. `getSumeragiRbcDelivered(height, view)` לוכד את מטא-נתוני המסירה של הקבוצה כדי שהמבקרים
   יכול להשמיע את ההוכחה מקצה לקצה.

```js
import assert from "node:assert";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080", {
  apiToken: process.env.TORII_API_TOKEN,
});

const candidate =
  (await torii.findRbcSamplingCandidate().catch(() => null)) ??
  (await torii.getSumeragiRbcSessions()).items.find((session) => session.delivered);
if (!candidate) {
  throw new Error("no delivered RBC session available; set IROHA_TORII_INTEGRATION_RBC_SAMPLE");
}

const request = ToriiClient.buildRbcSampleRequest(candidate, {
  count: Number(process.env.RBC_SAMPLE_COUNT ?? 2),
  seed: Number(process.env.RBC_SAMPLE_SEED ?? 0),
  apiToken: process.env.RBC_SAMPLE_API_TOKEN ?? process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks(request);
sample.samples.forEach((chunk) => {
  assert.ok(Buffer.from(chunk.chunkHex, "hex").length > 0, "chunk must be hex");
});

const delivery = await torii.getSumeragiRbcDelivered(sample.height, sample.view);
console.log(
  `rbc height=${sample.height} view=${sample.view} chunks=${sample.samples.length} delivered=${delivery?.delivered}`,
);
```התמידו בשתי התגובות תחת שורש החפץ שאתם מגישים לממשל. תעקוף את
הפעלה שנבחרה אוטומטית באמצעות `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
בכל פעם שאתה צריך לחקור בלוק מסוים, ולהתייחס לכשלים בהבאת צילומי RBC כאל
שגיאת שער לפני טיסה במקום שדרוג לאחור בשקט למצב ישיר.

## בדיקות ו-CI

1. מטמון מטען וחפצי npm.
2. הפעל את `npm run build:native`.
3. בצע את `npm test` (או `node --test` עבור עבודות עשן).

זרימת העבודה של GitHub Actions נמצאת בהפניה
`docs/source/examples/iroha_js_ci.md`.

## השלבים הבאים

- סקור את הסוגים שנוצרו ב-`javascript/iroha_js/index.d.ts`.
- חקור את המתכונים תחת `javascript/iroha_js/recipes/`.
- חבר את `ToriiClient` עם ההתחלה המהירה Norito כדי לבדוק מטענים לצד
  שיחות SDK.