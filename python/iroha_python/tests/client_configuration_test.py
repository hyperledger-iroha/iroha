"""Fail-closed configuration and transport tests for the public Torii client."""

from __future__ import annotations

from dataclasses import replace
from unittest.mock import Mock

import pytest
import requests
from iroha_torii_client.client import ToriiLocalSigningContext

import iroha_python.client as client_module
from iroha_python import (
    LocalSigningContext,
    NetworkId,
    ToriiClient,
    create_torii_client,
    resolve_torii_client_config,
)


class TrackingResponse(requests.Response):
    """Response that records whether the retry loop released it."""

    def __init__(self, status: int) -> None:
        super().__init__()
        self.status_code = status
        self._content = b"{}"
        self.closed_by_client = False

    def close(self) -> None:
        self.closed_by_client = True
        super().close()


class RecordingSession(requests.Session):
    """Session stub that serves deterministic responses without network I/O."""

    def __init__(self, responses: list[requests.Response] | None = None) -> None:
        super().__init__()
        self.responses = list(responses or [])
        self.calls: list[tuple[str, str, dict[str, object]]] = []
        self.close_count = 0

    def request(self, method: str, url: str, **kwargs: object) -> requests.Response:
        self.calls.append((method, url, dict(kwargs)))
        if not self.responses:
            raise AssertionError(f"unexpected request {method} {url}")
        response = self.responses.pop(0)
        response.url = url
        return response

    def close(self) -> None:
        self.close_count += 1
        super().close()


class FalseyRecordingSession(RecordingSession):
    """Caller-owned session whose truthiness must not affect injection."""

    def __bool__(self) -> bool:
        return False


def test_resolver_is_deterministic_and_environment_is_explicit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("IROHA_TORII_TIMEOUT_MS", "1")

    assert resolve_torii_client_config().timeout == 30.0
    assert (
        resolve_torii_client_config(env={"IROHA_TORII_TIMEOUT_MS": "250"}).timeout
        == 0.25
    )


def test_factory_applies_resolved_config_and_explicit_overrides() -> None:
    config = {
        "torii_client": {
            "timeout": 2.5,
            "max_retries": 4,
            "backoff_initial": 0.25,
            "backoff_multiplier": 3.0,
            "max_backoff": 1.5,
            "retry_statuses": [429, 503],
            "retry_methods": ["GET", "POST"],
            "default_headers": {"X-Client-Test": "enabled"},
        }
    }
    session = RecordingSession()

    client = create_torii_client(
        "https://torii.example",
        session=session,
        config=config,
        timeout=1.25,
    )

    assert client._timeout == 1.25
    assert client._max_retries == 4
    assert client._backoff_initial == 0.25
    assert client._backoff_multiplier == 3.0
    assert client._backoff_cap == 1.5
    assert client._retry_statuses == frozenset({429, 503})
    assert client._retry_methods == frozenset({"GET", "POST"})
    assert client._default_headers == {
        "Accept": "application/json",
        "X-Client-Test": "enabled",
    }


def test_resolved_config_is_immutable_and_redacts_tokens() -> None:
    resolved = resolve_torii_client_config(
        overrides={
            "auth_token": "auth-secret",
            "api_token": "api-secret",
            "default_headers": {"X-Client": "python"},
        }
    )

    rendered = repr(resolved)
    assert "auth-secret" not in rendered
    assert "api-secret" not in rendered
    with pytest.raises(TypeError):
        resolved.default_headers["X-Client"] = "changed"  # type: ignore[index]

    with pytest.raises(ValueError, match="auth_token explicitly"):
        replace(
            resolved,
            default_headers={"Authorization": "Bearer leaked-secret"},
        )
    with pytest.raises(TypeError, match="SorafsAliasPolicy"):
        replace(
            resolved,
            sorafs_alias_policy={},  # type: ignore[arg-type]
        )


@pytest.mark.parametrize(
    ("setting", "value", "message"),
    [
        ("timeout", 0, "timeout"),
        ("timeout", float("nan"), "finite"),
        ("max_retries", True, "integer"),
        ("backoff_multiplier", 0.5, "at least 1"),
        ("max_backoff", 0.1, "max_backoff"),
        ("retry_statuses", [200], "HTTP error statuses"),
        ("retry_methods", ["TRACE"], "unsupported HTTP method"),
        ("auth_token", "line\nbreak", "printable ASCII"),
        (
            "default_headers",
            {"X-Trace": "contains\ttab"},
            "control characters",
        ),
        (
            "default_headers",
            {"X-Trace": "first", "x-trace": "second"},
            "duplicate HTTP header",
        ),
    ],
)
def test_resolver_rejects_unsafe_values(setting: str, value: object, message: str) -> None:
    with pytest.raises((TypeError, ValueError), match=message):
        resolve_torii_client_config(overrides={setting: value})


