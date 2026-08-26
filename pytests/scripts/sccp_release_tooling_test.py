"""Adversarial tests for canonical SCCP V1 release evidence and bundles."""

from __future__ import annotations

import ast
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

CORRIDOR_PHASES = (
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "tvm-contract-smoke",
    "core-admission",
    "runtime-api",
)


def test_release_evidence_requires_every_production_corridor_phase() -> None:
    """Keep signed evidence inventory identical to the executable corridor."""

    listed = subprocess.run(
        ["bash", str(ROOT / "scripts" / "check_sccp_production_corridor.sh"), "--list"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert tuple(line.strip() for line in listed.stdout.splitlines()[1:]) == CORRIDOR_PHASES
    assert common.REQUIRED_PHASES == CORRIDOR_PHASES


def validator_path() -> Path:
    """Return the corridor-built production validator or skip integration checks."""

    configured = os.environ.get("SCCP_RELEASE_RUST_VALIDATOR")
    if configured:
        candidate = Path(configured)
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
        pytest.skip("configured production sccp_release_evidence validator is unavailable")

    expected_hash = json.loads(FIXTURE_EVIDENCE.read_text(encoding="utf-8"))["validator"][
        "executable_sha256_hex"
    ]
    candidates = (
        ROOT / "target" / "sccp-production-corridor" / "debug" / "sccp_release_evidence",
        ROOT / "target" / "debug" / "sccp_release_evidence",
    )
    for candidate in candidates:
        if (
            candidate.is_file()
            and os.access(candidate, os.X_OK)
            and hashlib.sha256(candidate.read_bytes()).hexdigest() == expected_hash
        ):
            return candidate
    pytest.skip(
        "the exact signed production sccp_release_evidence validator has not been built"
    )


def _unit_v4_hash(*parts: str) -> str:
    """Derive one deterministic, role-separated unit-test digest."""

    label = ":".join(("sccp-v4-unit", *parts)).encode("ascii")
    return hashlib.sha256(label).hexdigest()


def _unit_v4_keypair(label: str) -> tuple[bytes, bytes, int]:
    """Derive a deterministic Ed25519 key used only inside this test process."""

    entropy = hashlib.sha256(f"sccp-v4-unit-key:{label}".encode("ascii")).digest()
    digest = hashlib.sha512(entropy).digest()
    scalar_bytes = bytearray(digest[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, scalar))
    return public, digest[32:], scalar


def _unit_v4_sign(keypair: tuple[bytes, bytes, int], message: bytes) -> str:
    """Sign one unit-test payload without persisting private material."""

    public, prefix, scalar = keypair
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % common._ED_L
    encoded_r = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, nonce))
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public + message).digest(), "little"
    ) % common._ED_L
    encoded_s = ((nonce + challenge * scalar) % common._ED_L).to_bytes(32, "little")
    signature = encoded_r + encoded_s
    assert common.verify_ed25519(public, signature, message)
    return base64.b64encode(signature).decode("ascii")


_UNIT_V4_POLICY_CACHE = None


def unit_v4_policy() -> tuple[
    dict[str, object],
    bytes,
    dict[str, tuple[bytes, bytes, int]],
]:
    """Return a fresh copy of a valid, test-schema-only protocol-v4 policy."""

    global _UNIT_V4_POLICY_CACHE
    if _UNIT_V4_POLICY_CACHE is None:
        release_keys = {
            role: _unit_v4_keypair(f"release:{role}")
            for role in common.PROVENANCE_ROLES
        }
        attestor_keys = {
            profile: _unit_v4_keypair(f"attestor:{profile}")
            for profile in common.PROFILE_ORDER
        }
        auditor_keys = {
            role: _unit_v4_keypair(f"auditor:{role}")
            for role in common.CIRCUIT_AUDITOR_ROLES
        }
        policy: dict[str, object] = {
            "schema": common.TEST_TRUST_POLICY_SCHEMA,
            "environment": "test-fixture",
            "policy_id": "sccp-v4-ephemeral-unit-policy-v1",
            "roles": [
                {
                    "role": role,
                    "signer_id": f"unit-v4-{role}",
                    "public_key_hex": release_keys[role][0].hex(),
                }
                for role in common.PROVENANCE_ROLES
            ],
            "destination_attestors": [
                {
                    "counterparty_profile": profile,
                    "attestor_id": f"unit-v4-{profile}-attestor",
                    "public_key_hex": attestor_keys[profile][0].hex(),
                }
                for profile in common.PROFILE_ORDER
            ],
            "circuit_auditors": [
                {
                    "role": role,
                    "auditor_id": f"unit-v4-{role}-auditor",
                    "public_key_hex": auditor_keys[role][0].hex(),
                }
                for role in common.CIRCUIT_AUDITOR_ROLES
            ],
            "proof_systems": [],
        }
        destination_fields = (
            "source_bundle_sha256_hex",
            "compiler_build_sha256_hex",
            "token_artifact_sha256_hex",
            "token_interface_sha256_hex",
            "token_runtime_hash_hex",
            "verifier_artifact_sha256_hex",
            "verifier_interface_sha256_hex",
            "verifier_runtime_hash_hex",
            "route_artifact_sha256_hex",
            "route_interface_sha256_hex",
            "route_runtime_hash_hex",
        )
        proof_systems: list[dict[str, object]] = []
        for index, profile in enumerate(common.PROFILE_ORDER):
            circuit_artifact = bytes.fromhex(_unit_v4_hash(profile, "circuit-artifact"))
            witness_generator = bytes.fromhex(_unit_v4_hash(profile, "witness-generator"))
            public_signal_schema = bytes.fromhex(common.PUBLIC_SIGNAL_SCHEMA_HASH_HEX)
            anchor: dict[str, object] = {
                "version": 1,
                "source_profile": "sora-taira",
                "protocol_version": common.SORA_TAIRA_SUMERAGI_PROTOCOL_VERSION,
                "chain_id_hash_hex": common.SORA_TAIRA_CHAIN_ID_HASH_HEX,
                "checkpoint_height": 10_000 + index,
                "checkpoint_block_hash_hex": _unit_v4_hash(profile, "checkpoint-block"),
                "checkpoint_context_id_hex": _unit_v4_hash(profile, "checkpoint-context"),
                "checkpoint_finality_artifact_hash_hex": _unit_v4_hash(
                    profile, "checkpoint-finality-artifact"
                ),
            }
            proof: dict[str, object] = {
                "counterparty_profile": profile,
                "circuit_id": common.RELEASE_CIRCUIT_IDS[index],
                "semantics": list(common.REQUIRED_SEMANTICS),
                "circuit_artifact_sha256_hex": circuit_artifact.hex(),
                "witness_generator_sha256_hex": witness_generator.hex(),
                "public_signal_schema_hash_hex": public_signal_schema.hex(),
                "semantic_proof_profile_hash_hex": common.semantic_proof_profile_hash(
                    circuit_artifact,
                    witness_generator,
                    public_signal_schema,
                ).hex(),
                "sora_finality_anchor": anchor,
                "sora_finality_anchor_hash_hex": common.sora_finality_anchor_hash(
                    anchor
                ).hex(),
                "verifier_key_hash_hex": _unit_v4_hash(profile, "verifier-key"),
                "route_revision": index + 1,
                "verifying_key_sha256_hex": _unit_v4_hash(profile, "verifying-key"),
                "prover_build_sha256_hex": _unit_v4_hash(profile, "prover-build"),
                "toolchain_lock_sha256_hex": _unit_v4_hash(profile, "toolchain-lock"),
                "destination_build": {
                    field: _unit_v4_hash(profile, field)
                    for field in destination_fields
                },
                "audit_attestations": [],
            }
            proof["audit_attestations"] = [
                {
                    "role": role,
                    "auditor_id": f"unit-v4-{role}-auditor",
                    "algorithm": "ed25519",
                    "public_key_hex": auditor_keys[role][0].hex(),
                    "report_sha256_hex": (
                        report_hash := _unit_v4_hash(profile, role, "audit-report")
                    ),
                    "signature_b64": _unit_v4_sign(
                        auditor_keys[role],
                        common.circuit_policy_signing_payload(proof, report_hash),
                    ),
                }
                for role in common.CIRCUIT_AUDITOR_ROLES
            ]
            proof_systems.append(proof)
        policy["proof_systems"] = proof_systems
        policy_bytes = common.canonical_json_file_bytes(policy)
        validated, validated_bytes = common.validate_trust_policy_bytes(
            policy_bytes, allow_test_policy=True
        )
        signing_keys = {**release_keys, **auditor_keys}
        _UNIT_V4_POLICY_CACHE = (validated, validated_bytes, signing_keys)
    policy, policy_bytes, signing_keys = _UNIT_V4_POLICY_CACHE
    return copy.deepcopy(policy), policy_bytes, signing_keys


