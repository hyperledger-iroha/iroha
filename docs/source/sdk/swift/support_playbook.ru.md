---
lang: ru
direction: ltr
source: docs/source/sdk/swift/support_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f37497a18e63ee939dd124f6a807d32ee55288b5676b3ccb7e1212726ee5df8e
source_last_modified: "2026-01-04T10:50:53.649928+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: Swift SDK Support Playbook
summary: Support scope, escalation paths, and release evidence required for IOS8 partner pilots, GA, and LTS maintenance.
---

# Swift SDK Support Playbook

This outline satisfies the IOS8 roadmap requirement to document how the Swift SDK
is supported from the Q3 2026 partner pilot through GA and the long-term support
(LTS) cadence. It references the reproducibility checklist
(`docs/source/sdk/swift/reproducibility_checklist.md`), telemetry readiness plan,
and automation described in `roadmap.md` so program leads, SRE, and partner
engineers share a single source of truth.

## 1. Purpose & Scope

- Capture the ownership model, SLAs, and escalation steps for Swift SDK incidents.
- Tie release automation (`make swift-ci`, `scripts/swift_status_export.py`,
  `.buildkite/swift-status-export.yml`) to partner-facing commitments.
- Enumerate the artefacts and runbooks that must accompany every pilot, GA, hotfix,
  and LTS release so Release/Compliance and Docs stay aligned.

## 2. Ownership & Roles

| Role | Responsibilities | Notes |
|------|------------------|-------|
| Swift Program Lead | Approves roadmap gates, owns this playbook, and records decisions in `status.md`. | Confirms partner pilot readiness and GA sign-off. |
| Support Engineering Lead | Runs the Swift support rota, triages Sev 1/2 incidents, maintains partner distribution lists. | Publishes weekly digest via `make swift-status`. |
| Release Engineering | Operates XCFramework builds, provenance, and artifact promotion. | Executes the reproducibility checklist and archives evidence. |
| Telemetry & Observability | Maintains `dashboards/mobile_parity.swift`, `dashboards/mobile_ci.swift`, alert rules, and `scripts/swift_collect_redaction_status.py`. | Owns telemetry redaction diff reviews and chaos drills. |
| Docs & Support Enablement | Keeps README/status entries current, localises notices, curates partner updates. | Syncs translations (`index.ja.md`, `index.he.md`) after each release. |
| Compliance & Legal | Tracks regulator artefacts, retention requirements, and data-sharing approvals. | Ensures telemetry overrides follow `docs/source/sdk/swift/telemetry_redaction.md`. |

### 2.1 Communications & On-call

1. Incidents page `swift-sdk-primary` (PagerDuty) with `swift-sdk-secondary`
   cc’d for mirroring.
2. Announce status in `#sdk-parity` (internal) and partner-specific channels
   within **15 minutes** for Sev 1, **1 hour** for Sev 2.
3. Bridge invites include Release, Telemetry, and Compliance by default for
   Sev 1. Sev 2 invites Release + Telemetry as needed.
4. After-action review (AAR) template: reuse
   `docs/source/status/swift_weekly_digest.md` sections and file within
   **5 business days**.

## 3. Release Phases & Support Windows

| Phase | Window | Support Scope | Exit Criteria |
|-------|--------|---------------|---------------|
| Pilot (IOS8 P0) | Q3 2026 targeted partners | 9×5 support, manual telemetry reviews, direct Slack bridges. | Pilot validation report and parity dashboard snapshot uploaded to `status.md`. |
| General Availability | Target Q4 2026 | 24×5 SLA, PagerDuty rotation live, automated dashboards + exporter in CI. | Signed GA packet (reproducibility checklist + parity digest + release notes) stored in `artifacts/swift_release/<tag>/`. |
| Hotfix | Any time | Sev 1 regression/security fix, dedicated bridge with Release + Compliance. | Hotfix advisory in README/status, provenance reissued, telemetry report filed. |

Release calendars must be published at least **60 days** ahead of GA/LTS windows
and shared with partner success leads together with blackout constraints.

## 4. SLA & Severity Matrix

