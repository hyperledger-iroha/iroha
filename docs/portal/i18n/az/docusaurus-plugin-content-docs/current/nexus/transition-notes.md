---
id: nexus-transition-notes
lang: az
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus transition notes
description: Mirror of `docs/source/nexus_transition_notes.md`, covering Phase B transition evidence, audit schedule, and mitigations.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus Keçid Qeydləri

Bu jurnal davam edən **PhaseB — Nexus Transition Foundations** işini izləyir.
çox zolaqlı buraxılış yoxlama siyahısı bitənə qədər. Bu mərhələni tamamlayır
`roadmap.md`-də qeydlər və B1-B4 tərəfindən istinad edilən sübutları bir yerdə saxlayır
beləliklə, idarəetmə, SRE və SDK rəhbərləri eyni həqiqət mənbəyini paylaşa bilər.

## Əhatə dairəsi və ritm

- Marşrutlanmış iz auditlərini və telemetriya qoruyucularını (B1/B2) əhatə edir
  idarəetmə tərəfindən təsdiqlənmiş konfiqurasiya delta dəsti (B3) və çox zolaqlı işə salınma
  məşq təqibləri (B4).
- Əvvəllər burada yaşamış müvəqqəti kadans notunu əvəz edir; 2026-cı ildən
  Q1 audit ətraflı hesabatda yerləşir
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`, bu səhifənin sahibi isə
  iş qrafiki və təsirin azaldılması reyestri.
- Hər marşrutlaşdırılmış izləmə pəncərəsindən, idarəetmə səsverməsindən və ya işə salındıqdan sonra cədvəlləri yeniləyin
  məşq. Artefaktlar hərəkət etdikdə, bu səhifənin içərisində yeni yeri əks etdirin
  beləliklə, aşağı axın sənədləri (status, idarə panelləri, SDK portalları) sabit bir sənədlə əlaqə saxlaya bilər
  lövbər.

## Sübut Snapshot (2026 Q1–Q2)

| İş axını | Sübut | Sahib(lər) | Status | Qeydlər |
|------------|----------|----------|--------|-------|
| **B1 — Marşrutlaşdırılmış izləmə auditləri** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | ✅ Tamamlandı (2026-cı ilin 1-ci rübü) | Üç yoxlama pəncərəsi qeydə alınıb; `TRACE-CONFIG-DELTA`-dən TLS gecikməsi 2-ci rübün təkrarı zamanı bağlandı. |
| **B2 — Telemetriya təmiri və qoruyucu barmaqlıqlar** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | ✅ Tam | Xəbərdarlıq paketi, fərqli bot siyasəti və OTLP toplu ölçüsü (`nexus.scheduler.headroom` log + Grafana başlıq paneli) göndərildi; açıq imtina yoxdur. |
| **B3 — Konfiqurasiya delta təsdiqləri** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | ✅ Tam | GOV-2026-03-19 səs tutuldu; imzalanmış paket aşağıda qeyd olunan telemetriya paketini qidalandırır. |
| **B4 — Çox zolaqlı buraxılış məşqi** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | ✅ Tamamlandı (2026-cı rüb) | 2-ci rüb kanareykanın təkrarı TLS gecikməsinin azaldılmasını bağladı; validator manifest + `.sha256` tutma yuvası diapazonu 912–936, iş yükü toxumu `NEXUS-REH-2026Q2` və təkrar icradan qeydə alınmış TLS profil hashı. |

## Rüblük Marşrutlu-İzləmə Audit Cədvəli

| İz ID | Pəncərə (UTC) | Nəticə | Qeydlər |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ✅ Keçid | Növbəyə giriş P95 ≤750ms hədəfindən xeyli aşağıda qaldı. Heç bir tədbir tələb olunmur. |
| `TRACE-TELEMETRY-BRIDGE` | 24-02-2026 10:00-10:45 | ✅ Keçid | `status.md`-ə əlavə edilmiş OTLP replay heshləri; SDK fərqi bot pariteti sıfır sürüşməni təsdiq etdi. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | ✅ Həll olundu | Q2 təkrar icrası zamanı TLS profilinin gecikməsi bağlandı; `NEXUS-REH-2026Q2` üçün telemetriya paketi TLS profil hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (bax `artifacts/nexus/tls_profile_rollout_2026q2/`) və sıfır stragglerləri qeyd edir. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ Keçid | İş yükü toxumu `NEXUS-REH-2026Q2`; telemetriya paketi + `artifacts/nexus/rehearsals/2026q2/` gündəliyi ilə `artifacts/nexus/rehearsals/2026q1/` (slot diapazonu 912–936) altında manifest/dijest. |

Gələcək dörddəbirlər yeni satırlar əlavə etməli və hərəkət etməlidir
Cədvəl cərəyandan kənara çıxdıqda əlavəyə daxil edilmiş qeydlər
rüb. Bu bölməyə marşrutlaşdırılmış izləmə hesabatlarından və ya idarəetmə protokollarından istinad edin
`#quarterly-routed-trace-audit-schedule` ankerindən istifadə etməklə.

