#!/usr/bin/env python3
"""
Inventory serde/serde_json usage across the workspace.

The script scans Rust sources and Cargo manifests for serde identifiers and
emits JSON and/or Markdown summaries. Production hits are flagged so the
Norito-only policy stays enforceable.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import re
import sys
from pathlib import Path
from typing import Iterable, List, Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent

SEARCH_ROOTS: Sequence[str] = (
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
    "docs",
)

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
    "__pycache__",
    ".idea",
    ".vscode",
}

SOURCE_PATTERNS = {
    # Matches serde derives without tripping on Norito or Json-prefixed derives.
    "derive": re.compile(
        r"#\s*\[derive\([^]]*(?<![A-Za-z])(Serialize|Deserialize)\b[^]]*\)]"
    ),
    "use": re.compile(r"\buse\s+serde\b"),
    "path": re.compile(r"\bserde::"),
    "serde_attr": re.compile(r"#\s*\[\s*serde(?:::|\s*\()"),
    "serde_json": re.compile(r"\bserde_json\b"),
    "serde_with": re.compile(r"\bserde_with\b"),
}

SECTION_RE = re.compile(r"^\s*\[(.+?)\]\s*$")
DEP_RE = re.compile(r"^\s*(serde(?:_json)?|serde_with)\b")
FEATURE_RE = re.compile(r'"(serde(?:_json)?|serde_with)"')


@dataclasses.dataclass(frozen=True)
class Match:
    category: str  # dependency|source|feature
    kind: str
    path: Path
    line: int
    scope: str
    allowed: bool
    reason: str
    context: str

    def to_dict(self) -> dict:
        return {
            "category": self.category,
            "kind": self.kind,
            "path": str(self.path),
            "line": self.line,
            "scope": self.scope,
            "allowed": self.allowed,
            "reason": self.reason,
            "context": self.context,
        }


def load_allowlist(path: Path) -> Sequence[str]:
    if not path.exists():
        return ()
    entries: List[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        entries.append(stripped)
    return tuple(entries)


def is_allowlisted(rel_path: Path, allowlist: Sequence[str]) -> bool:
    as_posix = rel_path.as_posix()
    if as_posix.startswith("crates/norito"):
        return True
    return any(as_posix.startswith(prefix) for prefix in allowlist)


def classify_scope(rel_path: Path) -> str:
    parts = rel_path.parts
    if "tests" in parts:
        return "test"
    if "benches" in parts:
        return "bench"
    if "examples" in parts:
        return "example"
    if "fuzz" in parts:
        return "fuzz"
    if parts and parts[0] == "integration_tests":
        return "integration"
    if parts and parts[0] == "docs":
        return "docs"
    if parts and parts[0] in {"tools", "xtask"}:
        return "tool"
    if parts and parts[0] == "sdk":
        return "sdk"
    if parts and parts[0] == "IrohaSwift":
        return "swift"
    return "prod"


def scope_allows_serde(scope: str) -> bool:
    return scope in {"test", "bench", "example", "fuzz", "integration", "docs", "tool", "sdk", "swift"}


def iter_rust_files(root: Path) -> Iterable[Path]:
    for base in SEARCH_ROOTS:
        base_path = root / base
        if not base_path.exists():
            continue
        for path in base_path.rglob("*.rs"):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            yield path


def iter_cargo_tomls(root: Path) -> Iterable[Path]:
    for base in SEARCH_ROOTS:
        base_path = root / base
        if not base_path.exists():
            continue
        for path in base_path.rglob("Cargo.toml"):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            yield path
    root_manifest = root / "Cargo.toml"
    if root_manifest.exists():
        yield root_manifest


def dependency_role(section: str) -> str:
    if "dev-dependencies" in section:
        return "dev-dependencies"
    if "build-dependencies" in section:
        return "build-dependencies"
    if section.endswith("dependencies"):
        return "dependencies"
    if section.startswith("features"):
        return "features"
    return section or "unknown"


def scan_rust_file(path: Path, allowlist: Sequence[str]) -> List[Match]:
    rel_path = path.relative_to(REPO_ROOT)
    scope = classify_scope(rel_path)
    allowlisted = is_allowlisted(rel_path, allowlist)
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (UnicodeDecodeError, OSError):
        return []
    matches: List[Match] = []
    for lineno, line in enumerate(lines, start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        for kind, pattern in SOURCE_PATTERNS.items():
            if pattern.search(line):
                allowed = allowlisted or scope_allows_serde(scope)
                reason = "allowed scope" if allowed else "production serde usage"
                matches.append(
                    Match(
                        category="source",
                        kind=kind,
                        path=rel_path,
                        line=lineno,
                        scope=scope,
                        allowed=allowed,
                        reason=reason,
                        context=stripped,
                    )
                )
    return matches


def scan_cargo_toml(path: Path, allowlist: Sequence[str]) -> List[Match]:
    rel_path = path.relative_to(REPO_ROOT)
    scope = classify_scope(rel_path)
    allowlisted = is_allowlisted(rel_path, allowlist)
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (UnicodeDecodeError, OSError):
        return []
    matches: List[Match] = []
    section = ""
    for lineno, line in enumerate(lines, start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if m := SECTION_RE.match(stripped):
            section = m.group(1)
            continue
        if m := DEP_RE.match(stripped):
            crate = m.group(1)
            role = dependency_role(section)
            kind = f"{role}:{crate}"
            allowed = allowlisted or scope_allows_serde(scope) or role != "dependencies"
            if allowlisted:
                reason = "allowlisted crate"
            elif role != "dependencies":
                reason = f"{role} usage"
            elif scope_allows_serde(scope):
                reason = "non-production scope"
            else:
                reason = "production dependency"
            matches.append(
                Match(
                    category="dependency",
                    kind=kind,
                    path=rel_path,
                    line=lineno,
                    scope=scope,
                    allowed=allowed,
                    reason=reason,
                    context=stripped,
                )
            )
        elif section.startswith("features"):
            for feat in FEATURE_RE.findall(stripped):
                allowed = allowlisted or scope_allows_serde(scope)
                reason = "feature gate" if allowed else "feature enables serde"
                matches.append(
                    Match(
                        category="feature",
                        kind=f"feature:{feat}",
                        path=rel_path,
                        line=lineno,
                        scope=scope,
                        allowed=allowed,
                        reason=reason,
                        context=stripped,
                    )
                )
    return matches


def collect_inventory(allowlist: Sequence[str]) -> List[Match]:
    matches: List[Match] = []
    for path in iter_rust_files(REPO_ROOT):
        matches.extend(scan_rust_file(path, allowlist))
    for path in iter_cargo_tomls(REPO_ROOT):
        matches.extend(scan_cargo_toml(path, allowlist))
    matches.sort(key=lambda m: (m.allowed, str(m.path), m.line, m.kind))
    return matches


def summarize(matches: Sequence[Match]) -> dict:
    flagged = [m for m in matches if not m.allowed]
    by_scope: dict[str, int] = {}
    by_category: dict[str, int] = {}
    for m in matches:
        by_scope[m.scope] = by_scope.get(m.scope, 0) + 1
        by_category[m.category] = by_category.get(m.category, 0) + 1
    return {
        "total": len(matches),
        "flagged": len(flagged),
        "by_scope": by_scope,
        "by_category": by_category,
    }


def write_json(path: Path, matches: Sequence[Match], allowlist: Sequence[str], command: str) -> None:
    payload = {
        "generated_at": dt.datetime.utcnow().isoformat(timespec="seconds") + "Z",
        "command": command,
        "root": REPO_ROOT.name,
        "allowlist": list(allowlist) if allowlist else [],
        "summary": summarize(matches),
        "matches": [m.to_dict() for m in matches],
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def write_markdown(path: Path, matches: Sequence[Match], allowlist: Sequence[str], command: str) -> None:
    summary = summarize(matches)
    flagged = [m for m in matches if not m.allowed]
    allowed = [m for m in matches if m.allowed]
    allowed_by_scope: dict[str, int] = {}
    for m in allowed:
        allowed_by_scope[m.scope] = allowed_by_scope.get(m.scope, 0) + 1

    lines: List[str] = []
    lines.append("# Serde usage inventory")
    lines.append("")
    lines.append(f"_Last refreshed via `{command}`_")
    lines.append("")
    lines.append(
        f"Total matches: **{summary['total']}** · Flagged (production) matches: **{summary['flagged']}**"
    )
    if allowlist:
        lines.append(f"Allowlist prefixes: {', '.join(allowlist)}")
    else:
        lines.append("Allowlist prefixes: _none_ (crates/norito is implicitly allowed)")
    lines.append("")

    lines.append("## Flagged hotspots")
    if not flagged:
        lines.append("- None detected.")
    else:
        for m in flagged:
            lines.append(
                f"- {m.category}: {m.path}:{m.line} ({m.scope}) — {m.kind} — `{m.context}`"
            )
    lines.append("")

    lines.append("## Allowed references (tests/benches/docs/dev)")
    if not allowed:
        lines.append("- None.")
    else:
        scope_summaries = ", ".join(
            f"{scope}: {count}" for scope, count in sorted(allowed_by_scope.items())
        )
        lines.append(f"- Allowed by scope counts — {scope_summaries}")
        lines.append("- Full details remain in the JSON inventory for audit trails.")
    lines.append("")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines), encoding="utf-8")


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Inventory serde/serde_json usage across the workspace."
    )
    parser.add_argument(
        "--json",
        dest="json_path",
        type=Path,
        help="Write JSON inventory to the given path.",
    )
    parser.add_argument(
        "--md",
        dest="md_path",
        type=Path,
        help="Write Markdown summary to the given path.",
    )
    parser.add_argument(
        "--allowlist",
        dest="allowlist_path",
        type=Path,
        default=REPO_ROOT / "scripts" / "serde_allowlist.txt",
        help="Path to the serde allowlist (default: scripts/serde_allowlist.txt).",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str]) -> int:
    args = parse_args(argv)
    allowlist = load_allowlist(args.allowlist_path)
    matches = collect_inventory(allowlist)

    command = "python3 scripts/inventory_serde_usage.py"
    if args.json_path:
        command += f" --json {args.json_path}"
    if args.md_path:
        command += f" --md {args.md_path}"

    if args.json_path:
        write_json(args.json_path, matches, allowlist, command)
    if args.md_path:
        write_markdown(args.md_path, matches, allowlist, command)

    if not args.json_path and not args.md_path:
        summary = summarize(matches)
        print(f"Total matches: {summary['total']} (flagged: {summary['flagged']})")
        for m in matches:
            status = "allowed" if m.allowed else "FLAGGED"
            print(f"{status}\t{m.category}\t{m.path}:{m.line}\t{m.kind}\t{m.context}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
