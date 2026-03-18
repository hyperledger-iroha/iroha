---
lang: fr
direction: ltr
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page correspond à `docs/source/da/ingest_plan.md`. Consultez les versions suivantes
:::

# Planifier l'ingestion de disponibilité des données Sora Nexus

_Черновик: 2026-02-20 — Groupes : Core Protocol WG / Storage Team / DA WG_

Le robot DA-2 a ingéré l'API Torii pour le blob, ce qui vous permet de le faire
Norito-métaданные и запускает репликацию SoraFS. Description du document
le système d'API et de validation de pot, qui permet de réaliser ce qui se passe sans blocage
ожидающих симуляциях (suivi DA-1). La charge utile de tous les formats est utilisée
codes Norito ; le repli sur serde/JSON n'est pas disponible.

## Celi

- Présence d'un plus grand blob (séquences Taikai, voie de side-car, mise en œuvre d'art.)
  Déterminez-le à partir du Torii.
- Создавать канонические Norito manifestes, описывающие blob, paramètres codec,
  effacement du profil et rétention politique.
- Enregistrez le bloc de métadonnées dans le stockage à chaud SoraFS et sauvegardez les réplications dans
  очередь.
- Publier les intentions de broches + les balises de stratégie dans le fichier SoraFS et sur le site
  управления.
- Obtenez les reçus d'admission, les clients sont autorisés à le faire
  подтверждение публикации.

## Surface API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Charge utile — c'est `DaIngestRequest`, correspondant à Norito. Les utilisateurs utilisent
`application/norito+v1` et `DaIngestReceipt`.| Ответ | Значение |
| --- | --- |
| 202 Accepté | Blob est posté pour le découpage/réplication ; reçu возвращен. |
| 400 requêtes incorrectes | Нарушение schema/размера (см. проверки). |
| 401 Non autorisé | Отсутствует/некорректен API-токен. |
| 409 Conflit | Publier `client_blob_id` avec la métadonnée requise. |
| 413 Charge utile trop importante | La limite précédente est celle du blob. |
| 429 Trop de demandes | Limite de taux précédente. |
| 500 Erreur interne | Неожиданная ошибка (log + alerte). |

## Schéma préliminaire Norito

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

> Exemples de réalisation : Canonique Rust-predstavления этих payload теперь
> находятся в `iroha_data_model::da::types`, avec les emballages de demande/réception dans
> `iroha_data_model::da::ingest` et manifeste structuré dans
> `iroha_data_model::da::manifest`.

Le pôle `compression` indique que l'appelant a transmis la charge utile. Torii принимает
`identity`, `gzip`, `deflate` et `zstd`, vous pouvez utiliser des batteries avant le hachage,
chunking и проверкой опциональных manifestes.

