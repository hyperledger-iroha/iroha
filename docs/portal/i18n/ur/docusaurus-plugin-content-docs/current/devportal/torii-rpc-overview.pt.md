---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC جائزہ

Norito-RPC Torii APIs کے لئے بائنری ٹرانسپورٹ ہے۔ یہ وہی HTTP راستوں کو دوبارہ استعمال کرتا ہے جیسے `/v2/pipeline` لیکن Norito فریم پے لوڈ کو تبدیل کرتا ہے جس میں اسکیما ہیش اور چیکسم شامل ہیں۔ جب آپ کو عین مطابق اور توثیق شدہ ردعمل کی ضرورت ہو یا جب JSON پائپ لائن کے ردعمل رکاوٹ بن جائیں تو استعمال کریں۔

## کیوں تبدیلی؟
- CRC64 اور اسکیمہ ہیشس کے ساتھ تعی .ن فریمنگ سے ضابطہ کشائی کی غلطیوں کو کم کیا جاتا ہے۔
- Norito SDKs کے مابین مشترکہ مددگار موجودہ ڈیٹا ماڈل کی اقسام کو دوبارہ استعمال کرنے کی اجازت دیتے ہیں۔
- Torii ٹیلی میٹری میں پہلے ہی Norito سیشنوں کا نشان ہے ، لہذا آپریٹر فراہم کردہ ڈیش بورڈز کے ساتھ گود لینے کی نگرانی کرسکتے ہیں۔

## درخواست دینا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Norito کوڈیک (`iroha_client` ، SDK مددگار یا `norito::to_bytes`) کے ساتھ اپنے پے لوڈ کو سیریلائز کریں۔
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کا استعمال کرتے ہوئے Norito جواب کی درخواست کریں۔
4. متعلقہ SDK مددگار کے ساتھ ردعمل کو ڈیکوڈ کریں۔

گائیڈ بذریعہ SDK:
۔
- ** ازگر **: `NoritoRpcClient` `iroha_python.norito_rpc` سے استعمال کریں۔
- ** Android **: Android SDK میں `NoritoRpcClient` اور `NoritoRpcRequestOptions` استعمال کریں۔
- ** جاوا اسکرپٹ/سوئفٹ **: مددگاروں کو `docs/source/torii/norito_rpc_tracker.md` میں ٹریک کیا گیا ہے اور وہ NRPC-3 کے حصے کے طور پر پہنچیں گے۔

## اسے کنسول مثال کی کوشش کریں

ڈویلپر پورٹل میں ایک آزمائشی پراکسی شامل ہے تاکہ جائزہ لینے والے کسٹم اسکرپٹ لکھے بغیر Norito پے لوڈ کو دوبارہ چلاسکتے ہیں۔

1. [پراکسی شروع کریں] (./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` سیٹ کریں تاکہ ویجٹ جانتے ہو کہ ٹریفک کہاں بھیجنا ہے۔
2. اس صفحے یا `/reference/torii-swagger` پینل پر ** کارڈ آزمائیں ** کارڈ کھولیں اور `POST /v2/pipeline/submit` کی طرح ایک اختتامی نقطہ منتخب کریں۔
3. ** مواد کی قسم ** کو `application/x-norito` میں تبدیل کریں ، ** بائنری ** ایڈیٹر کا انتخاب کریں اور `fixtures/norito_rpc/transfer_asset.norito` (یا `fixtures/norito_rpc/transaction_fixtures.manifest.json` میں درج کوئی بھی پے لوڈ) بھیجیں۔
4. OAUTH ڈیوائس کوڈ ویجیٹ یا دستی فیلڈ کے ذریعہ ایکئرر ٹوکن فراہم کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کے ساتھ تشکیل شدہ ہے تو پراکسی `X-TryIt-Auth` اوور رائڈس کو قبول کرتا ہے)۔
5. درخواست بھیجیں اور تصدیق کریں کہ Torii `fixtures/norito_rpc/schema_hashes.json` میں درج `schema_hash` کی بازگشت کرتا ہے۔ مساوی ہیش نے تصدیق کی ہے کہ Norito ہیڈر براؤزر/پراکسی جمپ سے بچ گیا ہے۔

روڈ میپ شواہد کے لئے ، `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کے رن کے ساتھ آزمائیں اسکرین شاٹ کو یکجا کریں۔ اسکرپٹ `cargo xtask norito-rpc-verify` کو لپیٹتا ہے ، JSON DISED کو `artifacts/norito_rpc/<timestamp>/` پر لکھتا ہے ، اور پورٹل کے ذریعہ استعمال ہونے والے وہی فکسچر کو اپنی گرفت میں لے جاتا ہے۔

## خرابیوں کا سراغ لگانا| علامت | یہ کہاں ظاہر ہوتا ہے | ممکنہ وجہ | اصلاح |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii سے جواب | ہیڈر `Content-Type` گمشدہ یا غلط | پے لوڈ بھیجنے سے پہلے `Content-Type: application/x-norito` سیٹ کریں۔ |
| `X-Norito-Error-Code: schema_mismatch` (HTTP 400) | Torii رسپانس باڈی/ہیڈر | فکسچر اسکیما ہیش Torii بلڈ سے مختلف ہے `cargo xtask norito-rpc-fixtures` کے ساتھ فکسچر کو دوبارہ تخلیق کریں اور `fixtures/norito_rpc/schema_hashes.json` پر ہیش کی تصدیق کریں۔ JSON فال بیک کا استعمال کریں اگر اختتامی نقطہ نے ابھی تک Norito کو فعال نہیں کیا ہے۔ |
| `{"error":"origin_forbidden"}` (HTTP 403) | اس کی کوشش کریں پراکسی رسپانس | درخواست ایک ایسے ذریعہ سے آئی ہے جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں ہے ماحولیاتی متغیر میں پورٹل ماخذ (مثال کے طور پر ، `Norito`) شامل کریں اور پراکسی کو دوبارہ شروع کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | اس کی کوشش کریں پراکسی رسپانس | کوٹہ فی IP سے تجاوز کیا گیا بجٹ `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | اندرونی بوجھ ٹیسٹوں کی حد میں اضافہ کریں یا ونڈو کے دوبارہ اسٹارٹ ہونے کا انتظار کریں (JSON جواب میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) یا `{"error":"upstream_error"}` (HTTP 502) | اس کی کوشش کریں پراکسی رسپانس | Torii میعاد ختم ہو گیا یا پراکسی تشکیل شدہ پسدید تک پہنچنے سے قاصر تھا | چیک کریں کہ آیا `TRYIT_PROXY_TARGET` قابل رسائی ہے ، Torii کی صحت کو چیک کریں ، یا بڑے `TRYIT_PROXY_TIMEOUT_MS` کے ساتھ دوبارہ کوشش کریں۔ |

مزید کوشش کریں کہ تشخیصی اور oauth کے نکات [`devportal/try-it.md`] (./try-it.md#norito-rpc-samples) پر ہیں۔

## اضافی خصوصیات
- ٹرانسپورٹ آر ایف سی: `docs/source/torii/norito_rpc.md`
- ایگزیکٹو خلاصہ: `docs/source/torii/norito_rpc_brief.md`
- اسٹاک ٹریکر: `docs/source/torii/norito_rpc_tracker.md`
- پراکسی ہدایات کی کوشش کریں: `docs/portal/docs/devportal/try-it.md`