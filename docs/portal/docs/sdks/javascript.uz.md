---
lang: uz
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

`@iroha/iroha-js` - Torii bilan o'zaro ishlash uchun kanonik Node.js to'plami. Bu
to'plamlar Norito quruvchilar, Ed25519 yordamchilari, sahifalash yordam dasturlari va moslashuvchan
HTTP/WebSocket mijozi, shunda siz TypeScript-dan CLI oqimlarini aks ettirishingiz mumkin.

## O'rnatish

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

Qurilish bosqichi `cargo build -p iroha_js_host` ni o'rab oladi. Asboblar zanjiridan ishonch hosil qiling
`rust-toolchain.toml` `npm run build:native` ishga tushirishdan oldin mahalliy sifatida mavjud.

## Kalit boshqaruvi

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

## Tranzaktsiyalarni yaratish

Norito yo'riqnomasini yaratuvchilari identifikatorlar, metama'lumotlar va miqdorlarni normallashtiradi
kodlangan tranzaktsiyalar Rust/CLI foydali yuklariga mos keladi.

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

## Torii mijoz konfiguratsiyasi

`ToriiClient` `iroha_config`ni aks ettiruvchi qayta urinish/vaqt tugashi tugmalarini qabul qiladi. Foydalanish
CamelCase konfiguratsiya ob'ektini birlashtirish uchun `resolveToriiClientConfig` (normallashtirish
`iroha_config` birinchi), env bekor qilish va inline parametrlari.

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

Mahalliy ishlab chiqaruvchi uchun atrof-muhit o'zgaruvchilari:

| O'zgaruvchi | Maqsad |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | So'rovni kutish vaqti tugashi (millisekundlar). |
| `IROHA_TORII_MAX_RETRIES` | Maksimal qayta urinishlar. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Dastlabki qayta urinish. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Eksponensial orqaga qaytish ko'paytmasi. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Qayta urinishning maksimal kechikishi. |
| `IROHA_TORII_RETRY_STATUSES` | Qayta urinish uchun vergul bilan ajratilgan HTTP holat kodlari. |
| `IROHA_TORII_RETRY_METHODS` | Qayta urinish uchun vergul bilan ajratilgan HTTP usullari. |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` qo'shadi. |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` sarlavhasini qo'shadi. |

Qayta urinish profillari Android standartlarini aks ettiradi va paritet tekshiruvlari uchun eksport qilinadi:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. `docs/source/sdk/js/torii_retry_policy.md` ga qarang
so'nggi nuqtadan profilga xaritalash va parametrlarni boshqarish tekshiruvlari uchun
JS4/JS7.

## Takrorlanadigan roʻyxatlar va sahifalash

Pagination yordamchilari `/v1/accounts` uchun Python SDK ergonomikasini aks ettiradi,
`/v1/domains`, `/v1/assets/definitions`, NFTlar, balanslar, aktiv egalari va
hisob operatsiyalari tarixi.

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

## Oflayn imtiyozlar va hukm metamaʼlumotlari

Oflayn to'lov javoblari boyitilgan daftar metama'lumotlarini oldindan ochib beradi -
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` va `remaining_amount` xom ashyo bilan birga qaytariladi
yozib oling, shuning uchun asboblar paneli o'rnatilgan Norito foydali yuklarini dekodlashi shart emas. Yangi
orqaga hisoblash yordamchilari (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) keyingi tugash muddatini belgilang (yangilash → siyosat
→ sertifikat) shuning uchun foydalanuvchi interfeysi nishonlari operatorlarni har doim ruxsat berilganda ogohlantirishi mumkin
<24 soat qoldi. SDK
`/v1/offline/reserve/topup` tomonidan ta'sirlangan REST filtrlarini aks ettiradi:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` va
`requireVerdict` / `onlyMissingVerdict` mantiqiy. Yaroqsiz kombinatsiyalar (uchun
misol `onlyMissingVerdict` + `verdictIdHex`) Torii dan oldin mahalliy ravishda rad etiladi
deyiladi.

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

## Oflayn to'ldirish (muammo + ro'yxatdan o'tish)

Sertifikat bermoqchi bo'lganingizda va darhol to'ldirish yordamchilaridan foydalaning
uni daftarda ro'yxatdan o'tkazing. SDK berilgan va ro'yxatdan o'tgan sertifikatni tekshiradi
Qaytishdan oldin identifikatorlar mos keladi va javob ikkala foydali yukni ham o'z ichiga oladi. bor
maxsus to'ldirish so'nggi nuqtasi yo'q; yordamchi muammoni zanjirlaydi + qo'ng'iroqlarni ro'yxatdan o'tkazish. Agar
Sizda allaqachon imzolangan sertifikat bor, `registerOfflineAllowance` (yoki
`renewOfflineAllowance`) to'g'ridan-to'g'ri.

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

