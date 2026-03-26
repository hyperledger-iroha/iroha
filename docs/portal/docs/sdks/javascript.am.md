---
lang: am
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

`@iroha/iroha-js` ከ Torii ጋር ለመገናኘት ቀኖናዊው Node.js ጥቅል ነው። እሱ
ጥቅሎች Norito ግንበኞች፣ Ed25519 ረዳቶች፣ የገጽታ ግንባታ መገልገያዎች እና ተከላካይ
የCLI ፍሰቶችን ከTyScript ን ማንጸባረቅ እንድትችል HTTP/WebSocket ደንበኛ።

## መጫን

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

የግንባታ ደረጃ `cargo build -p iroha_js_host` ይጠቀልላል። የመሳሪያውን ሰንሰለት ከ
`rust-toolchain.toml` I18NI0000040X ከመሮጥዎ በፊት በአገር ውስጥ ይገኛል።

## ቁልፍ አስተዳደር

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

## ግብይቶችን ይገንቡ

Norito መመሪያ ገንቢዎች መለያዎችን፣ ዲበ ዳታ እና መጠኖችን መደበኛ ያደርጋሉ።
ኮድ የተደረገባቸው ግብይቶች ከ Rust/CLI ጭነት ጋር ይዛመዳሉ።

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

## Torii ደንበኛ ውቅር

`ToriiClient` `iroha_config` የሚያንፀባርቁ ድጋሚ መሞከር/የጊዜ ማብቂያ ቁልፎችን ይቀበላል። ተጠቀም
`resolveToriiClientConfig` የግመል ኬዝ ውቅር ነገርን ለማዋሃድ (መደበኛ ማድረግ)
`iroha_config` መጀመሪያ)፣ env ይሽራል፣ እና የመስመር ውስጥ አማራጮች።

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

የአካባቢ ተለዋዋጮች ለአካባቢያዊ ዴቭ፡

| ተለዋዋጭ | ዓላማ |
|-------|--------|
| `IROHA_TORII_TIMEOUT_MS` | የጊዜ ማብቂያ ጠይቅ (ሚሊሰከንዶች)። |
| `IROHA_TORII_MAX_RETRIES` | ከፍተኛው የድጋሚ ሙከራዎች። |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | መጀመሪያ እንደገና ይሞክሩ። |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | ገላጭ የኋላ ማባዛት። |
| `IROHA_TORII_MAX_BACKOFF_MS` | ከፍተኛው የድጋሚ ሙከራ መዘግየት። |
| `IROHA_TORII_RETRY_STATUSES` | እንደገና ለመሞከር በነጠላ ሰረዝ የተለያዩ የኤችቲቲፒ ሁኔታ ኮዶች። |
| `IROHA_TORII_RETRY_METHODS` | እንደገና ለመሞከር በነጠላ ሰረዝ የተለዩ HTTP ዘዴዎች። |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` ይጨምራል። |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` ራስጌ ያክላል። |

እንደገና ይሞክሩ መገለጫዎች የአንድሮይድ ነባሪዎችን ያንፀባርቃሉ እና ለተመጣጣኝ ፍተሻዎች ወደ ውጭ ይላካሉ፡
`DEFAULT_TORII_CLIENT_CONFIG`፣ `DEFAULT_RETRY_PROFILE_PIPELINE`፣
`DEFAULT_RETRY_PROFILE_STREAMING`. `docs/source/sdk/js/torii_retry_policy.md` ይመልከቱ
ለመጨረሻ ነጥብ-መገለጫ ካርታ እና ግቤቶች የአስተዳደር ኦዲት ወቅት
JS4/JS7.

## ሊደረጉ የሚችሉ ዝርዝሮች እና የገጽ መግለጫዎች

የገጽታ ረዳቶች የ Python SDK ergonomics ለI18NI0000060X ያንጸባርቃሉ፣
`/v1/domains`፣ `/v1/assets/definitions`፣ ኤንኤፍቲዎች፣ ሚዛኖች፣ የንብረት መያዣዎች እና
የመለያ ግብይት ታሪክ።

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

## ከመስመር ውጭ ድጎማዎች እና ዲበ ውሂብን ይወስኑ

