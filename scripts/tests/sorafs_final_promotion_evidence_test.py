"""Exact-byte adapter tests; mocked native outcomes are not custody qualification."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

import pytest

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))
import sorafs_final_promotion_evidence as module


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


@pytest.fixture
def inputs(tmp_path):
    root = tmp_path.resolve(strict=True)
    fixture = SCRIPT_DIR.parent / "crates/sorafs_manifest/src/signer/final_promotion/tests/statement_fixture.message"
    statement = fixture.read_bytes()
    payload = json.loads(statement.split(b"\0", 1)[1])
    payload["authentication"]["signature_hex"] = "72" * 64
    args = argparse.Namespace(
        provenance_receipt_verifier=root / "reviewed-iroha",
        provenance_signer_policy=root / "policy",
        provenance_custody_trust=root / "trust",
        provenance_completed_operation_state=root / "state",
        provenance_operation_receipt=root / "receipt",
        provenance_signer_policy_digest_hex="41" * 32,
        provenance_signer_key_revision=7,
        provenance_signer_policy_revision=9,
        provenance_signer_service_id="promotion-primary",
        provenance_signer_administrator_id="promotion-security-primary",
        provenance_deployment_id="production-primary",
        provenance_chain_id="promotion-chain",
        provenance_network_id_hex="11" * 32,
        now_unix=120,
    )
    for key in ("signer_policy", "custody_trust", "completed_operation_state", "operation_receipt"):
        path = getattr(args, f"provenance_{key}")
        path.write_bytes(f"synthetic adapter input: {key}".encode("ascii"))
        path.chmod(0o400)
    args.provenance_receipt_verifier.write_bytes(b"synthetic pinned executable, never executed")
    args.provenance_receipt_verifier.chmod(0o500)
    args.provenance_receipt_verifier_sha256 = digest(args.provenance_receipt_verifier.read_bytes())
    args.provenance_signer_policy_sha256 = digest(args.provenance_signer_policy.read_bytes())
    args.provenance_custody_trust_sha256 = digest(args.provenance_custody_trust.read_bytes())
    return args, payload, bytes([0x21] * 32), statement


def result_for(command, args):
    flags = dict(zip(command[5::2], command[6::2], strict=True))
    documents = {flag: Path(flags[f"--{flag}"]).read_bytes() for flag in (
        "statement", "signature", "public-key", "signer-policy", "custody-trust",
        "completed-operation-state", "operation-receipt",
    )}
    return {
        "schema": "sorafs.final_promotion_receipt_verification.v1",
        "status": "verified", "verification_scope": "final_promotion_signer_receipt",
        "statement_sha256": digest(documents["statement"]),
        "statement_size": len(documents["statement"]),
        "signature_sha256": digest(documents["signature"]),
        "public_key_fingerprint_sha256": digest(documents["public-key"]),
        "signer_policy_sha256": digest(documents["signer-policy"]),
        "custody_trust_sha256": digest(documents["custody-trust"]),
        "completed_operation_state_sha256": digest(documents["completed-operation-state"]),
        "operation_receipt_sha256": digest(documents["operation-receipt"]),
        "operation_id": "12" * 32, "custody_record_digest": "13" * 32,
        "policy_digest": args.provenance_signer_policy_digest_hex,
        "key_revision": args.provenance_signer_key_revision,
        "policy_revision": args.provenance_signer_policy_revision,
        "service_id": args.provenance_signer_service_id,
        "administrator_id": args.provenance_signer_administrator_id,
        "role": "final_promotion_provenance",
        "deployment_id": args.provenance_deployment_id, "chain_id": args.provenance_chain_id,
        "network_id": args.provenance_network_id_hex,
        "finalized_height": 100, "finalized_block_hash": "14" * 32,
        "verified_at_unix_ms": args.now_unix * 1000,
    }


def install_result(monkeypatch, args, mutation=lambda result: None):
    calls = []

    def run(command, root, *, max_stdout_bytes, expected_stderr):
        calls.append((command, root))
        assert command[1:5] == ["app", "sorafs", "toolkit", "final-promotion-receipt"]
        assert max_stdout_bytes == module.MAX_VALIDATION_BYTES
        assert expected_stderr == b""
        result = result_for(command, args)
        mutation(result)
        return (json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n").encode()

    monkeypatch.setattr(module.verifier_process, "run_verifier", run)
    return calls


def test_adapter_captures_exact_reviewed_inputs_and_cleans_private_copy(inputs, monkeypatch):
    args, payload, public_key, statement = inputs
    calls = install_result(monkeypatch, args)
    original = module.verifier_process.run_verifier

    def inspect(command, root, *, max_stdout_bytes, expected_stderr):
        assert expected_stderr == b""
        assert (root / "statement").read_bytes() == statement
        assert (root / "signature").read_bytes() == bytes.fromhex(payload["authentication"]["signature_hex"])
        assert (root / "public-key").read_bytes() == public_key
        assert (root / "iroha").read_bytes() == args.provenance_receipt_verifier.read_bytes()
        # A later source change cannot change the bytes consumed by the verifier.
        args.provenance_operation_receipt.chmod(0o600)
        args.provenance_operation_receipt.write_bytes(b"replaced after private snapshot")
        return original(
            command, root, max_stdout_bytes=max_stdout_bytes, expected_stderr=expected_stderr,
        )

    monkeypatch.setattr(module.verifier_process, "run_verifier", inspect)
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == []
    assert len(calls) == 1
    assert not calls[0][1].exists()


@pytest.mark.parametrize("field", sorted(module.VALIDATION_FIELDS))
def test_adapter_rejects_each_substituted_native_claim(inputs, monkeypatch, field):
    args, payload, public_key, statement = inputs
    calls = install_result(monkeypatch, args, lambda result: result.__setitem__(field, None))
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]
    assert len(calls) == 1


@pytest.mark.parametrize("field", ["statement_size", "key_revision", "policy_revision", "finalized_height", "verified_at_unix_ms"])
def test_adapter_rejects_boolean_integer_claims(inputs, monkeypatch, field):
    args, payload, public_key, statement = inputs
    install_result(monkeypatch, args, lambda result: result.__setitem__(field, True))
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]


@pytest.mark.parametrize("field", ["provenance_receipt_verifier_sha256", "provenance_signer_policy_sha256", "provenance_custody_trust_sha256"])
def test_adapter_rejects_wrong_independent_pin_before_execution(inputs, monkeypatch, field):
    args, payload, public_key, statement = inputs
    setattr(args, field, "ab" * 32)
    calls = install_result(monkeypatch, args)
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("clock", [0, -1, True, "120", 1 << 64, ((1 << 64) - 1) // 1000 + 1])
def test_adapter_rejects_invalid_independent_time_before_execution(inputs, monkeypatch, clock):
    args, payload, public_key, statement = inputs
    args.now_unix = clock
    calls = install_result(monkeypatch, args)
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("raw", [b"", b"{}", b"[]", b'{"status":"verified","status":"verified"}', b"x" * (module.MAX_VALIDATION_BYTES + 1)])
def test_adapter_rejects_malformed_native_result_without_leaking_output(inputs, monkeypatch, raw):
    args, payload, public_key, statement = inputs
    def malformed_result(command, root, *, max_stdout_bytes, expected_stderr):
        assert expected_stderr == b""
        return raw

    monkeypatch.setattr(module.verifier_process, "run_verifier", malformed_result)
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]


def test_adapter_rejects_extra_native_fields(inputs, monkeypatch):
    args, payload, public_key, statement = inputs
    install_result(monkeypatch, args, lambda result: result.update(promotion_eligible=True))
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]


@pytest.mark.parametrize("field", ["provenance_signer_policy", "provenance_custody_trust", "provenance_completed_operation_state", "provenance_operation_receipt"])
def test_adapter_rejects_oversized_and_unsafe_receipt_files(inputs, monkeypatch, field):
    args, payload, public_key, statement = inputs
    path = getattr(args, field)
    path.chmod(0o600)
    with path.open("wb") as stream:
        stream.truncate(module.MAX_DOCUMENT_BYTES + 1)
    calls = install_result(monkeypatch, args)
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]
    path.unlink()
    path.symlink_to(args.provenance_receipt_verifier)
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]
    assert calls == []


def test_adapter_does_not_forward_process_diagnostics(inputs, monkeypatch):
    args, payload, public_key, statement = inputs

    def failure(command, root, *, max_stdout_bytes, expected_stderr):
        assert expected_stderr == b""
        raise ValueError("candidate controlled secret text")

    monkeypatch.setattr(module.verifier_process, "run_verifier", failure)
    assert module.verify_final_promotion_receipt(args, statement, bytes.fromhex(payload["authentication"]["signature_hex"]), public_key) == [module.FAILURE]


@pytest.mark.parametrize("signature", [b"", bytes(64), b"s" * 63, b"s" * 65, "72" * 64, bytearray(b"s" * 64)])
def test_adapter_requires_exact_raw_signature_before_execution(inputs, monkeypatch, signature):
    args, _payload, public_key, statement = inputs
    calls = install_result(monkeypatch, args)
    assert module.verify_final_promotion_receipt(args, statement, signature, public_key) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("statement", [b"", "canonical bytes required", bytearray(b"message"), b"m" * (module.MAX_STATEMENT_BYTES + 1)])
def test_adapter_requires_bounded_immutable_statement_before_execution(inputs, monkeypatch, statement):
    args, payload, public_key, _statement = inputs
    calls = install_result(monkeypatch, args)
    signature = bytes.fromhex(payload["authentication"]["signature_hex"])
    assert module.verify_final_promotion_receipt(args, statement, signature, public_key) == [module.FAILURE]
    assert calls == []


def test_adapter_rejects_unresolved_private_directory(inputs, monkeypatch):
    args, payload, public_key, statement = inputs
    calls = install_result(monkeypatch, args)
    monkeypatch.setattr(module, "resolve_path_identity", lambda *args: None)
    signature = bytes.fromhex(payload["authentication"]["signature_hex"])
    assert module.verify_final_promotion_receipt(args, statement, signature, public_key) == [module.FAILURE]
    assert calls == []
