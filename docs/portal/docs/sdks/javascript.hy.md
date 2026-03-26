---
lang: hy
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

`@iroha/iroha-js`-ը Node.js կանոնական փաթեթն է՝ Torii-ի հետ փոխգործակցության համար: Այն
փաթեթներ Norito շինարարներ, Ed25519 օգնականներ, էջադրման կոմունալ ծառայություններ և ճկուն
HTTP/WebSocket հաճախորդ, որպեսզի կարողանաք արտացոլել CLI հոսքերը TypeScript-ից:

## Տեղադրում

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

Կառուցման քայլը փաթաթում է `cargo build -p iroha_js_host`: Ապահովեք գործիքների շղթան
`rust-toolchain.toml`-ը հասանելի է տեղում՝ նախքան `npm run build:native`-ը գործարկելը:

## Բանալիների կառավարում

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

## Կառուցեք գործարքներ

Norito հրահանգների ստեղծողները նորմալացնում են նույնացուցիչները, մետատվյալները և քանակները
կոդավորված գործարքները համընկնում են Rust/CLI բեռների հետ:

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

## Torii հաճախորդի կազմաձևում

`ToriiClient`-ն ընդունում է կրկնակի/ժամկետի կոճակները, որոնք արտացոլում են `iroha_config`: Օգտագործեք
`resolveToriiClientConfig`՝ camelCase կազմաձևման օբյեկտը միացնելու համար (նորմալացնել
`iroha_config` առաջինը), env-ի վերափոխումները և ներդիրային տարբերակները:

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

Շրջակա միջավայրի փոփոխականներ տեղական մշակողի համար.

| Փոփոխական | Նպատակը |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Հարցման ժամանակի սպառում (միլիվայրկյաններ): |
| `IROHA_TORII_MAX_RETRIES` | Առավելագույն կրկնակի փորձեր: |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Նախնական կրկնակի հետադարձ աշխատանք: |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Էքսպոնենցիալ հետընթաց բազմապատկիչ: |
| `IROHA_TORII_MAX_BACKOFF_MS` | Կրկնակի փորձի առավելագույն ուշացում: |
| `IROHA_TORII_RETRY_STATUSES` | Ստորակետերով բաժանված HTTP կարգավիճակի կոդեր՝ նորից փորձելու համար: |
| `IROHA_TORII_RETRY_METHODS` | Ստորակետերով բաժանված HTTP մեթոդներ՝ նորից փորձելու համար: |
| `IROHA_TORII_API_TOKEN` | Ավելացնում է `X-API-Token`: |
| `IROHA_TORII_AUTH_TOKEN` | Ավելացնում է `Authorization: Bearer …` վերնագիր: |

Կրկին փորձեք պրոֆիլները արտացոլում են Android-ի կանխադրվածները և արտահանվում են հավասարության ստուգման համար.
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. Տես `docs/source/sdk/js/torii_retry_policy.md`
վերջնակետից պրոֆիլ քարտեզագրման և պարամետրերի կառավարման աուդիտի ընթացքում
JS4/JS7.

## Կրկնվող ցուցակներ և էջադրում

Էջավորման օգնականները արտացոլում են Python SDK էրգոնոմիկան `/v1/accounts`-ի համար,
`/v1/domains`, `/v1/assets/definitions`, NFT-ներ, մնացորդներ, ակտիվների սեփականատերեր և
հաշվի գործարքների պատմություն.

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

## Անցանց նպաստներ և դատավճիռների մետատվյալներ

Անցանց արտոնությունների պատասխանները բացահայտում են մատյանների հարստացված մետատվյալները նախօրոք.
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex` և `remaining_amount` վերադարձվում են հումքի հետ միասին
ձայնագրեք, որպեսզի վահանակները ստիպված չլինեն վերծանել ներկառուցված Norito օգտակար բեռները: Նորը
հետհաշվարկի օգնականներ (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) ընդգծում է հաջորդ ավարտվող ժամկետը (թարմացնել → քաղաքականություն
→ վկայագիր), այնպես որ UI կրծքանշանները կարող են նախազգուշացնել օպերատորներին, երբ որևէ արտոնություն կա
Մնաց <24 ժամ։ SDK-ն
արտացոլում է REST ֆիլտրերը, որոնք ենթարկվում են `/v1/offline/reserve/topup`-ի կողմից.
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` և
`requireVerdict` / `onlyMissingVerdict` բուլյաններ: Անվավեր համակցություններ (համար
օրինակ `onlyMissingVerdict` + `verdictIdHex`) մերժվում են տեղայնորեն մինչև Torii
կոչվում է.

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