ከመስመር ውጭ አበል ምላሾች የበለፀገውን የሂሳብ መዝገብ ሜታዳታ ከፊት ለፊት ያጋልጣሉ -
`expires_at_ms`፣ `policy_expires_at_ms`፣ `refresh_at_ms`፣ `verdict_id_hex`፣
`attestation_nonce_hex` እና I18NI0000068X ከጥሬው ጋር ተመልሰዋል
መዝገብ ስለዚህ ዳሽቦርዶች የተካተቱትን I18NT0000004X ጭነት መፍታት የለባቸውም። አዲሱ
ቆጠራ ረዳቶች (I18NI0000069X፣ I18NI0000070X፣ I18NI0000071X፣
`deadline_ms_remaining`) የሚቀጥለውን የማለቂያ ቀነ-ገደብ ያደምቃል (አድስ → ፖሊሲ
→ ሰርተፍኬት) ስለዚህ የUI ባጆች አበል በሚኖርበት ጊዜ ኦፕሬተሮችን ሊያስጠነቅቁ ይችላሉ።
<24 ሰአት ይቀራል። ኤስዲኬ
በ `/v1/offline/reserve/topup` የተጋለጡትን የ REST ማጣሪያዎች ያንጸባርቃል:
`certificateExpiresBeforeMs/AfterMs`፣ `policyExpiresBeforeMs/AfterMs`፣
`verdictIdHex`፣ `attestationNonceHex`፣ `refreshBeforeMs/AfterMs`፣ እና እ.ኤ.አ.
`requireVerdict` / `onlyMissingVerdict` ቡሊያንስ። ልክ ያልሆኑ ጥምሮች (ለ
ለምሳሌ `onlyMissingVerdict` + `verdictIdHex`) ከ Torii በፊት በአገር ውስጥ ውድቅ ተደርጓል
ይባላል።

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

## ከመስመር ውጭ ገንዘብ መሙላት (ችግር + መመዝገብ)

የምስክር ወረቀት ለመስጠት ሲፈልጉ እና ወዲያውኑ ከፍተኛ አጋዥዎችን ይጠቀሙ
በመዝገብ ላይ ያስመዝግቡት። ኤስዲኬ የተሰጠውን እና የተመዘገበውን የምስክር ወረቀት ያረጋግጣል
መታወቂያዎች ከመመለሳቸው በፊት ይዛመዳሉ፣ እና ምላሹ ሁለቱንም ጭነቶች ያካትታል። አለ።
ምንም የተለየ የመጨመሪያ ነጥብ የለም; ረዳቱ ጉዳዩን ያሰራል + ጥሪዎችን ይመዝገቡ። ከሆነ
ቀደም ሲል የተፈረመ የምስክር ወረቀት አልዎት፣ ወደ `registerOfflineAllowance` ይደውሉ (ወይም
`renewOfflineAllowance`) በቀጥታ።

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

## Torii መጠይቆች እና ዥረት (WebSockets)

የጥያቄ ረዳቶች ሁኔታን፣ I18NT00000000 ሜትሪክስ፣ የቴሌሜትሪ ቅጽበተ-ፎቶዎችን እና ክስተትን ያጋልጣሉ
የI18NT0000005X ማጣሪያ ሰዋሰው በመጠቀም ዥረቶች። በዥረት መልቀቅ በራስ-ሰር ይሻሻላል
የድጋሚ ሙከራ በጀቱ ሲፈቅድ WebSockets እና ከቆመበት ይቀጥላል።

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

ለሌላው `streamBlocks`፣ `streamTransactions`፣ ወይም `streamTelemetry` ይጠቀሙ
WebSocket የመጨረሻ ነጥቦች. ሁሉም የዥረት ረዳቶች እንደገና ሞክሩ፣ ስለዚህ ያያይዙት።
ዳሽቦርዶችን ለመመገብ እና ለማስጠንቀቅ `onReconnect` መልሶ መደወል።

## ኤክስፕሎረር ቅጽበተ-ፎቶዎች እና የQR ጭነቶች

ኤክስፕሎረር ቴሌሜትሪ ለ`/v1/explorer/metrics` እና የተተየቡ ረዳቶችን ያቀርባል
ዳሽቦርዶች እንደገና መጫወት እንዲችሉ `/v1/explorer/accounts/{account_id}/qr` የመጨረሻ ነጥቦች
ፖርታሉን የሚያንቀሳቅሱ ተመሳሳይ ቅጽበተ-ፎቶዎች። `getExplorerMetrics()` መደበኛ ያደርገዋል
መንገዱ ሲቋረጥ `null` ይጭናል እና ይመልሳል። ጋር ያጣምሩት።
`getExplorerAccountQr()` በሚፈልጉበት ጊዜ ሁሉ i105 (የተመረጡ)/ሶራ (ሁለተኛ-ምርጥ) ቀጥተኛ እና የመስመር ላይ
SVG ለማጋራት አዝራሮች።

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

