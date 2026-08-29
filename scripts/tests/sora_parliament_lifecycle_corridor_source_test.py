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
NO_RESULT_PATHS = ROOT / "integration_tests/tests/sora_parliament_no_result_paths.rs"
FAILURE_PATHS = ROOT / "integration_tests/tests/sora_parliament_failure_paths.rs"
USER_CONFIG = ROOT / "crates/iroha_config/src/parameters/user.rs"
ACTUAL_CONFIG = ROOT / "crates/iroha_config/src/parameters/actual.rs"
TEST_NETWORK = ROOT / "crates/iroha_test_network/src/lib.rs"
DAEMON = ROOT / "crates/irohad/src/main.rs"
BEACON = ROOT / "crates/iroha_core/src/beacon.rs"
BEACON_TEST_SIGNER = (
    ROOT / "crates/iroha_core/src/beacon/parliament_test_network_signer.rs"
)
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
EXACT_SUPERSEDING_HEAD_ASSERTION = '''assert_eq!(
        superseded.superseding_head(),
        Some(GovernanceExpectedHeadV1::Present(
            GovernanceExpectedHeadPresentV1 {
                subject_id: deploy_subject_id,
                version: 1,
                head_root: competing_contract_code_hash.into(),
            },
        )),
        "supersession must bind the exact authoritative contract head",
    );'''
DISTINCT_SUPERSEDING_ARTIFACT = '''let (competing_contract_code_hash, competing_abi_hash) = stage_contract_artifact(
        &client,
        &minimal_contract_artifact_with_identity(
            "ParliamentSupersessionCompetitor",
            "integration-tests-supersession-competitor",
        ),
    )?;'''
DISTINCT_SUPERSEDING_ARTIFACT_ASSERTION = '''assert_ne!(
        competing_contract_code_hash, code_hash,
        "the supersession fixture must install a genuinely distinct artifact head",
    );'''
CERTIFIED_SUPERSEDING_DEPLOYMENT_MARKERS = (
    "let competing_deploy_proposal = ProposalKind::DeployContract(",
    "let competing_deploy_create = CreateParliamentGovernanceAttemptV1 {",
    "let competing_deploy_attempt_id = competing_deploy_create.governance_attempt_id();",
    '''InstructionBox::from(ProposeDeployContract {
                contract_address: contract_address.clone(),
                code_hash: competing_contract_code_hash,
                abi_hash: competing_abi_hash,
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            })''',
    "InstructionBox::from(competing_deploy_create)",
    "let competing_deploy_certificate = certify_failure_path_attempt(",
    "assert!(current_height(&client)? < competing_deploy_certificate.enact_at_height);",
    '''assert_eq!(
        deploy_certificate.expected_head, competing_deploy_certificate.expected_head,
        "both certified deployments must compare against the same pre-enactment head",
    );''',
    "let competing_enacted = read_attempt(&client, competing_deploy_attempt_id)?;",
    '''assert_eq!(
        competing_enacted.attempt().status,
        GovernanceAttemptStatusV1::Enacted,
    );''',
    '''assert_eq!(
        competing_enacted.certificate(),
        Some(&competing_deploy_certificate),
    );''',
)
DIRECT_SUPERSESSION_BYPASSES = (
    "ActivateContractInstance",
    "CommitContractDeployment",
)
NO_RESULT_RESTART_CONTRACT_ABSENCE = '''assert_governed_contract_absent(
        &restart_peer.client(),
        contract_address,
        "public-finding restart effect isolation",
    )?;'''
EXACT_INACTIVE_CONTRACT_PROJECTION = '''if object.len() != 3
        || object.get("found").and_then(norito::json::Value::as_bool) != Some(false)
        || object
            .get("contract_address")
            .and_then(norito::json::Value::as_str)
            != Some(contract_address.as_ref())
        || object
            .get("dataspace")
            .and_then(norito::json::Value::as_str)
            != Some("universal")'''
EXACT_ACTIVE_CONTRACT_ENTRYPOINTS = '''let has_exact_entrypoints = object
        .get("public_entrypoints")
        .and_then(norito::json::Value::as_array)
        .is_some_and(|entrypoints| {
            entrypoints.len() == 1 && entrypoints[0].as_str() == Some("main")
        });'''
EXACT_ACTIVE_CONTRACT_PROJECTION = '''if object.len() != 7
        || object.get("found").and_then(norito::json::Value::as_bool) != Some(true)
        || object
            .get("contract_address")
            .and_then(norito::json::Value::as_str)
            != Some(contract_address.as_ref())
        || object
            .get("contract_subject_account")
            .and_then(norito::json::Value::as_str)
            != Some(expected_subject.as_str())
        || object
            .get("dataspace")
            .and_then(norito::json::Value::as_str)
            != Some("universal")
        || object
            .get("code_hash_hex")
            .and_then(norito::json::Value::as_str)
            != Some(expected_code_hash.as_str())
        || object
            .get("abi_hash_hex")
            .and_then(norito::json::Value::as_str)
            != Some(expected_abi_hash.as_str())
        || !has_exact_entrypoints'''
EXACT_GOVERNED_CONTRACT_BINDING_CALLS = (
    '''assert_governed_contract_binding(
        &client,
        &contract_address,
        code_hash,
        abi_hash,
        "consensus-owned certificate enactment must bind the staged contract",
    )?;''',
    '''assert_governed_contract_binding(
            &peer_client,
            &contract_address,
            code_hash,
            abi_hash,
            "every validator must expose the consensus-enacted contract",
        )?;''',
    '''assert_governed_contract_binding(
        &restart_peer.client(),
        &contract_address,
        code_hash,
        abi_hash,
        "normal restart must restore the consensus-enacted contract",
    )?;''',
    '''assert_governed_contract_binding(
        &client,
        &contract_address,
        competing_contract_code_hash,
        competing_abi_hash,''',
    '''assert_governed_contract_binding(
            &peer_client,
            &contract_address,
            competing_contract_code_hash,
            competing_abi_hash,
            "all validators must retain the competing contract binding",
        )?;''',
    '''assert_governed_contract_binding(
        &restored_client,
        &contract_address,
        competing_contract_code_hash,
        competing_abi_hash,
        "restart must retain the competing contract binding",
    )?;''',
)
EXACT_NON_BOUNDARY_PULSE_ABSENCE = '''assert_no_global_beacon_pulse_at(
        &client,
        pulse_height - 1,
        "an unrequested non-boundary height must not emit a global pulse",
    )?;'''
