---
lang: fr
direction: ltr
source: docs/portal/docs/da/commitments-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Sora Nexus Plan d'engagements en matière de disponibilité des données (DA-3)

_مسودہ : 2026-03-25 -- مالکان : Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 Nexus est une voie déterministe et déterministe
records d'enregistrement et de DA-2 pour les blobs d'enregistrement et d'enregistrement یہ نوٹ
structures de données canoniques, crochets de pipeline de bloc, preuves de clients légers, ici
Torii/RPC surfaces pour l'admission et la gouvernance
vérifie les engagements du DA پر بھروسہ کرنے سے پہلے لازمی ہیں۔ Charges utiles تمام
Code codé en Norito SCALE et JSON ad hoc

## مقاصد

- Nexus contient des engagements par blob (racine de morceau + hachage manifeste + facultatif
  Engagement KZG) شامل کرنا تاکہ peers stockage hors grand livre دیکھے بغیر
  état de disponibilité دوبارہ بنا سکیں۔
- les preuves d'adhésion déterministes pour les clients légers vérifient les clients légers
  Le hachage manifeste est terminé et finalisé
- Requêtes Torii (`/v1/da/commitments/*`) et preuves pour les relais
  SDK pour l'automatisation de la gouvernance et la relecture ainsi que la disponibilité et la disponibilité
  audit کر سکیں۔
- Enveloppe `SignedBlockWire` et structures canoniques et structures Norito
  en-tête de métadonnées et dérivation de hachage de bloc et fil de discussion

## Aperçu de la portée1. **Ajouts au modèle de données** Bloc `iroha_data_model::da::commitment` میں اور
   changements d'en-tête `iroha_data_model::block` میں۔
