---
slug: /sdks/javascript
lang: dz
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

I18NI000000037X འདི་ Torii དང་ཅིག་ཁར་ འབྲེལ་བ་འཐབ་ནིའི་དོན་ལུ་ ཀེ་ནོ་ནིག་ནོ་ཌི་.ཇེ་ཨེསི་ ཐུམ་སྒྲིལ་ཨིན། འདི
bundles I18NT0000002X བཟོ་སྐྲུན་པ་, Ed25519 རོགས་རམ་ ཤོག་ངོས་བཀོལ་སྤྱོད་དང་ བཟོད་བསྲན།
HTTP/WebSocket མཁོ་སྤྲོད་པ་དེ་གིས་ དབྱེ་བ་ཡིག་གཟུགས་ལས་ CLI རྒྱུན་འགྲུལ་ཚུ་ མེ་ལོང་ནང་ མེ་ལོང་བཟོ་ཚུགས།

## གཞི་བཙུགས་འབད་ནི།

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

བཟོ་བསྐྲུན་གྱི་རིམ་པ་དེ་ `cargo build -p iroha_js_host`. ལག་ཆས་རིམ་སྒྲིག་འདི་ ལས་ངེས་གཏན་བཟོ།
`rust-toolchain.toml` འདི་ `npm run build:native` གཡོག་བཀོལ་མ་འབད་བའི་ཧེ་མ་ ས་གནས་ནང་ ཐོབ་ཚུགས།

## ལྡེ་མིག་འཛིན་སྐྱོང་པ།

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

## ཚོང་འབྲེལ།

I18NT0000003X བཀོད་རྒྱ་བཟོ་མི་ཚུ་གིས་ ངོས་འཛིན་དང་ མེ་ཊ་ཌེ་ཊ་ དེ་ལས་ འབོར་ཚད་ཚུ་ སྤྱིར་བཏང་བཟོཝ་ཨིན།
ཨིན་ཀོ་ཌི་འབད་ཡོད་པའི་ཚོང་འབྲེལ་ཚུ་ རཱསི་/སི་ཨེལ་ཨའི་ པེ་ལོཌི་ཚུ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།

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

## Torii མཁོ་སྤྲོད་པའི་རིམ་སྒྲིག་།

I18NI000000041X གིས་ I18NI0000042X གིས་ retry/timeout knos དང་ལེན་འབདཝ་ཨིན། ལག་ལེན་འཐབ་ནི
`resolveToriiClientConfig` རྔ་མོང་རིམ་སྒྲིག་དངོས་པོ་ (སྤྱིར་བཏང་བཟོ་ནི།
`iroha_config` དང་པ་) env བསྐྱར་ལོག་དང་ ནང་ཐིག་གདམ་ཁ་ཚུ།

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

ས་གནས་ཀྱི་ dev གི་དོན་ལུ་ མཐའ་འཁོར་གྱི་འགྱུར་ཅན་ཚུ།

| འགྱུར་ཅན་ | དམིགས་ཡུལ། |
|-----------------------------------------------------------------------------------------------------------------------------------------
| `IROHA_TORII_TIMEOUT_MS` | ཞུ་བ་འབད་བའི་དུས་ཚོད་ (milliseconds)། |
| I18NI0000046X | མཐོ་ཤོས་ལོག་འབད་རྩོལ་བསྐྱེད་དོ། |
| I18NI0000047X | འགོ་ཐོག་ལོག་འབད་ནི་གི་དཔའ་བཅམ། |
| I18NI0000048X | བརྒྱུད་འཕྲིན་རྒྱབ་ལོག་བསྒྱུར་བཅོས། |
| I18NI0000049X | བསྐྱར་ལོག་མཐོ་ཤོས་ཕྱིར་འགྱངས་འབད་རྩོལ་བསྐྱེད། |
| `IROHA_TORII_RETRY_STATUSES` | ལོག་འབད་རྩོལ་བསྐྱེད་ནི་ལུ་ ཀེམ་-དབྱེ་ཕྱེ་འབད་ཡོད་པའི་ ཨེཆ་ཊི་ཊི་པི་གནས་རིམ་ཨང་རྟགས་ཚུ། |
| `IROHA_TORII_RETRY_METHODS` | ལོག་འབད་རྩོལ་བསྐྱེད་ནི་ལུ་ མགྱོགས་དབྱེ་ཨེཆ་ཊི་ཊི་པི་ཐབས་ལམ་ཚུ་ཨིན། |
| I18NI0000002X | `X-API-Token` ཁ་སྐོང་འབདཝ་ཨིན། |
| `IROHA_TORII_AUTH_TOKEN` | I18NI0000005X མགོ་ཡིག་ཁ་སྐོང་འབདཝ་ཨིན། |

