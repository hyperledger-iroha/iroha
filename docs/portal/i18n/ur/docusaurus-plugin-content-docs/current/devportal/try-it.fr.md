---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/try-it.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# اسے سینڈ باکس آزمائیں

ڈویلپر پورٹل ایک اختیاری "کوشش کریں" کنسول فراہم کرتا ہے جس میں دستاویزات کو چھوڑنے کے بغیر Torii اختتامی نقطہ پر کال کریں۔ کنسول ریلیڈ پراکسی کے ذریعے براؤزرز کے لئے درخواستوں کو ریلیز کرتا ہے تاکہ شرح کو محدود کرنے اور توثیق کو نافذ کرتے ہوئے CORS کی حدود کو نظرانداز کیا جاسکے۔

## شرائط

- نوڈ ڈاٹ جے ایس 18.18 یا نیا (پورٹل بلڈ کی ضروریات سے میل کھاتا ہے)
- اسٹیجنگ ماحول Torii تک نیٹ ورک تک رسائی
- ایک بیئر ٹوکن جس میں آپ کی جانچ کرنا چاہتے ہیں Torii راستوں کو کال کرنے کی صلاحیت رکھتا ہے

تمام پراکسی ترتیب ماحول کے متغیرات سے گزرتی ہے۔ نیچے دیئے گئے جدول میں سب سے اہم نوبس کی فہرست ہے:

| مختلف ہوتا ہے | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | بیس URL Torii جس پر پراکسی ریلے کی درخواستیں | ** مطلوبہ ** |
| `TRYIT_PROXY_LISTEN` | مقامی ترقی کے لئے سننے کا پتہ (فارمیٹ `host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | پراکسی کو کال کرنے کی اجازت کی اصل کی کوما سے الگ فہرست | `Norito` |
| `TRYIT_PROXY_CLIENT_ID` | شناخت کرنے والا ہر اپ اسٹریم درخواست کے لئے `X-TryIt-Client` میں رکھا گیا ہے `docs-portal` |
| `TRYIT_PROXY_BEARER` | Torii | پر ڈیفالٹ ریلے کے ذریعہ بیئرر ٹوکن _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | اختتامی صارفین کو `X-TryIt-Auth` کے ذریعے اپنا ٹوکن فراہم کرنے کی اجازت دیتا ہے `0` |
| `TRYIT_PROXY_MAX_BODY` | درخواست جسم کا زیادہ سے زیادہ سائز (بائٹس) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | ملی سیکنڈ میں اپ اسٹریم ٹائم آؤٹ | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | درخواستوں کو فی ریٹ ونڈو فی کلائنٹ IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | شرح کو محدود کرنے (ایم ایس) کے لئے سلائڈنگ ونڈو | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | میٹرکس اینڈپوائنٹ اسٹائل Prometheus (`host:port` یا `[ipv6]:port`) کے لئے اختیاری سننے کا پتہ | _empty (غیر فعال) _ |
| `TRYIT_PROXY_METRICS_PATH` | میٹرکس اختتامی نقطہ کے ذریعہ HTTP راستہ پیش کیا گیا `/metrics` |

پراکسی `GET /healthz` کو بھی بے نقاب کرتا ہے ، ساختہ JSON غلطیوں کو لوٹاتا ہے ، اور بیئرر ٹوکن کو لاگوں میں چھپا دیتا ہے۔

جب ڈی او سی صارفین کو پراکسی کو بے نقاب کرتے ہو تو `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کو فعال کریں تاکہ سویگر اور ریپڈوک پینل صارف کے فراہم کردہ بیئرر ٹوکن کو ریلے کرسکیں۔ پراکسی ہمیشہ شرح کی حد کو نافذ کرتا ہے ، اسناد کو چھپاتا ہے ، اور ریکارڈ کرتا ہے کہ آیا کسی درخواست میں پہلے سے طے شدہ ٹوکن یا فی درخواست اوور ہیڈ استعمال ہوتا ہے۔ `TRYIT_PROXY_CLIENT_ID` کو اس لیبل کے ساتھ تشکیل دیں جس کو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(پہلے سے طے شدہ `docs-portal`)۔ پراکسی کالر کے ذریعہ فراہم کردہ `X-TryIt-Client` اقدار کو چھڑاتی ہے اور اس کی توثیق کرتی ہے ، پھر اس ڈیفالٹ پر واپس آجائے گی تاکہ اسٹیجنگ گیٹ وے براؤزر میٹا ڈیٹا سے وابستہ کیے بغیر آڈٹ پرووننس کرسکیں۔

## مقامی طور پر پراکسی شروع کریں

پہلے پورٹل کی تشکیل کرتے وقت انحصار انسٹال کریں:

```bash
cd docs/portal
npm install
```

پراکسی لانچ کریں اور اسے اپنے Torii مثال کی طرف اشارہ کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```اسکرپٹ پابند ایڈریس کو لاگ ان کرتا ہے اور `/proxy/*` سے تشکیل شدہ اصل Torii تک درخواستوں کو لاگ ان کرتا ہے۔

ساکٹ کو پابند کرنے سے پہلے اسکرپٹ اس کی توثیق کرتا ہے
`static/openapi/torii.json` میں ریکارڈ شدہ ڈائجسٹ کے مساوی ہے
`static/openapi/manifest.json`۔ اگر فائلیں ہٹ جاتی ہیں تو ، کمانڈ A کے ساتھ ناکام ہوجاتا ہے
غلطی اور آپ سے `npm run sync-openapi -- --latest` پر عمل درآمد کرنے کو کہتی ہے۔ برآمد
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی حدود کے لئے ؛ لاگ پراکسی
ایک انتباہ اور دیکھ بھال کی کھڑکیوں کے دوران آپ کو صحت یاب ہونے کی اجازت دیتا ہے۔

## پورٹل ویجٹ کی کیبلنگ

جب آپ ڈویلپر پورٹل کی تعمیر یا خدمت کرتے ہیں تو ، یو آر ایل مرتب کریں جو ویجٹ کو ہونا چاہئے
پراکسی کے لئے استعمال کریں:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

مندرجہ ذیل اجزاء `docusaurus.config.js` سے ان اقدار کو پڑھتے ہیں:

- ** سویگر UI ** - `/reference/torii-swagger` پر پیش کیا گیا ؛ اسکیما بیئرر کو پہلے سے مجاز بناتا ہے
  جب ایک ٹوکن موجود ہوتا ہے تو ، `X-TryIt-Client` کے ساتھ ٹیگس کی درخواست کرتا ہے ، انجیکشن
  `X-TryIt-Auth` ، اور پراکسی کے ذریعے کالوں کو دوبارہ لکھتا ہے
  `TRYIT_PROXY_PUBLIC_URL` کی وضاحت کی گئی ہے۔
- ** ریپڈوک ** - `/reference/torii-rapidoc` پر واپس آگیا ؛ ٹوکن فیلڈ کی عکاسی کرتا ہے ،
  سویگر پینل کی طرح ہیڈرز کو دوبارہ استعمال کریں ، اور خود بخود اہداف
  پراکسی جب یو آر ایل تشکیل دیا جاتا ہے۔
- ** اسے کنسول کی کوشش کریں ** - API جائزہ صفحے میں مربوط ؛ آپ کو بھیجنے کی اجازت دیتا ہے
  اپنی مرضی کے مطابق درخواستیں ، ہیڈر دیکھیں ، اور جوابی اداروں کا معائنہ کریں۔

دونوں پینل ایک ** اسنیپ شاٹ سلیکٹر ** ظاہر کرتے ہیں جو پڑھتا ہے
`docs/portal/static/openapi/versions.json`۔ اس انڈیکس کے ساتھ مکمل کریں
`npm run sync-openapi -- --version=<label> --mirror=current --latest` تاکہ جائزہ لینے والے
تاریخی چشمی کے مابین سوئچ کرسکتے ہیں ، ریکارڈ شدہ SHA-256 ڈائجسٹ کو دیکھ سکتے ہیں اور تصدیق کرسکتے ہیں
اگر ریلیز اسنیپ شاٹ انٹرایکٹو ویجٹ استعمال کرنے سے پہلے کسی مینی فیسٹ علامت کو سرایت کرتا ہے۔

ایک ویجیٹ میں ٹوکن کو تبدیل کرنے سے موجودہ براؤزر سیشن کو متاثر ہوتا ہے۔ پراکسی نہیں کرتا ہے
کبھی بھی برقرار نہیں رہتا ہے اور نہ ہی فراہم کردہ ٹوکن کو لاگ ان کرتا ہے۔

## مختصر زندگی کے OAUTH ٹوکن

جائزہ لینے والوں کو طویل عرصے تک Torii ٹوکن تقسیم کرنے سے بچنے کے لئے ، کوشش کریں کہ اس کو کنسول کی کوشش کریں
آپ کا OAUTH سرور۔ جب ذیل میں ماحولیاتی متغیرات موجود ہوں تو ، پورٹل
لاگ ان ڈیوائس کوڈ ویجیٹ کو پیش کرتا ہے ، قلیل مدتی بیئرر ٹوکن کا اخراج کرتا ہے ، اور ان کو انجیکشن دیتا ہے
خود بخود کنسول کی شکل میں۔| مختلف ہوتا ہے | مقصد | ڈیفالٹ |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | اجازت کے اختتام پر آلہ OAUTH (`/oauth/device/code`) | _empty (غیر فعال) _ |
| `DOCS_OAUTH_TOKEN_URL` | اختتامی نقطہ ٹوکن جو `grant_type=urn:ietf:params:oauth:grant-type:device_code` کو قبول کرتا ہے _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | پیش نظارہ دستاویزات کے لئے OAuth کلائنٹ ID محفوظ کیا گیا | _empty_ |
| `DOCS_OAUTH_SCOPE` | لاگ ان کے دوران درخواست کردہ خالی جگہوں کے ذریعہ حدود کے ذریعہ اسکوپس `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | ٹوکن کو باندھنے کے لئے اختیاری API سامعین | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | منظوری کے انتظار میں کم سے کم پولنگ کا وقفہ (ایم ایس) | `5000` (اقدار <5000 MS مسترد) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | ڈیوائس کوڈ میعاد ختم ہونے والی ونڈو (سیکنڈ) | `600` (300 s اور 900 s کے درمیان رہنا چاہئے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | رسائی ٹوکن لائف ٹائم (سیکنڈ) | `900` (300 s اور 900 s کے درمیان رہنا چاہئے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | مقامی پیش نظاروں کے لئے `1` سیٹ کریں جو جان بوجھ کر OAUTH انفورسمنٹ کو چھوڑ دیں _Unset_ |

مثال کی تشکیل:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

جب آپ `npm run start` یا `npm run build` لانچ کرتے ہیں تو ، پورٹل ان اقدار کو مربوط کرتا ہے
`docusaurus.config.js` میں۔ مقامی پیش نظارہ میں کوشش کریں یہ کارڈ ایک بٹن دکھاتا ہے
"ڈیوائس کوڈ کے ساتھ سائن ان کریں"۔ صارفین آپ کے OAUTH صفحے پر ظاہر کردہ کوڈ درج کرتے ہیں۔ ایک بار جب آلہ کا بہاؤ ویجیٹ سے گزر جاتا ہے:

- کوشش کرنے والے کو کنسول فیلڈ میں جاری کردہ بیئرر ٹوکن کو انجیکشن لگاتا ہے ،
- موجودہ ہیڈر کے ساتھ ٹیگ کی درخواستیں `X-TryIt-Client` اور `X-TryIt-Auth` ،
- باقی وقت دکھاتا ہے ، اور
- جب اس کی میعاد ختم ہوجاتی ہے تو خود بخود ٹوکن صاف کردیتی ہے۔

دستی بیئرر انٹری دستیاب ہے۔ جب آپ چاہیں تو oauth متغیر کو چھوڑ دیں
جائزہ لینے والوں کو خود کو عارضی ٹوکن چسپاں کرنے ، یا برآمد کرنے پر مجبور کریں
`DOCS_OAUTH_ALLOW_INSECURE=1` الگ تھلگ مقامی پیش نظارہ کے لئے جہاں گمنام رسائی ہے
قابل قبول OAUTH کے بغیر تعمیرات اب مکمل طور پر مطمئن ہونے میں ناکام ہوجاتے ہیں
دستاویزات -1B روڈ میپ کے گیٹ کو پورا کرنے کے لئے۔

نوٹ: [سیکیورٹی ہارڈنگ اور قلم ٹیسٹ چیک لسٹ] (./security-hardening.md) سے مشورہ کریں
لیب کے باہر پورٹل کو بے نقاب کرنے سے پہلے ؛ یہ خطرے کے ماڈل کی دستاویز کرتا ہے ،
سی ایس پی/قابل اعتماد اقسام کا پروفائل ، اور قلمی ٹیسٹ کے اقدامات جو اب دستاویزات 1 بی کو روکتے ہیں۔

## مثالوں Norito-RPC

Norito-RPC درخواستیں ایک ہی پراکسی اور OAUTH پلمبنگ کو JSON روٹس کی طرح شیئر کرتی ہیں۔
وہ آسانی سے `Content-Type: application/x-norito` مرتب کرتے ہیں اور پے لوڈ Norito بھیجیں
پری انکوڈ این آر پی سی کی تفصیلات میں بیان کیا گیا ہے
(`docs/source/torii/nrpc_spec.md`)۔
ذخیرہ `fixtures/norito_rpc/` کے تحت کیننیکل پے لوڈ فراہم کرتا ہے تاکہ مصنفین کے مصنفین
پورٹل ، ایس ڈی کے مالکان ، اور جائزہ لینے والے سی آئی کے ذریعہ استعمال ہونے والے عین مطابق بائٹس کو دوبارہ چلا سکتے ہیں۔

### ایک پے لوڈ Norito کو آزمائیں کنسول سے1. `fixtures/norito_rpc/transfer_asset.norito` کی طرح ایک حقیقت کا انتخاب کریں۔ یہ
   فائلیں کچی Norito لفافے ہیں۔ ** نہ کریں ** بیس 64 انک کوڈ کریں۔
2. سویگر یا ریپڈوک میں ، NRPC اختتامی نقطہ تلاش کریں (جیسے۔
   `POST /v2/pipeline/submit`) اور ** مواد کی قسم ** سلیکٹر کو تبدیل کریں
   `application/x-norito`۔
3. باڈی ایڈیٹر کو ** بائنری ** ("فائل" موڈ آف سویگر یا پر سوئچ کریں
   "بائنری/فائل" ریپڈوک کا سلیکٹر) اور فائل `.norito` کو لوڈ کریں۔ ویجیٹ
   بغیر کسی تبدیلی کے پراکسی کے ذریعے بائٹس کو منتقل کرتا ہے۔
4. درخواست جمع کروائیں۔ اگر Torii `X-Prometheus-Error-Code: schema_mismatch` واپس کرتا ہے ،
   تصدیق کریں کہ آپ ایک اختتامی نقطہ پر کال کر رہے ہیں جو بائنری پے لوڈ کو قبول کرتا ہے اور تصدیق کرتا ہے
   کہ ہیش اسکیما `fixtures/norito_rpc/schema_hashes.json` میں ریکارڈ کرتا ہے
   Torii کو ہدف سے مطابقت رکھتا ہے۔

کنسول میموری میں حالیہ فائل کو برقرار رکھتا ہے تاکہ آپ اسے دوبارہ بھیج سکیں
مختلف اجازت والے ٹوکن کی جانچ کرتے وقت ایک ہی پے لوڈ یا Torii کی میزبانی کرتے ہیں۔ شامل کریں
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` آپ کے ورک فلو میں بنڈل تیار کرتا ہے
NRPC-4 کو اپنانے کے منصوبے (لاگ + JSON سمری) میں شواہد کا حوالہ ، جو ٹھیک ہے
جائزوں کے دوران کوشش کرنے کی کوشش کے اسکرین شاٹ کے ساتھ۔

### CLI مثال (curl)

اسی فکسچر کو `curl` کے ذریعے پورٹل کے باہر دوبارہ چلایا جاسکتا ہے ، جو مفید ہے
جب آپ پراکسی یا ڈیبگ گیٹ وے کے جوابات کی توثیق کرتے ہیں:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` میں درج کسی بھی اندراج کے ساتھ حقیقت کو تبدیل کریں
یا `cargo xtask norito-rpc-fixtures` کے ساتھ اپنے پے لوڈ کو انکوڈ کریں۔ جب Torii اندر ہے
کینری وضع آپ پراکسی کی کوشش کرنے کے لئے `curl` کی نشاندہی کرسکتے ہیں
(`Norito`) اسی بنیادی ڈھانچے کو ورزش کرنے کے لئے
کہ پورٹل ویجٹ استعمال کرتے ہیں۔

## مشاہدہ اور کاروائیاں

ہر درخواست میں ایک بار طریقہ ، راستہ ، اصلیت ، upstream کی حیثیت اور ماخذ کے ساتھ لاگ ان کیا جاتا ہے
توثیق (`override` ، `default` یا `client`)۔ ٹوکن کبھی بھی ذخیرہ نہیں ہوتے ہیں:
ہیڈر بیئرر اور `X-TryIt-Auth` اقدار لاگ ان کرنے سے پہلے دوبارہ تیار کی گئی ہیں ، لہذا آپ کر سکتے ہیں
بغیر کسی لیک کے خوف کے مرکزی کلکٹر کو ریلے stdout۔

### صحت کی تحقیقات اور انتباہات

تعیناتی کے دوران یا کسی شیڈول پر شامل تحقیقات چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

ماحولیات نوبس:

- `TRYIT_PROXY_SAMPLE_PATH` - اختیاری روٹ Torii (بغیر `/proxy` کے) استعمال کیا جائے۔
- `TRYIT_PROXY_SAMPLE_METHOD` - پہلے سے طے شدہ `GET` ؛ تحریری راستوں کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - مثال کے طور پر کال کے لئے ایک عارضی بیئرر ٹوکن انجیکشن کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - 5 s کے پہلے سے طے شدہ ٹائم آؤٹ کو اوور رائٹ کرتا ہے۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - اختیاری متن منزل Prometheus برائے `probe_success`/`probe_duration_seconds`۔
- `TRYIT_PROXY_PROBE_LABELS` - جوڑے `key=value` کوما کے ذریعہ الگ الگ میٹرکس میں شامل کیا گیا (بطور ڈیفالٹ `job=tryit-proxy` اور `instance=<proxy URL>`)۔
- `TRYIT_PROXY_PROBE_METRICS_URL` - اختیاری میٹرکس اینڈپوائنٹ یو آر ایل (مثال کے طور پر ، `http://localhost:9798/metrics`) جو `TRYIT_PROXY_METRICS_LISTEN` فعال ہونے پر کامیابی کے ساتھ جواب دینا چاہئے۔نتائج کو ٹیکسٹ فائل جمع کرنے والے میں ایک تحریر کے راستے کی طرف اشارہ کرکے انجیکشن لگائیں
(مثال کے طور پر ، `/var/lib/node_exporter/textfile_collector/tryit.prom`) اور لیبل شامل کرنا
مشخص:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ میٹرک فائل کو جوہری طور پر دوبارہ لکھتا ہے تاکہ آپ کا جمع کرنے والا پڑھے
ہمیشہ ایک مکمل پے لوڈ۔

جب `TRYIT_PROXY_METRICS_LISTEN` تشکیل دیا گیا ہے ، سیٹ کریں
`TRYIT_PROXY_PROBE_METRICS_URL` میٹرکس کے اختتامی نقطہ پر تاکہ تحقیقات جلد ناکام ہوجاتی ہیں اگر
کھرچنی سطح غائب ہوجاتی ہے (مثال کے طور پر ناقص تشکیل شدہ انجری یا فائر وال کے ضوابط سے محروم)۔
ایک عام پیداوار کی ترتیب ہے
`TRYIT_PROXY_PROBE_METRICS_URL="Norito"`۔

روشنی کے انتباہات کے ل the ، تحقیقات کو اپنے مانیٹرنگ اسٹیک سے مربوط کریں۔ مثال Prometheus
مسلسل دو ناکامیوں کے بعد کون سا صفحہ:

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

اس سے پہلے `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (یا کوئی میزبان/پورٹ جوڑی) کی وضاحت کریں
Prometheus کی شکل میں میٹرکس کے اختتامی نقطہ کو بے نقاب کرنے کے لئے پراکسی لانچ کریں۔ راستہ
پہلے سے طے شدہ `/metrics` ہے لیکن اس کی جگہ `TRYIT_PROXY_METRICS_PATH=/custom` کی جگہ دی جاسکتی ہے۔ ہر ایک
کھرچنی فی طریقہ کار کے کل کاؤنٹرز ، شرح کی حد سے تجاویز ، غلطیاں/ٹائم آؤٹ واپس کرتا ہے
اپ اسٹریم ، پراکسی نتائج اور تاخیر کے خلاصے:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

اپنے Prometheus/OTLP جمعکاروں کو میٹرکس کے اختتامی نقطہ کی طرف اشارہ کریں اور پینلز کو دوبارہ استعمال کریں
`dashboards/grafana/docs_portal.json` میں موجود ہے تاکہ SRE دم کی تاخیر کا مشاہدہ کرے
اور نوشتہ جات کی تجزیہ کیے بغیر مسترد چوٹیوں۔ پراکسی خود بخود شائع کرتا ہے
`tryit_proxy_start_timestamp_ms` آپریٹرز کو دوبارہ شروع کرنے میں مدد کرنے کے لئے۔

### رول بیک آٹومیشن

ہدف URL Torii کو اپ ڈیٹ کرنے یا بحال کرنے کے لئے مینجمنٹ ہیلپر کا استعمال کریں۔ اسکرپٹ
`.env.tryit-proxy.bak` میں پچھلی ترتیب کو اسٹور کرتا ہے تاکہ رول بیکس ہوں
صرف ایک آرڈر۔

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

`--env` یا `TRYIT_PROXY_ENV` کے ساتھ env فائل کے راستے کو اوور رائڈ کریں اگر آپ کی تعیناتی ہے
ترتیب کہیں اور ترتیب دیتا ہے۔