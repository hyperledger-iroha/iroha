---
lang: uz
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

SampleDownloadni '@site/src/components/SampleDownload'dan import qilish;

Bu retsept Node.js `@iroha2/torii-client` va foydalanadi
`@iroha2/crypto-target-node` to'plamlari CLI daftarini ko'paytirish uchun.

<Namunani yuklab olish
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  fayl nomi = "ledger-flow.mjs"
  description="Ushbu daftar ko'rsatmasida foydalanilgan aniq JavaScript skriptini yuklab oling."
/>

## Old shartlar

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Misol skript

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
  if (asset.id.definition === 'coffee#wonderland') {
    console.log('Receiver holds', asset.value, 'units of', asset.id.definition);
  }
}
```

`node --env-file=.env ledger-flow.mjs` bilan ishga tushiring (yoki muhitni eksport qiling
o'zgaruvchilar qo'lda). Jurnal tranzaksiya xeshini ko'rsatishi kerak (kvitansiyadan
foydali yuk) va qabul qiluvchining yangilangan balansi.

## Paritetni tekshiring

- Tranzaksiya tafsilotlarini `iroha --config defaults/client.toml transaction get --hash <hash>` orqali oling.
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` bilan balanslarni o'zaro tekshirish.
- SDK paritetini ta'minlash uchun chiqarilgan xeshni Rust va Python retseptlari bilan solishtiring.