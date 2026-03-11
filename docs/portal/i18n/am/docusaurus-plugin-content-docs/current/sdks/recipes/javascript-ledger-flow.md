---
slug: /sdks/recipes/javascript-ledger-flow
lang: am
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ናሙና አውርድን ከ'@site/src/components/SampleDownload' አስመጣ;

ይህ የምግብ አሰራር Node.js `@iroha2/torii-client` እና ይጠቀማል
`@iroha2/crypto-target-node` ጥቅሎች የCLI ደብተር መራመጃን ለማባዛት።

<ናሙና አውርድ
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="በዚህ መዝገብ ውስጥ ጥቅም ላይ የዋለውን ትክክለኛውን የጃቫ ስክሪፕት ስክሪፕት አውርድ።"
/>

## ቅድመ ሁኔታዎች

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## ምሳሌ ስክሪፕት።

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

ከ `node --env-file=.env ledger-flow.mjs` ጋር ያሂዱ (ወይም አካባቢውን ወደ ውጭ ይላኩ
ተለዋዋጮች በእጅ)። ምዝግብ ማስታወሻው የግብይቱን ሃሽ (ከደረሰኙ) ማሳየት አለበት።
ክፍያ) እና የተሻሻለው የተቀባዩ ቀሪ ሂሳብ።

## ተመሳሳይነት ያረጋግጡ

- የግብይቱን ዝርዝሮች በ `iroha --config defaults/client.toml transaction get --hash <hash>` በኩል ያግኙ።
- ሂሳቦችን ከ `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` ጋር ያረጋግጡ።
- የኤስዲኬን እኩልነት ለማረጋገጥ የወጣውን ሃሽ ከ Rust እና Python አዘገጃጀት ጋር ያወዳድሩ።