def unit_v4_fixture(tmp_path: Path, name: str = "unit-v4-source") -> dict[str, object]:
    """Write one tmp-only, unit-signed v4 policy/evidence tree."""

    root = tmp_path / name
    root.mkdir()
    policy, policy_bytes, signing_keys = unit_v4_policy()
    policy_path = root / "unit-v4-trust-policy.json"
    policy_path.write_bytes(policy_bytes)

    artifact_contents: dict[str, bytes] = {}
    for phase in common.REQUIRED_PHASES:
        artifact_contents[f"artifacts/phases/{phase}.log"] = (
            f"unit v4 phase {phase}\n".encode("ascii")
        )
    for profile in common.PROFILE_ORDER:
        artifact_contents[f"artifacts/lanes/{profile}.json"] = (
            common.canonical_json_file_bytes(
                {
                    "schema": "sccp-v4-unit-lane-placeholder-v1",
                    "counterparty_profile": profile,
                }
            )
        )
    artifacts = []
    for relative, data in sorted(artifact_contents.items()):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        artifacts.append(
            {
                "path": relative,
                "kind": "lane-evidence" if "/lanes/" in relative else "phase-transcript",
                "sha256_hex": hashlib.sha256(data).hexdigest(),
                "size_bytes": len(data),
            }
        )

    evidence: dict[str, object] = {
        "schema": common.EVIDENCE_SCHEMA,
        "release_id": "sccp-v4-ephemeral-unit-release-v1",
        "protocol_version": 1,
        "hub_profile": "sora-taira",
        "hub_chain_id": common.HUB_CHAIN_IDS["sora-taira"],
        "created_at_unix_ms": 1_800_000_000_000,
        "trust_policy_id": policy["policy_id"],
        "trust_policy_sha256_hex": hashlib.sha256(policy_bytes).hexdigest(),
        "validator": current_synthetic_validator_identity(),
        "lanes": [
            {
                "counterparty_profile": profile,
                "counterparty_domain": common.PROFILE_DOMAINS[profile],
                "inbound_status": "verified" if index == 0 else "unavailable",
                "outbound_status": "unavailable",
                "evidence_artifact_path": f"artifacts/lanes/{profile}.json",
            }
            for index, profile in enumerate(common.PROFILE_ORDER)
        ],
        "artifacts": artifacts,
        "validation": {
            "corridor": "sccp-production-corridor-v1",
            "phases": [
                {
                    "name": phase,
                    "status": "passed",
                    "artifact_path": f"artifacts/phases/{phase}.log",
                }
                for phase in common.REQUIRED_PHASES
            ],
        },
        "provenance": [],
    }
    payload = common.evidence_signing_payload(evidence)
    evidence["provenance"] = [
        {
            "role": role,
            "signer_id": f"unit-v4-{role}",
            "algorithm": "ed25519",
            "public_key_hex": signing_keys[role][0].hex(),
            "signature_b64": _unit_v4_sign(signing_keys[role], payload),
        }
        for role in common.PROVENANCE_ROLES
    ]
    evidence_path = root / "evidence.json"
    evidence_path.write_bytes(common.canonical_json_file_bytes(evidence))
    evidence, evidence_bytes = common.load_evidence_file(evidence_path, policy)

    validator = root / "unit-validator"
    validator.write_bytes(b"synthetic-sccp-release-validator")
    validator.chmod(validator.stat().st_mode | stat.S_IXUSR)
    return {
        "root": root,
        "policy": policy,
        "policy_bytes": policy_bytes,
        "policy_path": policy_path,
        "evidence": evidence,
        "evidence_bytes": evidence_bytes,
        "evidence_path": evidence_path,
        "validator": validator,
    }


def write_json(path: Path, value: object) -> None:
    """Write the release canonical JSON form used by mutation tests."""

    path.write_bytes(common.canonical_json_file_bytes(value))


def current_synthetic_validator_identity() -> dict[str, object]:
    """Build an exact local-source identity without requiring a Cargo build."""

    identity: dict[str, object] = {
        "protocol_version": 1,
        "crate_name": "iroha_sccp",
        "crate_version": common._workspace_crate_version(),
        "enabled_features": ["dev-tools"],
        "build_profile": "release",
        "target_triple": "aarch64-apple-darwin",
        "rustc_version": (
            f"rustc {common._locked_rust_version()} "
            "(0123456789abcdef0123456789abcdef01234567 2026-01-01)"
        ),
        "source_sha256_hex": hashlib.sha256(
            common.RUST_VALIDATOR_SOURCE.read_bytes()
        ).hexdigest(),
        "crate_manifest_sha256_hex": hashlib.sha256(
            common.SCCP_CRATE_MANIFEST.read_bytes()
        ).hexdigest(),
        "build_script_sha256_hex": hashlib.sha256(
            common.SCCP_BUILD_SCRIPT.read_bytes()
        ).hexdigest(),
        "workspace_manifest_sha256_hex": hashlib.sha256(
            common.WORKSPACE_MANIFEST.read_bytes()
        ).hexdigest(),
        "cargo_lock_sha256_hex": hashlib.sha256(
            common.CARGO_LOCK.read_bytes()
        ).hexdigest(),
        "toolchain_lock_sha256_hex": hashlib.sha256(
            common.RUST_TOOLCHAIN_LOCK.read_bytes()
        ).hexdigest(),
        "executable_sha256_hex": hashlib.sha256(
            b"synthetic-sccp-release-validator"
        ).hexdigest(),
        "build_identity_hex": "00" * 32,
    }
    identity["build_identity_hex"] = common.validator_build_identity_hex(identity)
    return common._validate_validator_identity(identity)


def synthetic_bundle_evidence(
    *, production_semantics: bool
) -> tuple[dict[str, object], bytes, dict[str, object]]:
    """Build an unsigned evidence projection sufficient for bundle-index tests."""

    artifacts: list[dict[str, object]] = []

    def add(kind: str, path: str, label: str) -> None:
        artifacts.append(
            {
                "path": path,
                "kind": kind,
                "sha256_hex": hashlib.sha256(label.encode("ascii")).hexdigest(),
                "size_bytes": len(label),
            }
        )

    for phase in common.REQUIRED_PHASES:
        add("phase-transcript", f"artifacts/phases/{phase}.log", f"phase:{phase}")
    for profile in common.PROFILE_ORDER:
        add("lane-evidence", f"artifacts/lanes/{profile}.json", f"lane:{profile}")

    if production_semantics:
        for profile in common.PROFILE_ORDER:
            for role, kind, filename in common.SEMANTIC_ARTIFACT_ROLES:
                label = f"semantic:{profile}:{role}"
                digest = hashlib.sha256(label.encode("ascii")).hexdigest()
                add(
                    kind,
                    common._semantic_artifact_path(role, digest, filename),
                    label,
                )
            for auditor_role in common.CIRCUIT_AUDITOR_ROLES:
                add(
                    "circuit-audit-report",
                    common._circuit_audit_report_path(profile, auditor_role),
                    f"audit:{profile}:{auditor_role}",
                )

    artifacts.sort(key=lambda entry: entry["path"])
    policy = {"policy_id": "synthetic-bundle-policy-v1"}
    policy_bytes = common.canonical_json_file_bytes(policy)
    evidence: dict[str, object] = {
        "release_id": "synthetic-bundle-release-v1",
        "trust_policy_id": policy["policy_id"],
        "trust_policy_sha256_hex": hashlib.sha256(policy_bytes).hexdigest(),
        "validator": current_synthetic_validator_identity(),
        "artifacts": artifacts,
    }
    evidence_bytes = common.canonical_json_file_bytes(evidence)
    index = common.make_bundle_index(
        evidence,
        evidence_bytes,
        policy,
        policy_bytes,
        evidence["validator"]["executable_sha256_hex"],
    )
    return evidence, evidence_bytes, index


def reseal_bundle_index(index: dict[str, object]) -> None:
    """Recompute the root so mutation tests reach inventory validation."""

    index["entries"].sort(key=lambda entry: entry["path"])
    index["bundle_root_hash_hex"] = common.bundle_root_hash_hex(
        index["entries"],
        trust_policy_id=index["trust_policy_id"],
        trust_policy_sha256_hex=index["trust_policy_sha256_hex"],
        validator=index["validator"],
        validator_executable_sha256_hex=index[
            "validator_executable_sha256_hex"
        ],
    )


def swap_first_bundle_entry_kinds(
    index: dict[str, object], left_kind: str, right_kind: str
) -> None:
    """Swap two entry kinds without changing aggregate structural counts."""

    left = next(entry for entry in index["entries"] if entry["kind"] == left_kind)
    right = next(entry for entry in index["entries"] if entry["kind"] == right_kind)
    left["kind"], right["kind"] = right_kind, left_kind


def mutated_policy(tmp_path: Path, mutation) -> Path:
    policy, _, _ = unit_v4_policy()
    mutation(policy)
    path = tmp_path / "unit-v4-mutated-policy.json"
    write_json(path, policy)
    return path


def mutated_evidence(tmp_path: Path, mutation) -> tuple[dict[str, object], Path]:
    material = unit_v4_fixture(tmp_path)
    value = copy.deepcopy(material["evidence"])
    mutation(value)
    path = tmp_path / "unit-v4-mutated-evidence.json"
    write_json(path, value)
    return material["policy"], path


def build_unit_v4_bundle(
    tmp_path: Path, name: str = "bundle"
) -> tuple[Path, dict[str, object], dict[str, object]]:
    """Assemble a structural unit bundle without claiming Rust validation."""

    material = unit_v4_fixture(tmp_path, f"{name}-source")
    output = tmp_path / name
    output.mkdir()
    (output / "evidence.json").write_bytes(material["evidence_bytes"])
    for entry in material["evidence"]["artifacts"]:
        relative = entry["path"]
        destination = output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((material["root"] / relative).read_bytes())
    index = common.make_bundle_index(
        material["evidence"],
        material["evidence_bytes"],
        material["policy"],
        material["policy_bytes"],
        material["evidence"]["validator"]["executable_sha256_hex"],
    )
    (output / "bundle.json").write_bytes(common.canonical_json_file_bytes(index))
    return output, index, material


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


def test_retired_v3_fixture_is_rejected_by_the_policy_loader() -> None:
    with pytest.raises(common.SccpReleaseError, match="protocol_version"):
        common.load_trust_policy(FIXTURE_POLICY, allow_test_policy=True)


@pytest.mark.skip(
    reason=(
        "requires fresh external v4 circuit-auditor and release-role signatures; "
        "unit keys must never claim release readiness"
    )
)
def test_fresh_v4_release_evidence_is_fully_valid_and_readiness_is_honest() -> None:
    """Reserved for externally signed current-protocol release evidence."""


