---
lang: ur
direction: rtl
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T15:38:30.661072+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SORA Nexus ڈیٹا کی دستیابی انجسٹ پلان

_ ڈرافٹڈ: 2026-02-20-مالک: کور پروٹوکول WG / اسٹوریج ٹیم / DA WG_

DA-2 ورک اسٹریم Torii کو ایک بلاب Ingest API کے ساتھ بڑھاتا ہے جو Norito کو خارج کرتا ہے
میٹا ڈیٹا اور بیج SoraFS نقل۔ اس دستاویز نے مجوزہ کو اپنی گرفت میں لے لیا
اسکیما ، API سطح ، اور توثیق کا بہاؤ تاکہ عمل درآمد بغیر آگے بڑھ سکتا ہے
بقایا تخروپن (DA-1 فالو اپس) پر مسدود کرنا۔ تمام پے لوڈ فارمیٹس کو لازمی ہے
Norito کوڈیکس استعمال کریں۔ کسی سیرڈ/JSON فال بیکس کی اجازت نہیں ہے۔

## اہداف

- بڑے بلبس کو قبول کریں (تائکائی طبقات ، لین سڈیکارس ، گورننس نوادرات)
  Torii سے زیادہ عزم کے مطابق۔
- بلاب ، کوڈیک پیرامیٹرز کو بیان کرنے والے کیننیکل Norito کے مظہر کی تیاری کریں ،
  مٹانے والا پروفائل ، اور برقرار رکھنے کی پالیسی۔
- SoraFS ہاٹ اسٹوریج اور enqueue نقل کی نوکریوں میں حصہ میٹا ڈیٹا کو برقرار رکھیں۔
- SoraFS رجسٹری اور گورننس میں پن کے ارادے + پالیسی ٹیگز شائع کریں
  مبصرین
- داخلے کی رسیدوں کو بے نقاب کریں تاکہ کلائنٹ اشاعت کے عین مطابق ثبوت کو دوبارہ حاصل کریں۔

## API سطح (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

پے لوڈ ایک Norito-encoded `DaIngestRequest` ہے۔ جوابات استعمال کرتے ہیں
`application/norito+v1` اور واپسی `DaIngestReceipt`۔

| جواب | مطلب |
| --- | --- |
| 202 قبول شدہ | ٹکرانے/نقل کے لئے بلاب قطار میں کھڑا ؛ رسید واپس آگئی۔ |
| 400 بری درخواست | اسکیما/سائز کی خلاف ورزی (توثیق کی جانچ پڑتال دیکھیں)۔ |
| 401 غیر مجاز | گمشدہ/غلط API ٹوکن۔ |
| 409 تنازعہ | مماثل میٹا ڈیٹا کے ساتھ `client_blob_id` کی نقل۔ |
| 413 پے لوڈ بہت بڑا | تشکیل شدہ بلاب کی لمبائی کی حد سے تجاوز کرتا ہے۔ |
| 429 بہت ساری درخواستیں | شرح کی حد ہٹ۔ |
| 500 داخلی غلطی | غیر متوقع ناکامی (لاگ ان + الرٹ)۔ |

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

موجودہ لین کیٹلاگ سے اخذ کردہ ایک ورژن شدہ `DaProofPolicyBundle` واپس کرتا ہے۔
بنڈل میں `version` (فی الحال `1`) ، `policy_hash` (کی ہیش کی تشہیر کی گئی ہے
پالیسی کی فہرست کی فہرست) ، اور `policies` اندراجات `lane_id` ، `dataspace_id` ،
`alias` ، اور نافذ `proof_scheme` (`merkle_sha256` آج ؛ KZG لین ہیں
جب تک کے زیڈ جی کے وعدے دستیاب نہیں ہوتے ہیں)۔ اب بلاک ہیڈر
`da_proof_policies_hash` کے توسط سے بنڈل سے وابستگی کرتا ہے ، تاکہ کلائنٹ اس پن کو پن کرسکیں
ڈی اے کے وعدوں یا ثبوتوں کی تصدیق کرتے وقت فعال پالیسی سیٹ کریں۔ اس اختتامی نقطہ کو لائیں
اس بات کو یقینی بنانے کے لئے ثبوت بنانے سے پہلے کہ وہ لین کی پالیسی اور کرنٹ سے مماثل ہوں
بنڈل ہیش عزم کی فہرست/ثابت اختتامی نکات ایک ہی بنڈل کو لے کر SDKs رکھتے ہیں
فعال پالیسی سیٹ کے ثبوت کو پابند کرنے کے لئے اضافی راؤنڈ ٹرپ کی ضرورت نہیں ہے۔

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

آرڈرڈ پالیسی لسٹ کے علاوہ ایک `DaProofPolicyBundle` واپس کرتا ہے
`policy_hash` تاکہ SDKs جب بلاک تیار کیا گیا تو استعمال شدہ ورژن کو پن کرسکیں۔
Norito-encoded پالیسی سرنی پر ہیش کی گنتی کی جاتی ہے اور جب بھی a
لین کا `proof_scheme` اپ ڈیٹ ہے ، جس سے مؤکلوں کو اس کے درمیان بہاؤ کا پتہ لگانے کی اجازت ہے
کیشڈ ثبوت اور چین کی تشکیل۔

## تجویز کردہ Norito اسکیما

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Prometheus lane
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
    pub manifest_hash: BlobDigest,      // Prometheus manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Sumeragi-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```> عمل درآمد نوٹ: ان پے لوڈ کے لئے اب تک کی زنگ کی نمائندگی کے تحت زندہ رہتے ہیں
> `iroha_data_model::da::types` ، `iroha_data_model::da::ingest` میں درخواست/رسید ریپرس کے ساتھ
> اور `iroha_data_model::da::manifest` میں ظاہر ڈھانچہ۔

`compression` فیلڈ یہ اشتہار دیتا ہے کہ کال کرنے والوں نے پے لوڈ کو کس طرح تیار کیا۔ Torii قبول کرتا ہے
`identity` ، `gzip` ، `deflate` ، اور `zstd` ، شفاف طور پر اس سے پہلے بائٹس کو ڈیکم کرتے ہیں
ہیشنگ ، چنکنگ ، اور اختیاری ظاہر کی توثیق کرنا۔

### توثیق چیک لسٹ

