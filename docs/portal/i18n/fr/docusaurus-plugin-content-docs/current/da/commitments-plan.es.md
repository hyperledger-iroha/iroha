---
lang: fr
direction: ltr
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Refleja `docs/source/da/commitments_plan.md`. Mantenga ambas versions fr
:::

# Plan de compromis sur la disponibilité des données de Sora Nexus (DA-3)

_Rédigé : 2026-03-25 -- Responsables : Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 étend le format de bloc de Nexus pour que chaque voie incruste des enregistrements
déterministes qui décrivent les blobs acceptés par DA-2. Cette note a été capturée
structures de données canoniques, los hooks del pipeline de bloques, las pruebas de
client léger et superficies Torii/RPC qui doivent s'attarder avant de les
les validateurs peuvent confier des compromis à l'admission ou des chèques de
gobernanza. Toutes les charges utiles sont codifiées en Norito ; sin SCALE ni annonce JSON
ponctuellement.

## Objets- Faire des compromis par blob (racine de morceau + hachage manifeste + engagement KZG
  optionnel) à l'intérieur de chaque bloc Nexus pour que les pairs puissent reconstruire le
  estado de disponibilité sin consultar almacenamiento fuera del ledger.
- Proveer pruebas de memberia deterministas para que clientses ligeros verifiquen
  qu'un hachage manifeste est finalisé et bloqué.
- Exposant consulte Torii (`/v1/da/commitments/*`) et vérifie que cela permet de
  relais, SDK et automatisation de la vérification de la disponibilité sans reproduction
  chaque bloc.
- Garder l'enveloppe `SignedBlockWire` canonique à l'écriture des nouvelles
  structures à travers l'en-tête des métadonnées Norito et la dérivation du hachage de
  bloquer.

## Panorama de l'alcance

1. **Adiciones al modelo de datos** en `iroha_data_model::da::commitment` mas
   changements d'en-tête de blocage en `iroha_data_model::block`.
2. **Hooks de l'exécuteur** pour que `iroha_core` ingère les reçus DA émis par
   Torii (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **Persistance/index** pour que le WSV réponde aux consultations de compromis
   rapide (`iroha_core/src/wsv/mod.rs`).
4. **Adiciones RPC en Torii** pour les points de terminaison de la liste/consultation/évaluation ci-dessous
   `/v1/da/commitments`.
5. **Tests d'intégration + luminaires** validant la disposition des fils et le flux de
   preuve en `integration_tests/tests/da/commitments.rs`.

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
```- `KzgCommitment` réutilise le point de 48 octets utilisé dans `iroha_crypto::kzg`.
  Quand c'est ausente, vous voyez les preuves de Merkle seules.
- `proof_scheme` est dérivé du catalogue de voies ; Las Lanes Merkle Rechazan
  charges utiles KZG mientras que las voies `kzg_bls12_381` nécessitent des engagements KZG
  pas de zéro. Torii produit actuellement des compromis sur les voies Merkle et Rechaza
  configurés avec KZG.
- `KzgCommitment` réutilise le point de 48 octets utilisé dans `iroha_crypto::kzg`.
  Lorsque Merkle est ausente sur les voies Merkle, il se voit seul dans les preuves de Merkle.
- `proof_digest` anticipe l'intégration DA-5 PDP/PoTR pour le même enregistrement
  Énumérez le calendrier d'échantillonnage utilisé pour maintenir les blobs vivants.

### 1.2 Extension de l'en-tête du bloc

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

Le hachage du bundle entre également dans le hachage du bloc comme dans les métadonnées de
`SignedBlockWire`. Lorsque le blocage des données sur le terrain est permanent `None`

Note de mise en œuvre : `BlockPayload` et le transparent `BlockBuilder` maintenant
setters/getters d'exponen `da_commitments` (version `BlockBuilder::set_da_commitments`
et `SignedBlock::set_da_commitments`), car les hôtes peuvent ajouter un bundle
préconstruit avant de vendre un bloc. Tous les constructeurs assistant Dejan El
campo en `None` hasta que Torii enhebre bundles reales.

### 1.3 Encodage du fil- `SignedBlockWire::canonical_wire()` ajouter l'en-tête Norito pour
  `DaCommitmentBundle` immédiatement après la liste des transactions
  existant. L'octet de version est `0x01`.
- `SignedBlockWire::decode_wire()` rechaza bundles cuyo `version` es desconocido,
  suivez la politique Norito décrite en `norito.md`.
- Les mises à jour de dérivation de hash viven solo en `block::Hasher` ; Los
  clients légers qui décodifient le format de fil existant dans le nouveau champ
  automatiquement lorsque l'en-tête Norito annonce votre présence.

## 2. Flux de production de blocs

1. L'ingesta DA de Torii finalise un `DaIngestReceipt` et le publie dans le cola
   interne (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` recopila tous les reçus avec `lane_id` coïncident avec el
   bloquer la construction, dédupliquer par `(lane_id, client_blob_id,
   manifest_hash)`.
3. Juste avant de vendre, le constructeur ordonne les compromis pour `(lane_id,
   époque, séquence)` pour maintenir le hachage déterministe, codifier le bundle avec
   le codec Norito, et actualisez `da_commitments_hash`.
4. Le bundle complet est stocké dans le WSV et émis conjointement au bloc à l'intérieur de
   `SignedBlockWire`.

Si la création du bloc tombe, les reçus seront permanents dans le cola pour que le
suivant intento los tome; le constructeur enregistre le dernier `sequence` inclus
por lane para eviter les attaques de replay.## 3. Superficie RPC et consultation

Torii expose trois points de terminaison :

| Itinéraire | Méthode | Charge utile | Notes |
|------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtre par rang de voie/époque/séquence, pagination) | Devuelve `DaCommitmentPage` avec total, compromis et hachage de blocage. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (voie + hachage manifeste ou tupla `(epoch, sequence)`). | Répondez avec `DaCommitmentProof` (record + ruta Merkle + hash de bloque). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless qui renvoie le calcul du hachage du bloc et valide l'inclusion ; utilisé par les SDK qui ne peuvent pas être connectés directement à `iroha_crypto`. |

