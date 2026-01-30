---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/observability.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# مراقبة البوابة والتحليلات

خارطة طريق DOCS-SORA تتطلب تحليلات، وprobes اصطناعية، واتمتة الروابط المكسورة لكل build معاينة.
توثق هذه الملاحظة التوصيلات التي تأتي مع البوابة حتى يتمكن المشغلون من ربط المراقبة دون تسريب بيانات الزوار.

## وسم الاصدار

- اضبط `DOCS_RELEASE_TAG=<identifier>` (يرجع الى `GIT_COMMIT` او `dev` عند الغياب) عند
  بناء البوابة. يتم حقن القيمة في `<meta name="sora-release">`
  حتى تتمكن probes وdashboards من تمييز عمليات النشر.
- `npm run build` ينتج `build/release.json` (يكتبه
  `scripts/write-checksums.mjs`) ويصف الوسم والطابع الزمني و`DOCS_RELEASE_SOURCE` الاختياري.
  يتم تضمين الملف نفسه في اثار المعاينة ويشار اليه في تقرير فحص الروابط.

## تحليلات تحافظ على الخصوصية

- اضبط `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` لتمكين المتتبع الخفيف.
  تحتوي الحمولات على `{ event, path, locale, release, ts }` دون metadata مرجع او IP، ويتم استخدام
  `navigator.sendBeacon` كلما امكن لتجنب حجب التنقلات.
- تحكم في اخذ العينات عبر `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). يخزن المتتبع اخر path مرسل ولا يرسل
  احداثا مكررة لنفس التنقل.
- التنفيذ موجود في `src/components/AnalyticsTracker.jsx` ويتم تركيبه عالميا عبر `src/theme/Root.js`.

## probes اصطناعية

- `npm run probe:portal` يرسل طلبات GET الى المسارات الشائعة
  (`/`, `/norito/overview`, `/reference/torii-swagger`, وغيرها) ويتحقق من ان وسم
  `sora-release` يطابق `--expect-release` (او `DOCS_RELEASE_TAG`). مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

يتم الابلاغ عن الاخفاقات لكل path، ما يسهل بوابة CD على نجاح probes.

## اتتمة الروابط المكسورة

- `npm run check:links` يفحص `build/sitemap.xml`، ويتاكد ان كل مدخل يطابق ملفا محليا
  (مع التحقق من fallbacks `index.html`)، ويكتب `build/link-report.json` الذي يحتوي على
  metadata الاصدار، الاجمالي، الاخفاقات، وبصمة SHA-256 لـ `checksums.sha256`
  (معروضة كـ `manifest.id`) حتى يرتبط كل تقرير بmanifest الاثر.
- السكربت يخرج بكود غير صفري عند فقدان صفحة، لذا يمكن لـ CI منع الاصدارات عند وجود مسارات قديمة او
  مكسورة. التقارير تذكر المسارات المرشحة التي تمت تجربتها، ما يساعد على تتبع تراجعات التوجيه
  الى شجرة docs.

## لوحة Grafana والتنبيهات

- `dashboards/grafana/docs_portal.json` ينشر لوحة Grafana **Docs Portal Publishing**.
  وتتضمن اللوحات التالية:
  - *Gateway Refusals (5m)* يستخدم `torii_sorafs_gateway_refusals_total` بنطاق
    `profile`/`reason` حتى يتمكن SRE من كشف دفعات سياسة خاطئة او فشل رموز.
  - *Alias Cache Refresh Outcomes* و*Alias Proof Age p90* تتبع
    `torii_sorafs_alias_cache_*` لاثبات وجود proofs حديثة قبل DNS cut over.
  - *Pin Registry Manifest Counts* واحصائية *Active Alias Count* تعكس backlog pin-registry
    واجمالي alias حتى تتمكن الحوكمة من تدقيق كل اصدار.
  - *Gateway TLS Expiry (hours)* يبرز عندما يقترب TLS cert لبوابة النشر من الانتهاء
    (عتبة التنبيه 72 h).
  - *Replication SLA Outcomes* و*Replication Backlog* تراقب telemetria
    `torii_sorafs_replication_*` لضمان ان كل النسخ تحقق معيار GA بعد النشر.
- استخدم متغيرات القالب المدمجة (`profile`, `reason`) للتركيز على ملف نشر `docs.sora`
  او التحقيق في الطفرات عبر كل البوابات.
- توجيه PagerDuty يستخدم لوحات dashboard كدليل: التنبيهات
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, و`DocsPortal/TLSExpiry`
  تطلق عندما تتجاوز السلاسل المقابلة عتباتها. اربط runbook التنبيه بهذه الصفحة حتى يتمكن
  مهندسو on-call من اعادة تشغيل استعلامات Prometheus الدقيقة.

## جمع الخطوات

1. اثناء `npm run build`، اضبط متغيرات بيئة release/analytics ودع خطوة ما بعد البناء تصدر
   `checksums.sha256`, `release.json`, و`link-report.json`.
2. شغل `npm run probe:portal` مقابل hostname المعاينة مع
   `--expect-release` مربوطا بنفس الوسم. احفظ stdout لقائمة نشر النشر.
3. شغل `npm run check:links` للفشل السريع على مدخلات sitemap المكسورة وارشف تقرير JSON الناتج
   مع اثار المعاينة. تقوم CI بوضع اخر تقرير في `artifacts/docs_portal/link-report.json` حتى تتمكن
   الحوكمة من تنزيل حزمة الادلة مباشرة من سجلات البناء.
4. وجه endpoint التحليلات الى مجمع يحافظ على الخصوصية (Plausible، OTEL ingest مستضاف ذاتيا، وغيرها)
   وتاكد من توثيق معدلات العينة لكل اصدار حتى تفسر لوحات التحكم العدادات بدقة.
5. CI تربط هذه الخطوات بالفعل عبر workflows المعاينة/النشر
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`)، لذلك يكفي ان تغطي الاختبارات المحلية
   سلوك الاسرار فقط.
