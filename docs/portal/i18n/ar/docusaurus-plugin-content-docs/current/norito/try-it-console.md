---
lang: ar
direction: rtl
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: وحدة Try-It لـ Norito
description: استخدم وكيل بوابة المطورين وأدوات Swagger وRapiDoc لإرسال طلبات Torii / Norito-RPC الحقيقية مباشرة من موقع الوثائق.
---

تجمع البوابة ثلاث واجهات تفاعلية تعيد توجيه الحركة إلى Torii:

- **Swagger UI** عند `/reference/torii-swagger` تعرض مواصفات OpenAPI الموقعة وتعيد كتابة الطلبات عبر الوكيل تلقائيا عندما يتم ضبط `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** عند `/reference/torii-rapidoc` تعرض المخطط نفسه مع رفع الملفات ومحددات نوع المحتوى التي تعمل جيدا مع `application/x-norito`.
- **Try it sandbox** في صفحة نظرة عامة على Norito يوفر نموذجا خفيفا لطلبات REST المخصصة وتسجيلات الدخول عبر جهاز OAuth.

ترسل الأدوات الثلاث الطلبات إلى **وكيل Try-It** المحلي (`docs/portal/scripts/tryit-proxy.mjs`). يتحقق الوكيل من أن `static/openapi/torii.json` يطابق البصمة الموقعة في `static/openapi/manifest.json`، ويفرض محدد معدل، وينقح رؤوس `X-TryIt-Auth` في السجلات، ويضع علامة على كل نداء باتجاه المنبع بواسطة `X-TryIt-Client` لكي يتمكن مشغلو Torii من تدقيق مصادر الحركة.

## تشغيل الوكيل

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` هو عنوان Torii الأساسي الذي تريد اختباره.
- `TRYIT_PROXY_ALLOWED_ORIGINS` يجب أن يتضمن كل أصل للبوابة (خادم محلي، اسم مضيف الإنتاج، عنوان معاينة) ينبغي أن يضم وحدة التحكم.
- `TRYIT_PROXY_PUBLIC_URL` تستهلكه `docusaurus.config.js` ويتم حقنه في الأدوات عبر `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` يتم تحميله فقط عندما يكون `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`؛ وإلا يجب على المستخدمين توفير رمزهم عبر وحدة التحكم أو تدفق جهاز OAuth.
- `TRYIT_PROXY_CLIENT_ID` يضبط علامة `X-TryIt-Client` المحمولة في كل طلب.
  السماح بإرسال `X-TryIt-Client` من المتصفح متاح لكن القيم يتم قصها
  ويتم رفضها إذا احتوت على محارف تحكم.

عند بدء التشغيل ينفذ الوكيل `verifySpecDigest` وينهي التنفيذ مع تلميح للمعالجة إذا كان البيان قديما. شغل `npm run sync-openapi -- --latest` لتنزيل أحدث مواصفات Torii أو مرر `TRYIT_PROXY_ALLOW_STALE_SPEC=1` لتجاوزات الطوارئ.

لتحديث هدف الوكيل أو التراجع عنه بدون تعديل ملفات البيئة يدويا، استخدم الأداة المساعدة:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## توصيل الأدوات

شغل البوابة بعد أن يبدأ الوكيل بالاستماع:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

يعرض `docusaurus.config.js` الإعدادات التالية:

| المتغير | الغرض |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | عنوان URL محقون في Swagger وRapiDoc وsandbox Try it. اتركه غير مضبوط لإخفاء الأدوات أثناء المعاينات غير المصرح بها. |
| `TRYIT_PROXY_DEFAULT_BEARER` | رمز افتراضي اختياري محفوظ في الذاكرة. يتطلب `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` وحماية CSP الخاصة بـ HTTPS فقط (DOCS-1b) ما لم تمرر `DOCS_SECURITY_ALLOW_INSECURE=1` محليا. |
| `DOCS_OAUTH_*` | يفعّل تدفق جهاز OAuth (`OAuthDeviceLogin` component) حتى يتمكن المراجعون من إصدار رموز قصيرة الأجل دون مغادرة البوابة. |

عند توفر متغيرات OAuth يعرض الـ sandbox زر **Sign in with device code** الذي يمر عبر خادم Auth المضبوط (راجع `config/security-helpers.js` للشكل الدقيق). يتم تخزين الرموز الصادرة عبر تدفق الجهاز في جلسة المتصفح فقط.

