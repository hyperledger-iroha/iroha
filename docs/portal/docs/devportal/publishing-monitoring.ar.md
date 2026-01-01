---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6efe6943d41c95ebaf768360ead55a18996db371587c20571ece906c5ede56f1
source_last_modified: "2025-11-20T04:38:45.090032+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: publishing-monitoring
title: نشر SoraFS والمراقبة
sidebar_label: النشر والمراقبة
description: توثيق تدفق المراقبة الشامل لاصدارات بوابة SoraFS حتى يحصل DOCS-3c على probes حتمية وقياس عن بعد وحزم ادلة.
---

يتطلب بند خارطة الطريق **DOCS-3c** اكثر من قائمة فحص للتغليف: بعد كل نشر لـ SoraFS يجب ان نثبت
باستمرار ان بوابة المطورين ووكيل Try it وارتباطات البوابة سليمة. توثق هذه الصفحة سطح المراقبة
الذي يرافق [دليل النشر](./deploy-guide.md) حتى يتمكن CI ومهندسو المناوبة من تنفيذ نفس الفحوصات
التي تستخدمها Ops لفرض SLO.

## مراجعة خط الانابيب

1. **البناء والتوقيع** - اتبع [دليل النشر](./deploy-guide.md) لتشغيل
   `npm run build` و `scripts/preview_wave_preflight.sh` وخطوات ارسال Sigstore + manifest.
   يصدر سكربت preflight ملف `preflight-summary.json` حتى تحمل كل معاينة بيانات build/link/probe.
2. **التثبيت والتحقق** - `sorafs_cli manifest submit` و `verify-sorafs-binding.mjs`
   وخطة تحويل DNS توفر artefacts حتمية للحوكمة.
3. **ارشفة الادلة** - احفظ ملخص CAR وحزمة Sigstore ودليل alias ومخرجات probe ولقطات لوحة
   `docs_portal.json` تحت `artifacts/sorafs/<tag>/`.

## قنوات المراقبة

### 1. مراقبات النشر (`scripts/monitor-publishing.mjs`)

الامر الجديد `npm run monitor:publishing` يجمع probe البوابة وprobe وكيل Try it
ومتحقق الارتباطات في فحص واحد مناسب لـ CI. وفر ملف config بصيغة JSON
(مخزن في اسرار CI او `configs/docs_monitor.json`) ثم شغل:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

اضف `--prom-out ../../artifacts/docs_monitor/monitor.prom` (واختياريا
`--prom-job docs-preview`) لاصدار metrics بصيغة نص Prometheus المناسبة
لـ Pushgateway او scrapes مباشرة في staging/production. تعكس هذه المقاييس
ملخص JSON حتى تتمكن لوحات SLO وقواعد التنبيه من تتبع صحة البوابة وTry it
والارتباطات وDNS بدون تحليل حزمة الادلة.

مثال config مع knobs المطلوبة وروابط متعددة:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/wonderland@wonderland/assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/manifest",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeiaff84aef0aaaf6a7c246c8ca1889e62d69c8d9b20d94933cb7b09902f3",
      "manifest": "8b8f3d2a4a7e92abdb17e5fafd4f9d67c6c7a8547ff985bb0d71f87209c1444d",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    },
    {
      "label": "openapi",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/openapi",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeidevopenapi",
      "manifest": "dad4b9fd48e35297c7fd71cd15b52c4ff0bb62dd8a1da4c5c2c1536ae2732b55",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    },
    {
      "label": "portal-sbom",
      "url": "https://docs-preview.sora.link/.well-known/sorafs/portal-sbom",
      "alias": "docs-preview.sora.link",
      "contentCid": "bafybeiportalssbom",
      "manifest": "e2b2790f9f4c1ecbc8f1bdb9f8ba3fd65fd687e9e5e4de3c3d67c3d3192b79c8",
      "status": "ok",
      "expectHost": "docs-preview.sora.link"
    }
  ],
  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

يكتب المراقب ملخص JSON (مناسب لـ S3/SoraFS) ويخرج برمز غير صفري عند فشل اي probe،
ما يجعله مناسبا لCron jobs وخطوات Buildkite وwebhooks الخاصة بـ Alertmanager.
تمرير `--evidence-dir` يحفظ `summary.json` و`portal.json` و`tryit.json` و`binding.json`
مع manifest باسم `checksums.sha256` حتى يتمكن مراجعو الحوكمة من مقارنة النتائج
دون اعادة تشغيل probes.

> **حاجز TLS:** يرفض `monitorPortal` عناوين `http://` الا اذا ضبطت
> `allowInsecureHttp: true` في config. ابق اختبارات production/staging على HTTPS؛
> هذا الخيار موجود فقط للمعاينات المحلية.

كل ادخال binding يفرض `Sora-Name` و`Sora-Proof` و`Sora-Content-CID`
(الرؤوس والمحتوى) اضافة الى حارس `expectHost` حتى لا ينحرف ترويج DNS
(`docs.sora` مقابل `docs-preview.sora.link`) عن alias المسجل في pin registry.
تفشل الفحوصات بسرعة اذا توقف gateway عن تثبيت رؤوس `Sora-Content-CID`/`Sora-Proof`،
او ظهر base64 غير صالح في الدليل، او اختلف manifest/CID المعلن عن payloads المثبتة
(الموقع وOpenAPI وSBOM).

