---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-transition-notes
title: Nexus ٹرانزیشن نوٹس
description: `docs/source/nexus_transition_notes.md` کا آئینہ، جو Phase B ٹرانزیشن ثبوت، آڈٹ شیڈول، اور میٹیگیشنز کو کور کرتا ہے۔
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus ٹرانزیشن نوٹس

یہ لاگ **Phase B - Nexus Transition Foundations** کے باقی کام کو اس وقت تک ٹریک کرتا ہے جب تک multi-lane لانچ چیک لسٹ مکمل نہ ہو جائے۔ یہ `roadmap.md` میں milestones اندراجات کی تکمیل کرتا ہے اور B1-B4 میں حوالہ دی گئی evidence کو ایک جگہ رکھتا ہے تاکہ governance، SRE اور SDK لیڈز ایک ہی source of truth شیئر کر سکیں۔

## اسکوپ اور cadence

- routed-trace آڈٹس اور telemetry guardrails (B1/B2)، governance سے منظور شدہ config delta سیٹ (B3)، اور multi-lane launch rehearsal follow-ups (B4) کو کور کرتا ہے۔
- یہ عارضی cadence نوٹ کی جگہ لیتا ہے جو پہلے یہاں تھا؛ Q1 2026 آڈٹ کے بعد تفصیلی رپورٹ `docs/source/nexus_routed_trace_audit_report_2026q1.md` میں ہے، جبکہ یہ صفحہ جاری شیڈول اور mitigation رجسٹر رکھتا ہے۔
- ہر routed-trace ونڈو، governance ووٹ، یا launch rehearsal کے بعد ٹیبلز اپ ڈیٹ کریں۔ جب artifacts حرکت کریں تو نئی جگہ کو اسی صفحے میں ریفلیکٹ کریں تاکہ downstream docs (status, dashboards, SDK portals) ایک مستحکم anchor سے لنک کر سکیں۔

## Evidence snapshot (2026 Q1-Q2)

| ورک اسٹریم | ثبوت | مالکان | اسٹیٹس | نوٹس |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | مکمل (Q1 2026) | تین آڈٹ ونڈوز ریکارڈ ہوئیں؛ `TRACE-CONFIG-DELTA` کا TLS lag Q2 rerun میں بند ہوا۔ |
| **B2 - Telemetry remediation & guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مکمل | alert pack، diff bot policy، اور OTLP batch size (`nexus.scheduler.headroom` log + Grafana headroom panel) فراہم ہو چکے ہیں؛ کوئی واورز اوپن نہیں۔ |
| **B3 - Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مکمل | GOV-2026-03-19 ووٹ ریکارڈ ہے؛ signed bundle نیچے والے telemetry pack کو feed کرتا ہے۔ |
| **B4 - Multi-lane launch rehearsal** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مکمل (Q2 2026) | Q2 canary rerun نے TLS lag mitigation بند کیا؛ validator manifest + `.sha256` نے slots 912-936، workload seed `NEXUS-REH-2026Q2` اور rerun میں ریکارڈ شدہ TLS profile hash کو capture کیا۔ |

## سہ ماہی routed-trace آڈٹ شیڈول

| Trace ID | ونڈو (UTC) | نتیجہ | نوٹس |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | پاس | Queue-admission P95 ہدف <=750 ms سے کافی نیچے رہا۔ کوئی ایکشن درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | پاس | OTLP replay hashes `status.md` کے ساتھ منسلک ہیں؛ SDK diff bot parity نے zero drift کی تصدیق کی۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | حل شدہ | TLS lag Q2 rerun میں بند ہوا؛ `NEXUS-REH-2026Q2` کے telemetry pack میں TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ریکارڈ ہے (دیکھیں `artifacts/nexus/tls_profile_rollout_2026q2/`) اور صفر stragglers ہیں۔ |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | پاس | Workload seed `NEXUS-REH-2026Q2`; telemetry pack + manifest/digest `artifacts/nexus/rehearsals/2026q1/` میں (slot range 912-936) اور agenda `artifacts/nexus/rehearsals/2026q2/` میں ہے۔ |

آنے والے سہ ماہیوں میں نئی قطاریں شامل کریں اور جب ٹیبل موجودہ سہ ماہی سے بڑا ہو جائے تو مکمل اندراجات کو appendix میں منتقل کریں۔ routed-trace رپورٹس یا governance minutes سے اس سیکشن کو `#quarterly-routed-trace-audit-schedule` anchor کے ذریعے ریفرنس کریں۔

## Mitigations اور backlog items

