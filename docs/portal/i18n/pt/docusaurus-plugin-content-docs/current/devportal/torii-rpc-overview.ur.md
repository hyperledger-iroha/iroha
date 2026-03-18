---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC کا جائزہ

Norito-RPC Torii APIs کے لئے بائنری ٹرانسپورٹ ہے۔ یہ `/v1/pipeline` e HTTP راستے استعمال کرتا ہے لیکن Norito فریم والے payloads کا تبادلہ کرتا ہے جن میں hashes de esquema e somas de verificação شامل ہوتے ہیں۔ O que você precisa saber sobre o pipeline de pipeline کے JSON جوابات بوتل نیک بن جائیں۔

## کیوں تبدیل کریں؟
- CRC64 e hashes de esquema کے ساتھ متعین enquadramento ڈیکوڈنگ غلطیوں کو کم کرتی ہے۔
- SDKs کے درمیان مشترکہ Norito helpers موجودہ modelo de dados اقسام کو دوبارہ استعمال کرنے دیتی ہیں۔
- Torii پہلے ہی ٹیلی میٹری میں Norito سیشنز کو ٹَیگ کرتا ہے, اس لئے آپریٹرز فراہم کردہ painéis de controle کے ذریعے اپنانے کی نگرانی کر سکتے ہیں۔

## درخواست بھیجنا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. A carga útil do codec Norito (`iroha_client`, auxiliares do SDK, ou `norito::to_bytes`) é a mesma.
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کے ذریعے Norito resposta مانگیں۔
4. Ajudante do SDK کے ساتھ resposta ڈی کوڈ کریں۔

SDK para instalação:
- **Rust**: `iroha_client::Client` ou `Accept` header سیٹ کرتے ہیں تو Norito خودکار طور پر negociar کرتا ہے۔
- **Python**: `iroha_python.norito_rpc` ou `NoritoRpcClient` para usar
- **Android**: Android SDK `NoritoRpcClient` e `NoritoRpcRequestOptions` استعمال کریں۔
- **JavaScript/Swift**: ajudantes کو `docs/source/torii/norito_rpc_tracker.md` میں ٹریک کیا جاتا ہے اور یہ NRPC-3 کے حصے کے طور پر آئیں گے۔

## Experimente

ڈویلپر پورٹل ایک Experimente پراکسی فراہم کرتا ہے تاکہ ریویورز بغیر کسی مخصوص اسکرپٹ کے Cargas úteis Norito

1. [پراکسی شروع کریں](./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` سیٹ کریں تاکہ widgets جان سکیں کہ ٹریفک کہاں بھیجنی ہے۔
2. اس صفحے پر **Experimente** کارڈ یا `/reference/torii-swagger` پینل کھولیں اور `POST /v1/pipeline/submit` جیسا endpoint منتخب کریں۔
3. **Content-Type** کو `application/x-norito` پر سوئچ کریں, **Binary** ایڈیٹر منتخب کریں, اور `fixtures/norito_rpc/transfer_asset.norito` اپ لوڈ کریں (یا `fixtures/norito_rpc/transaction_fixtures.manifest.json` میں درج کوئی بھی carga útil)۔
4. Widget de código de dispositivo OAuth یا دستی ٹوکن فیلڈ کے ذریعے token de portador فراہم کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کنفیگر ہو تو پراکسی `X-TryIt-Auth` substitui o قبول کرتا ہے)۔
5. درخواست بھیجیں اور تصدیق کریں کہ Torii `fixtures/norito_rpc/schema_hashes.json` میں درج `schema_hash` واپس دیتا ہے۔ مطابقت پذیر hashes تصدیق کرتے ہیں کہ Cabeçalho Norito براؤزر/پراکسی hop سے محفوظ رہا۔

روڈ میپ ثبوت کے لئے Try It کا اسکرین شاٹ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کی رَن کے ساتھ جوڑیں۔ یہ اسکرپٹ `cargo xtask norito-rpc-verify` کو ریپ کرتا ہے, JSON خلاصہ `artifacts/norito_rpc/<timestamp>/` میں لکھتا ہے, اور وہی fixtures حاصل کرتا ہے جو پورٹل نے استعمال کئے تھے۔

## مسئلہ حل کرنا

| علامت | کہاں نظر آتا ہے | ممکنہ وجہ | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Resposta Torii | Cabeçalho `Content-Type` | carga útil بھیجنے سے پہلے `Content-Type: application/x-norito` سیٹ کریں۔ |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Corpo/cabeçalhos de resposta Torii | fixtures کا esquema hash Torii build سے مختلف ہے | `cargo xtask norito-rpc-fixtures` سے fixtures دوبارہ بنائیں اور `fixtures/norito_rpc/schema_hashes.json` میں hash تصدیق کریں؛ O endpoint é Norito é o valor do endpoint e o substituto JSON é o valor do fallback |
| `{"error":"origin_forbidden"}` (HTTP403) | Experimente a resposta do proxy | درخواست ایسے origem سے آئی جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں | پورٹل کا origin (مثلا `https://docs.devnet.sora.example`) env var میں شامل کریں اور پراکسی ری اسٹارٹ کریں۔ |
| `{"error":"rate_limited"}` (HTTP429) | Experimente a resposta do proxy | فی IP کوٹا `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` بجٹ سے تجاوز کر گیا | Carregar ٹیسٹ کے لئے حد بڑھائیں یا ونڈو ری سیٹ ہونے تک انتظار کریں (resposta JSON میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Experimente a resposta do proxy | Torii ٹائم آؤٹ ہوا یا پراکسی back-end configurado تک نہیں پہنچ سکا | `TRYIT_PROXY_TARGET` کی دستیابی چیک کریں, Torii کی صحت دیکھیں, یا `TRYIT_PROXY_TIMEOUT_MS` زیادہ رکھ کر دوبارہ کوشش کریں۔ |

مزید Try It diagnostics e dicas OAuth [`devportal/try-it.md`](./try-it.md#norito-rpc-samples) میں موجود ہیں۔

## اضافی وسائل
- RFC de transporte: `docs/source/torii/norito_rpc.md`
- Resumo executivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de ação: `docs/source/torii/norito_rpc_tracker.md`
Instruções de proxy Try-It: `docs/portal/docs/devportal/try-it.md`