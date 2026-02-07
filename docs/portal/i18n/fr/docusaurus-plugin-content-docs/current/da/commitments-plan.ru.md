---
lang: fr
direction: ltr
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page correspond à `docs/source/da/commitments_plan.md`. Consultez les versions suivantes
:::

# Planifier les engagements Disponibilité des données Sora Nexus (DA-3)

_Черновик: 2026-03-25 — Groupes : Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 a créé le bloc de format Nexus, qui correspond à la voie utilisée
Déterminez les erreurs, analysez les blobs, utilisez DA-2. Dans ce document
зафиксированы канонические структуры данных, хуки блокового пайплайна,
Les clients et les fournisseurs de services Torii/RPC, dont les documents sont disponibles
donc, les validateurs doivent se renseigner sur les comités DA pour l'admission ou
проверках управления. Il s'agit de la charge utile Norito-кодированы ; sans SCALE et JSON ad hoc.

## Celi- Des commentaires sur le blob (racine de morceau + hachage manifeste + KZG optionnel
  engagement) dans le bloc Nexus, qui peut être reconstruit
  disponibilité de la garantie par rapport au stockage hors grand livre.
- Дать детерминированные preuves d'adhésion, чтобы light clients могли проверить,
  ce hachage manifeste est finalisé dans le bloc concret.
- Exportation des produits Torii (`/v1/da/commitments/*`) et preuves, livraison
  les relais, les SDK et la gouvernance d'automatisation vérifient la disponibilité sans remplacement
  blocs.
- Enveloppe canonique `SignedBlockWire`, nouvelle structure
  Recherchez l'en-tête de métadonnées Norito et le hachage du bloc de dérivation.

## Обзор области работ

1. **Modèle de données complet** dans `iroha_data_model::da::commitment` et définition
   en-tête de bloc `iroha_data_model::block`.
