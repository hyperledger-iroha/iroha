---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# افتح Norito-RPC

Norito-RPC هو النقل الثنائي لـ API Torii. سيتم إعادة استخدام ميمات HTTP التي `/v2/pipeline` بالإضافة إلى تبادل الرسوم المضمنة على أساس Norito والتي تتضمن تجزئات المخطط والمجاميع الاختبارية. استخدمه عندما تحتاج إلى إجابات محددة وصحيحة أو عندما تؤدي إجابات JSON لخط الأنابيب إلى تشابك.

## بوركوي المغير؟
- مجموعة محددة مع CRC64 وتجزئات المخطط تقلل من أخطاء فك التشفير.
- تسمح لك مشاركة المساعدين Norito بين SDK بإعادة استخدام الأنواع الموجودة في نماذج البيانات.
- Torii تم الانتهاء من الجلسات Norito في القياس عن بعد، ويمكن للمشغلين متابعة اعتماد لوحات المعلومات المتوفرة.

## Faire une requete

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. قم بتسلسل الحمولة مع برنامج الترميز Norito (`iroha_client`، SDK المساعد أو `norito::to_bytes`).
2. أرسل الطلب مع `Content-Type: application/x-norito`.
3. اطلب الرد Norito عبر `Accept: application/x-norito`.
4. قم بفك تشفير الاستجابة باستخدام مساعد SDK المتوافق.

النصائح على قدم المساواة SDK:
- **الصدأ**: `iroha_client::Client` negocie Norito يتم تلقائيًا عند تحديد `Accept`.
- **بايثون**: استخدم `NoritoRpcClient` من `iroha_python.norito_rpc`.
- **Android**: استخدم `NoritoRpcClient` و`NoritoRpcRequestOptions` في SDK Android.
- **JavaScript/Swift**: المساعدون موجودون في `docs/source/torii/norito_rpc_tracker.md` ويصلون في NRPC-3.

## مثال على وحدة التحكم جربها

يوفر تطوير البوابة وكيلًا Try It حتى يتمكن الباحثون من تجديد الحمولات Norito دون كتابة نصوص برمجية على القياس.

1. [اكتشف الوكيل](./try-it.md#start-the-proxy-locally) وحدد `TRYIT_PROXY_PUBLIC_URL` لتضمين الأدوات أو ترسل حركة المرور.
2. افتح البطاقة **Try it** على هذه الصفحة أو اللوحة `/reference/torii-swagger` واختر نقطة نهاية مثل `POST /v2/pipeline/submit`.
3. قم بتمرير **Content-Type** إلى `application/x-norito`، ثم اختر محرر **Binary** وشحن `fixtures/norito_rpc/transfer_asset.norito` (أو كل قائمة الحمولة في `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. قم بتوفير رمز مميز لحامله عبر رمز جهاز OAuth أو الدليل اليدوي (يقبل الوكيل التجاوزات `X-TryIt-Auth` عندما يتم تكوينه مع `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. قم بإرسال الطلب والتحقق من أن Torii قام بمراجعة القائمة `schema_hash` في `fixtures/norito_rpc/schema_hashes.json`. تؤكد التجزئات المعرّفة على Norito من خلال التنقل/الوكيل.

من أجل خريطة طريق الأدلة، قم بإقران التقاط الشاشة Try It بتنفيذ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. يحتوي البرنامج النصي على `cargo xtask norito-rpc-verify`، ويكتب استئناف JSON في `artifacts/norito_rpc/<timestamp>/`، ويلتقط الميمات الثابتة التي تنقلها إلى المستهلكين.

## ديباناج

| الأعراض | Ou cela apparait | السبب المحتمل | تصحيح |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | الرد Torii | En-tete `Content-Type` manquant ou true | قم بتعريف `Content-Type: application/x-norito` قبل إرسال الحمولة. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Corps/en-tetes de reponse Torii | تجزئة مخطط التركيبات المختلفة للبناء Torii | قم بإعادة إنشاء التركيبات باستخدام `cargo xtask norito-rpc-fixtures` وتأكد من التجزئة في `fixtures/norito_rpc/schema_hashes.json`؛ أعد تمرير JSON إذا لم تعد نقطة النهاية نشطة Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | رد الوكيل جربه | الطلب المقدم من أصل غير مدرج في `TRYIT_PROXY_ALLOWED_ORIGINS` | قم بإضافة أصل الباب (على سبيل المثال `https://docs.devnet.sora.example`) إلى متغير البيئة وإعادة تشغيل الوكيل. |
| `{"error":"rate_limited"}` (HTTP 429) | رد الوكيل جربه | تجاوز حصة IP للميزانية `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | قم بزيادة الحد الأقصى لاختبارات الشحن الداخلي أو قم بإعادة تهيئة النافذة (انظر `retryAfterMs` في استجابة JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) أو `{"error":"upstream_error"}` (HTTP 502) | رد الوكيل جربه | Torii انتهاء صلاحية الوكيل أو عدم تمكنك من الوصول إلى تكوين الواجهة الخلفية | تحقق من إمكانية الوصول إلى `TRYIT_PROXY_TARGET`، وتحكم في صحة Torii أو أعد الكتابة باستخدام `TRYIT_PROXY_TIMEOUT_MS` بالإضافة إلى أحد عشر. |

بالإضافة إلى تشخيصات Try It ونصائح OAuth التي تم العثور عليها في [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## الموارد التكميلية
- نقل RFC: `docs/source/torii/norito_rpc.md`
- السيرة الذاتية التنفيذية: `docs/source/torii/norito_rpc_brief.md`
- متتبع الحركة: `docs/source/torii/norito_rpc_tracker.md`
- تعليمات du proxy Try-It: `docs/portal/docs/devportal/try-it.md`