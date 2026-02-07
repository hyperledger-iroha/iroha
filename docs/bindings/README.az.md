---
lang: az
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
---

# SDK Binding & Fixture Governance

WP1-E on the roadmap calls out “docs/bindings” as the canonical place to keep the
cross-language binding state. This document records the binding inventory,
regeneration commands, drift guards, and evidence locations so the GPU parity
gates (WP1-E/F/G) and the cross-SDK cadence council have a single reference.

## Shared guardrails
- **Canonical playbook:** `docs/source/norito_binding_regen_playbook.md` spells out
  the rotation policy, expected evidence, and the escalation workflow for Android,
  Swift, Python, and future bindings.
- **Norito schema parity:** `scripts/check_norito_bindings_sync.py` (invoked via
  `scripts/check_norito_bindings_sync.sh` and gated in CI by
  `ci/check_norito_bindings_sync.sh`) blocks builds when the Rust, Java, or Python
  schema artefacts drift.
- **Cadence watchdog:** `scripts/check_fixture_cadence.py` reads the
  `artifacts/*_fixture_regen_state.json` files and enforces the Tue/Fri (Android,
  Python) and Wed (Swift) windows so roadmap gates have auditable timestamps.

## Binding matrix

| Binding | Entry points | Fixture / regen command | Drift guards | Evidence |
|---------|--------------|-------------------------|--------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (optionally `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Binding details

### Android (Java)
The Android SDK lives under `java/iroha_android/` and consumes the canonical Norito
fixtures produced by `scripts/android_fixture_regen.sh`. That helper exports
Fresh `.norito` blobs from the Rust toolchain, updates
`artifacts/android_fixture_regen_state.json`, and records cadence metadata that
`scripts/check_fixture_cadence.py` and governance dashboards consume. Drift is
detected by `scripts/check_android_fixtures.py` (also wired into
`ci/check_android_fixtures.sh`) and by `java/iroha_android/run_tests.sh`, which
exercises the JNI bindings, WorkManager queue replay, and StrongBox fallbacks.
Rotation evidence, failure notes, and rerun transcripts live under
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` mirrors the same Norito payloads via `scripts/swift_fixture_regen.sh`.
The script records rotation owner, cadence label, and source (`live` vs `archive`)
inside `artifacts/swift_fixture_regen_state.json` and feeds the metadata into the
cadence checker. `scripts/swift_fixture_archive.py` allows maintainers to ingest
Rust-generated archives; `scripts/check_swift_fixtures.py` and
`ci/check_swift_fixtures.sh` enforce byte-level parity plus SLA age limits, while
`scripts/swift_fixture_regen.sh` supports `SWIFT_FIXTURE_EVENT_TRIGGER` for manual
rotations. The escalation workflow, KPIs, and dashboards are documented in
`docs/source/swift_parity_triage.md` and the cadence briefs under
`docs/source/sdk/swift/`.

### Python
The Python client (`python/iroha_python/`) shares the Android fixtures. Running
`scripts/python_fixture_regen.sh` pulls the latest `.norito` payloads, refreshes
`python/iroha_python/tests/fixtures/`, and will emit cadence metadata into
`artifacts/python_fixture_regen_state.json` once the first post-roadmap rotation
is captured. `scripts/check_python_fixtures.py` and
`python/iroha_python/scripts/run_checks.sh` gate pytest, mypy, ruff, and fixture
parity locally and in CI. The end-to-end docs (`docs/source/sdk/python/…`) and
the binding regen playbook describe how to coordinate rotations with the Android
owners.

### JavaScript
`javascript/iroha_js/` does not rely on local `.norito` files, but WP1-E tracks
its release evidence so GPU CI lanes inherit complete provenance. Every release
captures provenance via `npm run release:provenance` (powered by
`javascript/iroha_js/scripts/record-release-provenance.mjs`), generates and signs
SBOM bundles with `scripts/js_sbom_provenance.sh`, runs the signed staging dry-run
(`scripts/js_signed_staging.sh`), and verifies the registry artefact with
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. The resulting metadata
lands under `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/`, and `artifacts/js/verification/`, providing deterministic
evidence for roadmap JS5/JS6 and WP1-F benchmark runs. The publishing playbook in
`docs/source/sdk/js/` ties the automation together.