EXACT_ABSENCE_CLASSIFICATION_MARKERS = (
    "fn assert_governed_contract_absent(",
    ".get_gov_contract_response(contract_address)",
    '.wrap_err_with(|| format!("{label}: inactive governed-contract lookup failed"))?;',
    "response.status() != iroha::http::StatusCode::OK",
    "expected governed-contract HTTP 200",
    '''let projection: norito::json::Value = norito::json::from_slice(response.body())
        .wrap_err_with(|| format!("{label}: inactive governed-contract response is not JSON"))?;''',
    EXACT_INACTIVE_CONTRACT_PROJECTION,
    "expected the exact inactive governed-contract projection",
    "fn assert_asset_not_found(client: &Client, asset_id: &AssetId, label: &str)",
    "let query = FindAssetById::new(asset_id.clone());",
    "query.asset_id(),",
    "singular asset query must remain bound to the exact requested identifier",
    "FindError::Asset(missing),",
    "if missing.as_ref() == asset_id => Ok(())",
    "Err(QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))) => {",
    ".query(FindAssets::new())",
    '.filter_with(|asset| asset.equals("id", asset_id.clone()).into_predicate())',
    ".execute_single_opt()",
    "exact-ID asset query failed after a generic not-found response",
    "generic not-found contradicted by exact-ID query returning asset",
    "expected an exact asset-not-found result",
    "fn assert_timed_ovn_casting_context_not_castable(",
    '.expect_err("a sealed timed-OVN corpus must not return a casting context");',
    'rendered.contains("400 Bad Request")',
    "timed-OVN lifecycle is no longer in a casting phase",
    '''assert_timed_ovn_casting_context_not_castable(
        &client,
        ballot_attempt_id,
        "a sealed corpus is no longer a cast-capable context",
    )?;''',
    "fn assert_no_global_beacon_pulse_at(client: &Client, height: u64, label: &str)",
    "fn exact_block(client: &Client, height: u64) -> Result<SignedBlock>",
    "NonZeroU64::new(height)",
    ".query(FindBlocks)",
    '.filter_with(|block| block.equals("height", height).into_predicate())',
    ".execute_single()",
    "if block.header().height() != requested_height",
    "finalized block stream returned height",
    "let block = exact_block(client, height)",
    '''if block
        .npos_consensus_effects()
        .and_then(|effects| effects.finalized_global_beacon_pulse)
        .is_some()''',
    EXACT_NON_BOUNDARY_PULSE_ABSENCE,
)
EXACT_ACTIVE_BINDING_MARKERS = (
    "fn assert_governed_contract_binding(",
    "expected_code_hash: ContractCodeHash",
    "expected_abi_hash: ContractAbiHash",
    '.wrap_err_with(|| format!("{label}: active governed-contract lookup failed"))?;',
    "let expected_subject = contract_address.subject_id().to_string();",
    "let expected_code_hash = expected_code_hash.to_hex();",
    "let expected_abi_hash = expected_abi_hash.to_hex();",
    EXACT_ACTIVE_CONTRACT_ENTRYPOINTS,
    EXACT_ACTIVE_CONTRACT_PROJECTION,
    "expected the exact active governed-contract projection",
    *EXACT_GOVERNED_CONTRACT_BINDING_CALLS,
)
CAPACITY_DOMAIN_REGISTRATION = (
    ".with_genesis_instruction(Register::domain(Domain::new(citizenship_domain.clone())))"
)
CAPACITY_ASSET_REGISTRATION = (
    ".with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric("
)
CAPACITY_EXACT_ASSET = '''citizenship_asset_definition.clone(),
            "Citizenship Bond".to_owned(),
            AssetBalancePolicy::Global,
            None,'''
CAPACITY_ACCOUNT_REGISTRATION = (
    ".with_genesis_instruction(Register::account(Account::new(citizen.clone())))"
)
CAPACITY_BOND_MINT = ".with_genesis_instruction(Mint::asset_quantity("
CAPACITY_CITIZEN_REGISTRATION = ".with_genesis_instruction(RegisterCitizen {"
PARLIAMENT_FAILURE_PATH_MARKERS = {
    "four_validator_certified_effects_record_supersession_and_execution_failure": (
        DISTINCT_SUPERSEDING_ARTIFACT,
        DISTINCT_SUPERSEDING_ARTIFACT_ASSERTION,
        *CERTIFIED_SUPERSEDING_DEPLOYMENT_MARKERS,
        "GovernanceAttemptStatusV1::Superseded",
        "superseded.certificate(), Some(&deploy_certificate)",
        EXACT_SUPERSEDING_HEAD_ASSERTION,
        "GovernanceAttemptStatusV1::ExecutionFailed",
        "parliament_execution_failure_root_v1(",
        "execution_failed.certificate(), Some(&runtime_certificate)",
        "assert_runtime_upgrade_registry_empty(&restored_client)?;",
    ),
    "four_validator_narrow_policy_aborts_when_confirmation_capacity_is_one": (
        "GovernanceAttemptStatusV1::Rejected",
        "required.body != ParliamentBody::ConfirmationJury",
        "rejected.certificate().is_none()",
        "ParliamentBallotFailureKindV1::ConfirmationJuryCapacityUnavailable",
        "Confirmation-capacity rejection effect isolation",
        "Confirmation-capacity peer effect isolation",
        "Confirmation-capacity restart effect isolation",
        "restart must retain the complete Confirmation-capacity transcript",
    ),
    "four_validator_hidden_capacity_retains_then_releases_citizenship_bond": (
        'DomainId::try_new("parliament-bond", "universal")?',
        'AssetDefinitionId::derive_from_components(citizenship_domain.clone(), "xor".parse()?)',
        "&citizenship_domain,\n        &citizenship_asset_definition,",
        "retryable hidden-capacity evidence must retain the bond",
        "GovernanceAttemptStatusV1::Rejected",
        "assert_asset_not_found(",
        "genesis citizenship escrow custody",
        "pre-request capacity evidence must not consume or demand a beacon pulse",
        "assert_no_global_beacon_pulse_at(",
        "returned_bond.value(),\n        &Quantity::from(CAPACITY_BOND_AMOUNT)",
        "terminal rejection must release the exact citizenship collateral",
        "restored_bond.value(),\n        &Quantity::from(CAPACITY_BOND_AMOUNT)",
        "restart must retain the released owner balance",
    ),
}
PARLIAMENT_NETWORK_TEST_ATTRIBUTE = "#[test]"


