---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# نظرة عامة على Norito-RPC

Norito-RPC هو النقل الثنائي لواجهات Torii. يعيد استخدام مسارات HTTP نفسها مثل `/v1/pipeline` لكنه يتبادل حمولات مؤطرة عبر Norito تتضمن hashes للمخطط و checksums. استخدمه عندما تحتاج الى ردود حتمية ومتحقق منها او عندما تصبح استجابات JSON الخاصة بالـ pipeline عنق زجاجة.

## لماذا ننتقل؟
- اطار حتمي مع CRC64 و hashes المخطط يقلل اخطاء فك الترميز.
- مساعدات Norito المشتركة عبر SDKs تتيح اعادة استخدام انواع نموذج البيانات الحالية.
- Torii يوسم جلسات Norito بالفعل في القياس عن بعد، لذا يمكن للمشغلين متابعة الاعتماد عبر لوحات المراقبة المتاحة.

## تنفيذ طلب

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. قم بتسلسل الحمولة باستخدام codec Norito (`iroha_client`, مساعدات SDK او `norito::to_bytes`).
2. ارسل الطلب مع `Content-Type: application/x-norito`.
3. اطلب استجابة Norito باستخدام `Accept: application/x-norito`.
4. افك ترميز الاستجابة باستخدام مساعد SDK المطابق.

ارشادات خاصة بالـ SDK:
- **Rust**: `iroha_client::Client` يتفاوض على Norito تلقائيا عندما تضبط ترويسة `Accept`.
- **Python**: استخدم `NoritoRpcClient` من `iroha_python.norito_rpc`.
- **Android**: استخدم `NoritoRpcClient` و `NoritoRpcRequestOptions` في Android SDK.
- **JavaScript/Swift**: المساعدات متتبعة في `docs/source/torii/norito_rpc_tracker.md` وستصل ضمن NRPC-3.

## مثال وحدة Try It

يوفر بوابة المطورين وكيل Try It كي يتمكن المراجعون من اعادة تشغيل حمولات Norito دون كتابة نصوص مخصصة.

1. [ابدأ الوكيل](./try-it.md#start-the-proxy-locally) واضبط `TRYIT_PROXY_PUBLIC_URL` حتى تعرف الادوات المصغرة اين ترسل الحركة.
2. افتح بطاقة **Try it** في هذه الصفحة او لوحة `/reference/torii-swagger` واختر مسارا مثل `POST /v1/pipeline/submit`.
3. بدل **Content-Type** الى `application/x-norito`، اختر محرر **Binary**، وارفع `fixtures/norito_rpc/transfer_asset.norito` (او اي حمولة مدرجة في `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. قدم bearer token عبر ودجت OAuth device-code او حقل الرمز اليدوي (الوكيل يقبل تجاوزات `X-TryIt-Auth` عند ضبط `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. ارسل الطلب وتحقق ان Torii يعيد قيمة `schema_hash` المدرجة في `fixtures/norito_rpc/schema_hashes.json`. تطابق القيم يؤكد ان راس Norito نجا من قفزة المتصفح/الوكيل.

لادلة خارطة الطريق، ارفق لقطة شاشة Try It مع تشغيل `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. يقوم السكربت بتغليف `cargo xtask norito-rpc-verify`، ويكتب ملخص JSON الى `artifacts/norito_rpc/<timestamp>/`، ويلتقط نفس fixtures التي استخدمها البوابة.

## استكشاف الاخطاء

| العرض | مكان الظهور | السبب المحتمل | الحل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | استجابة Torii | ترويسة `Content-Type` مفقودة او غير صحيحة | اضف `Content-Type: application/x-norito` قبل ارسال الحمولة. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | جسم/ترويسات استجابة Torii | hash مخطط fixtures يختلف عن build Torii | اعادة توليد fixtures عبر `cargo xtask norito-rpc-fixtures` وتأكيد hash في `fixtures/norito_rpc/schema_hashes.json`؛ استخدم fallback JSON اذا لم يكن المسار يدعم Norito بعد. |
| `{"error":"origin_forbidden"}` (HTTP 403) | استجابة وكيل Try It | جاء الطلب من اصل غير مدرج في `TRYIT_PROXY_ALLOWED_ORIGINS` | اضف اصل البوابة (مثلا `https://docs.devnet.sora.example`) الى متغير البيئة واعادة تشغيل الوكيل. |
| `{"error":"rate_limited"}` (HTTP 429) | استجابة وكيل Try It | تجاوزت الحصة لكل IP ميزانية `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | زد الحد لاختبارات الحمل الداخلية او انتظر اعادة ضبط النافذة (راجع `retryAfterMs` في استجابة JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) او `{"error":"upstream_error"}` (HTTP 502) | استجابة وكيل Try It | Torii انتهت مهلة الاستجابة او لم يستطع الوكيل الوصول الى الخلفية | تحقق من الوصول الى `TRYIT_PROXY_TARGET`، وافحص صحة Torii، او اعد المحاولة مع `TRYIT_PROXY_TIMEOUT_MS` اكبر. |

المزيد من تشخيصات Try It ونصائح OAuth موجودة في [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## موارد اضافية
- RFC النقل: `docs/source/torii/norito_rpc.md`
- الملخص التنفيذي: `docs/source/torii/norito_rpc_brief.md`
- متعقب الاجراءات: `docs/source/torii/norito_rpc_tracker.md`
- تعليمات وكيل Try-It: `docs/portal/docs/devportal/try-it.md`
