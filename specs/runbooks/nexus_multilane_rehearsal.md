# Nexus Multi-Lane Launch Rehearsal Runbook

The first part of this runbook records the historical Phase B4 Nexus rehearsal.
It validated that the governance-approved `iroha_config` bundle plus the
multi-lane genesis manifest behaved deterministically across telemetry,
routing, and rollback drills.

> **Evidence scope:** The April 9, 2026 material below is preserved as a
> historical B4 configuration/telemetry rehearsal. It is not fresh evidence for
> any current multilane release gate (`G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`,
> `G-SCALE`, `G-SDK`, or `G-FINAL`), and this document does not assert that its
> referenced external artifact directory is present in the source tree.

## Historical B4 scope

- Exercise all three Nexus lanes (`core`, `governance`, `zk`) with mixed Torii
  ingress (transactions, contract deploys, governance actions) using the signed
  workload seed `NEXUS-REH-2026Q1`.
- Capture telemetry/trace artefacts required by B4 acceptance (Prometheus
  scrape, OTLP export, structured logs, Norito admission traces, RBC metrics).
- Execute rollback drill `B4-RB-2026Q1` immediately after the dry-run and
  confirm the single-lane profile re-applies cleanly.

## Historical B4 preconditions

1. `specs/project_tracker/nexus_config_deltas/2026Q1.md` reflects the
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

## Historical B4 timeline

| Phase | Target window | Owner(s) | Exit criteria |
|-------|---------------|----------|---------------|
| Preparation | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed published, dashboards staged, rehearsal nodes provisioned. |
| Staging freeze | Apr 8 2026 18:00 UTC | @release-eng | Config/genesis hashes re-verified; change freeze notice sent. |
| Execution | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Checklist completed without blocking incidents; telemetry pack archived. |
| Rollback drill | Immediately post-execution | @sre-core | `B4-RB-2026Q1` checklist completed; rollback telemetry captured. |
| Retrospective | Due Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | Retro/lessons learned doc + blocker tracker published. |

## Historical execution checklist (Apr 9 2026 15:00 UTC)

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
   Nexus evidence bucket; link them in `specs/nexus_transition_notes.md`.
8. **Manifest/validation** — run `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` to produce `telemetry_manifest.json`
   + `.sha256`, then attach the manifest to the tracker entry for the rehearsal.
   The helper normalises slot boundaries (recorded as integers in the manifest)
   and fails fast when either hint is missing so the governance artefacts remain
   deterministic.

## Historical outputs

