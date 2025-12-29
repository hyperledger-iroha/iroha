#!/usr/bin/env bash
set -euo pipefail

SIGNING_KEY_PATH="${PYTHON_RELEASE_SIGNING_KEY:-}"
MANIFEST_OUT=""
SIGSTORE_MODE="${PYTHON_RELEASE_SIGSTORE_MODE:-auto}"
SIGSTORE_TOKEN_ENV="${PYTHON_RELEASE_SIGSTORE_TOKEN_ENV:-SIGSTORE_ID_TOKEN}"
COSIGN_BIN_DEFAULT="${COSIGN:-cosign}"
COSIGN_BIN="${PYTHON_RELEASE_SIGSTORE_COSIGN_BIN:-${COSIGN_BIN_DEFAULT}}"

while (($#)); do
    case "$1" in
        --signing-key)
            SIGNING_KEY_PATH="${2:-}"
            shift 2
            ;;
        --manifest-out)
            MANIFEST_OUT="${2:-}"
            shift 2
            ;;
        --sigstore-token-env)
            SIGSTORE_TOKEN_ENV="${2:-SIGSTORE_ID_TOKEN}"
            shift 2
            ;;
        --require-sigstore)
            SIGSTORE_MODE="require"
            shift
            ;;
        --skip-sigstore)
            SIGSTORE_MODE="skip"
            shift
            ;;
        --cosign-bin)
            COSIGN_BIN="${2:-${COSIGN_BIN}}"
            shift 2
            ;;
        *)
            printf 'Unknown argument: %s\n' "$1" >&2
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PYTHON_DIR="${PROJECT_ROOT}/python/iroha_python"
DIST_DIR="${PYTHON_DIR}/dist"
KEEP_DIST="${PYTHON_RELEASE_SMOKE_KEEP_DIST:-}"
EPHEMERAL_SIGNING_KEY=false

TMPDIR="$(mktemp -d)"
cleanup() {
    rm -rf "${TMPDIR}"
    if [[ -z "${KEEP_DIST}" ]]; then
        rm -rf "${DIST_DIR}"
    fi
    if [[ "${EPHEMERAL_SIGNING_KEY}" == "true" && -n "${SIGNING_KEY_PATH:-}" ]]; then
        rm -f "${SIGNING_KEY_PATH}"
    fi
}
trap cleanup EXIT

pushd "${PYTHON_DIR}" >/dev/null

python -m pip install --upgrade pip setuptools wheel build twine >/dev/null
python -m build >/dev/null

WHEEL="$(ls "${DIST_DIR}"/*.whl | head -n 1)"
WHEEL_NAME="$(basename "${WHEEL}")"
popd >/dev/null

python -m venv "${TMPDIR}/venv"
source "${TMPDIR}/venv/bin/activate"
pip install --upgrade pip >/dev/null
pip install "${WHEEL}" >/dev/null
pip install pytest >/dev/null

python - <<'PY'
import iroha_python as sdk

print(f"iroha_python version: {sdk.__version__}")
assert hasattr(sdk, "ToriiClient")
PY

PYTHON_BIN="${TMPDIR}/venv/bin/python"
(
    cd "${PROJECT_ROOT}"
    PYTHON_BIN="${PYTHON_BIN}" python/iroha_python/scripts/run_norito_rpc_smoke.sh
)

