<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Publishing @iroha/iroha-js

This document captures the guidance for tagging and publishing the
Iroha JS SDK to npm. The current repository does not contain the
previously documented `javascript-sdk-publish` workflow, so the checked-in
package scripts and release helpers below are the authoritative release path.

## Authoritative Release Boundary

Run release work from an exact, clean source revision. A tag or GitHub release
does not publish the package by itself in the current repository. The release
owner must run the checked-in gates, retain their evidence, and invoke the
publish command explicitly.

`npm run build:native` is the only supported generator for the local native
pair. It runs `scripts/build-native.mjs` followed by
`scripts/copy-native.mjs`. The builder seals the source observed before and
after one Cargo invocation and records V3 provenance with
`build_execution_policy: "trusted-local-cargo-v1"`. The publisher then requires
both the build provenance and current source to be clean and at the same Git
revision, signs the staged Mach-O ad hoc on Darwin, verifies the current native
ABI and required exports, and transactionally publishes:

```text
javascript/iroha_js/native/iroha_js_host.node
javascript/iroha_js/native/iroha_js_host.checksums.json
```

The generated manifest binds the native bytes to the exact source revision,
source-tree digest, Cargo profile, and, where applicable, a platform-specific
signing-independent digest. Do not edit the manifest or substitute a checksum
by hand. If an old binary or invalid binary/manifest pair remains from another
revision, preserve it outside the destination or remove it before rebuilding;
the transactional publisher deliberately refuses to overwrite unauthenticated
state.

V3 provenance is a trusted-local-build statement, not a reproducible-build or
hostile-executor proof. When policy requires independently produced native
release evidence, obtain a complete binary and manifest pair from a controlled
builder running this same generator against the exact clean release revision,
verify its supplied release/signing evidence, and expose that verified
directory through `IROHA_JS_NATIVE_DIR`. Never reconstruct a manifest around a
binary whose V3 build provenance is unavailable.

## Prerequisites

1. Ensure the version, changelog, and release notes reflect the upcoming
   release, then confirm that the source tree is clean:

   ```bash
   git status --short
   ```

2. Install dependencies and generate the native pair from the source checkout:

   ```bash
   cd javascript/iroha_js
   npm ci
   npm run build:native
   ```

   The default native profile is `debug`. If the release policy calls for
   another supported profile, select it explicitly; the choice is authenticated
   in both build provenance and the checksum manifest. Keep the variable set
   for every subsequent build and test gate:

   ```bash
   export IROHA_JS_NATIVE_BUILD_PROFILE=release
   npm run build:native
   ```

3. Run the complete test and release gates. `npm test` invokes
   `npm run build:native` before the Node test runner, so the native evidence is
   regenerated and revalidated rather than trusted from an earlier command:

   ```bash
   npm test
   npm run check:changelog
   npm run bundle:check
   npm run test:pack-install
   ```

4. Run `npm run release:provenance`. When a native binding is present, this
   gate requires a valid V3 manifest, the exact clean release revision, and the
   current native ABI/export contract before recording it as release evidence.

5. Log in to npm using an account with publish rights:

   ```bash
   npm login
   ```

6. Export `NPM_CONFIG_PROVENANCE=true` to enable Sigstore provenance for the
   package tarball (requires npm 9.5+).

## Metadata & Automated Checks

- `package.json` now declares the repository, homepage, files allowlist, and
  `publishConfig` (public access + provenance). No additional CLI flags are
  required when running `npm publish`.
- Run `npm run check:changelog` to ensure `CHANGELOG.md` contains an entry for
  the package version. The `prepublishOnly` hook runs this check automatically,
  followed by `npm test`, so publishing without updating the changelog or
  running the test suite will fail fast.

## Provenance Bundle

- Run `npm run release:provenance` before publishing to capture the exact
  `npm pack` tarball, `npm-pack.json`, and deterministic checksums inside
  `artifacts/js-sdk-provenance/v<version>_<timestamp>/`. The script records
  `metadata.json` with git commit, Node/npm versions, and SHA-256/SHA-512
  digests so reviewers can audit the artefact without rerunning the build. A
  verbatim copy of `package.json` is stored alongside the metadata so auditors
  can confirm the published manifest matches the provenance bundle without
  network access.
