#!/usr/bin/env bash
set -euo pipefail

# Reconstruct the reviewed DPN validator closure identically on both native
# release targets. The output identity is canonical JSON and is compared byte
# for byte across the Linux/aarch64 and macOS/arm64 jobs.

usage() {
  cat <<'EOF'
Usage: prepare_taira_release_source.sh \
  --workspace PATH \
  --output-dir PATH \
  --validator-release-ref COMMIT \
  --validator-lock-sha256 SHA256
EOF
}

WORKSPACE=""
OUTPUT_DIR=""
VALIDATOR_RELEASE_REF=""
VALIDATOR_LOCK_SHA256=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      WORKSPACE="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --validator-release-ref)
      VALIDATOR_RELEASE_REF="$2"
      shift 2
      ;;
    --validator-lock-sha256)
      VALIDATOR_LOCK_SHA256="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ ! "$VALIDATOR_RELEASE_REF" =~ ^[0-9a-f]{40}$ ]]; then
  echo "validator release ref must be an exact lowercase 40-hex DPN commit" >&2
  exit 1
fi
if [[ ! "$VALIDATOR_LOCK_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "validator lock SHA-256 must be exact lowercase hexadecimal" >&2
  exit 1
fi
if [[ -z "$WORKSPACE" || "$WORKSPACE" != /* || ! -d "$WORKSPACE" || -L "$WORKSPACE" ]]; then
  echo "workspace must be an absolute non-symlink directory" >&2
  exit 1
fi
if [[ "$(cd "$WORKSPACE" && pwd -P)" != "$WORKSPACE" ]]; then
  echo "workspace must use its canonical physical path" >&2
  exit 1
fi
if [[ -z "$OUTPUT_DIR" || "$OUTPUT_DIR" != /* || -e "$OUTPUT_DIR" || -L "$OUTPUT_DIR" ]]; then
  echo "output directory must be a fresh absolute path" >&2
  exit 1
fi
mkdir -m 0700 "$OUTPUT_DIR"

release_dir="$OUTPUT_DIR/dpn-validator-release"
bundle_dir="$release_dir/source-bundle"
mkdir -m 0700 "$release_dir" "$bundle_dir"
base_url="https://raw.githubusercontent.com/soramitsu/dpn-api-rust/${VALIDATOR_RELEASE_REF}"
curl_args=(--proto '=https' --tlsv1.2 --fail --silent --show-error --location --retry 3)
curl "${curl_args[@]}" --output "$release_dir/iroha_source_bundle.py" \
  "$base_url/scripts/iroha_source_bundle.py"
for component in provenance.json tracked.patch untracked.tar untracked.manifest.json source.manifest.json; do
  curl "${curl_args[@]}" --output "$bundle_dir/$component" \
    "$base_url/ops/iroha/taira-validator-source/$component"
done
curl "${curl_args[@]}" --output "$OUTPUT_DIR/Cargo.lock.download" \
  "$base_url/ops/iroha/taira-validator.Cargo.lock"

python3 -I -S - \
  "$OUTPUT_DIR/Cargo.lock.download" \
  "$VALIDATOR_LOCK_SHA256" \
  "$bundle_dir" <<'PY'
import hashlib
import os
import stat
import sys
from pathlib import Path

lock = Path(sys.argv[1])
expected = sys.argv[2]
bundle = Path(sys.argv[3])
for path in [lock, *(bundle / name for name in (
    "provenance.json",
    "tracked.patch",
    "untracked.tar",
    "untracked.manifest.json",
    "source.manifest.json",
))]:
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or stat.S_ISLNK(info.st_mode) or info.st_nlink != 1:
        raise SystemExit(f"downloaded release input is not one regular file: {path}")
    if info.st_size <= 0:
        raise SystemExit(f"downloaded release input is empty: {path}")
if hashlib.sha256(lock.read_bytes()).hexdigest() != expected:
    raise SystemExit("downloaded validator Cargo.lock checksum mismatch")
PY

install -m 0600 "$OUTPUT_DIR/Cargo.lock.download" "$WORKSPACE/Cargo.lock"
python3 "$release_dir/iroha_source_bundle.py" reconstruct \
  --repo "$WORKSPACE" \
  --bundle-dir "$bundle_dir"
python3 "$release_dir/iroha_source_bundle.py" verify \
  --repo "$WORKSPACE" \
  --bundle-dir "$bundle_dir"

workspace_manifest="$(
  python3 -I -S "$WORKSPACE/scripts/compute_workspace_source_manifest.py" \
    --root "$WORKSPACE"
)"
[[ "$workspace_manifest" =~ ^[0-9a-f]{64}$ ]]
git_head="$(git -C "$WORKSPACE" rev-parse HEAD)"
[[ "$git_head" =~ ^[0-9a-f]{40}$ ]]
git -C "$WORKSPACE" verify-commit "$git_head"
git_status="$(git -C "$WORKSPACE" status --short)"
iroha_worktree_clean=False
if [[ -z "$git_status" ]]; then
  iroha_worktree_clean=True
fi
source_date_epoch="$(git -C "$WORKSPACE" show -s --format=%ct "$git_head")"
[[ "$source_date_epoch" =~ ^[0-9]+$ ]]

WORKSPACE_MANIFEST="$workspace_manifest" \
GIT_HEAD="$git_head" \
SOURCE_DATE_EPOCH="$source_date_epoch" \
VALIDATOR_RELEASE_REF="$VALIDATOR_RELEASE_REF" \
VALIDATOR_LOCK_SHA256="$VALIDATOR_LOCK_SHA256" \
IROHA_WORKTREE_CLEAN="$iroha_worktree_clean" \
WORKSPACE="$WORKSPACE" \
BUNDLE_DIR="$bundle_dir" \
OUTPUT_DIR="$OUTPUT_DIR" \
python3 -I -S - <<'PY'
import hashlib
import json
import os
from pathlib import Path

bundle = Path(os.environ["BUNDLE_DIR"])
with (bundle / "provenance.json").open(encoding="utf-8") as stream:
    provenance = json.load(stream)

required = ("source_tree_sha256", "tracked_patch_sha256")
for field in required:
    value = provenance.get(field)
    if not isinstance(value, str) or len(value) != 64 or any(
        char not in "0123456789abcdef" for char in value
    ):
        raise SystemExit(f"DPN source provenance has invalid {field}")

output = Path(os.environ["OUTPUT_DIR"])
build_provenance = {
    "iroha_git_head": os.environ["GIT_HEAD"],
    "iroha_source_attested": True,
    "iroha_source_bundle_provenance_sha256": hashlib.sha256(
        (bundle / "provenance.json").read_bytes()
    ).hexdigest(),
    "iroha_source_tree_sha256": provenance["source_tree_sha256"],
    "iroha_tracked_patch_sha256": provenance["tracked_patch_sha256"],
    "iroha_worktree_clean": os.environ["IROHA_WORKTREE_CLEAN"] == "True",
    "schema_version": 1,
    "validator_lock_sha256": os.environ["VALIDATOR_LOCK_SHA256"],
}
identity = {
    "dpn_validator_release_commit": os.environ["VALIDATOR_RELEASE_REF"],
    "source": {
        "cargo_lock_sha256": os.environ["VALIDATOR_LOCK_SHA256"],
        "commit": os.environ["GIT_HEAD"],
        "workspace_source_manifest_sha256": os.environ["WORKSPACE_MANIFEST"],
    },
    "source_date_epoch": int(os.environ["SOURCE_DATE_EPOCH"]),
}
for name, payload in (
    ("validator-build-provenance-v1.json", build_provenance),
    ("taira-source-identity-v1.json", identity),
):
    path = output / name
    path.write_text(
        json.dumps(payload, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n",
        encoding="ascii",
    )
    path.chmod(0o600)
PY

test "$(
  python3 -I -S "$WORKSPACE/scripts/compute_workspace_source_manifest.py" \
    --root "$WORKSPACE"
)" = "$workspace_manifest"
echo "Taira release source reconstructed: $OUTPUT_DIR/taira-source-identity-v1.json"
