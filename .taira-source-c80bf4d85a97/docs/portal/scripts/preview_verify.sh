#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"

die() {
  echo "[preview-verify] error: $*" >&2
  exit 1
}

info() {
  echo "[preview-verify] $*" >&2
}

usage() {
  cat <<'EOF'
Usage: scripts/preview_verify.sh [--build-dir PATH] [--descriptor PATH] [--archive PATH]

Verify a docs portal preview artifact by checking the checksum manifest and,
optionally, the preview descriptor (and archive) hashes.

Options:
  --build-dir PATH    Path to the extracted Docusaurus build directory (default: ./build)
  --descriptor PATH   Optional path to preview-descriptor JSON for manifest/archive verification
  --archive PATH      Optional path to preview-site archive (tarball) referenced by the descriptor
  -h, --help          Show this message
EOF
}

BUILD_DIR="${SCRIPT_DIR}/../build"
DESCRIPTOR_PATH=""
ARCHIVE_PATH=""

while (($#)); do
  case "$1" in
    --build-dir)
      [[ $# -ge 2 ]] || die "missing value for $1"
      BUILD_DIR="$2"
      shift 2
      ;;
    --descriptor)
      [[ $# -ge 2 ]] || die "missing value for $1"
      DESCRIPTOR_PATH="$2"
      shift 2
      ;;
    --archive)
      [[ $# -ge 2 ]] || die "missing value for $1"
      ARCHIVE_PATH="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

[[ -d "$BUILD_DIR" ]] || die "build directory not found at $BUILD_DIR"
BUILD_DIR="$(cd "$BUILD_DIR" >/dev/null 2>&1 && pwd)"
CHECKSUM_MANIFEST="${BUILD_DIR}/checksums.sha256"

[[ -f "$CHECKSUM_MANIFEST" ]] || die "missing checksum manifest at ${CHECKSUM_MANIFEST}"

SHA_MODE=""
if command -v sha256sum >/dev/null 2>&1; then
  SHA_MODE="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
  SHA_MODE="shasum"
else
  die "sha256sum/shasum not found; install coreutils or ensure shasum is available"
fi

run_sha() {
  if [[ "$SHA_MODE" == "sha256sum" ]]; then
    sha256sum "$@"
  else
    shasum -a 256 "$@"
  fi
}

compute_sha() {
  run_sha "$1" | awk '{print $1}'
}

command -v node >/dev/null 2>&1 \
  || die "node not found; install Node.js to validate the checksum inventory"
info "checking checksum manifest file inventory"
node - "$BUILD_DIR" "$CHECKSUM_MANIFEST" <<'NODE'
const fs = require('fs');
const path = require('path');

const [buildDir, manifestPath] = process.argv.slice(2);

function fail(message) {
  console.error(`[preview-verify] error: ${message}`);
  process.exit(1);
}

const manifestRelative = path.relative(buildDir, manifestPath).split(path.sep).join('/');
if (
  manifestRelative === '' ||
  manifestRelative === '..' ||
  manifestRelative.startsWith('../') ||
  path.posix.isAbsolute(manifestRelative)
) {
  fail('checksum manifest must be a file inside the build directory');
}
if (!fs.lstatSync(manifestPath).isFile()) {
  fail('checksum manifest must be a regular file');
}

function walk(root) {
  const pending = [root];
  const files = [];
  while (pending.length > 0) {
    const current = pending.pop();
    for (const entry of fs.readdirSync(current, {withFileTypes: true})) {
      const resolved = path.join(current, entry.name);
      const relative = path.relative(root, resolved).split(path.sep).join('/');
      if (entry.isDirectory()) {
        pending.push(resolved);
      } else if (entry.isFile()) {
        if (relative !== manifestRelative && entry.name !== '.DS_Store') {
          files.push(relative);
        }
      } else {
        fail(`build contains non-regular entry: ${relative}`);
      }
    }
  }
  return files.sort();
}

const expected = [];
const seen = new Set();
const lines = fs.readFileSync(manifestPath, 'utf8').split(/\r?\n/);
for (let index = 0; index < lines.length; index += 1) {
  const line = lines[index];
  if (!line) continue;
  const match = line.match(/^[0-9a-f]{64}  (.+)$/);
  if (!match) {
    fail(`checksum manifest line ${index + 1} is malformed`);
  }
  const entry = match[1];
  const segments = entry.split('/');
  if (
    path.posix.isAbsolute(entry) ||
    entry.includes('\\') ||
    segments.some((segment) => segment === '' || segment === '.' || segment === '..')
  ) {
    fail(`checksum manifest line ${index + 1} has an unsafe path`);
  }
  if (seen.has(entry)) {
    fail(`checksum manifest contains duplicate path: ${entry}`);
  }
  seen.add(entry);
  expected.push(entry);
}
expected.sort();

const actual = walk(buildDir);
const expectedSet = new Set(expected);
const actualSet = new Set(actual);
for (const entry of expected) {
  if (!actualSet.has(entry)) {
    fail(`checksum manifest references missing file: ${entry}`);
  }
}
for (const entry of actual) {
  if (!expectedSet.has(entry)) {
    fail(`build contains file absent from checksum manifest: ${entry}`);
  }
}
NODE
info "checksum manifest file inventory verified"

info "verifying ${CHECKSUM_MANIFEST}"
(
  cd "$BUILD_DIR"
  run_sha -c "checksums.sha256"
)
info "checksum manifest verified"

manifest_sha="$(compute_sha "$CHECKSUM_MANIFEST")"

if [[ -n "$DESCRIPTOR_PATH" ]]; then
  [[ -f "$DESCRIPTOR_PATH" ]] || die "descriptor not found at ${DESCRIPTOR_PATH}"
  descriptor_args=("$DESCRIPTOR_PATH" "$CHECKSUM_MANIFEST" "$manifest_sha")
  archive_sha=""
  if [[ -n "$ARCHIVE_PATH" ]]; then
    [[ -f "$ARCHIVE_PATH" ]] || die "archive not found at ${ARCHIVE_PATH}"
    archive_sha="$(compute_sha "$ARCHIVE_PATH")"
    descriptor_args+=("$ARCHIVE_PATH" "$archive_sha")
  else
    descriptor_args+=("" "")
  fi

  info "validating descriptor ${DESCRIPTOR_PATH}"
  node - "${descriptor_args[@]}" <<'NODE'
const fs = require('fs');
const path = require('path');

const [
  descriptorPath,
  manifestPath,
  manifestSha,
  archivePath,
  archiveSha,
] = process.argv.slice(2);

function fail(msg) {
  console.error(`[preview-verify] error: ${msg}`);
  process.exit(1);
}

let descriptor;
try {
  descriptor = JSON.parse(fs.readFileSync(descriptorPath, 'utf8'));
} catch (error) {
  fail(`failed to parse descriptor JSON: ${error.message}`);
}

const manifest = descriptor?.checksums_manifest;
if (!manifest) {
  fail('descriptor missing checksums_manifest block');
}
const descriptorManifestSha = String(manifest.sha256 ?? '').toLowerCase();
if (!descriptorManifestSha) {
  fail('descriptor missing checksums_manifest.sha256');
}
if (descriptorManifestSha !== manifestSha.toLowerCase()) {
  fail(
    `descriptor manifest digest mismatch (expected ${manifestSha}, found ${manifest.sha256})`,
  );
}
if (manifest.filename && path.basename(manifest.filename) !== path.basename(manifestPath)) {
  fail(
    `descriptor manifest filename mismatch (expected ${path.basename(manifestPath)}, found ${manifest.filename})`,
  );
}

if (archivePath) {
  const archive = descriptor.archive;
  if (!archive) {
    fail('descriptor missing archive block');
  }
  const descriptorArchiveSha = String(archive.sha256 ?? '').toLowerCase();
  if (!descriptorArchiveSha) {
    fail('descriptor missing archive.sha256');
  }
  if (descriptorArchiveSha !== archiveSha.toLowerCase()) {
    fail(
      `descriptor archive digest mismatch (expected ${archiveSha}, found ${archive.sha256})`,
    );
  }
  if (archive.filename && path.basename(archive.filename) !== path.basename(archivePath)) {
    fail(
      `descriptor archive filename mismatch (expected ${path.basename(archivePath)}, found ${archive.filename})`,
    );
  }
}
NODE
  info "descriptor digest check passed"
  if [[ -n "$ARCHIVE_PATH" ]]; then
    info "archive digest check passed for ${ARCHIVE_PATH}"
  fi
fi

info "preview verification complete"
