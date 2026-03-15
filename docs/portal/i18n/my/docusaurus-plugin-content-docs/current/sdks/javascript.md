---
slug: /sdks/javascript
lang: my
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`@iroha/iroha-js` သည် Torii နှင့် အပြန်အလှန်တုံ့ပြန်ရန်အတွက် canonical Node.js ပက်ကေ့ဂျ်ဖြစ်သည်။ အဲဒါ
Norito အစုအဝေးများ၊ Ed25519 အထောက်အကူများ၊ pagination utilities နှင့် ခံနိုင်ရည်ရှိသော
TypeScript မှ CLI စီးဆင်းမှုများကို သင်ထင်ဟပ်နိုင်စေရန် HTTP/WebSocket သုံးစွဲသူ။

## တပ်ဆင်ခြင်း။

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

တည်ဆောက်ပုံ အဆင့်သည် `cargo build -p iroha_js_host` ဖြစ်သည်။ toolchain မှသေချာပါစေ။
`rust-toolchain.toml` သည် `npm run build:native` ကို အသုံးမပြုမီ စက်တွင်း၌ ရနိုင်ပါသည်။

## အဓိကစီမံခန့်ခွဲမှု

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

## အရောင်းအဝယ်လုပ်ပါ။

Norito ညွှန်ကြားချက်များကို တည်ဆောက်သူများသည် ခွဲခြားသတ်မှတ်မှုများ၊ မက်တာဒေတာနှင့် ပမာဏများကို ပုံမှန်ဖြစ်စေသည်
ကုဒ်လုပ်ထားသော ငွေပေးငွေယူများသည် Rust/CLI payload များနှင့် ကိုက်ညီပါသည်။

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
  destinationAccountId: "i105...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "i105...",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "i105...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Torii လိုင်းထည့်သွင်းမှု

`ToriiClient` သည် `iroha_config` ကိုထင်ဟပ်ထားသည့် ထပ်စမ်းခြင်း/အချိန်လွန်ခလုတ်များကို လက်ခံသည်။ သုံးပါ။
`resolveToriiClientConfig` ကို camelCase config အရာဝတ္ထုကို ပေါင်းစည်းရန် (ပုံမှန်လုပ်ပါ
ပထမဆုံး `iroha_config`) env overrides နှင့် inline ရွေးစရာများ။

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

Local dev အတွက် Environment variables များ

| ပြောင်းလဲနိုင်သော | ရည်ရွယ်ချက် |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | အချိန်ကုန် (မီလီစက္ကန့်များ) တောင်းဆိုပါ။ |
| `IROHA_TORII_MAX_RETRIES` | အမြင့်ဆုံး ထပ်ကြိုးစားပါ။ |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | အစပိုင်း ပြန်စမ်းကြည့်တော့ backoff |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Exponential backoff မြှောက်ကိန်း။ |
| `IROHA_TORII_MAX_BACKOFF_MS` | အများဆုံး ပြန်ကြိုးစားရန် နှောင့်နှေးခြင်း။ |
| `IROHA_TORII_RETRY_STATUSES` | ထပ်စမ်းကြည့်ရန် ကော်မာ-ခြားထားသော HTTP အခြေအနေကုဒ်များ။ |
| `IROHA_TORII_RETRY_METHODS` | ထပ်စမ်းကြည့်ရန် ကော်မာ-ခြားထားသော HTTP နည်းလမ်းများ။ |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` ကိုထည့်သည်။ |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` ခေါင်းစီးကို ထည့်သည်။ |

