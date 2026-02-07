---
lang: ba
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 182b1616ff32d0be52d0a6e33178fac4261e7e92a088048b11e5f93e4b005750
source_last_modified: "2026-01-05T09:28:11.838358+00:00"
translation_last_reviewed: 2026-02-07
id: preview-integrity-plan
title: Checksum-Gated Preview Plan
sidebar_label: Preview Integrity Plan
description: Implementation roadmap for securing the docs portal preview pipeline with checksum validation and notarised artefacts.
---

This plan outlines the remaining work required to make every portal preview artefact verifiable before publication. The goal is to guarantee that reviewers download the exact snapshot built in CI, that the checksum manifest is immutable, and that the preview is discoverable through SoraFS with Norito metadata.

## Objectives

- **Deterministic builds:** Ensure `npm run build` produces reproducible output and always emits `build/checksums.sha256`.
- **Verified previews:** Require every preview artefact to ship with a checksum manifest and refuse publication when verification fails.
- **Norito-published metadata:** Persist preview descriptors (commit metadata, checksum digest, SoraFS CID) as Norito JSON so governance tooling can audit releases.
- **Operator tooling:** Provide a one-step verification script that consumers can run locally (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); the script now wraps the checksum + descriptor validation flow end-to-end. The standard preview command (`npm run serve`) now invokes this helper automatically before `docusaurus serve` so local snapshots stay checksum-gated (with `npm run serve:verified` kept as an explicit alias).

## Phase 1 — CI Enforcement

1. Update `.github/workflows/docs-portal-preview.yml` to:
   - Run `node docs/portal/scripts/write-checksums.mjs` after the Docusaurus build (already invoked locally).
   - Execute `cd build && sha256sum -c checksums.sha256` and fail the job on mismatch.
   - Package the build directory as `artifacts/preview-site.tar.gz`, copy the checksum manifest, call `scripts/generate-preview-descriptor.mjs`, and execute `scripts/sorafs-package-preview.sh` with a JSON config (see `docs/examples/sorafs_preview_publish.json`) so the workflow emits both metadata and a deterministic SoraFS bundle.
   - Upload the static site, metadata artefacts (`docs-portal-preview`, `docs-portal-preview-metadata`), and the SoraFS bundle (`docs-portal-preview-sorafs`) so the manifest, CAR summary, and plan can be inspected without re-running the build.
2. Add a CI badge comment summarising the checksum verification result in pull requests (✅ implemented via `docs-portal-preview.yml` GitHub Script comment step).
3. Document the workflow in `docs/portal/README.md` (CI section) and link to the verification steps in the publishing checklist.

## Verification Script

`docs/portal/scripts/preview_verify.sh` validates downloaded preview artefacts without requiring manual `sha256sum` invocations. Use `npm run serve` (or the explicit `npm run serve:verified` alias) to run the script and launch `docusaurus serve` in one step when sharing local snapshots. The verification logic:

1. Runs the appropriate SHA tool (`sha256sum` or `shasum -a 256`) against `build/checksums.sha256`.
2. Optionally compares the preview descriptor’s `checksums_manifest` digest/filename and, when provided, the preview archive digest/filename.
3. Exits non-zero when any mismatch is detected so reviewers can block tampered previews.

Example usage (after extracting the CI artefacts):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI and release engineers should call the script whenever they download a preview bundle or attach artefacts to a release ticket.

## Phase 2 — SoraFS Publishing

1. Extend the preview workflow with a job that:
   - Uploads the built site to the SoraFS staging gateway using `sorafs_cli car pack` and `manifest submit`.
   - Captures the returned manifest digest and SoraFS CID.
   - Serialises `{ commit, branch, checksum_manifest, cid }` into Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. Store the descriptor alongside the build artefact and expose the CID in the pull-request comment.
3. Add integration tests that exercise `sorafs_cli` in dry-run mode to ensure future changes keep the metadata schema stable.

## Phase 3 — Governance & Auditing

1. Publish a Norito schema (`PreviewDescriptorV1`) describing the descriptor structure under `docs/portal/schemas/`.
2. Update the DOCS-SORA publishing checklist to require:
   - Running `sorafs_cli manifest verify` against the uploaded CID.
   - Recording the checksum manifest digest and CID in the release PR description.
3. Wire the governance automation to cross-check the descriptor against the checksum manifest during release votes.

## Deliverables & Ownership

| Milestone | Owner(s) | Target | Notes |
|-----------|----------|--------|-------|
| CI checksum enforcement landed | Docs Infrastructure | Week 1 | Adds failure gate + artefact uploads. |
| SoraFS preview publishing | Docs Infrastructure / Storage Team | Week 2 | Requires access to staging credentials and Norito schema updates. |
| Governance integration | Docs/DevRel Lead / Governance WG | Week 3 | Publishes schema + updates checklists and roadmap entries. |

## Open Questions

- Which SoraFS environment should hold preview artefacts (staging vs. dedicated preview lane)?
- Do we need dual signatures (Ed25519 + ML-DSA) on the preview descriptor before publication?
- Should the CI workflow pin the orchestrator configuration (`orchestrator_tuning.json`) when running `sorafs_cli` to keep manifests reproducible?

Capture decisions in `docs/portal/docs/reference/publishing-checklist.md` and update this plan once the unknowns are resolved.
