#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

if [[ "${SERDE_GUARD_ALLOW:-0}" == 1 ]]; then
	echo "serde-guard: SERDE_GUARD_ALLOW=1 set; skipping serde denylist enforcement." >&2
	exit 0
fi

if ! command -v python3 >/dev/null 2>&1; then
	echo "error: python3 is required for serde denylist enforcement." >&2
	exit 1
fi

JSON_PATH="docs/source/norito_json_inventory.json"
MD_PATH="docs/source/norito_json_inventory.md"
ALLOWLIST_PATH="scripts/serde_allowlist.txt"

if [[ ! -f "${ALLOWLIST_PATH}" ]]; then
	echo "error: serde allowlist missing at ${ALLOWLIST_PATH}." >&2
	exit 1
fi

resolve_base() {
	if [[ -n "${AGENTS_BASE_REF:-}" ]]; then
		if git rev-parse --verify "${AGENTS_BASE_REF}" >/dev/null 2>&1; then
			echo "${AGENTS_BASE_REF}"
			return
		fi
		echo "warning: AGENTS_BASE_REF=${AGENTS_BASE_REF} not found; falling back to defaults." >&2
	fi

	for candidate in origin/main origin/master main master; do
		if git rev-parse --verify "${candidate}" >/dev/null 2>&1; then
			echo "${candidate}"
			return
		fi
	done

	echo "HEAD^"
}

tmp_json="$(mktemp)"
tmp_md="$(mktemp)"
base_json="$(mktemp)"
trap 'rm -f "${tmp_json}" "${tmp_md}" "${base_json}"' EXIT

python3 scripts/inventory_serde_usage.py --json "${tmp_json}" --md "${tmp_md}" --allowlist "${ALLOWLIST_PATH}"

fail=false

if [[ ! -f "${JSON_PATH}" ]] || ! diff -u -I '^  "generated_at":' -I '^  "root":' -I '^  "command":' "${JSON_PATH}" "${tmp_json}" >/dev/null; then
	echo "serde-guard: ${JSON_PATH} is stale. Regenerate with:" >&2
	echo "  python3 scripts/inventory_serde_usage.py --json ${JSON_PATH} --md ${MD_PATH}" >&2
	diff -u -I '^  "generated_at":' -I '^  "root":' -I '^  "command":' "${JSON_PATH}" "${tmp_json}" 2>/dev/null || true
	fail=true
else
	echo "serde-guard: JSON inventory up to date."
fi

if [[ ! -f "${MD_PATH}" ]] || ! diff -u -I '^_Last refreshed' "${MD_PATH}" "${tmp_md}" >/dev/null; then
	echo "serde-guard: ${MD_PATH} is stale. Regenerate with:" >&2
	echo "  python3 scripts/inventory_serde_usage.py --json ${JSON_PATH} --md ${MD_PATH}" >&2
	diff -u -I '^_Last refreshed' "${MD_PATH}" "${tmp_md}" 2>/dev/null || true
	fail=true
else
	echo "serde-guard: Markdown inventory up to date."
fi

flagged_report="$(
	JSON="${tmp_json}" python3 - <<'PY'
import json
import os
from pathlib import Path

path = Path(os.environ["JSON"])
data = json.loads(path.read_text())
flagged = [m for m in data.get("matches", []) if not m.get("allowed")]
print(len(flagged))
for item in flagged:
    print(f"{item.get('path')}:{item.get('line')} — {item.get('kind')} — {item.get('context')}")
PY
)"

flagged_count="$(printf '%s\n' "${flagged_report}" | head -n1 | tr -d '\r' || echo 0)"
flagged_entries="$(printf '%s\n' "${flagged_report}" | tail -n +2)"

base_ref="$(resolve_base)"
base_commit="$(git merge-base "${base_ref}" HEAD 2>/dev/null || git rev-parse "${base_ref}" 2>/dev/null || true)"
new_findings=""

if [[ -n "${base_commit}" ]] && git show "${base_commit}:${JSON_PATH}" >"${base_json}" 2>/dev/null; then
	new_findings="$(
		HEAD_JSON="${tmp_json}" BASE_JSON="${base_json}" python3 - <<'PY'
import json
import os
from pathlib import Path

def flagged_set(raw: dict) -> set[tuple[str, int, str]]:
    return {
        (item.get("path", ""), int(item.get("line", 0)), item.get("kind", ""))
        for item in raw.get("matches", [])
        if not item.get("allowed")
    }

head = json.loads(Path(os.environ["HEAD_JSON"]).read_text())
base = json.loads(Path(os.environ["BASE_JSON"]).read_text())

for path, line, kind in sorted(flagged_set(head) - flagged_set(base)):
    print(f"{path}:{line} — {kind}")
PY
	)"
fi

if [[ "${flagged_count}" =~ ^[0-9]+$ ]] && [[ "${flagged_count}" -gt 0 ]]; then
	echo "serde-guard: found ${flagged_count} flagged serde/serde_json references:" >&2
	printf '%s\n' "${flagged_entries}" >&2
	if [[ -n "${new_findings}" ]]; then
		echo "New findings relative to ${base_ref}:" >&2
		printf '%s\n' "${new_findings}" >&2
	fi
	fail=true
else
	echo "serde-guard: no production serde/serde_json references detected."
fi

if [[ "${fail}" == "true" ]]; then
	exit 1
fi

echo "serde-guard: OK."
