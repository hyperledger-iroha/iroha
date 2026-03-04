---
id: observability
lang: az
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Portal Observability & Analytics
sidebar_label: Observability
description: Telemetry, release tagging, and verification automation for the developer portal.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

DOCS-SORA yol xəritəsi analitika, sintetik zondlar və pozulmuş əlaqə tələb edir
hər bir önizləmə quruluşu üçün avtomatlaşdırma. Bu qeyd indiki santexnika sənədlərini təsdiqləyir
portalla göndərilir ki, operatorlar ziyarətçini sızdırmadan monitorinqi həyata keçirə bilsinlər
data.

## Etiketləməni buraxın

- `DOCS_RELEASE_TAG=<identifier>` təyin edin (`GIT_COMMIT` və ya `dev`-ə qayıdır)
  portalın tikintisi. Dəyər `<meta name="sora-release">`-ə vurulur
  beləliklə, zondlar və idarə panelləri yerləşdirmələri ayırd edə bilir.
- `npm run build` `build/release.json` yayır (müəllif:
  `scripts/write-checksums.mjs`) etiketi, vaxt damğasını və isteğe bağlı təsviri
  `DOCS_RELEASE_SOURCE`. Eyni fayl önizləmə artefaktlarına və
  keçid yoxlayıcı hesabatına istinad edilir.

## Məxfiliyi qoruyan analitika

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>`-i konfiqurasiya edin
  yüngül izləyicini aktivləşdirin. Faydalı yüklər `{ hadisə, yol, yerli,
  buraxılış, ts }` with no referrer or IP metadata, and `navigator.sendBeacon`
  naviqasiyaların qarşısını almaq üçün mümkün olduqda istifadə olunur.
- `DOCS_ANALYTICS_SAMPLE_RATE` (0–1) ilə nümunə götürməyə nəzarət. İzləyici mağazalar
  son göndərilən yoldur və eyni naviqasiya üçün heç vaxt dublikat hadisələr yaymır.
- Tətbiq `src/components/AnalyticsTracker.jsx`-də yaşayır və belədir
  `src/theme/Root.js` vasitəsilə qlobal olaraq quraşdırılmışdır.

## Sintetik zondlar

- `npm run probe:portal` ümumi marşrutlara qarşı GET sorğuları verir
  (`/`, `/norito/overview`, `/reference/torii-swagger` və s.) və yoxlayır
  `sora-release` meta teqi `--expect-release` ilə uyğun gəlir (və ya
  `DOCS_RELEASE_TAG`). Misal:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Uğursuzluqlar hər bir yolda bildirilir, bu, CD-ni araşdırmanın uğuru ilə əlaqələndirməyi asanlaşdırır.

## Qırılmış keçid avtomatlaşdırılması

- `npm run check:links` `build/sitemap.xml`-i skan edir, hər bir giriş xəritələrini bir
  yerli fayl (`index.html` ehtiyatlarını yoxlayır) və yazır
  `build/link-report.json` buraxılış metadatasını, yekunları, uğursuzluqları,
  və `checksums.sha256` SHA-256 barmaq izi (`manifest.id` kimi ifşa olunur)
  buna görə də hər hesabat artefakt manifestinə bağlana bilər.
- Səhifə əskik olduqda skript sıfırdan çıxır, beləliklə CI buraxılışları bloklaya bilər
  köhnəlmiş və ya pozulmuş marşrutlar. Hesabatlar cəhd edilən namizəd yollarına istinad edir,
  Bu, marşrutlaşdırma reqressiyalarını sənədlər ağacına qaytarmağa kömək edir.

## Grafana idarə paneli və xəbərdarlıqlar

- `dashboards/grafana/docs_portal.json` **Docs Portal Publishing**-i dərc edir
  Grafana lövhəsi. Aşağıdakı panelləri göndərir:
  - *Gateway imtinaları (5m)* əhatə dairəsi daxilində `torii_sorafs_gateway_refusals_total` istifadə edir
    `profile`/`reason` beləliklə, SRE-lər pis siyasət təkanlarını və ya token uğursuzluqlarını aşkar edə bilər.
  - *Alias Cache Refresh Nəticələri* və *Alias Proof Age p90* treki
    DNS kəsilməsindən əvvəl yeni sübutların mövcud olduğunu sübut etmək üçün `torii_sorafs_alias_cache_*`
    bitdi.
  - *Pin Registry Manifest Counts* üstəgəl *Aktiv Alias Sayısı* statını əks etdirir
    pin-registrinin yığılması və ümumi ləqəblər, beləliklə idarəetmə hər buraxılışı yoxlaya bilsin.
  - *Gateway TLS İstifadə müddəti (saat)* nəşr şlüzünün TLS-ni vurğulayır
    sertifikatın müddəti başa çatır (xəbərdarlıq həddi 72 saat).
  - *Replikasiya SLA Nəticələri* və *Replikasiya Geridəliyi* diqqətli olun
    Bütün replikaların GA-ya cavab verməsini təmin etmək üçün `torii_sorafs_replication_*` telemetriyası
    dərc edildikdən sonra bar.
- Daxil edilmiş şablon dəyişənlərindən (`profile`, `reason`) istifadə edin.
  `docs.sora` nəşr profili və ya bütün şlüzlər arasında sıçrayışları araşdırın.
- PagerDuty marşrutu sübut kimi tablosuna panellərdən istifadə edir: adlı xəbərdarlıqlar
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` və
  `DocsPortal/TLSExpiry` yanğın müvafiq sıra onların pozulması
  eşiklər. Xəbərdarlığın runbook-nu bu səhifə ilə əlaqələndirin ki, zəng edən mühəndislər edə bilsin
  dəqiq Prometheus sorğularını təkrarlayın.

## Birlikdə qoymaq

1. `npm run build` zamanı buraxılış/analitik mühit dəyişənlərini təyin edin və
   post-quraşdırma addımı `checksums.sha256`, `release.json` və
   `link-report.json`.
2. ilə önizləmə host adına qarşı `npm run probe:portal`-i işə salın
   `--expect-release` eyni etiketə qoşulmuşdur. Nəşr üçün stdout-u saxlayın
   yoxlama siyahısı.
3. Xırdalanmış sayt xəritəsi girişlərində və arxivdə tez uğursuz olmaq üçün `npm run check:links`-i işə salın
   yaradılan JSON hesabatı önizləmə artefaktları ilə birlikdə. CI düşür
   `artifacts/docs_portal/link-report.json`-də ən son hesabatda idarəetmə bunu edə bilər
   sübut paketini birbaşa qurma qeydlərindən endirin.
4. Analitikanın son nöqtəsini məxfiliyi qoruyan kollektorunuza yönləndirin (Məqbul,
   öz ev sahibliyi etdiyi OTEL qəbulu və s.) və seçmə dərəcələrinin sənədləşdirildiyini təmin edin
   buraxın ki, idarə panelləri sayları düzgün şərh etsin.
5. CI artıq bu addımları önizləmə/yerləşdirmə iş axınları vasitəsilə həyata keçirir
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), belə ki, yerli quru qaçışlar yalnız lazımdır
   gizli xüsusi davranışı əhatə edir.