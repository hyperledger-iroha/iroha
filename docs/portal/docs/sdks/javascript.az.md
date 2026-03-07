---
lang: az
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

`@iroha/iroha-js` Torii ilə qarşılıqlı əlaqə üçün kanonik Node.js paketidir. Bu
paketləri Norito qurucuları, Ed25519 köməkçiləri, səhifələşdirmə utilitləri və elastik
HTTP/WebSocket müştərisi beləliklə TypeScript-dən CLI axınlarını əks etdirə biləsiniz.

## Quraşdırma

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

Quraşdırma addımı `cargo build -p iroha_js_host`-i əhatə edir. Alət zəncirindən əmin olun
`rust-toolchain.toml` `npm run build:native`-i işə salmadan əvvəl yerli olaraq mövcuddur.

## Açar idarəetmə

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

## Əməliyyatlar qurun

Norito təlimat qurucuları identifikatorları, metadataları və kəmiyyətləri belə normallaşdırır
kodlaşdırılmış əməliyyatlar Rust/CLI yüklərinə uyğun gəlir.

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

## Torii müştəri konfiqurasiyası

`ToriiClient`, `iroha_config`-i əks etdirən təkrar cəhd/taymout düymələrini qəbul edir. istifadə edin
CamelCase konfiqurasiya obyektini birləşdirmək üçün `resolveToriiClientConfig` (normallaşdırın
`iroha_config`), env ləğv edir və daxili seçimlər.

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

Yerli inkişaf üçün mühit dəyişənləri:

| Dəyişən | Məqsəd |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Tələb müddəti (millisaniyə). |
| `IROHA_TORII_MAX_RETRIES` | Maksimum təkrar cəhdlər. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | İlkin təkrar cəhd geri çəkilmə. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Eksponensial geriləmə çarpanı. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Maksimum təkrar cəhd gecikməsi. |
| `IROHA_TORII_RETRY_STATUSES` | Yenidən cəhd etmək üçün vergüllə ayrılmış HTTP status kodları. |
| `IROHA_TORII_RETRY_METHODS` | Yenidən cəhd etmək üçün vergüllə ayrılmış HTTP üsulları. |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` əlavə edir. |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` başlığını əlavə edir. |

Yenidən cəhd profilləri Android defoltlarını əks etdirir və paritet yoxlamaları üçün ixrac edilir:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. Baxın `docs/source/sdk/js/torii_retry_policy.md`
son nöqtədən profilə xəritəçəkmə və parametrlərin idarəetmə auditləri zamanı
JS4/JS7.

## Təkrarlanan siyahılar və səhifələmə

Səhifələmə köməkçiləri `/v1/accounts` üçün Python SDK erqonomikasını əks etdirir,
`/v1/domains`, `/v1/assets/definitions`, NFT-lər, balanslar, aktiv sahibləri və
hesab əməliyyatı tarixi.

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

## Oflayn müavinətlər və hökm metadatası

Oflayn müavinət cavabları zənginləşdirilmiş kitab metadatasını qabaqcadan ifşa edir -
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` və `remaining_amount` xammal ilə birlikdə qaytarılır
qeyd edin ki, tablosuna daxil edilmiş Norito faydalı yükləri deşifrə etməli olmasın. Yeni
geri sayım köməkçiləri (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) növbəti bitən son tarixi vurğulayın (təzələyin → siyasət
→ sertifikat) beləliklə UI nişanları müavinət olduqda operatorları xəbərdar edə bilər
<24 saat qalıb. SDK
`/v1/offline/allowances` tərəfindən ifşa edilən REST filtrlərini əks etdirir:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` və
`requireVerdict` / `onlyMissingVerdict` booleanları. Yanlış birləşmələr (üçün
misal `onlyMissingVerdict` + `verdictIdHex`) Torii-dən əvvəl yerli olaraq rədd edilir
adlanır.

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

## Oflayn əlavələr (məsələ + qeydiyyat)

Sertifikat vermək istədiyiniz zaman və dərhal doldurma köməkçilərindən istifadə edin
kitabçasında qeyd edin. SDK verilmiş və qeydə alınmış sertifikatı yoxlayır
ID-lər geri qayıtmazdan əvvəl uyğun gəlir və cavab hər iki faydalı yükü ehtiva edir. var
xüsusi əlavə son nöqtə yoxdur; köməkçi problemi zəncirləyir + zəngləri qeyd edir. Əgər
artıq imzalanmış sertifikatınız var, `registerOfflineAllowance` (və ya
`renewOfflineAllowance`) birbaşa.

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

## Torii sorğular və axın (WebSockets)

