#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Generate Prometheus metrics from one verified declarative alias setup plan.

The input is the secret-free ``AliasTransactionPlanV1`` written by the typed
planner client. Metrics describe planner dispositions, exact quotes, caps, and
diagnostics only. They never manufacture payment or submission evidence.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import defaultdict
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Dict, List


class MetricsError(ValueError):
    """Raised when an input is not an executable canonical plan document."""


def _object(value: object, context: str) -> Dict[str, object]:
    if not isinstance(value, dict):
        raise MetricsError(f"{context} must be a JSON object")
    return value


def _array(value: object, context: str) -> List[object]:
    if not isinstance(value, list):
        raise MetricsError(f"{context} must be a JSON array")
    return value


def _decimal(value: object, context: str) -> Decimal:
    if isinstance(value, bool) or not isinstance(value, (int, str)):
        raise MetricsError(f"{context} must be a canonical decimal integer or string")
    rendered = str(value)
    if re.fullmatch(r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?", rendered) is None:
        raise MetricsError(f"{context} must be a canonical non-negative decimal value")
    try:
        amount = Decimal(rendered)
    except InvalidOperation as error:
        raise MetricsError(f"{context} must be a canonical decimal value") from error
    if not amount.is_finite() or amount < 0:
        raise MetricsError(f"{context} must be a finite non-negative value")
    return amount


def _format_decimal(value: Decimal) -> str:
    rendered = format(value, "f")
    if "." in rendered:
        rendered = rendered.rstrip("0").rstrip(".")
    return rendered or "0"


def _enum_kind(value: object, context: str) -> str:
    if isinstance(value, str):
        marker = value
    else:
        marker_value = _object(value, context).get("kind")
        if not isinstance(marker_value, str):
            raise MetricsError(f"{context}.kind must be a string")
        marker = marker_value
    normalized = "".join(character for character in marker.lower() if character.isalnum())
    if not normalized:
        raise MetricsError(f"{context} kind must not be empty")
    return normalized


def _resource_kind(resource: Dict[str, object], index: int) -> str:
    marker = _enum_kind(resource.get("intent"), f"plan resource {index} intent")
    kinds = {
        "dataspace": "dataspace",
        "dataspacealias": "dataspace",
        "domain": "domain",
        "domainalias": "domain",
        "account": "account_alias",
        "accountalias": "account_alias",
    }
    try:
        return kinds[marker]
    except KeyError as error:
        raise MetricsError(f"plan resource {index} has an unknown intent kind") from error


def _disposition(resource: Dict[str, object], index: int) -> str:
    marker = _enum_kind(resource.get("disposition"), f"plan resource {index} disposition")
    dispositions = {
        "noop": "no_op",
        "repair": "repair",
        "create": "create",
        "conflict": "conflict",
    }
    try:
        return dispositions[marker]
    except KeyError as error:
        raise MetricsError(f"plan resource {index} has an unknown disposition") from error


def _escape_label(value: str) -> str:
    return value.replace("\\", "\\\\").replace("\n", "\\n").replace('"', '\\"')


def _load_plan(path: Path) -> Dict[str, object]:
    try:
        loaded = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise MetricsError("plan file must contain valid UTF-8 JSON") from error
    document = _object(loaded, "plan")
    nested = document.get("plan")
    if nested is not None:
        document = _object(nested, "plan.plan")
    body = _object(document.get("body"), "plan.body")
    plan_hash = document.get("plan_hash")
    if not isinstance(plan_hash, str) or re.fullmatch(
        r"hash:[0-9A-F]{64}#[0-9A-F]{4}", plan_hash
    ) is None:
        raise MetricsError("plan.plan_hash must be a non-empty canonical hash")
    blockers = _array(body.get("blockers"), "plan.body.blockers")
    if blockers:
        raise MetricsError("blocked planner output is not an executable plan")
    return body


def render_metrics(plan_path: Path, release: str) -> str:
    """Render plan-only metrics without treating a plan as payment evidence."""

    body = _load_plan(plan_path)
    resources = _array(body.get("resources"), "plan.body.resources")
    totals = _array(body.get("totals_by_asset"), "plan.body.totals_by_asset")
    warnings = _array(body.get("warnings"), "plan.body.warnings")

    resource_counts: Dict[tuple[str, str], int] = defaultdict(int)
    cap_totals: Dict[str, Decimal] = defaultdict(Decimal)
    for index, raw_resource in enumerate(resources):
        resource = _object(raw_resource, f"plan resource {index}")
        resource_kind = _resource_kind(resource, index)
        disposition = _disposition(resource, index)
        if disposition == "conflict":
            raise MetricsError("executable plan must not contain a conflict disposition")
        resource_counts[(resource_kind, disposition)] += 1

        quote = resource.get("quote")
        if quote is None:
            continue
        guard = _object(
            _object(quote, f"plan resource {index} quote").get("guard"),
            f"plan resource {index} quote guard",
        )
        asset = guard.get("expected_payment_asset")
        if not isinstance(asset, str) or not asset:
            raise MetricsError(f"plan resource {index} quote guard payment asset is invalid")
        cap_totals[asset] += _decimal(
            guard.get("max_amount"), f"plan resource {index} quote guard max amount"
        )

    exact_totals: Dict[str, Decimal] = {}
    for index, raw_total in enumerate(totals):
        total = _object(raw_total, f"plan asset total {index}")
        asset = total.get("payment_asset")
        if not isinstance(asset, str) or not asset:
            raise MetricsError(f"plan asset total {index} payment asset is invalid")
        if asset in exact_totals:
            raise MetricsError("plan contains a duplicate payment asset total")
        exact_totals[asset] = _decimal(total.get("amount"), f"plan asset total {index} amount")

    release_label = _escape_label(release)
    lines: List[str] = [
        "# HELP iroha_alias_setup_plan_resources Number of resources in one verified alias setup plan.",
        "# TYPE iroha_alias_setup_plan_resources gauge",
    ]
    for (resource_kind, disposition), count in sorted(resource_counts.items()):
        lines.append(
            f'iroha_alias_setup_plan_resources{{release="{release_label}",resource_kind="{resource_kind}",disposition="{disposition}"}} {count}'
        )

    lines.extend(
        [
            "# HELP iroha_alias_setup_plan_exact_quote_units Exact create-only planner total by payment asset.",
            "# TYPE iroha_alias_setup_plan_exact_quote_units gauge",
        ]
    )
    for asset, amount in sorted(exact_totals.items()):
        lines.append(
            f'iroha_alias_setup_plan_exact_quote_units{{release="{release_label}",asset_id="{_escape_label(asset)}"}} {_format_decimal(amount)}'
        )

    lines.extend(
        [
            "# HELP iroha_alias_setup_plan_cap_units Sum of payer-authorized quote caps by payment asset.",
            "# TYPE iroha_alias_setup_plan_cap_units gauge",
        ]
    )
    for asset, amount in sorted(cap_totals.items()):
        lines.append(
            f'iroha_alias_setup_plan_cap_units{{release="{release_label}",asset_id="{_escape_label(asset)}"}} {_format_decimal(amount)}'
        )

    lines.extend(
        [
            "# HELP iroha_alias_setup_plan_diagnostics Number of non-blocking diagnostics on the verified plan.",
            "# TYPE iroha_alias_setup_plan_diagnostics gauge",
            f'iroha_alias_setup_plan_diagnostics{{release="{release_label}",severity="warning"}} {len(warnings)}',
            f'iroha_alias_setup_plan_diagnostics{{release="{release_label}",severity="blocker"}} 0',
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--plan", type=Path, required=True, help="Verified AliasTransactionPlanV1 JSON"
    )
    parser.add_argument("--release", required=True, help="Release identifier label")
    parser.add_argument("--output", type=Path, required=True, help="Output metrics file")
    args = parser.parse_args()

    try:
        metrics = render_metrics(args.plan, args.release)
        args.output.write_text(metrics, encoding="utf-8")
    except MetricsError as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
