from __future__ import annotations

import base64
import hashlib
import json
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import release_artifact_contract as artifacts
import taira_privacy_protocol_receipt as evidence


SOURCE = {
    "cargo_lock_sha256": "33" * 32,
    "commit": "11" * 20,
    "dpn_validator_release_commit": "22" * 20,
    "workspace_source_manifest_sha256": "44" * 32,
}
BINDINGS = {
    "artifact_handoff_sha256": "55" * 32,
    "exact12_matrix_sha256": "66" * 32,
    "linux_release_archive_sha256": "77" * 32,
    "validator_binary_sha256": "88" * 32,
}
DRIVER_BYTES = {
    evidence.ACTION_DRIVER: b"native-action-driver-v1\0",
    evidence.JINDO_DRIVER: b"native-jindo-test-driver-v1\0",
    evidence.NETWORK_DRIVER: b"native-network-test-driver-v1\0",
}
DRIVERS = {
    name: hashlib.sha256(payload).hexdigest()
    for name, payload in DRIVER_BYTES.items()
}
NOW = 1_900_000_000


def _write(path: Path, value: object) -> bytes:
    payload = artifacts.canonical_json_bytes(value)
    path.write_bytes(payload)
    return payload


def _network_output(case: str) -> bytes:
    return (
        f"running 1 test\n"
        f"test {case} ... ok\n\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
        "100 filtered out; finished in 1.00s\n"
    ).encode()


def _jindo_output() -> bytes:
    return (
        "running 1 test\n"
        "test privacy_engines::jindo::security::tests::release_boundary ... ok\n\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
        "100 filtered out; finished in 0.01s\n"
    ).encode()


def build_valid_evidence(
    root: Path,
    *,
    source: dict[str, str] = SOURCE,
    bindings: dict[str, str] = BINDINGS,
    drivers: dict[str, str] = DRIVERS,
    now_unix: int = NOW,
) -> str:
    root.mkdir()
    for name, payload in DRIVER_BYTES.items():
        path = root / evidence.DRIVER_EVIDENCE_NAMES[name]
        path.write_bytes(payload)
        path.chmod(0o600)
    candidate = {
        "artifact_handoff_sha256": bindings["artifact_handoff_sha256"],
        "drivers": drivers,
        "exact12_matrix_sha256": bindings["exact12_matrix_sha256"],
        "linux_release_archive_sha256": bindings[
            "linux_release_archive_sha256"
        ],
        "source": source,
        "validator_binary_sha256": bindings["validator_binary_sha256"],
    }
    candidate_id = evidence.compute_candidate_binding_sha256(candidate)
    case_rows = []
    for index, (case, kind) in enumerate(evidence.CASE_DEFINITIONS):
        commands = []
        for command_index, (driver, args) in enumerate(evidence.command_plan(index)):
            output = _network_output(case) if driver == evidence.NETWORK_DRIVER else _jindo_output()
            commands.append(
                {
                    "args": list(args),
                    "driver": driver,
                    "driver_sha256": drivers[driver],
                    "exit_code": 0,
                    "index": command_index,
                    "output_base64": base64.b64encode(output).decode(),
                    "output_sha256": hashlib.sha256(output).hexdigest(),
                    "output_size": len(output),
                }
            )
        transcript_body = {
            "candidate_binding_sha256": candidate_id,
            "case": case,
            "commands": commands,
            "index": index,
            "kind": kind,
            "schema": evidence.TRANSCRIPT_SCHEMA,
            "schema_version": evidence.TRANSCRIPT_SCHEMA_VERSION,
        }
        transcript_id = evidence.compute_transcript_id(transcript_body)
        transcript_payload = _write(
            root / evidence.transcript_name(index),
            {**transcript_body, "transcript_id": transcript_id},
        )
        result_body = {
            "candidate_binding_sha256": candidate_id,
            "case": case,
            "index": index,
            "kind": kind,
            "schema": evidence.RESULT_SCHEMA,
            "schema_version": evidence.RESULT_SCHEMA_VERSION,
            "status": "passed",
            "transcript_id": transcript_id,
            "transcript_path": evidence.transcript_name(index),
            "transcript_sha256": hashlib.sha256(transcript_payload).hexdigest(),
            "transcript_size": len(transcript_payload),
        }
        result_id = evidence.compute_result_id(result_body)
        result_payload = _write(
            root / evidence.result_name(index),
            {**result_body, "result_id": result_id},
        )
        case_rows.append(
            {
                "case": case,
                "index": index,
                "kind": kind,
                "result_id": result_id,
                "result_path": evidence.result_name(index),
                "result_sha256": hashlib.sha256(result_payload).hexdigest(),
                "result_size": len(result_payload),
                "transcript_id": transcript_id,
                "transcript_path": evidence.transcript_name(index),
                "transcript_sha256": hashlib.sha256(transcript_payload).hexdigest(),
                "transcript_size": len(transcript_payload),
            }
        )
    receipt_body = {
        "candidate": candidate,
        "cases": case_rows,
        "expires_at_unix": now_unix + 3600,
        "issued_at_unix": now_unix,
        "outcomes": [
            {
                "case_index": case_index,
                "closed_reason": closed_reason,
                "index": index,
                "production_outcome": production_outcome,
                "profile": profile,
                "protocol": protocol,
                "security_boundary": security_boundary,
            }
            for index, (
                protocol,
                case_index,
                profile,
                production_outcome,
                closed_reason,
                security_boundary,
            ) in enumerate(evidence.OUTCOMES)
        ],
        "platform": {"arch": "arm64", "os": "macos", "peer_count": 4},
        "schema": evidence.RECEIPT_SCHEMA,
        "schema_version": evidence.RECEIPT_SCHEMA_VERSION,
    }
    receipt_id = evidence.compute_receipt_id(receipt_body)
    _write(root / evidence.RECEIPT_NAME, {**receipt_body, "receipt_id": receipt_id})
    return receipt_id


