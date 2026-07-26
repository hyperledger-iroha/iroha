#!/usr/bin/env bash
set -euo pipefail

usage() {
cat <<'USAGE'
Usage: package_docs_portal_sorafs.sh [options]

Builds the developer portal, regenerates OpenAPI + Norito artefacts, emits
CycloneDX SBOMs, and packages the portal, OpenAPI, and SBOM payloads into
SoraFS-ready CARs + content manifests. Release authenticity is applied later to
the aggregate release manifest through the governed Ed25519/HSM release path.

Options:
  --out DIR                   Output directory (default artifacts/devportal/sorafs/<timestamp>)
  --portal-dir PATH           Path to the portal sources (default docs/portal)
  --build-dir PATH            Portal build directory (default docs/portal/build)
  --openapi-dir PATH          Directory containing the OpenAPI payload (default docs/portal/static/openapi)
  --openapi-file PATH         OpenAPI JSON file for SBOM generation (default <openapi-dir>/torii.json)
  --chunker-handle HANDLE     Chunker handle used for car pack (default sorafs.sf1@1.0.0)
  --pin-min-replicas N        Pin policy min replicas (default 5)
  --pin-storage-class CLASS   Pin storage class hot|warm|cold (default warm)
  --pin-retention-epoch N     Pin retention epoch (default 14)
  --sorafs-cli PATH           Use an already-built sorafs_cli binary
  --skip-build                Reuse existing build artifacts (skips npm install/build/tests)
  --skip-sync-openapi         Skip npm run sync-openapi (for offline environments)
  --skip-sbom                 Skip SBOM generation/packaging steps
  --proof                     Run sorafs_cli proof verify for each artifact
  -h, --help                  Show this help text

Examples:
  ./ci/package_docs_portal_sorafs.sh
  ./ci/package_docs_portal_sorafs.sh --proof
  ./ci/package_docs_portal_sorafs.sh --skip-build --skip-sbom --out artifacts/devportal/sorafs/manual

This packager does not sign release artifacts. Pass its outputs into the
canonical release pipeline, then sign the aggregate release manifest with
`scripts/release_sorafs_cli.sh`.
USAGE
}

log() {
    printf '[docs-sorafs] %s\n' "$*" >&2
}

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log "error: $1 is required for this script"
        exit 1
    fi
}

abs_path() {
    python3 - "${1:-}" <<'PY'
import os, sys
if len(sys.argv) != 2:
    sys.exit("expected 1 argument to abs_path helper")
print(os.path.abspath(sys.argv[1]))
PY
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"

OUT_DIR=""
PORTAL_DIR_REL="docs/portal"
BUILD_DIR_REL="docs/portal/build"
OPENAPI_DIR_REL="docs/portal/static/openapi"
OPENAPI_FILE_REL="docs/portal/static/openapi/torii.json"
CHUNKER_HANDLE="${DOCS_PORTAL_CHUNKER_HANDLE:-sorafs.sf1@1.0.0}"
PIN_MIN="${DOCS_PORTAL_PIN_MIN_REPLICAS:-5}"
PIN_STORAGE="${DOCS_PORTAL_PIN_STORAGE_CLASS:-warm}"
PIN_RETENTION="${DOCS_PORTAL_PIN_RETENTION_EPOCH:-14}"
SORA_CLI_BIN="${DOCS_PORTAL_SORAFS_CLI:-}"
SKIP_BUILD=0
GENERATE_SBOM=1
RUN_SYNC_OPENAPI=1
RUN_PROOF=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --out)
            OUT_DIR="$2"
            shift 2
            ;;
        --portal-dir)
            PORTAL_DIR_REL="$2"
            shift 2
            ;;
        --build-dir)
            BUILD_DIR_REL="$2"
            shift 2
            ;;
        --openapi-dir)
            OPENAPI_DIR_REL="$2"
            shift 2
            ;;
        --openapi-file)
            OPENAPI_FILE_REL="$2"
            shift 2
            ;;
        --chunker-handle)
            CHUNKER_HANDLE="$2"
            shift 2
            ;;
        --pin-min-replicas)
            PIN_MIN="$2"
            shift 2
            ;;
        --pin-storage-class)
            PIN_STORAGE="$2"
            shift 2
            ;;
        --pin-retention-epoch)
            PIN_RETENTION="$2"
            shift 2
            ;;
        --sorafs-cli)
            SORA_CLI_BIN="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --skip-sync-openapi)
            RUN_SYNC_OPENAPI=0
            shift
            ;;
        --skip-sbom)
            GENERATE_SBOM=0
            shift
            ;;
        --proof)
            RUN_PROOF=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log "error: unknown option $1"
            usage >&2
            exit 1
            ;;
    esac
