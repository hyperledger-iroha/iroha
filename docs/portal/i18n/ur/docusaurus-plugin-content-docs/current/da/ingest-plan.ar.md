---
lang: ur
direction: rtl
source: docs/portal/docs/da/ingest-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ معیاری ماخذ
`docs/source/da/ingest_plan.md` کی عکاسی کرتا ہے۔ چیک آؤٹ ہونے تک دونوں کاپیاں مطابقت پذیری میں رکھیں
پرانی دستاویزات۔
:::

# SORA Nexus میں ڈیٹا کی دستیابی کا منصوبہ تیار کریں

_ ڈرافٹ: 02-20-2026-مالک: کور پروٹوکول WG / اسٹوریج ٹیم / DA WG_

ڈی اے 2 روٹ Torii پلیٹ فارم میں توسیع کرتا ہے جس میں بلبس کے لئے ایک Ingest انٹرفیس ہے جو Norito میٹا ڈیٹا برآمد کرتا ہے۔
بوائی کا اعادہ SoraFS ہے۔ اس دستاویز میں مجوزہ اسکیما ، API کی سطح ، اور توثیق کے بہاؤ کو دستاویز کیا گیا ہے
پھانسی باقی DA-1 تخروپن کا انتظار کیے بغیر ترقی کرتی ہے۔ تمام فارمیٹس کو استعمال کرنا چاہئے
پے لوڈ انکوڈنگز Norito ؛ سیرڈ/JSON کو کسی فال بیک کی اجازت نہیں ہے۔

## مقاصد

- لازمی طور پر بڑے بلبس (تائکائی سیکٹر ، لین سائڈیکارس ، اور گورننس ٹولز) کو قبول کریں
  Torii کے ذریعے۔
- بلاب ، کوڈیک پیرامیٹرز ، اور مٹانے والی فائل کو بیان کرنے والے معیاری Norito مینی فیسٹ تیار کریں۔
  اور برقرار رکھنے کی پالیسی۔
- SoraFS گرم اسٹوریج میں حصوں کو میٹا ڈیٹا محفوظ کریں اور نقل کے کاموں میں ڈالیں
  قطار
- SoraFS رجسٹری اور گورننس مانیٹر پر پن کے ارادے + پالیسی ٹیگز شائع کریں۔
- قبولیت کی رسیدوں کو قابل بنائیں تاکہ صارفین اشاعت کا قطعی ثبوت واپس آجائیں۔

## API سطح (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

پے لوڈ `DaIngestRequest` Norito کے ساتھ انکوڈڈ ہے۔ جوابات استعمال کیے جاتے ہیں
`application/norito+v1` اور `DaIngestReceipt` واپس کرتا ہے۔

| جواب | مطلب |
| --- | --- |
| 202 قبول شدہ | بلاب ہیش/نقل کی قطار میں رکھا گیا ہے۔ رسید۔ |
| 400 بری درخواست | اسکیما/سائز کی خلاف ورزی (توثیق کی جانچ پڑتال دیکھیں)۔ |
| 401 غیر مجاز | گمشدہ/غلط API ٹوکن۔ |
| 409 تنازعہ | مماثل میٹا ڈیٹا کے ساتھ `client_blob_id` کی نقل۔ |
| 413 پے لوڈ بہت بڑا | تشکیل شدہ بلاب کی لمبائی کی حد سے تجاوز کرتا ہے۔ |
| 429 بہت ساری درخواستیں | شرح کی حد تک پہنچ گئی۔ |
| 500 داخلی غلطی | غیر متوقع ناکامی (لاگ + الرٹ) |

## تجویز کردہ Norito اسکیم

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Norito lane
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

> ایگزیکٹو نوٹ: ان پے لوڈ کے لئے معیاری زنگ کی نمائندگی کو منتقل کردیا گیا ہے
> `iroha_data_model::da::types` ، درخواست/رسید کے لئے ریپر کے ساتھ
> `iroha_data_model::da::ingest` اور `iroha_data_model::da::manifest` میں ظاہر ڈھانچہ۔

`compression` فیلڈ نے اعلان کیا کہ کال کرنے والوں نے پے لوڈ کو کس طرح تیار کیا۔ Torii ، `identity` اور `gzip` کو قبول کرتا ہے۔
`deflate` اور `zstd` سے پہلے سے ہیشنگ ڈیکمپریشن کے ساتھ ، ہیشنگ اور ظاہر ہوتا ہے توثیق کرتا ہے
انتخاب

### توثیق چیک لسٹ

1. تصدیق کریں کہ درخواست ہیڈر Norito `DaIngestRequest` سے ملتی ہے۔
2. اگر `total_size` قانونی پے لوڈ کی لمبائی (ڈیکمپریشن کے بعد) سے مختلف ہے تو ناکام ہوجائیں یا
   زیادہ سے زیادہ جزو سے زیادہ ہے۔
