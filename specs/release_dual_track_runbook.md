# Dual-Track Release Procedure

This is the repository-local procedure for producing the current Iroha release
artifacts. It covers the implemented `iroha2=single` and `iroha3=nexus`
packaging profiles. It does not prescribe a calendar, branch naming scheme,
upload destination, or approval hierarchy that is not encoded in this
repository.

Public installation and operator documentation belongs in the sibling
`iroha-docs` repository. This file stays here because it is coupled directly to
the release scripts, profile manifest, and workflows.

## Automated boundary

The checked-in automation has distinct responsibilities:

| Surface | Current responsibility |
| --- | --- |
| `.github/workflows/workspace_release.yml` | Exact-SHA workspace release gate. It runs nightly, on `main`, on `v*` tags, by manual dispatch, and as a reusable workflow. Its jobs check formatting, build the workspace, build Rustdoc, run the workspace tests and compile-unit guard, collect coverage, and run strict Clippy. |
| `scripts/run_release_pipeline.py` | Local dual-profile coordinator. It builds from reviewed prebuilt binaries, produces matrices and checksums, creates the aggregate manifest, optionally signs it through an external signer, and creates and validates a publication plan. |
| `.github/workflows/publish.yml` | Existing `v2*` container publication workflow. It is not the generic dual-track coordinator. |
| `.github/workflows/sorafs-cli-release.yml` | Separate SoraFS CLI/reference-validator release workflow for `sorafs-cli-v*`. Its evidence does not certify the generic Iroha bundles. |
| `.github/workflows/mobile_sdk_artifacts.yml` | Separate mobile SDK artifact workflow. |

No checked-in workflow invokes `scripts/run_release_pipeline.py`. A local run
therefore stages release artifacts and a publication plan; it does not prove
that anything was uploaded, promoted, or released.

## 1. Fix the candidate identity

Choose one reviewed source commit, release version, and
`SOURCE_DATE_EPOCH`. Use a fresh checkout and a fresh output directory. The
coordinator rejects:

- a short or malformed source commit
- a source commit that differs from `git rev-parse HEAD`
- a version that differs from the version declared by the workspace
- an output directory for that version that already exists

Record the full source commit and epoch with the release evidence. Do not reuse
an output directory from an earlier attempt.

Before building artifacts, obtain a successful exact-SHA run of
`.github/workflows/workspace_release.yml` for the candidate. The workflow is
the source-backed full-workspace gate; a green pull-request subset is not a
substitute.

## 2. Resolve the packaging profiles

`release/network_profiles.toml` is the source of truth for network-to-artifact
selection. The current mapping sends Sora Nexus aliases and chain ID
`sora:nexus:global` to `iroha3`; unmatched networks use the `iroha2` default.

Review the committed mapping and resolve the intended network:

```bash
python3 scripts/select_release_profile.py --list
python3 scripts/select_release_profile.py --network sora-nexus
python3 scripts/select_release_profile.py --network <network-alias>
```

The coordinator defaults to:

```text
iroha2=single
iroha3=nexus
```

Use repeated `--profile name=config` arguments only when the reviewed profile
manifest and release scope require an explicit override.

## 3. Prepare reviewed build inputs

Run:

```bash
python3 scripts/run_release_pipeline.py --help
scripts/build_release_bundle.sh --help
scripts/build_release_image.sh --help
```

The coordinator requires the following input groups for an ordinary
dual-profile build:

- the full source commit, canonical epoch, release version, and a fresh output
  root
- an absolute, reviewed `git-cliff` executable and its SHA-256
- reviewed prebuilt binaries for every profile and mandatory
  Linux/macOS/Windows bundle target
- an absolute, reviewed `zstd` executable and its SHA-256
- reviewed prebuilt Linux binaries for both `linux/amd64` and `linux/arm64`
  image platforms
- digest-pinned builder and runtime images
- exact reviewed Docker and Buildx executables, their SHA-256 values, the
  reviewed Buildx version string, and a bounded reviewed builder instance
- the required local evidence inputs, including a CBDC rollout directory
  unless the corresponding check is explicitly skipped

Skip flags are useful for bounded development runs. A summary that records a
skipped production gate is not promotion evidence.

The direct bundle and image builders are deterministic unsigned artifact
producers. Builders do not invoke signers, accept private-key material, or emit
per-artifact signature keys. The coordinator signs only the final closed
aggregate manifest.

## 4. Run the coordinator

Supply every reviewed input required by `--help`. This command outline shows
the identity, profile, signing, and publication controls; the target and tool
matrix arguments are intentionally represented by named placeholders so they
cannot be mistaken for reviewed values:

```bash
python3 scripts/run_release_pipeline.py \
  --version <X.Y.Z> \
  --source-commit <FULL_COMMIT> \
  --source-date-epoch <EPOCH> \
  --output-dir artifacts/releases \
  --profile iroha2=single \
  --profile iroha3=nexus \
  --git-cliff <ABSOLUTE_REVIEWED_GIT_CLIFF> \
  --trusted-git-cliff-sha256 <SHA256> \
  <REVIEWED_BUNDLE_AND_IMAGE_MATRIX_ARGUMENTS> \
  <REQUIRED_EVIDENCE_ARGUMENTS> \
  --external-signer <ABSOLUTE_AUTHENTICATED_EXTERNAL_SOFTWARE_SIGNER_ADAPTER> \
  --signing-public-key <RAW_ED25519_PUBLIC_KEY_FILE> \
  --trusted-signing-fingerprint <REVIEWED_SHA256> \
  --release-manifest-verifier <ABSOLUTE_REVIEWED_SORAFS_VALIDATE> \
  --trusted-release-manifest-verifier-sha256 <REVIEWED_SHA256> \
  --publish-target iroha2=<REVIEWED_TARGET_URI> \
  --publish-target iroha3=<REVIEWED_TARGET_URI>
```