| آئٹم | تفصیل | مالک | ہدف | اسٹیٹس / نوٹس |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` کے دوران پیچھے رہ جانے والے TLS profile کی propagation مکمل کریں، rerun evidence capture کریں، اور mitigation log بند کریں۔ | @release-eng, @sre-core | Q2 2026 routed-trace ونڈو | بند - TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کو `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` میں capture کیا گیا؛ rerun نے zero stragglers کی تصدیق کی۔ |
| `TRACE-MULTILANE-CANARY` prep | Q2 rehearsal شیڈول کریں، telemetry pack کے ساتھ fixtures منسلک کریں، اور یقینی بنائیں کہ SDK harnesses validate شدہ helper reuse کریں۔ | @telemetry-ops, SDK Program | 2026-04-30 planning call | مکمل - agenda `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` میں محفوظ ہے جس میں slot/workload metadata شامل ہے؛ harness reuse tracker میں نوٹ ہے۔ |
| Telemetry pack digest rotation | ہر rehearsal/release سے پہلے `scripts/telemetry/validate_nexus_telemetry_pack.py` چلائیں اور digests کو config delta tracker کے ساتھ لاگ کریں۔ | @telemetry-ops | ہر release candidate | مکمل - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں جاری ہوئے (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests tracker اور evidence index میں کاپی کیے گئے۔ |

## Config delta bundle integration

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` canonical diff summary ہے۔ جب نئے `defaults/nexus/*.toml` یا genesis changes آئیں تو پہلے tracker اپ ڈیٹ کریں اور پھر خلاصہ یہاں شامل کریں۔
- Signed config bundles rehearsal telemetry pack کو feed کرتے ہیں۔ یہ pack، جو `scripts/telemetry/validate_nexus_telemetry_pack.py` سے validate ہوتا ہے، config delta evidence کے ساتھ publish ہونا چاہیے تاکہ operators B4 میں استعمال ہونے والے بالکل وہی artifacts replay کر سکیں۔
- Iroha 2 bundles lanes کے بغیر رہتے ہیں: `nexus.enabled = false` والی configs اب lane/dataspace/routing overrides کو reject کرتی ہیں جب تک Nexus profile (`--sora`) enable نہ ہو، اس لئے single-lane templates سے `nexus.*` sections ہٹا دیں۔
- governance vote log (GOV-2026-03-19) کو tracker اور اس نوٹ دونوں سے لنک رکھیں تاکہ مستقبل کے votes اسی فارمیٹ کو دوبارہ دریافت کیے بغیر استعمال کر سکیں۔

## Launch rehearsal follow-ups

- `docs/source/runbooks/nexus_multilane_rehearsal.md` canary پلان، participant roster اور rollback steps کو محفوظ کرتا ہے؛ جب lane topology یا telemetry exporters بدلیں تو runbook اپ ڈیٹ کریں۔
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 اپریل کے rehearsal کے ہر artifact کو لسٹ کرتا ہے اور اب Q2 prep notes/agenda بھی رکھتا ہے۔ مستقبل کے rehearsals اسی tracker میں شامل کریں تاکہ evidence monotonic رہے۔
- OTLP collector snippets اور Grafana exports (دیکھیں `docs/source/telemetry.md`) تب publish کریں جب exporter batching guidance بدلے؛ Q1 update نے headroom alerts سے بچنے کے لئے batch size کو 256 samples تک بڑھایا۔
- multi-lane CI/test evidence اب `integration_tests/tests/nexus/multilane_pipeline.rs` میں ہے اور `Nexus Multilane Pipeline` workflow (`.github/workflows/integration_tests_multilane.yml`) کے تحت چلتا ہے، جس نے `pytests/nexus/test_multilane_pipeline.py` ریفرنس کو ریٹائر کیا؛ `defaults/nexus/config.toml` کے hash (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) کو tracker کے ساتھ sync رکھیں جب rehearsal bundles ریفریش ہوں۔

## Runtime lane lifecycle

- Runtime lane lifecycle plans اب dataspace bindings validate کرتے ہیں اور Kura/tiered storage reconciliation fail ہونے پر abort کر دیتے ہیں، جس سے catalog بغیر تبدیلی کے رہتا ہے۔ helpers ریٹائر lanes کے cached relays کو prune کرتے ہیں تاکہ merge-ledger synthesis پرانی proofs reuse نہ کرے۔
- Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) کے ذریعے plans apply کریں تاکہ lanes کو restart کے بغیر add/retire کیا جا سکے؛ routing، TEU snapshots اور manifest registries کامیاب plan کے بعد خودکار طور پر reload ہوتے ہیں۔
- Operators کے لئے رہنمائی: اگر plan fail ہو تو missing dataspaces یا storage roots چیک کریں جو بن نہیں سکتے (tiered cold root/Kura lane directories)۔ base paths درست کریں اور دوبارہ کوشش کریں؛ کامیاب plans lane/dataspace telemetry diff دوبارہ emit کرتے ہیں تاکہ dashboards نئی topology دکھا سکیں۔

## NPoS telemetry اور backpressure evidence

