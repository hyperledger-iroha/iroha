#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: build_release_image.sh --profile <name> --config <config> [options]

Options:
  --profile <name>        Logical profile name: iroha2 or iroha3. Required.
  --config <config>       Configuration bundle to embed (single, nexus, or taira). Required.
  --features <list>       Optional comma-separated Cargo feature list passed to the Docker build.
  --cargo-build-jobs <n>  Optional Cargo parallelism limit passed as CARGO_BUILD_JOBS.
  --binaries "<list>"     Optional space-separated binary list passed as BINARIES.
  --use-target-prebuilt   Package existing target/deploy binaries instead of compiling them in Docker.
  --validator-lock-sha256 <sha256>
                          Reviewed Cargo.lock digest required for Taira builds.
  --validator-source-tree-sha256 <sha256>
                          Attested source-tree digest required for Taira builds.
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
cargo_build_jobs=""
binaries=""
use_target_prebuilt="0"
validator_lock_sha256=""
validator_source_tree_sha256=""
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
        --cargo-build-jobs)
            cargo_build_jobs="${2:-}"
            shift 2
            ;;
        --binaries)
            binaries="${2:-}"
            shift 2
            ;;
        --use-target-prebuilt)
            use_target_prebuilt="1"
            shift 1
            ;;
        --validator-lock-sha256)
            validator_lock_sha256="${2:-}"
            shift 2
            ;;
        --validator-source-tree-sha256)
            validator_source_tree_sha256="${2:-}"
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

case "$profile" in
    iroha2|iroha3)
        ;;
    *)
        printf 'Unsupported profile value: %s (expected iroha2 or iroha3)\n' "$profile" >&2
        exit 1
        ;;
esac

if ! command -v python3 >/dev/null 2>&1; then
    printf 'python3 is required to write the release manifest\n' >&2
    exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
    printf 'docker is required to build release images\n' >&2
    exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
build_context="$repo_root"
temp_build_context=""

cleanup() {
    if [[ -n "$temp_build_context" && -d "$temp_build_context" ]]; then
        rm -rf "$temp_build_context"
    fi
}

trap cleanup EXIT

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
    taira)
        config_profile="taira"
        if [[ "${IROHA_VALIDATOR_RELEASE_VERIFIED:-}" != "1" ]]; then
            printf 'Taira image builds must enter through the DPN attested validator-image wrapper\n' >&2
            exit 1
        fi
        if [[ ! "$validator_lock_sha256" =~ ^[0-9a-f]{64}$ ]]; then
            printf '%s\n' '--validator-lock-sha256 must be exactly 64 lowercase hex characters' >&2
            exit 1
        fi
        if [[ ! "$validator_source_tree_sha256" =~ ^[0-9a-f]{64}$ ]]; then
            printf '%s\n' '--validator-source-tree-sha256 must be exactly 64 lowercase hex characters' >&2
            exit 1
        fi
        if [[ ! -f Cargo.lock || -L Cargo.lock ]]; then
            printf 'reviewed Taira Cargo.lock is missing or not a regular file\n' >&2
            exit 1
        fi
        if command -v sha256sum >/dev/null 2>&1; then
            actual_validator_lock_sha256="$(sha256sum Cargo.lock | awk '{print $1}')"
        elif command -v shasum >/dev/null 2>&1; then
            actual_validator_lock_sha256="$(shasum -a 256 Cargo.lock | awk '{print $1}')"
        else
            printf 'sha256sum or shasum is required to verify the Taira Cargo.lock\n' >&2
            exit 1
        fi
        if [[ "$actual_validator_lock_sha256" != "$validator_lock_sha256" ]]; then
            printf 'reviewed Taira Cargo.lock checksum mismatch\n' >&2
            exit 1
        fi
        if [[ "$use_target_prebuilt" == "1" ]]; then
            printf 'Taira images cannot use unproven target-prebuilt binaries\n' >&2
            exit 1
        fi
        case ",${features}," in
            *,embedded-soracloud-runtime,*)
                ;;
            *)
                features="${features:+${features},}embedded-soracloud-runtime"
                ;;
        esac
        if [[ -z "$cargo_build_jobs" ]]; then
            cargo_build_jobs="1"
        fi
        if [[ -z "$binaries" ]]; then
            binaries="irohad"
        fi
        ;;
    *)
        printf 'Unsupported config value: %s\n' "$config" >&2
        exit 1
        ;;
