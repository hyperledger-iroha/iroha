---
lang: he
direction: rtl
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T15:38:30.660808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus תוכנית התחייבויות זמינות נתונים (DA-3)

_נוסח: 2026-03-25 — בעלים: Core Protocol WG / צוות חוזה חכם / צוות אחסון_

DA-3 מרחיב את פורמט הבלוק Nexus כך שכל נתיב מטמיע רשומות דטרמיניסטיות
מתאר את הכתמים המקובלים על ידי DA-2. הערה זו לוכדת את הנתונים הקנוניים
מבנים, ווי צינור בלוקים, הוכחות לקוח אור ומשטחי Torii/RPC
שחייבים לנחות לפני שמאמתים יוכלו להסתמך על התחייבויות DA במהלך הקבלה או
בדיקות ממשל. כל המטענים מקודדים Norito; ללא SCALE או JSON אד-הוק.

## יעדים

- נשא בהתחייבויות לכל כתם (שורש נתח + חשיש מניפסט + KZG אופציונלי
  מחויבות) בתוך כל בלוק Nexus כדי שעמיתים יוכלו לשחזר את הזמינות
  מצב מבלי להתייעץ עם אחסון מחוץ לפנקס החשבונות.
- ספק הוכחות חברות דטרמיניסטיות כדי שלקוחות קלים יוכלו לאמת שא
  הגיבוב של מניפסט הושלם בבלוק נתון.
- חשוף שאילתות Torii (`/v1/da/commitments/*`) והוכחות המאפשרות לממסרים,
  ערכות SDK ואוטומציה של ממשל בודקים זמינות מבלי להפעיל מחדש כל
  לחסום.
- שמור את המעטפה הקיימת `SignedBlockWire` קנונית על ידי השחלה של המעטפה החדשה
  מבנים דרך כותרת המטא-נתונים Norito וגזירת חשיש של בלוק.

## סקירת היקף

1. **תוספות דגם נתונים** בלוק `iroha_data_model::da::commitment` פלוס
   שינויים בכותרת ב-`iroha_data_model::block`.
2. **הווים מבצעים** כך ש-`iroha_core` קולט קבלות DA הנפלטות על ידי Torii
   (`crates/iroha_core/src/queue.rs` ו-`crates/iroha_core/src/block.rs`).
3. **התמדה/אינדקסים** כך שה-WSV יוכל לענות על שאילתות התחייבות במהירות
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii תוספות RPC** עבור רשימה/שאילתה/הוכחה של נקודות קצה תחת
   `/v1/da/commitments`.
5. **מבחני אינטגרציה + מתקנים** המאמתים את פריסת החוט וזרימת ההוכחה פנימה
   `integration_tests/tests/da/commitments.rs`.

## 1. תוספות מודל נתונים

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

- `KzgCommitment` עושה שימוש חוזר בנקודת 48 בתים הקיימת בשימוש תחת
  `iroha_crypto::kzg`. נתיבי מרקל משאירים אותו ריק; נתיבי `kzg_bls12_381` עכשיו
  לקבל מחויבות BLAKE3-XOF דטרמיניסטית הנגזרת משורש ה-chunk ו
  כרטיס אחסון כך ש-hashs לחסום יישארו יציבים ללא מוכיח חיצוני.
- `proof_scheme` נגזר מקטלוג הנתיבים; נתיבי מרקל דוחים KZG תועה
  מטענים בעוד נתיבים `kzg_bls12_381` דורשים התחייבויות KZG שאינן אפס.
- `proof_digest` צופה שילוב DA-5 PDP/PoTR כך שרשומה זהה
  מונה את לוח הזמנים של הדגימה המשמש לשמירה על כתמים חיים.

### 1.2 סיומת כותרת חסום

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

הגיבוב של החבילה ניזון הן ל-hash הבלוק והן למטא נתונים של `SignedBlockWire`.
מִמַעַל.

הערת יישום: `BlockPayload` וה-`BlockBuilder` השקוף חושפים כעת
`da_commitments` מגדירים/גיטרים (ראה `BlockBuilder::set_da_commitments` ו-
`SignedBlock::set_da_commitments`), כך שהמארחים יכולים לצרף חבילה בנויה מראש
לפני איטום בלוק. כל הבנאים המסייעים ברירת המחדל של השדה הוא `None`
עד ש-Torii יעביר חבילות אמיתיות.

### 1.3 קידוד חוט- `SignedBlockWire::canonical_wire()` מוסיף את הכותרת Norito עבור
  `DaCommitmentBundle` מיד לאחר רשימת העסקאות הקיימת. ה
  בייט הגרסה הוא `0x01`.
