---
lang: az
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-12-29T18:16:35.174056+00:00"
translation_last_reviewed: 2026-02-07
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
---

# SNS Metrikləri və Onboarding Kit

Yol xəritəsi elementi **SN-8** iki vədi birləşdirir:

1. Qeydiyyatları, yenilənmələri, ARPU-nu, mübahisələri və
   `.sora`, `.nexus` və `.dao` üçün pəncərələri dondurun.
2. Qeydiyyatçılar və stüardlar DNS, qiymət və
   API-lər ardıcıl olaraq hər hansı bir şəkilçi yayımlanmadan əvvəl.

Bu səhifə mənbə versiyasını əks etdirir
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
buna görə də xarici rəyçilər eyni proseduru yerinə yetirə bilərlər.

## 1. Metrik paket

### Grafana idarə paneli və portalın yerləşdirilməsi

- `dashboards/grafana/sns_suffix_analytics.json`-i Grafana-ə (və ya başqasına) idxal edin
  analitik host) standart API vasitəsilə:

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Eyni JSON bu portal səhifəsinin iframe-inə güc verir (bax: **SNS KPI İdarə Paneli**).
  İdarə panelini hər dəfə döyəndə qaçın
  `npm run build && npm run serve-verified-preview` daxilində `docs/portal`
  həm Grafana, həm də yerləşdirmənin sinxron qaldığını təsdiqləyin.

### Panellər və sübutlar

| Panel | Metriklər | İdarəetmə sübutları |
|-------|---------|---------------------|
| Qeydiyyatlar və yenilənmələr | `sns_registrar_status_total` (uğur + yeniləmə həlledici etiketləri) | Hər bir şəkilçi keçid qabiliyyəti + SLA izləmə. |
| ARPU / xalis vahidlər | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Maliyyə registrator manifestlərini gəlirlə uyğunlaşdıra bilər. |
| Mübahisələr və donmalar | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Aktiv dondurmaları, arbitraj ritmini və qəyyumun iş yükünü göstərir. |
| SLA/səhv dərəcələri | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Müştərilərə təsir etməzdən əvvəl API reqressiyalarını vurğulayır. |
| Toplu manifest izləyicisi | `sns_bulk_release_manifest_total`, `manifest_id` etiketləri ilə ödəniş ölçüləri | CSV damcılarını hesablaşma biletlərinə bağlayır. |

Aylıq KPI zamanı Grafana-dən (və ya daxil edilmiş iframe-dən) PDF/CSV ixrac edin
nəzərdən keçirin və müvafiq əlavə qeydinə əlavə edin
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Stüardlar həmçinin SHA-256-nı tuturlar
`docs/source/sns/reports/` altında ixrac edilmiş paketin (məsələn,
`steward_scorecard_2026q1.md`) beləliklə, auditlər sübut yolunu təkrarlaya bilər.

### Əlavənin avtomatlaşdırılması

Əlavə faylları birbaşa tablosuna ixracdan yaradın ki, rəyçilər a
ardıcıl həzm:

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- Köməkçi ixracı hash edir, UID/teqlər/panel sayını tutur və yazır
  `docs/source/sns/reports/.<suffix>/<cycle>.md` altında Markdown əlavəsi (bax
  `.sora/2026-03` nümunəsi bu sənədlə birlikdə).
- `--dashboard-artifact` ixracı kopyalayır
  `artifacts/sns/regulatory/<suffix>/<cycle>/` beləliklə əlavədə istinad edilir
  kanonik sübut yolu; `--dashboard-label` yalnız sizə işarə etmək lazım olduqda istifadə edin
  qrupdan kənar arxivdə.
- `--regulatory-entry` idarəetmə sənədində qeyd edir. Köməkçi daxil edir (və ya
  əvəz edir) əlavə yolunu, tablosunu qeyd edən `KPI Dashboard Annex` bloku
  artefakt, həzm və vaxt damğası, beləliklə dəlil təkrar qaçışlardan sonra sinxronlaşdırılır.
- `--portal-entry` Docusaurus surətini saxlayır (`docs/portal/docs/sns/regulatory/*.md`)
  uyğunlaşdırılmışdır ki, rəyçilər ayrıca əlavə xülasələrini əl ilə fərqləndirmək məcburiyyətində qalmasınlar.
- `--regulatory-entry`/`--portal-entry`-i ötürsəniz, yaradılan faylı əlavə edin
  qeydləri əl ilə və hələ də Grafana-dən çəkilmiş PDF/CSV görüntülərini yükləyin.
- Təkrarlanan ixraclar üçün şəkilçi/dövr cütlərini qeyd edin
  `docs/source/sns/regulatory/annex_jobs.json` və işə salın
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Köməkçi hər girişi gəzir,
  tablosunun ixracını kopyalayır (defolt olaraq `dashboards/grafana/sns_suffix_analytics.json`
  müəyyən edilmədikdə) və hər bir tənzimləmə daxilində əlavə bloku yeniləyir (və,
  mövcud olduqda, portal) bir keçiddə memo.
