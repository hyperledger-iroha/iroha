#!/usr/bin/env python3
"""
Summarise `sorafs reserve ledger` output for dashboards and evidence bundles.

The CLI emits JSON describing the rent that must be paid, the reserve top-up,
and the sequence of Norito `Transfer` instructions that treasury automation
should apply. Amounts are canonical exact XOR decimal strings. This helper
validates that public contract without a legacy micro-XOR conversion, computes
a digest, and optionally writes Markdown/JSON summaries that can be attached
to runbooks or piped into automation.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import re
from decimal import Decimal
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence

_CANONICAL_XOR = re.compile(r"(?:0|[1-9][0-9]*)(?:\.([0-9]{1,9}))?\Z")
_MAX_QUANTITY_MANTISSA = (1 << 511) - 1


def _current_timestamp() -> str:
    """Return an ISO-8601 timestamp without microseconds."""

    return _dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z"


def _load_json(path: Path) -> Dict[str, Any]:
    def reject_duplicate_keys(pairs: List[tuple[str, Any]]) -> Dict[str, Any]:
        result: Dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON field `{key}`")
            result[key] = value
        return result

    def reject_nonfinite(token: str) -> Any:
        raise ValueError(f"non-finite JSON number `{token}`")

    try:
        with path.open("r", encoding="utf-8") as handle:
            value = json.load(
                handle,
                object_pairs_hook=reject_duplicate_keys,
                parse_constant=reject_nonfinite,
            )
    except FileNotFoundError as err:
        raise SystemExit(f"ledger JSON `{path}` is missing") from err
    except (json.JSONDecodeError, UnicodeDecodeError, ValueError) as err:
        raise SystemExit(f"ledger JSON `{path}` is invalid: {err}") from err
    if type(value) is not dict:
        raise SystemExit(f"ledger JSON `{path}` must be an object")
    return value


def _parse_xor_quantity(value: Any, field: str) -> Optional[Decimal]:
    """Parse one canonical, non-negative XOR quantity with at most nine decimals."""

    if value is None:
        return None
    if type(value) is not str:
        raise SystemExit(f"ledger `{field}` must be a canonical XOR decimal string")
    match = _CANONICAL_XOR.fullmatch(value)
    if match is None or (match.group(1) is not None and match.group(1).endswith("0")):
        raise SystemExit(f"ledger `{field}` is not a canonical XOR decimal string")
    mantissa = int(value.replace(".", ""))
    if mantissa > _MAX_QUANTITY_MANTISSA:
        raise SystemExit(f"ledger `{field}` exceeds the bounded XOR quantity domain")
    return Decimal(value)


def _format_decimal(value: Optional[Decimal]) -> Optional[str]:
    if value is None:
        return None
    return format(value, "f")


def _default_label(path: Path) -> str:
    stem = path.stem
    if stem:
        return stem
    return path.name or str(path)


def _summarise(
    ledger: Dict[str, Any],
    label: Optional[str],
    *,
    generated_at: Optional[str] = None,
) -> Dict[str, Any]:
    required_amounts = ("rent_due", "reserve_shortfall", "top_up_shortfall")
    missing = [field for field in required_amounts if field not in ledger]
    if missing:
        raise SystemExit(f"ledger JSON is missing required field `{missing[0]}`")
    rent_xor = _parse_xor_quantity(ledger["rent_due"], "rent_due")
    reserve_shortfall_xor = _parse_xor_quantity(
        ledger["reserve_shortfall"], "reserve_shortfall"
    )
    top_up_shortfall_xor = _parse_xor_quantity(
        ledger["top_up_shortfall"], "top_up_shortfall"
    )
    instructions = ledger.get("instructions") or []
    transfers = _build_transfer_entries(rent_xor, reserve_shortfall_xor)
    timestamp = generated_at or _current_timestamp()
    summary = {
        "label": label,
        "quote_path": ledger.get("quote_path"),
        "generated_at_utc": timestamp,
        "instruction_count": len(instructions),
        "rent_due_xor": _format_decimal(rent_xor),
        "reserve_shortfall_xor": _format_decimal(reserve_shortfall_xor),
        "top_up_shortfall_xor": _format_decimal(top_up_shortfall_xor),
        "requires_top_up": bool(top_up_shortfall_xor and top_up_shortfall_xor > 0),
        "meets_underwriting": bool(reserve_shortfall_xor is None or reserve_shortfall_xor == 0),
        "transfers": transfers,
    }
    return summary


def _build_transfer_entries(
    rent_xor: Optional[Decimal],
    reserve_shortfall_xor: Optional[Decimal],
) -> List[Dict[str, Any]]:
    transfers: List[Dict[str, Any]] = []
    if rent_xor is not None:
        transfers.append(
            {
                "kind": "rent",
                "amount_xor": _format_decimal(rent_xor) or "0",
            }
        )
    if reserve_shortfall_xor is not None:
        transfers.append(
            {
                "kind": "reserve_top_up",
                "amount_xor": _format_decimal(reserve_shortfall_xor) or "0",
            }
        )
    return transfers


def _write_json(path: Path, summaries: List[Dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload: Any
    if len(summaries) == 1:
        payload = summaries[0]
    else:
        payload = summaries
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2, sort_keys=True)
        handle.write("\n")


def _write_markdown(path: Path, summaries: List[Dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if len(summaries) == 1:
        summary = summaries[0]
        lines = [
            "# Reserve Ledger Digest",
            "",
            f"*Generated:* {summary['generated_at_utc']}",
        ]
        if summary.get("label"):
            lines.append(f"*Label:* `{summary['label']}`")
        if summary.get("quote_path"):
            lines.append(f"*Quote:* `{summary['quote_path']}`")
        lines.extend(
            [
                "",
                "| Field | Value |",
                "|-------|-------|",
                f"| Instruction count | {summary['instruction_count']} |",
                f"| Rent due (XOR) | {summary.get('rent_due_xor') or '0'} |",
                f"| Reserve shortfall (XOR) | {summary.get('reserve_shortfall_xor') or '0'} |",
                f"| Top-up shortfall (XOR) | {summary.get('top_up_shortfall_xor') or '0'} |",
                f"| Requires top-up | {summary['requires_top_up']} |",
                f"| Meets underwriting | {summary['meets_underwriting']} |",
            ]
        )
        transfers = summary.get("transfers") or []
        if transfers:
            lines.extend(
                ["", "## Transfer Feed", "", "| Kind | Amount (XOR) |", "|------|--------------|"]
            )
            for transfer in transfers:
                lines.append(f"| {transfer['kind']} | {transfer.get('amount_xor') or '0'} |")
    else:
        lines = [
            "# Reserve Ledger Batch Digest",
            "",
            "| Label | Rent due (XOR) | Reserve shortfall (XOR) | Top-up shortfall (XOR) | "
            "Instructions | Meets underwriting | Requires top-up |",
            "|-------|----------------|-------------------------|------------------------|"
            "-------------|-------------------|------------------|",
        ]
        for summary in summaries:
            lines.append(
                f"| {summary.get('label') or 'ledger'} | "
                f"{summary.get('rent_due_xor') or '0'} | "
                f"{summary.get('reserve_shortfall_xor') or '0'} | "
                f"{summary.get('top_up_shortfall_xor') or '0'} | "
                f"{summary.get('instruction_count')} | "
                f"{summary.get('meets_underwriting')} | "
                f"{summary.get('requires_top_up')} |"
            )
        for summary in summaries:
            lines.extend(
                [
                    "",
                    f"## {summary.get('label') or 'Ledger'}",
                    "",
                    f"*Generated:* {summary['generated_at_utc']}",
                ]
            )
            if summary.get("quote_path"):
                lines.append(f"*Quote:* `{summary['quote_path']}`")
            lines.extend(
                [
                    "",
                    "| Field | Value |",
                    "|-------|-------|",
                    f"| Instruction count | {summary['instruction_count']} |",
                    f"| Rent due (XOR) | {summary.get('rent_due_xor') or '0'} |",
                    f"| Reserve shortfall (XOR) | {summary.get('reserve_shortfall_xor') or '0'} |",
                    f"| Top-up shortfall (XOR) | {summary.get('top_up_shortfall_xor') or '0'} |",
                    f"| Requires top-up | {summary['requires_top_up']} |",
                    f"| Meets underwriting | {summary['meets_underwriting']} |",
                ]
            )
            transfers = summary.get("transfers") or []
            if transfers:
                lines.extend(
                    ["", "### Transfer Feed", "", "| Kind | Amount (XOR) |", "|------|--------------|"]
                )
                for transfer in transfers:
                    lines.append(
                        f"| {transfer['kind']} | {transfer.get('amount_xor') or '0'} |"
                    )

    with path.open("w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
        handle.write("\n")


def _escape_label(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def _format_labels(summary: Dict[str, Any]) -> str:
    labels = []
    label_value = summary.get("label") or "ledger"
    labels.append(f'label="{_escape_label(label_value)}"')
    quote_path = summary.get("quote_path")
    if quote_path:
        quote_label = Path(quote_path).name or quote_path
        labels.append(f'quote="{_escape_label(quote_label)}"')
    return ",".join(labels)


def _prometheus_lines(summary: Dict[str, Any]) -> List[str]:
    """Emit a textfile-friendly Prometheus snapshot for dashboards/alerts."""

    def _value(key: str) -> str:
        raw = summary.get(key)
        if raw is None:
            return "0"
        return format(Decimal(str(raw)), "f")

    labels = _format_labels(summary)
    prom_lines = [
        "# HELP sorafs_reserve_ledger_rent_due_xor Rent due for the ledger projection (XOR)",
        "# TYPE sorafs_reserve_ledger_rent_due_xor gauge",
        f"sorafs_reserve_ledger_rent_due_xor{{{labels}}} {_value('rent_due_xor')}",
        "# HELP sorafs_reserve_ledger_reserve_shortfall_xor Reserve shortfall for underwriting (XOR)",
        "# TYPE sorafs_reserve_ledger_reserve_shortfall_xor gauge",
        f"sorafs_reserve_ledger_reserve_shortfall_xor{{{labels}}} {_value('reserve_shortfall_xor')}",
        "# HELP sorafs_reserve_ledger_top_up_shortfall_xor XOR still required to top up the reserve account",
        "# TYPE sorafs_reserve_ledger_top_up_shortfall_xor gauge",
        f"sorafs_reserve_ledger_top_up_shortfall_xor{{{labels}}} {_value('top_up_shortfall_xor')}",
        "# HELP sorafs_reserve_ledger_requires_top_up Whether the ledger projection still requires a top-up (1=yes)",
        "# TYPE sorafs_reserve_ledger_requires_top_up gauge",
        f"sorafs_reserve_ledger_requires_top_up{{{labels}}} "
        f"{1.0 if summary.get('requires_top_up') else 0.0}",
        "# HELP sorafs_reserve_ledger_meets_underwriting Whether the ledger projection meets underwriting requirements (1=yes)",
        "# TYPE sorafs_reserve_ledger_meets_underwriting gauge",
        f"sorafs_reserve_ledger_meets_underwriting{{{labels}}} "
        f"{1.0 if summary.get('meets_underwriting') else 0.0}",
        "# HELP sorafs_reserve_ledger_instruction_total Number of transfer instructions emitted by the ledger planner",
        "# TYPE sorafs_reserve_ledger_instruction_total gauge",
        f"sorafs_reserve_ledger_instruction_total{{{labels}}} {summary.get('instruction_count', 0)}",
    ]
    transfers = summary.get("transfers") or []
    if transfers:
        prom_lines.extend(
            [
                "# HELP sorafs_reserve_ledger_transfer_xor Ledger transfer amounts grouped by transfer kind (XOR)",
                "# TYPE sorafs_reserve_ledger_transfer_xor gauge",
            ]
        )
    for transfer in transfers:
        amount = transfer.get("amount_xor")
        try:
            value = format(Decimal(str(amount)), "f") if amount is not None else "0"
        except Exception:  # pragma: no cover - defensive
            value = "0"
        transfer_labels = f'{labels},transfer="{_escape_label(transfer.get("kind", "unknown"))}"'
        prom_lines.append(
            f"sorafs_reserve_ledger_transfer_xor{{{transfer_labels}}} {value}"
        )
    return prom_lines


def _write_prometheus(path: Path, summaries: List[Dict[str, Any]]) -> None:
    lines: List[str] = []
    for summary in summaries:
        lines.extend(_prometheus_lines(summary))
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
        handle.write("\n")


def _write_ndjson(path: Path, summaries: List[Dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for summary in summaries:
            handle.write(json.dumps(summary, sort_keys=True))
            handle.write("\n")


def summarize_ledgers(
    ledger_paths: Sequence[Path],
    labels: Optional[Sequence[Optional[str]]] = None,
) -> List[Dict[str, Any]]:
    if not ledger_paths:
        raise SystemExit("at least one --ledger path must be provided")

    normalized_labels: List[Optional[str]] = []
    if labels:
        if len(ledger_paths) > 1 and len(labels) not in (0, len(ledger_paths)):
            raise SystemExit(
                "when supplying multiple ledger files you must pass the same number of --label values"
            )
        if len(ledger_paths) == 1 and labels:
            normalized_labels = [labels[0]]
        else:
            normalized_labels = list(labels)

    summaries: List[Dict[str, Any]] = []
    timestamp = _current_timestamp()
    for idx, ledger_path in enumerate(ledger_paths):
        ledger_data = _load_json(ledger_path)
        label = None
        if normalized_labels:
            if len(normalized_labels) == len(ledger_paths):
                label = normalized_labels[idx]
            elif len(normalized_labels) == 1:
                label = normalized_labels[0]
        summary = _summarise(
            ledger_data,
            label or _default_label(ledger_path),
            generated_at=timestamp,
        )
        summary["ledger_path"] = str(ledger_path)
        summaries.append(summary)
    return summaries


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Summarise `sorafs reserve ledger` output for dashboards/evidence."
    )
    parser.add_argument(
        "--ledger",
        dest="ledgers",
        action="append",
        required=True,
        type=Path,
        help="Path to a ledger JSON output (repeat for multiple files)",
    )
    parser.add_argument(
        "--label",
        dest="labels",
        action="append",
        help="Optional label recorded per ledger (repeat to override defaults)",
    )
    parser.add_argument("--out-json", type=Path, help="Write structured summary JSON to this path")
    parser.add_argument("--out-md", type=Path, help="Write a Markdown digest to this path")
    parser.add_argument(
        "--ndjson-out",
        type=Path,
        help="Write newline-delimited JSON summaries to this path",
    )
    parser.add_argument(
        "--print",
        action="store_true",
        help="Print the summary JSON to stdout (useful for scripting)",
    )
    parser.add_argument(
        "--out-prom",
        type=Path,
        help="Write Prometheus textfile metrics for dashboards/alerts",
    )
    args = parser.parse_args()

    raw_labels = args.labels or []
    summaries = summarize_ledgers(args.ledgers, raw_labels)

    if args.out_json:
        _write_json(args.out_json, summaries)
    if args.out_md:
        _write_markdown(args.out_md, summaries)
    if args.ndjson_out:
        _write_ndjson(args.ndjson_out, summaries)
    if args.out_prom:
        _write_prometheus(args.out_prom, summaries)
    if args.print or (
        not args.out_json and not args.out_md and not args.ndjson_out and not args.out_prom
    ):
        payload: Any
        if len(summaries) == 1:
            payload = summaries[0]
        else:
            payload = summaries
        print(json.dumps(payload, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
