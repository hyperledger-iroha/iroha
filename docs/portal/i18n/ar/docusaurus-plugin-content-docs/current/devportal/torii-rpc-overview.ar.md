---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito-RPC

Norito-RPC هو الإصدار الجديد لواجهات Torii. أعاد استخدام مسارات HTTP لنفسه مثل `/v2/pipeline` لكنه يتبادل حمولات مؤطرة عبر Norito تتضمن hashes للمخطط و المجموع الاختباري. استخدمه عندما تحتاج الى ردود حتمية ومتحقق منها او عندما تصبح استجابات JSON الخاصة بالـ Pipeline رقيقة زجاجة.

## لماذا ننتقل؟
- إطار حتمي مع CRC64 و hashes سيحدد اخطاء فك الترميز.
- مساعدات Norito المشتركة عبر SDKs تكرار إعادة استخدام نموذج البيانات الحالية.
- Torii يوسم جلسات Norito بالفعل في القياس عن بعد، لذا يمكن للمشغلين الاعتماد عبر لوحات المراقبة المتاحة.

##تنفيذ الطلب

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. قم بالتسلسل الرقمي باستخدام برنامج الترميز Norito (`iroha_client`, مساعدات SDK او `norito::to_bytes`).
2. ارسل الطلب مع `Content-Type: application/x-norito`.
3. اطلب العضوية Norito باستخدام `Accept: application/x-norito`.
4. فك ترميز باستخدام مساعد SDK المطابق.

اتجاهات خاصة بالـ SDK:
- **الصدأ**: `iroha_client::Client` للتفاوض على Norito تلقائيا عندما تضبط ترويسة `Accept`.
- **بايثون**: استخدم `NoritoRpcClient` من `iroha_python.norito_rpc`.
- **Android**: استخدم `NoritoRpcClient` و`NoritoRpcRequestOptions` في Android SDK.
- **JavaScript/Swift**: المساعدات المتتابعة في `docs/source/torii/norito_rpc_tracker.md` ووصل ضمن NRPC-3.

## مثال وحدة جربها

ويتوفر بوابة المطورين وكيل Try It لتتمكن من إعادة بناء حمولات Norito دون كتابة النصوص المخصصة.

1. [ابدأ الوكيل](./try-it.md#start-the-proxy-locally) واضبط `TRYIT_PROXY_PUBLIC_URL` حتى تعرف الادوات المعتمدة اين تتجسس.
2. قم بتسجيل الدخول إلى البطاقة **جربها** في هذه الصفحة او لوحة `/reference/torii-swagger` والعالم بطريقا مثل `POST /v2/pipeline/submit`.
3. بدل **Content-Type** الى `application/x-norito`، برنامج محرر **Binary**، وارفع `fixtures/norito_rpc/transfer_asset.norito` (او اي حمولة مدرجة في `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. قدم الرمز المميز لحامله عبر ودجت OAuth جهاز كود او البحث الرمزي اليدوي (الوكيل يقبل تجاوزات `X-TryIt-Auth` عند ضبط `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. ارسل الطلب وتحقق ان Torii يعيد القيمة `schema_hash` المدرجة في `fixtures/norito_rpc/schema_hashes.json`. تطابق القيم حسب ان راس Norito نجا من تصفح المتصفح/الوكيل.

لمعادلة خارطة الطريق، ارفق لقطة شاشة Try It مع `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. يقوم السكربت بتغليف `cargo xtask norito-rpc-verify`، ويكتب ملخص JSON الى `artifacts/norito_rpc/<timestamp>/`، ويختار نفس التركيبات التي يستخدمها البوابة.

##استكشاف الاخطاء

| | مكان الموضة | عيد الميلاد | الحل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | عضو Torii | ترويسة `Content-Type` مفقودة او غير صحيحة | اضف `Content-Type: application/x-norito` قبل الإرسال الفضائي. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | جسم/ترويسات الأعضاء Torii | تركيبات مخطط التجزئة تختلف عن البناء Torii | إعادة توليد التركيبات عبر `cargo xtask norito-rpc-fixtures` وتأكيد التجزئة في `fixtures/norito_rpc/schema_hashes.json`؛ استخدم النسخة الاحتياطية JSON إذا لم يكن البرنامج مدعومًا Norito بعد. |
| `{"error":"origin_forbidden"}` (HTTP 403) | عضو وكيل جربه | جاء الطلب من اصل غير مدرج في `TRYIT_PROXY_ALLOWED_ORIGINS` | اضف اصل البوابة (مثلا `https://docs.devnet.sora.example`) الى بيئة متغيرة واعادة تشغيل المدير. |
| `{"error":"rate_limited"}` (HTTP 429) | عضو وكيل جربه | تجاوزت الحصة لكل IP `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | زرت زرادت النوافذ الداخليه قبل حدوث او إعادة ضبط (راجع `retryAfterMs` في JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) او `{"error":"upstream_error"}` (HTTP 502) | عضو وكيل جربه | Torii يبقى مهلة الخبرة او لم يطلب الوصول الى الخلفية | تحقق من الوصول الى `TRYIT_PROXY_TARGET`، وافحص صحة Torii، او اعد المحاولة مع `TRYIT_PROXY_TIMEOUT_MS` اكبر. |

المزيد من برامج التشخيص Try It ونصائح OAuth موجودة في [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## الموارد
- RFC النقل: `docs/source/torii/norito_rpc.md`
- الملخص التنفيذي: `docs/source/torii/norito_rpc_brief.md`
- متعقب: `docs/source/torii/norito_rpc_tracker.md`
- تعليمات وكيل Try-It: `docs/portal/docs/devportal/try-it.md`