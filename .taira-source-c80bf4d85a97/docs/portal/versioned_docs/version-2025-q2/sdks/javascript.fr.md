---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Démarrage rapide du SDK JavaScript

`@iroha2/torii-client` fournit un navigateur et un wrapper convivial Node.js autour de Torii.
Ce démarrage rapide reflète les flux principaux des recettes du SDK afin que vous puissiez obtenir un
client exécuté dans quelques minutes. Pour des exemples plus complets, voir
`javascript/iroha_js/recipes/` dans le référentiel.

## 1. Installer

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Si vous envisagez de signer des transactions localement, installez également les assistants cryptographiques :

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Créez un client Torii

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

La configuration reflète le constructeur utilisé dans les recettes. Si votre nœud
utilise l'authentification de base, transmettez `{username, password}` via l'option `basicAuth`.

## 3. Récupérer l'état du nœud

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Toutes les opérations de lecture renvoient des objets JSON basés sur Norito. Voir les types générés dans
`index.d.ts` pour les détails du champ.

## 4. Soumettre une transaction

Les signataires peuvent créer des transactions avec l'API d'assistance :

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: '<i105-account-id>',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

L'assistant encapsule automatiquement la transaction dans l'enveloppe Norito attendue
par Torii. Pour un exemple plus riche (y compris les attentes de finalité), voir
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Utilisez des assistants de haut niveau

Le SDK regroupe des flux spécialisés qui reflètent la CLI :

- **Aide à la gouvernance** – `recipes/governance.mjs` démontre la mise en scène
  propositions et bulletins de vote avec les constructeurs d'instructions `governance`.
- **Pont ISO** – `recipes/iso_bridge.mjs` montre comment soumettre `pacs.008` et
  interroger l’état du transfert à l’aide des points de terminaison `/v1/iso20022`.
- **SoraFS et déclencheurs** – Les aides à la pagination sous `src/toriiClient.js` exposent
  itérateurs typés pour les contrats, les actifs, les déclencheurs et les fournisseurs SoraFS.

Importez les fonctions de générateur pertinentes à partir de `@iroha2/torii-client` pour réutiliser ces flux.

## 6. Gestion des erreurs

Tous les appels du SDK génèrent des instances `ToriiClientError` riches avec des métadonnées de transport
et la charge utile d'erreur Norito. Enveloppez les appels dans `try/catch` ou utilisez `.catch()` pour
contexte de surface aux utilisateurs :

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Étapes suivantes

- Explorez les recettes dans `javascript/iroha_js/recipes/` pour les flux de bout en bout.
- Lisez les types générés dans `javascript/iroha_js/index.d.ts` pour plus de détails.
  signatures de méthode.
- Associez ce SDK au démarrage rapide Norito pour inspecter et déboguer les charges utiles
  vous envoyez à Torii.