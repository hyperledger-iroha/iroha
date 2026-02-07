---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 90869bf46975e3ac40cf606b2ac27597f81e34acbce25261dc9f588e0fac3103
source_last_modified: "2026-01-06T05:56:41.006216+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-transition-notes
title: Nexus transition notes
description: Mirror of `docs/source/nexus_transition_notes.md`, covering Phase B transition evidence, audit schedule, and mitigations.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus Transition Notes

This log tracks the lingering **Phase B — Nexus Transition Foundations** work
until the multi-lane launch checklist finishes. It supplements the milestone
entries in `roadmap.md` and keeps the evidence referenced by B1–B4 in one place
so governance, SRE, and SDK leads can share the same source of truth.

## Scope & Cadence

- Covers the routed-trace audits and telemetry guardrails (B1/B2), the
  governance-approved configuration delta set (B3), and the multi-lane launch
  rehearsal follow-ups (B4).
- Replaces the temporary cadence note that previously lived here; as of the 2026
  Q1 audit the detailed report resides in
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`, while this page owns
  the running schedule and mitigation register.
- Update the tables after each routed-trace window, governance vote, or launch
  rehearsal. Whenever artefacts move, mirror the new location inside this page
  so downstream docs (status, dashboards, SDK portals) can link to a stable
  anchor.

## Evidence Snapshot (2026 Q1–Q2)

| Workstream | Evidence | Owner(s) | Status | Notes |
|------------|----------|----------|--------|-------|
| **B1 — Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | ✅ Complete (Q1 2026) | Three audit windows recorded; TLS lag from `TRACE-CONFIG-DELTA` closed during the Q2 rerun. |
| **B2 — Telemetry remediation & guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | ✅ Complete | Alert pack, diff bot policy, and OTLP batch sizing (`nexus.scheduler.headroom` log + Grafana headroom panel) shipped; no open waivers. |
| **B3 — Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | ✅ Complete | GOV-2026-03-19 vote captured; signed bundle feeds the telemetry pack noted below. |
| **B4 — Multi-lane launch rehearsal** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | ✅ Complete (Q2 2026) | Q2 canary rerun closed the TLS lag mitigation; validator manifest + `.sha256` capture slot range 912–936, workload seed `NEXUS-REH-2026Q2`, and the recorded TLS profile hash from the rerun. |

## Quarterly Routed-Trace Audit Schedule

| Trace ID | Window (UTC) | Outcome | Notes |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ✅ Pass | Queue-admission P95 stayed well below the ≤750 ms target. No action required. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ✅ Pass | OTLP replay hashes attached to `status.md`; SDK diff bot parity confirmed zero drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ✅ Resolved | TLS profile lag closed during Q2 rerun; telemetry pack for `NEXUS-REH-2026Q2` records TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (see `artifacts/nexus/tls_profile_rollout_2026q2/`) and zero stragglers. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ Pass | Workload seed `NEXUS-REH-2026Q2`; telemetry pack + manifest/digest under `artifacts/nexus/rehearsals/2026q1/` (slot range 912–936) with agenda in `artifacts/nexus/rehearsals/2026q2/`. |

Future quarters should add new rows and move the
completed entries to an appendix when the table grows beyond the current
quarter. Reference this section from routed-trace reports or governance minutes
using the `#quarterly-routed-trace-audit-schedule` anchor.

## Mitigation & Backlog Items

| Item | Description | Owner | Target | Status / Notes |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Finish propagating the TLS profile that lagged during `TRACE-CONFIG-DELTA`, capture rerun evidence, and close the mitigation log. | @release-eng, @sre-core | Q2 2026 routed-trace window | ✅ Closed — TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` captured in `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; rerun confirmed no stragglers. |
| `TRACE-MULTILANE-CANARY` prep | Schedule the Q2 rehearsal, attach fixtures to the telemetry pack, and ensure SDK harnesses reuse the validated helper. | @telemetry-ops, SDK Program | Planning call 2026-04-30 | ✅ Completed — agenda stored at `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` with slot/workload metadata; harness reuse noted in the tracker. |
| Telemetry pack digest rotation | Run `scripts/telemetry/validate_nexus_telemetry_pack.py` before each rehearsal/release and log digests next to the config delta tracker. | @telemetry-ops | Per release candidate | ✅ Completed — `telemetry_manifest.json` + `.sha256` emitted in `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests copied into the tracker and evidence index. |

## Config Delta Bundle Integration

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` remains the
  canonical diff summary. When new `defaults/nexus/*.toml` or genesis changes
  land, update that tracker first, then mirror the highlights here.
- Signed config bundles feed the rehearsal telemetry pack. The pack, validated
  by `scripts/telemetry/validate_nexus_telemetry_pack.py`, must be published
  alongside the config delta evidence so operators can replay the exact
  artefacts used during B4.
- Iroha 2 bundles remain lane-free: configs with `nexus.enabled = false` now
  reject lane/dataspace/routing overrides unless the Nexus profile is enabled
  (`--sora`), so strip `nexus.*` sections from single-lane templates.
