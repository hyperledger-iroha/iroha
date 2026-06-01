#!/usr/bin/env python3
"""Check that Sumeragi formal modes stay wired across runner, CI, and docs."""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from functools import cache
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
SPEC_DIR = ROOT_DIR / "docs" / "formal" / "sumeragi"
APALACHE_RUNNER = ROOT_DIR / "scripts" / "formal" / "sumeragi_apalache.sh"
TLC_RUNNER = ROOT_DIR / "scripts" / "formal" / "sumeragi_tlc.sh"
FAST_CI = ROOT_DIR / "ci" / "check_sumeragi_formal.sh"
EXPECTED_FAILURE_CI = ROOT_DIR / "ci" / "check_sumeragi_formal_expected_failures.sh"
PR_WORKFLOW = ROOT_DIR / ".github" / "workflows" / "pr.yml"
NIGHTLY_WORKFLOW = ROOT_DIR / ".github" / "workflows" / "nightly_sumeragi_formal.yml"
README = SPEC_DIR / "README.md"

FORMAL_COVERAGE_COMMAND = "python3 scripts/formal/check_sumeragi_formal_coverage.py"
FORMAL_BASELINE_COMMAND = "bash ci/check_sumeragi_formal.sh"
FORMAL_EXPECTED_FAILURE_COMMAND = (
    "bash ci/check_sumeragi_formal_expected_failures.sh"
)
FRONTIER_NIGHTLY_COMMAND = (
    "bash scripts/formal/sumeragi_apalache.sh frontier-nightly"
)
APALACHE_COMMAND_PREFIX = "bash scripts/formal/sumeragi_apalache.sh"
TLC_COMMAND_PREFIX = "bash scripts/formal/sumeragi_tlc.sh"
INSTALL_APALACHE_COMMAND_PREFIX = "bash scripts/formal/install_apalache.sh"
APALACHE_EXPECTED_FAILURE_SNIPPETS = (
    'if [[ "$expect_failure" == "1" ]]; then',
    'if [[ "$status" == "0" ]]; then',
    'if [[ "$status" != "12" ]]; then',
    "expected Apalache invariant rejection",
    "expected Apalache rejection observed",
)
TLC_EXPECTED_FAILURE_SNIPPETS = (
    'if [[ "$expect_failure" -eq 1 ]]; then',
    'if [[ "$tlc_status" -eq 0 ]]; then',
    "Invariant .* is violated|Error: Invariant",
    "failed without the expected invariant violation",
    "produced the expected failure",
)
APALACHE_INVOCATION_SNIPPETS = (
    'check --length="$apalache_length" --config="$cfg_file" --run-dir="$run_dir" "$spec_file"',
    'check --length="$apalache_length" --config="$cfg_rel" --run-dir="$run_rel" "$spec_rel"',
)
TLC_INVOCATION_SNIPPETS = (
    'java ${TLC_JAVA_OPTS:-} -cp "$tlc_jar" tlc2.TLC',
    '-workers "$workers"',
    '-metadir "$run_dir"',
    '-config "$cfg_file"',
    '"$module"',
)

COMMAND_MODE_PATTERN = r"[A-Za-z0-9_.:/-]+"
COMMAND_MODE_RE = re.compile(rf"^{COMMAND_MODE_PATTERN}$")
APALACHE_COMMAND_RE = re.compile(
    rf"\b{re.escape(APALACHE_COMMAND_PREFIX)}\s+({COMMAND_MODE_PATTERN})"
)
TLC_COMMAND_RE = re.compile(
    rf"\b{re.escape(TLC_COMMAND_PREFIX)}\s+({COMMAND_MODE_PATTERN})"
)
CONFLICT_MARKER_RE = re.compile(r"^(?:<{7}|={7}|>{7})(?:\s|$)")
CASE_LABEL_RE = re.compile(r"^  ([A-Za-z0-9_-]+(?:-\*)?)\)\s*$", re.MULTILINE)
CASE_LABEL_LINE_RE = re.compile(
    r"^  (?:[A-Za-z0-9_-]+(?:-\*)?|\*)\)\s*$"
)
ASSIGN_RE = re.compile(
    r'^\s*(spec_file|cfg_file)="\$spec_dir/([^"]+)"\s*$', re.MULTILINE
)
PROOF_INPUT_ASSIGNMENT_RE = re.compile(r"^\s*(spec_file|cfg_file)\s*=")
MODULE_ASSIGN_RE = re.compile(r'^\s*module="([^"]+)"\s*$', re.MULTILINE)
TLC_CONSTRAINT_ASSIGN_RE = re.compile(
    r'^\s*tlc_constraint="([^"]*)"\s*$', re.MULTILINE
)
APALACHE_LENGTH_ASSIGN_RE = re.compile(
    r"^\s*apalache_length=([^\s#]+)\s*$", re.MULTILINE
)
RUNNER_APALACHE_VERSION_RE = re.compile(
    r'^\s*apalache_version="\$\{APALACHE_VERSION:-([0-9]+\.[0-9]+\.[0-9]+)\}"\s*$',
    re.MULTILINE,
)
INSTALLER_APALACHE_VERSION_RE = re.compile(
    r'^\s*version="\$\{1:-([0-9]+\.[0-9]+\.[0-9]+)\}"\s*$',
    re.MULTILINE,
)
INSTALL_APALACHE_COMMAND_VERSION_RE = re.compile(
    r"\bbash\s+scripts/formal/install_apalache\.sh\s+([0-9]+\.[0-9]+\.[0-9]+)\b"
)
APALACHE_TOOLCHAIN_PATH_VERSION_RE = re.compile(
    r"\btarget/apalache/toolchains/v([0-9]+\.[0-9]+\.[0-9]+)/"
)
APALACHE_DOCKER_IMAGE_VERSION_RE = re.compile(
    r"\bghcr\.io/apalache-mc/apalache:([0-9]+\.[0-9]+\.[0-9]+)\b"
)
TLA_MODULE_RE = re.compile(r"^-{4}\s+MODULE\s+([A-Za-z0-9_]+)\s+-{4}\s*$")
TLA_TERMINATOR_RE = re.compile(r"^={4}\s*$")
TLA_IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
TLA_OPERATOR_DEFINITION_RE = re.compile(
    r"^\s*(?:LOCAL\s+)?([A-Za-z_][A-Za-z0-9_]*)"
    r"\s*(?:\([^)]*\))?\s*=="
)
TLA_RECURSIVE_RE = re.compile(r"^\s*RECURSIVE\s+(.+)$")
TLA_EXTENDS_RE = re.compile(r"^\s*EXTENDS\s+(.+)$")
TLA_INSTANCE_RE = re.compile(
    r"^\s*(?:LOCAL\s+)?"
    r"(?:(?:[A-Za-z_][A-Za-z0-9_]*)\s*==\s*)?"
    r"INSTANCE\s+([A-Za-z_][A-Za-z0-9_]*)\b"
)
TLA_VARS_DEFINITION_RE = re.compile(r"^\s*vars\s*==\s*(.*)$")
TLA_IDENTIFIER_SCAN_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
TLA_STANDARD_MODULES = {
    "Bags",
    "FiniteSets",
    "Integers",
    "Naturals",
    "Randomization",
    "Reals",
    "RealTime",
    "Sequences",
    "TLC",
}
TLA_CONSTANT_DECLARATION_DIRECTIVES = {"CONSTANT", "CONSTANTS"}
TLA_CONSTANT_COLLECTION_STOP_DIRECTIVES = {
    "VARIABLE",
    "VARIABLES",
    "ASSUME",
    "THEOREM",
    "EXTENDS",
    "INSTANCE",
    "LOCAL",
    "RECURSIVE",
}
TLA_VARIABLE_DECLARATION_DIRECTIVES = {"VARIABLE", "VARIABLES"}
TLA_VARIABLE_COLLECTION_STOP_DIRECTIVES = {
    "CONSTANT",
    "CONSTANTS",
    "ASSUME",
    "THEOREM",
    "EXTENDS",
    "INSTANCE",
    "LOCAL",
    "RECURSIVE",
}
README_APALACHE_LENGTH_TABLE_HEADER = "| Mode | Length | Intended use |"
README_TABLE_SEPARATOR_RE = re.compile(
    r"^\|\s*:?-{3,}:?\s*\|\s*:?-{3,}:?\s*\|\s*:?-{3,}:?\s*\|\s*$"
)
README_APALACHE_LENGTH_TABLE_ROW_RE = re.compile(
    r"^\|\s*`([A-Za-z0-9_-]+)`\s*\|\s*([^|]*?)\s*\|\s*([^|]+?)\s*\|\s*$"
)
CFG_CONSTANT_BINDING_RE = re.compile(
    r"(^|\s)([A-Za-z_][A-Za-z0-9_]*)\s*(?:=|<-)"
)
CFG_CONSTANT_DIRECTIVES = {"CONSTANT", "CONSTANTS"}
CFG_CHECK_DIRECTIVES = {"INVARIANT", "INVARIANTS", "PROPERTY", "PROPERTIES"}
CFG_MISC_DIRECTIVES = {"CHECK_DEADLOCK"}
CFG_SINGLE_OPERATOR_DIRECTIVES = {
    "SPECIFICATION",
    "INIT",
    "NEXT",
    "CONSTRAINT",
    "INVARIANT",
    "PROPERTY",
}
CFG_MULTI_OPERATOR_DIRECTIVES = {"INVARIANTS", "PROPERTIES"}
CFG_ALLOWED_DIRECTIVES = (
    CFG_CONSTANT_DIRECTIVES
    | CFG_CHECK_DIRECTIVES
    | CFG_MISC_DIRECTIVES
    | CFG_SINGLE_OPERATOR_DIRECTIVES
    | CFG_MULTI_OPERATOR_DIRECTIVES
)
TLC_SPECIFIC_MUTATION_CFG_PREFIXES = ("commit-roots-bug-",)
FORMAL_FILE_SUFFIXES = {".cfg", ".tla"}