1. درخواست کی تصدیق Norito ہیڈر میچ `DaIngestRequest` سے ملتی ہے۔
2. اگر `total_size` پے لوڈ کی لمبائی کی لمبائی سے مختلف ہے یا تشکیل شدہ میکس سے زیادہ ہے تو ناکام ہوجائیں۔
3. `chunk_size` سیدھ کو نافذ کریں (پاور آف دو ،  = 2۔
5. `retention_policy.required_replica_count` کو گورننس بیس لائن کا احترام کرنا چاہئے۔
6. کیننیکل ہیش کے خلاف دستخطی توثیق (دستخطی فیلڈ کو چھوڑ کر)۔
7. ڈپلیکیٹ `client_blob_id` کو مسترد کریں جب تک کہ پے لوڈ ہیش + میٹا ڈیٹا ایک جیسی نہ ہو۔
8. جب `norito_manifest` فراہم کی گئی ہو تو ، اسکیما + ہیش میچوں کی تصدیق کریں
   ٹکرانے کے بعد ظاہر ؛ بصورت دیگر نوڈ ظاہر کرتا ہے اور اسے اسٹور کرتا ہے۔
9. تشکیل شدہ نقل کی پالیسی کو نافذ کریں: Torii جمع کروائے گئے کو دوبارہ لکھتا ہے
   `RetentionPolicy` `torii.da_ingest.replication_policy` کے ساتھ (دیکھیں
   `replication_policy.md`) اور پہلے سے تعمیر شدہ ظاہر ہوتا ہے جس کی برقراری
   میٹا ڈیٹا نافذ شدہ پروفائل سے مماثل نہیں ہے۔

### chunking اور نقل بہاؤ1. `chunk_size` میں حصہ پے لوڈ ، کمپیوٹ بلیک 3 فی حصہ + مرکل روٹ۔
2. Norito `DaManifestV1` (نیا ڈھانچہ) کو حاصل کرنے والے CHUNK کے وعدوں (رول/گروپ_ایڈ) کی تعمیر کریں ،
   ایریشور لے آؤٹ (قطار اور کالم پیریٹی گنتی کے علاوہ `ipa_commitment`) ، برقرار رکھنے کی پالیسی ،
   اور میٹا ڈیٹا۔
3. `config.da_ingest.manifest_store_dir` کے تحت کیننیکل منشور بائٹس کی قطار لگائیں
   (Torii writes `manifest.encoded` files keyed by lane/epoch/sequence/ticket/fingerprint) so SoraFS
   آرکیسٹریشن ان کو کھا سکتا ہے اور اسٹوریج کے ٹکٹ کو مستقل ڈیٹا سے جوڑ سکتا ہے۔
4. گورننس ٹیگ + پالیسی کے ساتھ `sorafs_car::PinIntent` کے ذریعے پن کے ارادوں کو شائع کریں۔
5. Norito واقعہ `DaIngestPublished` مبصرین کو مطلع کرنے کے لئے (ہلکے کلائنٹ ،
   گورننس ، تجزیات)۔
6. واپسی `DaIngestReceipt` (Torii DA سروس کی کے ذریعہ دستخط شدہ) اور شامل کریں
   `Sora-PDP-Commitment` رسپانس ہیڈر جس میں BASE64 Norito انکوڈنگ ہے
   اخذ کردہ عزم میں سے ایس ڈی کے نمونے لینے والے بیج کو فوری طور پر اسٹش کرسکتے ہیں۔
   رسید اب `rent_quote` (A `DaRentQuote`) اور `stripe_layout` کو سرایت کرتی ہے
   لہذا جمع کرانے والے XOR کی ذمہ داریوں ، ریزرو شیئر ، PDP/POTR بونس کی توقعات کو پورا کرسکتے ہیں ،
   اور فنڈز کا ارتکاب کرنے سے پہلے اسٹوریج ٹکٹ میٹا ڈیٹا کے ساتھ ساتھ 2D مٹانے والے میٹرکس کے طول و عرض۔
7. اختیاری رجسٹری میٹا ڈیٹا:
   - `da.registry.alias`- پبلک ، غیر خفیہ کردہ UTF-8 عرف اسٹرنگ کو پن رجسٹری کے اندراج کو بیج کرنے کے لئے۔
   - `da.registry.owner` - عوامی ، غیر خفیہ کردہ `AccountId` اسٹرنگ رجسٹری کی ملکیت کو ریکارڈ کرنے کے لئے۔
   Torii ان کو تیار کردہ `DaPinIntent` میں کاپی کرتا ہے لہذا بہاو پن پروسیسنگ عرفی کو باندھ سکتا ہے
   اور مالکان کچے میٹا ڈیٹا کے نقشے کو دوبارہ پارس کیے بغیر ؛ خراب یا خالی اقدار کے دوران مسترد کردیئے جاتے ہیں
   Ingest توثیق.

## اسٹوریج / رجسٹری کی تازہ کاری

- `sorafs_manifest` کو `DaManifestV1` کے ساتھ بڑھائیں ، جو عین مطابق تجزیہ کو چالو کریں۔
- نئی رجسٹری اسٹریم `da.pin_intent` کو ورژن شدہ پے لوڈ حوالہ کے ساتھ شامل کریں
  منشور ہیش + ٹکٹ ID۔
- آئیننگ لیٹینسی کو ٹریک کرنے کے لئے مشاہدہ کرنے والی پائپ لائنوں کو اپ ڈیٹ کریں ، چنکنگ تھرو پٹ ،
  نقل کا بیک بلاگ ، اور ناکامی کا شمار۔
- Torii `/status` جوابات میں اب ایک `taikai_ingest` سرنی شامل ہے جو تازہ ترین سطح پر ہے
  انکوڈر سے آنے والے دیر سے لیٹینسی ، براہ راست کنارے کا بہاؤ ، اور غلطی کاؤنٹرز فی (کلسٹر ، اسٹریم) ، ڈی اے 9 کو فعال کرنا
  Prometheus کو کھرچنے کے بغیر براہ راست نوڈس سے صحت کے اسنیپ شاٹس کو ڈیش بورڈز۔

## جانچ کی حکمت عملی- اسکیما کی توثیق ، ​​دستخطی چیک ، ڈپلیکیٹ کا پتہ لگانے کے لئے یونٹ ٹیسٹ۔
- `DaIngestRequest` ، منشور ، اور رسید کے Norito انکوڈنگ کی تصدیق کرنے والے گولڈن ٹیسٹ۔
- انضمام کا استعمال مذاق SoraFS + رجسٹری کو گھماؤ ، جس میں CHUNK + پن کے بہاؤ پر زور دیا گیا ہے۔
- پراپرٹی ٹیسٹ بے ترتیب مٹانے والے پروفائلز اور برقرار رکھنے کے امتزاج پر محیط۔
- خراب شدہ میٹا ڈیٹا کے خلاف حفاظت کے لئے Norito پے لوڈ کا فوزنگ۔
- ہر بلاب کلاس کے لئے گولڈن فکسچر زندہ رہتے ہیں
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` ایک ساتھی کے ساتھ
  `fixtures/da/ingest/sample_chunk_records.txt` میں فہرست سازی۔ نظرانداز ٹیسٹ
  `regenerate_da_ingest_fixtures` فکسچر کو تازہ دم کرتا ہے ، جبکہ
  `manifest_fixtures_cover_all_blob_classes` جیسے ہی ایک نیا `BlobClass` مختلف حالت شامل کی جائے
  Norito/JSON بنڈل کو اپ ڈیٹ کیے بغیر۔ اس سے Torii ، SDKs ، اور دستاویزات ایماندار رہتے ہیں جب بھی DA-2
  ایک نئی بلاب کی سطح کو قبول کرتا ہے۔ 【فکسچر/دا/انجسٹ/ریڈیم۔

## CLI & SDK ٹولنگ (DA-8)- `iroha app da submit` (نیا CLI انٹریپوائنٹ) اب مشترکہ ingest بلڈر/پبلشر کو لپیٹتا ہے لہذا آپریٹرز
  تائکائی بنڈل بہاؤ کے باہر صوابدیدی بلابز کھا سکتے ہیں۔ کمانڈ میں رہتا ہے
  `crates/iroha_cli/src/commands/da.rs:1` اور ایک پے لوڈ ، مٹانے/برقرار رکھنے کا پروفائل ، اور استعمال کرتا ہے
  CLI کے ساتھ کیننیکل `DaIngestRequest` پر دستخط کرنے سے پہلے اختیاری میٹا ڈیٹا/مینی فیسٹ فائلیں
  تشکیل کلید۔ کامیاب رنز `da_request.{norito,json}` اور `da_receipt.{norito,json}` کے تحت برقرار رہتے ہیں
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` کے ذریعے اوور رائڈ) لہذا ریلیز نوادرات کر سکتے ہیں
  عین مطابق Norito بائٹس کو انجیکشن کے دوران استعمال کیا جاتا ہے۔