def test_rust_independently_rejects_the_retired_v3_fixture() -> None:
    result = invoke_release_validator(FIXTURE_POLICY, FIXTURE_EVIDENCE)
    assert result.returncode != 0
    assert result.stdout == ""


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


@pytest.mark.parametrize(
    "case",
    (
        "circuit-id",
        "diagnostic-classification",
        "semantics",
        "signal-binding-artifact",
        "zero-witness",
        "aliased-witness",
        "signal-schema",
        "profile-hash",
        "anchor-chain",
        "anchor-height",
        "anchor-protocol",
        "anchor-zero-context",
        "anchor-zero-artifact",
        "anchor-context-alias",
        "anchor-artifact-alias",
        "anchor-protocol-type",
        "anchor-legacy-field",
        "anchor-hash",
    ),
)
def test_rust_release_trust_rejects_semantic_policy_and_anchor_drift(
    tmp_path: Path, case: str
) -> None:
    policy = json.loads(FIXTURE_POLICY.read_text(encoding="utf-8"))
    proof = policy["proof_systems"][0]
    anchor = proof["sora_finality_anchor"]
    if case == "circuit-id":
        proof["circuit_id"] = "sccp-sora-taira-generic-groth16-bn254-v1"
    elif case == "diagnostic-classification":
        proof["circuit_id"] = "public-signal-binding-material-only"
    elif case == "semantics":
        proof["semantics"] = list(common.REQUIRED_SEMANTICS[:-1])
    elif case == "signal-binding-artifact":
        proof["circuit_artifact_sha256_hex"] = hashlib.sha256(
            common._SIGNAL_BINDING_CIRCUIT.read_bytes()
        ).hexdigest()
    elif case == "zero-witness":
        proof["witness_generator_sha256_hex"] = "00" * 32
    elif case == "aliased-witness":
        proof["witness_generator_sha256_hex"] = proof[
            "circuit_artifact_sha256_hex"
        ]
    elif case == "signal-schema":
        proof["public_signal_schema_hash_hex"] = "51" * 32
    elif case == "profile-hash":
        proof["semantic_proof_profile_hash_hex"] = "52" * 32
    elif case == "anchor-chain":
        anchor["chain_id_hash_hex"] = "53" * 32
    elif case == "anchor-height":
        anchor["checkpoint_height"] = 0
    elif case == "anchor-protocol":
        anchor["protocol_version"] = 1
    elif case == "anchor-zero-context":
        anchor["checkpoint_context_id_hex"] = "00" * 32
    elif case == "anchor-zero-artifact":
        anchor["checkpoint_finality_artifact_hash_hex"] = "00" * 32
    elif case == "anchor-context-alias":
        anchor["checkpoint_context_id_hex"] = anchor["checkpoint_block_hash_hex"]
    elif case == "anchor-artifact-alias":
        anchor["checkpoint_finality_artifact_hash_hex"] = anchor[
            "checkpoint_context_id_hex"
        ]
    elif case == "anchor-protocol-type":
        anchor["protocol_version"] = True
    elif case == "anchor-legacy-field":
        anchor["validator_set_epoch"] = 1
    elif case == "anchor-hash":
        proof["sora_finality_anchor_hash_hex"] = "54" * 32
    else:
        raise AssertionError(case)
    policy_path = tmp_path / "policy.json"
    write_json(policy_path, policy)
    result = invoke_release_validator(policy_path, FIXTURE_EVIDENCE)
    assert result.returncode != 0
    assert result.stdout == ""


@pytest.mark.skip(
    reason=(
        "requires a fresh externally signed v4 bundle and its exact authenticated Rust "
        "validator; unit signatures are not release evidence"
    )
)
def test_fresh_v4_bundle_is_deterministic_and_independently_verifiable() -> None:
    """Reserved for a complete externally signed current-protocol bundle."""


