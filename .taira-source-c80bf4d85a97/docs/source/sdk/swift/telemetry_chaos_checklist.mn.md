---
lang: mn
direction: ltr
source: docs/source/sdk/swift/telemetry_chaos_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1770f62812b41a25aae4a20919107685a95249d2c2702ea3ecf8762a69b65393
source_last_modified: "2026-01-05T09:28:12.061754+00:00"
translation_last_reviewed: 2026-02-07
title: Swift Telemetry Chaos Checklist
summary: Scenarios and CLI steps for validating Swift telemetry redaction readiness.
---

# Swift Telemetry Chaos Checklist

This checklist mirrors the Android AND7 chaos programme and exercises the Swift
telemetry redaction tooling (salt rotation, override workflow, dashboard export).
Run the scenarios ahead of governance reviews and whenever telemetry changes land.

## Prerequisites

- Latest Swift workspace with `scripts/swift_collect_redaction_status.py`,
  `scripts/swift_enrich_parity_feed.py`, and the telemetry override helper
  `scripts/swift_telemetry_override.py`).
- Access to the override ledger (`artifacts/swift_telemetry_overrides.json`) or a
  scratch copy for staging drills.
- Salt status JSON (`dashboards/data/swift_salt_status.sample.json` or a live export).
- Buildkite environment variables wired through `ci/swift_status_export.sh`
  (or run the collector/enricher locally for dry-runs).
- Grafana dashboard/alert access to confirm telemetry deltas.

## Scenarios

| Scenario | Goal | Steps | Expected Outcome |
|----------|------|-------|------------------|
| **S1 — Override lifecycle** | Verify overrides appear in the parity telemetry block and clear after revocation. | 1. Create a temporary override:<br/>`python3 scripts/swift_status_export.py telemetry-override create --actor-role support --reason "chaos drill" --expires-in-hours 1 --store artifacts/swift_telemetry_overrides.json`<br/>2. Recompute telemetry JSON:<br/>`python3 scripts/swift_collect_redaction_status.py --salt-config dashboards/data/swift_salt_status.sample.json --overrides-store artifacts/swift_telemetry_overrides.json --output /tmp/telemetry.json`<br/>3. Inject telemetry into the parity feed (or let the CI job run):<br/>`python3 scripts/swift_enrich_parity_feed.py --input dashboards/data/mobile_parity.sample.json --output /tmp/parity.telemetry.json --telemetry-json /tmp/telemetry.json`<br/>4. Render dashboards / run the status exporter to confirm `overrides_open` increments.<br/>5. Revoke the override (`python3 scripts/swift_status_export.py telemetry-override revoke --id <UUID> --store artifacts/swift_telemetry_overrides.json`) and rerun steps 2–4. | `overrides_open` moves from `n→n+1` while the override is active and returns to `n` after revocation. Dashboards/alerts show the blip and clear automatically. |
| **S2 — Salt rotation drift** | Ensure stale salt state surfaces via telemetry + alerts. | 1. Copy the salt sample and backdate `last_rotation` by 72 h.<br/>2. Point the collector at the modified file (`--salt-config /tmp/salt_stale.json`).<br/>3. Run the status exporter or dashboards renderer; `salt_rotation_age_hours` should exceed the 48 h SLA and trigger the redaction alert (or at least highlight the anomaly). | Telemetry block shows `salt_rotation_age_hours` >= 48, dashboards flag the issue, and alerts clear once the salt file returns to the real rotation timestamp. |
| **S3 — Notes / alignment drill** | Validate the note/`device_profile_alignment` fields for status updates. | 1. Call the collector with `--profile-alignment drift --note "simulated drift"`.<br/>2. Inject telemetry and render dashboards to confirm the note block appears and the alignment status changes.<br/>3. Reset alignment to `ok`. | Notes surface verbatim in the parity digest and disappear once alignment is restored to `ok`. |

## Recording Results

- Archive collector outputs (`telemetry.status.json`), enriched parity feeds, and
  dashboard screenshots under `docs/source/sdk/swift/readiness/labs/<date>/`.
- Update `status.md` (Latest Updates) with a one-line summary per drill and link
  to the artifacts.
- Open follow-up issues for any failed expectations (e.g., alerts not firing).

## Related Tooling

- `scripts/swift_collect_redaction_status.py --salt-config … --overrides-store …`
- `scripts/swift_enrich_parity_feed.py --input … --telemetry-json …`
- `ci/swift_status_export.sh` (consumes the telemetry JSON via `SWIFT_TELEMETRY_*` env vars)

Run this checklist quarterly and before major Swift telemetry releases to keep
IOS7 readiness evidence up to date.