## Təsirlərin azaldılması və gecikmə maddələri

| Maddə | Təsvir | Sahibi | Hədəf | Status / Qeydlər |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` zamanı geridə qalan TLS profilinin yayılmasını tamamlayın, təkrar sübutları əldə edin və təsirin azaldılması jurnalını bağlayın. | @release-eng, @sre-core | 2026-cı rübün 2-ci marşrutu izləmə pəncərəsi | ✅ Qapalı — `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`-də çəkilmiş TLS profil hashı; Rerun heç bir stragglers təsdiqlədi. |
| `TRACE-MULTILANE-CANARY` hazırlıq | 2-ci rübün məşqini planlaşdırın, telemetriya paketinə qurğular əlavə edin və SDK qoşqularının təsdiqlənmiş köməkçidən yenidən istifadə etməsinə əmin olun. | @telemetry-ops, SDK Proqramı | Planlaşdırma zəngi 2026-04-30 | ✅ Tamamlandı — gündəm slot/iş yükü metadatası ilə `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md`-də saxlanılır; qoşquların təkrar istifadəsi izləyicidə qeyd olunub. |
| Telemetriya paketinin fırlanması | Hər məşq/buraxılışdan əvvəl `scripts/telemetry/validate_nexus_telemetry_pack.py`-i işə salın və konfiqurasiya delta izləyicisinin yanında həzmləri qeyd edin. | @telemetry-ops | Hər buraxılış namizədi | ✅ Tamamlandı — `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/`-də buraxıldı (slot diapazonu `912-936`, toxum `NEXUS-REH-2026Q2`); izləyiciyə və sübut indeksinə kopyalanan həzmlər. |

## Delta Bundle İnteqrasiyasını Konfiqurasiya edin

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` olaraq qalır
  kanonik fərq xülasəsi. Yeni `defaults/nexus/*.toml` və ya genezis dəyişdikdə
  yerləşdirin, əvvəlcə həmin izləyicini yeniləyin, sonra burada əsas məqamları əks etdirin.
- İmzalanmış konfiqurasiya paketləri məşq telemetriya paketini qidalandırır. Paket, təsdiqlənmişdir
  `scripts/telemetry/validate_nexus_telemetry_pack.py` tərəfindən dərc edilməlidir
  konfiqurasiya delta sübutları ilə yanaşı operatorlar dəqiqliyi təkrarlaya bilsinlər
  B4 zamanı istifadə olunan artefaktlar.
- Iroha 2 paket zolaqsız qalır: indi `nexus.enabled = false` ilə konfiqurasiyalar
  Nexus profili aktivləşdirilmədikcə, zolaq/məlumat məkanı/marşrutlaşdırma ləğvetmələrini rədd edin
  (`--sora`), beləliklə, tək zolaqlı şablonlardan `nexus.*` hissələrini ayırın.
- İdarəetmə səs jurnalını (GOV-2026-03-19) həm izləyici, həm də izləyici ilə əlaqələndirin
  Bu qeyd, gələcək səslər formatını yenidən kəşf etmədən kopyalaya bilər
  təsdiq ritualı.

## Məşq İzləmələrini başladın