@dataclass(frozen=True)
class RunnerCase:
    """A parsed mode branch from a Sumeragi formal runner."""

    label: str
    body: str
    line: int

    @property
    def is_wildcard(self) -> bool:
        return self.label.endswith("*")

    @property
    def wildcard_prefix(self) -> str:
        return self.label[:-1]


@cache
def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def display_path(path: Path) -> Path:
    try:
        return path.relative_to(ROOT_DIR)
    except ValueError:
        return path


def command_modes(
    path: Path, command_re: re.Pattern[str] = APALACHE_COMMAND_RE
) -> list[str]:
    modes: list[str] = []
    for line in read_text(path).splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        modes.extend(match.group(1) for match in command_re.finditer(line))
    return modes


def command_shape_errors(path: Path, command_prefix: str, owner: str) -> list[str]:
    errors: list[str] = []
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue

        start = 0
        while True:
            index = line.find(command_prefix, start)
            if index == -1:
                break
            tail = line[index + len(command_prefix) :]
            match = re.match(r"\s+(\S+)\s*$", tail)
            if match is None:
                errors.append(
                    f"{owner} {display_path(path)}:{line_number} has "
                    f"malformed command: {stripped}"
                )
            else:
                mode = match.group(1)
                if not COMMAND_MODE_RE.match(mode):
                    errors.append(
                        f"{owner} {display_path(path)}:{line_number} "
                        f"has invalid mode token {mode!r}"
                    )
            start = index + len(command_prefix)
    return errors


def conflict_marker_errors(paths: tuple[Path, ...]) -> list[str]:
    errors: list[str] = []
    for path in paths:
        for line_number, line in enumerate(read_text(path).splitlines(), 1):
            if CONFLICT_MARKER_RE.match(line):
                errors.append(
                    f"{display_path(path)}:{line_number} contains merge "
                    f"conflict marker: {line.strip()}"
                )
    return errors


def documented_fast_table_modes(path: Path = README) -> list[str]:
    return [
        mode
        for mode, _length in documented_apalache_length_rows(path)
        if mode.endswith("-fast")
    ]


def apalache_length_table_body_lines(path: Path = README) -> tuple[
    list[tuple[int, str]], list[str]
]:
    lines = read_text(path).splitlines()
    header_indices = [
        index
        for index, line in enumerate(lines)
        if line.strip() == README_APALACHE_LENGTH_TABLE_HEADER
    ]
    if len(header_indices) != 1:
        return (
            [],
            [
                f"{display_path(path)}: README Apalache length table header "
                f"{README_APALACHE_LENGTH_TABLE_HEADER!r} appears "
                f"{len(header_indices)} times"
            ],
        )

    errors: list[str] = []
    header_index = header_indices[0]
    separator_index = header_index + 1
    if separator_index >= len(lines) or not README_TABLE_SEPARATOR_RE.match(
        lines[separator_index]
    ):
        errors.append(
            f"{display_path(path)}:{header_index + 2}: README Apalache "
            "length table is missing a Markdown separator row"
        )
        first_row_index = header_index + 1
    else:
        first_row_index = separator_index + 1

    body_lines: list[tuple[int, str]] = []
    for index, line in enumerate(lines[first_row_index:], start=first_row_index):
        if not line.startswith("|"):
            break
        body_lines.append((index + 1, line.rstrip()))
    return body_lines, errors


def documented_apalache_length_rows(path: Path = README) -> list[tuple[str, int]]:
    rows: list[tuple[str, int]] = []
    body_lines, _ = apalache_length_table_body_lines(path)
    for _, line in body_lines:
        match = README_APALACHE_LENGTH_TABLE_ROW_RE.match(line)
        if match is None:
            continue
        mode, length, _intended_use = match.groups()
        length = length.strip()
        if length.isdigit():
            rows.append((mode, int(length)))
    return rows


def apalache_length_table_shape_errors(path: Path = README) -> list[str]:
    body_lines, errors = apalache_length_table_body_lines(path)
    for line_number, line in body_lines:
        match = README_APALACHE_LENGTH_TABLE_ROW_RE.match(line)
        if match is None:
            errors.append(
                f"{display_path(path)}:{line_number}: malformed README "
                f"Apalache length table row: {line.strip()}"
            )
            continue
        mode, length, intended_use = match.groups()
        length = length.strip()
        if not length.isdigit():
            if not length:
                length = "<empty>"
            errors.append(
                f"{display_path(path)}:{line_number}: README Apalache "
                f"length for {mode} is not a non-negative integer: {length}"
            )
        if not intended_use.strip():
            errors.append(
                f"{display_path(path)}:{line_number}: README Apalache "
                f"length row for {mode} has an empty intended-use cell"
            )
    return errors


def runner_case_labels(path: Path) -> list[str]:
    return CASE_LABEL_RE.findall(read_text(path))


def runner_case_shape_errors(path: Path, runner_name: str) -> list[str]:
    errors: list[str] = []
    lines = read_text(path).splitlines()
    label_lines: list[tuple[int, int, str]] = []
    starts = [
        index for index, line in enumerate(lines) if line == 'case "$mode" in'
    ]
    if len(starts) != 1:
        errors.append(
            f"{runner_name} runner {display_path(path)} declares "
            f'{len(starts)} mode case blocks'
        )
        return errors

    start = starts[0]
    try:
        end = next(
            index for index, line in enumerate(lines[start + 1 :], start + 1)
            if line == "esac"
        )
    except StopIteration:
        errors.append(
            f"{runner_name} runner {display_path(path)} mode case block has no esac"
        )
        return errors

    for index, line in enumerate(lines[start + 1 : end], start + 2):
        stripped = line.strip()
        if not stripped:
            continue
        if line.startswith("  ") and not line.startswith("    "):
            if CASE_LABEL_LINE_RE.fullmatch(line) is None:
                errors.append(
                    f"{runner_name} runner {display_path(path)}:{index} "
                    f"has malformed case label: {stripped}"
                )
            else:
                label_lines.append((index - 1, index, stripped))
        elif not line.startswith("    "):
            errors.append(
                f"{runner_name} runner {display_path(path)}:{index} "
                f"has malformed case content: {stripped}"
            )
        if stripped.startswith((";;", ";&", ";;&")) and line != "    ;;":
            errors.append(
                f"{runner_name} runner {display_path(path)}:{index} "
                f"has malformed case terminator: {stripped}"
            )

    for position, (line_index, line_number, label) in enumerate(label_lines):
        next_line_index = (
            label_lines[position + 1][0]
            if position + 1 < len(label_lines)
            else end
        )
        if "    ;;" not in lines[line_index + 1 : next_line_index]:
            errors.append(
                f"{runner_name} runner {display_path(path)}:{line_number} "
                f"case label has no exact terminator: {label}"
            )
    return errors


def exact_fast_runner_modes(cases: dict[str, RunnerCase]) -> set[str]:
    return {label for label in cases if "*" not in label and label.endswith("-fast")}


def used_runner_case_labels(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
) -> set[str]:
    used: set[str] = set()
    for mode in modes:
        case = matching_case(mode, cases)
        if case is not None:
            used.add(case.label)
    return used


def unused_runner_case_labels(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
) -> list[str]:
    return sorted(set(cases) - used_runner_case_labels(modes, cases))


