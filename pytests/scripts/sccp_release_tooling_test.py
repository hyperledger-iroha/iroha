"""Adversarial tests for canonical SCCP V1 release evidence and bundles."""

from __future__ import annotations

import base64
import copy
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import sccp_release_bundle as builder  # noqa: E402
import sccp_release_common as common  # noqa: E402
import sccp_verify_release_bundle as verifier  # noqa: E402


FIXTURE = ROOT / "fixtures" / "sccp" / "release_evidence_v1"
FIXTURE_POLICY = FIXTURE / "test-trust-policy.json"
FIXTURE_EVIDENCE = FIXTURE / "evidence.json"
FIXTURE_RUNNER = SCRIPTS / "sccp_release_fixture.py"
PRODUCTION_CLIS = (
    SCRIPTS / "sccp_all_lanes_evidence.py",
    SCRIPTS / "sccp_release_bundle.py",
    SCRIPTS / "sccp_verify_release_bundle.py",
    SCRIPTS / "sccp_release_readiness_report.py",
)


def validator_path() -> Path:
    """Return the corridor-built production validator or skip integration checks."""

    configured = os.environ.get("SCCP_RELEASE_RUST_VALIDATOR")
    candidates = (
        Path(configured) if configured else None,
        ROOT / "target" / "debug" / "sccp_release_evidence",
        ROOT / "target" / "sccp-production-corridor" / "debug" / "sccp_release_evidence",
    )
    for candidate in candidates:
        if candidate is not None and candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    pytest.skip("production sccp_release_evidence validator has not been built")


def load_fixture_policy() -> tuple[dict[str, object], bytes]:
    """Load only the deliberately separate test policy schema."""

    return common.load_trust_policy(FIXTURE_POLICY, allow_test_policy=True)


def load_fixture_evidence() -> tuple[dict[str, object], bytes, dict[str, object], bytes]:
    """Load fixture policy and its externally signed evidence."""

    policy, policy_bytes = load_fixture_policy()
    evidence, evidence_bytes = common.load_evidence_file(FIXTURE_EVIDENCE, policy)
    return policy, policy_bytes, evidence, evidence_bytes


def write_json(path: Path, value: object) -> None:
    """Write the release canonical JSON form used by mutation tests."""

    path.write_bytes(common.canonical_json_file_bytes(value))


def mutated_policy(tmp_path: Path, mutation) -> Path:
    value = json.loads(FIXTURE_POLICY.read_text(encoding="utf-8"))
    mutation(value)
    path = tmp_path / "policy.json"
    write_json(path, value)
    return path


def mutated_evidence(tmp_path: Path, mutation) -> Path:
    value = json.loads(FIXTURE_EVIDENCE.read_text(encoding="utf-8"))
    mutation(value)
    path = tmp_path / "evidence.json"
    write_json(path, value)
    return path


def build_fixture_bundle(tmp_path: Path, name: str = "bundle") -> tuple[Path, dict[str, object]]:
    policy, policy_bytes, _, _ = load_fixture_evidence()
    output = tmp_path / name
    index = builder.build_bundle(
        FIXTURE_EVIDENCE,
        FIXTURE,
        output,
        FIXTURE_POLICY,
        policy,
        policy_bytes,
        validator_path(),
    )
    return output, index


def invoke_validator(artifact: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            str(validator_path()),
            "validate",
            str(artifact),
            str(FIXTURE_POLICY),
            str(FIXTURE_EVIDENCE),
            "test-fixture",
        ],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def invoke_release_validator(
    policy: Path, evidence: Path, environment: str = "test-fixture"
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            str(validator_path()),
            "validate-release",
            str(policy),
            str(evidence),
            environment,
        ],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def test_fixture_is_fully_valid_but_honestly_not_release_ready() -> None:
    policy, _, evidence, _ = load_fixture_evidence()
    artifacts = common.verify_evidence_artifacts(evidence, FIXTURE)
    receipts, executable_hash = common.verify_rust_lane_evidence(
        evidence,
        FIXTURE,
        validator_path(),
        policy,
        trust_policy_path=FIXTURE_POLICY,
        evidence_path=FIXTURE_EVIDENCE,
        environment="test-fixture",
    )
    assert len(artifacts) == len(common.REQUIRED_PHASES) + len(common.PROFILE_ORDER)
    assert len(receipts) == len(common.PROFILE_ORDER)
    assert receipts[0]["profile"] == "ethereum-mainnet"
    assert receipts[0]["inbound_status"] == "verified"
    assert all(receipt["outbound_status"] == "unavailable" for receipt in receipts)
    assert len(executable_hash) == 64
    summary = common.readiness_summary(evidence, bundle_root_hash=None)
    assert summary["ready"] is False
    assert len(summary["blocking_capabilities"]) >= 5


