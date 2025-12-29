#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
check_sorafs_gateway_denylist.sh [--evidence-out PATH]

Validates the denylist pack/diff helpers and, when requested, captures the CLI
evidence summary so CI can persist it as build artefacts.

Options:
  --evidence-out PATH   Copy the generated evidence JSON to PATH after
                        validation (directories created automatically).
  -h, --help            Show this message.
USAGE
}

evidence_copy_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --evidence-out)
      [[ $# -lt 2 ]] && { echo "error: --evidence-out requires a path" >&2; usage; exit 1; }
      evidence_copy_path="$2"
      shift 2
      ;;
    --evidence-out=*)
      evidence_copy_path="${1#*=}"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      usage
      exit 1
      ;;
  esac
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SAMPLE_JSON="${ROOT_DIR}/docs/source/sorafs_gateway_denylist_sample.json"

echo "[sorafs-gateway-denylist] verifying bundle + diff tooling"

workdir="$(mktemp -d)"
cleanup() {
  rm -rf "${workdir}"
}
trap cleanup EXIT

old_json="${workdir}/denylist_old.json"
new_json="${workdir}/denylist_new.json"
cp "${SAMPLE_JSON}" "${new_json}"

python3 - <<'PY' "${SAMPLE_JSON}" "${old_json}"
import json, sys
src, dst = sys.argv[1], sys.argv[2]
with open(src, "r", encoding="utf-8") as fh:
    data = json.load(fh)
if len(data) < 2:
    raise SystemExit("sample denylist must contain at least two entries")
with open(dst, "w", encoding="utf-8") as fh:
    json.dump(data[:-1], fh, indent=2)
PY

old_out="${workdir}/bundle_old"
new_out="${workdir}/bundle_new"

echo "[sorafs-gateway-denylist] packing old denylist snapshot"
cargo xtask sorafs-gateway denylist pack \
  --input "${old_json}" \
  --out "${old_out}" \
  --label "ci-old" \
  --force >/dev/null

echo "[sorafs-gateway-denylist] packing new denylist snapshot"
cargo xtask sorafs-gateway denylist pack \
  --input "${new_json}" \
  --out "${new_out}" \
  --label "ci-new" \
  --force >/dev/null

old_bundle="$(ls "${old_out}"/*.json | head -n1)"
new_bundle="$(ls "${new_out}"/*.json | head -n1)"
diff_report="${workdir}/denylist_diff.json"

echo "[sorafs-gateway-denylist] running diff"
cargo xtask sorafs-gateway denylist diff \
  --old "${old_bundle}" \
  --new "${new_bundle}" \
  --report-json "${diff_report}" >/dev/null

if [[ ! -s "${diff_report}" ]]; then
  echo "error: diff report not written to ${diff_report}" >&2
  exit 1
fi

python3 - <<'PY' "${diff_report}"
import json, sys
report = json.load(open(sys.argv[1], "r", encoding="utf-8"))
added = len(report.get("added", []))
removed = len(report.get("removed", []))
if added == 0 or removed == 0:
    raise SystemExit("expected both added and removed entries in diff report")
PY

echo "[sorafs-gateway-denylist] diff evidence ok (${diff_report})"

evidence_json="${workdir}/denylist_evidence.json"
echo "[sorafs-gateway-denylist] generating evidence summary via iroha_cli"
cargo run -p iroha_cli --quiet -- \
  sorafs gateway evidence \
  --denylist "${new_bundle}" \
  --out "${evidence_json}" \
  --label "ci-denylist" >/dev/null

python3 - <<'PY' "${evidence_json}" "${new_bundle}"
import json, sys
evidence_path, denylist_path = sys.argv[1], sys.argv[2]
with open(evidence_path, "r", encoding="utf-8") as fh:
    report = json.load(fh)
with open(denylist_path, "r", encoding="utf-8") as fh:
    entries = json.load(fh)
entry_count = len(entries)
source = report.get("source", {})
if source.get("entry_count") != entry_count:
    raise SystemExit("evidence entry_count mismatch")
totals = report.get("totals", {})
kind_total = sum(item.get("count", 0) for item in totals.get("by_kind", []))
if kind_total != entry_count:
    raise SystemExit("kind totals do not match entry count")
tier_entries = totals.get("by_policy_tier", [])
tier_total = sum(item.get("count", 0) for item in tier_entries)
if tier_total != entry_count:
    raise SystemExit("policy tier totals do not match entry count")
tier_map = {item.get("tier"): item.get("count", 0) for item in tier_entries}
emergency_reviews = report.get("emergency_reviews", [])
if "emergency" in tier_map and tier_map["emergency"] != len(emergency_reviews):
    raise SystemExit("emergency review count mismatch")
PY

echo "[sorafs-gateway-denylist] evidence summary ok (${evidence_json})"

if [[ -n "${evidence_copy_path}" ]]; then
  echo "[sorafs-gateway-denylist] writing evidence copy to ${evidence_copy_path}"
  mkdir -p "$(dirname "${evidence_copy_path}")"
  cp "${evidence_json}" "${evidence_copy_path}"
fi
