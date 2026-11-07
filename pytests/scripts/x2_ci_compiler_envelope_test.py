"""Keep the X2 compiler bound scoped without dropping native test coverage."""

import json
import os
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class X2CompilerEnvelope(unittest.TestCase):
    def test_actual_commands_preserve_selectors_and_scope_compiler_jobs(self):
        """Execute the CI shell with an argv recorder, never Cargo or Rust."""
        workflow = (ROOT / ".github/workflows/pr.yml").read_text()
        job = workflow.split("  x2_maximum_proof_transport:\n", 1)[1].split("\n  norito_rpc_verify:", 1)[0]
        self.assertIn("    timeout-minutes: 120\n", job)
        self.assertNotRegex(job, r"(?m)^    (env|if|continue-on-error):")
        cases = (
            ("Complete canonical envelopes and exact/one-over contract bounds", [
                ["test", "--locked", "-p", "iroha_data_model", "--features", "privacy-exact12-conformance", "--test", "resource_contract_v1"],
            ], None),
            ("Committed policy coherence, chain frame and sync bounds, complete maximum frames", [
                ["test", "--locked", "-p", "iroha_data_model", "--lib", "--", "parameter::system::resource_coherence", "parameter::system::tests::sumeragi_parameter_defaults_match_the_consensus_core"],
                ["test", "--locked", "-p", "iroha_genesis", "--lib", "--", "effective_resource_parameters", "effective_chain_parameters_fit_the_protocol_transport_limit"],
                ["test", "--locked", "-p", "iroha_config", "--test", "iroha_config_integration", "--", "fixtures::network_defaults_carry_maximal_sumeragi_frames"],
                ["test", "--locked", "-p", "iroha_sumeragi", "--lib", "--", "pacemaker::", "machine::sync::tests::", "message::tests::generic_maximum_frames_fit_every_admissible_transport", "message::tests::source_complete_maximum_certificate_and_header_fit_declared_frame_allowance"],
            ], None),
            ("Daemon startup refuses frame caps below the chain, sync and control bounds", [
                ["test", "--locked", "-p", "irohad_lib", "--lib", "--", "block_sync_topic_must_carry_the_local_sync_budget", "control_topic_must_carry_the_largest_control_frame", "check_config_and_runtime_enforce_frame_cap_boundary"],
            ], None),
            ("Node admission, gossip, proposer, restore and serving bounds", [
                ["test", "--locked", "-p", "iroha_core", "--lib", "--",
                 "resource_coherence::",
                 "state::deserialize::state_snapshot_decode_error_tests::resource_restore_",
                 "gossiper::tests::shipped_gossip_frames_carry_the_maximum_admitted_signed_transaction",
                 "gossiper::tests::ordinary_gossip_exact_frame_boundary_preserves_original_body_on_requeue",
                 "queue::tests::default_limits_admit_and_select_the_largest_includable_transaction",
                 "queue::tests::transaction_cap_is_exact_at_admission_and_block_validation",
                 "queue::tests::queue_admission_and_proposer_selection_share_the_includable_bound",
                 "sumeragi::payload::resource_contract_tests::",
                 "sumeragi::executor::publication_tests::global_build_carries_a_transaction_of_the_payload_limit_less_the_reserve",
                 "sumeragi::driver::serve::tests::admitted_maximum_frames_fit_frame_overhead_and_the_block_sync_frame",
                 "sumeragi::driver::serve::tests::generic_maximum_control_frames_fit_the_control_topic",
                 "privacy::tests::privacy_block_counts_and_actual_aggregate_bytes_are_independent"],
            ], "1"),
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            recorder = root / "cargo"
            recorder.write_text(
                "#!" + sys.executable + "\nimport json, os, sys\n"
                "print(json.dumps({'argv': sys.argv[1:], 'jobs': os.environ.get('CARGO_BUILD_JOBS')}))\n"
            )
            recorder.chmod(0o700)
            for name, expected_commands, expected_jobs in cases:
                with self.subTest(step=name):
                    step = re.search(r"(?ms)^      - name: " + re.escape(name) + r"\n(.*?)(?=^      - |\Z)", job)
                    self.assertIsNotNone(step)
                    self.assertNotRegex(step.group(1), r"(?m)^        (if|continue-on-error):")
                    run = re.search(r"(?m)^        run: (.+)$", step.group(1))
                    self.assertIsNotNone(run)
                    command = run.group(1)
                    if command == "|":
                        command = textwrap.dedent(step.group(1)[run.end() + 1:])
                    jobs = re.search(r'(?m)^          CARGO_BUILD_JOBS: "([0-9]+)"$', step.group(1))
                    self.assertEqual(jobs.group(1) if jobs else None, expected_jobs)
                    environment = dict(os.environ)
                    environment.pop("CARGO_BUILD_JOBS", None)
                    if jobs:
                        environment["CARGO_BUILD_JOBS"] = jobs.group(1)
                    environment["PATH"] = str(root) + os.pathsep + environment.get("PATH", "")
                    result = subprocess.run(["/bin/sh", "-e", "-c", command], cwd=root,
                                            env=environment, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual([json.loads(line) for line in result.stdout.splitlines()],
                                     [{"argv": argv, "jobs": expected_jobs} for argv in expected_commands])
        before = job.index("      - name: Record X2 node compiler memory envelope\n")
        core = job.index("      - name: " + cases[-1][0] + "\n")
        after = job.index("      - name: Record X2 node compiler memory result\n        if: always()\n")
        self.assertLess(before, core)
        self.assertLess(core, after)


if __name__ == "__main__":
    unittest.main()
