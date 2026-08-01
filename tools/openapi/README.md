# Torii OpenAPI release tooling

This directory contains the code-coupled tooling for the canonical Torii
OpenAPI artifact in `artifacts/openapi/`. Public API documentation belongs in
the sibling `iroha-docs` repository.

The checked-in artifact uses manifest contract version 2. The Rust verifier and
these Node tools reject legacy manifests, unknown fields, unsafe paths, and
digest or signature mismatches. `allowed_signers.json` is deliberately empty;
production operators must supply a separately governed Ed25519 allowlist.

## Development refresh

Generate an explicitly unsigned first-release artifact, synchronize the
`current` alias, and run the metadata checks:

```bash
NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo run --locked --offline -p xtask --features dev-tools --bin xtask -- \
  openapi --output artifacts/openapi/torii.json --unsigned-manifest

npm --prefix tools/openapi run sync-openapi -- --allow-unsigned
npm --prefix tools/openapi test
npm --prefix tools/openapi run check:openapi-versions -- --allow-unsigned
npm --prefix tools/openapi run check:openapi-signatures -- \
  --allow-unsigned=latest --allow-unsigned=current
```

Unsigned artifacts are for development only. Their manifests still bind the
artifact path, byte count, SHA-256, BLAKE3, and generator provenance.

## Release signing

Release signing is detached-only: private keys remain in HSM or PKCS#11
custody. First commit the complete generator input tree. At that clean commit,
emit the deterministic signing payload:

```bash
NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo run --locked --offline -p xtask --features dev-tools --bin xtask -- \
  openapi --output artifacts/openapi/torii.json \
  --unsigned-manifest \
  --signing-payload <operator-staging>/openapi-manifest-v2.payload
```

After the HSM returns an Ed25519 signature envelope, regenerate from the same
source state and attach it:

```bash
NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo run --locked --offline -p xtask --features dev-tools --bin xtask -- \
  openapi --output artifacts/openapi/torii.json \
  --signature-envelope <operator-staging>/openapi-manifest-v2.signature.json

npm --prefix tools/openapi run sync-openapi -- \
  --allowed-signers=<operator-allowlist-path>

OPENAPI_REQUIRE_SIGNED=1 \
OPENAPI_ALLOWED_SIGNERS_FILE=<operator-allowlist-path> \
  bash ci/check_openapi_spec.sh
```

`ci/check_openapi_spec.sh` requires clean generator provenance, byte-identical
root/current artifacts and manifests, deterministic independent generation,
and exact agreement with `release/openapi-generator-inputs-v1.txt`. A valid
release therefore uses a clean source-input commit followed by an
output-bearing commit.
