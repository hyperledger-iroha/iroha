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
USER_CONFIG = ROOT / "crates/iroha_config/src/parameters/user.rs"
ACTUAL_CONFIG = ROOT / "crates/iroha_config/src/parameters/actual.rs"
TEST_NETWORK = ROOT / "crates/iroha_test_network/src/lib.rs"
DAEMON = ROOT / "crates/irohad/src/main.rs"
BEACON = ROOT / "crates/iroha_core/src/beacon.rs"
BEACON_LIFECYCLE = ROOT / "crates/iroha_core/src/sumeragi/v2_beacon.rs"
MANDATORY_NPOS_TEST_NAME = (
    "four_validator_mandatory_npos_epoch_boundary_threshold_beacon_release_gate"
)
FAIL_CLOSED_NPOS_TEST_NAME = (
    "four_validator_mandatory_npos_beacon_fails_closed_below_threshold"
)
PARLIAMENT_LIFECYCLE_TEST_NAME = (
    "four_validator_policy_jury_uses_future_pulses_and_mandatory_timed_ovn"
)
PARLIAMENT_NETWORK_TEST_ATTRIBUTE = "#[test]"
PARLIAMENT_LIFECYCLE_TEST = re.compile(
    rf"(?ms)^{re.escape(PARLIAMENT_NETWORK_TEST_ATTRIBUTE)}\n"
    rf"fn {PARLIAMENT_LIFECYCLE_TEST_NAME}\(\) -> Result<\(\)> \{{\n"
    r".*?^\}\n"
    rf"\nasync fn {PARLIAMENT_LIFECYCLE_TEST_NAME}_impl\(\)\s*"
    r"-> Result<\(\)>\s*\{\n"
    r".*?^\}\n"
    rf"(?=\n{re.escape(PARLIAMENT_NETWORK_TEST_ATTRIBUTE)}\n"
    rf"fn {MANDATORY_NPOS_TEST_NAME}\(\))"
)
MANDATORY_NPOS_TEST = re.compile(
    rf"(?ms)^{re.escape(PARLIAMENT_NETWORK_TEST_ATTRIBUTE)}\n"
    rf"fn {MANDATORY_NPOS_TEST_NAME}\(\) -> Result<\(\)> \{{\n"
    r".*?^\}\n"
    rf"\nasync fn {MANDATORY_NPOS_TEST_NAME}_impl\(\)\s*"
    r"-> Result<\(\)>\s*\{\n.*?^\}\n"
    rf"(?=\n{re.escape(PARLIAMENT_NETWORK_TEST_ATTRIBUTE)}\n"
    rf"fn {FAIL_CLOSED_NPOS_TEST_NAME}\(\))"
)
FAIL_CLOSED_NPOS_TEST = re.compile(
    rf"(?ms)^{re.escape(PARLIAMENT_NETWORK_TEST_ATTRIBUTE)}\n"
    rf"fn {FAIL_CLOSED_NPOS_TEST_NAME}\(\) -> Result<\(\)> \{{\n"
    r".*?^\}\n"
    rf"\nasync fn {FAIL_CLOSED_NPOS_TEST_NAME}_impl\(\)\s*"
    r"-> Result<\(\)>\s*\{\n"
    r".*?^\}\n"
    r"(?=\n#\[test\]\nfn parliament_network_corridor_has_no_legacy_or_consensus_bypass_surface\(\))"
)
AUTONOMOUS_SORTITION_PULSE_PROGRESSION = '''network.ensure_blocks(sortition_pulse_height).await?;
    assert_eq!(
        current_height(&client)?,
        sortition_pulse_height,
        "the demanded sortition threshold-beacon effect must autonomously finalize its exact height",
    );'''
AUTONOMOUS_BALLOT_RELEASE_PULSE_PROGRESSION = '''network.ensure_blocks(release_height).await?;
    assert_eq!(
        current_height(&client)?,
        release_height,
        "the demanded ballot-release threshold-beacon effect must autonomously finalize its exact height",
    );'''
AUTONOMOUS_PULSE_PROGRESSION = '''network.ensure_blocks(pulse_height).await?;
    assert_eq!(
        current_height(&client)?,
        pulse_height,
        "the mandatory threshold-beacon effect must autonomously finalize its exact pre-boundary height",
    );'''
BOUNDARY_PROGRESSION = '''assert_eq!(
        tick(&client, "commit mandatory NPoS boundary")?,
        boundary_height
    );'''
SUCCESSOR_PROGRESSION = '''assert_eq!(
        tick(&client, "prove successor epoch can finalize")?,
        boundary_height + 1
    );
    network.ensure_blocks(boundary_height + 1).await?;'''
SUCCESSOR_SEED_EQUALITY = (
    "assert_eq!(status.height_context.epoch_seed, successor_seed);"
)
POSITIVE_BEACON_MODES = """constPOSITIVE_BEACON_SIGNER_MODES:[ParliamentBeaconSignerMode;VALIDATOR_COUNT]=[ParliamentBeaconSignerMode::Valid,ParliamentBeaconSignerMode::Valid,ParliamentBeaconSignerMode::Absent,ParliamentBeaconSignerMode::Invalid,];"""
FAIL_CLOSED_BEACON_MODES = """constFAIL_CLOSED_BEACON_SIGNER_MODES:[ParliamentBeaconSignerMode;VALIDATOR_COUNT]=[ParliamentBeaconSignerMode::Valid,ParliamentBeaconSignerMode::Absent,ParliamentBeaconSignerMode::Absent,ParliamentBeaconSignerMode::Invalid,];"""
FAIL_CLOSED_TIMEOUT = """letunexpected_pulse_height=tokio::time::timeout(FAIL_CLOSED_BEACON_OBSERVATION_WINDOW,network.peers()[0].once_block(pulse_height),).await;"""
SORANET_POW_CORRIDOR_MARKERS = (
    '.write(["network","soranet_handshake","pow","puzzle","memory_kib",],i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),)',
    '.write(["network","soranet_handshake","pow","puzzle","time_cost"],1_i64,)',
    '.write(["network","soranet_handshake","pow","puzzle","lanes"],1_i64,)',
)
SORANET_POW_REQUIRED_OVERRIDE = (
    '["network","soranet_handshake","pow","required"]'
)


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


def parliament_lifecycle_test(source: str) -> tuple[re.Match[str], str]:
    """Return the one exact executable Parliament lifecycle test item."""

    matches = list(PARLIAMENT_LIFECYCLE_TEST.finditer(source))
    require(len(matches) == 1, "Parliament lifecycle corridor is not one exact test item")
    match = matches[0]
    previous_item_end = source.rfind("\n}\n", 0, match.start())
    require(previous_item_end >= 0, "Parliament lifecycle test has no preceding item boundary")
    leading = source[previous_item_end + len("\n}\n") : match.start()]
    require(
        "#[" not in leading,
        "Parliament lifecycle corridor gained an extra attribute",
    )
    return match, match.group(0)


def mutate_parliament_lifecycle_test(source: str, old: str, new: str = "") -> str:
    """Apply one exact mutation only inside the Parliament lifecycle test item."""

    match, test = parliament_lifecycle_test(source)
    require(old in test, f"Parliament lifecycle mutation target is absent: `{old}`")
    mutated = test.replace(old, new, 1)
    return source[: match.start()] + mutated + source[match.end() :]


def validate_optional_parliament_pulse_progression(source: str) -> None:
    """Require autonomous finalization for both demanded optional pulses."""

    _, test = parliament_lifecycle_test(source)
    for marker in (
        AUTONOMOUS_SORTITION_PULSE_PROGRESSION,
        AUTONOMOUS_BALLOT_RELEASE_PULSE_PROGRESSION,
    ):
        require(marker in test, f"Parliament lifecycle test lost `{marker}`")
    for retired_tick in (
        'tick(&client, "finalize the demanded sortition pulse")',
        'tick(&client, "finalize the demanded ballot-release pulse")',
    ):
        require(
            retired_tick not in test,
            f"demanded Parliament pulse regained racing tick `{retired_tick}`",
        )
    for marker in (
        "!status.restart_required",
        "!restarted_status.restart_required",
    ):
        require(
            marker in test,
            f"Parliament lifecycle test lost non-fail-stopped status proof `{marker}`",
        )


def validate_public_finding_impossible_quorum_retry(source: str) -> None:
    """Require the real four-peer early-NoResult and governance-retry corridor."""

    _, test = parliament_lifecycle_test(source)
    require(
        "exercise_public_finding_impossible_quorum_retry(" in test,
        "Parliament lifecycle test lost the impossible-quorum retry invocation",
    )
    helper = re.search(
        r"(?ms)^async fn exercise_public_finding_impossible_quorum_retry\(.*?^\}\n"
        rf"(?=\n{re.escape(PARLIAMENT_NETWORK_TEST_ATTRIBUTE)}\n"
        rf"fn {PARLIAMENT_LIFECYCLE_TEST_NAME}\(\))",
        source,
    )
    require(helper is not None, "impossible-quorum retry helper is absent")
    helper_source = helper.group(0)
    for marker in (
        "network.ensure_blocks(sortition_pulse_height).await?;",
        "ParliamentLifecycleTransitionV1::RecordAttemptAbsence(",
        "failed_height < public_finding_deadline",
        "BodyInstanceStatusV1::NoResult",
        "ParliamentNoResultKindV1::PublicFindingQuorumUnreachable",
        "client.get_gov_contract_json(contract_address).is_err()",
        "attempt_sequence: 1",
        "GovernanceStageV1::Qualification",
        "peer_rejected.state_payload_hex",
        "peer_retry.state_payload_hex",
    ):
        require(marker in helper_source, f"impossible-quorum retry corridor lost `{marker}`")


def mandatory_npos_test(source: str) -> tuple[re.Match[str], str]:
    """Return the one exact executable mandatory-NPoS test item."""

    matches = list(MANDATORY_NPOS_TEST.finditer(source))
    require(len(matches) == 1, "mandatory NPoS beacon corridor is not one exact test item")
    match = matches[0]
    previous_item_end = source.rfind("\n}\n", 0, match.start())
    require(previous_item_end >= 0, "mandatory NPoS test has no preceding item boundary")
    leading = source[previous_item_end + len("\n}\n") : match.start()]
    require(
        "#[" not in leading,
        "mandatory NPoS beacon corridor gained an extra attribute",
    )
    return match, match.group(0)


def mutate_mandatory_npos_test(source: str, old: str, new: str = "") -> str:
    """Apply one exact mutation only inside the mandatory-NPoS test item."""

    match, test = mandatory_npos_test(source)
    require(old in test, f"mandatory NPoS mutation target is absent: `{old}`")
    mutated = test.replace(old, new, 1)
    return source[: match.start()] + mutated + source[match.end() :]


def fail_closed_npos_test(source: str) -> tuple[re.Match[str], str]:
    """Return the one exact executable below-threshold NPoS test item."""

    matches = list(FAIL_CLOSED_NPOS_TEST.finditer(source))
    require(len(matches) == 1, "fail-closed NPoS beacon corridor is not one exact test item")
    match = matches[0]
    previous_item_end = source.rfind("\n}\n", 0, match.start())
    require(previous_item_end >= 0, "fail-closed NPoS test has no preceding item boundary")
    leading = source[previous_item_end + len("\n}\n") : match.start()]
    require(
        "#[" not in leading,
        "fail-closed NPoS beacon corridor gained an extra attribute",
    )
    return match, match.group(0)


def mutate_fail_closed_npos_test(source: str, old: str, new: str = "") -> str:
    """Apply one exact mutation only inside the fail-closed NPoS test item."""

    match, test = fail_closed_npos_test(source)
    require(old in test, f"fail-closed NPoS mutation target is absent: `{old}`")
    mutated = test.replace(old, new, 1)
    return source[: match.start()] + mutated + source[match.end() :]


def compact(source: str) -> str:
    """Remove formatting whitespace while retaining exact Rust tokens."""

    return re.sub(r"\s+", "", source)


def validate_required_bounded_soranet_pow(source: str) -> None:
    """Require mandatory, supported-cost SoraNet admission in every corridor."""

    tests = (
        ("Parliament lifecycle", parliament_lifecycle_test(source)[1]),
        ("mandatory NPoS", mandatory_npos_test(source)[1]),
        ("fail-closed NPoS", fail_closed_npos_test(source)[1]),
    )
    compacted_source = compact(source)
    require(
        SORANET_POW_REQUIRED_OVERRIDE not in compacted_source,
        "corridor must not override the hard-required SoraNet PoW admission field",
    )
    actual_config = ACTUAL_CONFIG.read_text(encoding="utf-8")
    pow_shape = re.search(r"pub struct SoranetPow \{(?P<body>.*?)^\}", actual_config, re.M | re.S)
    require(pow_shape is not None, "actual SoraNet PoW configuration shape is missing")
    require(
        "required" not in pow_shape.group("body"),
        "SoraNet PoW regained a runtime required/optional toggle",
    )
    require(
        "actual::SoranetPow{difficulty,max_future_skew,min_ticket_ttl,ticket_ttl,"
        in compact(USER_CONFIG.read_text(encoding="utf-8")),
        "user configuration no longer constructs the mandatory SoraNet PoW policy",
    )
    for marker in SORANET_POW_CORRIDOR_MARKERS:
        require(
            compacted_source.count(marker) == len(tests),
            f"SoraNet corridor marker is not present exactly once per builder: `{marker}`",
        )
    for label, test in tests:
        compacted_test = compact(test)
        for marker in SORANET_POW_CORRIDOR_MARKERS:
            require(
                compacted_test.count(marker) == 1,
                f"{label} corridor lost exact mandatory bounded SoraNet PoW marker `{marker}`",
            )


def validate_consensus_sized_test_stacks(source: str) -> None:
    """Require every four-validator corridor to run on the bounded large-stack harness."""

    require(
        source.count("const PARLIAMENT_NETWORK_STACK_BYTES: usize = 32 * 1024 * 1024;")
        == 1,
        "Parliament network stack size is not one exact 32 MiB constant",
    )
    tests = (
        (
            PARLIAMENT_LIFECYCLE_TEST_NAME,
            parliament_lifecycle_test(source)[1],
        ),
        (MANDATORY_NPOS_TEST_NAME, mandatory_npos_test(source)[1]),
        (FAIL_CLOSED_NPOS_TEST_NAME, fail_closed_npos_test(source)[1]),
    )
    for name, test in tests:
        compacted = compact(test)
        for marker in (
            "std::thread::Builder::new()",
            ".stack_size(PARLIAMENT_NETWORK_STACK_BYTES)",
            "tokio::runtime::Builder::new_multi_thread()",
            ".worker_threads(4)",
            ".thread_stack_size(PARLIAMENT_NETWORK_STACK_BYTES)",
        ):
            require(
                compacted.count(marker) == 1,
                f"{name} lost exact consensus-sized stack marker `{marker}`",
            )
        require(
            compacted.count(f"{name}_impl()") == 2,
            f"{name} wrapper no longer invokes its exact async implementation",
        )


def validate_mandatory_npos_boundary(source: str) -> None:
    """Require the exact executable four-validator pre-boundary beacon corridor."""

    _, test = mandatory_npos_test(source)
    require(
        source.count("const MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS: u64 = 8;") == 1,
        "mandatory NPoS epoch length is not one exact eight-block constant",
    )
    require(
        source.count("const VALIDATOR_COUNT: usize = 4;") == 1,
        "Parliament corridor validator count is not one exact four-peer constant",
    )
    required = (
        "SumeragiNposParameters::default()",
        ".with_peers(VALIDATOR_COUNT)",
        ".with_npos_consensus()",
        ".with_parliament_beacon_signer_modes(POSITIVE_BEACON_SIGNER_MODES)",
        "assert_eq!(network.peers().len(), VALIDATOR_COUNT);",
        "assert_eq!(beacon_record.session.committee_size, 4);",
        "assert_eq!(beacon_record.session.threshold, 2);",
        "let boundary_height = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS;",
        "pulse_height = boundary_height - 1",
        AUTONOMOUS_PULSE_PROGRESSION,
        "verify_finalized_global_threshold_beacon_pulse_v1(",
        "let successor_epoch = 1;",
        "global_threshold_beacon_npos_successor_seed_v1(",
        BOUNDARY_PROGRESSION,
        SUCCESSOR_PROGRESSION,
        "assert_eq!(status.height_context.epoch, successor_epoch);",
        SUCCESSOR_SEED_EQUALITY,
        "!status.restart_required",
    )
    for marker in required:
        require(marker in test, f"mandatory NPoS beacon test lost `{marker}`")
    require(
        ".with_permissioned_consensus()" not in test,
        "mandatory NPoS beacon test selected permissioned consensus",
    )
    require(
        'tick(&client, "commit mandatory pre-boundary pulse")' not in test,
        "mandatory pre-boundary pulse must not race a user transaction",
    )


def validate_beacon_mode_profiles(source: str) -> None:
    """Freeze exact 4-seat/2-threshold positive and fail-closed mode profiles."""

    compacted = compact(source)
    require(
        compacted.count(POSITIVE_BEACON_MODES) == 1,
        "positive beacon mode profile is not exact valid/valid/absent/invalid",
    )
    require(
        compacted.count(FAIL_CLOSED_BEACON_MODES) == 1,
        "fail-closed beacon mode profile is not exact valid/absent/absent/invalid",
    )
    _, parliament = parliament_lifecycle_test(source)
    require(
        ".with_parliament_beacon_signer_modes(POSITIVE_BEACON_SIGNER_MODES)"
        in parliament,
        "optional Parliament pulses lost the positive fault profile",
    )


def validate_fail_closed_npos_boundary(source: str) -> None:
    """Require four live validators to stall exactly below a mandatory pulse."""

    _, test = fail_closed_npos_test(source)
    required = (
        "SumeragiNposParameters::default()",
        ".with_peers(VALIDATOR_COUNT)",
        ".with_npos_consensus()",
        ".with_parliament_beacon_signer_modes(FAIL_CLOSED_BEACON_SIGNER_MODES)",
        "assert_eq!(network.peers().len(), VALIDATOR_COUNT);",
        "assert_eq!(beacon_record.session.committee_size, 4);",
        "assert_eq!(beacon_record.session.threshold, 2);",
        "filter(|mode| **mode == ParliamentBeaconSignerMode::Valid)",
        "let pulse_height = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS - 1;",
        "let predecessor_height = pulse_height - 1;",
        "unexpected_pulse_height.is_err()",
        "peer.is_running()",
        "!status.restart_required",
        "assert_eq!(status.last_committed_height, predecessor_height);",
        "assert_eq!(status.height, pulse_height);",
    )
    for marker in required:
        require(marker in test, f"fail-closed NPoS beacon test lost `{marker}`")
    require(
        FAIL_CLOSED_TIMEOUT in compact(test),
        "fail-closed NPoS beacon test lost its bounded no-block observation",
    )
    require(
        ".with_permissioned_consensus()" not in test,
        "fail-closed beacon test selected permissioned consensus",
    )
    require(
        ".with_parliament_test_signers()" not in test,
        "fail-closed beacon test replaced the exact per-peer fault profile",
    )
    require(
        "network.shutdown().await;" in test,
        "fail-closed beacon test must shut down all still-live validators",
    )