done

require_tool python3

PORTAL_DIR="$(abs_path "${PORTAL_DIR_REL}")"
BUILD_DIR="$(abs_path "${BUILD_DIR_REL}")"
OPENAPI_DIR="$(abs_path "${OPENAPI_DIR_REL}")"
OPENAPI_FILE="$(abs_path "${OPENAPI_FILE_REL}")"

if [[ -z "$OUT_DIR" ]]; then
    OUT_DIR="${REPO_ROOT}/artifacts/devportal/sorafs/${TIMESTAMP}"
fi
OUT_DIR="$(abs_path "$OUT_DIR")"
mkdir -p "$OUT_DIR"
mkdir -p "${REPO_ROOT}/artifacts/devportal/sorafs"
ln -sfn "$OUT_DIR" "${REPO_ROOT}/artifacts/devportal/sorafs/latest"

if [[ "$SKIP_BUILD" -eq 0 ]]; then
    require_tool npm
    pushd "$PORTAL_DIR" >/dev/null
    if [[ -n "${CI:-}" ]]; then
        npm ci
    else
        npm install
    fi
    if [[ "$RUN_SYNC_OPENAPI" -eq 1 ]]; then
        npm run sync-openapi
    else
        log "skipping npm run sync-openapi (--skip-sync-openapi supplied)"
    fi
    npm run sync-norito-snippets
    npm run test:norito-snippets
    npm run test:widgets
    npm run build
    popd >/dev/null
else
    log "skipping portal build (--skip-build supplied)"
fi

if [[ ! -d "$BUILD_DIR" ]]; then
    log "error: build directory not found at $BUILD_DIR"
    exit 1
fi

if [[ ! -d "$OPENAPI_DIR" ]]; then
    log "error: OpenAPI directory not found at $OPENAPI_DIR"
    exit 1
fi

if [[ ! -f "$OPENAPI_FILE" ]]; then
    log "error: OpenAPI file not found at $OPENAPI_FILE"
    exit 1
fi

PORTAL_SBOM=""
OPENAPI_SBOM=""
if [[ "$GENERATE_SBOM" -eq 1 ]]; then
    require_tool syft
    PORTAL_SBOM="${OUT_DIR}/portal.sbom.json"
    OPENAPI_SBOM="${OUT_DIR}/openapi.sbom.json"
    log "generating SBOM for portal build"
    syft "dir:${BUILD_DIR}" -o json >"$PORTAL_SBOM"
    log "generating SBOM for OpenAPI spec"
    syft "file:${OPENAPI_FILE}" -o json >"$OPENAPI_SBOM"
else
    log "skipping SBOM generation (--skip-sbom supplied)"
fi

if [[ -z "$SORA_CLI_BIN" ]]; then
    require_tool cargo
    log "building sorafs_cli (debug)"
    pushd "$REPO_ROOT" >/dev/null
    cargo build -p sorafs_orchestrator --bin sorafs_cli
    popd >/dev/null
    SORA_CLI_BIN="${REPO_ROOT}/target/debug/sorafs_cli"
fi
SORA_CLI_BIN="$(abs_path "$SORA_CLI_BIN")"
if [[ ! -x "$SORA_CLI_BIN" ]]; then
    log "error: sorafs_cli binary not found or not executable at $SORA_CLI_BIN"
    exit 1
fi

run_cli() {
    log "sorafs_cli $*"
    "$SORA_CLI_BIN" "$@"
}

SUMMARY_TMP="$(mktemp)"
trap 'rm -f "$SUMMARY_TMP"' EXIT
: >"$SUMMARY_TMP"

