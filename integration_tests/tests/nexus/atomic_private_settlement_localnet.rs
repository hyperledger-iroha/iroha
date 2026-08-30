#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Release-only real-process tests for atomic private settlement.
//!
//! The ignored test deliberately uses the production wallet prover, encrypted
//! auditor capsule, Torii restricted-DA routes, and node-held BLS committee
//! keys.  There is no fixture proof, hand-made vote, or QC verification bypass.
//! The included release-harness entrypoint parameterizes the same production
//! workflow across N=2,3,4,8,16 and publishes only measured process evidence.

use super::localnet_npos::npos_override_instruction;
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{
        Client, PrivateSettlementAuditApprovalRequestV1, PrivateSettlementBundleReceiptResponseV1,
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
            Grant, InstructionBox, Log, Mint, Register,
            privacy::RegisterPrivacyProtocolActivationV1,
            private_settlement::{
                ActivatePrivateSettlementPoolV1, FinalizeAtomicPrivateSettlementV1,
            },
            settlement::{
                DvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
                SettlementPlan,
            },
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::{
            ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1, DataSpaceId, LaneId,
            PrivateSettlementAuditAadV1, PrivateSettlementAuditEncryptionOpeningV1,
            PrivateSettlementAuditNoteOpeningV1, PrivateSettlementAuditOutputRoleV1,
            PrivateSettlementAuditOutputV1, PrivateSettlementAuditPayerAuthorizationBodyV1,
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
            PrivateSettlementSidecarAvailabilityBodyV1,
        },
        peer::PeerId,
        permission::Permission,
        prelude::{FindAssetById, FindAssets, FindPermissionsByAccountId},
        privacy::{
            PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1, PrivacyCommitmentV1,
            PrivacyEncryptedOutputV1, PrivacyEncryptionKeyV1, PrivacyNullifierV1, PrivacyPoolIdV1,
            PrivacyProposedLifecycleV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
            PrivacyRecipientIdV1, PrivacyRootV1,
        },
        query::block::prelude::FindBlocks,
        transaction::{FeePaymentIntent, SignedTransaction, TransactionEntrypoint},
    },
};
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_engines::{
        atomic_private_settlement::{
            consume_atomic_private_settlement_wallet_bundle_v1,
            derive_atomic_private_settlement_input_nullifiers_v1,
            encode_atomic_private_settlement_wallet_bundle_v1,
            plan_atomic_private_settlement_bootstrap_v1,
            prepare_atomic_private_settlement_input_openings_v1,
            prepare_atomic_private_settlement_outputs_v1,
        },
        ivm_private_note::{derive_note_authority_v1, ivm_private_recipient_public_key_v1},
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
use iroha_primitives::numeric::Quantity;
use iroha_test_network::{
    Network, NetworkBuilder, NetworkPeer, unexecuted_genesis_factory_with_post_topology,
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
const VALIDATORS_PER_LANE: usize = 4;
const GLOBAL_LANE_ID: u32 = 0;
const VALIDATOR_STAKE: u64 = 2_000;
const PRIVATE_SETTLEMENT_ACTIVATION_HEIGHT: u64 = 2;
const MAX_EXPIRY_BLOCKS: u64 = 4_096;
const SIDECAR_RETENTION_BLOCKS: u64 = 4_096;
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;
const TRANSPARENT_CONTROL_SEED_BALANCE: u64 = 10_000;
const TRANSPARENT_CONTROL_OUTPUT_BASELINE: u64 = 1;
const TEST_STACK_BYTES: usize = 64 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const FINALITY_TIMEOUT: Duration = Duration::from_secs(300);

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

    const fn peer_count(self) -> usize {
        self.lane_count() * VALIDATORS_PER_LANE
    }

    fn validator_range(self, lane: usize) -> Range<usize> {
        let start = lane * VALIDATORS_PER_LANE;
        start..start + VALIDATORS_PER_LANE
    }

    fn validate(self) -> Result<()> {
        ensure!(
            matches!(self.participants, 2 | 3 | 4 | 8 | 16),
            "real-process release matrix supports N=2,3,4,8,16"
        );
        Ok(())
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
    statement: PrivateSettlementProofStatementV1,
    proof: Vec<u8>,
    delta: PrivateSettlementDeltaV1,
    capsule: iroha::data_model::nexus::PrivateSettlementAuditCapsuleV1,
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

fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
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
        format!("private-{}", ordinal + 1),
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

fn genesis_post_topology(shape: TopologyShape, topology: &[PeerId]) -> Vec<Vec<InstructionBox>> {
    assert_eq!(topology.len(), shape.peer_count());
    let stake_definition = stake_asset_definition_id();
    let mut universal = vec![
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
        Grant::account_permission(Permission::from(CanEnactGovernance), ALICE_ID.clone()).into(),
    ];
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
    NetworkBuilder::new()
        .with_base_seed("atomic-private-settlement-n3-real-process-v1")
        .with_peers(shape.peer_count())
        .with_block_cadence(Duration::from_millis(50))
        .with_peer_startup_timeout(Duration::from_secs(20 * 60))
        .with_npos_consensus()
        .without_npos_genesis_bootstrap()
        .with_genesis_block(move |topology, topology_entries| {
            unexecuted_genesis_factory_with_post_topology(
                Vec::new(),
                genesis_post_topology(shape, topology.as_ref()),
                topology,
                topology_entries,
            )
        })
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
                            format!("lane-private-{lane}")
                        }),
                    );
                    table.insert(
                        "dataspace".into(),
                        TomlValue::String(if lane == 0 {
                            "universal".to_owned()
                        } else {
                            format!("private-{lane}")
                        }),
                    );
                    table.insert(
                        "visibility".into(),
                        TomlValue::String(if lane == 0 { "public" } else { "restricted" }.into()),
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
                            format!("private-{dataspace}")
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
                        TomlValue::String(format!("private-{}", ordinal + 1)),
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
                .write(["nexus", "lane_count"], shape.lane_count() as i64)
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                )
                .write(["nexus", "lane_catalog"], TomlValue::Array(lanes))
                .write(["nexus", "dataspace_catalog"], TomlValue::Array(dataspaces))
                .write(["nexus", "routing_policy"], TomlValue::Table(routing))
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
                    1_i64,
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

fn routes_from_network(
    network: &Network,
    shape: TopologyShape,
) -> Result<Vec<PrivateSettlementRouteV1>> {
    let status = network.client().get_lane_lifecycle_status()?;
    status
        .validate()
        .wrap_err("validate lane lifecycle status")?;
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
            let mut rows = network.peers()[shape.validator_range(lane)]
                .iter()
                .map(|peer: &NetworkPeer| {
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
    let current = client.get_privacy_capabilities()?.committed_height;
    let proposed_at = current
        .checked_add(1)
        .ok_or_else(|| eyre!("height overflow"))?;
    let activate_at = proposed_at
        .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
        .ok_or_else(|| eyre!("activation height overflow"))?;
    let activation = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)?
        .activation_record(PrivacyProtocolLifecycleV1::Proposed(
            PrivacyProposedLifecycleV1 {
                proposed_at_height: proposed_at,
                activate_at_height: activate_at,
            },
        ));
    let transaction = client.build_transaction(
        [InstructionBox::from(
            RegisterPrivacyProtocolActivationV1::new(activation),
        )],
        no_fee(),
        Metadata::default(),
    );
    client
        .submit_transaction_blocking(&transaction)
        .wrap_err("register governed IVM private-note activation")?;
    loop {
        let capability = client.get_privacy_capabilities()?;
        let row = capability
            .protocols
            .iter()
            .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
            .ok_or_else(|| eyre!("IVM private-note capability row is absent"))?;
        if matches!(
            row.activation,
            Some(activation)
                if matches!(activation.lifecycle, PrivacyProtocolLifecycleV1::Active(_))
        ) {
            ensure!(
                row.is_network_available(),
                "active IVM profile is not network-available"
            );
            return Ok(capability.committed_height);
        }
        let tick = client.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                format!(
                    "atomic-private-settlement activation tick {}",
                    capability.committed_height
                ),
            ))],
            no_fee(),
            Metadata::default(),
        );
        client.submit_transaction_blocking(&tick)?;
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

