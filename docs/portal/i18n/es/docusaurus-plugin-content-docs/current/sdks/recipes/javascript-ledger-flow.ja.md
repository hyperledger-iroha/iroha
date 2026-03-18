---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 36d68144cef2903e8b056f9ce0db73c22e810ed6cfeffda3384f0928fdea5d44
source_last_modified: "2026-01-30T14:57:12+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-ledger-flow
title: Receta del flujo del libro mayor en JavaScript
description: Registra un activo, acuña, transfiere y consulta saldos con `@iroha2/torii-client`.
---

import SampleDownload from '@site/src/components/SampleDownload';

Esta receta usa los paquetes de Node.js `@iroha2/torii-client` y `@iroha2/crypto-target-node` para reproducir el recorrido del libro mayor de la CLI.

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="Descarga el script exacto de JavaScript usado en este recorrido del libro mayor."
/>

## Prerequisitos

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Script de ejemplo

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

Ejecuta `node --env-file=.env ledger-flow.mjs` (o exporta las variables de entorno manualmente). El log debe mostrar el hash de la transacción (del payload del recibo) y el saldo actualizado del receptor.

## Verifica la paridad

- Obtén los detalles de la transacción mediante `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Contrasta los saldos con `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- Compara el hash emitido con las recetas de Rust y Python para garantizar la paridad de los SDK.