ማለፍ `i105` መስተዋቶች የአሳሽ ነባሪ የታመቀ
መራጮች; ለተመረጠው የi105 ውፅዓት መሻርን ያስወግዱ ወይም `i105_qr` ይጠይቁ
የQR-አስተማማኝ ልዩነት ሲፈልጉ። የታመቀው ቃል በቃል ሁለተኛው-ምርጥ ነው።
የሶራ-ብቻ አማራጭ ለ UX። ረዳቱ ሁል ጊዜ ቀኖናዊ መለያውን ይመልሳል ፣
የተመረጠው ቀጥተኛ እና ሜታዳታ (የአውታረ መረብ ቅድመ ቅጥያ፣ የQR ስሪት/ሞዱሎች፣ ስህተት
የእርምት እርከን፣ እና የመስመር ውስጥ SVG)፣ ስለዚህ CI/CD ያንን ተመሳሳይ የክፍያ ጭነቶች ማተም ይችላል።
ጠያቂ መቀየሪያዎችን ሳይጠራ ኤክስፕሎረር ይሸፍናል።

## ክፍለ-ጊዜዎችን እና ሰልፍን ያገናኙ

የግንኙነት አጋዥዎች I18NI0000096X መስታወት። የ
ለቅድመ እይታ ዝግጁ ክፍለ ጊዜ ፈጣኑ መንገድ `bootstrapConnectPreviewSession` ነው፣
የሚወስነው SID/URI ትውልድ እና Torii አንድ ላይ የሚገጣጠም
የምዝገባ ጥሪ.

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

ለQR/Deplink ቆራጥ ዩአርአይዎች ሲፈልጉ `register: false` ይለፉ
  ቅድመ እይታዎች.
- የክፍለ-ጊዜ መታወቂያዎችን ማግኘት ሲፈልጉ `generateConnectSid` እንዳለ ይቆያል
  ዩአርአይዎችን ሳይፈጥሩ።
- የአቅጣጫ ቁልፎች እና የምስጢር ጽሑፍ ፖስታዎች ከአገሬው ተወላጅ ድልድይ ይመጣሉ; መቼ ነው።
  አይገኝም ኤስዲኬ ወደ JSON ኮዴክ ተመልሶ ይጥላል
  `ConnectQueueError.bridgeUnavailable`.
- ከመስመር ውጭ ማቋቋሚያዎች በ IndexedDB ውስጥ እንደ Norito `.to` blobs ተቀምጠዋል። ወረፋ ይከታተሉ
  ግዛት በተለቀቀው I18NI0000102X /
  `.expired(ttlMs)` ስህተቶች እና ምግብ `connect.queue_depth` ቴሌሜትሪ እንደተገለፀው
  በመንገድ ካርታው ውስጥ.

### የመመዝገቢያ እና የፖሊሲ ቅጽበተ-ፎቶዎችን ያገናኙ

የፕላትፎርም ኦፕሬተሮች የግንኙን መዝገቡን ያለሱ ማየት እና ማዘመን ይችላሉ።
Node.js ትቶ `iterateConnectApps()` ገጾች በመመዝገቢያ በኩል, ሳለ
`getConnectStatus()` እና `getConnectAppPolicy()` የሩጫ ሰዓት ቆጣሪዎችን ያጋልጣሉ እና
የአሁኑ ፖሊሲ ፖስታ. `updateConnectAppPolicy()` የግመል ኬዝ መስኮችን ይቀበላል ፣
Torii የሚጠብቀውን ተመሳሳይ የ JSON ጭነት ደረጃ ማድረግ ይችላሉ።

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

ከማመልከትዎ በፊት ሁልጊዜ የቅርብ ጊዜውን I18NI0000109X ቅጽበታዊ ገጽ እይታን ያንሱ
ሚውቴሽን—የአስተዳደር ማረጋገጫ ዝርዝሩ የፖሊሲ ማሻሻያ መጀመሩን የሚያሳይ ማስረጃ ያስፈልገዋል
ከመርከቦቹ ወቅታዊ ገደቦች.

