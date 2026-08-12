#!/usr/bin/env bash
# Validate and emit Torii's static OpenAPI authority from one exact, sealed candidate mirror.
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

usage() {
  cat >&2 <<'EOF'
usage: bash ci/run_openapi_generator.sh --output-dir ABSOLUTE_PRIVATE_TMP_DIR \
         (--unsigned-manifest [--signing-payload ABSOLUTE_PRIVATE_TMP_FILE] | \
          --signature-envelope ABSOLUTE_PRIVATE_TMP_FILE)

The output directory and its dedicated artifacts parent must already exist,
be owner-private, resolve below /private/tmp, and remain outside the source
repository. By default the output is <run>/artifacts/<stage>, the artifacts
parent is the authenticated artifact root, and cancellation is
<run>/cancel-request.json. Cargo loads the package-local authority through a
live Torii router from a fresh, hard-link-free, sealed clone at the caller's
exact clean HEAD through the shared +1.93.1/--locked/--offline/-j1 process
policy.
EOF
  exit 2
}

OUTPUT_DIR=""
UNSIGNED_MANIFEST=0
SIGNING_PAYLOAD=""
SIGNATURE_ENVELOPE=""
SEEN_OUTPUT_DIR=0
SEEN_UNSIGNED_MANIFEST=0
SEEN_SIGNING_PAYLOAD=0
SEEN_SIGNATURE_ENVELOPE=0
while (($# > 0)); do
  case "$1" in
    --output-dir)
      (($# >= 2)) || usage
      [[ "${SEEN_OUTPUT_DIR}" == 0 ]] || usage
      SEEN_OUTPUT_DIR=1
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --unsigned-manifest)
      [[ "${SEEN_UNSIGNED_MANIFEST}" == 0 ]] || usage
      SEEN_UNSIGNED_MANIFEST=1
      UNSIGNED_MANIFEST=1
      shift
      ;;
    --signing-payload)
      (($# >= 2)) || usage
      [[ "${SEEN_SIGNING_PAYLOAD}" == 0 ]] || usage
      SEEN_SIGNING_PAYLOAD=1
      SIGNING_PAYLOAD="$2"
      shift 2
      ;;
    --signature-envelope)
      (($# >= 2)) || usage
      [[ "${SEEN_SIGNATURE_ENVELOPE}" == 0 ]] || usage
      SEEN_SIGNATURE_ENVELOPE=1
      SIGNATURE_ENVELOPE="$2"
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      printf 'error: unsupported OpenAPI generator argument: %s\n' "$1" >&2
      usage
      ;;
  esac
done

if [[ -z "${OUTPUT_DIR}" ]]; then
  echo "error: --output-dir is required." >&2
  usage
fi
if [[ "${UNSIGNED_MANIFEST}" == 1 && -n "${SIGNATURE_ENVELOPE}" ]]; then
  echo "error: unsigned generation and signature attachment are mutually exclusive." >&2
  usage
fi
if [[ -n "${SIGNING_PAYLOAD}" && "${UNSIGNED_MANIFEST}" != 1 ]]; then
  echo "error: --signing-payload requires --unsigned-manifest." >&2
  usage
fi
if [[ "${UNSIGNED_MANIFEST}" != 1 && -z "${SIGNATURE_ENVELOPE}" ]]; then
  echo "error: choose explicit unsigned generation or detached signature attachment." >&2
  usage
fi

umask 077
require_external_private_directory \
  "${REPO_ROOT}" "${OUTPUT_DIR}" "OpenAPI output"

validate_external_payload_path() {
  local path="$1"
  local purpose="$2"
  local disposition="$3"
  python3 -I -S - "${REPO_ROOT}" "${path}" "${purpose}" "${disposition}" <<'PY'
import os
import stat
import sys

source_root, path, purpose, disposition = sys.argv[1:]
if not os.path.isabs(path):
    print(f"{purpose} path must be absolute", file=sys.stderr)
    raise SystemExit(2)
canonical = os.path.realpath(path)
parent = os.path.dirname(path)
canonical_parent = os.path.realpath(parent)
private_tmp = os.path.realpath("/private/tmp")
source = os.path.realpath(source_root)
try:
    parent_stat = os.lstat(parent)
except OSError as error:
    print(f"{purpose} parent is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
try:
    under_private_tmp = os.path.commonpath((canonical_parent, private_tmp)) == private_tmp
    under_source = os.path.commonpath((canonical, source)) == source
except ValueError:
    under_private_tmp = False
    under_source = True
if (
    parent != canonical_parent
    or not stat.S_ISDIR(parent_stat.st_mode)
    or stat.S_ISLNK(parent_stat.st_mode)
    or parent_stat.st_uid != os.getuid()
    or parent_stat.st_mode & 0o077
    or canonical_parent == private_tmp
    or not under_private_tmp
    or under_source
):
    print(
        f"{purpose} parent must be one canonical private owner directory "
        "below /private/tmp and outside source",
        file=sys.stderr,
    )
    raise SystemExit(2)
if disposition == "output":
    try:
        os.lstat(path)
    except FileNotFoundError:
        raise SystemExit(0)
    print(f"{purpose} output must not already exist", file=sys.stderr)
    raise SystemExit(2)
try:
    observed = os.lstat(path)
except OSError as error:
    print(f"{purpose} input is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    canonical != path
    or not stat.S_ISREG(observed.st_mode)
    or stat.S_ISLNK(observed.st_mode)
    or observed.st_nlink != 1
    or observed.st_uid != os.getuid()
    or observed.st_mode & 0o077
):
    print(f"{purpose} input must be canonical, regular, single-link, and owner-private", file=sys.stderr)
    raise SystemExit(2)
PY
}

if [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  && -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  IROHA_RELEASE_ARTIFACT_ROOT="${OUTPUT_DIR%/*}"
  IROHA_RELEASE_CANCEL_REQUEST_PATH="${IROHA_RELEASE_ARTIFACT_ROOT%/*}/cancel-request.json"
  export IROHA_RELEASE_ARTIFACT_ROOT IROHA_RELEASE_CANCEL_REQUEST_PATH
elif [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  echo "error: IROHA_RELEASE_ARTIFACT_ROOT and IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied together." >&2
  exit 2
fi
require_external_release_artifact_root "${REPO_ROOT}"
if [[ "${OUTPUT_DIR}" == "${IROHA_RELEASE_ARTIFACT_ROOT}" ]]; then
  echo "error: OpenAPI output must be a child of the authenticated artifact root." >&2
  exit 2
fi
require_release_artifact_directory "${OUTPUT_DIR}"
CANCEL_REQUEST_PARENT="${IROHA_RELEASE_CANCEL_REQUEST_PATH%/*}"
if [[ -z "${CANCEL_REQUEST_PARENT}" ]]; then
  echo "error: IROHA_RELEASE_CANCEL_REQUEST_PATH must name a file below a private external directory." >&2
  exit 2
fi
require_external_private_directory \
  "${REPO_ROOT}" "${CANCEL_REQUEST_PARENT}" "release cancellation marker parent"

if [[ -n "${SIGNING_PAYLOAD}" ]]; then
  validate_external_payload_path "${SIGNING_PAYLOAD}" "signing payload" output
  require_release_artifact_directory "${SIGNING_PAYLOAD%/*}"
fi
if [[ -n "${SIGNATURE_ENVELOPE}" ]]; then
  validate_external_payload_path "${SIGNATURE_ENVELOPE}" "signature envelope" input
fi

if [[ -n "$(git -C "${REPO_ROOT}" status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "error: OpenAPI authority replay requires an exact clean candidate checkout." >&2
  exit 1
fi
CANDIDATE_COMMIT="$(git -C "${REPO_ROOT}" rev-parse --verify "HEAD^{commit}")"
CANDIDATE_TREE="$(git -C "${REPO_ROOT}" rev-parse --verify "${CANDIDATE_COMMIT}^{tree}")"

OPENAPI_RUN_ROOT="$(mktemp -d /private/tmp/iroha-openapi-generate.XXXXXX)"
chmod 700 "${OPENAPI_RUN_ROOT}"
SOURCE_ROOT="${OPENAPI_RUN_ROOT}/source"
CARGO_TARGET_DIR="${OPENAPI_RUN_ROOT}/target"
mkdir -m 700 "${CARGO_TARGET_DIR}"
export CARGO_TARGET_DIR
require_external_cargo_target_dir "${REPO_ROOT}"
require_disjoint_release_roots "${REPO_ROOT}"
release_gate_boundary "openapi-generator:channels-ready"
OPENAPI_GENERATOR_EVIDENCE_DIR="$(
  mktemp -d "${IROHA_RELEASE_ARTIFACT_ROOT}/openapi-generator-evidence.XXXXXX"
)"
chmod 700 "${OPENAPI_GENERATOR_EVIDENCE_DIR}"
require_release_artifact_directory "${OPENAPI_GENERATOR_EVIDENCE_DIR}"

report_openapi_generator_paths() {
  local status=$?
  trap - EXIT
  printf 'OpenAPI generator immutable source and target: %s\n' "${OPENAPI_RUN_ROOT}" >&2
  printf 'OpenAPI generator staged output: %s\n' "${OUTPUT_DIR}" >&2
  printf 'OpenAPI generator retained evidence: %s\n' \
    "${OPENAPI_GENERATOR_EVIDENCE_DIR}" >&2
  exit "${status}"
}
trap report_openapi_generator_paths EXIT
printf 'OpenAPI generator authenticated artifact root: %s\n' \
  "${IROHA_RELEASE_ARTIFACT_ROOT}" >&2
printf 'OpenAPI generator cooperative cancellation marker: %s\n' \
  "${IROHA_RELEASE_CANCEL_REQUEST_PATH}" >&2

release_gate_boundary "openapi-generator:before-source-mirror"
git clone --quiet --local --no-hardlinks --no-checkout \
  "${REPO_ROOT}" "${SOURCE_ROOT}"
git -C "${SOURCE_ROOT}" checkout --quiet --detach "${CANDIDATE_COMMIT}"

node --input-type=module - \
  "${SOURCE_ROOT}" \
  "${REPO_ROOT}/Cargo.lock" <<'NODE'
import {realpath} from 'node:fs/promises';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

const [sourceRootArgument, lockSourceArgument] = process.argv.slice(2);
const sourceRoot = await realpath(sourceRootArgument);
const sourcePath = await realpath(lockSourceArgument);
const moduleUrl = pathToFileURL(join(
  sourceRoot,
  'tools',
  'openapi',
  'scripts',
  'provision-openapi-cargo-lock.mjs',
)).href;
const {
  OPENAPI_CARGO_LOCK_EXPECTED_BYTES,
  OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX,
  OPENAPI_CARGO_LOCK_PROVISION_SCHEMA,
  provisionOpenApiCargoLock,
} = await import(moduleUrl);
const summary = await provisionOpenApiCargoLock({repoRoot: sourceRoot, sourcePath});
if (
  summary.schema !== OPENAPI_CARGO_LOCK_PROVISION_SCHEMA ||
  summary.status !== 'installed' ||
  summary.source !== 'operator' ||
  summary.path !== 'Cargo.lock' ||
  summary.bytes !== OPENAPI_CARGO_LOCK_EXPECTED_BYTES ||
  summary.sha256_hex !== OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX
) {
  throw new Error('immutable OpenAPI generator Cargo.lock provisioning was not exact');
}
NODE
if ! cmp -s "${REPO_ROOT}/Cargo.lock" "${SOURCE_ROOT}/Cargo.lock"; then
  echo "error: immutable OpenAPI generator Cargo.lock is not byte-identical." >&2
  exit 1
fi
if [[ -n "$(GIT_OPTIONAL_LOCKS=0 git -C "${SOURCE_ROOT}" status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "error: immutable OpenAPI generator source is not clean." >&2
  exit 1
fi
if [[ -e "${SOURCE_ROOT}/.git/objects/info/alternates" || \
      -L "${SOURCE_ROOT}/.git/objects/info/alternates" ]]; then
  echo "error: immutable OpenAPI generator source has a borrowed object database." >&2
  exit 1
fi
python3 -I -S "${SOURCE_ROOT}/scripts/seal_workspace_source.py" \
  --seal --root "${SOURCE_ROOT}" --no-writable-paths
python3 -I -S "${SOURCE_ROOT}/scripts/seal_workspace_source.py" \
  --verify --root "${SOURCE_ROOT}" --no-writable-paths

XTASK_ARGS=(openapi --output-root "${OUTPUT_DIR}")
if [[ "${UNSIGNED_MANIFEST}" == 1 ]]; then
  XTASK_ARGS+=(--unsigned-manifest)
fi
if [[ -n "${SIGNING_PAYLOAD}" ]]; then
  XTASK_ARGS+=(--signing-payload "${SIGNING_PAYLOAD}")
fi
if [[ -n "${SIGNATURE_ENVELOPE}" ]]; then
  XTASK_ARGS+=(--signature-envelope "${SIGNATURE_ENVELOPE}")
fi

(
  cd "${SOURCE_ROOT}"
  GIT_OPTIONAL_LOCKS=0 \
  NORITO_SKIP_BINDINGS_SYNC=1 \
    run_cargo run \
      --locked \
      --offline \
      -p xtask \
      --features dev-tools \
      --bin xtask \
      -- \
      "${XTASK_ARGS[@]}"
)

for emitted_artifact in torii.json manifest.json; do
  if [[ ! -f "${OUTPUT_DIR}/${emitted_artifact}" || \
        -L "${OUTPUT_DIR}/${emitted_artifact}" ]]; then
    printf 'error: OpenAPI authority replay omitted regular staged %s.\n' \
      "${emitted_artifact}" >&2
    exit 1
  fi
done

python3 -I -S "${SOURCE_ROOT}/scripts/seal_workspace_source.py" \
  --verify --root "${SOURCE_ROOT}" --no-writable-paths
ACTUAL_COMMIT="$(GIT_OPTIONAL_LOCKS=0 git -C "${SOURCE_ROOT}" rev-parse --verify "HEAD^{commit}")"
ACTUAL_TREE="$(GIT_OPTIONAL_LOCKS=0 git -C "${SOURCE_ROOT}" rev-parse --verify "HEAD^{tree}")"
if [[ "${ACTUAL_COMMIT}" != "${CANDIDATE_COMMIT}" || \
      "${ACTUAL_TREE}" != "${CANDIDATE_TREE}" || \
      -n "$(GIT_OPTIONAL_LOCKS=0 git -C "${SOURCE_ROOT}" status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "error: immutable OpenAPI generator source identity changed." >&2
  exit 1
fi
if ! cmp -s "${REPO_ROOT}/Cargo.lock" "${SOURCE_ROOT}/Cargo.lock"; then
  echo "error: OpenAPI generator Cargo.lock identity changed between checkpoints." >&2
  exit 1
fi
if [[ -n "$(git -C "${REPO_ROOT}" status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "error: caller checkout changed during OpenAPI generation." >&2
  exit 1
fi
release_gate_boundary "openapi-generator:before-completion-publication"
python3 -I -S - \
  "${OPENAPI_GENERATOR_EVIDENCE_DIR}/source-identity.json" \
  "${CANDIDATE_COMMIT}" \
  "${CANDIDATE_TREE}" \
  "${SOURCE_ROOT}" \
  "${OUTPUT_DIR}" <<'PY'
import json
import os
import sys

receipt, commit, tree, source, output = sys.argv[1:]
descriptor = os.open(receipt, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
    json.dump(
        {
            "candidate_commit": commit,
            "candidate_tree": tree,
            "output_directory": output,
            "schema_version": 1,
            "source_mirror": source,
        },
        stream,
        sort_keys=True,
        separators=(",", ":"),
    )
    stream.write("\n")
PY
release_gate_boundary "openapi-generator:after-completion-publication"
