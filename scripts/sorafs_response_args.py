"""Shared argparse helpers for SoraFS operator scripts."""

from __future__ import annotations

import argparse
import re
import shlex
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from sorafs_path_identity import resolve_path_identity


MAX_RESPONSE_ARGFILE_BYTES = 256 * 1024
MAX_RESPONSE_ARGFILE_DEPTH = 16
MAX_EXPANDED_ARGS = 8192
CANONICAL_DECIMAL_INTEGER_RE = re.compile(r"-?(?:0|[1-9][0-9]*)\Z")


def _require_string_sequence(values: Any, *, label: str) -> Sequence[Any]:
    if (
        isinstance(values, (str, bytes))
        or isinstance(values, Mapping)
        or not isinstance(values, Sequence)
    ):
        raise ValueError(f"{label} must be a sequence of strings")
    return values


def require_expanded_arg_limit(expanded: Sequence[str]) -> None:
    """Reject expanded operator arguments that exceed the shared cap."""

    _require_string_sequence(expanded, label="expanded arguments")
    if len(expanded) > MAX_EXPANDED_ARGS:
        raise ValueError(f"expanded arguments must be <= {MAX_EXPANDED_ARGS}")


class EvidenceArgumentParser(argparse.ArgumentParser):
    """Argument parser with shell-like reviewed response-file support."""

    def convert_arg_line_to_args(self, arg_line: str) -> list[str]:
        """Parse one response-file line, ignoring blank and comment lines."""

        if not isinstance(arg_line, str):
            raise ValueError("response-file line must be a string")
        line = arg_line.strip()
        if not line or line.startswith("#"):
            return []
        return shlex.split(line, comments=True)


def parse_int_arg(value: str) -> int:
    """Parse an integer argparse value with a stable diagnostic."""

    if not isinstance(value, str):
        raise argparse.ArgumentTypeError("must be an integer")
    if not CANONICAL_DECIMAL_INTEGER_RE.fullmatch(value) or value == "-0":
        raise argparse.ArgumentTypeError("must be an integer")
    return int(value)


def positive_int_arg(value: str) -> int:
    """Parse a positive integer argparse value."""

    parsed = parse_int_arg(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def non_negative_int_arg(value: str) -> int:
    """Parse a non-negative integer argparse value."""

    parsed = parse_int_arg(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be non-negative")
    return parsed


def expand_response_args(
    args: Sequence[str],
    parser: argparse.ArgumentParser,
    seen: frozenset[Path] | None = None,
    *,
    depth: int = 0,
) -> list[str]:
    """Expand shell-style @ARGFILE entries before argparse consumes options."""

    if seen is None:
        seen = frozenset()
    if depth > MAX_RESPONSE_ARGFILE_DEPTH:
        raise ValueError(
            f"@ARGFILE nesting depth must be <= {MAX_RESPONSE_ARGFILE_DEPTH}"
        )

    expanded: list[str] = []
    for arg in _require_string_sequence(args, label="arguments"):
        if not isinstance(arg, str):
            raise ValueError(f"argument `{arg}` must be a string")
        if not arg.startswith("@") or arg == "@":
            expanded.append(arg)
            require_expanded_arg_limit(expanded)
            continue
        path = Path(arg[1:]).expanduser()
        resolve_errors: list[str] = []
        resolved = resolve_path_identity(
            path,
            resolve_errors,
            label="@ARGFILE",
            failure_template="failed to resolve @ARGFILE `{path}`: {error}",
        )
        if resolved is None:
            raise ValueError(resolve_errors[-1])
        if resolved in seen:
            raise ValueError(f"recursive @ARGFILE reference `{path}`")
        try:
            if not path.is_file():
                raise ValueError(f"@ARGFILE `{path}` must exist and be a file")
            size = path.stat().st_size
        except (OSError, RuntimeError) as error:
            raise ValueError(f"failed to stat @ARGFILE `{path}`: {error}") from error
        if size > MAX_RESPONSE_ARGFILE_BYTES:
            raise ValueError(
                f"@ARGFILE `{path}` exceeds {MAX_RESPONSE_ARGFILE_BYTES} bytes"
            )
        try:
            contents = path.read_bytes().decode("utf-8")
        except (OSError, RuntimeError) as error:
            raise ValueError(f"failed to read @ARGFILE `{path}`: {error}") from error
        except UnicodeDecodeError as error:
            raise ValueError(f"@ARGFILE `{path}` must be UTF-8: {error}") from error
        file_args: list[str] = []
        for line_number, line in enumerate(contents.splitlines(), 1):
            try:
                line_args = parser.convert_arg_line_to_args(line)
                line_args = _require_string_sequence(
                    line_args, label="response-file line arguments"
                )
                for line_arg in line_args:
                    if not isinstance(line_arg, str):
                        raise ValueError(f"argument `{line_arg}` must be a string")
                file_args.extend(line_args)
            except ValueError as error:
                raise ValueError(
                    f"@ARGFILE `{path}` line {line_number}: {error}"
                ) from error
        expanded.extend(
            expand_response_args(
                file_args,
                parser,
                seen | {resolved},
                depth=depth + 1,
            )
        )
        require_expanded_arg_limit(expanded)
    return expanded