## إرسال حمولات Norito-RPC

1. أنشئ حمولة `.norito` باستخدام CLI أو المقاطع الموضحة في [البدء السريع لـ Norito](./quickstart.md). الوكيل يعيد توجيه أجسام `application/x-norito` دون تغيير، لذا يمكنك إعادة استخدام الأثر نفسه الذي سترسله عبر `curl`.
2. افتح `/reference/torii-rapidoc` (مفضل للحمولات الثنائية) أو `/reference/torii-swagger`.
3. اختر لقطة Torii المطلوبة من القائمة المنسدلة. اللقطات موقعة؛ وتعرض اللوحة بصمة البيان المسجلة في `static/openapi/manifest.json`.
4. اختر نوع المحتوى `application/x-norito` في درج "Try it"، واضغط **Choose File**، وحدد الحمولة الخاصة بك. يعيد الوكيل كتابة الطلب إلى `/proxy/v2/pipeline/submit` ويضع وسم `X-TryIt-Client=docs-portal-rapidoc`.
5. لتنزيل استجابات Norito اضبط `Accept: application/x-norito`. تعرض Swagger/RapiDoc محدد الرؤوس في الدرج نفسه وتعيد الباينري عبر الوكيل.

بالنسبة للمسارات التي تستخدم JSON فقط، يكون sandbox Try it المضمّن غالبا أسرع: أدخل المسار (على سبيل المثال `/v2/accounts/i105.../assets`)، اختر طريقة HTTP، الصق جسم JSON عند الحاجة، واضغط **Send request** لفحص الرؤوس والمدة والحمولات مباشرة.

## استكشاف الأخطاء وإصلاحها

| العرض | السبب المحتمل | المعالجة |
| --- | --- | --- |
| وحدة التحكم في المتصفح تظهر أخطاء CORS أو يحذر sandbox من أن عنوان الوكيل مفقود. | الوكيل غير قيد التشغيل أو الأصل غير مدرج في القائمة المسموحة. | شغل الوكيل، وتأكد من أن `TRYIT_PROXY_ALLOWED_ORIGINS` يغطي مضيف البوابة، ثم أعد تشغيل `npm run start`. |
| ينتهي `npm run tryit-proxy` برسالة “digest mismatch”. | حزمة OpenAPI الخاصة بـ Torii تغيرت في المنبع. | شغل `npm run sync-openapi -- --latest` (أو `--version=<tag>`) ثم أعد المحاولة. |
| الأدوات تعيد `401` أو `403`. | الرمز مفقود أو منتهي أو يمتلك صلاحيات غير كافية. | استخدم تدفق جهاز OAuth أو الصق رمز bearer صالحا في sandbox. للرموز الثابتة يجب تصدير `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` من الوكيل. | تم تجاوز حد المعدل لكل IP. | ارفع `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` للبيئات الموثوقة أو خفف من نصوص الاختبار. جميع حالات الرفض بسبب حد المعدل تزيد `tryit_proxy_rate_limited_total`. |

## قابلية المراقبة

- `npm run probe:tryit-proxy` (غلاف حول `scripts/tryit-proxy-probe.mjs`) يستدعي `/healthz`، وقد يختبر مسارا نموذجيا، ويولد ملفات نصية لـ Prometheus من أجل `probe_success` / `probe_duration_seconds`. اضبط `TRYIT_PROXY_PROBE_METRICS_FILE` لدمجه مع node_exporter.
- اضبط `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` لكشف العدادات (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) ومخططات الكمون. تقرأ لوحة `dashboards/grafana/docs_portal.json` هذه المقاييس لفرض SLOs الخاصة بـ DOCS-SORA.
- سجلات وقت التشغيل على stdout. كل إدخال يتضمن معرف الطلب وحالة المنبع ومصدر المصادقة (`default` أو `override` أو `client`) والمدة؛ ويتم تنقيح الأسرار قبل الإخراج.

إذا احتجت إلى التحقق من أن حمولات `application/x-norito` تصل إلى Torii دون تغيير، شغل مجموعة اختبارات Jest (`npm test -- tryit-proxy`) أو افحص الـ fixtures تحت `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. تغطي اختبارات الانحدار ثنائيات Norito المضغوطة وبيانات OpenAPI الموقعة ومسارات تخفيض الوكيل بحيث تحافظ عمليات طرح NRPC على مسار أدلة دائم.
