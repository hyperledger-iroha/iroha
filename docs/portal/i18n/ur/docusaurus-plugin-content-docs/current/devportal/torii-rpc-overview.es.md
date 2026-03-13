---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC کا خلاصہ

Norito-RPC Torii APIs کے لئے بائنری ٹرانسپورٹ ہے۔ یہ اسی HTTP راستوں کو دوبارہ استعمال کرتا ہے جیسے `/v2/pipeline` لیکن Norito فریم پے لوڈ کو تبدیل کرتا ہے جس میں اسکیما ہیش اور چیکمس شامل ہیں۔ جب آپ کو عین مطابق ، توثیق شدہ ردعمل کی ضرورت ہو یا جب پائپ لائن JSON ردعمل رکاوٹ بن جائے تو اس کا استعمال کریں۔

## کیوں تبدیلی؟
- CRC64 اور اسکیمہ ہیشس کے ساتھ تعی .ن فریمنگ سے ضابطہ کشائی کی غلطیوں کو کم کیا جاتا ہے۔
- Norito SDKs کے مابین مشترکہ مددگار آپ کو موجودہ ڈیٹا ماڈل کی اقسام کو دوبارہ استعمال کرنے کی اجازت دیتے ہیں۔
- Torii پہلے ہی ٹیلی میٹری میں Norito سیشنز ، تاکہ آپریٹر فراہم کردہ ڈیش بورڈز کے ساتھ گود لینے کی نگرانی کرسکیں۔

## درخواست کیسے کریں

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Norito کوڈیک (`iroha_client` ، SDK مددگار یا `norito::to_bytes`) کے ساتھ اپنے پے لوڈ کو سیریلائز کریں۔
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کا استعمال کرتے ہوئے Norito کے جواب کی درخواست کریں۔
4. متعلقہ SDK مددگار کے ساتھ ردعمل کو ڈیکوڈ کریں۔

گائیڈ بذریعہ SDK:
۔
- ** ازگر **: `NoritoRpcClient` `iroha_python.norito_rpc` سے استعمال کریں۔
- ** Android **: Android SDK میں `NoritoRpcClient` اور `NoritoRpcRequestOptions` استعمال کریں۔
- ** جاوا اسکرپٹ/سوئفٹ **: مددگاروں کو `docs/source/torii/norito_rpc_tracker.md` میں ٹریک کیا گیا ہے اور وہ NRPC-3 کے حصے کے طور پر پہنچیں گے۔

## اسے کنسول مثال کی کوشش کریں

ڈویلپر پورٹل میں ایک آزمائشی پراکسی شامل ہے تاکہ جائزہ لینے والے کسٹم اسکرپٹ لکھے بغیر Norito پے لوڈ کھیل سکتے ہیں۔

1. [پراکسی شروع کریں] (./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` کی وضاحت کریں تاکہ ویجٹ کو معلوم ہو کہ ٹریفک کہاں بھیجنا ہے۔
2. اس صفحے یا `/reference/torii-swagger` پینل پر ** کارڈ کو ** آزمائیں اور `POST /v2/pipeline/submit` جیسے اختتامی نقطہ کو منتخب کریں۔
3. ** مواد کی قسم ** کو `application/x-norito` میں تبدیل کریں ، ** بائنری ** ایڈیٹر کا انتخاب کریں اور `fixtures/norito_rpc/transfer_asset.norito` (یا `fixtures/norito_rpc/transaction_fixtures.manifest.json` میں درج کوئی بھی پے لوڈ) کو اپ لوڈ کریں۔
4. OAUTH ڈیوائس کوڈ ویجیٹ یا دستی ٹوکن فیلڈ کے ذریعے بیئرر ٹوکن فراہم کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کے ساتھ تشکیل دیا جاتا ہے تو پراکسی `X-TryIt-Auth` اوور رائڈس کو قبول کرتا ہے)۔
5. درخواست جمع کروائیں اور تصدیق کریں کہ Torii `fixtures/norito_rpc/schema_hashes.json` میں درج `schema_hash` کی عکاسی کرتا ہے۔ مماثل ہیشوں نے اس بات کی تصدیق کی ہے کہ Norito ہیڈر براؤزر/پراکسی جمپ سے بچ گیا ہے۔

روڈ میپ کے ثبوت کے لئے ، `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کے رن کے ساتھ کوشش کریں اس اسکرین شاٹ کو یکجا کریں۔ اسکرپٹ `cargo xtask norito-rpc-verify` لپیٹتا ہے ، JSON کا خلاصہ `artifacts/norito_rpc/<timestamp>/` پر لکھتا ہے اور وہی فکسچر حاصل کرتا ہے جو پورٹل نے کھایا تھا۔

## خرابیوں کا سراغ لگانا| علامت | جہاں یہ ظاہر ہوتا ہے | ممکنہ وجہ | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii سے جواب | ہیڈر `Content-Type` غائب ہے یا غلط ہے | پے لوڈ بھیجنے سے پہلے `Content-Type: application/x-norito` کی وضاحت کریں۔ |
| `X-Norito-Error-Code: schema_mismatch` (HTTP 400) | Torii رسپانس باڈی/ہیڈر | حقیقت میں اسکیمیٹک ہیش Torii کی تعمیر سے مختلف ہے `cargo xtask norito-rpc-fixtures` کے ساتھ فکسچر کو دوبارہ تخلیق کریں اور `fixtures/norito_rpc/schema_hashes.json` میں ہیش کی تصدیق کریں۔ اگر فیل بیک JSON استعمال کریں اگر اختتامی نقطہ نے ابھی تک Norito کو فعال نہیں کیا ہے۔ |
| `{"error":"origin_forbidden"}` (HTTP 403) | اس کی کوشش کریں پراکسی رسپانس | درخواست ایک ایسے ذریعہ سے آئی ہے جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں ہے ماحولیاتی متغیر میں پورٹل ماخذ (مثال کے طور پر ، `Norito`) شامل کریں اور پراکسی کو دوبارہ شروع کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | اس کی کوشش کریں پراکسی رسپانس | آئی پی کوٹہ نے `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | کے بجٹ سے تجاوز کیا اندرونی بوجھ ٹیسٹوں کی حد میں اضافہ کریں یا ونڈو کے دوبارہ شروع ہونے تک انتظار کریں (JSON ردعمل میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) یا `{"error":"upstream_error"}` (HTTP 502) | اس کی کوشش کریں پراکسی رسپانس | Torii کا وقت ختم ہوا یا پراکسی تشکیل شدہ پسدید تک نہیں پہنچ سکا تصدیق کریں کہ `TRYIT_PROXY_TARGET` قابل رسائ ہے ، Torii کی صحت کو چیک کریں ، یا بڑے `TRYIT_PROXY_TIMEOUT_MS` کے ساتھ دوبارہ کوشش کریں۔ |

[`devportal/try-it.md`] (./try-it.md#norito-rpc-samples) میں براہ راست تشخیص اور OAUTH کے اشارے مزید آزمائیں۔

## اضافی وسائل
- ٹرانسپورٹ آر ایف سی: `docs/source/torii/norito_rpc.md`
- ایگزیکٹو خلاصہ: `docs/source/torii/norito_rpc_brief.md`
- ایکشن ٹریکر: `docs/source/torii/norito_rpc_tracker.md`
- پراکسی ہدایات کی کوشش کریں: `docs/portal/docs/devportal/try-it.md`