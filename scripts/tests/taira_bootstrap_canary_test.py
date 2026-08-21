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


def test_default_network_id_targets_the_exact_public_taira_genesis() -> None:
    assert MODULE.DEFAULT_NETWORK_ID == (
        "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
    )


def test_default_alias_uses_canonical_dataspace_root() -> None:
    assert MODULE.build_alias(
        "canary",
        "ed0123456789ABCDEF",
        MODULE.DEFAULT_DOMAIN,
    ) == "canary0123456789abcdef@universal"


def test_build_alias_preserves_full_normalized_domain() -> None:
    assert MODULE.build_alias(
        "Taira-Rollout_Canary",
        "ed0123456789ABCDEF",
        " Sora.Universal ",
    ) == "tairarolloutcanary0123456789abcdef@sora.universal"


def test_build_alias_preserves_single_segment_dataspace_root() -> None:
    assert MODULE.build_alias(
        "canary",
        "ed0123456789ABCDEF",
        "Sora",
    ) == "canary0123456789abcdef@sora"


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
    monkeypatch.setattr(
        MODULE,
        "solve_puzzle",
        lambda account, _puzzle: {
            "account_id": account,
            "pow_anchor_height": 5,
            "pow_nonce_hex": "00" * 8,
        },
    )
    result = MODULE.attempt_faucet(
        account_id,
        "https://taira.example",
        faucet_asset_id=asset_definition_id,
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
    monkeypatch.setattr(
        MODULE,
        "solve_puzzle",
        lambda account, _puzzle: {
            "account_id": account,
            "pow_anchor_height": 5,
            "pow_nonce_hex": "00" * 8,
        },
    )
    result = MODULE.attempt_faucet(
        account_id,
        "https://taira.example",
        faucet_asset_id="gas-asset",
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
    monkeypatch.setattr(
        MODULE,
        "solve_puzzle",
        lambda account, _puzzle: {
            "account_id": account,
            "pow_anchor_height": 5,
            "pow_nonce_hex": "00" * 8,
        },
    )
    result = MODULE.attempt_faucet(
        account_id,
        "https://taira.example",
        faucet_asset_id="gas-asset",
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


def current_onboarding_receipt(request_payload: dict) -> dict:
    return {
        "body": {
            "version": 1,
            "request": dict(request_payload),
            "authority": "sora-onboarding-authority",
            "network_id": MODULE.DEFAULT_NETWORK_ID,
            "anchor": {"block_height": 1, "block_hash": "11" * 32},
            "resource": {"disposition": {"kind": "create"}},
            "acquisition": {"term_years": 1},
            "quote_guard": {"valid_until_ms": 9999999999999},
            "instructions": [],
            "owner_auto_renew_instruction": None,
            "valid_until_ms": 9999999999999,
        },
        "plan_hash": "22" * 32,
        "signature": "33" * 64,
    }


def current_onboarding_apply_response(account_id: str, alias: str) -> dict:
    return {
        "account_id": account_id,
        "alias": alias,
        "tx_hash_hex": "ab" * 32,
        "status": "Queued",
        "disposition": {"kind": "create"},
    }


def test_onboarding_plans_then_applies_exact_receipt(monkeypatch) -> None:
    captured = []
    account_id = "sora-test-account"
    alias = "canary@universal"

    def fake_http_json(method, url, payload=None, **kwargs):
        captured.append({"method": method, "url": url, "payload": payload, **kwargs})
        if url.endswith("/plan"):
            return 200, current_onboarding_receipt(payload)
        return 202, current_onboarding_apply_response(account_id, alias)

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)
    result = MODULE.onboard_account(
        "https://taira.example",
        alias,
        account_id,
        network_id=MODULE.DEFAULT_NETWORK_ID,
        onboarding_token=ONBOARDING_TOKEN,
        permissions=["CanFoo", "", "CanFoo", "CanBar"],
    )

    assert result["status"] == "created"
    assert result["response_status"] == 202
    expected_request = {
        "version": 1,
        "alias": alias,
        "account_id": account_id,
        "permissions": ["CanBar", "CanFoo"],
    }
    expected_receipt = current_onboarding_receipt(expected_request)
    common = {
        "method": "POST",
        "headers": {MODULE.ACCOUNT_ONBOARDING_TOKEN_HEADER: ONBOARDING_TOKEN},
        "allow_redirects": False,
        "sensitive_value": ONBOARDING_TOKEN,
    }
    assert captured == [
        {
            **common,
            "url": "https://taira.example/v1/accounts/onboard/plan",
            "payload": expected_request,
        },
        {
            **common,
            "url": "https://taira.example/v1/accounts/onboard",
            "payload": {"receipt": expected_receipt},
        },
    ]
    rendered_requests = json.dumps(captured, sort_keys=True)
    assert ONBOARDING_TOKEN not in json.dumps([item["payload"] for item in captured])
    assert "public_key_hex" not in rendered_requests
    assert "private_key" not in rendered_requests
    assert result["receipt"] == expected_receipt


def test_onboarding_rejects_retired_synchronous_response(monkeypatch) -> None:
    account_id = "sora-test-account"
    alias = "canary@universal"

    def fake_http_json(_method, url, payload=None, **_kwargs):
        if url.endswith("/plan"):
            return 200, current_onboarding_receipt(payload)
        response = current_onboarding_apply_response(account_id, alias)
        response["status"] = "Applied"
        return 200, response

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)

    try:
        MODULE.onboard_account(
            "https://taira.example",
            alias,
            account_id,
            network_id=MODULE.DEFAULT_NETWORK_ID,
            onboarding_token=ONBOARDING_TOKEN,
        )
    except RuntimeError as error:
        assert "unexpected account onboarding apply status" in str(error)
    else:  # pragma: no cover
        raise AssertionError("retired synchronous onboarding was accepted")


def test_onboarding_rejects_substituted_receipt_request(monkeypatch) -> None:
    def fake_http_json(_method, _url, payload=None, **_kwargs):
        receipt = current_onboarding_receipt(payload)
        receipt["body"]["request"]["alias"] = "substituted@universal"
        return 200, receipt

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)

    try:
        MODULE.onboard_account(
            "https://taira.example",
            "canary@universal",
            "sora-test-account",
            network_id=MODULE.DEFAULT_NETWORK_ID,
            onboarding_token=ONBOARDING_TOKEN,
        )
    except RuntimeError as error:
        assert "differs from the submitted intent" in str(error)
    else:  # pragma: no cover
        raise AssertionError("substituted onboarding receipt was accepted")


