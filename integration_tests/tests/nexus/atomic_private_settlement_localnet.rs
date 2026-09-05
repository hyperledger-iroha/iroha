#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Release-only real-process tests for atomic private settlement.
//!
//! The ignored test deliberately uses the production wallet prover, encrypted
//! auditor capsule, Torii restricted-DA routes, and node-held BLS committee
//! keys.  There is no fixture proof, hand-made vote, or QC verification bypass.
//! The primary N=3 topology settles one public participant dataspace and two
//! restricted participant dataspaces in the same confidential atomic bundle.
//! The included release-harness entrypoint parameterizes the same production
//! workflow across N=2,3,4,8,16 and publishes only measured process evidence.

use super::localnet_npos::npos_override_instruction;
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{
        BorrowedKeyPairIdentityRequestSignerV1, Client, PrivateSettlementAuditApprovalRequestV1,
        PrivateSettlementAuditorCapsuleRequestV1, PrivateSettlementBundleReceiptResponseV1,
        PrivateSettlementBundleSubmitRequestV1, PrivateSettlementLegUploadRequestV1,
        PrivateSettlementLifecycleDtoV1,
    },
    data_model::{
        Level,
        account::{Account, AccountId},
        asset::{
            AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId, AssetId,
        },
        block::{
            BlockHeader,
            consensus::{NativeAmxReceipt, SumeragiDiagnosticsStatus},
        },
        domain::{Domain, DomainId},
        isi::{
            Grant, GrantBox, InstructionBox, Log, Mint, Register,
            privacy::RegisterPrivacyProtocolActivationV1,
            private_settlement::{
                ActivatePrivateSettlementPoolV1, FinalizeAtomicPrivateSettlementV1,
            },
            register::RegisterCommitteePeerWithPop,
            settlement::{
                DvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
                SettlementPlan,
            },
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::{
            ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1, DataSpaceId, LaneId,
            LaneVisibility, PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1, PrivateSettlementAuditAadV1,
            PrivateSettlementAuditEncryptionOpeningV1, PrivateSettlementAuditNoteOpeningV1,
            PrivateSettlementAuditOutputRoleV1, PrivateSettlementAuditOutputV1,
            PrivateSettlementAuditPayerAuthorizationBodyV1,
            PrivateSettlementAuditPayerAuthorizationV1, PrivateSettlementAuditPayerInputV1,
            PrivateSettlementAuditPayerSignatureV1, PrivateSettlementAuditPlaintextV1,
            PrivateSettlementAuditPolicyBodyV1, PrivateSettlementAuditPolicyV1,
            PrivateSettlementAuditViewKeyAuthorizationBodyV1,
            PrivateSettlementAuditViewKeyAuthorizationV1, PrivateSettlementAuditViewKeySignatureV1,
            PrivateSettlementAuditorV1, PrivateSettlementCapsulePaddingV1,
            PrivateSettlementCommitBundleV1, PrivateSettlementCommitteeAuthorityV1,
            PrivateSettlementDeltaV1, PrivateSettlementHybridPublicKeyV1,
            PrivateSettlementLegCommitmentV1, PrivateSettlementLegReceiptV1,
            PrivateSettlementPoolGovernanceLifecycleV1, PrivateSettlementPoolGovernanceV1,
            PrivateSettlementProofProfileV1, PrivateSettlementProofStatementV1,
            PrivateSettlementProvisionalLegMaterialV1, PrivateSettlementRouteV1,
        },
        peer::PeerId,
        permission::Permission,
        prelude::{FindAssetById, FindAssets, FindPermissionsByAccountId},
        privacy::{
            PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1, PrivacyCommitmentV1,
            PrivacyCompiledProfileResultV1, PrivacyEncryptedOutputV1, PrivacyEncryptionKeyV1,
            PrivacyNullifierV1, PrivacyPoolIdV1, PrivacyProposedLifecycleV1,
            PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
            PrivacyRecipientIdV1, PrivacyRootV1,
        },
        query::block::prelude::FindBlocks,
        transaction::{
            FeeChargeKind, FeeChargeLimit, FeePaymentIntent, SignedTransaction,
            TransactionEntrypoint,
        },
    },
};
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_engines::{
        atomic_private_settlement::{
            AtomicPrivateSettlementPreparedLegV1, AtomicPrivateSettlementProvisionalLegInputV1,
            complete_atomic_private_settlement_prepared_leg_v1,
            consume_atomic_private_settlement_wallet_bundle_v1,
            derive_atomic_private_settlement_input_nullifiers_v1,
            encode_atomic_private_settlement_wallet_bundle_v1,
            finalize_atomic_private_settlement_provisional_bundle_v1,
            plan_atomic_private_settlement_bootstrap_v1,
            prepare_atomic_private_settlement_input_openings_v1,
            prepare_atomic_private_settlement_outputs_v1,
        },
        ivm_private_note::{
            derive_ivm_private_recipient_id_v1, derive_note_authority_v1,
            ivm_private_recipient_public_key_v1,
        },
    },
    privacy_profiles::compiled_privacy_profile_v1,
    private_settlement::{
        PrivateSettlementAuditEvaluationV1, PrivateSettlementAuditorSidecarViewV1,
        PrivateSettlementSidecarLifecycleV1, approve_private_settlement_leg_v1,
        seal_private_settlement_audit_capsule_v1_with_rng,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, HybridKeyPair, KeyPair, SignatureOf};
use iroha_data_model::prelude::QueryBuilderExt;
use iroha_executor_data_model::permission::{
    governance::CanEnactGovernance, settlement::CanExecuteSettlement,
};
use iroha_genesis::GenesisTopologyEntry;
use iroha_primitives::numeric::Quantity;
use iroha_test_network::{
    CommitteeValidatorP2pBootstrap, Network, NetworkBuilder, NetworkPeer,
    unexecuted_genesis_factory_with_post_topology,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use reqwest::Url;
use std::{
    ops::Range,
    thread,
    time::{Duration, Instant},
};
use toml::{Table, Value as TomlValue};

const PARTICIPANT_COUNT: usize = 3;
const PRIMARY_PUBLIC_PARTICIPANT_ORDINAL: usize = 0;
const VALIDATORS_PER_LANE: usize = 4;
const REAL_PROCESS_VALIDATOR_WORKER_THREADS: u64 = 4;
const GLOBAL_LANE_ID: u32 = 0;
const VALIDATOR_STAKE: u64 = 2_000;
const PRIVACY_GENESIS_PROPOSAL_HEIGHT: u64 = 1;
const PRIVACY_PROFILE_ACTIVATION_HEIGHT: u64 =
    PRIVACY_GENESIS_PROPOSAL_HEIGHT + PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1;
const PRIVATE_SETTLEMENT_MINIMUM_ACTIVATION_NOTICE_BLOCKS: u64 = 1;
const PRIVATE_SETTLEMENT_NOTICE_ACTIVATION_HEIGHT: u64 =
    PRIVACY_GENESIS_PROPOSAL_HEIGHT + PRIVATE_SETTLEMENT_MINIMUM_ACTIVATION_NOTICE_BLOCKS;
const PRIVATE_SETTLEMENT_ACTIVATION_HEIGHT: u64 =
    if PRIVACY_PROFILE_ACTIVATION_HEIGHT > PRIVATE_SETTLEMENT_NOTICE_ACTIVATION_HEIGHT {
        PRIVACY_PROFILE_ACTIVATION_HEIGHT
    } else {
        PRIVATE_SETTLEMENT_NOTICE_ACTIVATION_HEIGHT
    };
const MAX_EXPIRY_BLOCKS: u64 = 4_096;
const SIDECAR_RETENTION_BLOCKS: u64 = 4_096;
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;
const NEXUS_FEE_SEED_BALANCE: u64 = 10_000;
const NEXUS_FEE_SIGNED_MAXIMUM: u64 = 1;
const NEXUS_FEE_PER_PRIVATE_SETTLEMENT_CARRIER: &str = "0.001";
const TRANSPARENT_CONTROL_SEED_BALANCE: u64 = 10_000;
const TRANSPARENT_CONTROL_OUTPUT_BASELINE: u64 = 1;
const TEST_STACK_BYTES: usize = 64 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const FINALITY_TIMEOUT: Duration = Duration::from_secs(300);
const PRIVATE_SETTLEMENT_LEG_PRIVATE_MATERIAL_DOMAIN_V1: &[u8] =
    b"iroha:atomic-private-settlement:release-leg-private-material:v1\0";
const PRIVATE_SETTLEMENT_REIMBURSEMENT_SALT_DOMAIN_V1: &[u8] =
    b"iroha:atomic-private-settlement:release-reimbursement-salt:v1\0";

fn zero_hash() -> Hash {
    Hash::prehashed([0; Hash::LENGTH])
}

fn approve_all_audit_material(_: PrivateSettlementAuditEvaluationV1<'_>) -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TopologyShape {
    participants: usize,
}

impl TopologyShape {
    const fn new(participants: usize) -> Self {
        Self { participants }
    }

    const fn lane_count(self) -> usize {
        self.participants + 1
    }

    const fn global_validator_count(self) -> usize {
        VALIDATORS_PER_LANE
    }

    const fn participant_validator_count(self) -> usize {
        self.participants * VALIDATORS_PER_LANE
    }

    const fn process_count(self) -> usize {
        self.global_validator_count() + self.participant_validator_count()
    }

    fn committee_range(self, lane: usize) -> Range<usize> {
        assert!(lane < self.lane_count(), "committee lane is in range");
        let start = lane * VALIDATORS_PER_LANE;
        start..start + VALIDATORS_PER_LANE
    }

    fn participant_visibility(self, ordinal: usize) -> LaneVisibility {
        assert!(
            ordinal < self.participants,
            "participant ordinal is in range"
        );
        if ordinal == PRIMARY_PUBLIC_PARTICIPANT_ORDINAL {
            LaneVisibility::Public
        } else {
            LaneVisibility::Restricted
        }
    }

    fn participant_visibility_profile(self) -> Vec<LaneVisibility> {
        (0..self.participants)
            .map(|ordinal| self.participant_visibility(ordinal))
            .collect()
    }

    fn p2p_process_counts_by_visibility(self) -> (usize, usize) {
        let participant_visibilities = self.participant_visibility_profile();
        let public_lanes = 1 + participant_visibilities
            .iter()
            .filter(|visibility| **visibility == LaneVisibility::Public)
            .count();
        let restricted_lanes = participant_visibilities
            .iter()
            .filter(|visibility| **visibility == LaneVisibility::Restricted)
            .count();
        (
            public_lanes * VALIDATORS_PER_LANE,
            restricted_lanes * VALIDATORS_PER_LANE,
        )
    }

    fn validate(self) -> Result<()> {
        ensure!(
            matches!(self.participants, 2 | 3 | 4 | 8 | 16),
            "real-process release matrix supports N=2,3,4,8,16"
        );
        Ok(())
    }
}

fn process_peer(network: &Network, index: usize) -> &NetworkPeer {
    network
        .all_peers()
        .nth(index)
        .unwrap_or_else(|| panic!("process index {index} is in range"))
}

fn participant_dataspace_alias(ordinal: usize) -> String {
    let number = ordinal + 1;
    if ordinal == PRIMARY_PUBLIC_PARTICIPANT_ORDINAL {
        format!("public-{number}")
    } else {
        format!("private-{number}")
    }
}

fn participant_lane_alias(ordinal: usize) -> String {
    format!("lane-{}", participant_dataspace_alias(ordinal))
}

const fn visibility_config_value(visibility: LaneVisibility) -> &'static str {
    match visibility {
        LaneVisibility::Public => "public",
        LaneVisibility::Restricted => "restricted",
    }
}

#[derive(Clone)]
struct CommitteeEndpoints {
    authority: PrivateSettlementCommitteeAuthorityV1,
    endpoints: Vec<Url>,
    validator_keys: Vec<KeyPair>,
}

struct GovernedLeg {
    route: PrivateSettlementRouteV1,
    policy: PrivateSettlementAuditPolicyV1,
    governance: PrivateSettlementPoolGovernanceV1,
    auditor_signing: KeyPair,
    auditor_encryption: HybridKeyPair,
}

struct PreparedLeg {
    governed: GovernedLeg,
    prepared: AtomicPrivateSettlementPreparedLegV1,
    initial_commitments: [PrivacyCommitmentV1; 2],
}

