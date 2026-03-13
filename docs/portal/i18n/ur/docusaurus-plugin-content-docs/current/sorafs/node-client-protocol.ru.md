---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پروٹوکول SoraFS نوڈ ↔ کلائنٹ

یہ گائیڈ میں پروٹوکول کی کیننیکل تعریف کا خلاصہ پیش کیا گیا ہے
[`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)۔
بائٹ Norito لے آؤٹ اور چینجلوگس کے لئے upstream تصریح کا استعمال کریں۔
پورٹل کاپی آپریشنل زور کو باقی SoraFS رن بک کے قریب رکھتی ہے۔

## فراہم کنندہ اعلامیہ اور توثیق

SoraFS فراہم کنندگان `ProviderAdvertV1` پے لوڈ تقسیم کرتے ہیں (دیکھیں۔
`crates/sorafs_manifest::provider_advert`) منظم آپریٹر کے ذریعہ دستخط شدہ۔
اشتہارات دریافت میٹا ڈیٹا اور محافظوں پر قبضہ کرتے ہیں
ملٹی سورس آرکسٹریٹر رن ٹائم پر لاگو ہوتا ہے۔

- ** درست مدت ** - `issued_at < expires_at ≤ issued_at + 86 400 s`۔ فراہم کرنے والے
  ہر 12 گھنٹے میں اپ ڈیٹ ہونا ضروری ہے۔
- ** صلاحیت TLV ** - TLV فہرست میں نقل و حمل کی صلاحیتوں کا اشتہار دیا گیا (Torii ،
  کوئیک+شور ، سورانیٹ ریلے ، وینڈر ایکسٹینشنز)۔ نامعلوم کوڈ ہوسکتے ہیں
  سفارشات کے بعد ، `allow_unknown_capabilities = true` پر جائیں
  چکنائی
- ** QOS اشارے ** - ٹائر `availability` (گرم/گرم/سردی) ، زیادہ سے زیادہ تاخیر
  نکالنے ، مسابقت کی حد اور اختیاری اسٹریم بجٹ۔ QOS لازمی ہے
  میچ کا مشاہدہ ٹیلی میٹری اور داخلے کے بعد اس کی تصدیق ہوتی ہے۔
- ** اختتامی نکات اور رینڈیزوس عنوانات ** - TLS/ALPN کے ساتھ مخصوص سروس یو آر ایل
  میٹا ڈیٹا پلس ڈسکوری عنوانات جو کلائنٹ کب سبسکرائب کرتے ہیں
  بلڈنگ گارڈ سیٹ۔
- ** راہ تنوع کی پالیسی ** - `min_guard_weight` ، کے لئے فین آؤٹ حدود
  AS/تالاب اور `provider_failure_threshold` عصبی طور پر فراہم کرتے ہیں
  ملٹی پیئر بازیافت۔
- ** پروفائل آئی ڈی ** - فراہم کنندگان کو کیننیکل شائع کرنے کی ضرورت ہے
  ہینڈل (مثال کے طور پر ، `sorafs.sf1@1.0.0`) ؛ اختیاری `profile_aliases` مدد
  پرانے گاہکوں کی ہجرت۔

توثیق کے قواعد صفر داؤ ، صلاحیتوں/اختتامی نکات/عنوانات کی خالی فہرستوں کو مسترد کرتے ہیں ،
ڈیڈ لائن یا QoS کے اہداف کو کھوئے ہوئے۔ داخلہ لفافے
اس سے پہلے اعلامیہ اور شق باڈیز (`compare_core_fields`) کا موازنہ کریں
تازہ کاریوں کی تقسیم۔

### رینج بازیافت ایکسٹینشن

رینج کے مطابق فراہم کرنے والوں میں مندرجہ ذیل میٹا ڈیٹا شامل ہیں:

| فیلڈ | منزل |
| ------ | ----------- |
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span` ، `min_granularity` ، اور سیدھ/ثبوت کے جھنڈوں کا اعلان کرتا ہے۔ |
| `StreamBudgetV1` | اختیاری ہم آہنگی/تھروپپٹ لفافہ (`max_in_flight` ، `max_bytes_per_sec` ، اختیاری `burst`)۔ حد کی صلاحیت کی ضرورت ہے۔ |
| `TransportHintV1` | آرڈرڈ ٹرانسپورٹ کی ترجیحات (مثال کے طور پر ، `torii_http_range` ، `quic_stream` ، `soranet_relay`)۔ ترجیحات `0–15` ، نقلیں مسترد کردی گئیں۔ |

ٹولنگ سپورٹ:

- فراہم کنندہ کے اشتہار پائپ لائنوں کو حد کی صلاحیت ، اسٹریم بجٹ کی توثیق کرنی ہوگی
  اور آڈٹ کے ل det ڈٹرمینسٹک پے لوڈ کو جاری کرنے سے پہلے نقل و حمل کے اشارے۔
- `cargo xtask sorafs-admission-fixtures` پیک کیننیکل ملٹی سورس
  میں کمی کے ساتھ ساتھ ڈاؤن گریڈ فکسچر کے ساتھ اشتہارات
  `fixtures/sorafs_manifest/provider_admission/`۔
- `stream_budget` یا `transport_hints` کے بغیر رینج اشتہارات ڈاؤن لوڈ کرنے والوں کے ذریعہ مسترد کردیئے گئے ہیں
  شیڈولنگ سے پہلے CLI/SDK ، ملٹی سورس کنٹرول کو مطابق رکھنا
  داخلہ کی توقعات Torii۔

## رینج اینڈپوائنٹس گیٹ وےگیٹ وے AD میٹا ڈیٹا کی عکاسی کرنے والی تشخیصی HTTP درخواستوں کو قبول کرتے ہیں۔

### `GET /v2/sorafs/storage/car/{manifest_id}`

| ضرورت | تفصیلات |
| ------------ | -------- |
| ** ہیڈر ** | `Range` (ایک وقفہ جس میں منشیات کی آفسیٹس کے ذریعہ منسلک کیا گیا ہے) ، `dag-scope: block` ، `X-SoraFS-Chunker` ، اختیاری `X-SoraFS-Nonce` اور لازمی BASE64 `X-SoraFS-Stream-Token`۔ |
| ** جوابات ** | `206` کے ساتھ `Content-Type: application/vnd.ipld.car` ، `Content-Range` جاری کردہ وقفہ کی وضاحت ، `X-Sora-Chunk-Range` میٹا ڈیٹا اور ایکو ہیڈر چنکر/ٹوکن۔ |
| ** ناکامی کے طریقوں ** | `416` غلط طور پر منسلک حدود کے لئے ، `401` گمشدہ/غلط ٹوکن کے لئے ، `429` اسٹریم/بائٹ بجٹ سے زیادہ کے لئے۔ |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

ایک ہی ہیڈر کے ساتھ ایک حصہ لائیں اور ایک ڈٹرمینسٹک ڈائجسٹ حص .ہ۔
جب کار کے ٹکڑوں کی ضرورت نہیں ہے تو دوبارہ لینے یا فرانزک ڈاؤن لوڈ کے لئے مفید ہے۔

## ملٹی سورس آرکسٹریٹر ورک فلو

جب SF-6 ملٹی سورس بازیافت فعال ہوجائے (مورچا CLI `sorafs_fetch` کے ذریعے ،
`sorafs_orchestrator` کے ذریعے SDKS):

1. ** ان پٹ ڈیٹا اکٹھا کریں ** - منشور منقطع منصوبہ کو ڈی کوڈ کریں ، حاصل کریں
   تازہ ترین اشتہارات اور اختیاری طور پر ٹیلی میٹری اسنیپ شاٹ بھیجیں
   (`--telemetry-json` یا `TelemetrySnapshot`)۔
2. ** اسکور بورڈ بنائیں ** - `Orchestrator::build_scoreboard` شرحیں
   مناسبیت اور انکار کی وجوہات ریکارڈ ؛ `sorafs_fetch --scoreboard-out`
   JSON کو بچاتا ہے۔
3
   حدود کی پابندیاں ، اسٹریم بجٹ ، دوبارہ کوشش/ہم مرتبہ کی حدود (`--retry-budget` ،
   `--max-peers`) اور ہر درخواست کے لئے دائرہ کار میں ایک اسٹریم ٹوکن جاری کرتا ہے۔
4. ** رسیدیں چیک کریں ** - نتائج میں `chunk_receipts` اور شامل ہیں
   `provider_reports` ؛ سی ایل آئی کا خلاصہ بچائیں `provider_reports` ، `chunk_receipts`
   اور شواہد کے بنڈل کے لئے `ineligible_providers`۔

عام غلطیاں آپریٹرز/ایس ڈی کے کے ذریعہ واپس کی گئیں:

| غلطی | تفصیل |
| -------- | ------------ |
| `no providers were supplied` | فلٹرنگ کے بعد کوئی مماثل ریکارڈ موجود نہیں ہے۔ |
| `no compatible providers available for chunk {index}` | کسی خاص حصے کے لئے رینج یا بجٹ کی عدم مطابقت۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` میں اضافہ کریں یا ناکام ساتھیوں کو ختم کریں۔ |
| `no healthy providers remaining` | بار بار ناکامیوں کے بعد تمام فراہم کنندگان کو غیر فعال کردیا گیا ہے۔ |
| `streaming observer failed` | بہاو ​​کار مصنف ناکام ہوگیا۔ |
| `orchestrator invariant violated` | ٹریج کے لئے منشور ، اسکور بورڈ ، ٹیلی میٹری اسنیپ شاٹ اور کلی جیسن جمع کریں۔ |

## ٹیلی میٹری اور ثبوت

- آرکسٹریٹر کے ذریعہ جاری کردہ میٹرکس:  
  `sorafs_orchestrator_active_fetches` ، `sorafs_orchestrator_fetch_duration_ms` ،
  `sorafs_orchestrator_retries_total` ، `sorafs_orchestrator_provider_failures_total`
  (ٹیگز کے ساتھ مینی فیسٹ/علاقہ/فراہم کنندہ)۔ تشکیل یا میں `telemetry_region` کی وضاحت کریں
  سی ایل آئی کے جھنڈوں کے ذریعے تاکہ ڈیش بورڈز کو بیڑے کے ذریعہ تقسیم کیا جائے۔
- CLI/SDK بازیافت کے خلاصے میں محفوظ اسکور بورڈ JSON ، چنک رسیدیں اور شامل ہیں
  فراہم کنندہ کی رپورٹیں ، جن کو SF-6/SF-7 گیٹس کے لئے رول آؤٹ بنڈل میں شامل کیا جانا چاہئے۔
- گیٹ وے ہینڈلرز `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` شائع کریں ،
  تاکہ SRE ڈیش بورڈز سرور سلوک کے ساتھ آرکیسٹریٹر کے فیصلوں سے وابستہ ہوں۔

## سی ایل آئی اور ریسٹ مددگار- `iroha app sorafs pin list|show` ، `alias list` اور `replication list` لپیٹنا
  پن رجسٹری ریسٹ اینڈ پوائنٹس اور پرنٹ را Norito JSON تصدیق کے بلاکس کے ساتھ
  آڈٹ شواہد کے لئے۔
- `iroha app sorafs storage pin` اور `torii /v2/sorafs/pin/register` Norito کو قبول کریں
  یا JSON کے علاوہ اختیاری عرفی ثبوت اور جانشینوں کے علاوہ ظاہر ہوتا ہے۔ خراب ثبوت
  واپسی `400` ، باسی ثبوت `Warning: 110` کے ساتھ `503` ، اور سخت مجاز ثبوت
  واپس `412`۔
- آرام کے اختتامی مقامات (`/v2/sorafs/pin` ، `/v2/sorafs/aliases` ،
  `/v2/sorafs/replication`) تصدیق کے ڈھانچے شامل کریں تاکہ کلائنٹ کرسکیں
  کارروائی کرنے سے پہلے آخری بلاک ہیڈر سے متعلق ڈیٹا کو چیک کریں۔

## لنکس

- کیننیکل تفصیلات:
  [`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito اقسام: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- سی ایل آئی مددگار: `crates/iroha_cli/src/commands/sorafs.rs` ،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- آرکیسٹریٹر کریٹ: `crates/sorafs_orchestrator`
- ڈیش بورڈ پیک: `dashboards/grafana/sorafs_fetch_observability.json`