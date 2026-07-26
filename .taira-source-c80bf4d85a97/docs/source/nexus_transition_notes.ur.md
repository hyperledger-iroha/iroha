<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ur
direction: rtl
source: docs/source/nexus_transition_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: baf4e91fdb2c447c453711170e7df58a9a4831b15957052818736e0e7914e8a3
source_last_modified: "2025-12-13T06:33:05.401788+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_transition_notes.md -->

# Nexus ٹرانزیشن نوٹس

یہ لاگ **فیز B - Nexus ٹرانزیشن کی بنیادیں** کے باقی کام کو ٹریک کرتا ہے
جب تک ملٹی لین لانچ چیک لسٹ مکمل نہ ہو جائے۔ یہ `roadmap.md` میں موجود
مائل اسٹون اندراجات کی تکمیل کرتا ہے اور B1-B4 سے جڑی شہادتوں کو ایک
جگہ رکھتا ہے تاکہ گورننس، SRE اور SDK لیڈز ایک ہی source of truth شیئر کریں۔

## دائرہ اور رفتار

- routed-trace آڈٹس اور telemetria guardrails (B1/B2)، گورننس سے منظور شدہ
  config delta سیٹ (B3)، اور ملٹی لین لانچ ریہرسل follow-ups (B4) کو کور کرتا ہے۔
- عارضی cadence نوٹ کو بدلتا ہے جو پہلے یہاں تھا؛ 2026 Q1 آڈٹ کے بعد تفصیلی رپورٹ
  `docs/source/nexus_routed_trace_audit_report_2026q1.md` میں ہے، جبکہ یہ صفحہ
  چلتا شیڈول اور mitigation رجسٹر رکھتا ہے۔
- ہر routed-trace ونڈو، گورننس ووٹ یا لانچ ریہرسل کے بعد ٹیبل اپ ڈیٹ کریں۔ جب
  artifacts منتقل ہوں تو نئی لوکیشن یہیں درج کریں تاکہ downstream docs
  (status, dashboards, SDK portals) مستحکم anchor سے لنک کر سکیں۔

## شواہد کا خلاصہ (2026 Q1-Q2)

| ورک اسٹریم | شواہد | مالکان | اسٹیٹس | نوٹس |
|-----------|--------|--------|--------|------|
| **B1 - Routed-trace آڈٹس** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | مکمل (Q1 2026) | تین آڈٹ ونڈوز ریکارڈ ہوئیں؛ `TRACE-CONFIG-DELTA` کا TLS lag Q2 rerun میں بند ہو گیا۔ |
| **B2 - Telemetria remediation اور guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مکمل | alert pack، diff bot پالیسی اور OTLP batch size (`nexus.scheduler.headroom` log + Grafana headroom panel) جاری؛ کوئی کھلی waiver نہیں۔ |
| **B3 - Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مکمل | GOV-2026-03-19 ووٹ محفوظ؛ signed bundle نیچے والے telemetria pack کو feed کرتا ہے۔ |
| **B4 - Multi-lane لانچ ریہرسل** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مکمل (Q2 2026) | Q2 canary rerun نے TLS lag mitigation بند کی؛ validator manifest + `.sha256` نے slots 912-936، workload seed `NEXUS-REH-2026Q2` اور rerun TLS profile hash محفوظ کیا۔ |

## سہ ماہی routed-trace آڈٹ شیڈول

| Trace ID | ونڈو (UTC) | نتیجہ | نوٹس |
|----------|-------------|-------|------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | پاس | queue-admission P95 ہدف <=750 ms سے کافی کم رہا۔ کوئی کارروائی درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | پاس | OTLP replay hashes `status.md` کے ساتھ منسلک؛ SDK diff bot نے zero drift تصدیق کی۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | حل شدہ | TLS lag Q2 rerun میں بند؛ `NEXUS-REH-2026Q2` telemetria pack نے TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ریکارڈ کیا (دیکھیں `artifacts/nexus/tls_profile_rollout_2026q2/`) اور صفر stragglers۔ |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | پاس | workload seed `NEXUS-REH-2026Q2`; telemetria pack + manifest/digest `artifacts/nexus/rehearsals/2026q1/` (slots 912-936) میں، اور agenda `artifacts/nexus/rehearsals/2026q2/` میں۔ |

آنے والے quarters میں نئی قطاریں شامل کریں اور مکمل اندراجات کو appendix میں
منتقل کریں جب ٹیبل موجودہ quarter سے بڑا ہو۔ اس سیکشن کو routed-trace رپورٹس
یا governance minutes سے `#quarterly-routed-trace-audit-schedule` anchor کے
ذریعے ریفرنس کریں۔

## Mitigation اور backlog آئٹمز

