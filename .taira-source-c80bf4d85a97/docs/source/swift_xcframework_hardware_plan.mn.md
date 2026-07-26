---
lang: mn
direction: ltr
source: docs/source/swift_xcframework_hardware_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ade1e5aa764276a79e8b91348d67cf5084b5b68e914f256288530106d1b99e44
source_last_modified: "2025-12-29T18:16:36.223155+00:00"
translation_last_reviewed: 2026-02-07
---

# Swift XCFramework Hardware Pool Plan

This note tracks the hardware requirements and outstanding actions needed to keep the
IOS6 XCFramework smoke harness green. Update this file whenever the device inventory or
ownership changes.

## Target matrix recap

See `docs/source/swift_xcframework_device_matrix.md` for the canonical simulator and
physical device coverage. Required physical hardware:

| Device | Minimum OS | Capability | Notes |
|--------|------------|------------|-------|
| iPhone 14 Pro (physical) | iOS 17.x | StrongBox / Secure Enclave | Dedicated to the `xcframework-smoke/strongbox` lane; avoid other CI jobs that may reboot mid-run. |

## Current status (May 2027)

- **Device:** `iPhone 14 Pro` (UDID recorded in SecOps vault entry `CI/ios14p-strongbox`; checksum `sha256:5e6b3fa4b1a60f0d6b6d5a3704b8d603d2677f511ff38d1ad49d6e0b3df4a9d2`)
- **Owner:** Swift QA Lead (contact: qa-swift@sora.org)
- **Location:** Cupertino lab rack 3 (slot A12)
- **Online state:** Buildkite tag and matrix snapshot verified by `scripts/check_xcframework_device_pool.py` (see `configs/swift/xcframework_device_pool_snapshot.json`)

## Action plan

1. **Inventory audit:** Confirm the existing StrongBox-capable devices, capture the actual UDID, and ensure battery health >85%. _Owner: Swift QA Lead. Deadline: Feb 7, 2026. Status: ✅ Complete (snapshot recorded in `configs/swift/xcframework_device_pool_snapshot.json` via `scripts/check_xcframework_device_pool.py`)._
2. **Buildkite agent mapping:** Ensure the `mac-sim` agent selected in
   `.buildkite/xcframework-smoke.yml` has exclusive access to the device (apply DeviceKit
   tag `ios14p-strongbox`). _Owner: Build Infra. Deadline: Feb 10, 2026. Status: ✅ Complete
   (DeviceKit tag enforced in `.buildkite/xcframework-smoke.yml`)._
   - Verify each deployment by checking the next Buildkite run exposes
     `ci/xcframework-smoke:strongbox:device_tag=strongbox`; record failures in the
     telemetry dashboard and alert Swift QA if metadata is missing.