- کمانڈ `client_blob_id = blake3(payload)` میں ڈیفالٹ کرتا ہے لیکن اس کے ذریعے اوور رائڈس کو قبول کرتا ہے
  `--client-blob-id` ، آنرز میٹا ڈیٹا JSON نقشہ (`--metadata-json`) اور پہلے سے تیار کردہ ظاہر
  .
  Torii میزبان۔ رسید JSON ڈسک پر لکھے جانے کے علاوہ ، STDOUT پر چھپی ہوئی ہے ، بند کردی گئی ہے
  DA-8 "subme_blob" ٹولنگ کی ضرورت اور SDK پیریٹی کام کو غیر مسدود کرنا۔
-`iroha app da get` ملٹی سورس آرکیسٹریٹر کے لئے DA پر مبنی عرف شامل کرتا ہے جو پہلے ہی طاقتوں کو حاصل کرتا ہے
  `iroha app sorafs fetch`۔ آپریٹرز اسے منشور + chunk-plan کے نوادرات (`--manifest` ، پر اشارہ کرسکتے ہیں
  `--plan` ، `--manifest-id`) ** یا ** `--storage-ticket` کے ذریعے Torii اسٹوریج ٹکٹ کو آسانی سے پاس کریں۔ جب
  ٹکٹ کا راستہ استعمال کیا جاتا ہے CLI `/v1/da/manifests/<ticket>` سے ظاہر ہوتا ہے ، بنڈل برقرار رہتا ہے
  `artifacts/da/fetch_<timestamp>/` کے تحت (`--manifest-cache-dir` کے ساتھ اوور رائڈ) ، ** مینی فیسٹ سے ماخوذ ہے
  `--manifest-id` کے لئے ہیش ** ، اور پھر فراہم کردہ `--gateway-provider` کے ساتھ آرکسٹریٹر چلاتا ہے
  فہرست پے لوڈ کی توثیق اب بھی ایمبیڈڈ کار/`blob_hash` ڈائجسٹ پر انحصار کرتی ہے جبکہ گیٹ وے ID ہے
  اب مینی فیسٹ ہیش تاکہ کلائنٹ اور توثیق کار ایک ہی بلاب شناخت کنندہ کا اشتراک کرتے ہیں۔ سے تمام جدید نوبس
  SoraFS فیچر سطح برقرار (مینی فیسٹ لفافے ، کلائنٹ لیبل ، گارڈ کیچز ، گمنامی کی نقل و حمل
  اوور رائڈس ، اسکور بورڈ ایکسپورٹ ، اور `--output` راستے) ، اور ظاہر اختتامی نقطہ کے ذریعے اوورراڈ کیا جاسکتا ہے
  `--manifest-endpoint` کسٹم Torii میزبانوں کے لئے ، لہذا آخر سے آخر تک دستیابی چیک مکمل طور پر رب کے تحت زندہ رہتی ہے
  `da` نام کی جگہ بغیر کسی آرکیسٹریٹر منطق کی نقل کے۔
- `iroha app da get-blob` Torii سے `GET /v1/da/manifests/{storage_ticket}` کے ذریعے سیدھے کیننیکل منشور کو کھینچتا ہے۔
  کمانڈ اب مینی فیسٹ ہیش (بلب ID) کے ساتھ نوادرات کا لیبل لگا دیتا ہے ،
  `manifest_{manifest_hash}.norito` ، `manifest_{manifest_hash}.json` ، اور `chunk_plan_{manifest_hash}.json`
  `artifacts/da/fetch_<timestamp>/` (یا صارف فراہم کردہ `--output-dir`) کے تحت عین مطابق گونجتے ہوئے
  `iroha app da get` درخواست (بشمول `--manifest-id`) فالو اپ آرکسٹریٹر بازیافت کے لئے ضروری ہے۔
  یہ آپریٹرز کو مینی فیسٹ اسپل ڈائریکٹریوں سے دور رکھتا ہے اور اس بات کی ضمانت دیتا ہے کہ بازیافت ہمیشہ استعمال کرتا ہے
  Torii کے ذریعہ خارج ہونے والے دستخط شدہ نوادرات۔ جاوا اسکرپٹ Torii کلائنٹ اس بہاؤ کے ذریعے آئینہ دار ہے
  `ToriiClient.getDaManifest(storageTicketHex)` جبکہ سوئفٹ SDK اب بے نقاب ہوتا ہے
  `ToriiClient.getDaManifestBundle(...)`۔ دونوں ڈیکوڈڈ Norito بائٹس ، منشور JSON ، منشور ہیش لوٹاتے ہیں ،اور حصہ منصوبہ ہے تاکہ ایس ڈی کے کال کرنے والے آرکیسٹریٹر سیشنوں کو سی ایل آئی اور سوئفٹ میں گولہ باری کے بغیر ہائیڈریٹ کرسکتے ہیں۔
  کلائنٹ اضافی طور پر `fetchDaPayloadViaGateway(...)` کو مقامی کے ذریعہ ان بنڈلوں کو پائپ کرنے کے لئے کال کرسکتے ہیں
  SoraFS آرکیسٹریٹر ریپر۔