#[derive(Clone)]
struct PrivateSettlementLegPrivateData {
    payer: KeyPair,
    recipient: KeyPair,
    amount: u128,
    memo: Vec<u8>,
}

const LEAKAGE_ACCOUNT_LEFT_I105: &str = "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP";
const LEAKAGE_ACCOUNT_RIGHT_I105: &str = "sorauﾛ1NﾑﾅpﾐTm5Yfﾕ3ｦSヰﾏBｶA5ｻﾔｽｱｼDkDｸkVZBｳﾈyｽﾜヰ9NA1NP";
const LEAKAGE_ASSET_LEFT: &str = "4Zust3cNxfvUrJRuFjSMmNXho9rF";
const LEAKAGE_ASSET_RIGHT: &str = "7fnqfbvxnCke21nA2Zy1C3KktDdi";

fn nexus_fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("universal", "universal").expect("Nexus fee domain"),
        "xor".parse().expect("Nexus fee asset name"),
    )
}

fn bounded_nexus_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(
        vec![FeeChargeLimit::new(
            FeeChargeKind::Nexus,
            nexus_fee_asset_definition_id(),
            Quantity::from(NEXUS_FEE_SIGNED_MAXIMUM),
        )],
        None,
    )
}

fn sponsor_nexus_fee_balance(client: &Client) -> Result<Quantity> {
    let asset = client.query_single(FindAssetById::new(AssetId::new(
        nexus_fee_asset_definition_id(),
        ALICE_ID.clone(),
    )))?;
    Ok(asset.value().clone())
}

fn ensure_exact_private_settlement_carrier_fee(
    before: &Quantity,
    after: &Quantity,
    context: &str,
) -> Result<()> {
    let expected: Quantity = NEXUS_FEE_PER_PRIVATE_SETTLEMENT_CARRIER
        .parse()
        .expect("canonical private-settlement carrier fee");
    let charged = before
        .checked_sub(after)
        .wrap_err_with(|| format!("compute {context} Nexus fee"))?;
    ensure!(
        charged == expected,
        "{context} charged {charged}, expected exactly {expected}"
    );
    Ok(())
}

fn genesis_private_note_activation() -> PrivacyProtocolActivationRecordV1 {
    compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
        .expect("compiled IVM private-note profile")
        .activation_record(PrivacyProtocolLifecycleV1::Proposed(
            PrivacyProposedLifecycleV1 {
                proposed_at_height: PRIVACY_GENESIS_PROPOSAL_HEIGHT,
                activate_at_height: PRIVACY_PROFILE_ACTIVATION_HEIGHT,
            },
        ))
}

fn hash(seed: u8) -> Hash {
    Hash::prehashed([seed.max(1); Hash::LENGTH])
}

fn bytes(seed: u8) -> [u8; 32] {
    [seed.max(1); 32]
}

fn validator_authority_keypair(index: usize) -> KeyPair {
    let mut seed = vec![0_u8; 32];
    seed[0] = 0xC1;
    seed[1..9].copy_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
    KeyPair::try_from_seed(seed, Algorithm::Ed25519).expect("validator authority key")
}

fn stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn cbdc_asset_definition_id(ordinal: usize) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("settlement", "universal").expect("settlement domain"),
        format!("cbdc{}", ordinal + 1)
            .parse()
            .expect("CBDC asset name"),
    )
}

fn transparent_control_domain_id(ordinal: usize) -> DomainId {
    DomainId::try_new(
        format!("control{}", ordinal + 1),
        participant_dataspace_alias(ordinal),
    )
    .expect("transparent-control domain")
}

fn transparent_control_asset_definition_id(ordinal: usize) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        transparent_control_domain_id(ordinal),
        format!("controlcbdc{}", ordinal + 1)
            .parse()
            .expect("transparent-control CBDC asset name"),
    )
}

fn transparent_control_keypair(ordinal: usize) -> KeyPair {
    let mut seed = vec![0_u8; 32];
    seed[0] = 0xD7;
    seed[1..9].copy_from_slice(&u64::try_from(ordinal).unwrap_or(u64::MAX).to_le_bytes());
    KeyPair::try_from_seed(seed, Algorithm::Ed25519).expect("transparent-control account key")
}

fn transparent_control_account_id(ordinal: usize) -> AccountId {
    AccountId::new(transparent_control_keypair(ordinal).public_key().clone())
}

fn transparent_control_asset_id(asset_ordinal: usize, owner_ordinal: usize) -> AssetId {
    AssetId::with_scope(
        transparent_control_asset_definition_id(asset_ordinal),
        transparent_control_account_id(owner_ordinal),
        AssetBalanceScope::Dataspace(DataSpaceId::new(
            u64::try_from(asset_ordinal + 1).expect("control dataspace fits u64"),
        )),
    )
}

