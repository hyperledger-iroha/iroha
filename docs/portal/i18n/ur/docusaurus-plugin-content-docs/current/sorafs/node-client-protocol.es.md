---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS نوڈ ↔ کلائنٹ پروٹوکول

اس گائیڈ میں پروٹوکول کی کیننیکل تعریف کا خلاصہ کیا گیا ہے
[`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)۔
بائٹ کی سطح پر Norito لے آؤٹ کے لئے upstream تصریح کا استعمال کریں اور
چینجلاگس ؛ پورٹل کاپی آپریٹنگ پوائنٹس کو باقی کے قریب رکھتی ہے
SoraFS رن بوکس کی۔

## وینڈر کے اعلانات اور توثیق

SoraFS فراہم کرنے والے `ProviderAdvertV1` پے لوڈ کو پھیلاتے ہیں (دیکھیں
`crates/sorafs_manifest::provider_advert`) گورنمنٹ آپریٹر کے ذریعہ دستخط شدہ۔
اشتہارات نے دریافت میٹا ڈیٹا اور محافظوں کو طے کیا ہے
ملٹی سورس آرکسٹریٹر رن ٹائم پر نافذ کرتا ہے۔

- ** درستگی ** - `issued_at < expires_at ≤ issued_at + 86,400 s`۔ سپلائرز
  انہیں ہر 12 گھنٹے میں تازہ دم کرنا چاہئے۔
- ** صلاحیتوں TLVS ** - TLV فہرست نے ٹرانسپورٹ کے افعال کا اعلان کیا (Torii ،
  کوئیک+شور ، سورانیٹ ریلے ، سپلائر ایکسٹینشن)۔ نامعلوم کوڈز
  گائیڈ کے بعد ، جب `allow_unknown_capabilities = true` جب چھوڑ دیا جاسکتا ہے
  چکنائی
- ** QOS ٹریک ** - `availability` (گرم/گرم/سردی) کی سطح ، زیادہ سے زیادہ تاخیر
  بازیابی ، ہم آہنگی کی حد اور اختیاری اسٹریم بجٹ۔ Qos
  مشاہدہ شدہ ٹیلی میٹری کے ساتھ سیدھ میں ہونا چاہئے اور انٹیک پر آڈٹ کیا گیا ہے۔
- ** رینڈیزواوس اینڈ پوائنٹس اور عنوانات ** - مخصوص سروس یو آر ایل کے ساتھ
  TLS/ALPN میٹا ڈیٹا کے علاوہ دریافت کے عنوانات جس کے مؤکل ہیں
  جب گارڈ سیٹ کرتا ہے تو سبسکرائب کرنا ضروری ہے۔
- ** راہ تنوع کی پالیسی **- `min_guard_weight` ، فین آؤٹ کیپس
  AS/پول اور `provider_failure_threshold` عزم بخش بازوں کو ممکن بناتا ہے
  ملٹی پیئر
- ** پروفائل شناخت کار ** - فراہم کنندگان کو ہینڈل کو بے نقاب کرنا ہوگا
  کیننیکل (جیسے `sorafs.sf1@1.0.0`) ؛ اختیاری `profile_aliases` مدد
  پرانے کلائنٹوں کو ہجرت کریں۔

توثیق کے قواعد صفر داؤ ، صلاحیتوں کی خالی فہرستوں کو مسترد کرتے ہیں ،
اختتامی مقامات یا عنوانات ، زندگی کے اوقات میں ناگوار یا QoS مقاصد سے محروم۔
داخلہ لفافے اشتہار کی لاشوں اور تجویز کا موازنہ کرتے ہیں
(`compare_core_fields`) تازہ کاریوں کو پھیلانے سے پہلے۔

### رینج بازیافت ایکسٹینشن

درجہ بند فراہم کرنے والوں میں مندرجہ ذیل میٹا ڈیٹا شامل ہے:

| فیلڈ | مقصد |
| ------- | ------------ |
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span` ، `min_granularity` اور سیدھ/ٹیسٹ کے جھنڈوں کا اعلان کرتا ہے۔ |
| `StreamBudgetV1` | اختیاری ہم آہنگی/تھرو پٹ لفافہ (`max_in_flight` ، `max_bytes_per_sec` ، `burst` اختیاری)۔ رینج کی صلاحیت کی ضرورت ہے۔ |
| `TransportHintV1` | آرڈرڈ ٹرانسپورٹ کی ترجیحات (جیسے `torii_http_range` ، `quic_stream` ، `soranet_relay`)۔ ترجیحات `0–15` سے ہوتی ہیں اور نقلیں مسترد کردی جاتی ہیں۔ |

ٹولنگ سپورٹ:- فراہم کنندہ کے اشتہار پائپ لائنوں کو حد کی گنجائش ، ندی کی توثیق کرنی ہوگی
  آڈٹ کے ل detact ڈٹرمینسٹک پے لوڈ جاری کرنے سے پہلے بجٹ اور ٹرانسپورٹ کے اشارے۔
- `cargo xtask sorafs-admission-fixtures` پیکیج ملٹی سورس اشتہارات
  ڈاون گریڈ فکسچر کے ساتھ ساتھ کیننیکل
  `fixtures/sorafs_manifest/provider_admission/`۔
- رینج کے ساتھ اشتہارات جو `stream_budget` یا `transport_hints` کو چھوڑ دیتے ہیں
  پروگرامنگ سے پہلے CLI/SDK لوڈرز کے ذریعہ مسترد کردیا گیا ، اس کو برقرار رکھتے ہوئے
  ملٹی سورس Torii داخلے کی توقعات کے ساتھ منسلک ہے۔

## گیٹ وے رینج اینڈ پوائنٹس

گیٹ وے ڈٹرمینسٹک HTTP درخواستوں کو قبول کرتے ہیں جو کے میٹا ڈیٹا کی عکاسی کرتے ہیں
اشتہارات

### `GET /v1/sorafs/storage/car/{manifest_id}`

| ضرورت | تفصیلات |
| ----------- | ------------ |
| ** ہیڈر ** | `Range` (سنگل ونڈو منڈ آفسیٹس کے ساتھ منسلک) ، `dag-scope: block` ، `X-SoraFS-Chunker` ، `X-SoraFS-Nonce` اختیاری اور `X-SoraFS-Stream-Token` BASE64 لازمی۔ |
| ** جوابات ** | `206` `Content-Type: application/vnd.ipld.car` کے ساتھ ، `Content-Range` ونڈو کی وضاحت کرتے ہوئے ، میٹا ڈیٹا `X-Sora-Chunk-Range` اور بازگشت چنکر/ٹوکن ہیڈرز کی بازگشت۔ |
| ** ناکامی کے طریقوں ** | `416` غلط سلیگینڈ حدود کے لئے ، `401` گمشدہ یا غلط ٹوکن کے لئے ، `429` جب اسٹریم/بائٹ بجٹ سے تجاوز کیا جاتا ہے۔ |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

ایک ہی ہیڈر کے ساتھ ایک ہی حصہ لائیں نیز
ٹکڑے جب سلائسوں کی ضرورت نہیں ہوتی ہے تو دوبارہ کوششوں یا فرانزک ڈاؤن لوڈ کے ل useful مفید ہے
کار

## ملٹی سورس آرکسٹریٹر ورک فلو

جب SF-6 ملٹی سورس بازیافت فعال ہوجائے (مورچا CLI کے ذریعے `sorafs_fetch` ،
`sorafs_orchestrator` کے ذریعے SDKS):

1.
   تازہ ترین اعلانات اور اختیاری طور پر ٹیلی میٹری اسنیپ شاٹ پاس کرتے ہیں
   (`--telemetry-json` یا `TelemetrySnapshot`)۔
2. ** ایک اسکور بورڈ بنائیں ** - `Orchestrator::build_scoreboard` کا اندازہ کرتا ہے
   اہلیت اور ریکارڈ کو مسترد کرنے کی وجوہات ؛ `sorafs_fetch --scoreboard-out`
   JSON برقرار رہتا ہے۔
3
   حدود کی پابندیاں ، اسٹریم بجٹ ، دوبارہ کوشش/ہم مرتبہ کیپس
   .
   ہر درخواست کے لئے ظاہر.
4
   `provider_reports` ؛ CLI ڈائجسٹس `provider_reports` کو برقرار رکھتا ہے ،
   `chunk_receipts` اور `ineligible_providers` ثبوت کے بنڈل کے لئے۔

عام غلطیاں جو آپریٹرز/ایس ڈی کے تک پہنچ جاتی ہیں:

| غلطی | تفصیل |
| ------- | --------------- |
| `no providers were supplied` | فلٹرنگ کے بعد کوئی اہل اندراجات نہیں ہیں۔ |
| `no compatible providers available for chunk {index}` | ایک مخصوص حصہ کے لئے رینج یا بجٹ سے مماثلت نہیں۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` میں اضافہ کریں یا ناکام ساتھیوں کو خارج کریں۔ |
| `no healthy providers remaining` | بار بار ناکامیوں کے بعد تمام فراہم کنندگان کو غیر فعال کردیا گیا۔ |
| `streaming observer failed` | کار بہاو مصنف نے اسقاط حمل کیا۔ |
| `orchestrator invariant violated` | ٹریج کے لئے سی ایل آئی سے منشور ، اسکور بورڈ ، ٹیلی میٹری اسنیپ شاٹ اور جے ایس او این کی گرفتاری۔ |

## ٹیلی میٹری اور ثبوت- آرکسٹریٹر کے ذریعہ جاری کردہ میٹرکس:  
  `sorafs_orchestrator_active_fetches` ، `sorafs_orchestrator_fetch_duration_ms` ،
  `sorafs_orchestrator_retries_total` ، `sorafs_orchestrator_provider_failures_total`
  (منشور/خطے/فروش کے ذریعہ لیبل لگا ہوا ہے)۔ `telemetry_region` کو سیٹ کریں
  سی ایل آئی کے جھنڈوں کی تشکیل یا اس کے ذریعے تاکہ ڈیش بورڈز بیڑے کے ذریعہ الگ ہوجائیں۔
- CLI/SDK میں بازیافت کے خلاصے میں مستقل JSON اسکور بورڈ ، رسیدیں شامل ہیں
  ٹکڑوں اور سپلائرز کی رپورٹس کی جو رول آؤٹ بنڈل میں سفر کرنا ضروری ہیں
  SF-6/SF-7 دروازوں کے لئے۔
- گیٹ وے ہینڈلرز `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` کو بے نقاب کرتے ہیں
  تاکہ SRE ڈیش بورڈز آرکیسٹریٹر کے فیصلوں سے وابستہ ہوں
  سرور سلوک

## سی ایل آئی اور ریسٹ مدد کرتا ہے

- `iroha app sorafs pin list|show` ، `alias list` اور `replication list` لپیٹ کر
  پن رجسٹری سے آرام کے اختتامی نکات اور Norito را json کے بلاکس کے ساتھ پرنٹ کریں
  آڈٹ شواہد کی تصدیق۔
- `iroha app sorafs storage pin` اور `torii /v1/sorafs/pin/register` ظاہر قبول کریں
  Norito یا JSON کے علاوہ اختیاری عرفی اور جانشینوں کے ثبوت ؛ خراب ثبوت
  `400` کو بڑھاؤ ، متروک ثبوت `503` کو `Warning: 110` کے ساتھ بے نقاب کریں ، اور ثبوت
  میعاد ختم ہونے والی واپسی `412`۔
- باقی اختتامی مقامات (`/v1/sorafs/pin` ، `/v1/sorafs/aliases` ،
  `/v1/sorafs/replication`) تصدیق کے ڈھانچے شامل کریں تاکہ
  کلائنٹ اداکاری سے پہلے آخری بلاک کے ہیڈر کے خلاف ڈیٹا کی تصدیق کرتے ہیں۔

## حوالہ جات

- کیننیکل تفصیلات:
  [`docs/source/sorafs_node_client_protocol.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- قسم Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- سی ایل آئی سپورٹ: `crates/iroha_cli/src/commands/sorafs.rs` ،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- آرکیسٹریٹر کریٹ: `crates/sorafs_orchestrator`
- ڈیش بورڈ پیکیج: `dashboards/grafana/sorafs_fetch_observability.json`