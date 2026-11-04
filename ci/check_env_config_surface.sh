#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

# shellcheck source=ci/lib/base_ref.sh
source "${ROOT}/ci/lib/base_ref.sh"

JSON_PATH="specs/agents/env_var_inventory.json"
MD_PATH="specs/agents/env_var_inventory.md"

tmp_json="$(mktemp)"
tmp_md="$(mktemp)"
base_json="$(mktemp)"
trap 'rm -f "${tmp_json}" "${tmp_md}" "${base_json}"' EXIT

python3 scripts/inventory_env_toggles.py --json "${tmp_json}" --md "${tmp_md}"

fail=false

if [[ ! -f "${JSON_PATH}" ]] || ! diff -u "${JSON_PATH}" "${tmp_json}" >/dev/null; then
	echo "env-config guard: env_var_inventory.json is stale. Regenerate with:" >&2
	echo "  python3 scripts/inventory_env_toggles.py --json ${JSON_PATH} --md ${MD_PATH}" >&2
	diff -u "${JSON_PATH}" "${tmp_json}" 2>/dev/null || true
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
				BASE_JSON="${base_json}" HEAD_JSON="${tmp_json}" BASE_COMMIT="${base_commit}" python3 - <<'PY'
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from collections import Counter
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
    loc = path.get("path", "unknown")
    return f"{loc} — {context}" if context else loc


def base_source(name: str) -> bytes | None:
    """Read only an in-repository source's selected-base Git blob."""
    relative = Path(name)
    if (relative.is_absolute() or ".." in relative.parts or not relative.is_file()
            or not relative.resolve().is_relative_to(Path.cwd())):
        return None
    result = subprocess.run(
        ["git", "show", f"{os.environ['BASE_COMMIT']}:{relative.as_posix()}"],
        capture_output=True, check=False,
    )
    return result.stdout if result.returncode == 0 else None


scanner_path = Path("scripts/inventory_env_toggles.py")
scanner_bytes = base_source(scanner_path.as_posix())
scanner_unchanged = scanner_bytes is not None and scanner_bytes == scanner_path.read_bytes()


def inherited_production_references(name: str, rows: list[dict]) -> bool:
    """Reclassify base bytes instead of trusting a stale inventory or changed source."""
    if not scanner_unchanged or not rows:
        return False
    try:
        spec = importlib.util.spec_from_file_location("iroha_env_base_inventory", scanner_path)
        scanner = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = scanner
        spec.loader.exec_module(scanner)
        with tempfile.TemporaryDirectory(prefix="iroha-env-base-") as temporary:
            root = Path(temporary)
            scanner.REPO_ROOT = root
            base_refs = []
            for path in sorted({row.get("path", "") for row in rows}):
                original = base_source(path)
                if original is None:
                    return False
                destination = root / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(original)
                base_refs.extend(scanner.extract_references(destination))
            observed = Counter((ref.path.as_posix(), ref.context.strip())
                               for ref in base_refs if ref.name == name and ref.scope == "prod")
            expected = Counter((row.get("path", ""), row.get("context", "").strip())
                               for row in rows)
            return bool(expected) and all(observed[key] >= count for key, count in expected.items())
    except (ImportError, OSError, ValueError, TypeError, AttributeError):
        return False


# Reconstruct omitted references with the unchanged repository classifier and
# original Git blobs. Every current production reference must retain its exact
# path/context and production classification in the base. Unrelated edits may
# differ; new references, test-to-production moves and classifier changes refuse.
new_prod = []
for name, entry in head.get("variables", {}).items():
    head_prod = prod_count(head, name)
    if head_prod <= 0:
        continue
    base_prod = prod_count(base, name)
    if base_prod <= 0:
        production_rows = [row for row in entry.get("paths", []) if row.get("scope") == "prod"]
        if inherited_production_references(name, production_rows):
            print(f"env-config guard: {name} is already present in reclassified base source.",
                  file=sys.stderr)
            continue
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
