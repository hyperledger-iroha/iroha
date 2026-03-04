---
lang: fr
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T15:38:30.660808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Plan d'engagements en matière de disponibilité des données (DA-3)

_Rédigé : 2026-03-25 — Propriétaires : Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 étend le format de bloc Nexus afin que chaque voie intègre des enregistrements déterministes
décrivant les blobs acceptés par DA-2. Cette note capture les données canoniques
structures, crochets de pipeline en bloc, épreuves client léger et surfaces Torii/RPC
qui doivent atterrir avant que les validateurs puissent s'appuyer sur les engagements du DA lors de l'admission ou
contrôles de gouvernance. Toutes les charges utiles sont codées en Norito ; pas d'ÉCHELLE ou de JSON ad hoc.

## Objectifs

- Effectuer des engagements par blob (racine de morceau + hachage manifeste + KZG facultatif
  engagement) à l'intérieur de chaque bloc Nexus afin que les pairs puissent reconstruire la disponibilité
  état sans consulter le stockage hors grand livre.
- Fournir des preuves d'adhésion déterministes afin que les clients légers puissent vérifier qu'un
  le hachage manifeste a été finalisé dans un bloc donné.
- Exposer les requêtes Torii (`/v1/da/commitments/*`) et les preuves qui laissent les relais,
  Les SDK et l'automatisation de la gouvernance auditent la disponibilité sans rejouer chaque
  bloquer.
- Gardez l'enveloppe `SignedBlockWire` existante canonique en enfilant le nouveau
  structures via l’en-tête de métadonnées Norito et la dérivation de hachage de bloc.

## Aperçu de la portée

1. **Ajouts de modèles de données** dans le bloc `iroha_data_model::da::commitment` plus
   changements d’en-tête dans `iroha_data_model::block`.
