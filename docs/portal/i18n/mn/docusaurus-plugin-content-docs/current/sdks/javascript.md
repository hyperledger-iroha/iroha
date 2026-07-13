---
slug: /sdks/javascript
lang: mn
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`@iroha/iroha-js` нь Torii-тэй харилцахад зориулагдсан каноник Node.js багц юм. Энэ
багцууд Norito барилгачид, Ed25519 туслахууд, хуудасны хэрэгслүүд болон уян хатан
HTTP/WebSocket клиент нь TypeScript-ээс CLI урсгалыг тусгах боломжтой.

## Суурилуулалт

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

Барилга угсралтын алхам нь `cargo build -p iroha_js_host`-ийг ороож байна. Хэрэгслийн хэлхээг баталгаажуулна уу
`npm run build:native`-г ажиллуулахаас өмнө `rust-toolchain.toml` нь дотоодод боломжтой.

## Гол удирдлага

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

## Гүйлгээ хийх

Norito заавар бүтээгчид танигч, мета өгөгдөл, тоо хэмжээг хэвийн болгодог.
кодлогдсон ажил гүйлгээ нь Rust/CLI ачаалалтай таарч байна.

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

## Torii үйлчлүүлэгчийн тохиргоо

`ToriiClient` нь `iroha_config`-г тусгадаг дахин оролдох/хугацаа дуусах товчлууруудыг хүлээн авдаг. Ашиглах
CamelCase тохиргооны объектыг нэгтгэхийн тулд `resolveToriiClientConfig` (хэвийн болгох)
`iroha_config` эхлээд), env хүчингүй болгох, шугамын сонголтууд.

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

Орон нутгийн хөгжүүлэгчийн орчны хувьсагч:

| Хувьсагч | Зорилго |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Хүсэлтийн завсарлага (миллисекунд). |
| `IROHA_TORII_MAX_RETRIES` | Хамгийн их дахин оролдох оролдлого. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Анхны дахин оролдлого буцаах. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Экспоненциал буцаалтын үржүүлэгч. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Дахин оролдох хамгийн их саатал. |
| `IROHA_TORII_RETRY_STATUSES` | Дахин оролдох таслалаар тусгаарлагдсан HTTP төлөвийн кодууд. |
| `IROHA_TORII_RETRY_METHODS` | Дахин оролдох таслалаар тусгаарлагдсан HTTP аргууд. |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` нэмдэг. |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` толгойг нэмнэ. |

Дахин оролдох профайлууд нь Андройдын өгөгдмөл тохиргоог тусгаж, паритет шалгах зорилгоор экспортлоно:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. `docs/source/sdk/js/torii_retry_policy.md`-г үзнэ үү
төгсгөлийн цэгээс профайлын зураглал болон параметрийн засаглалын аудитын үеэр
JS4/JS7.

## Давтагдах жагсаалт ба хуудаслалт

Хуудасны туслахууд нь `/v1/accounts`-д зориулсан Python SDK эргономикийг тусгадаг.
`/v1/domains`, `/v1/assets/definitions`, NFT, үлдэгдэл, хөрөнгө эзэмшигчид болон
дансны гүйлгээний түүх.

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
## Torii асуулга ба дамжуулалт (WebSockets)

Асуулгын туслахууд статус, Prometheus хэмжигдэхүүн, телеметрийн агшин зуурын зураг болон үйл явдлыг ил гаргадаг
Norito шүүлтүүрийн дүрмийг ашиглан урсгал. Дамжуулалт автоматаар шинэчлэгдэнэ
Дахин оролдох төсөв зөвшөөрвөл WebSockets болон үргэлжлүүлнэ.

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

Бусаддаа `streamBlocks`, `streamTransactions`, эсвэл `streamTelemetry` ашиглана уу.
WebSocket төгсгөлийн цэгүүд. Бүх урсгалын туслахууд дахин оролдох оролдлого хийдэг тул залгана уу
`onReconnect` хяналтын самбар болон дохиолол өгөхийн тулд буцааж залгана.

## Explorer агшин зуурын зураг ба QR ачаалал

Explorer телеметр нь `/v1/explorer/metrics` болон
`/v1/explorer/accounts/{account_id}/qr` төгсгөлийн цэгүүд нь хяналтын самбаруудыг дахин тоглуулах боломжтой
порталыг идэвхжүүлдэг ижил хормын хувилбарууд. `getExplorerMetrics()`-г хэвийн болгож байна
ачаалал ба чиглүүлэлт идэвхгүй болсон үед `null` буцаана. Үүнийг хослуул
`getExplorerAccountQr()` хэрэгтэй үед i105 (давуу)/sora (хоёр дахь шилдэг) литералууд дээр нэмэх нь шугам
Хуваалцах товчлууруудад зориулсан SVG.

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

