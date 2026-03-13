---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/try-it.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# اسے سینڈ باکس آزمائیں

ڈویلپر پورٹل میں ایک اختیاری "کوشش کریں" کنسول شامل ہے تاکہ آپ دستاویزات کو چھوڑنے کے بغیر Torii اختتامی نکات پر کال کرسکیں۔ کنسول شامل پراکسی کے ذریعہ درخواستوں کو ریلے کرتا ہے تاکہ براؤزر اب بھی شرح کی حدود اور توثیق کا اطلاق کرتے ہوئے CORS کی حدود کو نظرانداز کرسکیں۔

## شرائط

- نوڈ ڈاٹ جے ایس 18.18 یا نیا (پورٹل بلڈ کی ضروریات سے میل کھاتا ہے)
- Torii کے اسٹیجنگ ماحول تک نیٹ ورک تک رسائی
- بیئرر ٹوکن جو آپ کو ورزش کرنے کا ارادہ رکھتے ہیں Torii راستوں پر کال کرسکتا ہے

ماحولیاتی متغیرات کا استعمال کرتے ہوئے تمام پراکسی کنفیگریشن کی جاتی ہے۔ مندرجہ ذیل جدول میں انتہائی اہم نوبس کی فہرست دی گئی ہے:

| متغیر | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii کا بیس URL جس میں پراکسی فارورڈز درخواستیں | ** مطلوبہ ** |
| `TRYIT_PROXY_LISTEN` | مقامی ترقی کے لئے سننے کا پتہ (`host:port` یا `[ipv6]:port` فارمیٹ) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | کوما سے الگ الگ ذرائع کی فہرست جو پراکسی کو کال کرسکتے ہیں | `Norito` |
| `TRYIT_PROXY_CLIENT_ID` | شناخت کرنے والا ہر اپ اسٹریم درخواست کے لئے `X-TryIt-Client` پر رکھا گیا ہے `docs-portal` |
| `TRYIT_PROXY_BEARER` | ڈیفالٹ بیئرر ٹوکن Torii | پر بھیج دیا گیا _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | صارفین کو `X-TryIt-Auth` کے ذریعے اپنا ٹوکن فراہم کرنے کی اجازت دیتا ہے `0` |
| `TRYIT_PROXY_MAX_BODY` | زیادہ سے زیادہ درخواست جسمانی سائز (بائٹس) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ملی سیکنڈ میں اپ اسٹریم ٹائم آؤٹ | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | درخواستوں کو فی ریٹ ونڈو فی کلائنٹ IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | شرح کو محدود کرنے (ایم ایس) کے لئے سلائڈنگ ونڈو | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus اسٹائل میٹرکس اختتامی نقطہ (`host:port` یا `[ipv6]:port`) کے لئے اختیاری سننے کا پتہ | _empty (غیر فعال) _ |
| `TRYIT_PROXY_METRICS_PATH` | میٹرکس اینڈ پوائنٹ کے ذریعہ پیش کردہ HTTP روٹ | `/metrics` |

پراکسی `GET /healthz` کو بھی بے نقاب کرتا ہے ، ساختہ JSON غلطیوں کو لوٹاتا ہے ، اور لاگ ان آؤٹ پٹ سے بیئرر ٹوکن کو چھپا دیتا ہے۔

جب DOCS صارفین کو پراکسی کو بے نقاب کرتے ہو تو `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کو متحرک کرتا ہے تاکہ سویگر اور ریپڈوک پینل صارف کے ذریعہ فراہم کردہ بیئرر ٹوکن کو آگے بھیج سکیں۔ پراکسی اب بھی ریٹ کیپس کا اطلاق کرتا ہے ، اسناد کو چھپاتا ہے ، اور لاگ ان ہوتا ہے کہ آیا کسی درخواست میں پہلے سے طے شدہ ٹوکن یا فی درخواست اوور رائڈ استعمال ہوتا ہے۔ `TRYIT_PROXY_CLIENT_ID` کو اس لیبل کے ساتھ سیٹ کریں جس کو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(پہلے سے طے شدہ `docs-portal`)۔ پراکسی ٹرمز اور کلائنٹ کے ذریعہ فراہم کردہ `X-TryIt-Client` اقدار کی توثیق اور توثیق کرتا ہے ، اس ڈیفالٹ میں واپس آجاتا ہے تاکہ اسٹیجنگ گیٹ وے براؤزر میٹا ڈیٹا سے وابستہ کیے بغیر پروویژن کا آڈٹ کرسکیں۔

## مقامی طور پر پراکسی شروع کریں

پہلی بار جب آپ پورٹل تشکیل دیتے ہیں تو انحصار انسٹال کریں:

```bash
cd docs/portal
npm install
```

پراکسی چلائیں اور اسے اپنے Torii مثال کی طرف اشارہ کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```اسکرپٹ پابند ایڈریس کو رجسٹر کرتا ہے اور `/proxy/*` سے تشکیل شدہ ماخذ Torii تک درخواستوں کو آگے بھیجتا ہے۔