def runner_case_shadow_errors(
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    errors: list[str] = []
    ordered_cases = sorted(cases.values(), key=lambda case: case.line)
    for index, case in enumerate(ordered_cases):
        for prior in ordered_cases[:index]:
            if not prior.is_wildcard:
                continue
            if case.label.startswith(prior.wildcard_prefix):
                errors.append(
                    f"{runner_name} runner case {case.label!r} at line {case.line} "
                    f"is shadowed by earlier wildcard case {prior.label!r} "
                    f"at line {prior.line}"
                )
                break
    return errors


def bug_modes(modes: list[str]) -> set[str]:
    return {mode for mode in modes if "-bug-" in mode}


def parse_runner_cases(path: Path = APALACHE_RUNNER) -> dict[str, RunnerCase]:
    text = read_text(path)
    cases: dict[str, RunnerCase] = {}
    for match in CASE_LABEL_RE.finditer(text):
        label = match.group(1)
        end = text.find("\n    ;;", match.end())
        if end == -1:
            line = text.count("\n", 0, match.start()) + 1
            raise ValueError(f"runner case {label!r} at line {line} has no terminator")
        line = text.count("\n", 0, match.start()) + 1
        cases[label] = RunnerCase(label=label, body=text[match.end() : end], line=line)
    return cases


def matching_case(mode: str, cases: dict[str, RunnerCase]) -> RunnerCase | None:
    exact = cases.get(mode)
    if exact is not None:
        return exact

    wildcards = [
        case
        for case in cases.values()
        if case.is_wildcard and mode.startswith(case.wildcard_prefix)
    ]
    if not wildcards:
        return None
    return max(wildcards, key=lambda case: len(case.wildcard_prefix))


def resolve_spec_path(mode: str, case: RunnerCase, value: str) -> str:
    if case.is_wildcard:
        bug_name = mode[len(case.wildcard_prefix) :]
        value = value.replace("${bug_name}", bug_name)
        value = value.replace("${cfg_bug_name}", bug_name.replace("-", "_"))
    value = value.replace("$mode", mode)
    return value


def formal_file_path(
    mode: str,
    case: RunnerCase,
    variable: str,
    resolved: str,
) -> tuple[Path | None, list[str]]:
    candidate = Path(resolved)
    expected_suffix = {"spec_file": ".tla", "cfg_file": ".cfg"}.get(variable)
    if expected_suffix is not None and candidate.suffix != expected_suffix:
        return (
            None,
            [
                f"{mode}: {variable} in runner case {case.label!r} "
                f"must reference a {expected_suffix} file: {resolved}"
            ],
        )
    if candidate.name != resolved:
        return (
            None,
            [
                f"{mode}: {variable} in runner case {case.label!r} "
                f"must reference a flat Sumeragi formal file: {resolved}"
            ],
        )
    path = SPEC_DIR / candidate
    if candidate.is_absolute() or path.parent.resolve() != SPEC_DIR.resolve():
        return (
            None,
            [
                f"{mode}: {variable} in runner case {case.label!r} "
                f"escapes Sumeragi formal directory: {resolved}"
            ],
        )
    return path, []


def referenced_files(
    mode: str,
    case: RunnerCase,
    required_variables: tuple[str, ...] = ("spec_file", "cfg_file"),
) -> tuple[list[Path], list[str]]:
    assignments: dict[str, list[str]] = {}
    errors: list[str] = []
    for offset, line in enumerate(case.body.splitlines(), 1):
        if PROOF_INPUT_ASSIGNMENT_RE.match(line) and ASSIGN_RE.match(line) is None:
            line_number = case.line + offset - 1
            errors.append(
                f"{mode}: runner case {case.label!r} line {line_number} "
                f"has malformed proof-input assignment: {line.strip()}"
            )
    for variable, value in ASSIGN_RE.findall(case.body):
        assignments.setdefault(variable, []).append(value)
    files: list[Path] = []

    for variable in required_variables:
        values = assignments.get(variable, [])
        if len(values) != 1:
            errors.append(
                f"{mode}: runner case {case.label!r} at line {case.line} "
                f"assigns {variable} {len(values)} times"
            )
            continue
        value = values[0]
        resolved = resolve_spec_path(mode, case, value)
        if "$" in resolved or "{" in resolved:
            errors.append(
                f"{mode}: {variable} in runner case {case.label!r} "
                f"did not resolve statically: {resolved}"
            )
            continue
        path, path_errors = formal_file_path(mode, case, variable, resolved)
        errors.extend(path_errors)
        if path is not None:
            files.append(path)

    return files, errors


def malformed_scalar_assignment_errors(
    mode: str,
    case: RunnerCase,
    variable: str,
    assignment_re: re.Pattern[str],
    owner: str,
) -> list[str]:
    errors: list[str] = []
    candidate_re = re.compile(rf"^\s*{re.escape(variable)}\s*=")
    for offset, line in enumerate(case.body.splitlines(), 1):
        if candidate_re.match(line) and assignment_re.match(line) is None:
            line_number = case.line + offset - 1
            errors.append(
                f"{mode}: {owner} case {case.label!r} line {line_number} "
                f"has malformed {variable} assignment: {line.strip()}"
            )
    return errors


def tlc_module_files(mode: str, case: RunnerCase) -> tuple[list[Path], list[str]]:
    errors = malformed_scalar_assignment_errors(
        mode, case, "module", MODULE_ASSIGN_RE, "TLC runner"
    )
    modules = MODULE_ASSIGN_RE.findall(case.body)
    if len(modules) != 1:
        return (
            [],
            errors
            + [
                f"{mode}: TLC runner case {case.label!r} at line {case.line} "
                f"assigns module {len(modules)} times"
            ],
        )

    module = modules[0]
    if "$" in module or "{" in module or "/" in module:
        return (
            [],
            errors
            + [
                f"{mode}: module in TLC runner case {case.label!r} "
                f"did not resolve statically: {module}"
            ],
        )
    if not TLA_IDENTIFIER_RE.match(module):
        return (
            [],
            errors
            + [
                f"{mode}: module in TLC runner case {case.label!r} "
                f"must be a TLA identifier: {module}"
            ],
        )

    return [SPEC_DIR / f"{module}.tla"], errors


def tlc_runner_constraint_errors(
    mode: str,
    case: RunnerCase,
    module_path: Path,
) -> list[str]:
    errors = malformed_scalar_assignment_errors(
        mode, case, "tlc_constraint", TLC_CONSTRAINT_ASSIGN_RE, "TLC runner"
    )
    values = TLC_CONSTRAINT_ASSIGN_RE.findall(case.body)
    if len(values) > 1:
        errors.append(
            f"{mode}: TLC runner case {case.label!r} at line {case.line} "
            f"assigns tlc_constraint {len(values)} times"
        )
        return errors
    if len(values) == 0:
        return errors

    constraint = values[0]
    if not TLA_IDENTIFIER_RE.match(constraint):
        errors.append(
            f"{mode}: tlc_constraint in TLC runner case {case.label!r} "
            f"does not name a static TLA operator: {constraint}"
        )
        return errors
    if not module_path.exists():
        return errors
    definitions = tla_operator_definitions(module_path)
    if constraint in definitions:
        return errors
    errors.append(
        f"{mode}: TLC runner case {case.label!r} appends CONSTRAINT "
        f"{constraint}, but {display_path(module_path)} does not define it"
    )
    return errors


def apalache_length_value(
    mode: str,
    case: RunnerCase,
) -> tuple[int | None, list[str]]:
    errors = malformed_scalar_assignment_errors(
        mode, case, "apalache_length", APALACHE_LENGTH_ASSIGN_RE, "runner"
    )
    values = APALACHE_LENGTH_ASSIGN_RE.findall(case.body)
    if len(values) != 1:
        return (
            None,
            errors
            + [
                f"{mode}: runner case {case.label!r} at line {case.line} "
                f"assigns apalache_length {len(values)} times"
            ],
        )

    value = values[0]
    try:
        length = int(value)
    except ValueError:
        return (
            None,
            errors
            + [
                f"{mode}: apalache_length in runner case {case.label!r} "
                f"is not a non-negative integer: {value}"
            ],
        )
    if length < 0:
        return (
            None,
            errors
            + [
                f"{mode}: apalache_length in runner case {case.label!r} "
                f"is not a non-negative integer: {value}"
            ],
        )
    return length, errors


def apalache_length_errors(mode: str, case: RunnerCase) -> list[str]:
    _, errors = apalache_length_value(mode, case)
    return errors


def tla_module_header_errors(mode: str, paths: list[Path]) -> list[str]:
    errors: list[str] = []
    for path in paths:
        if path.suffix != ".tla" or not path.exists():
            continue

        headers: list[tuple[int, str]] = []
        terminators: list[int] = []
        first_nonempty_line: int | None = None
        for line_number, line in enumerate(read_text(path).splitlines(), 1):
            if first_nonempty_line is None and line.strip():
                first_nonempty_line = line_number
            match = TLA_MODULE_RE.match(line.strip())
            if match is not None:
                headers.append((line_number, match.group(1)))
            if TLA_TERMINATOR_RE.match(line.strip()):
                terminators.append(line_number)

        relative = display_path(path)
        if not headers:
            errors.append(f"{mode}: {relative} has no TLA MODULE declaration")
            continue

        if len(headers) != 1:
            errors.append(
                f"{mode}: {relative} declares TLA MODULE {len(headers)} times"
            )

        line_number, declared = headers[0]
        if first_nonempty_line is not None and line_number != first_nonempty_line:
            errors.append(
                f"{mode}: {relative}:{line_number} declares MODULE after "
                f"content at line {first_nonempty_line}"
            )
        if declared != path.stem:
            errors.append(
                f"{mode}: {relative} declares MODULE {declared}, "
                f"expected {path.stem}"
            )
        if len(terminators) != 1:
            errors.append(
                f"{mode}: {relative} declares TLA terminator {len(terminators)} times"
            )
        elif any(
            line.strip()
            for line in read_text(path).splitlines()[terminators[0] :]
        ):
            errors.append(
                f"{mode}: {relative}:{terminators[0]} has content after "
                "TLA terminator"
            )
    return errors


def cfg_shape_errors(mode: str, paths: list[Path]) -> list[str]:
    errors: list[str] = []
    for path in paths:
        if path.suffix != ".cfg" or not path.exists():
            continue

        relative = display_path(path)
        text = read_text(path)
        if not text.strip():
            errors.append(f"{mode}: {relative} is empty")
            continue
        errors.extend(f"{mode}: {error}" for error in cfg_directive_errors(path))

        directives = {
            stripped.split()[0]
            for line in text.splitlines()
            if (stripped := line.strip()) and not stripped.startswith("\\*")
        }
        has_specification = "SPECIFICATION" in directives
        has_init = "INIT" in directives
        has_next = "NEXT" in directives
        if has_specification and (has_init or has_next):
            errors.append(
                f"{mode}: {relative} mixes SPECIFICATION with INIT/NEXT behavior"
            )
        elif not has_specification and not (has_init and has_next):
            errors.append(
                f"{mode}: {relative} must define SPECIFICATION or both INIT and NEXT"
            )

        if not (CFG_CHECK_DIRECTIVES & directives):
            errors.append(f"{mode}: {relative} has no invariant or property checks")
    return errors


@cache
def cfg_directive_errors(path: Path) -> list[str]:
    errors: list[str] = []
    collecting: str | None = None
    seen_check_deadlock_line: int | None = None

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0].strip()
        if not stripped:
            collecting = None
            continue

        parts = stripped.split()
        directive = parts[0]
        if collecting is not None and line[:1].isspace():
            continue

        if directive not in CFG_ALLOWED_DIRECTIVES:
            errors.append(
                f"{display_path(path)}:{line_number} unknown CFG directive "
                f"{directive}"
            )
            collecting = None
            continue

        if directive == "CHECK_DEADLOCK":
            if len(parts) != 2 or parts[1] not in {"TRUE", "FALSE"}:
                errors.append(
                    f"{display_path(path)}:{line_number} CHECK_DEADLOCK "
                    "must be TRUE or FALSE"
                )
            if seen_check_deadlock_line is not None:
                errors.append(
                    f"{display_path(path)}:{line_number} repeats "
                    "CHECK_DEADLOCK directive first declared at line "
                    f"{seen_check_deadlock_line}"
                )
            else:
                seen_check_deadlock_line = line_number
            collecting = None
            continue

        if (
            directive in CFG_CONSTANT_DIRECTIVES | CFG_MULTI_OPERATOR_DIRECTIVES
            and len(parts) == 1
        ):
            collecting = directive
        else:
            collecting = None

    return errors