- `/v1/da/manifests` جوابات اب سطح `manifest_hash` ، اور دونوں CLI + SDK مددگار (`iroha app da get` ،
  `ToriiClient.fetchDaPayloadViaGateway` ، اور سوئفٹ/جے ایس گیٹ وے ریپرز) اس ڈائجسٹ کو بطور سلوک کریں
  ایمبیڈڈ کار/بلاب ہیش کے خلاف پے لوڈ کی تصدیق کرتے ہوئے کیننیکل مینی فیسٹ شناخت کنندہ۔
- `iroha app da rent-quote` فراہم کردہ اسٹوریج کے سائز کے ل det ڈٹرمینسٹک کرایہ اور ترغیبی خرابی کی گنتی کرتا ہے
  اور برقرار رکھنے والی ونڈو۔ مددگار یا تو فعال `DaRentPolicyV1` (JSON یا Norito بائٹس) یا استعمال کرتا ہے
  بلٹ ان ڈیفالٹ ، پالیسی کی توثیق کرتا ہے ، اور JSON خلاصہ (`gib` ، `months` ، پالیسی میٹا ڈیٹا ، پرنٹ کرتا ہے ،
  اور `DaRentQuote` فیلڈز) تاکہ آڈیٹر گورننس منٹ کے اندر عین مطابق XOR چارجز کا حوالہ دے سکیں
  ایڈہاک اسکرپٹ لکھنا۔ کمانڈ اب JSON سے پہلے ایک لائن `rent_quote ...` کا خلاصہ بھی خارج کرتا ہے
  جب واقعات کے دوران قیمت درج کی جاتی ہے تو کنسول لاگز اور رن بکس بنانے کے لئے پے لوڈ۔
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` (یا کوئی دوسرا راستہ) پاس کریں
  خوبصورت پرنٹ شدہ سمری کو برقرار رکھنے اور جب `--policy-label "governance ticket #..."` استعمال کریں
  نوادرات کو ایک مخصوص ووٹ/کنفیگ بنڈل پیش کرنے کی ضرورت ہے۔ سی ایل آئی کسٹم لیبل کو تراشتا ہے اور خالی جگہ کو مسترد کرتا ہے
  `policy_source` اقدار کو ثبوت کے بنڈل میں معنی خیز رکھنے کے لئے ڈور۔ دیکھو
  `crates/iroha_cli/src/commands/da.rs` سب کامنڈ اور `docs/source/da/rent_policy.md` کے لئے
  پالیسی اسکیمہ کے لئے۔ 【کریٹس/اروہ_ سی ایل آئی/ایس آر سی/کمانڈز/ڈی اے آر ایس: 1 】【 دستاویزات/ماخذ/ڈا/کرایہ_پولیسی.MD: 1】
- پن رجسٹری کی برابری اب SDKs تک پھیلی ہوئی ہے: `ToriiClient.registerSorafsPinManifest(...)` میں
  جاوا اسکرپٹ ایس ڈی کے نے عین مطابق پے لوڈ کی تعمیر کی ہے جو `iroha app sorafs pin register` کے ذریعہ استعمال ہوتا ہے ، کیننیکل کو نافذ کرتا ہے
  چنکر میٹا ڈیٹا ، پن کی پالیسیاں ، عرفی ثبوت ، اور جانشین پوسٹ کرنے سے پہلے ہضم ہوتا ہے
  `/v1/sorafs/pin/register`۔ اس سے سی آئی بوٹس اور آٹومیشن کو جب سی ایل آئی میں گولہ باری سے روکتا ہے
  ریکارڈنگ مینی فیسٹ رجسٹریشن ، اور ٹائپ اسکرپٹ/ریڈم کوریج کے ساتھ مددگار جہاز
  زنگ/سوئفٹ کے ساتھ ساتھ جے ایس پر "جمع کروائیں/ثابت کریں" ٹولنگ کی برابری پوری طرح مطمئن ہے۔
- `iroha app da prove-availability` مندرجہ بالا سبھی زنجیروں میں: یہ اسٹوریج کا ٹکٹ لیتا ہے ، ڈاؤن لوڈ کرتا ہے
  کیننیکل مینی فیسٹ بنڈل ، ملٹی سورس آرکسٹریٹر (`iroha app sorafs fetch`) کے خلاف چلتا ہے
  فراہم کردہ `--gateway-provider` فہرست ، ڈاؤن لوڈ شدہ پے لوڈ + اسکور بورڈ کے تحت برقرار ہے
  `artifacts/da/prove_availability_<timestamp>/` ، اور فوری طور پر موجودہ پور مددگار کو طلب کرتا ہے
  (`iroha app da prove`) بازیافت بائٹس کا استعمال کرتے ہوئے۔ آپریٹرز آرکسٹریٹر نوبس کو موافقت دے سکتے ہیں
  (`--max-peers`, `--scoreboard-out`, manifest endpoint overrides) and the proof sampler
  .
  DA-5/DA-9 آڈٹ کے ذریعہ متوقع: پے لوڈ کاپی ، اسکور بورڈ ثبوت ، اور JSON پروف خلاصے۔- `da_reconstruct` (DA-6 میں نیا) ایک کیننیکل مینی فیسٹ کے علاوہ حصہ کے ذریعہ خارج ہونے والی منشیات کو پڑھتا ہے
  اسٹور (`chunk_{index:05}.bin` لے آؤٹ)
  ہر بلیک 3 عزم۔ CLI `crates/sorafs_car/src/bin/da_reconstruct.rs` اور جہازوں کے تحت رہتا ہے
  SoraFS ٹولنگ بنڈل کا حصہ۔ عام بہاؤ:
  1. `iroha app da get-blob --storage-ticket <ticket>` ڈاؤن لوڈ کرنے کے لئے `manifest_<manifest_hash>.norito` اور حصہ منصوبہ۔
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (یا `iroha app da prove-availability` ، جو بازیافت کے نوادرات کے تحت لکھتا ہے
     `artifacts/da/prove_availability_<ts>/` اور `chunks/` ڈائرکٹری کے اندر فی چنک فائلوں پر برقرار ہے)۔
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`۔

  ریگریشن فکسچر `fixtures/da/reconstruct/rs_parity_v1/` کے تحت رہتا ہے اور مکمل مظہر کو اپنی گرفت میں لے جاتا ہے
  اور chunk میٹرکس (ڈیٹا + پیریٹی) استعمال کیا جاتا ہے `tests::reconstructs_fixture_with_parity_chunks` کے ذریعہ۔ اس کے ساتھ دوبارہ تخلیق کریں

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  حقیقت کا اخراج:

  - `manifest.{norito.hex,json}` - کیننیکل `DaManifestV1` انکوڈنگز۔
  - `chunk_matrix.json` - DOC/جانچ کے حوالوں کے لئے آرڈرڈ انڈیکس/آفسیٹ/لمبائی/ڈائجسٹ/پیریٹی قطاریں۔
  - `chunks/` - `chunk_{index:05}.bin` دونوں ڈیٹا اور پیریٹی شارڈس کے لئے پے لوڈ سلائسز۔
  - `payload.bin`- برابری سے آگاہی کے ٹیسٹ کے ذریعہ استعمال ہونے والے عین مطابق پے لوڈ۔
  - `commitment_bundle.{json,norito.hex}` - نمونہ `DaCommitmentBundle` دستاویزات/ٹیسٹوں کے لئے ایک تعصب کے KZG عزم کے ساتھ۔

  کنٹرول نے گمشدہ یا کٹے ہوئے حصوں سے انکار کردیا ، `blob_hash` کے خلاف حتمی پے لوڈ بلیک 3 ہیش چیک کرتا ہے ،
  اور ایک سمری JSON بلاب (پے لوڈ بائٹس ، چنک گنتی ، اسٹوریج ٹکٹ) خارج کرتا ہے تاکہ CI تعمیر نو پر زور دے سکے
  ثبوت اس سے ڈی اے -6 کی ضرورت کو ختم ہوجاتا ہے جس میں دوبارہ تعمیر نو کے آلے کو آپریٹرز اور کیو اے کی ضرورت ہوتی ہے
  ملازمتیں بیسپوک اسکرپٹس کو وائرنگ کے بغیر طلب کرسکتی ہیں۔

