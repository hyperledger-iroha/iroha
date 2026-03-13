---
lang: ur
direction: rtl
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
`docs/source/da/ingest_plan.md` کی عکاسی کرتا ہے۔ دونوں ورژن جاری رکھیں
:::

# SORA ڈیٹا کی دستیابی INGESTION پلان Nexus

_ رائٹ: 2026-02-20-ذمہ دار: کور پروٹوکول WG / اسٹوریج ٹیم / DA WG_

DA-2 ورک اسٹریم Torii کو ایک بلاب انجشن API کے ساتھ بڑھاتا ہے جو خارج ہوتا ہے
میٹا ڈیٹا Norito اور بیج SoraFS کی نقل۔ اس دستاویز نے اسے اپنی گرفت میں لے لیا
مجوزہ اسکیم ، API سطح اور توثیق کا بہاؤ تاکہ
زیر التواء مشابہت (فالو اپس کے ذریعہ بلاک کیے بغیر عمل درآمد کی پیشرفت
DA-1)۔ تمام پے لوڈ فارمیٹس کو Norito کوڈیکس استعمال کرنا چاہئے۔ اجازت نہیں ہے
سیرڈ/JSON فال بیکس۔

## مقاصد

- بڑے بلبس کو قبول کریں (تائکائی طبقات ، لین سڈیکارس ، نمونے)
  گورننس) Torii کے ذریعے عزم کے مطابق۔
- کیننیکل Norito کے مظہروں کو تیار کریں جو بلاب ، پیرامیٹرز کے بارے میں بیان کرتے ہیں
  کوڈیک ، مٹانے والی پروفائل اور برقرار رکھنے کی پالیسی۔
- SoraFS ہاٹ اسٹوریج اور قطار SoraFS ملازمتوں میں حصہ میٹا ڈیٹا کو جاری رکھیں
  نقل
- پن کے ارادے + پالیسی ٹیگز کو رجسٹری SoraFS اور مبصرین پر شائع کریں
  گورننس کی
- داخلے کی رسیدوں کو بے نقاب کریں تاکہ مؤکلوں نے عین مطابق ثبوت کی بازیافت کریں
  اشاعت کی

## API سطح (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

پے لوڈ Norito میں ایک `DaIngestRequest` انکوڈ کیا گیا ہے۔ جوابات استعمال کرتے ہیں
`application/norito+v1` اور واپسی `DaIngestReceipt`۔

| جواب | مطلب |
| --- | --- |
| 202 قبول شدہ | chunking/نقل کے لئے قطار میں کھڑا ہونا ؛ رسید واپس کردی گئی ہے۔ |
| 400 بری درخواست | اسکیم/سائز کی خلاف ورزی (توثیق دیکھیں) |
| 401 غیر مجاز | گمشدہ/غلط API ٹوکن۔ |
| 409 تنازعہ | مماثل میٹا ڈیٹا کے ساتھ `client_blob_id` کی نقل۔ |
| 413 پے لوڈ بہت بڑا | تشکیل شدہ بلاب کی لمبائی کی حد سے تجاوز کرتا ہے۔ |
| 429 بہت ساری درخواستیں | شرح کی حد تک پہنچ گئی ہے۔ |
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

> عمل درآمد نوٹ: ان کے لئے زنگ میں کیننیکل نمائندگی
> پے لوڈ اب ریپروں کے ساتھ ، `iroha_data_model::da::types` کے تحت رہتے ہیں
> `iroha_data_model::da::ingest` اور مینی فیسٹ ڈھانچے میں درخواست/رسید
> `iroha_data_model::da::manifest` میں۔

`compression` فیلڈ نے اعلان کیا کہ کال کرنے والوں نے پے لوڈ کو کس طرح تیار کیا۔ Torii
`identity` ، `gzip` ، `deflate` ، اور `zstd` کو قبول کرتا ہے ، اس سے پہلے بائٹس کو ڈیکپریس کرتے ہوئے
ہیشنگ ، چنکنگ اور اختیاری توثیق کی توثیق کرنا۔

### توثیق چیک لسٹ1. تصدیق کریں کہ درخواست کا ہیڈر Norito `DaIngestRequest` سے ملتا ہے۔
2. اگر `total_size` پے لوڈ کی کیننیکل لمبائی (غیر سنجیدہ) سے مختلف ہے تو ناکام ہوجائیں۔
   یا تشکیل شدہ زیادہ سے زیادہ سے زیادہ ہے۔
