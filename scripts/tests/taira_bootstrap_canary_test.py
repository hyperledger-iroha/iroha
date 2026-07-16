"""Tests for scripts/taira_bootstrap_canary.py."""

from __future__ import annotations

import importlib.util
import io
import json
import sys
from pathlib import Path
from urllib import error

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "taira_bootstrap_canary.py"
SPEC = importlib.util.spec_from_file_location("taira_bootstrap_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

ONBOARDING_TOKEN = "0123456789abcdef0123456789ABCDEF"


def test_default_chain_id_targets_public_sumeragi_v2_taira() -> None:
    assert MODULE.DEFAULT_CHAIN_ID == "fc56984b-2be7-431d-840e-21514d1883f0"
    assert MODULE.derive_canary_uaid(" AABBCC ") == (
        "uaid:54b79979e70601aa2d9573b9c37778809a62ff3096ed05f940b964752f45cd69"
    )


def test_pipeline_status_requires_current_nested_shape() -> None:
    assert MODULE.transaction_status_kind({"status": {"kind": "Applied"}}) == "Applied"
    assert MODULE.transaction_status_kind({"status": "Applied"}) is None
    assert MODULE.transaction_status_kind({"summary": "Applied"}) is None


def current_faucet_response(account_id: str, asset_definition_id: str) -> dict:
    return {
        "account_id": account_id,
        "asset_definition_id": asset_definition_id,
        "asset_id": f"{asset_definition_id}#{account_id}",
        "amount": "1000",
        "tx_hash_hex": "cd" * 32,
        "status": "QUEUED",
    }


def test_faucet_requires_queued_receipt_and_pipeline_finality(monkeypatch) -> None:
    calls = []
    account_id = "sora-test-account"
    asset_definition_id = "gas-asset"

    def fake_http_json(method, url, payload=None):
        calls.append((method, url, payload))
        if url.endswith("/v1/accounts/faucet/puzzle"):
            return 200, {"difficulty_bits": 0}
        if url.endswith("/v1/accounts/faucet"):
            return 202, current_faucet_response(account_id, asset_definition_id)
        assert "/v1/pipeline/transactions/status?" in url
        assert "hash=" + "cd" * 32 in url
        assert "scope=global" in url
        return 200, {"status": {"kind": "Applied", "block_height": 7}}

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)
    result = MODULE.attempt_faucet(
        account_id,
        "https://taira.example",
        gas_asset_id=asset_definition_id,
        status_timeout_ms=1_000,
    )

    assert result["status"] == "claimed"
    assert result["response_status"] == 202
    assert MODULE.transaction_status_kind(result["final_status"]) == "Applied"
    assert len(calls) == 3


def test_faucet_rejects_retired_synchronous_receipt(monkeypatch) -> None:
    calls = []
    account_id = "sora-test-account"
    response = current_faucet_response(account_id, "gas-asset")
    response["status"] = "Applied"

    def fake_http_json(method, url, payload=None):
        calls.append((method, url, payload))
        if url.endswith("/v1/accounts/faucet/puzzle"):
            return 200, {"difficulty_bits": 0}
        return 200, response

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)
    result = MODULE.attempt_faucet(
        account_id,
        "https://taira.example",
        gas_asset_id="gas-asset",
    )

    assert result["status"] == "failed"
    assert result["response_status"] == 200
    assert len(calls) == 2


def test_faucet_rejects_short_queued_hash(monkeypatch) -> None:
    account_id = "sora-test-account"
    response = current_faucet_response(account_id, "gas-asset")
    response["tx_hash_hex"] = "cd"

    def fake_http_json(_method, url, _payload=None):
        if url.endswith("/v1/accounts/faucet/puzzle"):
            return 200, {"difficulty_bits": 0}
        return 202, response

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)
    result = MODULE.attempt_faucet(
        account_id,
        "https://taira.example",
        gas_asset_id="gas-asset",
    )

    assert result["status"] == "failed"
    assert "must encode 32 bytes" in result["error"]


def test_pipeline_status_rejects_noncanonical_http_status(monkeypatch) -> None:
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: (202, {"status": {"kind": "Applied"}}),
    )

    try:
        MODULE.wait_for_transaction_status(
            "https://taira.example",
            "ab" * 32,
            timeout_ms=1_000,
        )
    except RuntimeError as error:
        assert "status=202" in str(error)
    else:  # pragma: no cover
        raise AssertionError("noncanonical pipeline-status HTTP code was accepted")


