"""Shared argparse helpers for SoraFS operator scripts."""

from __future__ import annotations

import argparse
import os
import re
import shlex
import stat
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from sorafs_path_identity import (
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    resolve_path_identity,
)


MAX_RESPONSE_ARGFILE_BYTES = 256 * 1024
MAX_RESPONSE_ARGFILE_DEPTH = 16
MAX_EXPANDED_ARGS = 8192
CANONICAL_DECIMAL_INTEGER_RE = re.compile(r"-?(?:0|[1-9][0-9]*)\Z")
RESPONSE_ARGFILE_CHUNK_BYTES = 1024 * 1024
ARGFILE_PARENT_SYMLINK_DIAGNOSTIC = "@ARGFILE parent must not be a symlink"
ARGFILE_PARENT_DIRECTORY_DIAGNOSTIC = (
    "@ARGFILE parent must be a directory when it exists"
)
ARGFILE_PARENT_INSPECTION_DIAGNOSTIC = "@ARGFILE parent cannot be inspected"
ARGFILE_SYMLINK_DIAGNOSTIC = "@ARGFILE must not be a symlink"
ARGFILE_MISSING_DIAGNOSTIC = "@ARGFILE must exist and be a file"
ARGFILE_INSPECTION_DIAGNOSTIC = "@ARGFILE cannot be inspected"
ARGFILE_READ_DIAGNOSTIC = "@ARGFILE cannot be read"
ARGFILE_CHANGED_DIAGNOSTIC = "@ARGFILE changed while it was read"
ARGFILE_RESOLUTION_DIAGNOSTIC = "@ARGFILE cannot be resolved"
ARGFILE_RECURSION_DIAGNOSTIC = "recursive @ARGFILE reference"
ARGFILE_UTF8_DIAGNOSTIC = "@ARGFILE must be UTF-8"


def _require_string_sequence(values: Any, *, label: str) -> Sequence[Any]:
    if (
        isinstance(values, (str, bytes, bytearray))
        or isinstance(values, Mapping)
        or not isinstance(values, Sequence)
    ):
        raise ValueError(f"{label} must be a sequence of strings")
    return values


def _require_argument_string(value: Any) -> str:
    if not isinstance(value, str):
        raise ValueError("argument must be a string")
    if not diagnostic_text_is_canonical(value):
        raise ValueError("argument must be a non-empty canonical string")
    return value


def require_expanded_arg_limit(expanded: Sequence[str]) -> None:
    """Reject expanded operator arguments that exceed the shared cap."""

    _require_string_sequence(expanded, label="expanded arguments")
    if len(expanded) > MAX_EXPANDED_ARGS:
        raise ValueError(f"expanded arguments must be <= {MAX_EXPANDED_ARGS}")


def require_equals_form_option_values(
    args: Sequence[str],
    option: str,
    diagnostic: str,
) -> list[str]:
    """Reject split option values for options that require exact equals form."""

    _require_string_sequence(args, label="expanded arguments")
    option_name = _require_argument_string(option)
    diagnostic_text = _require_argument_string(diagnostic)
    for arg in args:
        argument = _require_argument_string(arg)
        if argument == option_name:
            raise ValueError(diagnostic_text)
    return list(args)


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


def _response_argfile_open_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_BINARY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def _response_argfile_directory_open_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def _response_argfile_components(path: Path) -> tuple[str, ...]:
    absolute = path if path.is_absolute() else Path.cwd() / path
    components = absolute.parts[1:]
    if not components or any(
        component in {"", ".", ".."} for component in components
    ):
        raise OSError("response-file path is not canonical")
    return components


def _open_response_argfile_parent(path: Path) -> tuple[int, str, list[int]]:
    components = _response_argfile_components(path)
    directory_fds: list[int] = []
    try:
        current_fd = os.open("/", _response_argfile_directory_open_flags())
        directory_fds.append(current_fd)
        for component in components[:-1]:
            current_fd = os.open(
                component,
                _response_argfile_directory_open_flags(),
                dir_fd=current_fd,
            )
            directory_fds.append(current_fd)
        return current_fd, components[-1], directory_fds
    except BaseException:
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)
        raise


def _response_argfile_path_identity_matches(
    path: Path,
    *,
    expected_parent: os.stat_result,
    expected_leaf: os.stat_result,
) -> bool:
    directory_fds: list[int] = []
    try:
        parent_fd, leaf, directory_fds = _open_response_argfile_parent(path)
        observed_parent = os.fstat(parent_fd)
        observed_leaf = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        return (
            observed_parent.st_dev,
            observed_parent.st_ino,
            observed_leaf.st_dev,
            observed_leaf.st_ino,
        ) == (
            expected_parent.st_dev,
            expected_parent.st_ino,
            expected_leaf.st_dev,
            expected_leaf.st_ino,
        )
    except (OSError, RuntimeError):
        return False
    finally:
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)