- Signed rehearsal checklist + incident drill log.
- Telemetry pack (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Telemetry manifest + digest generated by the validation script.
- Retrospective doc summarising blockers, mitigations, and owner assignments.

## Historical execution summary — Apr 9 2026

- Rehearsal executed 15:00 UTC–16:12 UTC with seed `NEXUS-REH-2026Q1`; all
  three lanes sustained ~2.4k TEU per slot and `nexus_lane_state_total`
  reported balanced envelopes.
- Telemetry pack archived at `artifacts/nexus/rehearsals/2026q1/` (includes
  `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, incident log,
  and rollback evidence). Checksums recorded in
  `specs/project_tracker/nexus_rehearsal_2026q1.md`.
- Rollback drill `B4-RB-2026Q1` completed at 16:18 UTC; single-lane profile
  re-applied in 6m42s with no stalled lanes, then Nexus bundle re-enabled after
  telemetry confirmation.
- Lane saturation incident injected at slot 842 (forced headroom clamp) fired
  the expected alerts; mitigation playbook closed the page in 11m with
  documented PagerDuty timeline.
- No blockers prevented completion; follow-up items (TEU headroom logging
  automation, telemetry pack validator script) are tracked in the Apr 15
  retrospective.

## Production multilane release rehearsal

This supplement is the current evidence-collection contract. Implementation
availability is not a pass: every gate below remains **Open** until a fresh,
unskipped run from one clean source revision archives its logs, hashes,
configuration, deterministic seeds, and—where applicable—hardware identity.

### Evidence sources

Capture these surfaces from every peer at every phase boundary and before and
after each restart:

- `GET /v1/nexus/lifecycle` for the active catalog, lane/dataspace/incarnation
  geometry, activation and close heights, and autoscale transitions;
- `GET /v1/sumeragi/status` for only the authoritative
  `SumeragiV2Status` reducer snapshot and committed frontier; and
- `GET /v1/sumeragi/diagnostics` for non-authoritative, State/Kura-derived lane
  evidence, including `native_amx_participant_applications` and
  `autonomous_lane_executions`.

Do not pass a status document to a lifecycle parser or use diagnostics as
consensus authority. For Native rows, retain all four states
(`certified_pending_carrier`, `committed_evidence_pending`,
`durably_applied`, and `conflict`) and fail the rehearsal on an unexplained
conflict. For autonomous rows, correlate the typed route/incarnation and
reservation/source identities across reservation, payload, availability, lane
QC, certified bundle, merge candidate, carrier, Kura/WSV application, and
`queue_finalized`. A stalled stage is a failure to diagnose, not permission to
skip the source.

### Fresh four-peer suite (`G-4P`)

Run every required DA/RBC case with skips treated as failures:

1. Automatically expand, activate, and execute useful reservation-bound work
   on multiple lanes concurrently.
2. Restart at every reservation, availability, lane-QC, certified-bundle,
   merge, Kura/WSV, Native sidecar/index, queue Commit, and ForgetCommit crash
   boundary; prove no source is lost or executed twice.
3. Exercise grouped and mixed-role Native AMX, including same-route
   coordinator handling, wrong predecessor, stale incarnation, body pruning,
   missing-sidecar recovery from authenticated holders, and forged evidence.
4. Rotate one validator offline and one Byzantine input source while committees
   and global leaders change.
5. Drain only after ordinary queue work, reservations, certified-unmerged
   bundles, delayed work, pending merge entries, and unverifiable Native
   controls clear. Carry the exact drain certificate, retire in a later block,
   and validate the archive.
6. Recreate the same lane ID through an A/B/A sequence and reject every delayed
   QC, reservation, marker, receipt, manifest/index, signing claim, and merge
   artifact from the retired incarnations.
7. Require peer convergence on canonical Kura/WSV state, transaction queries,
   Native receipts, queue ownership, and all endpoint projections.

### Fresh 13-peer corridor and soak (`G-12P`)

`G-12P` names the twelve lane-validator assignments retained by the release
receipt schema; every fresh network has a valid 13-member revision-4 global
committee plus one global-only lane observer. Provision at least three
independent four-validator dataspaces. Combine grouped
DvP and autonomous work with rotating outage/restart, scale-out, drain,
scale-in, pruning, and same-ID recreation. Run **10/10 fresh deterministic
seeds**, archiving the accepted transaction set and final identity of every
source. Then run the separate **two-hour fault soak**. Every peer must converge;
Native participant receipts must be durable; lost, duplicate, or
rejected-after-acceptance transactions must remain zero. Use the bounded
autonomous stage records to prove the former
payload-recovery-to-canonical-application stall does not recur.

### Scaling and final validation (`G-SCALE`, `G-SDK`, `G-FINAL`)

- On pinned hardware, run five paired one-lane versus four-active-lane trials at
  matched offered load. Four lanes must reach at least 1.5× median committed
  throughput, with p95 latency no worse than 1.25× and every configured queue,
  index, memory, and disk bound respected.
- Run the Rust/OpenAPI, Python, JavaScript, Swift, Kotlin, and Java parity
  suites against the same Rust-owned grouped fixture and negative corpus.
- Before every Cargo command, inspect `ps -axo pid,etime,command` and wait while
  another Cargo/rustc process is active. Use the isolated target directory with
  `--locked --offline`, then run focused crates, SDK parity, formal runners,
  workspace build, full workspace test, strict workspace Clippy, formatting,
  and the legacy-codec guard.

The gate registry is intentionally unchanged by this runbook: `G-UNIT`,
`G-FORMAL`, `G-4P`, `G-12P`, `G-SCALE`, `G-SDK`, and `G-FINAL` all remain
**Open** until their fresh artifacts pass and are archived.

## Escalation

- Blocking incidents or telemetry regressions halt the rehearsal and require a
  governance escalation within 4 business hours.
- Any deviation from the approved config/genesis bundle must restart the
  rehearsal after re-approval.

## Historical telemetry pack validation

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