@cache
def cfg_operator_references(path: Path) -> tuple[list[tuple[int, str, str]], list[str]]:
    references: list[tuple[int, str, str]] = []
    errors: list[str] = []
    collecting: str | None = None

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0].strip()
        if not stripped:
            collecting = None
            continue

        parts = stripped.split()
        directive = parts[0]
        if directive in CFG_SINGLE_OPERATOR_DIRECTIVES:
            collecting = None
            if len(parts) != 2:
                errors.append(
                    f"{display_path(path)}:{line_number} directive {directive} "
                    f"must reference exactly one operator"
                )
            elif not TLA_IDENTIFIER_RE.match(parts[1]):
                errors.append(
                    f"{display_path(path)}:{line_number} directive {directive} "
                    f"must reference a static TLA operator: {parts[1]}"
                )
            else:
                references.append((line_number, directive, parts[1]))
            continue

        if directive in CFG_MULTI_OPERATOR_DIRECTIVES:
            if len(parts) > 1:
                for operator in parts[1:]:
                    if not TLA_IDENTIFIER_RE.match(operator):
                        errors.append(
                            f"{display_path(path)}:{line_number} directive "
                            f"{directive} must reference static TLA operators: "
                            f"{operator}"
                        )
                    else:
                        references.append((line_number, directive, operator))
                collecting = None
            else:
                collecting = directive
            continue

        if collecting is not None and line[:1].isspace():
            if len(parts) != 1 or not TLA_IDENTIFIER_RE.match(parts[0]):
                errors.append(
                    f"{display_path(path)}:{line_number} {collecting} "
                    "block line must reference exactly one static TLA operator"
                )
            else:
                references.append((line_number, collecting, parts[0]))
            continue

        collecting = None

    return references, errors


@cache
def tla_operator_definition_entries(path: Path) -> list[tuple[int, str]]:
    entries: list[tuple[int, str]] = []
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0]
        if stripped.startswith((" ", "\t")):
            continue
        match = TLA_OPERATOR_DEFINITION_RE.match(stripped)
        if match is not None:
            entries.append((line_number, match.group(1)))
    return entries


@cache
def tla_recursive_declaration_entries(path: Path) -> list[tuple[int, str]]:
    entries: list[tuple[int, str]] = []
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0]
        if stripped.startswith((" ", "\t")):
            continue

        match = TLA_RECURSIVE_RE.match(stripped)
        if match is None:
            continue
        for part in match.group(1).split(","):
            name = part.strip().split("(", 1)[0].strip()
            if TLA_IDENTIFIER_RE.match(name):
                entries.append((line_number, name))
    return entries


@cache
def tla_operator_definitions(path: Path) -> set[str]:
    definitions = {name for _, name in tla_operator_definition_entries(path)}
    for _, name in tla_recursive_declaration_entries(path):
        definitions.add(name)
    return definitions


def tla_duplicate_operator_definition_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    errors: list[str] = []
    seen_definitions: dict[str, int] = {}
    for line_number, operator in tla_operator_definition_entries(path):
        previous_line = seen_definitions.get(operator)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA operator definition {operator} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_definitions[operator] = line_number

    seen_recursive: dict[str, int] = {}
    for line_number, operator in tla_recursive_declaration_entries(path):
        previous_line = seen_recursive.get(operator)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA RECURSIVE declaration {operator} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_recursive[operator] = line_number
    return errors


@cache
def tla_module_dependency_references(
    path: Path,
) -> tuple[list[tuple[int, str, str]], list[str]]:
    references: list[tuple[int, str, str]] = []
    errors: list[str] = []

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0].strip()
        if not stripped:
            continue

        match = TLA_EXTENDS_RE.match(stripped)
        if match is not None:
            modules = TLA_IDENTIFIER_SCAN_RE.findall(match.group(1))
            if not modules:
                errors.append(
                    f"{display_path(path)}:{line_number} EXTENDS "
                    "must name at least one module"
                )
            references.extend(
                (line_number, "EXTENDS", module) for module in modules
            )
            continue

        match = TLA_INSTANCE_RE.match(stripped)
        if match is not None:
            references.append((line_number, "INSTANCE", match.group(1)))

    return references, errors


def tla_module_dependency_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    references, parse_errors = tla_module_dependency_references(path)
    errors = [f"{mode}: {error}" for error in parse_errors]
    for line_number, directive, module in references:
        if module in TLA_STANDARD_MODULES:
            continue
        dependency_path = path.with_name(f"{module}.tla")
        if dependency_path.exists():
            continue
        errors.append(
            f"{mode}: {display_path(path)}:{line_number} references "
            f"{directive} module {module}, but neither TLA standard module "
            f"nor {display_path(dependency_path)} exists"
        )
    return errors


@cache
def tla_constant_declaration_entries(path: Path) -> list[tuple[int, str]]:
    entries: list[tuple[int, str]] = []
    collecting = False

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0].strip()
        if not stripped:
            continue

        parts = stripped.split()
        directive = parts[0]
        if directive in TLA_CONSTANT_DECLARATION_DIRECTIVES:
            collecting = True
            rest = stripped[len(directive) :].strip()
            entries.extend(
                (line_number, name)
                for name in TLA_IDENTIFIER_SCAN_RE.findall(rest)
            )
            continue

        if not collecting:
            continue
        if directive in TLA_CONSTANT_COLLECTION_STOP_DIRECTIVES or "==" in stripped:
            collecting = False
            continue
        entries.extend(
            (line_number, name)
            for name in TLA_IDENTIFIER_SCAN_RE.findall(stripped)
        )

    return entries


@cache
def tla_constant_declarations(path: Path) -> set[str]:
    declarations = {name for _, name in tla_constant_declaration_entries(path)}
    return declarations


def tla_duplicate_constant_declaration_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    errors: list[str] = []
    seen: dict[str, int] = {}
    for line_number, constant in tla_constant_declaration_entries(path):
        previous_line = seen.get(constant)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA constant declaration {constant} first declared at line "
                f"{previous_line}"
            )
        else:
            seen[constant] = line_number
    return errors


@cache
def tla_variable_declaration_entries(path: Path) -> list[tuple[int, str]]:
    entries: list[tuple[int, str]] = []
    collecting = False

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0].strip()
        if not stripped:
            continue

        parts = stripped.split()
        directive = parts[0]
        if directive in TLA_VARIABLE_DECLARATION_DIRECTIVES:
            collecting = True
            rest = stripped[len(directive) :].strip()
            entries.extend(
                (line_number, name)
                for name in TLA_IDENTIFIER_SCAN_RE.findall(rest)
            )
            continue

        if not collecting:
            continue
        if directive in TLA_VARIABLE_COLLECTION_STOP_DIRECTIVES or "==" in stripped:
            collecting = False
            continue
        entries.extend(
            (line_number, name)
            for name in TLA_IDENTIFIER_SCAN_RE.findall(stripped)
        )

    return entries


@cache
def tla_vars_tuple_entries(
    path: Path,
) -> tuple[list[tuple[int, str]], list[str]]:
    entries: list[tuple[int, str]] = []
    errors: list[str] = []
    definitions: list[tuple[int, str]] = []
    lines = read_text(path).splitlines()

    for index, line in enumerate(lines):
        stripped = line.split("\\*", 1)[0]
        if stripped.startswith((" ", "\t")):
            continue
        match = TLA_VARS_DEFINITION_RE.match(stripped)
        if match is None:
            continue

        body = [match.group(1)]
        if ">>" not in body[0]:
            for continuation in lines[index + 1 :]:
                continuation = continuation.split("\\*", 1)[0]
                body.append(continuation)
                if ">>" in continuation:
                    break
                if continuation.strip() and not continuation.startswith((" ", "\t")):
                    break
        definitions.append((index + 1, "\n".join(body)))

    if len(definitions) != 1:
        errors.append(
            f"{display_path(path)} defines vars tuple {len(definitions)} times"
        )
        return entries, errors

    line_number, body = definitions[0]
    start = body.find("<<")
    end = body.rfind(">>")
    if start == -1 or end == -1 or end <= start:
        errors.append(
            f"{display_path(path)}:{line_number} vars must be a static tuple"
        )
        return entries, errors

    content = body[start + 2 : end]
    names = [name.strip() for name in content.split(",")]
    if not names or any(not name for name in names):
        errors.append(
            f"{display_path(path)}:{line_number} vars must list static variables"
        )
        return entries, errors

    for name in names:
        if not TLA_IDENTIFIER_RE.match(name):
            errors.append(
                f"{display_path(path)}:{line_number} vars must list "
                f"static variables: {name}"
            )
        else:
            entries.append((line_number, name))
    return entries, errors


def tla_variable_surface_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    errors: list[str] = []
    declarations = tla_variable_declaration_entries(path)
    vars_entries, parse_errors = tla_vars_tuple_entries(path)
    errors.extend(f"{mode}: {error}" for error in parse_errors)

    seen_declarations: dict[str, int] = {}
    for line_number, variable in declarations:
        previous_line = seen_declarations.get(variable)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA variable declaration {variable} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_declarations[variable] = line_number

    seen_vars: dict[str, int] = {}
    for line_number, variable in vars_entries:
        previous_line = seen_vars.get(variable)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"vars tuple variable {variable} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_vars[variable] = line_number

    declared = set(seen_declarations)
    tupled = set(seen_vars)
    for variable in sorted(declared - tupled):
        errors.append(
            f"{mode}: {display_path(path)} declares variable {variable} "
            "but vars does not include it"
        )
    for variable in sorted(tupled - declared):
        errors.append(
            f"{mode}: {display_path(path)} vars includes undeclared variable "
            f"{variable}"
        )
    return errors