### የዌብሶኬት መደወያ ያገናኙ

`ToriiClient.openConnectWebSocket()` ቀኖናውን ይሰበስባል
`/v1/connect/ws` URL (I18NI0000112X፣ `role`፣ እና የማስመሰያ መለኪያዎችን ጨምሮ)፣ ማሻሻያዎች
`http→ws`/`https→wss`፣ እና የመጨረሻውን ዩአርኤል ለየትኛው ዌብሶኬት አስረክቡ።
እርስዎ የሚያቀርቡት ትግበራ. አሳሾች በራስ ሰር አለምአቀፉን እንደገና ይጠቀማሉ
`WebSocket`. Node.js ደዋዮች እንደ `ws` ያለ ግንበኛ ማለፍ አለባቸው፡

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

ዩአርኤሉን ብቻ ሲፈልጉ፣ `torii.buildConnectWebSocketUrl(params)` ይደውሉ ወይም ይደውሉ
ከፍተኛ-ደረጃ `buildConnectWebSocketUrl(baseUrl, params)` አጋዥ እና እንደገና ይጠቀሙ
በብጁ ማጓጓዣ/ወረፋ ውስጥ የውጤት ሕብረቁምፊ።

የተሟላ CLI-ተኮር ናሙና ይፈልጋሉ? የ
[የቅድመ እይታ አዘገጃጀትን ያገናኙ](./recipes/javascript-connect-preview.md) ሀን ያካትታል
ሊሄድ የሚችል ስክሪፕት እና የቴሌሜትሪ መመሪያ የሚቀርበውን የመንገድ ካርታ የሚያንፀባርቅ ነው።
የግንኙነት ወረፋ + የዌብሶኬት ፍሰትን መመዝገብ።

### ወረፋ ቴሌሜትሪ እና ማንቂያ

ዳሽቦርዶች ማንጸባረቅ እንዲችሉ የሽቦ ወረፋ መለኪያዎች በቀጥታ ወደ ረዳት ፎቆች
የመንገድ ካርታ KPIs.

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

`ConnectQueueError#toConnectError()` የወረፋ ውድቀቶችን ወደ አጠቃላይ ይለውጣል
`ConnectError` ታክሶኖሚ ስለዚህ የተጋሩ HTTP/WebSocket interceptors
መደበኛ `connect.queue_depth`፣ `connect.queue_overflow_total`፣ እና
`connect.queue_expired_total` ሜትሪክስ በፍኖተ ካርታው በሙሉ ተጠቅሷል።

## የዥረት ተመልካቾች እና የክስተት ጠቋሚዎች

`ToriiClient.streamEvents()` `/v1/events/sse` እንደ ያልተመሳሰለ ድግግሞሽ አውቶማቲክ ያጋልጣል
እንደገና ይሞክራል፣ ስለዚህ Node/Bun CLIs Rust CLI በሚያደርገው መንገድ የቧንቧ መስመር እንቅስቃሴን ሊጭኑ ይችላሉ።
ኦፕሬተሮች እንዲችሉ የ`Last-Event-ID` ጠቋሚውን ከRunbook artefactsዎ ጎን ያቆዩ
ሂደቱ እንደገና ሲጀመር ክስተቶችን ሳይዘለሉ ዥረቱን ይቀጥሉ።

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

