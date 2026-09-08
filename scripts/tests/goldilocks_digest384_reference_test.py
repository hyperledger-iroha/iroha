"""Adversarial checks for the independent final six-lane arithmetic reference."""

import importlib.util
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "check_goldilocks_digest384_reference.py"
SPEC = importlib.util.spec_from_file_location("goldilocks_digest384_reference", SCRIPT)
REFERENCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REFERENCE)


def test_reference_reproduces_complete_parameter_frame_and_digest_assets():
    result = REFERENCE.check_reference()
    assert result["frame_words"] == 58
    assert result["lane_word_index"] == 49
    assert len(set(result["digest_words"])) == 6


def test_reference_rejects_invalid_lanes_and_preserves_byte_boundaries():
    for lane in (-1, 6, 2**64):
        with pytest.raises(ValueError):
            REFERENCE.frame(lane)
        with pytest.raises(ValueError):
            REFERENCE.params(lane)
    variants = (b"", b"\0", b"123456", b"1234567", b"1234567\0")
    framed = [REFERENCE.field(12, value) for value in variants]
    assert len({tuple(words) for words in framed}) == len(variants)
    assert all(word < REFERENCE.P for words in framed for word in words)


def test_reference_detects_parameter_and_round_shape_substitutions(monkeypatch):
    original = REFERENCE.params

    def changed_lane_parameters(lane):
        state, rounds = original(lane)
        rounds[0][0] = (rounds[0][0] + 1) % REFERENCE.P
        return state, rounds

    with monkeypatch.context() as patch:
        patch.setattr(REFERENCE, "params", changed_lane_parameters)
        with pytest.raises(AssertionError):
            REFERENCE.check_reference()

    def shortened_rounds(lane):
        state, rounds = original(lane)
        return state, rounds[:-1]

    monkeypatch.setattr(REFERENCE, "params", shortened_rounds)
    with pytest.raises(AssertionError):
        REFERENCE.check_reference()
