---
lang: he
direction: rtl
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::הערה מקור קנוניק
Reflete `docs/source/da/ingest_plan.md`. גרסאות גרדז לסינכרון
:::

# Plan d'ingest זמינות נתונים Sora Nexus

_Redige: 2026-02-20 - אחראים: Core Protocol WG / Storage Team / DA WG_

Le workstream DA-2 etend Torii עם אחד API d'ingest de blobs qui emet des
metadonnees Norito et amorce la replication SoraFS. Ce מסמכי לכידת le
schema propose, la surface API et le flux de validation afin que
l'implementation avance sans bloquer sur les simulations restantes (suivi
DA-1). עוד פורמטים של מטען DOIVENT משתמש של קודקים Norito; אוקון
ניתנת הרשאה/JSON קיימת.

## אובייקטים

- Accepter des blobs volumineux (קטעי Taikai, קרונות צד דה ליין, חפצי אמנות
  gouvernance) de maniere deterministe via Torii.
- Produire des manifests Norito canoniques decrivant le blob, les paramets de
  codec, le profil d'erasure et la politique de retention.
- Persister la metadata de chunks dans le stockage hot de SoraFS et mettre en
  file les jobs de replication.
- Publier les intents de pin + tags de politique dans le registry SoraFS et les
  observateurs de governance.
- Exposer des Receipts d'admission pour que les clients retrouvent une preuve
  deterministe de publication.

## Surface API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Leadload est un `DaIngestRequest` קידוד ב-Norito. Les reponses utilisent
`application/norito+v1` et renvoient `DaIngestReceipt`.

| תגובה | סימן |
| --- | --- |
| 202 מקובל | Blob en file pour chunking/שכפול; קבלה renvoye. |
| 400 בקשה רעה | Violation de schema/tail (voir checks de validation). |
| 401 לא מורשה | Token API manquant/invalide. |
| 409 קונפליקט | Doublon `client_blob_id` עם מטא נתונים לא זהים. |
| 413 מטען גדול מדי | Depasse la limite configuree de longueur du blob. |
| 429 יותר מדי בקשות | שימו לב למגבלת תעריף. |
| 500 שגיאה פנימית | Echec inattendu (יומן + התראה). |

## סכימה Norito מציעה

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

> הערה ליישום: les representations Rust canoniques pour ces מטענים
> vivent Maintenant sous `iroha_data_model::da::types`, avec des wrappers
> בקשה/קבלה dans `iroha_data_model::da::ingest` et la structure de
> מניפסט ב-`iroha_data_model::da::manifest`.

Le champ `compression` מודעה הערה les callers ont prepare le payload. Torii
קבל את `identity`, `gzip`, `deflate`, et `zstd`, en decomprimant les bytes avant
le hashing, le chunking ו-la verification des manifests optionnels.

### רשימת אימות1. Verifier que le header Norito de la requete מתאים ל-`DaIngestRequest`.
2. Echouer si `total_size` differe de la longueur canonique du payload
   (decompresse) או depasse את ההגדרה המקסימלית.
3. Forcer l'alignement de `chunk_size` (puissance de deux, <= 2 MiB).
4. מבטח `data_shards + parity_shards` <= שוויון גלובלי מרבי >= 2.
5. `retention_policy.required_replica_count` doit respecter la baseline de
   ממשל.
6. Verification de signature contre le hash canonique (en excluant le champ
   חתימה).
7. Rejeter un `client_blob_id` duplique sauf si le hash du payload et la metadata
   sont identiques.
8. Quand `norito_manifest` est fourni, מאמת que schema + כתב hash
   au Manifeste recalcule apres chunking; sinon le noeud genere le manifest et le
   stocke.
9. התצורה של אפליקציית שכפול פוליטית: Torii reecrit le
   `RetentionPolicy` soumis avec `torii.da_ingest.replication_policy` (voir
   `replication-policy.md`) et rejette les manifests preconstruits dont la
   metadata de retention ne correspond pas au profil impose.

### Flux de chunking ושכפול

1. Decouper le payload en `chunk_size`, מחשבון BLAKE3 par chunk + racine Merkle.
2. Construire Norito `DaManifestV1` (נואבל struct) לכידת התקשרויות
   de chunk (role/group_id), le layout d'erasure (comptes de parite de lignes et
   colonnes plus `ipa_commitment`), la politique de retention et la metadata.