# Verify metadata and perform a dry-run upload with twine. Dummy credentials allow the upload
# pipeline to execute without talking to PyPI.
python -m twine check "${DIST_DIR}"/* >/dev/null
TWINE_USERNAME="__token__" TWINE_PASSWORD="pypi-dry-run-token" \
    python -m twine upload --repository-url https://upload.pypi.org/legacy/ --dry-run "${DIST_DIR}"/* >/dev/null

# Produce a changelog preview summarising commits since the latest tag. This keeps release notes
# close to the smoke artefacts so release managers can review them in one place.
CHANGELOG_PREVIEW="${DIST_DIR}/CHANGELOG_PREVIEW.md"
python - <<'PY' > "${CHANGELOG_PREVIEW}"
import subprocess
from datetime import datetime, timezone

def latest_tag() -> str | None:
    try:
        return (
            subprocess.check_output(
                ["git", "describe", "--tags", "--abbrev=0"],
                stderr=subprocess.DEVNULL,
                text=True,
            ).strip()
        )
    except subprocess.CalledProcessError:
        return None

tag = latest_tag()
rev_range = f"{tag}..HEAD" if tag else "HEAD"
log_output = subprocess.check_output(
    ["git", "log", "--pretty=format:- %s (%an)", rev_range],
    stderr=subprocess.DEVNULL,
    text=True,
).strip()

generated = datetime.now(tz=timezone.utc).isoformat()
print("# Release Changelog Preview")
print(f"_Generated: {generated}_\n")
if tag:
    print(f"Changes since `{tag}`:\n")
else:
    print("Changes in current history:\n")

if log_output:
    print(log_output)
else:
    print("- No commits were recorded in the selected range.")
PY

# Generate deterministic hashes and detached signatures so downstream automation can validate
# the wheel without rebuilding. An ephemeral key is used here purely for the smoke test.
SHA256_FILE="${DIST_DIR}/SHA256SUMS"
if command -v sha256sum >/dev/null 2>&1; then
    WHEEL_SHA256="$(sha256sum "${WHEEL}" | awk '{print $1}')"
    CHANGELOG_SHA256="$(sha256sum "${CHANGELOG_PREVIEW}" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
    WHEEL_SHA256="$(shasum -a 256 "${WHEEL}" | awk '{print $1}')"
    CHANGELOG_SHA256="$(shasum -a 256 "${CHANGELOG_PREVIEW}" | awk '{print $1}')"
else
    echo "warning: no sha256 implementation found; skipping checksum generation" >&2
fi

if [[ -n "${WHEEL_SHA256:-}" ]]; then
    {
        printf '%s  %s\n' "${WHEEL_SHA256}" "${WHEEL_NAME}"
        printf '%s  %s\n' "${CHANGELOG_SHA256}" "$(basename "${CHANGELOG_PREVIEW}")"
    } > "${SHA256_FILE}"
fi

SIG_PATH=""
PUB_PATH=""
CHANGELOG_SIG_PATH=""
if command -v openssl >/dev/null 2>&1; then
    if [[ -n "${SIGNING_KEY_PATH}" ]]; then
        SIGNING_KEY="${SIGNING_KEY_PATH}"
    else
        SIGNING_KEY="${TMPDIR}/release-signing-key.pem"
        openssl genrsa -out "${SIGNING_KEY}" 2048 >/dev/null 2>&1
        EPHEMERAL_SIGNING_KEY=true
    fi
    SIGNING_KEY_PATH="${SIGNING_KEY}"
    SIG_PATH="${DIST_DIR}/${WHEEL_NAME}.sig"
    PUB_PATH="${DIST_DIR}/${WHEEL_NAME}.pub"
    openssl dgst -sha256 -sign "${SIGNING_KEY}" -out "${SIG_PATH}" "${WHEEL}" >/dev/null 2>&1
    openssl rsa -in "${SIGNING_KEY}" -pubout -out "${PUB_PATH}" >/dev/null 2>&1
    CHANGELOG_SIG_PATH="${DIST_DIR}/$(basename "${CHANGELOG_PREVIEW}").sig"
    openssl dgst -sha256 -sign "${SIGNING_KEY}" -out "${CHANGELOG_SIG_PATH}" "${CHANGELOG_PREVIEW}" >/dev/null 2>&1
else
    echo "warning: openssl not available; skipping detached signature generation" >&2
fi

SIGSTORE_BUNDLE_PATH=""
SIGSTORE_SIGNATURE_PATH=""
SIGSTORE_STATUS="skipped"
if [[ "${SIGSTORE_MODE}" == "skip" ]]; then
    SIGSTORE_STATUS="disabled"
else
    if ! command -v "${COSIGN_BIN}" >/dev/null 2>&1; then
        if [[ "${SIGSTORE_MODE}" == "require" ]]; then
            echo "error: cosign binary '${COSIGN_BIN}' not found (set COSIGN/PYTHON_RELEASE_SIGSTORE_COSIGN_BIN or use --cosign-bin)" >&2
            exit 1
        fi
        SIGSTORE_STATUS="missing-binary"
        echo "warning: cosign binary '${COSIGN_BIN}' not available; skipping Sigstore signing" >&2
    else
        SIGSTORE_BUNDLE_PATH="${DIST_DIR}/${WHEEL_NAME}.sigstore"
        SIGSTORE_SIGNATURE_PATH="${DIST_DIR}/${WHEEL_NAME}.sigstore.sig"
        SIGSTORE_STATUS="signed"
        SIGSTORE_TOKEN_VALUE=""
        if [[ -n "${SIGSTORE_TOKEN_ENV}" ]]; then
            SIGSTORE_TOKEN_VALUE="${!SIGSTORE_TOKEN_ENV-}"
        fi
        COSIGN_ARGS=(
            "sign-blob"
            "--yes"
            "--bundle" "${SIGSTORE_BUNDLE_PATH}"
            "--output-signature" "${SIGSTORE_SIGNATURE_PATH}"
            "${WHEEL}"
        )
        if [[ -n "${SIGSTORE_TOKEN_VALUE}" ]]; then
            COSIGN_ARGS+=("--identity-token" "${SIGSTORE_TOKEN_VALUE}")
        fi
        if ! COSIGN_EXPERIMENTAL=1 "${COSIGN_BIN}" "${COSIGN_ARGS[@]}" >/dev/null 2>&1; then
            if [[ "${SIGSTORE_MODE}" == "require" ]]; then
                echo "error: cosign failed while creating the Sigstore bundle" >&2
                exit 1
            fi
            echo "warning: cosign failed; skipping Sigstore bundle generation" >&2
            SIGSTORE_STATUS="error"
            SIGSTORE_BUNDLE_PATH=""
            SIGSTORE_SIGNATURE_PATH=""
        fi
    fi
fi

if [[ -z "${MANIFEST_OUT}" ]]; then
    MANIFEST_OUT="${DIST_DIR}/release_artifacts.json"
fi

export WHEEL_PATH="${WHEEL}"
export WHEEL_SHA256="${WHEEL_SHA256:-}"
export WHEEL_SIG="${SIG_PATH}"
export WHEEL_PUB="${PUB_PATH}"
export CHANGELOG_PATH="${CHANGELOG_PREVIEW}"
export CHANGELOG_SHA256="${CHANGELOG_SHA256:-}"
export CHANGELOG_SIG="${CHANGELOG_SIG_PATH}"
export SHA256_MANIFEST="${SHA256_FILE}"
export MANIFEST_OUT
export SIGNATURE_EPHEMERAL="${EPHEMERAL_SIGNING_KEY}"
export SIGSTORE_BUNDLE_PATH
export SIGSTORE_SIGNATURE_PATH
export SIGSTORE_STATUS
export SIGSTORE_MODE
export SIGSTORE_TOKEN_ENV_NAME="${SIGSTORE_TOKEN_ENV}"
export SIGSTORE_COSIGN_BIN="${COSIGN_BIN}"
python - <<'PY'
import importlib.metadata as md
import json
import os
from datetime import datetime, timezone
from pathlib import Path

def optional_path(value: str) -> str | None:
    return str(Path(value)) if value else None

wheel_path = Path(os.environ["WHEEL_PATH"])
manifest_path = Path(os.environ["MANIFEST_OUT"])
manifest_path.parent.mkdir(parents=True, exist_ok=True)

try:
    version = md.version("iroha-python")
except md.PackageNotFoundError:
    version = None

manifest = {
    "generated_at": datetime.now(tz=timezone.utc).isoformat(),
    "version": version,
    "wheel": {
        "file": str(wheel_path),
        "size": wheel_path.stat().st_size,
        "sha256": os.environ.get("WHEEL_SHA256") or None,
        "signature": optional_path(os.environ.get("WHEEL_SIG", "")),
        "public_key": optional_path(os.environ.get("WHEEL_PUB", "")),
        "ephemeral_signature": os.environ.get("SIGNATURE_EPHEMERAL", "false").lower() == "true",
    },
    "changelog_preview": {
        "file": str(Path(os.environ["CHANGELOG_PATH"])),
        "size": Path(os.environ["CHANGELOG_PATH"]).stat().st_size,
        "sha256": os.environ.get("CHANGELOG_SHA256") or None,
        "signature": optional_path(os.environ.get("CHANGELOG_SIG", "")),
        "public_key": optional_path(os.environ.get("WHEEL_PUB", "")),
    },
    "checksum_manifest": str(Path(os.environ["SHA256_MANIFEST"])),
}

sigstore_bundle = optional_path(os.environ.get("SIGSTORE_BUNDLE_PATH", ""))
sigstore_signature = optional_path(os.environ.get("SIGSTORE_SIGNATURE_PATH", ""))
manifest["sigstore"] = {
    "status": os.environ.get("SIGSTORE_STATUS") or "skipped",
    "mode": os.environ.get("SIGSTORE_MODE") or "auto",
    "bundle": sigstore_bundle,
    "signature": sigstore_signature,
    "cosign_binary": os.environ.get("SIGSTORE_COSIGN_BIN") or None,
    "identity_token_env": os.environ.get("SIGSTORE_TOKEN_ENV_NAME") or None,
}

manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY

printf '%s\n' "${WHEEL}"
