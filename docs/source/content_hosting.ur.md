---
lang: ur
direction: rtl
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

٪ مواد کی میزبانی لین
٪ Iroha کور

# مواد کی میزبانی لین

مواد لین چھوٹے چھوٹے جامد بنڈل (ٹار آرکائیوز) آن چین کو اسٹور کرتی ہے اور خدمت کرتی ہے
Torii سے براہ راست انفرادی فائلیں۔

- ** شائع کریں **: `PublishContentBundle` کو ٹار آرکائیو کے ساتھ جمع کروائیں ، اختیاری میعاد ختم
  اونچائی ، اور ایک اختیاری منشور۔ بنڈل ID بلیک 2 بی ہیش ہے
  ٹربال۔ ٹار اندراجات باقاعدگی سے فائلیں ہونی چاہئیں۔ نام عام طور پر UTF-8 راستے ہیں۔
  سائز/پاتھ/فائل گنتی کیپس `content` کنفگ (`max_bundle_bytes` ، سے آتی ہیں ،
  `max_files` ، `max_path_len` ، `max_retention_blocks` ، `chunk_size_bytes`)۔
  منشور میں Norito-انڈیکس ہیش ، ڈیٹا اسپیس/لین ، کیشے کی پالیسی شامل ہے
  .
  `sponsor:<uaid>`) ، برقرار رکھنے کی پالیسی پلیس ہولڈر ، اور MIME کو اوور رائڈز۔
۔
  حوالہ گنتی کے ساتھ ہیش ؛ بنڈل کی کمی اور کٹے ہوئے حصوں کو ریٹائر کرنا۔
- ** خدمت **: Torii `GET /v1/content/{bundle}/{path}` کو بے نقاب کرتا ہے۔ جوابات ندی
  براہ راست `ETag` = فائل ہیش ، `Accept-Ranges: bytes` کے ساتھ حصہ اسٹور سے ،
  رینج سپورٹ ، اور کیشے پر قابو پانے سے منشور سے اخذ کیا گیا ہے۔ اعزاز کے بارے میں پڑھتا ہے
  مینی فیسٹ آتھ موڈ: رول گیٹڈ اور کفیل گیٹڈ ردعمل کی ضرورت ہوتی ہے
  دستخط شدہ کے لئے ہیڈر (`X-Iroha-Account` ، `X-Iroha-Signature`) کی درخواست کریں
  اکاؤنٹ ؛ گمشدہ/میعاد ختم ہونے والے بنڈل 404 لوٹتے ہیں۔
- ** CLI **: `iroha content publish --bundle <path.tar>` (یا `--root <dir>`) اب
  آٹو آٹو کو ایک منشور تیار کرتا ہے ، اختیاری `--manifest-out/--bundle-out` کا اخراج کرتا ہے ، اور
  `--auth` ، `--cache-max-age-secs` ، `--dataspace` ، `--lane` ، `--immutable` کو قبول کرتا ہے۔
  اور `--expires-at-height` اوور رائڈز۔ `iroha content pack --root <dir>` تعمیر کرتا ہے
  کچھ بھی جمع کرائے بغیر ایک عین مطابق ٹربال + ظاہر ہوتا ہے۔
- **Config**: cache/auth knobs live under `content.*` in `iroha_config`
  (`default_cache_max_age_secs` ، `max_cache_max_age_secs` ، `immutable_bundles` ،
  `default_auth_mode`) اور اشاعت کے وقت نافذ کیا جاتا ہے۔
- ** سلو + حدود **: `content.max_requests_per_second` / `request_burst` اور
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` CAP پڑھنے کی طرف
  تھرو پٹ ؛ Torii بائٹس اور برآمدات کی خدمت سے پہلے دونوں کو نافذ کرتا ہے
  `torii_content_requests_total` ، `torii_content_request_duration_seconds` ، اور
  `torii_content_response_bytes_total` میٹرکس نتائج کے لیبل کے ساتھ۔ تاخیر
  اہداف `content.target_p50_latency_ms` / کے تحت رہتے ہیں
  `content.target_p99_latency_ms` / `content.target_availability_bps`۔
۔
  اختیاری پاو گارڈ (`content.pow_difficulty_bits` ، `content.pow_header`) CAN
  پڑھنے سے پہلے ضروری ہو۔ دا پٹی لے آؤٹ ڈیفالٹس سے آتے ہیں
  `content.stripe_layout` اور رسیدوں/مینی فیسٹ ہیشوں میں بازگشت ہے۔
- ** رسیدیں اور ڈی اے ثبوت **: کامیاب جوابات منسلک ہوتے ہیں
  Torii (BASE64 Norito- فریم `ContentDaReceipt` بائٹس) لے جانا
  `bundle_id` ، `path` ، `file_hash` ، `served_bytes` ، پیش کردہ بائٹ رینج ،
  `chunk_root` / `stripe_layout` ، اختیاری PDP عزم ، اور ایک ٹائم اسٹیمپ تو
  کلائنٹ جسم کو دوبارہ پڑھے بغیر جو کچھ حاصل کیا گیا تھا اسے پن کر سکتے ہیں۔

کلیدی حوالہ جات:- ڈیٹا ماڈل: `crates/iroha_data_model/src/content.rs`
- پھانسی: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii ہینڈلر: `crates/iroha_torii/src/content.rs`
- سی ایل آئی ہیلپر: `crates/iroha_cli/src/content.rs`