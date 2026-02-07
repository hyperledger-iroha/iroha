---
lang: kk
direction: ltr
source: docs/source/sdk/swift/fixture_cadence_rfc.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e869d472c89f5b832be1125d0c29263328338de0e118ea1fd7c016d4d18db533
source_last_modified: "2025-12-29T18:16:36.069342+00:00"
translation_last_reviewed: 2026-02-07
---

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

| Stream | Owner | Cadence | Evidence |
|--------|-------|---------|----------|
| Swift | Swift Lead | 48 h regen window; weekly fallback on governance slip | `scripts/swift_fixture_regen.sh` + provenance emitted to `artifacts/swift_fixture_provenance.json`; parity gate `ci/check_swift_fixtures.sh` |
| Android | Android Foundations TL | 48 h regen window; mirrors Swift schedule | Regen + parity diffs recorded in `status.md`; run via workspace automation |
| Python | Python Maintainer | 48 h regen window | Regen script + fixture diff gate under `python/` harness; results echoed to `status.md` |
| JS | JS Lead | 48 h regen window | Regen helper wired into Torii mock harness CI; evidence stored with JS release artefacts |

Rotation owners must coordinate via the SDK council calendar and publish a
diff-friendly summary in `status.md` when a regeneration completes or when a
fallback/override is invoked.

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
