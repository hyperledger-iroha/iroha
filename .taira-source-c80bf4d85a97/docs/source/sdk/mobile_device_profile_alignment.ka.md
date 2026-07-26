---
lang: ka
direction: ltr
source: docs/source/sdk/mobile_device_profile_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e603d40c7632b5bc150d634d57aabc691e9eb01ed9b73f5c2229d71b49a827a
source_last_modified: "2025-12-29T18:16:36.063124+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Mobile Device Profile Alignment (AND7 · IOS7)

Roadmap items **AND7** (Android telemetry redaction) and **IOS7** (Swift
Connect/telemetry) share the same governance gate: telemetry dashboards must
compare “like for like” device classes across SDKs before operators can trust
SLO trends. This note defines the canonical bucket mapping consumed by the
shared dashboards (`dashboards/mobile_parity.swift`) and parity exporters
(`dashboards/data/mobile_parity.sample.json`, `scripts/swift_enrich_parity_feed.py`).

- **Android source:** `android.telemetry.device_profile` gauge emitted by the
  SDK per `docs/source/sdk/android/telemetry_redaction.md` and tracked in the
  signal inventory worksheet (`docs/source/sdk/android/readiness/signal_inventory_worksheet.md`).
- **Swift source:** `device_profile_bucket` field present on
  `swift.offline.queue_depth`, `swift.sdk.error`, and related signals documented
  in `docs/source/sdk/swift/telemetry_redaction.md`.
- **JS/Rust plan:** JS SDK telemetry reuses the same canonical classes once the
  Torii client metrics land (tracked under JS4/JS5); Rust nodes keep exposing
  the underlying hardware SKU list, but dashboards now translate those values
  to the canonical class before overlaying mobile data.

## Canonical Buckets

| Canonical class | Description | Android emission | Swift emission | Notes |
|-----------------|-------------|------------------|----------------|-------|
| `lab` | Non-customer lab devices used in CI, chaos drills, or managed device pools. | `hardware_tier = emulator` (from `android.telemetry.device_profile`). | `device_profile_bucket = simulator`. | Alerts exclude this class by default; parity dashboards keep it for drift detection. |
| `consumer` | Retail handsets/tablets carried by operators or partners. | `hardware_tier = consumer` (SDK majors 21/23/29+ reported via `sdk_level`). | `device_profile_bucket ∈ {iphone_small, iphone_large, ipad}`. | iPad traffic folds into `consumer` so dashboards can compare Android phones/tablets with iOS devices. |
| `enterprise` | Managed or desktop-class runtimes (rugged Android builds, Mac Catalyst apps, kiosk deployments). | `hardware_tier = enterprise`. | `device_profile_bucket = mac_catalyst` (and future `ios_enterprise` bucket if required). | Dashboards treat this class as “managed desktops” and keep it separate from consumer cohorts. |

Any unmapped value falls back to `unknown` during ingestion; dashboards surface
that as a warning panel so owners can update this file and the ingest map.

## Normalisation Rules

1. **Ingest:** Exporters emit the raw SDK-specific fields (`hardware_tier`,
   `sdk_level`, and `device_profile_bucket`). CI tests in
   `ci/sdk_sorafs_orchestrator.sh` and mobile parity jobs make sure the fields
   stay present before release.
2. **Translate:** Dashboard builders run the lookup table above to derive the
   canonical `mobile_profile_class` label. Current implementations:
   `scripts/swift_enrich_parity_feed.py` (Swift weekly digest) and
   `dashboards/mobile_parity.swift`.
3. **Compare:** The canonical class aggregates feed alert rules (bucket drift,
   queue depth deltas) and the readiness screenshots under
   `docs/source/sdk/android/readiness/screenshots/`.
4. **Report:** Governance artefacts reference the canonical class, not the
   SDK-specific buckets, so auditors can confirm that Android, Swift, and
   (eventually) JS metrics talk about the same cohorts.

## Validation & Updates

- Whenever Android adds a new `hardware_tier` value or Swift introduces another
  `device_profile_bucket`, update this file, the schema diff documents, and the
  dashboard data samples in the same patch.
- Nightly schema diff runs (`docs/source/sdk/android/telemetry_schema_diff.md`,
  `dashboards/data/swift_schema.sample.json`) must flag any drift between the
  canonical class and SDK-specific fields.
- The governance follow-ups recorded in both telemetry redaction plans are now
  satisfied by referencing this file; future SDKs (JS/Python) should extend the
  table rather than inventing new per-SDK classes.

Tracking the mapping here keeps AND7/IOS7 in lockstep and makes it obvious when
a new platform value needs release sign-off.
