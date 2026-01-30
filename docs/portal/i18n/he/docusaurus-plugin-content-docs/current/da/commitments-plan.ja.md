---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8f5f28f5ebd0c946d6de1775fef39c3cddefb601562e9f42f7d70b219e1294c
source_last_modified: "2025-11-14T04:43:19.713786+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/da/commitments_plan.md`. שמרו על שתי הגרסאות
מסונכרנות עד שהמסמכים הישנים יפרשו.
:::

# תוכנית התחייבויות Data Availability של Sora Nexus (DA-3)

_טיוטה: 2026-03-25 -- בעלי ענין: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 מרחיב את פורמט הבלוק של Nexus כך שכל lane משלב רשומות דטרמיניסטיות
המתארות את ה-blobs שהתקבלו ב-DA-2. מסמך זה מפרט את מבני הנתונים הקנוניים,
hooks של צנרת הבלוקים, הוכחות לקוח קל, ומשטחי Torii/RPC שחייבים להגיע לפני
שהמאמתים יוכלו להסתמך על התחייבויות DA בבדיקות קבלה או ממשל. כל ה-payloads
מקודדים ב-Norito; בלי SCALE ובלי JSON אד-הוק.

## יעדים

- לשאת התחייבויות לכל blob (chunk root + manifest hash + התחייבות KZG אופציונלית)
  בתוך כל בלוק Nexus כך שעמיתים יוכלו לשחזר מצב availability בלי לגשת לאחסון
  מחוץ ללדג'ר.
- לספק הוכחות membership דטרמיניסטיות כך שלקוחות קלים יאמתו ש-manifest hash
  הושלם בבלוק נתון.
- לחשוף שאילתות Torii (`/v1/da/commitments/*`) והוכחות שמאפשרות ל-relays,
  SDKs ואוטומציה של ממשל לבצע audit ל-availability בלי לשחזר כל בלוק.
- לשמור על המעטפת הקנונית `SignedBlockWire` על ידי השחלת המבנים החדשים דרך
  כותרת metadata של Norito וגזירת ה-hash של הבלוק.

## סקירת היקף

1. **תוספות למודל הנתונים** ב-`iroha_data_model::da::commitment` לצד שינויים
   בכותרת הבלוק ב-`iroha_data_model::block`.
2. **Hooks של executor** כדי ש-`iroha_core` יקלוט receipts של DA שנפלטו מ-Torii
   (`crates/iroha_core/src/queue.rs` ו-`crates/iroha_core/src/block.rs`).
3. **Persistence/indexes** כדי שה-WSV יענה במהירות לשאילתות התחייבות
   (`iroha_core/src/wsv/mod.rs`).
4. **תוספות RPC ב-Torii** עבור endpoints של list/query/prove תחת
   `/v1/da/commitments`.
5. **בדיקות אינטגרציה + fixtures** המאמתות את wire layout ואת זרימת ה-proof
   ב-`integration_tests/tests/da/commitments.rs`.

## 1. תוספות למודל הנתונים

