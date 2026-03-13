---
lang: fr
direction: ltr
source: docs/portal/docs/da/ingest-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Plan d'ingestion de disponibilité des données Sora Nexus

_مسودہ : 2026-02-20 -- Nom : Core Protocol WG / Storage Team / DA WG_

DA-2 utilise Torii pour l'API d'ingestion de blob pour Norito.
جاری کرتی ہے اور SoraFS ریپلیکیشن کو seed کرتی ہے۔ یہ دستاویز مجوزہ schéma،
Surface de l'API, flux de validation et mise en œuvre pour la mise en œuvre
simulations (suivi DA-1) Voici les formats de charge utile
Codecs Norito pour les utilisateurs serde/JSON fallback pour les utilisateurs

## اہداف

- Des blobs (segments Taikai, side-cars de voie, artefacts de gouvernance) comme Torii
  ذریعے déterministe قبول کرنا۔
- blob, paramètres du codec, profil d'effacement, et politique de rétention et politique de rétention
  والے canonique Norito manifeste تیار کرنا۔
- métadonnées de fragments pour le stockage à chaud SoraFS et les tâches de réplication
  mettre en file d'attente کرنا۔
- Intentions de broches + balises de stratégie et registre SoraFS et observateurs de gouvernance.
  publier کرنا۔
- les reçus d'admission pour les clients et les preuves déterministes de
  publication مل سکے۔

## Surface API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Charge utile Norito codée `DaIngestRequest` Réponses `application/norito+v1`
Demande d'achat pour `DaIngestReceipt` et demande d'achat| Réponse | مطلب |
| --- | --- |
| 202 Accepté | Blob et segmentation/réplication et file d'attente reçu واپس۔ |
| 400 requêtes incorrectes | Violation du schéma/taille (vérifications de validation par exemple)۔ |
| 401 Non autorisé | Jeton API موجود نہیں/غلط۔ |
| 409 Conflit | `client_blob_id` ڈپلیکیٹ ہے اور métadonnées مختلف ہے۔ |
| 413 Charge utile trop importante | Limite de longueur de blob configurée |
| 429 Trop de demandes | Limite de débit atteinte۔ |
| 500 Erreur interne | غیر متوقع échec (journal + alerte)۔ |

## Schéma Norito proposé

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Note d'implémentation : les charges utiles et les représentations canoniques de Rust.
> `iroha_data_model::da::types` pour les emballages de demande/réception
> `iroha_data_model::da::ingest` میں، اور structure manifeste
> `iroha_data_model::da::manifest` ہے۔

Champ `compression` pour les appelants et la charge utile pour les appels Torii
`identity`, `gzip`, `deflate`, et `zstd` pour le hachage, le chunking et le hachage
les manifestes facultatifs vérifient les octets et décompressent les octets

### Liste de contrôle de validation1. Vérifiez que la demande de l'en-tête Norito `DaIngestRequest` correspond à la correspondance.
2. Longueur de la charge utile canonique (décompressée) `total_size`
   configuré max زیادہ ہو تو fail کریں۔
3. L'alignement `chunk_size` applique کریں (puissance de deux, = 2 یقینی بنائیں۔
5. `retention_policy.required_replica_count` pour le respect des bases de gouvernance
6. Hachage canonique et vérification de la signature (champ de signature)
7. Les doublons `client_blob_id` rejettent le hachage de la charge utile + les métadonnées identiques.
8. `norito_manifest` contient un schéma + hachage et un chunking.
   manifeste recalculé et correspondance کریں؛ Le manifeste du nœud génère un magasin et un magasin
9. La politique de réplication configurée est appliquée comme suit : Torii `RetentionPolicy`.
   `torii.da_ingest.replication_policy` سے réécriture کرتا ہے ( `replication-policy.md`
   دیکھیں ) Les manifestes prédéfinis et le rejet des métadonnées de rétention
   profil forcé سے match نہ کرے۔

