---
lang: ur
direction: rtl
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
`docs/source/da/commitments_plan.md` کی عکاسی کرتا ہے۔ دونوں ورژن جاری رکھیں
:::

# SORA ڈیٹا کی دستیابی کے عزم کا منصوبہ Nexus (DA-3)

_ رائٹ: 2026-03-25-ذمہ دار: کور پروٹوکول ڈبلیو جی / سمارٹ کنٹریکٹ ٹیم / اسٹوریج ٹیم_

DA-3 Nexus کے بلاک فارمیٹ میں توسیع کرتا ہے تاکہ ہر لین نے رجسٹرڈ کیا
ڈی اے 2 کے ذریعہ قبول کردہ بلبوں کو بیان کرنے والے تعی .ن پسند۔ اس نوٹ نے اسے اپنی گرفت میں لے لیا
کیننیکل ڈیٹا ڈھانچے ، بلاک پائپ لائن ہکس ،
پتلی کلائنٹ اور Torii/RPC سطحیں جو اس سے پہلے لازمی طور پر اترنی ہوں گی
تصدیق کرنے والے داخلے یا توثیق کی جانچ پڑتال کے دوران ڈی اے کے وعدوں پر انحصار کرسکتے ہیں۔
گورننس Norito میں تمام پے لوڈ کو انکوڈ کیا گیا ہے۔ اسکیل یا JSON اشتہار کے بغیر
ہاک۔

## مقاصد

- ہر بلاب (chunk جڑ + منشور ہیش + عزم KZG)
  اختیاری) ہر Nexus بلاک کے اندر تاکہ ساتھی اس کی تشکیل نو کرسکیں
  لیجر سے باہر اسٹوریج سے استفسار کیے بغیر دستیابی کی حیثیت۔
- پتلی گاہکوں کو تصدیق کے ل dat ڈٹرمینسٹک ممبرشپ ٹیسٹ فراہم کریں
  کہ کسی دیئے گئے بلاک میں ایک منشور ہیش کو حتمی شکل دی گئی تھی۔
- پوسٹ سوالات Torii (`/v1/da/commitments/*`) اور ٹیسٹ جو اجازت دیتے ہیں
  ریلے ، ایس ڈی کے اور گورننس آٹومیشن آڈٹ کی دستیابی بغیر دوبارہ تیار کی
  ہر بلاک
- نئے کو تھریڈ کرتے وقت لفافہ `SignedBlockWire` کینونیکل رکھیں
  میٹا ڈیٹا ہیڈر Norito کے ذریعے ڈھانچے اور ہیش کا اخذ کرنا
  بلاک.

## دائرہ کار کا جائزہ

1. ** ڈیٹا ماڈل میں اضافہ ** `iroha_data_model::da::commitment` مزید میں
   `iroha_data_model::block` میں ہیڈر کی تبدیلیوں کو مسدود کریں۔
2.
   Torii (`crates/iroha_core/src/queue.rs` اور `crates/iroha_core/src/block.rs`)۔
3.
   فاسٹ (`iroha_core/src/wsv/mod.rs`)۔
4
   `/v1/da/commitments`۔
5. ** انضمام ٹیسٹ + فکسچر ** تار کی ترتیب اور بہاؤ کی توثیق کرنا
   `integration_tests/tests/da/commitments.rs` میں ثبوت۔

## 1. ڈیٹا ماڈل میں اضافے

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

- `KzgCommitment` `iroha_crypto::kzg` میں استعمال ہونے والے 48 بائٹ پوائنٹ کو دوبارہ استعمال کرتا ہے۔
  جب غیر حاضر رہتے ہیں تو ، یہ صرف مرکل کے ثبوتوں کی طرف لوٹتا ہے۔
