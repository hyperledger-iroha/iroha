---
lang: mn
direction: ltr
source: docs/source/sorafs/developer/releases.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 234a62563af48e9eab372bfb34962a9049bb1f144438841aec316611f44ca311
source_last_modified: "2026-01-05T09:28:12.075835+00:00"
translation_last_reviewed: 2026-02-07
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

- Download the most recent SF-6 security review memo (`docs/source/sorafs/reports/sf6_security_review.md`)
  and record its SHA256 hash in the release ticket.
- Attach the remediation ticket link (e.g., `governance/tickets/SF6-SR-2026.md`) and note the sign-off
  approvers from Security Engineering and the Tooling Working Group.
- Verify that the remediation checklist in the memo is closed; unresolved items block the release.
- Prepare to upload parity harness logs (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  alongside the manifest bundle.
- Confirm the signing command you plan to run includes both `--identity-token-provider` and an explicit
  `--identity-token-audience=<aud>` so Fulcio scope is captured in the release evidence.

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
- `cargo clippy --locked --all-targets` for `sorafs_car` (with the `cli` feature),
  `sorafs_manifest`, and `sorafs_chunker`
- `cargo test --locked --all-targets` for those same crates

If any step fails, fix the regression before tagging. Release builds must be
continuous with main; do not cherry-pick fixes into release branches. The gate
also checks that keyless signing flags (`--identity-token-issuer`, `--identity-token-audience`)
are provided where applicable; missing arguments fail the run.

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
`docs/examples/sorafs_release_notes.md` (copy it to your release artifacts
directory and fill in the sections with concrete details).

Minimum content:

- **Highlights**: feature headlines for CLI and SDK consumers.
  requirements.
- **Upgrade steps**: TL;DR commands for bumping cargo dependencies and rerunning
  deterministic fixtures.
- **Verification**: command output hashes or envelopes and the exact
  `ci/check_sorafs_cli_release.sh` revision executed.

Attach the filled release notes to the tag (e.g., GitHub release body) and store
them alongside deterministically generated artefacts.

## 4. Execute release hooks

Run `scripts/release_sorafs_cli.sh` to generate the signature bundle and
verification summary that ship with every release. The wrapper builds the CLI
when necessary, calls `sorafs_cli manifest sign`, and immediately replays
`manifest verify-signature` so failures surface before tagging. Example:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Tips:

- Track release inputs (payload, plans, summaries, expected token hash) in your
  repo or deployment config so the script remains reproducible. The CI fixture
  bundle under `fixtures/sorafs_manifest/ci_sample/` shows the canonical layout.
- Base CI automation on `.github/workflows/sorafs-cli-release.yml`; it runs the
  release gate, invokes the script above, and archives bundles/signatures as
  workflow artefacts. Mirror the same command order (release gate → sign →
  verify) in other CI systems so audit logs line up with the generated hashes.
- Keep the generated `manifest.bundle.json`, `manifest.sig`,
  `manifest.sign.summary.json`, and `manifest.verify.summary.json` together—they
  form the packet referenced in the governance notification.
- When the release updates canonical fixtures, copy the refreshed manifest,
  chunk plan, and summaries into `fixtures/sorafs_manifest/ci_sample/` (and update
  `docs/examples/sorafs_ci_sample/manifest.template.json`) before tagging.
  Downstream operators depend on the committed fixtures to reproduce the release
  bundle.
- Capture the run log for `sorafs_cli proof stream` bounded-channel verification and attach it to the
  release packet to demonstrate proof streaming safeguards remain active.
- Record the exact `--identity-token-audience` used during signing in the release notes; governance
  cross-checks the audience against Fulcio policy before approving publication.

Use `scripts/sorafs_gateway_self_cert.sh` when the release also carries a
gateway rollout. Point it at the same manifest bundle to prove the attestation
matches the candidate artefact:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag and publish

After the checks pass and hooks complete:

1. Run `sorafs_cli --version` and `sorafs_fetch --version` to confirm binaries
   report the new version.
2. Prepare the release configuration in a checked-in `sorafs_release.toml`
   (preferred) or another config file tracked by your deployment repo. Avoid
   relying on ad-hoc environment variables; pass paths to the CLI with
   `--config` (or equivalent) so the release inputs are explicit and
   reproducible.
3. Create a signed tag (preferred) or annotated tag:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Upload artefacts (CAR bundles, manifests, proof summaries, release notes,
   attestation outputs) to the project registry following the governance
   checklist in `docs/source/sorafs/developer/deployment.md`. If the release
   minted new fixtures, push them to the shared fixture repo or object store so
   audit automation can diff the published bundle against source control.
5. Notify the governance channel with links to the signed tag, release notes,
   manifest bundle/signature hashes, the archived `manifest.sign/verify` summaries,
   and any attestation envelopes. Include the CI job URL (or log archive) that
   ran `ci/check_sorafs_cli_release.sh` and `scripts/release_sorafs_cli.sh`. Update
   the governance ticket so auditors can trace approvals to artefacts; when the
   `.github/workflows/sorafs-cli-release.yml` job posts notifications, link the
   recorded hash outputs rather than pasting ad-hoc summaries.

## 6. Post-release follow-up

- Ensure documentation pointing at the new version (quickstarts, CI templates)
  is updated or confirm no changes are required.
- File roadmap entries if follow-on work (e.g., migration flags, deprecation of
- Archive the release gate output logs for auditors—store them beside the signed
  artefacts.

Following this pipeline keeps the CLI, SDK crates, and governance collateral in
lock-step for each release cycle.
