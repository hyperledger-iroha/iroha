#!/usr/bin/env bash
# Validate the checksum-pinned binary pod and the source pod against one final archive.
set -euo pipefail
umask 077

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PODSPEC_PATH="${REPO_ROOT}/IrohaSwift/IrohaSwift.podspec"
RENDERER="${REPO_ROOT}/scripts/render_norito_bridge_podspec.py"
PACKAGE_DIR="${MOBILE_SDK_PACKAGE_OUT_DIR:-}"
REPORT_DIR="${SWIFT_POD_REPORT_DIR:-${REPO_ROOT}/artifacts/swift_pod_bridge}"
SUMMARY_PATH="${SWIFT_POD_SUMMARY:-${REPORT_DIR}/summary.json}"
LOG_PATH="${SWIFT_POD_LOG:-${REPORT_DIR}/pod_lint.log}"
STAGE=""

write_summary() {
  local status="$1"
  local reason="$2"
  mkdir -p "$(dirname "${SUMMARY_PATH}")"
  printf '{"status":"%s","reason":"%s","podspec":"%s","archive":"%s","binary_podspec":"%s","log_path":"%s"}\n' \
    "$status" "$reason" "$PODSPEC_PATH" "${ARCHIVE:-}" "${BINARY_PODSPEC:-}" "$LOG_PATH" \
    >"$SUMMARY_PATH"
}

fail() {
  local reason="$1"
  echo "[swift-pod-bridge] error: $reason" >&2
  write_summary "failed" "$reason"
  exit 1
}

cleanup() {
  local status=$?
  if [[ -n "$STAGE" && -d "$STAGE" ]]; then
    case "$STAGE" in
      "${TMPDIR:-/tmp}/norito-bridge-pod-lint."*) rm -rf -- "$STAGE" ;;
      *) echo "[swift-pod-bridge] refusing to clean unexpected stage: $STAGE" >&2 ;;
    esac
  fi
  exit "$status"
}
trap cleanup EXIT HUP INT TERM

command -v pod >/dev/null 2>&1 \
  || fail "cocoapods CLI not available; refusing to skip lint"
command -v python3 >/dev/null 2>&1 \
  || fail "python3 is required for authenticated archive staging"
[[ -f "$PODSPEC_PATH" && ! -L "$PODSPEC_PATH" ]] \
  || fail "missing regular source podspec at $PODSPEC_PATH"
[[ -f "$RENDERER" && ! -L "$RENDERER" ]] \
  || fail "missing regular CocoaPods renderer at $RENDERER"
