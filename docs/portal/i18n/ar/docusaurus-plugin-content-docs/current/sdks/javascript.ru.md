---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/javascript.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: التشغيل السريع لـ JavaScript SDK
الوصف: إنشاء المعاملات، وبث الأحداث، وقيادة معاينات Connect باستخدام `@iroha/iroha-js`.
سبيكة: /sdks/javascript
---

`@iroha/iroha-js` هي حزمة Node.js الأساسية للتفاعل مع Torii. ذلك
حزم منشئي Norito ومساعدي Ed25519 وأدوات مساعدة لترقيم الصفحات وأدوات مرنة
عميل HTTP/WebSocket حتى تتمكن من عكس تدفقات واجهة سطر الأوامر (CLI) من TypeScript.

## التثبيت

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

تلتف خطوة الإنشاء `cargo build -p iroha_js_host`. تأكد من سلسلة الأدوات من
يتوفر `rust-toolchain.toml` محليًا قبل تشغيل `npm run build:native`.

## إدارة المفاتيح

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

## بناء المعاملات

يقوم منشئو التعليمات Norito بتطبيع المعرفات وبيانات التعريف والكميات
تتطابق المعاملات المشفرة مع حمولات Rust/CLI.

```ts
import {
  buildMintAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
} from "@iroha/iroha-js";

const mint = buildMintAssetInstruction({
  assetId: "norito:4e52543000000001",
  quantity: "10",
});

const transfer = buildTransferAssetInstruction({
  sourceAssetId: "norito:4e52543000000001",
  destinationAccountId: "<katakana-i105-account-id>",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "<katakana-i105-account-id>",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "<katakana-i105-account-id>", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## تكوين العميل Torii

يقبل `ToriiClient` مقابض إعادة المحاولة/المهلة التي تعكس `iroha_config`. استخدم
`resolveToriiClientConfig` لدمج كائن تكوين CamelCase (تطبيع
`iroha_config` أولاً)، وتجاوزات env، والخيارات المضمنة.

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

متغيرات البيئة للتطوير المحلي:| متغير | الغرض |
|----------|--------|
| `IROHA_TORII_TIMEOUT_MS` | مهلة الطلب (ملي ثانية). |
| `IROHA_TORII_MAX_RETRIES` | الحد الأقصى لمحاولات إعادة المحاولة. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | التراجع عن إعادة المحاولة الأولية. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | مضاعف التراجع الأسي. |
| `IROHA_TORII_MAX_BACKOFF_MS` | الحد الأقصى لتأخير إعادة المحاولة. |
| `IROHA_TORII_RETRY_STATUSES` | رموز حالة HTTP مفصولة بفواصل لإعادة المحاولة. |
| `IROHA_TORII_RETRY_METHODS` | طرق HTTP مفصولة بفواصل لإعادة المحاولة. |
| `IROHA_TORII_API_TOKEN` | يضيف `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | إضافة رأس `Authorization: Bearer …`. |

تعكس ملفات تعريف إعادة المحاولة الإعدادات الافتراضية لنظام Android ويتم تصديرها للتحقق من التكافؤ:
`DEFAULT_TORII_CLIENT_CONFIG`، `DEFAULT_RETRY_PROFILE_PIPELINE`،
`DEFAULT_RETRY_PROFILE_STREAMING`. انظر `docs/source/sdk/js/torii_retry_policy.md`
لتعيين نقطة النهاية إلى الملف الشخصي وعمليات تدقيق حوكمة المعلمات أثناء
JS4/JS7.

## قوائم قابلة للتكرار وترقيم الصفحات

تعكس مساعدات ترقيم الصفحات بيئة العمل الخاصة بـ Python SDK لـ `/v1/accounts`،
`/v1/domains`، `/v1/assets/definitions`، NFTs، الأرصدة، أصحاب الأصول، و
تاريخ معاملات الحساب.

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

