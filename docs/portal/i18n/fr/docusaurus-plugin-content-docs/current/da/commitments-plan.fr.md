---
lang: fr
direction: ltr
source: docs/portal/docs/da/commitments-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Reflète `docs/source/da/commitments_plan.md`. Gardez les deux versions en synchronisation
:::

# Plan des engagements Disponibilité des données Sora Nexus (DA-3)

_Redige : 2026-03-25 -- Responsables : Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 étend le format de bloc Nexus pour que chaque voie intègre des enregistrements
déterministes décrivant les blobs accepte par DA-2. Cette note capture les
structures de données canoniques, les hooks du pipeline de blocs, les preuves de
client léger, et les surfaces Torii/RPC qui doivent arriver avant que les
les validateurs peuvent s'appuyer sur les engagements DA lors des chèques
d'admission ou de gouvernance. Toutes les charges utiles sont encodées en Norito; pas de
SCALE ni de JSON ad hoc.

## Objectifs- Porter des engagements par blob (chunk root + manifest hash + engagement KZG
  optionnel) dans chaque bloc Nexus afin que les paires puissent reconstruire
  l'état de disponibilité sans consulter le stockage hors grand livre.
- Fournir des preuves d'appartenance déterministes afin que les clients légers
  vérifiez qu'un manifest hash a été finalisé dans un bloc donné.
- Exposer des requêtes Torii (`/v1/da/commitments/*`) et des preuves permettant
  aux relais, SDK et automatisations de gouvernance d'auditer la disponibilité
  sans rejouer chaque bloc.
- Conserver l'enveloppe `SignedBlockWire` canonique en transmettant les nouvelles
  structures via le header de métadonnées Norito et la dérivation du hash de bloc.

## Vue d'ensemble du scope

1. **Ajouts au data model** dans `iroha_data_model::da::commitment` plus
   modifications du header de bloc dans `iroha_data_model::block`.
