"""Tests for shared SoraFS evidence sensitivity checks."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_sensitivity import (  # noqa: E402
    MAX_SENSITIVE_FIELD_DEPTH,
    visit_sensitive_fields,
)


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
    assert "transport.accessToken must not be present in rollout evidence" in joined
    assert "transport.api-key must not be present in rollout evidence" in joined
    assert (
        "transport.httpAuthorizationHeader must not be present in rollout evidence"
        in joined
    )
    assert "transport.payloadIncluded must be false" in joined
    assert "transport.privateKey must not be present in rollout evidence" in joined
    assert (
        "transport.sealedSigningKeyMaterial must not be present in rollout evidence"
        in joined
    )
    assert (
        "transport.rawRequestBodyPreview must not be present in rollout evidence"
        in joined
    )
    assert "transport.response-body must not be present in rollout evidence" in joined


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


def test_sensitive_key_fragments_allow_payload_free_digest_and_absence_fields() -> None:
    errors: list[str] = []
    payload = {
        "probe": {
            "request_body_blake3": "a" * 64,
            "responseBodySha256": "b" * 64,
            "response_body_included": False,
            "private_key_absent": True,
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
        "payloadIncluded must be false",
        "nested.response_bodies_included must be false",
        "nested.request_body_included must be false",
    ]


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
        "7 key must be a string",
        "7.privateKey must not be present in rollout evidence",
    ]


def test_malformed_sensitive_key_configuration_fails_closed() -> None:
    cases = (
        "payload",
        b"payload",
        ("payload", 7),
        ("payload", ""),
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
            ["sensitive keys must be non-empty strings"],
        )


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
