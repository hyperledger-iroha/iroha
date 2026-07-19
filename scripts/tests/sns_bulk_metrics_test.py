"""Tests for declarative alias setup plan metrics."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "sns_bulk_metrics.py"
SPEC = importlib.util.spec_from_file_location("sns_bulk_metrics", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)  # type: ignore[attr-defined]

PLAN_HASH = (
    "hash:1112131415161718191A1B1C1D1E1F202122232425262728292A2B2C2D2E2F31#011A"
)


def _resource(
    kind: str,
    disposition: str,
    *,
    asset: str | None = None,
    cap: str | None = None,
) -> dict[str, object]:
    quote = None
    if asset is not None and cap is not None:
        quote = {
            "exact_amount": cap,
            "guard": {
                "expected_policy_version": 1,
                "expected_payment_asset": asset,
                "max_amount": cap,
                "valid_until_ms": 1_900_000_000_000,
            },
        }
    return {
        "intent": {"kind": kind, "intent": {"owner": "fixture"}},
        "disposition": {"kind": disposition, "value": None},
        "quote": quote,
        "instruction_index": 0 if disposition in {"create", "repair"} else None,
    }


def _plan() -> dict[str, object]:
    return {
        "body": {
            "version": 1,
            "authority": "ed0120payer",
            "chain_id": "fixture-chain",
            "anchor": {"block_height": 1, "block_hash": "hash:fixture"},
            "resources": [
                _resource("dataspace", "no_op"),
                _resource("domain", "repair"),
                _resource("account_alias", "create", asset="xor#sora", cap="3.25"),
                _resource("account_alias", "create", asset="xor#sora", cap="4.75"),
            ],
            "instructions": [],
            "totals_by_asset": [
                {"payment_asset": "xor#sora", "amount": "6.5"},
            ],
            "warnings": [{"code": "alias.parent_expiry.warning"}],
            "blockers": [],
            "valid_until_ms": 1_900_000_000_000,
        },
        "plan_hash": PLAN_HASH,
    }


def test_metrics_use_plan_dispositions_exact_quotes_and_caps(tmp_path: Path) -> None:
    plan_path = tmp_path / "plan.json"
    plan_path.write_text(json.dumps(_plan()), encoding="utf-8")

    metrics = MODULE.render_metrics(plan_path, "release-1")

    assert 'resource_kind="dataspace",disposition="no_op"} 1' in metrics
    assert 'resource_kind="domain",disposition="repair"} 1' in metrics
    assert 'resource_kind="account_alias",disposition="create"} 2' in metrics
    assert 'exact_quote_units{release="release-1",asset_id="xor#sora"} 6.5' in metrics
    assert 'cap_units{release="release-1",asset_id="xor#sora"} 8' in metrics
    assert 'severity="warning"} 1' in metrics
    assert 'severity="blocker"} 0' in metrics
    assert "submission" not in metrics
    assert "payment_gross" not in metrics
    assert "payment_net" not in metrics


def test_metrics_accept_ready_envelope_and_escape_labels(tmp_path: Path) -> None:
    plan_path = tmp_path / "plan.json"
    plan_path.write_text(
        json.dumps({"status": "Ready", "plan": _plan()}), encoding="utf-8"
    )

    metrics = MODULE.render_metrics(plan_path, 'release"\nnext')

    assert 'release="release\\"\\nnext"' in metrics


def test_metrics_reject_blocked_or_conflicting_output(tmp_path: Path) -> None:
    blocked = _plan()
    blocked["body"]["blockers"] = [{"code": "alias.blocked"}]
    blocked_path = tmp_path / "blocked.json"
    blocked_path.write_text(json.dumps(blocked), encoding="utf-8")

    with pytest.raises(MODULE.MetricsError, match="not an executable plan"):
        MODULE.render_metrics(blocked_path, "release")

    conflict = _plan()
    conflict["body"]["resources"][0]["disposition"] = {
        "kind": "conflict",
        "value": None,
    }
    conflict_path = tmp_path / "conflict.json"
    conflict_path.write_text(json.dumps(conflict), encoding="utf-8")

    with pytest.raises(MODULE.MetricsError, match="conflict disposition"):
        MODULE.render_metrics(conflict_path, "release")


def test_metrics_reject_legacy_intent_or_submission_log_shape(tmp_path: Path) -> None:
    legacy_path = tmp_path / "manifest.json"
    legacy_path.write_text(
        json.dumps({"dataspaces": [], "domains": [], "accounts": []}),
        encoding="utf-8",
    )

    with pytest.raises(MODULE.MetricsError, match="plan.body"):
        MODULE.render_metrics(legacy_path, "release")
