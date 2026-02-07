---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/observability.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مراقبة البوابة والتحليلات

خارطة طريق DOCS-SORA تتطلب تحليلات، وprobes اصطناعية، واتمتة الروابط المكسورة لكل build معاينة.
توثق هذه الملاحظة التوصيلات التي تأتي مع البوابة حتى يتمكن المشغلون من ربط المراقبة دون تسريب بيانات الزوار.

## وسم الاصدار

- `DOCS_RELEASE_TAG=<identifier>` (`GIT_COMMIT` et `dev`)
  بناء البوابة. يتم حقن القيمة في `<meta name="sora-release">`
  Il y a des sondes et des tableaux de bord avec des outils.
- `npm run build` ou `build/release.json` (
  `scripts/write-checksums.mjs`) et `DOCS_RELEASE_SOURCE` الاختياري.
  يتم تضمين الملف نفسه في اثار المعاينة ويشار اليه في تقرير فحص الروابط.

## تحليلات تحافظ على الخصوصية

- اضبط `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` لتمكين المتتبع الخفيف.
  Utilisez le code `{ event, path, locale, release, ts }` pour les métadonnées et l'adresse IP.
  `navigator.sendBeacon` كلما امكن لتجنب حجب التنقلات.
- Utilisez la fonction `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). يخزن المتتبع اخر chemin مرسل ولا يرسل
  احداثا مكررة لنفس التنقل.
- Le lien entre `src/components/AnalyticsTracker.jsx` et le lien vers `src/theme/Root.js`.

## sondes اصطناعية

- `npm run probe:portal` يرسل طلبات GET الى المسارات الشائعة
  (`/`, `/norito/overview`, `/reference/torii-swagger`, et) ويتحقق من ان وسم
  `sora-release` ou `--expect-release` (et `DOCS_RELEASE_TAG`). مثال:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Il s'agit d'un chemin d'accès et d'un CD pour les sondes.

## اتتمة الروابط المكسورة- `npm run check:links` et `build/sitemap.xml`, et vous avez besoin de plus d'informations.
  (avec les solutions de repli `index.html`) et `build/link-report.json` comme vous le souhaitez
  métadonnées de métadonnées par SHA-256 pour `checksums.sha256`
  (معروضة كـ `manifest.id`) حتى يرتبط كل تقرير بmanifest الاثر.
- السكربت يخرج بكود غير صفري عند فقدان صفحة، لذا يمكن لـ CI منع الاصدارات عند وجود مسارات قديمة او
  مكسورة. التقارير تذكر المسارات المرشحة التي تمت تجربتها، ما يساعد على تتبع تراجعات التوجيه
  الى شجرة docs.

## لوحة Grafana والتنبيهات- `dashboards/grafana/docs_portal.json` ينشر لوحة Grafana **Publication du portail Docs**.
  وتتضمن اللوحات التالية:
  - *Refus de passerelle (5 m)* يستخدم `torii_sorafs_gateway_refusals_total` بنطاق
    `profile`/`reason` est le SRE le plus proche de vous.
  - *Résultats de l'actualisation du cache d'alias* et*Alias Proof Age p90* تتبع
    `torii_sorafs_alias_cache_*` لاثبات وجود proofs حديثة قبل DNS cut-over.
  - *Nombre de manifestes du registre des broches* et *Nombre d'alias actifs* Plus de registre de broches en retard
    واجمالي alias حتى تتمكن الحوكمة من تدقيق كل اصدار.
  - *Expiration TLS de la passerelle (heures)* يبرز عندما يقترب TLS cert لبوابة النشر من الانتهاء
    (عتبة التنبيه 72 h).
  - *Résultats SLA de réplication* et*Replication Backlog* par télémétrie
    `torii_sorafs_replication_*` لضمان ان كل النسخ تحقق معيار GA بعد النشر.
- استخدم متغيرات القالب المدمجة (`profile`, `reason`) للتركيز على ملف نشر `docs.sora`
  او التحقيق في الطفرات عبر كل البوابات.
- Utiliser PagerDuty pour le tableau de bord de la page : Informations sur le tableau de bord
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry`
  تطلق عندما تتجاوز السلاسل المقابلة عتباتها. اربط runbook التنبيه بهذه الصفحة حتى يتمكن
  مهندسو on-call من اعادة تشغيل استعلامات Prometheus الدقيقة.

## جمع الخطوات1. Utilisez `npm run build` pour les versions release/analytics et pour les versions ultérieures.
   `checksums.sha256`, `release.json`, et `link-report.json`.
2. Utilisez `npm run probe:portal` pour le nom d'hôte de votre ordinateur.
   `--expect-release` مربوطا بنفس الوسم. احفظ stdout لقائمة نشر النشر.
3. Utilisez `npm run check:links` pour télécharger le plan du site et utiliser JSON.
   مع اثار المعاينة. تقوم CI بوضع اخر تقرير في `artifacts/docs_portal/link-report.json` حتى تتمكن
   الحوكمة من تنزيل حزمة الادلة مباشرة من سجلات البناء.
4. Le point final est également pris en compte (Plausible, OTEL ingest مستضاف ذاتيا، وغيرها)
   وتاكد من توثيق معدلات العينة لكل اصدار حتى تفسر لوحات التحكم العدادات بدقة.
5. CI تربط هذه الخطوات بالفعل عبر workflows المعاينة/النشر
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), لذلك يكفي ان تغطي الاختبارات المحلية
   سلوك الاسرار فقط.