def test_rust_independently_verifies_release_and_auditor_signatures() -> None:
    result = invoke_release_validator(FIXTURE_POLICY, FIXTURE_EVIDENCE)
    assert result.returncode == 0, result.stderr
    receipt = json.loads(result.stdout)
    assert receipt["release_signatures_verified"] == 2
    assert receipt["circuit_audit_signatures_verified"] == 2 * len(common.PROFILE_ORDER)
    assert receipt["destination_attestors_validated"] == len(common.PROFILE_ORDER)


@pytest.mark.parametrize("case", ("release-replay", "audit-replay", "high-s", "small-order"))
def test_rust_release_trust_rejects_malformed_and_cross_role_replay(
    tmp_path: Path, case: str
) -> None:
    policy = json.loads(FIXTURE_POLICY.read_text(encoding="utf-8"))
    evidence = json.loads(FIXTURE_EVIDENCE.read_text(encoding="utf-8"))
    if case == "release-replay":
        evidence["provenance"][1]["signature_b64"] = evidence["provenance"][0][
            "signature_b64"
        ]
    elif case == "audit-replay":
        policy["proof_systems"][0]["audit_attestations"][1]["signature_b64"] = policy[
            "proof_systems"
        ][0]["audit_attestations"][0]["signature_b64"]
    elif case == "high-s":
        signature = bytearray(
            base64.b64decode(evidence["provenance"][0]["signature_b64"], validate=True)
        )
        signature[32:] = b"\xff" * 32
        evidence["provenance"][0]["signature_b64"] = base64.b64encode(signature).decode()
    else:
        policy["roles"][0]["public_key_hex"] = "01" + "00" * 31
        evidence["provenance"][0]["public_key_hex"] = "01" + "00" * 31
    policy_path = tmp_path / "policy.json"
    evidence_path = tmp_path / "evidence.json"
    write_json(policy_path, policy)
    write_json(evidence_path, evidence)
    result = invoke_release_validator(policy_path, evidence_path)
    assert result.returncode != 0
    assert result.stdout == ""


def test_bundle_is_deterministic_and_independently_verifiable(tmp_path: Path) -> None:
    first, first_index = build_fixture_bundle(tmp_path, "first")
    second, second_index = build_fixture_bundle(tmp_path, "second")
    assert first_index == second_index
    first_files = common.enumerate_direct_files(first)
    assert first_files == common.enumerate_direct_files(second)
    for relative in first_files:
        assert (first / relative).read_bytes() == (second / relative).read_bytes()
    policy, policy_bytes = load_fixture_policy()
    evidence, index = verifier.verify_bundle(
        first, FIXTURE_POLICY, policy, policy_bytes, validator_path()
    )
    assert evidence["release_id"] == index["release_id"]
    assert index["validator_executable_sha256_hex"] == hashlib.sha256(
        validator_path().read_bytes()
    ).hexdigest()


