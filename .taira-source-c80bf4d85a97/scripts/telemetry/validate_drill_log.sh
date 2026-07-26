#!/usr/bin/env bash

set -euo pipefail

LOG_FILE=${1:-ops/drill-log.md}

python3 - <<'PYCODE' "${LOG_FILE}"
from __future__ import annotations
from datetime import date as calendar_date
import errno
import os
import sys
import re
import stat

ALLOWED_STATUSES = {"pass", "fail", "follow-up", "scheduled"}
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
TIME_RE = re.compile(r"^\d{2}:\d{2}Z$")
EXPECTED_COLUMNS = [
    "Date",
    "Scenario",
    "Status",
    "Incident Commander",
    "Scribe",
    "Start (UTC)",
    "End (UTC)",
    "Notes",
    "Follow-up / Incident Link",
]
EXPECTED_SEPARATOR = [
    "------",
    "----------",
    "--------",
    "--------------------",
    "--------",
    "-------------",
    "-----------",
    "-------",
    "---------------------------",
]


class DrillLogPathError(RuntimeError):
    """Raised when an operator-supplied drill-log path is unsafe."""


def fail(message: str) -> None:
    raise DrillLogPathError(message)


def validate_calendar_date(value: str, row_idx: int) -> None:
    if not value or not DATE_RE.match(value):
        raise SystemExit(
            f"row {row_idx} has invalid date '{value}'. Expected YYYY-MM-DD."
        )
    try:
        calendar_date.fromisoformat(value)
    except ValueError:
        raise SystemExit(
            f"row {row_idx} has invalid date '{value}'. Expected a real calendar date."
        )


def validate_utc_time(
    value: str, row_idx: int, column_name: str, *, allow_open_end: bool = False
) -> None:
    if allow_open_end and value == "-":
        return
    if not TIME_RE.match(value):
        suffix = " or -" if allow_open_end else ""
        raise SystemExit(
            f"row {row_idx} has invalid {column_name} time '{value}'. "
            f"Expected HH:MMZ{suffix}."
        )
    hour, minute = map(int, value[:-1].split(":"))
    if hour > 23 or minute > 59:
        raise SystemExit(
            f"row {row_idx} has invalid {column_name} time '{value}'. "
            "Expected a real UTC time."
        )


def display_path(components: list[str]) -> str:
    return "/" + "/".join(components)


def absolute_components(raw_path: str) -> list[str]:
    if not raw_path:
        fail("Error: drill log path must not be empty")
    raw_components = [part for part in raw_path.split("/") if part and part != "."]
    if not raw_components:
        fail("Error: drill log path must name a file")
    if any(part == ".." for part in raw_components):
        fail("Error: drill log parent must not contain parent-directory segments")
    if os.path.isabs(raw_path):
        return raw_components
    return [part for part in os.getcwd().split("/") if part] + raw_components


def dir_open_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def read_open_flags() -> int:
    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)


def parent_component_stat(parent_fd: int, component: str, component_path: str) -> os.stat_result | None:
    try:
        return os.stat(component, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return None
    except OSError as exc:
        fail(f"Error: unable to inspect drill log parent component {component_path}: {exc}")


def open_parent_dir(components: list[str], raw_path: str) -> int:
    fd = os.open("/", dir_open_flags())
    opened_components: list[str] = []
    try:
        for component in components[:-1]:
            opened_components.append(component)
            component_path = display_path(opened_components)
            component_stat = parent_component_stat(fd, component, component_path)
            if component_stat is None:
                fail(f"Error: drill log not found at {raw_path}")
            if stat.S_ISLNK(component_stat.st_mode):
                fail(f"Error: drill log parent must not be a symlink: {component_path}")
            if not stat.S_ISDIR(component_stat.st_mode):
                fail(f"Error: drill log parent component must be a directory: {component_path}")
            try:
                next_fd = os.open(component, dir_open_flags(), dir_fd=fd)
            except OSError as exc:
                if exc.errno in (errno.ELOOP, errno.ENOTDIR):
                    fail(f"Error: drill log parent must not be a symlink: {component_path}")
                fail(f"Error: unable to open drill log parent {component_path}: {exc}")
            os.close(fd)
            fd = next_fd
        return fd
    except Exception:
        os.close(fd)
        raise


def read_drill_log(raw_path: str) -> str:
    components = absolute_components(raw_path)
    parent_fd = open_parent_dir(components, raw_path)
    name = components[-1]
    target_path = display_path(components)
    try:
        try:
            target_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            fail(f"Error: drill log not found at {raw_path}")
        except OSError as exc:
            fail(f"Error: unable to inspect drill log {target_path}: {exc}")
        if stat.S_ISLNK(target_stat.st_mode):
            fail(f"Error: drill log must not be a symlink: {target_path}")
        if not stat.S_ISREG(target_stat.st_mode):
            fail(f"Error: drill log must be a regular file path: {target_path}")
        try:
            fd = os.open(name, read_open_flags(), dir_fd=parent_fd)
        except OSError as exc:
            if exc.errno in (errno.ELOOP, errno.ENOTDIR):
                fail(f"Error: drill log must not be a symlink: {target_path}")
            fail(f"Error: unable to open drill log {target_path}: {exc}")
        try:
            opened_stat = os.fstat(fd)
            if not stat.S_ISREG(opened_stat.st_mode):
                fail(f"Error: drill log must be a regular file path: {target_path}")
            handle = os.fdopen(fd, "r", encoding="utf-8")
            fd = -1
            with handle:
                return handle.read()
        except Exception:
            if fd >= 0:
                os.close(fd)
            raise
    finally:
        os.close(parent_fd)


try:
    lines = read_drill_log(sys.argv[1]).splitlines()
except DrillLogPathError as exc:
    print(exc, file=sys.stderr)
    raise SystemExit(1)

table_rows = [line.strip() for line in lines if line.strip().startswith("|")]

if len(table_rows) < 2:
    raise SystemExit("drill log must contain header and separator rows.")

header = table_rows[0]
columns = [col.strip() for col in header.strip("|").split("|")]
if columns != EXPECTED_COLUMNS:
    raise SystemExit(
        "drill log header mismatch.\n"
        f"Expected: {EXPECTED_COLUMNS}\n"
        f"Found:    {columns}"
    )

separator = table_rows[1]
separator_columns = [col.strip() for col in separator.strip("|").split("|")]
if separator_columns != EXPECTED_SEPARATOR:
    raise SystemExit(
        "drill log separator mismatch.\n"
        f"Expected: {EXPECTED_SEPARATOR}\n"
        f"Found:    {separator_columns}"
    )

entries = table_rows[2:]
for idx, row in enumerate(entries, start=1):
    parts = [col.strip() for col in row.strip("|").split("|")]
    if len(parts) != len(EXPECTED_COLUMNS):
        raise SystemExit(
            f"row {idx} has {len(parts)} columns; expected {len(EXPECTED_COLUMNS)}.\n"
            f"Row content: {row}"
        )
    status = parts[2]
    validate_calendar_date(parts[0], idx)
    if not parts[1]:
        raise SystemExit(f"row {idx} has empty scenario.")
    if status not in ALLOWED_STATUSES:
        raise SystemExit(
            f"row {idx} has invalid status '{parts[2]}'. "
            f"Allowed exact lowercase values: {', '.join(sorted(ALLOWED_STATUSES))}"
        )
    validate_utc_time(parts[5], idx, "start")
    validate_utc_time(parts[6], idx, "end", allow_open_end=True)

print(f"Drill log OK: {len(entries)} entries validated.")
PYCODE
