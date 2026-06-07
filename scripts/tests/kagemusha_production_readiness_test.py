"""Tests for scripts/kagemusha_production_readiness.py."""

from __future__ import annotations

import importlib.util
import hashlib
import io
import json
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import tempfile
import unittest


SCRIPT_DIR = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_DIR / "kagemusha_production_readiness.py"
SPEC = importlib.util.spec_from_file_location("kagemusha_production_readiness", MODULE_PATH)
assert SPEC and SPEC.loader  # pragma: no cover - import guard
readiness = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(readiness)  # type: ignore[misc]

EVIDENCE_HELPER_PATH = SCRIPT_DIR / "kagemusha_lineage_proof_evidence.py"
EVIDENCE_HELPER_SPEC = importlib.util.spec_from_file_location(
    "kagemusha_lineage_proof_evidence",
    EVIDENCE_HELPER_PATH,
)
assert EVIDENCE_HELPER_SPEC and EVIDENCE_HELPER_SPEC.loader  # pragma: no cover - import guard
evidence_helper = importlib.util.module_from_spec(EVIDENCE_HELPER_SPEC)
EVIDENCE_HELPER_SPEC.loader.exec_module(evidence_helper)  # type: ignore[misc]

SLOT_HELPER_PATH = SCRIPT_DIR / "tests" / "check_android_device_lab_slot_test.py"
SLOT_HELPER_SPEC = importlib.util.spec_from_file_location(
    "check_android_device_lab_slot_test",
    SLOT_HELPER_PATH,
)
assert SLOT_HELPER_SPEC and SLOT_HELPER_SPEC.loader  # pragma: no cover - import guard
slot_helpers = importlib.util.module_from_spec(SLOT_HELPER_SPEC)
SLOT_HELPER_SPEC.loader.exec_module(slot_helpers)  # type: ignore[misc]


REPO_ROOT = Path(__file__).resolve().parents[2]


def create_complete_matrix(root: Path, signer: dict[str, Path | str]) -> None:
    for index, family in enumerate(slot_helpers.device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
        slot_helpers.create_slot(root, f"slot-{index}", family, signer)


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def create_lineage_proof_evidence(root: Path) -> Path:
    create_lineage_artifact_files(root)
    proof_log = root / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
    write_passing_lineage_proof_log(proof_log)
    evidence_path = root / "lineage-proof-evidence.json"
    write_json(
        evidence_path,
        {
            "schema": readiness.LINEAGE_PROOF_EVIDENCE_SCHEMA,
            "generated_at_utc": readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            "opening_len": readiness.EXPECTED_LINEAGE_PROOF_OPENING_LEN,
            "ipa_k": readiness.EXPECTED_LINEAGE_PROOF_IPA_K,
            "verifier_backend": readiness.EXPECTED_LINEAGE_PROOF_BACKEND,
            "verifier_witness_profile": readiness.EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE,
            "record_archive_proof_runtime_keygen_env": "unset",
            "circuit_ids": dict(readiness.EXPECTED_LINEAGE_CIRCUIT_IDS),
            "artifacts": {
                artifact: hashlib.sha256((root / artifact).read_bytes()).hexdigest()
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            },
            "tests": {
                "record_archive_proof": {
                    "name": readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
                    "status": "passed",
                    "ignored": True,
                    "command": readiness.expected_lineage_proof_command(
                        readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
                    ),
                    "elapsed_seconds": 14400.0,
                    "log_path": readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                        "record_archive_proof"
                    ],
                    "log_sha256": hashlib.sha256(proof_log.read_bytes()).hexdigest(),
                }
            },
        },
    )
    return evidence_path


def create_lineage_artifact_files(root: Path) -> None:
    for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS:
        path = root / artifact
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f"lineage artifact {artifact}\n".encode("utf-8"))


def write_passing_lineage_proof_log(path: Path) -> None:
    test_name = readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
    path.write_text(
        "\n".join(
            (
                "running 1 test",
                f"test {test_name} ... ok",
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14400.00s",
                "",
            )
        ),
        encoding="utf-8",
    )


