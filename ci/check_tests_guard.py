#!/usr/bin/env python3
"""
Guardrail that fails when Rust functions change without unit-test coverage
evidence (crate `tests/` or inline `#[cfg(test)]`/`#[test]` blocks).
"""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass
from typing import Dict, Iterable, List, Optional, Sequence, Set, Tuple

DEFAULT_BASE_CANDIDATES = ("origin/main", "origin/master", "main", "master")
DEFAULT_ROOT = pathlib.Path(__file__).resolve().parent.parent

FUNC_DEF_RE = re.compile(
    r"^\s*(?:(?:pub(?:\s*\([^)]*\))?)\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)
TEST_MARKER_RE = re.compile(r"(?:#\[(?:tokio::)?test|cfg\(test\)|mod\s+tests\b)")
TEST_NAME_RE_CACHE: Dict[str, re.Pattern[str]] = {}


@dataclass(frozen=True)
class Violation:
    crate: pathlib.Path
    path: pathlib.Path
    functions: Set[str]


def run_git(args: Sequence[str], *, cwd: pathlib.Path, check: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=check,
    )


def resolve_base_ref(requested: Optional[str], *, root: pathlib.Path = DEFAULT_ROOT) -> str:
    if requested:
        result = run_git(["rev-parse", "--verify", requested], cwd=root)
        if result.returncode == 0:
            return requested
        print(f"warning: AGENTS_BASE_REF={requested} not found; falling back to defaults.", file=sys.stderr)

    for candidate in DEFAULT_BASE_CANDIDATES:
        if run_git(["rev-parse", "--verify", candidate], cwd=root).returncode == 0:
            return candidate

    return "HEAD^"


def resolve_base_commit(base_ref: str, *, root: pathlib.Path = DEFAULT_ROOT) -> str:
    merge_base = run_git(["merge-base", base_ref, "HEAD"], cwd=root)
    if merge_base.returncode == 0 and merge_base.stdout.strip():
        return merge_base.stdout.strip()

    resolved = run_git(["rev-parse", base_ref], cwd=root)
    if resolved.returncode == 0 and resolved.stdout.strip():
        return resolved.stdout.strip()

    return "HEAD"


def changed_rust_files(base_commit: str, *, root: pathlib.Path = DEFAULT_ROOT) -> List[str]:
    result = run_git(["diff", "--name-only", f"{base_commit}...HEAD", "--", "*.rs"], cwd=root)
    return [line for line in result.stdout.splitlines() if line.strip()]


def closest_crate_root(rel_path: pathlib.Path, *, root: pathlib.Path = DEFAULT_ROOT) -> Optional[pathlib.Path]:
    root = root.resolve()
    current = (root / rel_path).parent.resolve()
    while True:
        cargo = current / "Cargo.toml"
        if cargo.exists():
            try:
                content = cargo.read_text(encoding="utf-8")
            except OSError:
                return None
            if "[package]" in content:
                try:
                    return current.relative_to(root)
                except ValueError:
                    return None
        if current == root or current == current.parent:
            return None
        current = current.parent


def file_diff(path: str, base_commit: str, *, root: pathlib.Path = DEFAULT_ROOT) -> List[str]:
    diff = run_git(["diff", "--unified=0", f"{base_commit}...HEAD", "--", path], cwd=root)
    return diff.stdout.splitlines()


def is_test_path(rel_path: pathlib.Path) -> bool:
    parts = rel_path.parts
    return any(part == "tests" for part in parts) or rel_path.name.endswith(("tests.rs", "_test.rs", "_tests.rs"))


def is_prod_file(rel_to_crate: pathlib.Path) -> bool:
    return not any(part in {"tests", "benches", "examples", "target"} for part in rel_to_crate.parts)


def read_text_at_head(path: pathlib.Path) -> Optional[str]:
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return None


def read_text_at_base(path: pathlib.Path, base_commit: str, *, root: pathlib.Path = DEFAULT_ROOT) -> Optional[str]:
    try:
        return subprocess.check_output(["git", "show", f"{base_commit}:{path}"], text=True, cwd=root)
    except subprocess.CalledProcessError:
        return None


def find_test_blocks(lines: Sequence[str]) -> List[Tuple[int, int]]:
    blocks: List[Tuple[int, int]] = []
    for idx, line in enumerate(lines):
        if "mod tests" not in line:
            continue
        window_start = max(idx - 3, 0)
        if not any("cfg(test)" in candidate for candidate in lines[window_start:idx]):
            continue

        brace_depth = 0
        found_open = False
        for end_idx in range(idx, len(lines)):
            brace_depth += lines[end_idx].count("{")
            brace_depth -= lines[end_idx].count("}")
            if "{" in lines[end_idx]:
                found_open = True
            if found_open and brace_depth == 0:
                blocks.append((idx + 1, end_idx + 1))
                break
    return blocks


def extract_test_only_text(content: str) -> str:
    lines = content.splitlines()
    snippets: List[str] = []

    for start, end in find_test_blocks(lines):
        snippets.append("\n".join(lines[start - 1 : end]))

    for idx, line in enumerate(lines):
        stripped = line.strip()
        if not (stripped.startswith("#[test") or stripped.startswith("#[tokio::test")):
            continue

        brace_depth = 0
        found_open = False
        for end_idx in range(idx, len(lines)):
            brace_depth += lines[end_idx].count("{")
            brace_depth -= lines[end_idx].count("}")
            if "{" in lines[end_idx]:
                found_open = True
            if found_open and brace_depth == 0:
                snippets.append("\n".join(lines[idx : end_idx + 1]))
                break

    return "\n".join(snippets)


def find_function_ranges(content: str) -> List[Tuple[str, int, int]]:
    lines = content.splitlines()
    ranges: List[Tuple[str, int, int]] = []
    test_blocks = find_test_blocks(lines)

    for idx, line in enumerate(lines):
        match = FUNC_DEF_RE.match(line)
        if not match:
            continue

        start_line = idx + 1
        if any(start <= start_line <= end for start, end in test_blocks):
            continue

        recent = lines[max(idx - 3, 0) : idx]
        if any(TEST_MARKER_RE.search(candidate) for candidate in recent):
            continue

        brace_depth = 0
        found_open = False
        end_line = len(lines)
        for pos in range(idx, len(lines)):
            brace_depth += lines[pos].count("{")
            brace_depth -= lines[pos].count("}")
            if "{" in lines[pos]:
                found_open = True
            if found_open and brace_depth == 0:
                end_line = pos + 1
                break

        ranges.append((match.group("name"), start_line, end_line))

    return ranges


def parse_changed_lines(diff_lines: Iterable[str]) -> Tuple[List[int], List[int], bool]:
    added: List[int] = []
    removed: List[int] = []
    inline_tests_touched = False
    old_line = 0
    new_line = 0
    hunk_re = re.compile(r"@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@")

    def is_code_line(text: str) -> bool:
        stripped = text.strip()
        if not stripped:
            return False
        return not stripped.startswith(("//", "/*", "*", "///"))

    for raw in diff_lines:
        if raw.startswith("@@"):
            match = hunk_re.match(raw)
            if match:
                old_line = int(match.group(1))
                new_line = int(match.group(2))
            continue

        if raw.startswith("+++ ") or raw.startswith("--- "):
            continue

        if raw.startswith("+"):
            content = raw[1:]
            if TEST_MARKER_RE.search(content):
                inline_tests_touched = True
            if is_code_line(content):
                added.append(new_line)
            new_line += 1
        elif raw.startswith("-"):
            content = raw[1:]
            if is_code_line(content):
                removed.append(old_line)
            old_line += 1
        else:
            old_line += 1
            new_line += 1

    return added, removed, inline_tests_touched


def functions_for_lines(lines: Iterable[int], ranges: Sequence[Tuple[str, int, int]]) -> Set[str]:
    names: Set[str] = set()
    for line in lines:
        for name, start, end in ranges:
            if start <= line <= end:
                names.add(name)
                break
    return names


def collect_test_blobs(crate_root: pathlib.Path) -> str:
    blobs: List[str] = []
    for path in crate_root.rglob("*.rs"):
        if any(part in {"target", "target-codex", "target_codex", "target-snapshot"} for part in path.parts):
            continue

        rel = path.relative_to(crate_root)
        raw_text = read_text_at_head(path) or ""
        include_text = ""
        if is_test_path(rel):
            include_text = raw_text
        else:
            test_only = extract_test_only_text(raw_text)
            if test_only:
                include_text = test_only

        if include_text:
            blobs.append(include_text)

    return "\n".join(blobs)


def function_has_test(name: str, test_blob: str) -> bool:
    if name not in TEST_NAME_RE_CACHE:
        TEST_NAME_RE_CACHE[name] = re.compile(rf"\b{re.escape(name)}\s*\(")
    return bool(TEST_NAME_RE_CACHE[name].search(test_blob))


def gather_violations(base_commit: str, *, root: pathlib.Path = DEFAULT_ROOT) -> List[Violation]:
    root = root.resolve()
    files = changed_rust_files(base_commit, root=root)
    if not files:
        return []

    changed_functions: Dict[pathlib.Path, Dict[pathlib.Path, Set[str]]] = {}

    for file_str in files:
        rel_path = pathlib.Path(file_str)
        crate = closest_crate_root(rel_path, root=root)
        if crate is None:
            continue

        try:
            rel_to_crate = (root / rel_path).resolve().relative_to(root / crate)
        except ValueError:
            continue

        added_lines, removed_lines, _inline = parse_changed_lines(
            file_diff(file_str, base_commit, root=root)
        )

        if not is_prod_file(rel_to_crate):
            continue

        head_text = read_text_at_head(root / rel_path) or ""
        base_text = read_text_at_base(rel_path, base_commit, root=root) or ""
        head_ranges = find_function_ranges(head_text)
        base_ranges = find_function_ranges(base_text)
        head_function_names = {name for name, _, _ in head_ranges}

        names: Set[str] = set()
        names.update(functions_for_lines(added_lines, head_ranges))
        removed_names = functions_for_lines(removed_lines, base_ranges)
        names.update(name for name in removed_names if name in head_function_names)

        if names:
            crate_entry = changed_functions.setdefault(crate, {})
            crate_entry.setdefault(rel_path, set()).update(names)

    if not changed_functions:
        return []

    violations: List[Violation] = []
    test_blob_cache: Dict[pathlib.Path, str] = {}

    for crate, paths in sorted(changed_functions.items(), key=lambda item: str(item[0])):
        if crate not in test_blob_cache:
            test_blob_cache[crate] = collect_test_blobs(root / crate)
        blob = test_blob_cache[crate]

        for path, functions in sorted(paths.items()):
            missing = {fn for fn in functions if not blob or not function_has_test(fn, blob)}
            if missing:
                violations.append(Violation(crate=crate, path=path, functions=missing))

    return violations


def format_violation(violation: Violation) -> str:
    fn_list = ", ".join(sorted(violation.functions))
    return f"{violation.path}: missing coverage for {fn_list}"


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="Git base ref or commit hash to diff against.")
    parser.add_argument(
        "--repo-root",
        help="Override repository root (defaults to repository of this script).",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    if os.environ.get("TEST_GUARD_ALLOW", "0") == "1":
        print("warning: TEST_GUARD_ALLOW=1 set; skipping tests-added guard.", file=sys.stderr)
        return 0

    root = pathlib.Path(args.repo_root).resolve() if args.repo_root else DEFAULT_ROOT
    base_ref = resolve_base_ref(args.base or os.environ.get("AGENTS_BASE_REF"), root=root)
    base_commit = resolve_base_commit(base_ref, root=root)

    violations = gather_violations(base_commit, root=root)
    if not violations:
        return 0

    print(
        "error: changed functions lack unit-test coverage. "
        f"Compared against {base_commit}.",
        file=sys.stderr,
    )
    for violation in violations:
        print(f"  - {format_violation(violation)}", file=sys.stderr)
    print(
        "Add or reference tests in the touched crate (tests/ or #[cfg(test)] modules), "
        "or set TEST_GUARD_ALLOW=1 to bypass temporarily.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