def current_onboarding_response(public_key_hex: str, alias: str) -> dict:
    return {
        "account_id": "sora-test-account",
        "uaid": MODULE.derive_canary_uaid(public_key_hex),
        "tx_hash_hex": "ab" * 32,
        "status": "QUEUED",
        "lease": {
            "alias": alias,
            "dataspace": "universal",
            "domain": "wonderland.universal",
            "is_primary": True,
            "lease_status": "active",
            "expires_at_ms": 1000,
            "grace_expires_at_ms": 2000,
            "redemption_expires_at_ms": 3000,
            "auto_renew_enabled": False,
        },
    }


def test_onboarding_uses_current_universal_account_dto(monkeypatch) -> None:
    captured = {}
    public_key_hex = "AABBCC"
    alias = "canary@universal"

    def fake_http_json(method, url, payload=None, **kwargs):
        captured.update(method=method, url=url, payload=payload, **kwargs)
        return 202, current_onboarding_response(public_key_hex, alias)

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)
    result = MODULE.onboard_account(
        "https://taira.example",
        alias,
        public_key_hex,
        onboarding_token=ONBOARDING_TOKEN,
        permissions=["CanFoo", "", "CanFoo", "CanBar"],
    )

    assert result["status"] == "created"
    assert result["response_status"] == 202
    assert captured == {
        "method": "POST",
        "url": "https://taira.example/v1/accounts/onboard",
        "payload": {
            "alias": alias,
            "public_key_hex": public_key_hex,
            "uaid": MODULE.derive_canary_uaid(public_key_hex),
            "permissions": ["CanFoo", "CanBar"],
        },
        "headers": {MODULE.ACCOUNT_ONBOARDING_TOKEN_HEADER: ONBOARDING_TOKEN},
        "allow_redirects": False,
        "sensitive_value": ONBOARDING_TOKEN,
    }


def test_onboarding_rejects_retired_synchronous_response(monkeypatch) -> None:
    response = current_onboarding_response("aabbcc", "canary@universal")
    response["status"] = "Applied"
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: (200, response),
    )

    try:
        MODULE.onboard_account(
            "https://taira.example",
            "canary@universal",
            "aabbcc",
            onboarding_token=ONBOARDING_TOKEN,
        )
    except RuntimeError as error:
        assert "status=200" in str(error)
    else:  # pragma: no cover
        raise AssertionError("retired synchronous onboarding was accepted")


def test_onboarding_rejects_mismatched_uaid(monkeypatch) -> None:
    response = current_onboarding_response("aabbcc", "canary@universal")
    response["uaid"] = "uaid:" + "01" * 32
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: (202, response),
    )

    try:
        MODULE.onboard_account(
            "https://taira.example",
            "canary@universal",
            "aabbcc",
            onboarding_token=ONBOARDING_TOKEN,
        )
    except RuntimeError as error:
        assert "does not match" in str(error)
    else:  # pragma: no cover
        raise AssertionError("mismatched onboarding UAID was accepted")


@pytest.mark.parametrize(
    "token",
    ["", "T" * 31, "T" * 257, "T" * 31 + " ", "T" * 31 + "é"],
)
def test_onboarding_rejects_malformed_token_before_http(monkeypatch, token) -> None:
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: pytest.fail("malformed token reached HTTP dispatch"),
    )

    with pytest.raises(ValueError) as captured:
        MODULE.onboard_account(
            "https://taira.example",
            "canary@universal",
            "aabbcc",
            onboarding_token=token,
        )

    if token:
        assert token not in str(captured.value)
    assert captured.value.__cause__ is None
    assert captured.value.__context__ is None