### 1.1 `DaCommitmentRecord`

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
```

- `KzgCommitment` עושה שימוש מחדש בנקודת 48 bytes הקיימת תחת
  `iroha_crypto::kzg`. כאשר הוא חסר, נופלים להוכחות Merkle בלבד.
- `proof_scheme` נגזר מקטלוג lanes; lanes מסוג Merkle דוחים payloads של KZG
  בעוד lanes `kzg_bls12_381` דורשים commitments של KZG שאינם אפס. Torii מפיק
  כרגע רק התחייבויות Merkle ודוחה lanes שמוגדרות עם KZG.
- `KzgCommitment` עושה שימוש מחדש בנקודת 48 bytes הקיימת תחת
  `iroha_crypto::kzg`. כאשר הוא חסר ב-lanes Merkle חוזרים להוכחות Merkle בלבד.
- `proof_digest` מקדים אינטגרציית DA-5 PDP/PoTR כדי שאותה רשומה תפרט את
  לוח הדגימות המשמש לשמירת blobs בחיים.

### 1.2 הרחבת כותרת הבלוק

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

ה-hash של ה-bundle נכנס גם ל-hash הבלוק וגם למטא-דטה של `SignedBlockWire`.

הערת מימוש: `BlockPayload` ו-`BlockBuilder` השקוף חושפים כעת setters/getters
של `da_commitments` (ראו `BlockBuilder::set_da_commitments` ו-
`SignedBlock::set_da_commitments`), כך שמארחים יכולים לצרף bundle מוכן מראש
לפני איטום בלוק. כל ה-helpers משאירים את השדה `None` עד ש-Torii משחיל bundles
אמיתיים.

### 1.3 Wire encoding

- `SignedBlockWire::canonical_wire()` מוסיף את כותרת Norito עבור
  `DaCommitmentBundle` מיד לאחר רשימת הטרנזקציות הקיימת. ה-byte של הגרסה הוא
  `0x01`.
- `SignedBlockWire::decode_wire()` דוחה bundles עם `version` לא מוכר, בהתאם
  למדיניות Norito המפורטת ב-`norito.md`.
- עדכוני נגזרת hash קיימים רק ב-`block::Hasher`; לקוחות קלים שמפענחים את
  ה-wire format הקיים מרוויחים את השדה החדש אוטומטית כי כותרת Norito מודיעה
  על נוכחותו.

## 2. זרימת הפקת בלוקים

1. Torii DA ingest מסיים `DaIngestReceipt` ומפרסם אותו בתור הפנימי
   (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` אוסף את כל ה-receipts שה-`lane_id` שלהם תואם לבלוק
   בבניה, תוך דה-דופליקציה לפי `(lane_id, client_blob_id, manifest_hash)`.
3. רגע לפני האיטום, ה-builder ממיין commitments לפי `(lane_id, epoch, sequence)`
   כדי לשמור על hash דטרמיניסטי, מקודד את ה-bundle באמצעות codec של Norito,
   ומעדכן `da_commitments_hash`.
4. ה-bundle המלא נשמר ב-WSV ונפלט יחד עם הבלוק בתוך `SignedBlockWire`.

אם יצירת הבלוק נכשלת, ה-receipts נשארים בתור כדי שהנסיון הבא יאסוף אותם; ה-builder
מתעד את ה-`sequence` האחרון שנכלל בכל lane כדי למנוע מתקפות replay.

## 3. משטח RPC ושאילתות

Torii חושף שלושה endpoints:

| Route | Method | Payload | Notes |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (סינון טווח לפי lane/epoch/sequence, pagination) | מחזיר `DaCommitmentPage` עם סכום כולל, commitments ו-hash בלוק. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash או tuple `(epoch, sequence)`). | מחזיר `DaCommitmentProof` (record + Merkle path + hash בלוק). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | helper stateless שמחשב מחדש את hash הבלוק ומוודא inclusion; משמש SDKs שלא יכולים לקשר ישירות ל-`iroha_crypto`. |

כל ה-payloads נמצאים תחת `iroha_data_model::da::commitment`. הנתבים של Torii
מרכיבים את ה-handlers לצד endpoints ingest הקיימים כדי למחזר מדיניות token/mTLS.

## 4. הוכחות inclusion ולקוחות קלים

- מפיק הבלוקים בונה עץ Merkle בינארי על רשימת `DaCommitmentRecord` המוסדרת.
  השורש מזין את `da_commitments_hash`.
- `DaCommitmentProof` אורז את הרשומה היעד עם וקטור של `(sibling_hash, position)`
  כדי שמאמתים יוכלו לשחזר את השורש. ההוכחות כוללות גם hash בלוק וכותרת חתומה
  כדי שלקוחות קלים יאמתו finality.