- `npm run release:publish` invokes the package's `prepublishOnly` lifecycle,
  which runs the checked-in gates and this provenance helper before npm is
  allowed to publish.

### Signed SBOM (JS5 gate)

- Generate the signed SBOM bundle explicitly before publishing:

  ```bash
  scripts/js_sbom_provenance.sh <version>
  ```

  Ensure `syft` and `cosign` are installed. The script writes the npm tarball,
  CycloneDX report, Sigstore bundle, and checksums to
  `artifacts/js/sbom/<version>/`; archive that directory with the release
  evidence.

### Release Candidate Matrix (JS5 gate)

- Copy `javascript/iroha_js/scripts/release_matrix.targets.example.json` to
  `javascript/iroha_js/scripts/release_matrix.targets.json` (this path is
  `.gitignore`d) and adjust it to list the Node/OS combinations you exercise.
  Each target defines a human-readable `label`, the `command` to execute (for
  example `nvm use 20 && npm run lint:test`), an optional `versionCommand`
  (useful when `command` switches Node versions), `timeoutSeconds`, and any
  environment overrides required for that slot.
- Run the matrix helper before publishing to collect deterministic evidence:

  ```bash
  cd javascript/iroha_js
  npm run release:matrix -- \
    --config scripts/release_matrix.targets.json \
    --matrix-name node-lts
  ```

  The script (`scripts/release-matrix.mjs`) executes each target sequentially,
  streams the output to the console, and writes per-target logs, `matrix.json`,
  `matrix.md`, and a Prometheus textfile (`matrix.prom`) under
  `artifacts/js-sdk-release-matrix/<label>_<timestamp>/`. Attach these artifacts
  to the release evidence bundle so governance reviews can confirm which Node
  versions exercised the candidate build.

  Use `--metrics-out <path>` to store the Prometheus gauges elsewhere and
  `--textfile-dir <dir>` (or set `JS_RELEASE_MATRIX_TEXTFILE_DIR`) to mirror the
  gauges into a node_exporter textfile directory so CI dashboards can scrape
  `js_release_matrix_*` counters automatically.

## Verification & Rollback Drill (JS5 Guardrail)

Roadmap item **JS5 — publishing automation & provenance** requires a repeatable
guardrail that proves the registry tarball matches the artefact we staged
locally and that operators can rehearse the rollback path. Use the new verifier
as part of every release and as the first step of any rollback drill.

1. Compare the local artefact (from `npm pack` or the provenance bundle) against
   the registry copy:

   ```bash
   cd javascript/iroha_js
   npm run release:verify -- --version 0.0.2
   # or to compare against a saved artefact
   npm run release:verify -- --version 0.0.2 --local ../../artifacts/js-sdk-provenance/v0.0.2_20260219T110000/npm-pack/@iroha-iroha-js-0.0.2.tgz
  ```

  The script (`scripts/verify-release-tarball.mjs`) downloads
  `@iroha/iroha-js@<version>` from the selected registry (defaults to npm),
  compares both SHA-256 *and* SHA-512 digests, and writes evidence to
  `artifacts/js/verification/v<version>_<timestamp>/summary.json` plus the
  companion `checksums.txt`. Mismatches exit non-zero and highlight the hash
  pairs so rollback drills have deterministic evidence.

2. If a mismatch occurs, run the signed staging rehearsal script to rebuild the
   intended artefact, publish the rollback candidate to the staging tag, and
   repeat verification before promoting the fix:

   ```bash
   scripts/js_signed_staging.sh 0.0.2
   npm run release:verify -- --version 0.0.2 --registry https://registry.npmjs.org/
   ```

3. Store the verification artefacts alongside the provenance and SBOM bundles,
   then add a short entry to `status.md` (referencing the verification folder)
   so governance reviews can trace when the guardrail was executed.

These steps must be exercised during the quarterly JS5 rollback drills: stage a
release, run the verification helper, simulate a rollback (deprecate, rebuild),
and verify again before unblocking npm.

### Support playbook for incidents (JS5 guardrail)

