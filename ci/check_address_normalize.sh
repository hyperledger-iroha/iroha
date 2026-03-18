#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_PATH="${REPO_ROOT}/fixtures/account/address_vectors.json"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/address-normalize.XXXXXX")"
RAW_PATH="${WORK_DIR}/local_raw.txt"
NORMALIZED_PATH="${WORK_DIR}/normalized.txt"
AUDIT_PATH="${WORK_DIR}/audit.json"

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
        i105_value = encodings.get("i105")
        if isinstance(i105_value, dict):
            i105_value = i105_value.get("string")
        if not i105_value:
            i105_value = encodings.get("compressed")
        if not i105_value:
            raise SystemExit(
                f"local12 case {case.get('case_id')} missing i105/compressed encoding"
            )
        entries.append(i105_value)

payload = ""
if entries:
    payload = "\n".join(entries) + "\n"
raw_path.write_text(payload, encoding="utf-8")
PY

ENTRY_COUNT="$(wc -l < "${RAW_PATH}")"
if [[ "${ENTRY_COUNT}" -eq 0 ]]; then
  echo "[addr-normalize] no local selectors discovered in fixture; selector-free canonical fixture detected."
  exit 0
fi

echo "[addr-normalize] converting ${ENTRY_COUNT} Local selectors..."
cargo run -p iroha_cli -- \
  address normalize \
  --input "${RAW_PATH}" \
  --output "${NORMALIZED_PATH}" \
  --network-prefix 753 \
  --format i105 >/dev/null

echo "[addr-normalize] auditing normalized output..."
cargo run -p iroha_cli -- \
  address audit \
  --input "${NORMALIZED_PATH}" \
  --network-prefix 753 \
  --format json >"${AUDIT_PATH}"

export AUDIT_PATH
python3 <<'PY'
import json
import os
from pathlib import Path

audit_path = Path(os.environ["AUDIT_PATH"])
with audit_path.open("r", encoding="utf-8") as handle:
    report = json.load(handle)

stats = report.get("stats") or {}
errors = int(stats.get("errors", 0))
if errors:
    raise SystemExit(f"address audit reported {errors} parse error(s)")

for entry in report.get("entries", []):
    if entry.get("status") != "parsed":
        continue
    summary = entry.get("summary") or {}
    domain = summary.get("domain") or {}
    if domain.get("kind") == "local12":
        raise SystemExit("address audit reported residual local12 domain entries")
PY

echo "[addr-normalize] completed successfully."