def write_abi7_fail_closed_marker_files(repo: Path) -> None:
    core_path = repo / "crates/iroha_core/src/zk.rs"
    core_path.parent.mkdir(parents=True, exist_ok=True)
    core_path.write_text(
        "\n".join(
            (
                "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
                "semantic ABI-7 compact tokens are disabled for production",
                "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
                "pub fn verify_kagemusha_recursive_compact_payment_token(",
                "false",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    bridge_path = repo / "crates/connect_norito_bridge/src/lib.rs"
    bridge_path.parent.mkdir(parents=True, exist_ok=True)
    bridge_path.write_text(
        "ERR_KAGEMUSHA_RECURSIVE_COMPACT_UNAVAILABLE\n",
        encoding="utf-8",
    )


def write_lineage_key_release_tooling_marker_files(repo: Path) -> None:
    for relative, snippets in readiness.LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS.items():
        path = repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(snippets) + "\n", encoding="utf-8")


def resign_slot_evidence_with_timestamp(
    slot: Path,
    signer: dict[str, Path | str],
    signed_at_utc: str,
) -> None:
    evidence_path = slot / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    evidence["signed_at_utc"] = signed_at_utc
    write_json(evidence_path, slot_helpers.sign_evidence(evidence, signer))
    slot_helpers.refresh_signed_evidence_hash(slot)


def copy_slot_binding(
    *,
    source: Path,
    target: Path,
    signer: dict[str, Path | str],
    key: str,
) -> None:
    """Copy one signed slot binding across all artifacts for adversarial tests."""

    source_metadata = json.loads((source / "slot.json").read_text(encoding="utf-8"))
    copied = source_metadata[key]

    metadata_path = target / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata[key] = copied
    write_json(metadata_path, metadata)

    attestation_path = target / "attestation" / "result.json"
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    if key in attestation:
        attestation[key] = copied
        write_json(attestation_path, attestation)

    transcript_path = target / "handoff" / "d2d-payment.json"
    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
    if key in transcript:
        transcript[key] = copied
        write_json(transcript_path, transcript)
        slot_helpers.refresh_d2d_payment_transcript_hash(target, signer)

    wallet_transcript_path = target / "wallet" / "integrity.json"
    wallet_transcript = json.loads(wallet_transcript_path.read_text(encoding="utf-8"))
    if key in wallet_transcript:
        wallet_transcript[key] = copied
        write_json(wallet_transcript_path, wallet_transcript)
        slot_helpers.refresh_wallet_integrity_transcript_hash(target, signer)

    evidence_path = target / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    if key in evidence:
        evidence[key] = copied
    evidence["artifact_digests"] = slot_helpers.required_artifact_digests(target)
    write_json(evidence_path, slot_helpers.sign_evidence(evidence, signer))
    slot_helpers.refresh_signed_evidence_hash(target)


class KagemushaProductionReadinessTest(unittest.TestCase):
    def test_complete_signed_android_matrix_passes_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            create_complete_matrix(root, signer)

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            expected_artifact_sha256 = {
                artifact: hashlib.sha256(
                    (lineage_evidence.parent / artifact).read_bytes()
                ).hexdigest()
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            expected_log_sha256 = hashlib.sha256(
                (
                    lineage_evidence.parent
                    / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
                ).read_bytes()
            ).hexdigest()
            expected_android_signed_evidence = {}
            for index in range(
                len(slot_helpers.device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES)
            ):
                slot_id = f"slot-{index}"
                slot = root / slot_id
                metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
                evidence = json.loads(
                    (slot / "evidence" / "signed-evidence.json").read_text(
                        encoding="utf-8"
                    )
                )
                expected_android_signed_evidence[slot_id] = {
                    "artifact_sha256": metadata["signed_evidence_artifact_sha256"],
                    "signed_at_utc": evidence["signed_at_utc"],
                    "signer_public_key_sha256": evidence["signer_public_key_sha256"],
                }

        self.assertEqual(status, 0)
        self.assertTrue(summary["ready"])
        self.assertEqual(summary["status"], "ready")
        self.assertEqual(summary["schema"], readiness.SUMMARY_SCHEMA)
        self.assertTrue(summary["abi6_reserved_lineage"]["ok"])
        self.assertEqual(summary["abi7_recursive_compact"]["state"], "fail_closed")
        self.assertEqual(
            summary["lineage_key_release_tooling"]["state"],
            "record_artifacts_wired",
        )
        self.assertEqual(
            summary["lineage_proof_evidence"]["state"],
            "production_width_proof_passed",
        )
        self.assertEqual(
            summary["lineage_proof_evidence"]["generated_at_utc"],
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        )
        self.assertEqual(
            summary["lineage_proof_evidence"]["artifact_sha256"],
            expected_artifact_sha256,
        )
        self.assertEqual(
            summary["lineage_proof_evidence"]["test_log_sha256"],
            {"record_archive_proof": expected_log_sha256},
        )
        self.assertIsNotNone(summary["lineage_proof_evidence"]["max_generated_at_utc"])
        self.assertEqual(
            summary["android_device_lab"]["root"],
            readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
        )
        self.assertEqual(summary["android_device_lab"]["missing_device_families"], [])
        self.assertEqual(
            summary["android_device_lab"]["min_signed_at_utc"],
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        )
        self.assertIsNotNone(summary["android_device_lab"]["max_signed_at_utc"])
        self.assertEqual(
            summary["android_device_lab"]["signed_evidence"],
            expected_android_signed_evidence,
        )

    def test_missing_android_root_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            missing_root = Path(temp) / "missing-slots"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(missing_root),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 1)
        self.assertNotIn(str(missing_root), json.dumps(summary))
        self.assertEqual(
            summary["android_device_lab"]["root"],
            readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
        )
        self.assertFalse(summary["ready"])
        self.assertIn(
            "android_device_lab_root_missing",
            {item["code"] for item in summary["blockers"]},
        )
        self.assertEqual(
            summary["android_device_lab"]["min_signed_at_utc"],
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        )
        self.assertIsNotNone(summary["android_device_lab"]["max_signed_at_utc"])

    def test_symlinked_android_root_blocks_rollup_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            summary_path = temp_path / "summary.json"
            signer = slot_helpers.create_test_signer(temp_path / "keys")
            real_root = temp_path / "real-slots"
            linked_root = temp_path / "linked-slots"
            create_complete_matrix(real_root, signer)
            slot_helpers.create_dir_symlink(self, linked_root, real_root)
            lineage_evidence = create_lineage_proof_evidence(temp_path / "lineage")

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(linked_root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 1)
        rendered = json.dumps(summary)
        self.assertNotIn(str(linked_root), rendered)
        self.assertNotIn(str(real_root), rendered)
        self.assertFalse(summary["ready"])
        self.assertEqual(
            summary["android_device_lab"]["root"],
            readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
        )
        self.assertIn(
            "android_device_lab_root_invalid",
            {item["code"] for item in summary["blockers"]},
        )
        self.assertIn(
            "device-lab root must not be a symlink",
            {item["message"] for item in summary["blockers"]},
        )

    def test_symlinked_android_root_ancestor_blocks_rollup_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            summary_path = temp_path / "summary.json"
            signer = slot_helpers.create_test_signer(temp_path / "keys")
            external_parent = temp_path / "external-parent"
            real_root = external_parent / "device_lab"
            linked_parent = temp_path / "linked-parent"
            create_complete_matrix(real_root, signer)
            slot_helpers.create_dir_symlink(self, linked_parent, external_parent)
            linked_root = linked_parent / "device_lab"
            lineage_evidence = create_lineage_proof_evidence(temp_path / "lineage")

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(linked_root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 1)
        rendered = json.dumps(summary)
        self.assertNotIn(str(linked_root), rendered)
        self.assertNotIn(str(real_root), rendered)
        self.assertNotIn(str(linked_parent), rendered)
        self.assertFalse(summary["ready"])
        self.assertEqual(
            summary["android_device_lab"]["root"],
            readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
        )
        self.assertIn(
            "android_device_lab_root_invalid",
            {item["code"] for item in summary["blockers"]},
        )
        self.assertIn(
            "device-lab root ancestor directory must not be a symlink",
            {item["message"] for item in summary["blockers"]},
        )

    def test_missing_standard_family_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            slot_helpers.create_slot(
                root,
                "pixel8",
                slot_helpers.device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )
            trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                [signer["public_key"]]
            )
            self.assertEqual(errors, [])
            summary = readiness.build_summary(
                repo_root=REPO_ROOT,
                device_lab_root=root.resolve(),
                lineage_proof_evidence_path=lineage_evidence.resolve(),
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(status, 1)
        self.assertIn(
            "android_device_lab_standard_matrix_missing",
            {item["code"] for item in summary["blockers"]},
        )
        self.assertGreater(len(summary["android_device_lab"]["missing_device_families"]), 0)

    def test_duplicate_device_fingerprint_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            create_complete_matrix(root, signer)
            copy_slot_binding(
                source=root / "slot-0",
                target=root / "slot-1",
                signer=signer,
                key="device_fingerprint",
            )
            trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                [signer["public_key"]]
            )
            self.assertEqual(errors, [])

            summary = readiness.build_summary(
                repo_root=REPO_ROOT,
                device_lab_root=root.resolve(),
                lineage_proof_evidence_path=lineage_evidence.resolve(),
                trusted_signer_public_keys=trusted,
            )

        self.assertFalse(summary["ready"])
        blockers = [
            item
            for item in summary["blockers"]
            if item["code"] == "android_device_lab_duplicate_device_fingerprint"
        ]
        self.assertEqual(len(blockers), 1)
        self.assertEqual(blockers[0]["slots"], ["slot-0", "slot-1"])
        self.assertNotIn("slot-0/fingerprint", json.dumps(summary))

    def test_duplicate_attestation_challenge_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            create_complete_matrix(root, signer)
            copy_slot_binding(
                source=root / "slot-0",
                target=root / "slot-1",
                signer=signer,
                key="attestation_challenge_sha256",
            )
            trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                [signer["public_key"]]
            )
            self.assertEqual(errors, [])

            summary = readiness.build_summary(
                repo_root=REPO_ROOT,
                device_lab_root=root.resolve(),
                lineage_proof_evidence_path=lineage_evidence.resolve(),
                trusted_signer_public_keys=trusted,
            )

        self.assertFalse(summary["ready"])
        blockers = [
            item
            for item in summary["blockers"]
            if item["code"] == "android_device_lab_duplicate_attestation_challenge"
        ]
        self.assertEqual(len(blockers), 1)
        self.assertEqual(blockers[0]["slots"], ["slot-0", "slot-1"])

    def test_stale_signed_evidence_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            create_complete_matrix(root, signer)
            resign_slot_evidence_with_timestamp(
                root / "slot-0",
                signer,
                "2026-06-05T23:59:59Z",
            )

            trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                [signer["public_key"]]
            )
            self.assertEqual(errors, [])
            summary = readiness.build_summary(
                repo_root=REPO_ROOT,
                device_lab_root=root.resolve(),
                lineage_proof_evidence_path=lineage_evidence.resolve(),
                trusted_signer_public_keys=trusted,
                min_signed_at=readiness.parse_utc_timestamp(
                    readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                    "test cutoff",
                )[0],
            )

        self.assertFalse(summary["ready"])
        self.assertIn(
            "android_signed_evidence_stale",
            {item["code"] for item in summary["blockers"]},
        )

    def test_future_signed_evidence_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            create_complete_matrix(root, signer)
            resign_slot_evidence_with_timestamp(
                root / "slot-0",
                signer,
                "2026-06-06T00:05:01Z",
            )

            trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                [signer["public_key"]]
            )
            self.assertEqual(errors, [])
            summary = readiness.build_summary(
                repo_root=REPO_ROOT,
                device_lab_root=root.resolve(),
                lineage_proof_evidence_path=lineage_evidence.resolve(),
                trusted_signer_public_keys=trusted,
                min_signed_at=readiness.parse_utc_timestamp(
                    readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                    "test cutoff",
                )[0],
                max_signed_at=readiness.parse_utc_timestamp(
                    "2026-06-06T00:05:00Z",
                    "test max timestamp",
                )[0],
            )

        self.assertFalse(summary["ready"])
        self.assertIn(
            "android_signed_evidence_future_dated",
            {item["code"] for item in summary["blockers"]},
        )

    def test_signed_evidence_freshness_uses_validated_report_timestamp(self) -> None:
        min_signed_at = readiness.parse_utc_timestamp(
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            "test cutoff",
        )[0]
        blockers = readiness._check_android_signed_evidence_freshness(
            [
                {
                    "status": "ok",
                    "slot": "slot-0",
                    "kagemusha": {"signed_at_utc": "2026-06-05T23:59:59Z"},
                }
            ],
            min_signed_at,
            None,
        )

        self.assertIn(
            "android_signed_evidence_stale",
            {item["code"] for item in blockers},
        )

    def test_signed_evidence_freshness_requires_report_timestamp(self) -> None:
        blockers = readiness._check_android_signed_evidence_freshness(
            [{"status": "ok", "slot": "slot-0", "kagemusha": {}}],
            None,
            readiness.parse_utc_timestamp(
                "2026-06-06T00:05:00Z",
                "test max timestamp",
            )[0],
        )

        self.assertIn(
            "android_signed_evidence_timestamp_missing",
            {item["code"] for item in blockers},
        )

    def test_duplicate_signed_evidence_json_key_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            create_complete_matrix(root, signer)
            slot = root / "slot-0"
            evidence_path = slot / "evidence" / "signed-evidence.json"
            schema_line = f'"schema": "{slot_helpers.device_lab.SIGNED_EVIDENCE_SCHEMA}"'
            evidence_path.write_text(
                evidence_path.read_text(encoding="utf-8").replace(
                    schema_line,
                    f'"schema": "shadow",\n  {schema_line}',
                    1,
                ),
                encoding="utf-8",
            )
            slot_helpers.refresh_signed_evidence_hash(slot)

            trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                [signer["public_key"]]
            )
            self.assertEqual(errors, [])
            summary = readiness.build_summary(
                repo_root=REPO_ROOT,
                device_lab_root=root.resolve(),
                lineage_proof_evidence_path=lineage_evidence.resolve(),
                trusted_signer_public_keys=trusted,
                min_signed_at=readiness.parse_utc_timestamp(
                    readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                    "test cutoff",
                )[0],
            )

        self.assertFalse(summary["ready"])
        blockers = [
            item
            for item in summary["blockers"]
            if item["code"] == "android_device_lab_slot_invalid"
            and item.get("slot") == "slot-0"
        ]
        self.assertEqual(len(blockers), 1)
        self.assertIn(
            "signed evidence artifact contains duplicate JSON object key schema",
            blockers[0]["errors"],
        )

    def test_explicit_missing_slot_blocks_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir(parents=True)
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--slot",
                        "missing-slot",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("android_device_lab_slot_invalid", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())

    def test_unsafe_slot_id_blocks_rollup_without_path_escape(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir(parents=True)
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--slot",
                        "../outside",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("android_device_lab_slot_id_invalid", stderr.getvalue())
        self.assertIn(
            "slot id '../outside' must be a single safe directory name",
            stderr.getvalue(),
        )
        self.assertNotIn("Traceback", stderr.getvalue())

    def test_android_report_secret_material_is_redacted_before_summary(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir(parents=True)
            original_slot_reports = readiness._slot_reports

            def fake_slot_reports(*_args: object, **_kwargs: object) -> list[dict[str, object]]:
                return [
                    {
                        "slot": "token=supersecret-slot",
                        "status": "error",
                        "errors": ["artifact path token=supersecret-artifact"],
                        "present": {},
                        "file_counts": {},
                        "kagemusha": {"required": True},
                    }
                ]

            readiness._slot_reports = fake_slot_reports
            try:
                summary = readiness.check_android_device_lab(
                    root,
                    {"1" * 64: Path(temp) / "trusted-public.pem"},
                )
            finally:
                readiness._slot_reports = original_slot_reports

        rendered = json.dumps(summary, sort_keys=True)
        self.assertFalse(summary["ok"])
        self.assertIn("android_device_lab_report_secret_material", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("token=supersecret-slot", rendered)
        self.assertNotIn("token=supersecret-artifact", rendered)

    def test_untrusted_signed_evidence_blocks_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            other_signer = slot_helpers.create_test_signer(Path(temp) / "other-keys")
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            slot_helpers.create_slot(
                root,
                "pixel8",
                slot_helpers.device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--trusted-signer-public-key",
                        str(other_signer["public_key"]),
                    ]
                )
            trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                [other_signer["public_key"]]
            )
            self.assertEqual(errors, [])
            summary = readiness.build_summary(
                repo_root=REPO_ROOT,
                device_lab_root=root.resolve(),
                lineage_proof_evidence_path=lineage_evidence.resolve(),
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(status, 1)
        slot_errors = "\n".join(
            "\n".join(item.get("errors", []))
            for item in summary["blockers"]
            if item["code"] == "android_device_lab_slot_invalid"
        )
        self.assertIn(
            "signer_public_key_sha256 must match a trusted signer public key",
            slot_errors,
        )
        self.assertEqual(summary["android_device_lab"]["signed_evidence"], {})

    def test_abi6_manifest_drift_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            manifest = {
                "schema": "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
                "bridge_abi_version": 6,
                "operation_count": 8,
                "operations": [{"symbol": symbol} for symbol in readiness.ABI6_OPERATION_SYMBOLS],
                "limits": readiness.EXPECTED_ABI6_LIMITS,
                "modes": {
                    "preferred_when_recursive_available": "recursive_spend_v1",
                    "fallback_when_recursive_unavailable": "checked_prefold_v1",
                },
            }
            write_json(repo / readiness.ABI6_MANIFEST_PATH, manifest)

            result = readiness.check_abi6_reserved_lineage(repo)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi6_manifest_operation_count",
            {item["code"] for item in result["blockers"]},
        )

    def test_abi6_manifest_rejects_symlinked_manifest_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            repo = root / "repo"
            manifest_path = repo / readiness.ABI6_MANIFEST_PATH
            external_manifest = root / "external-manifest.json"
            manifest = {
                "schema": "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
                "bridge_abi_version": 6,
                "operation_count": len(readiness.ABI6_OPERATION_SYMBOLS),
                "operations": [
                    {"symbol": symbol} for symbol in readiness.ABI6_OPERATION_SYMBOLS
                ],
                "limits": readiness.EXPECTED_ABI6_LIMITS,
                "modes": {
                    "preferred_when_recursive_available": "recursive_spend_v1",
                    "fallback_when_recursive_unavailable": "checked_prefold_v1",
                },
            }
            write_json(external_manifest, manifest)
            manifest_path.parent.mkdir(parents=True, exist_ok=True)
            try:
                manifest_path.symlink_to(external_manifest)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            result = readiness.check_abi6_reserved_lineage(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi6_manifest_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "ABI-6 manifest must not be a symlink",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(external_manifest), rendered)

    def test_abi6_manifest_rejects_symlinked_manifest_ancestor(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            repo = real_parent / "repo"
            manifest = {
                "schema": "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
                "bridge_abi_version": 6,
                "operation_count": len(readiness.ABI6_OPERATION_SYMBOLS),
                "operations": [
                    {"symbol": symbol} for symbol in readiness.ABI6_OPERATION_SYMBOLS
                ],
                "limits": readiness.EXPECTED_ABI6_LIMITS,
                "modes": {
                    "preferred_when_recursive_available": "recursive_spend_v1",
                    "fallback_when_recursive_unavailable": "checked_prefold_v1",
                },
            }
            write_json(repo / readiness.ABI6_MANIFEST_PATH, manifest)
            linked_parent = root / "linked-parent"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)

            result = readiness.check_abi6_reserved_lineage(linked_parent / "repo")
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "kagemusha_repo_root_path_invalid",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "--repo-root ancestor directory must not be a symlink",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(linked_parent), rendered)

    def test_abi6_manifest_rejects_hardlinked_manifest_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            repo = root / "repo"
            manifest_path = repo / readiness.ABI6_MANIFEST_PATH
            external_manifest = root / "external-manifest.json"
            manifest = {
                "schema": "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
                "bridge_abi_version": 6,
                "operation_count": len(readiness.ABI6_OPERATION_SYMBOLS),
                "operations": [
                    {"symbol": symbol} for symbol in readiness.ABI6_OPERATION_SYMBOLS
                ],
                "limits": readiness.EXPECTED_ABI6_LIMITS,
                "modes": {
                    "preferred_when_recursive_available": "recursive_spend_v1",
                    "fallback_when_recursive_unavailable": "checked_prefold_v1",
                },
            }
            write_json(external_manifest, manifest)
            write_json(manifest_path, manifest)
            slot_helpers.replace_with_hardlink(self, manifest_path, external_manifest)

            result = readiness.check_abi6_reserved_lineage(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi6_manifest_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "ABI-6 manifest must not be hardlinked",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(external_manifest), rendered)

    def test_release_local_json_validator_rejects_secret_path_directly_without_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            manifest_path = (
                Path(temp)
                / "token=supersecret-release-json"
                / "manifest.json"
            )
            manifest_path.parent.mkdir(parents=True, exist_ok=True)
            manifest_path.write_text("{not-json", encoding="utf-8")

            errors = readiness.validate_release_local_json_file(
                manifest_path,
                "ABI-6 manifest",
            )
            rendered = json.dumps(errors)

        self.assertEqual(
            errors,
            ["ABI-6 manifest path must not contain secret-looking material"],
        )
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn(str(manifest_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_repo_source_marker_validator_rejects_secret_path_directly_without_metadata(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            marker_path = (
                Path(temp)
                / "token=supersecret-source-marker"
                / "zk.rs"
            )
            errors = readiness.validate_repo_source_marker_file(
                marker_path,
                "ABI-7 core marker file",
            )
            rendered = json.dumps(errors)

        self.assertEqual(
            errors,
            ["ABI-7 core marker file path must not contain secret-looking material"],
        )
        self.assertNotIn("is missing", rendered)
        self.assertNotIn(str(marker_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_abi7_fail_closed_rejects_symlinked_source_marker_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            repo = root / "repo"
            write_abi7_fail_closed_marker_files(repo)
            marker_path = repo / "crates/iroha_core/src/zk.rs"
            external_marker = root / "external-core-marker.rs"
            external_marker.write_text(marker_path.read_text(encoding="utf-8"), encoding="utf-8")
            slot_helpers.replace_with_symlink(self, marker_path, external_marker)

            result = readiness.check_abi7_fail_closed(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi7_source_marker_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "ABI-7 core marker file must not be a symlink",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(external_marker), rendered)

    def test_abi7_fail_closed_rejects_hardlinked_source_marker_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            repo = root / "repo"
            write_abi7_fail_closed_marker_files(repo)
            marker_path = repo / "crates/iroha_core/src/zk.rs"
            external_marker = root / "external-core-marker.rs"
            external_marker.write_text(marker_path.read_text(encoding="utf-8"), encoding="utf-8")
            slot_helpers.replace_with_hardlink(self, marker_path, external_marker)

            result = readiness.check_abi7_fail_closed(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi7_source_marker_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "ABI-7 core marker file must not be hardlinked",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(external_marker), rendered)

    def test_lineage_key_release_tooling_drift_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            for relative, snippets in readiness.LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS.items():
                path = repo / relative
                payload = list(snippets)
                if relative == "crates/iroha_cli/src/zk.rs":
                    payload.remove("record_out: Option<std::path::PathBuf>")
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("\n".join(payload) + "\n", encoding="utf-8")

            result = readiness.check_lineage_key_release_tooling(repo)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "lineage_key_release_marker_missing",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "record_out: Option<std::path::PathBuf>",
            {item.get("marker") for item in blockers},
        )

    def test_lineage_key_release_tooling_rejects_symlinked_marker_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            repo = root / "repo"
            write_lineage_key_release_tooling_marker_files(repo)
            marker_path = repo / "crates/iroha_cli/src/zk.rs"
            external_marker = root / "external-lineage-cli.rs"
            external_marker.write_text(marker_path.read_text(encoding="utf-8"), encoding="utf-8")
            slot_helpers.replace_with_symlink(self, marker_path, external_marker)

            result = readiness.check_lineage_key_release_tooling(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_key_release_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "Reserved-lineage release-tooling marker file must not be a symlink",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(external_marker), rendered)

    def test_lineage_key_release_tooling_rejects_hardlinked_marker_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            repo = root / "repo"
            write_lineage_key_release_tooling_marker_files(repo)
            marker_path = repo / "crates/iroha_cli/src/zk.rs"
            external_marker = root / "external-lineage-cli.rs"
            external_marker.write_text(marker_path.read_text(encoding="utf-8"), encoding="utf-8")
            slot_helpers.replace_with_hardlink(self, marker_path, external_marker)

            result = readiness.check_lineage_key_release_tooling(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_key_release_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "Reserved-lineage release-tooling marker file must not be hardlinked",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(external_marker), rendered)

    def test_missing_lineage_proof_evidence_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            result = readiness.check_lineage_proof_evidence(
                Path(temp) / "missing-lineage-proof-evidence.json"
            )

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_missing",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_noncanonical_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            noncanonical_path = evidence_path.with_name("lineage-proof-copy.json")
            evidence_path.rename(noncanonical_path)

            result = readiness.check_lineage_proof_evidence(noncanonical_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_filename",
            {item["code"] for item in result["blockers"]},
        )
        self.assertNotIn(str(noncanonical_path.parent), json.dumps(result))

    def test_lineage_proof_evidence_rejects_symlinked_evidence_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_lineage_proof_evidence(root / "real-lineage")
            evidence_link = root / "lineage-proof-evidence.json"
            try:
                evidence_link.symlink_to(evidence_path)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            result = readiness.check_lineage_proof_evidence(evidence_link)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "Reserved-lineage proof evidence file must not be a symlink",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(evidence_path), rendered)

    def test_lineage_proof_evidence_rejects_symlinked_evidence_ancestor(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            evidence_path = create_lineage_proof_evidence(real_parent / "lineage")
            linked_parent = root / "linked-parent"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)
            linked_evidence = linked_parent / "lineage" / evidence_path.name

            result = readiness.check_lineage_proof_evidence(linked_evidence)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "Reserved-lineage proof evidence ancestor directory must not be a symlink",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(linked_parent), rendered)

    def test_lineage_proof_evidence_rejects_secret_path_before_json_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = (
                Path(temp)
                / "token=supersecret-lineage"
                / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
            )
            evidence_path.parent.mkdir(parents=True, exist_ok=True)
            evidence_path.write_text("{not-json", encoding="utf-8")

            result = readiness.check_lineage_proof_evidence(evidence_path)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "Reserved-lineage proof evidence file path must not contain secret-looking material",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn("lineage_proof_evidence_invalid_json", rendered)
        self.assertNotIn(str(evidence_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_evidence_rejects_duplicate_json_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            schema_line = f'"schema": "{readiness.LINEAGE_PROOF_EVIDENCE_SCHEMA}"'
            evidence_path.write_text(
                evidence_path.read_text(encoding="utf-8").replace(
                    schema_line,
                    f'"schema": "shadow",\n  {schema_line}',
                    1,
                ),
                encoding="utf-8",
            )

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_invalid_json",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn("duplicate JSON object key schema", json.dumps(result["blockers"]))

    def test_lineage_proof_evidence_redacts_secret_duplicate_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = Path(temp) / "lineage" / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
            evidence_path.parent.mkdir(parents=True, exist_ok=True)
            evidence_path.write_text(
                '{"token=supersecret": 1, "token=supersecret": 2}\n',
                encoding="utf-8",
            )

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        rendered = json.dumps(result["blockers"])
        self.assertIn("lineage_proof_evidence_invalid_json", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_stale_lineage_proof_evidence_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["generated_at_utc"] = "2026-06-05T23:59:59Z"
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(
                evidence_path,
                min_generated_at=readiness.parse_utc_timestamp(
                    readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                    "test cutoff",
                )[0],
            )

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_stale",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_noncanonical_timestamp(self) -> None:
        for timestamp in (
            "2026-06-06T00:00:00+00:00",
            " 2026-06-06T00:00:00Z ",
        ):
            with tempfile.TemporaryDirectory() as temp:
                evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
                evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
                evidence["generated_at_utc"] = timestamp
                write_json(evidence_path, evidence)

                result = readiness.check_lineage_proof_evidence(evidence_path)

            self.assertFalse(result["ok"])
            self.assertIn(
                "lineage_proof_evidence_timestamp_noncanonical",
                {item["code"] for item in result["blockers"]},
            )

    def test_future_lineage_proof_evidence_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["generated_at_utc"] = "2026-06-06T00:05:01Z"
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(
                evidence_path,
                max_generated_at=readiness.parse_utc_timestamp(
                    "2026-06-06T00:05:00Z",
                    "test max timestamp",
                )[0],
            )

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_future_dated",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_drift_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["record_archive_proof_runtime_keygen_env"] = "set"
            evidence["circuit_ids"]["append"] = "kagemusha-recursive-spend-lineage-v1"
            evidence["artifacts"]["lineage-append-len128.pk"] = "0" * 64
            evidence["tests"]["record_archive_proof"]["status"] = "failed"
            evidence["tests"]["record_archive_proof"]["command"] = "cargo test quick_smoke"
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn(
            "lineage_proof_evidence_record_archive_proof_runtime_keygen_env",
            codes,
        )
        self.assertIn("lineage_proof_evidence_circuit_id", codes)
        self.assertIn("lineage_proof_evidence_artifact_digest", codes)
        self.assertIn("lineage_proof_evidence_test_status", codes)
        self.assertIn("lineage_proof_evidence_test_command", codes)

    def test_lineage_proof_evidence_rejects_float_scalar_claims(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["opening_len"] = 128.0
            evidence["ipa_k"] = 8.0
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn("lineage_proof_evidence_opening_len", codes)
        self.assertIn("lineage_proof_evidence_ipa_k", codes)

    def test_lineage_proof_evidence_rejects_runtime_keygen_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["tests"]["record_archive_proof"]["command"] = (
                f"{readiness.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_RUNTIME_KEYGEN_ENV}=1 "
                f"{evidence['tests']['record_archive_proof']['command']}"
            )
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_command",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "--command must not set runtime lineage keygen for the production proof run",
            {item.get("issue") for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_fake_runner_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["tests"]["record_archive_proof"]["command"] = (
                "python3 fake_runner.py "
                f"{readiness.LINEAGE_PROOF_REQUIRED_TESTS['record_archive_proof']} "
                "--lib -- --ignored --test-threads=1 --nocapture"
            )
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "--command must exactly match the production Reserved-lineage proof command",
            {item.get("issue") for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_appended_shell_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["tests"]["record_archive_proof"]["command"] += " ; echo ok"
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "--command must exactly match the production Reserved-lineage proof command",
            {item.get("issue") for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_shell_equivalent_noncanonical_command(self) -> None:
        canonical = readiness.expected_lineage_proof_command(
            readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
        )
        for command in (
            canonical.replace("cargo test", "'cargo' test", 1),
            canonical.replace(" -p ", "\n-p ", 1),
            f" {canonical} ",
        ):
            with tempfile.TemporaryDirectory() as temp:
                evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
                evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
                evidence["tests"]["record_archive_proof"]["command"] = command
                write_json(evidence_path, evidence)

                result = readiness.check_lineage_proof_evidence(evidence_path)

            self.assertFalse(result["ok"])
            self.assertIn(
                "--command must exactly match the canonical production Reserved-lineage proof command string",
                {item.get("issue") for item in result["blockers"]},
            )

    def test_lineage_proof_evidence_rejects_secret_looking_command_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["tests"]["record_archive_proof"]["command"] += " token=supersecret"
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)
            rendered = json.dumps(result["blockers"])

        self.assertFalse(result["ok"])
        self.assertIn(
            "--command must not contain secret-looking material",
            {item.get("issue") for item in result["blockers"]},
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_evidence_rejects_missing_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            (evidence_path.parent / "lineage-append-len128.vk").unlink()

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "lineage_proof_evidence_artifact_missing",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "lineage-append-len128.vk",
            {item.get("artifact") for item in blockers},
        )

    def test_lineage_proof_evidence_rejects_symlinked_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_lineage_proof_evidence(root / "lineage")
            artifact = evidence_path.parent / "lineage-append-len128.vk"
            external = root / "external-lineage-append-len128.vk"
            external.write_bytes(artifact.read_bytes())
            slot_helpers.replace_with_symlink(self, artifact, external)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "lineage_proof_evidence_artifact_file_shape",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "Reserved-lineage proof evidence artifact file must not be a symlink",
            {item["message"] for item in blockers},
        )
        self.assertNotIn(str(evidence_path.parent), json.dumps(blockers))

    def test_lineage_proof_evidence_rejects_hardlinked_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_lineage_proof_evidence(root / "lineage")
            artifact = evidence_path.parent / "lineage-append-len128.vk"
            external = root / "external-lineage-append-len128.vk"
            external.write_bytes(artifact.read_bytes())
            slot_helpers.replace_with_hardlink(self, artifact, external)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "lineage_proof_evidence_artifact_file_shape",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "Reserved-lineage proof evidence artifact file must not be hardlinked",
            {item["message"] for item in blockers},
        )
        self.assertNotIn(str(evidence_path.parent), json.dumps(blockers))

    def test_lineage_proof_evidence_rejects_local_artifact_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            (evidence_path.parent / "lineage-init-len128.pk").write_bytes(
                b"tampered lineage proving key\n"
            )

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "lineage_proof_evidence_artifact_file_digest",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "lineage-init-len128.pk",
            {item.get("artifact") for item in blockers},
        )
        self.assertNotIn(
            "lineage-init-len128.pk",
            result["artifact_sha256"],
        )
        self.assertNotIn(str(evidence_path.parent), json.dumps(result))

    def test_lineage_proof_evidence_rejects_missing_local_proof_log_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            (
                evidence_path.parent
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            ).unlink()

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_log_missing",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_symlinked_local_proof_log_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_lineage_proof_evidence(root / "lineage")
            log_path = (
                evidence_path.parent
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            )
            external = root / "external-record-archive-proof.log"
            external.write_bytes(log_path.read_bytes())
            slot_helpers.replace_with_symlink(self, log_path, external)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_log_unreadable",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "production proof log must not be a symlink",
            {item.get("issue") for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_hardlinked_local_proof_log_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_lineage_proof_evidence(root / "lineage")
            log_path = (
                evidence_path.parent
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            )
            external = root / "external-record-archive-proof.log"
            external.write_bytes(log_path.read_bytes())
            slot_helpers.replace_with_hardlink(self, log_path, external)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_log_unreadable",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "production proof log must not be hardlinked",
            {item.get("issue") for item in result["blockers"]},
        )

    def test_lineage_proof_log_rejects_secret_path_before_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            log_path = Path(temp) / "token=supersecret-record-archive-proof.log"
            log_path.write_text(
                "test result: FAILED\npanicked at token=supersecret\n",
                encoding="utf-8",
            )

            digest, errors = readiness.validate_lineage_proof_log(
                log_path,
                readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
            )
            rendered = json.dumps(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["production proof log path must not contain secret-looking material"],
        )
        self.assertNotIn("FAILED", rendered)
        self.assertNotIn(str(log_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_log_rejects_symlinked_ancestor_before_digest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-lineage"
            real_parent.mkdir()
            log_path = real_parent / "record-archive-proof.log"
            write_passing_lineage_proof_log(log_path)
            linked_parent = root / "linked-lineage"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)

            digest, errors = readiness.validate_lineage_proof_log(
                linked_parent / "record-archive-proof.log",
                readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
            )
            rendered = json.dumps(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["production proof log ancestor directory must not be a symlink"],
        )
        self.assertNotIn(str(linked_parent), rendered)
        self.assertNotIn(str(real_parent), rendered)

    def test_lineage_proof_evidence_rejects_oversized_local_proof_log(self) -> None:
        old_limit = readiness.MAX_LINEAGE_PROOF_LOG_BYTES
        try:
            readiness.MAX_LINEAGE_PROOF_LOG_BYTES = 8
            with tempfile.TemporaryDirectory() as temp:
                evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")

                result = readiness.check_lineage_proof_evidence(evidence_path)
        finally:
            readiness.MAX_LINEAGE_PROOF_LOG_BYTES = old_limit

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_log_unreadable",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "lineage_proof_evidence_test_log_content",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_local_proof_log_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            (
                evidence_path.parent
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            ).write_text(
                "\n".join(
                    (
                        "running 1 test",
                        (
                            "test "
                            f"{readiness.LINEAGE_PROOF_REQUIRED_TESTS['record_archive_proof']} "
                            "... ok"
                        ),
                        (
                            "test result: ok. 1 passed; 0 failed; 0 ignored; "
                            "0 measured; 0 filtered out"
                        ),
                        "tampered trailer",
                        "",
                    )
                ),
                encoding="utf-8",
            )

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_log_file_digest",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_bad_local_proof_log_content(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            log_path = (
                evidence_path.parent
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            )
            log_path.write_text(
                "running 1 test\n"
                "test unrelated_smoke_test ... ok\n"
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
                encoding="utf-8",
            )
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["tests"]["record_archive_proof"]["log_sha256"] = hashlib.sha256(
                log_path.read_bytes()
            ).hexdigest()
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_log_content",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_marker_stuffed_local_proof_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            log_path = (
                evidence_path.parent
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            )
            test_name = readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
            log_path.write_text(
                "\n".join(
                    (
                        "running 2 tests",
                        f"test {test_name} ... ok",
                        "test unrelated_smoke_test ... ok",
                        (
                            "test result: ok. 2 passed; 0 failed; 0 ignored; "
                            "0 measured; 0 filtered out; finished in 14400.00s"
                        ),
                        "",
                    )
                ),
                encoding="utf-8",
            )
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["tests"]["record_archive_proof"]["log_sha256"] = hashlib.sha256(
                log_path.read_bytes()
            ).hexdigest()
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        issues = {
            item.get("issue")
            for item in result["blockers"]
            if item["code"] == "lineage_proof_evidence_test_log_content"
        }
        self.assertIn(
            "--proof-log must contain only the single production proof test line",
            issues,
        )
        self.assertIn(
            "--proof-log must contain exactly one cargo test result for one passed production test",
            issues,
        )
        self.assertNotIn("record_archive_proof", result["test_log_sha256"])

    def test_lineage_proof_evidence_rejects_boolean_or_nonfinite_elapsed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["tests"]["record_archive_proof"]["elapsed_seconds"] = True
            write_json(evidence_path, evidence)

            boolean_result = readiness.check_lineage_proof_evidence(evidence_path)

            evidence["tests"]["record_archive_proof"]["elapsed_seconds"] = float("nan")
            write_json(evidence_path, evidence)
            nonfinite_result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(boolean_result["ok"])
        self.assertFalse(nonfinite_result["ok"])
        self.assertIn(
            "lineage_proof_evidence_test_elapsed",
            {item["code"] for item in boolean_result["blockers"]},
        )
        self.assertIn(
            "lineage_proof_evidence_test_elapsed",
            {item["code"] for item in nonfinite_result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_unexpected_top_level_field_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["token=supersecret"] = "must not ship"
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        rendered = json.dumps(result["blockers"])
        self.assertIn("lineage_proof_evidence_unexpected_field", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_evidence_rejects_unexpected_nested_fields_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["circuit_ids"]["token=secret-circuit"] = "unexpected"
            evidence["artifacts"]["token=secret-artifact"] = "1" * 64
            evidence["tests"]["token=secret-test"] = {}
            evidence["tests"]["record_archive_proof"]["token=secret-test-field"] = True
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn("lineage_proof_evidence_circuit_ids_unexpected_field", codes)
        self.assertIn("lineage_proof_evidence_artifacts_unexpected_field", codes)
        self.assertIn("lineage_proof_evidence_tests_unexpected_field", codes)
        self.assertIn("lineage_proof_evidence_test_unexpected_field", codes)
        rendered = json.dumps(result["blockers"])
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=secret", rendered)

    def test_lineage_proof_evidence_helper_generates_validator_accepted_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )
            result = readiness.check_lineage_proof_evidence(out)
            evidence = json.loads(out.read_text(encoding="utf-8"))
            proof_log_hash = hashlib.sha256(proof_log.read_bytes()).hexdigest()

        self.assertEqual(status, 0)
        self.assertTrue(result["ok"])
        self.assertEqual(
            evidence["artifacts"]["lineage-init-len128.vk"],
            hashlib.sha256(b"lineage artifact lineage-init-len128.vk\n").hexdigest(),
        )
        self.assertEqual(
            evidence["tests"]["record_archive_proof"]["log_sha256"],
            proof_log_hash,
        )

    def test_lineage_proof_evidence_document_validator_rejects_symlinked_artifact_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_dir = root / "real-artifacts"
            real_dir.mkdir()
            linked_dir = root / "linked-artifacts"
            slot_helpers.create_dir_symlink(self, linked_dir, real_dir)

            errors = evidence_helper.validate_evidence_document({}, linked_dir)

            self.assertIn("--artifact-dir must not be a symlink", errors)
            self.assertFalse(list(real_dir.glob(".lineage-proof-evidence-*.json")))

    def test_lineage_proof_evidence_document_validator_rejects_secret_artifact_dir(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_dir = Path(temp) / "token=supersecret-artifacts"

            errors = evidence_helper.validate_evidence_document({}, secret_dir)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["--artifact-dir must not contain secret-looking material"])
        self.assertFalse(secret_dir.exists())
        self.assertNotIn(str(secret_dir), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_artifact_dir_validator_rejects_secret_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_dir = Path(temp) / "token=supersecret-artifacts"

            errors = evidence_helper.validate_artifact_dir_path(secret_dir)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["--artifact-dir must not contain secret-looking material"])
        self.assertFalse(secret_dir.exists())
        self.assertNotIn(str(secret_dir), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_evidence_helper_rejects_missing_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            (artifact_dir / "lineage-append-len128.pk").unlink()
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("missing lineage artifact lineage-append-len128.pk", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_symlinked_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            artifact_dir = root / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            artifact = artifact_dir / "lineage-append-len128.pk"
            external = root / "external-lineage-append-len128.pk"
            external.write_bytes(artifact.read_bytes())
            slot_helpers.replace_with_symlink(self, artifact, external)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn(
            "lineage artifact lineage-append-len128.pk must not be a symlink",
            stderr.getvalue(),
        )

    def test_lineage_proof_evidence_helper_rejects_hardlinked_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            artifact_dir = root / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            artifact = artifact_dir / "lineage-append-len128.pk"
            external = root / "external-lineage-append-len128.pk"
            external.write_bytes(artifact.read_bytes())
            slot_helpers.replace_with_hardlink(self, artifact, external)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn(
            "lineage artifact lineage-append-len128.pk must not be hardlinked",
            stderr.getvalue(),
        )

    def test_lineage_proof_evidence_helper_rejects_noncanonical_generated_at_utc(self) -> None:
        for generated_at_utc in (
            "2026-06-06T00:00:00+00:00",
            " 2026-06-06T00:00:00Z ",
        ):
            with self.subTest(generated_at_utc=generated_at_utc):
                with tempfile.TemporaryDirectory() as temp:
                    artifact_dir = Path(temp) / "artifacts"
                    create_lineage_artifact_files(artifact_dir)
                    proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                        "record_archive_proof"
                    ]
                    write_passing_lineage_proof_log(proof_log)
                    out = artifact_dir / "lineage-proof-evidence.json"
                    stderr = io.StringIO()

                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = evidence_helper.main(
                            [
                                "--artifact-dir",
                                str(artifact_dir),
                                "--proof-log",
                                str(proof_log),
                                "--elapsed-seconds",
                                "14400.5",
                                "--generated-at-utc",
                                generated_at_utc,
                                "--out",
                                str(out),
                            ]
                        )

                self.assertEqual(status, 1)
                self.assertFalse(out.exists())
                self.assertIn(
                    "--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    stderr.getvalue(),
                )

    def test_lineage_proof_evidence_helper_rejects_runtime_keygen_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()
            command = (
                f"{readiness.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_RUNTIME_KEYGEN_ENV}=1 "
                f"{evidence_helper.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND}"
            )

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--command",
                        command,
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("must not set runtime lineage keygen", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_fake_runner_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--command",
                        (
                            "python3 fake_runner.py "
                            f"{readiness.LINEAGE_PROOF_REQUIRED_TESTS['record_archive_proof']} "
                            "--lib -- --ignored --test-threads=1 --nocapture"
                        ),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("must exactly match the production", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_shell_equivalent_noncanonical_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--command",
                        evidence_helper.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND.replace(
                            "cargo test", "'cargo' test", 1
                        ),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("canonical production", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_appended_shell_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--command",
                        f"{evidence_helper.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND} ; echo ok",
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("must exactly match the production", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_secret_looking_command_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--command",
                        f"{evidence_helper.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND} token=supersecret",
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )
            stderr_text = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("must not contain secret-looking material", stderr_text)
        self.assertNotIn("token=supersecret", stderr_text)

    def test_lineage_proof_evidence_helper_rejects_nonfinite_elapsed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "nan",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("--elapsed-seconds must be a positive finite number", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_outside_artifact_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = Path(temp) / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn(
            "--out must be written directly under --artifact-dir",
            stderr.getvalue(),
        )

    def test_lineage_proof_evidence_helper_rejects_noncanonical_output_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-copy.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("--out must be named lineage-proof-evidence.json", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_symlinked_artifact_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_artifact_dir = root / "real-artifacts"
            create_lineage_artifact_files(real_artifact_dir)
            proof_log = real_artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            artifact_dir_link = root / "linked-artifacts"
            slot_helpers.create_dir_symlink(self, artifact_dir_link, real_artifact_dir)
            out = artifact_dir_link / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir_link),
                        "--proof-log",
                        str(artifact_dir_link / proof_log.name),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("--artifact-dir must not be a symlink", stderr.getvalue())
        self.assertFalse((real_artifact_dir / "lineage-proof-evidence.json").exists())

    def test_lineage_proof_evidence_helper_preflights_output_ancestor_before_artifact_reads(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            artifact_dir = real_parent / "artifacts"
            artifact_dir.mkdir(parents=True)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            linked_parent = root / "linked-parent"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)
            out = linked_parent / "artifacts" / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )
            stderr_text = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("--out ancestor directory must not be a symlink", stderr_text)
        self.assertNotIn("missing lineage artifact", stderr_text)
        self.assertFalse((artifact_dir / "lineage-proof-evidence.json").exists())

    def test_lineage_proof_evidence_helper_rejects_symlinked_output_ancestor(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            artifact_dir = real_parent / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            linked_parent = root / "linked-parent"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)
            out = linked_parent / "artifacts" / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("--out ancestor directory must not be a symlink", stderr.getvalue())
        self.assertFalse((artifact_dir / "lineage-proof-evidence.json").exists())

    def test_lineage_proof_output_validator_rejects_symlinked_ancestor_before_creating_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            real_parent.mkdir()
            linked_parent = root / "linked-parent"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)

            errors = evidence_helper.validate_output_path(
                linked_parent / "missing" / "lineage-proof-evidence.json",
                "--out",
            )

        self.assertEqual(errors, ["--out ancestor directory must not be a symlink"])
        self.assertFalse((real_parent / "missing").exists())

    def test_lineage_proof_output_preflight_rejects_secret_path_directly_before_creating_parent(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_out = Path(temp) / "token=supersecret" / "lineage-proof-evidence.json"

            errors = evidence_helper.preflight_output_path(secret_out, "--out")
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["--out must not contain secret-looking material"])
        self.assertFalse(secret_out.exists())
        self.assertFalse(secret_out.parent.exists())
        self.assertNotIn(str(secret_out), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_write_evidence_rejects_secret_output_path_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_out = Path(temp) / "token=supersecret-evidence.json"

            errors = evidence_helper.write_evidence(secret_out, {"schema": "test"})
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["--out must not contain secret-looking material"])
        self.assertFalse(secret_out.exists())
        self.assertNotIn(str(secret_out), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_evidence_helper_rejects_symlinked_output_leaf(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            target = artifact_dir / "aliased-lineage-proof-evidence.json"
            target.write_text("external\n", encoding="utf-8")
            try:
                out.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("--out must not be a symlink", stderr.getvalue())
        self.assertEqual(target_text, "external\n")

    def test_lineage_proof_evidence_helper_rejects_hardlinked_output_leaf(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            artifact_dir = root / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            target = root / "external-lineage-proof-evidence.json"
            target.write_text("external\n", encoding="utf-8")
            out.write_text("placeholder\n", encoding="utf-8")
            slot_helpers.replace_with_hardlink(self, out, target)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("--out must not be hardlinked", stderr.getvalue())
        self.assertEqual(target_text, "external\n")

    def test_lineage_proof_evidence_helper_rejects_detached_proof_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = Path(temp) / "record-archive-proof.log"
            write_passing_lineage_proof_log(proof_log)
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn(
            "--proof-log must be written directly under --artifact-dir",
            stderr.getvalue(),
        )

    def test_lineage_proof_build_evidence_rejects_detached_proof_log_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = Path(temp) / "record-archive-proof.log"
            write_passing_lineage_proof_log(proof_log)

            evidence, errors = evidence_helper.build_evidence(
                artifact_dir=artifact_dir,
                proof_log=proof_log,
                command=evidence_helper.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND,
                elapsed_seconds=14400.5,
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertEqual(
            errors,
            [
                "--proof-log must be written directly under --artifact-dir as "
                f"{readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS['record_archive_proof']}"
            ],
        )

    def test_lineage_proof_build_evidence_rejects_secret_looking_proof_log_before_reads(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            proof_log = artifact_dir / "token=supersecret.log"

            evidence, errors = evidence_helper.build_evidence(
                artifact_dir=artifact_dir,
                proof_log=proof_log,
                command=evidence_helper.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND,
                elapsed_seconds=14400.5,
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(evidence)
        self.assertIn("--proof-log must not contain secret-looking material", rendered)
        self.assertNotIn(str(proof_log), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("missing lineage artifact", rendered)
        self.assertNotIn("missing production proof log", rendered)

    def test_lineage_proof_input_validator_rejects_secret_proof_log_directly_before_resolve(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            proof_log = artifact_dir / "token=supersecret.log"

            errors = evidence_helper.validate_lineage_input_paths(artifact_dir, proof_log)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["--proof-log must not contain secret-looking material"])
        self.assertFalse(artifact_dir.exists())
        self.assertNotIn(
            "--proof-log must be written directly under --artifact-dir",
            rendered,
        )
        self.assertNotIn(str(proof_log), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_evidence_helper_rejects_log_without_test_name(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            proof_log.write_text(
                "running 1 test\n"
                "test unrelated_smoke_test ... ok\n"
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
                encoding="utf-8",
            )
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("must contain the passing production proof test line", stderr.getvalue())

    def test_lineage_proof_evidence_helper_rejects_marker_stuffed_proof_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            test_name = readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
            proof_log.write_text(
                "\n".join(
                    (
                        "running 2 tests",
                        f"test {test_name} ... ok",
                        "test unrelated_smoke_test ... ok",
                        (
                            "test result: ok. 2 passed; 0 failed; 0 ignored; "
                            "0 measured; 0 filtered out; finished in 14400.00s"
                        ),
                        "",
                    )
                ),
                encoding="utf-8",
            )
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn(
            "--proof-log must contain only the single production proof test line",
            stderr.getvalue(),
        )
        self.assertIn(
            "--proof-log must contain exactly one cargo test result for one passed production test",
            stderr.getvalue(),
        )

    def test_lineage_proof_evidence_helper_rejects_failed_proof_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                "record_archive_proof"
            ]
            proof_log.write_text(
                "\n".join(
                    (
                        "running 1 test",
                        (
                            "test "
                            f"{readiness.LINEAGE_PROOF_REQUIRED_TESTS['record_archive_proof']} "
                            "... FAILED"
                        ),
                        "failures:",
                        (
                            "test result: FAILED. 0 passed; 1 failed; 0 ignored; "
                            "0 measured; 0 filtered out"
                        ),
                        "",
                    )
                ),
                encoding="utf-8",
            )
            out = artifact_dir / "lineage-proof-evidence.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--proof-log",
                        str(proof_log),
                        "--elapsed-seconds",
                        "14400.5",
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertFalse(out.exists())
        self.assertIn("must contain a passing cargo test result", stderr.getvalue())
        self.assertIn("must not contain cargo failure markers", stderr.getvalue())

    def test_summary_does_not_leak_trusted_signer_key_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            create_complete_matrix(root, signer)

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary_text = summary_path.read_text(encoding="utf-8")

        self.assertEqual(status, 0)
        self.assertNotIn(str(signer["public_key"]), summary_text)
        self.assertNotIn(str(signer["private_key"]), summary_text)
        self.assertIn(str(signer["public_key_sha256"]), summary_text)

    def test_summary_does_not_leak_device_lab_root_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            create_complete_matrix(root, signer)
            stdout = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(io.StringIO()):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--lineage-proof-evidence",
                        str(lineage_evidence),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary_text = summary_path.read_text(encoding="utf-8")
            stdout_text = stdout.getvalue()

        self.assertEqual(status, 0)
        self.assertNotIn(str(root), summary_text)
        self.assertNotIn(str(summary_path), stdout_text)
        self.assertIn(readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL, summary_text)
        self.assertIn("[kagemusha-readiness] wrote summary", stdout_text)

    def test_secret_looking_device_lab_root_blocks_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            secret_root = Path(temp) / "token=supersecret-slots"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(secret_root),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary_text = summary_path.read_text(encoding="utf-8")
            rendered = stdout.getvalue() + stderr.getvalue() + summary_text

        self.assertEqual(status, 1)
        self.assertNotIn("token=supersecret", rendered)
        self.assertIn("android_device_lab_root_path_invalid", rendered)
        self.assertIn("--device-lab-root must not contain secret-looking material", rendered)

    def test_validate_repo_root_rejects_secret_path_directly_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_repo_root = Path(temp) / "token=supersecret-repo"

            errors = readiness.validate_repo_root_path(secret_repo_root)
            rendered = json.dumps(errors)

        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_repo_root_path_invalid",
                    "message": "--repo-root must not contain secret-looking material",
                }
            ],
        )
        self.assertNotIn(str(secret_repo_root), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_trust_root_sections_reject_secret_repo_root_before_reads(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_repo_root = Path(temp) / "token=supersecret-repo"
            write_json(secret_repo_root / readiness.ABI6_MANIFEST_PATH, {"schema": "bad"})
            write_abi7_fail_closed_marker_files(secret_repo_root)
            write_lineage_key_release_tooling_marker_files(secret_repo_root)

            results = [
                readiness.check_abi6_reserved_lineage(secret_repo_root),
                readiness.check_abi7_fail_closed(secret_repo_root),
                readiness.check_lineage_key_release_tooling(secret_repo_root),
            ]
            rendered = json.dumps(results)

        for result in results:
            self.assertFalse(result["ok"])
            self.assertEqual(
                result["blockers"],
                [
                    {
                        "code": "kagemusha_repo_root_path_invalid",
                        "message": "--repo-root must not contain secret-looking material",
                    }
                ],
            )
        self.assertNotIn("abi6_manifest_schema", rendered)
        self.assertNotIn("abi7_fail_closed_marker_missing", rendered)
        self.assertNotIn("lineage_key_release_marker_missing", rendered)
        self.assertNotIn(str(secret_repo_root), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_symlinked_repo_root_blocks_before_rollup_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_repo = root / "real-repo"
            real_repo.mkdir()
            linked_repo = root / "linked-repo"
            slot_helpers.create_dir_symlink(self, linked_repo, real_repo)
            summary_path = root / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(linked_repo),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary_text = summary_path.read_text(encoding="utf-8")
            rendered = stdout.getvalue() + stderr.getvalue() + summary_text

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_repo_root_path_invalid", rendered)
        self.assertIn("--repo-root must not be a symlink", rendered)
        self.assertNotIn(str(linked_repo), rendered)
        self.assertNotIn(str(real_repo), rendered)

    def test_symlinked_repo_root_ancestor_blocks_before_rollup_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            real_repo = real_parent / "repo"
            real_repo.mkdir(parents=True)
            linked_parent = root / "linked-parent"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)
            linked_repo = linked_parent / "repo"
            summary_path = root / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(linked_repo),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary_text = summary_path.read_text(encoding="utf-8")
            rendered = stdout.getvalue() + stderr.getvalue() + summary_text

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_repo_root_path_invalid", rendered)
        self.assertIn("--repo-root ancestor directory must not be a symlink", rendered)
        self.assertNotIn(str(linked_repo), rendered)
        self.assertNotIn(str(real_repo), rendered)
        self.assertNotIn(str(linked_parent), rendered)

    def test_secret_looking_summary_out_blocks_before_write_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            create_complete_matrix(root, signer)
            secret_summary_path = Path(temp) / "token=supersecret-summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(secret_summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertFalse(secret_summary_path.exists())
        self.assertNotIn("token=supersecret", rendered)
        self.assertIn("kagemusha_summary_out_path_invalid", rendered)
        self.assertIn("--summary-out must not contain secret-looking material", rendered)

    def test_write_summary_rejects_secret_path_before_direct_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_summary_path = Path(temp) / "token=supersecret-summary.json"

            errors = readiness.write_summary(
                secret_summary_path,
                {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
            )
            rendered = json.dumps(errors)

        self.assertFalse(secret_summary_path.exists())
        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out must not contain secret-looking material",
                }
            ],
        )
        self.assertNotIn(str(secret_summary_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_symlinked_summary_out_blocks_without_following_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            create_complete_matrix(root, signer)
            target = Path(temp) / "external-summary.json"
            target.write_text("external\n", encoding="utf-8")
            summary_link = Path(temp) / "summary.json"
            try:
                summary_link.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_link),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertEqual(target_text, "external\n")
        self.assertIn("kagemusha_summary_out_path_invalid", rendered)
        self.assertIn("--summary-out must not be a symlink", rendered)
        self.assertNotIn("[kagemusha-readiness] wrote summary", rendered)
        self.assertNotIn(str(summary_link), rendered)

    def test_symlinked_summary_out_ancestor_blocks_before_creating_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "external"
            real_parent.mkdir()
            linked_parent = root / "linked"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)
            summary_path = linked_parent / "nested" / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertFalse((real_parent / "nested").exists())
        self.assertIn("kagemusha_summary_out_path_invalid", rendered)
        self.assertIn("--summary-out ancestor directory must not be a symlink", rendered)
        self.assertNotIn("[kagemusha-readiness] wrote summary", rendered)
        self.assertNotIn(str(summary_path), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)

    def test_hardlinked_summary_out_blocks_without_overwriting_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            create_complete_matrix(root, signer)
            target = Path(temp) / "external-summary.json"
            target.write_text("external\n", encoding="utf-8")
            summary_path = Path(temp) / "summary.json"
            summary_path.write_text("placeholder\n", encoding="utf-8")
            slot_helpers.replace_with_hardlink(self, summary_path, target)
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertEqual(target_text, "external\n")
        self.assertIn("kagemusha_summary_out_path_invalid", rendered)
        self.assertIn("--summary-out must not be hardlinked", rendered)
        self.assertNotIn("[kagemusha-readiness] wrote summary", rendered)
        self.assertNotIn(str(summary_path), rendered)

    def test_secret_looking_trusted_signer_path_blocks_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = slot_helpers.create_test_signer(Path(temp) / "keys")
            create_complete_matrix(root, signer)
            secret_public_key_path = Path(temp) / "token=supersecret-public.pem"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--device-lab-root",
                        str(root),
                        "--trusted-signer-public-key",
                        str(secret_public_key_path),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary_text = summary_path.read_text(encoding="utf-8")
            rendered = stdout.getvalue() + stderr.getvalue() + summary_text

        self.assertEqual(status, 1)
        self.assertNotIn("token=supersecret", rendered)
        self.assertIn("android_trusted_signer_path_invalid", rendered)
        self.assertIn(
            "--trusted-signer-public-key[0] must not contain secret-looking material",
            rendered,
        )

    def test_negative_lineage_proof_future_skew_blocks_before_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--max-lineage-proof-evidence-future-skew-seconds",
                        "-1",
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 1)
        self.assertFalse(summary["ready"])
        self.assertIn(
            "lineage_proof_evidence_max_timestamp_invalid",
            {item["code"] for item in summary["blockers"]},
        )
        self.assertIn("lineage_proof_evidence_max_timestamp_invalid", stderr.getvalue())


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