ထပ်စမ်းကြည့်ပါ ပရိုဖိုင်များသည် Android ပုံသေများကို ထင်ဟပ်စေပြီး တူညီမှုစစ်ဆေးမှုများအတွက် တင်ပို့သည်-
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`၊
`DEFAULT_RETRY_PROFILE_STREAMING`။ `docs/source/sdk/js/torii_retry_policy.md` ကိုကြည့်ပါ။
အဆုံးမှတ်မှ ပရိုဖိုင်မြေပုံဆွဲခြင်းနှင့် ဘောင်များအတွင်း အုပ်ချုပ်မှုစာရင်းစစ်ခြင်းအတွက်
JS4/JS7။

## သာဓုခေါ်နိုင်သောစာရင်းများနှင့် စာမျက်နှာခွဲခြင်း။

Pagination helpers များသည် `/v1/accounts` အတွက် Python SDK ergonomics ကို ထင်ဟပ်စေသည်
`/v1/domains`၊ `/v1/assets/definitions`၊ NFTs၊ လက်ကျန်များ၊ ပစ္စည်းကိုင်ဆောင်ထားသူများနှင့်
အကောင့်အရောင်းအ၀ယ်မှတ်တမ်း။

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
const balances = await torii.listAccountAssets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("rose#wonderland", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## အော့ဖ်လိုင်းထောက်ပံ့ကြေးများနှင့် စီရင်ချက်မက်တာဒေတာ

အော့ဖ်လိုင်း ထောက်ပံ့ကြေး တုံ့ပြန်မှုများသည် ကြွယ်ဝသော လယ်ဂျာ မက်တာဒေတာကို ရှေ့တွင် ထုတ်ဖော်သည် —
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`၊
`attestation_nonce_hex` နှင့် `remaining_amount` ကို ကုန်ကြမ်းနှင့်အတူ ပြန်ပေးသည်
မှတ်တမ်းတင်ထားသောကြောင့် ဒက်ရှ်ဘုတ်များသည် မြှုပ်သွင်းထားသော Norito payloads ကို ကုဒ်ကုဒ်လုပ်ရန် မလိုအပ်ပါ။ အသစ်
နှစ်သစ်အတွက် ကူညီသူများ (`deadline_kind`၊ `deadline_state`၊ `deadline_ms`၊
`deadline_ms_remaining`) နောက်သက်တမ်းကုန်ဆုံးမည့် နောက်ဆုံးရက်ကို မီးမောင်းထိုးပြပါ (ပြန်လည်စတင်ခြင်း → မူဝါဒ
→ လက်မှတ်) ထို့ကြောင့် UI တံဆိပ်များသည် ထောက်ပံ့ကြေးရရှိသည့်အခါတိုင်း အော်ပရေတာများအား သတိပေးနိုင်သည်။
<24 နာရီ ကျန်သေးတယ်။ SDK
`/v1/offline/allowances` မှ ဖော်ထုတ်ထားသော REST စစ်ထုတ်မှုများကို မှန်ကြည့်သည်-
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`၊
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs` ရယ်၊
`requireVerdict` / `onlyMissingVerdict` ဘူလီယံ။ မမှန်ကန်သော ပေါင်းစပ်မှုများ (အတွက်
ဥပမာ `onlyMissingVerdict` + `verdictIdHex`) သည် Torii မတိုင်မီ ပြည်တွင်း၌ ငြင်းပယ်ခံရသည်
ဟုခေါ်သည်။

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

## အော့ဖ်လိုင်းငွေဖြည့်ခြင်း (ပြဿနာ + မှတ်ပုံတင်ခြင်း)

လက်မှတ်ကိုချက်ချင်းထုတ်ချင်တဲ့အခါ ငွေဖြည့်အကူတွေကို သုံးပါ။
လယ်ဂျာတွင် စာရင်းသွင်းပါ။ SDK သည် ထုတ်ပေးပြီး မှတ်ပုံတင်ထားသော လက်မှတ်ကို စစ်ဆေးသည်။
မပြန်မီ ID များသည် တူညီပြီး တုံ့ပြန်မှုတွင် payload နှစ်ခုလုံး ပါဝင်ပါသည်။ ရှိတယ်။
သီးသန့်ငွေဖြည့်ပေးသည့် အဆုံးမှတ်မရှိပါ။ အကူအညီပေးသူက ပြဿနာကို ဆွဲကြိုးချ + ဖုန်းခေါ်ဆိုမှုများကို စာရင်းသွင်းပါ။ အကယ်လို့
သင့်တွင် လက်မှတ်ရေးထိုးထားသော လက်မှတ်တစ်ခုရှိပြီး၊ `registerOfflineAllowance` (သို့မဟုတ်) ခေါ်ဆိုပါ။
`renewOfflineAllowance`) တိုက်ရိုက်။

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

