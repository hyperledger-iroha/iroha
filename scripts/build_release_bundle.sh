#!/usr/bin/env bash
# shellcheck disable=SC2317
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: build_release_bundle.sh --profile <name> --config <config> [options]

Options:
  --profile <name>        Logical profile name: iroha2 or iroha3. Required.
  --config <config>       Configuration bundle to embed (single, nexus, or path). Required.
  --features <list>       Optional comma-separated Cargo feature list.
  --target <triple>       Optional target triple to pass to Cargo.
  --artifacts-dir <path>  Output directory for generated bundles (default: dist).
  --external-signer <path>
                          Optional reviewed PKCS#11/HSM wrapper. It receives the
                          archive path and a new signature-output path and must
                          write exactly one raw 64-byte Ed25519 signature.
  --signing-public-key <path>
                          Raw 32-byte Ed25519 public key for signature verification.
  --trusted-signing-fingerprint <hex>
                          Reviewed lowercase SHA256 of the exact raw public key.
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
external_signer=""
signing_public_key=""
trusted_signing_fingerprint=""
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
        --external-signer)
            external_signer="${2:-}"
            shift 2
            ;;
        --signing-public-key)
            signing_public_key="${2:-}"
            shift 2
            ;;
        --trusted-signing-fingerprint)
            trusted_signing_fingerprint="${2:-}"
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

signing_option_count=0
[[ -n "$external_signer" ]] && signing_option_count=$((signing_option_count + 1))
[[ -n "$signing_public_key" ]] && signing_option_count=$((signing_option_count + 1))
[[ -n "$trusted_signing_fingerprint" ]] &&
    signing_option_count=$((signing_option_count + 1))
if [[ "$signing_option_count" -ne 0 && "$signing_option_count" -ne 3 ]]; then
    printf '%s\n' \
        '--external-signer, --signing-public-key, and --trusted-signing-fingerprint must be supplied together' >&2
    exit 1
fi
if [[ -n "$trusted_signing_fingerprint" ]] &&
    [[ ! "$trusted_signing_fingerprint" =~ ^[0-9a-f]{64}$ ]]; then
    printf '%s\n' \
        '--trusted-signing-fingerprint must be exactly 64 lowercase hexadecimal characters' >&2
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

python3 - \
    "$bundle_dir/PROFILE.toml" \
    "$profile" \
    "$config" \
    "$version" \
    "$commit" \
    "$timestamp" \
    "$os_tag" \
    "$arch" \
    "$feature_label" <<'PROFILE_PY'
import json
import sys
from pathlib import Path

(
    profile_path_raw,
    profile,
    config,
    version,
    commit,
    built_at,
    os_tag,
    arch,
    features,
) = sys.argv[1:]
profile_path = Path(profile_path_raw)
values = (
    ("profile", profile),
    ("config", config),
    ("version", version),
    ("commit", commit),
    ("built_at", built_at),
    ("os", os_tag),
    ("arch", arch),
    ("features", features),
)
rendered = "\n".join(
    f"{key} = {json.dumps(value, ensure_ascii=False)}" for key, value in values
)
profile_path.write_text(rendered + "\n", encoding="utf-8")
PROFILE_PY

tarball="${bundle_root}/${profile}-${version}-${os_tag}.tar.zst"
log "Packaging bundle $(basename "$tarball")"
tar -C "$bundle_root" -c "$(basename "$bundle_dir")" | zstd -19 --long=31 -o "$tarball"
rm -rf "$bundle_dir"

checksum_dir="$(dirname "$tarball")"
checksum_name="$(basename "$tarball")"
if command -v sha256sum >/dev/null 2>&1; then
    (cd "$checksum_dir" && sha256sum "$checksum_name") > "${tarball}.sha256"
elif command -v shasum >/dev/null 2>&1; then
    (cd "$checksum_dir" && shasum -a 256 "$checksum_name") > "${tarball}.sha256"
else
    printf 'sha256sum or shasum is required to hash artifacts\n' >&2
    exit 1
fi

checksum="$(cut -d' ' -f1 "${tarball}.sha256")"

