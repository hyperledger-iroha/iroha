#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: build_release_image.sh --profile <name> --config <config> [options]

Options:
  --profile <name>        Logical profile name (e.g. iroha2, iroha3). Required.
  --config <config>       Configuration bundle to embed (single, nexus, or path). Required.
  --features <list>       Optional comma-separated Cargo feature list passed to the Docker build.
  --tag <tag>             Docker image tag (default: hyperledger/iroha:<profile>-<version>).
  --artifacts-dir <dir>   Output directory for saved images/manifests (default: dist).
  --signing-key <path>    Optional PEM private key for signing the saved image tarball.
  --manifest-out <path>   Optional JSON manifest destination (default: <artifacts-dir>/<profile>-<version>-image.json).
  -h, --help              Show this help message.
EOF
}

log() {
    printf '[dual-build-image] %s\n' "$*" >&2
}

profile=""
config=""
features=""
image_tag=""
artifacts_dir="dist"
signing_key=""
manifest_out=""

while (($#)); do
    case "$1" in
        --profile)
            profile="${2:-}"
            shift 2
            ;;
        --config)
            config="${2:-}"
            shift 2
            ;;
        --features)
            features="${2:-}"
            shift 2
            ;;
        --tag)
            image_tag="${2:-}"
            shift 2
            ;;
        --artifacts-dir)
            artifacts_dir="${2:-}"
            shift 2
            ;;
        --signing-key)
            signing_key="${2:-}"
            shift 2
            ;;
        --manifest-out)
            manifest_out="${2:-}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n\n' "$1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ -z "$profile" || -z "$config" ]]; then
    usage >&2
    exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
    printf 'docker is required to build release images\n' >&2
    exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="$(awk -F\" '/^version *=/ { print $2; exit }' Cargo.toml)"
commit="$(git rev-parse --short HEAD)"
timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
arch="$(uname -m)"
os_tag="$(uname -s | tr '[:upper:]' '[:lower:]')"

if [[ -z "$image_tag" ]]; then
    image_tag="hyperledger/iroha:${profile}-${version}"
fi

if [[ -z "$manifest_out" ]]; then
    manifest_out="${artifacts_dir%/}/${profile}-${version}-image.json"
fi

bundle_root="${artifacts_dir%/}"
mkdir -p "$bundle_root"

# Determine config profile handled by Dockerfile.
case "$config" in
    single)
        config_profile="single"
        ;;
    nexus)
        config_profile="nexus"
        ;;
    *)
        printf 'Unsupported config value: %s\n' "$config" >&2
        exit 1
        ;;
esac

log "Building Docker image ${image_tag}"
docker build \
    --build-arg PROFILE=deploy \
    --build-arg FEATURES="${features}" \
    --build-arg CONFIG_PROFILE="${config_profile}" \
    --tag "${image_tag}" \
    --file Dockerfile \
    .

tarball="${bundle_root}/${profile}-${version}-${os_tag}-image.tar"
log "Saving image ${image_tag} -> $(basename "$tarball")"
docker save "${image_tag}" > "${tarball}"

if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${tarball}" > "${tarball}.sha256"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${tarball}" > "${tarball}.sha256"
else
    printf 'sha256sum or shasum is required to hash image tarball\n' >&2
    exit 1
fi

checksum="$(cut -d' ' -f1 "${tarball}.sha256")"

sig_path=""
pub_path=""
if [[ -n "$signing_key" ]]; then
    if ! command -v openssl >/dev/null 2>&1; then
        printf 'openssl is required when --signing-key is provided\n' >&2
        exit 1
    fi
    sig_path="${tarball}.sig"
    pub_path="${tarball}.pub"
    openssl dgst -sha256 -sign "$signing_key" -out "$sig_path" "$tarball"
    openssl rsa -in "$signing_key" -pubout -out "$pub_path" >/dev/null 2>&1
fi

image_id="$(docker image inspect "${image_tag}" --format '{{.Id}}')"

python - <<PY
import json
from pathlib import Path

manifest_path = Path("${manifest_out}")
manifest_path.parent.mkdir(parents=True, exist_ok=True)
manifest = {
    "profile": "${profile}",
    "config": "${config}",
    "version": "${version}",
    "commit": "${commit}",
    "built_at": "${timestamp}",
    "os": "${os_tag}",
    "arch": "${arch}",
    "features": "${features}",
    "image_tag": "${image_tag}",
    "image_id": "${image_id}",
    "artifacts": [
        {
            "file": "${tarball}",
            "sha256": "${checksum}",
            "signature": "${sig_path}" if "${sig_path}" else None,
            "public_key": "${pub_path}" if "${pub_path}" else None,
        }
    ],
}
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY

printf '%s\n' "$tarball"
