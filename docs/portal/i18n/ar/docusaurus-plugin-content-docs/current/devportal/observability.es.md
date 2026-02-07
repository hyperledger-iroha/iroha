---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/observability.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إمكانية المراقبة والتحليل للبوابة

تتطلب خريطة الطريق DOCS-SORA التحليلات والمسبارات الاصطناعية وأتمتة عمليات التثبيت الدوارة
لكل بناء المعاينة. هذه ملاحظة مستندة إلى البوابة الآن
حتى يتمكن المشغلون من الاتصال بالمراقبة دون تصفية بيانات الزوار.

## آداب الإصدار

- تحديد `DOCS_RELEASE_TAG=<identifier>` (الرجوع إلى `GIT_COMMIT` أو `dev`)
  بناء البوابة. يتم إدخال الشجاعة في `<meta name="sora-release">`
  لتمييز المسابير ولوحات المعلومات عن التباينات.
- `npm run build` تنبعث `build/release.json` (الكاتب
  `scripts/write-checksums.mjs`) يصف العلامة والطابع الزمني والعلامة
  `DOCS_RELEASE_SOURCE` اختياري. يتم تغليف نفس الأرشيف في المصنوعات اليدوية للمعاينة
  يتم الرجوع إلى تقرير مدقق الارتباط.

## Analitica مع الحفاظ على الخصوصية

- تكوين `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` الفقرة
  تأهيل المقتفي ليفيانو. تحتوي الحمولات النافعة على `{ حدث، مسار، لغة،
  الإصدار، TS }` sin metadata de referrer o IP, y se usa `navigator.sendBeacon`
  يمكنك دائمًا تجنب حظر التنقلات.
- التحكم في المحرك باستخدام `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). حارس التعقب
  تم إرسال المسار الأخير ولن يصدر أي أحداث مكررة لنفس التنقل.
- التنفيذ حي في `src/components/AnalyticsTracker.jsx` وهو موجود
  عالميًا عبر `src/theme/Root.js`.

## مجسات سينتيتيكوس

- `npm run probe:portal` يصدر طلبات GET contra rutas comunes
  (`/`، `/norito/overview`، `/reference/torii-swagger`، وما إلى ذلك) والتحقق من ذلك
  العلامة الوصفية `sora-release` تتزامن مع `--expect-release` (o
  `DOCS_RELEASE_TAG`). مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

يتم الإبلاغ عن السقوط من خلال المسار، مما يسهل عليك تشغيل القرص المضغوط بنتيجة التحقيق.

## Automatizacion de enlaces rotos

- `npm run check:links` من خلال `build/sitemap.xml`، تأكد من أن كل شخص يدخل يرسم خريطة واحدة
  أرشيف محلي (الاختيارات الاحتياطية `index.html`)، واكتب
  `build/link-report.json` مع البيانات الوصفية للإصدار والإجمالي والسقوط وبصمة الإصبع
  SHA-256 de `checksums.sha256` (يُطرح مثل `manifest.id`) لكل تقرير
  إذا كان من الممكن أن تظهر القطعة الأثرية.
- يتم بيع النص باستخدام رمز مميز عند ترك صفحة واحدة، حتى تتمكن من CI
  قم بحظر الإصدارات في المسارات القديمة أو الدوارة. تشير التقارير إلى المرشحين المحتملين
  إذا كنت تقصد، فإن المساعدة في التراجعات النقطية للتوجيه ستؤدي إلى تضييق نطاق المستندات.

## لوحة المعلومات والتنبيهات Grafana

- `dashboards/grafana/docs_portal.json` نشر الطاولة Grafana **نشر بوابة المستندات**.
  تشمل اللوحات التالية:
  - *رفض البوابة (5 م)* نطاق usa `torii_sorafs_gateway_refusals_total`
    `profile`/`reason` لكي يقوم SRE باكتشاف الدفعات السياسية السيئة أو فشل الرموز المميزة.
  - *نتائج تحديث ذاكرة التخزين المؤقت للاسم المستعار* و *الاسم المستعار لإثبات العمر ص90*
    `torii_sorafs_alias_cache_*` لإظهار ما إذا كانت هناك بروفات جدارية قبل القطع
    عبر دي DNS.
  - *عدد بيانات بيان التسجيل* والإحصائيات *عدد الأسماء المستعارة النشطة* راجع القائمة
    تراكم سجل الدبوس والأسماء المستعارة الإجمالية حتى تتمكن من التدقيق
    الافراج عن كادا.
  - *انتهاء صلاحية بوابة TLS (ساعات)* عند إزالة شهادة TLS من بوابة النشر
    يتم الوصول إلى النار (ظلة التنبيه لمدة 72 ساعة).
  - *نتائج النسخ المتماثل لاتفاقية مستوى الخدمة* و *تراكم النسخ المتماثل* لمراقبة القياس عن بعد
    `torii_sorafs_replication_*` للتأكد من أن جميع النسخ المتماثلة ستكتمل
    مستوى GA بعد النشر.
- استخدام متغيرات النباتات المتكاملة (`profile`، `reason`) للعرض على الكمبيوتر
  ملف نشر `docs.sora` أو استكشاف الصور في جميع البوابات.
- يستخدم توجيه PagerDuty لوحات لوحة القيادة مثل الأدلة: التنبيهات
  `DocsPortal/GatewayRefusals`، `DocsPortal/AliasCache` و`DocsPortal/TLSExpiry`
  Disparan cuando la serie concedente Cruza su umbral. قم بتشغيل دليل التنبيهات
  هذه الصفحة حتى تتمكن عند الطلب من إعادة إنتاج الاستعلامات الدقيقة لـ Prometheus.

## Poniendolo بالاقتران

1. Durante `npm run build`، حدد المتغيرات أثناء الإصدار/التحليل
   هذا يعني أن ما بعد البناء يصدر `checksums.sha256`, `release.json` y
   `link-report.json`.
2. قم بتشغيل `npm run probe:portal` مقابل معاينة اسم المضيف
   `--expect-release` متصل بنفس العلامة. احرص على وضع قائمة مرجعية للنشر.
3. قم بتشغيل `npm run check:links` للدخول بسرعة إلى خريطة الموقع والأرشيف
   تم إنشاء تقرير JSON جنبًا إلى جنب مع عناصر المعاينة. CI deja el ultimo reporte en
   `artifacts/docs_portal/link-report.json` حتى تتمكن من تنزيل حزمة الأدلة
   مباشرة من سجلات البناء.
4. قم بإدخال نقطة نهاية التحليل الخاصة بك مع الحفاظ على الخصوصية (معقول،
   تستوعب OTEL استضافة ذاتية، وما إلى ذلك) وتتأكد من أن أعمالها تستحق التوثيق
   قم بالتحرير حتى تقوم لوحات المعلومات بتفسير المحتويات بشكل صحيح.
5. قم بتوصيل هذه الخطوات في سير عمل المعاينة/النشر
   (`.github/workflows/docs-portal-preview.yml`،
   `.github/workflows/docs-portal-deploy.yml`)، لأن المناطق التي تعمل بالتشغيل الجاف ضرورية فقط
   يستكشف سلوكًا خاصًا بالأسرار.