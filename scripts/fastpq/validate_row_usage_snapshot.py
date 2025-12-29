#!/usr/bin/env python3
"""
Validate FASTPQ row_usage snapshots for Stage7-3 evidence bundles.

Release engineering uses this helper from ci/check_fastpq_rollout.sh to ensure
that every `row_usage/*.json` file attached to a rollout bundle advertises the
per-selector row counts and transfer ratio required by the Stage7 telemetry/SLO
contract before GPU lanes can be mandated.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any, Sequence


COUNT_FIELDS = (
    "total_rows",
    "transfer_rows",
    "non_transfer_rows",
    "mint_rows",
    "burn_rows",
    "role_grant_rows",
    "role_revoke_rows",
    "meta_set_rows",
    "permission_rows",
)
RATIO_FIELD = "transfer_ratio"
RATIO_EPSILON = 1e-6


class ValidationContext:
    """Lightweight helper for referencing a row_usage entry in error messages."""

    def __init__(self, path: Path | None, batch_index: int) -> None:
        self.path = path
        self.batch_index = batch_index

    def prefix(self) -> str:
        location = str(self.path) if self.path else "<memory>"
        return f"{location} fastpq_batches[{self.batch_index}]"


class ValidationError(Exception):
    """Raised when a snapshot violates the Stage7 row_usage contract."""


def _require_int(field: str, value: Any, ctx: ValidationContext) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValidationError(f"{ctx.prefix()} row_usage.{field} must be an integer")
    if value < 0:
        raise ValidationError(f"{ctx.prefix()} row_usage.{field} must be ≥ 0 (got {value})")
    return value


def _require_ratio(value: Any, ctx: ValidationContext) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValidationError(f"{ctx.prefix()} row_usage.{RATIO_FIELD} must be numeric")
    ratio = float(value)
    if not (0.0 <= ratio <= 1.0):
        raise ValidationError(
            f"{ctx.prefix()} row_usage.{RATIO_FIELD} must be between 0 and 1 (got {ratio})"
        )
    return ratio


def validate_row_usage_snapshot(payload: Any, path: Path | None = None) -> None:
    """Validate the decoded JSON payload for a FASTPQ row_usage snapshot."""
    if not isinstance(payload, dict):
        raise ValidationError(f"{path or '<memory>'} root must be a JSON object")
    batches = payload.get("fastpq_batches")
    if not isinstance(batches, list) or not batches:
        raise ValidationError(
            f"{path or '<memory>'} fastpq_batches must be a non-empty array"
        )

    for index, batch in enumerate(batches):
        if not isinstance(batch, dict):
            raise ValidationError(
                f"{path or '<memory>'} fastpq_batches[{index}] must be an object"
            )
        usage = batch.get("row_usage")
        if not isinstance(usage, dict):
            raise ValidationError(
                f"{path or '<memory>'} fastpq_batches[{index}] missing row_usage object"
            )
        ctx = ValidationContext(path=path, batch_index=index)
        for field in COUNT_FIELDS:
            if field not in usage:
                raise ValidationError(f"{ctx.prefix()} row_usage.{field} missing")
        if RATIO_FIELD not in usage:
            raise ValidationError(f"{ctx.prefix()} row_usage.{RATIO_FIELD} missing")

        total_rows = _require_int("total_rows", usage["total_rows"], ctx)
        transfer_rows = _require_int("transfer_rows", usage["transfer_rows"], ctx)
        non_transfer_rows = _require_int(
            "non_transfer_rows", usage["non_transfer_rows"], ctx
        )
        if total_rows == 0:
            if transfer_rows != 0 or non_transfer_rows != 0:
                raise ValidationError(
                    f"{ctx.prefix()} total_rows is 0 but transfer/non-transfer rows are non-zero"
                )
        else:
            if transfer_rows + non_transfer_rows != total_rows:
                raise ValidationError(
                    f"{ctx.prefix()} transfer_rows + non_transfer_rows "
                    f"({transfer_rows + non_transfer_rows}) must equal total_rows ({total_rows})"
                )

        ratio = _require_ratio(usage[RATIO_FIELD], ctx)
        expected_ratio = 0.0 if total_rows == 0 else transfer_rows / total_rows
        if abs(ratio - expected_ratio) > RATIO_EPSILON:
            raise ValidationError(
                f"{ctx.prefix()} row_usage.{RATIO_FIELD} ({ratio}) does not match "
                f"transfer_rows / total_rows ({expected_ratio:.6f})"
            )

        for field in COUNT_FIELDS[3:]:
            _require_int(field, usage[field], ctx)


def _parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate FASTPQ row_usage JSON snapshots for Stage7 rollouts."
    )
    parser.add_argument(
        "snapshots",
        nargs="+",
        type=Path,
        help="Path(s) to row_usage JSON files produced by `iroha_cli audit witness --decode`.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    rc = 0
    for snapshot in args.snapshots:
        try:
            payload = json.loads(snapshot.read_text(encoding="utf-8"))
        except FileNotFoundError:
            print(f"[fastpq] row_usage file not found: {snapshot}", file=sys.stderr)
            return 1
        except json.JSONDecodeError as exc:
            print(f"[fastpq] {snapshot} is not valid JSON: {exc}", file=sys.stderr)
            return 1

        try:
            validate_row_usage_snapshot(payload, snapshot)
        except ValidationError as exc:
            print(f"[fastpq] {exc}", file=sys.stderr)
            rc = 1
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
