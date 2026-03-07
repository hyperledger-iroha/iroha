---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/javascript.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: جاوا اسکرپٹ ایس ڈی کے کوئیک اسٹارٹ
تفصیل: `@iroha/iroha-js` کے ساتھ لین دین ، ​​اسٹریم ایونٹس ، اور ڈرائیو کنیکٹ کے پیش نظارہ کی تعمیر کریں۔
سلگ: /ایس ڈی کے /جاوا اسکرپٹ
---

`@iroha/iroha-js` Torii کے ساتھ بات چیت کرنے کے لئے کیننیکل نوڈ. جے ایس پیکیج ہے۔ یہ
بنڈل Norito بلڈرز ، ED25519 مددگار ، صفحہ بندی کی افادیت ، اور ایک لچکدار
HTTP/ویب ساکٹ کلائنٹ تاکہ آپ ٹائپ اسکرپٹ سے CLI کے بہاؤ کو آئینہ دے سکیں۔

## تنصیب

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

بلڈ مرحلہ `cargo build -p iroha_js_host` کو لپیٹتا ہے۔ ٹول چین کو یقینی بنائیں
`rust-toolchain.toml` `npm run build:native` چلانے سے پہلے مقامی طور پر دستیاب ہے۔

## کلیدی انتظام

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

## لین دین کی تعمیر

Norito ہدایات بلڈرز شناخت کنندگان ، میٹا ڈیٹا اور مقدار کو معمول پر لائیں
انکوڈڈ ٹرانزیکشنز زنگ/سی ایل آئی پے لوڈ سے ملتے ہیں۔

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

## Torii کلائنٹ کی تشکیل

`ToriiClient` دوبارہ کوشش/ٹائم آؤٹ knobs کو قبول کرتا ہے جو `iroha_config` کا آئینہ دار ہے۔ استعمال کریں
Norito اونٹ کیس کنفیگ آبجیکٹ کو ضم کرنے کے لئے (معمول بنائیں
`iroha_config` پہلے) ، ENV اوور رائڈس ، اور ان لائن آپشنز۔

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

مقامی دیو کے لئے ماحولیاتی متغیرات:

| مختلف ہوتا ہے | مقصد |
| --------- | --------- |
| `IROHA_TORII_TIMEOUT_MS` | ٹائم آؤٹ (ملی سیکنڈ) کی درخواست کریں۔ |
| `IROHA_TORII_MAX_RETRIES` | زیادہ سے زیادہ دوبارہ کوشش کی کوششیں۔ |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | ابتدائی دوبارہ کوشش کرنا بیک آف۔ |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | ایکسپونشنل بیک آف ضرب۔ |
| `IROHA_TORII_MAX_BACKOFF_MS` | زیادہ سے زیادہ دوبارہ کوشش میں تاخیر۔ |
| `IROHA_TORII_RETRY_STATUSES` | کوشش کرنے کے لئے کوما سے الگ HTTP اسٹیٹس کوڈز۔ |
| `IROHA_TORII_RETRY_METHODS` | کوشش کرنے کے لئے کوما سے الگ HTTP کے طریقے۔ |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` شامل کرتا ہے۔ |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` ہیڈر شامل کرتا ہے۔ |

پروفائلز آئینہ Android ڈیفالٹس کی دوبارہ کوشش کریں اور برابری کی جانچ پڑتال کے لئے برآمد کیے جاتے ہیں:
`DEFAULT_TORII_CLIENT_CONFIG` ، `DEFAULT_RETRY_PROFILE_PIPELINE` ،
`DEFAULT_RETRY_PROFILE_STREAMING`۔ `docs/source/sdk/js/torii_retry_policy.md` دیکھیں
اختتامی نقطہ سے پروفائل میپنگ اور پیرامیٹرز گورننس آڈٹ کے دوران
جے ایس 4/جے ایس 7۔

## قابل فہرست فہرستیں اور صفحہ بندی

صفحہ بندی کے مددگار `/v1/accounts` کے لئے ازگر ایس ڈی کے ایرگونومکس کو آئینہ دار کرتے ہیں ،
`/v1/domains` ، `/v1/assets/definitions` ، NFTS ، بیلنس ، اثاثہ ہولڈرز ، اور دی
اکاؤنٹ کے لین دین کی تاریخ۔

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

