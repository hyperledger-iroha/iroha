# SoraFS CLI & SDK — Release Notes (v0.1.0)

## Highlights
- `sorafs_cli` covers deterministic content packaging (`car pack`, `manifest
  build`, `proof verify`) while aggregate release authentication is isolated in
  the governed raw-Ed25519/HSM release helper.
- The multi-source fetch *scoreboard* ships as part of `sorafs_car`: it normalises
  provider telemetry, enforces capability penalties, persists JSON/Norito reports, and
  feeds the orchestrator simulator (`sorafs_fetch`) through the shared registry handle.
  Fixtures under `fixtures/sorafs_manifest/ci_sample/` demonstrate the deterministic
  inputs and outputs that CI/CD is expected to diff against.
- Release automation is codified in `ci/check_sorafs_cli_release.sh` and
  `scripts/release_sorafs_cli.sh`. Every release archives the aggregate release
  manifest, raw signature/public key, reviewed signer fingerprint, pinned native
  verifier digest, verification receipt, and scoreboard snapshot.

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
3. Authenticate the canonical aggregate release manifest on the protected
   signer:
   ```bash
   scripts/release_sorafs_cli.sh \
     --manifest artifacts/release/release_manifest.json \
     --external-signer /run/sorafs-release/ed25519-sign \
     --signing-public-key /run/sorafs-release/release.ed25519.pub \
     --trusted-signing-fingerprint "$REVIEWED_SIGNER_SHA256" \
     --release-manifest-verifier /opt/iroha/bin/sorafs-validate \
     --trusted-release-manifest-verifier-sha256 "$REVIEWED_VERIFIER_SHA256"
   ```
   Regenerate content fixtures separately when the release changes canonical
   vectors; fixture signatures are never production evidence.

## Verification
- Release gate commit: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` immediately after the gate succeeded).
- `ci/check_sorafs_cli_release.sh` output: archived in
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (attached to the release bundle).
- Aggregate release manifest digest: `<SHA256>` (`release_manifest.json`).
- Raw Ed25519 signature/public-key digests and reviewed signer fingerprint:
  `<SHA256 values>`.
- Native verifier digest and successful verification receipt: `<SHA256 and
  artifact path>`.
- Proof summary digest: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).

## Notes for Operators
- Every HTTP gateway provider descriptor now requires
  `gateway-key=<32-byte Ed25519 public key hex>`. Distribute that pin through
  authenticated provider inventory and rotate it together with a newly issued
  token; never trust a key learned only from the token response.
- Stream-token signatures are domain separated over
  `b"sorafs.stream-token.signature.v1\0" || canonical_norito_body`. Legacy
  body-only signatures are rejected. Gateway client origins must use public
  HTTPS port 443 at `/`; redirects, local/private/reserved addresses,
  credentials, queries, fragments, and non-root paths are rejected.
- The Torii gateway now enforces the `X-Sora-Chunk-Range` capability header. Update
  allowlists so clients presenting the new stream token scopes are admitted; older tokens
  without the range claim will be throttled.
- `scripts/sorafs_gateway_self_cert.sh` integrates manifest verification. When running
  the self-cert harness, supply the aggregate release manifest, raw
  signature/public key, reviewed signer fingerprint, and pinned native verifier
  tuple so the wrapper fails before the harness on authenticity drift.
- Telemetry dashboards should ingest the new scoreboard export (`scoreboard.json`) to
  reconcile provider eligibility, weight assignments, and refusal reasons.
- Archive `release_manifest.json`, `release_manifest.ed25519.sig`,
  `release_manifest.ed25519.pub`, and `release_manifest.verify.json` with every
  rollout, together with the reviewed fingerprints/digests.

## Rollback / Yank Record

- Last verified rollback release: `<sorafs-cli-vX.Y.Z>`; archive SHA-256:
  `<lowercase-hex>`; provenance/signature verification receipt: `<ticket-path>`.
- Deployment rollback: `<not_invoked | completed | failed>`; regional gateway
  results and clean-consumer smoke hashes: `<ticket-path>`.
- Package disposition: record one `<withdrawn | not_published | failed>` result
  and registry receipt for every package row in `release/version-map.toml`.
- Signing-key disposition: `<unchanged | rotated | revoked>`; reviewed
  fingerprint and receipt: `<ticket-path>`.
- Incident/release decision: `<ticket-id>`; UTC timestamp: `<timestamp>`;
  release operator and governance approvers: `<identities>`.

Follow the
[`release_rollback_yank.md`](../../specs/sorafs/runbooks/release_rollback_yank.md)
runbook. Never delete or rewrite the affected signed tag, archive, checksums,
SBOM, provenance, or signatures, and never reuse a withdrawn version.

## Acknowledgements
- Storage Team — end-to-end CLI consolidation, chunk-plan renderer, and scoreboard
  telemetry plumbing.
- Tooling WG — release pipeline (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) and deterministic fixture bundle.
- Gateway Operations — capability gating, stream-token policy review, and updated
  self-certification playbooks.
