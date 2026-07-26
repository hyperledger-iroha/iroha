---
lang: es
direction: ltr
source: docs/portal/docs/devportal/observability.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مراقبة البوابة والتحليلات

Utilice DOCS-SORA para instalar sondas y sondas y para construir una computadora.
توثق هذه الملاحظة التوصيلات التي تأتي مع البوابة حتى يتمكن المشغلون من ربط المراقبة دون تسريب بيانات الزوار.

## وسم الاصدار

- اضبط `DOCS_RELEASE_TAG=<identifier>` (يرجع الى `GIT_COMMIT` او `dev` عند الغياب) عند
  بناء البوابة. يتم حقن القيمة في `<meta name="sora-release">`
  حتى تتمكن sondas y paneles de control من تمييز عمليات النشر.
- `npm run build` ينتج `build/release.json` (يكتبه
  `scripts/write-checksums.mjs`) y `DOCS_RELEASE_SOURCE`.
  يتم تضمين الملف نفسه في اثار المعاينة ويشار اليه في تقرير فحص الروابط.

## تحليلات تحافظ على الخصوصية

- Lea `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para que funcione correctamente.
  تحتوي الحمولات على `{ event, path, locale, release, ts }` دون metadatos مرجع او IP, ويتم استخدام
  `navigator.sendBeacon` كلما امكن لتجنب حجب التنقلات.
- تحكم في اخذ العينات عبر `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). يخزن المتتبع اخر camino مرسل ولا يرسل
  احداثا مكررة لنفس التنقل.
- التنفيذ موجود في `src/components/AnalyticsTracker.jsx` ويتم تركيبه عالميا عبر `src/theme/Root.js`.

## sondas اصطناعية

- `npm run probe:portal` يرسل طلبات GET الى المسارات الشائعة
  (`/`, `/norito/overview`, `/reference/torii-swagger`, y وغيرها) ويتحقق من ان وسم
  `sora-release` y `--expect-release` (y `DOCS_RELEASE_TAG`). Nombre:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

يتم الابلاغ عن الاخفاقات لكل path، ما يسهل بوابة CD على نجاح sondas.

## اتتمة الروابط المكسورة- `npm run check:links` يفحص `build/sitemap.xml`، ويتاكد ان كل مدخل يطابق ملفا محليا
  (مع التحقق من fallbacks `index.html`) ، ويكتب `build/link-report.json` الذي يحتوي على
  metadatos الاصدار، الاجمالي، الاخفاقات، وبصمة SHA-256 لـ `checksums.sha256`
  (معروضة كـ `manifest.id`) حتى يرتبط كل تقرير بmanifest الاثر.
- السكربت يخرج بكود غير صفري عند فقدان صفحة, لذا يمكن لـ CI منع الاصدارات عند وجود مسارات قديمة او
  مكسورة. التقارير تذكر المسارات المرشحة التي تمت تجربتها، ما يساعد على تتبع تراجعات التوجيه
  الى شجرة docs.

## لوحة Grafana والتنبيهات- `dashboards/grafana/docs_portal.json` ينشر لوحة Grafana **Publicación del portal de documentos**.
  وتضمن اللوحات التالية:
  - *Rechazos de puerta de enlace (5m)* يستخدم `torii_sorafs_gateway_refusals_total` بنطاق
    `profile`/`reason` حتى يتمكن SRE من كشف دفعات سياسة خاطئة او فشل رموز.
  - *Resultados de la actualización de caché de alias* y*Edad de prueba de alias p90* تتبع
    `torii_sorafs_alias_cache_*` Hay pruebas y pruebas de corte de DNS.
  - *Recuentos de manifiestos de registro de pines* y *Recuento de alias activos* Registro de pines pendientes
    واجمالي alias حتى تتمكن الحوكمة من تدقيق كل اصدار.
  - *Caducidad de TLS de puerta de enlace (horas)* يبرز عندما يقترب Certificado TLS لبوابة النشر من الانتهاء
    (عتبة التنبيه 72 h).
  - *Resultados del SLA de replicación* y *Replicación pendiente* Telemetría
    `torii_sorafs_replication_*` لضمان ان كل النسخ تحقق معيار GA بعد النشر.
- Adaptadores de corriente (`profile`, `reason`) en lugar de `docs.sora`
  او التحقيق في الطفرات عبر كل البوابات.
- توجيه PagerDuty يستخدم لوحات panel de control: التنبيهات
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, y `DocsPortal/TLSExpiry`
  تطلق عندما تتجاوز السلاسل المقابلة عتباتها. اربط runbook التنبيه بهذه الصفحة حتى يتمكن
  مهندسو de guardia من اعادة تشغيل استعلامات Prometheus الدقيقة.

## جمع الخطوات1. اثناء `npm run build`, اضبط متغيرات بيئة release/analytics ودع خطوة ما بعد البناء تصدر
   `checksums.sha256`, `release.json`, y `link-report.json`.
2. Introduzca `npm run probe:portal` en el nombre de host de المعاينة مع
   `--expect-release` مربوطا بنفس الوسم. احفظ stdout لقائمة نشر النشر.
3. Haga clic en `npm run check:links` para ver el mapa del sitio y crear archivos JSON.
   مع اثار المعاينة. تقوم CI بوضع اخر تقرير في `artifacts/docs_portal/link-report.json` حتى تمكن
   الحوكمة من تنزيل حزمة الادلة مباشرة من سجلات البناء.
4. وجه endpoint التحليلات الى مجمع يحافظ على الخصوصية (Plausible, OTEL ingest مستضاف ذاتيا، وغيرها)
   وتاكد من توثيق معدلات العينة لكل اصدار حتى تفسر لوحات التحكم العدادات بدقة.
5. CI تربط هذه الخطوات بالفعل عبر flujos de trabajo المعاينة/النشر
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`) ، لذلك يكفي ان تغطي الاختبارات المحلية
   سلوك الاسرار فقط.