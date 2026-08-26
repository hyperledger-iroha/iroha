from __future__ import annotations

import hashlib

import pytest

from iroha_python import NetworkId, ToriiClient
from iroha_python.address import AccountAddress

NETWORK_ID = NetworkId.from_bytes(bytes([0xA5]) * 32)


def _faucet_account_id(chain_discriminant: int = 753) -> str:
    return AccountAddress.from_account(
        public_key=bytes(range(32)),
    ).to_i105(chain_discriminant)


def _account_faucet_puzzle(network_id: NetworkId = NETWORK_ID) -> dict[str, object]:
    return {
        "algorithm": "scrypt-leading-zero-bits-v1",
        "network_id": network_id.literal,
        "chain_discriminant": 753,
        "difficulty_bits": 1,
        "anchor_height": 7,
        "anchor_block_hash_hex": "00" * 32,
        "challenge_salt_hex": None,
        "scrypt_log_n": 1,
        "scrypt_r": 1,
        "scrypt_p": 1,
        "max_anchor_age_blocks": 256,
    }


def test_solve_account_faucet_pow_rejects_zero_difficulty_puzzle() -> None:
    puzzle = _account_faucet_puzzle()
    puzzle["difficulty_bits"] = 0

    with pytest.raises(ValueError, match="difficulty_bits.*greater than zero"):
        ToriiClient.solve_account_faucet_pow(
            _faucet_account_id(),
            puzzle,
        )


def test_solve_account_faucet_pow_rejects_pre_release_algorithm_label() -> None:
    puzzle = _account_faucet_puzzle()
    puzzle["algorithm"] = "scrypt-leading-zero-bits-v2"

    with pytest.raises(ValueError, match="scrypt-leading-zero-bits-v1"):
        ToriiClient.solve_account_faucet_pow(
            _faucet_account_id(),
            puzzle,
        )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("scrypt_log_n", "13"),
        ("scrypt_log_n", 0),
        ("scrypt_log_n", 32),
        ("scrypt_r", True),
        ("scrypt_r", 0),
        ("scrypt_r", 262_145),
        ("scrypt_p", 0),
        ("scrypt_p", 17),
    ],
)
def test_solve_account_faucet_pow_rejects_unsafe_scrypt_parameters(
    field: str,
    value: object,
) -> None:
    puzzle = _account_faucet_puzzle()
    puzzle[field] = value

    with pytest.raises((TypeError, ValueError), match="scrypt"):
        ToriiClient.solve_account_faucet_pow(
            _faucet_account_id(),
            puzzle,
            max_nonce=1,
        )


@pytest.mark.parametrize("max_nonce", [False, 0, -1, 1.5, (1 << 64) + 1])
def test_solve_account_faucet_pow_rejects_invalid_nonce_budget(
    max_nonce: object,
) -> None:
    with pytest.raises(ValueError, match="max_nonce"):
        ToriiClient.solve_account_faucet_pow(
            _faucet_account_id(),
            _account_faucet_puzzle(),
            max_nonce=max_nonce,  # type: ignore[arg-type]
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
