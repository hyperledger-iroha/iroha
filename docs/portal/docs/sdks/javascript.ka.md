---
lang: ka
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

`@iroha/iroha-js` არის კანონიკური Node.js პაკეტი Torii-თან ურთიერთობისთვის. ის
პაკეტები Norito მშენებლები, Ed25519 დამხმარეები, პაგინაციის საშუალებები და ელასტიური
HTTP/WebSocket კლიენტი, ასე რომ თქვენ შეგიძლიათ ასახოთ CLI ნაკადები TypeScript-დან.

## ინსტალაცია

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

მშენებლობის საფეხური ახვევს `cargo build -p iroha_js_host`. დარწმუნდით, რომ ხელსაწყოების ჯაჭვი
`rust-toolchain.toml` ხელმისაწვდომია ადგილობრივად `npm run build:native`-ის გაშვებამდე.

## გასაღების მართვა

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

## შექმენით ტრანზაქციები

Norito ინსტრუქციების შემქმნელები ახდენს იდენტიფიკატორების, მეტამონაცემების და რაოდენობების ნორმალიზებას.
დაშიფრული ტრანზაქციები ემთხვევა Rust/CLI დატვირთვას.

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

## Torii კლიენტის კონფიგურაცია

`ToriiClient` იღებს ხელახლა ცდის/დროის ამოღების ღილაკებს, რომლებიც ასახავს `iroha_config`-ს. გამოყენება
`resolveToriiClientConfig` camelCase კონფიგურაციის ობიექტის გაერთიანებისთვის (ნორმალიზება
`iroha_config` ჯერ), env გადაფარვები და ინლაინ ვარიანტები.

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

გარემოს ცვლადები ადგილობრივი დეველოპერებისთვის:

