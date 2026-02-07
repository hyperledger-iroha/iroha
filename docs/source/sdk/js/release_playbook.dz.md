---
lang: dz
direction: ltr
source: docs/source/sdk/js/release_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 129bc4887b6d2365bc2c85e23d6e471febce5edae6d5ab0a761112ca21a5a49d
source_last_modified: "2025-12-29T18:16:36.061404+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# JavaScript SDK Release Playbook

This playbook documents the end-to-end procedure for releasing
`@iroha/iroha-js`. It satisfies the JS-09 roadmap requirement by standardising
Sigstore-enabled publishing, staged dry-runs, evidence capture, and rollback
steps. Follow it whenever you promote a new npm version or re-issue a hotfix.

## Roles & Scope

- **Release owner:** The engineer coordinating the cut. Responsible for running
  the checklist below and filing an incident if anything diverges.
- **Reviewers:** Another SDK maintainer plus Release Engineering. They confirm
  evidence (checksums, workflow logs) and sign off before announcing the new
  version.
- **Repository paths:** `javascript/iroha_js` (sources) and
  `.github/workflows/javascript-sdk-publish.yml` (automation).

## Pre-flight Checklist

1. **Update metadata.**
   - Bump `package.json#version` and add the matching section in
     `CHANGELOG.md`.
   - Ensure the roadmap/status entries describe the release payload.
2. **Install dependencies.**

   ```bash
   cd javascript/iroha_js
   npm ci
   ```

3. **Build bindings and run the full test suite.**

   ```bash
   npm run build:native
   npm test
   npm run check:changelog
   ```

4. **Enable provenance locally (even if automation will publish).**

   ```bash
   export NPM_CONFIG_PROVENANCE=true
   npm login  # requires publish rights
   ```

5. **Record artifacts.** Run `npm run release:provenance` to capture the local
   `npm pack` output (tarball, `npm-pack.json`, `checksums.txt`,
   `metadata.json`) inside `artifacts/js-sdk-provenance/v<version>_<stamp>/`.
   After the publish workflow finishes it uploads a `js-sdk-release-<version>`
   artifact containing the CI tarball, checksums, and release metadata.
   Download that archive and store it alongside the provenance folder and the
   `npm test` logs so auditors can diff CI vs local outputs if needed.
6. **Capture a bundle-size report.**

   ```bash
   npm run report:bundle-size -- --out artifacts/js-sdk-bundle-size/v<version>.json
   ```

   The helper runs `npm pack`, records the tarball checksum, and writes a JSON
   summary listing the package’s total unpacked bytes plus the top offenders.
   Attach the report to the release evidence bundle so roadmap JS-04’s
   “bundle-size impact” gate stays satisfied without waiting for CI artifacts.
   The publish workflow also writes `dist/bundle-size.json` automatically so
   every CI artifact set carries a fresh report.

## Staged Dry-Runs

Dry-runs prove the package is publishable before touching npm:

1. **Tarball inspection**

   ```bash
   npm run release:provenance -- --out-dir artifacts/js-sdk-provenance/staging
   ```

   Inspect the generated `iroha-iroha-js-<version>.tgz` and the accompanying
   `metadata.json`, `npm-pack.json`, and checksum files.

2. **Local publish simulation**

   ```bash
   npm publish --access public --provenance --dry-run
   ```

3. **GitHub workflow dry-run**

   ```bash
   gh workflow run javascript-sdk-publish.yml \
     -f version=<version> \
     -f dry-run=true
   ```

   Attach the workflow URL to the release tracking issue/PR. The dry-run also
   emits a downloadable `js-sdk-release-<version>` artifact (the tarball +
   metadata) so reviewers can inspect the bundle before a real publish.

## Release-candidate matrix workflow (JS5 guardrail)

Roadmap JS5 calls for release-candidate matrix builds so every cut proves the
package compiles, tests, and packs across the supported OS/Node combinations.
Trigger the `javascript-sdk-rc-matrix` workflow after the dry-run succeeds:

```bash
gh workflow run javascript-sdk-rc-matrix.yml
```

The workflow fans out across `ubuntu-latest`, `macos-latest`, and
`windows-latest` with Node.js 18 and 20. Each run installs dependencies,
executes `npm run build:native`, runs `npm test`, and captures an `npm pack`
tarball inside `rc-matrix-<os>-node<version>` artifacts. Download those
artifacts and store them next to the provenance bundle so reviewers can confirm
the RC tarball is portable before the final publish. If a matrix leg fails,
file an issue, fix the regression, and re-run the workflow before proceeding.

## Publishing Paths

### Tag-driven automation (preferred)

1. Merge the release PR.
2. Create a Git tag `js-v<version>` (or `v<version>`) and push it.
3. Publish a GitHub release referencing that tag. The
   `javascript-sdk-publish` workflow:
   - validates the tag format,
   - reruns `npm ci`, `npm run build:native`, `npm test`,
   - records a bundle-size report (`dist/bundle-size.json`) before packaging,
   - runs `npm pack` (capturing `npm-pack.json`, tarball, SHA256 sums),
   - publishes with `npm publish --access public --provenance`,
   - captures `npm audit signatures`, `npm view`, `npm dist-tag ls`, and
     writes `release-metadata.json`,
   - uploads everything as `js-sdk-release-<version>` workflow artifacts.