- `PIPELINE_STATUS` ቀይር (ለምሳሌ I18NI0000129X፣ `Applied`፣ ወይም `Approved`) ወይም አዘጋጅ
  `STREAM_FILTER_JSON` CLI የሚቀበላቸውን ተመሳሳይ ማጣሪያዎች እንደገና ለማጫወት።
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` ተደጋጋሚውን እስከ ሀ
  ምልክት ተቀብሏል; የመጀመሪያዎቹን ክስተቶች ብቻ ሲፈልጉ `STREAM_MAX_EVENTS=25` ማለፍ
  ለጭስ ምርመራ.
- `ToriiClient.streamSumeragiStatus()` መስተዋቶች ተመሳሳይ በይነገጽ ለ
  `/v1/sumeragi/status/sse` ስለዚህ የጋራ መግባባት ቴሌሜትሪ ለብቻው ሊደረደር ይችላል ፣ እና
  iterator `Last-Event-ID` በተመሳሳይ መንገድ ያከብራል።
- ለመታጠፊያ CLI (የጠቋሚ ጽናት፣) `javascript/iroha_js/recipes/streaming.mjs` ይመልከቱ።
  env-var ማጣሪያ ይሽራል፣ እና `extractPipelineStatusKind` ሎግንግ) በJS4 ውስጥ ጥቅም ላይ ይውላል።
  ዥረት/WebSocket የመንገድ ካርታ ሊደርስ ይችላል።

## የ UAID ፖርትፎሊዮዎች እና የጠፈር ማውጫ

የስፔስ ማውጫ ኤፒአይዎች ሁለንተናዊ መለያ መታወቂያ (UAID) የህይወት ኡደት ላይ ናቸው። የ
ረዳቶች `uaid:<hex>` በቀጥታ ወይም ጥሬ ባለ 64-ሄክስ መፍጨት (LSB=1) ይቀበላሉ እና
ጥያቄዎችን ከማቅረቡ በፊት ቀኖናዊ አድርጋቸው፡-

- `getUaidPortfolio(uaid, { assetId })` ሒሳቦች በአንድ የውሂብ ቦታ፣
  የንብረት ይዞታዎችን በካኖናዊ መለያ መታወቂያዎች ማቧደን; ለማጣራት `assetId` ማለፍ
  ፖርትፎሊዮ ወደ ነጠላ ንብረት ምሳሌ።
- `getUaidBindings(uaid)` ሁሉንም የውሂብ ቦታ ↔ መለያ ይዘረዝራል።
  ማሰሪያ (`i105` የ `i105` ቃል በቃል ይመልሳል)።
- `getUaidManifests(uaid, { dataspaceId })` እያንዳንዱን የችሎታ መግለጫ ይመልሳል ፣
  የሕይወት ዑደት ሁኔታ፣ እና ለኦዲት የታሰሩ ሂሳቦች።ለኦፕሬተር ማስረጃዎች ጥቅሎች፣ የህትመት ፍሰቶችን አንጸባራቂ ማተም/መሻር እና የኤስዲኬ ፍልሰት
መመሪያ፣ ሁለንተናዊ መለያ መመሪያን ተከተል (`docs/source/universal_accounts_guide.md`)
ከእነዚህ የደንበኛ ረዳቶች ጎን ለጎን ፖርታል እና የምንጭ ሰነዱ እንደተመሳሰሉ ይቆያሉ።

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

ኦፕሬተሮች እንዲሁ ማኒፌክቶችን ማሽከርከር ወይም የአደጋ መካድ-አሸናፊዎችን ፍሰት ያለሱ ማከናወን ይችላሉ።
ወደ CLI መጣል. ሁለቱም ረዳቶች እንደ አማራጭ `{ signal }` ነገር ይቀበላሉ።
የረዥም ጊዜ ማቅረቢያዎች በ `AbortController` ሊሰረዙ ይችላሉ; ያልሆነ ነገር
አማራጮች ወይም `AbortSignal` ያልሆኑ ግብዓቶች የተመሳሰለ `TypeError` ያሳድጋሉ
ጥያቄ Torii ይደርሳል:

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

`publishSpaceDirectoryManifest()` ጥሬ አንጸባራቂ JSONን ይቀበላል (ከ
በ `fixtures/space_directory/`) ወይም በተከታታይ የሚቀርብ ማንኛውም ዕቃ
ተመሳሳይ መዋቅር. `privateKey`፣ `privateKeyHex`፣ ወይም `privateKeyMultihash` ካርታ ወደ
የ `ExposedPrivateKey` መስክ Torii ይጠብቃል እና ነባሪ `ed25519`
ምንም ቅድመ ቅጥያ በማይሰጥበት ጊዜ አልጎሪዝም. ሁለቱም ጥያቄዎች አንዴ I18NT0000016X ወረፋዎች ይመለሳሉ
መመሪያው (`202 Accepted`) ፣ በዚህ ጊዜ የሂሳብ ደብተር ያወጣል።
ተዛማጅ `SpaceDirectoryEvent`.

## አስተዳደር እና አይኤስኦ ድልድይ

`ToriiClient` የአስተዳደር ኤፒአይዎችን ኮንትራቶችን ለመፈተሽ ያጋልጣል።
ፕሮፖዛል፣ የምርጫ ካርዶችን (ሜዳ ወይም ዜድኬ) ማቅረብ፣ ምክር ቤቱን ማዞር እና መጥራት
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` ያለ በእጅ የተፃፉ DTOs። ISO 20022 ረዳቶች
በ `buildPacs008Message`/I18NI0000165X እና በ
`submitIso*`/`waitForIsoMessageStatus` ትሪዮ።

