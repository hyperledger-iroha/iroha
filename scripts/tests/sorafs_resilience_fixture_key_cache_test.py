"""Parity and isolation controls for deterministic fixture public-key reuse."""

from __future__ import annotations

from pathlib import Path
import sys
from unittest import mock

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_resilience_test_support as fixture


@pytest.fixture(autouse=True)
def isolated_public_keys():
    """Keep cache observations local to each control."""
    fixture.public_key_from_seed.cache_clear()
    yield
    fixture.public_key_from_seed.cache_clear()


def test_public_keys_match_uncached_ed25519_and_distinguish_complete_seeds():
    seeds = (bytes(32), bytes(31) + b"\x01")
    keys = [fixture.public_key_from_seed(seed) for seed in seeds]
    assert all(isinstance(key, bytes) and len(key) == 32 for key in keys)
    assert keys[0] != keys[1]
    assert keys == [fixture.public_key_from_seed.__wrapped__(seed) for seed in seeds]
    assert [fixture.public_key_from_seed(seed) for seed in seeds] == keys


def test_eviction_recomputes_the_identical_public_key_with_a_bounded_memo():
    seed = bytes(32)
    with mock.patch.object(
        fixture.RELEASE_CRYPTO, "_ed_scalar_multiply",
        wraps=fixture.RELEASE_CRYPTO._ed_scalar_multiply,
    ) as multiply:
        expected = fixture.public_key_from_seed(seed)
        assert fixture.public_key_from_seed(seed) == expected
        assert multiply.call_count == 1
        for index in range(1, 17):
            fixture.public_key_from_seed(bytes([index]) * 32)
        assert fixture.public_key_from_seed.cache_info().currsize == 16
        before = multiply.call_count
        assert fixture.public_key_from_seed(seed) == expected
        assert multiply.call_count == before + 1
        assert fixture.public_key_from_seed.cache_info().currsize == 16


def test_cached_key_still_signs_and_verifies_each_real_message():
    seed = fixture.DEFAULT_SIGNING_SEED
    key = fixture.public_key_from_seed(seed)
    messages = (b"first authenticated fixture", b"second authenticated fixture")
    signatures = [fixture.sign(seed, message) for message in messages]
    assert signatures[0] != signatures[1]
    for message, signature in zip(messages, signatures):
        assert fixture.RELEASE_CRYPTO.verify_ed25519(key, signature, message)
        assert not fixture.RELEASE_CRYPTO.verify_ed25519(key, signature, message + b"changed")
