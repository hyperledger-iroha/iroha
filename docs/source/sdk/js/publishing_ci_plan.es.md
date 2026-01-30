---
lang: es
direction: ltr
source: docs/source/sdk/js/publishing_ci_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 748b9fb125dec3341f1858d153c39ce6774335541797bd0b51064e9f316e56e0
source_last_modified: "2026-01-03T18:08:00.524822+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# JavaScript Publishing CI Plan (JS5)

This plan fulfils the **"Plan publishing automation CI (npm)"** roadmap follow-up
under **JS5 — publishing automation & provenance**. It documents how the
`@iroha/iroha-js` release workflow runs in CI, the guardrails that gate a
release, and the evidence/rollback steps SRE expects before the SDK can reach
GA.

The plan assumes the GitHub Actions workflow
[`.github/workflows/javascript-sdk-publish.yml`](../../../../.github/workflows/javascript-sdk-publish.yml)
remains the single entry point for production publishes. Local helpers such as
`scripts/js_signed_staging.sh`, `scripts/js_sbom_provenance.sh`, and
`javascript/iroha_js/scripts/release-matrix.mjs` provide the same checks for dry
runs, staging rehearsals, and incident drills.

## 1. Pipeline Overview

| Stage | Trigger | CI Steps | Evidence |
|-------|---------|----------|----------|
| **Preparation** | Maintainer submits PR with version + changelog | `npm run check:changelog` via `prepublishOnly`, `npm run lint:test` | PR CI logs, `CHANGELOG.md` diff |
| **Release tag** | GitHub release tagged `js-v<semver>` (preferred) or manual `workflow_dispatch` | `determine release version` step ensures semantic tag; fails if package version mismatches | Workflow log section `Determine release version` |
| **Build/Test** | Workflow job `publish` | `npm ci`, native build via `npm run lint:test` (runs format/lint/tests), Rust toolchain provisioned for N-API build | Workflow logs `Install dependencies`, `Run lint + tests` |
| **Artefact capture** | Same job | `npm pack` to `javascript/iroha_js/dist`, SHA-256 recorded, `scripts/js_sbom_provenance.sh` produces signed CycloneDX bundle | Uploaded artifacts `js-sdk-release-<version>`, `js-sdk-sbom-<version>` |
| **Publish** | `npm publish --access public --provenance` (or dry-run) | Runs once the `npm-publish` environment is approved | `dist/npm-publish.log`, workflow summary |
| **Registry verification** | Post-publish | `npm audit signatures`, `npm view`, `npm dist-tag ls` with retries to block if npm has not propagated metadata | Files in `dist/` and release artifact |
| **Release metadata** | Post-publish | `dist/release-metadata.json` stores git SHA, tarball, checksum, workflow run URL | Artifact `js-sdk-release-<version>` |
| **Evidence bundle** | Local automation | `npm run release:provenance`, `npm run release:matrix`, `scripts/js_signed_staging.sh`, `npm run release:verify` | `artifacts/js-sdk-provenance`, `artifacts/js-sdk-release-matrix`, `artifacts/js/npm_staging`, `artifacts/js/verification` |

## 2. Workflow Details

1. **Trigger & version detection**
   - Release tags must follow `js-vX.Y.Z` (preferred) or `vX.Y.Z`. The workflow
     strips prefixes and validates semver before running. Manual dispatches must
     pass `version=<semver>`.
   - `package.json` *must* already contain the same version; otherwise the
     `Verify package version` step fails.

2. **Test matrix requirements**
   - `npm run lint:test` compiles native bindings, runs ESLint/TypeScript checks,
     unit tests, and the dockerised Torii integration suite (because
     `npm run lint:test` depends on `test:integration`).
   - For release candidates, run the release matrix helper locally *before*
     tagging:
     ```bash
     cd javascript/iroha_js
     npm run release:matrix -- --config scripts/release_matrix.targets.json --matrix-name node-lts
     ```
     Keep the resulting `artifacts/js-sdk-release-matrix/<name>_<stamp>/` folder
     with the provenance bundle. The helper writes `matrix.json`, `matrix.md`,
     and a Prometheus textfile (`matrix.prom`). Use `--metrics-out <path>` to
     override the textfile location and `--textfile-dir <dir>` (or set
     `JS_RELEASE_MATRIX_TEXTFILE_DIR`) when you want the `js_release_matrix_*`
     gauges mirrored into a node_exporter textfile directory for release
     dashboards.

