"""One-owner Requests preparation and dispatch for canonical authentication."""

from __future__ import annotations

from typing import Any, Callable, Mapping, MutableMapping, Optional

import requests

from .canonical_request_v1 import require_zero_retry_adapter


class CanonicalRequestHeaderPlan(dict[str, str]):
    """Base headers plus signer state deferred until Requests fixes the target."""

    def __init__(self, headers: Mapping[str, str], canonical_auth: Any) -> None:
        super().__init__(headers)
        self.canonical_auth = canonical_auth
        self.reject_ambient_auth = False


class OperatorRequestHeaderPlan(dict[str, str]):
    """Base headers plus operator signer state deferred until preparation."""

    def __init__(self, headers: Mapping[str, str], context: Any) -> None:
        super().__init__(headers)
        self.context = context


class _NoAmbientAuth(requests.auth.AuthBase):
    """Truthful no-op auth that prevents Requests from consulting netrc."""

    def __call__(self, request: requests.PreparedRequest) -> requests.PreparedRequest:
        return request


_NO_AMBIENT_AUTH = _NoAmbientAuth()


def send_request(
    *,
    session: requests.Session,
    base_url: str,
    method: str,
    path: str,
    params: Optional[Mapping[str, Any]],
    headers: Optional[MutableMapping[str, str]],
    data: Optional[bytes],
    stream: bool,
    allow_retry: bool,
    allow_redirects: bool,
    timeout: Optional[float],
    build_headers: Callable[..., dict[str, str]],
    build_operator_headers: Callable[..., dict[str, str]],
) -> requests.Response:
    """Dispatch unsigned requests normally and signed requests through one preparation."""

    url = f"{base_url}{path}"
    if not isinstance(
        headers,
        (CanonicalRequestHeaderPlan, OperatorRequestHeaderPlan),
    ):
        if not allow_retry:
            require_zero_retry_adapter(session=session, url=url)
        return session.request(
            method,
            url,
            params=params,
            headers=headers,
            data=data,
            stream=stream,
            allow_redirects=allow_redirects,
            timeout=timeout,
        )
    if allow_retry or allow_redirects:
        raise ValueError("canonical requests must disable redirects and retries")
    reject_ambient_auth = bool(
        isinstance(headers, CanonicalRequestHeaderPlan)
        and headers.reject_ambient_auth
    )
    request = requests.Request(
        method,
        url,
        params=params,
        headers=dict(headers),
        data=data,
        auth=_NO_AMBIENT_AUTH if reject_ambient_auth else None,
    )
    try:
        prepared = session.prepare_request(request)
    except AttributeError as exc:
        raise ValueError(
            "canonical requests require a verifiable prepared-request transport"
        ) from exc
    if reject_ambient_auth:
        for name in ("Authorization", "Cookie", "Proxy-Authorization"):
            if name in prepared.headers:
                raise ValueError(
                    f"canonical request preparation introduced ambient {name}"
                )
    prepared_body = prepared.body
    if prepared_body is None:
        body = b""
    elif isinstance(prepared_body, (bytes, bytearray, memoryview)):
        body = bytes(prepared_body)
    else:
        raise TypeError("canonical request body must remain exact bytes after preparation")
    if isinstance(headers, CanonicalRequestHeaderPlan):
        auth = headers.canonical_auth
        signed_headers = build_headers(
            network_id=auth.network_id,
            account_id=auth.account_id,
            signer=auth.signer,
            method=method,
            path=prepared.path_url,
            body=body,
            timestamp_ms=auth.timestamp_ms,
            nonce=auth.nonce,
        )
    else:
        signed_headers = build_operator_headers(
            headers.context,
            method,
            prepared.path_url,
            body,
        )
    prepared.headers.update(signed_headers)
    prepared_url = prepared.url
    if not isinstance(prepared_url, str):
        raise ValueError("canonical request preparation returned no exact URL")
    require_zero_retry_adapter(session=session, url=prepared_url)
    try:
        # Canonical authentication signs the prepared target and headers.  Do not
        # consult trust_env after that audit: environment proxies can inject
        # Proxy-Authorization and environment CA variables can silently replace
        # the caller's TLS policy.  Snapshot only the explicit Session settings.
        settings: dict[str, Any] = {
            "stream": stream,
            "verify": session.verify,
            "cert": session.cert,
            "proxies": dict(session.proxies),
        }
        return session.send(
            prepared,
            allow_redirects=False,
            timeout=timeout,
            **settings,
        )
    except AttributeError as exc:
        raise ValueError(
            "canonical requests require a verifiable prepared-request transport"
        ) from exc