## Torii မေးမြန်းချက်များနှင့် တိုက်ရိုက်ကြည့်ရှုခြင်း (WebSockets)

Query helpers သည် အခြေအနေ၊ Prometheus မက်ထရစ်များ၊ တယ်လီမီတာ လျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် ဖြစ်ရပ်ကို ဖော်ထုတ်ပြသသည်
Norito စစ်ထုတ်ခြင်းသဒ္ဒါကို အသုံးပြု၍ ထုတ်လွှင့်သည်။ တိုက်ရိုက်လွှင့်ခြင်းသို့ အလိုအလျောက် အဆင့်မြှင့်ပေးသည်။
ထပ်စမ်းကြည့်ရန် ဘတ်ဂျက်ခွင့်ပြုသောအခါတွင် WebSocket များနှင့် ပြန်လည်စတင်သည်။

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

အခြားတစ်ခုအတွက် `streamBlocks`၊ `streamTransactions` သို့မဟုတ် `streamTelemetry` ကို အသုံးပြုပါ။
WebSocket အဆုံးမှတ်များ။ တိုက်ရိုက်ကြည့်ရှုခြင်းဆိုင်ရာ အကူအညီပေးသူများအားလုံးသည် ထပ်စမ်းကြည့်ပါက ကြိုးပမ်းမှုများ ပေါ်လာသောကြောင့် ချိတ်လိုက်ပါ။
ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များအတွက် `onReconnect` ပြန်ခေါ်ပါ။

## Explorer လျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် QR ငွေပေးချေမှုများ

Explorer telemetry သည် `/v1/explorer/metrics` နှင့် အတွက် ရိုက်နှိပ်ထားသော အကူအညီများကို ပေးပါသည်။
`/v1/explorer/accounts/{account_id}/qr` သည် အဆုံးမှတ်များဖြစ်သောကြောင့် ဒက်ရှ်ဘုတ်များသည် ၎င်းကို ပြန်လည်ဖွင့်နိုင်သည်။
ပေါ်တယ်ကို စွမ်းအားပေးသော လျှပ်တစ်ပြက်ပုံများ။ `getExplorerMetrics()` သည် ပုံမှန်ဖြစ်စေသည်။
လမ်းကြောင်းကိုပိတ်ထားသောအခါ payload နှင့် `null` ကိုပြန်ပေးသည်။ ၎င်းကိုတွဲပါ။
`getExplorerAccountQr()` I105 (ဦးစားပေး)/sora (ဒုတိယအကောင်းဆုံး) စာလုံးများ နှင့် inline လိုအပ်သည့်အခါတိုင်း
မျှဝေခလုတ်များအတွက် SVG

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

const qr = await torii.getExplorerAccountQr("i105...");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

`I105` ကို ဖြတ်သွားခြင်းသည် Explorer ၏ ပုံသေချုံ့ထားသော ပုံဖြစ်သည်။
ရွေးချယ်သူများ; နှစ်သက်သော I105 အထွက်အတွက် အစားထိုးခြင်း သို့မဟုတ် `i105_qr` တောင်းဆိုခြင်း
QR-safe ဗားရှင်းကို သင်လိုအပ်သောအခါ။ compressed literal သည် ဒုတိယအကောင်းဆုံးဖြစ်သည်။
UX အတွက် Sora-သီးသန့် ရွေးချယ်မှု။ ကူညီသူသည် ကျမ်းဂန်အမှတ်အသားကို အမြဲတမ်း ပြန်ပေးသည်၊
ရွေးချယ်ထားသော ပကတိနှင့် မက်တာဒေတာ (ကွန်ရက်ရှေ့ဆက်၊ QR ဗားရှင်း/မော်ဂျူးများ၊ အမှား
အမှားပြင်ဆင်ခြင်းအဆင့်နှင့် SVG လိုင်း)၊ ထို့ကြောင့် CI/CD သည် ထိုတူညီသော payload များကို ထုတ်ဝေနိုင်သည်။
စိတ်ကြိုက် converters မခေါ်ဘဲ Explorer သည် မျက်နှာပြင်များ။

## ဆက်ရှင်များနှင့် တန်းစီခြင်းကို ချိတ်ဆက်ပါ။

