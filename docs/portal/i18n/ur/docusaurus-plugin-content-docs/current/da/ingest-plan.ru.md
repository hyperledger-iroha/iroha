---
lang: ur
direction: rtl
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/da/ingest_plan.md` کی عکاسی کرتا ہے۔ دونوں ورژن رکھیں
:::

# منصوبہ بندی کریں ڈیٹا کی دستیابی SORA Nexus

_ ڈرافٹ: 2026-02-20-مالکان: کور پروٹوکول WG / اسٹوریج ٹیم / DA WG_

DA-2 ورکر تھریڈ Torii API میں توسیع کرتا ہے جس میں ریلیز ہوتا ہے
Norito میٹا ڈیٹا اور نقل شروع کرتا ہے SoraFS۔ دستاویز میں مجوزہ کی وضاحت کی گئی ہے
اسکیما ، API دائرہ کار اور توثیق کا بہاؤ تاکہ عمل درآمد بلاک کیے بغیر آگے بڑھ جائے
زیر التواء نقالی (فالو اپ ڈی اے -1)۔ تمام پے لوڈ فارمیٹس کو استعمال کرنا چاہئے
کوڈیکس Norito ؛ سیرڈ/JSON پر فال بیک کی اجازت نہیں ہے۔

## اہداف

- بڑے بلبس کو قبول کریں (تائکائی طبقات ، سڈیکار لین ، کنٹرول نمونے)
  Torii کے ذریعے تعی .ن کے لحاظ سے۔
- بلاب ، کوڈیک پیرامیٹرز کی وضاحت کرنے والے کیننیکل Norito مینی فیسٹ بنائیں ،
  مٹانے والی پروفائل اور برقرار رکھنے کی پالیسی۔
- گرم اسٹوریج SoraFS میں حصہ میٹا ڈیٹا کو محفوظ کریں اور نقل کے کاموں کو مرتب کریں
  قطار
- SoraFS رجسٹری اور مبصرین کو پن کے ارادے + پالیسی ٹیگز شائع کریں
  انتظامیہ
- داخلے کی رسیدیں جاری کریں تاکہ مؤکلوں کو تعصب کا سامنا کرنا پڑے
  اشاعت کی تصدیق.

## سطح API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

پے لوڈ `DaIngestRequest` ہے ، انکوڈڈ Norito ہے۔ جوابات استعمال
`application/norito+v1` اور واپسی `DaIngestReceipt`۔

| جواب | مطلب |
| --- | --- |
| 202 قبول شدہ | بلاب کو chunking/نقل کے لئے قطار میں کھڑا کیا گیا ہے۔ رسید واپس آگئی۔ |
| 400 بری درخواست | اسکیما/سائز کی خلاف ورزی (چیک دیکھیں) |
| 401 غیر مجاز | API ٹوکن غائب/غلط ہے۔ |
| 409 تنازعہ | مماثل میٹا ڈیٹا کے ساتھ `client_blob_id` کی نقل۔ |
| 413 پے لوڈ بہت بڑا | بلاب کی لمبائی کی حد سے تجاوز کر گئی۔ |
| 429 بہت ساری درخواستیں | شرح کی حد سے تجاوز |
| 500 داخلی غلطی | غیر متوقع غلطی (لاگ + الرٹ) |

## تجویز کردہ Norito سرکٹ

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

> نفاذ کا نوٹ: ان پے لوڈوں کی کیننیکل زنگ کی نمائندگی اب ہیں
> `iroha_data_model::da::types` میں ہیں ، درخواست/رسید ریپر کے ساتھ
> `iroha_data_model::da::ingest` اور ظاہر ڈھانچہ
> `iroha_data_model::da::manifest`۔

`compression` فیلڈ بیان کرتا ہے کہ کال کرنے والے نے پے لوڈ کو کس طرح تیار کیا۔ Torii قبول کرتا ہے
`identity` ، `gzip` ، `deflate` اور `zstd` ، ہیشنگ سے پہلے بائٹس کو کھولتے ہوئے ،
چنکنگ اور چیکنگ اختیاری منشور۔

### توثیق چیک لسٹ1. چیک کریں کہ ہیڈر Norito `DaIngestRequest` سے ملتا ہے۔
2. غلطی اگر `total_size` کیننیکل پے لوڈ کی لمبائی سے مختلف ہے
   (پیک کھولنے کے بعد) یا زیادہ سے زیادہ تشکیل شدہ زیادہ سے زیادہ ہے۔
3. فورس سیدھ `chunk_size` (دو کی طاقت ،  = 2۔
5. `retention_policy.required_replica_count` کو گورننس بیس لائن کی تعمیل کرنی ہوگی۔
6. کیننیکل ہیش (دستخطی فیلڈ کے بغیر) استعمال کرتے ہوئے دستخط کی تصدیق کرنا۔
7. ڈپلیکیٹ `client_blob_id` کو مسترد کریں اگر ہیش پے لوڈ اور میٹا ڈیٹا نہیں ہے
   ایک جیسی
8. اگر `norito_manifest` موجود ہے تو ، میچ کے لئے اسکیما + ہیش چیک کریں
   منشور ، ٹکرانے کے بعد دوبارہ گنتی ؛ بصورت دیگر نوڈ ظاہر ہوتا ہے اور
   اسے بچاتا ہے۔
9. تشکیل شدہ نقل کی پالیسی کا اطلاق کریں: Torii بھیجے ہوئے بھیجے
   `RetentionPolicy` `torii.da_ingest.replication_policy` کے ذریعے (دیکھیں
   `replication-policy.md`) اور اگر وہ ہیں تو پہلے سے تیار کردہ ظاہر ہونے کو مسترد کرتے ہیں
   برقرار رکھنے کا میٹا ڈیٹا نافذ شدہ پروفائل سے مماثل نہیں ہے۔

### اسٹریم چنکنگ اور نقل

1. پے لوڈ کو `chunk_size` میں تقسیم کریں ، ہر ایک حص + ہ + مرکل کے لئے بلیک 3 کا حساب لگائیں
   جڑ
2. فارم Norito `DaManifestV1` (نیا ڈھانچہ) ، عزم کو ٹھیک کرنا
   (role/group_id), erasure layout (row and column parity numbers plus
   `ipa_commitment`) ، برقرار رکھنے کی پالیسی اور میٹا ڈیٹا۔
3. قطار کی قطاریں بائٹس کے تحت ظاہر ہوتی ہیں
   `config.da_ingest.manifest_store_dir` (Torii `manifest.encoded` بذریعہ بذریعہ لکھتا ہے
   لین/ایپوچ/تسلسل/ٹکٹ/فنگر پرنٹ) تاکہ آرکیسٹریشن SoraFS کر سکے
   انہیں جذب کریں اور محفوظ شدہ ڈیٹا کے ساتھ اسٹوریج ٹکٹ کو منسلک کریں۔
4. کنٹرول ٹیگ کے ساتھ `sorafs_car::PinIntent` کے ذریعے پن کے ارادوں کو شائع کریں اور
   سیاست
5. ایونٹ ایونٹ Norito `DaIngestPublished` مبصرین کو مطلع کرنے کے لئے
   (ہلکے کلائنٹ ، گورننس ، تجزیات)
6. واپسی `DaIngestReceipt` کالر (Torii DA کلید کے ساتھ دستخط شدہ) اور بھیجیں
   ہیڈر `Sora-PDP-Commitment` تاکہ SDK فوری طور پر عزم حاصل کرے۔ رسید
   اب `rent_quote` (Norito `DaRentQuote`) اور `stripe_layout` سے شامل ہیں
   بھیجنے والے بیس کرایے ، ریزرو شیئر ، بونس کی توقعات کو دکھا سکتے ہیں
   فنڈز کا ارتکاب کرنے سے پہلے اسٹوریج ٹکٹ کے آگے PDP/POTR اور 2D مٹانے والی ترتیب۔

## اسٹوریج / رجسٹری کی تازہ کاری

- `sorafs_manifest` کو نئے `DaManifestV1` کے ساتھ بڑھاؤ ، جو اختیاری فراہم کرتا ہے
  پارسنگ
- ایک نیا رجسٹری اسٹریم `da.pin_intent` کو ورژن میں شامل پے لوڈ کے ساتھ شامل کریں ،
  جس سے مراد ہیش + ٹکٹ کی شناخت ہے۔
- ingest اور تھروپپٹ تاخیر کی نگرانی کے لئے مشاہدہ کرنے والی پائپ لائنوں کو اپ ڈیٹ کریں
  chunking ، نقل تیار کرنے والا بیکلاگ اور غلطی کاؤنٹر۔

## جانچ کی حکمت عملی- اسکیما کی توثیق ، ​​دستخطی توثیق ، ​​ڈپلیکیٹ کا پتہ لگانے کے لئے یونٹ ٹیسٹ۔
- Norito انکوڈنگ `DaIngestRequest` ، مینی فیسٹ اور رسید چیک کرنے کے لئے گولڈن ٹیسٹ۔
- انضمام کا استعمال ، فرضی SoraFS + رجسٹری بڑھانا اور جانچ پڑتال کرنا
  ندیوں کا حصہ + پن۔
- بے ترتیب مٹانے والے پروفائلز اور برقرار رکھنے کے امتزاج کے لئے پراپرٹی ٹیسٹ۔
- غلط میٹا ڈیٹا سے بچانے کے لئے Norito پے لوڈ کو فوزنگ کرنا۔

## CLI اور SDK ٹولنگ (DA-8)- `iroha app da submit` (نیا CLI انٹریپوائنٹ) عام ingest بلڈر/ لپیٹتا ہے
  ناشر تاکہ آپریٹرز دھاگے کے باہر صوابدیدی بلابز کھا سکتے ہیں
  تائکائی بنڈل۔ کمانڈ `crates/iroha_cli/src/commands/da.rs:1` اور میں ہے
  پے لوڈ ، مٹانے/برقرار رکھنے کا پروفائل اور اختیاری فائلوں کو قبول کرتا ہے
  کیننیکل `DaIngestRequest` کلید پر دستخط کرنے سے پہلے میٹا ڈیٹا/ظاہر
  سی ایل آئی کنفیگریشنز۔ کامیاب رنز `da_request.{norito,json}` اور محفوظ کرتے ہیں
  `da_receipt.{norito,json}` `artifacts/da/submission_<timestamp>/` کے تحت
  (override via `--artifact-dir`) so that release artefacts record accurate
  Norito بائٹس استعمال کرنے کے لئے استعمال کیا جاتا ہے۔
- پہلے سے طے شدہ کمانڈ `client_blob_id = blake3(payload)` استعمال کرتی ہے ، لیکن
  `--client-blob-id` ، JSON نقشہ میٹا ڈیٹا کے ذریعے اوور رائڈس کی حمایت کرتا ہے
  (`--metadata-json`) اور پہلے سے تیار کردہ منشور (`--manifest`) کے ساتھ ساتھ
  `--no-submit` آف لائن تیاری کے لئے اور `--endpoint` کسٹم Torii میزبانوں کے لئے۔
  رسید JSON STDOUT پر چھپی ہوئی ہے اور DA-8 کی درخواست کو بند کرتے ہوئے ڈسک پر لکھا گیا ہے
  "جمع کرانے_بلوب" اور کام کرنے کے لئے ایس ڈی کے برابری کو غیر مقفل کرنا۔
-`iroha app da get` ملٹی سورس آرکیسٹریٹر کے لئے ڈی اے پر مبنی عرف شامل کرتا ہے ،
  جو پہلے ہی `iroha app sorafs fetch` کو طاقت دیتا ہے۔ آپریٹر نوادرات کی وضاحت کرسکتے ہیں
  منشور + چنک پلان (`--manifest` ، `--plan` ، `--manifest-id`) ** یا ** پاس
  Torii اسٹوریج ٹکٹ `--storage-ticket` کے ذریعے۔ جب ٹکٹ CLI استعمال کرتے ہو
  `/v1/da/manifests/<ticket>` سے ظاہر ہوتا ہے ، بنڈل کو بچاتا ہے
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir` کے ساتھ اوور رائڈ) ، آؤٹ پٹ
  `--manifest-id` کے لئے بلاب ہیش اور دیئے گئے کے ساتھ آرکیسٹریٹر شروع کرتا ہے
  `--gateway-provider` فہرست۔ SoraFS فیچر سے تمام جدید نوبس
  محفوظ ہیں (مینی فیسٹ لفافے ، کلائنٹ لیبل ، گارڈ کیچز ، گمنام
  ٹرانسپورٹ اوور رائڈس ، ایکسپورٹ اسکور بورڈ ، `--output` راستے) ، اور ظاہر اختتامی نقطہ
  کسٹم Torii میزبانوں کے لئے `--manifest-endpoint` کے ذریعے اوورراڈ کیا جاسکتا ہے ،
  لہذا آخر سے آخر تک دستیابی کی جانچ پڑتال نام کی جگہ `da` میں بغیر رہتی ہے
  آرکیسٹریٹر منطق کی نقل۔
