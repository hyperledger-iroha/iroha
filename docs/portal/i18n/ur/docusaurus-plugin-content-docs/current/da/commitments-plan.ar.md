---
lang: ur
direction: rtl
source: docs/portal/docs/da/commitments-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ معیاری ماخذ
`docs/source/da/commitments_plan.md` کی عکاسی کرتا ہے۔ چیک آؤٹ ہونے تک دونوں کاپیاں مطابقت پذیری میں رکھیں
پرانی دستاویزات۔
:::

# SORA Nexus (DA-3) میں ڈیٹا کی دستیابی کے عزم کا منصوبہ

_ ڈرافٹ: 03-25-2026-مالکان: کور پروٹوکول ڈبلیو جی / سمارٹ کنٹریکٹ ٹیم / اسٹوریج ٹیم_

DA-3 Nexus بلاک فارمیٹ میں توسیع کرتا ہے تاکہ ہر لین میں قبول شدہ بلبس کو بیان کرنے والے عین مطابق ریکارڈ شامل ہوں۔
DA-2 سے۔ اس نوٹ میں معیاری ڈیٹا ڈھانچے ، بلاک پائپ لائن رابطوں کی وضاحت کی گئی ہے ،
جیسا کہ لائٹ کلائنٹس کے ذریعہ ثبوت دیا گیا ہے ، Torii/RPC انٹرفیس اپنانے سے پہلے مکمل ہونا ضروری ہے۔
قبولیت یا گورننس چیک کے دوران ڈی اے کے آڈیٹرز۔ تمام پے لوڈ کو خفیہ کردہ ہیں
Norito کے ساتھ ؛ نہ تو پیمانہ اور نہ ہی کسٹم JSON۔

## مقاصد

- ہر ایک کے اندر ہر بلاب (chunk جڑ + مینی فیسٹ ہیش + اختیاری KZG کمٹ) کے لئے بوجھ کا ارتکاب کرتا ہے
  Nexus کو بلاک کریں تاکہ ساتھی پیچھے گرے بغیر دستیابی کی حالت کو دوبارہ تعمیر کرسکیں
  نوٹ بک کے باہر اسٹور کرنا۔
- ڈٹرمینسٹک ممبرشپ کے ثبوت فراہم کریں تاکہ ہلکے وزن والے کلائنٹ اس بات کی تصدیق کرسکیں کہ ظاہر ہیش مکمل ہو
  اسے ایک مخصوص کلسٹر میں انسٹال کریں۔
- Torii (`/v2/da/commitments/*`) اور ایسے ثبوتوں کے لئے سوالات کی فہرست بنائیں جو ریلے اور ایس ڈی کے کی اجازت دیتے ہیں۔
  گورننس ٹولز ہر بلاک کو دوبارہ شروع کیے بغیر دستیابی کی جانچ پڑتال کرتے ہیں۔
- معیاری `SignedBlockWire` لفافے کو نئی بلڈز کے ذریعے منتقل کرکے برقرار رکھیں
  Norito میٹا ڈیٹا ہیڈر اور بلاک ہیش کا مشتق۔

## ڈومین جائزہ

1. ** ڈیٹا ماڈل کے اضافے ** تبدیلیوں کے ساتھ `iroha_data_model::da::commitment` میں
   بلاک ہیڈر `iroha_data_model::block` میں ہے۔
2. ** بندرگاہ کے لئے ہکس ** تاکہ `iroha_core` ڈی اے کی رسیدیں
   Torii (`crates/iroha_core/src/queue.rs` اور کے ذریعہ جاری کیا گیا
   `crates/iroha_core/src/block.rs`)۔
3.
   (`iroha_core/src/wsv/mod.rs`)۔
4. ** Torii میں RPC اضافے ** فہرست/استفسار/`/v2/da/commitments` کے تحت ثابت پوائنٹس کے لئے۔
5. ** انضمام ٹیسٹ + فکسچر ** تار کی ترتیب کو چیک کرنے اور بہاؤ کا ثبوت
   `integration_tests/tests/da/commitments.rs`۔

