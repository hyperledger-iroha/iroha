---
lang: es
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fce6fbde7987c30354be74cb32984189b4bdb91341c052012eec37cf78730b32
source_last_modified: "2025-11-11T10:22:54.086603+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Receita de fluxo do ledger em JavaScript
description: Registre um ativo, cunhe, transfira e consulte saldos com `@iroha2/torii-client`.
slug: /sdks/recipes/javascript-ledger-flow
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
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
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

Execute `node --env-file=.env ledger-flow.mjs` (ou exporte as variáveis de ambiente manualmente). O log deve mostrar o hash da transação (do payload do recibo) e o saldo atualizado do destinatário.

## Verifique a paridade

- Busque os detalhes da transação via `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Confira os saldos com `iroha --config defaults/client.toml asset list filter '{"id":"coffee#wonderland##<account>"}'`.
- Compare o hash emitido com as receitas de Rust e Python para garantir a paridade dos SDKs.