@cache
def cfg_constant_bindings(path: Path) -> tuple[list[tuple[int, str]], list[str]]:
    bindings: list[tuple[int, str]] = []
    errors: list[str] = []
    collecting = False

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.split("\\*", 1)[0].strip()
        if not stripped:
            collecting = False
            continue

        parts = stripped.split()
        directive = parts[0]
        if directive in CFG_CONSTANT_DIRECTIVES:
            rest = stripped[len(directive) :].strip()
            if not rest:
                collecting = True
                continue
            matches = list(CFG_CONSTANT_BINDING_RE.finditer(rest))
            if not matches:
                errors.append(
                    f"{display_path(path)}:{line_number} directive {directive} "
                    "must bind at least one constant"
                )
            bindings.extend((line_number, match.group(2)) for match in matches)
            collecting = False
            continue

        if not collecting:
            continue
        if not line[:1].isspace():
            collecting = False
            continue
        match = CFG_CONSTANT_BINDING_RE.search(stripped)
        if match is None:
            errors.append(
                f"{display_path(path)}:{line_number} CONSTANTS block line "
                "must bind a constant"
            )
            continue
        bindings.append((line_number, match.group(2)))

    return bindings, errors


def cfg_constant_binding_errors(mode: str, module_path: Path, cfg_path: Path) -> list[str]:
    if not module_path.exists() or not cfg_path.exists():
        return []

    bindings, parse_errors = cfg_constant_bindings(cfg_path)
    declarations = tla_constant_declarations(module_path)
    errors = [f"{mode}: {error}" for error in parse_errors]
    bound_constants: set[str] = set()
    for line_number, constant in bindings:
        bound_constants.add(constant)
        if constant not in declarations:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} binds constant "
                f"{constant}, but {display_path(module_path)} does not declare it"
            )
    for constant in sorted(declarations - bound_constants):
        errors.append(
            f"{mode}: {display_path(cfg_path)} does not bind constant {constant} "
            f"declared by {display_path(module_path)}"
        )
    return errors


def cfg_module_ownership_errors(
    mode: str,
    module_path: Path,
    cfg_path: Path,
) -> list[str]:
    if (
        cfg_path.stem == module_path.stem
        or cfg_path.stem.startswith(f"{module_path.stem}_")
    ):
        return []
    return [
        f"{mode}: CFG {display_path(cfg_path)} does not belong to TLA module "
        f"{display_path(module_path)}; expected filename stem {module_path.stem} "
        f"or {module_path.stem}_*"
    ]


def cfg_duplicate_constant_binding_errors(mode: str, cfg_path: Path) -> list[str]:
    if not cfg_path.exists():
        return []

    bindings, parse_errors = cfg_constant_bindings(cfg_path)
    if parse_errors:
        return []

    errors: list[str] = []
    seen: dict[str, int] = {}
    for line_number, constant in bindings:
        previous_line = seen.get(constant)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} repeats "
                f"constant binding {constant} first declared at line "
                f"{previous_line}"
            )
        else:
            seen[constant] = line_number
    return errors


def cfg_operator_reference_errors(mode: str, module_path: Path, cfg_path: Path) -> list[str]:
    if not module_path.exists() or not cfg_path.exists():
        return []

    references, parse_errors = cfg_operator_references(cfg_path)
    definitions = tla_operator_definitions(module_path)
    errors = [f"{mode}: {error}" for error in parse_errors]
    for line_number, directive, operator in references:
        if operator not in definitions:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} references "
                f"{directive} operator {operator}, but {display_path(module_path)} "
                f"does not define it"
            )
    return errors


def normalized_cfg_check_directive(directive: str) -> str:
    if directive in {"INVARIANT", "INVARIANTS"}:
        return "INVARIANT"
    if directive in {"PROPERTY", "PROPERTIES"}:
        return "PROPERTY"
    return directive


def cfg_duplicate_operator_reference_errors(mode: str, cfg_path: Path) -> list[str]:
    if not cfg_path.exists():
        return []

    references, parse_errors = cfg_operator_references(cfg_path)
    if parse_errors:
        return []

    errors: list[str] = []
    seen_behavior: dict[str, int] = {}
    seen_checks: dict[tuple[str, str], int] = {}
    for line_number, directive, operator in references:
        if directive in {"SPECIFICATION", "INIT", "NEXT"}:
            previous_line = seen_behavior.get(directive)
            if previous_line is not None:
                errors.append(
                    f"{mode}: {display_path(cfg_path)}:{line_number} repeats "
                    f"{directive} behavior directive first declared at line "
                    f"{previous_line}"
                )
            else:
                seen_behavior[directive] = line_number
            continue

        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        normalized = normalized_cfg_check_directive(directive)
        key = (normalized, operator)
        previous_line = seen_checks.get(key)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} repeats "
                f"{normalized} check {operator} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_checks[key] = line_number
    return errors


def cfg_semantic_check_errors(
    mode: str,
    cfg_file: Path,
    runner_name: str,
) -> list[str]:
    if not cfg_file.exists():
        return []

    references, parse_errors = cfg_operator_references(cfg_file)
    if parse_errors:
        return []
    checks = [
        operator
        for _, directive, operator in references
        if directive in CFG_CHECK_DIRECTIVES
    ]
    semantic_checks = [operator for operator in checks if operator != "TypeInvariant"]
    if semantic_checks:
        return []
    return [
        f"{mode}: {runner_name} cfg {display_path(cfg_file)} "
        "has no non-TypeInvariant invariant/property check"
    ]


def unreferenced_formal_file_errors(referenced_paths: set[Path]) -> list[str]:
    referenced_formal_paths = {
        path for path in referenced_paths if path.suffix in FORMAL_FILE_SUFFIXES
    }
    formal_inventory = {
        path
        for suffix in FORMAL_FILE_SUFFIXES
        for path in SPEC_DIR.glob(f"*{suffix}")
    }
    return [
        f"{display_path(path)} is not referenced by any checked or documented "
        "Sumeragi formal mode"
        for path in sorted(formal_inventory - referenced_formal_paths)
    ]


def sorted_unique(values: list[str] | set[str]) -> list[str]:
    return sorted(set(values))


def duplicate_values(values: list[str]) -> list[str]:
    seen: set[str] = set()
    duplicates: set[str] = set()
    for value in values:
        if value in seen:
            duplicates.add(value)
        else:
            seen.add(value)
    return sorted(duplicates)


def format_items(values: list[str], limit: int = 80) -> str:
    if not values:
        return ""
    shown = values[:limit]
    suffix = ""
    if len(values) > limit:
        suffix = f"\n  ... and {len(values) - limit} more"
    return "\n".join(f"  - {value}" for value in shown) + suffix


def print_error_sections(errors: list[str]) -> None:
    print("Sumeragi formal coverage check failed:", file=sys.stderr)
    for section in errors:
        print(f"\n{section}", file=sys.stderr)


def required_command_errors(
    path: Path,
    commands: tuple[str, ...],
    owner: str,
) -> list[str]:
    text = read_text(path)
    return [
        f"{owner} {display_path(path)} is missing command: {command}"
        for command in commands
        if command not in text
    ]


def required_text_errors(
    path: Path,
    snippets: tuple[str, ...],
    owner: str,
) -> list[str]:
    text = read_text(path)
    return [
        f"{owner} {display_path(path)} is missing required text: {snippet}"
        for snippet in snippets
        if snippet not in text
    ]


def command_order_errors(
    path: Path,
    first: str,
    second: str,
    owner: str,
) -> list[str]:
    text = read_text(path)
    first_index = text.find(first)
    second_index = text.find(second)
    if first_index == -1 or second_index == -1 or first_index < second_index:
        return []
    return [
        f"{owner} {display_path(path)} must run {first!r} before {second!r}"
    ]


def regex_values(path: Path, pattern: re.Pattern[str]) -> list[str]:
    return pattern.findall(read_text(path))


def single_regex_value(
    path: Path,
    pattern: re.Pattern[str],
    label: str,
) -> tuple[str | None, list[str]]:
    values = regex_values(path, pattern)
    if len(values) == 1:
        return values[0], []
    return (
        None,
        [
            f"{label} {display_path(path)} declares Apalache version "
            f"{len(values)} times"
        ],
    )


def version_values_mismatch_errors(
    path: Path,
    pattern: re.Pattern[str],
    expected: str,
    label: str,
) -> list[str]:
    values = regex_values(path, pattern)
    if not values:
        return [
            f"{label} {display_path(path)} does not declare Apalache {expected}"
        ]
    return [
        f"{label} {display_path(path)} uses Apalache {value}, expected {expected}"
        for value in sorted_unique(values)
        if value != expected
    ]


def apalache_version_pin_errors() -> list[str]:
    runner_version, errors = single_regex_value(
        APALACHE_RUNNER, RUNNER_APALACHE_VERSION_RE, "Apalache runner"
    )
    if runner_version is None:
        return errors

    pinned_sources: tuple[tuple[Path, re.Pattern[str], str], ...] = (
        (TLC_RUNNER, RUNNER_APALACHE_VERSION_RE, "TLC runner"),
        (ROOT_DIR / "scripts" / "formal" / "install_apalache.sh", INSTALLER_APALACHE_VERSION_RE, "Apalache installer"),
        (PR_WORKFLOW, INSTALL_APALACHE_COMMAND_VERSION_RE, "PR workflow install command"),
        (PR_WORKFLOW, APALACHE_TOOLCHAIN_PATH_VERSION_RE, "PR workflow toolchain path"),
        (NIGHTLY_WORKFLOW, INSTALL_APALACHE_COMMAND_VERSION_RE, "nightly workflow install command"),
        (README, INSTALL_APALACHE_COMMAND_VERSION_RE, "formal README install command"),
        (README, APALACHE_TOOLCHAIN_PATH_VERSION_RE, "formal README toolchain path"),
        (README, APALACHE_DOCKER_IMAGE_VERSION_RE, "formal README Docker image"),
        (ROOT_DIR / "ci" / "README.md", INSTALL_APALACHE_COMMAND_VERSION_RE, "CI README install command"),
    )
    for path, pattern, label in pinned_sources:
        errors.extend(
            version_values_mismatch_errors(path, pattern, runner_version, label)
        )
    return errors