3. Norito کی فورس سیدھ (دو کی طاقت ،  = 2۔
5. `retention_policy.required_replica_count` کو لازمی طور پر بیس لائن کا احترام کرنا چاہئے
   گورننس
6. کیننیکل ہیش کے خلاف دستخطی توثیق (دستخطی فیلڈ کو چھوڑ کر)۔
7. ڈپلیکیٹ `client_blob_id` کو مسترد کریں جب تک کہ پے لوڈ ہیش اور
   میٹا ڈیٹا ایک جیسے ہیں۔
8. جب `norito_manifest` فراہم کیا جاتا ہے تو ، چیک اسکیما + ہیش سے ملتے ہیں
   منشور chunking کے بعد دوبارہ گنتی ؛ ورنہ نوڈ پیدا کرتا ہے
   ظاہر اور اسٹور کرتا ہے۔
9. تشکیل شدہ نقل کی پالیسی پر مجبور کریں: Torii کو دوبارہ لکھیں
   `RetentionPolicy` `torii.da_ingest.replication_policy` کے ساتھ بھیج دیا گیا (دیکھیں
   `replication-policy.md`) اور پری بلٹ منشور کو مسترد کرتا ہے جس کے میٹا ڈیٹا
   ود ہولڈنگ مسلط پروفائل سے مماثل نہیں ہے۔

### chunking اور نقل بہاؤ

1. پے لوڈ کو `chunk_size` میں چنک کریں ، بلیک 3 کا حساب کتاب + مرکل روٹ کے ذریعہ کریں۔
2. Norito `DaManifestV1` (نیا ڈھانچہ) پر قبضہ کرنے کے وعدوں کو بنائیں
   حصہ (کردار/گروپ_ایڈ) ، مٹانے کی ترتیب (قطار کی برابری کی گنتی اور
   کالم پلس `ipa_commitment`) ، برقرار رکھنے کی پالیسی اور میٹا ڈیٹا۔
3. قطار کے تحت کیننیکل منشور بائٹس کی قطار لگائیں
   `config.da_ingest.manifest_store_dir` (Torii فائلیں لکھتا ہے
   `manifest.encoded` بذریعہ لین/ایپوچ/ترتیب/ٹکٹ/فنگر پرنٹ) تاکہ
   SoraFS کا آرکیسٹریشن ان کو کھاتا ہے اور اسٹوریج کے ٹکٹ کو ڈیٹا سے جوڑتا ہے
   برقرار رہا۔
4. گورننس ٹیگ کے ساتھ `sorafs_car::PinIntent` کے ذریعے پن کے ارادے شائع کریں اور
   سیاست
5. جاری کردہ واقعہ Norito `DaIngestPublished` مبصرین (مؤکلوں کو مطلع کرنے کے لئے
   روشنی ، گورننس ، تجزیات)۔
6. کالر کو `DaIngestReceipt` واپس کریں (DA سروس کی کے ذریعہ دستخط شدہ
   Torii) اور ہیڈر `Sora-PDP-Commitment` خارج کریں تاکہ SDKs کو گرفت میں لے لیں
   عزم فوری طور پر انکوڈ ہوگیا۔ رسید میں اب `rent_quote` شامل ہے
   (ایک Norito `DaRentQuote`) اور `stripe_layout` ، بھیجنے والوں کی اجازت دیتے ہیں
   بیس انکم ، ریزرو ، PDP/POTR بونس کی توقعات اور دکھائیں
   فنڈز کا ارتکاب کرنے سے پہلے اسٹوریج کے ٹکٹ کے آگے 2D لے آؤٹ کو مٹا دیں۔

## اسٹوریج/رجسٹری کی تازہ کاری

- Norito کے ساتھ `sorafs_manifest` میں توسیع کریں ، تجزیہ کو چالو کریں
  تعصب پسند
- نئی رجسٹری اسٹریم `da.pin_intent` کو ورژن والے پے لوڈ کے ساتھ شامل کریں
  منشور ہیش ریفرنس + ٹکٹ ID۔
- ادخال لیٹینسی کو ٹریک کرنے کے لئے مشاہدہ کرنے والی پائپ لائنوں کو اپ ڈیٹ کریں ،
  چنکنگ تھروپپٹ ، نقل کا بیکلاگ اور ناکامی کی گنتی۔

## جانچ کی حکمت عملی- اسکیما کی توثیق ، ​​دستخطی چیک اور اس کا پتہ لگانے کے لئے یونٹ ٹیسٹ
  نقل
- Norito `DaIngestRequest` ، منشور اور کی تصدیق کرنے والے گولڈن ٹیسٹ
  رسید
- انضمام کا استعمال SoraFS + مصنوعی رجسٹری ، توثیق کرنا
  حصہ + پن بہاؤ۔
- پراپرٹی ٹیسٹ مٹانے والے پروفائلز اور کے امتزاج پر محیط
  بے ترتیب برقرار رکھنا۔
- خراب شدہ میٹا ڈیٹا سے بچانے کے لئے Norito پے لوڈ کا فوزنگ۔

## CLI & SDK ٹولنگ (DA-8)- `iroha app da submit` (نیا CLI انٹری پوائنٹ) اب بلڈر/پبلشر کو لپیٹتا ہے
  انجیکشن شیئرنگ تاکہ آپریٹرز صوابدیدی بلابز کھا سکتے ہیں
  تائکائی بنڈل کے بہاؤ کے باہر۔ کمانڈ میں رہتا ہے
  `crates/iroha_cli/src/commands/da.rs:1` اور ایک پے لوڈ ، پروفائل استعمال کرتا ہے
  دستخط کرنے سے پہلے مٹانے/برقرار رکھنے اور اختیاری میٹا ڈیٹا/مینی فیسٹ فائلیں
  CLI کنفیگ کلید کے ساتھ کیننیکل `DaIngestRequest`۔ پھانسی
  کامیاب `da_request.{norito,json}` اور `da_receipt.{norito,json}` کے تحت برقرار ہے
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` کے ذریعے اوور رائڈ) تاکہ
  ریلیز نمونے کے دوران استعمال ہونے والے عین مطابق Norito بائٹس کو ریکارڈ کریں
  انٹیک
- کمانڈ `client_blob_id = blake3(payload)` کو بطور ڈیفالٹ استعمال کرتا ہے لیکن قبول کرتا ہے
  `--client-blob-id` کے ذریعے اوور رائڈس ، JSON میٹا ڈیٹا میپس کا احترام کرتا ہے
  (`--metadata-json`) اور پہلے سے تیار کردہ ظاہر (`--manifest`) ، اور سپورٹ
  `--no-submit` آف لائن تیاری کے علاوہ `--endpoint` کے لئے میزبان Torii کے لئے
  ذاتی نوعیت کا JSON رسید لکھنے کے علاوہ STDOUT پر بھی پرنٹ کی گئی ہے
  ڈسک ، DA-8 "submis_blob" ٹولنگ کی ضرورت کو بند کرنا اور انلاک کرنا
  ایس ڈی کے برابری کا کام۔
-`iroha app da get` ملٹی سورس آرکیسٹریٹر کے لئے ڈی اے پر مبنی عرف شامل کرتا ہے
  جو پہلے ہی `iroha app sorafs fetch` کو طاقت دیتا ہے۔ آپریٹرز اس کی طرف اشارہ کرسکتے ہیں
  منشور + چنک پلان پلان نمونے (`--manifest` ، `--plan` ، `--manifest-id`)
  ** یا ** Torii سے `--storage-ticket` کے ذریعے اسٹوریج ٹکٹ پاس کریں۔ جب استعمال کیا جائے
  ٹکٹ کا راستہ ، CLI `/v2/da/manifests/<ticket>` سے ظاہر ہوتا ہے ،
  بنڈل `artifacts/da/fetch_<timestamp>/` کے تحت برقرار ہے (اس کے ساتھ اوور رائڈ
  `--manifest-cache-dir`) ، بلاب ہیش کو `--manifest-id` پر اخذ کریں ، اور پھر
  فراہم کردہ `--gateway-provider` فہرست کے ساتھ آرکسٹریٹر چلاتا ہے۔ سب
  SoraFS ایڈوانسڈ فیچر نوبس برقرار رہتا ہے (منشور
  لفافے ، کسٹمر لیبل ، گارڈ کیچز ، گمنام ٹرانسپورٹ اوور رائڈز ،
  اسکور بورڈ اور راستے `--output` کی برآمد) ، اور ظاہر اختتامی نقطہ
  کسٹم Torii میزبانوں کے لئے بھی `--manifest-endpoint` کے ذریعے اوور رائٹ کیا گیا ہے
  کہ آخری سے آخر تک دستیابی کی جانچ پڑتال مکمل طور پر نام کی جگہ کے تحت زندہ رہتی ہے
  `da` بغیر آرکیسٹریٹر منطق کی نقل کے بغیر۔
- `iroha app da get-blob` ڈاؤن لوڈ ، اتارنا Torii سے براہ راست کیننیکل منشور
  `GET /v2/da/manifests/{storage_ticket}`۔ کمانڈ لکھتا ہے
  `manifest_{ticket}.norito` ، `manifest_{ticket}.json` اور
  `chunk_plan_{ticket}.json` `artifacts/da/fetch_<timestamp>/` کے تحت (یا a
  `--output-dir` صارف کے ذریعہ فراہم کردہ) عین مطابق کمانڈ پرنٹ کرتے وقت
  `iroha app da get` (بشمول `--manifest-id`) کی بازیافت کے لئے ضروری ہے
  آرکسٹریٹر یہ آپریٹرز کو اسپلڈ ڈائریکٹریوں سے دور رکھتا ہے۔
  ظاہر ہوتا ہے اور اس بات کو یقینی بناتا ہے کہ بازیافت ہمیشہ دستخط شدہ نمونے استعمال کرتا ہے
  Torii کے ذریعہ جاری کیا گیا۔ جاوا اسکرپٹ کلائنٹ Torii اسی بہاؤ کے ذریعے آئینہ کرتا ہے
  `ToriiClient.getDaManifest(storageTicketHex)` ، بائٹس واپس کرنا Norito
  ہائیڈریٹ کے لئے ایس ڈی کے کال کرنے والوں کے لئے ڈیکوڈ ، منشور JSON اور حصہ منصوبہ
  آرکسٹریٹر سیشن بغیر کسی CLI کا استعمال کیے۔ سوئفٹ SDK اب بے نقاب کرتا ہے
  اسی سطحوں (`ToriiClient.getDaManifestBundle(...)` مزید`fetchDaPayloadViaGateway(...)`) ، کے آبائی ریپر کو بنڈل چینل کرتے ہوئے
  SoraFS آرکسٹریٹر تاکہ iOS کلائنٹ مینی فیسٹ ڈاؤن لوڈ کرسکیں ، چلائیں
  ملٹی سورس سی ایل آئی کی درخواست کے بغیر ٹیسٹوں کو بازیافت اور گرفتاری کے ٹیسٹ کرتا ہے۔
  .
- `iroha app da rent-quote` عزم آمدنی اور ترغیبی خرابی کا حساب لگاتا ہے
  فراہم کردہ اسٹوریج سائز اور برقرار رکھنے والی ونڈو کے لئے۔ مددگار
  فعال `DaRentPolicyV1` (JSON یا Norito بائٹس) یا بلٹ ان پہلے سے طے شدہ استعمال کرتا ہے ،
  پالیسی کی توثیق کرتا ہے اور JSON سمری پرنٹ کرتا ہے (`gib` ، `months` ، میٹا ڈیٹا
  پالیسی اور `DaRentQuote` فیلڈز) آڈیٹرز کے لئے عین مطابق XOR چارجز کا حوالہ دیتے ہیں
  ایڈہاک اسکرپٹ کے بغیر گورننس منٹ میں۔ کمانڈ بھی ایک خلاصہ پیش کرتا ہے
  JSON پے لوڈ سے پہلے ایک لائن میں `rent_quote ...`
  واقعہ کی مشقوں کے دوران کنسول لاگز۔ جوڑی
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` کے ساتھ
  `--policy-label "governance ticket #..."` خوبصورت نمونے کو برقرار رکھنے کے لئے
  عین مطابق ووٹ یا کنفیگ بنڈل کا حوالہ دیں۔ سی ایل آئی کسٹم لیبل کو ٹرم کرتا ہے
  اور خالی تاروں کو مسترد کرتا ہے تاکہ `policy_source` اقدار کو رکھا جائے
  ٹریژری ڈیش بورڈز میں قابل عمل۔ دیکھو
  `crates/iroha_cli/src/commands/da.rs` subcommand اور کے لئے
  پالیسی اسکیم کے لئے `docs/source/da/rent_policy.md`۔
  .
- `iroha app da prove-availability` مندرجہ بالا سبھی زنجیریں: اسٹوریج لیتا ہے
  ٹکٹ ، کیننیکل مینی فیسٹ بنڈل ڈاؤن لوڈ کریں ، آرکسٹریٹر چلائیں
  فہرست `--gateway-provider` کے خلاف ملٹی سورس (`iroha app sorafs fetch`)
  فراہم کردہ ، ڈاؤن لوڈ شدہ پے لوڈ برقرار رہتا ہے + کم اسکور بورڈ
  `artifacts/da/prove_availability_<timestamp>/` ، اور فوری طور پر مددگار کو طلب کرتا ہے
  لائے ہوئے بائٹس کا استعمال کرتے ہوئے موجودہ پور (`iroha app da prove`)۔ آپریٹرز
  آرکسٹریٹر نوبس کو ایڈجسٹ کرسکتے ہیں (`--max-peers` ، `--scoreboard-out` ،
  مینیفیسٹ اینڈ پوائنٹ اوور رائڈز) اور پروف سیمپلر (`--sample-count` ،
  `--leaf-index` ، `--sample-seed`) جبکہ ایک ہی کمانڈ تیار کرتا ہے
  DA-5/DA-9 آڈٹ کے ذریعہ متوقع نمونے: پے لوڈ کی کاپی ، اس کے ثبوت
  اسکور بورڈ اور JSON ٹیسٹ کے خلاصے۔