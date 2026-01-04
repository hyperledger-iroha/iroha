#!/usr/bin/env python3
"""
Validate that the canonical SM2 fixture matches the cross-SDK expectations.

The roadmap (task SM-3c) requires that Rust, Python, and JavaScript SDKs share
the same deterministic SM2 signing vectors. This script confirms that the
canonical `fixtures/sm/sm2_fixture.json` file still carries the agreed values.
It is intended to run in CI to guard against accidental drift.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Dict, Iterable, Tuple

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FIXTURE_PATH = REPO_ROOT / "fixtures" / "sm" / "sm2_fixture.json"

# Expected values for the top-level fields.
_EXPECTED_TOP_LEVEL: Dict[str, str] = {
    "distid": "1234567812345678",
    "seed_hex": "1111111111111111111111111111111111111111111111111111111111111111",
    "message_hex": "69726F686120736D2073646B2066697874757265",
    "private_key_hex": "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569",
    "public_key_sec1_hex": (
        "04361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69"
        "BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F"
    ),
    "public_key_multihash": (
        "86264104361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4"
        "C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F"
    ),
    "public_key_prefixed": (
        "sm2:86264104361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D"
        "99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F"
    ),
    "za": "E54EDEDE2A2FCC1C9DF868C56F8A2DD8C562F1AD3C78DC11DD7D91BB6F0EBD46",
    "signature": (
        "1877845D5FFE0305946EEA3046D0279BE886B866EF620B7325413602CAD17C7FF72EBF26C29E77AA"
        "AB2226EDFBEE2D6D6ABC0D6C9B2C9A2248E2BD9324A12268"
    ),
    "r": "1877845D5FFE0305946EEA3046D0279BE886B866EF620B7325413602CAD17C7F",
    "s": "F72EBF26C29E77AAAB2226EDFBEE2D6D6ABC0D6C9B2C9A2248E2BD9324A12268",
}

# Expected subset for each structured vector entry.
_EXPECTED_VECTORS: Dict[str, Dict[str, str]] = {
    "sm2-fixture-default-v1": {
        "distid": _EXPECTED_TOP_LEVEL["distid"],
        "seed_hex": _EXPECTED_TOP_LEVEL["seed_hex"],
        "message_hex": _EXPECTED_TOP_LEVEL["message_hex"],
        "private_key_hex": _EXPECTED_TOP_LEVEL["private_key_hex"],
        "public_key_sec1_hex": _EXPECTED_TOP_LEVEL["public_key_sec1_hex"],
        "signature": _EXPECTED_TOP_LEVEL["signature"],
    },
    "sm2-rust-sdk-fixture-v1": {
        "distid": "iroha-sdk-sm2-fixture",
        "seed_hex": (
            "69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265"
        ),
        "message_hex": "527573742053444B20534D32207369676E696E672066697874757265207631",
        "private_key_hex": "E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA",
        "signature": (
            "4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76299CFF374026D9E0"
            "C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5"
        ),
    },
    "gm-t-0003-annex-d-example1": {
        "distid": "ALICE123@YAHOO.COM",
        "message_hex": "6D65737361676520646967657374",
        "signature": (
            "40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D16FC6DAC32C5D5CF1"
            "0C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7"
        ),
        "r": "40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1",
        "s": "6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7",
    },
}


def _uppercase_hex(value: str) -> str:
    """Return `value` uppercased when it looks like a hexadecimal string."""
    if all(ch in "0123456789abcdefABCDEF" for ch in value):
        return value.upper()
    return value


def _compare(expected: str, actual: str) -> bool:
    return _uppercase_hex(expected) == _uppercase_hex(actual)


def _validate_top_level(data: Dict[str, str]) -> Tuple[bool, str]:
    for key, expected in _EXPECTED_TOP_LEVEL.items():
        actual = data.get(key)
        if actual is None:
            return False, f"missing top-level key '{key}'"
        if not _compare(expected, actual):
            return (
                False,
                f"top-level '{key}' mismatch (expected {_uppercase_hex(expected)}, got {actual})",
            )
    return True, ""


def _validate_vectors(data: Dict[str, object]) -> Tuple[bool, str]:
    vectors = data.get("vectors")
    if not isinstance(vectors, list):
        return False, "fixture 'vectors' field missing or not a list"
    by_case = {entry.get("case_id"): entry for entry in vectors if isinstance(entry, dict)}
    for case_id, expectations in _EXPECTED_VECTORS.items():
        entry = by_case.get(case_id)
        if entry is None:
            return False, f"missing vector '{case_id}'"
        for key, expected in expectations.items():
            actual = entry.get(key)
            if actual is None:
                return False, f"vector '{case_id}' missing key '{key}'"
            if not _compare(expected, actual):
                return (
                    False,
                    f"vector '{case_id}' field '{key}' mismatch "
                    f"(expected {_uppercase_hex(expected)}, got {actual})",
                )
    return True, ""


def validate(path: Path) -> Tuple[bool, str]:
    if not path.exists():
        return False, f"fixture not found at {path}"
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive
        return False, f"failed to parse JSON: {exc}"
    if not isinstance(data, dict):
        return False, "fixture root must be an object"
    ok, reason = _validate_top_level(data)
    if not ok:
        return ok, reason
    ok, reason = _validate_vectors(data)
    if not ok:
        return ok, reason
    return True, ""


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify canonical SM2 fixtures used by SDKs."
    )
    parser.add_argument(
        "--fixture",
        type=Path,
        default=DEFAULT_FIXTURE_PATH,
        help=f"Path to sm2_fixture.json (default: {DEFAULT_FIXTURE_PATH})",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress success output.",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    ok, reason = validate(args.fixture)
    if not ok:
        print(f"[error] {reason}", file=sys.stderr)
        return 1
    if not args.quiet:
        print(f"[ok] {args.fixture} matches the expected SM2 SDK vectors")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