- `iroha app da get-blob` براہ راست Torii کے ذریعے کیننیکل ظاہر کرتا ہے
  `GET /v1/da/manifests/{storage_ticket}`۔ ٹیم لکھتی ہے
  `manifest_{ticket}.norito` ، `manifest_{ticket}.json` اور `chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/` (یا کسٹم `--output-dir`) میں ، کے ساتھ
  اس سے عین مطابق کمانڈ `iroha app da get` (جس میں `--manifest-id` بھی شامل ہے) کی ضرورت ہے
  اس کے بعد کے آرکیسٹریٹر بازیافت کے لئے۔ اس سے آپریٹرز سے نمٹنے سے بچ جاتا ہے
  منشور اسپل ڈائریکٹریز اور اس بات کو یقینی بناتا ہے کہ بازیافت ہمیشہ استعمال کرتا ہے
  دستخط شدہ نوادرات Torii۔ جاوا اسکرپٹ کلائنٹ Torii اس بہاؤ کو دہراتا ہے
  `ToriiClient.getDaManifest(storageTicketHex)` ، واپسی decoded Norito
  بائٹس ، منشور JSON اور حصہ کا منصوبہ تاکہ SDK کال کرنے والے اٹھا سکیں
  آرکسٹریٹر سیشن کے بغیر سی ایل آئی۔ سوئفٹ ایس ڈی کے اب وہی سطحیں فراہم کرتا ہے
  (`ToriiClient.getDaManifestBundle(...)` اور `fetchDaPayloadViaGateway(...)`) ،
  بنڈل کو آبائی ریپر SoraFS آرکیسٹریٹر میں منتقل کرنا تاکہ iOS کلائنٹ کرسکیں
  منشور ڈاؤن لوڈ کریں ، ملٹی سورس بازیافت کریں اور بغیر ثبوت جمع کریں
  CLI کال..