def expected_failure_semantics_errors(
    apalache_runner: Path = APALACHE_RUNNER,
    tlc_runner: Path = TLC_RUNNER,
) -> list[str]:
    errors: list[str] = []
    errors.extend(
        required_text_errors(
            apalache_runner,
            APALACHE_EXPECTED_FAILURE_SNIPPETS,
            "Apalache expected-failure path",
        )
    )
    errors.extend(
        required_text_errors(
            tlc_runner,
            TLC_EXPECTED_FAILURE_SNIPPETS,
            "TLC expected-failure path",
        )
    )
    return errors


def runner_invocation_errors(
    apalache_runner: Path = APALACHE_RUNNER,
    tlc_runner: Path = TLC_RUNNER,
) -> list[str]:
    errors: list[str] = []
    errors.extend(
        required_text_errors(
            apalache_runner,
            APALACHE_INVOCATION_SNIPPETS,
            "Apalache runner invocation",
        )
    )
    errors.extend(
        required_text_errors(
            tlc_runner,
            TLC_INVOCATION_SNIPPETS,
            "TLC runner invocation",
        )
    )
    return errors


def workflow_entrypoint_errors() -> list[str]:
    errors: list[str] = []
    errors.extend(
        required_command_errors(
            PR_WORKFLOW,
            (FORMAL_BASELINE_COMMAND,),
            "PR workflow",
        )
    )
    errors.extend(
        command_order_errors(
            PR_WORKFLOW,
            INSTALL_APALACHE_COMMAND_PREFIX,
            FORMAL_BASELINE_COMMAND,
            "PR workflow",
        )
    )
    errors.extend(
        required_command_errors(
            NIGHTLY_WORKFLOW,
            (FORMAL_BASELINE_COMMAND, FRONTIER_NIGHTLY_COMMAND),
            "nightly workflow",
        )
    )
    errors.extend(
        command_order_errors(
            NIGHTLY_WORKFLOW,
            INSTALL_APALACHE_COMMAND_PREFIX,
            FORMAL_BASELINE_COMMAND,
            "nightly workflow",
        )
    )
    errors.extend(
        command_order_errors(
            NIGHTLY_WORKFLOW,
            FORMAL_BASELINE_COMMAND,
            FRONTIER_NIGHTLY_COMMAND,
            "nightly workflow",
        )
    )
    errors.extend(
        required_command_errors(
            FAST_CI,
            (FORMAL_COVERAGE_COMMAND, FORMAL_EXPECTED_FAILURE_COMMAND),
            "formal baseline script",
        )
    )
    errors.extend(
        command_order_errors(
            FAST_CI,
            FORMAL_COVERAGE_COMMAND,
            APALACHE_COMMAND_PREFIX,
            "formal baseline script",
        )
    )
    return errors


