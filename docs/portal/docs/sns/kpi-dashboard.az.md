---
lang: az
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3649db9b00f9be968cfeb98bc34bbc797aaf22d7ac3936698b4f562094911073
source_last_modified: "2025-12-29T18:16:35.173090+00:00"
translation_last_reviewed: 2026-02-07
title: SNS KPI dashboard
description: Live Grafana panels that aggregate registrar, freeze, and revenue metrics for SN-8a.
translator: machine-google-reviewed
---

# Sora Adı Xidməti KPI İdarə Paneli

KPI idarə paneli stüardlara, qəyyumlara və tənzimləyicilərə vahid yer verir
aylıq əlavə kadansından əvvəl qəbul, səhv və gəlir siqnallarını nəzərdən keçirin
(SN-8a). Grafana tərifi anbarda göndərilir
`dashboards/grafana/sns_suffix_analytics.json` və portal eyni şəkildə əks olunur
daxili iframe vasitəsilə panellər, beləliklə təcrübə daxili Grafana ilə uyğun gəlir
misal.

## Filtrlər və Məlumat Mənbələri

- **Suffiks filter** – `sns_registrar_status_total{suffix}` sorğularını belə idarə edir
  `.sora`, `.nexus` və `.dao` müstəqil şəkildə yoxlana bilər.
- **Toplu buraxılış filtri** – `sns_bulk_release_payment_*` ölçülərini əhatə edir
  maliyyə xüsusi qeydiyyatçı manifestini uzlaşdıra bilər.
- **Metriklər** – Torii (`sns_registrar_status_total`,
  `torii_request_duration_seconds`), qəyyum CLI (`guardian_freeze_active`),
  `sns_governance_activation_total` və toplu qoşulma köməkçi ölçüləri.

## Panellər

1. **Qeydiyyatlar (son 24 saat)** – uğurlu qeydiyyatçı tədbirlərinin sayı
   seçilmiş şəkilçi.
2. **İdarəetmə aktivləşdirmələri (30d)** – təşkilat tərəfindən qeydə alınan nizamnamə/əlavə təklifləri
   CLI.
3. **Registratorun ötürmə qabiliyyəti** – müvəffəqiyyətli qeydiyyatçı hərəkətlərinin hər şəkilçi dərəcəsi.
4. **Qeydiyyatçı xətası rejimləri** – xəta ilə işarələnmiş 5 dəqiqəlik sürət
   `sns_registrar_status_total` sayğacları.
5. **Guardian dondurma pəncərələri** – `guardian_freeze_active` olduğu canlı seçicilər
   açıq dondurulmuş bileti bildirir.
6. **Aktivlər üzrə xalis ödəniş vahidləri** – hesabatın yekunları
   Aktiv üçün `sns_bulk_release_payment_net_units`.
7. **Suffiksə görə toplu sorğular** – şəkilçi identifikatoru üzrə manifest həcmləri.
8. **Sorğu üzrə xalis vahidlər** – buraxılışdan əldə edilən ARPU tipli hesablama
   ölçülər.

## Aylıq KPI Baxış Siyahısı

Maliyyə rəhbəri hər ayın ilk çərşənbə axşamı təkrarlanan bir araşdırma aparır:

1. Portalın **Analitika → SNS KPI** səhifəsini (və ya Grafana idarə paneli `sns-kpis`) açın.
2. Qeydiyyatçının ötürmə qabiliyyəti və gəlir cədvəllərinin PDF/CSV ixracını çəkin.
3. SLA pozuntuları üçün şəkilçiləri müqayisə edin (səhv dərəcəsi sıçrayışları, dondurulmuş seçicilər >72 saat,
   ARPU deltaları >10%).
4. Xülasələri + altındakı müvafiq əlavə girişində fəaliyyət elementlərini qeyd edin
   `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. Eksport edilmiş tablosuna aid artefaktları əlavə öhdəliyinə əlavə edin və onları birləşdirin
   şuranın gündəliyi.

Baxış SLA pozuntularını aşkar edərsə, təsirə məruz qalanlar üçün PagerDuty hadisəsi bildirin
sahibi (registratorun növbətçi meneceri, çağırış üzrə qəyyum və ya stüard proqramı rəhbəri) və
əlavə jurnalda düzəlişləri izləyin.