fn genesis_post_topology(
    shape: TopologyShape,
    topology: &[PeerId],
    committee_validator_entries: &[GenesisTopologyEntry],
) -> Vec<Vec<InstructionBox>> {
    assert_eq!(topology.len(), shape.process_count());
    assert_eq!(
        committee_validator_entries.len(),
        shape.participant_validator_count()
    );
    let committee_validator_peers = committee_validator_entries
        .iter()
        .map(|entry| entry.peer.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        &topology[shape.global_validator_count()..],
        committee_validator_peers.as_slice(),
        "committee-validator PoP entries must match the participant-process suffix"
    );
    let stake_definition = stake_asset_definition_id();
    // Participant processes are proof-bound WSV peer identities with Committee
    // keys. They are deliberately registered after the exact signed global
    // topology and never become global Sumeragi voters.
    let mut universal = committee_validator_entries
        .iter()
        .map(|entry| {
            let pop = entry
                .pop_bytes()
                .expect("committee-validator topology PoP must be valid hex")
                .expect("committee-validator topology entry must carry a PoP");
            RegisterCommitteePeerWithPop::new(entry.peer.clone(), pop).into()
        })
        .collect::<Vec<InstructionBox>>();
    universal.extend([
        Register::domain(Domain::new(
            DomainId::try_new("universal", "universal").expect("Nexus fee domain"),
        ))
        .into(),
        Register::domain(Domain::new(
            DomainId::try_new("nexus", "universal").expect("nexus domain"),
        ))
        .into(),
        Register::domain(Domain::new(
            DomainId::try_new("settlement", "universal").expect("settlement domain"),
        ))
        .into(),
        Register::asset_definition(AssetDefinition::numeric(
            stake_definition.clone(),
            "xor".to_owned(),
            AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Register::asset_definition(AssetDefinition::numeric(
            nexus_fee_asset_definition_id(),
            "xor".to_owned(),
            AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Mint::asset_quantity(
            NEXUS_FEE_SEED_BALANCE,
            AssetId::new(nexus_fee_asset_definition_id(), ALICE_ID.clone()),
        )
        .into(),
        Grant::account_permission(Permission::from(CanEnactGovernance), ALICE_ID.clone()).into(),
    ]);
    for ordinal in 0..shape.participants {
        let definition = cbdc_asset_definition_id(ordinal);
        universal.push(
            Register::asset_definition(AssetDefinition::numeric(
                definition,
                format!("CBDC {}", ordinal + 1),
                AssetBalancePolicy::Global,
                None,
            ))
            .into(),
        );
    }
    for (index, _) in topology.iter().enumerate() {
        let validator = AccountId::new(validator_authority_keypair(index).public_key().clone());
        universal.push(Register::account(Account::new(validator.clone())).into());
        universal.push(
            Mint::asset_quantity(
                VALIDATOR_STAKE,
                AssetId::new(stake_definition.clone(), validator),
            )
            .into(),
        );
    }
    // Keep the grant and governed registration in one genesis transaction.
    // Genesis pre-exec evaluates its transactions independently, so a grant in
    // an earlier transaction is not an authorization source for a later one.
    // Instruction order inside this transaction makes the grant visible before
    // the profile is registered at canonical height one.
    universal
        .push(RegisterPrivacyProtocolActivationV1::new(genesis_private_note_activation()).into());
    let mut transactions = vec![universal];
    for ordinal in 0..shape.participants {
        let control_domain = transparent_control_domain_id(ordinal);
        transactions.push(vec![
            Register::domain(Domain::new(control_domain.clone())).into(),
            Register::asset_definition(AssetDefinition::numeric(
                transparent_control_asset_definition_id(ordinal),
                format!("Transparent control CBDC {}", ordinal + 1),
                AssetBalancePolicy::DataspaceRestricted,
                Some(control_domain),
            ))
            .into(),
            Register::account(Account::new(transparent_control_account_id(ordinal))).into(),
        ]);
    }
    // Nexus fees are globally scoped even when the business instruction is
    // routed to a restricted dataspace. Fund every authority that signs a
    // transparent control transaction in one universal transaction after the
    // corresponding accounts have been registered.
    transactions.push(
        (0..shape.participants)
            .map(|ordinal| {
                Mint::asset_quantity(
                    NEXUS_FEE_SEED_BALANCE,
                    AssetId::new(
                        nexus_fee_asset_definition_id(),
                        transparent_control_account_id(ordinal),
                    ),
                )
                .into()
            })
            .collect(),
    );
    // Keep each restricted balance mutation in its authoritative dataspace.
    for asset_ordinal in 0..shape.participants {
        let mut mints = Vec::new();
        if asset_ordinal == 0 {
            mints.push(
                Mint::asset_quantity(
                    TRANSPARENT_CONTROL_SEED_BALANCE,
                    transparent_control_asset_id(0, 0),
                )
                .into(),
            );
            for owner_ordinal in 1..shape.participants {
                mints.push(
                    Mint::asset_quantity(
                        TRANSPARENT_CONTROL_OUTPUT_BASELINE,
                        transparent_control_asset_id(0, owner_ordinal),
                    )
                    .into(),
                );
            }
        } else {
            mints.push(
                Mint::asset_quantity(
                    TRANSPARENT_CONTROL_OUTPUT_BASELINE,
                    transparent_control_asset_id(asset_ordinal, 0),
                )
                .into(),
            );
            mints.push(
                Mint::asset_quantity(
                    TRANSPARENT_CONTROL_SEED_BALANCE,
                    transparent_control_asset_id(asset_ordinal, asset_ordinal),
                )
                .into(),
            );
        }
        transactions.push(mints);
    }
    // Staking uses one globally scoped stake asset, so all lane registrations
    // remain together in the targetless transaction routed through universal.
    let mut authority_registration =
        Vec::with_capacity(shape.lane_count() * VALIDATORS_PER_LANE * 2);
    for lane_ordinal in 0..shape.lane_count() {
        let lane = LaneId::new(u32::try_from(lane_ordinal).expect("lane fits u32"));
        for index in lane_ordinal * VALIDATORS_PER_LANE..(lane_ordinal + 1) * VALIDATORS_PER_LANE {
            let peer = topology.get(index).expect("lane validator peer");
            let validator = AccountId::new(validator_authority_keypair(index).public_key().clone());
            authority_registration.push(
                RegisterPublicLaneValidator::new(
                    lane,
                    validator.clone(),
                    peer.clone(),
                    validator.clone(),
                    Quantity::from(VALIDATOR_STAKE),
                    Metadata::default(),
                )
                .into(),
            );
            authority_registration.push(ActivatePublicLaneValidator::new(lane, validator).into());
        }
    }
    transactions.push(authority_registration);
    transactions
}

fn localnet_builder(shape: TopologyShape) -> NetworkBuilder {
    let stake_escrow = ALICE_ID
        .canonical_i105()
        .expect("canonical staking escrow account");
    let validator_worker_threads = i64::try_from(REAL_PROCESS_VALIDATOR_WORKER_THREADS)
        .expect("validator worker width fits i64");
    NetworkBuilder::new()
        .with_base_seed("atomic-private-settlement-n3-real-process-v1")
        .with_peers(shape.global_validator_count())
        .with_committee_validator_p2p_bootstrap(
            CommitteeValidatorP2pBootstrap::new(shape.participant_validator_count())
                .expect("participant committee validator count fits the P2P capacity"),
        )
        .expect("global and participant committee validators fit P2P fanout")
        // Keep every release profile, including the correctness-only N=3
        // smoke, on a production-like signed cadence. The smoke deliberately
        // pays the mandatory 300-height governance notice in full.
        .with_block_cadence(Duration::from_secs(4))
        .with_peer_startup_timeout(Duration::from_secs(20 * 60))
        .with_npos_consensus()
        .without_npos_genesis_bootstrap()
        .with_genesis_block_and_committee_validator_entries(
            move |topology, topology_entries, committee_validator_entries| {
                let mut process_topology = topology.iter().cloned().collect::<Vec<_>>();
                process_topology.extend(
                    committee_validator_entries
                        .iter()
                        .map(|entry| entry.peer.clone()),
                );
                assert_eq!(process_topology.len(), shape.process_count());
                unexecuted_genesis_factory_with_post_topology(
                    Vec::new(),
                    genesis_post_topology(shape, &process_topology, &committee_validator_entries),
                    topology,
                    topology_entries,
                )
            },
        )
        .with_genesis_instruction(npos_override_instruction(VALIDATORS_PER_LANE))
        .with_config_layer(move |layer| {
            let lanes = (0..shape.lane_count())
                .map(|lane| {
                    let mut table = Table::new();
                    table.insert("index".into(), TomlValue::Integer(lane as i64));
                    table.insert(
                        "alias".into(),
                        TomlValue::String(if lane == 0 {
                            "lane-global".to_owned()
                        } else {
                            participant_lane_alias(lane - 1)
                        }),
                    );
                    table.insert(
                        "dataspace".into(),
                        TomlValue::String(if lane == 0 {
                            "universal".to_owned()
                        } else {
                            participant_dataspace_alias(lane - 1)
                        }),
                    );
                    table.insert(
                        "visibility".into(),
                        TomlValue::String(
                            if lane == 0 {
                                "public"
                            } else {
                                visibility_config_value(shape.participant_visibility(lane - 1))
                            }
                            .to_owned(),
                        ),
                    );
                    table.insert("metadata".into(), TomlValue::Table(Table::new()));
                    TomlValue::Table(table)
                })
                .collect::<Vec<_>>();
            let dataspaces = (0..shape.lane_count())
                .map(|dataspace| {
                    let mut table = Table::new();
                    table.insert(
                        "alias".into(),
                        TomlValue::String(if dataspace == 0 {
                            "universal".to_owned()
                        } else {
                            participant_dataspace_alias(dataspace - 1)
                        }),
                    );
                    table.insert("id".into(), TomlValue::Integer(dataspace as i64));
                    table.insert("fault_tolerance".into(), TomlValue::Integer(1));
                    table.insert(
                        "description".into(),
                        TomlValue::String(format!("atomic settlement dataspace {dataspace}")),
                    );
                    if dataspace != 0 {
                        table.insert(
                            "manifest_hash".into(),
                            TomlValue::String(format!("{dataspace:02x}{}", "00".repeat(31))),
                        );
                    }
                    TomlValue::Table(table)
                })
                .collect::<Vec<_>>();
            let routing_rules = (0..shape.participants)
                .map(|ordinal| {
                    let mut matcher = Table::new();
                    matcher.insert(
                        "account".into(),
                        TomlValue::String(transparent_control_account_id(ordinal).to_string()),
                    );
                    let mut rule = Table::new();
                    rule.insert(
                        "lane".into(),
                        TomlValue::Integer(i64::try_from(ordinal + 1).expect("lane fits i64")),
                    );
                    rule.insert(
                        "dataspace".into(),
                        TomlValue::String(participant_dataspace_alias(ordinal)),
                    );
                    rule.insert("matcher".into(), TomlValue::Table(matcher));
                    TomlValue::Table(rule)
                })
                .collect::<Vec<_>>();
            let mut routing = Table::new();
            routing.insert("default_lane".into(), TomlValue::Integer(0));
            routing.insert(
                "default_dataspace".into(),
                TomlValue::String("universal".to_owned()),
            );
            routing.insert("rules".into(), TomlValue::Array(routing_rules));
            layer
                .write(
                    ["concurrency", "scheduler_min_threads"],
                    validator_worker_threads,
                )
                .write(
                    ["concurrency", "scheduler_max_threads"],
                    validator_worker_threads,
                )
                .write(
                    ["concurrency", "rayon_global_threads"],
                    validator_worker_threads,
                )
                .write(["pipeline", "workers"], validator_worker_threads)
                .write(["nexus", "lane_count"], shape.lane_count() as i64)
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                )
                .write(["nexus", "lane_catalog"], TomlValue::Array(lanes))
                .write(["nexus", "dataspace_catalog"], TomlValue::Array(dataspaces))
                .write(["nexus", "routing_policy"], TomlValue::Table(routing))
                .write(
                    ["nexus", "fees", "fee_asset_id"],
                    nexus_fee_asset_definition_id().to_string(),
                )
                .write(["nexus", "fees", "base_fee"], "0")
                .write(["nexus", "fees", "per_byte_fee"], "0")
                .write(
                    ["nexus", "fees", "per_instruction_fee"],
                    NEXUS_FEE_PER_PRIVATE_SETTLEMENT_CARRIER,
                )
                .write(["nexus", "fees", "per_gas_unit_fee"], "0")
                .write(
                    ["nexus", "staking", "public_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "restricted_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_definition_id().to_string(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    stake_escrow.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    stake_escrow.clone(),
                )
                .write(
                    ["nexus", "staking", "max_validators"],
                    VALIDATORS_PER_LANE as i64,
                )
                .write(["zk", "stark", "enabled"], true)
                .write(["nexus", "atomic_private_settlement", "enabled"], true)
                .write(
                    ["nexus", "atomic_private_settlement", "activation_height"],
                    PRIVATE_SETTLEMENT_ACTIVATION_HEIGHT as i64,
                )
                .write(
                    [
                        "nexus",
                        "atomic_private_settlement",
                        "minimum_activation_notice_blocks",
                    ],
                    PRIVATE_SETTLEMENT_MINIMUM_ACTIVATION_NOTICE_BLOCKS as i64,
                )
                .write(
                    ["nexus", "atomic_private_settlement", "max_participants"],
                    16_i64,
                )
                .write(
                    ["nexus", "atomic_private_settlement", "max_expiry_blocks"],
                    MAX_EXPIRY_BLOCKS as i64,
                )
                .write(
                    ["nexus", "atomic_private_settlement", "audit_timeout_blocks"],
                    1_024_i64,
                )
                .write(
                    [
                        "nexus",
                        "atomic_private_settlement",
                        "prepare_timeout_blocks",
                    ],
                    1_024_i64,
                )
                .write(
                    [
                        "nexus",
                        "atomic_private_settlement",
                        "commit_timeout_blocks",
                    ],
                    1_024_i64,
                )
                .write(
                    [
                        "nexus",
                        "atomic_private_settlement",
                        "sidecar_retention_blocks",
                    ],
                    SIDECAR_RETENTION_BLOCKS as i64,
                )
                .write(
                    ["nexus", "atomic_private_settlement", "sidecar_max_records"],
                    64_i64,
                )
                .write(
                    [
                        "nexus",
                        "atomic_private_settlement",
                        "sidecar_max_total_bytes",
                    ],
                    1_073_741_824_i64,
                )
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1_i64,
                );
        })
}

fn n3_smoke_builder(shape: TopologyShape) -> NetworkBuilder {
    // Keep the production-like four-second cadence so a release host running
    // sixteen independent validators has enough time to validate and relay the
    // mandatory DA payload before the view deadline. This release-only smoke
    // deliberately pays the full 300-height privacy-governance notice instead
    // of weakening the consensus rule or using a test-only activation path.
    // The authenticated test controller exposes the same financial-state
    // observation route used by the release fault campaign. No fault rule is
    // installed by the positive smoke test.
    localnet_builder(shape).with_consensus_message_control()
}

fn routes_from_network(
    network: &Network,
    shape: TopologyShape,
) -> Result<Vec<PrivateSettlementRouteV1>> {
    let status = network.client().get_lane_lifecycle_status()?;
    status
        .validate()
        .wrap_err("validate lane lifecycle status")?;
    for ordinal in 0..shape.participants {
        let lane_id = LaneId::new(u32::try_from(ordinal + 1).expect("lane fits u32"));
        let configured = status
            .lanes
            .iter()
            .find(|lane| lane.id == lane_id)
            .ok_or_else(|| eyre!("participant lane {} is absent", ordinal + 1))?;
        let expected = shape.participant_visibility(ordinal);
        ensure!(
            configured.visibility == expected,
            "participant lane {} visibility is {}, expected {}",
            ordinal + 1,
            configured.visibility.as_str(),
            expected.as_str()
        );
    }
    (1..=shape.participants)
        .map(|lane| {
            let lane_id = LaneId::new(u32::try_from(lane).expect("lane fits u32"));
            let incarnation = status
                .incarnations
                .iter()
                .find(|entry| entry.lane_id == lane_id)
                .ok_or_else(|| eyre!("lane {lane} has no active incarnation"))?
                .incarnation;
            Ok(PrivateSettlementRouteV1 {
                dataspace_id: DataSpaceId::new(u64::try_from(lane).expect("dataspace fits u64")),
                lane_id,
                lane_incarnation: incarnation,
            })
        })
        .collect()
}

fn committees_from_network(
    network: &Network,
    shape: TopologyShape,
    routes: &[PrivateSettlementRouteV1],
) -> Result<Vec<CommitteeEndpoints>> {
    routes
        .iter()
        .enumerate()
        .map(|(ordinal, route)| {
            let lane = ordinal + 1;
            let processes = network.all_peers().collect::<Vec<_>>();
            let mut rows = processes[shape.committee_range(lane)]
                .iter()
                .map(|peer: &&NetworkPeer| {
                    let validator = PeerId::from(
                        peer.bls_public_key()
                            .ok_or_else(|| eyre!("validator has no BLS identity"))?
                            .clone(),
                    );
                    let pop = peer
                        .bls_pop()
                        .ok_or_else(|| eyre!("validator has no BLS PoP"))?
                        .to_vec();
                    Ok((
                        validator,
                        pop,
                        Url::parse(&peer.torii_url())?,
                        peer.bls_key_pair()
                            .ok_or_else(|| eyre!("validator has no BLS key pair"))?
                            .clone(),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            rows.sort_by(|left, right| left.0.cmp(&right.0));
            let validators = rows.iter().map(|row| row.0.clone()).collect::<Vec<_>>();
            let authority = PrivateSettlementCommitteeAuthorityV1 {
                route: *route,
                validator_set_hash: HashOf::new(&validators),
                validators,
                validator_pops: rows.iter().map(|row| row.1.clone()).collect(),
            };
            authority
                .validate()
                .wrap_err("validate real four-validator authority")?;
            Ok(CommitteeEndpoints {
                authority,
                endpoints: rows.iter().map(|row| row.2.clone()).collect(),
                validator_keys: rows.into_iter().map(|row| row.3).collect(),
            })
        })
        .collect()
}

fn activate_ivm_private_note(client: &Client) -> Result<u64> {
    let expected = genesis_private_note_activation();
    let expected_compiled_profile = PrivacyCompiledProfileResultV1::Available(
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)?.into(),
    );
    let mut ticks = 0_u64;
    let tick_limit = PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1
        .checked_add(16)
        .expect("privacy activation tick limit fits u64");
    loop {
        let capability = client.get_privacy_capabilities()?;
        let row = capability
            .protocols
            .iter()
            .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
            .ok_or_else(|| eyre!("IVM private-note capability row is absent"))?;
        let activation = row
            .activation
            .ok_or_else(|| eyre!("governed IVM private-note activation is absent"))?;
        let mut expected_at_lifecycle = expected;
        expected_at_lifecycle.lifecycle = activation.lifecycle;
        ensure!(
            activation == expected_at_lifecycle,
            "governed IVM private-note activation bindings differ from genesis"
        );
        match activation.lifecycle {
            PrivacyProtocolLifecycleV1::Active(active) => {
                ensure!(
                    active.proposed_at_height == PRIVACY_GENESIS_PROPOSAL_HEIGHT
                        && active.activated_at_height == PRIVACY_PROFILE_ACTIVATION_HEIGHT
                        && active.state_since_height == active.activated_at_height,
                    "governed IVM private-note activation history differs from genesis schedule"
                );
                ensure!(
                    row.compiled_profile == expected_compiled_profile,
                    "active IVM profile differs from the exact compiled private-note profile"
                );
                return Ok(capability.committed_height);
            }
            PrivacyProtocolLifecycleV1::Proposed(proposed) => {
                ensure!(
                    proposed
                        == match expected.lifecycle {
                            PrivacyProtocolLifecycleV1::Proposed(expected) => expected,
                            _ => unreachable!("genesis activation is proposed"),
                        },
                    "governed IVM private-note proposal schedule differs from genesis"
                );
            }
            PrivacyProtocolLifecycleV1::Suspended(_) | PrivacyProtocolLifecycleV1::Retired(_) => {
                return Err(eyre!(
                    "governed IVM private-note activation became unavailable before the smoke"
                ));
            }
        }
        ensure!(
            ticks < tick_limit,
            "governed IVM private-note activation did not promote within {tick_limit} blocks"
        );
        let tick = client.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                format!(
                    "atomic-private-settlement activation tick {}",
                    capability.committed_height
                ),
            ))],
            bounded_nexus_fee(),
            Metadata::default(),
        );
        client.submit_transaction_blocking(&tick)?;
        ticks += 1;
    }
}

fn signing_key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

fn leakage_canary_keypair(variant: &str) -> Result<KeyPair> {
    let seed = match variant {
        "left" => [
            0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
            0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
            0x1c, 0xae, 0x7f, 0x60,
        ],
        "right" => [
            0x4c, 0xcd, 0x08, 0x9b, 0x28, 0xff, 0x96, 0xda, 0x9d, 0xb6, 0xc3, 0x46, 0xec, 0x11,
            0x4e, 0x0f, 0x5b, 0x8a, 0x31, 0x9f, 0x35, 0xab, 0xa6, 0x24, 0xda, 0x8c, 0xf6, 0xed,
            0x4f, 0xb8, 0xa6, 0xfb,
        ],
        _ => return Err(eyre!("leakage variant must be left or right")),
    };
    KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
        .wrap_err("derive the fixed RFC 8032 leakage canary key")
}

fn leakage_canary_account_id(variant: &str) -> Result<AccountId> {
    Ok(AccountId::new(
        leakage_canary_keypair(variant)?.public_key().clone(),
    ))
}

fn leakage_canary_asset_definition_id(variant: &str) -> Result<AssetDefinitionId> {
    let literal = match variant {
        "left" => LEAKAGE_ASSET_LEFT,
        "right" => LEAKAGE_ASSET_RIGHT,
        _ => return Err(eyre!("leakage variant must be left or right")),
    };
    literal
        .parse()
        .wrap_err("parse the fixed canonical leakage asset definition id")
}

fn default_private_settlement_leg_data(ordinal: usize) -> PrivateSettlementLegPrivateData {
    PrivateSettlementLegPrivateData {
        payer: signing_key(0xA1 + ordinal as u8),
        recipient: signing_key(0xB1 + ordinal as u8),
        amount: 42 + ordinal as u128,
        memo: format!("BCK26-private-settlement-leg-{ordinal}").into_bytes(),
    }
}

fn governed_legs(
    routes: &[PrivateSettlementRouteV1],
    authority_context_height: u64,
    expiry_height: u64,
) -> Result<Vec<GovernedLeg>> {
    governed_legs_with_asset_definitions(routes, authority_context_height, expiry_height, None)
}

fn governed_legs_with_asset_definitions(
    routes: &[PrivateSettlementRouteV1],
    authority_context_height: u64,
    expiry_height: u64,
    asset_definition_ids: Option<&[AssetDefinitionId]>,
) -> Result<Vec<GovernedLeg>> {
    if let Some(asset_definition_ids) = asset_definition_ids {
        ensure!(
            asset_definition_ids.len() == routes.len(),
            "private settlement asset override count must equal the route count"
        );
    }
    routes
        .iter()
        .enumerate()
        .map(|(ordinal, route)| {
            let auditor_signing = signing_key(0x30 + ordinal as u8);
            let auditor_id = AccountId::new(auditor_signing.public_key().clone());
            let mut rng = iroha_crypto::rng_from_seed_slice(&bytes(0x40 + ordinal as u8));
            let auditor_encryption = HybridKeyPair::generate(&mut rng)?;
            let policy = PrivateSettlementAuditPolicyV1::new(PrivateSettlementAuditPolicyBodyV1 {
                version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                dataspace_id: route.dataspace_id,
                policy_id: hash(0x50 + ordinal as u8),
                revision: 1,
                key_epoch: 1,
                activation_height: authority_context_height,
                retirement_height: Some(expiry_height + 1),
                min_approvals: 1,
                auditors: vec![PrivateSettlementAuditorV1 {
                    auditor_id,
                    signing_key: auditor_signing.public_key().clone(),
                    encryption_key: PrivateSettlementHybridPublicKeyV1::from_hybrid(
                        auditor_encryption.public(),
                    ),
                }],
            })?;
            let governance = PrivateSettlementPoolGovernanceV1::from_restricted_mapping(
                *route,
                PrivacyPoolIdV1::new(bytes(0x60 + ordinal as u8)),
                asset_definition_ids.map_or_else(
                    || cbdc_asset_definition_id(ordinal),
                    |ids| ids[ordinal].clone(),
                ),
                bytes(0x70 + ordinal as u8),
                &policy,
                PrivateSettlementPoolGovernanceLifecycleV1 {
                    governance_revision: 1,
                    activation_height: authority_context_height,
                    retirement_height: Some(expiry_height + 1),
                },
            )?;
            Ok(GovernedLeg {
                route: *route,
                policy,
                governance,
                auditor_signing,
                auditor_encryption,
            })
        })
        .collect()
}

fn placeholder_payer_authorization(
    network_id: iroha::data_model::NetworkId,
    route: PrivateSettlementRouteV1,
    payer: &KeyPair,
    expiry_height: u64,
) -> PrivateSettlementAuditPayerAuthorizationV1 {
    let body = PrivateSettlementAuditPayerAuthorizationBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        purpose: hash(0x81),
        network_id,
        bundle_id: hash(0x82),
        leg_ordinal: 0,
        route,
        payer: AccountId::new(payer.public_key().clone()),
        expiry_height,
        inputs: vec![
            PrivateSettlementAuditPayerInputV1 {
                input_ordinal: 0,
                active: true,
                commitment: PrivacyCommitmentV1::new(bytes(0x83)),
                nullifier: PrivacyNullifierV1::new(bytes(0x84)),
                note_spending_authority: bytes(0x85),
                dummy_domain: None,
            },
            PrivateSettlementAuditPayerInputV1 {
                input_ordinal: 1,
                active: false,
                commitment: PrivacyCommitmentV1::new(bytes(0x86)),
                nullifier: PrivacyNullifierV1::new(bytes(0x87)),
                note_spending_authority: bytes(0x88),
                dummy_domain: Some(hash(0x89)),
            },
        ],
    };
    PrivateSettlementAuditPayerAuthorizationV1::new(
        body.clone(),
        vec![PrivateSettlementAuditPayerSignatureV1::new(
            payer.public_key().clone(),
            SignatureOf::try_new(payer.private_key(), &body).expect("placeholder signature"),
        )],
    )
}

fn placeholder_view_authorization(
    network_id: iroha::data_model::NetworkId,
    route: PrivateSettlementRouteV1,
    signer: &KeyPair,
    expiry_height: u64,
) -> PrivateSettlementAuditViewKeyAuthorizationV1 {
    let body = PrivateSettlementAuditViewKeyAuthorizationBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        purpose: hash(0x8A),
        network_id,
        bundle_id: hash(0x8B),
        leg_ordinal: 0,
        route,
        output_ordinal: 0,
        role: PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
        authorized_account: AccountId::new(signer.public_key().clone()),
        recipient_view_key: bytes(0x8C),
        output_active: true,
        note_spending_authority: bytes(0x8D),
        expiry_height,
    };
    PrivateSettlementAuditViewKeyAuthorizationV1::new(
        body.clone(),
        vec![PrivateSettlementAuditViewKeySignatureV1::new(
            signer.public_key().clone(),
            SignatureOf::try_new(signer.private_key(), &body).expect("placeholder signature"),
        )],
    )
}

fn private_settlement_leg_private_material(
    manifest: &AtomicPrivateSettlementV1,
    governed: &GovernedLeg,
    leg_ordinal: usize,
    material_ordinal: usize,
    purpose: &[u8],
) -> [u8; 32] {
    use sha2::Digest as _;

    let mut digest = sha2::Sha256::new();
    digest.update(PRIVATE_SETTLEMENT_LEG_PRIVATE_MATERIAL_DOMAIN_V1);
    digest.update(manifest.bundle_id.as_ref());
    digest.update(governed.governance.governance_digest.as_ref());
    digest.update(
        u64::try_from(leg_ordinal)
            .expect("private-settlement leg ordinal fits u64")
            .to_le_bytes(),
    );
    digest.update(
        u64::try_from(material_ordinal)
            .expect("private-settlement material ordinal fits u64")
            .to_le_bytes(),
    );
    digest.update(
        u64::try_from(purpose.len())
            .expect("private-settlement material purpose length fits u64")
            .to_le_bytes(),
    );
    digest.update(purpose);
    digest.finalize().into()
}

fn private_settlement_reimbursement_terms_salt(
    manifest: &AtomicPrivateSettlementV1,
    governed: &GovernedLeg,
) -> [u8; 32] {
    use sha2::Digest as _;

    // `bundle_id` commits to the reimbursement commitment, so derive this
    // salt from the rest of the unique bundle preimage to avoid a cycle.
    let mut digest = sha2::Sha256::new();
    digest.update(PRIVATE_SETTLEMENT_REIMBURSEMENT_SALT_DOMAIN_V1);
    digest.update(manifest.network_id.as_bytes());
    digest.update(manifest.authority_context_height.to_le_bytes());
    digest.update(manifest.expiry_height.to_le_bytes());
    digest.update(manifest.fee_intent_digest.as_ref());
    digest.update(governed.governance.governance_digest.as_ref());
    digest.finalize().into()
}

fn note_opening(
    manifest: &AtomicPrivateSettlementV1,
    governed: &GovernedLeg,
    leg_ordinal: usize,
    purpose: &[u8],
    note_ordinal: usize,
    active: bool,
    value: u128,
) -> PrivateSettlementAuditNoteOpeningV1 {
    let field_base = note_ordinal
        .checked_mul(6)
        .expect("private-settlement note material ordinal fits usize");
    let field = |offset| {
        private_settlement_leg_private_material(
            manifest,
            governed,
            leg_ordinal,
            field_base + offset,
            purpose,
        )
    };
    PrivateSettlementAuditNoteOpeningV1 {
        active,
        commitment: PrivacyCommitmentV1::new(field(0)),
        value,
        spending_authority: field(1),
        rho: field(2),
        blinding: field(3),
        memo_digest: field(4),
        dummy_domain: (!active).then(|| Hash::prehashed(field(5))),
    }
}

fn placeholder_encrypted_output(seed: u8) -> PrivacyEncryptedOutputV1 {
    let mut ciphertext = vec![seed.max(1); PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1];
    ciphertext[..4].copy_from_slice(b"IPNE");
    PrivacyEncryptedOutputV1 {
        recipient: PrivacyRecipientIdV1::new(bytes(seed.wrapping_add(1))),
        ephemeral_public_key: PrivacyEncryptionKeyV1::new(bytes(seed.wrapping_add(2))),
        commitment: PrivacyCommitmentV1::new(bytes(seed.wrapping_add(3))),
        ciphertext,
    }
}

fn reimbursement_commitment(
    manifest: &AtomicPrivateSettlementV1,
    governed: &GovernedLeg,
) -> Result<Hash> {
    let payer = signing_key(0x91);
    let probe = PrivateSettlementAuditPlaintextV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: manifest.network_id,
        bundle_id: hash(0x92),
        leg_ordinal: 0,
        route: governed.route,
        pool_id: governed.governance.body.pool_id,
        payer: AccountId::new(payer.public_key().clone()),
        payer_authorization: placeholder_payer_authorization(
            manifest.network_id,
            governed.route,
            &payer,
            manifest.expiry_height,
        ),
        recipient: AccountId::new(signing_key(0x93).public_key().clone()),
        sponsor: manifest.sponsor.clone(),
        asset_definition_id: governed.governance.body.asset_definition_id.clone(),
        asset_binding_salt: governed.governance.body.asset_binding_salt,
        amount: 1,
        sponsor_reimbursement_amount: 5,
        fee_intent_digest: manifest.fee_intent_digest,
        settlement_expiry_height: manifest.expiry_height,
        reimbursement_terms_salt: private_settlement_reimbursement_terms_salt(manifest, governed),
        memo: Vec::new(),
        policy_references: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    Ok(probe.reimbursement_terms_commitment()?)
}

fn proof_manifest(
    network_id: iroha::data_model::NetworkId,
    authority_context_height: u64,
    expiry_height: u64,
    governed: &[GovernedLeg],
) -> Result<AtomicPrivateSettlementV1> {
    let mut manifest = AtomicPrivateSettlementV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id,
        bundle_id: hash(0xA0),
        authority_context_height,
        expiry_height,
        sponsor: ALICE_ID.clone(),
        public_fee_intent: bounded_nexus_fee(),
        fee_intent_digest: hash(0xA1),
        reimbursement_terms_commitment: hash(0xA2),
        reimbursement_leg_ordinal: 0,
        legs: governed
            .iter()
            .enumerate()
            .map(|(ordinal, leg)| PrivateSettlementLegCommitmentV1 {
                ordinal: u8::try_from(ordinal).expect("ordinal fits u8"),
                route: leg.route,
                pool_id: leg.governance.body.pool_id,
                asset_binding_commitment: leg.governance.body.asset_binding_commitment,
                audit_policy_digest: leg.policy.policy_digest,
                payload_digest: hash(0xB0 + ordinal as u8),
                availability_certificate_digest: hash(0xC0 + ordinal as u8),
                delta_digest: hash(0xD0 + ordinal as u8),
            })
            .collect(),
    };
    manifest.fee_intent_digest = manifest.computed_fee_intent_digest()?;
    manifest.reimbursement_terms_commitment = reimbursement_commitment(&manifest, &governed[0])?;
    manifest.bundle_id = manifest.computed_bundle_id()?;
    manifest.validate()?;
    Ok(manifest)
}

