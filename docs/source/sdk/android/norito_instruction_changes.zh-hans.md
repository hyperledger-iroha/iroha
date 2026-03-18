---
lang: zh-hans
direction: ltr
source: docs/source/sdk/android/norito_instruction_changes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7e1d8b230107209e01b794c4f55c9cf5458c2d5ac600263ef68f930f7242d298
source_last_modified: "2026-01-05T09:28:12.057795+00:00"
translation_last_reviewed: 2026-02-07
---

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
   make android-fixtures
   make android-codegen-docs
   ```
   These targets call `scripts/export_norito_fixtures.rs`,
   `tools/norito_codegen_exporter`, and `scripts/android_codegen_docs.py` to
   refresh `instruction_manifest.json`, `builder_index.json`, and the generated
   Markdown references.
2. Run `scripts/check_android_fixtures.py --json-out \
   artifacts/android/parity/latest/summary.json` (or the Buildkite job linked in
   `docs/source/sdk/android/parity_dashboard_plan.md`) to capture the parity
   evidence bundle and alert on drift.
3. Compute the SHA-256 digest of
   `target-codex/android_codegen/instruction_manifest.json` and record it in the
   table below. This value, plus the manifest path, uniquely identify the
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
| Manifest digest | SHA-256 + path of `instruction_manifest.json`. |
| `iroha_data_model` ref | Commit, tag, or PR introducing the change. |
| Summary | Human-readable description of the discriminant/field update. |
| Android follow-up | Required actions (codegen, fixtures, docs, shims). |
| Evidence | Links to artefacts (manifest, builder index, parity summary, docs). |

## Change log

| Date (UTC) | Manifest digest | `iroha_data_model` ref | Summary | Android follow-up | Evidence |
|------------|----------------|------------------------|---------|-------------------|----------|
| 2025-11-22 | `1d14214994b2875e3942ca1218074dcac04f64f0788b62fe57f3a2b44fdf9182` (`target-codex/android_codegen/instruction_manifest.json`) | `bafab4c48460` | Refreshed the manifest/builder docs (104 discriminants) and updated the recorded digests ahead of turning on the Android codegen parity gate. | Regenerate docs via `make android-codegen-docs`, update `codegen_manifest_metadata.json`, and keep `make android-codegen-verify` wired into CI through the new `android-codegen-parity` workflow. | `docs/source/sdk/android/generated/codegen_manifest_metadata.json`, `.github/workflows/android-codegen-parity.yml`, `artifacts/android/codegen_parity_summary.json` |
| 2026-03-05 | `20132f673b82484c5afb7a8c67a38fd5d47735c6e89c4b7e479a4c378ffa93ca` (`target-codex/android_codegen/instruction_manifest.json`) | `439295f10c916124eb8e1bbc7ba2e1894dc2ba84` | Regenerated manifest/builder docs to cover the new governance ballots, manifest fixtures, and trigger metadata exported by `norito_codegen_exporter` (104 discriminants total). | Refresh `docs/source/sdk/android/generated/` via `make android-codegen-docs`, update `codegen_manifest_metadata.json`, and enable the parity gate through `make android-codegen-verify`. | `docs/source/sdk/android/generated/codegen_manifest_metadata.json`, `scripts/check_android_codegen_parity.py`, `artifacts/android/codegen_parity_summary.json` (CI upload). |

Add new rows for every discriminant addition, removal, or schema change.
