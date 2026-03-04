#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_PATH="${REPO_ROOT}/fixtures/account/address_vectors.json"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/address-normalize.XXXXXX")"
RAW_PATH="${WORK_DIR}/local_raw.txt"
NORMALIZED_PATH="${WORK_DIR}/normalized.txt"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

if [[ ! -f "${FIXTURE_PATH}" ]]; then
  echo "[addr-normalize] missing fixture: ${FIXTURE_PATH}" >&2
  exit 1
fi

export FIXTURE_PATH RAW_PATH

python3 <<'PY'
import json
import os
from pathlib import Path

fixture = Path(os.environ["FIXTURE_PATH"])
raw_path = Path(os.environ["RAW_PATH"])

with fixture.open("r", encoding="utf-8") as handle:
    data = json.load(handle)

entries = []
for group in data.get("cases", {}).values():
    if not isinstance(group, list):
        continue
    for case in group:
        selector = case.get("selector") or {}
        if selector.get("kind") != "local12":
            continue
        encodings = case.get("encodings") or {}
        ih58_value = encodings.get("ih58")
        if isinstance(ih58_value, dict):
            ih58_value = ih58_value.get("string")
        if not ih58_value:
            raise SystemExit(
                f"local12 case {case.get('case_id')} missing ih58 encoding"
            )
        inputs = case.get("input") or {}
        domain = (
            inputs.get("normalized_domain")
            or inputs.get("equivalent_domain")
            or inputs.get("raw_domain")
            or inputs.get("domain")
        )
        if not domain:
            raise SystemExit(
                f"local12 case {case.get('case_id')} missing domain metadata"
            )
        entries.append(f"{ih58_value}@{domain}")

if not entries:
    raise SystemExit("no local12 selectors discovered in fixture")

raw_path.write_text("\n".join(entries) + "\n", encoding="utf-8")
PY

echo "[addr-normalize] converting $(wc -l < "${RAW_PATH}") Local selectors..."
cargo run -p iroha_cli -- \
  address normalize \
  --input "${RAW_PATH}" \
  --output "${NORMALIZED_PATH}" \
  --only-local \
  --append-domain \
  --network-prefix 753 \
  --format ih58 >/dev/null

echo "[addr-normalize] auditing normalized output..."
cargo run -p iroha_cli -- \
  address audit \
  --input "${NORMALIZED_PATH}" \
  --network-prefix 753 \
  --fail-on-warning >/dev/null

echo "[addr-normalize] completed successfully."
