---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# السيرة الذاتية لـ Norito-RPC

Norito-RPC هو النقل الثنائي لواجهات برمجة التطبيقات الخاصة بـ Torii. أعد استخدام نفس مسار HTTP الذي `/v1/pipeline` ولكنه يحتوي على شحنات متداخلة مرسوم عليها Norito تتضمن تجزئات الترميز والمجاميع الاختبارية. يتم الاستخدام عندما تتطلب الإجابات المحددة والتحقق من الصحة عندما تكون استجابات JSON لخط الأنابيب بمثابة زجاجة من الزجاجة.

## لماذا تتغير؟
- التحديد المحدد مع CRC64 وعلامات التجزئة تقلل من أخطاء فك التشفير.
- تتيح لك المساعدات Norito المشتركة بين مجموعات SDK إعادة استخدام أنواع نماذج البيانات الموجودة.
- Torii يقوم بضبط الجلسات Norito على القياس عن بعد، حيث يمكن للمشغلين مراقبة الاعتماد من خلال اللوحات المتوفرة.

## لنطلب منك طلبًا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. قم بتسلسل الحمولة باستخدام برنامج الترميز Norito (`iroha_client`، مساعدي SDK أو `norito::to_bytes`).
2. قم بإرسال الطلب إلى `Content-Type: application/x-norito`.
3. اطلب الرد Norito باستخدام `Accept: application/x-norito`.
4. قم بفك ترميز الرد باستخدام مساعد SDK المقابل.

دليل SDK:
- **الصدأ**: `iroha_client::Client` تفاوض Norito تلقائيًا عند تثبيت الرأس `Accept`.
- **بايثون**: الولايات المتحدة الأمريكية `NoritoRpcClient` من `iroha_python.norito_rpc`.
- **Android**: الولايات المتحدة الأمريكية `NoritoRpcClient` و`NoritoRpcRequestOptions` وSDK لنظام Android.
- **JavaScript/Swift**: يتم تحديد المساعدين في `docs/source/torii/norito_rpc_tracker.md` ويتم دمجهم كجزء من NRPC-3.

## مثال وحدة التحكم جربه

تشتمل بوابة المطورين على وكيل Try It حتى يتمكن المراجعون من إعادة إنتاج الحمولات النافعة Norito بدون كتابة البرامج النصية عبر الإنترنت.

1. [بدء الوكيل](./try-it.md#start-the-proxy-locally) وتحديد `TRYIT_PROXY_PUBLIC_URL` حتى تتمكن الأدوات من إرسال المرور.
2. افتح البطاقة **Try it** في هذه الصفحة أو اللوحة `/reference/torii-swagger` واختر نقطة نهاية مثل `POST /v1/pipeline/submit`.
3. قم بتغيير **Content-Type** إلى `application/x-norito`، ثم قم باختيار المحرر **Binary** والفرعي `fixtures/norito_rpc/transfer_asset.norito` (أي قائمة حمولة أخرى في `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. قم بتوفير رمز مميز لحامله عبر كود جهاز OAuth أو نطاق الرمز المميز (يقبل الوكيل `X-TryIt-Auth` عندما يتم تكوينه مع `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. أرسل طلبًا وتحقق مما إذا كان Torii يشير إلى `schema_hash` المدرجة في `fixtures/norito_rpc/schema_hashes.json`. تؤكد التجزئات المتزامنة أن Norito يغطي المتصفح/الوكيل.

لإثبات خريطة الطريق، قم بدمج لقطة الشاشة Try It مع تشغيل `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. يتضمن البرنامج النصي `cargo xtask norito-rpc-verify`، واكتب السيرة الذاتية JSON في `artifacts/norito_rpc/<timestamp>/`، والتقط نفس التركيبات التي تستهلك البوابة.

## حل المشاكل

| سينتوما | لا تظهر | السبب المحتمل | الحل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | الرد على Torii | خطأ أو خطأ في الرأس `Content-Type` | حدد `Content-Type: application/x-norito` قبل إرسال الحمولة. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | الجلد/رؤوس الاستجابة Torii | تجزئة التركيبات المتنوعة للبنية Torii | تركيبات Regenera مع `cargo xtask norito-rpc-fixtures` وتأكيد التجزئة على `fixtures/norito_rpc/schema_hashes.json`؛ الولايات المتحدة الأمريكية الاحتياطية JSON إذا كانت نقطة النهاية غير قادرة على Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | رد الوكيل جربه | طلب مقدم من مصدر غير مدرج في `TRYIT_PROXY_ALLOWED_ORIGINS` | قم بإضافة أصل البوابة (على سبيل المثال، `https://docs.devnet.sora.example`) إلى متغير المدخل وإعادة تشغيل الوكيل. |
| `{"error":"rate_limited"}` (HTTP 429) | رد الوكيل جربه | قطع IP يتجاوز شرط `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | قم بزيادة الحد لاختبار الشحن الداخلي أو التوقع حتى يتم إغلاق النافذة (راجع `retryAfterMs` في الرد JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) أو `{"error":"upstream_error"}` (HTTP 502) | رد الوكيل جربه | Torii منذ وقت طويل أو الوكيل لا يمكنك البحث عن تكوين الواجهة الخلفية | للتحقق من إمكانية الوصول إلى `TRYIT_PROXY_TARGET`، قم بمراجعة صحة Torii أو إعادة الاتصال بعمدة `TRYIT_PROXY_TIMEOUT_MS`. |

يتم تشغيل تشخيصات Try It ونصائح OAuth في [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## الموارد الإضافية
- RFC للنقل: `docs/source/torii/norito_rpc.md`
- السيرة الذاتية التنفيذية: `docs/source/torii/norito_rpc_brief.md`
- تعقب الأحداث: `docs/source/torii/norito_rpc_tracker.md`
- تعليمات الوكيل Try-It: `docs/portal/docs/devportal/try-it.md`