3. **Spare procurement:** Draft procurement request for one additional StrongBox-capable iPhone (15 Pro or newer) to serve as hot spare. Include budget estimate and ETA. _Owner: Swift QA Lead (with Procurement). Deadline: Feb 17, 2026. Status: ✅ Submitted — see ticket `PROC-4123` (linked in `docs/source/swift_xcframework_procurement_request.md`)._
4. **Maintenance runbook:** Draft a brief SOP covering battery cycling, OS updates, and
   steps for re-enrolling the device after hardware resets. Link the SOP here once ready.
   _Owner: Build Infra. Deadline: Feb 21, 2026. Status: ✅ Complete (see [Maintenance SOP](#maintenance-sop))._
5. **Monitoring hook:** Configure Buildkite to send PagerDuty alerts when the StrongBox
   lane is skipped due to device unavailability (`xcframework_smoke_strongbox_unavailable`
   incident). _Owner: Telemetry WG. Deadline: Feb 28, 2026. Status: ✅ Complete (webhook merged in TELEMETRY-366; PagerDuty service `svc_a1b2c3` now receives incidents when the step is skipped)._

## Device pool schedule & booking (effective 2026-05-12)

| Window (UTC) | Days | Responsible team | Purpose / Notes |
|--------------|------|------------------|-----------------|
| 00:00–04:00 | Daily | Swift QA Lead | Reserved for the automated `xcframework-smoke` Buildkite run. M2 runners `mac-sim-01/02` auto-claim the `ios14p-strongbox` tag and block other jobs until the nightly completes. |
| 04:00–06:00 | Tue & Thu | Release Engineering | Buffer window for RC sign-offs, manual reruns, or updating `scripts/ci/run_xcframework_smoke.sh` prior to release announcements. Use only after the nightly window has produced artefacts. |
| 06:00–10:00 | Mon/Wed/Fri | Cross-SDK parity rotation (Swift QA + Android Observability) | Shared slot for Connect/telemetry parity drills that need the same StrongBox device. Android teams must file their replay plan in advance (see booking workflow) and confirm the Swift rerun queue is empty. |
| 10:00–22:00 | Daily | On-demand | Available for ad-hoc investigations (e.g., flaky CI, hardware diagnostics). Requests go through the booking workflow and appear on the shared calendar so Build Infra can pre-stage the device. |
| Weekends (Sat/Sun) 02:00–08:00 | On-demand | Swift QA Lead (primary), Build Infra (secondary) | Optional maintenance windows (OS updates, battery cycling). PagerDuty hook suppresses smoke-run alerts if a maintenance flag is present. |

### Booking workflow

1. **Submit request:** Open a `hardware-lab` queue ticket (component “Swift XCFramework pool”)
   at least 24 h before the desired slot. Include the target window, expected duration,
   and whether Buildkite metadata or manual Xcode runs are required.
2. **Announce in chat:** Post the ticket link in `#build-infra` with the subject “XCFramework pool reservation”
   and @-mention the Swift QA lead plus any cross-SDK stakeholders (Android, JS) that need awareness.
3. **Update calendar:** Hardware lab staff update the shared calendar entry
   (`ops/calendars/swift_xcframework_device_pool.ics`) so the reservation appears next to the nightly run.
4. **Tag enforcement:** Build Infra re-tags the device (if needed) via `devicekit assign --label ios14p-strongbox --ticket <ID>` so no other pipeline can preempt the booking during the reserved window.
5. **Close-out:** After the slot, comment on the ticket with the Buildkite run URL or manual test log,
   then clear any overrides (unset maintenance flag, remove temporary tags).

All reservations must keep at least one daily 00:00–04:00 window free for the automated run.
If an emergency booking overlaps with the nightly slot, the Swift QA lead must sign off in the ticket
and schedule a compensating smoke run within 12 hours.

## Open questions / blockers

- **USB/power for spare:** The spare StrongBox slot is pre-booked alongside the primary lane in the hardware calendar (`ops/calendars/swift_xcframework_device_pool.ics`); the rack PDU port is recorded in the vault entry so Build Infra can re-cable without ad-hoc approval.
- **MDM OS updates:** Quarterly MDM updates run in the Sat 02:00–04:00 LT maintenance window with the `xcframework-smoke` step paused; the booking workflow now requires tagging the maintenance ticket and posting the rerun URL after the manual `xcframework-smoke` verification.
- **Disk headroom:** `scripts/ci/run_xcframework_smoke.sh` enforces a 20 GB minimum on the DerivedData/artefact volume and aborts early with an explicit error if the budget is not met.

## Validation snapshot (2027-05)

- `scripts/check_xcframework_device_pool.py --json-out configs/swift/xcframework_device_pool_snapshot.json` proves the pipeline uses `ios14p-strongbox`, the harness reads `configs/swift/xcframework_device_matrix.json`, and the destination defaults are present. Attach the JSON artefact to readiness reviews.
- Disk budget guard (`ensure_disk_budget`) now runs before the harness kicks off, failing with a clear message if <20 GB is available under the DerivedData/artefact mount.

## Maintenance SOP

> Applies to devices tagged `xcframework-smoke/strongbox`. Last reviewed: May 2027.

1. **Weekly health check (Mondays).**
   - Verify device appears online in MDM (`mdm/xcframework-strongbox`) and that battery health ≥ 85%.
   - Ensure DeviceKit tag `ios14p-strongbox` is still attached to the Buildkite agent; remove any secondary agents that claim the tag.
2. **Battery cycle.**
   - Once per month, disconnect the device from power at 09:00 LT, run the `battery_drain` shortcut (launches test UI automation) until ≤ 25%, then reconnect and charge to 100% before 18:00.
   - Record the cycle in the Ops checklist (`ops/xcframework_battery_cycles.md`).
3. **OS/firmware updates.**
   - Quarterly (aligned with Apple RC releases): schedule an MDM update window outside CI hours (Sat 02:00–04:00 LT).
   - After upgrade, run `xcframework-smoke` manually (`buildkite-agent pipeline upload --pipeline .buildkite/xcframework-smoke.yml --env FORCE_DEVICE=ios14p-strongbox`) and confirm strongbox tests pass.
4. **Re-enrollment / recovery.**
   - If the device is factory-reset or drops from MDM, follow the `SecOps/iOS_reenroll.md` checklist: supervised setup, install the `xcframework-strongbox` profile, and reapply the DeviceKit tag.
   - Run `swift test --filter StrongboxKeychainTests` locally via the agent to confirm Secure Enclave availability before re-enabling the Buildkite step.
5. **Quarterly audit.**
   - Export `xcframework_smoke_strongbox` build metrics to the telemetry dashboard and archive a screenshot in `docs/assets/swift_xcframework/audits/`.

Escalations: notify `build-infra@sora.org` and create a PagerDuty low-urgency incident (`service: xcframework-smoke`) if the device is offline for >12 hours or battery health drops below threshold.

## Monitoring Hook Rollout

- Buildkite pipeline `.buildkite/xcframework-smoke.yml` publishes metrics via the existing
  `ci/xcframework-smoke:strongbox` step. As of **2026-02-26**, the telemetry webhook is live
  and emits a POST request to PagerDuty whenever the step is skipped or exits with code 199 (`DEVICE_UNAVAILABLE`).
- PagerDuty service `svc_a1b2c3` (Swift StrongBox CI) receives the webhook, opens a
  low-urgency incident tagged `xcframework_smoke_strongbox_unavailable`, and notifies the
  on-call rotation (`Swift QA Oncall`). First live alert (incident `INC-48271`) confirmed delivery on 2026-02-27 after a scheduled maintenance skip.
- Manual fallback checks are no longer required; Build Infra only reviews the dashboard if the webhook health check fails (`telemetry_swift_strongbox_webhook` metric).
- Telemetry dashboards (`dashboards/swift_ci.json`) now plot incident counts and mean time to resolution sourced from PagerDuty aggregation metrics.

## Checklist audit (2026-02-28)

- [x] **SecOps vault + MDM record** — Vault entry `CI/ios14p-strongbox` now includes the UDID checksum, DeviceKit tag, and MDM profile pointer. The Swift QA lead confirmed the metadata during the Jan 30 MDM sweep; next audit is scheduled for Apr 2026.
- [x] **Procurement ticket** — Ticket `PROC-4123` is approved and archived in the procurement queue with budget line `$1,420`. Lead time tracker shows ETA 2026‑03‑05; reference added in the SOP escalation section.
- [x] **DeviceKit tagging** — `.buildkite/xcframework-smoke.yml` enforces the `ios14p-strongbox` tag. The last five Buildkite runs report `device_claim=exclusive` in the step metadata, satisfying the exclusivity guard.
- [x] **Maintenance SOP attachment** — Section [Maintenance SOP](#maintenance-sop) is linked from Build Infra’s runbook (`ops/swift_ci_devices.md`) and versioned in Git; the on-call checklist points back here.
- [x] **PagerDuty webhook verification** — Incident `INC-48271` (2026‑02‑27) demonstrates end-to-end delivery. PagerDuty analytics confirm the rule fired within 90 s of the skipped step, meeting the SLA.
