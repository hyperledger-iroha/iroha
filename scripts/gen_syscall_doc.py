#!/usr/bin/env python3
"""Generate the public ABI-v1 syscall constant table."""

from __future__ import annotations

import json
import os
import re
import stat
import sys
import tempfile
from pathlib import Path
from typing import NamedTuple, Sequence


ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/ivm_abi/src/syscalls.rs"
SPEC = ROOT / "crates/ivm/spec/syscalls.toml"
CONSTANT = re.compile(r"^pub const (SYSCALL_[A-Za-z0-9_]+): u32 = ([^;]+);$")
HEX_LITERAL = re.compile(r"0x[0-9A-F]+(?:_[0-9A-F]+)*")
DECIMAL_LITERAL = re.compile(r"(?:0|[1-9][0-9]*)")
ASSIGNMENT = re.compile(
    r'^(?P<key>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?P<value>"(?:[^"\\]|\\.)*")$'
)
EXPECTED_FIELDS = frozenset({"number", "args", "ret", "gas"})
DEFAULT_OUTPUT = ROOT / "specs/ivm_syscalls_generated.md"
USAGE = "usage: gen_syscall_doc.py (--write|--check) [--output <path>]"


class GeneratorOptions(NamedTuple):
    """One explicit publication mode and generated-document destination."""

    check: bool
    output: Path


def _parse_syscall_number(value: str, *, line_number: int) -> int:
    if re.fullmatch(r"0x[0-9A-Fa-f]+", value):
        number = int(value[2:], 16)
    elif re.fullmatch(r"[0-9]+", value):
        number = int(value, 10)
    else:
        raise RuntimeError(
            f"syscall spec line {line_number}: malformed syscall number {value!r}"
        )
    if number > 0xFFFF_FFFF:
        raise RuntimeError(
            f"syscall spec line {line_number}: syscall number {value!r} exceeds u32"
        )
    return number


def parse_syscall_spec(text: str) -> list[dict[str, str | int]]:
    """Parse the canonical, deliberately small syscall TOML schema strictly."""

    rows: list[dict[str, str | int]] = []
    current: dict[str, str] | None = None
    current_line = 0

    def finish_row() -> None:
        nonlocal current
        if current is None:
            return
        keys = frozenset(current)
        if keys != EXPECTED_FIELDS:
            missing = sorted(EXPECTED_FIELDS - keys)
            unexpected = sorted(keys - EXPECTED_FIELDS)
            raise RuntimeError(
                "syscall spec row beginning on line "
                f"{current_line} has invalid fields; "
                f"missing={missing}, unexpected={unexpected}"
            )
        if not current["args"] or not current["ret"] or not current["gas"]:
            raise RuntimeError(
                f"syscall spec row beginning on line {current_line} "
                "must define non-empty args, ret, and gas"
            )
        gas = current["gas"]
        gas_token = gas.split(maxsplit=1)[0]
        gas_tokens = re.findall(r"(?<![A-Za-z0-9_])(G[A-Za-z0-9_]*)", gas)
        asset_expression = (
            re.fullmatch(r"G_[a-z0-9_]+", gas_token) is not None
            and bool(gas_tokens)
            and all(re.fullmatch(r"G_[a-z0-9_]+", token) for token in gas_tokens)
        )
        explicit_formula = (
            not gas_tokens
            and gas[0].isdigit()
            and " per " in gas
            and re.fullmatch(r"[0-9A-Za-z ,+\-/()]+", gas) is not None
        )
        if not asset_expression and not explicit_formula:
            raise RuntimeError(
                f"syscall spec row beginning on line {current_line} "
                "must use a canonical G_<name> gas token or bounded numeric "
                "`per` formula"
            )
        number = _parse_syscall_number(
            current["number"], line_number=current_line
        )
        if any(row["number"] == number for row in rows):
            raise RuntimeError(
                f"syscall spec row beginning on line {current_line}: "
                f"duplicate syscall number 0x{number:06X}"
            )
        rows.append(
            {
                "number": number,
                "args": current["args"],
                "ret": current["ret"],
                "gas": current["gas"],
            }
        )
        current = None

    for line_number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped == "[[syscall]]":
            finish_row()
            current = {}
            current_line = line_number
            continue
        if stripped.startswith("["):
            raise RuntimeError(
                f"syscall spec line {line_number}: "
                f"unknown table declaration {stripped!r}"
            )
        match = ASSIGNMENT.fullmatch(stripped)
        if match is None:
            raise RuntimeError(
                f"syscall spec line {line_number}: "
                'expected canonical key = "value" assignment'
            )
        key = match.group("key")
        if key not in EXPECTED_FIELDS:
            raise RuntimeError(
                f"syscall spec line {line_number}: unknown field {key!r}"
            )
        if current is None:
            raise RuntimeError(
                f"syscall spec line {line_number}: "
                f"field {key!r} appears outside [[syscall]]"
            )
        if key in current:
            raise RuntimeError(
                f"syscall spec line {line_number}: duplicate field {key!r}"
            )
        raw_string = match.group("value")
        for escape in re.finditer(r"\\(.)", raw_string[1:-1]):
            if escape.group(1) not in {'"', "\\", "n", "r", "t"}:
                raise RuntimeError(
                    f"syscall spec line {line_number}: "
                    f"unsupported escape {escape.group(0)!r}"
                )
        try:
            value = json.loads(raw_string)
        except json.JSONDecodeError as error:
            raise RuntimeError(
                f"syscall spec line {line_number}: invalid basic string: {error.msg}"
            ) from error
        if not isinstance(value, str):
            raise RuntimeError(
                f"syscall spec line {line_number}: field {key!r} must be a string"
            )
        current[key] = value

    if current is None and not rows:
        raise RuntimeError("syscall spec contains no [[syscall]] rows")
    finish_row()
    return rows