fn prepare_leg(
    ordinal: usize,
    governed: GovernedLeg,
    manifest: &AtomicPrivateSettlementV1,
    authority_digest: Hash,
) -> Result<PreparedLeg> {
    let private_data = default_private_settlement_leg_data(ordinal);
    prepare_leg_with_private_data(ordinal, governed, manifest, authority_digest, &private_data)
}

fn prepare_leg_with_private_data(
    ordinal: usize,
    governed: GovernedLeg,
    manifest: &AtomicPrivateSettlementV1,
    authority_digest: Hash,
    private_data: &PrivateSettlementLegPrivateData,
) -> Result<PreparedLeg> {
    let output_rng_seed = private_settlement_leg_private_material(
        manifest,
        &governed,
        ordinal,
        0,
        b"output-encryption-rng",
    );
    let capsule_rng_seed = private_settlement_leg_private_material(
        manifest,
        &governed,
        ordinal,
        0,
        b"audit-capsule-rng",
    );
    let mut output_rng = iroha_crypto::rng_from_seed_slice(&output_rng_seed);
    let mut capsule_rng = iroha_crypto::rng_from_seed_slice(&capsule_rng_seed);
    prepare_leg_with_private_data_and_rngs(
        ordinal,
        governed,
        manifest,
        authority_digest,
        private_data,
        &mut output_rng,
        &mut capsule_rng,
    )
}

