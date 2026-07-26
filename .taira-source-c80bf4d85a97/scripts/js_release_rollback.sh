#!/usr/bin/env bash
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

usage() {
  cat <<'USAGE' >&2
Usage: scripts/js_release_rollback.sh --incident <ID> --bad-version <semver> [options]

Collect verification artefacts for a problematic JS SDK release, copy them into
artifacts/js/incidents/<ID>/, and emit a summary.json bundle for governance.

Required arguments:
  --incident <ID>          Incident or ticket identifier (e.g., INC-1234)
  --bad-version <semver>   Published version that triggered the rollback

Optional arguments:
  --known-good <semver>           Version tagged as the temporary latest (documents dist-tag target)
  --replacement-version <semver>  Candidate version that will replace the bad artefact
  --registry <url>                npm registry used for verification (default: https://registry.npmjs.org/)
  --staging-registry <url>        Registry forwarded to js_signed_staging.sh (default: registry)
  --notes <TEXT>                  Free-form notes stored in the summary
  --out-dir <PATH>                Override artefact root (default: artifacts/js/incidents)
  --verify-replacement            Verify --replacement-version against the registry (run after publish)
  --skip-staging                  Do not run js_signed_staging.sh for --replacement-version
  --dry-run                       Print the plan without executing npm commands
  --help                          Show this help text
USAGE
}

info() {
  echo "[js_release_rollback] $*" >&2
}

error() {
  echo "[js_release_rollback] ERROR: $*" >&2
  exit 1
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JS_DIR="${ROOT_DIR}/javascript/iroha_js"

INCIDENT_ID=""
BAD_VERSION=""
KNOWN_GOOD_VERSION=""
REPLACEMENT_VERSION=""
REGISTRY_URL="https://registry.npmjs.org/"
STAGING_REGISTRY_URL=""
NOTES_VALUE=""
OUT_DIR=""
VERIFY_REPLACEMENT=0
SKIP_STAGING=0
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --incident)
      INCIDENT_ID="$2"
      shift 2
      ;;
    --bad-version)
      BAD_VERSION="$2"
      shift 2
      ;;
    --known-good)
      KNOWN_GOOD_VERSION="$2"
      shift 2
      ;;
    --replacement-version)
      REPLACEMENT_VERSION="$2"
      shift 2
      ;;
    --registry)
      REGISTRY_URL="$2"
      shift 2
      ;;
    --staging-registry)
      STAGING_REGISTRY_URL="$2"
      shift 2
      ;;
    --notes)
      NOTES_VALUE="$2"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="$2"
      shift 2
      ;;
    --verify-replacement)
      VERIFY_REPLACEMENT=1
      shift
      ;;
    --skip-staging)
      SKIP_STAGING=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage
      error "Unknown argument: $1"
      ;;
  esac
done

[[ -n "${INCIDENT_ID}" ]] || error "missing required --incident argument"
[[ -n "${BAD_VERSION}" ]] || error "missing required --bad-version argument"

if [[ -n "${REPLACEMENT_VERSION}" && "${REPLACEMENT_VERSION}" == "${BAD_VERSION}" ]]; then
  error "--replacement-version must differ from --bad-version"
fi

if [[ -n "${KNOWN_GOOD_VERSION}" && "${KNOWN_GOOD_VERSION}" == "${BAD_VERSION}" ]]; then
  error "--known-good must differ from --bad-version"
fi

if [[ -z "${STAGING_REGISTRY_URL}" ]]; then
  STAGING_REGISTRY_URL="${REGISTRY_URL}"
fi

OUTPUT_ROOT="${OUT_DIR:-${ROOT_DIR}/artifacts/js/incidents}"
INCIDENT_DIR="${OUTPUT_ROOT}/${INCIDENT_ID}"
LOG_DIR="${INCIDENT_DIR}/logs"
EVIDENCE_DIR="${INCIDENT_DIR}/evidence"
mkdir -p "${LOG_DIR}" "${EVIDENCE_DIR}"

relpath() {
  local target="$1"
  if [[ ! -e "${target}" && ! -d "${target}" ]]; then
    echo ""
    return
  fi
  python3 - "$ROOT_DIR" "$target" <<'PY'
import os
import sys
root, target = sys.argv[1:]
print(os.path.relpath(target, root))
PY
}

run_verify() {
  local version="$1"
  local log_path="$2"
  local label="$3"
  info "Verifying ${label} release ${version} against ${REGISTRY_URL}"
  if (( DRY_RUN )); then
    printf '[dry-run] npm run release:verify -- --version %s --registry %s\n' \
      "${version}" "${REGISTRY_URL}" | tee "${log_path}" >&2
    echo 0
    return 0
  fi
  pushd "${JS_DIR}" >/dev/null
  set +e
  npm run release:verify -- --version "${version}" --registry "${REGISTRY_URL}" \
    2>&1 | tee "${log_path}" >&2
  local status=${PIPESTATUS[0]}
  set -e
  popd >/dev/null
  if [[ ${status} -ne 0 ]]; then
    info "Verification for ${label} (${version}) exited with status ${status}"
  fi
  echo "${status}"
  return "${status}"
}

