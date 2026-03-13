---
lang: fr
direction: ltr
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Fuente canonica
Refleja `docs/source/da/ingest_plan.md`. Mantenga ambas versions fr
:::

# Plan d'ingestion de disponibilité des données de Sora Nexus

_Rédigé : 2026-02-20 - Responsable : Core Protocol WG / Storage Team / DA WG_

Le flux de travail DA-2 étend Torii avec une API d'acquisition de blobs qui émettent
métadonnées Norito et afficher la réplication de SoraFS. Ce document capture le
esquema propuesto, la surface de l'API et le flux de validation pour que la
la mise en œuvre avancée sans bloquer les simulations pendantes (suites
DA-1). Tous les formats de charge utile DEBEN utilisent les codecs Norito ; ne se permet pas
solutions de secours serde/JSON.

## Objets

- Accepter les blobs grandes (segments Taikai, side-cars de lane, artefactos de
  gobernanza) de forme déterministe via Torii.
- Produire les manifestes Norito canoniques qui décrivent le blob, paramètres de
  codec, profil d'effacement et politique de rétention.
- Conserver les métadonnées des morceaux dans l'espace de stockage chaud de SoraFS et insérer les tâches de
  réplication.
- Publier les intentions de broche + les balises politiques dans le registre SoraFS et les observateurs
  de gobernanza.
- Exposer des recettes d'admission pour que les clients récupèrent des tests déterministes
  de publication.

## API Superficie (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```La charge utile est un `DaIngestRequest` codifié en Norito. Las réponses usan
`application/norito+v1` et développé `DaIngestReceipt`.

| Réponse | Signifié |
| --- | --- |
| 202 Accepté | Blob et cola pour le chunking/réplication ; se dévuelve el recibo. |
| 400 requêtes incorrectes | Violation de esquema/tamano (ver validaciones). |
| 401 Non autorisé | API de jeton autorisée/invalide. |
| 409 Conflit | Duplication `client_blob_id` avec métadonnées sans coïncidence. |
| 413 Charge utile trop importante | Dépasser la limite configurée de longitude du blob. |
| 429 Trop de demandes | Se alcanzo el taux limite. |
| 500 Erreur interne | Fallo inesperado (log + alerta). |

## Esquema Norito proposé

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

> Note de mise en œuvre : les représentations canoniques en Rust pour cela
> charges utiles maintenant viven bas `iroha_data_model::da::types`, avec wrappers de
> demande/réception en `iroha_data_model::da::ingest` et la structure du manifeste
> fr `iroha_data_model::da::manifest`.

Le champ `compression` déclare que les appelants préparent la charge utile. Torii
accepte `identity`, `gzip`, `deflate`, et `zstd`, décomprimant les octets avant de
hashear, chunking et vérifier les manifestes optionnels.

### Checklist de validation1. Vérifiez que l'en-tête Norito de la demande coïncide avec `DaIngestRequest`.
2. Fallar si `total_size` diffère de la grande taille canonique de la charge utile (décomprimée)
   o dépasser le maximum configuré.
3. Forzar alineacion de `chunk_size` (puissance de deux, = 2.
5. `retention_policy.required_replica_count` doit respecter la ligne de base de
   gobernanza.
6. Vérification de l'entreprise contre le hachage canonique (à l'exclusion de la signature du champ).
7. Rechazar duplicado `client_blob_id` salvo que el hash del payload y la
   métadonnées sean identiques.
8. Lorsque vous prouvez `norito_manifest`, vérifiez que le schéma + le hachage coïncident avec le
   manifeste recalculé après le chunking ; de lo contrario el nodo genera el
   manifeste et lo almacena.
9. Forzar la politique de réplication configurée : Torii réécrire le
   `RetentionPolicy` envoyé avec `torii.da_ingest.replication_policy` (version
   `replication-policy.md`) et rechaza manifeste des métadonnées préconstruidos de
   la rétention ne coïncide pas avec le profil impuesto.

### Flux de chunking et de réplication1. Recherchez la charge utile dans `chunk_size`, calculez BLAKE3 par chunk + Raiz Merkle.
2. Construire Norito `DaManifestV1` (struct nouvelle) capturer les compromis de
   chunk (role/group_id), layout d'effacement (conteos de paridad de filas y
   columnas mas `ipa_commitment`), politique de rétention et métadonnées.
