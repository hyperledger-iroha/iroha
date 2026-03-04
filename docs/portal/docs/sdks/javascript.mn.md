---
lang: mn
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c677012c88133bce76df1704fb9c23a98f4843d891cf13f48b5cebbe2d898ce6
source_last_modified: "2026-01-30T18:06:01.644371+00:00"
translation_last_reviewed: 2026-02-07
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
slug: /sdks/javascript
translator: machine-google-reviewed
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

## Офлайн тэтгэмж ба шийдвэрийн мета өгөгдөл

Офлайн тэтгэмжийн хариултууд нь дэвтэрийн баяжуулсан мета өгөгдлүүдийг урьдаас ил болгодог —
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex`, `remaining_amount`-г түүхий эдтэй хамт буцаана
Бичлэг хийх тул хяналтын самбарууд суулгагдсан Norito ачааллын кодыг тайлах шаардлагагүй болно. Шинэ
тоолох туслахууд (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) дараагийн хугацаа нь дуусч байгааг онцол (шинэчлэх → бодлого)
→ гэрчилгээ) тул UI тэмдэг нь тэтгэмж авах бүрд операторуудад анхааруулах боломжтой
<24 цаг үлдсэн. SDK
`/v1/offline/allowances`-д илэрсэн REST шүүлтүүрүүдийг толин тусгал:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` болон
`requireVerdict` / `onlyMissingVerdict` логик. Буруу хослолууд (for
жишээ `onlyMissingVerdict` + `verdictIdHex`) нь Torii-ээс өмнө орон нутагт татгалзсан
гэж нэрлэдэг.

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

## Офлайн цэнэглэлт (асуудал + бүртгэл)

Сертификат олгохыг хүсвэл нэн даруй цэнэглэх туслахуудыг ашиглаарай
дэвтэрт бүртгүүлнэ үү. SDK нь олгосон болон бүртгэгдсэн гэрчилгээг баталгаажуулдаг
Буцахаасаа өмнө ID-ууд таарч, хариу нь ачааллыг хоёуланг нь агуулна. Байна
цэнэглэх зориулалтын эцсийн цэг байхгүй; туслагч асуудлыг гинжлэх + дуудлага бүртгэх. Хэрэв
Танд гарын үсэг зурсан гэрчилгээ байгаа бол `registerOfflineAllowance` (эсвэл
`renewOfflineAllowance`) шууд.

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
`getExplorerAccountQr()` хэрэгтэй үед IH58 (давуу)/sora (хоёр дахь шилдэг) литералууд дээр нэмэх нь шугам
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

const qr = await torii.getExplorerAccountQr("ih58...", {
  addressFormat: "compressed",
});
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

`addressFormat: "compressed"`-ийг дамжуулснаар Explorer-ийн өгөгдмөл шахагдсан хувилбарыг тусгана
сонгогчид; илүүд үздэг IH58 гаралт эсвэл `ih58_qr` хүсэлтийг дарж бичихийг орхих
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

## Дамжуулж буй ажиглагчид болон үйл явдлын курсорууд

`ToriiClient.streamEvents()` нь `/v1/events/sse`-г автоматаар асинхронгүй давталт болгон харуулж байна.
дахин оролдох тул Node/Bun CLI нь Rust CLI-тай адил дамжуулах хоолойн үйл ажиллагааг зогсоож чадна.
Runbook олдворуудын хажууд `Last-Event-ID` курсорыг байрлуулснаар операторууд боломжтой болно.
процессыг дахин эхлүүлэх үед үйл явдлыг алгасахгүйгээр урсгалыг үргэлжлүүлэх.

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

