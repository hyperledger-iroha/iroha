---
title: Release Process
summary: Run the CLI/SDK release gate, apply the shared versioning policy, and publish canonical release notes.
---

# Release Process

SoraFS binaries (`sorafs_cli`, `sorafs_fetch`, helpers) and SDK crates
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) ship together. The release
pipeline keeps the CLI and libraries aligned, ensures lint/test coverage, and
captures artefacts for downstream consumers. Run the checklist below for every
candidate tag.

## 0. Confirm security review sign-off

Before executing the technical release gate, capture the latest security review
artefacts:

- Download the most recent SF-6 security review memo ([reports/sf6-security-review](./reports/sf6-security-review.md))
  and record its SHA256 hash in the release ticket.
- Attach the remediation ticket link (e.g., `governance/tickets/SF6-SR-2026.md`) and note the sign-off
  approvers from Security Engineering and the Tooling Working Group.
- Verify that the remediation checklist in the memo is closed; unresolved items block the release.
- Prepare to upload parity harness logs (`cargo test -p sorafs_orchestrator --test sorafs_cli proof_stream_consumes_ndjson_and_reports_metrics -- --nocapture`)
  alongside the aggregate release-manifest verification receipt.
- Confirm the protected signing job has an external Ed25519/HSM adapter, the
  governed raw public key and reviewed fingerprint, and the exact
  `sorafs-validate` path plus reviewed SHA256.

Include these artefacts when notifying governance and publishing the release.

## 1. Execute the release/test gate

The `ci/check_sorafs_cli_release.sh` helper runs formatting, Clippy, and tests
across the CLI and SDK crates with a workspace-local target directory (`.target`)
to avoid permission conflicts when executing inside CI containers.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

The script performs the following assertions:

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked -p sorafs_orchestrator --all-targets` for `sorafs_cli`, plus `cargo clippy --locked -p sorafs_car --features cli --all-targets`, `sorafs_manifest`, and `sorafs_chunker`
- `cargo test --locked -p sorafs_orchestrator --test sorafs_cli`, plus `cargo test --locked -p sorafs_car --features cli --all-targets`, `sorafs_manifest`, and `sorafs_chunker`

If any step fails, fix the regression before tagging. Release builds must be
continuous with main; do not cherry-pick fixes into release branches. The gate
also exercises the raw-Ed25519 release helper and rejects missing fingerprints,
unpinned native verifiers, malformed keys/signatures, and unsafe paths.

## 2. Apply the versioning policy

All SoraFS CLI/SDK crates use SemVer:

- `MAJOR`: Introduced for the first 1.0 release. Before 1.0 the `0.y` minor bump
  **indicates breaking changes** in the CLI surface or Norito schemas.
  fields gated behind optional policy, telemetry additions).
- `PATCH`: Bug fixes, documentation-only releases, and dependency updates that
  do not change observable behaviour.

Always keep `sorafs_car`, `sorafs_manifest`, and `sorafs_chunker` on the same
version so downstream SDK consumers can depend on a single aligned version
string. When bumping versions:

1. Update `version =` fields in each crate’s `Cargo.toml`.
2. Regenerate the `Cargo.lock` via `cargo update -p <crate>@<new-version>` (the
   workspace enforces explicit versions).
3. Run the release gate again to ensure no stale artefacts remain.

## 3. Prepare release notes

Every release must publish a markdown changelog that highlights CLI, SDK, and
governance-impacting changes. Use the template in
`fixtures/documentation/sorafs_release_notes.md` (copy it to your release artifacts
directory and fill in the sections with concrete details).

Minimum content:

- **Highlights**: feature headlines and compatibility requirements for CLI and
  SDK consumers.
- **Upgrade steps**: TL;DR commands for bumping cargo dependencies and rerunning
  deterministic fixtures.
- **Verification**: command output hashes or envelopes and the exact
  `ci/check_sorafs_cli_release.sh` revision executed.
- **Rollback / Yank Record**: the last verified rollback release and one package
  disposition for every row in `release/version-map.toml`.

Attach the filled release notes to the tag (e.g., GitHub release body) and store
them alongside deterministically generated artefacts.

## 4. Execute release hooks

Run `scripts/release_sorafs_cli.sh` on the canonical aggregate release manifest.
The wrapper invokes the reviewed external signer and immediately verifies the
raw 64-byte signature and raw 32-byte public key through a SHA256-pinned
`sorafs-validate release-manifest` binary:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/release/release_manifest.json \
  --external-signer /run/sorafs-release/ed25519-sign \
  --signing-public-key /run/sorafs-release/release.ed25519.pub \
  --trusted-signing-fingerprint "$REVIEWED_SIGNER_SHA256" \
  --release-manifest-verifier /opt/iroha/bin/sorafs-validate \
  --trusted-release-manifest-verifier-sha256 "$REVIEWED_VERIFIER_SHA256"
```

Tips:

