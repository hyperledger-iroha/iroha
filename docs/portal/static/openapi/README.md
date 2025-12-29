OpenAPI signing
---------------

- The Torii OpenAPI spec (`torii.json`) must be signed, and the manifest is verified by `cargo xtask openapi-verify`.
- Allowed signer keys live in `allowed_signers.json`; rotate this file whenever the signing key changes. Keep the `version` field at `1`.
- CI (`ci/check_openapi_spec.sh`) already enforces the allowlist for both the latest and current specs. If another portal or pipeline consumes the signed spec, point its verification step at the same allowlist file to avoid drift.
- To re-sign after a key rotation:
  1. Update `allowed_signers.json` with the new public key.
  2. Regenerate/sign the spec: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Re-run `ci/check_openapi_spec.sh` (or `cargo xtask openapi-verify` manually) to confirm the manifest matches the allowlist.
