---
lang: ur
direction: rtl
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
`docs/source/da/ingest_plan.md` کی عکاسی کرتا ہے۔ دونوں ورژن کو مطابقت پذیری میں رکھیں
:::

# سورہ ڈیٹا کی دستیابی انجسٹ پلان Nexus

_ رائٹ: 2026-02-20-ذمہ دار: کور پروٹوکول WG / اسٹوریج ٹیم / DA WG_

DA-2 ورک اسٹریم Torii کو ایک بلاب Ingest API کے ساتھ بڑھاتا ہے جو خارج ہوتا ہے
میٹا ڈیٹا Norito اور نقل SoraFS شروع کرتا ہے۔ اس دستاویز نے اسے اپنی گرفت میں لے لیا
اسکیما تجویز کرتی ہے ، API سطح اور توثیق کا بہاؤ تاکہ
نفاذ باقی نقوش (نگرانی) پر بلاک کیے بغیر ترقی کرتا ہے
DA-1)۔ تمام پے لوڈ فارمیٹس کو Norito کوڈیکس استعمال کرنا چاہئے۔ کوئی نہیں
فال بیک بیک سرڈ/JSON کی اجازت نہیں ہے۔

## مقاصد

- بڑے بلبس کو قبول کریں (تائیکائی طبقات ، لین سڈیکارس ، نمونے
  گورننس) Torii کے ذریعے ایک عزم انداز میں۔
- کیننیکل Norito تیار کریں ، بلاب کی وضاحت کرتے ہوئے ، پیرامیٹرز
  کوڈیک ، مٹانے والی پروفائل اور برقرار رکھنے کی پالیسی۔
- SoraFS کے گرم اسٹوریج میں حصوں کو میٹا ڈیٹا برقرار رکھیں اور ڈالیں
  قطار کی نقلیں ملازمتیں۔
- رجسٹری SoraFS میں پن کے ارادے + پالیسی ٹیگز شائع کریں
  گورننس مبصرین۔
- داخلہ کی رسیدیں ڈسپلے کریں تاکہ صارفین ثبوت تلاش کرسکیں
  اشاعت کا تعین۔

## API سطح (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

پے لوڈ `DaIngestRequest` کو Norito کے بطور انکوڈ کیا گیا ہے۔ جوابات استعمال
`application/norito+v1` اور واپسی `DaIngestReceipt`۔

| جواب | مطلب |
| --- | --- |
| 202 قبول شدہ | chunking/نقل کے لئے قطار میں بلاب ؛ رسید واپس آگئی۔ |
| 400 بری درخواست | اسکیما/سائز کی خلاف ورزی (توثیق کی جانچ پڑتال دیکھیں)۔ |
| 401 غیر مجاز | گمشدہ/غلط API ٹوکن۔ |
| 409 تنازعہ | غیر قانونی میٹا ڈیٹا کے ساتھ Norito کی نقل۔ |
| 413 پے لوڈ بہت بڑا | تشکیل شدہ بلاب کی لمبائی کی حد سے تجاوز کرتا ہے۔ |
| 429 بہت ساری درخواستیں | شرح کی حد تک پہنچ گئی۔ |
| 500 داخلی غلطی | غیر متوقع ناکامی (لاگ + الرٹ) |

## اسکیما Norito تجویز کرتا ہے

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

> نفاذ کا نوٹ: ان پے لوڈز کے لئے کیننیکل زنگ کی نمائندگی
> اب ریپروں کے ساتھ ، `iroha_data_model::da::types` کے تحت رہیں
> `iroha_data_model::da::ingest` اور کی ساخت میں درخواست/رسید
> `iroha_data_model::da::manifest` میں ظاہر ہوں۔

`compression` فیلڈ نے اعلان کیا کہ کال کرنے والوں نے پے لوڈ کو کس طرح تیار کیا۔ Torii
`identity` ، `gzip` ، `deflate` ، اور `zstd` کو قبول کرتا ہے ، اس سے پہلے بائٹس کو ڈیکپریس کرتے ہوئے
ہیشنگ ، چنکنگ اور اختیاری منشور کی جانچ کرنا۔

### توثیق چیک لسٹ1. چیک کریں کہ درخواست کا ہیڈر Norito `DaIngestRequest` سے مساوی ہے۔
2. اگر `total_size` کیننیکل پے لوڈ کی لمبائی سے مختلف ہے تو ناکام
   (ڈیکمپریسز) یا زیادہ سے زیادہ تشکیل شدہ زیادہ سے زیادہ ہے۔
3. Norito کی فورس سیدھ (دو کی طاقت ،  = 2۔
5. `retention_policy.required_replica_count` کو لازمی طور پر بیس لائن کا احترام کرنا چاہئے
   گورننس
6. کیننیکل ہیش کے خلاف دستخطی توثیق (فیلڈ کو چھوڑ کر)
   دستخط)۔