fn prepare_leg_with_private_data_and_rngs(
    ordinal: usize,
    governed: GovernedLeg,
    manifest: &AtomicPrivateSettlementV1,
    authority_digest: Hash,
    private_data: &PrivateSettlementLegPrivateData,
    output_rng: &mut (impl rand_core_06::RngCore + rand_core_06::CryptoRng),
    capsule_rng: &mut impl rand::rand_core::TryCryptoRng,
) -> Result<PreparedLeg> {
    let profile = PrivateSettlementProofProfileV1::IvmPrivateNoteFixed2In3Out;
    let placeholders = [
        placeholder_encrypted_output(0x11 + ordinal as u8 * 6),
        placeholder_encrypted_output(0x13 + ordinal as u8 * 6),
        placeholder_encrypted_output(0x15 + ordinal as u8 * 6),
    ];
    let mut statement = PrivateSettlementProofStatementV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        profile,
        proof_profile_digest: profile.digest(),
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        leg_ordinal: ordinal as u8,
        route: governed.route,
        authority_context_height: manifest.authority_context_height,
        pool_id: governed.governance.body.pool_id,
        asset_binding_commitment: governed.governance.body.asset_binding_commitment,
        old_root: PrivacyRootV1::new(bytes(0x20 + ordinal as u8)),
        new_root: PrivacyRootV1::new(bytes(0x24 + ordinal as u8)),
        old_epoch: 1,
        new_epoch: 2,
        nullifiers: vec![
            PrivacyNullifierV1::new(bytes(0x30 + ordinal as u8 * 2)),
            PrivacyNullifierV1::new(bytes(0x31 + ordinal as u8 * 2)),
        ],
        output_commitments: placeholders
            .iter()
            .map(|output| output.commitment)
            .collect(),
        encrypted_outputs: placeholders.to_vec(),
        audit_plaintext_commitment: hash(0x40 + ordinal as u8),
        audit_capsule_digest: hash(0x50 + ordinal as u8),
        audit_policy_digest: governed.policy.policy_digest,
        audit_key_epoch: governed.policy.body.key_epoch,
        fee_intent_digest: manifest.fee_intent_digest,
        reimbursement_terms_commitment: manifest.reimbursement_terms_commitment,
        reimbursement_leg_ordinal: manifest.reimbursement_leg_ordinal,
        expiry_height: manifest.expiry_height,
    };
    statement.validate()?;

    let payer = &private_data.payer;
    let recipient = &private_data.recipient;
    let input_secrets = [
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            0,
            b"input-spending-secret",
        ),
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            1,
            b"input-spending-secret",
        ),
    ];
    let output_secrets = [
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            0,
            b"output-spending-secret",
        ),
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            1,
            b"output-spending-secret",
        ),
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            2,
            b"output-spending-secret",
        ),
    ];
    let view_secrets = [
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            0,
            b"output-view-secret",
        ),
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            1,
            b"output-view-secret",
        ),
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            2,
            b"output-view-secret",
        ),
    ];
    let ephemeral_secrets = [
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            0,
            b"output-ephemeral-secret",
        ),
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            1,
            b"output-ephemeral-secret",
        ),
        private_settlement_leg_private_material(
            manifest,
            &governed,
            ordinal,
            2,
            b"output-ephemeral-secret",
        ),
    ];
    let reimbursement = if ordinal == 0 { 5 } else { 0 };
    let change = 7;
    let amount = private_data.amount;
    let input_amount = amount
        .checked_add(change)
        .and_then(|value| value.checked_add(reimbursement))
        .ok_or_else(|| eyre!("private settlement input amount overflow"))?;
    let mut plaintext = PrivateSettlementAuditPlaintextV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        leg_ordinal: ordinal as u8,
        route: governed.route,
        pool_id: governed.governance.body.pool_id,
        payer: AccountId::new(payer.public_key().clone()),
        payer_authorization: placeholder_payer_authorization(
            manifest.network_id,
            governed.route,
            payer,
            manifest.expiry_height,
        ),
        recipient: AccountId::new(recipient.public_key().clone()),
        sponsor: manifest.sponsor.clone(),
        asset_definition_id: governed.governance.body.asset_definition_id.clone(),
        asset_binding_salt: governed.governance.body.asset_binding_salt,
        amount,
        sponsor_reimbursement_amount: reimbursement,
        fee_intent_digest: manifest.fee_intent_digest,
        settlement_expiry_height: manifest.expiry_height,
        reimbursement_terms_salt: private_settlement_reimbursement_terms_salt(manifest, &governed),
        memo: private_data.memo.clone(),
        policy_references: vec![governed.governance.governance_digest],
        inputs: vec![
            note_opening(
                manifest,
                &governed,
                ordinal,
                b"input-note",
                0,
                true,
                input_amount,
            ),
            note_opening(manifest, &governed, ordinal, b"input-note", 1, false, 0),
        ],
        outputs: vec![
            PrivateSettlementAuditOutputV1 {
                role: PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
                recipient_view_key: ivm_private_recipient_public_key_v1(&view_secrets[0])?,
                view_key_authorization: placeholder_view_authorization(
                    manifest.network_id,
                    governed.route,
                    recipient,
                    manifest.expiry_height,
                ),
                encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                    ephemeral_secret: ephemeral_secrets[0],
                },
                note: note_opening(
                    manifest,
                    &governed,
                    ordinal,
                    b"output-note",
                    0,
                    true,
                    amount,
                ),
            },
            PrivateSettlementAuditOutputV1 {
                role: PrivateSettlementAuditOutputRoleV1::PayerChange,
                recipient_view_key: ivm_private_recipient_public_key_v1(&view_secrets[1])?,
                view_key_authorization: placeholder_view_authorization(
                    manifest.network_id,
                    governed.route,
                    payer,
                    manifest.expiry_height,
                ),
                encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                    ephemeral_secret: ephemeral_secrets[1],
                },
                note: note_opening(
                    manifest,
                    &governed,
                    ordinal,
                    b"output-note",
                    1,
                    true,
                    change,
                ),
            },
            PrivateSettlementAuditOutputV1 {
                role: PrivateSettlementAuditOutputRoleV1::SponsorReimbursement,
                recipient_view_key: ivm_private_recipient_public_key_v1(&view_secrets[2])?,
                view_key_authorization: placeholder_view_authorization(
                    manifest.network_id,
                    governed.route,
                    &ALICE_KEYPAIR,
                    manifest.expiry_height,
                ),
                encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                    ephemeral_secret: ephemeral_secrets[2],
                },
                note: note_opening(
                    manifest,
                    &governed,
                    ordinal,
                    b"output-note",
                    2,
                    ordinal == 0,
                    reimbursement,
                ),
            },
        ],
    };
    for (opening, secret) in plaintext.inputs.iter_mut().zip(input_secrets) {
        opening.spending_authority = derive_note_authority_v1(&secret)?;
    }
    for (output, secret) in plaintext.outputs.iter_mut().zip(output_secrets) {
        output.note.spending_authority = derive_note_authority_v1(&secret)?;
    }
    prepare_atomic_private_settlement_input_openings_v1(
        manifest,
        &statement,
        &mut plaintext.inputs,
    )?;
    statement.nullifiers = derive_atomic_private_settlement_input_nullifiers_v1(
        manifest,
        &statement,
        &plaintext.inputs,
        &input_secrets,
    )?
    .to_vec();
    let payer_body = plaintext.payer_authorization_body(&statement.nullifiers)?;
    plaintext.payer_authorization = PrivateSettlementAuditPayerAuthorizationV1::new(
        payer_body.clone(),
        vec![PrivateSettlementAuditPayerSignatureV1::new(
            payer.public_key().clone(),
            SignatureOf::try_new(payer.private_key(), &payer_body)?,
        )],
    );
    for (index, signer) in [recipient, payer, &*ALICE_KEYPAIR].into_iter().enumerate() {
        let body = plaintext.output_view_key_authorization_body(index)?;
        plaintext.outputs[index].view_key_authorization =
            PrivateSettlementAuditViewKeyAuthorizationV1::new(
                body.clone(),
                vec![PrivateSettlementAuditViewKeySignatureV1::new(
                    signer.public_key().clone(),
                    SignatureOf::try_new(signer.private_key(), &body)?,
                )],
            );
    }
    statement.audit_plaintext_commitment = plaintext.commitment()?;
    statement.encrypted_outputs = prepare_atomic_private_settlement_outputs_v1(
        output_rng,
        manifest,
        &statement,
        &mut plaintext.outputs,
    )?;
    statement.output_commitments = plaintext
        .outputs
        .iter()
        .map(|output| output.note.commitment)
        .collect();
    let canonical_plaintext = norito::encode_canonical(&plaintext)?;
    let aad = PrivateSettlementAuditAadV1 {
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        leg_ordinal: ordinal as u8,
        route: governed.route,
        authority_digest,
        authority_context_height: manifest.authority_context_height,
        audit_policy_digest: governed.policy.policy_digest,
        audit_key_epoch: governed.policy.body.key_epoch,
        plaintext_commitment: statement.audit_plaintext_commitment,
    };
    let capsule = seal_private_settlement_audit_capsule_v1_with_rng(
        &canonical_plaintext,
        aad,
        PrivateSettlementCapsulePaddingV1::KiB16,
        &governed.policy,
        capsule_rng,
    )?;
    statement.audit_capsule_digest = capsule.digest()?;
    let bootstrap = plan_atomic_private_settlement_bootstrap_v1(
        statement.pool_id,
        [
            plaintext.inputs[0].commitment,
            plaintext.inputs[1].commitment,
        ],
        statement
            .output_commitments
            .as_slice()
            .try_into()
            .map_err(|_| eyre!("private settlement output commitment shape changed"))?,
        input_secrets,
    )?;
    statement.old_root = bootstrap.old_root;
    statement.new_root = bootstrap.new_root;
    statement.old_epoch = bootstrap.old_epoch;
    statement.new_epoch = bootstrap.new_epoch;
    let initial_commitments = bootstrap.initial_commitments;
    statement.validate()?;
    let wallet_id = format!("atomic-private-settlement-release-leg-{ordinal}");
    let owner_bundle = encode_atomic_private_settlement_wallet_bundle_v1(
        &wallet_id,
        manifest,
        &statement,
        &capsule,
        &governed.policy,
        &plaintext,
        &bootstrap.into_input_secrets(),
    )?;
    let mut owner_material = owner_bundle.to_vec();
    let prepared = consume_atomic_private_settlement_wallet_bundle_v1(
        &mut owner_material,
        &wallet_id,
        manifest,
        &statement,
        &capsule,
        &governed.policy,
        *manifest.network_id.as_genesis_hash().as_ref(),
        manifest.authority_context_height,
    )?;
    ensure!(
        owner_material.iter().all(|byte| *byte == 0),
        "owner bundle was not wiped"
    );
    let prepared = complete_atomic_private_settlement_prepared_leg_v1(prepared)?;
    Ok(PreparedLeg {
        governed,
        prepared,
        initial_commitments,
    })
}

