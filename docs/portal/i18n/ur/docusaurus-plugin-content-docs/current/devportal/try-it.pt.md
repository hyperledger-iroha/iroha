---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/try-it.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# اسے سینڈ باکس آزمائیں

ڈویلپر پورٹل میں ایک اختیاری "کوشش کریں" کنسول شامل ہے تاکہ آپ دستاویزات کو چھوڑنے کے بغیر Torii اختتامی نکات پر کال کرسکیں۔ کنسول بلٹ ان پراکسی کے ذریعہ درخواستوں کو ریلے کرتا ہے تاکہ براؤزر اب بھی شرح کی حدود اور توثیق کا اطلاق کرتے ہوئے CORS کی حدود کو نظرانداز کرسکیں۔

## شرائط

- نوڈ ڈاٹ جے ایس 18.18 یا نیا (پورٹل بلڈ کی ضروریات سے میل کھاتا ہے)
- Torii اسٹیجنگ ماحول تک نیٹ ورک تک رسائی
- بیئرر ٹوکن جو Torii راستوں کو کال کرسکتا ہے جس کا آپ ورزش کرنا چاہتے ہیں

ماحولیاتی متغیرات کا استعمال کرتے ہوئے تمام پراکسی کنفیگریشن کی جاتی ہے۔ نیچے دیئے گئے جدول میں سب سے اہم نوبس کی فہرست ہے:

| متغیر | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii کا بیس URL جس میں پراکسی فارورڈز درخواستیں | ** مطلوبہ ** |
| `TRYIT_PROXY_LISTEN` | مقامی ترقی کے لئے سننے کا پتہ (فارمیٹ `host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | کوما سے الگ الگ ذرائع کی فہرست جو پراکسی کو کال کرسکتے ہیں | `Norito` |
| `TRYIT_PROXY_CLIENT_ID` | شناخت کرنے والا ہر اپ اسٹریم درخواست کے لئے `X-TryIt-Client` میں رکھا گیا ہے `docs-portal` |
| `TRYIT_PROXY_BEARER` | ڈیفالٹ ٹوکن بیئرر Torii | پر بھیج دیا گیا _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | اختتامی صارفین کو `X-TryIt-Auth` کے ذریعے اپنا ٹوکن فراہم کرنے کی اجازت دیتا ہے `0` |
| `TRYIT_PROXY_MAX_BODY` | درخواست جسم کا زیادہ سے زیادہ سائز (بائٹس) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ملی سیکنڈ میں اپ اسٹریم ٹائم آؤٹ | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | درخواستوں کو فی ریٹ ونڈو فی کلائنٹ IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | شرح کو محدود کرنے (ایم ایس) کے لئے سلائڈنگ ونڈو | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus اسٹائل میٹرکس اختتامی نقطہ (`host:port` یا `[ipv6]:port`) کے لئے اختیاری سننے کا پتہ | _empty (غیر فعال) _ |
| `TRYIT_PROXY_METRICS_PATH` | میٹرکس اختتامی نقطہ کے ذریعہ پیش کردہ HTTP راستہ | `/metrics` |

پراکسی `GET /healthz` کو بھی بے نقاب کرتا ہے ، ساختہ JSON غلطیوں کو لوٹاتا ہے ، اور نوشتہ جات میں بیئرر ٹوکن ماسک کرتا ہے۔

جب ڈی او سی صارفین کو پراکسی کو بے نقاب کرتے ہو تو `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کو فعال کریں تاکہ سویگر اور ریپڈوک پینل صارف کے ذریعہ فراہم کردہ بیئرر ٹوکن کو آگے بڑھا سکیں۔ پراکسی اب بھی شرح کی حدود ، ماسک اسناد اور ریکارڈوں کا اطلاق کرتا ہے چاہے کسی درخواست میں پہلے سے طے شدہ ٹوکن یا فی درخواست اوور رائڈ استعمال کیا جائے۔ `TRYIT_PROXY_CLIENT_ID` کو اس لیبل کے ساتھ تشکیل دیں جس کو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(پہلے سے طے شدہ `docs-portal`)۔ پراکسی کلائنٹ کے ذریعہ فراہم کردہ `X-TryIt-Client` اقدار کو کٹوتی اور توثیق کرتا ہے ، اس ڈیفالٹ پر واپس آجاتا ہے تاکہ اسٹیجنگ گیٹ وے براؤزر میٹا ڈیٹا سے وابستہ کیے بغیر پروویژن کا آڈٹ کرسکیں۔

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
```

اسکرپٹ پابند ایڈریس رجسٹر کرتا ہے اور `/proxy/*` سے تشکیل شدہ Torii ماخذ تک درخواستوں کو آگے بھیجتا ہے۔ساکٹ کو پابند کرنے سے پہلے ، اسکرپٹ اس کی توثیق کرتا ہے
`static/openapi/torii.json` رجسٹرڈ ڈائجسٹ سے مطابقت رکھتا ہے
`static/openapi/manifest.json`۔ اگر فائلیں ہٹ جاتی ہیں تو ، کمانڈ کسی غلطی کے ساتھ ختم ہوجاتی ہے اور
آپ کو `npm run sync-openapi -- --latest` پر عمل کرنے کی ہدایت کرتا ہے۔ برآمد
صرف ہنگامی حدود کے لئے `TRYIT_PROXY_ALLOW_STALE_SPEC=1` ؛ پراکسی ایک انتباہ لاگ ان کرتا ہے
اور جاری ہے تاکہ آپ بحالی کی کھڑکیوں کے دوران صحت یاب ہوسکیں۔

## پورٹل ویجٹ سے رابطہ کریں

جب آپ ڈویلپر پورٹل کی تعمیر یا خدمت کرتے ہیں تو ، یو آر ایل کی وضاحت کریں کہ ویجٹ کو ہونا چاہئے
پراکسی کے لئے استعمال کریں:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

نیچے دیئے گئے اجزاء `docusaurus.config.js` سے ان اقدار کو پڑھیں:

- ** سویگر UI ** - `/reference/torii-swagger` میں پیش کیا گیا ؛ اسکیم کو پیش کرتا ہے
  بیئرر جب کوئی ٹوکن ہوتا ہے تو ، `X-TryIt-Client` کے ساتھ درخواستوں کو نشان زد کرتا ہے ،
  `X-TryIt-Auth` انجیکشن لگائیں ، اور پراکسی کے ذریعے کالوں کو دوبارہ لکھیں
  `TRYIT_PROXY_PUBLIC_URL` تشکیل شدہ ہے۔
- ** ریپڈوک ** - `/reference/torii-rapidoc` میں پیش کیا گیا ؛ ٹوکن فیلڈ کو آئینہ دیتا ہے ،
  سویگر پینل سے ایک ہی ہیڈر کو دوبارہ استعمال کریں اور پراکسی کی طرف اشارہ کریں
  خود بخود جب URL تشکیل دیا جاتا ہے۔
- ** اسے کنسول آزمائیں ** - API جائزہ صفحے میں سرایت شدہ ؛ آپ کو بھیجنے کی اجازت دیتا ہے
  اپنی مرضی کے مطابق درخواستیں ، ہیڈر دیکھیں اور جوابی اداروں کا معائنہ کریں۔

دونوں پینل ایک ** اسنیپ شاٹ سلیکٹر ** دکھاتے ہیں جو پڑھتا ہے
`docs/portal/static/openapi/versions.json`۔ اس انڈیکس کو بھریں
جائزہ لینے والوں کے لئے `npm run sync-openapi -- --version=<label> --mirror=current --latest`
تاریخی چشمی کے مابین سوئچ کرسکتے ہیں ، ریکارڈ شدہ SHA-256 ڈائجسٹ کو دیکھ سکتے ہیں ، اور اس کی تصدیق کرسکتے ہیں
ایک ریلیز اسنیپ شاٹ انٹرایکٹو ویجٹ استعمال کرنے سے پہلے دستخط شدہ مینی فیسٹ کو لوڈ کرتا ہے۔

کسی بھی ویجیٹ میں ٹوکن کو تبدیل کرنے سے صرف موجودہ براؤزر سیشن پر اثر پڑتا ہے۔ پراکسی کبھی بھی برقرار نہیں رہتی
نہ ہی یہ فراہم کردہ ٹوکن لاگ ان ہوتا ہے۔

## مختصر زندگی کے OAUTH ٹوکن

جائزہ لینے والوں کو طویل عرصے تک Torii ٹوکن تقسیم کرنے سے بچنے کے لئے ، کوشش کریں کہ اس کو کنسول کی کوشش کریں
آپ کا OAUTH سرور۔ جب ذیل میں ماحولیاتی متغیرات موجود ہوں تو ، پورٹل پیش کرتا ہے
ڈیوائس کوڈ کے ساتھ ایک لاگ ان ویجیٹ ، مختصر المیعاد بیئرر ٹوکن تیار کرتا ہے اور ان کو انجیکشن دیتا ہے
خود بخود کنسول کی شکل میں۔

| متغیر | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth ڈیوائس کی اجازت اختتامی نقطہ (`/oauth/device/code`) | _empty (غیر فعال) _ |
| `DOCS_OAUTH_TOKEN_URL` | ٹوکن اینڈ پوائنٹ جو `grant_type=urn:ietf:params:oauth:grant-type:device_code` کو قبول کرتا ہے _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | ڈاکٹر پیش نظارہ کے لئے رجسٹرڈ OAuth کلائنٹ شناخت کنندہ | _empty_ |
| `DOCS_OAUTH_SCOPE` | لاگ ان میں درخواست کردہ خلائی سے الگ الگ اسکوپس | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | بائنڈنگ ٹوکن کے لئے اختیاری API سامعین | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | منظوری کے انتظار میں کم سے کم پولنگ کا وقفہ (ایم ایس) | `5000` (اقدار <5000 MS مسترد کردیئے گئے ہیں) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | ڈیوائس کوڈ میعاد ختم ہونے والی ونڈو (سیکنڈ) | `600` (300 s اور 900 s کے درمیان ہونا چاہئے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | رسائی ٹوکن دورانیہ (سیکنڈ) | `900` (300 s اور 900 s کے درمیان ہونا چاہئے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | مقامی پیش نظاروں کے لئے `1` سیٹ کریں جو جان بوجھ کر OAUTH انفورسمنٹ کو چھوڑ دیں _Unset_ |

ترتیب مثال:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```جب آپ `npm run start` یا `npm run build` چلاتے ہیں تو ، پورٹل ان اقدار کو سرایت کرتا ہے
`docusaurus.config.js`۔ ایک مقامی پیش نظارہ کے دوران کوشش کریں کہ آئی ٹی کارڈ ایک بٹن دکھاتا ہے
"ڈیوائس کوڈ کے ساتھ سائن ان کریں"۔ صارفین آپ کے OAUTH صفحے پر دکھایا گیا کوڈ درج کرتے ہیں۔ جب آلہ کا بہاؤ ویجیٹ کو کامیاب کرتا ہے:

- جاری کردہ بیئرر ٹوکن کو کوشش کریں کہ کوشش کنسول فیلڈ میں ،
- موجودہ ہیڈر `X-TryIt-Client` اور `X-TryIt-Auth` کے ساتھ درخواستوں کو نشان زد کرتا ہے ،
- باقی زندگی کا وقت دکھاتا ہے ، اور
- جب اس کی میعاد ختم ہوجاتی ہے تو خود بخود ٹوکن صاف کردیتی ہے۔

بیئرر دستی ان پٹ دستیاب ہے۔ جب بھی آپ چاہیں Oauth متغیر کو چھوڑ دیں
جائزہ لینے والوں کو خود ہی ایک عارضی ٹوکن چسپاں کرنے پر مجبور کریں ، یا برآمد کریں
`DOCS_OAUTH_ALLOW_INSECURE=1` الگ تھلگ مقامی پیش نظارہ کے لئے جہاں گمنام رسائی اور
قابل قبول تعمیر کے بغیر تعمیرات اب گیٹ وے سے ملنے میں تیزی سے ناکام ہوجاتے ہیں
دستاویزات -1 بی روڈ میپ۔

نوٹ: [سیکیورٹی ہارڈنگ اور قلم ٹیسٹ چیک لسٹ] کا جائزہ لیں (./security-hardening.md)
لیبارٹری کے باہر پورٹل کو بے نقاب کرنے سے پہلے ؛ یہ خطرے کے ماڈل کی دستاویز کرتا ہے ،
سی ایس پی/قابل اعتماد اقسام کا پروفائل اور قلم ٹیسٹ کے اقدامات جو اب دستاویزات 1-بی کو روکتے ہیں۔

## نمونے Norito-RPC

Norito-RPC درخواستیں ایک ہی پراکسی اور OAUTH پلمبنگ کو JSON روٹس کی طرح شیئر کرتی ہیں۔
وہ صرف `Content-Type: application/x-norito` مرتب کرتے ہیں اور پے لوڈ Norito بھیجیں
پری انکوڈڈ NRPC تفصیلات میں بیان کیا گیا ہے
(`docs/source/torii/nrpc_spec.md`)۔
ذخیرے میں `fixtures/norito_rpc/` کے تحت کیننیکل پے لوڈ شامل ہیں تاکہ مصنفین کے مصنفین
پورٹل ، ایس ڈی کے مالکان اور جائزہ لینے والے عین بائٹس کو دوبارہ پیش کرسکتے ہیں جو سی آئی استعمال کرتا ہے۔

### کوشش کریں کنسول سے Norito پے لوڈ بھیجیں

1. `fixtures/norito_rpc/transfer_asset.norito` کی طرح ایک حقیقت کا انتخاب کریں۔ یہ
   فائلیں کچی Norito لفافے ہیں۔ ** نہیں ** بیس 64 کریں۔
2. سویگر یا ریپڈوک میں ، NRPC اختتامی نقطہ تلاش کریں (جیسے۔
   `POST /v2/pipeline/submit`) اور ** مواد کی قسم ** سلیکٹر کو تبدیل کریں
   `application/x-norito`۔
3. باڈی ایڈیٹر کو ** بائنری ** ("فائل" موڈ میں تبدیل کریں یا میں تبدیل کریں
   ریپڈوک کی "بائنری/فائل" سلیکٹر) اور فائل `.norito` بھیجیں۔ ویجیٹ
   بغیر کسی تبدیلی کے پراکسی کے ذریعے بائٹس کو منتقل کرتا ہے۔
4. درخواست بھیجیں۔ اگر Torii `X-Prometheus-Error-Code: schema_mismatch` واپس کرتا ہے ،
   چیک کریں کہ کیا آپ کسی اختتامی نقطہ پر کال کر رہے ہیں جو بائنری پے لوڈ کو قبول کرتا ہے اور تصدیق کرتا ہے
   کہ ہیش اسکیما `fixtures/norito_rpc/schema_hashes.json` میں رجسٹرڈ ہے
   Torii کی تعمیر سے مطابقت رکھتا ہے جسے آپ استعمال کررہے ہیں۔

کنسول میموری میں حالیہ فائل کو برقرار رکھتا ہے تاکہ آپ اسے دوبارہ بھیج سکیں
پے لوڈ کے دوران مختلف اجازت والے ٹوکن کی جانچ کرتے ہوئے یا Torii کی میزبانی کرتے ہیں۔ شامل کریں
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` آپ کے ورک فلو سے تیار ہوتا ہے
 NRPC-4 کو اپنانے کے منصوبے (لاگ + JSON خلاصہ) میں حوالہ دیا گیا ہے ، جو اچھی طرح سے مماثل ہے
جائزوں کے دوران کوشش کرنے کی کوشش کرنے کے اسکرین شاٹس پر قبضہ کرنے کے ساتھ۔

### CLI مثال (curl)

وہی فکسچر `curl` کے ذریعے پورٹل کے باہر کھیلا جاسکتا ہے ، جو مدد کرتا ہے
جب آپ پراکسی یا ڈیبگ گیٹ وے کے جوابات کی توثیق کرتے ہیں:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
````transaction_fixtures.manifest.json` میں درج کسی بھی ان پٹ کے ساتھ حقیقت کو تبدیل کریں
یا `cargo xtask norito-rpc-fixtures` کے ساتھ اپنے پے لوڈ کو انکوڈ کریں۔ جب Torii ہے
کینری وضع میں آپ `curl` کو TRY-IT پراکسی کی طرف اشارہ کرسکتے ہیں
(`Norito`) ایک ہی ورزش کرنے کے لئے
انفراسٹرکچر جو پورٹل ویجٹ کے ذریعہ استعمال ہوتا ہے۔

## مشاہدہ اور کاروائیاں

ہر درخواست ایک بار طریقہ ، راستہ ، اصلیت ، upstream کی حیثیت اور ماخذ کے ساتھ رجسٹرڈ ہے
توثیق کا کوڈ (`override` ، `default` یا `client`)۔ ٹوکن کبھی بھی ذخیرہ نہیں ہوتے ہیں: دونوں
بیئرر ہیڈر اور `X-TryIt-Auth` اقدار لاگ سے پہلے لکھے گئے ہیں ،
لہذا آپ لیک کے بارے میں فکر کیے بغیر STDOUT کو مرکزی کلکٹر کو آگے بھیج سکتے ہیں۔

### صحت کی تحقیقات اور انتباہات

تعیناتی کے دوران یا شیڈول پر شامل تحقیقات چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

ماحولیات نوبس:

- `TRYIT_PROXY_SAMPLE_PATH` - اختیاری روٹ Torii (بغیر `/proxy` کے) ورزش کرنے کے لئے۔
- `TRYIT_PROXY_SAMPLE_METHOD` - پہلے سے طے شدہ `GET` ؛ تحریری راستوں کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - نمونہ کال کے لئے ایک عارضی بیئرر ٹوکن انجیکشن کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - 5 s کے پہلے سے طے شدہ ٹائم آؤٹ کو اوور رائڈس۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` کے لئے اختیاری Prometheus ٹیکسٹ منزل۔
- `TRYIT_PROXY_PROBE_LABELS` - جوڑے `key=value` میٹرکس سے منسلک کوما کے ذریعہ الگ الگ (ڈیفالٹ `job=tryit-proxy` اور `instance=<proxy URL>`)۔
- `TRYIT_PROXY_PROBE_METRICS_URL` - اختیاری میٹرکس اینڈپوائنٹ یو آر ایل (جیسے `http://localhost:9798/metrics`) جو `TRYIT_PROXY_METRICS_LISTEN` فعال ہونے پر کامیابی کے ساتھ جواب دینا چاہئے۔

نتائج کو ایک ٹیکسٹ فائل جمع کرنے والے میں کھانا کھلانا ہے جس میں تحقیقات کو ایک تحریر کے راستے کی طرف اشارہ کیا گیا ہے
(مثال کے طور پر `/var/lib/node_exporter/textfile_collector/tryit.prom`) اور لیبل شامل کرنا
اپنی مرضی کے مطابق:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ میٹرک فائل کو جوہری طور پر لکھتا ہے تاکہ اس کا جمع کرنے والا ہمیشہ پڑھتا ہے
ایک مکمل پے لوڈ۔

جب `TRYIT_PROXY_METRICS_LISTEN` تشکیل دیا گیا ہے ، سیٹ کریں
میٹرکس کے اختتامی نقطہ کے لئے `TRYIT_PROXY_PROBE_METRICS_URL` تاکہ تحقیقات میں تیزی سے ناکام ہوجاتا ہے اگر
کھرچنی سطح غائب ہوجاتی ہے (جیسے غلط کنفیگرڈ انجری یا فائر وال کے ضوابط سے محروم)۔
ایک عام پیداوار کی ترتیب اور
`TRYIT_PROXY_PROBE_METRICS_URL="Norito"`۔

ہلکی انتباہ کے ل the ، تحقیقات کو اپنے مانیٹرنگ اسٹیک سے مربوط کریں۔ مثال Prometheus جو
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

اس سے پہلے `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (یا کوئی میزبان/پورٹ جوڑی) سیٹ کریں
Prometheus کی شکل میں میٹرکس کے اختتامی نقطہ کو بے نقاب کرنے کے لئے پراکسی شروع کریں۔ راستہ
پہلے سے طے شدہ `/metrics` ہے لیکن اس کے ذریعے اوور رائٹ کیا جاسکتا ہے
`TRYIT_PROXY_METRICS_PATH=/custom`۔ ہر کھرچنی ہر طریقہ کار کو کل کاؤنٹرز کی واپسی کرتا ہے ،
شرح کی حد سے تجاویز ، upstream غلطیاں/ٹائم آؤٹ ، پراکسی نتائج اور تاخیر کے خلاصے:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

اپنے Prometheus/OTLP جمعکاروں کو میٹرکس کے اختتامی نقطہ کی طرف اشارہ کریں اور پینلز کو دوبارہ استعمال کریں
`dashboards/grafana/docs_portal.json` میں SRE کے لئے دم کی تاخیر کا مشاہدہ کرنے کے لئے موجود ہے
اور نوشتہ جات کو پارس کیے بغیر مسترد کرنا۔ پراکسی خود بخود `tryit_proxy_start_timestamp_ms` شائع کرتا ہے
آپریٹرز کو دوبارہ شروع کرنے میں مدد کرنے کے لئے۔

### رول بیک آٹومیشنTorii ہدف URL کو اپ ڈیٹ کرنے یا بحال کرنے کے لئے مینجمنٹ ہیلپر کا استعمال کریں۔ اسکرپٹ
پچھلی ترتیب کو `.env.tryit-proxy.bak` میں اسٹور کرتا ہے تاکہ رول بیکس ایک ہوں
سنگل کمانڈ

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

`--env` یا `TRYIT_PROXY_ENV` کے ساتھ ENV ​​فائل کے راستے کو اوور رائٹ کریں اگر آپ کی تعیناتی
ترتیب کہیں اور ترتیب دیں۔