const assetId = "norito:4e52543000000001";
const balances = await torii.listAccountAssets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## البدلات غير المتصلة بالإنترنت والبيانات الوصفية للحكمتكشف استجابات المخصصات دون اتصال عن بيانات تعريف دفتر الأستاذ المعززة مقدمًا —
`expires_at_ms`، `policy_expires_at_ms`، `refresh_at_ms`، `verdict_id_hex`،
يتم إرجاع `attestation_nonce_hex` و`remaining_amount` إلى جانب الملف الخام
قم بالتسجيل حتى لا تضطر لوحات المعلومات إلى فك تشفير حمولات Norito المضمنة. الجديد
مساعدو العد التنازلي (`deadline_kind`، `deadline_state`، `deadline_ms`،
`deadline_ms_remaining`) قم بتسليط الضوء على الموعد النهائي التالي لانتهاء الصلاحية (التحديث → السياسة
→ الشهادة) حتى تتمكن شارات واجهة المستخدم من تحذير المشغلين عند وجود بدل
<24 ساعة متبقية. SDK
يعكس مرشحات REST المكشوفة بواسطة `/v1/offline/reserve/topup`:
`certificateExpiresBeforeMs/AfterMs`، `policyExpiresBeforeMs/AfterMs`،
`verdictIdHex`، `attestationNonceHex`، `refreshBeforeMs/AfterMs`، و
`requireVerdict` / `onlyMissingVerdict` القيم المنطقية. مجموعات غير صالحة (ل
مثال `onlyMissingVerdict` + `verdictIdHex`) تم رفضه محليًا قبل Torii
يسمى.

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

## عمليات تعبئة الرصيد دون الاتصال بالإنترنت (الإصدار + التسجيل)

استخدم مساعدي تعبئة الرصيد عندما تريد إصدار شهادة وعلى الفور
تسجيله على دفتر الأستاذ. يتحقق SDK من الشهادة الصادرة والمسجلة
تتطابق المعرفات قبل العودة، وتتضمن الاستجابة كلا الحمولتين. هناك
لا توجد نقطة نهاية مخصصة لزيادة الرصيد؛ يقوم المساعد بتسلسل المشكلة + تسجيل المكالمات. إذا
لديك بالفعل شهادة موقعة، اتصل بالرقم `registerOfflineAllowance` (أو
`renewOfflineAllowance`) مباشرة.

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "<account_i105>",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "<account_i105>",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii الاستعلامات والبث (WebSockets)تعرض مساعدات الاستعلام الحالة ومقاييس Prometheus ولقطات القياس عن بعد والحدث
التدفقات باستخدام قواعد التصفية Norito. يتم ترقية البث تلقائيًا إلى
WebSockets ويستأنف عندما تسمح ميزانية إعادة المحاولة.

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

استخدم `streamBlocks`، أو `streamTransactions`، أو `streamTelemetry` للأجهزة الأخرى
نقاط النهاية WebSocket. تظهر جميع مساعدات البث محاولات إعادة المحاولة، لذا قم بتوصيل
رد اتصال `onReconnect` لتغذية لوحات المعلومات والتنبيهات.

## لقطات المستكشف وحمولات QR

يوفر القياس عن بعد في Explorer مساعدين مكتوبين لـ `/v1/explorer/metrics` و
نقاط النهاية `/v1/explorer/accounts/{account_id}/qr` حتى تتمكن لوحات المعلومات من إعادة تشغيل
نفس اللقطات التي تعمل على تشغيل البوابة. `getExplorerMetrics()` يقوم بتطبيع ملف
الحمولة وإرجاع `null` عند تعطيل المسار. إقرانها مع
`getExplorerAccountQr()` عندما تحتاج إلى i105 (المفضل)/sora (ثاني أفضل) حرفية بالإضافة إلى المضمنة
SVG لأزرار المشاركة.

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

const qr = await torii.getExplorerAccountQr("<katakana-i105-account-id>");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

يؤدي تمرير `i105` إلى عكس ضغط Explorer الافتراضي
محددات. حذف التجاوز لمخرج i105 المفضل أو طلب `i105_qr`
عندما تحتاج إلى متغير QR الآمن. الحرفي المضغوط هو ثاني أفضل
خيار Sora فقط لـ UX. يقوم المساعد دائمًا بإرجاع المعرف الأساسي،
البيانات الحرفية والبيانات التعريفية المحددة (بادئة الشبكة، إصدار/وحدات QR، الخطأ
طبقة التصحيح، وSVG المضمنة)، بحيث يمكن لـ CI/CD نشر نفس الحمولات النافعة
أسطح Explorer دون استدعاء محولات مخصصة.## ربط الجلسات وقائمة الانتظار

