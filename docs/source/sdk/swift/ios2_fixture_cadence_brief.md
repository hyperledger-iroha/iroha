<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: Swift Norito Fixture Cadence Brief
summary: Governance-approved cadence, fallback, and monitoring expectations for IOS2 fixture rotations and SRE dashboards.
---

# Swift Norito Fixture Cadence Brief (IOS2)

This brief captures the actionable version of the IOS2/AND1 fixture cadence
decision referenced in the roadmap (“Provide decision brief and fallback manual
cadences; map fallback plan to SRE metrics if governance slips.”). Pair it with
the governance pre-read (`docs/source/swift_fixture_cadence_pre_read.md`) for
historical context and with the automation artefacts listed below whenever you
rotate fixtures or report on parity health.

## Decision Summary

- **Baseline cadence:** Regenerate Swift fixtures within **48 hours** of every
  approved Norito discriminator/ABI change, with a **scheduled slot each
  Wednesday at 17:00 UTC**. Ownership alternates weekly (odd ISO weeks =
  Android Foundations TL, even ISO weeks = Swift Lead) unless
  `SWIFT_FIXTURE_ROTATION_OWNER` overrides the slot in CI.
- **Cadence labels & alerts:** `scripts/swift_fixture_regen.sh` accepts
  `SWIFT_FIXTURE_CADENCE` values `weekly-wed-1700utc` (default),
  `rolling-48h` (continuous 48 h windows), and
  `fallback-mon-thu-utc` (Monday/Thursday slots). The cadence interval
  recorded in `artifacts/swift_fixture_regen_state.json` derives from
  `SWIFT_FIXTURE_CADENCE_INTERVAL_HOURS` (defaults to 48). Set
  `SWIFT_FIXTURE_ALERT_CONTACT`/`SWIFT_FIXTURE_ALERT_CHANNEL` so the state
  file embeds the PagerDuty rotation metadata consumed by the parity
  dashboards.
- **CI enforcement:** `ci/check_swift_fixtures.sh` now honours
  `SWIFT_FIXTURE_EXPECTED_CADENCE` (comma-separated labels) when verifying
  cadence metadata, making it safe to allow fallback/event-driven windows
  without silencing the guardrail.
- **Source of truth:** Until the Rust exporter lands Swift bindings, mirror the
  Android canonical set via `scripts/swift_fixture_regen.sh`. The script emits
  `artifacts/swift_fixture_regen_state.json`, capturing cadence label,
  rotation owner, trigger (`scheduled` vs `event`), archive digests, and slot
  metadata for audit trails.
- **SRE observability:** `scripts/swift_status_export.py` and
  `ci/swift_status_export.sh` publish `swift_parity_success_total`,
  `swift_parity_failure_total`, `swift_parity_status`,
  `swift_parity_outstanding_diffs`, and
  `swift_parity_regen_hours_since_success` to Prometheus plus the
  `dashboards/mobile_ci.swift` / `dashboards/mobile_parity.swift` bundles.
  Alert thresholds mirror the 48 h SLA (warning at 36 h, critical at 48 h).
- **Reporting:** Every regeneration (scheduled or event-driven) must update the
  `status.md` Swift section, attach the regenerated fixture diff, and note the
  parity outcome. Governance minutes reference this brief when cadence slips or
  when overrides are invoked outside the normal rotation.

## Cadence Modes & Escalation

| Mode | Trigger | Owner(s) | Required actions | Evidence & metrics |
|------|---------|----------|------------------|--------------------|
| Scheduled slot | Wednesday 17:00 UTC slot reached; no pending incident | Alternating owners (odd weeks Android Foundations TL, even weeks Swift Lead) | Run `make swift-fixtures`, `make swift-fixtures-check`, and `make swift-ci`. Set `SWIFT_FIXTURE_ROTATION_OWNER` to the on-call name and keep the default `SWIFT_FIXTURE_CADENCE=weekly-wed-1700utc`. | `artifacts/swift_fixture_regen_state.json` updated; `swift_parity_success_total` increments; `status.md` entry records “scheduled” outcome; dashboards remain green. |
| Event-driven regen | Governance-approved Norito change or urgent ABI fix (outside scheduled slot) | Change author (Rust maintainer) + Swift Lead | Run `scripts/swift_fixture_regen.sh` with `SWIFT_FIXTURE_EVENT_TRIGGER=1` and `SWIFT_FIXTURE_EVENT_REASON="<ticket or commit>"`, then re-run `make swift-ci`. Post intent/result in `#sdk-parity`. | State file shows trigger=`event`; `swift_parity_regen_hours_since_success` resets; `swift_parity_outstanding_diffs` drops to zero. |
| Governance fallback | Governance vote slips past 7 days or scheduled slot collides with change freeze | Swift Lead (primary) + Release Eng (backup) | Follow the manual fallback procedure below to keep parity within SLA while governance finalises the decision. File an action item in the next governance sync log and link to the resulting `status.md` note. | `SWIFT_FIXTURE_CADENCE=fallback-weekly-monthu` (see below) recorded in state file, `swift_parity_failure_total` remains unchanged, dashboards annotate the fallback window. |
| Archive replay | Rust exporter publishes signed archive; Swift consumes archive for reproducibility | Norito tooling maintainer + Swift Lead | Invoke `scripts/swift_fixture_regen.sh` with `SWIFT_FIXTURE_ARCHIVE=/path/to/archive.tar.zst` (or ZIP). Confirm SHA/digest fields land in the state file and attach archive metadata to `status.md`. | Archive path + SHA recorded; `swift_parity_success_total` increments; reproducibility checklist (`docs/source/sdk/swift/reproducibility_checklist.md`) references the archive. |