esac

log "Building Docker image ${image_tag}"
docker_build_args=(
    --build-arg PROFILE=deploy
    --build-arg "FEATURES=${features}"
    --build-arg "CONFIG_PROFILE=${config_profile}"
)

if [[ "$config_profile" == "taira" ]]; then
    docker_build_args+=(
        --build-arg "VALIDATOR_LOCK_SHA256=${validator_lock_sha256}"
        --build-arg "VALIDATOR_SOURCE_TREE_SHA256=${validator_source_tree_sha256}"
    )
fi

if [[ "$use_target_prebuilt" == "1" ]]; then
    prebuilt_dir="${repo_root}/dist/docker-bin"
    mkdir -p "$prebuilt_dir"
    for bin in $binaries; do
        source_path="${repo_root}/target/deploy/${bin}"
        target_path="${prebuilt_dir}/${bin}"
        if [[ ! -f "$source_path" ]]; then
            printf 'missing prebuilt binary: %s\n' "$source_path" >&2
            printf 'build it first so target/deploy/%s exists before using --use-target-prebuilt\n' "$bin" >&2
            exit 1
        fi
        cp "$source_path" "$target_path"
        chmod 755 "$target_path"
    done

    temp_build_context="$(mktemp -d "${TMPDIR:-/tmp}/iroha-image-context.XXXXXX")"
    build_context="$temp_build_context"
    mkdir -p \
        "${build_context}/scripts" \
        "${build_context}/configs/soranexus" \
        "${build_context}/dist" \
        "${build_context}/defaults" \
        "${build_context}/codec/rans"
    cp "${repo_root}/Dockerfile" "${build_context}/Dockerfile"
    cp "${repo_root}/scripts/docker_entrypoint.sh" "${build_context}/scripts/docker_entrypoint.sh"
    cp -R "${repo_root}/configs/soranexus/taira" "${build_context}/configs/soranexus/taira"
    cp -R "${repo_root}/dist/docker-bin" "${build_context}/dist/docker-bin"
    cp -R "${repo_root}/defaults/." "${build_context}/defaults/"
    cp -R "${repo_root}/codec/rans/tables" "${build_context}/codec/rans/tables"
    docker_build_args+=(--build-arg USE_PREBUILT=1)
fi

if [[ -n "$cargo_build_jobs" ]]; then
    docker_build_args+=(--build-arg "CARGO_BUILD_JOBS=${cargo_build_jobs}")
fi

if [[ -n "$binaries" ]]; then
    docker_build_args+=(--build-arg "BINARIES=${binaries}")
fi

docker build \
    "${docker_build_args[@]}" \
    --tag "${image_tag}" \
    --file "${build_context}/Dockerfile" \
    "${build_context}"

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

python3 - \
    "$manifest_out" \
    "$profile" \
    "$config" \
    "$version" \
    "$commit" \
    "$timestamp" \
    "$os_tag" \
    "$arch" \
    "$features" \
    "$validator_lock_sha256" \
    "$validator_source_tree_sha256" \
    "$image_tag" \
    "$image_id" \
    "$tarball" \
    "$checksum" \
    "$sig_path" \
    "$pub_path" <<'MANIFEST_PY'
import json
import sys
from pathlib import Path

(
    manifest_out,
    profile,
    config,
    version,
    commit,
    timestamp,
    os_tag,
    arch,
    features,
    validator_lock_sha256,
    validator_source_tree_sha256,
    image_tag,
    image_id,
    tarball,
    checksum,
    sig_path,
    pub_path,
) = sys.argv[1:]

manifest_path = Path(manifest_out)
manifest_path.parent.mkdir(parents=True, exist_ok=True)
manifest = {
    "profile": profile,
    "config": config,
    "version": version,
    "commit": commit,
    "built_at": timestamp,
    "os": os_tag,
    "arch": arch,
    "features": features,
    "validator_lock_sha256": validator_lock_sha256 or None,
    "validator_source_tree_sha256": validator_source_tree_sha256 or None,
    "image_tag": image_tag,
    "image_id": image_id,
    "artifacts": [
        {
            "file": tarball,
            "sha256": checksum,
            "signature": sig_path or None,
            "public_key": pub_path or None,
        }
    ],
}
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
MANIFEST_PY

printf '%s\n' "$tarball"