def load_allowed_numbers(path: Path = SPEC) -> set[int]:
    """Return syscall numbers declared by the canonical ABI-v1 specification."""
    return {
        int(row["number"])
        for row in parse_syscall_spec(path.read_text(encoding="utf-8"))
    }


def build_rows(src: Path = SRC, spec: Path = SPEC) -> list[str]:
    """Build public table rows while resolving constant aliases deterministically."""
    if not src.exists():
        raise RuntimeError(f"{src} not found")
    allowed_numbers = load_allowed_numbers(spec)
    values: dict[str, int] = {}
    seen_names: set[str] = set()
    rows: list[str] = []
    for line_number, line in enumerate(
        src.read_text(encoding="utf-8").splitlines(), start=1
    ):
        match = CONSTANT.fullmatch(line)
        if not match:
            if "pub const SYSCALL_" in line:
                raise RuntimeError(
                    f"{src}:{line_number}: noncanonical public syscall constant"
                )
            continue
        name, rhs = match.group(1), match.group(2).strip()
        if name in seen_names:
            raise RuntimeError(f"{src}:{line_number}: duplicate syscall constant {name}")
        seen_names.add(name)
        if name.startswith("SYSCALL_KOTO_TEST_"):
            continue
        note = ""
        if HEX_LITERAL.fullmatch(rhs):
            value = int(rhs.replace("_", ""), 16)
        elif DECIMAL_LITERAL.fullmatch(rhs):
            value = int(rhs)
        elif rhs.startswith("SYSCALL_"):
            if rhs not in values:
                raise RuntimeError(f"unresolved syscall alias {name} = {rhs}")
            value = values[rhs]
            note = f"alias of {rhs}"
        else:
            raise RuntimeError(f"unsupported syscall expression {name} = {rhs}")
        if value > 0xFFFF_FFFF:
            raise RuntimeError(f"syscall constant {name} exceeds u32")
        values[name] = value
        if value in allowed_numbers:
            rows.append(f"| {name} | 0x{value:X} | {note} |")
    missing_numbers = sorted(allowed_numbers - set(values.values()))
    if missing_numbers:
        rendered = ", ".join(f"0x{number:X}" for number in missing_numbers)
        raise RuntimeError(
            f"{src} has no public syscall constant for canonical numbers: {rendered}"
        )
    return rows


def _validate_parent(path: Path) -> None:
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise RuntimeError(f"output parent must be a real directory: {path}")


def _target_mode(path: Path) -> int:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return 0o644
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise RuntimeError(
            f"generated output must be a regular non-symlink file: {path}"
        )
    return stat.S_IMODE(metadata.st_mode)


class PreparedOutput:
    """One validated and fully rendered generated output."""

    __slots__ = ("path", "original", "expected", "mode")

    def __init__(
        self,
        path: Path,
        original: bytes | None,
        expected: bytes,
        mode: int,
    ) -> None:
        self.path = path
        self.original = original
        self.expected = expected
        self.mode = mode


def _prepare_output(path: Path, content: str) -> PreparedOutput:
    _validate_parent(path.parent)
    mode = _target_mode(path)
    try:
        original = path.read_bytes()
    except FileNotFoundError:
        original = None
    # Reject a destination that changed type while it was read.
    _target_mode(path)
    return PreparedOutput(path, original, content.encode("utf-8"), mode)


