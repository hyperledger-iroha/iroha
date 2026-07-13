OpenAPI signing
---------------

- Release Torii OpenAPI specs (`torii.json`) must be signed, and the manifest is verified by `cargo xtask openapi-verify`.
- First-release development snapshots may carry an unsigned manifest only when CI passes `--allow-unsigned`; this still verifies the artifact path, size, SHA-256, and BLAKE3 digest, but skips the Ed25519 signature check.
- Generator provenance is explicit. Clean manifests record `generator_commit` as the exact lowercase 40-hex Git SHA-1, set `generator_dirty` to `false`, and omit `generator_source_sha256_hex`. A dirty unsigned development snapshot instead sets `generator_commit` to `null`, sets `generator_dirty` to `true`, and records a deterministic SHA-256 digest of all non-generated tracked and untracked source state. Canonical OpenAPI outputs and manifests are excluded from that source digest so identical source produces the same digest across regenerations.
- Dirty provenance is development-only: the generator refuses to sign it, and release verification rejects it even if a signature is injected into the manifest. Commit or clean the source changes and regenerate before signing a release artifact.
- Allowed signer keys live in `allowed_signers.json`; rotate this file whenever the signing key changes. Keep the `version` field at `1`.
- CI (`ci/check_openapi_spec.sh`) already enforces the allowlist for both the latest and current specs. If another portal or pipeline consumes the signed spec, point its verification step at the same allowlist file to avoid drift.
- The canonical `xtask` dependency compiles Torii's supported documentation profile (`app_api`, `telemetry`, `profiling`, `schema`, `p2p_ws`, `connect`, `gov_vrf`, `zk-verify-batch`, and `push`). The generated operation set is the exact enabled `RouteCatalog` OpenAPI projection; do not hand-add disabled or uncataloged paths to a snapshot.
- To re-sign after a key rotation:
  1. Update `allowed_signers.json` with the new public key.
  2. Regenerate/sign the spec: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Re-run `ci/check_openapi_spec.sh` (or `cargo xtask openapi-verify` manually) to confirm the manifest matches the allowlist.
- If the signing key is held outside the checkout, generate the spec first, have the operator sign the exact bytes of `docs/portal/static/openapi/torii.json`, then refresh the manifest with `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --signature-envelope <signature-envelope-json>`. The detached envelope must be JSON shaped as `{"algorithm":"ed25519","public_key_hex":"...","signature_hex":"..."}` and is verified before `manifest.json` is written.
- For unsigned first-release snapshots, use `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --unsigned-manifest`, then run `npm run sync-openapi -- --allow-unsigned --latest` from `docs/portal`.