བསྐྱར་ལོག་གསལ་སྡུད་ཚུ་གིས་ ཨེན་ཌོརཌི་སྔོན་སྒྲིག་ཚུ་ མེ་ལོང་དང་ ཆ་སྙོམས་ཞིབ་དཔྱད་ཚུ་གི་དོན་ལུ་ ཕྱིར་འདྲེན་འབདཝ་ཨིན།
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. གཟིགས། `docs/source/sdk/js/torii_retry_policy.md`
མཐའ་མཇུག་ལས་ གསལ་སྡུད་ཀྱི་སབ་ཁྲ་བཟོ་ནི་དང་ གཞུང་སྐྱོང་རྩིས་ཞིབ་ཀྱི་དོན་ལས་ ཚད་གཞི་ཚུ་གི་དོན་ལུ་ཨིན།
JS4/JS7.

## བསྐྱར་བརྗོད་འབད་བཏུབ་པའི་ཐོ་ཡིག་དང་ རིམ།

འགྲུལ་བཞུད་ཀྱི་གྲོགས་རམ་པ་ཚུ་གིས་ I18NI000000060X, གི་དོན་ལུ་ Python SDK ergonomics འདི་ མེ་ལོང་ནང་ གསལ་སྟོན་འབདཝ་ཨིན།
I18NI000000061X, `/v1/assets/definitions`, NFTs, balues, རྒྱུ་དངོས་འཛིན་མཁན་, དང་།
རྩིས་ཁྲའི་ཚོང་འབྲེལ་གྱི་ལོ་རྒྱུས།

I18NF0000022X

## Kagemusha offline cash

The first-release JavaScript package does not expose Kagemusha readiness, top-up, redemption, or operation polling. Those flows require canonical Norito archives and device-bound mobile custody; use IrohaSwift or the JVM SDK instead of hand-encoding requests in JavaScript.

## Kagemusha offline cash

The first-release JavaScript package does not expose Kagemusha readiness, top-up, redemption, or operation polling. Those flows require canonical Norito archives and device-bound mobile custody; use IrohaSwift or the JVM SDK instead of hand-encoding requests in JavaScript.

## Torii འདྲི་དཔྱད་དང་རྒྱུན་སྤེལ་ (ཝེབ་སོ་ཀེཊི་ཚུ།)

འདྲི་དཔྱད་གྲོགས་རམ་པ་ཚུ་གིས་ གནས་ཚད་ཕྱིར་བཏོན་འབདཝ་ཨིན།
Norito ཚགས་མ་ཡིག་གཟུགས་ལག་ལེན་འཐབ་ཐོག་ལས་ རྒྱུན་ལམ་ཚུ། ལུ་ རང་བཞིན་གྱིས་ ཡར་བསྐྱེད་འབད་དོ།
ཝེབ་སོ་ཀེཊི་ཚུ་དང་ བསྐྱར་ཚོད་འཆར་དངུལ་གྱིས་ གནང་བ་བྱིན་པའི་སྐབས་ སླར་འབྱུང་འགོ་བཙུགསཔ་ཨིན།

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