كتلة `dns` الاختيارية توصل rollout SoraDNS في DOCS-7 الى نفس المراقب. كل ادخال
يحل زوج hostname/record-type (مثلا CNAME `docs-preview.sora.link` ->
`docs-preview.sora.link.gw.sora.name`) ويتحقق من تطابق الاجابات مع
`expectedRecords` او `expectedIncludes`. الادخال الثاني في المثال اعلاه يثبت
الاسم canonical المشفر الذي ينتجه `cargo xtask soradns-hosts --name docs-preview.sora.link`؛
المراقب يثبت الان ان alias المفضل والهاش canonical (`igjssx53...gw.sora.id`)
يحلان الى المضيف المثبت. هذا يجعل دليل ترويج DNS تلقائيا:
سيفشل المراقب اذا انحرف اي مضيف حتى لو ظلت bindings HTTP تثبت manifest الصحيح.

### 2. حارس manifest لاصدارات OpenAPI

شرط DOCS-2b لـ "manifest OpenAPI موقع" يوفر الان حارسا مؤتمتا:
`ci/check_openapi_spec.sh` يستدعي `npm run check:openapi-versions`، الذي يشغل
`scripts/verify-openapi-versions.mjs` لمقارنة
`docs/portal/static/openapi/versions.json` مع مواصفات Torii وmanifests الحقيقية.
يتحقق الحارس من:

- كل اصدار مدرج في `versions.json` لديه مجلد مطابق تحت `static/openapi/versions/`.
- حقولا `bytes` و`sha256` تطابقان ملف spec على القرص.
- alias `latest` يعكس مدخل `current` (metadata digest/size/signature)
  حتى لا ينحرف التحميل الافتراضي.
- الادخالات الموقعة تشير الى manifest يشير `artifact.path` فيه الى نفس spec،
  وقيم التوقيع/المفتاح العام بالهيكس تطابق manifest.

شغل الحارس محليا عند نسخ spec جديدة:

```bash
cd docs/portal
npm run check:openapi-versions
```

تتضمن رسائل الفشل تلميح الملف القديم (`npm run sync-openapi -- --latest`)
حتى يعرف مساهمو البوابة كيفية تحديث snapshots. ابقاء الحارس في CI يمنع
اصدارات البوابة التي يخرج فيها manifest الموقع وdigest المنشور عن التزامن.

### 2. لوحات القياس والتنبيهات

- **`dashboards/grafana/docs_portal.json`** - اللوحة الرئيسية لـ DOCS-3c. اللوحات
  تتبع `torii_sorafs_gateway_refusals_total` واخفاقات SLA الخاصة بالنسخ المتماثل
  واخطاء وكيل Try it وزمن الاستجابة (overlay `docs.preview.integrity`). صدّر اللوحة
  بعد كل اصدار وارفقها بتذكرة العمليات.
- **تنبيهات وكيل Try it** - قاعدة Alertmanager `TryItProxyErrors` تعمل عند انخفاض
  مستمر في `probe_success{job="tryit-proxy"}` او ارتفاعات
  `tryit_proxy_requests_total{status="error"}`.
- **Gateway SLO** - `DocsPortal/GatewayRefusals` يضمن ان bindings alias تستمر
  في اعلان digest الـ manifest المثبت؛ عمليات التصعيد ترتبط بنص CLI
  `verify-sorafs-binding.mjs` الملتقط اثناء النشر.

### 3. مسار الادلة

يجب ان تضيف كل عملية مراقبة:

- حزمة ادلة `monitor-publishing` (`summary.json`، ملفات الاقسام، و`checksums.sha256`).
- لقطات Grafana للوحة `docs_portal` خلال نافذة الاصدار.
- سجلات تغيير/تراجع وكيل Try it (سجلات `npm run manage:tryit-proxy`).
- مخرجات تحقق alias من `scripts/verify-sorafs-binding.mjs`.

احفظ هذه العناصر تحت `artifacts/sorafs/<tag>/monitoring/` واربطها في تذكرة الاصدار
حتى يبقى مسار التدقيق بعد انتهاء صلاحية سجلات CI.

## قائمة تشغيل تشغيلية

1. نفذ دليل النشر حتى الخطوة 7.
2. نفذ `npm run monitor:publishing` بتكوين production؛ وارشف مخرجات JSON.
3. التقط لوحات Grafana (`docs_portal`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`)
   وارفقها بتذكرة الاصدار.
4. جدول مراقبات دورية (موصى به: كل 15 دقيقة) تشير الى عناوين production بنفس التكوين
   لتحقيق بوابة SLO الخاصة بـ DOCS-3c.
5. اثناء الحوادث، اعد تشغيل امر المراقبة مع `--json-out` لتسجيل ادلة قبل/بعد
   وارفقها بالتقرير اللاحق.

اتباع هذا المسار يغلق DOCS-3c: تدفق build للبوابة، وخط نشر، ومكدس المراقبة
اصبحوا في playbook واحد مع اوامر قابلة لاعادة التنفيذ وconfigs نموذجية وربطات القياس.
