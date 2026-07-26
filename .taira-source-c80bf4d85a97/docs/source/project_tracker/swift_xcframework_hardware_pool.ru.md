---
lang: ru
direction: ltr
source: docs/source/project_tracker/swift_xcframework_hardware_pool.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cf8413851c82b9bf7b936b2bc771004c258d454cfc59cd6a00132a7e591027fd
source_last_modified: "2026-01-03T18:08:02.023438+00:00"
translation_last_reviewed: 2026-01-30
---

# Swift XCFramework Smoke Harness Hardware Plan

This tracker outlines the hardware requirements and procurement steps needed to
support the IOS6 XCFramework smoke harness on Buildkite/macOS runners.

## Goal

Reserve a dedicated StrongBox-capable iPhone device pool and ensure simulator
fallback capacity so the `xcframework-smoke` pipeline can run continuously
without device contention.

## Current status

- **Device inventory audit:** Build Infra confirmed on 2026-03-05 that three
  iPhone 14 Pro devices (StrongBox-capable, iOS 17.4.1) are idle in the hardware
  lab. The roster export (`devicekit report ios14p-strongbox`) is archived under
  `ops/devicekit/2026-03-05-roster.csv`.
- **Procurement ticket:** Ticket `INFRA-9321` submitted 2026-03-06 with the
  validated roster and approved the next day. Finance signed off on the
  accessory bundle (USB-C power bricks + desk mounts) under PO `FIN-22871`.
- **Runner capacity:** Devices `ios14p-strongbox-01/02/03` pinned to the
  `xcframework-smoke` pipeline on 2026-03-07. Metadata from the first guarded
  run (`buildkite #9821`) shows `device_tag=strongbox` and matches the expected
  Buildkite analytics dashboard.

## Required actions

1. Confirm the number of StrongBox-capable devices required (target: two active
   + one spare) and associated maintenance rotation with the hardware lab.
2. File or update the procurement ticket (INFRA-9321) to cover the device pool
   and any replacement stock; attach the XCFramework device matrix as context.
3. Coordinate with Buildkite admins to pin the new devices to the
   `xcframework-smoke` pipeline (`devicekit: ios14p-strongbox` group) and verify
  _metadata reporting.
4. Update `status.md` once the pool is live and annotate the roadmap milestone.

## Owners & contacts

| Role | Owner | Notes |
|------|-------|-------|
| Swift QA lead | @swift-qa | Coordinates smoke harness execution and device usage |
| Build Infra | @infra-build | Provisions Buildkite runners and devicekit groups |
| Hardware lab | @hardware-lab | Sources devices, maintains StrongBox pool |
| Procurement | @finance-procurement | Approves ticket INFRA-9321 |

## Links

- Device matrix reference: `docs/source/swift_xcframework_device_matrix.md`
- Buildkite pipeline: `.buildkite/xcframework-smoke.yml`
- Harness script: `scripts/ci/run_xcframework_smoke.sh`
- Dashboard feed: `dashboards/mobile_ci.swift`

## Procurement ticket draft

```
Summary: Secure StrongBox iPhone pool for Swift XCFramework smoke harness

- Requestor: Swift QA Lead (@swift-qa)
- Timeline: Needed before XCFramework IOS6 milestone (Sprint 3, Q1 2026)
- Devices requested: 3× iPhone 14 Pro (256 GB), iOS 17.x, StrongBox-enabled
  - 2 units assigned to Buildkite queue `mac-sim` (`devicekit: ios14p-strongbox`)
  - 1 spare in hardware lab rotation for failover
- Accessories: USB-C cables, spare chargers, desk mounts for continuous power
- Runner attachment: Build Infra to wire devices into existing M2 Max runners
- Rationale: Smoke harness requires StrongBox coverage; simulator fallback cannot
  validate Secure Enclave flows. Failing runs currently block pipeline parity dashboards.
- References:
  - Device matrix: docs/source/swift_xcframework_device_matrix.md
  - Harness script: scripts/ci/run_xcframework_smoke.sh
  - Dashboard feed: dashboards/mobile_ci.swift
```

## Action items (status as of 2026-03-01)

- [x] Receive device availability confirmation from Build Infra.  
      _Owner:_ Swift QA liaison (`@swift-qa`).  
      _Status:_ Build Infra responded on 2026-03-05 with roster export `HWPOOL-221` showing three healthy units (battery >85%, Secure Enclave self-test PASS).  
      _Next step:_ None — roster archived under `ops/devicekit/2026-03-05-roster.csv`.
- [x] Submit procurement ticket INFRA-9321 with final device count.  
      _Owner:_ Build Infra program manager (`@build-infra-pm`).  
      _Status:_ Ticket submitted 2026-03-06 with roster + accessory list; finance approved 2026-03-07 and tagged as `budget_line: mobile_ci.ios`.  
      _Next step:_ Track delivery via hardware lab work order `LAB-9924`; update ticket once accessories arrive (due 2026-03-12).
- [x] Validate Buildkite metadata after devices are added to the pool.  
      _Owner:_ CI engineer (`@ci-swift`).  
      _Status:_ Dry-run executed 2026-03-07 using `scripts/ci/run_xcframework_smoke.sh --tags devicekit:ios14p-strongbox --dry-run`; metadata diff stored at `artifacts/swift_ci/2026-03-07-metadata.json`. First live job (`buildkite #9821`) emitted `device_tag=strongbox` and a green incident heartbeat.  
      _Next step:_ Weekly sanity check procedure documented in `docs/source/references/ci_operations.md` under “Weekly XCFramework StrongBox sanity check”.

## Communication template

Copy the following into the Build Infra channel to request the updated device
roster and kick off provisioning:

```
Hi Build Infra team,

We're getting the Swift XCFramework smoke harness (Buildkite `xcframework-smoke`)
ready for the IOS6 milestone and need to confirm StrongBox device availability.

- Request: 3× iPhone 14 Pro (256 GB, iOS 17.x, StrongBox enabled) — 2 active on
  the queue plus 1 spare in rotation
- Queue: `mac-sim` (devicekit label `ios14p-strongbox`)
- Target: devices pinned before Sprint 3 (mid-Feb 2026) so the harness can run
  continuously

Could you share the current roster for `devicekit: ios14p-strongbox` and let us
know if we already have enough inventory? If not, we'll file procurement ticket
INFRA-9321 right away.

Thanks!
```

## Outreach log

| Date (UTC) | Action | Notes |
|------------|--------|-------|
| 2026-02-04 | Drafted Build Infra request template | Pending send by Swift QA lead. |
| 2026-03-05 | Build Infra provided roster export `HWPOOL-221` | Included device health metrics and confirmed three StrongBox units idle in the lab. |
| 2026-03-06 | Procurement ticket `INFRA-9321` submitted | Attached roster, accessory quote, and smoke harness justification; finance looped in. |
| 2026-03-07 | CI engineer validated Buildkite metadata | Dry-run and live job logs archived; on-call notified that strongbox lane is green. |
