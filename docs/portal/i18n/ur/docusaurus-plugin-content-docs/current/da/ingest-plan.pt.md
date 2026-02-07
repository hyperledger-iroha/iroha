---
lang: ur
direction: rtl
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
آئینہ `docs/source/da/ingest_plan.md`۔ دونوں ورژن کو اندر رکھیں
:::

# سورہ ڈیٹا کی دستیابی انجسٹ پلان Nexus

_ رائٹ: 2026-02-20-ذمہ دار: کور پروٹوکول WG / اسٹوریج ٹیم / DA WG_

DA-2 ورک اسٹریم Torii کو ایک بلاب انجشن API کے ساتھ بڑھاتا ہے جو خارج ہوتا ہے
میٹا ڈیٹا Norito اور بیجوں کی نقل SoraFS۔ اس دستاویز نے اسکیما کو اپنی گرفت میں لے لیا
مجوزہ ، API سطح اور توثیق کا بہاؤ تاکہ عمل درآمد
زیر التواء نقالی (DA-1 ترتیب) کو بلاک کیے بغیر پیش قدمی۔ سب
پے لوڈ فارمیٹس کو Norito کوڈیکس استعمال کرنا چاہئے۔ فال بیکس کی اجازت نہیں ہے
سیرڈ/JSON۔

## مقاصد

- بڑے بلبس کو قبول کریں (تائیکائی طبقات ، لین سڈیکارس ، نمونے
  گورننس) Torii کے ذریعے عزم کے مطابق۔
- کیننیکل Norito مظہر پیدا کریں جو بلاب ، کوڈیک پیرامیٹرز کو بیان کرتے ہیں ،
  مٹانے والی پروفائل اور برقرار رکھنے کی پالیسی۔
- SoraFS اور قطار ملازمتوں کے گرم اسٹوریج میں حصہ میٹا ڈیٹا کو برقرار رکھیں
  نقل
- پن کے ارادے + پالیسی ٹیگز کو رجسٹری SoraFS اور مبصرین پر شائع کریں
  گورننس کی
- مؤکلوں کے لئے داخلہ کی رسیدوں کو بے نقاب کریں
  اشاعت کی

## سطح API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

پے لوڈ Norito میں ایک `DaIngestRequest` انکوڈ کیا گیا ہے۔ جوابات استعمال
`application/norito+v1` اور واپسی `DaIngestReceipt`۔

| جواب | مطلب |
| --- | --- |
| 202 قبول شدہ | ٹکرانے/نقل کے لئے بلاب قطار میں کھڑا ؛ رسید واپس آگئی۔ |
| 400 بری درخواست | اسکیما/سائز کی خلاف ورزی (توثیق دیکھیں) |
| 401 غیر مجاز | گمشدہ/غلط API ٹوکن۔ |
| 409 تنازعہ | ڈوپلجنٹ میٹا ڈیٹا کے ساتھ Norito کی نقل۔ |
| 413 پے لوڈ بہت بڑا | تشکیل شدہ بلاب سائز کی حد سے تجاوز کرتا ہے۔ |
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

> عمل درآمد نوٹ: ان پے لوڈ کے لئے زنگ میں کیننیکل نمائندگی
> اب درخواست/رسید ریپرس کے ساتھ ، `iroha_data_model::da::types` میں براہ راست رہیں
> `iroha_data_model::da::ingest` میں اور میں ظاہر ڈھانچہ
> `iroha_data_model::da::manifest`۔

`compression` فیلڈ نے بتایا کہ کال کرنے والوں نے پے لوڈ کو کس طرح تیار کیا۔ Torii قبول کرتا ہے
`identity` ، `gzip` ، `deflate` ، اور `zstd` ، ہیشنگ سے پہلے بائٹس کو ڈیکپریس کرتے ہوئے ،
چنکنگ اور اختیاری ظاہر کی توثیق۔

### توثیق چیک لسٹ1. چیک کریں کہ آیا درخواست ہیڈر Norito `DaIngestRequest` سے مطابقت رکھتا ہے۔
2. اگر `total_size` کیننیکل پے لوڈ سائز (غیر سنجیدہ) سے مختلف ہے تو ناکام ہوجائیں
   یا زیادہ سے زیادہ تشکیل شدہ حد سے تجاوز کریں۔
