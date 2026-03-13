---
lang: ur
direction: rtl
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/da/commitments_plan.md` کی عکاسی کرتا ہے۔ دونوں ورژن رکھیں
:::

# عزم پلان ڈیٹا کی دستیابی SORA Nexus (DA-3)

_ ڈرافٹ: 2026-03-25-مالکان: کور پروٹوکول ڈبلیو جی / سمارٹ کنٹریکٹ ٹیم / اسٹوریج ٹیم_

DA-3 بلاک Nexus کی شکل میں توسیع کرتا ہے تاکہ ہر لین سرایت کرے
DA-2 کے ذریعہ قبول کردہ بلبوں کی وضاحت کرنے والے اعصابی ریکارڈ۔ اس دستاویز میں
فکسڈ کیننیکل ڈیٹا ڈھانچے ، بلاک پائپ لائن ہکس ،
ہلکے کلائنٹ کے ثبوت اور Torii/RPC سطحیں جو ظاہر ہونے چاہئیں
اس سے پہلے کہ توثیق کرنے والے داخلے کے لئے ڈی اے کمٹ پر بھروسہ کرسکتے ہیں یا
مینجمنٹ چیک تمام پے لوڈ Norito- انکوڈڈ ہیں۔ اسکیل اور ایڈہاک JSON کے بغیر۔

## اہداف

- لے جانے والے بلاب (chunk روٹ + مینی فیسٹ ہیش + اختیاری kzg
  عزم) ہر بلاک کے اندر Nexus تاکہ ساتھی انجینئر کو ریورس کرسکیں
  آف لیجر اسٹوریج تک رسائی کے بغیر دستیابی کی حالت۔
- ڈٹرمینسٹک ممبرشپ کے ثبوت دیں تاکہ ہلکے کلائنٹ تصدیق کرسکیں
  کہ منشور ہیش کو ایک مخصوص بلاک میں حتمی شکل دی گئی ہے۔
- Torii درخواستیں (`/v2/da/commitments/*`) اور ثبوت ، اجازت دیں
  ریلے ، ایس ڈی کے اور آٹومیشن گورننس سب کو دوبارہ چلانے کے بغیر دستیابی چیک کریں
  بلاکس
- کیننیکل `SignedBlockWire` لفافہ محفوظ کریں ، نئے ڈھانچے کو چھوڑیں
  Norito میٹا ڈیٹا ہیڈر اور مشتق بلاک ہیش کے ذریعے۔

## دائرہ کار کا جائزہ

1. ** ڈیٹا ماڈل میں اضافے ** `iroha_data_model::da::commitment` پلس میں تبدیلیوں میں
   `iroha_data_model::block` میں ہیڈر کو بلاک کریں۔
2
   Torii (`crates/iroha_core/src/queue.rs` اور `crates/iroha_core/src/block.rs`)۔
3. ** استقامت/اشاریہ ** تاکہ WSV فوری طور پر عزم کے سوالات کا جواب دے
   (`iroha_core/src/wsv/mod.rs`)۔
4
   `/v2/da/commitments`۔
5. ** انضمام ٹیسٹ + فکسچر ** تار کی ترتیب اور پروف فلو کو چیک کرنے کے لئے
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

- `KzgCommitment` `iroha_crypto::kzg` سے 48 بائٹ پوائنٹ کو دوبارہ استعمال کرتا ہے۔
  اگر دستیاب نہیں ہے تو ، صرف مرکل کے ثبوت استعمال کریں۔
- `proof_scheme` لین ڈائرکٹری سے لیا گیا ہے۔ مرکل لینز کے زیڈ جی پے لوڈ کو مسترد کرتے ہیں ،
  اور `kzg_bls12_381` لینوں کو غیر صفر KZG وعدوں کی ضرورت ہوتی ہے۔ Torii اب
  صرف مرکل کے وعدے پیدا کرتا ہے اور KZG ترتیب کے ساتھ لینوں کو مسترد کرتا ہے۔
- `KzgCommitment` `iroha_crypto::kzg` سے 48 بائٹ پوائنٹ کو دوبارہ استعمال کرتا ہے۔
  اگر کوئی مرکل لین نہیں ہے تو ، ہم صرف مرکل کے ثبوت استعمال کرتے ہیں۔
- `proof_digest` DA-5 PDP/POTR انضمام فراہم کرتا ہے تاکہ ریکارڈنگ پر مشتمل ہو
  نمونے لینے کا شیڈول بلبس کو برقرار رکھنے کے لئے استعمال ہوتا ہے۔

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

بنڈل ہیش بلاک ہیش اور میٹا ڈیٹا `SignedBlockWire` دونوں میں شامل ہے۔ جب
اوور ہیڈ لاگت۔عمل درآمد نوٹ: `BlockPayload` اور شفاف `BlockBuilder` اب ہے
سیٹٹرز/گیٹرز `da_commitments` (`BlockBuilder::set_da_commitments` دیکھیں اور دیکھیں
`SignedBlock::set_da_commitments`) ، لہذا میزبان منسلک ہوسکتے ہیں
بلاک پر مہر لگانے سے پہلے پہلے سے جمع بنڈل۔ تمام مددگار تعمیر کنندگان
`None` فیلڈ کو چھوڑیں جب تک کہ Torii اصلی بنڈل منتقل کرنا شروع کردے۔

### 1.3 وائر انکوڈنگ

- `SignedBlockWire::canonical_wire()` Norito ہیڈر کے لئے شامل کرتا ہے
  `DaCommitmentBundle` لین دین کی فہرست کے فورا بعد۔ ورژن بائٹ `0x01`۔
- `SignedBlockWire::decode_wire()` نامعلوم `version` کے ساتھ بنڈل کو مسترد کرتا ہے ،
  Norito سے Norito پالیسی کے مطابق۔
- ہیش مشتق صرف `block::Hasher` میں اپ ڈیٹ ہوتا ہے۔ ہلکے کلائنٹ کون ہیں
  موجودہ تار کی شکل کو ڈی کوڈ کریں ، خود بخود ایک نیا فیلڈ وصول کریں ، کیونکہ
  کہ Norito ہیڈر اپنی موجودگی کا اعلان کرتا ہے۔

## 2. بلاک ریلیز کا بہاؤ

1. Torii DA انجسٹ `DaIngestReceipt` کو حتمی شکل دیتا ہے اور اسے داخلی پر شائع کرتا ہے
   قطار (`iroha_core::gossiper::QueueMessage::DaReceipt`)۔
2. `PendingBlocks` بلاک کے لئے `lane_id` کے ملاپ کے ساتھ تمام رسیدیں جمع کرتا ہے
   تعمیر ، `(lane_id, client_blob_id, manifest_hash)` کے ذریعہ کٹوتی کرنا۔
3. سگ ماہی سے پہلے ، بلڈر `(LANE_ID ، EPOCH ، کے ذریعہ وعدوں کو ترتیب دیتا ہے
   تسلسل) `ایک عصبی ہیش کے لئے ، انکوڈس بنڈل Norito کوڈیک اور
   تازہ ترین معلومات `da_commitments_hash`۔
4. مکمل بنڈل WSV میں محفوظ کیا جاتا ہے اور بلاک میں جاری کیا جاتا ہے
   `SignedBlockWire`۔

اگر بلاک تخلیق ناکام ہوجاتی ہے تو ، رسیدیں اگلے ایک کے لئے قطار میں رہتی ہیں۔
کوششیں ؛ بلڈر نے ہر لین کے لئے آخری فعال `sequence` ریکارڈ کیا ،
ری پلے حملوں کو روکنے کے لئے۔

## 3. آر پی سی اور استفسار کی سطح

Torii تین اختتامی نکات فراہم کرتا ہے:

| روٹ | طریقہ | پے لوڈ | نوٹ |
| ------- | -------- | --------- | ------- |
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (رینج فلٹر بذریعہ لین/ایپوچ/تسلسل ، صفحہ بندی) | کل گنتی ، وعدوں اور بلاک ہیش کے ساتھ `DaCommitmentPage` لوٹتا ہے۔ |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (لین + منشور ہیش یا ٹپل `(epoch, sequence)`)۔ | جوابات `DaCommitmentProof` (ریکارڈ + مرکل پاتھ + بلاک ہیش)۔ |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | اسٹیٹ لیس مددگار ، بلاک ہیش کو دوبارہ گنتی کرنا اور شامل کرنا چیک کرنا۔ `iroha_crypto` تک براہ راست رسائی کے بغیر SDKs کے لئے مفید ہے۔ |

تمام پے لوڈ `iroha_data_model::da::commitment` میں واقع ہیں۔ Torii روٹرز
دوبارہ استعمال کرنے کے لئے موجودہ ڈی اے ایجسٹ اینڈ پوائنٹس کے اگلے ماؤنٹ ہینڈلرز
ٹوکن/ایم ٹی ایل ایس پالیسیاں۔

## 4. شمولیت کے ثبوت اور ہلکے کلائنٹ

- بلاک پروڈیوسر سیریلائزڈ فہرست سے بائنری مرکل کا درخت بناتا ہے
  `DaCommitmentRecord`۔ جڑ `da_commitments_hash` کی فراہمی ہے۔
- `DaCommitmentProof` ہدف ریکارڈ اور ویکٹر `(بہن بھائی_ہش ، پیک کرتا ہے ،
  پوزیشن) `تاکہ تصدیق کرنے والے جڑ کو بحال کرسکیں۔ ثبوت میں ہیش شامل ہے
  بلاک اور دستخط شدہ ہیڈر تاکہ ہلکے کلائنٹ حتمی جانچ کرسکیں۔
- سی ایل آئی مددگار (`iroha_cli app da prove-commitment`) پروف کی درخواست/ لوپ کو لپیٹیں
  آپریٹرز کے لئے Norito/ہیکس آؤٹ پٹ کی تصدیق اور دکھائیں۔

## 5. اسٹوریج اور انڈیکسنگWSV کلید `manifest_hash` کے ساتھ ایک علیحدہ کالم فیملی میں وعدوں کو اسٹور کرتا ہے۔
ثانوی اشاریہ جات کا احاطہ `(lane_id, epoch)` اور `(lane_id, sequence)` سے
درخواستوں نے مکمل بنڈل اسکین نہیں کیا۔ ہر ریکارڈ بلاک کی اونچائی کو محفوظ کرتا ہے
جس میں اسے مہر لگا دیا گیا تھا ، جو کیچ اپ نوڈس کو جلدی سے صحت یاب ہونے دیتا ہے
بلاک لاگ سے انڈیکس۔

## 6. ٹیلی میٹری اور مشاہدہ

- جب بلاک مہر کم سے کم ہوتا ہے تو `torii_da_commitments_total` بڑھ جاتا ہے
  ایک ریکارڈ
- `torii_da_commitment_queue_depth` بنڈل کے منتظر رسیدیں
  (لین کے ذریعہ)
- Grafana ڈیش بورڈ `dashboards/grafana/da_commitments.json` تصور کرتا ہے
  بلاک ، قطار کی گہرائی اور پروف تھروپپٹ میں شامل کرنا DA-3 ریلیز آڈٹ کے لئے
  گیٹ

## 7. جانچ کی حکمت عملی

1. ** یونٹ ٹیسٹ ** انکوڈنگ/ڈیکوڈنگ `DaCommitmentBundle` اور تازہ کاریوں کے لئے
   بلاک ہیش مشتق
2
   اور مرکل کے ثبوت۔
3.
   بنڈل اور استفسار/پروف جوابات کی مستقل مزاجی۔
4. ** ہلکے کلائنٹ ٹیسٹ ** `integration_tests/tests/da/commitments.rs` (زنگ) میں ،
   `/prove` پر کال کرنا اور Torii کو کال کیے بغیر ثبوت چیک کرنا۔
5
   آپریٹر ٹولنگ۔

## 8. رول آؤٹ پلان

| مرحلہ | تفصیل | باہر نکلنے کے معیارات |
| ------- | ------------ | ---------------- |
| P0 - ڈیٹا ماڈل انضمام | `DaCommitmentRecord` ، بلاک ہیڈر اور Norito کوڈیکس اپڈیٹس کو قبول کریں۔ | نئے فکسچر کے ساتھ `cargo test -p iroha_data_model` گرین۔ |
| P1 - کور/WSV وائرنگ | مسلسل قطار + بلاک بلڈر منطق ، اشاریہ جات اور اوپن آر پی سی ہینڈلرز کو محفوظ کریں۔ | `cargo test -p iroha_core` ، `integration_tests/tests/da/commitments.rs` پاس بنڈل پروف چیک۔ |
| P2 - آپریٹر ٹولنگ | پروف تصدیق کے ل CL CLI مددگار ، Grafana ڈیش بورڈ اور دستاویزات کی تازہ کاریوں کو انسٹال کریں۔ | `iroha_cli app da prove-commitment` devnet پر چلتا ہے ؛ ڈیش بورڈ براہ راست ڈیٹا دکھاتا ہے۔ |
| P3 - گورننس گیٹ | بلاک کی توثیق کنندہ کو فعال کریں ، جس کے لئے `iroha_config::nexus` میں نشان زد لینوں پر DA وعدوں کی ضرورت ہے۔ | اسٹیٹس انٹری + روڈ میپ اپ ڈیٹ DA-3 کو مکمل طور پر نشان زد کرتا ہے۔ |

## سوالات کھولیں

1.
   بلاک سائز کو کم کرنے کے لئے کے زیڈ جی کے وعدے؟ تجویز: چھوڑ دو
   `kzg_commitment` اختیاری اور گیٹنگ `iroha_config::da.enable_kzg` کے ذریعے۔
2. ** تسلسل کے فرق ** - کیا ترتیب کے وقفے کی اجازت دی جانی چاہئے؟ موجودہ منصوبہ
   جب تک کہ گورننس `allow_sequence_skips` کے قابل نہ بنائے
   ایمرجنسی ری پلے۔
3.
   ڈی اے 8 کے تحت مزید کام۔

ان سوالوں کے جوابات PRS میں DA-3 کو "مسودہ" کی حیثیت سے نکال دے گا
(یہ دستاویز) کوڈنگ کے کام کے آغاز کے بعد "پیشرفت" میں۔