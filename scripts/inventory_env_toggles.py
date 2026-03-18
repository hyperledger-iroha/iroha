#!/usr/bin/env python3
"""
Inventory environment-variable toggles across Rust sources.

The script scans `.rs` files under the repository root for common environment
lookup patterns (`env::var*`, `env!`, `option_env!`) and emits a deterministic
inventory. Use the JSON output for tooling/CI checks and the Markdown output for
human review of production vs test/dev usage.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Dict, Iterable, List, Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent

CFG_TEST_PATTERN = re.compile(r"cfg!?\s*\([^)]*test[^)]*\)", re.IGNORECASE)
CFG_DEBUG_PATTERN = re.compile(r"cfg!?\s*\([^)]*debug_assertions[^)]*\)", re.IGNORECASE)
TEST_PATH_HINTS = {
    "tests",
    "integration_tests",
    "iroha_test_network",
    "iroha_test_samples",
}

# Top-level search roots (falls back to the repo root if none exist).
SEARCH_ROOTS = (
    "crates",
    "xtask",
    "tools",
    "integration_tests",
    "tests",
    "examples",
    "sdk",
    "IrohaSwift",
    "mochi",
    "benchmarks",
    "fuzz",
)

# Directories to skip during the walk (keep the list conservative to avoid
# traversing build artefacts).
SKIP_DIRS = {
    ".git",
    "target",
    "target-codex",
    "target_codex",
    "target-snapshot",
    "target-wallet",
    "build",
    "vendor",
    "artifacts",
    "node_modules",
}

ENV_PATTERNS: Sequence[re.Pattern[str]] = (
    re.compile(r"""\b(?:std::)?env::var(?:_os)?\(\s*"([A-Za-z0-9_]+)"""),
    re.compile(r"""\benv!\(\s*"([A-Za-z0-9_]+)"""),
    re.compile(r"""\boption_env!\(\s*"([A-Za-z0-9_]+)"""),
)
HEURISTIC_LOOKBACK = 200


@dataclasses.dataclass(frozen=True)
class EnvReference:
    name: str
    path: Path
    line: int
    scope: str
    context: str


def iter_rust_files(root: Path) -> Iterable[Path]:
    candidates = [root / entry for entry in SEARCH_ROOTS if (root / entry).exists()]
    if not candidates:
        candidates = [root]

    for base in candidates:
        for path in base.rglob("*.rs"):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            yield path


def classify_scope(path: Path) -> str:
    parts = set(path.parts)
    if path.name == "build.rs":
        return "build"
    if TEST_PATH_HINTS.intersection(parts):
        return "test"
    if "benches" in parts:
        return "bench"
    if "examples" in parts:
        return "example"
    if {"xtask", "tools"}.intersection(parts):
        return "tool"
    return "prod"


def extract_references(path: Path) -> List[EnvReference]:
    refs: List[EnvReference] = []
    scope = classify_scope(path)

    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError:
        return refs

    cfg_masks = compute_cfg_masks(lines)

    for idx, line in enumerate(lines, start=1):
        is_test, is_debug = cfg_masks[idx - 1]
        if is_test:
            local_scope = "test"
        elif is_debug:
            local_scope = "debug"
        else:
            # Heuristic: env accesses guarded by `dev_env_flag` are debug-only.
            recent = lines[max(idx - HEURISTIC_LOOKBACK, 0) : idx - 1]
            if any("dev_env_flag" in candidate for candidate in recent):
                local_scope = "debug"
            else:
                local_scope = scope
        for pattern in ENV_PATTERNS:
            for match in pattern.finditer(line):
                name = match.group(1)
                refs.append(
                    EnvReference(
                        name=name,
                        path=path.relative_to(REPO_ROOT),
                        line=idx,
                        scope=local_scope,
                        context=line.strip(),
                    )
                )
    return refs


def collect_inventory(root: Path) -> List[EnvReference]:
    refs: List[EnvReference] = []
    for file_path in iter_rust_files(root):
        refs.extend(extract_references(file_path))
    refs.sort(key=lambda r: (r.name, str(r.path), r.line))
    return refs


