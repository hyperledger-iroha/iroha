---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC جائزہ

Norito-RPC Torii انٹرفیس کی بائنری ٹرانسپورٹ ہے۔ یہ وہی HTTP راستوں کو دوبارہ استعمال کرتا ہے جیسے `/v2/pipeline` لیکن Norito کے ذریعہ فریمڈ پے لوڈ کا تبادلہ کرتا ہے جس میں اسکیما ہیش اور چیکمس شامل ہیں۔ جب آپ کو لازمی ، توثیق شدہ ردعمل کی ضرورت ہو یا جب آپ کی پائپ لائن کے JSON ردعمل رکاوٹ بن جائیں تو اس کا استعمال کریں۔

## ہم کیوں منتقل ہوتے ہیں؟
- CRC64 اور ہیشس اسکیم کے ساتھ فریم ڈٹرمینسٹک ضابطہ کشائی کی غلطیوں کو کم کرتا ہے۔
- Norito SDKs کے ذریعہ مشترکہ مددگار موجودہ ڈیٹا ماڈل کی اقسام کو دوبارہ استعمال کرنے کی اجازت دیتے ہیں۔
- Torii ٹیگز Norito سیشن پہلے ہی ٹیلی میٹری میں ہیں ، لہذا آپریٹر دستیاب نگرانی کے پینلز کے ذریعہ منظوری کو ٹریک کرسکتے ہیں۔

## درخواست پر عمل درآمد

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. کوڈیک Norito (`iroha_client` ، SDK پلگ انز یا `norito::to_bytes`) کا استعمال کرتے ہوئے پے لوڈ کو سیریلائز کریں۔
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کا استعمال کرتے ہوئے Norito کے جواب کی درخواست کریں۔
4. متعلقہ SDK مددگار کا استعمال کرتے ہوئے ردعمل کو ڈیکوڈ کریں۔

ایس ڈی کے ہدایات:
۔
- ** ازگر **: `NoritoRpcClient` `iroha_python.norito_rpc` سے استعمال کریں۔
- ** Android **: Android SDK میں `NoritoRpcClient` اور `NoritoRpcRequestOptions` استعمال کریں۔
- ** جاوا اسکرپٹ/سوئفٹ **: مددگاروں کو `docs/source/torii/norito_rpc_tracker.md` پر ٹریک کیا جاتا ہے اور وہ NRPC-3 کے تحت پہنچیں گے۔

## اس کو ماڈیول کی مثال دیں

ڈویلپر پورٹل ایک آزمائشی آئی ٹی ایجنٹ فراہم کرتا ہے تاکہ جائزہ لینے والے کسٹم اسکرپٹ لکھے بغیر Norito پے لوڈ کو دوبارہ چلاسکتے ہیں۔

1. [اسٹارٹ ایجنٹ] (./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` سیٹ کریں تاکہ ویجٹ کو معلوم ہو کہ ٹریفک کہاں بھیجنا ہے۔
2. اس صفحے یا `/reference/torii-swagger` پینل پر ** کارڈ کو آزمائیں ** کارڈ کھولیں اور `POST /v2/pipeline/submit` جیسے راستہ کا انتخاب کریں۔
3. تبدیل کریں ** مواد کی قسم ** `application/x-norito` میں ، ** بائنری ** ایڈیٹر کا انتخاب کریں ، اور `fixtures/norito_rpc/transfer_asset.norito` (یا کوئی بھی پے لوڈ `fixtures/norito_rpc/transaction_fixtures.manifest.json` کے طور پر درج) اپ لوڈ کریں۔
4. OAUTH ڈیوائس کوڈ ویجیٹ یا دستی ٹوکن فیلڈ کے ذریعے بیئرر ٹوکن فراہم کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` سیٹ ہوتا ہے تو ایجنٹ `X-TryIt-Auth` اوور رائڈس کو قبول کرتا ہے)۔
5. درخواست جمع کروائیں اور تصدیق کریں کہ Torii `schema_hash` کی قیمت کو `fixtures/norito_rpc/schema_hashes.json` میں شامل کرتا ہے۔ مماثل اقدار اس بات کی تصدیق کرتی ہیں کہ Norito ہیڈر براؤزر/پراکسی جمپ سے بچ گیا۔

روڈ میپ گائڈز کے ل it ، `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` چلانے کے ساتھ آزمائیں کا اسکرین شاٹ منسلک کریں۔ اسکرپٹ `cargo xtask norito-rpc-verify` کو لپیٹتا ہے ، `artifacts/norito_rpc/<timestamp>/` پر JSON ڈائجسٹ لکھتا ہے ، اور وہی فکسچر اٹھاتا ہے جو گیٹ وے نے استعمال کیا تھا۔

## خرابیوں کا سراغ لگانا| چوڑائی | ظاہری مقام ممکنہ وجہ | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | جواب Torii | `Content-Type` ہیڈر غائب ہے یا غلط ہے | پے لوڈ بھیجنے سے پہلے `Content-Type: application/x-norito` شامل کریں۔ |
| `X-Norito-Error-Code: schema_mismatch` (HTTP 400) | جسم/رسپانس ہیڈر Torii | ہیش فکسچر چارٹ بلڈ Torii | سے مختلف ہے `cargo xtask norito-rpc-fixtures` کے ذریعے فکسچر کو دوبارہ تخلیق کریں اور `fixtures/norito_rpc/schema_hashes.json` پر ہیش کی تصدیق کریں۔ اگر راستہ ابھی تک Norito کی حمایت نہیں کرتا ہے تو فال بیک JSON استعمال کریں۔ |
| `{"error":"origin_forbidden"}` (HTTP 403) | ایجنٹ کا جواب اس کی کوشش کریں | درخواست اس اصلیت سے آئی ہے جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں ہے ماحولیاتی متغیر میں گیٹ وے کی اصل (مثال کے طور پر `https://docs.devnet.sora.example`) شامل کریں اور ایجنٹ کو دوبارہ شروع کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | ایجنٹ کا جواب اس کی کوشش کریں | فی آئی پی کوٹہ سے تجاوز کیا گیا بجٹ `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | اندرونی بوجھ ٹیسٹوں کی حد میں اضافہ کریں یا ونڈو کو دوبارہ ترتیب دینے کا انتظار کریں (JSON جواب میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) یا `{"error":"upstream_error"}` (HTTP 502) | ایجنٹ کا جواب اس کی کوشش کریں | Torii ردعمل کا وقت ختم ہوا یا ایجنٹ پسدید تک رسائی حاصل نہیں کرسکا | `TRYIT_PROXY_TARGET` تک رسائی کی تصدیق کریں ، Torii کی صداقت کو چیک کریں ، یا بڑے `TRYIT_PROXY_TIMEOUT_MS` کے ساتھ دوبارہ کوشش کریں۔ |

مزید کوشش کریں کہ تشخیصی اور oauth کے نکات [`devportal/try-it.md`] (./try-it.md#norito-rpc-samples) پر ہیں۔

## اضافی وسائل
- ٹرانسپورٹ آر ایف سی: `docs/source/torii/norito_rpc.md`
- ایگزیکٹو خلاصہ: `docs/source/torii/norito_rpc_brief.md`
- ایکشن ٹریکر: `docs/source/torii/norito_rpc_tracker.md`
- TRY-IT ایجنٹ کی ہدایات: `docs/portal/docs/devportal/try-it.md`