## Torii so'rovlar va oqim (WebSockets)

So‘rov yordamchilari holat, Prometheus ko‘rsatkichlari, telemetriya suratlari va voqeani ko‘rsatadi.
Norito filtri grammatikasidan foydalangan holda oqimlar. Oqim avtomatik ravishda yangilanadi
WebSockets va qayta urinish byudjeti ruxsat berganda davom etadi.

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

Boshqasi uchun `streamBlocks`, `streamTransactions` yoki `streamTelemetry` dan foydalaning
WebSocket so'nggi nuqtalari. Barcha striming yordamchilari qayta urinib ko‘radi, shuning uchun uni ulang
`onReconnect` boshqaruv paneli va ogohlantirish uchun qayta qo'ng'iroq.

## Explorer suratlari va QR yuklamalari

Explorer telemetriyasi `/v1/explorer/metrics` va uchun yozilgan yordamchilarni taqdim etadi
`/v1/explorer/accounts/{account_id}/qr` so'nggi nuqtalari, shuning uchun asboblar paneli qayta o'ynashi mumkin
portalni quvvatlaydigan bir xil suratlar. `getExplorerMetrics()` normallashtiradi
foydali yuk va marshrut o'chirilganida `null` ni qaytaradi. U bilan bog'lang
`getExplorerAccountQr()` i105 (afzal)/sora (ikkinchi-eng yaxshi) literallar va inline kerak boʻlganda
Ulashish tugmalari uchun SVG.

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

`i105` dan o'tish Explorerning standart siqilganini aks ettiradi
selektorlar; afzal qilingan i105 chiqishi yoki `i105_qr` so'rovi uchun bekor qilishni o'tkazib yubormang
QR-xavfsiz variant kerak bo'lganda. Siqilgan literal ikkinchi eng yaxshisidir
UX uchun faqat Sora varianti. Yordamchi har doim kanonik identifikatorni qaytaradi,
tanlangan literal va metadata (tarmoq prefiksi, QR versiyasi/modullar, xato
tuzatish darajasi va inline SVG), shuning uchun CI/CD bir xil foydali yuklarni nashr qilishi mumkin
Explorer maxsus konvertorlarni chaqirmasdan ishlaydi.

## Seanslar va navbatlarni ulash

Connect yordamchilari `docs/source/connect_architecture_strawman.md` aks etadi. The
Oldindan ko'rishga tayyor sessiyaning eng tezkor yo'li - `bootstrapConnectPreviewSession`,
Bu deterministik SID/URI avlodi va Torii ni birlashtiradi
ro'yxatga olish qo'ng'irog'i.

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

- QR/deeplink uchun faqat deterministik URI kerak bo'lganda `register: false` dan o'ting
  oldindan ko'rishlar.
- `generateConnectSid` seans identifikatorlarini olishingiz kerak bo'lganda mavjud bo'lib qoladi
  URI zarb qilmasdan.
- Yo'nalish kalitlari va shifrlangan matn konvertlari mahalliy ko'prikdan keladi; qachon
  mavjud bo'lmasa, SDK JSON kodekiga qaytadi va uradi
  `ConnectQueueError.bridgeUnavailable`.
- Oflayn buferlar IndexedDB da Norito `.to` bloblari sifatida saqlanadi. Monitor navbati
  chiqarilgan `ConnectQueueError.overflow(limit)` orqali davlat /
  `.expired(ttlMs)` xatolari va `connect.queue_depth` telemetriyasini ta'riflanganidek ta'minlash
  yo'l xaritasida.

### Roʻyxatga olish kitobi va siyosat suratlarini ulang

Platforma operatorlari Connect registrini introspektsiya qilishlari va yangilashlari mumkin
Node.js. Ro'yxatga olish kitobi orqali `iterateConnectApps()` sahifalar, esa
`getConnectStatus()` va `getConnectAppPolicy()` ish vaqti hisoblagichlarini ochadi va
joriy siyosat konverti. `updateConnectAppPolicy()` camelCase maydonlarini qabul qiladi,
Shunday qilib, siz Torii kutgan bir xil JSON foydali yukini yaratishingiz mumkin.

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

Qo'llashdan oldin har doim eng so'nggi `getConnectStatus()` suratini oling
mutatsiyalar - boshqaruv nazorati ro'yxati siyosat yangilanishi boshlanishini tasdiqlovchi dalillarni talab qiladi
flotning joriy chegaralaridan.

### WebSocket terishni ulang

`ToriiClient.openConnectWebSocket()` kanonikni yig'adi
`/v1/connect/ws` URL (jumladan, `sid`, `role` va token parametrlari), yangilanishlar
`http→ws` / `https→wss` va yakuniy URLni qaysi WebSocket-ga topshiradi
amalga oshirish siz taqdim etadi. Brauzerlar avtomatik ravishda globaldan foydalanadi
`WebSocket`. Node.js qo'ng'iroq qiluvchilar `ws` kabi konstruktordan o'tishi kerak:

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

