<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: Cross-SDK Norito Fixture Cadence RFC Outcome
summary: Governance decision and enforcement plan for Swift/Android/Python/JS Norito fixture regeneration.
---

# Cross-SDK Norito Fixture Cadence RFC Outcome

The cross-SDK fixture cadence RFC is now **approved**. This note captures the
agreed schedule, enforcement hooks, and evidence requirements so roadmap/status
updates can point to a single source of truth instead of scattered meeting
notes.

## Decision

- **Cadence:** regenerate Norito fixtures every 48 hours across Swift, Android,
  Python, and JS. If governance cannot meet the 48 hour cadence, fall back to a
  **weekly automatic regeneration**, accompanied by a status digest entry.
- **Scope:** Norito fixture packs, schema hashes, and `/v1/pipeline` parity
  vectors that back CI dashboards and SDK parity gates.
- **Approvals:** SDK council (Swift/Android/Python/JS leads + Torii delegate)
  signed off during the Jan 2026 governance review after reviewing telemetry and
  rollback coverage.

## Rotation & Ownership

The SDK council assigns one rotation owner for the shared fixture set. The
owner runs the sole generator twice, each time with an independent absent
absolute output root:

```bash
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/first-absent-root
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/second-absent-root
```

The path sets, modes, manifests, and bytes must match before either sealed tree
is reviewed as a mechanical patch to the identical tracked paths. After that
reviewed update, record the result of
`cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify --json-out <report-path>`.
Swift, Android, Python, and JavaScript only consume and check their tracked
mirrors; they have no regeneration or archive-extraction entry points.

## Enforcement

- **CI gates:** `ci/check_swift_fixtures.sh` (Swift) and the Android/Python/JS
  equivalents block merges when fixtures drift outside the cadence or when
  schema hashes diverge. Dashboards consume the same feeds so on-call staff see
  the same data the gates used.
- **Telemetry:** cadence state is exported through the parity dashboard feeds
  (`mobile_parity` schema) so alerts can page owners when the oldest diff exceeds
  the SLA or when regen streaks break.
- **Fallback plan:** weekly regen is mandatory if governance decision meetings
  slip; log the fallback in the cadence brief and `status.md`, and reset the SLA
  timers.

## Reporting

- Weekly `status.md` digests must include the latest parity/regen state and any
  SLA breaches.
- Governance reviews cite this note plus the parity dashboard snapshots as the
  canonical evidence bundle.
- Any override or emergency regen requires a short incident note linked from the
  SDK parity sections in `status.md`.