copy_verification_artifacts() {
  local version="$1"
  local dest_dir="$2"
  local label="$3"
  if (( DRY_RUN )); then
    info "[dry-run] would copy verification artefacts for ${label} (${version})"
    return 0
  fi
  local latest
  latest="$(ls -td "${ROOT_DIR}/artifacts/js/verification/v${version}_"* 2>/dev/null | head -n 1 || true)"
  if [[ -z "${latest}" ]]; then
    info "No verification artefacts found for ${label} (${version})"
    return 1
  fi
  rm -rf "${dest_dir}"
  mkdir -p "${dest_dir}"
  cp -R "${latest}/." "${dest_dir}/"
  info "Copied verification artefacts for ${label} (${version}) into ${dest_dir}"
}

stage_replacement() {
  local version="$1"
  local dest_dir="$2"
  info "Staging replacement release ${version} via js_signed_staging.sh"
  if (( DRY_RUN )); then
    info "[dry-run] would run js_signed_staging.sh ${version}"
    return 0
  fi
  NPM_STAGING_REGISTRY="${STAGING_REGISTRY_URL}" \
    "${ROOT_DIR}/scripts/js_signed_staging.sh" "${version}"
  local staging_src="${ROOT_DIR}/artifacts/js/npm_staging/${version}"
  if [[ ! -d "${staging_src}" ]]; then
    info "Staging artefacts not found at ${staging_src}"
    return 1
  fi
  rm -rf "${dest_dir}"
  mkdir -p "${dest_dir}"
  cp -R "${staging_src}/." "${dest_dir}/"
  info "Copied staging artefacts into ${dest_dir}"
}

verify_replacement_against_registry() {
  local version="$1"
  local log_path="$2"
  info "Verifying replacement release ${version} against ${REGISTRY_URL}"
  if (( DRY_RUN )); then
    printf '[dry-run] npm run release:verify -- --version %s --registry %s\n' \
      "${version}" "${REGISTRY_URL}" | tee "${log_path}" >&2
    echo 0
    return 0
  fi
  pushd "${JS_DIR}" >/dev/null
  set +e
  npm run release:verify -- --version "${version}" --registry "${REGISTRY_URL}" \
    2>&1 | tee "${log_path}" >&2
  local status=${PIPESTATUS[0]}
  set -e
  popd >/dev/null
  if [[ ${status} -ne 0 ]]; then
    info "Replacement verification exited with status ${status}"
  fi
  echo "${status}"
  return "${status}"
}

BAD_VERIFY_LOG="${LOG_DIR}/verify_bad.log"
BAD_STATUS_STR="$(run_verify "${BAD_VERSION}" "${BAD_VERIFY_LOG}" "bad")"
BAD_VERIFY_STATUS="${BAD_STATUS_STR:-0}"
BAD_ARTIFACT_DIR="${EVIDENCE_DIR}/verify_bad"
copy_verification_artifacts "${BAD_VERSION}" "${BAD_ARTIFACT_DIR}" "bad" || true

KNOWN_GOOD_LOG=""
KNOWN_GOOD_STATUS=""
KNOWN_GOOD_ARTIFACT_DIR=""
if [[ -n "${KNOWN_GOOD_VERSION}" ]]; then
  KNOWN_GOOD_LOG="${LOG_DIR}/verify_known_good.log"
  KNOWN_STATUS_STR="$(run_verify "${KNOWN_GOOD_VERSION}" "${KNOWN_GOOD_LOG}" "known-good")"
  KNOWN_GOOD_STATUS="${KNOWN_STATUS_STR:-0}"
  KNOWN_GOOD_ARTIFACT_DIR="${EVIDENCE_DIR}/verify_known_good"
  copy_verification_artifacts "${KNOWN_GOOD_VERSION}" "${KNOWN_GOOD_ARTIFACT_DIR}" "known-good" || true
fi

REPLACEMENT_STAGE_DIR=""
if [[ -n "${REPLACEMENT_VERSION}" && ${SKIP_STAGING} -eq 0 ]]; then
  REPLACEMENT_STAGE_DIR="${EVIDENCE_DIR}/staging_${REPLACEMENT_VERSION}"
  stage_replacement "${REPLACEMENT_VERSION}" "${REPLACEMENT_STAGE_DIR}" || true
fi

REPLACEMENT_VERIFY_LOG=""
REPLACEMENT_VERIFY_STATUS=""
REPLACEMENT_VERIFY_DIR=""
if [[ -n "${REPLACEMENT_VERSION}" && ${VERIFY_REPLACEMENT} -eq 1 ]]; then
  REPLACEMENT_VERIFY_LOG="${LOG_DIR}/verify_replacement.log"
  REPLACEMENT_STATUS_STR="$(verify_replacement_against_registry "${REPLACEMENT_VERSION}" "${REPLACEMENT_VERIFY_LOG}")"
  REPLACEMENT_VERIFY_STATUS="${REPLACEMENT_STATUS_STR:-0}"
  REPLACEMENT_VERIFY_DIR="${EVIDENCE_DIR}/verify_replacement"
  copy_verification_artifacts "${REPLACEMENT_VERSION}" "${REPLACEMENT_VERIFY_DIR}" "replacement" || true