3. **Provenance + SBOM**
   - `npm pack --json` emits both the tarball and `dist/npm-pack.json`. The
     `js_sbom_provenance.sh` helper then signs the CycloneDX report and writes it
     to `artifacts/js/sbom/<version>/` before uploading it as
     `js-sdk-sbom-<version>`.
   - Local releases must run `npm run release:provenance` beforehand to capture
     the tarball/checksum (`artifacts/js-sdk-provenance/v<version>_<stamp>/`).

4. **Publishing & approvals**
   - The job runs in the `npm-publish` environment, so GitHub prompts for human
     approval before `npm publish` executes. Maintainers should review the
     workflow log (version detection, tests, SHA output) before approving.
   - Set `dry-run` when dispatching manually to exercise every gate without
     uploading; logs still land in `dist/` for audit.

5. **Registry checks & telemetry**
   - After the publish step, the workflow loops until `npm audit signatures` and
     `npm view` succeed. Failures block the job so maintainers never end up with
     a half-published release.
   - `npm dist-tag ls` output is stored in `dist/npm-dist-tags.txt` to capture
     current `latest`/`staging` assignments.

6. **Evidence hand-off**
   - Download both workflow artifacts and merge them with the local provenance,
     matrix, and verification folders. Attach the folder paths to `status.md`
     and governance checklists.

## 3. Verification & Rollback Hooks

1. **Verification helper** (`npm run release:verify`)
   ```bash
   cd javascript/iroha_js
   npm run release:verify -- --version 0.0.2 \
     --local ../../artifacts/js-sdk-provenance/v0.0.2_20260219T110000/npm-pack/@iroha-iroha-js-0.0.2.tgz
   ```
   - Downloads the registry tarball, compares SHA-256 values, and writes
     `artifacts/js/verification/v<version>_<stamp>/summary.json`.
   - Run after every publish and during quarterly rollback drills.

2. **Signed staging rehearsal** (`scripts/js_signed_staging.sh <version>`)
   - Builds, packs, and performs `npm publish --dry-run --tag staging`, writing
     logs to `artifacts/js/npm_staging/<version>/`. Use it before big releases or
     when reproducing issues without touching `latest`.

3. **Rollback drill**
   - Steps for the simulated rollback:
     1. Deprecate or dist-tag the bad version (if already public).
     2. Run the staging rehearsal script against the fixed commit.
     3. Regenerate provenance, SBOM, and release matrix.
     4. Re-run `npm run release:verify` to prove the new tarball matches the
        intended artefact.
     5. Publish via the CI workflow and store all evidence with the incident log.

4. **Failure handling**
   - If any workflow step fails, GitHub keeps the partial logs. Rerun is allowed
     only after the root cause is addressed (for example, changelog missing or
     tests failing). Never “retry” by bypassing the changelog or provenance
     scripts.
   - Document failures in `status.md` + incident tracker, referencing the
     workflow run URL from `dist/release-metadata.json`.

## 4. Release-Day Checklist

1. Regenerate provenance (`npm run release:provenance`), release matrix, and SBOM locally.
2. Tag the release (`git tag js-vX.Y.Z && git push --tags`).
3. Publish the GitHub release and approve the workflow.
4. Download artifacts (`js-sdk-release-*` and `js-sdk-sbom-*`) and store them
   alongside the local provenance/matrix folders.
5. Run `npm run release:verify -- --version X.Y.Z` and archive the verification
   summary.
6. Update `status.md` with links to evidence and note the release window.
7. Schedule/record the quarterly rollback drill (staging script + verification).

Following this plan keeps the JS SDK aligned with the JS5 guardrails: every
publish has deterministic evidence, CI automation owns the npm push, and the
rollback workflow is rehearsed with the same scripts that power production.