def read_corridor_source() -> str:
    """Read the lifecycle target together with its source-budget support module."""

    return "\n".join(
        (
            CORRIDOR.read_text(encoding="utf-8"),
            NO_RESULT_PATHS.read_text(encoding="utf-8"),
            FAILURE_PATHS.read_text(encoding="utf-8"),
        )
    )


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
BOUNDARY_PROGRESSION = '''network.ensure_blocks(boundary_height).await?;
    assert_eq!(current_height(&client)?, boundary_height);'''
SUCCESSOR_PROGRESSION = '''network.ensure_blocks(boundary_height + 1).await?;
    assert_eq!(current_height(&client)?, boundary_height + 1);'''
SUCCESSOR_SEED_EQUALITY = (
    "assert_eq!(status.height_context.epoch_seed, successor_seed);"
)
POSITIVE_BEACON_MODES = """constPOSITIVE_BEACON_SIGNER_MODES:[ParliamentBeaconSignerMode;VALIDATOR_COUNT]=[ParliamentBeaconSignerMode::Valid,ParliamentBeaconSignerMode::Valid,ParliamentBeaconSignerMode::Absent,ParliamentBeaconSignerMode::Invalid,];"""
FAIL_CLOSED_BEACON_MODES = """constFAIL_CLOSED_BEACON_SIGNER_MODES:[ParliamentBeaconSignerMode;VALIDATOR_COUNT]=[ParliamentBeaconSignerMode::Valid,ParliamentBeaconSignerMode::Absent,ParliamentBeaconSignerMode::Absent,ParliamentBeaconSignerMode::Invalid,];"""
FAIL_CLOSED_STATUS_REQUEST_BOUND = """letstatus_poll_request_timeout=status_poll_window.checked_div(requests_per_sweep).unwrap_or(Duration::ZERO).max(Duration::from_millis(1)).min(Duration::from_secs(5));"""
FAIL_CLOSED_ACTIVATION_DEADLINE = (
    "letactivation_deadline=Instant::now().checked_add(status_poll_window)"
)
FAIL_CLOSED_TIMEOUT = """letunexpected_pulse_height=tokio::time::timeout(FAIL_CLOSED_BEACON_OBSERVATION_WINDOW,network.peers()[0].once_block(pulse_height),).await;"""
FAIL_CLOSED_POST_OBSERVATION_DEADLINE = (
    "letpost_observation_deadline=Instant::now().checked_add(status_poll_window)"
)
SORANET_POW_CORRIDOR_MARKERS = (
    '.write(["network","soranet_handshake","pow","puzzle","memory_kib",],i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),)',
    '.write(["network","soranet_handshake","pow","puzzle","time_cost"],1_i64,)',
    '.write(["network","soranet_handshake","pow","puzzle","lanes"],1_i64,)',
)
SORANET_POW_REQUIRED_OVERRIDE = (
    '["network","soranet_handshake","pow","required"]'
)
PARLIAMENT_CRYPTO_WORKFLOW_MARKERS = (
    "SORA_PARLIAMENT_CRYPTO_EVIDENCE_DIR: target/sora-parliament-crypto-evidence-",
    "pytest==9.0.3",
    "scripts/tests/check_sora_parliament_crypto_bench_test.py",
    "IROHA_PARLIAMENT_CRYPTO_ALLOCATION_EVIDENCE_V1",
    "cargo bench --locked -p iroha_crypto --bench parliament_crypto",
    "parliament_crypto_allocation_budgets.json",
    "--write-report",
    "--verify-report",
    "SHA256SUMS",
    "name: sora-parliament-crypto-${{ github.sha }}-${{ github.run_id }}-${{ github.run_attempt }}",
    "if-no-files-found: error",
    "retention-days: 90",
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
        *PARLIAMENT_CRYPTO_WORKFLOW_MARKERS,
    ):
        require(marker in body, f"Parliament PR job lost `{marker}`")
    require(
        body.count("cargo bench --locked -p iroha_crypto --bench parliament_crypto") == 2,
        "Parliament crypto evidence needs one allocation and one Criterion invocation",
    )
    for marker, count in (
        ("parliament_crypto_allocation_budgets.json", 3),
        ("SHA256SUMS", 3),
        ("if-no-files-found: error", 2),
    ):
        require(
            body.count(marker) == count,
            f"Parliament PR job needs exactly {count} `{marker}` occurrences",
        )
    require("--release" not in body, "Parliament PR job must not consume release binaries")


def mutate_parliament_workflow(source: str, marker: str) -> str:
    """Remove one marker only from the dedicated Parliament workflow job."""

    job = re.search(
        r"(?ms)^  sora_parliament_lifecycle:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
        source,
    )
    require(job is not None, "PR workflow does not schedule the Parliament corridor")
    body = job.group("body")
    require(marker in body, f"Parliament PR job does not contain `{marker}`")
    mutated = body.replace(marker, "", 1)
    return source[: job.start("body")] + mutated + source[job.end("body") :]


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


