# Torii OpenAPI release tooling

This directory contains the code-coupled tooling for the canonical Torii
OpenAPI artifact in `artifacts/openapi/`. Public API documentation belongs in
the sibling `iroha-docs` repository.

The checked-in artifact uses manifest contract version 2. The Rust verifier and
these Node tools reject legacy manifests, unknown fields, unsafe paths, and
digest or signature mismatches. `allowed_signers.json` is deliberately empty;
production operators must supply a separately governed Ed25519 allowlist.

## Cargo.lock pin owner

`release/openapi-cargo-lock-v1.txt` is generated metadata, not a second lock
authority. Derive it only from an explicit, stable Cargo.lock into an absolute
path outside the repository:

```bash
REPO_ROOT="$(pwd -P)"
PIN_STAGE=<task-owned-cache>/openapi-cargo-lock-v1.txt

node tools/openapi/scripts/provision-openapi-cargo-lock.mjs pin \
  --source="${REPO_ROOT}/Cargo.lock" \
  --output="${PIN_STAGE}"
node tools/openapi/scripts/provision-openapi-cargo-lock.mjs pin \
  --source="${REPO_ROOT}/Cargo.lock" \
  --check="${PIN_STAGE}"
```

The owner rejects relative paths, links, executable inputs, source races, and
repository output paths. It never edits `Cargo.lock` or the tracked pin.
Publish reviewed staged bytes through the repository's per-file preimage guard,
then run the `--check` form against the tracked pin. Rust and Node parse that
tracked file as the sole size and SHA-256 authority; no hash constant is
hand-maintained in either implementation.

## Staging-safe development refresh

Copy the existing artifact tree to a task-owned cache directory, generate an
explicitly unsigned first-release artifact there, synchronize the `current`
alias from that canonical spec, and run the metadata check. The Node steps do
not launch Cargo or read the live artifact tree.

```bash
OPENAPI_STAGE=<task-owned-cache>/openapi
mkdir -p "${OPENAPI_STAGE}"
cp -R artifacts/openapi/. "${OPENAPI_STAGE}/"

NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo run --locked --offline --jobs 1 -Z unstable-options \
  --lockfile-path Cargo.lock -p xtask --features dev-tools --bin xtask -- \
  openapi --output-root "${OPENAPI_STAGE}" --unsigned-manifest

node tools/openapi/scripts/sync-openapi.mjs \
  --version=current --latest --allow-unsigned \
  --output-dir="${OPENAPI_STAGE}"
node tools/openapi/scripts/verify-openapi-versions.mjs \
  --output-dir="${OPENAPI_STAGE}" --allow-unsigned
npm --prefix tools/openapi test
```

Unsigned artifacts are for development only. Their manifests still bind the
artifact path, byte count, SHA-256, BLAKE3, and generator provenance.
Publish the five generated JSON files only after comparing this complete cache
tree with `artifacts/openapi/`; `allowed_signers.json` remains an input.

## Release signing

Release signing is detached-only: private keys remain in HSM or PKCS#11
custody. First commit the complete generator input tree. At that clean commit,
emit the deterministic signing payload:

```bash
NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo run --locked --offline --jobs 1 -Z unstable-options \
  --lockfile-path Cargo.lock -p xtask --features dev-tools --bin xtask -- \
  openapi --output-root artifacts/openapi \
  --unsigned-manifest \
  --signing-payload <operator-staging>/openapi-manifest-v2.payload
```

After the HSM returns an Ed25519 signature envelope, regenerate from the same
source state and attach it:

```bash
NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo run --locked --offline --jobs 1 -Z unstable-options \
  --lockfile-path Cargo.lock -p xtask --features dev-tools --bin xtask -- \
  openapi --output-root artifacts/openapi \
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
