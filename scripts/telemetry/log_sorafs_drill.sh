#!/usr/bin/env bash

set -euo pipefail

print_usage() {
    cat <<'USAGE'
Usage: log_sorafs_drill.sh --scenario <name> --status <pass|fail|follow-up|scheduled> \
  [--date YYYY-MM-DD] [--start HH:MMZ] [--end HH:MMZ] \
  [--ic NAME] [--scribe NAME] [--link URL] [--notes "free-form notes"] \
  [--log /custom/path/drill-log.md]

Appends a Markdown row to ops/drill-log.md so chaos drills and incident
exercises remain traceable. The script creates the log file with a header when
it does not yet exist.
USAGE
}

escape_markdown_table_cell() {
    local value="$1"
    value="${value//$'\r'/ }"
    value="${value//$'\n'/ }"
    value="${value//|/\&#124;}"
    printf '%s' "${value}"
}

append_drill_log_entry() {
    local log_path="$1"
    local date_value="$2"
    local scenario_value="$3"
    local status_value="$4"
    local ic_value="$5"
    local scribe_value="$6"
    local start_value="$7"
    local end_value="$8"
    local notes_value="$9"
    local link_value="${10}"

    python3 - \
        "${log_path}" \
        "${date_value}" \
        "${scenario_value}" \
        "${status_value}" \
        "${ic_value}" \
        "${scribe_value}" \
        "${start_value}" \
        "${end_value}" \
        "${notes_value}" \
        "${link_value}" <<'PYCODE'
from __future__ import annotations
from datetime import date as calendar_date
import errno
import os
import re
import stat
import sys

HEADER = """---
title: SoraFS Chaos Drill Log
summary: Registry of executed chaos drills and incident rehearsals.
---

| Date | Scenario | Status | Incident Commander | Scribe | Start (UTC) | End (UTC) | Notes | Follow-up / Incident Link |
|------|----------|--------|--------------------|--------|-------------|-----------|-------|---------------------------|
"""
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
TIME_RE = re.compile(r"^\d{2}:\d{2}Z$")


class DrillLogPathError(RuntimeError):
    """Raised when an operator-supplied drill-log path is unsafe."""


def fail(message: str) -> None:
    raise DrillLogPathError(message)


def validate_date(value: str) -> None:
    if not DATE_RE.match(value):
        fail(f"error: --date must use YYYY-MM-DD; got {value!r}")
    try:
        calendar_date.fromisoformat(value)
    except ValueError:
        fail(f"error: --date is not a valid calendar date: {value!r}")


def validate_time(value: str, flag_name: str, *, allow_open_end: bool = False) -> None:
    if allow_open_end and value == "-":
        return
    if not TIME_RE.match(value):
        fail(f"error: {flag_name} must use HH:MMZ; got {value!r}")
    hour, minute = map(int, value[:-1].split(":"))
    if hour > 23 or minute > 59:
        fail(f"error: {flag_name} is not a valid UTC time: {value!r}")


def display_path(components: list[str]) -> str:
    return "/" + "/".join(components)


def absolute_components(raw_path: str) -> list[str]:
    if not raw_path:
        fail("error: drill log path must not be empty")
    raw_components = [part for part in raw_path.split("/") if part and part != "."]
    if not raw_components:
        fail("error: drill log path must name a file")
    if any(part == ".." for part in raw_components):
        fail("error: drill log parent must not contain parent-directory segments")
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


def write_open_flags() -> int:
    return (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_APPEND
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def parent_component_stat(parent_fd: int, component: str, component_path: str) -> os.stat_result | None:
    try:
        return os.stat(component, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return None
    except OSError as exc:
        fail(f"error: unable to inspect drill log parent component {component_path}: {exc}")


def open_parent_dir(components: list[str]) -> int:
    fd = os.open("/", dir_open_flags())
    opened_components: list[str] = []
    try:
        for component in components[:-1]:
            opened_components.append(component)
            component_path = display_path(opened_components)
            component_stat = parent_component_stat(fd, component, component_path)
            if component_stat is None:
                try:
                    os.mkdir(component, 0o777, dir_fd=fd)
                except FileExistsError:
                    component_stat = parent_component_stat(fd, component, component_path)
                except OSError as exc:
                    fail(f"error: unable to create drill log parent {component_path}: {exc}")
            if component_stat is None:
                component_stat = parent_component_stat(fd, component, component_path)
            if component_stat is None:
                fail(f"error: drill log parent directory not found: {component_path}")
            if stat.S_ISLNK(component_stat.st_mode):
                fail(f"error: drill log parent must not be a symlink: {component_path}")
            if not stat.S_ISDIR(component_stat.st_mode):
                fail(f"error: drill log parent component must be a directory: {component_path}")
            try:
                next_fd = os.open(component, dir_open_flags(), dir_fd=fd)
            except OSError as exc:
                if exc.errno in (errno.ELOOP, errno.ENOTDIR):
                    fail(f"error: drill log parent must not be a symlink: {component_path}")
                fail(f"error: unable to open drill log parent {component_path}: {exc}")
            os.close(fd)
            fd = next_fd
        return fd
    except Exception:
        os.close(fd)
        raise


def open_log_file(raw_path: str) -> tuple[int, int, bool]:
    components = absolute_components(raw_path)
    parent_fd = open_parent_dir(components)
    name = components[-1]
    target_path = display_path(components)
    try:
        try:
            target_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            target_stat = None
        except OSError as exc:
            fail(f"error: unable to inspect drill log {target_path}: {exc}")
        if target_stat is not None:
            if stat.S_ISLNK(target_stat.st_mode):
                fail(f"error: drill log must not be a symlink: {target_path}")
            if not stat.S_ISREG(target_stat.st_mode):
                fail(f"error: drill log must be a regular file path: {target_path}")
        try:
            fd = os.open(name, write_open_flags(), 0o666, dir_fd=parent_fd)
        except OSError as exc:
            if exc.errno in (errno.ELOOP, errno.ENOTDIR):
                fail(f"error: drill log must not be a symlink: {target_path}")
            fail(f"error: unable to open drill log {target_path}: {exc}")
        try:
            opened_stat = os.fstat(fd)
            if not stat.S_ISREG(opened_stat.st_mode):
                fail(f"error: drill log must be a regular file path: {target_path}")
            had_content = opened_stat.st_size > 0
            return parent_fd, fd, had_content
        except Exception:
            os.close(fd)
            raise
    except Exception:
        os.close(parent_fd)
        raise


def write_all(fd: int, payload: bytes) -> None:
    view = memoryview(payload)
    while view:
        written = os.write(fd, view)
        if written == 0:
            fail("error: drill log write made no progress")
        view = view[written:]


def sync_parent(parent_fd: int) -> None:
    try:
        os.fsync(parent_fd)
    except OSError:
        pass


def main() -> int:
    log_path, date, scenario, status_value, ic, scribe, start, end, notes, link = sys.argv[1:]
    validate_date(date)
    validate_time(start, "--start")
    validate_time(end, "--end", allow_open_end=True)
    entry = (
        f"| {date} | {scenario} | {status_value} | {ic} | {scribe} | "
        f"{start} | {end} | {notes} | {link} |\n"
    )
    parent_fd, fd, had_content = open_log_file(log_path)
    try:
        payload = entry if had_content else HEADER + entry
        write_all(fd, payload.encode("utf-8"))
        os.fsync(fd)
        sync_parent(parent_fd)
    finally:
        os.close(fd)
        os.close(parent_fd)
    return 0


try:
    raise SystemExit(main())
except DrillLogPathError as exc:
    print(exc, file=sys.stderr)
    raise SystemExit(1)
PYCODE
}

SCENARIO=""
STATUS=""
DATE_OVERRIDE=""
START_TIME=""
END_TIME=""
IC_NAME=""
SCRIBE_NAME=""
NOTES=""
LINK=""
LOG_OVERRIDE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --scenario)
            SCENARIO=$2
            shift 2
            ;;
        --status)
            STATUS=$2
            shift 2
            ;;
        --date)
            DATE_OVERRIDE=$2
            shift 2
            ;;
        --start)
            START_TIME=$2
            shift 2
            ;;
        --end)
            END_TIME=$2
            shift 2
            ;;
        --ic)
            IC_NAME=$2
            shift 2
            ;;
        --scribe)
            SCRIBE_NAME=$2
            shift 2
            ;;
        --notes)
            NOTES=$2
            shift 2
            ;;
        --link)
            LINK=$2
            shift 2
            ;;
        --log)
            LOG_OVERRIDE=$2
            shift 2
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            print_usage
            exit 1
            ;;
    esac