## Անցանց լիցքավորումներ (թողարկում + գրանցում)

Օգտագործեք լիցքավորման օգնականները, երբ ցանկանում եք վկայական տալ և անմիջապես
գրանցել այն մատյանում: SDK-ն ստուգում է տրված և գրանցված վկայականը
ID-ները համընկնում են վերադարձից առաջ, և պատասխանը ներառում է երկու ծանրաբեռնվածությունը: Կա
ոչ հատուկ լիցքավորման վերջնակետ; օգնականը շղթայում է խնդիրը + գրանցել զանգերը: Եթե
դուք արդեն ունեք ստորագրված վկայական, զանգահարեք `registerOfflineAllowance` (կամ
`renewOfflineAllowance`) ուղղակիորեն:

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

## Torii հարցումներ և հոսք (WebSockets)

Հարցման օգնականները բացահայտում են կարգավիճակը, Prometheus չափումները, հեռաչափության պատկերները և իրադարձությունները
հոսքեր՝ օգտագործելով Norito ֆիլտրի քերականությունը: Հոսքը ավտոմատ կերպով թարմացվում է
WebSockets և վերսկսվում է, երբ կրկնակի բյուջեն թույլ է տալիս:

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

Օգտագործեք `streamBlocks`, `streamTransactions` կամ `streamTelemetry` մյուսի համար
WebSocket վերջնակետեր. Բոլոր հոսքային օգնականները վերևում փորձում են նորից փորձել, այնպես որ միացրեք այն
`onReconnect` հետադարձ զանգ՝ վահանակները կերակրելու և ահազանգելու համար:

## Explorer-ի նկարներ և QR օգտակար բեռներ

Explorer հեռաչափությունը տրամադրում է տպագրված օգնականներ `/v1/explorer/metrics` և
`/v1/explorer/accounts/{account_id}/qr` վերջնակետերը, որպեսզի վահանակները կարողանան վերարտադրել այն
նույն նկարները, որոնք ապահովում են պորտալը: `getExplorerMetrics()`-ը նորմալացնում է
օգտակար բեռ և վերադարձնում է `null`, երբ երթուղին անջատված է: Զուգակցել այն
`getExplorerAccountQr()`, երբ ձեզ անհրաժեշտ է i105 (նախընտրելի)/սորա (երկրորդ լավագույն) բառացի գումարած ներդիր
SVG՝ համօգտագործման կոճակների համար:

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

`i105` անցնելը արտացոլում է Explorer-ի լռելյայն սեղմվածը
ընտրիչներ; բաց թողեք նախընտրելի i105 ելքի փոխարինումը կամ պահանջեք `i105_qr`
երբ ձեզ անհրաժեշտ է QR-անվտանգ տարբերակը: Սեղմված բառացիությունը երկրորդն է լավագույնը
Միայն Sora տարբերակ UX-ի համար: Օգնականը միշտ վերադարձնում է կանոնական նույնացուցիչը,
ընտրված բառացի և մետատվյալներ (ցանցային նախածանց, QR տարբերակ/մոդուլներ, սխալ
ուղղման մակարդակ և ներկառուցված SVG), այնպես որ CI/CD-ն կարող է հրապարակել նույն օգտակար բեռները, որոնք
Explorer-ը դուրս է գալիս առանց պատվերով փոխարկիչներ կանչելու:

## Միացնել նիստերը և հերթերը

The Connect helpers հայելին `docs/source/connect_architecture_strawman.md`: Այն
Նախադիտման համար պատրաստ նստաշրջանի ամենաարագ ճանապարհը `bootstrapConnectPreviewSession` է,
որը միավորում է դետերմինիստական SID/URI սերունդը և Torii
գրանցման զանգ.

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

- Անցեք `register: false`, երբ ձեզ անհրաժեշտ են միայն որոշիչ URI-ներ QR/deeplink-ի համար
  նախադիտումներ.
- `generateConnectSid`-ը հասանելի է մնում, երբ անհրաժեշտ է ստանալ նստաշրջանի ID-ներ
  առանց URI-ների հատման:
