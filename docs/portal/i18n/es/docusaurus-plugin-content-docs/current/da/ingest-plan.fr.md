---
lang: es
direction: ltr
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Reflete `docs/source/da/ingest_plan.md`. Gardez les deux versions en sync tant
:::

# Plan d'ingest Data Availability Sora Nexus

_Redige: 2026-02-20 - Responsables: Core Protocol WG / Storage Team / DA WG_

Le workstream DA-2 etend Torii avec une API d'ingest de blobs qui emet des
metadonnees Norito et amorce la replication SoraFS. Ce document capture le
schema propose, la surface API et le flux de validation afin que
l'implementation avance sans bloquer sur les simulations restantes (suivi
DA-1). Tous les formats de payload DOIVENT utiliser des codecs Norito; aucun
fallback serde/JSON n'est permis.

## Objectifs

- Accepter des blobs volumineux (segments Taikai, sidecars de lane, artefacts de
  gouvernance) de maniere deterministe via Torii.
- Produire des manifests Norito canoniques decrivant le blob, les parametres de
  codec, le profil d'erasure et la politique de retention.
- Persister la metadata de chunks dans le stockage hot de SoraFS et mettre en
  file les jobs de replication.
- Publier les intents de pin + tags de politique dans le registry SoraFS et les
  observateurs de gouvernance.
- Exposer des receipts d'admission pour que les clients retrouvent une preuve
  deterministe de publication.

## Surface API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Le payload est un `DaIngestRequest` encode en Norito. Les reponses utilisent
`application/norito+v1` et renvoient `DaIngestReceipt`.

| Reponse | Signification |
| --- | --- |
| 202 Accepted | Blob en file pour chunking/replication; receipt renvoye. |
| 400 Bad Request | Violation de schema/taille (voir checks de validation). |
| 401 Unauthorized | Token API manquant/invalide. |
| 409 Conflict | Doublon `client_blob_id` avec metadata non identique. |
| 413 Payload Too Large | Depasse la limite configuree de longueur du blob. |
| 429 Too Many Requests | Rate limit atteint. |
| 500 Internal Error | Echec inattendu (log + alerte). |

## Schema Norito propose

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

> Note d'implementation: les representations Rust canoniques pour ces payloads
> vivent maintenant sous `iroha_data_model::da::types`, avec des wrappers
> request/receipt dans `iroha_data_model::da::ingest` et la structure de
> manifest dans `iroha_data_model::da::manifest`.

Le champ `compression` annonce comment les callers ont prepare le payload. Torii
accepte `identity`, `gzip`, `deflate`, et `zstd`, en decomprimant les bytes avant
le hashing, le chunking et la verification des manifests optionnels.

### Checklist de validation

1. Verifier que le header Norito de la requete correspond a `DaIngestRequest`.
2. Echouer si `total_size` differe de la longueur canonique du payload
   (decompresse) ou depasse le maximum configure.
3. Forcer l'alignement de `chunk_size` (puissance de deux, <= 2 MiB).
4. Assurer `data_shards + parity_shards` <= maximum global et parity >= 2.
5. `retention_policy.required_replica_count` doit respecter la baseline de
   gouvernance.
6. Verification de signature contre le hash canonique (en excluant le champ
   signature).
7. Rejeter un `client_blob_id` duplique sauf si le hash du payload et la metadata
   sont identiques.
8. Quand `norito_manifest` est fourni, verifier que schema + hash correspondent
   au manifest recalcule apres chunking; sinon le noeud genere le manifest et le
   stocke.
9. Appliquer la politique de replication configuree: Torii reecrit le
   `RetentionPolicy` soumis avec `torii.da_ingest.replication_policy` (voir
   `replication-policy.md`) et rejette les manifests preconstruits dont la
   metadata de retention ne correspond pas au profil impose.

### Flux de chunking et replication