def test_fixture_cli_proves_the_retired_v3_fixture_is_rejected() -> None:
    result = subprocess.run(
        [sys.executable, str(FIXTURE_RUNNER), "reject"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    value = json.loads(result.stdout)
    assert value == {
        "fixture_only": True,
        "policy_id": "sccp-v1-fixture-policy-20260711",
        "release_id": "sccp-v1-typed-fixture-20260711",
        "rejected": True,
        "retired_protocol_version": 3,
        "schema": "sccp-retired-v3-fixture-rejection-v1",
    }


@pytest.mark.parametrize("script", PRODUCTION_CLIS)
def test_production_clis_cannot_accept_fixture_policy(script: Path, tmp_path: Path) -> None:
    if script.name == "sccp_verify_release_bundle.py":
        source = tmp_path
    else:
        source = FIXTURE_EVIDENCE
    command = [
        sys.executable,
        str(script),
        str(source),
        "--trust-policy",
        str(FIXTURE_POLICY),
        "--rust-validator",
        str(FIXTURE_RUNNER),
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
        lambda value: value.update(policy_id="Rogue Policy"),
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
        lambda value: value["proof_systems"][0].update(
            circuit_id="sccp-sora-taira-generic-groth16-bn254-v1"
        ),
        lambda value: value["proof_systems"][0].update(
            circuit_id="sccp-labeled-signal-binding-v1"
        ),
        lambda value: value["proof_systems"][0].update(
            circuit_id="public-signal-binding-material-only"
        ),
        lambda value: value["proof_systems"][0].update(
            circuit_artifact_sha256_hex=hashlib.sha256(
                common._SIGNAL_BINDING_CIRCUIT.read_bytes()
            ).hexdigest()
        ),
        lambda value: value["proof_systems"][0].update(
            witness_generator_sha256_hex="00" * 32
        ),
        lambda value: value["proof_systems"][0].update(
            witness_generator_sha256_hex=value["proof_systems"][0][
                "circuit_artifact_sha256_hex"
            ]
        ),
        lambda value: value["proof_systems"][0].update(
            public_signal_schema_hash_hex="61" * 32
        ),
        lambda value: value["proof_systems"][0].update(
            semantic_proof_profile_hash_hex="62" * 32
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            source_profile="sora-nexus"
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            version=True
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            chain_id_hash_hex="63" * 32
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            checkpoint_height=0
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            checkpoint_block_hash_hex="00" * 32
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            protocol_version=1
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            protocol_version=True
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            checkpoint_context_id_hex="00" * 32
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            checkpoint_finality_artifact_hash_hex="00" * 32
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            checkpoint_context_id_hex=value["proof_systems"][0]["sora_finality_anchor"][
                "checkpoint_block_hash_hex"
            ]
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            checkpoint_finality_artifact_hash_hex=value["proof_systems"][0][
                "sora_finality_anchor"
            ]["checkpoint_context_id_hex"]
        ),
        lambda value: value["proof_systems"][0]["sora_finality_anchor"].update(
            validator_set_epoch=1
        ),
        lambda value: value["proof_systems"][0].update(
            sora_finality_anchor_hash_hex="00" * 32
        ),
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


@pytest.mark.parametrize(
    "mutation",
    (
        lambda proof: proof.update(
            verifier_key_hash_hex=proof["prover_build_sha256_hex"]
        ),
        lambda proof: proof["destination_build"].update(
            compiler_build_sha256_hex=proof["verifying_key_sha256_hex"]
        ),
        lambda proof: proof.update(
            verifier_key_hash_hex=proof["sora_finality_anchor"][
                "checkpoint_block_hash_hex"
            ]
        ),
    ),
)
def test_policy_rejects_aliases_across_all_hash_roles(tmp_path: Path, mutation) -> None:
    path = mutated_policy(tmp_path, lambda value: mutation(value["proof_systems"][0]))
    with pytest.raises(common.SccpReleaseError, match="distinct"):
        common.load_trust_policy(path, allow_test_policy=True)


def test_policy_rejects_cross_profile_cross_category_hash_alias(tmp_path: Path) -> None:
    path = mutated_policy(
        tmp_path,
        lambda value: value["proof_systems"][1].update(
            verifier_key_hash_hex=value["proof_systems"][0][
                "circuit_artifact_sha256_hex"
            ]
        ),
    )
    with pytest.raises(common.SccpReleaseError, match="across profiles and roles"):
        common.load_trust_policy(path, allow_test_policy=True)


def test_policy_rejects_reused_audit_report_across_profiles(tmp_path: Path) -> None:
    path = mutated_policy(
        tmp_path,
        lambda value: value["proof_systems"][1]["audit_attestations"][0].update(
            report_sha256_hex=value["proof_systems"][0]["audit_attestations"][0][
                "report_sha256_hex"
            ]
        ),
    )
    with pytest.raises(common.SccpReleaseError, match="distinct report"):
        common.load_trust_policy(path, allow_test_policy=True)


def test_policy_rejects_audit_report_aliased_to_proof_role(tmp_path: Path) -> None:
    path = mutated_policy(
        tmp_path,
        lambda value: value["proof_systems"][0]["audit_attestations"][0].update(
            report_sha256_hex=value["proof_systems"][0]["verifier_key_hash_hex"]
        ),
    )
    with pytest.raises(common.SccpReleaseError, match="report aliases"):
        common.load_trust_policy(path, allow_test_policy=True)


def test_forbidden_signal_only_circuit_check_does_not_depend_on_repo_file(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(common, "_SIGNAL_BINDING_CIRCUIT", tmp_path / "absent.circom")
    path = mutated_policy(
        tmp_path,
        lambda value: value["proof_systems"][0].update(
            circuit_artifact_sha256_hex=common.FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256_HEX
        ),
    )
    with pytest.raises(common.SccpReleaseError, match="labeled-signal-only"):
        common.load_trust_policy(path, allow_test_policy=True)


def test_production_loader_rejects_test_policy_without_override() -> None:
    with pytest.raises(common.SccpReleaseError, match="schema/environment"):
        common.load_trust_policy(FIXTURE_POLICY)


def test_production_loader_rejects_relabelled_public_fixture_keys(tmp_path: Path) -> None:
    policy = json.loads(FIXTURE_POLICY.read_text(encoding="utf-8"))
    policy["schema"] = common.TRUST_POLICY_SCHEMA
    policy["environment"] = "production"
    policy["policy_id"] = "forged-production-policy-v1"
    for index, role in enumerate(policy["roles"]):
        role["signer_id"] = f"forged-release-role-{index}"
    for index, attestor in enumerate(policy["destination_attestors"]):
        attestor["attestor_id"] = f"forged-attestor-{index}"
    for index, auditor in enumerate(policy["circuit_auditors"]):
        auditor["auditor_id"] = f"forged-auditor-{index}"
    path = tmp_path / "forged-production-policy.json"
    write_json(path, policy)
    with pytest.raises(common.SccpReleaseError, match="fixture-only"):
        common.load_trust_policy(path)


def test_policy_rejects_unknown_and_duplicate_json_keys(tmp_path: Path) -> None:
    raw = FIXTURE_POLICY.read_text(encoding="utf-8")
    unknown = json.loads(raw)
    unknown["allow_test_keys"] = True
    unknown_path = tmp_path / "unknown.json"
    write_json(unknown_path, unknown)
    with pytest.raises(common.SccpReleaseError, match="inexact field set"):
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
        lambda value: value.update(trust_policy_sha256_hex="24" * 32),
        lambda value: value["provenance"].reverse(),
        lambda value: value["provenance"][0].update(signer_id="rogue-signer"),
        lambda value: value["provenance"][0].update(public_key_hex="12" * 32),
        lambda value: value["lanes"][0].update(inbound_status="unavailable"),
        lambda value: value["lanes"][0].update(counterparty_domain=2),
        lambda value: value["lanes"][0].update(counterparty_domain=True),
        lambda value: value.update(protocol_version=True),
        lambda value: value.update(
            hub_profile="sora-nexus",
            hub_chain_id="00000000-0000-0000-0000-000000000753",
        ),
        lambda value: value["validator"].update(protocol_version=True),
        lambda value: value["validator"].update(crate_name="placeholder-validator"),
        lambda value: value["validator"].update(enabled_features=["test-fixtures"]),
        lambda value: value["validator"].update(target_triple="unknown-target-placeholder"),
        lambda value: value["validator"].update(
            rustc_version="rustc 0.0.0 (000000000 1970-01-01)"
        ),
        lambda value: value["validator"].update(source_sha256_hex="22" * 32),
        lambda value: value["validator"].update(executable_sha256_hex="23" * 32),
        lambda value: value["validator"].update(
            toolchain_lock_sha256_hex=value["validator"]["source_sha256_hex"]
        ),
        lambda value: value["validation"]["phases"][0].update(status="skipped"),
        lambda value: value["artifacts"].append(copy.deepcopy(value["artifacts"][0])),
    ),
)
def test_signed_evidence_rejects_tampering(tmp_path: Path, mutation) -> None:
    policy, path = mutated_evidence(tmp_path, mutation)
    with pytest.raises(common.SccpReleaseError):
        common.load_evidence_file(path, policy)


def test_evidence_rejects_noncanonical_encoding_duplicate_keys_and_nonfinite_numbers(
    tmp_path: Path,
) -> None:
    material = unit_v4_fixture(tmp_path)
    policy = material["policy"]
    value = material["evidence"]
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
    material = unit_v4_fixture(tmp_path)
    policy_link = tmp_path / "policy-link.json"
    policy_link.symlink_to(material["policy_path"])
    with pytest.raises(common.SccpReleaseError, match="regular file"):
        common.load_trust_policy(policy_link, allow_test_policy=True)
    evidence_copy = tmp_path / "evidence-copy.json"
    evidence_copy.write_bytes(material["evidence_bytes"])
    evidence_hardlink = tmp_path / "evidence-hardlink.json"
    os.link(evidence_copy, evidence_hardlink)
    policy = material["policy"]
    with pytest.raises(common.SccpReleaseError, match="hard-linked"):
        common.load_evidence_file(evidence_hardlink, policy)
    root = tmp_path / "artifact-root"
    shutil.copytree(material["root"], root)
    lane = root / "artifacts" / "lanes" / "ethereum-mainnet.json"
    replacement = tmp_path / "lane-copy.json"
    replacement.write_bytes(lane.read_bytes())
    lane.unlink()
    os.link(replacement, lane)
    evidence, _ = common.load_evidence_file(root / "evidence.json", policy)
    with pytest.raises(common.SccpReleaseError, match="hard-linked"):
        common.verify_evidence_artifacts(evidence, root)


def test_artifact_tamper_is_rejected_before_rust_validation(tmp_path: Path) -> None:
    material = unit_v4_fixture(tmp_path)
    root = tmp_path / "fixture"
    shutil.copytree(material["root"], root)
    artifact = root / "artifacts" / "lanes" / "ethereum-mainnet.json"
    artifact.write_bytes(artifact.read_bytes() + b" ")
    policy = material["policy"]
    evidence, _ = common.load_evidence_file(root / "evidence.json", policy)
    with pytest.raises(common.SccpReleaseError, match="signed size and SHA-256"):
        common.verify_evidence_artifacts(evidence, root)


@pytest.mark.parametrize(
    "mutation", ("extra", "empty-dir", "artifact", "index", "policy", "validator")
)
def test_bundle_rejects_inventory_and_commitment_tampering(
    tmp_path: Path, mutation: str
) -> None:
    bundle, _, material = build_unit_v4_bundle(tmp_path)
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
            bundle,
            material["policy_path"],
            material["policy"],
            material["policy_bytes"],
            material["validator"],
        )


@pytest.mark.parametrize("production_semantics", (False, True))
def test_bundle_index_accepts_exact_fixture_and_production_inventory(
    production_semantics: bool,
) -> None:
    evidence, evidence_bytes, index = synthetic_bundle_evidence(
        production_semantics=production_semantics
    )
    assert common.validate_bundle_index(copy.deepcopy(index)) == index
    assert (
        common.validate_bundle_index_against_evidence(
            index, evidence, evidence_bytes
        )
        == index
    )
    expected_artifacts = len(common.REQUIRED_PHASES) + len(common.PROFILE_ORDER)
    if production_semantics:
        expected_artifacts += (
            len(common.PROFILE_ORDER) * len(common.SEMANTIC_ARTIFACT_ROLES)
            + len(common.PROFILE_ORDER) * len(common.CIRCUIT_AUDITOR_ROLES)
        )
    assert len(index["entries"]) == expected_artifacts + 1


def test_bundle_index_accepts_policy_shared_semantic_roles_but_distinct_proofs() -> None:
    evidence, _, _ = synthetic_bundle_evidence(production_semantics=True)
    shared_kinds = {
        kind
        for _, kind, _ in common.SEMANTIC_ARTIFACT_ROLES
        if kind != "honest-proof"
    }
    retained_shared: set[str] = set()
    retained = []
    for artifact in evidence["artifacts"]:
        kind = artifact["kind"]
        if kind in shared_kinds:
            if kind in retained_shared:
                continue
            retained_shared.add(kind)
        retained.append(artifact)
    evidence["artifacts"] = retained
    evidence_bytes = common.canonical_json_file_bytes(evidence)
    policy = {"policy_id": "synthetic-bundle-policy-v1"}
    policy_bytes = common.canonical_json_file_bytes(policy)
    index = common.make_bundle_index(
        evidence,
        evidence_bytes,
        policy,
        policy_bytes,
        evidence["validator"]["executable_sha256_hex"],
    )
    assert sum(
        entry["kind"] == "honest-proof" for entry in index["entries"]
    ) == len(common.PROFILE_ORDER)
    for kind in shared_kinds:
        assert sum(entry["kind"] == kind for entry in index["entries"]) == 1


@pytest.mark.parametrize(
    "mutation,match",
    (
        (
            lambda entries: entries.append(
                {
                    "path": "artifacts/semantic/circuit-artifact/"
                    + "91" * 32
                    + "-circuit.bin",
                    "kind": "semantic-circuit",
                    "sha256_hex": "91" * 32,
                    "size_bytes": 1,
                }
            ),
            "semantic-circuit",
        ),
        (
            lambda entries: entries.__setitem__(
                slice(None),
                [entry for entry in entries if entry["kind"] != "circuit-audit-report"],
            ),
            "audit reports",
        ),
        (
            lambda entries: entries.__setitem__(
                slice(None),
                [
                    entry
                    for entry in entries
                    if entry["kind"] != "witness-generator"
                ],
            ),
            "witness-generator",
        ),
        (
            lambda entries: entries.append(
                {
                    "path": "artifacts/semantic/honest-proof/"
                    + "92" * 32
                    + "-honest-proof.norito",
                    "kind": "honest-proof",
                    "sha256_hex": "92" * 32,
                    "size_bytes": 1,
                }
            ),
            "honest-proof",
        ),
    ),
)
def test_bundle_index_rejects_partial_or_count_drifted_semantic_inventory(
    mutation, match: str
) -> None:
    _, _, index = synthetic_bundle_evidence(production_semantics=True)
    mutation(index["entries"])
    reseal_bundle_index(index)
    with pytest.raises(common.SccpReleaseError, match=match):
        common.validate_bundle_index(index)


def test_bundle_index_rejects_fixture_with_partial_semantic_inventory() -> None:
    _, _, index = synthetic_bundle_evidence(production_semantics=False)
    index["entries"].append(
        {
            "path": "artifacts/semantic/circuit-artifact/"
            + "93" * 32
            + "-circuit.bin",
            "kind": "semantic-circuit",
            "sha256_hex": "93" * 32,
            "size_bytes": 1,
        }
    )
    reseal_bundle_index(index)
    with pytest.raises(common.SccpReleaseError, match="audit reports"):
        common.validate_bundle_index(index)


@pytest.mark.parametrize(
    "mutation",
    (
        lambda index: index["entries"].pop(
            next(
                position
                for position, entry in enumerate(index["entries"])
                if entry["kind"] == "semantic-circuit"
            )
        ),
        lambda index: index["entries"].__setitem__(
            next(
                position
                for position, entry in enumerate(index["entries"])
                if entry["kind"] == "semantic-circuit"
            ),
            {
                **next(
                    entry
                    for entry in index["entries"]
                    if entry["kind"] == "semantic-circuit"
                ),
                "path": "artifacts/semantic/circuit-artifact/"
                + "94" * 32
                + "-circuit.bin",
                "sha256_hex": "94" * 32,
            },
        ),
        lambda index: next(
            entry for entry in index["entries"] if entry["kind"] == "semantic-circuit"
        ).update(size_bytes=2),
        lambda index: next(
            entry for entry in index["entries"] if entry["kind"] == "semantic-circuit"
        ).update(sha256_hex="95" * 32),
        lambda index: swap_first_bundle_entry_kinds(
            index, "semantic-circuit", "witness-generator"
        ),
    ),
)
def test_bundle_index_must_exactly_match_signed_production_artifacts(mutation) -> None:
    evidence, evidence_bytes, index = synthetic_bundle_evidence(
        production_semantics=True
    )
    mutation(index)
    reseal_bundle_index(index)
    common.validate_bundle_index(index)
    with pytest.raises(common.SccpReleaseError, match="exactly equal"):
        common.validate_bundle_index_against_evidence(
            index, evidence, evidence_bytes
        )


def test_bundle_index_rejects_omission_plus_untrusted_extra_substitution() -> None:
    evidence, evidence_bytes, index = synthetic_bundle_evidence(
        production_semantics=True
    )
    position = next(
        position
        for position, entry in enumerate(index["entries"])
        if entry["kind"] == "semantic-circuit"
    )
    index["entries"][position] = {
        **index["entries"][position],
        "path": "artifacts/semantic/circuit-artifact/"
        + "96" * 32
        + "-circuit.bin",
        "sha256_hex": "96" * 32,
    }
    reseal_bundle_index(index)
    common.validate_bundle_index(index)
    with pytest.raises(common.SccpReleaseError, match="exactly equal"):
        common.validate_bundle_index_against_evidence(
            index, evidence, evidence_bytes
        )


def test_bundle_index_rejects_cross_entry_hash_alias() -> None:
    _, _, index = synthetic_bundle_evidence(production_semantics=True)
    index["entries"][1]["sha256_hex"] = index["entries"][0]["sha256_hex"]
    reseal_bundle_index(index)
    with pytest.raises(common.SccpReleaseError, match="distinct SHA-256"):
        common.validate_bundle_index(index)


@pytest.mark.parametrize(
    "hostile_path",
    (
        "../escape.json",
        "/absolute.json",
        "artifacts//double.json",
        "artifacts/./dot.json",
        "artifacts/../parent.json",
        "artifacts\\windows.json",
        "artifacts/phases/white space.log",
    ),
)
def test_bundle_index_rejects_path_aliases_and_traversal(hostile_path: str) -> None:
    _, _, index = synthetic_bundle_evidence(production_semantics=False)
    index["entries"][1]["path"] = hostile_path
    reseal_bundle_index(index)
    with pytest.raises(common.SccpReleaseError, match="path|component"):
        common.validate_bundle_index(index)


@pytest.mark.parametrize(
    "mutation,match",
    (
        (
            lambda entries: entries.pop(
                next(
                    position
                    for position, entry in enumerate(entries)
                    if entry["kind"] == "phase-transcript"
                )
            ),
            "bounded signed artifact inventory|phase-transcript count",
        ),
        (
            lambda entries: next(
                entry for entry in entries if entry["kind"] == "lane-evidence"
            ).update(kind="phase-transcript"),
            "phase-transcript count",
        ),
        (
            lambda entries: next(
                entry for entry in entries if entry["kind"] == "phase-transcript"
            ).update(kind="lane-evidence"),
            "phase-transcript count",
        ),
        (
            lambda entries: next(
                entry for entry in entries if entry["kind"] == "lane-evidence"
            ).update(kind="not-a-release-kind"),
            "not part",
        ),
        (
            lambda entries: next(
                entry for entry in entries if entry["kind"] == "release-evidence"
            ).update(path="artifacts/evidence.json"),
            "at evidence.json",
        ),
    ),
)
def test_bundle_index_rejects_core_kind_and_count_confusion(mutation, match: str) -> None:
    _, _, index = synthetic_bundle_evidence(production_semantics=False)
    mutation(index["entries"])
    reseal_bundle_index(index)
    with pytest.raises(common.SccpReleaseError, match=match):
        common.validate_bundle_index(index)


def test_bundle_rejects_symlink_and_hardlink_entries(tmp_path: Path) -> None:
    symlink_bundle, _, symlink_material = build_unit_v4_bundle(
        tmp_path, "symlink-bundle"
    )
    artifact = symlink_bundle / "artifacts" / "phases" / "rust-sccp.log"
    content = artifact.read_bytes()
    artifact.unlink()
    target = tmp_path / "target.log"
    target.write_bytes(content)
    artifact.symlink_to(target)
    with pytest.raises(common.SccpReleaseError):
        verifier.verify_bundle(
            symlink_bundle,
            symlink_material["policy_path"],
            symlink_material["policy"],
            symlink_material["policy_bytes"],
            symlink_material["validator"],
        )

    hardlink_bundle, _, hardlink_material = build_unit_v4_bundle(
        tmp_path, "hardlink-bundle"
    )
    artifact = hardlink_bundle / "artifacts" / "phases" / "rust-sccp.log"
    target = tmp_path / "hard-target.log"
    target.write_bytes(artifact.read_bytes())
    artifact.unlink()
    os.link(target, artifact)
    with pytest.raises(common.SccpReleaseError):
        verifier.verify_bundle(
            hardlink_bundle,
            hardlink_material["policy_path"],
            hardlink_material["policy"],
            hardlink_material["policy_bytes"],
            hardlink_material["validator"],
        )


def test_bundle_enumeration_applies_entry_bound_before_sorting(tmp_path: Path) -> None:
    bundle = tmp_path / "oversized-tree"
    bundle.mkdir()
    for index in range(2 * common.MAX_ARTIFACTS + 9):
        (bundle / f"entry-{index:03d}").write_bytes(b"x")
    with pytest.raises(common.SccpReleaseError, match="too many entries"):
        common.enumerate_direct_files(bundle)


def test_directory_relative_writer_refuses_symlinked_parent(tmp_path: Path) -> None:
    root = tmp_path / "root"
    outside = tmp_path / "outside"
    root.mkdir()
    outside.mkdir()
    (root / "artifacts").symlink_to(outside, target_is_directory=True)
    descriptor = common.open_direct_directory(root, label="test output root")
    try:
        with pytest.raises(common.SccpReleaseError, match="opened safely"):
            builder._write_relative_output(
                descriptor,
                "artifacts/escaped.json",
                b"{}\n",
            )
    finally:
        os.close(descriptor)
    assert not (outside / "escaped.json").exists()


def test_builder_refuses_existing_or_unsafe_output(tmp_path: Path) -> None:
    material = unit_v4_fixture(tmp_path)
    existing = tmp_path / "existing"
    existing.mkdir()
    with pytest.raises(common.SccpReleaseError, match="never overwrites"):
        builder.build_bundle(
            material["evidence_path"],
            material["root"],
            existing,
            material["policy_path"],
            material["policy"],
            material["policy_bytes"],
            material["validator"],
        )
    with pytest.raises(common.SccpReleaseError, match="canonical artifact alphabet"):
        builder.build_bundle(
            material["evidence_path"],
            material["root"],
            tmp_path / "unsafe output",
            material["policy_path"],
            material["policy"],
            material["policy_bytes"],
            material["validator"],
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


def test_secret_scanner_rejects_deep_encoding_and_colon_credentials() -> None:
    encoded = b"private_key=abc"
    for _ in range(6):
        encoded = encoded.replace(b"%", b"%25").replace(b"_", b"%5f").replace(b"=", b"%3d")
    with pytest.raises(common.SccpReleaseError, match="credential material"):
        common.reject_secret_material(encoded, label="nested artifact")
    with pytest.raises(common.SccpReleaseError, match="credential material"):
        common.reject_secret_material(b"password: hunter2", label="colon artifact")


def test_public_error_is_bounded_and_redacts_userinfo_and_secret_markers() -> None:
    error = ValueError(
        "https://alice:password@example.test private_key=\n\x1b[31m\u202eforged"
        + "x" * 5000
    )
    rendered = common.public_error(error)
    assert len(rendered.encode()) <= common.MAX_PUBLIC_ERROR_BYTES
    assert "alice:password" not in rendered
    assert "private_key" not in rendered.lower()
    assert "\n" not in rendered and "\x1b" not in rendered and "\u202e" not in rendered


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


def test_validator_substitution_is_rejected_before_execution(tmp_path: Path) -> None:
    material = unit_v4_fixture(tmp_path)
    marker = tmp_path / "executed"
    substitute = tmp_path / "substitute"
    substitute.write_text(f"#!/bin/sh\ntouch {marker}\n", encoding="utf-8")
    substitute.chmod(substitute.stat().st_mode | stat.S_IXUSR)
    with pytest.raises(common.SccpReleaseError, match="signed release evidence"):
        common.verify_rust_lane_evidence(
            material["evidence"],
            material["root"],
            substitute,
            material["policy"],
            trust_policy_path=material["policy_path"],
            evidence_path=material["evidence_path"],
            environment="test-fixture",
        )
    assert not marker.exists()


def test_authenticated_validator_output_flood_is_bounded(tmp_path: Path, monkeypatch) -> None:
    flood = tmp_path / "flood"
    flood.write_text("#!/bin/sh\nyes x\n", encoding="utf-8")
    flood.chmod(flood.stat().st_mode | stat.S_IXUSR)
    monkeypatch.setattr(common, "MAX_VALIDATOR_SECONDS", 1)
    monkeypatch.setattr(common, "MAX_VALIDATOR_OUTPUT_BYTES", 128)
    with pytest.raises(common.SccpReleaseError, match="output limit|time limit"):
        common._invoke_validator_command(
            flood,
            (),
            hashlib.sha256(flood.read_bytes()).hexdigest(),
        )


def test_python_release_path_only_reimplements_the_two_policy_hashes() -> None:
    source = (SCRIPTS / "sccp_release_common.py").read_text(encoding="utf-8")
    for forbidden in (
        "def canonical_lane",
        "def source_identity_hash",
        "sccp_exact_evm_xor_route_config_hash_v1(",
        "proof_valid",
        "allow_unready",
    ):
        assert forbidden not in source


def test_policy_hash_derivation_matches_rust_and_solidity_golden_vectors() -> None:
    assert common.keccak256(b"").hex() == (
        "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    )
    assert common.keccak256(b"abc").hex() == (
        "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
    )
    public_taira_id = common.HUB_CHAIN_IDS["sora-taira"]
    assert public_taira_id == "fc56984b-2be7-431d-840e-21514d1883f0"
    assert common.keccak256(bytes.fromhex(public_taira_id.replace("-", ""))).hex() == (
        common.SORA_TAIRA_CHAIN_ID_HASH_HEX
    )
    for length, expected in (
        (135, "cbdfd9dee5faad3818d6b06f95a219fd290b0e1706f6a82e5a595b9ce9faca62"),
        (136, "7ce759f1ab7f9ce437719970c26b0a66ff11fe3e38e17df89cf5d29c7d7f807e"),
        (137, "ac73d4fae68b8453f764007c1a20ce95994187861f0c3227a3a8e99a73a3b1db"),
    ):
        assert common.keccak256(bytes(range(256))[:length]).hex() == expected
    profile_hash = common.semantic_proof_profile_hash(
        bytes([0x71]) * 32,
        bytes([0x72]) * 32,
        bytes.fromhex(common.PUBLIC_SIGNAL_SCHEMA_HASH_HEX),
    )
    assert profile_hash.hex() == (
        "ce5a1e17aca3cafe47a403fd66479f0a36339eb56092dafa67c8d97bdeeb60ef"
    )
    anchor_hash = common.sora_finality_anchor_hash(
        {
            "version": 1,
            "source_profile": "sora-taira",
            "protocol_version": common.SORA_TAIRA_SUMERAGI_PROTOCOL_VERSION,
            "chain_id_hash_hex": common.SORA_TAIRA_CHAIN_ID_HASH_HEX,
            "checkpoint_height": 5,
            "checkpoint_block_hash_hex": "73" * 32,
            "checkpoint_context_id_hex": "74" * 32,
            "checkpoint_finality_artifact_hash_hex": "75" * 32,
        }
    )
    assert anchor_hash.hex() == (
        "31328ad8005a0f33e6050e8ae96f012b3285f7f14737486dce34f972686862f5"
    )


@pytest.mark.parametrize(
    "mutation",
    (
        lambda anchor: anchor.update(
            protocol_version=common.SORA_TAIRA_SUMERAGI_PROTOCOL_VERSION - 1
        ),
        lambda anchor: anchor.update(
            protocol_version=common.SORA_TAIRA_SUMERAGI_PROTOCOL_VERSION + 1
        ),
        lambda anchor: anchor.update(protocol_version=True),
        lambda anchor: anchor.update(
            protocol_version=str(common.SORA_TAIRA_SUMERAGI_PROTOCOL_VERSION)
        ),
        lambda anchor: anchor.update(checkpoint_height=True),
        lambda anchor: anchor.update(checkpoint_context_id_hex="00" * 32),
        lambda anchor: anchor.update(
            checkpoint_finality_artifact_hash_hex="00" * 32
        ),
        lambda anchor: anchor.update(checkpoint_context_id_hex=True),
        lambda anchor: anchor.update(
            checkpoint_finality_artifact_hash_hex=bytes([0x75]) * 32
        ),
        lambda anchor: anchor.update(
            checkpoint_context_id_hex=anchor["chain_id_hash_hex"]
        ),
        lambda anchor: anchor.update(
            checkpoint_context_id_hex=anchor["checkpoint_block_hash_hex"]
        ),
        lambda anchor: anchor.update(
            checkpoint_context_id_hex=anchor[
                "checkpoint_finality_artifact_hash_hex"
            ]
        ),
        lambda anchor: anchor.update(
            checkpoint_finality_artifact_hash_hex=anchor[
                "checkpoint_context_id_hex"
            ]
        ),
        lambda anchor: anchor.update(
            checkpoint_finality_artifact_hash_hex=anchor[
                "checkpoint_block_hash_hex"
            ]
        ),
        lambda anchor: anchor.update(
            checkpoint_finality_artifact_hash_hex=anchor["chain_id_hash_hex"]
        ),
        lambda anchor: anchor.update(validator_set_epoch=1),
        lambda anchor: anchor.update(validator_set_hash_hex="76" * 32),
        lambda anchor: anchor.update(validator_set_hash_version=1),
        lambda anchor: anchor.pop("checkpoint_context_id_hex"),
        lambda anchor: anchor.pop("checkpoint_finality_artifact_hash_hex"),
    ),
)
def test_sumeragi_v2_anchor_hash_rejects_protocol_role_and_schema_drift(
    mutation,
) -> None:
    anchor = {
        "version": 1,
        "source_profile": "sora-taira",
        "protocol_version": common.SORA_TAIRA_SUMERAGI_PROTOCOL_VERSION,
        "chain_id_hash_hex": common.SORA_TAIRA_CHAIN_ID_HASH_HEX,
        "checkpoint_height": 5,
        "checkpoint_block_hash_hex": "73" * 32,
        "checkpoint_context_id_hex": "74" * 32,
        "checkpoint_finality_artifact_hash_hex": "75" * 32,
    }
    mutation(anchor)
    with pytest.raises(common.SccpReleaseError):
        common.sora_finality_anchor_hash(anchor)


def test_required_semantics_bind_v2_finality_artifact_and_dual_quorum() -> None:
    assert common.REQUIRED_SEMANTICS == (
        "sccp-canonical-transfer-v1",
        "sccp-message-leaf-v1",
        "sccp-merkle-inclusion-v1",
        "sora-taira-block-commitment-v1",
        "sora-taira-v2-finality-artifact-v1",
        "sora-taira-v2-dual-quorum-v1",
        "sora-taira-anchor-continuity-v1",
    )


def test_validator_build_identity_matches_rust_golden() -> None:
    identity = {
        "protocol_version": 1,
        "crate_name": "iroha_sccp",
        "crate_version": "2.0.0-rc.2.0",
        "enabled_features": ["dev-tools"],
        "build_profile": "release",
        "target_triple": "aarch64-apple-darwin",
        "rustc_version": "rustc 1.93.1 (01f6ddf75 2026-02-11)",
        "source_sha256_hex": "01" * 32,
        "crate_manifest_sha256_hex": "02" * 32,
        "build_script_sha256_hex": "03" * 32,
        "workspace_manifest_sha256_hex": "04" * 32,
        "cargo_lock_sha256_hex": "05" * 32,
        "toolchain_lock_sha256_hex": "06" * 32,
        "executable_sha256_hex": "07" * 32,
        "build_identity_hex": "08" * 32,
    }
    assert common.validator_build_identity_hex(identity) == (
        "8ed0a7bfbff79b302fe1193ca51a992b65b7ab136862ecc871efcce391449414"
    )


def test_validator_build_attestation_rejects_placeholders_aliases_and_drift() -> None:
    identity = {
        "protocol_version": 1,
        "crate_name": "iroha_sccp",
        "crate_version": common._workspace_crate_version(),
        "enabled_features": ["dev-tools"],
        "build_profile": "debug",
        "target_triple": "aarch64-apple-darwin",
        "rustc_version": "rustc 1.93.1 (01f6ddf75 2026-02-11)",
        "source_sha256_hex": hashlib.sha256(
            common.RUST_VALIDATOR_SOURCE.read_bytes()
        ).hexdigest(),
        "crate_manifest_sha256_hex": hashlib.sha256(
            common.SCCP_CRATE_MANIFEST.read_bytes()
        ).hexdigest(),
        "build_script_sha256_hex": hashlib.sha256(
            common.SCCP_BUILD_SCRIPT.read_bytes()
        ).hexdigest(),
        "workspace_manifest_sha256_hex": hashlib.sha256(
            common.WORKSPACE_MANIFEST.read_bytes()
        ).hexdigest(),
        "cargo_lock_sha256_hex": hashlib.sha256(common.CARGO_LOCK.read_bytes()).hexdigest(),
        "toolchain_lock_sha256_hex": hashlib.sha256(
            common.RUST_TOOLCHAIN_LOCK.read_bytes()
        ).hexdigest(),
        "executable_sha256_hex": "a7" * 32,
        "build_identity_hex": "a8" * 32,
    }
    identity["build_identity_hex"] = common.validator_build_identity_hex(identity)
    assert common._validate_validator_identity(copy.deepcopy(identity)) == identity

    mutations = (
        lambda value: value.update(enabled_features=[]),
        lambda value: value.update(enabled_features=["test-fixtures"]),
        lambda value: value.update(
            enabled_features=["dev-tools", "test-fixtures"]
        ),
        lambda value: value.update(enabled_features=["dev-tools", "dev-tools"]),
        lambda value: value.update(build_profile=True),
        lambda value: value.update(target_triple="unknown-target-placeholder"),
        lambda value: value.update(
            rustc_version="rustc 0.0.0 (000000000 1970-01-01)"
        ),
        lambda value: value.update(executable_sha256_hex="00" * 32),
        lambda value: value.update(
            toolchain_lock_sha256_hex=value["source_sha256_hex"]
        ),
        lambda value: value.update(build_identity_hex="a9" * 32),
    )
    for mutation in mutations:
        candidate = copy.deepcopy(identity)
        mutation(candidate)
        with pytest.raises(common.SccpReleaseError):
            common._validate_validator_identity(candidate)


def test_validator_build_attestation_rejects_historical_bls_feature() -> None:
    identity = {
        "protocol_version": 1,
        "crate_name": "iroha_sccp",
        "crate_version": common._workspace_crate_version(),
        "enabled_features": ["bls"],
        "build_profile": "debug",
        "target_triple": "aarch64-apple-darwin",
        "rustc_version": "rustc 1.93.1 (01f6ddf75 2026-02-11)",
        "source_sha256_hex": hashlib.sha256(
            common.RUST_VALIDATOR_SOURCE.read_bytes()
        ).hexdigest(),
        "crate_manifest_sha256_hex": hashlib.sha256(
            common.SCCP_CRATE_MANIFEST.read_bytes()
        ).hexdigest(),
        "build_script_sha256_hex": hashlib.sha256(
            common.SCCP_BUILD_SCRIPT.read_bytes()
        ).hexdigest(),
        "workspace_manifest_sha256_hex": hashlib.sha256(
            common.WORKSPACE_MANIFEST.read_bytes()
        ).hexdigest(),
        "cargo_lock_sha256_hex": hashlib.sha256(common.CARGO_LOCK.read_bytes()).hexdigest(),
        "toolchain_lock_sha256_hex": hashlib.sha256(
            common.RUST_TOOLCHAIN_LOCK.read_bytes()
        ).hexdigest(),
        "executable_sha256_hex": "a7" * 32,
        "build_identity_hex": "a8" * 32,
    }
    identity["build_identity_hex"] = common.validator_build_identity_hex(identity)

    with pytest.raises(
        common.SccpReleaseError,
        match=r"exact production feature set \['dev-tools'\]",
    ):
        common._validate_validator_identity(identity)


@pytest.mark.parametrize(
    "commitments",
    (
        (b"", bytes([2]) * 32, bytes([3]) * 32),
        (bytes([1]) * 31, bytes([2]) * 32, bytes([3]) * 32),
        (bytes(32), bytes([2]) * 32, bytes([3]) * 32),
        (bytes([1]) * 32, bytes([1]) * 32, bytes([3]) * 32),
    ),
)
def test_semantic_profile_hash_rejects_malformed_and_aliased_roles(
    commitments: tuple[bytes, bytes, bytes],
) -> None:
    with pytest.raises(common.SccpReleaseError):
        common.semantic_proof_profile_hash(*commitments)


def _semantic_hash(label: str) -> str:
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def synthetic_production_semantic_inventory() -> tuple[
    dict[str, object], dict[str, object], dict[str, bytes]
]:
    """Build unsigned but internally exact semantic evidence for unit checks."""

    auditors = [
        {"role": role, "auditor_id": f"independent-{index + 1}"}
        for index, role in enumerate(common.CIRCUIT_AUDITOR_ROLES)
    ]
    policy: dict[str, object] = {
        "environment": "production",
        "policy_id": "synthetic-production-policy",
        "circuit_auditors": auditors,
        "proof_systems": [],
    }
    evidence: dict[str, object] = {
        "release_id": "synthetic-production-release",
        "artifacts": [],
    }
    contents: dict[str, bytes] = {}
    artifact_by_path: dict[str, dict[str, object]] = {}
    for profile_index, profile in enumerate(common.PROFILE_ORDER):
        artifact_rows: list[dict[str, object]] = []
        role_digests: dict[str, str] = {}
        for role, kind, filename in common.SEMANTIC_ARTIFACT_ROLES:
            content = f"audited:{profile}:{role}:v1".encode("ascii")
            digest = hashlib.sha256(content).hexdigest()
            path = common._semantic_artifact_path(role, digest, filename)
            row = {
                "role": role,
                "kind": kind,
                "path": path,
                "sha256_hex": digest,
                "size_bytes": len(content),
            }
            artifact_rows.append(row)
            role_digests[role] = digest
            contents[path] = content
            artifact_by_path[path] = {
                "path": path,
                "kind": kind,
                "sha256_hex": digest,
                "size_bytes": len(content),
            }

        semantic_profile_hash = common.semantic_proof_profile_hash(
            bytes.fromhex(role_digests["circuit-artifact"]),
            bytes.fromhex(role_digests["witness-generator"]),
            bytes.fromhex(common.PUBLIC_SIGNAL_SCHEMA_HASH_HEX),
        ).hex()
        anchor = {
            "version": 1,
            "source_profile": "sora-taira",
            "protocol_version": common.SORA_TAIRA_SUMERAGI_PROTOCOL_VERSION,
            "chain_id_hash_hex": common.SORA_TAIRA_CHAIN_ID_HASH_HEX,
            "checkpoint_height": 100 + profile_index,
            "checkpoint_block_hash_hex": _semantic_hash(
                f"{profile}:anchor-block"
            ),
            "checkpoint_context_id_hex": _semantic_hash(
                f"{profile}:height-context"
            ),
            "checkpoint_finality_artifact_hash_hex": _semantic_hash(
                f"{profile}:finality-artifact"
            ),
        }
        anchor_hash = common.sora_finality_anchor_hash(anchor).hex()
        proof_system: dict[str, object] = {
            "counterparty_profile": profile,
            "circuit_id": f"sccp-sora-taira-to-{profile}-groth16-bn254-v1",
            "semantics": list(common.REQUIRED_SEMANTICS),
            "circuit_artifact_sha256_hex": role_digests["circuit-artifact"],
            "witness_generator_sha256_hex": role_digests["witness-generator"],
            "public_signal_schema_hash_hex": common.PUBLIC_SIGNAL_SCHEMA_HASH_HEX,
            "semantic_proof_profile_hash_hex": semantic_profile_hash,
            "sora_finality_anchor": anchor,
            "sora_finality_anchor_hash_hex": anchor_hash,
            "verifier_key_hash_hex": _semantic_hash(f"{profile}:vk-keccak"),
            "route_revision": profile_index + 1,
            "verifying_key_sha256_hex": role_digests["verifying-key"],
            "prover_build_sha256_hex": role_digests["prover-build"],
            "toolchain_lock_sha256_hex": role_digests["toolchain-lock"],
            "audit_attestations": [],
        }
        claim = {
            "source_profile": "sora-taira",
            "target_profile": profile,
            "target_domain": common.PROFILE_DOMAINS[profile],
            "route_revision": profile_index + 1,
            "message_id_hex": _semantic_hash(f"{profile}:message"),
            "payload_hash_hex": _semantic_hash(f"{profile}:payload"),
            "commitment_root_hex": _semantic_hash(f"{profile}:root"),
            "finality_height": str(1_000 + profile_index),
            "finality_block_hash_hex": _semantic_hash(f"{profile}:finality-block"),
            "destination_binding_hash_hex": _semantic_hash(f"{profile}:binding"),
            "route_configuration_hash_hex": _semantic_hash(f"{profile}:route"),
            "statement_hash_hex": _semantic_hash(f"{profile}:statement"),
            "request_hash_hex": _semantic_hash(f"{profile}:request"),
            "result_hash_hex": _semantic_hash(f"{profile}:result"),
            "verifier_key_hash_hex": proof_system["verifier_key_hash_hex"],
            "semantic_proof_profile_hash_hex": semantic_profile_hash,
            "sora_finality_anchor_hash_hex": anchor_hash,
            "public_signal_words_hex": [
                _semantic_hash(f"{profile}:signal:{index}") for index in range(11)
            ],
        }
        for auditor_index, role in enumerate(common.CIRCUIT_AUDITOR_ROLES):
            report = {
                "schema": "sccp-circuit-audit-report-v1",
                "role": role,
                "auditor_id": auditors[auditor_index]["auditor_id"],
                "counterparty_profile": profile,
                "circuit_id": proof_system["circuit_id"],
                "semantics": list(common.REQUIRED_SEMANTICS),
                "artifacts": artifact_rows,
                "honest_proof_claim": claim,
            }
            report_bytes = common.canonical_json_file_bytes(report)
            report_hash = hashlib.sha256(report_bytes).hexdigest()
            report_path = common._circuit_audit_report_path(profile, role)
            contents[report_path] = report_bytes
            artifact_by_path[report_path] = {
                "path": report_path,
                "kind": "circuit-audit-report",
                "sha256_hex": report_hash,
                "size_bytes": len(report_bytes),
            }
            proof_system["audit_attestations"].append(
                {"report_sha256_hex": report_hash}
            )
        policy["proof_systems"].append(proof_system)
    evidence["artifacts"] = sorted(artifact_by_path.values(), key=lambda row: row["path"])
    return policy, evidence, contents


def test_production_semantic_inventory_closes_two_audits_and_three_honest_proofs() -> None:
    policy, evidence, contents = synthetic_production_semantic_inventory()
    records = common.verify_production_semantic_artifacts(evidence, contents, policy)
    assert tuple(record[0] for record in records) == common.PROFILE_ORDER
    assert len({record[1] for record in records}) == len(common.PROFILE_ORDER)
    assert all(len(record[2]["public_signal_words_hex"]) == 11 for record in records)


@pytest.mark.parametrize(
    "mutation, message",
    (
        (lambda report: report["honest_proof_claim"].update(finality_height="01"), "u64"),
        (
            lambda report: report["honest_proof_claim"].update(
                public_signal_words_hex=report["honest_proof_claim"][
                    "public_signal_words_hex"
                ][:-1]
            ),
            "exactly 11",
        ),
        (
            lambda report: report["honest_proof_claim"]["public_signal_words_hex"].__setitem__(
                3, "AA" * 32
            ),
            "lowercase",
        ),
        (
            lambda report: report["artifacts"].__setitem__(
                6, {**report["artifacts"][6], "kind": "honest-witness"}
            ),
            "roles, kinds, and paths",
        ),
    ),
)
def test_circuit_audit_report_rejects_adversarial_claims_and_role_substitution(
    mutation, message: str
) -> None:
    policy, _, contents = synthetic_production_semantic_inventory()
    proof_system = policy["proof_systems"][0]
    role = common.CIRCUIT_AUDITOR_ROLES[0]
    path = common._circuit_audit_report_path(common.PROFILE_ORDER[0], role)
    report = json.loads(contents[path])
    mutation(report)
    with pytest.raises(common.SccpReleaseError, match=message):
        common._validate_circuit_audit_report(
            common.canonical_json_file_bytes(report),
            profile=common.PROFILE_ORDER[0],
            role=role,
            auditor_id=policy["circuit_auditors"][0]["auditor_id"],
            proof_system=proof_system,
        )


@pytest.mark.parametrize(
    "replacement, message",
    (
        (b"\0" * 32, "zero or fixture-only"),
        (b"audited fixture-only circuit", "zero or fixture-only"),
    ),
)
def test_production_semantic_inventory_rejects_placeholder_artifact_bytes(
    replacement: bytes, message: str
) -> None:
    policy, evidence, contents = synthetic_production_semantic_inventory()
    circuit_path = next(
        row["path"] for row in evidence["artifacts"] if row["kind"] == "semantic-circuit"
    )
    contents[circuit_path] = replacement
    with pytest.raises(common.SccpReleaseError, match=message):
        common.verify_production_semantic_artifacts(evidence, contents, policy)


def test_production_semantic_inventory_rejects_unattested_extra_artifact() -> None:
    policy, evidence, contents = synthetic_production_semantic_inventory()
    content = b"unattested semantic witness"
    digest = hashlib.sha256(content).hexdigest()
    path = common._semantic_artifact_path(
        "honest-witness", digest, "honest-witness.bin"
    )
    evidence["artifacts"].append(
        {
            "path": path,
            "kind": "honest-witness",
            "sha256_hex": digest,
            "size_bytes": len(content),
        }
    )
    evidence["artifacts"].sort(key=lambda row: row["path"])
    contents[path] = content
    with pytest.raises(common.SccpReleaseError, match="invalid honest-witness artifact cardinality"):
        common.verify_production_semantic_artifacts(evidence, contents, policy)


def test_honest_proof_artifact_uses_the_protocol_decode_bound() -> None:
    assert common.artifact_limit("honest-proof") == 16 * 1024 * 1024 + 64 * 1024
    assert common.artifact_limit("honest-proof") < common.artifact_limit("honest-witness")


def _mock_semantic_receipt(
    *,
    profile: str,
    proof_path: str,
    claim: dict[str, object],
    policy: dict[str, object],
    evidence: dict[str, object],
    policy_bytes: bytes,
    evidence_bytes: bytes,
) -> dict[str, object]:
    metadata = next(row for row in evidence["artifacts"] if row["path"] == proof_path)
    return {
        "schema": "sccp-semantic-proof-validation-v1",
        "environment": "production",
        "policy_id": policy["policy_id"],
        "release_id": evidence["release_id"],
        "policy_sha256_hex": hashlib.sha256(policy_bytes).hexdigest(),
        "evidence_sha256_hex": hashlib.sha256(evidence_bytes).hexdigest(),
        "proof_artifact_path": proof_path,
        "proof_artifact_sha256_hex": metadata["sha256_hex"],
        "canonical_norito_verified": True,
        "pairing_verified": True,
        "claim": claim,
    }


def test_authenticated_rust_semantic_receipts_must_equal_both_auditors_claims(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    policy, evidence, contents = synthetic_production_semantic_inventory()
    records = common.verify_production_semantic_artifacts(evidence, contents, policy)
    policy_bytes = common.canonical_json_file_bytes(policy)
    evidence_bytes = common.canonical_json_file_bytes(evidence)
    validator = tmp_path / "validator"
    validator.write_bytes(b"authenticated semantic validator")
    validator.chmod(0o500)
    executable_hash = hashlib.sha256(validator.read_bytes()).hexdigest()
    by_profile = {profile: (path, claim) for profile, path, claim in records}

    def invoke(_validator, arguments, expected_hash):
        assert arguments[0] == "validate-semantic-proof"
        profile = arguments[4]
        path, claim = by_profile[profile]
        receipt = _mock_semantic_receipt(
            profile=profile,
            proof_path=path,
            claim=claim,
            policy=policy,
            evidence=evidence,
            policy_bytes=policy_bytes,
            evidence_bytes=evidence_bytes,
        )
        return common.canonical_json_file_bytes(receipt), b"", 0, expected_hash

    monkeypatch.setattr(common, "_invoke_validator_command", invoke)
    receipts = common.verify_rust_semantic_proofs(
        evidence=evidence,
        evidence_bytes=evidence_bytes,
        artifact_root=tmp_path,
        semantic_records=records,
        trust_policy=policy,
        trust_policy_bytes=policy_bytes,
        trust_policy_path=tmp_path / "policy.json",
        evidence_path=tmp_path / "evidence.json",
        validator_path=validator,
        expected_executable_hash=executable_hash,
    )
    assert tuple(row["claim"]["target_profile"] for row in receipts) == common.PROFILE_ORDER


@pytest.mark.parametrize(
    "mutation",
    (
        lambda receipt: receipt.update(pairing_verified=False),
        lambda receipt: receipt.update(canonical_norito_verified=1),
        lambda receipt: receipt["claim"]["public_signal_words_hex"].__setitem__(
            0, "ff" * 32
        ),
        lambda receipt: receipt.update(proof_artifact_path="artifacts/semantic/substituted"),
    ),
)
def test_authenticated_rust_semantic_receipt_rejects_false_or_substituted_results(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, mutation
) -> None:
    policy, evidence, contents = synthetic_production_semantic_inventory()
    records = common.verify_production_semantic_artifacts(evidence, contents, policy)
    policy_bytes = common.canonical_json_file_bytes(policy)
    evidence_bytes = common.canonical_json_file_bytes(evidence)
    validator = tmp_path / "validator"
    validator.write_bytes(b"authenticated semantic validator")
    validator.chmod(0o500)
    executable_hash = hashlib.sha256(validator.read_bytes()).hexdigest()
    first_profile, first_path, first_claim = records[0]

    def invoke(_validator, arguments, expected_hash):
        profile = arguments[4]
        path, claim = next((path, claim) for item, path, claim in records if item == profile)
        receipt = _mock_semantic_receipt(
            profile=profile,
            proof_path=path,
            claim=copy.deepcopy(claim),
            policy=policy,
            evidence=evidence,
            policy_bytes=policy_bytes,
            evidence_bytes=evidence_bytes,
        )
        if profile == first_profile:
            mutation(receipt)
        return common.canonical_json_file_bytes(receipt), b"", 0, expected_hash

    assert first_path and first_claim
    monkeypatch.setattr(common, "_invoke_validator_command", invoke)
    with pytest.raises(common.SccpReleaseError, match="does not match"):
        common.verify_rust_semantic_proofs(
            evidence=evidence,
            evidence_bytes=evidence_bytes,
            artifact_root=tmp_path,
            semantic_records=records,
            trust_policy=policy,
            trust_policy_bytes=policy_bytes,
            trust_policy_path=tmp_path / "policy.json",
            evidence_path=tmp_path / "evidence.json",
            validator_path=validator,
            expected_executable_hash=executable_hash,
        )


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
    assert "build_bundle" not in fixture_source
    assert "verify_bundle" not in fixture_source
    for forbidden in (
        "private_key",
        "FIXTURE_KEY_DOMAIN",
        "_fixture_signature",
        "_fixture_signing_material",
        '"regenerate"',
    ):
        assert forbidden not in fixture_source


def test_production_tooling_has_no_private_key_api_or_signing_call() -> None:
    for path in (*PRODUCTION_CLIS, SCRIPTS / "sccp_release_common.py"):
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source, filename=str(path))
        imported_modules = {
            alias.name
            for node in ast.walk(tree)
            if isinstance(node, ast.Import)
            for alias in node.names
        } | {
            node.module
            for node in ast.walk(tree)
            if isinstance(node, ast.ImportFrom) and node.module is not None
        }
        assert not imported_modules & {
            "nacl.signing",
            "cryptography.hazmat.primitives.asymmetric.ed25519",
        }
        assert not any(
            isinstance(node, ast.Attribute) and node.attr in {"sign", "private_bytes"}
            for node in ast.walk(tree)
        )
        assert not any(
            isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and (node.name == "sign" or node.name.startswith("sign_"))
            for node in ast.walk(tree)
        )