fn note_opening(seed: u8, active: bool, value: u128) -> PrivateSettlementAuditNoteOpeningV1 {
    PrivateSettlementAuditNoteOpeningV1 {
        active,
        commitment: PrivacyCommitmentV1::new(bytes(seed)),
        value,
        spending_authority: bytes(seed.wrapping_add(1)),
        rho: bytes(seed.wrapping_add(2)),
        blinding: bytes(seed.wrapping_add(3)),
        memo_digest: bytes(seed.wrapping_add(4)),
        dummy_domain: (!active).then(|| hash(seed.wrapping_add(5))),
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
        reimbursement_terms_salt: bytes(0x94),
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
        public_fee_intent: no_fee(),
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
    let mut output_rng = iroha_crypto::rng_from_seed_slice(&bytes(0xF1 + ordinal as u8));
    let mut capsule_rng = iroha_crypto::rng_from_seed_slice(&bytes(0xF4 + ordinal as u8));
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
        old_epoch: 1,
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
        bytes(0xC1 + ordinal as u8 * 2),
        bytes(0xC2 + ordinal as u8 * 2),
    ];
    let output_secrets = [
        bytes(0xD1 + ordinal as u8 * 3),
        bytes(0xD2 + ordinal as u8 * 3),
        bytes(0xD3 + ordinal as u8 * 3),
    ];
    let view_secrets = [
        bytes(0x61 + ordinal as u8 * 3),
        bytes(0x62 + ordinal as u8 * 3),
        bytes(0x63 + ordinal as u8 * 3),
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
        reimbursement_terms_salt: bytes(0x94),
        memo: private_data.memo.clone(),
        policy_references: vec![governed.governance.governance_digest],
        inputs: vec![
            note_opening(0x70 + ordinal as u8 * 5, true, input_amount),
            note_opening(0x71 + ordinal as u8 * 5, false, 0),
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
                    ephemeral_secret: bytes(0xE1 + ordinal as u8 * 3),
                },
                note: note_opening(0x80 + ordinal as u8 * 4, true, amount),
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
                    ephemeral_secret: bytes(0xE2 + ordinal as u8 * 3),
                },
                note: note_opening(0x81 + ordinal as u8 * 4, true, change),
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
                    ephemeral_secret: bytes(0xE3 + ordinal as u8 * 3),
                },
                note: note_opening(0x82 + ordinal as u8 * 4, ordinal == 0, reimbursement),
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
        &statement,
        [
            plaintext.inputs[0].commitment,
            plaintext.inputs[1].commitment,
        ],
        input_secrets,
    )?;
    statement.old_root = bootstrap.old_root;
    let new_root = bootstrap.new_root;
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
    let delta = PrivateSettlementDeltaV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        bundle_id: manifest.bundle_id,
        leg_ordinal: ordinal as u8,
        route: governed.route,
        pool_id: governed.governance.body.pool_id,
        asset_binding_commitment: governed.governance.body.asset_binding_commitment,
        old_root: statement.old_root,
        new_root,
        old_epoch: 1,
        new_epoch: 2,
        nullifiers: statement.nullifiers.clone(),
        output_commitments: statement.output_commitments.clone(),
        encrypted_outputs: statement.encrypted_outputs.clone(),
        statement_digest: statement.digest()?,
        proof_digest: iroha::data_model::nexus::private_settlement_proof_digest_v1(&prepared.proof),
        capsule_digest: capsule.digest()?,
        audit_policy_digest: governed.policy.policy_digest,
        audit_key_epoch: governed.policy.body.key_epoch,
    };
    delta.validate_against(&statement)?;
    Ok(PreparedLeg {
        governed,
        statement,
        proof: prepared.proof,
        delta,
        capsule,
        initial_commitments,
    })
}

