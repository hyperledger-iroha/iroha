#!/usr/bin/env python3
"""Check that Sumeragi formal modes stay wired across runner, CI, and docs."""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
SPEC_DIR = ROOT_DIR / "docs" / "formal" / "sumeragi"
RUNNER = ROOT_DIR / "scripts" / "formal" / "sumeragi_apalache.sh"
FAST_CI = ROOT_DIR / "ci" / "check_sumeragi_formal.sh"
EXPECTED_FAILURE_CI = ROOT_DIR / "ci" / "check_sumeragi_formal_expected_failures.sh"
README = SPEC_DIR / "README.md"

COMMAND_RE = re.compile(
    r"\bbash\s+scripts/formal/sumeragi_apalache\.sh\s+([A-Za-z0-9_.:/-]+)"
)
CASE_LABEL_RE = re.compile(r"^  ([A-Za-z0-9_-]+(?:-\*)?)\)\s*$", re.MULTILINE)
ASSIGN_RE = re.compile(
    r'^\s*(spec_file|cfg_file)="\$spec_dir/([^"]+)"\s*$', re.MULTILINE
)


@dataclass(frozen=True)
class RunnerCase:
    """A parsed mode branch from the Sumeragi Apalache runner."""

    label: str
    body: str
    line: int

    @property
    def is_wildcard(self) -> bool:
        return self.label.endswith("*")

    @property
    def wildcard_prefix(self) -> str:
        return self.label[:-1]


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def command_modes(path: Path) -> list[str]:
    modes: list[str] = []
    for line in read_text(path).splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        modes.extend(match.group(1) for match in COMMAND_RE.finditer(line))
    return modes


def parse_runner_cases() -> dict[str, RunnerCase]:
    text = read_text(RUNNER)
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


def referenced_files(mode: str, case: RunnerCase) -> tuple[list[Path], list[str]]:
    assignments = dict(ASSIGN_RE.findall(case.body))
    errors: list[str] = []
    files: list[Path] = []

    for variable in ("spec_file", "cfg_file"):
        value = assignments.get(variable)
        if value is None:
            errors.append(
                f"{mode}: runner case {case.label!r} at line {case.line} "
                f"does not assign {variable}"
            )
            continue
        resolved = resolve_spec_path(mode, case, value)
        if "$" in resolved or "{" in resolved:
            errors.append(
                f"{mode}: {variable} in runner case {case.label!r} "
                f"did not resolve statically: {resolved}"
            )
            continue
        files.append(SPEC_DIR / resolved)

    return files, errors


def sorted_unique(values: list[str] | set[str]) -> list[str]:
    return sorted(set(values))


def format_items(values: list[str], limit: int = 80) -> str:
    if not values:
        return ""
    shown = values[:limit]
    suffix = ""
    if len(values) > limit:
        suffix = f"\n  ... and {len(values) - limit} more"
    return "\n".join(f"  - {value}" for value in shown) + suffix


def main() -> int:
    errors: list[str] = []

    cases = parse_runner_cases()
    fast_ci_modes = command_modes(FAST_CI)
    expected_failure_modes = command_modes(EXPECTED_FAILURE_CI)
    ci_modes = fast_ci_modes + expected_failure_modes
    readme_modes = command_modes(README)

    all_documented_modes = set(readme_modes)
    all_checked_modes = set(ci_modes)
    all_modes_to_resolve = sorted_unique(all_checked_modes | all_documented_modes)

    unsupported_ci_modes: list[str] = []
    unsupported_readme_modes: list[str] = []
    missing_files: list[str] = []
    reference_errors: list[str] = []
    expected_failure_without_marker: list[str] = []

    for mode in all_modes_to_resolve:
        case = matching_case(mode, cases)
        if case is None:
            if mode in all_checked_modes:
                unsupported_ci_modes.append(mode)
            if mode in all_documented_modes:
                unsupported_readme_modes.append(mode)
            continue

        files, mode_reference_errors = referenced_files(mode, case)
        reference_errors.extend(mode_reference_errors)
        for path in files:
            if not path.exists():
                missing_files.append(f"{mode}: {path.relative_to(ROOT_DIR)}")

    for mode in sorted_unique(expected_failure_modes):
        case = matching_case(mode, cases)
        if case is not None and "expect_failure=1" not in case.body:
            expected_failure_without_marker.append(
                f"{mode}: runner case {case.label!r} at line {case.line}"
            )

    missing_readme_commands = sorted_unique(all_checked_modes - all_documented_modes)
    exact_runner_modes = {label for label in cases if "*" not in label}
    pr_baseline_modes = {
        mode
        for mode in exact_runner_modes
        if mode in {"fast", "deep", "fork-npos"} or mode.endswith("-fast")
    }
    fast_ci_set = set(fast_ci_modes)
    missing_fast_ci_modes = sorted_unique(pr_baseline_modes - fast_ci_set)

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
    if expected_failure_without_marker:
        errors.append(
            "Expected-failure CI modes are not marked expect_failure=1 in the runner:\n"
            + format_items(expected_failure_without_marker)
        )
    if reference_errors:
        errors.append(
            "Could not resolve runner spec/config references:\n"
            + format_items(reference_errors)
        )
    if missing_files:
        errors.append(
            "Runner modes reference missing Sumeragi formal files:\n"
            + format_items(sorted_unique(missing_files))
        )

    if errors:
        print("Sumeragi formal coverage check failed:", file=sys.stderr)
        for section in errors:
            print(f"\n{section}", file=sys.stderr)
        return 1

    print(
        "[formal] Sumeragi coverage wiring is consistent "
        f"({len(set(fast_ci_modes))} PR modes, "
        f"{len(set(expected_failure_modes))} expected-failure modes, "
        f"{len(set(readme_modes))} documented modes)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