`i105`-ийг дамжуулснаар Explorer-ийн өгөгдмөл шахагдсан хувилбарыг тусгана
сонгогчид; илүүд үздэг i105 гаралт эсвэл `i105_qr` хүсэлтийг дарж бичихийг орхих
танд QR-аюулгүй хувилбар хэрэгтэй үед. Шахсан үгийн утга нь хоёрдугаарт ордог
UX-д зориулсан цорын ганц Sora сонголт. Туслах нь үргэлж каноник танигчийг буцаадаг.
сонгосон үгийн утга, мета өгөгдөл (сүлжээний угтвар, QR хувилбар/модуль, алдаа
залруулгын түвшин ба доторлогооны SVG), тиймээс CI/CD нь ижил ачааллыг нийтлэх боломжтой.
Explorer нь захиалгат хувиргагчийг дуудахгүйгээр ажилладаг.

## Сеанс болон дараалалыг холбоно уу

Connect туслахууд `docs/source/connect_architecture_strawman.md` толин тусгал. The
Урьдчилан үзэхэд бэлэн сесс рүү хүрэх хамгийн хурдан зам бол `bootstrapConnectPreviewSession`,
Энэ нь тодорхойлогч SID/URI үүсгэх ба Torii-ийг хооронд нь холбодог.
бүртгэлийн дуудлага.

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

- QR/deeplink-д зөвхөн тодорхой URI хэрэгтэй үед `register: false`-г дамжуулаарай.
  урьдчилан үзэх.
- `generateConnectSid` нь сессийн ID-г авах шаардлагатай үед боломжтой хэвээр байна
  URI-г задлахгүйгээр.
- Чиглэлийн түлхүүрүүд болон шифр текстийн дугтуйнууд нь эх гүүрнээс ирдэг; хэзээ
  боломжгүй үед SDK буцаад JSON кодлогч руу унаж, шиддэг
  `ConnectQueueError.bridgeUnavailable`.
- Офлайн буферууд нь IndexedDB-д Norito `.to` blob хэлбэрээр хадгалагддаг. Хяналтын дараалал
  ялгаруулсан `ConnectQueueError.overflow(limit)`-ээр дамжуулан төлөв /
  `.expired(ttlMs)` алдаа ба тэжээлийн `connect.queue_depth` телеметрийг тодорхойлсоны дагуу
  замын зурагт.

### Бүртгэл болон бодлогын агшин зуурын зургийг холбоно уу

Платформын операторууд ямар ч шаардлагагүйгээр Connect бүртгэлийг судалж, шинэчлэх боломжтой
Node.js-г орхиж байна. Бүртгэлээр дамжуулан `iterateConnectApps()` хуудсууд, харин
`getConnectStatus()` болон `getConnectAppPolicy()` нь ажиллах цагийн тоолуур болон
одоогийн бодлогын дугтуй. `updateConnectAppPolicy()` camelCase талбаруудыг хүлээн авдаг,
Тиймээс та Torii-ийн хүлээж байгаа JSON ачааллыг үе шаттайгаар хийж болно.

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

Өргөдөл гаргахаасаа өмнө үргэлж хамгийн сүүлийн үеийн `getConnectStatus()` агшин зуурын зургийг аваарай.
мутаци-засаглалын хяналтын хуудас нь бодлогын шинэчлэлийг эхлүүлж байгааг нотлох баримт шаарддаг
флотын одоогийн хязгаараас.

### WebSocket залгалтыг холбоно уу

`ToriiClient.openConnectWebSocket()` каноникийг угсардаг
`/v1/connect/ws` URL (`sid`, `role` болон жетон параметрүүдийг оруулаад), шинэчлэлтүүд
`http→ws` / `https→wss`, эцсийн URL-г WebSocket-д өгнө.
хэрэгжилтийг хангах. Хөтөчүүд глобалыг автоматаар дахин ашигладаг
`WebSocket`. Node.js руу залгаагчид `ws` гэх мэт бүтээгчийг дамжуулах ёстой:

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

Хэрэв танд зөвхөн URL хэрэгтэй бол `torii.buildConnectWebSocketUrl(params)` эсвэл утсаар холбогдоно уу
дээд түвшний `buildConnectWebSocketUrl(baseUrl, params)` туслагч болон дахин ашиглах
өөрчлөн тээвэрлэх/дараалал үүсгэсэн мөр.

Бүрэн CLI-д чиглэсэн дээж хайж байна уу? The
[Урьдчилан үзэх жорыг холбох](./recipes/javascript-connect-preview.md) нь a
Ажиллуулах боломжтой скрипт ба телеметрийн зааварчилгааг тусгаж өгөх боломжтой
Холболтын дараалал + WebSocket урсгалыг баримтжуулах.

### Алсын зайн хэмжилт ба анхааруулга

Утасны дарааллын хэмжигдэхүүнийг туслах гадаргуу руу шууд оруулснаар хяналтын самбарыг толин тусгал болгох боломжтой
замын зураглалын KPI.

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

