---
lang: ur
direction: rtl
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It کنسول
description: ڈویلپر پورٹل کے پراکسی اور Swagger و RapiDoc ویجیٹس استعمال کریں تاکہ دستاویزات کی سائٹ سے براہ راست حقیقی Torii / Norito-RPC درخواستیں بھیجی جا سکیں۔
---

پورٹل تین انٹرایکٹو سطحیں فراہم کرتا ہے جو Torii کی طرف ٹریفک ریلے کرتی ہیں:

- **Swagger UI** `/reference/torii-swagger` پر دستخط شدہ OpenAPI اسپیسیفکیشن دکھاتا ہے اور جب `TRYIT_PROXY_PUBLIC_URL` سیٹ ہو تو درخواستوں کو خودکار طور پر پراکسی کے ذریعے دوبارہ لکھتا ہے۔
- **RapiDoc** `/reference/torii-rapidoc` پر اسی اسکیما کو فائل اپ لوڈ اور کنٹینٹ ٹائپ سلیکٹرز کے ساتھ ظاہر کرتا ہے جو `application/x-norito` کے لئے موزوں ہیں۔
- **Try it sandbox** Norito کے جائزہ صفحے پر ad hoc REST درخواستوں اور OAuth ڈیوائس لاگ اِن کے لئے ایک ہلکا فارم فراہم کرتا ہے۔

تینوں ویجیٹس مقامی **Try-It پراکسی** (`docs/portal/scripts/tryit-proxy.mjs`) کو درخواستیں بھیجتے ہیں۔ پراکسی اس بات کی تصدیق کرتا ہے کہ `static/openapi/torii.json`، `static/openapi/manifest.json` میں موجود دستخط شدہ ڈائجسٹ سے میل کھاتا ہے، ریٹ لِمٹر نافذ کرتا ہے، لاگز میں `X-TryIt-Auth` ہیڈرز کو ریڈیکٹ کرتا ہے، اور ہر upstream کال پر `X-TryIt-Client` ٹیگ لگاتا ہے تاکہ Torii آپریٹرز ٹریفک کے ذرائع کا آڈٹ کر سکیں۔

## پراکسی چلائیں

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` Torii کا بیس URL ہے جسے آپ ٹیسٹ کرنا چاہتے ہیں۔
- `TRYIT_PROXY_ALLOWED_ORIGINS` میں پورٹل کے ہر اوریجن (لوکل ڈیولپمنٹ سرور، پروڈکشن ہوسٹ نیم، پری ویو URL) کو شامل ہونا چاہیے جو کنسول کو ایمبیڈ کرے۔
- `TRYIT_PROXY_PUBLIC_URL` کو `docusaurus.config.js` استعمال کرتا ہے اور `customFields.tryIt` کے ذریعے ویجیٹس میں انجیکٹ کیا جاتا ہے۔
- `TRYIT_PROXY_BEARER` صرف اسی وقت لوڈ ہوتا ہے جب `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ہو؛ ورنہ صارفین کو کنسول یا OAuth ڈیوائس فلو کے ذریعے اپنا ٹوکن فراہم کرنا ہوگا۔
- `TRYIT_PROXY_CLIENT_ID` ہر درخواست کے ساتھ جانے والا `X-TryIt-Client` ٹیگ مقرر کرتا ہے۔
  براؤزر سے `X-TryIt-Client` بھیجنا اجازت یافتہ ہے مگر ویلیوز ٹرم کی جاتی ہیں
  اور اگر ان میں کنٹرول کریکٹرز ہوں تو مسترد کر دی جاتی ہیں۔

اسٹارٹ اپ پر پراکسی `verifySpecDigest` چلاتا ہے اور اگر مینی فیسٹ پرانا ہو تو اصلاحی ہنٹ کے ساتھ نکل جاتا ہے۔ تازہ ترین Torii اسپیسیفکیشن کے لئے `npm run sync-openapi -- --latest` چلائیں یا ہنگامی اوور رائیڈ کے لئے `TRYIT_PROXY_ALLOW_STALE_SPEC=1` پاس کریں۔