## آف لائن الاؤنسز اور ورڈکٹ میٹا ڈیٹا

آف لائن الاؤنس کے ردعمل افزودہ لیجر میٹا ڈیٹا اپ فرنٹ کو بے نقاب کرتے ہیں۔
`expires_at_ms` ، `policy_expires_at_ms` ، `refresh_at_ms` ، `verdict_id_hex` ،
`attestation_nonce_hex` ، اور `remaining_amount` خام کے ساتھ ساتھ واپس کردیئے گئے ہیں
ریکارڈ کریں لہذا ڈیش بورڈز کو ایمبیڈڈ Norito پے لوڈ کو ڈی کوڈ کرنے کی ضرورت نہیں ہے۔ نیا
الٹی گنتی مددگار (`deadline_kind` ، `deadline_state` ، `deadline_ms` ،
`deadline_ms_remaining`) اگلی میعاد ختم ہونے والی آخری تاریخ (ریفریش → پالیسی کو اجاگر کریں
→ سرٹیفکیٹ) لہذا جب بھی الاؤنس ہوتا ہے تو UI بیج آپریٹرز کو متنبہ کرسکتے ہیں
<24h باقی۔ ایس ڈی کے
`/v1/offline/allowances` کے ذریعہ بے نقاب باقی فلٹرز کی آئینہ دار:
`certificateExpiresBeforeMs/AfterMs` ، `policyExpiresBeforeMs/AfterMs` ،
`verdictIdHex` ، `attestationNonceHex` ، `refreshBeforeMs/AfterMs` ، اور The
`requireVerdict` / `onlyMissingVerdict` بولینز۔ غلط امتزاج (کے لئے
مثال `onlyMissingVerdict` + `verdictIdHex`) Torii سے پہلے مقامی طور پر مسترد کردی گئی ہے
کہا جاتا ہے۔

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

## آف لائن ٹاپ اپ (جاری کریں + رجسٹر)جب آپ سرٹیفکیٹ جاری کرنا چاہتے ہیں اور فوری طور پر ٹاپ اپ مددگار استعمال کریں
اس کو لیجر پر رجسٹر کریں۔ ایس ڈی کے جاری کردہ اور رجسٹرڈ سرٹیفکیٹ کی تصدیق کرتا ہے
آئی ڈی واپس آنے سے پہلے میچ کرتے ہیں ، اور جواب میں دونوں پے لوڈ شامل ہیں۔ وہاں ہے
کوئی سرشار ٹاپ اپ اختتامی نقطہ نہیں۔ مددگار زنجیروں میں اس مسئلے کو + رجسٹر کال کرتا ہے۔ یو
آپ کے پاس پہلے ہی دستخط شدہ سرٹیفکیٹ ہے ، `registerOfflineAllowance` پر کال کریں (یا
`renewOfflineAllowance`) براہ راست۔

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "<account_ih58>",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "<account_ih58>",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii سوالات اور اسٹریمنگ (ویب ساکٹس)

استفسار مددگار حیثیت ، Prometheus میٹرکس ، ٹیلی میٹری اسنیپ شاٹس ، اور واقعہ کو بے نقاب کرتے ہیں
Norito فلٹر گرائمر کا استعمال کرتے ہوئے اسٹریمز۔ اسٹریمنگ خود بخود اپ گریڈ کرتی ہے
ویب ساکٹس اور دوبارہ شروع ہونے پر دوبارہ شروع ہوتا ہے جب دوبارہ کوشش کرنے والا بجٹ اجازت دیتا ہے۔

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

دوسرے کے لئے `streamBlocks` ، `streamTransactions` ، یا `streamTelemetry` استعمال کریں
ویب ساکٹ اینڈ پوائنٹس۔ تمام اسٹریمنگ مددگار کوششوں کی سطح پر دوبارہ کوشش کرتے ہیں ، لہذا اس کو ہک
`onReconnect` کال بیک فیڈ ڈیش بورڈز اور انتباہ کرنے کے لئے۔

## سنیپ شاٹس اور کیو آر پے لوڈز کو دریافت کریں