def validate_public_finding_no_result_retries_and_restore(source: str) -> None:
    """Require both public-finding NoResult classes, retries, and restore."""

    _, test = parliament_lifecycle_test(source)
    require(
        "exercise_public_finding_no_result_retries_and_restore(" in test,
        "Parliament lifecycle test lost the public-finding closure invocation",
    )
    helper = re.search(
        r"(?ms)^pub\(super\) async fn "
        r"exercise_public_finding_no_result_retries_and_restore\(.*?^\}\n",
        source,
    )
    require(helper is not None, "public-finding closure helper is absent")
    helper_source = helper.group(0)
    for marker in (
        "network.ensure_blocks(sortition_pulse_height).await?;",
        "ParliamentLifecycleTransitionV1::RecordAttemptAbsence(",
        "failed_height < public_finding_deadline",
        "BodyInstanceStatusV1::NoResult",
        "ParliamentNoResultKindV1::PublicFindingQuorumUnreachable",
        "public-finding quorum-unreachable effect isolation",
        "public-finding peer effect isolation",
        "attempt_sequence: 1",
        "GovernanceStageV1::Qualification",
        "iroha.integration.parliament.competing-public-finding.v1",
        "deadline retry has no nonmember permissionless relayer",
        "public-finding endorsement after the inclusive frozen deadline",
        "ParliamentLifecycleTransitionV1::FailPublicFindingNoResult(",
        "ParliamentNoResultKindV1::PublicFindingDeadlineExpired",
        "attempt_sequence: 2",
        "peer_rejected.state_payload_hex",
        "peer_retry.state_payload_hex",
        "peer_second_retry.state_payload_hex",
        "restart_peer.start_checked(config_layers.iter(), None)",
        "restored_retry.state_payload_hex",
        "restored_second_retry.state_payload_hex",
        NO_RESULT_RESTART_CONTRACT_ABSENCE,
    ):
        require(marker in helper_source, f"public-finding closure corridor lost `{marker}`")


def validate_exact_absence_classification(source: str) -> None:
    """Require exact positive and negative state projections, not transport success."""

    for marker in (
        *EXACT_ABSENCE_CLASSIFICATION_MARKERS,
        *EXACT_ACTIVE_BINDING_MARKERS,
    ):
        require(
            marker in source,
            f"Parliament state coverage lost exact classifier `{marker}`",
        )
    require(
        source.count(".get_gov_contract_json(") == 1
        and source.count(".get_gov_contract_response(") == 1,
        "governed-contract reads must remain centralized in the exact projection helpers",
    )
    require(
        source.count("assert_governed_contract_absent(") == 7,
        "all six inactive-contract checks must use the exact projection helper",
    )
    require(
        source.count("assert_governed_contract_binding(") == 7,
        "all six active-contract checks must use the exact projection helper",
    )
    require(
        source.count(
            "Err(QueryError::Validation(ValidationFail::QueryFailed("
            "QueryExecutionFail::NotFound))) => {"
        )
        == 1,
        "asset absence must classify exactly one nested NotFound fallback",
    )
    require(
        source.count("let query = FindAssetById::new(asset_id.clone());") == 1
        and source.count("query.asset_id(),") == 1
        and source.count(
            '.filter_with(|asset| asset.equals("id", asset_id.clone()).into_predicate())'
        )
        == 1
        and source.count(".execute_single_opt()") == 1,
        "generic asset NotFound must be corroborated by one bounded exact-ID query",
    )
    require(
        source.count(
            '.filter_with(|block| block.equals("height", height).into_predicate())'
        )
        and source.count(".execute_single()") == 1,
        "exact finalized-block checks must use one bounded exact-height query",
    )
    require(
        ".query(FindBlocks)\n        .execute_all()" not in source,
        "finalized-block checks must not use an unbounded block inventory query",
    )

    broad_absence_patterns = {
        "governed-contract lookup": (
            r"\.get_gov_contract_json\((?:&)?contract_address\)\s*\.is_err\(\)"
        ),
        "citizenship asset lookup": (
            r"\.query_single\(FindAssetById::new\(citizen_asset_id\.clone\(\)\)\)"
            r"\s*\.is_err\(\)"
        ),
        "global beacon pulse lookup": (
            r"pulse_at\(\s*&client,\s*pulse_height\s*-\s*1\s*\)\s*\.is_err\(\)"
        ),
        "sealed timed-OVN casting-context lookup": (
            r"\.get_parliament_timed_ovn_casting_context\([^)]*\)\s*\.is_err\(\)"
        ),
    }
    for label, pattern in broad_absence_patterns.items():
        require(
            re.search(pattern, source) is None,
            f"Parliament {label} regained a broad `is_err()` absence assertion",
        )


def parliament_failure_path_test(source: str, name: str) -> tuple[re.Match[str], str]:
    """Return one exact executable Parliament failure-path test and its async body."""

    pattern = re.compile(
        rf"(?ms)^#\[test\]\nfn {re.escape(name)}\(\) -> Result<\(\)> \{{\n"
        r".*?^\}\n"
        rf"\nasync fn {re.escape(name)}_impl\(\)\s*-> Result<\(\)>\s*\{{\n"
        r".*?^\}\n(?=\n#\[test\]|\Z)"
    )
    matches = list(pattern.finditer(source))
    require(len(matches) == 1, f"Parliament failure path `{name}` is not one exact test item")
    match = matches[0]
    return match, match.group(0)


def validate_parliament_failure_paths(source: str) -> None:
    """Pin terminal-state, rollback, capacity-abort, and bond-release coverage."""

    for name, markers in PARLIAMENT_FAILURE_PATH_MARKERS.items():
        _, test = parliament_failure_path_test(source, name)
        for marker in markers:
            require(marker in test, f"Parliament failure path `{name}` lost `{marker}`")
        if name == (
            "four_validator_certified_effects_record_supersession_and_execution_failure"
        ):
            certified_positions = [
                test.index(marker)
                for marker in CERTIFIED_SUPERSEDING_DEPLOYMENT_MARKERS
            ]
            require(
                certified_positions == sorted(certified_positions),
                "certified supersession path must propose, certify, and autonomously "
                "enact the competing deployment in order",
            )
            for bypass in DIRECT_SUPERSESSION_BYPASSES:
                require(
                    bypass not in test,
                    f"certified supersession path regained direct `{bypass}` authority",
                )