## ٹوڈو ریزولوشن سمری

پہلے سے بلاک شدہ تمام ٹوڈو کو نافذ کیا گیا ہے اور اس کی تصدیق کی گئی ہے:۔
  `zstd`) اور توثیق سے پہلے پے لوڈ کو معمول بناتا ہے لہذا کیننیکل مینی فیسٹ ہیش مماثل ہے
  ڈیکمپریسڈ بائٹ
- ** گورننس صرف میٹا ڈیٹا انکرپشن **- Torii اب گورننس میٹا ڈیٹا کو انکرپٹ کرتا ہے
  تشکیل شدہ چاچا 20-پولی 1305 کلید ، مماثل لیبلوں کو مسترد کرتا ہے ، اور دو واضح طور پر سطحوں پر
  کنفیگریشن نوبس (`torii.da_ingest.governance_metadata_key_hex` ،
  `torii.da_ingest.governance_metadata_key_label`) گردش کا تعین کرنے کے لئے
- ** بڑے پے لوڈ اسٹریمنگ **- ملٹی پارٹ انجشن رواں ہے۔ کلائنٹ ڈٹارٹینسٹک کو اسٹریم کرتے ہیں
  `DaIngestChunk` لفافے `client_blob_id` کے ذریعہ کلید ، Torii ہر سلائس کی توثیق کرتا ہے ، ان کو مراحل بناتا ہے
  `manifest_store_dir` کے تحت ، اور ایک بار `is_last` پرچم اترنے کے بعد ، جوہری طور پر دوبارہ ظاہر کرتا ہے ،
  سنگل کال اپلوڈز کے ساتھ دکھائے جانے والے رام اسپائکس کو ختم کرنا۔
۔
  نامعلوم ورژن ، جب نئے مینی فیسٹ لے آؤٹ جہاز جہاز کے شپ میں بحال کرنے کی ضمانت دی جاتی ہے۔
۔
  ظاہر ہونے کے علاوہ DA-5 شیڈولرز کیننیکل ڈیٹا سے نمونے لینے کے چیلنجوں کا آغاز کرسکتے ہیں۔
  `Sora-PDP-Commitment` ہیڈر اب دونوں `/v1/da/ingest` اور `/v1/da/manifests/{ticket}` کے ساتھ جہاز بھیجتا ہے
  جوابات تو ایس ڈی کے فوری طور پر دستخط شدہ عزم کو سیکھتے ہیں کہ آئندہ کی تحقیقات کا حوالہ دیا جائے گا۔ 【کریٹس/sorafs_car/src/lib.rs: 360 】【 creats/sorafs_manifest/src/pdp.rs: 1 】【 creats/iroha_torii/src/da/ingest.rs: 476
- ** شارڈ کرسر جرنل ** - لین میٹا ڈیٹا `da_shard_id` کی وضاحت کرسکتا ہے (`lane_id` سے پہلے سے طے شدہ) ، اور
  Sumeragi اب سب سے زیادہ `(epoch, sequence)` فی `(shard_id, lane_id)` میں برقرار ہے
  `da-shard-cursors.norito` DA Spool کے ساتھ ساتھ دوبارہ شروع کرتا ہے
  ری پلے ڈٹرمینسٹک۔ ان میموری میں شارڈ کرسر انڈیکس اب کے وعدوں پر تیزی سے ناکام ہوجاتا ہے
  لین ID کو ڈیفالٹ کرنے کے بجائے بغیر نقش شدہ لینیں ، کرسر کی ترقی اور ری پلے غلطیاں بناتے ہیں
  واضح ، اور بلاک کی توثیق نے ایک سرشار کے ساتھ شارڈ-کرسر رجعتوں کو مسترد کردیا
  آپریٹرز کے لئے `DaShardCursorViolation` وجہ + ٹیلی میٹری لیبل۔ اسٹارٹ اپ/کیچ اپ اب ڈی اے کو روکتا ہے
  انڈیکس ہائیڈریشن اگر کورا میں نامعلوم لین یا ریگریسنگ کرسر پر مشتمل ہے اور اس نے مجرم قرار دیا ہے
  اونچائی کو بلاک کریں تاکہ آپریٹر ڈی اے کی خدمت سے پہلے علاج کراسکیں ریاست. 【کریٹس/اروہہ_کونفگ/ایس آر سی/پیرامیٹرز/اصل۔ سمرگی/مین_لوپ.رس 】【 کریٹس/آئروہ_کور/ایس آر سی/اسٹیٹ۔ آر ایس 】【 کریٹ/آئروہ_کور/ایس آر سی/بلاک۔
