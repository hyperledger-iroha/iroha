---
slug: /sdks/recipes/javascript-ledger-flow
lang: az
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload-ı '@site/src/components/SampleDownload'dan idxal edin;

Bu resept Node.js `@iroha2/torii-client` və istifadə edir
`@iroha2/crypto-target-node` paketləri CLI mühasibat dəftərini təkrar etmək üçün.

<Nümunə Yüklə
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  fayl adı = "ledger-flow.mjs"
  description="Bu kitabda istifadə olunan dəqiq JavaScript skriptini yükləyin."
/>

## İlkin şərtlər

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Nümunə skript

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

`node --env-file=.env ledger-flow.mjs` ilə işləyin (və ya ətraf mühiti ixrac edin
dəyişənləri əl ilə). Jurnal əməliyyat hashını göstərməlidir (qəbzdən
faydalı yük) və yenilənmiş qəbuledici balansı.

## Pariteti yoxlayın

- `iroha --config defaults/client.toml transaction get --hash <hash>` vasitəsilə əməliyyat təfərrüatlarını əldə edin.
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` ilə balansları çarpaz yoxlayın.
- SDK paritetini təmin etmək üçün buraxılan hashı Rust və Python reseptləri ilə müqayisə edin.