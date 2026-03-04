---
lang: fr
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T15:38:30.661072+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plan d'ingestion de disponibilité des données Sora Nexus

_Rédigé : 2026-02-20 - Propriétaire : Core Protocol WG / Storage Team / DA WG_

Le flux de travail DA-2 étend Torii avec une API d'ingestion de blob qui émet Norito.
métadonnées et graines de réplication SoraFS. Ce document capture la proposition
schéma, surface de l'API et flux de validation afin que la mise en œuvre puisse se poursuivre sans
blocage sur simulations en cours (suivis DA-1). Tous les formats de charge utile DOIVENT
utilisez les codecs Norito ; aucune solution de secours serde/JSON n'est autorisée.

## Objectifs

- Accepter les gros blobs (segments Taikai, side-cars de voie, artefacts de gouvernance)
  de manière déterministe sur Torii.
- Produire des manifestes canoniques Norito décrivant le blob, les paramètres du codec,
  profil d’effacement et politique de conservation.
- Conserver les métadonnées des blocs dans le stockage à chaud SoraFS et mettre en file d'attente les tâches de réplication.
- Publier les intentions de broches + les balises de stratégie dans le registre et la gouvernance SoraFS
  observateurs.
- Exposez les reçus d'admission afin que les clients retrouvent une preuve déterministe de publication.

## Surface API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

La charge utile est un `DaIngestRequest` codé en Norito. Utilisation des réponses
`application/norito+v1` et renvoyer `DaIngestReceipt`.

| Réponse | Signification |
| --- | --- |
| 202 Accepté | Blob mis en file d'attente pour le regroupement/la réplication ; reçu retourné. |
| 400 requêtes incorrectes | Violation de schéma/taille (voir contrôles de validation). |
| 401 Non autorisé | Jeton API manquant/invalide. |
| 409 Conflit | Dupliquez `client_blob_id` avec des métadonnées incompatibles. |
| 413 Charge utile trop importante | Dépasse la limite de longueur de blob configurée. |
| 429 Trop de demandes | La limite de débit a été atteinte. |
| 500 Erreur interne | Échec inattendu (enregistré + alerte). |

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