3. Mettre en file les bytes du manifest canonique sous
   `config.da_ingest.manifest_store_dir` (Torii ecrit des fichiers
   `manifest.encoded` אינדקסים לפי מסלול/תקופה/רצף/כרטיס/טביעת אצבע) afin
   que l'orchestration SoraFS les ingere et relie le כרטיס אחסון aux donnees
   מתמיד.
4. Publier les intents de pin דרך `sorafs_car::PinIntent` avec tag de governance
   ופוליטיקה.
5. Emettre l'evenement Norito `DaIngestPublished` pour notifier les observateurs
   (למרות לקוחות, ניהול, אנליטיקה).
6. Renvoyer `DaIngestReceipt` או מתקשר (signe par la cle de service DA de Torii)
   et emettre le header `Sora-PDP-Commitment` pour que les SDKs capturent le
   התחייבות מקודד מיידית. Le קבלה כולל תחזוקה `rent_quote`
   (un Norito `DaRentQuote`) et `stripe_layout`, שולחי עזר קבועים
   d'afficher la rent de base, la reserve, les attentes de bonus PDP/PoTR et le
   layout d'erasure 2D aux cotes du כרטיס אחסון Avant d'engager des fonds.

## מחמיץ מלאי/רישום

- Etendre `sorafs_manifest` avec `DaManifestV1`, ניתוח קבוע
  דטרמיניסטית.
- Ajouter un nouveau stream de registry `da.pin_intent` עם גרסת מטען
  referencant le hash de manifest + מזהה כרטיס.
- Mettre a jour les pipelines d'observabilite pour suivre la latence d'ingest,
  תפוקה של chunking, צבר של שכפול ו-les compteurs
  d'echecs.

## אסטרטגיית בדיקות- בדיקות unitaires pour validation de schema, checks de signature, detection de
  דאבלונים.
- בודק golden verifiant l'encoding Norito de `DaIngestRequest`, manifest et
  קבלה.
- רתמת אינטגרציה של SoraFS + סימולציות רישום, רישומים תקפים
  Flux de Chunk + סיכה.
- Tests de proprietes couvrant les profils d'erasure et combinaisons de
  שימור aleatoires.
- מטענים מטושטשים Norito כדי להגן על המטא נתונים.