2. **Executor hooks** afin que `iroha_core` ingère les reçus DA émis par Torii
   (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **Persistance/index** pour que le WSV puisse répondre rapidement aux requêtes d'engagement
   (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** pour les points de terminaison de liste/requête/prouver sous
   `/v1/da/commitments`.
5. **Tests d'intégration + montages** validant la disposition des fils et le flux de preuve dans
   `integration_tests/tests/da/commitments.rs`.

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

- `KzgCommitment` réutilise le point de 48 octets existant utilisé sous
  `iroha_crypto::kzg`. Les voies Merkle le laissent vide ; Voies `kzg_bls12_381` maintenant
  recevoir un engagement déterministe BLAKE3-XOF dérivé de la racine du chunk et
  ticket de stockage afin que les hachages de blocs restent stables sans prouveur externe.
- `proof_scheme` est dérivé du catalogue de voies ; Les voies Merkle rejettent les KZG errants
  charges utiles tandis que les voies `kzg_bls12_381` nécessitent des engagements KZG non nuls.
- `proof_digest` anticipe l'intégration DA-5 PDP/PoTR donc le même record
  énumère le programme d’échantillonnage utilisé pour maintenir les blobs en vie.

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
```

Le hachage du bundle alimente à la fois le hachage de bloc et les métadonnées `SignedBlockWire`.
aérien.

Note d'implémentation : `BlockPayload` et le transparent `BlockBuilder` exposent désormais
`da_commitments` setters/getters (voir `BlockBuilder::set_da_commitments` et
`SignedBlock::set_da_commitments`), afin que les hôtes puissent attacher un bundle prédéfini
avant de sceller un bloc. Tous les constructeurs d'assistance utilisent par défaut le champ `None`.
jusqu'à ce que Torii fasse passer de vrais paquets.

### 1.3 Codage des fils- `SignedBlockWire::canonical_wire()` ajoute l'en-tête Norito pour
  `DaCommitmentBundle` immédiatement après la liste des transactions existante. Le
  l'octet de version est `0x01`.
- `SignedBlockWire::decode_wire()` rejette les bundles dont `version` est inconnu,
  correspondant à la stratégie Norito décrite dans `norito.md`.
- Les mises à jour de dérivation de hachage existent uniquement dans `block::Hasher` ; décodage des clients légers
  le format de fil existant gagne automatiquement le nouveau champ car le Norito
  l'en-tête annonce sa présence.

## 2. Bloquer le flux de production

1. L'ingestion Torii DA conserve les reçus signés et les enregistrements d'engagement dans le
   Bobine DA (`da-receipt-*.norito` / `da-commitment-*.norito`). Le résistant
   le journal des reçus amorce les curseurs au redémarrage afin que les reçus relus soient toujours ordonnés
   déterministe.
2. L'assemblage du bloc charge les reçus de la bobine et laisse tomber les reçus périmés/déjà scellés.
   entrées à l'aide de l'instantané de curseur validé et applique la contiguïté par
   `(lane, epoch)`. Si un reçu accessible ne comporte pas d'engagement correspondant ou si le
   le hachage manifeste diverge, la proposition est abandonnée au lieu de l'omettre silencieusement.
3. Juste avant de sceller, le constructeur coupe le paquet d'engagement en deux.
   ensemble piloté par reçu, trié par `(lane_id, epoch, sequence)`, code le
   bundle avec le codec Norito et met à jour `da_commitments_hash`.
4. Le bundle complet est stocké dans le WSV et émis avec le bloc à l'intérieur
   `SignedBlockWire` ; les liasses validées avancent les curseurs de réception (hydratés
   de Kura au redémarrage) et élaguer les entrées de spool obsolètes pour limiter la croissance du disque.

L'assemblage de blocs et l'ingestion `BlockCreated` revalident chaque engagement par rapport à
le catalogue des voies : les voies Merkle rejettent les engagements KZG parasites, les voies KZG nécessitent un
engagement KZG non nul et `chunk_root` non nul, et les voies inconnues sont
laissé tomber. Le point de terminaison `/v1/da/commitments/verify` de Torii reflète la même garde,
et l'ingestion intègre désormais l'engagement déterministe KZG dans chaque
Enregistrement `kzg_bls12_381` afin que les ensembles conformes aux règles atteignent l'assemblage de blocs.

Les appareils manifestes décrits dans le plan d'ingestion DA-2 servent également de source de
vérité pour le bundler d’engagement. Le test Torii
`manifest_fixtures_cover_all_blob_classes` régénère les manifestes pour chaque
Variante `BlobClass` et refuse de compiler jusqu'à ce que de nouvelles classes obtiennent des appareils,
s'assurer que le hachage du manifeste codé à l'intérieur de chaque `DaCommitmentRecord` correspond au
paire dorée Norito/JSON.【crates/iroha_torii/src/da/tests.rs:2902】

Si la création du bloc échoue, les reçus restent dans la file d'attente, donc le bloc suivant
une tentative peut les récupérer ; le constructeur enregistre le dernier `sequence` inclus par
voie pour éviter les attaques par rejeu.

## 3. RPC et surface de requête

Torii expose trois points de terminaison :| Itinéraire | Méthode | Charge utile | Remarques |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtre de plage par voie/époque/séquence, pagination) | Renvoie `DaCommitmentPage` avec le nombre total, les engagements et le hachage de bloc. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (voie + hachage manifeste ou tuple `(epoch, sequence)`). | Répond avec `DaCommitmentProof` (enregistrement + chemin Merkle + hachage de bloc). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Assistant apatride qui rejoue le calcul du hachage de bloc et valide l'inclusion ; utilisé par les SDK qui ne peuvent pas se lier directement à `iroha_crypto`. |

Toutes les charges utiles résident sous `iroha_data_model::da::commitment`. Support de routeurs Torii
les gestionnaires à côté des points de terminaison d'ingestion DA existants pour réutiliser le jeton/mTLS
politiques.

## 4. Preuves d'inclusion et clients légers

- Le producteur de blocs construit un arbre Merkle binaire sur le bloc sérialisé
  Liste `DaCommitmentRecord`. La racine alimente `da_commitments_hash`.
- `DaCommitmentProof` regroupe l'enregistrement cible plus un vecteur de `(sibling_hash,
  position)` afin que les vérificateurs puissent reconstruire la racine. Les preuves incluent également
  le hachage de bloc et l'en-tête signé afin que les clients légers puissent vérifier la finalité.
- Les assistants CLI (`iroha_cli app da prove-commitment`) enveloppent la demande de preuve/vérifient
  sorties cycle et surface Norito/hex pour les opérateurs.

## 5. Stockage et indexation

WSV stocke les engagements dans une famille de colonnes dédiée, saisie par `manifest_hash`.
Les index secondaires couvrent `(lane_id, epoch)` et `(lane_id, sequence)` donc les requêtes
évitez de numériser des lots complets. Chaque enregistrement suit la hauteur du bloc qui l'a scellé,
permettant aux nœuds de rattrapage de reconstruire rapidement l'index à partir du journal de bloc.

## 6. Télémétrie et observabilité

- `torii_da_commitments_total` s'incrémente chaque fois qu'un bloc scelle au moins un
  enregistrer.
- `torii_da_commitment_queue_depth` suit les reçus en attente d'être regroupés (par
  voie).
- Le tableau de bord Grafana `dashboards/grafana/da_commitments.json` visualise le bloc
  inclusion, profondeur de file d'attente et débit de preuve afin que les portes de publication DA-3 puissent effectuer un audit
  comportement.

## 7. Stratégie de test

1. **Tests unitaires** pour l'encodage/décodage `DaCommitmentBundle` et le hachage de bloc
   mises à jour de dérivation.
2. **Montages dorés** sous `fixtures/da/commitments/` capturant canonique
   regrouper les octets et les preuves Merkle. Chaque bundle fait référence aux octets du manifeste
   de `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`, donc
   régénérant `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`
   maintient l'histoire Norito cohérente avant que `ci/check_da_commitments.sh` actualise l'engagement
   preuves.【fixtures/da/ingest/README.md:1】
3. **Tests d'intégration** démarrant deux validateurs, ingérant des exemples de blobs et
   affirmant que les deux nœuds sont d'accord sur le contenu du bundle et la requête/preuve
   réponses.
4. **Tests client léger** dans `integration_tests/tests/da/commitments.rs`
   (Rust) qui appelle `/prove` et vérifie la preuve sans parler à Torii.
5. Script **CLI smoke** `scripts/da/check_commitments.sh` pour garder l'opérateur
   outillage reproductible.

## 8. Plan de déploiement| Phases | Descriptif | Critères de sortie |
|-------|-------------|--------------------|
| P0 — Fusion de modèles de données | Atterrissez `DaCommitmentRecord`, bloquez les mises à jour d’en-tête et les codecs Norito. | `cargo test -p iroha_data_model` vert avec de nouveaux luminaires. |
| P1 — Câblage noyau/WSV | File d'attente de threads + logique de création de blocs, persistance des index et exposition des gestionnaires RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` réussissent avec les assertions de preuve de bundle. |
| P2 — Outillage de l'opérateur | Expédiez les assistants CLI, le tableau de bord Grafana et les mises à jour des documents de vérification des preuves. | `iroha_cli app da prove-commitment` fonctionne contre Devnet ; le tableau de bord affiche les données en direct. |
| P3 — Porte de gouvernance | Activez le validateur de bloc exigeant des engagements DA sur les voies signalées dans `iroha_config::nexus`. | Entrée du statut + mise à jour de la feuille de route marque DA-3 comme 🈴. |

## Questions ouvertes

1. **Par défaut KZG vs Merkle** — Les petits blobs devraient-ils toujours ignorer les engagements KZG pour
   réduire la taille des blocs ? Proposition : garder `kzg_commitment` en option et portail via
   `iroha_config::da.enable_kzg`.
2. **Écarts de séquence** — Autorisons-nous les voies dans le désordre ? Le plan actuel rejette les lacunes
   sauf si la gouvernance bascule `allow_sequence_skips` pour une relecture d'urgence.
3. **Cache client léger** — L'équipe SDK a demandé un cache SQLite léger pour
   preuves; en attente de suivi sous DA-8.

Répondre à ces questions dans la mise en œuvre des PR déplace DA-3 de 🈸 (ce document) à 🈺
une fois que le travail de code commence.