---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/try-it.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#یہ ماحول ڈیمو ماحول ہے

ڈویلپر پورٹل ایک اختیاری "کوشش کریں" کنسول فراہم کرتا ہے تاکہ آپ دستاویزات کو چھوڑنے کے بغیر Torii اختتامی نکات کی درخواست کرسکیں۔ کنسول ایمبیڈڈ پراکسی کے ذریعے درخواستوں کو منظور کرتا ہے تاکہ براؤزر CORS کی پابندیوں کو نظرانداز کرسکیں جبکہ اب بھی شرح کی حد اور توثیق کو نافذ کرتے ہیں۔

## شرائط

- نوڈ ڈاٹ جے ایس 18.18 یا اس کے بعد (پورٹل بلڈ کی ضروریات سے میل کھاتا ہے)
- Torii سے اسٹیجنگ ماحول تک نیٹ ورک تک رسائی
بیئرر ٹوکن Torii راستوں پر کال کرسکتا ہے جس کا آپ ٹیسٹ کرنے کا ارادہ رکھتے ہیں

تمام ایجنٹ کی ترتیب ماحولیاتی متغیر کے ذریعہ کی جاتی ہے۔ نیچے دیئے گئے جدول میں انتہائی اہم چابیاں کی فہرست ہے:

| متغیر | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii بنیادی پتہ جس میں ایجنٹ درخواست کرتا ہے | ** مطلوبہ ** |
| `TRYIT_PROXY_LISTEN` | مقامی ترقی سننے کا پتہ (فارمیٹ `host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | پراکسی کو کال کرنے کی اجازت والے ذرائع کی ایک کوما سے الگ کردہ فہرست | `Norito` |
| `TRYIT_PROXY_CLIENT_ID` | شناخت کنندہ کو `X-TryIt-Client` میں ہر ایک کے لئے رکھا جاتا ہے | درخواست `docs-portal` |
| `TRYIT_PROXY_BEARER` | ڈیفالٹ بیئرر ٹوکن کو Torii | پر ری ڈائریکٹ کیا جاتا ہے _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | `X-TryIt-Auth` کے ذریعے صارفین کو اپنا ٹوکن جمع کروانے کی اجازت دیں `0` |
| `TRYIT_PROXY_MAX_BODY` | زیادہ سے زیادہ درخواست جسمانی سائز (بائٹس) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ملی سیکنڈ میں اپ اسٹریم ٹائم آؤٹ | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | درخواستوں کی تعداد فی ریٹ ونڈو فی کلائنٹ IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | شرح کو محدود کرنے والی سلائڈنگ ونڈو (ایم ایس) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | اسٹائل Prometheus میں میٹرکس پوائنٹ کے لئے اختیاری سننے کا پتہ (`host:port` یا `[ipv6]:port`) | _empty (غیر فعال) _ |
| `TRYIT_PROXY_METRICS_PATH` | میٹرکس کے ذریعہ پیش کردہ HTTP راستہ | `/metrics` |

پراکسی `GET /healthz` کو بھی بے نقاب کرتا ہے ، ساختہ JSON غلطیوں کو لوٹاتا ہے ، اور لاگ ان آؤٹ پٹ سے بیئرر ٹوکن کو چھپا دیتا ہے۔

دستاویزی صارفین کو پراکسی کی نمائش کرتے وقت `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کو فعال کریں تاکہ سویگر اور ریپڈوک پینل صارف کے ذریعہ فراہم کردہ بیئرر ٹوکن کو پاس کرسکیں۔ پراکسی اب بھی شرح کی حد کو نافذ کرتا ہے ، حساس اعداد و شمار کو چھپاتا ہے ، اور ریکارڈ کرتا ہے کہ آیا درخواست نے ڈیفالٹ ٹوکن استعمال کیا ہے یا ہر درخواست کو نظرانداز کیا ہے۔ `TRYIT_PROXY_CLIENT_ID` کو ٹیگ کے ساتھ سیٹ کریں جس کو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(پہلے سے طے شدہ `docs-portal`)۔ پراکسی ٹرمز اور کلائنٹ کے ذریعہ فراہم کردہ `X-TryIt-Client` اقدار کی تصدیق اور اس کی تصدیق کرتا ہے اور اس ڈیفالٹ میں واپس آجاتا ہے تاکہ اسٹیجنگ گیٹ وے براؤزر کے اعداد و شمار کو پابند کیے بغیر ماخذ کا آڈٹ کرسکیں۔

## مقامی طور پر ایجنٹ چلائیں

پہلی بار جب آپ پورٹل تیار کرتے ہیں تو انحصار انسٹال کریں:

```bash
cd docs/portal
npm install
```

ایجنٹ کو چلائیں اور اسے اپنی مثال Torii کی طرف اشارہ کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

اسکرپٹ میں بائنڈنگ ایڈریس کو ریکارڈ کیا گیا ہے اور `/proxy/*` سے مخصوص اصل Torii تک درخواستوں کو ری ڈائریکٹ کرتا ہے۔

بندرگاہ کو پابند کرنے سے پہلے ، اسکرپٹ چیک کرتا ہے
`static/openapi/torii.json` میں رجسٹرڈ ڈائجسٹ سے میل کھاتا ہے
`static/openapi/manifest.json`۔ اگر فائلیں اسکیچ ہوجاتی ہیں تو ، کمانڈ کسی غلطی کے ساتھ ختم ہوجاتا ہے اور آپ کو چلانے کے لئے کہتا ہے
`npm run sync-openapi -- --latest`۔ برآمد
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی صورتحال کے لئے ؛ ایجنٹ ایک انتباہ ریکارڈ کرے گا اور آگے بڑھے گا تاکہ آپ کر سکیں
بحالی کی کھڑکیوں کے دوران بازیافت کریں۔

## لنک پورٹل عناصر

پورٹل کی تعمیر یا چلاتے وقت ، یو آر ایل مرتب کریں جو آبجیکٹ کو پراکسی کے لئے استعمال کرنا چاہئے:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

مندرجہ ذیل اجزاء `docusaurus.config.js` سے ان اقدار کو پڑھتے ہیں:- ** سویگر UI ** - `/reference/torii-swagger` پر ڈسپلے ؛ جب ٹوکن موجود ہوتا ہے تو وفد بیئرر اسکیم سے پہلے ہوتا ہے ،
  `X-TryIt-Client` کے ساتھ درخواستوں ، `X-TryIt-Auth` کو انجیکشن لگائیں ، اور کالوں کے ذریعے کال کریں
  ایجنٹ جب `TRYIT_PROXY_PUBLIC_URL` سیٹ ہوتا ہے۔
- ** ریپڈوک ** - `/reference/torii-rapidoc` پر ڈسپلے ؛ ٹوکن فیلڈ کی عکاسی کرتا ہے ،
  یہ ایک ہی ہیڈر کو سویگر کی طرح دوبارہ استعمال کرتا ہے ، اور جب ہیڈر سیٹ ہوتا ہے تو خود بخود پراکسی کو نشانہ بناتا ہے۔
- ** اسے کنسول آزمائیں ** - API جائزہ صفحے میں شامل ؛ اپنی مرضی کے مطابق درخواستیں بھیجنے کی اجازت دیتا ہے ،
  ہیڈر ڈسپلے کریں ، اور جوابی اشیاء کی جانچ کریں۔

دونوں پینل ** اسنیپ شاٹس ** ظاہر کرتے ہیں جو پڑھے جاتے ہیں
`docs/portal/static/openapi/versions.json`۔ اس انڈیکس کو بھریں
`npm run sync-openapi -- --version=<label> --mirror=current --latest` تاکہ جائزہ لینے والے کر سکیں
تاریخی وضاحتوں کے مابین تشریف لے جائیں ، ریکارڈ شدہ SHA-256 ڈائجسٹ دیکھیں ، اور تصدیق کریں کہ آیا یہ اسنیپ شاٹ ہے یا نہیں
یہ ورژن انٹرایکٹو عناصر کو استعمال کرنے سے پہلے واضح مقام کو لوڈ کرتا ہے۔

کسی بھی عنصر میں ٹوکن کو تبدیل کرنا صرف موجودہ براؤزر سیشن کو متاثر کرتا ہے۔ پراکسی فراہم کردہ ٹوکن کو اسٹور یا ریکارڈ نہیں کرتا ہے۔

## OAuth ٹوکن قلیل المدت ہیں

جائزہ لینے والوں کو طویل عرصے تک Torii ٹوکن تقسیم کرنے سے بچنے کے لئے ، کوشش کریں کہ آئی ٹی ماڈیول کو اپنے OAUTH سرور سے لنک کریں۔ جب آپ ہیں
ذیل میں ماحولیاتی متغیرات موجود ہیں ، گیٹ وے ایک ڈیوائس کوڈ لاگ ان انٹرفیس دکھاتا ہے ، اور مختصر بیئرر ٹوکن کو جاری کرتا ہے
عمر اور خود بخود ماڈل کنسول میں انجکشن لگایا جاتا ہے۔

| متغیر | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | ڈیوائس کی اجازت کا نقطہ OAUTH (`/oauth/device/code`) | _empty (غیر فعال) _ |
| `DOCS_OAUTH_TOKEN_URL` | ٹوکن پوائنٹ `grant_type=urn:ietf:params:oauth:grant-type:device_code` کو قبول کرتا ہے _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth کلائنٹ ID دستاویز پیش نظارہ کے لئے رجسٹرڈ | _empty_ |
| `DOCS_OAUTH_SCOPE` | لاگ ان کے دوران کسی جگہ سے الگ ہونے والے ڈومینز کی ضرورت ہوتی ہے `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | بائنڈنگ ٹوکن کے لئے اختیاری سامعین API | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | منظوری کے منتظر (ایم ایس) کے انتظار میں کم سے کم پولنگ کی مدت | `5000` (اقدار <5000 MS مسترد کردیئے گئے ہیں) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | بیک اپ ڈیوائس کوڈ کی درستگی کی مدت (سیکنڈ) | `600` (300 s اور 900 s کے درمیان رکھنا ضروری ہے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | بیک اپ رسائی ٹوکن کی درستگی کی مدت (سیکنڈ) | `900` (300 اور 900s کے درمیان رکھنا ضروری ہے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | مقامی پیش نظاروں کے لئے `1` سیٹ کریں جو جان بوجھ کر OAuth کو نظرانداز کریں _Unset_ |

ابتدا کی مثال:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

جب `npm run start` یا `npm run build` آن کیا جاتا ہے تو ، گیٹ وے میں ان اقدار کو شامل کیا جاتا ہے
`docusaurus.config.js`۔ مقامی پیش نظارہ کے دوران کوشش کریں یہ کارڈ ایک بٹن دکھاتا ہے
"ڈیوائس کوڈ کے ساتھ سائن ان کریں"۔ صارفین OAUTH تصدیقی صفحے پر ظاہر کردہ کوڈ درج کرتے ہیں۔ کامیابی پر
ڈیوائس فلو ٹول مندرجہ ذیل کام کرتا ہے:

- جاری کردہ بیئر ٹوکن کو آزمائشی ماڈیول فیلڈ میں انجیکشن لگانا ،
- موجودہ ہیڈر `X-TryIt-Client` اور `X-TryIt-Auth` کے ساتھ درخواستوں کو نشان زد کرنا ،
- باقی زندگی ڈسپلے کریں ، اور
- ٹوکن خود بخود میعاد ختم ہونے پر حذف ہوجاتا ہے۔

دستی بیئرر انٹری دستیاب ہے۔ جب آپ جائزہ لینے والوں کو عارضی ٹوکن چسپاں کرنے کے لئے مجبور کرنا چاہتے ہیں تو Oauth متغیرات کو چھوڑ دیں
خود ، یا الگ تھلگ مقامی پیش نظارہ کے لئے `DOCS_OAUTH_ALLOW_INSECURE=1` برآمد کریں جہاں گمنام رسائی قابل قبول ہے۔
OAUTH تشکیل کے بغیر تعمیراتی کام اب تیزی سے روڈ میپ میں DOCS-1B کی ضرورت کو پورا کرنے میں ناکام ہوجاتے ہیں۔

نوٹ: دیکھیں [سیکیورٹی ہارڈننگ اور قلم ٹیسٹ چیک لسٹ] (./security-hardening.md)
لیبارٹری کے باہر گیٹ کھولنے سے پہلے ؛ یہ خطرے کے ماڈل ، سی ایس پی/قابل اعتماد اقسام کی فائل ، اور قلم ٹیسٹ کے اقدامات کی دستاویز کرتا ہے
یہ DOCS-1B کو روکتا ہے۔

## مثالوں Norito-RPCNorito-RPC درخواستیں ایک ہی پراکسی اور OAuth پلمبنگ کو JSON راہوں کی طرح شیئر کرتی ہیں۔ یہ کنٹرول ہے
`Content-Type: application/x-norito` اور NRPC تفصیلات میں بیان کردہ پہلے سے خفیہ کردہ پے لوڈ Norito بھیجتا ہے۔
(`docs/source/torii/nrpc_spec.md`)۔ ذخیرہ معیاری پے لوڈ کے تحت فراہم کرتا ہے
`fixtures/norito_rpc/` تاکہ پورٹل مصنفین ، ایس ڈی کے مالکان ، اور جائزہ لینے والے بھی اسی کو دوبارہ جمع کراسکیں
بائٹس سی آئی کے ذریعہ استعمال کیا جاتا ہے۔

### آزمائیں Norito کو آزمائیں IT یونٹ سے

1. حقیقت کو منتخب کریں جیسے `fixtures/norito_rpc/transfer_asset.norito`۔ یہ فائلیں ہیں
   Norito خام کاسنگز ؛ ** نہ کریں ** اسے بیس 64 میں تبدیل کریں۔
2. سویگر یا ریپڈوک میں ، اختتامی نقطہ NRPC کو منتخب کریں (جیسے
   `POST /v2/pipeline/submit`) اور ** مواد کی قسم ** سلیکٹر کو تبدیل کریں
   `application/x-norito`۔
3. درخواست باڈی ایڈیٹر کو ** بائنری ** میں سوئچ کریں (ریپڈوک میں سویگر یا "بائنری/فائل" سلیکٹر میں "فائل" ترتیب دینا)
   اور فائل `.norito` اپ لوڈ کریں۔ ٹول بائٹس کو بغیر کسی ترمیم کے پراکسی کے ذریعے منتقل کرتا ہے۔
4. درخواست جمع کروائیں۔ اگر Torii `X-Iroha-Error-Code: schema_mismatch` واپس کرتا ہے تو ، چیک کریں کہ آپ کال کر رہے ہیں
   اختتامی نقطہ بائنری پے لوڈ کو قبول کرتا ہے اور اس بات کو یقینی بناتا ہے کہ اسکیما ہیش میں ریکارڈ کیا گیا
   `fixtures/norito_rpc/schema_hashes.json` آپ جس بلڈ Torii کو استعمال کررہے ہیں اس سے میل کھاتا ہے۔

کنٹرولر میموری میں آخری فائل کو محفوظ کرتا ہے تاکہ وہ مختلف ٹیسٹ ٹوکن کے ساتھ ایک ہی پے لوڈ کو دوبارہ سینڈ کرسکے
یا میرا میزبان Torii۔ اپنے ورک فلو میں `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` شامل کریں
بنڈل NRPC-4 سرٹیفیکیشن اسکیم (لاگ + JSON سمری) میں حوالہ کردہ ڈائریکٹریز تیار کرتا ہے ، جو اس کے ساتھ اچھی طرح سے مربوط ہوتا ہے
جائزوں کے دوران کوشش کرنے والے جواب کے اسکرین شاٹس پر قبضہ کریں۔

### CLI مثال (curl)

اسی فکسچر کو `curl` کے توسط سے گیٹ سے دوبارہ شروع کیا جاسکتا ہے ، جو ڈیلر سے جانچ پڑتال کرتے وقت مفید ہے یا
گیٹ وے کے صحیح جوابات:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` میں درج کسی بھی اندراج کے ساتھ حقیقت کو تبدیل کریں یا اپنے پے لوڈ کو انکوڈ کریں
`cargo xtask norito-rpc-fixtures`۔ جب Torii کینری موڈ میں ہے تو آپ `curl` کو ہدایت کرسکتے ہیں
ایک ہی تعمیر کو جانچنے کے لئے پراکسی ٹر-یہ (`Norito`)
پورٹل عناصر کے ذریعہ استعمال کیا جاتا ہے۔

## مانیٹرنگ اور آپریشنز

ہر درخواست میں ایک بار طریقہ ، راستہ ، اصلیت ، upstream کی حیثیت ، اور توثیق کے ذریعہ لاگ ان ہوتا ہے
(`override` یا `default` یا `client`)۔ ٹوکن کبھی ذخیرہ نہیں ہوتے ہیں۔ بیئرر ہیڈر پر نظر ثانی کی جاتی ہے
لاگنگ سے پہلے `X-TryIt-Auth` قدریں ، لہذا آپ STDOUT کو کسی مرکزی مرتب کو لیک ہونے کی فکر کیے بغیر کسی مرکزی مرتب کو منتقل کرسکتے ہیں۔

### صحت کی جانچ پڑتال اور انتباہات

تعیناتی کے دوران یا کسی شیڈول پر منسلک تحقیقات چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

ماحولیات کی چابیاں:

- `TRYIT_PROXY_SAMPLE_PATH` - اختیاری Torii راستہ (NO `/proxy`) جانچنے کے لئے۔
- `TRYIT_PROXY_SAMPLE_METHOD` - پہلے سے طے شدہ `GET` ؛ تحریری کارروائیوں کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - آزمائشی آرڈر کے لئے ایک عارضی بیئرر ٹوکن انجیکشن کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - 5 s کے پہلے سے طے شدہ ٹائم آؤٹ کی جگہ لے لیتا ہے۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` کے لئے اختیاری Prometheus ٹیکسٹ منزل۔
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` جوڑے کوما کے ذریعہ الگ الگ ترازو میں شامل کیا جاتا ہے (پہلے سے طے شدہ `job=tryit-proxy` اور `instance=<proxy URL>`)۔
- `TRYIT_PROXY_PROBE_METRICS_URL` - میٹرکس پوائنٹ (جیسے `http://localhost:9798/metrics`) کے لئے ایک اختیاری پتہ جو `TRYIT_PROXY_METRICS_LISTEN` کو فعال ہونے پر جواب دینا چاہئے۔

نتائج کو تحریری راستے پر تحقیقات کی ہدایت کرکے ٹیکسٹ فائل کلیکٹر میں جمع کریں
(جیسے `/var/lib/node_exporter/textfile_collector/tryit.prom`) اور کسٹم ٹیگز شامل کرنا:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ میٹرکس فائل کو ایٹم سے دوبارہ لکھتا ہے تاکہ مرتب کرنے والا ہمیشہ پورے پے لوڈ کو پڑھتا ہے۔جب `TRYIT_PROXY_METRICS_LISTEN` سیٹ کیا جاتا ہے ، سیٹ کریں
`TRYIT_PROXY_PROBE_METRICS_URL` میٹرکس پوائنٹ پر
(جیسے انجیریس سیٹ یا فائر وال کے قواعد سے محروم نہیں)۔ ایک عام پیداوار کی ترتیب ہے
`TRYIT_PROXY_PROBE_METRICS_URL="Norito"`۔

ہلکے الارم کے ل the ، تحقیقات کو اپنے مانیٹرنگ انٹرفیس سے مربوط کریں۔ مثال Prometheus مسلسل دو ناکامیوں کے بعد بڑھتی ہے:

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

### اسکور میٹرکس اور ڈیش بورڈز

ایجنٹ چلانے سے پہلے `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (یا کوئی میزبان/پورٹ جوڑی) سیٹ کریں
Prometheus کی شکل میں میٹرکس پوائنٹ کو ظاہر کرنے کے لئے۔ پہلے سے طے شدہ راستہ `/metrics` ہے اور اس کے ذریعے تبدیل کیا جاسکتا ہے
`TRYIT_PROXY_METRICS_PATH=/custom`۔ ہر سکریپ طریقہ کے ذریعہ کل درخواستوں کے لئے کاؤنٹرز لوٹاتا ہے اور اسے مسترد کردیا جاتا ہے
شرح کی حد ، upstream غلطیاں/ٹائم آؤٹ ، پراکسی نتائج ، اور تاخیر کے خلاصے:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Prometheus/OTLP جمع کرنے والوں کو میٹرکس پوائنٹ پر اشارہ کریں اور بورڈز کو دوبارہ استعمال کریں۔
`dashboards/grafana/docs_portal.json` تاکہ SRE دم کی تاخیر اور مسترد ہونے والی اونچائیوں کی نگرانی کرسکے
نوشتہ جات کا تجزیہ کیے بغیر۔ آپریٹرز کی مدد کے لئے ایجنٹ خود بخود `tryit_proxy_start_timestamp_ms` تعینات کرتا ہے
پتہ لگانے کے ریبوٹ پر۔

### آٹومیٹ کو کالعدم کریں

ہدف ایڈریس Torii کو اپ ڈیٹ کرنے یا بحال کرنے کے لئے انتظامیہ کے آلے کا استعمال کریں۔ اسکرپٹ میں پچھلی ترتیب کو بچاتا ہے
`.env.tryit-proxy.bak` لہذا کالعدم کرنا ایک چیز ہے۔

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

اگر آپ کی تعیناتی کسی اور جگہ میں ترتیبات کو محفوظ کرتی ہے تو آپ `--env` یا `TRYIT_PROXY_ENV` کے ذریعے ENV فائل کے راستے کو اوور رائڈ کرسکتے ہیں۔