@pytest.mark.parametrize(
    ("milliseconds_key", "seconds_key"),
    [
        ("timeout_ms", "timeout"),
        ("backoff_initial_ms", "backoff_initial"),
        ("max_backoff_ms", "max_backoff"),
    ],
)
def test_resolver_rejects_ambiguous_duration_units(
    milliseconds_key: str,
    seconds_key: str,
) -> None:
    with pytest.raises(TypeError, match="cannot contain both"):
        resolve_torii_client_config(
            overrides={milliseconds_key: 1, seconds_key: 1}
        )


@pytest.mark.parametrize(
    "base_url",
    [
        "torii.example",
        "ftp://torii.example",
        "https://user:secret@torii.example",
        "https://torii.example/v1",
        "https://torii.example?token=secret",
        "https://torii.example/#fragment",
        "https://torii.exa\nmple",
        "https:\\attacker.example",
    ],
)
def test_client_requires_an_origin_only_http_url(base_url: str) -> None:
    with pytest.raises(ValueError, match="base_url"):
        ToriiClient(base_url, session=RecordingSession())


@pytest.mark.parametrize("token", ["", "has space", "line\nbreak", "tøkən"])
def test_route_tokens_reject_ambiguous_or_unsafe_values(token: str) -> None:
    client = ToriiClient("https://torii.example", session=RecordingSession())

    with pytest.raises(ValueError, match="printable ASCII"):
        client.set_auth_token(token)
    with pytest.raises(ValueError, match="printable ASCII"):
        client.set_api_token(token)

    client.set_auth_token("bearer-token")
    client.set_api_token("api-token")
    client.set_auth_token(None)
    client.set_api_token(None)
    assert "Authorization" not in client._default_headers
    assert "X-API-Token" not in client._default_headers


def test_request_is_origin_relative_does_not_redirect_and_closes_retries() -> None:
    retry = TrackingResponse(503)
    success = TrackingResponse(200)
    session = RecordingSession([retry, success])
    client = ToriiClient(
        "https://torii.example",
        session=session,
        max_retries=1,
        backoff_initial=0,
        backoff_max=0,
        retry_on_methods=["POST"],
    )

    response = client._request("post", "/v1/example", json_body={"value": 1})

    assert response is success
    assert retry.closed_by_client is True
    assert len(session.calls) == 2
    for method, url, kwargs in session.calls:
        assert method == "POST"
        assert url == "https://torii.example/v1/example"
        assert kwargs["allow_redirects"] is False
        assert kwargs["data"] == b'{"value":1}'

    with pytest.raises(ValueError, match="origin-relative"):
        client._request("GET", "//attacker.example/steal")
    with pytest.raises(ValueError, match="origin-relative"):
        client._request("GET", "https://attacker.example/steal")
    for hostile_path in ("/v1/has space", "/v1/line\nbreak", "/\\attacker"):
        with pytest.raises(ValueError, match="backslashes, spaces, or control"):
            client._request("GET", hostile_path)
    with pytest.raises(TypeError, match="data must be exact immutable bytes"):
        client._request("POST", "/v1/example", data=bytearray(b"mutable"))  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="params must be a mapping"):
        client._request("GET", "/v1/example", params=[])  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="json_body must be a mapping"):
        client._request("POST", "/v1/example", json_body=[])  # type: ignore[arg-type]


def test_request_rejects_hidden_http_adapter_retries_before_io() -> None:
    session = RecordingSession()
    session.mount("https://", requests.adapters.HTTPAdapter(max_retries=1))
    client = ToriiClient("https://torii.example", session=session)

    with pytest.raises(ValueError, match="adapter retries to be disabled"):
        client._request("GET", "/v1/example")
    assert session.calls == []


def test_client_closes_only_its_owned_session() -> None:
    supplied = RecordingSession()
    client = ToriiClient("https://torii.example", session=supplied)
    client.close()
    assert supplied.close_count == 0

    owned = ToriiClient("https://torii.example")
    assert owned._session.trust_env is False
    close = Mock(wraps=owned._session.close)
    owned._session.close = close
    with owned as active:
        assert active is owned
    close.assert_called_once_with()


def test_client_rejects_duck_typed_sessions() -> None:
    with pytest.raises(TypeError, match="requests.Session"):
        ToriiClient(
            "https://torii.example",
            session=object(),  # type: ignore[arg-type]
        )


def test_client_preserves_a_falsey_caller_owned_session() -> None:
    session = FalseyRecordingSession()

    client = ToriiClient("https://torii.example", session=session)

    assert client._session is session
    client.close()
    assert session.close_count == 0


