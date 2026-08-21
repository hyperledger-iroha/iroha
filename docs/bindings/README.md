# SDK Binding & Fixture Governance

WP1-E on the roadmap calls out “docs/bindings” as the canonical place to keep the
cross-language binding state. This document records the binding inventory,
regeneration commands, drift guards, and evidence locations so the GPU parity
gates (WP1-E/F/G) and the cross-SDK cadence council have a single reference.

## Shared guardrails
- **Fixture owner:** `fixtures/norito_rpc/` is the only authoritative Norito RPC
  fixture corpus. Run `cargo run --locked -p xtask --features dev-tools --bin
  xtask -- norito-rpc-fixtures --output-root <absent-absolute-external-root>` at two
  independent absent roots. Before any tracked update, require identical exact
  path sets, entry types, modes, completion manifests, and every file byte.
  Apply the reviewed identity-relative patch from either sealed root, then run
  `norito-rpc-verify` and the consumer checks. There are no SDK-local fixture
  regeneration entry points.
- **Canonical playbook:** `specs/norito_binding_regen_playbook.md` spells out
  the rotation policy, expected evidence, and the escalation workflow for Android,
  Swift, Python, and future bindings.
- **Norito schema parity:** `scripts/check_norito_bindings_sync.py` (invoked via
  `scripts/check_norito_bindings_sync.sh` and gated in CI by
  `ci/check_norito_bindings_sync.sh`) blocks CI when the Rust, Java, Kotlin, or
  Python schema artefacts drift. Ordinary Cargo builds skip this multi-SDK guard
  unless `NORITO_CHECK_BINDINGS_SYNC=1` is set.
- **Cadence watchdog:** `scripts/check_fixture_cadence.py` reads the
  `artifacts/*_fixture_regen_state.json` files and enforces the Tue/Fri (Android,
  Python) and Wed (Swift) windows so roadmap gates have auditable timestamps.

## Binding matrix

| Binding | Entry points | Fixture / regen command | Drift guards | Evidence |
|---------|--------------|-------------------------|--------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | Two-root owner procedure above → generated descriptor and `.norito` mirror | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/gradlew :core:test` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | Two-root owner procedure above → descriptor-only mirror | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh` | `specs/swift_parity_triage.md`, `specs/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | Two-root owner procedure above → descriptor-only mirror | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `specs/norito_binding_regen_playbook.md`, `specs/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`specs/sdk/js/publishing.md`) | Reads `fixtures/norito_rpc/` directly; refresh with the two-root owner procedure above | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Binding details

### Android (Java)
The Android SDK lives under `java/iroha_android/`. Its files under
`java/iroha_android/src/test/resources/` are a generated mirror, never the source
of fixture truth. The canonical owner command reads the tracked canonical
descriptor and writes only a complete, absent external
output root. Review that sealed tree as a mechanical patch to its identical
tracked relative paths, including the descriptor JSON, manifest, and owned
`.norito` blobs in the Java resource mirror. Drift is
detected by `scripts/check_android_fixtures.py` (also wired into
`ci/check_android_fixtures.sh`) and by `java/iroha_android/gradlew :core:test`, which
exercises the JNI bindings, WorkManager queue replay, and StrongBox fallbacks.
Rotation evidence, failure notes, and rerun transcripts live under
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/Fixtures/` is a descriptor-only mirror of
`fixtures/norito_rpc/transaction_payloads.json` and
`transaction_fixtures.manifest.json`; canonical `.norito` blobs remain in the
owner directory. Use the two-root owner procedure above to render and review the
complete publication. `scripts/check_swift_fixtures.py` plus
`ci/check_swift_fixtures.sh` enforce exact
descriptor parity and reject canonical blob copies in the Swift mirror. The
escalation workflow, KPIs, and dashboards are documented in
`specs/swift_parity_triage.md` and the cadence briefs under
`specs/sdk/swift/`.

### Python
The Python client (`python/iroha_python/`) receives only the canonical payload
descriptor and manifest under `python/iroha_python/tests/fixtures/`; it does not
mirror `.norito` blobs. Use the two-root owner procedure above to render and
review the complete publication. `scripts/check_python_fixtures.py` and
`python/iroha_python/scripts/run_checks.sh` gate pytest, mypy, ruff, and fixture
parity locally and in CI. The end-to-end docs (`specs/sdk/python/…`) and
the binding regen playbook describe how to coordinate rotations with the
canonical owner.

### JavaScript
`javascript/iroha_js/` reads the canonical descriptors and blobs from
`fixtures/norito_rpc/` instead of maintaining an SDK mirror. WP1-E also tracks
its release evidence so GPU CI lanes inherit complete provenance. Every release
captures provenance via `npm run release:provenance` (powered by
`javascript/iroha_js/scripts/record-release-provenance.mjs`), generates and signs
SBOM bundles with `scripts/js_sbom_provenance.sh`, runs the signed staging dry-run
(`scripts/js_signed_staging.sh`), and verifies the registry artefact with
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. The resulting metadata
lands under `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/`, and `artifacts/js/verification/`, providing deterministic
evidence for roadmap JS5/JS6 and WP1-F benchmark runs. The publishing playbook in
`specs/sdk/js/` ties the automation together.
