#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

OPENAPI_DIR="${REPO_ROOT}/artifacts/openapi"
SPEC_PATH="${OPENAPI_DIR}/torii.json"
CURRENT_SPEC_PATH="${OPENAPI_DIR}/versions/current/torii.json"
MANIFEST_PATH="${OPENAPI_DIR}/manifest.json"
CURRENT_MANIFEST_PATH="${OPENAPI_DIR}/versions/current/manifest.json"
CONFIGURED_ALLOWED_SIGNERS_PATH="${OPENAPI_ALLOWED_SIGNERS_FILE:-${OPENAPI_DIR}/allowed_signers.json}"
case "${CONFIGURED_ALLOWED_SIGNERS_PATH}" in
  /*)
    ALLOWED_SIGNERS_PATH="${CONFIGURED_ALLOWED_SIGNERS_PATH}"
    ;;
  *)
    ALLOWED_SIGNERS_PATH="${REPO_ROOT}/${CONFIGURED_ALLOWED_SIGNERS_PATH}"
    ;;
esac
REQUIRE_SIGNED="${OPENAPI_REQUIRE_SIGNED:-0}"

case "${REQUIRE_SIGNED}" in
  0|1) ;;
  *)
    echo "error: OPENAPI_REQUIRE_SIGNED must be 0 or 1." >&2
    exit 2
    ;;
esac

XTASK_VERIFY_POLICY_ARGS=()
SIGNATURE_VERIFY_POLICY_ARGS=()
if [[ "${REQUIRE_SIGNED}" == "0" ]]; then
  XTASK_VERIFY_POLICY_ARGS+=(--allow-unsigned)
  SIGNATURE_VERIFY_POLICY_ARGS+=(--allow-unsigned=latest --allow-unsigned=current)
fi

TMP_DIR="$(mktemp -d)"
REPLAY_WORKTREES=()
cleanup() {
  local worktree
  for worktree in "${REPLAY_WORKTREES[@]}"; do
    git -C "${REPO_ROOT}" worktree remove --force "${worktree}" >/dev/null 2>&1 || true
  done
  rm -rf -- "${TMP_DIR}"
}
trap cleanup EXIT

REPLAY_CARGO_TARGET_DIR="${TMP_DIR}/cargo-target"
mkdir -p "${REPLAY_CARGO_TARGET_DIR}"

run_xtask() {
  local -a args=("$@")
  NORITO_SKIP_BINDINGS_SYNC=1 \
    CARGO_TARGET_DIR="${REPLAY_CARGO_TARGET_DIR}" \
    cargo run \
      --locked \
      --offline \
      -p xtask \
      --features dev-tools \
      --bin xtask \
      -- \
      "${args[@]}"
}

run_xtask_in_repo() {
  local source_root="$1"
  shift
  local -a args=("$@")
  (
    cd "${source_root}"
    NORITO_SKIP_BINDINGS_SYNC=1 \
      CARGO_TARGET_DIR="${REPLAY_CARGO_TARGET_DIR}" \
      cargo run \
        --locked \
        --offline \
        -p xtask \
        --features dev-tools \
        --bin xtask \
        -- \
        "${args[@]}"
  )
}

require_clean_checkout() {
  if [[ -n "$(git -C "${REPO_ROOT}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "error: Torii OpenAPI release generation requires a clean checkout." >&2
    echo "Commit or remove every tracked and untracked source change, then rerun from the pinned commit." >&2
    exit 1
  fi
}

create_replay_worktree() {
  local worktree="$1"
  REPLAY_WORKTREES+=("${worktree}")
  git -C "${REPO_ROOT}" worktree add --quiet --detach "${worktree}" "${REPLAY_COMMIT}"
  node --input-type=module - \
    "${worktree}" \
    "${REPO_ROOT}/Cargo.lock" <<'NODE'
import {realpath} from 'node:fs/promises';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

const [worktreeArgument, sourceArgument] = process.argv.slice(2);
if (!worktreeArgument || !sourceArgument) {
  throw new Error('replay worktree and Cargo.lock source paths are required');
}
const worktreeRoot = await realpath(worktreeArgument);
const sourcePath = await realpath(sourceArgument);
const provisionModule = pathToFileURL(
  join(
    worktreeRoot,
    'docs',
    'portal',
    'scripts',
    'provision-openapi-cargo-lock.mjs',
  ),
).href;
const {
  OPENAPI_CARGO_LOCK_EXPECTED_BYTES,
  OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX,
  OPENAPI_CARGO_LOCK_PROVISION_SCHEMA,
  provisionOpenApiCargoLock,
} = await import(provisionModule);
const summary = await provisionOpenApiCargoLock({
  repoRoot: worktreeRoot,
  sourcePath,
});
if (
  summary.schema !== OPENAPI_CARGO_LOCK_PROVISION_SCHEMA ||
  summary.status !== 'installed' ||
  summary.source !== 'operator' ||
  summary.path !== 'Cargo.lock' ||
  summary.bytes !== OPENAPI_CARGO_LOCK_EXPECTED_BYTES ||
  summary.sha256_hex !== OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX
) {
  throw new Error('isolated OpenAPI replay Cargo.lock provisioning was not exact');
}
NODE
  if [[ -n "$(git -C "${worktree}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "error: isolated OpenAPI replay worktree is not clean after Cargo.lock provisioning." >&2
    exit 1
  fi
}

sync_unsigned_replay_bundle() {
  local source_root="$1"
  local output_dir="$2"
  local allowed_signers_path="$3"
  node --input-type=module - \
    "${source_root}" \
    "${output_dir}" \
    "${allowed_signers_path}" <<'NODE'
import {copyFile, readFile} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const [
  sourceRootArgument,
  outputDirArgument,
  allowedSignersFileArgument,
] = process.argv.slice(2);
if (
  !sourceRootArgument ||
  !outputDirArgument ||
  !allowedSignersFileArgument
) {
  throw new Error(
    'isolated OpenAPI replay source, output, and allowed-signers paths are required',
  );
}
const sourceRoot = resolve(sourceRootArgument);
const outputDir = resolve(outputDirArgument);
const allowedSignersFile = resolve(allowedSignersFileArgument);
const versionsDir = join(outputDir, 'versions');
const generatedSpec = join(outputDir, 'torii.json');
const syncModule = pathToFileURL(
  join(sourceRoot, 'tools', 'openapi', 'scripts', 'sync-openapi.mjs'),
).href;
const {syncOpenApi} = await import(syncModule);

await syncOpenApi(
  {
    version: 'current',
    latest: true,
    mirrors: [],
    requireSigned: false,
  },
  {
    repoRoot: sourceRoot,
    outputDir,
    versionsDir,
    allowedSignersFile,
    async generateSpec(_repoRoot, outputFile) {
      await copyFile(generatedSpec, outputFile);
    },
  },
);

for (const relativeManifest of [
  'manifest.json',
  join('versions', 'current', 'manifest.json'),
]) {
  const manifest = JSON.parse(
    await readFile(join(outputDir, relativeManifest), 'utf8'),
  );
  if (
    manifest.generator_dirty !== false ||
    !/^[0-9a-f]{40}$/.test(manifest.generator_commit) ||
    manifest.artifact?.signature !== null
  ) {
    throw new Error(
      `isolated OpenAPI replay manifest ${relativeManifest} is not clean and unsigned`,
    );
  }
}
NODE
}

build_unsigned_replay_bundle() {
  local source_root="$1"
  local output_dir="$2"
  run_xtask_in_repo "${source_root}" openapi --unsigned-manifest
  mkdir -p "${output_dir}"
  cp -R "${REPLAY_BASELINE}/." "${output_dir}/"
  cp "${source_root}/artifacts/openapi/torii.json" "${output_dir}/torii.json"
  cp "${source_root}/artifacts/openapi/manifest.json" "${output_dir}/manifest.json"
  sync_unsigned_replay_bundle \
    "${source_root}" \
    "${output_dir}" \
    "${ALLOWED_SIGNERS_PATH}"
}

REPLAY_WORKTREE_FIRST="${TMP_DIR}/openapi-replay-source-first"
REPLAY_WORKTREE_SECOND="${TMP_DIR}/openapi-replay-source-second"
REPLAY_BASELINE="${TMP_DIR}/openapi-replay-baseline"
REPLAY_BUNDLE_FIRST="${TMP_DIR}/openapi-replay-first"
REPLAY_BUNDLE_SECOND="${TMP_DIR}/openapi-replay-second"
GENERATED_SPEC_FIRST="${REPLAY_BUNDLE_FIRST}/torii.json"
RELEASE_INPUT_SUMMARY_FIRST="${TMP_DIR}/release-inputs-first.json"
RELEASE_INPUT_SUMMARY_SECOND="${TMP_DIR}/release-inputs-second.json"
VERSION_MAP_SUMMARY_FIRST="${TMP_DIR}/version-map-first.json"
VERSION_MAP_SUMMARY_SECOND="${TMP_DIR}/version-map-second.json"
GENERATED_RELEASE_ARTIFACTS=(
  "torii.json"
  "manifest.json"
  "versions/current/torii.json"
  "versions/current/manifest.json"
  "versions.json"
)

print_refresh_help() {
  cat >&2 <<'EOF'
Refresh the canonical manifest before syncing snapshots:
  development: cargo run --locked --offline -p xtask --features dev-tools --bin xtask -- openapi --unsigned-manifest
               (cd tools/openapi && npm run sync-openapi -- --allow-unsigned)
  release payload:
               cargo run --locked --offline -p xtask --features dev-tools --bin xtask -- openapi \
                 --unsigned-manifest --signing-payload <operator-staging>/openapi-manifest-v2.payload
  release attach after the Ed25519 HSM signs those exact bytes:
               cargo run --locked --offline -p xtask --features dev-tools --bin xtask -- openapi \
                 --signature-envelope <operator-staging>/openapi-manifest-v2.signature.json
               (cd tools/openapi && npm run sync-openapi -- --allowed-signers=<operator-allowlist-path>)
Local private-key signing is intentionally unavailable; release signing is detached-only.
For an operator release, set OPENAPI_REQUIRE_SIGNED=1 and
OPENAPI_ALLOWED_SIGNERS_FILE=<operator-allowlist-path> when running this gate.
The checked-in allowlist is intentionally empty. Signed mode requires the
root/latest/current release artifacts to be signed.
This gate always requires a clean checkout and clean mutable generator
provenance. generator_commit must resolve to a real ancestor of HEAD, and
generator_source_sha256_hex must match the canonical release-input inventory at
both that pinned commit and the output-bearing HEAD. --allow-unsigned relaxes
only the detached-signature requirement; it never permits generator_dirty.
EOF
}

require_clean_checkout
REPLAY_COMMIT="$(git -C "${REPO_ROOT}" rev-parse --verify "HEAD^{commit}")"

if ! diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}" >/dev/null; then
  diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}" || true
  echo "error: checked-in latest/current OpenAPI manifests are not byte-identical." >&2
  print_refresh_help
  exit 1
fi

(
  cd "${REPO_ROOT}"
  node tools/openapi/scripts/verify-openapi-release-inputs.mjs \
    >"${RELEASE_INPUT_SUMMARY_FIRST}"
  python3 scripts/check_sorafs_release_version_map.py \
    >"${VERSION_MAP_SUMMARY_FIRST}"
  node tools/openapi/scripts/verify-openapi-versions.mjs
)

# xtask intentionally emits manifests only beside the canonical spec path.
# Generate in two pristine detached worktrees, then assemble each replay from
# the same immutable checked-in baseline so the caller's tree remains read-only.
mkdir -p "${REPLAY_BASELINE}"
cp -R "${OPENAPI_DIR}/." "${REPLAY_BASELINE}/"
create_replay_worktree "${REPLAY_WORKTREE_FIRST}"
create_replay_worktree "${REPLAY_WORKTREE_SECOND}"
build_unsigned_replay_bundle "${REPLAY_WORKTREE_FIRST}" "${REPLAY_BUNDLE_FIRST}"
build_unsigned_replay_bundle "${REPLAY_WORKTREE_SECOND}" "${REPLAY_BUNDLE_SECOND}"

for artifact in "${GENERATED_RELEASE_ARTIFACTS[@]}"; do
  first="${REPLAY_BUNDLE_FIRST}/${artifact}"
  second="${REPLAY_BUNDLE_SECOND}/${artifact}"
  if [[ ! -f "${first}" || -L "${first}" || ! -f "${second}" || -L "${second}" ]]; then
    echo "error: complete OpenAPI replay did not produce regular ${artifact} artifacts." >&2
    exit 1
  fi
  if ! diff -u "${first}" "${second}" >/dev/null; then
    diff -u "${first}" "${second}" || true
    echo "error: two complete Torii OpenAPI replay bundles disagreed at ${artifact}." >&2
    exit 1
  fi
done

if ! diff -ru "${REPLAY_BUNDLE_FIRST}" "${REPLAY_BUNDLE_SECOND}" >/dev/null; then
  diff -ru "${REPLAY_BUNDLE_FIRST}" "${REPLAY_BUNDLE_SECOND}" || true
  echo "error: two complete Torii OpenAPI replay trees produced different bytes." >&2
  exit 1
fi

if ! diff -u "${SPEC_PATH}" "${GENERATED_SPEC_FIRST}" >/dev/null; then
  diff -u "${SPEC_PATH}" "${GENERATED_SPEC_FIRST}" || true
  echo "error: artifacts/openapi/torii.json is stale." >&2
  print_refresh_help
  exit 1
fi

if ! diff -u "${SPEC_PATH}" "${CURRENT_SPEC_PATH}" >/dev/null; then
  diff -u "${SPEC_PATH}" "${CURRENT_SPEC_PATH}" || true
  echo "error: artifacts/openapi/versions/current/torii.json is out of sync with the latest spec." >&2
  print_refresh_help
  exit 1
fi

require_clean_checkout

(
  cd "${REPO_ROOT}"
  run_xtask openapi-verify \
    --spec "${SPEC_PATH}" \
    --manifest "${MANIFEST_PATH}" \
    --allowed-signers "${ALLOWED_SIGNERS_PATH}" \
    "${XTASK_VERIFY_POLICY_ARGS[@]}"
)

(
  cd "${REPO_ROOT}"
  run_xtask openapi-verify \
    --spec "${CURRENT_SPEC_PATH}" \
    --manifest "${CURRENT_MANIFEST_PATH}" \
    --allowed-signers "${ALLOWED_SIGNERS_PATH}" \
    "${XTASK_VERIFY_POLICY_ARGS[@]}"
)

(
  cd "${REPO_ROOT}"
  node tools/openapi/scripts/verify-openapi-versions.mjs
  node tools/openapi/scripts/check-openapi-signatures.mjs \
    --allowed-signers="${ALLOWED_SIGNERS_PATH}" \
    "${SIGNATURE_VERIFY_POLICY_ARGS[@]}"
  node tools/openapi/scripts/verify-openapi-release-inputs.mjs \
    >"${RELEASE_INPUT_SUMMARY_SECOND}"
  python3 scripts/check_sorafs_release_version_map.py \
    >"${VERSION_MAP_SUMMARY_SECOND}"
)

if ! diff -u "${RELEASE_INPUT_SUMMARY_FIRST}" "${RELEASE_INPUT_SUMMARY_SECOND}" >/dev/null; then
  diff -u "${RELEASE_INPUT_SUMMARY_FIRST}" "${RELEASE_INPUT_SUMMARY_SECOND}" || true
  echo "error: two clean Torii OpenAPI release-input verification passes disagreed." >&2
  exit 1
fi

if ! diff -u "${VERSION_MAP_SUMMARY_FIRST}" "${VERSION_MAP_SUMMARY_SECOND}" >/dev/null; then
  diff -u "${VERSION_MAP_SUMMARY_FIRST}" "${VERSION_MAP_SUMMARY_SECOND}" || true
  echo "error: two SoraFS release version-map verification passes disagreed." >&2
  exit 1
fi

require_clean_checkout