def test_constructor_validates_before_session_creation_and_closes_failed_owned_session(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    created = 0

    def unexpected_session() -> RecordingSession:
        nonlocal created
        created += 1
        return RecordingSession()

    monkeypatch.setattr(client_module.requests, "Session", unexpected_session)
    with pytest.raises(ValueError, match="timeout"):
        ToriiClient("https://torii.example", timeout=0)
    assert created == 0

    hostile = RecordingSession()
    hostile.headers["Authorization"] = "Bearer ambient"
    monkeypatch.setattr(client_module.requests, "Session", lambda: hostile)
    with pytest.raises(ValueError, match="auth_token explicitly"):
        ToriiClient("https://torii.example")
    assert hostile.close_count == 1


def test_factory_rejects_ambiguous_config_sources() -> None:
    resolved = resolve_torii_client_config()
    with pytest.raises(ValueError, match="cannot be combined"):
        create_torii_client(
            "https://torii.example",
            session=RecordingSession(),
            resolved_config=resolved,
            overrides={"timeout": 1},
        )


@pytest.mark.parametrize(
    ("keyword", "value", "message"),
    [
        ("config", [], "config must be a mapping"),
        ("env", [], "env must be a mapping"),
        ("overrides", [], "overrides must be a mapping"),
        (
            "config",
            {"torii_client": []},
            "config\\['torii_client'\\] must be a mapping",
        ),
        (
            "config",
            {"torii": []},
            "config\\['torii'\\] must be a mapping",
        ),
    ],
)
def test_resolver_rejects_non_mapping_sources(
    keyword: str,
    value: object,
    message: str,
) -> None:
    with pytest.raises(TypeError, match=message):
        resolve_torii_client_config(**{keyword: value})  # type: ignore[arg-type]


def test_resolver_validates_all_configured_api_tokens() -> None:
    with pytest.raises(TypeError, match=r"torii\.api_tokens\[1\]"):
        resolve_torii_client_config(
            config={"torii": {"api_tokens": ["first", object()]}}
        )


def test_factory_rejects_invalid_resolved_config_and_header_mapping() -> None:
    with pytest.raises(TypeError, match="ResolvedToriiClientConfig"):
        create_torii_client(
            "https://torii.example",
            session=RecordingSession(),
            resolved_config=object(),  # type: ignore[arg-type]
        )
    with pytest.raises(TypeError, match="default_headers must be a mapping"):
        create_torii_client(
            "https://torii.example",
            session=RecordingSession(),
            default_headers=[],  # type: ignore[arg-type]
        )


def test_explicit_headers_override_resolved_headers_case_insensitively() -> None:
    client = create_torii_client(
        "https://torii.example",
        session=RecordingSession(),
        resolved_config=resolve_torii_client_config(
            overrides={"default_headers": {"X-Trace": "resolved"}}
        ),
        default_headers={"x-trace": "explicit"},
    )

    assert client._default_headers["x-trace"] == "explicit"
    assert "X-Trace" not in client._default_headers


def test_high_level_signing_context_is_adapted_for_inherited_workflows() -> None:
    network_id = NetworkId.from_bytes(bytes([0xA5]) * 32)
    context = LocalSigningContext(network_id)

    client = ToriiClient(
        "https://torii.example",
        session=RecordingSession(),
        local_signing_context=context,
    )

    assert client.local_signing_context is context
    assert isinstance(client._local_signing_context, ToriiLocalSigningContext)
    assert client._local_signing_context.network_id == network_id.literal


def test_wrong_high_level_signing_context_type_fails_before_session_use() -> None:
    session = RecordingSession()

    with pytest.raises(TypeError, match="LocalSigningContext"):
        ToriiClient(
            "https://torii.example",
            session=session,
            local_signing_context=object(),  # type: ignore[arg-type]
        )

    assert session.calls == []


def test_native_query_signing_keys_require_exact_canonical_encoding() -> None:
    expected = bytes([0x11]) * 32
    assert (
        ToriiClient._native_query_signing_key(
            private_key=None,
            private_key_hex="11" * 32,
        )
        == expected
    )
    assert (
        ToriiClient._native_query_signing_key(
            private_key=expected,
            private_key_hex=None,
        )
        == expected
    )

    for hostile in ("11" * 31, "AA" * 32, " " + "11" * 32):
        with pytest.raises(ValueError, match="lowercase hexadecimal"):
            ToriiClient._native_query_signing_key(
                private_key=None,
                private_key_hex=hostile,
            )
    with pytest.raises(TypeError, match="exact immutable bytes"):
        ToriiClient._native_query_signing_key(
            private_key=bytearray(32),  # type: ignore[arg-type]
            private_key_hex=None,
        )