Connect helpers သည် `docs/source/connect_architecture_strawman.md` ကို မှန်ပါသည်။ ဟိ
အစမ်းကြည့်ရှုရန် အဆင်သင့်ရှိသော စက်ရှင်ဆီသို့ အမြန်ဆုံးလမ်းကြောင်းမှာ `bootstrapConnectPreviewSession`၊
အဆုံးအဖြတ်ပေးသော SID/URI မျိုးဆက်နှင့် Torii တို့ကို ပေါင်းစပ်ချုပ်လုပ်ထားသည့်
မှတ်ပုံတင်ခေါ်ဆိုခြင်း။

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

- QR/deeplink အတွက် အဆုံးအဖြတ်ပေးသော URI များကိုသာ လိုအပ်သောအခါတွင် `register: false` ကို ကျော်ဖြတ်ပါ။
  အစမ်းကြည့်ရှုမှုများ
- သင် session ids များကိုရယူရန်လိုအပ်သောအခါ `generateConnectSid` သည်ဆက်လက်ရှိနေသည်
  URIs များကို မသုံးဘဲ
- လမ်းညွှန်သော့များနှင့် စာဝှက်စာသား စာအိတ်များသည် ဇာတိတံတားမှ လာပါသည်။ ဘယ်အချိန်မှာ
  SDK မရရှိနိုင်ပါက JSON codec သို့ ပြန်ကျသွားပြီး ပစ်ချပါ။
  `ConnectQueueError.bridgeUnavailable`။
- အော့ဖ်လိုင်းကြားခံများကို IndexedDB တွင် Norito `.to` blobs အဖြစ် သိမ်းဆည်းထားသည်။ တန်းစီစောင့်ကြပ်ပါ။
  ထုတ်လွှတ်သော `ConnectQueueError.overflow(limit)`/ မှတစ်ဆင့် ပြည်နယ်၊
  `.expired(ttlMs)` အမှားအယွင်းများနှင့် `connect.queue_depth` တယ်လီမီတာကို ဖော်ပြထားသည့်အတိုင်း ဖြည့်စွက်ပါ
  လမ်းပြမြေပုံထဲမှာ။

### မှတ်ပုံတင်ခြင်းနှင့် မူဝါဒလျှပ်တစ်ပြက်များကို ချိတ်ဆက်ပါ။

ပလပ်ဖောင်းအော်ပရေတာများသည် Connect registry ကို စူးစမ်းလေ့လာခြင်းမရှိဘဲ အပ်ဒိတ်လုပ်နိုင်ပါသည်။
Node.js ကို ချန်ထားပါ။ `iterateConnectApps()` စာမျက်နှာများကို registry မှတဆင့်၊
`getConnectStatus()` နှင့် `getConnectAppPolicy()` သည် runtime ကောင်တာများကိုဖော်ထုတ်ရန်နှင့်
လက်ရှိမူဝါဒစာအိတ်။ `updateConnectAppPolicy()` သည် camelCase အကွက်များကို လက်ခံသည်၊
ထို့ကြောင့် သင်သည် Torii မျှော်လင့်ထားသည့် တူညီသော JSON payload ကို အဆင့်သတ်မှတ်နိုင်သည်။

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

လျှောက်ထားခြင်းမပြုမီ နောက်ဆုံးပေါ် `getConnectStatus()` လျှပ်တစ်ပြက်ဓာတ်ပုံကို အမြဲဖမ်းယူပါ။
ပြောင်းလဲမှုများ— အုပ်ချုပ်ရေးစစ်ဆေးမှုစာရင်းတွင် မူဝါဒမွမ်းမံမှုများစတင်ကြောင်း အထောက်အထားများ လိုအပ်သည်။
ရေယာဉ်၏လက်ရှိကန့်သတ်ချက်များမှ။

### WebSocket ခေါ်ဆိုခြင်းကို ချိတ်ဆက်ပါ။

