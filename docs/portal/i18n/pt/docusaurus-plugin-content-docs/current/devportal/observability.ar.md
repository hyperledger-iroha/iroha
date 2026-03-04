---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مراقبة البوابة والتحليلات

O documento DOCS-SORA está disponível para teste e sondas para construção Senhor.
توثق هذه الملاحظة التوصيلات التي تأتي مع البوابة حتى يتمكن المشغلون من ربط المراقبة دون تسريب بيانات الزوار.

## وسم الاصدار

- `DOCS_RELEASE_TAG=<identifier>` (`GIT_COMMIT` e `dev`)
  بناء البوابة. Você pode usar o `<meta name="sora-release">`
  Você pode usar sondas e painéis de controle para ajudá-lo.
- `npm run build` ou `build/release.json` (يكتبه
  `scripts/write-checksums.mjs`) e `DOCS_RELEASE_SOURCE`.
  Não se preocupe com o problema e com a ajuda de uma máquina de lavar louça.

## تحليلات تحافظ على الخصوصية

- Verifique `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para obter mais informações.
  A localização do `{ event, path, locale, release, ts }` tem metadados de metadados e IP, e de metadados
  `navigator.sendBeacon` é um problema que pode ocorrer.
- Verifique o valor do arquivo `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). يخزن المتتبع اخر path مرسل ولا يرسل
  Faça isso com cuidado.
- O nome do arquivo é `src/components/AnalyticsTracker.jsx` e o arquivo `src/theme/Root.js`.

## probes

- `npm run probe:portal` يرسل طلبات GET الى المسارات الشائعة
  (`/`, `/norito/overview`, `/reference/torii-swagger`, وغيرها) ويتحقق من ان وسم
  `sora-release` é `--expect-release` (e `DOCS_RELEASE_TAG`). Exemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Não use o caminho para o caminho, mas o CD não será testado.

## اتتمة الروابط المكسورة

- `npm run check:links` يفحص `build/sitemap.xml`, ويتاكد ان كل مدخل يطابق ملفا محليا
  (Mesmo com fallbacks `index.html`), e `build/link-report.json` não está disponível.
  metadata
  (معروضة كـ `manifest.id`) حتى يرتبط كل تقرير بmanifest الاثر.
- السكربت يخرج بكود غير صفري عند فقدان صفحة, لذا يمكن لـ CI منع الاصدارات عند وجود مسارات قديمة او
  Obrigado. Você pode fazer isso sem precisar de mais nada.
  Todos os documentos.

## لوحة Grafana والتنبيهات

- `dashboards/grafana/docs_portal.json` ينشر لوحة Grafana **Publicação no Portal de Documentos**.
  وتتضمن اللوحات التالية:
  - *Recusas de gateway (5m)* يستخدم `torii_sorafs_gateway_refusals_total` بنطاق
    `profile`/`reason` é um SRE que não pode ser usado em nenhum lugar e em nenhum outro lugar.
  - *Resultados de atualização do cache de alias* e *Alias Proof Age p90* تتبع
    `torii_sorafs_alias_cache_*` As provas e provas são cortadas através do corte de DNS.
  - *Pin Registry Manifest Counts* e *Active Alias Count* تعكس backlog pin-registry
    واجمالي alias حتى تتمكن الحوكمة من تدقيق كل اصدار.
  - *Expiração do Gateway TLS (horas)* يبرز عندما يقترب Certificado TLS لبوابة النشر من الانتهاء
    (72 horas).
  - *Resultados de SLA de replicação* e *Backlog de replicação* por telemetria
    `torii_sorafs_replication_*` não é compatível com GA.
- استخدم متغيرات القالب المدمجة (`profile`, `reason`) للتركيز على ملف نشر `docs.sora`
  E isso é algo que você pode fazer.
- O PagerDuty é usado no painel do painel.
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, e`DocsPortal/TLSExpiry`
  Certifique-se de que está tudo bem. اربط runbook التنبيه بهذه الصفحة حتى يتمكن
  O telefone de plantão está disponível no Prometheus.

## جمع الخطوات

1. اثناء `npm run build`, اضبط متغيرات بيئة release/analytics ودع خطوة ما بعد البناء تصدر
   `checksums.sha256`, `release.json`, e`link-report.json`.
2. Use `npm run probe:portal` para definir o nome do host
   `--expect-release` é um problema. Selecione stdout para esta opção.
3. Coloque `npm run check:links` no sitemap do sitemap e no JSON do site.
   مع اثار المعاينة. O CI pode ser instalado em `artifacts/docs_portal/link-report.json`.
   Você pode fazer isso com uma chave de fenda.
4. O endpoint do وجه التحليلات الى مجمع يحافظ على الخصوصية (Plausível, OTEL ingest مستضاف ذاتيا, وغيرها)
   Isso significa que você pode usar o dispositivo para obter mais informações.
5. CI تربط هذه الخطوات بالفعل عبر fluxos de trabalho المعاينة/النشر
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), لذلك يكفي ان تغطي الاختبارات المحلية
   سلوك الاسرار فقط.