3. Encadrer les octets du manifeste canonique bas
   `config.da_ingest.manifest_store_dir` (Torii écrire des archives
   `manifest.encoded` por lane/epoch/sequence/ticket/fingerprint) pour que la
   orquestación de SoraFS los ingiera y vincule el storage ticket con datos
   persistants.
4. Publier les intentions de broche via `sorafs_car::PinIntent` avec la balise de gouvernement et
   politique.
5. Émettre l'événement Norito `DaIngestPublished` pour notifier les observateurs (clients
   ligeros, gobernanza, analitica).
6. Devolver `DaIngestReceipt` à l'appelant (firmé par la clé de service DA de
   Torii) et émettre l'en-tête `Sora-PDP-Commitment` pour que les SDK capturent le
   engagement codificado de inmediato. Le reçu aujourd'hui inclut `rent_quote`
   (un Norito `DaRentQuote`) et `stripe_layout`, permettant les versements
   afficher la base de location, la réserve, les attentes de bonus PDP/PoTR et le
   disposition d'effacement 2D avec le ticket de stockage avant le fond de compteur.

## Actualizaciones de almacenamiento/registry- Extender `sorafs_manifest` avec `DaManifestV1`, permettant l'analyse
  déterministe.
- Ajouter un nouveau flux de registre `da.pin_intent` avec la charge utile versionnée
  référence hash du manifeste + identifiant du ticket.
- Actualiser les pipelines d'observabilité pour suivre la latence d'ingesta,
  débit de chunking, backlog de réplication et conteos de fallos.

## Stratégie d'essai

- Tests unitaires pour la validation d'esquema, les contrôles d'entreprise et la détection de
  duplicados.
- Tests dorés vérifiant le codage Norito de `DaIngestRequest`, manifeste y
  reçu.
- Harnais d'intégration levantando SoraFS + registre simulados, validé
  flujos de chunk + pin.
- Tests de propriétés cubriendo profils d'effacement et combinaisons de
  rétention aléatoire.
- Fuzzing des charges utiles Norito pour protéger les métadonnées mal formées.

## Outillage de CLI et SDK (DA-8)- `iroha app da submit` (nouveau point d'entrée de CLI) maintenant disponible pour le constructeur/éditeur
  Compartiment à ingérer pour que les opérateurs puissent ingérer des blobs arbitraires
  paquet de Taikai hors du flux. Le commandement vive en
  `crates/iroha_cli/src/commands/da.rs:1` et consommer une charge utile, profil de
  effacement/conservation et archives optionnelles de métadonnées/manifestes avant l'entreprise
  le `DaIngestRequest` canonique avec la clé de configuration de la CLI. Les exécutions
  exitosas persisten `da_request.{norito,json}` et `da_receipt.{norito,json}` bas
  `artifacts/da/submission_<timestamp>/` (remplacement via `--artifact-dir`) pour que
  les artefacts de version enregistrent les octets Norito exacts utilisés pendant la
  ingérer.
- La commande utilisée par défaut `client_blob_id = blake3(payload)` est acceptée
  remplacements via `--client-blob-id`, correspondance avec les métadonnées JSON
  (`--metadata-json`) y manifeste des pré-générés (`--manifest`), et supporte
  `--no-submit` pour la préparation hors ligne des hommes `--endpoint` pour les hôtes Torii
  personnalisés. Le reçu JSON est imprimé en standard avant d'être écrit
  disco, vérifier les exigences de l'outillage "submit_blob" de DA-8 et débloquer
  le travail de parité du SDK.
