---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fce6fbde7987c30354be74cb32984189b4bdb91341c052012eec37cf78730b32
source_last_modified: "2025-11-11T10:22:54.086603+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Recette de flux du registre JavaScript
description: Enregistrer un actif, frapper, transférer et interroger les soldes avec `@iroha2/torii-client`.
slug: /sdks/recipes/javascript-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

Cette recette utilise les paquets Node.js `@iroha2/torii-client` et `@iroha2/crypto-target-node` pour reproduire le parcours du registre dans la CLI.

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="Téléchargez le script JavaScript exact utilisé dans ce parcours du registre."
/>

## Prérequis

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Script d'exemple

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

Exécutez `node --env-file=.env ledger-flow.mjs` (ou exportez les variables d’environnement manuellement). Le journal doit afficher le hash de transaction (depuis le payload du reçu) et le solde mis à jour du destinataire.

## Vérifier la parité

- Récupérez les détails de la transaction via `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Vérifiez les soldes avec `iroha --config defaults/client.toml asset list filter '{"id":"coffee#wonderland##<account>"}'`.
- Comparez le hash émis avec les recettes Rust et Python pour garantir la parité des SDK.