Toutes les charges utiles vivent sous `iroha_data_model::da::commitment`. Les routeurs de
Torii montre les gestionnaires avec les points finaux d'acquisition des données existantes pour
réutiliser la politique de token/mTLS.

## 4. Essais d'inclusion et clients légers- Le producteur de blocs construit un arbre binaire Merkle sur la liste
  sérialisé de `DaCommitmentRecord`. La racine alimenta `da_commitments_hash`.
- `DaCommitmentProof` empaqueta el record objetivo mas un vector de
  `(sibling_hash, position)` pour que les vérificateurs reconstruisent la racine. Las
  les tests incluent également le hachage de blocage et l'en-tête confirmé pour cela
  clients ligeros vérifient la finalité.
- Helpers de CLI (`iroha_cli app da prove-commitment`) envuelven le cycle de
  demande/vérification d'essais et d'explications de vente Norito/hex para
  opérateurs.

## 5. Stockage et indexation

Le WSV a fait des compromis dans une famille de colonnes dédiées avec une clé
`manifest_hash`. Les index secondaires cubren `(lane_id, epoch)` et
`(lane_id, sequence)` pour que les consultations évitent de rechercher des bundles complets.
Chaque fois que vous enregistrez la hauteur du bloc qui le vend, vous permettant de nouer des nœuds
rattrapage reconstruire l'indice rapidement à partir du journal de bloc.

## 6. Télémétrie et observabilité

- `torii_da_commitments_total` incrémente lorsqu'un bloc de vente est activé à moindre coût
  enregistrer.
- `torii_da_commitment_queue_depth` reçus rastrea espérant être empaquetados
  (par voie).
- Le tableau de bord Grafana `dashboards/grafana/da_commitments.json` visualise le
  inclusion en blocs, profondeur de cola et débit de tests pour que
  Les portes de libération du DA-3 peuvent auditer le comportement.

## 7. Stratégie d'essai1. **Tests unitaires** pour l'encodage/décodage de `DaCommitmentBundle` et
   Actualisations de la dérivation du hachage du bloc.
2. **Fixtures golden** sous `fixtures/da/commitments/` qui capture les octets
   canonicos del bundle y pruebas Merkle.
3. **Tests d'intégration** menés par les validateurs, insérant des blobs de
   assurez-vous et vérifiez que tous les nœuds sont concuerdan dans le contenu du bundle y
   las repuestas de consulta/prueba.
4. **Tests de client léger** en `integration_tests/tests/da/commitments.rs`
   (Rouille) que vous devez appeler `/prove` et vérifier la vérification sans avoir à faire avec Torii.
5. **Smoke de CLI** avec `scripts/da/check_commitments.sh` pour l'outillage de maintenance
   de operadores reproductible.

## 8. Plan de déploiement| Phase | Description | Critère de sortie |
|------|-------------|---------------|
| P0 - Fusionner le modèle de données | Intégrer `DaCommitmentRecord`, mises à jour de l'en-tête de bloc et des codecs Norito. | `cargo test -p iroha_data_model` en vert avec de nouveaux luminaires. |
| P1 - Câble Core/WSV | Enrichir la logique de cola + le constructeur de blocs, les index persistants et les gestionnaires d'exposants RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` ne contiennent pas d'affirmations de preuve groupée. |
| P2 - Outillage des opérateurs | Lanzar helpers de CLI, tableau de bord Grafana et actualisations de documents de vérification de preuve. | `iroha_cli app da prove-commitment` fonctionne contre Devnet ; Le tableau de bord affiche des données en vivo. |
| P3 - Porte d'administration | Habiliter le validateur de blocs qui nécessitent des compromis DA sur les voies marquées en `iroha_config::nexus`. | Entrée du statut + mise à jour de la feuille de route marcan DA-3 comme COMPLETADO. |

## Questions ouvertes

1. **Par défaut KZG vs Merkle** - Nous devons omettre les compromis KZG et les petits blobs
   pour réduire le tampon du blocage ? Propuesta: maintenance `kzg_commitment`
   en option et via `iroha_config::da.enable_kzg`.
2. **Écarts de séquence** - Permitimos voies fuera de orden ? El plan rechaza réel
   lacunes salvo que gobernanza active `allow_sequence_skips` pour replay de
   urgence.
3. **Light-client cache** - L'équipe du SDK a un cache SQLite disponible pour
   preuves; puis pendent sous le DA-8.Répondez à ces questions concernant les PR de mise en œuvre du DA-3 de BORRADOR (cette
documento) a EN PROGRESO cuando el trabajo de codigo comience.