- `SignedBlockWire::decode_wire()` דוחה חבילות ש-`version` שלהם לא ידוע,
  תואם למדיניות Norito המתוארת ב-`norito.md`.
- עדכוני גזירת Hash קיימים רק ב-`block::Hasher`; פענוח לקוחות קל
  פורמט החוט הקיים מקבל אוטומטית את השדה החדש בגלל Norito
  header מפרסם את נוכחותו.

## 2. זרימת ייצור בלוק

1. Torii DA בליעה נמשכת קבלות חתומות ורשומות התחייבות ב-
   סליל DA (`da-receipt-*.norito` / `da-commitment-*.norito`). העמיד
   יומן קבלות משאיר סמנים בהפעלה מחדש, כך שהקבלות המשוחזרות עדיין מסודרות
   באופן דטרמיניסטי.
2. מכלול הבלוק טוען קבלות מהסליל, נופל מיושן/סתום כבר
   ערכים באמצעות תמונת המצב של הסמן המחויב, ואוכף צמידות לכל
   `(lane, epoch)`. אם קבלה ניתנת להשגה חסרה התחייבות תואמת או ה
   חשיש מניפסט מרחיק את ההצעה מבטלת במקום להשמיט אותה בשקט.
3. ממש לפני האיטום, הבנאי פורס את צרור ההתחייבות ל-
   סט מונע קבלה, ממיין לפי `(lane_id, epoch, sequence)`, מקודד את
   חבילה עם ה-Codec Norito, ומעדכנת את `da_commitments_hash`.
4. החבילה המלאה מאוחסנת ב-WSV ונפלטת לצד הבלוק שבתוכו
   `SignedBlockWire`; חבילות מחויבות מקדימות את סמני הקבלה (מודר
   מ-Kura בהפעלה מחדש) וגזום ערכי סליל מעופשים כדי לחייב את צמיחת הדיסק.

הרכבת בלוקים ו-`BlockCreated` מאמתים מחדש כל התחייבות כנגד
קטלוג הנתיבים: נתיבי מרקל דוחים התחייבויות תועה של KZG, נתיבי KZG דורשים א
מחויבות לא-אפס KZG ואי-אפס `chunk_root`, ונתיבים לא ידועים הם
ירד. נקודת הקצה `/v1/da/commitments/verify` של Torii משקפת את אותו מגן,
וצריבה עכשיו משחיל את המחויבות KZG הדטרמיניסטית לכל
`kzg_bls12_381` מתעד כך שחבילות תואמות מדיניות מגיעות להרכבת בלוקים.

גופי המניפסט המתוארים בתוכנית בליעת DA-2 משמשים כמקור
אמת לצרור המחויבות. מבחן Torii
`manifest_fixtures_cover_all_blob_classes` מחדש מניפסטים עבור כל
גרסה `BlobClass` ומסרבת לבצע קומפילציה עד שהמחלקות החדשות ירוויחו מתקנים,
להבטיח שה-hash המניפסט המקודד בתוך כל `DaCommitmentRecord` תואם את
זוג זהב Norito/JSON.【ארגזים/iroha_torii/src/da/tests.rs:2902】

אם יצירת הבלוק נכשל, הקבלות נשארות בתור אז החסימה הבאה
ניסיון יכול להרים אותם; הבנאי רושם את האחרון הכלולים `sequence` לכל
נתיב כדי למנוע התקפות חוזרות.

## 3. RPC ומשטח שאילתה

Torii חושף שלוש נקודות קצה:| מסלול | שיטה | מטען | הערות |
|-------|--------|--------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (מסנן טווח לפי נתיב/תקופה/רצף, עימוד) | מחזירה `DaCommitmentPage` עם ספירה כוללת, התחייבויות ו-hash חסימה. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (נתיב + מניפסט hash או `(epoch, sequence)` tuple). | מגיב עם `DaCommitmentProof` (רשומה + נתיב Merkle + hash בלוק). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | עוזר חסר מדינה שמציג מחדש את חישוב הגיבוב של הבלוק ומאמת הכללה; בשימוש על-ידי SDK שאינם יכולים לקשר ישירות ל-`iroha_crypto`. |

כל המטענים חיים מתחת ל-`iroha_data_model::da::commitment`. תושבת נתבים Torii
המטפלים שליד ה-DA הקיימים בולעים נקודות קצה כדי לעשות שימוש חוזר ב-token/mTLS
מדיניות.

## 4. הוכחות הכללה ולקוחות אור

- מפיק הבלוקים בונה עץ מרקל בינארי על הרצף
  רשימת `DaCommitmentRecord`. השורש מזין את `da_commitments_hash`.