[[ "$PACKAGE_DIR" == /* && -d "$PACKAGE_DIR" && ! -L "$PACKAGE_DIR" ]] \
  || fail "MOBILE_SDK_PACKAGE_OUT_DIR must be an absolute non-symbolic package directory"
PACKAGE_DIR="$(cd "$PACKAGE_DIR" && pwd -P)"
case "$PACKAGE_DIR/" in
  "$REPO_ROOT/"*) fail "MOBILE_SDK_PACKAGE_OUT_DIR must be outside the repository" ;;
esac
package_mode="$(stat -f '%Lp' "$PACKAGE_DIR" 2>/dev/null || stat -c '%a' "$PACKAGE_DIR")"
package_uid="$(stat -f '%u' "$PACKAGE_DIR" 2>/dev/null || stat -c '%u' "$PACKAGE_DIR")"
[[ "$package_mode" == "700" && "$package_uid" == "$(id -u)" ]] \
  || fail "MOBILE_SDK_PACKAGE_OUT_DIR must be current-UID-owned with exact mode 0700"

VERSION_PATH="$REPO_ROOT/IrohaSwift/VERSION"
[[ -f "$VERSION_PATH" && ! -L "$VERSION_PATH" ]] \
  || fail "IrohaSwift VERSION must be a regular non-symbolic file"
POD_VERSION="$(<"$VERSION_PATH")"
[[ "$POD_VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] \
  || fail "IrohaSwift VERSION must be canonical SemVer"
ARCHIVE="$PACKAGE_DIR/NoritoBridge-v${POD_VERSION}.xcframework.zip"
APPLE_MANIFEST="$PACKAGE_DIR/NoritoBridge-v${POD_VERSION}.artifacts.json"
BINARY_PODSPEC="$PACKAGE_DIR/NoritoBridge-${POD_VERSION}.podspec"
PACKAGE_CHECKSUMS="$PACKAGE_DIR/SHA256SUMS-apple-${MOBILE_SDK_VERSION:-$POD_VERSION}.txt"
PACKAGE_MANIFEST="$PACKAGE_DIR/mobile-sdk-apple-${MOBILE_SDK_VERSION:-$POD_VERSION}.artifacts.json"
python3 -I -S -B - "$PACKAGE_DIR" \
  "$ARCHIVE" "$APPLE_MANIFEST" "$BINARY_PODSPEC" \
  "$PACKAGE_CHECKSUMS" "$PACKAGE_MANIFEST" <<'PY' \
  || fail "packaged CocoaPods inputs are not trusted regular files"
import os
from pathlib import Path
import stat
import sys

root = Path(sys.argv[1])
paths = tuple(map(Path, sys.argv[2:]))
expected_names = {path.name for path in paths}
visible_names = {path.name for path in root.iterdir()}
if visible_names != expected_names:
    raise SystemExit("package directory does not contain the exact five Apple files")
for path in paths:
    metadata = path.lstat()
    if (
        path.parent != root
        or path.resolve(strict=True) != path
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o022
    ):
        raise SystemExit(f"untrusted packaged input: {path}")
PY

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/norito-bridge-pod-lint.XXXXXXXX")"
LOCAL_PODSPEC="$STAGE/NoritoBridge.podspec"
python3 -I -S -B "$RENDERER" \
  --root "$REPO_ROOT" \
  --archive "$ARCHIVE" \
  --output "$LOCAL_PODSPEC" \
  --local-source \
  || fail "packaged NoritoBridge archive authentication failed"

python3 -I -S -B - \
  "$ARCHIVE" "$APPLE_MANIFEST" "$BINARY_PODSPEC" \
  "$PACKAGE_CHECKSUMS" "$PACKAGE_MANIFEST" \
  "$LOCAL_PODSPEC" "$POD_VERSION" <<'PY' \
  || fail "packaged CocoaPods inventory authentication failed"
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
import zipfile

archive, apple_manifest, binary_spec, checksums, manifest, local_spec = map(
    Path, sys.argv[1:7]
)
version = sys.argv[7]
archive_payload = archive.read_bytes()

def digest(path):
    if path == archive:
        return hashlib.sha256(archive_payload).hexdigest()
    return hashlib.sha256(path.read_bytes()).hexdigest()

inventory = {}
for line in checksums.read_text(encoding="utf-8").splitlines():
    if not re.fullmatch(r"[0-9a-f]{64}  [A-Za-z0-9._+-]+", line):
        raise SystemExit("checksum inventory contains a noncanonical row")
    sha256, name = line.split("  ", 1)
    if name in inventory:
        raise SystemExit("checksum inventory contains a duplicate path")
    inventory[name] = sha256
manifest_name = manifest.name
expected_checksum_names = {
    archive.name,
    apple_manifest.name,
    binary_spec.name,
    manifest_name,
}
if set(inventory) != expected_checksum_names:
    raise SystemExit("checksum inventory does not contain the exact Apple package set")
for path in (archive, apple_manifest, binary_spec, manifest):
    if inventory.get(path.name) != digest(path):
        raise SystemExit(f"checksum mismatch for {path.name}")

document = json.loads(manifest.read_text(encoding="utf-8"))
apple_document = json.loads(apple_manifest.read_text(encoding="utf-8"))
if not isinstance(apple_document, dict) or apple_document.get("version") != version:
    raise SystemExit("embedded NoritoBridge manifest version does not match pod SemVer")
if set(document) != {"version", "apple_sdk_semver", "mode", "artifacts"}:
    raise SystemExit("package manifest is not schema-closed")
manifest_prefix = "mobile-sdk-apple-"
manifest_suffix = ".artifacts.json"
if not manifest.name.startswith(manifest_prefix) or not manifest.name.endswith(manifest_suffix):
    raise SystemExit("package manifest filename is not canonical")
diagnostic_version = manifest.name[len(manifest_prefix) : -len(manifest_suffix)]
if not diagnostic_version or document.get("version") != diagnostic_version:
    raise SystemExit("package manifest diagnostic version does not match its filename")
if document.get("mode") != "apple" or document.get("apple_sdk_semver") != version:
    raise SystemExit("package manifest does not bind the canonical Apple SDK SemVer")
artifact_rows = document.get("artifacts")
if not isinstance(artifact_rows, list) or len(artifact_rows) != 3:
    raise SystemExit("package manifest must contain exactly three Apple artifacts")
if any(
    not isinstance(entry, dict)
    or set(entry) != {"kind", "name", "path", "sha256", "bytes"}
    for entry in artifact_rows
):
    raise SystemExit("package manifest artifact rows are not schema-closed")
records = {entry["name"]: entry for entry in artifact_rows}
if len(records) != len(artifact_rows):
    raise SystemExit("package manifest contains duplicate artifact names")
expected_kinds = {
    archive.name: "apple-xcframework",
    apple_manifest.name: "apple-manifest",
    binary_spec.name: "apple-cocoapods-podspec",
}
if set(records) != set(expected_kinds):
    raise SystemExit("package manifest does not contain the exact Apple artifact set")
for name, kind in expected_kinds.items():
    record = records.get(name)
    path = manifest.parent / name
    if (
        record.get("kind") != kind
        or record.get("name") != name
        or record.get("path") != name
        or record.get("sha256") != inventory[name]
        or type(record.get("bytes")) is not int
        or record["bytes"] != path.stat().st_size
        or digest(path) != inventory[name]
    ):
        raise SystemExit(f"package manifest mismatch for {name}")

archive_sha256 = digest(archive)
production = binary_spec.read_text(encoding="utf-8")
local = local_spec.read_text(encoding="utf-8")
required_source = (
    "https://github.com/hyperledger-iroha/iroha/releases/download/"
    f"v{version}/{archive.name}"
)
for needle in (
    f"s.version          = '{version}'",
    f":http => '{required_source}'",
    f":sha256 => '{archive_sha256}'",
    "s.vendored_frameworks = 'NoritoBridge.xcframework'",
):
    if needle not in production:
        raise SystemExit(f"production binary podspec is missing {needle}")
if f":http => '{archive.as_uri()}'" not in local or f":sha256 => '{archive_sha256}'" not in local:
    raise SystemExit("local binary podspec does not bind the exact packaged archive")

# CocoaPods resolves --include-podspecs through :path, so materialize the
# already-authenticated archive beside the generated local podspec. The stage
# is a private mode-0700 directory created above and contains no caller files.
stage = local_spec.parent
stage_metadata = stage.lstat()
if (
    not stat.S_ISDIR(stage_metadata.st_mode)
    or stat.S_IMODE(stage_metadata.st_mode) != 0o700
    or stage_metadata.st_uid != os.geteuid()
):
    raise SystemExit("local CocoaPods stage is not current-UID-owned mode 0700")
directories = set()
with zipfile.ZipFile(io.BytesIO(archive_payload)) as bundle:
    for entry in bundle.infolist():
        components = entry.filename.rstrip("/").split("/")
        if (
            not components
            or components[0] != "NoritoBridge.xcframework"
            or any(component in {"", ".", ".."} for component in components)
            or "\\" in entry.filename
        ):
            raise SystemExit("authenticated archive contains an unsafe extraction path")
        target = stage.joinpath(*components)
        unix_mode = entry.external_attr >> 16
        file_type = stat.S_IFMT(unix_mode)
        is_directory = entry.is_dir() or file_type == stat.S_IFDIR
        if is_directory:
            target.mkdir(mode=0o700, parents=True, exist_ok=True)
            directories.add(target)
            continue
        target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        directories.update(
            parent for parent in target.parents if parent != stage and stage in parent.parents
        )
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        flags |= getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
        descriptor = os.open(target, flags, 0o600)
        try:
            with os.fdopen(descriptor, "wb", closefd=False) as handle:
                handle.write(bundle.read(entry))
                handle.flush()
                os.fchmod(handle.fileno(), 0o644)
        finally:
            os.close(descriptor)
for directory in sorted(directories, key=lambda path: len(path.parts), reverse=True):
    directory.chmod(0o755)
framework = stage / "NoritoBridge.xcframework"
if not framework.is_dir() or framework.is_symlink():
    raise SystemExit("local CocoaPods dependency extraction failed")
PY

mkdir -p "$REPORT_DIR"
: >"$LOG_PATH"
export COCOAPODS_DISABLE_STATS=1
export COCOAPODS_NO_REPO_UPDATE=1

run_lint() {
  local label="$1"
  shift
  set +e
  pod "$@" 2>&1 | tee -a "$LOG_PATH"
  local status=${PIPESTATUS[0]}
  set -e
  [[ $status -eq 0 ]] || fail "$label failed (see $LOG_PATH)"
}

COMMON_ARGS=(
  "--fail-fast"
  "--configuration=Release"
  "--private"
  "--use-libraries"
  "--platforms=ios"
  "--no-clean"
  "--verbose"
)
run_lint "binary pod spec lint" spec lint "$LOCAL_PODSPEC" "${COMMON_ARGS[@]}"
run_lint "source pod lib lint" lib lint "$PODSPEC_PATH" \
  "--include-podspecs=$LOCAL_PODSPEC" "${COMMON_ARGS[@]}"

write_summary "passed" "binary pod spec lint and source pod lib lint succeeded"
echo "[swift-pod-bridge] authenticated CocoaPods lints succeeded (summary: $SUMMARY_PATH)"