def _validate_response_argfile_parent_chain(path: Path) -> None:
    for parent in (path.parent, *path.parent.parents):
        try:
            if parent.is_symlink():
                raise ValueError(ARGFILE_PARENT_SYMLINK_DIAGNOSTIC)
            if parent.exists() and not parent.is_dir():
                raise ValueError(ARGFILE_PARENT_DIRECTORY_DIAGNOSTIC)
        except ValueError:
            raise
        except (OSError, RuntimeError) as error:
            del error
            raise ValueError(ARGFILE_PARENT_INSPECTION_DIAGNOSTIC) from None


def _read_response_argfile_bytes(path: Path) -> bytes:
    try:
        if path.is_symlink():
            raise ValueError(ARGFILE_SYMLINK_DIAGNOSTIC)
        _validate_response_argfile_parent_chain(path)
        if not path.is_file():
            raise ValueError(ARGFILE_MISSING_DIAGNOSTIC)
    except ValueError:
        raise
    except (OSError, RuntimeError) as error:
        del error
        raise ValueError(ARGFILE_INSPECTION_DIAGNOSTIC) from None

    fd = -1
    directory_fds: list[int] = []
    try:
        parent_fd, leaf, directory_fds = _open_response_argfile_parent(path)
        parent_identity = os.fstat(parent_fd)
        before_path = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        fd = os.open(
            leaf,
            _response_argfile_open_flags(),
            dir_fd=parent_fd,
        )
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode):
            raise ValueError(ARGFILE_MISSING_DIAGNOSTIC)
        if (before.st_dev, before.st_ino) != (
            before_path.st_dev,
            before_path.st_ino,
        ):
            raise ValueError(ARGFILE_CHANGED_DIAGNOSTIC)
        if before.st_nlink != 1:
            raise ValueError("@ARGFILE must not be hardlinked")
        if before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
            raise ValueError("@ARGFILE must not be group- or world-writable")
        if before.st_size > MAX_RESPONSE_ARGFILE_BYTES:
            raise ValueError(f"@ARGFILE exceeds {MAX_RESPONSE_ARGFILE_BYTES} bytes")

        chunks: list[bytes] = []
        size = 0
        while True:
            chunk = os.read(
                fd,
                min(
                    RESPONSE_ARGFILE_CHUNK_BYTES,
                    MAX_RESPONSE_ARGFILE_BYTES + 1 - size,
                ),
            )
            if not chunk:
                break
            size += len(chunk)
            if size > MAX_RESPONSE_ARGFILE_BYTES:
                raise ValueError(
                    f"@ARGFILE exceeds {MAX_RESPONSE_ARGFILE_BYTES} bytes"
                )
            chunks.append(chunk)
        after = os.fstat(fd)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(
            getattr(before, field) != getattr(after, field)
            for field in stable_fields
        ):
            raise ValueError(ARGFILE_CHANGED_DIAGNOSTIC)
        if not _response_argfile_path_identity_matches(
            path,
            expected_parent=parent_identity,
            expected_leaf=after,
        ):
            raise ValueError(ARGFILE_CHANGED_DIAGNOSTIC)
    except ValueError:
        raise
    except (OSError, RuntimeError) as error:
        del error
        raise ValueError(ARGFILE_READ_DIAGNOSTIC) from None
    finally:
        if fd >= 0:
            os.close(fd)
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)
    return b"".join(chunks)


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
        arg = _require_argument_string(arg)
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
        )
        if resolved is None:
            # Python 3.12 may consult ``Path.stat()`` while resolving. Preserve
            # the public inspection diagnostic when that probe is what failed.
            try:
                path.stat()
            except FileNotFoundError:
                pass
            except (OSError, RuntimeError):
                raise ValueError(ARGFILE_INSPECTION_DIAGNOSTIC) from None
            raise ValueError(ARGFILE_RESOLUTION_DIAGNOSTIC)
        if resolved in seen:
            raise ValueError(ARGFILE_RECURSION_DIAGNOSTIC)
        try:
            contents = _read_response_argfile_bytes(path).decode("utf-8")
        except UnicodeDecodeError as error:
            del error
            raise ValueError(ARGFILE_UTF8_DIAGNOSTIC) from None
        except ValueError:
            raise
        file_args: list[str] = []
        for line_number, line in enumerate(contents.splitlines(), 1):
            try:
                line_args = parser.convert_arg_line_to_args(line)
                line_args = _require_string_sequence(
                    line_args, label="response-file line arguments"
                )
                for line_arg in line_args:
                    file_args.append(_require_argument_string(line_arg))
            except ValueError as error:
                raise ValueError(
                    "@ARGFILE line {}: {}".format(
                        line_number, error_diagnostic_label(error)
                    )
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