| ცვლადი | დანიშნულება |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | მოთხოვნის დროის ამოწურვა (მილიწამები). |
| `IROHA_TORII_MAX_RETRIES` | განმეორებითი ცდის მაქსიმალური მცდელობა. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | თავდაპირველი ხელახალი უკან დაბრუნება. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | ექსპონენციალური უკან დახევის მულტიპლიკატორი. |
| `IROHA_TORII_MAX_BACKOFF_MS` | განმეორებითი ცდის მაქსიმალური დაგვიანება. |
| `IROHA_TORII_RETRY_STATUSES` | მძიმით გამოყოფილი HTTP სტატუსის კოდები ხელახლა საცდელად. |
| `IROHA_TORII_RETRY_METHODS` | მძიმით გამოყოფილი HTTP მეთოდები ხელახლა საცდელად. |
| `IROHA_TORII_API_TOKEN` | ამატებს `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | ამატებს `Authorization: Bearer …` სათაურს. |

პროფილები ხელახლა ასახავს Android-ის ნაგულისხმევს და ექსპორტირებულია პარიტეტის შესამოწმებლად:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. იხილეთ `docs/source/sdk/js/torii_retry_policy.md`
ბოლო წერტილიდან პროფილის რუკების და პარამეტრების მართვის აუდიტის დროს
JS4/JS7.

## გამეორებადი სიები და პაგინაცია

პაგინაციის დამხმარეები ასახავს Python SDK ერგონომიკას `/v1/accounts`-ისთვის,
`/v1/domains`, `/v1/assets/definitions`, NFTs, ნაშთები, აქტივების მფლობელები და
ანგარიშის ტრანზაქციის ისტორია.

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

## ოფლაინ შეღავათები და განაჩენის მეტამონაცემები

ოფლაინ შემწეობის პასუხები ავლენს გამდიდრებულ წიგნში მეტამონაცემებს -
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` და `remaining_amount` ბრუნდება ნედლეულთან ერთად
ჩაწერეთ ისე, რომ დაფებს არ დასჭირდეთ ჩაშენებული Norito დატვირთვის გაშიფვრა. ახალი
ათვლის დამხმარეები (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) მონიშნეთ შემდეგი ვადა (განახლება → პოლიტიკა
→ სერთიფიკატი) ასე რომ, UI სამკერდე ნიშნებს შეუძლიათ გააფრთხილონ ოპერატორები, როცა შემწეობა აქვს
<24 საათი დარჩა. SDK
ასახავს `/v1/offline/reserve/topup`-ის მიერ გამოვლენილ REST ფილტრებს:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` და
`requireVerdict` / `onlyMissingVerdict` ლოგინები. არასწორი კომბინაციები (ამისთვის
მაგალითად `onlyMissingVerdict` + `verdictIdHex`) ადგილობრივად უარყოფილია Torii-მდე
ეწოდება.

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

## ოფლაინ შევსება (გამოცემა + რეგისტრაცია)

გამოიყენეთ შევსების დამხმარეები, როდესაც გსურთ სერთიფიკატის გაცემა და დაუყოვნებლივ
დაარეგისტრირეთ იგი წიგნში. SDK ამოწმებს გაცემულ და რეგისტრირებულ სერტიფიკატს
ID-ები ემთხვევა დაბრუნებამდე და პასუხი მოიცავს ორივე დატვირთვას. არსებობს
არ არის გამოყოფილი შევსების საბოლოო წერტილი; დამხმარე აკავშირებს საკითხს + დაარეგისტრირებს ზარებს. თუ
თქვენ უკვე გაქვთ ხელმოწერილი სერთიფიკატი, დარეკეთ `registerOfflineAllowance` (ან
`renewOfflineAllowance`) პირდაპირ.

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

## Torii მოთხოვნები და ნაკადი (WebSockets)

შეკითხვის დამხმარეები აჩვენებენ სტატუსს, Prometheus მეტრიკას, ტელემეტრიის კადრებს და მოვლენას
ნაკადები Norito ფილტრის გრამატიკის გამოყენებით. სტრიმინგი ავტომატურად განახლდება
WebSockets და განახლდება, როდესაც ხელახალი ცდის ბიუჯეტი საშუალებას იძლევა.

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

გამოიყენეთ `streamBlocks`, `streamTransactions` ან `streamTelemetry` სხვებისთვის
WebSocket საბოლოო წერტილები. ყველა სტრიმინგის დამხმარე ზედაპირზე ხელახლა ცდის მცდელობას, ასე რომ, მიამაგრეთ
`onReconnect` გამოძახება დაფების შესანახად და გაფრთხილებისთვის.

## Explorer-ის სნეპშოტები და QR დატვირთვები

Explorer ტელემეტრია უზრუნველყოფს აკრეფილ დამხმარეებს `/v1/explorer/metrics`-სთვის და
`/v1/explorer/accounts/{account_id}/qr` ბოლო წერტილებია, რათა დაფამ შეძლოს მისი ხელახლა დაკვრა
იგივე კადრები, რომლებიც აძლიერებენ პორტალს. `getExplorerMetrics()` ახდენს ნორმალიზებას
დატვირთვა და აბრუნებს `null`, როდესაც მარშრუტი გამორთულია. დააწყვილეთ იგი
`getExplorerAccountQr()` როცა დაგჭირდებათ i105 (სასურველია)/სორა (მეორე საუკეთესო) ლიტერალები პლუს ინლაინ
SVG გაზიარების ღილაკებისთვის.

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

`i105`-ის გავლა ასახავს Explorer-ის ნაგულისხმევ შეკუმშვას
სელექტორები; გამოტოვეთ უგულებელყოფა სასურველი i105 გამომავალისთვის ან მოითხოვეთ `i105_qr`
როცა გჭირდებათ QR-უსაფრთხო ვარიანტი. შეკუმშული ლიტერალი მეორე საუკეთესოა
მხოლოდ Sora-ის ვარიანტი UX-ისთვის. დამხმარე ყოველთვის აბრუნებს კანონიკურ იდენტიფიკატორს,
არჩეული ლიტერალი და მეტამონაცემები (ქსელის პრეფიქსი, QR ვერსია/მოდულები, შეცდომა
კორექტირების დონე და inline SVG), ასე რომ CI/CD-ს შეუძლია გამოაქვეყნოს იგივე დატვირთვები, რაც
Explorer ზედაპირზე ჩნდება შეკვეთილი გადამყვანების გამოძახების გარეშე.

## სესიების და რიგის დაკავშირება

Connect დამხმარე სარკე `docs/source/connect_architecture_strawman.md`. The
უსწრაფესი გზა წინასწარი გადახედვისთვის მზად სესიისკენ არის `bootstrapConnectPreviewSession`,
რომელიც აერთიანებს დეტერმინისტულ SID/URI თაობას და Torii-ს
რეგისტრაციის ზარი.

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

- გაიარეთ `register: false`, როდესაც გჭირდებათ მხოლოდ განმსაზღვრელი URI-ები QR/deeplink-ისთვის
  გადახედვები.
- `generateConnectSid` რჩება ხელმისაწვდომი, როდესაც გჭირდებათ სესიის ID-ების გამომუშავება
  URI-ების მოჭრის გარეშე.
- მიმართულების გასაღებები და შიფრული ტექსტის კონვერტები მომდინარეობს მშობლიური ხიდიდან; როცა
  მიუწვდომელია SDK ბრუნდება JSON კოდეკში და ისვრის
  `ConnectQueueError.bridgeUnavailable`.
- ოფლაინ ბუფერები ინახება როგორც Norito `.to` blobs IndexedDB-ში. მონიტორის რიგი
  მდგომარეობა გამოსხივებული `ConnectQueueError.overflow(limit)` /
  `.expired(ttlMs)` შეცდომები და შესანახი `connect.queue_depth` ტელემეტრია, როგორც აღწერილია
  საგზაო რუკაში.

### დააკავშირეთ რეესტრისა და პოლიტიკის სნეპშოტები

პლატფორმის ოპერატორებს შეუძლიათ დაათვალიერონ და განაახლონ Connect რეესტრის გარეშე
ტოვებს Node.js-ს. `iterateConnectApps()` გვერდები რეესტრის მეშვეობით, ხოლო
`getConnectStatus()` და `getConnectAppPolicy()` ავლენს გაშვების მრიცხველებს და
მიმდინარე პოლიტიკის კონვერტი. `updateConnectAppPolicy()` იღებს camelCase ველებს,
ასე რომ, თქვენ შეგიძლიათ დადგმოთ იგივე JSON დატვირთვა, რომელსაც Torii ელოდება.

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

ყოველთვის გადაიღეთ უახლესი `getConnectStatus()` სნეპშოტი განაცხადის დაწყებამდე
მუტაციები — მმართველობის საკონტროლო სია მოითხოვს მტკიცებულებას, რომ პოლიტიკის განახლებები იწყება
ფლოტის ამჟამინდელი საზღვრებიდან.

### შეაერთეთ WebSocket აკრეფა

`ToriiClient.openConnectWebSocket()` აწყობს კანონიკურს
`/v1/connect/ws` URL (მათ შორის, `sid`, `role` და ტოკენის პარამეტრები), განახლებები
`http→ws` / `https→wss` და გადასცემს საბოლოო URL ნებისმიერ WebSocket-ს
განხორციელებას თქვენ აწვდით. ბრაუზერები ავტომატურად ხელახლა იყენებენ გლობალურს
`WebSocket`. Node.js აბონენტებმა უნდა გაიარონ ისეთი კონსტრუქტორი, როგორიცაა `ws`:

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

როცა მხოლოდ URL გჭირდებათ, დარეკეთ `torii.buildConnectWebSocketUrl(params)` ან
უმაღლესი დონის `buildConnectWebSocketUrl(baseUrl, params)` დამხმარე და ხელახლა გამოიყენეთ
შედეგად მიღებული სტრიქონი საბაჟო ტრანსპორტის/რიგში.

ეძებთ სრულ CLI-ზე ორიენტირებულ ნიმუშს? The
[დაკავშირება წინასწარი გადახედვის რეცეპტი] (./recipes/javascript-connect-preview.md) მოიცავს ა
გაშვებადი სკრიპტი პლუს ტელემეტრიის სახელმძღვანელო, რომელიც ასახავს საგზაო რუქის მიწოდებას
Connect queue + WebSocket ნაკადის დოკუმენტირება.

### რიგი ტელემეტრია და გაფრთხილება

მავთულის რიგის მეტრიკა პირდაპირ დამხმარე ზედაპირებზეა, რათა დაფები ასახული იყოს
საგზაო რუკა KPIs.

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

`ConnectQueueError#toConnectError()` გარდაქმნის რიგის წარუმატებლობას ზოგად
`ConnectError` ტაქსონომია, ასე რომ გაზიარებულ HTTP/WebSocket-ის ჩამჭრელებს შეუძლიათ ასხივონ
სტანდარტული `connect.queue_depth`, `connect.queue_overflow_total` და
`connect.queue_expired_total` მეტრიკა მითითებულია საგზაო რუკაზე.