ساکٹ کو پابند کرنے سے پہلے اسکرپٹ اس کی توثیق کرتا ہے
`static/openapi/torii.json` میں رجسٹرڈ ڈائجسٹ سے میل کھاتا ہے
`static/openapi/manifest.json`۔ اگر فائلیں مطابقت پذیری سے باہر ہیں تو ، کمانڈ ایک کے ساتھ ختم ہوجاتی ہے
غلطی اور آپ کو `npm run sync-openapi -- --latest` چلانے کے لئے کہتا ہے۔ برآمد
صرف ہنگامی حدود کے لئے `TRYIT_PROXY_ALLOW_STALE_SPEC=1` ؛ پراکسی رجسٹر ہوگی a
انتباہ اور جاری رہے گا تاکہ آپ بحالی کی کھڑکیوں کے دوران صحت یاب ہوسکیں۔

## پورٹل ویجٹ سے رابطہ کریں

جب آپ ڈویلپر پورٹل کی تعمیر یا خدمت کرتے ہیں تو ، یو آر ایل کی وضاحت کریں کہ ویجٹ
پراکسی کے لئے استعمال کرنا چاہئے:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

مندرجہ ذیل اجزاء `docusaurus.config.js` سے ان اقدار کو پڑھتے ہیں:

- ** سویگر UI ** - `/reference/torii-swagger` میں پیش کیا گیا ؛ اسکیم کو پہلے سے اختیار دیں
  بیئرر جب کوئی ٹوکن ہوتا ہے تو ، درخواستوں کو `X-TryIt-Client` کے ساتھ ٹیگ کریں ،
  `X-TryIt-Auth` انجیکشن لگائیں ، اور پراکسی کے ذریعے کالوں کو دوبارہ لکھتے ہیں جب
  `TRYIT_PROXY_PUBLIC_URL` کی وضاحت کی گئی ہے۔
- ** ریپڈوک ** - `/reference/torii-rapidoc` میں پیش کیا گیا ؛ ٹوکن فیلڈ کی عکاسی کرتا ہے ،
  سویگر پینل کی طرح ہیڈر کو دوبارہ استعمال کرتا ہے ، اور پراکسی کی طرف اشارہ کرتا ہے
  خود بخود جب URL تشکیل دیا جاتا ہے۔
- ** اسے کنسول آزمائیں ** - API جائزہ صفحے میں سرایت شدہ ؛ بھیجنے کی اجازت دیتا ہے
  اپنی مرضی کے مطابق درخواستیں ، ہیڈر دیکھیں اور جوابی اداروں کا معائنہ کریں۔

دونوں پینل ایک ** اسنیپ شاٹ سلیکٹر ** ظاہر کرتے ہیں جو پڑھتا ہے
`docs/portal/static/openapi/versions.json`۔ اس انڈیکس کو بھریں
جائزہ لینے والوں کے لئے `npm run sync-openapi -- --version=<label> --mirror=current --latest`
تاریخی چشمی کے درمیان کود سکتے ہیں ، لاگ ان SHA-256 ڈائجسٹ کو دیکھ سکتے ہیں ، اور تصدیق کر سکتے ہیں کہ اگر a
ریلیز اسنیپ شاٹ انٹرایکٹو ویجٹ استعمال کرنے سے پہلے ایک دستخط شدہ مینی فیسٹ لاتا ہے۔

کسی بھی ویجیٹ پر ٹوکن کو تبدیل کرنے سے صرف موجودہ براؤزر سیشن پر اثر پڑتا ہے۔ پراکسی کبھی نہیں
فراہم کردہ ٹوکن برقرار یا لاگ ان کرتا ہے۔

## مختصر زندگی کے OAUTH ٹوکن