### Flux de segmentation et de réplication1. Payload `chunk_size` est un morceau de morceau de BLAKE3 et de racine Merkle.
2. Norito `DaManifestV1` (structure originale) pour les engagements de fragments (role/group_id)
   disposition d'effacement (nombre de parité ligne/colonne + `ipa_commitment`), politique de rétention,
   اور métadonnées کو capture کرے۔
3. Octets du manifeste canonique comme `config.da_ingest.manifest_store_dir` pour la file d'attente
   (Torii `manifest.encoded` fichiers voie/époque/séquence/ticket/empreinte digitale کے لحاظ سے لکھتا ہے)
   L'orchestration SoraFS ingère un ticket de stockage et des données persistantes et un lien
4. `sorafs_car::PinIntent` pour la balise de gouvernance + la politique pour les intentions de broche de publication
5. L'événement Norito `DaIngestPublished` émet des observateurs de premier plan (clients légers, gouvernance, analyses)
   کو اطلاع ملے۔
6. Appelant `DaIngestReceipt` et clé de service (clé de service Torii DA signée) et
   L'en-tête `Sora-PDP-Commitment` émet des SDK pour la capture d'engagement.
   Reçu pour `rent_quote` (Norito `DaRentQuote`) pour `stripe_layout` en stock
   Par rapport au loyer de base des soumissionnaires, à la part de réserve, aux attentes en matière de bonus PDP/PoTR et à 2D
   disposition d'effacement et ticket de stockage pour les fonds engagés et pour le stockage des fonds

## Mises à jour du stockage/du registre- `sorafs_manifest` et `DaManifestV1` pour étendre l'analyse déterministe et l'analyse déterministe
- Le flux de registre `da.pin_intent` contient une charge utile versionnée
  hachage manifeste + identifiant du ticket et référence کرتا ہے۔
- Pipelines d'observabilité, latence d'acquisition, débit de segmentation, arriéré de réplication
  L'échec compte

## Stratégie de test

- Validation du schéma, vérifications de signature, détection des doublons, tests unitaires
- Encodage Norito pour tests d'or (`DaIngestRequest`, manifeste, reçu)۔
- Faisceau d'intégration et simulation SoraFS + registre et bloc + vérification des flux de broches.
- Les tests de propriété et les profils d'effacement aléatoires et les combinaisons de rétention couvrent tous les détails
- Charge utile Norito fuzzing et métadonnées mal formées

## Outils CLI et SDK (DA-8)- `iroha app da submit` (point d'entrée CLI) pour le générateur/éditeur d'ingestion partagé et l'enveloppement des fichiers.
  opérateurs Taikai bundle flow pour les blobs arbitraires ingérés یہ کمانڈ
  `crates/iroha_cli/src/commands/da.rs:1` charge utile, profil d'effacement/rétention et facultatif
  métadonnées/fichiers manifestes avec la clé de configuration CLI et avec le signe canonique `DaIngestRequest`.
  Le modèle exécute `da_request.{norito,json}` et `da_receipt.{norito,json}`.
  `artifacts/da/submission_<timestamp>/` est un système de remplacement (remplacement via `--artifact-dir`)
  libérer les artefacts ingérer میں استعمال ہونے والے exact Norito octets record کریں۔
- La valeur par défaut de `client_blob_id = blake3(payload)` est utilisée pour les remplacements par défaut de `--client-blob-id`.
  Cartes JSON de métadonnées (`--metadata-json`) et manifestes pré-générés (`--manifest`) et accepter les fichiers
  Par `--no-submit` (préparation hors ligne) Par `--endpoint` (hôtes Torii personnalisés) Reçu JSON
  stdout permet d'imprimer et de disque et de créer un périphérique d'impression avec DA-8 et l'exigence "submit_blob"
  اور Déblocage du travail de parité du SDK ہوتا ہے۔
