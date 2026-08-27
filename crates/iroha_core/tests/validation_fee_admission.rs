//! Integration coverage for validator admission of Parliament-enacted validation-fee policy.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_config::parameters::actual::ParliamentTimedOvn;
use iroha_core::{
    governance::{
        manifest::LaneManifestRegistry,
        parliament::{
            ParliamentAttemptStateV1, ParliamentDecisionModeV1, RequiredParliamentBodyV1,
        },
    },
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    smartcontracts::ivm::cache::IvmCache,
    state::{State, StateTransaction, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::DomainId,
    events::{
        EventFilterBox,
        time::{ExecutionTime, TimeEventFilter},
    },
    governance::types::{
        BallotAttemptId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId,
        DeliberationPhaseV1, GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceAttemptV1,
        GovernanceCertificateId, GovernanceExpectedHeadAbsentV1, GovernanceExpectedHeadV1,
        GovernanceStageV1, ParliamentAggregateOutcomeV1, ParliamentAggregateTallyV1,
        ParliamentBody, ProposalContentId, ProposalKind, RiskTierV1, SortitionRequestV1,
        TleKeySessionId, TleSessionId, ValidationFeePayoutLifecycleProposal,
        ValidationFeePolicyProposal, parliament_candidate_root_v1,
    },
    isi::{SetParameter, Transfer, TransferAssetBatch, TransferAssetBatchEntry},
    nexus::DataSpaceId,
    parameter::Parameter,
    prelude::*,
    smart_contract::{
        ContractAddress,
        manifest::{TriggerCallback, TriggerDescriptor},
    },
    transaction::{Executable, IvmBytecode, IvmProved, SignedTransaction},
    trigger::action::Repeats,
    validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY,
        VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY,
        VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS, ValidationFeeChargingMode,
        ValidationFeeParliamentAuthorizationV1, ValidationFeePayoutLifecycleReferenceV1,
        ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryV1, ValidationFeePolicyV1,
        ValidationFeeTreasuryPayoutBindingV1, ValidationFeeTreasuryPayoutRecipientV1,
    },
};
use iroha_primitives::{json::Json, numeric::NumericSpec};
use mv::storage::StorageReadOnly;
use sha2::{Digest as _, Sha256};
use std::{num::NonZeroU64, sync::Arc};
const TEST_VALIDATION_FEE_ASSET_SCALE: u8 = VALIDATION_FEE_DS_SCALE;
const TEST_POLICY_ENACTMENT_HEIGHT: u64 = 7_202;
const TEST_POLICY_EFFECTIVE_HEIGHT: u64 =
    TEST_POLICY_ENACTMENT_HEIGHT + VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;