| Severity | Examples | Detection Sources | Acknowledge | Mitigation & Updates |
|----------|----------|-------------------|-------------|----------------------|
| Sev 1 | Signing failures, reproducibility gap, telemetry redaction regression, P0 crash in pilot | `ci/xcode-swift-parity`, partner reports, `dashboards/mobile_ci.swift` alerts | ≤15 min | Status updates every 30 min, Release + Compliance on bridge, incident logged in `status.md`. |
| Sev 2 | Telemetry drift, doc/SLA mistakes, degraded Connect retries | Dashboards, `scripts/swift_status_export.py`, support tickets | ≤1 hour | Hourly updates, optional Release involvement, summary posted within 24 h. |
| Sev 3 | Cosmetic issues, documentation clarifications | Support queue | ≤1 business day | Weekly digest entry; track in Jira/notion. |

Response timer starts when Alerts fire or the partner email lands. Every
incident must include links to parity dashboards, fixture age from
`ci/check_swift_fixtures.sh`, and the reproducibility checklist row.

## 5. Release Gating & Evidence

Before sharing artefacts outside engineering:

1. **Parity + Fixtures:** Run `make swift-fixtures` and
   `make swift-fixtures-check`; archive `artifacts/swift_fixture_regen_state.json`.
2. **CI & Dashboards:** Execute `make swift-ci` (wraps fixtures + dashboards)
   and save the resulting parity/telemetry snapshots.
3. **Reproducibility:** Follow
   `docs/source/sdk/swift/reproducibility_checklist.md`, capturing
   `swift package compute-checksum` output for every XCFramework.
4. **Status Export:** Run `make swift-status` (which invokes
   `scripts/swift_status_export.py`) to produce Markdown/JSON digests, then file
   them under `docs/source/status/`.
5. **Provenance & Publishing:** Document signing and SBOM steps in the release
   packet; reference Buildkite job IDs and the Git commit.
6. **Partner Packet:** Bundle release notes, SLA reminders, telemetry summary,
   and localized notices before distribution.

CI should block merges when `ci/swift_status_export.sh`, `ci/xcode-swift-parity`,
or `ci/check_swift_fixtures.sh` fail; rerun only after the root cause is fixed.

## 6. Telemetry, Redaction, and Chaos

- Redaction policy lives in `docs/source/sdk/swift/telemetry_redaction.md`;
  overrides must be recorded via
  helper) and logged in the parity digest.
- Quarterly chaos drills follow
  `docs/source/sdk/swift/telemetry_chaos_checklist.md` (salt rotation,
  collector stall, override replay). Store artefacts in
  `dashboards/data/swift_*` directories and reference them in `status.md`.
- Dashboards: `dashboards/mobile_parity.swift` for fixture/parity health and
  `dashboards/mobile_ci.swift` for CI/telemetry lane uptime. Keep alert routing
  up to date with SRE.

## 7. Incident & Partner Communication Flow

1. Open PagerDuty incident and announce in `#sdk-parity`.
2. Notify affected partners via the distribution list (email + shared channel)
   with impact, workaround, and next update time.
3. Track remediation tasks (fix, hotfix build, doc updates) in the shared issue
   tracker and reflect status in `status.md`.
4. Close the incident only after telemetry + parity dashboards confirm recovery
   and the weekly digest captures the outcome.
5. For partner-impacting changes, update `docs/source/sdk/swift/index.md`,
   README, and localized pages with the new instructions.

## 8. Documentation & Localization Checklist

- Update this playbook whenever ownership/SLA, release cadence, or command
  surface changes; link revisions in `status.md`.
- Translate critical updates into the Japanese and Hebrew index pages within
  two weeks. Track translation owners in `docs/source/sdk/swift/index.ja.md`
  / `index.he.md`.
- Keep the following docs in sync with releases:
  `docs/source/sdk/swift/index.md`, `native_bridge_instrumentation_checklist.md`,
  `telemetry_redaction.md`, `telemetry_chaos_checklist.md`, and
  `ios2_fixture_cadence_brief.md`.

## 9. References

- `roadmap.md` — IOS8 Production Readiness & Support requirements.
- `status.md` — weekly Swift updates referencing this playbook.
- `Makefile` targets: `swift-fixtures`, `swift-fixtures-check`, `swift-ci`,
  `swift-status`, `swift-dashboards`.
- CI wrappers: `.buildkite/swift-status-export.yml`,
  `ci/swift_status_export.sh`, `ci/xcode-swift-parity`.
- Scripts: `scripts/swift_fixture_regen.sh`,
  `scripts/check_swift_fixtures.py`, `scripts/swift_status_export.py`,
  `scripts/swift_collect_redaction_status.py`,
  `scripts/swift_telemetry_override.py`.

Maintaining parity between this playbook, the reproducibility checklist, and
the telemetry runbooks is mandatory before IOS8 can exit 🈯 status.
