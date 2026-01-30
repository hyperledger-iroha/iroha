---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito-RPC کا جائزہ

Norito-RPC Torii APIs کے لئے بائنری ٹرانسپورٹ ہے۔ یہ `/v1/pipeline` والی وہی HTTP راستے استعمال کرتا ہے لیکن Norito فریم والے payloads کا تبادلہ کرتا ہے جن میں schema hashes اور checksums شامل ہوتے ہیں۔ اسے وہاں استعمال کریں جہاں آپ کو متعین اور تصدیق شدہ جوابات درکار ہوں، یا جب pipeline کے JSON جوابات بوتل نیک بن جائیں۔

## کیوں تبدیل کریں؟
- CRC64 اور schema hashes کے ساتھ متعین framing ڈیکوڈنگ غلطیوں کو کم کرتی ہے۔
- SDKs کے درمیان مشترکہ Norito helpers موجودہ data-model اقسام کو دوبارہ استعمال کرنے دیتی ہیں۔
- Torii پہلے ہی ٹیلی میٹری میں Norito سیشنز کو ٹَیگ کرتا ہے، اس لئے آپریٹرز فراہم کردہ dashboards کے ذریعے اپنانے کی نگرانی کر سکتے ہیں۔

## درخواست بھیجنا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. اپنے payload کو Norito codec (`iroha_client`, SDK helpers، یا `norito::to_bytes`) سے سیریلائز کریں۔
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کے ذریعے Norito response مانگیں۔
4. متعلقہ SDK helper کے ساتھ response ڈی کوڈ کریں۔

SDK کے لحاظ سے رہنمائی:
- **Rust**: `iroha_client::Client` جب آپ `Accept` header سیٹ کرتے ہیں تو Norito خودکار طور پر negotiate کرتا ہے۔
- **Python**: `iroha_python.norito_rpc` سے `NoritoRpcClient` استعمال کریں۔
- **Android**: Android SDK میں `NoritoRpcClient` اور `NoritoRpcRequestOptions` استعمال کریں۔
- **JavaScript/Swift**: helpers کو `docs/source/torii/norito_rpc_tracker.md` میں ٹریک کیا جاتا ہے اور یہ NRPC-3 کے حصے کے طور پر آئیں گے۔

## Try It کنسول کی مثال

ڈویلپر پورٹل ایک Try It پراکسی فراہم کرتا ہے تاکہ ریویورز بغیر کسی مخصوص اسکرپٹ کے Norito payloads کو دوبارہ چلا سکیں۔

1. [پراکسی شروع کریں](./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` سیٹ کریں تاکہ widgets جان سکیں کہ ٹریفک کہاں بھیجنی ہے۔
2. اس صفحے پر **Try it** کارڈ یا `/reference/torii-swagger` پینل کھولیں اور `POST /v1/pipeline/submit` جیسا endpoint منتخب کریں۔
3. **Content-Type** کو `application/x-norito` پر سوئچ کریں، **Binary** ایڈیٹر منتخب کریں، اور `fixtures/norito_rpc/transfer_asset.norito` اپ لوڈ کریں (یا `fixtures/norito_rpc/transaction_fixtures.manifest.json` میں درج کوئی بھی payload)۔
4. OAuth device-code widget یا دستی ٹوکن فیلڈ کے ذریعے bearer token فراہم کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کنفیگر ہو تو پراکسی `X-TryIt-Auth` overrides قبول کرتا ہے)۔
5. درخواست بھیجیں اور تصدیق کریں کہ Torii `fixtures/norito_rpc/schema_hashes.json` میں درج `schema_hash` واپس دیتا ہے۔ مطابقت پذیر hashes تصدیق کرتے ہیں کہ Norito header براؤزر/پراکسی hop سے محفوظ رہا۔

روڈ میپ ثبوت کے لئے Try It کا اسکرین شاٹ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کی رَن کے ساتھ جوڑیں۔ یہ اسکرپٹ `cargo xtask norito-rpc-verify` کو ریپ کرتا ہے، JSON خلاصہ `artifacts/norito_rpc/<timestamp>/` میں لکھتا ہے، اور وہی fixtures حاصل کرتا ہے جو پورٹل نے استعمال کئے تھے۔

## مسئلہ حل کرنا

| علامت | کہاں نظر آتا ہے | ممکنہ وجہ | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii response | `Content-Type` header غائب یا غلط | payload بھیجنے سے پہلے `Content-Type: application/x-norito` سیٹ کریں۔ |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii response body/headers | fixtures کا schema hash Torii build سے مختلف ہے | `cargo xtask norito-rpc-fixtures` سے fixtures دوبارہ بنائیں اور `fixtures/norito_rpc/schema_hashes.json` میں hash تصدیق کریں؛ اگر endpoint ابھی Norito فعال نہیں کرتا تو JSON fallback استعمال کریں۔ |
| `{"error":"origin_forbidden"}` (HTTP 403) | Try It proxy response | درخواست ایسے origin سے آئی جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں | پورٹل کا origin (مثلا `https://docs.devnet.sora.example`) env var میں شامل کریں اور پراکسی ری اسٹارٹ کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | Try It proxy response | فی IP کوٹا `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` بجٹ سے تجاوز کر گیا | اندرونی load ٹیسٹ کے لئے حد بڑھائیں یا ونڈو ری سیٹ ہونے تک انتظار کریں (JSON response میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) یا `{"error":"upstream_error"}` (HTTP 502) | Try It proxy response | Torii ٹائم آؤٹ ہوا یا پراکسی configured backend تک نہیں پہنچ سکا | `TRYIT_PROXY_TARGET` کی دستیابی چیک کریں، Torii کی صحت دیکھیں، یا `TRYIT_PROXY_TIMEOUT_MS` زیادہ رکھ کر دوبارہ کوشش کریں۔ |

مزید Try It diagnostics اور OAuth tips [`devportal/try-it.md`](./try-it.md#norito-rpc-samples) میں موجود ہیں۔

## اضافی وسائل
- Transport RFC: `docs/source/torii/norito_rpc.md`
- Executive summary: `docs/source/torii/norito_rpc_brief.md`
- Action tracker: `docs/source/torii/norito_rpc_tracker.md`
- Try-It proxy instructions: `docs/portal/docs/devportal/try-it.md`