تعكس مساعدات الاتصال `docs/source/connect_architecture_strawman.md`. ال
أسرع مسار لجلسة جاهزة للمعاينة هو `bootstrapConnectPreviewSession`،
الذي يجمع بين توليد SID/URI الحتمي وTorii
مكالمة التسجيل.

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

- قم بتمرير `register: false` عندما تحتاج فقط إلى معرفات URI محددة لـ QR/deeplink
  معاينات.
- يظل `generateConnectSid` متاحًا عندما تحتاج إلى استخلاص معرفات الجلسة
  دون سك عناوين URI.
- مفاتيح الاتجاه ومظاريف النص المشفر تأتي من الجسر الأصلي؛ متى
  غير متوفر، يعود SDK إلى برنامج ترميز JSON ويلقي
  `ConnectQueueError.bridgeUnavailable`.
- يتم تخزين المخازن المؤقتة غير المتصلة كـ Norito `.to` blobs في IndexedDB. مراقبة قائمة الانتظار
  الحالة عبر `ConnectQueueError.overflow(limit)` / المنبعثة
  أخطاء `.expired(ttlMs)` وتغذية القياس عن بعد `connect.queue_depth` كما هو موضح
  في خارطة الطريق.

### ربط لقطات التسجيل والسياسة

يمكن لمشغلي النظام الأساسي استكشاف سجل Connect وتحديثه دون الحاجة إلى ذلك
مغادرة Node.js. صفحات `iterateConnectApps()` من خلال التسجيل، في حين
يعرض `getConnectStatus()` و`getConnectAppPolicy()` عدادات وقت التشغيل و
غلاف السياسة الحالية. `updateConnectAppPolicy()` يقبل حقول حالة الجمل،
حتى تتمكن من تنظيم نفس حمولة JSON التي يتوقعها Torii.

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

قم دائمًا بالتقاط أحدث لقطة `getConnectStatus()` قبل التقديم
الطفرات - تتطلب القائمة المرجعية للحوكمة دليلاً على بدء تحديثات السياسات
من حدود الأسطول الحالية.### توصيل اتصال WebSocket

يقوم `ToriiClient.openConnectWebSocket()` بتجميع الملف الأساسي
عنوان URL `/v1/connect/ws` (بما في ذلك `sid`، و`role`، ومعلمات الرمز المميز)، والترقيات
`http→ws` / `https→wss`، ويسلم عنوان URL النهائي إلى أي WebSocket
التنفيذ الذي تقوم بتزويده. تقوم المتصفحات تلقائيًا بإعادة استخدام النطاق العالمي
`WebSocket`. يجب على مناديب Node.js تمرير مُنشئ مثل `ws`:

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

عندما تحتاج فقط إلى عنوان URL، اتصل بـ `torii.buildConnectWebSocketUrl(params)` أو
مساعد `buildConnectWebSocketUrl(baseUrl, params)` عالي المستوى وإعادة استخدام ملف
السلسلة الناتجة في قائمة انتظار/نقل مخصصة.

هل تبحث عن عينة كاملة موجهة لـ CLI؟ ال
[وصفة معاينة الاتصال](./recipes/javascript-connect-preview.md) تتضمن أ
نص قابل للتشغيل بالإضافة إلى إرشادات القياس عن بعد التي تعكس خارطة الطريق التي يمكن تسليمها لـ
توثيق قائمة انتظار الاتصال + تدفق WebSocket.

### القياس عن بعد والتنبيه في قائمة الانتظار

قم بتوصيل مقاييس قائمة الانتظار مباشرة إلى الأسطح المساعدة حتى تتمكن لوحات المعلومات من عكسها
مؤشرات الأداء الرئيسية لخارطة الطريق.

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

يقوم `ConnectQueueError#toConnectError()` بتحويل حالات فشل قائمة الانتظار إلى فشل عام
تصنيف `ConnectError` بحيث يمكن لمعترضات HTTP/WebSocket المشتركة إصدار
المعيار `connect.queue_depth`، و`connect.queue_overflow_total`، و
مقاييس `connect.queue_expired_total` المشار إليها في خريطة الطريق.

