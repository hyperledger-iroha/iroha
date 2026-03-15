---
lang: fr
direction: ltr
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Source canonique
Reflète `docs/source/da/ingest_plan.md`. Gardez les deux versions en synchronisation tant
:::

# Plan d'ingest Disponibilité des données Sora Nexus

_Redige : 20/02/2026 - Responsables : Core Protocol WG / Storage Team / DA WG_

Le workstream DA-2 étend Torii avec une API d'ingest de blobs qui emet des
métadonnées Norito et amorce la réplication SoraFS. Ce fichier de capture de document
schéma proposé, la surface API et le flux de validation afin que
l'implémentation avancée sans blocage sur les simulations restantes (suivi
DA-1). Tous les formats de charge utile DOIVENT utiliser les codecs Norito; aucun
fallback serde/JSON n'est permis.

## Objectifs

- Accepter des blobs volumineux (segments Taikai, side-cars de lane, artefacts de
  gouvernance) de manière déterministe via Torii.
- Produire des manifestes Norito canoniques décrivant le blob, les paramètres de
  codec, le profil d'effacement et la politique de rétention.
- Persister les métadonnées de chunks dans le stockage chaud de SoraFS et mettre en
  fichier les jobs de réplication.
- Publier les intentions de pin + tags de politique dans le registre SoraFS et les
  observateurs de gouvernance.
- Exposer des reçus d'admission pour que les clients récupèrent une preuve
  déterministe de publication.

## API Surface (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```La charge utile est un code `DaIngestRequest` et Norito. Les réponses utilisent
`application/norito+v1` et renvoient `DaIngestReceipt`.

| Réponse | Signification |
| --- | --- |
| 202 Accepté | Blob et fichier pour le chunking/réplication ; reçu renvoyé. |
| 400 requêtes incorrectes | Violation de schéma/taille (voir contrôles de validation). |
| 401 Non autorisé | API de jeton manquant/invalide. |
| 409 Conflit | Doublon `client_blob_id` avec métadonnées non identiques. |
| 413 Charge utile trop importante | Dépassez la limite configurée de longueur du blob. |
| 429 Trop de demandes | Limite de taux atteinte. |
| 500 Erreur interne | Echec inattendue (log + alerte). |

## Schéma Norito proposer

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

> Note d'implémentation : les représentations Rust canoniques pour ces payloads
> vivent maintenant sous `iroha_data_model::da::types`, avec des wrappers
> demande/réception dans `iroha_data_model::da::ingest` et la structure de
> manifeste dans `iroha_data_model::da::manifest`.

Le champ `compression` annonce comment les appelants ont préparé le payload. Torii
acceptez `identity`, `gzip`, `deflate`, et `zstd`, en décomprimant les octets avant
le hachage, le chunking et la vérification des manifestes optionnels.

### Checklist de validation1. Vérifier que l'en-tête Norito de la requête correspond à un `DaIngestRequest`.
2. Echouer si `total_size` diffère de la longueur canonique du payload
   (décompresser) ou dépasser le maximum de configuration.
3. Forcer l'alignement de `chunk_size` (puissance de deux, = 2.
5. `retention_policy.required_replica_count` doit respecter la ligne de base de
   gouvernance.
6. Vérification de signature contre le hash canonique (en excluant le champ
   signature).
7. Rejeter un doublon `client_blob_id` sauf si le hash du payload et les métadonnées
   sont identiques.
8. Quand `norito_manifest` est fourni, vérifier que le schéma + le hachage correspondant
   au manifeste recalculer après chunking ; sinon le noeud génère le manifeste et le
   stocke.
9. Appliquer la politique de réplication configurée : Torii reecrit le
   `RetentionPolicy` soumis avec `torii.da_ingest.replication_policy` (voir
   `replication-policy.md`) et rejette les manifestes préconstruits dont la
   les métadonnées de rétention ne correspondent pas au profil imposé.

