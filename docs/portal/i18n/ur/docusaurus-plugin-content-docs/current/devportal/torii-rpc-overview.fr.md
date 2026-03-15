---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC کا جائزہ

Norito-RPC Torii APIs کے لئے بائنری ٹرانسپورٹ ہے۔ یہ وہی HTTP راستوں کو دوبارہ استعمال کرتا ہے جیسے `/v1/pipeline` لیکن Norito کے ذریعہ تیار کردہ پے لوڈ کا تبادلہ کرتا ہے جس میں اسکیما ہیش اور چیکسم شامل ہیں۔ جب آپ کو عین مطابق ، توثیق شدہ ردعمل کی ضرورت ہو یا جب پائپ لائن JSON ردعمل رکاوٹ بن جائے تو اس کا استعمال کریں۔

## کیوں تبدیلی؟
- CRC64 اور اسکیمہ ہیشس کے ساتھ تعی .ن فریمنگ سے ضابطہ کشائی کی غلطیوں کو کم کیا جاتا ہے۔
- Norito SDKs کے مابین مشترکہ مددگار آپ کو موجودہ ڈیٹا ماڈل کی اقسام کو دوبارہ استعمال کرنے کی اجازت دیتے ہیں۔
- Torii پہلے ہی ٹیلی میٹری میں Norito سیشنز ، تاکہ آپریٹرز فراہم کردہ ڈیش بورڈز کے ساتھ گود لینے کا سراغ لگاسکیں۔

## ایک درخواست کریں

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. کوڈیک Norito (`iroha_client` ، SDK مددگار یا `norito::to_bytes`) کے ساتھ اپنے پے لوڈ کو سیریلائز کریں۔
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کے ذریعے جواب Norito کی درخواست کریں۔
4. متعلقہ SDK مددگار کے ساتھ ردعمل کو ڈیکوڈ کریں۔

ایس ڈی کے کے ذریعہ نکات:
۔
- ** ازگر **: `NoritoRpcClient` `iroha_python.norito_rpc` سے استعمال کریں۔
- ** Android **: Android SDK میں `NoritoRpcClient` اور `NoritoRpcRequestOptions` استعمال کریں۔
- ** جاوا اسکرپٹ/سوئفٹ **: مددگاروں کو `docs/source/torii/norito_rpc_tracker.md` میں ٹریک کیا گیا ہے اور وہ NRPC-3 میں پہنچیں گے۔

## اسے کنسول مثال کی کوشش کریں

ڈویلپر پورٹل ایک پراکسی کی کوشش کرتا ہے تاکہ جائزہ لینے والے کسٹم اسکرپٹ لکھے بغیر Norito پے لوڈ کو دوبارہ چلاسکتے ہیں۔

1. [پراکسی شروع کریں] (./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` سیٹ کریں تاکہ ویجٹ کو معلوم ہو کہ ٹریفک کہاں بھیجنا ہے۔
2. اس صفحے یا `/reference/torii-swagger` پینل پر ** نقشہ ** آزمائیں اور `POST /v1/pipeline/submit` کی طرح ایک اختتامی نقطہ منتخب کریں۔
3. ** مواد کی قسم ** کو `application/x-norito` پر منتقل کریں ، ** بائنری ** ایڈیٹر کا انتخاب کریں اور `fixtures/norito_rpc/transfer_asset.norito` (یا `fixtures/norito_rpc/transaction_fixtures.manifest.json` میں درج کوئی بھی پے لوڈ)۔
4. OAUTH ڈیوائس کوڈ ویجیٹ یا دستی فیلڈ کے ذریعہ ایکئرر ٹوکن فراہم کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کے ساتھ تشکیل شدہ ہے تو پراکسی `X-TryIt-Auth` اوور رائڈس کو قبول کرتا ہے)۔
5. درخواست جمع کروائیں اور تصدیق کریں کہ Torii `fixtures/norito_rpc/schema_hashes.json` میں درج `schema_hash` واپس کرتا ہے۔ ایک جیسی ہیشوں نے تصدیق کی ہے کہ Norito ہیڈر براؤزر/پراکسی ہاپ سے بچ گیا ہے۔

روڈ میپ شواہد کے لئے ، `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` پر عمل درآمد کے ساتھ کوشش کریں اسکرین شاٹ کو یکجا کریں۔ اسکرپٹ `cargo xtask norito-rpc-verify` لپیٹتا ہے ، JSON DISED کو `artifacts/norito_rpc/<timestamp>/` پر لکھتا ہے اور وہی فکسچر حاصل کرتا ہے جو پورٹل نے کھایا۔

## خرابیوں کا سراغ لگانا| علامت | یہ کہاں ظاہر ہوتا ہے | ممکنہ وجہ | فکس |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | جواب Torii | ہیڈر `Content-Type` گمشدہ یا غلط | پے لوڈ بھیجنے سے پہلے `Content-Type: application/x-norito` سیٹ کریں۔ |
| `X-Norito-Error-Code: schema_mismatch` (HTTP 400) | رسپانس باڈی/ہیڈر Torii | فکسچر اسکیما ہیش بلڈ Torii سے مختلف ہے `cargo xtask norito-rpc-fixtures` کے ساتھ فکسچر کو دوبارہ تخلیق کریں اور `fixtures/norito_rpc/schema_hashes.json` میں ہیش کی تصدیق کریں۔ JSON پر واپس جائیں اگر اختتامی نقطہ نے ابھی تک Norito کو چالو نہیں کیا ہے۔ |
| `{"error":"origin_forbidden"}` (HTTP 403) | اس کی کوشش کریں پراکسی رسپانس | درخواست اس اصلیت سے ہے جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں ہے ماحولیاتی متغیر میں پورٹل اصل (جیسے `Norito`) شامل کریں اور پراکسی کو دوبارہ شروع کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | اس کی کوشش کریں پراکسی رسپانس | کوٹہ فی IP سے تجاوز کیا گیا بجٹ `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | داخلی بوجھ کی جانچ کی حد میں اضافہ کریں یا ونڈو کو دوبارہ ترتیب دینے کا انتظار کریں (JSON جواب میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) یا `{"error":"upstream_error"}` (HTTP 502) | اس کی کوشش کریں پراکسی رسپانس | Torii ٹائم آؤٹ یا پراکسی کنفیگر بیکینڈ تک نہیں پہنچ سکا چیک کریں کہ `TRYIT_PROXY_TARGET` قابل رسائی ہے ، Torii کی صحت کو چیک کریں یا اعلی `TRYIT_PROXY_TIMEOUT_MS` کے ساتھ دوبارہ کوشش کریں۔ |

مزید کوشش کریں کہ تشخیصی اور OAuth رہنمائی [`devportal/try-it.md`] (./try-it.md#norito-rpc-samples) میں مل سکتی ہے۔

## اضافی وسائل
- ٹرانسپورٹ آر ایف سی: `docs/source/torii/norito_rpc.md`
- ایگزیکٹو خلاصہ: `docs/source/torii/norito_rpc_brief.md`
- ایکشن ٹریکر: `docs/source/torii/norito_rpc_tracker.md`
- پراکسی ہدایات کی کوشش کریں: `docs/portal/docs/devportal/try-it.md`