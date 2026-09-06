"""Exercise KAGEMUSHA formal proof gates without Cargo or release evidence claims."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FORMAL = ROOT / "formal" / "kagemusha_v1"
JAR = os.environ.get("TLA2TOOLS_JAR")


class KagemushaFormalProofGateTests(unittest.TestCase):
    """Keep the full safety configuration and require genuine gate counterexamples."""

    def test_full_configuration_retains_three_message_monetary_and_rotation_safety(self):
        config = (FORMAL / "KagemushaV1.cfg").read_text()
        for invariant in (
            "ThreeMessageWireShape", "RequestsNeverBindReceiverState",
            "DistinctPaymentsMayShareARequest", "SenderCommitWithinRequestWindow",
            "PaymentsBindExactRequests", "HardwareAuthorityIsRecursive",
            "ReceiveFoldUsesReplayNonmembership", "ExactNextNonForking",
            "EveryTransitionWasPrepared", "SameIdDifferentBytesAreRejected",
            "AcknowledgementsFollowDurableStaging",
            "ExactDuplicateReturnsSameDurableAcknowledgement",
            "RotationCarriesCompleteState", "ReserveEquation",
            "LiabilityConservation", "TotalValueConservation",
            "SenderSplitIsConservative", "NoCountBasedReceiveRejection",
            "CommittedPaymentsRemainReceivable", "OutboxNeverFreezesSenderRemainder",
            "MintRequiresFinalityAndAuthorization", "RedemptionSplitsAreFullOrPartial",
            "AppliedRedemptionNullifiersAreUnique",
            "PublicStateAndProofShapeIsHistoryIndependent",
        ):
            self.assertRegex(config, rf"(?m)^  {invariant}$")
        for temporal_property in (
            "CumulativeEvidenceNeverShrinks", "PostCommitArtifactsNeverChange",
        ):
            self.assertRegex(config, rf"(?m)^  {temporal_property}$")
        source = (FORMAL / "KagemushaV1.tla").read_text()
        self.assertIn('<<"Request", "Payment", "Acknowledgement">>', source)
        self.assertNotIn('"AcceptanceIntent"', source)
        self.assertNotIn('"AcceptanceTicket"', source)
        self.assertNotIn("ReceiveFoldBatch", source)
        self.assertNotIn("SuiteUpgrade", source)

    def run_tlc(self, mutation):
        if not JAR:
            self.skipTest("Set TLA2TOOLS_JAR to the checksum-pinned TLA+ jar")
        self.assertTrue(Path(JAR).is_file(), "TLA2TOOLS_JAR must name an existing jar")
        config = (FORMAL / "KagemushaV1.cfg").read_text()
        config = config.replace("CONSTANTS\n", f'CONSTANTS\n  Mutation = "{mutation}"\n', 1)
        config = config.replace("SPECIFICATION Spec", "SPECIFICATION HarnessSpec")
        config = config.replace("INVARIANTS\n", "INVARIANTS\n  HarnessCompletion\n", 1)
        with tempfile.TemporaryDirectory(prefix="kagemusha-proof-gates-") as temporary:
            directory = Path(temporary)
            cfg = directory / "proof-gates.cfg"
            cfg.write_text(config)
            result = subprocess.run(
                ["java", "-Xmx768m", "-XX:+UseParallelGC", "-cp", JAR, "tlc2.TLC",
                 "-workers", "1", "-metadir", str(directory / "states"),
                 "-config", str(cfg), "KagemushaV1ProofGates.tla"],
                cwd=FORMAL, capture_output=True, text=True, timeout=120, check=False,
            )
        output = result.stdout + result.stderr
        self.assertNotIn("Parsing or semantic analysis failed", output)
        return result.returncode, output

    def test_both_full_crash_rotation_traces_complete_with_every_invariant(self):
        code, output = self.run_tlc("none")
        self.assertEqual(code, 0, output)
        self.assertIn("Model checking completed. No error has been found.", output)
        self.assertRegex(output, r"52 distinct states found")
        self.assertRegex(output, r"depth of the complete state graph search is 31")

    def test_nonrecursive_payment_has_counterexample(self):
        self.assert_counterexample("nonrecursive-payment", "HardwareAuthorityIsRecursive")

    def test_second_successor_has_counterexample(self):
        self.assert_counterexample("fork", "ExactNextNonForking")

    def test_ack_before_durable_stage_has_counterexample(self):
        self.assert_counterexample("ack-before-stage", "AcknowledgementsFollowDurableStaging")

    def test_conflicting_credit_bytes_have_counterexample(self):
        self.assert_counterexample("conflict-accepted", "SameIdDifferentBytesAreRejected")

    def test_same_credit_replay_has_counterexample(self):
        self.assert_counterexample("replay", "ReceiveFoldUsesReplayNonmembership")

    def test_expired_sender_commit_has_counterexample(self):
        self.assert_counterexample("expired-commit", "SenderCommitWithinRequestWindow")

    def test_reserve_accounting_omission_has_counterexample(self):
        self.assert_counterexample("reserve-accounting", "ReserveEquation")

    def test_rotation_state_loss_has_counterexample(self):
        self.assert_counterexample("rotation-loss", "RotationCarriesCompleteState")

    def assert_counterexample(self, mutation, invariant):
        code, output = self.run_tlc(mutation)
        self.assertNotEqual(code, 0, output)
        self.assertIn(f"Invariant {invariant} is violated.", output)
        self.assertIn("The behavior up to this point is:", output)


if __name__ == "__main__":
    unittest.main()