## სტრიმინგის დამკვირვებლები და მოვლენის კურსორები

`ToriiClient.streamEvents()` ავლენს `/v1/events/sse`-ს, როგორც ასინქრონულ იტერატორს ავტომატური
ხელახლა ცდის, ასე რომ Node/Bun CLI-ებს შეუძლიათ მილსადენის აქტივობის კუდში შეყვანა ისევე, როგორც Rust CLI აკეთებს.
შეინახეთ `Last-Event-ID` კურსორი თქვენი runbook არტეფაქტებთან ერთად, რათა ოპერატორებმა შეძლონ
განაახლეთ ნაკადი მოვლენების გამოტოვების გარეშე, როდესაც პროცესი განახლდება.

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

- გადართეთ `PIPELINE_STATUS` (მაგალითად `Pending`, `Applied`, ან `Approved`) ან დააყენეთ
  `STREAM_FILTER_JSON` იგივე ფილტრების გასამეორებლად, რომლებსაც CLI იღებს.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` ინარჩუნებს იტერატორს ა
  სიგნალი მიიღება; გაიარეთ `STREAM_MAX_EVENTS=25`, როდესაც დაგჭირდებათ მხოლოდ პირველი რამდენიმე ღონისძიება
  კვამლის ტესტისთვის.
- `ToriiClient.streamSumeragiStatus()` ასახავს იმავე ინტერფეისს
  `/v1/sumeragi/status/sse` ასე რომ, კონსენსუსის ტელემეტრია შეიძლება ცალ-ცალკე განხორციელდეს და
  iterator პატივს სცემს `Last-Event-ID`-ს იმავე გზით.
- იხილეთ `javascript/iroha_js/recipes/streaming.mjs` ანაზრაურების CLI-სთვის (კურსორის გამძლეობა,
  env-var ფილტრის უგულებელყოფა და `extractPipelineStatusKind` logging) გამოიყენება JS4-ში
  ნაკადი/WebSocket საგზაო რუკის მიწოდება.

## UAID პორტფოლიოები და კოსმოსური დირექტორია

Space Directory API-ები ასახავს უნივერსალური ანგარიშის ID (UAID) სასიცოცხლო ციკლს. The
დამხმარეები იღებენ `uaid:<hex>` ლიტერალებს ან ნედლეულს 64-თექვსმეტიან დიჯესტს (LSB=1) და
შეასრულეთ ისინი კანონიკურად მოთხოვნის გაგზავნამდე:

- `getUaidPortfolio(uaid, { assetId })` აგროვებს ნაშთებს მონაცემთა სივრცეში,
  აქტივების ქონების დაჯგუფება კანონიკური ანგარიშის ID-ების მიხედვით; გაფილტრეთ `assetId`
  პორტფელი ერთ აქტივამდე.
- `getUaidBindings(uaid)` ჩამოთვლის ყველა მონაცემთა სივრცეს ↔ ანგარიშს
  სავალდებულო (`i105` აბრუნებს `i105` ლიტერალებს).
- `getUaidManifests(uaid, { dataspaceId })` აბრუნებს თითოეულ შესაძლებლობის მანიფესტს,
  სასიცოცხლო ციკლის სტატუსი და შეკრული ანგარიშები აუდიტისთვის.ოპერატორის მტკიცებულების პაკეტებისთვის, მანიფესტის გამოქვეყნება/გაუქმება ნაკადები და SDK მიგრაცია
მითითებები, მიჰყევით უნივერსალური ანგარიშის სახელმძღვანელოს (`docs/source/universal_accounts_guide.md`)
კლიენტის დამხმარეებთან ერთად, ასე რომ, პორტალი და წყაროს დოკუმენტაცია სინქრონიზებული რჩება.

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

ოპერატორებს ასევე შეუძლიათ მანიფესტების როტაცია ან გადაუდებელი უარყოფა-გამარჯვების ნაკადების შესრულება
ჩაშვება CLI-ზე. ორივე დამხმარე იღებს არასავალდებულო `{ signal }` ობიექტს
ხანგრძლივი წარდგენის გაუქმება შესაძლებელია `AbortController`-ით; არაობიექტური
ოფციები ან არა-`AbortSignal` შეყვანები ზრდის სინქრონულ `TypeError`-ს
მოთხოვნა მოხვდება Torii:

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

`publishSpaceDirectoryManifest()` იღებს ან დაუმუშავებელ მანიფესტ JSON-ს (შეესაბამება
მოწყობილობები `fixtures/space_directory/` ქვეშ) ან ნებისმიერი ობიექტი, რომელიც სერიულირდება
იგივე სტრუქტურა. `privateKey`, `privateKeyHex`, ან `privateKeyMultihash` რუკაზე
`ExposedPrivateKey` ველი Torii მოელის და ნაგულისხმევია `ed25519`
ალგორითმი, როდესაც პრეფიქსი არ არის მოწოდებული. ორივე მოთხოვნა ბრუნდება ერთხელ Torii რიგში
ინსტრუქცია (`202 Accepted`), რა დროსაც წიგნი გამოსცემს
შესატყვისი `SpaceDirectoryEvent`.

## მმართველობა და ISO ხიდი

`ToriiClient` ავლენს მმართველობის API-ებს კონტრაქტების შესამოწმებლად, დადგმისთვის
წინადადებები, ბიულეტენების წარდგენა (უბრალო ან ზ.კ.), საბჭოს როტაცია და მოწვევა
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` ხელით დაწერილი DTO-ების გარეშე. ISO 20022 დამხმარეები
დაიცავით იგივე ნიმუში `buildPacs008Message`/`buildPacs009Message` და
`submitIso*`/`waitForIsoMessageStatus` ტრიო.