- `proof_scheme` لین کیٹلاگ سے ماخوذ ہے۔ مرکل لین مسترد کردیتے ہیں
  کے زیڈ جی پے لوڈ جبکہ `kzg_bls12_381` لینوں کے لئے KZG وعدوں کی ضرورت ہوتی ہے
  صفر نہیں۔ Torii فی الحال صرف مرکل کا کمٹٹس تیار کرتا ہے اور لینوں کو مسترد کرتا ہے
  کے زیڈ جی کے ساتھ تشکیل شدہ۔
- `KzgCommitment` `iroha_crypto::kzg` میں استعمال ہونے والے 48 بائٹ پوائنٹ کو دوبارہ استعمال کرتا ہے۔
  جب مرکل میں غیر حاضر رہتے ہیں تو یہ صرف مرکل کے ثبوتوں کی طرف لوٹتا ہے۔
- `proof_digest` DA-5 PDP/POTR انضمام کی توقع کرتا ہے تاکہ وہی ریکارڈ ہوں
  بلابز کو زندہ رکھنے کے لئے استعمال ہونے والے نمونے لینے کے شیڈول کی فہرست بنائیں۔

### 1.2 بلاک ہیڈر توسیع

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```بنڈل ہیش بلاک ہیش اور بلاک میٹا ڈیٹا دونوں میں جاتا ہے۔
`SignedBlockWire`۔ جب کوئی بلاک ڈی اے ڈیٹا نہیں رکھتا ہے تو ، فیلڈ `None` رہتا ہے

نفاذ نوٹ: `BlockPayload` اور شفاف `BlockBuilder` اب
سیٹٹرز/گیٹرز کو بے نقاب کریں `da_commitments` (`BlockBuilder::set_da_commitments` دیکھیں
اور `SignedBlock::set_da_commitments`) ، لہذا میزبان ایک بنڈل جوڑ سکتے ہیں
کسی بلاک پر مہر لگانے سے پہلے پہلے سے بلٹ۔ تمام مددگار تعمیر کنندگان چھوڑ دیتے ہیں
`None` میں فیلڈ جب تک Torii اصل بنڈلوں کو تھریڈ کرتا ہے۔

### 1.3 وائر انکوڈنگ

- `SignedBlockWire::canonical_wire()` Norito ہیڈر میں شامل کرتا ہے
  `DaCommitmentBundle` ٹرانزیکشن لسٹ کے فورا بعد
  موجودہ ورژن بائٹ `0x01` ہے۔
- `SignedBlockWire::decode_wire()` ان بنڈلوں کو مسترد کرتا ہے جن کے `version` نامعلوم ہے ،
  `norito.md` میں بیان کردہ پالیسی Norito کے بعد۔
- ہیش بائی پاس کی تازہ کاری صرف `block::Hasher` پر براہ راست رہتی ہے۔
  موجودہ تار کی شکل کو ڈی کوڈ کرنے والے پتلی کلائنٹ نئے فیلڈ کو جیتتے ہیں
  خود بخود کیونکہ ہیڈر Norito اپنی موجودگی کا اعلان کرتا ہے۔

## 2۔ بلاک پروڈکشن فلو

1. Torii کے دا ingsion ایک `DaIngestReceipt` کو حتمی شکل دیتا ہے اور اسے قطار میں پوسٹ کرتا ہے
   اندرونی (`iroha_core::gossiper::QueueMessage::DaReceipt`)۔
2. `PendingBlocks` وہ تمام رسیدیں جمع کرتا ہے جن کی `lane_id` ملتی ہے
   زیر تعمیر بلاک ، `(LANE_ID ، کلائنٹ_بلوب_ایڈ ، کے ذریعہ کٹوتی کرنا ،
   منشور_ہش) `.
3. سگ ماہی سے ٹھیک پہلے ، بلڈر `(لین_ایڈ ، کے ذریعہ کمٹ کرتا ہے
   عہد ، ترتیب) he ہیش کو تعصب رکھنے کے لئے ، بنڈل کو انکوڈ کریں
   کوڈیک Norito ، اور تازہ ترین `da_commitments_hash`۔
