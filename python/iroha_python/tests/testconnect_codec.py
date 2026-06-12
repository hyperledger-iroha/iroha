from __future__ import annotations

import pytest

import iroha_python._native as native_loader
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


def test_connect_sign_result_ok_normalizes_exact_ed25519_algorithm() -> None:
    payload = connect.ConnectSignResultOkPayload(
        signature=bytes([0x11]) * 64,
        algorithm="Ed25519",
    )

    assert payload.algorithm == "ed25519"
    assert payload.to_wire_dict()["signature"]["algorithm"] == "ed25519"


@pytest.mark.parametrize(
    "algorithm",
    [
        "secp256k1",
        "",
        " ",
        " Ed25519",
        "Ed25519 ",
        "\tEd25519",
        "Ed25519\n",
        "ed\t25519",
        "\u00a0Ed25519",
        "Ed25519\u00a0",
        "ed\u200b25519",
        "\u0435d25519",
        "ed\uff0d25519",
    ],
)
def test_connect_sign_result_ok_rejects_confusable_algorithms(algorithm: str) -> None:
    with pytest.raises(ValueError, match="unsupported wallet signature algorithm"):
        connect.ConnectSignResultOkPayload(
            signature=bytes([0x11]) * 64,
            algorithm=algorithm,
        )


@pytest.mark.parametrize(
    "algorithm",
    [
        "",
        " ",
        " Ed25519",
        "Ed25519 ",
        "\tEd25519",
        "Ed25519\n",
        "\u00a0Ed25519",
        "Ed25519\u00a0",
    ],
)
def test_connect_sign_result_ok_from_dict_rejects_padded_algorithm(
    algorithm: str,
) -> None:
    with pytest.raises(ValueError, match="unsupported wallet signature algorithm"):
        connect.ConnectSignResultOkPayload.from_dict(
            {
                "signature": {
                    "algorithm": algorithm,
                    "signature": bytes([0x11]) * 64,
                }
            }
        )


@pytest.mark.parametrize(
    "algorithm",
    [
        "secp256k1",
        "",
        " ",
        " Ed25519",
        "Ed25519 ",
        "\tEd25519",
        "Ed25519\n",
        "ed\t25519",
        "\u00a0Ed25519",
        "Ed25519\u00a0",
        "ed\u200b25519",
        "\u0435d25519",
        "ed\uff0d25519",
    ],
)
def test_connect_control_approve_rejects_confusable_algorithms(algorithm: str) -> None:
    with pytest.raises(ValueError, match="unsupported wallet signature algorithm"):
        connect.ConnectControlApprove(
            wallet_public_key=bytes([0x22]) * 32,
            account_id="account-i105",
            signature=bytes([0x33]) * 64,
            algorithm=algorithm,
        )


@pytest.mark.parametrize(
    "algorithm",
    [
        "",
        " ",
        " Ed25519",
        "Ed25519 ",
        "\tEd25519",
        "Ed25519\n",
        "\u00a0Ed25519",
        "Ed25519\u00a0",
    ],
)
def test_connect_control_approve_from_dict_rejects_padded_algorithm(
    algorithm: str,
) -> None:
    with pytest.raises(ValueError, match="unsupported wallet signature algorithm"):
        connect.ConnectControlApprove.from_dict(
            {
                "wallet_public_key": bytes([0x22]) * 32,
                "account_id": "account-i105",
                "signature": bytes([0x33]) * 64,
                "algorithm": algorithm,
            }
        )


def test_native_loader_accepts_current_python_framework(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path,
) -> None:
    candidate = tmp_path / "_crypto.abi3.so"
    candidate.write_bytes(b"")
    current = (
        f"{native_loader.sys.version_info.major}."
        f"{native_loader.sys.version_info.minor}"
    )

    monkeypatch.setattr(
        native_loader,
        "_linked_python_framework_versions",
        lambda path: (current,),
    )

    native_loader._assert_extension_compatible(candidate)


def test_native_loader_rejects_wrong_python_framework(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path,
) -> None:
    candidate = tmp_path / "_crypto.abi3.so"
    candidate.write_bytes(b"")
    current = (
        f"{native_loader.sys.version_info.major}."
        f"{native_loader.sys.version_info.minor}"
    )
    wrong = "3.14" if current != "3.14" else "3.13"

    monkeypatch.setattr(
        native_loader,
        "_linked_python_framework_versions",
        lambda path: (wrong,),
    )

    with pytest.raises(RuntimeError, match="links Python"):
        native_loader._assert_extension_compatible(candidate)
