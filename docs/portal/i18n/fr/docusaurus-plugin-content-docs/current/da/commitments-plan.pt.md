---
lang: fr
direction: ltr
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fonte canonica
Espelha `docs/source/da/commitments_plan.md`. Mantenha comme duas versoes em
:::

# Plan de compromis sur la disponibilité des données par Sora Nexus (DA-3)

_Redigido : 2026-03-25 -- Responsable : Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 est le format de bloc du Nexus pour que chaque voie incorpore des enregistrements
déterministes qui décrivent les blobs liés au DA-2. Cette note a été capturée comme
estruturas de dados canonicas, os hooks do pipeline de blocos, comme prouvé
cliente leve e as superficies Torii/RPC que precisam chegar antes que
Les validateurs peuvent confier nos compromissos DA lors de l'admission ou des contrôles de
gouvernance. Toutes les charges utiles sont codifiées dans Norito ; sem SCALE ou JSON annonce
ponctuellement.

## Objets- Faire des compromis par blob (racine de morceau + hachage manifeste + engagement KZG
  optionnel) à l'intérieur de chaque bloc Nexus pour que les pairs puissent reconstruire l'état
  de disponibilité sem consulter les forums de stockage du grand livre.
- Fornecer provas de member deterministicas para que clientes leves
  Vérifiez que le hachage manifeste a été finalisé dans un bloc.
- Expor consultas Torii (`/v1/da/commitments/*`) et prouver que vous autorisez un relais,
  Les SDK et la vérification automatique de la disponibilité se reproduisent chaque bloc.
- Manter ou enveloppe `SignedBlockWire` canonico ao enfiar as novas estruturas
  l'en-tête des métadonnées Norito et la dérivation du hachage du bloc.

## Panorama de l'escopo

1. **Adicoes ao data model** em `iroha_data_model::da::commitment` plus modifiés
   de l'en-tête du bloc dans `iroha_data_model::block`.
2. **Hooks do executor** pour que `iroha_core` ingère les reçus DA émis par
   Torii (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/block.rs`).
3. **Persistencia/indexes** pour que le WSV réponde aux consultations de compromis
   rapidement (`iroha_core/src/wsv/mod.rs`).
4. **Adicoes RPC em Torii** pour les points de terminaison de la liste/consultation/prova sob
   `/v1/da/commitments`.
5. **Tests d'intégration + luminaires** validation de la disposition des fils et du flux de preuve
   em `integration_tests/tests/da/commitments.rs`.

## 1. Adicoes et le modèle de données

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
```- `KzgCommitment` réutilise le pont de 48 octets utilisé dans `iroha_crypto::kzg`.
  Quando ausente, cai para provas Merkle apenas.
- `proof_scheme` dérivé du catalogue de voies ; voies Merkle rejeitam charges utiles KZG
  enquanto voies `kzg_bls12_381` exigences engagements KZG nao zéro. Torii actuellement
  donc produz des compromis entre Merkle et des voies réservées configurées avec KZG.
- `KzgCommitment` réutilise le pont de 48 octets utilisé dans `iroha_crypto::kzg`.
  Quando ausente em lanes Merkle, cai para provas Merkle apenas.
- `proof_digest` anticipe l'intégration DA-5 PDP/PoTR pour mon enregistrement
  Énumérez le calendrier d'échantillonnage utilisé pour garder les blobs vivants.

### 1.2 Extension du bloc d'en-tête

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

Le hachage du bundle entre tant qu'il n'y a pas de hachage dans le bloc de métadonnées
`SignedBlockWire`. Lorsqu'un bloc nao carrega dados DA, o campo fica `None` para

Note de mise en œuvre : `BlockPayload` et `BlockBuilder` maintenant transparente
setters/getters d'exposition `da_commitments` (ver `BlockBuilder::set_da_commitments`
et `SignedBlock::set_da_commitments`), entao héberge peut-être une analyse d'un bundle
préconstruit avant de sceller un bloc. Tous les assistants deixam o campo em `None`
ate que Torii encadeie bundles reais.

### 1.3 Encodage du fil- `SignedBlockWire::canonical_wire()` ajouter l'en-tête Norito pour
  `DaCommitmentBundle` immédiatement après la liste des transacoes existantes. Ô
  octet de versao et `0x01`.
- `SignedBlockWire::decode_wire()` rejeita bundles cujo `version` seja
  desconhecido, alinhado a politica Norito décrit dans `norito.md`.
- Les actualisations de dérivation de hachage existent seulement dans `block::Hasher` ; clients
  leves que décodificam o wire format existe ganham o novo campo
  automatiquement lorsque l'en-tête Norito annonce sa présence.

## 2. Flux de production de blocs