`streamBlocks`, I18NI0000086X, ཡང་ན་ I18NI0000000087X འདི་གཞན་གྱི་དོན་ལུ་ལག་ལེན་འཐབ།
ཝེབ་སོ་ཀེཊི་མཐའ་ཐིག་ཚུ། རྒྱུན་སྤེལ་གྱི་གྲོགས་རམ་པ་ཆ་མཉམ་གྱིས་ བསྐྱར་ལོག་འབད་ནི་གི་དཔའ་བཅམ་དོ་ཡོདཔ་ལས་ དེ་གིས་ ཧུཀ་འབདཝ་ཨིན།
I18NI000000088X བརྡ་བཀོད་དང་ཉེན་བརྡ་ཚུ་ལུ་ལོག་འབོད་བརྡ་འབདཝ་ཨིན།

## འཚོལ་ཞིབ་པ་པར་ཆས་དང་ QR པེ་ལོསི།

འཚོལ་ཞིབ་ཀྱི་བརྡ་འཕྲིན་གྱིས་ `/v1/explorer/metrics` དང་ གི་དོན་ལུ་ ཡིག་དཔར་རྐྱབ་མི་གྲོགས་རམ་པ་བྱིནམ་ཨིན།
I18NI000000090X མཐའ་མཚམས་ཚུ་ དེ་ལས་ ཌེཤ་བོརཌི་ཚུ་གིས་ བསྐྱར་རྩེད་འབད་ཚུགས།
དེ་དང་འདྲ་བའི་པར་བརྙན་དེ་གིས་ དྲྭ་ཚིགས་ལུ་དབང་ཚད་ཡོདཔ་ཨིན། `getExplorerMetrics()` སྤྱིར་བཏང་བཟོཝ་ཨིན།
འགྲུལ་ལམ་འདི་ལྕོགས་མིན་བཟོ་བའི་སྐབས་ པེ་ལོཌི་དང་ `null` སླར་ལོག་འབདཝ་ཨིན། དེ་དང་མཉམ་དུ་བསྡམས།
I18NI000000093X ཁྱོད་ལུ་ i105 (དགའ་གདམ་)/སོ་ར་ (ངོ་མ) དང་ ནང་ཐིག་ཚུ་དགོཔ་ཨིན།
བགོ་བཤའ་ཨེབ་རྟ་ཚུ་གི་དོན་ལུ་ SVG.

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

`i105` བརྒྱུད་དེ་ Explorer གི་སྔོན་སྒྲིག་བསྡམ་བཞག་ཡོདཔ།
འདམ་ཁ་རྐྱབ་མི་ཚུ། དང་འདོད་ཡོད་པའི་ i105 ཐོན་འབྲས་ཡང་ན་ ཞུ་བ་ i105 གི་དོན་ལུ་ བཀག་ཆ་འབད་མི་འདི་ ཕྱིར་བཏོན་འབད།
ཁྱོད་ལུ་ QR-safe དབྱེ་བ་དགོཔ་སྐབས། བསྡམས་པའི་ཚིག་འདི་ དྲག་ཤོས་གཉིས་པ་ཨིན།
UX གི་དོན་ལུ་ སོ་ར་རྐྱངམ་ཅིག་གི་གདམ་ཁ། གྲོགས་རམ་པ་དེ་གིས་ ཨ་རྟག་ར་ ཀེ་ནོ་ནིག་ངོས་འཛིན་པ་སླར་ལོག་འབདཝ་ཨིན།
སེལ་འཐུ་འབད་ཡོད་པའི་ཚིག་དོན་དང་ མེ་ཊ་ཌེ་ཊ་ (ཡོངས་འབྲེལ་སྔོན་སྒྲིག་ ཀིའུ་ཨར་ཐོན་རིམ་/ཚད་གཞི་ འཛོལ་བ།
ནོར་བཅོས་འབད་ནིའི་རིམ་པ་ དེ་ལས་ ནང་ཐིག་ཨེསི་ཝི་ཇི་) དེ་འབདཝ་ལས་ སི་ཨའི་/སི་ཌི་གིས་ དེ་བཟུམ་མའི་ པེ་ལོཌི་ཅོག་འཐདཔ་ཚུ་ དཔར་བསྐྲུན་འབད་ཚུགས།
བརྟག་ཞིབ་ཀྱི་ཁ་ཐོག་འདི་ བེ་སི་པོཀ་གཞི་བསྒྱུར་འབད་མི་ཚུ་ལུ་ འབོཝ་ཨིན།

