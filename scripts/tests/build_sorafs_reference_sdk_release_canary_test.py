"""Tests for scripts/build_sorafs_reference_sdk_release_canary.py."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_reference_sdk_release_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_reference_sdk_release_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_reference_sdk_release_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reference_sdk_release_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)

import sorafs_topology_qualification as TOPOLOGY  # noqa: E402
from sorafs_reference_sdk_supply_chain import (  # noqa: E402
    SourceArtifactBinding,
    SupplyChainSourceResult,
    SupplyChainTargetResult,
)
from sorafs_resilience_test_support import (  # noqa: E402
    public_key_from_seed,
    sign,
)


NOW_UNIX = 1_800_700_000
GENERATED_AT = NOW_UNIX - 120
MANIFEST_DIGEST = "a" * 64
ARCHIVE_DIGEST = "b" * 64
PACKAGE_DIGEST = "c" * 64
SMOKE_DIGEST = "d" * 64
HEADER_DIGEST = "e" * 64
FFI_DIGEST = "f" * 64
POLICY_DIGEST = "1" * 64
PUBLIC_KEY_DIGEST = "2" * 64
SBOM_DIGEST = "3" * 64
VULNERABILITY_DIGEST = "4" * 64
PROVENANCE_DIGEST = "5" * 64
RELEASE_REHEARSAL_DIGEST = "6" * 64
PROVENANCE_CERTIFICATE_IDENTITY = (
    "https://github.com/hyperledger/iroha/"
    ".github/workflows/sorafs-cli-release.yml@refs/tags/sorafs-cli-v1.0.0"
)
PROVENANCE_OIDC_ISSUER = "https://token.actions.githubusercontent.com"
PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX = (
    "d75a980182b10ab7d54bfed3c964073a"
    "0ee172f3daa62325af021a68f707511a"
)
PROVENANCE_VERIFICATION_KEY_FINGERPRINT = hashlib.sha256(
    bytes.fromhex(PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX)
).hexdigest()
PROVENANCE_VERIFICATION_SIGNATURE = bytes.fromhex(
    "e5564300c360ac729086e2cc806e828a"
    "84877f1eb8e5d974d873e06522490155"
    "5fb8821590a33bacc61e39701cf9b46b"
    "d25bf5f0595bbe24655141438e7a100b"
)
TOPOLOGY_SIGNER_SERVICE_ID = "sorafs-sf11-topology-signer-a"
TOPOLOGY_SIGNER_ADMINISTRATOR_ID = "sorafs-sf11-topology-admin-b"
TOPOLOGY_SIGNER_KEY_REVISION = 7
TOPOLOGY_SIGNER_POLICY_REVISION = 9
TOPOLOGY_SIGNER_POLICY_DIGEST_HEX = hashlib.sha256(
    b"sorafs-sf11-topology-signer-policy-v1"
).hexdigest()
TOPOLOGY_SIGNING_SEED = hashlib.sha256(
    b"sorafs-sf11-topology-qualification-builder-test-key"
).digest()
TOPOLOGY_VERIFICATION_PUBLIC_KEY = public_key_from_seed(TOPOLOGY_SIGNING_SEED)
TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX = TOPOLOGY_VERIFICATION_PUBLIC_KEY.hex()
TOPOLOGY_MAX_REVIEW_AGE_SECS = 3_600


def supply_chain_result(
    *,
    generated_at_unix: int = GENERATED_AT,
) -> SupplyChainSourceResult:
    target_results = tuple(
        SupplyChainTargetResult(
            target=target,
            binary_smoke_passed=True,
            deterministic_archive_replay_passed=True,
            installation_verified=True,
            rollback_verified=True,
            yank_verified=True,
            sbom_generated=True,
            critical_vulnerability_count=0,
            high_vulnerability_count=0,
            oidc_identity_verified=True,
            cosign_provenance_verified=True,
        )
        for target in MODULE.REQUIRED_RELEASE_TARGETS
    )
    return SupplyChainSourceResult(
        generated_at_unix=generated_at_unix,
        deployment_id="reference-sdk-release-20260701",
        environment="production",
        release_manifest_digest_hex=MANIFEST_DIGEST,
        source_artifacts=(
            SourceArtifactBinding(
                "release_rehearsal",
                "release-rehearsal.json",
                RELEASE_REHEARSAL_DIGEST,
            ),
            SourceArtifactBinding("sbom_index", "sbom-index.json", SBOM_DIGEST),
            SourceArtifactBinding(
                "vulnerability_report",
                "vulnerability-report.json",
                VULNERABILITY_DIGEST,
            ),
            SourceArtifactBinding(
                "provenance_bundle",
                "provenance-bundle.json",
                PROVENANCE_DIGEST,
            ),
        ),
        target_results=target_results,
        sbom_index_digest_hex=SBOM_DIGEST,
        vulnerability_report_digest_hex=VULNERABILITY_DIGEST,
        provenance_bundle_digest_hex=PROVENANCE_DIGEST,
    )


@pytest.fixture(autouse=True)
def mock_supply_chain_source_validator(monkeypatch: pytest.MonkeyPatch) -> None:
    def validate(_source_root: Path, **kwargs):
        assert kwargs["expected_certificate_identity"] == (
            PROVENANCE_CERTIFICATE_IDENTITY
        )
        assert kwargs["expected_oidc_issuer"] == PROVENANCE_OIDC_ISSUER
        assert kwargs["release_rehearsal_path"] == "release-rehearsal.json"
        assert kwargs["sbom_index_path"] == "sbom-index.json"
        assert kwargs["vulnerability_report_path"] == (
            "vulnerability-report.json"
        )
        assert kwargs["provenance_bundle_path"] == "provenance-bundle.json"
        fingerprint = kwargs["expected_verification_key_fingerprint_hex"]
        authenticator = kwargs["verification_receipt_authenticator"]
        if not authenticator(
            fingerprint,
            b"",
            PROVENANCE_VERIFICATION_SIGNATURE,
        ):
            return None, ["trusted provenance receipt signature is invalid"]
        return supply_chain_result(), []

    monkeypatch.setattr(MODULE, "validate_supply_chain_sources", validate)
    monkeypatch.setitem(
        MODULE.validate_evidence_payload.__globals__,
        "validate_supply_chain_sources",
        validate,
    )
    monkeypatch.setattr(CHECKER, "validate_supply_chain_sources", validate)


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "reference-sdk-release-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if kind in MODULE.RELEASE_MANIFEST_BOUND_KINDS:
        args.extend(["--release-manifest-digest-hex", MANIFEST_DIGEST])
    if kind == "release_archive":
        args.extend(["--archive-index-digest-hex", ARCHIVE_DIGEST])
        for target in MODULE.REQUIRED_RELEASE_TARGETS:
            args.extend(["--target", target])
    elif kind == "signed_manifest":
        args.extend(
            [
                "--manifest-digest-hex",
                MANIFEST_DIGEST,
                "--public-key-fingerprint-hex",
                PUBLIC_KEY_DIGEST,
                "--policy-digest-hex",
                POLICY_DIGEST,
                "--signing-provider",
                "authenticated_external_signer",
                "--signing-backend",
                "software",
                "--signing-provider-revision",
                "7",
            ]
        )
    elif kind == "supply_chain":
        args.extend(
            [
                "--supply-chain-source-root",
                str(tmp_path / "supply-chain-sources"),
                "--provenance-certificate-identity",
                PROVENANCE_CERTIFICATE_IDENTITY,
                "--provenance-oidc-issuer",
                PROVENANCE_OIDC_ISSUER,
                "--provenance-verification-public-key-hex",
                PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX,
            ]
        )
    elif kind == "downstream_bindings":
        args.extend(["--package-index-digest-hex", PACKAGE_DIGEST])
        for package in MODULE.REQUIRED_DOWNSTREAM_PACKAGES:
            args.extend(["--package", package])
    elif kind == "cookbook_smoke":
        args.extend(["--smoke-output-digest-hex", SMOKE_DIGEST])
    elif kind == "ffi_header_contract":
        args.extend(
            [
                "--header-digest-hex",
                HEADER_DIGEST,
                "--ffi-contract-digest-hex",
                FFI_DIGEST,
            ]
        )
    elif kind == "governance_approval":
        args.extend(
            [
                "--policy-digest-hex",
                POLICY_DIGEST,
                "--public-key-fingerprint-hex",
                PUBLIC_KEY_DIGEST,
            ]
        )
    return args


def write_topology_qualification(path: Path) -> Path:
    payload = {
        "schema": "sorafs.l1.deployment_qualification.summary.v1",
        "status": "configuration-qualified",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": hashlib.sha256(b"builder-release-manifest").hexdigest(),
        "canonical_manifest_sha256": hashlib.sha256(
            b"canonical-builder-release-manifest"
        ).hexdigest(),
        "deployment": {
            "deployment_id": "reference-sdk-release-20260701",
            "environment": "production",
            "network": "taira",
            "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0",
            "chain_discriminant": 369,
        },
        "validator_count": 4, "validator_ids": ["taira-validator-1", "taira-validator-2", "taira-validator-3", "taira-validator-4"],
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": ["monitoring", "external_signer", "kms", "webauthn"],
        "runtime_material_policy_valid": True,
        "signed_model_artifact_count": 1,
        "required_lane_slots": list(TOPOLOGY.CANONICAL_READINESS_LANES),
        "recognized_lane_slot_count": len(TOPOLOGY.CANONICAL_READINESS_LANES),
        "errors": [],
    }
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    binding, errors = TOPOLOGY.load_topology_qualification_binding(
        path,
        expected_deployment_id="reference-sdk-release-20260701",
        expected_environment="production",
    )
    assert errors == []
    assert binding is not None
    envelope = {
        "schema": TOPOLOGY.SIGNED_QUALIFICATION_ENVELOPE_SCHEMA,
        **binding,
        "signer_authentication_kind": "external-ed25519",
        "signer_backend": "software",
        "signer_service_id": TOPOLOGY_SIGNER_SERVICE_ID,
        "signer_administrator_id": TOPOLOGY_SIGNER_ADMINISTRATOR_ID,
        "signer_key_revision": TOPOLOGY_SIGNER_KEY_REVISION,
        "signer_policy_revision": TOPOLOGY_SIGNER_POLICY_REVISION,
        "signer_public_key_fingerprint_sha256": hashlib.sha256(
            TOPOLOGY_VERIFICATION_PUBLIC_KEY
        ).hexdigest(),
        "signer_policy_digest_sha256": TOPOLOGY_SIGNER_POLICY_DIGEST_HEX,
        "reviewed_at_unix": NOW_UNIX - 60,
        "signature_algorithm": "ed25519",
        "signature_hex": "00" * 64,
    }
    envelope["signature_hex"] = sign(
        TOPOLOGY_SIGNING_SEED,
        TOPOLOGY.topology_qualification_envelope_signing_bytes(envelope),
    ).hex()
    topology_envelope_path(path).write_text(
        json.dumps(envelope, sort_keys=True),
        encoding="utf-8",
    )
    return path


def topology_envelope_path(qualification_path: Path) -> Path:
    """Return the deterministic signed companion for a topology summary."""

    return qualification_path.with_name(f"{qualification_path.name}.ed25519")


def assert_rejected_without_artifact(
    args: list[str],
    *,
    kind: str,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_builds_payload_free_release_archive_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("release_archive", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "release_archive").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reference_sdk.release_archive_canary.v1"
    assert payload["status"] == "passed"
    assert payload["release_manifest_digest_hex"] == MANIFEST_DIGEST
    assert payload["archive_index_digest_hex"] == ARCHIVE_DIGEST
    assert payload["raw_archives_included"] is False
    errors = MODULE.validate_generated_payload(
        payload,
        MODULE.parse_args(args_for("release_archive", tmp_path)),
    )
    assert errors == []


def test_generated_canaries_pass_full_reference_sdk_release_gate(
    tmp_path: Path,
) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    topology_summary = write_topology_qualification(
        tmp_path / "l1-topology.summary"
    )
    command = [
        "--now-unix",
        str(NOW_UNIX),
        "--topology-qualification-summary",
        str(topology_summary),
        "--topology-qualification-envelope",
        str(topology_envelope_path(topology_summary)),
        "--topology-qualification-verification-public-key-hex",
        TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX,
        "--topology-qualification-signer-service-id",
        TOPOLOGY_SIGNER_SERVICE_ID,
        "--topology-qualification-signer-administrator-id",
        TOPOLOGY_SIGNER_ADMINISTRATOR_ID,
        "--topology-qualification-signer-key-revision",
        str(TOPOLOGY_SIGNER_KEY_REVISION),
        "--topology-qualification-signer-policy-revision",
        str(TOPOLOGY_SIGNER_POLICY_REVISION),
        "--topology-qualification-signer-policy-digest-hex",
        TOPOLOGY_SIGNER_POLICY_DIGEST_HEX,
        "--max-topology-qualification-review-age-secs",
        str(TOPOLOGY_MAX_REVIEW_AGE_SECS),
        "--supply-chain-source-root",
        str(tmp_path / "supply-chain-sources"),
        "--provenance-certificate-identity",
        PROVENANCE_CERTIFICATE_IDENTITY,
        "--provenance-oidc-issuer",
        PROVENANCE_OIDC_ISSUER,
        "--provenance-verification-public-key-hex",
        PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX,
    ]
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_release_manifest_digests"] == [MANIFEST_DIGEST]
    assert payload["valid_release_manifest_reference_digests"] == [MANIFEST_DIGEST]
    assert payload["valid_release_key_fingerprints"] == [PUBLIC_KEY_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["valid_provenance_bundle_digests"] == [PROVENANCE_DIGEST]
    assert payload["valid_sbom_index_digests"] == [SBOM_DIGEST]
    assert payload["valid_vulnerability_report_digests"] == [
        VULNERABILITY_DIGEST
    ]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_signed_manifest_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "signed-manifest.args"
    args_file.write_text(
        "\n".join(args_for("signed_manifest", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "signed_manifest").read_text("utf-8"))
    assert payload["manifest_digest_hex"] == MANIFEST_DIGEST
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["private_key_absent"] is True
    assert payload["signing_provider"] == "authenticated_external_signer"
    assert payload["signing_backend"] == "software"
    assert payload["signing_provider_revision"] == 7
    assert payload["signer_response_verified"] is True
    assert "hsm_signature_verified" not in payload
    assert payload["raw_manifest_included"] is False


def test_builds_complete_supply_chain_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("supply_chain", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "supply_chain").read_text("utf-8"))
    assert payload["target_count"] == 5
    assert [row["target"] for row in payload["target_results"]] == list(
        MODULE.REQUIRED_RELEASE_TARGETS
    )
    assert all(row["high_vulnerability_count"] == 0 for row in payload["target_results"])
    assert all(row["cosign_provenance_verified"] for row in payload["target_results"])
    assert payload["sbom_index_digest_hex"] == SBOM_DIGEST
    assert payload["vulnerability_report_digest_hex"] == VULNERABILITY_DIGEST
    assert payload["provenance_bundle_digest_hex"] == PROVENANCE_DIGEST
    assert payload["source_artifacts"] == [
        {
            "kind": "release_rehearsal",
            "artifact_path": "release-rehearsal.json",
            "sha256": RELEASE_REHEARSAL_DIGEST,
        },
        {
            "kind": "sbom_index",
            "artifact_path": "sbom-index.json",
            "sha256": SBOM_DIGEST,
        },
        {
            "kind": "vulnerability_report",
            "artifact_path": "vulnerability-report.json",
            "sha256": VULNERABILITY_DIGEST,
        },
        {
            "kind": "provenance_bundle",
            "artifact_path": "provenance-bundle.json",
            "sha256": PROVENANCE_DIGEST,
        },
    ]
    assert (
        payload["provenance_certificate_identity"]
        == PROVENANCE_CERTIFICATE_IDENTITY
    )
    assert payload["provenance_oidc_issuer"] == PROVENANCE_OIDC_ISSUER
    assert (
        payload["provenance_verification_key_fingerprint_hex"]
        == PROVENANCE_VERIFICATION_KEY_FINGERPRINT
    )
    assert set(payload) == {
        "schema",
        "status",
        "generated_at_unix",
        "deployment_id",
        "environment",
        "deployment_context_reviewed",
        "target_count",
        "target_results",
        "source_artifacts",
        "release_manifest_digest_hex",
        "sbom_index_digest_hex",
        "vulnerability_report_digest_hex",
        "provenance_bundle_digest_hex",
        "provenance_certificate_identity",
        "provenance_oidc_issuer",
        "provenance_verification_key_fingerprint_hex",
        "raw_sboms_included",
        "raw_vulnerability_reports_included",
        "raw_provenance_included",
    }


def test_supply_chain_source_errors_fail_before_write(
    tmp_path: Path,
    capsys,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        MODULE,
        "validate_supply_chain_sources",
        lambda _source_root, **_kwargs: (None, ["SBOM index is stale"]),
    )

    assert MODULE.main(args_for("supply_chain", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "supply-chain source: SBOM index is stale" in captured.err
    assert not canary_path(tmp_path, "supply_chain").exists()


@pytest.mark.parametrize(
    "required_option",
    (
        "--supply-chain-source-root",
        "--provenance-certificate-identity",
        "--provenance-oidc-issuer",
        "--provenance-verification-public-key-hex",
    ),
)
def test_supply_chain_requires_source_and_trust_inputs(
    required_option: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("supply_chain", tmp_path)
    index = args.index(required_option)
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert f"{required_option} is required for supply_chain" in captured.err
    assert not canary_path(tmp_path, "supply_chain").exists()


def test_supply_chain_timestamp_must_equal_oldest_source(
    tmp_path: Path,
    capsys,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        MODULE,
        "validate_supply_chain_sources",
        lambda _source_root, **_kwargs: (
            supply_chain_result(generated_at_unix=GENERATED_AT - 1),
            [],
        ),
    )

    assert MODULE.main(args_for("supply_chain", tmp_path)) == 2

    captured = capsys.readouterr()
    assert (
        "--generated-at-unix must equal the oldest validated "
        "supply-chain source timestamp"
    ) in captured.err
    assert not canary_path(tmp_path, "supply_chain").exists()


def test_supply_chain_authenticator_binds_ed25519_key_and_fingerprint() -> None:
    public_key = bytes.fromhex(PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX)
    fingerprint, authenticate = MODULE.provenance_receipt_authenticator(public_key)

    assert fingerprint == PROVENANCE_VERIFICATION_KEY_FINGERPRINT
    assert authenticate(
        fingerprint,
        b"",
        PROVENANCE_VERIFICATION_SIGNATURE,
    )
    assert not authenticate(
        "f" * 64,
        b"",
        PROVENANCE_VERIFICATION_SIGNATURE,
    )
    assert not authenticate(
        fingerprint,
        b"tampered",
        PROVENANCE_VERIFICATION_SIGNATURE,
    )


def test_supply_chain_default_source_paths_are_exact_v1_names(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(args_for("supply_chain", tmp_path))

    assert args.release_rehearsal_path == "release-rehearsal.json"
    assert args.sbom_index_path == "sbom-index.json"
    assert args.vulnerability_report_path == "vulnerability-report.json"
    assert args.provenance_bundle_path == "provenance-bundle.json"


def test_supply_chain_rejects_wrong_verification_public_key(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("supply_chain", tmp_path)
    index = args.index("--provenance-verification-public-key-hex")
    args[index + 1] = "01" * 32

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "trusted provenance receipt signature is invalid" in captured.err
    assert not canary_path(tmp_path, "supply_chain").exists()


def test_supply_chain_rejects_manual_target(tmp_path: Path, capsys) -> None:
    args = args_for("supply_chain", tmp_path)
    args.extend(["--target", MODULE.REQUIRED_RELEASE_TARGETS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--target is retired for supply_chain; targets are source-derived"
        in captured.err
    )
    assert not canary_path(tmp_path, "supply_chain").exists()


@pytest.mark.parametrize(
    "retired_flag",
    (
        "--sbom-index-digest-hex",
        "--vulnerability-report-digest-hex",
        "--provenance-bundle-digest-hex",
    ),
)
def test_supply_chain_rejects_retired_manual_digest_flags(
    retired_flag: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("supply_chain", tmp_path)
    args.extend([retired_flag, "a" * 64])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "unrecognized arguments" in captured.err
    assert retired_flag in captured.err
    assert not canary_path(tmp_path, "supply_chain").exists()


def test_policy_digest_kind_inventory_matches_generated_payloads(tmp_path: Path) -> None:
    assert MODULE.POLICY_DIGEST_KINDS == ("signed_manifest", "governance_approval")

    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        payload = json.loads(canary_path(tmp_path, kind).read_text("utf-8"))
        if kind in MODULE.POLICY_DIGEST_KINDS:
            assert payload["policy_digest_hex"] == POLICY_DIGEST
        else:
            assert "policy_digest_hex" not in payload


def test_signed_manifest_requires_policy_digest_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("signed_manifest", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for signed_manifest" in captured.err
    assert not canary_path(tmp_path, "signed_manifest").exists()


def test_signed_manifest_rejects_unsupported_signature_algorithm_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("signed_manifest", tmp_path)
    args.extend(["--signature-algorithm", "none"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--signature-algorithm must be `ed25519`" in captured.err
    assert not canary_path(tmp_path, "signed_manifest").exists()


@pytest.mark.parametrize("provider", ("external_ed25519_hsm", "local_file"))
def test_signed_manifest_rejects_legacy_or_unapproved_provider_before_write(
    tmp_path: Path,
    capsys,
    provider: str,
) -> None:
    args = args_for("signed_manifest", tmp_path)
    index = args.index("--signing-provider")
    args[index + 1] = provider

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--signing-provider must be `authenticated_external_signer`"
        in captured.err
    )
    assert not canary_path(tmp_path, "signed_manifest").exists()


def test_signed_manifest_rejects_non_software_backend_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("signed_manifest", tmp_path)
    index = args.index("--signing-backend")
    args[index + 1] = "hsm"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--signing-backend must be `software`" in captured.err
    assert not canary_path(tmp_path, "signed_manifest").exists()


@pytest.mark.parametrize(
    "option",
    ("--policy-digest-hex", "--public-key-fingerprint-hex"),
)
def test_signed_manifest_rejects_zero_policy_or_key_binding_before_write(
    tmp_path: Path,
    capsys,
    option: str,
) -> None:
    args = args_for("signed_manifest", tmp_path)
    index = args.index(option)
    args[index + 1] = "0" * 64

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert f"{option} must not be zero" in captured.err
    assert not canary_path(tmp_path, "signed_manifest").exists()


def test_signed_manifest_rejects_nonpositive_provider_revision_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("signed_manifest", tmp_path)
    index = args.index("--signing-provider-revision")
    args[index + 1] = "0"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "argument --signing-provider-revision: must be positive" in captured.err
    assert not canary_path(tmp_path, "signed_manifest").exists()


def test_governance_approval_canary_binds_public_key_fingerprint(
    tmp_path: Path,
) -> None:
    assert MODULE.main(args_for("governance_approval", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "governance_approval").read_text("utf-8"))

    assert payload["release_manifest_digest_hex"] == MANIFEST_DIGEST
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["public_key_fingerprint_hex"] == PUBLIC_KEY_DIGEST


def test_governance_approval_requires_public_key_fingerprint_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    index = args.index("--public-key-fingerprint-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--public-key-fingerprint-hex is required for governance_approval"
        in captured.err
    )
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_governance_approval_rejects_malformed_public_key_fingerprint_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_approval", tmp_path)
    index = args.index("--public-key-fingerprint-hex")
    args[index + 1] = "not-hex"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--public-key-fingerprint-hex must be exact lowercase 32-byte hex"
        in captured.err
    )
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_signed_manifest_canary_rejects_rsa_sha256_signature_algorithm(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("signed_manifest", tmp_path)
    args.extend(["--signature-algorithm", "rsa-sha256"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--signature-algorithm must be `ed25519`" in captured.err
    assert not canary_path(tmp_path, "signed_manifest").exists()


def test_missing_release_target_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("release_archive", tmp_path)
    index = args.index("--target")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--target must include every required value" in captured.err
    assert not canary_path(tmp_path, "release_archive").exists()


def test_duplicate_release_target_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("release_archive", tmp_path)
    args.extend(["--target", MODULE.REQUIRED_RELEASE_TARGETS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--target must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "release_archive").exists()


def test_unknown_release_target_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("release_archive", tmp_path)
    args.extend(["--target", "shadow-release-target"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--target contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "release_archive").exists()


def test_missing_downstream_package_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("downstream_bindings", tmp_path)
    index = args.index("--package")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--package must include every required value" in captured.err
    assert not canary_path(tmp_path, "downstream_bindings").exists()


def test_duplicate_downstream_package_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("downstream_bindings", tmp_path)
    args.extend(["--package", MODULE.REQUIRED_DOWNSTREAM_PACKAGES[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--package must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "downstream_bindings").exists()


def test_unknown_downstream_package_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("downstream_bindings", tmp_path)
    args.extend(["--package", "shadow-sdk-package"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--package contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "downstream_bindings").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "release_archive",
            "--target",
            MODULE.REQUIRED_RELEASE_TARGETS[0],
            "shadow-release-target",
        ),
        (
            "downstream_bindings",
            "--package",
            MODULE.REQUIRED_DOWNSTREAM_PACKAGES[0],
            "shadow-sdk-package",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    kind: str,
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for(kind, tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for(kind, unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_smoke_duration_threshold_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("cookbook_smoke", tmp_path)
    args.extend(["--smoke-duration-seconds", "1801"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--smoke-duration-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "cookbook_smoke").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("governance_approval", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_refused(tmp_path: Path, capsys) -> None:
    directory = tmp_path / "out-dir"
    directory.mkdir()
    args = args_for("governance_approval", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(directory)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a directory" in captured.err
