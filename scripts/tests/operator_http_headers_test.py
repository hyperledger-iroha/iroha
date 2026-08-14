"""Focused tests for runtime-only operator HTTP header construction."""

from pathlib import Path

import pytest

from scripts import operator_http_headers as helper


def test_context_loader_requires_owner_private_absolute_single_file(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    key_file = tmp_path / "operator.key"
    key_file.write_text("802620" + "11" * 32 + "\n", encoding="ascii")
    key_file.chmod(0o600)
    observed = []
    context = object()
    monkeypatch.setattr(
        helper,
        "load_operator_signing_context",
        lambda network_id, private_key: observed.append((network_id, private_key)) or context,
    )

    assert helper.load_operator_context_from_file("network-id", key_file) is context
    assert observed == [("network-id", "802620" + "11" * 32)]

    key_file.chmod(0o640)
    with pytest.raises(ValueError, match="0600"):
        helper.load_operator_context_from_file("network-id", key_file)
    with pytest.raises(ValueError, match="absolute"):
        helper.load_operator_context_from_file("network-id", Path("operator.key"))


def test_request_target_preserves_exact_query_and_rejects_credentials() -> None:
    assert (
        helper._request_target("https://validator.test/v1/sumeragi/status?view=2&height=1")
        == "/v1/sumeragi/status?view=2&height=1"
    )
    assert helper._request_target("https://validator.test") == "/"
    for url in [
        "relative/path",
        "https://user:secret@validator.test/v1/sumeragi/status",
        "https://validator.test/v1/sumeragi/status#fragment",
    ]:
        with pytest.raises(ValueError):
            helper._request_target(url)