[የአስተዳደር እና የአይኤስኦ ድልድይ አሰራር](./recipes/javascript-governance-iso.md) ይመልከቱ
ለ CLI ዝግጁ የሆኑ ናሙናዎች እና ጠቋሚዎች ወደ ሙሉ የመስክ መመሪያ ይመለሱ
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC ናሙና እና ማቅረቢያ ማስረጃ

የJS ፍኖተ ካርታ ኦፕሬተሮች እንዲችሉ የRoadrunner Block Commitment (RBC) ናሙና ያስፈልገዋል
በ Sumeragi ያመጡት ብሎክ ከሚያረጋግጡት ቁርጥራጭ ማረጋገጫዎች ጋር የሚዛመድ መሆኑን ያረጋግጡ።
ጭነትን በእጅ ከመገንባት ይልቅ አብሮ የተሰሩ ረዳቶችን ይጠቀሙ፡-

1. `getSumeragiRbcSessions()` መስተዋቶች I18NI0000170X፣ እና
   `findRbcSamplingCandidate()` በብሎክ ሃሽ የመጀመሪያውን የተላከ ክፍለ ጊዜ በራስ-ሰር ይመርጣል
   (የውህደት ክፍሉ በማንኛውም ጊዜ ወደ እሱ ይመለሳል
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` አልተቀናበረም)።
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` መደበኛ `{blockHash,height,view}`
   ሲደመር አማራጭ `{count,seed,apiToken}` በጣም የተበላሹ ሄክስ ወይም አሉታዊ ኢንቲጀር ፈጽሞ ይሽራል
   Torii ይድረሱ።
3. `sampleRbcChunks()` ጥያቄውን ወደ `/v1/sumeragi/rbc/sample` ይለጠፋል፣ የቁርጥ ማስረጃዎችን ይመልሳል።
   እና Merkle ዱካዎች (I18NI0000178X፣ `chunkRoot`፣ `payloadHash`) በማህደር ማስቀመጥ አለቦት
   ቀሪው የጉዲፈቻ ማስረጃዎ።
4. `getSumeragiRbcDelivered(height, view)` ኦዲተሮች እንዲያደርጉ የቡድኑን የመላኪያ ሜታዳታ ይይዛል
   ማስረጃውን ከጫፍ እስከ ጫፍ መድገም ይችላል።

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

ሁለቱንም ምላሾች ለአስተዳደር በሚያስገቡት አርቲፊሻል ስር ይቆዩ። ይሽረው
በ`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` በኩል በራስ-የተመረጠ ክፍለ ጊዜ
አንድን የተወሰነ ብሎክ ለመመርመር በሚፈልጉበት ጊዜ እና RBC ቅጽበተ-ፎቶዎችን ለማምጣት አለመሳካቶችን እንደ ሀ
በፀጥታ ወደ ቀጥታ ሁነታ ከማውረድ ይልቅ የቅድመ በረራ ጌቲንግ ስህተት።

## ሙከራ እና CI

1. የመሸጎጫ ጭነት እና npm ቅርሶች።
2. `npm run build:native` አሂድ.
3. `npm test` (ወይም `node --test` ለጭስ ስራዎች) ያስፈጽሙ.

የማመሳከሪያው GitHub Actions የስራ ፍሰት ይኖራል
`docs/source/examples/iroha_js_ci.md`.

## ቀጣይ እርምጃዎች

- በ `javascript/iroha_js/index.d.ts` ውስጥ የተፈጠሩትን ዓይነቶች ይገምግሙ።
- የምግብ አዘገጃጀቱን በ `javascript/iroha_js/recipes/` ስር ያስሱ።
- የክፍያ ጭነቶችን ከጎን ለመመርመር `ToriiClient`ን ከI18NT0000007X ፈጣን ጅምር ጋር ያጣምሩ
  የኤስዲኬ ጥሪዎች።