### Flux de chunking et de réplication1. Découper le payload en `chunk_size`, calculer BLAKE3 par chunk + racine Merkle.
2. Construire Norito `DaManifestV1` (nouvelle struct) capturant les engagements
   de chunk (role/group_id), le layout d'erasure (comptes de parite de lignes et
   colonnes plus `ipa_commitment`), la politique de rétention et la métadonnée.
3. Mettre en fichier les octets du manifeste canonique sous
   `config.da_ingest.manifest_store_dir` (Torii ecrit des fichiers
   `manifest.encoded` index par voie/époque/séquence/ticket/empreinte digitale) afin
   que l'orchestration SoraFS les ingère et relie le stockage ticket aux données
   persiste.
4. Publier les intentions de pin via `sorafs_car::PinIntent` avec tag de gouvernance
   et politique.
5. Emettre l'événement Norito `DaIngestPublished` pour notifier les observateurs
   (clients légers, gouvernance, analytique).
6. Renvoyer `DaIngestReceipt` au caller (signe par la clé de service DA de Torii)
   et emettre le header `Sora-PDP-Commitment` pour que les SDK capturent le
   l’engagement code l’immédiat. Le reçu inclut maintenant `rent_quote`
   (un Norito `DaRentQuote`) et `stripe_layout`, permettant aux soumetteurs
   d'afficher la rente de base, la réserve, les attentes de bonus PDP/PoTR et le
   layout d'erasure 2D aux cotes du storage ticket avant d'engager des fonds.

## Mises à jour stockage/registry- Etendre `sorafs_manifest` avec `DaManifestV1`, permettant un parsing
  déterministe.
- Ajouter un nouveau flux de registre `da.pin_intent` avec une version de charge utile
  référençant le hash du manifeste + l'identifiant du ticket.
- Mettre à jour les pipelines d'observabilité pour suivre la latence d'ingest,
  le débit de chunking, le backlog de réplication et les compteurs
  d'echecs.

## Stratégie de tests

- Tests unitaires pour validation de schéma, vérifications de signature, détection de
  des doublons.
- Tests dorés vérifiant l'encodage Norito de `DaIngestRequest`, manifeste et
  reçu.
- Harnais d'intégration démarrant un SoraFS + registre simules, validant les
  flux de chunk + pin.
- Tests de propriétés couvrant les profils d'effacement et combinaisons de
  rétention aléatoire.
- Fuzzing des payloads Norito pour se protéger contre la métadonnée mal formée.