پراکسی ٹارگٹ کو ہاتھ سے ماحول فائلیں ایڈٹ کئے بغیر اپ ڈیٹ یا رول بیک کرنے کے لئے یہ ہیلپر استعمال کریں:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## ویجیٹس جوڑیں

جب پراکسی سننے لگے تو پورٹل چلائیں:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` درج ذیل سیٹنگز فراہم کرتا ہے:

| متغیر | مقصد |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | Swagger، RapiDoc اور Try it sandbox میں انجیکٹ کیا گیا URL۔ غیر مجاز پری ویوز کے دوران ویجیٹس چھپانے کے لئے اسے خالی چھوڑ دیں۔ |
| `TRYIT_PROXY_DEFAULT_BEARER` | میموری میں رکھا گیا اختیاری ڈیفالٹ ٹوکن۔ اس کے لئے `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` اور HTTPS-only CSP guard (DOCS-1b) درکار ہے، الا یہ کہ آپ لوکل طور پر `DOCS_SECURITY_ALLOW_INSECURE=1` دیں۔ |
| `DOCS_OAUTH_*` | OAuth ڈیوائس فلو (`OAuthDeviceLogin` component) کو فعال کرتا ہے تاکہ ریویورز پورٹل چھوڑے بغیر مختصر مدت کے ٹوکن جاری کر سکیں۔ |

جب OAuth ویری ایبلز موجود ہوں تو sandbox **Sign in with device code** بٹن دکھاتا ہے جو کنفیگر شدہ Auth سرور کے ذریعے چلتا ہے (درست شکل کے لئے `config/security-helpers.js` دیکھیں)۔ ڈیوائس فلو سے جاری ہونے والے ٹوکن صرف براؤزر سیشن میں کیش ہوتے ہیں۔

## Norito-RPC پے لوڈ بھیجنا

1. CLI یا [Norito quickstart](./quickstart.md) میں بتائے گئے اسنیپٹس کے ساتھ `.norito` پے لوڈ بنائیں۔ پراکسی `application/x-norito` باڈیز کو بغیر بدلے فارورڈ کرتا ہے، اس لئے آپ وہی آرٹیفیکٹ دوبارہ استعمال کر سکتے ہیں جو `curl` کے ساتھ بھیجتے۔
2. `/reference/torii-rapidoc` (بائنری پے لوڈز کے لئے ترجیحی) یا `/reference/torii-swagger` کھولیں۔
3. ڈراپ ڈاؤن سے مطلوبہ Torii اسنیپ شاٹ منتخب کریں۔ اسنیپ شاٹس دستخط شدہ ہوتے ہیں؛ پینل `static/openapi/manifest.json` میں ریکارڈ شدہ مینی فیسٹ ڈائجسٹ دکھاتا ہے۔
4. "Try it" ڈراور میں `application/x-norito` کنٹینٹ ٹائپ منتخب کریں، **Choose File** پر کلک کریں، اور اپنا پے لوڈ منتخب کریں۔ پراکسی درخواست کو `/proxy/v1/pipeline/submit` پر ری رائٹ کرتا ہے اور اسے `X-TryIt-Client=docs-portal-rapidoc` کے ساتھ ٹیگ کرتا ہے۔
5. Norito ریسپانسز ڈاؤن لوڈ کرنے کے لئے `Accept: application/x-norito` سیٹ کریں۔ Swagger/RapiDoc اسی ڈراور میں ہیڈر سلیکٹر دکھاتے ہیں اور بائنری کو پراکسی کے ذریعے واپس اسٹریم کرتے ہیں۔

صرف JSON روٹس کے لئے ایمبیڈڈ Try it sandbox عموما تیز ہوتا ہے: پاتھ درج کریں (مثلا `/v1/accounts/soraカタカナ.../assets`)، HTTP طریقہ منتخب کریں، ضرورت ہو تو JSON باڈی پیسٹ کریں، اور **Send request** پر کلک کریں تاکہ ہیڈرز، دورانیہ اور پے لوڈز فوری طور پر دیکھ سکیں۔

## خرابیوں کا ازالہ

| علامت | ممکنہ وجہ | حل |
| --- | --- | --- |
| براؤزر کنسول میں CORS ایررز دکھائی دیں یا sandbox پراکسی URL کے غائب ہونے کی وارننگ دے۔ | پراکسی چل نہیں رہا یا اوریجن وائٹ لسٹ میں نہیں۔ | پراکسی چلائیں، یقینی بنائیں کہ `TRYIT_PROXY_ALLOWED_ORIGINS` پورٹل ہوسٹ کو کور کرتا ہے، اور `npm run start` دوبارہ چلائیں۔ |
| `npm run tryit-proxy` "digest mismatch" کے ساتھ ختم ہو۔ | Torii کا OpenAPI بنڈل اپ اسٹریم تبدیل ہو گیا ہے۔ | `npm run sync-openapi -- --latest` (یا `--version=<tag>`) چلائیں اور دوبارہ کوشش کریں۔ |
| ویجیٹس `401` یا `403` لوٹائیں۔ | ٹوکن موجود نہیں، ختم ہو چکا ہے، یا اسکوپس ناکافی ہیں۔ | OAuth ڈیوائس فلو استعمال کریں یا sandbox میں درست bearer ٹوکن پیسٹ کریں۔ اسٹیٹک ٹوکنز کے لئے `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ایکسپورٹ کرنا ہوگا۔ |
| پراکسی سے `429 Too Many Requests` ملے۔ | فی IP ریٹ لِمٹ تجاوز ہو گئی ہے۔ | قابل اعتماد ماحول کے لئے `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` بڑھائیں یا ٹیسٹ اسکرپٹس کو محدود کریں۔ ریٹ لِمٹ کی تمام مستردیاں `tryit_proxy_rate_limited_total` بڑھاتی ہیں۔ |