## Tooling CLI & SDK (DA-8)- `iroha app da submit` (novel entrypoint CLI) תחזוקת המעטפה le Builder
  d'ingest partage afin que les operateurs puissent ingerer des blobs
  arbitraires hors du flux חבילת Taikai. La commande vit dans
  `crates/iroha_cli/src/commands/da.rs:1` et consomme unload, un profile
  d'erasure/retention et des fichiers optionnels de metadata/manifest avant de
  חותם le `DaIngestRequest` canonique avec la cle de config CLI. לס רץ
  reussis persistent `da_request.{norito,json}` et `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (עקיפה דרך `--artifact-dir`) afin que
  les artefacts de release enregistrent les bytes Norito מדוייק משתמש בתליון
  אני בולע.
- La commande use par defaut `client_blob_id = blake3(payload)` mais accepte
  des overrides דרך `--client-blob-id`, כבוד les maps JSON de metadata
  (`--metadata-json`) et les manifests pre-generes (`--manifest`), et supporte
  `--no-submit` pour la הכנה לא מקוון בתוספת `--endpoint` pour des hosts
  Torii מתפרסמת. קבלה JSON est imprime sur stdout en plus d'etre
  ecrit sur disque, כלי עבודה מתמשך "submit_blob" de DA-8 et
  debloquant le travail de parite SDK.
- `iroha app da get` ajoute un alias DA pour l'orchestrateur multi-source qui alimente
  deja `iroha app sorafs fetch`. Les Operaurs peuvent le pointer vers des artefacts
  מניפסט + תוכנית נתח (`--manifest`, `--plan`, `--manifest-id`) **ou** fournir
  un כרטיס אחסון Torii דרך `--storage-ticket`. כרטיס Quand le chemin est
  utilise, la CLI recupere le manifest depuis `/v2/da/manifests/<ticket>`,
  persiste le bundle sous `artifacts/da/fetch_<timestamp>/` (עקוף avec
  `--manifest-cache-dir`), נגזר את ה-hash du blob pour `--manifest-id`, puis
  execute l'orchestrateur avec la list `--gateway-provider` fournie. Tous les
  ידיות avances du fetcher SoraFS שלמים (מעטפות גלויות, תוויות
  לקוח, שומר מטמון, עוקף את הובלה אנונימית, לוח תוצאות ייצוא וכו'
  נתיבים `--output`), et l'endpoint manifest peut etre היטל באמצעות
  `--manifest-endpoint` pour des hosts Torii פרסונליות, דונק לבדיקות
  זמינות מקצה לקצה vivent entierement sous le namespace `da` sans
  dupliquer la logique d'orchestrateur.
- `iroha app da get-blob` recupere les manifests canoniques directement depuis Torii
  דרך `GET /v2/da/manifests/{storage_ticket}`. La commande ecrit
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` et
  `chunk_plan_{ticket}.json` sous `artifacts/da/fetch_<timestamp>/` (ou un
  `--output-dir` fourni par l'utilisateur) tout en affichant la commande exacte
  `iroha app da get` (כולל `--manifest-id`) דורש pour le fetch orchestrateur.
  Cela garde les Operaurs hors des repertoires spool de manifests et garantit
  que le fetcher use toujours les artefacts signes emis par Torii. ללקוח
  Torii JavaScript reproduit ce flux via
  `ToriiClient.getDaManifest(storageTicketHex)`, renvoyant les bytes Norito
  מפענח, למניפסט JSON ותוכנית נתח אפינ que les callers SDK hydratent
  des sessions d'orchestrateur sans passer par la CLI. חשיפה של Le SDK Swift
  תחזוקה של משטחי les memes (`ToriiClient.getDaManifestBundle(...)` plus`fetchDaPayloadViaGateway(...)`), branchant les bundles sur le wrapper natif
  d'orchestraateur SoraFS pour que les clients iOS puissent telecharger des
  manifests, executer des מביאה ריבוי מקורות ו-capturer des preuves sans
  invoquer la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcule des rentas deterministes et la ventilation des
  תמריצים לשפוך une taille de storage et une fenetre de retention fournies.
  L'helper consomme la `DaRentPolicyV1` active (JSON ou bytes Norito) ou le
  ברירת המחדל של אינטגרה, תקף לפוליטיקה ושמירה על קורות חיים של JSON (`gib`,
  `months`, metadata de politique, champs `DaRentQuote`) afin que les auditeurs
  citent les חיובים XOR מחייב בדקות הממשל ללא תסריטים
  hoc. La commande emet aussi un resume en une ligne `rent_quote ...` avant le
  מטען JSON pour garder les logs consoles lisibles תליון les drills
  ד'תקרית. Associez `--quote-out artifacts/da/rent_quotes/<stamp>.json` avec
  `--policy-label "governance ticket #..."` pour persister des artefacts soignes
  citant le vote ou bundle de config exact; la CLI tronque le label personalise
  et refuse les chaines vides afin que les valeurs `policy_source` restent
  actionnables dans les לוחות המחוונים של tresorerie. Voir
  `crates/iroha_cli/src/commands/da.rs` pour le sous-commande et
  `docs/source/da/rent_policy.md` pour le schema de politique.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- שרשרת `iroha app da prove-availability` קודמת: il prend un storage
  כרטיס, העברת חבילה קנונית דה מניפסט, ביצוע ל'תזמורת
  ריבוי מקורות (`iroha app sorafs fetch`) לעומת הרשימה `--gateway-provider`
  fournie, persiste le payload telecharge + לוח תוצאות sous
  `artifacts/da/prove_availability_<timestamp>/`, et invoque immediatement le
  עוזר PoR קיים (`iroha app da prove`) עם התאוששות בתים. Les Operators
  peuvent ajuster les knobs de l'orchestraateur (`--max-peers`, `--scoreboard-out`,
  עוקף מניפסט d'endpoint) et le sampler de proof (`--sample-count`,
  `--leaf-index`, `--sample-seed`) tandis qu'une seule commande produit les
  חפצי אמנות משתתפים בביקורת DA-5/DA-9: עותק מטען, ראיות דה
  לוח תוצאות וקורות חיים של JSON.