Sorğu köməkçiləri statusu, Prometheus ölçülərini, telemetriya görüntülərini və hadisəni ifşa edir
Norito filtr qrammatikasından istifadə edərək axınlar. Axın avtomatik olaraq yüksəlir
Yenidən cəhd büdcəsi icazə verdikdə WebSockets və davam edir.

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

Digəri üçün `streamBlocks`, `streamTransactions` və ya `streamTelemetry` istifadə edin
WebSocket son nöqtələri. Bütün axın köməkçiləri təkrar cəhdləri üzə çıxarır, ona görə də onu bağlayın
`onReconnect` tablosunu qidalandırmaq və xəbərdarlıq etmək üçün geri çağırış.

## Explorer anlıq görüntüləri və QR yükləri

Explorer telemetriyası `/v1/explorer/metrics` və üçün tipli köməkçilər təqdim edir
`/v1/explorer/accounts/{account_id}/qr` son nöqtələr, beləliklə, tablosuna təkrar oxuya bilərsiniz
portalı gücləndirən eyni görüntülər. `getExplorerMetrics()` normallaşdırır
faydalı yük və marşrut qeyri-aktiv olduqda `null` qaytarır. ilə cütləşdirin
`getExplorerAccountQr()` sizə lazım olduqda IH58 (üstünlük verilir)/sora (ikinci ən yaxşı) literal üstəgəl inline
Paylaşım düymələri üçün SVG.

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

`addressFormat: "compressed"`-dən keçmək Explorer-in defolt sıxılmışını əks etdirir
seçicilər; üstünlük verilən IH58 çıxışı üçün ləğvi buraxın və ya sorğu `ih58_qr`
QR təhlükəsiz variantına ehtiyacınız olduqda. Sıxılmış hərf ikinci ən yaxşısıdır
UX üçün yalnız Sora variantı. Köməkçi həmişə kanonik identifikatoru qaytarır,
seçilmiş literal və metadata (şəbəkə prefiksi, QR versiyası/modulları, xəta
korreksiya səviyyəsi və daxili SVG), beləliklə CI/CD eyni faydalı yükləri dərc edə bilər
Explorer sifarişli çeviriciləri çağırmadan işləyir.

## Sessiyaları və növbələri birləşdirin

Connect köməkçiləri `docs/source/connect_architecture_strawman.md` aynasıdır. The
önizləmə üçün hazır sessiyanın ən sürətli yolu `bootstrapConnectPreviewSession`,
deterministik SID/URI nəslini və Torii-i birləşdirən
qeydiyyat zəngi.

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

- QR/deeplink üçün yalnız deterministik URI-lərə ehtiyacınız olduqda `register: false`-i keçin
  önizləmələr.
- `generateConnectSid` sessiya identifikatorlarını əldə etmək lazım olduqda əlçatan qalır
  URI-ləri vurmadan.
- İstiqamət açarları və şifrəli mətn zərfləri yerli körpüdən gəlir; nə vaxt
  əlçatmaz SDK JSON kodekinə qayıdır və atır
  `ConnectQueueError.bridgeUnavailable`.
- Oflayn buferlər IndexedDB-də Norito `.to` blobları kimi saxlanılır. Monitor növbəsi
  emissiya edilmiş `ConnectQueueError.overflow(limit)` vasitəsilə dövlət /
  `.expired(ttlMs)` xətaları və qeyd edildiyi kimi `connect.queue_depth` telemetriyası
  yol xəritəsində.

### Qeyd dəftərini və siyasət snapshotlarını birləşdirin

Platforma operatorları olmadan da Connect reyestrini introspektsiya edə və yeniləyə bilər
Node.js-dən ayrılır. `iterateConnectApps()` səhifələri reyestrdən keçərkən
`getConnectStatus()` və `getConnectAppPolicy()` iş vaxtı sayğaclarını ifşa edir və
cari siyasət zərfi. `updateConnectAppPolicy()` camelCase sahələrini qəbul edir,
beləliklə siz Torii-in gözlədiyi JSON yükünü hazırlaya bilərsiniz.

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

Tətbiq etməzdən əvvəl həmişə ən son `getConnectStatus()` şəklini çəkin
mutasiyalar—idarəetmə yoxlama siyahısı siyasət yeniləmələrinin başladığına dair sübut tələb edir
donanmanın cari məhdudiyyətlərindən.

### WebSocket yığımını birləşdirin

`ToriiClient.openConnectWebSocket()` kanonikləri yığır
`/v1/connect/ws` URL (`sid`, `role` və nişan parametrləri daxil olmaqla), təkmilləşdirmələr
`http→ws` / `https→wss` və yekun URL-ni hansı WebSocket-ə verir
təmin etdiyiniz həyata keçirilməsi. Brauzerlər avtomatik olaraq qlobaldan yenidən istifadə edir
`WebSocket`. Node.js zəng edənlər `ws` kimi konstruktordan keçməlidir:

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