- helpers של CLI (`iroha_cli app da prove-commitment`) עוטפים את מחזור הבקשה/אימות
  ומציגים פלטים Norito/hex עבור מפעילים.

## 5. Storage ואינדוקס

ה-WSV מאחסן commitments בעמודת column family ייעודית שממופתחת לפי `manifest_hash`.
אינדקסים משניים מכסים `(lane_id, epoch)` ו-`(lane_id, sequence)` כך ששאילתות לא
סורקות bundles מלאים. כל record מתעד את גובה הבלוק שאטם אותו, ומאפשר לצמתים
ב-catch-up לבנות מחדש את האינדקס במהירות מה-block log.

## 6. Telemetry ו-Observability

- `torii_da_commitments_total` עולה כאשר בלוק אוטם לפחות record אחד.
- `torii_da_commitment_queue_depth` עוקב אחרי receipts שממתינים ל-bundle (לפי lane).
- דשבורד Grafana `dashboards/grafana/da_commitments.json` מציג inclusion בבלוקים,
  עומק תור ו-throughput של proofs כדי ש-gates של DA-3 יוכלו לבצע audit התנהגות.

## 7. אסטרטגיית בדיקות

1. **בדיקות יחידה** ל-encoding/decoding של `DaCommitmentBundle` ולעדכוני נגזרת
   hash של בלוק.
2. **Fixtures golden** תחת `fixtures/da/commitments/` הלוכדות bytes קנוניים של
   bundle והוכחות Merkle.
3. **בדיקות אינטגרציה** שמרימות שני מאמתים, ingest של blobs לדוגמה, ומוודאות
   ששני הצמתים מסכימים על תוכן ה-bundle ועל תשובות query/proof.
4. **בדיקות light-client** ב-`integration_tests/tests/da/commitments.rs` (Rust)
   שקוראות `/prove` ומאמתות את ההוכחה בלי לדבר עם Torii.
5. **Smoke CLI** עם `scripts/da/check_commitments.sh` לשמירה על tooling מפעילים
   בר-שחזור.

## 8. תוכנית rollout

| Phase | Description | Exit Criteria |
|-------|-------------|---------------|
| P0 - Data model merge | שילוב `DaCommitmentRecord`, עדכוני כותרת בלוק וקודקים Norito. | `cargo test -p iroha_data_model` ירוק עם fixtures חדשים. |
| P1 - Core/WSV wiring | חיווט לוגיקת תור + block builder, שמירת indexes וחשיפת handlers של RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` עוברים עם assertions של bundle proof. |
| P2 - Operator tooling | שחרור helpers של CLI, דשבורד Grafana ועדכוני docs לאימות proofs. | `iroha_cli app da prove-commitment` עובד מול devnet; הדשבורד מציג נתונים חיים. |
| P3 - Governance gate | הפעלת מאמת בלוקים שמחייב commitments של DA ב-lanes המסומנות ב-`iroha_config::nexus`. | עדכון status + roadmap מסמנים את DA-3 כשלם. |

## שאלות פתוחות

1. **KZG vs Merkle defaults** - האם עלינו לדלג על commitments של KZG עבור blobs
   קטנים כדי להפחית את גודל הבלוק? הצעה: להשאיר `kzg_commitment` אופציונלי
   ולגדר דרך `iroha_config::da.enable_kzg`.
2. **Sequence gaps** - האם לאפשר lanes מחוץ לסדר? התכנית הנוכחית דוחה gaps אלא
   אם ממשל מפעיל `allow_sequence_skips` עבור replay חירום.
3. **Light-client cache** - צוות ה-SDK ביקש cache SQLite קל עבור proofs; במעקב
   תחת DA-8.

מענה לשאלות אלו ב-PRs של מימוש יעביר את DA-3 מ"טיוטה" (מסמך זה) ל"בתהליך"
כאשר עבודת הקוד תתחיל.
