---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/da/ingest-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3cdf19adcf88eb75e8a66b62251aa5e9538d2b5bae061d960e753413e0927959
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/da/ingest_plan.md`. שמרו על שתי הגרסאות
מסונכרנות עד שהמסמכים הישנים יפרשו.
:::

# תוכנית ingest ל-Data Availability של Sora Nexus

_טיוטה: 2026-02-20 - בעלים: Core Protocol WG / Storage Team / DA WG_

Workstream DA-2 מרחיב את Torii עם API ingest ל-blobs שמפיק מטא-דטה Norito
ומזניק שכפול SoraFS. המסמך מתעד את הסכמה המוצעת, משטח ה-API וזרימת האימות
כדי שהמימוש יתקדם בלי להיתקע על סימולציות שנותרו (מעקבי DA-1). כל פורמטי
payload חייבים להשתמש בקודקים Norito; אין לאפשר fallback ל-serde/JSON.

## יעדים

- לקבל blobs גדולים (סגמנטים של Taikai, sidecars של lane, וארטיפקטי ממשל)
  באופן דטרמיניסטי דרך Torii.
- להפיק manifests Norito קנוניים שמתארים את ה-blob, פרמטרי codec,
  פרופיל erasure ומדיניות retention.
- לשמר מטא-דטה של chunks באחסון hot של SoraFS ולהכניס עבודות שכפול לתור.
- לפרסם כוונות pin + תגיות מדיניות לרג'יסטרי של SoraFS ולמשקיפי ממשל.
- לחשוף receipts של קבלה כדי שלקוחות ישיגו הוכחה דטרמיניסטית לפרסום.

## משטח API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

ה-payload הוא `DaIngestRequest` מקודד Norito. התשובות משתמשות
`application/norito+v1` ומחזירות `DaIngestReceipt`.

| תשובה | משמעות |
| --- | --- |
| 202 Accepted | ה-blob נכנס לתור chunking/replication; הוחזר receipt. |
| 400 Bad Request | הפרת schema/גודל (ראו בדיקות אימות). |
| 401 Unauthorized | טוקן API חסר/לא תקין. |
| 409 Conflict | כפילות `client_blob_id` עם מטא-דטה שאינה תואמת. |
| 413 Payload Too Large | חורג ממגבלת אורך ה-blob המוגדרת. |
| 429 Too Many Requests | הושג rate limit. |
| 500 Internal Error | כשל לא צפוי (log + alert). |

## סכמה Norito מוצעת

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

> הערת מימוש: הייצוגים הקנוניים ב-Rust עבור payloads אלו נמצאים כעת תחת
> `iroha_data_model::da::types`, עם wrappers לבקשה/receipt ב-
> `iroha_data_model::da::ingest` ומבנה manifest ב-
> `iroha_data_model::da::manifest`.

השדה `compression` מציין כיצד callers הכינו את ה-payload. Torii מקבל
`identity`, `gzip`, `deflate`, ו-`zstd`, עם דה-קומפרסיה לפני hashing, chunking
ובדיקת manifests אופציונליים.

### רשימת בדיקות אימות

1. לוודא שכותרת Norito של הבקשה תואמת `DaIngestRequest`.
2. להכשיל אם `total_size` שונה מאורך ה-payload הקנוני (לאחר דה-קומפרסיה) או
   חורג מהמקסימום המוגדר.
3. לאכוף יישור `chunk_size` (חזקת שתיים, <= 2 MiB).
4. לוודא `data_shards + parity_shards` <= מקסימום גלובלי ו-parity >= 2.
5. `retention_policy.required_replica_count` חייב לכבד baseline של ממשל.
6. אימות חתימה מול hash קנוני (ללא שדה החתימה).
7. לדחות `client_blob_id` כפול אלא אם hash ה-payload והמטא-דטה זהים.
8. כאשר מסופק `norito_manifest`, לבדוק ש-schema + hash תואמים ל-manifest
   המחושב מחדש לאחר chunking; אחרת הצומת מייצר manifest ושומר אותו.
9. לאכוף את מדיניות השכפול המוגדרת: Torii כותב מחדש את `RetentionPolicy`
   שנשלח עם `torii.da_ingest.replication_policy` (ראו `replication-policy.md`)
   ודוחה manifests מוכנים מראש שהמטא-דטה של retention בהם לא תואמת את הפרופיל
   הנאכף.

### זרימת chunking ושכפול

1. לחתוך את ה-payload ל-`chunk_size`, לחשב BLAKE3 לכל chunk + שורש Merkle.
2. לבנות Norito `DaManifestV1` (struct חדשה) שתופסת התחייבויות chunk
   (role/group_id), פריסת erasure (ספירות parity לשורות ולעמודות +
   `ipa_commitment`), מדיניות retention והמטא-דטה.
3. להכניס את bytes של ה-manifest הקנוני לתחתית
   `config.da_ingest.manifest_store_dir` (Torii כותב קבצי `manifest.encoded`
   לפי lane/epoch/sequence/ticket/fingerprint) כדי שאורקסטרציית SoraFS תבלע
   אותם ותקשור את ה-storage ticket לנתונים המתמידים.
4. לפרסם כוונות pin באמצעות `sorafs_car::PinIntent` עם תג ממשל ומדיניות.
5. לשדר אירוע Norito `DaIngestPublished` כדי להודיע למשקיפים (לקוחות קלים,
   ממשל, אנליטיקה).
6. להחזיר `DaIngestReceipt` ל-caller (חתום במפתח שירות DA של Torii) ולשלוח את
   הכותרת `Sora-PDP-Commitment` כדי שה-SDKs יקליטו מיד את ה-commitment המקודד.
   ה-receipt כולל כעת `rent_quote` (Norito `DaRentQuote`) ו-`stripe_layout`,
   כך שמגישים יכולים להציג את שכר הדירה הבסיסי, חלק הרזרבה, ציפיות בונוס
   PDP/PoTR ופריסת erasure דו-ממדית לצד ה-storage ticket לפני התחייבות לכספים.

## עדכוני Storage / Registry

- להרחיב את `sorafs_manifest` עם `DaManifestV1`, מה שמאפשר parsing דטרמיניסטי.
- להוסיף stream registry חדש `da.pin_intent` עם payload בגרסה שמצביע על hash
  manifest + ticket id.
- לעדכן את pipelines של observability למעקב אחרי לטנטיות ingest, throughput
  של chunking, backlog שכפול ומניין כשלים.

## אסטרטגיית בדיקות

- בדיקות יחידה לאימות schema, בדיקות חתימה וזיהוי כפילויות.
- בדיקות golden לאימות Norito encoding של `DaIngestRequest`, manifest ו-receipt.
- Harness אינטגרציה שמרים SoraFS + registry מדומים ומאמת זרימות chunk + pin.
- בדיקות property עבור פרופילי erasure וצירופי retention אקראיים.
- Fuzzing של payloads Norito להגנה מפני מטא-דטה פגומה.

## כלי CLI ו-SDK (DA-8)

- `iroha app da submit` (כניסת CLI חדשה) עוטף כעת את builder/publisher המשותף של
  ingest כך שמפעילים יכולים להגיש blobs שרירותיים מחוץ לזרימת Taikai bundle.
  הפקודה נמצאת ב-`crates/iroha_cli/src/commands/da.rs:1` וצורכת payload,
  פרופיל erasure/retention וקבצי metadata/manifest אופציונליים לפני חתימה על
  `DaIngestRequest` הקנוני עם מפתח ה-config של ה-CLI. ריצות מוצלחות שומרות
  `da_request.{norito,json}` ו-`da_receipt.{norito,json}` תחת
  `artifacts/da/submission_<timestamp>/` (override via `--artifact-dir`) כך
  ש-artefacts של שחרור מתעדים את bytes Norito המדויקים ששימשו ב-ingest.
- הפקודה ברירת מחדל `client_blob_id = blake3(payload)` אבל מקבלת overrides
  דרך `--client-blob-id`, מכבדת מפות metadata JSON (`--metadata-json`) ו-manifests
  שהוכנו מראש (`--manifest`), ותומכת ב-`--no-submit` להכנה offline וגם
  `--endpoint` למארחי Torii מותאמים. ה-receipt JSON מודפס ל-stdout בנוסף
  לכתיבה לדיסק, סוגר את דרישת tooling "submit_blob" של DA-8 ומאפשר עבודת
  פריטי SDK.
- `iroha app da get` מוסיף alias ממוקד DA לאורקסטרטור multi-source שמפעיל כבר את
  `iroha app sorafs fetch`. מפעילים יכולים לכוון אותו ל-artefacts של manifest +
  chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **או** להעביר storage
  ticket של Torii דרך `--storage-ticket`. כאשר משתמשים במסלול ticket, ה-CLI
  מוריד manifest מ-`/v2/da/manifests/<ticket>`, שומר את החבילה תחת
  `artifacts/da/fetch_<timestamp>/` (override עם `--manifest-cache-dir`), מפיק את
  hash ה-blob עבור `--manifest-id`, ואז מריץ את האורקסטרטור עם רשימת
  `--gateway-provider` שסופקה. כל ה-knobs המתקדמים מה-fetcher של SoraFS נשארים
  intact (manifest envelopes, labels של לקוח, guard caches, overrides של transport
  אנונימי, export של scoreboard ונתיבי `--output`), וה-manifest endpoint ניתן
  להחלפה באמצעות `--manifest-endpoint` עבור מארחי Torii מותאמים, כך שבדיקות
  availability מקצה לקצה חיות כולן תחת namespace `da` בלי לשכפל לוגיקת
  orchestrator.
- `iroha app da get-blob` מושך manifests קנוניים ישירות מ-Torii דרך
  `GET /v2/da/manifests/{storage_ticket}`. הפקודה כותבת
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` ו-
  `chunk_plan_{ticket}.json` תחת `artifacts/da/fetch_<timestamp>/` (או
  `--output-dir` שמספק המשתמש) תוך הדפסה של פקודת `iroha app da get` המדויקת
  (כולל `--manifest-id`) הנדרשת למשיכת orchestrator. זה משאיר את המפעילים מחוץ
  לספריות spool של manifest ומבטיח שה-fetcher תמיד משתמש ב-artefacts חתומים
  שנפלטים מ-Torii. לקוח Torii ב-JavaScript משקף את הזרימה דרך
  `ToriiClient.getDaManifest(storageTicketHex)`, ומחזיר bytes Norito מפוענחים,
  manifest JSON ו-chunk plan כדי ש-callers של SDK יוכלו להרים sessions של
  orchestrator בלי להפעיל CLI. ה-SDK של Swift חושף כעת את אותם משטחים
  (`ToriiClient.getDaManifestBundle(...)` וכן `fetchDaPayloadViaGateway(...)`),
  ומנתב bundles ל-wrapper הטבעי של orchestrator SoraFS כך שלקוחות iOS יוכלו
  להוריד manifests, להריץ fetches multi-source, ולאסוף הוכחות ללא CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` מחשב rent דטרמיניסטי ופירוט תמריצים לגודל storage נתון
  וחלון retention. ה-helper צורך את `DaRentPolicyV1` הפעילה (JSON או bytes Norito)
  או את ברירת המחדל המובנית, מאמת את המדיניות ומדפיס סיכום JSON (`gib`,
  `months`, מטא-דטה של מדיניות ושדות `DaRentQuote`) כדי שאודיטורים יוכלו לצטט
  חיובי XOR מדויקים בפרוטוקולי ממשל ללא סקריפטים נקודתיים. הפקודה גם מפיקה
  שורת `rent_quote ...` לפני payload ה-JSON כדי להשאיר logs קריאים בזמן drills.
  חברו `--quote-out artifacts/da/rent_quotes/<stamp>.json` עם
  `--policy-label "governance ticket #..."` כדי לשמור artefacts מסודרים שמצטטים
  את ההצבעה או חבילת ה-config המדויקת; ה-CLI מקצר תוויות מותאמות ודוחה מחרוזות
  ריקות כדי לשמור על `policy_source` שימושי בדשבורדים של אוצר. ראו
  `crates/iroha_cli/src/commands/da.rs` עבור תת-הפקודה ו-
  `docs/source/da/rent_policy.md` עבור סכמה המדיניות.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` קושר הכל יחד: הוא לוקח storage ticket, מוריד
  את bundle ה-manifest הקנוני, מריץ orchestrator multi-source (`iroha app sorafs fetch`)
  מול רשימת `--gateway-provider` שסופקה, שומר את ה-payload שהורד + scoreboard תחת
  `artifacts/da/prove_availability_<timestamp>/`, ומיד מפעיל את helper PoR הקיים
  (`iroha app da prove`) עם ה-bytes שהורדו. מפעילים יכולים לכוונן את knobs של
  orchestrator (`--max-peers`, `--scoreboard-out`, overrides של manifest endpoint)
  ואת sampler ה-proof (`--sample-count`, `--leaf-index`, `--sample-seed`) תוך
  שפועל פקודה אחת שמייצרת את ה-artefacts הנדרשים לביקורות DA-5/DA-9: עותק
  payload, עדות scoreboard וסיכומי proof ב-JSON.