const TEST_PARLIAMENT_POLICY_VERSION: u64 = 1;
fn quantity(value: &str) -> Quantity {
    value
        .parse()
        .expect("canonical validation-fee fixture quantity")
}
fn block_header(height: u64, timestamp_ms: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("height"),
        None,
        None,
        None,
        timestamp_ms,
        0,
    )
}
fn key_pair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair")
}
fn account(seed: u8) -> (AccountId, KeyPair) {
    let key_pair = key_pair(seed);
    (AccountId::new(key_pair.public_key().clone()), key_pair)
}
fn fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "fee_token".parse().expect("asset name"),
    )
}
fn xor_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "xor".parse().expect("asset name"),
    )
}
fn payout_contract_address() -> ContractAddress {
    ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &account(1).0,
        42,
        DataSpaceId::UNIVERSAL,
    )
    .expect("payout contract address")
}
fn pool_contract_address() -> ContractAddress {
    ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &account(2).0,
        43,
        DataSpaceId::UNIVERSAL,
    )
    .expect("pool contract address")
}
fn payout_contract_artifact() -> (
    Vec<u8>,
    iroha_data_model::smart_contract::manifest::ContractManifest,
) {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version: 1,
    };
    let entrypoint = iroha_data_model::smart_contract::manifest::EntrypointDescriptor {
        name: "autonomous_validation_fee_tick".to_owned(),
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        return_schema: None,
        permission: Some("CanInvokeContractEntrypoint".to_owned()),
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: None,
        access_hints_skipped: Vec::new(),
        triggers: vec![TriggerDescriptor {
            id: "validation_fee_payout_tick"
                .parse()
                .expect("payout trigger id"),
            repeats: Repeats::Indefinitely,
            filter: EventFilterBox::Time(TimeEventFilter(ExecutionTime::PreCommit)),
            authority: None,
            metadata: Metadata::default(),
            callback: TriggerCallback {
                namespace: None,
                entrypoint: "autonomous_validation_fee_tick".to_owned(),
            },
        }],
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "ValidationFeePayout".to_owned(),
        compiler_fingerprint: "validation-fee-admission-test".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: entrypoint.name.clone(),
            kind: entrypoint.kind,
            params: entrypoint.params.clone(),
            argument_schema: entrypoint.argument_schema.clone(),
            return_type: entrypoint.return_type.clone(),
            return_schema: entrypoint.return_schema.clone(),
            permission: entrypoint.permission.clone(),
            read_keys: entrypoint.read_keys.clone(),
            write_keys: entrypoint.write_keys.clone(),
            access_hints_complete: entrypoint.access_hints_complete,
            access_hints_skipped: entrypoint.access_hints_skipped.clone(),
            triggers: entrypoint.triggers.clone(),
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let verified =
        ivm::verify_contract_artifact(&artifact).expect("valid payout contract artifact");
    (artifact, verified.manifest)
}
fn pool_contract_artifact() -> (
    Vec<u8>,
    iroha_data_model::smart_contract::manifest::ContractManifest,
) {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version: 1,
    };
    let entrypoint = iroha_data_model::smart_contract::manifest::EntrypointDescriptor {
        name: "swap_exact_in_quote_public".to_owned(),
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        return_schema: None,
        permission: Some("CanInvokeContractEntrypoint".to_owned()),
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: None,
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "ValidationFeePool".to_owned(),
        compiler_fingerprint: "validation-fee-pool-admission-test".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: entrypoint.name.clone(),
            kind: entrypoint.kind,
            params: entrypoint.params.clone(),
            argument_schema: entrypoint.argument_schema.clone(),
            return_type: entrypoint.return_type.clone(),
            return_schema: entrypoint.return_schema.clone(),
            permission: entrypoint.permission.clone(),
            read_keys: entrypoint.read_keys.clone(),
            write_keys: entrypoint.write_keys.clone(),
            access_hints_complete: entrypoint.access_hints_complete,
            access_hints_skipped: entrypoint.access_hints_skipped.clone(),
            triggers: entrypoint.triggers.clone(),
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    artifact.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let verified = ivm::verify_contract_artifact(&artifact).expect("valid pool contract artifact");
    (artifact, verified.manifest)
}
fn payout_binding(fee_asset: &AssetDefinitionId) -> ValidationFeeTreasuryPayoutBindingV1 {
    let contract_address = payout_contract_address();
    let (contract_artifact, _) = payout_contract_artifact();
    ValidationFeeTreasuryPayoutBindingV1 {
        treasury_account_id: contract_address.subject_id(),
        contract_address,
        code_hash: <[u8; 32]>::from(Sha256::digest(contract_artifact)),
        entrypoint: "autonomous_validation_fee_tick"
            .parse()
            .expect("payout entrypoint"),
        ds_asset_id: fee_asset.clone(),
        xor_asset_id: xor_asset_definition_id(),
        pool_vault_account_id: pool_contract_address().subject_id(),
        batch_ds: iroha_data_model::validation_fee::validation_fee_payout_batch_ds(),
        min_xor_out: iroha_data_model::validation_fee::validation_fee_payout_min_xor(),
        max_xor_out: iroha_data_model::validation_fee::validation_fee_payout_max_xor(),
        recipients: (3..=6)
            .map(|seed| ValidationFeeTreasuryPayoutRecipientV1 {
                account_id: account(seed).0,
                share: iroha_data_model::validation_fee::validation_fee_payout_recipient_share(),
            })
            .collect(),
    }
}
fn test_state() -> (
    State,
    AccountId,
    KeyPair,
    AccountId,
    AccountId,
    AssetDefinitionId,
) {
    let (user, user_key_pair) = account(1);
    let (recipient, _) = account(8);
    let domain_id = DomainId::try_new("fees", "paynet").expect("domain id");
    let domain = Domain::new(domain_id).build(&user);
    let fee_asset = fee_asset_definition_id();
    let treasury = payout_contract_address().subject_id();
    let asset_definition = AssetDefinition::new(
        fee_asset.clone(),
        "fee_token".to_owned(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&user);
    let xor_asset_definition = AssetDefinition::new(
        xor_asset_definition_id(),
        "xor".to_owned(),
        NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&user);
    let user_asset = Asset::new(
        AssetId::new(fee_asset.clone(), user.clone()),
        Quantity::from(100_u64),
    );
    let mut accounts = vec![
        Account::new(user.clone()).build(&user),
        Account::new(recipient.clone()).build(&user),
        Account::new(treasury.clone()).build(&user),
        Account::new(pool_contract_address().subject_id()).build(&user),
    ];
    accounts.extend((2..=6).map(|seed| Account::new(account(seed).0).build(&user)));
    let state = State::new_for_testing(
        World::with_assets(
            [domain],
            accounts,
            [asset_definition, xor_asset_definition],
            [user_asset],
            [],
        ),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    state
        .block(block_header(1, 1_700_000_000_000))
        .commit()
        .expect("commit canonical genesis asset-incarnation state");
    (state, user, user_key_pair, recipient, treasury, fee_asset)
}
fn accept_transaction(state: &State, tx: SignedTransaction) -> AcceptedTransaction<'static> {
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();
    AcceptedTransaction::accept(
        tx,
        state.network_id_ref(),
        max_clock_drift,
        tx_params,
        crypto.as_ref(),
    )
    .expect("transaction admission should pass stateless checks")
}
fn validation_fee_policy(
    state: &State,
    fee_asset: AssetDefinitionId,
    treasury: AccountId,
) -> ValidationFeePolicyV1 {
    let payout_binding = payout_binding(&fee_asset);
    assert_eq!(treasury, payout_binding.treasury_account_id);
    ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        network_id: *state.network_id_ref(),
        policy_version: 1,
        previous_policy_hash: None,
        ds_asset_id: fee_asset,
        ds_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
        fee: iroha_data_model::validation_fee::initial_validation_fee_amount(),
        treasury_account_id: treasury,
        charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
        effective_from_height: TEST_POLICY_EFFECTIVE_HEIGHT,
        expires_after_height: Some(TEST_POLICY_EFFECTIVE_HEIGHT + 100),
        exemption_classes: vec![VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.to_owned()],
        treasury_payout_binding: Some(payout_binding),
    }
}
fn parliament_test_root(tag: u8) -> [u8; 32] {
    [tag.max(1); 32]
}
fn parliament_test_candidates() -> Vec<AccountId> {
    let mut candidates = (1_u8..=24).map(|seed| account(seed).0).collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates
}
fn validation_fee_parliament_requirements() -> Vec<RequiredParliamentBodyV1> {
    [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::CoordinationCouncil,
        ParliamentBody::FmaCommittee,
        ParliamentBody::OversightCommittee,
        ParliamentBody::PolicyJury,
    ]
    .into_iter()
    .map(|body| RequiredParliamentBodyV1 {
        body,
        decision_mode: if body == ParliamentBody::PolicyJury {
            ParliamentDecisionModeV1::HiddenBindingBallot
        } else {
            ParliamentDecisionModeV1::PublicFinding
        },
    })
    .collect()
}
fn parliament_test_governance(
    requirements: &[RequiredParliamentBodyV1],
) -> iroha_config::parameters::actual::Governance {
    let mut governance = iroha_config::parameters::actual::Governance {
        parliament_alternate_size: Some(0),
        ..iroha_config::parameters::actual::Governance::default()
    };
    for requirement in requirements {
        match requirement.body {
            ParliamentBody::RulesCommittee => governance.rules_committee_size = 3,
            ParliamentBody::AgendaCouncil => governance.agenda_council_size = 3,
            ParliamentBody::InterestPanel => governance.interest_panel_size = 3,
            ParliamentBody::ReviewPanel => governance.review_panel_size = 3,
            ParliamentBody::CoordinationCouncil => governance.coordination_council_size = 3,
            ParliamentBody::MpcCommittee => governance.mpc_committee_size = 3,
            ParliamentBody::FmaCommittee => governance.fma_committee_size = 3,
            ParliamentBody::OversightCommittee => governance.oversight_committee_size = 3,
            ParliamentBody::PolicyJury => governance.policy_jury_size = 3,
            ParliamentBody::ConfirmationJury => governance.confirmation_jury_size = 3,
        }
    }
    governance
}
fn complete_parliament_body_for_authorization(
    attempt: &mut ParliamentAttemptStateV1,
    requirement: RequiredParliamentBodyV1,
    election_attempt_id: BodyElectionAttemptId,
    result_tag: u8,
) {
    let governance_attempt_id = attempt.attempt().id;
    attempt
        .begin_invitation_acceptance(governance_attempt_id, election_attempt_id, 20, 1)
        .expect("open deterministic Parliament invitation window");
    let members = attempt
        .election(&election_attempt_id)
        .expect("drawn Parliament election")
        .primary_assignments()
        .iter()
        .map(|assignment| assignment.member.clone())
        .collect::<Vec<_>>();
    for member in &members {
        attempt
            .record_invitation_response(
                governance_attempt_id,
                election_attempt_id,
                member,
                true,
                20,
            )
            .expect("accept deterministic Parliament invitation");
    }
    let body_instance_id = attempt
        .seal_body_roster(governance_attempt_id, election_attempt_id, 21)
        .expect("seal deterministic Parliament roster");
    let mut phases = vec![
        DeliberationPhaseV1::Orientation,
        DeliberationPhaseV1::Evidence,
        DeliberationPhaseV1::Questions,
        DeliberationPhaseV1::Responses,
        DeliberationPhaseV1::Deliberation,
        DeliberationPhaseV1::Reflection,
    ];
    if requirement.decision_mode == ParliamentDecisionModeV1::HiddenBindingBallot {
        phases.push(DeliberationPhaseV1::Vote);
    }
    for phase in phases {
        attempt
            .advance_body_phase(governance_attempt_id, body_instance_id, phase, 22, 1)
            .expect("advance deterministic Parliament deliberation");
    }
    match requirement.decision_mode {
        ParliamentDecisionModeV1::PublicFinding => {
            let result_root = parliament_test_root(result_tag);
            let mut finalized = false;
            for member in &members {
                finalized = attempt
                    .endorse_public_finding(
                        governance_attempt_id,
                        body_instance_id,
                        result_root,
                        member,
                        22,
                    )
                    .expect("endorse deterministic public finding");
                if finalized {
                    break;
                }
            }
            assert!(
                finalized,
                "three seats must reach the two-thirds finding quorum"
            );
        }
        ParliamentDecisionModeV1::HiddenBindingBallot => {
            let ballot_attempt_id = BallotAttemptId::derive_v1(body_instance_id, 0);
            let release_beacon_session_id = BeaconSessionId::new(parliament_test_root(0xD0));
            let tle_key_session_id = TleKeySessionId::new(parliament_test_root(0xD1));
            let release_height = 42;
            let tle_session_id = TleSessionId::derive_v1(
                ballot_attempt_id,
                tle_key_session_id,
                release_beacon_session_id,
                release_height,
            );
            attempt
                .register_ballot_attempt(
                    governance_attempt_id,
                    body_instance_id,
                    ballot_attempt_id,
                    0,
                    tle_session_id,
                    tle_key_session_id,
                    release_beacon_session_id,
                    30,
                    ParliamentTimedOvn {
                        registration_phase_blocks: 4,
                        survivor_freeze_phase_blocks: 3,
                        commitment_phase_blocks: 1,
                        release_delay_blocks: 4,
                        opening_phase_blocks: 2,
                        max_ballot_retries: 2,
                        max_corpus_entries: 3,
                    },
                    release_height,
                )
                .expect("register deterministic binding ballot");
            let registration_root = parliament_test_root(0xD2);
            let dropout_root = parliament_test_root(0xD3);
            let survivor_root = parliament_test_root(0xD4);
            let no_recovery_root = parliament_test_root(0xD5);
            let corpus_root = parliament_test_root(0xD6);
            let timed_commitment_root = parliament_test_root(0xD7);
            attempt
                .close_ballot_registration(
                    governance_attempt_id,
                    ballot_attempt_id,
                    registration_root,
                    3,
                    32,
                )
                .expect("close deterministic ballot registration");
            attempt
                .freeze_ballot_survivors(
                    governance_attempt_id,
                    ballot_attempt_id,
                    dropout_root,
                    survivor_root,
                    3,
                    no_recovery_root,
                    37,
                )
                .expect("freeze deterministic ballot survivors");
            attempt
                .freeze_timed_ovn_corpus(
                    governance_attempt_id,
                    ballot_attempt_id,
                    corpus_root,
                    survivor_root,
                    3,
                    timed_commitment_root,
                    38,
                )
                .expect("freeze deterministic timed-OVN corpus");
            attempt
                .begin_ballot_opening_batch(
                    governance_attempt_id,
                    vec![ballot_attempt_id],
                    release_beacon_session_id,
                    release_height,
                    release_height,
                    BeaconPulseId::new(parliament_test_root(0xD8)),
                )
                .expect("open deterministic timed ballot");
            let outcome = attempt
                .finalize_opened_ballot(
                    governance_attempt_id,
                    ballot_attempt_id,
                    corpus_root,
                    no_recovery_root,
                    tle_session_id,
                    parliament_test_root(0xD9),
                    3,
                    ParliamentAggregateTallyV1 {
                        original_seats: 3,
                        accepted_ballots: 3,
                        aye: 2,
                        nay: 1,
                        abstain: 0,
                    },
                    43,
                )
                .expect("finalize deterministic aggregate ballot");
            assert_eq!(outcome, ParliamentAggregateOutcomeV1::Approved);
        }
    }
}
fn test_parliament_authorization(
    state: &State,
    proposal_kind: &ProposalKind,
    enacted_at_height: u64,
) -> (
    ValidationFeeParliamentAuthorizationV1,
    ParliamentAttemptStateV1,
) {
    let proposal_operator = match proposal_kind {
        ProposalKind::ValidationFeePolicy(proposal) => proposal.proposal_operator.clone(),
        ProposalKind::ValidationFeePayoutLifecycle(proposal) => proposal.proposal_operator.clone(),
        _ => panic!("validation-fee fixture requires a validation-fee proposal"),
    };
    let proposal_fingerprint = proposal_kind.fingerprint();
    let proposal_content_id = ProposalContentId::new(proposal_fingerprint);
    let governance_attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
    let requirements = validation_fee_parliament_requirements();
    let expected_head = GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
        subject_id: proposal_kind
            .governed_subject_id_v1()
            .expect("derive exact validation-fee governed subject"),
    });
    let mut attempt = ParliamentAttemptStateV1::try_new(
        GovernanceAttemptV1 {
            id: governance_attempt_id,
            proposal_content_id,
            sequence: 0,
            risk_tier: RiskTierV1::Constitutional,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        },
        TEST_PARLIAMENT_POLICY_VERSION,
        proposal_kind.effect_preimage_hash_v1(),
        expected_head,
        requirements.clone(),
    )
    .expect("create exact validation-fee Parliament attempt");
    attempt
        .complete_qualification(governance_attempt_id)
        .expect("complete deterministic qualification");
    let candidates = parliament_test_candidates();
    let candidate_count = u32::try_from(candidates.len()).expect("candidate count fits u32");
    let sortition_session = BeaconSessionId::new(parliament_test_root(0xB0));
    let mut request_ids = Vec::with_capacity(requirements.len());
    for requirement in &requirements {
        let election_attempt_id =
            BodyElectionAttemptId::derive_v1(governance_attempt_id, requirement.body, 0);
        let request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            requirement.body,
            parliament_candidate_root_v1(governance_attempt_id, requirement.body, &candidates),
            candidate_count,
            3,
            10,
            20,
            sortition_session,
            None,
        )
        .expect("construct deterministic sortition request");
        request_ids.push(request.id);
        attempt
            .register_sortition_request(governance_attempt_id, 0, request, candidates.clone())
            .expect("register deterministic sortition request");
    }
    request_ids.sort_unstable();
    let sortition_pulse_id = BeaconPulseId::new(parliament_test_root(0xB1));
    attempt
        .consume_sortition_pulse_batch(
            governance_attempt_id,
            request_ids,
            sortition_session,
            20,
            sortition_pulse_id,
            *sortition_pulse_id.as_bytes(),
            state.network_id_ref(),
            &parliament_test_governance(&requirements),
        )
        .expect("consume deterministic simultaneous Parliament draw");
    for (index, requirement) in requirements.iter().copied().enumerate() {
        complete_parliament_body_for_authorization(
            &mut attempt,
            requirement,
            BodyElectionAttemptId::derive_v1(governance_attempt_id, requirement.body, 0),
            0xC0_u8
                .checked_add(u8::try_from(index).expect("body index fits u8"))
                .expect("result tag does not overflow"),
        );
    }
    assert_eq!(attempt.attempt().stage, GovernanceStageV1::Certification);
    let governance_certificate = attempt
        .construct_certificate(
            governance_attempt_id,
            enacted_at_height
                .checked_sub(1)
                .expect("enactment follows certification"),
            enacted_at_height,
        )
        .expect("construct complete validation-fee Parliament certificate");
    governance_certificate
        .validate()
        .expect("validation-fee Parliament certificate validates");
    attempt
        .mark_enacted(governance_attempt_id, enacted_at_height)
        .expect("mark exact-due validation-fee attempt enacted");
    attempt
        .validate()
        .expect("enacted validation-fee Parliament attempt validates");
    let authorization = ValidationFeeParliamentAuthorizationV1 {
        proposal_operator,
        proposal_fingerprint,
        governance_certificate_id: GovernanceCertificateId::derive_v1(&governance_certificate),
        governance_certificate,
        enacted_at_height,
    };
    assert_eq!(authorization.invariant_error(), None);
    (authorization, attempt)
}
fn policy_treasury_account(policy: &ValidationFeePolicyV1) -> AccountId {
    policy.treasury_account_id.clone()
}
fn payout_lifecycle_proposal(policy: &ValidationFeePolicyV1) -> ProposalKind {
    ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
        proposal_operator: account(1).0,
        payout_binding: policy
            .treasury_payout_binding
            .clone()
            .expect("enabled policy must carry its exact payout binding"),
    })
}
fn payout_lifecycle_proposal_id(policy: &ValidationFeePolicyV1) -> [u8; 32] {
    payout_lifecycle_proposal(policy).fingerprint()
}
fn policy_proposal(policy: &ValidationFeePolicyV1) -> ProposalKind {
    ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
        proposal_operator: account(1).0,
        policy: policy.clone(),
        payout_lifecycle_proposal_id: Some(payout_lifecycle_proposal_id(policy)),
    })
}
fn canonical_policy_registry_state(
    state: &State,
    policy: &ValidationFeePolicyV1,
) -> (
    ValidationFeePolicyRegistryV1,
    Vec<(ProposalKind, ParliamentAttemptStateV1)>,
) {
    let enacted_at_height = policy
        .effective_from_height
        .checked_sub(VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS)
        .expect("test policy leaves the full activation delay");
    let lifecycle_proposal = payout_lifecycle_proposal(policy);
    let lifecycle_seal = policy
        .treasury_payout_binding
        .as_ref()
        .expect("enabled policy must carry its exact payout binding")
        .lifecycle_seal()
        .expect("derive payout lifecycle seal");
    let policy_proposal = policy_proposal(policy);
    let (lifecycle_authorization, lifecycle_attempt) =
        test_parliament_authorization(state, &lifecycle_proposal, enacted_at_height);
    let (policy_authorization, policy_attempt) =
        test_parliament_authorization(state, &policy_proposal, enacted_at_height);
    let entry = ValidationFeePolicyRegistryEntryV1::from_enactment(
        policy.clone(),
        policy_authorization,
        Some(ValidationFeePayoutLifecycleReferenceV1 {
            lifecycle_seal,
            parliament_authorization: lifecycle_authorization,
        }),
    )
    .expect("registry entry");
    let registry = ValidationFeePolicyRegistryV1 {
        registered_policies: vec![entry],
    };
    registry
        .validate()
        .expect("canonical validation-fee registry validates");
    (
        registry,
        vec![
            (lifecycle_proposal, lifecycle_attempt),
            (policy_proposal, policy_attempt),
        ],
    )
}
fn policy_registry(state: &State, policy: &ValidationFeePolicyV1) -> ValidationFeePolicyRegistryV1 {
    canonical_policy_registry_state(state, policy).0
}
fn seed_canonical_enacted_proposal(
    kind: ProposalKind,
    proposer: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> [u8; 32] {
    let proposal_id = kind.fingerprint();
    assert!(
        matches!(
            &kind,
            ProposalKind::ValidationFeePolicy(_) | ProposalKind::ValidationFeePayoutLifecycle(_)
        ),
        "validation-fee admission fixture requires a validation-fee proposal"
    );
    let selection_epoch = 1;
    state_transaction.world.governance_proposals_mut().insert(
        proposal_id,
        iroha_core::state::GovernanceProposalRecord {
            proposer: proposer.clone(),
            kind,
            created_height: selection_epoch,
            status: iroha_core::state::GovernanceProposalStatus::Enacted,
        },
    );
    proposal_id
}
fn install_canonical_post_enactment_validation_fee_state(
    state: &State,
    authority: &AccountId,
    authority_key_pair: &KeyPair,
    policy: ValidationFeePolicyV1,
) {
    let (registry, enacted_attempts) = canonical_policy_registry_state(state, &policy);
    assert_eq!(
        registry.registered_policies[0]
            .parliament_authorization
            .enacted_at_height,
        TEST_POLICY_ENACTMENT_HEIGHT
    );
    let mut block = state.block(block_header(
        TEST_POLICY_ENACTMENT_HEIGHT,
        1_700_000_006_000,
    ));
    let mut state_transaction = block.transaction();

    let register_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into();
    Grant::account_permission(register_permission, authority.clone())
        .execute(authority, &mut state_transaction)
        .expect("grant payout-contract registration authority");
    let (contract_artifact, contract_manifest) = payout_contract_artifact();
    let registered_code_hash = iroha_core::smartcontracts::code::register_code_bytes(
        authority,
        contract_artifact,
        &mut state_transaction,
    )
    .expect("register payout-contract bytes");
    iroha_core::smartcontracts::code::register_manifest(
        authority,
        contract_manifest.signed(authority_key_pair),
        &mut state_transaction,
    )
    .expect("register signed payout-contract manifest");
    iroha_core::smartcontracts::code::activate_instance(
        authority,
        payout_contract_address(),
        registered_code_hash,
        &mut state_transaction,
    )
    .expect("activate immutable payout-contract subject");

    let (pool_artifact, pool_manifest) = pool_contract_artifact();
    let pool_code_hash = iroha_core::smartcontracts::code::register_code_bytes(
        authority,
        pool_artifact,
        &mut state_transaction,
    )
    .expect("register pool-contract bytes");
    iroha_core::smartcontracts::code::register_manifest(
        authority,
        pool_manifest.signed(authority_key_pair),
        &mut state_transaction,
    )
    .expect("register signed pool-contract manifest");
    iroha_core::smartcontracts::code::activate_instance(
        authority,
        pool_contract_address(),
        pool_code_hash,
        &mut state_transaction,
    )
    .expect("activate pool contract");

    let payout_binding = policy
        .treasury_payout_binding
        .as_ref()
        .expect("enabled policy carries its payout binding");
    let wrapper_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
            contract: payout_contract_address(),
            entrypoint: "autonomous_validation_fee_tick".to_owned(),
        }
        .into();
    let pool_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
            contract: pool_contract_address(),
            entrypoint: "swap_exact_in_quote_public".to_owned(),
        }
        .into();
    let wrapper_ds_transfer_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::asset::CanTransferAsset {
            asset: AssetId::new(
                policy.ds_asset_id.clone(),
                policy.treasury_account_id.clone(),
            ),
        }
        .into();
    for (permission, holder) in [
        (
            wrapper_permission,
            payout_binding.treasury_account_id.clone(),
        ),
        (pool_permission, payout_binding.treasury_account_id.clone()),
        (
            wrapper_ds_transfer_permission,
            payout_binding.pool_vault_account_id.clone(),
        ),
    ] {
        Grant::account_permission(permission, holder)
            .execute(authority, &mut state_transaction)
            .expect("grant exact enacted payout-lifecycle effect permission");
    }

    for (proposal_kind, attempt) in enacted_attempts {
        let proposal_id = proposal_kind.fingerprint();
        assert_eq!(
            attempt.proposal_content_id(),
            ProposalContentId::new(proposal_id)
        );
        assert_eq!(
            seed_canonical_enacted_proposal(proposal_kind, authority, &mut state_transaction),
            proposal_id
        );
        let governance_attempt_id = attempt.attempt().id;
        state_transaction
            .world
            .put_parliament_attempt_for_testing(governance_attempt_id, attempt)
            .expect("persist validated enacted Parliament attempt");
    }
    state_transaction
        .world
        .parameters_mut_for_testing()
        .get_mut()
        .set_parameter(Parameter::Custom(registry.into_custom_parameter()));
    state_transaction.apply();
    block
        .commit()
        .expect("commit canonical post-enactment validation-fee state");
}
fn metadata_for_policy(policy: &ValidationFeePolicyV1, fee_instruction_index: usize) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        VALIDATION_FEE_POLICY_VERSION_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(policy.policy_version),
    );
    metadata.insert(
        VALIDATION_FEE_POLICY_HASH_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(hex::encode(policy.policy_hash().expect("policy hash"))),
    );
    metadata.insert(
        VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(u64::try_from(fee_instruction_index).expect("instruction index fits")),
    );
    metadata
}
fn metadata_for_batch_policy(
    policy: &ValidationFeePolicyV1,
    fee_instruction_index: usize,
    fee_entry_index: usize,
) -> Metadata {
    let mut metadata = metadata_for_policy(policy, fee_instruction_index);
    metadata.insert(
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(u64::try_from(fee_entry_index).expect("entry index fits")),
    );
    metadata
}
fn signed_transfer(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    policy: &ValidationFeePolicyV1,
    include_fee: bool,
) -> SignedTransaction {
    let metadata = if include_fee {
        metadata_for_policy(policy, 1)
    } else {
        Metadata::default()
    };
    signed_transfer_with_metadata(
        state,
        user,
        user_key_pair,
        recipient,
        fee_asset,
        policy,
        include_fee,
        metadata,
    )
}
fn signed_transfer_with_metadata(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    policy: &ValidationFeePolicyV1,
    include_fee: bool,
    metadata: Metadata,
) -> SignedTransaction {
    let fee_instruction =
        include_fee.then(|| (policy.fee.clone(), policy_treasury_account(policy)));
    signed_transfer_with_fee_instruction(
        state,
        user,
        user_key_pair,
        recipient,
        fee_asset,
        fee_instruction,
        metadata,
    )
}
fn signed_transfer_with_fee_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    fee_instruction: Option<(Quantity, AccountId)>,
    metadata: Metadata,
) -> SignedTransaction {
    signed_transfer_with_principal_and_fee_instruction(
        state,
        user,
        user_key_pair,
        recipient,
        fee_asset,
        Quantity::from(1_u32),
        fee_instruction,
        metadata,
    )
}
fn signed_transfer_with_principal_and_fee_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    principal_amount: Quantity,
    fee_instruction: Option<(Quantity, AccountId)>,
    metadata: Metadata,
) -> SignedTransaction {
    let principal = Transfer::asset_quantity(
        AssetId::new(fee_asset.clone(), user.clone()),
        principal_amount,
        recipient.clone(),
    );
    let mut instructions: Vec<InstructionBox> = vec![principal.into()];
    if let Some((fee_amount, fee_recipient)) = fee_instruction {
        instructions.push(
            Transfer::asset_quantity(
                AssetId::new(fee_asset.clone(), user.clone()),
                fee_amount,
                fee_recipient,
            )
            .into(),
        );
    }
    TransactionBuilder::new(
        *state.network_id_ref(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}
fn signed_ivm_proved_overlay(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    overlay: Vec<InstructionBox>,
    metadata: Metadata,
) -> SignedTransaction {
    let mut program = ivm::ProgramMetadata {
        max_cycles: 1_000,
        ..ivm::ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    TransactionBuilder::new(
        *state.network_id_ref(),
        user.clone(),
        FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(1_000)),
    )
    .with_executable(Executable::IvmProved(IvmProved {
        bytecode: IvmBytecode::from_compiled(program),
        overlay: overlay.into(),
        events_commitment: Hash::new(b"events"),
        gas_policy_commitment: Hash::new(b"gas-policy"),
    }))
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}
fn signed_transfer_with_explicit_fee_asset_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    principal_asset: &AssetDefinitionId,
    fee_asset: &AssetDefinitionId,
    fee_amount: Quantity,
    fee_recipient: AccountId,
    metadata: Metadata,
) -> SignedTransaction {
    TransactionBuilder::new(
        *state.network_id_ref(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(principal_asset.clone(), user.clone()),
            1_u32,
            recipient.clone(),
        )),
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            fee_amount,
            fee_recipient,
        )),
    ])
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}
fn signed_transfer_with_explicit_fee_source_instruction(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    fee_source: &AccountId,
    fee_amount: Quantity,
    fee_recipient: AccountId,
    metadata: Metadata,
) -> SignedTransaction {
    TransactionBuilder::new(
        *state.network_id_ref(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            1_u32,
            recipient.clone(),
        )),
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), fee_source.clone()),
            fee_amount,
            fee_recipient,
        )),
    ])
    .with_metadata(metadata)
    .sign(user_key_pair.private_key())
}
fn signed_batch_transfer_with_principal_amounts(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    recipient: &AccountId,
    fee_asset: &AssetDefinitionId,
    policy: &ValidationFeePolicyV1,
    first_principal_amount: Quantity,
    second_principal_amount: Quantity,
) -> SignedTransaction {
    signed_batch_transfer_with_entries(
        state,
        user,
        user_key_pair,
        policy,
        vec![
            TransferAssetBatchEntry::new(
                user.clone(),
                recipient.clone(),
                fee_asset.clone(),
                first_principal_amount,
            ),
            TransferAssetBatchEntry::new(
                user.clone(),
                recipient.clone(),
                fee_asset.clone(),
                second_principal_amount,
            ),
            TransferAssetBatchEntry::new(
                user.clone(),
                policy_treasury_account(policy),
                fee_asset.clone(),
                quantity("0.2"),
            ),
        ],
    )
}
fn signed_batch_transfer_with_entries(
    state: &State,
    user: &AccountId,
    user_key_pair: &KeyPair,
    policy: &ValidationFeePolicyV1,
    entries: Vec<TransferAssetBatchEntry>,
) -> SignedTransaction {
    let batch = TransferAssetBatch::new(entries);
    TransactionBuilder::new(
        *state.network_id_ref(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(batch)])
    .with_metadata(metadata_for_batch_policy(policy, 0, 2))
    .sign(user_key_pair.private_key())
}
fn validate_in_block(state: &State, height: u64, tx: SignedTransaction) -> String {
    let accepted = accept_transaction(state, tx);
    let mut block = state.block(block_header(height, 1_700_000_002_000 + height));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Ok(_) => "ok".to_string(),
        Err(error) => format!("{error:?}"),
    }
}
fn accept_transaction_error(state: &State, tx: SignedTransaction) -> String {
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();
    match AcceptedTransaction::accept(
        tx,
        state.network_id_ref(),
        max_clock_drift,
        tx_params,
        crypto.as_ref(),
    ) {
        Ok(_) => "ok".to_string(),
        Err(error) => format!("{error:?}"),
    }
}
fn asset_balance(world: &impl WorldReadOnly, asset_id: &AssetId) -> Quantity {
    world
        .assets()
        .get(asset_id)
        .map_or_else(Quantity::zero, |value| value.clone().into_inner())
}
#[test]
fn raw_fee_asset_transfer_is_rejected_without_exact_active_validation_fee() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury);
    install_canonical_post_enactment_validation_fee_state(
        &state,
        &user,
        &user_key_pair,
        policy.clone(),
    );
    let missing_fee_error = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT,
        signed_transfer(
            &state,
            &user,
            &user_key_pair,
            &recipient,
            &fee_asset,
            &policy,
            false,
        ),
    );
    assert!(
        missing_fee_error.contains("missing validation-fee transfer of 10 minor units"),
        "unexpected missing-fee rejection: {missing_fee_error}"
    );
    let exact_fee_result = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT + 1,
        signed_transfer(
            &state,
            &user,
            &user_key_pair,
            &recipient,
            &fee_asset,
            &policy,
            true,
        ),
    );
    assert_eq!(exact_fee_result, "ok");
}
#[test]
fn validation_fee_registry_cannot_be_installed_through_generic_parameter_path() {
    let (state, user, _, _, treasury, fee_asset) = test_state();
    let policy = validation_fee_policy(&state, fee_asset, treasury);
    let custom = policy_registry(&state, &policy).into_custom_parameter();
    let mut block = state.block(block_header(
        TEST_POLICY_ENACTMENT_HEIGHT,
        1_700_000_001_000,
    ));
    let mut state_transaction = block.transaction();
    let error = SetParameter::new(Parameter::Custom(custom))
        .execute(&user, &mut state_transaction)
        .expect_err("generic parameter writes must not bypass Parliament");
    let error_debug = format!("{error:?}");
    assert!(
        error_debug.contains("can only be changed by an enacted SORA Parliament proposal"),
        "unexpected protected-registry rejection: {error_debug}"
    );
}
#[test]
fn active_registry_rejects_missing_enacted_parliament_attempt() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury);
    install_canonical_post_enactment_validation_fee_state(
        &state,
        &user,
        &user_key_pair,
        policy.clone(),
    );
    let proposal_id = policy_proposal(&policy).fingerprint();
    {
        let mut block = state.block(block_header(
            TEST_POLICY_ENACTMENT_HEIGHT + 1,
            1_700_000_007_000,
        ));
        let mut stx = block.transaction();
        let attempt_id = GovernanceAttemptId::derive_v1(ProposalContentId::new(proposal_id), 0);
        assert!(
            stx.world
                .remove_parliament_attempt_for_testing(&attempt_id)
                .is_some(),
            "canonical fixture must retain the enacted Parliament attempt"
        );
        stx.apply();
        block
            .commit()
            .expect("commit adversarial missing-attempt state");
    }
    let error = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT,
        signed_transfer(
            &state,
            &user,
            &user_key_pair,
            &recipient,
            &fee_asset,
            &policy,
            true,
        ),
    );
    assert!(
        error.contains("authorized Parliament attempt is missing"),
        "missing enacted Parliament attempt must fail closed: {error}"
    );
}
#[test]
fn enacted_lifecycle_pins_exact_wrapper_pool_and_asset_effect_permissions() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let policy = validation_fee_policy(&state, fee_asset, treasury.clone());
    install_canonical_post_enactment_validation_fee_state(&state, &user, &user_key_pair, policy);
    let wrapper_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
            contract: payout_contract_address(),
            entrypoint: "autonomous_validation_fee_tick".to_owned(),
        }
        .into();
    let pool_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
            contract: pool_contract_address(),
            entrypoint: "swap_exact_in_quote_public".to_owned(),
        }
        .into();
    let wrapper_ds_transfer_permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::asset::CanTransferAsset {
            asset: AssetId::new(fee_asset_definition_id(), treasury.clone()),
        }
        .into();
    let mut block = state.block(block_header(
        TEST_POLICY_ENACTMENT_HEIGHT + 1,
        1_700_000_007_000,
    ));
    let mut stx = block.transaction();
    for (permission, required_owner) in [
        (wrapper_permission, treasury.clone()),
        (pool_permission, treasury.clone()),
        (
            wrapper_ds_transfer_permission,
            pool_contract_address().subject_id(),
        ),
    ] {
        let grant_error = Grant::account_permission(permission.clone(), recipient.clone())
            .execute(&user, &mut stx)
            .expect_err(
                "payout runtime permission must not be delegated after lifecycle enactment",
            );
        assert!(
            grant_error
                .to_string()
                .contains("forbids delegating its exact runtime permissions"),
            "unexpected payout runtime delegation error: {grant_error}"
        );
        let revoke_error = Revoke::account_permission(permission, required_owner)
            .execute(&user, &mut stx)
            .expect_err("required payout runtime permission must remain pinned");
        assert!(
            revoke_error
                .to_string()
                .contains("pins its exact runtime permissions"),
            "unexpected payout runtime revocation error: {revoke_error}"
        );
    }
}
#[test]
fn ivm_proved_overlay_reaches_active_validation_fee_admission() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury.clone());
    install_canonical_post_enactment_validation_fee_state(
        &state,
        &user,
        &user_key_pair,
        policy.clone(),
    );
    let principal = || {
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            1_u32,
            recipient.clone(),
        ))
    };
    let fee = || {
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            policy.fee.clone(),
            treasury.clone(),
        ))
    };
    let missing_fee_error = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT,
        signed_ivm_proved_overlay(
            &state,
            &user,
            &user_key_pair,
            vec![principal()],
            Metadata::default(),
        ),
    );
    assert!(
        missing_fee_error.contains("missing validation-fee transfer of 10 minor units"),
        "unexpected proved-IVM missing-fee rejection: {missing_fee_error}"
    );
    let exact_fee_result = validate_in_block(
        &state,
        TEST_POLICY_EFFECTIVE_HEIGHT + 1,
        signed_ivm_proved_overlay(
            &state,
            &user,
            &user_key_pair,
            vec![principal(), fee()],
            metadata_for_policy(&policy, 1),
        ),
    );
    assert!(
        !exact_fee_result.contains("validation-fee admission rejected transaction")
            && !exact_fee_result.contains("UnsupportedExecutable"),
        "exact proved-IVM overlay fee must pass validation-fee admission: {exact_fee_result}"
    );
}
#[test]
fn principal_and_fee_commit_atomically_under_active_validation_fee_policy() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury.clone());
    install_canonical_post_enactment_validation_fee_state(
        &state,
        &user,
        &user_key_pair,
        policy.clone(),
    );
    let recipient_asset = AssetId::new(fee_asset.clone(), recipient.clone());
    let treasury_asset = AssetId::new(fee_asset.clone(), treasury.clone());
    let missing_fee_tx = signed_transfer(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        false,
    );
    let accepted = accept_transaction(&state, missing_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT,
        1_700_000_003_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_err(), "missing fee must reject before commit");
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "principal transfer must not commit when validation-fee admission fails"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "treasury must not be credited by a transaction rejected before execution"
    );
    drop(view);
    let underpaid_fee_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Quantity::from(1_u32),
        Some((quantity("0.09"), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let accepted = accept_transaction(&state, underpaid_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 1,
        1_700_000_004_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_err(), "underpaid fee must reject before commit");
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "principal transfer must not commit when the fee amount is wrong"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "wrong fee amount must not credit the treasury"
    );
    drop(view);
    let fee_then_overdrawn_principal_tx = TransactionBuilder::new(
        *state.network_id_ref(),
        user.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            policy.fee.clone(),
            policy_treasury_account(&policy),
        )),
        InstructionBox::from(Transfer::asset_quantity(
            AssetId::new(fee_asset.clone(), user.clone()),
            100_u32,
            recipient.clone(),
        )),
    ])
    .with_metadata(metadata_for_policy(&policy, 0))
    .sign(user_key_pair.private_key());
    let accepted = accept_transaction(&state, fee_then_overdrawn_principal_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 2,
        1_700_000_005_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(
        result.is_err(),
        "overdrawn principal after fee execution must reject"
    );
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "recipient must not be credited by a rejected transaction"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "fee transfer must roll back when the later principal transfer fails"
    );
    drop(view);
    let principal_then_overdrawn_fee_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        quantity("99.95"),
        Some((policy.fee.clone(), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let accepted = accept_transaction(&state, principal_then_overdrawn_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 3,
        1_700_000_006_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(
        result.is_err(),
        "overdrawn fee after principal execution must reject"
    );
    drop(block);
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::zero(),
        "principal transfer must roll back when the later fee transfer fails"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        Quantity::zero(),
        "treasury must not be credited by a rejected transaction"
    );
    drop(view);
    let exact_fee_tx = signed_transfer(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
    );
    let accepted = accept_transaction(&state, exact_fee_tx);
    let mut block = state.block(block_header(
        TEST_POLICY_EFFECTIVE_HEIGHT + 4,
        1_700_000_007_000,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = block.validate_transaction(accepted, &mut ivm_cache);
    assert_eq!(result, Ok(Vec::new()));
    block
        .commit()
        .expect("commit exact validation-fee transfer");
    let view = state.view();
    assert_eq!(
        asset_balance(view.world(), &recipient_asset),
        Quantity::from(1_u64),
        "principal transfer must commit with the exact fee"
    );
    assert_eq!(
        asset_balance(view.world(), &treasury_asset),
        policy.fee.clone(),
        "fee transfer must commit with the principal transfer"
    );
}
#[test]
fn fee_instruction_policy_hash_amount_and_treasury_are_covered_by_user_signature() {
    let (state, user, user_key_pair, recipient, treasury, fee_asset) = test_state();
    let policy = validation_fee_policy(&state, fee_asset.clone(), treasury);
    install_canonical_post_enactment_validation_fee_state(
        &state,
        &user,
        &user_key_pair,
        policy.clone(),
    );
    let mut exact_fee_tx = signed_transfer(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
    );
    let exact_fee_result = validate_in_block(&state, 3, exact_fee_tx.clone());
    assert_eq!(exact_fee_result, "ok");
    let mut wrong_policy_hash_metadata = metadata_for_policy(&policy, 1);
    wrong_policy_hash_metadata.insert(
        VALIDATION_FEE_POLICY_HASH_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(hex::encode([0x55u8; 32])),
    );
    let wrong_policy_hash_tx = signed_transfer_with_metadata(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
        wrong_policy_hash_metadata,
    );
    let mut policy_hash_mutation_tx = exact_fee_tx.clone();
    policy_hash_mutation_tx.set_signature(wrong_policy_hash_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, policy_hash_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "policy-hash payload mutation must fail signature admission, got {signature_error}"
    );
    let mut wrong_policy_version_metadata = metadata_for_policy(&policy, 1);
    wrong_policy_version_metadata.insert(
        VALIDATION_FEE_POLICY_VERSION_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(policy.policy_version + 1),
    );
    let wrong_policy_version_tx = signed_transfer_with_metadata(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
        wrong_policy_version_metadata,
    );
    let mut policy_version_mutation_tx = exact_fee_tx.clone();
    policy_version_mutation_tx.set_signature(wrong_policy_version_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, policy_version_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "policy-version payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_fee_coordinate_tx = signed_transfer_with_metadata(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        true,
        metadata_for_policy(&policy, 0),
    );
    let mut fee_coordinate_mutation_tx = exact_fee_tx.clone();
    fee_coordinate_mutation_tx.set_signature(wrong_fee_coordinate_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_coordinate_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-coordinate payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_principal_amount_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Quantity::from(2_u32),
        Some((policy.fee.clone(), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let mut principal_amount_mutation_tx = exact_fee_tx.clone();
    principal_amount_mutation_tx.set_signature(wrong_principal_amount_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, principal_amount_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "principal-amount payload mutation must fail signature admission, got {signature_error}"
    );
    let (alternate_recipient, _) = account(4);
    let wrong_principal_recipient_tx = signed_transfer_with_principal_and_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &alternate_recipient,
        &fee_asset,
        Quantity::from(1_u32),
        Some((policy.fee.clone(), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let mut principal_recipient_mutation_tx = exact_fee_tx.clone();
    principal_recipient_mutation_tx.set_signature(wrong_principal_recipient_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, principal_recipient_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "principal-recipient payload mutation must fail signature admission, got {signature_error}"
    );
    let exact_batch_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Quantity::from(1_u64),
        Quantity::from(1_u64),
    );
    let exact_batch_result = accept_transaction_error(&state, exact_batch_tx.clone());
    assert_eq!(exact_batch_result, "ok");
    let wrong_batch_principal_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Quantity::from(1_u64),
        Quantity::from(2_u64),
    );
    let mut batch_principal_mutation_tx = exact_batch_tx.clone();
    batch_principal_mutation_tx.set_signature(wrong_batch_principal_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_principal_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-principal payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_batch_source_tx = signed_batch_transfer_with_entries(
        &state,
        &user,
        &user_key_pair,
        &policy,
        vec![
            TransferAssetBatchEntry::new(
                recipient.clone(),
                recipient.clone(),
                fee_asset.clone(),
                1_u32,
            ),
            TransferAssetBatchEntry::new(user.clone(), recipient.clone(), fee_asset.clone(), 1_u32),
            TransferAssetBatchEntry::new(
                user.clone(),
                policy_treasury_account(&policy),
                fee_asset.clone(),
                quantity("0.2"),
            ),
        ],
    );
    let mut batch_source_mutation_tx = exact_batch_tx.clone();
    batch_source_mutation_tx.set_signature(wrong_batch_source_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_source_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-source payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_batch_amount_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &policy,
        Quantity::from(2_u64),
        Quantity::from(1_u64),
    );
    let mut batch_amount_mutation_tx = exact_batch_tx.clone();
    batch_amount_mutation_tx.set_signature(wrong_batch_amount_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_amount_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-amount payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_batch_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "wrong_batch_token".parse().expect("asset name"),
    );
    let wrong_batch_asset_tx = signed_batch_transfer_with_entries(
        &state,
        &user,
        &user_key_pair,
        &policy,
        vec![
            TransferAssetBatchEntry::new(user.clone(), recipient.clone(), wrong_batch_asset, 1_u32),
            TransferAssetBatchEntry::new(user.clone(), recipient.clone(), fee_asset.clone(), 1_u32),
            TransferAssetBatchEntry::new(
                user.clone(),
                policy_treasury_account(&policy),
                fee_asset.clone(),
                quantity("0.2"),
            ),
        ],
    );
    let mut batch_asset_mutation_tx = exact_batch_tx.clone();
    batch_asset_mutation_tx.set_signature(wrong_batch_asset_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_asset_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-asset payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_batch_recipient_tx = signed_batch_transfer_with_principal_amounts(
        &state,
        &user,
        &user_key_pair,
        &alternate_recipient,
        &fee_asset,
        &policy,
        Quantity::from(1_u64),
        Quantity::from(1_u64),
    );
    let mut batch_recipient_mutation_tx = exact_batch_tx;
    batch_recipient_mutation_tx.set_signature(wrong_batch_recipient_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, batch_recipient_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "batch-recipient payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_fee_amount_tx = signed_transfer_with_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Some((quantity("0.11"), policy_treasury_account(&policy))),
        metadata_for_policy(&policy, 1),
    );
    let mut fee_amount_mutation_tx = exact_fee_tx.clone();
    fee_amount_mutation_tx.set_signature(wrong_fee_amount_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_amount_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-amount payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_fee_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").expect("domain id"),
        "wrong_fee_token".parse().expect("asset name"),
    );
    let wrong_fee_asset_tx = signed_transfer_with_explicit_fee_asset_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &wrong_fee_asset,
        policy.fee.clone(),
        policy_treasury_account(&policy),
        metadata_for_policy(&policy, 1),
    );
    let mut fee_asset_mutation_tx = exact_fee_tx.clone();
    fee_asset_mutation_tx.set_signature(wrong_fee_asset_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_asset_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-asset payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_fee_source_tx = signed_transfer_with_explicit_fee_source_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        &recipient,
        policy.fee.clone(),
        policy_treasury_account(&policy),
        metadata_for_policy(&policy, 1),
    );
    let mut fee_source_mutation_tx = exact_fee_tx.clone();
    fee_source_mutation_tx.set_signature(wrong_fee_source_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, fee_source_mutation_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-source payload mutation must fail signature admission, got {signature_error}"
    );
    let wrong_treasury_tx = signed_transfer_with_fee_instruction(
        &state,
        &user,
        &user_key_pair,
        &recipient,
        &fee_asset,
        Some((policy.fee.clone(), recipient.clone())),
        metadata_for_policy(&policy, 1),
    );
    exact_fee_tx.set_signature(wrong_treasury_tx.signature().clone());
    let signature_error = accept_transaction_error(&state, exact_fee_tx);
    assert!(
        signature_error.contains("SignatureVerification")
            || signature_error.contains("signature verification"),
        "fee-treasury payload mutation must fail signature admission, got {signature_error}"
    );
}
