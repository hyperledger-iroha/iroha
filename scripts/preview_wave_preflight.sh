#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Helper script that automates the docs portal preview preflight sequence:
#   1. Build the portal with a specific release tag (writes checksums).
#   2. Run the checksum verifier (and optional descriptor/archive checks).
#   3. Probe a staging URL to confirm the served release tag.
#   4. Run the sitemap/link checker.
#   5. (Optional) Update the Try it proxy target for the upcoming wave.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." >/dev/null 2>&1 && pwd)"
PORTAL_DIR="${REPO_ROOT}/docs/portal"
PREVIEW_TAG=""
BASE_URL=""
DESCRIPTOR_PATH=""
ARCHIVE_PATH=""
TRYIT_TARGET=""
TRYIT_ENV=""
OUTPUT_JSON=""
SKIP_BUILD=false
SKIP_PROBE=false
SKIP_LINKS=false
SKIP_PROXY=false

usage() {
  cat <<'EOF'
Usage: scripts/preview_wave_preflight.sh --tag <preview-tag> [options]

Options:
  --tag TAG                 Preview release tag (required; exported as DOCS_RELEASE_TAG).
  --portal-dir PATH         Override docs portal directory (default: docs/portal).
  --descriptor PATH         Optional preview descriptor JSON to validate.
  --archive PATH            Optional preview archive to checksum (requires --descriptor).
  --base-url URL            Run the portal probe against URL (skip with --skip-probe).
  --tryit-target URL        Update the Try it proxy target via `npm run manage:tryit-proxy`.
  --tryit-env PATH          Custom .env file for the proxy manager (default .env.tryit-proxy).
  --output-json PATH        Write a JSON summary of executed steps to PATH.
  --skip-build              Do not run `npm run build` (assumes build/ is already present).
  --skip-probe              Skip the portal probe even if --base-url is provided.
  --skip-links              Skip `npm run check:links`.
  --skip-proxy              Skip Try it proxy updates even if --tryit-target is provided.
  -h, --help                Show this help text.
EOF
}

abs_path() {
  local target="$1"
  if [[ -z "$target" ]]; then
    echo ""
  else
    cd -- "$(dirname -- "$target")" >/dev/null 2>&1 && {
      local dir
      dir="$(pwd)"
      echo "${dir}/$(basename -- "$target")"
    }
  fi
}

while (($#)); do
  case "$1" in
    --tag)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      PREVIEW_TAG="$2"
      shift 2
      ;;
    --portal-dir)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      PORTAL_DIR="$(abs_path "$2")"
      shift 2
      ;;
    --descriptor)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      DESCRIPTOR_PATH="$(abs_path "$2")"
      shift 2
      ;;
    --archive)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      ARCHIVE_PATH="$(abs_path "$2")"
      shift 2
      ;;
    --base-url)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      BASE_URL="$2"
      shift 2
      ;;
    --tryit-target)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      TRYIT_TARGET="$2"
      shift 2
      ;;
    --tryit-env)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      TRYIT_ENV="$(abs_path "$2")"
      shift 2
      ;;
    --output-json)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      OUTPUT_JSON="$(abs_path "$2")"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=true
      shift
      ;;
    --skip-probe)
      SKIP_PROBE=true
      shift
      ;;
    --skip-links)
      SKIP_LINKS=true
      shift
      ;;
    --skip-proxy)
      SKIP_PROXY=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

[[ -n "$PREVIEW_TAG" ]] || { echo "error: --tag is required" >&2; usage; exit 1; }
[[ -d "$PORTAL_DIR" ]] || { echo "error: portal directory not found at $PORTAL_DIR" >&2; exit 1; }

BUILD_DIR="${PORTAL_DIR}/build"
VERIFY_SCRIPT="${PORTAL_DIR}/scripts/preview_verify.sh"
[[ -x "$VERIFY_SCRIPT" ]] || { echo "error: verifier not found at $VERIFY_SCRIPT" >&2; exit 1; }

step_log=()

run_portal() {
  (cd "$PORTAL_DIR" && "$@")
}

record_step() {
  step_log+=("$1")
  echo "[preview-preflight] $1"
}

if ! $SKIP_BUILD; then
  record_step "Building portal (DOCS_RELEASE_TAG=${PREVIEW_TAG})"
  DOCS_RELEASE_TAG="$PREVIEW_TAG" run_portal npm run build
else
  record_step "Skipping build step (requested)"
fi

verify_args=(--build-dir "$BUILD_DIR")
[[ -n "$DESCRIPTOR_PATH" ]] && verify_args+=(--descriptor "$DESCRIPTOR_PATH")
[[ -n "$ARCHIVE_PATH" ]] && verify_args+=(--archive "$ARCHIVE_PATH")
record_step "Verifying checksums (${verify_args[*]})"
bash "$VERIFY_SCRIPT" "${verify_args[@]}"

if [[ -n "$BASE_URL" && $SKIP_PROBE == false ]]; then
  record_step "Running portal probe against ${BASE_URL}"
  PORTAL_BASE_URL="$BASE_URL" DOCS_RELEASE_TAG="$PREVIEW_TAG" \
    run_portal npm run probe:portal -- --expect-release="${PREVIEW_TAG}"
elif [[ -n "$BASE_URL" ]]; then
  record_step "Probe skipped (requested) for ${BASE_URL}"
else
  record_step "Probe skipped (no --base-url provided)"
fi

if ! $SKIP_LINKS; then
  record_step "Running link checker"
  DOCS_RELEASE_TAG="$PREVIEW_TAG" run_portal npm run check:links
else
  record_step "Link checker skipped (requested)"
fi

if [[ -n "$TRYIT_TARGET" && $SKIP_PROXY == false ]]; then
  record_step "Updating Try it proxy target -> ${TRYIT_TARGET}"
  env_args=()
  [[ -n "$TRYIT_ENV" ]] && env_args+=(--env "$TRYIT_ENV")
  run_portal npm run manage:tryit-proxy -- update --target "${TRYIT_TARGET}" "${env_args[@]}"
elif [[ -n "$TRYIT_TARGET" ]]; then
  record_step "Try it proxy update skipped (requested)"
else
  record_step "Try it proxy update skipped (no --tryit-target provided)"
fi

if [[ -n "$OUTPUT_JSON" ]]; then
  record_step "Writing summary to ${OUTPUT_JSON}"
  mkdir -p -- "$(dirname -- "$OUTPUT_JSON")"
  {
    printf '{\n'
    printf '  "tag": "%s",\n' "$PREVIEW_TAG"
    printf '  "portal_dir": "%s",\n' "$PORTAL_DIR"
    printf '  "base_url": "%s",\n' "$BASE_URL"
    printf '  "descriptor": "%s",\n' "$DESCRIPTOR_PATH"
    printf '  "archive": "%s",\n' "$ARCHIVE_PATH"
    printf '  "tryit_target": "%s",\n' "$TRYIT_TARGET"
    printf '  "timestamp": "%s",\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    printf '  "steps": [\n'
    for i in "${!step_log[@]}"; do
      printf '    "%s"%s\n' "${step_log[$i]}" $([[ $i -lt $(( ${#step_log[@]} - 1 )) ]] && echo ',' || echo '')
    done
    printf '  ]\n}\n'
  } >"$OUTPUT_JSON"
fi

record_step "Preview preflight complete"