`ToriiClient.openConnectWebSocket()` သည် canonical ကို စုစည်းသည်။
`/v1/connect/ws` URL (`sid`၊ `role`၊ နှင့် တိုကင်ဘောင်များ အပါအဝင်) အဆင့်မြှင့်တင်မှုများ၊
`http→ws` / `https→wss` နှင့် WebSocket မှ နောက်ဆုံး URL ကို ပေးသည်။
သင်ထောက်ပံ့သောအကောင်အထည်ဖော်မှု။ ဘရောက်ဆာများသည် ဂလိုဘယ်ကို အလိုအလျောက် ပြန်သုံးသည်။
`WebSocket`။ Node.js ခေါ်ဆိုသူများသည် `ws` ကဲ့သို့သော constructor ကိုဖြတ်သန်းသင့်သည်-

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

URL ကိုသာလိုအပ်သောအခါ၊ `torii.buildConnectWebSocketUrl(params)` သို့မဟုတ် အဆိုပါကိုခေါ်ဆိုပါ။
ထိပ်တန်းအဆင့် `buildConnectWebSocketUrl(baseUrl, params)` အကူအညီပေးသူနှင့် ပြန်လည်အသုံးပြုပါ။
စိတ်ကြိုက်သယ်ယူပို့ဆောင်ရေး/တန်းစီမှုတွင် ရလဒ်ထွက်ရှိသောစာကြောင်း။

ပြီးပြည့်စုံသော CLI-အသားပေးနမူနာကို ရှာဖွေနေပါသလား။ ဟိ
[ချိတ်ဆက်အကြိုကြည့်ခြင်း စာရွက်](./recipes/javascript-connect-preview.md) တစ်ခု ပါဝင်သည်။
runnable script နှင့် telemetry လမ်းညွှန်များအတွက် ပေးပို့နိုင်သော လမ်းပြမြေပုံကို ထင်ဟပ်စေပါသည်။
ချိတ်ဆက်မှုတန်းစီ + WebSocket စီးဆင်းမှုကို မှတ်တမ်းတင်ခြင်း။

### တယ်လီမီတာနှင့် သတိပေးချက်

ဒက်ရှ်ဘုတ်များသည် မှန်ကြည့်နိုင်စေရန် အကူအညီပေးသည့် မျက်နှာပြင်များအတွင်းသို့ ဝိုင်ယာတန်းစီတိုင်းတာမှုများကို တိုက်ရိုက်လုပ်ဆောင်သည်။
လမ်းပြမြေပုံ KPIs။

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

`ConnectQueueError#toConnectError()` သည် တန်းစီခြင်းပျက်ကွက်မှုများကို ယေဘူယျအဖြစ်သို့ ပြောင်းလဲပေးသည်။
`ConnectError` အမျိုးအစားခွဲခြားသတ်မှတ်ထားသောကြောင့် မျှဝေထားသော HTTP/WebSocket ကြားဖြတ်ကိရိယာများမှ ထုတ်လွှတ်နိုင်သည်
စံသတ်မှတ်ချက် `connect.queue_depth`၊ `connect.queue_overflow_total` နှင့်
လမ်းပြမြေပုံတစ်လျှောက် ကိုးကားထားသော `connect.queue_expired_total` မက်ထရစ်များ။

## ထုတ်လွှင့်ကြည့်ရှုသူများနှင့် ပွဲချိန်ကာများ

`ToriiClient.streamEvents()` သည် `/v1/events/sse` ကို အလိုအလျောက် async iterator အဖြစ် ထုတ်ပြသည်
ပြန်စမ်းကြည့်ပါ၊ ထို့ကြောင့် Node/Bun CLI များသည် Rust CLI ကဲ့သို့ပင် ပိုက်လိုင်းလှုပ်ရှားမှုကို နောက်ဆုတ်နိုင်ပါသည်။
အော်ပရေတာများလုပ်နိုင်စေရန်အတွက် သင်၏ runbook artefact များနှင့်အတူ `Last-Event-ID` cursor ကို ဆက်ထားပါ
လုပ်ငန်းစဉ်ပြန်လည်စတင်သည့်အခါ အစီအစဉ်များကို မကျော်ဘဲ ထုတ်လွှင့်မှုကို ပြန်လည်စတင်ပါ။

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