fn provisional_materials(
    manifest: AtomicPrivateSettlementV1,
    prepared: &[PreparedLeg],
    committees: &[CommitteeEndpoints],
) -> Result<Vec<PrivateSettlementProvisionalLegMaterialV1>> {
    ensure!(
        prepared.len() == committees.len() && prepared.len() == manifest.legs.len(),
        "private-settlement prepared-leg and committee counts must match the manifest"
    );
    let retention_until_height = manifest
        .authority_context_height
        .checked_add(SIDECAR_RETENTION_BLOCKS)
        .and_then(|height| height.checked_add(512))
        .ok_or_else(|| eyre!("private-settlement sidecar retention height overflow"))?;
    let inputs = prepared
        .iter()
        .zip(committees)
        .map(|(leg, committee)| {
            AtomicPrivateSettlementProvisionalLegInputV1::new(
                leg.prepared.clone(),
                leg.governed.policy.clone(),
                committee.authority.clone(),
                retention_until_height,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(finalize_atomic_private_settlement_provisional_bundle_v1(manifest, inputs)?.materials)
}

fn assert_no_partial_visibility(network: &Network, bundle_id: Hash, phase: &str) -> Result<()> {
    for peer in network.all_peers() {
        match peer
            .client()
            .private_settlement_bundle_receipt_v1(bundle_id)
        {
            Ok(PrivateSettlementBundleReceiptResponseV1::Pending { .. }) => {}
            Ok(PrivateSettlementBundleReceiptResponseV1::Finalized(receipt)) => {
                return Err(eyre!(
                    "{phase}: peer {} exposed {} finalized legs before global carrier",
                    peer.id(),
                    receipt.legs.len()
                ));
            }
            Ok(PrivateSettlementBundleReceiptResponseV1::Aborted(_)) => {
                return Err(eyre!(
                    "{phase}: peer {} exposed an unexpected abort",
                    peer.id()
                ));
            }
            Err(error) => {
                return Err(eyre!(
                    "{phase}: peer {} receipt query failed instead of proving pending state: {error}",
                    peer.id()
                ));
            }
        }
    }
    Ok(())
}

fn wait_for_identical_receipt(
    network: &Network,
    bundle_id: Hash,
) -> Result<iroha::data_model::nexus::PrivateSettlementReceiptV1> {
    let started = Instant::now();
    let mut last = String::new();
    while started.elapsed() < FINALITY_TIMEOUT {
        let mut receipts = Vec::new();
        for peer in network.all_peers() {
            match peer
                .client()
                .private_settlement_bundle_receipt_v1(bundle_id)
            {
                Ok(PrivateSettlementBundleReceiptResponseV1::Finalized(receipt)) => {
                    receipts.push(receipt)
                }
                Ok(other) => last = format!("{} returned {other:?}", peer.id()),
                Err(error) => last = format!("{}: {error}", peer.id()),
            }
        }
        if receipts.len() == network.all_peers().count()
            && receipts.windows(2).all(|pair| pair[0] == pair[1])
        {
            return Ok(receipts.remove(0));
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "all peers did not converge on one atomic receipt: {last}"
    ))
}

fn run_n3_real_process_smoke() -> Result<()> {
    let (bound, request_sha) = read_bound_real_process_request()?;
    let RealProcessBoundRequestV1::Smoke(smoke_request) = bound else {
        return Err(eyre!("positive smoke received a non-smoke request"));
    };
    let evidence_root = fault_evidence_root().wrap_err("initialize smoke evidence directory")?;
    let mut evidence_files = vec![write_smoke_evidence(
        &evidence_root,
        "request.json",
        &smoke_request,
    )?];
    let shape = TopologyShape::new(PARTICIPANT_COUNT);
    shape.validate()?;
    ensure!(
        shape.global_validator_count() == 4
            && shape.participant_validator_count() == 12
            && shape.process_count() == 16,
        "N=3 requires four global voters plus twelve non-global participant committee validators"
    );
    ensure!(
        shape.participant_visibility_profile()
            == [
                LaneVisibility::Public,
                LaneVisibility::Restricted,
                LaneVisibility::Restricted,
            ],
        "primary N=3 must mix one public and two restricted participant dataspaces"
    );
    let context = "atomic_private_settlement_n3_real_process_smoke";
    let builder = n3_smoke_builder(shape).with_base_seed(format!(
        "aps-smoke:{}:{}:{}",
        smoke_request.seed, smoke_request.run, smoke_request.invocation_nonce
    ));
    let started = sandbox::start_network_blocking_or_skip(builder, context)?;
    let Some((network, runtime)) = sandbox::enforce_network_start_requirement(started, context)?
    else {
        return Err(eyre!("required sixteen-process smoke network was skipped"));
    };
    verify_controller_readiness(&network, &runtime)?;
    let initial_inventory = smoke_process_inventory(&network, &runtime, shape)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "processes-before.json",
        &initial_inventory,
    )?);
    let sponsor = network.client();
    let activated_height = activate_ivm_private_note(&sponsor)?;
    let authority_context_height = activated_height + 1;
    let expiry_height = authority_context_height + 1_000;
    let routes = routes_from_network(&network, shape)?;
    ensure!(
        routes.len() == 3,
        "exactly three mixed-visibility participant dataspaces are required"
    );
    let committees = committees_from_network(&network, shape, &routes)?;
    let governed = governed_legs(&routes, authority_context_height, expiry_height)?;
    let manifest = proof_manifest(
        network.network_id(),
        authority_context_height,
        expiry_height,
        &governed,
    )?;
    let prepared = governed
        .into_iter()
        .zip(&committees)
        .enumerate()
        .map(|(ordinal, (leg, committee))| {
            prepare_leg(ordinal, leg, &manifest, committee.authority.digest()?)
        })
        .collect::<Result<Vec<_>>>()?;
    let activations = prepared
        .iter()
        .map(|leg| {
            ActivatePrivateSettlementPoolV1::from_restricted(
                &leg.governed.governance,
                leg.initial_commitments.to_vec(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let activation_transaction =
        sponsor.build_transaction_from_items(activations, bounded_nexus_fee(), Metadata::default());
    sponsor
        .submit_transaction_blocking(&activation_transaction)
        .wrap_err("activate all three governed confidential pools at the bound context height")?;
    ensure!(
        sponsor.get_privacy_capabilities()?.committed_height == authority_context_height,
        "pool activation did not land at the manifest authority context"
    );
    let before = wait_for_converged_fault_state_snapshot(&network, "smoke-before")?;
    ensure!(
        before.validators.len() == shape.process_count(),
        "positive smoke omitted a global or participant validator"
    );

    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-before.json",
        &before,
    )?);
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "authorities.json",
        &committees
            .iter()
            .map(|committee| &committee.authority)
            .collect::<Vec<_>>(),
    )?);
    let mut observer = FaultContinuousObserverV1::start_retaining_evidence(
        &network,
        &before,
        shape.participants,
        &manifest.bundle_id,
        false,
    )?;
    let materials = provisional_materials(manifest, &prepared, &committees)?;
    let certificates = materials
        .iter()
        .zip(&committees)
        .map(|(material, committee)| {
            sponsor.certify_private_settlement_leg_availability_v1(&committee.endpoints, material)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut final_manifest = materials[0].manifest.clone();
    for (ordinal, certificate) in certificates.iter().enumerate() {
        final_manifest.legs[ordinal].availability_certificate_digest = certificate.digest()?;
    }
    final_manifest.validate()?;
    for (ordinal, ((material, certificate), committee)) in materials
        .iter()
        .zip(&certificates)
        .zip(&committees)
        .enumerate()
    {
        let request = PrivateSettlementLegUploadRequestV1 {
            manifest: final_manifest.clone(),
            audit_policy: material.audit_policy.clone(),
            committee_authority: material.committee_authority.clone(),
            payload: material.payload_with_certificate(certificate.clone()),
        };
        for endpoint in &committee.endpoints {
            let response = sponsor.upload_private_settlement_leg_to_v1(endpoint, &request)?;
            ensure!(
                usize::from(response.leg_ordinal) == ordinal,
                "upload ordinal substitution"
            );
        }
    }
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "collecting")?;
    let state = capture_fault_state_snapshot(&network, "smoke-collecting")?;
    ensure_fault_ledger_unchanged_before_finality(&before, &state)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-collecting.json",
        &state,
    )?);

    for (ordinal, (leg, committee)) in prepared.iter().zip(&committees).enumerate() {
        let auditor_transport_signer =
            BorrowedKeyPairIdentityRequestSignerV1::new(&leg.governed.auditor_signing);
        let capsule_request = PrivateSettlementAuditorCapsuleRequestV1 {
            audit_policy: leg.governed.policy.clone(),
        };
        let fetched = sponsor.private_settlement_auditor_capsule_quorum_for_authority_v1(
            &committee.endpoints,
            &materials[ordinal].committee_authority,
            final_manifest.legs[ordinal].payload_digest,
            &capsule_request,
            &auditor_transport_signer,
        )?;
        ensure!(
            fetched.lifecycle == PrivateSettlementLifecycleDtoV1::Collecting,
            "unexpected audit lifecycle"
        );
        let authoritative_height = fetched.authoritative_height;
        let view = PrivateSettlementAuditorSidecarViewV1 {
            manifest: fetched.manifest,
            policy: fetched.audit_policy,
            authority: fetched.committee_authority,
            statement: fetched.statement,
            delta: fetched.delta,
            audit_capsule: fetched.audit_capsule,
            availability: fetched.availability,
            lifecycle: PrivateSettlementSidecarLifecycleV1::Collecting,
        };
        let auditor_id = AccountId::new(leg.governed.auditor_signing.public_key().clone());
        let approval = approve_private_settlement_leg_v1(
            &view,
            &leg.governed.governance,
            authoritative_height,
            &auditor_id,
            leg.governed.auditor_encryption.secret(),
            &leg.governed.auditor_signing,
            &approve_all_audit_material,
        )?;
        let response = sponsor.submit_private_settlement_audit_approval_quorum_for_authority_v1(
            &committee.endpoints,
            &materials[ordinal].committee_authority,
            final_manifest.legs[ordinal].payload_digest,
            &auditor_transport_signer,
            &PrivateSettlementAuditApprovalRequestV1 {
                audit_policy: capsule_request.audit_policy,
                approval,
            },
        )?;
        ensure!(
            response.lifecycle == PrivateSettlementLifecycleDtoV1::Audited,
            "approval quorum was not durable"
        );
    }
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "audited")?;
    let state = capture_fault_state_snapshot(&network, "smoke-audited")?;
    ensure_fault_ledger_unchanged_before_finality(&before, &state)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-audited.json",
        &state,
    )?);

    let endpoint_matrix = committees
        .iter()
        .map(|committee| committee.endpoints.clone())
        .collect::<Vec<_>>();
    let authorities = committees
        .iter()
        .map(|committee| committee.authority.clone())
        .collect::<Vec<_>>();
    let deltas = prepared
        .iter()
        .map(|leg| leg.prepared.delta.clone())
        .collect::<Vec<_>>();
    let barrier = sponsor.prepare_private_settlement_bundle_v1(
        &endpoint_matrix,
        &final_manifest,
        &authorities,
        &deltas,
    )?;
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "prepared")?;
    let state = capture_fault_state_snapshot(&network, "smoke-prepared")?;
    ensure_fault_ledger_unchanged_before_finality(&before, &state)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-prepared.json",
        &state,
    )?);
    let fee_before_registration = sponsor_nexus_fee_balance(&sponsor)?;
    sponsor.register_private_settlement_prepare_and_wait_v1(
        &barrier,
        u64::try_from(PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1)
            .expect("V1 carrier ceiling fits u64"),
        iroha::client::TransactionWaitOptions {
            timeout: FINALITY_TIMEOUT,
            poll_interval: POLL_INTERVAL,
        },
    )?;
    let fee_after_registration = sponsor_nexus_fee_balance(&sponsor)?;
    ensure_exact_private_settlement_carrier_fee(
        &fee_before_registration,
        &fee_after_registration,
        "Prepare registration",
    )?;
    let registered = capture_fault_state_snapshot(&network, "smoke-registered")?;
    ensure_fault_ledger_unchanged_before_finality(&before, &registered)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-registered.json",
        &registered,
    )?);
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "prepare-barrier.json",
        &barrier,
    )?);
    let commits =
        sponsor.recover_or_commit_private_settlement_bundle_v1(&endpoint_matrix, &barrier)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "commit-certificates.json",
        &commits,
    )?);
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "commit-certified")?;
    let state = capture_fault_state_snapshot(&network, "smoke-commit-certified")?;
    ensure_fault_ledger_unchanged_before_finality(&before, &state)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-commit-certified.json",
        &state,
    )?);

    let request = sponsor.build_private_settlement_finalization_request_v1(
        &barrier,
        &commits,
        u64::try_from(PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1)
            .expect("V1 carrier ceiling fits u64"),
    )?;
    let fee_before_finalization = sponsor_nexus_fee_balance(&sponsor)?;
    observer.begin_phase("finalization", &[], true)?;
    observer.checkpoint_active_phase(&[])?;
    sponsor.submit_private_settlement_bundle_v1(&request)?;
    let receipt = wait_for_identical_receipt(&network, final_manifest.bundle_id)?;
    let fee_after_finalization = sponsor_nexus_fee_balance(&sponsor)?;
    ensure_exact_private_settlement_carrier_fee(
        &fee_before_finalization,
        &fee_after_finalization,
        "financial finalization",
    )?;
    ensure!(
        receipt.legs.len() == PARTICIPANT_COUNT,
        "receipt does not contain exactly three legs"
    );
    for (ordinal, leg) in receipt.legs.iter().enumerate() {
        ensure!(
            usize::from(leg.delta.leg_ordinal) == ordinal,
            "receipt reordered a leg"
        );
        ensure!(
            receipt
                .legs
                .iter()
                .filter(|candidate| candidate.delta.route == leg.delta.route)
                .count()
                == 1,
            "a private leg became visible more than once"
        );
    }
    let after = wait_for_converged_fault_state_snapshot(&network, "smoke-finalized")?;
    ensure_fault_state_finalized_once(&before, &after, shape.participants)?;
    observer.complete_phase()?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-finalized.json",
        &after,
    )?);
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "receipt.json",
        &receipt,
    )?);
    let (finality, files) = collect_signed_rs16_finality(
        &network,
        receipt.finalized_height,
        Some((&evidence_root, "finality-before")),
    )?;
    evidence_files.extend(files);
    ensure!(
        finality.observations == u64::try_from(shape.process_count())?,
        "positive smoke lacks a signed RS16 finality observation from every process"
    );
    ensure!(
        sponsor
            .submit_private_settlement_bundle_v1(&request)
            .is_err(),
        "replaying the exact finalized carrier was accepted"
    );
    ensure!(
        sponsor_nexus_fee_balance(&sponsor)? == fee_after_finalization,
        "rejected finalization replay charged a third carrier fee"
    );
    ensure!(
        wait_for_identical_receipt(&network, final_manifest.bundle_id)? == receipt,
        "replay changed the terminal receipt"
    );
    let replayed = wait_for_converged_fault_state_snapshot(&network, "smoke-replay")?;
    ensure_fault_state_reverted(&after, &replayed)?;
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "state-replay.json",
        &replayed,
    )?);
    let (summaries, observations) = observer.finish_with_evidence(&replayed)?;
    ensure!(
        summaries.len() == shape.process_count()
            && observations.len() == shape.process_count()
            && summaries.iter().all(|row| row.check_count >= 3
                && row.finalized_observations > 0
                && row.poll_failure_count == 0),
        "smoke continuous observer omitted validators, finality or successful polling"
    );
    for (index, (summary, observations)) in summaries.iter().zip(&observations).enumerate() {
        evidence_files.push(write_smoke_evidence(
            &evidence_root,
            &format!("continuous-{index:02}.json"),
            &SmokeContinuousEvidenceV1 {
                summary: summary.clone(),
                observations: observations.clone(),
            },
        )?);
    }
    let mut restarts = Vec::new();

    // Recover each durable store while preserving a live 3-of-4 quorum in
    // every committee. A receipt alone would miss duplicated nullifiers,
    // outputs, or residual reservations, so recheck the complete APS state.
    for (peer_index, peer) in network.all_peers().enumerate() {
        let before_pid = runtime
            .block_on(peer.process_id())
            .ok_or_else(|| eyre!("smoke restart target #{peer_index} has no live PID"))?;
        let config_layers = network.config_layers_for_peer(peer).collect::<Vec<_>>();
        ensure!(
            runtime.block_on(peer.shutdown_if_started())
                && runtime.block_on(peer.process_id()).is_none(),
            "smoke restart target #{peer_index} did not stop"
        );
        runtime
            .block_on(async {
                tokio::time::timeout(
                    FINALITY_TIMEOUT,
                    peer.start_checked(config_layers.iter(), None),
                )
                .await
            })
            .wrap_err_with(|| format!("smoke restart target #{peer_index} timed out"))??;
        let after_pid = runtime
            .block_on(peer.process_id())
            .ok_or_else(|| eyre!("smoke restart target #{peer_index} did not recover"))?;
        ensure!(
            before_pid != after_pid && peer.client().get_status().is_ok(),
            "smoke restart target #{peer_index} lacks a healthy replacement process"
        );
        ensure!(
            wait_for_identical_receipt(&network, final_manifest.bundle_id)? == receipt,
            "smoke restart target #{peer_index} changed the finalized receipt"
        );
        let recovered = wait_for_converged_fault_state_snapshot(&network, "smoke-restarted")?;
        ensure_fault_state_reverted(&after, &recovered)?;
        evidence_files.push(write_smoke_evidence(
            &evidence_root,
            &format!("state-restarted-{peer_index:02}.json"),
            &recovered,
        )?);
        restarts.push(SmokeRestartV1 {
            peer_index,
            before_pid,
            after_pid,
        });
        println!(
            "APS smoke restart verified: peer_index={peer_index} before_pid={before_pid} after_pid={after_pid}"
        );
    }
    let (recovered_finality, files) = collect_signed_rs16_finality(
        &network,
        receipt.finalized_height,
        Some((&evidence_root, "finality-after")),
    )?;
    ensure!(
        recovered_finality == finality,
        "restarted smoke network changed its finalized block, authority context, or coverage"
    );
    evidence_files.extend(files);
    let recovered_inventory = smoke_process_inventory(&network, &runtime, shape)?;
    ensure!(
        initial_inventory
            .iter()
            .zip(&recovered_inventory)
            .all(|(before, after)| before.peer_id == after.peer_id
                && before.configuration_sha256 == after.configuration_sha256
                && before.pid != after.pid),
        "smoke restart changed identity/configuration or retained its process"
    );
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "processes-after.json",
        &recovered_inventory,
    )?);
    evidence_files.push(write_smoke_evidence(
        &evidence_root,
        "restarts.json",
        &restarts,
    )?);
    write_real_process_result(&RealProcessSmokeResultV1 {
        version: 1,
        protocol: "AtomicPrivateSettlementV1".to_owned(),
        kind: "smoke".to_owned(),
        request: smoke_request,
        request_sha256: request_sha,
        network_id: norito::json::to_value(&network.network_id())?,
        participants: shape.participants,
        processes: shape.process_count(),
        restarted: restarts.len(),
        activation_height: activated_height,
        authority_context_height,
        finalized_height: receipt.finalized_height,
        signed_rs16_observations: finality.observations,
        continuous_checks: summaries.iter().map(|row| row.check_count).sum::<u64>(),
        passed: true,
        artifacts: evidence_files,
    })?;
    println!(
        "APS smoke completed: participants={} processes={} restarted={} finalized_height={}",
        shape.participants,
        shape.process_count(),
        shape.process_count(),
        receipt.finalized_height,
    );
    Ok(())
}

