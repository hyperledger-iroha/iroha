"""Tests for scripts/taira_faucet_canary.py."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "taira_faucet_canary.py"
SPEC = importlib.util.spec_from_file_location("taira_faucet_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_leading_zero_bits_counts_prefix() -> None:
    assert MODULE.leading_zero_bits(bytes.fromhex("000f")) == 12
    assert MODULE.leading_zero_bits(bytes.fromhex("80")) == 0


def test_build_challenge_matches_known_digest() -> None:
    challenge = MODULE.build_challenge(
        account_id="sorauロ1example",
        anchor_height=5,
        anchor_block_hash_hex="00" * 32,
        challenge_salt_hex=None,
    )
    assert challenge.hex() == "fc7d21d12e97804f7266be24199d25f4b4c6260779540e43fd2c13eb5f8118e3"


def test_solve_puzzle_returns_expected_nonce_for_easy_case() -> None:
    puzzle = {
        "difficulty_bits": 8,
        "anchor_height": 5,
        "anchor_block_hash_hex": "00" * 32,
        "challenge_salt_hex": None,
        "scrypt_log_n": 1,
        "scrypt_r": 1,
        "scrypt_p": 1,
    }
    body = MODULE.solve_puzzle("sorauロ1example", puzzle)
    assert body["account_id"] == "sorauロ1example"
    assert body["pow_anchor_height"] == 5
    assert body["pow_nonce_hex"] == "000000000000021a"