fi

SUMMARY_PATH="${INCIDENT_DIR}/summary.json"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

BAD_LOG_REL="$(relpath "${BAD_VERIFY_LOG}")"
BAD_ARTIFACT_REL="$(relpath "${BAD_ARTIFACT_DIR}")"
KNOWN_LOG_REL="$(relpath "${KNOWN_GOOD_LOG}")"
KNOWN_ARTIFACT_REL="$(relpath "${KNOWN_GOOD_ARTIFACT_DIR}")"
REPLACEMENT_STAGE_REL="$(relpath "${REPLACEMENT_STAGE_DIR}")"
REPLACEMENT_LOG_REL="$(relpath "${REPLACEMENT_VERIFY_LOG}")"
REPLACEMENT_ARTIFACT_REL="$(relpath "${REPLACEMENT_VERIFY_DIR}")"

RECOMMENDED_ACTIONS_STR=""
if [[ -n "${KNOWN_GOOD_VERSION}" ]]; then
  RECOMMENDED_ACTIONS_STR+="npm dist-tag add @iroha/iroha-js@${KNOWN_GOOD_VERSION} latest"
fi

if [[ -n "${RECOMMENDED_ACTIONS_STR}" ]]; then
  RECOMMENDED_ACTIONS_STR+="||"
fi
RECOMMENDED_ACTIONS_STR+="npm deprecate @iroha/iroha-js@${BAD_VERSION} \"${INCIDENT_ID} rollback\""

if [[ -n "${REPLACEMENT_VERSION}" ]]; then
  if [[ -n "${RECOMMENDED_ACTIONS_STR}" ]]; then
    RECOMMENDED_ACTIONS_STR+="||"
  fi
  RECOMMENDED_ACTIONS_STR+="npm dist-tag add @iroha/iroha-js@${REPLACEMENT_VERSION} latest"
fi

export INCIDENT_ID BAD_VERSION KNOWN_GOOD_VERSION REPLACEMENT_VERSION
export BAD_VERIFY_STATUS KNOWN_GOOD_STATUS REPLACEMENT_VERIFY_STATUS
export NOTES_VALUE TIMESTAMP REGISTRY_URL DRY_RUN VERIFY_REPLACEMENT
export BAD_LOG_REL BAD_ARTIFACT_REL KNOWN_LOG_REL KNOWN_ARTIFACT_REL
export REPLACEMENT_STAGE_REL REPLACEMENT_LOG_REL REPLACEMENT_ARTIFACT_REL
export RECOMMENDED_ACTIONS="${RECOMMENDED_ACTIONS_STR}"

python3 - "$SUMMARY_PATH" <<PY
import json
import os
import sys

summary_path = sys.argv[1]
summary = {
    "incident_id": os.environ.get("INCIDENT_ID"),
    "timestamp": os.environ.get("TIMESTAMP"),
    "registry": os.environ.get("REGISTRY_URL"),
    "notes": os.environ.get("NOTES_VALUE") or None,
    "dry_run": bool(int(os.environ.get("DRY_RUN", "0"))),
    "bad_version": {
        "version": os.environ.get("BAD_VERSION"),
        "verify_exit_status": int(os.environ.get("BAD_VERIFY_STATUS", "0")),
        "log": os.environ.get("BAD_LOG_REL") or None,
        "evidence_dir": os.environ.get("BAD_ARTIFACT_REL") or None,
    },
    "recommended_actions": [
        cmd for cmd in os.environ.get("RECOMMENDED_ACTIONS", "").split("||") if cmd
    ],
}

known_good_version = os.environ.get("KNOWN_GOOD_VERSION")
if known_good_version:
    summary["known_good_version"] = {
        "version": known_good_version,
        "verify_exit_status": int(os.environ.get("KNOWN_GOOD_STATUS", "0")),
        "log": os.environ.get("KNOWN_LOG_REL") or None,
        "evidence_dir": os.environ.get("KNOWN_ARTIFACT_REL") or None,
    }

replacement_version = os.environ.get("REPLACEMENT_VERSION")
if replacement_version:
    replacement = {
        "version": replacement_version,
        "staging_dir": os.environ.get("REPLACEMENT_STAGE_REL") or None,
    }
    if os.environ.get("VERIFY_REPLACEMENT") == "1":
        replacement["verify_exit_status"] = int(os.environ.get("REPLACEMENT_VERIFY_STATUS", "0"))
        replacement["log"] = os.environ.get("REPLACEMENT_LOG_REL") or None
        replacement["evidence_dir"] = os.environ.get("REPLACEMENT_ARTIFACT_REL") or None
    summary["replacement_version"] = replacement

with open(summary_path, "w", encoding="utf8") as handle:
    json.dump(summary, handle, indent=2)
    handle.write("\n")
PY

SUMMARY_REL="$(relpath "${SUMMARY_PATH}")"
INCIDENT_REL="$(relpath "${INCIDENT_DIR}")"

info "Summary written to ${SUMMARY_REL}"
info "Incident artefacts stored under ${INCIDENT_REL}"