Agar sizga faqat URL kerak bo'lsa, `torii.buildConnectWebSocketUrl(params)` yoki
yuqori darajadagi `buildConnectWebSocketUrl(baseUrl, params)` yordamchisi va uni qayta ishlating
natijada maxsus transport/navbatdagi satr.

To'liq CLI-yo'naltirilgan namunani qidiryapsizmi? The
[Ulanishni oldindan koʻrish retsepti](./recipes/javascript-connect-preview.md) quyidagilarni oʻz ichiga oladi: a
ishga tushirish mumkin bo'lgan skript va telemetriya bo'yicha ko'rsatmalar, bu etkazib beriladigan yo'l xaritasini aks ettiradi
Ulanish navbatini + WebSocket oqimini hujjatlashtirish.

### Navbatda telemetriya va ogohlantirish

Navbat ko‘rsatkichlarini to‘g‘ridan-to‘g‘ri yordamchi yuzalarga ulang, shunda asboblar paneli aks ettira oladi
KPI yo'l xaritasi.

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

`ConnectQueueError#toConnectError()` navbatdagi nosozliklarni umumiy holatga aylantiradi
`ConnectError` taksonomiyasi, shuning uchun umumiy HTTP/WebSocket tutqichlari
standart `connect.queue_depth`, `connect.queue_overflow_total` va
`connect.queue_expired_total` ko'rsatkichlari yo'l xaritasida havola qilingan.

## Oqimli kuzatuvchilar va hodisalar kursorlari

`ToriiClient.streamEvents()` `/v1/events/sse` avtomatik ravishda asinxron iterator sifatida taqdim etadi
qayta urinib ko'ring, shuning uchun Node/Bun CLI'lar xuddi Rust CLI kabi quvur liniyasi faoliyatini davom ettirishi mumkin.
Operatorlar imkon qadar `Last-Event-ID` kursorini runbook artefaktlari bilan birga saqlang.
jarayon qayta boshlanganda voqealarni o'tkazib yubormasdan oqimni davom ettiring.

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

- `PIPELINE_STATUS` kaliti (masalan, `Pending`, `Applied` yoki `Approved`) yoki o'rnating
  CLI qabul qilgan filtrlarni takrorlash uchun `STREAM_FILTER_JSON`.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` iteratorni agacha tirik saqlaydi
  signal qabul qilinadi; faqat birinchi bir necha voqealar kerak bo'lganda `STREAM_MAX_EVENTS=25` o'ting
  tutun sinovi uchun.
- `ToriiClient.streamSumeragiStatus()` bir xil interfeysni aks ettiradi
  `/v1/sumeragi/status/sse`, shuning uchun konsensus telemetriyasi alohida-alohida bo'lishi mumkin va
  iterator `Last-Event-ID` ni xuddi shu tarzda hurmat qiladi.
- `javascript/iroha_js/recipes/streaming.mjs` ga kalit topshirilgan CLI (kursorning barqarorligi,
  env-var filtrini bekor qiladi va JS4 da ishlatiladigan `extractPipelineStatusKind` jurnali)
  oqim/WebSocket yo'l xaritasi yetkazib beriladi.

## UAID portfellari va kosmik katalogi

Space Directory API'lari universal hisob identifikatori (UAID) hayot aylanishini ko'rsatadi. The
yordamchilar `uaid:<hex>` literallarini yoki xom 64-hex dayjestlarini (LSB=1) qabul qiladi va
so'rovlarni yuborishdan oldin ularni kanoniklashtiring:

- `getUaidPortfolio(uaid, { assetId })` har bir ma'lumot maydoni uchun balanslarni birlashtiradi,
  aktivlarni kanonik hisob identifikatorlari bo'yicha guruhlash; filtrlash uchun `assetId` dan o'ting
  portfelni bitta aktiv misoliga tushiring.
- `getUaidBindings(uaid)` har bir maʼlumot maydoni ↔ hisobini sanab beradi
  majburiy (`i105` `i105` harflarini qaytaradi).
- `getUaidManifests(uaid, { dataspaceId })` har bir qobiliyat manifestini qaytaradi,
  hayot aylanishi holati va audit uchun bog'langan hisoblar.Operator dalillar paketlari, manifest nashri/bekor qilish oqimlari va SDK migratsiyasi uchun
yo'l-yo'riq, Universal Account Guide (`docs/source/universal_accounts_guide.md`) ga amal qiling
ushbu mijoz yordamchilari bilan bir qatorda portal va manba hujjatlari sinxronlashtiriladi.

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

Operatorlar manifestlarni aylantirishi yoki favqulodda rad etish-yutuq oqimlarisiz ham bajarishi mumkin
CLI ga tushish. Ikkala yordamchi ham ixtiyoriy `{ signal }` ob'ektini qabul qiladi
uzoq muddatli taqdimotlar `AbortController` bilan bekor qilinishi mumkin; ob'ekt bo'lmagan
variantlari yoki `AbortSignal` bo'lmagan kirishlar sinxronlashdan oldin `TypeError` ni oshiradi.
so'rov soni Torii:

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
```

