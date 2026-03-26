---
lang: az
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f68e8cc639bd6a780c33fd14ab4e25df1c6e9381595c7a4c44ff577fea02400d
source_last_modified: "2026-01-22T16:26:46.494851+00:00"
translation_last_reviewed: 2026-02-07
id: publishing-monitoring
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
---

Yol xəritəsi elementi **DOCS-3c** qablaşdırma yoxlama siyahısından daha çoxunu tələb edir: hər dəfə
SoraFS dərc edir, biz davamlı olaraq sübut etməliyik ki, geliştirici portalı, Sınayın
proxy və şlüz bağlamaları sağlam qalır. Bu səhifə monitorinqi sənədləşdirir
[yerləşdirmə təlimatını](./deploy-guide.md) müşayiət edən səth, yəni CI və
zəng mühəndisləri Ops-un SLO-nu tətbiq etmək üçün istifadə etdiyi eyni yoxlamaları həyata keçirə bilər.

## Boru kəmərinin xülasəsi

1. **Quraşdırın və imzalayın** – işə salmaq üçün [yerləşdirmə təlimatına](./deploy-guide.md) əməl edin
   `npm run build`, `scripts/preview_wave_preflight.sh` və Sigstore +
   açıq təqdimat addımları. Uçuşdan əvvəl skript `preflight-summary.json` yayır
   buna görə də hər bir önizləmə qurma/bağlantı/probe metadatasını daşıyır.
2. **Pin və doğrulayın** – `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   və DNS kəsmə planı idarəetmə üçün deterministik artefaktlar təmin edir.
3. **Arxiv sübutu** – CAR xülasəsini, Sigstore paketini, ləqəb sübutunu,
   zond çıxışı və `docs_portal.json` tablosunun anlıq görüntüləri
   `artifacts/sorafs/<tag>/`.

## Monitorinq kanalları

### 1. Nəşriyyat monitorları (`scripts/monitor-publishing.mjs`)

Yeni `npm run monitor:publishing` əmri portal zondunu əhatə edir, Sınayın
proksi zondu və tək CI-dostluq yoxlamasına məcburi yoxlayıcı. Təmin edin a
JSON konfiqurasiyası (CI sirrlərində yoxlanılır və ya `configs/docs_monitor.json`) və işə salın:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom` əlavə edin (və istəyə görə
`--prom-job docs-preview`) üçün uyğun olan Prometheus mətn formatı ölçülərini yaymaq
Pushgateway yükləmələri və ya səhnələşdirmə/istehsalda birbaşa Prometheus qırıntıları. The
ölçülər JSON xülasəsini əks etdirir ki, SLO panelləri və xəbərdarlıq qaydaları izləyə bilsin
portal, Sınayın, bağlayın və sübut paketini təhlil etmədən DNS sağlamlığı.

Tələb olunan düymələr və çoxlu bağlama ilə nümunə konfiqurasiya:

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
    "samplePath": "/proxy/v1/accounts/<katakana-i105-account-id>/assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
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

Monitor JSON xülasəsini yazır (S3/SoraFS dostudur) və o zaman sıfırdan çıxır.
hər hansı bir prob uğursuz olur, bu onu Cron işlərinə, Buildkite addımlarına və ya
Alertmanager webhooks. `--evidence-dir` keçmək davam edir `summary.json`,
`portal.json`, `tryit.json` və `binding.json`, `checksums.sha256` ilə birlikdə
manifestdir ki, idarəetmə rəyçiləri məcburiyyət olmadan monitor nəticələrini fərqləndirə bilsinlər
zondları yenidən işə salın.

> **TLS qoruyucu:** Siz təyin etmədiyiniz halda `monitorPortal` `http://` əsas URL-lərini rədd edir
> konfiqurasiyada `allowInsecureHttp: true`. İstehsal/yaradıcı zondları aktiv saxlayın
> HTTPS; qoşulma yalnız yerli önizləmələr üçün mövcuddur.

Hər bir məcburi giriş tutulana qarşı `cargo xtask soradns-verify-binding` işləyir
`portal.gateway.binding.json` paketi (və isteğe bağlı `manifestJson`), yəni ləqəb,
sübut statusu və məzmun CID dərc edilmiş sübutlarla uyğunlaşır. The
isteğe bağlı `hostname` qoruyucu ləqəbdən əldə edilən kanonik hostun uyğun olduğunu təsdiqləyir
tanıtmaq niyyətində olduğunuz şlüz hostu, DNS kəsilməsinin qarşısını alır
qeydə alınmış bağlama.

İsteğe bağlı `dns` blok telləri DOCS-7-nin SoraDNS-ni eyni monitora bağlayır.
Hər bir giriş host adı/qeyd növü cütünü həll edir (məsələn,
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) və
cavabların `expectedRecords` və ya `expectedIncludes` uyğunluğunu təsdiqləyir. ikinci
tərəfindən istehsal olunan kanonik hashed hostnamenin sərt kodlarının üstündəki parçaya giriş
`cargo xtask soradns-hosts --name docs-preview.sora.link`; monitor indi sübut edir
həm insana uyğun ləqəb, həm də kanonik hash (`igjssx53…gw.sora.id`)
bərkidilmiş yaraşıqlı ev sahibinə qərar verin. Bu, DNS təşviqi sübutunu avtomatik edir:
HTTP bağlı olsa belə, hostlardan biri sürüşsə, monitor uğursuz olacaq
düzgün manifestti qeyd edin.