- Ուղղորդող ստեղները և ծածկագրված տեքստի ծրարները գալիս են հայրենի կամրջից; երբ
  անհասանելի է SDK-ն հետ է ընկնում JSON կոդեկին և նետում
  `ConnectQueueError.bridgeUnavailable`.
- Անցանց բուֆերները պահվում են որպես Norito `.to` բլբեր IndexedDB-ում: Մոնիտորինգի հերթ
  վիճակը արտանետվող `ConnectQueueError.overflow(limit)`-ի միջոցով /
  `.expired(ttlMs)` սխալներ և սնուցող `connect.queue_depth` հեռաչափություն, ինչպես նշված է
  ճանապարհային քարտեզում։

### Միացրեք ռեեստրի և քաղաքականության ակնարկները

Պլատֆորմի օպերատորները կարող են ինքնուրույն ուսումնասիրել և թարմացնել Connect ռեեստրը առանց
հեռանալով Node.js-ից: `iterateConnectApps()` էջերը ռեեստրի միջոցով, մինչդեռ
`getConnectStatus()` և `getConnectAppPolicy()` ցուցադրում են գործարկման ժամանակի հաշվիչները և
ընթացիկ քաղաքականության ծրարը: `updateConnectAppPolicy()` ընդունում է camelCase դաշտերը,
այնպես որ կարող եք բեմադրել նույն JSON ծանրաբեռնվածությունը, որն ակնկալում է Torii:

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

Միշտ նկարահանեք `getConnectStatus()` վերջին լուսանկարը, նախքան դիմելը
մուտացիաներ. կառավարման ստուգաթերթը պահանջում է ապացույցներ, որ քաղաքականության թարմացումները սկսվում են
նավատորմի ընթացիկ սահմաններից:

### Միացնել WebSocket-ի հավաքումը

`ToriiClient.openConnectWebSocket()`-ը հավաքում է կանոնականը
`/v1/connect/ws` URL (ներառյալ `sid`, `role` և նշանի պարամետրերը), բարելավումներ
`http→ws` / `https→wss` և վերջնական URL-ը հանձնում է WebSocket-ին
իրականացում, որը դուք մատակարարում եք: Բրաուզերներն ավտոմատ կերպով նորից օգտագործում են գլոբալը
`WebSocket`. Node.js զանգահարողները պետք է անցնեն այնպիսի կոնստրուկտոր, ինչպիսին է `ws`:

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

Երբ ձեզ միայն անհրաժեշտ է URL-ը, զանգահարեք `torii.buildConnectWebSocketUrl(params)` կամ
վերին մակարդակի `buildConnectWebSocketUrl(baseUrl, params)` օգնական և նորից օգտագործեք
արդյունքում տողը մաքսային տրանսպորտում/հերթում:

Փնտրու՞մ եք ամբողջական CLI-ի վրա հիմնված նմուշ: Այն
[Միացեք նախադիտման բաղադրատոմսը] (./recipes/javascript-connect-preview.md) ներառում է ա
runnable script գումարած հեռաչափության ուղեցույց, որը արտացոլում է ճանապարհային քարտեզը, որը հասանելի է
փաստաթղթավորելով Connect հերթը + WebSocket հոսքը:

### Հերթի հեռաչափություն և զգուշացում

Լարերի հերթի չափումները անմիջապես օգնական մակերեսների մեջ, որպեսզի վահանակները կարողանան արտացոլվել
ճանապարհային քարտեզի KPI-ներ:

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

`ConnectQueueError#toConnectError()`-ը փոխակերպում է հերթի ձախողումները ընդհանուրի
`ConnectError` տաքսոնոմիա, այնպես որ համօգտագործվող HTTP/WebSocket interceptors կարող են արտանետել
ստանդարտ `connect.queue_depth`, `connect.queue_overflow_total` և
`connect.queue_expired_total` չափումները, որոնք հղում են կատարում ճանապարհային քարտեզում:

## Հոսքային դիտորդներ և իրադարձությունների կուրսորներ

`ToriiClient.streamEvents()`-ը ցուցադրում է `/v1/events/sse`-ը որպես ավտոմատ կրկնվող համաժամացման
կրկնում է, այնպես որ Node/Bun CLI-ները կարող են հետևել խողովակաշարի գործունեությունը նույն կերպ, ինչպես Rust CLI-ն է անում:
Պահեք `Last-Event-ID` կուրսորը ձեր runbook արտեֆակտների կողքին, որպեսզի օպերատորները կարողանան
վերսկսել հոսքը՝ առանց իրադարձությունները բաց թողնելու, երբ գործընթացը վերսկսվում է:

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