See also {doc}`support_playbook`, which captures the SLA matrix, ownership
model, and governance evidence expectations. The summary below focuses on the
hands-on rollback/verification commands that accompany each drill or incident.

Trigger this flow whenever npm telemetry, CI, or support tickets flag a bad
package. Track the incident/ticket id throughout the steps below so the audit
trail stays searchable. Start each rollback drill with the evidence helper so
`summary.json` and the copied verification artefacts can be attached to
governance reviews:

```bash
scripts/js_release_rollback.sh \
  --incident INC-1234 \
  --bad-version 0.0.2 \
  --known-good 0.0.1 \
  --replacement-version 0.0.3 \
  --registry https://registry.npmjs.org/
```

The helper stores logs under `artifacts/js/incidents/<incident>/logs/`, copies
verification directories into `artifacts/js/incidents/<incident>/evidence/`,
and writes `summary.json` describing the captured artefacts plus the recommended
dist-tag/deprecate commands. Pass `--dry-run` to preview the steps, `--skip-staging`
when a replacement build already exists, and `--verify-replacement` once the
new version is published to the registry so the helper captures the follow-up
checks in the same incident folder.

1. Freeze distribution immediately:

   ```bash
   cd javascript/iroha_js
   npm dist-tag add @iroha/iroha-js@<known-good> latest
   npm deprecate @iroha/iroha-js@<bad-version> "INC-1234 rollback to <known-good>"
   ```

2. Capture evidence:
   - Run `npm run release:verify` against the suspect version (plus any staged or
     prior artefacts) and copy the generated
     `artifacts/js/verification/v<version>_*` directory into
     `artifacts/js/incidents/INC-1234/`.
   - If the rollback requires a rebuild, execute `scripts/js_signed_staging.sh`
     for the replacement version and store the staging summary in the same
     incident folder.

3. Recover and announce:
   - Promote the replacement tarball after verification succeeds:

     ```bash
     npm dist-tag add @iroha/iroha-js@<replacement-version> latest
     npm deprecate @iroha/iroha-js@<bad-version> "INC-1234 replaced by <replacement-version>"
     ```

   - Send the incident summary (what failed, hashes observed, verification
     folder path) to the SDK support channel and add a short line to `status.md`
     so the JS5 guardrail log stays current.

## Versioning & Tagging

1. Bump the version in `javascript/iroha_js/package.json` following semver.
2. Add a matching git tag prefixed with `js-v` (preferred) or `v`, for example:

   ```bash
   git tag js-v0.2.0
   ```

3. Push the tag and publish the release notes only after the explicit package
   publish and verification steps below succeed. In the current repository,
   pushing the tag does not promote the npm package automatically.

## Publishing

After every prerequisite and evidence gate succeeds, publish explicitly:

```bash
cd javascript/iroha_js
NPM_CONFIG_PROVENANCE=true npm run release:publish
```

Notes:

- The publish command signs the tarball using Sigstore when provenance is
  enabled and automatically stores the provenance bundle next to the tarball.
- Use `npm publish --dry-run` to inspect the generated manifest before pushing.
- The registry tarball intentionally contains no platform-specific `.node`
  binary, Cargo workspace, install hook, or implicit downloader. Registry
  consumers cannot build the native host from a clean package installation.
  Native-only applications must provision a separately built, checksum-verified
  binary/manifest pair through `IROHA_JS_NATIVE_DIR`; portable browser exports
  do not require that host.

## Post-Publish Checklist

1. If you published manually, sync release notes across the changelog, status,
   and roadmap files:

   ```bash
   npm run release:update-docs -- --version 0.1.0 --date 2026-02-01 \
     --note "Highlight the key changes for this release"
   ```

   Omit `--date` to default to today, and provide additional `--note` flags or
   `--notes-file <path>` entries as needed.

2. Record the published tag, version, command transcript, and registry
   verification result in the release evidence.
3. Archive the latest `artifacts/js-sdk-provenance/v<version>_<timestamp>/`
   directory alongside the signed SBOM and publish logs so auditors can refer
   to the deterministic metadata.

## Future Automation

- Generate release candidates via GitHub Actions using matrix builds for
  Node 18/20 with Arm64 and x64.