def capacity_failure_builder(source: str) -> tuple[re.Match[str], str]:
    """Return the exact hidden-capacity network builder."""

    pattern = re.compile(
        r"(?ms)^fn capacity_failure_builder\(.*?^\}\n"
        r"(?=\nfn confirmation_capacity_builder\()"
    )
    matches = list(pattern.finditer(source))
    require(len(matches) == 1, "hidden-capacity network builder is not one exact item")
    match = matches[0]
    return match, match.group(0)


def validate_capacity_failure_genesis(source: str) -> None:
    """Require the exact citizenship asset hierarchy before bond mint/custody."""

    _, builder = capacity_failure_builder(source)
    ordered = (
        CAPACITY_DOMAIN_REGISTRATION,
        CAPACITY_ASSET_REGISTRATION,
        CAPACITY_ACCOUNT_REGISTRATION,
        CAPACITY_BOND_MINT,
        CAPACITY_CITIZEN_REGISTRATION,
    )
    positions = []
    for marker in (*ordered, CAPACITY_EXACT_ASSET):
        require(marker in builder, f"hidden-capacity genesis lost `{marker}`")
        if marker in ordered:
            positions.append(builder.index(marker))
    require(
        positions == sorted(positions),
        "hidden-capacity genesis must register domain, asset, and account before mint/custody",
    )


def mutate_capacity_failure_builder(
    source: str, old: str, new: str = ""
) -> str:
    """Apply one exact mutation only inside the hidden-capacity builder."""

    match, builder = capacity_failure_builder(source)
    require(old in builder, f"hidden-capacity mutation target is absent: `{old}`")
    mutated = builder.replace(old, new, 1)
    return source[: match.start()] + mutated + source[match.end() :]


def mutate_parliament_failure_path_test(
    source: str, name: str, old: str, new: str = ""
) -> str:
    """Apply one exact mutation only inside a Parliament failure-path test item."""

    match, test = parliament_failure_path_test(source, name)
    require(old in test, f"Parliament failure-path mutation target is absent: `{old}`")
    mutated = test.replace(old, new, 1)
    return source[: match.start()] + mutated + source[match.end() :]


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
    compacted_tests = "".join(compact(test) for _, test in tests)
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
            compacted_tests.count(marker) == len(tests),
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
    """Require old-session boundary safety and a genuine successor pulse."""

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
        "deterministic_parliament_beacon_successor_key_record_v1(",
        "assert_ne!(\n        successor_beacon_record.session.session_id,",
        "let boundary_height = MANDATORY_NPOS_EPOCH_LENGTH_BLOCKS;",
        "pulse_height = boundary_height - 1",
        "lifecycle_certificate_replacing(",
        "Some(beacon_record.session.session_id)",
        "successor_beacon_record.session.session_id",
        AUTONOMOUS_PULSE_PROGRESSION,
        "verify_finalized_global_threshold_beacon_pulse_v1(",
        "let successor_epoch = 1;",
        "global_threshold_beacon_npos_successor_seed_v1(",
        BOUNDARY_PROGRESSION,
        SUCCESSOR_PROGRESSION,
        "assert_eq!(status.height_context.epoch, successor_epoch);",
        SUCCESSOR_SEED_EQUALITY,
        "let successor_pulse_height = boundary_height",
        "assert_eq!(\n        successor_pulse.session_id,",
        "&validated_successor_beacon_session",
        "let second_successor_epoch = 2;",
        "assert_eq!(status.height_context.epoch, second_successor_epoch);",
        "assert_eq!(status.height_context.epoch_seed, second_successor_seed);",
        "!status.restart_required",
    )
    for marker in required:
        require(marker in test, f"mandatory NPoS beacon test lost `{marker}`")
    require(
        test.count("verify_finalized_global_threshold_beacon_pulse_v1(") == 2,
        "mandatory NPoS beacon test must independently verify predecessor and successor pulses",
    )
    require(
        test.count("!status.restart_required") == 2,
        "mandatory NPoS beacon test must prove both successor epochs remain live",
    )
    require(
        ".with_permissioned_consensus()" not in test,
        "mandatory NPoS beacon test selected permissioned consensus",
    )
    require(
        'tick(&client, "commit mandatory pre-boundary pulse")' not in test,
        "mandatory pre-boundary pulse must not race a user transaction",
    )