- Անջատեք `PIPELINE_STATUS` (օրինակ՝ `Pending`, `Applied` կամ `Approved`) կամ սահմանեք
  `STREAM_FILTER_JSON`՝ CLI-ի կողմից ընդունված նույն ֆիլտրերը վերարտադրելու համար:
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs`-ը կենդանի է պահում կրկնիչը մինչև ա
  ազդանշան է ստացվել; անցեք `STREAM_MAX_EVENTS=25`, երբ ձեզ անհրաժեշտ են միայն առաջին մի քանի իրադարձությունները
  ծխի փորձարկման համար.
- `ToriiClient.streamSumeragiStatus()`-ը արտացոլում է նույն ինտերֆեյսը
  `/v1/sumeragi/status/sse`, այնպես որ կոնսենսուսային հեռաչափությունը կարող է լինել առանձին, և
  iterator-ը նույն կերպ պատվում է `Last-Event-ID`-ին:
- Տե՛ս `javascript/iroha_js/recipes/streaming.mjs` ստեղնաշարի CLI-ի համար (կուրսորի համառություն,
  env-var ֆիլտրի վերացումները և `extractPipelineStatusKind` գրանցումը) օգտագործվում է JS4-ում
  հոսքային/WebSocket ճանապարհային քարտեզի առաքում:

## UAID պորտֆելներ և տիեզերական տեղեկատու

Space Directory API-ները ներկայացնում են Համընդհանուր հաշվի ID (UAID) կյանքի ցիկլը: Այն
օգնականներն ընդունում են `uaid:<hex>` բառացի կամ չմշակված 64 վեցանկյուն մարսողություններ (LSB=1) և
կանոնականացնել դրանք նախքան հարցումներ ներկայացնելը.

- `getUaidPortfolio(uaid, { assetId })` ագրեգատները մնացորդները տվյալների տարածության համար,
  ակտիվների խմբավորումն ըստ կանոնական հաշվի ID-ների. անցեք `assetId`՝ զտելու համար
  պորտֆելը մինչև մեկ ակտիվի օրինակ:
- `getUaidBindings(uaid)`-ը թվարկում է տվյալների յուրաքանչյուր տարածք ↔ հաշիվ
  պարտադիր (`i105`-ը վերադարձնում է `i105` բառացիները):
- `getUaidManifests(uaid, { dataspaceId })`-ը վերադարձնում է յուրաքանչյուր կարողության մանիֆեստ,
  կյանքի ցիկլի կարգավիճակը և աուդիտի համար պարտադիր հաշիվները:Օպերատորի ապացույցների փաթեթների համար՝ մանիֆեստի հրապարակման/չեղարկման հոսքերը և SDK-ի միգրացիան
ուղեցույց, հետևեք Ունիվերսալ հաշվի ուղեցույցին (`docs/source/universal_accounts_guide.md`)
այս հաճախորդի օգնականների կողքին, այնպես որ պորտալի և աղբյուրի փաստաթղթերը մնան համաժամեցված:

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

Օպերատորները կարող են նաև պտտել մանիֆեստները կամ կատարել արտակարգ իրավիճակների մերժման-հաղթանակների հոսքեր առանց
թողնելով դեպի CLI: Երկու օգնականներն էլ ընդունում են կամընտիր `{ signal }` օբյեկտը
երկարաժամկետ ներկայացումները կարող են չեղարկվել `AbortController`-ի միջոցով; ոչ օբյեկտ
ընտրանքները կամ ոչ `AbortSignal` մուտքերը բարձրացնում են համաժամանակյա `TypeError` մինչև
հարցումը հարվածում է Torii:

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

`publishSpaceDirectoryManifest()` ընդունում է չմշակված մանիֆեստի JSON (համապատասխանող
հարմարանքներ `fixtures/space_directory/`) կամ որևէ առարկա, որը սերիական է
նույն կառուցվածքը։ `privateKey`, `privateKeyHex` կամ `privateKeyMultihash` քարտեզ
`ExposedPrivateKey` դաշտը Torii ակնկալում է և կանխադրված է `ed25519`
ալգորիթմ, երբ նախածանց չի տրվում: Երկու հարցումներն էլ վերադառնում են Torii հերթագրվելուց հետո
հրահանգը (`202 Accepted`), որի ժամանակ մատյանը կթողարկի
համապատասխանող `SpaceDirectoryEvent`:

## Կառավարում և ISO կամուրջ

`ToriiClient`-ը բացահայտում է կառավարման API-ները՝ պայմանագրերը ստուգելու, բեմադրելու համար
առաջարկություններ, քվեաթերթիկներ ներկայացնելը (պարզ կամ ԶԿ), խորհրդի ռոտացիա և հրավիրում
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` առանց ձեռագիր DTO-ների: ISO 20022 օգնականներ
հետևեք նույն օրինակին `buildPacs008Message`/`buildPacs009Message`-ի միջոցով և
`submitIso*`/`waitForIsoMessageStatus` եռյակ.