ایکسپلورر ٹیلی میٹری `/v1/explorer/metrics` اور کے لئے ٹائپڈ مددگار فراہم کرتا ہے
`/v1/explorer/accounts/{account_id}/qr` اختتامی نکات تاکہ ڈیش بورڈز دوبارہ چلاسکیں
وہی سنیپ شاٹس جو پورٹل کو طاقت دیتے ہیں۔ `getExplorerMetrics()` کو معمول بناتا ہے
جب راستہ غیر فعال ہوتا ہے تو پے لوڈ اور واپسی `null`۔ اس کے ساتھ جوڑ
`getExplorerAccountQr()` جب بھی آپ کو IH58 (ترجیحی)/سورہ (دوسرا بہترین) لٹریلس پلس ان لائن کی ضرورت ہو
شیئر بٹنوں کے لئے ایس وی جی۔

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

پاسنگ `addressFormat: "compressed"` ایکسپلورر کے پہلے سے طے شدہ کمپریسڈ آئینہ
سلیکٹرز ؛ ترجیحی IH58 آؤٹ پٹ کے لئے اوور رائڈ کو چھوڑ دیں یا `ih58_qr` کی درخواست کریں
جب آپ کو QR-SAFE مختلف قسم کی ضرورت ہو۔ کمپریسڈ لفظی دوسرا بہترین ہے
UX کے لئے صرف SORA- آپشن۔ مددگار ہمیشہ کیننیکل شناخت کنندہ کو لوٹاتا ہے ،
منتخب لفظی ، اور میٹا ڈیٹا (نیٹ ورک کا سابقہ ​​، QR ورژن/ماڈیول ، خرابی
اصلاح کا درجہ ، اور ان لائن SVG) ، لہذا CI/CD وہی پے لوڈ شائع کرسکتا ہے جو
ایکسپلورر بیسپوک کنورٹرز کو فون کیے بغیر سطح پر ہے۔

## سیشنز اور قطار میں جڑیں

کنیکٹ ہیلپرز آئینہ `docs/source/connect_architecture_strawman.md`۔
پیش نظارہ کے لئے تیار سیشن کا تیز ترین راستہ `bootstrapConnectPreviewSession` ہے ،
جو ایک ساتھ مل کر ڈٹرمینسٹک سیڈ/یو آر آئی جنریشن اور Torii کو ایک ساتھ ٹانکا دیتا ہے
رجسٹریشن کال

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

- `register: false` کو پاس کریں جب آپ کو صرف QR/DEEPLINK کے لئے ڈٹرمینسٹک URIS کی ضرورت ہو
  پیش نظارہ
- `generateConnectSid` دستیاب رہتا ہے جب آپ کو سیشن IDs حاصل کرنے کی ضرورت ہوتی ہے
  uris کے بغیر.
- دشاتمک چابیاں اور سائفر ٹیکسٹ لفافے آبائی پل سے آتے ہیں۔ جب
  دستیاب نہیں ہے SDK JSON کوڈیک پر واپس آتا ہے اور پھینک دیتا ہے
  `ConnectQueueError.bridgeUnavailable`۔
- آف لائن بفر Norito `.to` BLOBs کے طور پر انڈیکسڈ ڈی بی میں محفوظ ہیں۔ دم کی نگرانی کریں
  خارج `ConnectQueueError.overflow(limit)` / کے ذریعے ریاست
  `.expired(ttlMs)` غلطیاں اور فیڈ `connect.queue_depth` ٹیلی میٹری جیسا کہ بیان کیا گیا ہے
  روڈ میپ میں

### رجسٹری اور پالیسی سنیپ شاٹس سے رابطہ کریںپلیٹ فارم آپریٹرز بغیر کنیکٹ رجسٹری کو انٹرا اسپیکٹ اور اپ ڈیٹ کرسکتے ہیں
نوڈ. js. چھوڑنا `iterateConnectApps()` صفحات رجسٹری کے ذریعے ، جبکہ
`getConnectStatus()` اور `getConnectAppPolicy()` رن ٹائم کاؤنٹرز کو بے نقاب کریں اور
موجودہ پالیسی لفافہ۔ `updateConnectAppPolicy()` اونٹ کیس فیلڈز کو قبول کرتا ہے ،
لہذا آپ وہی JSON پے لوڈ کر سکتے ہیں جس کی Torii کی توقع ہے۔

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