### 2. OpenAPI versiyası manifest qoruyucusu

DOCS-2b-nin "imzalanmış OpenAPI manifest" tələbi indi avtomatlaşdırılmış qoruyucu göndərir:
`ci/check_openapi_spec.sh`, `npm run check:openapi-versions` çağırır
Çarpaz yoxlamaq üçün `scripts/verify-openapi-versions.mjs`
`docs/portal/static/openapi/versions.json` faktiki Torii xüsusiyyətləri ilə və
təzahür edir. Mühafizəçi təsdiqləyir:

- `versions.json`-də sadalanan hər versiyanın altında uyğun kataloq var
  `static/openapi/versions/`.
- Hər bir girişin `bytes` və `sha256` sahələri diskdəki spesifikasiya faylına uyğun gəlir.
- `latest` ləqəbi `current` girişini əks etdirir (həzm/ölçü/imza metadatası)
  belə ki, standart endirmə sürüşə bilməz.
- İmzalanmış qeydlər `artifact.path`-ə işarə edən manifestə istinad edir.
  eyni spesifikasiya və imza/ictimai açar hex dəyərləri manifestlə uyğun gəlir.

Yeni bir spesifikasiyanı əks etdirdiyiniz zaman mühafizəçini yerli olaraq işə salın:

```bash
cd docs/portal
npm run check:openapi-versions
```

Uğursuzluq mesajlarına köhnə fayl işarəsi daxildir (`npm run sync-openapi -- --latest`)
buna görə də portal müəllifləri anlıq görüntüləri necə yeniləməyi bilirlər. Mühafizəçini içəridə saxlamaq
CI, imzalanmış manifest və dərc edilmiş digestin olduğu portal buraxılışlarının qarşısını alır
sinxronizasiyadan çıxmaq.

### 2. İdarə panelləri və xəbərdarlıqlar

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c üçün əsas lövhə. Panellər
  track `torii_sorafs_gateway_refusals_total`, replikasiya SLA əldən verir, Sınayın
  proksi xətaları və araşdırma gecikməsi (`docs.preview.integrity` üst-üstə düşmə). İxrac et
  hər buraxılışdan sonra lövhəyə qoyun və əməliyyat biletinə əlavə edin.
- **Proksi xəbərdarlıqlarını sınayın** – Alertmanager qaydası `TryItProxyErrors` işə salınır
  davamlı `probe_success{job="tryit-proxy"}` damcı və ya
  `tryit_proxy_requests_total{status="error"}` sünbüllər.
- **Gateway SLO** – `DocsPortal/GatewayRefusals` ləqəb bağlamalarının davam etməsini təmin edir
  bərkidilmiş manifest həzmini reklam etmək; eskalasiyalar ilə əlaqələndirilir
  `cargo xtask soradns-verify-binding` CLI transkripti dərc zamanı çəkildi.

### 3. Sübut izi

Hər bir monitorinq proqramı əlavə edilməlidir:

- `monitor-publishing` sübut paketi (`summary.json`, bölmə üzrə fayllar və
  `checksums.sha256`).
- Buraxılış pəncərəsi üzərində `docs_portal` lövhəsi üçün Grafana ekran görüntüləri.
- Proksi dəyişdirmə/geri qaytarma transkriptlərini sınayın (`npm run manage:tryit-proxy` qeydləri).
- `cargo xtask soradns-verify-binding`-dən ləqəb yoxlama çıxışı.

Bunları `artifacts/sorafs/<tag>/monitoring/` altında saxlayın və onları birləşdirin
CI qeydlərinin müddəti bitdikdən sonra audit cığırının sağ qalması üçün buraxılış məsələsi.

## Əməliyyat yoxlama siyahısı

1. Addım 7 vasitəsilə yerləşdirmə təlimatını işə salın.
2. `npm run monitor:publishing`-i istehsal konfiqurasiyası ilə icra edin; arxiv
   JSON çıxışı.
3. Grafana panellərini çəkin (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) və onları buraxılış biletinə əlavə edin.
4. Təkrarlanan monitorları planlaşdırın (tövsiyə olunur: hər 15 dəqiqədən bir)
   DOCS-3c SLO qapısını təmin etmək üçün eyni konfiqurasiyaya malik istehsal URL-ləri.
5. Hadisələr zamanı qeyd etmək üçün monitor əmrini `--json-out` ilə yenidən işə salın
   sübutdan əvvəl/sonra və onu postmortemə əlavə edin.

Bu dövrədən sonra DOCS-3c bağlanır: portal qurma axını, nəşriyyat boru kəməri,
və monitorinq yığını indi təkrarlana bilən əmrlərlə tək bir oyun kitabında yaşayır,
nümunə konfiqurasiyaları və telemetriya qarmaqları.