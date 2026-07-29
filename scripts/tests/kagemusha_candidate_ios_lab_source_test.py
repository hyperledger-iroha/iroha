"""Static contract tests for the non-shipping physical-iOS candidate lane."""

from __future__ import annotations

import pathlib
import subprocess
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
RUST = ROOT / "crates/connect_norito_bridge/src/kagemusha_candidate_apple.rs"
LIB = ROOT / "crates/connect_norito_bridge/src/lib.rs"
HEADER = ROOT / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
BUILD = ROOT / "scripts/build_kagemusha_candidate_apple_native.sh"
RUNNER = ROOT / "scripts/run_kagemusha_candidate_ios_lab.sh"
GATE = ROOT / "ci/check_kagemusha_production_readiness.sh"
EVIDENCE_VALIDATOR = ROOT / "scripts/kagemusha_candidate_ios_evidence.py"
PROJECT = ROOT / "IrohaSwift/KagemushaCandidateEvidenceLab/project.yml"
TEST = (
    ROOT
    / "IrohaSwift/KagemushaCandidateEvidenceLab/Tests/"
    "KagemushaCandidateEvidenceLabTests.swift"
)


EXPECTED_OPERATIONS = (
    "candidate_install",
    "build_init_request",
    "init",
    "build_append_hop_01_request",
    "append_hop_01",
    "build_append_hop_02_request",
    "append_hop_02",
    "candidate_reinstall_after_process_restart",
    "restore_init_result_after_restart",
    "restore_hop_01_result_after_restart",
    "restore_hop_02_result_after_restart",
    "validate_init_branch_after_restart",
    "validate_hop_01_change_continuity",
    "validate_hop_01_recipient_branch",
    "validate_hop_02_recipient_branch",
    "validate_sender_change_branch",
    "build_verify_first_recipient_proof_request",
    "verify_first_recipient_proof",
    "build_verify_multi_hop_recipient_proof_request",
    "verify_multi_hop_recipient_proof",
    "build_duplicate_input_request_from_observed_branch",
    "duplicate_input_rejection",
    "build_redeem_first_recipient_request",
    "redeem_first_recipient",
    "build_redeem_second_recipient_request",
    "redeem_second_recipient",
    "build_redeem_sender_change_request",
    "redeem_sender_change",
)


class CandidateIOSLabSourceTest(unittest.TestCase):
    def test_shell_sources_parse(self) -> None:
        subprocess.run(
            ["/bin/bash", "-n", str(BUILD), str(RUNNER), str(GATE)],
            check=True,
            cwd=ROOT,
        )

    def test_native_module_is_physical_ios_only(self) -> None:
        source = LIB.read_text(encoding="utf-8")
        module_guard = source[source.index("mod kagemusha_candidate_apple") - 140 :]
        module_guard = module_guard[:220]
        self.assertIn('target_os = "ios"', module_guard)
        self.assertIn('not(target_abi = "sim")', module_guard)
        for symbol in (
            "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1",
            "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1",
        ):
            self.assertEqual(source.count(f"fn {symbol}("), 1)
            self.assertIn(symbol, HEADER.read_text(encoding="utf-8"))

    def test_native_transcript_has_exact_lifecycle_contract(self) -> None:
        source = RUST.read_text(encoding="utf-8")
        self.assertIn(
            "const APPLE_RESOURCE_CEILING_BYTES: u64 = 6 * 1024 * 1024 * 1024;",
            source,
        )
        self.assertIn("const EXPECTED_DUPLICATE_REJECTION_CODE: c_int = -311;", source)
        self.assertIn('"final_unspent_atomic_units"', source)
        self.assertIn('"reviewed_source_closure_descriptor_sha256"', source)
        self.assertIn('"artifact_inventory"', source)
        self.assertIn("causal_events.len() != 28", source)
        positions = [source.index(f'"{operation}"') for operation in EXPECTED_OPERATIONS]
        self.assertEqual(positions, sorted(positions))
        self.assertEqual(len(set(positions)), 28)

    def test_build_profile_has_no_simulator_or_production_capability(self) -> None:
        source = BUILD.read_text(encoding="utf-8")
        self.assertIn("--target aarch64-apple-ios", source)
        self.assertNotIn("--target aarch64-apple-ios-sim", source)
        self.assertNotIn("x86_64-apple-ios", source)
        self.assertIn("--features kagemusha-candidate-evidence-lab", source)
        self.assertIn('"simulator_slice_present": False', source)
        self.assertIn('"production_capability_enabled": False', source)
        self.assertIn("KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2", source)

    def test_harness_measures_offline_window_and_two_processes(self) -> None:
        source = TEST.read_text(encoding="utf-8")
        self.assertIn("#if targetEnvironment(simulator)", source)
        self.assertIn("NWPathMonitor()", source)
        self.assertIn('sample("before")', source)
        self.assertIn('sample("through_before_native")', source)
        self.assertIn('sample("through_after_native")', source)
        self.assertIn('sample("after")', source)
        self.assertIn("CountingURLProtocol.observedCount()", source)
        self.assertIn("proofPID != getpid()", source)
        self.assertIn("writeDurably(checkpoint", source)
        self.assertIn("reopenedCheckpoint == checkpoint", source)
        self.assertIn("taira-testnet-physical-ios-xcode-paired-v1", source)
        self.assertIn('"app_attest_used": false', source)

    def test_runner_retains_only_hashed_device_identifiers(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        self.assertIn('"device_udid_sha256"', source)
        self.assertIn('"device_ecid_sha256"', source)
        self.assertIn('"device_serial_sha256"', source)
        self.assertIn("find \"$TRANSIENT\" -depth -delete", source)
        self.assertIn("testProofPhase", source)
        self.assertIn("testRestartPhase", source)
        self.assertIn("-parallel-testing-enabled NO", source.replace("\\\n", " "))
        self.assertNotIn("platform=iOS Simulator", source)

    def test_xcode_project_links_only_external_candidate_framework(self) -> None:
        source = PROJECT.read_text(encoding="utf-8")
        self.assertIn("${KAGEMUSHA_CANDIDATE_XCFRAMEWORK_PATH}", source)
        self.assertNotIn("NoritoBridge.xcframework", source)
        self.assertIn("SUPPORTED_PLATFORMS: iphoneos", source)
        self.assertIn("SUPPORTS_MACCATALYST: NO", source)

    def test_signed_evidence_and_promotion_gate_bind_the_raw_candidate(self) -> None:
        validator = EVIDENCE_VALIDATOR.read_text(encoding="utf-8")
        gate = GATE.read_text(encoding="utf-8")
        self.assertIn("SIGNED_EVIDENCE_FIELDS", validator)
        self.assertIn("canonical_signature_payload", validator)
        self.assertIn("verify_ed25519", validator)
        self.assertIn("CODE_SIGN_MEASUREMENTS_FIELDS", validator)
        self.assertIn("TEST_RESULT_FIELDS", validator)
        self.assertIn("CAUSAL_OPERATIONS", validator)
        self.assertIn("KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT", gate)
        self.assertIn("check_kagemusha_candidate_ios_evidence.py", gate)
        self.assertIn('artifact_digests.get("input/candidate-v4.norito")', gate)
        self.assertIn('report.get("candidate_sha256")', gate)


if __name__ == "__main__":
    unittest.main()