fn provisional_materials(
    mut manifest: AtomicPrivateSettlementV1,
    prepared: &[PreparedLeg],
    committees: &[CommitteeEndpoints],
) -> Result<Vec<PrivateSettlementProvisionalLegMaterialV1>> {
    for leg in &mut manifest.legs {
        leg.availability_certificate_digest = zero_hash();
    }
    let mut materials = prepared
        .iter()
        .zip(committees)
        .enumerate()
        .map(|(ordinal, (leg, committee))| {
            Ok(PrivateSettlementProvisionalLegMaterialV1 {
                version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                manifest: manifest.clone(),
                audit_policy: leg.governed.policy.clone(),
                committee_authority: committee.authority.clone(),
                statement: leg.statement.clone(),
                proof: leg.proof.clone(),
                delta: leg.delta.clone(),
                audit_capsule: leg.capsule.clone(),
                availability_body: PrivateSettlementSidecarAvailabilityBodyV1 {
                    version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                    network_id: manifest.network_id,
                    bundle_id: manifest.bundle_id,
                    leg_ordinal: ordinal as u8,
                    route: leg.governed.route,
                    authority_digest: committee.authority.digest()?,
                    authority_context_height: manifest.authority_context_height,
                    payload_digest: hash(0xF0 + ordinal as u8),
                    payload_bytes: 1,
                    retention_until_height: manifest.authority_context_height
                        + SIDECAR_RETENTION_BLOCKS
                        + 512,
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    for (ordinal, material) in materials.iter_mut().enumerate() {
        let payload_digest = material.payload_digest()?;
        let payload_bytes = u32::try_from(material.sidecar_material_bytes_len()?)?;
        material.availability_body.payload_digest = payload_digest;
        material.availability_body.payload_bytes = payload_bytes;
        manifest.legs[ordinal].payload_digest = payload_digest;
        manifest.legs[ordinal].delta_digest = material.delta.digest()?;
    }
    manifest.validate_provisional()?;
    for material in &mut materials {
        material.manifest = manifest.clone();
        material.validate()?;
    }
    Ok(materials)
}

fn assert_no_partial_visibility(network: &Network, bundle_id: Hash, phase: &str) -> Result<()> {
    for peer in network.peers() {
        match peer
            .client()
            .private_settlement_bundle_receipt_v1(bundle_id)
        {
            Ok(PrivateSettlementBundleReceiptResponseV1::Pending { .. }) | Err(_) => {}
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
        for peer in network.peers() {
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
        if receipts.len() == network.peers().len()
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
    let shape = TopologyShape::new(PARTICIPANT_COUNT);
    shape.validate()?;
    ensure!(
        shape.peer_count() == 16,
        "N=3 requires 4 global + 12 participant validators"
    );
    let context = "atomic_private_settlement_n3_real_process_smoke";
    let started = sandbox::start_network_blocking_or_skip(localnet_builder(shape), context)?;
    let Some((network, _runtime)) = sandbox::enforce_network_start_requirement(started, context)?
    else {
        return Ok(());
    };
    let sponsor = network.client();
    let activated_height = activate_ivm_private_note(&sponsor)?;
    let authority_context_height = activated_height + 1;
    let expiry_height = authority_context_height + 1_000;
    let routes = routes_from_network(&network, shape)?;
    ensure!(
        routes.len() == 3,
        "exactly three private dataspaces are required"
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
        sponsor.build_transaction_from_items(activations, no_fee(), Metadata::default());
    sponsor
        .submit_transaction_blocking(&activation_transaction)
        .wrap_err("activate all three governed private pools at the bound context height")?;
    ensure!(
        sponsor.get_privacy_capabilities()?.committed_height == authority_context_height,
        "pool activation did not land at the manifest authority context"
    );

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

    for (ordinal, (leg, committee)) in prepared.iter().zip(&committees).enumerate() {
        let fetched = sponsor.private_settlement_auditor_capsule_v1(
            final_manifest.legs[ordinal].payload_digest,
            &leg.governed.auditor_signing,
        )?;
        ensure!(
            fetched.lifecycle == PrivateSettlementLifecycleDtoV1::Collecting,
            "unexpected audit lifecycle"
        );
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
            authority_context_height,
            &auditor_id,
            leg.governed.auditor_encryption.secret(),
            &leg.governed.auditor_signing,
            &approve_all_audit_material,
        )?;
        for endpoint in &committee.endpoints {
            let mut endpoint_client = sponsor.clone();
            endpoint_client.torii_url = endpoint.clone();
            let response = endpoint_client.submit_private_settlement_audit_approval_v1(
                final_manifest.legs[ordinal].payload_digest,
                &leg.governed.auditor_signing,
                &PrivateSettlementAuditApprovalRequestV1 {
                    approval: approval.clone(),
                },
            )?;
            ensure!(
                response.lifecycle == PrivateSettlementLifecycleDtoV1::Audited,
                "approval was not durable"
            );
        }
    }
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "audited")?;

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
        .map(|leg| leg.delta.clone())
        .collect::<Vec<_>>();
    let barrier = sponsor.prepare_private_settlement_bundle_v1(
        &endpoint_matrix,
        &final_manifest,
        &authorities,
        &deltas,
    )?;
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "prepared")?;
    let commits = sponsor.commit_private_settlement_bundle_v1(&endpoint_matrix, &barrier)?;
    assert_no_partial_visibility(&network, final_manifest.bundle_id, "commit-certified")?;

    let legs = deltas
        .into_iter()
        .zip(barrier.prepare_certificates)
        .zip(commits)
        .map(|((delta, prepare), commit)| PrivateSettlementLegReceiptV1 {
            delta,
            prepare,
            commit,
        })
        .collect();
    let carrier = FinalizeAtomicPrivateSettlementV1::new(PrivateSettlementCommitBundleV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: final_manifest.clone(),
        authority_catalog: authorities,
        legs,
    });
    let transaction = sponsor.build_transaction(
        [InstructionBox::from(carrier)],
        final_manifest.public_fee_intent.clone(),
        Metadata::default(),
    );
    let request = PrivateSettlementBundleSubmitRequestV1 {
        transaction: transaction.clone(),
    };
    sponsor.submit_private_settlement_bundle_v1(&request)?;
    let receipt = wait_for_identical_receipt(&network, final_manifest.bundle_id)?;
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
    ensure!(
        sponsor
            .submit_private_settlement_bundle_v1(&request)
            .is_err(),
        "replaying the exact finalized carrier was accepted"
    );
    ensure!(
        wait_for_identical_receipt(&network, final_manifest.bundle_id)? == receipt,
        "replay changed the terminal receipt"
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
fn n3_topology_has_one_global_and_three_disjoint_four_validator_committees() {
    let shape = TopologyShape::new(3);
    assert_eq!(shape.lane_count(), 4);
    assert_eq!(shape.peer_count(), 16);
    assert_eq!(shape.validator_range(0), 0..4);
    assert_eq!(shape.validator_range(1), 4..8);
    assert_eq!(shape.validator_range(2), 8..12);
    assert_eq!(shape.validator_range(3), 12..16);
}

#[test]
fn release_matrix_shapes_are_disjoint_and_exact() {
    for participants in [2, 3, 4, 8, 16] {
        let shape = TopologyShape::new(participants);
        shape.validate().expect("supported release shape");
        let ranges = (0..shape.lane_count())
            .map(|lane| shape.validator_range(lane))
            .collect::<Vec<_>>();
        assert_eq!(ranges.first().expect("global range").start, 0);
        assert_eq!(ranges.last().expect("last range").end, shape.peer_count());
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
