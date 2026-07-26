"""Tests for shared SoraFS evidence sensitivity checks."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_sensitivity import (  # noqa: E402
    COMMON_SENSITIVE_KEYS,
    MAX_SENSITIVE_FIELD_DEPTH,
    normalize_sensitive_key,
    visit_sensitive_fields,
)


def test_common_sensitive_keys_inventory_is_canonical_and_enforced() -> None:
    assert COMMON_SENSITIVE_KEYS
    assert len(COMMON_SENSITIVE_KEYS) == len(
        {normalize_sensitive_key(key) for key in COMMON_SENSITIVE_KEYS}
    )
    assert all(
        isinstance(key, str)
        and key
        and key == key.strip()
        and key == key.lower()
        and all(character.isalnum() or character == "_" for character in key)
        for key in COMMON_SENSITIVE_KEYS
    )

    errors: list[str] = []
    visit_sensitive_fields(
        {key: "redacted" for key in COMMON_SENSITIVE_KEYS},
        "",
        errors,
        sensitive_keys=frozenset(),
    )

    assert len(errors) == len(COMMON_SENSITIVE_KEYS)
    assert set(errors) == {"<sensitive-key> must not be present in rollout evidence"}


def test_normalized_sensitive_key_variants_fail() -> None:
    errors: list[str] = []
    payload = {
        "transport": {
            "accessToken": "runtime-only-token",
            "api-key": "runtime-only-key",
            "httpAuthorizationHeader": "Bearer runtime-only-token",
            "payloadIncluded": True,
            "privateKey": "runtime-only-private-key",
            "sealedSigningKeyMaterial": "runtime-only-signing-key",
            "rawRequestBodyPreview": "{}",
            "response-body": "{}",
        }
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"payload", "private_key", "response_body"},
    )

    joined = "\n".join(errors)
    assert (
        joined.count("transport.<sensitive-key> must not be present in rollout evidence")
        == 7
    )
    assert "transport.<sensitive-inclusion-marker> must be false" in joined
    assert "accessToken" not in joined
    assert "api-key" not in joined
    assert "httpAuthorizationHeader" not in joined
    assert "payloadIncluded" not in joined
    assert "privateKey" not in joined
    assert "sealedSigningKeyMaterial" not in joined
    assert "rawRequestBodyPreview" not in joined
    assert "response-body" not in joined


def test_sensitive_key_fragments_do_not_reject_legitimate_proof_token_fields() -> None:
    errors: list[str] = []
    payload = {
        "proofTokenId": "token-1",
        "proof_token_count": 3,
        "token_expiry_present": True,
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"payload", "response_body"},
    )

    assert errors == []


def test_token_alias_sensitive_key_fragments_fail_without_leaking_names() -> None:
    errors: list[str] = []
    payload = {
        "apiToken": "runtime-only-token",
        "auth-token": "runtime-only-token",
        "idToken": "runtime-only-token",
        "jwt": "runtime-only-token",
        "oauthToken": "runtime-only-token",
        "refreshToken": "runtime-only-token",
        "sessionToken": "runtime-only-token",
        "setCookie": "runtime-only-cookie",
        "xApiToken": "runtime-only-token",
        "password": "runtime-only-password",
    }

    visit_sensitive_fields(payload, "", errors, sensitive_keys={"payload"})

    assert errors == [
        "<sensitive-key> must not be present in rollout evidence"
    ] * len(payload)
    joined = "\n".join(errors)
    assert "apiToken" not in joined
    assert "auth-token" not in joined
    assert "idToken" not in joined
    assert "jwt" not in joined
    assert "oauthToken" not in joined
    assert "refreshToken" not in joined
    assert "sessionToken" not in joined
    assert "setCookie" not in joined
    assert "xApiToken" not in joined
    assert "password" not in joined


def test_encoded_sensitive_key_fragments_fail_without_leaking_names() -> None:
    errors: list[str] = []
    payload = {
        "transport": {
            "private%5Fkey": "runtime-only-private-key",
            "access%54oken": "runtime-only-token",
            "response&#95;body": "{}",
            "session%26%2395%3Btoken": "runtime-only-token",
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"private_key", "response_body"},
    )

    assert errors == [
        "transport.<sensitive-key> must not be present in rollout evidence"
    ] * 4
    joined = "\n".join(errors)
    assert "private%5Fkey" not in joined
    assert "access%54oken" not in joined
    assert "response&#95;body" not in joined
    assert "session%26%2395%3Btoken" not in joined
    assert "private_key" not in joined
    assert "accessToken" not in joined
    assert "response_body" not in joined
    assert "session_token" not in joined


def test_unicode_compatibility_sensitive_key_fragments_fail_without_leaking_names() -> None:
    errors: list[str] = []
    fullwidth_private_key = (
        "\uff50\uff52\uff49\uff56\uff41\uff54\uff45"
        "\uff3f\uff4b\uff45\uff59"
    )
    fullwidth_bearer_token = (
        "\uff42\uff45\uff41\uff52\uff45\uff52\uff3f"
        "\uff54\uff4f\uff4b\uff45\uff4e"
    )
    payload = {
        "transport": {
            fullwidth_private_key: "runtime-only-private-key",
            fullwidth_bearer_token: "runtime-only-token",
        },
    }

    assert normalize_sensitive_key(fullwidth_private_key) == "privatekey"
    assert normalize_sensitive_key(fullwidth_bearer_token) == "bearertoken"

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"private_key"},
    )

    assert errors == [
        "transport.<sensitive-key> must not be present in rollout evidence",
        "transport.<sensitive-key> must not be present in rollout evidence",
    ]
    joined = "\n".join(errors)
    assert fullwidth_private_key not in joined
    assert fullwidth_bearer_token not in joined
    assert "private_key" not in joined
    assert "bearer_token" not in joined


def test_secret_like_values_under_neutral_keys_fail_without_leaking_values() -> None:
    errors: list[str] = []
    jwt_value = (
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9."
        "eyJzdWIiOiIxMjM0NTY3ODkwIn0."
        "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
    )
    payload = {
        "transport": {
            "observedHeader": "Bearer runtime-only-token",
            "encodedHeader": "Bearer%20runtime-only-token",
            "zeroWidthHeader": "Be\u200darer runtime-only-token",
            "encodedZeroWidthHeader": "Be%E2%80%8Darer runtime-only-token",
            "authorizationAssignment": "authorization=Bearer runtime-only-assignment",
            "encodedAssignment": "auth%2Dtoken=runtime-only-assignment",
            "basicHeader": "Basic dXNlcjpwYXNz",
            "headers": [
                "Authorization: Bearer runtime-only-header",
                "Cookie: session=runtime-only-cookie",
                "Set-Cookie: session=runtime-only-cookie",
            ],
            "headerBlock": (
                "HTTP/1.1 200 OK\n"
                "Content-Type: application/json\n"
                "Authorization: Bearer runtime-only-block"
            ),
            "encodedHeaderBlock": (
                "HTTP%2F1.1%20200%20OK%0A"
                "Set-Cookie%3A%20session%3Druntime-only-block"
            ),
            "foldedAuthorizationBlock": (
                "HTTP/1.1 200 OK\n"
                "Content-Type: application/json\n"
                "Authorization:\n"
                " runtime-only-folded-token"
            ),
            "encodedFoldedCookieBlock": (
                "HTTP%2F1.1%20200%20OK%0A"
                "Cache-Control%3A%20no-store%0A"
                "Set-Cookie%3A%0A%20session%3Druntime-only-folded"
            ),
            "jwtValue": jwt_value,
            "pemValue": "-----BEGIN PRIVATE KEY-----\nruntime-only-key",
        },
    }

    visit_sensitive_fields(payload, "", errors, sensitive_keys={"payload"})

    assert errors == [
        "transport.observedHeader must not contain secret-looking values in rollout evidence",
        "transport.encodedHeader must not contain secret-looking values in rollout evidence",
        "transport.zeroWidthHeader must not contain secret-looking values in rollout evidence",
        "transport.encodedZeroWidthHeader must not contain secret-looking values in rollout evidence",
        "transport.authorizationAssignment must not contain secret-looking values in rollout evidence",
        "transport.encodedAssignment must not contain secret-looking values in rollout evidence",
        "transport.basicHeader must not contain secret-looking values in rollout evidence",
        "transport.headers[0] must not contain secret-looking values in rollout evidence",
        "transport.headers[1] must not contain secret-looking values in rollout evidence",
        "transport.headers[2] must not contain secret-looking values in rollout evidence",
        "transport.headerBlock must not contain secret-looking values in rollout evidence",
        "transport.encodedHeaderBlock must not contain secret-looking values in rollout evidence",
        "transport.foldedAuthorizationBlock must not contain secret-looking values in rollout evidence",
        "transport.encodedFoldedCookieBlock must not contain secret-looking values in rollout evidence",
        "transport.jwtValue must not contain secret-looking values in rollout evidence",
        "transport.pemValue must not contain secret-looking values in rollout evidence",
    ]
    joined = "\n".join(errors)
    assert "Bearer runtime-only-token" not in joined
    assert "Bearer%20runtime-only-token" not in joined
    assert "Be\u200darer runtime-only-token" not in joined
    assert "Be%E2%80%8Darer runtime-only-token" not in joined
    assert "authorization=Bearer" not in joined
    assert "auth%2Dtoken" not in joined
    assert "runtime-only-assignment" not in joined
    assert "runtime-only-header" not in joined
    assert "runtime-only-block" not in joined
    assert "runtime-only-folded-token" not in joined
    assert "runtime-only-folded" not in joined
    assert "Set-Cookie" not in joined
    assert "dXNlcjpwYXNz" not in joined
    assert "runtime-only-cookie" not in joined
    assert jwt_value not in joined
    assert "BEGIN PRIVATE KEY" not in joined


def test_unicode_compatibility_secret_like_values_fail_without_leaking_values() -> None:
    errors: list[str] = []
    fullwidth_bearer = (
        "\uff22\uff45\uff41\uff52\uff45\uff52 runtime-only-token"
    )
    fullwidth_private_key_assignment = (
        "\uff50\uff52\uff49\uff56\uff41\uff54\uff45"
        "\uff3f\uff4b\uff45\uff59=runtime-only-key"
    )
    fullwidth_secret_url = (
        "\uff48\uff54\uff54\uff50\uff53://torii.example/path?"
        "\uff41\uff43\uff43\uff45\uff53\uff53"
        "\uff3f\uff54\uff4f\uff4b\uff45\uff4e=runtime-only-token"
    )
    payload = {
        "transport": {
            "fullwidthBearer": fullwidth_bearer,
            "fullwidthAssignment": fullwidth_private_key_assignment,
            "fullwidthUrl": fullwidth_secret_url,
        },
    }

    visit_sensitive_fields(payload, "", errors, sensitive_keys={"payload"})

    assert errors == [
        "transport.fullwidthBearer must not contain secret-looking values in rollout evidence",
        (
            "transport.fullwidthAssignment must not contain secret-looking values "
            "in rollout evidence"
        ),
        "transport.fullwidthUrl must not contain secret-looking values in rollout evidence",
    ]
    joined = "\n".join(errors)
    assert fullwidth_bearer not in joined
    assert fullwidth_private_key_assignment not in joined
    assert fullwidth_secret_url not in joined
    assert "runtime-only-token" not in joined
    assert "runtime-only-key" not in joined
    assert "private_key" not in joined
    assert "access_token" not in joined


def test_secret_like_url_values_under_neutral_keys_fail_without_leaking() -> None:
    errors: list[str] = []
    payload = {
        "transport": {
            "safeUrl": "https://torii.example/request_body_digest/abc?digest=123",
            "safeEncodedUrl": (
                "https%3A%2F%2Ftorii.example%2Frequest_body_digest%2Fabc"
                "%3Fdigest%3D123"
            ),
            "safeIpfsDigestUrl": "ipfs://bafybeigdyrzt/request_body_digest?digest=123",
            "userinfoUrl": "https://user:runtime-secret@torii.example/path",
            "encodedWholeUrl": (
                "https%3A%2F%2Fuser%3Aruntime-secret%40torii.example%2Fpath"
            ),
            "databaseUserinfoUrl": "postgres://user:runtime-secret@db.example/sorafs",
            "encodedDatabaseUserinfoUrl": (
                "postgres%3A%2F%2Fuser%3Aruntime-secret%40db.example%2Fsorafs"
            ),
            "queryUrl": "https://torii.example/path?access_token=runtime-secret",
            "websocketQueryUrl": (
                "wss://torii.example/socket?access_token=runtime-secret"
            ),
            "zeroWidthWebsocketQueryUrl": (
                "w\u200dss://torii.example/socket?access_token=runtime-secret"
            ),
            "encodedZeroWidthWebsocketQueryUrl": (
                "w%E2%80%8Dss://torii.example/socket?access_token=runtime-secret"
            ),
            "doubleEncodedQueryUrl": (
                "https%253A%252F%252Ftorii.example%252Fpath"
                "%253Faccess_token%253Druntime-secret"
            ),
            "queryBearerValueUrl": (
                "https://torii.example/path?redirect=Bearer%20runtime-secret"
            ),
            "queryJwtValueUrl": (
                "https://torii.example/path?"
                "assertion=eyJhbGciOiJIUzI1NiJ9."
                "eyJzdWIiOiIxMjM0NTY3ODkwIn0.signature000"
            ),
            "queryRedirectValueUrl": (
                "https://torii.example/path?"
                "return_to=https%3A%2F%2Fuser%3Aruntime-secret%40torii.example"
            ),
            "encodedQueryUrl": (
                "https://torii.example/path?session%255Ftoken=runtime-secret"
            ),
            "hostUrl": "https://api-token.example/hook",
            "pathUrl": "https://torii.example/private%5Fkey/hook",
            "filePathUrl": "file:///tmp/private_key/material",
        },
    }

    visit_sensitive_fields(payload, "", errors, sensitive_keys={"payload"})

    assert errors == [
        "transport.userinfoUrl must not contain secret-looking values in rollout evidence",
        "transport.encodedWholeUrl must not contain secret-looking values in rollout evidence",
        "transport.databaseUserinfoUrl must not contain secret-looking values in rollout evidence",
        "transport.encodedDatabaseUserinfoUrl must not contain secret-looking values in rollout evidence",
        "transport.queryUrl must not contain secret-looking values in rollout evidence",
        "transport.websocketQueryUrl must not contain secret-looking values in rollout evidence",
        "transport.zeroWidthWebsocketQueryUrl must not contain secret-looking values in rollout evidence",
        "transport.encodedZeroWidthWebsocketQueryUrl must not contain secret-looking values in rollout evidence",
        "transport.doubleEncodedQueryUrl must not contain secret-looking values in rollout evidence",
        "transport.queryBearerValueUrl must not contain secret-looking values in rollout evidence",
        "transport.queryJwtValueUrl must not contain secret-looking values in rollout evidence",
        "transport.queryRedirectValueUrl must not contain secret-looking values in rollout evidence",
        "transport.encodedQueryUrl must not contain secret-looking values in rollout evidence",
        "transport.hostUrl must not contain secret-looking values in rollout evidence",
        "transport.pathUrl must not contain secret-looking values in rollout evidence",
        "transport.filePathUrl must not contain secret-looking values in rollout evidence",
    ]
    joined = "\n".join(errors)
    assert "safeUrl" not in joined
    assert "safeEncodedUrl" not in joined
    assert "safeIpfsDigestUrl" not in joined
    assert "runtime-secret" not in joined
    assert "encodedWholeUrl" in joined
    assert "encodedDatabaseUserinfoUrl" in joined
    assert "websocketQueryUrl" in joined
    assert "zeroWidthWebsocketQueryUrl" in joined
    assert "encodedZeroWidthWebsocketQueryUrl" in joined
    assert "doubleEncodedQueryUrl" in joined
    assert "access_token" not in joined
    assert "postgres://" not in joined
    assert "w\u200dss://" not in joined
    assert "w%E2%80%8Dss://" not in joined
    assert "Bearer%20runtime-secret" not in joined
    assert "signature000" not in joined
    assert "return_to" not in joined
    assert "session%255Ftoken" not in joined
    assert "api-token" not in joined
    assert "private%5Fkey" not in joined
    assert "file:///tmp" not in joined


def test_secret_like_values_under_sensitive_keys_do_not_duplicate_errors() -> None:
    errors: list[str] = []
    payload = {
        "authorization": "Bearer runtime-only-token",
        "token": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signature000",
    }

    visit_sensitive_fields(payload, "", errors, sensitive_keys={"payload"})

    assert errors == [
        "<sensitive-key> must not be present in rollout evidence",
        "<sensitive-key> must not be present in rollout evidence",
    ]
    joined = "\n".join(errors)
    assert "Bearer runtime-only-token" not in joined
    assert "signature000" not in joined


def test_sensitive_key_fragments_allow_payload_free_digest_and_absence_fields() -> None:
    errors: list[str] = []
    payload = {
        "probe": {
            "request_body_blake3": "a" * 64,
            "responseBodySha256": "b" * 64,
            "response_body_included": False,
            "private_key_absent": True,
            "absenceAssignment": "private_key_absent=true",
            "digestAssignment": "request_body_digest=" + "a" * 64,
            "multilineDigestMetadata": "\n".join(
                (
                    "request_body_digest=" + "a" * 64,
                    "response_body_included=false",
                )
            ),
            "foldedDigestMetadata": "\n".join(
                (
                    "request_body_digest:",
                    " " + "a" * 64,
                    "response_body_included:",
                    " false",
                )
            ),
            "included": False,
        }
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"request_body", "response_body", "private_key"},
    )

    assert errors == []


def test_encoded_payload_free_sensitive_references_remain_allowed() -> None:
    errors: list[str] = []
    payload = {
        "probe": {
            "private%5Fkey%5Fabsent": True,
            "request%5Fbody%5Fdigest": "a" * 64,
            "response%42odySha256": "b" * 64,
        }
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"request_body", "response_body", "private_key"},
    )

    assert errors == []


def test_inclusion_markers_reject_non_false_values() -> None:
    errors: list[str] = []
    payload = {
        "included": True,
        "payloadIncluded": "true",
        "nested": {
            "response_bodies_included": 1,
            "request_body_included": None,
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"payload", "response_body", "request_body"},
    )

    assert errors == [
        "included must be false",
        "<sensitive-inclusion-marker> must be false",
        "nested.<sensitive-inclusion-marker> must be false",
        "nested.<sensitive-inclusion-marker> must be false",
    ]
    joined = "\n".join(errors)
    assert "payloadIncluded" not in joined
    assert "response_bodies_included" not in joined
    assert "request_body_included" not in joined


def test_sensitive_inclusion_marker_key_names_are_redacted() -> None:
    errors: list[str] = []
    payload = {
        "transport": {
            "privateKeyIncluded": True,
            "accessTokenIncluded": True,
            "responseBodyIncluded": True,
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"access_token", "private_key", "response_body"},
    )

    joined = "\n".join(errors)
    assert (
        joined.count("transport.<sensitive-inclusion-marker> must be false")
        == 3
    )
    assert "privateKeyIncluded" not in joined
    assert "accessTokenIncluded" not in joined
    assert "responseBodyIncluded" not in joined


def test_encoded_sensitive_inclusion_marker_key_names_are_redacted() -> None:
    errors: list[str] = []
    payload = {
        "transport": {
            "private%4BeyIncluded": True,
            "access%54okenIncluded": True,
            "response%5Fbody%5Fincluded": True,
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"access_token", "private_key", "response_body"},
    )

    assert errors == [
        "transport.<sensitive-inclusion-marker> must be false"
    ] * 3
    joined = "\n".join(errors)
    assert "private%4BeyIncluded" not in joined
    assert "access%54okenIncluded" not in joined
    assert "response%5Fbody%5Fincluded" not in joined
    assert "privateKeyIncluded" not in joined
    assert "accessTokenIncluded" not in joined
    assert "response_body_included" not in joined


def test_non_string_payload_keys_fail_closed_and_still_scan_children() -> None:
    errors: list[str] = []
    payload = {
        7: {
            "privateKey": "runtime-only-private-key",
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"private_key"},
    )

    assert errors == [
        "<non-string-key> key must be a string",
        "<non-string-key>.<sensitive-key> must not be present in rollout evidence",
    ]


def test_noncanonical_sensitive_key_paths_are_sanitized() -> None:
    errors: list[str] = []
    payload = {
        "private\nkey": "runtime-only-private-key",
        "nested": {
            "response\nbody": "{}",
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"private_key", "response_body"},
    )

    joined = "\n".join(errors)
    assert "<sensitive-key> must not be present in rollout evidence" in joined
    assert "nested.<sensitive-key> must not be present in rollout evidence" in joined
    assert "private\nkey" not in joined
    assert "response\nbody" not in joined


def test_encoded_noncanonical_marker_paths_are_sanitized() -> None:
    errors: list[str] = []
    payload = {
        "public%0Aincluded": True,
        "nested": {
            "public%2Fincluded": True,
            "public.included": True,
            "public[included]": True,
            "public%5Fincluded": True,
            "public included": True,
            "publïcIncluded": True,
            "public&#95;included": True,
            "_publicIncluded": True,
            "publicIncluded_": True,
            "---included": True,
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"private_key", "response_body"},
    )

    assert errors == [
        "<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
        "nested.<non-canonical-key> must be false",
    ]
    joined = "\n".join(errors)
    assert "public%0Aincluded" not in joined
    assert "public%2Fincluded" not in joined
    assert "public.included" not in joined
    assert "public[included]" not in joined
    assert "public%5Fincluded" not in joined
    assert "public included" not in joined
    assert "publïcIncluded" not in joined
    assert "public&#95;included" not in joined
    assert "_publicIncluded" not in joined
    assert "publicIncluded_" not in joined
    assert "---included" not in joined
    assert "public_included" not in joined
    assert "public\nincluded" not in joined
    assert "public/included" not in joined


def test_sensitive_parent_path_is_redacted_for_nested_diagnostics() -> None:
    errors: list[str] = []
    payload = {
        "operatorPrivateKey": {
            "responseBody": "{}",
            "included": True,
        },
    }

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"private_key", "response_body"},
    )

    joined = "\n".join(errors)
    assert "<sensitive-key> must not be present in rollout evidence" in joined
    assert (
        "<sensitive-key>.<sensitive-key> must not be present in rollout evidence"
        in joined
    )
    assert "<sensitive-key>.included must be false" in joined
    assert "operatorPrivateKey" not in joined
    assert "responseBody" not in joined


def test_malformed_sensitive_key_configuration_fails_closed() -> None:
    cases = (
        "payload",
        b"payload",
        bytearray(b"payload"),
        {"payload": True},
        ("payload", 7),
        ("payload", ""),
        ("payload", " private_key"),
        ("payload", "private_key "),
        ("payload", "private\nkey"),
        ("payload", "idToken"),
        ("payload", "api-key"),
        ("payload", "_secret"),
        ("payload", "secret_"),
        ("payload", "private__key"),
    )

    for sensitive_keys in cases:
        errors: list[str] = []
        visit_sensitive_fields(
            {"payloadIncluded": True},
            "",
            errors,
            sensitive_keys=sensitive_keys,
        )
        assert errors in (
            ["sensitive keys must be a sequence of strings"],
            ["sensitive keys must be non-empty canonical strings"],
        )


def test_duplicate_sensitive_key_configuration_fails_closed() -> None:
    for sensitive_keys in (("payload", "payload"), ("raw_payload", "rawpayload")):
        errors: list[str] = []
        visit_sensitive_fields(
            {"payloadIncluded": True},
            "",
            errors,
            sensitive_keys=sensitive_keys,
        )
        assert errors == [
            "sensitive keys must not contain duplicate normalized names"
        ]


def test_common_sensitive_key_alias_configuration_fails_closed() -> None:
    errors: list[str] = []

    visit_sensitive_fields(
        {"payloadIncluded": True},
        "",
        errors,
        sensitive_keys={"apitoken"},
    )

    assert errors == ["sensitive keys must not alias common sensitive names"]


def test_sensitive_scan_rejects_malformed_error_container() -> None:
    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            visit_sensitive_fields(
                {"privateKey": "runtime-only-private-key"},
                "",
                errors,
                sensitive_keys={"private_key"},
            )
        except ValueError as error:
            assert "sensitive field errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_sensitive_scan_rejects_malformed_existing_error_text() -> None:
    for errors in (
        [""],
        [" old"],
        ["old "],
        ["old\nerror"],
        ["old\u200derror"],
        ["old\u202eerror"],
    ):
        try:
            visit_sensitive_fields(
                {"privateKey": "runtime-only-private-key"},
                "",
                errors,
                sensitive_keys={"private_key"},
            )
        except ValueError as error:
            assert (
                "sensitive field errors must contain non-empty canonical strings"
                in str(error)
            )
        else:
            raise AssertionError(f"accepted malformed error text {errors!r}")


def test_sensitive_scan_rejects_malformed_path_before_payload_scan() -> None:
    for path in (
        " root",
        "root ",
        "root\nchild",
        "root\u200dchild",
        "root\u202echild",
        "root%5Fchild",
        "root..child",
        "root[01]",
        "root[]",
        "root[0]tail",
        "root[0].child%2Ename",
        "root.χild",
        "_",
        "---",
        "root._child",
        "root.child_",
        "root.-child",
        7,
    ):
        errors: list[str] = []

        visit_sensitive_fields(
            {"privateKey": "runtime-only-private-key"},
            path,
            errors,
            sensitive_keys={"private_key"},
        )

        assert errors == [
            "sensitive field path must be a non-empty canonical string"
        ]


def test_sensitive_scan_accepts_canonical_starting_paths() -> None:
    for path in ("root", "root_child", "root-child", "root[0]", "root[0].child[12]"):
        errors: list[str] = []

        visit_sensitive_fields(
            {"privateKey": "runtime-only-private-key"},
            path,
            errors,
            sensitive_keys={"private_key"},
        )

        assert errors == [
            f"{path}.<sensitive-key> must not be present in rollout evidence"
        ]


def test_sensitive_scan_rejects_malformed_evidence_label_before_payload_scan() -> None:
    for evidence_label in (
        "",
        " rollout",
        "rollout ",
        "rollout  evidence",
        "rollout\nevidence",
        "rollout\u200devidence",
        "rollout\u202eevidence",
        "rollout%20evidence",
        "rollout&#95;evidence",
        "rollout/evidence",
        "rollout.evidence",
        "rollout[evidence]",
        "rollout|evidence",
        "rolloutévidence",
        "private_key evidence",
        "accessToken evidence",
        "_",
        "---",
        "_rollout",
        "rollout_",
        "-rollout",
        "rollout-",
        "rollout -",
        7,
    ):
        errors: list[str] = []

        visit_sensitive_fields(
            {"privateKey": "runtime-only-private-key"},
            "",
            errors,
            sensitive_keys={"private_key"},
            evidence_label=evidence_label,
        )

        assert errors == [
            "sensitive field evidence label must be a non-empty canonical string"
        ]


def test_sensitive_scan_accepts_canonical_evidence_labels() -> None:
    for evidence_label in (
        "rollout",
        "rollout evidence",
        "rollout_evidence",
        "rollout-evidence",
        "release",
        "SoraFS production readiness summary",
    ):
        errors: list[str] = []

        visit_sensitive_fields(
            {"privateKey": "runtime-only-private-key"},
            "",
            errors,
            sensitive_keys={"private_key"},
            evidence_label=evidence_label,
        )

        assert errors == [
            f"<sensitive-key> must not be present in {evidence_label}"
        ]


def test_overly_deep_sensitive_scan_fails_closed_without_recursion_error() -> None:
    errors: list[str] = []
    payload: dict = {}
    cursor = payload
    for index in range(MAX_SENSITIVE_FIELD_DEPTH + 2):
        child: dict = {}
        cursor[f"level_{index}"] = child
        cursor = child

    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys={"payload", "response_body", "request_body"},
    )

    assert len(errors) == 1
    assert f"nesting exceeds {MAX_SENSITIVE_FIELD_DEPTH} levels" in errors[0]