sig_path=""
pub_path=""
if [[ -n "$external_signer" ]]; then
    sig_path="${tarball}.sig"
    pub_path="${tarball}.pub"
    python3 - \
        "$tarball" \
        "$external_signer" \
        "$signing_public_key" \
        "$trusted_signing_fingerprint" \
        "$sig_path" \
        "$pub_path" <<'SIGNING_PY'
from __future__ import annotations

import base64
import hashlib
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

artifact_raw, signer_raw, public_key_raw, fingerprint, signature_raw, public_out_raw = (
    sys.argv[1:]
)


def fail(message: str) -> "NoReturn":
    print(f"release Ed25519 signing failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def absolute(raw: str) -> Path:
    return Path(os.path.abspath(raw))


def reject_symlink_chain(path: Path, label: str, *, leaf_may_be_missing: bool = False) -> None:
    components = list(reversed(path.parents)) + [path]
    for index, component in enumerate(components):
        try:
            metadata = component.lstat()
        except FileNotFoundError:
            if leaf_may_be_missing and index == len(components) - 1:
                return
            fail(f"{label} path component is missing: {component}")
        except OSError as error:
            fail(f"cannot inspect {label} path component {component}: {error}")
        if stat.S_ISLNK(metadata.st_mode):
            fail(f"{label} must not contain a symlink path component: {component}")


def inspect_regular(path: Path, label: str, *, executable: bool = False) -> os.stat_result:
    reject_symlink_chain(path, label)
    try:
        metadata = path.lstat()
    except OSError as error:
        fail(f"cannot inspect {label}: {error}")
    if not stat.S_ISREG(metadata.st_mode):
        fail(f"{label} must be a regular file")
    if metadata.st_nlink != 1:
        fail(f"{label} must have exactly one hard link")
    if metadata.st_mode & 0o022:
        fail(f"{label} must not be group- or world-writable")
    allowed_owners = {os.getuid(), 0} if hasattr(os, "getuid") else {metadata.st_uid}
    if metadata.st_uid not in allowed_owners:
        fail(f"{label} must be owned by the invoking user or root")
    if executable and not os.access(path, os.X_OK):
        fail(f"{label} must be executable")
    return metadata


def stable_read(path: Path, label: str, expected_size: int | None = None) -> bytes:
    before = inspect_regular(path, label)
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open {label}: {error}")
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            fail(f"{label} changed while it was opened")
        chunks = []
        total = 0
        while True:
            chunk = os.read(descriptor, 4096)
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if expected_size is not None and total > expected_size:
                fail(f"{label} must contain exactly {expected_size} raw bytes")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    identity_before = (
        opened.st_dev,
        opened.st_ino,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_nlink,
    )
    identity_after = (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        after.st_nlink,
    )
    if identity_before != identity_after:
        fail(f"{label} changed while it was read")
    payload = b"".join(chunks)
    if expected_size is not None and len(payload) != expected_size:
        fail(f"{label} must contain exactly {expected_size} raw bytes")
    return payload


def digest_and_identity(path: Path, label: str) -> tuple[str, tuple[int, ...]]:
    metadata = inspect_regular(path, label)
    digest = hashlib.sha256()
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"cannot open {label}: {error}")
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino):
            fail(f"{label} changed while it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        closed = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    identity = (
        opened.st_dev,
        opened.st_ino,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_nlink,
    )
    if identity != (
        closed.st_dev,
        closed.st_ino,
        closed.st_size,
        closed.st_mtime_ns,
        closed.st_ctime_ns,
        closed.st_nlink,
    ):
        fail(f"{label} changed while it was hashed")
    return digest.hexdigest(), identity


def require_new_output(path: Path, label: str) -> None:
    reject_symlink_chain(path.parent, f"{label} parent")
    if not path.parent.is_dir():
        fail(f"{label} parent must be a directory")
    try:
        path.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        fail(f"cannot inspect {label}: {error}")
    fail(f"{label} already exists")


def install_exclusive(path: Path, payload: bytes, label: str) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o644)
    except OSError as error:
        fail(f"cannot create {label}: {error}")
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                fail(f"short write while creating {label}")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