- `iroha app da get` Alias axé sur DA pour un orchestrateur multi-source et pour utiliser un orchestrateur multi-source
  `iroha app sorafs fetch` en français Manifeste des opérateurs + artefacts de plan de fragments (`--manifest`, `--plan`, `--manifest-id`)
  **یا** Ticket de stockage Torii via `--storage-ticket` en ligne Chemin d'accès aux tickets CLI `/v2/da/manifests/<ticket>`manifeste télécharger le bundle کو `artifacts/da/fetch_<timestamp>/` میں محفوظ کرتی ہے (remplacement via
  `--manifest-cache-dir`) et `--manifest-id` pour le hachage blob dérivé et pour la liste `--gateway-provider`
  کے ساتھ orchestrator run کرتی ہے۔ SoraFS récupérateur de boutons avancés pour les enveloppes de manifeste,
  étiquettes client, caches de garde, remplacements de transport d'anonymat, exportation du tableau de bord (chemins `--output`)
  `--manifest-endpoint` pour le remplacement du point de terminaison du manifeste pour les contrôles de disponibilité de bout en bout
  Utilisez l'espace de noms `da` pour créer une copie de la logique de l'orchestrateur
- `iroha app da get-blob` Torii et `GET /v2/da/manifests/{storage_ticket}` pour les manifestes canoniques
  Voir `manifest_{ticket}.norito`, `manifest_{ticket}.json`, et `chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/` میں لکھتی (یا fourni par l'utilisateur `--output-dir`) et `iroha app da get`
  invocation (par `--manifest-id`) echo کرتی ہے جو suivi orchestrateur chercher کیلئے درکار ہے۔ اس سے opérateurs
  répertoires de bobine de manifeste pour récupérer les objets signés et récupérer Torii pour les artefacts signés
  JavaScript Torii client et flux `ToriiClient.getDaManifest(storageTicketHex)` sont décodés
  Norito octets, manifeste JSON et plan de fragments, ainsi que les appelants du SDK CLI et les sessions d'orchestrateur hydratées
  کر سکیں۔ Les surfaces du SDK Swift exposent les surfaces (`ToriiClient.getDaManifestBundle(...)` ici`fetchDaPayloadViaGateway(...)`), des bundles et un wrapper d'orchestrateur natif SoraFS avec des tuyaux et des clients iOS
  téléchargement des manifestes et récupérations multi-sources et capture des preuves par le biais d'une CLI
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` pour la taille du stockage et la fenêtre de rétention pour le loyer déterministe et la répartition des incitations
  calculer کرتا ہے۔ Assistant actif `DaRentPolicyV1` (JSON et Norito octets) et logiciel intégré par défaut.
  validation de la stratégie et résumé JSON (`gib`, `months`, métadonnées de la stratégie, champs `DaRentQuote`) impression de la stratégie
  Les procès-verbaux de gouvernance des auditeurs et les frais XOR exacts citent des scripts ad hoc. Charge utile JSON
  Les journaux de la console `rent_quote ...` sont résumés par les journaux de la console lisibles.
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` et `--policy-label "governance ticket #..."` sont disponibles
  Il s'agit d'objets joliment embellis et d'un vote politique dans le bundle de configuration citant ici Étiquette personnalisée CLI et garniture
  Les chaînes de rejet de chaînes sont rejetées et le tableau de bord `policy_source` est exploitable. دیکھیں
  `crates/iroha_cli/src/commands/da.rs` (sous-commande) et `docs/source/da/rent_policy.md` (schéma de stratégie)
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` Un ticket de stockage et un paquet de manifeste canoniquetélécharger l'orchestrateur multi-source (`iroha app sorafs fetch`) et la liste `--gateway-provider`
  Charge utile téléchargée + tableau de bord `artifacts/da/prove_availability_<timestamp>/` avec charge utile téléchargée + tableau de bord
  Il s'agit de l'assistant PoR (`iroha app da prove`) et des octets récupérés pour invoquer le système. Boutons d'orchestration des opérateurs
  (`--max-peers`, `--scoreboard-out`, remplacements de point de terminaison du manifeste) et échantillonneur de preuve (`--sample-count`, `--leaf-index`,
  `--sample-seed`) ajuster les audits DA-5/DA-9 et les artefacts de la commande :
  copie de la charge utile, preuves du tableau de bord, et résumés des preuves JSON