1. Decouper le payload en `chunk_size`, calculer BLAKE3 par chunk + racine Merkle.
2. Construire Norito `DaManifestV1` (nouvelle struct) capturant les engagements
   de chunk (role/group_id), le layout d'erasure (comptes de parite de lignes et
   colonnes plus `ipa_commitment`), la politique de retention et la metadata.
3. Mettre en file les bytes du manifest canonique sous
   `config.da_ingest.manifest_store_dir` (Torii ecrit des fichiers
   `manifest.encoded` indexes par lane/epoch/sequence/ticket/fingerprint) afin
   que l'orchestration SoraFS les ingere et relie le storage ticket aux donnees
   persistees.
4. Publier les intents de pin via `sorafs_car::PinIntent` avec tag de gouvernance
   et politique.
5. Emettre l'evenement Norito `DaIngestPublished` pour notifier les observateurs
   (clients legers, gouvernance, analytique).
6. Renvoyer `DaIngestReceipt` au caller (signe par la cle de service DA de Torii)
   et emettre le header `Sora-PDP-Commitment` pour que les SDKs capturent le
   commitment encode immediatement. Le receipt inclut maintenant `rent_quote`
   (un Norito `DaRentQuote`) et `stripe_layout`, permettant aux submitters
   d'afficher la rente de base, la reserve, les attentes de bonus PDP/PoTR et le
   layout d'erasure 2D aux cotes du storage ticket avant d'engager des fonds.

## Mises a jour stockage/registry

- Etendre `sorafs_manifest` avec `DaManifestV1`, permettant un parsing
  deterministe.
- Ajouter un nouveau stream de registry `da.pin_intent` avec un payload versione
  referencant le hash de manifest + ticket id.
- Mettre a jour les pipelines d'observabilite pour suivre la latence d'ingest,
  le throughput de chunking, le backlog de replication et les compteurs
  d'echecs.

## Strategie de tests

- Tests unitaires pour validation de schema, checks de signature, detection de
  doublons.
- Tests golden verifiant l'encoding Norito de `DaIngestRequest`, manifest et
  receipt.
- Harness d'integration demarrant un SoraFS + registry simules, validant les
  flux de chunk + pin.
- Tests de proprietes couvrant les profils d'erasure et combinaisons de
  retention aleatoires.
- Fuzzing des payloads Norito pour se proteger contre la metadata mal formee.

## Tooling CLI & SDK (DA-8)

