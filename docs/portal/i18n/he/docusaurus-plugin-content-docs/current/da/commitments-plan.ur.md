---
lang: he
direction: rtl
source: docs/portal/docs/da/commitments-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Sora Nexus תוכנית התחייבויות זמינות נתונים (DA-3)

_مسودہ: 2026-03-25 -- مالکان: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 Nexus דטרמיניסטית מסלול רצועה.
records شامل ہوں جو DA-2 کے ذریعے قبول شدہ blobs کو بیان کریں۔ یہ نوٹ
מבני נתונים קנוניים, ווי צנרת בלוק, הוכחות לקוח אור, ועוד
Torii/RPC משטחים
checks میں DA commitments پر بھروسہ کرنے سے پہلے لازمی ہیں۔ تمام payloads
Norito בקידוד SCALE یا ad-hoc JSON نہیں۔

## מידע

- ہر Nexus بلاک میں per-blob commitments (chunk root + manifest hash + optional
  KZG commitment) شامل کرنا تاکہ peers off-ledger storage دیکھے بغیر
  מצב זמינות
- הוכחות חברות דטרמיניסטיות.
  کہ manifest hash کسی مخصوص بلاک میں finalised ہوا تھا۔
- שאילתות Torii (`/v1/da/commitments/*`) הוכחות של ממסרים,
  SDKs، اور governance automation ہر بلاک replay کئے بغیر availability کا
  audit کر سکیں۔
- `SignedBlockWire` envelope کو canonical رکھنا، نئی structures کو Norito
  metadata header اور block hash derivation سے thread کرتے ہوئے۔

## סקירת היקף

1. **Data model additions** `iroha_data_model::da::commitment` میں اور block
   header changes `iroha_data_model::block` میں۔
2. **Executor hooks** تاکہ `iroha_core` Torii سے emit شدہ DA receipts ingest
   کرے (`crates/iroha_core/src/queue.rs` اور `crates/iroha_core/src/block.rs`).
3. **Persistence/indexes** تاکہ WSV commitments queries تیزی سے handle کرے
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii RPC additions** list/query/prove endpoints کیلئے
   `/v1/da/commitments` کے تحت۔
5. **מבחני אינטגרציה + מתקנים** או פריסת חוטים או זרימת הוכחה ואימות
   کریں `integration_tests/tests/da/commitments.rs` میں۔

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

- `KzgCommitment` נייד 48-בייט שימוש חוזר ב-`iroha_crypto::kzg`
  میں ہے۔ جب absent ہو تو صرف Merkle proofs استعمال ہوتے ہیں۔
- קטלוג נתיב `proof_scheme` מרקל נתיבי KZG מטענים
  reject کرتی ہیں جبکہ `kzg_bls12_381` lanes non-zero KZG commitments require
  کرتی ہیں۔ Torii فی الحال صرف Merkle commitments بناتا ہے اور KZG-configured
  lanes کو reject کرتا ہے۔
- `KzgCommitment` נייד 48-בייט שימוש חוזר ב-`iroha_crypto::kzg`
  میں ہے۔ جب Merkle lanes پر absent ہو تو صرف Merkle proofs استعمال ہوتے ہیں۔
- שילוב של `proof_digest` DA-5 PDP/PoTR.
  record میں sampling schedule درج ہو جو blobs کو زندہ رکھنے کیلئے استعمال
  ہوتا ہے۔

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

Bundle hash بلاک hash اور `SignedBlockWire` metadata دونوں میں شامل ہوتا ہے۔ جب
بچیں۔הערת יישום: `BlockPayload` או שקוף `BlockBuilder`
`da_commitments` setters/getters expose کرتے ہیں (دیکھیں
`BlockBuilder::set_da_commitments` اور `SignedBlock::set_da_commitments`)، تاکہ
hosts بلاک seal ہونے سے پہلے pre-built bundle attach کر سکیں۔ تمام helper
constructors field کو `None` رکھتے ہیں جب تک Torii حقیقی bundles thread نہ کرے۔

### 1.3 קידוד חוט

- `SignedBlockWire::canonical_wire()` موجودہ transactions list کے فوراً بعد
  `DaCommitmentBundle` کیلئے Norito header append کرتا ہے۔ version byte `0x01` ہے۔
- `SignedBlockWire::decode_wire()` לא ידוע.
  ہے، جیسا کہ `norito.md` میں Norito policy بیان ہے۔
- Hash derivation updates صرف `block::Hasher` میں ہیں؛ פורמט חוט קיים
  decode کرنے والے light clients خود بخود نیا field حاصل کرتے ہیں کیونکہ
  Norito header اس کی موجودگی بتاتا ہے۔

## 2. זרימת ייצור בלוק

