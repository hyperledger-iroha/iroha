---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 819e58e445a7d20cd85cc9738ea77a4d81aa3c330c232b76d1826d79d1221b58
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Reflete `docs/source/da/commitments_plan.md`. Gardez les deux versions en sync
:::

# Plan des engagements Data Availability Sora Nexus (DA-3)

_Redige: 2026-03-25 -- Responsables: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 etend le format de bloc Nexus pour que chaque lane integre des enregistrements
deterministes decrivant les blobs acceptes par DA-2. Cette note capture les
structures de donnees canoniques, les hooks du pipeline de blocs, les preuves de
client leger, et les surfaces Torii/RPC qui doivent arriver avant que les
validateurs puissent s'appuyer sur les engagements DA lors des checks
d'admission ou de gouvernance. Tous les payloads sont encodes en Norito; pas de
SCALE ni de JSON ad hoc.

## Objectifs

- Porter des engagements par blob (chunk root + manifest hash + commitment KZG
  optionnel) dans chaque bloc Nexus afin que les pairs puissent reconstruire
  l'etat d'availability sans consulter le stockage hors ledger.
- Fournir des preuves de membership deterministes afin que les clients legers
  verifient qu'un manifest hash a ete finalise dans un bloc donne.
- Exposer des requetes Torii (`/v1/da/commitments/*`) et des preuves permettant
  aux relays, SDKs et automatisations de gouvernance d'auditer l'availability
  sans rejouer chaque bloc.
- Conserver l'enveloppe `SignedBlockWire` canonique en transmettant les nouvelles
  structures via le header de metadata Norito et la derivation du hash de bloc.

## Vue d'ensemble du scope

1. **Ajouts au data model** dans `iroha_data_model::da::commitment` plus
   modifications du header de bloc dans `iroha_data_model::block`.
2. **Hooks d'executor** pour que `iroha_core` ingeste les receipts DA emis par
   Torii (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **Persistence/indexes** afin que le WSV reponde rapidement aux requetes de
   commitments (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** pour les endpoints de liste/lecture/proof sous
   `/v1/da/commitments`.
5. **Tests d'integration + fixtures** validant le wire layout et le flux de proof
   dans `integration_tests/tests/da/commitments.rs`.

## 1. Ajouts au data model

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` reutilise le point 48 octets utilise dans `iroha_crypto::kzg`.
  Quand il est absent, on retombe sur des preuves Merkle uniquement.
- `proof_scheme` derive du catalog de lanes; les lanes Merkle rejettent les
  payloads KZG tandis que les lanes `kzg_bls12_381` exigent des commitments KZG
  non nuls. Torii ne produit actuellement que des commitments Merkle et rejette
  les lanes configurees en KZG.
- `KzgCommitment` reutilise le point 48 octets utilise dans `iroha_crypto::kzg`.
  Quand il est absent sur les lanes Merkle on retombe sur des preuves Merkle
  uniquement.
- `proof_digest` anticipe l'integration DA-5 PDP/PoTR afin que le meme record
  enumere le schedule de sampling utilise pour maintenir les blobs en vie.

### 1.2 Extension du header de bloc

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

Le hash du bundle entre a la fois dans le hash de bloc et dans la metadata de
`SignedBlockWire`. Quand un bloc ne transporte pas de donnees DA, le champ reste

Note d'implementation: `BlockPayload` et le `BlockBuilder` transparent exposent
maintenant des setters/getters `da_commitments` (voir
`BlockBuilder::set_da_commitments` et `SignedBlock::set_da_commitments`), donc
les hosts peuvent attacher un bundle preconstruit avant de sceller un bloc. Tous
les helpers laissent le champ a `None` tant que Torii ne fournit pas de bundles
reels.

### 1.3 Encodage wire

- `SignedBlockWire::canonical_wire()` ajoute le header Norito pour
  `DaCommitmentBundle` immediatement apres la liste de transactions existante.
  Le byte de version est `0x01`.
- `SignedBlockWire::decode_wire()` rejette les bundles dont `version` est
  inconnue, en ligne avec la politique Norito decrite dans `norito.md`.
- Les mises a jour de derivation du hash vivent uniquement dans `block::Hasher`;
  les clients legers qui decodent le wire format existant gagnent le nouveau
  champ automatiquement car le header Norito annonce sa presence.

## 2. Flux de production de blocs

1. L'ingest DA Torii finalise un `DaIngestReceipt` et le publie sur la queue
   interne (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` collecte tous les receipts dont `lane_id` correspond au bloc
   en construction, en dedupliquant par `(lane_id, client_blob_id,
   manifest_hash)`.
3. Juste avant le scellage, le builder trie les commitments par `(lane_id, epoch,
   sequence)` pour garder le hash deterministe, encode le bundle avec le codec
   Norito, et met a jour `da_commitments_hash`.
4. Le bundle complet est stocke dans le WSV et emis avec le bloc dans
   `SignedBlockWire`.

Si la creation du bloc echoue, les receipts restent dans la queue pour que la
prochaine tentative les reprenne; le builder enregistre le dernier `sequence`
inclus par lane afin d'eviter les attaques de replay.

## 3. Surface RPC et requetes

Torii expose trois endpoints:

| Route | Methode | Payload | Notes |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtre de range lane/epoch/sequence, pagination) | Renvoie `DaCommitmentPage` avec total, commitments et hash de bloc. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash ou tuple `(epoch, sequence)`). | Repond avec `DaCommitmentProof` (record + chemin Merkle + hash de bloc). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless qui rejoue le calcul du hash de bloc et valide l'inclusion; utilise par les SDKs qui ne peuvent pas lier directement `iroha_crypto`. |