جائزہ لینے والوں کو طویل عرصے تک Torii ٹوکن تقسیم کرنے سے بچنے کے لئے ، کوشش کریں کہ اس کو کنسول کی کوشش کریں
آپ کا OAUTH سرور۔ جب ذیل میں ماحولیاتی متغیرات موجود ہوں تو پورٹل پیش کرتا ہے a
ڈیوائس کوڈ کے ساتھ لاگ ان ویجیٹ ، بیئرر کو قلیل زندگی کے ٹوکن تیار کرتا ہے ، اور انہیں خود بخود انجکشن لگاتا ہے
کنسول فارم میں۔| متغیر | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth ڈیوائس کی اجازت اختتامی نقطہ (`/oauth/device/code`) | _empty (غیر فعال) _ |
| `DOCS_OAUTH_TOKEN_URL` | ٹوکن اینڈ پوائنٹ کو قبول کرنا `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth کلائنٹ شناخت کنندہ ڈاکٹر پیش نظارہ کے لئے رجسٹرڈ | _empty_ |
| `DOCS_OAUTH_SCOPE` | لاگ ان کے دوران درخواست کردہ خالی جگہوں کے ذریعہ الگ الگ اسکوپس | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | اختیاری API سامعین ٹوکن کو پابند کرنے کے لئے | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | منظوری کے انتظار میں کم سے کم پولنگ کا وقفہ (ایم ایس) | `5000` (اقدار <5000 MS مسترد کردیئے گئے ہیں) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | ڈیوائس کوڈ میعاد ختم ہونے والی ونڈو (سیکنڈ) | `600` (300 s اور 900 s کے درمیان برقرار رکھنا چاہئے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | رسائی ٹوکن دورانیہ (سیکنڈ) | `900` (300 s اور 900 s کے درمیان برقرار رکھنا ضروری ہے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | مقامی پیش نظاروں کے لئے `1` سیٹ کریں جو جان بوجھ کر OAUTH انفورسمنٹ کو نظرانداز کریں _Unset_ |

ترتیب مثال:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

جب آپ `npm run start` یا `npm run build` چلاتے ہیں تو ، پورٹل ان اقدار کو سرایت کرتا ہے
`docusaurus.config.js` پر۔ ایک مقامی پیش نظارہ کے دوران کوشش کریں کہ آئی ٹی کارڈ دکھاتا ہے a
"ڈیوائس کوڈ کے ساتھ سائن ان کریں" بٹن۔ صارفین آپ کے OAUTH تصدیقی صفحے پر دکھایا گیا کوڈ درج کرتے ہیں۔ ایک بار جب آلہ کا بہاؤ کامیاب ہوجائے تو ویجیٹ:

- جاری کردہ بیئرر ٹوکن کو اس کی کوشش کنسول فیلڈ میں انجیکشن کریں ،
- موجودہ ہیڈر `X-TryIt-Client` اور `X-TryIt-Auth` کے ساتھ درخواستوں کو ٹیگ کریں ،
- باقی زندگی کا وقت دکھاتا ہے ، اور
- جب اس کی میعاد ختم ہوجاتی ہے تو خود بخود ٹوکن کو حذف کردیتی ہے۔

بیئرر دستی اندراج ابھی بھی دستیاب ہے۔ جب بھی آپ چاہیں OAUTH متغیر کو نظرانداز کریں
جائزہ لینے والوں کو خود کو عارضی ٹوکن چسپاں کرنے ، یا برآمد کرنے پر مجبور کریں
`DOCS_OAUTH_ALLOW_INSECURE=1` الگ تھلگ مقامی پیش نظارہ کے لئے جہاں گمنام رسائی ہے
قابل قبول او auth ہ کے بغیر تشکیل شدہ تعمیرات اب گیٹ کو مطمئن کرنے میں تیزی سے ناکام ہوجاتے ہیں
دستاویزات -1 بی روڈ میپ کا۔

نوٹ: [سیکیورٹی ہارڈنگ اور قلم ٹیسٹ چیک لسٹ] (./security-hardening.md) چیک کریں
لیبارٹری کے باہر پورٹل کو بے نقاب کرنے سے پہلے ؛ خطرے کے ماڈل کی دستاویزات ،
سی ایس پی/قابل اعتماد اقسام کا پروفائل ، اور قلمی ٹیسٹ کے اقدامات جو اب دستاویزات 1 بی کو روکتے ہیں۔

## نمونے Norito-RPC

Norito-RPC درخواستیں ایک ہی پراکسی اور OAUTH پلمبنگ کو JSON روٹس کی طرح شیئر کرتی ہیں۔
وہ آسانی سے `Content-Type: application/x-norito` تشکیل دیتے ہیں اور پے لوڈ Norito بھیجیں
پری انکوڈنگ NRPC تفصیلات میں بیان کی گئی ہے
(`docs/source/torii/nrpc_spec.md`)۔
ذخیرہ میں `fixtures/norito_rpc/` کے تحت کیننیکل پے لوڈ شامل ہیں تاکہ پورٹل مصنفین ،
ایس ڈی کے مالکان اور جائزہ لینے والے سی آئی کے ذریعہ استعمال ہونے والے عین مطابق بائٹس کو دوبارہ پیش کرسکتے ہیں۔

### کوشش کریں کنسول سے Norito پے لوڈ بھیجیں1. `fixtures/norito_rpc/transfer_asset.norito` کی طرح ایک حقیقت کا انتخاب کریں۔ یہ
   Norito فائلیں خام لفافے ہیں۔ ** نہ کریں ** انہیں بیس 64 میں انکوڈ کریں۔
2. سویگر یا ریپڈوک میں ، NRPC اختتامی نقطہ تلاش کریں (مثال کے طور پر
   `POST /v2/pipeline/submit`) اور ** مواد کی قسم ** سلیکٹر کو تبدیل کریں
   `application/x-norito`۔
3. درخواست باڈی ایڈیٹر کو ** بائنری ** (swagger "فائل" موڈ میں تبدیل کریں یا
   ریپڈوک "بائنری/فائل" سلیکٹر) اور فائل `.norito` کو لوڈ کریں۔ ویجیٹ
   یہ بائٹس کو پراکسی کے ذریعے تبدیل کیے بغیر منتقل کرتا ہے۔
4. درخواست بھیجیں۔ اگر Torii `X-Prometheus-Error-Code: schema_mismatch` واپس کرتا ہے ،
   تصدیق کریں کہ آپ ایک اختتامی نقطہ پر کال کر رہے ہیں جو بائنری پے لوڈ کو قبول کرتا ہے اور تصدیق کرتا ہے
   `fixtures/norito_rpc/schema_hashes.json` میں رجسٹرڈ اسکیما ہیش کے مقابلے میں
   یہ Torii کی تعمیر سے مماثل ہے جو آپ استعمال کررہے ہیں۔

کنسول میموری میں حالیہ فائل کو برقرار رکھتا ہے تاکہ آپ اسے دوبارہ بھیج سکیں
Torii سے مختلف اجازت والے ٹوکن یا میزبانوں کی جانچ کرتے وقت پے لوڈ۔ شامل کریں
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` آپ کے ورک فلو میں اس کا بنڈل تیار کرتا ہے
NRPC-4 کو اپنانے کے منصوبے (لاگ + JSON سمری) میں حوالہ دیا گیا ہے ، جو جوڑتا ہے
جائزوں کے دوران کوشش کرنے والے جواب کے اسکرین شاٹس پر قبضہ کرنے کے ساتھ ٹھیک ہے۔