7. ایک ڈپلیکیٹ `client_blob_id` کو مسترد کریں جب تک کہ پے لوڈ ہیش اور میٹا ڈیٹا
   ایک جیسے ہیں۔
8. جب `norito_manifest` فراہم کیا جاتا ہے تو ، چیک کریں کہ اسکیما + ہیش کے مطابق ہے
   chunking کے بعد منشور کو دوبارہ گنتی ؛ بصورت دیگر نوڈ منشور اور پیدا کرتا ہے
   اسٹورز
9. تشکیل شدہ نقل کی پالیسی کا اطلاق کریں: Torii دوبارہ لکھتا ہے
   `RetentionPolicy` `torii.da_ingest.replication_policy` کے ساتھ جمع کرایا گیا (دیکھیں
   `replication-policy.md`) اور اس سے پہلے سے تیار کردہ ظاہر ہونے والے منشور کو مسترد کرتا ہے
   برقرار رکھنے کا میٹا ڈیٹا مسلط کردہ پروفائل سے مطابقت نہیں رکھتا ہے۔

### بہاؤ اور نقل تیار کرنا

1. پے لوڈ کو `chunk_size` میں کاٹ دیں ، بلیک 3 کا حساب کتاب + مرکل روٹ کے ذریعہ کریں۔
2. Norito `DaManifestV1` (نیا ڈھانچہ) کیپنگ کمنٹس کی تعمیر کریں
   حصہ (کردار/گروپ_ایڈ) ، مٹانے کی ترتیب (لائن پیریٹی گنتی اور
   کالم پلس `ipa_commitment`) ، برقرار رکھنے کی پالیسی اور میٹا ڈیٹا۔
3. قطار کی بائٹس کے بائٹس کے تحت بائٹس کے تحت
   `config.da_ingest.manifest_store_dir` (Torii فائلیں لکھتا ہے
   `manifest.encoded` لین/ایپوچ/تسلسل/ٹکٹ/فنگر پرنٹ کے ذریعہ ترتیب دیا گیا ہے) ترتیب میں
   کہ آرکیسٹریشن SoraFS ان کو کھاتا ہے اور اسٹوریج کے ٹکٹ کو ڈیٹا سے جوڑتا ہے
   برقرار رہا۔
4. گورننس ٹیگ کے ساتھ `sorafs_car::PinIntent` کے ذریعے پن کے ارادے شائع کریں
   اور سیاست۔
5. ایونٹ ایونٹ Norito `DaIngestPublished` مبصرین کو مطلع کرنے کے لئے
   (پتلی کلائنٹ ، گورننس ، تجزیات)
6. واپس `DaIngestReceipt` کالر کو واپس کریں (Torii کی DA سروس کلید کے ذریعہ دستخط شدہ)
   اور ہیڈر `Sora-PDP-Commitment` خارج کریں تاکہ SDKs کو گرفت میں لے
   عزم فوری طور پر انکوڈ ہوجاتا ہے۔ رسید میں اب `rent_quote` شامل ہے
   (an Norito `DaRentQuote`) and `stripe_layout`, allowing submitters
   بنیادی سالانہ ، ریزرو ، PDP/POTR بونس کی توقعات اور اس کو ظاہر کرنے کے لئے
   فنڈز کا ارتکاب کرنے سے پہلے اسٹوریج ٹکٹ کے ساتھ ساتھ 2D مٹانے والی ترتیب۔

## اسٹوریج/رجسٹری کی تازہ کاری

- تجزیہ کی اجازت دیتے ہوئے ، `DaManifestV1` کے ساتھ `sorafs_manifest` میں توسیع کریں
  تعی .ن پسند
- ایک نیا رجسٹری اسٹریم `da.pin_intent` کو ورژن میں شامل پے لوڈ کے ساتھ شامل کریں
  منشور + ٹکٹ ID کے ہیش کا حوالہ دینا۔
- لیٹینسی کو ٹریک کرنے کے لئے مشاہدہ کرنے والی پائپ لائنوں کو اپ ڈیٹ کریں ،
  چنکنگ تھروپپٹ ، نقل بیک بیک اور کاؤنٹرز
  شطرنج

## جانچ کی حکمت عملی- اسکیما کی توثیق ، ​​دستخطی چیک ، کا پتہ لگانے کے لئے یونٹ ٹیسٹ
  نقل
- Norito `DaIngestRequest` ، منشور اور کی تصدیق کرنے والے گولڈن ٹیسٹ
  رسید
- انضمام کا استعمال ایک مصنوعی SoraFS + رجسٹری شروع کرنا ، اس کی توثیق کرنا
  حصہ + پن اسٹریم۔
