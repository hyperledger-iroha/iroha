---
lang: ba
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

импорт SapleDownload '@site/src/компоненттар/SampleDownload';

Был рецепт Node.js `@iroha2/torii-client` һәм
I18NI000000003Х пакеттары CLI леджер проходкаһын ҡабатлау өсөн.

<СэмплДау-лог
  href="/sdk-рецепттары/жаваскрипт/леджер-ағым.mjs".
  файл исеме="милли-ағым.мжс".
  тасуирлама="Скачать теүәл JavaScript скрипты ҡулланылған был леджер проходка."
/>

## Алдан шарттар

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Миҫал сценарийы

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

I18NI000000004X менән йүгерергә (йәки тирә-яҡ мөхитте экспортлау
үҙгәртеүселәр ҡул менән). Бүрән транзакция хеш күрһәтергә тейеш (квитанциянан
файҙалы йөк) һәм яңыртылған приемник балансы.

## Паритетты раҫлау

- `iroha --config defaults/client.toml transaction get --hash <hash>` аша транзакция реквизиттарын алыу.
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` менән крест-тикшерергә.
- SDK паритетын тәьмин итеү өсөн ристь һәм Python рецептары менән сығарылған хешты сағыштырырға.