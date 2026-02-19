from __future__ import annotations

import pickle

import pytest

import iroha_python.connect as connect
from iroha_python.connect_stub import ConnectCodecStub


def test_connect_codec_fails_closed_when_native_unavailable(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("IROHA_PYTHON_CONNECT_CODEC", raising=False)
    monkeypatch.delenv("IROHA_PYTHON_CONNECT_STUB", raising=False)
    monkeypatch.setattr(connect, "_CODEC_MODULE", None)
    monkeypatch.setattr(connect, "_STUB_NOTICE_EMITTED", False)

    def _raise_native_unavailable() -> object:
        raise RuntimeError("extension missing")

    monkeypatch.setattr(connect, "load_crypto_extension", _raise_native_unavailable)
    with pytest.raises(RuntimeError, match="native Connect codec unavailable"):
        connect._require_codec_module()


def test_connect_codec_supports_explicit_stub_override(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("IROHA_PYTHON_CONNECT_CODEC", "stub")
    monkeypatch.setattr(connect, "_CODEC_MODULE", None)
    monkeypatch.setattr(connect, "_STUB_NOTICE_EMITTED", False)

    def _raise_native_unavailable() -> object:
        raise RuntimeError("extension missing")

    monkeypatch.setattr(connect, "load_crypto_extension", _raise_native_unavailable)
    codec = connect._require_codec_module()
    assert isinstance(codec, ConnectCodecStub)


def test_connect_stub_roundtrip_preserves_binary_fields() -> None:
    codec = ConnectCodecStub()
    frame = {
        "sid": bytes(range(32)),
        "direction": "WalletToApp",
        "sequence": 7,
        "kind": {
            "type": "Control",
            "control_type": "Close",
            "fields": {
                "role": "Wallet",
                "code": 1000,
                "reason": "done",
                "retryable": False,
                "proof": bytes([0xAA, 0xBB, 0xCC]),
            },
        },
    }
    encoded = codec.encode_connect_frame(frame)
    decoded = codec.decode_connect_frame(encoded)
    assert decoded == frame


def test_connect_stub_rejects_pickle_payloads() -> None:
    codec = ConnectCodecStub()
    payload = pickle.dumps({"sid": b"abc"})
    with pytest.raises(ValueError, match="valid .*JSON"):
        codec.decode_connect_frame(payload)
