#!/usr/bin/env python3
"""Derive synthetic native sender-parser fixture inputs from current test sources.

This only writes the public input JSON for the ignored Rust exporter. It never
creates, relabels, or approves canonical Norito command bytes.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
GOLDEN = ROOT / "fixtures/offline/kagemusha_sender_release_parser_v1.json"
RELEASE_TEST = ROOT / "pytests/scripts/kagemusha_v1_release_evidence_test.py"
EXPECTED_CASES = {
    (case, operation)
    for case in ("trusted_valid", "lease_end", "trusted_before_expiry")
    for operation in ("send_split", "redeem_split")
}


def _load_release_test() -> Any:
    spec = importlib.util.spec_from_file_location("kagemusha_fixture_input_release_test", RELEASE_TEST)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def build_input() -> dict[str, Any]:
    """Capture three contexts and positive attempts before any native golden lookup."""
    test = _load_release_test()
    physical = test.PHYSICAL_TEST
    standalone: list[dict[str, Any]] = []
    builder = physical._TranscriptBuilder(None, {})
    document = builder.build(include_sender=False)
    builder.add_sender_validity(document, fixture_input_capture=standalone)
    assert len(standalone) == 1

    with tempfile.TemporaryDirectory(prefix="kagemusha-sender-fixture-input-") as directory:
        base = Path(directory).resolve()
        default: list[dict[str, Any]] = []
        provider_ab: list[dict[str, Any]] = []
        test._fixture(base / "default", sender_fixture_input_capture=default)
        test._fixture(
            base / "provider-ab", provider_commitment="ab" * 32,
            sender_fixture_input_capture=provider_ab,
        )
    assert len(default) == len(provider_ab) == 1

    bundles = {
        "physical_standalone": standalone[0],
        "release_default": default[0],
        "release_provider_ab": provider_ab[0],
    }
    for name, bundle in bundles.items():
        context = bundle["context"]
        profile = context["hardware_profile"]
        credential, = context["credentials"]
        assert profile["hardware_profile_id"] == test.VERIFIER.rust_hardware_profile_id(profile), name
        assert credential["credential_id"] == physical.physical.credential_identity(credential), name
        assert credential["hardware_profile_id"] == profile["hardware_profile_id"], name
        assert test.VERIFIER._p256_verify(
            bytes.fromhex(profile["governance_credential_public_key"]),
            physical.physical.credential_signing_bytes(credential),
            bytes.fromhex(credential["governance_signature"]),
        ), name
        rows = bundle["positive_attempts"]
        assert {(row["case"], row["operation_kind"]) for row in rows} == EXPECTED_CASES, name
        assert len(rows) == len(EXPECTED_CASES), name
        assert len({row["operation_id"] for row in rows}) == len(rows), name
        assert all(row["credential_id"] == credential["credential_id"] for row in rows), name
    return {
        "schema": "iroha.kagemusha_v1.sender_release_parser_fixture_inputs",
        "schema_version": 1,
        "contexts": {name: bundle["context"] for name, bundle in bundles.items()},
        "positive_attempts": {name: bundle["positive_attempts"] for name, bundle in bundles.items()},
    }


def validate_candidate(source: dict[str, Any], candidate: dict[str, Any]) -> None:
    """Check exporter structure and raw-byte hashes before installing a golden.

    This is not a Norito parser. The Rust golden replay remains mandatory.
    """
    assert candidate["schema"] == "iroha.kagemusha_v1.sender_release_parser_test_fixtures"
    assert candidate["schema_version"] == 1
    assert set(candidate["contexts"]) == set(source["contexts"])
    hashes: set[str] = set()
    for name, bundle in candidate["contexts"].items():
        assert bundle["context"] == source["contexts"][name], name
        rows = bundle["fixtures"]
        expected = {
            (attempt["case"], attempt["operation_kind"]): attempt
            for attempt in source["positive_attempts"][name]
        }
        assert len(rows) == len(expected), name
        assert {(row["case"], row["operation_kind"]) for row in rows} == set(expected), name
        for row in rows:
            attempt = expected[(row["case"], row["operation_kind"])]
            projection = row["projection"]
            assert projection["structural_only"] is True
            assert projection["operation"] == 12
            assert projection["operation_id"] == attempt["operation_id"]
            assert projection["credential_id"] == attempt["credential_id"]
            assert projection["operation_kind"] == attempt["operation_kind"]
            assert projection["preparation_id"] == attempt["preparation_sha256"]
            assert projection["candidate_digest"] == attempt["candidate_sha256"]
            assert projection["commit_evidence_commitment"] == attempt["commit_evidence_commitment"]
            assert projection["commit_evidence_source"] == attempt["source"]
            for raw_field, digest_field in (
                ("canonical_sender_command_hex", "command_sha256"),
                ("canonical_envelope_hex", "envelope_sha256"),
                ("canonical_certificate_hex", "certificate_sha256"),
                ("canonical_terminal_receipt_hex", "terminal_receipt_sha256"),
                ("canonical_hardware_authorization_hex", "hardware_authorization_sha256"),
            ):
                raw = bytes.fromhex(row[raw_field])
                assert raw and hashlib.sha256(raw).hexdigest() == projection[digest_field]
            assert projection["command_sha256"] not in hashes
            hashes.add(projection["command_sha256"])
            if attempt["operation_kind"] == "send_split":
                request = bytes.fromhex(row["canonical_request_hex"])
                assert request and hashlib.sha256(request).hexdigest() == projection["request_sha256"]
                assert projection["request_start_ms"] == attempt["request_start_ms"]
                assert projection["request_end_ms"] == attempt["request_end_ms"]
                assert projection["payment_committed_at_ms"] == attempt["decision_trusted_time_ms"]
            else:
                assert row["canonical_request_hex"] == ""
                assert projection["request_sha256"] is None
                assert projection["payment_committed_at_ms"] is None
    assert len(hashes) == 18


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="public JSON input for the ignored Rust exporter")
    parser.add_argument("--validate-candidate", type=Path, help="validate a native-exported candidate against current source inputs")
    args = parser.parse_args()
    output = args.output.resolve()
    if output == GOLDEN:
        parser.error("the input generator cannot replace the reviewed native-byte golden")
    source = build_input()
    payload = json.dumps(source, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(payload + "\n")
    print(f"Wrote source-derived Rust exporter input: {output}")
    if args.validate_candidate is not None:
        validate_candidate(source, json.loads(args.validate_candidate.read_text()))
        print(f"Validated native exporter candidate: {args.validate_candidate.resolve()}")


if __name__ == "__main__":
    main()