## ལཱ་ཡུན་དང་གྱལ་རིམ་མཐུད།

མཐུད་བྱེད་པ་དེ་གིས་ `docs/source/connect_architecture_strawman.md` མེ་ལོང་། ཚིག༌ཕྲད
སྔོན་ལྟ་གྲ་སྒྲིག་ཡོད་པའི་ལཱ་ཡུན་ལུ་མགྱོགས་ཤོས་འདི་ I18NI000000097X ཨིན།
འདི་ཡང་ གཅིག་ཁར་བསྡོམས་ཏེ་ SID/URI དང་ I18NT0000012X ཚུ་གཅིག་ཁར་བསྡོམས་ཏེ་ཡོདཔ་ཨིན།
ཐོ་བཀོད་ཁ་པར་ཨང་།

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

- ཁྱོད་ལུ་ QR/deeplink གི་དོན་ལུ་ གཏན་འབེབས་ཀྱི་ཡུ་ཨར་ཨའི་ཚུ་རྐྱངམ་ཅིག་དགོཔ་ད་ `register: false` འདི་ བརྒྱུད་དེ་འགྱོཝ་ཨིན།
  སྔོན་ལྟ་ཚུ།
- ཁྱོད་ཀྱིས་ ལཱ་ཡུན་ཨའི་ཌི་ཚུ་ བཏོན་དགོཔ་ད་ I18NI00000099 འཐོབ་ཚུགསཔ་ཨིན།
  URIs མ་བཙུགས་པར་།
- ཕྱོགས་སྟོན་ལྡེ་མིག་དང་ སི་ཕར་ཊེགསི་ ཡིག་ཤུབས་ཚུ་ ས་གནས་ཀྱི་ཟམ་ལས་འོངམ་ཨིན། ནམ༌
  འཐོབ་མ་ཚུགས་པའི་ ཨེསི་ཌི་ཀེ་འདི་ ཇེ་ཨེསི་ཨོ་ཨེན་ ཀོ་ཌེཀ་ལུ་ལོག་འགྱོ་སྟེ་ བཀོ་ནི་ལུ་ འཐོབ་མ་ཚུགས།
  `ConnectQueueError.bridgeUnavailable`.
- ཨོཕ་ལིན་བཱ་ཕར་ཚུ་ I18NT000000006X I18NI000001010101010 འདི་ IndexedDB ནང་ལུ་ གསོག་འཇོག་འབད་ཡོདཔ་ཨིན། བལྟ་རྟོག་འབད་ནི།
  བཙན་འཕྲོག་ཐོག་ལས་ `ConnectQueueError.overflow(limit)` / མངའ་སྡེ་
  `.expired(ttlMs)` འཛོལ་བ་དང་ `connect.queue_depth` བཀོད་སྒྲིག་འབད་ཡོདཔ་བཟུམ་སྦེ་ བརྒྱུད་འཕྲིན་གཏང་།
  ལམ་སྟོན་ནང་།

### ཐོ་བཀོད་དང་སྲིད་བྱུས་པར་རིས།