def compute_cfg_masks(lines: Sequence[str]) -> List[tuple[bool, bool]]:
    masks: List[tuple[bool, bool]] = []
    cfg_test_stack: List[bool] = [False]
    cfg_debug_stack: List[bool] = [False]
    pending_cfg_test = False
    pending_cfg_debug = False

    for line in lines:
        stripped = line.strip()
        if CFG_TEST_PATTERN.search(stripped):
            pending_cfg_test = True
        if CFG_DEBUG_PATTERN.search(stripped):
            pending_cfg_debug = True

        current_test = cfg_test_stack[-1] or pending_cfg_test
        current_debug = cfg_debug_stack[-1] or pending_cfg_debug
        masks.append((current_test, current_debug))

        opens = stripped.count("{")
        closes = stripped.count("}")

        for _ in range(opens):
            cfg_test_stack.append(cfg_test_stack[-1] or pending_cfg_test)
            cfg_debug_stack.append(cfg_debug_stack[-1] or pending_cfg_debug)
        if opens and pending_cfg_test:
            pending_cfg_test = False
        if opens and pending_cfg_debug:
            pending_cfg_debug = False
        for _ in range(closes):
            if len(cfg_test_stack) > 1:
                cfg_test_stack.pop()
            if len(cfg_debug_stack) > 1:
                cfg_debug_stack.pop()

    return masks


def build_summary(refs: Sequence[EnvReference]) -> Dict[str, Dict[str, object]]:
    summary: Dict[str, Dict[str, object]] = {}
    by_name: Dict[str, List[EnvReference]] = defaultdict(list)
    for ref in refs:
        by_name[ref.name].append(ref)

    for name, entries in sorted(by_name.items()):
        scope_counts = Counter(ref.scope for ref in entries)
        summary[name] = {
            "counts": dict(sorted(scope_counts.items())),
            "paths": [
                {
                    "path": str(ref.path),
                    "line": ref.line,
                    "scope": ref.scope,
                    "context": ref.context,
                }
                for ref in entries
            ],
        }
    return summary


def write_json(output: Path, refs: Sequence[EnvReference]) -> None:
    data = {
        "generated_at": dt.datetime.utcnow().isoformat(timespec="seconds") + "Z",
        "command": "python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md",
        "root": REPO_ROOT.name,
        "total_refs": len(refs),
        "unique_vars": len({ref.name for ref in refs}),
        "variables": build_summary(refs),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_markdown(output: Path, refs: Sequence[EnvReference]) -> None:
    summary = build_summary(refs)
    lines: List[str] = []
    lines.append("# Environment toggle inventory")
    lines.append("")
    lines.append(
        "_Last refreshed via `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`_"
    )
    lines.append("")
    lines.append(
        f"Total references: **{len(refs)}** · Unique variables: **{len(summary)}**"
    )
    lines.append("")

    for name, data in summary.items():
        counts = data["counts"]
        counts_str = ", ".join(f"{scope}: {count}" for scope, count in counts.items())
        lines.append(f"## {name} ({counts_str})")
        lines.append("")
        for entry in data["paths"]:
            lines.append(
                f"- {entry['scope']}: {entry['path']}:{entry['line']} — `{entry['context']}`"
            )
        lines.append("")

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines), encoding="utf-8")


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Inventory environment-variable toggles across Rust sources."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=REPO_ROOT,
        help="Repository root to scan (defaults to repo root).",
    )
    parser.add_argument(
        "--json",
        dest="json_path",
        type=Path,
        help="Write JSON inventory to this path.",
    )
    parser.add_argument(
        "--md",
        dest="md_path",
        type=Path,
        help="Write Markdown summary to this path.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str]) -> int:
    args = parse_args(argv)
    refs = collect_inventory(args.root)

    if args.json_path:
        write_json(args.json_path, refs)
    if args.md_path:
        write_markdown(args.md_path, refs)

    if not args.json_path and not args.md_path:
        for ref in refs:
            print(f"{ref.name}\t{ref.scope}\t{ref.path}:{ref.line}\t{ref.context}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