Տես [կառավարման և ISO կամուրջի բաղադրատոմսը] (./recipes/javascript-governance-iso.md)
CLI-ին պատրաստ նմուշների համար, գումարած ցուցիչներ՝ վերադառնալ դեպի ամբողջական դաշտային ուղեցույց
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC նմուշառման և առաքման ապացույցներ

JS ճանապարհային քարտեզը պահանջում է նաև Roadrunner Block Commitment (RBC) նմուշառում, որպեսզի օպերատորները կարողանան
ապացուցել, որ բլոկը, որը նրանք վերցրել են Sumeragi-ի միջոցով, համընկնում է նրանց կողմից հաստատված ապացույցների հետ:
Ձեռքով օգտակար բեռներ կառուցելու փոխարեն օգտագործեք ներկառուցված օգնականները.

1. `getSumeragiRbcSessions()` հայելիներ `/v1/sumeragi/rbc/sessions` և
   `findRbcSamplingCandidate()`-ն ավտոմատ ընտրում է առաջին առաքված նիստը բլոկային հեշով
   (ինտեգրման փաթեթը հետ է ընկնում դրան, երբ
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` կարգավորված չէ):
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` նորմալացնում է `{blockHash,height,view}`
   գումարած կամընտիր `{count,seed,apiToken}`-ը երբեք չի վերացնում այդքան սխալ ձևավորված վեցանկյուն կամ բացասական ամբողջ թվերը
   հասնել Torii:
3. `sampleRbcChunks()`-ը փակցնում է հարցումը `/v1/sumeragi/rbc/sample`-ին՝ վերադարձնելով ամբողջական ապացույցները
   և Մերկլի ուղիները (`samples[].chunkHex`, `chunkRoot`, `payloadHash`), որոնցով պետք է արխիվացնեք
   մնացած ձեր որդեգրման ապացույցները:
4. `getSumeragiRbcDelivered(height, view)`-ը գրավում է խմբի առաքման մետատվյալները, որպեսզի աուդիտորները
   կարող է վերարտադրել ապացույցը ծայրից ծայր:

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

Պահպանեք երկու պատասխաններն էլ արտեֆակտ արմատի ներքո, որը դուք ներկայացնում եք կառավարմանը: Անտեսել
ավտոմատ ընտրված նստաշրջան `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`-ի միջոցով
երբ ձեզ անհրաժեշտ է հետազոտել կոնկրետ բլոկ և վերաբերվել RBC-ի լուսանկարները ստանալու ձախողումներին որպես
թռիչքից առաջ դարպասի սխալ, այլ ոչ թե լուռ իջեցում ուղիղ ռեժիմի:

## Փորձարկում և CI

1. Քեշ բեռներ և npm արտեֆակտներ:
2. Գործարկել `npm run build:native`:
3. Կատարեք `npm test` (կամ `node --test` ծխի աշխատանքների համար):

GitHub Actions-ի տեղեկանքը գործում է
`docs/source/examples/iroha_js_ci.md`.

## Հաջորդ քայլերը

- Վերանայեք ստեղծված տեսակները `javascript/iroha_js/index.d.ts`-ում:
- Բացահայտեք `javascript/iroha_js/recipes/`-ի բաղադրատոմսերը:
- Զուգավորեք `ToriiClient`-ը Norito արագ մեկնարկի հետ՝ կողքին օգտակար բեռները ստուգելու համար
  SDK զանգեր.