იხილეთ [მართვისა და ISO ხიდის რეცეპტი] (./recipes/javascript-governance-iso.md)
CLI-სთვის მზა ნიმუშებისთვის, პლუს მაჩვენებლების დაბრუნება ველის სრულ სახელმძღვანელოში
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC სინჯის აღება და მიწოდების მტკიცებულება

JS საგზაო რუკა ასევე მოითხოვს Roadrunner Block Commitment (RBC) შერჩევას, რათა ოპერატორებმა შეძლონ
დაამტკიცეთ, რომ ბლოკი, რომელიც მათ მოიტანა Sumeragi-ით, ემთხვევა მათ მიერ დამოწმებულ ნაწილს.
გამოიყენეთ ჩაშენებული დამხმარეები იმის ნაცვლად, რომ ააშენოთ ტვირთი ხელით:

1. `getSumeragiRbcSessions()` სარკეები `/v1/sumeragi/rbc/sessions` და
   `findRbcSamplingCandidate()` ავტომატურად ირჩევს პირველ მიწოდებულ სესიას ბლოკის ჰეშით
   (ინტეგრაციის კომპლექტი მას უბრუნდება ნებისმიერ დროს
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` არ არის დაყენებული).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` ნორმალიზდება `{blockHash,height,view}`
   პლუს არასავალდებულო `{count,seed,apiToken}` უგულებელყოფს ასე არასწორი თექვსმეტობით ან უარყოფით მთელ რიცხვებს არასოდეს
   მიაღწიეთ Torii.
3. `sampleRbcChunks()` აგზავნის მოთხოვნას `/v1/sumeragi/rbc/sample`-ზე, აბრუნებს ნაწილაკების მტკიცებულებებს
   და მერკლის ბილიკები (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) უნდა დაარქივოთ
   თქვენი შვილად აყვანის დანარჩენი მტკიცებულებები.
4. `getSumeragiRbcDelivered(height, view)` იღებს კოჰორტის მიწოდების მეტამონაცემებს, ასე რომ აუდიტორები
   შეუძლია მტკიცებულების გამეორება ბოლომდე.

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

შეინარჩუნეთ ორივე პასუხი იმ არტეფაქტის ძირის ქვეშ, რომელსაც წარუდგენთ მმართველობას. გადალახეთ
ავტომატურად შერჩეული სესია `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`-ის საშუალებით
როდესაც გჭირდებათ კონკრეტული ბლოკის გამოკვლევა და RBC კადრების მოტანის წარუმატებლობა, როგორც
ფრენის წინ შეცდომა, ვიდრე ჩუმად დაქვეითება პირდაპირ რეჟიმში.

## ტესტირება და CI

1. ქეში ტვირთი და npm არტეფაქტები.
2. გაუშვით `npm run build:native`.
3. შეასრულეთ `npm test` (ან `node --test` კვამლის სამუშაოებისთვის).

საცნობარო GitHub Actions სამუშაო ნაკადი ცხოვრობს
`docs/source/examples/iroha_js_ci.md`.

## შემდეგი ნაბიჯები

- გადახედეთ გენერირებულ ტიპებს `javascript/iroha_js/index.d.ts`-ში.
- შეისწავლეთ რეცეპტები `javascript/iroha_js/recipes/`-ში.
- დააწყვილეთ `ToriiClient` Norito სწრაფი სტარტთან, რათა შეამოწმოთ ტვირთამწეობა ერთად
  SDK ზარები.