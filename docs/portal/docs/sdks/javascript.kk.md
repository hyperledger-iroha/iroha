---
lang: kk
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

`@iroha/iroha-js` — Torii қызметімен әрекеттесу үшін канондық Node.js бумасы. Ол
пакеттері Norito құрастырушылары, Ed25519 көмекшілері, беттеу утилиталары және серпімді
HTTP/WebSocket клиенті, осылайша TypeScript-тен CLI ағындарын көрсетуге болады.

## Орнату

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

Құрастыру қадамы `cargo build -p iroha_js_host` орады. Аспаптар тізбегін қайдан қамтамасыз етіңіз
`rust-toolchain.toml` `npm run build:native` іске қоспас бұрын жергілікті жерде қол жетімді.

## Кілтті басқару

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

## Транзакцияларды құру

Norito нұсқаулығын құрастырушылар идентификаторларды, метадеректерді және мөлшерлерді қалыпқа келтіреді.
кодталған транзакциялар Rust/CLI пайдалы жүктемелеріне сәйкес келеді.

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
  destinationAccountId: "soraカタカナ...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "soraカタカナ...",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "soraカタカナ...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Torii клиент конфигурациясы

`ToriiClient` `iroha_config` шағылыстыратын қайталау/күту уақыты тұтқаларын қабылдайды. Қолдану
CamelCase конфигурациялау нысанын біріктіру үшін `resolveToriiClientConfig` (қалыптылау
`iroha_config` бірінші), env қайта анықтау және кірістірілген опциялар.

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

Жергілікті әзірлеушіге арналған орта айнымалылары:

| Айнымалы | Мақсаты |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Сұраныс күту уақыты (миллисекунд). |
| `IROHA_TORII_MAX_RETRIES` | Ең көп қайталау әрекеттері. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Бастапқы әрекетті кері қайтару. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Экспоненциалды кері көбейткіш. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Ең көп қайталау кідірісі. |
| `IROHA_TORII_RETRY_STATUSES` | Қайталау үшін үтірмен бөлінген HTTP күй кодтары. |
| `IROHA_TORII_RETRY_METHODS` | Қайталау үшін үтірмен бөлінген HTTP әдістері. |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` қосады. |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` тақырыбын қосады. |

Қайталау профильдері Android әдепкі параметрлерін көрсетеді және теңдік тексерулері үшін экспортталады:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. `docs/source/sdk/js/torii_retry_policy.md` қараңыз
соңғы нүктеден профильге салыстыру және параметрлерді басқару аудиттері үшін
JS4/JS7.

## Қайталанатын тізімдер және беттеу