| آئٹم | تفصیل | مالک | ہدف | اسٹیٹس / نوٹس |
|------|-------|------|------|---------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` کے دوران lagging TLS profile کی propagation مکمل کریں، rerun ثبوت محفوظ کریں، اور mitigation لاگ بند کریں۔ | @release-eng, @sre-core | Q2 2026 routed-trace ونڈو | بند - TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` میں محفوظ؛ rerun نے stragglers ختم ہونے کی تصدیق کی۔ |
| `TRACE-MULTILANE-CANARY` تیاری | Q2 ریہرسل شیڈول کریں، fixtures telemetria pack کے ساتھ لگائیں، اور SDK harness کو validated helper دوبارہ استعمال کرنے دیں۔ | @telemetry-ops, SDK Program | پلاننگ کال 2026-04-30 | مکمل - agenda `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` میں slot/workload metadata کے ساتھ محفوظ؛ harness reuse tracker میں نوٹ۔ |
| Telemetria pack digest rotation | ہر rehearsal/release سے پہلے `scripts/telemetry/validate_nexus_telemetry_pack.py` چلائیں اور digests کو config delta tracker کے ساتھ لاگ کریں۔ | @telemetry-ops | ہر release candidate | مکمل - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں (slots `912-936`, seed `NEXUS-REH-2026Q2`); digests tracker اور evidence index میں کاپی۔ |