درخواست دینے سے پہلے ہمیشہ تازہ ترین `getConnectStatus()` اسنیپ شاٹ پر قبضہ کریں
تغیرات - گورننس چیک لسٹ میں اس بات کا ثبوت درکار ہوتا ہے کہ پالیسی کی تازہ کارییں شروع ہوتی ہیں
بیڑے کی موجودہ حدود سے۔

### ویب سائٹ ڈائلنگ سے رابطہ کریں

`ToriiClient.openConnectWebSocket()` نے کیننیکل کو جمع کیا
`/v1/connect/ws` URL (بشمول `sid` ، `role` ، اور ٹوکن پیرامیٹرز) ، اپ گریڈ
`http→ws` / `https→wss` ، اور حتمی URL کو جو بھی ویب سائٹ پر دے رہا ہے
عمل درآمد جو آپ سپلائی کرتے ہیں۔ براؤزر خود بخود عالمی سطح پر دوبارہ استعمال کرتے ہیں
`WebSocket`۔ نوڈ ڈاٹ جے ایس کال کرنے والوں کو ایک کنسٹرکٹر کو پاس کرنا چاہئے جیسے `ws`:

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

جب آپ کو صرف URL کی ضرورت ہو تو ، `torii.buildConnectWebSocketUrl(params)` یا کال کریں
ٹاپ لیول `buildConnectWebSocketUrl(baseUrl, params)` مددگار اور دوبارہ استعمال کریں
کسٹم ٹرانسپورٹ/قطار میں نتیجے میں تار۔

ایک مکمل کلائی پر مبنی نمونے کی تلاش ہے؟
[Connect preview recipe](./recipes/javascript-connect-preview.md) includes a
چلانے کے قابل اسکرپٹ کے علاوہ ٹیلی میٹری رہنمائی جو روڈ میپ کی فراہمی کے لئے آئینہ دار ہے
کنیکٹ کی قطار + ویب ساکٹ فلو کی دستاویز کرنا۔

### قطار ٹیلی میٹری اور انتباہ

وائر قطار میٹرکس براہ راست مددگار سطحوں پر
روڈ میپ کے پی آئی۔

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

`ConnectQueueError#toConnectError()` قطار کی ناکامیوں کو عام میں تبدیل کرتا ہے
`ConnectError` ٹیکسنومی لہذا مشترکہ HTTP/Websocket انٹرسیپٹرز خارج کرسکتے ہیں
معیاری `connect.queue_depth` ، `connect.queue_overflow_total` ، اور
`connect.queue_expired_total` میٹرکس روڈ میپ میں حوالہ دیا گیا۔

## نگاہ رکھنے والے اور ایونٹ کے کرسر کو اسٹریمنگ کرنا

`ToriiClient.streamEvents()` `/v1/events/sse` کو خودکار کے ساتھ Async Iterator کے طور پر بے نقاب کرتا ہے
دوبارہ کوششیں ، لہذا نوڈ/بن سی ایل آئی ایس پائپ لائن کی سرگرمی کو اسی طرح دم کر سکتا ہے جس طرح مورچا سی ایل آئی کرتا ہے۔
اپنے رن بوک نمونے کے ساتھ ساتھ `Last-Event-ID` کرسر کو بھی برقرار رکھیں تاکہ آپریٹرز کرسکیں
جب عمل دوبارہ شروع ہوجائے تو واقعات کو چھوڑنے کے بغیر ایک ندی دوبارہ شروع کریں۔

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

