#!/usr/bin/env python3
"""Fail closed when a GitHub workflow executes an unpinned remote action."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
FULL_CONTAINER_DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
USES_LINE_RE = re.compile(r"^\s*(?:-\s*)?uses:\s*(?P<value>.*?)\s*$")
USES_VALUE_RE = re.compile(
    r"""^(?:"(?P<double>[^"]+)"|'(?P<single>[^']+)'|(?P<plain>[^#\s]+))"""
    r"""(?:\s+#.*)?$"""
)


@dataclass(frozen=True)
class Violation:
    path: Path
    line: int
    message: str

    def render(self) -> str:
        return f"{self.path}:{self.line}: {self.message}"


def _workflow_files(workflows_dir: Path) -> Iterable[Path]:
    for suffix in ("*.yml", "*.yaml"):
        yield from sorted(workflows_dir.rglob(suffix))


def _composite_action_files(workflows_dir: Path) -> Iterable[Path]:
    actions_dir = workflows_dir.parent / "actions"
    if not actions_dir.is_dir():
        return
    for filename in ("action.yml", "action.yaml"):
        yield from sorted(actions_dir.rglob(filename))


def _uses_target(raw_value: str) -> str | None:
    match = USES_VALUE_RE.fullmatch(raw_value)
    if match is None:
        return None
    return next(
        value
        for value in (
            match.group("double"),
            match.group("single"),
            match.group("plain"),
        )
        if value is not None
    )


def audit_workflows(workflows_dir: Path) -> list[Violation]:
    violations: list[Violation] = []
    if not workflows_dir.is_dir():
        return [
            Violation(
                workflows_dir,
                0,
                "workflow directory is missing or is not a directory",
            )
        ]

    files = list(_workflow_files(workflows_dir))
    if not files:
        return [Violation(workflows_dir, 0, "workflow directory contains no YAML files")]
    files.extend(_composite_action_files(workflows_dir))

    for path in files:
        if path.is_symlink():
            violations.append(Violation(path, 0, "workflow files must not be symlinks"))
            continue
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as exc:
            violations.append(Violation(path, 0, f"cannot read workflow as UTF-8: {exc}"))
            continue

        for line_number, line in enumerate(source.splitlines(), start=1):
            match = USES_LINE_RE.fullmatch(line)
            if match is None:
                continue
            target = _uses_target(match.group("value"))
            if target is None:
                violations.append(
                    Violation(
                        path,
                        line_number,
                        "uses value must be one static YAML scalar with no interpolation",
                    )
                )
                continue

            if target.startswith("./"):
                continue

            if target.startswith("docker://"):
                image = target.removeprefix("docker://")
                if "@" not in image:
                    violations.append(
                        Violation(
                            path,
                            line_number,
                            "container action must use an immutable sha256 image digest",
                        )
                    )
                    continue
                _, digest = image.rsplit("@", 1)
                if FULL_CONTAINER_DIGEST_RE.fullmatch(digest) is None:
                    violations.append(
                        Violation(
                            path,
                            line_number,
                            "container action digest must be sha256 plus 64 lowercase hex digits",
                        )
                    )
                continue

            if "@" not in target:
                violations.append(
                    Violation(path, line_number, "remote action is missing a commit reference")
                )
                continue
            action, commit = target.rsplit("@", 1)
            if "/" not in action or FULL_COMMIT_RE.fullmatch(commit) is None:
                violations.append(
                    Violation(
                        path,
                        line_number,
                        "remote action must be pinned to one full 40-character lowercase commit SHA",
                    )
                )

    return violations


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workflows-dir",
        type=Path,
        default=Path(".github/workflows"),
        help="workflow directory to audit (default: .github/workflows)",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    violations = audit_workflows(args.workflows_dir)
    if violations:
        print("GitHub Actions pin policy failed:", file=sys.stderr)
        for violation in violations:
            print(f" - {violation.render()}", file=sys.stderr)
        return 1
    print(
        "GitHub Actions pin policy passed: every workflow and composite remote action "
        "uses a full commit SHA."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
