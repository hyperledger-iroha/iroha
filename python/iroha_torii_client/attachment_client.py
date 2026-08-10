"""One-shot exact-network transport for tenant-scoped ZK attachments."""

from __future__ import annotations

from typing import Any, Mapping, Optional


def authenticated_attachment_request(
    client: Any,
    method: str,
    path: str,
    canonical_auth: Any,
    *,
    data: Optional[bytes] = None,
    headers: Optional[Mapping[str, str]] = None,
) -> Any:
    """Sign and dispatch one attachment request without redirects or retries."""

    auth = client._require_canonical_auth(canonical_auth, f"attachment {method.lower()}")
    body = data or b""
    signed_headers = client._canonical_request_headers(
        method,
        path,
        body,
        canonical_auth=auth,
        headers=headers,
        has_body=data is not None,
    )
    return client._request(
        method,
        path,
        data=data,
        headers=signed_headers,
        allow_retry=False,
        allow_redirects=False,
    )