## مراقبو البث ومؤشرات الأحداثيعرض `ToriiClient.streamEvents()` `/v1/events/sse` كمكرر غير متزامن مع تلقائي
إعادة المحاولة، لذلك يمكن لـ Node/Bun CLIs تتبع نشاط خط الأنابيب بنفس الطريقة التي تعمل بها Rust CLI.
استمر في استخدام المؤشر `Last-Event-ID` بجانب عناصر دليل التشغيل الخاص بك حتى يتمكن المشغلون من
استئناف الدفق دون تخطي الأحداث عند إعادة تشغيل العملية.

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

- قم بالتبديل `PIPELINE_STATUS` (على سبيل المثال `Pending` أو `Applied` أو `Approved`) أو قم بتعيينه
  `STREAM_FILTER_JSON` لإعادة تشغيل نفس المرشحات التي يقبلها CLI.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` يبقي المكرِّر على قيد الحياة حتى أ
  يتم استقبال الإشارة قم بتمرير `STREAM_MAX_EVENTS=25` عندما تحتاج فقط إلى الأحداث القليلة الأولى
  لاختبار الدخان.
- `ToriiClient.streamSumeragiStatus()` يعكس نفس الواجهة لـ
  `/v1/sumeragi/status/sse` لذلك يمكن تفصيل القياس عن بعد المتفق عليه بشكل منفصل، و
  يكرم المكرر `Last-Event-ID` بنفس الطريقة.
- راجع `javascript/iroha_js/recipes/streaming.mjs` للتعرف على واجهة سطر الأوامر الجاهزة (استمرارية المؤشر،
  تجاوزات مرشح env-var وتسجيل `extractPipelineStatusKind`) المستخدمة في JS4
  البث / خريطة طريق WebSocket قابلة للتسليم.

## محافظ UAID ودليل الفضاء

تعرض واجهات برمجة تطبيقات دليل الفضاء دورة حياة معرف الحساب العالمي (UAID). ال
يقبل المساعدون حرف `uaid:<hex>` أو الملخصات الأولية ذات 64 سداسيًا (LSB=1) و
قم بتحديدها بشكل أساسي قبل تقديم الطلبات:- يقوم `getUaidPortfolio(uaid, { assetId })` بتجميع الأرصدة لكل مساحة بيانات،
  تجميع ممتلكات الأصول حسب معرفات الحساب الأساسية؛ قم بتمرير `assetId` لتصفية ملف
  المحفظة وصولاً إلى مثيل أصل واحد.
- يقوم `getUaidBindings(uaid)` بتعداد كل حساب ↔ لمساحة البيانات
  الربط (`i105` يُرجع القيم الحرفية `i105`).
- `getUaidManifests(uaid, { dataspaceId })` يُرجع كل بيان قدرة،
  حالة دورة الحياة، والحسابات المقيدة للتدقيق.

بالنسبة لحزم أدلة المشغل، وتدفقات النشر/الإلغاء الواضحة، وترحيل SDK
التوجيه، اتبع دليل الحساب العالمي (`docs/source/universal_accounts_guide.md`)
جنبًا إلى جنب مع مساعدي العملاء بحيث تظل البوابة الإلكترونية ووثائق المصدر متزامنتين.

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "norito:4e52543000000002",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, {} );
console.log("bindings", bindings.dataspaces);

const manifests = await torii.getUaidManifests(uaid, { dataspaceId: 11 });
console.log("manifests", manifests.manifests[0].manifest.entries.length);
```

