---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/javascript.ar.md
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
  destinationAccountId: "<i105-account-id>",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "<i105-account-id>",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "<i105-account-id>", quantity: "5" }],
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
const balances = await torii.listAccountAssets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Offline readiness

JavaScript integrations should use `GET /v1/offline/readiness?asset_definition_id=xor%23wonderland` for offline feature discovery.
Kagemusha readiness fields advertise the active offline payment implementation.

```ts
const readiness = await torii.getOfflineReadiness("xor#wonderland");
console.log("offline ready", readiness.ready, readiness.blockers);
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

const qr = await torii.getExplorerAccountQr("<i105-account-id>");
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

## Live event streams

`ToriiClient.streamEvents()` exposes `/v1/events/sse` as a live-only async
iterator. Torii retains no replay log for this route, so the helper has no
`lastEventId` option and reconnecting can leave a gap. A terminal
`event: stream_error` is yielded before the iterator ends; handle it explicitly
instead of treating closure as a lossless continuation point.

```js
import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080");
const controller = new AbortController();

process.once("SIGINT", () => controller.abort());
process.once("SIGTERM", () => controller.abort());

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Transaction: { status: "Committed" } } },
  signal: controller.signal,
})) {
  if (event.event === "stream_error") {
    console.error("terminal stream error", event.data);
    break;
  }
  const status = event.data ? extractPipelineStatusKind(event.data) : null;
  console.log(`[${event.event}] status=${status ?? "n/a"}`);
}
```

- Switch `PIPELINE_STATUS` (for example `Pending`, `Applied`, or `Approved`) or set
  `STREAM_FILTER_JSON` to use the same filters the CLI accepts.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` keeps the iterator alive until a
  signal is received; pass `STREAM_MAX_EVENTS=25` when you only need the first few events
  for a smoke test.
- `ToriiClient.streamSumeragiStatus()` exposes the separate
  `/v1/sumeragi/status/sse` consensus telemetry feed.
- See `javascript/iroha_js/recipes/streaming.mjs` for a live-only turnkey CLI with
  environment-driven filters and explicit terminal-error handling.

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
    authority: "<i105-account-id>",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "<i105-account-id>",
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

## Sumeragi availability telemetry

Reliable broadcast remains an internal Sumeragi v2 transport and recovery mechanism.
The public Torii catalog exposes aggregate diagnostics through
`GET /v1/sumeragi/telemetry`; it does not publish per-session RBC state, chunk
samples, delivery probes, or a deterministic collector plan.

```js
const telemetry = await torii.getSumeragiTelemetryTyped();
console.log(`collector votes=${telemetry.availability.total_votes_ingested}`);
console.log(`pending sessions=${telemetry.rbc_backlog.pending_sessions}`);
```

Archive `availability.collectors`, `rbc_backlog`, and `rbc_pending` from the raw
telemetry response together with Prometheus counters and consensus logs. These
fields are aggregate operational evidence and must not be treated as light-client
chunk proofs or transaction-finality evidence.

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