pack_payload() {
    local label="$1"
    local input_path="$2"
    local car_path="$3"
    local plan_path="$4"
    local summary_path="$5"
    local manifest_path="$6"
    local manifest_json="$7"
    local proof_path="$8"

    log "packing ${label} payload from ${input_path}"
    local car_args=(
        "car" "pack"
        "--input=${input_path}"
        "--car-out=${car_path}"
        "--plan-out=${plan_path}"
        "--summary-out=${summary_path}"
    )
    if [[ -n "$CHUNKER_HANDLE" ]]; then
        car_args+=("--chunker-handle=${CHUNKER_HANDLE}")
    fi
    run_cli "${car_args[@]}"

    log "building manifest for ${label}"
    run_cli \
        "manifest" "build" \
        "--summary=${summary_path}" \
        "--manifest-out=${manifest_path}" \
        "--manifest-json-out=${manifest_json}" \
        "--pin-min-replicas=${PIN_MIN}" \
        "--pin-storage-class=${PIN_STORAGE}" \
        "--pin-retention-epoch=${PIN_RETENTION}"

    local proof_summary=""
    if [[ "$RUN_PROOF" -eq 1 ]]; then
        log "verifying CAR for ${label}"
        run_cli \
            "proof" "verify" \
            "--manifest=${manifest_path}" \
            "--car=${car_path}" \
            "--summary-out=${proof_path}"
        proof_summary="${proof_path}"
    else
        log "skipping proof verification for ${label} (--proof not supplied)"
    fi

    printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
        "$label" "$input_path" "$car_path" "$plan_path" "$summary_path" \
        "$manifest_path" "$manifest_json" "$proof_summary" \
        >>"$SUMMARY_TMP"
}

PORTAL_CAR="${OUT_DIR}/portal.car"
PORTAL_PLAN="${OUT_DIR}/portal.plan.json"
PORTAL_SUMMARY="${OUT_DIR}/portal.car.json"
PORTAL_MANIFEST="${OUT_DIR}/portal.manifest.to"
PORTAL_MANIFEST_JSON="${OUT_DIR}/portal.manifest.json"
PORTAL_PROOF="${OUT_DIR}/portal.proof.json"

pack_payload \
    "portal" \
    "$BUILD_DIR" \
    "$PORTAL_CAR" \
    "$PORTAL_PLAN" \
    "$PORTAL_SUMMARY" \
    "$PORTAL_MANIFEST" \
    "$PORTAL_MANIFEST_JSON" \
    "$PORTAL_PROOF"

OPENAPI_CAR="${OUT_DIR}/openapi.car"
OPENAPI_PLAN="${OUT_DIR}/openapi.plan.json"
OPENAPI_SUMMARY="${OUT_DIR}/openapi.car.json"
OPENAPI_MANIFEST="${OUT_DIR}/openapi.manifest.to"
OPENAPI_MANIFEST_JSON="${OUT_DIR}/openapi.manifest.json"
OPENAPI_PROOF="${OUT_DIR}/openapi.proof.json"

pack_payload \
    "openapi" \
    "$OPENAPI_DIR" \
    "$OPENAPI_CAR" \
    "$OPENAPI_PLAN" \
    "$OPENAPI_SUMMARY" \
    "$OPENAPI_MANIFEST" \
    "$OPENAPI_MANIFEST_JSON" \
    "$OPENAPI_PROOF"

if [[ -n "$PORTAL_SBOM" && -s "$PORTAL_SBOM" ]]; then
    PORTAL_SBOM_CAR="${OUT_DIR}/portal.sbom.car"
    PORTAL_SBOM_PLAN="${OUT_DIR}/portal.sbom.plan.json"
    PORTAL_SBOM_SUMMARY="${OUT_DIR}/portal.sbom.car.json"
    PORTAL_SBOM_MANIFEST="${OUT_DIR}/portal.sbom.manifest.to"
    PORTAL_SBOM_MANIFEST_JSON="${OUT_DIR}/portal.sbom.manifest.json"
    PORTAL_SBOM_PROOF="${OUT_DIR}/portal.sbom.proof.json"

    pack_payload \
        "portal-sbom" \
        "$PORTAL_SBOM" \
        "$PORTAL_SBOM_CAR" \
        "$PORTAL_SBOM_PLAN" \
        "$PORTAL_SBOM_SUMMARY" \
        "$PORTAL_SBOM_MANIFEST" \
        "$PORTAL_SBOM_MANIFEST_JSON" \
        "$PORTAL_SBOM_PROOF"
