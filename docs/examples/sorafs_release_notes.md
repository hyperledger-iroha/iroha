# SoraFS CLI & SDK — Release Notes (v0.1.0)

## Highlights
- `sorafs_cli` now wraps the entire packaging pipeline (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) so CI runners invoke a
  single binary instead of bespoke helpers. The new keyless signing flow defaults to
  `SIGSTORE_ID_TOKEN`, understands GitHub Actions OIDC providers, and emits deterministic
  summary JSON alongside the signature bundle.
- The multi-source fetch *scoreboard* ships as part of `sorafs_car`: it normalises
  provider telemetry, enforces capability penalties, persists JSON/Norito reports, and
  feeds the orchestrator simulator (`sorafs_fetch`) through the shared registry handle.
  Fixtures under `fixtures/sorafs_manifest/ci_sample/` demonstrate the deterministic
  inputs and outputs that CI/CD is expected to diff against.
- Release automation is codified in `ci/check_sorafs_cli_release.sh` and
  `scripts/release_sorafs_cli.sh`. Every release now archives the manifest bundle,
  signature, `manifest.sign/verify` summaries, and the scoreboard snapshot so governance
  reviewers can trace artefacts without re-running the pipeline.

## Upgrade Steps
1. Update the aligned crates in your workspace:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Re-run the release gate locally (or in CI) to confirm fmt/clippy/test coverage:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Regenerate signed artefacts and summaries with the curated config:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Copy refreshed bundles/proofs into `fixtures/sorafs_manifest/ci_sample/` if the
   release updates canonical fixtures.

## Verification
- Release gate commit: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` immediately after the gate succeeded).
- `ci/check_sorafs_cli_release.sh` output: archived in
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (attached to the release bundle).
- Manifest bundle digest: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Proof summary digest: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Manifest digest (for downstream attestation cross-checks):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (from `manifest.sign.summary.json`).

## Notes for Operators
- The Torii gateway now enforces the `X-Sora-Chunk-Range` capability header. Update
  allowlists so clients presenting the new stream token scopes are admitted; older tokens
  without the range claim will be throttled.
- `scripts/sorafs_gateway_self_cert.sh` integrates manifest verification. When running
  the self-cert harness, supply the freshly generated manifest bundle so the wrapper can
  fail fast on signature drift.
- Telemetry dashboards should ingest the new scoreboard export (`scoreboard.json`) to
  reconcile provider eligibility, weight assignments, and refusal reasons.
- Archive the four canonical summaries with every rollout:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Governance tickets reference these exact files during
  approval.

## Acknowledgements
- Storage Team — end-to-end CLI consolidation, chunk-plan renderer, and scoreboard
  telemetry plumbing.
- Tooling WG — release pipeline (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) and deterministic fixture bundle.
- Gateway Operations — capability gating, stream-token policy review, and updated
  self-certification playbooks.
