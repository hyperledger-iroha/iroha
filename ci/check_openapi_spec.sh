#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROCESS_POLICY="${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"
if [[ ! -f "${PROCESS_POLICY}" || -L "${PROCESS_POLICY}" ]]; then
  echo "error: shared release process policy is unavailable or symbolic." >&2
  exit 2
fi
# shellcheck source=../scripts/sumeragi_v2_release_process_policy.sh
source "${PROCESS_POLICY}"

umask 077
OPENAPI_RUN_ROOT="$(mktemp -d /private/tmp/iroha-openapi-check.XXXXXX)"
chmod 700 "${OPENAPI_RUN_ROOT}"

REPLAY_CARGO_TARGET_DIR_FIRST="${OPENAPI_RUN_ROOT}/target-first"
REPLAY_CARGO_TARGET_DIR_SECOND="${OPENAPI_RUN_ROOT}/target-second"
mkdir -m 700 \
  "${REPLAY_CARGO_TARGET_DIR_FIRST}" \
  "${REPLAY_CARGO_TARGET_DIR_SECOND}"

if [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  && -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  IROHA_RELEASE_ARTIFACT_ROOT="${OPENAPI_RUN_ROOT}/artifacts"
  mkdir -m 700 "${IROHA_RELEASE_ARTIFACT_ROOT}"
  IROHA_RELEASE_CANCEL_REQUEST_PATH="${OPENAPI_RUN_ROOT}/cancel-request.json"
  export IROHA_RELEASE_ARTIFACT_ROOT IROHA_RELEASE_CANCEL_REQUEST_PATH
elif [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  echo "error: IROHA_RELEASE_ARTIFACT_ROOT and IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied together." >&2
  exit 2
fi
require_external_private_directory \
  "${REPO_ROOT}" "${REPLAY_CARGO_TARGET_DIR_FIRST}" "OpenAPI Cargo target"
require_external_private_directory \
  "${REPO_ROOT}" "${REPLAY_CARGO_TARGET_DIR_SECOND}" "OpenAPI Cargo target"
require_external_release_artifact_root "${REPO_ROOT}"
CANCEL_REQUEST_PARENT="${IROHA_RELEASE_CANCEL_REQUEST_PATH%/*}"
if [[ -z "${CANCEL_REQUEST_PARENT}" ]]; then
  echo "error: IROHA_RELEASE_CANCEL_REQUEST_PATH must name a file below a private external directory." >&2
  exit 2
fi
require_external_private_directory \
  "${REPO_ROOT}" "${CANCEL_REQUEST_PARENT}" "release cancellation marker parent"
CARGO_TARGET_DIR="${REPLAY_CARGO_TARGET_DIR_FIRST}" \
  require_disjoint_release_roots "${REPO_ROOT}"
CARGO_TARGET_DIR="${REPLAY_CARGO_TARGET_DIR_SECOND}" \
  require_disjoint_release_roots "${REPO_ROOT}"
release_gate_boundary "openapi:channels-ready"
OPENAPI_EVIDENCE_DIR="$(mktemp -d "${IROHA_RELEASE_ARTIFACT_ROOT}/openapi-check.XXXXXX")"
chmod 700 "${OPENAPI_EVIDENCE_DIR}"
require_release_artifact_directory "${OPENAPI_EVIDENCE_DIR}"

report_openapi_run_paths() {
  local status=$?
  trap - EXIT
  printf 'OpenAPI immutable sources and build state: %s\n' "${OPENAPI_RUN_ROOT}" >&2
  printf 'OpenAPI retained evidence: %s\n' "${OPENAPI_EVIDENCE_DIR}" >&2
  exit "${status}"
}
trap report_openapi_run_paths EXIT
printf 'OpenAPI authenticated artifact root: %s\n' \
  "${IROHA_RELEASE_ARTIFACT_ROOT}" >&2
printf 'OpenAPI cooperative cancellation marker: %s\n' \
  "${IROHA_RELEASE_CANCEL_REQUEST_PATH}" >&2

CONFIGURED_ALLOWED_SIGNERS_PATH="${OPENAPI_ALLOWED_SIGNERS_FILE:-artifacts/openapi/allowed_signers.json}"
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

run_xtask_in_repo() {
  local source_root="$1"
  local target_root="$2"
  shift 2
  local -a args=("$@")
  (
    cd "${source_root}"
    GIT_OPTIONAL_LOCKS=0 \
    NORITO_SKIP_BINDINGS_SYNC=1 \
      CARGO_TARGET_DIR="${target_root}" \
      run_cargo run \
        --locked \
        --offline \
        -p xtask \
        --features dev-tools \
        --bin xtask \
        -- \
      "${args[@]}"
  )
}

resolve_allowed_signers_path() {
  local source_root="$1"
  case "${CONFIGURED_ALLOWED_SIGNERS_PATH}" in
    /*) printf '%s\n' "${CONFIGURED_ALLOWED_SIGNERS_PATH}" ;;
    *) printf '%s\n' "${source_root}/${CONFIGURED_ALLOWED_SIGNERS_PATH}" ;;
  esac
}

require_clean_checkout() {
  if [[ -n "$(git -C "${REPO_ROOT}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "error: Torii OpenAPI authority replay requires a clean checkout." >&2
    echo "Commit or remove every tracked and untracked source change, then rerun from the pinned commit." >&2
    exit 1
  fi
}

stage_replay_openapi_dependencies() {
  local source_root="$1"
  local source="${REPO_ROOT}/tools/openapi/node_modules"
  local target="${source_root}/tools/openapi/node_modules"
  if [[ ! -d "${source}" || -L "${source}" ]]; then
    echo "error: install the pinned OpenAPI dependency graph before replay." >&2
    exit 1
  fi
  if [[ -e "${target}" || -L "${target}" ]]; then
    echo "error: immutable OpenAPI replay dependency destination is not fresh." >&2
    exit 1
  fi
  if [[ -n "$(find "${source}" -type l -print -quit)" ]]; then
    echo "error: installed OpenAPI dependency graph must not contain symlinks." >&2
    exit 1
  fi
  if ! npm --prefix "${REPO_ROOT}/tools/openapi" ls --all --omit=dev --json >/dev/null; then
    echo "error: installed OpenAPI dependency graph does not match its lockfile." >&2
    exit 1
  fi
  mkdir "${target}"
  cp -R "${source}/." "${target}/"
  if ! diff -qr "${source}" "${target}" >/dev/null; then
    echo "error: immutable OpenAPI replay dependency copy is not exact." >&2
    exit 1
  fi
}

verify_replay_source_identity() {
  local source_root="$1"
  local actual_commit
  local actual_tree
  local source_status

  actual_commit="$(GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" rev-parse --verify "HEAD^{commit}")"
  if [[ "${actual_commit}" != "${REPLAY_COMMIT}" ]]; then
    echo "error: immutable OpenAPI replay source identity changed." >&2
    exit 1
  fi
  actual_tree="$(GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" rev-parse --verify "HEAD^{tree}")"
  if [[ "${actual_tree}" != "${REPLAY_TREE}" ]]; then
    echo "error: immutable OpenAPI replay source tree identity changed." >&2
    exit 1
  fi
  source_status="$(GIT_OPTIONAL_LOCKS=0 git -C "${source_root}" status --porcelain=v1 --untracked-files=all)"
  if [[ -n "${source_status}" ]]; then
    echo "error: immutable OpenAPI replay source is not at its exact clean candidate identity." >&2
    exit 1
  fi
  if [[ -e "${source_root}/.git/objects/info/alternates" || \
        -L "${source_root}/.git/objects/info/alternates" ]]; then
    echo "error: immutable OpenAPI replay source must have an independent object database." >&2
    exit 1
  fi
}

create_replay_source() {
  local source_root="$1"
  git clone --quiet --local --no-hardlinks --no-checkout \
    "${REPO_ROOT}" "${source_root}"
  git -C "${source_root}" checkout --quiet --detach "${REPLAY_COMMIT}"
  GIT_OPTIONAL_LOCKS=0 node --input-type=module - \
    "${source_root}" \
    "${REPO_ROOT}/Cargo.lock" <<'NODE'
import {realpath} from 'node:fs/promises';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

const [sourceRootArgument, lockSourceArgument] = process.argv.slice(2);
if (!sourceRootArgument || !lockSourceArgument) {
  throw new Error('replay source root and Cargo.lock source paths are required');
}
const replaySourceRoot = await realpath(sourceRootArgument);
const sourcePath = await realpath(lockSourceArgument);
const provisionModule = pathToFileURL(
  join(
    replaySourceRoot,
    'tools',
    'openapi',
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
  repoRoot: replaySourceRoot,
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
  if ! cmp -s "${REPO_ROOT}/Cargo.lock" "${source_root}/Cargo.lock"; then
    echo "error: immutable OpenAPI replay Cargo.lock copy is not byte-identical." >&2
    exit 1
  fi
  stage_replay_openapi_dependencies "${source_root}"
  verify_replay_source_identity "${source_root}"
  python3 -I -S "${source_root}/scripts/seal_workspace_source.py" \
    --seal --root "${source_root}" --no-writable-paths
  python3 -I -S "${source_root}/scripts/seal_workspace_source.py" \
    --verify --root "${source_root}" --no-writable-paths
  verify_replay_source_identity "${source_root}"
}

sync_unsigned_replay_bundle() {
  local source_root="$1"
  local output_dir="$2"
  local allowed_signers_path="$3"
  GIT_OPTIONAL_LOCKS=0 node --input-type=module - \
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
  local target_root="$2"
  local generated_dir="$3"
  local output_dir="$4"
  local allowed_signers_path
  allowed_signers_path="$(resolve_allowed_signers_path "${source_root}")"
  mkdir -m 700 "${generated_dir}" "${output_dir}"
  run_xtask_in_repo \
    "${source_root}" \
    "${target_root}" \
    openapi \
    --output-root "${generated_dir}" \
    --unsigned-manifest
  for generated_artifact in torii.json manifest.json; do
    if [[ ! -f "${generated_dir}/${generated_artifact}" || \
          -L "${generated_dir}/${generated_artifact}" ]]; then
      printf 'error: OpenAPI replay omitted regular generated %s.\n' \
        "${generated_artifact}" >&2
      exit 1
    fi
  done
  python3 -I -S "${source_root}/scripts/seal_workspace_source.py" \
    --verify --root "${source_root}" --no-writable-paths
  verify_replay_source_identity "${source_root}"
  cp -R "${REPLAY_BASELINE}/." "${output_dir}/"
  cp "${generated_dir}/torii.json" "${output_dir}/torii.json"
  cp "${generated_dir}/manifest.json" "${output_dir}/manifest.json"
  sync_unsigned_replay_bundle \
    "${source_root}" \
    "${output_dir}" \
    "${allowed_signers_path}"
}

REPLAY_SOURCE_FIRST="${OPENAPI_RUN_ROOT}/source-first"
REPLAY_SOURCE_SECOND="${OPENAPI_RUN_ROOT}/source-second"
REPLAY_BASELINE="${OPENAPI_EVIDENCE_DIR}/baseline"
REPLAY_GENERATED_FIRST="${OPENAPI_EVIDENCE_DIR}/generated-first"
REPLAY_GENERATED_SECOND="${OPENAPI_EVIDENCE_DIR}/generated-second"
REPLAY_BUNDLE_FIRST="${OPENAPI_EVIDENCE_DIR}/bundle-first"
REPLAY_BUNDLE_SECOND="${OPENAPI_EVIDENCE_DIR}/bundle-second"
GENERATED_SPEC_FIRST="${REPLAY_BUNDLE_FIRST}/torii.json"
RELEASE_INPUT_SUMMARY_FIRST="${OPENAPI_EVIDENCE_DIR}/release-inputs-first.json"
RELEASE_INPUT_SUMMARY_SECOND="${OPENAPI_EVIDENCE_DIR}/release-inputs-second.json"
VERSION_MAP_SUMMARY_FIRST="${OPENAPI_EVIDENCE_DIR}/version-map-first.json"
VERSION_MAP_SUMMARY_SECOND="${OPENAPI_EVIDENCE_DIR}/version-map-second.json"
GENERATED_RELEASE_ARTIFACTS=(
  "torii.json"
  "manifest.json"
  "versions/current/torii.json"
  "versions/current/manifest.json"
  "versions.json"
)

print_refresh_help() {
  cat >&2 <<'EOF'
Update artifacts/openapi/torii.json and its package-local mirror together, then
replay the authority to refresh the canonical manifest and current alias:
  development: bash ci/run_openapi_generator.sh \
                 --output-dir <absolute-private-tmp>/openapi \
                 --unsigned-manifest
               node tools/openapi/scripts/sync-openapi.mjs \
                 --version=current --latest --allow-unsigned \
                 --output-dir=<absolute-private-tmp>/openapi
  release payload:
               bash ci/run_openapi_generator.sh \
                 --output-dir <absolute-private-tmp>/openapi \
                 --unsigned-manifest \
                 --signing-payload <absolute-operator-staging>/openapi-manifest-v2.payload
  release attach after the authenticated external software Ed25519 signer signs
  those exact bytes:
               bash ci/run_openapi_generator.sh \
                 --output-dir <absolute-private-tmp>/openapi \
                 --signature-envelope <absolute-operator-staging>/openapi-manifest-v2.signature.json
               node tools/openapi/scripts/sync-openapi.mjs \
                 --version=current --latest \
                 --allowed-signers=<absolute-operator-allowlist-path> \
                 --output-dir=<absolute-private-tmp>/openapi
The V1 release policy fixes signing_provider=authenticated_external_signer and
signing_backend=software; successfully verified release output is
signer_qualification=software-key-qualified. The provider boundary remains
compatible with a later HSM adapter, which requires new HSM-backed evidence.
Local private-key signing is intentionally unavailable; release signing is detached-only.
For an operator release, set OPENAPI_REQUIRE_SIGNED=1 and
OPENAPI_ALLOWED_SIGNERS_FILE=<absolute-operator-allowlist-path> when running this gate.
The checked-in allowlist is intentionally empty. Signed mode requires the
root/latest/current release artifacts to be signed.
This gate always requires a clean checkout and clean mutable release
provenance (the manifest retains its V2 generator_* field names).
generator_commit must resolve to a real ancestor of HEAD, and
generator_source_sha256_hex must match the canonical release-input inventory at
both that pinned commit and the output-bearing HEAD. --allow-unsigned relaxes
only the detached-signature requirement; it never permits generator_dirty.
EOF
}

require_clean_checkout
REPLAY_COMMIT="$(git -C "${REPO_ROOT}" rev-parse --verify "HEAD^{commit}")"
REPLAY_TREE="$(git -C "${REPO_ROOT}" rev-parse --verify "${REPLAY_COMMIT}^{tree}")"
release_gate_boundary "openapi:before-source-mirrors"
create_replay_source "${REPLAY_SOURCE_FIRST}"
create_replay_source "${REPLAY_SOURCE_SECOND}"
require_clean_checkout

OPENAPI_DIR="${REPLAY_SOURCE_FIRST}/artifacts/openapi"
SPEC_PATH="${OPENAPI_DIR}/torii.json"
CURRENT_SPEC_PATH="${OPENAPI_DIR}/versions/current/torii.json"
PACKAGE_SPEC_PATH="${REPLAY_SOURCE_FIRST}/crates/iroha_torii/assets/openapi/torii.json"
MANIFEST_PATH="${OPENAPI_DIR}/manifest.json"
CURRENT_MANIFEST_PATH="${OPENAPI_DIR}/versions/current/manifest.json"
ALLOWED_SIGNERS_PATH="$(resolve_allowed_signers_path "${REPLAY_SOURCE_FIRST}")"

# The release authority, current alias, and package-local runtime mirror are one
# byte identity. Reject drift before paying for either live-router replay.
if [[ ! -f "${SPEC_PATH}" || -L "${SPEC_PATH}" ]]; then
  echo "error: canonical Torii OpenAPI authority must be a regular file: ${SPEC_PATH}" >&2
  exit 1
fi
for authority in "${CURRENT_SPEC_PATH}" "${PACKAGE_SPEC_PATH}"; do
  if [[ ! -f "${authority}" || -L "${authority}" ]]; then
    echo "error: checked-in Torii OpenAPI authority must be a regular file: ${authority}" >&2
    exit 1
  fi
  if ! cmp -s "${SPEC_PATH}" "${authority}"; then
    diff -u "${SPEC_PATH}" "${authority}" || true
    echo "error: checked-in Torii OpenAPI authorities are not byte-identical." >&2
    print_refresh_help
    exit 1
  fi
done

# Reject a stale first-release Musubi route/model contract before paying for
# two complete live-router replays. The read-only check runs from the same
# sealed candidate mirror used by every Cargo gate below.
(
  cd "${REPLAY_SOURCE_FIRST}"
  GIT_OPTIONAL_LOCKS=0 node tools/openapi/scripts/verify-musubi-v1-contract.mjs
)

if ! diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}" >/dev/null; then
  diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}" || true
  echo "error: checked-in latest/current OpenAPI manifests are not byte-identical." >&2
  print_refresh_help
  exit 1
fi

(
  cd "${REPLAY_SOURCE_FIRST}"
  GIT_OPTIONAL_LOCKS=0 node tools/openapi/scripts/verify-openapi-release-inputs.mjs \
    >"${RELEASE_INPUT_SUMMARY_FIRST}"
  GIT_OPTIONAL_LOCKS=0 python3 scripts/check_sorafs_release_version_map.py \
    >"${VERSION_MAP_SUMMARY_FIRST}"
  GIT_OPTIONAL_LOCKS=0 node tools/openapi/scripts/verify-openapi-versions.mjs
)

# Load the static authority through live Torii routers in two independent,
# hard-link-free, sealed candidate clones. Assemble both replays from the same
# immutable checked-in baseline; the caller's checkout remains read-only.
mkdir -m 700 "${REPLAY_BASELINE}"
cp -R "${OPENAPI_DIR}/." "${REPLAY_BASELINE}/"
build_unsigned_replay_bundle \
  "${REPLAY_SOURCE_FIRST}" \
  "${REPLAY_CARGO_TARGET_DIR_FIRST}" \
  "${REPLAY_GENERATED_FIRST}" \
  "${REPLAY_BUNDLE_FIRST}"
build_unsigned_replay_bundle \
  "${REPLAY_SOURCE_SECOND}" \
  "${REPLAY_CARGO_TARGET_DIR_SECOND}" \
  "${REPLAY_GENERATED_SECOND}" \
  "${REPLAY_BUNDLE_SECOND}"

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
  echo "error: the live Torii router did not serve the checked-in static authority." >&2
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

run_xtask_in_repo \
  "${REPLAY_SOURCE_FIRST}" \
  "${REPLAY_CARGO_TARGET_DIR_FIRST}" \
  openapi-verify \
  --spec "${SPEC_PATH}" \
  --manifest "${MANIFEST_PATH}" \
  --allowed-signers "${ALLOWED_SIGNERS_PATH}" \
  "${XTASK_VERIFY_POLICY_ARGS[@]}"

run_xtask_in_repo \
  "${REPLAY_SOURCE_FIRST}" \
  "${REPLAY_CARGO_TARGET_DIR_FIRST}" \
  openapi-verify \
  --spec "${CURRENT_SPEC_PATH}" \
  --manifest "${CURRENT_MANIFEST_PATH}" \
  --allowed-signers "${ALLOWED_SIGNERS_PATH}" \
  "${XTASK_VERIFY_POLICY_ARGS[@]}"

(
  cd "${REPLAY_SOURCE_FIRST}"
  GIT_OPTIONAL_LOCKS=0 node tools/openapi/scripts/verify-openapi-versions.mjs
  GIT_OPTIONAL_LOCKS=0 node tools/openapi/scripts/check-openapi-signatures.mjs \
    --allowed-signers="${ALLOWED_SIGNERS_PATH}" \
    "${SIGNATURE_VERIFY_POLICY_ARGS[@]}"
  GIT_OPTIONAL_LOCKS=0 node tools/openapi/scripts/verify-openapi-release-inputs.mjs \
    >"${RELEASE_INPUT_SUMMARY_SECOND}"
  GIT_OPTIONAL_LOCKS=0 python3 scripts/check_sorafs_release_version_map.py \
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

python3 -I -S "${REPLAY_SOURCE_FIRST}/scripts/seal_workspace_source.py" \
  --verify --root "${REPLAY_SOURCE_FIRST}" --no-writable-paths
python3 -I -S "${REPLAY_SOURCE_SECOND}/scripts/seal_workspace_source.py" \
  --verify --root "${REPLAY_SOURCE_SECOND}" --no-writable-paths
verify_replay_source_identity "${REPLAY_SOURCE_FIRST}"
verify_replay_source_identity "${REPLAY_SOURCE_SECOND}"
if ! cmp -s "${REPO_ROOT}/Cargo.lock" "${REPLAY_SOURCE_FIRST}/Cargo.lock" || \
   ! cmp -s "${REPLAY_SOURCE_FIRST}/Cargo.lock" "${REPLAY_SOURCE_SECOND}/Cargo.lock"; then
  echo "error: immutable OpenAPI replay Cargo.lock identity changed between checkpoints." >&2
  exit 1
fi
require_clean_checkout
release_gate_boundary "openapi:before-completion-publication"
python3 -I -S - \
  "${OPENAPI_EVIDENCE_DIR}/source-identity.json" \
  "${REPLAY_COMMIT}" \
  "${REPLAY_TREE}" \
  "${REPLAY_SOURCE_FIRST}" \
  "${REPLAY_SOURCE_SECOND}" <<'PY'
import json
import os
import sys

receipt_path, commit, tree, source_first, source_second = sys.argv[1:]
payload = {
    "candidate_commit": commit,
    "candidate_tree": tree,
    "schema_version": 1,
    "source_mirrors": [source_first, source_second],
}
descriptor = os.open(
    receipt_path,
    os.O_WRONLY | os.O_CREAT | os.O_EXCL,
    0o600,
)
with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as output:
    json.dump(payload, output, sort_keys=True, separators=(",", ":"))
    output.write("\n")
PY
release_gate_boundary "openapi:after-completion-publication"