`publishSpaceDirectoryManifest()` xom manifest JSON-ni qabul qiladi (mos keladi).
`fixtures/space_directory/` ostidagi moslamalar) yoki seriyali har qanday ob'ekt
bir xil tuzilish. `privateKey`, `privateKeyHex` yoki `privateKeyMultihash` xaritasi
`ExposedPrivateKey` maydoni Torii kutadi va sukut bo'yicha `ed25519`
prefiks berilmaganda algoritm. Ikkala so'rov ham Torii navbati bilan qaytariladi
ko'rsatma (`202 Accepted`), shu nuqtada daftar chiqadi
mos keladigan `SpaceDirectoryEvent`.

## Boshqaruv va ISO ko'prigi

`ToriiClient` shartnomalarni tekshirish, bosqichma-bosqich ko'rsatish uchun boshqaruv API'larini ochib beradi
takliflar, byulletenlarni topshirish (oddiy yoki ZK), kengashni aylantirish va chaqirish
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` qo'lda yozilgan DTOlarsiz. ISO 20022 yordamchilari
`buildPacs008Message`/`buildPacs009Message` orqali bir xil naqshga amal qiling va
`submitIso*`/`waitForIsoMessageStatus` trio.

[boshqaruv va ISO koʻprigi retsepti](./recipes/javascript-governance-iso.md) bilan tanishing
CLI-ga tayyor namunalar va to'liq maydon qo'llanmasiga ko'rsatgichlar uchun
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC namunalari va yetkazib berish dalillari

JS yo'l xaritasi, shuningdek, operatorlar uchun Roadrunner Block Commitment (RBC) namunalarini olishni talab qiladi.
Sumeragi orqali olingan blok ular tekshirgan bo'lak dalillariga mos kelishini isbotlang.
Qo'lda foydali yuklarni qurish o'rniga o'rnatilgan yordamchilardan foydalaning:

1. `getSumeragiRbcSessions()` oynalari `/v1/sumeragi/rbc/sessions` va
   `findRbcSamplingCandidate()` blok xeshi bilan birinchi yetkazib berilgan seansni avtomatik tanlaydi
   (integratsiya to'plami istalgan vaqtda unga qaytadi
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` sozlanmagan).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` `{blockHash,height,view}`ni normallashtiradi
   plyus ixtiyoriy `{count,seed,apiToken}` bekor qiladi, shuning uchun noto'g'ri tuzilgan olti burchakli yoki manfiy tamsayılar hech qachon bo'lmaydi
   Torii ga yeting.
3. `sampleRbcChunks()` so‘rovni `/v1/sumeragi/rbc/sample` ga POSTga qo‘yadi va bo‘lak dalillarni qaytaradi
   va Merkle yo'llari (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) bilan arxivlashingiz kerak
   asrab olish haqidagi dalillaringizning qolgan qismi.
4. `getSumeragiRbcDelivered(height, view)` auditorlar uchun kohortning yetkazib berish metamaʼlumotlarini yozib oladi
   isbotni oxirigacha takrorlashi mumkin.

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

Boshqaruvga topshirgan artefakt ildizi ostida ikkala javobni ham davom ettiring. ni bekor qiling
`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` orqali avtomatik tanlangan seans
Agar ma'lum bir blokni tekshirishingiz kerak bo'lsa va RBC snapshotlarini olishda xatoliklarni hal qiling.
to'g'ridan-to'g'ri rejimga jimgina pasayishdan ko'ra, parvoz oldidan o'tish xatosi.

## Sinov va CI

1. Kesh yuk va npm artefaktlari.
2. `npm run build:native` ni ishga tushiring.
3. `npm test` (yoki tutun ishlari uchun `node --test`) ni bajaring.

Malumot GitHub Actions ish oqimi mavjud
`docs/source/examples/iroha_js_ci.md`.

## Keyingi qadamlar

- `javascript/iroha_js/index.d.ts` da yaratilgan turlarni ko'rib chiqing.
- `javascript/iroha_js/recipes/` ostidagi retseptlarni o'rganing.
- Foydali yuklarni tekshirish uchun `ToriiClient`-ni Norito tezkor ishga tushirish bilan bog'lang
  SDK qo'ng'iroqlari.