### CLI مثال (curl)

اسی فکسچر کو `curl` کے ذریعے پورٹل کے باہر دوبارہ پیش کیا جاسکتا ہے ، جو مفید ہے
جب آپ پراکسی یا ڈیبگ گیٹ وے کے جوابات کی توثیق کرتے ہیں:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` میں درج کسی بھی ان پٹ میں حقیقت کو تبدیل کریں
یا `cargo xtask norito-rpc-fixtures` کے ساتھ اپنے پے لوڈ کو انکوڈ کریں۔ جب Torii اندر ہے
کینری وضع آپ `curl` کو TRY-IT پراکسی کی طرف اشارہ کرسکتے ہیں
(`Norito`) ایک ہی ورزش کرنے کے لئے
انفراسٹرکچر جو پورٹل ویجٹ کے ذریعہ استعمال ہوتا ہے۔

## مشاہدہ اور کاروائیاں

ہر درخواست ایک بار طریقہ ، راستہ ، اصل ، upstream ریاست اور ماخذ کے ساتھ رجسٹرڈ ہے
توثیق (`override` ، `default` یا `client`)۔ ٹوکن کبھی بھی ذخیرہ نہیں ہوتے ہیں: دونوں
بیئرر ہیڈر جیسے `X-TryIt-Auth` اقدار لاگ ان کرنے سے پہلے دوبارہ تیار کیے جاتے ہیں ،
لہذا آپ لیک کے بارے میں فکر کیے بغیر STDOUT کو مرکزی کلکٹر کو آگے بھیج سکتے ہیں۔

### صحت کی تحقیقات اور انتباہات

تعیناتی کے دوران یا کسی شیڈول پر شامل تحقیقات چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

ماحولیات نوبس:

- `TRYIT_PROXY_SAMPLE_PATH` - اختیاری Torii روٹ (بغیر `/proxy` کے) ورزش کرنے کے لئے۔
- `TRYIT_PROXY_SAMPLE_METHOD` - پہلے سے طے شدہ `GET` ؛ تحریری راستوں کے لئے `POST` کی وضاحت کرتا ہے۔
- `TRYIT_PROXY_PROBE_TOKEN` - نمونہ کال کے لئے ایک عارضی بیئرر ٹوکن انجیکشن کریں۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - 5 s کے پہلے سے طے شدہ ٹائم آؤٹ کو اوور رائڈس۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - اختیاری متن منزل Prometheus برائے `probe_success`/`probe_duration_seconds`۔
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` جوڑے کوما کے ذریعہ الگ الگ میٹرکس میں شامل (بطور ڈیفالٹ `job=tryit-proxy` اور `instance=<proxy URL>`)۔
- `TRYIT_PROXY_PROBE_METRICS_URL` - میٹرکس کے اختتامی نقطہ کا اختیاری URL (مثال کے طور پر ، `http://localhost:9798/metrics`) جو `TRYIT_PROXY_METRICS_LISTEN` فعال ہونے پر کامیابی کے ساتھ جواب دینا چاہئے۔نتائج کو ایک ٹیکسٹ فائل جمع کرنے والے میں کھانا کھلانا ہے جس میں تحقیقات کو ایک تحریر کے راستے کی طرف اشارہ کیا گیا ہے
(جیسے `/var/lib/node_exporter/textfile_collector/tryit.prom`) اور ٹیگ شامل کرنا
مشخص:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ میٹرک فائل کو جوہری طور پر دوبارہ لکھتا ہے تاکہ آپ کا جمع کرنے والا ہمیشہ پڑھتا ہے
ایک مکمل پے لوڈ۔

