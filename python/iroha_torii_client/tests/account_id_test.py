"""First-release Rust-owned key admission and canonical-I105 AccountId tests."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest

# Load the transport adapter without unrelated HTTP imports; identity operations
# still require the real packaged native owner, with no codec substitute.
MODULE_PATH = Path(__file__).resolve().parents[1] / "_account_id.py"
MODULE_SPEC = importlib.util.spec_from_file_location(
    "iroha_torii_client_account_id_test_module", MODULE_PATH
)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
ACCOUNT_ID = importlib.util.module_from_spec(MODULE_SPEC)
sys.modules[MODULE_SPEC.name] = ACCOUNT_ID
MODULE_SPEC.loader.exec_module(ACCOUNT_ID)

ED25519_RFC8032_PUBLIC_KEY_1 = bytes.fromhex(
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
)
ED25519_RFC8032_PUBLIC_KEY_2 = bytes.fromhex(
    "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
)
VALID_SINGLE_CANONICAL = b"\x02\x00\x01\x20" + ED25519_RFC8032_PUBLIC_KEY_1
VALID_SINGLE_I105 = (
    "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP"
)
CHECKSUM_VALID_ZERO_KEY_I105 = (
    "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"
)


def _member(public_key: bytes, weight: int) -> bytes:
    return (
        b"\x01"
        + weight.to_bytes(2, byteorder="big")
        + len(public_key).to_bytes(2, byteorder="big")
        + public_key
    )


def _single_controller(curve: int, public_key: bytes) -> bytes:
    if len(public_key) <= 0xFF:
        controller = bytes((0, curve, len(public_key))) + public_key
    else:
        controller = bytes((2, curve)) + len(public_key).to_bytes(2, "big") + public_key
    return b"\x02" + controller


SECP256K1_GENERATOR = bytes.fromhex(
    "0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798"
)
SM2_GENERATOR = bytes.fromhex(
    "04"
    "32C4AE2C1F1981195F9904466A39C9948FE30BBFF2660BE1715A4589334C74C7"
    "BC3736A2F4F6779C59BDCEE36B692153D0A9877CC62A474002DF32E52139F0A0"
)
SM2_DISTID = b"1234567812345678"
SM2_PAYLOAD = len(SM2_DISTID).to_bytes(2, "big") + SM2_DISTID + SM2_GENERATOR


def _fixture_i105(raw: bytes) -> str:
    # Only test data construction: admit no identity and bypass no production guard.
    # This independently computes a checksum-valid envelope so parse negatives test
    # key admission rather than accidentally failing the checksum first.
    value = int.from_bytes(raw, "big")
    digits = []
    while value:
        value, digit = divmod(value, 105)
        digits.append(digit)
    digits.reverse()
    values = []
    bits = accumulator = 0
    for byte in raw:
        accumulator = (accumulator << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            values.append((accumulator >> bits) & 31)
    if bits:
        values.append((accumulator << (5 - bits)) & 31)
    hrp = "snx"
    expanded = [ord(c) >> 5 for c in hrp] + [0] + [ord(c) & 31 for c in hrp]
    checksum = 1
    for value in expanded + values + [0] * 6:
        top = checksum >> 25
        checksum = ((checksum & 0x1FFFFFF) << 5) ^ value
        for bit, generator in enumerate((0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)):
            if (top >> bit) & 1:
                checksum ^= generator
    checksum ^= 0x2BC830A3
    return "sora" + "".join(ACCOUNT_ID.I105_ALPHABET[digit] for digit in digits + [(checksum >> (5 * (5 - i))) & 31 for i in range(6)])


def test_canonical_i105_parser_accepts_valid_single_key_account() -> None:
    assert ACCOUNT_ID.encode_i105_account_id(VALID_SINGLE_CANONICAL, 0x02F1) == VALID_SINGLE_I105
    assert (
        ACCOUNT_ID.decode_canonical_i105_account_id(
            VALID_SINGLE_I105, expected_discriminant=0x02F1
        )
        == VALID_SINGLE_CANONICAL
    )


def test_canonical_i105_parser_accepts_valid_multisig_account() -> None:
    canonical = (
        b"\x0a"  # V1, multisig class, normalization V1, no extension
        b"\x01"  # multisig controller tag
        b"\x01"  # multisig policy version
        b"\x00\x02"  # threshold
        b"\x00\x02"  # member count
        + _member(ED25519_RFC8032_PUBLIC_KEY_2, 2)
        + _member(ED25519_RFC8032_PUBLIC_KEY_1, 1)
    )
    encoded = ACCOUNT_ID.encode_i105_account_id(canonical, 0x0171)

    assert ACCOUNT_ID.decode_canonical_i105_account_id(encoded) == canonical


def test_canonical_i105_parser_accepts_extended_mldsa_single_key() -> None:
    public_key = bytes([0xA5]) * 1_952
    canonical = (
        b"\x02\x02\x02"
        + len(public_key).to_bytes(2, byteorder="big")
        + public_key
    )
    encoded = ACCOUNT_ID.encode_i105_account_id(canonical, 0)

    assert ACCOUNT_ID.decode_canonical_i105_account_id(encoded) == canonical


@pytest.mark.parametrize("item", json.loads((Path(__file__).resolve().parents[3] / "fixtures/account/multisig_wire_v1.json").read_text())["positive"][:11], ids=lambda item: item["name"])
def test_native_parser_covers_the_closed_release_curve_inventory(item) -> None:
    canonical = bytes.fromhex(item["canonical_address_hex"])
    curve = canonical[2]
    public_key = canonical[4 if canonical[1] == 0 else 5:]
    assert _single_controller(curve, public_key) == canonical
    encoded = ACCOUNT_ID.encode_i105_account_id(canonical, 0x02F1)
    assert ACCOUNT_ID.decode_canonical_i105_account_id(encoded) == canonical


def test_canonical_i105_parser_rejects_noncanonical_sentinel_rerender() -> None:
    noncanonical = "n753" + VALID_SINGLE_I105.removeprefix("sora")

    with pytest.raises(ValueError, match="unsupported account address format"):
        ACCOUNT_ID.decode_canonical_i105_account_id(noncanonical)


def test_extended_controller_tag_rejects_a_compact_key_length() -> None:
    canonical = (
        b"\x02\x02\x01\x00\x20" + ED25519_RFC8032_PUBLIC_KEY_1
    )
    encoded = _fixture_i105(canonical)
    with pytest.raises(ValueError):
        ACCOUNT_ID.encode_i105_account_id(canonical, 0x02F1)

    with pytest.raises(ValueError, match="invalid length"):
        ACCOUNT_ID.decode_canonical_i105_account_id(encoded)


def test_checksum_valid_zero_key_literal_is_not_an_account_id() -> None:
    malformed = b"\x02\x00\x01\x20" + bytes(32)
    assert (
        _fixture_i105(malformed)
        == CHECKSUM_VALID_ZERO_KEY_I105
    )

    with pytest.raises(ValueError, match="invalid public key"):
        ACCOUNT_ID.encode_i105_account_id(malformed, 0x02F1)
    with pytest.raises(ValueError, match="invalid public key"):
        ACCOUNT_ID.decode_canonical_i105_account_id(CHECKSUM_VALID_ZERO_KEY_I105)


def test_parser_matches_shared_ed25519_admission_vectors() -> None:
    fixture_path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "crypto"
        / "ed25519_public_key_admission_v1.json"
    )
    vectors = json.loads(fixture_path.read_text(encoding="utf-8"))["vectors"]

    for vector in vectors:
        literal = vector.get("single_i105") or vector["malformed_i105"]
        if vector["valid"]:
            assert ACCOUNT_ID.decode_canonical_i105_account_id(literal) == bytes.fromhex(
                vector["single_canonical_hex"]
            )
        else:
            with pytest.raises(ValueError):
                ACCOUNT_ID.decode_canonical_i105_account_id(literal)


@pytest.mark.parametrize(
    "canonical, message",
    [
        (
            b"\x0a\x00\x01\x20" + ED25519_RFC8032_PUBLIC_KEY_1,
            "unsupported account address format",
        ),
        (VALID_SINGLE_CANONICAL + b"\x00", "trailing"),
        (
            b"\x0a\x01\x01\x00\x00\x00\x01"
            + _member(ED25519_RFC8032_PUBLIC_KEY_1, 1),
            "threshold",
        ),
        (
            b"\x0a\x01\x01\x00\x02\x00\x01"
            + _member(ED25519_RFC8032_PUBLIC_KEY_1, 1),
            "exceeds",
        ),
        (
            b"\x0a\x01\x01\x00\x02\x00\x02"
            + _member(ED25519_RFC8032_PUBLIC_KEY_1, 1)
            + _member(ED25519_RFC8032_PUBLIC_KEY_2, 1),
            "canonical key order",
        ),
        (
            b"\x0a\x01\x01\x00\x01\x00\x02"
            + _member(ED25519_RFC8032_PUBLIC_KEY_1, 1)
            + _member(ED25519_RFC8032_PUBLIC_KEY_1, 1),
            "duplicate",
        ),
        (
            b"\x0a\x01\x01\x00\x01\x00\x01"
            + _member(ED25519_RFC8032_PUBLIC_KEY_1, 0),
            "weight",
        ),
        (
            b"\x0a\x01\x02\x00\x01\x00\x01"
            + _member(ED25519_RFC8032_PUBLIC_KEY_1, 1),
            "version",
        ),
    ],
)
def test_controller_invariants_reject_checksum_valid_malformed_literals(
    canonical: bytes, message: str
) -> None:
    encoded = _fixture_i105(canonical)
    with pytest.raises(ValueError, match=message):
        ACCOUNT_ID.encode_i105_account_id(canonical, 0x02F1)
    with pytest.raises(ValueError, match=message):
        ACCOUNT_ID.decode_canonical_i105_account_id(encoded)