- سوئچ `PIPELINE_STATUS` (مثال کے طور پر `Pending` ، `Applied` ، یا `Approved`) یا سیٹ کریں
  `STREAM_FILTER_JSON` اسی فلٹرز کو دوبارہ چلانے کے لئے CLI قبول کرتا ہے۔
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` تکرار کو زندہ رکھتا ہے جب تک کہ
  سگنل موصول ہوا ہے ؛ جب آپ کو صرف پہلے چند واقعات کی ضرورت ہو تو `STREAM_MAX_EVENTS=25` پاس کریں
  دھواں ٹیسٹ کے لئے۔
- `ToriiClient.streamSumeragiStatus()` اسی انٹرفیس کے لئے آئینہ دار ہے
  `/v1/sumeragi/status/sse` لہذا اتفاق رائے ٹیلی میٹری کو الگ سے تفصیل سے سمجھا جاسکتا ہے ، اور
  Iterator `Last-Event-ID` اسی طرح آنرز کرتا ہے۔
- CLI ٹرنکی کے لئے `javascript/iroha_js/recipes/streaming.mjs` دیکھیں (کرسر استقامت ،
  env-var فلٹر اوور رائڈز ، اور `extractPipelineStatusKind` لاگنگ) JS4 میں استعمال کیا جاتا ہے
  اسٹریمنگ/ویب ساکٹ روڈ میپ کی فراہمی۔

## UAID پورٹ فولیوز اور خلائی ڈائرکٹری

خلائی ڈائرکٹری APIs یونیورسل اکاؤنٹ ID (UAID) لائف سائیکل کی سطح پر ہے۔
مددگار `uaid:<hex>` لٹرالس یا را 64-ہیکس ڈائجسٹس (LSB = 1) کو قبول کرتے ہیں اور
درخواستیں جمع کروانے سے پہلے ان کو کیننیکلائز کریں:- `getUaidPortfolio(uaid, { assetId })` مجموعی طور پر توازن فی ڈیٹا اسپیس ،
  کیننیکل اکاؤنٹ IDs کے ذریعہ اثاثوں کے حصول کو گروپ کرنا ؛ فلٹر کرنے کے لئے `assetId` پاس کریں
  ایک ہی اثاثہ مثال کے طور پر پورٹ فولیو۔
- `getUaidBindings(uaid, { addressFormat })` ہر ڈیٹا اسپیس ↔ اکاؤنٹ کی گنتی کرتا ہے
  بائنڈنگ (`addressFormat: "compressed"` `sora…` لفظی لوٹاتا ہے)۔
- `getUaidManifests(uaid, { dataspaceId })` ہر صلاحیت کو ظاہر کرتا ہے ،
  لائف سائیکل کی حیثیت ، اور آڈٹ کے لئے پابند اکاؤنٹس۔

آپریٹر شواہد پیک کے لئے ، منشور شائع کریں/فلو کو منسوخ کریں ، اور ایس ڈی کے ہجرت
رہنمائی ، یونیورسل اکاؤنٹ گائیڈ (`docs/source/universal_accounts_guide.md`) کی پیروی کریں
ان کلائنٹ مددگاروں کے ساتھ ساتھ پورٹل اور ماخذ دستاویزات ہم آہنگی میں ہی رہیں۔

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

آپریٹرز منشور کو بھی گھوم سکتے ہیں یا بغیر کسی ہنگامی طور پر ہنگامی طور پر انکار کی جیت کے بہاؤ کو انجام دے سکتے ہیں
سی ایل آئی کو گر رہا ہے۔ دونوں مددگار ایک اختیاری `{ signal }` آبجیکٹ کو قبول کرتے ہیں
`AbortController` کے ساتھ طویل عرصے سے چلنے والی گذارشات منسوخ کی جاسکتی ہیں۔ غیر آبجیکٹ
اختیارات یا غیر `AbortSignal` ان پٹ ایک ہم وقت ساز `TypeError` کو اس سے پہلے اٹھاتے ہیں
درخواست ہٹٹس Torii:

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

`publishSpaceDirectoryManifest()` یا تو کچے منشور JSON (مماثل سے ملاپ کرتا ہے
`fixtures/space_directory/` کے تحت فکسچر) یا کوئی بھی شے جو سیریلائز کرتا ہے
ایک ہی ڈھانچہ. `privateKey` ، `privateKeyHex` ، یا `privateKeyMultihash` نقشہ
`ExposedPrivateKey` فیلڈ Torii توقع کرتا ہے اور `ed25519` سے پہلے سے طے شدہ ہے
الگورتھم جب کوئی سابقہ ​​فراہم نہیں کیا جاتا ہے۔ دونوں درخواستیں ایک بار Torii enqueues سے لوٹ آئیں
ہدایات (`202 Accepted`) ، جس مقام پر لیجر کا اخراج ہوگا
`SpaceDirectoryEvent` سے ملاپ۔

## گورننس اور آئی ایس او برج

`ToriiClient` معاہدوں کا معائنہ کرنے ، اسٹیجنگ کے لئے گورننس API کو بے نقاب کرتا ہے
تجاویز ، بیلٹ جمع کروانا (سادہ یا زیڈ کے) ، کونسل کو گھومانا ، اور کال کرنا
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` بغیر ہاتھ سے لکھے ہوئے DTOS کے۔ آئی ایس او 20022 مددگار
`buildPacs008Message`/`buildPacs009Message` اور دی کے ذریعے اسی پیٹرن پر عمل کریں
`submitIso*`/`waitForIsoMessageStatus` تینوں۔