Tous les payloads vivent sous `iroha_data_model::da::commitment`. Les routeurs
Torii montent les handlers a cote des endpoints d'ingest DA existants pour
reutiliser les politiques token/mTLS.

## 4. Preuves d'inclusion et clients legers

- Le producteur de bloc construit un arbre Merkle binaire sur la liste
  serialisee des `DaCommitmentRecord`. La racine alimente `da_commitments_hash`.
- `DaCommitmentProof` emballe le record cible plus un vecteur de
  `(sibling_hash, position)` afin que les verificateurs puissent reconstruire la
  racine. Les preuves incluent aussi le hash de bloc et le header signe pour que
  les clients legers puissent verifier la finalite.
- Les helpers CLI (`iroha_cli app da prove-commitment`) enveloppent le cycle
  request/verify et exposent des sorties Norito/hex pour les operateurs.

## 5. Storage et indexation

Le WSV stocke les commitments dans une column family dediee, cle par
`manifest_hash`. Des indexes secondaires couvrent `(lane_id, epoch)` et
`(lane_id, sequence)` afin que les requetes eviten de scanner des bundles
complets. Chaque record suit la hauteur de bloc qui l'a scelle, permettant aux
noeuds en rattrapage de reconstruire l'index rapidement a partir du block log.

## 6. Telemetrie et observabilite

- `torii_da_commitments_total` incremente des qu'un bloc scelle au moins un
  record.
- `torii_da_commitment_queue_depth` suit les receipts en attente de bundle
  (par lane).
- Le dashboard Grafana `dashboards/grafana/da_commitments.json` visualise
  l'inclusion de blocs, la profondeur de queue, et le throughput de preuve pour
  que les gates de release DA-3 puissent auditer le comportement.

## 7. Strategie de tests

1. **Tests unitaires** pour l'encoding/decoding de `DaCommitmentBundle` et les
   mises a jour de derivation de hash de bloc.
2. **Fixtures golden** sous `fixtures/da/commitments/` capturant les bytes
   canoniques du bundle et les preuves Merkle.
3. **Tests d'integration** demarrant deux validateurs, ingestant des blobs de
   test, et verifiant que les deux noeuds concordent sur le contenu du bundle et
   les reponses de query/proof.
4. **Tests light-client** dans `integration_tests/tests/da/commitments.rs`
   (Rust) qui appellent `/prove` et verifient la preuve sans parler a Torii.
5. **Smoke CLI** avec `scripts/da/check_commitments.sh` pour garder l'outillage
   operateur reproductible.

## 8. Plan de rollout

| Phase | Description | Exit Criteria |
|-------|-------------|---------------|
| P0 - Merge du data model | Integrer `DaCommitmentRecord`, mises a jour du header de bloc et codecs Norito. | `cargo test -p iroha_data_model` vert avec nouvelles fixtures. |
| P1 - Wiring Core/WSV | Cablage logique de queue + block builder, persister les indexes et exposer les handlers RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passent avec assertions de bundle proof. |
| P2 - Tooling operateur | Livrer helpers CLI, dashboard Grafana, et mises a jour des docs de verification de proof. | `iroha_cli app da prove-commitment` fonctionne sur devnet; le dashboard affiche des donnees live. |
| P3 - Gate de gouvernance | Activer le validateur de blocs requerant les commitments DA sur les lanes signalees dans `iroha_config::nexus`. | Entree de status + update de roadmap marquent DA-3 comme TERMINE. |

## Questions ouvertes

1. **KZG vs Merkle defaults** - Doit-on toujours ignorer les commitments KZG pour
   les petits blobs afin de reduire la taille des blocs? Proposition: garder
   `kzg_commitment` optionnel et le gater via `iroha_config::da.enable_kzg`.
2. **Sequence gaps** - Autorise-t-on des lanes hors ordre? Le plan actuel rejette
   les gaps sauf si la gouvernance active `allow_sequence_skips` pour un replay
   d'urgence.
3. **Light-client cache** - L'equipe SDK a demande un cache SQLite leger pour les
   proofs; suivi en attente sous DA-8.

Repondre a ces questions dans les PRs d'implementation fera passer DA-3 de
BROUILLON (ce document) a EN COURS des que le travail de code commence.
