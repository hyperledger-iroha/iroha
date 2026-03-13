---
lang: he
direction: rtl
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T15:38:30.661072+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus תוכנית הטמעת זמינות נתונים

_נוסח: 2026-02-20 - בעלים: Core Protocol WG / Storage Team / DA WG_

זרם העבודה של DA-2 מרחיב את Torii עם ממשק API של בולמי שפולט Norito
שכפול מטא נתונים וזרעים SoraFS. מסמך זה לוכד את המוצע
סכימה, משטח API וזרימת אימות כך שהיישום יכול להמשיך בלי
חסימה על סימולציות מצטיינות (מעקבי DA-1). כל פורמטי המטען חייבים
השתמש ב-Codec Norito; אין נפילות של serde/JSON מותרות.

## יעדים

- קבל כתמים גדולים (מקטעי טאיקאי, קרונות צד, חפצי שלטון)
  באופן דטרמיניסטי מעל Torii.
- הפקת מניפסטים קנוניים של Norito המתארים את הכתם, פרמטרי ה-codec,
  פרופיל מחיקה ומדיניות שמירה.
- שמירה על מטא נתונים של נתחים ב-SoraFS אחסון חם ועבודות שכפול של תור.
- פרסם כוונות סיכה + תגי מדיניות לרישום וממשל SoraFS
  משקיפים.
- חשפו את קבלות הקבלה כדי שהלקוחות יקבלו חזרה הוכחה דטרמיניסטית לפרסום.

## משטח API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

המטען הוא Norito מקודד `DaIngestRequest`. שימוש בתגובות
`application/norito+v1` והחזרה `DaIngestReceipt`.

| תגובה | המשמעות |
| --- | --- |
| 202 מקובל | כתם תור לנתחים/שכפול; הוחזרה קבלה. |
| 400 בקשה רעה | הפרת סכימה/גודל (ראה בדיקות אימות). |
| 401 לא מורשה | אסימון API חסר/לא חוקי. |
| 409 קונפליקט | שכפול `client_blob_id` עם מטא נתונים לא תואמים. |
| 413 מטען גדול מדי | חורג ממגבלת אורך הכתם המוגדרת. |
| 429 יותר מדי בקשות | פגיעה במגבלת שיעור. |
| 500 שגיאה פנימית | כשל בלתי צפוי (נרשם + התראה). |

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

מחזירה גרסה `DaProofPolicyBundle` הנגזרת מקטלוג הנתיבים הנוכחי.
החבילה מפרסמת את `version` (כיום `1`), `policy_hash` (hash של
רשימת המדיניות המוזמנת), וערכים `policies` הנושאים `lane_id`, `dataspace_id`,
`alias`, וה-`proof_scheme` האכיפה (`merkle_sha256` כיום; נתיבי KZG הם
נדחה על ידי בליעה עד שההתחייבויות של KZG יהיו זמינות). כותרת החסימה עכשיו
מתחייב לחבילה דרך `da_proof_policies_hash`, כך שלקוחות יכולים להצמיד את
מדיניות פעילה שנקבעה בעת אימות התחייבויות או הוכחות של DA. תביא את נקודת הקצה הזו
לפני בניית הוכחות כדי להבטיח שהן תואמות את מדיניות הנתיב והזרם
חבילת hash. רשימת התחייבויות/נקודות קצה להוכחה נושאות את אותו חבילה כך ש-SDKs
לא צריך נסיעה נוספת הלוך ושוב כדי לאגד הוכחה למערכת המדיניות הפעילה.

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

מחזירה `DaProofPolicyBundle` הנושא את רשימת הפוליסות שהוזמנה בתוספת א
`policy_hash` כך ש-SDKs יכולים להצמיד את הגרסה ששימשה כאשר הופק בלוק. ה
Hash מחושב על פני מערך המדיניות המקודד Norito ומשתנה בכל פעם
`proof_scheme` של ליין מעודכן, ומאפשר ללקוחות לזהות סחיפה בין
הוכחות במטמון ותצורת השרשרת.