4. Approve the workflow under the `npm-publish` environment and download the
   artifact for archival.

### Manual workflow dispatch

When no tag exists (e.g., hotfix before notes are ready):

```bash
gh workflow run javascript-sdk-publish.yml \
  -f version=<version> \
  -f dry-run=false
```

Ensure `package.json#version` already matches.

### Emergency CLI fallback

As a last resort (e.g., GitHub outage):

```bash
cd javascript/iroha_js
export NODE_AUTH_TOKEN=<npm automation token>
NPM_CONFIG_PROVENANCE=true npm run release:publish
```

Log the reason in the release issue and upload the terminal transcript.

## Provenance & Evidence

- All publish paths pass `--provenance`, so npm signs the tarball via Sigstore.
- `npm run release:provenance` (see
  `javascript/iroha_js/scripts/record-release-provenance.mjs`) writes the local
  provenance bundle under `artifacts/js-sdk-provenance/v<version>_<stamp>/`
  before any manual publish.
- The GitHub workflow runs `npm pack`, `npm publish`, `npm audit signatures`,
  `npm view`, and `npm dist-tag ls`, then bundles the outputs inside the
  `js-sdk-release-<version>` artifact. Keep that archive with the release logs.
- For extra redundancy you can still run `npm audit signatures` locally and
  drop the output alongside the workflow artifact.

### Signed npm staging dry-run

Before the GA cutover, capture a rehearsed publication so auditors can see the
exact tarball, checksum, and npm output that would have been pushed. Run:

```bash
scripts/js_signed_staging.sh <version>
```

The helper:

1. Installs dependencies and builds the native binding.
2. Runs `npm pack --json`, copies the tarball and metadata into
   `artifacts/js/npm_staging/<version>/`, and records a SHA-256 checksum.
3. Executes `npm publish --provenance --tag staging --dry-run` (respecting
   `NPM_STAGING_REGISTRY` when set) and saves the full log.
4. Writes a `summary.json` with commit hash, timestamp, Node/npm versions, and
   the computed checksum.

Provide `NODE_AUTH_TOKEN` or an `npmrc` pointing at the staging registry so the
dry run exercises the same credentials as the real publish. Upload the entire
`artifacts/js/npm_staging/<version>/` directory with the rest of the release
evidence.

### CycloneDX SBOM workflow (JS5 readiness)

Roadmap JS5 requires a signed SBOM bundle for every release. The
`javascript-sdk-publish` GitHub workflow now runs
`scripts/js_sbom_provenance.sh <version>` automatically and uploads the outputs
as the `js-sdk-sbom-<version>` artifact (tarball, CycloneDX JSON, Sigstore
bundle, checksums).

When you need to regenerate the bundle locally—e.g., before triggering the
workflow or while rehearsing on an air-gapped laptop—run:

```bash
scripts/js_sbom_provenance.sh <version>
```

The helper builds the native binding, runs `npm pack`, generates a CycloneDX
report via `syft`, signs it with `cosign`, and writes everything to
`artifacts/js/sbom/<version>/`. Ensure `syft` and `cosign` are on PATH (override
with `SYFT`/`COSIGN`) before invoking the script, then archive the directory
with the rest of the release evidence.

## Post-release Actions

1. Run the doc helper to refresh release notes:

   ```bash
   npm run release:update-docs -- --version <version> --notes-file CHANGELOG.md
   ```

2. Update `status.md` and `roadmap.md` entries pointing to the new version.
3. Announce the release (Slack/email) linking to:
   - GitHub release,
   - workflow run,
   - evidence folders (`artifacts/js-sdk-provenance/...` plus the downloaded
     workflow artifact).

## Rollback & Incident Response

1. **Within 72 h:** `npm unpublish @iroha/iroha-js@<version>`.
2. **After 72 h:** `npm deprecate @iroha/iroha-js@<version> "<reason>"` and
   publish a fixed `<version+patch>` release with the bugfix.
3. Delete/recreate the `js-v<version>` Git tag to reflect the rollback.
4. Record the incident in `status.md` (Release Notes section) and open a
   follow-up issue documenting corrective actions.

## Release Checklist

| Step | Command / Evidence |
|------|--------------------|
| Metadata updated | `package.json`, `CHANGELOG.md`, roadmap/status entries |
| Tests & changelog guard | `npm run build:native && npm test && npm run check:changelog` |
| Signed staging rehearsal | `scripts/js_signed_staging.sh <version>` |
| RC matrix workflow | `gh workflow run javascript-sdk-rc-matrix.yml` |
| Dry-run tarball | `npm pack --pack-destination artifacts/js-release/<version>` |
| Workflow dry-run | `gh workflow run ... -f dry-run=true` |
| Publish | Tag release **or** workflow dispatch / manual publish |
| Provenance audit | `npm audit signatures @iroha/iroha-js@<version>` |
| Docs refresh | `npm run release:update-docs -- --version <version>` |
| Verification | `npm view @iroha/iroha-js version`, `npm dist-tag ls @iroha/iroha-js` |
| Rollback-ready | Incident/rollback plan acknowledged in release issue |

Archive the filled checklist with the release evidence so auditors can replay
the process without re-running the commands.
