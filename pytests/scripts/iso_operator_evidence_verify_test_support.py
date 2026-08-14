"""Shared fixtures for ISO operator evidence verifier tests."""

import json

import iso_operator_evidence_verify as EVIDENCE


TEST_NETWORK_ID = "hash:0808080808080808080808080808080808080808080808080808080808080809#9F75"


def receipt_summary_ok(
    *,
    receipt_kind,
    verified_receipts,
    allow_failed,
    allow_insecure_http,
    allow_default_profile,
    require_source_files,
    receipts,
):
    return (
        set(receipt_kind) == EVIDENCE.REQUIRED_RECEIPT_KINDS
        and verified_receipts == len(receipts)
        and len(receipts) > 0
        and not allow_failed
        and not allow_insecure_http
        and not allow_default_profile
        and require_source_files
        and all(receipt.get("ok") is True for receipt in receipts)
        and not any(receipt.get("endpoint_requires_insecure_http") for receipt in receipts)
        and not any(
            receipt.get("receipt_kind") == "iso-rail-gateway"
            and receipt.get("profile") is None
            for receipt in receipts
        )
    )


def digest_summary(body):
    body.pop(EVIDENCE.SUMMARY_DIGEST_FIELD, None)
    body[EVIDENCE.SUMMARY_DIGEST_FIELD] = EVIDENCE.sha256_hex(
        EVIDENCE._canonical_json_bytes(body)
    )
    return body


def digest_receipt_summary(body):
    body.pop(EVIDENCE.SUMMARY_DIGEST_FIELD, None)
    body[EVIDENCE.SUMMARY_DIGEST_FIELD] = EVIDENCE.sha256_hex(
        EVIDENCE._canonical_json_bytes(body)
    )
    return body


def write_json(path, body):
    path.write_text(json.dumps(body, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def canary_receipt_path(kind, offset, name=None):
    receipt_dir = (
        "/ops/iso/notary-receipts"
        if kind == "iso-audit-notary"
        else "/ops/iso/rail-receipts"
    )
    return f"{receipt_dir}/{name or f'{kind}.{offset}.receipt.json'}"