def test_fixture_cli_validates_builds_and_verifies_real_bundle(tmp_path: Path) -> None:
    bundle = tmp_path / "fixture-bundle"
    base = [sys.executable, str(FIXTURE_RUNNER), "--rust-validator", str(validator_path())]
    validate = subprocess.run(
        [*base, "validate"], cwd=ROOT, check=True, text=True, stdout=subprocess.PIPE
    )
    build = subprocess.run(
        [*base, "build", "--output-dir", str(bundle)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    verify = subprocess.run(
        [*base, "verify", str(bundle)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    for output in (validate.stdout, build.stdout, verify.stdout):
        value = json.loads(output)
        assert value["fixture_only"] is True
        assert value["ready"] is False


@pytest.mark.parametrize("script", PRODUCTION_CLIS)
def test_production_clis_cannot_accept_fixture_policy(script: Path, tmp_path: Path) -> None:
    if script.name == "sccp_verify_release_bundle.py":
        source, _ = build_fixture_bundle(tmp_path)
    else:
        source = FIXTURE_EVIDENCE
    command = [
        sys.executable,
        str(script),
        str(source),
        "--trust-policy",
        str(FIXTURE_POLICY),
        "--rust-validator",
        str(validator_path()),
    ]
    if script.name == "sccp_release_bundle.py":
        command.extend(("--output-dir", str(tmp_path / "production-output")))
    result = subprocess.run(
        command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    assert result.returncode == 1
    assert "schema/environment is not valid" in result.stderr


@pytest.mark.parametrize(
    "mutation",
    (
        lambda value: value.update(schema=common.TRUST_POLICY_SCHEMA),
        lambda value: value.update(environment="production"),
        lambda value: value.update(policy_id="rogue-policy"),
        lambda value: value["roles"].reverse(),
        lambda value: value["destination_attestors"].reverse(),
        lambda value: value["circuit_auditors"].reverse(),
        lambda value: value["proof_systems"].reverse(),
        lambda value: value["proof_systems"][0].update(
            counterparty_profile="solana-mainnet-beta"
        ),
        lambda value: value["destination_attestors"][0].update(
            counterparty_profile="ton-mainnet"
        ),
        lambda value: value["roles"][1].update(
            public_key_hex=value["roles"][0]["public_key_hex"]
        ),
        lambda value: value["roles"][0].update(
            signer_id=value["roles"][0]["public_key_hex"]
        ),
        lambda value: value["roles"][1].update(
            signer_id=value["roles"][0]["public_key_hex"]
        ),
        lambda value: value["destination_attestors"][0].update(
            public_key_hex=value["roles"][0]["public_key_hex"]
        ),
        lambda value: value["proof_systems"][0].update(
            semantics=["pairing-valid-v1", "sccp-exact-statement-v1"]
        ),
        lambda value: value["proof_systems"][0].update(circuit_id="smoke-circuit-v1"),
        lambda value: value["proof_systems"][0].update(route_revision=0),
        lambda value: value["proof_systems"][0].update(
            verifying_key_sha256_hex="00" * 32
        ),
        lambda value: value["proof_systems"][0].update(
            verifier_key_hash_hex=common.FORBIDDEN_ALGEBRAIC_SMOKE_VK
        ),
        lambda value: value["proof_systems"][0]["audit_attestations"][0].update(
            report_sha256_hex="11" * 32
        ),
        lambda value: value["proof_systems"][0]["destination_build"].update(
            token_runtime_hash_hex=value["proof_systems"][0]["destination_build"][
                "route_runtime_hash_hex"
            ]
        ),
        lambda value: value["proof_systems"][0]["destination_build"].update(
            token_artifact_sha256_hex=value["proof_systems"][0]["destination_build"][
                "route_artifact_sha256_hex"
            ]
        ),
    ),
)
def test_external_trust_policy_rejects_substitution_and_semantic_drift(
    tmp_path: Path, mutation
) -> None:
    path = mutated_policy(tmp_path, mutation)
    with pytest.raises(common.SccpReleaseError):
        common.load_trust_policy(path, allow_test_policy=True)


def test_production_loader_rejects_test_policy_without_override() -> None:
    with pytest.raises(common.SccpReleaseError, match="schema/environment"):
        common.load_trust_policy(FIXTURE_POLICY)


def test_policy_rejects_unknown_and_duplicate_json_keys(tmp_path: Path) -> None:
    raw = FIXTURE_POLICY.read_text(encoding="utf-8")
    unknown = json.loads(raw)
    unknown["allow_test_keys"] = True
    unknown_path = tmp_path / "unknown.json"
    write_json(unknown_path, unknown)
    with pytest.raises(common.SccpReleaseError, match="exact keys"):
        common.load_trust_policy(unknown_path, allow_test_policy=True)
    duplicate_path = tmp_path / "duplicate.json"
    duplicate_path.write_text(raw.replace('{', '{"schema":"duplicate",', 1))
    with pytest.raises(common.SccpReleaseError):
        common.load_trust_policy(duplicate_path, allow_test_policy=True)


@pytest.mark.parametrize(
    "mutation",
    (
        lambda value: value.update(release_id="tampered-release"),
        lambda value: value.update(trust_policy_id="rogue-policy"),
        lambda value: value["provenance"].reverse(),
        lambda value: value["provenance"][0].update(signer_id="rogue-signer"),
        lambda value: value["provenance"][0].update(public_key_hex="12" * 32),
        lambda value: value["lanes"][0].update(inbound_status="unavailable"),
        lambda value: value["lanes"][0].update(counterparty_domain=2),
        lambda value: value["validator"].update(source_sha256_hex="22" * 32),
        lambda value: value["validation"]["phases"][0].update(status="skipped"),
        lambda value: value["artifacts"].append(copy.deepcopy(value["artifacts"][0])),
    ),
)
def test_signed_evidence_rejects_tampering(tmp_path: Path, mutation) -> None:
    policy, _ = load_fixture_policy()
    path = mutated_evidence(tmp_path, mutation)
    with pytest.raises(common.SccpReleaseError):
        common.load_evidence_file(path, policy)


def test_evidence_rejects_noncanonical_encoding_duplicate_keys_and_nonfinite_numbers(
    tmp_path: Path,
) -> None:
    policy, _ = load_fixture_policy()
    value = json.loads(FIXTURE_EVIDENCE.read_text(encoding="utf-8"))
    pretty = tmp_path / "pretty.json"
    pretty.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    with pytest.raises(common.SccpReleaseError, match="canonical"):
        common.load_evidence_file(pretty, policy)
    duplicate = tmp_path / "duplicate.json"
    duplicate.write_text('{"schema":"a","schema":"b"}\n', encoding="utf-8")
    with pytest.raises(common.SccpReleaseError, match="duplicate"):
        common.load_evidence_file(duplicate, policy)
    nonfinite = tmp_path / "nan.json"
    nonfinite.write_text('{"value":NaN}\n', encoding="utf-8")
    with pytest.raises(common.SccpReleaseError, match="non-finite"):
        common.load_evidence_file(nonfinite, policy)


@pytest.mark.skipif(not hasattr(os, "link"), reason="hard links are not supported")
def test_policy_evidence_and_artifacts_reject_links(tmp_path: Path) -> None:
    policy_link = tmp_path / "policy-link.json"
    policy_link.symlink_to(FIXTURE_POLICY)
    with pytest.raises(common.SccpReleaseError, match="regular file"):
        common.load_trust_policy(policy_link, allow_test_policy=True)
    evidence_hardlink = tmp_path / "evidence-hardlink.json"
    os.link(FIXTURE_EVIDENCE, evidence_hardlink)
    policy, _ = load_fixture_policy()
    with pytest.raises(common.SccpReleaseError, match="hard-linked"):
        common.load_evidence_file(evidence_hardlink, policy)
    root = tmp_path / "artifact-root"
    shutil.copytree(FIXTURE, root)
    lane = root / "artifacts" / "lanes" / "ethereum-mainnet.json"
    replacement = tmp_path / "lane-copy.json"
    replacement.write_bytes(lane.read_bytes())
    lane.unlink()
    os.link(replacement, lane)
    evidence, _ = common.load_evidence_file(root / "evidence.json", policy)
    with pytest.raises(common.SccpReleaseError, match="hard-linked"):
        common.verify_evidence_artifacts(evidence, root)


def test_artifact_tamper_is_rejected_before_rust_validation(tmp_path: Path) -> None:
    root = tmp_path / "fixture"
    shutil.copytree(FIXTURE, root)
    artifact = root / "artifacts" / "lanes" / "ethereum-mainnet.json"
    artifact.write_bytes(artifact.read_bytes().replace(b"0", b"1", 1))
    policy, _ = load_fixture_policy()
    evidence, _ = common.load_evidence_file(root / "evidence.json", policy)
    with pytest.raises(common.SccpReleaseError, match="signed size and SHA-256"):
        common.verify_evidence_artifacts(evidence, root)


@pytest.mark.parametrize(
    "mutation", ("extra", "empty-dir", "artifact", "index", "policy", "validator")
)
def test_bundle_rejects_inventory_and_commitment_tampering(
    tmp_path: Path, mutation: str
) -> None:
    bundle, _ = build_fixture_bundle(tmp_path)
    policy, policy_bytes = load_fixture_policy()
    if mutation == "extra":
        (bundle / "extra.txt").write_text("not indexed\n", encoding="utf-8")
    elif mutation == "empty-dir":
        (bundle / "uncommitted-empty-directory").mkdir()
    elif mutation == "artifact":
        artifact = bundle / "artifacts" / "phases" / "rust-sccp.log"
        artifact.write_text("tampered\n", encoding="utf-8")
    else:
        index_path = bundle / "bundle.json"
        index = json.loads(index_path.read_text(encoding="utf-8"))
        if mutation == "index":
            index["bundle_root_hash_hex"] = "12" * 32
        elif mutation == "policy":
            index["trust_policy_sha256_hex"] = "13" * 32
        else:
            index["validator_executable_sha256_hex"] = "14" * 32
        write_json(index_path, index)
    with pytest.raises(common.SccpReleaseError):
        verifier.verify_bundle(
            bundle, FIXTURE_POLICY, policy, policy_bytes, validator_path()
        )


def test_bundle_rejects_symlink_and_hardlink_entries(tmp_path: Path) -> None:
    policy, policy_bytes = load_fixture_policy()
    symlink_bundle, _ = build_fixture_bundle(tmp_path, "symlink-bundle")
    artifact = symlink_bundle / "artifacts" / "phases" / "rust-sccp.log"
    content = artifact.read_bytes()
    artifact.unlink()
    target = tmp_path / "target.log"
    target.write_bytes(content)
    artifact.symlink_to(target)
    with pytest.raises(common.SccpReleaseError):
        verifier.verify_bundle(
            symlink_bundle, FIXTURE_POLICY, policy, policy_bytes, validator_path()
        )

    hardlink_bundle, _ = build_fixture_bundle(tmp_path, "hardlink-bundle")
    artifact = hardlink_bundle / "artifacts" / "phases" / "rust-sccp.log"
    target = tmp_path / "hard-target.log"
    target.write_bytes(artifact.read_bytes())
    artifact.unlink()
    os.link(target, artifact)
    with pytest.raises(common.SccpReleaseError):
        verifier.verify_bundle(
            hardlink_bundle, FIXTURE_POLICY, policy, policy_bytes, validator_path()
        )


def test_builder_refuses_existing_or_unsafe_output(tmp_path: Path) -> None:
    policy, policy_bytes, _, _ = load_fixture_evidence()
    existing = tmp_path / "existing"
    existing.mkdir()
    with pytest.raises(common.SccpReleaseError, match="never overwrites"):
        builder.build_bundle(
            FIXTURE_EVIDENCE,
            FIXTURE,
            existing,
            FIXTURE_POLICY,
            policy,
            policy_bytes,
            validator_path(),
        )
    with pytest.raises(common.SccpReleaseError, match="canonical artifact alphabet"):
        builder.build_bundle(
            FIXTURE_EVIDENCE,
            FIXTURE,
            tmp_path / "unsafe output",
            FIXTURE_POLICY,
            policy,
            policy_bytes,
            validator_path(),
        )


@pytest.mark.parametrize(
    "payload",
    (
        b"private_key=abc",
        b"client%255fsecret%253dabc",
        b"authorization&#58; bearer abc",
        base64.b64encode(b"client_secret=abc"),
    ),
)
def test_secret_scanner_rejects_plain_and_encoded_credentials(payload: bytes) -> None:
    with pytest.raises(common.SccpReleaseError, match="credential material"):
        common.reject_secret_material(payload, label="adversarial artifact")


def test_public_error_is_bounded_and_redacts_userinfo_and_secret_markers() -> None:
    error = ValueError("https://alice:password@example.test private_key=" + "x" * 5000)
    rendered = common.public_error(error)
    assert len(rendered.encode()) <= common.MAX_PUBLIC_ERROR_BYTES
    assert "alice:password" not in rendered
    assert "private_key" not in rendered.lower()


def test_strict_ed25519_verifier_accepts_rfc8032_and_rejects_malleability() -> None:
    public_key = bytes.fromhex(
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    )
    signature = bytes.fromhex(
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155"
        "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
    )
    assert common.verify_ed25519(public_key, signature, b"")
    assert not common.verify_ed25519(public_key, signature, b"x")
    malleable = signature[:32] + common._ED_L.to_bytes(32, "little")
    assert not common.verify_ed25519(public_key, malleable, b"")
    assert not common.verify_ed25519(b"\x01" + b"\x00" * 31, signature, b"")


def test_rust_validator_rejects_opaque_booleans_as_proof(tmp_path: Path) -> None:
    artifact = tmp_path / "forged.json"
    write_json(
        artifact,
        {
            "schema": "sccp-release-lane-evidence-v1",
            "version": 1,
            "profile": "ethereum-mainnet",
            "inbound": {
                "status": "available",
                "evidence": {"proof_valid": True, "finalized": True, "route_matches": True},
            },
            "outbound": {
                "status": "available",
                "evidence": {"runtime_matches": True, "bridge_immutable": True},
            },
        },
    )
    result = invoke_validator(artifact)
    assert result.returncode != 0
    assert result.stdout == ""
    assert len(result.stderr) < 4096


def test_rust_validator_rejects_mutated_native_proof_bytes(tmp_path: Path) -> None:
    source = FIXTURE / "artifacts" / "lanes" / "ethereum-mainnet.json"
    text = source.read_text(encoding="utf-8")
    marker = '"source_event_digest"'
    position = text.find(marker)
    assert position >= 0
    nibble = next(index for index in range(position, len(text)) if text[index] in "123456789abcdef")
    replacement = "0" if text[nibble] != "0" else "1"
    artifact = tmp_path / "mutated.json"
    artifact.write_text(text[:nibble] + replacement + text[nibble + 1 :], encoding="utf-8")
    result = invoke_validator(artifact)
    assert result.returncode != 0
    assert result.stdout == ""


def test_rust_validator_rejects_noncanonical_lane_json(tmp_path: Path) -> None:
    source = FIXTURE / "artifacts" / "lanes" / "bsc-mainnet.json"
    value = json.loads(source.read_text(encoding="utf-8"))
    pretty = tmp_path / "pretty.json"
    pretty.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    assert invoke_validator(pretty).returncode != 0
    duplicate = tmp_path / "duplicate.json"
    raw = source.read_text(encoding="utf-8")
    duplicate.write_text(raw.replace("{", '{"schema":"duplicate",', 1), encoding="utf-8")
    assert invoke_validator(duplicate).returncode != 0


def test_validator_substitution_and_output_flood_fail_closed(tmp_path: Path, monkeypatch) -> None:
    policy, _, evidence, _ = load_fixture_evidence()
    flood = tmp_path / "flood"
    flood.write_text("#!/bin/sh\nyes x\n", encoding="utf-8")
    flood.chmod(flood.stat().st_mode | stat.S_IXUSR)
    monkeypatch.setattr(common, "MAX_VALIDATOR_SECONDS", 1)
    monkeypatch.setattr(common, "MAX_VALIDATOR_OUTPUT_BYTES", 128)
    with pytest.raises(common.SccpReleaseError, match="output limit|time limit"):
        common.verify_rust_lane_evidence(
            evidence,
            FIXTURE,
            flood,
            policy,
            trust_policy_path=FIXTURE_POLICY,
            evidence_path=FIXTURE_EVIDENCE,
            environment="test-fixture",
        )


def test_python_release_path_contains_no_protocol_hash_reimplementation() -> None:
    source = (SCRIPTS / "sccp_release_common.py").read_text(encoding="utf-8")
    for forbidden in (
        "def canonical_lane",
        "def source_identity_hash",
        "def keccak256",
        "sccp_exact_evm_xor_route_config_hash_v1(",
        "proof_valid",
        "allow_unready",
    ):
        assert forbidden not in source


def test_lane_validator_receives_complete_signed_context_not_trust_projections() -> None:
    source = (SCRIPTS / "sccp_release_common.py").read_text(encoding="utf-8")
    start = source.index("def _invoke_lane_validator(")
    end = source.index("\ndef verify_rust_release_signatures(", start)
    invocation = source[start:end]
    assert "trust_policy_path" in invocation
    assert "evidence_path" in invocation
    assert "environment" in invocation
    for forbidden in ("proof_system", "attestor_id", "public_key_hex", "runtime_hash"):
        assert forbidden not in invocation


def test_production_entrypoints_expose_no_test_policy_or_signing_switch() -> None:
    for path in PRODUCTION_CLIS:
        source = path.read_text(encoding="utf-8")
        assert "allow_test_policy=True" not in source
        assert "private_key" not in source
        assert "--sign" not in source
    fixture_source = FIXTURE_RUNNER.read_text(encoding="utf-8")
    assert "allow_test_policy=True" in fixture_source
    assert "FIXTURE_RELEASE_ID" in fixture_source
    assert "FIXTURE_POLICY_ID" in fixture_source