- پراپرٹی ٹیسٹ مٹانے والے پروفائلز اور کے امتزاج پر محیط
  بے ترتیب برقرار رکھنا۔
- خراب شدہ میٹا ڈیٹا سے بچانے کے لئے فوزنگ پے لوڈ Norito۔

## ٹولنگ CLI & SDK (DA-8)- `iroha app da submit` (نیا CLI انٹری پوائنٹ) اب بلڈر کو لپیٹتا ہے
  مشترکہ انجینج تاکہ آپریٹرز بلابز کھا سکیں
  تائیکائی بنڈل اسٹریم سے صوابدیدی۔ آرڈر میں رہتا ہے
  `crates/iroha_cli/src/commands/da.rs:1` اور ایک پے لوڈ ، ایک پروفائل استعمال کرتا ہے
  اس سے پہلے مٹانے/برقرار رکھنے اور اختیاری میٹا ڈیٹا/مینی فیسٹ فائلیں
  CLI کنفگ کلید کے ساتھ کیننیکل `DaIngestRequest` پر دستخط کریں۔ رنز
  کامیابی کے تحت `da_request.{norito,json}` اور `da_receipt.{norito,json}` کے تحت جاری رکھیں
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` کے ذریعے اوور رائڈ) تاکہ
  ریلیز نمونے کے دوران استعمال ہونے والے عین مطابق Norito بائٹس کو ریکارڈ کیا جاتا ہے
  اس میں گھس جائیں۔
- کمانڈ `client_blob_id = blake3(payload)` میں ڈیفالٹ کرتا ہے لیکن قبول کرتا ہے
  `--client-blob-id` کے ذریعے اوور رائڈس ، آنرز میٹا ڈیٹا JSON نقشہ جات
  (`--metadata-json`) اور پہلے سے تیار کردہ ظاہر (`--manifest`) ، اور سپورٹ
  `--no-submit` آف لائن تیاری کے علاوہ `--endpoint` میزبانوں کے لئے
  Torii ذاتی نوعیت کا۔ JSON رسید ہونے کے علاوہ STDOUT پر بھی چھپی ہوئی ہے
  ڈسک پر لکھا ہوا ، DA-8 "سبم_بلوب" ٹولنگ کی ضرورت کو بند کرنا اور
  ایس ڈی کے برابری کے کام کو غیر مقفل کرنا۔
- `iroha app da get` ملٹی سورس آرکسٹریٹر کے لئے ایک DA عرف شامل کرتا ہے جو کھانا کھلاتا ہے
  پہلے ہی `iroha app sorafs fetch`۔ آپریٹرز اسے نمونے کی طرف اشارہ کرسکتے ہیں
  منشور + چنک پلان (`--manifest` ، `--plan` ، `--manifest-id`) ** یا ** فراہم کریں
  `--storage-ticket` کے ذریعے ایک اسٹوریج ٹکٹ Torii۔ جب ٹکٹ کا راستہ ہے
  استعمال کیا جاتا ہے ، سی ایل آئی `/v1/da/manifests/<ticket>` سے ظاہر ہوتا ہے ،
  `artifacts/da/fetch_<timestamp>/` کے تحت بنڈل برقرار رہتا ہے (اس کے ساتھ اوور رائڈ
  `--manifest-cache-dir`) ، `--manifest-id` کے لئے بلاب ہیش اخذ کرتا ہے ، پھر
  فراہم کردہ فہرست `--gateway-provider` کے ساتھ آرکسٹریٹر پر عملدرآمد کرتا ہے۔ سب
  فیچر SoraFS کے اعلی درجے کی نوبس برقرار رہیں (مینی فیسٹ لفافے ، لیبل
  کلائنٹ ، گارڈ کیچز ، گمنام ٹرانسپورٹ اوور رائڈس ، ایکسپورٹ اسکور بورڈ اور
  راستے `--output`) ، اور اختتامی نقطہ ظاہر کے ذریعے اوورلوڈ کیا جاسکتا ہے
  `--manifest-endpoint` ذاتی نوعیت کے میزبان Torii کے لئے ، لہذا چیک
  آخر سے آخر تک دستیابی مکمل طور پر نام کی جگہ `da` کے بغیر رہتی ہے
  ڈپلیکیٹ آرکیسٹریٹر منطق۔
- `iroha app da get-blob` براہ راست Torii سے کیننیکل منشور بازیافت کرتا ہے
  `GET /v1/da/manifests/{storage_ticket}` کے ذریعے۔ کمانڈ لکھتا ہے
  `manifest_{ticket}.norito` ، `manifest_{ticket}.json` اور
  `chunk_plan_{ticket}.json` `artifacts/da/fetch_<timestamp>/` کے تحت (یا a
  عین مطابق کمانڈ کی نمائش کرتے وقت صارف کی فراہم کردہ `--output-dir`)
  `iroha app da get` (بشمول `--manifest-id`) آرکسٹریٹر بازیافت کے لئے ضروری ہے۔
  اس سے آپریٹرز کو منشور اسپل ڈائریکٹریوں سے دور رہتا ہے اور اس بات کو یقینی بناتا ہے
  کہ فیچر ہمیشہ Torii کے ذریعہ خارج ہونے والے اشارے کے نمونے استعمال کرتا ہے۔ گاہک
  Torii جاوا اسکرپٹ اس بہاؤ کو دوبارہ تیار کرتا ہے
  `ToriiClient.getDaManifest(storageTicketHex)` ، بائٹس Norito واپس کرنا
  ڈیکوڈس ، JSON منشور اور حصہ کا منصوبہ تاکہ SDK کال کرنے والے ہائیڈریٹ کریں
  آرکیسٹریٹر سیشن بغیر کسی سی ایل آئی کے ذریعے۔ سوئفٹ ایس ڈی کے بے نقاب ہوتا ہے
  اب وہی سطحیں (`ToriiClient.getDaManifestBundle(...)` مزید`fetchDaPayloadViaGateway(...)`) ، بنڈلوں کو آبائی ریپر میں پلگ ان کریں
  آرکسٹریٹر SoraFS تاکہ iOS کلائنٹ ڈاؤن لوڈ کرسکیں
  منشور ، کثیر سورس کی بازیافت اور بغیر شواہد پر قبضہ کریں
  سی ایل آئی کو طلب کریں۔
  .
- `iroha app da rent-quote` تعصب کے کرایوں اور خرابی کا حساب لگاتا ہے
  اسٹوریج کے سائز اور برقرار رکھنے والی ونڈو کے لئے مراعات فراہم کی گئیں۔
  مددگار فعال `DaRentPolicyV1` (JSON یا بائٹس Norito) یا
  ڈیفالٹ انضمام کرتا ہے ، پالیسی کی توثیق کرتا ہے اور JSON خلاصہ (`gib` ، پرنٹ کرتا ہے ،
  `months` ، پالیسی میٹا ڈیٹا ، فیلڈز `DaRentQuote`) تاکہ سننے والے
  اشتہار کے اسکرپٹ کے بغیر گورننس منٹ میں عین مطابق XOR پے لوڈ کا حوالہ دیں
  ہاک۔ اس سے پہلے کمانڈ ون لائن سمری `rent_quote ...` بھی خارج کرتی ہے
  مشقوں کے دوران کنسول لاگز کو پڑھنے کے قابل رکھنے کے لئے JSON پے لوڈ
  واقعہ `--quote-out artifacts/da/rent_quotes/<stamp>.json` کے ساتھ ایسوسی ایٹ کریں
  `--policy-label "governance ticket #..."` علاج شدہ نمونے کو برقرار رکھنے کے لئے
  عین مطابق ووٹ یا کنفیگ بنڈل کا حوالہ دینا ؛ سی ایل آئی کسٹم لیبل کو چھوٹا کرتا ہے
  اور خالی تاروں سے انکار کرتا ہے تاکہ `policy_source` اقدار باقی رہیں
  ٹریژری ڈیش بورڈز میں قابل عمل۔ دیکھو
  `crates/iroha_cli/src/commands/da.rs` subcommand اور کے لئے
  پالیسی اسکیما کے لئے `docs/source/da/rent_policy.md`۔
  .
- `iroha app da prove-availability` چین مندرجہ بالا سبھی: یہ اسٹوریج لیتا ہے
  ٹکٹ ، کیننیکل مینی فیسٹ بنڈل ڈاؤن لوڈ کریں ، آرکسٹریٹر چلائیں
  فہرست `--gateway-provider` کے خلاف ملٹی سورس (`iroha app sorafs fetch`)
  بشرطیکہ ، ڈاؤن لوڈ شدہ پے لوڈ + اسکور بورڈ کے تحت برقرار رہتا ہے
  `artifacts/da/prove_availability_<timestamp>/` ، اور فوری طور پر اس کی درخواست کرتا ہے
  بازیافت شدہ بائٹس کے ساتھ موجودہ پور ہیلپر (`iroha app da prove`)۔ آپریٹرز
  آرکسٹریٹر نوبس کو ایڈجسٹ کرسکتے ہیں (`--max-peers` ، `--scoreboard-out` ،
  اینڈ پوائنٹ مینی فیسٹ اوور رائڈز) اور پروف سیمپلر (`--sample-count` ،
  `--leaf-index` ، `--sample-seed`) جبکہ ایک ہی کمانڈ تیار کرتا ہے
  DA-5/DA-9 آڈٹ کے ذریعہ متوقع نمونے: پے لوڈ کی کاپی ، اس کے ثبوت
  اسکور بورڈ اور JSON پروف خلاصہ۔