སྟེགས་རིས་བཀོལ་སྤྱོད་པ་ཚུ་གིས་ མཐུད་ལམ་ཐོ་བཀོད་འདི་ མེད་པར་ བཙུགས་ནི་དང་དུས་མཐུན་བཟོ་ཚུགས།
བཏོན་གཏང་ནི་.js. ཐོ་བཀོད་བརྒྱུད་དེ་ `iterateConnectApps()` ཤོག་ལེབ་ཚུ་
`getConnectStatus()` དང་ `getConnectAppPolicy()` རན་ཊའིམ་ གྱངས་ཁ་ཚུ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
ད་ལྟའི་སྲིད་བྱུས་ཡིག་ཤུབས་ཁང་། `updateConnectAppPolicy()` རྔ་མོང་ས་ཁོངས་འདི་ངོས་ལེན་འབདཝ་ཨིན།
དེ་འབདཝ་ལས་ ཁྱོད་ཀྱིས་ Torii རེ་བ་བསྐྱེད་མི་ JSON པེ་ལོཌ་གཅིགཔོ་འདི་ འཁྲབ་སྟོན་འབད་ཚུགས།

I18NF0000028X

ཞུ་ཡིག་མ་བཙུགས་པའི་ཧེ་མ་ ཨ་རྟག་ར་ `getConnectStatus()` པར་ཚུ་བཏབ་དགོ།
འགྱུར་བ།—གཞུང་སྐྱོང་ཞིབ་དཔྱད་ཐོ་ཡིག་ནང་ སྲིད་བྱུས་དུས་མཐུན་བཟོ་ནི་འགོ་བཙུགས་པའི་ སྒྲུབ་བྱེད་དགོཔ་ཨིན།
གྲུ་གཟིངས་ཀྱི་ད་ལྟོའི་ཚད་གཞི་ལས་།

### ཝེབ་སོ་ཀེཊི་ ཌའི་ལིང་མཐུད།

`ToriiClient.openConnectWebSocket()` གིས་ ཁྲིམས་ལུགས་འདི་བསྡུ་སྒྲིག་འབད་ཡོདཔ་ཨིན།
`/v1/connect/ws` URL (`sid`, I18NI000000113X, དང་ ཊོ་ཀེན་ཚད་བཟུང་ཚུ་རྩིས་ཏེ་) ཡར་འཕེལ་ཚུ།
`http→ws` / `https→wss`, དང་ ཝེབ་སོ་ཀེཊི་གང་རུང་ཅིག་ལུ་ མཐའ་མཇུག་གི་ཡུ་ཨར་ཨེལ་ལུ་ སྤྲོད་དགོ།
ལག་ལེན་ཁྱོད་ཀྱིས་བཀྲམ་སྤེལ་འབད་ཡོདཔ། བརའུ་ཟར་གྱིས་ རང་བཞིན་གྱིས་ ཡོངས་ཁྱབ་འདི་ ལོག་ལག་ལེན་འཐབ།
`WebSocket`. Node.js འབོད་བརྡ་གཏང་མི་ཚུ་གིས་ I18NI000000117X བཟུམ་གྱི་ བཟོ་བསྐྲུན་པ་ཅིག་སྤྲོད་དགོ།

I18NF0000029X

ཁྱོད་ལུ་ཡུ་ཨར་ཨེལ་རྐྱངམ་ཅིག་དགོ་པའི་སྐབས་ `torii.buildConnectWebSocketUrl(params)` ཡང་ན་ ལུ་ཁ་པར་གཏང་།
མཐོ་རིམ་གནས་རིམ་ `buildConnectWebSocketUrl(baseUrl, params)` གྲོགས་རམ་འབད་མི་དང་ ལོག་སྟེ་ལག་ལེན་འཐབ།
སྲོལ་སྒྲིག་སྐྱེལ་འདྲེན་/གྲལ་ཐིག་ནང་ཡིག་རྒྱུན་གྲུབ་འབྲས་ཐོན་ཡོདཔ།

