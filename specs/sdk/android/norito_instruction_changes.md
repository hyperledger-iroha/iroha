<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Norito Instruction Change Log (AND3)

Roadmap item **AND3 — Norito Parity & Instruction Builders** requires Android
to keep a deterministic log of every `InstructionBox`/Norito discriminant
addition, removal, or schema tweak. This document captures the manifest hash,
`iroha_data_model` reference, and the Android follow-up so governance reviews
can verify that codegen, fixtures, and documentation kept pace with the Rust
toolchain.

## Update workflow

Use the checklist below whenever a Norito instruction or discriminant changes:

1. Regenerate the manifest and fixture artefacts:
   ```bash
   cargo run -p xtask --features dev-tools --bin xtask -- norito-rpc-fixtures
   make android-codegen-docs
   ```
   The xtask command is the sole owner of `fixtures/norito_rpc`: it refreshes the
   canonical corpus, the generated Java resource mirror, and the descriptor-only
   Python and Swift mirrors in one publication. `make android-fixtures` is only a
   convenience wrapper around that owner. The codegen-docs target uses
   `tools/norito_codegen_exporter` and `scripts/android_codegen_docs.py` to refresh
   `instruction_manifest.json`, `builder_index.json`, and the generated Markdown
   references; it does not create a second fixture source.
2. Run `scripts/check_android_fixtures.py --json-out \
   artifacts/android/parity/latest/summary.json` (or the Buildkite job linked in
   `specs/sdk/android/parity_dashboard_plan.md`) to capture the parity
   evidence bundle and alert on drift.
3. Record the deterministic canonical SHA-256 digest from
   `specs/sdk/android/generated/codegen_manifest_metadata.json`. The
   parity tooling sorts manifest entries and blanks the wall-clock
   `generated_at` value before hashing; do not record the timestamp-dependent
   raw file digest. This value, plus the manifest path, uniquely identifies the
   schema snapshot used by Android.
4. Add a row to the log with:
   - Date in UTC.
   - The Rust commit (or tag) that introduced the change.
   - A concise summary of the affected instructions (new variant, removed
     field, doc-only update, etc.).
   - Required Android follow-up (codegen run, Kotlin doc refresh, fixtures, CI
   - Links to supporting evidence (parity summary, PR, roadmap status entry,
     Grafana snapshot).
5. Reference the new entry from the pull request/commit description so reviewers
   can confirm the change log was updated as part of the release workflow.

## Entry fields

| Column | Description |
|--------|-------------|
| Date (UTC) | Day the manifest/parity bundle was regenerated. |
| Manifest digest | Deterministic canonical SHA-256 + path of `instruction_manifest.json`; this is not the raw file-byte digest. |
| `iroha_data_model` ref | Commit, tag, or PR introducing the change. |
| Summary | Human-readable description of the discriminant/field update. |
| Android follow-up | Required actions (codegen, fixtures, docs, shims). |
| Evidence | Links to artefacts (manifest, builder index, parity summary, docs). |

## Change log

| Date (UTC) | Manifest digest | `iroha_data_model` ref | Summary | Android follow-up | Evidence |
|------------|----------------|------------------------|---------|-------------------|----------|
| 2025-11-22 | `1d14214994b2875e3942ca1218074dcac04f64f0788b62fe57f3a2b44fdf9182` (`target-codex/android_codegen/instruction_manifest.json`) | `bafab4c48460` | Refreshed the manifest/builder docs (104 discriminants) and updated the recorded digests ahead of turning on the Android codegen parity gate. | Regenerate docs via `make android-codegen-docs`, update `codegen_manifest_metadata.json`, and keep `make android-codegen-verify` wired into CI through the new `android-codegen-parity` workflow. | `specs/sdk/android/generated/codegen_manifest_metadata.json`, `.github/workflows/android-codegen-parity.yml`, `artifacts/android/codegen_parity_summary.json` |
| 2026-03-05 | `20132f673b82484c5afb7a8c67a38fd5d47735c6e89c4b7e479a4c378ffa93ca` (`target-codex/android_codegen/instruction_manifest.json`) | `439295f10c916124eb8e1bbc7ba2e1894dc2ba84` | Regenerated manifest/builder docs to cover the new governance ballots, manifest fixtures, and trigger metadata exported by `norito_codegen_exporter` (104 discriminants total). | Refresh `specs/sdk/android/generated/` via `make android-codegen-docs`, update `codegen_manifest_metadata.json`, and enable the parity gate through `make android-codegen-verify`. | `specs/sdk/android/generated/codegen_manifest_metadata.json`, `scripts/check_android_codegen_parity.py`, `artifacts/android/codegen_parity_summary.json` (CI upload). |
| 2026-07-12 | `769ce89c0686acecd107d91bbe5d7fdaf638e0e1fabf19cfe5d2b00d11115334` (`target-codex/android_codegen/instruction_manifest.json`) | first-release `RegisterPinManifest` schema migration | Regenerated the 104-entry manifest and builder catalog after folding the retired chunk digest into `manifest_payload` and retaining the epoch, alias, and successor fields. | Refreshed every Android generated instruction, builder, and manifest-catalog page and replayed the SoraFS pin-registration fixture. | `specs/sdk/android/generated/codegen_manifest_metadata.json`, `specs/sdk/android/generated/codegen_hash_tree.json`, `artifacts/android/codegen_parity_summary.json` |
| 2026-07-25 | `cf97bbea1e90a41b700776e4022e66caba44cf71845b027a472c3231343ff4c9` (`target-codex/android_codegen/instruction_manifest.json`) | uncommitted Kotodama V1 reset (base HEAD `136bd4f1006609970e85a3174e1d58d1674b2e4e`) | Regenerated the 107-entry manifest for the Quantity schema migration across mint/burn/transfer, plain-ballot, and shield/unshield payloads, and added `UploadSmartContractCodeChunk`, `FinalizeSmartContractCodeUpload`, and `CancelSmartContractCodeUpload`. | Regenerate every Android instruction, builder, and manifest-catalog page, replay the SoraFS pin-registration fixture, and rerun the codegen parity check. | `specs/sdk/android/generated/codegen_manifest_metadata.json` (builder index `63756519f24fe11b14f553ef4349c4abe2bc2092ccf120e5b94cdfd554feba37`, 107 entries), `specs/sdk/android/generated/codegen_hash_tree.json`, `artifacts/android/codegen_parity_summary.json` |

Add new rows for every discriminant addition, removal, or schema change.
