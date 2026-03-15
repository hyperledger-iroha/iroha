---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# فيساو عامة تفعل Norito-RPC

Norito-RPC وينقل الثنائي كما تفعل واجهات برمجة التطبيقات (API) مع Torii. سيتم إعادة استخدام بروتوكول HTTP الخاص بـ `/v2/pipeline` مع تحميل الحمولات المعززة لـ Norito التي تتضمن تجزئات المخطط والمجاميع الاختبارية. استخدم عند تحديد الإجابات الحتمية والتحقق من الصحة أو عند إرسال إجابات JSON لخط الأنابيب بشكل متكرر.

## لماذا مدر؟
- التوسع الحتمي مع CRC64 وأجزاء المخطط تقلل من أخطاء فك التشفير.
- المساعدون Norito يشاركون بين مجموعات SDK ويسمحون بإعادة استخدام أنواع نماذج البيانات الموجودة.
- Torii وعلامة الجلسات Norito على القياس عن بعد، ويمكن للمشغلين مرافقتهم مع لوحات المعلومات الخاصة بهم.

## ما هو المطلوب

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. قم بإجراء تسلسل للحمولة النافعة الخاصة بك عبر برنامج الترميز Norito (`iroha_client`، والمساعدون في SDK أو `norito::to_bytes`).
2. قم بالتحقق من المتطلبات مع `Content-Type: application/x-norito`.
3. اطلب الرد Norito باستخدام `Accept: application/x-norito`.
4. قم بفك التشفير من خلال مساعد مراسل SDK.

دليل SDK:
- **الصدأ**: `iroha_client::Client` negocia Norito تلقائيًا عندما يحدد الصوت رأس `Accept`.
- **بايثون**: استخدم `NoritoRpcClient` من `iroha_python.norito_rpc`.
- **Android**: استخدم `NoritoRpcClient` و`NoritoRpcRequestOptions` بدون SDK Android.
- **JavaScript/Swift**: المساعدون موجودون في `docs/source/torii/norito_rpc_tracker.md` ويتم تشغيلهم كجزء من NRPC-3.

## مثال على وحدة التحكم جربها

تشتمل البوابة المصممة على وكيل Try It حتى تتمكن من إعادة إنتاج الحمولات النافعة Norito دون فك البرامج النصية على طول.

1. [بدء الوكيل](./try-it.md#start-the-proxy-locally) وتحديد `TRYIT_PROXY_PUBLIC_URL` حتى تتمكن الأدوات من إرسال حركة المرور.
2. استخدم البطاقة **جربها** في الصفحة أو اللوحة `/reference/torii-swagger` واختر نقطة النهاية مثل `POST /v2/pipeline/submit`.
3. قم بتعديل **Content-Type** لـ `application/x-norito`، ثم اختر محرر **Binary** وقم بإرسال `fixtures/norito_rpc/transfer_asset.norito` (أو أي حمولة مدرجة في `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. احصل على الرمز المميز لحامله عبر رمز جهاز OAuth أو دليل الاستخدام (يتجاوز استخدام الوكيل `X-TryIt-Auth` عندما يتم تكوينه مع `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. أرسل الطلب وتحقق مما إذا كان Torii صديقًا أو `schema_hash` مدرجًا في `fixtures/norito_rpc/schema_hashes.json`. تؤكد التجزئة أن الاسم Norito يتم تشغيله في المتصفح/الوكيل.

للحصول على أدلة خريطة الطريق، قم بدمج لقطة الشاشة التي تقوم بها Try It مع تنفيذ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. يشتمل البرنامج النصي على `cargo xtask norito-rpc-verify`، ثم قم بإخراج JSON من `artifacts/norito_rpc/<timestamp>/` والتقط التركيبات نفسها التي تستهلكها البوابة.

## حل المشاكل

| سينتوما | على السطح | سبب إثبات | كوريكاو |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | الرد على Torii | الرأس `Content-Type` مفتوح أو غير صحيح | قم بتعريف `Content-Type: application/x-norito` قبل إرسال الحمولة. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | جسم/رؤوس الرد Torii | تجزئة مخطط دوس، تركيبات مختلفة، بناء Torii | قم بإعادة إنشاء التركيبات عبر `cargo xtask norito-rpc-fixtures` وقم بتأكيد التجزئة على `fixtures/norito_rpc/schema_hashes.json`؛ استخدم JSON الاحتياطي إذا كانت نقطة النهاية غير مؤهلة Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Resposta do proxy جربه | مطلوب مشاهدة الأصل الموجود في `TRYIT_PROXY_ALLOWED_ORIGINS` | قم بإضافة البوابة الأصلية (على سبيل المثال، `https://docs.devnet.sora.example`) إلى بيئة محيطة مختلفة وتنشيط الوكيل. |
| `{"error":"rate_limited"}` (HTTP 429) | Resposta do proxy جربه | تتجاوز تكلفة IP الميزانية `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | زيادة الحد الأقصى للشحن الداخلي أو توقع إعادة الشحن (راجع `retryAfterMs` في الرد JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) أو `{"error":"upstream_error"}` (HTTP 502) | Resposta do proxy جربه | Torii انتهاء الصلاحية أو الوكيل دون الحاجة إلى تكوين الواجهة الخلفية | تحقق من إمكانية الوصول إلى `TRYIT_PROXY_TARGET`، وتأكد من Torii أو قم بالتحديث باستخدام `TRYIT_PROXY_TIMEOUT_MS` الأكبر. |

قم بإجراء تشخيصات Try It وقم بإدراج OAuth في [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## الموارد الإضافية
- RFC للنقل: `docs/source/torii/norito_rpc.md`
- الملخص التنفيذي: `docs/source/torii/norito_rpc_brief.md`
- تعقب البيانات: `docs/source/torii/norito_rpc_tracker.md`
- يقوم Instrucoes بإجراء تجربة الوكيل: `docs/portal/docs/devportal/try-it.md`