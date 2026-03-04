#!/usr/bin/env bash
#
# Compute a deterministic hash tree for documentation directories.
# Usage: scripts/docs/hash_tree.sh <directory> [output.json]

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <directory> [output.json]" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_DIR="$(cd "$1" && pwd)"
OUTPUT_PATH=""
if [[ $# -ge 2 ]]; then
  OUTPUT_PATH="$2"
fi

python3 - <<'PY' "${REPO_ROOT}" "${TARGET_DIR}" "${OUTPUT_PATH}"
import hashlib
import json
import os
import sys
from pathlib import Path

repo_root = Path(sys.argv[1])
target_dir = Path(sys.argv[2])
output_path = sys.argv[3]

if not target_dir.exists():
    raise SystemExit(f"hash_tree: target directory not found: {target_dir}")

entries = []
for entry in sorted(target_dir.rglob("*")):
    if not entry.is_file():
        continue
    # Ignore temporary files that should never ship in docs artefacts.
    if entry.name in {".DS_Store"}:
        continue
    rel_path = entry.relative_to(repo_root).as_posix()
    sha256 = hashlib.sha256(entry.read_bytes()).hexdigest()
    entries.append(
        {
            "path": rel_path,
            "sha256": sha256,
            "size": entry.stat().st_size,
        }
    )

concatenated = "\n".join(f"{item['sha256']}  {item['path']}" for item in entries).encode("utf-8")
tree_sha256 = hashlib.sha256(concatenated).hexdigest()
payload = {
    "root": target_dir.relative_to(repo_root).as_posix(),
    "tree_sha256": tree_sha256,
    "files": entries,
}

if output_path:
    output = Path(output_path)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
else:
    json.dump(payload, sys.stdout, indent=2)
    sys.stdout.write("\n")
PY
