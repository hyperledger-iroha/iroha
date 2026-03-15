---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC کا جائزہ

Norito -RPC - API Torii کے لئے بائنری ٹرانسپورٹ۔ یہ وہی HTTP راستوں کا استعمال کرتا ہے جیسے `/v1/pipeline` ، لیکن اسکیما اور چیکسم ہیشس کے ساتھ Norito فریمنگ کا تبادلہ کرتا ہے۔ جب اس کا استعمال کریں جب عین مطابق اور توثیق شدہ ردعمل کی ضرورت ہو یا جب JSON پائپ لائن کے ردعمل رکاوٹ بن جائیں۔

## سوئچ کیوں؟
- CRC64 اور سرکٹ ہیشس کے ساتھ تعی .ن فریمنگ سے ضابطہ کشائی کی غلطیوں کو کم کیا جاتا ہے۔
- ایس ڈی کے کے مابین عام Norito مددگار آپ کو موجودہ ڈیٹا ماڈل کی اقسام کو دوبارہ استعمال کرنے کی اجازت دیتے ہیں۔
- Torii ٹیلی میٹری میں پہلے ہی جھنڈے Norito سیشنز ، لہذا آپریٹر دستیاب ڈیش بورڈز کے ذریعے گود لینے کا سراغ لگاسکتے ہیں۔

## درخواست بھیجنا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Norito کوڈیک (`iroha_client` ، SDK مددگار یا `norito::to_bytes`) کے ذریعے پے لوڈ کو سیریلائز کریں۔
2. `Content-Type: application/x-norito` کے ساتھ ایک درخواست جمع کروائیں۔
3. درخواست جواب Norito `Accept: application/x-norito` کے ساتھ۔
4. مناسب SDK مددگار کے ساتھ جواب کو ڈی کوڈ کریں۔

ایس ڈی کے کی سفارشات:
۔
- ** ازگر **: `NoritoRpcClient` `iroha_python.norito_rpc` سے استعمال کریں۔
- ** Android **: Android SDK میں `NoritoRpcClient` اور `NoritoRpcRequestOptions` استعمال کریں۔
- ** جاوا اسکرپٹ/سوئفٹ **: مددگاروں کو `docs/source/torii/norito_rpc_tracker.md` میں ٹریک کیا گیا ہے اور وہ NRPC-3 کے حصے کے طور پر ظاہر ہوں گے۔

## اسے کنسول مثال کی کوشش کریں

ڈویلپر پورٹل فراہم کرتا ہے اس کی پراکسیوں کی کوشش کرتا ہے تاکہ جائزہ لینے والے الگ الگ اسکرپٹ لکھے بغیر Norito پے لوڈ کھیل سکیں۔

1. [پراکسی شروع کریں] (./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` مرتب کریں تاکہ ویجٹ کو معلوم ہو کہ ٹریفک کہاں بھیجنا ہے۔
2. اس صفحے یا `/reference/torii-swagger` پینل پر ** کارڈ کو ** آزمائیں اور ایک اختتامی نقطہ منتخب کریں ، مثال کے طور پر `POST /v1/pipeline/submit`۔
3.
4. OAUTH ڈیوائس کوڈ ویجیٹ یا دستی طور پر بیئرر ٹوکن کی وضاحت کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کی وضاحت کی گئی ہو تو پراکسی `X-TryIt-Auth` کو اوور رائڈس قبول کرتا ہے)۔
5. درخواست جمع کروائیں اور تصدیق کریں کہ Torii `fixtures/norito_rpc/schema_hashes.json` میں مخصوص `schema_hash` واپس کرتا ہے۔ ہیشوں کا میچ اس بات کی تصدیق کرتا ہے کہ Norito ہیڈر براؤزر/پراکسی ہاپ سے بچ گیا۔

روڈ میپ شواہد کے لئے ، `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کے اجراء کے ساتھ کوشش کرنے کے اسکرین شاٹ کو یکجا کریں۔ اسکرپٹ `cargo xtask norito-rpc-verify` کو لپیٹتا ہے ، `artifacts/norito_rpc/<timestamp>/` پر JSON کا خلاصہ لکھتا ہے ، اور وہی فکسچر پکڑتا ہے جو پورٹل نے استعمال کیا تھا۔

## خرابیوں کا سراغ لگانا| علامت | یہ کہاں ظاہر ہوتا ہے | ممکنہ وجہ | اصلاح |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | جواب دیں Torii | گمشدہ یا غلط ہیڈر `Content-Type` | پے لوڈ بھیجنے سے پہلے `Content-Type: application/x-norito` سیٹ کریں۔ |
| `X-Norito-Error-Code: schema_mismatch` (HTTP 400) | رسپانس باڈی/ہیڈر Torii | فکسچر اسکیما ہیش اسمبلی Torii سے مماثل نہیں ہے `cargo xtask norito-rpc-fixtures` کے ذریعے فکسچر کو دوبارہ بنائیں اور `fixtures/norito_rpc/schema_hashes.json` میں ہیش کو چیک کریں۔ JSON فال بیک کا استعمال کریں اگر اختتامی نقطہ پہلے سے Norito شامل نہیں کرتا ہے۔ |
| `{"error":"origin_forbidden"}` (HTTP 403) | جواب دیں پراکسی کی کوشش کریں | درخواست اصل سے آئی ہے ، جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں نہیں ہے ماحولیاتی متغیر میں پورٹل اصل (مثال کے طور پر ، `Norito`) شامل کریں اور پراکسی کو دوبارہ شروع کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | جواب دیں پراکسی کی کوشش کریں | آئی پی کوٹہ سے تجاوز کیا گیا بجٹ `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | اندرونی بوجھ ٹیسٹوں کی حد میں اضافہ کریں یا ونڈو کو دوبارہ ترتیب دینے کا انتظار کریں (JSON جواب میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) یا `{"error":"upstream_error"}` (HTTP 502) | جواب دیں پراکسی کی کوشش کریں | Torii ٹائم آؤٹ یا پراکسی بیک اینڈ تک نہیں پہنچ سکا `TRYIT_PROXY_TARGET` کی دستیابی کو چیک کریں ، Torii کی حیثیت ، یا بڑے `TRYIT_PROXY_TIMEOUT_MS` کے ساتھ دہرائیں۔ |

مزید کوشش کریں کہ تشخیصی اور OAuth کے نکات [`devportal/try-it.md`] (./try-it.md#norito-rpc-samples) میں ہیں۔

## اضافی وسائل
- ٹرانسپورٹ آر ایف سی: `docs/source/torii/norito_rpc.md`
- ایگزیکٹو خلاصہ: `docs/source/torii/norito_rpc_brief.md`
- ٹاسک ٹریکر: `docs/source/torii/norito_rpc_tracker.md`
- پراکسی ہدایات کی کوشش کریں: `docs/portal/docs/devportal/try-it.md`