1. Torii DA ingest ایک `DaIngestReceipt` finalize کرتا ہے اور اسے internal queue
   پر publish کرتا ہے (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` تمام receipts جمع کرتا ہے جن کا `lane_id` زیر تعمیر بلاک سے
   match کرتا ہے، اور `(lane_id, client_blob_id, manifest_hash)` پر deduplicate
   کرتا ہے۔
3. Seal کرنے سے پہلے builder commitments کو `(lane_id, epoch, sequence)` سے
   sort کرتا ہے تاکہ hash deterministic رہے، bundle کو Norito codec سے encode
   کرتا ہے، اور `da_commitments_hash` update کرتا ہے۔
4. مکمل bundle WSV میں store ہوتا ہے اور `SignedBlockWire` کے ساتھ بلاک میں
   emit ہوتا ہے۔

اگر بلاک بنانا fail ہو تو receipts queue میں رہتے ہیں تاکہ اگلی کوشش انہیں لے
سکے؛ builder ہر lane کیلئے آخری شامل شدہ `sequence` ریکارڈ کرتا ہے تاکہ replay
attacks سے بچا جا سکے۔

## 3. RPC ומשטח שאילתה

Torii נקודות קצה אחרות:

| מסלול | שיטה | מטען | הערות |
|-------|--------|--------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (מסנן טווח נתיב/תקופה/רצף, עימוד) | `DaCommitmentPage` واپس کرتا ہے جس میں total count، commitments، اور block hash شامل ہے۔ |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash یا `(epoch, sequence)` tuple)۔ | `DaCommitmentProof` واپس کرتا ہے (record + Merkle path + block hash)۔ |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Stateless helper جو block hash calculation دوبارہ کرتا ہے اور inclusion validate کرتا ہے؛ ایسے SDKs کیلئے جو `iroha_crypto` سے براہ راست link نہیں کر سکتے۔ |

تمام payloads `iroha_data_model::da::commitment` کے تحت ہیں۔ נתבים Torii
handlers کو موجودہ DA ingest endpoints کے ساتھ mount کرتے ہیں تاکہ token/mTLS
policies reuse ہوں۔

## 4. הוכחות הכללה ולקוחות אור

- بلاک producer serialized `DaCommitmentRecord` list پر binary Merkle tree بناتا
  ہے۔ اس کی root `da_commitments_hash` کو feed کرتی ہے۔
- `DaCommitmentProof` target record کے ساتھ `(sibling_hash, position)` کا vector
  پیک کرتا ہے تاکہ verifiers root دوبارہ بنا سکیں۔ הוכחות לחסום hash אוור
  signed header بھی شامل ہیں تاکہ light clients finality verify کر سکیں۔
- CLI עוזרי (`iroha_cli app da prove-commitment`) בקשת הוכחה/מחזור אימות
  wrap کرتے ہیں اور operators کیلئے Norito/hex outputs دیتے ہیں۔## 5. אחסון ואינדקס

WSV commitments کو dedicated column family میں `manifest_hash` key کے ساتھ
store کرتا ہے۔ אינדקסים משניים `(lane_id, epoch)` אור `(lane_id, sequence)`
کو cover کرتے ہیں تاکہ queries پورے bundles scan نہ کریں۔ ہر record اس block
height کو track کرتا ہے جس نے اسے seal کیا، جس سے catch-up nodes block log سے
index تیزی سے rebuild کر سکتے ہیں۔

## 6. טלמטריה וצפייה

- `torii_da_commitments_total` تب increment ہوتا ہے جب کوئی بلاک کم از کم ایک
  record seal کرے۔
- `torii_da_commitment_queue_depth` bundle ہونے کے انتظار میں receipts کو
  track کرتا ہے (per lane)۔
- Grafana הכללת בלוק `dashboards/grafana/da_commitments.json`,
  queue depth اور proof throughput دکھاتا ہے تاکہ DA-3 release gates
  behavior audit کر سکیں۔

## 7. אסטרטגיית בדיקה

1. `DaCommitmentBundle` encoding/decoding اور block hash derivation updates کیلئے
   **בדיקות יחידה**.
2. `fixtures/da/commitments/` میں **golden fixtures** جو canonical bundle bytes
   اور Merkle proofs capture کرتے ہیں۔
3. **מבחני אינטגרציה** או אתחול של validators
   کرتے ہیں، اور bundle contents اور query/proof responses پر اتفاق verify کرتے
   ہیں۔
4. **בדיקות קלות לקוח** `integration_tests/tests/da/commitments.rs` (חלודה)
   میں جو `/prove` call کر کے Torii سے بات کئے بغیر proof verify کرتے ہیں۔
5. סקריפט **CLI עשן** `scripts/da/check_commitments.sh` כלי מפעיל
   reproducible رہے۔

## 8. תוכנית השקה

| שלב | תיאור | קריטריוני יציאה |
|-------|-------------|------------|
| P0 — מיזוג מודל נתונים | `DaCommitmentRecord`, block header updates اور Norito codecs land کریں۔ | `cargo test -p iroha_data_model` نئی fixtures کے ساتھ green۔ |
| P1 — חיווט ליבה/WSV | queue + block builder logic thread کریں، indexes persist کریں، اور RPC handlers expose کریں۔ | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` bundle proof assertions کے ساتھ pass۔ |
| P2 — כלי עבודה למפעיל | CLI helpers، Grafana dashboard، اور proof verification docs ship کریں۔ | `iroha_cli app da prove-commitment` devnet پر چلتا ہو؛ dashboard live data دکھائے۔ |
| P3 — שער ממשל | `iroha_config::nexus` میں flagged lanes کیلئے DA commitments require کرنے والا block validator enable کریں۔ | status entry + roadmap update DA-3 کو complete mark کریں۔ |

## שאלות פתוחות

1. **KZG vs Merkle defaults** — کیا چھوٹے blobs کیلئے KZG commitments ہمیشہ
   چھوڑ دئے جائیں تاکہ block size کم ہو؟ טקסט: `kzg_commitment` אופציונלי
   اور `iroha_config::da.enable_kzg` کے ذریعے gate کریں۔
2. **Sequence gaps** — کیا out-of-order lanes کی اجازت ہو؟ موجودہ پلان gaps کو
   reject کرتا ہے جب تک governance `allow_sequence_skips` کو emergency replay کیلئے
   enable نہ کرے۔
3. **Light-client cache** — SDK ٹیم نے proofs کیلئے ایک ہلکا SQLite cache مانگا
   20 DA-8 میں follow-up باقی ہے۔

ان سوالات کے جوابات implementation PRs میں دینے سے DA-3 اس مسودے سے نکل کر
"in progress" کی حالت میں جائے گا جب code work شروع ہوگا۔