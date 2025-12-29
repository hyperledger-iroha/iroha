#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

if ! command -v python3 >/dev/null 2>&1; then
	echo "error: python3 is required for dependency discipline guardrails." >&2
	exit 1
fi

resolve_base() {
	if [[ -n "${AGENTS_BASE_REF:-}" ]]; then
		git rev-parse --verify "${AGENTS_BASE_REF}" 2>/dev/null && return
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

base_ref="$(resolve_base)"
base_commit="$(git merge-base "${base_ref}" HEAD 2>/dev/null || git rev-parse "${base_ref}")"

toml_files=()
while IFS= read -r path; do
	[[ -z "${path}" ]] && continue
	toml_files+=("${path}")
done < <(git ls-files '*Cargo.toml' | grep -Ev '^(vendor|patches)/' || true)

if [[ ${#toml_files[@]} -eq 0 ]]; then
	echo "No Cargo.toml files found; skipping dependency discipline guard."
	exit 0
fi

ALLOW_LIST="${DEPENDENCY_DISCIPLINE_ALLOW:-}"

python3 - "$base_commit" "$ALLOW_LIST" "${toml_files[@]}" <<'PY'
import os
import subprocess
import sys
from typing import Dict, Iterable, List, Optional, Set, Tuple

try:
    import tomllib  # type: ignore
except ModuleNotFoundError:  # pragma: no cover - fallback for older interpreters
    import tomli as tomllib  # type: ignore


def load_text(path: str) -> Optional[str]:
    try:
        with open(path, "r", encoding="utf-8") as handle:
            return handle.read()
    except FileNotFoundError:
        return None


def load_from_git(commit: str, path: str) -> Optional[str]:
    try:
        return subprocess.check_output(
            ["git", "show", f"{commit}:{path}"], text=True, stderr=subprocess.DEVNULL
        )
    except subprocess.CalledProcessError:
        return None


def collect_deps(doc: Dict[str, object]) -> Set[str]:
    deps: Set[str] = set()
    sections = ("dependencies", "dev-dependencies", "build-dependencies")

    def collect_from_table(table: object) -> None:
        if isinstance(table, dict):
            deps.update(str(key) for key in table.keys())

    for section in sections:
        collect_from_table(doc.get(section))

    workspace = doc.get("workspace")
    if isinstance(workspace, dict):
        for section in sections:
            collect_from_table(workspace.get(section))

    target = doc.get("target")
    if isinstance(target, dict):
        for cfg_table in target.values():
            if isinstance(cfg_table, dict):
                for section in sections:
                    collect_from_table(cfg_table.get(section))

    patch = doc.get("patch")
    if isinstance(patch, dict):
        for patch_table in patch.values():
            collect_from_table(patch_table)

    return deps


def parse_toml(text: str) -> Dict[str, object]:
    return tomllib.loads(text)


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: check_dependency_discipline.py <base_commit> <allow_csv> <paths...>", file=sys.stderr)
        return 1

    base_commit = sys.argv[1]
    allowed_raw = sys.argv[2]
    paths = sys.argv[3:]

    allowed = {item.strip() for item in allowed_raw.split(",") if item.strip()}

    failures: List[Tuple[str, List[str]]] = []

    for path in paths:
        current_text = load_text(path)
        if current_text is None:
            continue

        base_text = load_from_git(base_commit, path)
        if base_text is None:
            # New crates are already caught by the workspace guard; skip duplicate noise.
            continue

        try:
            current_doc = parse_toml(current_text)
            base_doc = parse_toml(base_text)
        except Exception as exc:  # pragma: no cover - guardrail diagnostics
            print(f"error: failed to parse {path}: {exc}", file=sys.stderr)
            return 1

        added = collect_deps(current_doc) - collect_deps(base_doc) - allowed
        if added:
            failures.append((path, sorted(added)))

    if failures:
        print("error: new dependencies detected; obtain approval before adding third-party crates.", file=sys.stderr)
        for path, deps in failures:
            dep_list = ", ".join(deps)
            print(f" - {path}: {dep_list}", file=sys.stderr)
        print("Set DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2> to acknowledge intentional additions.", file=sys.stderr)
        return 1

    print("Dependency discipline guard passed: no new dependencies added relative to base.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
PY
