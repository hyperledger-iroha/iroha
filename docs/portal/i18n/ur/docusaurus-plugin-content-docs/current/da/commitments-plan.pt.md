---
lang: ur
direction: rtl
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
آئینہ `docs/source/da/commitments_plan.md`۔ دونوں ورژن کو اندر رکھیں
:::

# SORA ڈیٹا کی دستیابی کے عزم کا منصوبہ Nexus (DA-3)

_ رائٹ: 2026-03-25-ذمہ دار: کور پروٹوکول ڈبلیو جی / سمارٹ کنٹریکٹ ٹیم / اسٹوریج ٹیم_

DA-3 Nexus بلاک فارمیٹ میں توسیع کرتا ہے تاکہ ہر لین میں ریکارڈ شامل ہوں
DA-2 کے ذریعہ قبول کردہ بلبوں کو بیان کرنے والے اعداد و شمار کا اعداد و شمار۔ اس نوٹ نے اسے اپنی گرفت میں لے لیا
کیننیکل ڈیٹا ڈھانچے ، بلاک پائپ لائن ہکس ، ثبوت
لائٹ کلائنٹ اور Torii/RPC سطحیں جن کو پہلے پہنچنے کی ضرورت ہے
تصدیق کرنے والے داخلے یا سیکیورٹی چیک کے دوران ڈی اے کے وعدوں پر بھروسہ کرسکتے ہیں۔
گورننس Norito میں تمام پے لوڈ کو انکوڈ کیا گیا ہے۔ اسکیل یا JSON اشتہار کے بغیر
ہاک۔

## مقاصد

۔
  اختیاری) ہر Nexus بلاک کے اندر تاکہ ہم عمر ریاست کی تشکیل نو کرسکیں
  لیجر سے باہر اسٹوریج سے مشورہ کیے بغیر دستیابی کا۔
- صارفین کو لینے کے ل det ڈٹرمینسٹک ممبرشپ کے ثبوت فراہم کریں
  تصدیق کریں کہ کسی بلاک میں ایک منشور ہیش کو حتمی شکل دی گئی ہے۔
- سوالات کو بے نقاب کریں Torii (`/v1/da/commitments/*`) اور ایسے ثبوت جو ریلے کی اجازت دیتے ہیں ،
  ہر بلاک کو دوبارہ چلائے بغیر ایس ڈی کے اور گورننس آٹومیشن آڈٹ کی دستیابی۔
- نئے ڈھانچے کو تھریڈ کرتے وقت کیننیکل `SignedBlockWire` لفافہ برقرار رکھیں
  میٹا ڈیٹا ہیڈر Norito اور بلاک ہیش کی اخذ کے ذریعہ۔

## دائرہ کار کا جائزہ

1. ** ڈیٹا ماڈل میں اضافے ** `iroha_data_model::da::commitment` میں مزید تبدیلیاں
   `iroha_data_model::block` میں بلاک ہیڈر کا۔
2.
   Torii (`crates/iroha_core/src/queue.rs` اور `crates/iroha_core/src/block.rs`)۔
3
   جلدی (`iroha_core/src/wsv/mod.rs`)۔
4
   `/v1/da/commitments`۔
5. ** انضمام ٹیسٹ + فکسچر ** تار کی ترتیب اور پروف فلو کی توثیق کرنا
   `integration_tests/tests/da/commitments.rs` پر۔

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

- `KzgCommitment` `iroha_crypto::kzg` میں استعمال ہونے والے 48 بائٹ ڈاٹ کو دوبارہ استعمال کرتا ہے۔
  جب غیر حاضر رہتے ہیں تو ، یہ صرف مرکل کے ثبوتوں پر پڑتا ہے۔
- `proof_scheme` لین کیٹلاگ سے ماخوذ ہے۔ مرکل لینز کے زیڈ جی پے لوڈ کو مسترد کرتے ہیں
  جبکہ `kzg_bls12_381` لینوں کو غیر صفر KZG وعدوں کی ضرورت ہوتی ہے۔ Torii فی الحال
  صرف مرکل کے وعدے پیدا کرتا ہے اور KZG کے ساتھ تشکیل شدہ لینوں کو مسترد کرتا ہے۔