## 1۔ ڈیٹا ماڈل کے اضافے

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
    pub acknowledgement_sig: Signature,       // Norito DA service key
}
```

-`KzgCommitment` 48 بائٹ پوائنٹ میں دوبارہ استعمال کرتا ہے
  `iroha_crypto::kzg`۔ جب یہ غیر حاضر ہے تو ، ہم صرف میرکل کے ثبوتوں پر واپس آجاتے ہیں۔
- `proof_scheme` لینز کیٹلاگ سے ماخوذ ہے۔ مرکل لینز کے زیڈ جی پے لوڈ کو مسترد کرتے ہیں ،
  جبکہ لین `kzg_bls12_381` کو غیر صفر KZG وعدوں کی ضرورت ہے۔ Torii فی الحال پیداوار میں ہے
  مرکل صرف کے زیڈ جی پر فارمیٹ شدہ لینوں کا ارتکاب اور مسترد کرتا ہے۔
-`KzgCommitment` میں پائے جانے والے 48 بائٹ پوائنٹ کو دوبارہ استعمال کیا گیا
  `iroha_crypto::kzg`۔ جب یہ مرکل لین میں غیر حاضر رہتا ہے تو ، ہم صرف مرکل کے ثبوتوں پر واپس آجاتے ہیں۔
- Norito DA-5 PDP/POTR انضمام کے لئے راہ ہموار کرتا ہے تاکہ ریکارڈ اسی طرح کی میز کی فہرست بنائے۔
  نمونے بلبوں کو محفوظ رکھنے کے لئے استعمال ہوتے ہیں۔

### 1.2 بلاک ہیڈر کو بڑھاؤ

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

پیکٹ ہیش بلاک ہیش میں اور `SignedBlockWire` ڈیٹا میں داخل ہے۔ جب نہیں

ایگزیکٹو نوٹ: شفاف `BlockPayload` اور `BlockBuilder` اب دکھا رہے ہیں
`da_commitments` کے لئے سیٹٹرز/گیٹر (`BlockBuilder::set_da_commitments` دیکھیں
اور `SignedBlock::set_da_commitments`) تاکہ میزبان کسی پیکیج کو پہلے سے جوڑ سکے
بلاک اسٹیمپنگ سے پہلے تعمیر۔ تمام معاون افعال Torii کے گزرنے تک فیلڈ `None` چھوڑ دیتے ہیں۔
اصلی پیکیجز۔

### 1.3 وائر انکوڈنگ- `SignedBlockWire::canonical_wire()` ہیڈر Norito کے لئے شامل کرتا ہے
  موجودہ ٹرانزیکشن لسٹ کے فورا بعد `DaCommitmentBundle`۔ ورژن بائٹ ہے
  `0x01`۔
- Norito نامعلوم `version` کے ساتھ پیکٹوں کو مسترد کرتا ہے ، بشمول
  `norito.md` میں بیان کردہ پالیسی Norito کے ساتھ تعمیل کرتا ہے۔
- ہیش مشتق کی تازہ کاری صرف `block::Hasher` میں موجود ہے۔ ہلکے صارفین جو
  موجودہ تار کی شکل کو کھولنے سے خود بخود نیا فیلڈ مل جاتا ہے کیونکہ ہیڈر Norito ہے
  اپنی موجودگی کا اعلان کرتا ہے۔

## 2۔ بلاک پروڈکشن فلو

1. Torii DA انجسٹ عمل `DaIngestReceipt` رسید کو حتمی شکل دیتا ہے اور شائع کرتا ہے۔
   اندرونی قطار پر (`iroha_core::gossiper::QueueMessage::DaReceipt`)۔
2. `PendingBlocks` تمام رسیدیں جمع کرتا ہے جو بلاک کی تعمیر کے لئے `lane_id` سے ملتے ہیں ،
   `(lane_id, client_blob_id, manifest_hash)` کے مطابق ڈپلیکیٹ کے ساتھ ہٹا دیا گیا۔
3. سگ ماہی سے ٹھیک پہلے ، بلڈر `(lane_id, epoch, sequence)` کے ذریعہ کمٹ کرتا ہے
   ایک عصبی ہیش کو برقرار رکھنے کے لئے ، پیکٹ کو Norito کے ساتھ انکوڈ کیا گیا ہے ، اور
   `da_commitments_hash`۔
4. مکمل پیکیج WSV میں محفوظ ہے اور `SignedBlockWire` کے اندر بلاک کے ساتھ جاری کیا گیا ہے۔

اگر بلاک تخلیق ناکام ہوجاتی ہے تو ، رسیدیں قطار میں رہتی ہیں تاکہ کوشش میں قبضہ کیا جاسکے
اگلا ؛ ایک اور بلڈر `sequence` کو ریگلے حملوں سے بچنے کے لئے ہر لین کے لئے شامل کرتا ہے۔

## 3. آر پی سی اور استفسار کی سطح

Torii تین اختتامی نکات فراہم کرتا ہے:

| راستہ | طریقہ | پے لوڈ | نوٹ |
| -------- | --------- | --------- | --------- |
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (لین/ایپوچ/ترتیب فلٹرنگ ، صفحہ بندی کے ساتھ) | `DaCommitmentPage` بلاک کے کل ، وعدوں اور ہیش کی تعداد واپس کرتا ہے۔ |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (لین + منشور ہیش یا ٹپل `(epoch, sequence)`)۔ | `DaCommitmentProof` (ریکارڈ + مرکل پاتھ + بلاک ہیش) لوٹاتا ہے۔ |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | اسٹیٹ لیس مددگار بلاک ہیش اور چیک شامل کرنے کی جانچ پڑتال کرتا ہے۔ SDKs کے ذریعہ استعمال کیا جاتا ہے جو براہ راست `iroha_crypto` سے لنک نہیں کرسکتے ہیں۔ |

`iroha_data_model::da::commitment` کے تحت تمام پے لوڈ زندہ رہتے ہیں۔ Torii انسٹال کرتا ہے
ٹوکن/ایم ٹی ایل ایس پالیسیوں کو دوبارہ استعمال کرنے کے لئے ڈی اے کے موجودہ اختتامی مقامات کے ساتھ ہینڈلر۔

## 4. شمولیت اور ہلکے موکلوں کے ثبوت

- بلاک پروڈیوسر `DaCommitmentRecord` فہرست میں ایک مرکل بائنری درخت بناتا ہے
  سیریز جڑ `da_commitments_hash` کو کھانا کھلاتا ہے۔
- `DaCommitmentProof` `(sibling_hash, position)` کے ویکٹر کے ساتھ ٹارگٹ رجسٹر کو پیک کرتا ہے
  آڈیٹرز کو جڑ کی تعمیر نو کے ل .۔ ثبوتوں میں بلاک ہیش اور ہیڈر بھی شامل ہے
  دستخط شدہ تاکہ ہلکا پھلکا کلائنٹ حتمی ہونے کی تصدیق کرسکیں۔
- سی ایل آئی کمانڈز (`iroha_cli app da prove-commitment`) درخواست/تصدیق سائیکل کو نافذ کرنے میں مدد کریں
  ثبوت آپریٹرز کے لئے Norito/ہیکس کے نتائج کو ظاہر کرتے ہیں۔

## 5. اسٹوریج اور انڈیکسنگ

ڈبلیو ایس وی اسٹورز کلیدی `manifest_hash` کے ساتھ مختص کالم فیملی میں ارتکاب کرتا ہے۔ کور
سوالات سے بچنے کے لئے ثانوی اشاریہ `(lane_id, epoch)` اور `(lane_id, sequence)`
مکمل پیکیج اسکین کریں۔ ہر ریکارڈ اس بلاک کی اونچائی پر نظر رکھتا ہے جس نے اس پر مہر ثبت کردی ہے ، جس سے نوڈس کو ...
کیچ اپ مرحلہ تیزی سے بلاک کی تاریخ سے انڈیکس کی تعمیر نو کرتا ہے۔

## 6. پیمائش اور نگرانی

- `torii_da_commitments_total` بڑھتا ہے جب کم از کم ایک ریکارڈ پر مشتمل بلاک پر مہر لگ جاتی ہے۔
- `torii_da_commitment_queue_depth` رسیدیں جمع کرنے کا انتظار کر رہی ہیں (فی لین)
- Grafana `dashboards/grafana/da_commitments.json` پینل بلاک شمولیت اور گہرائی کو ظاہر کرتا ہے۔
  قطار اور تھرو پٹ ثبوت تاکہ گیٹ ویز آڈٹ کے طرز عمل کے لئے DA-3 جاری کرسکیں۔

## 7. جانچ کی حکمت عملی1. ** یونٹ ٹیسٹ ** `DaCommitmentBundle` انکوڈنگ/ڈیکوڈنگ اور ہیش مشتق کی تازہ کاریوں کے لئے
   کلسٹر
2
   معیاری اور مرکل کے ثبوت۔
3.
   دونوں نوڈس پیکٹ کے مواد اور استفسار/پروف جوابات پر متفق ہیں۔
4. ** ہلکے کلائنٹ ٹیسٹ ** `integration_tests/tests/da/commitments.rs` پر
   (زنگ) `/prove` کال کرتا ہے اور Torii سے بات کیے بغیر ثبوت کی تصدیق کرتا ہے۔
5
   تولیدی

## 8۔ لانچ پلان

| اسٹیج | تفصیل | باہر نکلنے کا معیار |
| --------- | ------- | ---------------- |
| P0 - ڈیٹا ماڈل کو ضم کریں | `DaCommitmentRecord` ، بلاک ہیڈر کی تازہ کاریوں ، اور Norito کوڈیکس کو ضم کرنا۔ | `cargo test -p iroha_data_model` نئے فکسچر کے ساتھ کامیاب ہوتا ہے۔ |
| P1 - کنیکٹ کور/WSV | قطار منطق + بلاک بلڈر کو پاس کرنا ، اشاریہ کی بچت ، آر پی سی ہینڈلرز کو بے نقاب کرنا۔ | `cargo test -p iroha_core` اور `integration_tests/tests/da/commitments.rs` بنڈل ثبوتوں کے ساتھ کامیاب ہوتا ہے۔ |
| P2 - آپریٹرز ٹولز | CLI اور Grafana بورڈ اور پروف اپ ڈیٹس کے لئے شپنگ مددگار۔ | `iroha_cli app da prove-commitment` Devnet پر چل رہا ہے ؛ پینل براہ راست ڈیٹا دکھاتا ہے۔ |
| P3 - گورننس پورٹل | `iroha_config::nexus` میں مخصوص لینوں پر DA کا ارتکاب کرنے والے بلاک کی توثیق کنندہ کو فعال کریں۔ | تازہ ترین حیثیت اور روڈ میپ DA-3 کی تکمیل کی نشاندہی کرتی ہے۔ |

## سوالات کھولیں

1.
   بلاک سائز؟ مشورہ: `kzg_commitment` اختیاری بنائیں اور اس کے ذریعے چالو کریں
   `iroha_config::da.enable_kzg`۔
2. ** تسلسل کے فرق ** - کیا ہم درجہ بندی کے فرق کی اجازت دیتے ہیں؟ موجودہ منصوبہ سوائے اس کے فرق کو مسترد کرتا ہے
   اگر آپ ہنگامی ریبوٹ کے لئے گورننس `allow_sequence_skips` کو چالو کرتے ہیں۔
3. عمل کریں
   DA-8 کے تحت لاحقہ۔

ان سوالوں کے جوابات PRS میں DA-3 کو مسودہ (اس دستاویز) سے منتقل کرتے ہیں
اسکرپٹ پر عمل درآمد شروع ہوتے ہی چل رہا ہے۔