- ** شارڈ کرسر وقفہ ٹیلی میٹری ** - `da_shard_cursor_lag_blocks{lane,shard}` گیج کی اطلاع ہے کہ کیسےدور ایک شارڈ اونچائی کی توثیق کرنے کی وجہ سے ٹریل کرتا ہے۔ گمشدہ/باسی/نامعلوم لینوں نے وقفہ کو اس پر سیٹ کیا
  مطلوبہ اونچائی (یا ڈیلٹا) ، اور کامیاب پیشرفتیں اسے صفر پر دوبارہ ترتیب دیں لہذا مستحکم ریاست فلیٹ رہتی ہے۔
  آپریٹرز کو غیر صفر کی لاگ ان پر خطرے کی گھنٹی چاہئے ، مجرم لین کے لئے ڈی اے اسپل/جرنل کا معائنہ کرنا چاہئے ،
  اور صاف کرنے کے لئے بلاک کو دوبارہ چلانے سے پہلے حادثاتی طور پر دوبارہ منظم ہونے کے لئے لین کیٹلاگ کی تصدیق کریں
  گیپ
- ** خفیہ کمپیوٹ لین ** - لینوں کے ساتھ نشان زد کیا گیا
  `metadata.confidential_compute=true` اور `confidential_key_version` کے ساتھ سلوک کیا جاتا ہے
  ایس ایم پی سی/انکرپٹڈ ڈی اے راستے: Sumeragi غیر صفر پے لوڈ/منشور ہضم اور اسٹوریج ٹکٹوں کو نافذ کرتا ہے ،
  مکمل ریپلیکا اسٹوریج پروفائلز کو مسترد کرتا ہے ، اور SoraFS ٹکٹ + پالیسی ورژن کے بغیر اشاریہ کرتا ہے
  پے لوڈ بائٹس کو بے نقاب کرنا۔ ری پلے کے دوران کورا سے وصولیاں ہائیڈریٹ ہیں لہذا توثیق کرنے والے اسی کو بازیافت کرتے ہیں
  رازداری کا میٹا ڈیٹا دوبارہ شروع ہونے کے بعد۔ 【کریٹس/آئروہ_کونفگ/ایس آر سی/پیرامیٹرز/اصل۔

## نفاذ کے نوٹ- Torii کا `/v1/da/ingest` اختتامی نقطہ اب پے لوڈ کمپریشن کو معمول بناتا ہے ، ری پلے کیشے کو نافذ کرتا ہے ،
  تعی .ن کے ساتھ کیننیکل بائٹس کو گھٹا دیتا ہے ، `DaManifestV1` کی تعمیر نو کرتا ہے ، اور انکوڈڈ پے لوڈ کو گراتا ہے
  `config.da_ingest.manifest_store_dir` میں SoraFS آرکیسٹریشن کے لئے رسید جاری کرنے سے پہلے ؛
  ہینڈلر `Sora-PDP-Commitment` ہیڈر بھی جوڑتا ہے تاکہ کلائنٹ انکوڈڈ عزم کو حاصل کرسکیں
  فوری طور پر۔ 【کریٹس/اروہہ_ٹوری/ایس آر سی/ڈی اے/ingest.rs: 220】
- کیننیکل `DaCommitmentRecord` کو برقرار رکھنے کے بعد ، Torii اب خارج ہوتا ہے
  `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` فائل مینی فیسٹ اسپل کے ساتھ۔
  ہر اندراج ریکارڈ کو RAW Norito `PdpCommitment` بائٹس کے ساتھ بنڈل کرتا ہے لہذا DA-3 بنڈل بلڈرز اور
  DA-5 شیڈولرز مینیفیسٹس یا کنٹ اسٹورز کو دوبارہ پڑھنے کے بغیر ایک جیسے آدانوں کو کھولتے ہیں۔