3. Norito کی فورس سیدھ (دو کی طاقت ،  = 2۔
5. `retention_policy.required_replica_count` کو بیس لائن کا احترام کرنا چاہئے
   گورننس
6. کیننیکل ہیش کے خلاف دستخطی توثیق (دستخطی فیلڈ کو چھوڑ کر)۔
7. ڈپلیکیٹ `client_blob_id` کو مسترد کریں جب تک کہ پے لوڈ ہیش اور میٹا ڈیٹا نہ ہو
   ایک جیسے ہیں۔
8. جب `norito_manifest` فراہم کیا جاتا ہے تو ، اسکیما + ہیش میچ چیک کریں
   منشور کو ٹکرانے کے بعد دوبارہ گنتی ؛ بصورت دیگر نوڈ منشور پیدا کرتا ہے
   اور اسے اسٹور کرتا ہے۔
9. تشکیل شدہ نقل کی پالیسی پر مجبور کریں: Torii کو دوبارہ لکھیں
   `RetentionPolicy` `torii.da_ingest.replication_policy` کے ساتھ بھیج دیا گیا (دیکھیں
   `replication-policy.md`) اور پہلے سے تعمیر شدہ ظاہر ہوتا ہے جس کا میٹا ڈیٹا
   برقرار رکھنا مسلط پروفائل سے مماثل نہیں ہے۔

### chunking اور نقل بہاؤ

1. پے لوڈ کو `chunk_size` میں کاٹیں ، بلیک 3 کا حساب کتاب + مرکل روٹ کے ذریعہ کریں۔
2. Norito `DaManifestV1` (نیا ڈھانچہ) پر قبضہ کرنے کے وعدوں کو بنائیں
   حصہ (کردار/گروپ_ایڈ) ، مٹانے کی ترتیب (قطار کی برابری کی گنتی اور
   کالم پلس `ipa_commitment`) ، برقرار رکھنے کی پالیسی اور میٹا ڈیٹا۔
3. قطار کے تحت کیننیکل منشور بائٹس کی قطار لگائیں
   `config.da_ingest.manifest_store_dir` (Torii فائلیں لکھتا ہے
   `manifest.encoded` فی لین/ایپوچ/ترتیب/ٹکٹ/فنگر پرنٹ) تاکہ
   آرکیسٹریشن SoraFS ان کو انجینج کریں اور اسٹوریج کے ٹکٹ کو ڈیٹا سے جوڑیں
   برقرار رہا۔
4. گورننس ٹیگ کے ساتھ `sorafs_car::PinIntent` کے ذریعے پن کے ارادے شائع کریں اور
   سیاست
5. جاری کردہ واقعہ Norito `DaIngestPublished` مبصرین کو مطلع کرنے کے لئے
   (ہلکے کلائنٹ ، گورننس ، تجزیات)
6. کالر کو `DaIngestReceipt` واپس کریں (DA سروس کی کے ذریعہ دستخط شدہ
   Torii) اور SDKs کے لئے `Sora-PDP-Commitment` ہیڈر کا اخراج کریں
   عزم فوری طور پر کوڈڈ ہوگیا۔ رسید میں اب `rent_quote` شامل ہے
   (ایک Norito `DaRentQuote`) اور `stripe_layout` ، بھیجنے والوں کی اجازت دیتے ہیں
   بیس انکم ، ریزرو ، PDP/POTR بونس کی توقعات اور اس کی ترتیب کو ظاہر کریں
   فنڈز کا ارتکاب کرنے سے پہلے اسٹوریج ٹکٹ کے آگے 2 ڈی مٹانے۔

## اسٹوریج/رجسٹری کی تازہ کاری

- Norito کے ساتھ `sorafs_manifest` میں توسیع کریں ، تجزیہ کو چالو کریں
  تعصب پسند
- ورژن والے پے لوڈ کے ساتھ نئی رجسٹری اسٹریم `da.pin_intent` شامل کریں
  مینی فیسٹ ہیش + ٹکٹ ID کا حوالہ دینا۔
- ادخال لیٹینسی کو ٹریک کرنے کے لئے مشاہدہ کرنے والی پائپ لائنوں کو اپ ڈیٹ کریں ،
  چنکنگ تھروپپٹ ، نقل کا بیکلاگ اور ناکامی کی گنتی۔

## جانچ کی حکمت عملی- اسکیما کی توثیق ، ​​دستخطی چیک ، کا پتہ لگانے کے لئے یونٹ ٹیسٹ
  نقل
- گولڈن ٹیسٹ چیکنگ انکوڈنگ Norito اور `DaIngestRequest` ، منشور اور
  رسید
- انضمام کا استعمال فرضی SoraFS + رجسٹری ، کام کے بہاؤ کی توثیق کرنا
  chunk + پن.
- مٹانے والے پروفائلز اور برقرار رکھنے کے امتزاج کو ڈھکنے والے پراپرٹی ٹیسٹ
  بے ترتیب
- خراب شدہ میٹا ڈیٹا سے بچانے کے لئے Norito پے لوڈ کا فوزنگ۔

## CLI & SDK ٹولنگ (DA-8)- `iroha app da submit` (نیا CLI انٹری پوائنٹ) اب بلڈر/پبلشر شامل ہے
  مشترکہ ادخال تاکہ آپریٹرز صوابدیدی بلابز کھا سکتے ہیں
  تائکائی بنڈل اسٹریم کے باہر۔ کمانڈ میں رہتا ہے
  `crates/iroha_cli/src/commands/da.rs:1` اور ایک پے لوڈ ، پروفائل استعمال کرتا ہے
  دستخط کرنے سے پہلے مٹانے/برقرار رکھنے اور اختیاری میٹا ڈیٹا/مینی فیسٹ فائلوں کو
  CLI کنفگ کلید کے ساتھ کیننیکل `DaIngestRequest`۔ کامیاب پھانسی
  `da_request.{norito,json}` اور `da_receipt.{norito,json}` کے تحت جاری رکھیں
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` کے ذریعے اوور رائڈ) تاکہ
  ریلیز نمونے کے دوران استعمال ہونے والے عین مطابق Norito بائٹس کو ریکارڈ کیا جاتا ہے
  انٹیک
- کمانڈ `client_blob_id = blake3(payload)` کو بطور ڈیفالٹ استعمال کرتا ہے لیکن قبول کرتا ہے
  `--client-blob-id` کے ذریعے اوور رائڈس ، آنر میٹا ڈیٹا JSON نقشہ (`--metadata-json`)
  اور پہلے سے تیار کردہ ظاہر (`--manifest`) ، اور اسٹیجنگ کے لئے `--no-submit` کی حمایت کرتا ہے
  کسٹم Torii میزبانوں کے لئے آف لائن پلس `--endpoint`۔ JSON رسید اور
  ڈسک پر لکھے جانے کے علاوہ ، ایس ٹی ڈی آؤٹ پر چھپی ہوئی ، ضرورت کو بند کرنا
  ٹولنگ DA-8 کے "سبم_بلوب" اور SDK پیریٹی ورک کو غیر مقفل کرنا۔
-`iroha app da get` ملٹی سورس آرکیسٹریٹر کے لئے ڈی اے پر مبنی عرف شامل کرتا ہے
  جو پہلے ہی `iroha app sorafs fetch` کو کھانا کھلاتا ہے۔ آپریٹر نمونے کی طرف اشارہ کرسکتے ہیں
  منشور + چنک پلان سے (`--manifest` ، `--plan` ، `--manifest-id`) ** یا **
  `--storage-ticket` کے ذریعے اسٹوریج ٹکٹ Torii پاس کریں۔ جب کا راستہ
  ٹکٹ اور استعمال شدہ ، CLI `/v1/da/manifests/<ticket>` کا مظہر ڈاؤن لوڈ کرتا ہے ،
  بنڈل `artifacts/da/fetch_<timestamp>/` کے تحت برقرار ہے (اس کے ساتھ اوور رائڈ
  `--manifest-cache-dir`) ، `--manifest-id` کے لئے بلاب ہیش کو اخذ کرتا ہے ، اور پھر
  فراہم کردہ فہرست `--gateway-provider` کے ساتھ آرکسٹریٹر چلاتا ہے۔ سب
  SoraFS فیچر کے اعلی درجے کی نوبس برقرار رہیں (مینی فیسٹ لفافے ،
  کلائنٹ لیبل ، گارڈ کیچز ، گمنام ٹرانسپورٹ اوور رائڈز ، برآمد
  اسکور بورڈ اور راستے `--output`) ، اور ظاہر اختتامی نقطہ کو ختم کیا جاسکتا ہے
  `--manifest-endpoint` کے ذریعے کسٹم Torii میزبانوں کے لئے ، لہذا چیک
  آخری سے آخر تک دستیابی کے معیارات بغیر کسی نقل کے `da` نام کی جگہ کے تحت مکمل طور پر رواں دواں ہیں
  آرکسٹریٹر منطق۔
- `iroha app da get-blob` ڈاؤن لوڈ ، اتارنا Torii سے براہ راست منشور ظاہر کرتا ہے
  `GET /v1/da/manifests/{storage_ticket}`۔ کمانڈ لکھتا ہے
  `manifest_{ticket}.norito` ، `manifest_{ticket}.json` اور
  `chunk_plan_{ticket}.json` `artifacts/da/fetch_<timestamp>/` کے تحت (یا a
  `--output-dir` صارف کے ذریعہ فراہم کردہ) عین مطابق کمانڈ پرنٹ کرتے وقت
  `iroha app da get` (بشمول `--manifest-id`) لانے کے لئے درکار ہے
  آرکیسٹریٹر۔ اس سے آپریٹرز کو منشور اسپل ڈائریکٹریوں سے دور رکھتا ہے اور
  اس بات کو یقینی بناتا ہے کہ فیچر ہمیشہ Torii کے ذریعہ جاری کردہ دستخط شدہ نمونے استعمال کرتا ہے۔ اے
  Torii جاوا اسکرپٹ کلائنٹ اسی بہاؤ کے ذریعے آئینہ کرتا ہے
  `ToriiClient.getDaManifest(storageTicketHex)` ، Norito بائٹس کو واپس کرنا
  ہائیڈریٹ کے لئے ایس ڈی کے کال کرنے والوں کے لئے ڈیکوڈ ، منشور JSON اور حصہ منصوبہ
  آرکسٹریٹر سیشن بغیر کسی CLI کا استعمال کیے۔ سوئفٹ ایس ڈی کے اب اسی کو بے نقاب کرتا ہے
  سطحیں (`ToriiClient.getDaManifestBundle(...)` مزید
  `fetchDaPayloadViaGateway(...)`) ، مقامی ریپر کو پائپنگ بنڈلSoraFS آرکسٹریٹر تاکہ iOS کلائنٹ مینی فیسٹ ڈاؤن لوڈ کرسکیں ، چلائیں
  ملٹی سورس سی ایل آئی کی درخواست کے بغیر ثبوت لاتا ہے اور شواہد حاصل کرتا ہے۔
  .
- `iroha app da rent-quote` اس کے عین مطابق کرایوں اور تفصیلات کا حساب لگاتا ہے
  فراہم کردہ اسٹوریج سائز اور برقرار رکھنے والی ونڈو کے لئے مراعات۔ مددگار
  فعال `DaRentPolicyV1` (JSON یا Norito بائٹس) یا بلٹ ان پہلے سے طے شدہ استعمال کرتا ہے ،
  پالیسی کی توثیق کرتا ہے اور JSON خلاصہ (`gib` ، `months` ، پرنٹ کرتا ہے ،
  آڈیٹرز کے لئے عین مطابق XOR کارٹونز کا حوالہ دینے کے لئے `DaRentQuote` سے پالیسی اور فیلڈز
  ایڈہاک اسکرپٹ کے بغیر گورننس منٹ میں۔ کمانڈ میں بھی ایک سمری جاری ہے
  کنسول لاگ کو برقرار رکھنے کے لئے JSON پے لوڈ سے پہلے ایک `rent_quote ...` لائن
  واقعہ کی مشقوں کے دوران پڑھنے کے قابل۔ جوڑی
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` com
  `--policy-label "governance ticket #..."` خوبصورت نمونے کو برقرار رکھنے کے لئے
  عین مطابق ووٹ یا کنفیگ بنڈل کا حوالہ دیں۔ CLI کسٹم لیبل کو کاٹتا ہے اور
  خالی تاروں سے انکار کرتا ہے تاکہ `policy_source` اقدار قابل عمل رہیں
  ٹریژری ڈیش بورڈز میں۔ دیکھو
  `crates/iroha_cli/src/commands/da.rs` subcommand اور کے لئے
  پالیسی اسکیما کے لئے `docs/source/da/rent_policy.md`۔
  .
- `iroha app da prove-availability` اوپر کی ہر چیز کو زنجیروں میں ڈالتا ہے: اسٹوریج کا ٹکٹ وصول کرتا ہے ،
  کیننیکل مینی فیسٹ بنڈل ڈاؤن لوڈ کریں ، ملٹی سورس آرکسٹریٹر چلائیں
  (`iroha app sorafs fetch`) دیئے گئے `--gateway-provider` فہرست کے خلاف ، برقرار ہے
  ڈاؤن لوڈ شدہ پے لوڈ + اسکور بورڈ `artifacts/da/prove_availability_<timestamp>/` کے تحت ،
  اور فوری طور پر موجودہ پور مددگار (`iroha app da prove`) کا استعمال کرتے ہوئے
  بائٹس ڈاؤن لوڈ. آپریٹر آرکسٹریٹر نوبس کو ایڈجسٹ کرسکتے ہیں
  (`--max-peers`, `--scoreboard-out`, manifest endpoint overrides) and the
  پروف سیمپلر (`--sample-count` ، `--leaf-index` ، `--sample-seed`) جبکہ ایک
  سنگل کمانڈ DA-5/DA-9 آڈٹ کے ذریعہ متوقع نمونے تیار کرتی ہے: کاپی
  پے لوڈ ، اسکور بورڈ ثبوت اور JSON ثبوت کے خلاصے۔