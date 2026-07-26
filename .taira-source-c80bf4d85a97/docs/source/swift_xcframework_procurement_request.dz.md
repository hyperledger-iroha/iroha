---
lang: dz
direction: ltr
source: docs/source/swift_xcframework_procurement_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7cfd10c450c0e17aaea2db3da7b3242d0497587192a8973b8b96236d0368bbc6
source_last_modified: "2025-12-29T18:16:36.223593+00:00"
translation_last_reviewed: 2026-02-07
---

# Procurement Request – StrongBox Spare Device

- **Requestor:** Swift QA Lead (qa-swift@sora.org)
- **Date:** 2026-01-27
- **Purpose:** Provide a hot-spare StrongBox-capable iPhone for the XCFramework smoke harness (`xcframework-smoke/strongbox` lane) to avoid downtime when the primary device is under maintenance.
- **Recommended device:** iPhone 15 Pro (128 GB, unlocked) – maintains StrongBox support and future-proofs for iOS 18.
- **Estimated cost:** $1,199 USD (Apple Store business pricing, excluding tax).
- **Accessories:** USB-C to USB-C cable (included) + spare USB-C power adapter (20W) — $19 USD.
- **Total estimated budget:** $1,218 USD + local tax and shipping.

## Justification
- Ensures coverage of the mandatory StrongBox lane documented in `docs/source/swift_xcframework_device_matrix.md`.
- Mitigates CI downtime caused by battery wear, OS reinstallation, or hardware failure.
- Allows parallel smoke runs if the harness needs to scale during release weeks.

## Next steps
1. Submit procurement ticket (`PROC-STRONGBOX-2026`) referencing this document.
2. Coordinate delivery to Cupertino lab rack 3 (slot A12 spare) and update `docs/source/swift_xcframework_hardware_plan.md` once the device arrives.
3. Tag the new device with DeviceKit label `ios15p-strongbox-spare`, enroll it in MDM, and verify the `ci/xcframework-smoke:strongbox:device_tag` metadata appears in the next Buildkite run before declaring the spare ready.