def _load(path: Path) -> dict[str, object]:
    value = json.loads(path.read_bytes())
    assert isinstance(value, dict)
    return value


def _rewrite_receipt(root: Path, receipt: dict[str, object]) -> None:
    body = dict(receipt)
    body.pop("receipt_id", None)
    receipt["receipt_id"] = evidence.compute_receipt_id(body)
    _write(root / evidence.RECEIPT_NAME, receipt)


def _rechain_case(root: Path, index: int) -> None:
    transcript_path = root / evidence.transcript_name(index)
    transcript = _load(transcript_path)
    transcript_body = dict(transcript)
    transcript_body.pop("transcript_id", None)
    transcript_id = evidence.compute_transcript_id(transcript_body)
    transcript["transcript_id"] = transcript_id
    transcript_payload = _write(transcript_path, transcript)

    result_path = root / evidence.result_name(index)
    result = _load(result_path)
    result.update(
        {
            "transcript_id": transcript_id,
            "transcript_sha256": hashlib.sha256(transcript_payload).hexdigest(),
            "transcript_size": len(transcript_payload),
        }
    )
    result_body = dict(result)
    result_body.pop("result_id", None)
    result_id = evidence.compute_result_id(result_body)
    result["result_id"] = result_id
    result_payload = _write(result_path, result)

    receipt = _load(root / evidence.RECEIPT_NAME)
    cases = receipt["cases"]
    assert isinstance(cases, list) and isinstance(cases[index], dict)
    cases[index].update(
        {
            "result_id": result_id,
            "result_sha256": hashlib.sha256(result_payload).hexdigest(),
            "result_size": len(result_payload),
            "transcript_id": transcript_id,
            "transcript_sha256": hashlib.sha256(transcript_payload).hexdigest(),
            "transcript_size": len(transcript_payload),
        }
    )
    _rewrite_receipt(root, receipt)


def validate(root: Path, receipt_id: str | None = None) -> dict[str, object]:
    if receipt_id is None:
        receipt_id = str(_load(root / evidence.RECEIPT_NAME)["receipt_id"])
    # Unsigned v2 parsing is intentionally structural-only.  Release callers
    # use validate_evidence_directory(), which is closed on independent
    # controller-origin authentication.
    return evidence.validate_unsigned_v2_structure(
        root,
        expected_source=SOURCE,
        expected_validator_binary_sha256=BINDINGS["validator_binary_sha256"],
        expected_linux_release_archive_sha256=BINDINGS[
            "linux_release_archive_sha256"
        ],
        expected_exact12_matrix_sha256=BINDINGS["exact12_matrix_sha256"],
        expected_artifact_handoff_sha256=BINDINGS["artifact_handoff_sha256"],
        expected_receipt_id=receipt_id,
        now_unix=NOW,
    )


def test_unsigned_v2_structure_preserves_six_protocol_cases_and_governance(
    tmp_path: Path,
) -> None:
    root = tmp_path / "evidence"
    receipt_id = build_valid_evidence(root)
    result = validate(root, receipt_id)
    assert result["case_count"] == 7
    assert result["drivers"] == DRIVERS
    assert len(result["outcomes"]) == 12
    assert set(artifacts.scan_inventory_paths(root)) == set(evidence.EVIDENCE_NAMES)
    assert all(row["profile"] != "engine-unavailable" for row in result["outcomes"])

    # These bytes are deliberately synthesized libtest text with recomputed
    # self-hashes.  Structural consistency must never promote them to release
    # authority, even though every legacy v2 relationship is internally valid.
    with pytest.raises(
        evidence.PrivacyProtocolEvidenceError,
        match=evidence.CONTROLLER_ORIGIN_AUTHORITY_CONTRACT,
    ):
        evidence.validate_evidence_directory(
            root,
            expected_source=SOURCE,
            expected_validator_binary_sha256=BINDINGS[
                "validator_binary_sha256"
            ],
            expected_linux_release_archive_sha256=BINDINGS[
                "linux_release_archive_sha256"
            ],
            expected_exact12_matrix_sha256=BINDINGS[
                "exact12_matrix_sha256"
            ],
            expected_artifact_handoff_sha256=BINDINGS[
                "artifact_handoff_sha256"
            ],
            expected_receipt_id=receipt_id,
            now_unix=NOW,
        )