#[test]
#[ignore = "release-only: starts 16 real validators and generates three native STARK proofs"]
fn atomic_private_settlement_n3_real_process_smoke() -> Result<()> {
    let handle = thread::Builder::new()
        .name("atomic-private-settlement-n3".to_owned())
        .stack_size(TEST_STACK_BYTES)
        .spawn(run_n3_real_process_smoke)
        .expect("spawn release smoke thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[test]
fn release_fee_intent_is_bounded_and_uses_canonical_nexus_xor() {
    let intent = bounded_nexus_fee();
    intent.validate().expect("release fee intent is canonical");
    let [limit] = intent.charge_limits() else {
        panic!("release fee intent must contain exactly one Nexus charge limit");
    };
    assert_eq!(limit.kind(), FeeChargeKind::Nexus);
    assert_eq!(
        limit.asset_definition_id(),
        &nexus_fee_asset_definition_id()
    );
    assert_eq!(
        limit.max_amount(),
        &Quantity::from(NEXUS_FEE_SIGNED_MAXIMUM)
    );
}

#[test]
fn release_sources_do_not_construct_fee_free_non_genesis_transactions() {
    let forbidden_constructor = ["FeePaymentIntent::authority(", "Vec::new(), None)"].concat();
    let retired_helper = ["no_", "fee()"].concat();
    for (name, source) in [
        (
            "localnet",
            include_str!("atomic_private_settlement_localnet.rs"),
        ),
        (
            "release harness",
            include_str!("atomic_private_settlement_real_process_harness.rs"),
        ),
    ] {
        assert!(
            !source.contains(&forbidden_constructor),
            "{name} constructs a fee-free non-genesis intent"
        );
        assert!(
            !source.contains(&retired_helper),
            "{name} calls the retired fee-free helper"
        );
    }
    let client_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/iroha/src/client/private_settlement.rs"
    ));
    assert!(
        client_source.contains("expected_manifest.public_fee_intent.clone(),"),
        "Prepare registration must carry the manifest's bounded public fee intent"
    );
}

