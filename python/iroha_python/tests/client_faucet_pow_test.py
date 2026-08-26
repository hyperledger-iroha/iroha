from __future__ import annotations

import hashlib

import pytest

from iroha_python import NetworkId, ToriiClient
from iroha_python.address import AccountAddress

NETWORK_ID = NetworkId.from_bytes(bytes([0xA5]) * 32)


def _crc16(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return crc


def _marked_network_id(network_id: NetworkId = NETWORK_ID) -> str:
    body = network_id.literal.upper()
    return f"hash:{body}#{_crc16(f'hash:{body}'.encode('ascii')):04X}"


def _faucet_account_id(chain_discriminant: int = 369) -> str:
    return AccountAddress.from_account(
        domain="default",
        public_key=bytes(range(32)),
    ).to_i105(chain_discriminant)


def _account_faucet_puzzle(network_id: NetworkId = NETWORK_ID) -> dict[str, object]:
    return {
        "algorithm": "scrypt-leading-zero-bits-v2",
        "network_id": _marked_network_id(network_id),
        "chain_discriminant": 369,
        "difficulty_bits": 1,
        "anchor_height": 7,
        "anchor_block_hash_hex": "00" * 32,
        "challenge_salt_hex": None,
        "scrypt_log_n": 1,
        "scrypt_r": 1,
        "scrypt_p": 1,
    }


def test_solve_account_faucet_pow_rejects_zero_difficulty_puzzle() -> None:
    puzzle = _account_faucet_puzzle()
    puzzle["difficulty_bits"] = 0

    with pytest.raises(ValueError, match="difficulty_bits.*greater than zero"):
        ToriiClient.solve_account_faucet_pow(
            _faucet_account_id(),
            puzzle,
        )


def test_solve_account_faucet_pow_binds_exact_network_bytes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    salts: list[bytes] = []

    def fake_scrypt(_password: bytes, *, salt: bytes, **_kwargs: object) -> bytes:
        salts.append(salt)
        return bytes(32)

    monkeypatch.setattr(hashlib, "scrypt", fake_scrypt)
    account_id = _faucet_account_id()
    foreign_network = NetworkId.from_bytes(bytes([0xA7]) * 32)

    first = ToriiClient.solve_account_faucet_pow(
        account_id,
        _account_faucet_puzzle(NETWORK_ID),
        max_nonce=1,
    )
    second = ToriiClient.solve_account_faucet_pow(
        account_id,
        _account_faucet_puzzle(foreign_network),
        max_nonce=1,
    )

    assert first == second == (7, "0000000000000000")
    assert salts[0] != salts[1]


@pytest.mark.parametrize(
    ("network_id", "match"),
    (
        (NETWORK_ID.literal, "canonical `hash:<uppercase hex>#<CRC16>`"),
        (_marked_network_id().lower(), "canonical `hash:<uppercase hex>#<CRC16>`"),
        (_marked_network_id()[:-1] + "0", "checksum mismatch"),
    ),
)
def test_solve_account_faucet_pow_rejects_noncanonical_network_json(
    network_id: str,
    match: str,
) -> None:
    puzzle = _account_faucet_puzzle()
    puzzle["network_id"] = network_id

    with pytest.raises(ValueError, match=match):
        ToriiClient.solve_account_faucet_pow(
            _faucet_account_id(),
            puzzle,
            max_nonce=1,
        )


def test_solve_account_faucet_pow_rejects_unmarked_network_hash() -> None:
    body = "A4" * 32
    puzzle = _account_faucet_puzzle()
    puzzle["network_id"] = (
        f"hash:{body}#{_crc16(f'hash:{body}'.encode('ascii')):04X}"
    )

    with pytest.raises(ValueError, match="marker bit"):
        ToriiClient.solve_account_faucet_pow(
            _faucet_account_id(),
            puzzle,
            max_nonce=1,
        )
