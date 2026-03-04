# Nexus Multi-Lane Rehearsal — 2026 Q1

Phase B4 required a multi-lane launch rehearsal before promoting Nexus to the
launch checklist. The rehearsal ran on Apr 9 2026 at 15:00 UTC using the
`NEXUS-REH-2026Q1` workload seed and the governance-approved `iroha_config`
bundle (vote GOV-2026-03-19).

## Summary

- Duration: 72 minutes end-to-end, including rollback drill `B4-RB-2026Q1`.
- Lanes: `core`, `governance`, `zk` sustained ~2.4k TEU/slot with balanced
  envelopes and no starvation breaches.
- Telemetry coverage: Prometheus, OTLP, Torii structured logs, DA/RBC metrics,
  and Norito admission traces captured for every phase.
- Rollback drill: single-lane profile re-applied in 6m42s, Nexus bundle reapplied
  after telemetry confirmation without drift.

## Checklist Results

| Checklist item | Owner(s) | Result | Notes |
|----------------|----------|--------|-------|
| Config attestation | @release-eng | ✅ Pass | Hashes matched tracker entry prior to warm-up. |
| Lane warm-up | @nexus-core | ✅ Pass | `nexus_lane_state_total` showed activity across all lanes within 2 slots. |
| Telemetry capture | @telemetry-ops | ✅ Pass | Samples recorded at T+5 min and T+45 min; no gaps detected. |
| Governance hooks | @torii-sdk | ✅ Pass | Governance transactions routed to lane 1, telemetry tags matched expectations. |
| Incident drill | @program-mgmt, @qa-veracity | ⚠️ Pass w/ alert | Forced TEU headroom clamp triggered alerts; resolved within 11 min (see incidents). |
| Rollback drill `B4-RB-2026Q1` | @sre-core | ✅ Pass | Single-lane fallback reapplied, logs archived, Nexus bundle restored. |
| Artefact upload | @telemetry-ops | ✅ Pass | Evidence uploaded to `artifacts/nexus/rehearsals/2026q1/`. |

## Telemetry Artefacts

| Artifact | Path | SHA-256 | Notes |
|----------|------|---------|-------|
| Telemetry pack | `artifacts/nexus/rehearsals/2026q1/prometheus.tgz` | `33bce52dd641997e046661273639bc1b8371b43ef44760063142bb12672ab631` | Includes `/metrics` snapshots + dashboards JSON. |
| OTLP export | `artifacts/nexus/rehearsals/2026q1/otlp.ndjson` | `22d317fb35d55c8634996cee90defa0041890db367440ceb1c65b4f2264240dc` | Contains span + metric exports referenced by ops. |
| Torii structured logs | `artifacts/nexus/rehearsals/2026q1/torii_structured_logs.jsonl` | `55ce9fcdd2eccea264ccdb87215c99d523c3d7450942215f93301a1f7ce0f08a` | Annotated with lane/dataspace, used for routed-trace verification. |
| Rollback drill log | `artifacts/nexus/rehearsals/2026q1/B4-RB-2026Q1.log` | `b7e9a25975b391a32e8d321cfa9ef95c2cd426d04a4bd76158cd5ee1436c96e0` | Captures single-lane fallback timeline + approvals. |
| Telemetry manifest | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` | `221befa6cc5936da41ffac8e4755193398c36c6bd2b77aebc15d9e1395b77220` | Generated via `validate_nexus_telemetry_pack.py` (slot range `912-936`, workload seed `NEXUS-REH-2026Q2`); companion digest in `.sha256`. |

## Incidents

| ID | Description | Impact | Resolution |
|----|-------------|--------|------------|
| B4-INC-01 | Intentional TEU headroom clamp triggered `nexus_scheduler_lane_teu_deferral_total` alert in lane `governance`. | Alert noise only; no missed commits. | Reduced injected load after verifying alert, documented PagerDuty incident timeline (11 minutes). |
| B4-INC-02 | OTLP exporter throttled at T+55 min causing 30 s backlog. | None (buffer drained). | Increased OTLP batch size to 256, noted in follow-ups. |

## Blockers & Follow-Ups

| Item | Owner | Due | Status | Notes |
|------|-------|-----|--------|-------|
| Automate telemetry pack validation script | @telemetry-ops | 2026-04-30 | Completed | `scripts/telemetry/validate_nexus_telemetry_pack.py` emits `telemetry_manifest.json` + `.sha256`; runbook + tracker updated. |
| Increase TEU headroom observability (alert tuning) | @nexus-core | 2026-05-07 | Completed | `nexus.scheduler.headroom` telemetry log + `nexus_scheduler_lane_headroom_events_total` metric document per `docs/source/telemetry.md`. |
| Document OTLP batch sizing knob in ops guide | @program-mgmt | 2026-04-22 | Completed | Added OTLP capture tuning guidance + collector snippet to `docs/source/telemetry.md`. |

## Q2 Canary Prep Notes

- Planning agenda captured in `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` (window 2026-05-08 14:00–15:00 UTC, slot range `910-950`, workload seed `TRACE-MULTILANE-CANARY-2026Q2`).
- TLS bundle fingerprint recorded at `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` for pre-flight validation.
- Telemetry pack validator run for the latest rehearsal is stored with the Q1 artefacts above to keep a single evidence root; reuse the validator with the Q2 slot range/seed before promotion.

Retrospective outcomes and blocker ownership are mirrored in the Apr 15 2026
update (`docs/source/nexus_transition_notes.md`, `roadmap.md`, `status.md`).