- `PIPELINE_STATUS` (ဥပမာ `Pending`၊ `Applied`၊ သို့မဟုတ် `Approved`) သို့မဟုတ် သတ်မှတ်
  CLI လက်ခံသော တူညီသော စစ်ထုတ်မှုများကို ပြန်ဖွင့်ရန် `STREAM_FILTER_JSON`။
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` သည် iterator ကို အသက်ဝင်သည်အထိ ထိန်းပေးသည်။
  signal ကိုလက်ခံရရှိသည်; ပထမအကြိမ်ဖြစ်ရပ်အနည်းငယ်သာလိုအပ်သောအခါ `STREAM_MAX_EVENTS=25` ကိုကျော်ဖြတ်ပါ။
  မီးခိုးစမ်းသပ်မှုတစ်ခုအတွက်။
- `ToriiClient.streamSumeragiStatus()` သည် တူညီသော အင်တာဖေ့စ်ကို ကြည့်သည်။
  `/v1/sumeragi/status/sse` ထို့ကြောင့် အများသဘောတူသော တယ်လီမီတာကို သီးခြားစီ အမြီးပိုင်းထားနိုင်ပြီး၊
  iterator သည် `Last-Event-ID` ကို ထိုနည်းအတိုင်း ဂုဏ်ပြုပါသည်။
- turnkey CLI အတွက် `javascript/iroha_js/recipes/streaming.mjs` ကိုကြည့်ပါ (ကာဆာဆက်မြဲမှု၊
  JS4 တွင်အသုံးပြုသော env-var filter နှင့် `extractPipelineStatusKind` မှတ်တမ်း)
  တိုက်ရိုက်ထုတ်လွှင့်ခြင်း/WebSocket လမ်းပြမြေပုံကို ပေးအပ်နိုင်မည်ဖြစ်သည်။

## UAID အစုစုနှင့် အာကာသလမ်းညွှန်

Space Directory APIs များသည် Universal Account ID (UAID) lifecycle ကို ဖော်ပြသည်။ ဟိ
အကူအညီပေးသူများသည် `uaid:<hex>` literals သို့မဟုတ် 64-hex အကြမ်းစားများ (LSB=1) ကို လက်ခံပြီး
တောင်းဆိုချက်များကိုမတင်ပြမီ ၎င်းတို့ကို canonicalise လုပ်ပါ-

- `getUaidPortfolio(uaid, { assetId })` သည် dataspace တစ်ခုလျှင် လက်ကျန်များကို စုစည်းသည်၊
  Canonical အကောင့် ID များဖြင့် ပိုင်ဆိုင်မှုပိုင်ဆိုင်မှုများကို အုပ်စုဖွဲ့ခြင်း၊ စစ်ထုတ်ရန် `assetId` ကို ကျော်ဖြတ်ပါ။
  အစုစုသည် တစ်ခုတည်းသော ပိုင်ဆိုင်မှု သာဓကသို့ ဆင်းသက်သည်။
- `getUaidBindings(uaid)` သည် dataspace ↔ အကောင့်တိုင်းကို ရေတွက်သည်။
  စည်းနှောင်ခြင်း (`I105` သည် `i105` စာလုံးများကို ပြန်ပေးသည်)။
- `getUaidManifests(uaid, { dataspaceId })` သည် လုပ်ဆောင်နိုင်စွမ်းတစ်ခုစီကို ထင်ရှားစွာပြသည်၊
  ဘဝသံသရာအခြေအနေနှင့် စာရင်းစစ်များအတွက် ချည်နှောင်ထားသော အကောင့်များ။အော်ပရေတာ အထောက်အထားထုပ်များ အတွက်၊ ထုတ်ဝေခြင်း/ပြန်လည်ရုပ်သိမ်းခြင်း စီးဆင်းမှုများကို ထင်ရှားစေပြီး SDK ပြောင်းရွှေ့ခြင်း။
လမ်းညွှန်ချက်၊ Universal အကောင့်လမ်းညွှန် (`docs/source/universal_accounts_guide.md`) ကို လိုက်နာပါ
ဤကလိုင်းယင့်အကူအညီပေးသူများနှင့်အတူ ပေါ်တယ်နှင့် ရင်းမြစ်စာရွက်စာတမ်းများ တစ်ပြိုင်တည်းရှိနေပါသည်။

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

အော်ပရေတာများသည် မန်နီးဖက်စ်များကို လှည့်ခြင်း သို့မဟုတ် အရေးပေါ် ငြင်းဆိုခြင်းများကို မလိုအပ်ဘဲ လုပ်ဆောင်နိုင်သည်။
CLI သို့ကျဆင်းသွားသည်။ အကူအညီပေးသူနှစ်ဦးစလုံးသည် ရွေးချယ်နိုင်သော `{ signal }` အရာဝတ္ထုတစ်ခုကို လက်ခံပါသည်။
ရေရှည်တင်ပြမှုများကို `AbortController` ဖြင့် ပယ်ဖျက်နိုင်သည်။ အရာဝတ္ထုမဟုတ်သော
ရွေးချယ်စရာများ သို့မဟုတ် `AbortSignal` မဟုတ်သော သွင်းအားစုများသည် တူညီသော `TypeError` ကို မြှင့်တင်ခြင်းမပြုမီ၊
တောင်းဆိုချက် Torii ထိသွားသည်-

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "i105...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "i105...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` သည် အကြမ်းဖျင်းဖော်ပြချက် JSON ကို လက်ခံသည် (၎င်းနှင့် ကိုက်ညီသော
`fixtures/space_directory/`) အောက်တွင် တပ်ဆင်ထားသော ပစ္စည်းများ သို့မဟုတ် တွဲဆက်ထားသည့် မည်သည့်အရာ၊
တူညီသောဖွဲ့စည်းပုံ။ `privateKey`၊ `privateKeyHex` သို့မဟုတ် `privateKeyMultihash` သို့မြေပုံ
`ExposedPrivateKey` အကွက် Torii သည် `ed25519` သို့ ပုံသေမျှော်လင့်ထားသည်
ရှေ့ဆက်မပေးရသောအခါ algorithm။ တောင်းဆိုချက်နှစ်ခုလုံးသည် Torii ၏စာရင်းများကို တစ်ကြိမ်ပြန်ပေးသည်။
ညွှန်ကြားချက် (`202 Accepted`)၊ ထိုအချိန်တွင် လယ်ဂျာမှ ထုတ်လွှတ်သော၊
`SpaceDirectoryEvent` နှင့် ကိုက်ညီသည်။