- `docs/source/runbooks/nexus_multilane_rehearsal.md` kanareyka planını çəkir,
  iştirakçı siyahısı və geri çəkilmə addımları; zolaqda olduqda runbook-u yeniləyin
  topologiya və ya telemetriya ixracatçıları dəyişir.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` hər artefaktı sadalayır
  9 aprel məşqi zamanı yoxlanılıb və indi Q2 hazırlıq qeydlərini/gündəmini daşıyır.
  Birdəfəlik açmaq əvəzinə gələcək məşqləri eyni izləyiciyə əlavə edin
  sübutları monoton saxlamaq üçün izləyicilər.
- OTLP kollektor fraqmentlərini və Grafana ixracını dərc edin (bax: `docs/source/telemetry.md`)
  ixracatçının paketləşdirmə təlimatı dəyişdikdə; Q1 yeniləməsi onu vurdu
  boşluq siqnallarının qarşısını almaq üçün partiyanın ölçüsünü 256 nümunəyə çatdırın.
- Çox zolaqlı CI/test sübutları indi yaşayır
  `integration_tests/tests/nexus/multilane_pipeline.rs` və altında çalışır
  `Nexus Multilane Pipeline` iş axını
  (`.github/workflows/integration_tests_multilane.yml`), təqaüdçüləri əvəz edir
  `pytests/nexus/test_multilane_pipeline.py` arayışı; üçün hash saxlamaq
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) sinxronlaşdırılır
  məşq paketlərini təzələyərkən izləyici ilə.

## Runtime Lane Lifecycle

- Runtime zolaqlı həyat dövrü planları indi məlumat məkanı bağlamalarını təsdiqləyir və nə vaxt dayandırılır
  Kür/səviyyəli yaddaş uyğunlaşdırılması uğursuz oldu və kataloqu dəyişməz qaldı. The
  köməkçilər təqaüdə çıxmış zolaqlar üçün keşlənmiş zolaqlı releləri kəsir, beləliklə birləşmiş kitab sintezi
  köhnəlmiş sübutlardan təkrar istifadə etmir.
- Nexus konfiqurasiya/həyat dövrü köməkçiləri vasitəsilə planları tətbiq edin (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) yenidən başlamadan zolaqları əlavə etmək/çıxarmaq; marşrutlaşdırma,
  TEU anlıq görüntüləri və manifest qeydləri uğurlu plandan sonra avtomatik olaraq yenidən yüklənir.
- Operator təlimatı: plan uğursuz olduqda, çatışmayan məlumat məkanlarını və ya yaddaşı yoxlayın
  yaradıla bilməyən köklər (səviyyəli soyuq kök/Kür zolağı kataloqları). düzəltmək
  dəstək yolları və yenidən cəhd edin; uğurlu planlar zolaq/dataspace telemetriyasını yenidən yayır
  diff belədir ki, tablolar yeni topologiyanı əks etdirir.

## NPoS Telemetriya və əks təzyiq sübutu

PhaseB-nin işə salınma-məşq retrosu deterministik telemetriya tələb etdi ki, bunu tutur
NPoS kardiostimulyatorunun və dedi-qodu təbəqələrinin əks təzyiq daxilində qaldığını sübut edin
məhdudiyyətlər. Bu inteqrasiya qoşqu
`integration_tests/tests/sumeragi_npos_performance.rs` bunları həyata keçirir
ssenarilər və JSON xülasələrini yayır (`sumeragi_baseline_summary::<scenario>::…`)
yeni ölçülər düşəndə. Onu yerli olaraq işlədin:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Set `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K`, və ya
Daha yüksək gərginlikli topologiyaları araşdırmaq üçün `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`; the
defoltlar B4-də istifadə olunan 1s/`k=3` kollektor profilini əks etdirir.

| Ssenari / test | Əhatə | Əsas telemetriya |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Sübut paketini seriyalaşdırmadan əvvəl EMA gecikmə zərflərini, növbə dərinliklərini və lazımsız göndərmə ölçülərini qeyd etmək üçün məşq bloklama vaxtı ilə 12 raund bloklayın. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Qəbul təxirə salınmalarının deterministik şəkildə başlamasını və növbənin tutum/doyma sayğaclarını ixrac etməsini təmin etmək üçün əməliyyat növbəsini doldurur. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Konfiqurasiya edilmiş ±125‰ diapazonunun tətbiq olunduğunu sübut edənə qədər kardiostimulyator titrəmə nümunələri alır və fasilələrə baxın. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Seansları və bayt sayğaclarının mağazanı aşmadan qalxdığını, geri çəkildiyini və yerləşdiyini göstərmək üçün böyük RBC yüklərini yumşaq/sərt mağaza limitlərinə itələyir. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Forces retransmissiya edir ki, lazımsız-göndərmə nisbəti ölçücüləri və kollektorlar-hədəf sayğacları irəliləyərək, tələb olunan retro telemetriyanın uçdan-uca simli olduğunu sübut edir. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Arxa plan monitorlarının yükləri səssizcə boşaltmaq əvəzinə nasazlıqları artırdığını yoxlamaq üçün müəyyən aralıqlı parçaları düşür. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |JSON xətlərini qoşqu çaplarını Prometheus qırıntı ilə birləşdirin
Qaçış əsnasında ələ keçirildiyi zaman idarəçilik arxa təzyiqə dair sübut istəsə
həyəcan siqnalları məşq topologiyasına uyğun gəlir.

## Yeniləmə Yoxlama Siyahısı

1. Yeni marşrutlaşdırılmış iz pəncərələri əlavə edin və dörddəbirlər yuvarlanan kimi köhnə pəncərələri ləğv edin.
2. Hər Alertmanager təqibindən sonra yumşaldıcı cədvəli yeniləyin, hətta əgər
   hərəkət bileti bağlamaqdır.
3. Konfiqurasiya deltaları dəyişdikdə, izləyicini, bu qeydi və telemetriyanı yeniləyin
   eyni çəkmə sorğusunda həzm siyahısı paketi.
4. Gələcək yol xəritəsi statusu üçün hər hansı yeni məşq/temetriya artefaktlarını burada birləşdirin
   yeniləmələr səpələnmiş xüsusi qeydlər əvəzinə tək sənədə istinad edə bilər.

## Sübut indeksi

| Aktiv | Məkan | Qeydlər |
|-------|----------|-------|
| Marşrutlaşdırılmış izləmə audit hesabatı (2026-cı ilin 1-ci rübü) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | PhaseB1 sübut üçün kanonik mənbə; `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` altında portal üçün əks olunub. |
| Konfiqurasiya delta izləyicisi | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA fərq xülasələrini, rəyçinin baş hərflərini və GOV-2026-03-19 səs jurnalını ehtiva edir. |
| Telemetriya remediasiya planı | `docs/source/nexus_telemetry_remediation_plan.md` | Xəbərdarlıq paketini, OTLP toplu ölçüsünü və B2 ilə əlaqəli ixrac büdcə qoruyucularını sənədləşdirir. |
| Çox zolaqlı məşq izləyicisi | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 aprel məşq artefaktları, təsdiqləyici manifest/həzm, 2-ci rüb hazırlıq qeydləri/gündəmi və geri çəkilmə sübutlarını sadalayır. |
| Telemetriya paketi manifest/dijest (ən son) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | 912–936 slot diapazonunu, toxum `NEXUS-REH-2026Q2` və idarəetmə paketləri üçün artefakt heşlərini qeyd edir. |
| TLS profil manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | 2-ci rübün təkrarı zamanı əldə edilmiş təsdiq edilmiş TLS profilinin hashı; marşrutlu izləmə əlavələrində istinad edin. |
| TRACE-MULTILANE-CANARY gündəliyi | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | 2-ci rübün məşqi üçün planlaşdırma qeydləri (pəncərə, yuva diapazonu, iş yükü toxumu, fəaliyyət sahibləri). |
| Məşq runbookunu işə salın | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Mərhələ üçün əməliyyat yoxlama siyahısı → icra → geriyə; zolaq topologiyası və ya ixracatçı təlimatı dəyişdikdə yeniləyin. |
| Telemetriya paketi təsdiqləyicisi | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro tərəfindən istinad edilən CLI; paket dəyişdikdə izləyici ilə birlikdə arxiv həzmləri. |
| Çox zolaqlı reqressiya | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Çox zolaqlı konfiqurasiyalar üçün `nexus.enabled = true`-i sübut edir, Sora kataloqu heşlərini qoruyur və artefakt həzmlərini dərc etməzdən əvvəl `ConfigLaneRouter` vasitəsilə zolaqlı yerli Kür/birləşmə jurnalı yollarını (`blocks/lane_{id:03}_{slug}`) təmin edir. |