يمكن للمشغلين أيضًا تدوير البيانات أو تنفيذ تدفقات رفض الفوز في حالات الطوارئ بدونها
يسقط إلى CLI. يقبل كلا المساعدين كائن `{ signal }` الاختياري
يمكن إلغاء عمليات الإرسال طويلة الأمد باستخدام `AbortController`؛ غير كائن
تؤدي الخيارات أو المدخلات غير `AbortSignal` إلى رفع `TypeError` المتزامن قبل
يصل الطلب إلى Torii:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "<katakana-i105-account-id>",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "<katakana-i105-account-id>",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```يقبل `publishSpaceDirectoryManifest()` إما البيان الخام JSON (المطابق لملف
التركيبات تحت `fixtures/space_directory/`) أو أي كائن يتم تسلسله إلى
نفس الهيكل. تعيين `privateKey` أو `privateKeyHex` أو `privateKeyMultihash` إلى
الحقل `ExposedPrivateKey` المتوقع Torii والقيمة الافتراضية هي `ed25519`
الخوارزمية عندما لا يتم توفير أي بادئة. يتم إرجاع كلا الطلبين بمجرد وضع Torii في قائمة الانتظار
التعليمات (`202 Accepted`)، وعند هذه النقطة سيصدر دفتر الأستاذ الملف
مطابقة `SpaceDirectoryEvent`.

## جسر الحوكمة وISO

يعرض `ToriiClient` واجهات برمجة تطبيقات الإدارة لفحص العقود والتجهيز المرحلي
المقترحات، وتقديم بطاقات الاقتراع (العادية أو ZK)، وتناوب المجلس، والدعوة
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` بدون DTOs مكتوبة بخط اليد. مساعدين ISO 20022
اتبع نفس النمط عبر `buildPacs008Message`/`buildPacs009Message` و
الثلاثي `submitIso*`/`waitForIsoMessageStatus`.

راجع [وصفة الحوكمة وجسر ISO](./recipes/javascript-governance-iso.md)
للحصول على عينات جاهزة لـ CLI بالإضافة إلى مؤشرات للعودة إلى الدليل الميداني الكامل في
`docs/source/sdk/js/governance_iso_examples.md`.

## أخذ عينات كرات الدم الحمراء وأدلة التسليم

تتطلب خريطة طريق JS أيضًا أخذ عينات من التزام كتلة Roadrunner (RBC) حتى يتمكن المشغلون من القيام بذلك
إثبات أن الكتلة التي جلبوها من خلال Sumeragi تتطابق مع أدلة القطعة التي قاموا بالتحقق منها.
استخدم المساعدين المضمنين بدلاً من إنشاء الحمولات يدويًا:1. `getSumeragiRbcSessions()` المرايا `/v1/sumeragi/rbc/sessions`، و
   يقوم `findRbcSamplingCandidate()` بتحديد الجلسة الأولى التي تم تسليمها تلقائيًا باستخدام تجزئة الكتلة
   (تعود مجموعة التكامل إليها في أي وقت
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` غير محدد).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` يقوم بتسوية `{blockHash,height,view}`
   بالإضافة إلى تجاوزات `{count,seed,apiToken}` الاختيارية بحيث لا يتم أبدًا استخدام الأعداد السداسية أو الأعداد الصحيحة السالبة المشوهة
   الوصول إلى Torii.
3. يقوم `sampleRbcChunks()` بنشر الطلب إلى `/v1/sumeragi/rbc/sample`، مع إرجاع بروفات القطع
   ومسارات Merkle (`samples[].chunkHex`، `chunkRoot`، `payloadHash`) التي يجب أرشفتها باستخدام
   بقية أدلة التبني الخاصة بك.
4. يلتقط `getSumeragiRbcDelivered(height, view)` بيانات تعريف تسليم المجموعة حتى يتمكن المدققون من
   يمكن إعادة تشغيل الدليل من البداية إلى النهاية.

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
```

احتفظ بكلا الإجابتين تحت الجذر الاصطناعي الذي ترسله إلى الإدارة. تجاوز
الجلسة المحددة تلقائيًا عبر `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
عندما تحتاج إلى التحقق من كتلة معينة، وتعامل مع حالات الفشل في جلب لقطات RBC على أنها
خطأ في بوابة ما قبل الرحلة بدلاً من الرجوع بصمت إلى الوضع المباشر.

## الاختبار وCI

1. تخزين البضائع والتحف npm.
2. قم بتشغيل `npm run build:native`.
3. قم بتنفيذ `npm test` (أو `node --test` لمهام الدخان).

يوجد سير عمل GitHub Actions المرجعي
`docs/source/examples/iroha_js_ci.md`.

## الخطوات التالية

- مراجعة الأنواع التي تم إنشاؤها في `javascript/iroha_js/index.d.ts`.
- اكتشف الوصفات تحت `javascript/iroha_js/recipes/`.
- قم بإقران `ToriiClient` مع التشغيل السريع Norito لفحص الحمولات الصافية جنبًا إلى جنب
  مكالمات SDK.