#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

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

JSON_PATH="docs/source/agents/env_var_inventory.json"
MD_PATH="docs/source/agents/env_var_inventory.md"

tmp_json="$(mktemp)"
tmp_md="$(mktemp)"
base_json="$(mktemp)"
trap 'rm -f "${tmp_json}" "${tmp_md}" "${base_json}"' EXIT

python3 scripts/inventory_env_toggles.py --json "${tmp_json}" --md "${tmp_md}"

fail=false

if [[ ! -f "${JSON_PATH}" ]] || ! diff -u -I '^  "generated_at":' -I '^  "root":' "${JSON_PATH}" "${tmp_json}" >/dev/null; then
	echo "env-config guard: env_var_inventory.json is stale. Regenerate with:" >&2
	echo "  python3 scripts/inventory_env_toggles.py --json ${JSON_PATH} --md ${MD_PATH}" >&2
	diff -u -I '^  "generated_at":' -I '^  "root":' "${JSON_PATH}" "${tmp_json}" 2>/dev/null || true
	fail=true
else
	echo "env-config guard: JSON inventory up to date."
fi

if [[ ! -f "${MD_PATH}" ]] || ! diff -u "${MD_PATH}" "${tmp_md}" >/dev/null; then
	echo "env-config guard: env_var_inventory.md is stale. Regenerate with:" >&2
	echo "  python3 scripts/inventory_env_toggles.py --json ${JSON_PATH} --md ${MD_PATH}" >&2
	diff -u "${MD_PATH}" "${tmp_md}" 2>/dev/null || true
	fail=true
else
	echo "env-config guard: Markdown inventory up to date."
fi

base_ref="$(resolve_base)"
base_commit="$(git merge-base "${base_ref}" HEAD 2>/dev/null || git rev-parse "${base_ref}" 2>/dev/null || true)"

if [[ -n "${base_commit}" ]]; then
	if git show "${base_commit}:${JSON_PATH}" >"${base_json}" 2>/dev/null; then
		if [[ "${ENV_CONFIG_GUARD_ALLOW:-0}" == 1 ]]; then
			echo "env-config guard: ENV_CONFIG_GUARD_ALLOW=1 set; skipping new-production-toggle detection." >&2
		else
			new_prod="$(
				BASE_JSON="${base_json}" HEAD_JSON="${tmp_json}" python3 - <<'PY'
import json
import os
from pathlib import Path

base_path = Path(os.environ["BASE_JSON"])
head_path = Path(os.environ["HEAD_JSON"])

try:
    base = json.loads(base_path.read_text())
    head = json.loads(head_path.read_text())
except Exception:
    raise SystemExit()


def prod_count(data: dict, name: str) -> int:
    try:
        return int(data["variables"].get(name, {}).get("counts", {}).get("prod", 0))
    except Exception:
        return 0


def first_context(entry: dict) -> str:
    paths = entry.get("paths") or []
    if not paths:
        return ""
    path = paths[0]
    if not isinstance(path, dict):
        return ""
    context = path.get("context", "").strip()
    loc = f"{path.get('path', 'unknown')}:{path.get('line', '?')}"
    return f"{loc} — {context}" if context else loc


new_prod = []
for name, entry in head.get("variables", {}).items():
    head_prod = prod_count(head, name)
    if head_prod <= 0:
        continue
    base_prod = prod_count(base, name)
    if base_prod <= 0:
        new_prod.append((name, head_prod, first_context(entry)))

for name, count, context in sorted(new_prod):
    detail = f"{name} (prod refs: {count})"
    if context:
        detail = f"{detail} first seen at {context}"
    print(detail)
PY
			)"

			if [[ -n "${new_prod}" ]]; then
				echo "env-config guard: new production env toggles detected relative to ${base_ref}:" >&2
				echo "${new_prod}" >&2
				echo "Set ENV_CONFIG_GUARD_ALLOW=1 to acknowledge intentional additions (after updating migration docs)." >&2
				fail=true
			else
				echo "env-config guard: no new production env toggles relative to ${base_ref}."
			fi
		fi
	else
		echo "warning: unable to load ${JSON_PATH} at ${base_ref}; skipping new-production-toggle detection." >&2
	fi
else
	echo "warning: could not resolve base commit for env-config guard; skipping new-production-toggle detection." >&2
fi

if [[ "${fail}" == "true" ]]; then
	exit 1
fi

echo "env-config guard: OK."
