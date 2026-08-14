"""Exact-network request admission for subscription mutation endpoints."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Iterable, Mapping

if TYPE_CHECKING:
    from .client import ToriiCanonicalRequestAuth
else:
    ToriiCanonicalRequestAuth = Any

_SUBSCRIPTION_STATUSES = frozenset(
    {"active", "paused", "past_due", "canceled", "suspended"}
)


def normalize_subscription_status(value: Any, context: str) -> str:
    """Return one canonical subscription lifecycle status."""

    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    normalized = value.strip().lower()
    if not normalized:
        raise ValueError(f"{context} must be a non-empty string")
    if normalized not in _SUBSCRIPTION_STATUSES:
        raise ValueError(f"{context} must be one of {sorted(_SUBSCRIPTION_STATUSES)}")
    return normalized


def signed_subscription_post(
    client: Any,
    path: str,
    payload: Mapping[str, Any],
    *,
    canonical_auth: ToriiCanonicalRequestAuth,
    context: str,
    expected_status: Iterable[int] = (200,),
) -> Mapping[str, Any]:
    """Send one body-bound subscription command without redirects or retries."""

    auth = client._require_canonical_auth(canonical_auth, context)
    authority = client._require_non_empty_string(
        payload.get("authority"),
        f"{context} authority",
    )
    if auth.account_id != authority:
        raise ValueError(
            f"{context}.canonical_auth.account_id must equal payload authority"
        )
    data = client._encode_json_body(payload)
    headers = client._canonical_request_headers(
        "POST",
        path,
        data,
        canonical_auth=auth,
        headers=None,
        has_body=True,
    )
    response = client._request(
        "POST",
        path,
        headers=headers,
        data=data,
        allow_retry=False,
        allow_redirects=False,
    )
    client._expect_status(response, expected_status)
    if response.status_code == 204:
        return {}
    return client._ensure_mapping(response.json(), context)
