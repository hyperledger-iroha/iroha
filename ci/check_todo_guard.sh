#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

if [[ "${TODO_GUARD_ALLOW:-0}" == 1 ]]; then
	echo "warning: TODO_GUARD_ALLOW=1 set; skipping TODO drop guard." >&2
	exit 0
fi

if ! command -v python3 >/dev/null 2>&1; then
	echo "error: python3 is required for the TODO guard." >&2
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

base_ref="$(resolve_base)"
base_commit="$(git merge-base "${base_ref}" HEAD 2>/dev/null || git rev-parse "${base_ref}" || true)"
if [[ -z "${base_commit}" ]]; then
	base_commit="HEAD"
fi

python3 - "${base_commit}" <<'PY'
import pathlib
import re
import subprocess
import sys
from typing import Dict, Iterable, List, Tuple

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASE = sys.argv[1]
TODO_RE = re.compile(r"todo", re.IGNORECASE)


def changed_files() -> List[pathlib.Path]:
    result = subprocess.run(
        ["git", "diff", "--name-only", f"{BASE}...HEAD"],
        capture_output=True,
        text=True,
        check=False,
    )
    return [
        ROOT / line.strip()
        for line in result.stdout.splitlines()
        if line.strip()
    ]


def is_prod_file(path: pathlib.Path) -> bool:
    parts = set(path.parts)
    return not parts.intersection({"tests", "benches", "examples", "target"})


def is_test_or_doc(path: pathlib.Path) -> bool:
    if "tests" in path.parts:
        return True
    name = path.name
    if name.endswith(("_test.rs", "_tests.rs", "tests.rs")):
        return True
    return path.suffix.lower() in {".md", ".rst"}


def todo_deltas(path: pathlib.Path) -> Tuple[List[Tuple[int, str]], int]:
    diff = subprocess.run(
        ["git", "diff", "--unified=0", f"{BASE}...HEAD", "--", str(path.relative_to(ROOT))],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()

    removed: List[Tuple[int, str]] = []
    added = 0
    old_line = 0
    new_line = 0

    for line in diff:
        if line.startswith("+++ ") or line.startswith("--- "):
            continue
        if line.startswith("@@ "):
            match = re.search(
                r"-([0-9]+)(?:,([0-9]+))? \+([0-9]+)(?:,([0-9]+))?", line
            )
            if match:
                old_line = int(match.group(1)) - 1
                new_line = int(match.group(3)) - 1
            continue
        if line.startswith("-") and not line.startswith("---"):
            old_line += 1
            text = line[1:]
            if TODO_RE.search(text):
                removed.append((old_line, text.strip()))
            continue
        if line.startswith("+") and not line.startswith("+++"):
            new_line += 1
            if TODO_RE.search(line[1:]):
                added += 1
            continue
        if line.startswith(" "):
            old_line += 1
            new_line += 1

    return removed, added


def main() -> None:
    files = changed_files()
    if not files:
        return

    tests_or_docs_touched = any(is_test_or_doc(path) for path in files)
    violations: Dict[pathlib.Path, List[Tuple[int, str]]] = {}

    for path in files:
        if path.suffix != ".rs":
            continue
        if not is_prod_file(path.relative_to(ROOT)):
            continue

        removed, added = todo_deltas(path)
        net_removed = len(removed) - added
        if net_removed > 0:
            violations[path] = removed

    if violations and not tests_or_docs_touched:
        print(
            "error: TODO markers were removed without accompanying docs/tests. "
            f"Compared against {BASE}.",
            file=sys.stderr,
        )
        for path, entries in sorted(violations.items()):
            for line_no, text in entries:
                print(f"  - {path.relative_to(ROOT)}:{line_no}: {text}", file=sys.stderr)
        print(
            "Add or update tests/docs alongside TODO removals, or set TODO_GUARD_ALLOW=1 "
            "to acknowledge intentional removal.",
            file=sys.stderr,
        )
        sys.exit(1)


if __name__ == "__main__":
    main()
PY