## סכימת Norito מוצעת

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
```> הערת יישום: ייצוגי החלודה הקנוניים עבור מטענים אלה חיים כעת
> `iroha_data_model::da::types`, עם עטיפות בקשה/קבלה ב-`iroha_data_model::da::ingest`
> ומבנה המניפסט ב-`iroha_data_model::da::manifest`.

השדה `compression` מפרסם כיצד המתקשרים הכינו את המטען. Torii מקבל
`identity`, `gzip`, `deflate`, ו-`zstd`, משחררים בשקיפות את הבתים לפני
hashing, chunking ואימות מניפסטים אופציונליים.

### רשימת אימות

1. בדוק את הבקשה Norito תואמת לכותרת `DaIngestRequest`.
2. נכשל אם `total_size` שונה מאורך המטען הקנוני (הלא דחוס) או חורג מהאורך המקסימלי שהוגדר.
3. אכוף יישור `chunk_size` (עוצמה של שניים, = 2.
5. על `retention_policy.required_replica_count` לכבד את קו הממשל הבסיסי.
6. אימות חתימה מול hash קנוני (לא כולל שדה חתימה).
.
8. כאשר `norito_manifest` מסופק, אמת את הסכימה + התאמות גיבוב מחושבות מחדש
   להתבטא לאחר חתיכה; אחרת הצומת יוצר מניפסט ומאחסן אותו.
9. אכוף את מדיניות השכפול שהוגדרה: Torii משכתב את ההודעה שנשלחה
   `RetentionPolicy` עם `torii.da_ingest.replication_policy` (ראה
   `replication_policy.md`) ודוחה מניפסטים שנבנו מראש שהשמירה שלהם
   מטא נתונים אינם תואמים לפרופיל האכיפה.

### זרימת נתחים ושכפול1. נתח מטען לתוך `chunk_size`, מחשב BLAKE3 לכל נתח + שורש מרקל.
2. בניית Norito `DaManifestV1` (מבנה חדש) לכידת התחייבויות נתח (role/group_id),
   פריסת מחיקה (ספירת שוויון שורות ועמודות בתוספת `ipa_commitment`), מדיניות שמירה,
   ומטא נתונים.
3. תור את הבייטים של המניפסט הקנוני תחת `config.da_ingest.manifest_store_dir`
   (Torii כותב קבצי `manifest.encoded` מקודדים לפי נתיב/תקופה/רצף/כרטיס/טביעת אצבע) אז SoraFS
   תזמור יכול להטמיע אותם ולקשר את כרטיס האחסון לנתונים מתמידים.
4. פרסם כוונות סיכה באמצעות `sorafs_car::PinIntent` עם תג ניהול + מדיניות.
5. שלח אירוע Norito `DaIngestPublished` כדי להודיע למשקיפים (לקוחות קלים,
   ממשל, ניתוח).
6. החזר את `DaIngestReceipt` (חתום על ידי מפתח השירות Torii DA) והוסף את
   כותרת התגובה `Sora-PDP-Commitment` המכילה את הקידוד base64 Norito
   של ההתחייבות הנגזרת כך ש-SDKs יוכלו לאחסן את זרעי הדגימה באופן מיידי.
   הקבלה מטביעה כעת `rent_quote` (a `DaRentQuote`) ו-`stripe_layout`
   כך שהמגישים יוכלו להציג את התחייבויות XOR, נתח מילואים, ציפיות בונוס PDP/PoTR,
   וממדי מטריצת המחיקה הדו-ממדית לצד המטא נתונים של כרטיסי האחסון לפני התחייבות כספים.
7. מטא נתונים אופציונליים של הרישום:
   - `da.registry.alias` - מחרוזת כינוי ציבורית, לא מוצפנת של UTF-8 כדי לחלץ את ערך רישום ה-PIN.
   - `da.registry.owner` - מחרוזת `AccountId` ציבורית, לא מוצפנת לתיעוד בעלות על הרישום.
   Torii מעתיק אותם ל-`DaPinIntent` שנוצר כך שעיבוד פינים במורד הזרם יכול לאגד כינויים
   ובעלים מבלי לנתח מחדש את מפת המטא-נתונים הגולמית; ערכים שגויים או ריקים נדחים במהלך
   בליעת אימות.

## עדכוני אחסון / רישום

- הרחב את `sorafs_manifest` עם `DaManifestV1`, המאפשר ניתוח דטרמיניסטי.
- הוסף זרם רישום חדש `da.pin_intent` עם התייחסות לטעינה מנוסחת
  מניפסט hash + מזהה כרטיס.
- עדכן צינורות של צפיות כדי לעקוב אחר זמן השהייה של הטמעה, נתח תפוקה,
  צבר שכפול וספירת כשלים.
- תגובות Torii `/status` כוללות כעת מערך `taikai_ingest` שמציג את הגרסה העדכנית ביותר
  זמן אחזור מקודד לכניסה, סחיפה של קצה חי ומדדי שגיאות לכל (אשכול, זרם), המאפשרים DA-9
  לוחות מחוונים להטמעת צילומי מצב בריאות ישירות מצמתים מבלי לגרד את Prometheus.

## אסטרטגיית בדיקה- בדיקות יחידות לאימות סכימה, בדיקות חתימה, זיהוי כפילויות.
- בדיקות זהב המאמתות קידוד Norito של `DaIngestRequest`, מניפסט וקבלה.
- רתמת אינטגרציה מסתובבת מדומה SoraFS + רישום, ומבטיחה זרימות של נתחים + סיכות.
- בדיקות נכס המכסות פרופילי מחיקה אקראיים ושילובי שמירה.
- טשטוש של מטענים Norito כדי להגן מפני מטא נתונים שגויים.
- אביזרי זהב לכל מעמד כתמים חיים מתחת
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` עם נתח נלווה
  רישום ב-`fixtures/da/ingest/sample_chunk_records.txt`. המבחן שהתעלם ממנו
  `regenerate_da_ingest_fixtures` מרענן את המתקנים, תוך כדי
  `manifest_fixtures_cover_all_blob_classes` נכשל ברגע שמתווסף גרסה חדשה של `BlobClass`
  מבלי לעדכן את חבילת Norito/JSON. זה שומר על Torii, SDKs ומסמכים כנים בכל פעם DA-2
  מקבל משטח כתם חדש.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI & SDK Tooling (DA-8)- `iroha app da submit` (נקודת כניסת CLI חדשה) עוטפת כעת את בונה/מוציא לאור המשותף להטמעה כך שהמפעילים
  יכול לבלוע כתמים שרירותיים מחוץ לזרימת צרור Taikai. הפקודה גרה בפנים
  `crates/iroha_cli/src/commands/da.rs:1` וצורכת מטען, פרופיל מחיקה/שמירה, ו
  קובצי מטא נתונים/מניפסט אופציונליים לפני החתימה על `DaIngestRequest` הקנוני עם ה-CLI
  מפתח תצורה. ריצות מוצלחות נמשכות `da_request.{norito,json}` ו-`da_receipt.{norito,json}` תחת
  `artifacts/da/submission_<timestamp>/` (עקוף באמצעות `--artifact-dir`) כדי לשחרר חפצי אמנות יכולים
  רשום את ה-Norito המדויקים בשימוש במהלך הבליעה.
