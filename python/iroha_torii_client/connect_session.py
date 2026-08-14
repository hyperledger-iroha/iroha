"""Exact Connect session request and response validation."""

from __future__ import annotations

import base64
import binascii
import hashlib
import re
from typing import Any, Callable, Dict, Mapping, Tuple
from urllib.parse import parse_qsl, urlsplit

from .client_status_models import ConnectSessionInfo

HashLiteralValidator = Callable[[Any, str], str]

_SESSION_REQUEST_FIELDS = frozenset({"sid", "network_id", "app_pk", "nonce"})
_SESSION_OPTIONAL_FIELDS = frozenset({"node"})
_URI_FIELDS = frozenset(
    {"sid", "network_id", "app_pk", "nonce", "node", "v", "role", "token", "relay"}
)


def _require_exact_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    stripped = value.strip()
    if not stripped:
        raise ValueError(f"{context} must be a non-empty string")
    if stripped != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    return value


def _network_identity(
    value: Any,
    context: str,
    hash_literal: HashLiteralValidator,
) -> Tuple[str, bytes]:
    literal = _require_exact_non_empty_string(value, context)
    try:
        canonical = hash_literal(literal, context)
    except RuntimeError as exc:
        raise ValueError(str(exc)) from exc
    return canonical, bytes.fromhex(canonical[5:69])


def _base64url(value: Any, length: int, context: str) -> Tuple[str, bytes]:
    encoded = _require_exact_non_empty_string(value, context)
    if re.fullmatch(r"[A-Za-z0-9_-]+", encoded) is None:
        raise ValueError(f"{context} must be canonical unpadded base64url")
    try:
        decoded = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4))
    except (binascii.Error, ValueError) as exc:
        raise ValueError(f"{context} must be canonical unpadded base64url") from exc
    canonical = base64.urlsafe_b64encode(decoded).rstrip(b"=").decode("ascii")
    if len(decoded) != length or canonical != encoded:
        raise ValueError(
            f"{context} must be canonical unpadded base64url for exactly {length} bytes"
        )
    return encoded, decoded


def canonical_connect_sid(value: Any, context: str = "Connect session sid") -> str:
    """Return a canonical unpadded base64url 32-byte session identifier."""

    return _base64url(value, 32, context)[0]


def canonical_connect_token(value: Any, context: str) -> str:
    """Return a canonical unpadded base64url 32-byte Connect bearer token."""

    return _base64url(value, 32, context)[0]


def normalize_connect_session_request(
    payload: Mapping[str, Any],
    *,
    hash_literal: HashLiteralValidator,
) -> Dict[str, str]:
    """Validate and canonicalize an exact Connect session request."""

    if not isinstance(payload, Mapping):
        raise TypeError("Connect session request must be a mapping")
    request = dict(payload)
    retired = {"chain", "chain_id", "chainId", "genesis_hash", "genesisHash"}.intersection(
        request
    )
    if retired:
        raise ValueError("chain identity aliases are retired; provide exact network_id")
    missing = _SESSION_REQUEST_FIELDS.difference(request)
    if missing:
        raise ValueError(f"Connect session request missing required fields: {sorted(missing)}")
    unsupported = set(request).difference(_SESSION_REQUEST_FIELDS | _SESSION_OPTIONAL_FIELDS)
    if unsupported:
        rendered = sorted(str(field) for field in unsupported)
        raise ValueError(f"Connect session request has unsupported fields: {rendered}")

    network_id, network_bytes = _network_identity(
        request["network_id"],
        "Connect session request.network_id",
        hash_literal,
    )
    sid, sid_bytes = _base64url(request["sid"], 32, "Connect session request.sid")
    app_pk, app_pk_bytes = _base64url(request["app_pk"], 32, "Connect session request.app_pk")
    nonce, nonce_bytes = _base64url(request["nonce"], 16, "Connect session request.nonce")
    if not any(app_pk_bytes) or not any(nonce_bytes):
        raise ValueError("Connect session app_pk and nonce must not be all zero")
    expected_sid = hashlib.blake2b(
        b"iroha-connect|sid|" + network_bytes + app_pk_bytes + nonce_bytes,
        digest_size=32,
    ).digest()
    if sid_bytes != expected_sid:
        raise ValueError("Connect session sid does not match network_id, app_pk, and nonce")

    normalized = {
        "sid": sid,
        "network_id": network_id,
        "app_pk": app_pk,
        "nonce": nonce,
    }
    if "node" in request:
        normalized["node"] = _require_exact_non_empty_string(
            request["node"], "Connect session request.node"
        )
    return normalized


