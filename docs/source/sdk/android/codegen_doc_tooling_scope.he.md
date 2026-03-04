---
lang: he
direction: rtl
source: docs/source/sdk/android/codegen_doc_tooling_scope.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 654d17db5bcab1e1e31eb23e66cceefe7dee44b43a29c94ddcd2186953b4e150
source_last_modified: "2026-01-03T18:08:00.613201+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Codegen Documentation Tooling Scope (AND3)

This document scopes the documentation generation tooling required by roadmap
item **AND3 — Norito codegen MVP**. The goal is to turn the metadata already
produced by the Kotlin/Java codegen pipeline into deterministic reference docs
that stay in lockstep with Rust data-model changes and can be published in the
Docs/DevRel portal without manual editing.

## Objectives

- **Authoritative reference:** Generate per-instruction/per-builder API docs
  directly from the codegen metadata so Kotlin docs track Norito schema changes
  within the 48-hour SLA defined for AND1/AND3.
- **Deterministic artefacts:** Produce Markdown + Dokka bundles with embedded
  hashes and provenance notes so Release Engineering can attest to the output.
- **Portal-ready content:** Emit Docs portal sections (EN) plus Ja/He stubs that
  feed `docs/i18n/manifest.json` and keep localized copies in sync.
- **Automation hooks:** Provide `make android-codegen-docs` and a CI workflow
  so docs regenerate automatically whenever codegen manifests change.

## Deliverables

### D1 — Metadata & Manifest Export (Android Data Model TL)
- Extend the existing Norito codegen run to emit `target-codex/android_codegen/`
  metadata:
  - `instruction_manifest.json` — discriminant, schema hash, field metadata,
    doc comments from the Rust source.
  - `builder_index.json` — generated Kotlin/Java builder names, target package,
    feature gates, and stability annotations.
  - `instruction_examples/` — sample Norito payloads per discriminant captured
    via the Rust fixture exporter.
- Attach a `SHA256` digest file alongside each manifest so CI can assert
  provenance (mirrors fixture regeneration gating).
- Ownership: Android Data Model TL (0.5 eng-week).

### D2 — Doc Generator CLI (Docs/DevRel + Android Data Model TL)
- New tool: `scripts/android_codegen_docs.py` that consumes the manifests above
  and renders:
  - `docs/source/sdk/android/generated/instructions/*.md` (one file per
    discriminant with field tables, Norito examples, Kotlin usage snippet).
  - `docs/source/sdk/android/generated/manifest_catalog.md` (index referencing
    discriminant ↔ builder mapping with schema hashes).
  - Dokka sourcesets wired into `java/iroha_android/build.gradle` so `./gradlew
    dokkaHtml -PcodegenDocs` produces HTML+Javadoc bundles that match the
    Markdown content.
- The CLI supports `--locale` to emit Ja/He stubs (English text wrapped in
  `<!-- translate -->` blocks) so `scripts/sync_docs_i18n.py` can pull in
  the new files automatically. Example:
  `python3 scripts/android_codegen_docs.py --manifest target-codex/.../instruction_manifest.json --builders target-codex/.../builder_index.json --out docs/source/sdk/android/generated --locale ja --locale he`.
- Ownership: Docs/DevRel (0.25 eng-week) partnering with Android Data Model TL
  (0.25 eng-week for schema annotations).

### D3 — Publishing & Validation (Docs/DevRel + Release Engineering)
- Add `make android-codegen-docs` that executes:
  1. `cargo run -p norito_codegen_exporter -- --out target-codex/android_codegen`
     (existing step).
  2. `python3 scripts/android_codegen_docs.py --manifest target-codex/...`.
  3. `./gradlew :java:iroha_android:dokkaHtml -PcodegenDocs`.
  4. `scripts/docs/hash_tree.sh docs/source/sdk/android/generated`.
- New CI workflow `.github/workflows/android_codegen_docs.yml` (mirrors
  fixture-check) that runs the make target, uploads artefacts, and fails if the
  docs are stale.
- Publish guidance:
  - Link the generated Markdown via `docs/source/sdk/android/index.md`.
  - Drop Dokka HTML archives in `artifacts/android/codegen_docs/<rev>/`.
  - Update `status.md` when new docs are published, referencing manifest hashes.
- Ownership: Docs/DevRel (0.25 eng-week) + Release Engineering (0.25 eng-week).