- ברירת המחדל של הפקודה היא `client_blob_id = blake3(payload)` אך מקבלת דרישות באמצעות
  `--client-blob-id`, מכבד מפות JSON עם מטא נתונים (`--metadata-json`) ומניפסטים שנוצרו מראש
  (`--manifest`), ותומך ב-`--no-submit` להכנה לא מקוונת בתוספת `--endpoint` להתאמה אישית
  Torii מארחים. קבלה JSON מודפסת ל-stdout בנוסף לכתיבה לדיסק, מה שסוגר את
  דרישת כלי DA-8 "submit_blob" וביטול חסימת עבודת זוגיות SDK.
- `iroha app da get` מוסיף כינוי ממוקד DA עבור מתזמר ריבוי מקורות שכבר מפעיל
  `iroha app sorafs fetch`. מפעילים יכולים להפנות אותו אל חפצי אמנות של מניפסט + תוכנית נתח (`--manifest`,
  `--plan`, `--manifest-id`) **או** פשוט להעביר כרטיס אחסון Torii דרך `--storage-ticket`. כאשר ה
  נעשה שימוש בנתיב הכרטיס, ה-CLI מושך את המניפסט מ-`/v2/da/manifests/<ticket>`, ממשיך את החבילה
  תחת `artifacts/da/fetch_<timestamp>/` (עקוף עם `--manifest-cache-dir`), נגזרת **המניפסט
  hash** עבור `--manifest-id`, ולאחר מכן מפעיל את התזמר עם ה-`--gateway-provider` שסופק
  רשימה. אימות מטען עדיין מסתמך על תקציר CAR/`blob_hash` המוטבע בזמן שמזהה השער הוא
  עכשיו ה-hash של המניפסט כך שלקוחות ומאמתים חולקים מזהה גוש יחיד. כל הכפתורים המתקדמים מ
  משטח המשאב SoraFS שלם (מעטפות גלויות, תוויות לקוחות, מטמוני שמירה, הובלת אנונימיות
  עקיפות, ייצוא לוח תוצאות ונתיבי `--output`), וניתן לעקוף את נקודת הקצה המניפסט באמצעות
  `--manifest-endpoint` עבור מארחי Torii מותאמים אישית, כך שבדיקות זמינות מקצה לקצה פועלות לחלוטין תחת
  `da` מרחב שמות ללא שכפול לוגיקה של מתזמר.
- `iroha app da get-blob` מושך מניפסטים קנוניים הישר מ-Torii דרך `GET /v2/da/manifests/{storage_ticket}`.
  הפקודה מסמנת כעת חפצים עם ה-hash המניפסט (מזהה blob), כתיבה
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` ו-`chunk_plan_{manifest_hash}.json`
  תחת `artifacts/da/fetch_<timestamp>/` (או `--output-dir` שסופק על ידי המשתמש) תוך הדהוד מדויק
  דרושה הפעלת `iroha app da get` (כולל `--manifest-id`) עבור אחזור התזמר ההמשך.
  זה מרחיק את המפעילים מספריות הסליל של המניפסט ומבטיחה שהמשחזר תמיד משתמש ב-
  חפצי אמנות חתומים שנפלטו על ידי Torii. לקוח JavaScript Torii משקף זרימה זו דרך
  `ToriiClient.getDaManifest(storageTicketHex)` בעוד SDK SDK חושפת כעת
  `ToriiClient.getDaManifestBundle(...)`. שניהם מחזירים את ה-Norito בתים המפוענחים, JSON מניפסט, Hash של מניפסט,ותוכנית נתחים כך שמתקשרי SDK יוכלו להרטיב הפעלות מתזמר מבלי להפגיז ל-CLI ול-Swift
  לקוחות יכולים בנוסף להתקשר ל-`fetchDaPayloadViaGateway(...)` כדי להעביר את החבילות האלה דרך המקור
  SoraFS מעטפת מתזמר.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- תגובות `/v2/da/manifests` מופיעות כעת `manifest_hash`, ושני עוזרי CLI + SDK (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway`, ועטיפות השער של Swift/JS) מתייחסים לעיכול הזה כאל
  מזהה מניפסט קנוני תוך כדי המשך אימות מטענים מול CAR/blob hash המוטבע.
