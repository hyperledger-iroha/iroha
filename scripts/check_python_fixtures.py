#!/usr/bin/env python3
"""Verify Python's generated Norito RPC descriptor mirrors."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
from pathlib import Path
from typing import Iterable, List, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from norito_fixture_frame import (
    SIGNED_TRANSACTION_SCHEMA,
    TRANSACTION_PAYLOAD_SCHEMA,
    decode_canonical_norito_frame,
    iroha_hash_hex,
    signed_transaction_entrypoint_hash_hex,
    signed_transaction_payload,
)

DEFAULT_SOURCE = Path("fixtures/norito_rpc")
DEFAULT_TARGET = Path("python/iroha_python/tests/fixtures")
MANAGED_FIXTURES = (
    Path("transaction_payloads.json"),
    Path("transaction_fixtures.manifest.json"),
)
MAX_U64 = 0xFFFF_FFFF_FFFF_FFFF
PAYLOAD_ENTRY_FIELDS = frozenset(
    {
        "authority",
        "creation_time_ms",
        "name",
        "network_id",
        "nonce",
        "payload",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "time_to_live_ms",
    }
)
PAYLOAD_FIELDS = frozenset(
    {
        "admission_intent",
        "authority",
        "creation_time_ms",
        "executable",
        "fee_payment",
        "metadata",
        "network_id",
        "nonce",
        "time_to_live_ms",
    }
)
EXECUTABLE_VARIANTS = frozenset({"Batch", "ContractCall", "Instructions", "Ivm"})


class DuplicateJsonKeyError(ValueError):
    """Raised before a last-wins JSON decoder can hide duplicate fields."""


def _reject_duplicate_json_keys(pairs: list[tuple[str, object]]) -> dict:
    result: dict = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateJsonKeyError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def parse_json_strict(path: Path) -> object:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except (DuplicateJsonKeyError, json.JSONDecodeError) as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc


def require_exact_fields(record: dict, expected: frozenset[str], context: str) -> None:
    actual = set(record)
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing or unexpected:
        raise ValueError(
            f"{context} has invalid fields: missing={missing}, unexpected={unexpected}"
        )


def validate_admission_intent(value: object, context: str) -> None:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(value, frozenset({"intent", "value"}), context)
    if value["intent"] != "ordinary" or value["value"] is not None:
        raise ValueError(
            f"{context} must be exactly {{'intent': 'ordinary', 'value': null}}"
        )


def validate_fee_payment(value: object, context: str) -> int | None:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(value, frozenset({"payer", "value"}), context)
    if value["payer"] != "authority":
        raise ValueError(f"{context}.payer must be exactly 'authority'")
    fee_value = value["value"]
    if not isinstance(fee_value, dict):
        raise ValueError(f"{context}.value must be an object")
    require_exact_fields(
        fee_value,
        frozenset({"charge_limits", "gas_limit"}),
        f"{context}.value",
    )
    if not isinstance(fee_value["charge_limits"], list):
        raise ValueError(f"{context}.value.charge_limits must be an array")
    gas_limit = fee_value["gas_limit"]
    if gas_limit is not None and (
        not isinstance(gas_limit, int)
        or isinstance(gas_limit, bool)
        or gas_limit < 1
        or gas_limit > MAX_U64
    ):
        raise ValueError(
            f"{context}.value.gas_limit must be null or an integer in 1..={MAX_U64}"
        )
    return gas_limit


def executable_requires_gas_limit(value: object, context: str) -> bool:
    if not isinstance(value, dict) or len(value) != 1:
        raise ValueError(f"{context} must contain exactly one executable variant")
    variant, body = next(iter(value.items()))
    if variant not in EXECUTABLE_VARIANTS:
        raise ValueError(f"{context} has unknown variant {variant!r}")
    if variant == "Ivm":
        if not isinstance(body, str):
            raise ValueError(f"{context}.Ivm must be a base64 string")
        return True
    if variant == "ContractCall":
        if not isinstance(body, dict):
            raise ValueError(f"{context}.ContractCall must be an object")
        return True
    if variant == "Instructions":
        if not isinstance(body, list):
            raise ValueError(f"{context}.Instructions must be an array")
        return False
    if not isinstance(body, list) or not body:
        raise ValueError(f"{context}.Batch must be a non-empty array")
    requires_gas_limit = False
    for index, item in enumerate(body):
        item_context = f"{context}.Batch[{index}]"
        if not isinstance(item, dict) or len(item) != 1:
            raise ValueError(f"{item_context} must contain exactly one variant")
        item_variant = next(iter(item))
        if item_variant == "ContractCall":
            if not isinstance(item[item_variant], dict):
                raise ValueError(f"{item_context}.ContractCall must be an object")
            requires_gas_limit = True
        elif item_variant != "Instruction":
            raise ValueError(f"{item_context} has unknown variant {item_variant!r}")
    return requires_gas_limit


def validate_payload_descriptors(path: Path) -> None:
    document = parse_json_strict(path)
    if not isinstance(document, list):
        raise ValueError(f"payload descriptor at {path} must be an array")
    seen_names: set[str] = set()
    for index, entry in enumerate(document):
        context = f"{path} fixture[{index}]"
        if not isinstance(entry, dict):
            raise ValueError(f"{context} must be an object")
        require_exact_fields(entry, PAYLOAD_ENTRY_FIELDS, context)
        name = entry["name"]
        if not isinstance(name, str) or not name:
            raise ValueError(f"{context}.name must be a non-empty string")
        if name in seen_names:
            raise ValueError(f"duplicate fixture name {name!r} in {path}")
        seen_names.add(name)
        payload = entry["payload"]
        if not isinstance(payload, dict):
            raise ValueError(f"{context}.payload must be an object")
        require_exact_fields(payload, PAYLOAD_FIELDS, f"{context}.payload")
        validate_admission_intent(
            payload["admission_intent"], f"{context}.payload.admission_intent"
        )
        gas_limit = validate_fee_payment(
            payload["fee_payment"], f"{context}.payload.fee_payment"
        )
        requires_gas_limit = executable_requires_gas_limit(
            payload["executable"], f"{context}.payload.executable"
        )
        if requires_gas_limit and gas_limit is None:
            raise ValueError(
                f"{context}.payload.fee_payment.value.gas_limit must be positive "
                "for Ivm, ContractCall, or a Batch containing ContractCall"
            )
        if not isinstance(payload["metadata"], dict):
            raise ValueError(f"{context}.payload.metadata must be an object")
        for field in (
            "authority",
            "creation_time_ms",
            "network_id",
            "nonce",
            "time_to_live_ms",
        ):
            if entry[field] != payload[field]:
                raise ValueError(f"{context}.{field} does not match payload.{field}")


def fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def validate_canonical_frames(manifest_path: Path) -> None:
    document = parse_json_strict(manifest_path)
    fixtures = document.get("fixtures") if isinstance(document, dict) else None
    if not isinstance(fixtures, list):
        raise ValueError(f"invalid fixture manifest: {manifest_path}")
    for entry in fixtures:
        if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
            raise ValueError(f"invalid fixture entry in {manifest_path}")
        name = entry["name"]
        try:
            payload_frame = base64.b64decode(entry["payload_base64"], validate=True)
            signed_frame = base64.b64decode(entry["signed_base64"], validate=True)
        except (KeyError, TypeError, ValueError) as exc:
            raise ValueError(f"{name}: invalid fixture base64") from exc
        if base64.b64encode(payload_frame).decode("ascii") != entry["payload_base64"]:
            raise ValueError(f"{name}: non-canonical payload base64")
        if base64.b64encode(signed_frame).decode("ascii") != entry["signed_base64"]:
            raise ValueError(f"{name}: non-canonical signed base64")
        payload_bare = decode_canonical_norito_frame(
            payload_frame,
            f"{name}.payload_base64",
            expected_schema=TRANSACTION_PAYLOAD_SCHEMA,
        )
        signed_bare = decode_canonical_norito_frame(
            signed_frame,
            f"{name}.signed_base64",
            expected_schema=SIGNED_TRANSACTION_SCHEMA,
        )
        if iroha_hash_hex(payload_frame) != entry.get("payload_hash"):
            raise ValueError(f"{name}: payload_hash does not authenticate the framed bytes")
        if signed_transaction_payload(signed_bare) != payload_bare:
            raise ValueError(f"{name}: signed transaction does not contain its payload")
        if signed_transaction_entrypoint_hash_hex(signed_bare) != entry.get("signed_hash"):
            raise ValueError(f"{name}: signed_hash does not match compact External semantics")


def compare(
    source: Path, target: Path
) -> Tuple[List[Path], List[Path], List[Tuple[Path, Path]]]:
    for root in (source, target):
        if not root.is_dir():
            raise FileNotFoundError(f"missing directory: {root}")

    source_map = {}
    target_map = {}
    for relative in MANAGED_FIXTURES:
        source_path = source / relative
        if not source_path.is_file():
            raise FileNotFoundError(f"missing canonical fixture: {source_path}")
        source_map[relative] = source_path
        target_path = target / relative
        if target_path.is_file():
            target_map[relative] = target_path

    validate_payload_descriptors(source / "transaction_payloads.json")
    validate_canonical_frames(source / "transaction_fixtures.manifest.json")

    missing = sorted(rel for rel in source_map if rel not in target_map)
    extra = sorted(
        path.relative_to(target)
        for path in target.rglob("*.norito")
        if path.is_file()
    )

    diffs: List[Tuple[Path, Path]] = []
    for rel, src_path in source_map.items():
        tgt_path = target_map.get(rel)
        if tgt_path is None:
            continue
        if fingerprint(src_path) != fingerprint(tgt_path):
            diffs.append((src_path, tgt_path))
    if not missing and not diffs:
        validate_payload_descriptors(target / "transaction_payloads.json")
        validate_canonical_frames(target / "transaction_fixtures.manifest.json")
    return missing, extra, diffs


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check Python Norito RPC descriptor parity"
    )
    parser.add_argument(
        "--source",
        type=Path,
        default=DEFAULT_SOURCE,
        help=f"Canonical fixture directory (default: {DEFAULT_SOURCE})",
    )
    parser.add_argument(
        "--target",
        type=Path,
        default=DEFAULT_TARGET,
        help=f"Python fixture directory (default: {DEFAULT_TARGET})",
    )
    parser.add_argument(
        "--quiet", action="store_true", help="Suppress success output"
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    try:
        missing, extra, diffs = compare(args.source, args.target)
    except (FileNotFoundError, ValueError) as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    has_error = False
    if missing:
        has_error = True
        print("[error] missing files in target:")
        for rel in missing:
            print(f"    {rel}")
    if extra:
        has_error = True
        print("[error] unexpected files in target:")
        for rel in extra:
            print(f"    {rel}")
    if diffs:
        has_error = True
        print("[error] content mismatches:")
        for src, tgt in diffs:
            rel = tgt.relative_to(args.target)
            print(f"    {rel} (source={src}, target={tgt})")

    if has_error:
        return 1

    if not args.quiet:
        print(f"[ok] Norito RPC descriptors match between {args.source} and {args.target}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