Yalnız URL-ə ehtiyacınız olduqda, `torii.buildConnectWebSocketUrl(params)` və ya
yüksək səviyyəli `buildConnectWebSocketUrl(baseUrl, params)` köməkçisi və yenidən istifadə edin
xüsusi nəqliyyat/növbə ilə nəticələnən sətir.

Tam CLI yönümlü nümunə axtarırsınız? The
[Önizləmə reseptini birləşdirin](./recipes/javascript-connect-preview.md) a daxildir
icra edilə bilən skript və çatdırıla bilən yol xəritəsini əks etdirən telemetriya təlimatı
Qoşulma növbəsi + WebSocket axınının sənədləşdirilməsi.

### Növbədə telemetriya və xəbərdarlıq

Növbə ölçülərini birbaşa köməkçi səthlərə bağlayın ki, tablosuna yansısın
yol xəritəsi KPI.

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

`ConnectQueueError#toConnectError()` növbə xətalarını ümumiyə çevirir
`ConnectError` taksonomiyası belə paylaşılan HTTP/WebSocket kəsiciləri
standart `connect.queue_depth`, `connect.queue_overflow_total` və
`connect.queue_expired_total` ölçüləri yol xəritəsində istinad edilir.

## Axın izləyiciləri və hadisə kursorları

`ToriiClient.streamEvents()` `/v1/events/sse` avtomatik asinxron iterator kimi təqdim edir
təkrar cəhd edin, beləliklə, Node/Bun CLI-lər boru kəməri fəaliyyətini Rust CLI ilə eyni şəkildə dayandıra bilər.
`Last-Event-ID` kursorunu runbook artefaktlarınızla birlikdə saxlayın ki, operatorlar
proses yenidən başladıqda hadisələri atlamadan axını davam etdirin.

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

- `PIPELINE_STATUS` açarı (məsələn, `Pending`, `Applied` və ya `Approved`) və ya təyin edin
  CLI-nin qəbul etdiyi eyni filtrləri təkrar oxutmaq üçün `STREAM_FILTER_JSON`.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` iteratoru a qədər canlı saxlayır
  siqnal qəbul edilir; yalnız ilk bir neçə hadisəyə ehtiyacınız olduqda `STREAM_MAX_EVENTS=25`-i keçin
  tüstü testi üçün.
- `ToriiClient.streamSumeragiStatus()` eyni interfeysi əks etdirir
  `/v1/sumeragi/status/sse` belə ki, konsensus telemetriyası ayrı-ayrılıqda aparıla bilər və
  iterator `Last-Event-ID`-i eyni şəkildə qiymətləndirir.
- Açar təslim CLI üçün baxın `javascript/iroha_js/recipes/streaming.mjs` (kursorun davamlılığı,
  env-var filtrini ləğv edir və JS4-də istifadə olunan `extractPipelineStatusKind` girişi)
  axın/WebSocket yol xəritəsi çatdırıla bilər.

## UAID portfeli və Kosmik kataloqu

Space Directory API-ləri Universal Account ID (UAID) həyat dövrünü əhatə edir. The
köməkçilər `uaid:<hex>` literallarını və ya xam 64 hex həzmləri (LSB=1) qəbul edir və
sorğu göndərməzdən əvvəl onları kanonikləşdirin:

- `getUaidPortfolio(uaid, { assetId })` məlumat məkanı üçün balansları toplayır,
  aktivlərin kanonik hesab identifikatorlarına görə qruplaşdırılması; filtrləmək üçün `assetId` keçir
  portfelin tək aktiv nümunəsinə qədər.
- `getUaidBindings(uaid, { addressFormat })` hər məlumat məkanını ↔ hesabını sadalayır
  bağlama (`addressFormat: "compressed"` `sora…` literallarını qaytarır).
- `getUaidManifests(uaid, { dataspaceId })` hər bir qabiliyyət manifestini qaytarır,
  həyat dövrü statusu və audit üçün bağlı hesablar.Operator sübut paketləri, manifest dərc/ləğv axınları və SDK miqrasiyası üçün
təlimat, Universal Hesab Bələdçisinə əməl edin (`docs/source/universal_accounts_guide.md`)
bu müştəri köməkçiləri ilə yanaşı, portal və mənbə sənədləri sinxron olaraq qalır.

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

Operatorlar həmçinin manifestləri döndərə və ya təcili inkar etmə-qazanma axınları olmadan icra edə bilərlər
CLI-yə düşür. Hər iki köməkçi belə isteğe bağlı `{ signal }` obyektini qəbul edir
uzun müddət davam edən təqdimatlar `AbortController` ilə ləğv edilə bilər; qeyri-obyekt
opsionlar və ya qeyri-`AbortSignal` girişləri sinxron `TypeError`-i qaldırmadan əvvəl
sorğu hitləri Torii:

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

`publishSpaceDirectoryManifest()` ya xam manifest JSON-u qəbul edir (uyğundur
`fixtures/space_directory/` altındakı qurğular) və ya seriala çevrilən hər hansı obyekt
eyni struktur. `privateKey`, `privateKeyHex` və ya `privateKeyMultihash` xəritəsi
`ExposedPrivateKey` sahəsi Torii gözləyir və defolt olaraq `ed25519`
prefiks verilmədikdə alqoritm. Hər iki sorğu Torii növbəyə daxil olduqdan sonra geri qayıdır
təlimat (`202 Accepted`), bu zaman kitab kitabçası
uyğun `SpaceDirectoryEvent`.

## İdarəetmə və ISO körpüsü

`ToriiClient` müqavilələrin yoxlanılması, mərhələlərin hazırlanması üçün idarəetmə API-lərini ifşa edir
təkliflər, bülletenlərin təqdim edilməsi (düz və ya ZK), şuranın dəyişdirilməsi və çağırılması
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` əl ilə yazılmış DTO-lar olmadan. ISO 20022 köməkçiləri
`buildPacs008Message`/`buildPacs009Message` vasitəsilə eyni nümunəyə əməl edin və
`submitIso*`/`waitForIsoMessageStatus` üçlüyü.