#[test]
fn genesis_registers_only_participant_processes_as_committee_peers() {
    let shape = TopologyShape::new(PARTICIPANT_COUNT);
    let process_entries = (0..shape.process_count())
        .map(|index| {
            let seed = u8::try_from(index + 1).expect("fixture process index fits u8");
            let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS process key");
            let pop = iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                .expect("derive process PoP");
            GenesisTopologyEntry::new(PeerId::new(keypair.public_key().clone()), pop)
        })
        .collect::<Vec<_>>();
    let topology = process_entries
        .iter()
        .map(|entry| entry.peer.clone())
        .collect::<Vec<_>>();
    let committee_validator_entries = process_entries[shape.global_validator_count()..].to_vec();

    let transactions = genesis_post_topology(shape, &topology, &committee_validator_entries);
    let registrations = transactions
        .iter()
        .flatten()
        .filter_map(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterCommitteePeerWithPop>()
        })
        .collect::<Vec<_>>();

    assert_eq!(registrations.len(), shape.participant_validator_count());
    for (registration, entry) in registrations.iter().zip(&committee_validator_entries) {
        assert_eq!(registration.peer, entry.peer);
        assert_eq!(
            registration.pop,
            entry
                .pop_bytes()
                .expect("fixture PoP hex")
                .expect("fixture carries PoP")
        );
    }
    assert!(registrations.iter().all(|registration| {
        !topology[..shape.global_validator_count()].contains(&registration.peer)
    }));
}

#[test]
fn genesis_ivm_private_note_activation_is_exact() {
    assert_eq!(
        PRIVACY_PROFILE_ACTIVATION_HEIGHT,
        PRIVACY_GENESIS_PROPOSAL_HEIGHT + PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
        "compiled private-note governance delay determines profile activation"
    );
    assert_eq!(
        PRIVATE_SETTLEMENT_ACTIVATION_HEIGHT,
        PRIVACY_PROFILE_ACTIVATION_HEIGHT.max(PRIVATE_SETTLEMENT_NOTICE_ACTIVATION_HEIGHT),
        "APS must activate at the earliest height satisfying both profile and notice schedules"
    );
    assert!(
        PRIVATE_SETTLEMENT_ACTIVATION_HEIGHT >= PRIVACY_PROFILE_ACTIVATION_HEIGHT
            && PRIVATE_SETTLEMENT_ACTIVATION_HEIGHT
                >= PRIVACY_GENESIS_PROPOSAL_HEIGHT
                    + PRIVATE_SETTLEMENT_MINIMUM_ACTIVATION_NOTICE_BLOCKS,
        "APS activation must not precede either prerequisite"
    );
    let shape = TopologyShape::new(PARTICIPANT_COUNT);
    let topology = (0..shape.process_count())
        .map(|index| PeerId::new(validator_authority_keypair(index).public_key().clone()))
        .collect::<Vec<_>>();
    let committee_validator_entries = topology[shape.global_validator_count()..]
        .iter()
        .cloned()
        .map(|peer| GenesisTopologyEntry::new(peer, vec![1]))
        .collect::<Vec<_>>();
    let transactions = genesis_post_topology(shape, &topology, &committee_validator_entries);
    let governance_permission = Permission::from(CanEnactGovernance);
    let (governance_transaction, governance_instruction) = transactions
        .iter()
        .enumerate()
        .find_map(|(transaction_index, transaction)| {
            transaction
                .iter()
                .position(|instruction| {
                    matches!(
                        instruction.as_any().downcast_ref::<GrantBox>(),
                        Some(GrantBox::Permission(grant))
                            if grant.destination == ALICE_ID.clone()
                                && grant.object == governance_permission
                    )
                })
                .map(|instruction_index| (transaction_index, instruction_index))
        })
        .expect("genesis grants the proposal authority governance permission");
    let activations = transactions
        .iter()
        .enumerate()
        .flat_map(|(transaction_index, transaction)| {
            transaction
                .iter()
                .enumerate()
                .filter_map(move |(instruction_index, instruction)| {
                    instruction
                        .as_any()
                        .downcast_ref::<RegisterPrivacyProtocolActivationV1>()
                        .map(|registration| (transaction_index, instruction_index, registration))
                })
        })
        .collect::<Vec<_>>();
    let [(activation_transaction, activation_instruction, registration)] = activations.as_slice()
    else {
        panic!(
            "genesis must contain exactly one IVM private-note activation, found {}",
            activations.len()
        );
    };
    assert_eq!(
        *activation_transaction, governance_transaction,
        "genesis grant and governed activation must be one atomic transaction"
    );
    assert!(
        *activation_instruction > governance_instruction,
        "governed activation must follow its permission grant"
    );
    assert_eq!(registration.activation, genesis_private_note_activation());
    assert_eq!(
        registration.activation.lifecycle,
        PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
            proposed_at_height: PRIVACY_GENESIS_PROPOSAL_HEIGHT,
            activate_at_height: PRIVACY_PROFILE_ACTIVATION_HEIGHT,
        })
    );
}

#[test]
fn repeat_bundle_private_material_is_reproducible_and_disjoint() {
    let routes = (0..PARTICIPANT_COUNT)
        .map(|ordinal| PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::new(
                u64::try_from(ordinal + 1).expect("fixture dataspace ordinal fits u64"),
            ),
            lane_id: LaneId::new(
                u32::try_from(ordinal + 1).expect("fixture lane ordinal fits u32"),
            ),
            lane_incarnation: hash(0xE0 + ordinal as u8),
        })
        .collect::<Vec<_>>();
    let network_id = iroha::data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(hash(0xF8)),
    );
    let first_authority_height = PRIVATE_SETTLEMENT_ACTIVATION_HEIGHT + 10;
    let first_expiry_height = first_authority_height + 100;
    let first_governed = governed_legs(&routes, first_authority_height, first_expiry_height)
        .expect("first deterministic governance set");
    let first_manifest = proof_manifest(
        network_id,
        first_authority_height,
        first_expiry_height,
        &first_governed,
    )
    .expect("first deterministic manifest");
    let second_authority_height = first_authority_height + 1;
    let second_expiry_height = first_expiry_height + 1;
    let second_governed = governed_legs(&routes, second_authority_height, second_expiry_height)
        .expect("second deterministic governance set");
    let second_manifest = proof_manifest(
        network_id,
        second_authority_height,
        second_expiry_height,
        &second_governed,
    )
    .expect("second deterministic manifest");
    assert_ne!(first_manifest.bundle_id, second_manifest.bundle_id);

    let material_shapes: &[(&[u8], usize)] = &[
        (b"output-encryption-rng", 1),
        (b"audit-capsule-rng", 1),
        (b"input-spending-secret", 2),
        (b"output-spending-secret", 3),
        (b"output-view-secret", 3),
        (b"output-ephemeral-secret", 3),
        (b"input-note", 12),
        (b"output-note", 18),
    ];
    let expected_materials_per_leg = material_shapes
        .iter()
        .map(|(_, count)| count)
        .sum::<usize>()
        + 1;
    let mut materials = std::collections::BTreeSet::<[u8; 32]>::new();
    let mut recipient_ids = std::collections::BTreeSet::<PrivacyRecipientIdV1>::new();
    for (bundle_ordinal, (manifest, governed)) in [
        (&first_manifest, first_governed.as_slice()),
        (&second_manifest, second_governed.as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        for (leg_ordinal, leg) in governed.iter().enumerate() {
            assert!(
                materials.insert(private_settlement_reimbursement_terms_salt(manifest, leg,)),
                "bundle {bundle_ordinal} leg {leg_ordinal} reused its reimbursement salt"
            );
            for (purpose, count) in material_shapes {
                for material_ordinal in 0..*count {
                    let material = private_settlement_leg_private_material(
                        manifest,
                        leg,
                        leg_ordinal,
                        material_ordinal,
                        purpose,
                    );
                    assert_eq!(
                        material,
                        private_settlement_leg_private_material(
                            manifest,
                            leg,
                            leg_ordinal,
                            material_ordinal,
                            purpose,
                        ),
                        "bundle-private derivation must be reproducible"
                    );
                    assert!(
                        materials.insert(material),
                        "bundle {bundle_ordinal} leg {leg_ordinal} reused {purpose:?} slot {material_ordinal}"
                    );
                }
            }
            for output_ordinal in 0..3 {
                let view_secret = private_settlement_leg_private_material(
                    manifest,
                    leg,
                    leg_ordinal,
                    output_ordinal,
                    b"output-view-secret",
                );
                let view_public = ivm_private_recipient_public_key_v1(&view_secret)
                    .expect("derived view secret is valid");
                let recipient_id = derive_ivm_private_recipient_id_v1(view_public)
                    .expect("derived view public key is valid");
                assert!(
                    recipient_ids.insert(recipient_id),
                    "repeat bundles must not reuse an encrypted-output recipient id"
                );
            }
        }
    }
    assert_eq!(
        materials.len(),
        2 * PARTICIPANT_COUNT * expected_materials_per_leg
    );
    assert_eq!(recipient_ids.len(), 2 * PARTICIPANT_COUNT * 3);
}

#[test]
fn n3_correctness_smoke_retains_the_release_network_cadence() {
    let builder = n3_smoke_builder(TopologyShape::new(PARTICIPANT_COUNT));
    assert_eq!(
        builder.configured_block_cadence(),
        Some(Duration::from_secs(4))
    );
}

#[test]
fn n3_topology_has_one_global_and_three_disjoint_four_validator_committees() {
    let shape = TopologyShape::new(3);
    assert_eq!(shape.lane_count(), 4);
    assert_eq!(shape.global_validator_count(), 4);
    assert_eq!(shape.participant_validator_count(), 12);
    assert_eq!(shape.process_count(), 16);
    assert_eq!(shape.committee_range(0), 0..4);
    assert_eq!(shape.committee_range(1), 4..8);
    assert_eq!(shape.committee_range(2), 8..12);
    assert_eq!(shape.committee_range(3), 12..16);
}

#[test]
fn n3_primary_topology_mixes_public_and_permissioned_participant_dataspaces() {
    let shape = TopologyShape::new(PARTICIPANT_COUNT);
    assert_eq!(
        shape.participant_visibility_profile(),
        [
            LaneVisibility::Public,
            LaneVisibility::Restricted,
            LaneVisibility::Restricted,
        ]
    );
    assert_eq!(participant_dataspace_alias(0), "public-1");
    assert_eq!(participant_dataspace_alias(1), "private-2");
    assert_eq!(participant_dataspace_alias(2), "private-3");
    assert_eq!(shape.p2p_process_counts_by_visibility(), (8, 8));
}

#[test]
fn release_matrix_shapes_are_disjoint_and_exact() {
    for participants in [2, 3, 4, 8, 16] {
        let shape = TopologyShape::new(participants);
        shape.validate().expect("supported release shape");
        let ranges = (0..shape.lane_count())
            .map(|lane| shape.committee_range(lane))
            .collect::<Vec<_>>();
        assert_eq!(ranges.first().expect("global range").start, 0);
        assert_eq!(
            ranges.last().expect("last range").end,
            shape.process_count()
        );
        assert!(ranges.windows(2).all(|pair| pair[0].end == pair[1].start));
        assert!(
            ranges
                .iter()
                .all(|range| range.len() == VALIDATORS_PER_LANE)
        );
    }
}

#[test]
fn unsupported_real_process_participant_count_fails_closed() {
    assert!(TopologyShape::new(1).validate().is_err());
    assert!(TopologyShape::new(5).validate().is_err());
    assert!(TopologyShape::new(17).validate().is_err());
    assert_eq!(GLOBAL_LANE_ID, 0);
}

#[test]
fn leakage_canary_identifiers_are_canonical_typed_values() {
    let left_account = leakage_canary_account_id("left").expect("left canary account");
    let right_account = leakage_canary_account_id("right").expect("right canary account");
    assert_eq!(left_account.to_string(), LEAKAGE_ACCOUNT_LEFT_I105);
    assert_eq!(right_account.to_string(), LEAKAGE_ACCOUNT_RIGHT_I105);
    assert_ne!(left_account, right_account);

    let left_asset =
        leakage_canary_asset_definition_id("left").expect("left canary asset definition");
    let right_asset =
        leakage_canary_asset_definition_id("right").expect("right canary asset definition");
    assert_eq!(left_asset.to_string(), LEAKAGE_ASSET_LEFT);
    assert_eq!(right_asset.to_string(), LEAKAGE_ASSET_RIGHT);
    assert_ne!(left_asset, right_asset);
    assert!(leakage_canary_account_id("unknown").is_err());
    assert!(leakage_canary_asset_definition_id("unknown").is_err());
}

include!("atomic_private_settlement_real_process_harness.rs");