## Manual Fallback Procedure

Use this checklist when governance delays the cadence decision, when the
scheduled slot collides with a release freeze, or when SRE explicitly requests a
manual cadence. The fallback keeps parity within SLA while signalling to
governance that the automated rotation is paused.

1. **announce intent** in `#sdk-parity` (tagging governance chair + SRE). Include
   the reason, expected slot, and ticket/incident reference.
2. **set fallback env vars** before invoking the tooling:

   ```bash
   SWIFT_FIXTURE_CADENCE="fallback-mon-thu-utc" \
   SWIFT_FIXTURE_ROTATION_OWNER="swift-lead" \
   SWIFT_FIXTURE_EVENT_REASON="governance-fallback:<ticket>" \
   make swift-fixtures
   make swift-fixtures-check
   make swift-ci
   ```

   The cadence label propagates into `artifacts/swift_fixture_regen_state.json`
   so dashboards and `scripts/swift_status_export.py` flag the fallback slot.
   When CI needs to tolerate multiple cadence windows (scheduled + fallback),
   export `SWIFT_FIXTURE_EXPECTED_CADENCE=weekly-wed-1700utc,fallback-mon-thu-utc`
   before invoking `ci/check_swift_fixtures.sh`.
3. **update dashboards & metrics** by running `ci/swift_status_export.sh` (locally
   or via Buildkite) so `swift_parity_regen_hours_since_success` resets and the
   parity digest records the unscheduled run.
4. **log the action** in `status.md` (Swift section) and in the governance sync
   minutes, including links to CI runs, state file diff, and any pending follow-up.
5. **close the loop** at the next governance sync: either re-enter the normal slot
   or document why fallback cadence must remain in place.

## Metrics & SRE Hooks

- **`swift_parity_success_total` / `swift_parity_failure_total`** — incremented
  by `scripts/swift_status_export.py` after every `make swift-ci` run. These feed
  Alertmanager rules referenced by SRE’s AND1/IOS2 board.
- **`swift_parity_status` (gauge)** — 1 when the latest digest has zero diffs and
  the 48 h SLA has not been breached, 0 otherwise.
- **`swift_parity_outstanding_diffs`** — number of Norito fixture records that
  still differ from the canonical Android set. Must be zero before shipping.
- **`swift_parity_regen_hours_since_success`** — primary SLA indicator; alerts
  at 36 h (warning) and 48 h (critical). This hook is consumed by
  `dashboards/mobile_ci.swift`, the Buildkite Slack digest, and the
  `swift_status_export.py` log line printed in CI.
- **Dashboard artefacts:** `dashboards/mobile_parity.sample.json`,
  `dashboards/mobile_ci.sample.json`, and the parity feed stored under
  `dashboards/data/` must be updated when fixtures change so `make swift-ci`
  reflects the latest metadata.

## Reporting & Evidence

- **Automation outputs:** Commit or archive the updated fixtures under
  `IrohaSwift/Fixtures`, keep `artifacts/swift_fixture_regen_state.json` in sync,
  and ensure `scripts/check_swift_fixtures.py` passes locally before pushing.
- **Status log:** Add a bullet to `status.md` describing the cadence (scheduled,
  event-driven, or fallback), the trigger, and any follow-up required.
- **Roadmap cross-link:** Reference this brief when closing the IOS2 roadmap
  action (line 1767) so future readers know where the operational details live.
- **Governance minutes:** When fallback is invoked, append the action item to the
  relevant meeting entry (see `docs/source/android_fixture_changelog.md` governance
  log) to keep the shared sync agenda accurate.

Keeping these artefacts in lockstep satisfies the roadmap requirement for a
documented decision brief, gives SRE concrete metrics to monitor, and clarifies
what happens when governance cadence slips.
