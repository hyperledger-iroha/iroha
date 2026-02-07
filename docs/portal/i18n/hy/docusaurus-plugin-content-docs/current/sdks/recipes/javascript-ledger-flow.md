---
slug: /sdks/recipes/javascript-ledger-flow
lang: hy
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ներմուծել SampleDownload-ից '@site/src/components/SampleDownload';

Այս բաղադրատոմսը օգտագործում է Node.js `@iroha2/torii-client` և
`@iroha2/crypto-target-node` փաթեթներ՝ CLI մատյանային շրջադարձը վերարտադրելու համար:

<Նմուշի ներբեռնում
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename = "ledger-flow.mjs"
  description="Ներբեռնեք ճշգրիտ JavaScript սկրիպտը, որն օգտագործվում է այս մատյանում:"
/>

## Նախադրյալներ

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Օրինակ սցենար

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

Գործարկեք `node --env-file=.env ledger-flow.mjs`-ով (կամ արտահանեք շրջակա միջավայրը
փոփոխականները ձեռքով): Գրանցամատյանը պետք է ցույց տա գործարքի հեշը (անդորրագրից
օգտակար բեռ) և ստացողի թարմացված մնացորդը:

## Ստուգեք հավասարությունը

- Ստացեք գործարքի մանրամասները `iroha --config defaults/client.toml transaction get --hash <hash>`-ի միջոցով:
- Խաչաձև ստուգեք մնացորդները `iroha --config defaults/client.toml asset list filter '{"id":"coffee#wonderland##<account>"}'`-ով:
- Համեմատեք թողարկված հեշը Rust-ի և Python-ի բաղադրատոմսերի հետ՝ ապահովելու SDK-ի հավասարությունը: