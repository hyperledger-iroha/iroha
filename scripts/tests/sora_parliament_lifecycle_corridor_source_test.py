#!/usr/bin/env python3
"""Source contract for the isolated four-validator Parliament PR corridor."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "ci/check_sora_parliament_lifecycle.sh"
WORKFLOW = ROOT / ".github/workflows/pr.yml"
MANIFEST = ROOT / "integration_tests/Cargo.toml"
CORRIDOR = ROOT / "integration_tests/tests/sora_parliament_lifecycle_smoke.rs"


class ContractError(AssertionError):
    """Raised when the source-only corridor contract is incomplete."""


def require(condition: bool, message: str) -> None:
    """Fail the source contract with one stable diagnostic."""

    if not condition:
        raise ContractError(message)


def validate_runner(source: str) -> None:
    """Validate the exact debug prebuild and no-skip execution corridor."""

    required = (
        "cargo build --locked",
        "--features test-network-parliament-signers",
        "TEST_NETWORK_BIN_IROHAD_PARLIAMENT_SIGNERS",
        "IROHA_TEST_SKIP_BUILD=1",
        "IROHA_TEST_REQUIRE_NETWORK=1",
        "IROHA_FAIL_ON_SANDBOX_SKIP=1",
        "cargo test --locked",
        "--features parliament-test-signers",
        "--test sora_parliament_lifecycle_smoke",
        "--test-threads=1",
    )
    for marker in required:
        require(marker in source, f"Parliament runner lost `{marker}`")
    require("--release" not in source, "test signer runner must remain debug-only")
    require(source.count("cargo build ") == 1, "runner needs one exact daemon prebuild")
    require(source.count("cargo test ") == 1, "runner needs one exact test invocation")
    require(
        source.index("cargo build ")
        < source.index("TEST_NETWORK_BIN_IROHAD_PARLIAMENT_SIGNERS")
        < source.index("cargo test "),
        "runner must prebuild, pin the exact daemon, then run the test",
    )


def validate_workflow(source: str) -> None:
    """Validate that PR CI schedules the isolated runner on four-peer capacity."""

    job = re.search(
        r"(?ms)^  sora_parliament_lifecycle:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
        source,
    )
    require(job is not None, "PR workflow does not schedule the Parliament corridor")
    body = job.group("body")
    for marker in (
        "runs-on: [self-hosted, Linux, iroha2]",
        "CARGO_TARGET_DIR: target/sora-parliament-lifecycle",
        "python3 -I -S scripts/tests/sora_parliament_lifecycle_corridor_source_test.py",
        "bash ci/check_sora_parliament_lifecycle.sh",
    ):
        require(marker in body, f"Parliament PR job lost `{marker}`")
    require("--release" not in body, "Parliament PR job must not consume release binaries")


class SoraParliamentLifecycleCorridorSourceTests(unittest.TestCase):
    """Mutation-resistant source checks for the dedicated network corridor."""

    def test_runner_and_workflow_are_exact_and_no_skip(self) -> None:
        validate_runner(RUNNER.read_text(encoding="utf-8"))
        validate_workflow(WORKFLOW.read_text(encoding="utf-8"))

    def test_runner_contract_rejects_each_removed_security_marker(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        for marker in (
            "cargo build --locked",
            "--features test-network-parliament-signers",
            "TEST_NETWORK_BIN_IROHAD_PARLIAMENT_SIGNERS",
            "IROHA_TEST_SKIP_BUILD=1",
            "IROHA_TEST_REQUIRE_NETWORK=1",
            "IROHA_FAIL_ON_SANDBOX_SKIP=1",
            "cargo test --locked",
            "--features parliament-test-signers",
            "--test sora_parliament_lifecycle_smoke",
            "--test-threads=1",
        ):
            with self.subTest(marker=marker), self.assertRaises(ContractError):
                validate_runner(source.replace(marker, "", 1))

    def test_target_and_boundary_guards_remain_feature_isolated(self) -> None:
        manifest = MANIFEST.read_text(encoding="utf-8")
        target = re.search(
            r'(?ms)^\[\[test\]\]\nname = "sora_parliament_lifecycle_smoke"\n'
            r'path = "tests/sora_parliament_lifecycle_smoke.rs"\n'
            r'required-features = \["parliament-test-signers"\]$',
            manifest,
        )
        require(target is not None, "Parliament test target lost its opt-in feature")

        corridor = CORRIDOR.read_text(encoding="utf-8")
        helper_calls = corridor.count("assert_transition_rejected_without_state_change(")
        require(
            helper_calls == 10,
            "corridor must retain the helper plus nine boundary/replay checks",
        )
        for retired in (
            "CastPlainBallot",
            "FinalizeReferendum",
            "EnactReferendum",
            "ConstructCertificate",
            "MarkEnacted",
        ):
            require(retired not in corridor, f"corridor regained retired `{retired}`")


if __name__ == "__main__":
    unittest.main()