- `iroha app da rent-quote` מחשבת שכר דירה דטרמיניסטי ופירוק תמריצים עבור גודל אחסון שסופק
  וחלון שמירה. העוזר צורך את `DaRentPolicyV1` הפעילים (JSON או Norito בתים) או
  ברירת המחדל המובנית, מאמתת את המדיניות ומדפיסה סיכום JSON (`gib`, `months`, מטא נתונים של מדיניות,
  ושדות `DaRentQuote`) כך שמבקרים יכולים לצטט חיובי XOR מדויקים בתוך דקות ממשל ללא
  כתיבת תסריטים אד הוק. הפקודה פולטת כעת גם סיכום `rent_quote ...` בשורה אחת לפני ה-JSON
  מטען כדי להקל על סריקה של יומני קונסולה וספרי ריצה כאשר מופקים ציטוטים במהלך תקריות.
  מעבר `--quote-out artifacts/da/rent_quotes/<stamp>.json` (או כל נתיב אחר)
  כדי להתמיד בסיכום המודפס יפה ולהשתמש ב-`--policy-label "governance ticket #..."` כאשר
  חפץ צריך לצטט חבילת הצבעה/תצורה ספציפית; ה-CLI חותך תוויות מותאמות אישית ודוחה ריקים
  מחרוזות כדי לשמור על ערכי `policy_source` משמעותיים בחבילות ראיות. ראה
  `crates/iroha_cli/src/commands/da.rs` עבור תת-פקודה ו-`docs/source/da/rent_policy.md`
  עבור סכימת המדיניות.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- שוויון רישום פינים מתרחב כעת ל-SDKs: `ToriiClient.registerSorafsPinManifest(...)` ב-
  JavaScript SDK בונה את המטען המדויק שבו נעשה שימוש על ידי `iroha app sorafs pin register`, אוכפת קנונית
  מטא נתונים של chunker, מדיניות סיכות, הוכחות כינוי ותקצירים ממשיכים לפני פרסום ב-
  `/v2/sorafs/pin/register`. זה מונע מבוטי CI ואוטומציה להפגיז ל-CLI כאשר
  מקליט רישומי מניפסט, והעוזר מגיע עם כיסוי TypeScript/README כך של DA-8
  שוויון הכלים "שלח/קבל/הוכח" מתקיים במלואו ב-JS לצד Rust/Swift.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` משרשרת את כל האמור לעיל: זה לוקח כרטיס אחסון, מוריד את
  חבילת מניפסט קנונית, מריץ את מתזמר ריבוי מקורות (`iroha app sorafs fetch`) כנגד
  רשימת `--gateway-provider` שסופקה, ממשיכה את המטען שהורד + לוח התוצאות תחת
  `artifacts/da/prove_availability_<timestamp>/`, ומיד מפעיל את עוזר ה-PoR הקיים
  (`iroha app da prove`) באמצעות הבתים שנלקחו. מפעילים יכולים לכוונן את כפתורי התזמר
  (`--max-peers`, `--scoreboard-out`, עקיפות נקודות קצה מניפסט) ודגימת ההוכחה
  (`--sample-count`, `--leaf-index`, `--sample-seed`) בעוד שפקודה יחידה מייצרת את החפצים
  צפוי על ידי ביקורת DA-5/DA-9: עותק מטען, ראיות לוח תוצאות וסיכומי הוכחה JSON.- `da_reconstruct` (חדש ב-DA-6) קורא מניפסט קנוני בתוספת ספריית ה-chunk שנפלט מה-chunk
  חנות (פריסה `chunk_{index:05}.bin`) ומרכיבה מחדש באופן דטרמיניסטי את המטען תוך אימות
  כל התחייבות של Blake3. ה-CLI חי תחת `crates/sorafs_car/src/bin/da_reconstruct.rs` ונשלח כמו
  חלק מחבילת הכלים SoraFS. זרימה אופיינית:
  1. `iroha app da get-blob --storage-ticket <ticket>` כדי להוריד את `manifest_<manifest_hash>.norito` ואת תוכנית הנתחים.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (או `iroha app da prove-availability`, אשר כותב את חפצי האחזור תחת
     `artifacts/da/prove_availability_<ts>/` ומחזיק קבצים לכל נתח בתוך ספריית `chunks/`).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  מתקן רגרסיה חי תחת `fixtures/da/reconstruct/rs_parity_v1/` ולוכד את המניפסט המלא
  ומטריצת נתחים (נתונים + זוגיות) בשימוש על ידי `tests::reconstructs_fixture_with_parity_chunks`. תחדש את זה עם

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  המתקן פולט:

  - `manifest.{norito.hex,json}` — קידודי `DaManifestV1` קנוניים.
  - `chunk_matrix.json` - סדרות אינדקס/היסט/אורך/תקציר/זוגיות עבור הפניות למסמכים/בדיקות.
  - `chunks/` — `chunk_{index:05}.bin` פרוסות מטען הן עבור נתונים והן עבור רסיסי זוגיות.
  - `payload.bin` - מטען דטרמיניסטי המשמש את מבחן הרתמה המודע לזוגיות.
  - `commitment_bundle.{json,norito.hex}` - מדגם `DaCommitmentBundle` עם מחויבות KZG דטרמיניסטית עבור מסמכים/בדיקות.

  הרתמה מסרבת לגושים חסרים או קטומים, בודקת את ה-hash הסופי של Blake3 מול `blob_hash`,
  ופולט כתם JSON סיכום (בתים של מטען, ספירת נתחים, כרטיס אחסון) כדי ש-CI יוכל לטעון שחזור
  ראיות. זה סוגר את דרישת ה-DA-6 לכלי שחזור דטרמיניסטי שמפעילים ו-QA
  משימות יכולות להפעיל ללא חיווט של סקריפטים מותאמים אישית.

## סיכום רזולוציית TODO

כל פעולות הטמעה שנחסמו בעבר יושמו ואומתו:- **רמזי דחיסה** — Torii מקבל תוויות המסופקות על ידי המתקשר (`identity`, `gzip`, `deflate`,
  `zstd`) ומנרמל עומסים לפני אימות כך שה-hash המניפסט הקנוני יתאים ל-
  בתים לא דחוסים.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **הצפנת מטא נתונים לממשל בלבד** — Torii מצפין כעת מטא נתונים של ממשל עם
  מפתח ChaCha20-Poly1305 מוגדר, דוחה תוויות לא תואמות ומציג שניים מפורשים
  ידיות תצורה (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) כדי לשמור על סיבוב דטרמיניסטי.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **זרימת מטען גדול** - קליטה מרובה חלקים בשידור חי. לקוחות זרם דטרמיניסטי
  מעטפות `DaIngestChunk` ממוקמות על ידי `client_blob_id`, Torii מאמתת כל פרוסה, משלבת אותן
  תחת `manifest_store_dir`, ובונה מחדש את המניפסט באופן אטומי ברגע שדגל `is_last` נוחת,
  ביטול ספייק זיכרון RAM שנראה עם העלאות של שיחה אחת.【crates/iroha_torii/src/da/ingest.rs:392】
- **גירסאות מניפסט** - `DaManifestV1` נושא שדה `version` מפורש ו-Torii מסרב
  גרסאות לא ידועות, המבטיחות שדרוגים דטרמיניסטיים כאשר פריסות מניפסט חדשות נשלחות.【crates/iroha_data_model/src/da/types.rs:308】
- ** PDP/PoTR hooks** - התחייבויות PDP נובעות ישירות מחנות הנתחים ונמשכות
  לצד מניפסטים כדי שמתזמני DA-5 יוכלו להשיק אתגרי דגימה מנתונים קנוניים; את
  כותרת `Sora-PDP-Commitment` נשלחת כעת עם `/v2/da/ingest` ו-`/v2/da/manifests/{ticket}`
  תגובות כך ש-SDKs ילמדו מיד את ההתחייבות החתומה שבדיקות עתידיות יפנו אליה.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest】s:476.
- **יומן סמן רסיס** - מטא נתונים של נתיבים עשויים לציין `da_shard_id` (ברירת המחדל ל-`lane_id`), וכן
  Sumeragi מחזיק כעת את ה-`(epoch, sequence)` הגבוה ביותר לכל `(shard_id, lane_id)`
  `da-shard-cursors.norito` לצד סליל ה-DA, אז הפעלה מחדש זרוק נתיבים משובצים/לא ידועים ושמור
  שידור חוזר דטרמיניסטי. אינדקס סמן הרסיסים בזיכרון נכשל כעת במהירות בהתחייבויות עבור
  נתיבים לא ממופים במקום ברירת מחדל למזהה הנתיב, ביצוע שגיאות התקדמות סמן והפעלה חוזרת
  אימות מפורש וחסימה דוחה רגרסיות של סמן רסיסים עם ייעודי
  `DaShardCursorViolation` סיבה + תוויות טלמטריה למפעילים. ההפעלה/הדפיסה עוצר כעת את DA
  אינדקס הידרציה אם Kura מכיל נתיב לא ידוע או סמן נסוג ומתעד את הפוגע
  גובה הבלוק כך שהמפעילים יכולים לתקן לפני שירות DA מצב.【ארגזים/iroha_config/src/parameters/actual.rs】【ארגזים/iroha_core/src/da/shard_cursor.rs】【ארגזים/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **טלמטריית השהיית סמן רסיס** - מד `da_shard_cursor_lag_blocks{lane,shard}` מדווח כיצדרסיס רחוק אחרי הגובה המאושר. נתיבים חסרים/מעופשים/לא ידועים מגדירים את הפיגור ל-
  הגובה הנדרש (או דלתא), והתקדמות מוצלחות מאפסות אותו לאפס כך שהמצב היציב יישאר שטוח.
  מפעילים צריכים להתריע על איחורים שאינם אפס, לבדוק את הסליל/יומן DA עבור הנתיב הפוגע,
  ואמת את קטלוג הנתיבים לשינוי בשוגג לפני הפעלה חוזרת של הבלוק כדי לנקות את
  פער.
- **נתיבי חישוב חסויים** - נתיבים המסומנים ב
  `metadata.confidential_compute=true` ו-`confidential_key_version` מטופלים כאל
  SMPC/נתיבי DA מוצפנים: Sumeragi אוכפת תקצירי עומס/מניפסט שאינם אפס וכרטיסי אחסון,
  דוחה פרופילי אחסון בעותק מלא, ומוסיף לאינדקס את גרסת כרטיס SoraFS + מדיניות ללא
  חשיפת בתים של מטען. הקבלות מתנקות מ-Kura במהלך השידור החוזר, כך שמאמתים משחזרים אותו דבר
  מטא נתונים של סודיות לאחר מופעלת מחדש.【ארגזים/iroha_config/src/parameters/actual.rs】【ארגזים/iroha_core/src/da/confidential.rs】【ארגזים/iroha_core/src/da/confidential_store.rs】】【ecrates/irha_stat

## הערות יישום- נקודת הקצה `/v2/da/ingest` של Torii מנרמלת כעת את דחיסת המטען, אוכפת את מטמון השידור החוזר,
  חותך באופן דטרמיניסטי את הבתים הקנוניים, בונה מחדש את `DaManifestV1` ומפיל את המטען המקודד
  לתוך `config.da_ingest.manifest_store_dir` עבור SoraFS תזמור לפני הנפקת הקבלה; את
  המטפל מצרף גם כותרת `Sora-PDP-Commitment` כדי שלקוחות יוכלו ללכוד את המחויבות המקודדת
  מיד.【ארגזים/iroha_torii/src/da/ingest.rs:220】
- לאחר שהתמידו ב-`DaCommitmentRecord` הקנוני, Torii פולט כעת
  קובץ `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ליד סליל המניפסט.
  כל רשומה מאגדת את הרשומה עם הבתים הגולמיים Norito `PdpCommitment` כך שבוני חבילות DA-3 ו
  מתזמני DA-5 בולעים תשומות זהות מבלי לקרוא מחדש מניפסטים או מאגרי נתחים.【crates/iroha_torii/src/da/ingest.rs:1814】
