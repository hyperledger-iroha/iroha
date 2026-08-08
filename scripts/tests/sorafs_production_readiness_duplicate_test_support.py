"""Duplicate-summary assertions shared by the production-readiness tests."""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import Any


def assert_duplicate_gate_summary_fails(
    tmp_path: Path,
    *,
    module: Any,
    write_gate: Callable[[Path, str], Path],
    run_gate: Callable[..., int],
) -> None:
    """Exercise deterministic duplicate diagnostics for one readiness lane."""

    first = write_gate(tmp_path, "gateway_load")
    second = tmp_path / "gateway_load_duplicate.json"
    second.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
    third = tmp_path / "gateway_load_duplicate_2.json"
    third.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    row_errors = result["required"]["gateway_load"]["errors"]
    assert row_errors.count("duplicate gateway_load production readiness summary") == 1
    assert (
        result["errors"].count("duplicate gateway_load production readiness summary")
        == 2
    )

    errors: list[str] = []
    result["required"]["gateway_load"]["errors"] = [
        "duplicate gateway_load production readiness summary",
        "duplicate gateway_load production readiness summary",
    ]
    module.validate_duplicate_summary_diagnostics(
        result["required"],
        {"gateway_load"},
        2,
        errors,
    )
    assert (
        "gateway_load duplicate summary row errors must contain the deterministic duplicate summary diagnostic exactly once"
        in "\n".join(errors)
    )
    errors = []
    result["required"]["gateway_load"]["errors"] = [
        "duplicate gateway_load production readiness summary"
    ]
    module.validate_duplicate_summary_diagnostics(
        result["required"],
        {"gateway_load"},
        3,
        errors,
    )
    assert (
        "aggregate summary duplicate-summary diagnostics must match duplicate summary inputs"
        in "\n".join(errors)
    )


def assert_duplicate_and_unrequired_summaries_fail_closed(
    tmp_path: Path,
    *,
    module: Any,
    write_gate: Callable[[Path, str], Path],
    run_gate: Callable[..., int],
    now_unix: int,
    deployment_id: str,
    environment: str,
    topology_cli_args: Callable[[Path], list[str]],
) -> None:
    """Exercise duplicate and unrequired summary rejection for every lane."""

    gate_names = [gate.name for gate in module.GATE_SUMMARY_KINDS]
    assert len(gate_names) > 1

    for index, gate_name in enumerate(gate_names):
        root = tmp_path / f"{index}_{gate_name}_duplicate"
        root.mkdir()
        first = write_gate(root, gate_name)
        for duplicate_index in (1, 2):
            duplicate = root / f"{gate_name}_duplicate_{duplicate_index}.json"
            duplicate.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
        summary = root / "summary.json"

        assert (
            run_gate(
                root,
                "--require-gate",
                gate_name,
                "--summary-out",
                str(summary),
            )
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        duplicate_error = f"duplicate {gate_name} production readiness summary"
        row_errors = result["required"][gate_name]["errors"]
        assert row_errors.count(duplicate_error) == 1
        assert result["errors"].count(duplicate_error) == 2
        assert f"{gate_name}_duplicate_" not in "\n".join(result["errors"])

    for index, gate_name in enumerate(gate_names):
        unrequired_gate = gate_names[(index + 1) % len(gate_names)]
        root = tmp_path / f"{index}_{gate_name}_unrequired"
        root.mkdir()
        required_summary = write_gate(root, gate_name)
        unrequired_summary = write_gate(root, unrequired_gate)
        summary = root / "summary.json"

        assert (
            module.main(
                [
                    "--evidence",
                    str(required_summary),
                    "--evidence",
                    str(unrequired_summary),
                    "--require-gate",
                    gate_name,
                    "--now-unix",
                    str(now_unix),
                    "--deployment-id",
                    deployment_id,
                    "--environment",
                    environment,
                    *topology_cli_args(tmp_path),
                    "--summary-out",
                    str(summary),
                ]
            )
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        errors = "\n".join(result["errors"])
        assert (
            result["errors"].count(
                "explicit production readiness summary belongs to unrequired gate"
            )
            == 1
        )
        assert module.GATE_BY_NAME[unrequired_gate].schema not in errors
        assert f"{unrequired_gate}` gate" not in errors
