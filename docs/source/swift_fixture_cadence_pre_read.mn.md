---
lang: mn
direction: ltr
source: docs/source/swift_fixture_cadence_pre_read.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19563f1fd4202e160de50412d9b5c60d916cc2f74de277a71404dfeba06a9f8c
source_last_modified: "2026-01-30T18:06:03.626415+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
Swift/Android/Python shared Norito fixture cadence pre-read.
Add translations once localisation staffing is available.
-->

# Norito Fixture Cadence Governance Pre-Read (Swift / Android / Python)

> Last updated: 2026-01-12  
> Authors: Swift SDK Lead, Android Foundations TL, Norito Tooling Maintainer  
> Status: Ready for Jan 2026 governance vote

## Decision Summary

- **Approve a shared regeneration cadence for Norito fixtures** that applies to
  Swift (`IOS2`), Android (`AND1/AND3`), and Python (`PY3`) SDKs.
- **Adopt the Rust exporter (`scripts/export_norito_fixtures`) as the canonical
  source** once Swift wiring lands; until then Android remains the temporary
  source of truth.
- **Commit to a 48-hour SLA for propagating approved discriminator/ABI changes**
  into all SDK fixture sets, with dashboards enforcing alerts when breached.
- **Delegate ongoing ownership** to the Swift Lead (rotation with Android
  Foundations TL) for scheduling and reporting, with governance review every
  quarter.

> **Update (2026-01-15):** Governance council approved the cadence as proposed. The
> alternating rotation (odd weeks Android Foundations TL, even weeks Swift Lead)
> starts with the 2026-01-22 17:00 UTC slot, and the decision record is logged in the
> 2026-01-15 council minutes alongside the follow-up checklist.

## Background

- Norito fixtures (signed transactions, manifest payloads)
  ensure SDK encoders stay byte-for-byte aligned with the Rust reference
  implementation.
- Android currently publishes the canonical fixtures; Swift mirrors them via
  `scripts/swift_fixture_regen.sh`, while Python consumes the same set for CLI
  smoke tests.
- Governance requested a formal cadence after repeated ad-hoc updates delayed
  parity metrics (`dashboards/mobile_parity.swift`) and `/v1/pipeline`
  migrations tracked under IOS2/AND4.
- Cross-SDK automation now exists:
  - `scripts/swift_fixture_regen.sh` (Swift) + comparable Android tooling.
  - `scripts/check_swift_fixtures.py` and `ci/check_swift_fixtures.sh`
    gatekeeper checks.
  - `make swift-dashboards` enforces age/SLA thresholds (14 day outstanding
    cap, 48 hour regen breach alerts).

## Current Pain Points

- **Unpredictable regen timing**: Rust contract changes land without a clear
  schedule, forcing Swift/Android to scramble and occasionally breach the 48 h
  SLA.
- **Fragmented ownership**: Each SDK triages diffs independently; governance has
  no single escalation path when cadence slips.
- **Manual reporting**: `status.md` and dashboards require manual updates,
  slowing cross-SDK awareness.

## Proposed Cadence & Process

1. **Weekly scheduled regeneration**
   - Wednesdays 17:00 UTC (aligns with Android regression window) the owning
     SDK runs the exporter and updates fixtures.
   - Rotation: Android (odd weeks), Swift (even weeks) until shared Rust
     exporter publishes Swift bindings; Python validates parity post-update.
2. **Event-driven regen within 48 hours**
   - Any governance-approved Norito discriminator/ABI change triggers an
     immediate regen regardless of schedule.
   - Owner posts intent in `#sdk-parity` Slack, links to Rust commit, and opens
     tracking ticket if the 48 h SLA is at risk.
3. **Source of truth transition**
   - Until Swift wiring lands, continue mirroring from Android fixtures.
   - Once Rust exporter emits Swift artifacts, all SDKs consume the same archive
     produced by CI (signed artifact uploaded to shared bucket).
4. **Monitoring & reporting**
   - `make swift-dashboards` (and Android/Python equivalents) remain the gating
     check; breaches raise PagerDuty alerts owned by the Swift Lead rotation.
   - `status.md` Swift section records regen outcomes weekly; governance minutes
     capture any SLA breaches and remediation.
   - CI reports now include per-lane `device_tag` metadata (e.g., `iphone-sim`,
     `strongbox`) so dashboards and on-call rotations can immediately identify which
     hardware path failed; keep the tags accurate when updating Buildkite jobs.
   - Before handing off a regen, run `make swift-ci` to validate fixture parity and
     dashboard feeds locally; CI relies on the same target and Buildkite metadata to keep
     the council’s SLA checks deterministic.

## Implementation Plan (If Approved)

| Week | Action | Owner(s) |
|------|--------|----------|
| Jan Week 3 | Circulate cadence decision + rotation calendar; update runbooks (`docs/source/swift_parity_triage.md`, Android equivalent). | Swift Lead / Android Foundations TL |
| Jan Week 3 | Add automation toggle so `scripts/swift_fixture_regen.sh` accepts Rust exporter archives (prep for hand-off) — delivered via the `SWIFT_FIXTURE_ARCHIVE` switch that extracts `.tar.*`/`.zip` bundles and records archive digests in cadence state. | Norito tooling maintainer |
| Jan Week 4 | Update CI jobs (`ci/check_swift_fixtures.sh`, Android parity jobs) to reference the new cadence and include rotation owner in alerts. | Swift/Android CI owners |
| Feb Week 1 | Publish signed Rust exporter artifact and validate Swift/Android/Python consumption paths. | Norito tooling maintainer |
| Feb Week 1 | Update `status.md` and dashboards to include cadence owner + next scheduled regen. | Swift Program PM |

## Dependencies & Roles

| Role | Responsibility |
|------|----------------|
| Swift Lead | Alternate weekly owner, maintain Swift fixtures, update `status.md`. |
| Android Foundations TL | Alternate weekly owner, maintain Android fixtures, share regen outputs. |
| Norito Tooling Maintainer | Maintain exporter, publish signed archives, document schema changes. |
| Python Maintainer | Validate fixture parity in CLI tests, flag regressions. |
| Governance Council | Approve cadence, review breaches quarterly. |

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Rust exporter delays block Swift from consuming canonical archives. | Swift remains dependent on Android mirror beyond Q1. | Keep rsync mirror as fallback; track exporter readiness in `status.md`; escalate to Rust WP2 owners if past Feb target. |
| Owner rotation misses scheduled regen. | Dashboard alerts fire; parity drifts. | PagerDuty rotation tied to weekly owner; backup owner on-call; governance review if two misses in a quarter. |
| Governance changes land without notice. | SLA breach; inconsistent fixtures. | Require governance approvals to include fixture impact section; tooling maintainer posts heads-up in `#sdk-governance`. |

## Vote Requested

Governance council is asked to approve the following motion during the Jan 2026
session:

1. Adopt the cadence and rotation described above for Swift/Android/Python
   Norito fixtures.
2. Ratify the 48-hour SLA for event-driven updates and the use of dashboard/CI
   alerts as the enforcement mechanism.
3. Confirm the Swift Lead + Android Foundations TL rotation as accountable
   owners, with Norito tooling maintaining the exporter deliverable.

Approval unblocks IOS2/AND3 parity gating and allows the Swift dashboard alerts
to transition from draft to enforcing mode.