- עוזרי SDK חושפים את בתים של כותרת PDP מבלי לאלץ כל לקוח ליישם מחדש את ניתוח Norito:
  כיסוי `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` Rust, ה-Python `ToriiClient`
  מייצאת כעת `decode_pdp_commitment_header`, ו-`IrohaSwift` שולחת עוזרים תואמים כל כך ניידים
  לקוחות יכולים לאחסן את לוח הזמנים של הדגימה המקודד באופן מיידי.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii חושף גם את `GET /v2/da/manifests/{storage_ticket}` כך ש-SDK ואופרטורים יכולים להביא מניפסטים
  ותוכניות נתחים מבלי לגעת בספריית הסליל של הצומת. התגובה מחזירה את ה-Norito בתים
  (base64), מניפסט JSON שעבר עיבוד, כתם `chunk_plan` JSON מוכן ל-`sorafs fetch`, בתוספת הרלוונטי
  hex digests (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) כך שהכלים במורד הזרם יכולים
  להאכיל את המתזמר מבלי לחשב מחדש תקצירים, ופולט את אותה כותרת `Sora-PDP-Commitment` ל
  תגובות בליעת מראה. העברת `block_hash=<hex>` כפרמטר שאילתה מחזירה דטרמיניסטית
  `sampling_plan` מושרש ב-`block_hash || client_blob_id` (משותף בין מאמתים) המכיל את
  `assignment_hash`, ה-`sample_window` המבוקש, ו-`(index, role, group)` דוגמת tuples משתרעות
  את כל פריסת הפסים הדו-ממדיים כך שדגמי PoR ומאמתים יכולים להפעיל מחדש את אותם מדדים. הסמפלר
  מערבבת `client_blob_id`, `chunk_root` ו-`ipa_commitment` לתוך ה-hash של ההקצאה; `iroha app da get
  --block-hash ` now writes `sampling_plan_.json` לצד המניפסט + תוכנית נתח עם
  ה-hash נשמר, ולקוחות JS/Swift Torii חושפים את אותו `assignment_hash_hex` כך שמאמתים
  ומוכיחים חולקים מערך בדיקה דטרמיניסטי יחיד. כאשר Torii מחזירה תוכנית דגימה, `iroha app da
  prove-availability` now reuses that deterministic probe set (seed derived from `sample_seed`) במקום זאת
  של דגימה אד-הוק כך שעדי PoR יעמדו בתור עם הקצאות אימות גם אם המפעיל משמיט
  `--block-hash` לעקוף.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】 【javascript/iroha_js/src/toriiClient.js:15903】】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### זרימת מטען גדוללקוחות שצריכים להטמיע נכסים גדולים מהמגבלה שהוגדרה לבקשה יחידה יוזמים א