1. L'acquisition du DA de Torii finalise le `DaIngestReceipt` et le public sur le fil
   interne (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` colle tous les reçus avec `lane_id` correspond au bloc
   dans la construction, déduplication par `(lane_id, client_blob_id, manifest_hash)`.
3. Peut-être avant de se coucher, le constructeur ordonne les compromis pour `(lane_id, epoch,
   séquence)` pour le sens du hachage déterministe, codifiant le bundle avec le codec
   Norito, et actualise `da_commitments_hash`.
4. Le bundle complet et armé du WSV et émis conjointement avec le bloc à l'intérieur de
   `SignedBlockWire`.

Se a criacao do bloco falhar, os reçus permanents na fila para que a proxima

tentative de capture; o constructeur registra o ultimo `sequence` inclus por lane
pour éviter les attaques de replay.

## 3. Superficie RPC et consultations

Torii expose trois points de terminaison :| Rotation | Méthode | Charge utile | Notes |
|------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtre de plage par voie/époque/séquence, page) | Retour `DaCommitmentPage` avec total, compromis et hachage de bloc. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (voie + hachage manifeste ou tupla `(epoch, sequence)`). | Répondez avec `DaCommitmentProof` (enregistrement + chemin Merkle + hachage de bloc). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless qui révise le calcul du hachage de bloc et valide l'inclusion ; utilisé par les SDK qui ne peuvent pas être liés directement à `iroha_crypto`. |

Toutes les charges utiles vivent sur `iroha_data_model::da::commitment`. Les routeurs de
Torii montez les gestionnaires d'exploitation à la place des points de terminaison d'acquisition des données existantes pour
réutiliser la politique de token/mTLS.

## 4. Preuves d'inclusion et de niveaux de clientèle

- Le producteur de blocs construit une binaire Merkle sur une liste
  sérialisé de `DaCommitmentRecord`. Une alimentation saine `da_commitments_hash`.
- `DaCommitmentProof` empacota o record alvo mais un vetor de
  `(sibling_hash, position)` pour que les vérificateurs reconstruisent une racine. Comme
  vérifiez également que vous incluez le hachage du bloc et l'en-tête supprimé pour les clients
  laisse validem une finalité.
- Les Helpers de CLI (`iroha_cli app da prove-commitment`) impliquent le cycle de
  demander/vérifier l'expoem dit Norito/hex pour les opérateurs.

## 5. Stockage et indexationO WSV armazena compromissos em uma famille de colonnes dédiées avec chave
`manifest_hash`. Index secondaires cobrem `(lane_id, epoch)` e
`(lane_id, sequence)` pour que vous puissiez consulter des bundles complets. Cada
enregistrer rastreia a altura do bloco que o selou, permettant que nous em rattrapons
reconstruire l'indice rapidement à partir du journal de bloc.

## 6. Télémétrie et observation

- `torii_da_commitments_total` incrémente quand un bloc est à moins d'un
  enregistrer.
- `torii_da_commitment_queue_depth` rastreia reçus aguardando bundle (por
  voie).
- O tableau de bord Grafana `dashboards/grafana/da_commitments.json` visualiser un
  inclus dans les blocs, la profondeur du fil et le débit des preuves pour les utilisateurs
  Les portes de libération du DA-3 peuvent auditer le comportement.

## 7. Stratégie testiculaire

1. **Tests unitaires** pour l'encodage/décodage de `DaCommitmentBundle` et
   actualisations de dérivation du hachage du bloc.
2. **Luminaires dorés** sanglot `fixtures/da/commitments/` capturando bytes canonicos
   faire bundle e provas Merkle.
3. **Tests d'intégration** avec deux validateurs, ingérant des blobs d'exemple et
   verificando que ambos os nos concordam no conteudo do bundle e nas respostas
   de requête/preuve.
4. **Tests de niveau client** em `integration_tests/tests/da/commitments.rs`
   (Rouille) que chamam `/prove` et vérifier à prouver sem falar com Torii.
5. **Smoke CLI** avec `scripts/da/check_commitments.sh` pour l'outillage
   opérateurs reproduzivel.

## 8. Plan de déploiement| Phase | Description | Critère de dite |
|------|-----------|---------|
| P0 - Fusionner le modèle de données | Intégrer `DaCommitmentRecord`, actualisations de l'en-tête de bloc et des codecs Norito. | `cargo test -p iroha_data_model` luminaires verde com novas. |
| P1 - Noyau de câblage/WSV | Encadrer une logique de fil + un générateur de blocs, des index persistants et des gestionnaires d'exportation RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passam com assertions de bundle proof. |
| P2 - Outillage des opérateurs | Entregar helpers CLI, tableau de bord Grafana et mises à jour des documents de vérification de preuve. | `iroha_cli app da prove-commitment` fonctionne contre Devnet ; Le tableau de bord montre les données à vivo. |
| P3 - Porte de gouvernance | Habiliter le validateur de blocs qui nécessitent des compromis sur les voies marquées dans `iroha_config::nexus`. | Entrée du statut + mise à jour de la feuille de route marcam DA-3 comme COMPLETADO. |

## Perguntas ouvertes

1. **Par défaut KZG vs Merkle** - Nous devons prendre des engagements semper pular KZG dans les blobs
   Des petits trucs pour réduire le tamanho du bloco ? Proposition : manter `kzg_commitment`
   en option et via `iroha_config::da.enable_kzg`.
2. **Écarts de séquence** - Des voies autorisées pour l'ordre ? O plano atual rejeita lacunes
   salvo se un gouvernement ativar `allow_sequence_skips` pour replay d'urgence.
3. **Cache client léger** - Il est temps que le SDK envoie un cache SQLite au niveau des preuves ;
   pendant le DA-8.Le répondeur est en poste dans les PR de mise en œuvre du mouvement DA-3 de RASCUNHO (este
documento) para EM ANDAMENTO quando o trabalho de codigo comecar.