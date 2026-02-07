---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/observability.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بوابة المراقبة والتحليلات

تتطلب خريطة الطريق DOCS-SORA تحليلات ومسبارات مزامنة وارتباطات آلية
لكل بناء المعاينة. تشير هذه الوثيقة إلى البنية التحتية التي ترافقها البوابة الآن
لكي يقوم المشغلون بتوصيل المراقبة دون متابعة بيانات الزوار.

## وضع العلامات على الإصدار

- تعريف `DOCS_RELEASE_TAG=<identifier>` (احتياطي لـ `GIT_COMMIT` أو `dev`)
  بناء يا بوابة. يا لها من بسالة وقوة في `<meta name="sora-release">`
  من أجل إجراء تحقيقات ولوحات المعلومات وعمليات النشر المختلفة.
- `npm run build` تنبعث `build/release.json` (الكاتب
  `scripts/write-checksums.mjs`) يقوم بفك العلامة والطابع الزمني والملف
  `DOCS_RELEASE_SOURCE` اختياري. نفس الملف وتحمل أعمال المعاينة الخاصة بنا
  Rereenciado pelo relatorio قم بمدقق الارتباط.

## التحليلات مع الحفاظ على الخصوصية

- تكوين `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` الفقرة
  تأهيل مستوى التعقب. الحمولات النافعة `{ event, path, locale, release, ts }`
  مجرد بيانات وصفية للإحالة أو IP، و`navigator.sendBeacon` واستخدامها دائمًا ما يكون ممكنًا
  لتجنب انسداد الملاحة.
- التحكم في أخذ العينات عبر `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). يا متتبع أرمازينا
  تم إرسال المسار الأخير ولن يتم إصدار أحداث مكررة إلا أثناء التنقل.
- تم التنفيذ في `src/components/AnalyticsTracker.jsx` و في المنتدى
  عالمي عبر `src/theme/Root.js`.

## مجسات سينتيتيكوس

- `npm run probe:portal` تطلب dispara GET contra rotas comuns
  (`/`، `/norito/overview`، `/reference/torii-swagger`، وما إلى ذلك) والتحقق من ذلك
  العلامة الوصفية `sora-release` تتوافق مع `--expect-release` (ou `DOCS_RELEASE_TAG`).
  مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Falhas ao Reportadas por path, facilitando getar o CD from the subscesso dos probes.

## الروابط التلقائية المهمة

- `npm run check:links` varre `build/sitemap.xml`، لضمان دخول كل خريطة إلى
  قم بإنشاء ملف محلي (اختيار العناصر الاحتياطية `index.html`)، ثم قم بالخروج
  `build/link-report.json` يتنافس على البيانات الوصفية للإصدار والإجمالي والخطأ والطباعة
  SHA-256 de `checksums.sha256` (عرض مثل `manifest.id`) لكل ما يمكن من العلاقة
  إنه مرتبط ببيان العمل الفني.
- عند انتهاء البرنامج النصي مع كوديغو ناو صفر عند فشل صفحة ما، يمكن حظر CI
  يتم إطلاقها في دوارات مضادة أو quebradas. يستشهد العلاقات بالمرشحين المرشحين،
  o ما يساعد على تراجعات الدوران النقطية لفتح المستندات.

## لوحة المعلومات Grafana والتنبيهات

- `dashboards/grafana/docs_portal.json` نشر اللوحة Grafana **نشر بوابة المستندات**.
  تتضمن الخطوات التالية:
  - *رفض البوابة (5 م)* usa `torii_sorafs_gateway_refusals_total` com escopo
    `profile`/`reason` حتى تتمكن SREs من اكتشاف عمليات التشويه السياسي أو أخطاء الرموز المميزة.
  - *نتائج تحديث ذاكرة التخزين المؤقت للاسم المستعار* e *الاسم المستعار إثبات العمر صفحة 90* مرافق
    `torii_sorafs_alias_cache_*` لإثبات وجود البراهين الحديثة قبل القطع
    عبر دي DNS.
  - *عدد بيانات بيان التسجيل* وإحصائيات *عدد الأسماء المستعارة النشطة* على سبيل المثال
    backlog do pin-registry ومجموع الأسماء المستعارة حتى تتمكن الإدارة من التدقيق
    الافراج عن كادا.
  - *انتهاء صلاحية بوابة TLS (ساعات)* عند بوابة النشر لشهادة TLS
    إذا اقتربت من عملية البيع (انتهى التنبيه لمدة 72 ساعة).
  - *نتائج النسخ المتماثل لاتفاقية مستوى الخدمة* e *تراكم النسخ المتماثل* مصاحب للقياس عن بعد
    `torii_sorafs_replication_*` لضمان أن جميع النسخ المتماثلة حاضرة
    patamar GA apos a publicacao.
- استخدم كنماذج متنوعة للقوالب (`profile`، `reason`) للتركيز على الملف الشخصي
  لنشر `docs.sora` أو استكشاف الصور في جميع البوابات.
- يتم استخدام توجيه PagerDuty من خلال لوحة المعلومات كدليل: التنبيهات
  `DocsPortal/GatewayRefusals`، `DocsPortal/AliasCache` و`DocsPortal/TLSExpiry`
  Disparam quando a serie مراسل Ultrapassa Seus Limiares. الدوري الفرنسي أو كتاب التشغيل
  قم بتنبيه هذه الصفحة حتى تكرر النصائح عند الطلب مثل الاستعلامات الواردة في Prometheus.

## جونتاندو تودو

1. Durante `npm run build`، تم تعريفه على أنه بيئة الإصدار/التحليلات المتنوعة
   هذا هو مصدر بناء نقطة البيع `checksums.sha256`، `release.json` e
   `link-report.json`.
2.Rode `npm run probe:portal` ضد اسم المضيف للمعاينة com
   `--expect-release` متصل بالعلامة نفسها. قم بحفظ النموذج القياسي لقائمة مرجعية للنشر.
3. ركب `npm run check:links` للسرعة في الدخول إلى خريطة الموقع والأرشفة
   يتم إنشاء علاقة JSON جنبًا إلى جنب مع عناصر المعاينة. إيداع CI o
   آخر علاقة بـ `artifacts/docs_portal/link-report.json` للحكم
   قم بإضافة حزمة من الأدلة مباشرة إلى سجلات الإنشاء.
4. قم بإجراء نقطة نهاية التحليلات لجامعتك مع الحفاظ على الخصوصية (معقول،
   تستوعب OTEL استضافة ذاتية، وما إلى ذلك) وتضمن أن ضرائب هذه الخدمة موثقة من قبل
   حرر حتى تفسر لوحات المعلومات وحدات التخزين بشكل صحيح.
5. سيتمكن CI من الاتصال بسير عمل المعاينة/النشر
   (`.github/workflows/docs-portal-preview.yml`،
   `.github/workflows/docs-portal-deploy.yml`)، قم بتشغيل التشغيل الجاف في مكانه بدقة شديدة
   كوبرير سلوك خاص بالعزلة.