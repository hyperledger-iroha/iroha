#!/usr/bin/env python3
"""
Reconcile SoraFS capacity fee ledgers against executed XOR transfers.

The capacity registry exposes expected settlements and penalties per provider in
`fee_ledger` entries (via `/v1/sorafs/capacity/state`). This helper ingests that
snapshot alongside a ledger export (JSON or NDJSON with provider IDs, amounts,
and transfer kinds) and reports any missing/overpaid settlements or penalties.
It can also emit Prometheus textfile metrics to feed alert rules and dashboards.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import sys
from pathlib import Path
from typing import Any, Dict, Iterable, List, Tuple


class CapacityLedgerError(Exception):
    """Raised when inputs are malformed."""


def _now_iso() -> str:
    return _dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z"


def _load_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as err:  # pragma: no cover - handled in CLI
        raise CapacityLedgerError(f"snapshot `{path}` not found") from err
    except json.JSONDecodeError as err:  # pragma: no cover - handled in CLI
        raise CapacityLedgerError(f"snapshot `{path}` is invalid JSON: {err}") from err


def _parse_int(value: Any, label: str) -> int:
    if isinstance(value, bool):
        raise CapacityLedgerError(f"{label} must be numeric, not boolean")
    if isinstance(value, (int, float)):
        return int(value)
    if isinstance(value, str):
        text = value.strip()
        if not text:
            raise CapacityLedgerError(f"{label} must not be empty")
        try:
            return int(text, 10)
        except ValueError as err:
            raise CapacityLedgerError(f"{label} must be an integer: {value!r}") from err
    raise CapacityLedgerError(f"{label} must be numeric (got {type(value).__name__})")


def _normalize_hex(value: str) -> str:
    text = value.lower().strip()
    if text.startswith("0x"):
        text = text[2:]
    if len(text) != 64 or any(ch not in "0123456789abcdef" for ch in text):
        raise CapacityLedgerError(f"provider_id_hex must be 64 hex chars (got `{value}`)")
    return text


def _extract_fee_ledger(raw: Any) -> List[Dict[str, Any]]:
    if isinstance(raw, list):
        return raw
    if isinstance(raw, dict):
        if "fee_ledger" in raw:
            ledger = raw["fee_ledger"]
            if isinstance(ledger, list):
                return ledger
            raise CapacityLedgerError("`fee_ledger` must be a list")
    raise CapacityLedgerError("snapshot must be a list or contain `fee_ledger` list")


def _load_snapshot(path: Path) -> Dict[str, Dict[str, int]]:
    data = _load_json(path)
    ledger_entries = _extract_fee_ledger(data)
    expectations: Dict[str, Dict[str, int]] = {}
    for entry in ledger_entries:
        if not isinstance(entry, dict):
            raise CapacityLedgerError("fee ledger entries must be objects")
        provider_raw = entry.get("provider_id_hex") or entry.get("provider_id") or ""
        provider_hex = _normalize_hex(str(provider_raw))
        settlement = _parse_int(
            entry.get("expected_settlement_nano", 0),
            f"expected_settlement_nano for {provider_hex}",
        )
        penalty = _parse_int(
            entry.get("penalty_slashed_nano", 0),
            f"penalty_slashed_nano for {provider_hex}",
        )
        expectations[provider_hex] = {
            "expected_settlement_nano": settlement,
            "expected_penalty_nano": penalty,
        }
    return expectations


def _load_transfers(paths: Iterable[Path]) -> List[Dict[str, Any]]:
    transfers: List[Dict[str, Any]] = []
    for path in paths:
        try:
            text = path.read_text(encoding="utf-8")
        except FileNotFoundError as err:  # pragma: no cover - handled in CLI
            raise CapacityLedgerError(f"ledger export `{path}` not found") from err
        except OSError as err:  # pragma: no cover - handled in CLI
            raise CapacityLedgerError(f"failed to read ledger export `{path}`: {err}") from err
        stripped = text.strip()
        if not stripped:
            continue
        records: List[Any]
        if stripped.lstrip().startswith("["):
            try:
                records = json.loads(stripped)
            except json.JSONDecodeError as err:
                raise CapacityLedgerError(f"ledger export `{path}` is invalid JSON: {err}") from err
            if not isinstance(records, list):
                raise CapacityLedgerError(f"ledger export `{path}` must be a JSON array")
        else:
            records = []
            for line in stripped.splitlines():
                line_text = line.strip()
                if not line_text:
                    continue
                try:
                    records.append(json.loads(line_text))
                except json.JSONDecodeError as err:
                    raise CapacityLedgerError(
                        f"ledger export `{path}` contains invalid JSON line: {err}"
                    ) from err
        for record in records:
            transfers.append(_normalize_transfer(record, source=path))
    return transfers


def _normalize_transfer(record: Dict[str, Any], *, source: Path) -> Dict[str, Any]:
    if not isinstance(record, dict):
        raise CapacityLedgerError(f"ledger export `{source}` entries must be objects")
    provider_raw = record.get("provider_id_hex") or record.get("provider_id")
    if provider_raw is None:
        raise CapacityLedgerError(f"ledger export `{source}` missing `provider_id_hex`")
    provider_hex = _normalize_hex(str(provider_raw))
    kind = (record.get("kind") or "settlement").strip().lower()
    if kind not in {"settlement", "penalty"}:
        raise CapacityLedgerError(
            f"ledger export `{source}` has unsupported kind `{kind}` (expected settlement|penalty)"
        )
    amount = _parse_int(record.get("amount_nano", 0), f"amount_nano for {provider_hex}")
    epoch = record.get("epoch")
    return {
        "provider_id_hex": provider_hex,
        "kind": kind,
        "amount_nano": amount,
        "epoch": epoch,
        "raw": record,
    }


def _reconcile(
    expected: Dict[str, Dict[str, int]],
    transfers: List[Dict[str, Any]],
    *,
    label: str | None = None,
) -> Dict[str, Any]:
    actual: Dict[Tuple[str, str], int] = {}
    unexpected: List[Dict[str, Any]] = []
    for item in transfers:
        provider = item["provider_id_hex"]
        kind = item["kind"]
        if provider not in expected:
            unexpected.append(item)
            continue
        key = (provider, kind)
        actual[key] = actual.get(key, 0) + item["amount_nano"]

    providers: List[Dict[str, Any]] = []
    totals = {
        "expected_settlement_nano": 0,
        "actual_settlement_nano": 0,
        "expected_penalty_nano": 0,
        "actual_penalty_nano": 0,
        "missing_settlement": 0,
        "overpaid_settlement": 0,
        "missing_penalty": 0,
        "overpaid_penalty": 0,
    }

    for provider, values in expected.items():
        expected_settlement = values["expected_settlement_nano"]
        expected_penalty = values["expected_penalty_nano"]
        actual_settlement = actual.get((provider, "settlement"), 0)
        actual_penalty = actual.get((provider, "penalty"), 0)
        settlement_diff = expected_settlement - actual_settlement
        penalty_diff = expected_penalty - actual_penalty

        if settlement_diff > 0:
            totals["missing_settlement"] += 1
        elif settlement_diff < 0:
            totals["overpaid_settlement"] += 1
        if penalty_diff > 0:
            totals["missing_penalty"] += 1
        elif penalty_diff < 0:
            totals["overpaid_penalty"] += 1

        totals["expected_settlement_nano"] += expected_settlement
        totals["expected_penalty_nano"] += expected_penalty
        totals["actual_settlement_nano"] += actual_settlement
        totals["actual_penalty_nano"] += actual_penalty

        providers.append(
            {
                "provider_id_hex": provider,
                "expected_settlement_nano": expected_settlement,
                "actual_settlement_nano": actual_settlement,
                "settlement_diff_nano": settlement_diff,
                "expected_penalty_nano": expected_penalty,
                "actual_penalty_nano": actual_penalty,
                "penalty_diff_nano": penalty_diff,
                "status": _status_label(settlement_diff, penalty_diff),
            }
        )

    providers.sort(key=lambda entry: entry["provider_id_hex"])
    summary = {
        "label": label,
        "generated_at_utc": _now_iso(),
        "providers": providers,
        "totals": totals,
        "unexpected_transfers": unexpected,
    }
    return summary


def _status_label(settlement_diff: int, penalty_diff: int) -> str:
    if settlement_diff == 0 and penalty_diff == 0:
        return "ok"
    labels: List[str] = []
    if settlement_diff > 0:
        labels.append("settlement_missing")
    elif settlement_diff < 0:
        labels.append("settlement_overpaid")
    if penalty_diff > 0:
        labels.append("penalty_missing")
    elif penalty_diff < 0:
        labels.append("penalty_overpaid")
    return "|".join(labels)


def _render_prom(summary: Dict[str, Any]) -> str:
    label = summary.get("label") or "default"
    totals = summary["totals"]
    lines = [
        "# TYPE sorafs_capacity_reconciliation_missing_total gauge",
        f'sorafs_capacity_reconciliation_missing_total{{label="{label}",kind="settlement"}} {totals["missing_settlement"]}',
        f'sorafs_capacity_reconciliation_missing_total{{label="{label}",kind="penalty"}} {totals["missing_penalty"]}',
        "# TYPE sorafs_capacity_reconciliation_overpaid_total gauge",
        f'sorafs_capacity_reconciliation_overpaid_total{{label="{label}",kind="settlement"}} {totals["overpaid_settlement"]}',
        f'sorafs_capacity_reconciliation_overpaid_total{{label="{label}",kind="penalty"}} {totals["overpaid_penalty"]}',
        "# TYPE sorafs_capacity_reconciliation_unexpected_transfers_total gauge",
        f'sorafs_capacity_reconciliation_unexpected_transfers_total{{label="{label}"}} {len(summary.get("unexpected_transfers", []))}',
        "# TYPE sorafs_capacity_reconciliation_expected_nano gauge",
        f'sorafs_capacity_reconciliation_expected_nano{{label="{label}",kind="settlement"}} {totals["expected_settlement_nano"]}',
        f'sorafs_capacity_reconciliation_expected_nano{{label="{label}",kind="penalty"}} {totals["expected_penalty_nano"]}',
        "# TYPE sorafs_capacity_reconciliation_actual_nano gauge",
        f'sorafs_capacity_reconciliation_actual_nano{{label="{label}",kind="settlement"}} {totals["actual_settlement_nano"]}',
        f'sorafs_capacity_reconciliation_actual_nano{{label="{label}",kind="penalty"}} {totals["actual_penalty_nano"]}',
    ]
    return "\n".join(lines) + "\n"


def _print_human(summary: Dict[str, Any]) -> None:
    totals = summary["totals"]
    print(f"label={summary.get('label') or 'default'} generated_at={summary['generated_at_utc']}")
    print(
        f"providers={len(summary['providers'])} "
        f"missing_settlement={totals['missing_settlement']} "
        f"overpaid_settlement={totals['overpaid_settlement']} "
        f"missing_penalty={totals['missing_penalty']} "
        f"overpaid_penalty={totals['overpaid_penalty']} "
        f"unexpected_transfers={len(summary.get('unexpected_transfers', []))}"
    )
    for provider in summary["providers"]:
        if provider["status"] == "ok":
            continue
        print(
            f"provider={provider['provider_id_hex']} status={provider['status']} "
            f"settlement_diff={provider['settlement_diff_nano']} "
            f"penalty_diff={provider['penalty_diff_nano']}"
        )


def _write_json(path: Path, summary: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(summary, handle, indent=2, sort_keys=True)
        handle.write("\n")


def _write_prom(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        handle.write(payload)


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Reconcile SoraFS capacity fee ledger entries against executed transfers.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--snapshot",
        required=True,
        type=Path,
        help="Path to capacity state snapshot JSON (list or object with `fee_ledger`).",
    )
    parser.add_argument(
        "--ledger",
        required=True,
        type=Path,
        action="append",
        help="Path to ledger export (JSON array or NDJSON with provider_id_hex, kind, amount_nano).",
    )
    parser.add_argument("--label", help="Optional label applied to summaries/metrics.")
    parser.add_argument("--json-out", type=Path, help="Write JSON summary to file.")
    parser.add_argument("--prom-out", type=Path, help="Write Prometheus textfile metrics to file.")
    parser.add_argument(
        "--warn-only",
        action="store_true",
        help="Always exit 0 even when mismatches are detected (still prints summary).",
    )
    args = parser.parse_args(argv)

    try:
        expected = _load_snapshot(args.snapshot)
        transfers = _load_transfers(args.ledger)
        summary = _reconcile(expected, transfers, label=args.label)
    except CapacityLedgerError as err:
        print(f"ERROR: {err}", file=sys.stderr)
        return 2

    if args.json_out:
        _write_json(args.json_out, summary)
    if args.prom_out:
        _write_prom(args.prom_out, _render_prom(summary))

    _print_human(summary)
    mismatches = (
        summary["totals"]["missing_settlement"]
        + summary["totals"]["overpaid_settlement"]
        + summary["totals"]["missing_penalty"]
        + summary["totals"]["overpaid_penalty"]
        + len(summary.get("unexpected_transfers", []))
    )
    if mismatches > 0 and not args.warn_only:
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
