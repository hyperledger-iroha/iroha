#!/usr/bin/env python3
"""Source-only tests for the fail-closed Vega Figure 9 artifact corridor.

No test in this module invents setup bytes or substitutes a mock cryptographic
validator. Successful package qualification is intentionally reserved for the
real governed full-shape PK/VK pair.
"""

from __future__ import annotations

import hashlib
import importlib.util
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
OWNER = ROOT / "scripts" / "package_vega_figure9_artifacts.py"
RUST_TOOL = ROOT / "crates" / "iroha_core" / "src" / "bin" / "vega_figure9_artifact_tool.rs"
ARTIFACT_OWNER = ROOT / "crates" / "iroha_core" / "src" / "privacy_engines" / "vega" / "artifacts.rs"
EVIDENCE_OWNER = ROOT / "crates" / "iroha_core" / "src" / "privacy_release_evidence.rs"
CARGO_MANIFEST = ROOT / "crates" / "iroha_core" / "Cargo.toml"
SPEC = importlib.util.spec_from_file_location("package_vega_figure9_artifacts", OWNER)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def public_schema_report() -> dict[str, object]:
    """Build public identities only; these are not key or proof fixtures."""
    evidence: list[dict[str, object]] = []
    for index, (case_kind, file_name, failure_class) in enumerate(MODULE.EVIDENCE_FILES):
        evidence.append(
            {
                "archive_sha256": format(index + 1, "x") * 64,
                "case_kind": case_kind,
                "exact_byte_len": 201 + index,
                "file_name": file_name,
                "failure_class": failure_class,
                "proof_artifacts": [
                    {
                        "artifact_ordinal": 0,
                        "canonical_proof_exact_byte_len": 101 + index,
                        "proof_bytes_ceiling": MODULE.VEGA_PROOF_BYTES_CEILING,
                        "proof_sha256": format(index + 5, "x") * 64,
                    }
                ],
                "protocol_id": MODULE.VEGA_PROTOCOL_ID,
                "public_statement_sha256": format(index + 9, "x") * 64,
                "resources": {
                    "primary_ceiling": MODULE.VEGA_PRIMARY_UNITS,
                    "primary_units": MODULE.VEGA_PRIMARY_UNITS,
                    "relation_depth": MODULE.VEGA_RELATION_DEPTH,
                    "relation_depth_ceiling": MODULE.VEGA_RELATION_DEPTH,
                    "secondary_ceiling": MODULE.VEGA_SECONDARY_UNITS,
                    "secondary_units": MODULE.VEGA_SECONDARY_UNITS,
                },
                "stage_ordinal": 16 + index,
            }
        )
    report: dict[str, object] = {
        "artifact_manifest_schema": MODULE.ARTIFACT_MANIFEST_SCHEMA,
        "artifact_manifest_schema_version": MODULE.ARTIFACT_MANIFEST_SCHEMA_VERSION,
        "artifact_manifest_sha256": "1" * 64,
        "canonical_relation_digest": MODULE.CANONICAL_RELATION_DIGEST,
        "cargo_lock_sha256": MODULE.CARGO_LOCK_SHA256,
        "compiled_profile_digest": MODULE.COMPILED_PROFILE_DIGEST,
        "evidence": evidence,
        "evidence_set_sha256": MODULE._evidence_set_digest(evidence),
        "iroha_signed_source_commit": "3" * 40,
        "logical_governed_verifier_digest": MODULE.LOGICAL_GOVERNED_VERIFIER_DIGEST,
        "proving_key": {
            "exact_byte_len": 101,
            "raw_canonical_sha256": "4" * 64,
            "role": "proving-key",
        },
        "release_qualification": "passed-native-four-case",
        "schema": MODULE.NATIVE_SCHEMA,
        "schema_version": MODULE.NATIVE_SCHEMA_VERSION,
        "source_allowed_signers_sha256": "5" * 64,
        "source_revocation_sha256": "6" * 64,
        "upstream_source_commit": MODULE.UPSTREAM_SOURCE_COMMIT,
        "upstream_source_tree": MODULE.UPSTREAM_SOURCE_TREE,
        "validator_arch": "schema-test-arch",
        "validator_os": "schema-test-os",
        "validator_role": "prover-pair-and-four-case-release-evidence",
        "vendor_manifest_sha256": MODULE.VENDOR_MANIFEST_SHA256,
        "verifier_key": {
            "exact_byte_len": 103,
            "raw_canonical_sha256": "7" * 64,
            "role": "verifier-key",
        },
        "workspace_source_manifest_sha256": "8" * 64,
    }
    report["artifact_manifest_sha256"] = MODULE._manifest_digest(report)
    return report


