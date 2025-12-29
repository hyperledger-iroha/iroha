#!/usr/bin/env python3

"""Generate an inventory of crates missing crate-level docs or using allow(missing_docs)."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
from typing import Iterable, List, Optional, Sequence, Tuple

ROOT = pathlib.Path(__file__).resolve().parent.parent

ALLOW_RE = re.compile(r"allow\s*\(\s*missing_docs\s*\)", re.IGNORECASE)


def iter_crate_tomls() -> Iterable[pathlib.Path]:
    """Yield Cargo.toml files that define a package (ignores virtual workspaces)."""
    result = subprocess.run(
        ["git", "ls-files", "*Cargo.toml"],
        capture_output=True,
        text=True,
        check=False,
    )
    for line in result.stdout.splitlines():
        path = ROOT / line.strip()
        if not path.is_file():
            continue
        parts = path.parts
        if "vendor" in parts or any(part.startswith("target") for part in parts):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except OSError:
            continue
        if "[package]" not in text:
            continue
        yield path


def parse_crate_name(toml_text: str) -> Optional[str]:
    """Best-effort parse of the crate name without pulling in external deps."""
    try:
        import tomllib  # type: ignore[attr-defined]
    except ModuleNotFoundError:  # pragma: no cover
        tomllib = None

    if tomllib:
        try:
            data = tomllib.loads(toml_text)
            package = data.get("package")
            if isinstance(package, dict):
                name = package.get("name")
                if isinstance(name, str):
                    return name
        except Exception:
            pass

    in_package = False
    for raw_line in toml_text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[package"):
            in_package = True
            continue
        if in_package and line.startswith("["):
            break
        if in_package and line.startswith("name"):
            parts = line.split("=", 1)
            if len(parts) == 2:
                name = parts[1].strip().strip('"').strip("'")
                if name:
                    return name
    return None


def crate_has_docs(crate_root: pathlib.Path) -> bool:
    """Check for crate-level //! or #![doc] comments near the top of lib/main."""
    candidates = [
        crate_root / "src" / "lib.rs",
        crate_root / "src" / "main.rs",
    ]
    saw_root = False
    for candidate in candidates:
        if not candidate.exists():
            continue
        saw_root = True
        try:
            lines = candidate.read_text(encoding="utf-8").splitlines()
        except OSError:
            continue
        head = lines[:80]
        if any(
            line.lstrip().startswith("//!")
            or line.lstrip().startswith("#![doc")
            for line in head
        ):
            return True
    # If no src root exists, treat it as documented to avoid false positives.
    return not saw_root


def allow_sites(crate_root: pathlib.Path) -> List[Tuple[pathlib.Path, int, str]]:
    """Return locations of allow(missing_docs) within a crate."""
    sites: List[Tuple[pathlib.Path, int, str]] = []
    for path in sorted(crate_root.rglob("*.rs")):
        if any(part in {"target", "tests", "benches", "examples"} for part in path.parts):
            continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError:
            continue
        for idx, line in enumerate(lines, start=1):
            if ALLOW_RE.search(line):
                sites.append((path.relative_to(ROOT), idx, line.strip()))
    return sites


def build_inventory() -> Tuple[List[dict], List[str], int]:
    crates = []
    entries = []
    missing_crate_docs = 0
    allow_count = 0

    for toml_path in sorted(iter_crate_tomls(), key=lambda p: p.as_posix()):
        crate_root = toml_path.parent
        try:
            toml_text = toml_path.read_text(encoding="utf-8")
        except OSError:
            toml_text = ""

        name = parse_crate_name(toml_text) or toml_path.parent.name
        missing_docs = not crate_has_docs(crate_root)
        allows = allow_sites(crate_root)

        crates.append(crate_root)
        if missing_docs or allows:
            missing_crate_docs += int(missing_docs)
            allow_count += len(allows)
            entries.append(
                {
                    "crate": str(crate_root.relative_to(ROOT)),
                    "name": name,
                    "missing_crate_docs": missing_docs,
                    "allow_missing_docs": [
                        {"path": str(path), "line": line, "context": context}
                        for path, line, context in allows
                    ],
                }
            )

    entries.sort(key=lambda entry: entry["crate"])
    return entries, [str(crate.relative_to(ROOT)) for crate in crates], allow_count


def write_json(path: pathlib.Path, entries: Sequence[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(list(entries), indent=2) + "\n", encoding="utf-8")


def write_markdown(
    path: pathlib.Path, entries: Sequence[dict], crates_scanned: int, allow_count: int
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    missing_docs = sum(1 for entry in entries if entry["missing_crate_docs"])

    lines = [
        "# Missing Docs Inventory",
        "",
        "Generated by `scripts/inventory_missing_docs.py`. Do not hand-edit; "
        "run the script after touching crates to refresh the inventory.",
        "",
        f"- Crates scanned: {crates_scanned}",
        f"- Crates needing attention: {len(entries)}",
        f"- Missing crate-level docs: {missing_docs}",
        f"- `allow(missing_docs)` occurrences: {allow_count}",
        "",
    ]

    if not entries:
        lines.append("The inventory is clean—no crates are missing crate-level docs and "
                     "`allow(missing_docs)` does not appear in production sources.")
        lines.append("")
        path.write_text("\n".join(lines), encoding="utf-8")
        return

    lines.append("## Entries")
    lines.append("")
    for entry in entries:
        lines.append(
            f"- `{entry['name']}` ({entry['crate']}): "
            f"{'missing crate docs; ' if entry['missing_crate_docs'] else ''}"
            f"{len(entry['allow_missing_docs'])} allow(missing_docs) site(s)"
        )
        if entry["allow_missing_docs"]:
            for site in entry["allow_missing_docs"]:
                lines.append(
                    f"  - {site['path']}:{site['line']} — {site['context']}"
                )
        lines.append("")

    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--json",
        default="docs/source/agents/missing_docs_inventory.json",
        type=pathlib.Path,
        help="Path to write the JSON inventory (default: %(default)s)",
    )
    parser.add_argument(
        "--md",
        default="docs/source/agents/missing_docs_inventory.md",
        type=pathlib.Path,
        help="Path to write the Markdown summary (default: %(default)s)",
    )
    args = parser.parse_args()

    entries, crates, allow_count = build_inventory()
    write_json(ROOT / args.json, entries)
    write_markdown(ROOT / args.md, entries, len(crates), allow_count)


if __name__ == "__main__":
    main()