הפעלת סטרימינג על ידי התקשרות ל-`POST /v2/da/ingest/chunk/start`. Torii מגיב עם א
`ChunkSessionId` (BLAKE3-נגזר מהמטא-נתונים של הכתם המבוקשים) וגודל הנתח שנקבע.
כל בקשה הבאה `DaIngestChunk` נושאת:

- `client_blob_id` - זהה ל-`DaIngestRequest` הסופי.
- `chunk_session_id` - קושר פרוסות לסשן הריצה.
- `chunk_index` ו-`offset` - אכיפת סדר דטרמיניסטי.
- `payload` - עד לגודל הנתח שנקבע.
- `payload_hash` — גיבוב BLAKE3 של הפרוסה כך ש-Torii יכול לאמת מבלי לאחסן את כל הבלוק.
- `is_last` - מציין את פרוסת המסוף.

Torii נמשך פרוסות מאומתות תחת `config.da_ingest.manifest_store_dir/chunks/<session>/` ו
מתעד התקדמות בתוך מטמון השידור החוזר כדי לכבד את האי-דמוקרטיה. כאשר הפרוסה האחרונה נוחתת, Torii
מרכיב מחדש את המטען בדיסק (הזרמה דרך ספריית ה-chunk כדי למנוע עליות זיכרון),
מחשב את המניפסט/הקבלה הקנונית בדיוק כמו בהעלאות של צילום יחיד, ולבסוף מגיב ל
`POST /v2/da/ingest` על ידי צריכת החפץ המבוים. הפעלות שנכשלו ניתן לבטל באופן מפורש או
נאספים אשפה לאחר `config.da_ingest.replay_cache_ttl`. עיצוב זה שומר על פורמט הרשת
Norito ידידותית, נמנעת מפרוטוקולים הניתנים לחידוש פעולה ספציפיים ללקוח, ומשתמשת מחדש בצינור המניפסט הקיים
ללא שינוי.

