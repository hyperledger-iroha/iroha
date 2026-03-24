---
slug: /sdks/recipes/javascript-ledger-flow
lang: ka
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload-ის იმპორტი '@site/src/components/SampleDownload'-დან;

ეს რეცეპტი იყენებს Node.js `@iroha2/torii-client` და
`@iroha2/crypto-target-node` პაკეტები CLI ledger-ის მიღწევის რეპროდუცირებისთვის.

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  ფაილის სახელი = "ledger-flow.mjs"
  description="ჩამოტვირთეთ ზუსტი JavaScript სკრიპტი, რომელიც გამოიყენება ამ წიგნის სახელმძღვანელოში."
/>

## წინაპირობები

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## მაგალითი სკრიპტი

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
    {Register: {assetDefinition: {numeric: {id: '7Sp2j6zDvJFnMoscAiMaWbWHRDBZ'}}}},
    {Mint: {asset: {id: `norito:4e52543000000002`}, value: {quantity: '250'}}},
    {Transfer: {
      asset: {id: `norito:4e52543000000002`},
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
  if (asset.id.definition === '7Sp2j6zDvJFnMoscAiMaWbWHRDBZ') {
    console.log('Receiver holds', asset.value, 'units of', asset.id.definition);
  }
}
```

გაუშვით `node --env-file=.env ledger-flow.mjs`-ით (ან გარემოს ექსპორტი
ცვლადები ხელით). ჟურნალი უნდა აჩვენოს ტრანზაქციის ჰეში (ქვითიდან
payload) და განახლებული მიმღების ბალანსი.

## დაადასტურეთ პარიტეტი

- მიიღეთ ტრანზაქციის დეტალები `iroha --config defaults/client.toml transaction get --hash <hash>`-ის საშუალებით.
- გადაამოწმეთ ნაშთები `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`-ით.
- შეადარეთ გამოშვებული ჰეში Rust-ისა და Python-ის რეცეპტებთან, რათა უზრუნველყოთ SDK პარიტეტი.