def test_v1_receipt_is_rejected_without_compatibility(tmp_path: Path) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    receipt = _load(root / evidence.RECEIPT_NAME)
    receipt["schema_version"] = 1
    _rewrite_receipt(root, receipt)
    with pytest.raises(evidence.PrivacyProtocolEvidenceError, match="v1 is rejected"):
        validate(root)


def test_fabricated_transcript_id_is_rejected(tmp_path: Path) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    transcript = _load(root / evidence.transcript_name(0))
    transcript["transcript_id"] = "ff" * 32
    _write(root / evidence.transcript_name(0), transcript)
    with pytest.raises(evidence.PrivacyProtocolEvidenceError):
        validate(root)


@pytest.mark.parametrize("mutation", ["duplicate", "reordered", "missing", "truncated"])
def test_missing_duplicate_reordered_or_truncated_transcripts_fail_closed(
    tmp_path: Path, mutation: str
) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    receipt = _load(root / evidence.RECEIPT_NAME)
    cases = receipt["cases"]
    assert isinstance(cases, list)
    if mutation == "duplicate":
        cases[1] = dict(cases[0])
        _rewrite_receipt(root, receipt)
    elif mutation == "reordered":
        cases[0], cases[1] = cases[1], cases[0]
        _rewrite_receipt(root, receipt)
    elif mutation == "missing":
        (root / evidence.transcript_name(0)).unlink()
    else:
        path = root / evidence.transcript_name(0)
        path.write_bytes(path.read_bytes()[:-1])
    with pytest.raises(evidence.PrivacyProtocolEvidenceError):
        validate(root)


def test_unavailable_outcome_substitution_is_rejected_even_with_new_receipt_id(
    tmp_path: Path,
) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    receipt = _load(root / evidence.RECEIPT_NAME)
    outcomes = receipt["outcomes"]
    assert isinstance(outcomes, list) and isinstance(outcomes[3], dict)
    outcomes[3]["profile"] = "engine-unavailable"
    outcomes[3]["production_outcome"] = "governance-and-action-rejected"
    _rewrite_receipt(root, receipt)
    with pytest.raises(evidence.PrivacyProtocolEvidenceError, match="unavailable"):
        validate(root)


@pytest.mark.parametrize(
    "field",
    [
        "artifact_handoff_sha256",
        "exact12_matrix_sha256",
        "linux_release_archive_sha256",
        "validator_binary_sha256",
    ],
)
def test_stale_candidate_bindings_are_rejected(tmp_path: Path, field: str) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    receipt = _load(root / evidence.RECEIPT_NAME)
    candidate = receipt["candidate"]
    assert isinstance(candidate, dict)
    candidate[field] = "ee" * 32
    _rewrite_receipt(root, receipt)
    with pytest.raises(evidence.PrivacyProtocolEvidenceError, match="stale"):
        validate(root)


def test_marker_only_lying_helper_is_rejected_after_all_ids_are_recomputed(
    tmp_path: Path,
) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    transcript = _load(root / evidence.transcript_name(0))
    commands = transcript["commands"]
    assert isinstance(commands, list) and isinstance(commands[0], dict)
    case = evidence.CASE_DEFINITIONS[0][0]
    output = (
        f"TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:{case}:passed\n"
        "running 1 test\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
        "0 filtered out; finished in 0.00s\n"
    ).encode()
    commands[0].update(
        {
            "output_base64": base64.b64encode(output).decode(),
            "output_sha256": hashlib.sha256(output).hexdigest(),
            "output_size": len(output),
        }
    )
    _write(root / evidence.transcript_name(0), transcript)
    _rechain_case(root, 0)
    with pytest.raises(evidence.PrivacyProtocolEvidenceError, match="libtest pass"):
        validate(root)


def test_transcript_cannot_substitute_a_different_native_driver(
    tmp_path: Path,
) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    transcript = _load(root / evidence.transcript_name(0))
    commands = transcript["commands"]
    assert isinstance(commands, list) and isinstance(commands[0], dict)
    commands[0]["driver_sha256"] = "ee" * 32
    _write(root / evidence.transcript_name(0), transcript)
    _rechain_case(root, 0)
    with pytest.raises(evidence.PrivacyProtocolEvidenceError, match="different driver"):
        validate(root)


def test_preserved_native_driver_bytes_cannot_be_substituted(
    tmp_path: Path,
) -> None:
    root = tmp_path / "evidence"
    build_valid_evidence(root)
    path = root / evidence.DRIVER_EVIDENCE_NAMES[evidence.ACTION_DRIVER]
    path.write_bytes(b"different-action-driver\0")
    with pytest.raises(
        evidence.PrivacyProtocolEvidenceError, match="preserved.*bytes differ"
    ):
        validate(root)