2. **Crochets d'exécuteur** تاکہ `iroha_core` Torii سے émettent شدہ DA reçus ingèrent
   Ici (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **Persistance/index** Les requêtes d'engagements WSV et la gestion des requêtes
   (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** liste/requête/prouver les points de terminaison
   `/v1/da/commitments` est en cours
5. **Tests d'intégration + montages** et disposition des fils et flux de preuve et validation
   `integration_tests/tests/da/commitments.rs` میں۔

## 1. Ajouts de modèles de données

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
```

- `KzgCommitment` prend en charge 48 octets pour la réutilisation et `iroha_crypto::kzg`
  میں ہے۔ جب absent ہو تو صرف Merkle proofs استعمال ہوتے ہیں۔
- Catalogue de voies `proof_scheme` et dériver ہوتا ہے؛ Charges utiles KZG des voies Merkle
  rejeter کرتی ہیں جبکہ `kzg_bls12_381` voies non nulles les engagements KZG exigent
  کرتی ہیں۔ Torii pour les engagements Merkle pour la configuration KZG
  voies et rejeter کرتا ہے۔
- `KzgCommitment` prend en charge 48 octets pour la réutilisation et `iroha_crypto::kzg`
  میں ہے۔ جب Merkle Lanes پر absent ہو تو صرف Merkle proofs استعمال ہوتے ہیں۔
- Intégration `proof_digest` DA-5 PDP/PoTR
  enregistrer le calendrier d'échantillonnage des blobs et des blobs
  ہوتا ہے۔

### 1.2 Extension d'en-tête de bloc

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```Bundle de hachage ou de hachage et de métadonnées `SignedBlockWire` disponible en ligne جب
چیں۔

Note d'implémentation : `BlockPayload` et transparent `BlockBuilder`.
`da_commitments` les setters/getters exposent les éléments de base (دیکھیں
`BlockBuilder::set_da_commitments` et `SignedBlock::set_da_commitments`)
hôtes بلاک seal ہونے سے پہلے pré-construit bundle attach کر سکیں۔ تمام assistant
champ des constructeurs `None` est un fil de discussion pour Torii.

### 1.3 Codage des fils

- `SignedBlockWire::canonical_wire()` Liste des transactions en cours
  `DaCommitmentBundle` en-tête Norito ajouté en haut octet de version `0x01` ہے۔
- `SignedBlockWire::decode_wire()` inconnu `version` et bundles et rejets
  Politique `norito.md` et Norito pour la France
- Mises à jour de la dérivation de hachage `block::Hasher` میں ہیں؛ format de fil existant
  décoder les clients légers et les clients légers field حاصل کرتے ہیں کیونکہ
  En-tête Norito pour le téléchargement en ligne

## 2. Bloquer le flux de production1. Torii DA ingère et `DaIngestReceipt` finalise la file d'attente interne
   پر publier کرتا ہے (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` reçus pour les reçus en ligne `lane_id` pour les reçus en ligne
   match کرتا ہے، اور `(lane_id, client_blob_id, manifest_hash)` pour dédupliquer
   کرتا ہے۔
3. Sceau des engagements du constructeur sous `(lane_id, epoch, sequence)`
   trier les fichiers par hachage déterministe et bundle avec le codec Norito et encoder
   Mise à jour `da_commitments_hash` Mise à jour en ligne
4. Le bundle WSV est disponible dans le magasin `SignedBlockWire` pour le magasin.
   émettre ہوتا ہے۔

اگر بلاک بنانا échouer ہو تو file d'attente des reçus میں رہتے ہیں تاکہ اگلی کوشش انہیں لے
سکے؛ constructeur ہر lane کیلئے آخری شامل شدہ `sequence` ریکارڈ کرتا ہے تاکہ replay
attaques سے بچا جا سکے۔

## 3. RPC et surface de requête

Torii contient les points de terminaison suivants :| Itinéraire | Méthode | Charge utile | Remarques |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtre de voie/époque/plage de séquence, pagination) | `DaCommitmentPage` indique le nombre total d'engagements et le hachage de bloc |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (voie + hachage manifeste یا Tuple `(epoch, sequence)`)۔ | `DaCommitmentProof` واپس کرتا ہے (enregistrement + chemin Merkle + hachage de bloc)۔ |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Assistant apatride et calcul de hachage de bloc pour la validation de l'inclusion et la validation Les SDK sont basés sur `iroha_crypto` et le lien ci-dessous est disponible. |

Charges utiles `iroha_data_model::da::commitment` pour les charges utiles Routeurs Torii
gestionnaires pour les points de terminaison d'ingestion DA et pour le montage de points de terminaison pour le jeton/mTLS
les politiques de réutilisation ہوں۔

## 4. Preuves d'inclusion et clients légers

- Liste des producteurs sérialisés `DaCommitmentRecord` pour l'arbre Merkle binaire
  ہے۔ Il s'agit de la racine `da_commitments_hash` et du flux de données.
- Enregistrement cible `DaCommitmentProof` et vecteur `(sibling_hash, position)`
  پیک کرتا ہے تاکہ verifiers root دوبارہ بنا سکیں۔ preuves میں block hash اور
  en-tête signé pour la vérification de la finalité des clients légers
- Aides CLI (`iroha_cli app da prove-commitment`) cycle de demande/vérification de preuve
  envelopper les opérateurs et les opérateurs Norito/sorties hexadécimales

## 5. Stockage et indexationEngagements WSV et famille de colonnes dédiées et clé `manifest_hash`.
magasin کرتا ہے۔ Index secondaires `(lane_id, epoch)` et `(lane_id, sequence)`
کو cover کرتے ہیں تاکہ requêtes پورے bundles scan نہ کریں۔ ہر enregistrer اس bloquer
hauteur کو piste کرتا ہے جس نے اسے joint کیا، جس سے nœuds de rattrapage bloc journal سے
index تیزی سے reconstruire کر سکتے ہیں۔

## 6. Télémétrie et observabilité

- `torii_da_commitments_total` incrément d'incrémentation de la valeur de l'incrémentation
  sceau d'enregistrement کرے۔
- `torii_da_commitment_queue_depth` bundle pour les reçus et les reçus
  piste کرتا ہے (par voie)۔
- Inclusion du bloc Grafana du tableau de bord `dashboards/grafana/da_commitments.json`,
  profondeur de file d'attente et débit de preuve et portes de libération DA-3
  audit de comportement کر سکیں۔

## 7. Stratégie de test

1. Encodage/décodage `DaCommitmentBundle` et mises à jour de dérivation de hachage de bloc
   **tests unitaires**۔
2. `fixtures/da/commitments/` میں **fixations dorées** et octets du paquet canonique
   Les preuves Merkle capturent کرتے ہیں۔
3. **Tests d'intégration** et les validateurs démarrent et ingèrent des échantillons de blobs
   Vérifier le contenu du bundle et les réponses aux requêtes/preuves et vérifier les réponses
   ہیں۔
4. **Tests client léger** `integration_tests/tests/da/commitments.rs` (Rouille)
   Il s'agit d'un appel `/prove` et d'un Torii pour une vérification de preuve.
5. Script **CLI smoke** `scripts/da/check_commitments.sh` pour l'outillage de l'opérateur
   reproductible

## 8. Plan de déploiement| Phases | Descriptif | Critères de sortie |
|-------|-------------|--------------------|
| P0 — Fusion de modèles de données | `DaCommitmentRecord`, mises à jour de l'en-tête de bloc et des codecs Norito atterrissent ici | `cargo test -p iroha_data_model` luminaires vert vert |
| P1 — Câblage noyau/WSV | file d'attente + thread logique du générateur de blocs les index persistent et les gestionnaires RPC exposent les fichiers | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` bundle assertions de preuve et réussite |
| P2 — Outillage de l'opérateur | Aides CLI, tableau de bord Grafana, et documents de vérification des preuves expédiés ici | `iroha_cli app da prove-commitment` Devnet est disponible données en direct du tableau de bord |
| P3 — Porte de gouvernance | `iroha_config::nexus` Pour les voies signalées et les engagements DA nécessitent l'activation du validateur de bloc. | entrée du statut + mise à jour de la feuille de route DA-3 et marque complète |

## Questions ouvertes

1. ** Valeurs par défaut KZG vs Merkle ** — Les blobs et les engagements KZG sont également disponibles.
   La taille des blocs est définie par la taille du bloc. Modèle : `kzg_commitment` optionnel
   Pour `iroha_config::da.enable_kzg` porte d'entrée
2. **Écarts de séquence** — Pour les voies dans le désordre et pour les problèmes موجودہ پلان lacunes کو
   rejeter la procédure de gouvernance `allow_sequence_skips` et la rediffusion d'urgence
   activer نہ کرے۔
3. **Cache client léger** — SDK pour toutes les preuves de cache SQLite
   ہے؛ DA-8 میں suivi باقی ہے۔Les PR de mise en œuvre des PR sont en cours pour le DA-3 et la mise en œuvre des PR.
"en cours" est un travail de code en cours