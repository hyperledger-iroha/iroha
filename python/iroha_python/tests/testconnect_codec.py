from __future__ import annotations

import pytest

import iroha_python.connect as connect


def test_connect_codec_fails_closed_when_native_unavailable(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(connect, "_CODEC_MODULE", None)

    def _raise_native_unavailable() -> object:
        raise RuntimeError("extension missing")

    monkeypatch.setattr(connect, "load_crypto_extension", _raise_native_unavailable)
    with pytest.raises(RuntimeError, match="native Connect codec unavailable"):
        connect._require_codec_module()


def test_connect_codec_caches_native_module(monkeypatch: pytest.MonkeyPatch) -> None:
    module = object()
    monkeypatch.setattr(connect, "_CODEC_MODULE", None)

    def _load_native() -> object:
        return module

    monkeypatch.setattr(connect, "load_crypto_extension", _load_native)
    assert connect._require_codec_module() is module
    assert connect._require_codec_module() is module
