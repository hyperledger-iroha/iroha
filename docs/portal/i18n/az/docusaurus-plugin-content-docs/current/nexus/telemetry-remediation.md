---
id: nexus-telemetry-remediation
lang: az
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Baxış

Yol xəritəsi elementi **B2 — telemetriya boşluğuna sahiblik** dərc edilmiş plan bağlamasını tələb edir
hər bir əlamətdar Nexus telemetriya boşluğunu siqnala, xəbərdarlıq qoruyucusuna, sahibinə,
son tarix və 2026-cı rübün 1-ci audit pəncərələri başlamazdan əvvəl doğrulama artefaktı.
Bu səhifə `docs/source/nexus_telemetry_remediation_plan.md`-i əks etdirir, buna görə buraxın
mühəndislik, telemetriya əməliyyatları və SDK sahibləri əhatə dairəsini əvvəlcədən təsdiqləyə bilər
routed-trace və `TRACE-TELEMETRY-BRIDGE` məşqləri.

# Boşluq matrisi

| Gap ID | Siqnal və xəbərdarlıq qoruyucusu | Sahib / eskalasiya | Vaxtı (UTC) | Sübut və yoxlama |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | 5 dəqiqə ərzində `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` (`dashboards/alerts/soranet_lane_rules.yml`) işə salındıqda **`SoranetLaneAdmissionLatencyDegraded`** xəbərdarlığı ilə `torii_lane_admission_latency_seconds{lane_id,endpoint}` histoqramı. | `@torii-sdk` (siqnal) + `@telemetry-ops` (xəbərdarlıq); Zəng zamanı Nexus marşrutlu izləmə vasitəsilə artırın. | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` və [I18NT00000005-də arxivləşdirilmiş `dashboards/alerts/tests/soranet_lane_rules.test.yml` və işə salınmış/bərpa edilmiş xəbərdarlığı və Torii `/metrics` qırışını göstərən `TRACE-LANE-ROUTING` sınaq çəkilişi altında xəbərdarlıq testləri qeydlər](./nexus-transition-notes). |
| `GAP-TELEM-002` | `nexus_config_diff_total{knob,profile}` sayğacı qoruyucu `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` qapısı ilə yerləşdirilir (`docs/source/telemetry.md`). | `@nexus-core` (alətlər) → `@telemetry-ops` (xəbərdarlıq); sayğac gözlənilmədən artımlar olduqda idarəetmə növbətçisi səhifələndi. | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`-in yanında saxlanılan idarəçilik quru iş çıxışları; buraxılış yoxlama siyahısına Prometheus sorğu ekran görüntüsü və `StateTelemetry::record_nexus_config_diff` fərqi yaydığını sübut edən jurnaldan çıxarış daxildir. |
| `GAP-TELEM-003` | Arızalar və ya çatışmayan nəticələr >30 dəqiqə davam etdikdə xəbərdarlıqla **`NexusAuditOutcomeFailure`** hadisəsi `TelemetryEvent::AuditOutcome` (metrik `nexus.audit.outcome`) (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (boru kəməri) `@sec-observability` səviyyəsinə yüksəlir. | 2026-02-27 | CI qapısı `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON yüklərini arxivləşdirir və TRACE pəncərəsində müvəffəqiyyət hadisəsi olmadıqda uğursuz olur; marşrutlaşdırılmış izləmə hesabatına əlavə edilmiş xəbərdarlıq ekran görüntüləri. |
| `GAP-TELEM-004` | Ölçmə `nexus_lane_configured_total` qoruyucu barmaqlığı olan `nexus_lane_configured_total != EXPECTED_LANE_COUNT` ilə SRE çağırış üzrə yoxlama siyahısını qidalandırır. | Qovşaqlar uyğun olmayan kataloq ölçülərini bildirdikdə `@telemetry-ops` (ölçü/ixrac) `@nexus-core` səviyyəsinə yüksəlir. | 2026-02-28 | Planlayıcı telemetriya testi `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` emissiyanı sübut edir; operatorlar TRACE məşq paketinə Prometheus diff + `StateTelemetry::set_nexus_catalogs` jurnalından çıxarış əlavə edirlər. |

# Əməliyyat iş axını

1. **Həftəlik triaj.** Sahiblər Nexus hazırlıq çağırışında irəliləyiş barədə məlumat verirlər;
   blokerlər və xəbərdarlıq testi artefaktları `status.md`-də qeyd olunur.
2. **Quru qaçış xəbərdarlığı.** Hər bir xəbərdarlıq qaydası a ilə yanaşı göndərilir
   `dashboards/alerts/tests/*.test.yml` girişi beləliklə CI `promtool testini həyata keçirir
   qoruyucu rels dəyişdikdə qaydaları`.
3. **Audit sübutları.** `TRACE-LANE-ROUTING` və
   `TRACE-TELEMETRY-BRIDGE` məşq edir, zəng zamanı Prometheus sorğusunu çəkir
   nəticələr, xəbərdarlıq tarixçəsi və müvafiq skript çıxışları
   (`scripts/telemetry/check_nexus_audit_outcome.py`,
   Əlaqəli siqnallar üçün `scripts/telemetry/check_redaction_status.py`) və
   onları marşrutlu iz artefaktları ilə saxlayır.
4. **Eskalasiya.** Hər hansı bir qoruyucu məşq edilmiş pəncərədən kənarda alovlanırsa, sahibi
   komanda bu plana istinad edən Nexus hadisə biletini, o cümlədən
   auditlərə davam etməzdən əvvəl metrik snapshot və təsirin azaldılması addımları.

Bu matris nəşr olundu və həm `roadmap.md`, həm də istinad edildi
`status.md` — yol xəritəsi bəndi **B2** indi “məsuliyyət, son tarix,
xəbərdarlıq, yoxlama” qəbul meyarları.