CLI-oriented དཔེ་ཚད་ཆ་ཚང་འཚོལ་དོ་ཡོདཔ་ཨིན་ན? ཚིག༌ཕྲད
[སྔོན་ལྟའི་བཟོ་ཐབས་འབྲེལ་མཐུད་](./recipes/javascript-connect-preview.md) ནང་ a ཚུད་ཡོད།
འགྲུལ་ལམ་གྱི་ས་ཁྲ་འདི་ བཀྲམ་སྤེལ་འབད་ཚུགས་པའི་ འགྲུལ་ལམ་གྱི་ ལམ་སྟོན་དང་ ཊེ་ལི་མི་ཊི་གི་ལམ་སྟོན།
མཐུད་ལམ་གྲལ་ཐིག་ + ཝེབ་སོ་ཀེཊི་ཕོལཔ་འདི་ ཡིག་ཐོག་ལུ་བཀོད་དོ།

### གྱལ་རིམ་བརྡ་འཕྲིན་དང་ཉེན་བརྡ།

ཐགཔ་གི་གྱལ་རིམ་ཚུ་ ཐད་ཀར་དུ་ གྲོགས་རམ་གྱི་ཁ་ཐོག་ལུ་ གློག་ཐག་ཚུ་གིས་ མེ་ལོང་ནང་ འབད་ཚུགས།
the rop map KPIs.

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

`ConnectQueueError#toConnectError()` གྱལ་རིམ་འཐུས་ཤོར་ཚུ་སྤྱིར་བཏང་ལུ་གཞི་བསྒྱུར་འབདཝ་ཨིན།
`ConnectError` བརྗེ་སོར་འབད་མི་ HTTP/WebSocket བར་ཆད་ཚུ་གིས་ བརྗེ་སོར་འབད་ཚུགས།
ཚད་ལྡན་ `connect.queue_depth`, `connect.queue_overflow_total`, དང་།
`connect.queue_expired_total` ལམ་གྱི་ས་ཁྲ་ནང་ལུ་ གཞི་བསྟུན་འབད་ཡོདཔ་ཨིན།

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

## UAID ཡིག་ཆའི་ཡིག་ཆའི་སྣོད་ཐོ།

གནམ་སྟོང་སྣོད་ཐོ་ཨེ་པི་ཨའི་ཚུ་གིས་ ཡོངས་ཁྱབ་རྩིས་ཐོ་ཨའི་ཌི་ (ཡུ་ཨེ་ཨའི་ཌི་) མི་ཚེ་འཁོར་རིམ་འདི་ ཁ་ཐོག་ལུ་ ཁ་ཐོག་ལུ་འཐོནམ་ཨིན། ཚིག༌ཕྲད
གྲོགས་རམ་པ་ཚུ་གིས་ `uaid:<hex>` ཡིག་འབྲུ་ཚུ་ཡང་ན་ ཧེགསི་ ༦༤-ཧེགསི་ བཞུ་བཅུག་མི་ (LSB=1) དང་།
ཞུ་བ་མ་ཕུལ་བའི་ཧེ་མ་ དེ་ཚུ་ ཁྲིམས་མཐུན་བཟོ་ནི།

- I18NI000000141X གནད་སྡུད་ས་སྟོང་རེ་ལུ་ ལྷག་ལུས་ཚུ་ བསྡོམས་རྩིས་འབདཝ་ཨིན།
  ཀེ་ནོ་ནིག་རྩིས་ཐོའི་ཨའི་ཌི་ཚུ་གིས་ བདག་དབང་གི་ བདག་དབང་ཚུ་ སྡེ་ཚན་བཟོ་ནི། འདི་ཚགས་མ་འབད་ནིའི་དོན་ལུ་ `assetId`
  spired of sesigh ཅིག་ལུ་མར་འབབ་ཨིན།