## အုပ်ချုပ်ရေး & ISO တံတား

`ToriiClient` သည် စာချုပ်များကို စစ်ဆေးခြင်း၊ အဆင့်သတ်မှတ်ခြင်းအတွက် အုပ်ချုပ်မှု API များကို ဖော်ထုတ်သည်
အဆိုပြုချက်များ၊ မဲများတင်သွင်းခြင်း (ရိုးရိုး သို့မဟုတ် ZK)၊ ကောင်စီကို အလှည့်ကျ ခေါ်ဆိုခြင်း၊
`governanceFinalizeReferendumTyped` /
လက်ဖြင့်ရေးထားသော DTO များမပါဘဲ `governanceEnactProposalTyped`။ ISO 20022 အကူအညီပေးသူများ
တူညီသောပုံစံအတိုင်း `buildPacs008Message`/`buildPacs009Message` နှင့်
`submitIso*`/`waitForIsoMessageStatus` ဒန်း။

[အုပ်ချုပ်မှုနှင့် ISO တံတားစာရွက်](./recipes/javascript-governance-iso.md) ကိုကြည့်ပါ
CLI-အဆင်သင့်နမူနာများအတွက် ညွှန်ပြချက်များ အပြည့်အစုံကို အကွက်လမ်းညွှန်ထဲတွင် ပြန်ထည့်ပါ။
`docs/source/sdk/js/governance_iso_examples.md`။

## RBC နမူနာယူခြင်းနှင့် ပေးပို့ခြင်းအထောက်အထား

JS လမ်းပြမြေပုံသည် အော်ပရေတာများ လုပ်ဆောင်နိုင်စေရန် Roadrunner Block Commitment (RBC) နမူနာကို လိုအပ်ပါသည်။
Sumeragi မှတစ်ဆင့် ၎င်းတို့ရယူခဲ့သည့် ဘလောက်သည် ၎င်းတို့စိစစ်ထားသော အတုံးအခဲများနှင့် ကိုက်ညီကြောင်း သက်သေပြပါ။
ဝန်တင်များကို လက်ဖြင့်တည်ဆောက်မည့်အစား built-in helpers ကိုသုံးပါ-