- `iroha app da submit` (nouvel entrypoint CLI) enveloppe maintenant le builder
  d'ingest partage afin que les operateurs puissent ingerer des blobs
  arbitraires hors du flux Taikai bundle. La commande vit dans
  `crates/iroha_cli/src/commands/da.rs:1` et consomme un payload, un profil
  d'erasure/retention et des fichiers optionnels de metadata/manifest avant de
  signer le `DaIngestRequest` canonique avec la cle de config CLI. Les runs
  reussis persistent `da_request.{norito,json}` et `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (override via `--artifact-dir`) afin que
  les artefacts de release enregistrent les bytes Norito exacts utilises pendant
  l'ingest.
- La commande utilise par defaut `client_blob_id = blake3(payload)` mais accepte
  des overrides via `--client-blob-id`, honore les maps JSON de metadata
  (`--metadata-json`) et les manifests pre-generes (`--manifest`), et supporte
  `--no-submit` pour la preparation offline plus `--endpoint` pour des hosts
  Torii personnalises. Le receipt JSON est imprime sur stdout en plus d'etre
  ecrit sur disque, fermant l'exigence tooling "submit_blob" de DA-8 et
  debloquant le travail de parite SDK.
- `iroha app da get` ajoute un alias DA pour l'orchestrateur multi-source qui alimente
  deja `iroha app sorafs fetch`. Les operateurs peuvent le pointer vers des artefacts
  manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou** fournir
  un storage ticket Torii via `--storage-ticket`. Quand le chemin ticket est
  utilise, la CLI recupere le manifest depuis `/v1/da/manifests/<ticket>`,
  persiste le bundle sous `artifacts/da/fetch_<timestamp>/` (override avec
  `--manifest-cache-dir`), derive le hash du blob pour `--manifest-id`, puis
  execute l'orchestrateur avec la liste `--gateway-provider` fournie. Tous les
  knobs avances du fetcher SoraFS restent intacts (manifest envelopes, labels
  client, guard caches, overrides de transport anonyme, export scoreboard et
  paths `--output`), et l'endpoint manifest peut etre surcharge via
  `--manifest-endpoint` pour des hosts Torii personnalises, donc les checks
  end-to-end d'availability vivent entierement sous le namespace `da` sans
  dupliquer la logique d'orchestrateur.
- `iroha app da get-blob` recupere les manifests canoniques directement depuis Torii
  via `GET /v1/da/manifests/{storage_ticket}`. La commande ecrit
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` et
  `chunk_plan_{ticket}.json` sous `artifacts/da/fetch_<timestamp>/` (ou un
  `--output-dir` fourni par l'utilisateur) tout en affichant la commande exacte
  `iroha app da get` (incluant `--manifest-id`) requise pour le fetch orchestrateur.
  Cela garde les operateurs hors des repertoires spool de manifests et garantit
  que le fetcher utilise toujours les artefacts signes emis par Torii. Le client
  Torii JavaScript reproduit ce flux via
  `ToriiClient.getDaManifest(storageTicketHex)`, renvoyant les bytes Norito
  decodes, le manifest JSON et le chunk plan afin que les callers SDK hydratent
  des sessions d'orchestrateur sans passer par la CLI. Le SDK Swift expose
  maintenant les memes surfaces (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), branchant les bundles sur le wrapper natif
  d'orchestrateur SoraFS pour que les clients iOS puissent telecharger des
  manifests, executer des fetches multi-source et capturer des preuves sans
  invoquer la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcule des rentas deterministes et la ventilation des
  incentives pour une taille de storage et une fenetre de retention fournies.
  L'helper consomme la `DaRentPolicyV1` active (JSON ou bytes Norito) ou le
  default integre, valide la politique et imprime un resume JSON (`gib`,
  `months`, metadata de politique, champs `DaRentQuote`) afin que les auditeurs
  citent les charges XOR exactes dans les minutes de gouvernance sans scripts ad
  hoc. La commande emet aussi un resume en une ligne `rent_quote ...` avant le
  payload JSON pour garder les logs console lisibles pendant les drills
  d'incident. Associez `--quote-out artifacts/da/rent_quotes/<stamp>.json` avec
  `--policy-label "governance ticket #..."` pour persister des artefacts soignes
  citant le vote ou bundle de config exact; la CLI tronque le label personnalise
  et refuse les chaines vides afin que les valeurs `policy_source` restent
  actionnables dans les dashboards de tresorerie. Voir
  `crates/iroha_cli/src/commands/da.rs` pour le sous-commande et
  `docs/source/da/rent_policy.md` pour le schema de politique.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` chaine tout ce qui precede: il prend un storage
  ticket, telecharge le bundle canonique de manifest, execute l'orchestrateur
  multi-source (`iroha app sorafs fetch`) contre la liste `--gateway-provider`
  fournie, persiste le payload telecharge + scoreboard sous
  `artifacts/da/prove_availability_<timestamp>/`, et invoque immediatement le
  helper PoR existant (`iroha app da prove`) avec les bytes recuperes. Les operateurs
  peuvent ajuster les knobs de l'orchestrateur (`--max-peers`, `--scoreboard-out`,
  overrides d'endpoint manifest) et le sampler de proof (`--sample-count`,
  `--leaf-index`, `--sample-seed`) tandis qu'une seule commande produit les
  artefacts attendus par les audits DA-5/DA-9: copie du payload, evidence de
  scoreboard et resumes de preuve JSON.