def modes_without_expected_failure_marker(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    missing: list[str] = []
    for mode in sorted_unique(modes):
        case = matching_case(mode, cases)
        if case is not None and "expect_failure=1" not in case.body:
            missing.append(
                f"{mode}: {runner_name} runner case {case.label!r} "
                f"at line {case.line}"
            )
    return missing


def modes_with_unexpected_failure_marker(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    unexpected: list[str] = []
    for mode in sorted_unique(modes):
        case = matching_case(mode, cases)
        if case is not None and "expect_failure=1" in case.body:
            unexpected.append(
                f"{mode}: {runner_name} runner case {case.label!r} "
                f"at line {case.line}"
            )
    return unexpected


def apalache_length_table_errors(
    documented_lengths: dict[str, int],
    cases: dict[str, RunnerCase],
) -> list[str]:
    errors: list[str] = []
    for mode, documented_length in sorted(documented_lengths.items()):
        case = matching_case(mode, cases)
        if case is None:
            continue
        actual_length, length_errors = apalache_length_value(mode, case)
        if length_errors or actual_length is None:
            continue
        if actual_length != documented_length:
            errors.append(
                f"{mode}: README length {documented_length} differs from "
                f"Apalache runner length {actual_length}"
            )
    return errors


def cfg_file_for_mode(mode: str, case: RunnerCase) -> tuple[Path | None, list[str]]:
    files, errors = referenced_files(mode, case, required_variables=("cfg_file",))
    cfg_files = [path for path in files if path.suffix == ".cfg"]
    if len(cfg_files) != 1:
        errors.append(
            f"{mode}: runner case {case.label!r} at line {case.line} "
            f"resolves {len(cfg_files)} cfg files"
        )
        return None, errors
    return cfg_files[0], errors


def spec_file_for_mode(mode: str, case: RunnerCase) -> tuple[Path | None, list[str]]:
    files, errors = referenced_files(mode, case, required_variables=("spec_file",))
    spec_files = [path for path in files if path.suffix == ".tla"]
    if len(spec_files) != 1:
        errors.append(
            f"{mode}: runner case {case.label!r} at line {case.line} "
            f"resolves {len(spec_files)} TLA spec files"
        )
        return None, errors
    return spec_files[0], errors


def module_identity_errors(
    modes: list[str] | set[str],
    apalache_cases: dict[str, RunnerCase],
    tlc_cases: dict[str, RunnerCase],
) -> list[str]:
    errors: list[str] = []
    for mode in sorted_unique(modes):
        apalache_case = matching_case(mode, apalache_cases)
        tlc_case = matching_case(mode, tlc_cases)
        if apalache_case is None or tlc_case is None:
            continue

        apalache_spec, apalache_errors = spec_file_for_mode(mode, apalache_case)
        tlc_modules, tlc_errors = tlc_module_files(mode, tlc_case)
        if apalache_errors or tlc_errors:
            continue
        if apalache_spec is None or len(tlc_modules) != 1:
            continue
        tlc_module = tlc_modules[0]
        if apalache_spec != tlc_module:
            errors.append(
                f"{mode}: Apalache spec {display_path(apalache_spec)} differs "
                f"from TLC module {display_path(tlc_module)}"
            )
    return errors


def allowed_mutation_cfg_pair(mode: str, apalache_cfg: Path, tlc_cfg: Path) -> bool:
    if apalache_cfg == tlc_cfg:
        return True
    if not any(
        mode.startswith(prefix) for prefix in TLC_SPECIFIC_MUTATION_CFG_PREFIXES
    ):
        return False
    if "_bug_" not in apalache_cfg.name:
        return False
    expected_tlc_name = apalache_cfg.name.replace("_bug_", "_tlc_bug_", 1)
    return tlc_cfg == apalache_cfg.with_name(expected_tlc_name)


def mutation_cfg_equivalence_errors(
    modes: list[str] | set[str],
    apalache_cases: dict[str, RunnerCase],
    tlc_cases: dict[str, RunnerCase],
) -> list[str]:
    errors: list[str] = []
    for mode in sorted_unique(modes):
        apalache_case = matching_case(mode, apalache_cases)
        tlc_case = matching_case(mode, tlc_cases)
        if apalache_case is None or tlc_case is None:
            continue

        apalache_cfg, apalache_errors = cfg_file_for_mode(mode, apalache_case)
        tlc_cfg, tlc_errors = cfg_file_for_mode(mode, tlc_case)
        if apalache_errors or tlc_errors:
            continue
        if apalache_cfg is None or tlc_cfg is None:
            continue
        if not allowed_mutation_cfg_pair(mode, apalache_cfg, tlc_cfg):
            errors.append(
                f"{mode}: Apalache cfg {display_path(apalache_cfg)} differs "
                f"from TLC cfg {display_path(tlc_cfg)}"
            )
    return errors


def main() -> int:
    errors: list[str] = []

    conflict_marker_mismatches = conflict_marker_errors(
        (
            APALACHE_RUNNER,
            TLC_RUNNER,
            FAST_CI,
            EXPECTED_FAILURE_CI,
            PR_WORKFLOW,
            NIGHTLY_WORKFLOW,
            README,
        )
    )
    if conflict_marker_mismatches:
        print_error_sections(
            [
                "Sumeragi formal wiring files contain merge conflict markers:\n"
                + format_items(conflict_marker_mismatches)
            ]
        )
        return 1

    runner_case_shape_mismatches = runner_case_shape_errors(
        APALACHE_RUNNER, "Apalache"
    )
    runner_case_shape_mismatches.extend(
        runner_case_shape_errors(TLC_RUNNER, "TLC")
    )
    if runner_case_shape_mismatches:
        print_error_sections(
            [
                "Sumeragi formal runner case blocks are malformed:\n"
                + format_items(runner_case_shape_mismatches)
            ]
        )
        return 1

    apalache_cases = parse_runner_cases(APALACHE_RUNNER)
    tlc_cases = parse_runner_cases(TLC_RUNNER)
    duplicate_apalache_case_labels = duplicate_values(
        runner_case_labels(APALACHE_RUNNER)
    )
    duplicate_tlc_case_labels = duplicate_values(runner_case_labels(TLC_RUNNER))
    shadowed_apalache_case_labels = runner_case_shadow_errors(
        apalache_cases, "Apalache"
    )
    shadowed_tlc_case_labels = runner_case_shadow_errors(tlc_cases, "TLC")
    apalache_version_mismatches = apalache_version_pin_errors()
    expected_failure_semantics_mismatches = expected_failure_semantics_errors()
    runner_invocation_mismatches = runner_invocation_errors()
    workflow_entrypoint_mismatches = workflow_entrypoint_errors()
    command_shape_mismatches: list[str] = []
    for path in (FAST_CI, EXPECTED_FAILURE_CI, NIGHTLY_WORKFLOW, README):
        command_shape_mismatches.extend(
            command_shape_errors(path, APALACHE_COMMAND_PREFIX, "Apalache command")
        )
    command_shape_mismatches.extend(
        command_shape_errors(README, TLC_COMMAND_PREFIX, "TLC command")
    )
    fast_ci_modes = command_modes(FAST_CI, APALACHE_COMMAND_RE)
    expected_failure_modes = command_modes(EXPECTED_FAILURE_CI, APALACHE_COMMAND_RE)
    nightly_ci_modes = command_modes(NIGHTLY_WORKFLOW, APALACHE_COMMAND_RE)
    ci_modes = fast_ci_modes + expected_failure_modes + nightly_ci_modes
    readme_modes = command_modes(README, APALACHE_COMMAND_RE)
    readme_tlc_modes = command_modes(README, TLC_COMMAND_RE)
    readme_fast_table_modes = documented_fast_table_modes(README)
    readme_apalache_length_rows = documented_apalache_length_rows(README)
    readme_apalache_length_shape_mismatches = apalache_length_table_shape_errors(
        README
    )
    readme_apalache_length_modes = [
        mode for mode, _ in readme_apalache_length_rows
    ]
    readme_apalache_lengths = dict(readme_apalache_length_rows)
    duplicate_fast_ci_modes = duplicate_values(fast_ci_modes)
    duplicate_expected_failure_ci_modes = duplicate_values(expected_failure_modes)
    duplicate_nightly_ci_modes = duplicate_values(nightly_ci_modes)
    duplicate_readme_apalache_commands = duplicate_values(readme_modes)
    duplicate_readme_apalache_length_modes = duplicate_values(
        readme_apalache_length_modes
    )
    overlapping_ci_modes = sorted_unique(
        set(fast_ci_modes) & set(expected_failure_modes)
    )
    readme_bug_modes = bug_modes(readme_modes)
    expected_failure_ci_set = set(expected_failure_modes)
    documented_bug_modes_missing_expected_failure_ci = sorted_unique(
        readme_bug_modes - expected_failure_ci_set
    )
    fast_ci_bug_modes = sorted_unique(bug_modes(fast_ci_modes))
    expected_failure_ci_non_bug_modes = sorted_unique(
        expected_failure_ci_set - bug_modes(expected_failure_modes)
    )

    all_documented_modes = set(readme_modes)
    all_checked_modes = set(ci_modes)
    all_modes_to_resolve = sorted_unique(all_checked_modes | all_documented_modes)

    unsupported_ci_modes: list[str] = []
    unsupported_readme_modes: list[str] = []
    missing_files: list[str] = []
    reference_errors: list[str] = []
    tlc_reference_errors: list[str] = []
    referenced_formal_files: set[Path] = set()

    for mode in all_modes_to_resolve:
        case = matching_case(mode, apalache_cases)
        if case is None:
            if mode in all_checked_modes:
                unsupported_ci_modes.append(mode)
            if mode in all_documented_modes:
                unsupported_readme_modes.append(mode)
            continue

        files, mode_reference_errors = referenced_files(mode, case)
        referenced_formal_files.update(
            path for path in files if path.suffix in FORMAL_FILE_SUFFIXES
        )
        reference_errors.extend(mode_reference_errors)
        reference_errors.extend(apalache_length_errors(mode, case))
        reference_errors.extend(tla_module_header_errors(mode, files))
        for spec_file in [path for path in files if path.suffix == ".tla"]:
            reference_errors.extend(tla_module_dependency_errors(mode, spec_file))
            reference_errors.extend(
                tla_duplicate_constant_declaration_errors(mode, spec_file)
            )
            reference_errors.extend(
                tla_duplicate_operator_definition_errors(mode, spec_file)
            )
            reference_errors.extend(tla_variable_surface_errors(mode, spec_file))
        reference_errors.extend(cfg_shape_errors(mode, files))
        spec_files = [path for path in files if path.suffix == ".tla"]
        cfg_files = [path for path in files if path.suffix == ".cfg"]
        for cfg_file in cfg_files:
            reference_errors.extend(
                cfg_duplicate_constant_binding_errors(mode, cfg_file)
            )
            reference_errors.extend(
                cfg_duplicate_operator_reference_errors(mode, cfg_file)
            )
            reference_errors.extend(
                cfg_semantic_check_errors(mode, cfg_file, "Apalache")
            )
        if len(spec_files) == 1:
            for cfg_file in cfg_files:
                reference_errors.extend(
                    cfg_module_ownership_errors(mode, spec_files[0], cfg_file)
                )
                reference_errors.extend(
                    cfg_operator_reference_errors(mode, spec_files[0], cfg_file)
                )
                reference_errors.extend(
                    cfg_constant_binding_errors(mode, spec_files[0], cfg_file)
                )
        for path in files:
            if not path.exists():
                missing_files.append(f"{mode}: {path.relative_to(ROOT_DIR)}")

    expected_failure_without_marker = modes_without_expected_failure_marker(
        expected_failure_modes, apalache_cases, "Apalache"
    )
    baseline_with_expected_failure_marker = modes_with_unexpected_failure_marker(
        set(fast_ci_modes) | set(nightly_ci_modes), apalache_cases, "Apalache"
    )

    missing_readme_commands = sorted_unique(all_checked_modes - all_documented_modes)
    exact_runner_modes = {label for label in apalache_cases if "*" not in label}
    pr_baseline_modes = {
        mode
        for mode in exact_runner_modes
        if mode in {"fast", "deep", "fork-npos"} or mode.endswith("-fast")
    }
    fast_ci_set = set(fast_ci_modes)
    missing_fast_ci_modes = sorted_unique(pr_baseline_modes - fast_ci_set)
    readme_apalache_length_set = set(readme_apalache_length_modes)
    missing_readme_apalache_length_modes = sorted_unique(
        pr_baseline_modes - readme_apalache_length_set
    )
    unsupported_readme_apalache_length_modes = sorted_unique(
        mode
        for mode in readme_apalache_length_set
        if matching_case(mode, apalache_cases) is None
    )
    apalache_length_mismatches = apalache_length_table_errors(
        readme_apalache_lengths, apalache_cases
    )
    missing_exact_runner_ci_modes = sorted_unique(
        exact_runner_modes - set(ci_modes)
    )
    unused_apalache_runner_cases = unused_runner_case_labels(
        all_modes_to_resolve, apalache_cases
    )
    readme_fast_table_set = set(readme_fast_table_modes)
    readme_tlc_set = set(readme_tlc_modes)
    tlc_modes_to_resolve = readme_fast_table_set | readme_tlc_set | readme_bug_modes
    missing_tlc_runner_modes = sorted_unique(
        mode
        for mode in readme_fast_table_set
        if matching_case(mode, tlc_cases) is None
    )
    missing_readme_tlc_commands = sorted_unique(
        readme_fast_table_set - readme_tlc_set
    )
    exact_tlc_fast_modes = exact_fast_runner_modes(tlc_cases)
    undocumented_tlc_runner_modes = sorted_unique(
        exact_tlc_fast_modes - readme_fast_table_set
    )
    documented_bug_modes_missing_tlc_runner = sorted_unique(
        mode for mode in readme_bug_modes if matching_case(mode, tlc_cases) is None
    )
    tlc_expected_failure_without_marker = modes_without_expected_failure_marker(
        readme_bug_modes, tlc_cases, "TLC"
    )
    tlc_non_bug_modes = tlc_modes_to_resolve - readme_bug_modes
    tlc_baseline_with_expected_failure_marker = modes_with_unexpected_failure_marker(
        tlc_non_bug_modes, tlc_cases, "TLC"
    )
    mutation_cfg_mismatches = mutation_cfg_equivalence_errors(
        readme_bug_modes, apalache_cases, tlc_cases
    )
    module_identity_mismatches = module_identity_errors(
        tlc_modes_to_resolve, apalache_cases, tlc_cases
    )
    unused_tlc_runner_cases = unused_runner_case_labels(
        tlc_modes_to_resolve, tlc_cases
    )
    duplicate_readme_fast_table_modes = duplicate_values(readme_fast_table_modes)
    duplicate_readme_tlc_commands = duplicate_values(readme_tlc_modes)
    unsupported_readme_tlc_commands = sorted_unique(
        mode for mode in readme_tlc_set if matching_case(mode, tlc_cases) is None
    )
    missing_tlc_files: list[str] = []
    for mode in sorted_unique(tlc_modes_to_resolve):
        case = matching_case(mode, tlc_cases)
        if case is None:
            continue
        files, mode_reference_errors = referenced_files(
            mode, case, required_variables=("cfg_file",)
        )
        module_files, module_reference_errors = tlc_module_files(mode, case)
        files.extend(module_files)
        referenced_formal_files.update(
            path for path in files if path.suffix in FORMAL_FILE_SUFFIXES
        )
        tlc_reference_errors.extend(mode_reference_errors)
        tlc_reference_errors.extend(module_reference_errors)
        tlc_reference_errors.extend(tla_module_header_errors(mode, module_files))
        for module_file in module_files:
            tlc_reference_errors.extend(tla_module_dependency_errors(mode, module_file))
            tlc_reference_errors.extend(
                tla_duplicate_constant_declaration_errors(mode, module_file)
            )
            tlc_reference_errors.extend(
                tla_duplicate_operator_definition_errors(mode, module_file)
            )
            tlc_reference_errors.extend(
                tla_variable_surface_errors(mode, module_file)
            )
        tlc_reference_errors.extend(cfg_shape_errors(mode, files))
        for cfg_file in files:
            if cfg_file.suffix == ".cfg":
                tlc_reference_errors.extend(
                    cfg_duplicate_constant_binding_errors(mode, cfg_file)
                )
                tlc_reference_errors.extend(
                    cfg_duplicate_operator_reference_errors(mode, cfg_file)
                )
                tlc_reference_errors.extend(
                    cfg_semantic_check_errors(mode, cfg_file, "TLC")
                )
        if len(module_files) == 1:
            tlc_reference_errors.extend(
                tlc_runner_constraint_errors(mode, case, module_files[0])
            )
            for cfg_file in files:
                if cfg_file.suffix == ".cfg":
                    tlc_reference_errors.extend(
                        cfg_module_ownership_errors(mode, module_files[0], cfg_file)
                    )
                    tlc_reference_errors.extend(
                        cfg_operator_reference_errors(mode, module_files[0], cfg_file)
                    )
                    tlc_reference_errors.extend(
                        cfg_constant_binding_errors(mode, module_files[0], cfg_file)
                    )
        for path in files:
            if not path.exists():
                missing_tlc_files.append(f"{mode}: {path.relative_to(ROOT_DIR)}")

    unreferenced_formal_files = unreferenced_formal_file_errors(
        referenced_formal_files
    )

    if unsupported_ci_modes:
        errors.append(
            "CI invokes Sumeragi formal modes unsupported by the runner:\n"
            + format_items(sorted_unique(unsupported_ci_modes))
        )
    if unsupported_readme_modes:
        errors.append(
            "README documents Sumeragi formal modes unsupported by the runner:\n"
            + format_items(sorted_unique(unsupported_readme_modes))
        )
    if missing_readme_commands:
        errors.append(
            "README is missing commands for CI-invoked Sumeragi formal modes:\n"
            + format_items(missing_readme_commands)
        )
    if missing_fast_ci_modes:
        errors.append(
            "PR CI is missing exact fast Sumeragi formal runner modes:\n"
            + format_items(missing_fast_ci_modes)
        )
    if readme_apalache_length_shape_mismatches:
        errors.append(
            "README Apalache length table is malformed:\n"
            + format_items(readme_apalache_length_shape_mismatches)
        )
    if missing_readme_apalache_length_modes:
        errors.append(
            "README Apalache length table is missing PR baseline modes:\n"
            + format_items(missing_readme_apalache_length_modes)
        )
    if unsupported_readme_apalache_length_modes:
        errors.append(
            "README Apalache length table documents modes unsupported by the runner:\n"
            + format_items(unsupported_readme_apalache_length_modes)
        )
    if apalache_length_mismatches:
        errors.append(
            "README Apalache length table disagrees with the runner:\n"
            + format_items(apalache_length_mismatches)
        )
    if missing_exact_runner_ci_modes:
        errors.append(
            "Exact Apalache runner modes are missing from formal CI:\n"
            + format_items(missing_exact_runner_ci_modes)
        )
    if unused_apalache_runner_cases:
        errors.append(
            "Apalache runner has case branches unused by CI or README modes:\n"
            + format_items(unused_apalache_runner_cases)
        )
    if duplicate_fast_ci_modes:
        errors.append(
            "PR CI has duplicate Sumeragi formal modes:\n"
            + format_items(duplicate_fast_ci_modes)
        )
    if duplicate_expected_failure_ci_modes:
        errors.append(
            "Expected-failure CI has duplicate Sumeragi formal modes:\n"
            + format_items(duplicate_expected_failure_ci_modes)
        )
    if duplicate_nightly_ci_modes:
        errors.append(
            "Scheduled/manual CI has duplicate Sumeragi formal modes:\n"
            + format_items(duplicate_nightly_ci_modes)
        )
    if overlapping_ci_modes:
        errors.append(
            "Sumeragi formal modes appear in both PR and expected-failure CI:\n"
            + format_items(overlapping_ci_modes)
        )
    if documented_bug_modes_missing_expected_failure_ci:
        errors.append(
            "README documents Sumeragi mutation modes missing from expected-failure CI:\n"
            + format_items(documented_bug_modes_missing_expected_failure_ci)
        )
    if fast_ci_bug_modes:
        errors.append(
            "PR CI includes Sumeragi mutation modes that belong in expected-failure CI:\n"
            + format_items(fast_ci_bug_modes)
        )
    if expected_failure_ci_non_bug_modes:
        errors.append(
            "Expected-failure CI includes non-mutation Sumeragi formal modes:\n"
            + format_items(expected_failure_ci_non_bug_modes)
        )
    if duplicate_readme_apalache_commands:
        errors.append(
            "README has duplicate Apalache commands for modes:\n"
            + format_items(duplicate_readme_apalache_commands)
        )
    if duplicate_readme_apalache_length_modes:
        errors.append(
            "README Apalache length table has duplicate modes:\n"
            + format_items(duplicate_readme_apalache_length_modes)
        )
    if expected_failure_without_marker:
        errors.append(
            "Expected-failure CI modes are not marked expect_failure=1 in the runner:\n"
            + format_items(expected_failure_without_marker)
        )
    if baseline_with_expected_failure_marker:
        errors.append(
            "PR or scheduled/manual Sumeragi formal modes are marked "
            "expect_failure=1 in the runner:\n"
            + format_items(baseline_with_expected_failure_marker)
        )
    if duplicate_apalache_case_labels:
        errors.append(
            "Apalache runner has duplicate case labels:\n"
            + format_items(duplicate_apalache_case_labels)
        )
    if duplicate_tlc_case_labels:
        errors.append(
            "TLC runner has duplicate case labels:\n"
            + format_items(duplicate_tlc_case_labels)
        )
    if runner_case_shape_mismatches:
        errors.append(
            "Sumeragi formal runner case blocks are malformed:\n"
            + format_items(runner_case_shape_mismatches)
        )
    if shadowed_apalache_case_labels:
        errors.append(
            "Apalache runner has case labels shadowed by earlier wildcards:\n"
            + format_items(shadowed_apalache_case_labels)
        )
    if shadowed_tlc_case_labels:
        errors.append(
            "TLC runner has case labels shadowed by earlier wildcards:\n"
            + format_items(shadowed_tlc_case_labels)
        )
    if workflow_entrypoint_mismatches:
        errors.append(
            "Sumeragi formal workflow entrypoints are not wired to the guard:\n"
            + format_items(workflow_entrypoint_mismatches)
        )
    if command_shape_mismatches:
        errors.append(
            "Sumeragi formal command lines are malformed:\n"
            + format_items(command_shape_mismatches)
        )
    if apalache_version_mismatches:
        errors.append(
            "Sumeragi formal Apalache version pins disagree:\n"
            + format_items(apalache_version_mismatches)
        )
    if expected_failure_semantics_mismatches:
        errors.append(
            "Sumeragi formal expected-failure runner semantics are weak:\n"
            + format_items(expected_failure_semantics_mismatches)
        )
    if runner_invocation_mismatches:
        errors.append(
            "Sumeragi formal runner invocations do not bind selected proof inputs:\n"
            + format_items(runner_invocation_mismatches)
        )
    if missing_tlc_runner_modes:
        errors.append(
            "README fast-mode table documents modes unsupported by the TLC runner:\n"
            + format_items(missing_tlc_runner_modes)
        )
    if missing_readme_tlc_commands:
        errors.append(
            "README is missing TLC commands for documented fast modes:\n"
            + format_items(missing_readme_tlc_commands)
        )
    if undocumented_tlc_runner_modes:
        errors.append(
            "TLC runner has exact fast modes missing from the README fast-mode table:\n"
            + format_items(undocumented_tlc_runner_modes)
        )
    if documented_bug_modes_missing_tlc_runner:
        errors.append(
            "README documents Sumeragi mutation modes unsupported by the TLC runner:\n"
            + format_items(documented_bug_modes_missing_tlc_runner)
        )
    if tlc_expected_failure_without_marker:
        errors.append(
            "README mutation modes are not marked expect_failure=1 in the TLC runner:\n"
            + format_items(tlc_expected_failure_without_marker)
        )
    if tlc_baseline_with_expected_failure_marker:
        errors.append(
            "README non-mutation TLC modes are marked expect_failure=1 "
            "in the runner:\n"
            + format_items(tlc_baseline_with_expected_failure_marker)
        )
    if mutation_cfg_mismatches:
        errors.append(
            "README mutation modes resolve to different Apalache/TLC cfg files:\n"
            + format_items(mutation_cfg_mismatches)
        )
    if module_identity_mismatches:
        errors.append(
            "TLC modes resolve to different TLA modules than Apalache:\n"
            + format_items(module_identity_mismatches)
        )
    if unused_tlc_runner_cases:
        errors.append(
            "TLC runner has case branches unused by README modes:\n"
            + format_items(unused_tlc_runner_cases)
        )
    if duplicate_readme_fast_table_modes:
        errors.append(
            "README fast-mode table has duplicate modes:\n"
            + format_items(duplicate_readme_fast_table_modes)
        )
    if duplicate_readme_tlc_commands:
        errors.append(
            "README has duplicate TLC commands for modes:\n"
            + format_items(duplicate_readme_tlc_commands)
        )
    if unsupported_readme_tlc_commands:
        errors.append(
            "README documents TLC commands unsupported by the TLC runner:\n"
            + format_items(unsupported_readme_tlc_commands)
        )
    if reference_errors:
        errors.append(
            "Could not resolve runner spec/config references:\n"
            + format_items(reference_errors)
        )
    if tlc_reference_errors:
        errors.append(
            "Could not resolve TLC runner config references:\n"
            + format_items(tlc_reference_errors)
        )
    if missing_files:
        errors.append(
            "Runner modes reference missing Sumeragi formal files:\n"
            + format_items(sorted_unique(missing_files))
        )
    if missing_tlc_files:
        errors.append(
            "TLC runner modes reference missing Sumeragi formal config files:\n"
            + format_items(sorted_unique(missing_tlc_files))
        )
    if unreferenced_formal_files:
        errors.append(
            "Sumeragi formal TLA+/CFG files are not reached by checked or "
            "documented modes:\n"
            + format_items(unreferenced_formal_files)
        )

    if errors:
        print_error_sections(errors)
        return 1

    print(
        "[formal] Sumeragi coverage wiring is consistent "
        f"({len(set(fast_ci_modes))} PR modes, "
        f"{len(set(expected_failure_modes))} expected-failure modes, "
        f"{len(set(nightly_ci_modes))} scheduled/manual modes, "
        f"{len(set(readme_modes))} documented modes, "
        f"{len(readme_fast_table_set)} TLC fast modes, "
        f"{len(readme_bug_modes)} TLC mutation modes)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