Беттеу көмекшілері `/v1/accounts` үшін Python SDK эргономикасын көрсетеді,
`/v1/domains`, `/v1/assets/definitions`, NFTs, баланстар, активтерді ұстаушылар және
шоттағы транзакциялар тарихы.

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
const balances = await torii.listAccountAssets("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Офлайн жеңілдіктер және үкім метадеректері

Офлайн төлем жауаптары байытылған кітап метадеректерін алдын ала көрсетеді —
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` және `remaining_amount` шикізатпен бірге қайтарылады
бақылау тақталары ендірілген Norito пайдалы жүктемелерін декодтауды қажет етпейтін етіп жазып алыңыз. Жаңа
кері санақ көмекшілері (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) келесі аяқталатын мерзімін белгілеу (жаңарту → саясат)
→ сертификат) сондықтан UI бейджиктері операторларға жәрдемақы болған кезде ескертеді
<24 сағат қалды. SDK
`/v1/offline/reserve/topup` көрсеткен REST сүзгілерін көрсетеді:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` және
`requireVerdict` / `onlyMissingVerdict` логикалық мәндер. Жарамсыз комбинациялар (үшін
мысалы `onlyMissingVerdict` + `verdictIdHex`) Torii алдында жергілікті түрде қабылданбады
деп аталады.

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

## Офлайн толтыру (мәселе + тіркеу)

Сертификат беру керек кезде және дереу толтыру көмекшілерін пайдаланыңыз
кітапқа тіркеңіз. SDK берілген және тіркелген куәлікті тексереді
Идентификаторлар қайтару алдында сәйкес келеді және жауап екі пайдалы жүктемені де қамтиды. Бар
арнайы толтыру нүктесі жоқ; көмекші мәселені тізбектейді + қоңырауларды тіркеу. Егер
Сізде қол қойылған сертификат бар, `registerOfflineAllowance` (немесе
`renewOfflineAllowance`) тікелей.

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

## Torii сұраулары және ағыны (WebSockets)

Сұрау көмекшілері күйді, Prometheus метрикасын, телеметрия суреттерін және оқиғаны көрсетеді
Norito сүзгі грамматикасы арқылы ағындар. Ағын автоматты түрде жаңартылады
WebSockets және қайталау бюджеті рұқсат еткенде жалғастырады.

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

Басқасы үшін `streamBlocks`, `streamTransactions` немесе `streamTelemetry` пайдаланыңыз.
WebSocket соңғы нүктелері. Барлық ағындық көмекшілер қайталау әрекеттерін жасайды, сондықтан ілмек
Бақылау тақталарын беру және ескерту үшін `onReconnect` кері шақыру.

## Explorer суреттері және QR пайдалы жүктемелері

Explorer телеметриясы `/v1/explorer/metrics` және үшін терілген көмекшілерді қамтамасыз етеді
`/v1/explorer/accounts/{account_id}/qr` соңғы нүктелері бақылау тақталары қайта ойната алады
порталға қуат беретін бірдей суреттер. `getExplorerMetrics()` қалыпқа келтіреді
пайдалы жүктеме және маршрут өшірілген кезде `null` қайтарады. Онымен жұптаңыз
`getExplorerAccountQr()` қажет кезде i105 (қалаулы)/sora (екінші ең жақсы) литералдар плюс кірістірілген
Бөлісу түймелеріне арналған SVG.

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

const qr = await torii.getExplorerAccountQr("soraカタカナ...");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

`i105` өту Explorer әдепкі қысылғанын көрсетеді
селекторлар; таңдаулы i105 шығысы немесе `i105_qr` сұрауы үшін қайта анықтауды өткізіп жіберіңіз
QR-қауіпсіз нұсқасы қажет болғанда. Қысылған литерал екінші ең жақсысы
UX үшін тек Sora опциясы. Көмекші әрқашан канондық идентификаторды қайтарады,
таңдалған литерал және метадеректер (желі префиксі, QR нұсқасы/модульдері, қате
түзету деңгейі және кірістірілген SVG), сондықтан CI/CD бірдей пайдалы жүктемелерді жариялай алады.
Explorer арнайы түрлендіргіштерді шақырмай жұмыс істейді.

## Сеанстарды және кезекті қосу

Connect көмекшілері `docs/source/connect_architecture_strawman.md` айнасы. The
алдын ала қарауға дайын сеанстың ең жылдам жолы - `bootstrapConnectPreviewSession`,
детерминирленген SID/URI генерациясын және Torii біріктіреді
тіркеу қоңырауы.

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

- QR/deeplink үшін тек детерминирленген URI қажет болғанда `register: false` өтіңіз.
  алдын ала қараулар.
- `generateConnectSid` сеанс идентификаторларын алу қажет болғанда қолжетімді болып қалады
  URI кодтары жоқ.
- бағыттаушы кілттер мен шифрлық мәтін конверттері жергілікті көпірден келеді; қашан
  қолжетімсіз болса, SDK JSON кодекке қайта түседі және лақтырады
  `ConnectQueueError.bridgeUnavailable`.
- Офлайн буферлер IndexedDB ішінде Norito `.to` блоктары ретінде сақталады. Монитор кезегі
  шығарылған `ConnectQueueError.overflow(limit)` арқылы күй /
  `.expired(ttlMs)` қателері және `connect.queue_depth` телеметриясын беру көрсетілгендей
  жол картасында.

### Тіркеу мен саясат суреттерін қосыңыз

Платформа операторлары Connect тізілімін интроспекциясыз және жаңарта алады
Node.js шығу. `iterateConnectApps()` беттер тізілім арқылы, ал
`getConnectStatus()` және `getConnectAppPolicy()` жұмыс уақыты есептегіштерін көрсетеді және
ағымдағы саясат конверті. `updateConnectAppPolicy()` camelCase өрістерін қабылдайды,
сондықтан Torii күткен JSON пайдалы жүктемесін орындай аласыз.

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

Қолданар алдында әрқашан соңғы `getConnectStatus()` суретін түсіріңіз.
мутациялар — басқаруды тексеру тізімі саясат жаңартуларының басталатынын дәлелдеуді талап етеді
флоттың ағымдағы шегінен.

### WebSocket теруді қосыңыз

`ToriiClient.openConnectWebSocket()` канондықты жинайды
`/v1/connect/ws` URL (соның ішінде `sid`, `role` және таңбалауыш параметрлері), жаңартулар
`http→ws` / `https→wss` және соңғы URL мекенжайын кез келген WebSocket-ке береді
жүзеге асыруды қамтамасыз етеді. Браузерлер жаһандықты автоматты түрде қайта пайдаланады
`WebSocket`. Node.js шақырушылары `ws` сияқты конструктордан өтуі керек:

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

Сізге тек URL мекенжайы қажет болғанда, `torii.buildConnectWebSocketUrl(params)` немесе телефон нөміріне қоңырау шалыңыз
жоғарғы деңгейлі `buildConnectWebSocketUrl(baseUrl, params)` көмекшісі және қайта пайдаланыңыз
реттелетін тасымалдау/кезекте нәтижесінде жол.

Толық CLI-бағдарланған үлгіні іздеп жүрсіз бе? The
[Қосылу рецептін алдын ала қарау](./recipes/javascript-connect-preview.md) мыналарды қамтиды:
орындалатын сценарий және жеткізуге болатын жол картасын көрсететін телеметриялық нұсқаулық
Қосылу кезегі + WebSocket ағынын құжаттау.

### Кезекте телеметрия және ескерту

Бақылау тақталары шағылыстыра алатындай сым кезек көрсеткіштерін тікелей көмекші беттерге қосыңыз
KPI жол картасы.

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

`ConnectQueueError#toConnectError()` кезек ақауларын жалпыға түрлендіреді
`ConnectError` таксономиясы ортақ HTTP/WebSocket ұстағыштары
стандартты `connect.queue_depth`, `connect.queue_overflow_total`, және
`connect.queue_expired_total` көрсеткіштерге жол картасы бойынша сілтеме жасалған.

## Ағынды бақылаушылар және оқиға курсорлары

`ToriiClient.streamEvents()` `/v1/events/sse` автоматты түрде асинхронды итератор ретінде көрсетеді
қайталап көріңіз, сондықтан Node/Bun CLI конвейер әрекетін Rust CLI сияқты кейінге қалдыра алады.
`Last-Event-ID` курсорын runbook артефактілерімен қатар ұстаңыз, осылайша операторлар
процесс қайта іске қосылғанда оқиғаларды өткізіп жібермей ағынды жалғастырыңыз.

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

- `PIPELINE_STATUS` қосқышы (мысалы, `Pending`, `Applied` немесе `Approved`) немесе орнату
  CLI қабылдайтын бірдей сүзгілерді қайта ойнату үшін `STREAM_FILTER_JSON`.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` итераторды a дейін тірі қалдырады
  сигнал қабылданады; Сізге тек алғашқы бірнеше оқиға қажет болғанда `STREAM_MAX_EVENTS=25` өтіңіз
  түтін сынағы үшін.
- `ToriiClient.streamSumeragiStatus()` бірдей интерфейсті көрсетеді
  `/v1/sumeragi/status/sse`, сондықтан консенсус телеметриясын бөлек бөлуге болады және
  итератор `Last-Event-ID`-ті дәл осылай құрметтейді.
- Кілтті CLI үшін `javascript/iroha_js/recipes/streaming.mjs` қараңыз (курсордың тұрақтылығы,
  env-var сүзгісін қайта анықтау және `extractPipelineStatusKind` журналы) JS4 ішінде пайдаланылады
  ағынды/WebSocket жол картасы жеткізіледі.

## UAID портфолиолары және ғарыштық каталог

Space Directory API интерфейстері әмбебап тіркелгі идентификаторының (UAID) өмірлік циклін көрсетеді. The
көмекшілер `uaid:<hex>` литералдарын немесе шикі 64 алтылық дайджесттерді (LSB=1) қабылдайды және
сұрауларды жібермес бұрын оларды канонизациялау:

- `getUaidPortfolio(uaid, { assetId })` деректер кеңістігіндегі баланстарды біріктіреді,
  активтерді ұстауды канондық шот идентификаторлары бойынша топтау; сүзу үшін `assetId` өтіңіз
  портфолио бір актив данасына дейін.
- `getUaidBindings(uaid)` әрбір деректер кеңістігін ↔ тіркелгісін санайды
  байланыстыру (`i105` `i105` литералдарын қайтарады).
- `getUaidManifests(uaid, { dataspaceId })` әрбір мүмкіндік манифестін қайтарады,
  өмірлік цикл күйі және аудит үшін байланыстырылған шоттар.Оператор дәлелдемелері бумалары үшін манифестті жариялау/қайтару ағындары және SDK тасымалдауы
Әмбебап тіркелгі нұсқаулығын (`docs/source/universal_accounts_guide.md`) орындаңыз
осы клиент көмекшілерімен қатар портал мен бастапқы құжаттама синхрондалған күйде қалады.

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

Операторлар манифесттерді айналдыра алады немесе төтенше жағдайдан бас тарту-жеңіс ағындарынсыз орындай алады
CLI-ге түсу. Екі көмекші де `{ signal }` қосымша нысанын қабылдайды
ұзақ мерзімді жіберулерден `AbortController` арқылы бас тартуға болады; объект емес
опциялары немесе `AbortSignal` емес кірістер синхронды `TypeError` мәнін
сұрау Torii соққылары:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "soraカタカナ...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "soraカタカナ...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` өңделмеген манифест JSON қабылдайды (сәйкес
`fixtures/space_directory/`) астында құрылғылар немесе серияланатын кез келген нысан
бірдей құрылым. `privateKey`, `privateKeyHex` немесе `privateKeyMultihash` картасы
`ExposedPrivateKey` өрісі Torii күтеді және әдепкі бойынша `ed25519`
префикс берілмеген кездегі алгоритм. Екі сұрау да Torii кезектерінен кейін қайтарылады
нұсқау (`202 Accepted`), бұл кезде кітап
сәйкес `SpaceDirectoryEvent`.

## Басқару және ISO көпірі

`ToriiClient` келісім-шарттарды, кезеңдерді тексеруге арналған басқару API интерфейстерін көрсетеді
ұсыныстар, дауыс беру бюллетеньдерін беру (қарапайым немесе ZK), кеңесті ротациялау және шақыру
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` қолмен жазылған DTO жоқ. ISO 20022 көмекшілері
`buildPacs008Message`/`buildPacs009Message` арқылы бірдей үлгіні орындаңыз және
`submitIso*`/`waitForIsoMessageStatus` трио.

[басқару және ISO көпір рецепті](./recipes/javascript-governance-iso.md) қараңыз
CLI-дайын үлгілер үшін плюс толық өріс нұсқаулығына қайтару көрсеткіштері
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC сынамаларын алу және жеткізу дәлелі

JS жол картасы сонымен қатар операторлардың
олар Sumeragi арқылы алынған блоктың олар тексеретін түйіндік дәлелдерге сәйкес келетінін дәлелдеңіз.
Пайдалы жүктемелерді қолмен құрудың орнына кірістірілген көмекшілерді пайдаланыңыз:

1. `getSumeragiRbcSessions()` айналары `/v1/sumeragi/rbc/sessions` және
   `findRbcSamplingCandidate()` блок хэшімен бірінші жеткізілген сеансты автоматты түрде таңдайды
   (интеграция жинағы кез келген уақытта оған қайта түседі
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` орнатылмаған).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` `{blockHash,height,view}` қалыпқа келтіреді
   плюс қосымша `{count,seed,apiToken}` қате пішінделген он алтылық немесе теріс бүтін сандарды қайта анықтайды.
   Torii жетеді.
3. `sampleRbcChunks()` сұрауды `/v1/sumeragi/rbc/sample` поштасына жіберіп, кесінді дәлелдерін қайтарады.
   және Merkle жолдары (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) арқылы мұрағаттау керек
   сіздің асырап алғаныңыз туралы қалған дәлелдер.
4. `getSumeragiRbcDelivered(height, view)` аудиторлар үшін когорттың жеткізу метадеректерін түсіреді
   дәлелдеуді басынан аяғына дейін қайталай алады.

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

Басқаруға жіберген артефакт түбірі бойынша екі жауапты да сақтаңыз. қайта белгілеңіз
`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` арқылы автоматты түрде таңдалған сеанс
белгілі бір блокты зерттеу қажет болған кезде және RBC суреттерін алу үшін сәтсіздіктерді қарастырыңыз.
Тікелей режимге үнсіз төмендетуге қарағанда, ұшу алдындағы қақпа қатесі.

## Тестілеу және CI

1. Кэш жүк және npm артефактілері.
2. `npm run build:native` іске қосыңыз.
3. `npm test` (немесе түтін жұмыстары үшін `node --test`) орындаңыз.

Анықтамалық GitHub Actions жұмыс процесі ішінде тұрады
`docs/source/examples/iroha_js_ci.md`.

## Келесі қадамдар

- `javascript/iroha_js/index.d.ts` ішінде жасалған түрлерді қарап шығыңыз.
- `javascript/iroha_js/recipes/` астындағы рецепттерді зерттеңіз.
- Пайдалы жүктемелерді тексеру үшін `ToriiClient` құрылғысын Norito жылдам іске қосу құралымен жұптаңыз.
  SDK қоңыраулары.