def validate_beacon_rotation_fixture(source: str) -> None:
    """Require two complete feature-only DKG fixtures and exact-session dispatch."""

    require(
        source.count("TEST_SUCCESSOR_SESSION_ID_DOMAIN_V1") == 2,
        "rotatable beacon fixture lost the unique successor domain binding",
    )
    for marker in (
        "deterministic_parliament_beacon_successor_key_record_v1",
        "deterministic_fixture_v1(network_id, ordered_roster, true)",
        "let initial_fixture =",
        "deterministic_fixture_v1(self.network_id, &self.ordered_roster, false)",
        "let successor_fixture =",
        "deterministic_fixture_v1(self.network_id, &self.ordered_roster, true)",
        "if successor_fixture.session.record() != session.record()",
        "exact_seat_signer_supports_one_domain_separated_successor_session",
        "assert_ne!(initial.session.session_id, successor.session.session_id);",
        "verify_partial_signature(payload, &partial)",
    ):
        require(marker in source, f"rotatable beacon test fixture lost `{marker}`")


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
        "let pulse_status_is_active = |status: &SumeragiV2Status| -> Result<bool> {",
        "SumeragiV2StatusPhase::PendingApply",
        "SumeragiV2BodyState::PendingApply",
        "SumeragiV2BodyState::Applied",
        "status.liveness.work.application,",
        "SumeragiV2LocalWorkStage::Queued",
        "SumeragiV2LocalWorkStage::Running",
        "SumeragiV2LocalWorkStage::Complete",
        "SumeragiV2ProgressTransition::Applied",
        "let status_poll_window = network.sync_timeout();",
        "status_poll_window.is_zero()",
        "let requests_per_sweep = u32::try_from(network.peers().len())",
        ".and_then(|peers| peers.checked_mul(2))",
        "client.torii_request_timeout = status_poll_request_timeout;",
        "let activation_deadline = Instant::now()",
        "let mut last_activation_status_error = None;",
        "for (peer_index, peer_client) in status_poll_clients.iter().enumerate() {",
        "let observed_height = match current_height(peer_client) {",
        'Some(format!("peer {peer_index} height: {error}"));',
        "let status = match peer_client.get_sumeragi_status() {",
        "last_activation_status_error =",
        'Some(format!("peer {peer_index} sumeragi status: {error}"));',
        "all_pulse_heights_active = false;",
        "activation_deadline.saturating_duration_since(Instant::now())",
        "in-flight request bound; last status fetch error: {}",
        "last status fetch error: {}",
        "all_pulse_heights_active &= pulse_status_is_active(&status)?;",
        "unexpected_pulse_height.is_err()",
        "let post_observation_deadline = Instant::now()",
        "let mut last_post_observation_status_error = None;",
        "let mut all_post_observation_statuses_verified = true;",
        "last_post_observation_status_error =",
        "all_post_observation_statuses_verified = false;",
        "if all_post_observation_statuses_verified {",
        "post_observation_deadline.saturating_duration_since(Instant::now())",
        "after the below-threshold observation; last status fetch error: {}",
        "without leaving detached blocking",
        "peer.is_running()",
        "!status.restart_required",
        "assert_eq!(status.last_committed_height, predecessor_height);",
        "assert_eq!(status.height, pulse_height);",
    )
    for marker in required:
        require(marker in test, f"fail-closed NPoS beacon test lost `{marker}`")
    compacted = compact(test)
    pre_apply_start = compacted.index(
        "ifstatus.body_state==SumeragiV2BodyState::PendingApply{"
    )
    applied_handoff_start = compacted.index(
        "assert_eq!(status.body_state,SumeragiV2BodyState::Applied);",
        pre_apply_start,
    )
    require(
        "returnOk(false);" in compacted[pre_apply_start:applied_handoff_start],
        "the durable pre-application predecessor must remain a retry, not an active pulse",
    )
    require(
        FAIL_CLOSED_STATUS_REQUEST_BOUND in compacted,
        "fail-closed NPoS beacon test lost its short non-zero per-request bound",
    )
    require(
        FAIL_CLOSED_TIMEOUT in compacted,
        "fail-closed NPoS beacon test lost its bounded no-block observation",
    )
    require(
        FAIL_CLOSED_ACTIVATION_DEADLINE in compacted,
        "fail-closed NPoS beacon test lost its monotonic pulse-context activation deadline",
    )
    require(
        FAIL_CLOSED_POST_OBSERVATION_DEADLINE in compacted,
        "fail-closed NPoS beacon test lost its fresh monotonic post-observation deadline",
    )
    require(
        compacted.index(FAIL_CLOSED_ACTIVATION_DEADLINE)
        < compacted.index(FAIL_CLOSED_TIMEOUT)
        < compacted.index(FAIL_CLOSED_POST_OBSERVATION_DEADLINE),
        "fail-closed NPoS beacon gates must order activation, observation, then post-observation verification",
    )
    require(
        test.count("let observed_height = match current_height(peer_client) {") == 2
        and test.count('Some(format!("peer {peer_index} height: {error}"));') == 2,
        "both fail-closed NPoS status gates must retry transient authoritative-height failures",
    )
    require(
        test.count(
            'Some(format!("peer {peer_index} sumeragi status: {error}"));'
        )
        == 2,
        "both fail-closed NPoS status gates must retry transient Sumeragi-status failures",
    )
    require(
        test.count("Instant::now() >= activation_deadline") == 3
        and test.count("Instant::now() >= post_observation_deadline") == 3,
        "both fail-closed NPoS status gates must check their deadline around every synchronous request",
    )
    require(
        test.count(".saturating_duration_since(Instant::now())") == 2,
        "both fail-closed NPoS status gates must bound their retry sleep by the remaining deadline",
    )
    require(
        test.count("pulse_status_is_active(&status)?") == 2,
        "fail-closed NPoS beacon test must validate the pulse context before and after observation",
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
        "spawn_blocking" not in test,
        "fail-closed beacon status polls must not leave detached blocking requests",
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

    def test_workflow_contract_rejects_each_removed_crypto_evidence_marker(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for marker in PARLIAMENT_CRYPTO_WORKFLOW_MARKERS:
            with self.subTest(marker=marker), self.assertRaises(ContractError):
                validate_workflow(mutate_parliament_workflow(source, marker))

    def test_target_and_boundary_guards_remain_feature_isolated(self) -> None:
        manifest = MANIFEST.read_text(encoding="utf-8")
        target = re.search(
            r'(?ms)^\[\[test\]\]\nname = "sora_parliament_lifecycle_smoke"\n'
            r'path = "tests/sora_parliament_lifecycle_smoke.rs"\n'
            r'required-features = \["parliament-test-signers"\]$',
            manifest,
        )
        require(target is not None, "Parliament test target lost its opt-in feature")

        corridor = read_corridor_source()
        require(
            '#[path = "sora_parliament_no_result_paths.rs"]\nmod no_result_paths;'
            in corridor,
            "Parliament lifecycle target lost its NoResult support module",
        )
        require(
            '#[path = "sora_parliament_failure_paths.rs"]\nmod failure_paths;'
            in corridor,
            "Parliament lifecycle target lost its failure-path module",
        )
        helper_calls = corridor.count("assert_transition_rejected_without_state_change(")
        require(
            helper_calls == 12,
            "corridor must retain the helper plus eleven boundary/replay checks",
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
        validate_public_finding_no_result_retries_and_restore(corridor)
        validate_exact_absence_classification(corridor)
        validate_parliament_failure_paths(corridor)
        validate_capacity_failure_genesis(corridor)
        validate_beacon_mode_profiles(corridor)
        validate_mandatory_npos_boundary(corridor)
        validate_beacon_rotation_fixture(
            BEACON_TEST_SIGNER.read_text(encoding="utf-8")
        )
        validate_fail_closed_npos_boundary(corridor)
        validate_feature_only_fault_wiring(
            TEST_NETWORK.read_text(encoding="utf-8"),
            DAEMON.read_text(encoding="utf-8"),
            BEACON.read_text(encoding="utf-8"),
            BEACON_LIFECYCLE.read_text(encoding="utf-8"),
        )

    def test_required_bounded_soranet_pow_rejects_adversarial_mutations(self) -> None:
        corridor = read_corridor_source()
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
        corridor = read_corridor_source()
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
        corridor = read_corridor_source()
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

    def test_public_finding_no_result_retries_and_restore_reject_mutations(self) -> None:
        corridor = read_corridor_source()
        for marker in (
            "exercise_public_finding_no_result_retries_and_restore(",
            "failed_height < public_finding_deadline",
            "ParliamentNoResultKindV1::PublicFindingQuorumUnreachable",
            "public-finding quorum-unreachable effect isolation",
            "public-finding peer effect isolation",
            "attempt_sequence: 1",
            "iroha.integration.parliament.competing-public-finding.v1",
            "ParliamentNoResultKindV1::PublicFindingDeadlineExpired",
            "attempt_sequence: 2",
            "peer_retry.state_payload_hex",
            "restored_second_retry.state_payload_hex",
            NO_RESULT_RESTART_CONTRACT_ABSENCE,
        ):
            with self.subTest(marker=marker), self.assertRaises(ContractError):
                validate_public_finding_no_result_retries_and_restore(
                    corridor.replace(marker, "", 1)
                )

    def test_exact_absence_classification_rejects_adversarial_mutations(self) -> None:
        corridor = read_corridor_source()
        for marker in (
            *EXACT_ABSENCE_CLASSIFICATION_MARKERS,
            *EXACT_ACTIVE_BINDING_MARKERS,
        ):
            with self.subTest(marker=marker), self.assertRaises(ContractError):
                validate_exact_absence_classification(corridor.replace(marker, "", 1))

        broad_mutations = {
            "governed-contract lookup": corridor
            + "\nclient.get_gov_contract_json(contract_address).is_err();\n",
            "citizenship asset lookup": corridor
            + "\nclient.query_single(FindAssetById::new("
            "citizen_asset_id.clone())).is_err();\n",
            "global beacon pulse lookup": corridor
            + "\npulse_at(&client, pulse_height - 1).is_err();\n",
            "sealed timed-OVN casting-context lookup": corridor
            + "\nclient.get_parliament_timed_ovn_casting_context(ballot_attempt_id)"
            ".is_err();\n",
            "generic asset not-found drops exact request binding": corridor.replace(
                "query.asset_id(),",
                "asset_id,",
                1,
            ),
            "generic asset not-found drops exact corroboration": corridor.replace(
                '.filter_with(|asset| asset.equals("id", asset_id.clone()).into_predicate())',
                '.filter_with(|asset| asset.equals("definition", asset_id.clone()).into_predicate())',
                1,
            ),
            "block lookup drops exact height filter": corridor.replace(
                '.filter_with(|block| block.equals("height", height).into_predicate())',
                '.filter_with(|block| block.equals("hash", height).into_predicate())',
                1,
            ),
            "unbounded block inventory query": corridor.replace(
                ".query(FindBlocks)\n        .filter_with(|block| block.equals(\"height\", height).into_predicate())\n        .execute_single()",
                ".query(FindBlocks)\n        .execute_all()",
                1,
            ),
            "bare active-contract transport success": corridor
            + "\nclient.get_gov_contract_json(&contract_address)?;\n",
            "inactive route still expects HTTP 404": corridor.replace(
                "response.status() != iroha::http::StatusCode::OK",
                "response.status() != iroha::http::StatusCode::NOT_FOUND",
                1,
            ),
            "inactive found=true projection": corridor.replace(
                "Some(false)", "Some(true)", 1
            ),
            "inactive response permits extra fields": corridor.replace(
                "object.len() != 3", "object.len() < 3", 1
            ),
            "active response permits extra fields": corridor.replace(
                "object.len() != 7", "object.len() < 7", 1
            ),
            "active response accepts any entrypoint": corridor.replace(
                'entrypoints.len() == 1 && entrypoints[0].as_str() == Some("main")',
                "!entrypoints.is_empty()",
                1,
            ),
        }
        for label, mutated in broad_mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_exact_absence_classification(mutated)

    def test_parliament_failure_paths_reject_adversarial_mutations(self) -> None:
        corridor = read_corridor_source()
        for name, markers in PARLIAMENT_FAILURE_PATH_MARKERS.items():
            with self.subTest(name=name, marker="test name"), self.assertRaises(
                ContractError
            ):
                validate_parliament_failure_paths(
                    corridor.replace(f"fn {name}()", f"fn {name}_disabled()", 1)
                )
            for marker in markers:
                with self.subTest(name=name, marker=marker), self.assertRaises(
                    ContractError
                ):
                    validate_parliament_failure_paths(
                        mutate_parliament_failure_path_test(corridor, name, marker)
                    )

        certified_name = (
            "four_validator_certified_effects_record_supersession_and_execution_failure"
        )
        certified_anchor = CERTIFIED_SUPERSEDING_DEPLOYMENT_MARKERS[0]
        for bypass in DIRECT_SUPERSESSION_BYPASSES:
            with self.subTest(name=certified_name, bypass=bypass), self.assertRaises(
                ContractError
            ):
                validate_parliament_failure_paths(
                    mutate_parliament_failure_path_test(
                        corridor,
                        certified_name,
                        certified_anchor,
                        f"{bypass};\n    {certified_anchor}",
                    )
                )

    def test_capacity_failure_genesis_rejects_adversarial_mutations(self) -> None:
        corridor = read_corridor_source()
        for marker in (
            CAPACITY_DOMAIN_REGISTRATION,
            CAPACITY_ASSET_REGISTRATION,
            CAPACITY_EXACT_ASSET,
            CAPACITY_ACCOUNT_REGISTRATION,
            CAPACITY_BOND_MINT,
            CAPACITY_CITIZEN_REGISTRATION,
        ):
            with self.subTest(marker=marker), self.assertRaises(ContractError):
                validate_capacity_failure_genesis(
                    mutate_capacity_failure_builder(corridor, marker)
                )

        reordered = mutate_capacity_failure_builder(
            corridor, CAPACITY_DOMAIN_REGISTRATION, "__CAPACITY_DOMAIN_REGISTRATION__"
        )
        reordered = mutate_capacity_failure_builder(
            reordered, CAPACITY_ASSET_REGISTRATION, CAPACITY_DOMAIN_REGISTRATION
        )
        reordered = mutate_capacity_failure_builder(
            reordered, "__CAPACITY_DOMAIN_REGISTRATION__", CAPACITY_ASSET_REGISTRATION
        )
        with self.subTest(marker="domain/asset order"), self.assertRaises(ContractError):
            validate_capacity_failure_genesis(reordered)

    def test_beacon_rotation_fixture_rejects_adversarial_mutations(self) -> None:
        source = BEACON_TEST_SIGNER.read_text(encoding="utf-8")
        for marker in (
            "TEST_SUCCESSOR_SESSION_ID_DOMAIN_V1",
            "deterministic_fixture_v1(network_id, ordered_roster, true)",
            "deterministic_fixture_v1(self.network_id, &self.ordered_roster, true)",
            "exact_seat_signer_supports_one_domain_separated_successor_session",
        ):
            with self.subTest(marker=marker), self.assertRaises(ContractError):
                validate_beacon_rotation_fixture(source.replace(marker, "", 1))

    def test_mandatory_npos_boundary_rejects_adversarial_mutations(self) -> None:
        corridor = read_corridor_source()
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
            "missing successor transcript": mutate_mandatory_npos_test(
                corridor,
                "deterministic_parliament_beacon_successor_key_record_v1(",
            ),
            "missing compare-and-set predecessor": mutate_mandatory_npos_test(
                corridor, "Some(beacon_record.session.session_id)"
            ),
            "successor pulse not bound to successor session": mutate_mandatory_npos_test(
                corridor,
                "assert_eq!(\n        successor_pulse.session_id,",
                "assert_ne!(\n        successor_pulse.session_id,",
            ),
            "rotated pulse not independently verified": mutate_mandatory_npos_test(
                corridor, "&validated_successor_beacon_session"
            ),
            "second epoch seed equality omitted": mutate_mandatory_npos_test(
                corridor,
                "assert_eq!(status.height_context.epoch_seed, second_successor_seed);",
                "let _ = (status.height_context.epoch_seed, second_successor_seed);",
            ),
            "successor fail-stop status omitted": mutate_mandatory_npos_test(
                corridor, "!status.restart_required", "true"
            ),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label), self.assertRaises(ContractError):
                validate_mandatory_npos_boundary(mutated)

    def test_beacon_mode_profiles_reject_adversarial_mutations(self) -> None:
        corridor = read_corridor_source()
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
        corridor = read_corridor_source()
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
            "per-request timeout disabled": mutate_fail_closed_npos_test(
                corridor,
                "client.torii_request_timeout = status_poll_request_timeout;",
                "client.torii_request_timeout = Duration::ZERO;",
            ),
            "pulse activation deadline omitted": mutate_fail_closed_npos_test(
                corridor,
                "if Instant::now() >= activation_deadline {",
                "if false {",
            ),
            "post-observation deadline omitted": mutate_fail_closed_npos_test(
                corridor,
                "if Instant::now() >= post_observation_deadline {",
                "if false {",
            ),
            "applied predecessor handoff weakened": mutate_fail_closed_npos_test(
                corridor,
                "SumeragiV2StatusPhase::PendingApply",
                "SumeragiV2StatusPhase::AwaitingProposal",
            ),
            "pre-application predecessor accepted as active": mutate_fail_closed_npos_test(
                corridor,
                '''            return Ok(false);
        }
        assert_eq!(status.body_state, SumeragiV2BodyState::Applied);''',
                '''            return Ok(true);
        }
        assert_eq!(status.body_state, SumeragiV2BodyState::Applied);''',
            ),
            "activation height-fetch retry omitted": mutate_fail_closed_npos_test(
                corridor,
                '''Err(error) => {
                    last_activation_status_error =
                        Some(format!("peer {peer_index} height: {error}"));
                    all_pulse_heights_active = false;
                    continue;
                }''',
                "Err(error) => return Err(error.into()),",
            ),
            "activation Sumeragi-status retry omitted": mutate_fail_closed_npos_test(
                corridor,
                '''Err(error) => {
                    last_activation_status_error =
                        Some(format!("peer {peer_index} sumeragi status: {error}"));
                    all_pulse_heights_active = false;
                    continue;
                }''',
                "Err(error) => return Err(error.into()),",
            ),
            "post-observation height-fetch retry omitted": mutate_fail_closed_npos_test(
                corridor,
                '''Err(error) => {
                    last_post_observation_status_error =
                        Some(format!("peer {peer_index} height: {error}"));
                    all_post_observation_statuses_verified = false;
                    continue;
                }''',
                "Err(error) => return Err(error.into()),",
            ),
            "post-observation Sumeragi-status retry omitted": mutate_fail_closed_npos_test(
                corridor,
                '''Err(error) => {
                    last_post_observation_status_error =
                        Some(format!("peer {peer_index} sumeragi status: {error}"));
                    all_post_observation_statuses_verified = false;
                    continue;
                }''',
                "Err(error) => return Err(error.into()),",
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