[İdarəetmə və ISO körpüsü reseptinə](./recipes/javascript-governance-iso.md) baxın
CLI-yə hazır nümunələr üçün və tam sahə təlimatına geri dönən göstəricilər
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC nümunəsi və çatdırılma sübutu

JS yol xəritəsi, həmçinin, operatorların məlumat əldə edə bilməsi üçün Roadrunner Block Commitment (RBC) nümunəsini tələb edir.
Sumeragi vasitəsilə əldə etdikləri blokun təsdiqlədikləri yığın sübutlarına uyğun olduğunu sübut edin.
Yükləri əl ilə qurmaq əvəzinə daxili köməkçilərdən istifadə edin:

1. `getSumeragiRbcSessions()` güzgülər `/v1/sumeragi/rbc/sessions` və
   `findRbcSamplingCandidate()` blok hash ilə ilk çatdırılan sessiyanı avtomatik seçir
   (inteqrasiya dəsti istənilən vaxt ona qayıdır
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` qurulmayıb).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` `{blockHash,height,view}` normallaşdırır
   üstəgəl isteğe bağlı `{count,seed,apiToken}` səhv formalaşdırılmış hex və ya mənfi tam ədədləri heç vaxt ləğv edir
   Torii-ə çatın.
3. `sampleRbcChunks()` sorğunu `/v1/sumeragi/rbc/sample` ünvanına göndərir, yığın sübutlarını qaytarır
   və Merkle yolları (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) ilə arxivləşdirməlisiniz
   övladlığa götürmə sübutunuzun qalan hissəsi.
4. `getSumeragiRbcDelivered(height, view)` auditorlar üçün kohortun çatdırılma metadatasını çəkir
   sübutu başdan sona təkrarlaya bilər.

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

Rəhbərliyə təqdim etdiyiniz artefakt kökü altında hər iki cavabı davam etdirin. -ı ləğv edin
`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` vasitəsilə avtomatik seçilmiş sessiya
müəyyən bir bloku araşdırmalı olduğunuz zaman və RBC anlıq görüntülərini əldə etmək üçün uğursuzluqları müalicə edin
səssizcə birbaşa rejimə endirməkdənsə, uçuşdan əvvəl keçid xətası.

## Test və CI

1. Keş yükü və npm artefaktları.
2. `npm run build:native`-i işə salın.
3. `npm test` (və ya tüstü işləri üçün `node --test`) yerinə yetirin.

İstinad GitHub Actions iş axını yaşayır
`docs/source/examples/iroha_js_ci.md`.

## Növbəti addımlar

- `javascript/iroha_js/index.d.ts`-də yaradılan növləri nəzərdən keçirin.
- `javascript/iroha_js/recipes/` altında reseptləri araşdırın.
- `ToriiClient` ilə birlikdə faydalı yükləri yoxlamaq üçün Norito sürətli başlanğıc ilə cütləşdirin
  SDK zəngləri.