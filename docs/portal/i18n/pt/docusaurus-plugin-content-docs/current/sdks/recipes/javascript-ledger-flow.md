---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-ledger-flow
title: Receita de fluxo do ledger em JavaScript
description: Registre um ativo, cunhe, transfira e consulte saldos com `@iroha2/torii-client`.
---

import SampleDownload from '@site/src/components/SampleDownload';

Esta receita usa os pacotes Node.js `@iroha2/torii-client` e `@iroha2/crypto-target-node` para reproduzir o passo a passo do ledger da CLI.

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="Baixe o script JavaScript exato usado neste passo a passo do ledger."
/>

## Pré-requisitos

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Script de exemplo

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

Execute `node --env-file=.env ledger-flow.mjs` (ou exporte as variáveis de ambiente manualmente). O log deve mostrar o hash da transação (do payload do recibo) e o saldo atualizado do destinatário.

## Verifique a paridade

- Busque os detalhes da transação via `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Confira os saldos com `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- Compare o hash emitido com as receitas de Rust e Python para garantir a paridade dos SDKs.