`ConnectQueueError#toConnectError()` нь дарааллын алдааг ерөнхий болгон хувиргадаг
`ConnectError` таксономи нь хуваалцсан HTTP/WebSocket interceptors нь
стандарт `connect.queue_depth`, `connect.queue_overflow_total`, ба
Замын зураг дээр дурдсан `connect.queue_expired_total` хэмжүүрүүд.

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

## UAID портфолио ба Сансрын лавлах

Сансрын лавлах API-ууд нь Universal Account ID (UAID) амьдралын мөчлөгийг харуулдаг. The
туслахууд `uaid:<hex>` литерал эсвэл түүхий 64-hex дижест (LSB=1) болон
хүсэлт илгээхээсээ өмнө тэдгээрийг каноникчил:

- `getUaidPortfolio(uaid, { assetId })` нь өгөгдлийн орон зай тус бүрийн үлдэгдлийг нэгтгэдэг,
  хөрөнгийн эзэмшлийг каноник дансны дугаараар бүлэглэх; шүүлтүүрийг `assetId` дамжуулна
  багцын нэг хөрөнгийн жишээ хүртэл.
- `getUaidBindings(uaid)` нь өгөгдлийн орон зай ↔ данс бүрийг тоолдог
  холбох (`i105` `i105` литералуудыг буцаана).
- `getUaidManifests(uaid, { dataspaceId })` чадварын манифест бүрийг буцаана.
  амьдралын мөчлөгийн төлөв, аудитын холбоотой дансууд.Операторын нотолгооны багц, манифест нийтлэх/цуцлах урсгал болон SDK шилжүүлгийн хувьд
зааварчилгаа, Бүх нийтийн дансны гарын авлагыг дагаарай (`docs/source/universal_accounts_guide.md`)
Эдгээр үйлчлүүлэгчийн туслахуудтай зэрэгцэн портал болон эх сурвалж бичиг баримтууд синхрончлогдсон хэвээр байна.

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

Операторууд мөн манифестуудыг эргүүлэх эсвэл яаралтай үгүйсгэх-хожих урсгалыг гүйцэтгэх боломжтой
CLI руу унаж байна. Туслагч хоёулаа нэмэлт `{ signal }` объектыг хүлээн зөвшөөрдөг
удаан үргэлжилсэн материалыг `AbortController`-ээр цуцалж болно; объект биш
сонголтууд эсвэл `AbortSignal` бус оролтууд нь синхрон `TypeError`-ийг эхлэхээс өмнө өсгөдөг.
хүсэлт Torii:

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
```

`publishSpaceDirectoryManifest()` нь түүхий манифест JSON-г (хамгийн
`fixtures/space_directory/`-ийн доор байрлах бэхэлгээ) эсвэл цуваатай аливаа объект
ижил бүтэц. `privateKey`, `privateKeyHex`, эсвэл `privateKeyMultihash` газрын зураг
`ExposedPrivateKey` талбар нь Torii хүлээж, `ed25519` гэж анхдагчаар тохируулна
угтвар байхгүй үед алгоритм. Хоёр хүсэлт хоёулаа Torii дараалалд орсны дараа буцаана
заавар (`202 Accepted`), энэ үед дэвтэр нь
тохирох `SpaceDirectoryEvent`.

## Засаглал ба ISO гүүр

`ToriiClient` нь гэрээ, үе шатыг шалгахад зориулсан засаглалын API-г илчилдэг.
санал, саналын хуудас өгөх (энгийн эсвэл ЗК), зөвлөлийг эргүүлэх, дуудах
`governanceFinalizeReferendumTyped` /
Гараар бичсэн DTO байхгүй `governanceEnactProposalTyped`. ISO 20022 туслахууд
ижил загварыг `buildPacs008Message`/`buildPacs009Message` болон
`submitIso*`/`waitForIsoMessageStatus` гурвал.

[Засаглал ба ISO гүүрний жор](./recipes/javascript-governance-iso.md)-г үзнэ үү.
CLI-д бэлэн дээж болон заагчийг бүрэн талбарын гарын авлага руу буцаана уу
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

## Туршилт & CI

1. Cargo болон npm олдворуудыг кэш.
2. `npm run build:native` ажиллуулна уу.
3. `npm test` (эсвэл утааны ажлын хувьд `node --test`) -ийг гүйцэтгэнэ.

GitHub Үйлдлүүдийн ажлын урсгалын лавлагаа нь амьдардаг
`docs/source/examples/iroha_js_ci.md`.

## Дараагийн алхамууд

- `javascript/iroha_js/index.d.ts` дээр үүсгэсэн төрлүүдийг шалгана уу.
- `javascript/iroha_js/recipes/` доорх жорыг судлаарай.
- `ToriiClient`-ийг Norito хурдан эхлүүлэх төхөөрөмжтэй хослуулан ачааллыг шалгах
  SDK дуудлага.