2. **Executor hooks** pour `iroha_core` ingérer les reçus DA, эмитированные
   Torii (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **Persistance/index** que WSV est en mesure de répondre aux requêtes d'engagement
   (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** pour la liste/requête/prouver les points de terminaison à la suite
   `/v1/da/commitments`.
5. **Tests d'intégration + montages** pour vérifier la disposition des fils et le flux de preuve
   `integration_tests/tests/da/commitments.rs`.

## 1. Modèle de données de mise à niveau

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
```- `KzgCommitment` utilise 48 touches à partir de `iroha_crypto::kzg`.
  En utilisant les preuves de Merkle.
- `proof_scheme` берется из каталога voies ; Les voies Merkle отклоняют les charges utiles KZG,
  Les voies `kzg_bls12_381` требуют ненулевых les engagements KZG. Torii maintenant
  производит только Merkle engagements и отклоняет voies с конфигурацией KZG.
- `KzgCommitment` utilise 48 touches à partir de `iroha_crypto::kzg`.
  En ce qui concerne les voies Merkle, seules les preuves Merkle sont utilisées.
- `proof_digest` intègre l'intégration DA-5 PDP/PoTR, ce qui permet d'installer le connecteur
  description de l'échantillonnage, utilisation pour la prise en charge des blobs.

### 1.2 Modification de l'en-tête du bloc

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

Ce bundle comprend le bloc de hachage, ainsi que les métadonnées `SignedBlockWire`. Cogda
накладных расходов.

Note d'implémentation : `BlockPayload` et `BlockBuilder` sont à votre disposition
setters/getters `da_commitments` (avec `BlockBuilder::set_da_commitments` et
`SignedBlock::set_da_commitments`), car ces hôtes peuvent être utilisés
предварительно собранный bundle до запечатывания блока. Все helper-constructors
L'installation du pôle `None` vers le Torii ne permet pas de créer des bundles réels.

### 1.3 Codage des fils- `SignedBlockWire::canonical_wire()` ajoute l'en-tête Norito pour
  `DaCommitmentBundle` sразу после списка транзакций. Octet de version `0x01`.
- `SignedBlockWire::decode_wire()` отклоняет bundles с неизвестной `version`,
  Dans les rapports politiques avec Norito et `norito.md`.
- La dérivation de hachage est également disponible dans `block::Hasher` ; clients légers, которые
  Décoder le format de fil souhaité, automatiquement à partir du nouveau pôle, du poste
  L'en-tête Norito s'applique à vous.

## 2. Blocs de poterie

1. Torii DA ingest finalise `DaIngestReceipt` et le publie sur l'ordinateur.
   очередь (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` enregistre tous les reçus avec le paiement `lane_id` pour le bloc
   La dédoublement est effectuée sur `(lane_id, client_blob_id, manifest_hash)`.
3. Avant de trier les engagements du constructeur vers `(lane_id, epoch,
   séquence)` pour déterminer le hachage, codec bundle Norito et
   обновляет `da_commitments_hash`.
4. Le bundle complet est disponible dans WSV et émis dans le bloc
   `SignedBlockWire`.

Si le bloc est vérifié, les reçus sont envoyés au destinataire
poppytki; le constructeur записывает последний включенный `sequence` по каждой lane,
чтобы предотвратить replay атаки.

## 3. RPC et surface de requête

Torii présélectionne trois points de terminaison :| Itinéraire | Méthode | Charge utile | Remarques |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (plage-filtre pour voie/époque/séquence, pagination) | Cela correspond à `DaCommitmentPage` avec le nombre total, les engagements et le bloc de hachage. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (voie + hachage manifeste ou court `(epoch, sequence)`). | Afficher `DaCommitmentProof` (enregistrement + chemin Merkle + bloc de hachage). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Aide apatride, пересчитывающий hash bloka и проверяющий inclusion; mis à jour pour les SDK avant de télécharger `iroha_crypto`. |

Toutes les charges utiles sont disponibles dans `iroha_data_model::da::commitment`. Routeurs Torii
Monter les gestionnaires pour gérer les points de terminaison d'ingestion DA, afin de les utiliser
Jeton politique/mTLS.

## 4. Preuves d'inclusion et clients légers

- Le bloc principal du bloc binaire Merkle est disponible en série.
  `DaCommitmentRecord`. La corée est `da_commitments_hash`.
- `DaCommitmentProof` crée un enregistrement et un vecteur `(sibling_hash,
  position)` чтобы верификаторы смогли восстановить корень. Preuve de hachage
  bloc et en-tête, les clients légers peuvent prouver la finalité.
- Les assistants CLI (`iroha_cli app da prove-commitment`) répondent à la demande de preuve de cycle/
  vérifiez et indiquez Norito/hex pour les opérateurs.

## 5. Stockage et indexationWSV хранит engagements в отдельной famille de colonnes с ключом `manifest_hash`.
Les index suivants incluent `(lane_id, epoch)` et `(lane_id, sequence)`, qui sont
запросы не сканировали полные bundles. Каждый record хранит высоту блока, в
котором он был запечатан, что позволяет rattrapage узлам быстро восстанавLIвать
индекс из journal de bloc.

## 6. Télémétrie et observabilité

- `torii_da_commitments_total` s'ouvre, le bloc prend en charge le minimum
  один enregistrement.
- `torii_da_commitment_queue_depth` отслеживает les reçus, ожидающие le regroupement
  (по voie).
- Tableau de bord Grafana `dashboards/grafana/da_commitments.json` visualisé
  inclusion dans le bloc, vérification du débit et preuve du débit pour l'auditoire de la version DA-3
  porte.

## 7. Test de stratégie

1. **Tests unitaires** pour l'encodage/décodage `DaCommitmentBundle` et la mise à jour
   bloquer la dérivation du hachage.
2. **Golden luminaires** dans `fixtures/da/commitments/` avec paquet d'octets canoniques
   и Preuves Merkle.
3. **Tests d'intégration** avec deux validateurs, ingérer des échantillons de blobs et vérifier
   согласованности bundle и requête/preuve ответов.
4. **Tests client léger** в `integration_tests/tests/da/commitments.rs` (Rouille),
   вызывающие `/prove` et preuve de preuve sans vérification par Torii.
5. **CLI smoke** écrit `scripts/da/check_commitments.sh` pour votre utilisation
   outillage de l'opérateur.

## 8. Plan de déploiement| Phases | Descriptif | Critères de sortie |
|-------|-------------|--------------------|
| P0 - Fusion de modèles de données | Entrez `DaCommitmentRecord`, mettez à jour l'en-tête de bloc et les codecs Norito. | `cargo test -p iroha_data_model` зеленый с новыми luminaires. |
| P1 - Câblage noyau/WSV | Sauvegardez la file d'attente + la logique du générateur de blocs, sauvegardez les index et ouvrez les gestionnaires RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` sont compatibles avec les preuves groupées. |
| P2 - Outillage opérateur | Publiez les assistants CLI, le tableau de bord Grafana et les documents de mise à jour pour la vérification des preuves. | `iroha_cli app da prove-commitment` fonctionne sur Devnet ; tableau de bord показывает live данные. |
| P3 - Porte de gouvernance | Ouvrez le validateur de bloc, traitez les engagements DA sur les voies, en passant par `iroha_config::nexus`. | Entrée de statut + mise à jour de la feuille de route помечают DA-3 как завершенный. |

## Открытые вопросы

1. **Par défaut KZG vs Merkle** — Il est facile de proposer des blobs de petite taille
   Engagements KZG, que peut-on faire pour le bloc de reconstruction ? Précédent: оставить
   `kzg_commitment` optionnel et portail par `iroha_config::da.enable_kzg`.
2. **Écarts de séquence** — Разрешать ли разрывы последовательности? Plan technique
   отклоняет gaps, если gouvernance ne включит `allow_sequence_skips` для
   экстренного replay.
3. **Cache client léger** — Le SDK de Command a fourni un cache SQLite léger pour les preuves ;
   дальнейшая работа под DA-8.Les réponses à ces projets concernant la mise en œuvre des PR ont précédé le DA-3 avec le statut « projet »
(ce document) dans "en cours" après le démarrage des codes de travail.