## Config delta bundle انضمام

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` canonical diff summary
  رہتا ہے۔ جب نئے `defaults/nexus/*.toml` یا genesis تبدیلیاں آئیں تو پہلے tracker
  اپ ڈیٹ کریں اور پھر یہاں highlights شامل کریں۔
- signed config bundles rehearsal telemetria pack کو feed کرتے ہیں۔ یہ pack
  `scripts/telemetry/validate_nexus_telemetry_pack.py` سے validate ہو کر شائع ہونا
  چاہیے تاکہ آپریٹرز B4 میں استعمال ہونے والے artifacts دوبارہ چلا سکیں۔
- Iroha 2 bundles lane-free رہتے ہیں: `nexus.enabled = false` configs اب
  lane/dataspace/routing overrides رد کرتے ہیں جب تک Nexus profile فعال نہ ہو (`--sora`)
  اس لئے single-lane templates سے `nexus.*` سیکشن ہٹائیں۔
- governance vote log (GOV-2026-03-19) کو tracker اور اس نوٹ میں لنک رکھیں تاکہ
  مستقبل کے ووٹس بغیر re-discovery اسی فارمیٹ کو اپنائیں۔

## Launch rehearsal follow-ups

- `docs/source/runbooks/nexus_multilane_rehearsal.md` canary پلان، شرکت کنندگان کی
  فہرست، اور rollback steps کو محفوظ کرتا ہے؛ lane topology یا telemetria exporters
  بدلنے پر runbook اپ ڈیٹ کریں۔
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 اپریل ریہرسل کے تمام
  artifacts لسٹ کرتا ہے اور اب Q2 prep notes/agenda رکھتا ہے۔ مستقبل کے rehearsals
  کو اسی tracker میں شامل کریں تاکہ evidence monotonic رہے۔
- OTLP collector snippets اور Grafana exports (دیکھیں `docs/source/telemetry.md`)
  اس وقت شائع کریں جب exporter batching guidance بدلے؛ Q1 اپ ڈیٹ نے batch size کو
  256 samples تک بڑھایا تاکہ headroom alerts نہ آئیں۔
- Multi-lane CI/test evidence اب
  `integration_tests/tests/nexus/multilane_pipeline.rs` میں ہے اور
  `Nexus Multilane Pipeline`
  (`.github/workflows/integration_tests_multilane.yml`) workflow کے تحت چلتا ہے،
  جس نے پرانا `pytests/nexus/test_multilane_pipeline.py` بدل دیا؛ rehearsal bundles
  ریفریش کرتے وقت `defaults/nexus/config.toml` کا hash (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) tracker کے ساتھ
  sync رکھیں۔

## Runtime lane lifecycle

- Runtime lane lifecycle plans اب dataspace bindings validate کرتے ہیں اور اگر Kura/tiered
  storage reconciliation fail ہو تو abort کرتے ہیں، catalog برقرار رہتا ہے۔ helpers
  retired lanes کے cached lane relays prune کرتے ہیں تاکہ merge-ledger synthesis پرانی
  proofs دوبارہ نہ استعمال کرے۔
- Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`)
  کے ذریعے plans apply کریں تاکہ restart کے بغیر lanes add/retire ہوں؛ routing، TEU snapshots
  اور manifest registries کامیاب plan کے بعد خود reload ہوتے ہیں۔
- Operator guidance: plan fail ہو تو missing dataspaces یا storage roots چیک کریں جو
  بن نہیں پا رہے (tiered cold root / Kura lane directories). paths درست کریں اور retry کریں؛
  کامیاب plans lane/dataspace telemetria diff دوبارہ emit کریں تاکہ dashboards نئی topology دکھائیں۔

## NPoS telemetria اور backpressure ثبوت

Phase B launch-rehearsal retro نے deterministic telemetria captures مانگے جو ثابت کریں کہ
NPoS pacemaker اور gossip layers backpressure limits کے اندر رہتے ہیں۔
`integration_tests/tests/sumeragi_npos_performance.rs` والا integration harness یہ
scenarios چلتا ہے اور JSON summaries (`sumeragi_baseline_summary::<scenario>::...`) نکالتا ہے
جب نئی metrics آتی ہیں۔ مقامی طور پر چلائیں:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` یا
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` سیٹ کر کے زیادہ stress والی topologies دیکھیں؛
defaults 1 s/`k=3` collector پروفائل سے میچ کرتے ہیں جو B4 میں استعمال ہوا۔

| Scenario / test | Coverage | Key telemetria |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | rehearsal block time کے ساتھ 12 rounds بلاک کر کے EMA latency envelopes، queue depths اور redundant-send gauges ریکارڈ کرتا ہے، evidence bundle serialize کرنے سے پہلے۔ | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | transaction queue کو flood کر کے admission deferrals کو deterministically trigger کرتا ہے اور capacity/saturation counters export کرواتا ہے۔ | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | pacemaker jitter اور view timeouts sample کر کے ثابت کرتا ہے کہ +/-125 permille band نافذ ہے۔ | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | بڑے RBC payloads soft/hard store limits تک بھیجتا ہے تاکہ sessions اور byte counters بڑھیں، پیچھے ہٹیں، اور store کو overrun کئے بغیر settle ہوں۔ | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | retransmits force کر کے redundant-send gauges اور collectors-on-target counters کو آگے بڑھاتا ہے، اور end-to-end telemetria ثابت کرتا ہے۔ | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | deterministically spaced chunks drop کر کے verify کرتا ہے کہ backlog monitors faults raise کرتے ہیں، payloads خاموشی سے drain نہیں ہوتے۔ | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

harness کی JSON lines اور Prometheus scrape ساتھ لگائیں جب governance یہ دیکھنا چاہے کہ
backpressure alarms rehearsal topology کے مطابق ہیں۔

## اپ ڈیٹ چیک لسٹ

1. نئے routed-trace ونڈوز شامل کریں اور quarters بدلنے پر پرانے ہٹا دیں۔
2. ہر Alertmanager follow-up کے بعد mitigation ٹیبل اپ ڈیٹ کریں، چاہے action ticket بند کرنا ہی ہو۔
3. config deltas بدلیں تو tracker، یہ نوٹ اور telemetria pack digests کی فہرست ایک ہی PR میں اپ ڈیٹ کریں۔
4. نئے rehearsal/telemetria artifacts یہاں لنک کریں تاکہ مستقبل کی status updates ایک ہی ڈاک پر ریفرنس دیں۔

## Evidence index

| Asset | Location | Notes |
|-------|----------|-------|
| Routed-trace audit report (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Phase B1 evidence کے لئے canonical ماخذ؛ portal mirror `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA summaries، reviewer initials، اور GOV-2026-03-19 vote log شامل۔ |
| Telemetry remediation plan | `docs/source/nexus_telemetry_remediation_plan.md` | alert pack، OTLP batch sizing، اور export budget guardrails (B2) دستاویز کرتا ہے۔ |
| Multi-lane rehearsal tracker | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Apr 9 rehearsal artifacts، validator manifest/digest، Q2 prep notes/agenda، اور rollback evidence۔ |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | slots 912-936، seed `NEXUS-REH-2026Q2`، اور governance bundle hashes ریکارڈ کرتا ہے۔ |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Q2 rerun میں منظور شدہ TLS profile hash؛ routed-trace appendices میں cite کریں۔ |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 rehearsal planning notes (window، slot range، workload seed، action owners). |
| Launch rehearsal runbook | `docs/source/runbooks/nexus_multilane_rehearsal.md` | staging -> execution -> rollback operational checklist؛ lane topology یا exporter guidance بدلنے پر اپ ڈیٹ کریں۔ |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro میں مطلوب CLI؛ pack بدلنے پر digests tracker کے ساتھ archive کریں۔ |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | `nexus.enabled = true` کو ثابت کرتا ہے، Sora catalog hashes محفوظ رکھتا ہے، اور `ConfigLaneRouter` کے ذریعے مقامی Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) provision کر کے artifact digests شائع کرتا ہے۔ |

</div>
