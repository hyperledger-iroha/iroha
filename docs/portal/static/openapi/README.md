OpenAPI signing
---------------

- Release Torii OpenAPI specs (`torii.json`) must be signed, and the manifest is verified by `cargo xtask openapi-verify`.
- First-release development snapshots may carry an unsigned manifest only when CI passes `--allow-unsigned`; this still verifies the artifact path, size, SHA-256, and BLAKE3 digest, but skips the Ed25519 signature check.
- Allowed signer keys live in `allowed_signers.json`; rotate this file whenever the signing key changes. Keep the `version` field at `1`.
- CI (`ci/check_openapi_spec.sh`) already enforces the allowlist for both the latest and current specs. If another portal or pipeline consumes the signed spec, point its verification step at the same allowlist file to avoid drift.
- To re-sign after a key rotation:
  1. Update `allowed_signers.json` with the new public key.
  2. Regenerate/sign the spec: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Re-run `ci/check_openapi_spec.sh` (or `cargo xtask openapi-verify` manually) to confirm the manifest matches the allowlist.
- If the signing key is held outside the checkout, generate the spec first, have the operator sign the exact bytes of `docs/portal/static/openapi/torii.json`, then refresh the manifest with `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --signature-envelope <signature-envelope-json>`. The detached envelope must be JSON shaped as `{"algorithm":"ed25519","public_key_hex":"...","signature_hex":"..."}` and is verified before `manifest.json` is written.
- For unsigned first-release snapshots, use `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --unsigned-manifest`, then run `npm run sync-openapi -- --allow-unsigned --latest` from `docs/portal`.
