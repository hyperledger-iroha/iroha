#!/usr/bin/env python3
"""Generate the public ABI-v1 syscall constant table."""

import re
import sys
from pathlib import Path
from typing import Sequence


ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/ivm_abi/src/syscalls.rs"
SPEC = ROOT / "crates/ivm/spec/syscalls.toml"
TABLE_SEPARATOR = re.compile(r"^\|[-: |]+\|$")
CONSTANT = re.compile(r"^pub const (SYSCALL_[A-Za-z0-9_]+): u32 = ([^;]+);")


def load_allowed_numbers(path: Path = SPEC) -> set[int]:
    """Return syscall numbers declared by the canonical ABI-v1 specification."""
    return {
        int(value, 0)
        for value in re.findall(
            r'^number\s*=\s*"([^"]+)"\s*$',
            path.read_text(encoding="utf-8"),
            flags=re.MULTILINE,
        )
    }


def build_rows(src: Path = SRC, spec: Path = SPEC) -> list[str]:
    """Build public table rows while resolving constant aliases deterministically."""
    if not src.exists():
        raise RuntimeError(f"{src} not found")
    allowed_numbers = load_allowed_numbers(spec)
    values: dict[str, int] = {}
    rows: list[str] = []
    for line in src.read_text(encoding="utf-8").splitlines():
        match = CONSTANT.match(line)
        if not match:
            continue
        name, rhs = match.group(1), match.group(2).strip()
        if name.startswith("SYSCALL_KOTO_TEST_"):
            continue
        note = ""
        if rhs.lower().startswith("0x"):
            value = int(rhs.replace("_", ""), 16)
        elif rhs.isdigit():
            value = int(rhs)
        elif rhs.startswith("SYSCALL_"):
            if rhs not in values:
                raise RuntimeError(f"unresolved syscall alias {name} = {rhs}")
            value = values[rhs]
            note = f"alias of {rhs}"
        else:
            raise RuntimeError(f"unsupported syscall expression {name} = {rhs}")
        values[name] = value
        if value in allowed_numbers:
            rows.append(f"| {name} | 0x{value:X} | {note} |")
    return rows


def replace_localized_table(lines: Sequence[str], rows: Sequence[str]) -> list[str]:
    """Replace only the body of one generated localized syscall table."""
    separator = next(
        (index for index, line in enumerate(lines) if TABLE_SEPARATOR.fullmatch(line)),
        None,
    )
    if separator is None:
        raise RuntimeError("no syscall table separator found")
    first = separator + 1
    last = next(
        (index for index in range(first, len(lines)) if not lines[index].strip()),
        len(lines),
    )
    if first == last:
        raise RuntimeError("no syscall rows found")
    return list(lines[:first]) + list(rows) + list(lines[last:])


def sync_localized_tables(rows: Sequence[str], *, check: bool) -> None:
    """Synchronize every localized generated table with the canonical rows."""
    source_dir = ROOT / "docs/source"
    for path in sorted(source_dir.glob("ivm_syscalls_generated.*.md")):
        lines = path.read_text(encoding="utf-8").splitlines()
        try:
            updated = replace_localized_table(lines, rows)
        except RuntimeError as error:
            raise RuntimeError(f"{error} in {path}") from error
        if check:
            if updated != lines:
                raise RuntimeError(
                    f"{path} syscall table is stale; run make docs-syscalls"
                )
        else:
            path.write_text("\n".join(updated) + "\n", encoding="utf-8")


def render(rows: Sequence[str]) -> str:
    """Render the canonical generated Markdown document."""
    lines = [
        "# Generated IVM Syscall Table",
        "",
        "This file is generated from the ABI-v1 syscall specification and "
        "`crates/ivm_abi/src/syscalls.rs`. Edit those sources to change the "
        "surface; then re-run this script.",
        "",
        "| Name | Value (hex) | Note |",
        "|------|-------------|------|",
        *rows,
        "",
        "Note: This table contains only constants in the canonical ABI-v1 "
        "specification; retired and host-private constants are intentionally omitted.",
    ]
    return "\n".join(lines) + "\n"


def main(argv: Sequence[str]) -> int:
    """Run the generator command-line interface."""
    try:
        rows = build_rows()
        if list(argv) == ["--sync-localized"]:
            sync_localized_tables(rows, check=False)
        elif list(argv) == ["--check-localized"]:
            sync_localized_tables(rows, check=True)
        elif argv:
            print(
                "usage: gen_syscall_doc.py [--sync-localized|--check-localized]",
                file=sys.stderr,
            )
            return 2
        else:
            sys.stdout.write(render(rows))
    except (OSError, RuntimeError, ValueError) as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
