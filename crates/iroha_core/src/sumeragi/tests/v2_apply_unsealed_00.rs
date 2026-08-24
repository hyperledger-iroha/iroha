use super::*;
use crate::sumeragi::v2_core::{EventTag, Generation};
use crate::{
    block::BlockBuilder,
    governance::manifest::{
        GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
    },
    lane_consensus::LaneExecutablePayloadV1,
    query::{
        provider_ingest_finalized::{
            ProviderIngestFinalizedArchiveBoundsV1, ProviderIngestFinalizedArchiveInsertOutcomeV1,
            ProviderIngestFinalizedArchiveV1,
        },
        reputation_finalized::{
            ReputationFinalizedArchive, ReputationFinalizedArchiveBounds,
            ReputationFinalizedArchiveError, ReputationFinalizedArchiveInsertOutcome,
            ReputationFinalizedArchiveRetentionApprovalRecordV1,
            ReputationFinalizedArchiveRetentionAuthorityBindingV1,
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1,
            ReputationFinalizedArchiveRetentionAuthorityQualificationV1,
            ReputationFinalizedArchiveRetentionAuthorityV1,
        },
        store::LiveQueryStore,
    },
    queue::{LaneQueueReservationScopeV1, execution_context_for_routing_plan},
    state::World,
    sumeragi::{
        v2_body_store::{
            BlockSignaturePolicy, DurableBodyReceipt, V2BodyStore, ValidatedBodyReceipt,
        },
        v2_effects::ApplyTask,
    },
    tx::AcceptedTransaction,
};
use iroha_config::parameters::actual::{LaneConfig as RuntimeLaneConfig, Queue as QueueConfig};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    ChainId, Level, Registrable, ValidationFail,
    account::Account,
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    block::{
        BlockExecutionContextBundle, BlockHeader, BlockSignature, SignedBlock,
        consensus::{CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1},
        consensus_v2 as wire,
    },
    consensus::{ConsensusKeyRecord, ConsensusKeyStatus, VALIDATOR_SET_HASH_VERSION_V1},
    domain::{Domain, DomainId},
    isi::{
        InstructionBox, Log, Mint, SetParameter,
        sorafs::{
            SetSorafsOrderbookPolicy, SetSorafsReputationJournalAuthorityPolicy,
            SetSorafsReservePolicy,
        },
    },
    merge::{MergeExecutionBatch, MergeLaneExecution, MergeLedgerEntry, MergeQuorumCertificate},
    nexus::{DataSpaceId, LaneId},
    parameter::{Parameter, system::SumeragiParameter},
    peer::PeerId,
    permission::{Permission, Permissions},
    sorafs::{
        orderbook::{ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1},
        reputation::{
            REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1, ReputationJournalAuthorityPolicyV1,
        },
        reserve::{RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReservePolicyV1},
    },
    transaction::{
        TransactionBuilder, TransactionEntrypoint,
        error::TransactionRejectionReason,
        signed::{TransactionResult, TransactionResultInner},
    },
    trigger::DataTriggerSequence,
};
use iroha_executor_data_model::permission::sorafs::{
    CanManageSorafsReputationJournalPolicy, CanSetSorafsPricing, CanSetSorafsReservePolicy,
};
use sorafs_manifest::XorQuantity;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    sync::{Arc, Mutex},
};
include!("v2_apply_unsealed_00_reputation_retention_authority.rs");
#[test]
fn restart_recovery_classification_distinguishes_commit_boundaries() {
    assert!(
        V2ApplyError::Kura(crate::kura::Error::DaBlockRewriteCommitStateUnknown {
            detail: "unknown marker".to_owned(),
        })
        .requires_restart_recovery()
    );
    assert!(
        V2ApplyError::Kura(
            crate::kura::Error::CanonicalBlockCommittedRecoveryRequired {
                detail: "new marker won".to_owned(),
            }
        )
        .requires_restart_recovery()
    );
    assert!(
        V2ApplyError::committed_recovery_required(
            "post-apply metadata",
            &"injected persistence failure",
        )
        .requires_restart_recovery()
    );
    assert!(
        !V2ApplyError::Kura(crate::kura::Error::IO(
            std::io::Error::other("pre-marker retry"),
            std::path::PathBuf::from("pre-marker-stage"),
        ))
        .requires_restart_recovery()
    );
}
#[test]
fn native_amx_prevote_byte_failures_have_precommit_error_classification() {
    let construction = V2ApplyService::classify_native_amx_evidence_byte_budget_error(
        NativeAmxParticipantApplicationEvidenceByteBudgetError::ArtifactConstruction,
    );
    assert!(matches!(
        &construction,
        V2ApplyError::ExecutionCommitment(_)
    ));
    assert!(!construction.requires_restart_recovery());
    let budget = V2ApplyService::classify_native_amx_evidence_byte_budget_error(
        NativeAmxParticipantApplicationEvidenceByteBudgetError::Budget(
            "configured Native AMX artifact pair is oversized".to_owned(),
        ),
    );
    assert!(matches!(&budget, V2ApplyError::Validation(_)));
    assert!(!budget.requires_restart_recovery());
}
struct ApplyFixture {
    context: wire::HeightContext,
    body: SignedBlock,
    manifest: wire::PayloadManifest,
    task: ApplyTask,
    service: V2ApplyService,
    state: Arc<State>,
    kura: Arc<Kura>,
    body_root: tempfile::TempDir,
    genesis_key: KeyPair,
    validator_keys: Vec<KeyPair>,
    custody_account: AccountId,
    treasury_account: AccountId,
    include_projection_policies: bool,
    include_native_lane: bool,
}
fn fixture_orderbook_policy(authority: &AccountId) -> OrderbookAdmissionPolicyV1 {
    OrderbookAdmissionPolicyV1 {
        version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        market_id: [0xA5; 32],
        matcher_authority: authority.clone(),
        settlement_authority: authority.clone(),
        paused: false,
        min_order_gib: 2,
        max_order_gib: 1_024,
        price_tick_micro_xor: 10,
        max_maker_fee_bps: 100,
        max_taker_fee_bps: 200,
        max_order_lifetime_secs: 3_600,
        max_receipt_age_secs: 300,
        max_clock_skew_secs: 5,
        max_receipt_bytes: 1_024,
        max_receipts_per_channel: 2,
    }
}
fn fixture_reserve_policy(
    authority: &AccountId,
    custody_account: AccountId,
    treasury_account: AccountId,
) -> ReserveAuthorityPolicyV1 {
    ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: fixture_reserve_asset_definition(),
        custody_account,
        treasury_account,
        operations_authority: authority.clone(),
        decision_authority: authority.clone(),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: XorQuantity::try_from_micro(1_000_000_000)
            .expect("valid fixture debt ceiling"),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    }
}
fn fixture_world(
    transaction_authority: &AccountId,
    custody_account: &AccountId,
    treasury_account: &AccountId,
    include_projection_policies: bool,
    include_native_lane: bool,
) -> World {
    let reserve_asset_definition = fixture_reserve_asset_definition();
    let reserve_domain_id =
        DomainId::try_new("sorafs", "universal").expect("valid fixture settlement domain");
    let reserve_domain = Domain::new(reserve_domain_id.clone()).build(transaction_authority);
    let reserve_asset = AssetDefinition::numeric(
        reserve_asset_definition,
        "XOR".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(reserve_domain_id),
    )
    .build(transaction_authority);
    let mut fixture_domains = vec![reserve_domain];
    if include_native_lane {
        fixture_domains.extend((0..2).map(|index| {
            Domain::new(
                DomainId::try_new(format!("nativeparticipant{index}"), "independent-dataspace")
                    .expect("valid grouped Native participant domain"),
            )
            .build(transaction_authority)
        }));
    }
    let mut world = World::with_assets(
        fixture_domains,
        [
            Account::new(transaction_authority.clone()).build(transaction_authority),
            Account::new(custody_account.clone()).build(custody_account),
            Account::new(treasury_account.clone()).build(treasury_account),
        ],
        [reserve_asset],
        [],
        [],
    );
    if include_projection_policies {
        let mut authority_permissions = Permissions::new();
        authority_permissions.insert(Permission::from(CanManageSorafsReputationJournalPolicy));
        authority_permissions.insert(Permission::from(CanSetSorafsPricing));
        authority_permissions.insert(Permission::from(CanSetSorafsReservePolicy));
        world
            .account_permissions
            .insert(transaction_authority.clone(), authority_permissions);
    }
    world
}
fn install_fixture_validator_authority(
    state: &State,
    context: &wire::HeightContext,
    validator_set_pops: &[Vec<u8>],
) {
    assert_eq!(
        context.roster.len(),
        validator_set_pops.len(),
        "fixture roster and validator PoPs must remain positionally aligned"
    );
    let mut world_block = state.world.block();
    {
        let mut peers = world_block.peers_mut_for_testing().transaction();
        for validator in &context.roster {
            if !peers.iter().any(|peer| peer == &validator.validator) {
                peers.push(validator.validator.clone());
            }
        }
        peers.apply();
    }
    for (validator, pop) in context.roster.iter().zip(validator_set_pops) {
        let public_key = validator.validator.public_key().clone();
        let id = crate::state::derive_validator_key_id(&public_key);
        let record = ConsensusKeyRecord {
            id: id.clone(),
            public_key,
            pop: Some(pop.clone()),
            activation_height: 0,
            expiry_height: None,
            hsm: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        world_block
            .consensus_keys
            .insert(id.clone(), record.clone());
        world_block
            .consensus_keys_by_pk
            .insert(record.public_key.to_string(), vec![id]);
    }
    world_block.commit();
    let validators = context
        .roster
        .iter()
        .map(|validator| AccountId::new(validator.validator.public_key().clone()))
        .collect::<Vec<_>>();
    let validator_bindings = validators
        .iter()
        .zip(&context.roster)
        .map(|(validator, power)| ManifestValidatorBinding {
            validator: validator.clone(),
            peer_id: power.validator.clone(),
            torii_url: None,
        })
        .collect();
    let primary_lane = state
        .nexus_snapshot()
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == LaneId::SINGLE)
        .cloned()
        .expect("apply fixture has the primary lane");
    let status = LaneManifestStatus {
        lane: primary_lane.id,
        alias: primary_lane.alias,
        dataspace: primary_lane.dataspace_id,
        visibility: primary_lane.visibility,
        storage: primary_lane.storage,
        governance: primary_lane.governance,
        manifest_path: Some(std::path::PathBuf::from(
            "/tmp/sumeragi-v2-apply-fixture-manifest.json",
        )),
        governance_rules: Some(GovernanceRules {
            validators,
            validator_bindings,
            ..GovernanceRules::default()
        }),
        privacy_commitments: Vec::new(),
    };
    let mut statuses = {
        let manifests = state.lane_manifests.read();
        manifests
            .statuses()
            .into_iter()
            .map(|status| (status.lane, status))
            .collect::<BTreeMap<_, _>>()
    };
    statuses.insert(LaneId::SINGLE, status);
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
    let mut expected = context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect::<Vec<_>>();
    expected.sort();
    let mut actual = state
        .resolve_lane_committee_at_height(
            crate::state::LaneAuthorityRoute::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            context.height,
        )
        .expect("fixture lane authority must resolve")
        .into_validators();
    actual.sort();
    assert_eq!(
        actual, expected,
        "fixture must expose every authenticated validator as lane authority"
    );
}
fn install_fixture_kagemusha_runtime_lifecycle(
    state: &State,
    runtime_effective_config_sha256: [u8; 32],
) {
    let lifecycle = crate::smartcontracts::isi::offline::staged_lifecycle_for_test(
        runtime_effective_config_sha256,
        state.network_id,
    );
    let key: iroha_data_model::state_path::StatePath =
        iroha_data_model::offline::kagemusha_v4_release_lifecycle_state_key(
            &lifecycle.artifact_binding.manifest_sha256,
        )
        .expect("derive canonical Kagemusha lifecycle key")
        .parse()
        .expect("parse canonical Kagemusha lifecycle key");
    let mut world_block = state.world.block();
    world_block.smart_contract_state.insert(
        key,
        norito::encode_canonical(&lifecycle).expect("encode Kagemusha lifecycle fixture"),
    );
    world_block.commit();
}
impl ApplyFixture {
    fn new() -> Self {
        Self::new_with_lane_payload(false)
    }
    fn new_with_lane_payload(include_lane_payload: bool) -> Self {
        Self::new_with_options(include_lane_payload, false, false, false)
    }
    fn new_with_reputation_archive() -> Self {
        Self::new_with_options(false, true, false, false)
    }
    fn new_with_lane_lifecycle() -> Self {
        Self::new_with_options(false, false, true, false)
    }
    fn new_with_native_lane_lifecycle() -> Self {
        Self::new_with_options(false, false, true, true)
    }
    fn new_for_production_recovered_decision_apply() -> Self {
        Self::new_with_options_and_network(false, false, false, false, true)
    }
    fn new_for_production_recovered_decision_apply_with_lane_lifecycle() -> Self {
        Self::new_with_options_and_network(false, false, true, false, true)
    }
    fn new_for_kagemusha_runtime_projection() -> Self {
        Self::new_with_options_and_network_and_kagemusha(
            false,
            false,
            false,
            false,
            false,
            Some(([0x55; 32], Some([0x55; 32]))),
        )
    }
    fn new_with_options(
        include_lane_payload: bool,
        include_projection_policies: bool,
        include_lane_lifecycle: bool,
        include_native_lane: bool,
    ) -> Self {
        Self::new_with_options_and_network(
            include_lane_payload,
            include_projection_policies,
            include_lane_lifecycle,
            include_native_lane,
            false,
        )
    }
    fn new_with_options_and_network(
        include_lane_payload: bool,
        include_projection_policies: bool,
        include_lane_lifecycle: bool,
        include_native_lane: bool,
        match_context_network: bool,
    ) -> Self {
        Self::new_with_options_and_network_and_kagemusha(
            include_lane_payload,
            include_projection_policies,
            include_lane_lifecycle,
            include_native_lane,
            match_context_network,
            None,
        )
    }
    fn new_with_options_and_network_and_kagemusha(
        include_lane_payload: bool,
        include_projection_policies: bool,
        include_lane_lifecycle: bool,
        include_native_lane: bool,
        match_context_network: bool,
        kagemusha_runtime: Option<([u8; 32], Option<[u8; 32]>)>,
    ) -> Self {
        let chain_id: ChainId = "sumeragi-v2-apply-crash-test".into();
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let transaction_key = KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::Ed25519)
            .expect("deterministic transaction key");
        let custody_key = KeyPair::try_from_seed(vec![0xE8; 32], Algorithm::Ed25519)
            .expect("deterministic custody key");
        let treasury_key = KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::Ed25519)
            .expect("deterministic treasury key");
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let mut context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("sumeragi-v2-apply-crash-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"apply crash fixture Nexus/AMX"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
            leader_seed: [0x63; 32],
        };
        context.validate().expect("valid fixture context");
        let kura = if include_lane_lifecycle {
            crate::sumeragi::v2_lane_work::tests::locked_lane_work_test_kura(
                iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
            )
        } else {
            Kura::blank_kura_for_testing()
        };
        let transaction_authority = AccountId::new(transaction_key.public_key().clone());
        let custody_account = AccountId::new(custody_key.public_key().clone());
        let treasury_account = AccountId::new(treasury_key.public_key().clone());
        let world = fixture_world(
            &transaction_authority,
            &custody_account,
            &treasury_account,
            include_projection_policies,
            include_native_lane,
        );
        let mut state = if match_context_network {
            State::new_with_chain_and_network_id_for_testing(
                world,
                Arc::clone(&kura),
                LiveQueryStore::start_test(),
                chain_id.clone(),
                context.network_id,
            )
        } else {
            State::new_with_chain_for_testing(
                world,
                Arc::clone(&kura),
                LiveQueryStore::start_test(),
                chain_id.clone(),
            )
        };
        let validator_set_pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect::<Vec<_>>();
        install_fixture_validator_authority(&state, &context, &validator_set_pops);
        if include_native_lane {
            install_fixture_native_lane(&mut state, &mut context);
        }
        if let Some((expected, local)) = kagemusha_runtime {
            install_fixture_kagemusha_runtime_lifecycle(&state, expected);
            if let Some(local) = local {
                state
                    .install_kagemusha_runtime_effective_config_sha256(local)
                    .expect("install Kagemusha runtime projection fixture");
            }
        }
        if match_context_network {
            context.nexus_amx_context_hash =
                crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state);
        }
        context.execution_policy_hash =
            crate::sumeragi::v2_recovery::committed_execution_policy_hash(&state)
                .expect("derive apply fixture execution policy");
        context
            .validate()
            .expect("valid fixture context with committed execution policy");
        let state = Arc::new(state);
        let mut commit_topology = state.commit_topology.block();
        commit_topology.clear();
        for validator in &context.roster {
            commit_topology.push(validator.validator.clone());
        }
        commit_topology.commit();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
        let queue = fixture_queue(state.as_ref(), events_sender.clone());
        let block_cadence = state.sumeragi_block_cadence();
        let service = V2ApplyService::new(
            Arc::clone(&state),
            Arc::clone(&queue),
            Arc::clone(&kura),
            None,
            None,
            block_cadence,
            transaction_authority.clone(),
            events_sender,
            validator_set_pops,
        );
        assert_eq!(
            service.block_cadence,
            state.sumeragi_block_cadence(),
            "apply fixture validator cadence must equal the signed State cadence"
        );
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let leader_index = context.leader(0);
        let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
            &state.nexus_snapshot(),
            context.height,
        );
        let confidential_features = {
            let state_view = state.view();
            let digest = crate::state::compute_confidential_feature_digest(
                state_view.world(),
                &state_view.zk,
                state_view.sccp_registry.as_ref(),
                context.height,
            );
            (!digest.is_empty()).then_some(digest)
        };
        let build_genesis_body =
            |transaction: iroha_data_model::transaction::signed::SignedTransaction,
             execution_context: Option<BlockExecutionContextBundle>| {
                let creation_time_ms = (transaction.creation_time() + Duration::from_millis(1))
                    .as_millis()
                    .try_into()
                    .expect("fixture creation time fits u64");
                let mut header = BlockHeader::new(
                    NonZeroU64::new(1).expect("non-zero fixture height"),
                    None,
                    None,
                    None,
                    creation_time_ms,
                    0,
                );
                header.set_confidential_features(confidential_features);
                let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
                builder.push_transaction(transaction);
                builder.set_da_proof_policies(Some(proof_policy_bundle.clone()));
                builder.set_execution_context(execution_context);
                builder
                    .try_build_with_signature(0, transaction_key.private_key())
                    .expect("sign valid genesis fixture body")
                    .canonical_resultless_proposal()
            };
        let reputation_policy = ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: transaction_authority.clone(),
            dispute_recorder_authority: transaction_authority.clone(),
            token_recorder_authority: transaction_authority.clone(),
            max_source_age_ms: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
        };
        let transaction_instructions = || {
            let mut instructions = if kagemusha_runtime.is_some() {
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "Kagemusha runtime projection gate fixture".to_owned(),
                ))]
            } else {
                vec![InstructionBox::from(SetParameter::new(
                    Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)),
                ))]
            };
            if include_projection_policies {
                instructions.push(InstructionBox::from(
                    SetSorafsReputationJournalAuthorityPolicy::new(reputation_policy.clone()),
                ));
                instructions.push(InstructionBox::from(SetSorafsOrderbookPolicy::new(
                    fixture_orderbook_policy(&transaction_authority),
                )));
                instructions.push(InstructionBox::from(SetSorafsReservePolicy::new(
                    fixture_reserve_policy(
                        &transaction_authority,
                        custody_account.clone(),
                        treasury_account.clone(),
                    ),
                )));
            }
            instructions
        };
        let body = if include_lane_payload {
            let transaction = TransactionBuilder::new_genesis(
                transaction_authority.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(transaction_instructions())
            .sign(transaction_key.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
            let routing_plan = queue
                .route_plan_with_state(&accepted, state.as_ref())
                .expect("resolve canonical fixture route");
            let route = routing_plan.coordinator_route();
            let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
            let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
                state.as_ref(),
                kura.as_ref(),
                &context,
                0,
                &context.roster[usize::try_from(leader_index).expect("leader index")].validator,
                std::slice::from_ref(&route),
                std::slice::from_ref(&entrypoint_hash),
            )
            .expect("derive canonical fixture lane plan");
            assert!(lane_plan.unavailable_indices.is_empty());
            assert_eq!(lane_plan.ownerships.len(), 1);
            let execution_context =
                BlockExecutionContextBundle::new(vec![execution_context_for_routing_plan(
                    transaction.hash_as_entrypoint(),
                    &routing_plan,
                )])
                .with_lane_payload_ownerships(lane_plan.ownerships);
            build_genesis_body(transaction, Some(execution_context))
        } else {
            let transaction = TransactionBuilder::new_genesis(
                transaction_authority.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(transaction_instructions())
            .sign(transaction_key.private_key());
            build_genesis_body(transaction, None)
        };
        let canonical_wire = body.encode_wire().expect("canonical block wire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: body.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest =
            crate::sumeragi::v2_chunks::encode_payload(&context, round, subject, &canonical_wire)
                .expect("fixture manifest")
                .into_parts()
                .0;
        let execution_commitment = service
            .validate_candidate(&context, &body)
            .expect("derive exact fixture execution commitment");
        let mut certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                    &preimage,
                )
                .expect("sign fixture Commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate fixture Commit votes");
        let body_root = tempfile::tempdir().expect("body-store directory");
        let mut body_store = V2BodyStore::open_with_policy(
            body_root.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(transaction_key.public_key().clone()),
        )
        .expect("open body store");
        let durable = body_store
            .store(manifest.clone(), canonical_wire)
            .expect("persist exact body");
        let validated = body_store
            .validate(&durable, |candidate| {
                service.validate_candidate(&context, candidate)
            })
            .expect("persist production validation marker");
        let task = ApplyTask::for_test(
            1,
            EventTag::new(1, 0, Generation::new(1)),
            subject,
            certificate,
            validated,
        );
        drop(body_store);
        Self {
            context,
            body,
            manifest,
            task,
            service,
            state,
            kura,
            body_root,
            genesis_key: transaction_key,
            validator_keys: keys,
            custody_account,
            treasury_account,
            include_projection_policies,
            include_native_lane,
        }
    }
    fn reopen_body_store(&self) -> V2BodyStore {
        V2BodyStore::open_with_policy(
            self.body_root.path(),
            self.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(
                self.service
                    .genesis_account
                    .expect_single_signatory()
                    .clone(),
            ),
        )
        .expect("reopen body store after crash")
    }
    fn restart_service_from_last_finalized_snapshot(&self) -> (V2ApplyService, Arc<State>) {
        let authority = self.service.genesis_account.clone();
        let world = fixture_world(
            &authority,
            &self.custody_account,
            &self.treasury_account,
            self.include_projection_policies,
            self.include_native_lane,
        );
        let state = Arc::new(State::new_with_chain_for_testing(
            world,
            Arc::clone(&self.kura),
            LiveQueryStore::start_test(),
            self.service.state.chain_id.clone(),
        ));
        install_fixture_validator_authority(
            &state,
            &self.context,
            &self.service.validator_set_pops,
        );
        let mut commit_topology = state.commit_topology.block();
        commit_topology.clear();
        for validator in &self.context.roster {
            commit_topology.push(validator.validator.clone());
        }
        commit_topology.commit();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
        let queue = fixture_queue(state.as_ref(), events_sender.clone());
        let service = V2ApplyService::new(
            Arc::clone(&state),
            queue,
            Arc::clone(&self.kura),
            self.service.provider_ingest_finalized_archive.clone(),
            self.service.reputation_finalized_archive.clone(),
            self.service.block_cadence,
            authority,
            events_sender,
            self.service.validator_set_pops.clone(),
        );
        (service, state)
    }
    fn restart_service_with_kagemusha_runtime_projection(
        &self,
        local_runtime_effective_config_sha256: Option<[u8; 32]>,
    ) -> (V2ApplyService, Arc<State>) {
        let (service, state) = self.restart_service_from_last_finalized_snapshot();
        install_fixture_kagemusha_runtime_lifecycle(state.as_ref(), [0x55; 32]);
        if let Some(local) = local_runtime_effective_config_sha256 {
            state
                .install_kagemusha_runtime_effective_config_sha256(local)
                .expect("install restarted Kagemusha runtime projection");
        }
        (service, state)
    }
    fn execute(&self, store: &mut V2BodyStore) -> Result<(), V2ApplyError> {
        self.service
            .execute(&self.context, store, &self.task)
            .map(drop)
    }
    fn persist_exact_v2_finality_chain(&self, blocks: &[&SignedBlock]) {
        assert!(
            !blocks.is_empty(),
            "finality fixture chain must not be empty"
        );
        let mut parent_commit_qc = None;
        for block in blocks {
            let height = block.header().height().get();
            let mut context = self.context.clone();
            context.height = height;
            context.parent_commit_qc = parent_commit_qc;
            context
                .validate()
                .expect("valid exact finality fixture context");
            let executed_block_wire = block
                .encode_wire()
                .expect("encode exact executed block wire");
            let mut execution_commitment =
                wire::ExecutionCommitment::without_topups_or_merge_carrier(
                    Hash::new(b"v2 apply reservation finality parent state"),
                    Hash::new(b"v2 apply reservation finality post state"),
                    Hash::new(b"v2 apply reservation finality ordinary writes"),
                    u64::try_from(executed_block_wire.len())
                        .expect("executed block wire length fits u64"),
                    Hash::new(&executed_block_wire),
                );
            execution_commitment.merge_carrier = block
                .execution_context()
                .and_then(|bundle| bundle.merge_entry.as_ref())
                .map(|reference| wire::MergeCarrierCommitmentV1::new(reference.entry_hash));
            execution_commitment
                .validate()
                .expect("valid exact finality execution commitment");
            let subject = wire::BlockSubject {
                parent_block_hash: block.header().prev_block_hash(),
                block_hash: block.hash(),
                payload_hash: block
                    .canonical_proposal_wire_hash()
                    .expect("hash exact canonical proposal wire"),
            };
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height,
                view: block.header().view_change_index(),
            };
            let mut commit_qc = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            };
            let preimage = commit_qc
                .signer_preimage(&context, 0)
                .expect("valid exact finality fixture signer");
            let signatures = commit_qc
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        self.validator_keys[usize::try_from(*index).expect("fixture signer index")]
                            .private_key(),
                        &preimage,
                    )
                    .expect("sign exact finality fixture vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate exact finality fixture votes");
            let artifact = wire::finality::V2FinalityArtifact::new(
                context,
                subject,
                commit_qc,
                self.service.validator_set_pops.clone(),
            );
            artifact
                .verify()
                .expect("exact finality fixture is cryptographically valid");
            let receipt = self
                .kura
                .store_v2_finality_artifact(&artifact)
                .expect("persist exact finality fixture");
            assert_eq!(receipt.height(), height);
            assert_eq!(receipt.block_hash(), block.hash());
            parent_commit_qc = Some(artifact.commit_qc);
        }
    }
    fn assert_no_post_apply_sidecars(&self) {
        assert!(
            self.kura
                .wsv_checkpoint(self.context.height)
                .expect("read checkpoint")
                .is_none()
        );
        assert!(
            self.kura
                .commit_manifest(self.context.height)
                .expect("read manifest")
                .is_none()
        );
        assert!(
            self.kura
                .v2_finality_artifact(self.context.height)
                .expect("read finality")
                .is_none()
        );
    }
    fn assert_no_apply_mutation(&self) {
        assert_eq!(self.state.committed_height(), 0);
        assert_eq!(self.kura.exact_durable_blocks_count().unwrap(), 0);
        self.assert_no_post_apply_sidecars();
    }
    fn assert_complete(&self) {
        self.assert_complete_for_state(self.state.as_ref());
    }
    fn assert_complete_for_state(&self, state: &State) {
        assert_eq!(state.committed_height(), 1);
        assert_eq!(self.kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            self.kura
                .get_durable_block_hash(NonZeroUsize::new(1).expect("height")),
            Some(self.body.hash())
        );
        let durable = self
            .kura
            .get_block(NonZeroUsize::new(1).expect("height"))
            .expect("read complete durable block");
        assert!(durable.has_results());
        assert_eq!(
            durable.results().len(),
            self.body.external_entrypoint_count()
        );
        assert!(durable.results().all(|result| result.is_ok()));
        assert_eq!(durable.execution_context(), self.body.execution_context());
        assert!(
            self.kura
                .wsv_checkpoint(self.context.height)
                .expect("read checkpoint")
                .is_some()
        );
        let commit_manifest = self
            .kura
            .commit_manifest(self.context.height)
            .expect("read manifest")
            .expect("commit manifest exists");
        let artifact = self
            .kura
            .v2_finality_artifact(self.context.height)
            .expect("read finality")
            .expect("finality exists");
        assert_eq!(artifact.height_context, self.context);
        assert_eq!(artifact.subject, self.manifest.subject);
        assert_eq!(artifact.commit_qc, self.task.certificate().clone());
        assert!(
            self.kura
                .commit_manifest_has_wsv_binding(&commit_manifest)
                .expect("read checkpoint-to-manifest binding")
        );
        assert!(
            commit_manifest.binds_authenticated_v2_commit_authority(&artifact),
            "manifest must retain the exact QC roots and complete v2 authority seal"
        );
    }
}
struct SuccessorApplyFixture {
    context: wire::HeightContext,
    body: SignedBlock,
    task: ApplyTask,
    _body_root: tempfile::TempDir,
    store: V2BodyStore,
}
fn successor_height_context(fixture: &ApplyFixture) -> wire::HeightContext {
    let mut context = fixture.context.clone();
    context.height = 2;
    context.parent_commit_qc = Some(fixture.task.certificate().clone());
    context.validate().expect("valid successor height context");
    context
}
fn build_successor_apply_fixture(fixture: &ApplyFixture) -> SuccessorApplyFixture {
    build_successor_apply_fixture_with_autonomous_payloads(fixture, Vec::new())
}
fn build_successor_apply_fixture_with_autonomous_payloads(
    fixture: &ApplyFixture,
    autonomous_lane_payloads: Vec<iroha_data_model::block::AutonomousLanePayloadEnvelopeV1>,
) -> SuccessorApplyFixture {
    assert_eq!(
        fixture.state.committed_height(),
        1,
        "successor fixture requires the committed parent"
    );
    let context = successor_height_context(fixture);
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let transaction = TransactionBuilder::new(
        context.network_id,
        fixture.service.genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "reputation retained-capture successor".to_owned(),
    )])
    .sign(fixture.genesis_key.private_key());
    let leader_index = context.leader(round.view);
    let carries_only_autonomous_payloads = !autonomous_lane_payloads.is_empty();
    let execution_context = if carries_only_autonomous_payloads {
        BlockExecutionContextBundle::new(Vec::new())
            .with_autonomous_lane_payloads(autonomous_lane_payloads)
    } else {
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
        let routing_plan = fixture
            .service
            .queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve successor transaction route");
        let route = routing_plan.coordinator_route();
        let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
        let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
            fixture.state.as_ref(),
            fixture.kura.as_ref(),
            &context,
            round.view,
            &context.roster[usize::try_from(leader_index).expect("successor leader index")]
                .validator,
            std::slice::from_ref(&route),
            std::slice::from_ref(&entrypoint_hash),
        )
        .expect("derive canonical successor lane plan");
        assert!(
            lane_plan.unavailable_indices.is_empty(),
            "successor fixture lane must be available"
        );
        BlockExecutionContextBundle::new(vec![execution_context_for_routing_plan(
            transaction.hash_as_entrypoint(),
            &routing_plan,
        )])
        .with_lane_payload_ownerships(lane_plan.ownerships)
    };
    let mut logical_time = fixture
        .body
        .header()
        .creation_time()
        .checked_add(fixture.service.block_cadence)
        .expect("successor logical time fits Duration");
    if !carries_only_autonomous_payloads {
        logical_time = logical_time.max(
            transaction
                .creation_time()
                .checked_add(Duration::from_millis(1))
                .expect("successor transaction floor fits Duration"),
        );
    }
    let creation_time_ms = logical_time
        .as_millis()
        .try_into()
        .expect("successor creation time fits u64");
    let mut header = BlockHeader::new(
        NonZeroU64::new(context.height).expect("non-zero successor height"),
        Some(fixture.body.hash()),
        None,
        None,
        creation_time_ms,
        0,
    );
    let confidential_features = {
        let state_view = fixture.state.view();
        let digest = crate::state::compute_confidential_feature_digest(
            state_view.world(),
            &state_view.zk,
            state_view.sccp_registry.as_ref(),
            context.height,
        );
        (!digest.is_empty()).then_some(digest)
    };
    header.set_confidential_features(confidential_features);
    let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
        &fixture.state.nexus_snapshot(),
        context.height,
    );
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic successor BLS key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let leader = usize::try_from(leader_index).expect("successor leader index");
    assert_eq!(
        keys[leader].public_key(),
        context.roster[leader].validator.public_key(),
        "successor signer must be the rotating leader"
    );
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    if !carries_only_autonomous_payloads {
        builder.push_transaction(transaction);
    }
    builder.set_da_proof_policies(Some(proof_policy_bundle));
    builder.set_execution_context(Some(execution_context));
    let body = builder
        .try_build_with_signature(u64::from(leader_index), keys[leader].private_key())
        .expect("sign successor proposal")
        .canonical_resultless_proposal();
    let canonical_wire = body.encode_wire().expect("encode successor body");
    let subject = wire::BlockSubject {
        parent_block_hash: Some(fixture.body.hash()),
        block_hash: body.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let manifest =
        crate::sumeragi::v2_chunks::encode_payload(&context, round, subject, &canonical_wire)
            .expect("derive successor payload manifest")
            .into_parts()
            .0;
    let execution_commitment = fixture
        .service
        .validate_candidate(&context, &body)
        .expect("derive successor execution commitment");
    let mut certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = certificate
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keys[usize::try_from(*index).expect("successor signer index")].private_key(),
                &preimage,
            )
            .expect("sign successor Commit vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate successor Commit votes");
    let body_root = tempfile::tempdir().expect("successor body-store directory");
    let mut store = V2BodyStore::open(body_root.path(), context.clone())
        .expect("open successor rotating-leader body store");
    let durable = store
        .store(manifest, canonical_wire)
        .expect("persist exact successor body");
    let validated = store
        .validate(&durable, |candidate| {
            fixture.service.validate_candidate(&context, candidate)
        })
        .expect("persist successor validation marker");
    let task = ApplyTask::for_test(
        2,
        EventTag::new(2, 0, Generation::new(2)),
        subject,
        certificate,
        validated,
    );
    SuccessorApplyFixture {
        context,
        body,
        task,
        _body_root: body_root,
        store,
    }
}
#[test]
fn durable_application_evidence_rejects_identity_mutations() {
    let fixture = ApplyFixture::new();
    let mut store = fixture.reopen_body_store();
    let completion = fixture
        .service
        .execute(&fixture.context, &mut store, &fixture.task)
        .expect("apply exact fixture");
    let committed = fixture
        .kura
        .get_block(NonZeroUsize::new(1).expect("height"))
        .expect("load committed block");
    let artifact = completion.artifact().clone();
    let evidence = DurableApplicationEvidence {
        task_tag: fixture.task.tag(),
        owner_tag: fixture.task.authorized_owner_tag(),
        task_generation: fixture.task.tag().generation().get(),
        task_work_id: fixture.task.id(),
        context: fixture.context.clone(),
        commit_qc: fixture.task.certificate().clone(),
        subject: fixture.task.subject(),
        execution_commitment: fixture.task.validated_receipt().execution_commitment(),
        validated_receipt: fixture.task.validated_receipt().clone(),
        validated_manifest_hash: fixture.task.validated_receipt().durable().manifest_hash(),
        validated_body_frame_hash: fixture.task.validated_receipt().durable().frame_hash(),
        proposal_block_hash: fixture.body.hash(),
        canonical_proposal_wire_hash: fixture
            .body
            .canonical_proposal_wire_hash()
            .expect("hash proposal wire"),
        committed_block_hash: committed.hash(),
        executed_block_wire_hash: committed
            .executed_block_wire_hash()
            .expect("hash executed wire"),
        kura_receipt: completion.receipt().clone(),
        artifact_hash: HashOf::new(&artifact),
        artifact,
        completion_work_id: completion.work_id(),
        state_height_after: fixture.state.committed_height(),
    };
    assert!(evidence.is_exact());
    assert_eq!(
        prospective_application_refinement_projection(
            &fixture.context,
            &fixture.task,
            fixture.body.hash(),
            fixture
                .body
                .canonical_proposal_wire_hash()
                .expect("hash proposal wire"),
            evidence.artifact(),
        )
        .expect("prospective application projection"),
        evidence
            .application_refinement_projection()
            .expect("observed application projection"),
        "preflight and observed durable application identities must be exact"
    );
    assert_eq!(evidence.task_tag(), fixture.task.tag());
    assert_eq!(evidence.owner_tag(), fixture.task.authorized_owner_tag());
    assert_eq!(
        evidence.task_generation(),
        fixture.task.tag().generation().get()
    );
    assert_eq!(evidence.task_work_id(), fixture.task.id());
    assert_eq!(evidence.context(), &fixture.context);
    assert_eq!(evidence.commit_qc(), fixture.task.certificate());
    assert_eq!(evidence.commit_round(), fixture.task.certificate().round);
    assert_eq!(evidence.commit_phase(), wire::GlobalPhase::Commit);
    assert_eq!(
        evidence.commit_signers(),
        fixture.task.certificate().signers.as_slice()
    );
    assert_eq!(
        evidence.commit_aggregate_signature(),
        fixture.task.certificate().aggregate_signature.as_slice()
    );
    assert_eq!(evidence.subject(), fixture.task.subject());
    assert_eq!(
        evidence.execution_commitment(),
        fixture.task.certificate().execution_commitment
    );
    assert_eq!(
        evidence.validated_receipt(),
        fixture.task.validated_receipt()
    );
    assert_eq!(
        evidence.validated_context_id(),
        fixture.task.validated_receipt().durable().context_id()
    );
    assert_eq!(
        evidence.validated_round(),
        fixture.task.validated_receipt().durable().round()
    );
    assert_eq!(evidence.validated_subject(), fixture.task.subject());
    assert_eq!(
        evidence.validated_manifest_hash(),
        fixture.task.validated_receipt().durable().manifest_hash()
    );
    assert_eq!(
        evidence.validated_body_frame_hash(),
        fixture.task.validated_receipt().durable().frame_hash()
    );
    assert_eq!(evidence.proposal_block_hash(), fixture.body.hash());
    assert_eq!(
        evidence.canonical_proposal_wire_hash(),
        fixture.manifest.subject.payload_hash
    );
    assert_eq!(evidence.committed_block_hash(), committed.hash());
    assert_eq!(
        evidence.executed_block_wire_hash(),
        fixture
            .task
            .certificate()
            .execution_commitment
            .executed_block_wire_hash
    );
    assert_eq!(evidence.kura_height(), fixture.context.height);
    assert_eq!(evidence.kura_block_hash(), committed.hash());
    assert_eq!(evidence.kura_context_id(), fixture.context.id());
    assert_eq!(evidence.kura_subject(), fixture.task.subject());
    assert_eq!(
        evidence.kura_certificate(),
        fixture.task.certificate().as_ref()
    );
    assert_eq!(evidence.kura_artifact_hash(), evidence.artifact_hash());
    assert_eq!(evidence.artifact(), completion.artifact());
    assert_eq!(evidence.completion_work_id(), completion.work_id());
    assert_eq!(evidence.state_height_after(), 1);
    assert!(
        fixture
            .service
            .finish_durable_apply_completion(evidence.clone())
            .is_ok(),
        "the exact native evidence must mint the typed completion"
    );
    let mut delayed_decision = evidence.clone();
    delayed_decision.task_tag = EventTag::new(
        delayed_decision.task_tag.height(),
        delayed_decision
            .task_tag
            .view()
            .checked_add(1)
            .expect("fixture lifecycle view increment"),
        Generation::new(
            delayed_decision
                .task_generation
                .checked_add(1)
                .expect("fixture lifecycle generation increment"),
        ),
    );
    delayed_decision.owner_tag = delayed_decision.task_tag;
    delayed_decision.task_generation = delayed_decision.task_tag.generation().get();
    assert!(
        delayed_decision.is_exact(),
        "a current lifecycle owner must retain an exact historical CommitQC"
    );
    assert!(
        fixture
            .service
            .finish_durable_apply_completion(delayed_decision)
            .is_ok(),
        "a delayed CommitQC must mint the typed completion after a timeout fence"
    );
    let mut altered = evidence.clone();
    altered.owner_tag = EventTag::new(
        altered.task_tag.height(),
        altered
            .task_tag
            .view()
            .checked_add(1)
            .expect("fixture owner view increment"),
        altered.task_tag.generation(),
    );
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.task_generation = altered
        .task_generation
        .checked_add(1)
        .expect("fixture generation increment");
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.commit_qc.signers.swap(0, 1);
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.commit_qc.aggregate_signature.push(0xC1);
    assert!(!altered.is_exact());
    let alternate_durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.task.certificate().round,
        fixture.task.subject(),
        fixture.task.validated_receipt().durable().manifest_hash(),
    );
    assert_ne!(
        alternate_durable.frame_hash(),
        fixture.task.validated_receipt().durable().frame_hash()
    );
    let mut altered = evidence.clone();
    altered.validated_receipt = ValidatedBodyReceipt::for_test_with_commitment(
        alternate_durable,
        evidence.execution_commitment(),
    );
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.validated_manifest_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"altered validated manifest identity"));
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.validated_body_frame_hash = Hash::new(b"altered validated body frame identity");
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.canonical_proposal_wire_hash = Hash::new(b"altered proposal wire identity");
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.executed_block_wire_hash = Hash::new(b"altered executed wire identity");
    assert!(!altered.is_exact());
    let mut altered_artifact = evidence.artifact.clone();
    altered_artifact.block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"altered Kura receipt block identity"));
    let mut altered = evidence.clone();
    altered.kura_receipt = KuraV2CommitReceipt::for_test(&altered_artifact);
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.artifact_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"altered finality artifact identity"));
    assert!(!altered.is_exact());
    let mut altered = evidence.clone();
    altered.completion_work_id = EffectWorkId::for_test(2);
    assert!(matches!(
        fixture.service.finish_durable_apply_completion(altered),
        Err(V2ApplyError::CommittedRecoveryRequired {
            stage: "exact application evidence",
            ..
        })
    ));
    let mut altered = evidence;
    altered.state_height_after = 2;
    assert!(!altered.is_exact());
}
fn pending_merge_entry(
    context: &wire::HeightContext,
    view: wire::View,
    label: &[u8],
) -> MergeLedgerEntry {
    let validator_set = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    for index in 0..validator_set.len() {
        bitmap[index / 8] |= 1 << (index % 8);
    }
    MergeLedgerEntry {
        version: MergeLedgerEntry::VERSION,
        epoch_id: context.epoch,
        lane_catalog_hash: Hash::new(b"v2 apply decided-sidecar catalog"),
        active_lanes: Vec::new(),
        incarnation_root: Hash::new(b"v2 apply decided-sidecar incarnations"),
        activation_root: Hash::new(b"v2 apply decided-sidecar activations"),
        lane_snapshots: Vec::new(),
        lane_drain_certificates: Vec::new(),
        execution_batch: None,
        global_state_root: Hash::new(label),
        merge_qc: MergeQuorumCertificate::new(
            view,
            context.epoch,
            context.height,
            HashOf::from_untyped_unchecked(Hash::new(b"v2 apply decided-sidecar parent")),
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"v2 apply decided-sidecar chain"),
            )),
            VALIDATOR_SET_HASH_VERSION_V1,
            HashOf::new(&validator_set),
            validator_set,
            bitmap,
            Vec::new(),
            vec![0x5A; 96],
            Hash::new(label),
        ),
    }
}
fn merge_entry_with_reservation(
    context: &wire::HeightContext,
    entrypoint: TransactionEntrypoint,
    reservation: crate::queue::LaneQueueReservationKeyV2,
) -> (SignedBlock, MergeLedgerEntry) {
    merge_entry_with_reservations(context, vec![(entrypoint, reservation)])
}
fn merge_entry_with_reservations(
    context: &wire::HeightContext,
    members: Vec<(
        TransactionEntrypoint,
        crate::queue::LaneQueueReservationKeyV2,
    )>,
) -> (SignedBlock, MergeLedgerEntry) {
    assert!(!members.is_empty(), "merge fixture group must not be empty");
    let first_reservation = members[0].1;
    assert!(members.iter().all(|(_, reservation)| {
        reservation_group_identity(reservation) == reservation_group_identity(&first_reservation)
    }));
    let parent_key = KeyPair::try_from_seed(vec![0xC8; 32], Algorithm::BlsNormal)
        .expect("derive execution-carrier parent signer");
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
    let parent = BlockBuilder::new_with_time_source(Vec::new(), time_source)
        .chain(0, None)
        .try_sign_with_index(parent_key.private_key(), 0)
        .expect("sign execution-carrier parent")
        .unpack(|_| {});
    let parent = SignedBlock::from(parent);
    let application_block_header = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero fixture carrier height"),
        Some(parent.hash()),
        None,
        None,
        2,
        0,
    );
    let entrypoint_hashes = members
        .iter()
        .map(|(entrypoint, _)| Hash::from(entrypoint.hash()))
        .collect::<Vec<_>>();
    let results = members
        .iter()
        .map(|_| TransactionResult::from(Ok(DataTriggerSequence::default())))
        .collect::<Vec<_>>();
    let result_hashes = results
        .iter()
        .map(|result| Hash::from(result.hash()))
        .collect::<Vec<_>>();
    let validator_set = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let validator_count =
        u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
    let min_quorum = wire::DualQuorum::count_threshold(validator_count)
        .expect("non-empty fixture validator set has a quorum");
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: first_reservation.lane_id,
        dataspace_id: first_reservation.dataspace_id,
        lane_incarnation: first_reservation.lane_incarnation,
        proposal_height: first_reservation.proposal_height,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: first_reservation.lane_block_height,
        lane_block_view: first_reservation.lane_block_view,
        subject_hash: Hash::new(b"v2 reservation fixture subject"),
        payload_ownership_hash: Hash::new(b"v2 reservation fixture ownership"),
        rbc_instance_hash: Hash::new(b"v2 reservation fixture RBC"),
        accepted_candidate_indices: (0..members.len())
            .map(|index| u64::try_from(index).expect("fixture index fits u64"))
            .collect(),
        accepted_transaction_hashes: entrypoint_hashes.clone(),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count,
        min_quorum,
        qc_mode_tag: "v2-reservation-lifecycle-test".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    crate::lane_consensus::validate_lane_block_proposal(&proposal)
        .expect("reservation fixture proposal must satisfy production ingress validation");
    let settlement_commitment = LaneBlockCommitment {
        block_height: first_reservation.lane_block_height,
        lane_id: first_reservation.lane_id,
        lane_incarnation: first_reservation.lane_incarnation,
        dataspace_id: first_reservation.dataspace_id,
        tx_count: 0,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let routing_plan = crate::queue::RoutingPlan::single(RoutingDecision::new(
        first_reservation.lane_id,
        first_reservation.dataspace_id,
    ));
    assert!(
        members
            .iter()
            .all(|(_, reservation)| routing_plan.digest() == reservation.routing_plan_digest)
    );
    let entrypoints = members
        .iter()
        .map(|(entrypoint, _)| entrypoint.clone())
        .collect::<Vec<_>>();
    let reservation_key_values = members
        .iter()
        .map(|(_, reservation)| *reservation)
        .collect::<Vec<_>>();
    let routing_plan_values = vec![routing_plan; members.len()];
    let native_amx_receipts = vec![None; members.len()];
    let mut validator_keypairs = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("derive reservation fixture validator key")
        })
        .collect::<Vec<_>>();
    validator_keypairs.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert!(validator_set.iter().all(|validator| {
        validator_keypairs
            .iter()
            .any(|keypair| keypair.public_key() == validator.public_key())
    }));
    let producer = deterministic_lane_author(&validator_set, first_reservation.lane_block_height)
        .cloned()
        .expect("reservation fixture has a deterministic producer");
    let producer_keypair = validator_keypairs
        .iter()
        .find(|keypair| keypair.public_key() == producer.public_key())
        .expect("reservation fixture retains the deterministic producer key");
    let network_id = context.network_id;
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        context.epoch,
        proposal.clone(),
        entrypoints.clone(),
        reservation_key_values,
        routing_plan_values,
        native_amx_receipts.clone(),
        producer,
        producer_keypair.private_key(),
    )
    .expect("construct authenticated reservation fixture payload");
    let validator_pops = validator_set
        .iter()
        .map(|validator| {
            let keypair = validator_keypairs
                .iter()
                .find(|keypair| keypair.public_key() == validator.public_key())
                .expect("reservation fixture retains every validator key");
            iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                .expect("reservation fixture validator PoP")
        })
        .collect::<Vec<_>>();
    let selected_keypairs = validator_set
        .iter()
        .take(usize::try_from(min_quorum).expect("fixture quorum fits usize"))
        .map(|validator| {
            validator_keypairs
                .iter()
                .find(|keypair| keypair.public_key() == validator.public_key())
                .expect("reservation fixture retains every selected validator key")
        })
        .collect::<Vec<_>>();
    let prepare_body = proposal.vote_body(CertPhase::Prepare);
    let availability_body = crate::lane_consensus::lane_payload_availability_body(
        &payload,
        &proposal,
        network_id,
        context.epoch,
    )
    .expect("reservation fixture availability body");
    let prepare_votes = selected_keypairs
        .iter()
        .map(|keypair| {
            let availability_vote =
                crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                    availability_body.clone(),
                    PeerId::new(keypair.public_key().clone()),
                    validator_pops.clone(),
                    keypair.private_key(),
                )
                .expect("reservation fixture READY vote");
            crate::lane_consensus::LaneBlockVoteV1 {
                body: prepare_body.clone(),
                signer: PeerId::new(keypair.public_key().clone()),
                bls_signature: Signature::try_new(
                    keypair.private_key(),
                    &prepare_body.signature_preimage(),
                )
                .expect("reservation fixture Prepare signature")
                .payload()
                .to_vec(),
                payload_availability_vote: Some(availability_vote),
            }
        })
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        prepare_body,
        validator_set.clone(),
        &prepare_votes,
    )
    .expect("reservation fixture PrepareQC");
    let commit_body = proposal.vote_body(CertPhase::Commit);
    let commit_votes = selected_keypairs
        .iter()
        .map(|keypair| crate::lane_consensus::LaneBlockVoteV1 {
            body: commit_body.clone(),
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: Signature::try_new(
                keypair.private_key(),
                &commit_body.signature_preimage(),
            )
            .expect("reservation fixture Commit signature")
            .payload()
            .to_vec(),
            payload_availability_vote: None,
        })
        .collect::<Vec<_>>();
    let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        commit_body,
        validator_set,
        &commit_votes,
    )
    .expect("reservation fixture CommitQC");
    let signer_pops = selected_keypairs
        .iter()
        .map(|keypair| {
            (
                keypair.public_key().clone(),
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("reservation fixture selected signer PoP"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let certified = crate::kura::CertifiedLaneBlockArtifact::new(
        crate::lane_consensus::CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: prepare_qc.clone(),
            commit_qc: commit_qc.clone(),
        },
        signer_pops,
    );
    let bundle = crate::kura::AutonomousLaneMergeBundleV1 {
        version: crate::kura::AutonomousLaneMergeBundleV1::VERSION,
        autonomous: crate::kura::AutonomousLaneBlockArtifact {
            format: crate::kura::AutonomousLaneBlockArtifactFormat::Current,
            executable_payload: payload.clone(),
            availability_certificate: Some(
                crate::lane_consensus::DurableLanePayloadAvailabilityCertificateV1 {
                    certificate: prepare_qc.clone(),
                },
            ),
            view_checkpoint: None,
            new_view_certificates: Vec::new(),
        },
        certified: certified.clone(),
    };
    crate::kura::Kura::validate_autonomous_lane_merge_bundle(&bundle, network_id, context.epoch)
        .expect("validate authenticated reservation fixture bundle");
    let source_bundle = bundle
        .encode_framed()
        .expect("encode authenticated reservation fixture bundle");
    let decoded_bundle =
        norito::decode_canonical::<crate::kura::AutonomousLaneMergeBundleV1>(&source_bundle)
            .expect("decode authenticated reservation fixture bundle");
    decoded_bundle
        .autonomous
        .executable_payload
        .validate(network_id, context.epoch)
        .expect("canonical reservation fixture payload must remain valid after decoding");
    let source_bundle_hash = bundle
        .bundle_hash()
        .expect("hash authenticated reservation fixture bundle");
    let reservation_keys = payload
        .reservation_keys
        .iter()
        .map(|reservation| {
            norito::encode_canonical(reservation)
                .expect("fixture reservation key has canonical framed Norito bytes")
        })
        .collect::<Vec<_>>();
    let routing_plans = payload
        .routing_plans
        .iter()
        .map(|routing_plan| {
            norito::encode_canonical(routing_plan)
                .expect("fixture routing plan has canonical framed Norito bytes")
        })
        .collect::<Vec<_>>();
    let execution = MergeLaneExecution {
        source_bundle,
        source_bundle_hash,
        proposal: proposal.clone(),
        origin_proposal: proposal.clone(),
        prepare_qc,
        commit_qc,
        signer_proofs: certified
            .signer_pops
            .iter()
            .map(|(public_key, proof_of_possession)| {
                iroha_data_model::merge::MergeLaneSignerProof {
                    public_key: public_key.clone(),
                    proof_of_possession: proof_of_possession.clone(),
                }
            })
            .collect(),
        autonomous_network_id: network_id,
        autonomous_epoch: context.epoch,
        autonomous_payload_hash: payload.payload_hash,
        entrypoint_hashes,
        entrypoints,
        reservation_keys,
        routing_plans,
        native_amx_receipts,
        result_hashes,
        results,
        settlement_hash: iroha_data_model::nexus::compute_settlement_hash(&settlement_commitment)
            .expect("fixture settlement hashes canonically"),
        settlement_commitment,
        fastpq_transcripts: Vec::new().into(),
    };
    let lanes = vec![execution];
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"v2 reservation fixture base state"));
    let write_set_root = Hash::new(b"v2 reservation fixture write set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 0,
        base_state_hash,
        application_block_header: application_block_header.clone(),
        entrypoint_count: u64::try_from(members.len()).expect("fixture count fits u64"),
        entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&lanes)
            .expect("fixture has one entrypoint"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&lanes)
            .expect("fixture has one result"),
        execution_root: crate::merge::merge_execution_root(&lanes),
        lanes,
        application_write_set_root: Hash::new(b"v2 reservation fixture application writes"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            0,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let mut entry = pending_merge_entry(context, 0, b"v2 reservation fixture merge entry");
    entry.epoch_id = 1;
    entry.merge_qc.epoch_id = 1;
    entry.merge_qc.carrier_height = application_block_header.height().get();
    entry.merge_qc.carrier_parent_hash = application_block_header
        .prev_block_hash()
        .expect("non-genesis merge carrier has a parent");
    entry.merge_qc.view = application_block_header.view_change_index();
    entry.execution_batch = Some(batch);
    (parent, entry)
}
fn reserve_transaction_for_test(
    state: &State,
    queue: &Queue,
    transaction: iroha_data_model::transaction::SignedTransaction,
) -> (
    crate::queue::LaneQueueReservationKeyV2,
    TransactionEntrypoint,
) {
    reserve_transaction_for_test_with_identity(
        state,
        queue,
        transaction,
        Hash::new(b"v2 reservation fixture owner"),
        Hash::new(b"v2 reservation fixture proposal"),
    )
}
fn reserve_transaction_for_test_with_identity(
    state: &State,
    queue: &Queue,
    transaction: iroha_data_model::transaction::SignedTransaction,
    reservation_owner_hash: Hash,
    proposal_identity_hash: Hash,
) -> (
    crate::queue::LaneQueueReservationKeyV2,
    TransactionEntrypoint,
) {
    reserve_transaction_for_lane_test_with_identity(
        state,
        queue,
        transaction,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        reservation_owner_hash,
        proposal_identity_hash,
    )
}
fn reserve_transaction_for_lane_test_with_identity(
    state: &State,
    queue: &Queue,
    transaction: iroha_data_model::transaction::SignedTransaction,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    reservation_owner_hash: Hash,
    proposal_identity_hash: Hash,
) -> (
    crate::queue::LaneQueueReservationKeyV2,
    TransactionEntrypoint,
) {
    // These fixtures exercise the strict global QueuePlan corridor, so their transaction must
    // carry the same signature-bound intent and network domain as a production submission. The
    // Apply fixture body itself is a genesis-domain Ordinary transaction and cannot be reused
    // byte-for-byte after that admission contract became mandatory.
    let mut payload = transaction.payload().clone();
    payload.domain =
        iroha_data_model::transaction::TransactionDomain::Network(*state.network_id_ref());
    payload.admission_intent =
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced;
    let transaction_key = KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::Ed25519)
        .expect("deterministic reservation fixture transaction key");
    assert_eq!(
        payload.authority,
        AccountId::new(transaction_key.public_key().clone()),
        "reservation fixture must be controlled by the deterministic transaction key"
    );
    let transaction = TransactionBuilder::from_payload(payload)
        .expect("rebuild signature-bound QueuePlan reservation fixture")
        .sign(transaction_key.private_key());
    let entrypoint = TransactionEntrypoint::External(transaction.clone());
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
    let expected_route = RoutingDecision::new(lane_id, dataspace_id);
    let routing_plan = queue
        .route_plan_with_state(&accepted, state)
        .expect("resolve reservation fixture route from committed policy");
    assert_eq!(
        routing_plan.coordinator_route(),
        expected_route,
        "reservation fixture must use the production-derived coordinator route"
    );
    let admission_context = queue
        .plan_admission_context_with_state(state, &routing_plan)
        .expect("capture reservation fixture admission context");
    let proposal_height = admission_context.proposal_height;
    let lane_incarnation = {
        let coordinator = admission_context
            .route_incarnations
            .first()
            .expect("reservation fixture has a coordinator incarnation");
        assert_eq!(coordinator.leg.route, expected_route);
        coordinator.lane_incarnation
    };
    let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
        state.network_id_ref(),
        accepted.entrypoint(),
        &routing_plan,
        admission_context,
        queue.queue_plan_admission_timestamp_ms(),
    )
    .expect("build reservation fixture global admission binding");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            accepted,
            state,
            routing_plan,
            &binding,
        )
        .expect("durably enqueue globally bound reservation fixture transaction");
    install_fixture_queue_plan_registry_value(state, &binding);
    let scope = LaneQueueReservationScopeV1 {
        lane_id,
        dataspace_id,
        lane_incarnation,
        proposal_height,
        lane_block_height: proposal_height,
        lane_block_view: 0,
        reservation_owner_hash,
        proposal_identity_hash,
    };
    let reserved = queue
        .reserve_transactions_for_lane(
            state,
            scope,
            NonZeroUsize::new(1).expect("non-zero reservation limit"),
        )
        .expect("reserve exact fixture transaction");
    assert_eq!(reserved.len(), 1);
    (*reserved[0].key(), entrypoint)
}
fn install_recreatable_reservation_lane(
    fixture: &ApplyFixture,
) -> iroha_data_model::nexus::LaneConfig {
    let state = fixture.state.as_ref();
    let lane = iroha_data_model::nexus::LaneConfig {
        id: LaneId::new(1),
        alias: "recreatable-reservation-lane".to_owned(),
        ..iroha_data_model::nexus::LaneConfig::default()
    };
    state
        .apply_lane_lifecycle(&iroha_data_model::nexus::LaneLifecyclePlan {
            additions: vec![lane.clone()],
            retire: Vec::new(),
        })
        .expect("install recreatable reservation lane");
    let validators = fixture
        .context
        .roster
        .iter()
        .map(|validator| AccountId::new(validator.validator.public_key().clone()))
        .collect::<Vec<_>>();
    let validator_bindings = validators
        .iter()
        .zip(&fixture.context.roster)
        .map(|(validator, power)| ManifestValidatorBinding {
            validator: validator.clone(),
            peer_id: power.validator.clone(),
            torii_url: None,
        })
        .collect();
    let status = LaneManifestStatus {
        lane: lane.id,
        alias: lane.alias.clone(),
        dataspace: lane.dataspace_id,
        visibility: lane.visibility,
        storage: lane.storage,
        governance: lane.governance.clone(),
        manifest_path: Some(std::path::PathBuf::from(
            "/tmp/sumeragi-v2-apply-recreatable-lane-manifest.json",
        )),
        governance_rules: Some(GovernanceRules {
            validators,
            validator_bindings,
            ..GovernanceRules::default()
        }),
        privacy_commitments: Vec::new(),
    };
    let mut statuses = state
        .lane_manifests
        .read()
        .statuses()
        .into_iter()
        .map(|status| (status.lane, status))
        .collect::<BTreeMap<_, _>>();
    statuses.insert(lane.id, status);
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
    state.nexus.write().routing_policy.rules.insert(
        0,
        iroha_config::parameters::actual::LaneRoutingRule {
            lane: lane.id,
            dataspace: Some(lane.dataspace_id),
            matcher: iroha_config::parameters::actual::LaneRoutingMatcher {
                account: Some(fixture.service.genesis_account.to_string()),
                ..Default::default()
            },
        },
    );
    let mut expected = fixture
        .context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect::<Vec<_>>();
    expected.sort();
    let mut actual = state
        .resolve_lane_committee_at_height(
            crate::state::LaneAuthorityRoute::new(lane.id, lane.dataspace_id),
            1,
        )
        .expect("recreatable reservation lane authority must resolve")
        .into_validators();
    actual.sort();
    assert_eq!(
        actual, expected,
        "recreatable reservation lane must have authenticated fixture authority"
    );
    lane
}
fn replace_recreatable_reservation_lane(
    state: &State,
    lane: &iroha_data_model::nexus::LaneConfig,
) -> (Hash, Hash) {
    let old_incarnation = state
        .lane_incarnation(lane.id)
        .expect("recreatable reservation lane has an incarnation");
    state
        .apply_lane_lifecycle(&iroha_data_model::nexus::LaneLifecyclePlan {
            additions: vec![lane.clone()],
            retire: vec![lane.id],
        })
        .expect("replace reservation lane with the same lane id");
    let new_incarnation = state
        .lane_incarnation(lane.id)
        .expect("replacement reservation lane has an incarnation");
    assert_ne!(
        new_incarnation, old_incarnation,
        "same-ID replacement must rotate the reservation lane incarnation"
    );
    (old_incarnation, new_incarnation)
}
fn install_fixture_queue_plan_registry_value(
    state: &State,
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV2,
) {
    state
        .install_queue_plan_pending_binding_for_test(binding)
        .expect("install complete reservation fixture QueuePlan owner state");
}
fn reserve_autonomous_crash_batch(
    fixture: &ApplyFixture,
    queue: &Arc<Queue>,
    producer: &KeyPair,
) -> (LaneExecutablePayloadV1, Vec<HashOf<TransactionEntrypoint>>) {
    let transactions = (0_u8..4)
        .map(|index| {
            TransactionBuilder::new(
                fixture.context.network_id,
                fixture.service.genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("autonomous reservation crash boundary {index}"),
            )])
            .sign(fixture.genesis_key.private_key())
        })
        .collect::<Vec<_>>();
    let expected_fifo = transactions
        .iter()
        .map(|transaction| transaction.hash_as_entrypoint())
        .collect::<Vec<_>>();
    let entrypoints = transactions
        .iter()
        .take(3)
        .cloned()
        .map(TransactionEntrypoint::External)
        .collect::<Vec<_>>();
    let entrypoint_hashes = entrypoints
        .iter()
        .map(|entrypoint| Hash::from(entrypoint.hash()))
        .collect::<Vec<_>>();
    let lane_incarnation = fixture
        .state
        .lane_incarnation_at_height(LaneId::SINGLE, 1)
        .expect("default lane incarnation at autonomous proposal height");
    let validator_set = vec![PeerId::new(producer.public_key().clone())];
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_incarnation,
        proposal_height: 1,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::new(b"v2 autonomous crash reservation subject"),
        payload_ownership_hash: Hash::new(b"v2 autonomous crash reservation ownership"),
        rbc_instance_hash: Hash::new(b"v2 autonomous crash reservation RBC"),
        accepted_candidate_indices: (0_u64..3).collect(),
        accepted_transaction_hashes: entrypoint_hashes,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count: 1,
        min_quorum: 1,
        qc_mode_tag: "permissioned:v2-autonomous-reservation-crash".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    for transaction in &transactions {
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
        let routing_plan = queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve autonomous crash reservation route");
        let admission_context = queue
            .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
            .expect("capture autonomous crash admission context");
        let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
            fixture.state.network_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("build autonomous crash global admission binding");
        queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                accepted,
                fixture.state.as_ref(),
                routing_plan,
                &binding,
            )
            .expect("durably enqueue autonomous crash reservation transaction");
        install_fixture_queue_plan_registry_value(fixture.state.as_ref(), &binding);
    }
    let scope = LaneQueueReservationScopeV1 {
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"v2 autonomous crash reservation owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let reserved = queue
        .reserve_transactions_for_lane(
            fixture.state.as_ref(),
            scope,
            NonZeroUsize::new(3).expect("non-zero crash reservation count"),
        )
        .expect("reserve exact autonomous crash batch");
    assert_eq!(reserved.len(), 3);
    assert_eq!(
        reserved
            .iter()
            .map(|reserved| reserved.key().entrypoint_hash)
            .collect::<Vec<_>>(),
        expected_fifo[..3],
        "fixture must reserve the original FIFO prefix"
    );
    let reservation_keys = reserved
        .iter()
        .map(|reserved| *reserved.key())
        .collect::<Vec<_>>();
    let routing_plans = reserved
        .iter()
        .map(|reserved| reserved.routing_plan().clone())
        .collect::<Vec<_>>();
    let network_id = fixture.context.network_id;
    let epoch = {
        let world = fixture.state.world_view();
        crate::sumeragi::epoch_for_height_from_world(&world, proposal.descriptor.proposal_height)
    };
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal,
        entrypoints,
        reservation_keys,
        routing_plans,
        vec![None; 3],
        validator_set[0].clone(),
        producer.private_key(),
    )
    .expect("build exact autonomous crash payload");
    (payload, expected_fifo)
}
fn fixture_validator_keys() -> Vec<KeyPair> {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic fixture BLS key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}
fn verified_context_for_fixture(
    fixture: &ApplyFixture,
    context: &wire::HeightContext,
) -> super::super::v2::VerifiedHeightContext {
    let proofs = fixture_validator_keys()
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("derive fixture validator proof of possession")
        })
        .collect::<Vec<_>>();
    if context.height == 1 {
        return super::super::v2::VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verify fixture genesis context");
    }
    let (parent_artifact, parent_receipt) = fixture
        .kura
        .v2_finality_artifact_with_receipt(context.height - 1)
        .expect("read fixture parent finality")
        .expect("fixture successor context has parent finality");
    super::super::v2::VerifiedHeightContext::successor(
        context.clone(),
        proofs.clone(),
        &parent_artifact,
        &parent_receipt,
        &proofs,
    )
    .expect("verify fixture successor context")
}
fn commit_exact_fixture_block_metadata(state: &State, block: &SignedBlock) {
    let height = NonZeroUsize::new(
        usize::try_from(block.header().height().get()).expect("fixture block height fits usize"),
    )
    .expect("fixture block height is non-zero");
    assert_eq!(
        height.get(),
        state.committed_height().saturating_add(1),
        "fixture State metadata must advance contiguously"
    );
    assert_eq!(
        block.header().prev_block_hash(),
        state.committed_block_hashes_snapshot().last().copied(),
        "fixture State metadata must follow the exact committed predecessor"
    );
    assert_eq!(
        state.durable_block_hash(height),
        Some(block.hash()),
        "fixture State metadata must name the exact durable Kura block"
    );
    let mut state_block = state.block(block.header());
    state_block.block_hashes.push_for_tests(block.hash());
    state_block
        .transactions
        .insert_block(std::collections::HashSet::new(), height);
    state_block
        .commit()
        .expect("commit exact fixture block metadata to State");
}
fn commit_exact_fixture_carrier_chain_to_state(
    fixture: &ApplyFixture,
    parent: &SignedBlock,
    carrier: &SignedBlock,
) {
    assert_eq!(
        fixture.state.committed_height(),
        0,
        "fixture carrier chain starts from empty State history"
    );
    assert_eq!(parent.header().height().get(), 1);
    assert_eq!(carrier.header().height().get(), 2);
    assert_eq!(carrier.header().prev_block_hash(), Some(parent.hash()));
    commit_exact_fixture_block_metadata(fixture.state.as_ref(), parent);
    commit_exact_fixture_block_metadata(fixture.state.as_ref(), carrier);
    assert_eq!(fixture.state.committed_height(), 2);
    assert_eq!(fixture.state.latest_block_hash_fast(), Some(carrier.hash()));
}
fn verified_successor_context_after_fixture_tip(
    fixture: &ApplyFixture,
) -> super::super::v2::VerifiedHeightContext {
    assert_eq!(fixture.state.committed_height(), 2);
    let parent_artifact = fixture
        .kura
        .v2_finality_artifact(2)
        .expect("read fixture carrier finality")
        .expect("fixture carrier has finality");
    let state_view = fixture.state.view();
    let context = crate::sumeragi::v2_context::build_successor_height_context_from_state(
        &parent_artifact,
        &state_view,
        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(fixture.state.as_ref()),
    )
    .expect("derive fixture context after the exact canonical carrier");
    drop(state_view);
    assert_eq!(context.height, 3);
    verified_context_for_fixture(fixture, &context)
}
fn reserve_canonical_successor_autonomous_batch(
    fixture: &ApplyFixture,
    queue: &Arc<Queue>,
    context: &wire::HeightContext,
    count: usize,
) -> (LaneExecutablePayloadV1, Vec<HashOf<TransactionEntrypoint>>) {
    reserve_canonical_successor_autonomous_batch_with_instructions(
        fixture,
        queue,
        context,
        count,
        |index| {
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                format!("canonical successor autonomous reservation {index}"),
            ))]
        },
        false,
        None,
    )
}
fn reserve_canonical_successor_autonomous_batch_with_instructions(
    fixture: &ApplyFixture,
    queue: &Arc<Queue>,
    context: &wire::HeightContext,
    count: usize,
    instructions: impl Fn(usize) -> Vec<InstructionBox>,
    sort_by_signed_transaction_hash: bool,
    native_receipt_builder: Option<ApplyNativeReceiptBuilder>,
) -> (LaneExecutablePayloadV1, Vec<HashOf<TransactionEntrypoint>>) {
    assert_eq!(fixture.state.committed_height(), 1);
    assert_eq!(context.height, 2);
    assert!((1..=16).contains(&count));
    install_fixture_validator_authority(
        fixture.state.as_ref(),
        context,
        &fixture.service.validator_set_pops,
    );
    let mut transactions = (0..count)
        .map(|index| {
            let mut builder = TransactionBuilder::new(
                context.network_id,
                fixture.service.genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(instructions(index));
            let nonce = u32::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .and_then(NonZeroU32::new)
                .expect("bounded autonomous fixture index yields a unique nonce");
            builder.set_nonce(nonce);
            builder.sign(fixture.genesis_key.private_key())
        })
        .collect::<Vec<_>>();
    if sort_by_signed_transaction_hash {
        transactions.sort_by_key(|transaction| transaction.hash());
    }
    let signed_transaction_hashes = transactions
        .iter()
        .map(|transaction| transaction.hash())
        .collect::<Vec<_>>();
    assert_eq!(
        signed_transaction_hashes
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        count,
        "each autonomous fixture transaction must have one unique signed identity"
    );
    let entrypoints = transactions
        .iter()
        .cloned()
        .map(TransactionEntrypoint::External)
        .collect::<Vec<_>>();
    let expected_fifo = entrypoints
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect::<Vec<_>>();
    let mut planned_routing = Vec::with_capacity(count);
    for transaction in &transactions {
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
        let routing_plan = queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve canonical autonomous routing plan");
        let admission_context = queue
            .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
            .expect("capture canonical autonomous admission context");
        let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
            fixture.state.network_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("build canonical autonomous global admission binding");
        queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                accepted,
                fixture.state.as_ref(),
                routing_plan.clone(),
                &binding,
            )
            .expect("durably enqueue canonical autonomous transaction");
        install_fixture_queue_plan_registry_value(fixture.state.as_ref(), &binding);
        planned_routing.push(routing_plan);
    }
    let coordinator_routes = planned_routing
        .iter()
        .map(crate::queue::RoutingPlan::coordinator_route)
        .collect::<Vec<_>>();
    let coordinator_route = coordinator_routes
        .first()
        .expect("canonical autonomous batch has a coordinator route");
    assert!(
        coordinator_routes
            .iter()
            .all(|route| route == coordinator_route),
        "canonical autonomous fixture must target one reservation slot"
    );
    let reservation_slot = super::super::lane_planner::plan_autonomous_lane_reservation_slot(
        fixture.state.as_ref(),
        fixture.kura.as_ref(),
        context,
        coordinator_route.lane_id,
        coordinator_route.dataspace_id,
    )
    .expect("derive deterministic canonical autonomous reservation slot");
    let producer = reservation_slot.author.clone();
    let entrypoint_hashes = entrypoints
        .iter()
        .map(|entrypoint| Hash::from(entrypoint.hash()))
        .collect::<Vec<_>>();
    let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
        fixture.state.as_ref(),
        fixture.kura.as_ref(),
        context,
        0,
        &producer,
        &coordinator_routes,
        &entrypoint_hashes,
    )
    .expect("derive canonical successor autonomous proposal");
    assert!(lane_plan.unavailable_indices.is_empty());
    assert_eq!(lane_plan.proposals.len(), 1);
    let proposal = lane_plan.proposals[0].clone();
    let network_id = context.network_id;
    let (reservation_owner_hash, proposal_identity_hash) =
        super::super::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            context.id(),
            context.epoch,
            &proposal,
            &producer,
        )
        .expect("derive canonical successor reservation identity");
    assert_eq!(
        (reservation_owner_hash, proposal_identity_hash),
        (
            reservation_slot.reservation_owner_hash,
            reservation_slot.proposal_identity_hash,
        ),
        "proposal and pre-selection slot must bind identical queue ownership",
    );
    let scope = reservation_slot.reservation_scope();
    let reserved = queue
        .reserve_transactions_for_lane(
            fixture.state.as_ref(),
            scope,
            NonZeroUsize::new(count).expect("non-zero canonical reservation count"),
        )
        .expect("reserve canonical successor autonomous batch");
    assert_eq!(reserved.len(), count);
    assert_eq!(
        reserved
            .iter()
            .map(|reservation| reservation.key().entrypoint_hash)
            .collect::<Vec<_>>(),
        expected_fifo,
        "canonical autonomous reservation must preserve FIFO selection order"
    );
    let reservation_keys = reserved
        .iter()
        .map(|reservation| *reservation.key())
        .collect::<Vec<_>>();
    let routing_plans = reserved
        .iter()
        .map(|reservation| reservation.routing_plan().clone())
        .collect::<Vec<_>>();
    assert_eq!(routing_plans, planned_routing);
    let validator_keys = fixture_validator_keys();
    let producer_key = validator_keys
        .iter()
        .find(|key| key.public_key() == producer.public_key())
        .expect("fixture contains canonical autonomous producer key");
    let native_amx_receipts = match native_receipt_builder {
        Some(builder) => builder(
            fixture,
            context,
            network_id,
            &proposal,
            &entrypoints,
            &reservation_keys,
            &routing_plans,
        ),
        None => vec![None; count],
    };
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        context.epoch,
        proposal,
        entrypoints,
        reservation_keys,
        routing_plans,
        native_amx_receipts,
        producer,
        producer_key.private_key(),
    )
    .expect("build canonical successor autonomous payload");
    (payload, expected_fifo)
}
fn certify_autonomous_payload_for_test(fixture: &ApplyFixture, payload: &LaneExecutablePayloadV1) {
    let validator_keys = fixture_validator_keys();
    let signed_vote = |phase, key_pair: &KeyPair| {
        let body = payload.origin_proposal.vote_body(phase);
        let signature = Signature::try_new(key_pair.private_key(), &body.signature_preimage())
            .expect("sign autonomous fixture lane vote");
        crate::lane_consensus::LaneBlockVoteV1 {
            body,
            payload_availability_vote: None,
            signer: PeerId::new(key_pair.public_key().clone()),
            bls_signature: signature.payload().to_vec(),
        }
    };
    let qc = |phase| {
        let votes = validator_keys
            .iter()
            .take(3)
            .map(|key| signed_vote(phase, key))
            .collect::<Vec<_>>();
        crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            payload.origin_proposal.vote_body(phase),
            payload.origin_proposal.descriptor.validator_set.clone(),
            &votes,
        )
        .expect("autonomous fixture votes form a quorum certificate")
    };
    let session = crate::lane_consensus::CommittedLaneBlockSession {
        proposal: payload.origin_proposal.clone(),
        prepare_qc: qc(CertPhase::Prepare),
        commit_qc: qc(CertPhase::Commit),
    };
    let signer_pops = validator_keys
        .iter()
        .take(3)
        .map(|key| {
            (
                key.public_key().clone(),
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    fixture
        .kura
        .persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist exact autonomous certification");
}
fn body_with_merge_reference(reference: CertifiedMergeLedgerReference) -> SignedBlock {
    let key = KeyPair::try_from_seed(vec![0xC9; 32], Algorithm::BlsNormal)
        .expect("derive decided-body signer");
    let execution_context =
        BlockExecutionContextBundle::new(Vec::new()).with_merge_entry(reference);
    let block = BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
        .chain(0, None)
        .with_execution_context(Some(execution_context))
        .try_sign_with_index(key.private_key(), 0)
        .expect("sign decided body")
        .unpack(|_| {});
    SignedBlock::from(block)
}
fn body_with_exact_merge_execution_header(entry: &MergeLedgerEntry) -> SignedBlock {
    let key = KeyPair::try_from_seed(vec![0xCA; 32], Algorithm::BlsNormal)
        .expect("derive execution-carrier signer");
    let header = entry
        .execution_batch
        .as_ref()
        .expect("execution merge entry")
        .application_block_header
        .clone();
    assert_eq!(
        entry.merge_qc.carrier_height,
        header.height().get(),
        "execution fixture QC must bind the carrier height"
    );
    assert_eq!(
        Some(entry.merge_qc.carrier_parent_hash),
        header.prev_block_hash(),
        "execution fixture QC must bind the carrier parent"
    );
    assert_eq!(
        entry.merge_qc.view,
        header.view_change_index(),
        "execution fixture QC must bind the carrier view"
    );
    let execution_context = BlockExecutionContextBundle::new(Vec::new())
        .with_merge_entry(CertifiedMergeLedgerReference::new(entry));
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    builder.set_execution_context(Some(execution_context));
    let carrier = builder.build_with_signature(0, key.private_key());
    assert_eq!(
        crate::merge::merge_application_header_from_carrier(&carrier.header()),
        entry
            .execution_batch
            .as_ref()
            .expect("execution merge entry")
            .application_block_header,
        "signed carrier must preserve the certified application header"
    );
    carrier
}
struct DeferredCanonicalCarrierStartupFixture {
    fixture: ApplyFixture,
    queue: Arc<Queue>,
    plan: LaneReservationReconciliationPlan,
    expected_groups: Vec<crate::kura::AutonomousLifecyclePendingReservationGroupObservation>,
    outcome_paths: Vec<std::path::PathBuf>,
    _queue_root: tempfile::TempDir,
}
fn deferred_canonical_carrier_startup_fixture() -> DeferredCanonicalCarrierStartupFixture {
    let fixture = ApplyFixture::new_with_lane_lifecycle();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue_root = tempfile::tempdir().expect("deferred carrier Queue journal directory");
    let plan_path = queue_root.path().join("queue-plans.norito");
    let reservation_path = queue_root.path().join("lane-reservations.norito");
    let queue = Arc::new(Queue::from_config(
        QueueConfig::default(),
        events_sender.clone(),
    ));
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install deferred carrier QueuePlan journal");
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("install deferred carrier reservation journal");
    let first_transaction = fixture
        .body
        .external_transactions()
        .next()
        .expect("fixture transaction")
        .clone();
    let (first_key, first_entrypoint) = reserve_transaction_for_test_with_identity(
        fixture.state.as_ref(),
        queue.as_ref(),
        first_transaction,
        Hash::new(b"deferred carrier owned group"),
        Hash::new(b"deferred carrier owned proposal"),
    );
    let second_lane = install_recreatable_reservation_lane(&fixture);
    let (absent_events, _absent_receiver) = tokio::sync::broadcast::channel(8);
    let absent_root = tempfile::tempdir().expect("absent sibling Queue journal directory");
    let absent_queue = Queue::from_config(QueueConfig::default(), absent_events);
    absent_queue
        .install_plan_journal(
            absent_root.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install absent sibling QueuePlan journal");
    absent_queue
        .install_lane_reservation_journal(
            absent_root.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("install absent sibling reservation journal");
    let second_transaction = TransactionBuilder::new(
        fixture.context.network_id,
        fixture.service.genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "deferred carrier absent sibling".to_owned(),
    )])
    .sign(fixture.genesis_key.private_key());
    let (second_key, second_entrypoint) = reserve_transaction_for_lane_test_with_identity(
        fixture.state.as_ref(),
        &absent_queue,
        second_transaction,
        second_lane.id,
        second_lane.dataspace_id,
        Hash::new(b"deferred carrier absent group"),
        Hash::new(b"deferred carrier absent proposal"),
    );
    let (parent, mut entry) =
        merge_entry_with_reservation(&fixture.context, first_entrypoint, first_key);
    let (second_parent, mut second_entry) =
        merge_entry_with_reservation(&fixture.context, second_entrypoint, second_key);
    assert_eq!(second_parent.hash(), parent.hash());
    let mut second_batch = second_entry
        .execution_batch
        .take()
        .expect("absent sibling execution batch");
    let batch = entry
        .execution_batch
        .as_mut()
        .expect("owned execution batch");
    assert_eq!(
        batch.application_block_header,
        second_batch.application_block_header,
    );
    batch.lanes.append(&mut second_batch.lanes);
    batch.entrypoint_count = batch
        .lanes
        .iter()
        .try_fold(0_u64, |count, lane| {
            count.checked_add(u64::try_from(lane.entrypoints.len()).ok()?)
        })
        .expect("deferred carrier entrypoint count fits u64");
    batch.entrypoint_merkle_root =
        crate::merge::merge_execution_entrypoint_merkle_root(&batch.lanes)
            .expect("deferred carrier entrypoint root");
    batch.result_merkle_root = crate::merge::merge_execution_result_merkle_root(&batch.lanes)
        .expect("deferred carrier result root");
    batch.execution_root = crate::merge::merge_execution_root(&batch.lanes);
    batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
    let payloads = batch
        .lanes
        .iter()
        .map(|execution| {
            Kura::decode_autonomous_lane_merge_bundle(
                &execution.source_bundle,
                execution.autonomous_network_id,
                execution.autonomous_epoch,
            )
            .expect("decode deferred carrier autonomous source")
            .autonomous
            .executable_payload
        })
        .collect::<Vec<_>>();
    assert_eq!(payloads.len(), 2);
    assert_eq!(payloads[0].reservation_keys.as_slice(), &[first_key]);
    assert_eq!(payloads[1].reservation_keys.as_slice(), &[second_key]);
    let local_signer = fixture.validator_keys[0].clone();
    let local_peer = PeerId::new(local_signer.public_key().clone());
    fixture
        .kura
        .bind_local_peer_id(local_peer.clone())
        .expect("bind deferred carrier local peer");
    let network_id = fixture.context.network_id;
    let generation = fixture
        .kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim deferred carrier process generation");
    let runtime_lanes =
        RuntimeLaneConfig::from_catalog(&fixture.state.nexus_snapshot().lane_catalog);
    let mut groups = Vec::new();
    let mut outcome_paths = Vec::new();
    for payload in &payloads {
        let descriptor = &payload.origin_proposal.descriptor;
        fixture
            .kura
            .install_lane_incarnation_marker_for_test(
                runtime_lanes
                    .entry(descriptor.lane_id)
                    .expect("deferred carrier runtime lane"),
                descriptor.lane_incarnation,
                0,
            )
            .expect("install deferred carrier lane marker");
        fixture
            .kura
            .persist_lane_executable_payload(payload, payload.network_id, payload.epoch)
            .expect("persist deferred carrier executable payload");
        groups.push(install_live_lifecycle_cursor_for_apply_test(
            fixture.kura.as_ref(),
            &generation,
            payload,
            fixture.context.id(),
            &local_peer,
            &local_signer,
        ));
        outcome_paths.push(
            fixture
                .kura
                .autonomous_lifecycle_terminal_outcome_path_for_test(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    descriptor.proposal_height,
                )
                .expect("resolve deferred carrier terminal outcome path"),
        );
    }
    let carrier = body_with_exact_merge_execution_header(&entry);
    fixture
        .kura
        .store_block(Arc::new(parent.clone()))
        .expect("persist deferred carrier parent");
    fixture
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist deferred carrier and merge entry");
    fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
    fixture
        .kura
        .persist_merge_lane_block_application_receipts(&entry, 2, carrier.hash())
        .expect("persist deferred carrier application receipts");
    commit_exact_fixture_carrier_chain_to_state(&fixture, &parent, &carrier);
    fixture.state.record_direct_committed_entrypoints(
        [first_key.entrypoint_hash, second_key.entrypoint_hash],
        NonZeroUsize::new(2).expect("deferred carrier State height"),
    );
    drop(absent_queue);
    drop(queue);
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("replay only the owned deferred carrier group");
    assert_eq!(replay.restored, 1);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install replayed deferred carrier QueuePlan journal");
    queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay deferred carrier QueuePlan owner");
    assert!(queue.lane_reservation_startup_reconciliation_pending());
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture deferred carrier startup snapshot")
            .ordered_groups
            .len(),
        1,
        "only carrier group A is locally Queue-owned",
    );
    let _publication = fixture
        .kura
        .reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group(&groups[0])
        .expect("materialize complete A+B Pending carrier publication");
    assert!(outcome_paths.iter().all(|path| path.is_file()));
    let recoveries = fixture
        .kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory deferred A+B carrier");
    assert_eq!(recoveries.len(), 1);
    let expected_groups = recoveries[0]
        .pending_reservation_groups()
        .expect("deferred carrier exposes both Pending groups");
    assert_eq!(expected_groups.len(), 2);
    let verified_context = verified_successor_context_after_fixture_tip(&fixture);
    let active_context = verified_context.context().clone();
    let terminal = crate::sumeragi::v2_lifecycle_recovery::reconcile_pending_autonomous_lifecycle_terminal_outcomes(
        fixture.state.as_ref(),
        queue.as_ref(),
        fixture.kura.as_ref(),
        &active_context,
    )
    .expect("defer whole A+B carrier before Queue planning");
    assert_eq!(terminal.completed_outcomes(), 0);
    assert_eq!(terminal.deferred_pending_groups(), 2);
    let deferred = terminal.into_deferred_terminal_recovery();
    let initial = plan_lane_reservation_ownership(
        fixture.state.as_ref(),
        queue.as_ref(),
        fixture.kura.as_ref(),
        &verified_context,
        None,
    )
    .expect("plan the sole Queue-owned carrier anchor");
    let LaneReservationReconciliationPlanning::Ready(initial) = initial else {
        panic!("deferred carrier anchor must be immediately plannable");
    };
    let planner_evidence = initial
        .startup_snapshot_recovery_evidence()
        .expect("extract exact deferred carrier planner evidence");
    let lifecycle = crate::sumeragi::v2_lifecycle_recovery::reconcile_autonomous_lifecycle_startup(
        fixture.state.as_ref(),
        queue.as_ref(),
        fixture.kura.as_ref(),
        &active_context,
        planner_evidence,
        deferred,
        Some(&generation),
        &local_peer,
        &local_signer,
    )
    .expect("pair only Queue-owned A without mutating absent deferred B");
    assert_eq!(lifecycle.recovered_attempts(), 0);
    let replanned = plan_lane_reservation_ownership(
        fixture.state.as_ref(),
        queue.as_ref(),
        fixture.kura.as_ref(),
        &verified_context,
        Some(lifecycle),
    )
    .expect("replan with deferred A+B lifecycle handoff");
    let LaneReservationReconciliationPlanning::Ready(plan) = replanned else {
        panic!("paired deferred carrier plan must be ready for Queue application");
    };
    DeferredCanonicalCarrierStartupFixture {
        fixture,
        queue,
        plan,
        expected_groups,
        outcome_paths,
        _queue_root: queue_root,
    }
}
