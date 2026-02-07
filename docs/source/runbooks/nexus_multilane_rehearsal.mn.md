---
lang: mn
direction: ltr
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c609068317318d1c6d7b1009a545babc1dcac85ac83a63b049b9cde0a664f230
source_last_modified: "2025-12-29T18:16:36.028838+00:00"
translation_last_reviewed: 2026-02-07
---

# Nexus Multi-Lane Launch Rehearsal Runbook

This runbook guides the Phase B4 Nexus rehearsal. It validates that the
governance-approved `iroha_config` bundle plus the multi-lane genesis manifest
behave deterministically across telemetry, routing, and rollback drills.

## Scope

- Exercise all three Nexus lanes (`core`, `governance`, `zk`) with mixed Torii
  ingress (transactions, contract deploys, governance actions) using the signed
  workload seed `NEXUS-REH-2026Q1`.
- Capture telemetry/trace artefacts required by B4 acceptance (Prometheus
  scrape, OTLP export, structured logs, Norito admission traces, RBC metrics).
- Execute rollback drill `B4-RB-2026Q1` immediately after the dry-run and
  confirm the single-lane profile re-applies cleanly.

## Preconditions

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reflects the
   GOV-2026-03-19 approval (signed manifests + reviewer initials).
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, with
   `nexus.enabled = true` baked in) and `defaults/nexus/genesis.json` match the
   approved hashes; `kagami genesis bootstrap --profile nexus` reports the same
   digest recorded in the tracker.
3. The lane catalog matches the approved three-lane layout; `irohad --sora
   --config defaults/nexus/config.toml` should emit the Nexus router banner.
4. Multi-lane CI is green: `ci/check_nexus_multilane_pipeline.sh` (runs
   `integration_tests/tests/nexus/multilane_pipeline.rs` via
   `.github/workflows/integration_tests_multilane.yml`) and
   `ci/check_nexus_multilane.sh` (router coverage) both pass so the Nexus
   profile stays multi-lane-ready (`nexus.enabled = true`, Sora catalog hashes
   intact, lane storage under `blocks/lane_{id:03}_{slug}` and merge logs
   provisioned). Capture the artefact digests in the tracker when the defaults
   bundle changes.
5. Telemetry dashboards + alerts for Nexus metrics are imported into the
   rehearsal Grafana folder; alert routes point to the rehearsal PagerDuty
   service.
6. Torii SDK lanes are configured per the routing policy table and can replay
   the rehearsal workload locally.

## Timeline Overview

| Phase | Target window | Owner(s) | Exit criteria |
|-------|---------------|----------|---------------|
| Preparation | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed published, dashboards staged, rehearsal nodes provisioned. |
| Staging freeze | Apr 8 2026 18:00 UTC | @release-eng | Config/genesis hashes re-verified; change freeze notice sent. |
| Execution | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Checklist completed without blocking incidents; telemetry pack archived. |
| Rollback drill | Immediately post-execution | @sre-core | `B4-RB-2026Q1` checklist completed; rollback telemetry captured. |
| Retrospective | Due Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | Retro/lessons learned doc + blocker tracker published. |

## Execution Checklist (Apr 9 2026 15:00 UTC)

1. **Config attestation** — `iroha_cli config show --actual` on every node;
   confirm hashes match tracker entry.
2. **Lane warm-up** — replay seed workload for 2 slots, verify `nexus_lane_state_total`
   shows activity in all three lanes.
3. **Telemetry capture** — record Prometheus `/metrics` snapshots, OTLP packet
   samples, Torii structured logs (per lane/dataspace), and RBC metrics.
4. **Governance hooks** — execute governance transaction subset and verify lane
   routing + telemetry tags.
5. **Incident drill** — simulate lane saturation per plan; ensure alerts fire
   and the response is logged.
6. **Rollback drill `B4-RB-2026Q1`** — apply single-lane profile, replay
   rollback checklist, collect telemetry evidence, and re-apply Nexus bundle.
7. **Artefact upload** — push telemetry pack, Torii traces, and drill log to the
   Nexus evidence bucket; link them in `docs/source/nexus_transition_notes.md`.
8. **Manifest/validation** — run `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` to produce `telemetry_manifest.json`
   + `.sha256`, then attach the manifest to the tracker entry for the rehearsal.
   The helper normalises slot boundaries (recorded as integers in the manifest)
   and fails fast when either hint is missing so the governance artefacts remain
   deterministic.

## Outputs

- Signed rehearsal checklist + incident drill log.
- Telemetry pack (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Telemetry manifest + digest generated by the validation script.
- Retrospective doc summarising blockers, mitigations, and owner assignments.

## Execution Summary — Apr 9 2026

- Rehearsal executed 15:00 UTC–16:12 UTC with seed `NEXUS-REH-2026Q1`; all
  three lanes sustained ~2.4k TEU per slot and `nexus_lane_state_total`
  reported balanced envelopes.
- Telemetry pack archived at `artifacts/nexus/rehearsals/2026q1/` (includes
  `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, incident log,
  and rollback evidence). Checksums recorded in
  `docs/source/project_tracker/nexus_rehearsal_2026q1.md`.
- Rollback drill `B4-RB-2026Q1` completed at 16:18 UTC; single-lane profile
  re-applied in 6m42s with no stalled lanes, then Nexus bundle re-enabled after
  telemetry confirmation.
- Lane saturation incident injected at slot 842 (forced headroom clamp) fired
  the expected alerts; mitigation playbook closed the page in 11m with
  documented PagerDuty timeline.
- No blockers prevented completion; follow-up items (TEU headroom logging
  automation, telemetry pack validator script) are tracked in the Apr 15
  retrospective.

## Escalation

- Blocking incidents or telemetry regressions halt the rehearsal and require a
  governance escalation within 4 business hours.
- Any deviation from the approved config/genesis bundle must restart the
  rehearsal after re-approval.

## Telemetry Pack Validation (Completed)

Run `scripts/telemetry/validate_nexus_telemetry_pack.py` after every rehearsal
to prove the telemetry bundle contains the canonical artefacts (Prometheus
export, OTLP NDJSON, Torii structured logs, rollback log) and capture their
SHA-256 digests. The helper writes both `telemetry_manifest.json` and the
matching `.sha256` file so governance can cite the evidence hashes directly in
the retro packet.

For the Apr 9 2026 rehearsal, the validated manifest lives alongside the
artefacts under `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`
with its digest in `telemetry_manifest.json.sha256`. Attach both files to the
tracker entry when publishing the retro.

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

Pass `--require-slot-range` / `--require-workload-seed` inside CI to block
uploads that forget those annotations. Use `--expected <name>` to add extra
artefacts (e.g., DA receipts) if the rehearsal plan calls for them.
