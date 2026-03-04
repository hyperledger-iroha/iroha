---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS نمبر  کلائنٹ پروٹوکول

اس گائیڈ میں پروٹوکول کی کیننیکل تعریف کا خلاصہ کیا گیا ہے
[`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)۔
بائٹ لیول Norito لے آؤٹ اور چینجلوگس کے لئے upstream تصریح کا استعمال کریں۔
پورٹل کاپی آپریشنل جھلکیاں باقی رن بکس کے قریب رکھتی ہے
SoraFS۔

## فراہم کنندہ اور توثیق کی انتباہات

SoraFS فراہم کرنے والے `ProviderAdvertV1` پے لوڈ کو پھیلاتے ہیں (دیکھیں
`crates/sorafs_manifest::provider_advert`) گورنمنٹ آپریٹر کے ذریعہ دستخط شدہ۔
اشتہارات دریافت میٹا ڈیٹا اور سرپرستوں کو ٹھیک کرتے ہیں کہ آرکسٹریٹر
ملٹی سورس رن ٹائم پر لاگو ہوتا ہے۔

- ** درستگی ** - `issued_at < expires_at <= issued_at + 86,400 s`۔ فراہم کرنے والے
  ہر 12 گھنٹے کی تجدید کرنی ہوگی۔
- ** صلاحیت TLVS ** - TLV فہرست میں ٹرانسپورٹ کی صلاحیتوں کا اشتہار دیا گیا (Torii ،
  کوئیک+شور ، سورانیٹ ریلے ، وینڈر ایکسٹینشنز)۔ نامعلوم کوڈز
  جب `allow_unknown_capabilities = true` ، کے بعد ، کو نظرانداز کیا جاسکتا ہے
  چکنائی کا رخ
- ** QoS اشارے ** - `availability` (گرم/گرم/سردی) کا درجہ ، زیادہ سے زیادہ تاخیر
  بازیابی ، مسابقت کی حد اور اختیاری اسٹریم بجٹ۔ QOS لازمی ہے
  مشاہدہ ٹیلی میٹری کے ساتھ سیدھ کریں اور داخلے کے بعد آڈٹ کیا جاتا ہے۔
- ** اختتامی نکات اور رینڈیزوس عنوانات ** - میٹا ڈیٹا کے ساتھ کنکریٹ سروس یو آر ایل
  TLS/ALPN پلس دریافت کے عنوانات جن کو کلائنٹ کو سبسکرائب کرنا ہوگا
  جب بلڈنگ گارڈ سیٹ کرتا ہے۔
- ** راہ تنوع کی پالیسی ** - `min_guard_weight` ، فین آؤٹ ٹوپیاں
  AS/پول اور `provider_failure_threshold` عزم بخش بازوں کو ممکن بناتا ہے
  ملٹی پیئر
- ** پروفائل شناخت کار ** - فراہم کنندگان کو لازمی طور پر ہینڈل کو بے نقاب کرنا ہوگا (جیسے۔
  `sorafs.sf1@1.0.0`) ؛ اختیاری `profile_aliases` میراثی صارفین کو ہجرت کرنے میں مدد کرتا ہے۔

توثیق کے قواعد صفر داؤ ، صلاحیتوں/اختتامی مقامات/عنوانات کی خالی فہرستوں کو مسترد کرتے ہیں ،
آرڈر سے باہر کی مدت یا QoS اہداف سے محروم۔ داخلہ لفافے موازنہ کریں
پھیلانے سے پہلے اشتہار اور تجویز کی لاشیں (`compare_core_fields`)
تازہ کارییں۔

### رینج بازیافت ایکسٹینشن

رینجڈ فراہم کنندگان میں مندرجہ ذیل میٹا ڈیٹا شامل ہے:

| فیلڈ | مقصد |
| ------- | ----------- |
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span` ، `min_granularity` اور سیدھ/پروف جھنڈوں کا اعلان کرتا ہے۔ |
| `StreamBudgetV1` | اختیاری مقابلہ/تھرو پٹ لفافہ (`max_in_flight` ، `max_bytes_per_sec` ، `burst` اختیاری)۔ حد کی صلاحیت کی ضرورت ہے۔ |
| `TransportHintV1` | نقل و حمل کی ترجیحات ترتیب دی گئیں (جیسے `torii_http_range` ، `quic_stream` ، `soranet_relay`)۔ ترجیحات `0-15` ہیں اور نقلیں مسترد کردی گئیں۔ |

ٹولنگ سپورٹ:

- فراہم کنندہ کے اشتہار پائپ لائنوں کو حد کی گنجائش ، اسٹریم بجٹ اور کی توثیق کرنی ہوگی
  آڈٹ کے ل detact ڈٹرمینسٹک پے لوڈ جاری کرنے سے پہلے نقل و حمل کے اشارے۔
- `cargo xtask sorafs-admission-fixtures` گروپس کیننیکل ملٹی سورس اشتہارات
  `fixtures/sorafs_manifest/provider_admission/` میں ڈاون گریڈ فکسچر کے ساتھ۔
- ان حدود کے ساتھ اشتہارات جو `stream_budget` یا `transport_hints` کو چھوڑ دیتے ہیں۔
  شیڈولنگ سے پہلے CLI/SDK لوڈرز کے ذریعہ ، کثیر سورس کنٹرول کو برقرار رکھنا
  Torii داخلہ کی توقعات کے ساتھ منسلک۔

## گیٹ وے رینج اینڈ پوائنٹسگیٹ وے ڈٹرمینسٹک HTTP کی درخواستوں کو قبول کرتے ہیں جو کے میٹا ڈیٹا کو آئینہ دار کرتے ہیں
انتباہ

### `GET /v1/sorafs/storage/car/{manifest_id}`

| ضرورت | تفصیلات |
| ----------- | ------------ |
| ** ہیڈر ** | `Range` (سنگل ونڈو منڈ آفسیٹس کے ساتھ منسلک) ، `dag-scope: block` ، `X-SoraFS-Chunker` ، `X-SoraFS-Nonce` اختیاری اور `X-SoraFS-Stream-Token` BASE64 لازمی۔ |
| ** جوابات ** | `206` `Content-Type: application/vnd.ipld.car` ، `Content-Range` کے ساتھ پیش کردہ ونڈو ، `X-Sora-Chunk-Range` میٹا ڈیٹا اور گونجنے والے چنکر/ٹوکن ہیڈرز کی وضاحت کرتا ہے۔ |
| ** ناکامی ** | `416` غلط استعمال کی حدود کے لئے ، `401` گمشدہ/غلط ٹوکن کے لئے ، `429` جب اسٹریم/بائٹ بجٹ سے تجاوز کیا جاتا ہے۔ |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

ایک ہی ہیڈر کے ساتھ سنگل حصہ بازیافت کریں۔
جب کار کے ٹکڑے غیر ضروری ہوتے ہیں تو دوبارہ کوششوں یا فرانزک ڈاؤن لوڈ کے لئے مفید ہے۔

## ملٹی سورس آرکسٹریٹر ورک فلو

جب SF-6 ملٹی سورس بازیافت فعال ہوجائے (مورچا CLI `sorafs_fetch` کے ذریعے ،
`sorafs_orchestrator` کے ذریعے SDKS):

1. ** ان پٹ اکٹھا کریں ** - منشور سے حصہ کے منصوبے کو ڈیکوڈ کریں ، کھینچیں
   تازہ ترین اشتہارات اور اختیاری طور پر ٹیلی میٹری اسنیپ شاٹ پاس کرتے ہیں
   (`--telemetry-json` یا `TelemetrySnapshot`)۔
2. ** اسکور بورڈ بنائیں ** - `Orchestrator::build_scoreboard` اس کا اندازہ کرتا ہے
   اہلیت اور ریکارڈ کو مسترد کرنے کی وجوہات ؛ `sorafs_fetch --scoreboard-out`
   JSON برقرار رہتا ہے۔
3
   رینج ، اسٹریم بجٹ ، دوبارہ کوشش/ہم مرتبہ کیپس (`--retry-budget` ، `--max-peers`)
   اور ہر درخواست کے لئے ایک منشور اسکوپڈ اسٹریم ٹوکن کا اخراج کرتا ہے۔
4.
   سی ایل آئی کے خلاصے `provider_reports` ، `chunk_receipts` اور
   `ineligible_providers` ثبوت کے بنڈل کے لئے۔

آپریٹرز/ایس ڈی کے کو پیش کی جانے والی عام غلطیاں:

| غلطی | تفصیل |
| ------ | ----------- |
| `no providers were supplied` | فلٹر کے بعد کوئی اہل اندراجات نہیں ہیں۔ |
| `no compatible providers available for chunk {index}` | ایک مخصوص حصہ کے لئے رینج یا بجٹ سے مماثلت نہیں۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` میں اضافہ کریں یا ناکام ساتھیوں کو ہٹا دیں۔ |
| `no healthy providers remaining` | بار بار ناکامیوں کے بعد تمام فراہم کنندگان کو غیر فعال کردیا گیا ہے۔ |
| `streaming observer failed` | بہاو ​​کار مصنف نے اسقاط حمل کیا۔ |
| `orchestrator invariant violated` | ٹریج کے لئے مینی فیسٹ ، اسکور بورڈ ، ٹیلی میٹری اسنیپ شاٹ اور سی ایل آئی جےسن پر قبضہ کریں۔ |

## ٹیلی میٹری اور ثبوت

- آرکسٹریٹر کے ذریعہ جاری کردہ میٹرکس:  
  `sorafs_orchestrator_active_fetches` ، `sorafs_orchestrator_fetch_duration_ms` ،
  `sorafs_orchestrator_retries_total` ، `sorafs_orchestrator_provider_failures_total`
  (منشور/خطے/فراہم کنندہ کے ذریعہ ٹیگ کردہ)۔ تشکیل میں `telemetry_region` سیٹ کریں
  یا بیڑے کے ذریعہ پارٹیشن ڈیش بورڈز میں سی ایل آئی کے جھنڈوں کے ذریعے۔
- CLI/SDK بازیافت کے خلاصے میں JSON اسکور بورڈ ، حصہ کی رسیدیں شامل ہیں
  اور رپورٹس فراہم کنندہ جو SF-6/SF-7 گیٹس کے لئے رول آؤٹ بنڈل میں جانا چاہئے۔
- گیٹ وے ہینڈلرز `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` کو بے نقاب کرتے ہیں
  SRE ڈیش بورڈز کے لئے طرز عمل کے ساتھ آرکیسٹریٹر کے فیصلوں سے وابستہ ہیں
  سرور کا

## سی ایل آئی اور ریسٹ مددگار- `iroha app sorafs pin list|show` ، `alias list` اور `replication list` میں شامل ہے
  پن رجسٹری ریسٹ اینڈ پوائنٹس اور پرنٹ Norito را json کے ساتھ بلاکس
  آڈٹ شواہد کی تصدیق۔
- `iroha app sorafs storage pin` اور `torii /v1/sorafs/pin/register` ظاہر قبول کریں
  Norito یا JSON اختیاری عرف ثبوت اور جانشینوں کے ساتھ۔ خراب ثبوت
  `400` تیار کریں ، باسی ثبوت `503` کے ساتھ `Warning: 110` ، اور میعاد ختم ہونے والے ثبوت
  واپس `412`۔
- آرام کے اختتامی مقامات (`/v1/sorafs/pin` ، `/v1/sorafs/aliases` ، `/v1/sorafs/replication`)
  گاہکوں کے خلاف اعداد و شمار کی تصدیق کے ل attactiation تصدیق کے فریم ورک شامل کریں
  اداکاری سے پہلے آخری بلاک ہیڈر۔

## حوالہ جات

- کیننیکل اسپیک:
  [`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- قسم Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- سی ایل آئی مددگار: `crates/iroha_cli/src/commands/sorafs.rs` ،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- آرکیسٹریٹر کریٹ: `crates/sorafs_orchestrator`
- ڈیش بورڈ پیک: `dashboards/grafana/sorafs_fetch_observability.json`