[گورننس اور آئی ایس او برج ہدایت] (./recipes/javascript-governance-iso.md) دیکھیں
کلیڈی تیار نمونے پلس پوائنٹرز کے لئے مکمل فیلڈ گائیڈ میں واپس
`docs/source/sdk/js/governance_iso_examples.md`۔

## آر بی سی کے نمونے لینے اور ترسیل کے ثبوت

جے ایس روڈ میپ کے لئے روڈرنر بلاک کمٹمنٹ (آر بی سی) کے نمونے لینے کی بھی ضرورت ہے تاکہ آپریٹرز کر سکتے ہیں
یہ ثابت کریں کہ انہوں نے جس بلاک کو Sumeragi کے ذریعے لایا ہے وہ ان کے ثبوت سے مماثل ہے جس کی وہ تصدیق کرتے ہیں۔
ہاتھ سے پے لوڈ بنانے کے بجائے بلٹ ان مددگاروں کا استعمال کریں:

1. `getSumeragiRbcSessions()` آئینہ `/v1/sumeragi/rbc/sessions` ، اور
   `findRbcSamplingCandidate()` آٹو سلیکٹس پہلے ڈیلیور سیشن کو بلاک ہیش کے ساتھ
   (انضمام سویٹ جب بھی اس پر واپس آجاتا ہے
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` غیر سیٹ ہے)۔
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` معیاری `{blockHash,height,view}`
   پلس اختیاری `{count,seed,apiToken}` اوور رائڈس تو خراب شدہ ہیکس یا منفی انٹیگر کبھی نہیں
   Torii تک پہنچیں۔
3. `sampleRbcChunks()` `/v1/sumeragi/rbc/sample` پر درخواست پوسٹ کرتا ہے۔
   اور مرکل کے راستے (`samples[].chunkHex` ، `chunkRoot` ، `payloadHash`) آپ کو محفوظ شدہ دستاویزات بنائیں
   آپ کو اپنانے کا باقی ثبوت۔
4. `getSumeragiRbcDelivered(height, view)` نے ہم آہنگی کی ترسیل میٹا ڈیٹا کو حاصل کیا تاکہ آڈیٹرز
   ثبوت کے آخر سے آخر میں دوبارہ چلا سکتے ہیں۔

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
```روٹ آرٹیکٹیکٹ کے تحت دونوں ردعمل کو برقرار رکھیں جس کو آپ گورننس میں پیش کرتے ہیں۔ اوور رائڈ
`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` کے ذریعے آٹو منتخب کردہ سیشن
جب بھی آپ کو کسی مخصوص بلاک کی تحقیقات کرنے کی ضرورت ہوتی ہے ، اور آر بی سی اسنیپ شاٹس کو ایک کے طور پر لانے میں ناکامیوں کا علاج کریں
براہ راست موڈ میں خاموشی سے نیچے آنے کے بجائے پرواز سے پہلے کی گیٹنگ کی غلطی۔

## ٹیسٹنگ اور سی آئی

1. کارگو کیشے اور این پی ایم نمونے۔
2. `npm run build:native` چلائیں۔
3. `npm test` پر عمل کریں (یا دھواں ملازمتوں کے لئے `node --test`)۔

حوالہ گٹ ہب ایکشن ورک فلو میں رہتا ہے
`docs/source/examples/iroha_js_ci.md`۔

## اگلے اقدامات

- `javascript/iroha_js/index.d.ts` میں تیار کردہ اقسام کا جائزہ لیں۔
- `javascript/iroha_js/recipes/` کے تحت ترکیبیں دریافت کریں۔
- Norito کوئیک اسٹارٹ کے ساتھ جوڑے کے ساتھ ساتھ پے لوڈ کا معائنہ کرنے کے لئے `ToriiClient`
  ایس ڈی کے کالز۔