- I18NI000000143X གནད་སྡུད་ས་སྟོང་ ↔ རྩིས་ཐོ་ཆ་མཉམ་གྲངས་རྩིས་འབདཝ་ཨིན།
  བཱའིན་ཌིང་ (`i105` གིས་ `i105` ཚིག་དོན་ཚུ་སླར་ལོག་འབདཝ་ཨིན།)
- I18NI0000000146X གིས་ ལྕོགས་གྲུབ་རེ་རེ་བཞིན་ གསལ་སྟོན་འབདཝ་ཨིན།
  ཚེ་སྲོག་གི་གནས་རིམ་དང་ རྩིས་ཞིབ་འབད་ནིའི་དོན་ལུ་ མཐའ་མཚམས་ཀྱི་རྩིས་ཁྲ་ཚུ།བཀོལ་སྤྱོད་པའི་སྒྲུབ་བྱེད་ཐུམ་སྒྲིལ་ཚུ་གི་དོན་ལུ་ དཔར་བསྐྲུན་/ཆ་མེད་གཏང་ནི་གི་རྒྱུན་རིམ་ཚུ་གསལ་སྟོན་འབད་ཞིནམ་ལས་ ཨེསི་ཌི་ཀེ་གནས་སྤོ།
ལམ་སྟོན་, ཡོངས་ཁྱབ་རྩིས་ཁྲའི་ལམ་སྟོན་ (`docs/source/universal_accounts_guide.md`) ལུ་རྗེས་སུ་འབྲང་།
མཁོ་སྤྲོད་འབད་མི་གྲོགས་རམ་པ་ཚུ་དང་གཅིག་ཁར་ དྲྭ་ཚིགས་དང་འབྱུང་ཁུངས་ཡིག་ཆ་ཚུ་ མཉམ་འབྱུང་ནང་ལུསཔ་ཨིན།

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

བཀོལ་སྤྱོད་པ་ཚུ་གིས་ གསལ་སྟོན་ཚུ་ཡང་ བསྒྱིར་ཚུགས་ནི་དང་ ཡང་ན་ གློ་བུར་གྱི་ གློ་བུར་གྱི་རྒྱལ་ཁ་ཐོབ་མི་ཚུ་ མེད་པའི་ བསྒྱིར་ཚུགས།
CLI ལུ་བཀོག་བཞགཔ་ཨིན། གྲོགས་རམ་པ་གཉིས་ཆ་ར་གིས་ གདམ་ཁ་ཅན་གྱི་ `{ signal }` དངོས་པོ་ཅིག་ དང་ལེན་འབདཝ་ཨིན།
ཡུན་རིངམོ་སྦེ་ གཡོག་བཀོལ་མི་ ཞུ་ཡིག་ཚུ་ `AbortController` གིས་ ཆ་མེད་གཏང་ཚུགས། དངོས་པོ་མ་ཡིན་པ།
གདམ་ཁ་ ཡང་ན་ non-I18NI0000150X ཨིན་པུཊི་ཚུ་གིས་ མཉམ་འབྱུང་ `TypeError` འདི་ གོང་ལས་ ཡར་སེང་འབདཝ་ཨིན།
redity hits Torii:

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

`publishSpaceDirectoryManifest()` གིས་ ཇེ་ཨེསི་ཨོ་ཨེན་ (མཐུན་སྒྲིག་འབད་དོ་ཡོདཔ་ཨིན་) སྔོ་ལྗང་ཅན་གང་རུང་ཅིག་ གསལ་སྟོན་འབདཝ་ཨིན།
`fixtures/space_directory/` གི་འོག་ལུ་སྒྲིག་བཀོད་ཚུ་) ཡང་ན་ དངོས་པོ་གང་རུང་ཅིག་ལུ་ རིམ་སྒྲིག་འབདཝ་ཨིན།
གཅིག་པའི་གཞི་བཀོད། `privateKey`, `privateKeyHex`, ཡང་ན་ I18NI000000156X
`ExposedPrivateKey` རེ་བ་དང་ `ed25519` ལུ་སྔོན་སྒྲིག་འབད་ནི།
སྔོན་འཇུག་མ་བྱིན་པའི་སྐབས། ཞུ་བ་གཉིས་ཆ་ར་ I18NT0000016X enqueue ཚུ་ཚར་གཅིག་སླར་ལོག་འབདཝ་ཨིན།
བཀོད་སྒྲིག་ (`202 Accepted`) དེའི་སྐབས་ ལེཌ་ཇར་གྱིས་ ཕྱིར་འཐེན་འབད་འོང་།
མཐུན་སྒྲིག་ `SpaceDirectoryEvent`.