## Implementation Plan & Timeline

| Work Package | Owner(s) | Duration | Target Window | Dependencies |
|--------------|----------|----------|---------------|--------------|
| WP1 — Metadata export hardening | Android Data Model TL | 0.5 eng-week | 2026-03-07 | Norito fixture exporter, `AND1` regeneration hooks |
| WP2 — Doc generator CLI + Dokka wiring | Docs/DevRel + Android Data Model TL | 0.5 eng-week | 2026-03-21 | WP1 manifests, Kotlin builder annotations |
| WP3 — Publishing workflow & CI | Docs/DevRel + Release Eng | 0.5 eng-week | 2026-04-04 | WP2 CLI, `docs/i18n` sync tooling |

Total bandwidth: **1.5 eng-weeks** (Android Data Model TL) + **0.5 eng-week**
(Docs/DevRel) + **0.25 eng-week** (Release Eng) spread across Q2 2026 sprint
planning, matching the resource call-out in `roadmap.md`.

## Tooling Stack

- Rust exporter (`norito_codegen_exporter`) for schema metadata.
- Python 3.11 script for Markdown generation and locale stubs.
- Gradle/Dokka for Kotlin API docs (`dokkaHtml` + Dokka JSON for future portal
  ingestion).
- Existing Docs linting (`scripts/check_markdown.sh`) plus `docs/i18n`
  synchronization to keep localized placeholders fresh.

## Testing & Acceptance

- `make android-codegen-docs` passes locally and in CI with no uncommitted
  diffs; `scripts/git/check_clean_tree.sh` will guard against stale generated
  files.
- Dokka output zipped under `artifacts/android/codegen_docs/<git-sha>.zip` and
  attached to the CI run.
- Markdown lint + link-check succeed; `docs/portal` preview picks up the new
  content via checksum gated publish.
- Sign-off required from Android Data Model TL + Docs/DevRel Manager; once
  approved, add an entry under `status.md` → Android → AND3.

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Schema/manifest drift between Rust and Android runs | Broken builder docs or stale hashes | Tie manifest version to [`norito_instruction_changes.md`](norito_instruction_changes.md), fail CI when discriminants differ, document fallback workflow in `scripts/android_fixture_regen.sh`. |
| Translation backlog for Ja/He copies | Docs portal shows outdated locales | Emit stub files with checksum markers so localization contractors can diff and translate asynchronously; keep stubs in git until the translated body lands and update the front-matter status accordingly. |
| CI runtime spikes due to Dokka | Longer Android pipeline waits | Run Dokka only when manifests change (hash compare), cache Gradle outputs, and document override env vars for emergency skips. |
| Ownership gaps once automation lands | Docs fall behind | Runbook described in this scope will be linked from `status.md`, naming Docs/DevRel as DRI with Android Data Model TL as backup. |

## Next Steps

1. **Schema manifest alignment (due 2026-02-28):** Confirm final JSON schema for
   `instruction_manifest.json` / `builder_index.json` with Rust data-model
   maintainers; record sample files under `target-codex/android_codegen/sample/`.
2. **Prototype CLI (due 2026-03-14):** Implement
   `scripts/android_codegen_docs.py` with Markdown emission + locale stub
   support; validate against current manifests.
3. **Wire Dokka & CI (due 2026-03-28):** Add Gradle task, Makefile target, and
   GitHub Actions workflow; capture the process in README snippets plus
   `docs/source/sdk/android/index.md`.
4. **Governance review (due 2026-04-04):** Present this scope plus prototype
   output at the AND3 sprint review to unlock remaining automation work.

Once these steps land, the roadmap item “Scope codegen doc generation tooling”
can move to 🈴 Completed and the follow-on implementation work will track under
the main AND3 milestone.

**Status (2026-03-24):** `norito_codegen_exporter` and the companion
`scripts/android_codegen_docs.py` generator now automate the manifest → Markdown
pipeline, producing `docs/source/sdk/android/generated/` via
`make android-codegen-docs`. Hash-tree metadata lives in
`docs/source/sdk/android/generated/codegen_hash_tree.json` (emitted by
`scripts/docs/hash_tree.sh`) so governance reviewers can diff the generated docs
without re-running the pipeline, and the JSON hash tree becomes part of the
artefacts uploaded by CI.