- `KzgCommitment` `iroha_crypto::kzg` میں استعمال ہونے والے 48 بائٹ ڈاٹ کو دوبارہ استعمال کرتا ہے۔
  جب مرکل لین سے غیر حاضر رہتے ہیں تو ، یہ صرف مرکل کی آزمائشوں میں گر جاتا ہے۔
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
```

بنڈل ہیش بلاک ہیش اور میٹا ڈیٹا دونوں میں شامل ہے
`SignedBlockWire`۔ جب بلاک ڈی اے ڈیٹا نہیں رکھتا ہے تو ، فیلڈ `None` ہےنفاذ نوٹ: `BlockPayload` `BlockBuilder` شفاف ہے
سیٹٹرز/گیٹرز کو بے نقاب کریں `da_commitments` (`BlockBuilder::set_da_commitments` دیکھیں
اور `SignedBlock::set_da_commitments`) ، پھر میزبان ایک بنڈل جوڑ سکتے ہیں
کسی بلاک پر مہر لگانے سے پہلے پری بلٹ۔ تمام مددگار `None` پر فیلڈ چھوڑ دیتے ہیں
جب تک Torii زنجیروں کو اصلی بنڈل نہیں بناتا ہے۔

### 1.3 وائر انکوڈنگ

- `SignedBlockWire::canonical_wire()` Norito ہیڈر میں شامل کرتا ہے
  `DaCommitmentBundle` موجودہ ٹرانزیکشن لسٹ کے فورا بعد۔ اے
  ورژن بائٹ `0x01` ہے۔
- `SignedBlockWire::decode_wire()` ان بنڈلوں کو مسترد کرتا ہے جن کے `version` ہے
  `norito.md` میں بیان کردہ پالیسی Norito کے ساتھ منسلک ، نامعلوم۔
- ہیش مشتق کی تازہ کارییں صرف `block::Hasher` میں موجود ہیں۔ صارفین
  ہلکا پھلکا آلات جو موجودہ تار کی شکل کو ڈی کوڈ کرتے ہیں وہ نیا فیلڈ حاصل کرتے ہیں
  خود بخود کیونکہ Norito ہیڈر اپنی موجودگی کا اعلان کرتا ہے۔

## 2۔ بلاک پروڈکشن فلو

1. Torii کی داجشن `DaIngestReceipt` کو حتمی شکل دیتی ہے اور اسے قطار میں شائع کرتی ہے
   اندرونی (`iroha_core::gossiper::QueueMessage::DaReceipt`)۔
2. `PendingBlocks` وہ تمام رسیدیں جمع کرتا ہے جن کی `lane_id` بلاک سے ملتی ہے
   زیر تعمیر ، `(lane_id, client_blob_id, manifest_hash)` کے ذریعہ کٹوتی کرنا۔
3. سگ ماہی سے ٹھیک پہلے ، بلڈر تقرریوں کو `(LANE_ID ، EPOCH ، کے ذریعہ ترتیب دیتا ہے
   ترتیب) `ڈٹرمینسٹک ہیش کو برقرار رکھنے کے لئے ، کوڈیک کے ساتھ بنڈل کو انکوڈ کریں
   Norito ، اور تازہ ترین معلومات `da_commitments_hash`۔
4. مکمل بنڈل WSV میں محفوظ ہے اور اس کے ساتھ ساتھ بلاک کے ساتھ جاری کیا جاتا ہے
   `SignedBlockWire`۔

اگر بلاک تخلیق ناکام ہوجاتی ہے تو ، رسیدیں اگلے کے لئے قطار میں رہتی ہیں

ان پر قبضہ کرنے کی کوشش کریں۔ بلڈر نے لین کے ذریعہ شامل آخری `sequence` ریکارڈ کیا ہے
ری پلے حملوں کو روکنے کے لئے۔

## 3. سطح آر پی سی اور سوالات

Torii تین اختتامی نکات کو بے نقاب کرتا ہے:

| روٹ | طریقہ | پے لوڈ | نوٹ |
| ------ | -------- | --------- | ------- |
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (رینج فلٹر بذریعہ لین/ایپوچ/تسلسل ، صفحہ بندی) | کل ، وعدوں اور بلاک ہیش کے ساتھ `DaCommitmentPage` واپس کرتا ہے۔ |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (لین + منشور ہیش یا ٹپل `(epoch, sequence)`)۔ | `DaCommitmentProof` (ریکارڈ + مرکل پاتھ + بلاک ہیش) کے ساتھ جواب دیتا ہے۔ |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | اسٹیٹ لیس مددگار جو بلاک ہیش کو دوبارہ گنتی کرتا ہے اور اس میں شامل ہونے کی توثیق کرتا ہے۔ SDKs کے ذریعہ استعمال کیا جاتا ہے جو براہ راست `iroha_crypto` سے لنک نہیں کرسکتے ہیں۔ |

`iroha_data_model::da::commitment` کے تحت تمام پے لوڈ زندہ رہتے ہیں۔ روٹرز
Torii ماؤنٹ ہینڈلر کے ساتھ ساتھ موجودہ دا انیس اینڈ پوائنٹس کے ساتھ
ٹوکن/ایم ٹی ایل ایس پالیسیاں دوبارہ استعمال کریں۔

## 4. شمولیت ٹیسٹ اور ہلکے صارفین- بلاک پروڈیوسر فہرست میں بائنری مرکل کا درخت بناتا ہے
  `DaCommitmentRecord` سے سیریلائزڈ۔ جڑ `da_commitments_hash` کو کھانا کھلاتا ہے۔
- `DaCommitmentProof` پیکیجز ہدف ریکارڈ کے علاوہ ایک ویکٹر
  `(sibling_hash, position)` جڑ کو دوبارہ تعمیر کرنے کے لئے تصدیق کرنے والوں کے لئے۔
  ثبوتوں میں بلاک ہیش اور دستخط شدہ ہیڈر بھی شامل ہیں تاکہ مؤکل
  ہلکا پھلکا مقصد کی توثیق کرتا ہے۔
- CLI مددگار (`iroha_cli app da prove-commitment`) شامل ہے
  آپریٹرز کو Norito/ہیکس آؤٹ پٹ کی درخواست/تصدیق اور بے نقاب کریں۔

## 5. اسٹوریج اور انڈیکسنگ

ڈبلیو ایس وی ایک سرشار کالم فیملی میں تقرریوں کو کلید کے ساتھ اسٹور کرتا ہے
`manifest_hash`۔ ثانوی اشاریہ جات کا احاطہ `(lane_id, epoch)` اور
`(lane_id, sequence)` تاکہ سوالات پورے بنڈل کو اسکین کرنے سے گریز کریں۔ ہر ایک
ریکارڈ اس بلاک کی اونچائی کو ٹریک کرتا ہے جس نے اس پر مہر لگائی ، جس سے ہمیں پکڑنے کی اجازت ملتی ہے
بلاک لاگ سے جلدی سے انڈیکس کو دوبارہ تعمیر کریں۔

## 6. ٹیلی میٹری اور مشاہدہ

- `torii_da_commitments_total` انکریمنٹس جب ایک بلاک کم از کم ایک پر مہر لگاتا ہے
  ریکارڈ.
- `torii_da_commitment_queue_depth` رسیدیں بنڈل کا انتظار کر رہی ہیں (کے لئے
  لین)۔
- ڈیش بورڈ Grafana `dashboards/grafana/da_commitments.json` دکھاتا ہے
  بلاکس ، قطار کی گہرائی اور پروف ان پٹ میں شامل کرنا تاکہ
  ڈی اے 3 کی رہائی کے دروازے سلوک کا آڈٹ کرسکتے ہیں۔

## 7. جانچ کی حکمت عملی

1. ** یونٹ ٹیسٹ ** `DaCommitmentBundle` اور انکوڈنگ/ضابطہ کشائی کے لئے
   بلاک ہیش اخذ کردہ تازہ کاریوں کو۔
2
   بنڈل اور مرکل کے ثبوتوں کا۔
3.
   چیک کرنا کہ ہم دونوں بنڈل اور جوابات کے مواد پر متفق ہیں
   استفسار/ثبوت
4. ** ہلکے کلائنٹ ٹیسٹ ** `integration_tests/tests/da/commitments.rs` پر
   (زنگ) جو `/prove` پر کال کرتا ہے اور Torii سے بات کیے بغیر ثبوت کی جانچ پڑتال کرتا ہے۔
5
   تولیدی آپریٹرز۔

## 8. رول آؤٹ پلان

| مرحلہ | تفصیل | باہر نکلنے کے معیارات |
| ------ | ----------- | --------------------- |
| P0 - ڈیٹا ماڈل انضمام | `DaCommitmentRecord` ، بلاک ہیڈر کی تازہ کاریوں اور Norito کوڈیکس کو مربوط کریں۔ | نئے فکسچر کے ساتھ `cargo test -p iroha_data_model` گرین۔ |
| P1 - وائرنگ کور/WSV | چین قطار منطق + بلاک بلڈر ، انڈیکس کو برقرار رکھیں اور آر پی سی ہینڈلرز کو بے نقاب کریں۔ | `cargo test -p iroha_core` ، `integration_tests/tests/da/commitments.rs` بنڈل پروف کے دعوے کے ساتھ پاس کریں۔ |
| P2 - آپریٹر ٹولنگ | سی ایل آئی مددگار ، Grafana ڈیش بورڈ اور پروف تصدیقی دستاویز کی تازہ کاریوں کی تازہ کاری کریں۔ | `iroha_cli app da prove-commitment` ڈیونیٹ کے خلاف کام کرتا ہے۔ ڈیش بورڈ براہ راست ڈیٹا دکھاتا ہے۔ |
| P3 - گورننس گیٹ | اس بلاک کی توثیق کنندہ کو فعال کریں جس کے لئے `iroha_config::nexus` میں نشان زد لینوں میں DA وعدوں کی ضرورت ہوتی ہے۔ | اسٹیٹس انٹری + روڈ میپ اپ ڈیٹ DA-3 کو مکمل طور پر نشان زد کرتا ہے۔ |

## سوالات کھولیں1.
   بلاک سائز کو کم کرنے کے لئے چھوٹا؟ تجویز: `kzg_commitment` رکھیں
   اختیاری اور گیٹ `iroha_config::da.enable_kzg` کے ذریعے۔
2. ** تسلسل کے فرق موجودہ منصوبہ خلا کو مسترد کرتا ہے
   جب تک کہ انتظامیہ ہنگامی ری پلے کے لئے `allow_sequence_skips` کو چالو نہ کرے۔
3.
   DA-8 میں زیر التواء۔

ان سوالوں کے جوابات پر عمل درآمد PRS میں DA-3 کو مسودہ سے منتقل کرتا ہے (یہ)
دستاویز) جب کوڈ کا کام شروع ہوتا ہے تو جاری ہے۔