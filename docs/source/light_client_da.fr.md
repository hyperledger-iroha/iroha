---
lang: fr
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Échantillonnage léger de la disponibilité des données des clients

L'API Light Client Sampling permet aux opérateurs authentifiés de récupérer
Échantillons de fragments de RBC authentifiés par Merkle pour un bloc en vol. Clients légers
peut émettre des demandes d'échantillonnage aléatoires, vérifier les épreuves retournées par rapport au
racine de bloc annoncée et renforcez l'assurance que les données sont disponibles sans
récupérer la totalité de la charge utile.

## Point de terminaison

```
POST /v2/sumeragi/rbc/sample
```

Le point de terminaison nécessite un en-tête `X-API-Token` correspondant à l'un des éléments configurés.
Jetons API Torii. Les demandes sont en outre limitées et soumises à un tarif quotidien.
budget d'octets par appelant ; dépasser l’un ou l’autre renvoie HTTP 429.

### Corps de la demande

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – hachage du bloc cible en hexadécimal.
* `height`, `view` – identifiant le tuple pour la session RBC.
* `count` – nombre d'échantillons souhaité (par défaut à 1, plafonné par la configuration).
* `seed` – graine RNG déterministe en option pour un échantillonnage reproductible.

### Corps de réponse

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

Chaque exemple d'entrée contient l'index de bloc, les octets de charge utile (hex), la feuille SHA-256
digest, et une preuve d'inclusion Merkle (avec des frères et sœurs facultatifs codés en hexadécimal
cordes). Les clients peuvent vérifier les preuves en utilisant le champ `chunk_root`.

## Limites et budgets

* **Nombre maximum d'échantillons par demande** – configurable via `torii.rbc_sampling.max_samples_per_request`.
* **Nombre maximum d'octets par requête** – appliqué à l'aide de `torii.rbc_sampling.max_bytes_per_request`.
* **Budget quotidien en octets** – suivi par appelant via `torii.rbc_sampling.daily_byte_budget`.
* **Limitation de débit** – appliquée à l'aide d'un compartiment de jetons dédié (`torii.rbc_sampling.rate_per_minute`).

Les requêtes dépassant toute limite renvoient HTTP 429 (CapacityLimit). Quand le morceau
le magasin n'est pas disponible ou la session manque d'octets de charge utile au point de terminaison
renvoie HTTP 404.

## Intégration du SDK

###JavaScript

`@iroha/iroha-js` expose l'assistant `ToriiClient.sampleRbcChunks` afin que les données
les vérificateurs de disponibilité peuvent appeler le point de terminaison sans lancer leur propre récupération
logique. L'assistant valide les charges utiles hexadécimales, normalise les entiers et renvoie
objets typés qui reflètent le schéma de réponse ci-dessus :

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("RBC session is not available yet");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

L'assistant se lance lorsque le serveur renvoie des données mal formées, aidant ainsi la parité JS-04
les tests détectent les régressions aux côtés des SDK Rust et Python. Rouille
(`iroha_client::ToriiClient::sample_rbc_chunks`) et Python
(`IrohaToriiClient.sample_rbc_chunks`) expédier des aides équivalentes ; utiliser n'importe lequel
correspond à votre harnais d’échantillonnage.