def _validate_output_set(outputs: Sequence[PreparedOutput]) -> None:
    seen: set[Path] = set()
    for output in outputs:
        if output.path in seen:
            raise RuntimeError(
                f"duplicate generated output destination: {output.path}"
            )
        seen.add(output.path)
        _validate_parent(output.path.parent)
        _target_mode(output.path)
        try:
            current = output.path.read_bytes()
        except FileNotFoundError:
            current = None
        _target_mode(output.path)
        if current != output.original:
            raise RuntimeError(
                f"generated output changed after rendering: {output.path}"
            )


def _stage_output(output: PreparedOutput) -> Path:
    """Write and sync one same-directory temporary without publishing it."""

    path = output.path
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        if hasattr(os, "fchmod"):
            os.fchmod(descriptor, output.mode)
        else:
            os.chmod(temporary, output.mode)
        with os.fdopen(descriptor, "wb") as stream:
            descriptor = -1
            stream.write(output.expected)
            stream.flush()
            os.fsync(stream.fileno())
        return temporary
    except OSError as error:
        temporary.unlink(missing_ok=True)
        raise RuntimeError(
            f"failed to stage atomic output for {path}: {error}"
        ) from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def _sync_prepared_outputs(
    outputs: Sequence[PreparedOutput], *, check: bool
) -> tuple[Path, ...]:
    """Validate the closed set, then check it or atomically publish each file."""

    _validate_output_set(outputs)
    changed = tuple(
        output for output in outputs if output.original != output.expected
    )
    if check:
        if changed:
            stale = ", ".join(str(output.path) for output in changed)
            raise RuntimeError(
                f"generated outputs are stale or missing: {stale}; "
                "run make docs-syscalls"
            )
        return ()
    if not changed:
        return ()

    # Complete every temporary write, flush, and fsync before the first rename.
    # Validation failures are therefore non-mutating. Publication is atomic per
    # file; no cross-file power-loss atomicity is claimed.
    staged: list[tuple[PreparedOutput, Path]] = []
    try:
        for output in changed:
            staged.append((output, _stage_output(output)))
        _validate_output_set(outputs)
        updated: list[Path] = []
        directories: set[Path] = set()
        for output, temporary in staged:
            os.replace(temporary, output.path)
            updated.append(output.path)
            directories.add(output.path.parent)
        if hasattr(os, "O_DIRECTORY"):
            for parent in sorted(directories):
                directory = os.open(parent, os.O_RDONLY | os.O_DIRECTORY)
                try:
                    os.fsync(directory)
                finally:
                    os.close(directory)
        return tuple(updated)
    except OSError as error:
        raise RuntimeError(
            f"failed to atomically publish generated outputs: {error}"
        ) from error
    finally:
        for _, temporary in staged:
            temporary.unlink(missing_ok=True)


def _atomic_write(path: Path, content: str) -> None:
    """Publish one complete UTF-8 file through a synced same-directory rename."""

    _sync_prepared_outputs([_prepare_output(path, content)], check=False)


def sync_canonical_table(
    rows: Sequence[str], *, check: bool, path: Path | None = None
) -> None:
    """Check or atomically publish the canonical generated table."""

    if path is None:
        path = ROOT / "specs/ivm_syscalls_generated.md"
    _sync_prepared_outputs(
        [_prepare_output(path, render(rows))],
        check=check,
    )


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


def parse_args(argv: Sequence[str]) -> GeneratorOptions:
    """Parse one exact mode plus an optional explicit output path."""

    arguments = list(argv)
    check: bool | None = None
    output: Path | None = None
    index = 0
    while index < len(arguments):
        argument = arguments[index]
        if argument in {"--write", "--check"}:
            if check is not None:
                raise ValueError(USAGE)
            check = argument == "--check"
            index += 1
            continue
        if argument == "--output":
            if output is not None or index + 1 >= len(arguments):
                raise ValueError(USAGE)
            value = arguments[index + 1]
            if not value or value.startswith("-"):
                raise ValueError(USAGE)
            output = Path(value)
            index += 2
            continue
        raise ValueError(USAGE)
    if check is None:
        raise ValueError(USAGE)
    return GeneratorOptions(check=check, output=output or DEFAULT_OUTPUT)


def main(argv: Sequence[str]) -> int:
    """Run the generator command-line interface."""
    try:
        options = parse_args(argv)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    try:
        rows = build_rows()
        sync_canonical_table(rows, check=options.check, path=options.output)
    except (OSError, RuntimeError, ValueError) as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
