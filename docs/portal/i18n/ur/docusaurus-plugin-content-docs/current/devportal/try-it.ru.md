---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/try-it.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# سینڈ باکس اسے آزمائیں

ڈویلپر پورٹل میں ایک اختیاری "کوشش کریں" کنسول شامل ہے تاکہ آپ دستاویزات کو چھوڑنے کے بغیر Torii اختتامی مقامات پر کال کرسکیں۔ کنسول پراکسی ایک بلٹ ان پراکسی کے ذریعے درخواست کرتا ہے تاکہ براؤزرز کو شرح کی حدود اور توثیق کا احترام کرتے ہوئے CORS پابندیوں کو نظرانداز کرنے کی اجازت دی جاسکے۔

## شرائط

- نوڈ ڈاٹ جے ایس 18.18 یا اس کے بعد (بلڈ پورٹل کی ضروریات کو پورا کرتا ہے)
- اسٹیجنگ ماحول Torii تک نیٹ ورک تک رسائی
- بیئرر ٹوکن ، جو ضروری راستوں کو Torii پر کال کرسکتا ہے

تمام پراکسی ترتیب ماحول کے متغیر کے ذریعہ کی جاتی ہے۔ نیچے دیئے گئے جدول میں اہم نوبس کی فہرست ہے:

| متغیر | منزل | ڈیفالٹ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | بیس url Torii ، جہاں پراکسی فارورڈز درخواست کرتا ہے | ** مطلوبہ ** |
| `TRYIT_PROXY_LISTEN` | مقامی ترقی کے لئے سننے کا پتہ (فارمیٹ `host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | اصل کی فہرست جو پراکسی تک رسائی حاصل کرنے کی اجازت ہے (کوما کے ذریعہ الگ) | `Norito` |
| `TRYIT_PROXY_CLIENT_ID` | شناخت کنندہ نے ہر اپ اسٹریم درخواست کے لئے `X-TryIt-Client` میں شامل کیا `docs-portal` |
| `TRYIT_PROXY_BEARER` | ڈیفالٹ بیئرر ٹوکن Torii | پر بھیجا گیا _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | صارفین کو اپنا ٹوکن `X-TryIt-Auth` کے ذریعے منتقل کرنے کی اجازت دیں `0` |
| `TRYIT_PROXY_MAX_BODY` | زیادہ سے زیادہ درخواست جسمانی سائز (بائٹس) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ملی سیکنڈ میں اپ اسٹریم ٹائم آؤٹ | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | کلائنٹ کے IP پر ونڈو کو محدود کرنے والی شرح میں درخواستیں | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | سلائیڈنگ ونڈو ریٹ محدود (ایم ایس) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | میٹرک سننے والا پتہ Prometheus (`host:port` یا `[ipv6]:port`) | _empty (غیر فعال) _ |
| `TRYIT_PROXY_METRICS_PATH` | میٹرکس اختتامی نقطہ کے لئے HTTP راستہ | `/metrics` |

پراکسی `GET /healthz` کو بھی بے نقاب کرتا ہے ، ساختہ JSON غلطیوں اور لاگوں میں بیئرر ٹوکن میں ترمیم کرتا ہے۔

`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کو فعال کریں اگر پراکسی دستاویزات کے صارفین کو دستیاب ہے تاکہ سویگر اور ریپڈوک پینل صارف کے ذریعہ داخل کردہ بیئرر ٹوکن کو آگے بڑھا سکیں۔ پراکسی شرح کی حدود کو نافذ کرتا ہے ، اسناد میں ترمیم کرتا ہے ، اور نوٹ کرتا ہے کہ آیا درخواست میں درخواست پر ڈیفالٹ یا اوور رائڈ ٹوکن کا استعمال کیا گیا ہے۔ `TRYIT_PROXY_CLIENT_ID` کو اس لیبل کے ساتھ سیٹ کریں جس کو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(پہلے سے طے شدہ `docs-portal`)۔ پراکسی ٹرمز اور کلائنٹ سے `X-TryIt-Client` کی توثیق اور اس کی توثیق کرتا ہے اور اس ڈیفالٹ میں واپس آجاتا ہے تاکہ اسٹیجنگ گیٹ وے براؤزر میٹا ڈیٹا کے ساتھ ارتباط کے بغیر پروویژن آڈٹ کرسکیں۔

## مقامی طور پر پراکسی چلائیں

جب آپ پہلی بار پورٹل مرتب کرتے ہیں تو انحصار انسٹال کریں:

```bash
cd docs/portal
npm install
```

پراکسی شروع کریں اور اپنے Torii مثال کی وضاحت کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

اسکرپٹ بائنڈنگ ایڈریس اور پراکسی کی درخواستوں کو `/proxy/*` سے تشکیل شدہ Torii اصل تک لاگ ان کرتا ہے۔ساکٹ کو پابند کرنے سے پہلے ، اسکرپٹ چیک کرتا ہے
`static/openapi/torii.json` میں لکھا ہوا ہضم کرنے کے مساوی ہے
`static/openapi/manifest.json`۔ اگر فائلیں الگ ہوجاتی ہیں تو ، کمانڈ ناکام ہوجاتی ہے اور آپ کو `npm run sync-openapi -- --latest` چلانے کا اشارہ کرتی ہے۔ برآمد
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی بائی پاس کے لئے ہے۔ پراکسی ایک انتباہ لاگ ان کرے گی اور چلتی رہے گی تاکہ آپ بحالی کی ونڈو کے دوران صحت یاب ہوسکیں۔

## پورٹل ویجٹ کو جوڑتا ہے

ڈویلپر پورٹل کی تعمیر یا خدمت کرتے وقت ، یو آر ایل کی وضاحت کریں کہ ویجٹ کو پراکسی کے لئے استعمال کرنا چاہئے:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

مندرجہ ذیل اجزاء `docusaurus.config.js` سے اقدار پڑھتے ہیں:

- ** سویگر UI ** - `/reference/torii-swagger` پر ظاہر کیا گیا ؛ اگر ایک ٹوکن موجود ہے تو ، بیئرر اسکیم سے پہلے کی اسکیم کو ، جب `TRYIT_PROXY_PUBLIC_URL` کی وضاحت کی گئی ہے تو ، `X-TryIt-Client` ، `X-TryIt-Auth` کی درخواست کرتا ہے۔
- ** ریپڈوک ** - `/reference/torii-rapidoc` پر ظاہر کیا گیا ؛ ٹوکن فیلڈ کی عکاسی کرتا ہے ، سویگر ہیڈرز کو دوبارہ استعمال کرتا ہے اور جب دیئے گئے URL پر خود بخود پراکسی کے راستے راستے بن جاتے ہیں۔
- ** اسے کنسول آزمائیں ** - API جائزہ صفحے میں بنایا گیا۔ آپ کو اپنی مرضی کے مطابق درخواستیں بھیجنے ، ہیڈر دیکھنے اور جوابی اداروں کا معائنہ کرنے کی اجازت دیتا ہے۔

دونوں پینل ** اسنیپ شاٹ سلیکٹر ** دکھاتے ہیں جو پڑھتا ہے
`docs/portal/static/openapi/versions.json`۔ کمانڈ کے ساتھ انڈیکس کو آباد کریں
`npm run sync-openapi -- --version=<label> --mirror=current --latest` تاکہ جائزہ لینے والے تاریخی چشمی کے مابین تبدیل ہوسکیں ، ریکارڈ شدہ SHA-256 ڈائجسٹ دیکھیں ، اور اس بات کی تصدیق کریں کہ انٹرایکٹو ویجٹ استعمال کرنے سے پہلے اسنیپ شاٹ کی رہائی میں دستخط شدہ مینی فیسٹ ہے۔

کسی بھی ویجیٹ میں ٹوکن کو تبدیل کرنے سے صرف موجودہ براؤزر سیشن پر اثر پڑتا ہے۔ پراکسی کبھی بھی منتقل شدہ ٹوکن کو محفوظ یا لاگ ان نہیں کرتا ہے۔

## مختصر زندگی کے OAUTH ٹوکن

جائزہ نگاروں کے مابین طویل عرصے تک Torii ٹوکن تقسیم کرنے سے بچنے کے ل the ، کوشش کریں کہ کنسول کو OAUTH سرور سے جوڑیں۔ جب ذیل میں ماحولیاتی متغیرات موجود ہوں تو ، پورٹل ڈیوائس کوڈ ویجیٹ کو دکھاتا ہے ، قلیل زندگی گزارنے والے ٹوکن کو جاری کرتا ہے اور خود بخود انہیں کنسول کی شکل میں آباد کرتا ہے۔

| متغیر | منزل | ڈیفالٹ |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth ڈیوائس کی اجازت اختتامی نقطہ (`/oauth/device/code`) | _empty (غیر فعال) _ |
| `DOCS_OAUTH_TOKEN_URL` | ٹوکن اینڈ پوائنٹ کو قبول کرنا `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth کلائنٹ ID دستاویزات کے پیش نظارہ کے لئے رجسٹرڈ | _empty_ |
| `DOCS_OAUTH_SCOPE` | اسکوپس ، خالی جگہوں سے الگ ، لاگ ان پر درخواست کی گئی | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | ٹوکن بائنڈنگ کے لئے اختیاری API سامعین | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | تصدیق (ایم ایس) کے انتظار میں کم سے کم پولنگ کا وقفہ | `5000` (اقدار <5000 MS مسترد کردیئے گئے ہیں) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | فال بیک ٹی ٹی ایل ڈیوائس کوڈ (سیکنڈ) | `600` (300 s اور 900 s کے درمیان ہونا چاہئے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | فال بیک ٹی ٹی ایل رسائی ٹوکن (سیکنڈ) | `900` (300 اور 900s کے درمیان ہونا چاہئے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | مقامی پیش نظاروں کے لئے `1` سیٹ کریں جو جان بوجھ کر OAuth کو نظرانداز کریں _Unset_ |

مثال کی تشکیل:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```جب آپ `npm run start` یا `npm run build` چلاتے ہیں تو ، پورٹل ان اقدار کو `docusaurus.config.js` میں سرایت کرتا ہے۔ مقامی پیش نظارہ کے دوران ، ٹر آئی ٹی کارڈ "ڈیوائس کوڈ کے ساتھ سائن ان" کے بٹن کو دکھاتا ہے۔ صارفین آپ کے OAUTH تصدیقی صفحے پر دکھایا گیا کوڈ درج کرتے ہیں۔ ایک کامیاب ڈیوائس کے بہاؤ ویجیٹ کے بعد:

- جاری کردہ بیئرر ٹوکن کو کوشش کریں کہ آئی ٹی کنسول فیلڈ میں ،
- ہیڈر شامل کرتا ہے `X-TryIt-Client` اور `X-TryIt-Auth` ،
- باقی زندگی کا وقت دکھاتا ہے ،
- جب اس کی میعاد ختم ہوجاتی ہے تو خود بخود ٹوکن صاف کردیتی ہے۔

دستی بیئرر ان پٹ دستیاب ہے - اگر آپ جائزہ لینے والوں کو عارضی ٹوکن دستی طور پر داخل کرنے پر مجبور کرنا چاہتے ہیں ، یا الگ تھلگ مقامی پیش نظارہ کے لئے `DOCS_OAUTH_ALLOW_INSECURE=1` برآمد کریں جہاں گمنام رسائی قابل قبول ہے۔ OAuth کے بغیر تعمیرات اب دستاویزات -1B گیٹ کو پورا کرنے میں تیزی سے ناکام ہوجاتے ہیں۔

نوٹ: لیبارٹری کے باہر پورٹل شائع کرنے سے پہلے ، چیک کریں [سیکیورٹی ہارڈنگ اور قلم ٹیسٹ چیک لسٹ] (./security-hardening.md) ؛ اس میں دھمکی کے ماڈل ، سی ایس پی/قابل اعتماد اقسام کے پروفائل اور قلم ٹیسٹ اقدامات کی وضاحت کی گئی ہے ، جو اب گیٹ دستاویزات 1 بی ہیں۔

## مثالوں Norito-RPC

Norito-RPC درخواستیں وہی پراکسی اور OAuth پلمبنگ استعمال کریں جیسے JSON روٹس۔ انہوں نے آسانی سے `Content-Type: application/x-norito` ڈال دیا اور NRPC تفصیلات (`docs/source/torii/nrpc_spec.md`) میں بیان کردہ پری انکوڈڈ Norito پے لوڈ کو بھیجیں۔ ذخیرہ میں `fixtures/norito_rpc/` کے تحت کیننیکل پے لوڈز شامل ہیں تاکہ پورٹل مصنفین ، ایس ڈی کے مالکان اور جائزہ لینے والے CI کے ذریعہ استعمال ہونے والے عین بائٹس کو دوبارہ پیش کرسکیں۔

### کوشش کریں کہ آئی ٹی کنسول سے Norito پے لوڈ بھیجنا

1. ایک حقیقت منتخب کریں ، مثال کے طور پر `fixtures/norito_rpc/transfer_asset.norito`۔ یہ فائلیں کچی Norito لفافے ہیں۔ ** نہ کریں ** بیس 64 ان کو انکوڈ کریں۔
2. سویگر یا ریپڈوک میں ، NRPC اختتامی نقطہ (مثال کے طور پر ، `POST /v2/pipeline/submit`) تلاش کریں اور ** مواد کی قسم ** سلیکٹر کو `application/x-norito` پر تبدیل کریں۔
3. درخواست باڈی ایڈیٹر کو ** بائنری ** (Swagger میں فائل موڈ یا ریپڈوک میں بائنری/فائل موڈ میں فائل موڈ) پر سوئچ کریں اور `.norito` فائل کو لوڈ کریں۔ ویجیٹ بغیر کسی تبدیلی کے پراکسی کے ذریعے بائٹس بھیجے گا۔
4. اپنی درخواست جمع کروائیں۔ اگر Torii `X-Iroha-Error-Code: schema_mismatch` لوٹتا ہے تو ، یقینی بنائیں کہ آپ ایک اختتامی نقطہ پر کال کر رہے ہیں جو بائنری پے لوڈ کو قبول کرتا ہے اور اس بات کی تصدیق کرتا ہے کہ `fixtures/norito_rpc/schema_hashes.json` میں اسکیما ہیش Torii کی تعمیر کرتا ہے۔

کنسول آخری فائل کو میموری میں اسٹور کرتا ہے تاکہ آپ مختلف اجازت والے ٹوکن یا Torii میزبانوں کے ساتھ ایک ہی پے لوڈ کو دوبارہ بھیج سکیں۔ ورک فلو میں `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کو شامل کرنے سے NRPC-4 پر عمل درآمد کے منصوبے (LOG + JSON سمری) میں ذکر کردہ ثبوت بنڈل پیدا ہوتا ہے ، جو جائزوں کے دوران آئی ٹی کے ردعمل کے اسکرین شاٹ کے ساتھ اچھی طرح سے چلتا ہے۔

### CLI مثال (curl)

وہی فکسچر `curl` کے ذریعے پورٹل کے باہر کھیلا جاسکتا ہے ، جو پراکسی یا ڈیبگ گیٹ وے کے جوابات کی جانچ پڑتال کے لئے مفید ہے:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` سے کسی بھی اندراج کے ساتھ حقیقت کو تبدیل کریں یا اپنے پے لوڈ کو `cargo xtask norito-rpc-fixtures` کمانڈ کے ساتھ انکوڈ کریں۔ جب Torii کینری وضع میں چل رہا ہے تو ، آپ `curl` کو پورٹل ویجٹ کی طرح انفراسٹرکچر کی جانچ کرنے کے لئے TRY-IT پراکسی (`https://docs.sora.example/proxy/v2/pipeline/submit`) کی طرف اشارہ کرسکتے ہیں۔## مشاہدہ اور کاروائیاں

ہر درخواست میں ایک بار طریقہ ، راستہ ، اصل ، upstream کی حیثیت اور توثیق کا ماخذ (`override` ، `default` یا `client`) کے ساتھ لاگ ان کیا جاتا ہے۔ ٹوکن کبھی بھی ذخیرہ نہیں ہوتے ہیں - بیئرر ہیڈر اور `X-TryIt-Auth` اقدار لاگنگ سے پہلے ترمیم کی جاتی ہیں ، لہذا آپ STDOUT کو سینٹرل کلکٹر کو راز کے اخراج کے خطرے کے بغیر آگے بھیج سکتے ہیں۔

### صحت کی تحقیقات اور انتباہ

تعیناتی کے دوران یا کسی شیڈول پر بلٹ ان تحقیقات چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

ماحولیات نوبس:

- `TRYIT_PROXY_SAMPLE_PATH` - جانچ کے لئے اختیاری Torii روٹ (`/proxy` کے بغیر)۔
- `TRYIT_PROXY_SAMPLE_METHOD` - پہلے سے طے شدہ `GET` ؛ تحریری راستوں کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - نمونہ کال میں ایک عارضی بیئرر ٹوکن شامل کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - 5 s کے پہلے سے طے شدہ ٹائم آؤٹ کو اوور رائڈس۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` کے لئے اختیاری Prometheus ٹیکسٹ فائل۔
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` جوڑے میٹرکس میں شامل کیا گیا (بطور ڈیفالٹ `job=tryit-proxy` اور `instance=<proxy URL>`)۔
- `TRYIT_PROXY_PROBE_METRICS_URL` - اختیاری URL میٹرکس اینڈ پوائنٹ (مثال کے طور پر `http://localhost:9798/metrics`) ، جو `TRYIT_PROXY_METRICS_LISTEN` فعال ہونے پر کامیابی کے ساتھ جواب دینا چاہئے۔

نتائج کو ٹیکسٹ فائل کلیکٹر کو ایک قابل تحریر راستے کی طرف اشارہ کرکے (مثال کے طور پر ، `/var/lib/node_exporter/textfile_collector/tryit.prom`) اور کسٹم لیبل شامل کرکے نتائج کو منتقل کریں:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ ایٹم طور پر میٹرکس فائل کو دوبارہ لکھتا ہے تاکہ جمع کرنے والا ہمیشہ مکمل پے لوڈ کو پڑھتا ہے۔

اگر `TRYIT_PROXY_METRICS_LISTEN` تشکیل دیا گیا ہے ، سیٹ کریں
`TRYIT_PROXY_PROBE_METRICS_URL` میٹرکس کے اختتامی نقطہ پر تاکہ جب کھرچنی کی سطح ضائع ہوجائے تو تحقیقات تیزی سے کریش ہوجاتی ہیں (مثال کے طور پر ، غلط داخلہ یا فائر وال کے قواعد سے محروم)۔ عام پروڈکشن سیٹ اپ:
`TRYIT_PROXY_PROBE_METRICS_URL="Norito"`۔

آسانی سے آگاہ کرنے کے لئے ، تحقیقات کو اپنے مانیٹرنگ اسٹیک سے مربوط کریں۔ مثال Prometheus ، جو مسلسل دو کریشوں کے بعد صفحات:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### میٹرکس اینڈپوائنٹ اور ڈیش بورڈز

Prometheus-فارمیٹڈ میٹرکس اینڈ پوائنٹ کو کھولنے کے لئے پراکسی شروع کرنے سے پہلے `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (یا کوئی میزبان/بندرگاہ) سیٹ کریں۔ پہلے سے طے شدہ راستہ `/metrics` ہے ، لیکن `TRYIT_PROXY_METRICS_PATH=/custom` کے ساتھ اوورراڈ کیا جاسکتا ہے۔ ہر کھرچنی طریقوں ، شرح کی حد کو مسترد کرنے ، اپ اسٹریم غلطیاں/ٹائم آؤٹ ، پراکسی نتائج اور سمری لیٹینسی کے لئے کاؤنٹرز لوٹاتا ہے:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

براہ راست Prometheus/OTLP جمع کرنے والے میٹرکس کے اختتامی نقطہ پر اور `dashboards/grafana/docs_portal.json` سے پینلز کو دوبارہ استعمال کریں تاکہ SREs لاگوں کی تجزیہ کیے بغیر دم میں تاخیر اور ناکامیوں کے پھٹ کا مشاہدہ کرسکیں۔ پراکسی خود بخود `tryit_proxy_start_timestamp_ms` شائع کرتا ہے ، آپریٹرز کو دوبارہ شروع کرنے میں مدد کرتا ہے۔

### آٹومیشن رول بیک

ہدف URL Torii کو اپ ڈیٹ کرنے یا بحال کرنے کے لئے مینجمنٹ ہیلپر کا استعمال کریں۔ اسکرپٹ `.env.tryit-proxy.bak` میں پچھلی ترتیب کو بچاتا ہے ، لہذا رول بیک ایک کمانڈ ہے۔

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

`--env` یا `TRYIT_PROXY_ENV` کے ذریعے ENV فائل کے راستے کو اوور رائڈ کریں اگر آپ کی تعیناتی کہیں اور ترتیب دیتا ہے۔