جب `TRYIT_PROXY_METRICS_LISTEN` تشکیل دیا جاتا ہے ، تو یہ وضاحت کرتا ہے
`TRYIT_PROXY_PROBE_METRICS_URL` میٹرکس کے اختتامی نقطہ پر تاکہ تحقیقات میں تیزی سے ناکام ہوجاتا ہے اگر
کھرچنی سطح غائب ہوجاتی ہے (جیسے غلط کنفیگرڈ انجری یا فائر وال کے ضوابط سے محروم)۔
ایک عام پیداوار کی ترتیب ہے
`TRYIT_PROXY_PROBE_METRICS_URL="Norito"`۔

روشنی کے انتباہات کے ل the ، تحقیقات کو اپنے مانیٹرنگ اسٹیک سے مربوط کریں۔ Prometheus کی مثال
مسلسل دو ناکامیوں کے بعد صفحہ:

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

### میٹرکس اور ڈیش بورڈز اینڈ پوائنٹ

اس سے پہلے `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (یا کوئی میزبان/پورٹ جوڑی) تشکیل دیں
فارمیٹ Prometheus کے ساتھ میٹرکس کے اختتامی نقطہ کو بے نقاب کرنے کے لئے پراکسی شروع کریں۔ راستہ
پہلے سے طے شدہ `/metrics` ہے لیکن اسے `TRYIT_PROXY_METRICS_PATH=/custom` میں تبدیل کیا جاسکتا ہے۔ ہر ایک
کھرچنا کل کاؤنٹرز کو طریقہ کار کے ذریعہ لوٹاتا ہے ، شرح کی حد ، غلطیاں/ٹائم آؤٹ کے ذریعہ مسترد
اپ اسٹریم ، پراکسی نتائج اور تاخیر کے خلاصے:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

اپنے Prometheus/OTLP جمعکاروں کو میٹرکس کے اختتامی نقطہ کی طرف اشارہ کریں اور موجودہ پینلز کو دوبارہ استعمال کریں
`dashboards/grafana/docs_portal.json` میں SRE کے لئے دم کی تاخیر اور اسپائکس کا مشاہدہ کرنے کے لئے
نوشتہ جات کی تجزیہ کیے بغیر مسترد. پراکسی خود بخود `tryit_proxy_start_timestamp_ms` شائع کرتا ہے
آپریٹرز کو دوبارہ شروع کرنے میں مدد کرنے کے لئے۔

### رول بیک آٹومیشن

Torii کے ہدف URL کو اپ ڈیٹ کرنے یا بحال کرنے کے لئے مینجمنٹ ہیلپر کا استعمال کریں۔ اسکرپٹ
`.env.tryit-proxy.bak` میں پچھلی ترتیب کو محفوظ کریں تاکہ رول بیکس ایک ہوں
صرف کمانڈ۔

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

`--env` یا `TRYIT_PROXY_ENV` کے ساتھ ENV ​​فائل کے راستے کو اوور رائٹ کریں اگر آپ کی تعیناتی
ترتیب کو کہیں اور محفوظ کریں۔