def public_file_identity(name: str, size: int, digest_byte: str, mode: int):
    """Return one public package-file identity without creating artifact bytes."""
    return MODULE.FileIdentity(
        path=Path("/schema-only") / name,
        size=size,
        sha256=digest_byte * 64,
        mode=mode,
    )


def public_candidate_manifest() -> dict[str, object]:
    """Build the canonical public package schema without qualifying a package."""
    return MODULE._candidate_manifest(
        public_schema_report(),
        public_file_identity(MODULE.NATIVE_VALIDATOR_FILE, 107, "9", 0o500),
        public_file_identity(MODULE.PROVING_KEY_FILE, 101, "4", 0o400),
        public_file_identity(MODULE.VERIFIER_KEY_FILE, 103, "7", 0o400),
        public_file_identity(MODULE.NATIVE_REPORT_FILE, 109, "a", 0o400),
        {
            file_name: public_file_identity(file_name, 201 + index, format(index + 1, "x"), 0o400)
            for index, (_, file_name, _) in enumerate(MODULE.EVIDENCE_FILES)
        },
    )


class VegaFigure9ArtifactPackageTests(unittest.TestCase):
    def test_unreleased_source_pins_and_exact12_stage_coordinates_remain_fail_closed(self) -> None:
        artifact_source = ARTIFACT_OWNER.read_text()
        open_pin_patterns = (
            r"const VEGA_MDL_FIGURE9_RELEASE_PROVING_KEY_EXACT_BYTES_V1: u64 = 0;",
            r"const VEGA_MDL_FIGURE9_RELEASE_PROVING_KEY_RAW_SHA256_V1: \[u8; 32\] = \[0; 32\];",
            r"const VEGA_MDL_FIGURE9_RELEASE_VERIFIER_KEY_EXACT_BYTES_V1: u64 = 0;",
            r"const VEGA_MDL_FIGURE9_RELEASE_VERIFIER_KEY_RAW_SHA256_V1: \[u8; 32\] = \[0; 32\];",
            r"const VEGA_MDL_FIGURE9_RELEASE_ARTIFACT_MANIFEST_SHA256_V1: \[u8; 32\] = \[0; 32\];",
            r"const VEGA_MDL_FIGURE9_RELEASE_PACKAGE_SHA256_V1: \[u8; 32\] = \[0; 32\];",
            r"const VEGA_MDL_FIGURE9_RELEASE_EVIDENCE_SET_SHA256_V1: \[u8; 32\] = \[0; 32\];",
            r"const VEGA_MDL_FIGURE9_RELEASE_GOVERNANCE_AUTHORIZATION_SHA256_V1: \[u8; 32\] = \[0; 32\];",
        )
        for pattern in open_pin_patterns:
            self.assertEqual(len(re.findall(pattern, artifact_source)), 1, pattern)
        self.assertIn(
            'VEGA_FIGURE9_RELEASE_READINESS_BLOCKER_V1: &str = "MissingGovernedFigure9ProverArtifacts"',
            artifact_source,
        )

        evidence_source = EVIDENCE_OWNER.read_text()
        schedule = evidence_source.split(
            "pub const PRIVACY_RELEASE_STAGE_COORDINATES_V1", maxsplit=1
        )[1].split("const _: () = assert!(PRIVACY_RELEASE_STAGE_COUNT_V1 == 48);", maxsplit=1)[0]
        coordinates = re.findall(
            r"privacy_release_stage_coordinate_v1\(\s*(\d+),\s*"
            r"PrivacyProtocolIdV1::([A-Za-z0-9_]+),\s*"
            r"PrivacyReleaseCaseKindV1::([A-Za-z0-9_]+),\s*\)",
            schedule,
        )
        self.assertEqual(len(coordinates), 48)
        self.assertEqual([int(ordinal) for ordinal, _, _ in coordinates], list(range(48)))
        self.assertEqual(
            coordinates[16:20],
            [
                ("16", "VegaExistingCredentialZkV0", "PositiveCanonicalEndToEnd"),
                ("17", "VegaExistingCredentialZkV0", "PublicStatementBindingMutation"),
                ("18", "VegaExistingCredentialZkV0", "ProofCorruptionAndTruncation"),
                ("19", "VegaExistingCredentialZkV0", "MaximumShapeResource"),
            ],
        )
        self.assertEqual(
            [file_name for _, file_name, _ in MODULE.EVIDENCE_FILES],
            [
                "vega-evidence-16-positive-canonical-end-to-end.norito",
                "vega-evidence-17-public-statement-binding-mutation.norito",
                "vega-evidence-18-proof-corruption-and-truncation.norito",
                "vega-evidence-19-maximum-shape-resource.norito",
            ],
        )

    def test_candidate_manifest_is_canonical_closed_and_not_available(self) -> None:
        manifest = public_candidate_manifest()
        report, files = MODULE._validate_package_manifest(manifest)
        self.assertEqual(report, manifest["native_validation"])
        self.assertEqual(
            set(files),
            {
                "native_report",
                "native_validator",
                "proving_key",
                "verifier_key",
                *MODULE.EVIDENCE_FILE_NAMES,
            },
        )
        self.assertEqual(manifest["availability"], "unavailable-pending-reviewed-governance")
        self.assertFalse(manifest["network_activation_authorized"])
        self.assertEqual(manifest["native_release_qualification"], "passed-native-four-case")
        self.assertEqual(manifest["release_boundary"], "candidate-only")
        encoded = MODULE._canonical_json(manifest)
        self.assertEqual(MODULE._strict_json(encoded, "schema-only candidate"), manifest)
        self.assertRegex(hashlib.sha256(encoded).hexdigest(), r"^[0-9a-f]{64}$")

    def test_candidate_manifest_cannot_authorize_or_expand_itself(self) -> None:
        mutations = (
            ("availability", "available"),
            ("network_activation_authorized", True),
            ("native_release_qualification", "pair-only"),
            ("release_boundary", "governed-active"),
            ("schema", "iroha.vega.figure9.other"),
        )
        for field, value in mutations:
            with self.subTest(field=field):
                manifest = public_candidate_manifest()
                manifest[field] = value
                with self.assertRaisesRegex(MODULE.Refusal, "not fail closed"):
                    MODULE._validate_package_manifest(manifest)
        manifest = public_candidate_manifest()
        manifest["schema_version"] = MODULE.PACKAGE_SCHEMA_VERSION + 1
        with self.assertRaisesRegex(MODULE.Refusal, "exact integer"):
            MODULE._validate_package_manifest(manifest)
        manifest = public_candidate_manifest()
        manifest["files"]["undeclared"] = {
            "mode": "0400",
            "path": "undeclared.bin",
            "sha256": "b" * 64,
            "size": 1,
        }
        with self.assertRaisesRegex(MODULE.Refusal, "not closed"):
            MODULE._validate_package_manifest(manifest)

    def test_native_manifest_digest_and_profile_drift_are_rejected(self) -> None:
        report = public_schema_report()
        report["artifact_manifest_sha256"] = "1" * 64
        with self.assertRaisesRegex(MODULE.Refusal, "not reproducible"):
            MODULE._validate_native_report(report)
        report = public_schema_report()
        report["compiled_profile_digest"] = "c" * 64
        with self.assertRaisesRegex(MODULE.Refusal, "released profile"):
            MODULE._validate_native_report(report)

        report = public_schema_report()
        report["evidence"][2]["proof_artifacts"][0]["canonical_proof_exact_byte_len"] = 0
        with self.assertRaisesRegex(MODULE.Refusal, "proof length"):
            MODULE._validate_native_report(report)

        report = public_schema_report()
        report["evidence_set_sha256"] = "f" * 64
        with self.assertRaisesRegex(MODULE.Refusal, "not reproducible"):
            MODULE._validate_native_report(report)

    def test_json_numeric_lookalikes_cannot_satisfy_fixed_integer_coordinates(self) -> None:
        mutations = (
            ("native schema version", lambda report: report.__setitem__("schema_version", 2.0)),
            (
                "artifact schema version",
                lambda report: report.__setitem__("artifact_manifest_schema_version", 1.0),
            ),
            (
                "stage ordinal",
                lambda report: report["evidence"][0].__setitem__("stage_ordinal", 16.0),
            ),
            (
                "proof ordinal",
                lambda report: report["evidence"][0]["proof_artifacts"][0].__setitem__(
                    "artifact_ordinal", 0.0
                ),
            ),
            (
                "proof ceiling",
                lambda report: report["evidence"][0]["proof_artifacts"][0].__setitem__(
                    "proof_bytes_ceiling", float(MODULE.VEGA_PROOF_BYTES_CEILING)
                ),
            ),
            (
                "resource coordinate",
                lambda report: report["evidence"][0]["resources"].__setitem__(
                    "primary_units", float(MODULE.VEGA_PRIMARY_UNITS)
                ),
            ),
        )
        for label, mutate in mutations:
            with self.subTest(label=label):
                report = public_schema_report()
                mutate(report)
                with self.assertRaisesRegex(MODULE.Refusal, "exact integer"):
                    MODULE._validate_native_report(report)

        manifest = public_candidate_manifest()
        manifest["schema_version"] = float(MODULE.PACKAGE_SCHEMA_VERSION)
        with self.assertRaisesRegex(MODULE.Refusal, "exact integer"):
            MODULE._validate_package_manifest(manifest)

    def test_validator_digest_mismatch_stops_before_key_lookup_or_execution(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            root.chmod(0o700)
            validator = root / "unreviewed-owner-executable"
            validator.write_bytes(b"not executed")
            validator.chmod(0o500)
            output = root / "packages"
            output.mkdir(mode=0o700)
            absent = root / "intentionally-absent-artifact"
            with self.assertRaisesRegex(MODULE.Refusal, "reviewed digest"):
                MODULE.package(validator, "f" * 64, absent, absent, output)
            self.assertEqual(list(output.iterdir()), [])

    def test_vendor_source_manifest_is_bounded_reproducible_and_required_before_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            (root / MODULE.VENDOR_PROVENANCE_FILE).write_text("excluded provenance\n")
            (root / "a.txt").write_bytes(b"alpha\n")
            nested = root / "nested"
            nested.mkdir()
            (nested / "b.bin").write_bytes(b"beta\0")
            lines = b"".join(
                (
                    hashlib.sha256(b"alpha\n").hexdigest().encode() + b"  a.txt\n",
                    hashlib.sha256(b"beta\0").hexdigest().encode() + b"  nested/b.bin\n",
                )
            )
            expected = hashlib.sha256(lines).hexdigest()
            self.assertEqual(MODULE._vendor_source_manifest_sha256(root), expected)
            with mock.patch.object(MODULE, "VENDOR_SOURCE_ROOT", root), mock.patch.object(
                MODULE, "VENDOR_MANIFEST_SHA256", expected
            ):
                MODULE._require_reviewed_vendor_source_manifest()
            with mock.patch.object(MODULE, "VENDOR_SOURCE_ROOT", root), mock.patch.object(
                MODULE, "VENDOR_MANIFEST_SHA256", "f" * 64
            ):
                with self.assertRaisesRegex(MODULE.Refusal, "does not reproduce"):
                    MODULE._require_reviewed_vendor_source_manifest()

            validator = root / "reviewed-owner-executable"
            validator.write_bytes(b"not executed")
            validator.chmod(0o500)
            validator_sha256 = hashlib.sha256(b"not executed").hexdigest()
            output = root / "packages"
            output.mkdir(mode=0o700)
            absent = root / "intentionally-absent-artifact"
            with mock.patch.object(
                MODULE,
                "_require_reviewed_vendor_source_manifest",
                side_effect=MODULE.Refusal("source seal refused before keys"),
            ):
                with self.assertRaisesRegex(MODULE.Refusal, "source seal refused before keys"):
                    MODULE.package(validator, validator_sha256, absent, absent, output)
            self.assertEqual(list(output.iterdir()), [])

    def test_file_boundary_rejects_shared_links_and_permissions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            root.chmod(0o700)
            candidate = root / "public-metadata"
            candidate.write_bytes(b"public schema test data")
            candidate.chmod(0o400)
            identity = MODULE._file_identity(candidate, "public metadata", 1024)
            self.assertEqual(identity.size, candidate.stat().st_size)

            candidate.chmod(0o440)
            with self.assertRaisesRegex(MODULE.Refusal, "owner-only"):
                MODULE._file_identity(candidate, "public metadata", 1024)
            candidate.chmod(0o400)

            linked = root / "second-link"
            os.link(candidate, linked)
            with self.assertRaisesRegex(MODULE.Refusal, "singly linked"):
                MODULE._file_identity(candidate, "public metadata", 1024)

    def test_native_boundary_copies_before_execution_and_has_no_setup_path(self) -> None:
        rust_source = RUST_TOOL.read_text()
        package_source = OWNER.read_text().split("def package(", maxsplit=1)[1].split(
            "def _parser()", maxsplit=1
        )[0]
        cargo_manifest = CARGO_MANIFEST.read_text()
        first_validation = package_source.index("report, report_bytes, first_evidence = _run_native_validator(")
        for copied in (
            "packaged_validator = _copy_file(",
            "packaged_proving_key = _copy_file(",
            "packaged_verifier_key = _copy_file(",
        ):
            self.assertLess(package_source.index(copied), first_validation)
        self.assertIn("qualify_and_install_vega_mdl_figure9_prover_artifacts_v1", rust_source)
        self.assertIn("run_privacy_release_stage_v1", rust_source)
        self.assertIn("PrivacyReleaseCaseKindV1::ALL", rust_source)
        self.assertIn("validate_privacy_release_stage_evidence_v1", rust_source)
        self.assertIn("norito::decode_canonical", rust_source)
        self.assertIn("--evidence-output", rust_source)
        self.assertIn("VegaMdlFigure9ProverArtifactSourceV1", rust_source)
        self.assertIn('option_env!("IROHA_VEGA_SIGNED_SOURCE_COMMIT")', rust_source)
        self.assertIn('name = "vega_figure9_artifact_tool"', cargo_manifest)
        production = rust_source.split("fn main()", maxsplit=1)[0]
        for forbidden in (
            "std::net",
            "TcpStream",
            "UdpSocket",
            "generate_vega",
            "setup_vega",
            "from_env(proving",
            "validate-prover-pair",
        ):
            self.assertNotIn(forbidden, production)


if __name__ == "__main__":
    unittest.main()