Renvoie un `DaProofPolicyBundle` versionné dérivé du catalogue de voies actuel.
Le bundle annonce `version` (actuellement `1`), un `policy_hash` (hachage du
liste de politiques ordonnée) et les entrées `policies` portant `lane_id`, `dataspace_id`,
`alias` et `proof_scheme` (`merkle_sha256` aujourd'hui ; les voies KZG sont
rejeté par ingest jusqu'à ce que les engagements KZG soient disponibles). L'en-tête du bloc maintenant
s'engage dans le bundle via `da_proof_policies_hash`, afin que les clients puissent épingler le
politique active définie lors de la vérification des engagements ou des preuves DA. Récupérer ce point de terminaison
avant de construire des preuves pour s’assurer qu’elles correspondent à la politique de la voie et à la situation actuelle
hachage groupé. La liste d'engagement/les points de terminaison de preuve portent le même ensemble, donc les SDK
vous n’avez pas besoin d’un aller-retour supplémentaire pour lier une preuve à l’ensemble de règles actif.

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

Renvoie un `DaProofPolicyBundle` contenant la liste de politiques ordonnée plus un
`policy_hash` afin que les SDK puissent épingler la version utilisée lors de la production d'un bloc. Le
Le hachage est calculé sur le tableau de politiques codé Norito et change chaque fois qu'un
Le `proof_scheme` de la voie est mis à jour, permettant aux clients de détecter la dérive entre
les preuves mises en cache et la configuration de la chaîne.

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
```> Note d'implémentation : les représentations canoniques de Rust pour ces charges utiles se trouvent désormais sous
> `iroha_data_model::da::types`, avec enveloppes de demande/réception dans `iroha_data_model::da::ingest`
> et la structure du manifeste dans `iroha_data_model::da::manifest`.

Le champ `compression` indique comment les appelants ont préparé la charge utile. Torii accepte
`identity`, `gzip`, `deflate` et `zstd`, décompressant de manière transparente les octets avant
hachage, segmentation et vérification des manifestes facultatifs.

### Liste de contrôle de validation

1. Vérifiez que l’en-tête de la demande Norito correspond à `DaIngestRequest`.
2. Échec si `total_size` diffère de la longueur canonique (décompressée) de la charge utile ou dépasse la longueur maximale configurée.
3. Appliquez l'alignement `chunk_size` (puissance de deux, = 2.
5. `retention_policy.required_replica_count` doit respecter les règles de base en matière de gouvernance.
6. Vérification de la signature par rapport au hachage canonique (hors champ de signature).
7. Rejetez le `client_blob_id` en double, sauf si le hachage de la charge utile + les métadonnées sont identiques.
8. Lorsque `norito_manifest` est fourni, vérifiez les correspondances schéma + hachage recalculées
   se manifeste après le découpage ; sinon, le nœud génère un manifeste et le stocke.
9. Appliquez la stratégie de réplication configurée : Torii réécrit le
   `RetentionPolicy` avec `torii.da_ingest.replication_policy` (voir
   `replication_policy.md`) et rejette les manifestes prédéfinis dont la conservation
   les métadonnées ne correspondent pas au profil appliqué.

### Flux de segmentation et de réplication1. Regroupez la charge utile dans `chunk_size`, calculez BLAKE3 par morceau + racine Merkle.
2. Construisez Norito `DaManifestV1` (nouvelle structure) capturant les engagements de fragments (role/group_id),
   disposition d'effacement (nombre de parité de lignes et de colonnes plus `ipa_commitment`), politique de rétention,
   et les métadonnées.
3. Mettez en file d'attente les octets du manifeste canonique sous `config.da_ingest.manifest_store_dir`.
   (Torii écrit les fichiers `manifest.encoded` saisis par voie/époque/séquence/billet/empreinte digitale) donc SoraFS
   l'orchestration peut les ingérer et lier le ticket de stockage aux données persistantes.
4. Publiez les intentions de broche via `sorafs_car::PinIntent` avec la balise de gouvernance + la politique.
5. Émettez l'événement Norito `DaIngestPublished` pour avertir les observateurs (clients légers,
   gouvernance, analytique).
6. Renvoyez `DaIngestReceipt` (signé par la clé de service Torii DA) et ajoutez le
   En-tête de réponse `Sora-PDP-Commitment` contenant l'encodage base64 Norito
   de l'engagement dérivé afin que les SDK puissent stocker immédiatement la graine d'échantillonnage.
   Le reçu intègre désormais `rent_quote` (un `DaRentQuote`) et `stripe_layout`
   afin que les soumissionnaires puissent faire apparaître les obligations XOR, la part de réserve, les attentes de bonus PDP/PoTR,
   et les dimensions de la matrice d'effacement 2D ainsi que les métadonnées du ticket de stockage avant d'engager les fonds.
7. Métadonnées facultatives du registre :
   - `da.registry.alias` — chaîne d'alias UTF-8 publique et non chiffrée pour amorcer l'entrée de registre du code PIN.
   - `da.registry.owner` — chaîne `AccountId` publique et non chiffrée pour enregistrer la propriété du registre.
   Torii les copie dans le `DaPinIntent` généré afin que le traitement des broches en aval puisse lier les alias.
   et les propriétaires sans réanalyser la carte brute des métadonnées ; les valeurs mal formées ou vides sont rejetées lors
   ingérer la validation.

## Mises à jour du stockage/du registre

- Étendez `sorafs_manifest` avec `DaManifestV1`, permettant une analyse déterministe.
- Ajout d'un nouveau flux de registre `da.pin_intent` avec référencement de charge utile versionné
  hachage manifeste + identifiant du ticket.
- Mettre à jour les pipelines d'observabilité pour suivre la latence d'ingestion, le débit de fragmentation,
  le retard de réplication et le nombre d’échecs.
- Les réponses Torii `/status` incluent désormais un tableau `taikai_ingest` qui fait apparaître les dernières
  latence de l'encodeur à l'acquisition, dérive du live-edge et compteurs d'erreurs par (cluster, flux), permettant au DA-9
  tableaux de bord pour ingérer des instantanés de santé directement à partir des nœuds sans supprimer Prometheus.

## Stratégie de test- Tests unitaires pour la validation des schémas, vérifications des signatures, détection des doublons.
- Tests Golden vérifiant l'encodage Norito de `DaIngestRequest`, le manifeste et le reçu.
- Harnais d'intégration exécutant la simulation SoraFS + registre, affirmant les flux chunk + pin.
- Tests de propriété couvrant des profils d'effacement aléatoires et des combinaisons de rétention.
- Fuzzing des charges utiles Norito pour se prémunir contre les métadonnées mal formées.
- Des luminaires dorés pour chaque classe de blob en direct
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` avec un morceau compagnon
  inscription dans `fixtures/da/ingest/sample_chunk_records.txt`. Le test ignoré
  `regenerate_da_ingest_fixtures` rafraîchit les appareils, tandis que
  `manifest_fixtures_cover_all_blob_classes` échoue dès qu'une nouvelle variante `BlobClass` est ajoutée
  sans mettre à jour le bundle Norito/JSON. Cela permet de conserver Torii, les SDK et les documents honnêtes à chaque fois que DA-2
  accepte une nouvelle surface blob.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## Outils CLI et SDK (DA-8)- `iroha app da submit` (nouveau point d'entrée CLI) encapsule désormais le générateur/éditeur d'ingestion partagé afin que les opérateurs
  peut ingérer des blobs arbitraires en dehors du flux de bundle Taikai. Le commandement réside dans
  `crates/iroha_cli/src/commands/da.rs:1` et consomme une charge utile, un profil d'effacement/conservation et
  fichiers de métadonnées/manifestes facultatifs avant de signer le canonique `DaIngestRequest` avec la CLI
  clé de configuration. Les exécutions réussies persistent `da_request.{norito,json}` et `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (remplacement via `--artifact-dir`) pour que les artefacts de libération puissent
  enregistrez les octets Norito exacts utilisés lors de l'ingestion.
- La commande est par défaut `client_blob_id = blake3(payload)` mais accepte les remplacements via
  `--client-blob-id`, honore les cartes JSON de métadonnées (`--metadata-json`) et les manifestes pré-générés
  (`--manifest`) et prend en charge `--no-submit` pour la préparation hors ligne, plus `--endpoint` pour la préparation personnalisée.
  Hôtes Torii. Le reçu JSON est imprimé sur la sortie standard en plus d'être écrit sur le disque, fermant ainsi le
  Exigence d'outillage DA-8 « submit_blob » et déblocage du travail de parité du SDK.
- `iroha app da get` ajoute un alias axé sur DA pour l'orchestrateur multi-source qui alimente déjà
  `iroha app sorafs fetch`. Les opérateurs peuvent le pointer vers des artefacts de manifeste + plan de bloc (`--manifest`,
  `--plan`, `--manifest-id`) **ou** transmettez simplement un ticket de stockage Torii via `--storage-ticket`. Quand le
  le chemin du ticket est utilisé, la CLI extrait le manifeste de `/v1/da/manifests/<ticket>`, conserve le bundle
  sous `artifacts/da/fetch_<timestamp>/` (remplacement par `--manifest-cache-dir`), dérive le **manifeste
  hash** pour `--manifest-id`, puis exécute l'orchestrateur avec le `--gateway-provider` fourni
  liste. La vérification de la charge utile repose toujours sur le résumé CAR/`blob_hash` intégré tandis que l'identifiant de la passerelle est
  maintenant le hachage du manifeste afin que les clients et les validateurs partagent un seul identifiant blob. Tous les boutons avancés de
  la surface du récupérateur SoraFS intacte (enveloppes de manifestes, étiquettes clients, caches de garde, transport anonymisé)
  remplacements, exportation du tableau de bord et chemins `--output`), et le point de terminaison du manifeste peut être remplacé via
  `--manifest-endpoint` pour les hôtes Torii personnalisés, de sorte que les contrôles de disponibilité de bout en bout se déroulent entièrement sous le contrôle
  Espace de noms `da` sans dupliquer la logique de l'orchestrateur.
- `iroha app da get-blob` extrait les manifestes canoniques directement de Torii via `GET /v1/da/manifests/{storage_ticket}`.
  La commande étiquette désormais les artefacts avec le hachage manifeste (identifiant blob), en écrivant
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` et `chunk_plan_{manifest_hash}.json`
  sous `artifacts/da/fetch_<timestamp>/` (ou un `--output-dir` fourni par l'utilisateur) tout en faisant écho à l'exact
  Appel `iroha app da get` (y compris `--manifest-id`) requis pour la récupération de suivi de l'orchestrateur.
  Cela maintient les opérateurs hors des répertoires de spool de manifeste et garantit que le récupérateur utilise toujours le
  artefacts signés émis par Torii. Le client JavaScript Torii reflète ce flux via
  `ToriiClient.getDaManifest(storageTicketHex)` tandis que le SDK Swift expose désormais
  `ToriiClient.getDaManifestBundle(...)`. Les deux renvoient les octets Norito décodés, le JSON manifeste, le hachage manifeste,et un plan de fragmentation afin que les appelants du SDK puissent hydrater les sessions de l'orchestrateur sans avoir recours à la CLI et à Swift
  les clients peuvent également appeler `fetchDaPayloadViaGateway(...)` pour acheminer ces bundles via le natif
  Wrapper d'orchestrateur SoraFS.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- Les réponses `/v1/da/manifests` font désormais apparaître `manifest_hash` et les deux assistants CLI + SDK (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway` et les wrappers de passerelle Swift/JS) traitent ce résumé comme le
  identifiant de manifeste canonique tout en continuant à vérifier les charges utiles par rapport au hachage CAR/blob intégré.
- `iroha app da rent-quote` calcule les répartitions déterministes des loyers et des incitations pour une taille de stockage fournie
  et fenêtre de rétention. L'assistant consomme soit le `DaRentPolicyV1` actif (octets JSON ou Norito), soit
  la valeur par défaut intégrée, valide la stratégie et imprime un résumé JSON (`gib`, `months`, métadonnées de stratégie,
  et les champs `DaRentQuote`) afin que les auditeurs puissent citer les frais XOR exacts dans les procès-verbaux de gouvernance sans
  écrire des scripts ad hoc. La commande émet désormais également un résumé `rent_quote ...` d'une seule ligne avant le JSON
  charge utile pour faciliter l’analyse des journaux de console et des runbooks lorsque des devis sont générés lors d’incidents.
  Passer `--quote-out artifacts/da/rent_quotes/<stamp>.json` (ou tout autre chemin)
  pour conserver le résumé joliment imprimé et utiliser `--policy-label "governance ticket #..."` lorsque le
  l'artefact doit citer un bundle de vote/configuration spécifique ; la CLI coupe les étiquettes personnalisées et rejette les blancs
  chaînes pour conserver les valeurs `policy_source` significatives dans les ensembles de preuves. Voir
  `crates/iroha_cli/src/commands/da.rs` pour la sous-commande et `docs/source/da/rent_policy.md`
  pour le schéma de politique.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- La parité du registre des broches s'étend désormais aux SDK : `ToriiClient.registerSorafsPinManifest(...)` dans le
  Le SDK JavaScript crée la charge utile exacte utilisée par `iroha app sorafs pin register`, appliquant le code canonique
  métadonnées de chunker, politiques d'épinglage, preuves d'alias et résumés de successeurs avant de les publier sur
  `/v1/sorafs/pin/register`. Cela empêche les robots CI et l'automatisation de s'adresser à la CLI lorsque
  enregistrement des enregistrements de manifeste, et l'assistant est livré avec une couverture TypeScript/README afin que le DA-8
  La parité des outils « soumettre/obtenir/prove » est pleinement satisfaite sur JS aux côtés de Rust/Swift.
- `iroha app da prove-availability` enchaîne tout ce qui précède : il prend un ticket de stockage, télécharge le
  bundle de manifeste canonique, exécute l'orchestrateur multi-source (`iroha app sorafs fetch`) sur le
  liste `--gateway-provider` fournie, conserve la charge utile téléchargée + le tableau de bord sous
  `artifacts/da/prove_availability_<timestamp>/`, et appelle immédiatement l'assistant PoR existant
  (`iroha app da prove`) en utilisant les octets récupérés. Les opérateurs peuvent modifier les boutons de l'orchestrateur
  (`--max-peers`, `--scoreboard-out`, remplacements de point de terminaison du manifeste) et l'échantillonneur de preuves
  (`--sample-count`, `--leaf-index`, `--sample-seed`) alors qu'une seule commande produit les artefacts
  attendu par les audits DA-5/DA-9 : copie de la charge utile, preuves du tableau de bord et résumés des preuves JSON.- `da_reconstruct` (nouveau dans DA-6) lit un manifeste canonique ainsi que le répertoire du chunk émis par le chunk
  store (mise en page `chunk_{index:05}.bin`) et réassemble de manière déterministe la charge utile tout en vérifiant
  chaque engagement de Blake3. La CLI réside sous `crates/sorafs_car/src/bin/da_reconstruct.rs` et est livrée sous le nom
  fait partie de l'ensemble d'outils SoraFS. Flux typique :
  1. `iroha app da get-blob --storage-ticket <ticket>` pour télécharger `manifest_<manifest_hash>.norito` et le plan de fragmentation.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (ou `iroha app da prove-availability`, qui écrit les artefacts de récupération sous
     `artifacts/da/prove_availability_<ts>/` et conserve les fichiers par fragment dans le répertoire `chunks/`).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  Un appareil de régression réside sous `fixtures/da/reconstruct/rs_parity_v1/` et capture le manifeste complet
  et matrice de morceaux (données + parité) utilisée par `tests::reconstructs_fixture_with_parity_chunks`. Régénérez-le avec

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  Le luminaire émet :

  - `manifest.{norito.hex,json}` — encodages canoniques `DaManifestV1`.
  - `chunk_matrix.json` — lignes ordonnées d'index/décalage/longueur/digest/parité pour les références de documentation/test.
  - `chunks/` — Tranches de charge utile `chunk_{index:05}.bin` pour les fragments de données et de parité.
  - `payload.bin` — charge utile déterministe utilisée par le test de harnais sensible à la parité.
  - `commitment_bundle.{json,norito.hex}` — exemple `DaCommitmentBundle` avec un engagement KZG déterministe pour les documents/tests.

  Le harnais refuse les morceaux manquants ou tronqués, vérifie le hachage Blake3 de la charge utile finale par rapport à `blob_hash`,
  et émet un blob JSON récapitulatif (octets de charge utile, nombre de blocs, ticket de stockage) afin que CI puisse affirmer la reconstruction
  des preuves. Cela clôt l'exigence DA-6 concernant un outil de reconstruction déterministe que les opérateurs et l'assurance qualité
  les tâches peuvent être invoquées sans câbler de scripts sur mesure.

## TODO Résumé de la résolution

Toutes les TODO d'ingestion précédemment bloquées ont été implémentées et vérifiées :- **Conseils de compression** : Torii accepte les étiquettes fournies par l'appelant (`identity`, `gzip`, `deflate`,
  `zstd`) et normalise les charges utiles avant validation afin que le hachage canonique du manifeste corresponde au
  octets décompressés.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Chiffrement des métadonnées de gouvernance uniquement** — Torii chiffre désormais les métadonnées de gouvernance avec le
  configuré la clé ChaCha20-Poly1305, rejette les étiquettes incompatibles et fait apparaître deux explicites
  boutons de configuration (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) pour maintenir la rotation déterministe.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Diffusion de charges utiles volumineuses** : l'ingestion en plusieurs parties est en direct. Les clients sont déterministes
  Enveloppes `DaIngestChunk` saisies par `client_blob_id`, Torii valide chaque tranche, les met en scène
  sous `manifest_store_dir`, et reconstruit atomiquement le manifeste une fois que le drapeau `is_last` atterrit,
  éliminant les pics de RAM observés avec les téléchargements par appel unique.【crates/iroha_torii/src/da/ingest.rs:392】
- **Gestion des versions du manifeste** — `DaManifestV1` comporte un champ `version` explicite et Torii refuse
  versions inconnues, garantissant des mises à niveau déterministes lorsque de nouvelles présentations de manifeste sont expédiées.【crates/iroha_data_model/src/da/types.rs:308】
- **Hooks PDP/PoTR** — Les engagements PDP dérivent directement du magasin de blocs et sont conservés
  à côté des manifestes afin que les planificateurs DA-5 puissent lancer des défis d'échantillonnage à partir de données canoniques ; le
  L'en-tête `Sora-PDP-Commitment` est désormais livré avec `/v1/da/ingest` et `/v1/da/manifests/{ticket}`.
  réponses afin que les SDK apprennent immédiatement l'engagement signé auquel les futures sondes feront référence.
- **Journal du curseur de fragment** — les métadonnées de voie peuvent spécifier `da_shard_id` (par défaut `lane_id`), et
  Sumeragi conserve désormais le `(epoch, sequence)` le plus élevé par `(shard_id, lane_id)` dans
  `da-shard-cursors.norito` à côté de la bobine DA, donc les redémarrages suppriment les voies reparties/inconnues et conservent
  rejouer déterministe. L'index du curseur de partition en mémoire échoue désormais rapidement sur les engagements pour
  voies non cartographiées au lieu d'utiliser par défaut l'identifiant de la voie, ce qui entraîne des erreurs d'avancement du curseur et de relecture
  explicite, et la validation par bloc rejette les régressions de curseur de fragment avec un
  Raison `DaShardCursorViolation` + étiquettes de télémétrie pour les opérateurs. Le démarrage/rattrapage arrête désormais DA
  indice d'hydratation si Kura contient une voie inconnue ou un curseur en régression et enregistre l'infraction
  hauteur de bloc afin que les opérateurs puissent y remédier avant de servir DA state.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Télémétrie du décalage du curseur de fragment** — la jauge `da_shard_cursor_lag_blocks{lane,shard}` indique commentde loin un éclat traîne la hauteur en cours de validation. Les voies manquantes/périmées/inconnues règlent le décalage sur le
  la hauteur requise (ou delta) et les avancées réussies la remettent à zéro afin que l'état stable reste plat.
  Les opérateurs doivent déclencher une alarme en cas de décalage non nul, inspecter la bobine/le journal DA pour détecter la voie incriminée,
  et vérifiez le catalogue de voies pour tout repartitionnement accidentel avant de rejouer le bloc pour effacer le
  écart.
- **Voies de calcul confidentielles** — voies marquées d'un
  `metadata.confidential_compute=true` et un `confidential_key_version` sont traités comme
  Chemins SMPC/DA chiffrés : Sumeragi applique des résumés de charge utile/manifeste non nuls et des tickets de stockage,
  rejette les profils de stockage de réplique complète et indexe la version du ticket SoraFS + la stratégie sans
  exposant les octets de charge utile. Les reçus s'hydratent de Kura pendant la relecture afin que les validateurs récupèrent les mêmes
  métadonnées de confidentialité après le redémarrage.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## Notes de mise en œuvre- Le point de terminaison `/v1/da/ingest` de Torii normalise désormais la compression de la charge utile, applique le cache de relecture,
  décompose de manière déterministe les octets canoniques, reconstruit `DaManifestV1` et supprime la charge utile codée
  dans `config.da_ingest.manifest_store_dir` pour l'orchestration SoraFS avant d'émettre le récépissé ; le
  le gestionnaire attache également un en-tête `Sora-PDP-Commitment` afin que les clients puissent capturer l'engagement codé
  immédiatement.【crates/iroha_torii/src/da/ingest.rs:220】
- Après avoir persisté le canonique `DaCommitmentRecord`, Torii émet désormais un
  Fichier `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` à côté du spool de manifeste.
  Chaque entrée regroupe l'enregistrement avec les octets bruts Norito `PdpCommitment` afin que les constructeurs de bundles DA-3 et
  Les planificateurs DA-5 ingèrent des entrées identiques sans relire les manifestes ou les magasins de fragments.【crates/iroha_torii/src/da/ingest.rs:1814】
- Les assistants du SDK exposent les octets d'en-tête PDP sans obliger chaque client à réimplémenter l'analyse Norito :
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` housse Rust, le Python `ToriiClient`
  exporte désormais `decode_pdp_commitment_header`, et `IrohaSwift` expédie les aides correspondantes de manière si mobile
  les clients peuvent immédiatement cacher le programme d'échantillonnage codé.
- Torii expose également `GET /v1/da/manifests/{storage_ticket}` afin que les SDK et les opérateurs puissent récupérer les manifestes
  et fragmentez les plans sans toucher au répertoire spool du nœud. La réponse renvoie les octets Norito
  (base64), rendu JSON manifeste, un blob JSON `chunk_plan` prêt pour `sorafs fetch`, ainsi que le
  des résumés hexadécimaux (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) afin que les outils en aval puissent
  alimente l'orchestrateur sans recalculer les résumés et émet le même en-tête `Sora-PDP-Commitment` vers
  refléter les réponses d’ingestion. Passer `block_hash=<hex>` comme paramètre de requête renvoie un résultat déterministe
  `sampling_plan` enraciné dans `block_hash || client_blob_id` (partagé entre les validateurs) contenant le
  `assignment_hash`, le `sample_window` demandé et les tuples `(index, role, group)` échantillonnés couvrant
  l'intégralité de la disposition des bandes 2D afin que les échantillonneurs et validateurs PoR puissent rejouer les mêmes indices. L'échantillonneur
  mélange `client_blob_id`, `chunk_root` et `ipa_commitment` dans le hachage d'affectation ; `Iroha App Da Get
  --block-hash ` now writes `sampling_plan_.json` à côté du manifeste + plan de fragments avec
  le hachage est conservé et les clients JS/Swift Torii exposent le même `assignment_hash_hex` donc les validateurs
  et les prouveurs partagent un seul ensemble de sondes déterministes. Lorsque Torii renvoie un plan d'échantillonnage, `iroha app da
  prouver-disponibilité` now reuses that deterministic probe set (seed derived from `sample_seed`) à la place
  d'échantillonnage ad hoc afin que les témoins PoR s'alignent sur les missions du validateur même si l'opérateur omet un
  `--block-hash` override.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Flux de streaming de charges utiles importantesLes clients qui doivent ingérer des actifs supérieurs à la limite de requête unique configurée lancent une
session de streaming en appelant `POST /v1/da/ingest/chunk/start`. Torii répond avec un
`ChunkSessionId` (BLAKE3-dérivé des métadonnées blob demandées) et la taille du bloc négociée.
Chaque requête `DaIngestChunk` suivante transporte :

- `client_blob_id` — identique au `DaIngestRequest` final.
- `chunk_session_id` — lie les tranches à la session en cours.
- `chunk_index` et `offset` — appliquent un ordre déterministe.
- `payload` — jusqu'à la taille de bloc négociée.
- `payload_hash` — Hachage BLAKE3 de la tranche afin que Torii puisse valider sans mettre en mémoire tampon l'intégralité du blob.
- `is_last` — indique la tranche terminale.

Torii conserve les tranches validées sous `config.da_ingest.manifest_store_dir/chunks/<session>/` et
enregistre la progression dans le cache de relecture pour honorer l'idempotence. Lorsque la tranche finale atterrit, Torii
réassemble la charge utile sur le disque (en streaming via le répertoire de morceaux pour éviter les pics de mémoire),
calcule le manifeste/reçu canonique exactement comme pour les téléchargements en une seule fois, et répond enfin à
`POST /v1/da/ingest` en consommant l'artefact intermédiaire. Les sessions ayant échoué peuvent être abandonnées explicitement ou
sont récupérés après `config.da_ingest.replay_cache_ttl`. Cette conception conserve le format réseau
Compatible avec Norito, évite les protocoles de reprise spécifiques au client et réutilise le pipeline de manifeste existant
inchangé.

**État de mise en œuvre.** Les types canoniques Norito résident désormais dans
`crates/iroha_data_model/src/da/` :

- `ingest.rs` définit `DaIngestRequest`/`DaIngestReceipt`, ainsi que le
  Conteneur `ExtraMetadata` utilisé par Torii.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` héberge `DaManifestV1` et `ChunkCommitment`, que Torii émet après
  le découpage est terminé.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` fournit des alias partagés (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile`, etc.) et code les valeurs de politique par défaut documentées ci-dessous.【crates/iroha_data_model/src/da/types.rs:240】
- Les fichiers spool du manifeste atterrissent dans `config.da_ingest.manifest_store_dir`, prêts pour l'orchestration SoraFS.
  l'observateur doit accéder à l'admission au stockage.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi applique la disponibilité du manifeste lors de la scellement ou de la validation des bundles DA :
  les blocs échouent à la validation si le spool manque le manifeste ou si le hachage diffère
  de l'engagement.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

La couverture aller-retour pour les charges utiles de demande, de manifeste et de reçu est suivie dans
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, assurant le codec Norito
reste stable à travers les mises à jour.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Défauts de rétention.** La gouvernance a ratifié la politique de rétention initiale pendant
SF-6 ; les valeurs par défaut appliquées par `RetentionPolicy::default()` sont :- niveau chaud : 7 jours (`604_800` secondes)
- niveau froid : 90 jours (`7_776_000` secondes)
- répliques requises : `3`
- classe de stockage : `StorageClass::Hot`
- balise de gouvernance : `"da.default"`

Les opérateurs en aval doivent remplacer explicitement ces valeurs lorsqu'une voie adopte
des exigences plus strictes.

## Artefacts de preuve client Rust

Les SDK qui intègrent le client Rust n'ont plus besoin de passer par la CLI pour
produire le bundle canonique PoR JSON. Le `Client` expose deux assistants :

- `build_da_proof_artifact` renvoie la structure exacte générée par
  `iroha app da prove --json-out`, y compris les annotations de manifeste/charge utile fournies
  via [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` encapsule le constructeur et conserve l'artefact sur le disque
  (joli JSON + nouvelle ligne de fin par défaut) pour que l'automatisation puisse joindre le fichier
  aux versions ou aux ensembles de preuves de gouvernance.【crates/iroha/src/client.rs:3653】

### Exemple

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

La charge utile JSON qui quitte l'assistant correspond à la CLI jusqu'aux noms de champs
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest`, etc.), donc existant
l'automatisation peut comparer/parqueter/télécharger le fichier sans branches spécifiques au format.

## Benchmark de vérification des preuves

Utilisez le harnais de référence DA proof pour valider les budgets des vérificateurs sur des charges utiles représentatives avant
serrage des bouchons au niveau des blocs :

- `cargo xtask da-proof-bench` reconstruit le magasin de morceaux à partir de la paire manifeste/charge utile, échantillonne PoR
  les congés et la vérification des heures par rapport au budget configuré. Les métadonnées Taikai sont remplies automatiquement et le
  le harnais revient à un manifeste synthétique si la paire de luminaires est incohérente. Quand `--payload-bytes`
  est défini sans `--payload` explicite, le blob généré est écrit dans
  `artifacts/da/proof_bench/payload.bin` pour que les appareils restent intacts.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Les rapports sont par défaut `artifacts/da/proof_bench/benchmark.{json,md}` et incluent les épreuves/exécutions, le total et
  les délais par épreuve, le taux de réussite du budget et un budget recommandé (110 % de l'itération la plus lente) pour
  alignez-vous avec `zk.halo2.verifier_budget_ms`.【artifacts/da/proof_bench/benchmark.md:1】
- Dernière exécution (charge utile synthétique de 1 Mio, morceaux de 64 Ko, 32 épreuves/exécution, 10 itérations, budget de 250 ms)
  recommandé un budget de vérificateur de 3 ms avec 100 % des itérations à l'intérieur du plafond.【artifacts/da/proof_bench/benchmark.md:1】
- Exemple (génère une charge utile déterministe et écrit les deux rapports) :

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

L'assemblage en bloc applique les mêmes budgets : `sumeragi.da_max_commitments_per_block` et
`sumeragi.da_max_proof_openings_per_block` contrôle le bundle DA avant qu'il ne soit intégré dans un bloc, et
chaque engagement doit porter un `proof_digest` non nul. La garde considère la longueur du paquet comme étant la
nombre d'ouvertures de preuves jusqu'à ce que des résumés de preuves explicites soient transmis par consensus, en gardant le
Cible d'ouverture ≤ 128 applicable à la limite du bloc. 【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## Gestion et réduction des échecs PoRLes employés du stockage font désormais apparaître des séquences de défaillances PoR et des recommandations de barres obliques liées à côté de chacune d'entre elles.
jugement. Les échecs consécutifs au-dessus du seuil d'attaque configuré émettent une recommandation qui
inclut la paire fournisseur/manifeste, la longueur de la séquence qui a déclenché la barre oblique et la proposition
pénalité calculée à partir de la caution du prestataire et du `penalty_bond_bps` ; les fenêtres de temps de recharge (secondes) sont conservées
dupliquez les barres obliques dues au tir lors du même incident.

- Configurer les seuils/temps de recharge via le générateur de travailleurs de stockage (les valeurs par défaut reflètent la gouvernance
  politique de pénalité).
- Les recommandations Slash sont enregistrées dans le résumé du verdict JSON afin que la gouvernance/les auditeurs puissent les joindre
  les à des paquets de preuves.
- La disposition Stripe + les rôles par morceau sont désormais transmis via le point de terminaison de la broche de stockage de Torii.
  (champs `stripe_layout` + `chunk_roles`) et persisté dans le gestionnaire de stockage afin
  les auditeurs/outils de réparation peuvent planifier les réparations des lignes/colonnes sans re-dériver la disposition en amont

### Placement + harnais de réparation

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` maintenant
calcule un hachage de placement sur `(index, role, stripe/column, offsets)` et effectue d'abord la ligne, puis
réparation de la colonne RS(16) avant de reconstruire la charge utile :

- Le placement est par défaut `total_stripes`/`shards_per_stripe` lorsqu'il est présent et revient au bloc
- Les morceaux manquants/corrompus sont reconstruits en premier avec la parité des lignes ; les lacunes restantes sont réparées avec
  parité de bande (colonne). Les morceaux réparés sont réécrits dans le répertoire des morceaux et le JSON
  Le résumé capture le hachage de placement ainsi que les compteurs de réparation de ligne/colonne.
- Si la parité ligne+colonne ne peut pas satisfaire l'ensemble manquant, le faisceau échoue rapidement avec l'irrécupérable.
  indices afin que les auditeurs puissent signaler les manifestes irréparables.