def _session_uri_query(
    uri: Any,
    *,
    context: str,
    session: ConnectSessionInfo,
    role: str,
    token: str,
) -> Dict[str, str]:
    value = _require_exact_non_empty_string(uri, context)
    parsed = urlsplit(value)
    if (
        parsed.scheme != "iroha"
        or parsed.netloc != "connect"
        or parsed.path not in {"", "/"}
        or parsed.fragment
    ):
        raise ValueError(f"{context} must be an iroha://connect URI")
    try:
        pairs = parse_qsl(parsed.query, keep_blank_values=True, strict_parsing=True)
    except ValueError as exc:
        raise ValueError(f"{context} contains a malformed query") from exc
    keys = [key for key, _ in pairs]
    duplicates = sorted({key for key in keys if keys.count(key) > 1})
    if duplicates:
        raise ValueError(f"{context} contains duplicate parameters: {duplicates}")
    query = dict(pairs)
    if set(query) != _URI_FIELDS:
        missing = sorted(_URI_FIELDS.difference(query))
        unsupported = sorted(set(query).difference(_URI_FIELDS))
        raise ValueError(
            f"{context} has an inexact parameter set; missing={missing}, unsupported={unsupported}"
        )
    expected = {
        "sid": session.sid,
        "network_id": session.network_id,
        "app_pk": session.app_pk,
        "nonce": session.nonce,
        "v": "1",
        "role": role,
        "token": token,
        "relay": session.token_relay,
    }
    if any(query[field] != expected_value for field, expected_value in expected.items()):
        raise ValueError(f"{context} substituted Connect session identity or authorization")
    return query


def ensure_connect_session_matches_request(
    session: ConnectSessionInfo,
    request: Mapping[str, str],
) -> None:
    """Reject a Torii response that substitutes request identity or node data."""

    if any(
        (
            session.sid != request["sid"],
            session.network_id != request["network_id"],
            session.app_pk != request["app_pk"],
            session.nonce != request["nonce"],
        )
    ):
        raise ValueError("Torii Connect session response substituted request identity")
    expected_node = request.get("node", "")
    wallet_query = _session_uri_query(
        session.wallet_uri,
        context="connect session.wallet_uri",
        session=session,
        role="wallet",
        token=session.token_wallet,
    )
    app_query = _session_uri_query(
        session.app_uri,
        context="connect session.app_uri",
        session=session,
        role="app",
        token=session.token_app,
    )
    if wallet_query["node"] != expected_node or app_query["node"] != expected_node:
        raise ValueError("Torii Connect session response substituted request node")


def parse_connect_session(
    payload: Mapping[str, Any],
    *,
    context: str,
    hash_literal: HashLiteralValidator,
) -> ConnectSessionInfo:
    """Parse and validate an exact Connect session response."""

    if not isinstance(payload, Mapping):
        raise RuntimeError(f"{context} response must be a JSON object")
    record = payload
    known = {
        "sid",
        "network_id",
        "app_pk",
        "nonce",
        "wallet_uri",
        "app_uri",
        "token_app",
        "token_wallet",
        "token_management",
        "token_relay",
    }
    if set(record) != known:
        missing = sorted(known.difference(record))
        unsupported = sorted(set(record).difference(known))
        raise ValueError(
            f"{context} response has an inexact field set; "
            f"missing={missing}, unsupported={unsupported}"
        )
    identity = normalize_connect_session_request(
        {
            "sid": record.get("sid"),
            "network_id": record.get("network_id"),
            "app_pk": record.get("app_pk"),
            "nonce": record.get("nonce"),
        },
        hash_literal=hash_literal,
    )
    wallet_uri = _require_exact_non_empty_string(
        record.get("wallet_uri"), f"{context}.wallet_uri"
    )
    app_uri = _require_exact_non_empty_string(record.get("app_uri"), f"{context}.app_uri")
    token_app = _base64url(record.get("token_app"), 32, f"{context}.token_app")[0]
    token_wallet = _base64url(record.get("token_wallet"), 32, f"{context}.token_wallet")[0]
    token_management = _base64url(
        record.get("token_management"), 32, f"{context}.token_management"
    )[0]
    token_relay = _base64url(record.get("token_relay"), 32, f"{context}.token_relay")[0]
    session = ConnectSessionInfo(
        sid=identity["sid"],
        network_id=identity["network_id"],
        app_pk=identity["app_pk"],
        nonce=identity["nonce"],
        wallet_uri=wallet_uri,
        app_uri=app_uri,
        token_app=token_app,
        token_wallet=token_wallet,
        token_management=token_management,
        token_relay=token_relay,
    )
    wallet_query = _session_uri_query(
        wallet_uri,
        context=f"{context}.wallet_uri",
        session=session,
        role="wallet",
        token=token_wallet,
    )
    app_query = _session_uri_query(
        app_uri,
        context=f"{context}.app_uri",
        session=session,
        role="app",
        token=token_app,
    )
    if wallet_query["node"] != app_query["node"]:
        raise ValueError(f"{context} response URIs disagree on node")
    return session