Phase B launch rehearsal retro نے deterministic telemetry captures مانگے تھے جو ثابت کریں کہ NPoS pacemaker اور gossip layers اپنی backpressure حدود میں رہتے ہیں۔ `integration_tests/tests/sumeragi_npos_performance.rs` والا integration harness ان scenarios کو چلاتا ہے اور جب نئی metrics آئیں تو JSON summaries (`sumeragi_baseline_summary::<scenario>::...`) emit کرتا ہے۔ لوکل چلانے کے لئے:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` یا `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` سیٹ کریں تاکہ زیادہ stress والی topologies دیکھ سکیں؛ default values B4 میں استعمال ہونے والے 1 s/`k=3` collector پروفائل کو reflect کرتے ہیں۔

| Scenario / test | Coverage | Key telemetry |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | rehearsal block time کے ساتھ 12 rounds چلاتا ہے تاکہ EMA latency envelopes، queue depths اور redundant-send gauges ریکارڈ ہوں، پھر evidence bundle serialize کیا جاتا ہے۔ | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | transaction queue کو بھر کر یقینی بناتا ہے کہ admission deferrals deterministically trigger ہوں اور queue capacity/saturation counters export کرے۔ | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | pacemaker jitter اور view timeouts sample کرتا ہے جب تک +/-125 permille بینڈ نافذ ہونے کا ثبوت نہ ملے۔ | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | بڑے RBC payloads کو store کی soft/hard حدود تک push کرتا ہے تاکہ sessions اور bytes counters کا بڑھنا، واپس آنا، اور بغیر overflow کے stabilize ہونا دکھایا جا سکے۔ | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | retransmits force کرتا ہے تاکہ redundant-send ratio gauges اور collectors-on-target counters آگے بڑھیں، اور ظاہر ہو کہ retro والی telemetry end-to-end جڑی ہے۔ | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | deterministically spaced chunks drop کرتا ہے تاکہ backlog monitors خاموش drain کے بجائے faults raise کریں۔ | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

جب بھی governance یہ ثبوت مانگے کہ backpressure alarms rehearsal topology سے match کرتے ہیں، harness کے پرنٹ شدہ JSON lines کے ساتھ Prometheus scrape بھی منسلک کریں۔

## Update checklist

1. نئی routed-trace ونڈوز شامل کریں اور جب سہ ماہی بدلے تو پرانی اندراجات منتقل کریں۔
2. Alertmanager follow-up کے بعد mitigation table اپ ڈیٹ کریں، چاہے action ticket بند کرنا ہی کیوں نہ ہو۔
3. جب config deltas بدلیں، tracker، یہ نوٹ، اور telemetry pack digests کی فہرست کو اسی pull request میں اپ ڈیٹ کریں۔
4. کسی بھی نئے rehearsal/telemetry artefact کو یہاں لنک کریں تاکہ مستقبل کی roadmap updates ایک ہی دستاویز کو ریفرنس کریں، بکھری ہوئی ad-hoc نوٹس نہیں۔

## Evidence index

| اثاثہ | مقام | نوٹس |
|-------|----------|-------|
| Routed-trace audit report (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Phase B1 evidence کے لئے canonical source؛ پورٹل پر `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` میں mirror ہے۔ |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA diff summaries، reviewers کے initials، اور GOV-2026-03-19 vote log شامل ہیں۔ |
| Telemetry remediation plan | `docs/source/nexus_telemetry_remediation_plan.md` | alert pack، OTLP batch size، اور B2 سے جڑی export budget guardrails کو دستاویز کرتا ہے۔ |
| Multi-lane rehearsal tracker | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 اپریل rehearsal artifacts، validator manifest/digest، Q2 notes/agenda اور rollback evidence کی فہرست۔ |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | slot range 912-936، seed `NEXUS-REH-2026Q2` اور governance bundles کے artifacts hashes ریکارڈ کرتا ہے۔ |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Q2 rerun کے دوران capture شدہ منظور شدہ TLS profile hash؛ routed-trace appendices میں cite کریں۔ |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 rehearsal planning نوٹس (ونڈو، slot range، workload seed، action owners)۔ |
| Launch rehearsal runbook | `docs/source/runbooks/nexus_multilane_rehearsal.md` | staging -> execution -> rollback کے لئے عملی checklist؛ lane topology یا exporter guidance بدلنے پر اپ ڈیٹ کریں۔ |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro میں حوالہ دیا گیا CLI؛ pack بدلنے پر digests کو tracker کے ساتھ آرکائیو کریں۔ |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | multi-lane configs کے لئے `nexus.enabled = true` ثابت کرتا ہے، Sora catalog hashes محفوظ رکھتا ہے، اور `ConfigLaneRouter` کے ذریعے lane-local Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) provision کر کے artefact digests شائع کرتا ہے۔ |
