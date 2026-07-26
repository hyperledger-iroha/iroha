---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/observability.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البوابة والتحليلات

بخارة طريق DOCS-SORA تتطلب تحليلات، ومسبارات اصطناعية، واتمتة الروابط المكسورة لكل بناء معاينة.
تتفوق هذه المزايا التي تأتي مع البوابة حتى تتمكن من المساهمة في جذب دون ما يمكن من بيانات الزوار.

##اتصال الاصدار

- اضبط `DOCS_RELEASE_TAG=<identifier>` (يرجع الى `GIT_COMMIT` او `dev` عند الغياب) عند
  بناء البوابة. يتم حقن القيمة في `<meta name="sora-release">`
  حتى المبتدئين تحقيقات ولوحات المعلومات من تمييز عمليات النشر.
- `npm run build` النتيجة `build/release.json` (يكتبه
  `scripts/write-checksums.mjs`) ويصف الوسم والطابعة الزمنيّة و`DOCS_RELEASE_SOURCE` الاختياري.
  يتم تضمين الملف نفسه في اثار المعاينة ويشار إليه في تقرير فحص الروابط.

## تحليلات تحافظ على الخصوصية

- اضبط `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` متابعة التتبع الخفيف.
  تحتوي الحمولات على `{ event, path, locale, release, ts }` دون بيانات وصفية مرجع او IP، وربما استخدام
  `navigator.sendBeacon` كلما امكن تعديل حجبات الطيران.
- المراقب في أخذ الهدف عبر `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). يخزن المتتبع اخر المسار مرسل ولا يرسل
  احداثا مكررة وبعضها .
- التنفيذ موجود في `src/components/AnalyticsTracker.jsx` ويتم تركيبه عالميًا عبر `src/theme/Root.js`.

## مجسات اصطناعية

- `npm run probe:portal` يرسل طلبات GET الى المسارات الشائعة
  (`/`, `/norito/overview`, `/reference/torii-swagger`, وغيرها) ويتحقق من ان اتصال
  `sora-release` يطابق `--expect-release` (او `DOCS_RELEASE_TAG`). مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

يتم الابلاغ عن الاخفاقات لكل مسار، ما نقوم ببناء بوابة CD على مجسات النجاح.

## اتمة الروابط المكسورة

- `npm run check:links` يفحص `build/sitemap.xml`، ويتأكد ان كل مدخل يطابق ملفا محليا
  (مع التحقق من الاحتياطيات `index.html`)، ويكتب `build/link-report.json` الذي يحتوي على
  البيانات الوصفية الاصدار، الاجمالي، الاخفاقات، وبصمة SHA-256 لـ `checksums.sha256`
  (معرضة كـ `manifest.id`) حتى يرتبط كل تقرير بمانيفست الاثر.
- السكربت يخرج بكود غير صفري عند اكتشاف صفحة، لذا يمكن لـ CI منع الاصدارات عند وجود مسارات قديمة او غير متوقعة
  مكسورة. التسجيل تذكر المسارات التي ساعدتنا، ما يساعد على تتبع مسارات الرحلة
  الى شجرة مستندات.

## لوحة Grafana والتنبيهات

- `dashboards/grafana/docs_portal.json` ينشر لوحة Grafana **نشر بوابة المستندات**.
  العلامات التالية:
  - *رفض البوابة (5m)* يستخدم `torii_sorafs_gateway_refusals_total` بنطاق
    `profile`/`reason` حتى يتمكن SRE من كشف الدفعات بناء على او فشل الرموز.
  - *نتائج تحديث ذاكرة التخزين المؤقت للاسم المستعار* و*الاسم المستعار لإثبات العمر ص90* تتبع
    `torii_sorafs_alias_cache_*` لا يوجد إثباتات حديثة قبل قطع DNS.
  - *عدد بيانات بيان التسجيل* واحصائية *عدد الأسماء المستعارة النشطة*
    واجمالي الاسم المستعار حتى البناء والتكامل من تدقيق كل اصدار.
  - *انتهاء بوابة TLS (ساعات)* يبرز بمجرد شهادة TLS لبوابة النشر من النهاية
    (عتبة التنبيه 72 ح).
  - *نتائج النسخ المتماثل لاتفاقية مستوى الخدمة* و*تراكم النسخ المتماثل* مراقبة القياس عن بعد
    `torii_sorafs_replication_*` ودائما ان كل النوى تحقق معايير GA بعد النشر.
- استخدم التنسيقات المبتكرة على موقع إلكتروني مدمج (`profile`, `reason`) للتركيز على ملف النشر `docs.sora`
  او التحقيق في الطفرات عبر كل البوابات.
- توجيه PagerDuty يستخدم لوحة القيادة كدليل: الإشعارات
  `DocsPortal/GatewayRefusals`، `DocsPortal/AliasCache`، و`DocsPortal/TLSExpiry`
  عندما تتجاوز السلاسل عتباتها. قم بتشغيل Runbook التنبيه بهذه اللحظة حتى ولو كان
  مهندسو عملية إعادة البناء عند الطلب هي علامات Prometheus الدقيقة.

##جمع الخطوات

1. أثناء `npm run build`، ضبط التنوعات البيئية والإصدار/التحليلات وداعًا خطوة ما بعد البناء
   `checksums.sha256`، `release.json`، و`link-report.json`.
2. الوظيفة `npm run probe:portal` مقابل اسم المضيف المعاينة مع
   `--expect-release` مربوطا بنفسي الوسم. احفظ stdout لقائمة نشر النشر.
3. الوظيفة `npm run check:links` للفشل السريع في واردات خريطة الموقع المكسورة واشف تقرير JSON الناتج
   مع اثار المعاينة. تقوم CI بوضع اخر تقرير في `artifacts/docs_portal/link-report.json` حتى البدء
   يتم تنزيل حزمة التعويضات مباشرة من خلال السجلات التأسيسية.
4. وجه تحليلات نقطة النهاية الى مجمع الحفاظ على الخصوصية (معقول، OTEL ingest مستضاف ذاتيا، وغيرها)
   وتأكد من تجهيزات العينة لكل اصدار حتى تفسر لوحات التحكم بالعدادات بدقة.
5. CI موضحة هذه الخطوة بالفعل عبر سير العمل والمعاينة/النشر
   (`.github/workflows/docs-portal-preview.yml`،
   `.github/workflows/docs-portal-deploy.yml`)، لذلك يكفي ان تغطي المعالجة المحلية
   منع الاسرار فقط.