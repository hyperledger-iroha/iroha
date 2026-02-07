---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/observability.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مراقبة وتحليلات البوابة

تتطلب خريطة الطريق DOCS-SORA التحليلات والمسبارات التركيبية وأتمتة الامتيازات
الحالات من أجل كل بناء دي المعاينة. Cette note documente la plomberie livre avec le portail afin
يمكن للمشغلين فرع المراقبة دون الكشف عن بيانات الزوار.

## وضع العلامات على الإصدار

- تحديد `DOCS_RELEASE_TAG=<identifier>` (الرجوع إلى `GIT_COMMIT` أو `dev`) عند
  بناء دو بورتال. يتم حقن القيمة في `<meta name="sora-release">`
  لتمييز المسابير ولوحات المعلومات عن عمليات النشر.
- `npm run build` النوع `build/release.json` (القيمة المكتوبة
  `scripts/write-checksums.mjs`) الذي يصف العلامة والطابع الزمني وما إلى ذلك
  خيار `DOCS_RELEASE_SOURCE`. يتم إغلاق ملف meme في معاينة القطع الأثرية وما إلى ذلك
  مرجع على قدم المساواة لو علاقة دو مدقق الارتباط.

## تحليلات تحترم الحياة الخاصة

- المهيئ `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` للتنشيط
  le Tracker Leger. تحتوي الحمولات على `{ event, path, locale, release, ts }`
  بدون البيانات الوصفية المرجعية أو IP، و`navigator.sendBeacon` يستخدم ما هو ممكن
  لتجنب حجب التنقلات.
- جهاز التحكم بالكهرباء مع `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). الحفاظ على تعقب
  المسار الأخير مبعوث ولا يوجد عدد من الأحداث المزدوجة للملاحة الميمية.
- تم العثور على التنفيذ في `src/components/AnalyticsTracker.jsx` وهو قيد التنفيذ
  العولمة عبر `src/theme/Root.js`.

## مجسات صناعية

- `npm run probe:portal` emet des requetes GET sur des المسارات الرئيسية
  (`/`، `/norito/overview`، `/reference/torii-swagger`، وما إلى ذلك) وتحقق من ذلك
  العلامة الوصفية `sora-release` تتوافق مع `--expect-release` (ou `DOCS_RELEASE_TAG`).
  مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

تعتبر هذه التأثيرات مطابقة للمسار، مما يسهل بوابة القرص المضغوط على نجاح المسبار.

## أتمتة حالات الامتيازات

- `npm run check:links` scanne `build/sitemap.xml`، تأكد من أنه يمكنك كل إدخال من الخريطة إلى الأمم المتحدة
  ملف محلي (والتحقق من الاحتياطيات `index.html`)، وكتابة
  يحتوي `build/link-report.json` على البيانات التعريفية للإصدار، والمجموعات، والمؤثرات، وما إلى ذلك
  يتم تشغيل SHA-256 de `checksums.sha256` (يُعرض باسم `manifest.id`) لكل ما تحتاجه
  علاقة puisse etre ratache au maniste d'artefact.
- يقوم البرنامج النصي بإرجاع رمز غير صفري عند توقف الصفحة، حتى يتمكن CI من حظره
  الإصدارات على المسارات القديمة أو الأشرطة. تشير التقارير إلى مسارات المرشحين
  هذه هي الطريقة التي تساعد في تتبع تراجعات التوجيه حتى شجرة المستندات.

## لوحة المعلومات Grafana والتنبيهات

- `dashboards/grafana/docs_portal.json` نشر اللوحة Grafana **نشر بوابة المستندات**.
  يحتوي على الألواح التالية:
  - *رفض البوابة (5 م)* استخدم النطاق `torii_sorafs_gateway_refusals_total` على قدم المساواة
    `profile`/`reason` حتى يتمكن SRE من اكتشاف الدفعات السياسية غير الصحيحة أو
    echecs دي الرموز.
  - *نتائج تحديث ذاكرة التخزين المؤقت للاسم المستعار* و *الاسم المستعار لإثبات العمر ص90* التالي
    `torii_sorafs_alias_cache_*` لإثبات وجود البراهين الطازجة قبل القطع
    عبر DNS.
  - *عدد بيانات بيان التسجيل* والإحصائيات *عدد الأسماء المستعارة النشطة* يعكس
    تراكم السجل السري وإجمالي الأسماء المستعارة حتى تتمكن الإدارة من تدقيق الحسابات
    الافراج عن تشاك.
  - *انتهاء صلاحية بوابة TLS (ساعات)* قبل اقتراب انتهاء صلاحية شهادة TLS du
    بوابة النشر (seuil d'alerte a 72 h).
  - *نتائج النسخ المتماثل لجيش تحرير السودان* و*تراكم النسخ المتماثل* يراقب القياس عن بعد
    `torii_sorafs_replication_*` لضمان احترام جميع النسخ المتماثلة
    نيفو GA بعد النشر.
- استخدم متغيرات القالب المتكاملة (`profile`، `reason`) لتركيزك على
  ملف تعريف النشر `docs.sora` أو ابحث عن صور مجموعة البوابات.
- يستخدم توجيه PagerDuty لوحات لوحة القيادة كميزة أولية: التنبيهات
  `DocsPortal/GatewayRefusals`، `DocsPortal/AliasCache` و`DocsPortal/TLSExpiry`
  يتم إيقافه عند توقف المسلسل المراسل عن العمل. Liez le runbook de
  قم بتنبيه هذه الصفحة حتى يتمكن المتصل من تجديد الطلبات Prometheus بدقة.

## المجمع كله

1. قلادة `npm run build`، تحدد متغيرات بيئة الإصدار/التحليلات وما إلى ذلك
   اترك الشريط بعد البناء `checksums.sha256`، `release.json` وآخرون
   `link-report.json`.
2. المنفذ `npm run probe:portal` لمكافحة المعاينة الساخنة مع
   `--expect-release` فرع على علامة meme. قم بحفظ الإعدادات لقائمة التحقق
   دي النشر.
3. قم بتنفيذ `npm run check:links` للتكرار السريع لمدخلات ملفات خريطة الموقع
   ويتم أرشفة علاقة JSON مع معاينة القطع الأثرية. La CI إيداع جنيه
   أحدث علاقة في `artifacts/docs_portal/link-report.json` من أجل الحكم
   يمكنك تنزيل حزمة Preuves مباشرة من سجلات الإنشاء.
4. توجيه تحليلات نقطة النهاية إلى جامعتك التي تحترم الحياة الخاصة (معقول،
   تقوم OTEL باستقبال الرحلات الجوية، وما إلى ذلك) وتتأكد من أن مستندات echantillonnage هي وثائق
   قم بالإصدار حتى تتمكن لوحات المعلومات من تفسير وحدات التخزين بشكل صحيح.
5. قم بتمرير كابل CI عبر معاينة/نشر سير العمل
   (`.github/workflows/docs-portal-preview.yml`،
   `.github/workflows/docs-portal-deploy.yml`)، لا تقم بتغطية المسارات الجافة المحلية
   ما هو السلوك المحدد للأسرار.