---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-ledger-flow
title: Рецепт потока реестра на JavaScript
description: Зарегистрируйте актив, выпустите, переведите и запросите балансы с `@iroha2/torii-client`.
---

import SampleDownload from '@site/src/components/SampleDownload';

Этот рецепт использует пакеты Node.js `@iroha2/torii-client` и `@iroha2/crypto-target-node`, чтобы воспроизвести пошаговый проход по реестру в CLI.

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="Скачайте точный JavaScript-скрипт, используемый в этом проходе по реестру."
/>

## Предварительные требования

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Пример скрипта

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

Запустите `node --env-file=.env ledger-flow.mjs` (или экспортируйте переменные окружения вручную). В журнале должен появиться хеш транзакции (из payload квитанции) и обновлённый баланс получателя.

## Проверьте паритет

- Получите детали транзакции через `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Сверьте балансы с `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- Сравните полученный хеш с рецептами Rust и Python, чтобы подтвердить паритет SDK.
