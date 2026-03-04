---
lang: mn
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c39b3066b18659388a549cd23d5ba6bfed27844b5ad3ff2e3e6779301e4b53a3
source_last_modified: "2026-01-22T16:26:46.513222+00:00"
translation_last_reviewed: 2026-02-07
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
slug: /sdks/recipes/javascript-ledger-flow
translator: machine-google-reviewed
---

SampleDownload-г '@site/src/components/SampleDownload'-аас импортлох;

Энэхүү жор нь Node.js `@iroha2/torii-client` болон
`@iroha2/crypto-target-node` багцууд нь CLI дэвтэрийн зааварчилгааг хуулбарлах боломжтой.

<Жишээ татаж авах
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  файлын нэр = "ledger-flow.mjs"
  description="Энэ дэвтэрт ашигласан яг JavaScript скриптийг татаж аваарай."
/>

## Урьдчилсан нөхцөл

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Жишээ скрипт

```ts title="ledger-flow.mjs"
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';

const adminAccount = process.env.ADMIN_ACCOUNT!;
const receiverAccount = process.env.RECEIVER_ACCOUNT!;
const adminPrivateKey = process.env.ADMIN_PRIVATE_KEY!;

const client = ToriiClient.create({
  apiUrl: 'http://127.0.0.1:8080',
});

const {publicKey, privateKey} = createKeyPairFromHex(adminPrivateKey);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000',
  authority: adminAccount,
  instructions: [
    {Register: {assetDefinition: {numeric: {id: 'coffee#wonderland'}}}},
    {Mint: {asset: {id: `coffee#wonderland##${adminAccount}`}, value: {quantity: '250'}}},
    {Transfer: {
      asset: {id: `coffee#wonderland##${adminAccount}`},
      destination: receiverAccount,
      value: {quantity: '50'},
    }},
  ],
});

tx.sign({publicKey, privateKey});
const receipt = await client.submitTransaction(tx);
const txHash = receipt?.payload?.tx_hash ?? "<pending>";
console.log('Submitted tx', txHash);

const balances = await client.listAccountAssets(receiverAccount, {limit: 10});
for (const asset of balances.items) {
  if (asset.id.definition === 'coffee#wonderland') {
    console.log('Receiver holds', asset.value, 'units of', asset.id.definition);
  }
}
```

`node --env-file=.env ledger-flow.mjs` ашиглан ажиллуул (эсвэл орчныг экспортлох
хувьсагчдыг гараар). Бүртгэлд гүйлгээний хэшийг харуулах ёстой (баримтаас
ачаалал) болон шинэчлэгдсэн хүлээн авагчийн үлдэгдэл.

## Паритетийг баталгаажуулна уу

- `iroha --config defaults/client.toml transaction get --hash <hash>`-ээр дамжуулан гүйлгээний дэлгэрэнгүй мэдээллийг авна уу.
- Үлдэгдлийг `iroha --config defaults/client.toml asset list filter '{"id":"coffee#wonderland##<account>"}'` ашиглан шалгана уу.
- SDK паритыг баталгаажуулахын тулд ялгарсан хэшийг Rust болон Python жортой харьцуул.