done

if [[ -z "${SCENARIO}" || -z "${STATUS}" ]]; then
    echo "Error: --scenario and --status are required." >&2
    print_usage
    exit 1
fi

if [[ "${STATUS}" != "pass" && "${STATUS}" != "fail" && "${STATUS}" != "follow-up" && "${STATUS}" != "scheduled" ]]; then
    echo "Error: --status must be one of pass, fail, follow-up, scheduled." >&2
    exit 1
fi

LOG_PATH="${LOG_OVERRIDE:-ops/drill-log.md}"
DATE_VALUE="$(escape_markdown_table_cell "${DATE_OVERRIDE:-$(date -u +%F)}")"
SCENARIO_VALUE="$(escape_markdown_table_cell "${SCENARIO}")"
STATUS_VALUE="$(escape_markdown_table_cell "${STATUS}")"
START_VALUE="$(escape_markdown_table_cell "${START_TIME:-$(date -u +%H:%MZ)}")"
END_VALUE="$(escape_markdown_table_cell "${END_TIME:-"-"}")"
IC_VALUE="$(escape_markdown_table_cell "${IC_NAME:-"-"}")"
SCRIBE_VALUE="$(escape_markdown_table_cell "${SCRIBE_NAME:-"-"}")"
LINK_VALUE="$(escape_markdown_table_cell "${LINK:-"-"}")"
NOTES_VALUE="$(escape_markdown_table_cell "${NOTES:-"-"}")"

append_drill_log_entry \
    "${LOG_PATH}" \
    "${DATE_VALUE}" \
    "${SCENARIO_VALUE}" \
    "${STATUS_VALUE}" \
    "${IC_VALUE}" \
    "${SCRIBE_VALUE}" \
    "${START_VALUE}" \
    "${END_VALUE}" \
    "${NOTES_VALUE}" \
    "${LINK_VALUE}"

echo "Logged drill: ${DATE_VALUE} ${SCENARIO_VALUE} (${STATUS_VALUE})"