- `iroha app da rent-quote` اس کے لئے عین مطابق کرایہ اور تجزیہ کاروں کا حساب لگاتا ہے
  دیئے گئے اسٹوریج کا سائز اور برقرار رکھنے والی ونڈو۔ مددگار فعال استعمال کرتا ہے
  `DaRentPolicyV1` (JSON یا Norito بائٹس) یا بلٹ ان ڈیفالٹ ، توثیق کرتا ہے
  پالیسی اور پرنٹ ایک JSON خلاصہ (`gib` ، `months` ، پالیسی میٹا ڈیٹا اور فیلڈز
  `DaRentQuote`) تاکہ آڈیٹر پروٹوکول میں عین مطابق XOR چارجز کا حوالہ دے سکیں
  ایڈہاک اسکرپٹ کے بغیر کنٹرول کریں۔ کمانڈ ایک لائنر بھی پرنٹ کرتا ہے
  `rent_quote ...` مشقوں کے دوران نوشتہ جات کی پڑھنے کے ل J JSON پے لوڈ سے پہلے۔
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` سے لنک کریں
  `--policy-label "governance ticket #..."` نوادرات کو صاف رکھنے کے لئے
  ووٹ یا کنفیگ بنڈل کے عین مطابق لنک کے ساتھ۔ سی ایل آئی صارف کو چھوٹا کرتا ہے
  `policy_source` کو قابل استعمال رکھنے کے لئے خالی لائنوں کو لیبل اور مسترد کرتا ہے
  ڈیش بورڈز۔ سب کمانڈ کے لئے `crates/iroha_cli/src/commands/da.rs` دیکھیں اور
  پالیسی اسکیم کے لئے `docs/source/da/rent_policy.md`۔
  .
- `iroha app da prove-availability` مذکورہ بالا ہر چیز کو جوڑتا ہے: اسٹوریج کا ٹکٹ لیتا ہے ،
  کیننیکل مینی فیسٹ بنڈل ڈاؤن لوڈ کرتا ہے ، ملٹی سورس آرکسٹریٹر لانچ کرتا ہے
  (`iroha app sorafs fetch`) فہرست کے خلاف `--gateway-provider` ، بچت کرتا ہے
  `artifacts/da/prove_availability_<timestamp>/` میں ڈاؤن لوڈ شدہ پے لوڈ + اسکور بورڈ ،
  اور موصولہ کے ساتھ فوری طور پر موجودہ پور مددگار (`iroha app da prove`) کو کال کرتا ہے
  بائٹس آپریٹرز آرکسٹریٹر نوبس (`--max-peers` ، ، تشکیل دے سکتے ہیں
  `--scoreboard-out` ، مینی فیسٹ اینڈ پوائنٹ اوور رائڈز) اور پروف سیمپلر
  (`--sample-count` ، `--leaf-index` ، `--sample-seed`) ، ایک کمانڈ کے ساتھ
  DA-5/DA-9 آڈٹ کے ذریعہ درکار نوادرات جاری کرتا ہے: پے لوڈ کاپی ، ثبوت
  اسکور بورڈ اور JSON دوبارہ شروع کرنے کا ثبوت۔