- Keep the canonical release manifest and public verification artifacts in the
  evidence packet. Private keys, HSM credentials, and signer sessions remain
  runtime-only.
- Base CI automation on `.github/workflows/sorafs-cli-release.yml`; it runs the
  release gate and deterministic candidate packaging, then publishes the
  run-bound unsigned foundational manifest. Download and sign those exact bytes
  outside GitHub with the governed Ed25519 HSM. Provision only the raw signature,
  raw public key, reviewed signer fingerprint, pinned native verifier path, and
  reviewed verifier SHA256 on the protected `sorafs-release-auth` runner; approve
  the `sorafs-release-authentication` environment only after that handoff. The
  workflow verifies and archives the public tuple and receipt before provenance
  or the promoted artifact can run. No private key or HSM signing operation
  enters GitHub Actions.

The protected environment must define
`SORAFS_RELEASE_SIGNATURE_PATH`, `SORAFS_RELEASE_PUBLIC_KEY_PATH`,
`SORAFS_RELEASE_MANIFEST_VERIFIER_PATH`,
`SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT`, and
`SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256` as environment variables.
The three paths must be absolute, outside `GITHUB_WORKSPACE`, and pre-provisioned
on the protected runner. The signature is exactly 64 raw bytes and the public key
is exactly 32 raw bytes; both trust anchors are reviewed lowercase SHA-256.
- OIDC/cosign attestations are provenance evidence. They do not replace the
  governed Ed25519 release-manifest signature.
- When the release updates canonical fixtures, copy the refreshed manifest,
  chunk plan, and summaries into `fixtures/sorafs_manifest/ci_sample/` (and update
  `fixtures/documentation/sorafs_ci_sample/manifest.template.json`) before tagging.
  Downstream operators use those content-only fixtures for deterministic
  preparation tests; they are not release-authenticity evidence.
- Capture the run log for `sorafs_cli proof stream` bounded-channel verification and attach it to the
  release packet to demonstrate proof streaming safeguards remain active.

Use `scripts/sorafs_gateway_self_cert.sh` when the release also carries a
gateway rollout. Its config must provide the same release manifest, raw
signature, raw public key, trusted signer fingerprint, and pinned native
verifier tuple:

```bash
scripts/sorafs_gateway_self_cert.sh \
  --config /run/sorafs-release/gateway-self-cert.conf
```

## 5. Tag and publish

After the checks pass and hooks complete:

1. Run `sorafs_cli --version` and `sorafs_fetch --version` to confirm binaries
   report the new version.
2. Prepare the non-secret release configuration in the deployment repository.
   Supply signer/verifier paths and reviewed digests explicitly at runtime;
   never commit signer credentials or private keys.
3. Create a signed tag (preferred) or annotated tag. The release workflow
   accepts only the exact `sorafs-cli-v<release/version-map.toml version>`
   spelling:
   ```bash
   git tag -s sorafs-cli-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-cli-vX.Y.Z
   ```
4. Confirm the workflow produced all five native candidate archives:
   Linux x86_64/aarch64, macOS x86_64/aarch64, and the additional Windows
   x86_64 archive. Each job must rebuild the metadata-normalized archive,
   compare it byte-for-byte, extract it into a clean directory, and run all
   three packaged binaries there before checksums, provenance, and signing.
5. Upload artefacts (CAR bundles, manifests, proof summaries, release notes,
   attestation outputs) to the project registry following the governance
   checklist in [deployment guide](./developer-deployment.md). If the release
   minted new fixtures, push them to the shared fixture repo or object store so
   audit automation can diff the published bundle against source control.
6. Notify the governance channel with links to the signed tag, release notes,
   release-manifest/signature/public-key hashes, the native verification receipt,
   and any attestation envelopes. Include the CI job URL (or log archive) that
   ran `ci/check_sorafs_cli_release.sh` and `scripts/release_sorafs_cli.sh`. Update
   the governance ticket so auditors can trace approvals to artefacts; when the
   `.github/workflows/sorafs-cli-release.yml` job posts notifications, link the
   recorded hash outputs rather than pasting ad-hoc summaries.

## 6. Rollback and package withdrawal

Before promotion, select and verify the previous known-good signed release and
complete the release-notes rollback/yank record. If a candidate is stopped or a
published release is withdrawn, follow
[SoraFS Release Rollback and Yank](./release-rollback-yank.md). Preserve the
affected tag and evidence, restore only a checksum/provenance/signature-verified
archive, and record a registry-confirmed result for every Cargo, npm, Python,
C#/NuGet, JVM/Android, Swift, and GitHub artifact channel in scope.

## 7. Post-release follow-up

- Ensure documentation pointing at the new version (quickstarts, CI templates)
  is updated or confirm no changes are required.
- Record separately authorized post-V1 work outside the closed V1 release
  ledger; do not reopen compatibility branches or migration paths.
- Archive the release gate output logs for auditors—store them beside the signed
  artefacts.

Following this pipeline keeps the CLI, SDK crates, and governance collateral in
lock-step for each release cycle.