- `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (və ya `make check-sns-annex`) işə salın ki, iş siyahısı çeşidlənib/çıkarılıb, hər bir yaddaşda uyğun `sns-annex` marker var və əlavə stub var. Köməkçi idarəetmə paketlərində istifadə olunan yerli/hash xülasələrinin yanında `artifacts/sns/annex_schedule_summary.json` yazır.
Bu, əl ilə kopyalama/yapışdırma addımlarını aradan qaldırır və SN-8 əlavə sübutlarını eyni vaxtda saxlayır
CI-də mühafizə cədvəli, marker və lokalizasiya sürüşməsi.

## 2. Onboarding dəsti komponentləri

### Suffiks naqilləri

- Reyestr sxemi + seçici qaydaları:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  və [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS skelet köməkçisi:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  ilə çəkilən məşq axını
  [Gateway/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Hər registratorun işə salınması üçün altında qısa bir qeyd yazın
  `docs/source/sns/reports/` seçici nümunələrini, GAR sübutlarını və DNS hashlarını ümumiləşdirir.

### Qiymətləndirmə cədvəli

| Etiket uzunluğu | Baza haqqı (USD ekviv) |
|-------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

Suffiks əmsalları: `.sora` = 1,0×, `.nexus` = 0,8×, `.dao` = 1,3×.  
Müddət çarpanları: 2‑il −5%, 5‑il −12%; lütf pəncərəsi = 30 gün, satınalma
= 60 gün (20% ödəniş, minimum 5 dollar, maksimum 200 dollar). Müzakirə edilmiş sapmaları qeyd edin
qeydiyyatçı bileti.

### Premium auksionlar və yenilənmələr

1. **Premium hovuz** — möhürlənmiş təklif öhdəliyi/aşkarı (SN-3). ilə təklifləri izləyin
   `sns_premium_commit_total` və altında manifest dərc edin
   `docs/source/sns/reports/`.
2. **Hollandiya yenidən açılır** — güzəşt + geri ödəmə müddəti bitdikdən sonra 7 günlük Hollandiya satışına başlayın
   10×-də bu, gündə 15% azalır. Etiket `manifest_id` ilə özünü göstərir
   tablosuna irəliləyiş səthi ola bilər.
3. **Yeniləmə** — monitor `sns_registrar_status_total{resolver="renewal"}` və
   avtomatik yeniləmə yoxlama siyahısını əldə edin (bildirişlər, SLA, ehtiyat ödəniş relsləri)
   qeydiyyat biletinin içərisində.

### Developer API və avtomatlaşdırma

- API müqavilələri: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Toplu köməkçi və CSV sxemi:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Nümunə əmr:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

Manifest ID-ni (`--submission-log` çıxışı) KPI panel filtrinə daxil edin
beləliklə, maliyyə hər buraxılış üzrə gəlir panellərini uzlaşdıra bilər.

### Sübut dəsti

1. Kontaktlar, şəkilçi əhatə dairəsi və ödəniş relsləri olan qeydiyyatçı bileti.
2. DNS/həlledici sübut (zonefile skeletləri + GAR sübutları).
3. Qiymətləndirmə iş vərəqi + idarəetmə tərəfindən təsdiq edilmiş hər hansı ləğvetmələr.
4. API/CLI tüstü testi artefaktları (`curl` nümunələri, CLI transkriptləri).
5. KPI tablosunun ekran görüntüsü + CSV ixracı, aylıq əlavəyə əlavə olunur.

## 3. Yoxlama siyahısını işə salın

| Addım | Sahibi | Artefakt |
|------|-------|----------|
| Dashboard idxal | Məhsul Analitikası | Grafana API cavabı + tablosuna UID |
| Portalın yerləşdirilməsi təsdiqləndi | Sənədlər/DevRel | `npm run build` qeydləri + önizləmə ekran görüntüsü |
| DNS məşqi tamamlandı | Şəbəkə/Ops | `sns_zonefile_skeleton.py` çıxışları + runbook jurnalı |
| Registrar avtomatlaşdırma quru run | Registrar Eng | `sns_bulk_onboard.py` təqdimat jurnalı |
| İdarəetmə sübutları təqdim edildi | İdarəetmə Şurası | Əlavə link + İxrac edilmiş tablosunun SHA-256 |

Qeydiyyatçı və ya şəkilçini aktivləşdirməzdən əvvəl yoxlama siyahısını tamamlayın. İmzalanmış
bundle SN-8 yol xəritəsi qapısını təmizləyir və auditorlara bir arayış verir
bazara girişləri nəzərdən keçirir.