- `iroha app da get` agrége un alias associé à DA pour l'explorateur multi-source
  que tu as la puissance `iroha app sorafs fetch`. Les opérateurs peuvent signaler un problème
  artefacts de manifeste + plan de fragments (`--manifest`, `--plan`, `--manifest-id`)**o** passer un ticket de stockage de Torii via `--storage-ticket`. Lorsque vous êtes aux États-Unis
  le chemin du ticket, la CLI baja le manifeste depuis `/v2/da/manifests/<ticket>`,
  conserver le bundle sous `artifacts/da/fetch_<timestamp>/` (override con
  `--manifest-cache-dir`), dériver le hachage du blob pour `--manifest-id`, et ensuite
  exécutez l’orchestre avec la liste `--gateway-provider` suministrada. Toutes les tâches
  les boutons avancés du récupérateur de SoraFS restent intacts (manifeste
  enveloppes, étiquettes de client, caches de garde, remplacements de transporte anonimo,
  l'exportation du tableau de bord et des chemins `--output`), et le point de terminaison du manifeste peut
  écrire via `--manifest-endpoint` pour les hôtes Torii personnalisés, comme
  que les vérifications de disponibilité de bout en bout soient totalement réalisées sous l'espace de noms
  `da` sans dupliquer la logique de l'orchestre.
- `iroha app da get-blob` baja manifeste canonicos directo depuis Torii via
  `GET /v2/da/manifests/{storage_ticket}`. El comando écris
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` et
  `chunk_plan_{ticket}.json` sous `artifacts/da/fetch_<timestamp>/` (ou un
  `--output-dir` à condition que l'utilisateur puisse imprimer la commande exacte de
  `iroha app da get` (y compris `--manifest-id`) requis pour la récupération du
  orchestre. Ceci maintient les opérateurs hors des directeurs de la bobine de
  manifeste et garantit que le récupérateur utilise toujours les artefacts firmadosémis par Torii. Le client Torii de JavaScript reflète le même flux via
  `ToriiClient.getDaManifest(storageTicketHex)`, transfert des octets Norito
  décodifiés, manifeste JSON et plan de blocs pour que les appelants du SDK soient cachés
  les sessions de l’orchestre sans utiliser la CLI. Le SDK de Swift explique maintenant
  mismas superficies (`ToriiClient.getDaManifestBundle(...)` mas
  `fetchDaPayloadViaGateway(...)`), canaliser les bundles dans l'emballage national du
  opérateur SoraFS pour que les clients iOS puissent télécharger des manifestes, les exécuter
  récupère plusieurs sources et capture des essais sans appeler la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcule les rentes déterministes et desglose de incentivos
  pour un tamano de stockage et une ventana de rétention suministrados. L'assistant
  consommer l'actif `DaRentPolicyV1` (JSON ou octets Norito) ou l'intégré par défaut,
  valider la politique et imprimer un CV JSON (`gib`, `months`, métadonnées de
  politica y campos de `DaRentQuote`) pour les auditeurs citen cargos XOR exactos
  en actas de gobernanza sin scripts ad hoc. Le commandant émet également un CV
  sur une ligne `rent_quote ...` avant la charge utile JSON pour rester lisible
  journaux de console pendant les exercices d'incidents. Empareje
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` avec
  `--policy-label "governance ticket #..."` pour conserver les bons objets quecitez le vote ou le bundle de configuration exacte ; la CLI enregistre l'étiquette personnalisée
  y rechaza strings vacios para que los valores `policy_source` se mantengan
  accionables aux tableaux de bord de tesoreria. Ver
  `crates/iroha_cli/src/commands/da.rs` pour la sous-commande et
  `docs/source/da/rent_policy.md` pour le modèle politique.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo lo anterior: toma un storage
  ticket, descarga el bundle canonico de manifest, ejecuta el orquestador
  multi-source (`iroha app sorafs fetch`) contre la liste `--gateway-provider`
  résumé, persister la charge utile téléchargée + tableau de bord bas
  `artifacts/da/prove_availability_<timestamp>/`, et appel immédiat de l'assistant
  PoR existant (`iroha app da prove`) utilise les octets restants. Les opérateurs
  Vous pouvez ajuster les boutons de l'orchestre (`--max-peers`, `--scoreboard-out`,
  remplace le point final du manifeste) et l'échantillonneur de preuve (`--sample-count`,
  `--leaf-index`, `--sample-seed`) entre une commande solo pour produire les
  artefacts attendus par les auditoires DA-5/DA-9 : copie de la charge utile, preuve de
  tableau de bord et résumé de l'essai JSON.