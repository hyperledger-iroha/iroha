---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پروٹوکول نوڈ ↔ کلائنٹ SoraFS

اس گائیڈ میں پروٹوکول میں اپنایا ہوا تعریف کا خلاصہ کیا گیا ہے
[`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)۔
Norito بائٹ سطح کے میپنگز اور چینجلوگ کے لئے upstream تصریح کا استعمال کریں۔
پورٹل ورژن آپریشنل جھلکیاں SoraFS کے باقی رن بوکس کے قریب رکھتا ہے۔

## فراہم کنندہ اشتہارات اور توثیق

SoraFS فراہم کنندگان `ProviderAdvertV1` پے لوڈ نشر کرتے ہیں (دیکھیں
`crates/sorafs_manifest::provider_advert`) گورنمنٹ آپریٹر کے ذریعہ دستخط شدہ۔
اعلانات ملٹی سورس آرکیسٹریٹر کے ذریعہ عائد کردہ دریافت کے اعداد و شمار اور کنٹرولوں کی توثیق کرتے ہیں
آپریشن کے دوران

- ** شیلف لائف ** - `issued_at < expires_at ≤ issued_at + 86,400 s`۔ مجھے چاہئے
  فراہم کرنے والے ہر 12 گھنٹے میں تجدید کرتے ہیں۔
- ** TLV صلاحیتیں ** - TLV مینو میں منتقلی کی خصوصیات (Torii ، کوئیک+شور ، ریلے کی فہرست ہے
  سورانیٹ ، ایک توسیع فراہم کنندہ)۔ گمنام کوڈز کو نظرانداز کیا جاسکتا ہے جب ...
  چکنائی کی ہدایات پر عمل کرتے ہوئے `allow_unknown_capabilities = true`۔
- ** QOS اشارے ** - `availability` پرت (گرم/گرم/سردی) ، زیادہ سے زیادہ لوپ بیک لیٹینسی ،
  ہم آہنگی کی حد اور اسٹریم بجٹ اختیاری ہیں۔ Qos ٹیلی میٹری کے ساتھ مطابقت پذیر ہونا چاہئے
  نوٹ اور قبولیت کے لئے جائزہ لیا جاتا ہے۔
- ** اختتامی نکات اور رینڈیزوس عنوانات ** - TLS/ALPN ڈیٹا کے ساتھ مخصوص خدمت کے پتے شامل کریں
  گارڈ گروپس کی تعمیر کرتے وقت گاہکوں کو مشغول کرنے کے موضوعات کے مطابق۔
- ** راہ تنوع کی پالیسی **- `min_guard_weight` اور AS/پول کے لئے فین آؤٹ حدود اور
  `provider_failure_threshold` تعصب سے متعلق کثیر الجہتی بازیافت کو ممکن بناتا ہے۔
- ** پروفائل آئی ڈی ** - فراہم کنندگان کو منظور شدہ ہینڈل کو شائع کرنا ہوگا (جیسے
  `sorafs.sf1@1.0.0`) ؛ اختیاری `profile_aliases` پرانے صارفین کی مدد کرتا ہے
  جلاوطنی

صفر اسٹیک توثیق کے قواعد ، یا خالی قابلیت/عنوان/عنوان کی فہرستیں مسترد کردی گئیں۔
یا آرڈر سے باہر تاریخیں ، یا QoS اہداف سے محروم ہیں۔ قبولیت کے لفافے اشتہار کے مواد کا موازنہ کرتے ہیں
اور اپڈیٹس کو نشر کرنے سے پہلے مشورہ (`compare_core_fields`)۔

### اسکوپس کے ساتھ توسیع کی توسیع

ڈومین کی حمایت کرنے والے فراہم کنندگان میں درج ذیل ڈیٹا شامل ہیں:

| فیلڈ | مقصد |
| ------- | ------- |
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span` اور `min_granularity` اور سیدھ کے جھنڈوں/گائڈز کا اعلان کرتا ہے۔ |
| `StreamBudgetV1` | اختیاری ہم وقت سازی/شرح ریپر (`max_in_flight` ، `max_bytes_per_sec` ، اور `burst` اختیاری)۔ حد کی گنجائش کی ضرورت ہے۔ |
| `TransportHintV1` | منتقلی کی ترجیحات کا حکم دیا گیا (جیسے `torii_http_range` ، `quic_stream` ، `soranet_relay`)۔ ترجیحات `0–15` میں ہیں اور نقلیں مسترد کردی گئیں۔ |

ٹولز سپورٹ:

- فراہم کنندہ کے اشتہار لائنوں کو بینڈوتھ کی گنجائش ، اسٹریم بجٹ اور اشارے کی جانچ کرنی چاہئے
  آڈیٹنگ کے لئے لازمی پے لوڈ جاری کرنے سے پہلے ٹرانسپورٹ۔
- `cargo xtask sorafs-admission-fixtures` ملٹی سورس اشتہارات کے ساتھ مل کر
  `fixtures/sorafs_manifest/provider_admission/` کے تحت کم کرنے کے لئے فکسچر۔
- ڈومین کی حمایت کرنے والے اشتہارات جو `stream_budget` یا `transport_hints` کو چھوڑ دیتے ہیں۔
  شیڈولنگ سے پہلے CLI/SDK لوڈرز کے ذریعہ ، ملٹی سورس راہ کے ساتھ مطابقت رکھتے ہوئے
  Torii قبولیت کی توقعات۔

## گیٹ وے ڈومین اینڈ پوائنٹس

گیٹ وے ڈٹرمینسٹک HTTP درخواستوں کو قبول کرتے ہیں جو اشتہاری اعداد و شمار کی عکاسی کرتے ہیں۔

### `GET /v2/sorafs/storage/car/{manifest_id}`| ضرورت | تفصیلات |
| --------- | ------------ |
| ** ہیڈر ** | `Range` (ایک ونڈو سلائیڈ آفسیٹس کے ساتھ کھڑی ہے) ، `dag-scope: block` ، `X-SoraFS-Chunker` ، `X-SoraFS-Nonce` اختیاری ہے ، اور `X-SoraFS-Stream-Token` Base64 لازمی ہے۔ |
| ** جوابات ** | `206` کے ساتھ `Content-Type: application/vnd.ipld.car` ، `Content-Range` پیش کردہ ونڈو ، `X-Sora-Chunk-Range` ڈیٹا ، اور Chuncer/ٹوکن ہیڈروں کو بازیافت کرتا ہے۔ |
| ** ناکامی کے طریقوں ** | `416` سیدھ سے باہر ، `401` گمشدہ/غلط علامتوں کے لئے ، `429` جب اسٹریم/بائٹ بجٹ سے تجاوز کیا جاتا ہے۔ |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

ایک ہی ہیڈر کے ساتھ ایک سلائس لائیں نیز سلائس کے لازمی ڈائجسٹ۔ ری پلے کے لئے مفید ہے
جب کار کے چپس ضروری نہیں ہوں تو کوشش کریں یا فرانزک ڈاؤن لوڈ کریں۔

## ملٹی سورس آرکسٹریٹر ورک فلو

جب SF-6 ملٹی سورس بازیافت فعال ہوجاتی ہے (CLI مورچا `sorafs_fetch` کے ذریعے ، SDKS کے ذریعے
`sorafs_orchestrator`):

1. ** ان پٹ اکٹھا کریں ** - ظاہر سلائیڈوں کو کھولیں ، تازہ ترین اعلانات لائیں ، اور پاس کریں
   اختیاری ٹیلی میٹرک شاٹ (`--telemetry-json` یا `TelemetrySnapshot`)۔
2. ** اسکور بورڈ بنائیں ** - `Orchestrator::build_scoreboard` اہلیت کا اندازہ کرتا ہے
   مسترد ہونے کی وجوہات کی ریکارڈنگ ؛ `sorafs_fetch --scoreboard-out` JSON فائل کو محفوظ کرتا ہے۔
3.
   اسٹریم بجٹ ، دوبارہ کوشش کرنے والی حدود/ہم عمر (`--retry-budget` ، `--max-peers`)
   ہر درخواست کے لئے مینی فیسٹ رینج میں ایک اسٹریم ٹوکن جاری کیا جاتا ہے۔
4.
   سی ایل آئی کے خلاصے `provider_reports` ، `chunk_receipts` ، اور `ineligible_providers` کو بچائیں
   شواہد پیکیج کرنے کے لئے.

آپریٹرز/ایس ڈی کے کو مارنے والی عام غلطیاں:

| غلطی | تفصیل |
| ------- | ------- |
| `no providers were supplied` | فلٹرنگ کے بعد کوئی اہل اندراجات نہیں ہیں۔ |
| `no compatible providers available for chunk {index}` | کسی مخصوص طبقے کے لئے دائرہ کار یا بجٹ سے مماثلت۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` میں اضافہ کریں یا ڈیفالٹنگ ساتھیوں کو خارج کریں۔ |
| `no healthy providers remaining` | بار بار ناکامیوں کے بعد تمام فراہم کنندگان کو غیر فعال کردیا گیا۔ |
| `streaming observer failed` | کار اسٹارٹر نچلے راستے پر گر گیا۔ |
| `orchestrator invariant violated` | تجزیہ کے لئے مینی فیسٹ ، اسکور بورڈ ، ٹیلی میٹک اسنیپ شاٹ ، اور سی ایل آئی جےسن پر قبضہ کریں۔ |

## ٹیلی میٹری اور ثبوت

- کوآرڈینیٹر کے ذریعہ جاری کردہ معیارات:  
  `sorafs_orchestrator_active_fetches` ، `sorafs_orchestrator_fetch_duration_ms` ،
  `sorafs_orchestrator_retries_total` ، `sorafs_orchestrator_provider_failures_total`
  (منشور/خطے/فراہم کنندہ کے طور پر ٹیگ کردہ)۔ `telemetry_region` سیٹ اپ میں یا ویا میں سیٹ کریں
  سی ایل آئی کے جھنڈے تو ڈیش بورڈز کو بیڑے کے ذریعہ تقسیم کیا جاتا ہے۔
- CLI/SDK میں بازیافت کے خلاصے میں محفوظ کردہ اسکور بورڈ JSON فائل اور سلائیڈ کی رسیدیں شامل ہیں
  اور سپلائر رپورٹس جن کو SF-6/SF-7 گیٹس کے لانچ پیکجوں میں بھیجنا ضروری ہے۔
- گیٹ پروسیسرز `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` کا پتہ لگاتے ہیں
  تاکہ SRE بورڈ کوآرڈینیٹر کے فیصلوں کو سرور کے طرز عمل سے جوڑ سکتے ہیں۔

## سی ایل آئی اور ریسٹ مددگار

- `iroha app sorafs pin list|show` ، `alias list` اور `replication list` آرام سے نوڈس کو انکولیٹ کریں
  Norito آڈٹ شواہد کے لئے تصدیق شدہ بلاکس کے ساتھ خام JSON کو ریکارڈ کرتا ہے اور پرنٹ کرتا ہے۔
- `iroha app sorafs storage pin` اور `torii /v2/sorafs/pin/register` ظاہر قبول کریں
  Norito یا JSON میں عرف اور جانشین کے لئے اختیاری ثبوتوں کے ساتھ۔ لیڈ ثبوت
  `400` میں مسخ شدہ ، اور پرانے ثبوت `503` کو `Warning: 110` کے ساتھ دکھاتے ہیں ، جبکہ بحالی کرتے ہوئے
  مکمل طور پر ختم ہونے والے ثبوت `412`۔
- ریسٹ پوائنٹس (`/v2/sorafs/pin` ، `/v2/sorafs/aliases` ، `/v2/sorafs/replication`)
  تصدیق کے ڈھانچے پر مشتمل ہے تاکہ کلائنٹ تازہ ترین کے خلاف ڈیٹا چیک کرسکیں
  پھانسی سے پہلے ہیڈر بلاک کریں۔## حوالہ جات

- منظور شدہ تفصیلات:
  [`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito کی اقسام: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI مدد: `crates/iroha_cli/src/commands/sorafs.rs` ،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
فارمیٹ لائبریری: `crates/sorafs_orchestrator`
- مانیٹرنگ بورڈ پیکیج: `dashboards/grafana/sorafs_fetch_observability.json`