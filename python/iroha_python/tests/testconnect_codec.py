from __future__ import annotations

import pytest

import iroha_python._native as native_loader
import iroha_python.connect as connect


class FakeToriiConnectClient:
    def __init__(self, *, response: object | None = None) -> None:
        self.calls: list[dict[str, object]] = []
        self.response = response

    def create_connect_session(self, payload: dict[str, object]) -> object:
        self.calls.append(dict(payload))
        if self.response is not None:
            return self.response
        sid = str(payload["sid"])
        return connect.ConnectSessionInfo(
            sid=sid,
            app_uri=f"iroha://connect/app?sid={sid}",
            app_token="app-token",
            wallet_token="wallet-token",
            management_token="management-token",
            relay_token="relay-token",
        )


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


def test_generate_connect_sid_matches_deterministic_vector() -> None:
    result = connect.generate_connect_sid(
        chain_id="chain-A",
        app_public_key=bytes(range(32)),
        nonce=bytes(range(0xA0, 0xB0)),
    )

    assert result.sid_bytes.hex() == (
        "e247cb440f25c9a1dbfd7a6272a59b0d0a9f30a9b2a7faad5ad32b4268068b81"
    )
    assert result.sid_base64url == "4kfLRA8lyaHb_XpicqWbDQqfMKmyp_qtWtMrQmgGi4E"
    assert result.nonce == bytes(range(0xA0, 0xB0))


@pytest.mark.parametrize(
    ("field", "kwargs", "match"),
    [
        (
            "app_public_key",
            {"app_public_key": bytes(31), "nonce": bytes(16)},
            "app_public_key must be 32 bytes",
        ),
        (
            "nonce",
            {"app_public_key": bytes(32), "nonce": bytes(15)},
            "nonce must be 16 bytes",
        ),
        (
            "chain_id",
            {"chain_id": "", "app_public_key": bytes(32), "nonce": bytes(16)},
            "chain_id must not be empty",
        ),
    ],
)
def test_generate_connect_sid_rejects_malformed_inputs(
    field: str,
    kwargs: dict[str, object],
    match: str,
) -> None:
    payload = {"chain_id": "chain-A", "app_public_key": bytes(32), "nonce": bytes(16)}
    payload.update(kwargs)

    with pytest.raises((TypeError, ValueError), match=match):
        connect.generate_connect_sid(**payload)  # type: ignore[arg-type]

    assert field in payload


def test_create_connect_session_preview_builds_canonical_uris() -> None:
    key_pair = connect.ConnectKeyPair(
        private_key=bytes([0x44]) * 32,
        public_key=bytes(range(32)),
    )

    preview = connect.create_connect_session_preview(
        chain_id=" chain-A ",
        node=" torii.devnet.example ",
        nonce=bytes(range(0xA0, 0xB0)),
        app_key_pair=key_pair,
    )

    assert preview.chain_id == "chain-A"
    assert preview.node == "torii.devnet.example"
    assert preview.sid_base64url == "4kfLRA8lyaHb_XpicqWbDQqfMKmyp_qtWtMrQmgGi4E"
    assert preview.app_key_pair is key_pair
    assert preview.wallet_uri == (
        "iroha://connect?sid=4kfLRA8lyaHb_XpicqWbDQqfMKmyp_qtWtMrQmgGi4E"
        "&chain_id=chain-A&v=1&node=torii.devnet.example"
    )
    assert preview.app_uri == (
        "iroha://connect/app?sid=4kfLRA8lyaHb_XpicqWbDQqfMKmyp_qtWtMrQmgGi4E"
        "&chain_id=chain-A&v=1&node=torii.devnet.example"
    )


def test_bootstrap_connect_preview_session_registers_and_extracts_tokens() -> None:
    client = FakeToriiConnectClient()

    result = connect.bootstrap_connect_preview_session(
        client,
        chain_id="chain-A",
        node="torii.devnet.example",
        nonce=bytes(range(0xA0, 0xB0)),
        app_key_pair=connect.ConnectKeyPair(
            private_key=bytes([0x44]) * 32,
            public_key=bytes(range(32)),
        ),
    )

    assert client.calls == [
        {
            "sid": "4kfLRA8lyaHb_XpicqWbDQqfMKmyp_qtWtMrQmgGi4E",
            "node": "torii.devnet.example",
        }
    ]
    assert result.session is not None
    assert result.session.sid == result.preview.sid_base64url
    assert result.tokens == connect.ConnectPreviewTokens(
        wallet="wallet-token",
        app="app-token",
        management="management-token",
        relay="relay-token",
    )


def test_bootstrap_connect_preview_session_can_skip_registration() -> None:
    client = FakeToriiConnectClient()

    result = connect.bootstrap_connect_preview_session(
        client,
        chain_id="chain-A",
        register=False,
        nonce=bytes(range(0xA0, 0xB0)),
        app_key_pair=connect.ConnectKeyPair(
            private_key=bytes([0x44]) * 32,
            public_key=bytes(range(32)),
        ),
    )

    assert client.calls == []
    assert result.session is None
    assert result.tokens is None
    assert result.preview.sid_base64url == "4kfLRA8lyaHb_XpicqWbDQqfMKmyp_qtWtMrQmgGi4E"


def test_bootstrap_connect_preview_session_rejects_bad_options_before_registration() -> None:
    client = FakeToriiConnectClient()

    with pytest.raises(ValueError, match="unsupported session option"):
        connect.bootstrap_connect_preview_session(
            client,
            chain_id="chain-A",
            session_options={"ttl_ms": 1000},
            nonce=bytes(range(0xA0, 0xB0)),
            app_key_pair=connect.ConnectKeyPair(
                private_key=bytes([0x44]) * 32,
                public_key=bytes(range(32)),
            ),
        )

    assert client.calls == []


def test_bootstrap_connect_preview_session_rejects_missing_tokens() -> None:
    client = FakeToriiConnectClient(
        response={
            "app_token": "app-token",
            "management_token": "management-token",
            "relay_token": "relay-token",
        }
    )

    with pytest.raises(ValueError, match="wallet_token"):
        connect.bootstrap_connect_preview_session(
            client,
            chain_id="chain-A",
            nonce=bytes(range(0xA0, 0xB0)),
            app_key_pair=connect.ConnectKeyPair(
                private_key=bytes([0x44]) * 32,
                public_key=bytes(range(32)),
            ),
        )

    assert client.calls == [{"sid": "4kfLRA8lyaHb_XpicqWbDQqfMKmyp_qtWtMrQmgGi4E"}]


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
