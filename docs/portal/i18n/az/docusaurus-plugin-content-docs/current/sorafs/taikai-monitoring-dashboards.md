---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Taikai Monitoring Dashboards
description: Portal summary of the viewer/cache Grafana boards that back SN13-C evidence
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Taikai marşrutlaşdırma manifestinin (TRM) hazırlığı iki Grafana lövhəsində və onların
yoldaş xəbərdarlıqları. Bu səhifənin əsas məqamlarını əks etdirir
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`,
və `dashboards/alerts/taikai_viewer_rules.yml`, beləliklə rəyçilər izləyə bilsinlər
anbarı klonlaşdırmadan.

## Baxış paneli (`taikai_viewer.json`)

- **Canlı kənar və gecikmə:** Panellər p95/p99 gecikmə histoqramlarını vizuallaşdırır
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`)
  çoxluq/axın. p99 >900ms və ya drift >1,5s üçün baxın (tetikler
  `TaikaiLiveEdgeDrift` xəbərdarlığı).
- **Seqment xətaları:** `taikai_ingest_segment_errors_total{reason}`-i ayırır
  deşifrə uğursuzluqlarını, nəsil təkrarı cəhdlərini və ya uyğunsuzluqları aşkar edin.
  Bu panel yuxarı qalxdıqda SN13-C insidentlərinə ekran görüntülərini əlavə edin
  "xəbərdarlıq" qrupu.
- **İzləyici və CEK sağlamlığı:** Panellər `taikai_viewer_*` metrika trekindən götürülüb
  CEK fırlanma yaşı, PQ qoruyucu qarışığı, rebufer sayları və xəbərdarlıq toplanması. CEK
  panel, idarəetmənin yenisini təsdiq etməzdən əvvəl nəzərdən keçirdiyi rotasiya SLA-nı tətbiq edir
  ləqəblər.
- **Ad telemetriya şəkli:** `/status → telemetry.taikai_alias_rotations`
  masa birbaşa lövhədə oturur ki, operatorlar manifest həzmləri təsdiq edə bilsinlər
  idarəetmə sübutlarını əlavə etməzdən əvvəl.

## Keş paneli (`taikai_cache.json`)

- **Parti təzyiqi:** Panel diaqramı `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  və `sorafs_taikai_cache_promotions_total`. TRM olub olmadığını görmək üçün bunlardan istifadə edin
  fırlanma xüsusi səviyyələri həddən artıq yükləyir.
- **QoS inkarları:** `sorafs_taikai_qos_denied_total` keş təzyiqi zamanı səthlər
  tənzimləyici qüvvələr; dərəcə sıfırdan qalxdıqda qazma jurnalına şərh yazın.
- **Çıxışdan istifadə:** SoraFS çıxışlarının Taikai ilə ayaqlaşdığını təsdiq etməyə kömək edir
  CMAF pəncərələri fırlananda izləyicilər.

## Xəbərdarlıqlar və sübutların tutulması

- Peyjinq qaydaları `dashboards/alerts/taikai_viewer_rules.yml`-də yaşayır və birini xəritəyə qoyun
  yuxarıdakı panelləri olan birinə (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`,
  `TaikaiCekRotationLag`, sağlamlığa dair xəbərdarlıqlar). Hər bir istehsalı təmin edin
  klaster bunları Alertmanager-ə bağlayır.
- Məşqlər zamanı çəkilmiş şəkillər/skrinşotlar saxlanmalıdır
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` spool faylları ilə yanaşı və
  `/status` JSON. `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` istifadə edin
  icranı paylaşılan qazma jurnalına əlavə etmək.
- İdarə panelləri dəyişdikdə JSON faylının SHA-256 həzmini daxil edin
  portalın PR təsviri beləliklə, auditorlar idarə olunan Grafana qovluğunu
  repo versiyası.

## Sübut paketinin yoxlama siyahısı

SN13-C rəyləri hər bir qazma və ya hadisənin sadalanan eyni artefaktları göndərəcəyini gözləyir
Taikai anchor runbook-da. Onları aşağıdakı ardıcıllıqla çəkin ki, paket olsun
idarəetmənin nəzərdən keçirilməsinə hazırdır:

1. Ən son `taikai-anchor-request-*.json`-i kopyalayın,
   `taikai-trm-state-*.json` və `taikai-lineage-*.json` faylları
   `config.da_ingest.manifest_store_dir/taikai/`. Bu makara artefaktları sübut edir
   hansı marşrutlaşdırma manifest (TRM) və nəsil pəncərəsi aktiv idi. Köməkçi
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   spool fayllarını kopyalayacaq, heşlər yayacaq və istəyə görə xülasəni imzalayacaq.
2. Süzülən `/v2/status` çıxışını qeyd edin
   `.telemetry.taikai_alias_rotations[]` və onu makara fayllarının yanında saxlayın.
   Rəyçilər bildirilən `manifest_digest_hex` və pəncərə sərhədlərini müqayisə edirlər
   kopyalanan makara vəziyyəti.
3. Yuxarıda sadalanan ölçülər üçün Prometheus snapşotlarını ixrac edin və skrinşotlar çəkin
   müvafiq klaster/axın filtrləri olan izləyici/keş tablosunun
   görünüşü. Xam JSON/CSV və ekran görüntülərini artefakt qovluğuna atın.
4. Qaydalara istinad edən Alertmanager insident identifikatorlarını (əgər varsa) daxil edin
   `dashboards/alerts/taikai_viewer_rules.yml` və onların avtomatik bağlanıb-bağlanmadığını qeyd edin
   vəziyyət aydınlaşdıqdan sonra.

Hər şeyi `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` altında saxlayın ki, qazın
auditlər və SN13-C idarəetmə icmalı tək arxivi əldə edə bilər.

## Qazma kadansı və qeydi

- Hər ayın ilk çərşənbə axşamı saat 15:00UTC-də Taikai lövbər qazma məşqini yerinə yetirin.
  Cədvəl SN13 idarəetmə sinxronizasiyasından əvvəl sübutları təzə saxlayır.
- Yuxarıdakı artefaktları ələ keçirdikdən sonra icranı paylaşılan kitaba əlavə edin
  `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` ilə. The
  köməkçi `docs/source/sorafs/runbooks-index.md` tərəfindən tələb olunan JSON girişini yayır.
- Runbook indeksi girişində arxivləşdirilmiş artefaktları əlaqələndirin və hər hansı bir uğursuzluğu artırın
  Media Platforması WG/SRE vasitəsilə 48 saat ərzində xəbərdarlıqlar və ya idarə paneli reqressiyaları
  kanal.
- Qazma xülasəsi ekran görüntüsü dəstini saxlayın (gecikmə, sürüşmə, səhvlər, CEK fırlanması,
  keş təzyiqi) makara paketinin yanında yerləşdirin ki, operatorlar necə dəqiq göstərə bilsinlər
  məşq zamanı idarə panelləri özünü apardı.

[Taikai Anchor Runbook](./taikai-anchor-runbook.md) üçün yenidən baxın.
tam Sev1 proseduru və sübut yoxlama siyahısı. Bu səhifə yalnız onu çəkir
SN13-C-nin getməzdən əvvəl tələb etdiyi tablosuna xüsusi təlimat 🈺.