2. **Hooks d'exécuteur** pour que `iroha_core` ingère les reçus DA émis par
   Torii (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **Persistence/indexes** afin que le WSV réponde rapidement aux requêtes de
   engagements (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** pour les endpoints de liste/lecture/proof sous
   `/v1/da/commitments`.
5. **Tests d'intégration + montages** validant le câblage et le flux de preuve
   dans `integration_tests/tests/da/commitments.rs`.

## 1. Ajouts au modèle de données

### 1.1`DaCommitmentRecord`

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
```- `KzgCommitment` réutilise le point 48 octets utilisé dans `iroha_crypto::kzg`.
  Quand il est absent, on retombe sur des preuves Merkle uniquement.
- `proof_scheme` dérive du catalogue de voies ; les Lanes Merkle rejettent les
  payloads KZG tandis que les voies `kzg_bls12_381` exigent des engagements KZG
  non nuls. Torii ne produit actuellement que des engagements Merkle et rejette
  les voies configurées en KZG.
- `KzgCommitment` réutilise le point 48 octets utilisé dans `iroha_crypto::kzg`.
  Quand il est absent sur les voies Merkle on retombe sur des preuves Merkle
  uniquement.
- `proof_digest` anticipe l'intégration DA-5 PDP/PoTR afin que le meme record
  énumérer le planning d’échantillonnage utilisé pour maintenir les blobs en vie.

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

Le hash du bundle entre à la fois dans le hash de bloc et dans les métadonnées de
`SignedBlockWire`. Quand un bloc ne transporte pas de données DA, le champ reste

Note d'implémentation : `BlockPayload` et le `BlockBuilder` exposant transparent
maintenant des setters/getters `da_commitments` (voir
`BlockBuilder::set_da_commitments` et `SignedBlock::set_da_commitments`), donc
les hôtes peuvent attacher un bundle préconstruit avant de sceller un bloc. Tous
les helpers laissent le champ a `None` tant que Torii ne fournit pas de bundles
bobines.

### 1.3 Fil d'encodage- `SignedBlockWire::canonical_wire()` ajoute le header Norito pour
  `DaCommitmentBundle` immédiatement après la liste des transactions existantes.
  L’octet de version est `0x01`.
- `SignedBlockWire::decode_wire()` rejette les bundles dont `version` est
  inconnue, en ligne avec la politique Norito décrite dans `norito.md`.
- Les mises à jour de dérivation du hash vivent uniquement dans `block::Hasher`;
  les clients légers qui décodent le format wire existant gagnent le nouveau
  champ automatiquement car le header Norito annonce sa présence.

## 2. Flux de production de blocs

1. L'ingest DA Torii finalise un `DaIngestReceipt` et le publie sur la file d'attente
   interne (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` collecte tous les reçus dont `lane_id` correspondent au bloc
   en construction, en dédupliquant par `(lane_id, client_blob_id,
   manifest_hash)`.
3. Juste avant le scénario, le constructeur trie les engagements par `(lane_id, epoch,
   séquence)` pour garder le hash déterministe, encoder le bundle avec le codec
   Norito, et rencontré un jour `da_commitments_hash`.
4. Le bundle complet est stocké dans le WSV et émis avec le bloc dans
   `SignedBlockWire`.

Si la création du bloc échoue, les reçus restent dans la file d'attente pour que la
prochaine tentative les reprennes; le constructeur enregistre le dernier `sequence`
inclus par voie afin d'éviter les attaques de replay.## 3. Surface RPC et requêtes

Torii expose trois points de terminaison :

| Itinéraire | Méthode | Charge utile | Remarques |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtre de range lane/epoch/sequence, pagination) | Renvoie `DaCommitmentPage` avec total, engagements et hash de bloc. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (voie + hachage manifeste ou tuple `(epoch, sequence)`). | Répondez avec `DaCommitmentProof` (record + chemin Merkle + hash de bloc). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless qui rejoue le calcul du hash de bloc et valide l'inclusion; utiliser par les SDK qui ne peuvent pas lier directement `iroha_crypto`. |

Tous les charges utiles vivent sous `iroha_data_model::da::commitment`. Les routeurs
Torii monter les handlers a cote des endpoints d'ingest DA existants pour
réutiliser les politiques token/mTLS.

## 4. Preuves d'inclusion et clients légers- Le producteur de bloc construit un arbre Merkle binaire sur la liste
  numéro de série des `DaCommitmentRecord`. La racine alimentaire `da_commitments_hash`.
- `DaCommitmentProof` emballe le record cible plus un vecteur de
  `(sibling_hash, position)` afin que les vérificateurs puissent reconstruire la
  racine. Les preuves incluent également le hash de bloc et le header signe pour que
  les clients légers peuvent vérifier la finalité.
- Les helpers CLI (`iroha_cli app da prove-commitment`) enveloppent le cycle
  request/verify et exposant des sorties Norito/hex pour les opérateurs.

## 5. Stockage et indexation

Le WSV stocke les engagements dans une chronique famille dédiée, cle par
`manifest_hash`. Des index secondaires couvrent `(lane_id, epoch)` et
`(lane_id, sequence)` afin que les requêtes évitent de scanner des bundles
complète. Chaque disque convient à la hauteur de bloc qui l'a scelle, permettant aux
noeuds en rattrapage de reconstruire l'index rapidement à partir du block log.

## 6. Télémétrie et observabilité

- `torii_da_commitments_total` incrémente des qu'un bloc scelle au moins un
  enregistrer.
- `torii_da_commitment_queue_depth` suit les reçus en attente de bundle
  (par voie).
- Le tableau de bord Grafana `dashboards/grafana/da_commitments.json` visualise
  l'inclusion de blocs, la profondeur de file d'attente, et le débit de preuve pour
  que les portes de release DA-3 peuvent auditer le comportement.

## 7. Stratégie de tests1. **Tests unitaires** pour l'encodage/décodage de `DaCommitmentBundle` et les
   mises à jour de dérivation de hash de bloc.
2. **Fixtures golden** sous `fixtures/da/commitments/` capturant les octets
   canoniques du bundle et les preuves Merkle.
3. **Tests d'intégration** démarrant deux validateurs, ingérant des blobs de
   test, et vérifiant que les deux nœuds concordent sur le contenu du bundle et
   les réponses de query/proof.
4. **Tests light-client** dans `integration_tests/tests/da/commitments.rs`
   (Rust) qui appelle `/prove` et vérifie la preuve sans parler à Torii.
5. **Smoke CLI** avec `scripts/da/check_commitments.sh` pour garder l'outillage
   opérateur reproductible.

## 8. Plan de déploiement| Phases | Descriptif | Critères de sortie |
|-------|-------------|--------------------|
| P0 - Fusionner le modèle de données | Intégrateur `DaCommitmentRecord`, mise à jour du header de bloc et codecs Norito. | `cargo test -p iroha_data_model` vert avec nouvelles luminaires. |
| P1 - Noyau de câblage/WSV | Cablage logique de file d'attente + block builder, persister les index et exposer les handlers RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passent avec assertions de bundle proof. |
| P2 - Opérateur outillage | Livrer helpers CLI, tableau de bord Grafana, et mises à jour des documents de vérification de preuve. | `iroha_cli app da prove-commitment` fonctionne sur devnet; le tableau de bord affiche des données en direct. |
| P3 - Porte de gouvernance | Activer le validateur de blocs requérant les engagements DA sur les voies signalées dans `iroha_config::nexus`. | Entrée de statut + mise à jour de la feuille de route marquent DA-3 comme TERMINE. |

## Questions ouvertes

1. **KZG vs Merkle defaults** - Doit-on toujours ignorer les engagements KZG pour
   les petits blobs afin de réduire la taille des blocs ? Proposition : garder
   `kzg_commitment` optionnel et le gater via `iroha_config::da.enable_kzg`.
2. **Écarts de séquence** - Autoriser-t-on des voies hors ordre ? Le plan actuel rejeté
   les écarts sauf si la gouvernance active `allow_sequence_skips` pour un replay
   d'urgence.
3. **Light-client cache** - L'équipe SDK demande un cache SQLite léger pour les fichiers
   preuves; suivi en attente sous DA-8.Répondre à ces questions dans les PRs d'implémentation fera passer DA-3 de
BROUILLON (ce document) à EN COURS des que le travail de code commence.