## Outils CLI et SDK (DA-8)- `iroha app da submit` (nouvel Entrypoint CLI) enveloppe maintenant le builder
  d'ingest partage afin que les opérateurs puissent ingérer des blobs
  lots hors du flux Taikai. La commande vit dans
  `crates/iroha_cli/src/commands/da.rs:1` et consomme une charge utile, un profil
  d'erasure/retention et des fichiers optionnels de metadata/manifest avant de
  signez le `DaIngestRequest` canonique avec la clé de config CLI. Les courses
  reussis persistant `da_request.{norito,json}` et `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (remplacement via `--artifact-dir`) afin que
  les artefacts de release enregistrent les octets Norito exacts utilise pendant
  l'ingérer.
- La commande utiliser par défaut `client_blob_id = blake3(payload)` mais accepter
  des overrides via `--client-blob-id`, honore les maps JSON de métadonnées
  (`--metadata-json`) et les manifestes pré-généres (`--manifest`), et supporte
  `--no-submit` pour la préparation hors ligne plus `--endpoint` pour des hôtes
  Torii personnalise. Le reçu JSON est imprimé sur stdout en plus d'être
  écrit sur disque, fermant l'exigence d'outillage "submit_blob" de DA-8 et
  débloquant le travail de parité SDK.
- `iroha app da get` ajoute un alias DA pour l'orchestrateur multi-source qui alimente
  déjà `iroha app sorafs fetch`. Les opérateurs peuvent le pointer vers des artefacts
  manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou** fournirun ticket de stockage Torii via `--storage-ticket`. Quand le chemin ticket est
  utilisez, la CLI récupère le manifeste depuis `/v2/da/manifests/<ticket>`,
  persiste le bundle sous `artifacts/da/fetch_<timestamp>/` (override avec
  `--manifest-cache-dir`), dériver le hash du blob pour `--manifest-id`, puis
  exécutez l'orchestrateur avec la liste `--gateway-provider` fournie. Tous les
  les boutons avancés du fetcher SoraFS restent intacts (enveloppes du manifeste, étiquettes
  client, caches de garde, remplacements de transport anonyme, export scoreboard et
  paths `--output`), et l'endpoint manifest peut être surtaxé via
  `--manifest-endpoint` pour des hôtes Torii personnalise, donc les contrôles
  de bout en bout d'availability vivent entièrement sous le namespace `da` sans
  dupliquer la logique d'orchestrateur.
- `iroha app da get-blob` récupérer les manifestes canoniques directement depuis Torii
  via `GET /v2/da/manifests/{storage_ticket}`. La commande écrite
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` et
  `chunk_plan_{ticket}.json` sous `artifacts/da/fetch_<timestamp>/` (ou un
  `--output-dir` fourni par l'utilisateur) tout en affichant la commande exacte
  `iroha app da get` (incluant `--manifest-id`) requis pour le fetch orchestrateur.
  Cela garde les opérateurs hors des répertoires spool de manifestes et garantit
  que le fetcher utilise toujours les artefacts signes émis par Torii. Le client
  Torii JavaScript reproduit ce flux via`ToriiClient.getDaManifest(storageTicketHex)`, renvoyant les octets Norito
  décode, le manifest JSON et le chunk plan afin que les appelants SDK s'hydratent
  des sessions d'orchestrateur sans passer par la CLI. Le SDK Swift expose
  maintenant les memes surfaces (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), branchant les bundles sur le wrapper natif
  d'orchestrateur SoraFS pour que les clients iOS puissent télécharger des
  manifestes, exécuter des feches multi-source et capturer des preuves sans
  invoquer la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcule des rentas déterministes et la ventilation des
  incitations pour une taille de stockage et une fenêtre de rétention fournies.
  L'helper consomme la `DaRentPolicyV1` active (JSON ou bytes Norito) ou le
  intégrer par défaut, valider la politique et imprimer un CV JSON (`gib`,
  `months`, métadonnées de politique, champs `DaRentQuote`) afin que les auditeurs
  citent les charges XOR exactes dans les minutes de gouvernance sans scripts ad
  ponctuellement. La commande emet aussi un CV en une ligne `rent_quote ...` avant le
  payload JSON pour garder les logs console lisibles pendant les drills
  d'incident. Associez `--quote-out artifacts/da/rent_quotes/<stamp>.json` avec
  `--policy-label "governance ticket #..."` pour persister des artefacts soignescitant le vote ou le bundle de config exact; la CLI tronque le label personnaliser
  et refuser les chaines vides afin que les valeurs `policy_source` restent
  actionnables dans les tableaux de bord de trésorerie. Voir
  `crates/iroha_cli/src/commands/da.rs` pour le sous-commande et
  `docs/source/da/rent_policy.md` pour le schéma de politique.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` chaine tout ce qui précède : il prend un rangement
  ticket, télécharge le bundle canonique de manifeste, exécute l'orchestrateur
  multi-source (`iroha app sorafs fetch`) contre la liste `--gateway-provider`
  fournie, persiste le payload telecharge + scoreboard sous
  `artifacts/da/prove_availability_<timestamp>/`, et invoquez immédiatement le
  helper PoR existant (`iroha app da prove`) avec les octets récupérés. Les opérateurs
  peuvent régler les boutons de l'orchestrateur (`--max-peers`, `--scoreboard-out`,
  overrides d'endpoint manifest) et le sampler de proof (`--sample-count`,
  `--leaf-index`, `--sample-seed`) tandis qu'une seule commande produit les
  artefacts attendus par les audits DA-5/DA-9 : copie du payload, preuves de
  scoreboard et résumés de preuve JSON.