- `PIPELINE_STATUS` шилжүүлэгч (жишээ нь `Pending`, `Applied`, эсвэл `Approved`) эсвэл тохируулах
  `STREAM_FILTER_JSON` нь CLI-ийн хүлээн зөвшөөрсөн шүүлтүүрүүдийг дахин тоглуулах.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` нь давталтыг a хүртэл амьд байлгадаг
  дохио хүлээн авсан; Эхний хэдэн үйл явдал танд хэрэгтэй үед `STREAM_MAX_EVENTS=25`-г дамжуулаарай
  утааны шинжилгээнд зориулагдсан.
- `ToriiClient.streamSumeragiStatus()` нь ижил интерфэйсийг харуулдаг
  `/v1/sumeragi/status/sse` тиймээс зөвшилцлийн телеметрийг тусад нь авч үзэх боломжтой бөгөөд
  давталт нь `Last-Event-ID`-г мөн адил хүндэтгэдэг.
- Түлхүүр гардуулах CLI-г `javascript/iroha_js/recipes/streaming.mjs` харна уу (курсорын тогтвортой байдал
  env-var шүүлтүүрийг дарж, JS4-д ашигласан `extractPipelineStatusKind` бүртгэл)
  streaming/WebSocket замын зураглалыг хүргэх боломжтой.

## UAID портфолио ба Сансрын лавлах

Сансрын лавлах API-ууд нь Universal Account ID (UAID) амьдралын мөчлөгийг харуулдаг. The
туслахууд `uaid:<hex>` литерал эсвэл түүхий 64-hex дижест (LSB=1) болон
хүсэлт илгээхээсээ өмнө тэдгээрийг каноникчил:

- `getUaidPortfolio(uaid, { assetId })` нь өгөгдлийн орон зай тус бүрийн үлдэгдлийг нэгтгэдэг,
  хөрөнгийн эзэмшлийг каноник дансны дугаараар бүлэглэх; шүүлтүүрийг `assetId` дамжуулна
  багцын нэг хөрөнгийн жишээ хүртэл.
- `getUaidBindings(uaid, { addressFormat })` нь өгөгдлийн орон зай ↔ данс бүрийг тоолдог
  холбох (`addressFormat: "compressed"` `sora…` литералуудыг буцаана).
- `getUaidManifests(uaid, { dataspaceId })` чадварын манифест бүрийг буцаана.
  амьдралын мөчлөгийн төлөв, аудитын холбоотой дансууд.Операторын нотолгооны багц, манифест нийтлэх/цуцлах урсгал болон SDK шилжүүлгийн хувьд
зааварчилгаа, Бүх нийтийн дансны гарын авлагыг дагаарай (`docs/source/universal_accounts_guide.md`)
Эдгээр үйлчлүүлэгчийн туслахуудтай зэрэгцэн портал болон эх сурвалж бичиг баримтууд синхрончлогдсон хэвээр байна.

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

## RBC дээж авах, хүргэх нотолгоо

JS замын газрын зураг нь Roadrunner Block Commitment (RBC) түүврийг шаарддаг бөгөөд ингэснээр операторууд
Sumeragi-ээр дамжуулан тэдний татаж авсан блок нь тэдний баталгаажуулсан хэсэгчилсэн нотолгоотой таарч байгааг нотлох.
Ачаа ачааг гараар барихын оронд суурилуулсан туслахуудыг ашигла:

1. `getSumeragiRbcSessions()` толин тусгал `/v1/sumeragi/rbc/sessions`, мөн
   `findRbcSamplingCandidate()` блок хэштэй эхний хүргэсэн сессийг автоматаар сонгоно
   (Интеграцийн иж бүрдэл нь хэзээ ч түүнд буцаж ирдэг
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` тохируулаагүй байна).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` нь `{blockHash,height,view}`-г хэвийн болгодог
   Нэмж дурдахад `{count,seed,apiToken}` нь буруу хэлбэртэй зургаан өнцөгт эсвэл сөрөг бүхэл тоонуудыг хэзээ ч үгүйсгэхгүй
   Torii хүрэх.
3. `sampleRbcChunks()` хүсэлтийг `/v1/sumeragi/rbc/sample` руу илгээж, хэсэгчилсэн нотолгоог буцаана
   болон Merkle замуудыг (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) архивлах хэрэгтэй.
   Таны үрчлэлтийн нотлох баримтын үлдсэн хэсэг.
4. `getSumeragiRbcDelivered(height, view)` нь когортын хүргэх мета өгөгдлийг авдаг тул аудиторууд
   нотлох баримтыг төгсгөл хүртэл дахин тоглуулж болно.

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

Засаглалд өгсөн олдворын үндэс дор хоёр хариултыг хэвээр үлдээнэ үү. -г дарж бичнэ үү
`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`-ээр дамжуулан автоматаар сонгосон сесс
Хэрэв та тодорхой блокыг шалгах шаардлагатай бол RBC агшин зуурын агшин зуурын агшинг татаж авахын тулд бүтэлгүйтлийг авч үзэх хэрэгтэй.
Шууд горим руу чимээгүйхэн буулгахаас илүү нислэгийн өмнөх хаалганы алдаа.

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