### Validation du chèque1. Vérifiez que l'appareil Norito correspond à `DaIngestRequest`.
2. L'entreprise, si `total_size` libère la charge utile des lignes canoniques
   (après l'intervention) ou en prévenant le maximum possible.
3. Enregistrez la version `chunk_size` (niveau de données, = 2.
5. `retention_policy.required_replica_count` должен соблюдать référence de gouvernance.
6. Proverka подписи по каноническому hash (без поля подписи).
7. Ouvrez les doubles `client_blob_id`, si la charge utile de hachage et les métadonnées ne sont pas
   идентичны.
8. Lorsque `norito_manifest` vérifie le schéma + le hachage pour la connexion
   manifeste, пересчитанным после chunking; иначе узел генерирует manifest и
   сохраняет его.
9. Применять настроенную политику репликации: Torii переписывает отправленной
   `RetentionPolicy` par `torii.da_ingest.replication_policy` (см.
   `replication-policy.md`) et ouvrir les manifestes, sinon
   La conservation des métadonnées n'est pas compatible avec le profil appliqué.

### Morceau de morceaux et répliques1. Téléchargez la charge utile sur `chunk_size`, lancez BLAKE3 pour votre chunk + Merkle
   racine.
2. Former Norito `DaManifestV1` (nouvelle structure), fixer le morceau d'engagement
   (role/group_id), disposition d'effacement (числа паритета строк и столбцов плюс
   `ipa_commitment`), rétention politique et métadonnées.
3. Publier le manifeste des octets canoniques dans la prochaine étape
   `config.da_ingest.manifest_store_dir` (Torii par `manifest.encoded` pour
   voie/époque/séquence/billet/empreinte digitale), чтобы оркестрация SoraFS могла
   поглотить их и связать ticket de stockage сохраненными данными.
4. Publier les intentions de broches à partir de `sorafs_car::PinIntent` avec cette mise à jour et
   politique.
5. Émettez le Norito `DaIngestPublished` pour l'utilisation du téléphone
   (clients légers, gouvernance, analytique).
6. Sélectionnez l'appelant `DaIngestReceipt` (en utilisant le bouton Torii DA) et désactivez-le.
   En utilisant `Sora-PDP-Commitment`, le SDK a pris un engagement. Reçu
   Veuillez sélectionner `rent_quote` (Norito `DaRentQuote`) et `stripe_layout`, vous
   Vous pouvez obtenir des fonds pour votre argent, sans réserve, sans bonus
   PDP/PoTR et disposition d'effacement 2D pour le ticket de stockage pour la fixation des emplacements.

## Обновления Stockage / Registre- Remplacez `sorafs_manifest` par le nouveau `DaManifestV1`, en fonction du détergent
  parsing.
- Ajouter le nouveau flux de registre `da.pin_intent` avec la charge utile de version,
  Il s'agit du hachage du manifeste + de l'identifiant du ticket.
- Afficher les plans d'observabilité pour la surveillance de l'ingestion et du débit
  chunking, réplication du backlog et des sauvegardes.

## Test de stratégie

- Tests unitaires pour la validation du schéma, les tests de validation, les détections de doubles.
- Golden-testы на проверки Norito encodage `DaIngestRequest`, manifeste et reçu.
- Harnais d'intégration, maquette SoraFS + registre et vérification
  потоки morceau + broche.
- Tests de propriété pour l'effacement du profil et la conservation combinée.
- Fuzzing de la charge utile Norito pour les métadonnées inappropriées.

## Outils CLI et SDK (DA-8)- `iroha app da submit` (nouveau point d'entrée CLI) prend en charge le générateur d'ingestion/
  éditeur, les opérateurs peuvent ingérer des blobs de produits вне потока
  Pack Taikaï. La commande est disponible dans `crates/iroha_cli/src/commands/da.rs:1` et
  Principes de charge utile, d'effacement/conservation de profil et de fichiers optionnels
  métadonnées/manifeste avant l'arrivée du canal canonique `DaIngestRequest` ключом
  configuration CLI. Les prises de courant `da_request.{norito,json}` et
  `da_receipt.{norito,json}` à la suite de `artifacts/da/submission_<timestamp>/`
  (remplacer par `--artifact-dir`), pour supprimer les artefacts corrigés
  Norito octets, utilisés lors de l'ingestion.
- La commande peut utiliser `client_blob_id = blake3(payload)`, mais
  peut remplacer les métadonnées des cartes JSON par `--client-blob-id`
  (`--metadata-json`) et manifestes pré-générés (`--manifest`), ainsi que
  `--no-submit` pour les ordinateurs d'État et `--endpoint` pour les hôtes Torii.
  Le reçu JSON est envoyé sur la sortie standard et téléchargé sur le disque, en téléchargeant le DA-8
  "submit_blob" et le blocage du programme de parité du SDK.
- `iroha app da get` crée un alias DA-oriентированный pour l'orchestrateur multi-source,
  который уже питает `iroha app sorafs fetch`. Les opérateurs peuvent acheter des artefacts
  manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou** avant
  Ticket de stockage Torii par `--storage-ticket`. Lors de l'utilisation du ticket CLI
  télécharger le manifeste depuis `/v1/da/manifests/<ticket>`, avec le bundle dans`artifacts/da/fetch_<timestamp>/` (remplacer par `--manifest-cache-dir`), выводит
  Hachage de blob pour `--manifest-id` et installation d'orchestrator à votre place
  `--gateway-provider` списком. Vous trouverez des boutons pour le récupérateur SoraFS
  сохраняются (enveloppes de manifestes, étiquettes de clients, caches de garde, anonymes
  remplacements de transport, tableau de bord des exportations, `--output` пути), un point final du manifeste
  Vous pouvez utiliser `--manifest-endpoint` pour les hôtes Torii,
  Ainsi, la disponibilité de bout en bout est vérifiée dans l'espace de noms `da` sans
  дублирования orchestrator logiques.
- `iroha app da get-blob` забирает канонические manifestes напрямую из Torii ici
  `GET /v1/da/manifests/{storage_ticket}`. Commanda Pischet
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` et `chunk_plan_{ticket}.json`
  dans `artifacts/da/fetch_<timestamp>/` (ou `--output-dir`), par
  Ceci est la commande `iroha app da get` (`--manifest-id`), maintenant
  для последующего orchestrator fetch. C'est un opérateur de robots
  manifeste spool répertoires et garanties, que le récupérateur peut-il utiliser
  подписанные artefacts Torii. Le client JavaScript Torii est disponible ici
  `ToriiClient.getDaManifest(storageTicketHex)`, décodage pour Norito
  octets, manifeste JSON et plan de blocs, les appelants du SDK peuvent être téléchargés
  orchestrator сессии без CLI. Swift SDK offre une préconfiguration pour vos fonctionnalités avancées
  (`ToriiClient.getDaManifestBundle(...)` et `fetchDaPayloadViaGateway(...)`),proposer un bundle dans l'orchestrateur SoraFS du wrapper natif, pour les clients iOS
  télécharger des manifestes, effectuer une récupération multi-source et récupérer des documents sans
  вызова CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` vous permet de déterminer le loyer et le paiement incitatif pour
  заданного размера stockage et окна rétention. L'assistant s'active
  `DaRentPolicyV1` (JSON ou Norito octets) par défaut, validé
  Politique et gestion des fichiers JSON (`gib`, `months`, métadonnées de politique et pour
  `DaRentQuote`), les auditeurs peuvent déterminer les frais XOR dans les protocoles
  управления без ad hoc скриптов. La commande s'occupe de l'hygiène
  `rent_quote ...` avant la charge utile JSON pour les logos de fichiers pour les exercices.
  Remplacez `--quote-out artifacts/da/rent_quotes/<stamp>.json` par
  `--policy-label "governance ticket #..."`, pour récupérer les artefacts
  il s'agit simplement de l'ensemble ou du bundle de configuration ; CLI обрезает пользовательскую
  permet d'éviter les coups durs, comme `policy_source`, qui sont destinés à
  дашбORDов. См. `crates/iroha_cli/src/commands/da.rs` pour les modules et
  `docs/source/da/rent_policy.md` pour le régime politique.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` vous propose : un ticket de stockage,скачивает канонический manifest bundle, запускает multi-source orchestrator
  (`iroha app sorafs fetch`) pour le capteur `--gateway-provider`,
  charge utile complète + tableau de bord dans `artifacts/da/prove_availability_<timestamp>/`,
  et сразу вызывает существующий PoR helper (`iroha app da prove`) avec les principales
  octets. Les opérateurs peuvent configurer les boutons de l'orchestrateur (`--max-peers`,
  `--scoreboard-out`, remplacements de point de terminaison du manifeste) et échantillonneur de preuve
  (`--sample-count`, `--leaf-index`, `--sample-seed`), pour cette commande
  выпускает artefacts, требуемые аудитами DA-5/DA-9 : copie de la charge utile, documentation
  tableau de bord et preuve JSON-резюме.