1. `getSumeragiRbcSessions()` မှန်များ `/v1/sumeragi/rbc/sessions` ကိုလည်းကောင်း၊
   `findRbcSamplingCandidate()` သည် block hash ဖြင့် ပထမဆုံးပေးပို့သော စက်ရှင်ကို အလိုအလျောက်ရွေးချယ်သည်
   (ပေါင်းစည်းမှုအစုံသည် အချိန်တိုင်းတွင် ၎င်းထံသို့ ပြန်ကျသည်။
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` ကို သတ်မှတ်မထားပါ။
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` သည် `{blockHash,height,view}` ကို ပုံမှန်ဖြစ်စေသည်
   ထို့အပြင် ရွေးချယ်နိုင်သော `{count,seed,apiToken}` သည် ပုံစံမမှန်သော hex သို့မဟုတ် အနုတ်ကိန်းများကို ဘယ်သောအခါမှ အစားထိုးသည်
   Torii သို့ရောက်ရှိ။
3. `sampleRbcChunks()` သည် `/v1/sumeragi/rbc/sample` သို့ တောင်းဆိုချက်ကို ပို့စ်တင်သည်၊ အတုံးအခဲအထောက်အထားများကို ပြန်ပေးသည်
   နှင့် Merkle လမ်းကြောင်းများ (`samples[].chunkHex`၊ `chunkRoot`၊ `payloadHash`) ဖြင့် သိမ်းဆည်းသင့်သည်
   သင်၏မွေးစားခြင်းဆိုင်ရာ အထောက်အထားများ ကျန်ပါသည်။
4. `getSumeragiRbcDelivered(height, view)` သည် အဖွဲ့ခွဲ၏ပေးပို့မှု မက်တာဒေတာကို ဖမ်းယူထားသောကြောင့် စာရင်းစစ်များ၊
   သက်သေကို အစမှအဆုံး ပြန်ဖွင့်နိုင်သည်။

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

သင်အုပ်ချုပ်မှုသို့တင်ပြသည့် artefact root အောက်တွင် တုံ့ပြန်မှုနှစ်ခုစလုံးကို ဆက်လက်လုပ်ဆောင်ပါ။ ပဓာန
`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` မှတစ်ဆင့် အလိုအလျောက်ရွေးချယ်ထားသော စက်ရှင်
သတ်မှတ်ထားသော ပိတ်ဆို့ခြင်းကို စစ်ဆေးရန် လိုအပ်သည့်အခါတိုင်း၊ RBC လျှပ်တစ်ပြက်ရိုက်ချက်များကို ရယူရန် ပျက်ကွက်မှုများကို ဆက်ဆံပါ။
တိုက်ရိုက်မုဒ်သို့ တိတ်တဆိတ် အဆင့်နှိမ့်မည့်အစား လေယာဉ်အကြိုဂိတ်ပေါက် အမှားအယွင်း။

## စမ်းသပ်ခြင်းနှင့် CI

1. ကုန်တင်ကုန်ချနှင့် npm ရှေးဟောင်းပစ္စည်းများကို ကက်ရှ်လုပ်ပါ။
2. `npm run build:native` ကိုဖွင့်ပါ။
3. မီးခိုးအလုပ်များအတွက် `npm test` (သို့မဟုတ် `node --test`) ကို လုပ်ဆောင်ပါ။

ရည်ညွှန်း GitHub လုပ်ဆောင်ချက်များ အလုပ်အသွားအလာတွင် နေထိုင်ပါသည်။
`docs/source/examples/iroha_js_ci.md`။

## နောက်တစ်ဆင့်

- `javascript/iroha_js/index.d.ts` တွင် ထုတ်လုပ်ထားသော အမျိုးအစားများကို ပြန်လည်သုံးသပ်ပါ။
- `javascript/iroha_js/recipes/` အောက်တွင် ချက်ပြုတ်နည်းများကို စူးစမ်းပါ။
- `ToriiClient` ကို Norito နှင့်တွဲပြီး payloads စစ်ဆေးရန် အမြန်စတင်ပါ။
  SDK ခေါ်ဆိုမှုများ။