4. پورا بنڈل WSV میں محفوظ ہے اور اس کے ساتھ ساتھ بلاک کے ساتھ خارج ہوتا ہے
   `SignedBlockWire`۔

اگر بلاک کی تخلیق ناکام ہوجاتی ہے تو ، رسیدیں اس کے لئے قطار میں رہتی ہیں
اگلی کوشش میں نے انہیں لیا۔ بلڈر آخری میں شامل ہے `sequence`
ری پلے حملوں سے بچنے کے لئے فی لین۔

## 3. آر پی سی اور استفسار کی سطح

Torii تین اختتامی نکات کو بے نقاب کرتا ہے:

| روٹ | طریقہ | پے لوڈ | نوٹ |
| ------ | -------- | --------- | ------- |
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (فلٹر بذریعہ لین/ایپوچ/تسلسل کی حد ، صفحہ بندی) | کل ، کمٹٹس ، اور بلاک ہیش کے ساتھ `DaCommitmentPage` لوٹتا ہے۔ |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (لین + منشور ہیش یا ٹپل `(epoch, sequence)`)۔ | `DaCommitmentProof` (ریکارڈ + مرکل پاتھ + بلاک ہیش) کے ساتھ جواب دیں۔ |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | اسٹیٹ لیس ہیلپر جو بلاک ہیش کے حساب کتاب کو دوبارہ پھانسی دیتا ہے اور شمولیت کی توثیق کرتا ہے۔ SDKs کے ذریعہ استعمال کیا جاتا ہے جو براہ راست `iroha_crypto` سے لنک نہیں کرسکتے ہیں۔ |

`iroha_data_model::da::commitment` کے تحت تمام پے لوڈ زندہ رہتے ہیں۔ روٹرز
Torii موجودہ ڈا ingestion اختتامی مقامات کے ساتھ ہینڈلرز کو ماؤنٹ کریں
ٹوکن/ایم ٹی ایل ایس پالیسیاں دوبارہ استعمال کریں۔

## 4. شمولیت کی جانچ اور پتلی کلائنٹ- بلاک پروڈیوسر فہرست میں بائنری مرکل کا درخت بناتا ہے
  `DaCommitmentRecord` سے سیریلائزڈ۔ جڑ `da_commitments_hash` کو کھانا کھلاتا ہے۔
- `DaCommitmentProof` پیکیجز ہدف ریکارڈ کے علاوہ ایک ویکٹر
  `(sibling_hash, position)` جڑ کو دوبارہ بنانے کے لئے تصدیق کرنے والوں کے لئے۔
  ٹیسٹوں میں بلاک ہیش اور دستخط شدہ ہیڈر بھی شامل ہیں
  پتلی کلائنٹ حتمی ہونے کی تصدیق کرتے ہیں۔
- سی ایل آئی مددگار (`iroha_cli app da prove-commitment`) لپیٹیں
  ٹیسٹ کی درخواست/توثیق اور آؤٹ پٹ Norito/ہیکس کے لئے بے نقاب کریں
  آپریٹرز

## 5. اسٹوریج اور انڈیکسنگ

ڈبلیو ایس وی ایک سرشار کیڈ کالم فیملی میں وعدوں کو اسٹور کرتا ہے
`manifest_hash`۔ ثانوی اشاریہ جات کا احاطہ `(lane_id, epoch)` اور
`(lane_id, sequence)` تاکہ سوالات پورے بنڈل کو اسکین کرنے سے گریز کریں۔
ہر ریکارڈ بلاک کی اونچائی کو ٹریک کرتا ہے جس نے اس پر مہر لگائی ، جس سے نوڈس کو اندر داخل کیا جاسکے
کوچ اپ بلاک لاگ سے فوری طور پر انڈیکس کو دوبارہ تعمیر کریں۔

## 6. ٹیلی میٹری اور مشاہدہ

- `torii_da_commitments_total` انکریمنٹس جب ایک بلاک کم از کم ایک پر مہر لگاتا ہے
  ریکارڈ.
