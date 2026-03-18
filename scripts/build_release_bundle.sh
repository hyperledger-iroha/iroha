#!/usr/bin/env bash
# shellcheck disable=SC2317
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: build_release_bundle.sh --profile <name> --config <config> [options]

Options:
  --profile <name>        Logical profile name (e.g. iroha2, iroha3). Required.
  --config <config>       Configuration bundle to embed (single, nexus, or path). Required.
  --features <list>       Optional comma-separated Cargo feature list.
  --target <triple>       Optional target triple to pass to Cargo.
  --artifacts-dir <path>  Output directory for generated bundles (default: dist).
  --signing-key <path>    Optional PEM-encoded private key used to sign the archive.
  --manifest-out <path>   Optional JSON manifest destination (default: dist/<profile>-<version>-manifest.json).
  -h, --help              Show this help message.

The script builds the deploy profile binaries, collects the appropriate default
configuration, and emits a deterministic tar.zst bundle while writing a
PROFILE.toml manifest alongside the binaries.
EOF
}

log() {
    printf '[dual-build] %s\n' "$*" >&2
}

profile=""
config=""
features=""
target=""
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
        --target)
            target="${2:-}"
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

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v zstd >/dev/null 2>&1; then
    printf 'zstd is required to produce release bundles\n' >&2
    exit 1
fi

version="$(awk -F\" '/^version *=/ { print $2; exit }' Cargo.toml)"
commit="$(git rev-parse --short HEAD)"
timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [[ -z "$manifest_out" ]]; then
    manifest_out="${artifacts_dir%/}/${profile}-${version}-manifest.json"
fi

case "$(uname -s)" in
    Linux)
        os_tag="linux"
        ;;
    Darwin)
        os_tag="mac"
        ;;
    CYGWIN*|MINGW*|MSYS*)
        os_tag="win"
        ;;
    *)
        os_tag="$(uname -s | tr '[:upper:]' '[:lower:]')"
        ;;
esac

arch="$(uname -m)"

if [[ -z "$features" ]]; then
    case "$profile" in
        iroha2) features="build-i2" ;;
        iroha3) features="build-i3" ;;
    esac
fi

cargo_cmd=(cargo build --profile deploy --bins --locked)
if [[ -n "$target" ]]; then
    cargo_cmd+=(--target "$target")
fi
if [[ -n "$features" ]]; then
    cargo_cmd+=(--features "$features")
fi

log "Building binaries (profile=${profile}, config=${config}, features=${features:-<none>})"
"${cargo_cmd[@]}"

bundle_root="${artifacts_dir%/}"
mkdir -p "$bundle_root"

bundle_dir="${bundle_root}/${profile}-${version}-${os_tag}"
rm -rf "$bundle_dir"
mkdir -p "$bundle_dir/bin" "$bundle_dir/config"

daemon_bin="iroha3d"
cli_bin="iroha3"
if [[ "$profile" == "iroha2" ]]; then
    daemon_bin="iroha2d"
    cli_bin="iroha2"
fi

install -m 755 "target/deploy/${daemon_bin}" "$bundle_dir/bin/${daemon_bin}"
install -m 755 "target/deploy/${cli_bin}" "$bundle_dir/bin/${cli_bin}"
install -m 755 target/deploy/kagami "$bundle_dir/bin/kagami"
install -m 644 LICENSE "$bundle_dir/LICENSE"

case "$config" in
    single)
        install -m 644 defaults/genesis.json "$bundle_dir/config/genesis.json"
        install -m 644 defaults/client.toml "$bundle_dir/config/client.toml"
        if [[ -d defaults/config.d ]]; then
            mkdir -p "$bundle_dir/config/config.d"
            cp -a defaults/config.d/. "$bundle_dir/config/config.d/"
        fi
        ;;
    nexus)
        install -m 644 defaults/nexus/genesis.json "$bundle_dir/config/genesis.json"
        install -m 644 defaults/nexus/client.toml "$bundle_dir/config/client.toml"
        install -m 644 defaults/nexus/config.toml "$bundle_dir/config/config.toml"
        ;;
    *)
        if [[ -d "$config" ]]; then
            cp -a "$config"/. "$bundle_dir/config/"
        else
            printf 'Unsupported config value: %s\n' "$config" >&2
            exit 1
        fi
        ;;
esac

feature_label="$features"
if [[ -z "$feature_label" ]]; then
    feature_label="$config"
fi

cat >"$bundle_dir/PROFILE.toml" <<EOF
profile = "$profile"
config = "$config"
version = "$version"
commit = "$commit"
built_at = "$timestamp"
os = "$os_tag"
arch = "$arch"
features = "$feature_label"
EOF

tarball="${bundle_root}/${profile}-${version}-${os_tag}.tar.zst"
log "Packaging bundle $(basename "$tarball")"
tar -C "$bundle_root" -c "$(basename "$bundle_dir")" | zstd -19 --long=31 -o "$tarball"
rm -rf "$bundle_dir"

if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$tarball" > "${tarball}.sha256"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$tarball" > "${tarball}.sha256"
else
    printf 'sha256sum or shasum is required to hash artifacts\n' >&2
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
    "features": "${feature_label}",
    "artifacts": [
        {
            "file": "${tarball}",
            "sha256": "${checksum}",
            "signature": "${sig_path}" if "${sig_path}" else None,
            "public_key": "${pub_path}" if "${pub_path}" else None,
        }
    ],
}
manifest_path.write_text(json.dumps(manifest, indent=2) + "\\n", encoding="utf-8")
PY

echo "$tarball"