artifact = absolute(artifact_raw)
signer = absolute(signer_raw)
public_key_path = absolute(public_key_raw)
signature_path = absolute(signature_raw)
public_out_path = absolute(public_out_raw)

if signature_path == public_out_path:
    fail("signature and public-key outputs must be different paths")
for output, output_label in (
    (signature_path, "signature output"),
    (public_out_path, "public-key output"),
):
    if output in {artifact, signer, public_key_path}:
        fail(f"{output_label} must not overwrite a signing input")
    require_new_output(output, output_label)

inspect_regular(signer, "external signer", executable=True)
public_key = stable_read(public_key_path, "Ed25519 public key", 32)
if not any(public_key):
    fail("Ed25519 public key must not be all zero")
if hashlib.sha256(public_key).hexdigest() != fingerprint:
    fail("Ed25519 public key does not match the reviewed fingerprint")

artifact_digest, artifact_identity = digest_and_identity(artifact, "release artifact")
openssl = shutil.which("openssl")
if openssl is None:
    fail("openssl is required for Ed25519 signature verification")

spki_der = bytes.fromhex("302a300506032b6570032100") + public_key
public_pem = (
    b"-----BEGIN PUBLIC KEY-----\n"
    + base64.b64encode(spki_der)
    + b"\n-----END PUBLIC KEY-----\n"
)

with tempfile.TemporaryDirectory(
    prefix="iroha-ed25519-sign-", dir=str(artifact.parent)
) as temp_raw:
    temp_dir = Path(temp_raw)
    signature_temp = temp_dir / "signature.raw"
    public_temp = temp_dir / "public.pem"
    public_temp.write_bytes(public_pem)
    public_temp.chmod(0o600)
    completed = subprocess.run(
        [str(signer), str(artifact), str(signature_temp)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if completed.returncode != 0:
        fail(f"external signer exited with status {completed.returncode}")
    signature = stable_read(signature_temp, "external Ed25519 signature", 64)
    if not any(signature):
        fail("external Ed25519 signature must not be all zero")
    digest_after, identity_after = digest_and_identity(artifact, "release artifact")
    if (artifact_digest, artifact_identity) != (digest_after, identity_after):
        fail("release artifact changed while it was being signed")
    verified = subprocess.run(
        [
            openssl,
            "pkeyutl",
            "-verify",
            "-pubin",
            "-inkey",
            str(public_temp),
            "-rawin",
            "-in",
            str(artifact),
            "-sigfile",
            str(signature_temp),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if verified.returncode != 0:
        fail("external Ed25519 signature verification failed")

installed = []
try:
    install_exclusive(public_out_path, public_pem, "public-key output")
    installed.append(public_out_path)
    install_exclusive(signature_path, signature, "signature output")
    installed.append(signature_path)
    verified = subprocess.run(
        [
            openssl,
            "pkeyutl",
            "-verify",
            "-pubin",
            "-inkey",
            str(public_out_path),
            "-rawin",
            "-in",
            str(artifact),
            "-sigfile",
            str(signature_path),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if verified.returncode != 0:
        fail("installed Ed25519 signature verification failed")
except BaseException:
    for installed_path in reversed(installed):
        try:
            installed_path.unlink()
        except OSError:
            pass
    raise
SIGNING_PY
fi

python3 - \
    "$manifest_out" \
    "$profile" \
    "$config" \
    "$version" \
    "$commit" \
    "$timestamp" \
    "$os_tag" \
    "$arch" \
    "$feature_label" \
    "$tarball" \
    "$checksum" \
    "$sig_path" \
    "$pub_path" \
    "$trusted_signing_fingerprint" <<'MANIFEST_PY'
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
    feature_label,
    tarball,
    checksum,
    sig_path,
    pub_path,
    signer_fingerprint,
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
    "features": feature_label,
    "artifacts": [
        {
            "file": tarball,
            "sha256": checksum,
            "signature": sig_path or None,
            "public_key": pub_path or None,
            "signature_algorithm": "ed25519" if sig_path else None,
            "public_key_format": "pem-spki-ed25519" if pub_path else None,
            "signer_fingerprint_sha256": signer_fingerprint or None,
        }
    ],
}
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
MANIFEST_PY

echo "$tarball"