## གཞུང་སྐྱོང་དང་ ISO ཟམ་པ།

I18NI000000161X གཞུང་སྐྱོང་ཨེ་པི་ཨའེ་ཚུ་ གན་རྒྱ་ཚུ་བརྟག་དཔྱད་འབད་ནིའི་དོན་ལུ་ འགྲེམ་སྟོན་འབདཝ་ཨིན།
གྲོས་འཆར་ཚུ་ ཚོགས་རྒྱན་ཚུ་ ཕུལ་ཞིནམ་ལས་ (གཙང་མ་ཡང་ན་ ZK) ཚོགས་སྡེ་འདི་ བསྒྱིར་ཞིནམ་ལས་ ཁ་པར་བཏང་ནི།
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` ལགཔ་གིས་བྲིས་ཡོད་པའི་ ཌི་ཊི་ཨོ་ཚུ་མེདཔ་ཨིན། ISO ༢༠༠༢༢ རོགས་སྐྱོར།
`buildPacs008Message`/I18NI000000165X བརྒྱུད་དེ་ དཔེ་རིས་གཅིག་པ།
`submitIso*`/`waitForIsoMessageStatus` སུམ་སི།

བལྟ། [གཞུང་སྐྱོང་དང་ ISO ཟམ་པའི་བཟོ་ཐབས་](./recipes/javascript-governance-iso.md)
གི་དོན་ལུ་ CLI-red དཔེ་ཚད་ཚུ་ 2 ནང་ ས་སྒོ་ཆ་ཚང་ལམ་སྟོན་པ་ལུ་ ལོག་སྟེ་ དཔག་བྱེད་འབད།
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

## བརྟག་དཔྱད་དང་སི་ཨའི།

༡ འདྲ་མཛོད་དང་ npm ཅ་ལག་ཚུ།
2. `npm run build:native` རྒྱུགས།
༣ ཐལཝ་གི་ལཱ་གཡོག་གི་དོན་ལུ་ `npm test` (ཡང་ན་ `node --test`) ལག་ལེན་འཐབ་དགོ།

གཞི་བསྟུན་ GitHub Actions ལཱ་གི་རྒྱུན་རིམ་འདི་ ༢༠༠༨ ལུ་སྡོད་དོ་ཡོདཔ་ཨིན།
`docs/source/examples/iroha_js_ci.md`.

## ཤུལ་མམ་གྱི་གོམ་པ།

- `javascript/iroha_js/index.d.ts` ནང་ བཟོ་བཏོན་འབད་ཡོད་པའི་དབྱེ་བ་བསྐྱར་ཞིབ་འབད།
- `javascript/iroha_js/recipes/` འོག་ལུ་ཡོད་པའི་བཟའ་ཐབས་ཚུ་འཚོལ་ཞིབ་འབད།
- Pair `ToriiClient` དང་མཉམ་དུ་ I18NT000000007X མཉམ་དུ་ པེ་ལོཌ་ཚུ་ བརྟག་དཔྱད་འབད་ནིའི་དོན་ལུ་ མགྱོགས་འགོ་བཙུགས་ཡོདཔ།
  SDK འབོད་བརྡ་ཚུ།