def validate_feature_only_fault_wiring(
    test_network: str,
    daemon: str,
    beacon: str,
    lifecycle: str,
) -> None:
    """Pin the hidden child arg and receiver-side invalid-share corridor."""

    for marker in (
        "pub enum ParliamentBeaconSignerMode",
        "with_parliament_beacon_signer_modes",
        '"--test-network-parliament-beacon-signer-mode"',
        "append_parliament_beacon_signer_mode_arg",
    ):
        require(marker in test_network, f"test-network signer wiring lost `{marker}`")
    for marker in (
        '#[cfg(feature = "test-network-parliament-signers")]\n#[derive(',
        "enum TestNetworkParliamentBeaconSignerMode",
        'long = "test-network-parliament-beacon-signer-mode"',
        "hide = true",
        "TestNetworkParliamentBeaconSignerMode::Absent => None",
        "with_deliberately_invalid_outbound()",
    ):
        require(marker in daemon, f"feature-only daemon signer wiring lost `{marker}`")
    require(
        "test_network_emit_invalid_outbound_partial_v1" in beacon,
        "beacon signer trait lost the feature-only outbound hook",
    )
    for marker in (
        '#[cfg(feature = "test-network-parliament-signers")]',
        "test_network_emit_invalid_outbound_partial_v1()",
        "partial.signature_share[0] ^= 1;",
        "let _ = active.aggregator.accept_partial(partial)?;",
    ):
        require(marker in lifecycle, f"feature-only beacon lifecycle lost `{marker}`")


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
        validate_required_bounded_soranet_pow(corridor)
        validate_consensus_sized_test_stacks(corridor)
        validate_optional_parliament_pulse_progression(corridor)
        validate_public_finding_impossible_quorum_retry(corridor)
        validate_beacon_mode_profiles(corridor)
        validate_mandatory_npos_boundary(corridor)
        validate_fail_closed_npos_boundary(corridor)
        validate_feature_only_fault_wiring(
            TEST_NETWORK.read_text(encoding="utf-8"),
            DAEMON.read_text(encoding="utf-8"),
            BEACON.read_text(encoding="utf-8"),
            BEACON_LIFECYCLE.read_text(encoding="utf-8"),
        )

    def test_required_bounded_soranet_pow_rejects_adversarial_mutations(self) -> None:
        corridor = CORRIDOR.read_text(encoding="utf-8")
        mutations = {
            "lifecycle PoW override introduced": mutate_parliament_lifecycle_test(
                corridor,
                ".with_config_layer(|layer| {",
                '''.with_config_layer(|layer| {
            layer.write(
                ["network", "soranet_handshake", "pow", "required"],
                false,
            );''',
            ),
            "lifecycle memory cost raised": mutate_parliament_lifecycle_test(
                corridor,
                "i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB)",
                "64_i64 * 1024",
            ),
            "mandatory NPoS time cost raised": mutate_mandatory_npos_test(
                corridor, '"time_cost"', '"time_cost_mutated"'
            ),
            "fail-closed NPoS lanes raised": mutate_fail_closed_npos_test(
                corridor, '"lanes"', '"lanes_mutated"'
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_required_bounded_soranet_pow(mutated)

    def test_consensus_sized_test_stacks_reject_adversarial_mutations(self) -> None:
        corridor = CORRIDOR.read_text(encoding="utf-8")
        mutations = {
            "lifecycle caller stack removed": mutate_parliament_lifecycle_test(
                corridor, ".stack_size(PARLIAMENT_NETWORK_STACK_BYTES)"
            ),
            "mandatory worker stack removed": mutate_mandatory_npos_test(
                corridor, ".thread_stack_size(PARLIAMENT_NETWORK_STACK_BYTES)"
            ),
            "fail-closed wrapper disconnected": mutate_fail_closed_npos_test(
                corridor,
                f"{FAIL_CLOSED_NPOS_TEST_NAME}_impl()",
                f"{FAIL_CLOSED_NPOS_TEST_NAME}_disabled()",
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_consensus_sized_test_stacks(mutated)

    def test_optional_pulse_progression_rejects_adversarial_mutations(self) -> None:
        corridor = CORRIDOR.read_text(encoding="utf-8")
        mutations = {
            "missing sortition autonomous progression": mutate_parliament_lifecycle_test(
                corridor, AUTONOMOUS_SORTITION_PULSE_PROGRESSION
            ),
            "sortition pulse transaction race": mutate_parliament_lifecycle_test(
                corridor,
                AUTONOMOUS_SORTITION_PULSE_PROGRESSION,
                '''assert_eq!(
        tick(&client, "finalize the demanded sortition pulse")?,
        sortition_pulse_height,
    );
    network.ensure_blocks(sortition_pulse_height).await?;''',
            ),
            "missing ballot-release autonomous progression": mutate_parliament_lifecycle_test(
                corridor, AUTONOMOUS_BALLOT_RELEASE_PULSE_PROGRESSION
            ),
            "ballot-release pulse transaction race": mutate_parliament_lifecycle_test(
                corridor,
                AUTONOMOUS_BALLOT_RELEASE_PULSE_PROGRESSION,
                '''assert_eq!(
        tick(&client, "finalize the demanded ballot-release pulse")?,
        release_height,
    );
    network.ensure_blocks(release_height).await?;''',
            ),
            "enacted peer fail-stop status omitted": mutate_parliament_lifecycle_test(
                corridor, "!status.restart_required", "true"
            ),
            "restarted peer fail-stop status omitted": mutate_parliament_lifecycle_test(
                corridor, "!restarted_status.restart_required", "true"
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_optional_parliament_pulse_progression(mutated)

    def test_public_finding_impossible_quorum_retry_rejects_mutations(self) -> None:
        corridor = CORRIDOR.read_text(encoding="utf-8")
        for marker in (
            "exercise_public_finding_impossible_quorum_retry(",
            "failed_height < public_finding_deadline",
            "ParliamentNoResultKindV1::PublicFindingQuorumUnreachable",
            "attempt_sequence: 1",
            "peer_retry.state_payload_hex",
        ):
            with self.subTest(marker=marker), self.assertRaises(ContractError):
                validate_public_finding_impossible_quorum_retry(
                    corridor.replace(marker, "", 1)
                )

    def test_mandatory_npos_boundary_rejects_adversarial_mutations(self) -> None:
        corridor = CORRIDOR.read_text(encoding="utf-8")
        mutations = {
            "renamed test": mutate_mandatory_npos_test(
                corridor, MANDATORY_NPOS_TEST_NAME, f"{MANDATORY_NPOS_TEST_NAME}_disabled"
            ),
            "ignored test": mutate_mandatory_npos_test(
                corridor,
                PARLIAMENT_NETWORK_TEST_ATTRIBUTE,
                f"#[ignore]\n{PARLIAMENT_NETWORK_TEST_ATTRIBUTE}",
            ),
            "permissioned consensus": mutate_mandatory_npos_test(
                corridor, ".with_npos_consensus()", ".with_permissioned_consensus()"
            ),
            "one peer": mutate_mandatory_npos_test(
                corridor, ".with_peers(VALIDATOR_COUNT)", ".with_peers(1)"
            ),
            "one-peer validator constant": corridor.replace(
                "const VALIDATOR_COUNT: usize = 4;",
                "const VALIDATOR_COUNT: usize = 1;",
                1,
            ),
            "missing epoch constant": corridor.replace(
                "const MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS: u64 = 8;", "", 1
            ),
            "non-autonomous pulse": mutate_mandatory_npos_test(
                corridor,
                "network.ensure_blocks(pulse_height).await?;",
                'tick(&client, "commit mandatory pre-boundary pulse")?;',
            ),
            "missing autonomous-height equality": mutate_mandatory_npos_test(
                corridor, AUTONOMOUS_PULSE_PROGRESSION
            ),
            "missing boundary progression": mutate_mandatory_npos_test(
                corridor, BOUNDARY_PROGRESSION
            ),
            "missing successor progression": mutate_mandatory_npos_test(
                corridor, SUCCESSOR_PROGRESSION
            ),
            "seed read without equality": mutate_mandatory_npos_test(
                corridor,
                SUCCESSOR_SEED_EQUALITY,
                "let _ = (status.height_context.epoch_seed, successor_seed);",
            ),
            "missing pulse verifier": mutate_mandatory_npos_test(
                corridor, "verify_finalized_global_threshold_beacon_pulse_v1("
            ),
            "missing committee assertion": mutate_mandatory_npos_test(
                corridor, "assert_eq!(beacon_record.session.committee_size, 4);"
            ),
            "missing threshold assertion": mutate_mandatory_npos_test(
                corridor, "assert_eq!(beacon_record.session.threshold, 2);"
            ),
            "successor fail-stop status omitted": mutate_mandatory_npos_test(
                corridor, "!status.restart_required", "true"
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_mandatory_npos_boundary(mutated)

    def test_beacon_mode_profiles_reject_adversarial_mutations(self) -> None:
        corridor = CORRIDOR.read_text(encoding="utf-8")
        mutations = {
            "positive invalid becomes valid": corridor.replace(
                "const POSITIVE_BEACON_SIGNER_MODES",
                "const MUTATED_POSITIVE_BEACON_SIGNER_MODES",
                1,
            ),
            "fail-closed second absent becomes valid": corridor.replace(
                "const FAIL_CLOSED_BEACON_SIGNER_MODES",
                "const MUTATED_FAIL_CLOSED_BEACON_SIGNER_MODES",
                1,
            ),
            "optional lifecycle returns to all-valid shorthand": mutate_parliament_lifecycle_test(
                corridor,
                ".with_parliament_beacon_signer_modes(POSITIVE_BEACON_SIGNER_MODES)",
                ".with_parliament_test_signers()",
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_beacon_mode_profiles(mutated)

    def test_fail_closed_npos_boundary_rejects_adversarial_mutations(self) -> None:
        corridor = CORRIDOR.read_text(encoding="utf-8")
        mutations = {
            "renamed test": mutate_fail_closed_npos_test(
                corridor,
                FAIL_CLOSED_NPOS_TEST_NAME,
                f"{FAIL_CLOSED_NPOS_TEST_NAME}_disabled",
            ),
            "ignored test": mutate_fail_closed_npos_test(
                corridor,
                PARLIAMENT_NETWORK_TEST_ATTRIBUTE,
                f"#[ignore]\n{PARLIAMENT_NETWORK_TEST_ATTRIBUTE}",
            ),
            "permissioned consensus": mutate_fail_closed_npos_test(
                corridor,
                ".with_npos_consensus()",
                ".with_permissioned_consensus()",
            ),
            "one peer": mutate_fail_closed_npos_test(
                corridor,
                ".with_peers(VALIDATOR_COUNT)",
                ".with_peers(1)",
            ),
            "all-valid shorthand": mutate_fail_closed_npos_test(
                corridor,
                ".with_parliament_beacon_signer_modes(FAIL_CLOSED_BEACON_SIGNER_MODES)",
                ".with_parliament_test_signers()",
            ),
            "threshold weakened": mutate_fail_closed_npos_test(
                corridor,
                "assert_eq!(beacon_record.session.threshold, 2);",
                "assert_eq!(beacon_record.session.threshold, 1);",
            ),
            "unbounded block wait": mutate_fail_closed_npos_test(
                corridor,
                "let unexpected_pulse_height = tokio::time::timeout(",
                "let unexpected_pulse_height = passthrough(",
            ),
            "validator liveness omitted": mutate_fail_closed_npos_test(
                corridor,
                "peer.is_running()",
                "true",
            ),
            "validator fail-stop status omitted": mutate_fail_closed_npos_test(
                corridor,
                "!status.restart_required",
                "true",
            ),
            "stalled height omitted": mutate_fail_closed_npos_test(
                corridor,
                "assert_eq!(status.last_committed_height, predecessor_height);",
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_fail_closed_npos_boundary(mutated)


if __name__ == "__main__":
    unittest.main()