**סטטוס יישום.** סוגי Norito הקנוניים חיים כעת
`crates/iroha_data_model/src/da/`:

- `ingest.rs` מגדיר את `DaIngestRequest`/`DaIngestReceipt`, יחד עם
  מיכל `ExtraMetadata` בשימוש על ידי Torii.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` מארח את `DaManifestV1` ו-`ChunkCommitment`, ש-Torii פולט לאחר
  החתיכה הושלמה.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` מספק כינויים משותפים (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` וכו') ומקודד את ערכי ברירת המחדל של מדיניות המתועדים להלן.【crates/iroha_data_model/src/da/types.rs:240】
- קבצי סליל מניפסט נוחתים ב-`config.da_ingest.manifest_store_dir`, מוכנים לתזמור SoraFS
  צופה לכניסה לאחסון.【ארגזים/iroha_torii/src/da/ingest.rs:220】
- Sumeragi אוכף זמינות מניפסט בעת איטום או אימות של חבילות DA:
  בלוקים נכשלים באימות אם ב-spool חסר המניפסט או שה-hash שונה
  מההתחייבות.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

מעקב הלוך ושוב עבור מטעני הבקשה, המניפסט והקבלות מתבצע במעקב
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, הבטחת ה-Codec Norito
נשאר יציב בכל העדכונים.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**ברירות מחדל לשמירה.** הממשל אשרר את מדיניות השמירה הראשונית במהלך
SF-6; ברירות המחדל שנאכפות על ידי `RetentionPolicy::default()` הן:- שכבה חמה: 7 ימים (`604_800` שניות)
- שכבה קרה: 90 ימים (`7_776_000` שניות)
- העתקים נדרשים: `3`
- דרגת אחסון: `StorageClass::Hot`
- תג ניהול: `"da.default"`

מפעילים במורד הזרם חייבים לעקוף ערכים אלה במפורש כאשר נתיב מאמצים
דרישות מחמירות יותר.

## חפצי אמנות הוכחת לקוח חלודה

ערכות SDK שמטמיעות את לקוח Rust כבר לא צריכות לשלוף ל-CLI כדי
לייצר את חבילת PoR JSON הקנונית. ה-`Client` חושף שני עוזרים:

- `build_da_proof_artifact` מחזיר את המבנה המדויק שנוצר על ידי
  `iroha app da prove --json-out`, כולל הערות המניפסט/עומס המטען שסופקו
  דרך [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` עוטף את הבונה ומעביר את החפץ לדיסק
  (JSON יפה + שורה חדשה נגררת כברירת מחדל) כך שהאוטומציה יכולה לצרף את הקובץ
  לשחרורים או חבילות ראיות ממשל.【crates/iroha/src/client.rs:3653】

### דוגמה

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

מטען ה-JSON שעוזב את העוזר תואם את ה-CLI לשמות השדות
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest` וכו'), כל כך קיים
אוטומציה יכולה להבדיל/פרקט/להעלות את הקובץ ללא ענפים ספציפיים לפורמט.

## אמת מידה לאימות הוכחה

השתמש ברתום ההוכחה של DA כדי לאמת תקציבי מאמת על מטענים מייצגים לפני כן
הידוק מכסים ברמת בלוק:

- `cargo xtask da-proof-bench` בונה מחדש את חנות הנתחים מצמד המניפסט/מטען, דוגמת PoR
  משאיר ואימות זמנים מול התקציב שהוגדר. מטא-נתונים של Taikai ממולאים אוטומטית, וה-
  הרתמה נופלת בחזרה למניפסט סינתטי אם צמד המתקנים אינו עקבי. כאשר `--payload-bytes`
  מוגדר ללא `--payload` מפורש, הבלוק שנוצר נכתב אל
  `artifacts/da/proof_bench/payload.bin` כך שהמתקנים יישארו ללא נגיעה.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- דוחות כברירת מחדל ל-`artifacts/da/proof_bench/benchmark.{json,md}` וכוללים הוכחות/הפעלה, סה"כ ו
  תזמונים לכל הוכחה, שיעור מעבר תקציב ותקציב מומלץ (110% מהאיטרציה האיטית ביותר)
  קו עם `zk.halo2.verifier_budget_ms`.【artifacts/da/proof_bench/benchmark.md:1】
- ריצה אחרונה (עומס סינטטי של 1 MiB, נתחי 64 KiB, 32 הוכחות/ריצה, 10 איטרציות, תקציב של 250 אלפיות השנייה)
  המליץ על תקציב אימות של 3 אלפיות השנייה עם 100% מהחזרות בתוך המכסה.【artifacts/da/proof_bench/benchmark.md:1】
- דוגמה (יוצר מטען דטרמיניסטי וכותב את שני הדוחות):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

הרכבת בלוק אוכפת את אותם תקציבים: `sumeragi.da_max_commitments_per_block` ו
`sumeragi.da_max_proof_openings_per_block` משער את צרור ה-DA לפני שהוא מוטבע בבלוק, ו
כל התחייבות חייבת לשאת `proof_digest` שאינו אפס. השומר מתייחס לאורך הצרור כאל
ספירת פתיחת הוכחות עד לסיכומי הוכחה מפורשים מושחלים בקונצנזוס, תוך שמירה על
≤128 פתחים ניתנים לאכיפה בגבול הבלוק.【ארגזים/iroha_core/src/sumeragi/main_loop.rs:6573】

## טיפול בכשל PoR וחיתוךעובדי אחסון מציגים כעת פסי כשל ב-PoR והמלצות חתך מלוכדות לצד כל אחד מהם
פסק הדין. כשלים עוקבים מעל סף ההתקפה המוגדר פולטים המלצה ש
כולל את צמד הספק/מניפסט, אורך הרצף שהפעיל את הצלח, וההצעה המוצעת
קנס שחושב מאגרת הספק ו-`penalty_bond_bps`; חלונות קירור (שניות) לשמור
חתכים כפולים מירי על אותו תקרית.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/】s:343.

- הגדר ספים/התקררות באמצעות בונה עובדי האחסון (ברירות המחדל משקפות את הממשל
  מדיניות קנסות).
- המלצות סלאש נרשמות בסיכום פסק הדין JSON כך שממשל/מבקרים יכולים לצרף
  אותם לצרורות ראיות.
- פריסת פס + תפקידים לכל נתח מושחלים כעת דרך נקודת הקצה של סיכת האחסון של Torii
  (שדות `stripe_layout` + `chunk_roles`) והתמידו בעובד האחסון כך
  מבקרים/כלי תיקון יכולים לתכנן תיקוני שורות/עמודות מבלי לגזור מחדש את הפריסה ממעלה הזרם

### מיקום + רתמת תיקון

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` עכשיו
מחשב גיבוב של מיקום מעל `(index, role, stripe/column, offsets)` ומבצע שורה תחילה ואז
תיקון עמודה RS(16) לפני שחזור המטען:

- מיקום ברירת המחדל הוא `total_stripes`/`shards_per_stripe` כאשר הוא קיים ונופל בחזרה לנתח
- נתחים חסרים/פגומים נבנים מחדש עם שוויון שורה תחילה; הפערים הנותרים מתוקנים עם
  שוויון פס (עמודה). נתחים מתוקנים נכתבים בחזרה לספריית ה-chunk, ול-JSON
  סיכום לוכד את ה-hash של המיקום בתוספת מוני תיקון שורות/עמודות.
- אם שוויון שורה+עמודה אינו יכול לספק את הסט החסר, הרתמה נכשלת במהירות עם הבלתי ניתן לשחזור
  מדדים כדי שמבקרים יוכלו לסמן מניפסטים בלתי הפיכים.