def test_onboarding_token_file_is_exact_owner_only_regular_and_not_cached(
    tmp_path, monkeypatch
) -> None:
    token_file = tmp_path / "onboarding.token"
    token_file.write_bytes(ONBOARDING_TOKEN.encode("ascii"))
    token_file.chmod(0o600)

    assert MODULE.read_onboarding_token_file(token_file) == ONBOARDING_TOKEN
    replacement = "Z" * 32
    token_file.write_bytes(replacement.encode("ascii"))
    assert MODULE.read_onboarding_token_file(token_file) == replacement

    token_file.write_bytes((ONBOARDING_TOKEN + "\n").encode("ascii"))
    with pytest.raises(ValueError):
        MODULE.read_onboarding_token_file(token_file)

    token_file.write_bytes(b"T" * 31 + b"\xff")
    with pytest.raises(ValueError) as captured:
        MODULE.read_onboarding_token_file(token_file)
    assert captured.value.__cause__ is None
    assert captured.value.__context__ is None

    token_file.write_bytes(ONBOARDING_TOKEN.encode("ascii"))
    token_file.chmod(0o640)
    with pytest.raises(RuntimeError, match="group or other"):
        MODULE.read_onboarding_token_file(token_file)

    token_file.chmod(0o600)
    if hasattr(MODULE.os, "geteuid"):
        actual_uid = MODULE.os.geteuid()
        monkeypatch.setattr(MODULE.os, "geteuid", lambda: actual_uid + 1)
        with pytest.raises(RuntimeError, match="owned by the current user"):
            MODULE.read_onboarding_token_file(token_file)
        monkeypatch.setattr(MODULE.os, "geteuid", lambda: actual_uid)

    symlink = tmp_path / "onboarding-link.token"
    symlink.symlink_to(token_file)
    with pytest.raises(RuntimeError, match="non-symlink"):
        MODULE.read_onboarding_token_file(symlink)
    with pytest.raises(RuntimeError, match="regular"):
        MODULE.read_onboarding_token_file(tmp_path)


def test_onboarding_http_refuses_redirect_and_sends_one_header(monkeypatch) -> None:
    captured = {}

    class RedirectOpener:
        @staticmethod
        def open(req):
            captured["headers"] = [
                (key.lower(), value) for key, value in req.header_items()
            ]
            raise error.HTTPError(
                req.full_url,
                307,
                "Temporary Redirect",
                {"Location": "https://redirect.example/v1/accounts/onboard"},
                io.BytesIO(f"server echoed {ONBOARDING_TOKEN}".encode()),
            )

    monkeypatch.setattr(
        MODULE.request,
        "build_opener",
        lambda *_handlers: RedirectOpener(),
    )

    status, body = MODULE._http_json(
        "POST",
        "https://taira.example/v1/accounts/onboard",
        {"alias": "canary@universal"},
        headers={
            MODULE.ACCOUNT_ONBOARDING_TOKEN_HEADER.lower(): "stale-duplicate",
            MODULE.ACCOUNT_ONBOARDING_TOKEN_HEADER: ONBOARDING_TOKEN,
        },
        allow_redirects=False,
        sensitive_value=ONBOARDING_TOKEN,
    )

    assert status == 307
    assert body == "<invalid JSON response>"
    onboarding_headers = [
        value
        for name, value in captured["headers"]
        if name == MODULE.ACCOUNT_ONBOARDING_TOKEN_HEADER.lower()
    ]
    assert onboarding_headers == [ONBOARDING_TOKEN]


def test_onboarding_redacts_server_echo_before_error(monkeypatch) -> None:
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: (
            307,
            {
                "message": f"server echoed {ONBOARDING_TOKEN}",
                "nested": [ONBOARDING_TOKEN],
                ONBOARDING_TOKEN: "echoed as an object key",
            },
        ),
    )

    with pytest.raises(RuntimeError) as captured:
        MODULE.onboard_account(
            "https://taira.example",
            "canary@universal",
            "aabbcc",
            onboarding_token=ONBOARDING_TOKEN,
        )

    assert ONBOARDING_TOKEN not in str(captured.value)
    assert "<redacted>" in str(captured.value)


def test_http_json_uses_first_release_api_without_version_header(monkeypatch) -> None:
    captured = {}

    class FakeResponse:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return False

        @staticmethod
        def read():
            return json.dumps({"ok": True}).encode()

    def fake_urlopen(req):
        captured["headers"] = {key.lower(): value for key, value in req.header_items()}
        return FakeResponse()

    monkeypatch.setattr(MODULE.request, "urlopen", fake_urlopen)
    status, body = MODULE._http_json("GET", "https://taira.example/status")

    assert status == 200
    assert body == {"ok": True}
    assert "x-iroha-api-version" not in captured["headers"]