- `torii_da_commitment_queue_depth` رسیدیں پیک ہونے کے منتظر ہیں
  (فی لین)
- ڈیش بورڈ Grafana `dashboards/grafana/da_commitments.json` دکھاتا ہے
  بلاکس ، قطار کی گہرائی اور ٹیسٹ تھرو پٹ میں شامل کرنا تاکہ
  ڈی اے 3 کی رہائی کے دروازے اس سلوک کا آڈٹ کرسکتے ہیں۔

## 7. جانچ کی حکمت عملی

1. ** یونٹ ٹیسٹ ** `DaCommitmentBundle` اور انکوڈنگ/ضابطہ کشائی کے لئے
   بلاک ہیش اخذ کردہ تازہ کاریوں کو۔
2
   بنڈل کیننیکلز اور مرکل ٹیسٹ۔
3.
   یہ ظاہر کرنا اور اس کی تصدیق کرنا کہ دونوں نوڈس بنڈل کے مواد پر متفق ہیں اور
   استفسار/ٹیسٹ کے جوابات۔
4. ** پتلی کلائنٹ ٹیسٹ ** `integration_tests/tests/da/commitments.rs` پر
   (زنگ) جو `/prove` پر کال کرتا ہے اور Torii سے بات کیے بغیر ٹیسٹ کی تصدیق کرتا ہے۔
5
   تولیدی آپریٹرز کی۔

## 8. رول آؤٹ پلان

| مرحلہ | تفصیل | باہر نکلنے کے معیارات |
| ------ | ------------- | ------------------------ |
| P0 - ڈیٹا ماڈل انضمام | `DaCommitmentRecord` ، بلاک ہیڈر کی تازہ کاریوں اور Norito کوڈیکس کو مربوط کریں۔ | نئے فکسچر کے ساتھ سبز رنگ میں `cargo test -p iroha_data_model`۔ |
| P1 - کور/WSV وائرنگ | تھریڈ قطار منطق + بلاک بلڈر ، انڈیکس کو برقرار رکھیں اور آر پی سی ہینڈلرز کو بے نقاب کریں۔ | `cargo test -p iroha_core` ، `integration_tests/tests/da/commitments.rs` بنڈل پروف دعووں کے ساتھ پاس۔ |
| P2 - آپریٹر ٹولنگ | سی ایل آئی مددگار ، Grafana ڈیش بورڈ اور پروف تصدیقی دستاویز کی تازہ کاریوں کو لانچ کریں۔ | `iroha_cli app da prove-commitment` ڈیونیٹ کے خلاف کام کرتا ہے۔ ڈیش بورڈ براہ راست ڈیٹا دکھاتا ہے۔ |
| P3 - گورننس گیٹ | اس بلاک کی توثیق کنندہ کو فعال کریں جس کے لئے `iroha_config::nexus` میں نشان زد لینوں پر DA وعدوں کی ضرورت ہوتی ہے۔ | اسٹیٹس انٹری + روڈ میپ اپ ڈیٹ مارک ڈی اے 3 کے مطابق۔ |

## سوالات کھولیں1.
   بلاک سائز کو کم کرنے کے لئے؟ تجویز: `kzg_commitment` رکھیں
   `iroha_config::da.enable_kzg` کے ذریعے اختیاری اور کرال۔
2. ** تسلسل کے فرق موجودہ منصوبہ مسترد کرتا ہے
   گیپس جب تک کہ گورننس ری پلے کے لئے `allow_sequence_skips` کو چالو نہیں کرتا ہے
   ہنگامی صورتحال
3.
   ثبوت ؛ DA-8 کے تحت پیروی زیر التوا ہے۔

ان سوالوں کے جواب کے PRs میں عمل درآمد کے DA-3 کے مسودے (یہ)
دستاویز) جب کوڈ کا کام شروع ہوتا ہے تو جاری ہے۔