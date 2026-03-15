---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نوڈ ↔ کلائنٹ پروٹوکول SoraFS

اس گائیڈ میں پروٹوکول کی کیننیکل تعریف کا خلاصہ کیا گیا ہے
[`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)۔
بائٹ سطح پر لے آؤٹ Norito کے لئے upstream تفصیلات کا استعمال کریں اور
چینجلاگس ؛ پورٹل کی کاپی آپریشنل پوائنٹس کے قریب رکھتی ہے
باقی رن بکس SoraFS۔

## سپلائر اشتہارات اور توثیق

فراہم کنندگان SoraFS پے لوڈ کو تقسیم کرتے ہیں `ProviderAdvertV1` (دیکھیں
`crates/sorafs_manifest::provider_advert`) گورنمنٹ آپریٹر کے ذریعہ دستخط شدہ۔
اشتہارات نے دریافت میٹا ڈیٹا اور سرپرستوں کو ترتیب دیا جو آرکسٹریٹر ہے
ملٹی سورس رن ٹائم پر لاگو ہوتا ہے۔

- ** درست مدت ** - `issued_at < expires_at ≤ issued_at + 86 400 s`۔
  فراہم کنندگان کو ہر 12 گھنٹے میں تازہ دم کرنا چاہئے۔
- ** صلاحیتوں کی TLV ** - TLV فہرست نے ٹرانسپورٹ کی خصوصیات کا اعلان کیا
  (Torii ، کوئیک+شور ، سورانیٹ ریلے ، فروش ایکسٹینشن)۔ نامعلوم کوڈز
  جب `allow_unknown_capabilities = true` ، مندرجہ ذیل ہوں تو نظرانداز کیا جاسکتا ہے
  چکنائی کی سفارشات۔
- ** QOS انڈیکس ** - `availability` (گرم/گرم/سردی) کا درجہ ، زیادہ سے زیادہ تاخیر
  بازیابی ، ہم آہنگی کی حد اور اختیاری اسٹریم بجٹ۔ QOS لازمی ہے
  مشاہدہ ٹیلی میٹری کے ساتھ سیدھ کریں اور انٹیک پر آڈٹ کیا جاتا ہے۔
- ** اختتامی نکات اور ملاقات کے عنوانات ** - کنکریٹ سروس یو آر ایل کے ساتھ
  TLS/ALPN میٹا ڈیٹا ، نیز دریافت کے عنوانات کس کلائنٹ ہیں
  گارڈ سیٹ تیار کرتے وقت سبسکرائب کرنا ضروری ہے۔
- ** راہ تنوع کی پالیسی **- `min_guard_weight` ، فین آؤٹ کیپس
  AS/پول اور `provider_failure_threshold` بازیافتوں کو ممکن بناتا ہے
  کثیر الجہتی تعصب
- ** پروفائل شناخت کار ** - فراہم کنندگان کو ہینڈل کو بے نقاب کرنا ہوگا
  کیننیکل (جیسے `sorafs.sf1@1.0.0`) ؛ اختیاری `profile_aliases` مدد
  ہجرت کرنے کے لئے پرانے کلائنٹ۔

توثیق کے قواعد صفر داؤ ، خالی صلاحیتوں کی فہرستوں کو مسترد کرتے ہیں ،
اختتامی مقامات یا عنوانات ، ناقص حکم دیا گیا دورانیے ، یا QOS مقاصد سے محروم۔
داخلہ لفافے اشتہار اور تجویز کرنے والے اداروں کا موازنہ کرتے ہیں
(`compare_core_fields`) تازہ کاریوں کو جاری کرنے سے پہلے۔

### حدود کے ذریعہ توسیع کی توسیع

رینج سے چلنے والے فراہم کنندگان میں مندرجہ ذیل میٹا ڈیٹا شامل ہیں:

| فیلڈ | مقصد |
| ------- | --------- |
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span` ، `min_granularity` اور سیدھ/پروف جھنڈوں کا اعلان کرتا ہے۔ |
| `StreamBudgetV1` | اختیاری ہم آہنگی/تھرو پٹ لفافہ (`max_in_flight` ، `max_bytes_per_sec` ، `burst` اختیاری)۔ حد کی صلاحیت کی ضرورت ہے۔ |
| `TransportHintV1` | آرڈرڈ ٹرانسپورٹ کی ترجیحات (جیسے `torii_http_range` ، `quic_stream` ، `soranet_relay`)۔ ترجیحات `0–15` اور نقل سے ضائع کردی گئیں۔ |

ٹولنگ سپورٹ:- سپلائر ایڈورٹ پائپ لائنوں کو رینج ، اسٹریم کی گنجائش کی توثیق کرنی ہوگی
  کے لئے عین مطابق پے لوڈ جاری کرنے سے پہلے بجٹ اور ٹرانسپورٹ کے اشارے
  آڈٹ
- `cargo xtask sorafs-admission-fixtures` ملٹی سورس اشتہارات کو ایک ساتھ لاتا ہے
  ڈاون گریڈ فکسچر کے ساتھ کینونیکلز
  `fixtures/sorafs_manifest/provider_admission/`۔
- رینج اشتہارات جو `stream_budget` یا `transport_hints` کو چھوڑ دیتے ہیں
  منصوبہ بندی سے پہلے سی ایل آئی/ایس ڈی کے لوڈرز نے مسترد کرتے ہوئے ، استعمال کو سیدھا کیا
  داخلے کی توقعات پر ملٹی سورس Torii۔

## گیٹ وے کی اختتامی حدود

گیٹ وے ڈٹرمینسٹک HTTP درخواستوں کو قبول کرتے ہیں جو عکاسی کرتے ہیں
ایڈورٹ میٹا ڈیٹا۔

### `GET /v1/sorafs/storage/car/{manifest_id}`

| ضرورت | تفصیلات |
| --------- | --------- |
| ** ہیڈر ** | `Range` (سنگل ونڈو منڈ آفسیٹس کے ساتھ منسلک) ، `dag-scope: block` ، `X-SoraFS-Chunker` ، `X-SoraFS-Nonce` اختیاری ، اور `X-SoraFS-Stream-Token` base64 لازمی۔ |
| ** جوابات ** | `206` کے ساتھ `Content-Type: application/vnd.ipld.car` ، `Content-Range` پیش کردہ ونڈو کی وضاحت ، میٹا ڈیٹا `X-Sora-Chunk-Range` ، اور چنکر/ٹوکن ہیڈر واپس آئے۔ |
| ** ناکامی کے طریقوں ** | `416` غلط استعمال کی حدود کے لئے ، `401` گمشدہ/غلط ٹوکن کے لئے ، `429` جب اسٹریم/بائٹ بجٹ سے تجاوز کیا جاتا ہے۔ |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

ایک ہی ہیڈر کے ساتھ ایک ہی حصہ لائیں ، نیز اس کے اختیاری ڈائجسٹ
ٹکڑے فرانزک کوششوں یا ڈاؤن لوڈ کے ل useful مفید ہے
کار کے ٹکڑے بیکار ہیں۔

## ملٹی سورس آرکسٹریٹر ورک فلو

جب SF-6 ملٹی سورس بازیافت فعال ہوجائے (مورچا CLI کے ذریعے `sorafs_fetch` ،
`sorafs_orchestrator` کے ذریعے SDKS):

1. ** ان پٹ اکٹھا کریں ** - ڈیکوڈ منشور منشیات کا منصوبہ ، بازیافت کریں
   تازہ ترین اشتہارات ، اور اختیاری طور پر ٹیلی میٹری اسنیپ شاٹ لیتے ہیں
   (`--telemetry-json` یا `TelemetrySnapshot`)۔
2. ** ایک اسکور بورڈ بنائیں ** - `Orchestrator::build_scoreboard` تشخیص کرتا ہے
   اہلیت اور ریکارڈ کو مسترد کرنے کی وجوہات ؛ `sorafs_fetch --scoreboard-out`
   JSON برقرار رہتا ہے۔
3.
   رینج کی رکاوٹیں ، اسٹریم بجٹ ، دوبارہ کوشش/ہم مرتبہ کیپس (`--retry-budget` ،
   `--max-peers`) اور ہر ایک کے لئے مینی فیسٹ پر ایک اسٹریم ٹوکن گنجائش کا اخراج کرتا ہے
   استفسار
4
   `provider_reports` ؛ سی ایل آئی کے خلاصے `provider_reports` کو برقرار رکھتے ہیں ،
   `chunk_receipts` اور `ineligible_providers` ثبوت کے بنڈل کے لئے۔

آپریٹرز/ایس ڈی کے کو عام غلطیاں اطلاع دی گئیں:

| غلطی | تفصیل |
| -------- | -------- |
| `no providers were supplied` | فلٹرنگ کے بعد کوئی اہل اندراجات نہیں ہیں۔ |
| `no compatible providers available for chunk {index}` | کسی خاص حصے کے لئے رینج یا بجٹ کی خرابی۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` میں اضافہ کریں یا ناکام ساتھیوں کو بے دخل کریں۔ |
| `no healthy providers remaining` | بار بار ناکامیوں کے بعد تمام فراہم کنندگان غیر فعال ہوجاتے ہیں۔ |
| `streaming observer failed` | بہاو ​​کار مصنف نے اسقاط حمل کیا۔ |
| `orchestrator invariant violated` | ٹریج کے لئے مینی فیسٹ ، اسکور بورڈ ، ٹیلی میٹری اسنیپ شاٹ اور JSON CLI پر قبضہ کریں۔ |

## ٹیلی میٹری اور ثبوت- آرکسٹریٹر کے ذریعہ جاری کردہ میٹرکس:  
  `sorafs_orchestrator_active_fetches` ، `sorafs_orchestrator_fetch_duration_ms` ،
  `sorafs_orchestrator_retries_total` ، `sorafs_orchestrator_provider_failures_total`
  (منشور/خطے/فروش کے ذریعہ ٹیگ کردہ)۔ `telemetry_region` کو سیٹ کریں
  بیڑے کے ذریعہ ڈیش بورڈز کو پارٹیشن ڈیش بورڈز میں سی ایل آئی کے جھنڈوں کے ذریعے تشکیل دیں۔
- CLI/SDK بازیافت کے خلاصے میں JSON اسکور بورڈ ، رسیدیں شامل ہیں
  ٹکڑوں اور سپلائر رپورٹس کی جو بنڈل میں سفر کرنا ضروری ہے
  SF-6/SF-7 گیٹس کے لئے رول آؤٹ۔
- گیٹ وے ہینڈلرز `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` کو بے نقاب کرتے ہیں
  تاکہ SRE ڈیش بورڈز آرکیسٹریٹر کے فیصلوں سے وابستہ ہوں
  سرور سلوک

## سی ایل آئی اور ریسٹ مددگار

- `iroha app sorafs pin list|show` ، `alias list` اور `replication list` پیک کریں
  پن رجسٹری ریسٹ اینڈ پوائنٹس اور پرنٹ را json Norito بلاکس کے ساتھ
  آڈٹ کے لئے سرٹیفیکیشن۔
- `iroha app sorafs storage pin` اور `torii /v1/sorafs/pin/register` قبول کریں
  Norito یا JSON ظاہر ہوتا ہے ، نیز اختیاری عرف ثبوت اور جانشین ؛
  خراب شدہ ثبوت `400` کی واپسی کرتے ہیں ، متروک ثبوت `503` کے ساتھ بے نقاب کرتے ہیں
  `Warning: 110` ، اور میعاد ختم ہونے والے ثبوت `412` واپس کریں۔
- آرام کے اختتامی مقامات (`/v1/sorafs/pin` ، `/v1/sorafs/aliases` ،
  `/v1/sorafs/replication`) تصدیق کے ڈھانچے شامل کریں تاکہ
  اداکاری سے پہلے کلائنٹ تازہ ترین بلاک ہیڈر کے ساتھ ڈیٹا چیک کرتے ہیں۔

## حوالہ جات

- کیننیکل تفصیلات:
  [`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- قسم Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- سی ایل آئی مددگار: `crates/iroha_cli/src/commands/sorafs.rs` ،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- آرکیسٹریٹر کریٹ: `crates/sorafs_orchestrator`
- ڈیش بورڈ پیک: `dashboards/grafana/sorafs_fetch_observability.json`