3. فورس سیدھ `chunk_size` (دو کی طاقت ،  = 2۔
5. `retention_policy.required_replica_count` کو گورننس بیس لائن کا احترام کرنا چاہئے۔
6. معیاری ہیش (دستخطی فیلڈ کو چھوڑ کر) کے خلاف دستخط کی تصدیق کریں۔
7. ڈپلیکیٹ `client_blob_id` کو مسترد کریں جب تک کہ پے لوڈ ہیش ویلیو اور میٹا ڈیٹا
   ایک جیسی
8. جب `norito_manifest` فراہم کیا جاتا ہے تو ، تصدیق کریں کہ واپس ہونے والے مینی فیسٹ میچوں کا اسکیما + ہیش
   ہیشنگ کے بعد اس کا حساب لگائیں ؛ بصورت دیگر ، نوڈ ایک منشور پیدا کرتا ہے اور اسے اسٹور کرتا ہے۔
9. تشکیل شدہ بے کار پالیسی کا اطلاق کریں: Torii `RetentionPolicy` کے ذریعے بھیجے گئے
   `torii.da_ingest.replication_policy` (`replication-policy.md` دیکھیں) اور مسترد کریں
   اگر برقرار رکھنے کا ڈیٹا نافذ فائل سے مماثل نہیں ہے تو پری پیج کا اظہار کیا جاتا ہے۔

### ٹکڑے ٹکڑے اور تکرار کا بہاؤ1. ہیش پے لوڈ کو `chunk_size` میں ، اور ہر ایک حصہ + مرکل کی جڑ کے لئے بلیک 3 کا حساب لگائیں۔
2. Norito `DaManifestV1` (نیا ڈھانچہ) بنائیں جو حصہ حاصل کرتا ہے
   (کردار/گروپ_ایڈ) ، اور مٹانے والی ترتیب (قطار اور کالموں کی برابری کا قیام
   `ipa_commitment`) ، برقرار رکھنے کی پالیسی اور میٹا ڈیٹا۔
3. `config.da_ingest.manifest_store_dir` کے تحت معیاری مینی فیسٹ بائٹس رکھیں
   .
   تاکہ SoraFS سسٹم اسے نگل سکے اور اسٹوریج ٹکٹ کو محفوظ کردہ ڈیٹا سے جوڑ سکے۔
4. گورننس اور پالیسی ٹیگ کے ساتھ `sorafs_car::PinIntent` کے ذریعے پوسٹ پن کے ارادے۔
5. نشریاتی واقعہ Norito `DaIngestPublished` مبصرین کو مطلع کرنے کے لئے (ہلکے کلائنٹ ، گورننس ،
   تجزیات)۔
6. `DaIngestReceipt` کو کال کرنے والے کو واپس کریں (سروس کی کلید کے ساتھ واقع ہے Torii DA) اور ہیڈر بھیجیں
   `Sora-PDP-Commitment` تاکہ SDKs فوری طور پر خفیہ کردہ کمٹ کو منتخب کریں۔ رسید بھی شامل ہے
   اب `rent_quote` (Norito `DaRentQuote`) اور `stripe_layout` قابل بنانے کے لئے
   بھیجنے والے بیس کرایہ ، ریزرو شیئر ، اور PDP/POTR انعام کی توقعات دیکھ سکتے ہیں
   اور فنڈز کا ارتکاب کرنے سے پہلے اسٹوریج ٹکٹ کے آگے 2 ڈی مٹانے کی منصوبہ بندی۔

## اسٹوریج/لاگ اپ ڈیٹ

- `sorafs_manifest` کو `DaManifestV1` کے ساتھ بڑھاؤ تاکہ تشخیصی تجزیہ کو قابل بنایا جاسکے۔
- ریکارڈ `da.pin_intent` میں ایک نیا ندی شامل کریں جس میں ایک ورژن والے پے لوڈ کے ساتھ ہیش کی نشاندہی ہوتی ہے۔
  منشور + ٹکٹ ID۔
- وقت ، ہیش تھرو پٹ ، اور بیک بلاگ کو ٹریک کرنے کے لئے نوٹ لائنوں کو اپ ڈیٹ کریں
  تکرار اور ناکامیوں کی تعداد۔

## ٹیسٹ کی حکمت عملی

- اسکیما کی توثیق ، ​​دستخطی چیک ، اور ڈپلیکیٹ کا پتہ لگانے کے لئے یونٹ ٹیسٹ۔
- `DaIngestRequest` کے لئے Norito انکوڈنگ کی تصدیق کرنے کے لئے گولڈن ٹیسٹ ، ظاہر اور وصول کریں۔
- کنٹرول انضمام ایک جعلی SoraFS + رجسٹری چلاتا ہے ، اور chunk + پن ندیوں کی تصدیق کرتا ہے۔
- پراپرٹیز ٹیسٹ مٹانے والی فائلوں اور بے ترتیب برقراری کے امتزاج کو ڈھکنے والے۔
- خراب میٹا ڈیٹا سے بچانے کے لئے Norito پے لوڈ کو فوزنگ کرنا۔

## CLI اور SDK ٹولز (DA-8)- `iroha app da submit` (نیا CLI انٹری) اب مشترکہ بلڈر/پبلشر انجسٹ کو لپیٹتا ہے
  لہذا آپریٹر تائکائی بنڈل کے راستے سے باہر بے ترتیب بلب داخل کرسکتے ہیں۔ زندہ
  `crates/iroha_cli/src/commands/da.rs:1` پر کمانڈ ایک پے لوڈ اور فائل کا استعمال کرتے ہیں
  دستخط کرنے سے پہلے مٹانے/برقرار رکھنے اور اختیاری میٹا ڈیٹا/مینی فیسٹ فائلیں
  معیاری `DaIngestRequest` CLI کنفیگریشن کلید کے ساتھ۔ کامیاب رنز بچ گئے ہیں
  `da_request.{norito,json}` اور `da_receipt.{norito,json}` کے تحت
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` پر اوور رائڈ) ترتیب میں
  نوادرات کا عین مطابق بائٹس ورژن Norito ریکارڈ کرتا ہے جو ingest کے دوران استعمال ہوتا ہے۔
- کمانڈ `client_blob_id = blake3(payload)` کو بطور ڈیفالٹ استعمال کرتا ہے ، لیکن یہ اوور رائڈس کو قبول کرتا ہے
  `--client-blob-id` کے ذریعے ، JSON میٹا ڈیٹا کے نقشوں (`--metadata-json`) اور کا احترام کرتا ہے
  تیار منشور (`--manifest`) ، `--no-submit` آف لائن تیاری کے لئے معاون ہے
  `--endpoint` Torii کے لئے سرشار میزبانوں کے لئے۔ اس کے علاوہ JSON JSON کو STDOUT پر پرنٹ کرتا ہے
  اسے ڈسک پر لکھنا ، جو DA-8 میں "جمع کرانے والا" ٹولنگ کی ضرورت کو بند کردیتا ہے اور نوکری کھولتا ہے
  SDK parity.
- `iroha app da get` پہلے سے چلنے والے ملٹی سورس پلیئر کے ڈی اے کے لئے ایک ویکٹر عرف شامل کرتا ہے
  `iroha app sorafs fetch`۔ آپریٹرز اس کی نشاندہی کرتے ہیں کہ وہ + chunk-plan کے منشور کی طرف اشارہ کرسکتے ہیں
  (`--manifest`, `--plan`, `--manifest-id`) **or** pass storage ticket from Torii
  `--storage-ticket` کے ذریعے۔ جب ٹکٹ کا راستہ استعمال کرتے ہو تو ، سی ایل آئی اس سے ظاہر ہوتا ہے
  `/v2/da/manifests/<ticket>` ، اور پیکیج `artifacts/da/fetch_<timestamp>/` کے تحت محفوظ ہے
  ۔
  اس کے بعد آپ دیئے گئے فہرست `--gateway-provider` کے ساتھ آرکسٹریٹر چلاتے ہیں۔ تمام نوبس باقی ہیں
  ایڈوانسڈ SoraFS جیسا کہ ہے (مینی فیسٹ لفافے ، کسٹمر لیبل ، گارڈ
  کیچز ، اوور رائڈس ، گمنام منتقلی ، اسکور بورڈ برآمدات ، اور `--output` راستے) ، اور کر سکتے ہیں
  `--manifest-endpoint` کے ذریعے منشور اختتامی نقطہ کو کسٹم میزبان Torii کے ذریعے تبدیل کریں ، لہذا
  آخری سے آخر تک دستیابی کی جانچ پڑتال `da` جگہ کے اندر رہتی ہے بغیر منطق کو دہرائے
  آرکیسٹریٹر۔
- `iroha app da get-blob` Torii سے براہ راست معیاری ظاہر ہوتا ہے
  `GET /v2/da/manifests/{storage_ticket}`۔ وہ کمانڈ لکھتا ہے
  `manifest_{ticket}.norito` ، `manifest_{ticket}.json` ، اور `chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/` (یا صارف کی وضاحت `--output-dir`) کے ساتھ
  عین مطابق `iroha app da get` آرڈر (بشمول `--manifest-id`) لانے کے لئے درکار ہے
  آرکیسٹریٹر۔ اس سے آپریٹرز کو منشور اسپل کے ثبوتوں سے دور رہتا ہے اور اس بات کو یقینی بناتا ہے کہ ...
  فیچر ہمیشہ Torii کے ذریعہ جاری کردہ دستخط شدہ نوادرات کا استعمال کرتا ہے۔ کلائنٹ آئینے Torii
  جاوا اسکرپٹ میں یہ بہاؤ `ToriiClient.getDaManifest(storageTicketHex)` کے ذریعے ہے ،
  یہ ایس ڈی کے میں کال کرنے والوں کے لئے پیکڈ بائٹس Norito ، منشور JSON ، اور کال کرنے والوں کے لئے منصوبہ بندی کرتا ہے۔
  یہ سی ایل آئی کا استعمال کیے بغیر آرکسٹریٹر سیشن شروع کرتا ہے۔ سوئفٹ ایس ڈی کے اب وہی دکھاتا ہے
  سطحیں (`ToriiClient.getDaManifestBundle(...)` کے ساتھ
  `fetchDaPayloadViaGateway(...)`) ، آرکسٹریٹر شیل SoraFS پر پیکٹوں کی ہدایت کرنا۔
  آبائی لہذا iOS کلائنٹ منشور کو ڈاؤن لوڈ کرسکتے ہیں اور ملٹی سورس کی بازیافت پر عملدرآمد کرسکتے ہیں
  سی ایل آئی کو فون کیے بغیر شواہد پر قبضہ کریں۔
  .
- `iroha app da rent-quote` ذخیرہ کرنے والے حجم اور برقرار رکھنے والی ونڈو کے لئے عین مطابق کرایہ اور درزیوں کی ترغیبات کا حساب لگاتا ہے۔
  تعارف۔ مددگار فعال (JSON یا بائٹس Norito) یا ڈیفالٹ `DaRentPolicyV1` کھاتا ہے۔
  بلٹ ان ، پالیسی کی جانچ پڑتال کرتا ہے اور JSON کا خلاصہ (`gib` ، `months` ، پالیسی ڈیٹا ، پرنٹ کرتا ہے ،اور `DaRentQuote` فیلڈز) تاکہ آڈیٹر عین مطابق XOR گراف کا حوالہ دے سکیں
  کسٹم ٹیکسٹس کے بغیر گورننس منٹ۔ کمانڈ ایک لائن سمری بھی جاری کرتا ہے
  `rent_quote ...` JSON پے لوڈ سے پہلے ٹرمینل لاگز کی پڑھنے کی اہلیت کو برقرار رکھنے کے لئے
  حادثے کی مشقیں۔ جوڑی `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  `--policy-label "governance ticket #..."` کے ساتھ فارمیٹڈ نوادرات کو بچانے کے لئے حوالہ دیں
  پالیسی ووٹ یا ابتدائی پیکیج ؛ سی ایل آئی کسٹم ٹیگ کو تراشتا ہے اور خالی ڈوروں کو مسترد کرتا ہے
  لہذا `policy_source` اقدار ٹریژری بورڈز میں قابل عمل رہیں۔ دیکھو
  `crates/iroha_cli/src/commands/da.rs` سب کامنڈ اور کے لئے
  پالیسی اسکیما کے لئے `docs/source/da/rent_policy.md`۔
  .
- `iroha app da prove-availability` مذکورہ بالا سب کو جوڑتا ہے: اسٹوریج کا ٹکٹ لیتا ہے ، ایک پیکٹ ڈاؤن لوڈ کرتا ہے
  منشور معیاری ، ملٹی سورس آرکسٹریٹر چل رہا ہے (`iroha app sorafs fetch`) بمقابلہ
  فہرست `--gateway-provider` دی گئی ، ڈاؤن لوڈ شدہ پے لوڈ + اسکور بورڈ کو محفوظ کرتا ہے
  `artifacts/da/prove_availability_<timestamp>/` کے تحت ، یہ براہ راست POR HELPER کہتا ہے
  موجودہ (`iroha app da prove`) بھری ہوئی بائٹس کا استعمال کرتے ہوئے۔ آپریٹرز نوبس کو ایڈجسٹ کرسکتے ہیں
  آرکسٹریٹر (`--max-peers` ، `--scoreboard-out` ، اوور رائڈس مینی فیسٹ ایڈریس) اور
  تیار کرتے وقت ثبوت کے لئے نمونے (`--sample-count` ، `--leaf-index` ، `--sample-seed`)
  DA-5/DA-9 آڈٹ کے لئے درکار ایک نوادرات: پے لوڈ کی کاپی ، دستی
  اسکور بورڈ ، اور JSON پروف خلاصہ۔