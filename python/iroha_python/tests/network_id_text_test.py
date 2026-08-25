"""Focused public-text contract tests for the native NetworkId binding."""

from __future__ import annotations

import pytest

from iroha_python import NetworkId


CANONICAL = "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"
GENERIC_HASH_LITERAL = (
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
)


def test_network_id_round_trips_exact_lowercase_hex() -> None:
    parsed = NetworkId.parse(CANONICAL)

    assert parsed.literal == CANONICAL
    assert str(parsed) == CANONICAL
    assert parsed.to_bytes() == bytes.fromhex(CANONICAL)
    assert NetworkId.from_bytes(parsed.to_bytes()) == parsed


@pytest.mark.parametrize(
    "value",
    [
        CANONICAL.upper(),
        CANONICAL[:-1] + "8",
        CANONICAL[:-1],
        "g" + CANONICAL[1:],
        GENERIC_HASH_LITERAL,
        f" {CANONICAL}",
        f"{CANONICAL} ",
    ],
)
def test_network_id_rejects_noncanonical_public_text(value: str) -> None:
    with pytest.raises(ValueError, match="64 lowercase hexadecimal characters"):
        NetworkId.parse(value)
