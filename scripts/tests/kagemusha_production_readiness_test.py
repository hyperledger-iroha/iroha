"""Tests for scripts/kagemusha_production_readiness.py."""

from __future__ import annotations

import importlib.util
import hashlib
import io
import json
import shutil
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import tempfile
import unittest
from unittest import mock


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

COMPACT_KEY_HELPER_PATH = SCRIPT_DIR / "kagemusha_recursive_compact_key_evidence.py"
COMPACT_KEY_HELPER_SPEC = importlib.util.spec_from_file_location(
    "kagemusha_recursive_compact_key_evidence",
    COMPACT_KEY_HELPER_PATH,
)
assert COMPACT_KEY_HELPER_SPEC and COMPACT_KEY_HELPER_SPEC.loader  # pragma: no cover
compact_key_helper = importlib.util.module_from_spec(COMPACT_KEY_HELPER_SPEC)
COMPACT_KEY_HELPER_SPEC.loader.exec_module(compact_key_helper)  # type: ignore[misc]

RELEASE_BUNDLE_HELPER_PATH = SCRIPT_DIR / "kagemusha_release_bundle.py"
RELEASE_BUNDLE_HELPER_SPEC = importlib.util.spec_from_file_location(
    "kagemusha_release_bundle",
    RELEASE_BUNDLE_HELPER_PATH,
)
assert RELEASE_BUNDLE_HELPER_SPEC and RELEASE_BUNDLE_HELPER_SPEC.loader  # pragma: no cover
release_bundle = importlib.util.module_from_spec(RELEASE_BUNDLE_HELPER_SPEC)
RELEASE_BUNDLE_HELPER_SPEC.loader.exec_module(release_bundle)  # type: ignore[misc]

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
    create_compact_key_evidence(root)
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
            "artifact_size_bytes": {
                artifact: (root / artifact).stat().st_size
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


def create_compact_key_artifact_files(root: Path) -> None:
    for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS:
        path = root / artifact
        path.parent.mkdir(parents=True, exist_ok=True)
        digest = hashlib.sha256(f"fixture compact artifact {artifact}".encode("utf-8")).digest()
        path.write_bytes(b"KCGK\x00\x01" + digest + digest[::-1])
    write_compact_key_generator_log(root)


def write_compact_key_generator_log(root: Path) -> Path:
    log_path = root / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
    sizes = {
        artifact: (root / artifact).stat().st_size
        for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
    }
    log_path.write_text(
        readiness.expected_compact_key_generator_log_line(sizes) + "\n",
        encoding="utf-8",
    )
    return log_path


def create_compact_key_evidence(root: Path) -> Path:
    create_compact_key_artifact_files(root)
    generator_log = root / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
    evidence_path = root / readiness.COMPACT_KEY_EVIDENCE_FILENAME
    write_json(
        evidence_path,
        {
            "schema": readiness.COMPACT_KEY_EVIDENCE_SCHEMA,
            "generated_at_utc": readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            "opening_len": readiness.EXPECTED_COMPACT_KEY_OPENING_LEN,
            "ipa_k": readiness.EXPECTED_COMPACT_KEY_IPA_K,
            "verifier_backend": readiness.EXPECTED_COMPACT_KEY_BACKEND,
            "circuit_id": readiness.EXPECTED_COMPACT_KEY_CIRCUIT_ID,
            "record_namespace": readiness.EXPECTED_COMPACT_KEY_RECORD_NAMESPACE,
            "record_version": readiness.EXPECTED_COMPACT_KEY_RECORD_VERSION,
            "command": readiness.expected_compact_key_command(),
            "generator_log_path": readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME,
            "generator_log_sha256": hashlib.sha256(generator_log.read_bytes()).hexdigest(),
            "artifacts": {
                artifact: hashlib.sha256((root / artifact).read_bytes()).hexdigest()
                for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
            },
            "artifact_size_bytes": {
                artifact: (root / artifact).stat().st_size
                for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
            },
        },
    )
    return evidence_path


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
                "multi-hop proving requires the append verifier batch to be composed into the compact proof",
                "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
                "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN",
                "KAGEMUSHA_RECURSIVE_COMPACT_MIN_PROOF_BYTES",
                "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope",
                "height-aware detached compact Pallas archive must reject before proving",
                "height-aware extra compact Pallas opening must reject before proving",
                "height-aware missing compact Pallas opening must reject before proving",
                "duplicated multi-hop compact Pallas archive must reject before proving",
                "height-aware duplicated multi-hop compact Pallas archive must reject before proving",
                "forged multi-hop compact Pallas metadata must reject before proving",
                "height-aware forged multi-hop compact Pallas metadata must reject before proving",
                "reordered multi-hop compact Pallas archive must reject before proving",
                "height-aware reordered multi-hop compact Pallas archive must reject before proving",
                "fn prove_kagemusha_recursive_compact_payment_token_one_hop_from_record_bundle_and_pallas_open_envelopes(",
                ") -> Result<(), String> {",
                "    kagemusha_pallas_ipa_batch_verifier_preflight_bound_to_hop_proofs();",
                "    validate_kagemusha_recursive_one_hop_verifier_slice_preflight_binding();",
                "    kagemusha_recursive_spend_lineage_runtime_keygen_enabled();",
                "    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED;",
                "    \"missing compact one-hop proving key archive\";",
                "    prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch();",
                "}",
                "fn prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
                ") -> Result<(), String> {",
                "    prove_kagemusha_recursive_compact_payment_token_one_hop_from_record_bundle_and_pallas_open_envelopes();",
                "    for hop_index in 1..hop_count {",
                "        kagemusha_recursive_spend_lineage_runtime_keygen_enabled();",
                "        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED;",
                "        \"missing compact append proving key archive\";",
                "        prove_halo2_ipa_kagemusha_recursive_compact_payment_token_append_envelope_dispatch();",
                "    }",
                "}",
                "fn prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch(",
                ") -> Result<(), String> {",
                "    prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope::<$len>();",
                "    match usize::try_from(preflight.opening_len) {",
                "        4 => prove_len!(4),",
                "        _ => Err(\"unsupported\".to_owned()),",
                "    }",
                "}",
                "pub fn prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive(",
                ") -> Result<(), String> {",
                "    prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes();",
                "    proving_key_bytes;",
                "    None",
                "}",
                "pub fn prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_at_height(",
                ") -> Result<(), String> {",
                "    prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes();",
                "    proving_key_bytes;",
                "    Some(block_height);",
                "}",
                "pub fn preverify_kagemusha_recursive_compact_payment_token(",
                ") -> Result<(), String> {",
                "    preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id();",
                "    KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
                "}",
                "pub fn verify_kagemusha_recursive_compact_payment_token(",
                ") -> bool {",
                "    preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id();",
                "    verify_backend();",
                "}",
                "pub fn preverify_kagemusha_recursive_compact_payment_token_with_record(",
                ") -> Result<(), String> {",
                "    preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id();",
                "    KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
                "}",
                "pub fn preverify_kagemusha_recursive_compact_payment_token_with_record_at_height(",
                ") -> Result<(), String> {",
                "    preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id();",
                "    Some(block_height);",
                "    KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
                "}",
                "pub fn verify_kagemusha_recursive_compact_payment_token_with_record(",
                ") -> bool {",
                "    preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id();",
                "    verify_backend();",
                "}",
                "pub fn verify_kagemusha_recursive_compact_payment_token_with_record_at_height(",
                ") -> bool {",
                "    preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id();",
                "    Some(block_height);",
                "    verify_backend();",
                "}",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    bridge_path = repo / "crates/connect_norito_bridge/src/lib.rs"
    bridge_path.parent.mkdir(parents=True, exist_ok=True)
    bridge_path.write_text(
        "\n".join(
            (
                "ERR_KAGEMUSHA_RECURSIVE_COMPACT_UNAVAILABLE",
                "pub unsafe extern \"C\" fn connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(",
                ") -> c_int {",
                "    prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts()",
                "        .map_err(|err| {",
                "            if is_kagemusha_recursive_compact_unavailable_error(&err) {",
                "                BridgeError::KagemushaRecursiveCompactUnavailable",
                "            } else {",
                "                BridgeError::KagemushaProve",
                "            }",
                "        })?;",
                "    0",
                "}",
                "pub unsafe extern \"C\" fn connect_norito_kagemusha_verify_recursive_compact_payment_token(",
                ") -> c_int {",
                "    match preverify_kagemusha_recursive_compact_payment_token(&token, vk_box) {",
                "        Err(err) if is_kagemusha_recursive_compact_unavailable_error(&err) => {}",
                "        _ => {}",
                "    }",
                "    verify_kagemusha_recursive_compact_payment_token(&token, vk_box);",
                "    *out_valid = 0;",
                "    0",
                "}",
            )
        )
        + "\n",
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


def create_ready_release_bundle_fixture(root: Path) -> dict[str, object]:
    bundle_root = root / "bundle"
    device_lab_root = bundle_root / "artifacts" / "android" / "device_lab"
    lineage_evidence = create_lineage_proof_evidence(
        bundle_root / "artifacts" / "kagemusha"
    )
    compact_key_evidence = lineage_evidence.parent / readiness.COMPACT_KEY_EVIDENCE_FILENAME
    signer = slot_helpers.create_test_signer(root / "keys")
    create_complete_matrix(device_lab_root, signer)
    trusted, signer_errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
        [str(signer["public_key"])]
    )
    assert not signer_errors
    summary = readiness.build_summary(
        repo_root=REPO_ROOT,
        device_lab_root=device_lab_root,
        lineage_proof_evidence_path=lineage_evidence,
        compact_key_evidence_path=compact_key_evidence,
        trusted_signer_public_keys=trusted,
        min_signed_at=readiness.parse_utc_timestamp(
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            "fixture min signed_at",
        )[0],
        min_lineage_proof_evidence_at=readiness.parse_utc_timestamp(
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            "fixture min lineage evidence",
        )[0],
        min_compact_key_evidence_at=readiness.parse_utc_timestamp(
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            "fixture min compact evidence",
        )[0],
    )
    assert summary["ready"], summary["blockers"]
    summary_path = bundle_root / "dist" / "kagemusha-production-readiness.json"
    write_json(summary_path, summary)
    return {
        "bundle_root": bundle_root,
        "device_lab_root": device_lab_root,
        "lineage_evidence": lineage_evidence,
        "compact_key_evidence": compact_key_evidence,
        "signer": signer,
        "summary_path": summary_path,
        "summary": summary,
    }


def release_bundle_args(fixture: dict[str, object], *, out: Path | None = None) -> list[str]:
    bundle_root = fixture["bundle_root"]
    signer = fixture["signer"]
    assert isinstance(bundle_root, Path)
    assert isinstance(signer, dict)
    if out is None:
        out_arg = "dist/kagemusha-production-release-bundle.json"
    else:
        out_arg = str(out)
    return [
        "--repo-root",
        str(REPO_ROOT),
        "--bundle-root",
        str(bundle_root),
        "--readiness-summary",
        "dist/kagemusha-production-readiness.json",
        "--lineage-proof-evidence",
        f"artifacts/kagemusha/{readiness.LINEAGE_PROOF_EVIDENCE_FILENAME}",
        "--compact-key-evidence",
        f"artifacts/kagemusha/{readiness.COMPACT_KEY_EVIDENCE_FILENAME}",
        "--device-lab-root",
        "artifacts/android/device_lab",
        "--trusted-signer-public-key",
        str(signer["public_key"]),
        "--out",
        out_arg,
    ]


class KagemushaProductionReadinessTest(unittest.TestCase):
    def test_complete_signed_android_matrix_passes_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            lineage_evidence = create_lineage_proof_evidence(Path(temp) / "lineage")
            compact_key_evidence = create_compact_key_evidence(lineage_evidence.parent)
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
                        "--compact-key-evidence",
                        str(compact_key_evidence),
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
            expected_artifact_sizes = {
                artifact: (lineage_evidence.parent / artifact).stat().st_size
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            expected_compact_artifact_sha256 = {
                artifact: hashlib.sha256(
                    (lineage_evidence.parent / artifact).read_bytes()
                ).hexdigest()
                for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
            }
            expected_compact_artifact_sizes = {
                artifact: (lineage_evidence.parent / artifact).stat().st_size
                for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
            }
            expected_compact_generator_log_sha256 = hashlib.sha256(
                (
                    lineage_evidence.parent
                    / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
                ).read_bytes()
            ).hexdigest()
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
                    "offline_wallet_apk_path": metadata["offline_wallet_apk_path"],
                    "offline_wallet_apk_sha256": metadata["offline_wallet_apk_sha256"],
                    "d2d_payment_transcript_path": metadata[
                        "d2d_payment_transcript_path"
                    ],
                    "d2d_payment_transcript_sha256": metadata[
                        "d2d_payment_transcript_sha256"
                    ],
                    "wallet_integrity_transcript_path": metadata[
                        "wallet_integrity_transcript_path"
                    ],
                    "wallet_integrity_transcript_sha256": metadata[
                        "wallet_integrity_transcript_sha256"
                    ],
                    "attestation_certificate_chain_path": metadata[
                        "attestation_certificate_chain_path"
                    ],
                    "attestation_certificate_chain_sha256": metadata[
                        "attestation_certificate_chain_sha256"
                    ],
                }

        self.assertEqual(status, 0)
        self.assertTrue(summary["ready"])
        self.assertEqual(summary["status"], "ready")
        self.assertEqual(summary["schema"], readiness.SUMMARY_SCHEMA)
        self.assertTrue(summary["abi6_reserved_lineage"]["ok"])
        self.assertEqual(
            summary["abi7_recursive_compact"]["state"],
            "package_aware_multi_hop_composed",
        )
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
            summary["lineage_proof_evidence"]["artifact_size_bytes"],
            expected_artifact_sizes,
        )
        self.assertEqual(
            summary["lineage_proof_evidence"]["test_log_sha256"],
            {"record_archive_proof": expected_log_sha256},
        )
        self.assertIsNotNone(summary["lineage_proof_evidence"]["max_generated_at_utc"])
        self.assertEqual(
            summary["compact_key_evidence"]["state"],
            "compact_key_artifacts_validated",
        )
        self.assertEqual(
            summary["compact_key_evidence"]["generated_at_utc"],
            readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        )
        self.assertEqual(
            summary["compact_key_evidence"]["artifact_sha256"],
            expected_compact_artifact_sha256,
        )
        self.assertEqual(
            summary["compact_key_evidence"]["artifact_size_bytes"],
            expected_compact_artifact_sizes,
        )
        self.assertEqual(
            summary["compact_key_evidence"]["generator_log_sha256"],
            expected_compact_generator_log_sha256,
        )
        self.assertEqual(
            summary["compact_key_evidence"]["generator_log_artifact_size_bytes"],
            expected_compact_artifact_sizes,
        )
        self.assertTrue(summary["compact_key_evidence"]["command_validated"])
        self.assertIsNotNone(summary["compact_key_evidence"]["max_generated_at_utc"])
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

    def test_kagemusha_release_bundle_manifest_passes_ready_fixture(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))

            manifest = json.loads(out.read_text(encoding="utf-8"))
            rendered = json.dumps(manifest, sort_keys=True)
            expected_lineage_artifact_sizes = {
                artifact: (
                    bundle_root / "artifacts" / "kagemusha" / artifact
                ).stat().st_size
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            expected_lineage_artifact_sha256 = {
                artifact: hashlib.sha256(
                    (bundle_root / "artifacts" / "kagemusha" / artifact).read_bytes()
                ).hexdigest()
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            expected_compact_artifact_sizes = {
                artifact: (
                    bundle_root / "artifacts" / "kagemusha" / artifact
                ).stat().st_size
                for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
            }
            expected_compact_artifact_sha256 = {
                artifact: hashlib.sha256(
                    (bundle_root / "artifacts" / "kagemusha" / artifact).read_bytes()
                ).hexdigest()
                for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
            }
            expected_lineage_log_sha256 = hashlib.sha256(
                (
                    bundle_root
                    / "artifacts"
                    / "kagemusha"
                    / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
                ).read_bytes()
            ).hexdigest()
            expected_lineage_log_size = (
                bundle_root
                / "artifacts"
                / "kagemusha"
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            ).stat().st_size
            compact_generator_log = (
                bundle_root
                / "artifacts"
                / "kagemusha"
                / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
            )
            expected_compact_generator_log_sha256 = hashlib.sha256(
                compact_generator_log.read_bytes()
            ).hexdigest()
            expected_compact_generator_log_size = compact_generator_log.stat().st_size

        self.assertEqual(status, 0)
        self.assertTrue(manifest["ready"])
        self.assertEqual(manifest["schema"], release_bundle.RELEASE_BUNDLE_SCHEMA)
        self.assertEqual(manifest["blockers"], [])
        self.assertEqual(
            manifest["evidence"]["readiness_summary"]["path"],
            "dist/kagemusha-production-readiness.json",
        )
        self.assertEqual(
            manifest["evidence"]["lineage_proof_evidence"]["path"],
            "artifacts/kagemusha/lineage-proof-evidence.json",
        )
        self.assertEqual(
            manifest["evidence"]["compact_key_evidence"]["path"],
            "artifacts/kagemusha/recursive-compact-key-evidence.json",
        )
        self.assertNotIn(str(bundle_root), rendered)
        self.assertEqual(
            manifest["android_device_lab"]["missing_device_families"],
            [],
        )
        self.assertEqual(
            manifest["lineage_proof_evidence"]["artifact_size_bytes"],
            expected_lineage_artifact_sizes,
        )
        self.assertEqual(
            set(manifest["evidence"]["lineage_artifacts"]),
            set(readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS),
        )
        for artifact, entry in manifest["evidence"]["lineage_artifacts"].items():
            self.assertEqual(entry["path"], f"artifacts/kagemusha/{artifact}")
            self.assertEqual(entry["sha256"], expected_lineage_artifact_sha256[artifact])
            self.assertEqual(entry["size_bytes"], expected_lineage_artifact_sizes[artifact])
        self.assertEqual(
            set(manifest["evidence"]["compact_key_artifacts"]),
            set(readiness.COMPACT_KEY_REQUIRED_ARTIFACTS),
        )
        for artifact, entry in manifest["evidence"]["compact_key_artifacts"].items():
            self.assertEqual(entry["path"], f"artifacts/kagemusha/{artifact}")
            self.assertEqual(entry["sha256"], expected_compact_artifact_sha256[artifact])
            self.assertEqual(entry["size_bytes"], expected_compact_artifact_sizes[artifact])
        self.assertEqual(
            manifest["compact_key_evidence"]["generator_log_sha256"],
            expected_compact_generator_log_sha256,
        )
        self.assertEqual(
            manifest["compact_key_evidence"]["generator_log_artifact_size_bytes"],
            expected_compact_artifact_sizes,
        )
        self.assertEqual(
            manifest["evidence"]["compact_key_generator_log"],
            {
                "path": "artifacts/kagemusha/recursive-compact-key-artifacts.log",
                "sha256": expected_compact_generator_log_sha256,
                "size_bytes": expected_compact_generator_log_size,
            },
        )
        self.assertEqual(
            manifest["evidence"]["lineage_proof_logs"]["record_archive_proof"],
            {
                "path": "artifacts/kagemusha/record-archive-proof.log",
                "sha256": expected_lineage_log_sha256,
                "size_bytes": expected_lineage_log_size,
            },
        )
        self.assertEqual(
            set(manifest["evidence"]["android_signed_evidence"]),
            set(manifest["android_device_lab"]["signed_evidence"]),
        )
        for slot, entry in manifest["evidence"]["android_signed_evidence"].items():
            self.assertEqual(
                entry["path"],
                f"artifacts/android/device_lab/{slot}/evidence/signed-evidence.json",
            )
            self.assertEqual(
                entry["sha256"],
                manifest["android_device_lab"]["signed_evidence"][slot][
                    "artifact_sha256"
                ],
            )
        self.assertEqual(
            set(manifest["evidence"]["android_slot_artifacts"]),
            set(manifest["android_device_lab"]["signed_evidence"]),
        )
        for slot, artifacts in manifest["evidence"]["android_slot_artifacts"].items():
            summary = manifest["android_device_lab"]["signed_evidence"][slot]
            self.assertEqual(
                set(artifacts),
                {
                    "offline_wallet_apk",
                    "d2d_payment_transcript",
                    "wallet_integrity_transcript",
                    "attestation_certificate_chain",
                },
            )
            self.assertEqual(
                artifacts["offline_wallet_apk"]["path"],
                f"artifacts/android/device_lab/{slot}/{summary['offline_wallet_apk_path']}",
            )
            self.assertEqual(
                artifacts["offline_wallet_apk"]["sha256"],
                summary["offline_wallet_apk_sha256"],
            )
            self.assertGreater(artifacts["offline_wallet_apk"]["size_bytes"], 0)
            self.assertEqual(
                artifacts["d2d_payment_transcript"]["path"],
                (
                    f"artifacts/android/device_lab/{slot}/"
                    f"{summary['d2d_payment_transcript_path']}"
                ),
            )
            self.assertEqual(
                artifacts["d2d_payment_transcript"]["sha256"],
                summary["d2d_payment_transcript_sha256"],
            )
            self.assertGreater(
                artifacts["d2d_payment_transcript"]["size_bytes"],
                0,
            )
            self.assertEqual(
                artifacts["wallet_integrity_transcript"]["path"],
                (
                    f"artifacts/android/device_lab/{slot}/"
                    f"{summary['wallet_integrity_transcript_path']}"
                ),
            )
            self.assertEqual(
                artifacts["wallet_integrity_transcript"]["sha256"],
                summary["wallet_integrity_transcript_sha256"],
            )
            self.assertGreater(
                artifacts["wallet_integrity_transcript"]["size_bytes"],
                0,
            )
            self.assertEqual(
                artifacts["attestation_certificate_chain"]["path"],
                (
                    f"artifacts/android/device_lab/{slot}/"
                    f"{summary['attestation_certificate_chain_path']}"
                ),
            )
            self.assertEqual(
                artifacts["attestation_certificate_chain"]["sha256"],
                summary["attestation_certificate_chain_sha256"],
            )
            self.assertGreater(
                artifacts["attestation_certificate_chain"]["size_bytes"],
                0,
            )

    def test_kagemusha_release_bundle_rejects_missing_android_slot_apk_after_validation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary = fixture["summary"]
            device_lab_root = fixture["device_lab_root"]
            assert isinstance(summary, dict)
            assert isinstance(device_lab_root, Path)
            android = json.loads(json.dumps(summary["android_device_lab"]))
            slot = next(iter(android["signed_evidence"]))
            apk_relative = android["signed_evidence"][slot]["offline_wallet_apk_path"]
            (device_lab_root / slot / apk_relative).unlink()
            stderr = io.StringIO()

            with mock.patch.object(
                release_bundle.readiness,
                "check_android_device_lab",
                return_value=android,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture))

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_android_slot_artifact_file_shape", rendered)
        self.assertIn("kagemusha_release_android_slot_artifact_inventory", rendered)

    def test_kagemusha_release_bundle_rejects_android_slot_attestation_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary = fixture["summary"]
            device_lab_root = fixture["device_lab_root"]
            assert isinstance(summary, dict)
            assert isinstance(device_lab_root, Path)
            android = json.loads(json.dumps(summary["android_device_lab"]))
            slot = next(iter(android["signed_evidence"]))
            chain_relative = android["signed_evidence"][slot][
                "attestation_certificate_chain_path"
            ]
            (device_lab_root / slot / chain_relative).write_bytes(
                b"tampered attestation chain\n"
            )
            stderr = io.StringIO()

            with mock.patch.object(
                release_bundle.readiness,
                "check_android_device_lab",
                return_value=android,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture))

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_android_slot_artifact_digest_drift", rendered)
        self.assertIn("kagemusha_release_android_slot_artifact_inventory", rendered)

    def test_kagemusha_release_bundle_rejects_android_slot_d2d_transcript_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary = fixture["summary"]
            device_lab_root = fixture["device_lab_root"]
            assert isinstance(summary, dict)
            assert isinstance(device_lab_root, Path)
            android = json.loads(json.dumps(summary["android_device_lab"]))
            slot = next(iter(android["signed_evidence"]))
            d2d_relative = android["signed_evidence"][slot][
                "d2d_payment_transcript_path"
            ]
            (device_lab_root / slot / d2d_relative).write_text(
                "{}\n",
                encoding="utf-8",
            )
            stderr = io.StringIO()

            with mock.patch.object(
                release_bundle.readiness,
                "check_android_device_lab",
                return_value=android,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture))

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_android_slot_artifact_digest_drift", rendered)
        self.assertIn("kagemusha_release_android_slot_artifact_inventory", rendered)

    def test_kagemusha_release_bundle_rejects_android_slot_wallet_transcript_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary = fixture["summary"]
            device_lab_root = fixture["device_lab_root"]
            assert isinstance(summary, dict)
            assert isinstance(device_lab_root, Path)
            android = json.loads(json.dumps(summary["android_device_lab"]))
            slot = next(iter(android["signed_evidence"]))
            wallet_relative = android["signed_evidence"][slot][
                "wallet_integrity_transcript_path"
            ]
            (device_lab_root / slot / wallet_relative).write_text(
                "{}\n",
                encoding="utf-8",
            )
            stderr = io.StringIO()

            with mock.patch.object(
                release_bundle.readiness,
                "check_android_device_lab",
                return_value=android,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture))

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_android_slot_artifact_digest_drift", rendered)
        self.assertIn("kagemusha_release_android_slot_artifact_inventory", rendered)

    def test_kagemusha_release_bundle_rejects_all_zero_lineage_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            lineage_proof_evidence = fixture["lineage_evidence"]
            summary_path = fixture["summary_path"]
            assert isinstance(bundle_root, Path)
            assert isinstance(lineage_proof_evidence, Path)
            assert isinstance(summary_path, Path)
            artifact = bundle_root / "artifacts" / "kagemusha" / "lineage-init-len128.pk"
            zero_artifact = b"\x00" * 64
            artifact.write_bytes(zero_artifact)
            zero_sha256 = hashlib.sha256(zero_artifact).hexdigest()
            lineage_evidence = json.loads(lineage_proof_evidence.read_text(encoding="utf-8"))
            lineage_evidence["artifacts"][artifact.name] = zero_sha256
            lineage_evidence["artifact_size_bytes"][artifact.name] = len(zero_artifact)
            write_json(lineage_proof_evidence, lineage_evidence)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["lineage_proof_evidence"]["artifact_sha256"][artifact.name] = zero_sha256
            summary["lineage_proof_evidence"]["artifact_size_bytes"][artifact.name] = len(
                zero_artifact
            )
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        rendered = stderr.getvalue()
        self.assertIn("lineage_proof_evidence_artifact_placeholder", rendered)
        self.assertIn("kagemusha_release_lineage_artifact_placeholder", rendered)

    def test_kagemusha_release_bundle_rejects_placeholder_compact_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            compact_key_evidence = fixture["compact_key_evidence"]
            summary_path = fixture["summary_path"]
            assert isinstance(bundle_root, Path)
            assert isinstance(compact_key_evidence, Path)
            assert isinstance(summary_path, Path)
            artifact = bundle_root / "artifacts" / "kagemusha" / "recursive-compact-len4.pk"
            placeholder = b"recursive compact key artifact recursive-compact-len4.pk\n"
            artifact.write_bytes(placeholder)
            placeholder_sha256 = hashlib.sha256(placeholder).hexdigest()
            compact_evidence = json.loads(compact_key_evidence.read_text(encoding="utf-8"))
            compact_evidence["artifacts"][artifact.name] = placeholder_sha256
            compact_evidence["artifact_size_bytes"][artifact.name] = len(placeholder)
            write_json(compact_key_evidence, compact_evidence)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["compact_key_evidence"]["artifact_sha256"][
                artifact.name
            ] = placeholder_sha256
            summary["compact_key_evidence"]["artifact_size_bytes"][artifact.name] = len(
                placeholder
            )
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        rendered = stderr.getvalue()
        self.assertIn("compact_key_evidence_artifact_placeholder", rendered)
        self.assertIn("kagemusha_release_compact_artifact_placeholder", rendered)

    def test_kagemusha_release_bundle_rejects_all_placeholder_compact_prefixes(self) -> None:
        for marker in readiness.COMPACT_KEY_PLACEHOLDER_PREFIXES:
            with self.subTest(marker=marker):
                with tempfile.TemporaryDirectory() as temp:
                    fixture = create_ready_release_bundle_fixture(Path(temp))
                    bundle_root = fixture["bundle_root"]
                    compact_key_evidence = fixture["compact_key_evidence"]
                    summary_path = fixture["summary_path"]
                    assert isinstance(bundle_root, Path)
                    assert isinstance(compact_key_evidence, Path)
                    assert isinstance(summary_path, Path)
                    artifact = (
                        bundle_root
                        / "artifacts"
                        / "kagemusha"
                        / "recursive-compact-len4.pk"
                    )
                    placeholder = marker + b"recursive-compact-len4.pk\n"
                    artifact.write_bytes(placeholder)
                    placeholder_sha256 = hashlib.sha256(placeholder).hexdigest()
                    compact_evidence = json.loads(
                        compact_key_evidence.read_text(encoding="utf-8")
                    )
                    compact_evidence["artifacts"][artifact.name] = placeholder_sha256
                    compact_evidence["artifact_size_bytes"][artifact.name] = len(
                        placeholder
                    )
                    write_json(compact_key_evidence, compact_evidence)
                    summary = json.loads(summary_path.read_text(encoding="utf-8"))
                    summary["compact_key_evidence"]["artifact_sha256"][
                        artifact.name
                    ] = placeholder_sha256
                    summary["compact_key_evidence"]["artifact_size_bytes"][
                        artifact.name
                    ] = len(placeholder)
                    write_json(summary_path, summary)
                    stderr = io.StringIO()

                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = release_bundle.main(release_bundle_args(fixture))

                self.assertEqual(status, 1)
                rendered = stderr.getvalue()
                self.assertIn("compact_key_evidence_artifact_placeholder", rendered)
                self.assertIn("kagemusha_release_compact_artifact_placeholder", rendered)

    def test_kagemusha_release_bundle_rejects_all_zero_compact_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            compact_key_evidence = fixture["compact_key_evidence"]
            summary_path = fixture["summary_path"]
            assert isinstance(bundle_root, Path)
            assert isinstance(compact_key_evidence, Path)
            assert isinstance(summary_path, Path)
            artifact = bundle_root / "artifacts" / "kagemusha" / "recursive-compact-len4.pk"
            zero_artifact = b"\x00" * 64
            artifact.write_bytes(zero_artifact)
            zero_sha256 = hashlib.sha256(zero_artifact).hexdigest()
            compact_evidence = json.loads(compact_key_evidence.read_text(encoding="utf-8"))
            compact_evidence["artifacts"][artifact.name] = zero_sha256
            compact_evidence["artifact_size_bytes"][artifact.name] = len(zero_artifact)
            write_json(compact_key_evidence, compact_evidence)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["compact_key_evidence"]["artifact_sha256"][artifact.name] = zero_sha256
            summary["compact_key_evidence"]["artifact_size_bytes"][artifact.name] = len(
                zero_artifact
            )
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        rendered = stderr.getvalue()
        self.assertIn("compact_key_evidence_artifact_placeholder", rendered)
        self.assertIn("kagemusha_release_compact_artifact_placeholder", rendered)

    def test_kagemusha_release_bundle_verify_existing_passes_ready_fixture(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            original_manifest = out.read_text(encoding="utf-8")
            stdout = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(io.StringIO()):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )
            final_manifest = out.read_text(encoding="utf-8")

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 0)
        self.assertEqual(final_manifest, original_manifest)
        self.assertIn("[kagemusha-release-bundle] verified", stdout.getvalue())

    def test_kagemusha_release_bundle_verify_existing_allows_timestamp_refresh(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest = json.loads(out.read_text(encoding="utf-8"))
            manifest["generated_at_utc"] = "2026-01-08T00:00:00Z"
            write_json(out, manifest)
            stdout = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(io.StringIO()):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 0)
        self.assertIn("[kagemusha-release-bundle] verified", stdout.getvalue())

    def test_kagemusha_release_bundle_verify_existing_rejects_manifest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest = json.loads(out.read_text(encoding="utf-8"))
            manifest["evidence"]["readiness_summary"]["sha256"] = "0" * 64
            write_json(out, manifest)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 1)
        self.assertIn(
            "kagemusha_release_bundle_manifest_drift",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_verify_existing_rejects_missing_android_slot_artifacts(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest = json.loads(out.read_text(encoding="utf-8"))
            del manifest["evidence"]["android_slot_artifacts"]
            write_json(out, manifest)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 1)
        self.assertIn(
            "kagemusha_release_bundle_manifest_drift",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_verify_existing_rejects_unexpected_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest = json.loads(out.read_text(encoding="utf-8"))
            manifest["unexpected_release_claim"] = "production-ready"
            write_json(out, manifest)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 1)
        self.assertIn(
            "kagemusha_release_bundle_manifest_unexpected_field",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_verify_existing_rejects_secret_material(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest = json.loads(out.read_text(encoding="utf-8"))
            manifest["evidence"]["readiness_summary"]["path"] = "evidence/token=abc123.json"
            write_json(out, manifest)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 1)
        self.assertIn(
            "kagemusha_release_bundle_manifest_secret_material",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_verify_existing_rejects_duplicate_manifest_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest_text = out.read_text(encoding="utf-8")
            out.write_text(
                manifest_text.replace(
                    '"schema": "iroha.kagemusha.production_release_bundle.v1"',
                    (
                        '"schema": "iroha.kagemusha.production_release_bundle.v1", '
                        '"schema": "shadow"'
                    ),
                    1,
                ),
                encoding="utf-8",
            )
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 1)
        self.assertIn("kagemusha_release_bundle_manifest_invalid_json", stderr.getvalue())
        self.assertIn("duplicate JSON object key schema", stderr.getvalue())

    def test_kagemusha_release_bundle_verify_existing_rejects_nonfinite_manifest_json_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest = json.loads(out.read_text(encoding="utf-8"))
            manifest["generated_at_utc"] = float("nan")
            out.write_text(
                json.dumps(manifest, indent=2, sort_keys=True, allow_nan=True) + "\n",
                encoding="utf-8",
            )
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 1)
        self.assertIn("kagemusha_release_bundle_manifest_invalid_json", stderr.getvalue())
        self.assertIn("non-finite constant NaN is not allowed", stderr.getvalue())

    def test_kagemusha_release_bundle_verify_existing_rejects_noncanonical_manifest_timestamp(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = release_bundle.main(release_bundle_args(fixture))
            manifest = json.loads(out.read_text(encoding="utf-8"))
            manifest["generated_at_utc"] = "2026-01-08T00:00:00+00:00"
            write_json(out, manifest)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                verify_status = release_bundle.main(
                    [
                        *release_bundle_args(fixture),
                        "--verify-existing",
                        "dist/kagemusha-production-release-bundle.json",
                    ]
                )

        self.assertEqual(status, 0)
        self.assertEqual(verify_status, 1)
        self.assertIn("kagemusha_release_bundle_manifest_timestamp", stderr.getvalue())
        self.assertIn("must be canonical UTC YYYY-MM-DDTHH:MM:SSZ", stderr.getvalue())

    def test_kagemusha_release_bundle_verify_existing_rejects_outside_manifest_before_scanners(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            outside = Path(temp) / "outside-release-bundle.json"
            outside.write_text('{"leak": "token=supersecret"}\n', encoding="utf-8")
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    release_bundle,
                    "_load_local_json",
                    side_effect=AssertionError("outside release bundle must not be loaded"),
                ) as load_json,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_lineage_proof_evidence",
                    side_effect=AssertionError("lineage evidence must not be scanned"),
                ) as lineage_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_compact_key_evidence",
                    side_effect=AssertionError("compact evidence must not be scanned"),
                ) as compact_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_android_device_lab",
                    side_effect=AssertionError("device lab must not be scanned"),
                ) as android_scan,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(
                        [
                            *release_bundle_args(fixture),
                            "--verify-existing",
                            str(outside),
                        ]
                    )
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        load_json.assert_not_called()
        lineage_scan.assert_not_called()
        compact_scan.assert_not_called()
        android_scan.assert_not_called()
        self.assertIn("kagemusha_release_bundle_path_outside_root", rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_artifact_inventory_rejects_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            bundle_root = Path(temp) / "bundle"
            artifact_root = bundle_root / "artifacts" / "kagemusha"
            create_lineage_artifact_files(artifact_root)
            artifact_sha256 = {
                artifact: hashlib.sha256((artifact_root / artifact).read_bytes()).hexdigest()
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            artifact_size_bytes = {
                artifact: (artifact_root / artifact).stat().st_size
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            artifact_sha256["lineage-init-len128.pk"] = "0" * 64

            entries, blockers = release_bundle._artifact_inventory_entries(
                artifact_root,
                bundle_root,
                artifact_names=readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS,
                artifact_sha256=artifact_sha256,
                artifact_size_bytes=artifact_size_bytes,
                label_prefix="Reserved-lineage proof evidence",
                code_prefix="kagemusha_release_lineage_artifact",
            )

        self.assertNotIn("lineage-init-len128.pk", entries)
        self.assertIn(
            "kagemusha_release_lineage_artifact_digest_drift",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "kagemusha_release_lineage_artifact_inventory",
            {item["code"] for item in blockers},
        )

    def test_kagemusha_release_bundle_artifact_inventory_rejects_size_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            bundle_root = Path(temp) / "bundle"
            artifact_root = bundle_root / "artifacts" / "kagemusha"
            create_lineage_artifact_files(artifact_root)
            artifact_sha256 = {
                artifact: hashlib.sha256((artifact_root / artifact).read_bytes()).hexdigest()
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            artifact_size_bytes = {
                artifact: (artifact_root / artifact).stat().st_size
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            artifact_size_bytes["lineage-init-len128.pk"] += 1

            entries, blockers = release_bundle._artifact_inventory_entries(
                artifact_root,
                bundle_root,
                artifact_names=readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS,
                artifact_sha256=artifact_sha256,
                artifact_size_bytes=artifact_size_bytes,
                label_prefix="Reserved-lineage proof evidence",
                code_prefix="kagemusha_release_lineage_artifact",
            )

        self.assertNotIn("lineage-init-len128.pk", entries)
        self.assertIn(
            "kagemusha_release_lineage_artifact_size_drift",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "kagemusha_release_lineage_artifact_inventory",
            {item["code"] for item in blockers},
        )

    def test_kagemusha_release_bundle_artifact_inventory_rejects_outside_bundle_root(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bundle_root = root / "bundle"
            artifact_root = root / "outside-artifacts"
            bundle_root.mkdir()
            create_lineage_artifact_files(artifact_root)
            artifact_sha256 = {
                artifact: hashlib.sha256((artifact_root / artifact).read_bytes()).hexdigest()
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }
            artifact_size_bytes = {
                artifact: (artifact_root / artifact).stat().st_size
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }

            entries, blockers = release_bundle._artifact_inventory_entries(
                artifact_root,
                bundle_root,
                artifact_names=readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS,
                artifact_sha256=artifact_sha256,
                artifact_size_bytes=artifact_size_bytes,
                label_prefix="Reserved-lineage proof evidence",
                code_prefix="kagemusha_release_lineage_artifact",
            )

        self.assertEqual(entries, {})
        self.assertIn(
            "kagemusha_release_bundle_path_outside_root",
            {item["code"] for item in blockers},
        )

    def test_kagemusha_release_bundle_rejects_blocked_summary(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["ready"] = False
            summary["status"] = "blocked"
            summary["blockers"] = [
                {"code": "fixture_blocker", "message": "fixture blocked"}
            ]
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_not_ready", stderr.getvalue())
        self.assertIn(
            "kagemusha_release_summary_blockers_present",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_output_overwriting_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            original_summary = summary_path.read_text(encoding="utf-8")
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(
                    release_bundle_args(fixture, out=summary_path)
                )
            rendered = stderr.getvalue()
            final_summary = summary_path.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertEqual(final_summary, original_summary)
        self.assertIn("kagemusha_release_bundle_out_invalid", rendered)
        self.assertIn("--out must not overwrite bundled evidence input", rendered)

    def test_write_release_bundle_preserves_existing_output_on_replace_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            bundle_root = Path(temp) / "bundle"
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"
            out.parent.mkdir(parents=True)
            out.write_text("stale manifest\n", encoding="utf-8")

            with mock.patch.object(
                release_bundle.os,
                "replace",
                side_effect=OSError("replace failed"),
            ):
                errors = release_bundle.write_release_bundle(
                    out,
                    {
                        "schema": release_bundle.RELEASE_BUNDLE_SCHEMA,
                        "generated_at_utc": readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "ready": True,
                        "evidence": {},
                        "blockers": [],
                    },
                    bundle_root,
                )

            temp_outputs = list(out.parent.glob(f".{out.name}.*.tmp"))
            final_text = out.read_text(encoding="utf-8")

        self.assertEqual(
            [error["code"] for error in errors],
            ["kagemusha_release_bundle_out_invalid"],
        )
        self.assertEqual(final_text, "stale manifest\n")
        self.assertEqual(temp_outputs, [])

    def test_write_release_bundle_rejects_readback_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            bundle_root = Path(temp) / "bundle"
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"
            bundle = {
                "schema": release_bundle.RELEASE_BUNDLE_SCHEMA,
                "generated_at_utc": readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                "ready": True,
                "evidence": {},
                "blockers": [],
            }
            original_read_text = Path.read_text

            def corrupt_read_text(
                path: Path,
                *args: object,
                **kwargs: object,
            ) -> str:
                if path == out:
                    return "corrupted manifest\n"
                return original_read_text(path, *args, **kwargs)

            with mock.patch.object(Path, "read_text", corrupt_read_text):
                errors = release_bundle.write_release_bundle(out, bundle, bundle_root)

            written = out.read_text(encoding="utf-8")

        self.assertEqual(
            [error["code"] for error in errors],
            ["kagemusha_release_bundle_out_invalid"],
        )
        self.assertIn("readback did not match", errors[0]["message"])
        self.assertEqual(written, json.dumps(bundle, indent=2, sort_keys=True) + "\n")

    def test_kagemusha_release_bundle_rejects_summary_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["compact_key_evidence"]["artifact_sha256"][
                "recursive-compact-len4.pk"
            ] = "0" * 64
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_drift", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_compact_generator_log_inventory_digest_drift(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            original_entry_with_size = release_bundle._evidence_entry_with_size

            def forged_entry_with_size(path: Path, *args: object, **kwargs: object):
                entry, blockers = original_entry_with_size(path, *args, **kwargs)
                if (
                    path.name == readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
                    and entry is not None
                ):
                    entry = {**entry, "sha256": "0" * 64}
                return entry, blockers

            stderr = io.StringIO()
            with mock.patch.object(
                release_bundle,
                "_evidence_entry_with_size",
                forged_entry_with_size,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_compact_generator_log_digest_drift",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_lineage_size_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["lineage_proof_evidence"]["artifact_size_bytes"][
                "lineage-init-len128.pk"
            ] += 1
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_drift", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_android_summary_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            device_lab_root = fixture["device_lab_root"]
            signer = fixture["signer"]
            assert isinstance(device_lab_root, Path)
            assert isinstance(signer, dict)
            resign_slot_evidence_with_timestamp(
                device_lab_root / "slot-0",
                signer,
                "2026-06-06T00:00:01Z",
            )
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_drift", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_abi6_summary_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["abi6_reserved_lineage"]["operation_count"] = 999
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_drift", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_abi7_summary_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["abi7_recursive_compact"]["circuit_id"] = "forged-circuit"
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_drift", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_lineage_tooling_summary_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["lineage_key_release_tooling"]["checked_files"] = []
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_drift", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_wrong_repo_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            fake_repo = Path(temp) / "fake-repo"
            fake_repo.mkdir()
            args = release_bundle_args(fixture)
            args[args.index("--repo-root") + 1] = str(fake_repo)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)

        self.assertEqual(status, 1)
        self.assertIn("abi6", stderr.getvalue())
        self.assertIn("abi7", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_unexpected_summary_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["production_ready_claim"] = "skip remaining release evidence"
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_unexpected_field",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_unexpected_summary_section_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["lineage_proof_evidence"]["production_ready_claim"] = (
                "operator-approved without release packet evidence"
            )
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_unexpected_section_field",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_unexpected_android_signed_evidence_summary_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            slot = next(iter(summary["android_device_lab"]["signed_evidence"]))
            summary["android_device_lab"]["signed_evidence"][slot][
                "production_ready_claim"
            ] = "operator override"
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_android_signed_evidence_unexpected_field",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_missing_android_signed_evidence_summary_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            slot = next(iter(summary["android_device_lab"]["signed_evidence"]))
            del summary["android_device_lab"]["signed_evidence"][slot][
                "d2d_payment_transcript_sha256"
            ]
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_android_signed_evidence_missing_field",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_nonobject_android_signed_evidence_summary_entry(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            slot = next(iter(summary["android_device_lab"]["signed_evidence"]))
            summary["android_device_lab"]["signed_evidence"][slot] = "ready"
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_android_signed_evidence_shape",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_unsafe_android_signed_evidence_summary_slot_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            slot, entry = next(
                iter(summary["android_device_lab"]["signed_evidence"].items())
            )
            del summary["android_device_lab"]["signed_evidence"][slot]
            summary["android_device_lab"]["signed_evidence"][
                "token=supersecret"
            ] = entry
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_android_signed_evidence_slot",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_rejects_malformed_android_signed_evidence_summary_sha256(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            slot = next(iter(summary["android_device_lab"]["signed_evidence"]))
            summary["android_device_lab"]["signed_evidence"][slot][
                "wallet_integrity_transcript_sha256"
            ] = "A" * 64
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_android_signed_evidence_sha256",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_unsafe_android_signed_evidence_summary_path_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            slot = next(iter(summary["android_device_lab"]["signed_evidence"]))
            summary["android_device_lab"]["signed_evidence"][slot][
                "d2d_payment_transcript_path"
            ] = "../token=supersecret.json"
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_android_signed_evidence_path",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_rejects_noncanonical_android_signed_evidence_summary_timestamp(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            slot = next(iter(summary["android_device_lab"]["signed_evidence"]))
            summary["android_device_lab"]["signed_evidence"][slot][
                "signed_at_utc"
            ] = "2026-06-06T00:00:00+00:00"
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_android_signed_evidence_timestamp",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_ready_summary_section_blockers(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["abi7_recursive_compact"]["blockers"] = [
                {"code": "hidden_blocker", "message": "must not be hidden"}
            ]
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_section_blockers_present",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_secret_summary_material_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["lineage_proof_evidence"]["local_debug_path"] = (
                "token=supersecret"
            )
            write_json(summary_path, summary)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_summary_secret_material",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_rejects_duplicate_summary_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary_text = summary_path.read_text(encoding="utf-8")
            summary_path.write_text(
                summary_text.replace(
                    '"schema": "iroha.kagemusha.production_readiness.v1"',
                    (
                        '"schema": "iroha.kagemusha.production_readiness.v1", '
                        '"schema": "shadow"'
                    ),
                    1,
                ),
                encoding="utf-8",
            )
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_invalid_json", stderr.getvalue())
        self.assertIn("duplicate JSON object key schema", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_nonfinite_summary_json_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            summary["generated_at"] = float("nan")
            summary_path.write_text(
                json.dumps(summary, indent=2, sort_keys=True, allow_nan=True) + "\n",
                encoding="utf-8",
            )
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_invalid_json", stderr.getvalue())
        self.assertIn("non-finite constant NaN is not allowed", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_evidence_outside_bundle_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            outside = Path(temp) / "outside-summary.json"
            summary_path = fixture["summary_path"]
            assert isinstance(summary_path, Path)
            outside.write_text(summary_path.read_text(encoding="utf-8"), encoding="utf-8")
            args = release_bundle_args(fixture)
            args[args.index("--readiness-summary") + 1] = str(outside)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_bundle_path_outside_root",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_outside_summary_before_json_load(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            outside = Path(temp) / "outside-summary.json"
            outside.write_text(
                json.dumps({"leak": "token=supersecret"}) + "\n",
                encoding="utf-8",
            )
            args = release_bundle_args(fixture)
            args[args.index("--readiness-summary") + 1] = str(outside)
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    release_bundle,
                    "_load_local_json",
                    side_effect=AssertionError("outside summary must not be loaded"),
                ) as load_json,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_lineage_proof_evidence",
                    side_effect=AssertionError("lineage evidence must not be scanned"),
                ) as lineage_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_compact_key_evidence",
                    side_effect=AssertionError("compact evidence must not be scanned"),
                ) as compact_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_android_device_lab",
                    side_effect=AssertionError("device lab must not be scanned"),
                ) as android_scan,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        load_json.assert_not_called()
        lineage_scan.assert_not_called()
        compact_scan.assert_not_called()
        android_scan.assert_not_called()
        self.assertIn("kagemusha_release_bundle_path_outside_root", rendered)
        self.assertNotIn("kagemusha_release_summary_secret_material", rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_verify_existing_rejects_bundle_root_symlink_before_manifest_load(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            linked_root = Path(temp) / "linked-bundle"
            slot_helpers.create_dir_symlink(self, linked_root, bundle_root)
            args = release_bundle_args(fixture)
            args[args.index("--bundle-root") + 1] = str(linked_root)
            args.extend(["--verify-existing", "dist/kagemusha-production-release-bundle.json"])
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    release_bundle,
                    "_load_local_json",
                    side_effect=AssertionError("release manifest must not be loaded"),
                ) as load_json,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_lineage_proof_evidence",
                    side_effect=AssertionError("lineage evidence must not be scanned"),
                ) as lineage_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_compact_key_evidence",
                    side_effect=AssertionError("compact evidence must not be scanned"),
                ) as compact_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_android_device_lab",
                    side_effect=AssertionError("device lab must not be scanned"),
                ) as android_scan,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        load_json.assert_not_called()
        lineage_scan.assert_not_called()
        compact_scan.assert_not_called()
        android_scan.assert_not_called()
        self.assertIn("kagemusha_release_bundle_root_invalid", rendered)
        self.assertIn("--bundle-root must not be a symlink", rendered)

    def test_kagemusha_release_bundle_rejects_outside_evidence_before_scanners(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            outside = Path(temp) / "outside"
            outside.mkdir()
            outside_lineage = outside / "lineage-proof-evidence.json"
            outside_compact = outside / "recursive-compact-key-evidence.json"
            outside_device_lab = outside / "device_lab"
            outside_lineage.write_text('{"leak": "token=supersecret"}\n', encoding="utf-8")
            outside_compact.write_text('{"leak": "token=supersecret"}\n', encoding="utf-8")
            outside_device_lab.mkdir()
            args = release_bundle_args(fixture)
            args[args.index("--lineage-proof-evidence") + 1] = str(outside_lineage)
            args[args.index("--compact-key-evidence") + 1] = str(outside_compact)
            args[args.index("--device-lab-root") + 1] = str(outside_device_lab)
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    release_bundle.readiness,
                    "check_lineage_proof_evidence",
                    side_effect=AssertionError("outside lineage evidence must not be scanned"),
                ) as lineage_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_compact_key_evidence",
                    side_effect=AssertionError("outside compact evidence must not be scanned"),
                ) as compact_scan,
                mock.patch.object(
                    release_bundle.readiness,
                    "check_android_device_lab",
                    side_effect=AssertionError("outside device lab must not be scanned"),
                ) as android_scan,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        lineage_scan.assert_not_called()
        compact_scan.assert_not_called()
        android_scan.assert_not_called()
        self.assertIn("kagemusha_release_bundle_path_outside_root", rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_rejects_android_evidence_outside_bundle_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            device_lab_root = fixture["device_lab_root"]
            assert isinstance(device_lab_root, Path)
            outside_lab_root = Path(temp) / "outside-device-lab"
            shutil.copytree(device_lab_root, outside_lab_root)
            args = release_bundle_args(fixture)
            args[args.index("--device-lab-root") + 1] = str(outside_lab_root)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_bundle_path_outside_root",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_forged_android_slot_escape(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary = fixture["summary"]
            assert isinstance(summary, dict)
            android = json.loads(json.dumps(summary["android_device_lab"]))
            android["slots"][0]["slot"] = "../outside"
            stderr = io.StringIO()

            with mock.patch.object(
                release_bundle.readiness,
                "check_android_device_lab",
                return_value=android,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_android_signed_evidence_slot",
            stderr.getvalue(),
        )

    def test_kagemusha_release_bundle_rejects_secret_android_slot_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            summary = fixture["summary"]
            assert isinstance(summary, dict)
            android = json.loads(json.dumps(summary["android_device_lab"]))
            android["slots"][0]["slot"] = "token=supersecret"
            stderr = io.StringIO()

            with mock.patch.object(
                release_bundle.readiness,
                "check_android_device_lab",
                return_value=android,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture))
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "kagemusha_release_android_signed_evidence_slot",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_rejects_output_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"
            target = bundle_root / "dist" / "outside.json"
            write_json(target, {"stale": True})
            write_json(out, {"stale": True})
            slot_helpers.replace_with_symlink(self, out, target)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_bundle_out_invalid", stderr.getvalue())
        self.assertIn("--out must not be a symlink", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_output_hardlink(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out = bundle_root / "dist" / "kagemusha-production-release-bundle.json"
            target = bundle_root / "dist" / "outside.json"
            write_json(target, {"external": True})
            write_json(out, {"stale": True})
            slot_helpers.replace_with_hardlink(self, out, target)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(release_bundle_args(fixture))
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertEqual(json.loads(target_text), {"external": True})
        self.assertIn("kagemusha_release_bundle_out_invalid", stderr.getvalue())
        self.assertIn("--out must not be hardlinked", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_output_parent_symlink_after_create(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            out_parent = bundle_root / "release-out"
            out = out_parent / "kagemusha-production-release-bundle.json"
            external_parent = Path(temp) / "external-release-out"
            external_parent.mkdir()
            original_mkdir = Path.mkdir

            def replacing_mkdir(
                path: Path,
                mode: int = 0o777,
                parents: bool = False,
                exist_ok: bool = False,
            ) -> None:
                if path == out_parent:
                    path.symlink_to(external_parent, target_is_directory=True)
                    return
                original_mkdir(path, mode=mode, parents=parents, exist_ok=exist_ok)

            stderr = io.StringIO()
            with mock.patch.object(Path, "mkdir", replacing_mkdir):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(release_bundle_args(fixture, out=out))

        self.assertEqual(status, 1)
        self.assertFalse((external_parent / out.name).exists())
        self.assertIn("kagemusha_release_bundle_out_invalid", stderr.getvalue())
        self.assertIn("--out parent directory must not be a symlink", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_bundle_root_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            linked_root = Path(temp) / "linked-bundle"
            slot_helpers.create_dir_symlink(self, linked_root, bundle_root)
            args = release_bundle_args(fixture)
            args[args.index("--bundle-root") + 1] = str(linked_root)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_bundle_root_invalid", stderr.getvalue())
        self.assertIn("--bundle-root must not be a symlink", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_bundle_root_symlink_ancestor_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            bundle_root = fixture["bundle_root"]
            assert isinstance(bundle_root, Path)
            real_parent = bundle_root.parent
            linked_parent = Path(temp) / "linked-parent"
            slot_helpers.create_dir_symlink(self, linked_parent, real_parent)
            linked_root = linked_parent / bundle_root.name
            args = release_bundle_args(fixture)
            args[args.index("--bundle-root") + 1] = str(linked_root)
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_bundle_root_invalid", rendered)
        self.assertIn(
            "--bundle-root ancestor directory must not be a symlink",
            rendered,
        )
        self.assertNotIn(str(linked_parent), rendered)

    def test_kagemusha_release_bundle_rejects_secret_summary_path_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            args = release_bundle_args(fixture)
            args[args.index("--readiness-summary") + 1] = "token=supersecret.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_release_summary_path_invalid", rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_rejects_secret_repo_root_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            args = release_bundle_args(fixture)
            args[args.index("--repo-root") + 1] = "token=supersecret-repo"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_repo_root_path_invalid", rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_release_bundle_rejects_missing_trusted_signer(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            args = release_bundle_args(fixture)
            signer_index = args.index("--trusted-signer-public-key")
            del args[signer_index : signer_index + 2]
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = release_bundle.main(args)

        self.assertEqual(status, 1)
        self.assertIn("android_trusted_signer_missing", stderr.getvalue())

    def test_kagemusha_release_bundle_rejects_secret_signer_path_before_load(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = create_ready_release_bundle_fixture(Path(temp))
            args = release_bundle_args(fixture)
            args[args.index("--trusted-signer-public-key") + 1] = (
                str(Path(temp) / "token=supersecret-public.pem")
            )
            stderr = io.StringIO()

            with mock.patch.object(
                release_bundle.device_lab,
                "load_trusted_signer_public_keys",
                side_effect=AssertionError("trusted signer loader should not run"),
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = release_bundle.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("android_trusted_signer_path_invalid", rendered)
        self.assertNotIn("token=supersecret", rendered)

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

    def test_missing_android_root_uses_lstat_before_exists_preflight(self) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"
                signer = slot_helpers.create_test_signer(Path(temp) / "keys")
                missing_root = Path(temp) / "missing-slots"

                def failing_exists(path: Path, *args, **kwargs):
                    if path == missing_root:
                        raise OSError("simulated device-lab root exists failure")
                    return original_exists(path, *args, **kwargs)

                path_type.exists = failing_exists

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
        finally:
            path_type.exists = original_exists

        self.assertEqual(status, 1)
        self.assertIn(
            "android_device_lab_root_missing",
            {item["code"] for item in summary["blockers"]},
        )

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

    def test_android_root_discovery_failure_blocks_rollup_without_traceback(self) -> None:
        original_iterdir = Path.iterdir

        def failing_iterdir(path: Path):
            if path == device_lab_root:
                raise OSError("simulated root discovery failure")
            return original_iterdir(path)

        try:
            with tempfile.TemporaryDirectory() as temp:
                temp_path = Path(temp)
                device_lab_root = temp_path / "device_lab"
                device_lab_root.mkdir()
                signer = slot_helpers.create_test_signer(temp_path / "keys")
                lineage_evidence = create_lineage_proof_evidence(temp_path / "lineage")
                trusted, errors = slot_helpers.device_lab.load_trusted_signer_public_keys(
                    [signer["public_key"]]
                )
                self.assertEqual(errors, [])

                Path.iterdir = failing_iterdir
                summary = readiness.build_summary(
                    repo_root=REPO_ROOT,
                    device_lab_root=device_lab_root,
                    lineage_proof_evidence_path=lineage_evidence,
                    trusted_signer_public_keys=trusted,
                )
                rendered = json.dumps(summary)
        finally:
            Path.iterdir = original_iterdir

        self.assertFalse(summary["ready"])
        self.assertEqual(summary["android_device_lab"]["slots"], [])
        self.assertIn(
            "android_device_lab_root_unreadable",
            {item["code"] for item in summary["blockers"]},
        )
        self.assertIn(
            "device-lab root could not be listed",
            {item["message"] for item in summary["blockers"]},
        )
        self.assertNotIn(str(device_lab_root), rendered)
        self.assertNotIn("Traceback", rendered)

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

    def test_signed_evidence_freshness_rejects_noncanonical_report_timestamp(self) -> None:
        blockers = readiness._check_android_signed_evidence_freshness(
            [
                {
                    "status": "ok",
                    "slot": "slot-0",
                    "kagemusha": {"signed_at_utc": "2026-06-06T00:00:00+00:00"},
                }
            ],
            None,
            None,
        )

        self.assertIn(
            "android_signed_evidence_timestamp_noncanonical",
            {item["code"] for item in blockers},
        )

    def test_signed_evidence_freshness_redacts_noncanonical_secret_timestamp(self) -> None:
        blockers = readiness._check_android_signed_evidence_freshness(
            [
                {
                    "status": "ok",
                    "slot": "slot-0",
                    "kagemusha": {"signed_at_utc": "token=secret-time"},
                }
            ],
            None,
            None,
        )
        rendered = json.dumps(blockers)

        self.assertIn("android_signed_evidence_timestamp_noncanonical", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=secret-time", rendered)

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

            def fake_slot_reports(
                *_args: object, **_kwargs: object
            ) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
                return (
                    [
                        {
                            "slot": "token=supersecret-slot",
                            "status": "error",
                            "errors": ["artifact path token=supersecret-artifact"],
                            "present": {},
                            "file_counts": {},
                            "kagemusha": {"required": True},
                        }
                    ],
                    [],
                )

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

    def test_abi6_manifest_rejects_non_utf8_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            manifest_path = repo / readiness.ABI6_MANIFEST_PATH
            manifest_path.parent.mkdir(parents=True, exist_ok=True)
            manifest_path.write_bytes(b"\xff\xfe\xfd")

            result = readiness.check_abi6_reserved_lineage(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi6_manifest_unreadable",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "ABI-6 manifest could not be read",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(manifest_path), rendered)

    def test_abi6_manifest_rejects_nonfinite_json_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            manifest_path = repo / readiness.ABI6_MANIFEST_PATH
            manifest_path.parent.mkdir(parents=True, exist_ok=True)
            manifest_path.write_text('{"schema": Infinity}\n', encoding="utf-8")

            result = readiness.check_abi6_reserved_lineage(repo)
            rendered = json.dumps(result["blockers"])

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi6_manifest_invalid_json",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn("non-finite constant Infinity is not allowed", rendered)
        self.assertNotIn(str(manifest_path), rendered)

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

    def test_release_local_json_validator_rejects_hardlink_metadata_failure_before_parse(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                manifest_path = Path(temp) / "repo" / readiness.ABI6_MANIFEST_PATH
                manifest_path.parent.mkdir(parents=True, exist_ok=True)
                manifest_path.write_text("{not-json", encoding="utf-8")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == manifest_path:
                        raise OSError("simulated release JSON hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                errors = readiness.validate_release_local_json_file(
                    manifest_path,
                    "ABI-6 manifest",
                )
                rendered = json.dumps(errors)
        finally:
            path_type.stat = original_stat

        self.assertEqual(errors, ["ABI-6 manifest hardlink metadata could not be read"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn(str(manifest_path), rendered)

    def test_release_local_json_validator_rejects_file_metadata_failure_before_parse(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                manifest_path = Path(temp) / "repo" / readiness.ABI6_MANIFEST_PATH
                manifest_path.parent.mkdir(parents=True, exist_ok=True)
                manifest_path.write_text("{not-json", encoding="utf-8")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == manifest_path:
                        raise OSError("simulated release JSON file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = readiness.validate_release_local_json_file(
                    manifest_path,
                    "ABI-6 manifest",
                )
                rendered = json.dumps(errors)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["ABI-6 manifest file metadata could not be read"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn(str(manifest_path), rendered)

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

    def test_repo_source_marker_text_rejects_symlink_directly_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            marker_path = root / "repo" / "crates/iroha_core/src/zk.rs"
            marker_path.parent.mkdir(parents=True)
            marker_path.write_text("placeholder\n", encoding="utf-8")
            external_marker = root / "external-core-marker.rs"
            external_marker.write_text(
                "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE\n",
                encoding="utf-8",
            )
            slot_helpers.replace_with_symlink(self, marker_path, external_marker)

            text, errors = readiness._repo_source_marker_text(
                marker_path,
                "ABI-7 core marker file",
                "ABI-7 source marker file could not be read",
            )

        self.assertIsNone(text)
        self.assertEqual(errors, ["ABI-7 core marker file must not be a symlink"])

    def test_repo_source_marker_text_rejects_hardlink_directly_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            marker_path = root / "repo" / "crates/iroha_cli/src/zk.rs"
            marker_path.parent.mkdir(parents=True)
            marker_path.write_text("placeholder\n", encoding="utf-8")
            external_marker = root / "external-lineage-cli.rs"
            external_marker.write_text(
                "KagemushaCommand::LineageKeyArtifacts\n",
                encoding="utf-8",
            )
            slot_helpers.replace_with_hardlink(self, marker_path, external_marker)

            text, errors = readiness._repo_source_marker_text(
                marker_path,
                "Reserved-lineage release-tooling marker file",
                "Reserved-lineage release-tooling file could not be read",
            )

        self.assertIsNone(text)
        self.assertEqual(
            errors,
            ["Reserved-lineage release-tooling marker file must not be hardlinked"],
        )

    def test_repo_source_marker_text_rejects_hardlink_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                marker_path = Path(temp) / "repo" / "crates/iroha_cli/src/zk.rs"
                marker_path.parent.mkdir(parents=True)
                marker_path.write_text(
                    "KagemushaCommand::LineageKeyArtifacts\n",
                    encoding="utf-8",
                )

                def failing_stat(path: Path, *args, **kwargs):
                    if path == marker_path:
                        raise OSError("simulated source marker hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                text, errors = readiness._repo_source_marker_text(
                    marker_path,
                    "Reserved-lineage release-tooling marker file",
                    "Reserved-lineage release-tooling file could not be read",
                )
                rendered = json.dumps(errors)
        finally:
            path_type.stat = original_stat

        self.assertIsNone(text)
        self.assertEqual(
            errors,
            [
                "Reserved-lineage release-tooling marker file "
                "hardlink metadata could not be read"
            ],
        )
        self.assertNotIn(str(marker_path), rendered)

    def test_repo_source_marker_text_rejects_file_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                marker_path = Path(temp) / "repo" / "crates/iroha_cli/src/zk.rs"
                marker_path.parent.mkdir(parents=True)
                marker_path.write_text(
                    "KagemushaCommand::LineageKeyArtifacts\n",
                    encoding="utf-8",
                )

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == marker_path:
                        raise OSError("simulated source marker file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                text, errors = readiness._repo_source_marker_text(
                    marker_path,
                    "Reserved-lineage release-tooling marker file",
                    "Reserved-lineage release-tooling file could not be read",
                )
                rendered = json.dumps(errors)
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(text)
        self.assertEqual(
            errors,
            [
                "Reserved-lineage release-tooling marker file "
                "file metadata could not be read"
            ],
        )
        self.assertNotIn(str(marker_path), rendered)

    def test_repo_source_marker_text_rejects_non_utf8_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            marker_path = Path(temp) / "repo" / "crates/iroha_core/src/zk.rs"
            marker_path.parent.mkdir(parents=True)
            marker_path.write_bytes(b"\xff\xfe KAGEMUSHA marker")

            text, errors = readiness._repo_source_marker_text(
                marker_path,
                "ABI-7 core marker file",
                "ABI-7 source marker file could not be read",
            )

        self.assertIsNone(text)
        self.assertEqual(errors, ["ABI-7 source marker file could not be read"])

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

    def test_abi7_fail_closed_rejects_non_utf8_source_marker_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)
            marker_path = repo / "crates/iroha_core/src/zk.rs"
            marker_path.write_bytes(b"\xff\xfe invalid source marker")

            result = readiness.check_abi7_fail_closed(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi7_source_marker_file_unreadable",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "ABI-7 source marker file could not be read",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn("UnicodeDecodeError", rendered)

    def test_rust_function_body_ignores_braces_inside_strings_and_comments(self) -> None:
        source = "\n".join(
            (
                "fn target() -> bool {",
                '    let _quoted = "{ not a body brace }";',
                '    let _raw = r#"{ not a body brace }"#;',
                '    let _raw_bytes = br##"}"##;',
                "    let _lifetime: &'static str = \"ready\";",
                "    // }",
                "    /* { */",
                "    false",
                "}",
                "fn other() -> bool { true }",
            )
        )

        body = readiness._rust_function_body(source, "fn target(")

        self.assertIsNotNone(body)
        assert body is not None
        self.assertIn("false", body)
        self.assertNotIn("fn other", body)

    def test_abi7_fail_closed_accepts_strict_function_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)

            result = readiness.check_abi7_fail_closed(repo)

        self.assertTrue(result["ok"])
        self.assertEqual("package_aware_multi_hop_composed", result["state"])
        self.assertEqual([], result["blockers"])

    def test_abi7_fail_closed_rejects_dispatch_without_checked_opening_len_conversion(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)
            core_path = repo / "crates/iroha_core/src/zk.rs"
            core_text = core_path.read_text(encoding="utf-8")
            core_path.write_text(
                core_text.replace(
                    "match usize::try_from(preflight.opening_len)",
                    "match preflight.opening_len",
                ),
                encoding="utf-8",
            )

            result = readiness.check_abi7_fail_closed(repo)

        self.assertFalse(result["ok"])
        self.assertIn(
            {
                "code": "abi7_fail_closed_contract_missing",
                "function": "fn prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch(",
                "marker": "match usize::try_from(preflight.opening_len)",
                "message": "ABI-7 recursive compact launch-boundary function contract is missing",
            },
            result["blockers"],
        )

    def test_abi7_fail_closed_rejects_one_hop_runtime_keygen_fallback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)
            core_path = repo / "crates/iroha_core/src/zk.rs"
            core_path.write_text(
                core_path.read_text(encoding="utf-8").replace(
                    '    "missing compact one-hop proving key archive";\n',
                    "",
                ),
                encoding="utf-8",
            )

            result = readiness.check_abi7_fail_closed(repo)

        self.assertFalse(result["ok"])
        self.assertIn(
            {
                "code": "abi7_fail_closed_contract_missing",
                "function": "fn prove_kagemusha_recursive_compact_payment_token_one_hop_from_record_bundle_and_pallas_open_envelopes(",
                "marker": "missing compact one-hop proving key archive",
                "message": "ABI-7 recursive compact launch-boundary function contract is missing",
            },
            result["blockers"],
        )

    def test_abi7_fail_closed_rejects_append_runtime_keygen_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)
            core_path = repo / "crates/iroha_core/src/zk.rs"
            core_path.write_text(
                core_path.read_text(encoding="utf-8").replace(
                    '        "missing compact append proving key archive";\n',
                    "",
                ),
                encoding="utf-8",
            )

            result = readiness.check_abi7_fail_closed(repo)

        self.assertFalse(result["ok"])
        self.assertIn(
            {
                "code": "abi7_fail_closed_contract_missing",
                "function": "fn prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
                "marker": "missing compact append proving key archive",
                "message": "ABI-7 recursive compact launch-boundary function contract is missing",
            },
            result["blockers"],
        )

    def test_abi7_fail_closed_rejects_preverify_contract_without_unavailable_error(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)
            core_path = repo / "crates/iroha_core/src/zk.rs"
            core_text = core_path.read_text(encoding="utf-8")
            core_path.write_text(
                core_text.replace(
                    "\n".join(
                        (
                            "pub fn preverify_kagemusha_recursive_compact_payment_token(",
                            ") -> Result<(), String> {",
                            "    preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id();",
                            "    KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
                            "}",
                        )
                    ),
                    "\n".join(
                        (
                            "pub fn preverify_kagemusha_recursive_compact_payment_token(",
                            ") -> Result<(), String> {",
                            "    preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id();",
                            "    KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID_V1",
                            "}",
                        )
                    ),
                ),
                encoding="utf-8",
            )

            result = readiness.check_abi7_fail_closed(repo)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi7_fail_closed_contract_missing",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "pub fn preverify_kagemusha_recursive_compact_payment_token(",
            {item.get("function") for item in result["blockers"]},
        )

    def test_abi7_fail_closed_rejects_verify_contract_without_backend_call(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)
            core_path = repo / "crates/iroha_core/src/zk.rs"
            core_text = core_path.read_text(encoding="utf-8")
            core_path.write_text(
                core_text.replace(
                    "\n".join(
                        (
                            "pub fn verify_kagemusha_recursive_compact_payment_token(",
                            ") -> bool {",
                            "    preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id();",
                            "    verify_backend();",
                            "}",
                        )
                    ),
                    "\n".join(
                        (
                            "pub fn verify_kagemusha_recursive_compact_payment_token(",
                            ") -> bool {",
                            "    preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id();",
                            "    soft_invalid_without_backend();",
                            "}",
                        )
                    ),
                ),
                encoding="utf-8",
            )

            result = readiness.check_abi7_fail_closed(repo)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi7_fail_closed_contract_missing",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "pub fn verify_kagemusha_recursive_compact_payment_token(",
            {item.get("function") for item in result["blockers"]},
        )

    def test_abi7_fail_closed_rejects_bridge_contract_without_unavailable_mapping(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_abi7_fail_closed_marker_files(repo)
            bridge_path = repo / "crates/connect_norito_bridge/src/lib.rs"
            bridge_path.write_text(
                bridge_path.read_text(encoding="utf-8").replace(
                    "BridgeError::KagemushaRecursiveCompactUnavailable",
                    "BridgeError::KagemushaProve",
                ),
                encoding="utf-8",
            )

            result = readiness.check_abi7_fail_closed(repo)

        self.assertFalse(result["ok"])
        self.assertIn(
            "abi7_bridge_unavailable_contract_missing",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "native bridge must map ABI-7 recursive compact unavailable separately",
            {item["message"] for item in result["blockers"]},
        )

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

    def test_lineage_key_release_tooling_rejects_non_utf8_marker_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp) / "repo"
            write_lineage_key_release_tooling_marker_files(repo)
            marker_path = repo / "crates/iroha_cli/src/zk.rs"
            marker_path.write_bytes(b"\xff\xfe invalid release tooling marker")

            result = readiness.check_lineage_key_release_tooling(repo)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_key_release_file_unreadable",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "Reserved-lineage release-tooling file could not be read",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn("UnicodeDecodeError", rendered)

    def test_missing_compact_key_evidence_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            missing_path = Path(temp) / readiness.COMPACT_KEY_EVIDENCE_FILENAME
            result = readiness.check_compact_key_evidence(missing_path)

        self.assertFalse(result["ok"])
        self.assertEqual(
            readiness.COMPACT_KEY_EVIDENCE_SUMMARY_LABEL,
            result["path"],
        )
        self.assertNotIn(str(missing_path.parent), json.dumps(result))
        self.assertIn(
            "compact_key_evidence_missing",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_noncanonical_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            noncanonical_path = evidence_path.with_name("recursive-compact-key-copy.json")
            evidence_path.rename(noncanonical_path)

            result = readiness.check_compact_key_evidence(noncanonical_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_filename",
            {item["code"] for item in result["blockers"]},
        )
        self.assertNotIn(str(noncanonical_path.parent), json.dumps(result))

    def test_compact_key_evidence_rejects_symlinked_evidence_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_compact_key_evidence(root / "real-compact")
            evidence_link = root / readiness.COMPACT_KEY_EVIDENCE_FILENAME
            try:
                evidence_link.symlink_to(evidence_path)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            result = readiness.check_compact_key_evidence(evidence_link)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "ABI-7 recursive compact key evidence file must not be a symlink",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(evidence_path.parent), rendered)

    def test_compact_key_evidence_rejects_duplicate_json_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence_path.write_text(
                '{"schema":"x","schema":"y"}\n',
                encoding="utf-8",
            )

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_invalid_json",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_secret_duplicate_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence_path.write_text(
                '{"token=supersecret":"x","token=supersecret":"y"}\n',
                encoding="utf-8",
            )

            result = readiness.check_compact_key_evidence(evidence_path)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn("compact_key_evidence_invalid_json", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_compact_key_evidence_rejects_nonfinite_json_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence_path.write_text('{"schema": NaN}\n', encoding="utf-8")

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        rendered = json.dumps(result["blockers"])
        self.assertIn("compact_key_evidence_invalid_json", rendered)
        self.assertIn("non-finite constant NaN is not allowed", rendered)

    def test_stale_compact_key_evidence_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["generated_at_utc"] = "2026-06-05T23:59:59Z"
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(
                evidence_path,
                min_generated_at=readiness.parse_utc_timestamp(
                    readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                    "test cutoff",
                )[0],
            )

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_stale",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_noncanonical_timestamp(self) -> None:
        for generated_at in ("2026-06-06 00:00:00Z", "2026-06-06T00:00:00+00:00"):
            with tempfile.TemporaryDirectory() as temp:
                evidence_path = create_compact_key_evidence(Path(temp) / "compact")
                evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
                evidence["generated_at_utc"] = generated_at
                write_json(evidence_path, evidence)

                result = readiness.check_compact_key_evidence(evidence_path)

            self.assertFalse(result["ok"])
            self.assertIn(
                "compact_key_evidence_timestamp_noncanonical",
                {item["code"] for item in result["blockers"]},
            )

    def test_future_compact_key_evidence_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["generated_at_utc"] = "2026-06-06T00:05:01Z"
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(
                evidence_path,
                max_generated_at=readiness.parse_utc_timestamp(
                    "2026-06-06T00:05:00Z",
                    "test max timestamp",
                )[0],
            )

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_future_dated",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_drift_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["opening_len"] = 8
            evidence["ipa_k"] = 9
            evidence["record_version"] = 2
            evidence["verifier_backend"] = "mock"
            evidence["circuit_id"] = "kagemusha-recursive-compact-v2"
            evidence["record_namespace"] = "test"
            evidence["command"] = "python3 fake_runner.py recursive-compact-key-artifacts"
            evidence["artifacts"]["recursive-compact-len4.pk"] = "0" * 64
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn("compact_key_evidence_opening_len", codes)
        self.assertIn("compact_key_evidence_ipa_k", codes)
        self.assertIn("compact_key_evidence_record_version", codes)
        self.assertIn("compact_key_evidence_verifier_backend", codes)
        self.assertIn("compact_key_evidence_circuit_id", codes)
        self.assertIn("compact_key_evidence_record_namespace", codes)
        self.assertIn("compact_key_evidence_command", codes)
        self.assertIn("compact_key_evidence_artifact_digest", codes)

    def test_compact_key_evidence_rejects_float_scalar_claims(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["opening_len"] = 4.0
            evidence["ipa_k"] = 8.0
            evidence["record_version"] = 1.0
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn("compact_key_evidence_opening_len", codes)
        self.assertIn("compact_key_evidence_ipa_k", codes)
        self.assertIn("compact_key_evidence_record_version", codes)

    def test_compact_key_evidence_rejects_missing_artifact_size_map(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence.pop("artifact_size_bytes")
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn("compact_key_evidence_artifact_sizes", codes)
        self.assertIn("compact_key_evidence_artifact_size", codes)

    def test_compact_key_evidence_rejects_artifact_size_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifact_size_bytes"]["recursive-compact-len4.pk"] += 1
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "compact_key_evidence_artifact_size",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "recursive-compact-len4.pk",
            {item.get("artifact") for item in blockers},
        )
        self.assertNotIn(
            "recursive-compact-len4.pk",
            result["artifact_size_bytes"],
        )

    def test_compact_key_evidence_rejects_missing_generator_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            (evidence_path.parent / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME).unlink()

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_generator_log_file_shape",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIsNone(result["generator_log_sha256"])

    def test_compact_key_evidence_rejects_generator_log_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            log_path = evidence_path.parent / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
            log_path.write_text(
                log_path.read_text(encoding="utf-8").replace("pk=70", "pk=71"),
                encoding="utf-8",
            )

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn("compact_key_evidence_generator_log_digest", codes)
        self.assertIn("compact_key_evidence_generator_log_artifact_size", codes)

    def test_compact_key_evidence_rejects_generator_log_extra_lines(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            log_path = evidence_path.parent / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
            log_path.write_text(
                log_path.read_text(encoding="utf-8") + "test recursive compact key ok\n",
                encoding="utf-8",
            )
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["generator_log_sha256"] = hashlib.sha256(
                log_path.read_bytes()
            ).hexdigest()
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_generator_log_format",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_empty_generator_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            log_path = evidence_path.parent / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
            log_path.write_text("", encoding="utf-8")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["generator_log_sha256"] = hashlib.sha256(b"").hexdigest()
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_generator_log_format",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_noncanonical_generator_log_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["generator_log_path"] = "copy.log"
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_generator_log_path",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_secret_size_field_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifact_size_bytes"]["token=supersecret"] = 123
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)
            rendered = json.dumps(result["blockers"])

        self.assertFalse(result["ok"])
        self.assertIn("compact_key_evidence_artifact_sizes_unexpected_field", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_compact_key_evidence_rejects_appended_shell_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["command"] += " ; echo ok"
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "--command must exactly match the production ABI-7 recursive compact keygen command",
            {item.get("issue") for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_shell_equivalent_noncanonical_command(self) -> None:
        canonical = readiness.expected_compact_key_command()
        for command in (
            canonical.replace("iroha app", "'iroha' app", 1),
            canonical.replace(" --vk-out ", "\n--vk-out ", 1),
            f" {canonical} ",
        ):
            with tempfile.TemporaryDirectory() as temp:
                evidence_path = create_compact_key_evidence(Path(temp) / "compact")
                evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
                evidence["command"] = command
                write_json(evidence_path, evidence)

                result = readiness.check_compact_key_evidence(evidence_path)

            self.assertFalse(result["ok"])
            self.assertIn(
                "--command must exactly match the canonical ABI-7 recursive compact keygen command string",
                {item.get("issue") for item in result["blockers"]},
            )

    def test_compact_key_evidence_rejects_secret_looking_command_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["command"] += " token=supersecret"
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)
            rendered = json.dumps(result["blockers"])

        self.assertFalse(result["ok"])
        self.assertIn(
            "--command must not contain secret-looking material",
            {item.get("issue") for item in result["blockers"]},
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_compact_key_evidence_rejects_unexpected_secret_field_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["token=supersecret"] = "do not print"
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)
            rendered = json.dumps(result["blockers"])

        self.assertFalse(result["ok"])
        self.assertIn("compact_key_evidence_unexpected_field", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_compact_key_evidence_redacts_secret_required_scalars_in_full_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["schema"] = "token=secret-schema"
            evidence["generated_at_utc"] = "token=secret-time"
            evidence["verifier_backend"] = "token=secret-backend"
            evidence["circuit_id"] = "token=secret-circuit"
            evidence["record_namespace"] = "token=secret-namespace"
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        rendered = json.dumps(result)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=secret", rendered)

    def test_compact_key_evidence_rejects_missing_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            (evidence_path.parent / "recursive-compact-len4.pk").unlink()

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "compact_key_evidence_artifact_missing",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "recursive-compact-len4.pk",
            {item.get("artifact") for item in blockers},
        )

    def test_compact_key_evidence_rejects_symlinked_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_compact_key_evidence(root / "compact")
            artifact = evidence_path.parent / "recursive-compact-len4.vk"
            external = root / "external-recursive-compact-len4.vk"
            external.write_bytes(artifact.read_bytes())
            slot_helpers.replace_with_symlink(self, artifact, external)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "compact_key_evidence_artifact_file_shape",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "ABI-7 recursive compact key evidence artifact file must not be a symlink",
            {item["message"] for item in blockers},
        )
        self.assertNotIn(str(evidence_path.parent), json.dumps(blockers))

    def test_compact_key_evidence_rejects_hardlinked_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            evidence_path = create_compact_key_evidence(root / "compact")
            artifact = evidence_path.parent / "recursive-compact-len4.vk"
            external = root / "external-recursive-compact-len4.vk"
            external.write_bytes(artifact.read_bytes())
            slot_helpers.replace_with_hardlink(self, artifact, external)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "compact_key_evidence_artifact_file_shape",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "ABI-7 recursive compact key evidence artifact file must not be hardlinked",
            {item["message"] for item in blockers},
        )
        self.assertNotIn(str(evidence_path.parent), json.dumps(blockers))

    def test_compact_key_evidence_rejects_local_artifact_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            (evidence_path.parent / "recursive-compact-len4.pk").write_bytes(
                b"tampered compact proving key\n"
            )

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "compact_key_evidence_artifact_file_digest",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "recursive-compact-len4.pk",
            {item.get("artifact") for item in blockers},
        )
        self.assertNotIn(
            "recursive-compact-len4.pk",
            result["artifact_sha256"],
        )
        self.assertNotIn(str(evidence_path.parent), json.dumps(result))

    def test_compact_key_evidence_rejects_empty_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            artifact = evidence_path.parent / "recursive-compact-len4.record.norito"
            artifact.write_bytes(b"")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifacts"][artifact.name] = hashlib.sha256(b"").hexdigest()
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_artifact_empty",
            {item["code"] for item in result["blockers"]},
        )

    def test_compact_key_evidence_rejects_placeholder_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            artifact = evidence_path.parent / "recursive-compact-len4.pk"
            placeholder = b"recursive compact key artifact recursive-compact-len4.pk\n"
            artifact.write_bytes(placeholder)
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifacts"][artifact.name] = hashlib.sha256(placeholder).hexdigest()
            evidence["artifact_size_bytes"][artifact.name] = len(placeholder)
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_artifact_placeholder",
            {item["code"] for item in result["blockers"]},
        )
        self.assertNotIn(artifact.name, result["artifact_sha256"])
        self.assertNotIn(artifact.name, result["artifact_size_bytes"])

    def test_compact_key_evidence_rejects_all_placeholder_prefixes(self) -> None:
        for marker in readiness.COMPACT_KEY_PLACEHOLDER_PREFIXES:
            with self.subTest(marker=marker):
                with tempfile.TemporaryDirectory() as temp:
                    evidence_path = create_compact_key_evidence(Path(temp) / "compact")
                    artifact = evidence_path.parent / "recursive-compact-len4.pk"
                    placeholder = marker + b"recursive-compact-len4.pk\n"
                    artifact.write_bytes(placeholder)
                    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
                    evidence["artifacts"][artifact.name] = hashlib.sha256(
                        placeholder
                    ).hexdigest()
                    evidence["artifact_size_bytes"][artifact.name] = len(placeholder)
                    write_json(evidence_path, evidence)

                    result = readiness.check_compact_key_evidence(evidence_path)

                self.assertFalse(result["ok"])
                self.assertIn(
                    "compact_key_evidence_artifact_placeholder",
                    {item["code"] for item in result["blockers"]},
                )
                self.assertNotIn(artifact.name, result["artifact_sha256"])
                self.assertNotIn(artifact.name, result["artifact_size_bytes"])

    def test_compact_key_evidence_rejects_all_zero_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_compact_key_evidence(Path(temp) / "compact")
            artifact = evidence_path.parent / "recursive-compact-len4.pk"
            zero_artifact = b"\x00" * 64
            artifact.write_bytes(zero_artifact)
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifacts"][artifact.name] = hashlib.sha256(
                zero_artifact
            ).hexdigest()
            evidence["artifact_size_bytes"][artifact.name] = len(zero_artifact)
            write_json(evidence_path, evidence)

            result = readiness.check_compact_key_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "compact_key_evidence_artifact_placeholder",
            {item["code"] for item in result["blockers"]},
        )
        self.assertNotIn(artifact.name, result["artifact_sha256"])
        self.assertNotIn(artifact.name, result["artifact_size_bytes"])

    def test_compact_key_evidence_helper_generates_validator_accepted_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            out = artifact_dir / readiness.COMPACT_KEY_EVIDENCE_FILENAME

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = compact_key_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
                        "--generated-at-utc",
                        readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                        "--out",
                        str(out),
                    ]
                )
            evidence = json.loads(out.read_text(encoding="utf-8"))
            result = readiness.check_compact_key_evidence(out)
            expected_sizes = {
                artifact: (artifact_dir / artifact).stat().st_size
                for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
            }

        self.assertEqual(status, 0)
        self.assertEqual(evidence["artifact_size_bytes"], expected_sizes)
        self.assertTrue(result["ok"])
        self.assertEqual(result["state"], "compact_key_artifacts_validated")

    def test_compact_key_evidence_helper_rejects_missing_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            (artifact_dir / "recursive-compact-len4.pk").unlink()

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn("missing recursive compact key artifact recursive-compact-len4.pk", errors)

    def test_compact_key_evidence_helper_rejects_empty_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            (artifact_dir / "recursive-compact-len4.pk").write_bytes(b"")

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn("recursive compact key artifact recursive-compact-len4.pk must be non-empty", errors)

    def test_compact_key_evidence_helper_rejects_placeholder_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            (artifact_dir / "recursive-compact-len4.pk").write_bytes(
                b"recursive compact key artifact recursive-compact-len4.pk\n"
            )

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn(
            (
                "recursive compact key artifact recursive-compact-len4.pk must be "
                "generated key material, not a placeholder fixture"
            ),
            errors,
        )

    def test_compact_key_evidence_helper_rejects_all_placeholder_prefixes(self) -> None:
        for marker in readiness.COMPACT_KEY_PLACEHOLDER_PREFIXES:
            with self.subTest(marker=marker):
                with tempfile.TemporaryDirectory() as temp:
                    artifact_dir = Path(temp) / "compact"
                    create_compact_key_artifact_files(artifact_dir)
                    (artifact_dir / "recursive-compact-len4.pk").write_bytes(
                        marker + b"recursive-compact-len4.pk\n"
                    )

                    evidence, errors = compact_key_helper.build_evidence(
                        artifact_dir=artifact_dir,
                        command=readiness.expected_compact_key_command(),
                        generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
                    )

                self.assertIsNone(evidence)
                self.assertIn(
                    (
                        "recursive compact key artifact recursive-compact-len4.pk must be "
                        "generated key material, not a placeholder fixture"
                    ),
                    errors,
                )

    def test_compact_key_evidence_helper_rejects_all_zero_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            (artifact_dir / "recursive-compact-len4.pk").write_bytes(b"\x00" * 64)

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn(
            (
                "recursive compact key artifact recursive-compact-len4.pk must be "
                "generated key material, not all-zero placeholder bytes"
            ),
            errors,
        )

    def test_compact_key_evidence_helper_rejects_missing_generator_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            (artifact_dir / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME).unlink()

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn("missing recursive compact key generator log", errors)

    def test_compact_key_evidence_helper_rejects_generator_log_size_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            log_path = artifact_dir / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
            log_path.write_text(
                log_path.read_text(encoding="utf-8").replace("record=70", "record=71"),
                encoding="utf-8",
            )

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn(
            (
                "recursive compact key generator log size does not match local artifact "
                "recursive-compact-len4.record.norito"
            ),
            errors,
        )

    def test_compact_key_evidence_helper_rejects_empty_generator_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            log_path = artifact_dir / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
            log_path.write_text("", encoding="utf-8")

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn(
            "compact key generator log must contain exactly one summary line",
            errors,
        )

    def test_compact_key_evidence_helper_rejects_noncanonical_generated_at_utc(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=readiness.expected_compact_key_command(),
                generated_at_utc="2026-06-06T00:00:00+00:00",
            )

        self.assertIsNone(evidence)
        self.assertIn(
            "--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
            errors,
        )

    def test_compact_key_evidence_helper_rejects_appended_shell_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)

            evidence, errors = compact_key_helper.build_evidence(
                artifact_dir=artifact_dir,
                command=f"{readiness.expected_compact_key_command()} ; echo ok",
                generated_at_utc=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
            )

        self.assertIsNone(evidence)
        self.assertIn(
            "--command must exactly match the production ABI-7 recursive compact keygen command",
            errors,
        )

    def test_compact_key_evidence_helper_rejects_outside_artifact_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            artifact_dir.mkdir(parents=True)
            out = Path(temp) / readiness.COMPACT_KEY_EVIDENCE_FILENAME

            errors = compact_key_helper.validate_output_corridor(out, artifact_dir)

        self.assertIn("--out must be written directly under --artifact-dir", errors)

    def test_compact_key_evidence_helper_rejects_symlinked_output_leaf(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            create_compact_key_artifact_files(artifact_dir)
            out = artifact_dir / readiness.COMPACT_KEY_EVIDENCE_FILENAME
            target = artifact_dir / "aliased-recursive-compact-key-evidence.json"
            target.write_text("external\n", encoding="utf-8")
            try:
                out.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = compact_key_helper.main(
                    [
                        "--artifact-dir",
                        str(artifact_dir),
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

    def test_compact_key_evidence_helper_rejects_dangling_symlinked_output_leaf(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            artifact_dir.mkdir()
            out = artifact_dir / readiness.COMPACT_KEY_EVIDENCE_FILENAME
            target = artifact_dir / "missing-recursive-compact-key-evidence.json"
            try:
                out.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = compact_key_helper.write_evidence(out, {"schema": "test"})

        self.assertEqual(errors, ["--out must not be a symlink"])
        self.assertFalse(target.exists())

    def test_compact_key_output_preflight_rejects_parent_create_failure_before_write(
        self,
    ) -> None:
        original_mkdir = Path.mkdir

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            out = root / "missing-compact" / readiness.COMPACT_KEY_EVIDENCE_FILENAME

            def failing_mkdir(path: Path, *args, **kwargs):
                if path == out.parent:
                    raise OSError("simulated compact output parent create failure")
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(Path, "mkdir", failing_mkdir):
                errors = compact_key_helper.preflight_output_path(out, "--out")
            parent_exists = out.parent.exists()
            output_exists = out.exists()

        self.assertEqual(errors, ["--out parent directory could not be created"])
        self.assertFalse(parent_exists)
        self.assertFalse(output_exists)

    def test_compact_key_output_preflight_rejects_file_metadata_failure_before_write(
        self,
    ) -> None:
        original_lstat = Path.lstat

        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / readiness.COMPACT_KEY_EVIDENCE_FILENAME
            out.write_text("existing evidence\n", encoding="utf-8")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == out:
                    raise OSError("simulated compact output file metadata failure")
                return original_lstat(path, *args, **kwargs)

            with mock.patch.object(Path, "lstat", failing_lstat):
                errors = compact_key_helper.preflight_output_path(out, "--out")
            output_text = out.read_text(encoding="utf-8")

        self.assertEqual(errors, ["--out file metadata could not be read"])
        self.assertEqual(output_text, "existing evidence\n")

    def test_compact_key_output_preflight_rejects_hardlink_metadata_failure_before_write(
        self,
    ) -> None:
        original_stat = Path.stat

        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / readiness.COMPACT_KEY_EVIDENCE_FILENAME
            out.write_text("existing evidence\n", encoding="utf-8")

            def failing_stat(path: Path, *args, **kwargs):
                if path == out:
                    raise OSError("simulated compact output hardlink metadata failure")
                return original_stat(path, *args, **kwargs)

            with mock.patch.object(Path, "stat", failing_stat):
                errors = compact_key_helper.preflight_output_path(out, "--out")
            output_text = out.read_text(encoding="utf-8")

        self.assertEqual(errors, ["--out hardlink metadata could not be read"])
        self.assertEqual(output_text, "existing evidence\n")

    def test_compact_key_write_evidence_rejects_write_failure_after_preflight(
        self,
    ) -> None:
        original_write_text = Path.write_text

        def failing_write_text(path: Path, *args, **kwargs):
            if path.name == readiness.COMPACT_KEY_EVIDENCE_FILENAME:
                raise OSError("simulated write failure")
            return original_write_text(path, *args, **kwargs)

        try:
            Path.write_text = failing_write_text
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / readiness.COMPACT_KEY_EVIDENCE_FILENAME

                errors = compact_key_helper.write_evidence(out, {"schema": "test"})
        finally:
            Path.write_text = original_write_text

        self.assertEqual(errors, ["--out could not be written"])
        self.assertFalse(out.exists())

    def test_compact_key_evidence_document_validator_rejects_artifact_dir_create_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"

            with mock.patch.object(
                Path,
                "mkdir",
                side_effect=OSError("simulated compact artifact dir create failure"),
            ):
                errors = compact_key_helper.validate_evidence_document({}, artifact_dir)

            self.assertEqual(
                errors,
                ["--artifact-dir could not be created for evidence validation"],
            )
            self.assertFalse(artifact_dir.exists())

    def test_compact_key_evidence_document_validator_rejects_temp_write_failure_after_preflight(
        self,
    ) -> None:
        class FailingValidationTempFile:
            def __init__(self, path: Path) -> None:
                self.path = path
                self.name = str(path)
                self._handle = None

            def __enter__(self):
                self._handle = self.path.open("w", encoding="utf-8")
                return self

            def __exit__(self, exc_type, exc, traceback) -> bool:
                if self._handle is not None:
                    self._handle.close()
                return False

            def write(self, _text: str) -> int:
                raise OSError("simulated compact validation temp write failure")

        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            artifact_dir.mkdir()
            created_path: Path | None = None

            def failing_named_temp_file(*args, **kwargs):
                nonlocal created_path
                created_path = (
                    Path(kwargs["dir"])
                    / ".recursive-compact-key-evidence-failing-write.json"
                )
                return FailingValidationTempFile(created_path)

            with mock.patch.object(
                compact_key_helper.tempfile,
                "NamedTemporaryFile",
                side_effect=failing_named_temp_file,
            ):
                errors = compact_key_helper.validate_evidence_document({}, artifact_dir)

            self.assertEqual(
                errors,
                ["recursive compact key evidence validation file could not be written"],
            )
            self.assertIsNotNone(created_path)
            assert created_path is not None
            self.assertFalse(created_path.exists())

    def test_compact_key_evidence_document_validator_rejects_temp_cleanup_failure(
        self,
    ) -> None:
        original_unlink = Path.unlink

        def failing_unlink(path: Path, *args, **kwargs):
            if path.name.startswith(".recursive-compact-key-evidence-"):
                raise OSError("simulated compact validation temp cleanup failure")
            return original_unlink(path, *args, **kwargs)

        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"
            artifact_dir.mkdir()

            with mock.patch.object(Path, "unlink", failing_unlink):
                errors = compact_key_helper.validate_evidence_document(
                    {"schema": "invalid"},
                    artifact_dir,
                )

        self.assertEqual(
            errors,
            ["recursive compact key evidence validation file could not be removed"],
        )

    def test_compact_key_artifact_dir_validator_rejects_secret_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_dir = Path(temp) / "token=supersecret-compact"

            errors = compact_key_helper.validate_artifact_dir_path(secret_dir)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["--artifact-dir must not contain secret-looking material"])
        self.assertFalse(secret_dir.exists())
        self.assertNotIn(str(secret_dir), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_compact_key_artifact_dir_validator_rejects_metadata_failure_directly(
        self,
    ) -> None:
        original_lstat = Path.lstat

        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "compact"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == artifact_dir:
                    raise OSError("simulated compact artifact dir metadata failure")
                return original_lstat(path, *args, **kwargs)

            with mock.patch.object(Path, "lstat", failing_lstat):
                errors = compact_key_helper.validate_artifact_dir_path(artifact_dir)

        self.assertEqual(errors, ["--artifact-dir metadata could not be read"])
        self.assertFalse(artifact_dir.exists())

    def test_compact_key_sha256_file_rejects_secret_path_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_file = Path(temp) / "token=supersecret-compact.vk"

            digest, errors = compact_key_helper._sha256_file(
                secret_file,
                "recursive compact key artifact",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["recursive compact key artifact path must not contain secret-looking material"],
        )
        self.assertFalse(secret_file.exists())
        self.assertNotIn(str(secret_file), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_compact_key_sha256_file_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source.vk"
            source.write_text("external\n", encoding="utf-8")
            link = root / "recursive-compact-len4.vk"
            try:
                link.symlink_to(source)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            digest, errors = compact_key_helper._sha256_file(
                link,
                "recursive compact key artifact",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["recursive compact key artifact must not be a symlink"])

    def test_compact_key_sha256_file_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source.vk"
            source.write_text("external\n", encoding="utf-8")
            link = root / "recursive-compact-len4.vk"
            link.write_text("placeholder\n", encoding="utf-8")
            slot_helpers.replace_with_hardlink(self, link, source)

            digest, errors = compact_key_helper._sha256_file(
                link,
                "recursive compact key artifact",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["recursive compact key artifact must not be hardlinked"])

    def test_compact_key_sha256_file_rejects_read_failure_without_traceback(
        self,
    ) -> None:
        original_open = Path.open

        def failing_open(path: Path, *args, **kwargs):
            if path.name == "recursive-compact-len4.vk":
                raise OSError("simulated compact key artifact read failure")
            return original_open(path, *args, **kwargs)

        try:
            with tempfile.TemporaryDirectory() as temp:
                artifact = Path(temp) / "recursive-compact-len4.vk"
                artifact.write_text("artifact\n", encoding="utf-8")
                Path.open = failing_open

                digest, errors = compact_key_helper._sha256_file(
                    artifact,
                    "recursive compact key artifact",
                )
        finally:
            Path.open = original_open

        self.assertIsNone(digest)
        self.assertEqual(errors, ["recursive compact key artifact could not be read"])

    def test_missing_lineage_proof_evidence_blocks_rollup_section(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            missing_path = Path(temp) / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
            result = readiness.check_lineage_proof_evidence(missing_path)

        self.assertFalse(result["ok"])
        self.assertEqual(
            readiness.LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL,
            result["path"],
        )
        self.assertNotIn(str(missing_path.parent), json.dumps(result))
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

    def test_lineage_proof_evidence_rejects_non_utf8_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = Path(temp) / "lineage" / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
            evidence_path.parent.mkdir(parents=True, exist_ok=True)
            evidence_path.write_bytes(b"\xff\xfe\xfd")

            result = readiness.check_lineage_proof_evidence(evidence_path)
            rendered = json.dumps(result)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_unreadable",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(
            "Reserved-lineage proof evidence could not be read",
            {item["message"] for item in result["blockers"]},
        )
        self.assertNotIn(str(evidence_path), rendered)

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

    def test_lineage_proof_evidence_rejects_nonfinite_json_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = Path(temp) / "lineage" / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
            evidence_path.parent.mkdir(parents=True, exist_ok=True)
            evidence_path.write_text('{"schema": Infinity}\n', encoding="utf-8")

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        rendered = json.dumps(result["blockers"])
        self.assertIn("lineage_proof_evidence_invalid_json", rendered)
        self.assertIn("non-finite constant Infinity is not allowed", rendered)

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

    def test_lineage_proof_evidence_rejects_missing_artifact_size_map(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence.pop("artifact_size_bytes")
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        codes = {item["code"] for item in result["blockers"]}
        self.assertIn("lineage_proof_evidence_artifact_sizes", codes)
        self.assertIn("lineage_proof_evidence_artifact_size", codes)

    def test_lineage_proof_evidence_rejects_artifact_size_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifact_size_bytes"]["lineage-init-len128.pk"] += 1
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        blockers = result["blockers"]
        self.assertIn(
            "lineage_proof_evidence_artifact_size",
            {item["code"] for item in blockers},
        )
        self.assertIn(
            "lineage-init-len128.pk",
            {item.get("artifact") for item in blockers},
        )
        self.assertNotIn(
            "lineage-init-len128.pk",
            result["artifact_size_bytes"],
        )

    def test_lineage_proof_evidence_rejects_secret_size_field_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifact_size_bytes"]["token=supersecret"] = 123
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)
            rendered = json.dumps(result["blockers"])

        self.assertFalse(result["ok"])
        self.assertIn("lineage_proof_evidence_artifact_sizes_unexpected_field", rendered)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertNotIn("token=supersecret", rendered)

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

    def test_lineage_proof_evidence_rejects_empty_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            artifact = evidence_path.parent / "lineage-init-len128.record.norito"
            artifact.write_bytes(b"")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifacts"][artifact.name] = hashlib.sha256(b"").hexdigest()
            evidence["artifact_size_bytes"][artifact.name] = 0
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_artifact_empty",
            {item["code"] for item in result["blockers"]},
        )

    def test_lineage_proof_evidence_rejects_all_zero_local_artifact_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            artifact = evidence_path.parent / "lineage-init-len128.pk"
            zero_artifact = b"\x00" * 64
            artifact.write_bytes(zero_artifact)
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifacts"][artifact.name] = hashlib.sha256(zero_artifact).hexdigest()
            evidence["artifact_size_bytes"][artifact.name] = len(zero_artifact)
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        self.assertIn(
            "lineage_proof_evidence_artifact_placeholder",
            {item["code"] for item in result["blockers"]},
        )
        self.assertIn(readiness.LINEAGE_ARTIFACT_ALL_ZERO_ERROR, json.dumps(result))
        self.assertNotIn(artifact.name, result["artifact_sha256"])
        self.assertNotIn(artifact.name, result["artifact_size_bytes"])

    def test_lineage_proof_evidence_uses_local_file_validation_before_artifact_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file

        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            artifact_path = evidence_path.parent / "lineage-init-len128.vk"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == artifact_path:
                    raise OSError("simulated lineage artifact is_file failure")
                return original_is_file(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file

                result = readiness.check_lineage_proof_evidence(evidence_path)
            finally:
                path_type.is_file = original_is_file

        self.assertTrue(result["ok"])
        self.assertIn("lineage-init-len128.vk", result["artifact_sha256"])

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

    def test_lineage_proof_evidence_uses_log_validation_before_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            log_path = (
                evidence_path.parent
                / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
            )

            def failing_is_file(path: Path, *args, **kwargs):
                if path == log_path:
                    raise OSError("simulated lineage proof log is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == log_path:
                    raise OSError("simulated lineage proof log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat

                result = readiness.check_lineage_proof_evidence(evidence_path)
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        codes = {item["code"] for item in result["blockers"]}
        content_issues = {
            item.get("issue")
            for item in result["blockers"]
            if item["code"] == "lineage_proof_evidence_test_log_content"
        }
        self.assertIn("lineage_proof_evidence_test_log_unreadable", codes)
        self.assertIn(
            "production proof log file metadata could not be read",
            content_issues,
        )
        self.assertNotIn(
            "lineage_proof_evidence_test_log_missing",
            codes,
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

    def test_lineage_proof_log_rejects_metadata_read_failure_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        with tempfile.TemporaryDirectory() as temp:
            log_path = Path(temp) / "record-archive-proof.log"
            write_passing_lineage_proof_log(log_path)
            proof_log_stat_calls = 0

            def failing_stat(path: Path, *args, **kwargs):
                nonlocal proof_log_stat_calls
                if path == log_path:
                    proof_log_stat_calls += 1
                    if proof_log_stat_calls > 1:
                        raise OSError("simulated proof log metadata failure")
                return original_stat(path, *args, **kwargs)

            try:
                path_type.stat = failing_stat
                digest, errors = readiness.validate_lineage_proof_log(
                    log_path,
                    readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
                )
            finally:
                path_type.stat = original_stat

        self.assertIsNone(digest)
        self.assertEqual(errors, ["production proof log metadata could not be read"])

    def test_lineage_local_text_rejects_symlink_directly_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            log_path = root / "record-archive-proof.log"
            log_path.write_text("placeholder\n", encoding="utf-8")
            external = root / "external-record-archive-proof.log"
            external.write_text("test result: ok. forged marker\n", encoding="utf-8")
            slot_helpers.replace_with_symlink(self, log_path, external)

            text, errors = readiness._lineage_local_text(
                log_path,
                "production proof log",
                "production proof log could not be read",
                decode_errors="replace",
            )

        self.assertIsNone(text)
        self.assertEqual(errors, ["production proof log must not be a symlink"])

    def test_lineage_local_text_rejects_hardlink_directly_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            log_path = root / "record-archive-proof.log"
            log_path.write_text("placeholder\n", encoding="utf-8")
            external = root / "external-record-archive-proof.log"
            external.write_text("test result: ok. forged marker\n", encoding="utf-8")
            slot_helpers.replace_with_hardlink(self, log_path, external)

            text, errors = readiness._lineage_local_text(
                log_path,
                "production proof log",
                "production proof log could not be read",
                decode_errors="replace",
            )

        self.assertIsNone(text)
        self.assertEqual(errors, ["production proof log must not be hardlinked"])

    def test_lineage_readiness_sha256_file_rejects_secret_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_path = Path(temp) / "token=supersecret-artifact.norito"
            secret_path.write_bytes(b"lineage artifact bytes\n")

            digest, errors = readiness._sha256_file(
                secret_path,
                "lineage rollup artifact",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["lineage rollup artifact path must not contain secret-looking material"],
        )
        self.assertNotIn(str(secret_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_readiness_sha256_file_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external = root / "external-lineage-artifact.norito"
            external.write_bytes(b"external lineage artifact bytes\n")
            artifact = root / "lineage-artifact.norito"
            artifact.write_bytes(b"placeholder\n")
            slot_helpers.replace_with_symlink(self, artifact, external)

            digest, errors = readiness._sha256_file(
                artifact,
                "lineage rollup artifact",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["lineage rollup artifact must not be a symlink"])

    def test_lineage_readiness_sha256_file_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external = root / "external-lineage-artifact.norito"
            external.write_bytes(b"external lineage artifact bytes\n")
            artifact = root / "lineage-artifact.norito"
            artifact.write_bytes(b"placeholder\n")
            slot_helpers.replace_with_hardlink(self, artifact, external)

            digest, errors = readiness._sha256_file(
                artifact,
                "lineage rollup artifact",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["lineage rollup artifact must not be hardlinked"])

    def test_lineage_readiness_sha256_file_rejects_hardlink_metadata_failure_directly(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                artifact = Path(temp) / "lineage-artifact.norito"
                artifact.write_bytes(b"lineage artifact bytes\n")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == artifact:
                        raise OSError("simulated lineage local hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                digest, errors = readiness._sha256_file(
                    artifact,
                    "lineage rollup artifact",
                )
        finally:
            path_type.stat = original_stat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["lineage rollup artifact hardlink metadata could not be read"],
        )

    def test_lineage_readiness_sha256_file_rejects_file_metadata_failure_directly(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                artifact = Path(temp) / "lineage-artifact.norito"
                artifact.write_bytes(b"lineage artifact bytes\n")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == artifact:
                        raise OSError("simulated lineage local file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = readiness._sha256_file(
                    artifact,
                    "lineage rollup artifact",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["lineage rollup artifact file metadata could not be read"],
        )

    def test_lineage_readiness_sha256_file_rejects_read_failure_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact = Path(temp) / "lineage-artifact.norito"
            artifact.write_bytes(b"lineage artifact bytes\n")

            with mock.patch.object(
                Path,
                "open",
                side_effect=OSError("simulated read failure"),
            ):
                digest, errors = readiness._sha256_file(
                    artifact,
                    "lineage rollup artifact",
                )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["lineage rollup artifact could not be read"])

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
            "lineage_proof_evidence_invalid_json",
            {item["code"] for item in nonfinite_result["blockers"]},
        )
        self.assertIn(
            "non-finite constant NaN is not allowed",
            json.dumps(nonfinite_result["blockers"]),
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
        rendered_result = json.dumps(result)
        self.assertIn(slot_helpers.device_lab.SECRET_PATH_REDACTION, rendered_result)
        self.assertNotIn("token=secret", rendered_result)

    def test_lineage_proof_evidence_redacts_secret_required_scalars_in_full_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            evidence_path = create_lineage_proof_evidence(Path(temp) / "lineage")
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["schema"] = "token=secret-schema"
            evidence["generated_at_utc"] = "token=secret-time"
            evidence["record_archive_proof_runtime_keygen_env"] = "token=secret-env"
            evidence["circuit_ids"]["one_hop"] = "token=secret-circuit"
            write_json(evidence_path, evidence)

            result = readiness.check_lineage_proof_evidence(evidence_path)

        self.assertFalse(result["ok"])
        rendered = json.dumps(result)
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
            expected_sizes = {
                artifact: (artifact_dir / artifact).stat().st_size
                for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS
            }

        self.assertEqual(status, 0)
        self.assertTrue(result["ok"])
        self.assertEqual(evidence["artifact_size_bytes"], expected_sizes)
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

    def test_lineage_proof_evidence_document_validator_rejects_artifact_dir_create_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"

            with mock.patch.object(
                Path,
                "mkdir",
                side_effect=OSError("simulated artifact dir create failure"),
            ):
                errors = evidence_helper.validate_evidence_document({}, artifact_dir)

            self.assertEqual(
                errors,
                ["--artifact-dir could not be created for evidence validation"],
            )
            self.assertFalse(artifact_dir.exists())

    def test_lineage_proof_evidence_document_validator_rejects_temp_write_failure_after_preflight(
        self,
    ) -> None:
        class FailingValidationTempFile:
            def __init__(self, path: Path) -> None:
                self.path = path
                self.name = str(path)
                self._handle = None

            def __enter__(self):
                self._handle = self.path.open("w", encoding="utf-8")
                return self

            def __exit__(self, exc_type, exc, traceback) -> bool:
                if self._handle is not None:
                    self._handle.close()
                return False

            def write(self, _text: str) -> int:
                raise OSError("simulated validation temp write failure")

        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            artifact_dir.mkdir()
            created_path: Path | None = None

            def failing_named_temp_file(*args, **kwargs):
                nonlocal created_path
                created_path = (
                    Path(kwargs["dir"])
                    / ".lineage-proof-evidence-failing-write.json"
                )
                return FailingValidationTempFile(created_path)

            with mock.patch.object(
                evidence_helper.tempfile,
                "NamedTemporaryFile",
                side_effect=failing_named_temp_file,
            ):
                errors = evidence_helper.validate_evidence_document({}, artifact_dir)

            self.assertEqual(
                errors,
                ["lineage proof evidence validation file could not be written"],
            )
            self.assertIsNotNone(created_path)
            assert created_path is not None
            self.assertFalse(created_path.exists())

    def test_lineage_proof_evidence_document_validator_rejects_temp_cleanup_failure(
        self,
    ) -> None:
        original_unlink = Path.unlink

        def failing_unlink(path: Path, *args, **kwargs):
            if path.name.startswith(".lineage-proof-evidence-"):
                raise OSError("simulated validation temp cleanup failure")
            return original_unlink(path, *args, **kwargs)

        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            artifact_dir.mkdir()

            with mock.patch.object(Path, "unlink", failing_unlink):
                errors = evidence_helper.validate_evidence_document(
                    {"schema": "invalid"},
                    artifact_dir,
                )

        self.assertEqual(
            errors,
            ["lineage proof evidence validation file could not be removed"],
        )

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

    def test_lineage_proof_artifact_dir_validator_rejects_metadata_failure_directly(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                artifact_dir = Path(temp) / "artifacts"
                artifact_dir.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == artifact_dir:
                        raise OSError("simulated artifact-dir metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_helper.validate_artifact_dir_path(artifact_dir)
                artifact_dir_exists = artifact_dir.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--artifact-dir metadata could not be read"])
        self.assertTrue(artifact_dir_exists)

    def test_lineage_proof_sha256_file_rejects_secret_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_path = Path(temp) / "token=supersecret-artifact.norito"
            secret_path.write_bytes(b"lineage artifact bytes\n")

            digest, errors = evidence_helper._sha256_file(
                secret_path,
                "lineage artifact secret",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["lineage artifact secret path must not contain secret-looking material"],
        )
        self.assertNotIn(str(secret_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_lineage_proof_sha256_file_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external = root / "external-lineage-artifact.norito"
            external.write_bytes(b"external lineage artifact bytes\n")
            artifact = root / "lineage-artifact.norito"
            artifact.write_bytes(b"placeholder\n")
            slot_helpers.replace_with_symlink(self, artifact, external)

            digest, errors = evidence_helper._sha256_file(
                artifact,
                "lineage artifact direct hash",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["lineage artifact direct hash must not be a symlink"])

    def test_lineage_proof_sha256_file_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external = root / "external-lineage-artifact.norito"
            external.write_bytes(b"external lineage artifact bytes\n")
            artifact = root / "lineage-artifact.norito"
            artifact.write_bytes(b"placeholder\n")
            slot_helpers.replace_with_hardlink(self, artifact, external)

            digest, errors = evidence_helper._sha256_file(
                artifact,
                "lineage artifact direct hash",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["lineage artifact direct hash must not be hardlinked"])

    def test_lineage_proof_sha256_file_rejects_read_failure_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact = Path(temp) / "lineage-artifact.norito"
            artifact.write_bytes(b"lineage artifact bytes\n")

            with mock.patch.object(
                Path,
                "open",
                side_effect=OSError("simulated read failure"),
            ):
                digest, errors = evidence_helper._sha256_file(
                    artifact,
                    "lineage artifact direct hash",
                )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["lineage artifact direct hash could not be read"])

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

    def test_lineage_proof_evidence_helper_rejects_empty_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            (artifact_dir / "lineage-append-len128.pk").write_bytes(b"")
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
            "lineage artifact lineage-append-len128.pk must be non-empty",
            stderr.getvalue(),
        )

    def test_lineage_proof_evidence_helper_rejects_all_zero_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            create_lineage_artifact_files(artifact_dir)
            (artifact_dir / "lineage-append-len128.pk").write_bytes(b"\x00" * 64)
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
        self.assertIn(readiness.LINEAGE_ARTIFACT_ALL_ZERO_ERROR, stderr.getvalue())

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

    def test_lineage_proof_output_corridor_rejects_parent_resolve_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                artifact_dir = Path(temp) / "artifacts"
                artifact_dir.mkdir()
                out = artifact_dir / "lineage-proof-evidence.json"

                def failing_resolve(path: Path, *args, **kwargs):
                    if path == out.parent:
                        raise OSError("simulated output parent resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                errors = evidence_helper.validate_output_corridor(out, artifact_dir)
        finally:
            path_type.resolve = original_resolve

        self.assertEqual(errors, ["--out parent could not be resolved"])
        self.assertFalse(out.exists())

    def test_lineage_proof_output_corridor_rejects_artifact_dir_resolve_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                artifact_dir = Path(temp) / "artifacts"
                artifact_dir.mkdir()
                out = artifact_dir / "lineage-proof-evidence.json"
                output_parent_resolved = False

                def failing_resolve(path: Path, *args, **kwargs):
                    nonlocal output_parent_resolved
                    if path == out.parent and not output_parent_resolved:
                        output_parent_resolved = True
                        return original_resolve(path, *args, **kwargs)
                    if path == artifact_dir:
                        raise OSError("simulated artifact-dir resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                errors = evidence_helper.validate_output_corridor(out, artifact_dir)
        finally:
            path_type.resolve = original_resolve

        self.assertEqual(errors, ["--artifact-dir could not be resolved"])
        self.assertFalse(out.exists())

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

    def test_lineage_proof_output_preflight_uses_lstat_before_parent_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / "lineage" / "lineage-proof-evidence.json"
                out.parent.mkdir(parents=True)

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == out.parent:
                        raise OSError("simulated lineage output parent is_dir preflight failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = evidence_helper.preflight_output_path(out, "--out")
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertFalse(out.exists())

    def test_lineage_proof_output_preflight_rejects_parent_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / "lineage" / "lineage-proof-evidence.json"
                out.parent.mkdir(parents=True)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == out.parent:
                        raise OSError("simulated lineage output parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_helper.preflight_output_path(out, "--out")
                output_exists = out.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--out parent directory metadata could not be read"])
        self.assertFalse(output_exists)

    def test_lineage_proof_output_validator_uses_lstat_before_parent_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / "lineage" / "lineage-proof-evidence.json"
                out.parent.mkdir(parents=True)

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == out.parent:
                        raise OSError("simulated lineage output validator parent is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = evidence_helper.validate_output_path(out, "--out")
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertFalse(out.exists())

    def test_lineage_proof_output_validator_rejects_parent_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / "lineage" / "lineage-proof-evidence.json"
                out.parent.mkdir(parents=True)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == out.parent:
                        raise OSError("simulated lineage output validator parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_helper.validate_output_path(out, "--out")
                output_exists = out.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--out parent directory metadata could not be read"])
        self.assertFalse(output_exists)

    def test_lineage_proof_output_preflight_rejects_parent_create_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                out = root / "missing-lineage" / "lineage-proof-evidence.json"

                def failing_mkdir(path: Path, *args, **kwargs):
                    if path == out.parent:
                        raise OSError("simulated lineage output parent create failure")
                    return original_mkdir(path, *args, **kwargs)

                path_type.mkdir = failing_mkdir

                errors = evidence_helper.preflight_output_path(out, "--out")
                parent_exists = out.parent.exists()
                output_exists = out.exists()
        finally:
            path_type.mkdir = original_mkdir

        self.assertEqual(errors, ["--out parent directory could not be created"])
        self.assertFalse(parent_exists)
        self.assertFalse(output_exists)

    def test_lineage_proof_output_preflight_rechecks_parent_after_create(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                out = root / "late-linked-lineage" / "lineage-proof-evidence.json"
                alias_target = root / "external-lineage"
                alias_target.mkdir()

                def replacing_mkdir(path: Path, *args, **kwargs):
                    if path == out.parent:
                        slot_helpers.create_dir_symlink(self, path, alias_target)
                        return None
                    return original_mkdir(path, *args, **kwargs)

                path_type.mkdir = replacing_mkdir

                errors = evidence_helper.preflight_output_path(out, "--out")
        finally:
            path_type.mkdir = original_mkdir

        self.assertEqual(errors, ["--out parent directory must not be a symlink"])
        self.assertFalse((alias_target / "lineage-proof-evidence.json").exists())

    def test_lineage_proof_output_validator_rejects_parent_create_failure_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir
        original_preflight = evidence_helper.preflight_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                out = root / "late-missing-lineage" / "lineage-proof-evidence.json"
                mkdir_calls = 0

                def failing_second_mkdir(path: Path, *args, **kwargs):
                    nonlocal mkdir_calls
                    if path == out.parent:
                        mkdir_calls += 1
                        if mkdir_calls == 2:
                            raise OSError("simulated lineage output validator parent create failure")
                    return original_mkdir(path, *args, **kwargs)

                def removing_preflight(path: Path, label: str) -> list[str]:
                    errors = original_preflight(path, label)
                    if path == out and not errors and out.parent.exists():
                        out.parent.rmdir()
                    return errors

                path_type.mkdir = failing_second_mkdir
                evidence_helper.preflight_output_path = removing_preflight

                errors = evidence_helper.validate_output_path(out, "--out")
                parent_exists = out.parent.exists()
                output_exists = out.exists()
        finally:
            evidence_helper.preflight_output_path = original_preflight
            path_type.mkdir = original_mkdir

        self.assertEqual(errors, ["--out parent directory could not be created"])
        self.assertFalse(parent_exists)
        self.assertFalse(output_exists)

    def test_lineage_proof_output_preflight_rejects_file_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / "lineage-proof-evidence.json"
                out.write_text("existing evidence\n", encoding="utf-8")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == out:
                        raise OSError("simulated lineage output file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_helper.preflight_output_path(out, "--out")
                output_text = out.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--out file metadata could not be read"])
        self.assertEqual(output_text, "existing evidence\n")

    def test_lineage_proof_output_preflight_rejects_hardlink_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / "lineage-proof-evidence.json"
                out.write_text("existing evidence\n", encoding="utf-8")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == out:
                        raise OSError("simulated lineage output hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                errors = evidence_helper.preflight_output_path(out, "--out")
                output_text = out.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertEqual(errors, ["--out hardlink metadata could not be read"])
        self.assertEqual(output_text, "existing evidence\n")

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

    def test_lineage_proof_write_evidence_rejects_write_failure_after_preflight(
        self,
    ) -> None:
        original_write_text = Path.write_text

        def failing_write_text(path: Path, *args, **kwargs):
            if path.name == "lineage-proof-evidence.json":
                raise OSError("simulated write failure")
            return original_write_text(path, *args, **kwargs)

        try:
            Path.write_text = failing_write_text
            with tempfile.TemporaryDirectory() as temp:
                out = Path(temp) / "lineage-proof-evidence.json"

                errors = evidence_helper.write_evidence(out, {"schema": "test"})
        finally:
            Path.write_text = original_write_text

        self.assertEqual(errors, ["--out could not be written"])
        self.assertFalse(out.exists())

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

    def test_lineage_proof_evidence_helper_rejects_dangling_symlinked_output_leaf(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            artifact_dir = Path(temp) / "artifacts"
            artifact_dir.mkdir()
            out = artifact_dir / "lineage-proof-evidence.json"
            target = artifact_dir / "missing-lineage-proof-evidence.json"
            try:
                out.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = evidence_helper.write_evidence(out, {"schema": "test"})

        self.assertEqual(errors, ["--out must not be a symlink"])
        self.assertFalse(target.exists())

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

    def test_lineage_proof_input_validator_rejects_parent_resolve_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                artifact_dir = Path(temp) / "artifacts"
                artifact_dir.mkdir()
                proof_log = artifact_dir / readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                    "record_archive_proof"
                ]

                def failing_resolve(path: Path, *args, **kwargs):
                    if path == proof_log.parent:
                        raise OSError("simulated proof-log parent resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                errors = evidence_helper.validate_lineage_input_paths(
                    artifact_dir,
                    proof_log,
                )
        finally:
            path_type.resolve = original_resolve

        self.assertEqual(errors, ["--proof-log parent could not be resolved"])
        self.assertFalse(proof_log.exists())

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
            compact_key_evidence = create_compact_key_evidence(lineage_evidence.parent)
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
                        "--compact-key-evidence",
                        str(compact_key_evidence),
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
            compact_key_evidence = create_compact_key_evidence(lineage_evidence.parent)
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
                        "--compact-key-evidence",
                        str(compact_key_evidence),
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

    def test_validate_repo_root_rejects_metadata_failure_directly_without_leak(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                repo_root = Path(temp) / "repo"
                repo_root.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == repo_root:
                        raise OSError("simulated repo-root metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = readiness.validate_repo_root_path(repo_root)
                rendered = json.dumps(errors)
                repo_root_exists = repo_root.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_repo_root_path_invalid",
                    "message": "--repo-root metadata could not be read",
                }
            ],
        )
        self.assertTrue(repo_root_exists)
        self.assertNotIn(str(repo_root), rendered)

    def test_main_rejects_repo_root_resolve_failure_without_traceback(self) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                repo_root = root / "repo"
                repo_root.mkdir()
                summary_path = root / "summary.json"
                stdout = io.StringIO()
                stderr = io.StringIO()

                def failing_resolve(path: Path, *args, **kwargs):
                    if path == repo_root:
                        raise OSError("simulated repo-root resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                with redirect_stdout(stdout), redirect_stderr(stderr):
                    status = readiness.main(
                        [
                            "--repo-root",
                            str(repo_root),
                            "--summary-out",
                            str(summary_path),
                        ]
                    )
                summary_text = summary_path.read_text(encoding="utf-8")
                rendered = stdout.getvalue() + stderr.getvalue() + summary_text
        finally:
            path_type.resolve = original_resolve

        self.assertEqual(status, 1)
        self.assertIn("kagemusha_repo_root_path_invalid", rendered)
        self.assertIn("--repo-root could not be resolved", rendered)
        self.assertNotIn("Traceback", rendered)
        self.assertNotIn(str(repo_root), rendered)

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

    def test_write_summary_rejects_non_regular_output_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            summary_path.mkdir()

            errors = readiness.write_summary(
                summary_path,
                {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
            )

        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out must be a regular file",
                }
            ],
        )

    def test_validate_summary_output_path_uses_lstat_before_parent_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated summary parent is_dir preflight failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = readiness.validate_summary_output_path(summary_path)
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertFalse(summary_path.exists())

    def test_validate_summary_output_path_rejects_parent_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated summary parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = readiness.validate_summary_output_path(summary_path)
                output_exists = summary_path.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out parent directory metadata could not be read",
                }
            ],
        )
        self.assertFalse(output_exists)

    def test_write_summary_uses_lstat_before_parent_is_dir_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated summary writer parent is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = readiness.write_summary(
                    summary_path,
                    {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
                )
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertIn('"ready": false', summary_text)

    def test_write_summary_rejects_parent_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated summary writer parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = readiness.write_summary(
                    summary_path,
                    {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
                )
                output_exists = summary_path.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out parent directory metadata could not be read",
                }
            ],
        )
        self.assertFalse(output_exists)

    def test_write_summary_rejects_file_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"
                summary_path.write_text("existing summary\n", encoding="utf-8")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path:
                        raise OSError("simulated summary lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = readiness.write_summary(
                    summary_path,
                    {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
                )
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(summary_text, "existing summary\n")
        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out file metadata could not be read",
                }
            ],
        )

    def test_write_summary_rejects_hardlink_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"
                summary_path.write_text("existing summary\n", encoding="utf-8")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == summary_path:
                        raise OSError("simulated summary stat failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                errors = readiness.write_summary(
                    summary_path,
                    {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
                )
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertEqual(summary_text, "existing summary\n")
        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out hardlink metadata could not be read",
                }
            ],
        )

    def test_write_summary_rejects_write_failure_after_preflight(self) -> None:
        original_write_text = Path.write_text

        def failing_write_text(path: Path, *args, **kwargs):
            if path.name == "summary.json":
                raise OSError("simulated write failure")
            return original_write_text(path, *args, **kwargs)

        try:
            Path.write_text = failing_write_text
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                errors = readiness.write_summary(
                    summary_path,
                    {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
                )
        finally:
            Path.write_text = original_write_text

        self.assertFalse(summary_path.exists())
        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out could not be written",
                }
            ],
        )

    def test_write_summary_rejects_parent_create_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "missing-summary-parent" / "summary.json"

                def failing_mkdir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated readiness summary parent mkdir failure")
                    return original_mkdir(path, *args, **kwargs)

                path_type.mkdir = failing_mkdir

                errors = readiness.write_summary(
                    summary_path,
                    {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
                )
        finally:
            path_type.mkdir = original_mkdir

        self.assertFalse(summary_path.exists())
        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out parent directory could not be created",
                }
            ],
        )

    def test_write_summary_rechecks_parent_after_create_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                summary_path = root / "late-linked-summary" / "summary.json"
                alias_target = root / "external-summary"
                alias_target.mkdir()

                def replacing_mkdir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        slot_helpers.create_dir_symlink(self, path, alias_target)
                        return None
                    return original_mkdir(path, *args, **kwargs)

                path_type.mkdir = replacing_mkdir

                errors = readiness.write_summary(
                    summary_path,
                    {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
                )
        finally:
            path_type.mkdir = original_mkdir

        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out parent directory must not be a symlink",
                }
            ],
        )
        self.assertFalse((alias_target / "summary.json").exists())

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

    def test_dangling_symlinked_summary_out_blocks_without_following_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_link = Path(temp) / "summary.json"
            target = Path(temp) / "missing-summary.json"
            try:
                summary_link.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = readiness.write_summary(
                summary_link,
                {"schema": readiness.SUMMARY_SCHEMA, "ready": False},
            )

        self.assertEqual(
            errors,
            [
                {
                    "code": "kagemusha_summary_out_path_invalid",
                    "message": "--summary-out must not be a symlink",
                }
            ],
        )
        self.assertFalse(target.exists())

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

    def test_negative_compact_key_future_skew_blocks_before_rollup(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--max-compact-key-evidence-future-skew-seconds",
                        "-1",
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 1)
        self.assertFalse(summary["ready"])
        self.assertIn(
            "compact_key_evidence_max_timestamp_invalid",
            {item["code"] for item in summary["blockers"]},
        )
        self.assertIn("compact_key_evidence_max_timestamp_invalid", stderr.getvalue())

    def test_secret_looking_compact_key_evidence_path_blocks_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            secret_evidence_path = Path(temp) / "token=supersecret-compact.json"
            stderr = io.StringIO()

            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = readiness.main(
                    [
                        "--repo-root",
                        str(REPO_ROOT),
                        "--compact-key-evidence",
                        str(secret_evidence_path),
                        "--summary-out",
                        str(summary_path),
                    ]
                )
            summary_text = summary_path.read_text(encoding="utf-8")
            rendered = stderr.getvalue() + summary_text

        self.assertEqual(status, 1)
        self.assertIn("compact_key_evidence_path_invalid", rendered)
        self.assertIn("--compact-key-evidence must not contain secret-looking material", rendered)
        self.assertNotIn("token=supersecret", rendered)


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