## مشاہدہ پذیری

- `npm run probe:tryit-proxy` (`scripts/tryit-proxy-probe.mjs` کے گرد ریپر) `/healthz` کال کرتا ہے، اختیاری طور پر نمونہ روٹ آزما تا ہے، اور `probe_success` / `probe_duration_seconds` کے لئے Prometheus ٹیکسٹ فائلیں بناتا ہے۔ node_exporter کے ساتھ ضم کرنے کے لئے `TRYIT_PROXY_PROBE_METRICS_FILE` سیٹ کریں۔
- `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` سیٹ کریں تاکہ کاؤنٹرز (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) اور لیٹنسی ہسٹوگرامس دستیاب ہوں۔ `dashboards/grafana/docs_portal.json` ڈیش بورڈ ان میٹرکس کو پڑھ کر DOCS-SORA SLOs نافذ کرتا ہے۔
- رن ٹائم لاگز stdout پر ہوتے ہیں۔ ہر اندراج میں ریکویسٹ آئی ڈی، upstream اسٹیٹس، آتھنٹیکیشن سورس (`default`, `override` یا `client`) اور دورانیہ شامل ہوتا ہے؛ راز اخراج سے پہلے ریڈیکٹ کر دیے جاتے ہیں۔

اگر آپ کو تصدیق کرنی ہو کہ `application/x-norito` پے لوڈز بغیر تبدیلی کے Torii تک پہنچتے ہیں، تو Jest سوٹ (`npm test -- tryit-proxy`) چلائیں یا `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` میں fixtures دیکھیں۔ ریگریشن ٹیسٹس کمپریسڈ Norito بائنریز، سائنڈ OpenAPI مینی فیسٹس اور پراکسی ڈاؤن گریڈ راستوں کو کور کرتے ہیں تاکہ NRPC رول آؤٹس میں مستقل ثبوتی ٹریل برقرار رہے۔
