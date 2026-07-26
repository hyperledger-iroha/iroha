"""Tests for scripts/taira_faucet_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest


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


def test_scrypt_digest_matches_rfc_vector() -> None:
    digest = MODULE.scrypt_digest(b"", salt=b"", n=16, r=1, p=1, dklen=64)
    assert digest.hex() == (
        "77d6576238657b203b19ca42c18a0497"
        "f16b4844e3074ae8dfdffa3fede21442"
        "fcd0069ded0948f8326a753a0fc81f17"
        "e8d3e0fb2e0d3628cf35e20c38d18906"
    )


def test_scrypt_digest_skips_libressl_empty_output(monkeypatch) -> None:
    expected = bytes(range(32))
    calls: list[str] = []

    def fake_run(cmd, **kwargs):  # noqa: ANN001, ANN003
        calls.append(cmd[0])
        if cmd[0] == "/usr/bin/openssl":
            return SimpleNamespace(
                returncode=0,
                stdout="",
                stderr="openssl:Error: 'kdf' is an invalid command.",
            )
        return SimpleNamespace(returncode=0, stdout=expected.hex(), stderr="")

    monkeypatch.setattr(MODULE.hashlib, "scrypt", None, raising=False)
    monkeypatch.setattr(
        MODULE,
        "openssl_candidates",
        lambda: ["/usr/bin/openssl", "/opt/homebrew/bin/openssl"],
    )
    monkeypatch.setattr(MODULE.Path, "exists", lambda self: True)
    monkeypatch.setattr(MODULE.subprocess, "run", fake_run)

    digest = MODULE.scrypt_digest(b"pw", salt=b"salt", n=16, r=1, p=1, dklen=32)

    assert digest == expected
    assert calls == ["/usr/bin/openssl", "/opt/homebrew/bin/openssl"]


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


def faucet_receipt(account_id: str = "sorauロ1example") -> dict[str, str]:
    """Build a current first-release faucet receipt fixture."""

    return {
        "account_id": account_id,
        "asset_definition_id": "xor#sora",
        "asset_id": f"xor#sora#{account_id}",
        "amount": "100",
        "tx_hash_hex": "ab" * 32,
        "status": "QUEUED",
    }


def pipeline_status(kind: str, tx_hash_hex: str = "ab" * 32) -> dict[str, object]:
    """Build a current canonical pipeline-status fixture."""

    return {
        "hash": tx_hash_hex,
        "status": {"kind": kind},
        "summary": kind,
        "scope": "global",
        "resolved_from": "state",
    }


def test_validate_faucet_receipt_accepts_current_contract() -> None:
    receipt = faucet_receipt()
    assert MODULE.validate_faucet_receipt(receipt, "sorauロ1example") == "ab" * 32


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        ({"status": "Applied"}, "status must be QUEUED"),
        ({"tx_hash_hex": "ab"}, "must encode 32 bytes"),
        ({"tx_hash_hex": "not-hex"}, "is not hex"),
        ({"tx_hash_hex": "AB" * 32}, "must use canonical lowercase hex"),
        ({"account_id": "sorauロ1wrong"}, "does not match request"),
        ({"asset_id": ""}, "is missing asset_id"),
    ],
)
def test_validate_faucet_receipt_rejects_stale_or_invalid_contract(
    mutation: dict[str, str], message: str
) -> None:
    receipt = faucet_receipt()
    receipt.update(mutation)
    with pytest.raises(RuntimeError, match=message):
        MODULE.validate_faucet_receipt(receipt, "sorauロ1example")


def test_claim_faucet_requires_queued_receipt_and_canonical_finality(monkeypatch) -> None:
    calls: list[tuple[str, str, object]] = []
    responses = iter(
        [
            (200, {"difficulty_bits": 0}),
            (202, faucet_receipt()),
            (200, pipeline_status("Queued")),
            (200, pipeline_status("Applied")),
        ]
    )

    def fake_http(method, url, payload=None):  # noqa: ANN001
        calls.append((method, url, payload))
        return next(responses)

    monkeypatch.setattr(MODULE, "_http_json", fake_http)
    monkeypatch.setattr(MODULE.time, "sleep", lambda _seconds: None)

    result = MODULE.claim_faucet(
        "sorauロ1example",
        "https://taira.sora.org/",
        status_timeout_ms=1_000,
        poll_interval_ms=1,
    )

    status_url = (
        "https://taira.sora.org/v1/pipeline/transactions/status"
        f"?hash={'ab' * 32}&scope=global"
    )
    assert calls == [
        ("GET", "https://taira.sora.org/v1/accounts/faucet/puzzle", None),
        ("POST", "https://taira.sora.org/v1/accounts/faucet", {"account_id": "sorauロ1example"}),
        ("GET", status_url, None),
        ("GET", status_url, None),
    ]
    assert result["response_status"] == 202
    assert result["response"]["status"] == "QUEUED"
    assert result["final_status"]["status"]["kind"] == "Applied"


def test_claim_faucet_rejects_retired_synchronous_response(monkeypatch) -> None:
    responses = iter(
        [
            (200, {"difficulty_bits": 0}),
            (200, {**faucet_receipt(), "status": "Applied"}),
        ]
    )
    monkeypatch.setattr(MODULE, "_http_json", lambda *_args, **_kwargs: next(responses))

    with pytest.raises(RuntimeError, match="faucet claim failed: status=200"):
        MODULE.claim_faucet("sorauロ1example", "https://taira.sora.org")


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"status_timeout_ms": -1}, "status_timeout_ms must not be negative"),
        ({"poll_interval_ms": 0}, "poll_interval_ms must be positive"),
    ],
)
def test_claim_faucet_rejects_invalid_polling_before_network(
    monkeypatch, kwargs: dict[str, int], message: str
) -> None:
    def unexpected_http(*_args, **_kwargs):  # noqa: ANN202
        raise AssertionError("invalid polling configuration must fail before network I/O")

    monkeypatch.setattr(MODULE, "_http_json", unexpected_http)
    with pytest.raises(ValueError, match=message):
        MODULE.claim_faucet("sorauロ1example", "https://taira.sora.org", **kwargs)


def test_wait_for_faucet_finality_rejects_hash_mismatch(monkeypatch) -> None:
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: (200, pipeline_status("Applied", "cd" * 32)),
    )

    with pytest.raises(RuntimeError, match="hash does not match faucet receipt"):
        MODULE.wait_for_faucet_finality(
            "https://taira.sora.org", "ab" * 32, timeout_ms=0, poll_interval_ms=1
        )


def test_pipeline_status_kind_rejects_retired_flat_status() -> None:
    with pytest.raises(RuntimeError, match="missing status object"):
        MODULE.pipeline_status_kind(
            {
                "hash": "ab" * 32,
                "scope": "global",
                "resolved_from": "state",
                "status": "Applied",
            },
            "ab" * 32,
        )


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        ({"scope": "local"}, "must report global scope"),
        ({"resolved_from": ""}, "missing resolved_from"),
        ({"status": {"kind": "Final"}}, "unknown status.kind"),
    ],
)
def test_pipeline_status_kind_rejects_noncanonical_payload(
    mutation: dict[str, object], message: str
) -> None:
    payload = pipeline_status("Applied")
    payload.update(mutation)
    with pytest.raises(RuntimeError, match=message):
        MODULE.pipeline_status_kind(payload, "ab" * 32)


@pytest.mark.parametrize("kind", ["Rejected", "Expired"])
def test_wait_for_faucet_finality_rejects_terminal_failure(monkeypatch, kind: str) -> None:
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: (200, pipeline_status(kind)),
    )

    with pytest.raises(RuntimeError, match=f"terminal status {kind}"):
        MODULE.wait_for_faucet_finality(
            "https://taira.sora.org", "ab" * 32, timeout_ms=0, poll_interval_ms=1
        )


def test_wait_for_faucet_finality_rejects_noncanonical_http_status(monkeypatch) -> None:
    monkeypatch.setattr(
        MODULE,
        "_http_json",
        lambda *_args, **_kwargs: (202, pipeline_status("Queued")),
    )

    with pytest.raises(RuntimeError, match="transaction status failed: status=202"):
        MODULE.wait_for_faucet_finality(
            "https://taira.sora.org", "ab" * 32, timeout_ms=0, poll_interval_ms=1
        )


def test_wait_for_faucet_finality_times_out_after_not_found(monkeypatch) -> None:
    monotonic_values = iter([10.0, 10.1])
    monkeypatch.setattr(MODULE, "_http_json", lambda *_args, **_kwargs: (404, {}))
    monkeypatch.setattr(MODULE.time, "monotonic", lambda: next(monotonic_values))

    with pytest.raises(RuntimeError, match="last_status=not_found"):
        MODULE.wait_for_faucet_finality(
            "https://taira.sora.org", "ab" * 32, timeout_ms=0, poll_interval_ms=1
        )


def test_http_json_never_sends_retired_version_header(monkeypatch) -> None:
    captured_headers: dict[str, str] = {}

    class FakeResponse:
        status = 200

        def __enter__(self):  # noqa: ANN204
            return self

        def __exit__(self, *_args):  # noqa: ANN204
            return False

        @staticmethod
        def read() -> bytes:
            return b"{}"

    def fake_urlopen(req):  # noqa: ANN001
        captured_headers.update(req.header_items())
        return FakeResponse()

    monkeypatch.setattr(MODULE.request, "urlopen", fake_urlopen)
    assert MODULE._http_json("GET", "https://taira.sora.org/status") == (200, {})
    assert all(key.lower() != "x-iroha-api-version" for key in captured_headers)


def test_main_threads_finality_configuration_to_claim(monkeypatch, capsys) -> None:
    captured: dict[str, object] = {}

    def fake_claim(account_id, torii_root, **kwargs):  # noqa: ANN001
        captured.update(account_id=account_id, torii_root=torii_root, **kwargs)
        return {"status": "claimed"}

    monkeypatch.setattr(MODULE, "claim_faucet", fake_claim)
    assert (
        MODULE.main(
            [
                "--account-id",
                "sorauロ1example",
                "--torii-root",
                "https://taira.sora.org",
                "--status-timeout-ms",
                "34567",
                "--poll-interval-ms",
                "250",
            ]
        )
        == 0
    )
    assert captured == {
        "account_id": "sorauロ1example",
        "torii_root": "https://taira.sora.org",
        "status_timeout_ms": 34567,
        "poll_interval_ms": 250,
    }
    assert json.loads(capsys.readouterr().out) == {"status": "claimed"}


@pytest.mark.parametrize(
    ("flag", "value", "message"),
    [
        ("--status-timeout-ms", "-1", "must not be negative"),
        ("--poll-interval-ms", "0", "must be positive"),
    ],
)
def test_main_rejects_invalid_finality_configuration_before_claim(
    monkeypatch, capsys, flag: str, value: str, message: str
) -> None:
    monkeypatch.setattr(
        MODULE,
        "claim_faucet",
        lambda *_args, **_kwargs: pytest.fail("claim must not run for invalid arguments"),
    )
    with pytest.raises(SystemExit, match="2"):
        MODULE.main(
            [
                "--account-id",
                "sorauロ1example",
                "--torii-root",
                "https://taira.sora.org",
                flag,
                value,
            ]
        )
    assert message in capsys.readouterr().err