- Keep the governance vote log (GOV-2026-03-19) linked from both the tracker and
  this note so future votes can copy the format without re-discovering the
  approval ritual.

## Launch Rehearsal Follow-Ups

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captures the canary plan,
  participant roster, and rollback steps; refresh the runbook whenever the lane
  topology or telemetry exporters change.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lists every artefact
  checked during the April 9 rehearsal and now carries the Q2 prep notes/agenda.
  Append future rehearsals to the same tracker instead of opening one-off
  trackers to keep evidence monotonic.
- Publish OTLP collector snippets and Grafana exports (see `docs/source/telemetry.md`)
  whenever the exporter batching guidance changes; the Q1 update bumped the
  batch size to 256 samples to prevent headroom alerts.
- Multi-lane CI/test evidence now lives in
  `integration_tests/tests/nexus/multilane_pipeline.rs` and runs under the
  `Nexus Multilane Pipeline` workflow
  (`.github/workflows/integration_tests_multilane.yml`), replacing the retired
  `pytests/nexus/test_multilane_pipeline.py` reference; keep the hash for
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) in sync
  with the tracker when refreshing rehearsal bundles.

## Runtime Lane Lifecycle

- Runtime lane lifecycle plans now validate dataspace bindings and abort when
  Kura/tiered storage reconciliation fails, leaving the catalog unchanged. The
  helpers prune cached lane relays for retired lanes so merge-ledger synthesis
  does not reuse stale proofs.
- Apply plans through the Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) to add/retire lanes without restart; routing,
  TEU snapshots, and manifest registries reload automatically after a successful plan.
- Operator guidance: when a plan fails, check for missing dataspaces or storage
  roots that cannot be created (tiered cold root/Kura lane directories). Fix the
  backing paths and retry; successful plans re-emit the lane/dataspace telemetry
  diff so dashboards reflect the new topology.

## NPoS Telemetry & Backpressure Evidence

Phase B’s launch-rehearsal retro asked for deterministic telemetry captures that
prove the NPoS pacemaker and gossip layers stay within their backpressure
limits. The integration harness at
`integration_tests/tests/sumeragi_npos_performance.rs` exercises those
scenarios and emits JSON summaries (`sumeragi_baseline_summary::<scenario>::…`)
whenever new metrics land. Run it locally with:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Set `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K`, or
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` to explore higher-stress topologies; the
defaults mirror the 1 s/`k=3` collector profile used in B4.

| Scenario / test | Coverage | Key telemetry |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Blocks 12 rounds with the rehearsal block time to record EMA latency envelopes, queue depths, and redundant-send gauges before serialising the evidence bundle. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Floods the transaction queue to ensure admission deferrals kick in deterministically and that the queue exports capacity/saturation counters. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Samples pacemaker jitter and view timeouts until it proves the configured ±125 ‰ band is enforced. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pushes large RBC payloads to the soft/hard store limits to show sessions and byte counters climb, back off, and settle without overrunning the store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Forces retransmits so the redundant-send ratio gauges and collectors-on-target counters advance, proving the telemetry the retro requested is wired end-to-end. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Drops deterministically spaced chunks to verify backlog monitors raise faults instead of silently draining payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Attach the JSON lines the harness prints together with the Prometheus scrape
captured during the run whenever governance asks for evidence that backpressure
alarms match the rehearsal topology.

## Update Checklist

1. Append new routed-trace windows and retire old ones as quarters roll over.
2. Update the mitigation table after every Alertmanager follow-up, even if the
   action is to close the ticket.
3. When config deltas change, update the tracker, this note, and the telemetry
   pack digest list in the same pull request.
4. Link any new rehearsal/telemetry artefacts here so future roadmap status
   updates can reference a single document instead of scattered ad-hoc notes.

## Evidence Index

| Asset | Location | Notes |
|-------|----------|-------|
| Routed-trace audit report (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Canonical source for Phase B1 evidence; mirrored for the portal under `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contains the TRACE-CONFIG-DELTA diff summaries, reviewer initials, and GOV-2026-03-19 vote log. |
| Telemetry remediation plan | `docs/source/nexus_telemetry_remediation_plan.md` | Documents the alert pack, OTLP batch sizing, and export budget guardrails tied to B2. |
| Multi-lane rehearsal tracker | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lists Apr 9 rehearsal artefacts, validator manifest/digest, Q2 prep notes/agenda, and rollback evidence. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Records slot range 912–936, seed `NEXUS-REH-2026Q2`, and artefact hashes for governance bundles. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash of the approved TLS profile captured during the Q2 rerun; cite in routed-trace appendices. |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Planning notes for the Q2 rehearsal (window, slot range, workload seed, action owners). |
| Launch rehearsal runbook | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Operational checklist for staging → execution → rollback; update when lane topology or exporter guidance changes. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenced by B4 retro; archive digests alongside the tracker whenever the pack changes. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Proves `nexus.enabled = true` for multi-lane configs, preserves the Sora catalog hashes, and provisions lane-local Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` before publishing artefact digests. |
