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
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication
   make android-codegen-docs
   ```
   The xtask command is the sole fixture owner. Each invocation writes the
   canonical corpus, generated Java resource mirror, and descriptor-only Python
   and Swift mirrors into an absent external publication without touching
   tracked files. Before any tracked update, require identical exact path sets,
   entry types, modes, completion manifests, and every file byte, then apply the
   reviewed identity-relative patch from either sealed root. There is no
   Android-specific regeneration entry point. The codegen-docs target uses
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
| 2026-08-02 | `405804a394ac2de5454b51c9d8a494cad0ff622e96765292a33c65bf6f0af1e7` (`target-codex/android_codegen/instruction_manifest.json`) | H22 first-release reset (base HEAD `7ed17fcff33be137b33641e135d4bca917fbd02f`) | Regenerated the current 113-entry catalog. `Unshield` has exactly `asset`, `to`, `public_amount`, `inputs`, `proof`, and `root_hint`; the catalog also reflects the current governance, SoraFS, transfer-batch, and smart-contract upload schemas. | Refreshed the generated instruction, builder, manifest-catalog, metadata, and hash-tree artifacts and checked manifest/builder parity. | `specs/sdk/android/generated/codegen_manifest_metadata.json` (builder index `2c1ccae9fa6ff98a8abaf9282ec0d7efee14550f223691ea90383d10ef2c47aa`, 113 entries), `specs/sdk/android/generated/codegen_hash_tree.json` (tree `12cf40d746ad92dc6c4afc1dbc089d99c3f464ff3f9bb7d76039e2f7f64aab0b`), `artifacts/android/codegen_parity_summary.json` |
| 2026-08-03 | `f1629935bf10c0b8ceb1e4adc9bea473ce832a70470e171ecceace7aaaf5002d` (`target-codex/android_codegen/instruction_manifest.json`) | V1 generic confidential movement hard cut | Removed the generic `Shield`, `ZkTransfer`, and `Unshield` discriminants from the Android projection while retaining typed Kagemusha V1 proof/settlement APIs. The resulting catalog has 110 entries. | Removed Kotlin/Java instruction and signer shims, refreshed generated instruction/builder/catalog pages, and added negative class/method coverage. | `specs/sdk/android/generated/codegen_manifest_metadata.json` (builder index `3b6a2db6cd3590f0c3fe71cac3e3ddc9dc9de679095ddf947ffaf4cb80aabfb9`, 110 entries), `specs/sdk/android/generated/codegen_hash_tree.json` (tree `1ee981cb62b1a7ad4a8574aa115e3622d114010a39e8bc1d9e67ca219d0b7b74`) |
| 2026-08-08 | `be0c6cf3440b14ccaec1b1e052d6c2ef2bce453924c83fe38c6546b28736f4e6` (`target-codex/android_codegen/instruction_manifest.json`) | First-release SoraFS pin consensus-time hard cut | Removed caller-supplied lifecycle epochs from `RegisterPinManifest`, `ApprovePinManifest`, and `RetirePinManifest`; Core now derives each recorded epoch from the block consensus timestamp. | Removed Kotlin/Java setters and wire fields, made argument decoders reject retired epoch keys, refreshed generated docs/fixture, and added negative tests. | `specs/sdk/android/generated/codegen_manifest_metadata.json` (builder index `cecf23e165ce2fff0974b60100ee01d8231a5f0a39535a7e39676444fc683ecb`, 112 entries), `specs/sdk/android/generated/codegen_hash_tree.json` (tree `f866de2d71f38825342f1124fdc72e2109c2eda7c16eceae4bacc82d56f797ec`) |

Add new rows for every discriminant addition, removal, or schema change.
