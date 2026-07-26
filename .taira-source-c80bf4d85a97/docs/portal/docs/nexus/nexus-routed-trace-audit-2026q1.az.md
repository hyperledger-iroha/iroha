---
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd9f85b2c7845414c27016f699da179e13c41c9b9e0ce5b178ab88a950744500
source_last_modified: "2025-12-29T18:16:35.142540+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-routed-trace-audit-2026q1
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/nexus_routed_trace_audit_report_2026q1.md`-i əks etdirir. Qalan tərcümələr yerə düşənə qədər hər iki nüsxəni düzülmüş saxlayın.
:::

# 2026 R1 Marşrutlu-İzləmə Audit Hesabatı (B1)

Yol xəritəsi elementi **B1 — Marşrutlanmış İz Auditləri və Telemetriya Baseline** tələb edir
Nexus marşrutlu izləmə proqramının rüblük nəzərdən keçirilməsi. Bu hesabat sənədləşdirilir
Q12026 audit pəncərəsi (yanvar-mart) beləliklə, idarəetmə şurası müqaviləni imzalaya bilər.
Q2 buraxılış məşqlərindən əvvəl telemetriya duruş.

## Əhatə dairəsi və vaxt qrafiki

| İz ID | Pəncərə (UTC) | Məqsəd |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Çox zolaqlı aktivləşdirmədən əvvəl zolağa giriş histoqramlarını, növbə dedi-qodularını və xəbərdarlıq axınını yoxlayın. |
| `TRACE-TELEMETRY-BRIDGE` | 24-02-2026 10:00-10:45 | AND4/AND7 mərhələlərindən əvvəl OTLP təkrarını, fərqli bot paritetini və SDK telemetriya qəbulunu təsdiq edin. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 kəsilməsindən əvvəl idarəetmə tərəfindən təsdiqlənmiş `iroha_config` deltalarını və geri çəkilməyə hazırlığı təsdiq edin. |

Hər bir məşq marşrutu izləmə ilə istehsala bənzər topologiya üzrə aparılırdı
cihazlar aktivdir (`nexus.audit.outcome` telemetriya + Prometheus sayğacları),
Alertmanager qaydaları yükləndi və sübut `docs/examples/`-ə ixrac edildi.

## Metodologiya

1. **Telemetri kolleksiyası.** Bütün qovşaqlar strukturlaşdırılmışdır
   `nexus.audit.outcome` hadisəsi və onu müşayiət edən göstəricilər
   (`nexus_audit_outcome_total*`). Köməkçi
   `scripts/telemetry/check_nexus_audit_outcome.py` JSON jurnalını qeyd etdi,
   hadisə statusunu təsdiqlədi və faydalı yükü altında arxivləşdirdi
   `docs/examples/nexus_audit_outcomes/`.【scripts/telemetry/check_nexus_audit_outcome.py:1】
2. **Xəbərdarlığın təsdiqi.** `dashboards/alerts/nexus_audit_rules.yml` və onun testi
   qoşqu xəbərdarlığın səs-küy hədlərini və faydalı yük şablonunun qalmasını təmin etdi
   ardıcıl. CI `dashboards/alerts/tests/nexus_audit_rules.test.yml` üzərində işləyir
   hər dəyişiklik; eyni qaydalar hər pəncərə zamanı əl ilə həyata keçirilirdi.
3. **İdarəetmə panelinin tutulması.** Operatorlar yönləndirilmiş izləmə panellərini ixrac etdi
   `dashboards/grafana/soranet_sn16_handshake.json` (əl sıxma sağlamlığı) və
   növbə sağlamlığını audit nəticələri ilə əlaqələndirmək üçün telemetriya icmalı panelləri.
4. **Nəzərçi qeydləri.** İdarəetmə katibi rəyçinin baş hərflərini daxil etdi,
   qərar və [Nexus keçid qeydlərində](./nexus-transition-notes) hər hansı yumşaldıcı biletlər
   və konfiqurasiya delta izləyicisi (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Tapıntılar

| İz ID | Nəticə | Sübut | Qeydlər |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Keçid | Alert yanğın/bərpa ekran görüntüləri (daxili keçid) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` təkrarı; [Nexus keçid qeydlərində](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) qeydə alınan telemetriya fərqləri. | Növbə qəbulu P95 612ms olaraq qaldı (hədəf ≤750ms). Heç bir təqib tələb olunmur. |
| `TRACE-TELEMETRY-BRIDGE` | Keçid | Arxivləşdirilmiş nəticə yükü `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` üstəgəl `status.md`-də qeydə alınmış OTLP replay heş. | SDK redaksiya duzları Rust bazasına uyğundur; diff bot sıfır delta olduğunu bildirdi. |
| `TRACE-CONFIG-DELTA` | Keçid (azaltma qapalı) | İdarəetmə izləyicisi girişi (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS profil manifest (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + telemetriya paketi manifest (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | 2-ci rübün təkrarı təsdiqlənmiş TLS profilini heşləşdirdi və sıfır stragglerləri təsdiqlədi; telemetriya manifest 912–936 slot diapazonunu və iş yükünün toxumunu `NEXUS-REH-2026Q2` qeyd edir. |

Bütün izlər öz daxilində ən azı bir `nexus.audit.outcome` hadisəsi meydana gətirdi
Alertmanager qoruyucularına (`NexusAuditOutcomeFailure`) cavab verən pəncərələr
rüb üçün yaşıl qaldı).

## Təqiblər

- Yönləndirilmiş iz əlavəsi `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` TLS hash ilə yeniləndi;
  azaldılması `NEXUS-421` keçid qeydlərində bağlanıb.
- Arxivə xam OTLP təkrarları və Torii diff artefaktları əlavə etməyə davam edin
  Android AND4/AND7 rəyləri üçün paritet sübutlarını gücləndirin.
- Qarşıdan gələn `TRACE-MULTILANE-CANARY` məşqlərinin eyni şeyi təkrar istifadə etdiyini təsdiqləyin
  telemetriya köməkçisi, beləliklə, Q2 imzalanması təsdiqlənmiş iş axınından faydalanır.

## Artefakt İndeksi

| Aktiv | Məkan |
|-------|----------|
| Telemetriya təsdiqləyicisi | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Xəbərdarlıq qaydaları və testləri | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Nümunə nəticəsi yükü | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Konfiqurasiya delta izləyicisi | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Marşrutlu-iz cədvəli və qeydlər | [Nexus keçid qeydləri](./nexus-transition-notes) |

Bu hesabat, yuxarıdakı artefaktlar və xəbərdarlıq/telemetri ixracı olmalıdır
rüb üçün B1-i bağlamaq üçün idarəetmə qərarları jurnalına əlavə olunur.