def test_onboarding_rejects_foreign_network_receipt_before_apply(monkeypatch) -> None:
    calls = []

    def fake_http_json(_method, _url, payload=None, **_kwargs):
        calls.append(payload)
        receipt = current_onboarding_receipt(payload)
        receipt["body"]["network_id"] = "genesis"
        return 200, receipt

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)

    with pytest.raises(RuntimeError, match="exact local network"):
        MODULE.onboard_account(
            "https://taira.example",
            "canary@universal",
            "sora-test-account",
            network_id=MODULE.DEFAULT_NETWORK_ID,
            onboarding_token=ONBOARDING_TOKEN,
        )
    assert len(calls) == 1


@pytest.mark.parametrize("retired", ["chain", "chainId", "chain_id"])
@pytest.mark.parametrize("keep_network_id", [False, True])
def test_onboarding_rejects_retired_receipt_network_keys(
    monkeypatch,
    retired,
    keep_network_id,
) -> None:
    calls = []

    def fake_http_json(_method, _url, payload=None, **_kwargs):
        calls.append(payload)
        receipt = current_onboarding_receipt(payload)
        if not keep_network_id:
            receipt["body"].pop("network_id")
        receipt["body"][retired] = MODULE.DEFAULT_CHAIN_ID
        return 200, receipt

    monkeypatch.setattr(MODULE, "_http_json", fake_http_json)

    with pytest.raises(RuntimeError, match="invalid fields"):
        MODULE.onboard_account(
            "https://taira.example",
            "canary@universal",
            "sora-test-account",
            network_id=MODULE.DEFAULT_NETWORK_ID,
            onboarding_token=ONBOARDING_TOKEN,
        )
    assert len(calls) == 1


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
            network_id=MODULE.DEFAULT_NETWORK_ID,
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
                {"Location": "https://redirect.example/v1/accounts/onboard/plan"},
                io.BytesIO(f"server echoed {ONBOARDING_TOKEN}".encode()),
            )

    monkeypatch.setattr(
        MODULE.request,
        "build_opener",
        lambda *_handlers: RedirectOpener(),
    )

    status, body = MODULE._http_json(
        "POST",
        "https://taira.example/v1/accounts/onboard/plan",
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
            network_id=MODULE.DEFAULT_NETWORK_ID,
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