- `DaCommitmentProof` אורזת את רשומת היעד בתוספת וקטור של `(sibling_hash,
  position)` ערכים כך שמאמתים יכולים לשחזר את השורש. ההוכחות כוללות גם
  הגיבוב החסום והכותרת החתומה כדי שלקוחות קלים יוכלו לאמת את הסופיות.
- עוזרי CLI (`iroha_cli app da prove-commitment`) עוטפים את בקשת ההוכחה/מאמתים
  מחזור ומשטח Norito/יציאות משושה למפעילים.

## 5. אחסון ואינדקס

WSV מאחסנת התחייבויות במשפחת עמודים ייעודית עם מפתחות `manifest_hash`.
האינדקסים המשניים מכסים את `(lane_id, epoch)` ו-`(lane_id, sequence)`, כך ששאלות
הימנע מסריקת חבילות מלאות. כל רשומה עוקבת אחר גובה הבלוק שאטם אותו,
המאפשר לצמתים לתפוס לבנות מחדש את האינדקס במהירות מיומן הבלוק.

## 6. טלמטריה וצפייה

- `torii_da_commitments_total` עולה בכל פעם שבלוק אוטם לפחות אחד
  להקליט.
- `torii_da_commitment_queue_depth` עוקב אחר קבלות הממתינות לארוז (לפי
  נתיב).
- לוח המחוונים Grafana `dashboards/grafana/da_commitments.json` מדמיין בלוק
  הכללה, עומק תור ותפוקת הוכחה כך ששערי שחרור DA-3 יכולים לבצע ביקורת
  התנהגות.

## 7. אסטרטגיית בדיקה

1. **בדיקות יחידה** עבור `DaCommitmentBundle` קידוד/פענוח ו-hash בלוק
   עדכוני גזירה.
2. **מתקני זהב** תחת `fixtures/da/commitments/` לכידת קנונית
   צרור בתים והוכחות מרקל. כל חבילה מפנה לבייטים של המניפסט
   מ-`fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`, אז
   מחדשת `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`
   שומר על עקביות הסיפור של Norito לפני ש-`ci/check_da_commitments.sh` מרענן את המחויבות
   הוכחות.【fixtures/da/ingest/README.md:1】
3. **מבחני אינטגרציה** מאתחלים שני מאמתים, בליעת כתמי דגימה ו
   בטענה ששני הצמתים מסכימים על תוכן החבילה ושאילתה/הוכחה
   תגובות.
4. **בדיקות קלות-לקוח** ב-`integration_tests/tests/da/commitments.rs`
   (חלודה) שמתקשרים ל-`/prove` ומאמתים את ההוכחה מבלי לדבר עם Torii.
5. **CLI עשן** סקריפט `scripts/da/check_commitments.sh` כדי לשמור על המפעיל
   כלים ניתנים לשחזור.

## 8. תוכנית השקה| שלב | תיאור | קריטריוני יציאה |
|-------|-------------|------------|
| P0 — מיזוג מודל נתונים | Land `DaCommitmentRecord`, עדכוני כותרות חסימות ו-Codec Norito. | `cargo test -p iroha_data_model` ירוק עם מתקנים חדשים. |
| P1 — חיווט ליבה/WSV | לוגיקה של תור שרשורים + בונה בלוקים, אינדקסים מתמידים וחשיפת מטפלי RPC. | מעבר `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` עם הצהרות הוכחת חבילה. |
| P2 — כלי עבודה למפעיל | שלח עוזרי CLI, לוח מחוונים Grafana ועדכוני מסמכים לאימות הוכחה. | `iroha_cli app da prove-commitment` פועל נגד devnet; לוח המחוונים מציג נתונים חיים. |
| P3 — שער ממשל | אפשר אימות חסימות המחייב התחייבויות DA בנתיבים המסומנים ב-`iroha_config::nexus`. | הזנת סטטוס + עדכון מפת הדרכים סמן DA-3 בתור 🈴. |

## שאלות פתוחות

1. **ברירת המחדל של KZG לעומת מרקל** - האם כתמים קטנים צריכים תמיד לדלג על התחייבויות KZG ל
   להקטין את גודל הבלוק? הצעה: השאר את `kzg_commitment` אופציונלי ושער דרך
   `iroha_config::da.enable_kzg`.
2. **פערים ברצף** — האם אנו מאפשרים נתיבים לא בסדר? התוכנית הנוכחית דוחה פערים
   אלא אם הממשל מחליף את `allow_sequence_skips` לשידור חירום חוזר.
3. **מטמון קליינט קל** — צוות SDK ביקש מטמון SQLite קל משקל עבור
   הוכחות; ממתין למעקב במסגרת DA-8.

מענה על אלה בהטמעת יחסי ציבור מעביר את DA-3 מ 🈸 (מסמך זה) ל 🈺
לאחר תחילת עבודת הקוד.