- ایس ڈی کے مددگار ہر کلائنٹ کو Norito تجزیہ کرنے پر مجبور کیے بغیر PDP ہیڈر بائٹس کو بے نقاب کرتے ہیں:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` کور مورچا ، ازگر `ToriiClient`
  اب `decode_pdp_commitment_header` ، اور `IrohaSwift` جہازوں سے ملاپ کرنے والے مددگاروں کو برآمد کرتا ہے تاکہ موبائل
  کلائنٹ انکوڈڈ نمونے لینے کے شیڈول کو فوری طور پر اسٹش کرسکتے ہیں۔ 【کریٹس/آئروہ/ایس آر سی/ڈی اے آر ایس: 1 】【 ازگر/اروہہ_ٹوری_کلائنٹ/کلائنٹ ۔پی: 1 】【 Irohaswift/ذرائع/Irohaswift/toriiclient.swift: 1】
- Torii `GET /v1/da/manifests/{storage_ticket}` کو بھی بے نقاب کرتا ہے تاکہ SDKs اور آپریٹرز ظاہر ہوسکیں
  اور نوڈ کی اسپل ڈائرکٹری کو چھوئے بغیر اس کے منصوبے۔ جواب Norito بائٹس کو لوٹاتا ہے
  .
  ہیکس ہضم (`storage_ticket` ، `client_blob_id` ، `blob_hash` ، `chunk_root`) لہذا بہاو ٹولنگ کر سکتے ہیں
  ہضموں کی بحالی کے بغیر آرکیسٹریٹر کو کھانا کھلانا ، اور اسی `Sora-PDP-Commitment` ہیڈر کو خارج کرتا ہے
  آئینہ کے جوابات `block_hash=<hex>` کو استفسار پیرامیٹر کے طور پر پاس کرنا ایک تعصب پسند کرتا ہے
  `sampling_plan` کی جڑ `block_hash || client_blob_id` (جس میں جائزوں میں مشترکہ ہے) پر مشتمل ہے
  `assignment_hash` ، درخواست کی گئی `sample_window` ، اور نمونہ `(index, role, group)` tuples پھیلا ہوا ہے
  پوری 2D پٹی لے آؤٹ لہذا POR نمونے لینے والے اور جائز افراد ایک ہی اشاریہ کو دوبارہ چلا سکتے ہیں۔ نمونے لینے والا
  `client_blob_id` ، `chunk_root` ، اور `ipa_commitment` کو اسائنمنٹ ہیش میں ملا دیتا ہے۔ `اروہ ایپ ڈا حاصل کریں
  -بلاک ہیش  ` now writes `Sampling_plan_ 
  ہیش محفوظ ہے ، اور جے ایس/سوئفٹ Torii کلائنٹ ایک ہی `assignment_hash_hex` کو بے نقاب کرتے ہیں تاکہ توثیق کاروں
  اور پروورز ایک ہی ڈٹرمینسٹک تحقیقات کے سیٹ کا اشتراک کرتے ہیں۔ جب Torii نمونے لینے کا منصوبہ لوٹاتا ہے تو ، `اروہ ایپ ڈی اے
  اس کے بجائے ثابت-دستیابیت
  ایڈہاک کے نمونے لینے کا لہذا گواہوں کو توثیق کرنے والے اسائنمنٹس کے ساتھ لائن لگائیں یہاں تک کہ اگر آپریٹر ایک کو چھوڑ دیتا ہے
  `--block-hash` اوور رائڈ۔ 【کریٹس/آئروہ_ٹوری_شاریڈ/ایس آر سی/ڈی اے/نمونے لینے کے لئے۔ 【جاوا اسکرپٹ/آئروہ_ جے ایس/ایس آر سی/ٹورائکلینٹ. جے ایس: 15903 】【 Irohaswift/ذرائع/irohaswift/toriiclient.swift: 170】

### بڑے پے لوڈ اسٹریمنگ فلوکلائنٹ جن کو اثاثوں کو ترتیب دینے والی واحد درخواست کی حد سے بڑے اثاثوں کی ضرورت ہوتی ہے
`POST /v1/da/ingest/chunk/start` پر کال کرکے اسٹریمنگ سیشن۔ Torii a کے ساتھ جواب دیتا ہے
`ChunkSessionId` (BLAK3 سے ماخوذ بلاب میٹا ڈیٹا سے ماخوذ) اور مذاکرات شدہ حص size ہ سائز۔
ہر اس کے بعد `DaIngestChunk` درخواست کی جاتی ہے:

- `client_blob_id` - حتمی `DaIngestRequest` سے مماثل۔
- `chunk_session_id` - رننگ سیشن کے سلسلے کے سلسلے۔
- `chunk_index` اور `offset` - جینیاتی ترتیب کو نافذ کریں۔
- `payload` - مذاکرات کے حص size ے تک۔
- `payload_hash` - بلیک 3 سلائس کا ہیش لہذا Torii پورے بلاب کو بفر کیے بغیر توثیق کرسکتا ہے۔
- `is_last` - ٹرمینل سلائس کی نشاندہی کرتا ہے۔

Torii `config.da_ingest.manifest_store_dir/chunks/<session>/` کے تحت توثیق شدہ سلائسس کو برقرار رکھتا ہے اور
آئیڈیمپلیسی کے اعزاز کے لئے ری پلے کیشے کے اندر پیشرفت کو ریکارڈ کرتا ہے۔ جب آخری سلائس اترتا ہے ، Torii
ڈسک پر پے لوڈ کو دوبارہ جمع کرتا ہے (میموری کی بڑھتی ہوئی وارداتوں سے بچنے کے لئے منڈ ڈائرکٹری کے ذریعے اسٹریمنگ) ،
ایک شاٹ اپ لوڈ کے ساتھ بالکل اسی طرح کیننیکل مینی فیسٹ/رسید کی گنتی کریں ، اور آخر کار اس کا جواب دیتا ہے
`POST /v1/da/ingest` اسٹیجڈ نمونے کا استعمال کرکے۔ ناکام سیشنوں کو واضح طور پر یا اسقاط حمل کیا جاسکتا ہے
`config.da_ingest.replay_cache_ttl` کے بعد کوڑے دان کے جمع ہیں۔ یہ ڈیزائن نیٹ ورک کی شکل کو برقرار رکھتا ہے
Norito دوستانہ ، کلائنٹ سے متعلق قابل عمل پروٹوکول سے پرہیز کرتا ہے ، اور موجودہ مینی فیسٹ پائپ لائن کو دوبارہ استعمال کرتا ہے
کوئی تبدیلی نہیں

** نفاذ کی حیثیت۔ ** کیننیکل Norito اقسام اب رہتے ہیں
`crates/iroha_data_model/src/da/`:

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` کی وضاحت کرتا ہے
  `ExtraMetadata` کنٹینر استعمال کیا جاتا ہے Torii.
- `manifest.rs` میزبان `DaManifestV1` اور `ChunkCommitment` ، جو Torii اس کے بعد خارج ہوتا ہے
  چنکنگ مکمل ہوتی ہے۔
- `types.rs` مشترکہ عرفی (`BlobDigest` ، `RetentionPolicy` ، فراہم کرتا ہے ،
  `ErasureProfile` ، وغیرہ) اور ذیل میں دستاویزی دستاویزات کی پہلے سے طے شدہ پالیسی اقدار کو انکوڈ کرتا ہے۔ 【کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/ڈی اے/اقسام۔ آر ایس: 240】
- `config.da_ingest.manifest_store_dir` میں منشور اسپل فائلیں لینڈ لینڈ ، SoraFS آرکیسٹریشن کے لئے تیار ہیں
  اسٹوریج میں داخلہ لینے کے لئے نگاہ رکھنے والا۔
- Sumeragi DA بنڈلوں کو سیل کرنے یا توثیق کرتے وقت ظاہر دستیابی کو نافذ کرتا ہے:
  اگر اسپول مینی فیسٹ سے محروم ہو رہا ہے یا ہیش مختلف ہے تو بلاکس توثیق میں ناکام ہوجاتے ہیں
  عزم سے۔ 【کریٹس/آئروہ_کور/ایس آر سی/سومرگی/مین_لوپ۔