The signer adapter authenticates to the isolated software-signing service at
runtime. V1 fixes `signing_provider=authenticated_external_signer` and
`signing_backend=software`; a verified release reports
`signer_qualification=software-key-qualified`. Never place private keys, PINs,
bearer tokens, or forwarded authentication headers in repository files,
command arguments, or release artifacts. A future HSM adapter requires new
HSM-backed evidence and does not change this release's qualification.

The output is created at `artifacts/releases/<X.Y.Z>/` unless another output
root is supplied. Review at least:

- `artifacts/` with each bundle, OCI archive, checksum sidecar, per-build
  manifest, `SHA256SUMS`, and per-target `dual_profile_matrix-*.json`
- `release_manifest.json`
- `release_manifest.json.sig`
- `release_manifest.json.pub`
- `publish_plan.json`, `publish_plan.sh`, and `publish_plan.txt` when targets
  were supplied
- `publish_plan_report.json` and `publish_plan_report.txt`
- `SUMMARY.txt`

## 5. Verify the closed artifact set

The pipeline runs `ci/dual_profile_matrix.sh` for every mandatory bundle
target. For a direct or diagnostic bundle build, run:

```bash
ci/dual_profile_smoke.sh <iroha2-bundle> <iroha3-bundle>
ci/dual_profile_matrix.sh \
  --output <dual_profile_matrix.json> \
  --expect-version <X.Y.Z> \
  <iroha2-bundle> <iroha3-bundle>
```

The matrix verifies the bundle layout, executable inventory, profile metadata,
version, checksums, and basic `irohad --version`/`kagami --help` execution.
Treat each target-specific matrix as part of the aggregate inventory.

Follow the artifact-to-profile verification procedure in
[`release_artifact_selection.md`](release_artifact_selection.md) before
unpacking or loading a candidate.

## 6. Authenticate the aggregate manifest

Production authentication uses one external Ed25519 signature over the final
canonical `release_manifest.json`:

- `release_manifest.json.sig` contains exactly 64 canonical raw Ed25519 bytes
- `release_manifest.json.pub` contains exactly 32 raw Ed25519 public-key bytes
- `--trusted-signing-fingerprint` comes from an authenticated review channel,
  not from the downloaded artifact set
- `--release-manifest-verifier` identifies the reviewed
  `sorafs-validate` executable
- `--trusted-release-manifest-verifier-sha256` independently pins that exact
  executable

Verify the tuple with `scripts/release_manifest_signing.py verify`; the native
contract invoked is `sorafs-validate release-manifest`. Then verify every
candidate against both its checksum sidecar and the authenticated aggregate
manifest as described in `release_artifact_selection.md`.

The release pipeline accepts the signer contract through:

```text
--external-signer
--signing-public-key
--trusted-signing-fingerprint
--release-manifest-verifier
--trusted-release-manifest-verifier-sha256
```

There is no private-key or in-process signing fallback.

## 7. Generate and replay the publication plan

When `--publish-target` is supplied, the coordinator generates the plan and
immediately reconstructs it from the manifest, artifact directory, target map,
signature tuple, fingerprint, and verifier pins. A production plan is valid
only when `publish_plan_report.json` has status `ok`.

For independent replay, call `scripts/publish_plan.py validate` with the plan
and the same independent inputs. The validation command requires
`--manifest-signature`, the public key, `--trusted-signing-fingerprint`,
`--release-manifest-verifier`, and
`--trusted-release-manifest-verifier-sha256`. Use `--previous-plan` to make a
target or layout change visible, and use the remote probe options only after
the candidate has actually been staged.

`--development-allow-unsigned-manifest` and the coordinator's corresponding
development-only option may be used for tests. They are never valid for
promotion.

The plan is an authenticated upload inventory, not an uploader. Publication
requires separately authorized infrastructure and must produce independent
registry, bucket, gateway, or package-repository receipts.

## 8. Close the release record

Keep the following evidence together for the exact candidate:

- full source commit, release version, and canonical epoch
- successful exact-SHA workspace release workflow run
- all target-specific profile matrices and builder manifests
- aggregate manifest, signature, raw public key, authenticated fingerprint,
  and pinned verifier identity
- validated publication plan and report
- hosted SBOM and vulnerability scan
- OIDC/cosign provenance bundle and verification receipt, when produced by the
  authorized hosted release environment
- actual publication receipts
- rollback or yank rehearsal/record for the publication surface

The generic local pipeline does not create hosted SBOM, vulnerability,
OIDC/cosign, upload, promotion, or rollback evidence. Do not mark the release
published from `SUMMARY.txt` alone.

For an LTS designation, complete the additional selection and backport policy
in [`lts_selection.org`](lts_selection.org) after this procedure.