else
    log "portal SBOM skipped; no SBOM file present"
fi

if [[ -n "$OPENAPI_SBOM" && -s "$OPENAPI_SBOM" ]]; then
    OPENAPI_SBOM_CAR="${OUT_DIR}/openapi.sbom.car"
    OPENAPI_SBOM_PLAN="${OUT_DIR}/openapi.sbom.plan.json"
    OPENAPI_SBOM_SUMMARY="${OUT_DIR}/openapi.sbom.car.json"
    OPENAPI_SBOM_MANIFEST="${OUT_DIR}/openapi.sbom.manifest.to"
    OPENAPI_SBOM_MANIFEST_JSON="${OUT_DIR}/openapi.sbom.manifest.json"
    OPENAPI_SBOM_PROOF="${OUT_DIR}/openapi.sbom.proof.json"

    pack_payload \
        "openapi-sbom" \
        "$OPENAPI_SBOM" \
        "$OPENAPI_SBOM_CAR" \
        "$OPENAPI_SBOM_PLAN" \
        "$OPENAPI_SBOM_SUMMARY" \
        "$OPENAPI_SBOM_MANIFEST" \
        "$OPENAPI_SBOM_MANIFEST_JSON" \
        "$OPENAPI_SBOM_PROOF"
else
    log "openapi SBOM skipped; no SBOM file present"
fi

PACKAGE_SUMMARY="${OUT_DIR}/package_summary.json"
python3 - "$SUMMARY_TMP" "$OUT_DIR" "$REPO_ROOT" "$TIMESTAMP" >"$PACKAGE_SUMMARY" <<'PY'
import json, os, stat, sys

summary_path, out_dir, repo_root, generated_at = sys.argv[1:5]

def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)

def read_summary_lines_no_follow(path: str) -> list[str]:
    if os.path.islink(path):
        raise SystemExit(f"package summary input must not be a symlink: {path}")
    try:
        path_stat = os.lstat(path)
    except FileNotFoundError:
        raise SystemExit(f"package summary input missing: {path}")
    if not stat.S_ISREG(path_stat.st_mode):
        raise SystemExit(f"package summary input must be a regular file: {path}")
    fd = os.open(path, read_open_flags())
    try:
        descriptor_stat = os.fstat(fd)
        if not stat.S_ISREG(descriptor_stat.st_mode):
            raise SystemExit(f"package summary input must be a regular file: {path}")
        with os.fdopen(fd, "r", encoding="utf-8") as handle:
            fd = -1
            return handle.readlines()
    finally:
        if fd >= 0:
            os.close(fd)

def maybe_rel(path):
    if not path:
        return None
    try:
        return os.path.relpath(path, repo_root)
    except ValueError:
        return path

artifacts = []
for raw in read_summary_lines_no_follow(summary_path):
    raw = raw.rstrip("\n")
    if not raw:
        continue
    (
        label,
        input_path,
        car_path,
        plan_path,
        summary_out,
        manifest_path,
        manifest_json,
        proof_path,
    ) = raw.split("\t")
    artifacts.append(
        {
            "name": label,
            "input": maybe_rel(input_path),
            "car": maybe_rel(car_path),
            "plan": maybe_rel(plan_path),
            "car_summary": maybe_rel(summary_out),
            "manifest": maybe_rel(manifest_path),
            "manifest_json": maybe_rel(manifest_json),
            "proof": maybe_rel(proof_path),
        }
    )

output = {
    "generated_at": generated_at,
    "workspace_root": repo_root,
    "output_dir": out_dir,
    "artifacts": artifacts,
}
json.dump(output, sys.stdout, indent=2)
sys.stdout.write("\n")
PY

log "packaging complete; artefacts stored in ${OUT_DIR}"
log "summary written to ${PACKAGE_SUMMARY}"
