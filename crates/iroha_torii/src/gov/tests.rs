//! Governance route and conversion tests.

use super::*;
use crate::routing::MaybeTelemetry;
use axum::body::Bytes;
use iroha_config::parameters::actual::LaneConfig;
use iroha_core::{
    block::BlockBuilder,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{Queue, TransactionGuard},
    smartcontracts::code::{activate_instance, register_code_bytes, register_manifest},
    state::{
        ElectionState, GovernanceLockCustody, GovernanceLockRecord, GovernanceLocksForReferendum,
        GovernanceProposalRecord, GovernanceProposalStatus, GovernanceReferendumMode,
        GovernanceReferendumRecord, GovernanceReferendumStatus, State, World,
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::Domain,
    isi::{InstructionBox, governance::RegisterCitizen},
    permission::Permission,
    smart_contract::manifest::ContractManifest,
};
use iroha_model_base::chain::ChainId;
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;
use std::sync::Arc;
const ACCOUNT_AUTHORITY: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
const ACCOUNT_OWNER_ALT: &str = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D";

#[test]
fn governance_capability_proposal_kinds_match_the_append_only_v1_inventory() {
    assert_eq!(
        GOVERNANCE_SUPPORTED_PROPOSAL_KINDS_V1,
        [
            "DEPLOY_CONTRACT",
            "RUNTIME_UPGRADE",
            "SCCP_ROUTE_GOVERNANCE",
            "VALIDATION_FEE_POLICY",
            "VALIDATION_FEE_PAYOUT_LIFECYCLE",
            "MUSUBI_REGISTRY_GOVERNANCE",
            "SORAFS_PROVIDER_GOVERNANCE",
            "CONTRACT_LIFECYCLE_GOVERNANCE",
            "CONTRACT_EMERGENCY_HOLD",
            "GLOBAL_DATA_TRIGGER_PERMISSION_GOVERNANCE",
        ]
    );
}

#[test]
fn casting_context_route_holds_heavy_admission_through_blocking_replay() {
    let route_source = include_str!("../lib.rs");
    let handler = route_source
        .split("async fn handler_gov_parliament_timed_ovn_casting_context_read")
        .nth(1)
        .and_then(|tail| {
            tail.split("async fn handler_gov_parliament_tle_release_context_read")
                .next()
        })
        .expect("casting-context route source");
    let access = handler
        .find("check_access(")
        .expect("canonical access gate");
    let admission = handler
        .find("let replay_admission = acquire_query_admission(app.as_ref(), true).await?;")
        .expect("heavy admission gate");
    let joined = handler
        .find("crate::panic_recovery::join_recoverable(")
        .expect("recoverable replay join boundary");
    let blocking = handler
        .find("crate::panic_recovery::spawn_blocking_recoverable(")
        .expect("reviewed blocking replay isolation");
    let retained = handler
        .find("let _replay_admission = replay_admission;")
        .expect("permit retained by physical replay task");
    assert!(access < admission && admission < joined && joined < blocking && blocking < retained);
}

#[test]
fn release_context_route_holds_heavy_admission_through_blocking_replay() {
    let route_source = include_str!("../lib.rs");
    let handler = route_source
        .split("async fn handler_gov_parliament_tle_release_context_read")
        .nth(1)
        .and_then(|tail| {
            tail.split("async fn handler_gov_parliament_tle_partial_release")
                .next()
        })
        .expect("release-context route source");
    let access = handler
        .find("check_access(")
        .expect("canonical access gate");
    let admission = handler
        .find("let replay_admission = acquire_query_admission(app.as_ref(), true).await?;")
        .expect("heavy admission gate");
    let joined = handler
        .find("crate::panic_recovery::join_recoverable(")
        .expect("recoverable replay join boundary");
    let blocking = handler
        .find("crate::panic_recovery::spawn_blocking_recoverable(")
        .expect("reviewed blocking replay isolation");
    let retained = handler
        .find("let _replay_admission = replay_admission;")
        .expect("permit retained by physical replay task");
    assert!(access < admission && admission < joined && joined < blocking && blocking < retained);
}

#[test]
fn release_context_handler_validates_its_public_projection() {
    let source = include_str!("../gov.rs");
    let handler = source
        .split("pub fn handle_gov_parliament_tle_release_context_read")
        .nth(1)
        .and_then(|tail| {
            tail.split("/// Strict citizen registration draft request.")
                .next()
        })
        .expect("release-context handler source");
    let validation = handler
        .find("validate_for_ballot(ballot_attempt_id)")
        .expect("public response validation");
    let returned = handler
        .find("Ok(JsonBody(response))")
        .expect("validated public response return");
    assert!(validation < returned);
}

fn generic_lock_custody(state: &State) -> GovernanceLockCustody {
    GovernanceLockCustody {
        escrowed: !state.gov.min_bond_amount.is_zero(),
        asset_definition_id: state.gov.voting_asset_id.clone(),
        bond_escrow_account: state.gov.bond_escrow_account.clone(),
        slash_receiver_account: state.gov.slash_receiver_account.clone(),
    }
}
#[test]
fn first_release_capabilities_expose_only_attempt_based_private_parliament() {
    assert_eq!(
        GOVERNANCE_APPROVAL_MODE_V1,
        "PARLIAMENT_ATTEMPT_TIMED_OVN_V1"
    );
    let source = include_str!("../gov.rs");
    let retired_mode = ["LEGACY", "COUNCIL", "EPOCH"].join("_");
    let retired_resolver = ["fn governance", "approval", "mode"].join("_");
    assert!(!source.contains(&retired_mode));
    assert!(!source.contains(&retired_resolver));
    let capabilities_tail = &source[source
        .find("supported_routes: vec![")
        .expect("capability route projection")..];
    let capabilities = &capabilities_tail[..capabilities_tail
        .find("],\n    }))")
        .expect("capability route projection end")];
    assert!(capabilities.contains("/v1/gov/ballots/plain"));
    assert!(capabilities.contains("/v1/gov/ballots/zk-v1"));
    let advertised_parliament_routes = capabilities
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('"')?
                .strip_suffix("\".to_owned(),")
        })
        .filter(|route| route.starts_with("/v1/gov/parliament/"))
        .collect::<Vec<_>>();
    assert_eq!(
        advertised_parliament_routes,
        [
            "/v1/gov/parliament/attempts/draft",
            "/v1/gov/parliament/attempts/{governance_attempt_id}",
            "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-context",
            "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof",
            "/v1/gov/parliament/ballots/{ballot_attempt_id}/release-context",
            "/v1/gov/parliament/ballots/{ballot_attempt_id}/partial-release",
            "/v1/gov/parliament/transitions/draft",
        ]
    );
    assert!(!capabilities.contains("\"/v1/gov/parliament/ballots\".to_owned()"));
    assert!(!capabilities.contains("/v1/gov/finalize"));
    assert!(!capabilities.contains("/v1/gov/enact"));
}
#[tokio::test]
async fn parliament_draft_handlers_frame_exact_native_instructions() {
    use iroha_data_model::{
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal,
            GovernanceAttemptId, ProposalKind,
        },
        isi::{
            Instruction as _,
            governance::{
                CreateParliamentGovernanceAttemptV1, ParliamentLifecycleTransitionV1,
                SubmitParliamentLifecycleTransitionV1,
            },
        },
    };

    let attempt_request = ParliamentAttemptDraftRequestV1 {
        version: PARLIAMENT_API_VERSION_V1,
        proposal: ProposalKind::DeployContract(DeployContractProposal {
            proposal_operator: ALICE_ID.clone(),
            contract_address: sample_contract_address(),
            code_hash: ContractCodeHash::new([0x11; 32]),
            abi_hash: ContractAbiHash::new([0x22; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        }),
        attempt_sequence: 4,
    };
    let attempt_response = handle_gov_parliament_attempt_draft(NoritoJson(attempt_request))
        .await
        .expect("draft exact Parliament attempt")
        .0;
    assert_eq!(attempt_response.tx_instructions.len(), 1);
    let attempt_draft = &attempt_response.tx_instructions[0];
    let attempt_instruction = iroha_data_model::isi::decode_instruction_from_pair(
        &attempt_draft.wire_id,
        &hex::decode(&attempt_draft.payload_hex).expect("attempt payload hex"),
    )
    .expect("decode exact Parliament attempt instruction");
    let attempt_instruction = attempt_instruction
        .as_any()
        .downcast_ref::<CreateParliamentGovernanceAttemptV1>()
        .expect("exact attempt instruction type");
    assert_eq!(
        attempt_response.governance_attempt_id,
        attempt_instruction.governance_attempt_id()
    );

    let transition_request = ParliamentTransitionDraftRequestV1 {
        version: PARLIAMENT_API_VERSION_V1,
        governance_attempt_id: GovernanceAttemptId::new([0x33; 32]),
        transition: ParliamentLifecycleTransitionV1::CompleteQualification,
    };
    let expected_digest = transition_request.transition.digest_v1();
    let transition_response =
        handle_gov_parliament_transition_draft(NoritoJson(transition_request))
            .await
            .expect("draft exact Parliament transition")
            .0;
    assert_eq!(transition_response.transition_digest, expected_digest);
    let transition_draft = &transition_response.tx_instructions[0];
    let transition_instruction = iroha_data_model::isi::decode_instruction_from_pair(
        &transition_draft.wire_id,
        &hex::decode(&transition_draft.payload_hex).expect("transition payload hex"),
    )
    .expect("decode exact Parliament transition instruction");
    let transition_instruction = transition_instruction
        .as_any()
        .downcast_ref::<SubmitParliamentLifecycleTransitionV1>()
        .expect("exact transition instruction type");
    assert_eq!(
        transition_instruction.transition.digest_v1(),
        expected_digest
    );
}
#[tokio::test]
async fn parliament_attempt_draft_enforces_the_end_to_end_retry_ceiling() {
    use iroha_data_model::governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal,
        MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1, ProposalKind,
    };

    let request = |attempt_sequence| ParliamentAttemptDraftRequestV1 {
        version: PARLIAMENT_API_VERSION_V1,
        proposal: ProposalKind::DeployContract(DeployContractProposal {
            proposal_operator: ALICE_ID.clone(),
            contract_address: sample_contract_address(),
            code_hash: ContractCodeHash::new([0x11; 32]),
            abi_hash: ContractAbiHash::new([0x22; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        }),
        attempt_sequence,
    };

    handle_gov_parliament_attempt_draft(NoritoJson(request(
        MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1,
    )))
    .await
    .expect("the final bounded Parliament attempt draft is admissible");
    let error = handle_gov_parliament_attempt_draft(NoritoJson(request(
        MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1 + 1,
    )))
    .await
    .expect_err("an over-limit Parliament attempt must not be framed");
    assert!(
        format!("{error:?}").contains("Parliament attempt sequence exceeds the V1 retry limit")
    );
}
#[tokio::test]
async fn parliament_attempt_draft_rejects_inexact_json_u64_proposals_before_framing() {
    use iroha_data_model::{
        governance::types::{
            FIRST_RELEASE_MAX_EXACT_JSON_U64, ProposalKind, RuntimeUpgradeProposal,
        },
        runtime::RuntimeUpgradeManifest,
    };

    let proposal = |start_height, end_height| {
        ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
            proposal_operator: ALICE_ID.clone(),
            manifest: RuntimeUpgradeManifest {
                name: "bounded Parliament runtime upgrade".to_owned(),
                description: "Torii exact JSON integer guard".to_owned(),
                abi_version: 1,
                abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
                added_syscalls: Vec::new(),
                added_pointer_types: Vec::new(),
                start_height,
                end_height,
                sbom_digests: Vec::new(),
                slsa_attestation: Vec::new(),
                provenance: Vec::new(),
            },
        })
    };
    let maximum = FIRST_RELEASE_MAX_EXACT_JSON_U64;
    for (start_height, end_height, expected_message) in [
        (
            maximum + 1,
            maximum + 1,
            "runtime-upgrade proposal start height exceeds the exact JSON integer maximum",
        ),
        (
            maximum,
            maximum + 1,
            "runtime-upgrade proposal end height exceeds the exact JSON integer maximum",
        ),
    ] {
        let request = ParliamentAttemptDraftRequestV1 {
            version: PARLIAMENT_API_VERSION_V1,
            proposal: proposal(start_height, end_height),
            attempt_sequence: 0,
        };
        let error = handle_gov_parliament_attempt_draft(NoritoJson(request))
            .await
            .expect_err("an inexact public JSON integer must not produce a draft");
        assert!(
            format!("{error:?}").contains(expected_message),
            "unexpected rejection for ({start_height}, {end_height}): {error:?}"
        );
    }

    let boundary_request = ParliamentAttemptDraftRequestV1 {
        version: PARLIAMENT_API_VERSION_V1,
        proposal: proposal(maximum - 1, maximum),
        attempt_sequence: 0,
    };
    let boundary_response = handle_gov_parliament_attempt_draft(NoritoJson(boundary_request))
        .await
        .expect("the exact JSON u64 boundary remains admissible")
        .0;
    assert_eq!(boundary_response.tx_instructions.len(), 1);
}
#[test]
fn unlock_stats_handler_cannot_reintroduce_an_expiry_index_scan() {
    let source = include_str!("../gov.rs");
    let start = source
        .find("pub async fn handle_gov_unlock_stats(")
        .expect("unlock stats handler");
    let tail = &source[start..];
    let end = tail
        .find("pub struct TxInstr")
        .expect("unlock stats handler terminator");
    let implementation = &tail[..end];
    assert!(implementation.contains("let view = state.query_view();"));
    assert!(implementation.contains("view.height()"));
    assert!(implementation.contains("governance_unlock_stats()"));
    assert!(!implementation.contains("governance_lock_expiry_index()"));
    assert!(!implementation.contains(".range("));
}
#[test]
fn scalar_governance_handlers_cannot_reintroduce_history_scans() {
    let source = include_str!("../gov.rs");
    let citizen_start = source
        .find("pub async fn handle_gov_citizen_count(")
        .expect("citizen count handler");
    let citizen_tail = &source[citizen_start..];
    let citizen_end = citizen_tail
        .find("/// GET /v1/gov/citizens/{account_id}")
        .expect("citizen count handler terminator");
    let citizen_handler = &citizen_tail[..citizen_end];
    assert!(citizen_handler.contains("world.citizens().len()"));
    assert!(!citizen_handler.contains("citizens().iter()"));
}
#[test]
fn optional_ballot_direction_is_closed() {
    for direction in [None, Some("Aye"), Some("Nay"), Some("Abstain")] {
        validate_optional_ballot_direction(direction).expect("canonical direction");
    }
    assert_eq!(
        validate_optional_ballot_direction(Some("aye")),
        Err("direction must be Aye, Nay, or Abstain".to_owned())
    );
    assert_eq!(
        validate_optional_ballot_direction(Some("Approve")),
        Err("direction must be Aye, Nay, or Abstain".to_owned())
    );
}
#[test]
fn canonicalize_hex32_value_accepts_only_declared_wire_forms() {
    let uppercase = "AB".repeat(32);
    let expected = "ab".repeat(32);
    for literal in [
        uppercase.clone(),
        format!("0X{uppercase}"),
        format!("BlAkE2b32:{uppercase}"),
        format!("BLAKE2B32:0x{uppercase}"),
    ] {
        assert_eq!(canonicalize_hex32_value(&literal), Some(expected.clone()));
    }
    for literal in [
        format!(":{uppercase}"),
        format!(" {uppercase}"),
        format!("{uppercase} "),
        format!("sha256:{uppercase}"),
        "ab".repeat(31),
    ] {
        assert_eq!(canonicalize_hex32_value(&literal), None);
    }
}
fn conversion_message(err: crate::Error) -> String {
    match err {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
        )) => message,
        other => panic!("expected conversion query error, got {other:?}"),
    }
}
fn canonical_literal(raw: &str) -> String {
    iroha_data_model::account::AccountId::parse_encoded(raw)
        .expect("literal parses")
        .to_string()
}
fn canonical_account(raw: &str) -> AccountId {
    AccountId::parse_encoded(raw).expect("literal parses")
}
fn noncanonical_literal(raw: &str) -> String {
    AccountId::parse_encoded(raw)
        .expect("literal parses")
        .to_string()
        .replacen("sora", "ｓｏｒａ", 1)
}
fn mk_basic_context() -> (Arc<State>, Arc<Queue>, Arc<ChainId>) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    let events = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    ));
    let chain_id: ChainId = "chain".parse().expect("chain id");
    (state, queue, Arc::new(chain_id))
}
fn bind_account_alias_for_test(state: &Arc<State>, account_id: &AccountId, alias: &str) {
    let label = iroha_data_model::account::rekey::AccountAlias::from_literal(
        alias,
        &state.nexus_snapshot().dataspace_catalog,
    )
    .expect("valid account alias");
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let world = tx.world_mut_for_testing();
    world
        .account_aliases_mut_for_testing()
        .insert(label.clone(), account_id.clone());
    let mut labels = world
        .account_aliases_by_account_mut_for_testing()
        .get(account_id)
        .cloned()
        .unwrap_or_default();
    labels.insert(label.clone());
    world
        .account_aliases_by_account_mut_for_testing()
        .insert(account_id.clone(), labels);
    world.replace_account_rekey_record_for_testing(
        iroha_data_model::account::rekey::AccountRekeyRecord::new(label, account_id.clone()),
    );
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit account alias for test");
}
fn seed_typed_proposal_fingerprint_for_ballot_test(
    state: &Arc<State>,
    proposer: &AccountId,
) -> String {
    let kind = deploy_contract_proposal_kind(
        proposer,
        &sample_contract_address(),
        &[0x71; 32],
        &[0x72; 32],
        None,
    );
    let proposal_id = kind.fingerprint();
    let record = iroha_core::state::GovernanceProposalRecord {
        proposer: proposer.clone(),
        kind,
        created_height: 1,
        status: iroha_core::state::GovernanceProposalStatus::Proposed,
    };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    transaction
        .world_mut_for_testing()
        .governance_proposals_mut()
        .insert(proposal_id, record);
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit typed proposal ballot guard fixture");
    hex::encode(proposal_id)
}
fn typed_proposal_selector_aliases(canonical: &str) -> [String; 5] {
    let uppercase = canonical.to_ascii_uppercase();
    let mixed = canonical
        .chars()
        .enumerate()
        .map(|(index, character)| {
            if index % 2 == 0 {
                character.to_ascii_uppercase()
            } else {
                character
            }
        })
        .collect::<String>();
    [
        canonical.to_owned(),
        uppercase.clone(),
        mixed,
        format!("0x{canonical}"),
        format!("0X{uppercase}"),
    ]
}
struct GovHarness {
    state: Arc<State>,
    queue: Arc<Queue>,
    chain_id: Arc<ChainId>,
    authority: AccountId,
    authority_keypair: KeyPair,
    asset_def_id: AssetDefinitionId,
    escrow: AccountId,
}
fn checked_governance_keypair(seed: u8, algorithm: Algorithm) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], algorithm)
        .expect("test governance fixture key derivation should succeed")
}
fn checked_governance_ed25519_keypair(seed: u8) -> KeyPair {
    checked_governance_keypair(seed, Algorithm::Ed25519)
}
fn checked_governance_bls_keypair(seed: u8) -> KeyPair {
    checked_governance_keypair(seed, Algorithm::BlsNormal)
}
#[test]
fn checked_governance_keypairs_use_fallible_seed_derivation() {
    let ed25519 = checked_governance_ed25519_keypair(0x90);
    let bls = checked_governance_bls_keypair(0x91);
    let bls_repeat = checked_governance_bls_keypair(0x91);
    let bls_other = checked_governance_bls_keypair(0x92);
    assert_eq!(ed25519.algorithm(), Algorithm::Ed25519);
    assert_eq!(bls.algorithm(), Algorithm::BlsNormal);
    assert_eq!(bls.public_key(), bls_repeat.public_key());
    assert_ne!(bls.public_key(), bls_other.public_key());
    assert!(
        KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
        "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
    );
}
fn mk_governance_harness(with_permissions: bool) -> GovHarness {
    let authority_keypair = checked_governance_ed25519_keypair(0x93);
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let authority = AccountId::of(authority_keypair.public_key().clone());
    let escrow: AccountId =
        iroha_config::parameters::defaults::governance::bond_escrow_account_id();
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let escrow_account = Account::new(escrow.clone()).build(&escrow);
    let asset_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("vote").expect("asset definition name"),
    );
    let asset_def = {
        let __asset_definition_id = asset_def_id.clone();
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "vote".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
    }
    .build(&authority);
    let asset = Asset::new(
        AssetId::new(asset_def_id.clone(), authority.clone()),
        Quantity::from(1_000u32),
    );
    let escrow_asset = Asset::new(
        AssetId::new(asset_def_id.clone(), escrow.clone()),
        Quantity::from(0u32),
    );
    let world = World::with_assets(
        [domain],
        [authority_account, escrow_account],
        [asset_def],
        [asset, escrow_asset],
        [],
    );
    if with_permissions {
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let contract_address_literal = contract_address.to_string();
        let propose = Permission::new(
            "CanProposeContractDeployment".to_string(),
            norito::json!({ "contract_address": contract_address_literal }),
        );
        let ballot = Permission::new(
            "CanSubmitGovernanceBallot".to_string(),
            norito::json!({ "referendum_id": "any" }),
        );
        let register_contract: Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        let mut world_block = world.block();
        let mut world_tx = world_block.transaction_without_telemetry(LaneConfig::default(), 0);
        let _ = world_tx.add_account_permission(&authority, propose);
        let _ = world_tx.add_account_permission(&authority, ballot);
        let _ = world_tx.add_account_permission(&authority, register_contract);
        world_tx.apply();
        world_block.commit();
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let chain_id: ChainId = "chain".parse().expect("chain id");
    let mut state = State::new_with_chain_for_testing(world, kura, query, chain_id.clone());
    let mut gov_cfg = state.gov.clone();
    gov_cfg.voting_asset_id = asset_def_id.clone();
    gov_cfg.citizenship_asset_id = asset_def_id.clone();
    gov_cfg.bond_escrow_account = escrow.clone();
    gov_cfg.citizenship_escrow_account = escrow.clone();
    gov_cfg.slash_receiver_account = escrow.clone();
    gov_cfg.min_bond_amount = 0_u64.into();
    gov_cfg.citizenship_bond_amount = 0_u64.into();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.conviction_step_blocks = 1;
    gov_cfg.max_conviction = 1;
    gov_cfg.window_span = 10;
    gov_cfg.min_enactment_delay = 0;
    gov_cfg.approval_threshold_q_num = 1;
    gov_cfg.approval_threshold_q_den = 1;
    gov_cfg.min_turnout = 1;
    state.set_gov(gov_cfg);
    let nexus = state.nexus_snapshot();
    let lane_manifests = Arc::new(
        iroha_core::governance::manifest::LaneManifestRegistry::from_config(
            &nexus.lane_catalog,
            &iroha_config::parameters::actual::GovernanceCatalog::default(),
            &iroha_config::parameters::actual::LaneRegistry::default(),
        ),
    );
    state.install_lane_manifests(&lane_manifests);
    let events = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    ));
    GovHarness {
        state: Arc::new(state),
        queue,
        chain_id: Arc::new(chain_id),
        authority,
        authority_keypair,
        asset_def_id,
        escrow,
    }
}
fn mk_manifest_provenance(
    keypair: &KeyPair,
    code_hash: [u8; 32],
    abi_hash: [u8; 32],
) -> ManifestProvenance {
    let manifest = ContractManifest {
        seiyaku_name: None,
        code_hash: Some(iroha_crypto::Hash::prehashed(code_hash)),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        states: None,
        kotoba: None,
        error_types: None,
        provenance: None,
    }
    .signed(keypair);
    manifest
        .provenance
        .expect("signed manifest should carry provenance")
}
fn sample_contract_address() -> iroha_data_model::smart_contract::ContractAddress {
    "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
        .parse()
        .expect("contract address")
}
fn install_governed_contract_for_test(
    harness: &GovHarness,
) -> (
    iroha_data_model::smart_contract::ContractAddress,
    iroha_crypto::Hash,
) {
    let (artifact, manifest) = ivm::KotodamaCompiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku GovernedReadFixture {
    view fn balance() -> bool { return true; }
    kotoage fn transfer() authorize("CanTransferGovernedFixture") {}
}
"#,
        )
        .expect("compile governed contract fixture");
    let verified =
        ivm::verify_contract_artifact(&artifact).expect("verify governed contract fixture");
    assert_eq!(
        manifest.signature_payload(),
        verified.manifest.signature_payload()
    );
    let signed_manifest = manifest.signed(&harness.authority_keypair);
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &harness.authority,
        91,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .expect("governed contract address");
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = harness.state.block(header);
    let mut transaction = block.transaction();
    let code_hash = register_code_bytes(&harness.authority, artifact, &mut transaction)
        .expect("register governed contract bytes");
    assert_eq!(code_hash, verified.code_hash);
    register_manifest(&harness.authority, signed_manifest, &mut transaction)
        .expect("register governed contract manifest");
    transaction
        .world_mut_for_testing()
        .bind_inactive_contract_subject_for_testing(
            contract_address.clone(),
            harness.authority.clone(),
        );
    activate_instance(
        &harness.authority,
        contract_address.clone(),
        1,
        code_hash,
        &mut transaction,
    )
    .expect("activate governed contract");
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit governed contract fixture");
    (contract_address, code_hash)
}
fn sample_sccp_route_governance_action()
-> iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1 {
    iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(
        iroha_data_model::bridge::SccpRouteKeyV1 {
            lane_id: iroha_data_model::bridge::SccpLaneIdV1 {
                source: iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
                target: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
            },
            route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.to_owned(),
            asset_key: iroha_sccp::SCCP_TAIRA_XOR_ASSET_KEY_V1.to_owned(),
            revision: 1,
        },
    )
}
fn sample_agenda_proposal(proposal_id: &str) -> AgendaProposalV1 {
    AgendaProposalV1 {
        version: iroha_data_model::ministry::AGENDA_PROPOSAL_VERSION_V1,
        proposal_id: proposal_id.to_string(),
        submitted_at_unix_ms: 1_775_000_000_000,
        language: "en".to_string(),
        action: iroha_data_model::ministry::AgendaProposalAction::AddToDenylist,
        summary: iroha_data_model::ministry::AgendaProposalSummary {
            title: "Blacklist SoraFS CID bafy-test".to_string(),
            motivation: "Evidence review recommends blocking the published SoraFS root CID."
                .to_string(),
            expected_impact:
                "Participating gateways would deny delivery while the evidence is reviewed."
                    .to_string(),
        },
        tags: vec!["fraud".to_string()],
        targets: vec![iroha_data_model::ministry::AgendaProposalTarget {
            label: "bafy-test".to_string(),
            hash_family: "sorafs-root-cid".to_string(),
            hash_hex: "11".repeat(32),
            reason: "Fraud review evidence for the selected SoraFS CID.".to_string(),
        }],
        evidence: vec![iroha_data_model::ministry::AgendaEvidenceAttachment {
            kind: iroha_data_model::ministry::AgendaEvidenceKind::Url,
            uri: "https://example.org/evidence/case-42".to_string(),
            digest_blake3_hex: None,
            description: Some("Public incident report".to_string()),
        }],
        submitter: iroha_data_model::ministry::AgendaProposalSubmitter {
            name: "Review Council".to_string(),
            contact: "review@example.org".to_string(),
            organization: Some("SoraFS Moderation".to_string()),
            pgp_fingerprint: None,
        },
        duplicates: Vec::new(),
    }
}
fn decode_governance_proposal_instruction(
    instr: &GovernanceProposalInstructionDraftV1,
) -> iroha_data_model::isi::InstructionBox {
    let bytes = hex::decode(&instr.payload_hex).expect("instruction payload hex");
    iroha_data_model::isi::decode_instruction_from_pair(&instr.wire_id, &bytes)
        .expect("instruction payload decode")
}
fn decode_tx_instruction(instr: &TxInstr) -> iroha_data_model::isi::InstructionBox {
    let bytes = hex::decode(&instr.payload_hex).expect("instruction payload hex");
    iroha_data_model::isi::decode_instruction_from_pair(&instr.wire_id, &bytes)
        .expect("instruction payload decode")
}
fn queue_instruction_skeleton(harness: &GovHarness, tx_instructions: &[TxInstr]) {
    let instructions = tx_instructions
        .iter()
        .map(|instruction| {
            let payload = hex::decode(&instruction.payload_hex).expect("instruction payload hex");
            iroha_data_model::isi::decode_instruction_from_pair(&instruction.wire_id, &payload)
                .expect("instruction payload decode")
        })
        .collect::<Vec<_>>();
    let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
        *harness.state.network_id_ref(),
        harness.authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .sign(harness.authority_keypair.private_key());
    let params = harness.state.view().world().parameters().clone();
    let accepted = iroha_core::tx::AcceptedTransaction::accept(
        tx,
        harness.state.network_id_ref(),
        params.sumeragi().max_clock_drift(),
        params.transaction(),
        harness.state.crypto().as_ref(),
    )
    .expect("accepted governance instruction skeleton");
    harness
        .queue
        .push(accepted, harness.state.view())
        .expect("push governance instruction skeleton");
}
fn queue_governance_proposal_instruction_skeleton(
    harness: &GovHarness,
    tx_instructions: &[GovernanceProposalInstructionDraftV1],
) {
    let instructions = tx_instructions
        .iter()
        .map(decode_governance_proposal_instruction)
        .collect::<Vec<_>>();
    let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
        *harness.state.network_id_ref(),
        harness.authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .sign(harness.authority_keypair.private_key());
    let params = harness.state.view().world().parameters().clone();
    let accepted = iroha_core::tx::AcceptedTransaction::accept(
        tx,
        harness.state.network_id_ref(),
        params.sumeragi().max_clock_drift(),
        params.transaction(),
        harness.state.crypto().as_ref(),
    )
    .expect("accepted governance proposal instruction skeleton");
    harness
        .queue
        .push(accepted, harness.state.view())
        .expect("push governance proposal instruction skeleton");
}
fn apply_queued_block_allow_errors(
    state: &Arc<State>,
    queue: &Arc<Queue>,
    expected_height: u64,
) -> Vec<bool> {
    let max_txs_in_block = core::num::NonZeroUsize::new(1024).expect("nonzero");
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), max_txs_in_block, &mut guards);
    if guards.is_empty() {
        return Vec::new();
    }
    let accepted: Vec<_> = guards
        .iter()
        .map(TransactionGuard::clone_accepted)
        .collect();
    let latest_block = state.view().latest_block();
    let leader = checked_governance_bls_keypair(0x94);
    let new_block = BlockBuilder::new(accepted)
        .chain(0, latest_block.as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    assert_eq!(
        new_block.header().height().get(),
        expected_height,
        "unexpected block height"
    );
    let mut state_block = state.block(new_block.header());
    let valid_block = new_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let committed_block = valid_block.commit_unchecked().unpack(|_| {});
    let block_ref = committed_block.as_ref();
    let errors = block_ref
        .external_transactions()
        .enumerate()
        .map(|(idx, _)| {
            let error = block_ref.error(idx);
            if let Some(error) = error {
                eprintln!("governance fixture transaction {idx} failed: {error:?}");
            }
            error.is_some()
        })
        .collect::<Vec<_>>();
    crate::test_utils::finalize_committed_block(state, state_block, committed_block);
    errors
}
#[tokio::test]
async fn citizen_status_reports_registered_record() {
    let harness = mk_governance_harness(false);
    let instruction = InstructionBox::from(RegisterCitizen {
        owner: harness.authority.clone(),
        amount: Quantity::zero(),
    });
    let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
        *harness.state.network_id_ref(),
        harness.authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(harness.authority_keypair.private_key());
    let params = harness.state.view().world().parameters().clone();
    let accepted = iroha_core::tx::AcceptedTransaction::accept(
        tx,
        harness.state.network_id_ref(),
        params.sumeragi().max_clock_drift(),
        params.transaction(),
        harness.state.crypto().as_ref(),
    )
    .expect("accepted register citizen transaction");
    harness
        .queue
        .push(accepted, harness.state.view())
        .expect("push register citizen transaction");
    assert_eq!(
        apply_queued_block_allow_errors(&harness.state, &harness.queue, 1),
        vec![false]
    );
    let response = handle_gov_citizen_status(
        harness.state.clone(),
        axum::extract::Path(harness.authority.to_string()),
        MaybeTelemetry::disabled(),
    )
    .await
    .expect("citizen status response")
    .0;
    assert!(response.is_citizen);
    assert_eq!(response.account_id, harness.authority.to_string());
    assert_eq!(response.amount.as_deref(), Some("0"));
    assert_eq!(response.bonded_height.as_deref(), Some("1"));
}
#[tokio::test]
async fn citizen_count_reports_exact_registry_total() {
    let harness = mk_governance_harness(false);
    assert_eq!(
        handle_gov_citizen_count(harness.state.clone())
            .await
            .expect("empty citizen count")
            .0
            .total,
        "0"
    );
    let instruction = InstructionBox::from(RegisterCitizen {
        owner: harness.authority.clone(),
        amount: Quantity::zero(),
    });
    let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
        *harness.state.network_id_ref(),
        harness.authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(harness.authority_keypair.private_key());
    let params = harness.state.view().world().parameters().clone();
    let accepted = iroha_core::tx::AcceptedTransaction::accept(
        tx,
        harness.state.network_id_ref(),
        params.sumeragi().max_clock_drift(),
        params.transaction(),
        harness.state.crypto().as_ref(),
    )
    .expect("accepted register citizen transaction");
    harness
        .queue
        .push(accepted, harness.state.view())
        .expect("push register citizen transaction");
    assert_eq!(
        apply_queued_block_allow_errors(&harness.state, &harness.queue, 1),
        vec![false]
    );
    let response = handle_gov_citizen_count(harness.state.clone())
        .await
        .expect("citizen count response")
        .0;
    assert_eq!(response.total, "1");
}
#[test]
fn serde_shapes_compile() {
    let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let req = DeployContractProposalDraftRequestV1 {
        proposal_operator: ALICE_ID.clone(),
        contract_address: Some(sample_contract_address()),
        contract_alias: None,
        abi_version: AbiVersion::new(1),
        code_hash: ContractCodeHash::new([0xAA; 32]),
        abi_hash: ContractAbiHash::new(canonical_abi),
        manifest_provenance: None,
    };
    let s = norito::json::to_json(&req).unwrap();
    let _: DeployContractProposalDraftRequestV1 = norito::json::from_str(&s).unwrap();
    let sccp = SccpRouteGovernanceProposalDraftRequestV1 {
        action: sample_sccp_route_governance_action(),
    };
    let json = norito::json::to_json(&sccp).expect("encode SCCP governance DTO");
    let decoded: SccpRouteGovernanceProposalDraftRequestV1 =
        norito::json::from_str(&json).expect("decode SCCP governance DTO");
    assert_eq!(decoded.action, sccp.action);
}
#[tokio::test]
async fn protected_namespaces_set_drafts_transaction_without_mutating_state() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let before = handle_gov_protected_get(state.clone())
        .await
        .expect("protected namespaces get")
        .0;
    assert!(!before.found);
    assert!(before.namespaces.is_empty());
    let response = handle_gov_protected_set(
        state.clone(),
        MaybeTelemetry::disabled(),
        NoritoJson(ProtectedNamespacesDto {
            namespaces: vec!["apps".to_owned(), "system".to_owned()],
            authority: None,
        }),
    )
    .await
    .expect("protected namespaces draft")
    .0;
    assert!(response.ok);
    assert!(!response.submitted);
    assert_eq!(response.namespace_count, 2);
    assert_eq!(response.tx_instructions.len(), 1);
    assert!(response.signable_transaction_b64.is_none());
    let after = handle_gov_protected_get(state)
        .await
        .expect("protected namespaces get")
        .0;
    assert!(!after.found);
    assert!(after.namespaces.is_empty());
}
#[tokio::test]
async fn protected_namespaces_get_rejects_a_present_malformed_policy() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let id = iroha_data_model::parameter::CustomParameterId::new(
        iroha_core::smartcontracts::code::PROTECTED_CONTRACT_NAMESPACES_PARAMETER
            .parse()
            .expect("protected namespace parameter id"),
    );
    let parameter = iroha_data_model::parameter::custom::CustomParameter::new(
        id,
        iroha_primitives::json::Json::new("apps"),
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    transaction
        .world_mut_for_testing()
        .parameters_mut_for_testing()
        .get_mut()
        .set_parameter(iroha_data_model::parameter::Parameter::Custom(parameter));
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit malformed protected namespace fixture");

    let error = handle_gov_protected_get(state)
        .await
        .expect_err("malformed persisted policy must not project as an empty policy");
    assert!(
        matches!(
            &error,
            crate::Error::Query(iroha_data_model::ValidationFail::InternalError(message))
                if message.contains("must be an array of strings")
        ),
        "unexpected malformed-policy error: {error:?}"
    );
}
#[tokio::test]
async fn protected_namespaces_rejects_noncanonical_tokens_before_drafting() {
    let (state, _queue, _chain_id) = mk_basic_context();
    for namespace in ["", " system", "system ", "system namespace", "systèm"] {
        let error = handle_gov_protected_set(
            state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(ProtectedNamespacesDto {
                namespaces: vec![namespace.to_owned()],
                authority: None,
            }),
        )
        .await
        .expect_err("noncanonical namespace must fail before drafting");
        assert!(error.to_string().contains("namespaces[0]"));
    }
}
#[tokio::test]
async fn protected_namespaces_set_returns_checked_signable_payload_for_authority() {
    let harness = mk_governance_harness(false);
    let response = handle_gov_protected_set(
        harness.state.clone(),
        MaybeTelemetry::disabled(),
        NoritoJson(ProtectedNamespacesDto {
            namespaces: vec!["apps".to_owned(), "system".to_owned()],
            authority: Some(harness.authority.to_string()),
        }),
    )
    .await
    .expect("protected namespaces signable draft")
    .0;
    assert!(response.ok);
    assert!(!response.submitted);
    assert_eq!(response.namespace_count, 2);
    assert_eq!(response.tx_instructions.len(), 1);
    let signable_payload = response
        .signable_transaction_b64
        .expect("authority should produce a signable transaction payload");
    let tx_bytes = base64::engine::general_purpose::STANDARD
        .decode(signable_payload.as_bytes())
        .expect("decode signable payload");
    let payload: iroha_data_model::transaction::signed::TransactionPayload = {
        let _guard = norito::core::PayloadCtxGuard::enter(&tx_bytes);
        let mut cursor = std::io::Cursor::new(tx_bytes.as_slice());
        norito::codec::Decode::decode(&mut cursor).expect("decode transaction payload")
    };
    assert_eq!(payload.authority, harness.authority);
    assert_eq!(payload.instructions.instruction_count(), 1);
}
#[tokio::test]
async fn propose_deploy_builds_instruction_skeleton() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let code_hash_bytes = [0x11; 32];
    let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let provenance_key =
        KeyPair::try_from_seed(b"proposal-id-provenance".to_vec(), Algorithm::Ed25519)
            .expect("derive proposal provenance fixture key");
    let provenance = mk_manifest_provenance(&provenance_key, [0x11; 32], canonical_abi);
    let dto = DeployContractProposalDraftRequestV1 {
        proposal_operator: ALICE_ID.clone(),
        contract_address: Some(sample_contract_address()),
        contract_alias: None,
        abi_version: AbiVersion::new(1),
        code_hash: ContractCodeHash::new(code_hash_bytes),
        abi_hash: ContractAbiHash::new(canonical_abi),
        manifest_provenance: Some(provenance.clone()),
    };
    let res = handle_gov_propose_deploy(state, NoritoJson(dto))
        .await
        .expect("handler ok");
    let body = res.0;
    assert_eq!(body.tx_instructions.len(), 1);
    let expected_id = deploy_contract_proposal_kind(
        &ALICE_ID,
        &sample_contract_address(),
        &code_hash_bytes,
        &canonical_abi,
        Some(provenance.clone()),
    )
    .fingerprint();
    assert_eq!(body.proposal_id, ProposalContentId::new(expected_id));
    assert_ne!(
        expected_id,
        deploy_contract_proposal_kind(
            &ALICE_ID,
            &sample_contract_address(),
            &code_hash_bytes,
            &canonical_abi,
            None,
        )
        .fingerprint(),
        "proposal id must bind the complete manifest provenance"
    );
    // Payload decodes to the exact certificate-only proposal instruction.
    let instruction = decode_governance_proposal_instruction(&body.tx_instructions[0]);
    let decoded = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::governance::ProposeDeployContract>()
        .expect("exact deploy-contract proposal instruction");
    assert_eq!(decoded.contract_address, sample_contract_address());
    assert_eq!(decoded.code_hash, ContractCodeHash::new(code_hash_bytes));
    assert_eq!(decoded.abi_hash, ContractAbiHash::new(canonical_abi));
    assert_eq!(decoded.abi_version, AbiVersion::new(1));
    assert_eq!(decoded.manifest_provenance, Some(provenance));
}
#[tokio::test]
async fn propose_sccp_route_governance_builds_exact_instruction_and_proposal_id() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let action = sample_sccp_route_governance_action();
    let anchor = iroha_data_model::isi::bridge::SccpRouteGovernanceAnchorV1 {
        network_id: *state.network_id_ref(),
        action: action.clone(),
    };
    let expected_id = sccp_route_governance_proposal_kind(&anchor).fingerprint();
    let response = handle_gov_propose_sccp_route_governance(
        state,
        NoritoJson(SccpRouteGovernanceProposalDraftRequestV1 {
            action: action.clone(),
        }),
    )
    .await
    .expect("valid SCCP governance draft")
    .0;
    assert_eq!(response.proposal_id, ProposalContentId::new(expected_id));
    assert_eq!(response.tx_instructions.len(), 1);
    let instruction = decode_governance_proposal_instruction(&response.tx_instructions[0]);
    let decoded = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::governance::ProposeSccpRouteGovernance>()
        .expect("exact SCCP governance instruction");
    assert_eq!(decoded.anchor, anchor);
}
#[tokio::test]
async fn propose_sccp_route_governance_rejects_invalid_action_before_drafting() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let mut action = sample_sccp_route_governance_action();
    let iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(key) = &mut action
    else {
        unreachable!("fixture is a remove action");
    };
    key.revision = 0;
    let error = handle_gov_propose_sccp_route_governance(
        state,
        NoritoJson(SccpRouteGovernanceProposalDraftRequestV1 { action }),
    )
    .await
    .expect_err("invalid SCCP action must fail before returning a skeleton");
    assert!(
        format!("{error:?}").contains("invalid SCCP route governance action"),
        "unexpected error: {error:?}"
    );
}
#[tokio::test]
async fn propose_sccp_route_governance_rejects_inexact_json_numbers_before_drafting() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(key) =
        sample_sccp_route_governance_action()
    else {
        unreachable!("fixture is a remove action")
    };
    let action = iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::SetActivation(
        iroha_data_model::isi::bridge::SccpSetRouteActivationV1 {
            key,
            expected_current: iroha_data_model::bridge::SccpRouteActivationV1::InboundOnly,
            next: iroha_data_model::bridge::SccpRouteActivationV1::Retired,
            inbound_finality_cutoff: Some(iroha_data_model::bridge::SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0x91; 32],
                max_anchor_interval_height:
                    iroha_data_model::parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64 + 1,
            }),
        },
    );
    assert!(action.validate_static().is_ok());
    let error = handle_gov_propose_sccp_route_governance(
        state,
        NoritoJson(SccpRouteGovernanceProposalDraftRequestV1 { action }),
    )
    .await
    .expect_err("inexact SCCP JSON numbers must fail before returning a skeleton");
    assert!(
        format!("{error:?}").contains("exact JSON integer maximum"),
        "unexpected precision rejection: {error:?}"
    );
}
#[test]
fn propose_sccp_route_governance_rejects_retired_lifecycle_controls() {
    let canonical = norito::json::to_json(&SccpRouteGovernanceProposalDraftRequestV1 {
        action: sample_sccp_route_governance_action(),
    })
    .expect("canonical SCCP governance DTO");
    let body = canonical.strip_suffix('}').expect("DTO JSON is an object");
    for (field, value) in [
        ("mode", "\"Zk\""),
        ("window", "{\"lower\":10,\"upper\":20}"),
    ] {
        let injected = format!("{body},\"{field}\":{value}}}");
        let error = norito::json::from_str::<SccpRouteGovernanceProposalDraftRequestV1>(&injected)
            .expect_err("retired SCCP lifecycle control must reject");
        assert!(error.to_string().contains(field), "{field}: {error}");
    }
}
#[test]
fn sccp_route_governance_dto_rejects_retired_signing_and_unknown_fields() {
    let dto = SccpRouteGovernanceProposalDraftRequestV1 {
        action: sample_sccp_route_governance_action(),
    };
    let canonical = norito::json::to_json(&dto).expect("canonical SCCP governance DTO");
    let body = canonical.strip_suffix('}').expect("DTO JSON is an object");
    for (field, value) in [
        ("authority", "\"sorau...\""),
        ("private_key", "\"secret\""),
        ("manifest", "null"),
        ("window", "null"),
        ("mode", "\"Zk\""),
        ("future_action_policy", "null"),
    ] {
        let injected = format!("{body},\"{field}\":{value}}}");
        let error = norito::json::from_str::<SccpRouteGovernanceProposalDraftRequestV1>(&injected)
            .expect_err("retired or unknown SCCP draft field must reject");
        assert!(
            error.to_string().contains(field) || error.to_string().contains("unknown field"),
            "{field}: {error}"
        );
    }
}
#[test]
fn governance_mutation_dtos_reject_retired_signing_fields_during_decode() {
    macro_rules! assert_rejects_field {
            ($field:expr; $($request:ty),+ $(,)?) => {
                $(
                    let input = format!(r#"{{"{}":"must-not-cross-torii"}}"#, $field);
                    let error = norito::json::from_str::<$request>(&input)
                    .expect_err("retired signing field must fail JSON admission");
                    let message = error.to_string();
                    assert!(
                        message.contains("unknown field") && message.contains($field),
                        "{} admitted retired field `{}`: {message}",
                        stringify!($request),
                        $field,
                    );
                )+
            };
        }
    for field in [
        "private_key",
        "privateKey",
        "private_key_hex",
        "privateKeyHex",
        "private_key_bytes",
        "privateKeyBytes",
        "private_key_seed",
        "privateKeySeed",
        "private_key_multihash",
        "privateKeyMultihash",
        "private_key_algorithm",
        "privateKeyAlgorithm",
    ] {
        assert_rejects_field!(
            field;
            DeployContractProposalDraftRequestV1,
            SccpRouteGovernanceProposalDraftRequestV1,
            MinistryAgendaProposalDraftDto,
            PlainBallotDto,
            ZkBallotV1Dto,
            ZkBallotV1BallotProofDto,
            ProtectedNamespacesDto,
        );
    }
    assert_rejects_field!(
        "authority";
        DeployContractProposalDraftRequestV1,
    );
}
#[test]
fn governance_nested_request_types_reject_unknown_fields() {
    let ballot = iroha_data_model::isi::governance::BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1, 2, 3, 4],
        root_hint: None,
        owner: None,
        nullifier: None,
        amount: None,
        duration_blocks: None,
        direction: None,
    };
    let canonical = norito::json::to_json(&ballot).expect("encode canonical ballot proof");
    let body = canonical
        .strip_suffix('}')
        .expect("ballot proof JSON is an object");
    let injected = format!(r#"{body},"privateKeySeed":"secret"}}"#);
    let error = norito::json::from_str::<iroha_data_model::isi::governance::BallotProof>(&injected)
        .expect_err("ballot proof must be closed");
    assert!(error.to_string().contains("privateKeySeed"));
    let keypair =
        KeyPair::try_from_seed(b"closed-manifest-provenance".to_vec(), Algorithm::Ed25519)
            .expect("derive manifest provenance fixture key");
    let provenance = mk_manifest_provenance(&keypair, [0x11; 32], [0x22; 32]);
    let canonical =
        norito::json::to_json(&provenance).expect("encode canonical manifest provenance");
    let body = canonical
        .strip_suffix('}')
        .expect("manifest provenance JSON is an object");
    let injected = format!(r#"{body},"privateKeyAlgorithm":"secret"}}"#);
    let error = norito::json::from_str::<ManifestProvenance>(&injected)
        .expect_err("manifest provenance must be closed");
    assert!(error.to_string().contains("privateKeyAlgorithm"));
}
#[test]
fn propose_deploy_rejects_retired_lifecycle_controls_during_decode() {
    let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let dto = DeployContractProposalDraftRequestV1 {
        proposal_operator: ALICE_ID.clone(),
        contract_address: Some(sample_contract_address()),
        contract_alias: None,
        abi_version: AbiVersion::new(1),
        code_hash: ContractCodeHash::new([0x11; 32]),
        abi_hash: ContractAbiHash::new(canonical_abi),
        manifest_provenance: None,
    };
    let canonical = norito::json::to_json(&dto).expect("canonical deploy request");
    let body = canonical.strip_suffix('}').expect("DTO JSON is an object");
    for (field, value) in [
        ("mode", "\"Zk\""),
        ("window", "{\"lower\":10,\"upper\":20}"),
    ] {
        let injected = format!("{body},\"{field}\":{value}}}");
        let error = norito::json::from_str::<DeployContractProposalDraftRequestV1>(&injected)
            .expect_err("retired deploy lifecycle control must fail typed decoding");
        assert!(error.to_string().contains(field), "{field}: {error}");
    }
}
#[tokio::test]
async fn propose_deploy_accepts_only_exact_abi_v1_label() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    for abi_version in [0, 2, u16::MAX] {
        let dto = DeployContractProposalDraftRequestV1 {
            proposal_operator: ALICE_ID.clone(),
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: AbiVersion::new(abi_version),
            code_hash: ContractCodeHash::new([0x11; 32]),
            abi_hash: ContractAbiHash::new(canonical_abi),
            manifest_provenance: None,
        };
        let error = handle_gov_propose_deploy(state.clone(), NoritoJson(dto))
            .await
            .expect_err("only the exact first-release ABI label is accepted");
        assert!(
            format!("{error:?}").contains("unsupported abi_version"),
            "{abi_version}: {error:?}"
        );
    }
}
#[tokio::test]
async fn propose_deploy_rejects_mismatched_abi_hash() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let dto = DeployContractProposalDraftRequestV1 {
        proposal_operator: ALICE_ID.clone(),
        contract_address: Some(sample_contract_address()),
        contract_alias: None,
        abi_version: AbiVersion::new(1),
        code_hash: ContractCodeHash::new([0x11; 32]),
        abi_hash: ContractAbiHash::new([0x22; 32]),
        manifest_provenance: None,
    };
    let err = handle_gov_propose_deploy(state, NoritoJson(dto))
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("abi_hash does not match canonical hash"));
}
#[tokio::test]
async fn ministry_agenda_draft_returns_instruction_skeleton_and_signable_payload() {
    let harness = mk_governance_harness(true);
    let proposal = sample_agenda_proposal("AC-2026-241");
    let response = handle_ministry_agenda_proposal_draft(
        harness.state.clone(),
        MaybeTelemetry::disabled(),
        NoritoJson(MinistryAgendaProposalDraftDto {
            proposal: proposal.clone(),
            authority: harness.authority.to_string(),
        }),
    )
    .await
    .expect("draft ok");
    let MinistryAgendaProposalDraftOutcome::Draft(body) = response else {
        panic!("expected successful draft");
    };
    assert!(body.ok);
    assert_eq!(body.agenda_proposal_id, proposal.proposal_id);
    assert_eq!(body.authority, harness.authority.to_string());
    assert_eq!(body.tx_instructions.len(), 1);
    let tx_bytes = base64::engine::general_purpose::STANDARD
        .decode(body.signable_transaction_b64.as_bytes())
        .expect("decode signable payload");
    let payload: iroha_data_model::transaction::signed::TransactionPayload = {
        let _guard = norito::core::PayloadCtxGuard::enter(&tx_bytes);
        let mut cursor = std::io::Cursor::new(tx_bytes.as_slice());
        norito::codec::Decode::decode(&mut cursor).expect("decode transaction payload")
    };
    assert_eq!(payload.authority, harness.authority);
    assert_eq!(payload.instructions.instruction_count(), 1);
}
#[tokio::test]
async fn ministry_agenda_draft_rejects_noncanonical_authority_without_trimming() {
    let harness = mk_governance_harness(true);
    let authority = format!(" {}", harness.authority);
    let error = handle_ministry_agenda_proposal_draft(
        harness.state,
        MaybeTelemetry::disabled(),
        NoritoJson(MinistryAgendaProposalDraftDto {
            proposal: sample_agenda_proposal("AC-2026-240"),
            authority,
        }),
    )
    .await
    .expect_err("whitespace authority alias must fail before drafting");
    assert!(
        format!("{error:?}").contains("canonical I105"),
        "unexpected error: {error:?}"
    );
}
#[tokio::test]
async fn ministry_agenda_get_returns_missing_then_persisted_record() {
    let harness = mk_governance_harness(true);
    let proposal = sample_agenda_proposal("AC-2026-242");
    for invalid in [" AC-2026-242", "AC-2026-242 ", "ac-2026-242", "AC-2026-42"] {
        let error = handle_ministry_agenda_proposal_get(
            harness.state.clone(),
            axum::extract::Path(invalid.to_owned()),
        )
        .await
        .expect_err("noncanonical proposal id must fail before lookup");
        assert!(format!("{error:?}").contains("AC-YYYY-###"));
    }
    let missing = handle_ministry_agenda_proposal_get(
        harness.state.clone(),
        axum::extract::Path(proposal.proposal_id.clone()),
    )
    .await
    .expect("lookup ok")
    .0;
    assert!(!missing.found);
    assert!(missing.record.is_none());
    let draft = handle_ministry_agenda_proposal_draft(
        harness.state.clone(),
        MaybeTelemetry::disabled(),
        NoritoJson(MinistryAgendaProposalDraftDto {
            proposal: proposal.clone(),
            authority: harness.authority.to_string(),
        }),
    )
    .await
    .expect("draft ok");
    let MinistryAgendaProposalDraftOutcome::Draft(body) = draft else {
        panic!("expected successful draft");
    };
    queue_instruction_skeleton(&harness, &body.tx_instructions);
    let applied = crate::test_utils::apply_queued_in_one_block(
        &harness.state,
        &harness.queue,
        harness.chain_id.as_ref(),
        1,
    );
    assert_eq!(applied, 1);
    let persisted = handle_ministry_agenda_proposal_get(
        harness.state.clone(),
        axum::extract::Path(proposal.proposal_id.clone()),
    )
    .await
    .expect("lookup ok")
    .0;
    assert!(persisted.found);
    let record = persisted.record.expect("record");
    assert_eq!(record.proposal, proposal);
    assert_eq!(record.authority, harness.authority);
    assert!(!record.submitted_tx_hash_hex.is_empty());
    assert_eq!(record.submitted_height, 1);
}
#[tokio::test]
async fn ministry_agenda_draft_preflights_duplicate_proposal_ids() {
    let harness = mk_governance_harness(true);
    let proposal = sample_agenda_proposal("AC-2026-243");
    let draft = handle_ministry_agenda_proposal_draft(
        harness.state.clone(),
        MaybeTelemetry::disabled(),
        NoritoJson(MinistryAgendaProposalDraftDto {
            proposal: proposal.clone(),
            authority: harness.authority.to_string(),
        }),
    )
    .await
    .expect("draft ok");
    let MinistryAgendaProposalDraftOutcome::Draft(body) = draft else {
        panic!("expected successful draft");
    };
    queue_instruction_skeleton(&harness, &body.tx_instructions);
    let applied = crate::test_utils::apply_queued_in_one_block(
        &harness.state,
        &harness.queue,
        harness.chain_id.as_ref(),
        1,
    );
    assert_eq!(applied, 1);
    let duplicate = handle_ministry_agenda_proposal_draft(
        harness.state.clone(),
        MaybeTelemetry::disabled(),
        NoritoJson(MinistryAgendaProposalDraftDto {
            proposal,
            authority: harness.authority.to_string(),
        }),
    )
    .await
    .expect("duplicate preflight ok");
    let MinistryAgendaProposalDraftOutcome::Duplicate(body) = duplicate else {
        panic!("expected duplicate summary");
    };
    assert!(body.found);
    assert_eq!(
        body.record
            .as_ref()
            .map(|record| record.proposal.proposal_id.as_str()),
        Some("AC-2026-243")
    );
}
#[tokio::test]
async fn ballot_plain_builds_instruction_skeleton() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let canonical = canonical_literal(ACCOUNT_AUTHORITY);
    // Build DTO via JSON to ensure serde shape is satisfied
    let body = crate::json_object(vec![
        crate::json_entry("authority", canonical.clone()),
        crate::json_entry("network_id", *state.network_id_ref()),
        crate::json_entry("referendum_id", "r1"),
        crate::json_entry("owner", canonical.clone()),
        crate::json_entry("amount", "100"),
        crate::json_entry("duration_blocks", "600"),
        crate::json_entry("direction", "Aye"),
    ]);
    let parsed: PlainBallotDto =
        norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
    let res = handle_gov_ballot_plain_with_policy(
        state,
        &authenticated,
        NoritoJson(parsed),
        MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok");
    let body = res.0;
    assert!(body.drafted);
    assert!(body.tx_instructions.len() == 1);
}
#[tokio::test]
async fn standalone_plain_ballot_rejects_stored_typed_proposal_fingerprint() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let canonical = canonical_literal(ACCOUNT_AUTHORITY);
    let proposal_id = seed_typed_proposal_fingerprint_for_ballot_test(&state, &authenticated);
    for selector in typed_proposal_selector_aliases(&proposal_id) {
        let dto = PlainBallotDto {
            authority: canonical.clone(),
            network_id: *state.network_id_ref(),
            referendum_id: selector.clone(),
            owner: canonical.clone(),
            amount: 100_u64.into(),
            duration_blocks: "600".to_owned(),
            direction: "Aye".to_owned(),
        };
        crate::frame_test_support::assert_current_frame(&dto, "iroha_torii::gov::PlainBallotDto");
        let error = handle_gov_ballot_plain_with_policy(
            Arc::clone(&state),
            &authenticated,
            NoritoJson(dto),
            MaybeTelemetry::disabled(),
        )
        .await
        .expect_err("typed proposal alias must not enter the standalone plain ballot path");
        let message = conversion_message(error);
        assert_eq!(
            message,
            "typed proposal fingerprints use the authenticated Parliament lifecycle, not standalone referendum ballots",
            "unexpected typed rejection for {selector:?}"
        );
    }
}
#[tokio::test]
async fn ballot_plain_accepts_account_aliases() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authority = AccountId::parse_encoded(ACCOUNT_AUTHORITY).expect("account parses");
    bind_account_alias_for_test(&state, &authority, "ballot@universal");
    let body = crate::json_object(vec![
        crate::json_entry("authority", "ballot@universal"),
        crate::json_entry("network_id", *state.network_id_ref()),
        crate::json_entry("referendum_id", "r1"),
        crate::json_entry("owner", "ballot@universal"),
        crate::json_entry("amount", "100"),
        crate::json_entry("duration_blocks", "600"),
        crate::json_entry("direction", "Aye"),
    ]);
    let parsed: PlainBallotDto =
        norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
    let res = handle_gov_ballot_plain_with_policy(
        state,
        &authority,
        NoritoJson(parsed),
        MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok");
    assert!(res.0.drafted);
    assert_eq!(res.0.tx_instructions.len(), 1);
}
#[tokio::test]
async fn ballot_plain_rejects_authority_mismatch() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let canonical_authority = canonical_literal(ACCOUNT_AUTHORITY);
    let canonical_owner = canonical_literal(ACCOUNT_OWNER_ALT);
    let body = crate::json_object(vec![
        crate::json_entry("authority", canonical_authority),
        crate::json_entry("network_id", *state.network_id_ref()),
        crate::json_entry("referendum_id", "r1"),
        crate::json_entry("owner", canonical_owner),
        crate::json_entry("amount", "100"),
        crate::json_entry("duration_blocks", "600"),
        crate::json_entry("direction", "Aye"),
    ]);
    let parsed: PlainBallotDto =
        norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
    let err = handle_gov_ballot_plain_with_policy(
        state,
        &authenticated,
        NoritoJson(parsed),
        MaybeTelemetry::for_tests(),
    )
    .await
    .unwrap_err();
    let s = format!("{err:?}");
    assert!(s.contains("authority must equal owner"));
}
#[tokio::test]
async fn ballot_plain_accepts_raw_public_key_literals() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let body = crate::json_object(vec![
        crate::json_entry("authority", ACCOUNT_AUTHORITY),
        crate::json_entry("network_id", *state.network_id_ref()),
        crate::json_entry("referendum_id", "r1"),
        crate::json_entry("owner", ACCOUNT_AUTHORITY),
        crate::json_entry("amount", "100"),
        crate::json_entry("duration_blocks", "600"),
        crate::json_entry("direction", "Aye"),
    ]);
    let parsed: PlainBallotDto =
        norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
    handle_gov_ballot_plain_with_policy(
        state,
        &authenticated,
        NoritoJson(parsed),
        MaybeTelemetry::for_tests(),
    )
    .await
    .expect("raw public key literals should be accepted");
}
include!("network_id_tests.rs");
#[test]
fn exact_governance_path_token_grammar_rejects_aliasing_characters() {
    for valid in ["referendum-1", "A9_selector~with.dots"] {
        validate_governance_selector_v1("referendum id", valid)
            .expect("a bounded RFC 3986 unreserved selector is valid");
    }
    for invalid in [
        "",
        ".",
        "..",
        ".hidden",
        "a/b",
        "a%2Fb",
        "投票",
        " referendum",
        "referendum ",
        "refer\nendum",
        "refer\u{7f}endum",
    ] {
        validate_governance_selector_v1("referendum id", invalid)
            .expect_err("noncanonical path selectors must fail closed");
    }
    validate_governance_selector_v1("referendum id", &"a".repeat(128))
        .expect("the exact length boundary is valid");
    validate_governance_selector_v1("referendum id", &"a".repeat(129))
        .expect_err("overlong selectors must fail closed");
}
#[tokio::test]
async fn governance_get_handlers_reject_noncanonical_selectors_before_lookup() {
    fn assert_conversion(error: &crate::Error) {
        assert!(
            matches!(
                error,
                crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(_)
                ))
            ),
            "expected query conversion error, got {error:?}"
        );
    }
    let state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    for invalid in ["AA".repeat(32), format!("0x{}", "aa".repeat(32))] {
        let error = handle_gov_get_proposal(state.clone(), axum::extract::Path(invalid))
            .await
            .expect_err("proposal aliases must fail before lookup");
        assert_conversion(&error);
    }
    let missing = handle_gov_get_proposal(state.clone(), axum::extract::Path("aa".repeat(32)))
        .await
        .expect("an exact lowercase proposal id reaches lookup");
    assert!(!missing.0.found);
    for invalid in [
        "a/b".to_owned(),
        ".".to_owned(),
        ".hidden".to_owned(),
        "a%2Fb".to_owned(),
        "投票".to_owned(),
        "a".repeat(129),
    ] {
        let error = handle_gov_get_referendum(state.clone(), axum::extract::Path(invalid.clone()))
            .await
            .expect_err("noncanonical selectors must fail before referendum lookup");
        assert_conversion(&error);
    }
    let error = handle_gov_get_locks(state.clone(), axum::extract::Path(" referendum".to_owned()))
        .await
        .expect_err("leading whitespace must fail before lock lookup");
    assert_conversion(&error);
    let error =
        handle_gov_get_referendum(state.clone(), axum::extract::Path("referendum ".to_owned()))
            .await
            .expect_err("trailing whitespace must fail before referendum lookup");
    assert_conversion(&error);
    let error = handle_gov_get_tally(state, axum::extract::Path("refer\nendum".to_owned()))
        .await
        .expect_err("control characters must fail before tally lookup");
    assert_conversion(&error);
}
#[tokio::test]
async fn gov_get_tally_applies_conviction_factor() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let mut cfg = state.gov.clone();
    cfg.conviction_step_blocks = 2;
    cfg.max_conviction = 4;
    state.set_gov(cfg);
    let custody = generic_lock_custody(&state);
    let rid = "rid-tally-conviction".to_string();
    let header = BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    {
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        stx.world.governance_referenda_mut().insert(
            rid.clone(),
            GovernanceReferendumRecord {
                h_start: 1,
                h_end: 10,
                status: GovernanceReferendumStatus::Open,
                mode: GovernanceReferendumMode::Plain,
            },
        );
        let mut locks = GovernanceLocksForReferendum::default();
        locks.locks.insert(
            ALICE_ID.clone(),
            GovernanceLockRecord {
                owner: ALICE_ID.clone(),
                amount: 9_u64.into(),
                slashed: Quantity::zero(),
                expiry_height: 100,
                direction: 0,
                duration_blocks: 4,
                custody,
            },
        );
        stx.world.governance_locks_mut().insert(rid.clone(), locks);
        stx.apply();
        let iroha_core::state::StateBlock { world, .. } = sblock;
        world.commit();
    }
    let res = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
        .await
        .expect("handler ok");
    let body = res.0;
    assert_eq!(body.approve, 9);
    assert_eq!(body.reject, 0);
    assert_eq!(body.evaluated_block_hash.len(), 64);
    assert!(
        body.evaluated_block_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
}
#[tokio::test]
async fn gov_get_tally_uses_referendum_end_for_closed_plain_view() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let mut cfg = state.gov.clone();
    cfg.conviction_step_blocks = 1;
    cfg.max_conviction = 1;
    state.set_gov(cfg);
    let rid = "rid-tally-closed-lock".to_string();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let block_hash = iroha_crypto::HashOf::new(&header);
    let custody = generic_lock_custody(&state);
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.governance_referenda_mut().insert(
            rid.clone(),
            GovernanceReferendumRecord {
                h_start: 0,
                h_end: 0,
                status: GovernanceReferendumStatus::Closed,
                mode: GovernanceReferendumMode::Plain,
            },
        );
        let mut locks = GovernanceLocksForReferendum::default();
        locks.locks.insert(
            ALICE_ID.clone(),
            GovernanceLockRecord {
                owner: ALICE_ID.clone(),
                amount: 9_u64.into(),
                slashed: Quantity::zero(),
                expiry_height: 0,
                direction: 0,
                duration_blocks: 0,
                custody,
            },
        );
        tx.world.governance_locks_mut().insert(rid.clone(), locks);
        tx.apply();
        let iroha_core::state::StateBlock { world, .. } = block;
        world.commit();
    }
    state.push_block_hash_for_testing(block_hash);

    let response = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
        .await
        .expect("closed PLAIN tally");
    assert_eq!(response.0.evaluated_block_height, 1);
    assert_eq!(response.0.approve, 3);
    assert_eq!(response.0.reject, 0);
    assert_eq!(response.0.abstain, 0);
}
#[tokio::test]
async fn gov_get_tally_projects_zk_abstentions() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let rid = "rid-tally-zk-abstain".to_string();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.governance_referenda_mut().insert(
            rid.clone(),
            GovernanceReferendumRecord {
                h_start: 1,
                h_end: 10,
                status: GovernanceReferendumStatus::Closed,
                mode: GovernanceReferendumMode::Zk,
            },
        );
        tx.world.elections_mut().insert(
            rid.clone(),
            ElectionState {
                finalized: true,
                tally: vec![7, 3, 5],
                ..ElectionState::default()
            },
        );
        tx.apply();
        let iroha_core::state::StateBlock { world, .. } = block;
        world.commit();
    }

    let response = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
        .await
        .expect("finalized ZK tally");
    assert_eq!(response.0.approve, 7);
    assert_eq!(response.0.reject, 3);
    assert_eq!(response.0.abstain, 5);
}
#[tokio::test]
async fn gov_get_tally_rejects_missing_referendum() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let err = handle_gov_get_tally(
        Arc::new(state),
        axum::extract::Path("missing-referendum".to_owned()),
    )
    .await
    .expect_err("a nonexistent referendum must not look like a zero tally");
    assert!(matches!(
        err,
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound
        ))
    ));
}
#[tokio::test]
async fn gov_get_tally_rejects_invalid_plain_direction() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let custody = generic_lock_custody(&state);
    let rid = "rid-tally-invalid-direction".to_string();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.governance_referenda_mut().insert(
            rid.clone(),
            GovernanceReferendumRecord {
                h_start: 1,
                h_end: 10,
                status: GovernanceReferendumStatus::Open,
                mode: GovernanceReferendumMode::Plain,
            },
        );
        let mut locks = GovernanceLocksForReferendum::default();
        locks.locks.insert(
            ALICE_ID.clone(),
            GovernanceLockRecord {
                owner: ALICE_ID.clone(),
                amount: 9_u64.into(),
                slashed: Quantity::zero(),
                expiry_height: 100,
                direction: 3,
                duration_blocks: 4,
                custody,
            },
        );
        tx.world.governance_locks_mut().insert(rid.clone(), locks);
        tx.apply();
        let iroha_core::state::StateBlock { world, .. } = block;
        world.commit();
    }
    let err = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
        .await
        .expect_err("an invalid direction must fail closed");
    let message = conversion_message(err);
    assert!(
        message.contains("invalid direction 3"),
        "unexpected tally error: {message}"
    );
}
#[tokio::test]
async fn legacy_referendum_reads_reject_stored_typed_proposal_fingerprints() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let kind = iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(
        iroha_data_model::governance::types::ValidationFeePolicyProposal {
            proposal_operator: ALICE_ID.clone(),
            policy: iroha_data_model::validation_fee::ValidationFeePolicyV1 {
                schema_version:
                    iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_SCHEMA_VERSION,
                network_id: *state.network_id_ref(),
                policy_version: 1,
                previous_policy_hash: None,
                ds_asset_id: state.gov.voting_asset_id.clone(),
                ds_scale: iroha_data_model::validation_fee::VALIDATION_FEE_DS_SCALE,
                fee: Quantity::zero(),
                treasury_account_id: ALICE_ID.clone(),
                charging_mode:
                    iroha_data_model::validation_fee::ValidationFeeChargingMode::Disabled,
                effective_from_height: 1,
                expires_after_height: None,
                exemption_classes: Vec::new(),
                treasury_payout_binding: None,
            },
            payout_lifecycle_proposal_id: None,
        },
    );
    let proposal_id = kind.fingerprint();
    let rid = hex::encode(proposal_id);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.governance_proposals_mut().insert(
            proposal_id,
            GovernanceProposalRecord {
                proposer: ALICE_ID.clone(),
                kind,
                created_height: 1,
                status: GovernanceProposalStatus::Proposed,
            },
        );
        tx.world.governance_referenda_mut().insert(
            rid.clone(),
            GovernanceReferendumRecord {
                h_start: 1,
                h_end: 10,
                status: GovernanceReferendumStatus::Open,
                mode: GovernanceReferendumMode::Plain,
            },
        );
        tx.apply();
        let iroha_core::state::StateBlock { world, .. } = block;
        world.commit();
    }
    let state = Arc::new(state);
    for err in [
        handle_gov_get_tally(state.clone(), axum::extract::Path(rid.clone()))
            .await
            .expect_err("generic tally must reject a typed proposal fingerprint"),
        handle_gov_get_referendum(state.clone(), axum::extract::Path(rid.clone()))
            .await
            .expect_err("generic referendum read must reject a typed proposal fingerprint"),
        handle_gov_get_locks(state, axum::extract::Path(rid))
            .await
            .expect_err("generic lock read must reject a typed proposal fingerprint"),
    ] {
        let message = conversion_message(err);
        assert!(
            message.contains("authenticated Parliament lifecycle"),
            "unexpected legacy read error: {message}"
        );
    }
}
#[tokio::test]
async fn gov_get_tally_rejects_accumulator_overflow() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let mut cfg = state.gov.clone();
    cfg.conviction_step_blocks = 1;
    cfg.max_conviction = u64::MAX;
    state.set_gov(cfg);
    let custody = generic_lock_custody(&state);
    let rid = "rid-tally-overflow".to_string();
    let other = AccountId::parse_encoded(ACCOUNT_OWNER_ALT).expect("alternate account id");
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.governance_referenda_mut().insert(
            rid.clone(),
            GovernanceReferendumRecord {
                h_start: 1,
                h_end: u64::MAX,
                status: GovernanceReferendumStatus::Open,
                mode: GovernanceReferendumMode::Plain,
            },
        );
        let mut locks = GovernanceLocksForReferendum::default();
        for owner in [ALICE_ID.clone(), other] {
            locks.locks.insert(
                owner.clone(),
                GovernanceLockRecord {
                    owner,
                    amount: Quantity::from(u128::MAX),
                    slashed: Quantity::zero(),
                    expiry_height: u64::MAX,
                    direction: 0,
                    duration_blocks: u64::MAX - 1,
                    custody: custody.clone(),
                },
            );
        }
        tx.world.governance_locks_mut().insert(rid.clone(), locks);
        tx.apply();
        let iroha_core::state::StateBlock { world, .. } = block;
        world.commit();
    }
    let err = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
        .await
        .expect_err("overflowing tally must fail");
    let message = conversion_message(err);
    assert!(
        message.contains("governance tally arithmetic overflow"),
        "unexpected tally error: {message}"
    );
}
#[test]
fn governed_contract_entrypoint_names_are_closed_ascii_identifiers() {
    for name in ["a", "balance", "transfer_2"] {
        assert!(is_canonical_public_entrypoint_name(name), "{name}");
    }
    for name in [
        "",
        "Balance",
        "2transfer",
        "transfer-funds",
        "transfer funds",
        "tránsfer",
        &"a".repeat(129),
    ] {
        assert!(!is_canonical_public_entrypoint_name(name), "{name}");
    }
}
#[tokio::test]
async fn governed_contract_read_serializes_exact_missing_shape() {
    let harness = mk_governance_harness(true);
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &harness.authority,
        92,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .expect("inactive contract address");
    let response = handle_gov_contract_get(
        harness.state,
        axum::extract::Path(contract_address.to_string()),
    )
    .await
    .expect("inactive governed contract read");
    let value = norito::json::to_value(&response.0).expect("serialize inactive response");
    let object = value.as_object().expect("inactive response object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["found", "contract_address", "dataspace"]
            .into_iter()
            .collect()
    );
    assert_eq!(object.get("found"), Some(&norito::json::Value::Bool(false)));
    assert_eq!(
        object
            .get("contract_address")
            .and_then(norito::json::Value::as_str),
        Some(contract_address.as_ref())
    );
    assert_eq!(
        object
            .get("dataspace")
            .and_then(norito::json::Value::as_str),
        Some("universal")
    );
}
#[tokio::test]
async fn governed_contract_read_retains_inactive_lifecycle_projection() {
    let harness = mk_governance_harness(true);
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &harness.authority,
        94,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .expect("inactive contract address");
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = harness.state.block(header);
    let mut transaction = block.transaction();
    transaction
        .world_mut_for_testing()
        .bind_inactive_contract_subject_for_testing(
            contract_address.clone(),
            harness.authority.clone(),
        );
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit inactive lifecycle fixture");

    let response = handle_gov_contract_get(
        harness.state,
        axum::extract::Path(contract_address.to_string()),
    )
    .await
    .expect("inactive governed contract read");
    let value = norito::json::to_value(&response.0).expect("serialize inactive response");
    let object = value.as_object().expect("inactive response object");
    assert_eq!(object.get("found"), Some(&norito::json::Value::Bool(true)));
    assert_eq!(
        object.get("active"),
        Some(&norito::json::Value::Bool(false))
    );
    assert_eq!(
        object.get("emergency_hold_active"),
        Some(&norito::json::Value::Bool(false))
    );
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        [
            "found",
            "contract_address",
            "contract_subject_account",
            "dataspace",
            "active",
            "lifecycle",
            "emergency_hold_active",
        ]
        .into_iter()
        .collect()
    );
    assert!(object.contains_key("contract_subject_account"));
    let lifecycle = object
        .get("lifecycle")
        .and_then(norito::json::Value::as_object)
        .expect("complete lifecycle projection");
    assert_eq!(
        lifecycle
            .get("revision")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert!(lifecycle.contains_key("origin"));
    assert!(lifecycle.contains_key("origin_account"));
    assert!(lifecycle.contains_key("origin_proposal_content_id_hex"));
    assert!(lifecycle.contains_key("origin_governance_attempt_id_hex"));
    assert!(lifecycle.contains_key("owner"));
    assert!(lifecycle.contains_key("pending_owner"));
    assert!(lifecycle.contains_key("parliament_delegated"));
    assert!(lifecycle.contains_key("active_code_hash_hex"));
    assert!(lifecycle.contains_key("emergency_hold"));
    assert!(!object.contains_key("code_hash_hex"));
    assert!(!object.contains_key("abi_hash_hex"));
    assert!(!object.contains_key("public_entrypoints"));
}
#[tokio::test]
async fn governed_contract_read_verifies_real_artifact_and_exact_active_shape() {
    let harness = mk_governance_harness(true);
    let (contract_address, expected_code_hash) = install_governed_contract_for_test(&harness);
    let response = handle_gov_contract_get(
        harness.state,
        axum::extract::Path(contract_address.to_string()),
    )
    .await
    .expect("active governed contract read");
    let value = norito::json::to_value(&response.0).expect("serialize active response");
    let object = value.as_object().expect("active response object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        [
            "found",
            "contract_address",
            "contract_subject_account",
            "dataspace",
            "active",
            "lifecycle",
            "emergency_hold_active",
            "code_hash_hex",
            "abi_hash_hex",
            "public_entrypoints",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(object.get("found"), Some(&norito::json::Value::Bool(true)));
    assert_eq!(object.get("active"), Some(&norito::json::Value::Bool(true)));
    assert_eq!(
        object
            .get("contract_address")
            .and_then(norito::json::Value::as_str),
        Some(contract_address.as_ref())
    );
    assert_eq!(
        object
            .get("contract_subject_account")
            .and_then(norito::json::Value::as_str),
        Some(contract_address.subject_id().to_string().as_str())
    );
    assert_eq!(
        object
            .get("code_hash_hex")
            .and_then(norito::json::Value::as_str),
        Some(hex::encode(<[u8; 32]>::from(expected_code_hash)).as_str())
    );
    assert_eq!(
        object.get("public_entrypoints"),
        Some(&norito::json!(["balance", "transfer"]))
    );
}
#[tokio::test]
async fn governed_contract_read_rejects_incomplete_active_state() {
    let harness = mk_governance_harness(true);
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &harness.authority,
        93,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .expect("incomplete contract address");
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = harness.state.block(header);
    let mut transaction = block.transaction();
    transaction
        .world_mut_for_testing()
        .bind_active_contract_subject_for_testing(
            contract_address.clone(),
            iroha_crypto::Hash::prehashed([0x44; 32]),
        );
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit incomplete fixture");
    let error = handle_gov_contract_get(
        harness.state,
        axum::extract::Path(contract_address.to_string()),
    )
    .await
    .expect_err("incomplete active state must fail closed");
    assert!(error.to_string().contains("incomplete code"));
}
#[tokio::test]
async fn governed_contract_read_rejects_removed_manifest_provenance() {
    let harness = mk_governance_harness(true);
    let (contract_address, code_hash) = install_governed_contract_for_test(&harness);
    let mut manifest = harness
        .state
        .view()
        .world()
        .contract_manifests()
        .get(&code_hash)
        .cloned()
        .expect("registered manifest");
    manifest.provenance = None;
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block = harness.state.block(header);
    let mut transaction = block.transaction();
    transaction
        .world_mut_for_testing()
        .contract_manifests_mut_for_testing()
        .insert(code_hash, manifest);
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit corrupted manifest fixture");
    let error = handle_gov_contract_get(
        harness.state,
        axum::extract::Path(contract_address.to_string()),
    )
    .await
    .expect_err("unsigned active manifest must fail closed");
    assert!(error.to_string().contains("signed provenance"));
}
#[tokio::test]
async fn propose_deploy_rejected_without_permission() {
    let harness = mk_governance_harness(false);
    let code_hash_bytes = [0x22u8; 32];
    let abi_hash_bytes = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let manifest_provenance =
        mk_manifest_provenance(&harness.authority_keypair, code_hash_bytes, abi_hash_bytes);
    let propose = DeployContractProposalDraftRequestV1 {
        proposal_operator: harness.authority.clone(),
        contract_address: Some(sample_contract_address()),
        contract_alias: None,
        abi_version: AbiVersion::new(1),
        code_hash: ContractCodeHash::new(code_hash_bytes),
        abi_hash: ContractAbiHash::new(abi_hash_bytes),
        manifest_provenance: Some(manifest_provenance),
    };
    let res = handle_gov_propose_deploy(harness.state.clone(), NoritoJson(propose))
        .await
        .expect("handler ok");
    let proposal_id = res.0.proposal_id.clone();
    queue_governance_proposal_instruction_skeleton(&harness, &res.0.tx_instructions);
    let errors = apply_queued_block_allow_errors(&harness.state, &harness.queue, 1);
    assert_eq!(errors, vec![true]);
    let pid_arr = proposal_id.into_bytes();
    assert!(
        harness
            .state
            .view()
            .world()
            .governance_proposals()
            .get(&pid_arr)
            .is_none(),
        "proposal should not be persisted without permission"
    );
}
#[tokio::test]
async fn ballot_zk_v1_builds_instruction_skeleton() {
    use axum::{Router, routing::post};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    // Route for zk-v1
    let app = Router::new().route(
        "/v1/gov/ballots/zk-v1",
        post({
            let state = state.clone();
            let authenticated = authenticated.clone();
            move |body: crate::NoritoJsonWithBytes<super::ZkBallotV1Dto>| {
                let telemetry = MaybeTelemetry::disabled();
                let authenticated = authenticated.clone();
                async move {
                    super::handle_gov_ballot_zk_v1(state, &authenticated, telemetry, body).await
                }
            }
        }),
    );
    // Build DTO
    let owner = canonical_literal(ACCOUNT_AUTHORITY);
    let dto = super::ZkBallotV1Dto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        network_id: *state.network_id_ref(),
        election_id: "ref-1".to_string(),
        backend: "halo2/ipa".to_string(),
        envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
        root_hint: Some(hex::encode([0u8; 32])),
        owner: Some(owner),
        amount: Some(100_u64.into()),
        duration_blocks: Some(200),
        direction: Some("Aye".to_string()),
        nullifier: Some(hex::encode([0x11u8; 32])),
    };
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/gov/ballots/zk-v1")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let b = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&b).unwrap();
    assert_eq!(
        v.get("drafted").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert!(v.get("ok").is_none());
    assert!(v.get("accepted").is_none());
    assert!(v.get("reason").is_none());
    assert!(
        v.get("tx_instructions")
            .and_then(|x| x.as_array())
            .is_some()
    );

    let invalid_dto = super::ZkBallotV1Dto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        network_id: *state.network_id_ref(),
        election_id: "ref-1".to_string(),
        backend: "halo2/ipa".to_string(),
        envelope_b64: String::new(),
        root_hint: None,
        owner: None,
        amount: None,
        duration_blocks: None,
        direction: None,
        nullifier: None,
    };
    let request = http::Request::builder()
        .method("POST")
        .uri("/v1/gov/ballots/zk-v1")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            norito::json::to_vec(&norito::json::to_value(&invalid_dto).unwrap()).unwrap(),
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: crate::ErrorEnvelope = norito::decode_from_bytes(&body).unwrap();
    assert_eq!(error.code(), "query_validation_failed");
    assert_eq!(
        error.message(),
        "envelope_b64 must be non-empty canonical base64"
    );
}
#[tokio::test]
async fn standalone_zk_ballot_rejects_stored_typed_proposal_fingerprint() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let proposal_id = seed_typed_proposal_fingerprint_for_ballot_test(&state, &authenticated);
    for selector in typed_proposal_selector_aliases(&proposal_id) {
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            network_id: *state.network_id_ref(),
            election_id: selector.clone(),
            backend: "halo2/ipa".to_owned(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode([1_u8, 2, 3, 4]),
            root_hint: None,
            owner: None,
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier: None,
        };
        let raw = norito::json::to_vec(&dto).expect("encode exact ZK ballot DTO");
        let error = super::handle_gov_ballot_zk_v1(
            Arc::clone(&state),
            &authenticated,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes {
                value: dto,
                raw: raw.into(),
            },
        )
        .await
        .expect_err("typed-proposal alias collision must fail the ballot draft");
        assert!(
            conversion_message(error).contains("authenticated Parliament lifecycle"),
            "selector {selector:?}"
        );
    }
}
#[tokio::test]
async fn ballot_zk_v1_rejects_invalid_root_hint() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let dto = super::ZkBallotV1Dto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        network_id: *state.network_id_ref(),
        election_id: "ref-1".to_string(),
        backend: "halo2/ipa".to_string(),
        envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
        root_hint: Some("invalid".to_string()),
        owner: None,
        amount: None,
        duration_blocks: None,
        direction: None,
        nullifier: None,
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let error = super::handle_gov_ballot_zk_v1(
        state,
        &authenticated,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect_err("invalid root hint must fail the ballot draft");
    assert_eq!(conversion_message(error), "root_hint must be 32-byte hex");
}
#[tokio::test]
async fn ballot_zk_v1_rejects_partial_lock_hints() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let dto = super::ZkBallotV1Dto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        network_id: *state.network_id_ref(),
        election_id: "ref-1".to_string(),
        backend: "halo2/ipa".to_string(),
        envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
        root_hint: None,
        owner: Some(ACCOUNT_AUTHORITY.to_string()),
        amount: None,
        duration_blocks: None,
        direction: None,
        nullifier: None,
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let error = super::handle_gov_ballot_zk_v1(
        state,
        &authenticated,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect_err("partial lock hints must fail the ballot draft");
    assert_eq!(
        conversion_message(error),
        "lock hints must include owner, amount, duration_blocks"
    );
}
include!("ballot_v1_strictness_tests.rs");
include!("ballotproof_shape_tests.rs");