درخواست کے لئے راؤنڈ ٹریپ کوریج ، مینی فیسٹ ، اور رسید پے لوڈ کا پتہ لگایا جاتا ہے
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs` ، Norito کوڈیک کو یقینی بناتے ہوئے
تازہ کاریوں میں مستحکم رہتا ہے۔

** برقرار رکھنے کے پہلے سے طے شدہ۔ ** گورننس نے ابتدائی برقرار رکھنے کی پالیسی کی توثیق کی
SF-6 ؛ `RetentionPolicy::default()` کے ذریعہ نافذ شدہ پہلے سے طے شدہ ہیں:- گرم درجہ: 7 دن (`604_800` سیکنڈ)
- سرد درجہ: 90 دن (`7_776_000` سیکنڈ)
- مطلوبہ نقلیں: `3`
- اسٹوریج کلاس: `StorageClass::Hot`
- گورننس ٹیگ: `"da.default"`

جب کوئی لین اپنائے تو بہاو آپریٹرز کو ان اقدار کو واضح طور پر اوور رائڈ کرنا ہوگا
سخت ضروریات۔

## مورچا کلائنٹ پروف آرڈیکٹس

SDKs جو زنگ کلائنٹ کو سرایت کرتے ہیں اسے اب CLI میں گولہ باری کرنے کی ضرورت نہیں ہے
کیننیکل پور json بنڈل تیار کریں۔ `Client` نے دو مددگاروں کو بے نقاب کیا:

- `build_da_proof_artifact` عین مطابق ڈھانچے کو واپس کرتا ہے
  `iroha app da prove --json-out` ، بشمول منشور/پے لوڈ کی تشریحات فراہم کی گئیں
  [`DaProofArtifactMetadata`] کے ذریعے۔ 【کریٹس/اروہ/ایس آر سی/کلائنٹ۔ آر ایس: 3638】
- `write_da_proof_artifact` بلڈر کو لپیٹتا ہے اور ڈسک پر آرٹ فیکٹ برقرار رکھتا ہے
  ۔
  ریلیز یا گورننس شواہد کے بنڈل۔

### مثال

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

JSON پے لوڈ جو مددگار چھوڑ دیتا ہے CLI کو فیلڈ کے ناموں سے نیچے کرتا ہے
(`manifest_path` ، `payload_path` ، `proofs[*].chunk_digest` ، وغیرہ) ، لہذا موجودہ
آٹومیشن فارمیٹ مخصوص شاخوں کے بغیر فائل کو مختلف/پارکیٹ/اپ لوڈ کرسکتا ہے۔

## پروف تصدیق کا بینچ مارک

اس سے پہلے نمائندہ پے لوڈ پر تصدیق کنندہ بجٹ کی توثیق کرنے کے لئے ڈی اے پروف بینچ مارک استعمال کا استعمال کریں
بلاک سطح کی ٹوپیاں سخت کرنا:

- `cargo xtask da-proof-bench` منشور/پے لوڈ جوڑی ، نمونے پور سے حصہ اسٹور کو دوبارہ تعمیر کرتا ہے
  تشکیل شدہ بجٹ کے خلاف پتے ، اور اوقات کی توثیق۔ تائکائی میٹا ڈیٹا خود سے بھرا ہوا ہے ، اور
  اگر حقیقت میں جوڑی متضاد ہے تو کنٹرول مصنوعی مینی فیسٹ پر واپس آجاتا ہے۔ جب `--payload-bytes`
  واضح `--payload` کے بغیر سیٹ کیا گیا ہے ، پیدا شدہ بلاب لکھا گیا ہے
  `artifacts/da/proof_bench/payload.bin` لہذا فکسچر اچھوت رہیں۔
- `artifacts/da/proof_bench/benchmark.{json,md}` میں ڈیفالٹ کی اطلاعات اور اس میں ثبوت/رن ، کل اور شامل ہیں
  فی پروف وقت ، بجٹ پاس کی شرح ، اور ایک تجویز کردہ بجٹ (سب سے سست ترین تکرار کا 110 ٪)
  `zk.halo2.verifier_budget_ms` کے ساتھ لائن اپ کریں۔
- تازہ ترین رن (مصنوعی 1 ایم آئی بی پے لوڈ ، 64 KIB ٹک ، 32 ثبوت/رن ، 10 تکرار ، 250 ایم ایس بجٹ)
  ٹوپی کے اندر 100 ٪ تکرار کے ساتھ 3 ایم ایس تصدیق کنندہ بجٹ کی سفارش کی۔
- مثال (ایک تعی .ن پے لوڈ تیار کرتا ہے اور دونوں رپورٹس لکھتا ہے):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

بلاک اسمبلی ایک ہی بجٹ کو نافذ کرتی ہے: `sumeragi.da_max_commitments_per_block` اور
`sumeragi.da_max_proof_openings_per_block` گیٹ ڈی اے بنڈل اس سے پہلے کہ یہ بلاک میں سرایت کرے ، اور
ہر عزم کو غیر صفر `proof_digest` لے جانا چاہئے۔ گارڈ بنڈل کی لمبائی کو بطور سلوک کرتا ہے
ثبوت کھولنے کی گنتی جب تک کہ واضح ثبوت کے خلاصے کو اتفاق رائے کے ذریعے تھریڈ کیا جاتا ہے ، اسے برقرار رکھتے ہوئے
block 128 اوپننگ ہدف بلاک کی حد میں قابل عمل ہے۔

## پور ناکامی کو سنبھالنا اور سلیش کرنااسٹوریج ورکرز اب ہر ایک کے ساتھ ساتھ POR کی ناکامی کی لکیروں اور بانڈڈ سلیش سفارشات کی سطح پر ہیں
ورڈکٹ۔ تشکیل شدہ ہڑتال کی دہلیز کے اوپر لگاتار ناکامیوں سے ایک سفارش خارج ہوتی ہے کہ
فراہم کنندہ/مینی فیسٹ جوڑی ، سلیش کی لمبائی جس نے سلیش کو متحرک کیا ، اور مجوزہ پر مشتمل ہے
فراہم کنندہ بانڈ اور `penalty_bond_bps` سے حساب کتاب ؛ کولڈاؤن ونڈوز (سیکنڈ) رکھیں
اسی واقعے پر فائرنگ سے ڈپلیکیٹ سلیش۔

- اسٹوریج ورکر بلڈر کے ذریعہ دہلیز/کوولڈاؤن کی تشکیل کریں (ڈیفالٹس گورننس کا آئینہ دار ہے
  جرمانہ پالیسی)۔
- سلیش سفارشات کو ورڈکٹ سمری JSON میں ریکارڈ کیا جاتا ہے تاکہ گورننس/آڈیٹر منسلک ہوسکیں
  انہیں ثبوت کے بنڈل پر۔
- پٹی لے آؤٹ + فی چنک کرداروں کو اب Torii کے اسٹوریج پن اینڈ پوائنٹ کے ذریعے تھریڈ کیا گیا ہے
  (`stripe_layout` + `chunk_roles` فیلڈز) اور اسٹوریج ورکر میں برقرار رہا
  آڈیٹرز/مرمت کے ٹولنگ اپ اسٹریم سے دوبارہ ترتیب کے بغیر قطار/کالم کی مرمت کا منصوبہ بناسکتے ہیں

### پلیسمنٹ + مرمت کا استعمال

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` اب
`(index, role, stripe/column, offsets)` پر پلیسمنٹ ہیش کی گنتی کرتا ہے اور پھر قطار پہلے انجام دیتا ہے
پے لوڈ کی تشکیل نو سے پہلے کالم آر ایس (16) مرمت:

- `total_stripes`/`shards_per_stripe` پر پلیسمنٹ ڈیفالٹس جب موجود ہوتا ہے اور واپس گر جاتا ہے
- لاپتہ/خراب حصوں کو پہلے قطار برابری کے ساتھ دوبارہ تعمیر کیا گیا ہے۔ باقی خلیجوں کی مرمت کی جاتی ہے
  پٹی (کالم) برابری۔ مرمت شدہ حصے واپس منڈ ڈائرکٹری ، اور JSON پر لکھے گئے ہیں
  خلاصہ پلیسمنٹ ہیش پلس قطار/کالم کی مرمت کے کاؤنٹرز کو اپنی گرفت میں لے جاتا ہے۔
اگر قطار+کالم کی برابری گمشدہ سیٹ کو پورا نہیں کرسکتی ہے تو ، استعمال ناقابل تلافی کے ساتھ تیزی سے ناکام ہوجاتا ہے
  اشارے تو آڈیٹر ناقابل تلافی ظاہر ہوسکتے ہیں۔