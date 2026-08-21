<!--
Swift/Android/Python shared Norito fixture cadence pre-read.
Add translations once localisation staffing is available.
-->

# Norito Fixture Cadence Governance Pre-Read (Swift / Android / Python)

> Last updated: 2026-01-12  
> Authors: Swift SDK Lead, Android Foundations TL, Norito Tooling Maintainer  
> Status: Superseded execution model; retained as the Jan 2026 governance record

> **Superseded on 2026-08-21:** the cadence and SLA decisions remain historical
> context, but the SDK-local generation and archive hand-off described by the
> original pre-read are retired. The sole fixture owner is the canonical command
> below, repeated with two distinct output roots.

```bash
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root <absent-absolute-external-root>
```

Require identical sorted paths, file types, modes, completion manifests, and
bytes, apply the reviewed identity-relative patch from either sealed
publication, then run `cargo run --locked -p xtask --features dev-tools --bin
xtask -- norito-rpc-verify`. Swift, Android, and Python consume and check the
resulting mirrors; none is a fixture owner.

## Decision Summary

- **Approve a shared publication-review cadence for Norito fixtures** that
  applies to Swift (`IOS2`), Android (`AND1/AND3`), and Python (`PY3`) SDKs.
- **Use the canonical Rust owner command** to render the complete corpus and all
  managed mirrors together; `fixtures/norito_rpc/` is the sole source of truth.
- **Commit to a 48-hour SLA for propagating approved discriminator/ABI changes**
  into all SDK fixture sets, with dashboards enforcing alerts when breached.
- **Delegate scheduling and reporting** to the Swift Lead (rotation with Android
  Foundations TL), while keeping generation ownership in the canonical Rust
  command and treating every SDK as a consumer/checker.

> **Update (2026-01-15):** Governance council approved the cadence as proposed. The
> alternating rotation (odd weeks Android Foundations TL, even weeks Swift Lead)
> starts with the 2026-01-22 17:00 UTC slot, and the decision record is logged in the
> 2026-01-15 council minutes alongside the follow-up checklist.

## Background

- Norito fixtures (signed transactions, manifest payloads)
  ensure SDK encoders stay byte-for-byte aligned with the Rust reference
  implementation.
- The canonical owner publishes the canonical corpus plus the Java, Swift, and
  Python managed mirrors as one complete publication. SDK-local directories are
  outputs only.
- Governance requested a formal cadence after repeated ad-hoc updates delayed
  parity metrics (`dashboards/mobile_parity.swift`) and `/v1/pipeline`
  migrations tracked under IOS2/AND4.
- Cross-SDK automation now exists:
  - The create-only `xtask norito-rpc-fixtures --output-root` owner and
    read-only `xtask norito-rpc-verify` verifier.
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
   - Wednesdays 17:00 UTC (aligns with the Android regression window) the
     scheduled operator performs the two-root canonical publication procedure.
   - Rotation: Android Foundations (odd weeks), Swift (even weeks) coordinates
     the run and review; all SDKs validate their generated mirrors post-update.
2. **Event-driven regen within 48 hours**
   - Any governance-approved Norito discriminator/ABI change triggers an
     immediate regen regardless of schedule.
   - Owner posts intent in `#sdk-parity` Slack, links to Rust commit, and opens
     tracking ticket if the 48 h SLA is at risk.
3. **Single source of truth**
   - `fixtures/norito_rpc/` is the only fixture input. The owner renders its
     canonical outputs and every managed SDK mirror into each sealed root.
   - SDK-local generators, alternate-source modes, and fixture archives are not
     supported generation inputs.
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
| Jan Week 3 | Circulate cadence decision + rotation calendar; update runbooks (`specs/swift_parity_triage.md`, Android equivalent). | Swift Lead / Android Foundations TL |
| Jan Week 3 | Historical SDK-local archive-ingest work; superseded by the create-only two-root canonical owner and no longer a supported hand-off path. | Norito tooling maintainer |
| Jan Week 4 | Update CI jobs (`ci/check_swift_fixtures.sh`, Android parity jobs) to reference the new cadence and include rotation owner in alerts. | Swift/Android CI owners |
| Feb Week 1 | Validate Swift/Android/Python mirrors emitted by the canonical owner publication. | Norito tooling maintainer |
| Feb Week 1 | Update `status.md` and dashboards to include cadence owner + next scheduled regen. | Swift Program PM |

## Dependencies & Roles

| Role | Responsibility |
|------|----------------|
| Swift Lead | Alternate weekly coordinator, check the Swift mirror, update `status.md`. |
| Android Foundations TL | Alternate weekly coordinator, check the Java mirror, share parity evidence. |
| Norito Tooling Maintainer | Maintain the canonical owner/verifier and document schema changes. |
| Python Maintainer | Validate fixture parity in CLI tests, flag regressions. |
| Governance Council | Approve cadence, review breaches quarterly. |

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Canonical owner or verification fails. | No SDK mirror can be published. | Preserve both failed external roots as diagnostics, fix the canonical source or owner, and rerun two fresh roots; never fall back to an SDK-local source. |
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
   coordinators, with Norito tooling maintaining the sole owner/verifier and
   each SDK remaining a read-only consumer/checker.

Approval unblocks IOS2/AND3 parity gating and allows the Swift dashboard alerts
to transition from draft to enforcing mode.
