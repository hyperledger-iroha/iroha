#!/usr/bin/env python3
"""Fail fast while a Taira release stage is an unconditional source barrier.

The production release authorities deliberately refuse work until independent
brokers, replay journals, and semantic verifiers are provisioned.  This check
does not pretend to validate those host installations.  It only prevents an
expensive workflow from reaching Cargo while the repository implementation is
still an unconditional refusal stub.
"""

from __future__ import annotations

import argparse
import ast
import json
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Prerequisite:
    """One source-level authority prerequisite on the release critical path."""

    path: str
    function: str
    stage: str


PREREQUISITES = (
    Prerequisite(
        "scripts/taira_release_authority.py",
        "require_independent_native_evidence_authority_provisioned",
        "linux native-evidence authority",
    ),
    Prerequisite(
        "scripts/taira_privacy_protocol_receipt.py",
        "require_controller_origin_authority_provisioned",
        "privacy protocol receipt authority",
    ),
    Prerequisite(
        "scripts/taira_privacy_governance_authority.py",
        "_require_provisioned_privacy_governance_authority_v1",
        "Exact12 governance authority",
    ),
    Prerequisite(
        "scripts/build_privacy_v1_boi_handoff.py",
        "require_boi_qualification_isolation",
        "BOI qualification authority",
    ),
    Prerequisite(
        "scripts/deploy_taira_v21_reset.py",
        "require_deploy_issuance_contracts",
        "deployment issuance authority",
    ),
    Prerequisite(
        "scripts/build_taira_public_v2_deploy_handoff.py",
        "require_deploy_native_evidence_authority_provisioned",
        "post-deploy native-evidence authority",
    ),
    Prerequisite(
        "scripts/run_taira_public_v2_24h_soak.py",
        "require_public_soak_runner_authority_provisioned",
        "public 24-hour soak producer authority",
    ),
    Prerequisite(
        "scripts/taira_privacy_rollout_contract.py",
        "require_authenticated_rollout_observation_authority_provisioned",
        "rollout observation authority",
    ),
    Prerequisite(
        "scripts/taira_public_soak_authority_contract.py",
        "require_public_soak_authority_provisioned",
        "public 24-hour soak observation authority",
    ),
)


class PrerequisiteError(RuntimeError):
    """The prerequisite audit could not inspect an exact source contract."""


def _function(path: Path, name: str) -> ast.FunctionDef | ast.AsyncFunctionDef:
    """Return the unique named function from one Python source file."""

    try:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except (OSError, UnicodeError, SyntaxError) as error:
        raise PrerequisiteError(f"cannot parse {path}: {error}") from error
    matches = [
        node
        for node in ast.walk(tree)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        and node.name == name
    ]
    if len(matches) != 1:
        raise PrerequisiteError(
            f"expected one {name} definition in {path}, found {len(matches)}"
        )
    return matches[0]


def is_unconditional_refusal(
    function: ast.FunctionDef | ast.AsyncFunctionDef,
) -> bool:
    """Return whether ``function`` only discards inputs and then raises/fails."""

    body = list(function.body)
    if (
        body
        and isinstance(body[0], ast.Expr)
        and isinstance(body[0].value, ast.Constant)
        and isinstance(body[0].value.value, str)
    ):
        body.pop(0)
    body = [statement for statement in body if not isinstance(statement, ast.Delete)]
    if len(body) != 1:
        return False
    statement = body[0]
    if isinstance(statement, ast.Raise):
        return True
    if not isinstance(statement, ast.Expr) or not isinstance(statement.value, ast.Call):
        return False
    callee = statement.value.func
    return isinstance(callee, ast.Name) and callee.id in {"fail", "_fail"}


def unresolved_prerequisites(
    repository: Path,
    prerequisites: Iterable[Prerequisite] = PREREQUISITES,
) -> list[dict[str, object]]:
    """Return deterministic reports for unconditional release blockers."""

    reports: list[dict[str, object]] = []
    for prerequisite in prerequisites:
        path = repository / prerequisite.path
        function = _function(path, prerequisite.function)
        if is_unconditional_refusal(function):
            reports.append(
                {
                    "function": prerequisite.function,
                    "line": function.lineno,
                    "path": prerequisite.path,
                    "stage": prerequisite.stage,
                }
            )
    return reports


def build_parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (defaults to the parent of scripts/)",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Print one machine-readable readiness report and return fail-closed."""

    args = build_parser().parse_args(argv)
    try:
        repository = args.repository.resolve(strict=True)
        unresolved = unresolved_prerequisites(repository)
    except (OSError, PrerequisiteError) as error:
        print(json.dumps({"error": str(error), "ready": False}, sort_keys=True))
        return 2
    report = {"ready": not unresolved, "unresolved": unresolved}
    print(json.dumps(report, sort_keys=True))
    return 0 if not unresolved else 1


if __name__ == "__main__":
    raise SystemExit(main())
