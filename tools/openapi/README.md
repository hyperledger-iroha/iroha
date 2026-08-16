# Torii OpenAPI release tooling

This directory contains the code-coupled tooling for the canonical Torii
OpenAPI artifact in `artifacts/openapi/`. Public API documentation belongs in
the sibling `iroha-docs` repository.

`artifacts/openapi/torii.json` is the authored release authority. Runtime builds
embed an exact package-local mirror at
`crates/iroha_torii/assets/openapi/torii.json`, while
`artifacts/openapi/versions/current/torii.json` is the release alias. All three
files must remain byte-identical. The full Torii release profile serves those
exact bytes; reduced feature profiles prune only route-catalog-gated path and
method entries.

The xtask and release wrappers load the static authority through a live Torii
router, validate its OpenAPI shape and route contract, and emit the bytes for
manifest handling. They are verification/replay tooling, not an independent
schema derivation path. Rust tests additionally bind the authority to the route
catalog, component references, operation effects, and production constants.

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
OPENAPI_RUN_ROOT="$(mktemp -d /private/tmp/iroha-openapi-owner.XXXXXX)"
chmod 700 "${OPENAPI_RUN_ROOT}"
PIN_STAGE="${OPENAPI_RUN_ROOT}/openapi-cargo-lock-v1.txt"

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

`provision` is verification-only. It requires `Cargo.lock` to be one clean,
stage-zero mode-`100644` blob shared by the Git index and `HEAD`, with working
bytes matching that blob and the tracked pin. `--source` adds only a stable,
byte-identical comparison input; it never replaces the tracked root authority.
The provisioner never edits the checkout, starts Cargo, or generates lock
bytes. Release wrappers verify the lock already present in each isolated Git
clone. All Cargo work remains behind the shared `+1.93.1`, `--locked`,
`--offline`, `-j1` policy and its same-snapshot guard.

## Staging-safe development replay

Update `artifacts/openapi/torii.json` and its package-local mirror together,
without normalizing the JSON bytes, then commit the reviewed source authority.
Copy the existing artifact tree to a private `/private/tmp` directory, replay
an explicitly unsigned first-release artifact there through the shared release
process policy, synchronize the `current` alias, and run the metadata check.
The wrapper requires an exact clean candidate, creates a fresh hard-link-free
source clone, seals it read-only, and owns a fresh external Cargo target. The
Node steps do not launch Cargo or read the live artifact tree. Place output
below `<run>/artifacts`, use that directory as the authenticated artifact root,
and keep the cooperative cancellation marker at `<run>/cancel-request.json`,
outside the artifact and Cargo roots. Callers that override either
`IROHA_RELEASE_ARTIFACT_ROOT` or
`IROHA_RELEASE_CANCEL_REQUEST_PATH` must provide both; a cancellation request
is observed only between commands, never by interrupting an in-flight process.
Each script reports both authenticated channel paths before release work starts.

```bash
OPENAPI_RUN_ROOT="$(mktemp -d /private/tmp/iroha-openapi-refresh.XXXXXX)"
chmod 700 "${OPENAPI_RUN_ROOT}"
OPENAPI_ARTIFACT_ROOT="${OPENAPI_RUN_ROOT}/artifacts"
OPENAPI_STAGE="${OPENAPI_ARTIFACT_ROOT}/openapi"
mkdir -m 700 "${OPENAPI_ARTIFACT_ROOT}" "${OPENAPI_STAGE}"
cp -R artifacts/openapi/. "${OPENAPI_STAGE}/"
export IROHA_RELEASE_ARTIFACT_ROOT="${OPENAPI_ARTIFACT_ROOT}"
export IROHA_RELEASE_CANCEL_REQUEST_PATH="${OPENAPI_RUN_ROOT}/cancel-request.json"

bash ci/run_openapi_generator.sh \
  --output-dir "${OPENAPI_STAGE}" \
  --unsigned-manifest

node tools/openapi/scripts/sync-openapi.mjs \
  --version=current --latest --allow-unsigned \
  --output-dir="${OPENAPI_STAGE}"
node tools/openapi/scripts/verify-openapi-versions.mjs \
  --output-dir="${OPENAPI_STAGE}" --allow-unsigned
npm --prefix tools/openapi test
```

Unsigned artifacts are for development only. Their manifests still bind the
artifact path, byte count, SHA-256, BLAKE3, and generator provenance.
Publish the five emitted JSON files only after comparing this complete cache
tree with `artifacts/openapi/` and rechecking the package mirror byte-for-byte;
`allowed_signers.json` remains an input.

## Release signing

Release signing is detached-only: private keys remain encrypted and runtime-only in the external software signer
custody. First commit the complete generator input tree. At that clean commit,
create private out-of-tree staging, copy the existing artifact baseline, and
emit the deterministic signing payload:

```bash
OPENAPI_RUN_ROOT="$(mktemp -d /private/tmp/iroha-openapi-sign.XXXXXX)"
chmod 700 "${OPENAPI_RUN_ROOT}"
OPENAPI_ARTIFACT_ROOT="${OPENAPI_RUN_ROOT}/artifacts"
OPENAPI_STAGE="${OPENAPI_ARTIFACT_ROOT}/openapi"
OPERATOR_STAGE="${OPENAPI_ARTIFACT_ROOT}/operator"
mkdir -m 700 \
  "${OPENAPI_ARTIFACT_ROOT}" "${OPENAPI_STAGE}" "${OPERATOR_STAGE}"
cp -R artifacts/openapi/. "${OPENAPI_STAGE}/"
export IROHA_RELEASE_ARTIFACT_ROOT="${OPENAPI_ARTIFACT_ROOT}"
export IROHA_RELEASE_CANCEL_REQUEST_PATH="${OPENAPI_RUN_ROOT}/cancel-request.json"

bash ci/run_openapi_generator.sh \
  --output-dir "${OPENAPI_STAGE}" \
  --unsigned-manifest \
  --signing-payload "${OPERATOR_STAGE}/openapi-manifest-v2.payload"
```

After the external software signer returns an Ed25519 signature envelope,
replay and emit from the same source state, then attach it:

```bash
chmod 600 "${OPERATOR_STAGE}/openapi-manifest-v2.signature.json"
bash ci/run_openapi_generator.sh \
  --output-dir "${OPENAPI_STAGE}" \
  --signature-envelope "${OPERATOR_STAGE}/openapi-manifest-v2.signature.json"

node tools/openapi/scripts/sync-openapi.mjs \
  --version=current --latest \
  --allowed-signers=<absolute-operator-allowlist-path> \
  --output-dir="${OPENAPI_STAGE}"

OPENAPI_REQUIRE_SIGNED=1 \
OPENAPI_ALLOWED_SIGNERS_FILE=<absolute-operator-allowlist-path> \
  bash ci/check_openapi_spec.sh
```

`ci/check_openapi_spec.sh` requires clean release provenance; byte-identical
root, current, package-local, and live-router authority bytes; deterministic
replay from two independent sealed candidate mirrors; private out-of-tree
targets and staging; and exact agreement with
`release/openapi-generator-inputs-v1.txt`. It retains
the replay bundles and a commit/tree identity receipt below the authenticated
artifact root. A valid release therefore uses a reviewed clean source-input
commit followed by an output-bearing candidate commit. Both standalone scripts
publish their final source-identity receipt only between cooperative
before/after completion boundaries below that authenticated root.
