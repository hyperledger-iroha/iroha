//! Modeling block transitions.
//!
//! Operations on blocks:
//!
//! 1. Static analysis of the block. This is a _fallible_ operation
//! 2. Execute transactions and time triggers infallibly, recording transaction errors in the block.
//! 3. Voting
//! 4. Pre-commit signatures check
//! 5. Apply & commit
//!
//! Operations 1 + 2 form a process we call _validation_.
//!
//! Block lifecycle stages:
//!
//! 1. A node-created [`NewBlock`] is assumed valid and needs no static validation before [`ValidBlock`].
//! 2. Block is received/deserialized from disk (as [`SignedBlock`]). Such blocks require static
//!    validation before execution to transition to [`ValidBlock`].
//! 3. [`ValidBlock`] pairs with [`crate::state::StateBlock`] containing applied state changes and
//!    transaction errors.
//! 4. Block is committed ([`CommittedBlock`]). Created from [`ValidBlock`] once the consensus core
//!    has certified it; the canonical SignedBlockWire retains the exact native certificate.
//!
//! ### Scenario: a block ordered by the Sumeragi core
//!
//! Flow: the leader builds the payload with [`BlockBuilder`] (`sumeragi::payload`); every node
//! executes the ordered payload with [`ValidBlock::validate_sumeragi_block`] against its
//! committed parent and commits it with [`ValidBlock::commit_unchecked`] after the core's
//! `CommitBlock` (`sumeragi::executor`).
//!
//! ### Scenario: genesis (init or receive)
//!
//! Flow: authenticate the signed genesis handshake mode, call
//! [`ValidBlock::validate_signed_genesis`], then [`ValidBlock::commit`].
//!
//! ### Scenario: plain block execution
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::validate_unchecked`] (infallible),
//! [`ValidBlock::commit_unchecked`] (infallible)

use core::fmt;
use iroha_crypto::{Hash, HashOf, KeyPair, MerkleTree, PublicKey};
#[cfg(test)]
use iroha_data_model::block::consensus::ValidatorIndex;
use iroha_data_model::{
    NetworkId,
    account::{AccountController, AccountId, rekey::AccountAlias},
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    block::{
        consensus::{
            LaneSettlementReceipt,
            },
        *,
    },
    confidential::ConfidentialFeatureDigest,
    consensus::{ConsensusKeyRole, NposConsensusEffects, VALIDATOR_SET_HASH_VERSION_V1},
    da::{
        commitment::{DaCommitmentBundle, DaProofPolicyBundle},
        pin_intent::DaPinIntentBundle,
    },
    events::prelude::*,
    merge::{MAX_MERGE_EXECUTION_BATCH_BYTES, MAX_MERGE_EXECUTION_ENTRYPOINTS, MergeLaneBinding},
    nexus::{
        AxtPolicyEntry, AxtProofEnvelope, AxtRejectReason, DataSpaceCatalog, LaneConfig,
        LaneSettlementBufferPolicy, ProofBlob,
    },
    transaction::{SignedTransaction, TransactionEntrypoint, error::TransactionLimitError},
};
#[cfg(test)]
use iroha_data_model::{
    block::consensus::{CertPhase, NativeAmxAttestationBodyV2},
    isi::InstructionBox,
    transaction::{Executable, error::TransactionRejectionReason, signed::TransactionResultInner},
};
#[cfg(feature = "bls")]
use iroha_model_base::metadata::Metadata;
use iroha_model_base::peer::PeerId;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
#[cfg(test)]
use iroha_primitives::numeric::Numeric;
use iroha_primitives::numeric::Quantity;
#[cfg(test)]
use iroha_primitives::small::SmallVec;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;
#[cfg(feature = "bls")]
use norito::json::Value as JsonValue;
#[cfg(test)]
use std::hint::black_box;
#[cfg(feature = "bls")]
use std::sync::LazyLock;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashSet},
    num::NonZeroU64,
    str::FromStr,
    time::Duration,
};

/// Return the first ordinary external entrypoint which illegally claims the
/// autonomous-only QueuePlan-synchronized admission intent.
///
/// QueuePlan-synchronized payloads enter a block only through authenticated
/// autonomous lane ownership and a certified merge carrier. Keeping this
/// predicate outside the block-validation state machine lets locked/recovered
/// body ingress reject the same role conflict before it persists ownership or
/// retires a competing autonomous reservation.
pub(crate) fn external_queue_plan_synced_entrypoint_index(block: &SignedBlock) -> Option<usize> {
    block.external_entrypoints_cloned().position(|entrypoint| {
        entrypoint.admission_intent()
            == iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced
    })
}

fn ensure_confidential_features_match(
    expected: Option<ConfidentialFeatureDigest>,
    actual: Option<ConfidentialFeatureDigest>,
) -> Result<(), BlockValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(BlockValidationError::ConfidentialFeaturesMismatch { expected, actual })
    }
}
fn validate_external_entrypoint_count(
    actual: usize,
    configured_max: NonZeroU64,
) -> Result<(), BlockValidationError> {
    let max = usize::try_from(configured_max.get()).unwrap_or(usize::MAX);
    if actual > max {
        Err(BlockValidationError::TooManyTransactions { actual, max })
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod external_entrypoint_count_tests {
    use super::*;
    #[test]
    fn configured_block_limit_is_enforced_before_expensive_validation() {
        let max = NonZeroU64::new(1).expect("one is non-zero");
        assert_eq!(validate_external_entrypoint_count(1, max), Ok(()));
        assert_eq!(
            validate_external_entrypoint_count(2, max),
            Err(BlockValidationError::TooManyTransactions { actual: 2, max: 1 })
        );
    }
}
#[cfg(feature = "bls")]
fn bls_pop_from_metadata(
    metadata: &Metadata,
    key: &iroha_model_base::name::Name,
) -> Option<Vec<u8>> {
    let json = metadata.get(key)?;
    let val: JsonValue = norito::json::from_str(json.get()).ok()?;
    match val {
        JsonValue::String(s) => hex::decode(s).ok(),
        _ => None,
    }
}
#[cfg(feature = "bls")]
fn bls_small_pop_from_metadata(
    metadata: &Metadata,
    key: &iroha_model_base::name::Name,
) -> Option<Vec<u8>> {
    bls_pop_from_metadata(metadata, key)
}
#[cfg(test)]
fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("block fixture key generation should succeed")
}
#[cfg(test)]
fn deterministic_test_network_id(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([seed; Hash::LENGTH]),
    ))
}
#[cfg(test)]
fn checked_keypair_with_algorithm(algorithm: iroha_crypto::Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm)
        .expect("block fixture key generation for requested algorithm should succeed")
}
#[cfg(test)]
mod checked_keypair_tests {
    #[test]
    fn checked_keypair_helpers_preserve_requested_algorithms() {
        assert_eq!(
            super::checked_keypair().algorithm(),
            iroha_crypto::Algorithm::default()
        );
        for algorithm in [
            iroha_crypto::Algorithm::Ed25519,
            iroha_crypto::Algorithm::BlsNormal,
        ] {
            assert_eq!(
                super::checked_keypair_with_algorithm(algorithm).algorithm(),
                algorithm
            );
        }
    }
}
#[cfg(test)]
/// Preserve stable overlay rejection labels for scheduler regression tests.
fn map_overlay_error(
    err: &crate::pipeline::overlay::OverlayBuildError,
) -> TransactionRejectionReason {
    match err {
        crate::pipeline::overlay::OverlayBuildError::HeaderPolicy(e) => {
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::IvmAdmission(
                e.clone(),
            ))
        }
        crate::pipeline::overlay::OverlayBuildError::AxtReject(ctx) => {
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::AxtReject(
                ctx.clone(),
            ))
        }
        crate::pipeline::overlay::OverlayBuildError::InvalidAxtPolicySnapshot(error) => {
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::InternalError(
                format!("invalid AXT policy snapshot: {error}"),
            ))
        }
        crate::pipeline::overlay::OverlayBuildError::AmxBudgetViolation(violation) => {
            let message = crate::pipeline::overlay::amx_timeout_message(violation);
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::NotPermitted(
                format!(
                    "{message} code={}",
                    iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE
                ),
            ))
        }
        crate::pipeline::overlay::OverlayBuildError::IvmRun(ivm::VMError::AmxBudgetExceeded {
            dataspace,
            stage,
            elapsed_ms,
            budget_ms,
        }) => TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted(format!(
                "{} code={}",
                crate::pipeline::overlay::amx_timeout_message(
                    &crate::smartcontracts::ivm::host::AmxBudgetViolation {
                        dataspace: *dataspace,
                        stage: *stage,
                        elapsed_ms: u32::try_from((*elapsed_ms).min(u64::from(u32::MAX)))
                            .expect("elapsed_ms clamped to u32::MAX"),
                        budget_ms: u32::try_from((*budget_ms).min(u64::from(u32::MAX)))
                            .expect("budget_ms clamped to u32::MAX"),
                    }
                ),
                iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE
            )),
        ),
        other => TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted(other.to_string()),
        ),
    }
}
#[cfg(test)]
/// Reference VM-overlay classification retained for scheduler regression tests.
#[must_use]
const fn uses_live_vm_overlay_scheduler(executable: &Executable) -> bool {
    matches!(
        executable,
        Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_)
    )
}
#[cfg(test)]
/// Reference live-batch classification retained for scheduler regression tests.
#[must_use]
fn uses_live_batch_scheduler(executable: &Executable) -> bool {
    matches!(executable, Executable::Batch(_))
        || matches!(
            crate::state::standalone_governance_ballot_instruction_v1(executable),
            Ok(Some(_)) | Err(_)
        )
}
#[cfg(test)]
mod overlay_error_tests {
    use super::*;
    use iroha_data_model::{
        ValidationFail,
        nexus::{AxtPolicySnapshotValidationError, AxtRejectContext, AxtRejectReason},
        transaction::{ExecutableBatchItem, IvmBytecode, IvmProved},
    };
    use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
    #[test]
    fn map_overlay_error_preserves_axt_context() {
        let ctx = AxtRejectContext {
            reason: AxtRejectReason::Manifest,
            dataspace: Some(DataSpaceId::new(7)),
            lane: Some(LaneId::new(3)),
            snapshot_version: Some(42),
            detail: "manifest mismatch".to_string(),
            active_handle_era: None,
            next_handle_counter: None,
        };
        let mapped = map_overlay_error(&crate::pipeline::overlay::OverlayBuildError::AxtReject(
            ctx.clone(),
        ));
        match mapped {
            TransactionRejectionReason::Validation(ValidationFail::AxtReject(seen)) => {
                assert_eq!(seen.reason, ctx.reason);
                assert_eq!(seen.dataspace, ctx.dataspace);
                assert_eq!(seen.lane, ctx.lane);
                assert_eq!(seen.snapshot_version, ctx.snapshot_version);
                assert!(seen.detail.contains("manifest"));
            }
            other => panic!("unexpected mapping: {other:?}"),
        }
    }
    #[test]
    fn map_overlay_error_classifies_invalid_axt_snapshot_as_internal() {
        let mapped = map_overlay_error(
            &crate::pipeline::overlay::OverlayBuildError::InvalidAxtPolicySnapshot(
                AxtPolicySnapshotValidationError::VersionMismatch {
                    expected: 7,
                    actual: 8,
                },
            ),
        );
        assert!(matches!(
            mapped,
            TransactionRejectionReason::Validation(ValidationFail::InternalError(message))
                if message == "invalid AXT policy snapshot: policy snapshot version mismatch: expected 7, found 8"
        ));
    }
    #[test]
    fn ivm_proved_uses_live_overlay_scheduler_path() {
        let proved = Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(Vec::new()),
            overlay: Vec::<InstructionBox>::new().into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas-policy"),
        });
        assert!(uses_live_vm_overlay_scheduler(&proved));
        let instructions = Executable::Instructions(Vec::<InstructionBox>::new().into());
        assert!(!uses_live_vm_overlay_scheduler(&instructions));
    }
    #[test]
    fn mixed_batch_uses_live_scheduler_barrier_path() {
        let batch = Executable::Batch(
            vec![ExecutableBatchItem::Instruction(InstructionBox::from(
                iroha_data_model::isi::Log::new(
                    iroha_data_model::level::Level::INFO,
                    "live batch".to_owned(),
                ),
            ))]
            .into(),
        );
        assert!(uses_live_batch_scheduler(&batch));
        let instructions = Executable::Instructions(Vec::<InstructionBox>::new().into());
        assert!(!uses_live_batch_scheduler(&instructions));
    }
    #[test]
    fn governance_ballot_requires_an_exact_standalone_direct_entrypoint() {
        let ballot = InstructionBox::from(iroha_data_model::isi::governance::CastZkBallot {
            election_id: "referendum.v1".to_owned(),
            proof_b64: "AA==".to_owned(),
            public_inputs_json: "{}".to_owned(),
        });
        let instructions = Executable::Instructions(vec![ballot.clone()].into());
        assert!(uses_live_batch_scheduler(&instructions));
        assert_eq!(
            crate::state::standalone_governance_ballot_instruction_v1(&instructions),
            Ok(Some(ballot.clone()))
        );

        let singleton_batch =
            Executable::Batch(vec![ExecutableBatchItem::Instruction(ballot.clone())].into());
        assert_eq!(
            crate::state::standalone_governance_ballot_instruction_v1(&singleton_batch),
            Ok(Some(ballot.clone()))
        );

        let mixed = Executable::Instructions(
            vec![
                ballot,
                InstructionBox::from(iroha_data_model::isi::Log::new(
                    iroha_data_model::level::Level::INFO,
                    "must not follow a ballot".to_owned(),
                )),
            ]
            .into(),
        );
        assert!(uses_live_batch_scheduler(&mixed));
        assert!(crate::state::standalone_governance_ballot_instruction_v1(&mixed).is_err());
    }
}
const EMPTY_CONFIDENTIAL_FEATURE_DIGEST: ConfidentialFeatureDigest =
    iroha_data_model::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST;
#[cfg(test)]
pub(crate) use self::event::EventProducer;
pub(crate) use self::event::WithEvents;
pub use self::{chained::Chained, commit::CommittedBlock, new::NewBlock, valid::ValidBlock};
use crate::{
    da::{
        DaCommitmentValidationError, DaPinIntentValidationError, DaShardCursorError,
        receipts::DaReceiptCursorError,
    },
    fees::SwapEvidence,
};
#[cfg(feature = "telemetry")]
use settlement_router::haircut::LiquidityProfile;
use settlement_router::{XorQuantity, policy::BufferStatus};
use thiserror::Error;
#[derive(Default)]
struct LaneSummary {
    tx_vertices: u64,
    rbc_bytes_total: u64,
}
#[derive(Default)]
struct LaneSettlementBuilder {
    tx_count: u64,
    total_local_amount: Quantity,
    total_xor_due: Quantity,
    total_xor_after_haircut: Quantity,
    total_xor_variance: Quantity,
    swap_evidence: Option<SwapEvidence>,
    receipts: Vec<LaneSettlementReceipt>,
    nexus_fee_receipts: Vec<crate::settlement::PendingNexusFeeReceipt>,
    native_amx_receipts: Vec<NativeAmxReceipt>,
    buffer_snapshot: Option<SettlementBufferSnapshot>,
    source_counts: BTreeMap<AssetDefinitionId, u64>,
}
fn attach_manifest_roots_to_relays(
    envelopes: &mut [LaneRelayEnvelope],
    manifest_roots: &BTreeMap<DataSpaceId, [u8; 32]>,
) {
    for envelope in envelopes {
        envelope.manifest_root = manifest_roots.get(&envelope.dataspace_id).copied();
    }
}
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
#[derive(Clone)]
pub(crate) struct SettlementBufferSnapshot {
    config: LaneSettlementBufferPolicy,
    remaining: XorQuantity,
    status: BufferStatus,
}
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
impl SettlementBufferSnapshot {
    pub(crate) fn remaining(&self) -> &XorQuantity {
        &self.remaining
    }
    pub(crate) fn capacity(&self) -> &XorQuantity {
        &self.config.capacity
    }
    pub(crate) fn status(&self) -> BufferStatus {
        self.status
    }
}
fn compute_settlement_buffer_snapshot(
    state_block: &StateBlock,
    lane_id: LaneId,
) -> Result<Option<SettlementBufferSnapshot>, String> {
    let lane = lane_metadata_by_id(state_block, lane_id)
        .ok_or_else(|| format!("unknown settlement lane {}", lane_id.as_u32()))?;
    let Some(config) = lane.settlement_buffer.clone() else {
        return Ok(None);
    };
    let asset_id = AssetId::new(
        config.asset_definition_id.clone(),
        config.account_id.clone(),
    );
    let assets = state_block.world.assets();
    let remaining = assets.get(&asset_id).map_or_else(
        || Ok(XorQuantity::zero()),
        |value| {
            XorQuantity::try_from_quantity(value.as_ref().clone()).map_err(|error| {
                format!(
                    "settlement buffer asset `{asset_id}` violates the XOR quantity domain: {error}"
                )
            })
        },
    )?;
    let status = state_block
        .settlement_engine()
        .evaluate_buffer(&remaining, &config.capacity)
        .map_err(|error| format!("invalid settlement buffer policy: {error}"))?;
    Ok(Some(SettlementBufferSnapshot {
        config,
        remaining,
        status,
    }))
}
fn lane_metadata_by_id<'state>(
    state_block: &'state StateBlock<'state>,
    lane_id: LaneId,
) -> Option<&'state LaneConfig> {
    state_block
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == lane_id)
}
#[cfg(feature = "telemetry")]
fn liquidity_profile_label(profile: LiquidityProfile) -> &'static str {
    match profile {
        LiquidityProfile::Tier1 => "tier1-deep",
        LiquidityProfile::Tier2 => "tier2-medium",
        LiquidityProfile::Tier3 => "tier3-thin",
    }
}
#[cfg(feature = "telemetry")]
fn record_lane_settlement_metrics(
    telemetry: &crate::telemetry::StateTelemetry,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    builder: &LaneSettlementBuilder,
) {
    let xor_due_micro =
        crate::settlement::quantity_to_micro_units_saturating_for_telemetry(&builder.total_xor_due);
    let xor_variance_micro = crate::settlement::quantity_to_micro_units_saturating_for_telemetry(
        &builder.total_xor_variance,
    );
    let swapline = builder
        .swap_evidence
        .as_ref()
        .map(|e| (liquidity_profile_label(e.liquidity_profile), xor_due_micro));
    let haircut_bps = builder.swap_evidence.as_ref().map_or(0, |e| e.epsilon_bps);
    telemetry.record_lane_settlement_snapshot_metrics(
        lane_id,
        dataspace_id,
        xor_due_micro,
        xor_variance_micro,
        haircut_bps,
        swapline,
        builder.buffer_snapshot.as_ref(),
    );
    let lane_label = lane_id.as_u32().to_string();
    let dataspace_label = dataspace_id.as_u64().to_string();
    telemetry.inc_settlement_haircut_total(
        lane_label.as_str(),
        dataspace_label.as_str(),
        xor_variance_micro,
    );
    for (asset_id, count) in &builder.source_counts {
        if *count == 0 {
            continue;
        }
        let asset_label = asset_id.to_string();
        telemetry.inc_settlement_conversion_total(
            lane_label.as_str(),
            dataspace_label.as_str(),
            asset_label.as_str(),
            *count,
        );
    }
}
#[cfg(test)]
use crate::{
    kura::{PipelineDagSnapshot, PipelineRecoverySidecar, PipelineTxSnapshot},
    pipeline::{overlay::TxOverlay, smallset::sort_dedup_u32_in_place},
    tx::is_quarantine_transaction,
};
use crate::{
    prelude::*,
    queue::{
        reconcile_execution_routing_plan, resolve_routing_decision,
        routing_plan_from_execution_context,
    },
    state::{
        State, StateBlock, StatelessValidationContext, WorldReadOnly,
        compute_confidential_feature_digest,
    },
    sumeragi::{network_topology::Topology, v2_candidate::candidate_block_has_proposal_work},
    tx::{AcceptTransactionFail, SignatureRejectionCode, SignatureVerificationFail},
};
use std::sync::Arc;
type CommittedBlockEval = Result<CommittedBlock, (Box<ValidBlock>, Box<BlockValidationError>)>;
type WithCommittedBlockEvents = WithEvents<CommittedBlockEval>;
struct PreparedBlockTransaction {
    metadata: crate::tx::PreparedTransactionMetadata,
}
#[cfg(test)]
use crate::tx::QUARANTINE_METADATA_KEY;
#[cfg(test)]
#[derive(Clone)]
struct AccessIds {
    reads: SmallVec<[u32; 8]>,
    writes: SmallVec<[u32; 8]>,
}
#[cfg(test)]
const GLOBAL_WILDCARD_KEY: &str = "*";
#[cfg(test)]
const STATE_KEY_PREFIX: &str = "state:";
#[cfg(test)]
const STATE_WILDCARD_SUFFIX: &str = "[*]";
#[cfg(test)]
fn state_wildcard_base(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(STATE_KEY_PREFIX)?;
    if rest == "*" {
        return Some("*");
    }
    rest.strip_suffix(STATE_WILDCARD_SUFFIX)
}
#[cfg(test)]
fn state_map_entry_base(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(STATE_KEY_PREFIX)?;
    let (base, _) = rest.split_once('/')?;
    if base.is_empty() {
        return None;
    }
    Some(base)
}
#[cfg(test)]
fn state_wildcard_key(base: &str) -> String {
    if base == "*" {
        format!("{STATE_KEY_PREFIX}*")
    } else {
        format!("{STATE_KEY_PREFIX}{base}{STATE_WILDCARD_SUFFIX}")
    }
}
#[cfg(test)]
#[allow(clippy::explicit_iter_loop)]
fn intern_access(access: &[crate::pipeline::access::AccessSet]) -> (usize, Vec<AccessIds>) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut wildcard_bases: BTreeSet<String> = BTreeSet::new();
    let mut global_present = false;
    for aset in access.iter() {
        for key in aset.read_keys.iter().chain(aset.write_keys.iter()) {
            if key == GLOBAL_WILDCARD_KEY {
                global_present = true;
            }
            if let Some(base) = state_wildcard_base(key) {
                wildcard_bases.insert(base.to_string());
            }
        }
    }
    let mut wildcard_keys: BTreeMap<String, String> = BTreeMap::new();
    for base in &wildcard_bases {
        wildcard_keys.insert(base.clone(), state_wildcard_key(base));
    }
    let mut map: BTreeMap<&str, u32> = BTreeMap::new();
    // Assign stable IDs by iterating lexicographically over all keys
    for aset in access.iter() {
        for k in aset.read_keys.iter() {
            map.entry(k.as_str()).or_insert(u32::MAX);
        }
        for k in aset.write_keys.iter() {
            map.entry(k.as_str()).or_insert(u32::MAX);
        }
    }
    for k in wildcard_keys.values() {
        map.entry(k.as_str()).or_insert(u32::MAX);
    }
    if global_present {
        map.entry(GLOBAL_WILDCARD_KEY).or_insert(u32::MAX);
    }
    let mut next: u32 = 0;
    for value in map.values_mut() {
        *value = next;
        next = next.saturating_add(1);
    }
    let key_count = next as usize;
    let mut out: Vec<AccessIds> = Vec::with_capacity(access.len());
    for aset in access.iter() {
        let mut reads: SmallVec<[u32; 8]> = SmallVec::new();
        let mut writes: SmallVec<[u32; 8]> = SmallVec::new();
        let has_global = aset.read_keys.contains(GLOBAL_WILDCARD_KEY)
            || aset.write_keys.contains(GLOBAL_WILDCARD_KEY);
        let add_state_wildcard = |key: &str, reads: &mut SmallVec<[u32; 8]>| {
            if let Some(base) = state_map_entry_base(key) {
                if let Some(wildcard_key) = wildcard_keys.get(base) {
                    reads.push(*map.get(wildcard_key.as_str()).expect("key interned"));
                }
            }
            if wildcard_bases.contains("*") && key.starts_with(STATE_KEY_PREFIX) {
                if state_wildcard_base(key) == Some("*") {
                    return;
                }
                if let Some(wildcard_key) = wildcard_keys.get("*") {
                    reads.push(*map.get(wildcard_key.as_str()).expect("key interned"));
                }
            }
        };
        for key in aset.read_keys.iter() {
            if state_wildcard_base(key).is_some() {
                writes.push(*map.get(key.as_str()).expect("all keys interned"));
            } else {
                reads.push(*map.get(key.as_str()).expect("all keys interned"));
            }
            add_state_wildcard(key, &mut reads);
        }
        for key in aset.write_keys.iter() {
            writes.push(*map.get(key.as_str()).expect("all keys interned"));
            add_state_wildcard(key, &mut reads);
        }
        if has_global {
            writes.push(*map.get(GLOBAL_WILDCARD_KEY).expect("all keys interned"));
        } else if global_present {
            reads.push(*map.get(GLOBAL_WILDCARD_KEY).expect("all keys interned"));
        }
        let len_reads = sort_dedup_u32_in_place(reads.0.as_mut_slice());
        reads.0.truncate(len_reads);
        let len_writes = sort_dedup_u32_in_place(writes.0.as_mut_slice());
        writes.0.truncate(len_writes);
        out.push(AccessIds { reads, writes });
    }
    (key_count, out)
}
#[cfg(test)]
fn expected_pipeline_dag_fingerprint(
    height: u64,
    block_hash: HashOf<BlockHeader>,
    call_hashes: &[HashOf<TransactionEntrypoint>],
    sidecar: &PipelineRecoverySidecar,
) -> Option<[u8; 32]> {
    if sidecar.height != height {
        iroha_logger::debug!(
            height,
            sidecar_height = sidecar.height,
            "pipeline sidecar height mismatch; ignoring expected DAG fingerprint"
        );
        return None;
    }
    if sidecar.block_hash != block_hash {
        iroha_logger::debug!(
            height,
            expected = %block_hash,
            actual = %sidecar.block_hash,
            "pipeline sidecar block hash mismatch; ignoring expected DAG fingerprint"
        );
        return None;
    }
    let matches_block = sidecar.txs.len() == call_hashes.len()
        && sidecar
            .txs
            .iter()
            .zip(call_hashes.iter())
            .all(|(tx, hash)| tx.hash == *hash);
    if !matches_block {
        iroha_logger::debug!(
            height,
            sidecar_txs = sidecar.txs.len(),
            block_txs = call_hashes.len(),
            "pipeline sidecar does not match block transactions; ignoring expected DAG fingerprint"
        );
        return None;
    }
    Some(sidecar.dag.fingerprint)
}
pub(crate) fn parse_account_literal_with_world(
    world: &impl WorldReadOnly,
    dataspace_catalog: &DataSpaceCatalog,
    input: &str,
    now_ms: u64,
) -> Result<Option<AccountId>, crate::sns::SnsError> {
    let literal = input.trim();
    if literal.is_empty() {
        return Ok(None);
    }
    if let Ok(account_id) = AccountId::parse_encoded(literal) {
        return Ok(Some(account_id));
    }
    let Ok(alias) = AccountAlias::from_literal(literal, dataspace_catalog) else {
        return Ok(None);
    };
    resolve_account_alias_in_world(world, dataspace_catalog, &alias, now_ms)
}
pub(crate) fn resolve_account_alias_in_world(
    world: &impl WorldReadOnly,
    dataspace_catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    now_ms: u64,
) -> Result<Option<AccountId>, crate::sns::SnsError> {
    crate::sns::resolve_active_account_alias(world, dataspace_catalog, alias, now_ms)
}
pub(crate) fn parse_asset_definition_literal_with_world(
    world: &impl WorldReadOnly,
    input: &str,
    now_ms: u64,
) -> Option<AssetDefinitionId> {
    let literal = input.trim();
    if literal.is_empty() {
        return None;
    }
    AssetDefinitionId::parse_address_literal(literal)
        .ok()
        .or_else(|| {
            AssetDefinitionAlias::from_str(literal)
                .ok()
                .and_then(|alias| world.asset_definition_id_by_alias_at(&alias, now_ms))
        })
}
/// Resolve an exact fee selector against the network's committed XOR identity.
/// Chains without NPoS use the canonical default identity, never an arbitrary
/// locally configured token. Staking itself additionally requires signed NPoS.
pub(crate) fn resolve_network_xor_asset_definition(
    world: &impl WorldReadOnly,
    input: &str,
    now_ms: u64,
) -> Option<AssetDefinitionId> {
    if input.trim() != input
        || (input != "xor#universal" && AssetDefinitionId::parse_address_literal(input).is_err())
    {
        return None;
    }
    let asset = parse_asset_definition_literal_with_world(world, input, now_ms)?;
    let pin = match world.sumeragi_npos_parameters() {
        Some(params) => params.xor_asset_definition_id,
        None => {
            if world.parameters().custom().contains_key(
                &iroha_data_model::parameter::system::SumeragiNposParameters::parameter_id(),
            ) {
                return None;
            }
            AssetDefinitionId::parse_address_literal(
                &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
            )
            .ok()?
        }
    };
    (asset == pin).then_some(asset)
}
#[cfg(test)]
#[test]
fn network_xor_resolver_requires_exact_committed_identity() {
    use iroha_data_model::parameter::{Parameter, system::SumeragiNposParameters};
    let world = crate::state::World::new();
    let canonical = iroha_config::parameters::defaults::nexus::fees::fee_asset_id();
    let other = AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("test", "universal").expect("test domain"),
        "currency".parse().expect("name"),
    );
    assert!(resolve_network_xor_asset_definition(&world.view(), &canonical, 0).is_some());
    assert!(resolve_network_xor_asset_definition(&world.view(), &other.to_string(), 0).is_none());
    let mut parameters = world.parameters.block();
    parameters.get_mut().set_parameter(Parameter::Custom(
        SumeragiNposParameters {
            xor_asset_definition_id: other.clone(),
            ..Default::default()
        }
        .into_custom_parameter(),
    ));
    parameters.commit();
    assert_eq!(
        resolve_network_xor_asset_definition(&world.view(), &other.to_string(), 0),
        Some(other)
    );
    assert!(resolve_network_xor_asset_definition(&world.view(), &canonical, 0).is_none());
    assert!(
        resolve_network_xor_asset_definition(&world.view(), &format!(" {canonical}"), 0).is_none()
    );
}
#[cfg(test)]
fn parse_account_from_access_key(
    world: &impl WorldReadOnly,
    dataspace_catalog: &DataSpaceCatalog,
    key: &str,
    now_ms: u64,
) -> Result<Option<AccountId>, crate::sns::SnsError> {
    if let Some(rest) = key.strip_prefix("account:") {
        parse_account_literal_with_world(world, dataspace_catalog, rest, now_ms)
    } else if let Some(rest) = key.strip_prefix("account.detail:") {
        let Some((account_raw, _)) = rest.split_once(':') else {
            return Ok(None);
        };
        parse_account_literal_with_world(world, dataspace_catalog, account_raw, now_ms)
    } else {
        Ok(None)
    }
}
#[cfg(test)]
fn warm_overlay_chunk(overlay: &TxOverlay, chunk_size: usize) -> usize {
    let chunk = chunk_size.max(1);
    let mut warmed = 0usize;
    for instr in overlay.instructions().take(chunk) {
        let _ = black_box(instr.id());
        warmed = warmed.saturating_add(1);
    }
    warmed
}
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PrefetchStats {
    account_loaded: bool,
    tx_sequence_loaded: bool,
    permissions_touched: usize,
    roles_touched: usize,
}
#[cfg(test)]
fn prefetch_account_stores(state_block: &StateBlock<'_>, account_id: &AccountId) -> PrefetchStats {
    let mut stats = PrefetchStats::default();
    if let Some(account) = state_block.world.accounts.get(account_id) {
        let _ = black_box(account);
        stats.account_loaded = true;
    }
    if let Some(seq) = state_block.world.tx_sequences.get(account_id) {
        let _ = black_box(seq);
        stats.tx_sequence_loaded = true;
    }
    if let Some(perms) = state_block.world.account_permissions.get(account_id) {
        for perm in perms {
            let _ = black_box(perm);
            stats.permissions_touched = stats.permissions_touched.saturating_add(1);
        }
    }
    for (role, ()) in state_block.world.account_roles.iter() {
        if role.account == *account_id {
            let _ = black_box(role);
            stats.roles_touched = stats.roles_touched.saturating_add(1);
        }
    }
    stats
}
#[cfg(test)]
mod prefetch_tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        role::RoleIdWithOwner,
        state::{State, World},
    };
    use iroha_data_model::{
        Registrable,
        account::{Account, AccountAlias, AccountAliasDomain, AccountDetails, AccountValue},
        block::BlockHeader,
        domain::Domain,
        isi::{InstructionBox, Log},
        nexus::{DataSpaceCatalog, DataSpaceMetadata},
        role::RoleId,
    };
    use iroha_logger::Level;
    use iroha_model_base::domain::DomainId;
    use iroha_model_base::name::Name;
    use iroha_model_base::topology::DataSpaceId;
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;
    #[test]
    fn parse_account_key_variants() {
        let alice = (*ALICE_ID).clone();
        let world = World::new();
        let world_view = world.view();
        let detail_key = format!("account.detail:{alice}:quota");
        let expected = alice.clone();
        assert_eq!(
            parse_account_from_access_key(
                &world_view,
                &DataSpaceCatalog::default(),
                &format!("account:{alice}"),
                0,
            ),
            Ok(Some(expected.clone()))
        );
        assert_eq!(
            parse_account_from_access_key(
                &world_view,
                &DataSpaceCatalog::default(),
                &detail_key,
                0,
            ),
            Ok(Some(expected.clone()))
        );
        assert_eq!(expected.subject_id(), alice.subject_id());
        assert!(
            parse_account_from_access_key(
                &world_view,
                &DataSpaceCatalog::default(),
                "asset:coin#wonderland",
                0,
            )
            .expect("valid access-key account state")
            .is_none()
        );
    }
    #[test]
    fn parse_account_literal_rejects_i105_with_domain_suffix() {
        let alice = (*ALICE_ID).clone();
        let wonderland: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let domain = Domain::new(wonderland.clone()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let world_view = world.view();
        let i105 = alice.canonical_i105().expect("i105 encoding");
        let literal = format!("{i105}@{wonderland}");
        assert_eq!(
            parse_account_literal_with_world(
                &world_view,
                &DataSpaceCatalog::default(),
                &literal,
                0,
            ),
            Ok(None)
        );
    }
    #[test]
    fn parse_account_literal_accepts_encoded_without_selector_registry() {
        let alice = (*ALICE_ID).clone();
        let wonderland: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let domain = Domain::new(wonderland.clone()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let world_view = world.view();
        let i105 = alice.canonical_i105().expect("i105 encoding");
        assert_eq!(
            parse_account_literal_with_world(&world_view, &DataSpaceCatalog::default(), &i105, 0,),
            Ok(Some(alice))
        );
    }
    #[test]
    fn parse_account_literal_accepts_canonical_i105_without_domain_materialization() {
        let account = (*ALICE_ID).clone();
        let alpha: DomainId = DomainId::try_new("alpha", "universal").expect("alpha domain");
        let world = World::with(
            [Domain::new(alpha).build(&account)],
            [Account::new(account.clone()).build(&account)],
            [],
        );
        let world_view = world.view();
        let encoded = account
            .canonical_i105()
            .expect("canonical I105 account literal");
        assert_eq!(
            parse_account_literal_with_world(
                &world_view,
                &DataSpaceCatalog::default(),
                &encoded,
                0,
            ),
            Ok(Some(account)),
            "canonical I105 account ids must remain valid without domain-linked account materialization"
        );
    }
    #[test]
    fn parse_account_literal_resolves_on_chain_alias_literals() {
        let domain_id: DomainId = DomainId::try_new("ivm", "universal").expect("domain");
        let account_id = (*ALICE_ID).clone();
        let alias = AccountAlias::new(
            Name::from_str("gas").expect("alias name"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::UNIVERSAL,
        );
        let mut world = World::with(
            [Domain::new(domain_id.clone()).build(&account_id)],
            [Account::new(account_id.clone()).build(&account_id)],
            [],
        );
        let selector = crate::sns::selector_for_account_alias(&alias, &DataSpaceCatalog::default())
            .expect("SNS selector");
        let address = iroha_data_model::account::AccountAddress::from_account_id(&account_id)
            .expect("account address");
        let lease = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            account_id.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            iroha_model_base::metadata::Metadata::default(),
        );
        world.smart_contract_state_mut_for_testing().insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&lease),
        );
        world
            .account_aliases
            .insert(alias.clone(), account_id.clone());
        world.replace_account_rekey_record_for_testing(
            iroha_data_model::account::rekey::AccountRekeyRecord::new(alias, account_id.clone()),
        );
        let world_view = world.view();
        assert_eq!(
            parse_account_literal_with_world(
                &world_view,
                &DataSpaceCatalog::default(),
                "gas@ivm.universal",
                0,
            ),
            Ok(Some(account_id)),
            "account selectors must resolve active on-chain aliases to canonical account ids"
        );
    }
    #[test]
    fn parse_account_literal_resolves_aliases_in_non_default_dataspaces() {
        let account_id = (*ALICE_ID).clone();
        let alias = AccountAlias::domainless(
            Name::from_str("treasury").expect("alias name"),
            DataSpaceId::new(7),
        );
        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "retail".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let mut world = World::with(
            [],
            [Account::new(account_id.clone()).build(&account_id)],
            [],
        );
        let selector =
            crate::sns::selector_for_account_alias(&alias, &catalog).expect("SNS selector");
        let address = iroha_data_model::account::AccountAddress::from_account_id(&account_id)
            .expect("account address");
        let lease = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            account_id.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            iroha_model_base::metadata::Metadata::default(),
        );
        world.smart_contract_state_mut_for_testing().insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&lease),
        );
        world
            .account_aliases
            .insert(alias.clone(), account_id.clone());
        world.replace_account_rekey_record_for_testing(
            iroha_data_model::account::rekey::AccountRekeyRecord::new(alias, account_id.clone()),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(world, kura, query);
        state.set_dataspace_catalog_for_testing(catalog);
        let state_view = state.view();
        let world_view = state_view.world();
        assert_eq!(
            parse_account_literal_with_world(
                world_view,
                &state_view.nexus.dataspace_catalog,
                "treasury@retail",
                0,
            ),
            Ok(Some(account_id.clone())),
            "account selectors must resolve aliases in non-default dataspaces"
        );
    }
    #[test]
    fn warm_overlay_chunk_respectschunk_size() {
        let instrs = vec![
            InstructionBox::from(Log::new(Level::INFO, "a".to_owned())),
            InstructionBox::from(Log::new(Level::INFO, "b".to_owned())),
            InstructionBox::from(Log::new(Level::INFO, "c".to_owned())),
        ];
        let overlay = TxOverlay::from_instructions(instrs);
        assert_eq!(warm_overlay_chunk(&overlay, 2), 2);
        assert_eq!(warm_overlay_chunk(&overlay, 10), 3);
    }
    #[test]
    fn prefetch_account_reports_hits() {
        let alice = (*ALICE_ID).clone();
        let mut world = World::new();
        world
            .accounts
            .insert(alice.clone(), AccountValue::new(AccountDetails::default()));
        world.tx_sequences.insert(alice.clone(), 7);
        let role_id = RoleId {
            name: Name::from_str("auditor").expect("valid name"),
        };
        world
            .account_roles
            .insert(RoleIdWithOwner::new(alice.clone(), role_id), ());
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query_handle);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
        let state_block = state.block(header);
        let prefetch_stats = prefetch_account_stores(&state_block, &alice);
        assert!(prefetch_stats.account_loaded);
        assert!(prefetch_stats.tx_sequence_loaded);
        assert_eq!(prefetch_stats.roles_touched, 1);
        // No permissions were inserted above.
        assert_eq!(prefetch_stats.permissions_touched, 0);
    }
}
#[cfg(test)]
mod pipeline_recovery_tests {
    use super::*;
    #[test]
    fn expected_pipeline_dag_fingerprint_requires_matching_block_hash() {
        let height = 1;
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32]));
        let other_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32]));
        let call_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed([0x33; 32]));
        let dag = PipelineDagSnapshot {
            fingerprint: [0x44; 32],
            key_count: 1,
        };
        let txs = vec![PipelineTxSnapshot::compact(call_hash, 0, 0)];
        let sidecar_mismatch = PipelineRecoverySidecar::new(height, other_hash, dag, txs.clone());
        assert!(
            expected_pipeline_dag_fingerprint(height, block_hash, &[call_hash], &sidecar_mismatch)
                .is_none(),
            "sidecars anchored to a different block hash should be ignored"
        );
        let sidecar_match = PipelineRecoverySidecar::new(height, block_hash, dag, txs);
        assert_eq!(
            expected_pipeline_dag_fingerprint(height, block_hash, &[call_hash], &sidecar_match),
            Some(dag.fingerprint),
            "matching v1 sidecars should provide expected fingerprint"
        );
    }
}
#[cfg(test)]
#[derive(Debug)]
struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}
#[cfg(test)]
impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) {
        let mut ra = self.find(a);
        let mut rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            core::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        if self.rank[ra] == self.rank[rb] {
            self.rank[ra] = self.rank[ra].saturating_add(1);
        }
    }
}
#[cfg(test)]
fn component_iteration_order(
    components: &[Vec<usize>],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Vec<usize> {
    use core::cmp::Ordering;
    let mut indices: Vec<usize> = (0..components.len()).collect();
    let mut keys: Vec<
        Option<(
            iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>,
            usize,
        )>,
    > = Vec::with_capacity(indices.len());
    for component in components {
        let key = component
            .iter()
            .copied()
            .map(|idx| (call_hashes[idx], idx))
            .min_by(std::cmp::Ord::cmp);
        keys.push(key);
    }
    indices.sort_unstable_by(|&a, &b| match (&keys[a], &keys[b]) {
        (Some(ka), Some(kb)) => ka.cmp(kb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    indices
}
#[cfg(test)]
fn schedule_components_ready_heap(
    components: &[Vec<usize>],
    row_offsets: &[usize],
    cols: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Option<Vec<usize>> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    let n = call_hashes.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );
    if n == 0 {
        return Some(Vec::new());
    }
    let mut order = Vec::with_capacity(n);
    let mut in_component = vec![false; n];
    let mut local_indeg = vec![0usize; n];
    let mut heap: BinaryHeap<
        Reverse<(
            iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>,
            usize,
        )>,
    > = BinaryHeap::with_capacity(n);
    let ordered_components = component_iteration_order(components, call_hashes);
    for &component_idx in &ordered_components {
        let component = &components[component_idx];
        if component.is_empty() {
            continue;
        }
        for &idx in component {
            in_component[idx] = true;
            local_indeg[idx] = 0;
        }
        for &idx in component {
            let start = row_offsets[idx];
            let end = row_offsets[idx + 1];
            for &child in &cols[start..end] {
                debug_assert!(child < n, "CSR edge index out of bounds");
                if !in_component[child] {
                    return None;
                }
                local_indeg[child] = local_indeg[child].saturating_add(1);
            }
        }
        heap.clear();
        for &idx in component {
            if local_indeg[idx] == 0 {
                heap.push(Reverse((call_hashes[idx], idx)));
            }
        }
        let prior_len = order.len();
        while let Some(Reverse((_hash, node))) = heap.pop() {
            order.push(node);
            let start = row_offsets[node];
            let end = row_offsets[node + 1];
            for &child in &cols[start..end] {
                if in_component[child] {
                    let deg = local_indeg[child].saturating_sub(1);
                    local_indeg[child] = deg;
                    if deg == 0 {
                        heap.push(Reverse((call_hashes[child], child)));
                    }
                } else {
                    return None;
                }
            }
        }
        if order.len() - prior_len != component.len() {
            return None;
        }
        for &idx in component {
            in_component[idx] = false;
            local_indeg[idx] = 0;
        }
    }
    Some(order)
}
#[cfg(test)]
fn schedule_components_wave(
    components: &[Vec<usize>],
    row_offsets: &[usize],
    cols: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Option<Vec<usize>> {
    let n = call_hashes.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );
    if n == 0 {
        return Some(Vec::new());
    }
    let mut order = Vec::with_capacity(n);
    let mut in_component = vec![false; n];
    let mut local_indeg = vec![0usize; n];
    let mut ready_frontier: Vec<usize> = Vec::new();
    let mut current_layer: Vec<usize> = Vec::new();
    let ordered_components = component_iteration_order(components, call_hashes);
    for &component_idx in &ordered_components {
        let component = &components[component_idx];
        if component.is_empty() {
            continue;
        }
        for &idx in component {
            in_component[idx] = true;
            local_indeg[idx] = 0;
        }
        for &idx in component {
            let start = row_offsets[idx];
            let end = row_offsets[idx + 1];
            for &child in &cols[start..end] {
                debug_assert!(child < n, "CSR edge index out of bounds");
                if !in_component[child] {
                    return None;
                }
                local_indeg[child] = local_indeg[child].saturating_add(1);
            }
        }
        ready_frontier.clear();
        for &idx in component {
            if local_indeg[idx] == 0 {
                ready_frontier.push(idx);
            }
        }
        let prior_len = order.len();
        while !ready_frontier.is_empty() {
            ready_frontier.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            current_layer.clear();
            current_layer.extend(ready_frontier.iter().copied());
            ready_frontier.clear();
            for &node in &current_layer {
                order.push(node);
                let start = row_offsets[node];
                let end = row_offsets[node + 1];
                for &child in &cols[start..end] {
                    if in_component[child] {
                        let deg = local_indeg[child].saturating_sub(1);
                        local_indeg[child] = deg;
                        if deg == 0 {
                            ready_frontier.push(child);
                        }
                    } else {
                        return None;
                    }
                }
            }
        }
        if order.len() - prior_len != component.len() {
            return None;
        }
        for &idx in component {
            in_component[idx] = false;
            local_indeg[idx] = 0;
        }
    }
    Some(order)
}
#[cfg(test)]
fn conflict_free_component_layers(
    components: &[Vec<usize>],
    row_offsets: &[usize],
    cols: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Option<Vec<Vec<usize>>> {
    let n = call_hashes.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );
    if n == 0 {
        return Some(Vec::new());
    }
    let mut in_component = vec![false; n];
    let mut local_indeg = vec![0usize; n];
    let mut ready_frontier: Vec<usize> = Vec::new();
    let mut current_layer: Vec<usize> = Vec::new();
    let mut per_comp_layers: Vec<Vec<Vec<usize>>> = Vec::with_capacity(components.len());
    let mut max_depth = 0usize;
    let ordered_components = component_iteration_order(components, call_hashes);
    for &component_idx in &ordered_components {
        let component = &components[component_idx];
        if component.is_empty() {
            per_comp_layers.push(Vec::new());
            continue;
        }
        for &idx in component {
            debug_assert!(idx < n, "component vertex index out of bounds");
            in_component[idx] = true;
            local_indeg[idx] = 0;
        }
        for &idx in component {
            let start = row_offsets[idx];
            let end = row_offsets[idx + 1];
            for &child in &cols[start..end] {
                debug_assert!(child < n, "CSR edge index out of bounds");
                if !in_component[child] {
                    return None;
                }
                local_indeg[child] = local_indeg[child].saturating_add(1);
            }
        }
        ready_frontier.clear();
        for &idx in component {
            if local_indeg[idx] == 0 {
                ready_frontier.push(idx);
            }
        }
        let mut seen = 0usize;
        let mut comp_layers: Vec<Vec<usize>> = Vec::new();
        while !ready_frontier.is_empty() {
            ready_frontier.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            current_layer.clear();
            current_layer.extend(ready_frontier.iter().copied());
            ready_frontier.clear();
            let mut wave = Vec::with_capacity(current_layer.len());
            for &node in &current_layer {
                seen = seen.saturating_add(1);
                wave.push(node);
                let start = row_offsets[node];
                let end = row_offsets[node + 1];
                for &child in &cols[start..end] {
                    if !in_component[child] {
                        return None;
                    }
                    let deg = local_indeg[child].saturating_sub(1);
                    local_indeg[child] = deg;
                    if deg == 0 {
                        ready_frontier.push(child);
                    }
                }
            }
            comp_layers.push(wave);
        }
        if seen != component.len() {
            return None;
        }
        max_depth = max_depth.max(comp_layers.len());
        per_comp_layers.push(comp_layers);
        for &idx in component {
            in_component[idx] = false;
            local_indeg[idx] = 0;
        }
    }
    let mut layers: Vec<Vec<usize>> = Vec::with_capacity(max_depth);
    for depth in 0..max_depth {
        let mut wave: Vec<usize> = Vec::new();
        for comp_layers in &per_comp_layers {
            if let Some(layer) = comp_layers.get(depth) {
                wave.extend_from_slice(layer);
            }
        }
        if !wave.is_empty() {
            wave.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            layers.push(wave);
        }
    }
    Some(layers)
}
#[cfg(test)]
fn schedule_ready_heap_global(
    row_offsets: &[usize],
    cols: &[usize],
    indeg: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Vec<usize> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    let n = indeg.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );
    let mut indeg_s = indeg.to_vec();
    let mut heap: BinaryHeap<
        Reverse<(
            iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>,
            usize,
        )>,
    > = BinaryHeap::with_capacity(n);
    for i in 0..n {
        if indeg_s[i] == 0 {
            heap.push(Reverse((call_hashes[i], i)));
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(Reverse((_hash, node))) = heap.pop() {
        order.push(node);
        let start = row_offsets[node];
        let end = row_offsets[node + 1];
        for &child in &cols[start..end] {
            indeg_s[child] = indeg_s[child].saturating_sub(1);
            if indeg_s[child] == 0 {
                heap.push(Reverse((call_hashes[child], child)));
            }
        }
    }
    order
}
#[cfg(test)]
fn schedule_wave_global(
    row_offsets: &[usize],
    cols: &[usize],
    indeg: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Vec<usize> {
    let n = indeg.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );
    let mut indeg_s = indeg.to_vec();
    let mut ready_frontier: Vec<usize> = Vec::with_capacity(n);
    for (i, indegree) in indeg_s.iter().enumerate() {
        if *indegree == 0 {
            ready_frontier.push(i);
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut current_layer: Vec<usize> = Vec::new();
    while !ready_frontier.is_empty() {
        ready_frontier
            .sort_unstable_by(|&a, &b| call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b)));
        current_layer.clear();
        current_layer.extend(ready_frontier.iter().copied());
        ready_frontier.clear();
        for &node in &current_layer {
            order.push(node);
            let start = row_offsets[node];
            let end = row_offsets[node + 1];
            for &child in &cols[start..end] {
                indeg_s[child] = indeg_s[child].saturating_sub(1);
                if indeg_s[child] == 0 {
                    ready_frontier.push(child);
                }
            }
        }
    }
    order
}
#[cfg(test)]
impl Clone for DisjointSet {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            rank: self.rank.clone(),
        }
    }
}
/// Structured context for AXT envelope validation failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxtEnvelopeValidationDetails {
    /// Human-readable message describing the failure.
    pub message: String,
    /// Categorised reason label for the rejection.
    pub reason: AxtRejectReason,
    /// Policy snapshot version active during validation, when block results were present.
    pub snapshot_version: Option<u64>,
    /// Dataspace associated with the rejection (if known).
    pub dataspace: Option<DataSpaceId>,
    /// Lane associated with the rejection (if known).
    pub lane: Option<LaneId>,
    /// Exact active handle era for refresh guidance.
    pub active_handle_era: Option<u64>,
    /// Exact next handle counter for refresh guidance.
    pub next_handle_counter: Option<u64>,
}
impl fmt::Display for AxtEnvelopeValidationDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (reason={}, lane={:?}, dsid={:?}",
            self.message,
            self.reason.label(),
            self.lane,
            self.dataspace
        )?;
        if let Some(snapshot_version) = self.snapshot_version {
            write!(f, ", snapshot_version={snapshot_version}")?;
        }
        if let Some(hint) = self.active_handle_era {
            write!(f, ", active_handle_era={hint}")?;
        }
        if let Some(hint) = self.next_handle_counter {
            write!(f, ", next_handle_counter={hint}")?;
        }
        write!(f, ")")
    }
}
impl From<crate::state::StateBlockStartError<BlockValidationError>> for BlockValidationError {
    fn from(error: crate::state::StateBlockStartError<Self>) -> Self {
        match error {
            crate::state::StateBlockStartError::Storage(error) => {
                Self::StateStorageAdmission(error)
            }
            crate::state::StateBlockStartError::History(error) => Self::BlockHashAdmission(error),
            crate::state::StateBlockStartError::Membership(error) => {
                Self::MembershipAdmission(error)
            }
            crate::state::StateBlockStartError::Stage(error) => error,
        }
    }
}
/// Errors occurred on block validation
#[derive(Debug, displaydoc::Display, PartialEq, Eq, Error)]
pub enum BlockValidationError {
    /// Local World storage admission failed before State execution: {0}
    StateStorageAdmission(crate::state::StateStorageAdmissionError),
    /// Local evidence or stake-index penalty preparation failed: {0}
    EvidencePreparation(crate::state::EvidencePreparationError),
    /// Local execution did not complete: {0}
    ExecutionDeferred(crate::execution_attempt::ExecutionDeferred),
    /// Local hash-history admission failed before State execution: {0}
    BlockHashAdmission(crate::state::BlockHashAdmissionError),
    /// Local membership-history admission failed before State execution: {0}
    MembershipAdmission(crate::state::MembershipAdmissionError),
    /// Block has committed transactions
    HasCommittedTransactions,
    /// Block contained no committed overlays
    EmptyBlock,
    /// Block contains duplicate transactions
    DuplicateTransactions,
    /// Block contains too many external entrypoints. Actual: {actual}, maximum: {max}
    TooManyTransactions {
        /// Number of externally supplied entrypoints in the block.
        actual: usize,
        /// Consensus maximum active for this block.
        max: usize,
    },
    /// Mismatch between the actual and expected hashes of the previous block. Expected: {expected:?}, actual: {actual:?}
    PrevBlockHashMismatch {
        /// Expected value
        expected: Option<HashOf<BlockHeader>>,
        /// Actual value
        actual: Option<HashOf<BlockHeader>>,
    },
    /// Mismatch between the actual and expected height of the previous block. Expected: {expected}, actual: {actual}
    PrevBlockHeightMismatch {
        /// Expected value
        expected: usize,
        /// Actual value
        actual: usize,
    },
    /// The merkle root does not match the computed one.
    MerkleRootMismatch,
    /// Execution context invalid: {0}
    ExecutionContextInvalid(String),
    /// Local storage observation requires recovery before candidate validation: {reason}
    LocalStorageRecoveryRequired {
        /// Diagnostic from the failed local observation, never rejection authority.
        reason: String,
    },
    /// Certified merge sidecar `{entry_hash}` is not available locally yet
    MissingCertifiedMergeSidecar {
        /// Canonical hash of the full merge entry required by this block.
        entry_hash: HashOf<iroha_data_model::merge::MergeLedgerEntry>,
    },
    /// Committed fragment count mismatch. Expected: {expected}, actual: {actual}
    CommittedFragmentCountMismatch {
        /// Count recomputed while executing the block.
        expected: u64,
        /// Count advertised in the received block result.
        actual: u64,
    },
    /// Cannot accept a transaction
    TransactionAccept(#[from] AcceptTransactionFail),
    /// Mismatch between the actual and expected topology. Expected: {expected:?}, actual: {actual:?}
    TopologyMismatch {
        /// Expected value
        expected: Vec<PeerId>,
        /// Actual value
        actual: Vec<PeerId>,
    },
    /// Error during block signatures check
    SignatureVerification(#[from] SignatureVerificationError),
    /// Invalid genesis block: {0}
    InvalidGenesis(#[from] InvalidGenesisError),
    /// Signed genesis policy mismatch: execution {expected_execution} != {actual_execution}, Nexus {expected_nexus} != {actual_nexus}
    GenesisPolicyMismatch {
        /// Execution policy committed by the signed genesis body.
        expected_execution: Hash,
        /// Execution policy actually used by this original genesis execution.
        actual_execution: Hash,
        /// Nexus/AMX policy committed by the signed genesis body.
        expected_nexus: Hash,
        /// Nexus/AMX context derived from the original staged genesis effects.
        actual_nexus: Hash,
    },
    /// Block's creation time is earlier than that of the previous block
    BlockInThePast,
    /// Block's creation time is later than the current node local time
    BlockInTheFuture,
    /// Sumeragi v2 block creation time is not the canonical logical time. Expected: {expected_ms} ms, actual: {actual_ms} ms
    NonCanonicalV2BlockTime {
        /// Deterministic timestamp derived from the parent, cadence, and transactions.
        expected_ms: u64,
        /// Timestamp committed by the proposed block.
        actual_ms: u64,
    },
    /// Sumeragi v2 logical block time exceeded the canonical u64-millisecond range
    V2BlockTimeOverflow,
    /// Sumeragi v2 snapshot bootstrap parent geometry is invalid: {0}
    SnapshotBootstrapParentInvalid(String),
    /// Sumeragi v2 finality authority does not bind this block and execution: {0}
    V2FinalityAuthorityInvalid(String),
    /// Some transaction in the block is created after the block itself
    TransactionInTheFuture,
    /// Block confidential feature digest mismatch. Expected: {expected:?}, actual: {actual:?}
    ConfidentialFeaturesMismatch {
        /// Digest expected by the local node.
        expected: Option<ConfidentialFeatureDigest>,
        /// Digest committed in the incoming block.
        actual: Option<ConfidentialFeatureDigest>,
    },
    /// Proof policy hash mismatch. Expected: {expected:?}, actual: {actual:?}
    ProofPolicyHashMismatch {
        /// Hash derived from the local lane catalog.
        expected: HashOf<DaProofPolicyBundle>,
        /// Hash embedded in the incoming header.
        actual: Option<HashOf<DaProofPolicyBundle>>,
    },
    /// DA proof-policy sidecar hash does not match the signed header. Expected: {expected:?}, actual: {actual:?}
    DaProofPolicySidecarHashMismatch {
        /// Hash derived from the embedded policy sidecar.
        expected: Option<HashOf<DaProofPolicyBundle>>,
        /// Hash embedded in the incoming header.
        actual: Option<HashOf<DaProofPolicyBundle>>,
    },
    /// DA proof-policy sidecar differs from the active policy snapshot.
    DaProofPolicyBundleMismatch,
    /// DA commitment hash mismatch. Expected: {expected:?}, actual: {actual:?}
    DaCommitmentHashMismatch {
        /// Hash derived from the embedded DA commitment bundle.
        expected: Option<HashOf<DaCommitmentBundle>>,
        /// Hash embedded in the incoming header.
        actual: Option<HashOf<DaCommitmentBundle>>,
    },
    /// An empty DA commitment sidecar must be represented as `None`.
    NonCanonicalEmptyDaCommitmentBundle,
    /// DA pin-intent hash mismatch. Expected: {expected:?}, actual: {actual:?}
    DaPinIntentHashMismatch {
        /// Hash derived from the embedded DA pin-intent bundle.
        expected: Option<HashOf<DaPinIntentBundle>>,
        /// Hash embedded in the incoming header.
        actual: Option<HashOf<DaPinIntentBundle>>,
    },
    /// An empty DA pin-intent sidecar must be represented as `None`.
    NonCanonicalEmptyDaPinIntentBundle,
    /// DA commitment bundle failed validation: {0}
    DaCommitmentBundle(#[from] DaCommitmentValidationError),
    /// DA pin-intent bundle failed validation: {0}
    DaPinIntentBundle(#[from] DaPinIntentValidationError),
    /// DA index hydration failed before block validation: {0}
    DaIndexHydration(String),
    /// DA receipt cursor gate failed: {0}
    DaReceiptCursor(#[from] DaReceiptCursorError),
    /// DA shard cursor gate failed: {0}
    DaShardCursor(#[from] DaShardCursorError),
    /// AXT envelope export contained invalid or inconsistent fragments: {0}
    AxtEnvelopeValidationFailed(AxtEnvelopeValidationDetails),
    /// NPoS consensus effects are invalid: {0}
    NposEffectsInvalid(String),
}
impl BlockValidationError {
    /// Keep local autoscale observations out of deterministic block rejection.
    pub(crate) fn from_autoscale_lifecycle_error(error: crate::state::LaneLifecycleError) -> Self {
        let reason = format!("failed to evaluate Nexus autoscale: {error}");
        match error {
            LaneLifecycleError::DrainObservation(_)
            | LaneLifecycleError::Storage(_)
            | LaneLifecycleError::GeometryStorage(_)
            | LaneLifecycleError::PublicationBusy { .. } => {
                Self::LocalStorageRecoveryRequired { reason }
            }
            _ => Self::ExecutionContextInvalid(reason),
        }
    }

    /// Preserve local observation provenance at the certified merge staging boundary.
    pub(crate) fn from_certified_merge_stage_error(
        error: crate::state::MergeLedgerCommitError,
    ) -> Self {
        use crate::state::MergeLedgerCommitError;
        match error {
            MergeLedgerCommitError::StateStorageAdmission(error) => {
                Self::StateStorageAdmission(error)
            }
            MergeLedgerCommitError::ExecutionDeferred(reason) => Self::ExecutionDeferred(reason),
            MergeLedgerCommitError::BlockHashAdmission(error) => Self::BlockHashAdmission(error),
            MergeLedgerCommitError::MembershipAdmission(error) => Self::MembershipAdmission(error),
            MergeLedgerCommitError::NativeControlValidation(error) => *error,
            MergeLedgerCommitError::MissingCertifiedMergeSidecar { entry_hash } => {
                Self::MissingCertifiedMergeSidecar { entry_hash }
            }
            local @ (MergeLedgerCommitError::NativeResourceAdmission(_)
            | MergeLedgerCommitError::ExecutionObservationChanged
            | MergeLedgerCommitError::ExecutionRecorderConflict(_)
            | MergeLedgerCommitError::Persistence(_)
            | MergeLedgerCommitError::LocalDrainObservation(_)) => {
                Self::LocalStorageRecoveryRequired {
                    reason: format!("certified merge entry could not be staged: {local}"),
                }
            }
            other => Self::ExecutionContextInvalid(format!(
                "certified merge entry could not be staged: {other}"
            )),
        }
    }

    /// Keep local resource refusal outside the deterministic NPoS verdict channel.
    pub(crate) fn from_npos_application_error(error: eyre::Report, stage: &str) -> Self {
        if let Some(local) = error.downcast_ref::<crate::state::StateAdmissionError>() {
            match local {
                crate::state::StateAdmissionError::Storage(error) => {
                    Self::StateStorageAdmission(error.clone())
                }
                crate::state::StateAdmissionError::History(error) => {
                    Self::BlockHashAdmission(error.clone())
                }
                crate::state::StateAdmissionError::Membership(error) => {
                    Self::MembershipAdmission(error.clone())
                }
            }
        } else if let Some(local) = error.downcast_ref::<crate::state::StateStorageAdmissionError>()
        {
            Self::StateStorageAdmission(local.clone())
        } else if let Some(local) = error.downcast_ref::<crate::state::EvidencePreparationError>() {
            Self::EvidencePreparation(local.clone())
        } else {
            Self::NposEffectsInvalid(format!("{stage}: {error}"))
        }
    }
}
/// Preserve the original epoch-allocation refusal before any diagnostic formatting.
impl From<crate::sumeragi::schedule::ScheduleError> for BlockValidationError {
    fn from(error: crate::sumeragi::schedule::ScheduleError) -> Self {
        use crate::sumeragi::schedule::ScheduleError;
        match error {
            ScheduleError::CommittedSource(source) => Self::LocalStorageRecoveryRequired {
                reason: source.to_string(),
            },
            ScheduleError::Admission(refusal) => Self::ExecutionDeferred(refusal.into()),
            ScheduleError::Allocator { .. } => {
                Self::ExecutionDeferred(ivm::error::ExecutionDeferral::AllocationUnavailable.into())
            }
            deterministic => Self::ExecutionContextInvalid(deterministic.to_string()),
        }
    }
}

#[cfg(test)]
#[test]
fn epoch_schedule_capacity_preserves_original_local_release() {
    use crate::sumeragi::schedule::ScheduleError;
    let budget = mv::allocation::AllocationBudget::new(8);
    let occupied = budget.try_reserve_bytes(8).expect("occupy original pool");
    let refusal = budget
        .try_reserve_bytes(1)
        .expect_err("local capacity refusal");
    let error = BlockValidationError::from(ScheduleError::Admission(refusal.clone()));
    assert_eq!(event::map_block_err_to_reason(&error), None);
    let BlockValidationError::ExecutionDeferred(owner) = error else {
        panic!("local epoch capacity cannot reject a block");
    };
    assert_eq!(owner.allocation_refusal(), Some(&refusal));
    assert!(matches!(
        owner.allocation_refusal(),
        Some(mv::allocation::AllocationRefusal::Capacity { .. })
    ));
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[cfg(test)]
#[test]
fn epoch_schedule_allocator_refusal_and_invalid_context_stay_distinct() {
    let source = crate::sumeragi::certified_chain::ChainReadError::NotInView { height: 2 };
    let local = BlockValidationError::from(
        crate::sumeragi::schedule::ScheduleError::CommittedSource(source.into()),
    );
    assert!(matches!(
        local,
        BlockValidationError::LocalStorageRecoveryRequired { .. }
    ));
    assert_eq!(event::map_block_err_to_reason(&local), None);

    use crate::sumeragi::schedule::ScheduleError;
    let error = BlockValidationError::from(ScheduleError::Allocator {
        requested_bytes: 64,
    });
    assert_eq!(event::map_block_err_to_reason(&error), None);
    let BlockValidationError::ExecutionDeferred(owner) = error else {
        panic!("physical allocator refusal cannot reject a block");
    };
    assert_eq!(
        owner.reason(),
        ivm::error::ExecutionDeferral::AllocationUnavailable
    );
    assert!(owner.allocation_refusal().is_none());
    let error = BlockValidationError::from(ScheduleError::Epoch("wrong authority".into()));
    assert!(matches!(
        error,
        BlockValidationError::ExecutionContextInvalid(_)
    ));
    assert!(event::map_block_err_to_reason(&error).is_some());
}

impl From<crate::state::DaIndexHydrationError> for BlockValidationError {
    fn from(error: crate::state::DaIndexHydrationError) -> Self {
        // These errors arise while replaying already committed local history,
        // including its cursors. Preserve that context so the v2 validator does
        // not mistake local reconstruction failure for a malformed candidate.
        Self::DaIndexHydration(error.to_string())
    }
}
#[cfg(test)]
#[test]
fn native_resource_refusal_is_a_local_certified_merge_staging_error() {
    let error = BlockValidationError::from_certified_merge_stage_error(
        crate::state::MergeLedgerCommitError::NativeResourceAdmission(
            mv::allocation::AllocationRefusal::DemandOverflow,
        ),
    );
    assert!(matches!(
        error,
        BlockValidationError::LocalStorageRecoveryRequired { .. }
    ));
}
#[cfg(test)]
#[test]
fn native_execution_observation_and_recorder_conflicts_require_local_recovery() {
    for error in [
        crate::state::MergeLedgerCommitError::ExecutionObservationChanged,
        crate::state::MergeLedgerCommitError::ExecutionRecorderConflict(
            "recorder is already owned".to_owned(),
        ),
    ] {
        let classified = BlockValidationError::from_certified_merge_stage_error(error);
        assert!(matches!(
            &classified,
            BlockValidationError::LocalStorageRecoveryRequired { .. }
        ));
        assert!(event::map_block_err_to_reason(&classified).is_none());
    }
}
/// Error during signature verification
#[derive(Debug, displaydoc::Display, Clone, Copy, PartialEq, Eq, Error)]
pub enum SignatureVerificationError {
    /// The block doesn't have enough valid signatures to be committed (`{votes_count}` out of `{min_votes_for_commit}`)
    NotEnoughSignatures {
        /// Current number of signatures
        votes_count: usize,
        /// Minimal required number of signatures
        min_votes_for_commit: usize,
    },
    /// Multiple signatures were provided for the same signer index (`{signer}`)
    DuplicateSignature {
        /// Signer index that appeared more than once
        signer: usize,
    },
    /// Block signatory doesn't correspond to any in topology
    UnknownSignatory,
    /// Block signature doesn't correspond to block payload
    UnknownSignature,
    /// Missing proof-of-possession for validator consensus key
    MissingPop,
    /// The block doesn't have proxy tail signature
    ProxyTailMissing,
    /// The block doesn't have leader signature
    LeaderMissing,
    /// Block signer does not have an active consensus key for this height/role
    InactiveConsensusKey,
    /// Miscellaneous
    Other,
}
/// Errors occurred on genesis block validation
#[derive(Debug, Copy, Clone, displaydoc::Display, PartialEq, Eq, Error)]
pub enum InvalidGenesisError {
    /// Genesis block must be signed with genesis private key and not signed by any peer
    InvalidSignature,
    /// Every genesis transaction must carry a valid authorization proof for the genesis account
    InvalidTransactionSignature,
    /// Genesis authority must use a single-key account controller
    GenesisAuthorityNotSingleKey,
    /// Genesis transaction must be authorized by genesis account
    UnexpectedAuthority,
    /// Genesis block must carry deterministic execution results
    MissingResults,
    /// Genesis Network output count {actual} does not match input count {expected}
    NetworkOutputCountMismatch {
        /// Number of complete genesis Network inputs.
        expected: usize,
        /// Number of attached Network output rows.
        actual: usize,
    },
    /// Genesis execution outputs must not contain errors, including internal invocations
    ContainsErrors,
    /// Genesis proposal commitments do not match their complete source payload
    ProposalCommitmentMismatch,
    /// Genesis typed outputs have malformed source ownership, phase or metadata
    OutputStructureMismatch,
    /// Genesis output Merkle cache does not match the complete typed execution outputs
    OutputMerkleCacheMismatch,
    /// Genesis transaction must contain instructions
    NotInstructions,
    /// Genesis block must have 1 to 16 transactions (executor upgrade, parameters, ordinary instructions, IVM trigger registrations, initial topology)
    BadTransactionsAmount,
    /// Genesis block header must start the chain (height 1, no previous hash)
    InvalidHeader,
    /// Genesis Merkle root does not match the committed transactions
    MerkleRootMismatch,
    /// Genesis output count exceeds the canonical `u64` range
    GenesisOutputCountOverflow,
    /// Genesis committed fragment count {actual:?} is below the successful-output lower bound {minimum}
    CommittedFragmentCountBelowOutputCount {
        /// Minimum number of committed fragments implied by successful genesis outputs.
        minimum: u64,
        /// Fragment count advertised by the attached block result, if present.
        actual: Option<u64>,
    },
    /// A genesis transaction does not carry the explicit genesis-only domain.
    TransactionDomainMismatch,
    /// Genesis DA commitment hash does not match embedded bundle
    DaCommitmentMismatch,
    /// Genesis DA proof-policy hash does not match a valid embedded policy bundle
    DaProofPolicyMismatch,
    /// Genesis DA pin intent hash does not match embedded bundle
    DaPinIntentMismatch,
}
/// Validate the structural correctness of a genesis block before submitting it to the pipeline.
///
/// # Errors
///
/// Returns [`InvalidGenesisError`] when the block violates any of the required genesis invariants,
/// such as signature mismatch, invalid authorities, or malformed transactions.
#[allow(clippy::too_many_lines)]
pub fn check_genesis_block(
    block: &SignedBlock,
    genesis_account: &iroha_data_model::account::AccountId,
) -> Result<(), InvalidGenesisError> {
    authenticate_genesis_block_intents(block, genesis_account)?;
    check_genesis_execution_results(block)
}
/// Authenticate the immutable genesis intent before any bootstrap instruction executes.
///
/// Consensus proposals deliberately omit deterministic execution results. The configured
/// genesis key must therefore authenticate the height-one header and its ordered transaction
/// intents independently of whether those results have already been attached.
#[derive(Debug)]
pub(crate) struct AuthenticatedGenesisOutputSource {
    header: BlockHeader,
    account: AccountId,
}

impl AuthenticatedGenesisOutputSource {
    /// Recheck the exact immutable source authenticated against the configured genesis key.
    pub(crate) fn account_for(&self, block: &SignedBlock) -> Result<&AccountId, String> {
        block.validate_proposal_commitments()?;
        if block.header() != self.header {
            return Err("genesis execution capability belongs to another proposal".into());
        }
        Ok(&self.account)
    }
}

#[allow(clippy::too_many_lines)]
fn authenticate_genesis_block_intents(
    block: &SignedBlock,
    genesis_account: &iroha_data_model::account::AccountId,
) -> Result<AuthenticatedGenesisOutputSource, InvalidGenesisError> {
    const MAX_GENESIS_TRANSACTIONS: usize = 16;
    let signatures = block.signatures().collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(InvalidGenesisError::InvalidSignature);
    };
    if signature.index() != 0 {
        return Err(InvalidGenesisError::InvalidSignature);
    }
    let genesis_signatory = genesis_account
        .try_signatory()
        .ok_or(InvalidGenesisError::GenesisAuthorityNotSingleKey)?;
    signature
        .signature()
        .verify_hash(genesis_signatory, block.hash())
        .map_err(|_| InvalidGenesisError::InvalidSignature)?;
    if block.header().height().get() != 1 || block.header().prev_block_hash().is_some() {
        return Err(InvalidGenesisError::InvalidHeader);
    }
    let transactions: Vec<_> = block.external_transactions().collect();
    let external_entrypoints: Vec<_> = block.external_entrypoints_cloned().collect();
    if transactions.is_empty() || transactions.len() > MAX_GENESIS_TRANSACTIONS {
        return Err(InvalidGenesisError::BadTransactionsAmount);
    }
    if external_entrypoints.len() != transactions.len()
        || external_entrypoints.iter().any(|entrypoint| {
            !matches!(
                entrypoint,
                iroha_data_model::transaction::TransactionEntrypoint::External(_)
            )
        })
    {
        return Err(InvalidGenesisError::BadTransactionsAmount);
    }
    let expected_merkle_root = block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<MerkleTree<_>>()
        .root();
    if block.header().merkle_root() != expected_merkle_root {
        return Err(InvalidGenesisError::MerkleRootMismatch);
    }
    match (
        block.header().da_proof_policies_hash(),
        block.da_proof_policies(),
    ) {
        (Some(hash), Some(bundle))
            if hash == HashOf::new(bundle)
                && crate::da::validate_committed_proof_policy_bundle(bundle).is_ok() => {}
        _ => return Err(InvalidGenesisError::DaProofPolicyMismatch),
    }
    match (block.header().da_commitments_hash(), block.da_commitments()) {
        (None, None) => {}
        (Some(hash), Some(bundle)) => {
            if bundle.merkle_commitment() != Some(hash) {
                return Err(InvalidGenesisError::DaCommitmentMismatch);
            }
        }
        _ => return Err(InvalidGenesisError::DaCommitmentMismatch),
    }
    match (block.header().da_pin_intents_hash(), block.da_pin_intents()) {
        (None, None) => {}
        (Some(hash), Some(bundle)) => {
            let expected = bundle.merkle_commitment();
            if expected != Some(hash) {
                return Err(InvalidGenesisError::DaPinIntentMismatch);
            }
        }
        _ => return Err(InvalidGenesisError::DaPinIntentMismatch),
    }
    for transaction in transactions {
        if transaction.domain() != &iroha_data_model::transaction::TransactionDomain::Genesis {
            return Err(InvalidGenesisError::TransactionDomainMismatch);
        }
        if transaction.authority() != genesis_account {
            return Err(InvalidGenesisError::UnexpectedAuthority);
        }
        transaction
            .verify_signature()
            .map_err(|_| InvalidGenesisError::InvalidTransactionSignature)?;
        let iroha_data_model::transaction::Executable::Instructions(_isi) =
            transaction.instructions()
        else {
            return Err(InvalidGenesisError::NotInstructions);
        };
    }
    Ok(AuthenticatedGenesisOutputSource {
        header: block.header(),
        account: genesis_account.clone(),
    })
}
fn check_genesis_execution_results(block: &SignedBlock) -> Result<(), InvalidGenesisError> {
    use iroha_data_model::block::execution_output::ExecutionOutputV1;

    if !block.has_results() {
        return Err(InvalidGenesisError::MissingResults);
    }
    block
        .validate_proposal_commitments()
        .map_err(|_| InvalidGenesisError::ProposalCommitmentMismatch)?;
    let input_count = block.network_entrypoint_count();
    let outputs = block.execution_outputs();
    let network_count = outputs
        .iter()
        .filter(|output| matches!(output, ExecutionOutputV1::Network(_)))
        .count();
    if network_count != input_count {
        return Err(InvalidGenesisError::NetworkOutputCountMismatch {
            expected: input_count,
            actual: network_count,
        });
    }
    // Internal invocations have their own output positions. The complete shape
    // validator checks each Network.input_index, phase and invocation owner.
    block
        .validate_execution_result_structure()
        .map_err(|_| InvalidGenesisError::OutputStructureMismatch)?;
    block
        .validate_output_merkle_cache()
        .map_err(|_| InvalidGenesisError::OutputMerkleCacheMismatch)?;
    if outputs.iter().any(|output| output.result().is_err()) {
        return Err(InvalidGenesisError::ContainsErrors);
    }
    // Every successful root invocation applies at least one fragment; retained
    // protocol fragments can make the actual count larger than the output count.
    let minimum_committed_fragment_count = u64::try_from(outputs.len())
        .map_err(|_| InvalidGenesisError::GenesisOutputCountOverflow)?;
    let actual_committed_fragment_count = block.committed_fragment_count();
    if !matches!(
        actual_committed_fragment_count,
        Some(actual) if actual >= minimum_committed_fragment_count
    ) {
        return Err(
            InvalidGenesisError::CommittedFragmentCountBelowOutputCount {
                minimum: minimum_committed_fragment_count,
                actual: actual_committed_fragment_count,
            },
        );
    }
    Ok(())
}
/// Canonical millisecond time strictly after every timed execution input.
/// Admission controls are not execution inputs and do not advance this clock.
fn creation_time_after_inputs<I, T>(minimum: Duration, inputs: I) -> Option<Duration>
where
    I: IntoIterator<Item = T>,
    T: core::borrow::Borrow<TransactionEntrypoint>,
{
    let mut milliseconds = u64::try_from(minimum.as_millis()).ok()?;
    for input in inputs {
        if let Some(created) = input.borrow().creation_time_ms() {
            milliseconds = milliseconds.max(created.checked_add(1)?);
        }
    }
    Some(Duration::from_millis(milliseconds))
}

#[cfg(test)]
mod input_clock_tests {
    use super::*;
    use iroha_data_model::transaction::{
        TransactionBuilder,
        signed::{
            SealedTransactionCommitmentPayload, SealedTransactionReveal,
            SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
        },
    };

    fn timed_input(milliseconds: u64) -> (SignedTransaction, KeyPair) {
        let key = KeyPair::from_seed(vec![0x91; 32], iroha_crypto::Algorithm::Ed25519);
        let mut builder = TransactionBuilder::new(
            crate::unit_test_support::synthetic_network_id("input-clock"),
            AccountId::new(key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(milliseconds));
        (
            builder
                .with_instructions(Vec::<InstructionBox>::new())
                .sign(key.private_key()),
            key,
        )
    }

    #[test]
    fn input_clock_covers_external_and_sealed_inputs_without_timing_commitments() {
        let (transaction, key) = timed_input(10_000);
        let network_id = *transaction.network_id().unwrap();
        let commitment =
            compute_sealed_transaction_commitment(&network_id, &transaction, [0x92; 32], 100);
        let external = TransactionEntrypoint::External(transaction.clone());
        let reveal = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
            commitment,
            transaction.clone(),
            [0x92; 32],
        ));
        let sealed =
            TransactionEntrypoint::SealedCommitment(SignedSealedTransactionCommitment::sign(
                SealedTransactionCommitmentPayload {
                    network_id,
                    authority: transaction.authority().clone(),
                    commitment,
                    reveal_after_height: 1,
                    reveal_deadline_height: 100,
                    nonce: None,
                },
                key.private_key(),
            ));
        for input in [&external, &reveal] {
            assert_eq!(
                creation_time_after_inputs(Duration::from_millis(1), [input]),
                Some(Duration::from_millis(10_001))
            );
        }
        assert_eq!(
            creation_time_after_inputs(Duration::from_millis(1), [&sealed]),
            Some(Duration::from_millis(1))
        );
        assert_eq!(
            creation_time_after_inputs(
                Duration::from_millis(20_000),
                [&external, &reveal, &sealed]
            ),
            Some(Duration::from_millis(20_000))
        );
    }

    #[test]
    fn input_clock_rejects_unrepresentable_input_successor_and_baseline() {
        let (transaction, _) = timed_input(u64::MAX);
        assert!(
            creation_time_after_inputs(
                Duration::ZERO,
                [&TransactionEntrypoint::External(transaction)]
            )
            .is_none()
        );
        assert!(
            creation_time_after_inputs(
                Duration::from_millis(u64::MAX) + Duration::from_millis(1),
                std::iter::empty::<&TransactionEntrypoint>()
            )
            .is_none()
        );
        assert_eq!(
            creation_time_after_inputs(
                Duration::from_millis(u64::MAX),
                std::iter::empty::<&TransactionEntrypoint>(),
            ),
            Some(Duration::from_millis(u64::MAX))
        );
    }
}

/// Builder for blocks
#[derive(Debug, Clone)]
pub struct BlockBuilder<B>(B);
#[cfg(any(test, feature = "iroha-core-tests"))]
fn default_test_execution_context(
    transactions: &[AcceptedTransaction<'static>],
    header: &BlockHeader,
    validator: PeerId,
) -> BlockExecutionContextBundle {
    let external = transactions
        .iter()
        .map(|tx| {
            ExternalExecutionContext::new(
                tx.hash_as_entrypoint(),
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            )
        })
        .collect::<Vec<_>>();
    let has_network_transaction = transactions.iter().any(|tx| {
        match tx.entrypoint() {
            TransactionEntrypoint::External(tx) => tx.network_id(),
            TransactionEntrypoint::SealedCommitment(commitment) => {
                Some(&commitment.payload().network_id)
            }
            TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction().network_id(),
        }
        .is_some()
    });
    if !has_network_transaction {
        return BlockExecutionContextBundle::new(external);
    }
    let catalog = iroha_data_model::nexus::LaneCatalog::default();
    let lane_incarnation = crate::state::derive_static_lane_incarnations(&catalog)
        .get(&LaneId::SINGLE)
        .copied()
        .expect("default test catalog contains lane zero");
    let candidate_indices = (0..transactions.len())
        .map(|idx| u64::try_from(idx).expect("test transaction index fits u64"))
        .collect::<Vec<_>>();
    let candidate_hashes = transactions
        .iter()
        .map(|tx| Hash::from(tx.hash_as_entrypoint()))
        .collect::<Vec<_>>();
    let validator_set = vec![validator];
    let mut ownership = iroha_data_model::block::consensus::SumeragiLanePayloadOwnership {
        proposal_height: header.height().get(),
        proposal_view: header.view_change_index(),
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_incarnation,
        lane_block_height: 1,
        lane_block_view: header.view_change_index(),
        subject_hash: Hash::new(b"block-builder test lane subject placeholder"),
        qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            "block-builder-test",
        ),
        accepted_candidate_indices: candidate_indices,
        accepted_transaction_hashes: candidate_hashes,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_descriptor_hash: Some(Hash::new(
            b"block-builder test lane descriptor placeholder",
        )),
        lane_block_descriptor_validator_set: validator_set,
        lane_block_descriptor_validator_count: 1,
        lane_block_descriptor_min_quorum: 1,
        payload_ownership_hash: Hash::new(b"block-builder test ownership placeholder"),
        rbc_instance_hash: Hash::new(b"block-builder test RBC placeholder"),
    };
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("default test lane payload ownership must be complete");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    BlockExecutionContextBundle::new(external).with_lane_payload_ownerships(vec![ownership])
}
/// Return whether lane ownership was synthesized by the state-free block-builder test helper.
///
/// The helper can validate transaction routing, but it cannot bind the latest applied Kura lane
/// frontier. Its ownership is therefore validation scaffolding, not a durable lane artifact.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub(crate) fn is_default_test_execution_context_ownership(
    ownership: &iroha_data_model::block::consensus::SumeragiLanePayloadOwnership,
) -> bool {
    ownership.lane_id == LaneId::SINGLE
        && ownership.dataspace_id == DataSpaceId::UNIVERSAL
        && ownership.qc_mode_tag
            == LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                "block-builder-test",
            )
}
mod pending {
    use super::*;
    use iroha_primitives::time::TimeSource;
    use nonzero_ext::nonzero;
    /// First stage in the life-cycle of a [`Block`].
    /// In the beginning the block is assumed to be verified and to contain only accepted transactions.
    /// Additionally the block must retain events emitted during the execution of on-chain logic during
    /// the previous round, which might then be processed by the trigger system.
    #[derive(Debug, Clone)]
    pub struct Pending {
        /// Collection of transactions which have been accepted.
        transactions: Vec<AcceptedTransaction<'static>>,
        time_source: TimeSource,
    }
    impl BlockBuilder<Pending> {
        const TIME_PADDING: Duration = Duration::from_millis(1);
        /// Create [`Self`] while preserving the caller's transaction order.
        #[inline]
        pub fn new(transactions: Vec<AcceptedTransaction<'static>>) -> Self {
            Self::new_with_time_source(transactions, TimeSource::new_system())
        }
        /// Create with a provided [`TimeSource`] while preserving transaction order.
        pub fn new_with_time_source(
            transactions: Vec<AcceptedTransaction<'static>>,
            time_source: TimeSource,
        ) -> Self {
            // Empty blocks can be built for tests, but validation rejects them unless they carry
            // entrypoints (external transactions or time triggers) or deterministic artifacts
            // such as DA bundles; consensus should not emit them.
            Self(Pending {
                transactions,
                time_source,
            })
        }
        fn make_header(
            &self,
            prev_block: Option<&SignedBlock>,
            view_change_index: u64,
        ) -> BlockHeader {
            let prev_block_time =
                prev_block.map_or(Duration::ZERO, |block| block.header().creation_time());
            let latest_txn_time = self
                .0
                .transactions
                .iter()
                .map(crate::tx::AcceptedTransaction::creation_time)
                .max()
                // No transactions present; validation still rejects empty payloads.
                .unwrap_or(Duration::ZERO);
            let now = self.0.time_source.get_unix_time();
            // NOTE: Lower time bound must always be upheld for a valid block
            // If the clock has drifted too far this block will be rejected
            let creation_time = [
                now,
                latest_txn_time.saturating_add(Self::TIME_PADDING),
                prev_block_time.saturating_add(Self::TIME_PADDING),
            ]
            .into_iter()
            .max()
            .unwrap();
            let height = prev_block.map(|block| block.header().height()).map_or_else(
                || nonzero!(1_u64),
                |height| {
                    height
                        .checked_add(1)
                        .expect("INTERNAL BUG: Blockchain height exceeds usize::MAX")
                },
            );
            let prev_block_hash = prev_block.map(SignedBlock::hash);
            let merkle_root = self
                .0
                .transactions
                .iter()
                .map(crate::tx::AcceptedTransaction::hash_as_entrypoint)
                .collect::<MerkleTree<_>>()
                .root();
            let creation_time_ms = creation_time.as_millis().try_into().unwrap_or(u64::MAX);
            BlockHeader::new(
                height,
                prev_block_hash,
                merkle_root,
                creation_time_ms,
                view_change_index,
            )
        }
        /// Chain the block with existing blockchain.
        ///
        /// Upon executing this method current timestamp is stored in the block header.
        pub fn chain(
            self,
            view_change_index: u64,
            latest_block: Option<&SignedBlock>,
        ) -> BlockBuilder<Chained> {
            let mut header = self.make_header(latest_block, view_change_index);
            if header.confidential_features().is_none() {
                header.set_confidential_features(Some(EMPTY_CONFIDENTIAL_FEATURE_DIGEST));
            }
            BlockBuilder(Chained {
                header,
                transactions: self.0.transactions,
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
                npos_consensus_effects: None,
                global_beacon_pulse: None,
                execution_context: None,
            })
        }
        /// Chain the block to a known parent hash when the parent body is unavailable.
        pub fn chain_with_parent_hash(
            self,
            view_change_index: u64,
            parent_height: u64,
            parent_hash: HashOf<BlockHeader>,
        ) -> BlockBuilder<Chained> {
            let mut header = self.make_header(None, view_change_index);
            header.set_height(
                core::num::NonZeroU64::new(parent_height.saturating_add(1))
                    .expect("parent height plus one is non-zero"),
            );
            header.set_prev_block_hash(Some(parent_hash));
            if header.confidential_features().is_none() {
                header.set_confidential_features(Some(EMPTY_CONFIDENTIAL_FEATURE_DIGEST));
            }
            BlockBuilder(Chained {
                header,
                transactions: self.0.transactions,
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
                npos_consensus_effects: None,
                global_beacon_pulse: None,
                execution_context: None,
            })
        }
    }
}
mod chained {
    use super::*;
    use iroha_crypto::SignatureOf;
    use new::NewBlock;
    /// When a `Pending` block is chained with the blockchain it becomes [`Chained`] block.
    #[derive(Debug, Clone)]
    pub struct Chained {
        pub(super) header: BlockHeader,
        pub(super) transactions: Vec<AcceptedTransaction<'static>>,
        pub(super) da_commitments: Option<DaCommitmentBundle>,
        pub(super) da_proof_policies: Option<DaProofPolicyBundle>,
        pub(super) da_pin_intents: Option<DaPinIntentBundle>,
        pub(super) npos_consensus_effects: Option<NposConsensusEffects>,
        pub(super) global_beacon_pulse:
            Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
        pub(super) execution_context: Option<BlockExecutionContextBundle>,
    }
    impl BlockBuilder<Chained> {
        /// Creation time that will be embedded in the block header.
        #[inline]
        #[must_use]
        pub fn creation_time(&self) -> Duration {
            self.0.header.creation_time()
        }
        /// Derive the clock from the exact retained ordinary and Native inputs.
        /// Call again after removing Native groups with the original baseline,
        /// so deferred inputs cannot advance the signed carrier's timestamp.
        pub(crate) fn with_network_input_time_floor(mut self, minimum: Duration) -> Option<Self> {
            let inputs = self
                .0
                .transactions
                .iter()
                .map(AcceptedTransaction::entrypoint)
                .chain(
                    self.0
                        .execution_context
                        .iter()
                        .filter_map(|context| context.native_lane_decisions.as_ref())
                        .flat_map(|batch| batch.groups.iter())
                        .map(|group| &group.payload.input.entrypoint),
                );
            let time = creation_time_after_inputs(minimum, inputs)?;
            self.0.header.creation_time_ms = u64::try_from(time.as_millis()).ok()?;
            Some(self)
        }
        /// Header context selected for this proposal before payload/result roots are finalized.
        ///
        /// Certified merge execution strips those roots and binds the remaining height, parent,
        /// ledger-time, and view fields, so callers may use this clone to select an exact pending
        /// merge sidecar without introducing a header-hash cycle.
        #[inline]
        #[must_use]
        pub(crate) fn carrier_context_header(&self) -> BlockHeader {
            self.0.header.clone()
        }
        /// Bind this proposal to the exact ledger timestamp certified by a merge batch.
        ///
        /// Height, parent, and view are selected by the active global round and must already
        /// match. Only the timestamp may be adopted from the pre-executed merge application
        /// context; payload roots are deliberately excluded from that context.
        pub(crate) fn bind_certified_merge_application_context(
            mut self,
            application: &BlockHeader,
        ) -> Result<Self, &'static str> {
            if application.merkle_root().is_some()
                || application.creation_time().is_zero()
                || application.height() != self.0.header.height()
                || application.prev_block_hash() != self.0.header.prev_block_hash()
                || application.view_change_index() != self.0.header.view_change_index()
            {
                return Err(
                    "certified merge application context differs from the active global round",
                );
            }
            self.0.header.creation_time_ms = u64::try_from(application.creation_time().as_millis())
                .map_err(|_| "certified merge application timestamp exceeds u64")?;
            Ok(self)
        }
        /// Attach a DA commitment bundle and update the header hash accordingly.
        #[must_use]
        pub fn with_da_commitments(mut self, commitments: Option<DaCommitmentBundle>) -> Self {
            let hash = commitments
                .as_ref()
                .and_then(DaCommitmentBundle::merkle_commitment);
            self.0.header.set_da_commitments_hash(hash);
            self.0.da_commitments = commitments;
            self
        }
        /// Attach a DA proof policy bundle and update the header hash accordingly.
        #[must_use]
        pub fn with_da_proof_policies(mut self, policies: Option<DaProofPolicyBundle>) -> Self {
            let hash = policies.as_ref().map(HashOf::new);
            self.0.header.set_da_proof_policies_hash(hash);
            self.0.da_proof_policies = policies;
            self
        }
        /// Attach a DA pin intent bundle and update the header hash accordingly.
        #[must_use]
        pub fn with_da_pin_intents(mut self, intents: Option<DaPinIntentBundle>) -> Self {
            let hash = intents
                .as_ref()
                .and_then(DaPinIntentBundle::merkle_commitment);
            self.0.header.set_da_pin_intents_hash(hash);
            self.0.da_pin_intents = intents;
            self
        }
        /// Attach deterministic `NPoS` effects and update the header hash accordingly.
        #[must_use]
        pub fn with_npos_consensus_effects(
            mut self,
            effects: Option<NposConsensusEffects>,
        ) -> Self {
            let effects = effects.filter(|bundle| !bundle.is_empty());
            let hash = effects.as_ref().map(HashOf::new);
            self.0.header.set_npos_effects_hash(hash);
            self.0.npos_consensus_effects = effects;
            self
        }
        /// Attach the current threshold pulse; it never substitutes transaction work.
        #[must_use]
        pub fn with_global_beacon_pulse(
            mut self,
            pulse: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
        ) -> Self {
            self.0
                .header
                .set_global_beacon_pulse_hash(pulse.as_ref().map(HashOf::new));
            self.0.global_beacon_pulse = pulse;
            self
        }
        /// Attach durable execution context and update the header hash accordingly.
        #[must_use]
        pub fn with_execution_context(
            mut self,
            context: Option<BlockExecutionContextBundle>,
        ) -> Self {
            let context = context.filter(|bundle| !bundle.is_empty());
            let hash = context.as_ref().map(HashOf::new);
            self.0.header.set_execution_context_hash(hash);
            self.0.execution_context = context;
            self
        }
        /// Attach the confidential feature digest that this block commits to.
        #[must_use]
        pub fn with_confidential_features(
            mut self,
            digest: Option<ConfidentialFeatureDigest>,
        ) -> Self {
            self.0.header.set_confidential_features(digest);
            self
        }
        /// Count the exact canonical proposal wire with one signature, without signing.
        ///
        /// The caller must have finished the actual proposal metadata. Only the
        /// fixed-length signature contents are replaced with private sizing bytes;
        /// every transaction, control, policy, header and Norito prefix uses the
        /// normal `NewBlock` to `SignedBlockWire` conversion. No block or bytes
        /// escape this sizing operation.
        pub(crate) fn canonical_proposal_wire_len(
            &self,
            signatory_idx: u64,
            algorithm: iroha_crypto::Algorithm,
        ) -> Result<usize, String> {
            if self.0.da_proof_policies.is_none()
                || (!self.0.transactions.is_empty() && self.0.execution_context.is_none())
            {
                return Err(
                    "proposal sizing requires explicit proof policies and execution context"
                        .to_owned(),
                );
            }
            let signature =
                BlockSignature::new(
                    signatory_idx,
                    SignatureOf::from_signature(iroha_crypto::Signature::from_bytes(
                        &vec![0xa5; algorithm.signature_payload_len()],
                    )),
                );
            let sizing_only: SignedBlock = self.clone().into_new_block(signature).into();
            sizing_only
                .encode_wire()
                .map(|wire| wire.len())
                .map_err(|error| error.to_string())
        }
        /// Retain an exact canonical prefix of this proposal's admission controls.
        /// Other controls and external execution metadata remain on this builder.
        pub(crate) fn retain_queue_plan_admission_prefix(
            mut self,
            count: usize,
        ) -> Result<Self, String> {
            let Some(context) = self.0.execution_context.take() else {
                return if count == 0 {
                    Ok(self)
                } else {
                    Err("proposal has no admission controls".to_owned())
                };
            };
            if count > context.queue_plan_admissions().len() {
                return Err("admission prefix exceeds its actual control vector".to_owned());
            }
            let admissions = context.queue_plan_admissions()[..count].to_vec();
            Ok(self.with_execution_context(Some(context.with_queue_plan_admissions(admissions))))
        }
        /// Retain whole native groups in their canonical first-admission order.
        /// Rebuilding the context also recomputes the proposal input root; zero
        /// removes this economic form while retaining independent controls.
        pub(crate) fn retain_native_lane_decision_prefix(
            mut self,
            count: usize,
        ) -> Result<Self, String> {
            let Some(mut context) = self.0.execution_context.take() else {
                return if count == 0 {
                    Ok(self)
                } else {
                    Err("proposal has no native Decision input".into())
                };
            };
            let Some(batch) = context.native_lane_decisions.as_mut() else {
                return if count == 0 {
                    Ok(self.with_execution_context(Some(context)))
                } else {
                    Err("proposal has no native Decision input".into())
                };
            };
            if count > batch.groups.len() {
                return Err("native prefix exceeds its actual group vector".into());
            }
            if count == 0 {
                context.native_lane_decisions = None;
            } else {
                batch.groups.truncate(count);
            }
            context.validate_native_lane_decisions_shape()?;
            Ok(self.with_execution_context((!context.is_empty()).then_some(context)))
        }
        fn into_new_block(self, signature: BlockSignature) -> NewBlock {
            NewBlock {
                signature,
                header: self.0.header,
                transactions: self.0.transactions,
                da_commitments: self.0.da_commitments,
                da_proof_policies: self.0.da_proof_policies,
                da_pin_intents: self.0.da_pin_intents,
                npos_consensus_effects: self.0.npos_consensus_effects,
                global_beacon_pulse: self.0.global_beacon_pulse,
                execution_context: self.0.execution_context,
            }
        }
        /// Fallibly sign this block and get [`NewBlock`] using the provided validator index.
        ///
        /// # Errors
        ///
        /// Returns [`iroha_crypto::Error::Signing`] when the signing backend
        /// rejects the private key or block-header payload.
        pub fn try_sign_with_index(
            self,
            private_key: &PrivateKey,
            signatory_idx: u64,
        ) -> Result<WithEvents<NewBlock>, iroha_crypto::Error> {
            let mut builder = self;
            if builder.0.da_proof_policies.is_none()
                && builder.0.header.da_proof_policies_hash().is_none()
            {
                let default_policies = crate::da::proof_policy_bundle(
                    &iroha_config::parameters::actual::LaneConfig::default(),
                );
                builder = builder.with_da_proof_policies(Some(default_policies));
            }
            #[cfg(any(test, feature = "iroha-core-tests"))]
            if builder.0.execution_context.is_none() && !builder.0.transactions.is_empty() {
                let validator =
                    PeerId::new(iroha_crypto::PublicKey::from_private_key(private_key)?);
                let context = default_test_execution_context(
                    &builder.0.transactions,
                    &builder.0.header,
                    validator,
                );
                builder = builder.with_execution_context(Some(context));
            }
            let signature = BlockSignature::new(
                signatory_idx,
                SignatureOf::try_from_hash(private_key, builder.0.header.hash())?,
            );
            Ok(WithEvents::new(builder.into_new_block(signature)))
        }
        /// Finish this block without a block signature (`specs/sumeragi.md` §3.2): the
        /// canonical resultless proposal a Sumeragi leader proposes, which its certified core
        /// header authenticates without depending on a local signing key.
        #[must_use]
        pub fn into_unsigned_proposal(self) -> SignedBlock {
            let mut builder = self;
            if builder.0.da_proof_policies.is_none()
                && builder.0.header.da_proof_policies_hash().is_none()
            {
                let default_policies = crate::da::proof_policy_bundle(
                    &iroha_config::parameters::actual::LaneConfig::default(),
                );
                builder = builder.with_da_proof_policies(Some(default_policies));
            }
            let Chained {
                header,
                transactions,
                da_commitments,
                da_proof_policies,
                da_pin_intents,
                npos_consensus_effects,
                global_beacon_pulse,
                execution_context,
            } = builder.0;
            SignedBlock::unsigned_with_payload(BlockPayload {
                header,
                external_entrypoints: transactions
                    .into_iter()
                    .map(AcceptedTransaction::into_entrypoint)
                    .collect(),
                execution_context,
                da_commitments,
                da_proof_policies,
                da_pin_intents,
                npos_consensus_effects,
                global_beacon_pulse,
            })
        }
        /// Sign this block and get [`NewBlock`] using the provided validator index.
        pub fn sign_with_index(
            self,
            private_key: &PrivateKey,
            signatory_idx: u64,
        ) -> WithEvents<NewBlock> {
            self.try_sign_with_index(private_key, signatory_idx)
                .expect("block signing should succeed for a valid private key and header hash")
        }
        /// Fallibly sign this block and get [`NewBlock`] using validator index 0.
        ///
        /// # Errors
        ///
        /// Returns [`iroha_crypto::Error::Signing`] when the signing backend
        /// rejects the private key or block-header payload.
        pub fn try_sign(
            self,
            private_key: &PrivateKey,
        ) -> Result<WithEvents<NewBlock>, iroha_crypto::Error> {
            self.try_sign_with_index(private_key, 0)
        }
        /// Sign this block and get [`NewBlock`] using validator index 0.
        pub fn sign(self, private_key: &PrivateKey) -> WithEvents<NewBlock> {
            self.try_sign(private_key)
                .expect("block signing should succeed for a valid private key and header hash")
        }
    }
}
mod new {
    use super::*;
    #[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]
    use crate::state::StateBlock;
    /// First stage in the life-cycle of a block.
    ///
    /// Transactions in this block are not categorized.
    #[derive(Debug, Clone)]
    pub struct NewBlock {
        pub(super) signature: BlockSignature,
        pub(super) header: BlockHeader,
        pub(super) transactions: Vec<AcceptedTransaction<'static>>,
        pub(super) da_commitments: Option<DaCommitmentBundle>,
        pub(super) da_proof_policies: Option<DaProofPolicyBundle>,
        pub(super) da_pin_intents: Option<DaPinIntentBundle>,
        pub(super) npos_consensus_effects: Option<NposConsensusEffects>,
        pub(super) global_beacon_pulse:
            Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
        pub(super) execution_context: Option<BlockExecutionContextBundle>,
    }
    impl NewBlock {
        /// Transition to [`ValidBlock`]. Skips static checks and only applies state changes.
        #[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]
        pub fn validate_and_record_transactions(
            self,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<ValidBlock> {
            // Future pipeline overlap: the scheduler can pre-validate on a snapshot pinned at
            // height N-1 while proposing block N. For now we keep the simple path to preserve
            // deterministic behaviour; see specs/new_pipeline.md for the staged rollout.
            ValidBlock::validate_unchecked(self.into(), state_block)
        }
        /// Block signature
        pub fn signature(&self) -> &BlockSignature {
            &self.signature
        }
        /// Block header
        pub fn header(&self) -> BlockHeader {
            self.header
        }
        /// Block transactions
        pub fn transactions(&self) -> &[AcceptedTransaction<'_>] {
            &self.transactions
        }
        /// DA commitments embedded in this block, if any.
        pub fn da_commitments(&self) -> Option<&DaCommitmentBundle> {
            self.da_commitments.as_ref()
        }
        /// DA proof policies embedded in this block, if any.
        pub fn da_proof_policies(&self) -> Option<&DaProofPolicyBundle> {
            self.da_proof_policies.as_ref()
        }
        /// DA pin intents embedded in this block, if any.
        pub fn da_pin_intents(&self) -> Option<&DaPinIntentBundle> {
            self.da_pin_intents.as_ref()
        }
        /// `NPoS` consensus effects embedded in this block, if any.
        pub fn npos_consensus_effects(&self) -> Option<&NposConsensusEffects> {
            self.npos_consensus_effects.as_ref()
        }
    }
    impl From<NewBlock> for SignedBlock {
        fn from(block: NewBlock) -> Self {
            let mut external_entrypoints = Vec::with_capacity(block.transactions.len());
            for accepted in block.transactions {
                external_entrypoints.push(accepted.into_entrypoint());
            }
            SignedBlock::presigned_with_payload(
                block.signature,
                BlockPayload {
                    header: block.header,
                    external_entrypoints,
                    execution_context: block.execution_context,
                    da_commitments: block.da_commitments,
                    da_proof_policies: block.da_proof_policies,
                    da_pin_intents: block.da_pin_intents,
                    npos_consensus_effects: block.npos_consensus_effects,
                    global_beacon_pulse: block.global_beacon_pulse,
                },
            )
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{block::BlockBuilder, tx::AcceptedTransaction};
        use iroha_data_model::{isi::Log, transaction::TransactionBuilder};
        use iroha_logger::Level;
        use iroha_primitives::time::TimeSource;
        use iroha_test_samples::gen_account_in;
        use std::{borrow::Cow, time::Duration};
        #[test]
        fn into_signed_block_preserves_external_transactions_without_legacy_cache() {
            let network_id = deterministic_test_network_id(0x01);
            let (authority, keypair) = gen_account_in("wonderland");
            let tx1 = TransactionBuilder::new(
                network_id,
                authority.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "first".to_owned())])
            .sign(keypair.private_key());
            let tx2 = TransactionBuilder::new(
                network_id,
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "second".to_owned())])
            .sign(keypair.private_key());
            let mut submitted = vec![tx1, tx2];
            submitted.sort_by_key(|tx| core::cmp::Reverse(tx.hash_as_entrypoint()));
            assert!(submitted[0].hash_as_entrypoint() > submitted[1].hash_as_entrypoint());
            let expected = submitted.clone();
            let accepted = submitted
                .into_iter()
                .map(|tx| AcceptedTransaction::new_unchecked(Cow::Owned(tx)))
                .collect();
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(1));
            let builder = BlockBuilder::new_with_time_source(accepted, time_source);
            let block_signer = crate::block::checked_keypair();
            let new_block = builder
                .chain(0, None)
                .sign(block_signer.private_key())
                .unpack(|_| {});
            let signed_block: SignedBlock = new_block.into();
            assert_eq!(
                signed_block.external_transactions().collect::<Vec<_>>(),
                expected.iter().collect::<Vec<_>>(),
                "block construction must not replace FIFO order with grindable hash priority"
            );
        }
        #[test]
        fn block_builder_sign_with_index_sets_signature_index() {
            let (authority, keypair) = gen_account_in("wonderland");
            let tx = TransactionBuilder::new(
                deterministic_test_network_id(0x02),
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "signed".to_owned())])
            .sign(keypair.private_key());
            let accepted = vec![AcceptedTransaction::new_unchecked(Cow::Owned(tx))];
            let builder = BlockBuilder::new(accepted);
            let signer = crate::block::checked_keypair();
            let signatory_idx = 7_u64;
            let new_block = builder
                .chain(0, None)
                .sign_with_index(signer.private_key(), signatory_idx)
                .unpack(|_| {});
            assert_eq!(new_block.signature().index(), signatory_idx);
        }
        #[test]
        fn block_builder_try_sign_with_index_sets_verifiable_signature() {
            let (authority, keypair) = gen_account_in("wonderland");
            let tx = TransactionBuilder::new(
                deterministic_test_network_id(0x03),
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "try-signed".to_owned())])
            .sign(keypair.private_key());
            let accepted = vec![AcceptedTransaction::new_unchecked(Cow::Owned(tx))];
            let builder = BlockBuilder::new(accepted);
            let signer = crate::block::checked_keypair();
            let signatory_idx = 11_u64;
            let new_block = builder
                .chain(0, None)
                .try_sign_with_index(signer.private_key(), signatory_idx)
                .expect("fallible block signing succeeds")
                .unpack(|_| {});
            assert_eq!(new_block.signature().index(), signatory_idx);
            new_block
                .signature()
                .signature()
                .verify_hash(signer.public_key(), new_block.header().hash())
                .expect("fallibly signed block signature verifies");
        }
    }
}
pub(crate) mod valid {
    #[cfg(test)]
    #[path = "admission_batching.rs"]
    mod admission_batching_tests;
    #[cfg(test)]
    use super::event::{map_block_err_to_reason, map_sig_err_to_reason};
    use super::{event::emit_block_rejection, *};
    use crate::state::{StateBlock, storage_transactions::TransactionsReadOnly};
    use crate::sumeragi::network_topology::Role;
    use commit::CommittedBlock;
    use iroha_data_model::account::address::{ChainDiscriminantGuard, chain_discriminant};
    use iroha_data_model::events::pipeline::PipelineEventBox;
    use iroha_data_model::nexus::AxtPolicySnapshot;
    #[cfg(test)]
    use iroha_model_base::chain::ChainId;
    use iroha_primitives::time::TimeSource;
    use std::{num::NonZeroUsize, time::Instant};
    /// Block that was validated and accepted.
    #[derive(Debug, Clone)]
    pub struct ValidBlock {
        block: SignedBlock,
        signatures_verified: bool,
    }
    /// Timing breakdown for block validation stages.
    #[derive(Debug, Clone, Copy, Default)]
    #[allow(clippy::struct_field_names)]
    pub struct ValidationTimings {
        /// Elapsed milliseconds for stateless checks.
        pub(crate) stateless_ms: u64,
        /// Elapsed milliseconds spent in state-dependent stateless checks.
        pub(crate) stateless_state_dependent_ms: u64,
        /// Elapsed milliseconds spent in snapshot-based stateless checks.
        pub(crate) stateless_snapshot_ms: u64,
        /// Elapsed milliseconds for execution/stateful checks.
        pub(crate) execution_ms: u64,
        /// Elapsed milliseconds spent ensuring DA indexes are hydrated.
        pub(crate) execution_da_indexes_ms: u64,
        /// Elapsed milliseconds spent creating the state block.
        pub(crate) execution_state_block_ms: u64,
        /// Elapsed milliseconds spent executing transactions.
        pub(crate) execution_tx_ms: u64,
        /// Elapsed milliseconds spent applying overlays and finalizing results.
        pub(crate) execution_tx_apply_ms: u64,
        /// Elapsed milliseconds spent in the sequential apply path (when parallel apply is off).
        pub(crate) execution_tx_apply_sequential_ms: u64,
        /// Elapsed milliseconds spent validating AXT envelopes.
        pub(crate) execution_axt_ms: u64,
        /// Elapsed milliseconds spent validating DA shard cursors.
        pub(crate) execution_da_cursor_ms: u64,
        /// Elapsed milliseconds spent checking genesis transaction invariants.
        pub(crate) execution_genesis_clean_ms: u64,
        /// Total elapsed milliseconds for validation.
        pub(crate) total_ms: u64,
    }
    impl ValidationTimings {
        /// Create an empty timing snapshot.
        #[cfg(test)]
        pub(crate) fn new() -> Self {
            Self::default()
        }
    }
    type Error = (Box<SignedBlock>, Box<BlockValidationError>);
    #[cfg(feature = "telemetry")]
    type MetricsRef<'a> = Option<&'a crate::telemetry::StateTelemetry>;
    #[cfg(not(feature = "telemetry"))]
    type MetricsRef<'a> = ();
    #[derive(Debug)]
    struct StaticValidationData {
        expected_block_height: usize,
        max_clock_drift: Duration,
        tx_params: iroha_data_model::parameter::TransactionParameters,
        crypto_cfg: Arc<iroha_config::parameters::actual::Crypto>,
        pipeline_cfg: iroha_config::parameters::actual::Pipeline,
        pipeline_parallelism: crate::state::PipelineParallelism,
        aggregate_lane: LaneId,
    }
    #[derive(Debug, PartialEq, Eq)]
    enum ConsensusValidationProfile {
        /// A block ordered by the Sumeragi core (`specs/sumeragi.md` §4): the certified
        /// core header binds the nonempty payload, so block signatures are not checked;
        /// block time is canonical from the parent and
        /// the cadence; nothing depends on a v2 height context.
        Sumeragi {
            block_cadence: Duration,
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode,
            supplied_pulse:
                Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
            /// The block's lane step input (`specs/sumeragi_lanes.md` §4.3).
            lanes: crate::sumeragi::lanes::merge::LaneStepInput,
            expected_pulse_context:
                iroha_data_model::consensus::GlobalThresholdBeaconPulseContextV1,
            source_generation: u64,
        },
        /// The sole signed genesis path; installs the native consensus schedule and lane step.
        SumeragiGenesis {
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode,
        },
    }
    impl ConsensusValidationProfile {
        /// The genesis height when the block advances the Sumeragi schedule
        /// (`specs/sumeragi.md` §10).
        const fn sumeragi_schedule(&self) -> Option<u64> {
            match self {
                Self::Sumeragi { .. } | Self::SumeragiGenesis { .. } => {
                    Some(crate::sumeragi::startup::GENESIS_HEIGHT)
                }
                _ => None,
            }
        }
        /// Lane lifecycle input committed by this global block.
        fn take_sumeragi_lanes(&mut self) -> Option<crate::sumeragi::lanes::merge::LaneStepInput> {
            match self {
                Self::Sumeragi { lanes, .. } => Some(std::mem::take(lanes)),
                Self::SumeragiGenesis { .. } => Some(Default::default()),
                _ => None,
            }
        }
        /// The exact transported native control witness, never a local aggregator lookup.
        const fn sumeragi_pulse(
            &self,
        ) -> Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1> {
            match self {
                Self::Sumeragi { supplied_pulse, .. } => *supplied_pulse,
                _ => None,
            }
        }
        /// The authenticated native header source, absent only for signed genesis.
        const fn sumeragi_pulse_context(
            &self,
        ) -> Option<iroha_data_model::consensus::GlobalThresholdBeaconPulseContextV1> {
            match self {
                Self::Sumeragi {
                    expected_pulse_context,
                    ..
                } => Some(*expected_pulse_context),
                _ => None,
            }
        }
        const fn source_generation(&self) -> Option<u64> {
            match self {
                Self::Sumeragi {
                    source_generation, ..
                } => Some(*source_generation),
                _ => None,
            }
        }

        const fn enforce_local_wall_clock(&self) -> bool {
            matches!(self, Self::SumeragiGenesis { .. })
        }
        const fn block_cadence(&self) -> Option<Duration> {
            match self {
                Self::Sumeragi { block_cadence, .. } => Some(*block_cadence),
                _ => None,
            }
        }

        fn sccp_height_source(
            &self,
        ) -> crate::smartcontracts::isi::sccp::height::SccpHeightSourceV1<'_> {
            use crate::smartcontracts::isi::sccp::height::SccpHeightSourceV1;
            self.sumeragi_schedule()
                .map_or(SccpHeightSourceV1::Unauthenticated, |genesis_height| {
                    SccpHeightSourceV1::SumeragiSchedule {
                        genesis_height,
                        mode: self.authoritative_consensus_mode(),
                    }
                })
        }

        const fn authoritative_consensus_mode(
            &self,
        ) -> iroha_data_model::parameter::system::ConsensusMode {
            match self {
                Self::Sumeragi { consensus_mode, .. }
                | Self::SumeragiGenesis { consensus_mode } => *consensus_mode,
            }
        }
    }
    #[cfg(all(test, feature = "app_api"))]
    std::thread_local! {
        static AXT_FASTPQ_PROOF_VERIFICATION_COUNT: std::cell::Cell<usize> =
            const { std::cell::Cell::new(0) };
    }
    #[cfg(all(test, feature = "app_api"))]
    pub(super) fn reset_axt_fastpq_proof_verification_count() {
        AXT_FASTPQ_PROOF_VERIFICATION_COUNT.with(|count| count.set(0));
    }
    #[cfg(all(test, feature = "app_api"))]
    pub(super) fn axt_fastpq_proof_verification_count() -> usize {
        AXT_FASTPQ_PROOF_VERIFICATION_COUNT.with(std::cell::Cell::get)
    }
    pub(super) fn map_axt_fastpq_error(
        error: fastpq_prover::Error,
        context: &str,
        snapshot_version: u64,
        dataspace: DataSpaceId,
        lane: LaneId,
    ) -> BlockValidationError {
        if matches!(
            error,
            fastpq_prover::Error::LocalAllocationUnavailable { .. }
        ) {
            return BlockValidationError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable.into(),
            );
        }
        BlockValidationError::AxtEnvelopeValidationFailed(AxtEnvelopeValidationDetails {
            message: format!("{context}: {error}"),
            reason: AxtRejectReason::Proof,
            snapshot_version: Some(snapshot_version),
            dataspace: Some(dataspace),
            lane: Some(lane),
            active_handle_era: None,
            next_handle_counter: None,
        })
    }
    #[allow(clippy::too_many_lines)]
    pub fn validate_axt_envelopes<'block>(
        block: &'block SignedBlock,
        state_block: &StateBlock<'_>,
    ) -> Result<(), BlockValidationError> {
        let advertised_snapshot = block.axt_policy_snapshot().cloned().ok_or_else(|| {
            BlockValidationError::AxtEnvelopeValidationFailed(AxtEnvelopeValidationDetails {
                message: "block result is missing its required AXT policy snapshot".to_owned(),
                reason: AxtRejectReason::MissingPolicy,
                snapshot_version: None,
                dataspace: None,
                lane: None,
                active_handle_era: None,
                next_handle_counter: None,
            })
        })?;
        let advertised_transitioned_dataspaces =
            block.axt_transitioned_dataspaces().ok_or_else(|| {
                BlockValidationError::AxtEnvelopeValidationFailed(AxtEnvelopeValidationDetails {
                    message: "block result is missing its required AXT transition set".to_owned(),
                    reason: AxtRejectReason::PolicyDenied,
                    snapshot_version: Some(advertised_snapshot.version),
                    dataspace: None,
                    lane: None,
                    active_handle_era: None,
                    next_handle_counter: None,
                })
            })?;
        let snapshot_version = advertised_snapshot.version;
        let make_axt_error_with =
            |reason: AxtRejectReason,
             message: &str,
             dataspace: Option<DataSpaceId>,
             lane: Option<LaneId>,
             active_handle_era: Option<u64>,
             next_handle_counter: Option<u64>| {
                BlockValidationError::AxtEnvelopeValidationFailed(AxtEnvelopeValidationDetails {
                    message: message.to_owned(),
                    reason,
                    snapshot_version: Some(snapshot_version),
                    dataspace,
                    lane,
                    active_handle_era,
                    next_handle_counter,
                })
            };
        advertised_snapshot.validate().map_err(|error| {
            make_axt_error_with(
                AxtRejectReason::PolicyDenied,
                &format!("invalid AXT policy snapshot: {error}"),
                None,
                None,
                None,
                None,
            )
        })?;
        let block_start = state_block.axt_block_start_snapshot();
        let snapshot = block_start.policy_snapshot();
        snapshot.validate().map_err(|error| {
            make_axt_error_with(
                AxtRejectReason::PolicyDenied,
                &format!("invalid block-start AXT policy snapshot: {error}"),
                None,
                None,
                None,
                None,
            )
        })?;
        let axt_timing = state_block.nexus.axt;
        let policies: BTreeMap<_, _> = snapshot
            .entries
            .iter()
            .map(|binding| (binding.dsid, binding.policy))
            .collect();
        let advertised_policies: BTreeMap<_, _> = advertised_snapshot
            .entries
            .iter()
            .map(|binding| (binding.dsid, binding.policy))
            .collect();
        let next_sub_nonces: BTreeMap<DataSpaceId, u64> = policies
            .iter()
            .map(|(dsid, policy)| (*dsid, policy.next_handle_counter))
            .collect();
        if let Some(envelopes) = block.axt_envelopes() {
            if let Some(envelope) = envelopes
                .iter()
                .find(|envelope| !envelope.spends.is_empty())
            {
                return Err(make_axt_error_with(
                    AxtRejectReason::Proof,
                    crate::fastpq::AXT_UNANCHORED_REMOTE_SPEND_REJECTION,
                    Some(envelope.spends[0].draft.intent.asset_dsid),
                    Some(envelope.lane),
                    None,
                    None,
                ));
            }
            let validate_proof_expiry = |proof: &ProofBlob,
                                         dsid: DataSpaceId,
                                         policy: &AxtPolicyEntry,
                                         policy_slot: u64,
                                         min_expiry_slot: Option<u64>|
             -> Result<(), BlockValidationError> {
                if let Some(expiry_slot) = proof.expiry_slot {
                    if expiry_slot == 0 {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Proof,
                            "proof expiry slot is zero",
                            Some(dsid),
                            Some(policy.target_lane),
                            None,
                            None,
                        ));
                    }
                    let expiry_deadline = ivm::axt::expiry_slot_with_skew(
                        expiry_slot,
                        axt_timing.slot_length_ms,
                        axt_timing.max_clock_skew_ms,
                        None,
                    );
                    if policy_slot > 0 && policy_slot > expiry_deadline {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Expiry,
                            "proof expired relative to policy slot",
                            Some(dsid),
                            Some(policy.target_lane),
                            None,
                            None,
                        ));
                    }
                    if let Some(min_expiry) = min_expiry_slot {
                        if min_expiry > expiry_slot {
                            return Err(make_axt_error_with(
                                AxtRejectReason::Expiry,
                                "proof expires before handle",
                                Some(dsid),
                                Some(policy.target_lane),
                                None,
                                None,
                            ));
                        }
                    }
                }
                Ok(())
            };
            struct VerifiedProof<'block> {
                proof: &'block ProofBlob,
                dsid: DataSpaceId,
                facts: ivm::axt::AxtProofUseFacts,
            }
            type VerifiedProofBuckets = BTreeMap<(Option<u64>, Hash), Vec<usize>>;
            // Digest buckets make repeated exact-value lookups independent of the
            // number and size of unrelated proofs. The full-value equality check
            // inside a matching bucket keeps hash collisions fail-safe, while
            // references avoid copying proof payloads owned by the block.
            let mut verified_proofs = Vec::<VerifiedProof<'block>>::new();
            let mut verified_proof_buckets = VerifiedProofBuckets::new();
            let validate_proof = |verified_proofs: &mut Vec<VerifiedProof<'block>>,
                                  verified_proof_buckets: &mut VerifiedProofBuckets,
                                  proof: &'block ProofBlob,
                                  dsid: DataSpaceId,
                                  policy: &AxtPolicyEntry,
                                  policy_slot: u64,
                                  min_expiry_slot: Option<u64>|
             -> Result<usize, BlockValidationError> {
                if crate::fastpq::axt_proof_payload_exceeds_decode_limit(&proof.payload) {
                    return Err(make_axt_error_with(
                        AxtRejectReason::Proof,
                        &format!(
                            "proof payload exceeds the {}-byte decode limit",
                            fastpq_prover::MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES,
                        ),
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    ));
                }
                let cache_key = (proof.expiry_slot, Hash::new(&proof.payload));
                let cached_index = verified_proof_buckets.get(&cache_key).and_then(|bucket| {
                    bucket
                        .iter()
                        .copied()
                        .find(|index| verified_proofs[*index].proof == proof)
                });
                if let Some(index) = cached_index {
                    if verified_proofs[index].dsid != dsid {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Manifest,
                            "one exact FASTPQ proof cannot be relabelled for another dataspace",
                            Some(dsid),
                            Some(policy.target_lane),
                            None,
                            None,
                        ));
                    }
                    validate_proof_expiry(proof, dsid, policy, policy_slot, min_expiry_slot)?;
                    return Ok(index);
                }
                if proof.payload.is_empty() {
                    return Err(make_axt_error_with(
                        AxtRejectReason::Proof,
                        "empty proof payload",
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    ));
                }
                if policy.manifest_root.iter().all(|byte| *byte == 0) {
                    return Err(make_axt_error_with(
                        AxtRejectReason::Manifest,
                        "policy manifest root is zeroed",
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    ));
                }
                let envelope =
                    ivm::codec::decode_canonical_norito::<AxtProofEnvelope>(&proof.payload)
                        .map_err(|err| {
                            make_axt_error_with(
                                AxtRejectReason::Proof,
                                &format!("proof payload is not an AXT proof envelope: {err}"),
                                Some(dsid),
                                Some(policy.target_lane),
                                None,
                                None,
                            )
                        })?;
                if envelope.dsid != dsid || envelope.manifest_root != policy.manifest_root {
                    return Err(make_axt_error_with(
                        AxtRejectReason::Manifest,
                        "proof does not match policy manifest root",
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    ));
                }
                let binding = envelope.fastpq_binding.as_ref().ok_or_else(|| {
                    make_axt_error_with(
                        AxtRejectReason::Proof,
                        "FASTPQ proof envelope is missing fastpq_binding",
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    )
                })?;
                fastpq_prover::validate_axt_transfer_claim_binding(binding).map_err(|err| {
                    map_axt_fastpq_error(
                        err,
                        "generic AXT proof admission requires a witnessed transfer claim",
                        snapshot_version,
                        dsid,
                        policy.target_lane,
                    )
                })?;
                validate_proof_expiry(proof, dsid, policy, policy_slot, min_expiry_slot)?;
                #[cfg(all(test, feature = "app_api"))]
                AXT_FASTPQ_PROOF_VERIFICATION_COUNT
                    .with(|count| count.set(count.get().saturating_add(1)));
                fastpq_prover::verify_axt_proof_envelope_with_outer_metadata(
                    &envelope,
                    proof.expiry_slot,
                )
                .map_err(|err| {
                    map_axt_fastpq_error(
                        err,
                        "FASTPQ verification failed",
                        snapshot_version,
                        dsid,
                        policy.target_lane,
                    )
                })?;
                let index = verified_proofs.len();
                verified_proofs.push(VerifiedProof {
                    proof,
                    dsid,
                    facts: ivm::axt::AxtProofUseFacts::from_verified_envelope(envelope),
                });
                verified_proof_buckets
                    .entry(cache_key)
                    .or_default()
                    .push(index);
                Ok(index)
            };
            let make_env_error =
                |lane: LaneId,
                 reason: AxtRejectReason,
                 message: &str,
                 dsid: Option<DataSpaceId>,
                 active_handle_era: Option<u64>,
                 next_handle_counter: Option<u64>| {
                    make_axt_error_with(
                        reason,
                        message,
                        dsid,
                        Some(lane),
                        active_handle_era,
                        next_handle_counter,
                    )
                };
            for envelope in envelopes {
                let envelope_lane = envelope.lane;
                let expected_commit_height = block.header().height().get();
                if envelope.commit_height != expected_commit_height {
                    return Err(make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        &format!(
                            "envelope commit height {} does not match block height {expected_commit_height}",
                            envelope.commit_height
                        ),
                        None,
                        None,
                        None,
                    ));
                }
                if let Err(err) = iroha_data_model::nexus::validate_descriptor(&envelope.descriptor)
                {
                    return Err(make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        &format!("invalid descriptor: {err}"),
                        None,
                        None,
                        None,
                    ));
                }
                let expected_binding = envelope.descriptor.binding().map_err(|err| {
                    make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        &format!("failed to compute descriptor binding: {err}"),
                        None,
                        None,
                        None,
                    )
                })?;
                if expected_binding != envelope.binding {
                    return Err(make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        "descriptor binding does not match envelope binding",
                        None,
                        None,
                        None,
                    ));
                }
                let expected_dsids: BTreeSet<_> =
                    envelope.descriptor.dsids.iter().copied().collect();
                let mut touch_specs: BTreeMap<DataSpaceId, &iroha_data_model::nexus::AxtTouchSpec> =
                    BTreeMap::new();
                for spec in &envelope.descriptor.touches {
                    touch_specs.insert(spec.dsid, spec);
                }
                if envelope
                    .touches
                    .windows(2)
                    .any(|pair| pair[0].dsid >= pair[1].dsid)
                {
                    return Err(make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        "touch fragments are not strictly ordered by dataspace",
                        None,
                        None,
                        None,
                    ));
                }
                let mut touch_dsids: BTreeSet<DataSpaceId> = BTreeSet::new();
                for touch in &envelope.touches {
                    if ivm::axt::validate_model_touch_manifest(&touch.manifest).is_err() {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "touch manifest paths are not canonical",
                            Some(touch.dsid),
                            None,
                            None,
                        ));
                    }
                    if !expected_dsids.contains(&touch.dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "touch references undeclared dataspace",
                            Some(touch.dsid),
                            None,
                            None,
                        ));
                    }
                    if !touch_dsids.insert(touch.dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "duplicate touch manifest for dataspace",
                            Some(touch.dsid),
                            None,
                            None,
                        ));
                    }
                    if let Some(spec) = touch_specs.get(&touch.dsid) {
                        if (!spec.read.is_empty() || !spec.write.is_empty())
                            && touch.manifest.read.is_empty()
                            && touch.manifest.write.is_empty()
                        {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Descriptor,
                                "missing touch manifest for dataspace",
                                Some(touch.dsid),
                                None,
                                None,
                            ));
                        }
                        if !touch
                            .manifest
                            .read
                            .iter()
                            .all(|entry| spec.read.iter().any(|prefix| entry.starts_with(prefix)))
                        {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Descriptor,
                                "touch manifest read entry outside descriptor",
                                Some(touch.dsid),
                                None,
                                None,
                            ));
                        }
                        if !touch
                            .manifest
                            .write
                            .iter()
                            .all(|entry| spec.write.iter().any(|prefix| entry.starts_with(prefix)))
                        {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Descriptor,
                                "touch manifest write entry outside descriptor",
                                Some(touch.dsid),
                                None,
                                None,
                            ));
                        }
                    } else if !touch.manifest.read.is_empty() || !touch.manifest.write.is_empty() {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "touch manifest provided without descriptor spec",
                            Some(touch.dsid),
                            None,
                            None,
                        ));
                    }
                }
                for spec in &envelope.descriptor.touches {
                    if (!spec.read.is_empty() || !spec.write.is_empty())
                        && !touch_dsids.contains(&spec.dsid)
                    {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "missing touch manifest for dataspace",
                            Some(spec.dsid),
                            None,
                            None,
                        ));
                    }
                }
                let mut proofs_by_ds: BTreeMap<DataSpaceId, (&ProofBlob, usize)> = BTreeMap::new();
                if envelope
                    .proofs
                    .windows(2)
                    .any(|pair| pair[0].dsid >= pair[1].dsid)
                {
                    return Err(make_env_error(
                        envelope_lane,
                        AxtRejectReason::Proof,
                        "proof fragments are not strictly ordered by dataspace",
                        None,
                        None,
                        None,
                    ));
                }
                for proof in &envelope.proofs {
                    if !expected_dsids.contains(&proof.dsid) {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Descriptor,
                            "proof references undeclared dataspace",
                            Some(proof.dsid),
                            Some(envelope_lane),
                            None,
                            None,
                        ));
                    }
                    let policy = policies.get(&proof.dsid).ok_or_else(|| {
                        make_axt_error_with(
                            AxtRejectReason::MissingPolicy,
                            "no policy for dataspace",
                            Some(proof.dsid),
                            Some(envelope_lane),
                            None,
                            None,
                        )
                    })?;
                    let proof_index = validate_proof(
                        &mut verified_proofs,
                        &mut verified_proof_buckets,
                        &proof.proof,
                        proof.dsid,
                        policy,
                        policy.current_slot,
                        None,
                    )?;
                    if proofs_by_ds
                        .insert(proof.dsid, (&proof.proof, proof_index))
                        .is_some()
                    {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Proof,
                            "duplicate proof for dataspace",
                            Some(proof.dsid),
                            Some(envelope_lane),
                            None,
                            None,
                        ));
                    }
                }
                // No signed spend may consume a claim until finalized source
                // execution can be authenticated by State.
                let consumed_by_proof = proofs_by_ds
                    .iter()
                    .map(|(dsid, (_, proof_index))| (*proof_index, (*dsid, Vec::new())))
                    .collect::<BTreeMap<usize, (DataSpaceId, Vec<[u8; 32]>)>>();
                let dataspace_proofs_present: BTreeSet<DataSpaceId> =
                    proofs_by_ds.keys().copied().collect();
                for dsid in &expected_dsids {
                    if !dataspace_proofs_present.contains(dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Proof,
                            "proof missing for dataspace",
                            Some(*dsid),
                            None,
                            None,
                        ));
                    }
                }
                for (proof_index, (dsid, consumed)) in consumed_by_proof {
                    verified_proofs[proof_index]
                        .facts
                        .validate_remote_spend_consumption(&consumed)
                        .map_err(|_| {
                            make_env_error(
                                envelope_lane,
                                AxtRejectReason::Proof,
                                "FASTPQ remote-spend claims were not consumed exactly once",
                                Some(dsid),
                                None,
                                None,
                            )
                        })?;
                }
            }
            // Proofs with remote-spend commitments cannot be consumed while
            // anchored source execution admission is unavailable.
            for verified in &verified_proofs {
                verified
                    .facts
                    .validate_remote_spend_consumption(&[])
                    .map_err(|_| {
                        make_axt_error_with(
                            AxtRejectReason::Proof,
                            "FASTPQ remote-spend claims require anchored admission",
                            Some(verified.dsid),
                            None,
                            None,
                            None,
                        )
                    })?;
            }
        }
        let ratchet_dataspaces: BTreeSet<_> = advertised_policies
            .keys()
            .chain(policies.keys())
            .chain(advertised_transitioned_dataspaces.iter())
            .copied()
            .collect();
        for dsid in ratchet_dataspaces {
            let advertised_policy = advertised_policies.get(&dsid);
            let previous_policy = policies.get(&dsid);
            let counter_before_block = state_block.axt_handle_counter_at_block_start(&dsid);
            let reconstructed_next = next_sub_nonces
                .get(&dsid)
                .copied()
                .or_else(|| {
                    counter_before_block.map(iroha_data_model::nexus::AxtHandleCounterRecord::next)
                })
                .unwrap_or(0);
            let candidate_policy = state_block.world.axt_policies.get(&dsid);
            let error_lane = advertised_policy
                .or(candidate_policy)
                .or(previous_policy)
                .map(|policy| policy.target_lane);
            let minimum_generation = crate::state::axt_policy_generation_minimum(
                &state_block.world,
                dsid,
                candidate_policy,
            );
            let base_generation = counter_before_block
                .map(iroha_data_model::nexus::AxtHandleCounterRecord::authorization_generation)
                .or_else(|| previous_policy.map(|policy| policy.active_handle_era))
                .unwrap_or(minimum_generation);
            let current_counter = if reconstructed_next == 0 {
                None
            } else {
                Some(
                    iroha_data_model::nexus::AxtHandleCounterRecord::try_from_parts(
                        reconstructed_next,
                        base_generation,
                    )
                    .map_err(|_| {
                        make_axt_error_with(
                            AxtRejectReason::SubNonce,
                            "invalid reconstructed AXT permanent counter",
                            Some(dsid),
                            error_lane,
                            Some(base_generation),
                            Some(reconstructed_next),
                        )
                    })?,
                )
            };
            let authorization_identity_changed = advertised_transitioned_dataspaces.contains(&dsid)
                || block_start.authorization_identity_changed(
                    &state_block.world,
                    &state_block.lane_incarnations,
                    dsid,
                    candidate_policy,
                );
            let reconstructed = crate::state::axt_counter_after_block_boundary(
                previous_policy,
                candidate_policy,
                minimum_generation,
                authorization_identity_changed,
                counter_before_block.copied(),
                current_counter,
            )
            .map_err(|_| {
                make_axt_error_with(
                    AxtRejectReason::SubNonce,
                    "AXT permanent counter is exhausted during policy revocation",
                    Some(dsid),
                    error_lane,
                    Some(base_generation),
                    Some(reconstructed_next),
                )
            })?;
            let expected_next = reconstructed
                .as_ref()
                .map_or(0, iroha_data_model::nexus::AxtHandleCounterRecord::next);
            let expected_generation = reconstructed.as_ref().map_or(
                0,
                iroha_data_model::nexus::AxtHandleCounterRecord::authorization_generation,
            );
            if let Some(advertised_policy) = advertised_policy {
                if expected_next != advertised_policy.next_handle_counter
                    || expected_generation != advertised_policy.active_handle_era
                {
                    return Err(make_axt_error_with(
                        AxtRejectReason::SubNonce,
                        "authenticated AXT execution does not equal the committed permanent authorization-generation ratchet",
                        Some(dsid),
                        Some(advertised_policy.target_lane),
                        Some(expected_generation),
                        Some(expected_next),
                    ));
                }
            }
            match (
                state_block.world.axt_handle_counters.get(&dsid),
                reconstructed.as_ref(),
            ) {
                (Some(final_ratchet), Some(expected))
                    if final_ratchet.next() == expected_next
                        && final_ratchet.authorization_generation() == expected_generation
                        && final_ratchet == expected => {}
                (None, None) => {}
                _ => {
                    return Err(make_axt_error_with(
                        AxtRejectReason::SubNonce,
                        "authenticated AXT execution does not equal the committed authorization-generation ratchet",
                        Some(dsid),
                        error_lane,
                        Some(expected_generation),
                        Some(expected_next),
                    ));
                }
            }
        }
        Ok(())
    }
    /// Counts of signatures attached to a block.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SignatureTally {
        /// Total signatures present on the block (all roles).
        pub present: usize,
        /// Deduplicated signatures from commit-eligible roles (leader, validators, set-B, proxy tail).
        pub counted: usize,
        /// Signatures contributed by set-B validators (role `SetBValidator`).
        pub set_b_signatures: usize,
    }
    /// Build a signature tally for the given block under the provided topology.
    pub fn commit_signature_tally(block: &SignedBlock, topology: &Topology) -> SignatureTally {
        let mut counted = BTreeSet::new();
        let commit_roles: &[Role] = &[
            Role::Leader,
            Role::ProxyTail,
            Role::ValidatingPeer,
            Role::SetBValidator,
        ];
        for signature in topology.filter_signatures_by_roles(commit_roles, block.signatures()) {
            if let Ok(idx) = usize::try_from(signature.index()) {
                counted.insert(idx);
            }
        }
        let set_b_signatures = topology
            .filter_signatures_by_roles(&[Role::SetBValidator], block.signatures())
            .count();
        SignatureTally {
            present: block.signatures().count(),
            counted: counted.len(),
            set_b_signatures,
        }
    }
    /// Original committed cut admitted before the constructor acquires execution writers.
    struct PristineNativeSource<'state> {
        state: &'state State,
        generation: u64,
        header: BlockHeader,
    }
    include!("block/native_header_source.rs");
    include!("block/native_genesis_policy.rs");
    #[cfg(test)]
    #[path = "native_header_source_tests.rs"]
    mod native_header_source_tests;

    include!("block/post_execution_tail.rs");

    impl ValidBlock {
        #[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]
        fn new_unverified(block: SignedBlock) -> Self {
            Self {
                block,
                signatures_verified: false,
            }
        }
        #[cfg(test)]
        pub(crate) fn new_unverified_for_tests(block: SignedBlock) -> Self {
            Self::new_unverified(block)
        }
        fn new_signatures_verified(block: SignedBlock) -> Self {
            Self {
                block,
                signatures_verified: true,
            }
        }
        #[cfg(test)]
        pub(crate) fn committed_from_replay_signed_block(block: SignedBlock) -> CommittedBlock {
            Self::new_signatures_verified(block)
                .commit_unchecked()
                .unpack(|_| {})
        }
        #[cfg(test)]
        fn mark_signatures_verified(&mut self) {
            self.signatures_verified = true;
        }
        fn clear_signatures_verified(&mut self) {
            self.signatures_verified = false;
        }
        fn validate_advertised_axt_post_state(
            advertised: Option<&AxtPolicySnapshot>,
            computed: &AxtPolicySnapshot,
        ) -> Result<(), BlockValidationError> {
            let Some(advertised) = advertised else {
                return Ok(());
            };
            if advertised == computed {
                return Ok(());
            }
            Err(BlockValidationError::AxtEnvelopeValidationFailed(
                AxtEnvelopeValidationDetails {
                    message: "advertised AXT post-state policy snapshot does not match deterministic execution"
                        .to_owned(),
                    reason: AxtRejectReason::PolicyDenied,
                    snapshot_version: Some(advertised.version),
                    dataspace: None,
                    lane: None,
                    active_handle_era: None,
                    next_handle_counter: None,
                },
            ))
        }
        fn validate_advertised_axt_transitions(
            advertised: Option<&BTreeSet<DataSpaceId>>,
            computed: &BTreeSet<DataSpaceId>,
            snapshot_version: u64,
        ) -> Result<(), BlockValidationError> {
            let Some(advertised) = advertised else {
                return Ok(());
            };
            if advertised == computed {
                return Ok(());
            }
            Err(BlockValidationError::AxtEnvelopeValidationFailed(
                AxtEnvelopeValidationDetails {
                    message: "advertised AXT transition set does not match deterministic execution"
                        .to_owned(),
                    reason: AxtRejectReason::PolicyDenied,
                    snapshot_version: Some(snapshot_version),
                    dataspace: None,
                    lane: None,
                    active_handle_era: None,
                    next_handle_counter: None,
                },
            ))
        }
        #[cfg(test)]
        fn signatures_verified_for_tests(&self) -> bool {
            self.signatures_verified
        }
        fn verify_unique_signers(block: &SignedBlock) -> Result<(), SignatureVerificationError> {
            let mut seen = BTreeSet::new();
            for signature in block.signatures() {
                let signer = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                if !seen.insert(signer) {
                    return Err(SignatureVerificationError::DuplicateSignature { signer });
                }
            }
            Ok(())
        }
        fn is_bls_normal_public_key(public_key: &PublicKey) -> bool {
            crate::crypto_util::is_bls_normal_public_key(public_key)
        }
        fn verify_leader_signature(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            use SignatureVerificationError::{LeaderMissing, UnknownSignature};
            // Enforce BLS-normal for leader
            if !Self::is_bls_normal_public_key(topology.leader().public_key()) {
                return Err(LeaderMissing);
            }
            let Some(signature) = topology
                .filter_signatures_by_roles(&[Role::Leader], block.signatures())
                .next()
            else {
                return Err(LeaderMissing);
            };
            signature
                .signature()
                .verify_hash(topology.leader().public_key(), block.hash())
                .map_err(|_err| UnknownSignature)?;
            Ok(())
        }
        fn verify_validator_signatures(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            // Enforce BLS-normal for validator roles in Set A + Set B (including proxy tail).
            let valid_roles: &[Role] =
                &[Role::ValidatingPeer, Role::SetBValidator, Role::ProxyTail];
            topology
                .filter_signatures_by_roles(valid_roles, block.signatures())
                .try_for_each(|signature| {
                    use SignatureVerificationError::{UnknownSignatory, UnknownSignature};
                    let signatory =
                        usize::try_from(signature.index()).map_err(|_err| UnknownSignatory)?;
                    let signatory: &PeerId =
                        topology.as_ref().get(signatory).ok_or(UnknownSignatory)?;
                    if !Self::is_bls_normal_public_key(signatory.public_key()) {
                        return Err(UnknownSignature);
                    }
                    signature
                        .signature()
                        .verify_hash(signatory.public_key(), block.hash())
                        .map_err(|_err| UnknownSignature)?;
                    Ok(())
                })?;
            Ok(())
        }
        fn verify_no_undefined_signatures(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            if topology
                .filter_signatures_by_roles(&[Role::Undefined], block.signatures())
                .next()
                .is_some()
            {
                return Err(SignatureVerificationError::UnknownSignatory);
            }
            Ok(())
        }
        #[cfg(test)]
        fn verify_signer_set(
            topology: &Topology,
            signers: &BTreeSet<ValidatorIndex>,
            allow_quorum_bypass: bool,
        ) -> Result<(), SignatureVerificationError> {
            let roster_len = topology.as_ref().len();
            if roster_len <= 1 {
                return Ok(());
            }
            let min_votes_for_commit = topology.min_votes_for_commit();
            let mut seen = BTreeSet::new();
            for signer in signers {
                let signer = usize::try_from(*signer)
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                if signer >= roster_len {
                    return Err(SignatureVerificationError::UnknownSignatory);
                }
                if !seen.insert(signer) {
                    return Err(SignatureVerificationError::DuplicateSignature { signer });
                }
            }
            let votes_count = signers.len();
            if votes_count < min_votes_for_commit && !allow_quorum_bypass {
                return Err(SignatureVerificationError::NotEnoughSignatures {
                    votes_count,
                    min_votes_for_commit,
                });
            }
            Ok(())
        }
        /// Verify every signature present on the block against the provided topology.
        ///
        /// This only checks signatures that exist on the block; it does not require a particular
        /// role to be present and accepts partial signature sets as long as each entry is valid.
        fn verify_signatures_against_topology(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            let hash = block.hash();
            for signature in block.signatures() {
                let signatory = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                let peer = topology
                    .as_ref()
                    .get(signatory)
                    .ok_or(SignatureVerificationError::UnknownSignatory)?;
                let role = topology.role(peer);
                match role {
                    Role::Leader | Role::ValidatingPeer | Role::ProxyTail | Role::SetBValidator => {
                        if !Self::is_bls_normal_public_key(peer.public_key()) {
                            return Err(SignatureVerificationError::UnknownSignature);
                        }
                        signature
                            .signature()
                            .verify_hash(peer.public_key(), hash)
                            .map_err(|_| SignatureVerificationError::UnknownSignature)?;
                    }
                    Role::Undefined => return Err(SignatureVerificationError::UnknownSignatory),
                }
            }
            Ok(())
        }
        #[cfg(test)]
        fn verify_signatures_against_topology_with_pops(
            block: &SignedBlock,
            topology: &Topology,
            pops: &BTreeMap<PublicKey, Vec<u8>>,
        ) -> Result<(), SignatureVerificationError> {
            let hash = block.hash();
            let mut bls_normal_signatures: Vec<&[u8]> = Vec::new();
            let mut bls_normal_public_keys: Vec<&PublicKey> = Vec::new();
            let mut bls_normal_pops: Vec<&[u8]> = Vec::new();
            for signature in block.signatures() {
                let signatory = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                let peer = topology
                    .as_ref()
                    .get(signatory)
                    .ok_or(SignatureVerificationError::UnknownSignatory)?;
                let role = topology.role(peer);
                match role {
                    Role::Leader | Role::ValidatingPeer | Role::ProxyTail | Role::SetBValidator => {
                        if !Self::is_bls_normal_public_key(peer.public_key()) {
                            return Err(SignatureVerificationError::UnknownSignature);
                        }
                        let pop = pops
                            .get(peer.public_key())
                            .ok_or(SignatureVerificationError::MissingPop)?;
                        let signature_payload = signature.signature().payload();
                        iroha_crypto::Signature::try_from_bytes(signature_payload)
                            .map_err(|_| SignatureVerificationError::UnknownSignature)?;
                        bls_normal_signatures.push(signature_payload);
                        bls_normal_public_keys.push(peer.public_key());
                        bls_normal_pops.push(pop.as_slice());
                    }
                    Role::Undefined => return Err(SignatureVerificationError::UnknownSignatory),
                }
            }
            if !bls_normal_signatures.is_empty() {
                iroha_crypto::bls_normal_verify_aggregate_same_message_fast(
                    hash.as_ref(),
                    &bls_normal_signatures,
                    &bls_normal_public_keys,
                    &bls_normal_pops,
                )
                .map_err(|_| SignatureVerificationError::UnknownSignature)?;
            }
            Ok(())
        }
        /// Validate the signature set for the block against the provided topology and key registry.
        ///
        /// Unlike [`Self::is_commit`], this accepts partial signature sets and only enforces that
        /// each present signature is unique, maps to a known validator role, and uses a live
        /// consensus key.
        #[cfg(test)]
        pub(crate) fn validate_signatures_subset_world(
            block: &SignedBlock,
            topology: &Topology,
            world: &impl WorldReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            if block.header().is_genesis() {
                return Ok(());
            }
            Self::validate_signatures_subset_world_exact(block, topology, world)
        }
        /// Verify the exact stored signature indices and payloads, including at genesis.
        ///
        /// Certificate-bound replay uses this after authenticating the canonical block wire;
        /// signer-index recovery would change the certified payload and is therefore forbidden.
        #[cfg(test)]
        pub(crate) fn validate_signatures_subset_world_exact(
            block: &SignedBlock,
            topology: &Topology,
            world: &impl WorldReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            Self::verify_unique_signers(block)?;
            let params = world.parameters();
            let sumeragi = params.sumeragi();
            let height = block.header().height().get();
            if world.consensus_keys().is_empty() {
                Self::verify_signatures_against_topology(block, topology)?;
                return Self::enforce_consensus_key_lifecycle_world(block, topology, world);
            }
            let pops = Self::collect_validator_pops(
                world,
                height,
                sumeragi.key_overlap_grace_blocks,
                sumeragi.key_expiry_grace_blocks,
            )?;
            Self::verify_signatures_against_topology_with_pops(block, topology, &pops)?;
            Self::enforce_consensus_key_lifecycle_world(block, topology, world)
        }

        #[cfg(test)]
        pub(crate) fn validate_signatures_subset(
            block: &SignedBlock,
            topology: &Topology,
            state: &impl StateReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            Self::validate_signatures_subset_world(block, topology, state.world())
        }
        #[cfg(test)]
        fn collect_validator_pops(
            world: &impl WorldReadOnly,
            height: u64,
            overlap_grace_blocks: u64,
            expiry_grace_blocks: u64,
        ) -> Result<BTreeMap<PublicKey, Vec<u8>>, SignatureVerificationError> {
            let mut pops: BTreeMap<PublicKey, Vec<u8>> = BTreeMap::new();
            for (id, record) in world.consensus_keys().iter() {
                if id.role != ConsensusKeyRole::Validator {
                    continue;
                }
                if !record.is_live_at(height, overlap_grace_blocks, expiry_grace_blocks) {
                    continue;
                }
                if Self::is_bls_normal_public_key(&record.public_key) {
                    let Some(pop) = record.pop.as_ref() else {
                        return Err(SignatureVerificationError::MissingPop);
                    };
                    if let Some(existing) = pops.get(&record.public_key) {
                        if existing.as_slice() != pop.as_slice() {
                            return Err(SignatureVerificationError::Other);
                        }
                        continue;
                    }
                    pops.insert(record.public_key.clone(), pop.clone());
                }
            }
            Ok(pops)
        }
        pub(crate) fn enforce_consensus_key_lifecycle_world(
            block: &SignedBlock,
            topology: &Topology,
            world: &impl WorldReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            if block.header().is_genesis() {
                return Ok(());
            }
            // Skip enforcement until consensus keys are explicitly registered. Once any
            // registry entries exist, validators must present a live key for signing.
            if world.consensus_keys().is_empty() {
                return Ok(());
            }
            let params = world.parameters();
            let sumeragi = params.sumeragi();
            let overlap = sumeragi.key_overlap_grace_blocks;
            let expiry_grace = sumeragi.key_expiry_grace_blocks;
            let height = block.header().height().get();
            for signature in topology.filter_signatures_by_roles(
                &[
                    Role::ValidatingPeer,
                    Role::SetBValidator,
                    Role::Leader,
                    Role::ProxyTail,
                ],
                block.signatures(),
            ) {
                let signatory = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                let signatory = topology
                    .as_ref()
                    .get(signatory)
                    .ok_or(SignatureVerificationError::UnknownSignatory)?;
                let pk = signatory.public_key();
                let pk_label = pk.to_string();
                let mut found_index_record = false;
                let mut live = world
                    .consensus_keys_by_pk()
                    .get(&pk_label)
                    .is_some_and(|ids| {
                        ids.iter().any(|id| {
                            world.consensus_keys().get(id).is_some_and(|rec| {
                                found_index_record = true;
                                rec.id.role == ConsensusKeyRole::Validator
                                    && rec.is_live_at(height, overlap, expiry_grace)
                            })
                        })
                    });
                if !live && !found_index_record {
                    // Fallback when the pk index is stale or missing for this peer.
                    live = world.consensus_keys().iter().any(|(id, rec)| {
                        id.role == ConsensusKeyRole::Validator
                            && rec.public_key == *pk
                            && rec.is_live_at(height, overlap, expiry_grace)
                    });
                }
                if !live {
                    return Err(SignatureVerificationError::InactiveConsensusKey);
                }
            }
            Ok(())
        }
        pub(crate) fn enforce_consensus_key_lifecycle(
            block: &SignedBlock,
            topology: &Topology,
            state: &impl StateReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            Self::enforce_consensus_key_lifecycle_world(block, topology, state.world())
        }
        fn ensure_genesis_transactions_clean(
            block: &SignedBlock,
            genesis_account: &AccountId,
        ) -> Result<(), BlockValidationError> {
            if block.header().is_genesis() {
                if !block.has_results() {
                    iroha_logger::error!(
                        "Invalid genesis block rejected during validation: execution results missing"
                    );
                    return Err(BlockValidationError::InvalidGenesis(
                        InvalidGenesisError::MissingResults,
                    ));
                }
                if let Err(err) = check_genesis_block(block, genesis_account) {
                    iroha_logger::error!(
                        error = %err,
                        "Invalid genesis block rejected during validation"
                    );
                    return Err(BlockValidationError::InvalidGenesis(err));
                }
            }
            Ok(())
        }

        /// Component fixtures can start a recorder only when they contain no
        /// pre-staged consensus controls. Full prefix recording belongs to the
        /// applying constructor and must be supplied through its consuming seam.
        #[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]
        fn begin_component_fixture_recording(
            _state: &StateBlock<'_>,
        ) -> Result<crate::exec_witness::ExecWitnessGuard, BlockValidationError> {
            crate::exec_witness::begin_exec_witness_capture().map_err(Self::execution_context_error)
        }

        /// Execute the signed genesis through the sole native consensus path without publishing.
        /// The staged state includes the authenticated schedule, the actual global lane step,
        /// and the same complete execution witness used by startup and result certification.
        /// The original state stays boxed across validation and event delivery to bound stack use.
        ///
        /// The mode must come from the canonical signed genesis handshake metadata. It is threaded
        /// explicitly because the pre-execution world cannot yet contain genesis parameters.
        #[allow(clippy::too_many_arguments)]
        pub fn validate_signed_genesis<'state>(
            block: SignedBlock,
            topology: &Topology,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode,
        ) -> WithEvents<Result<(ValidBlock, Box<StateBlock<'state>>), Error>> {
            if !block.header().is_genesis() {
                return WithEvents::new(Err((
                    Box::new(block),
                    Box::new(BlockValidationError::InvalidGenesis(
                        InvalidGenesisError::InvalidHeader,
                    )),
                )));
            }
            Self::validate_with_profile(
                block,
                topology,
                genesis_account,
                time_source,
                state,
                false,
                None,
                false,
                ConsensusValidationProfile::SumeragiGenesis { consensus_mode },
                false,
                None,
            )
        }
        /// Execute the exact original nonempty proposal bound by its native consensus header.
        /// The source is verified before certified lane merges expand its execution inputs.
        /// Transaction validation, deterministic execution and original State generation checks
        /// remain mandatory; the native consensus certificate owns block authentication.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_sumeragi_block<'state>(
            block: SignedBlock,
            topology: &Topology,
            genesis_account: &AccountId,
            block_cadence: Duration,
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode,
            expansion: crate::sumeragi::lanes::merge::Expansion<'state>,
            native_header: &iroha_sumeragi::message::BlockHeader,
            native_payload: &[u8],
            state: &'state State,
        ) -> WithEvents<Result<(ValidBlock, Box<StateBlock<'state>>), Error>> {
            let source =
                match Self::native_header_source(&block, state, native_header, native_payload) {
                    Ok(source) => source,
                    Err(error) => return WithEvents::new(Err((Box::new(block), Box::new(error)))),
                };
            let (block, lanes) = match expansion.apply(block, source.state(), source.generation()) {
                Ok(expanded) => expanded,
                Err((block, error)) => {
                    return WithEvents::new(Err((
                        Box::new(block),
                        Box::new(match error {
                            crate::sumeragi::lanes::merge::MergeError::Pending(reason) => {
                                BlockValidationError::LocalStorageRecoveryRequired { reason }
                            }
                            crate::sumeragi::lanes::merge::MergeError::Invalid(reason) => {
                                Self::execution_context_error(reason)
                            }
                        }),
                    )));
                }
            };
            let (_, time_source) = TimeSource::new_mock(block.header().creation_time());
            Self::validate_with_profile(
                block,
                topology,
                genesis_account,
                &time_source,
                state,
                false,
                None,
                true,
                ConsensusValidationProfile::Sumeragi {
                    block_cadence,
                    consensus_mode,
                    supplied_pulse: source.pulse,
                    lanes,
                    expected_pulse_context: source.expected_context,
                    source_generation: source.generation,
                },
                true,
                None,
            )
        }

        fn state_block_for_execution<'state>(
            block: &SignedBlock,
            state: &'state State,
            source: PristineNativeSource<'state>,
            profile: &ConsensusValidationProfile,
        ) -> Result<
            (
                Box<StateBlock<'state>>,
                crate::exec_witness::ExecWitnessGuard,
            ),
            BlockValidationError,
        > {
            crate::exec_witness::ensure_exec_witness_capture_available()
                .map_err(Self::execution_context_error)?;
            Self::validate_sumeragi_consensus_effects(block)?;
            if block
                .execution_context()
                .is_some_and(|context| context.native_lane_decisions.is_some())
            {
                return Err(Self::execution_context_error(
                    "retired native Decision bodies are not supported",
                ));
            }
            state
                .block_with_recorded_pristine_carrier_stage(
                    block,
                    |overlay| {
                        overlay
                            .validate_native_pristine_control_owner(
                                source.state,
                                source.generation,
                                &source.header,
                            )
                            .map_err(Self::execution_context_error)?;
                        if let Some(genesis_height) = profile.sumeragi_schedule() {
                            overlay
                                .request_sumeragi_schedule(
                                    genesis_height,
                                    block,
                                    profile.sumeragi_pulse(),
                                    profile.sumeragi_pulse_context(),
                                )
                                .map_err(BlockValidationError::from)?;
                        }
                        Ok(())
                    },
                    Self::execution_context_error,
                )
                .map_err(BlockValidationError::from)
        }

        fn validate_staged_execution_controls(
            block: &SignedBlock,
            _state: &StateBlock<'_>,
        ) -> Result<(), BlockValidationError> {
            Self::checked_execution_context_header(block)?;
            if external_queue_plan_synced_entrypoint_index(block).is_some() {
                return Err(Self::execution_context_error(
                    "retired QueuePlanSynced input",
                ));
            }
            Ok(())
        }

        #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
        fn validate_with_profile<'state>(
            mut block: SignedBlock,
            topology: &Topology,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            soft_fork: bool,
            timings: Option<&mut ValidationTimings>,
            skip_block_signatures: bool,
            mut validation_profile: ConsensusValidationProfile,
            allow_empty_block: bool,
            mut send_events: Option<&mut dyn FnMut(PipelineEventBox)>,
        ) -> WithEvents<Result<(ValidBlock, Box<StateBlock<'state>>), Error>> {
            let total_start = Instant::now();
            let stateless_start = Instant::now();
            let to_ms = |duration: Duration| -> u64 {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            };
            let mut timings = timings;
            let mut emit_rejection = |block: &SignedBlock, error: &BlockValidationError| {
                if let Some(send_events) = send_events.as_deref_mut() {
                    emit_block_rejection(block.header(), error, send_events);
                }
            };
            let record_timings =
                |timings: &mut Option<&mut ValidationTimings>,
                 stateless_elapsed: Duration,
                 execution_start: Option<Instant>| {
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.stateless_ms = to_ms(stateless_elapsed);
                        timings.execution_ms =
                            execution_start.map_or(0, |start| to_ms(start.elapsed()));
                        timings.total_ms = to_ms(total_start.elapsed());
                    }
                };
            if let Some(index) = external_queue_plan_synced_entrypoint_index(&block) {
                let stateless_elapsed = stateless_start.elapsed();
                record_timings(&mut timings, stateless_elapsed, None);
                let error = Self::execution_context_error(format!(
                    "retired QueuePlanSynced external entrypoint at index {index}"
                ));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let static_state_start = Instant::now();
            let static_data = {
                let view = state.query_view();
                match Self::validate_static_state_dependent(
                    &block,
                    topology,
                    genesis_account,
                    &view,
                    soft_fork,
                    time_source,
                    skip_block_signatures,
                    &validation_profile,
                ) {
                    Ok(data) => {
                        if let Some(timings) = timings.as_deref_mut() {
                            timings.stateless_state_dependent_ms =
                                to_ms(static_state_start.elapsed());
                        }
                        data
                    }
                    Err(error) => {
                        let stateless_elapsed = stateless_start.elapsed();
                        if let Some(timings) = timings.as_deref_mut() {
                            timings.stateless_state_dependent_ms =
                                to_ms(static_state_start.elapsed());
                        }
                        record_timings(&mut timings, stateless_elapsed, None);
                        emit_rejection(&block, &error);
                        return WithEvents::new(Err((Box::new(block), Box::new(error))));
                    }
                }
            };
            let prepared_txs = Self::prepare_external_transactions(&block);
            let (committed_heights, committed_carrier_heights) = {
                let transactions_view = state.transactions.view();
                (
                    Self::committed_heights_for_prepared_transactions(
                        &prepared_txs,
                        &transactions_view,
                    ),
                    Self::committed_heights_for_entrypoint_carriers(&block, &transactions_view),
                )
            };
            let cache_cap = static_data.pipeline_cfg.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !block.header().is_genesis();
            let max_clock_drift_ms = static_data.max_clock_drift.as_millis();
            let cache_context = if cache_enabled {
                Some(StatelessValidationContext::new(
                    *state.network_id_ref(),
                    u64::try_from(max_clock_drift_ms).unwrap_or(u64::MAX),
                    static_data.tx_params,
                    static_data.crypto_cfg.allowed_signing.clone(),
                ))
            } else {
                None
            };
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            let static_snapshot_start = Instant::now();
            if let Err(error) = Self::validate_static_with_snapshot(
                &block,
                state.network_id_ref(),
                genesis_account,
                &static_data,
                &committed_heights,
                &committed_carrier_heights,
                &prepared_txs,
                metrics,
            ) {
                let stateless_elapsed = stateless_start.elapsed();
                if let Some(timings) = timings.as_deref_mut() {
                    timings.stateless_snapshot_ms = to_ms(static_snapshot_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, None);
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let consensus_effects =
                Self::validate_sumeragi_consensus_effects(&block).map(|()| PristineNativeSource {
                    state,
                    generation: validation_profile
                        .source_generation()
                        .unwrap_or_else(|| state.state_view_generation()),
                    header: block.header(),
                });
            let penalty_index = match consensus_effects {
                Ok(index) => index,
                Err(error) => {
                    let stateless_elapsed = stateless_start.elapsed();
                    record_timings(&mut timings, stateless_elapsed, None);
                    emit_rejection(&block, &error);
                    return WithEvents::new(Err((Box::new(block), Box::new(error))));
                }
            };
            if let Some(context) = cache_context {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (idx, (tx, prepared)) in Self::collect_external_signed_transactions(&block)
                    .into_iter()
                    .zip(prepared_txs.iter())
                    .enumerate()
                {
                    let expires_at_ms = tx
                        .time_to_live()
                        .and_then(|ttl| tx.creation_time().checked_add(ttl))
                        .map(|expires_at| expires_at.as_millis());
                    let not_before_ms = tx
                        .creation_time()
                        .as_millis()
                        .saturating_sub(max_clock_drift_ms);
                    cache.insert_ok(
                        prepared.metadata.stateless_cache_key,
                        expires_at_ms,
                        not_before_ms,
                    );
                }
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.stateless_snapshot_ms = to_ms(static_snapshot_start.elapsed());
            }
            let stateless_elapsed = stateless_start.elapsed();
            let execution_start = Instant::now();
            // Release block writer before creating new one
            let da_indexes_start = Instant::now();
            if let Err(error) = state.ensure_da_indexes_hydrated() {
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_da_indexes_ms = to_ms(da_indexes_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                let error = BlockValidationError::from(error);
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_da_indexes_ms = to_ms(da_indexes_start.elapsed());
            }
            let state_block_start = Instant::now();
            let (mut state_block, exec_witness_guard) = match Self::state_block_for_execution(
                &block,
                state,
                penalty_index,
                &validation_profile,
            ) {
                Ok((mut overlay, guard)) => {
                    if let Some(lanes) = validation_profile.take_sumeragi_lanes() {
                        overlay.request_sumeragi_lanes(lanes);
                    }
                    (overlay, guard)
                }
                Err(error) => {
                    record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                    emit_rejection(&block, &error);
                    return WithEvents::new(Err((Box::new(block), Box::new(error))));
                }
            };
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_state_block_ms = to_ms(state_block_start.elapsed());
            }
            if let Err(error) = Self::validate_staged_execution_controls(&block, &state_block) {
                drop(state_block);
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let tx_start = Instant::now();
            let genesis = if block.header().is_genesis() {
                match authenticate_genesis_block_intents(&block, genesis_account) {
                    Ok(genesis) => Some(genesis),
                    Err(error) => {
                        let error = BlockValidationError::from(error);
                        drop(state_block);
                        record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                        emit_rejection(&block, &error);
                        return WithEvents::new(Err((Box::new(block), Box::new(error))));
                    }
                }
            } else {
                None
            };
            if let Err(error) = Self::execute_and_record_canonical_outputs_in_context(
                &mut block,
                &mut state_block,
                timings.as_deref_mut(),
                genesis.as_ref(),
                validation_profile.sccp_height_source(),
            ) {
                drop(state_block);
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_tx_ms = to_ms(tx_start.elapsed());
            }
            let axt_start = Instant::now();
            if let Err(error) = validate_axt_envelopes(&block, &state_block) {
                drop(state_block);
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_axt_ms = to_ms(axt_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_axt_ms = to_ms(axt_start.elapsed());
            }
            let da_cursor_start = Instant::now();
            if let Err(error) = state_block.validate_da_shard_cursors(&block) {
                drop(state_block);
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_da_cursor_ms = to_ms(da_cursor_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_da_cursor_ms = to_ms(da_cursor_start.elapsed());
            }
            if let Err(error) = state_block
                .capture_exec_witness()
                .map_err(Self::execution_context_error)
            {
                drop(state_block);
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            drop(exec_witness_guard);
            if block.is_empty() && !allow_empty_block {
                let error = BlockValidationError::EmptyBlock;
                drop(state_block);
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let genesis_clean_start = Instant::now();
            if let Err(error) = Self::ensure_genesis_transactions_clean(&block, genesis_account) {
                drop(state_block);
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_genesis_clean_ms = to_ms(genesis_clean_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_genesis_clean_ms = to_ms(genesis_clean_start.elapsed());
            }
            record_timings(&mut timings, stateless_elapsed, Some(execution_start));
            WithEvents::new(Ok((
                ValidBlock::new_signatures_verified(block),
                state_block,
            )))
        }

        /// All static checks that require a state snapshot.
        fn canonical_v2_block_time(
            block: &SignedBlock,
            prev_block: &SignedBlock,
            block_cadence: Duration,
        ) -> Result<Duration, BlockValidationError> {
            Self::canonical_v2_block_time_from_parent_time(
                block,
                prev_block.header().creation_time(),
                block_cadence,
            )
        }
        /// The canonical creation time of a Sumeragi block over a parent created at
        /// `parent_creation_time`: at least one cadence later and strictly after every timed
        /// network input (`specs/sumeragi.md` Appendix E: parent + cadence rule).
        ///
        /// # Errors
        /// The time overflows.
        pub(crate) fn sumeragi_block_time(
            block: &SignedBlock,
            parent_creation_time: Duration,
            block_cadence: Duration,
        ) -> Result<Duration, BlockValidationError> {
            Self::canonical_v2_block_time_from_parent_time(
                block,
                parent_creation_time,
                block_cadence,
            )
        }
        fn canonical_v2_block_time_from_parent_time(
            block: &SignedBlock,
            parent_creation_time: Duration,
            block_cadence: Duration,
        ) -> Result<Duration, BlockValidationError> {
            let minimum = parent_creation_time
                .checked_add(block_cadence)
                .ok_or(BlockValidationError::V2BlockTimeOverflow)?;
            // Merged lane transactions do not set the time: the merge section's floor does
            // (`specs/sumeragi_lanes.md` §4.2), so the proposal alone fixes its time.
            let floor = Duration::from_millis(
                block
                    .lane_merge()
                    .map_or(0, |section| section.time_floor_ms),
            );
            let external = block.external_entrypoints_slice();
            let own = &external[..external.len() - block.merged_entrypoint_count()];
            let natives = block.network_entrypoints().skip(external.len());
            creation_time_after_inputs(minimum.max(floor), own.iter().chain(natives))
                .ok_or(BlockValidationError::V2BlockTimeOverflow)
        }
        #[allow(
            clippy::too_many_arguments,
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::collapsible_else_if
        )]
        fn validate_static_state_dependent(
            block: &SignedBlock,
            topology: &Topology,
            genesis_account: &AccountId,
            state: &impl StateReadOnly,
            soft_fork: bool,
            time_source: &TimeSource,
            skip_block_signatures: bool,
            validation_profile: &ConsensusValidationProfile,
        ) -> Result<StaticValidationData, BlockValidationError> {
            let state_height = state.block_hashes().len();
            let expected_block_height = if soft_fork {
                state_height
            } else {
                state_height
                    .checked_add(1)
                    .expect("INTERNAL BUG: Block height exceeds usize::MAX")
            };
            let actual_height = block
                .header()
                .height()
                .get()
                .try_into()
                .expect("INTERNAL BUG: Block height exceeds usize::MAX");
            if expected_block_height != actual_height {
                let state_latest_hash = state.block_hashes().iter().nth_back(0).copied();
                let state_prev_hash = state.block_hashes().iter().nth_back(1).copied();
                iroha_logger::warn!(
                    expected_height = expected_block_height,
                    actual_height,
                    state_height,
                    block_prev_hash = ?block.header().prev_block_hash(),
                    block_hash = ?block.hash(),
                    state_latest_hash = ?state_latest_hash,
                    state_prev_hash = ?state_prev_hash,
                    "prev block height mismatch during static validation"
                );
                return Err(BlockValidationError::PrevBlockHeightMismatch {
                    expected: expected_block_height,
                    actual: actual_height,
                });
            }
            let params = state.world().parameters();
            // Merged lane entrypoints are bounded by the lane merge rules
            // (`specs/sumeragi_lanes.md` §4.3); the cap applies to the block's own.
            validate_external_entrypoint_count(
                block
                    .external_entrypoint_count()
                    .saturating_sub(block.merged_entrypoint_count()),
                params.block().max_transactions(),
            )?;
            let max_clock_drift = params.sumeragi().max_clock_drift();
            let tx_params = params.transaction();
            if validation_profile.enforce_local_wall_clock() {
                let now = time_source.now();
                let block_creation_time = block.header().creation_time();
                if block_creation_time.saturating_sub(now) > max_clock_drift {
                    return Err(BlockValidationError::BlockInTheFuture);
                }
            }
            let expected_prev_block_hash = if soft_fork {
                state.block_hashes().iter().nth_back(1).copied()
            } else {
                state.block_hashes().iter().nth_back(0).copied()
            };
            let actual_prev_block_hash = block.header().prev_block_hash();
            if expected_prev_block_hash != actual_prev_block_hash {
                return Err(BlockValidationError::PrevBlockHashMismatch {
                    expected: expected_prev_block_hash,
                    actual: actual_prev_block_hash,
                });
            }
            if block.header().is_genesis() {
                // A consensus proposal is canonically resultless. Authenticate the configured
                // genesis root and every intent it commits before any genesis-only instruction
                // is allowed to execute against the bootstrap state.
                authenticate_genesis_block_intents(block, genesis_account)?;
            }
            Self::validate_sumeragi_consensus_effects(block)?;
            Self::validate_da_sidecar_hashes(block)?;
            Self::validate_da_pin_intent_bundle(block, state)?;
            Self::validate_execution_context_with_state(
                block,
                topology,
                state,
                validation_profile,
            )?;
            let block_height = block.header().height().get();
            let nexus = state.nexus();
            let expected_policy_bundle =
                crate::da::active_proof_policy_bundle_at_height(nexus, block_height);
            let expected_policy_hash = HashOf::new(&expected_policy_bundle);
            if block.header().da_proof_policies_hash() != Some(expected_policy_hash) {
                return Err(BlockValidationError::ProofPolicyHashMismatch {
                    expected: expected_policy_hash,
                    actual: block.header().da_proof_policies_hash(),
                });
            }
            if block.da_proof_policies() != Some(&expected_policy_bundle) {
                return Err(BlockValidationError::DaProofPolicyBundleMismatch);
            }
            let computed_digest =
                compute_confidential_feature_digest(state.world(), state.zk(), block_height);
            let expected_digest = if computed_digest.is_empty() {
                None
            } else {
                Some(computed_digest)
            };
            let actual_digest = block.header().confidential_features();
            ensure_confidential_features_match(expected_digest, actual_digest)?;
            if block.header().is_genesis() {
                if block.has_results() {
                    check_genesis_execution_results(block)?;
                }
            } else {
                let prev_block = if soft_fork {
                    state.prev_block()
                } else {
                    state.latest_block()
                };
                if let Some(prev_block) = prev_block {
                    let prev_block_time = prev_block.header().creation_time();
                    if let Some(block_cadence) = validation_profile.block_cadence() {
                        let expected =
                            Self::canonical_v2_block_time(block, &prev_block, block_cadence)?;
                        let actual = block.header().creation_time();
                        if actual != expected {
                            return Err(BlockValidationError::NonCanonicalV2BlockTime {
                                expected_ms: u64::try_from(expected.as_millis())
                                    .map_err(|_| BlockValidationError::V2BlockTimeOverflow)?,
                                actual_ms: u64::try_from(actual.as_millis())
                                    .map_err(|_| BlockValidationError::V2BlockTimeOverflow)?,
                            });
                        }
                    }
                    if block.header().creation_time() <= prev_block_time {
                        return Err(BlockValidationError::BlockInThePast);
                    }
                } else {
                    return Err(BlockValidationError::PrevBlockHashMismatch {
                        expected: expected_prev_block_hash,
                        actual: actual_prev_block_hash,
                    });
                }
                if !skip_block_signatures {
                    Self::verify_leader_signature(block, topology)?;
                    // Enforce BLS-normal for validator signatures (Set A + Set B).
                    Self::verify_validator_signatures(block, topology)?;
                    Self::verify_no_undefined_signatures(block, topology)?;
                    Self::verify_unique_signers(block)?;
                    Self::enforce_consensus_key_lifecycle(block, topology, state)?;
                }
            }
            let crypto_cfg = state.crypto();
            let pipeline_cfg = state.pipeline().clone();
            let pipeline_parallelism = crate::state::PipelineParallelism::new(&pipeline_cfg);
            let aggregate_lane = nexus.routing_policy.default_lane;
            Ok(StaticValidationData {
                expected_block_height,
                max_clock_drift,
                tx_params,
                crypto_cfg,
                pipeline_cfg,
                pipeline_parallelism,
                aggregate_lane,
            })
        }
        fn npos_effects_error(message: impl Into<String>) -> BlockValidationError {
            BlockValidationError::NposEffectsInvalid(message.into())
        }

        fn validate_da_sidecar_hashes(block: &SignedBlock) -> Result<(), BlockValidationError> {
            let expected_policies = block.da_proof_policies().map(HashOf::new);
            let actual_policies = block.header().da_proof_policies_hash();
            if expected_policies.is_none() || actual_policies != expected_policies {
                return Err(BlockValidationError::DaProofPolicySidecarHashMismatch {
                    expected: expected_policies,
                    actual: actual_policies,
                });
            }
            if block
                .da_commitments()
                .is_some_and(DaCommitmentBundle::is_empty)
            {
                return Err(BlockValidationError::NonCanonicalEmptyDaCommitmentBundle);
            }
            let expected_commitments = block
                .da_commitments()
                .and_then(DaCommitmentBundle::merkle_commitment);
            let actual_commitments = block.header().da_commitments_hash();
            if actual_commitments != expected_commitments {
                return Err(BlockValidationError::DaCommitmentHashMismatch {
                    expected: expected_commitments,
                    actual: actual_commitments,
                });
            }
            if block
                .da_pin_intents()
                .is_some_and(DaPinIntentBundle::is_empty)
            {
                return Err(BlockValidationError::NonCanonicalEmptyDaPinIntentBundle);
            }
            let expected_pin_intents = block
                .da_pin_intents()
                .and_then(DaPinIntentBundle::merkle_commitment);
            let actual_pin_intents = block.header().da_pin_intents_hash();
            if actual_pin_intents != expected_pin_intents {
                return Err(BlockValidationError::DaPinIntentHashMismatch {
                    expected: expected_pin_intents,
                    actual: actual_pin_intents,
                });
            }
            Ok(())
        }
        fn validate_da_pin_intent_bundle(
            block: &SignedBlock,
            state: &impl StateReadOnly,
        ) -> Result<(), BlockValidationError> {
            let Some(bundle) = block.da_pin_intents() else {
                return Ok(());
            };
            let world = state.world();
            crate::da::validate_pin_intent_bundle_against_nexus_at_height(
                bundle,
                state.nexus(),
                block.header().height().get(),
                |account| world.accounts().get(account).is_some(),
            )?;
            crate::da::validate_pin_intent_authorizations(
                bundle,
                *state.network_id(),
                |account| {
                    world
                        .accounts()
                        .get(account)
                        .map(|_| account.controller().clone())
                },
            )?;
            let admission_policy = world
                .parameters()
                .custom()
                .get(&iroha_data_model::da::ingest::DaIngestAdmissionPolicyV1::parameter_id())
                .map(|custom| {
                    iroha_data_model::da::ingest::DaIngestAdmissionPolicyV1::from_custom_parameter(
                        custom,
                    )
                    .map_err(|error| {
                        BlockValidationError::DaPinIntentBundle(
                            DaPinIntentValidationError::InvalidAdmissionPolicy {
                                reason: error.to_string(),
                            },
                        )
                    })?
                    .ok_or_else(|| {
                        BlockValidationError::DaPinIntentBundle(
                            DaPinIntentValidationError::InvalidAdmissionPolicy {
                                reason: "reserved parameter changed identity".to_owned(),
                            },
                        )
                    })
                })
                .transpose()?;
            crate::da::validate_pin_intent_admission_policy(
                bundle,
                admission_policy.as_ref(),
                |lane_id| state.lane_incarnation_at_height(lane_id, block.header().height().get()),
            )?;
            crate::da::quota::prepare_ingest_quota_writes(
                world.smart_contract_state(),
                bundle,
                block.header().height().get(),
                &state.nexus().da,
            )?;
            for intent in &bundle.intents {
                if world
                    .da_pin_intents_by_lane_epoch()
                    .get(&(intent.lane_id, intent.epoch, intent.sequence))
                    .is_some()
                {
                    return Err(BlockValidationError::DaPinIntentBundle(
                        DaPinIntentValidationError::DuplicateIntent {
                            lane: intent.lane_id,
                            epoch: intent.epoch,
                            sequence: intent.sequence,
                        },
                    ));
                }
                if world
                    .da_pin_intents_by_manifest()
                    .get(&intent.manifest_hash)
                    .is_some()
                {
                    return Err(BlockValidationError::DaPinIntentBundle(
                        DaPinIntentValidationError::DuplicateManifest {
                            lane: intent.lane_id,
                            epoch: intent.epoch,
                            sequence: intent.sequence,
                        },
                    ));
                }
                if world
                    .da_pin_intents_by_ticket()
                    .get(&intent.storage_ticket)
                    .is_some()
                {
                    return Err(BlockValidationError::DaPinIntentBundle(
                        DaPinIntentValidationError::DuplicateStorageTicket {
                            lane: intent.lane_id,
                            epoch: intent.epoch,
                            sequence: intent.sequence,
                        },
                    ));
                }
            }
            Ok(())
        }

        /// Native control belongs to the authenticated header; the payload has no second
        /// pulse owner. ScheduleStep validates and applies that witness exactly once.
        fn validate_sumeragi_consensus_effects(
            block: &SignedBlock,
        ) -> Result<(), BlockValidationError> {
            if block.header().npos_effects_hash().is_some()
                || block.npos_consensus_effects().is_some()
            {
                return Err(Self::npos_effects_error(
                    "current Sumeragi blocks reject retired NPoS consensus effects",
                ));
            }
            if block.global_beacon_pulse().is_some()
                || block.header().global_beacon_pulse_hash().is_some()
            {
                return Err(Self::npos_effects_error(
                    "native payload rejects a second beacon pulse owner",
                ));
            }
            Ok(())
        }

        fn execution_context_error(message: impl Into<String>) -> BlockValidationError {
            BlockValidationError::ExecutionContextInvalid(message.into())
        }
        fn validate_execution_context_header(
            block: &SignedBlock,
        ) -> Result<Option<&BlockExecutionContextBundle>, BlockValidationError> {
            Self::checked_execution_context_header(block)
        }
        fn checked_execution_context_header(
            block: &SignedBlock,
        ) -> Result<Option<&BlockExecutionContextBundle>, BlockValidationError> {
            match (
                block.header().execution_context_hash(),
                block.execution_context(),
            ) {
                (None, None) => Ok(None),
                (Some(_), None) => Err(Self::execution_context_error(
                    "header references execution context but payload is missing",
                )),
                (None, Some(_)) => Err(Self::execution_context_error(
                    "payload includes execution context but header hash is absent",
                )),
                (Some(expected), Some(bundle)) => {
                    if !bundle.has_current_version() {
                        return Err(Self::execution_context_error(format!(
                            "unsupported block execution-context bundle version {}",
                            bundle.version
                        )));
                    }
                    if bundle.native_lane_decisions.is_some()
                        || bundle.merge_entry.is_some()
                        || !bundle.queue_plan_admissions.is_empty()
                        || !bundle.autonomous_lane_payloads.is_empty()
                        || !bundle.lane_payload_ownerships.is_empty()
                        || bundle
                            .external
                            .iter()
                            .any(|context| context.native_amx_receipt.is_some())
                    {
                        return Err(Self::execution_context_error(
                            "retired consensus attachment in native execution context",
                        ));
                    }
                    let actual = HashOf::new(bundle);
                    if actual != expected {
                        return Err(Self::execution_context_error(
                            "execution context hash mismatch",
                        ));
                    }
                    Ok(Some(bundle))
                }
            }
        }

        fn validate_execution_context_alignment(
            block: &SignedBlock,
            bundle: &BlockExecutionContextBundle,
        ) -> Result<(), BlockValidationError> {
            let expected_len = block.external_entrypoint_count();
            if bundle.external.len() != expected_len {
                return Err(Self::execution_context_error(format!(
                    "execution context length mismatch: expected {expected_len}, got {}",
                    bundle.external.len()
                )));
            }
            for (idx, (entrypoint, context)) in block
                .external_entrypoints_cloned()
                .zip(bundle.external.iter())
                .enumerate()
            {
                let expected = entrypoint.hash();
                if context.entrypoint_hash != expected {
                    return Err(Self::execution_context_error(format!(
                        "execution context entrypoint hash mismatch at index {idx}"
                    )));
                }
            }
            Ok(())
        }

        fn validate_execution_context_with_state(
            block: &SignedBlock,
            _topology: &Topology,
            state: &impl StateReadOnly,
            _profile: &ConsensusValidationProfile,
        ) -> Result<(), BlockValidationError> {
            let bundle = Self::checked_execution_context_header(block)?;
            let Some(bundle) = bundle else {
                return if !block.header().is_genesis() && block.external_entrypoint_count() != 0 {
                    Err(Self::execution_context_error(
                        "missing execution context for external inputs",
                    ))
                } else {
                    Ok(())
                };
            };
            if bundle.merge_entry.is_some()
                || !bundle.autonomous_lane_payloads.is_empty()
                || !bundle.lane_payload_ownerships.is_empty()
                || bundle
                    .external
                    .iter()
                    .any(|context| context.native_amx_receipt.is_some())
            {
                return Err(Self::execution_context_error(
                    "retired receipt or merge authority in native proposal",
                ));
            }
            if bundle.native_lane_decisions.is_some() {
                return Err(Self::execution_context_error(
                    "retired native Decision body",
                ));
            }
            Self::validate_execution_context_alignment(block, bundle)?;
            if block.header().is_genesis() {
                return Ok(());
            }
            let routing = crate::sumeragi::lanes::routing::RoutingSnapshot::of(state);
            let inputs = routing.inputs(state.world());
            for (index, (entrypoint, context)) in block
                .external_entrypoints_slice()
                .iter()
                .zip(&bundle.external)
                .enumerate()
            {
                let plan = routing_plan_from_execution_context(context)
                    .map_err(|error| Self::execution_context_error(error.to_string()))?;
                let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
                    Cow::Borrowed(entrypoint),
                );
                let expected = inputs
                    .execution_route(&accepted, block.header().height().get())
                    .ok_or_else(|| {
                        Self::execution_context_error(
                            "committed native execution route is unavailable",
                        )
                    })?;
                if matches!(plan, crate::queue::RoutingPlan::NativeAmx(_))
                    || plan.coordinator_route() != expected
                {
                    return Err(Self::execution_context_error(format!(
                        "input {index} differs from its native committed execution route"
                    )));
                }
            }
            Ok(())
        }

        fn committed_heights_for_prepared_transactions(
            prepared_txs: &[PreparedBlockTransaction],
            transactions: &impl TransactionsReadOnly,
        ) -> Vec<Option<NonZeroUsize>> {
            prepared_txs
                .iter()
                .map(|prepared| transactions.get(&prepared.metadata.entrypoint_hash))
                .collect()
        }
        fn committed_heights_for_entrypoint_carriers(
            block: &SignedBlock,
            transactions: &impl TransactionsReadOnly,
        ) -> Vec<Option<NonZeroUsize>> {
            block
                .external_entrypoints_slice()
                .iter()
                .map(|entrypoint| transactions.get(&entrypoint.hash()))
                .collect()
        }
        /// Reject a block that carries more exempt-shaped SCCP transactions than the per-block
        /// caps allow (`specs/sccp.md` §4.19). The shapes are a pure function of the entry
        /// points and the caps are the committed parent's parameters, exactly what the
        /// proposer's queue selection counts (`Queue::bounded_pending_snapshot`), so a block
        /// its proposer built always passes. Without SCCP nothing is capped.
        fn validate_sccp_exempt_cap(
            block: &SignedBlock,
            state_block: &StateBlock<'_>,
        ) -> Result<(), BlockValidationError> {
            // SCCP is initialized only in genesis, so a block whose own state has no SCCP
            // parameters has no SCCP parent either; skip the parent view entirely.
            if !crate::smartcontracts::isi::sccp::params::exists(&state_block.world) {
                return Ok(());
            }
            Self::validate_sccp_exempt_cap_against(block, &state_block.sccp_parent_world_view())
        }
        /// Check the per-block SCCP exemption caps of `block` against the `parent` World.
        fn validate_sccp_exempt_cap_against(
            block: &SignedBlock,
            parent: &(impl crate::state::WorldReadOnly + ?Sized),
        ) -> Result<(), BlockValidationError> {
            use crate::smartcontracts::isi::sccp::{admission, params};
            if !params::exists(parent) {
                return Ok(());
            }
            let classes = block
                .network_entrypoints()
                .filter_map(admission::exempt_shape_of_entrypoint)
                .collect::<Vec<_>>();
            if admission::block_exempt_cap_ok(parent, &classes) {
                Ok(())
            } else {
                Err(Self::execution_context_error(format!(
                    "block carries {} exempt-shaped SCCP transactions beyond the per-block caps",
                    classes.len()
                )))
            }
        }
        fn signed_transaction_from_entrypoint(
            entrypoint: &TransactionEntrypoint,
        ) -> Option<&SignedTransaction> {
            match entrypoint {
                TransactionEntrypoint::External(tx) => Some(tx),
                TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
                TransactionEntrypoint::SealedCommitment(_) => None,
            }
        }
        fn collect_external_signed_transactions(block: &SignedBlock) -> Vec<&SignedTransaction> {
            block
                .external_entrypoints_slice()
                .iter()
                .filter_map(Self::signed_transaction_from_entrypoint)
                .collect()
        }
        /// Resolve the stateless-validation instant for each signed external entrypoint.
        ///
        /// Only an exact QueuePlan binding that was already pending in canonical parent state can
        /// replace the block timestamp. The lookup authenticates the complete transaction wire,
        /// committed routing plan, immutable registry owner, pending obligation, route markers,
        /// lane incarnations, and minimum execution height before its enqueue timestamp is used.

        fn prepare_external_transactions(block: &SignedBlock) -> Vec<PreparedBlockTransaction> {
            Self::collect_external_signed_transactions(block)
                .into_iter()
                .map(|tx| PreparedBlockTransaction {
                    metadata: crate::tx::AcceptedTransaction::prepare_signed_metadata(tx),
                })
                .collect()
        }
        #[cfg(feature = "bls")]
        #[allow(clippy::too_many_lines)]
        fn precheck_bls_transaction_signatures(
            signed_txs: &[&SignedTransaction],
            prepared_txs: &[PreparedBlockTransaction],
            cap: usize,
            prechecked_signature_results: &mut [Option<Result<(), SignatureVerificationFail>>],
            metrics: MetricsRef<'_>,
            lane_id: LaneId,
        ) {
            #[derive(Clone)]
            struct BlsItem {
                idx: usize,
                pk: iroha_crypto::PublicKey,
                pk_bytes: Vec<u8>,
                pop: Option<Vec<u8>>,
                msg: [u8; 32],
                sig: Vec<u8>,
            }
            static BLS_POP_KEY: LazyLock<iroha_model_base::name::Name> =
                LazyLock::new(|| "bls_pop".parse().expect("valid metadata key"));
            static BLS_POP_SMALL_KEY: LazyLock<iroha_model_base::name::Name> =
                LazyLock::new(|| "bls_pop_small".parse().expect("valid metadata key"));
            let mut all_normal_have_pop = true;
            let mut all_small_have_pop = true;
            let mut items_normal: Vec<BlsItem> = Vec::new();
            let mut items_small: Vec<BlsItem> = Vec::new();
            for (idx, (tx, prepared)) in signed_txs.iter().zip(prepared_txs.iter()).enumerate() {
                let AccountController::Single(signatory) = tx.authority().controller() else {
                    continue;
                };
                let Ok((algorithm, pk_bytes)) = signatory.try_to_bytes() else {
                    continue;
                };
                let small = match algorithm {
                    iroha_crypto::Algorithm::BlsNormal => false,
                    iroha_crypto::Algorithm::BlsSmall => true,
                    _ => continue,
                };
                let h = prepared.metadata.payload_hash;
                let mut msg = [0_u8; 32];
                msg.copy_from_slice(h.as_ref());
                let sig = tx.signature().payload().payload().to_vec();
                let mut pop = None;
                if small {
                    if let Some(pop_bytes) =
                        bls_small_pop_from_metadata(tx.metadata(), &BLS_POP_SMALL_KEY)
                    {
                        if iroha_crypto::bls_small_pop_verify(signatory, &pop_bytes).is_ok() {
                            pop = Some(pop_bytes);
                        } else {
                            all_small_have_pop = false;
                        }
                    } else {
                        all_small_have_pop = false;
                    }
                } else if let Some(pop_bytes) = bls_pop_from_metadata(tx.metadata(), &BLS_POP_KEY) {
                    if iroha_crypto::bls_normal_pop_verify(signatory, &pop_bytes).is_ok() {
                        pop = Some(pop_bytes);
                    } else {
                        all_normal_have_pop = false;
                    }
                } else {
                    all_normal_have_pop = false;
                }
                let item = BlsItem {
                    idx,
                    pk: signatory.clone(),
                    pk_bytes: pk_bytes.to_vec(),
                    pop,
                    msg,
                    sig,
                };
                if small {
                    items_small.push(item);
                } else {
                    items_normal.push(item);
                }
            }
            #[cfg(feature = "telemetry")]
            let mut same_msg_agg = 0_u64;
            #[cfg(feature = "telemetry")]
            let mut multi_msg_agg = 0_u64;
            #[cfg(feature = "telemetry")]
            let mut deterministic = 0_u64;
            #[cfg(feature = "telemetry")]
            let record_result = |same_message: bool, success: bool| {
                if let Some(metrics) = metrics {
                    metrics.inc_pipeline_sig_bls_result(lane_id, same_message, success);
                }
            };
            let mut verify_set = |items: &[BlsItem], small: bool| {
                if items.is_empty() {
                    return;
                }
                let mut groups: BTreeMap<[u8; 32], Vec<&BlsItem>> = BTreeMap::new();
                for item in items {
                    groups.entry(item.msg).or_default().push(item);
                }
                let mut singletons: Vec<&BlsItem> = Vec::new();
                for group in groups.values() {
                    if group.len() == 1 {
                        singletons.push(group[0]);
                        continue;
                    }
                    let Some(pops) = group
                        .iter()
                        .map(|item| item.pop.as_ref().map(Vec::as_slice))
                        .collect::<Option<Vec<_>>>()
                    else {
                        return;
                    };
                    let msg = group[0].msg.as_slice();
                    let sigs: Vec<&[u8]> = group.iter().map(|item| item.sig.as_slice()).collect();
                    let pks: Vec<&iroha_crypto::PublicKey> =
                        group.iter().map(|item| &item.pk).collect();
                    let ok = if small {
                        iroha_crypto::bls_small_verify_aggregate_same_message(
                            msg, &sigs, &pks, &pops,
                        )
                        .is_ok()
                    } else {
                        iroha_crypto::bls_normal_verify_aggregate_same_message(
                            msg, &sigs, &pks, &pops,
                        )
                        .is_ok()
                    };
                    #[cfg(feature = "telemetry")]
                    {
                        same_msg_agg = same_msg_agg.saturating_add(1);
                        record_result(true, ok);
                    }
                    if ok {
                        for item in group {
                            prechecked_signature_results[item.idx] = Some(Ok(()));
                        }
                    } else {
                        for item in group {
                            prechecked_signature_results[item.idx] = Some(
                                crate::tx::AcceptedTransaction::signature_verification_result(
                                    signed_txs[item.idx],
                                ),
                            );
                        }
                    }
                }
                if !singletons.is_empty() {
                    let msgs: Vec<&[u8]> =
                        singletons.iter().map(|item| item.msg.as_slice()).collect();
                    let sigs: Vec<&[u8]> =
                        singletons.iter().map(|item| item.sig.as_slice()).collect();
                    let pks: Vec<&[u8]> = singletons
                        .iter()
                        .map(|item| item.pk_bytes.as_slice())
                        .collect();
                    let ok = if small {
                        iroha_crypto::bls_small_verify_aggregate_multi_message(&msgs, &sigs, &pks)
                            .is_ok()
                    } else {
                        iroha_crypto::bls_normal_verify_aggregate_multi_message(&msgs, &sigs, &pks)
                            .is_ok()
                    };
                    #[cfg(feature = "telemetry")]
                    {
                        multi_msg_agg = multi_msg_agg.saturating_add(1);
                        record_result(false, ok);
                    }
                    if ok {
                        for item in singletons {
                            prechecked_signature_results[item.idx] = Some(Ok(()));
                        }
                    } else {
                        for item in singletons {
                            prechecked_signature_results[item.idx] = Some(
                                crate::tx::AcceptedTransaction::signature_verification_result(
                                    signed_txs[item.idx],
                                ),
                            );
                        }
                    }
                }
            };
            if cap > 0 {
                if all_normal_have_pop {
                    for chunk in items_normal.chunks(cap) {
                        verify_set(chunk, false);
                    }
                } else {
                    #[cfg(feature = "telemetry")]
                    {
                        deterministic = deterministic
                            .saturating_add(u64::try_from(items_normal.len()).unwrap_or(u64::MAX));
                    }
                }
                if all_small_have_pop {
                    for chunk in items_small.chunks(cap) {
                        verify_set(chunk, true);
                    }
                } else {
                    #[cfg(feature = "telemetry")]
                    {
                        deterministic = deterministic
                            .saturating_add(u64::try_from(items_small.len()).unwrap_or(u64::MAX));
                    }
                }
            } else {
                #[cfg(feature = "telemetry")]
                {
                    let item_count = items_normal.len().saturating_add(items_small.len());
                    deterministic =
                        deterministic.saturating_add(u64::try_from(item_count).unwrap_or(u64::MAX));
                }
            }
            #[cfg(feature = "telemetry")]
            if let Some(metrics) = metrics {
                metrics.set_pipeline_sig_bls_counts(same_msg_agg, multi_msg_agg, deterministic);
            }
            #[cfg(not(feature = "telemetry"))]
            let _ = (metrics, lane_id);
        }
        /// Static checks that do not require holding a state view.
        #[allow(
            clippy::too_many_arguments,
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::collapsible_else_if,
            clippy::items_after_statements,
            clippy::option_if_let_else,
            clippy::manual_flatten
        )]
        fn validate_static_with_snapshot(
            block: &SignedBlock,
            network_id: &NetworkId,
            genesis_account: &AccountId,
            static_data: &StaticValidationData,
            committed_heights: &[Option<NonZeroUsize>],
            committed_carrier_heights: &[Option<NonZeroUsize>],
            prepared_txs: &[PreparedBlockTransaction],
            _metrics: MetricsRef<'_>,
        ) -> Result<(), BlockValidationError> {
            let _ = static_data.aggregate_lane;
            // Rayon workers must use the caller's configured account address profile.
            let account_discriminant = chain_discriminant();
            #[cfg(test)]
            let account_profile_observer = tests::account_profile_validation_observer();
            let max_clock_drift = static_data.max_clock_drift;
            let tx_params = static_data.tx_params;
            let expected_block_height = static_data.expected_block_height;
            let pipeline_cfg = &static_data.pipeline_cfg;
            let crypto_cfg = &static_data.crypto_cfg;
            let block_creation_time = block.header().creation_time();
            debug_assert_eq!(
                committed_heights.len(),
                prepared_txs.len(),
                "committed-height snapshot must align with block transaction list",
            );
            if committed_heights.len() != prepared_txs.len() {
                return Err(BlockValidationError::MerkleRootMismatch);
            }
            if committed_carrier_heights.len() != block.external_entrypoint_count() {
                return Err(BlockValidationError::MerkleRootMismatch);
            }
            if committed_carrier_heights.iter().any(|committed_height| {
                committed_height
                    .as_ref()
                    .is_some_and(|height| height.get() < expected_block_height)
            }) {
                return Err(BlockValidationError::HasCommittedTransactions);
            }
            let signed_txs = Self::collect_external_signed_transactions(block);
            debug_assert_eq!(
                signed_txs.len(),
                prepared_txs.len(),
                "prepared metadata must align with signed block transactions",
            );
            if signed_txs.len() != prepared_txs.len() {
                return Err(BlockValidationError::MerkleRootMismatch);
            }
            let is_genesis_block = block.header().is_genesis();
            let mut prechecked_signature_results: Vec<
                Option<Result<(), SignatureVerificationFail>>,
            > = vec![None; prepared_txs.len()];
            // Genesis authenticates the ordered transaction intents once through the
            // genesis-authority block signature. Per-transaction proof bytes are not
            // an additional admission boundary.
            #[cfg(feature = "bls")]
            if !is_genesis_block {
                Self::precheck_bls_transaction_signatures(
                    &signed_txs,
                    prepared_txs,
                    pipeline_cfg.signature_batch_max_bls,
                    &mut prechecked_signature_results,
                    _metrics,
                    static_data.aggregate_lane,
                );
            }
            let mut seen_hashes: HashSet<HashOf<SignedTransaction>> =
                HashSet::with_capacity(signed_txs.len());
            let mut seen_sealed_commitments =
                HashSet::with_capacity(block.external_entrypoint_count());
            for ((tx, prepared), committed_height) in signed_txs
                .iter()
                .copied()
                .zip(prepared_txs.iter())
                .zip(committed_heights.iter())
            {
                let tx_hash = prepared.metadata.signed_hash;
                // In case of soft-fork transaction is check if it was added at the same height as candidate block.
                if committed_height
                    .as_ref()
                    .is_some_and(|height| height.get() < expected_block_height)
                {
                    return Err(BlockValidationError::HasCommittedTransactions);
                }
                if !seen_hashes.insert(tx_hash) {
                    iroha_logger::error!(
                        %tx_hash,
                        height = %block.header().height(),
                        "duplicate transaction detected during block validation"
                    );
                    return Err(BlockValidationError::DuplicateTransactions);
                }
                if tx.creation_time() >= block_creation_time {
                    return Err(BlockValidationError::TransactionInTheFuture);
                }
            }
            let mut entrypoint_hashes = Vec::with_capacity(block.external_entrypoint_count());
            let mut prepared_signed_idx = 0usize;
            for entrypoint in block.external_entrypoints_slice() {
                match entrypoint {
                    TransactionEntrypoint::External(_) => {
                        let prepared = prepared_txs
                            .get(prepared_signed_idx)
                            .ok_or(BlockValidationError::MerkleRootMismatch)?;
                        entrypoint_hashes.push(prepared.metadata.entrypoint_hash);
                        prepared_signed_idx = prepared_signed_idx.saturating_add(1);
                    }
                    TransactionEntrypoint::SealedReveal(_) => {
                        let _prepared = prepared_txs
                            .get(prepared_signed_idx)
                            .ok_or(BlockValidationError::MerkleRootMismatch)?;
                        entrypoint_hashes.push(entrypoint.hash());
                        prepared_signed_idx = prepared_signed_idx.saturating_add(1);
                    }
                    TransactionEntrypoint::SealedCommitment(commitment) => {
                        crate::tx::validate_sealed_commitment_stateless(
                            commitment, network_id, tx_params,
                        )
                        .map_err(BlockValidationError::TransactionAccept)?;
                        if commitment.payload().reveal_after_height
                            <= u64::try_from(expected_block_height).unwrap_or(u64::MAX)
                        {
                            return Err(BlockValidationError::TransactionAccept(
                                AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                                    reason: "sealed transaction reveal_after_height must be greater than commit height".into(),
                                }),
                            ));
                        }
                        if !seen_sealed_commitments.insert(*commitment.commitment()) {
                            return Err(BlockValidationError::DuplicateTransactions);
                        }
                        entrypoint_hashes.push(entrypoint.hash());
                    }
                }
            }
            debug_assert_eq!(
                prepared_signed_idx,
                prepared_txs.len(),
                "signed entrypoint preparation must align with external entries",
            );
            use rayon::prelude::*;
            let mut ed25519_prechecked = vec![false; prepared_txs.len()];
            let ed25519_batch_cap = pipeline_cfg.signature_batch_max_ed25519;
            if !is_genesis_block && ed25519_batch_cap > 0 {
                struct Ed25519BatchItem {
                    idx: usize,
                }
                fn verify_ed25519_batch_slices<'a>(
                    messages: &[&'a [u8]],
                    signatures: &[&'a [u8]],
                    public_keys: &[iroha_crypto::Ed25519ParsedPublicKey],
                    scratch: &mut iroha_crypto::Ed25519BatchScratch<'a>,
                ) -> Result<(), iroha_crypto::Error> {
                    iroha_crypto::ed25519_verify_batch_preparsed_deterministic_with_scratch(
                        messages,
                        signatures,
                        public_keys,
                        scratch,
                    )
                }
                let mut items = Vec::with_capacity(prepared_txs.len());
                let mut messages = Vec::with_capacity(prepared_txs.len());
                let mut signatures = Vec::with_capacity(prepared_txs.len());
                let mut public_keys = Vec::with_capacity(prepared_txs.len());
                let mut scratch = iroha_crypto::Ed25519BatchScratch::default();
                for (idx, (tx, prepared)) in signed_txs
                    .iter()
                    .copied()
                    .zip(prepared_txs.iter())
                    .enumerate()
                {
                    let signature = tx.signature().payload().payload();
                    if ed25519_prechecked[idx] {
                        continue;
                    }
                    if signature.len() != crate::tx::ED25519_SIGNATURE_LENGTH {
                        continue;
                    }
                    let Some(public_key) = prepared.metadata.single_ed25519_key else {
                        continue;
                    };
                    items.push(Ed25519BatchItem { idx });
                    messages.push(prepared.metadata.payload_hash.as_ref().as_slice());
                    signatures.push(signature);
                    public_keys.push(public_key);
                }
                let signature_error = |tx: &SignedTransaction, detail: String| {
                    BlockValidationError::TransactionAccept(
                        AcceptTransactionFail::SignatureVerification(
                            SignatureVerificationFail::new(
                                tx.signature().clone(),
                                SignatureRejectionCode::InvalidSignature,
                                detail,
                            ),
                        ),
                    )
                };
                for range_start in (0..items.len()).step_by(ed25519_batch_cap) {
                    let range_end = range_start
                        .saturating_add(ed25519_batch_cap)
                        .min(items.len());
                    let messages = &messages[range_start..range_end];
                    let signatures = &signatures[range_start..range_end];
                    let public_keys = &public_keys[range_start..range_end];
                    if let Err(err) =
                        verify_ed25519_batch_slices(messages, signatures, public_keys, &mut scratch)
                    {
                        if let Some((relative_idx, detail)) =
                            iroha_crypto::ed25519_first_bad_preparsed_deterministic_with_scratch(
                                messages,
                                signatures,
                                public_keys,
                                &mut scratch,
                            )
                        {
                            let idx = items[range_start + relative_idx].idx;
                            return Err(signature_error(signed_txs[idx], detail));
                        }
                        let idx = items.get(range_start).map_or(0, |item| item.idx);
                        return Err(signature_error(signed_txs[idx], err.to_string()));
                    }
                    for item in &items[range_start..range_end] {
                        ed25519_prechecked[item.idx] = true;
                    }
                }
            }
            let validate_tx = |(idx, (tx, prepared)): (
                usize,
                (&SignedTransaction, &PreparedBlockTransaction),
            )|
             -> Option<BlockValidationError> {
                #[cfg(test)]
                let inherited_account_profile = chain_discriminant();
                let _profile = ChainDiscriminantGuard::enter(account_discriminant);
                #[cfg(test)]
                if let Some(observer) = account_profile_observer.as_ref() {
                    observer.observe(idx, inherited_account_profile, chain_discriminant());
                }
                let prechecked_signature_result = prechecked_signature_results
                    .get(idx)
                    .and_then(|result| result.as_ref().cloned());
                let validation_time = block_creation_time;
                if is_genesis_block {
                    if let Some(Err(fail)) = prechecked_signature_result {
                        return Some(BlockValidationError::TransactionAccept(
                            AcceptTransactionFail::SignatureVerification(fail),
                        ));
                    }
                    return AcceptedTransaction::validate_genesis_with_now(
                        tx,
                        max_clock_drift,
                        genesis_account,
                        crypto_cfg.as_ref(),
                        block_creation_time,
                    )
                    .err()
                    .map(BlockValidationError::TransactionAccept);
                }
                let stateless = if let Some(prechecked_signature_result) =
                    prechecked_signature_result
                {
                    AcceptedTransaction::validate_with_now_with_signature_result_and_prepared_metadata(
                            tx,
                            network_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            validation_time,
                            Some(prechecked_signature_result),
                            &prepared.metadata,
                        )
                } else if ed25519_prechecked[idx] {
                    AcceptedTransaction::validate_with_now_after_single_ed25519_precheck_and_prepared_metadata(
                            tx,
                            network_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            validation_time,
                            &prepared.metadata,
                        )
                } else {
                    AcceptedTransaction::validate_with_now_and_prepared_metadata(
                        tx,
                        network_id,
                        max_clock_drift,
                        tx_params,
                        crypto_cfg.as_ref(),
                        validation_time,
                        &prepared.metadata,
                    )
                };
                stateless.err().map(BlockValidationError::TransactionAccept)
            };
            let static_pool = static_data.pipeline_parallelism.pool();
            let use_parallel = static_pool.is_some() && prepared_txs.len() > 1;
            let tx_errors: Vec<Option<BlockValidationError>> = if use_parallel {
                static_pool
                    .as_ref()
                    .expect("parallel validation requires a configured pipeline pool")
                    .install(|| {
                        signed_txs
                            .par_iter()
                            .copied()
                            .zip(prepared_txs.par_iter())
                            .enumerate()
                            .map(validate_tx)
                            .collect()
                    })
            } else {
                signed_txs
                    .iter()
                    .copied()
                    .zip(prepared_txs.iter())
                    .enumerate()
                    .map(validate_tx)
                    .collect()
            };
            for maybe_err in tx_errors {
                if let Some(err) = maybe_err {
                    return Err(err);
                }
            }
            let expected_merkle_root = if let Some(pool) = static_pool.as_ref() {
                pool.install(|| {
                    let merkle_tree: MerkleTree<TransactionEntrypoint> =
                        MerkleTree::from_typed_leaves_parallel(entrypoint_hashes);
                    merkle_tree.root()
                })
            } else {
                let merkle_tree: MerkleTree<TransactionEntrypoint> =
                    entrypoint_hashes.into_iter().collect();
                merkle_tree.root()
            };
            let actual_merkle_root = block.header().merkle_root();
            if expected_merkle_root != actual_merkle_root {
                return Err(BlockValidationError::MerkleRootMismatch);
            }
            Ok(())
        }
        /// Static checks for a strict Sumeragi-v2 test fixture.
        #[cfg(any(test, feature = "iroha-core-tests"))]
        #[allow(
            clippy::too_many_arguments,
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::collapsible_else_if
        )]

        /// Drain transaction-scoped settlement evidence and derive canonical
        /// post-execution statements bound to each transaction's exact route,
        /// lane-payload coordinate, and final result-bearing block header.
        ///
        /// Both the DAG and live-sequential execution paths must pass through this
        /// function after all deterministic effects. Relay/status publication is
        /// deliberately deferred until the accepted commit is durable.
        #[allow(clippy::too_many_lines)]
        fn finalize_lane_settlement_evidence(
            block: &SignedBlock,
            state_block: &mut StateBlock<'_>,
            routed_transactions: &[(HashOf<SignedTransaction>, crate::queue::RoutingDecision)],
            lane_summaries: &BTreeMap<LaneId, LaneSummary>,
        ) -> Result<Vec<iroha_data_model::nexus::LaneFinalityStatement>, BlockValidationError>
        {
            let mut native_amx_receipts_by_hash = BTreeMap::new();
            if let Some(bundle) = block.execution_context() {
                for (entrypoint, context) in block
                    .external_entrypoints_cloned()
                    .zip(bundle.external.iter())
                {
                    let Some(receipt) = context.native_amx_receipt.clone() else {
                        continue;
                    };
                    let Some(signed) = Self::signed_transaction_from_entrypoint(&entrypoint) else {
                        return Err(Self::execution_context_error(
                            "native AMX receipt is attached to an entrypoint without a signed transaction",
                        ));
                    };
                    let tx_hash = signed.hash();
                    if native_amx_receipts_by_hash
                        .insert(tx_hash, receipt)
                        .is_some()
                    {
                        return Err(Self::execution_context_error(format!(
                            "duplicate native AMX receipt for routed transaction {tx_hash}"
                        )));
                    }
                }
            }
            let mut lane_payload_coordinates = BTreeMap::new();
            if let Some(bundle) = block.execution_context() {
                for (ownership_idx, ownership) in bundle.lane_payload_ownerships.iter().enumerate()
                {
                    let lane_block_descriptor_hash = ownership
                        .lane_block_descriptor_hash
                        .ok_or_else(|| {
                            Self::execution_context_error(format!(
                                "lane payload ownership {ownership_idx} has no descriptor hash during settlement finalization"
                            ))
                        })?;
                    let previous = lane_payload_coordinates.insert(
                        (ownership.lane_id, ownership.dataspace_id),
                        LanePayloadCoordinate {
                            lane_incarnation: ownership.lane_incarnation,
                            lane_block_height: ownership.lane_block_height,
                            lane_block_descriptor_hash,
                        },
                    );
                    if previous.is_some() {
                        return Err(Self::execution_context_error(format!(
                            "duplicate exact lane payload ownership for lane {} dataspace {} during settlement finalization",
                            ownership.lane_id.as_u32(),
                            ownership.dataspace_id.as_u64()
                        )));
                    }
                }
            }
            let mut pending_settlements = state_block.drain_settlement_records();
            let mut pending_nexus_fee_receipts = state_block.drain_nexus_fee_records();
            let mut seen_transactions = BTreeSet::new();
            for (tx_hash, _) in routed_transactions {
                if !seen_transactions.insert(*tx_hash) {
                    return Err(Self::execution_context_error(format!(
                        "duplicate routed transaction {tx_hash} is not canonical"
                    )));
                }
            }
            let nexus_fee_receipts_active = state_block.nexus.fees.settlement_mode
                == iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            let mut lane_settlement_builders: BTreeMap<
                (LaneId, DataSpaceId),
                LaneSettlementBuilder,
            > = BTreeMap::new();
            for (tx_hash, decision) in routed_transactions {
                let mut counted_settlement_tx = false;
                if let Some(record) = pending_settlements.remove(tx_hash) {
                    lane_payload_coordinates
                        .get(&(decision.lane_id, decision.dataspace_id))
                        .ok_or_else(|| {
                            Self::execution_context_error(format!(
                                "settled lane {} dataspace {} has no exact lane payload ownership",
                                decision.lane_id.as_u32(),
                                decision.dataspace_id.as_u64()
                            ))
                        })?;
                    let builder = lane_settlement_builders
                        .entry((decision.lane_id, decision.dataspace_id))
                        .or_default();
                    builder.tx_count = builder.tx_count.saturating_add(1);
                    counted_settlement_tx = true;
                    builder.total_local_amount = builder
                        .total_local_amount
                        .try_add(&record.local_amount)
                        .map_err(|error| {
                            Self::execution_context_error(format!(
                                "lane settlement local total overflow: {error}"
                            ))
                        })?;
                    builder.total_xor_due = builder
                        .total_xor_due
                        .try_add(&record.xor_due)
                        .map_err(|error| {
                            Self::execution_context_error(format!(
                                "lane settlement XOR due total overflow: {error}"
                            ))
                        })?;
                    builder.total_xor_after_haircut = builder
                        .total_xor_after_haircut
                        .try_add(&record.xor_after_haircut)
                        .map_err(|error| {
                            Self::execution_context_error(format!(
                                "lane settlement post-haircut total overflow: {error}"
                            ))
                        })?;
                    builder.total_xor_variance = builder
                        .total_xor_variance
                        .try_add(&record.xor_variance)
                        .map_err(|error| {
                            Self::execution_context_error(format!(
                                "lane settlement variance total overflow: {error}"
                            ))
                        })?;
                    builder
                        .source_counts
                        .entry(record.asset_definition_id.clone())
                        .and_modify(|count| *count = count.saturating_add(1))
                        .or_insert(1);
                    let evidence = SwapEvidence {
                        epsilon_bps: record.epsilon_bps,
                        twap_window_seconds: record.twap_window_seconds,
                        liquidity_profile: record.liquidity_profile,
                        twap_local_per_xor: record.twap_local_per_xor.clone(),
                        volatility_bucket: record.volatility_bucket,
                    };
                    if builder
                        .swap_evidence
                        .as_ref()
                        .is_some_and(|existing| existing != &evidence)
                    {
                        return Err(Self::execution_context_error(format!(
                            "lane {} dataspace {} produced inconsistent settlement swap metadata",
                            decision.lane_id.as_u32(),
                            decision.dataspace_id.as_u64()
                        )));
                    }
                    builder.swap_evidence.get_or_insert(evidence);
                    builder.receipts.push(record.into_lane_receipt());
                }
                if let Some(record) = pending_nexus_fee_receipts.remove(tx_hash) {
                    if !nexus_fee_receipts_active {
                        iroha_logger::warn!(
                            height = block.header().height().get(),
                            tx = %tx_hash,
                            "dropping staged Nexus fee receipt before fee receipt activation height"
                        );
                    } else {
                        lane_payload_coordinates
                            .get(&(decision.lane_id, decision.dataspace_id))
                            .ok_or_else(|| {
                                Self::execution_context_error(format!(
                                    "fee-settled lane {} dataspace {} has no exact lane payload ownership",
                                    decision.lane_id.as_u32(),
                                    decision.dataspace_id.as_u64()
                                ))
                            })?;
                        let builder = lane_settlement_builders
                            .entry((decision.lane_id, decision.dataspace_id))
                            .or_default();
                        if !counted_settlement_tx {
                            builder.tx_count = builder.tx_count.saturating_add(1);
                            counted_settlement_tx = true;
                        }
                        builder.nexus_fee_receipts.push(record);
                    }
                }
                if let Some(receipt) = native_amx_receipts_by_hash.remove(tx_hash) {
                    let coordinate = lane_payload_coordinates
                        .get(&(decision.lane_id, decision.dataspace_id))
                        .ok_or_else(|| {
                            Self::execution_context_error(format!(
                                "native AMX lane {} dataspace {} has no exact lane payload ownership",
                                decision.lane_id.as_u32(),
                                decision.dataspace_id.as_u64()
                            ))
                        })?;
                    if receipt.lane_incarnation != coordinate.lane_incarnation
                        || receipt.lane_block_height != coordinate.lane_block_height
                    {
                        return Err(Self::execution_context_error(format!(
                            "native AMX receipt coordinates do not match exact lane payload ownership for lane {} dataspace {}",
                            decision.lane_id.as_u32(),
                            decision.dataspace_id.as_u64()
                        )));
                    }
                    let builder = lane_settlement_builders
                        .entry((decision.lane_id, decision.dataspace_id))
                        .or_default();
                    if !counted_settlement_tx {
                        builder.tx_count = builder.tx_count.saturating_add(1);
                    }
                    builder.native_amx_receipts.push(receipt);
                }
            }
            if !pending_settlements.is_empty()
                || !pending_nexus_fee_receipts.is_empty()
                || !native_amx_receipts_by_hash.is_empty()
            {
                return Err(Self::execution_context_error(format!(
                    "unbound settlement evidence remains after lane routing (settlement={}, nexus_fee={}, native_amx={})",
                    pending_settlements.len(),
                    pending_nexus_fee_receipts.len(),
                    native_amx_receipts_by_hash.len()
                )));
            }
            for ((lane_id, _), builder) in &mut lane_settlement_builders {
                if builder.buffer_snapshot.is_none() {
                    builder.buffer_snapshot =
                        compute_settlement_buffer_snapshot(state_block, *lane_id)
                            .map_err(Self::execution_context_error)?;
                }
                if let Some(snapshot) = &builder.buffer_snapshot
                    && let Some(metadata) = lane_metadata_by_id(state_block, *lane_id)
                {
                    match snapshot.status {
                        BufferStatus::Normal => {}
                        BufferStatus::Alert => iroha_logger::warn!(
                            lane = %metadata.alias,
                            "settlement buffer for lane {} dipped below the alert threshold (<{}%)",
                            metadata.alias,
                            state_block.settlement_engine().buffer_policy().alert
                        ),
                        BufferStatus::Throttle => iroha_logger::warn!(
                            lane = %metadata.alias,
                            "settlement buffer for lane {} entered throttle state (<{}%); reduce subsidised inclusion",
                            metadata.alias,
                            state_block.settlement_engine().buffer_policy().throttle
                        ),
                        BufferStatus::XorOnly => iroha_logger::warn!(
                            lane = %metadata.alias,
                            "settlement buffer for lane {} entered XOR-only state (<{}%); force XOR-denominated inclusion",
                            metadata.alias,
                            state_block.settlement_engine().buffer_policy().xor_only
                        ),
                        BufferStatus::Halt => iroha_logger::error!(
                            lane = %metadata.alias,
                            "settlement buffer for lane {} hit the halt threshold (<{}%); pause settlement until refilled",
                            metadata.alias,
                            state_block.settlement_engine().buffer_policy().halt
                        ),
                    }
                }
            }
            let lane_settlement_commitments = lane_settlement_builders
                .into_iter()
                .map(|((lane_id, dataspace_id), builder)| {
                    #[cfg(feature = "telemetry")]
                    record_lane_settlement_metrics(
                        state_block.metrics(),
                        lane_id,
                        dataspace_id,
                        &builder,
                    );
                    let coordinate = lane_payload_coordinates
                        .get(&(lane_id, dataspace_id))
                        .ok_or_else(|| {
                            Self::execution_context_error(format!(
                                "settled lane {} dataspace {} has no exact lane payload ownership during commitment finalization",
                                lane_id.as_u32(),
                                dataspace_id.as_u64()
                            ))
                        })?;
                    Ok(LaneBlockCommitment {
                        block_height: coordinate.lane_block_height,
                        lane_id,
                        lane_incarnation: coordinate.lane_incarnation,
                        dataspace_id,
                        tx_count: builder.tx_count,
                        total_local_amount: builder.total_local_amount,
                        total_xor_due: builder.total_xor_due,
                        total_xor_after_haircut: builder.total_xor_after_haircut,
                        total_xor_variance: builder.total_xor_variance,
                        swap_metadata: builder
                            .swap_evidence
                            .map(SwapEvidence::into_lane_metadata),
                        receipts: builder.receipts,
                        nexus_fee_receipts: builder
                            .nexus_fee_receipts
                            .into_iter()
                            .map(|receipt| {
                                receipt.into_lane_receipt(
                                    coordinate.lane_block_height,
                                    lane_id,
                                    dataspace_id,
                                )
                            })
                            .collect(),
                        native_amx_receipts: builder.native_amx_receipts,
                    })
                })
                .collect::<Result<Vec<_>, BlockValidationError>>()?;
            Self::finalize_lane_settlement_commitments(
                block,
                state_block,
                &lane_settlement_commitments,
                lane_summaries,
                &lane_payload_coordinates,
            )
        }


        fn validated_committed_fragment_count(
            state_block: &StateBlock<'_>,
            advertised_committed_fragments: Option<u64>,
        ) -> Result<u64, BlockValidationError> {
            let ordinary = u64::try_from(state_block.committed_fragment_count()).map_err(|_| {
                Self::execution_context_error(
                    "ordinary committed fragment count exceeds the canonical u64 range",
                )
            })?;
            let expected = ordinary;
            if let Some(actual) = advertised_committed_fragments
                && actual != expected
            {
                return Err(BlockValidationError::CommittedFragmentCountMismatch {
                    expected,
                    actual,
                });
            }
            Ok(expected)
        }
        /// Execute a component fixture without a consensus height context.
        #[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]
        fn execute_and_record_canonical_outputs(
            block: &mut SignedBlock,
            state_block: &mut StateBlock<'_>,
            timings: Option<&mut ValidationTimings>,
            genesis: Option<&AuthenticatedGenesisOutputSource>,
        ) -> Result<(), BlockValidationError> {
            Self::execute_and_record_canonical_outputs_in_context(
                block,
                state_block,
                timings,
                genesis,
                crate::smartcontracts::isi::sccp::height::SccpHeightSourceV1::Unauthenticated,
            )
        }
        /// Execute and seal ordinary outputs. `sccp_height` names the authenticated consensus
        /// inputs of the block's height that the SCCP post-execution hook consumes
        /// (`specs/sccp.md` §4.3.2); `Unauthenticated` is reserved for component fixtures and
        /// v2 signed genesis, whose height-one context is frozen from the staged genesis.
        fn execute_and_record_canonical_outputs_in_context(
            block: &mut SignedBlock,
            state_block: &mut StateBlock<'_>,
            timings: Option<&mut ValidationTimings>,
            genesis: Option<&AuthenticatedGenesisOutputSource>,
            sccp_height: crate::smartcontracts::isi::sccp::height::SccpHeightSourceV1<'_>,
        ) -> Result<(), BlockValidationError> {
            let start = Instant::now();
            let mut timings = timings;
            // Full input/control authority is checked by the caller. The State
            // owner rechecks proposal/source bindings and owns every actual phase.
            block
                .validate_proposal_commitments()
                .map_err(Self::execution_context_error)?;
            Self::validate_sccp_exempt_cap(block, state_block)?;
            let advertised_fragments = block.committed_fragment_count();
            let advertised_policy = block.axt_policy_snapshot().cloned();
            let advertised_transitions = block.axt_transitioned_dataspaces().cloned();
            if block.has_results() {
                block
                    .validate_output_merkle_cache()
                    .map_err(|error| Self::execution_context_error(error.to_string()))?;
                let policy = advertised_policy.as_ref().ok_or_else(|| {
                    BlockValidationError::AxtEnvelopeValidationFailed(
                        AxtEnvelopeValidationDetails {
                            message: "block result is missing its AXT post-state policy snapshot"
                                .to_owned(),
                            reason: AxtRejectReason::MissingPolicy,
                            snapshot_version: None,
                            dataspace: None,
                            lane: None,
                            active_handle_era: None,
                            next_handle_counter: None,
                        },
                    )
                })?;
                policy.validate().map_err(|error| {
                    BlockValidationError::AxtEnvelopeValidationFailed(
                        AxtEnvelopeValidationDetails {
                            message: format!("invalid AXT policy snapshot: {error}"),
                            reason: AxtRejectReason::PolicyDenied,
                            snapshot_version: Some(policy.version),
                            dataspace: None,
                            lane: None,
                            active_handle_era: None,
                            next_handle_counter: None,
                        },
                    )
                })?;
            }
            if block
                .execution_context()
                .is_some_and(|bundle| bundle.native_lane_decisions.is_some())
            {
                return Err(Self::execution_context_error(
                    "native outputs require their original recorded source owner",
                ));
            }
            let expired =
                crate::smartcontracts::isi::sorafs::expire_pin_manifests_at_consensus_time(
                    state_block,
                )
                .map_err(|error| match error {
                    crate::smartcontracts::isi::sorafs::PinExpiryMaintenanceError::Storage(
                        error,
                    ) => BlockValidationError::StateStorageAdmission(error),
                    crate::smartcontracts::isi::sorafs::PinExpiryMaintenanceError::Instruction(
                        error,
                    ) => Self::execution_context_error(format!(
                        "SoraFS pin expiry maintenance failed: {error}"
                    )),
                })?;
            if expired != 0 {
                iroha_logger::debug!(
                    count = expired,
                    "retired SoraFS pins at the block consensus timestamp"
                );
            }
            let finalize = |state: &mut StateBlock<'_>,
                            source: &SignedBlock,
                            routes: &[crate::queue::RoutingDecision]| {
                // The Sumeragi schedule is World state: advance it before the seal fixes the
                // block's World delta.
                state
                    .advance_requested_sumeragi_schedule()
                    .map_err(BlockValidationError::from)?;
                state.advance_requested_sumeragi_lanes();
                Self::validate_native_genesis_policy(source, state)?;
                Self::finalize_owned_execution_metadata(
                    source,
                    state,
                    routes,
                    advertised_fragments,
                    advertised_policy.as_ref(),
                    advertised_transitions.as_ref(),
                    sccp_height,
                )
            };
            // The applying constructor already owns the recorder over pristine
            // controls and start effects. A late reset would erase that prefix.
            state_block
                .require_original_execution_recorder()
                .map_err(Self::execution_context_error)?;
            let result = state_block.execute_and_seal_ordinary_outputs(block, genesis, finalize);
            result.map_err(|error| match error {
                crate::state::ExecutionOutputSealError::Storage(error) => {
                    BlockValidationError::StateStorageAdmission(error)
                }
                crate::state::ExecutionOutputSealError::Owner(reason) => {
                    Self::execution_context_error(reason)
                }
                crate::state::ExecutionOutputSealError::Deferred(reason) => {
                    BlockValidationError::ExecutionDeferred(reason)
                }
                crate::state::ExecutionOutputSealError::Finalizer(error) => error,
            })?;
            state_block
                .verify_execution_output_seal(block)
                .map_err(Self::execution_context_error)?;
            if let Some(timings) = timings.as_deref_mut() {
                let elapsed = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                timings.execution_tx_apply_ms = elapsed;
                timings.execution_tx_apply_sequential_ms = elapsed;
            }
            Ok(())
        }
        /// Execute an ordinary component fixture whose input admission is supplied by its test.
        ///
        /// This late-capture adapter does not cover already-applied start effects and
        /// cannot qualify a complete production block witness. Pre-staged consensus
        /// controls require the recording-aware constructor and consuming adapter.
        #[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]
        pub fn validate_unchecked(
            block: SignedBlock,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<ValidBlock> {
            let guard = Self::begin_component_fixture_recording(state_block)
                .expect("component fixture cannot claim a pre-staged control or prefix witness");
            Self::validate_recorded_unchecked(block, state_block, guard)
        }

        /// Execute a fixture with the original recorder already acquired by its
        /// writer-first applying constructor. This consumes that same guard.
        #[cfg(any(test, feature = "iroha-core-tests", feature = "bench"))]
        pub(crate) fn validate_recorded_unchecked(
            mut block: SignedBlock,
            state_block: &mut StateBlock<'_>,
            exec_witness_guard: crate::exec_witness::ExecWitnessGuard,
        ) -> WithEvents<ValidBlock> {
            Self::validate_staged_execution_controls(&block, state_block)
                .expect("unchecked certified merge block requires its exact pre-staged sidecar");
            Self::execute_and_record_canonical_outputs(&mut block, state_block, None, None)
                .expect("unchecked block should have internally consistent entrypoint hashes");
            if let Err(error) = validate_axt_envelopes(&block, state_block) {
                panic!("AXT envelope validation failed on unchecked block: {error}");
            }
            state_block
                .capture_exec_witness()
                .expect("component output must preserve its exact execution witness");
            drop(exec_witness_guard);
            WithEvents::new(ValidBlock::new_unverified(block))
        }
        #[cfg(any(test, feature = "iroha-core-tests"))]
        /// Add additional signature for [`Self`]
        ///
        /// # Errors
        ///
        /// If given signature doesn't match block hash
        pub fn add_signature(
            &mut self,
            signature: BlockSignature,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            use SignatureVerificationError::{Other, UnknownSignatory, UnknownSignature};
            let signatory = usize::try_from(signature.index()).map_err(|_err| UnknownSignatory)?;
            let signatory = topology.as_ref().get(signatory).ok_or(UnknownSignatory)?;
            if matches!(topology.role(signatory), Role::Leader | Role::Undefined) {
                return Err(UnknownSignatory);
            }
            signature
                .signature()
                .verify_hash(signatory.public_key(), self.as_ref().hash())
                .map_err(|_err| UnknownSignature)?;
            self.block.add_signature(signature).map_err(|_err| Other)?;
            self.clear_signatures_verified();
            Ok(())
        }
        /// Replace block's signatures. Returns previous block signatures
        ///
        /// # Errors
        ///
        /// - Replacement signatures don't contain the leader signature
        /// - Replacement signatures contain unknown signatories
        /// - Replacement signatures contain incorrect signatures
        /// - Replacement signatures contain duplicate signatures
        pub fn replace_signatures(
            &mut self,
            signatures: BTreeSet<BlockSignature>,
            topology: &Topology,
        ) -> WithEvents<Result<BTreeSet<BlockSignature>, SignatureVerificationError>> {
            let mut seen = BTreeSet::new();
            for signature in &signatures {
                let signer = match usize::try_from(signature.index()) {
                    Ok(idx) => idx,
                    Err(_) => {
                        return WithEvents::new(Err(SignatureVerificationError::UnknownSignatory));
                    }
                };
                if !seen.insert(signer) {
                    return WithEvents::new(Err(SignatureVerificationError::DuplicateSignature {
                        signer,
                    }));
                }
            }
            let was_verified = self.signatures_verified;
            let Ok(prev_signatures) = self.block.replace_signatures(signatures) else {
                return WithEvents::new(Err(SignatureVerificationError::Other));
            };
            self.clear_signatures_verified();
            let result = if let Err(err) = Self::is_commit(self.as_ref(), topology) {
                self.block
                    .replace_signatures(prev_signatures)
                    .expect("INTERNAL BUG: invalid signatures in block");
                self.signatures_verified = was_verified;
                Err(err)
            } else {
                Ok(prev_signatures)
            };
            WithEvents::new(result)
        }
        /// Transition block to [`CommittedBlock`].
        ///
        /// # Errors
        ///
        /// - Block is missing the leader signature
        /// - Block doesn't have enough valid signatures
        pub fn commit(self, topology: &Topology) -> WithCommittedBlockEvents {
            WithEvents::new(
                match Self::is_commit_internal(self.as_ref(), topology, self.signatures_verified) {
                    Err(err) => Err((Box::new(self), Box::new(err.into()))),
                    Ok(()) => Ok(CommittedBlock::from_execution(self)),
                },
            )
        }

        #[cfg(test)]
        /// Commit using a prevalidated signer set (e.g., from a QC).
        ///
        /// The block signatures are still verified to guard against forged aggregates; `signers`
        /// must match a quorum of signatures present on the block.
        pub fn commit_with_signers(
            self,
            topology: &Topology,
            signers: &BTreeSet<ValidatorIndex>,
            allow_quorum_bypass: bool,
        ) -> WithCommittedBlockEvents {
            let validation = (|| -> Result<(), SignatureVerificationError> {
                // Ensure the QC-reported signer set matches the expected quorum shape.
                Self::verify_signer_set(topology, signers, allow_quorum_bypass)?;
                // Block signatures can be a trimmed subset when the QC carries the quorum.
                // Validate all present signatures against the topology and ensure they
                // don't contradict the QC signer set.
                if !self.signatures_verified {
                    Self::verify_unique_signers(self.as_ref())?;
                    Self::verify_signatures_against_topology(self.as_ref(), topology)?;
                }
                Ok(())
            })();
            WithEvents::new(match validation {
                Err(err) => Err((Box::new(self), Box::new(err.into()))),
                Ok(()) => Ok(CommittedBlock::from_execution(self)),
            })
        }
        /// Like [`Self::commit`], but without block signature checks.
        ///
        /// Useful e.g. for Explorer, which assumes all blocks from Iroha are valid, and
        /// only executes them to produce state changes.
        pub fn commit_unchecked(self) -> WithEvents<CommittedBlock> {
            WithEvents::new(CommittedBlock::from_execution(self))
        }
        /// Check if block satisfy requirements to be committed
        ///
        /// # Errors
        ///
        /// - Block is missing the leader signature
        /// - Block doesn't have enough signatures for quorum
        pub(crate) fn is_commit(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            Self::is_commit_internal(block, topology, false)
        }
        fn is_commit_internal(
            block: &SignedBlock,
            topology: &Topology,
            signatures_verified: bool,
        ) -> Result<(), SignatureVerificationError> {
            if !block.header().is_genesis() {
                if !signatures_verified {
                    Self::verify_unique_signers(block)?;
                    Self::verify_leader_signature(block, topology)?;
                    Self::verify_signatures_against_topology(block, topology)?;
                }
                let SignatureTally {
                    present: present_signatures,
                    counted: votes_count,
                    set_b_signatures,
                } = commit_signature_tally(block, topology);
                iroha_logger::info!(
                    signatures_present = present_signatures,
                    votes = votes_count,
                    set_b_signatures,
                    min_votes = topology.min_votes_for_commit(),
                    topo_len = topology.as_ref().len(),
                    block_hash = %block.hash(),
                    "verifying block commit quorum"
                );
                if votes_count < topology.min_votes_for_commit() {
                    return Err(SignatureVerificationError::NotEnoughSignatures {
                        votes_count,
                        min_votes_for_commit: topology.min_votes_for_commit(),
                    });
                }
            }
            Ok(())
        }
        /// Fallibly add additional signatures for [`Self`].
        ///
        /// # Errors
        ///
        /// Returns [`iroha_crypto::Error::Signing`] when the configured signing
        /// backend rejects the private-key material or finalized block header hash.
        pub fn try_sign(
            &mut self,
            key_pair: &KeyPair,
            topology: &Topology,
        ) -> Result<(), iroha_crypto::Error> {
            let signatory_idx = topology
                .position(key_pair.public_key())
                .expect("INTERNAL BUG: Node is not in topology");
            self.block.try_sign(key_pair.private_key(), signatory_idx)?;
            self.clear_signatures_verified();
            Ok(())
        }
        /// Add additional signatures for [`Self`].
        pub fn sign(&mut self, key_pair: &KeyPair, topology: &Topology) {
            self.try_sign(key_pair, topology)
                .expect("signing should succeed for a valid validator key and block header");
        }
        #[cfg(test)]
        pub(crate) fn new_dummy(leader_private_key: &PrivateKey) -> Self {
            Self::new_dummy_and_modify_header(leader_private_key, |_| {})
        }
        #[cfg(test)]
        pub(crate) fn new_dummy_and_modify_header(
            leader_private_key: &PrivateKey,
            f: impl FnOnce(&mut BlockHeader),
        ) -> Self {
            let merkle_root = MerkleTree::<TransactionEntrypoint>::default().root();
            let mut header =
                BlockHeader::new(nonzero_ext::nonzero!(2_u64), None, merkle_root, 0, 0);
            f(&mut header);
            if header.confidential_features().is_none() {
                header.set_confidential_features(Some(EMPTY_CONFIDENTIAL_FEATURE_DIGEST));
            }
            let builder = BlockBuilder(Chained {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
                npos_consensus_effects: None,
                global_beacon_pulse: None,
                execution_context: None,
            });
            let default_policies = crate::da::proof_policy_bundle(
                &iroha_config::parameters::actual::LaneConfig::default(),
            );
            let mut unverified_block: SignedBlock = builder
                .with_da_proof_policies(Some(default_policies))
                .sign(leader_private_key)
                .unpack(|_| {})
                .into();
            unverified_block
                .set_execution_outputs(
                    Vec::new(),
                    0,
                    Default::default(),
                    Vec::new(),
                    Default::default(),
                    Default::default(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("empty structural block has complete execution metadata");
            // Keep the exact payload whose commitments were signed above.
            // The transaction-only constructor would discard the DA policy body.
            Self::new_unverified(unverified_block)
        }
    }
    impl From<ValidBlock> for SignedBlock {
        fn from(source: ValidBlock) -> Self {
            source.block
        }
    }
    impl AsRef<SignedBlock> for ValidBlock {
        fn as_ref(&self) -> &SignedBlock {
            &self.block
        }
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    impl AsMut<SignedBlock> for ValidBlock {
        fn as_mut(&mut self) -> &mut SignedBlock {
            &mut self.block
        }
    }
    #[test]
    fn dummy_block_populates_proof_policy_hash() {
        let kp = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let block = ValidBlock::new_dummy(kp.private_key());
        assert!(block.as_ref().header().da_proof_policies_hash().is_some());
        let block = block.as_ref();
        let policy = block
            .da_proof_policies()
            .expect("dummy retains its signed policy body");
        assert_eq!(
            block.header().da_proof_policies_hash(),
            Some(HashOf::new(policy))
        );
        block
            .validate_proposal_commitments()
            .expect("dummy proposal commitments match its actual payload");
        block
            .validate_output_merkle_cache()
            .expect("dummy outputs retain complete empty execution metadata");
        assert_eq!(block.committed_fragment_count(), Some(0));
        assert!(block.axt_policy_snapshot().is_some());
        assert!(
            block
                .axt_transitioned_dataspaces()
                .is_some_and(BTreeSet::is_empty)
        );
        block
            .signatures()
            .next()
            .unwrap()
            .signature()
            .verify_hash(kp.public_key(), block.hash())
            .expect("dummy signature authenticates the complete retained proposal");
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            soracloud_runtime::{
                SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
                SoracloudDeterministicStateMutation, SoracloudLocalReadRequest,
                SoracloudLocalReadResponse, SoracloudOrderedMailboxExecutionRequest,
                SoracloudOrderedMailboxExecutionResult, SoracloudRuntime,
                SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
                SoracloudRuntimeReadHandle, SoracloudRuntimeSnapshot,
            },
            state::{State, World},
            sumeragi::network_topology::{Topology, test_topology_with_keys},
            tx::AcceptedTransaction,
        };
        use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey, Signature, SignatureOf};
        use iroha_data_model::{
            Registrable,
            block::{
                consensus::SumeragiLanePayloadOwnership, error::BlockRejectionReason as Reason,
            },
            consensus::{
                ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
                NposConsensusEffects,
            },
            da::{
                commitment::{
                    DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, RetentionClass,
                },
                pin_intent::{DaPinIntent, DaPinIntentBundle},
                types::{BlobDigest, StorageTicketId},
            },
            isi::{InstructionBox, Log, error::Mismatch},
            merge::MergeQuorumCertificate,
            nexus::{
                AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, DataSpaceCatalog,
                DataSpaceMetadata, LaneCatalog, LaneConfig,
            },
            parameter::{Parameter, Parameters, system::SumeragiNposParameters},
            prelude::{Account, Domain, Register},
            soracloud::{
                SORA_STATE_BINDING_VERSION_V1, SoraCapabilityPolicyV1,
                SoraCertifiedResponsePolicyV1, SoraContainerManifestRefV1, SoraContainerManifestV1,
                SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraLifecycleHooksV1,
                SoraMailboxContractV1, SoraNetworkPolicyV1, SoraResourceLimitsV1,
                SoraRolloutPolicyV1, SoraRuntimeReceiptV1, SoraServiceDeploymentStateV1,
                SoraServiceHandlerClassV1, SoraServiceHandlerV1, SoraServiceHealthStatusV1,
                SoraServiceMailboxMessageV1, SoraServiceManifestV1, SoraServiceRuntimeStateV1,
                SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1,
                SoraStateMutationOperationV1,
            },
            sorafs::pin_registry::ManifestDigest,
            transaction::{
                Executable, SignedTransaction, TransactionBuilder, error::TransactionLimitError,
            },
            trigger::DataTriggerSequence,
        };
        use iroha_logger::Level;
        use iroha_model_base::domain::DomainId;
        use iroha_model_base::metadata::Metadata;
        use iroha_model_base::name::Name;
        use iroha_model_base::peer::PeerId;
        use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
        use iroha_primitives::time::TimeSource;
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, gen_account_in};
        use mv::cell::Cell;
        use mv::storage::StorageReadOnly;
        use nonzero_ext::nonzero;
        use std::{
            borrow::Cow,
            collections::{BTreeMap, BTreeSet},
            num::{NonZeroU16, NonZeroU32, NonZeroU64},
            path::PathBuf,
            str::FromStr,
            sync::Arc,
            time::Duration,
        };
        include!("block/canonical_carrier_source_tests.rs");
        include!("block/merge_beacon_owner_tests.rs");
        include!("block/parallel_account_profile_tests.rs");
        mod native_validation_tests {
            include!("block/native_validation_tests.rs");
        }
        fn sumeragi_v2_test_profile(block: &SignedBlock) -> ConsensusValidationProfile {
            ConsensusValidationProfile::SumeragiV2 {
                block_cadence: Duration::from_millis(1),
                context: SumeragiV2ValidationContext::for_body_without_context_bound_attachments(
                    block,
                ),
            }
        }
        macro_rules! validate_static_test_block {
            ($block:expr, $topology:expr, $view:expr, $time_source:expr) => {
                ValidBlock::validate_static_state_dependent(
                    $block,
                    $topology,
                    &ALICE_ID,
                    $view,
                    false,
                    $time_source,
                    false,
                    sumeragi_v2_test_profile($block),
                )
            };
        }
        macro_rules! validate_static_current_test_block {
            ($block:expr, $topology:expr, $view:expr, $time_source:expr) => {
                ValidBlock::validate_static_state_dependent(
                    $block,
                    $topology,
                    &ALICE_ID,
                    $view,
                    false,
                    $time_source,
                    false,
                    sumeragi_v2_test_profile($block),
                )
            };
        }
        macro_rules! validate_voting_test_block {
            ($block:expr, $topology:expr, $time_source:expr, $state:expr, $keys:expr, $cadence:expr) => {{
                let context = authenticated_permissioned_successor_context($state, $keys);
                ValidBlock::validate_sumeragi_v2_fixture_keep_voting_block(
                    $block,
                    $topology,
                    &ALICE_ID,
                    $time_source,
                    $cadence,
                    $state,
                    false,
                    false,
                    SumeragiV2ValidationContext::from_height_context(&context),
                )
            }};
            (without_authenticated_context; $block:expr, $topology:expr, $time_source:expr, $state:expr, $cadence:expr) => {{
                let block = $block;
                let context =
                    SumeragiV2ValidationContext::for_body_without_context_bound_attachments(&block);
                ValidBlock::validate_sumeragi_v2_fixture_keep_voting_block(
                    block,
                    $topology,
                    &ALICE_ID,
                    $time_source,
                    $cadence,
                    $state,
                    false,
                    false,
                    context,
                )
            }};
        }
        macro_rules! setup_static_validation_world {
            ($kura:ident, $query:ident, $key_pairs:ident, $topology:ident, $leader:ident, $world:ident) => {
                let $kura = Arc::new(Kura::blank_kura_for_testing());
                let $query = LiveQueryStore::start_test();
                let mut $key_pairs = core::iter::repeat_with(|| {
                    crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
                })
                .take(4)
                .collect::<Vec<_>>();
                $key_pairs.sort_by_key(|key| PeerId::new(key.public_key().clone()));
                let $topology = test_topology_with_keys(&$key_pairs);
                let $leader = &$key_pairs[0];
                let mut $world = World::new();
                insert_active_consensus_keys(&mut $world, &$key_pairs);
            };
        }
        macro_rules! setup_da_validation_world {
            ($kura:ident, $query:ident, $leader:ident, $topology:ident, $world:ident, $keys:ident) => {
                let $kura = Arc::new(Kura::blank_kura_for_testing());
                let $query = LiveQueryStore::start_test();
                let mut $keys = (0..4)
                    .map(|_| checked_keypair_with_algorithm(Algorithm::BlsNormal))
                    .collect::<Vec<_>>();
                $keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
                let $leader = $keys[0].clone();
                let $topology = test_topology_with_keys(&$keys);
                let mut $world = World::new();
                insert_active_consensus_keys(&mut $world, &$keys);
            };
        }
        macro_rules! validate_signed_voting_test_block {
            ($signed:ident, $topology:ident, $state:ident, $time_source:ident, $result:ident, $keys:ident, $cadence:expr) => {
                let (_handle, $time_source) =
                    TimeSource::new_mock($signed.header().creation_time());
                let $result = validate_voting_test_block!(
                    $signed,
                    &$topology,
                    &$time_source,
                    &$state,
                    &$keys,
                    $cadence
                )
                .unpack(|_| {});
            };
        }
        macro_rules! setup_elastic_lane_validation_state {
            ($kura:ident, $key_pairs:ident, $topology:ident, $leader:ident, $state:ident) => {
                let $kura = Arc::new(Kura::blank_kura_for_testing());
                let query = LiveQueryStore::start_test();
                let mut $key_pairs = core::iter::repeat_with(|| {
                    crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
                })
                .take(4)
                .collect::<Vec<_>>();
                $key_pairs.sort_by_key(|key| PeerId::new(key.public_key().clone()));
                let $topology = test_topology_with_keys(&$key_pairs);
                let $leader = &$key_pairs[0];
                let mut world = World::new();
                insert_active_consensus_keys(&mut world, &$key_pairs);
                let mut $state = State::new_for_testing(world, Arc::clone(&$kura), query);
                install_live_test_elastic_lane(&mut $state);
                install_test_lane_manifests_for_keypairs(&$state, &$key_pairs);
                let _prev_hash = commit_block_with_applied_lane_predecessors(
                    &$state,
                    &$kura,
                    &$topology,
                    $leader.private_key(),
                    &$key_pairs,
                    &[
                        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                        (LaneId::new(1), DataSpaceId::UNIVERSAL),
                    ],
                );
            };
        }
        macro_rules! setup_stateless_cache_state {
            ($kura:ident, $state:ident, $leader_private:ident, $topology:ident, $validator_keys:ident) => {
                let $kura = Arc::new(Kura::blank_kura_for_testing());
                let query = LiveQueryStore::start_test();
                let $validator_keys = core::iter::repeat_with(|| {
                    crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
                })
                .take(4)
                .collect::<Vec<_>>();
                let $topology = test_topology_with_keys(&$validator_keys);
                let $leader_private = $validator_keys[0].private_key().clone();
                let mut world = World::new();
                insert_active_consensus_keys(&mut world, &$validator_keys);
                let mut $state = State::new_for_testing(world, Arc::clone(&$kura), query);
                install_test_lane_manifests_for_keypairs(&$state, &$validator_keys);
                let mut pipeline = $state.view().pipeline().clone();
                pipeline.stateless_cache_cap = 64;
                $state.set_pipeline(pipeline);
                let _ = commit_block_at_height(
                    &$state,
                    &$kura,
                    &$topology,
                    &$leader_private,
                    1,
                    None,
                    0,
                );
            };
        }
        fn authenticated_permissioned_successor_context(
            state: &State,
            validator_keys: &[KeyPair],
        ) -> iroha_data_model::block::consensus_v2::HeightContext {
            use iroha_data_model::block::consensus_v2 as wire;

            let mut ordered_validator_keys = validator_keys.iter().collect::<Vec<_>>();
            ordered_validator_keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
            let roster = ordered_validator_keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let (mint_finality_authorization, mint_finality_authority) =
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                    state.network_id,
                    u64::MAX,
                    &roster,
                );
            let genesis_parameters = wire::SumeragiV2GenesisContextParameters::recommended();
            let mut parent_context = wire::HeightContext {
                network_id: state.network_id,
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                kagemusha_mint_finality_authorization: mint_finality_authorization,
                kagemusha_mint_finality_authority: mint_finality_authority,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster)
                    .expect("cache fixture has canonical equal-vote quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"cache fixture parent nexus context"),
                execution_policy_hash: Hash::prehashed(genesis_parameters.execution_policy_hash),
                da_layout: genesis_parameters.da_layout,
                leader_seed: [0x41; 32],
            };
            parent_context
                .validate()
                .expect("cache fixture parent height context is canonical");
            for height in 1..=state.view().height() {
                let parent = state
                    .kura()
                    .get_block(NonZeroUsize::new(height).expect("parent height is nonzero"))
                    .expect("authenticated validation fixture retains each parent body");
                let subject = wire::BlockSubject {
                    parent_block_hash: parent.header().prev_block_hash(),
                    block_hash: parent.hash(),
                    payload_hash: parent
                        .canonical_proposal_wire_hash()
                        .expect("cache fixture parent has canonical proposal bytes"),
                };
                let round = wire::ConsensusRound {
                    context_id: parent_context.id(),
                    height: parent_context.height,
                    view: parent.header().view_change_index(),
                };
                let parent_wire = parent
                    .encode_wire()
                    .expect("cache fixture parent has canonical executed bytes");
                let execution_commitment =
                    wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                        Hash::new(b"cache fixture parent state"),
                        Hash::new(b"cache fixture post state"),
                        Hash::new(b"cache fixture ordinary writes"),
                        u64::try_from(parent_wire.len())
                            .expect("cache fixture parent length fits u64"),
                        parent
                            .executed_block_wire_hash()
                            .expect("cache fixture parent has a result-bearing wire hash"),
                    );
                let vote = wire::Vote {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Commit,
                    subject,
                    execution_commitment,
                    signer: 0,
                    signature: Vec::new(),
                };
                let preimage = vote.signature_preimage();
                let shares = ordered_validator_keys[..3]
                    .iter()
                    .map(|key| {
                        Signature::new(key.private_key(), &preimage)
                            .payload()
                            .to_vec()
                    })
                    .collect::<Vec<_>>();
                let parent_qc = wire::QuorumCertificate {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Commit,
                    subject,
                    execution_commitment,
                    signers: vec![0, 1, 2],
                    aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    )
                    .expect("aggregate cache fixture parent CommitQC"),
                };
                let validator_set_pops = ordered_validator_keys
                    .iter()
                    .map(|key| {
                        iroha_crypto::bls_normal_pop_prove(key.private_key())
                            .expect("cache fixture validator PoP")
                    })
                    .collect::<Vec<_>>();
                let parent_artifact = wire::finality::V2FinalityArtifact::new(
                    parent_context,
                    subject,
                    parent_qc,
                    validator_set_pops,
                );
                let verified_parent = VerifiedV2FinalityArtifact::verify(parent_artifact)
                    .expect("cache fixture parent finality is genuinely signed");
                parent_context = crate::sumeragi::v2_context::build_successor_height_context(
                    verified_parent.artifact(),
                    Hash::new(b"cache fixture successor nexus context"),
                    None,
                )
                .expect("cache fixture has a canonical successor height context");
            }
            parent_context
        }
        macro_rules! setup_cacheable_transaction {
            ($state:ident, $tx_handle:ident, $tx_time_source:ident, $tx_hash:ident, $accepted:ident) => {
                let ($tx_handle, $tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
                let (authority, signer) = gen_account_in("cache-test");
                let tx = TransactionBuilder::new_with_time_source(
                    $state.network_id,
                    authority,
                    &$tx_time_source,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(Level::INFO, "cacheable".to_owned())])
                .sign(signer.private_key());
                let $tx_hash = crate::tx::StatelessValidationCacheKey::new(&tx);
                let $accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            };
        }
        macro_rules! build_cacheable_block {
            ($state:ident, $leader_private:ident, $accepted:ident, $block_handle:ident, $block_time_source:ident, $signed_block:ident) => {
                let ($block_handle, $block_time_source) =
                    TimeSource::new_mock(Duration::from_millis(1));
                let entrypoint_hash = $accepted.hash_as_entrypoint();
                let builder =
                    BlockBuilder::new_with_time_source(vec![$accepted], $block_time_source.clone());
                let builder = builder.chain(0, $state.view().latest_block().as_deref());
                let proposal_height = builder.0.header.height().get();
                let proposal_view = builder.0.header.view_change_index();
                let validator_set = $state
                    .resolve_lane_committee_at_height(
                        crate::state::LaneAuthorityRoute::new(
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        ),
                        proposal_height,
                    )
                    .expect("cache fixture lane authority resolves")
                    .into_validators();
                let ownership = sample_lane_payload_ownership_for_context_at_slot(
                    proposal_height,
                    proposal_view,
                    LaneId::SINGLE,
                    DataSpaceId::UNIVERSAL,
                    $state
                        .lane_incarnation(LaneId::SINGLE)
                        .expect("cache fixture default lane incarnation"),
                    1,
                    0,
                    vec![0],
                    vec![Hash::from(entrypoint_hash)],
                    &validator_set,
                );
                let execution_context =
                    BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                        entrypoint_hash,
                        LaneId::SINGLE,
                        DataSpaceId::UNIVERSAL,
                    )])
                    .with_lane_payload_ownerships(vec![ownership]);
                let builder = builder.with_execution_context(Some(execution_context));
                let new_block = with_current_state_da_sidecars(builder, &$state)
                    .sign(&$leader_private)
                    .unpack(|_| {});
                let $signed_block: SignedBlock = SignedBlock::from(new_block);
            };
        }
        include!("block/post_execution_tail_tests.rs");
        include!("block/sccp_call_site_tests.rs");
        include!("block/autonomous_merge_carrier_content_tests.rs");
        fn checked_block_signature(
            private_key: &PrivateKey,
            block_hash: HashOf<BlockHeader>,
        ) -> SignatureOf<BlockHeader> {
            SignatureOf::try_from_hash(private_key, block_hash)
                .expect("test block signing should succeed")
        }
        fn checked_seeded_keypair(seed: &[u8], algorithm: Algorithm) -> KeyPair {
            KeyPair::try_from_seed(seed.to_vec(), algorithm)
                .expect("test block seeded keypair should be valid")
        }
        fn checked_da_ack_signature(byte: u8) -> Signature {
            Signature::try_from_bytes(&[byte; 64])
                .expect("checked core block DA acknowledgement signature fixture")
        }
        fn test_da_network_id() -> iroha_data_model::NetworkId {
            iroha_data_model::NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xDA; 32])),
            )
        }
        fn test_da_owner_keypair() -> KeyPair {
            checked_seeded_keypair(&[0xDB; 32], Algorithm::Ed25519)
        }
        fn insert_test_da_owner(world: &mut World) {
            world.accounts.insert(
                iroha_data_model::account::AccountId::new(
                    test_da_owner_keypair().public_key().clone(),
                ),
                iroha_data_model::account::AccountValue::new(
                    iroha_data_model::account::AccountDetails::default(),
                ),
            );
        }
        fn install_test_da_admission_policy(state: &State, epoch: u64) {
            use iroha_data_model::da::ingest::{
                DaIngestAdmissionLaneV1, DaIngestAdmissionPolicyV1,
            };

            let lane_id = LaneId::SINGLE;
            let view = state.view();
            let proposal_height = u64::try_from(view.height())
                .expect("test height fits u64")
                .checked_add(1)
                .expect("test proposal height advances");
            let lane_incarnation =
                StateReadOnly::lane_incarnation_at_height(&view, lane_id, proposal_height)
                    .expect("default test lane is active at the candidate height");
            drop(view);
            let policy = DaIngestAdmissionPolicyV1 {
                version: DaIngestAdmissionPolicyV1::VERSION,
                revision: 1,
                expected_previous_policy_hash: None,
                lanes: vec![DaIngestAdmissionLaneV1 {
                    lane_id,
                    lane_incarnation,
                    producers: vec![AccountId::new(test_da_owner_keypair().public_key().clone())],
                    current_epoch: epoch,
                    grace_epoch: None,
                }],
            };
            policy
                .validate_transition(None)
                .expect("canonical test DA admission policy");
            let mut parameters = state.world.parameters.block();
            parameters.set_parameter(Parameter::Custom(policy.into_custom_parameter()));
            parameters.commit();
        }
        fn test_da_pin_intent(
            lane_id: LaneId,
            epoch: u64,
            sequence: u64,
            storage_ticket: StorageTicketId,
            manifest_hash: ManifestDigest,
        ) -> DaPinIntent {
            let owner_keypair = test_da_owner_keypair();
            crate::da::signed_test_pin_intent(
                crate::da::signed_test_ingest_authorization(
                    test_da_network_id(),
                    &owner_keypair,
                    lane_id,
                    epoch,
                    sequence,
                    1,
                ),
                &owner_keypair,
                storage_ticket,
                manifest_hash,
                None,
            )
        }
        fn axt_post_snapshot(sub_nonce: u64) -> AxtPolicySnapshot {
            let entries = vec![AxtPolicyBinding {
                dsid: DataSpaceId::new(7),
                policy: AxtPolicyEntry {
                    manifest_root: [0xA7; 32],
                    target_lane: LaneId::new(2),
                    active_handle_era: 3,
                    next_handle_counter: sub_nonce,
                    current_slot: 11,
                },
            }];
            AxtPolicySnapshot {
                version: AxtPolicySnapshot::compute_version(&entries),
                entries,
            }
        }
        #[test]
        fn advertised_axt_post_state_must_equal_deterministic_execution() {
            let computed = axt_post_snapshot(4);
            let forged = axt_post_snapshot(400);
            ValidBlock::validate_advertised_axt_post_state(None, &computed)
                .expect("locally produced blocks have no advertised pre-result snapshot");
            ValidBlock::validate_advertised_axt_post_state(Some(&computed), &computed)
                .expect("the exact deterministic post-state is accepted");
            assert!(matches!(
                ValidBlock::validate_advertised_axt_post_state(Some(&forged), &computed),
                Err(BlockValidationError::AxtEnvelopeValidationFailed(details))
                    if details.reason == AxtRejectReason::PolicyDenied
            ));
        }
        fn raw_block_with_da_sidecars(
            da_commitments: Option<DaCommitmentBundle>,
            da_pin_intents: Option<DaPinIntentBundle>,
        ) -> SignedBlock {
            let signer = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let policies = iroha_data_model::da::commitment::DaProofPolicyBundle::new(Vec::new());
            let mut header = BlockHeader::new(nonzero!(2_u64), None, None, 1, 0);
            header.set_da_proof_policies_hash(Some(HashOf::new(&policies)));
            let signature = BlockSignature::new(
                0,
                checked_block_signature(signer.private_key(), header.hash()),
            );
            let mut block = SignedBlock::presigned_with_payload(
                signature,
                BlockPayload {
                    header,
                    external_entrypoints: Vec::new(),
                    execution_context: None,
                    da_commitments: None,
                    da_proof_policies: Some(policies),
                    da_pin_intents: None,
                    npos_consensus_effects: None,
                    global_beacon_pulse: None,
                },
            );
            block.replace_da_sidecars_for_testing(da_commitments, da_pin_intents);
            block
        }
        #[test]
        fn da_sidecar_validation_rejects_noncanonical_empty_bundles() {
            let commitments = raw_block_with_da_sidecars(Some(DaCommitmentBundle::default()), None);
            assert_eq!(
                ValidBlock::validate_da_sidecar_hashes(&commitments),
                Err(BlockValidationError::NonCanonicalEmptyDaCommitmentBundle)
            );
            let pin_intents = raw_block_with_da_sidecars(None, Some(DaPinIntentBundle::default()));
            assert_eq!(
                ValidBlock::validate_da_sidecar_hashes(&pin_intents),
                Err(BlockValidationError::NonCanonicalEmptyDaPinIntentBundle)
            );
        }
        fn settlement_merge_reference_fixture() -> CertifiedMergeLedgerReference {
            let validator_set = Vec::<PeerId>::new();
            CertifiedMergeLedgerReference {
                version: 1,
                entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"block-settlement-sidecar")),
                encoded_len: 1,
                epoch_id: 1,
                execution_batch_hash: None,
                entrypoint_count: None,
                entrypoint_merkle_root: None,
                result_merkle_root: None,
                base_state_height: None,
                base_state_hash: None,
                merge_qc: MergeQuorumCertificate {
                    view: 0,
                    epoch_id: 1,
                    carrier_height: 1,
                    carrier_parent_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"block-settlement-parent",
                    )),
                    network_id: iroha_data_model::NetworkId::from_genesis_hash(
                        HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                            Hash::new(b"block-settlement-genesis"),
                        ),
                    ),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set,
                    signers_bitmap: Vec::new(),
                    signer_proofs: Vec::new(),
                    aggregate_signature: Vec::new(),
                    message_digest: Hash::new(b"block-settlement-qc"),
                },
            }
        }
        #[test]
        fn merge_reference_admission_rejects_partial_and_accepts_full_execution_projection() {
            let settlement = settlement_merge_reference_fixture();
            ValidBlock::validate_merge_reference_execution_projection(&settlement)
                .expect("settlement-only reference is admissible at the projection boundary");
            let mut partial = settlement.clone();
            partial.entrypoint_count = Some(1);
            let mut full = settlement.clone();
            full.execution_batch_hash = Some(Hash::new(b"execution-batch"));
            full.entrypoint_count = Some(1);
            full.entrypoint_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(
                b"execution-entrypoints",
            )));
            full.result_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(
                b"execution-results",
            )));
            full.base_state_height = Some(0);
            full.base_state_hash =
                Some(HashOf::from_untyped_unchecked(Hash::new(b"execution-base")));
            assert!(matches!(
                ValidBlock::validate_merge_reference_execution_projection(&partial),
                Err(BlockValidationError::ExecutionContextInvalid(reason))
                    if reason == "certified merge reference has a partial execution-batch binding"
            ));
            ValidBlock::validate_merge_reference_execution_projection(&full)
                .expect("complete execution projection is admissible");
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let mut npos_block =
                npos_effects_block(leader.private_key(), 2, Some(npos_marker_effects(2)));
            assert!(matches!(
                ValidBlock::validate_npos_merge_composition(&npos_block, &full),
                Err(BlockValidationError::ExecutionContextInvalid(reason))
                    if reason.contains("cannot mix NPoS finality effects")
            ));
            ValidBlock::validate_npos_merge_composition(&npos_block, &settlement)
                .expect("control-only merge certificates carry no independent execution write-set");
            npos_block.set_npos_consensus_effects(None);
            ValidBlock::validate_npos_merge_composition(&npos_block, &full)
                .expect("an execution merge is admissible when the carrier has no NPoS effects");
            for invalid_count in [0, MAX_MERGE_EXECUTION_ENTRYPOINTS as u64 + 1] {
                let mut invalid = full.clone();
                invalid.entrypoint_count = Some(invalid_count);
                assert!(matches!(
                    ValidBlock::validate_merge_reference_execution_projection(&invalid),
                    Err(BlockValidationError::ExecutionContextInvalid(reason))
                        if reason.contains("entrypoint count")
                ));
            }
        }
        fn equal_vote_merge_reference_fixture(
            signers: &[iroha_data_model::block::consensus_v2::ValidatorIndex],
        ) -> (
            State,
            SignedBlock,
            BlockExecutionContextBundle,
            ConsensusValidationProfile,
        ) {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), Arc::clone(&kura), query);

            let network_id = state.network_id;
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let parent_hash = HashOf::from_untyped_unchecked(Hash::new(b"equal-vote-merge-parent"));
            let block = SignedBlock::from(ValidBlock::new_dummy_and_modify_header(
                leader.private_key(),
                |header| {
                    header.set_prev_block_hash(Some(parent_hash));
                },
            ));
            let mut validators = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
            validators.sort();
            let roster = validators
                .iter()
                .cloned()
                .map(
                    |validator| iroha_data_model::block::consensus_v2::ValidatorPower {
                        validator,
                        power: 1,
                    },
                )
                .collect::<Vec<_>>();
            let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                    network_id,
                    u64::MAX,
                    &roster,
                );
            let height_context = iroha_data_model::block::consensus_v2::HeightContext {
                network_id,
                protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
                height: block.header().height().get(),
                epoch: 0,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: iroha_data_model::block::consensus_v2::ConsensusMode::Npos,
                parent_commit_qc: None,
                snapshot_bootstrap: Some(
                    iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                        snapshot_height: 1,
                        snapshot_block_hash: parent_hash,
                        snapshot_block_creation_time_ms: 1,
                        snapshot_state_hash: Hash::new(b"equal-vote-merge-snapshot-state"),
                    },
                ),
                quorum: iroha_data_model::block::consensus_v2::DualQuorum::from_roster(&roster)
                    .expect("equal-vote fixture has a canonical quorum"),
                roster,
                kagemusha_mint_finality_authorization,
                kagemusha_mint_finality_authority,
                nexus_amx_context_hash: Hash::new(b"equal-vote-merge-nexus-context"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout {
                    encoding: iroha_data_model::block::consensus_v2::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1_024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4_096,
                    max_chunk_count: 8,
                },
                leader_seed: [0x42; 32],
            };
            height_context
                .validate()
                .expect("equal-vote fixture height context is valid");
            let mut signers_bitmap = vec![0_u8; validators.len().div_ceil(8)];
            for signer in signers {
                signers_bitmap[usize::try_from(*signer).expect("fixture signer fits usize") / 8] |=
                    1_u8 << (*signer % 8);
            }
            let signer_proofs = signers
                .iter()
                .copied()
                .map(|signer| iroha_data_model::merge::MergeSignerProof {
                    signer,
                    proof_of_possession: vec![0xA5; 96],
                })
                .collect();
            let validator_set_hash = HashOf::new(&validators);
            let reference = CertifiedMergeLedgerReference {
                version: 1,
                entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"equal-vote-merge-sidecar")),
                encoded_len: 1,
                // Merge-ledger epochs are independently contiguous and do not
                // reuse the validator-election epoch from the height context.
                epoch_id: 1,
                execution_batch_hash: None,
                entrypoint_count: None,
                entrypoint_merkle_root: None,
                result_merkle_root: None,
                base_state_height: None,
                base_state_hash: None,
                merge_qc: MergeQuorumCertificate {
                    view: block.header().view_change_index(),
                    epoch_id: 1,
                    carrier_height: block.header().height().get(),
                    carrier_parent_hash: parent_hash,
                    network_id,
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash,
                    validator_set: validators,
                    signers_bitmap,
                    signer_proofs,
                    aggregate_signature: vec![0x5A; 96],
                    message_digest: Hash::new(b"equal-vote-merge-qc"),
                },
            };
            let profile = ConsensusValidationProfile::SumeragiV2 {
                block_cadence: Duration::from_millis(1),
                context: SumeragiV2ValidationContext::from_height_context(&height_context),
            };
            (
                state,
                block,
                BlockExecutionContextBundle::new(Vec::new()).with_merge_entry(reference),
                profile,
            )
        }
        include!("block/exact_quorum_cardinality_tests.rs");
        #[test]
        fn merge_reference_accepts_distinct_merge_epoch_with_equal_vote_quorum() {
            let (state, mut block, bundle, profile) =
                equal_vote_merge_reference_fixture(&[0, 1, 3]);
            ValidBlock::validate_execution_context_merge_reference(
                &block,
                state.network_id_ref(),
                &bundle,
                &profile,
            )
            .expect("an independently contiguous merge epoch with three signers satisfies quorum");
            let block_height = block.header().height().get();
            block.set_npos_consensus_effects(Some(npos_marker_effects(block_height)));
            ValidBlock::validate_execution_context_merge_reference(
                &block,
                state.network_id_ref(),
                &bundle,
                &profile,
            )
            .expect("control-only merge certification composes with NPoS finality effects");
        }
        struct AutonomousAnchorFixture {
            state: State,
            topology: Topology,
            block: SignedBlock,
            bundle: BlockExecutionContextBundle,
            profile: ConsensusValidationProfile,
            entrypoint: TransactionEntrypoint,
            validator_keys: Vec<KeyPair>,
        }
        fn autonomous_anchor_fixture(
            lane_incarnation_override: Option<Hash>,
            lane_block_view: u64,
        ) -> AutonomousAnchorFixture {
            autonomous_anchor_fixture_with_gas_limits(
                lane_incarnation_override,
                lane_block_view,
                None,
            )
        }
        fn autonomous_anchor_fixture_with_gas_limits(
            lane_incarnation_override: Option<Hash>,
            lane_block_view: u64,
            gas_limits: Option<&[u64]>,
        ) -> AutonomousAnchorFixture {
            autonomous_anchor_fixture_with_replay_lane(
                lane_incarnation_override,
                lane_block_view,
                gas_limits,
                None,
            )
        }
        fn autonomous_anchor_fixture_with_replay_lane(
            lane_incarnation_override: Option<Hash>,
            lane_block_view: u64,
            gas_limits: Option<&[u64]>,
            replay_lane: Option<LaneId>,
        ) -> AutonomousAnchorFixture {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let validator_keys = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&validator_keys);
            let mut world = World::new();
            for (index, key) in validator_keys.iter().enumerate() {
                let validator_id = insert_consensus_key(
                    &mut world,
                    &format!("autonomous-anchor-validator-{index}"),
                    key,
                    0,
                    None,
                    ConsensusKeyStatus::Active,
                );
                if replay_lane.is_some() {
                    let committee_id = crate::state::derive_committee_key_id(key.public_key());
                    let committee_record = ConsensusKeyRecord {
                        id: committee_id.clone(),
                        public_key: key.public_key().clone(),
                        pop: Some(
                            iroha_crypto::bls_normal_pop_prove(key.private_key())
                                .expect("replay lane committee proof of possession"),
                        ),
                        activation_height: 0,
                        expiry_height: None,
                        replaces: None,
                        status: ConsensusKeyStatus::Active,
                    };
                    world
                        .consensus_keys
                        .insert(committee_id.clone(), committee_record);
                    world.consensus_keys_by_pk.insert(
                        key.public_key().to_string(),
                        vec![validator_id, committee_id],
                    );
                }
            }
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let leader = &validator_keys[0];
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let proposal_height = 2;
            if let Some(lane_id) = replay_lane {
                state
                    .apply_lane_lifecycle(&iroha_data_model::nexus::LaneLifecyclePlan {
                        additions: vec![iroha_data_model::nexus::LaneConfig {
                            id: lane_id,
                            alias: "replay-added-lane".to_owned(),
                            ..iroha_data_model::nexus::LaneConfig::default()
                        }],
                        retire: Vec::new(),
                    })
                    .expect("add the lane through the native runtime lifecycle");
                install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            }
            let epoch = 4;
            let context_id = iroha_data_model::block::consensus_v2::HeightContextId(
                HashOf::from_untyped_unchecked(Hash::new(b"block-autonomous-anchor-context")),
            );
            let profile = ConsensusValidationProfile::SumeragiV2 {
                block_cadence: Duration::from_millis(1),
                context: SumeragiV2ValidationContext {
                    context_id,
                    height: proposal_height,
                    epoch,
                    consensus_mode:
                        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
                    view_zero_leader: 0,
                    snapshot_bootstrap: None,
                    authenticated_height_context: None,
                },
            };
            let profile = if replay_lane.is_some() {
                ConsensusValidationProfile::SumeragiV2 {
                    block_cadence: Duration::from_millis(1),
                    context: SumeragiV2ValidationContext::from_height_context(
                        &authenticated_permissioned_successor_context(&state, &validator_keys),
                    ),
                }
            } else {
                profile
            };
            let context = profile.v2_context().expect("fixture has a height context");
            let epoch = context.epoch;
            let context_id = context.context_id;
            let context_mode_tag = format!(
                "{}::height-context:{}::epoch:{epoch}",
                iroha_data_model::block::consensus_v2::PERMISSIONED_TAG,
                hex::encode(context_id.0.as_ref()),
            );
            let lane_id = replay_lane.unwrap_or(LaneId::SINGLE);
            let dataspace_id = DataSpaceId::UNIVERSAL;
            let lane_incarnation = lane_incarnation_override.unwrap_or_else(|| {
                state
                    .lane_incarnation_at_height(lane_id, proposal_height)
                    .expect("default lane has an active incarnation")
            });
            let (authority, signer) = gen_account_in("autonomous-anchor-control-only");
            let signed = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                "autonomous anchor control-only".to_owned(),
            )])
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            )
            .sign(signer.private_key());
            let entrypoints = gas_limits.map_or_else(
                || vec![TransactionEntrypoint::External(signed.clone())],
                |gas_limits| {
                    gas_limits.iter().enumerate().map(|(index, gas)| {
                        TransactionEntrypoint::External(TransactionBuilder::new_with_time_source(
                            state.network_id,
                            signed.authority().clone(),
                            &time_source,
                            iroha_data_model::transaction::FeePaymentIntent::authority(
                                Vec::new(), core::num::NonZeroU64::new(*gas),
                            ),
                        )
                        .with_executable(Executable::ContractCall(
                            iroha_data_model::transaction::executable::ContractInvocation {
                                contract_address: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
                                    .parse().expect("contract address"),
                                expected_code_hash: Hash::new(b"autonomous-anchor-gas-contract"),
                                entrypoint: format!("configure_{index}"),
                                arguments: None,
                            },
                        ))
                        .with_admission_intent(
                            iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
                        )
                        .sign(signer.private_key()))
                    }).collect::<Vec<_>>()
                },
            );
            let entrypoint = entrypoints[0].clone();
            let mut validator_set = topology.as_ref().to_vec();
            validator_set.sort();
            let validator_count =
                u32::try_from(validator_set.len()).expect("validator count fits u32");
            let min_quorum = u32::try_from(iroha_sumeragi::types::quorum(validator_set.len()))
                .expect("quorum fits u32");
            let mut descriptor = iroha_data_model::block::consensus::LaneBlockDescriptorV1 {
                lane_id,
                dataspace_id,
                lane_incarnation,
                proposal_height,
                previous_lane_block_height: 0,
                previous_lane_block_descriptor_hash: None,
                lane_block_height: 1,
                lane_block_view,
                subject_hash: Hash::new(b"block-autonomous-anchor-subject"),
                payload_ownership_hash: Hash::new(b"block-autonomous-anchor-ownership"),
                rbc_instance_hash: Hash::new(b"block-autonomous-anchor-rbc"),
                accepted_candidate_indices: (0..entrypoints.len())
                    .map(|index| u64::try_from(index).expect("fixture index fits u64"))
                    .collect(),
                accepted_transaction_hashes: entrypoints
                    .iter()
                    .map(|entrypoint| Hash::from(entrypoint.hash()))
                    .collect(),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set: validator_set.clone(),
                validator_count,
                min_quorum,
                qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
                    lane_id,
                    dataspace_id,
                    &context_mode_tag,
                ),
                descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
            };
            descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
            let mut proposal = LaneBlockProposalV1 {
                descriptor,
                proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
                payload_block_hint: None,
            };
            proposal.proposal_hash = proposal.computed_proposal_hash();
            let producer = crate::lane_consensus::deterministic_lane_author(
                &validator_set,
                proposal.descriptor.lane_block_height,
            )
            .cloned()
            .expect("fixture has a deterministic autonomous producer");
            let producer_key = validator_keys
                .iter()
                .find(|key| key.public_key() == producer.public_key())
                .expect("fixture retains the deterministic autonomous producer key");
            let routing_plan = crate::queue::RoutingPlan::single(
                crate::queue::RoutingDecision::new(lane_id, dataspace_id),
            );
            let network_id = state.network_id;
            let (reservation_owner_hash, proposal_identity_hash) =
                crate::sumeragi::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
                    network_id,
                    context_id,
                    epoch,
                    &proposal,
                    &producer,
                )
                .expect("derive canonical autonomous reservation identity");
            let reservation = crate::queue::LaneQueueReservationKeyV1 {
                version: crate::queue::LaneQueueReservationKeyV1::VERSION,
                entrypoint_hash: entrypoint.hash(),
                queue_plan_admission_binding_hash: Hash::new(
                    b"block-native-amx-queue-plan-admission-binding",
                ),
                routing_plan_digest: routing_plan.digest(),
                coordinator_leg: routing_plan.coordinator_leg(),
                lane_id,
                dataspace_id,
                lane_incarnation,
                proposal_height,
                lane_block_height: 1,
                lane_block_view,
                reservation_owner_hash,
                proposal_identity_hash,
            };
            let reservations = entrypoints
                .iter()
                .map(|entrypoint| {
                    let mut reservation = reservation.clone();
                    reservation.entrypoint_hash = entrypoint.hash();
                    reservation
                })
                .collect();
            let entrypoint_count = entrypoints.len();
            let payload =
                crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
                    network_id,
                    epoch,
                    proposal,
                    entrypoints,
                    reservations,
                    vec![routing_plan; entrypoint_count],
                    vec![None; entrypoint_count],
                    producer,
                    producer_key.private_key(),
                )
                .expect("construct valid autonomous anchor payload");
            let envelope = if lane_block_view == 0 {
                crate::lane_consensus::autonomous_lane_payload_envelope(&payload, network_id, epoch)
                    .expect("encode valid autonomous anchor envelope")
            } else {
                let descriptor = &payload.origin_proposal.descriptor;
                AutonomousLanePayloadEnvelopeV1 {
                    version: AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1,
                    network_id,
                    epoch,
                    lane_id: descriptor.lane_id,
                    dataspace_id: descriptor.dataspace_id,
                    lane_incarnation: descriptor.lane_incarnation,
                    proposal_height: descriptor.proposal_height,
                    lane_block_height: descriptor.lane_block_height,
                    lane_block_view: descriptor.lane_block_view,
                    proposal_hash: payload.origin_proposal.proposal_hash,
                    descriptor_hash: descriptor.descriptor_hash,
                    payload_hash: payload.payload_hash,
                    producer: payload.producer.clone(),
                    canonical_payload: norito::to_bytes(&payload)
                        .expect("encode negative non-origin-view payload"),
                }
            };
            let bundle = BlockExecutionContextBundle::new(Vec::new())
                .with_autonomous_lane_payloads(vec![envelope]);
            let block: SignedBlock =
                BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                    .chain(0, state.view().latest_block().as_deref())
                    .with_execution_context(Some(bundle.clone()))
                    .sign(leader.private_key())
                    .unpack(|_| {})
                    .into();
            AutonomousAnchorFixture {
                state,
                topology,
                block,
                bundle,
                profile,
                entrypoint,
                validator_keys,
            }
        }
        fn validate_autonomous_anchor_fixture(
            fixture: &AutonomousAnchorFixture,
            block: &SignedBlock,
            bundle: &BlockExecutionContextBundle,
        ) -> Result<(), BlockValidationError> {
            let view = fixture.state.query_view();
            ValidBlock::validate_execution_context_autonomous_lane_payloads(
                block,
                &fixture.topology,
                &view,
                bundle,
                fixture.profile.clone(),
            )
        }
        #[test]
        fn autonomous_anchor_admission_accepts_exact_control_only_payload() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let view = fixture.state.query_view();
            ValidBlock::validate_execution_context_with_state(
                &fixture.block,
                &fixture.topology,
                &view,
                fixture.profile.clone(),
            )
            .expect("exact control-only autonomous anchor must validate");
        }
        include!("block/replay_proposal_authority_tests.rs");
        include!("block/autonomous_anchor_network_tests.rs");
        include!("block/autonomous_anchor_gas_budget_tests.rs");
        #[test]
        fn autonomous_anchor_admission_uses_lane_slot_author_not_global_leader() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let ConsensusValidationProfile::SumeragiV2 {
                block_cadence,
                mut context,
            } = fixture.profile.clone()
            else {
                panic!("autonomous anchor fixture uses a v2 validation profile");
            };
            let payload = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
                &fixture.bundle.autonomous_lane_payloads[0],
                fixture.state.network_id,
                context.epoch,
            )
            .expect("fixture autonomous payload decodes");
            let global_leader_index = fixture
                .topology
                .as_ref()
                .iter()
                .position(|peer| peer != &payload.producer)
                .expect("four-validator fixture has a global leader distinct from lane author");
            context.view_zero_leader =
                u32::try_from(global_leader_index).expect("fixture global leader index fits u32");
            let profile = ConsensusValidationProfile::SumeragiV2 {
                block_cadence,
                context,
            };
            assert_ne!(
                fixture.topology.as_ref().get(global_leader_index),
                Some(&payload.producer),
                "the regression requires a global leader distinct from the lane-slot author",
            );
            let view = fixture.state.query_view();
            ValidBlock::validate_execution_context_autonomous_lane_payloads(
                &fixture.block,
                &fixture.topology,
                &view,
                &fixture.bundle,
                profile,
            )
            .expect("autonomous payload authority must follow the lane slot, not global view");
        }
        #[test]
        fn autonomous_anchor_admission_accepts_exact_durable_slot_retry() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let epoch = fixture
                .profile
                .v2_context()
                .expect("fixture v2 context")
                .epoch;
            let network_id = fixture.state.network_id;
            let payload = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
                &fixture.bundle.autonomous_lane_payloads[0],
                network_id,
                epoch,
            )
            .expect("fixture autonomous payload decodes");
            fixture
                .state
                .kura()
                .persist_lane_executable_payload(&payload, network_id, epoch)
                .expect("persist exact autonomous slot before retry");
            validate_autonomous_anchor_fixture(&fixture, &fixture.block, &fixture.bundle)
                .expect("an exact retry of the durable autonomous slot must remain admissible");
        }
        #[test]
        fn autonomous_anchor_local_storage_corruption_closes_present_and_later_output_guards() {
            for bind_before_read in [true, false] {
                let fixture = autonomous_anchor_fixture(None, 0);
                validate_autonomous_anchor_fixture(&fixture, &fixture.block, &fixture.bundle)
                    .expect("genuinely absent local certificate slot permits the valid proposal");
                let kura = fixture.state.kura();
                let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
                if bind_before_read {
                    kura.bind_consensus_output_guard(Arc::clone(&guard))
                        .expect("bind the live output guard");
                }
                assert!(!guard.restart_required());
                let envelope = &fixture.bundle.autonomous_lane_payloads[0];
                let entry = fixture
                    .state
                    .lane_storage_identity(envelope.lane_id)
                    .expect("active lane");
                let artifacts = entry.blocks_dir(kura.store_root()).join("lane_artifacts");
                std::fs::create_dir_all(&artifacts)
                    .expect("create owned fixture artifact directory");
                let data = artifacts.join("certified_blocks.norito");
                let index = artifacts.join("certified_blocks.index");
                assert!(
                    !data.exists() && !index.exists(),
                    "fixture begins with genuine absence"
                );
                let damaged = b"occupied certificate data without its required index";
                std::fs::write(&data, damaged).expect("introduce an actual local pair fault");
                let error =
                    validate_autonomous_anchor_fixture(&fixture, &fixture.block, &fixture.bundle)
                        .expect_err("local storage corruption cannot be treated as an absent slot");
                assert!(
                    matches!(error, BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("local certified slot is unreadable"))
                );
                assert_eq!(std::fs::read(&data).unwrap(), damaged);
                assert!(
                    !index.exists(),
                    "validation must not repair the damaged pair"
                );
                if !bind_before_read {
                    kura.bind_consensus_output_guard(Arc::clone(&guard))
                        .expect("the late binding observes the already published fault");
                }
                assert!(guard.restart_required());
                assert!(
                    guard.acquire().is_none(),
                    "the local fault closes consensus output"
                );
            }
        }
        #[test]
        fn autonomous_anchor_requires_sumeragi_v2_context() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let view = fixture.state.query_view();
            let error = ValidBlock::validate_execution_context_autonomous_lane_payloads(
                &fixture.block,
                &fixture.topology,
                &view,
                &fixture.bundle,
                ConsensusValidationProfile::SumeragiGenesis {
                    consensus_mode:
                        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
                },
            )
            .expect_err("autonomous admission must require an explicit Sumeragi-v2 context");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("require a Sumeragi v2 height context")
            ));
        }
        #[test]
        fn autonomous_anchor_predecessor_accepts_exact_hash_only_snapshot_artifact() {
            let (state, kura, topology, time_source, keys) = lane_payload_context_fixture();
            let mut snapshot_context = authenticated_permissioned_successor_context(&state, &keys);
            let leader = &keys[0];
            let mut predecessor = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "autonomous-anchor-snapshot-predecessor",
                1,
                None,
            );
            let predecessor_entrypoint_hashes = predecessor
                .external_entrypoints_cloned()
                .map(|entrypoint| entrypoint.hash())
                .collect::<Vec<_>>();
            let axt_snapshot = state.block(predecessor.header()).axt_policy_snapshot();
            predecessor
                .set_execution_outputs(
                    crate::execution_output_test_support::structural_network_outputs(
                        &predecessor,
                        &predecessor_entrypoint_hashes,
                        vec![
                            iroha_data_model::transaction::signed::TransactionResultInner::Ok(
                                DataTriggerSequence::default(),
                            ),
                        ],
                    ),
                    u64::try_from(predecessor.network_entrypoint_count())
                        .expect("fixture input count fits u64"),
                    BTreeMap::new(),
                    Vec::new(),
                    axt_snapshot,
                    BTreeSet::new(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("attach the canonical predecessor result and AXT policy snapshot");
            let predecessor_descriptor_hash = predecessor
                .execution_context()
                .and_then(|bundle| bundle.lane_payload_ownerships.first())
                .and_then(|ownership| ownership.lane_block_descriptor_hash)
                .expect("snapshot predecessor descriptor hash");
            let committed = ValidBlock::new_unverified_for_tests(predecessor.clone())
                .commit_unchecked()
                .unpack(|_| {});
            {
                let mut state_block = state.block(committed.as_ref().header());
                let _ = state_block.apply_without_execution(&committed, topology.as_ref().to_vec());
                state_block
                    .commit()
                    .expect("commit snapshot predecessor state");
            }
            kura.store_block(Arc::new(predecessor))
                .expect("store snapshot predecessor block");
            let successor = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "autonomous-anchor-snapshot-successor",
                2,
                Some(predecessor_descriptor_hash),
            );
            let snapshot_parent = state.view().latest_block().expect("snapshot parent body");
            snapshot_context.height = 3;
            snapshot_context.parent_commit_qc = None;
            snapshot_context.snapshot_bootstrap = Some(
                iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                    snapshot_height: 2,
                    snapshot_block_hash: snapshot_parent.hash(),
                    snapshot_block_creation_time_ms: snapshot_parent.header().creation_time_ms,
                    snapshot_state_hash: crate::snapshot::canonical_state_snapshot_hash(&state)
                        .expect("stable valid fixture snapshot"),
                },
            );
            snapshot_context.nexus_amx_context_hash =
                crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&state)
                    .expect("valid committed catalog");
            snapshot_context.execution_policy_hash =
                crate::sumeragi::v2_recovery::committed_execution_policy_hash(&state)
                    .expect("snapshot execution policy");
            let snapshot_record =
                iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord {
                    version:
                        iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord::VERSION,
                    context: snapshot_context,
                    validator_set_pops: keys
                        .iter()
                        .map(|key| {
                            iroha_crypto::bls_normal_pop_prove(key.private_key())
                                .expect("snapshot validator proof of possession")
                        })
                        .collect(),
                };
            snapshot_record
                .validate()
                .expect("canonical snapshot bootstrap record");
            let snapshot_payload =
                crate::snapshot::AuthenticatedSnapshotBootstrapPayload::for_testing(
                    snapshot_record,
                    state.committed_block_hashes_snapshot(),
                );
            kura.install_authenticated_snapshot_prefix_for_testing(&snapshot_payload)
                .expect("install the exact authenticated snapshot prefix");
            assert!(kura.is_audited_snapshot_import_height(nonzero!(2_usize)));
            assert!(kura.read_block_body(nonzero!(2_usize)).unwrap().is_none());
            let successor_ownership = successor
                .execution_context()
                .and_then(|bundle| bundle.lane_payload_ownerships.first())
                .expect("snapshot successor ownership");
            let successor_proposal =
                native_amx_coordinator_proposal_from_ownership(successor_ownership)
                    .expect("snapshot successor proposal reconstructs");
            let view = state.query_view();
            assert!(
                NativeAmxAuthorityContext::native_amx_participant_predecessor_is_current(
                    &view,
                    &successor_proposal,
                    None,
                )
                .expect("a first Native control can follow an authenticated ordinary snapshot tip")
            );
            assert!(
                !NativeAmxAuthorityContext::native_amx_participant_predecessor_is_current(
                    &view,
                    &successor_proposal,
                    Some(HashOf::from_untyped_unchecked(Hash::new(
                        b"unapplied Native predecessor"
                    ))),
                )
                .expect("a foreign signed Native link is ineligible, not storage corruption")
            );
            let mut competing = successor_proposal.clone();
            competing.descriptor.previous_lane_block_descriptor_hash =
                Some(Hash::new(b"valid competing Native predecessor"));
            competing.descriptor.descriptor_hash = competing.descriptor.computed_descriptor_hash();
            competing.proposal_hash = competing.computed_proposal_hash();
            crate::lane_consensus::validate_lane_block_proposal(&competing)
                .expect("structurally valid competing candidate");
            assert!(
                !State::lane_block_predecessor_is_applied_for_snapshot(
                    &view,
                    &competing,
                    crate::state::LanePredecessorApplicationMode::AppliedStatePrefix
                )
                .expect("valid competing predecessor is ordinary ineligibility")
            );
            assert!(
                !NativeAmxAuthorityContext::native_amx_participant_predecessor_is_current(
                    &view, &competing, None
                )
                .expect("authority wrapper preserves ordinary ineligibility")
            );
            assert!(
                ValidBlock::autonomous_lane_predecessor_is_current_or_snapshot_anchored(
                    &view,
                    &successor_proposal,
                )
                .expect("authenticate snapshot predecessor"),
                "the exact canonical hash-only predecessor must remain admissible"
            );
            let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            kura.bind_consensus_output_guard(Arc::clone(&guard))
                .expect("bind the real output admission guard");
            let directory = state
                .lane_storage_identity(successor_proposal.descriptor.lane_id)
                .expect("actual snapshot lane")
                .blocks_dir(kura.store_root())
                .join("lane_artifacts");
            let data = directory.join("ownerships.norito");
            let index = directory.join("ownerships.index");
            let before = std::fs::read(&data).expect("actual occupied snapshot ownership file");
            let index_before = std::fs::read(&index).expect("actual snapshot ownership index");
            assert!(!before.is_empty());
            let damaged = vec![0xA5; before.len()];
            std::fs::write(&data, &damaged)
                .expect("damage the actual indexed ownership without changing length");
            assert!(
                State::lane_block_predecessor_is_applied_for_snapshot(
                    &view,
                    &successor_proposal,
                    crate::state::LanePredecessorApplicationMode::AppliedStatePrefix,
                )
                .is_err()
            );
            assert!(
                !guard.restart_required(),
                "the typed State read preserves classification for its consensus caller"
            );
            let error = ValidBlock::autonomous_lane_predecessor_is_current_or_snapshot_anchored(
                &view,
                &successor_proposal,
            )
            .expect_err("local corruption must terminate before hash-only snapshot fallback");
            assert!(
                matches!(error, BlockValidationError::ExecutionContextInvalid(message)
                if message.contains("local Native AMX predecessor is unreadable"))
            );
            assert!(guard.restart_required());
            assert!(guard.acquire().is_none());
            assert!(
                NativeAmxAuthorityContext::native_amx_participant_predecessor_is_current(
                    &view,
                    &successor_proposal,
                    None
                )
                .is_err()
            );
            assert_eq!(std::fs::read(data).unwrap(), damaged);
            assert_eq!(std::fs::read(index).unwrap(), index_before);
        }
        #[test]
        fn autonomous_anchor_admission_rejects_legacy_unknown_tampered_and_duplicate_envelopes() {
            for version in [0, AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1 + 1] {
                let fixture = autonomous_anchor_fixture(None, 0);
                let mut bundle = fixture.bundle.clone();
                bundle.autonomous_lane_payloads[0].version = version;
                let error = validate_autonomous_anchor_fixture(&fixture, &fixture.block, &bundle)
                    .expect_err("unsupported envelope version must fail closed");
                assert!(matches!(
                    error,
                    BlockValidationError::ExecutionContextInvalid(message)
                        if message.contains("unsupported autonomous lane artifact version")
                ));
            }
            let fixture = autonomous_anchor_fixture(None, 0);
            let mut tampered = fixture.bundle.clone();
            tampered.autonomous_lane_payloads[0].payload_hash =
                Hash::new(b"tampered autonomous payload identity");
            let error = validate_autonomous_anchor_fixture(&fixture, &fixture.block, &tampered)
                .expect_err("tampered envelope identity must fail closed");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("envelope identity mismatch")
            ));
            let mut duplicated = fixture.bundle.clone();
            duplicated
                .autonomous_lane_payloads
                .push(duplicated.autonomous_lane_payloads[0].clone());
            let error = validate_autonomous_anchor_fixture(&fixture, &fixture.block, &duplicated)
                .expect_err("duplicate autonomous anchor must fail closed");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("strict canonical")
            ));
        }
        #[test]
        fn autonomous_anchor_admission_rejects_cross_kind_route_alias() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let epoch = fixture
                .profile
                .v2_context()
                .expect("fixture v2 context")
                .epoch;
            let payload = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
                &fixture.bundle.autonomous_lane_payloads[0],
                fixture.state.network_id,
                epoch,
            )
            .expect("fixture autonomous payload decodes");
            let descriptor = &payload.origin_proposal.descriptor;
            let ordinary = sample_lane_payload_ownership_for_context_at_slot(
                descriptor.proposal_height,
                fixture.block.header().view_change_index(),
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor
                    .lane_block_height
                    .checked_add(1)
                    .expect("fixture lane height has a successor"),
                0,
                vec![0],
                vec![Hash::new(b"cross-kind ordinary entrypoint")],
                &descriptor.validator_set,
            );
            let mut bundle = fixture.bundle.clone();
            bundle.lane_payload_ownerships.push(ordinary);
            let error = validate_autonomous_anchor_fixture(&fixture, &fixture.block, &bundle)
                .expect_err(
                    "ordinary and autonomous anchors must not share one lane route even at different slots",
                );
            assert!(
                matches!(
                    &error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("duplicates a route, slot, proposal")
                ),
                "unexpected cross-kind route-alias error: {error:?}"
            );
        }
        #[test]
        fn autonomous_anchor_admission_rejects_oversized_and_stale_artifacts() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let mut oversized = fixture.bundle.clone();
            oversized.autonomous_lane_payloads[0].canonical_payload =
                vec![0; iroha_data_model::merge::MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES + 1];
            let error = validate_autonomous_anchor_fixture(&fixture, &fixture.block, &oversized)
                .expect_err("oversized autonomous payload must fail closed");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("envelope byte limit exceeded")
            ));
            let stale = autonomous_anchor_fixture(Some(Hash::new(b"retired lane incarnation")), 0);
            let error = validate_autonomous_anchor_fixture(&stale, &stale.block, &stale.bundle)
                .expect_err("stale lane incarnation must fail closed");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("active proposal-height lane incarnation")
            ));
            let non_origin_view = autonomous_anchor_fixture(None, 1);
            let error = validate_autonomous_anchor_fixture(
                &non_origin_view,
                &non_origin_view.block,
                &non_origin_view.bundle,
            )
            .expect_err("non-zero origin lane view must fail closed");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("global anchor hint is invalid")
            ));
        }
        #[test]
        fn autonomous_anchor_admission_enforces_count_aggregate_and_control_only_bounds() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let envelope = fixture.bundle.autonomous_lane_payloads[0].clone();
            let too_many = vec![envelope.clone(); MAX_MERGE_EXECUTION_ENTRYPOINTS + 1];
            let error = ValidBlock::validate_autonomous_lane_payload_envelope_budget(&too_many)
                .expect_err("envelope count beyond the hard bound must fail closed");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("envelope count")
            ));
            let mut aggregate_member = envelope;
            aggregate_member.canonical_payload =
                vec![0; iroha_data_model::merge::MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES];
            let aggregate_oversized = vec![aggregate_member; 4];
            let error =
                ValidBlock::validate_autonomous_lane_payload_envelope_budget(&aggregate_oversized)
                    .expect_err("aggregate autonomous envelope bytes must be bounded");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("aggregate byte limit")
            ));
            let mut carrier = fixture.block.clone();
            carrier.set_external_entrypoints(vec![fixture.entrypoint.clone()]);
            let error = validate_autonomous_anchor_fixture(&fixture, &carrier, &fixture.bundle)
                .expect_err("carrier execution must not repeat an autonomous entrypoint");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(message)
                    if message.contains("anchors must be control-only")
            ));
        }
        #[test]
        fn autonomous_anchor_budget_is_ambient_layout_invariant() {
            let fixture = autonomous_anchor_fixture(None, 0);
            let mut exact_limit = fixture.bundle.autonomous_lane_payloads[0].clone();
            exact_limit
                .canonical_payload
                .resize(MAX_MERGE_EXECUTION_BATCH_BYTES, 0);
            let initial_len = norito::encode_canonical(&exact_limit)
                .expect("encode exact-limit autonomous envelope")
                .len();
            let envelope_overhead = initial_len
                .checked_sub(exact_limit.canonical_payload.len())
                .expect("framed envelope contains its canonical payload");
            exact_limit.canonical_payload.resize(
                MAX_MERGE_EXECUTION_BATCH_BYTES
                    .checked_sub(envelope_overhead)
                    .expect("autonomous envelope overhead fits the aggregate budget"),
                0,
            );
            assert_eq!(
                norito::encode_canonical(&exact_limit)
                    .expect("re-encode exact-limit autonomous envelope")
                    .len(),
                MAX_MERGE_EXECUTION_BATCH_BYTES
            );
            let baseline = ValidBlock::validate_autonomous_lane_payload_envelope_budget(
                core::slice::from_ref(&exact_limit),
            );
            assert_eq!(baseline, Ok(()));
            let alternate = {
                let alternate_flags =
                    norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
                let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
                ValidBlock::validate_autonomous_lane_payload_envelope_budget(core::slice::from_ref(
                    &exact_limit,
                ))
            };
            assert_eq!(
                alternate, baseline,
                "block admission must account exact canonical envelope bytes"
            );
        }
        fn state_confidential_features_at_height(
            state: &State,
            height: u64,
        ) -> Option<ConfidentialFeatureDigest> {
            let view = state.query_view();
            let digest = compute_confidential_feature_digest(view.world(), view.zk(), height);
            (!digest.is_empty()).then_some(digest)
        }
        fn with_current_state_da_sidecars(
            mut builder: BlockBuilder<Chained>,
            state: &State,
        ) -> BlockBuilder<Chained> {
            let height = builder.0.header.height().get();
            if builder.0.execution_context.is_none() && !builder.0.transactions.is_empty() {
                let validators = state
                    .resolve_lane_committee_at_height(
                        crate::state::LaneAuthorityRoute::new(
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        ),
                        height,
                    )
                    .expect("validation fixture default lane has exact committee authority")
                    .into_validators();
                let hashes = builder
                    .0
                    .transactions
                    .iter()
                    .map(AcceptedTransaction::hash_as_entrypoint)
                    .collect::<Vec<_>>();
                let external = hashes
                    .iter()
                    .map(|hash| {
                        ExternalExecutionContext::new(*hash, LaneId::SINGLE, DataSpaceId::UNIVERSAL)
                    })
                    .collect();
                let ownership = sample_lane_payload_ownership_for_context_at_slot(
                    height,
                    builder.0.header.view_change_index(),
                    LaneId::SINGLE,
                    DataSpaceId::UNIVERSAL,
                    state
                        .lane_incarnation_at_height(LaneId::SINGLE, height)
                        .expect("fixture lane is active"),
                    1,
                    0,
                    (0..hashes.len())
                        .map(|i| u64::try_from(i).expect("fixture index fits u64"))
                        .collect(),
                    hashes.into_iter().map(Hash::from).collect(),
                    &validators,
                );
                builder = builder.with_execution_context(Some(
                    BlockExecutionContextBundle::new(external)
                        .with_lane_payload_ownerships(vec![ownership]),
                ));
            }
            let nexus = state.nexus_snapshot();
            let proof_policies = crate::da::active_proof_policy_bundle_at_height(&nexus, height);
            builder
                .with_da_proof_policies(Some(proof_policies))
                .with_confidential_features(state_confidential_features_at_height(state, height))
        }
        fn with_current_state_confidential_features(
            mut block: SignedBlock,
            state: &State,
            signers: &[(u64, &PrivateKey)],
        ) -> SignedBlock {
            let mut header = block.header();
            header.set_confidential_features(state_confidential_features_at_height(
                state,
                header.height().get(),
            ));
            block.replace_header_for_testing(header);
            let block_hash = block.hash();
            let signatures = signers
                .iter()
                .map(|(index, private_key)| {
                    BlockSignature::new(*index, checked_block_signature(private_key, block_hash))
                })
                .collect();
            block
                .replace_signatures(signatures)
                .expect("replace signatures after refreshing confidential test sidecar");
            block
        }
        fn install_test_lane_manifests_for_keypairs(state: &State, keypairs: &[KeyPair]) {
            let validators = keypairs
                .iter()
                .map(|keypair| AccountId::new(keypair.public_key().clone()))
                .collect::<Vec<_>>();
            let validator_bindings = validators
                .iter()
                .map(
                    |validator| crate::governance::manifest::ManifestValidatorBinding {
                        validator: validator.clone(),
                        peer_id: PeerId::from(
                            validator
                                .try_signatory()
                                .expect("manifest test validators must be single-signatory")
                                .clone(),
                        ),
                        torii_url: None,
                    },
                )
                .collect::<Vec<_>>();
            let nexus = state.nexus_snapshot();
            let bindings_by_lane = nexus
                .lane_catalog
                .lanes()
                .iter()
                .map(|lane| (lane.id, validator_bindings.clone()))
                .collect();
            state.install_lane_manifests_for_testing(&Arc::new(
                crate::governance::manifest::test_support::validator_registry(
                    &nexus.lane_catalog,
                    &nexus.governance,
                    bindings_by_lane,
                ),
            ));
        }
        fn static_test_lane_incarnation(catalog: &LaneCatalog, lane_id: LaneId) -> Hash {
            crate::state::derive_static_lane_incarnations(catalog)
                .get(&lane_id)
                .copied()
                .expect("test lane exists in catalog")
        }
        fn insert_consensus_key(
            world: &mut World,
            name: &str,
            keypair: &KeyPair,
            activation_height: u64,
            expiry_height: Option<u64>,
            status: ConsensusKeyStatus,
        ) -> ConsensusKeyId {
            let peer_id = PeerId::from(keypair.public_key().clone());
            let mut peers = world.peers.block();
            let _ = peers.get_mut().push(peer_id);
            peers.commit();
            let id = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                Ident::from_str(name).expect("consensus key name parses"),
            );
            let pop = match keypair
                .public_key()
                .try_algorithm()
                .expect("fixture public key must be valid")
            {
                Algorithm::BlsNormal => Some(
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("pop for consensus key"),
                ),
                Algorithm::BlsSmall => Some(
                    iroha_crypto::bls_small_pop_prove(keypair.private_key())
                        .expect("pop for consensus key"),
                ),
                _ => None,
            };
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: keypair.public_key().clone(),
                pop,
                activation_height,
                expiry_height,
                replaces: None,
                status,
            };
            world.consensus_keys.insert(id.clone(), record.clone());
            let pk_label = record.public_key.to_string();
            world
                .consensus_keys_by_pk
                .insert(pk_label, vec![id.clone()]);
            id
        }
        fn insert_active_consensus_keys(world: &mut World, keypairs: &[KeyPair]) {
            for (index, keypair) in keypairs.iter().enumerate() {
                insert_consensus_key(
                    world,
                    &format!("validator-{index}"),
                    keypair,
                    0,
                    None,
                    ConsensusKeyStatus::Active,
                );
            }
        }
        fn insert_active_participant_keys(world: &mut World, keypairs: &[KeyPair]) {
            for keypair in keypairs {
                let id = crate::state::derive_committee_key_id(keypair.public_key());
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: keypair.public_key().clone(),
                    pop: Some(
                        iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                            .expect("participant committee proof of possession"),
                    ),
                    activation_height: 0,
                    expiry_height: None,
                    replaces: None,
                    status: ConsensusKeyStatus::Active,
                };
                world.consensus_keys.insert(id.clone(), record.clone());
                let pk = record.public_key.to_string();
                let mut by_pk = world
                    .consensus_keys_by_pk
                    .view()
                    .get(&pk)
                    .cloned()
                    .unwrap_or_default();
                if !by_pk.contains(&id) {
                    by_pk.push(id);
                    world.consensus_keys_by_pk.insert(pk, by_pk);
                }
            }
        }
        #[cfg(feature = "bls")]
        #[test]
        fn bls_normal_public_key_check_uses_checked_algorithm_access() {
            let bls_key = checked_seeded_keypair(b"checked-bls-key", Algorithm::BlsNormal);
            let ed25519_key = checked_seeded_keypair(b"checked-ed25519-key", Algorithm::Ed25519);
            assert!(ValidBlock::is_bls_normal_public_key(bls_key.public_key()));
            assert!(!ValidBlock::is_bls_normal_public_key(
                ed25519_key.public_key()
            ));
        }
        include!("block/soracloud_validation_tests.rs");
        fn commit_block_at_height(
            state: &State,
            kura: &Kura,
            topology: &Topology,
            leader_private: &PrivateKey,
            height: u64,
            prev_hash: Option<HashOf<BlockHeader>>,
            creation_time_ms: u64,
        ) -> HashOf<BlockHeader> {
            let valid = ValidBlock::new_dummy_and_modify_header(leader_private, |header| {
                header
                    .set_height(NonZeroU64::new(height).expect("non-zero height in commit helper"));
                header.set_prev_block_hash(prev_hash);
                header.creation_time_ms = creation_time_ms;
            });
            let committed = commit_result_bearing_synthetic_parent(valid, state, leader_private);
            {
                let mut state_block = state.block(committed.as_ref().header());
                let _ =
                    state_block.apply_without_execution(&committed, topology.as_ref().to_owned());
                state_block.commit().unwrap();
            }
            kura.store_block(committed.clone())
                .expect("store committed block");
            committed.as_ref().hash()
        }
        fn applied_lane_predecessor_finality(
            block: &SignedBlock,
            state: &State,
            validator_keys: &[KeyPair],
        ) -> iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact {
            use iroha_data_model::block::consensus_v2::{
                self as wire, finality::V2FinalityArtifact,
            };

            assert_eq!(validator_keys.len(), 4);
            assert!(
                validator_keys.windows(2).all(|pair| {
                    PeerId::new(pair[0].public_key().clone())
                        < PeerId::new(pair[1].public_key().clone())
                }),
                "predecessor keys must already match the canonical topology and signature slots"
            );
            let roster = validator_keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                    state.network_id,
                    u64::MAX,
                    &roster,
                );
            let context = if block.header().height().get() == 1 {
                assert!(block.header().prev_block_hash().is_none());
                let parameters = wire::SumeragiV2GenesisContextParameters::recommended();
                wire::HeightContext {
                    network_id: state.network_id,
                    protocol_version: wire::PROTOCOL_VERSION,
                    height: 1,
                    epoch: 0,
                    epoch_end_height: u64::MAX,
                    next_epoch_snapshot: None,
                    mode: wire::ConsensusMode::Permissioned,
                    parent_commit_qc: None,
                    snapshot_bootstrap: None,
                    quorum: wire::DualQuorum::from_roster(&roster)
                        .expect("exact four-validator quorum"),
                    roster,
                    kagemusha_mint_finality_authorization,
                    kagemusha_mint_finality_authority,
                    nexus_amx_context_hash:
                        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(state)
                            .expect("valid committed catalog"),
                    execution_policy_hash: Hash::prehashed(parameters.execution_policy_hash),
                    da_layout: parameters.da_layout,
                    leader_seed: [0x41; 32],
                }
            } else {
                let parent = state
                    .kura()
                    .v2_finality_artifact(block.header().height().get() - 1)
                    .expect("read authenticated predecessor finality")
                    .expect("every predecessor height retains exact finality");
                assert_eq!(block.header().prev_block_hash(), Some(parent.block_hash));
                let context =
                    crate::sumeragi::v2_context::build_successor_height_context_from_state(
                        &parent,
                        &state.view(),
                        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(state)
                            .expect("valid committed catalog"),
                    )
                    .expect("derive predecessor context from its exact parent authority");
                assert_eq!(context.roster, roster);
                context
            };
            context
                .validate()
                .expect("canonical predecessor height context");
            let subject = wire::BlockSubject {
                parent_block_hash: block.header().prev_block_hash(),
                block_hash: block.hash(),
                payload_hash: block
                    .canonical_proposal_wire_hash()
                    .expect("canonical predecessor proposal"),
            };
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: block.header().view_change_index(),
            };
            let bytes = block
                .encode_wire()
                .expect("canonical executed predecessor bytes");
            let execution = wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                Hash::new(b"applied lane predecessor parent state"),
                Hash::new(b"applied lane predecessor post state"),
                Hash::new(b"applied lane predecessor ordinary writes"),
                u64::try_from(bytes.len()).expect("predecessor wire length fits u64"),
                Hash::new(&bytes),
            );
            let vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: execution,
                signer: 0,
                signature: Vec::new(),
            };
            let preimage = vote.signature_preimage();
            let shares = validator_keys[..3]
                .iter()
                .map(|key| {
                    iroha_crypto::Signature::try_new(key.private_key(), &preimage)
                        .expect("sign predecessor Commit vote")
                        .payload()
                        .to_vec()
                })
                .collect::<Vec<_>>();
            let aggregate = iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate exact quorum predecessor Commit votes");
            let qc = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: execution,
                signers: vec![0, 1, 2],
                aggregate_signature: aggregate,
            };
            let pops = validator_keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("derive predecessor validator PoP")
                })
                .collect();
            let artifact = V2FinalityArtifact::new(context, subject, qc, pops);
            artifact
                .verify()
                .expect("verify actual predecessor finality");
            let mut forged = artifact.clone();
            forged.commit_qc.aggregate_signature[0] ^= 0x80;
            assert!(
                crate::block::VerifiedV2FinalityArtifact::verify(forged).is_err(),
                "the predecessor fixture must preserve invalid-proof rejection"
            );
            artifact
        }
        fn commit_block_with_applied_lane_predecessors(
            state: &State,
            kura: &Kura,
            topology: &Topology,
            leader_private: &PrivateKey,
            validator_keys: &[KeyPair],
            lanes: &[(LaneId, DataSpaceId)],
        ) -> HashOf<BlockHeader> {
            assert_eq!(
                topology.as_ref(),
                validator_keys
                    .iter()
                    .map(|key| PeerId::new(key.public_key().clone()))
                    .collect::<Vec<_>>()
                    .as_slice(),
                "predecessor topology must match the exact finality roster"
            );
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let mut transactions = Vec::with_capacity(lanes.len());
            for index in 0..lanes.len() {
                let (authority, signer) = gen_account_in(&format!("lane-predecessor-{index}"));
                transactions.push(
                    TransactionBuilder::new_with_time_source(
                        state.network_id,
                        authority,
                        &time_source,
                        iroha_data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                    .with_instructions([Log::new(Level::INFO, format!("lane-predecessor-{index}"))])
                    .sign(signer.private_key()),
                );
                time_handle.advance(Duration::from_millis(1));
            }
            let accepted = transactions
                .iter()
                .cloned()
                .map(|tx| AcceptedTransaction::new_unchecked(Cow::Owned(tx)))
                .collect::<Vec<_>>();
            let ownerships = lanes
                .iter()
                .zip(&transactions)
                .enumerate()
                .map(|(index, (&(lane_id, dataspace_id), transaction))| {
                    sample_lane_payload_ownership_for_context_at_slot(
                        1,
                        0,
                        lane_id,
                        dataspace_id,
                        state
                            .lane_incarnation_at_height(lane_id, 1)
                            .expect("predecessor lane incarnation at genesis height"),
                        1,
                        0,
                        vec![u64::try_from(index).expect("predecessor index fits u64")],
                        vec![Hash::from(transaction.hash_as_entrypoint())],
                        topology.as_ref(),
                    )
                })
                .collect::<Vec<_>>();
            let proposals = ownerships
                .iter()
                .map(|ownership| {
                    native_amx_coordinator_proposal_from_ownership(ownership)
                        .expect("applied predecessor proposal reconstructs")
                })
                .collect::<Vec<_>>();
            let external = transactions
                .iter()
                .zip(lanes)
                .map(|(transaction, &(lane_id, dataspace_id))| {
                    ExternalExecutionContext::new(
                        transaction.hash_as_entrypoint(),
                        lane_id,
                        dataspace_id,
                    )
                })
                .collect();
            let execution_context =
                BlockExecutionContextBundle::new(external).with_lane_payload_ownerships(ownerships);
            let mut signed: SignedBlock = with_current_state_da_sidecars(
                BlockBuilder::new_with_time_source(accepted, time_source)
                    .chain(0, None)
                    .with_execution_context(Some(execution_context)),
                state,
            )
            .sign(leader_private)
            .unpack(|_| {})
            .into();
            let entrypoint_hashes = signed
                .external_entrypoints_cloned()
                .map(|entrypoint| entrypoint.hash())
                .collect::<Vec<_>>();
            let policy_snapshot = state.block(signed.header()).axt_policy_snapshot();
            signed
                .set_execution_outputs(
                    crate::execution_output_test_support::structural_network_outputs(
                        &signed,
                        &entrypoint_hashes,
                        vec![Ok(DataTriggerSequence::default()); lanes.len()],
                    ),
                    u64::try_from(signed.network_entrypoint_count())
                        .expect("fixture input count fits u64"),
                    BTreeMap::new(),
                    Vec::new(),
                    policy_snapshot,
                    BTreeSet::new(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("attach canonical predecessor results and policy snapshot");
            signed
                .replace_signatures(
                    [BlockSignature::new(
                        0,
                        checked_block_signature(leader_private, signed.hash()),
                    )]
                    .into_iter()
                    .collect(),
                )
                .expect("resign predecessor after attaching final results");
            let artifact = applied_lane_predecessor_finality(&signed, state, validator_keys);
            let verified = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())
                .expect("verify exact applied predecessor authority");
            let committed = ValidBlock::new_unverified_for_tests(signed.clone())
                .commit_with_verified_v2_artifact(verified, artifact.commit_qc.execution_commitment)
                .unpack(|_| {})
                .expect("commit predecessor with exact finality authority");
            kura.store_block(Arc::new(signed))
                .expect("store applied lane predecessor block");
            let _ = kura
                .store_v2_finality_artifact(&artifact)
                .expect("persist actual predecessor finality before application receipts");
            {
                let mut state_block = state.block(committed.as_ref().header());
                // This structural predecessor fixture supplies results directly;
                // use the explicit metadata fixture path with its verified roster.
                state_block
                    .stage_ordinary_lane_frontiers(committed.as_ref())
                    .expect("stage the exact predecessor execution frontier");
                let _ = state_block.apply_without_execution(
                    &committed,
                    artifact
                        .height_context
                        .roster
                        .iter()
                        .map(|entry| entry.validator.clone())
                        .collect(),
                );
                state_block.commit().unwrap();
            }
            for proposal in proposals {
                kura.persist_lane_block_application_receipt(&proposal)
                    .expect("persist applied lane predecessor receipt");
            }
            committed.as_ref().hash()
        }
        fn bind_applied_lane_predecessor(
            kura: &Kura,
            ownership: &mut SumeragiLanePayloadOwnership,
        ) {
            let previous_height = ownership
                .lane_block_height
                .checked_sub(1)
                .expect("candidate lane block height is non-zero");
            let receipt = kura
                .read_lane_block_application_receipt(ownership.lane_id, previous_height)
                .expect("applied canonical lane predecessor receipt");
            ownership.previous_lane_block_descriptor_hash =
                Some(receipt.proposal.descriptor.descriptor_hash);
            let replay_hashes = ownership
                .compute_replay_hashes()
                .expect("candidate ownership replay hashes recompute");
            ownership.subject_hash = replay_hashes.subject_hash;
            ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
            ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
            ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
        }
        fn sample_lane_payload_ownership_for_context(
            proposal_height: u64,
            proposal_view: u64,
            lane_id: LaneId,
            dataspace_id: DataSpaceId,
            lane_incarnation: Hash,
            accepted_candidate_indices: Vec<u64>,
            candidate_hashes: Vec<Hash>,
            validator_set: &[PeerId],
        ) -> SumeragiLanePayloadOwnership {
            sample_lane_payload_ownership_for_context_at_slot(
                proposal_height,
                proposal_view,
                lane_id,
                dataspace_id,
                lane_incarnation,
                proposal_height,
                proposal_view,
                accepted_candidate_indices,
                candidate_hashes,
                validator_set,
            )
        }
        fn sample_lane_payload_ownership_for_context_at_slot(
            proposal_height: u64,
            proposal_view: u64,
            lane_id: LaneId,
            dataspace_id: DataSpaceId,
            lane_incarnation: Hash,
            lane_block_height: u64,
            lane_block_view: u64,
            accepted_candidate_indices: Vec<u64>,
            candidate_hashes: Vec<Hash>,
            validator_set: &[PeerId],
        ) -> SumeragiLanePayloadOwnership {
            let qc_mode_tag =
                LaneRelayEnvelope::lane_qc_mode_tag_for(lane_id, dataspace_id, "block-test");
            let mut descriptor_validator_set = validator_set.to_vec();
            descriptor_validator_set.sort();
            descriptor_validator_set.dedup();
            let validator_count = u32::try_from(descriptor_validator_set.len())
                .expect("test validator count fits u32");
            let min_quorum = u32::try_from(iroha_sumeragi::types::quorum(
                descriptor_validator_set.len(),
            ))
            .expect("test quorum fits u32");
            let previous_lane_block_height = lane_block_height
                .checked_sub(1)
                .expect("test lane block height is non-zero");
            let mut ownership = SumeragiLanePayloadOwnership {
                proposal_height,
                proposal_view,
                lane_id,
                dataspace_id,
                lane_incarnation,
                lane_block_height,
                lane_block_view,
                subject_hash: Hash::new(b"block-test lane subject placeholder"),
                qc_mode_tag,
                accepted_candidate_indices,
                accepted_transaction_hashes: candidate_hashes,
                previous_lane_block_height,
                previous_lane_block_descriptor_hash: (previous_lane_block_height > 0).then(|| {
                    Hash::new(
                        format!("{}:{}:prev", lane_id.as_u32(), dataspace_id.as_u64()).into_bytes(),
                    )
                }),
                lane_block_descriptor_hash: Some(Hash::new(
                    b"block-test lane descriptor placeholder",
                )),
                lane_block_descriptor_validator_set: descriptor_validator_set,
                lane_block_descriptor_validator_count: validator_count,
                lane_block_descriptor_min_quorum: min_quorum,
                payload_ownership_hash: Hash::new(b"block-test lane payload placeholder"),
                rbc_instance_hash: Hash::new(b"block-test lane rbc placeholder"),
            };
            let replay_hashes = ownership
                .compute_replay_hashes()
                .expect("lane payload ownership replay hashes compute");
            ownership.subject_hash = replay_hashes.subject_hash;
            ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
            ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
            ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
            ownership
        }
        #[test]
        fn native_amx_coordinator_ownership_index_is_unique_and_exact() {
            let validator = PeerId::new(
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            );
            let entrypoint_hash = Hash::new(b"native-amx-index-entrypoint");
            let ownership = sample_lane_payload_ownership_for_context(
                7,
                0,
                LaneId::new(1),
                DataSpaceId::new(2),
                Hash::new(b"native-amx-index-incarnation"),
                vec![0],
                vec![entrypoint_hash],
                &[validator],
            );
            let bundle = BlockExecutionContextBundle::new(Vec::new())
                .with_lane_payload_ownerships(vec![ownership.clone()]);
            let index = ValidBlock::index_native_amx_coordinator_ownerships(&bundle)
                .expect("one ownership must produce one exact index entry");
            assert_eq!(
                index.get(&(LaneId::new(1), DataSpaceId::new(2), 7, entrypoint_hash)),
                Some(&0)
            );
            let duplicate_bundle = BlockExecutionContextBundle::new(Vec::new())
                .with_lane_payload_ownerships(vec![ownership.clone(), ownership]);
            assert!(matches!(
                ValidBlock::index_native_amx_coordinator_ownerships(&duplicate_bundle),
                Err(BlockValidationError::ExecutionContextInvalid(message))
                    if message.contains("repeats entrypoint hash")
            ));
        }
        #[test]
        fn settlement_finalization_rejects_unbound_evidence() {
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let state = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let block = ValidBlock::new_dummy(leader.private_key());
            let mut state_block = state.block(block.as_ref().header());
            let tx_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::new(
                b"unbound sequential settlement transaction",
            ));
            let mut source_id = [0; Hash::LENGTH];
            source_id.copy_from_slice(tx_hash.as_ref());
            state_block.record_settlement_receipt(
                tx_hash,
                crate::settlement::PendingSettlement {
                    source_id,
                    asset_definition_id:
                        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                            DomainId::try_new("wonderland", "universal").expect("domain id"),
                            "settlement".parse().expect("asset name"),
                        ),
                    local_amount: crate::settlement::quantity_from_micro_units(11),
                    xor_due: crate::settlement::quantity_from_micro_units(7),
                    xor_after_haircut: crate::settlement::quantity_from_micro_units(6),
                    xor_variance: crate::settlement::quantity_from_micro_units(1),
                    timestamp_ms: 1,
                    liquidity_profile: settlement_router::LiquidityProfile::Tier1,
                    volatility_bucket: crate::settlement::VolatilityBucket::Stable,
                    twap_local_per_xor: Numeric::one(),
                    epsilon_bps: 25,
                    twap_window_seconds: 60,
                    oracle_timestamp_ms: 1,
                },
            );
            let error = ValidBlock::finalize_lane_settlement_evidence(
                block.as_ref(),
                &mut state_block,
                &[],
                &BTreeMap::new(),
            )
            .expect_err("settlement evidence without a routed transaction must fail closed");
            assert!(
                matches!(
                    error,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("unbound settlement evidence")
                            && message.contains("settlement=1")
                ),
                "unexpected unbound settlement rejection: {error}"
            );
        }
        fn lane_payload_context_fixture() -> (State, Arc<Kura>, Topology, TimeSource, Vec<KeyPair>)
        {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut keys = (0..4)
                .map(|_| crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal))
                .collect::<Vec<_>>();
            keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
            let topology = test_topology_with_keys(&keys);
            let mut world = World::new();
            insert_active_consensus_keys(&mut world, &keys);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &keys);
            commit_block_at_height(&state, &kura, &topology, keys[0].private_key(), 1, None, 1);
            let parent = state
                .view()
                .latest_block()
                .expect("committed fixture parent");
            let finality = applied_lane_predecessor_finality(&parent, &state, &keys);
            let _ = kura
                .store_v2_finality_artifact(&finality)
                .expect("publish the exact parent finality before successor validation");
            let authority = state
                .resolve_lane_committee_at_height(
                    crate::state::LaneAuthorityRoute::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                    2,
                )
                .expect("resolve the fixture's exact four-validator lane authority")
                .into_validators();
            assert_eq!(authority.as_slice(), topology.as_ref());
            let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            (state, kura, topology, time_source, keys)
        }
        fn signed_lane_payload_context_block(
            state: &State,
            topology: &Topology,
            leader: &KeyPair,
            time_source: &TimeSource,
            label: &str,
            lane_block_height: u64,
            predecessor_descriptor_hash: Option<Hash>,
        ) -> SignedBlock {
            signed_lane_payload_context_block_with_descriptor_validators(
                state,
                leader,
                time_source,
                label,
                lane_block_height,
                predecessor_descriptor_hash,
                topology.as_ref(),
            )
        }
        fn signed_lane_payload_context_block_with_descriptor_validators(
            state: &State,
            leader: &KeyPair,
            time_source: &TimeSource,
            label: &str,
            lane_block_height: u64,
            predecessor_descriptor_hash: Option<Hash>,
            descriptor_validators: &[PeerId],
        ) -> SignedBlock {
            let (authority, signer) = gen_account_in(label);
            let tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, label.to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            let proposal_height = u64::try_from(state.block_hashes.view().len().saturating_add(1))
                .expect("test block height fits u64");
            let mut ownership = sample_lane_payload_ownership_for_context_at_slot(
                proposal_height,
                0,
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                state
                    .lane_incarnation(LaneId::SINGLE)
                    .expect("default lane incarnation"),
                lane_block_height,
                0,
                vec![0],
                vec![Hash::from(tx.hash_as_entrypoint())],
                descriptor_validators,
            );
            if let Some(predecessor_descriptor_hash) = predecessor_descriptor_hash {
                ownership.previous_lane_block_descriptor_hash = Some(predecessor_descriptor_hash);
                let replay_hashes = ownership
                    .compute_replay_hashes()
                    .expect("exact predecessor fixture replay hashes compute");
                ownership.subject_hash = replay_hashes.subject_hash;
                ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
                ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
                ownership.lane_block_descriptor_hash =
                    Some(replay_hashes.lane_block_descriptor_hash);
            }
            let execution_context =
                BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                    tx.hash_as_entrypoint(),
                    LaneId::SINGLE,
                    DataSpaceId::UNIVERSAL,
                )])
                .with_lane_payload_ownerships(vec![ownership]);
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context));
            with_current_state_da_sidecars(builder, state)
                .sign(leader.private_key())
                .unpack(|_| {})
                .into()
        }
        fn signed_default_lane_block_with_execution_context(
            label: &str,
            transaction_count: usize,
            context_for: impl FnOnce(
                &[SignedTransaction],
                &[PeerId],
                Hash,
            ) -> BlockExecutionContextBundle,
        ) -> (State, Topology, TimeSource, SignedBlock) {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut key_pairs = (0..4)
                .map(|_| crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal))
                .collect::<Vec<_>>();
            key_pairs.sort_by_key(|key| PeerId::new(key.public_key().clone()));
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];
            let mut world = World::new();
            insert_active_consensus_keys(&mut world, &key_pairs);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &key_pairs);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let mut transactions = Vec::with_capacity(transaction_count);
            for idx in 0..transaction_count {
                let (authority, signer) = gen_account_in(&format!("{label}-{idx}"));
                let tx = TransactionBuilder::new_with_time_source(
                    state.network_id,
                    authority,
                    &time_source,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(Level::INFO, format!("{label}-{idx}"))])
                .sign(signer.private_key());
                transactions.push(tx);
                time_handle.advance(Duration::from_millis(1));
            }
            let accepted = transactions
                .iter()
                .cloned()
                .map(|tx| AcceptedTransaction::new_unchecked(Cow::Owned(tx)))
                .collect::<Vec<_>>();
            let lane_incarnation = state
                .lane_incarnation(LaneId::SINGLE)
                .expect("default lane incarnation");
            let execution_context = context_for(&transactions, topology.as_ref(), lane_incarnation);
            let builder = BlockBuilder::new_with_time_source(accepted, time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {});
            (state, topology, time_source, new_block.into())
        }
        fn assert_contextless_unknown_default_route_is_pristine(
            state: &State,
            block: &mut SignedBlock,
            unknown_dataspace: DataSpaceId,
        ) {
            let entrypoint_hash = block
                .external_entrypoints_cloned()
                .next()
                .expect("routing failure fixture has one entrypoint")
                .hash();
            let mut state_block = state.block(block.header());
            assert!(
                state_block
                    .nexus
                    .dataspace_catalog
                    .by_id(unknown_dataspace)
                    .is_none()
            );
            state_block.nexus.routing_policy.default_dataspace = unknown_dataspace;
            assert_eq!(
                state_block.transactions.get(&entrypoint_hash),
                None,
                "routing failure fixture starts without a transaction-index row"
            );
            let error = ValidBlock::execute_and_record_canonical_outputs(
                block,
                &mut state_block,
                None,
                None,
            )
            .expect_err("an unknown default dataspace must invalidate the whole block");
            assert_eq!(
                error,
                BlockValidationError::ExecutionContextInvalid(format!(
                    "Network route cannot be frozen at index 0: dataspace {unknown_dataspace} is not present in the dataspace catalog"
                ))
            );
            assert_eq!(
                state_block.transactions.get(&entrypoint_hash),
                None,
                "failed routing must not stage a transaction-index row"
            );
            assert!(
                !block.has_results(),
                "failed routing must not attach transaction results or routed metadata"
            );
            assert!(
                block.execution_context().is_none(),
                "contextless routing must not fabricate a lane-0/dataspace-0 context"
            );
            assert!(
                block.lane_finality_statements().is_empty(),
                "failed routing must not fabricate lane finality metadata"
            );
        }
        #[test]
        fn contextless_parallel_validation_fails_before_routing_metadata_or_index_mutation() {
            let (mut state, _, _, mut block) = signed_default_lane_block_with_execution_context(
                "contextless-parallel-routing-failure",
                1,
                |transactions, _, _| {
                    BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                        transactions[0].hash_as_entrypoint(),
                        LaneId::SINGLE,
                        DataSpaceId::UNIVERSAL,
                    )])
                },
            );
            let unknown_dataspace = DataSpaceId::new(4_242);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.workers = 2;
            state.set_pipeline(pipeline);
            block.set_execution_context(None);

            assert_contextless_unknown_default_route_is_pristine(
                &state,
                &mut block,
                unknown_dataspace,
            );
        }
        #[test]
        fn contextless_sealed_source_fails_before_routing_metadata_or_index_mutation() {
            let (state, _, _, mut block) = signed_default_lane_block_with_execution_context(
                "contextless-sequential-routing-failure",
                1,
                |transactions, _, _| {
                    BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                        transactions[0].hash_as_entrypoint(),
                        LaneId::SINGLE,
                        DataSpaceId::UNIVERSAL,
                    )])
                },
            );
            let unknown_dataspace = DataSpaceId::new(4_243);
            let signed = block
                .external_transactions()
                .next()
                .expect("signed fixture source")
                .clone();
            let reveal = iroha_data_model::transaction::signed::SealedTransactionReveal::new(
                Hash::new(b"unresolved sealed routing source"),
                signed,
                [0; 32],
            );
            block.set_external_entrypoints(vec![TransactionEntrypoint::SealedReveal(reveal)]);
            block.set_execution_context(None);
            assert!(
                matches!(
                    block.network_entrypoint_at(0),
                    Some(TransactionEntrypoint::SealedReveal(_))
                ),
                "a sealed Network source must pass through the same frozen routing gate"
            );

            assert_contextless_unknown_default_route_is_pristine(
                &state,
                &mut block,
                unknown_dataspace,
            );
        }
        fn future_created_autoscale_nexus(
            state: &State,
            lane_id: LaneId,
            created_height: u64,
        ) -> iroha_config::parameters::actual::Nexus {
            let mut elastic_lane = LaneConfig {
                id: lane_id,
                alias: format!("elastic-lane-{}", lane_id.as_u32()),
                ..LaneConfig::default()
            };
            elastic_lane
                .metadata
                .insert("autoscale.managed".to_owned(), "true".to_owned());
            elastic_lane.metadata.insert(
                "autoscale.created_height".to_owned(),
                created_height.to_string(),
            );
            crate::state::attach_synthetic_autoscale_committee_for_test(&mut elastic_lane);
            let lane_catalog =
                LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), elastic_lane])
                    .expect("future-created autoscale lane catalog");
            let mut nexus = state.nexus_snapshot();
            nexus.autoscale.enabled = true;
            nexus.autoscale.min_lane_id = nonzero!(1_u32);
            nexus.autoscale.max_lane_id_exclusive = nonzero!(3_u32);
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog);
            nexus.lane_catalog = lane_catalog;
            nexus
        }
        fn install_live_test_elastic_lane(state: &mut State) {
            let mut elastic_lane = LaneConfig {
                id: LaneId::new(1),
                alias: "elastic-lane-1".to_owned(),
                ..LaneConfig::default()
            };
            elastic_lane
                .metadata
                .insert("autoscale.managed".to_owned(), "true".to_owned());
            elastic_lane
                .metadata
                .insert("autoscale.created_height".to_owned(), "1".to_owned());
            crate::state::attach_synthetic_autoscale_committee_for_test(&mut elastic_lane);
            {
                let mut nexus = state.nexus_snapshot();
                nexus.autoscale.enabled = true;
                nexus.autoscale.min_lane_id = nonzero!(1_u32);
                nexus.autoscale.max_lane_id_exclusive = nonzero!(8_u32);
                state
                    .set_nexus_from_config(nexus)
                    .expect("install the pre-genesis autoscale policy");
            }
            state
                .apply_autoscale_lane_lifecycle_for_tests(
                    &iroha_data_model::nexus::LaneLifecyclePlan {
                        additions: vec![elastic_lane],
                        retire: Vec::new(),
                    },
                )
                .expect("apply live elastic lane through the autoscale lifecycle");
        }
        #[test]
        fn signature_verification_ok() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(7)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            key_pairs
                .iter()
                .enumerate()
                // Include only peers in validator set
                .take(topology.min_votes_for_commit())
                // Skip leader since already singed
                .skip(1)
                .filter(|(i, _)| *i != 4) // Skip proxy tail
                .map(|(i, key_pair)| {
                    BlockSignature::new(
                        i as u64,
                        checked_block_signature(key_pair.private_key(), block_hash),
                    )
                })
                .try_for_each(|signature| block.add_signature(signature, &topology))
                .expect("Failed to add signatures");
            block.sign(&key_pairs[4], &topology);
            let _ = block.commit(&topology).unpack(|_| {}).unwrap();
        }
        #[test]
        fn signature_verification_consensus_not_required_ok() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(1)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let block = ValidBlock::new_dummy(key_pairs[0].private_key());
            assert!(block.commit(&topology).unpack(|_| {}).is_ok());
        }
        /// Check requirement of having at least $2f + 1$ signatures in $3f + 1$ network
        #[test]
        fn signature_verification_not_enough_signatures() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(7)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[4], &topology);
            let err = block.commit(&topology).unpack(|_| {}).unwrap_err().1;
            assert_eq!(
                err.as_ref(),
                &BlockValidationError::SignatureVerification(
                    SignatureVerificationError::NotEnoughSignatures {
                        votes_count: 2,
                        min_votes_for_commit: topology.min_votes_for_commit(),
                    }
                )
            );
        }
        #[test]
        fn four_node_quorum_rejects_two_commit_signers() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            assert_eq!(topology.min_votes_for_commit(), 3);
            // Leader is signed by constructor; add only the proxy tail.
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[2], &topology);
            let tally = commit_signature_tally(block.as_ref(), &topology);
            assert_eq!(tally.counted, 2);
            assert_eq!(tally.present, 2);
            let err = block.commit(&topology).unpack(|_| {}).unwrap_err().1;
            assert_eq!(
                err.as_ref(),
                &BlockValidationError::SignatureVerification(
                    SignatureVerificationError::NotEnoughSignatures {
                        votes_count: 2,
                        min_votes_for_commit: 3
                    }
                )
            );
        }
        #[test]
        fn four_node_quorum_accepts_three_commit_signers() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            assert_eq!(topology.min_votes_for_commit(), 3);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[1], &topology); // validator
            block.sign(&key_pairs[2], &topology); // proxy tail
            let tally = commit_signature_tally(block.as_ref(), &topology);
            assert_eq!(tally.counted, 3);
            assert_eq!(tally.present, 3);
            assert_eq!(tally.set_b_signatures, 0);
            assert!(block.commit(&topology).unpack(|_| {}).is_ok());
        }
        #[cfg(feature = "bls")]
        #[test]
        fn v2_certificate_commit_requires_exact_cryptographic_artifact() {
            let mut key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            key_pairs.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let block =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(1_u64));
                });
            let signed = block.as_ref();
            let roster = key_pairs
                .iter()
                .map(
                    |key| iroha_data_model::block::consensus_v2::ValidatorPower {
                        validator: PeerId::new(key.public_key().clone()),
                        power: 1,
                    },
                )
                .collect::<Vec<_>>();
            let network_id =
                crate::unit_test_support::synthetic_network_id("v2-artifact-bound-commit");
            let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                    network_id,
                    u64::MAX,
                    &roster,
                );
            let context = iroha_data_model::block::consensus_v2::HeightContext {
                network_id,
                protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
                height: signed.header().height().get(),
                epoch: 0,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: iroha_data_model::block::consensus_v2::DualQuorum::from_roster(&roster)
                    .expect("fixture quorum"),
                roster,
                kagemusha_mint_finality_authorization,
                kagemusha_mint_finality_authority,
                nexus_amx_context_hash: Hash::new(b"v2 artifact-bound commit context"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: iroha_data_model::block::consensus_v2::DataAvailabilityLayout {
                    encoding: iroha_data_model::block::consensus_v2::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
                leader_seed: [0x41; 32],
            };
            let subject = iroha_data_model::block::consensus_v2::BlockSubject {
                parent_block_hash: signed.header().prev_block_hash(),
                block_hash: signed.hash(),
                payload_hash: signed
                    .canonical_proposal_wire_hash()
                    .expect("canonical proposal block wire"),
            };
            let round = iroha_data_model::block::consensus_v2::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: signed.header().view_change_index(),
            };
            let execution =
                iroha_data_model::block::consensus_v2::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                    Hash::new(b"artifact-bound parent state"),
                    Hash::new(b"artifact-bound post state"),
                    Hash::new(b"artifact-bound ordinary writes"),
                    u64::try_from(
                        signed
                            .encode_wire()
                            .expect("artifact-bound block wire")
                            .len(),
                    )
                    .expect("artifact-bound block wire length fits u64"),
                    signed
                        .executed_block_wire_hash()
                        .expect("canonical executed block wire"),
                );
            let vote = iroha_data_model::block::consensus_v2::Vote {
                round,
                proposal_round: round,
                phase: iroha_data_model::block::consensus_v2::GlobalPhase::Commit,
                subject,
                execution_commitment: execution,
                signer: 0,
                signature: Vec::new(),
            };
            let preimage = vote.signature_preimage();
            let shares = key_pairs[..3]
                .iter()
                .map(|key| {
                    iroha_crypto::Signature::new(key.private_key(), &preimage)
                        .payload()
                        .to_vec()
                })
                .collect::<Vec<_>>();
            let qc = iroha_data_model::block::consensus_v2::QuorumCertificate {
                round,
                proposal_round: round,
                phase: iroha_data_model::block::consensus_v2::GlobalPhase::Commit,
                subject,
                execution_commitment: execution,
                signers: vec![0, 1, 2],
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                    &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                )
                .expect("aggregate fixture CommitQC"),
            };
            let pops = key_pairs
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("fixture PoP")
                })
                .collect();
            let artifact = iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact::new(
                context, subject, qc, pops,
            );
            let forged_block = block.clone();
            let verified_artifact =
                crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())
                    .expect("verify exact fixture finality once");
            let commit_result = block
                .commit_with_verified_v2_artifact(verified_artifact, execution)
                .unpack(|_| {});
            assert!(
                commit_result.is_ok(),
                "an exact cryptographic artifact authorizes the v2 quorum conversion: \
                 {commit_result:?}"
            );
            let committed = commit_result.expect("verified artifact produces a committed block");
            assert_eq!(
                committed.verified_v2_finality_artifact(),
                Some(&artifact),
                "the committed lifecycle type must retain exact v2 State-apply authority"
            );
            let unchecked = forged_block.clone().commit_unchecked().unpack(|_| {});
            assert!(
                unchecked.verified_v2_finality_artifact().is_none(),
                "ordinary and unchecked commits must not mint v2 State-apply authority"
            );
            let mut forged = artifact.clone();
            forged.commit_qc.aggregate_signature[0] ^= 0x80;
            assert!(
                crate::block::VerifiedV2FinalityArtifact::verify(forged).is_err(),
                "a forged aggregate must not mint the commit capability"
            );
            drop(forged_block);
        }
        #[test]
        fn commit_with_signers_accepts_full_roster_quorum() {
            // Six-node topology (min_votes_for_commit = 5). Provide a quorum that excludes the
            // leader (0) but still spans the full roster beyond the first commit set.
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(6)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            assert_eq!(topology.min_votes_for_commit(), 5);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            // Populate the block with commit-role signatures so the QC signer set can omit the
            // leader while still carrying the full commit quorum.
            block.sign(&key_pairs[1], &topology);
            block.sign(&key_pairs[2], &topology);
            block.sign(&key_pairs[3], &topology);
            block.sign(&key_pairs[4], &topology);
            block.sign(&key_pairs[5], &topology);
            let signers: BTreeSet<_> = [1_u32, 2_u32, 3_u32, 4_u32, 5_u32].into_iter().collect();
            let result = block
                .commit_with_signers(&topology, &signers, false)
                .unpack(|_| {});
            assert!(
                result.is_ok(),
                "quorum signers outside the first commit set should still be accepted: {result:?}"
            );
        }
        #[test]
        fn duplicate_signatures_rejected() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(2)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                checked_block_signature(key_pairs[0].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                1,
                checked_block_signature(key_pairs[1].private_key(), block_hash),
            ));
            // Duplicate index with a different signature payload.
            let spoofing_key = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            signatures.insert(BlockSignature::new(
                1,
                checked_block_signature(spoofing_key.private_key(), block_hash),
            ));
            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(
                err,
                SignatureVerificationError::DuplicateSignature { signer: 1 }
            );
            // Original signature set should remain intact after the failed replacement.
            assert_eq!(
                block.as_ref().signatures().count(),
                1,
                "failed replacement must roll back"
            );
        }
        #[test]
        fn proxy_tail_signature_mismatch_rejected() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(2)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                checked_block_signature(key_pairs[0].private_key(), block_hash),
            ));
            // Proxy tail index signed with the wrong key.
            let wrong = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            signatures.insert(BlockSignature::new(
                1,
                checked_block_signature(wrong.private_key(), block_hash),
            ));
            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(err, SignatureVerificationError::UnknownSignature);
            // Original leader-only signature remains after rollback.
            assert_eq!(block.as_ref().signatures().count(), 1);
        }
        #[test]
        fn leader_signature_mismatch_rejected() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(3)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            let mut signatures = BTreeSet::new();
            // Leader slot signed with validator key instead of the leader's.
            signatures.insert(BlockSignature::new(
                0,
                checked_block_signature(key_pairs[1].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                1,
                checked_block_signature(key_pairs[1].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                2,
                checked_block_signature(key_pairs[2].private_key(), block_hash),
            ));
            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(err, SignatureVerificationError::UnknownSignature);
            assert_eq!(block.as_ref().signatures().count(), 1);
        }
        #[test]
        fn set_b_signatures_contribute_to_quorum() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            // Leader signature is included by constructor
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[1], &topology); // validator
            block.sign(&key_pairs[3], &topology); // set B
            assert!(
                block.commit(&topology).unpack(|_| {}).is_ok(),
                "set B signatures should count toward quorum without requiring proxy tail"
            );
        }
        #[test]
        fn set_b_signature_mismatch_rejected() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(5)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            // Set B signature forged with the wrong key should invalidate the block.
            let bogus_set_b = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                checked_block_signature(key_pairs[0].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                1,
                checked_block_signature(key_pairs[1].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                2,
                checked_block_signature(key_pairs[2].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                3,
                checked_block_signature(bogus_set_b.private_key(), block_hash),
            ));
            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(err, SignatureVerificationError::UnknownSignature);
            // Replacement should fail and leave the original leader-only signature set.
            assert_eq!(block.as_ref().signatures().count(), 1);
        }
        #[test]
        fn commit_signature_tally_tracks_present_and_counted_roles() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            // Proxy tail signature should count toward quorum.
            block
                .add_signature(
                    BlockSignature::new(
                        2,
                        checked_block_signature(key_pairs[2].private_key(), block_hash),
                    ),
                    &topology,
                )
                .expect("proxy tail signature");
            // Set B signature counts toward quorum.
            block.sign(&key_pairs[3], &topology);
            let tally = commit_signature_tally(block.as_ref(), &topology);
            assert_eq!(tally.present, 3);
            assert_eq!(tally.counted, 3);
            assert_eq!(tally.set_b_signatures, 1);
        }
        #[test]
        fn add_signature_rejects_leader_slot_without_panicking() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(3)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            let leader_signature = BlockSignature::new(
                0,
                checked_block_signature(key_pairs[0].private_key(), block_hash),
            );
            assert_eq!(
                block.add_signature(leader_signature, &topology),
                Err(SignatureVerificationError::UnknownSignatory)
            );
        }
        #[test]
        fn replace_signatures_rolls_back_on_failure() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(3)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            // Start from a valid quorum.
            block
                .add_signature(
                    BlockSignature::new(
                        1,
                        checked_block_signature(key_pairs[1].private_key(), block_hash),
                    ),
                    &topology,
                )
                .expect("validator signature");
            block.sign(&key_pairs[2], &topology);
            assert!(block.clone().commit(&topology).unpack(|_| {}).is_ok());
            let original = block.as_ref().signatures().cloned().collect::<Vec<_>>();
            // Replacement below quorum should fail and restore the original set.
            let mut replacement = BTreeSet::new();
            replacement.insert(BlockSignature::new(
                0,
                checked_block_signature(key_pairs[0].private_key(), block_hash),
            ));
            replacement.insert(BlockSignature::new(
                1,
                checked_block_signature(key_pairs[1].private_key(), block_hash),
            ));
            let err = block
                .replace_signatures(replacement, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(
                err,
                SignatureVerificationError::NotEnoughSignatures {
                    votes_count: 2,
                    min_votes_for_commit: 3
                }
            );
            let restored: Vec<_> = block.as_ref().signatures().cloned().collect();
            assert_eq!(restored, original);
        }
        #[test]
        fn consensus_key_lifecycle_requires_proxy_tail_entry() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(3)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(5_u64));
                });
            block.sign(&key_pairs[1], &topology);
            block.sign(&key_pairs[2], &topology);
            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                &key_pairs[0],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "validator",
                &key_pairs[1],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();
            let err = ValidBlock::enforce_consensus_key_lifecycle(block.as_ref(), &topology, &view)
                .expect_err("missing proxy tail consensus key should be rejected");
            assert_eq!(err, SignatureVerificationError::InactiveConsensusKey);
        }
        #[test]
        fn validate_signatures_subset_rejects_missing_pop() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(2)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let mut world = World::new();
            let id = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                Ident::from_str("leader").expect("consensus key name parses"),
            );
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key_pairs[0].public_key().clone(),
                pop: None,
                activation_height: 1,
                expiry_height: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world.consensus_keys.insert(id.clone(), record.clone());
            world
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![id.clone()]);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();
            let err = ValidBlock::validate_signatures_subset(block.as_ref(), &topology, &view)
                .expect_err("missing pop should be rejected");
            assert_eq!(err, SignatureVerificationError::MissingPop);
        }
        #[test]
        fn validate_signatures_subset_rejects_all_zero_signature_material_before_aggregate() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(2)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block
                .block
                .replace_signatures(BTreeSet::from([BlockSignature::new(
                    0,
                    SignatureOf::from_signature(iroha_crypto::Signature::from_bytes(&[0_u8; 96])),
                )]))
                .expect("replace block signature fixture");
            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                &key_pairs[0],
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();
            let err = ValidBlock::validate_signatures_subset(block.as_ref(), &topology, &view)
                .expect_err("all-zero block signature payload must reject before aggregation");
            assert_eq!(err, SignatureVerificationError::UnknownSignature);
        }
        #[test]
        fn validate_signatures_subset_accepts_without_consensus_registry() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(2)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let view = state.view();
            ValidBlock::validate_signatures_subset(block.as_ref(), &topology, &view)
                .expect("empty consensus key registry should use direct signature checks");
        }
        #[test]
        fn consensus_key_lifecycle_honours_grace_windows() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(3)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut params = iroha_data_model::parameter::Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 2;
            params.sumeragi.key_expiry_grace_blocks = 1;
            let mut world = World::new();
            world.parameters = mv::cell::Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader",
                &key_pairs[0],
                2,
                Some(5),
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "validator",
                &key_pairs[1],
                2,
                Some(5),
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy",
                &key_pairs[2],
                2,
                Some(5),
                ConsensusKeyStatus::Retiring,
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();
            let mut within_grace =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(6_u64));
                });
            within_grace.sign(&key_pairs[1], &topology);
            within_grace.sign(&key_pairs[2], &topology);
            assert!(
                ValidBlock::enforce_consensus_key_lifecycle(
                    within_grace.as_ref(),
                    &topology,
                    &view
                )
                .is_ok()
            );
            let mut beyond_grace =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(7_u64));
                });
            beyond_grace.sign(&key_pairs[1], &topology);
            beyond_grace.sign(&key_pairs[2], &topology);
            let err = ValidBlock::enforce_consensus_key_lifecycle(
                beyond_grace.as_ref(),
                &topology,
                &view,
            )
            .expect_err("expired consensus keys should be rejected after grace");
            assert_eq!(err, SignatureVerificationError::InactiveConsensusKey);
        }
        #[test]
        fn consensus_key_lifecycle_falls_back_for_stale_pk_index() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(3)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader-active",
                &key_pairs[0],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "validator",
                &key_pairs[1],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy",
                &key_pairs[2],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            // Simulate a stale pk → id index entry that omits the active record.
            let stale_id = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                Ident::from_str("stale").expect("ident parses"),
            );
            world
                .consensus_keys_by_pk
                .insert(key_pairs[0].public_key().to_string(), vec![stale_id]);
            // Active record remains available after inserting stale index entry.
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();
            let mut block =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(3_u64));
                });
            block.sign(&key_pairs[1], &topology);
            block.sign(&key_pairs[2], &topology);
            assert!(
                ValidBlock::enforce_consensus_key_lifecycle(block.as_ref(), &topology, &view)
                    .is_ok()
            );
        }
        fn npos_effects_block(
            leader_private_key: &PrivateKey,
            height: u64,
            effects: Option<NposConsensusEffects>,
        ) -> SignedBlock {
            let valid = ValidBlock::new_dummy_and_modify_header(leader_private_key, |header| {
                header.set_height(NonZeroU64::new(height).expect("non-zero height"));
            });
            let mut block: SignedBlock = valid.into();
            block.set_npos_consensus_effects(effects);
            block
        }
        fn npos_marker_effects(height: u64) -> NposConsensusEffects {
            NposConsensusEffects {
                penalty_actions: vec![
                    iroha_data_model::consensus::NposPenaltyAction::MarkConsensusEvidenceApplied(
                        iroha_data_model::consensus::NposMarkConsensusEvidenceAppliedAction {
                            evidence_key: iroha_crypto::Hash::new([0xA5]),
                            height,
                        },
                    ),
                ],
                ..NposConsensusEffects::default()
            }
        }
        #[test]
        fn penalty_derivation_capacity_is_local_validation_not_invalid_effects() {
            let budget = mv::allocation::AllocationBudget::new(8);
            let original_owner = budget.try_reserve_bytes(7).expect("original pool owner");
            let refusal = match budget.try_reserve_bytes(2) {
                Ok(_) => panic!("occupied pool must refuse the exact demand"),
                Err(refusal) => refusal,
            };
            let classified = ValidBlock::classify_npos_penalty_derivation_error(eyre::Report::new(
                crate::state::EvidencePreparationError::Admission(refusal),
            ));
            let BlockValidationError::EvidencePreparation(local) = classified else {
                panic!("penalty preparation capacity must stay a typed local refusal");
            };
            assert!(local.release_wait().is_some());
            drop(original_owner);
            assert_eq!(budget.reserved_bytes(), 0);
            assert!(matches!(
                ValidBlock::classify_npos_penalty_derivation_error(eyre::eyre!(
                    "malformed committed penalty source"
                )),
                BlockValidationError::NposEffectsInvalid(_)
            ));
        }
        #[test]
        fn soft_fork_replacement_rejects_npos_effects_before_overlay_construction() {
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let block = npos_effects_block(leader.private_key(), 2, Some(npos_marker_effects(2)));
            assert!(matches!(
                ValidBlock::validate_npos_soft_fork_composition(&block, true),
                Err(BlockValidationError::NposEffectsInvalid(reason))
                    if reason.contains("soft-fork replacement")
            ));
            ValidBlock::validate_npos_soft_fork_composition(&block, false)
                .expect("ordinary parent-preserving execution may apply NPoS effects");
        }
        #[test]
        fn validation_profiles_always_carry_an_explicit_consensus_mode() {
            use iroha_data_model::block::consensus_v2::ConsensusMode;
            // World/Parameters defaults do not authenticate a consensus mode.
            assert!(World::new().view().sumeragi_npos_parameters().is_none());
            assert_eq!(
                ConsensusValidationProfile::SumeragiGenesis {
                    consensus_mode: ConsensusMode::Npos,
                }
                .authoritative_consensus_mode(),
                ConsensusMode::Npos,
                "an explicitly authenticated NPoS mode must not be downgraded",
            );
        }
        #[test]
        fn consensus_mode_effects_permissioned_skips_npos_derivation_without_signed_parameters() {
            setup_stateless_cache_state!(kura, state, leader_private, topology, validator_keys);
            let block = npos_effects_block(&leader_private, 2, None);
            ValidBlock::validate_npos_effects_with_state(
                &block,
                &state,
                Some(iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned),
                Some(&authenticated_permissioned_successor_context(
                    &state,
                    &validator_keys,
                )),
            )
            .expect("permissioned validation must not derive NPoS-only penalties");
        }
        #[test]
        fn committed_parliament_request_rejects_candidate_without_pulse() {
            setup_stateless_cache_state!(kura, state, leader_private, topology, validator_keys);
            let mut context = authenticated_permissioned_successor_context(&state, &validator_keys);
            context.height = 12;
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>();
            let (_attempt_id, _request_ids, attempt) =
                crate::beacon::tests::pending_batched_sortition_attempt(
                    &context.network_id,
                    &roster,
                    context.height,
                );
            {
                let mut block = state.world.block();
                {
                    let mut transaction = block.transaction_without_telemetry(
                        iroha_config::parameters::actual::LaneConfig::default(),
                        0,
                    );
                    transaction
                        .put_parliament_attempt(attempt)
                        .expect("persist the committed Parliament pulse request and its indexes");
                    transaction.apply();
                }
                block.commit();
            }
            let candidate = npos_effects_block(&leader_private, context.height, None);
            let error = ValidBlock::validate_npos_effects_with_state(
                &candidate,
                &state,
                Some(iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned),
                Some(&context),
            )
            .expect_err("a committed Parliament pulse request must reject omission");
            assert!(matches!(
                error,
                BlockValidationError::NposEffectsInvalid(message)
                    if message.contains("requested by committed pre-state")
            ));
            drop((kura, topology));
        }
        #[test]
        fn consensus_mode_effects_npos_still_requires_signed_parameters() {
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let state = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let block = npos_effects_block(leader.private_key(), 2, None);
            let err = ValidBlock::validate_npos_effects_with_state(
                &block,
                &state,
                Some(iroha_data_model::block::consensus_v2::ConsensusMode::Npos),
                None,
            )
            .expect_err("NPoS validation must require an authenticated height context");
            assert!(matches!(
                err,
                BlockValidationError::NposEffectsInvalid(message)
                    if message.contains("authenticated height context")
            ));
        }
        #[test]
        fn execution_context_header_rejects_unsupported_bundle_version() {
            let (_, _, _, block) = signed_default_lane_block_with_execution_context(
                "execution-context-version",
                1,
                |transactions, _, _| {
                    let external = transactions
                        .iter()
                        .map(|transaction| {
                            ExternalExecutionContext::new(
                                transaction.hash_as_entrypoint(),
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                            )
                        })
                        .collect();
                    let mut bundle = BlockExecutionContextBundle::new(external);
                    bundle.version = BLOCK_EXECUTION_CONTEXT_BUNDLE_VERSION_V1 + 1;
                    bundle
                },
            );
            let error = ValidBlock::validate_execution_context_header(&block)
                .expect_err("unsupported execution-context bundle version must fail closed");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("unsupported block execution-context bundle version")
            ));
        }
        #[test]
        fn direct_ordinary_entries_and_exact_lifecycle_need_no_lane_ownership() {
            use iroha_data_model::isi::consensus_keys::{
                ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
                ThresholdKeyLifecycleCertificateV1,
            };
            let (state, topology, _, mut block) = signed_default_lane_block_with_execution_context(
                "lifecycle-control-coverage",
                1,
                |transactions, _, _| {
                    BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                        transactions[0].hash_as_entrypoint(),
                        LaneId::SINGLE,
                        DataSpaceId::UNIVERSAL,
                    )])
                },
            );
            let state_view = state.view();
            let ordinary_bundle = block.execution_context().expect("ordinary route");
            ValidBlock::validate_execution_context_lane_payload_ownerships(
                &block,
                &topology,
                &state_view,
                ordinary_bundle,
            )
            .expect("ordinary input binds its route in the signed global proposal");

            let (authority, signer) = gen_account_in("lifecycle-control-coverage-cert");
            let certificate = ThresholdKeyLifecycleCertificateV1 {
                version: 1,
                action: ThresholdKeyLifecycleActionV1::RetireParliamentTleKey,
                expected_active_session_id: Some([0x31; 32]),
                effective_height: block.header().height().get(),
                network_id: state.network_id,
                roster_hash: [0x32; 32],
                committee_size: 4,
                quorum: 3,
                session_id: [0x31; 32],
                transcript_hash: [0x33; 32],
                public_state: Vec::new(),
                signatures: Vec::new(),
            };
            let signed = TransactionBuilder::new(
                state.network_id,
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::Ordinary,
            )
            .with_instructions([ApplyThresholdKeyLifecycleCertificateV1 { certificate }])
            .sign(signer.private_key());
            let entrypoint = TransactionEntrypoint::External(signed);
            let bundle = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                entrypoint.hash(),
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            )]);
            block.set_external_entrypoints(vec![entrypoint]);
            ValidBlock::validate_execution_context_lane_payload_ownerships(
                &block,
                &topology,
                &state_view,
                &bundle,
            )
            .expect("sole exact-height global lifecycle control needs no legacy lane payload");
        }
        #[test]
        fn validate_static_state_dependent_accepts_lane_payload_ownership_context() {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-valid",
                    1,
                    |transactions, validators, lane_incarnation| {
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![
                            sample_lane_payload_ownership_for_context_at_slot(
                                2,
                                0,
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                                lane_incarnation,
                                1,
                                0,
                                vec![0],
                                vec![Hash::from(transactions[0].hash_as_entrypoint())],
                                validators,
                            ),
                        ])
                    },
                );
            let view = state.query_view();
            validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect("matching lane payload ownership must validate");
        }
        #[test]
        fn single_lane_context_uses_lane_authority_not_commit_topology() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let lane_keypairs = (0..4)
                .map(|_| crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal))
                .collect::<Vec<_>>();
            let topology = test_topology_with_keys(std::slice::from_ref(&leader));
            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            for (index, keypair) in lane_keypairs.iter().enumerate() {
                insert_consensus_key(
                    &mut world,
                    &format!("single-lane-validator-{index}"),
                    keypair,
                    0,
                    None,
                    ConsensusKeyStatus::Active,
                );
            }
            let state = State::new_for_testing(world, Arc::clone(&kura), query);

            install_test_lane_manifests_for_keypairs(&state, &lane_keypairs);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let lane_authority = state
                .resolve_lane_committee_at_height(
                    crate::state::LaneAuthorityRoute::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                    2,
                )
                .expect("single-lane authority must resolve")
                .into_validators();
            assert_eq!(lane_authority.len(), lane_keypairs.len());
            assert_ne!(lane_authority.as_slice(), topology.as_ref());

            let topology_authored = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "single-lane-topology-authority",
                1,
                None,
            );
            let view = state.query_view();
            let error =
                validate_static_test_block!(&topology_authored, &topology, &view, &time_source)
                    .expect_err("global topology must not authorize a Nexus lane");
            assert!(
                matches!(
                    error,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("lane block descriptor validator set mismatch")
                ),
                "unexpected topology-authority rejection: {error:?}"
            );
            drop(view);

            let lane_authored = signed_lane_payload_context_block_with_descriptor_validators(
                &state,
                &leader,
                &time_source,
                "single-lane-canonical-authority",
                1,
                None,
                &lane_authority,
            );
            let view = state.query_view();
            validate_static_test_block!(&lane_authored, &topology, &view, &time_source)
                .expect("Nexus must accept its exact lane authority");
        }
        #[test]
        fn validate_static_state_dependent_rejects_nonzero_planner_origin_lane_view() {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-nonzero-origin-view",
                    1,
                    |transactions, validators, lane_incarnation| {
                        let ownership = sample_lane_payload_ownership_for_context_at_slot(
                            2,
                            0,
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                            lane_incarnation,
                            1,
                            1,
                            vec![0],
                            vec![Hash::from(transactions[0].hash_as_entrypoint())],
                            validators,
                        );
                        ownership
                            .validate_replay_material()
                            .expect("nonzero lane-view fixture must carry fresh replay hashes");
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![ownership])
                    },
                );
            let view = state.query_view();
            let error = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("planner-origin lane payloads must start at lane view zero");
            assert!(
                matches!(
                    error,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("must originate at lane-local view zero")
                ),
                "unexpected nonzero lane-view rejection: {error}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_reused_lane_payload_artifact_height() {
            let (state, kura, topology, time_source, keys) = lane_payload_context_fixture();
            let leader = &keys[0];
            let mut first = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "lane-payload-artifact-first",
                2,
                None,
            );
            let entrypoint_hashes = first
                .external_entrypoints_cloned()
                .map(|entrypoint| entrypoint.hash())
                .collect::<Vec<_>>();
            let policy_snapshot = state.block(first.header()).axt_policy_snapshot();
            first
                .set_execution_outputs(
                    crate::execution_output_test_support::structural_network_outputs(
                        &first,
                        &entrypoint_hashes,
                        vec![Ok(DataTriggerSequence::default())],
                    ),
                    u64::try_from(first.network_entrypoint_count())
                        .expect("fixture input count fits u64"),
                    BTreeMap::new(),
                    Vec::new(),
                    policy_snapshot,
                    BTreeSet::new(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("attach canonical first-artifact results and AXT policy snapshot");
            first
                .replace_signatures(
                    [BlockSignature::new(
                        0,
                        checked_block_signature(leader.private_key(), first.hash()),
                    )]
                    .into_iter()
                    .collect(),
                )
                .expect("resign the exact result-bearing first-artifact fixture");
            let committed_first = ValidBlock::new_unverified_for_tests(first.clone())
                .commit_unchecked()
                .unpack(|_| {});
            {
                let mut state_block = state.block(committed_first.as_ref().header());
                let _ = state_block
                    .apply_without_execution(&committed_first, topology.as_ref().to_owned());
                state_block
                    .commit()
                    .expect("commit first lane artifact block");
            }
            kura.store_block(Arc::new(first))
                .expect("store first lane artifact block");
            assert!(
                kura.read_lane_block_artifact(LaneId::SINGLE, 2).is_some(),
                "test setup should persist the first lane-local artifact"
            );
            let second = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "lane-payload-artifact-reuse",
                2,
                None,
            );
            let view = state.query_view();
            let err = validate_static_test_block!(&second, &topology, &view, &time_source)
                .expect_err("reused lane-local artifact height must be rejected");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("reuses committed lane artifact")
                ),
                "unexpected validation error: {err:?}"
            );
        }
        #[test]
        fn validate_execution_context_requires_exact_canonical_lane_predecessor() {
            let (state, kura, topology, time_source, keys) = lane_payload_context_fixture();
            let leader = &keys[0];
            let mut first = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "lane-payload-predecessor-first",
                1,
                None,
            );
            let predecessor_descriptor_hash = first
                .execution_context()
                .expect("first execution context")
                .lane_payload_ownerships
                .first()
                .and_then(|ownership| ownership.lane_block_descriptor_hash)
                .expect("first lane descriptor hash");
            let predecessor_ownership = first
                .execution_context()
                .expect("first execution context")
                .lane_payload_ownerships[0]
                .clone();
            let predecessor_proposal =
                native_amx_coordinator_proposal_from_ownership(&predecessor_ownership)
                    .expect("first lane proposal reconstructs");
            let entrypoint_hashes = first
                .external_entrypoints_cloned()
                .map(|entrypoint| entrypoint.hash())
                .collect::<Vec<_>>();
            let policy_snapshot = state.block(first.header()).axt_policy_snapshot();
            first
                .set_execution_outputs(
                    crate::execution_output_test_support::structural_network_outputs(
                        &first,
                        &entrypoint_hashes,
                        vec![Ok(DataTriggerSequence::default())],
                    ),
                    u64::try_from(first.network_entrypoint_count())
                        .expect("fixture input count fits u64"),
                    BTreeMap::new(),
                    Vec::new(),
                    policy_snapshot,
                    BTreeSet::new(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("attach canonical predecessor results and AXT policy snapshot");
            first
                .replace_signatures(
                    [BlockSignature::new(
                        0,
                        checked_block_signature(leader.private_key(), first.hash()),
                    )]
                    .into_iter()
                    .collect(),
                )
                .expect("resign the exact result-bearing predecessor");
            let artifact = applied_lane_predecessor_finality(&first, &state, &keys);
            let verified = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())
                .expect("verify the complete predecessor authority");
            let committed_first = ValidBlock::new_unverified_for_tests(first.clone())
                .commit_with_verified_v2_artifact(verified, artifact.commit_qc.execution_commitment)
                .unpack(|_| {})
                .expect("retain exact finality authority in the committed predecessor");
            kura.store_block(Arc::new(first))
                .expect("store first lane predecessor artifact");
            let _ = kura
                .store_v2_finality_artifact(&artifact)
                .expect("publish exact finality before admitting raw predecessor ownership");
            {
                let mut state_block = state.block(committed_first.as_ref().header());
                let error = state_block
                    .apply_without_execution_with_verified_v2_finality(&committed_first)
                    .expect_err("finality alone cannot invent an unexecuted lane frontier");
                assert!(
                    matches!(
                        error,
                        crate::state::MergeLedgerCommitError::ExecutionMarkerConflict(ref message)
                            if message.contains("missing its exact executed frontier")
                    ),
                    "unexpected unexecuted predecessor error: {error:?}"
                );
                state_block
                    .stage_ordinary_lane_frontiers(committed_first.as_ref())
                    .expect("stage the result-bearing predecessor execution frontier");
                // The test supplies structural results rather than executing a
                // proposal. Prepare its predecessor with the explicit fixture API.
                let _ = state_block.apply_without_execution(
                    &committed_first,
                    artifact
                        .height_context
                        .roster
                        .iter()
                        .map(|entry| entry.validator.clone())
                        .collect(),
                );
                state_block
                    .commit()
                    .expect("commit first lane predecessor block");
            }
            assert!(
                kura.read_lane_application_receipt(LaneId::SINGLE, 1)
                    .expect("read the intentionally unpublished receipt slot")
                    .is_none(),
                "the positive raw-predecessor control must start before receipt publication"
            );
            let exact_predecessor = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "lane-payload-predecessor-exact",
                2,
                Some(predecessor_descriptor_hash),
            );
            let view = state.query_view();
            let v2_profile = sumeragi_v2_test_profile(&exact_predecessor);
            ValidBlock::validate_execution_context_with_state(
                &exact_predecessor,
                &topology,
                &view,
                v2_profile.clone(),
            )
            .expect(
                "Sumeragi v2 accepts the exact canonical raw predecessor while its receipt catches up",
            );
            let wrong_raw_predecessor = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "lane-payload-predecessor-wrong-raw",
                2,
                None,
            );
            let err = ValidBlock::validate_execution_context_with_state(
                &wrong_raw_predecessor,
                &topology,
                &view,
                v2_profile.clone(),
            )
            .expect_err("Sumeragi v2 must not accept a differently bound raw predecessor");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("does not extend the exact applied canonical predecessor")
                ),
                "unexpected wrong raw-predecessor validation error: {err:?}"
            );
            kura.persist_lane_block_application_receipt(&predecessor_proposal)
                .expect("persist first lane predecessor application receipt");
            assert!(
                kura.lane_block_application_receipt_available(&predecessor_proposal),
                "test setup must expose an applied canonical predecessor"
            );
            let wrong_predecessor = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "lane-payload-predecessor-wrong",
                2,
                None,
            );
            let err = ValidBlock::validate_execution_context_with_state(
                &wrong_predecessor,
                &topology,
                &view,
                v2_profile.clone(),
            )
            .expect_err("a self-consistent but wrong predecessor hash must be rejected");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("does not extend the exact applied canonical predecessor")
                ),
                "unexpected wrong-predecessor validation error: {err:?}"
            );
            ValidBlock::validate_execution_context_with_state(
                &exact_predecessor,
                &topology,
                &view,
                v2_profile,
            )
            .expect("the exact same-incarnation canonical predecessor must validate");
        }
        #[test]
        fn validate_execution_context_rejects_missing_canonical_lane_predecessor() {
            let (state, _kura, topology, time_source, keys) = lane_payload_context_fixture();
            let leader = &keys[0];
            let signed = signed_lane_payload_context_block(
                &state,
                &topology,
                &leader,
                &time_source,
                "lane-payload-predecessor-missing",
                2,
                None,
            );
            let view = state.query_view();
            let err = ValidBlock::validate_execution_context_with_state(
                &signed,
                &topology,
                &view,
                sumeragi_v2_test_profile(&signed),
            )
            .expect_err("lane-local height two without height one must be rejected");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("does not extend the exact applied canonical predecessor")
                ),
                "unexpected missing-predecessor validation error: {err:?}"
            );
        }
        fn assert_lane_payload_ownership_context_rejected(
            label: &str,
            mutate: impl FnOnce(&mut SumeragiLanePayloadOwnership),
            expected_message: &str,
        ) {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    label,
                    1,
                    |transactions, validators, lane_incarnation| {
                        let mut ownership = sample_lane_payload_ownership_for_context(
                            2,
                            0,
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                            lane_incarnation,
                            vec![0],
                            vec![Hash::from(transactions[0].hash_as_entrypoint())],
                            validators,
                        );
                        mutate(&mut ownership);
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![ownership])
                    },
                );
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("tampered lane payload ownership must be rejected");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains(expected_message)
                ),
                "unexpected validation error: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_blank_lane_payload_ownership_qc_mode_tag() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-blank-qc-mode",
                |ownership| ownership.qc_mode_tag.clear(),
                "blank QC mode tag",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_missing_lane_block_descriptor_hash() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-missing-descriptor",
                |ownership| ownership.lane_block_descriptor_hash = None,
                "no lane block descriptor hash",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_missing_lane_predecessor_descriptor_hash() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-missing-predecessor-descriptor",
                |ownership| ownership.previous_lane_block_descriptor_hash = None,
                "missing its non-genesis predecessor descriptor hash",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_reused_lane_block_descriptor_hash() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-reused-descriptor",
                |ownership| ownership.lane_block_descriptor_hash = Some(ownership.subject_hash),
                "reuses its lane block descriptor hash",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_lane_block_descriptor_hash_tamper() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-descriptor-tamper",
                |ownership| {
                    ownership.lane_block_descriptor_hash =
                        Some(Hash::new(b"tampered lane descriptor"))
                },
                "lane block descriptor hash mismatch",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_lane_block_descriptor_validator_set_drift() {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-descriptor-validator-drift",
                    1,
                    |transactions, _validators, lane_incarnation| {
                        let wrong_validator =
                            crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
                        let wrong_validators =
                            vec![PeerId::new(wrong_validator.public_key().clone())];
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![
                            sample_lane_payload_ownership_for_context(
                                2,
                                0,
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                                lane_incarnation,
                                vec![0],
                                vec![Hash::from(transactions[0].hash_as_entrypoint())],
                                &wrong_validators,
                            ),
                        ])
                    },
                );
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("validator-set drift in lane descriptor must be rejected");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("lane block descriptor validator set mismatch")
                ),
                "unexpected validation error: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_lane_payload_ownership_subject_hash_tamper() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-subject-tamper",
                |ownership| ownership.subject_hash = Hash::new(b"tampered lane subject"),
                "subject hash mismatch",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_self_consistent_stale_lane_incarnation() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-stale-incarnation",
                |ownership| {
                    ownership.lane_incarnation = Hash::new(b"retired lane incarnation");
                    let replay_hashes = ownership
                        .compute_replay_hashes()
                        .expect("stale-incarnation replay material remains internally consistent");
                    ownership.subject_hash = replay_hashes.subject_hash;
                    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
                    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
                    ownership.lane_block_descriptor_hash =
                        Some(replay_hashes.lane_block_descriptor_hash);
                },
                "does not bind the active proposal-height lane incarnation",
            );
        }
        #[test]
        fn validate_execution_context_profile_cannot_weaken_subject_hash_validation() {
            let (state, topology, _time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-profile-subject-reject",
                    1,
                    |transactions, validators, lane_incarnation| {
                        let mut ownership = sample_lane_payload_ownership_for_context_at_slot(
                            2,
                            0,
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                            lane_incarnation,
                            1,
                            0,
                            vec![0],
                            vec![Hash::from(transactions[0].hash_as_entrypoint())],
                            validators,
                        );
                        ownership.subject_hash = Hash::new(b"profile-subject-tamper");
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![ownership])
                    },
                );
            let view = state.query_view();
            let err = ValidBlock::validate_execution_context_with_state(
                &signed,
                &topology,
                &view,
                sumeragi_v2_test_profile(&signed),
            )
            .expect_err("network identity must never weaken current lane replay validation");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("subject hash mismatch")
                ),
                "unexpected validation error: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_lane_payload_ownership_candidate_hash_drift() {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-candidate-hash-drift",
                    1,
                    |transactions, validators, lane_incarnation| {
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![
                            sample_lane_payload_ownership_for_context(
                                2,
                                0,
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                                lane_incarnation,
                                vec![0],
                                vec![Hash::new(b"wrong lane candidate hash")],
                                validators,
                            ),
                        ])
                    },
                );
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("candidate-hash drift must be rejected");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("accepted transaction hashes mismatch")
                ),
                "unexpected validation error: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_lane_payload_ownership_hash_tamper() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-payload-tamper",
                |ownership| ownership.payload_ownership_hash = Hash::new(b"tampered lane payload"),
                "payload ownership hash mismatch",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_lane_payload_ownership_rbc_hash_tamper() {
            assert_lane_payload_ownership_context_rejected(
                "lane-payload-context-rbc-tamper",
                |ownership| ownership.rbc_instance_hash = Hash::new(b"tampered lane rbc"),
                "RBC instance hash mismatch",
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_lane_payload_ownership_route_mismatch() {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-route-mismatch",
                    1,
                    |transactions, validators, lane_incarnation| {
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![
                            sample_lane_payload_ownership_for_context(
                                2,
                                0,
                                LaneId::new(3),
                                DataSpaceId::UNIVERSAL,
                                lane_incarnation,
                                vec![0],
                                vec![Hash::from(transactions[0].hash_as_entrypoint())],
                                validators,
                            ),
                        ])
                    },
                );
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("ownership lane/dataspace drift must be rejected");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("lane payload ownership")
                            && message.contains("does not target the active proposal-height lane route")
                ),
                "unexpected ownership-route rejection: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_partial_lane_payload_ownership_coverage() {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-partial",
                    2,
                    |transactions, validators, lane_incarnation| {
                        BlockExecutionContextBundle::new(vec![
                            ExternalExecutionContext::new(
                                transactions[0].hash_as_entrypoint(),
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                            ),
                            ExternalExecutionContext::new(
                                transactions[1].hash_as_entrypoint(),
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                            ),
                        ])
                        .with_lane_payload_ownerships(vec![
                            sample_lane_payload_ownership_for_context(
                                2,
                                0,
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                                lane_incarnation,
                                vec![0],
                                vec![Hash::from(transactions[0].hash_as_entrypoint())],
                                validators,
                            ),
                        ])
                    },
                );
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("partial lane payload ownership coverage must be rejected");
            assert!(
                matches!(
                err,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("lane payload ownerships do not cover execution context index")
                ),
                "unexpected validation error: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_out_of_range_lane_payload_ownership_index() {
            let (state, topology, time_source, signed) =
                signed_default_lane_block_with_execution_context(
                    "lane-payload-context-out-of-range",
                    1,
                    |transactions, validators, lane_incarnation| {
                        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                            transactions[0].hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )])
                        .with_lane_payload_ownerships(vec![
                            sample_lane_payload_ownership_for_context(
                                2,
                                0,
                                LaneId::SINGLE,
                                DataSpaceId::UNIVERSAL,
                                lane_incarnation,
                                vec![1],
                                vec![Hash::from(transactions[0].hash_as_entrypoint())],
                                validators,
                            ),
                        ])
                    },
                );
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("out-of-range lane ownership indices must be rejected");
            assert!(matches!(
                err,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("entrypoint index 1 is out of bounds")
            ));
        }
        #[test]
        fn validate_static_state_dependent_rejects_committed_context_when_policy_derives_default() {
            let query = LiveQueryStore::start_test();
            let mut key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            key_pairs.sort_by_key(|key| PeerId::new(key.public_key().clone()));
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];
            let (authority, signer) = gen_account_in("context-check");
            let domain_id = DomainId::try_new("context-check", "universal").expect("domain id");
            let domain = Domain::new(domain_id).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let mut world = World::with([domain], [account], []);
            insert_active_consensus_keys(&mut world, &key_pairs);
            insert_active_participant_keys(&mut world, &key_pairs);
            let paynet_lane = LaneId::new(3);
            let paynet_dataspace = DataSpaceId::new(10);
            let nexus = {
                let mut nexus = iroha_config::parameters::actual::Nexus::default();
                nexus.lane_catalog = LaneCatalog::new(
                    nonzero!(4_u32),
                    vec![
                        LaneConfig::default(),
                        LaneConfig {
                            id: paynet_lane,
                            dataspace_id: paynet_dataspace,
                            alias: "paynet".to_owned(),
                            ..LaneConfig::default()
                        },
                    ],
                )
                .expect("lane catalog");
                nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
                    DataSpaceMetadata::default(),
                    DataSpaceMetadata {
                        id: paynet_dataspace,
                        alias: "paynet".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                ])
                .expect("dataspace catalog");
                nexus
            };
            let state = State::new_with_nexus_for_testing(world, nexus, query);
            let kura = state.kura();
            install_test_lane_manifests_for_keypairs(&state, &key_pairs);
            let _prev_hash = commit_block_with_applied_lane_predecessors(
                &state,
                kura,
                &topology,
                leader.private_key(),
                &key_pairs,
                &[
                    (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                    (paynet_lane, paynet_dataspace),
                ],
            );
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "context".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            time_handle.advance(Duration::from_millis(1));
            let mut ownership = sample_lane_payload_ownership_for_context(
                2,
                0,
                paynet_lane,
                paynet_dataspace,
                static_test_lane_incarnation(&state.nexus_snapshot().lane_catalog, paynet_lane),
                vec![0],
                vec![Hash::from(tx.hash_as_entrypoint())],
                topology.as_ref(),
            );
            bind_applied_lane_predecessor(kura, &mut ownership);
            let execution_context =
                BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                    tx.hash_as_entrypoint(),
                    paynet_lane,
                    paynet_dataspace,
                )])
                .with_lane_payload_ownerships(vec![ownership]);
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err(
                    "default-routed transactions must not accept arbitrary durable routing",
                );
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message == "execution context routing cannot be reconciled at index 0: committed routing plan differs from the fresh dataspace/role topology"
                ),
                "unexpected committed-route rejection: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_native_amx_participant_leg_mismatch() {
            let query = LiveQueryStore::start_test();
            let mut key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            key_pairs.sort_by_key(|key| PeerId::new(key.public_key().clone()));
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];
            let first_dataspace = DataSpaceId::new(7);
            let second_dataspace = DataSpaceId::new(8);
            let mut world = World::new();
            insert_active_consensus_keys(&mut world, &key_pairs);
            insert_active_participant_keys(&mut world, &key_pairs);
            let nexus = {
                let mut nexus = iroha_config::parameters::actual::Nexus::default();
                nexus.lane_catalog = LaneCatalog::new(
                    nonzero!(4_u32),
                    vec![
                        LaneConfig::default(),
                        LaneConfig {
                            id: LaneId::new(2),
                            dataspace_id: first_dataspace,
                            alias: "first".to_owned(),
                            ..LaneConfig::default()
                        },
                        LaneConfig {
                            id: LaneId::new(3),
                            dataspace_id: second_dataspace,
                            alias: "second".to_owned(),
                            ..LaneConfig::default()
                        },
                    ],
                )
                .expect("lane catalog");
                nexus.lane_config =
                    iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
                nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
                    DataSpaceMetadata::default(),
                    DataSpaceMetadata {
                        id: first_dataspace,
                        alias: "acme".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                    DataSpaceMetadata {
                        id: second_dataspace,
                        alias: "bank".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                ])
                .expect("dataspace catalog");
                nexus
            };
            let state = State::new_with_nexus_for_testing(world, nexus, query);
            let kura = state.kura();
            install_test_lane_manifests_for_keypairs(&state, &key_pairs);
            let _prev_hash = commit_block_with_applied_lane_predecessors(
                &state,
                &kura,
                &topology,
                leader.private_key(),
                &key_pairs,
                &[
                    (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                    (LaneId::new(2), first_dataspace),
                    (LaneId::new(3), second_dataspace),
                ],
            );
            let (authority, signer) = gen_account_in("context-check");
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ])
            .sign(signer.private_key());
            let accepted_for_plan = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            let plan = {
                let view = state.view();
                crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &view.nexus,
                    &accepted_for_plan,
                    view.world(),
                    u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
                    2,
                )
                .expect("mixed dataspace write targets should build a native AMX plan")
            };
            assert!(matches!(plan, crate::queue::RoutingPlan::NativeAmx(_)));
            let mut context =
                crate::queue::execution_context_for_routing_plan(tx.hash_as_entrypoint(), &plan);
            let coordinator_lane = context.lane_id;
            let coordinator_dataspace = context.dataspace_id;
            let stale_participant = context
                .routing_plan_legs
                .iter_mut()
                .find(|leg| {
                    leg.role == ExternalExecutionRouteRole::Participant
                        && leg.dataspace_id == second_dataspace
                })
                .expect("native AMX context should include second dataspace participant");
            stale_participant.lane_id = LaneId::new(99);
            let mut ownership = sample_lane_payload_ownership_for_context(
                2,
                0,
                coordinator_lane,
                coordinator_dataspace,
                state
                    .lane_incarnation_at_height(coordinator_lane, 2)
                    .expect("native AMX coordinator incarnation at candidate height"),
                vec![0],
                vec![Hash::from(tx.hash_as_entrypoint())],
                topology.as_ref(),
            );
            bind_applied_lane_predecessor(&kura, &mut ownership);
            let execution_context = BlockExecutionContextBundle::new(vec![context])
                .with_lane_payload_ownerships(vec![ownership]);
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            time_handle.advance(Duration::from_millis(1));
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("stale Native AMX participant leg must be rejected");
            assert!(matches!(
                err,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("routing plan is not canonical")
            ));
        }
        #[test]
        fn execution_context_validation_uses_sealed_reveal_entrypoint_as_native_amx_source() {
            let query = LiveQueryStore::start_test();
            let mut key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            key_pairs.sort_by_key(|key| PeerId::new(key.public_key().clone()));
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];
            let first_dataspace = DataSpaceId::new(7);
            let second_dataspace = DataSpaceId::new(8);
            let mut world = World::new();
            insert_active_consensus_keys(&mut world, &key_pairs);
            insert_active_participant_keys(&mut world, &key_pairs);
            let nexus = {
                let mut nexus = iroha_config::parameters::actual::Nexus::default();
                nexus.lane_catalog = LaneCatalog::new(
                    nonzero!(4_u32),
                    vec![
                        LaneConfig::default(),
                        LaneConfig {
                            id: LaneId::new(2),
                            dataspace_id: first_dataspace,
                            alias: "first".to_owned(),
                            ..LaneConfig::default()
                        },
                        LaneConfig {
                            id: LaneId::new(3),
                            dataspace_id: second_dataspace,
                            alias: "second".to_owned(),
                            ..LaneConfig::default()
                        },
                    ],
                )
                .expect("lane catalog");
                nexus.lane_config =
                    iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
                nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
                    DataSpaceMetadata::default(),
                    DataSpaceMetadata {
                        id: first_dataspace,
                        alias: "acme".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                    DataSpaceMetadata {
                        id: second_dataspace,
                        alias: "bank".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                ])
                .expect("dataspace catalog");
                nexus
            };
            let state = State::new_with_nexus_for_testing(world, nexus, query);
            let kura = state.kura();
            install_test_lane_manifests_for_keypairs(&state, &key_pairs);
            let _prev_hash = commit_block_with_applied_lane_predecessors(
                &state,
                &kura,
                &topology,
                leader.private_key(),
                &key_pairs,
                &[
                    (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                    (LaneId::new(2), first_dataspace),
                    (LaneId::new(3), second_dataspace),
                ],
            );
            let (authority, signer) = gen_account_in("sealed-native-context-check");
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let signed = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ])
            .sign(signer.private_key());
            let entrypoint = TransactionEntrypoint::SealedReveal(
                iroha_data_model::transaction::signed::SealedTransactionReveal::new(
                    Hash::new(b"ordinary-native-amx-sealed-reveal"),
                    signed.clone(),
                    [0xA5; 32],
                ),
            );
            let entrypoint_hash = entrypoint.hash();
            let accepted_for_plan =
                AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(&entrypoint));
            let plan = {
                let view = state.view();
                crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &view.nexus,
                    &accepted_for_plan,
                    view.world(),
                    u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
                    2,
                )
                .expect("sealed mixed-dataspace writes should build a native AMX plan")
            };
            assert!(matches!(plan, crate::queue::RoutingPlan::NativeAmx(_)));
            let coordinator = plan.coordinator_route();
            let mut ownership = sample_lane_payload_ownership_for_context(
                2,
                0,
                coordinator.lane_id,
                coordinator.dataspace_id,
                state
                    .lane_incarnation_at_height(coordinator.lane_id, 2)
                    .expect("native AMX coordinator incarnation at candidate height"),
                vec![0],
                vec![Hash::from(entrypoint_hash)],
                topology.as_ref(),
            );
            bind_applied_lane_predecessor(&kura, &mut ownership);
            let outer_source_id = native_amx_source_id_from_entrypoint_hash(entrypoint_hash);
            let inner_source_id =
                native_amx_source_id_from_entrypoint_hash(signed.hash_as_entrypoint());
            assert_ne!(outer_source_id, inner_source_id);
            time_handle.advance(Duration::from_millis(1));

            let build_block = |source_id| {
                let receipt = NativeAmxReceipt {
                    version: 2,
                    source_id,
                    network_id: iroha_data_model::NetworkId::from_genesis_hash(
                        HashOf::from_untyped_unchecked(Hash::new(
                            b"foreign-native-amx-network-for-source-boundary",
                        )),
                    ),
                    plan_digest: plan.digest(),
                    lane_id: coordinator.lane_id,
                    dataspace_id: coordinator.dataspace_id,
                    lane_incarnation: ownership.lane_incarnation,
                    authority_context_height: 2,
                    lane_block_height: ownership.lane_block_height,
                    lane_block_view: ownership.lane_block_view,
                    coordinator_proposal_hash: Hash::new(b"source-boundary-coordinator-proposal"),
                    legs: Vec::new(),
                };
                let context =
                    crate::queue::execution_context_for_routing_plan(entrypoint_hash, &plan)
                        .with_native_amx_receipt(receipt);
                let execution_context = BlockExecutionContextBundle::new(vec![context])
                    .with_lane_payload_ownerships(vec![ownership.clone()]);
                let accepted =
                    AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
                let builder =
                    BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                        .chain(0, state.view().latest_block().as_deref())
                        .with_execution_context(Some(execution_context));
                let block = with_current_state_da_sidecars(builder, &state)
                    .sign(leader.private_key())
                    .unpack(|_| {});
                SignedBlock::from(block)
            };
            let outer_source_block = build_block(outer_source_id);
            let view = state.query_view();
            let error = ValidBlock::validate_execution_context_with_state(
                &outer_source_block,
                &topology,
                &view,
                sumeragi_v2_test_profile(&outer_source_block),
            )
            .expect_err("the deliberately foreign receipt network must fail after source binding");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("network identity mismatch")
            ));

            let inner_source_block = build_block(inner_source_id);
            let error = ValidBlock::validate_execution_context_with_state(
                &inner_source_block,
                &topology,
                &view,
                sumeragi_v2_test_profile(&inner_source_block),
            )
            .expect_err("the underlying signed identity must fail at the source boundary");
            assert!(matches!(
                error,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("source entrypoint mismatch")
            ));
        }
        #[test]
        fn validate_static_state_dependent_rejects_stale_default_context_for_elastic_route() {
            setup_elastic_lane_validation_state!(kura, key_pairs, topology, leader, state);
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let mut selected = None;
            for attempt in 0..128 {
                let (authority, signer) = gen_account_in(&format!("elastic-context-{attempt}"));
                let tx = TransactionBuilder::new_with_time_source(
                    state.network_id,
                    authority,
                    &time_source,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(Level::INFO, format!("elastic-context-{attempt}"))])
                .sign(signer.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
                let plan = {
                    let view = state.view();
                    crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
                        2,
                    )
                    .expect("default elastic route resolves")
                };
                if plan.coordinator_route().lane_id == LaneId::new(1) {
                    selected = Some(tx);
                    break;
                }
                time_handle.advance(Duration::from_millis(1));
            }
            let tx = selected.expect("fixture should find a transaction routed to elastic lane");
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            time_handle.advance(Duration::from_millis(1));
            let mut ownership = sample_lane_payload_ownership_for_context(
                2,
                0,
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                state
                    .lane_incarnation_at_height(LaneId::SINGLE, 2)
                    .expect("default lane incarnation at candidate height"),
                vec![0],
                vec![Hash::from(tx.hash_as_entrypoint())],
                topology.as_ref(),
            );
            bind_applied_lane_predecessor(&kura, &mut ownership);
            let stale_default_context =
                BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                    tx.hash_as_entrypoint(),
                    LaneId::SINGLE,
                    DataSpaceId::UNIVERSAL,
                )])
                .with_lane_payload_ownerships(vec![ownership]);
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(stale_default_context));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("elastic-routed transactions must reject stale default-lane context");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("routing cannot be reconciled at index 0")
                            && message.contains(
                                "committed routing plan differs from the fresh dataspace/role topology"
                            )
                ),
                "unexpected stale-default-route rejection: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_accepts_live_autoscale_elastic_context() {
            setup_elastic_lane_validation_state!(kura, key_pairs, topology, leader, state);
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let mut selected = None;
            for attempt in 0..128 {
                let (authority, signer) =
                    gen_account_in(&format!("elastic-context-valid-{attempt}"));
                let tx = TransactionBuilder::new_with_time_source(
                    state.network_id,
                    authority,
                    &time_source,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(
                    Level::INFO,
                    format!("elastic-context-valid-{attempt}"),
                )])
                .sign(signer.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
                let plan = {
                    let view = state.view();
                    crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
                        2,
                    )
                    .expect("default elastic route resolves")
                };
                if plan.coordinator_route().lane_id == LaneId::new(1) {
                    selected = Some((tx, plan));
                    break;
                }
                time_handle.advance(Duration::from_millis(1));
            }
            let (tx, plan) =
                selected.expect("fixture should find a transaction routed to elastic lane");
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            time_handle.advance(Duration::from_millis(1));
            let coordinator = plan.coordinator_route();
            let lane_incarnation = state
                .lane_incarnation_at_height(coordinator.lane_id, 2)
                .expect("elastic lane incarnation at candidate height");
            let descriptor_validators = state
                .resolve_lane_committee_at_height(
                    crate::state::LaneAuthorityRoute::new(
                        coordinator.lane_id,
                        coordinator.dataspace_id,
                    ),
                    2,
                )
                .expect("elastic-lane authority must resolve")
                .into_validators();
            let mut ownership = sample_lane_payload_ownership_for_context(
                2,
                0,
                coordinator.lane_id,
                coordinator.dataspace_id,
                lane_incarnation,
                vec![0],
                vec![Hash::from(tx.hash_as_entrypoint())],
                &descriptor_validators,
            );
            bind_applied_lane_predecessor(&kura, &mut ownership);
            let execution_context = BlockExecutionContextBundle::new(vec![
                crate::queue::execution_context_for_routing_plan(tx.hash_as_entrypoint(), &plan),
            ])
            .with_lane_payload_ownerships(vec![ownership]);
            let proof_policies =
                crate::da::proof_policy_bundle(&state.nexus_snapshot().lane_config);
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context))
                .with_da_proof_policies(Some(proof_policies));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            let view = state.query_view();
            validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect("live autoscale elastic execution context must validate");
        }
        #[test]
        fn validate_static_state_dependent_rejects_stale_geometry_da_proof_policy_hash() {
            setup_static_validation_world!(kura, query, key_pairs, topology, leader, world);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &key_pairs);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let stale_lane = LaneId::new(1);
            let stale_geometry_catalog = LaneCatalog::new(
                nonzero!(2_u32),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: stale_lane,
                        alias: "stale-proof-policy".to_owned(),
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("stale derived geometry catalog");
            let stale_geometry =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&stale_geometry_catalog);
            let stale_policies = crate::da::proof_policy_bundle(&stale_geometry);
            let expected_policy_hash =
                crate::da::active_proof_policy_bundle_hash(&state.nexus_snapshot());
            let stale_policy_hash = Some(HashOf::new(&stale_policies));
            assert_ne!(
                stale_policy_hash,
                Some(expected_policy_hash),
                "stale derived geometry must produce a distinct DA proof-policy hash"
            );
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let new_block = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_proof_policies(Some(stale_policies))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            let view = state.query_view();
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err(
                    "stale derived geometry must not satisfy DA proof-policy hash validation",
                );
            assert!(matches!(
                err,
                BlockValidationError::ProofPolicyHashMismatch { expected, actual }
                    if expected == expected_policy_hash && actual == stale_policy_hash
            ));
        }
        #[test]
        fn validate_da_sidecars_rejects_missing_policy_body_under_signed_hash() {
            let signer = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let mut block: SignedBlock = BlockBuilder::new(Vec::new())
                .chain(0, None)
                .sign(signer.private_key())
                .unpack(|_| {})
                .into();
            let signed_header = block.header();
            assert!(block.da_proof_policies().is_some());
            block.set_da_proof_policies(None);
            block.replace_header_for_testing(signed_header);
            assert!(matches!(
                ValidBlock::validate_da_sidecar_hashes(&block),
                Err(BlockValidationError::DaProofPolicySidecarHashMismatch {
                    expected: None,
                    actual: Some(_),
                })
            ));
        }
        #[test]
        fn validate_da_sidecars_rejects_policy_body_substitution() {
            let signer = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let mut block: SignedBlock = BlockBuilder::new(Vec::new())
                .chain(0, None)
                .sign(signer.private_key())
                .unpack(|_| {})
                .into();
            let signed_header = block.header();
            let mut substituted = block
                .da_proof_policies()
                .expect("builder must attach default policies")
                .clone();
            substituted.policies[0].alias.push_str("-substituted");
            block.set_da_proof_policies(Some(substituted));
            block.replace_header_for_testing(signed_header);
            assert!(matches!(
                ValidBlock::validate_da_sidecar_hashes(&block),
                Err(BlockValidationError::DaProofPolicySidecarHashMismatch {
                    expected: Some(_),
                    actual: Some(_),
                })
            ));
        }
        #[test]
        fn validate_static_state_dependent_rejects_future_created_autoscale_da_policy_hash() {
            setup_static_validation_world!(kura, query, key_pairs, topology, leader, world);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &key_pairs);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let future_created_lane = LaneId::new(1);
            let nexus = future_created_autoscale_nexus(&state, future_created_lane, 7);
            let candidate_height = 2;
            let expected_policy_hash =
                crate::da::active_proof_policy_bundle_hash_at_height(&nexus, candidate_height);
            let heightless_policies = crate::da::active_proof_policy_bundle(&nexus);
            assert!(
                heightless_policies
                    .policies
                    .iter()
                    .any(|policy| policy.lane_id == future_created_lane),
                "fixture requires the heightless policy snapshot to include the future-created lane"
            );
            let heightless_policy_hash = Some(HashOf::new(&heightless_policies));
            assert_ne!(
                heightless_policy_hash,
                Some(expected_policy_hash),
                "height-aware block policy hash must exclude the not-yet-created autoscale lane"
            );
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let new_block = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_proof_policies(Some(heightless_policies))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            assert_eq!(signed.header().height().get(), candidate_height);
            let mut view = state.query_view();
            view.nexus = nexus;
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("heightless future-created autoscale policy hash must be rejected");
            assert!(matches!(
                err,
                BlockValidationError::ProofPolicyHashMismatch { expected, actual }
                    if expected == expected_policy_hash && actual == heightless_policy_hash
            ));
        }
        #[test]
        fn validate_static_state_dependent_accepts_height_aware_da_policy_hash() {
            setup_static_validation_world!(kura, query, key_pairs, topology, leader, world);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &key_pairs);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let future_created_lane = LaneId::new(1);
            let nexus = future_created_autoscale_nexus(&state, future_created_lane, 7);
            let candidate_height = 2;
            let height_aware_policies =
                crate::da::active_proof_policy_bundle_at_height(&nexus, candidate_height);
            assert!(
                height_aware_policies
                    .policies
                    .iter()
                    .all(|policy| policy.lane_id != future_created_lane),
                "policy snapshot before creation height must exclude the autoscale lane"
            );
            let expected_policy_hash = Some(HashOf::new(&height_aware_policies));
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let builder = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_proof_policies(Some(height_aware_policies));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            assert_eq!(signed.header().height().get(), candidate_height);
            assert_eq!(
                signed.header().da_proof_policies_hash(),
                expected_policy_hash
            );
            let mut view = state.query_view();
            view.nexus = nexus;
            validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect("height-aware DA policy hash must validate before autoscale lane creation");
        }
        #[test]
        fn validate_static_state_dependent_rejects_elastic_context_after_lane_removal() {
            setup_elastic_lane_validation_state!(kura, key_pairs, topology, leader, state);
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let mut selected = None;
            for attempt in 0..128 {
                let (authority, signer) =
                    gen_account_in(&format!("elastic-context-disabled-{attempt}"));
                let tx = TransactionBuilder::new_with_time_source(
                    state.network_id,
                    authority,
                    &time_source,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(
                    Level::INFO,
                    format!("elastic-context-disabled-{attempt}"),
                )])
                .sign(signer.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
                let plan = {
                    let view = state.view();
                    crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
                        2,
                    )
                    .expect("catalogued elastic lane should resolve its route")
                };
                if plan.coordinator_route().lane_id == LaneId::new(1) {
                    selected = Some((tx, plan));
                    break;
                }
                time_handle.advance(Duration::from_millis(1));
            }
            let (tx, plan) =
                selected.expect("fixture should find a transaction routed to elastic lane");
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            let coordinator = plan.coordinator_route();
            let lane_incarnation = state
                .lane_incarnation_at_height(coordinator.lane_id, 2)
                .expect("elastic lane incarnation before removing the lane");
            let descriptor_validators = state
                .resolve_lane_committee_at_height(
                    crate::state::LaneAuthorityRoute::new(
                        coordinator.lane_id,
                        coordinator.dataspace_id,
                    ),
                    2,
                )
                .expect("elastic-lane authority must resolve before removal")
                .into_validators();
            let mut invalid_nexus = state.nexus_snapshot();
            {
                let nexus = &mut invalid_nexus;
                nexus.lane_catalog = LaneCatalog::default();
                nexus.lane_config =
                    iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
            }
            assert_eq!(
                crate::state::consensus_lane_dataspace_at_height(
                    plan.coordinator_route().lane_id,
                    &invalid_nexus,
                    2,
                ),
                None,
                "a removed elastic lane must no longer be routable"
            );
            time_handle.advance(Duration::from_millis(1));
            let mut ownership = sample_lane_payload_ownership_for_context(
                2,
                0,
                coordinator.lane_id,
                coordinator.dataspace_id,
                lane_incarnation,
                vec![0],
                vec![Hash::from(tx.hash_as_entrypoint())],
                &descriptor_validators,
            );
            bind_applied_lane_predecessor(&kura, &mut ownership);
            let execution_context = BlockExecutionContextBundle::new(vec![
                crate::queue::execution_context_for_routing_plan(tx.hash_as_entrypoint(), &plan),
            ])
            .with_lane_payload_ownerships(vec![ownership]);
            let proof_policies = crate::da::proof_policy_bundle(&invalid_nexus.lane_config);
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context))
                .with_da_proof_policies(Some(proof_policies.clone()));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .with_da_proof_policies(Some(proof_policies))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            let mut view = state.query_view();
            view.nexus = invalid_nexus;
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err("lane removal must reject stale elastic execution context");
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains(
                            "lane payload ownership 0 does not target the active proposal-height lane route"
                        )
                ),
                "unexpected removed-lane rejection: {err:?}"
            );
        }
        #[test]
        fn validate_static_state_dependent_rejects_elastic_context_when_range_corrupt() {
            setup_elastic_lane_validation_state!(kura, key_pairs, topology, leader, state);
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let mut selected = None;
            for attempt in 0..128 {
                let (authority, signer) =
                    gen_account_in(&format!("elastic-context-corrupt-{attempt}"));
                let tx = TransactionBuilder::new_with_time_source(
                    state.network_id,
                    authority,
                    &time_source,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(
                    Level::INFO,
                    format!("elastic-context-corrupt-{attempt}"),
                )])
                .sign(signer.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
                let plan = {
                    let view = state.view();
                    crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
                        2,
                    )
                    .expect("Nexus should resolve the default elastic route")
                };
                if plan.coordinator_route().lane_id == LaneId::new(1) {
                    selected = Some((tx, plan));
                    break;
                }
                time_handle.advance(Duration::from_millis(1));
            }
            let (tx, plan) =
                selected.expect("fixture should find a transaction routed to elastic lane");
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            let coordinator = plan.coordinator_route();
            let lane_incarnation = state
                .lane_incarnation_at_height(coordinator.lane_id, 2)
                .expect("elastic lane incarnation before corrupting the active range");
            let descriptor_validators = state
                .resolve_lane_committee_at_height(
                    crate::state::LaneAuthorityRoute::new(
                        coordinator.lane_id,
                        coordinator.dataspace_id,
                    ),
                    2,
                )
                .expect("elastic-lane authority must resolve before corrupting the range")
                .into_validators();
            let mut invalid_nexus = state.nexus_snapshot();
            {
                let nexus = &mut invalid_nexus;
                let mut lanes = nexus.lane_catalog.lanes().to_vec();
                lanes.push(LaneConfig {
                    id: LaneId::new(2),
                    alias: "manual-lane-inside-elastic-range".to_owned(),
                    ..LaneConfig::default()
                });
                nexus.lane_catalog =
                    LaneCatalog::new(nonzero!(3_u32), lanes).expect("corrupted lane catalog");
                nexus.lane_config =
                    iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
            }
            time_handle.advance(Duration::from_millis(1));
            let mut ownership = sample_lane_payload_ownership_for_context(
                2,
                0,
                coordinator.lane_id,
                coordinator.dataspace_id,
                lane_incarnation,
                vec![0],
                vec![Hash::from(tx.hash_as_entrypoint())],
                &descriptor_validators,
            );
            bind_applied_lane_predecessor(&kura, &mut ownership);
            let execution_context = BlockExecutionContextBundle::new(vec![
                crate::queue::execution_context_for_routing_plan(tx.hash_as_entrypoint(), &plan),
            ])
            .with_lane_payload_ownerships(vec![ownership]);
            let proof_policies = crate::da::proof_policy_bundle(&invalid_nexus.lane_config);
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context))
                .with_da_proof_policies(Some(proof_policies.clone()));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .with_da_proof_policies(Some(proof_policies))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            let mut view = state.query_view();
            view.nexus = invalid_nexus;
            let err = validate_static_test_block!(&signed, &topology, &view, &time_source)
                .expect_err(
                    "corrupted active elastic range must reject stale elastic execution context",
                );
            assert!(
                matches!(
                    err,
                    BlockValidationError::ExecutionContextInvalid(ref message)
                        if message.contains("routing cannot be reconciled")
                            && message.contains(
                                "committed routing plan differs from the fresh dataspace/role topology"
                            )
                ),
                "unexpected corrupt-range rejection: {err:?}"
            );
        }
        #[test]
        fn validate_and_record_transactions_skip_stateless_matches_full() {
            let (alice_id, alice_keypair) = gen_account_in("wonderland");
            let domain_id: DomainId =
                DomainId::try_new("wonderland", "universal").expect("valid domain");
            let account = Account::new(alice_id.clone()).build(&alice_id);
            let domain = Domain::new(domain_id).build(&alice_id);
            let world = World::with([domain], [account], []);
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_handle);
            let (max_clock_drift, tx_limits) = {
                let state_view = state.world.view();
                let params = state_view.parameters();
                (params.sumeragi().max_clock_drift(), params.transaction())
            };
            let tx = TransactionBuilder::new(
                state.network_id,
                alice_id,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "test".to_string())])
            .sign(alice_keypair.private_key());
            let crypto_cfg = state.crypto();
            let (_clock, time_source) = TimeSource::new_mock(tx.creation_time());
            let tx = AcceptedTransaction::accept_with_time_source(
                tx,
                &state.network_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
                &time_source,
            )
            .expect("valid tx");
            state
                .seed_genesis_for_testing()
                .expect("authenticate ordinary fixture predecessor");
            let new_block = BlockBuilder::new(vec![tx.clone()])
                .chain(0, state.view().latest_block().as_deref())
                .sign(alice_keypair.private_key())
                .unpack(|_| {});
            let mut full_block: SignedBlock = new_block.clone().into();
            let mut state_block = state.block(full_block.header());
            ValidBlock::execute_and_record_canonical_outputs(
                &mut full_block,
                &mut state_block,
                None,
                None,
            )
            .expect("full validation should attach transaction results");
            let full_results: Vec<_> = full_block
                .output_results()
                .map(|result| result.as_ref().is_ok())
                .collect();
            drop(state_block);
            let mut skip_block: SignedBlock = new_block.into();
            let mut state_block = state.block(skip_block.header());
            ValidBlock::execute_and_record_canonical_outputs(
                &mut skip_block,
                &mut state_block,
                None,
                None,
            )
            .expect("skip-stateless validation should attach transaction results");
            let skip_results: Vec<_> = skip_block
                .output_results()
                .map(|result| result.as_ref().is_ok())
                .collect();
            assert_eq!(full_results, skip_results);
        }
        #[test]
        fn validate_keep_voting_block_rejects_stale_geometry_da_commitment_lane() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let stale_lane = LaneId::new(1);
            let stale_geometry_catalog = LaneCatalog::new(
                nonzero!(2_u32),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: stale_lane,
                        alias: "stale-da-commitment".to_owned(),
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("stale derived geometry catalog");
            {
                let mut nexus = state.nexus.write();
                nexus.lane_catalog = LaneCatalog::default();
                nexus.lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(
                    &stale_geometry_catalog,
                );
            }
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let record = DaCommitmentRecord::new(
                stale_lane,
                1,
                1,
                BlobDigest::new([0xAA; 32]),
                ManifestDigest::new([0xBB; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCC; 32]),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEE; 32]),
                checked_da_ack_signature(0x11),
            );
            let builder = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_commitments(Some(DaCommitmentBundle::new(vec![record])));
            let signed: SignedBlock = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {})
                .into();
            validate_signed_voting_test_block!(
                signed,
                topology,
                state,
                time_source,
                result,
                validator_keys,
                Duration::from_millis(1)
            );
            let Err((_, err)) = result else {
                panic!("expected stale-geometry DA commitment rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::DaCommitmentBundle(DaCommitmentValidationError::ProofPolicy(
                    crate::da::DaProofPolicyError::UnknownLane { lane }
                )) if *lane == stale_lane
            ));
        }
        #[test]
        fn validate_keep_voting_block_rejects_stale_geometry_da_pin_intent_lane() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let stale_lane = LaneId::new(1);
            let stale_geometry_catalog = LaneCatalog::new(
                nonzero!(2_u32),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: stale_lane,
                        alias: "stale-da-pin-intent".to_owned(),
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("stale derived geometry catalog");
            {
                let mut nexus = state.nexus.write();
                nexus.lane_catalog = LaneCatalog::default();
                nexus.lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(
                    &stale_geometry_catalog,
                );
            }
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let intent = test_da_pin_intent(
                stale_lane,
                1,
                1,
                StorageTicketId::new([0xAA; 32]),
                ManifestDigest::new([0xBB; 32]),
            );
            let signed: SignedBlock =
                BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                    .chain(0, state.view().latest_block().as_deref())
                    .with_da_pin_intents(Some(DaPinIntentBundle::new(vec![intent])))
                    .sign(leader.private_key())
                    .unpack(|_| {})
                    .into();
            validate_signed_voting_test_block!(
                signed,
                topology,
                state,
                time_source,
                result,
                validator_keys,
                Duration::from_millis(1)
            );
            let Err((_, err)) = result else {
                panic!("expected stale-geometry DA pin-intent rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::DaPinIntentBundle(DaPinIntentValidationError::UnknownLane {
                    lane
                }) if *lane == stale_lane
            ));
        }
        #[test]
        fn validate_keep_voting_block_rejects_future_created_autoscale_da_pin_intent_lane() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let future_created_lane = LaneId::new(1);
            let mut elastic_lane = LaneConfig {
                id: future_created_lane,
                alias: "elastic-lane-1".to_owned(),
                ..LaneConfig::default()
            };
            elastic_lane
                .metadata
                .insert("autoscale.managed".to_owned(), "true".to_owned());
            elastic_lane
                .metadata
                .insert("autoscale.created_height".to_owned(), "7".to_owned());
            crate::state::attach_synthetic_autoscale_committee_for_test(&mut elastic_lane);
            {
                let mut nexus = state.nexus.write();
                nexus.autoscale.enabled = true;
                nexus.autoscale.min_lane_id = nonzero!(1_u32);
                nexus.autoscale.max_lane_id_exclusive = nonzero!(3_u32);
                nexus.lane_catalog =
                    LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), elastic_lane])
                        .expect("future-created autoscale lane catalog");
                nexus.lane_config =
                    iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
            }
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let intent = test_da_pin_intent(
                future_created_lane,
                1,
                1,
                StorageTicketId::new([0xBC; 32]),
                ManifestDigest::new([0xBD; 32]),
            );
            let signed: SignedBlock =
                BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                    .chain(0, state.view().latest_block().as_deref())
                    .with_da_pin_intents(Some(DaPinIntentBundle::new(vec![intent])))
                    .sign(leader.private_key())
                    .unpack(|_| {})
                    .into();
            assert!(
                signed.header().height().get() < 7,
                "fixture block must precede the autoscale lane creation height"
            );
            validate_signed_voting_test_block!(
                signed,
                topology,
                state,
                time_source,
                result,
                validator_keys,
                Duration::from_millis(1)
            );
            let Err((_, err)) = result else {
                panic!("expected future-created autoscale DA pin-intent rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::DaPinIntentBundle(DaPinIntentValidationError::UnknownLane {
                    lane
                }) if *lane == future_created_lane
            ));
        }
        #[test]
        fn validate_keep_voting_block_enforces_consensus_da_ingest_quota() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            insert_test_da_owner(&mut world);
            let state = State::new_with_chain_and_network_id_for_testing(
                world,
                Arc::clone(&kura),
                query,
                iroha_model_base::chain::ChainId::from("da-consensus-quota-test"),
                test_da_network_id(),
            );
            state.nexus.write().da.ingest_quota_max_count_per_account =
                std::num::NonZeroU64::new(1).expect("non-zero quota fixture");
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            install_test_da_admission_policy(&state, 1);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let bundle = DaPinIntentBundle::new(vec![
                test_da_pin_intent(
                    LaneId::new(0),
                    1,
                    1,
                    StorageTicketId::new([0xC1; 32]),
                    ManifestDigest::new([0xD1; 32]),
                ),
                test_da_pin_intent(
                    LaneId::new(0),
                    1,
                    2,
                    StorageTicketId::new([0xC2; 32]),
                    ManifestDigest::new([0xD2; 32]),
                ),
            ]);
            let signed: SignedBlock =
                BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                    .chain(0, state.view().latest_block().as_deref())
                    .with_da_pin_intents(Some(bundle))
                    .sign(leader.private_key())
                    .unpack(|_| {})
                    .into();
            validate_signed_voting_test_block!(
                signed,
                topology,
                state,
                time_source,
                result,
                validator_keys,
                Duration::from_millis(1)
            );
            let Err((_, err)) = result else {
                panic!("expected consensus DA ingest quota rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::DaPinIntentBundle(
                    DaPinIntentValidationError::QuotaExceeded {
                        count: 2,
                        max_count: 1,
                        ..
                    }
                )
            ));
        }
        #[test]
        fn validate_keep_voting_block_rejects_committed_da_pin_intent_identity_reuse() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            let da_owner = iroha_data_model::account::AccountId::new(
                test_da_owner_keypair().public_key().clone(),
            );
            world.accounts.insert(
                da_owner.clone(),
                iroha_data_model::account::AccountValue::new(
                    iroha_data_model::account::AccountDetails::default(),
                ),
            );
            let committed_intent = test_da_pin_intent(
                LaneId::new(0),
                1,
                1,
                StorageTicketId::new([0xA1; 32]),
                ManifestDigest::new([0xB1; 32]),
            );
            let committed_with_location =
                iroha_data_model::da::pin_intent::DaPinIntentWithLocation {
                    intent: committed_intent.clone(),
                    location: iroha_data_model::da::commitment::DaCommitmentLocation {
                        block_height: 1,
                        index_in_bundle: 0,
                    },
                };
            world
                .da_pin_intents_by_ticket
                .insert(committed_intent.storage_ticket, committed_with_location);
            world.da_pin_intents_by_manifest.insert(
                committed_intent.manifest_hash,
                committed_intent.storage_ticket,
            );
            world.da_pin_intents_by_lane_epoch.insert(
                (
                    committed_intent.lane_id,
                    committed_intent.epoch,
                    committed_intent.sequence,
                ),
                committed_intent.storage_ticket,
            );
            let state = State::new_with_chain_and_network_id_for_testing(
                world,
                Arc::clone(&kura),
                query,
                iroha_model_base::chain::ChainId::from("da-replay-test"),
                test_da_network_id(),
            );
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            install_test_da_admission_policy(&state, 1);
            let validate_candidate = |intent: DaPinIntent,
                                      now: Duration|
             -> Box<BlockValidationError> {
                let (_handle, time_source) = TimeSource::new_mock(now);
                let signed: SignedBlock =
                    BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                        .chain(0, state.view().latest_block().as_deref())
                        .with_da_pin_intents(Some(DaPinIntentBundle::new(vec![intent])))
                        .sign(leader.private_key())
                        .unpack(|_| {})
                        .into();
                let (_handle, time_source) = TimeSource::new_mock(signed.header().creation_time());
                let result = validate_voting_test_block!(
                    signed,
                    &topology,
                    &time_source,
                    &state,
                    &validator_keys,
                    now
                )
                .unpack(|_| {});
                let Err((_, err)) = result else {
                    panic!("expected DA pin-intent committed identity rejection");
                };
                err
            };
            let duplicate_ticket = test_da_pin_intent(
                LaneId::new(0),
                1,
                2,
                committed_intent.storage_ticket,
                ManifestDigest::new([0xB2; 32]),
            );
            let err = validate_candidate(duplicate_ticket, Duration::from_millis(2));
            assert!(
                matches!(
                    err.as_ref(),
                    BlockValidationError::DaPinIntentBundle(
                        DaPinIntentValidationError::DuplicateStorageTicket {
                            lane,
                            epoch: 1,
                            sequence: 2
                        }
                    ) if *lane == LaneId::new(0)
                ),
                "unexpected committed ticket reuse error: {err:?}"
            );
            let duplicate_manifest = test_da_pin_intent(
                LaneId::new(0),
                1,
                3,
                StorageTicketId::new([0xA3; 32]),
                committed_intent.manifest_hash,
            );
            let err = validate_candidate(duplicate_manifest, Duration::from_millis(3));
            assert!(
                matches!(
                    err.as_ref(),
                    BlockValidationError::DaPinIntentBundle(
                        DaPinIntentValidationError::DuplicateManifest {
                            lane,
                            epoch: 1,
                            sequence: 3
                        }
                    ) if *lane == LaneId::new(0)
                ),
                "unexpected committed manifest reuse error: {err:?}"
            );
        }
        #[test]
        fn validate_keep_voting_block_rejects_da_cursor_regression() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let advance = DaCommitmentRecord::new(
                LaneId::new(0),
                2,
                3,
                BlobDigest::new([0xAB; 32]),
                ManifestDigest::new([0xBC; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCD; 32]),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEF; 32]),
                checked_da_ack_signature(0x12),
            );
            state
                .ensure_da_indexes_hydrated()
                .expect("DA indexes hydrate for cursor regression test");
            {
                let shard_id = state.nexus_snapshot().lane_config.shard_id(advance.lane_id);
                state
                    .da_shard_cursors
                    .write()
                    .advance(shard_id, &advance, 1)
                    .expect("initial cursor advance");
            }
            {
                let cursors = state.da_shard_cursor_index();
                let cursor = cursors.get(0, advance.lane_id).expect("cursor seeded");
                assert_eq!((cursor.epoch, cursor.sequence), (2, 3));
            }
            let regression = DaCommitmentRecord::new(
                LaneId::new(0),
                2,
                2,
                BlobDigest::new([0xAA; 32]),
                ManifestDigest::new([0xBB; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCC; 32]),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEE; 32]),
                checked_da_ack_signature(0x13),
            );
            let bundle = DaCommitmentBundle::new(vec![regression]);
            let builder = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_commitments(Some(bundle));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();
            validate_signed_voting_test_block!(
                signed,
                topology,
                state,
                time_source,
                result,
                validator_keys,
                Duration::from_millis(2)
            );
            let Err((_, err)) = result else {
                panic!("expected DA shard cursor regression rejection");
            };
            assert!(
                matches!(
                    err.as_ref(),
                    BlockValidationError::DaShardCursor(DaShardCursorError::Regression { .. })
                ),
                "unexpected error: {err:?}"
            );
        }
        #[test]
        fn validate_keep_voting_block_rejects_expired_consensus_keys() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let proxy_tail = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let validator_keys = vec![
                leader.clone(),
                proxy_tail.clone(),
                checked_keypair_with_algorithm(Algorithm::BlsNormal),
                checked_keypair_with_algorithm(Algorithm::BlsNormal),
            ];
            let topology = test_topology_with_keys(&validator_keys);
            let mut params = Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 0;
            params.sumeragi.key_expiry_grace_blocks = 0;
            let mut world = World::new();
            world.parameters = Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader-expired",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy-expired",
                &proxy_tail,
                0,
                Some(1),
                ConsensusKeyStatus::Active,
            );
            insert_active_consensus_keys(&mut world, &validator_keys[2..]);
            let state = State::new(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let prev_block = state.view().latest_block().expect("previous block");
            let mut signed: SignedBlock = with_current_state_da_sidecars(
                BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                    .chain(0, Some(prev_block.as_ref())),
                &state,
            )
            .sign(leader.private_key())
            .unpack(|_| {})
            .into();
            let block_hash = signed.hash();
            let proxy_idx = topology
                .position(proxy_tail.public_key())
                .expect("proxy tail in topology");
            signed
                .add_signature(BlockSignature::new(
                    proxy_idx as u64,
                    checked_block_signature(proxy_tail.private_key(), block_hash),
                ))
                .expect("proxy tail signature");
            assert_eq!(signed.external_transactions().count(), 0);
            let result = validate_voting_test_block!(
                signed,
                &topology,
                &time_source,
                &state,
                &validator_keys,
                Duration::from_millis(2)
            )
            .unpack(|_| {});
            let Err((_, err)) = result else {
                panic!("expected expired consensus key rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::SignatureVerification(
                    SignatureVerificationError::InactiveConsensusKey
                )
            ));
        }
        #[test]
        fn validate_keep_voting_block_allows_overlap_grace_window() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let proxy_tail = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let validator_keys = vec![
                leader.clone(),
                proxy_tail.clone(),
                checked_keypair_with_algorithm(Algorithm::BlsNormal),
                checked_keypair_with_algorithm(Algorithm::BlsNormal),
            ];
            let topology = test_topology_with_keys(&validator_keys);
            let mut params = Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 1;
            params.sumeragi.key_expiry_grace_blocks = 0;
            let mut world = World::new();
            world.parameters = Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader-overlap",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy-overlap",
                &proxy_tail,
                0,
                Some(2),
                ConsensusKeyStatus::Retiring,
            );
            insert_active_consensus_keys(&mut world, &validator_keys[2..]);
            // Signature grace authenticates the bound global roster. A newly selected lane
            // committee must use active keys, so replace its retiring member explicitly.
            let lane_replacement = checked_keypair_with_algorithm(Algorithm::BlsNormal);
            insert_consensus_key(
                &mut world,
                "lane-replacement",
                &lane_replacement,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let lane_keys = vec![
                validator_keys[0].clone(),
                validator_keys[2].clone(),
                validator_keys[3].clone(),
                lane_replacement,
            ];
            let state = State::new(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &lane_keys);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let (authority, signer) = gen_account_in("overlap-grace");
            let transaction = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "overlap-grace".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
            let prev_block = state.view().latest_block().expect("previous block");
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, Some(prev_block.as_ref()));
            let mut signed: SignedBlock = with_current_state_da_sidecars(builder, &state)
                .sign(leader.private_key())
                .unpack(|_| {})
                .into();
            let block_hash = signed.hash();
            let proxy_idx = topology
                .position(proxy_tail.public_key())
                .expect("proxy tail in topology");
            signed
                .add_signature(BlockSignature::new(
                    proxy_idx as u64,
                    checked_block_signature(proxy_tail.private_key(), block_hash),
                ))
                .expect("proxy tail signature");
            assert_eq!(signed.external_transactions().count(), 1);
            let result = validate_voting_test_block!(
                signed,
                &topology,
                &time_source,
                &state,
                &validator_keys,
                Duration::from_millis(2)
            )
            .unpack(|_| {});
            if let Err((_, err)) = result {
                panic!("overlap grace should permit signatures at expiry height, got {err:?}");
            }
        }
        #[test]
        fn validate_keep_voting_block_rejects_missing_proxy_tail_key() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let proxy_tail = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let validator_keys = vec![
                leader.clone(),
                proxy_tail.clone(),
                checked_keypair_with_algorithm(Algorithm::BlsNormal),
                checked_keypair_with_algorithm(Algorithm::BlsNormal),
            ];
            let topology = test_topology_with_keys(&validator_keys);
            let mut params = Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 0;
            params.sumeragi.key_expiry_grace_blocks = 0;
            let mut world = World::new();
            world.parameters = Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader-only",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            // Deliberately omit the proxy-tail consensus key to exercise the missing-key path.
            insert_active_consensus_keys(&mut world, &validator_keys[2..]);
            let state = State::new(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let mut candidate =
                ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                    header.set_height(nonzero!(2_u64));
                    header.set_prev_block_hash(Some(prev_hash));
                    header.creation_time_ms = 1;
                });
            candidate.sign(&proxy_tail, &topology);
            let mut signed: SignedBlock = candidate.into();
            signed.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
                &state.nexus_snapshot(),
                2,
            )));
            let signed = with_current_state_confidential_features(
                signed,
                &state,
                &[(0, leader.private_key()), (1, proxy_tail.private_key())],
            );
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let result = validate_voting_test_block!(
                signed,
                &topology,
                &time_source,
                &state,
                &validator_keys,
                Duration::from_millis(1)
            )
            .unpack(|_| {});
            let Err((_, err)) = result else {
                panic!("expected missing proxy-tail consensus key rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::SignatureVerification(
                    SignatureVerificationError::InactiveConsensusKey
                )
            ));
        }
        #[test]
        fn maps_signature_verification_errors() {
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::NotEnoughSignatures {
                    votes_count: 1,
                    min_votes_for_commit: 2
                }),
                Reason::InsufficientBlockSignatures
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::UnknownSignatory),
                Reason::UnknownBlockSignatory
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::DuplicateSignature {
                    signer: 0
                }),
                Reason::InvalidBlockSignature
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::UnknownSignature),
                Reason::InvalidBlockSignature
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::MissingPop),
                Reason::InvalidBlockSignature
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::ProxyTailMissing),
                Reason::ProxyTailSignatureMissing
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::LeaderMissing),
                Reason::LeaderSignatureMissing
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::Other),
                Reason::OtherSignatureError
            );
        }
        /// Check quorum requirement when proxy tail is missing.
        #[test]
        fn signature_verification_rejects_insufficient_quorum_without_proxy_tail() {
            let key_pairs = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(7)
            .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            key_pairs
                .iter()
                .enumerate()
                // Include only peers in validator set
                .take(topology.min_votes_for_commit())
                // Skip leader since already singed
                .skip(1)
                .filter(|(i, _)| *i != 4) // Skip proxy tail
                .map(|(i, key_pair)| {
                    BlockSignature::new(
                        i as u64,
                        checked_block_signature(key_pair.private_key(), block_hash),
                    )
                })
                .try_for_each(|signature| block.add_signature(signature, &topology))
                .expect("Failed to add signatures");
            let err = block.commit(&topology).unpack(|_| {}).unwrap_err().1;
            assert_eq!(
                err.as_ref(),
                &BlockValidationError::SignatureVerification(
                    SignatureVerificationError::NotEnoughSignatures {
                        votes_count: topology.min_votes_for_commit() - 1,
                        min_votes_for_commit: topology.min_votes_for_commit(),
                    }
                )
            );
        }
        #[test]
        fn maps_block_validation_errors() {
            for reason in [
                ivm::error::ExecutionDeferral::AllocationUnavailable,
                ivm::error::ExecutionDeferral::ActiveMemoryCapacity,
            ] {
                assert_eq!(
                    map_block_err_to_reason(&BlockValidationError::ExecutionDeferred(
                        reason.into()
                    )),
                    None
                );
            }
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::MerkleRootMismatch),
                Some(Reason::MerkleRootMismatch)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::EmptyBlock),
                Some(Reason::EmptyBlock)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::DuplicateTransactions),
                Some(Reason::TransactionValidationFailed)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::TooManyTransactions {
                    actual: 2,
                    max: 1
                }),
                Some(Reason::TransactionValidationFailed)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::SignatureVerification(
                    SignatureVerificationError::LeaderMissing
                )),
                Some(Reason::LeaderSignatureMissing)
            );
            let network_id = |seed| {
                NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                    Hash::prehashed([seed; Hash::LENGTH]),
                ))
            };
            let chain_mismatch = BlockValidationError::TransactionAccept(
                AcceptTransactionFail::TransactionDomainMismatch(Mismatch {
                    expected: iroha_data_model::transaction::TransactionDomain::Network(
                        network_id(0xA1),
                    ),
                    actual: iroha_data_model::transaction::TransactionDomain::Network(network_id(
                        0xB2,
                    )),
                }),
            );
            assert_eq!(
                map_block_err_to_reason(&chain_mismatch),
                Some(Reason::TransactionValidationFailed)
            );
            let tx_limit = BlockValidationError::TransactionAccept(
                AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                    reason: "too big".into(),
                }),
            );
            assert_eq!(
                map_block_err_to_reason(&tx_limit),
                Some(Reason::TransactionValidationFailed)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::InvalidGenesis(
                    InvalidGenesisError::ContainsErrors
                )),
                Some(Reason::InvalidGenesis)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::ConfidentialFeaturesMismatch {
                    expected: None,
                    actual: None
                }),
                Some(Reason::ConfidentialFeatureDigestMismatch)
            );
            let policy_err = BlockValidationError::ProofPolicyHashMismatch {
                expected: HashOf::from_untyped_unchecked(Hash::prehashed([1; Hash::LENGTH])),
                actual: None,
            };
            assert_eq!(
                map_block_err_to_reason(&policy_err),
                Some(Reason::DaProofPolicyMismatch)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::V2FinalityAuthorityInvalid(
                    "certificate does not bind the canonical execution".to_owned(),
                )),
                Some(Reason::ConsensusBlockRejection)
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::SnapshotBootstrapParentInvalid(
                    "snapshot parent body is unexpectedly available".to_owned(),
                )),
                Some(Reason::ConsensusBlockRejection)
            );
        }
        #[test]
        fn maps_transaction_future_error() {
            let err = BlockValidationError::TransactionInTheFuture;
            assert_eq!(
                map_block_err_to_reason(&err),
                Some(Reason::TransactionInTheFuture)
            );
        }
        #[test]
        fn v2_validation_is_wall_clock_independent_and_uses_height_context_for_reconfiguration() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let leader_private = leader.private_key().clone();
            let first =
                commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 1);
            let _second = commit_block_at_height(
                &state,
                &kura,
                &topology,
                &leader_private,
                2,
                Some(first),
                2,
            );
            let candidate_at = |creation_time_ms: u64, label: &str| {
                let (_clock, candidate_time) =
                    TimeSource::new_mock(Duration::from_millis(creation_time_ms));
                let transaction_time = TimeSource::new_fixed(Duration::from_millis(999_999));
                let (authority, signer) = gen_account_in(label);
                let transaction = TransactionBuilder::new_with_time_source(
                    state.network_id,
                    authority,
                    &transaction_time,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(Level::INFO, label.to_owned())])
                .sign(signer.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
                let parent = state.view().latest_block().expect("audited v2 parent");
                with_current_state_da_sidecars(
                    BlockBuilder::new_with_time_source(vec![accepted], candidate_time)
                        .chain(0, Some(parent.as_ref())),
                    &state,
                )
                .sign(&leader_private)
                .unpack(|_| {})
                .into()
            };
            let candidate: SignedBlock = candidate_at(1_000_000, "v2-wall-clock-work");
            let (_clock, local_time) = TimeSource::new_mock(Duration::ZERO);
            let v2 = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
                candidate.clone(),
                &topology,
                &ALICE_ID,
                &local_time,
                Duration::from_millis(999_998),
                SumeragiV2ValidationContext::from_height_context(
                    &authenticated_permissioned_successor_context(&state, &validator_keys),
                ),
                &state,
            )
            .unpack(|_| {});
            let (valid, staged) = v2.expect(
                "v2 validation must depend on parent time/context, not a validator's wall clock",
            );
            assert_eq!(valid.as_ref().external_transactions().count(), 1);
            assert!(
                valid
                    .as_ref()
                    .network_output_at(0)
                    .is_some_and(|(_, output)| output.result.is_err())
            );
            drop(staged);
            let noncanonical: SignedBlock = candidate_at(1_000_001, "v2-noncanonical-time-work");
            let rejected = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
                noncanonical.clone(),
                &topology,
                &ALICE_ID,
                &local_time,
                Duration::from_millis(999_998),
                SumeragiV2ValidationContext::from_height_context(
                    &authenticated_permissioned_successor_context(&state, &validator_keys),
                ),
                &state,
            )
            .unpack(|_| {});
            let error = match rejected {
                Err(error) => error,
                Ok(_) => panic!("non-canonical V2 block time must be rejected"),
            };
            assert!(
                matches!(
                    *error.1,
                    BlockValidationError::NonCanonicalV2BlockTime {
                        expected_ms: 1_000_000,
                        actual_ms: 1_000_001,
                    }
                ),
                "expected non-canonical V2 block time rejection, got {:?}",
                error.1
            );
        }
        #[test]
        fn v2_snapshot_parent_enforces_authenticated_hash_height_and_logical_time() {
            setup_da_validation_world!(kura, query, leader, topology, world, validator_keys);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let first =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let second = commit_block_at_height(
                &state,
                &kura,
                &topology,
                leader.private_key(),
                2,
                Some(first),
                2,
            );
            let audited_parent = state
                .view()
                .latest_block()
                .expect("snapshot parent is available before hash-only conversion");
            assert_eq!(audited_parent.hash(), second);
            kura.force_hash_only_block_for_testing(nonzero!(2_usize))
                .expect("remove audited parent body");
            assert!(state.view().latest_block().is_none());
            let anchor = consensus_v2::SnapshotBootstrapAnchor {
                snapshot_height: 2,
                snapshot_block_hash: second,
                snapshot_block_creation_time_ms: 2,
                snapshot_state_hash: crate::snapshot::canonical_state_snapshot_hash(&state)
                    .expect("stable valid fixture snapshot"),
            };
            let roster_peers = topology.as_ref().to_vec();
            let roster = roster_peers
                .into_iter()
                .map(|validator| consensus_v2::ValidatorPower {
                    validator,
                    power: 1,
                })
                .collect::<Vec<_>>();
            let network_id = *state.network_id_ref();
            let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                    network_id,
                    u64::MAX,
                    &roster,
                );
            let context = consensus_v2::HeightContext {
                network_id,
                protocol_version: consensus_v2::PROTOCOL_VERSION,
                height: 3,
                epoch: 0,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: consensus_v2::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: Some(anchor),
                quorum: consensus_v2::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                kagemusha_mint_finality_authorization,
                kagemusha_mint_finality_authority,
                nexus_amx_context_hash: Hash::new(b"snapshot validation Nexus/AMX"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: consensus_v2::DataAvailabilityLayout {
                    encoding: consensus_v2::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
                leader_seed: [0x51; 32],
            };
            context.validate().expect("valid snapshot context");
            let candidate_at = |creation_time_ms: u64| {
                let (_clock, candidate_time) =
                    TimeSource::new_mock(Duration::from_millis(creation_time_ms));
                let transaction_time = TimeSource::new_fixed(Duration::from_millis(11));
                let (authority, signer) = gen_account_in("v2-snapshot-parent-work");
                let transaction = TransactionBuilder::new_with_time_source(
                    state.network_id,
                    authority,
                    &transaction_time,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(Level::INFO, "v2-snapshot-parent-work".to_owned())])
                .sign(signer.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
                with_current_state_da_sidecars(
                    BlockBuilder::new_with_time_source(vec![accepted], candidate_time)
                        .chain(0, Some(audited_parent.as_ref())),
                    &state,
                )
                .sign(leader.private_key())
                .unpack(|_| {})
                .into()
            };
            let validate = |candidate: SignedBlock, context: &consensus_v2::HeightContext| {
                ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
                    candidate,
                    &topology,
                    &ALICE_ID,
                    &TimeSource::new_system(),
                    Duration::from_millis(10),
                    SumeragiV2ValidationContext::from_height_context(context),
                    &state,
                )
                .unpack(|_| {})
            };
            let (valid, staged) = validate(candidate_at(12), &context)
                .expect("exact anchor time plus cadence is accepted");
            assert_eq!(valid.as_ref().external_transactions().count(), 1);
            assert!(
                valid
                    .as_ref()
                    .network_output_at(0)
                    .is_some_and(|(_, output)| output.result.is_err())
            );
            drop(staged);
            assert!(matches!(
                validate(candidate_at(13), &context),
                Err(error)
                    if matches!(
                        *error.1,
                        BlockValidationError::NonCanonicalV2BlockTime {
                            expected_ms: 12,
                            actual_ms: 13,
                        }
                    )
            ));
            let mut wrong_hash_context = context;
            wrong_hash_context
                .snapshot_bootstrap
                .as_mut()
                .expect("fixture anchor")
                .snapshot_block_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"wrong snapshot parent"));
            assert!(matches!(
                validate(candidate_at(12), &wrong_hash_context),
                Err(error)
                    if matches!(
                        *error.1,
                        BlockValidationError::SnapshotBootstrapParentInvalid(_)
                    )
            ));
        }
        #[test]
        fn validate_keep_voting_block_rejects_forged_committed_fragment_count() {
            setup_stateless_cache_state!(kura, state, leader_private, topology, validator_keys);
            let prev_committed = state.view().latest_block().expect("fixture parent");
            let (authority, signer) = gen_account_in("wonderland");
            // This count attack requires one actual applied transaction; an
            // absent authority would reject without any economic fragment.
            let (account_id, account_value) = iroha_data_model::IntoKeyValue::into_key_value(
                Account::new(authority.clone()).build(&authority),
            );
            state.world.accounts.insert(account_id, account_value);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));
            let tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "forged-fragment-count".to_owned())])
            .sign(signer.private_key());
            let entry_hash = tx.hash_as_entrypoint();
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone());
            let builder = builder.chain(0, Some(prev_committed.as_ref()));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(&leader_private)
                .unpack(|_| {});
            let mut signed_block: SignedBlock = SignedBlock::from(new_block);
            signed_block
                .set_execution_outputs(
                    crate::execution_output_test_support::structural_network_outputs(
                        &signed_block,
                        &[entry_hash],
                        vec![
                            iroha_data_model::transaction::signed::TransactionResultInner::Ok(
                                iroha_data_model::trigger::DataTriggerSequence::default(),
                            ),
                        ],
                    ),
                    99,
                    BTreeMap::new(),
                    Vec::new(),
                    AxtPolicySnapshot::default(),
                    BTreeSet::new(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("fixture result roots match external entrypoint");
            let result = validate_voting_test_block!(
                signed_block,
                &topology,
                &time_source,
                &state,
                &validator_keys,
                Duration::from_millis(10)
            )
            .unpack(|_| {});
            let Err((_, err)) = result else {
                panic!("forged committed fragment counts must be rejected");
            };
            assert!(
                matches!(
                    err.as_ref(),
                    BlockValidationError::CommittedFragmentCountMismatch {
                        expected: 1,
                        actual: 99,
                    }
                ),
                "the actual applied transaction determines fragment count: {err:?}"
            );
        }
        #[test]
        fn advertised_zero_committed_fragment_count_is_rejected() {
            setup_stateless_cache_state!(kura, state, leader_private, topology, validator_keys);
            let prev_committed = state.view().latest_block().expect("fixture parent");
            let (authority, signer) = gen_account_in("wonderland");
            // This count attack requires one actual applied transaction; an
            // absent authority would reject without any economic fragment.
            let (account_id, account_value) = iroha_data_model::IntoKeyValue::into_key_value(
                Account::new(authority.clone()).build(&authority),
            );
            state.world.accounts.insert(account_id, account_value);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));
            let tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                "zero-fragment-count-mismatch".to_owned(),
            )])
            .sign(signer.private_key());
            let entry_hash = tx.hash_as_entrypoint();
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone());
            let builder = builder.chain(0, Some(prev_committed.as_ref()));
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(&leader_private)
                .unpack(|_| {});
            let mut signed_block: SignedBlock = SignedBlock::from(new_block);
            signed_block
                .set_execution_outputs(
                    crate::execution_output_test_support::structural_network_outputs(
                        &signed_block,
                        &[entry_hash],
                        vec![
                            iroha_data_model::transaction::signed::TransactionResultInner::Ok(
                                iroha_data_model::trigger::DataTriggerSequence::default(),
                            ),
                        ],
                    ),
                    0,
                    BTreeMap::new(),
                    Vec::new(),
                    AxtPolicySnapshot::default(),
                    BTreeSet::new(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("fixture result roots match external entrypoint");
            assert_eq!(signed_block.committed_fragment_count(), Some(0));
            let result = validate_voting_test_block!(
                signed_block,
                &topology,
                &time_source,
                &state,
                &validator_keys,
                Duration::from_millis(10)
            )
            .unpack(|_| {});
            let Err((_, error)) = result else {
                panic!("advertised zero must not normalize to the executed fragment count");
            };
            assert!(matches!(
                error.as_ref(),
                BlockValidationError::CommittedFragmentCountMismatch {
                    expected: 1,
                    actual: 0,
                }
            ));
        }
        #[derive(Clone, Copy)]
        enum QueuePlanTtlBindingFixture {
            Missing,
            Exact,
            Conflict,
            Stale,
        }
        struct QueuePlanTtlFixture {
            state: State,
            topology: Topology,
            block_time_source: TimeSource,
            block: SignedBlock,
            stateless_cache_key: crate::tx::StatelessValidationCacheKey,
        }
        #[allow(clippy::too_many_arguments)]
        fn queue_plan_ttl_fixture(
            label: &str,
            intent: iroha_data_model::transaction::TransactionAdmissionIntent,
            binding_fixture: QueuePlanTtlBindingFixture,
            creation_time_ms: u64,
            ttl_ms: u64,
            enqueue_timestamp_ms: u64,
            block_time_ms: u64,
            invalidate_signature: bool,
        ) -> QueuePlanTtlFixture {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let validator_keys = core::iter::repeat_with(|| {
                crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
            })
            .take(4)
            .collect::<Vec<_>>();
            let leader = &validator_keys[0];
            let topology = test_topology_with_keys(&validator_keys);
            let (authority, signer) = gen_account_in(label);
            let domain_id = DomainId::try_new(label, "universal").expect("fixture domain id");
            let account = Account::new(authority.clone()).build(&authority);
            let domain = Domain::new(domain_id).build(&authority);
            let mut world = World::with([domain], [account], []);
            let mut parameters = Parameters::default();
            parameters.set_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            ));
            world.parameters = Cell::new(parameters);
            insert_active_consensus_keys(&mut world, &validator_keys);
            let mut state = State::new_for_testing(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);
            let parent = ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.creation_time_ms = 1;
            });
            let mut parent: SignedBlock = parent.into();
            parent
                .set_execution_outputs(
                    crate::execution_output_test_support::structural_network_outputs(
                        &parent,
                        &[],
                        Vec::new(),
                    ),
                    0,
                    BTreeMap::new(),
                    Vec::new(),
                    AxtPolicySnapshot::default(),
                    BTreeSet::new(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
                .expect("QueuePlan TTL parent carries the canonical empty AXT snapshot");
            let parent = ValidBlock::new_unverified_for_tests(parent)
                .commit_unchecked()
                .unpack(|_| {});
            let predecessor_hash = parent.as_ref().hash();
            {
                let mut state_block = state.block(parent.as_ref().header());
                state_block.block_hashes.push(predecessor_hash);
                state_block.transactions.insert_block(
                    std::collections::HashSet::new(),
                    NonZeroUsize::new(1).expect("parent height is non-zero"),
                );
                state_block
                    .commit()
                    .expect("commit QueuePlan TTL parent metadata");
            }
            kura.store_block(parent)
                .expect("store QueuePlan TTL parent");
            let mut tx_builder = TransactionBuilder::new(
                state.network_id,
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, label.to_owned())])
            .with_admission_intent(intent);
            tx_builder.set_creation_time(Duration::from_millis(creation_time_ms));
            tx_builder.set_ttl(Duration::from_millis(ttl_ms));
            let mut signed = tx_builder.sign(signer.private_key());
            if invalidate_signature {
                let (forged_authority, _) = gen_account_in(&format!("{label}-forged"));
                signed = signed.with_authority(forged_authority);
            }
            let stateless_cache_key = crate::tx::StatelessValidationCacheKey::new(&signed);
            let entrypoint = TransactionEntrypoint::External(signed.clone());
            let routing_plan = crate::queue::RoutingPlan::single(
                crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            );
            if !matches!(binding_fixture, QueuePlanTtlBindingFixture::Missing) {
                let validator_set = state
                    .resolve_lane_committee_at_height(
                        crate::state::LaneAuthorityRoute::new(
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        ),
                        2,
                    )
                    .expect("resolve exact QueuePlan TTL lane authority")
                    .into_validators();
                let lane_incarnation =
                    if matches!(binding_fixture, QueuePlanTtlBindingFixture::Stale) {
                        Hash::new(b"retired-queue-plan-ttl-lane-incarnation")
                    } else {
                        state
                            .lane_incarnation_at_height(LaneId::SINGLE, 2)
                            .expect("default lane is active at candidate height")
                    };
                let admission_context = crate::queue::QueuePlanAdmissionContextV1 {
                    version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
                    authority_height: 1,
                    proposal_height: 2,
                    predecessor_block_hash: Some(predecessor_hash),
                    routing_plan_digest: routing_plan.digest(),
                    route_incarnations: vec![crate::queue::QueuePlanRouteIncarnationV1 {
                        leg: routing_plan.coordinator_leg(),
                        lane_incarnation,
                        validator_set_hash_version:
                            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                        validator_set_hash: HashOf::new(&validator_set),
                        validator_count: u16::try_from(validator_set.len())
                            .expect("fixture validator count fits u16"),
                        durability_threshold: u16::try_from(validator_set.len().div_ceil(3))
                            .expect("fixture threshold fits u16"),
                        validator_set,
                    }],
                };
                let binding = crate::torii_proxy::new_queue_plan_admission_binding(
                    state.network_id_ref(),
                    &entrypoint,
                    &routing_plan,
                    admission_context,
                    enqueue_timestamp_ms,
                )
                .expect("canonical QueuePlan TTL binding");
                state
                    .install_queue_plan_pending_binding_for_test(&binding)
                    .expect("install pending QueuePlan TTL binding");
                if matches!(binding_fixture, QueuePlanTtlBindingFixture::Conflict) {
                    state
                        .replace_queue_plan_registry_owner_for_test(
                            &binding,
                            Hash::new(b"conflicting-queue-plan-ttl-owner"),
                        )
                        .expect("replace exact registry owner for conflict fixture");
                }
            }
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed));
            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(block_time_ms));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone())
                    .chain(0, state.view().latest_block().as_deref());
            let execution_validator_set = state
                .resolve_lane_committee_at_height(
                    crate::state::LaneAuthorityRoute::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                    2,
                )
                .expect("resolve exact execution-context lane authority")
                .into_validators();
            let ownership = sample_lane_payload_ownership_for_context_at_slot(
                2,
                0,
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                state
                    .lane_incarnation_at_height(LaneId::SINGLE, 2)
                    .expect("default lane is active at candidate height"),
                1,
                0,
                vec![0],
                vec![Hash::from(entrypoint.hash())],
                &execution_validator_set,
            );
            let execution_context =
                BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                    entrypoint.hash(),
                    LaneId::SINGLE,
                    DataSpaceId::UNIVERSAL,
                )])
                .with_lane_payload_ownerships(vec![ownership]);
            let block = with_current_state_da_sidecars(
                builder.with_execution_context(Some(execution_context)),
                &state,
            )
            .sign(leader.private_key())
            .unpack(|_| {})
            .into();
            QueuePlanTtlFixture {
                state,
                topology,
                block_time_source,
                block,
                stateless_cache_key,
            }
        }
        fn validate_queue_plan_ttl_fixture(
            fixture: &QueuePlanTtlFixture,
        ) -> Result<(ValidBlock, Box<StateBlock<'_>>), Error> {
            // External QueuePlan roles are rejected before height-context validation.
            validate_voting_test_block!(without_authenticated_context;
                fixture.block.clone(),
                &fixture.topology,
                &fixture.block_time_source,
                &fixture.state,
                Duration::from_millis(1)
            )
            .unpack(|_| {})
        }
        fn assert_external_queue_plan_role_rejected(error: &BlockValidationError) {
            assert!(
                matches!(
                    error,
                    BlockValidationError::ExecutionContextInvalid(message)
                        if message.contains("must use autonomous lane ownership")
                ),
                "unexpected external QueuePlan rejection: {error:?}"
            );
        }
        #[test]
        fn exact_parent_queue_plan_admission_rejects_ordinary_external_execution() {
            use iroha_data_model::transaction::TransactionAdmissionIntent;
            let fixture = queue_plan_ttl_fixture(
                "queue-plan-ordinary-external-follower",
                TransactionAdmissionIntent::QueuePlanSynced,
                QueuePlanTtlBindingFixture::Exact,
                10,
                10,
                10,
                100,
                false,
            );
            let Err((_, error)) = validate_queue_plan_ttl_fixture(&fixture) else {
                panic!("QueuePlanSynced external execution must be rejected before voting");
            };
            assert_external_queue_plan_role_rejected(error.as_ref());
            assert!(
                !fixture
                    .state
                    .stateless_validation_cache()
                    .lock()
                    .contains_key(&fixture.stateless_cache_key),
                "rejected QueuePlan authority must not become a generic cache entry"
            );
        }
        #[test]
        fn unbound_external_queue_plan_rejects_before_block_time_expiry() {
            use iroha_data_model::transaction::TransactionAdmissionIntent;
            let fixture = queue_plan_ttl_fixture(
                "queue-plan-expired-without-binding",
                TransactionAdmissionIntent::QueuePlanSynced,
                QueuePlanTtlBindingFixture::Missing,
                10,
                10,
                10,
                100,
                false,
            );
            let Err((_, error)) = validate_queue_plan_ttl_fixture(&fixture) else {
                panic!("unbound external QueuePlan transaction must be rejected");
            };
            assert_external_queue_plan_role_rejected(error.as_ref());
        }
        #[test]
        fn queue_plan_enqueue_time_must_itself_be_within_signed_ttl() {
            use iroha_data_model::transaction::TransactionAdmissionIntent;
            let fixture = queue_plan_ttl_fixture(
                "queue-plan-expired-before-enqueue",
                TransactionAdmissionIntent::QueuePlanSynced,
                QueuePlanTtlBindingFixture::Exact,
                10,
                10,
                21,
                100,
                false,
            );
            let Err((_, error)) = validate_queue_plan_ttl_fixture(&fixture) else {
                panic!("exact ownership cannot waive expiry at the certified enqueue time");
            };
            assert_external_queue_plan_role_rejected(error.as_ref());
        }
        #[test]
        fn conflicting_or_stale_queue_plan_parent_binding_fails_closed() {
            use iroha_data_model::transaction::TransactionAdmissionIntent;
            for (label, binding_fixture) in [
                (
                    "queue-plan-conflicting-parent-owner",
                    QueuePlanTtlBindingFixture::Conflict,
                ),
                (
                    "queue-plan-stale-parent-owner",
                    QueuePlanTtlBindingFixture::Stale,
                ),
            ] {
                let fixture = queue_plan_ttl_fixture(
                    label,
                    TransactionAdmissionIntent::QueuePlanSynced,
                    binding_fixture,
                    10,
                    10,
                    10,
                    100,
                    false,
                );
                let Err((_, error)) = validate_queue_plan_ttl_fixture(&fixture) else {
                    panic!("non-exact QueuePlan parent authority must fail closed");
                };
                assert_external_queue_plan_role_rejected(error.as_ref());
            }
        }
        #[test]
        fn queue_plan_enqueue_time_still_runs_signature_and_governed_limit_checks() {
            use iroha_data_model::transaction::TransactionAdmissionIntent;
            let invalid_signature = queue_plan_ttl_fixture(
                "queue-plan-invalid-signature",
                TransactionAdmissionIntent::QueuePlanSynced,
                QueuePlanTtlBindingFixture::Exact,
                10,
                10,
                10,
                100,
                true,
            );
            let Err((_, error)) = validate_queue_plan_ttl_fixture(&invalid_signature) else {
                panic!("QueuePlan time authority must not bypass signature validation");
            };
            assert_external_queue_plan_role_rejected(error.as_ref());

            let max_ttl_ms = invalid_signature
                .state
                .view()
                .world()
                .parameters()
                .transaction()
                .max_time_to_live_ms()
                .get();
            let invalid_limit = queue_plan_ttl_fixture(
                "queue-plan-invalid-governed-ttl",
                TransactionAdmissionIntent::QueuePlanSynced,
                QueuePlanTtlBindingFixture::Exact,
                10,
                max_ttl_ms.saturating_add(1),
                10,
                100,
                false,
            );
            let Err((_, error)) = validate_queue_plan_ttl_fixture(&invalid_limit) else {
                panic!("QueuePlan time authority must not bypass governed limits");
            };
            assert_external_queue_plan_role_rejected(error.as_ref());
        }
        #[test]
        fn transaction_signature_validation_has_no_bypass_terms() {
            let needles = [
                ["signature", "_", "override"].concat(),
                ["signature", "_", "overrides"].concat(),
                ["skip", "_tx", "_signature", "_validation"].concat(),
                ["trust", "_replay", "_tx", "_signatures"].concat(),
                ["cached", "_stateless", "_ok"].concat(),
            ];
            let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
            let mut pending = vec![src.clone()];
            let mut hits = Vec::new();
            while let Some(path) = pending.pop() {
                let metadata = std::fs::metadata(&path).expect("source path metadata");
                if metadata.is_dir() {
                    for entry in std::fs::read_dir(&path).expect("source directory readable") {
                        pending.push(entry.expect("source directory entry").path());
                    }
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }
                let source = std::fs::read_to_string(&path).expect("Rust source readable");
                for needle in &needles {
                    if source.contains(needle) {
                        let relative = path.strip_prefix(&src).unwrap_or(&path);
                        hits.push(format!("{} contains {needle}", relative.display()));
                    }
                }
            }
            assert!(
                hits.is_empty(),
                "forbidden source terms:\n{}",
                hits.join("\n")
            );
        }
        #[test]
        fn sumeragi_v2_fixture_with_events_and_timing_populates_stateless_cache() {
            setup_stateless_cache_state!(kura, state, leader_private, topology, validator_keys);
            setup_cacheable_transaction!(state, _tx_handle, tx_time_source, tx_hash, accepted);
            build_cacheable_block!(
                state,
                leader_private,
                accepted,
                _block_handle,
                block_time_source,
                signed_block
            );
            let mut events = Vec::new();
            let mut timings = ValidationTimings::new();
            let height_context =
                authenticated_permissioned_successor_context(&state, &validator_keys);
            let result = ValidBlock::validate_sumeragi_v2_fixture_with_events_and_timing(
                signed_block,
                &topology,
                &ALICE_ID,
                &block_time_source,
                &state,
                false,
                SumeragiV2ValidationContext::from_height_context(&height_context),
                &mut timings,
                |event| events.push(event),
            )
            .unpack(|_| {});
            if let Err((_, error)) = result {
                panic!(
                    "validation with events should succeed and warm stateless cache: {error:?}; events={events:?}"
                );
            }
            assert!(events.is_empty(), "no rejection events expected");
            assert!(
                timings.total_ms >= timings.stateless_ms,
                "total validation timing should cover stateless timing"
            );
            let cache = state.stateless_validation_cache().lock();
            assert!(
                cache.contains_key(&tx_hash),
                "successful static validation with events should populate stateless cache",
            );
        }
        #[test]
        fn prevalidated_commit_skips_only_the_authenticated_block_signature() {
            setup_stateless_cache_state!(kura, state, leader_private, topology, validator_keys);
            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("prevalidated-commit");
            let tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &tx_time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "prevalidated".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone());
            let wrong_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let builder = builder.chain(0, state.view().latest_block().as_deref());
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(wrong_leader.private_key())
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);
            let full_result = validate_voting_test_block!(
                signed_block.clone(),
                &topology,
                &block_time_source,
                &state,
                &validator_keys,
                Duration::from_millis(10)
            )
            .unpack(|_| {});
            assert!(
                full_result.is_err(),
                "ordinary validation should reject the intentionally wrong leader signature"
            );
            let v2_cadence = Duration::from_millis(10);
            let v2_result = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
                signed_block.clone(),
                &topology,
                &ALICE_ID,
                &block_time_source,
                v2_cadence,
                SumeragiV2ValidationContext::from_height_context(
                    &authenticated_permissioned_successor_context(&state, &validator_keys),
                ),
                &state,
            )
            .unpack(|_| {});
            let (_validated, staged_state) = v2_result.expect(
                "v2 candidate validation trusts only the separately checked origin block signature",
            );
            drop(staged_state);
            let mut events = Vec::new();
            let mut timings = ValidationTimings::new();
            let result =
                ValidBlock::validate_sumeragi_v2_fixture_prevalidated_with_events_and_timing(
                    signed_block,
                    &topology,
                    &ALICE_ID,
                    &block_time_source,
                    Duration::from_millis(10),
                    &state,
                    SumeragiV2ValidationContext::from_height_context(
                        &authenticated_permissioned_successor_context(&state, &validator_keys),
                    ),
                    &mut timings,
                    |event| events.push(event),
                )
                .unpack(|_| {});
            assert!(
                result.is_ok(),
                "prevalidated commit execution may skip only the authenticated block signature"
            );
            assert!(events.is_empty(), "no rejection events expected");
            assert!(
                timings.total_ms >= timings.execution_ms,
                "prevalidated timing should still include execution"
            );
            drop(result);
            let (invalid_authority, invalid_signer) =
                gen_account_in("prevalidated-invalid-signature");
            let (forged_authority, _) = gen_account_in("prevalidated-forged-authority");
            let invalid_tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                invalid_authority,
                &tx_time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "invalid-prevalidated".to_owned())])
            .sign(invalid_signer.private_key())
            .with_authority(forged_authority);
            let invalid_builder = BlockBuilder::new_with_time_source(
                vec![AcceptedTransaction::new_unchecked(Cow::Owned(invalid_tx))],
                block_time_source.clone(),
            )
            .chain(0, state.view().latest_block().as_deref());
            let invalid_block = with_current_state_da_sidecars(invalid_builder, &state)
                .sign(wrong_leader.private_key())
                .unpack(|_| {});
            let mut invalid_events = Vec::new();
            let mut invalid_timings = ValidationTimings::new();
            let invalid_result =
                ValidBlock::validate_sumeragi_v2_fixture_prevalidated_with_events_and_timing(
                    invalid_block.into(),
                    &topology,
                    &ALICE_ID,
                    &block_time_source,
                    Duration::from_millis(10),
                    &state,
                    SumeragiV2ValidationContext::from_height_context(
                        &authenticated_permissioned_successor_context(&state, &validator_keys),
                    ),
                    &mut invalid_timings,
                    |event| invalid_events.push(event),
                )
                .unpack(|_| {});
            let Err(error) = invalid_result else {
                panic!("prevalidated commit must still verify every transaction signature");
            };
            assert!(matches!(
                *error.1,
                BlockValidationError::TransactionAccept(
                    AcceptTransactionFail::SignatureVerification(_)
                )
            ));
            assert_eq!(
                invalid_events.len(),
                1,
                "signature rejection should emit exactly one block event"
            );
        }
        #[test]
        fn validate_keep_voting_block_enforces_fraud_policy_with_stateless_cache() {
            use iroha_config::parameters::actual::{FraudMonitoring, FraudRiskBand};
            use iroha_data_model::{
                ValidationFail, account::Account, asset::AssetDefinition, domain::Domain,
                transaction::error::TransactionRejectionReason,
            };
            use std::iter;
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let (authority, signer) = gen_account_in("fraud-cache-test");
            let domain_id: DomainId = DomainId::try_new("fraud-cache-test", "universal")
                .expect("fraud-cache-test domain");
            let domain = Domain::new(domain_id.clone()).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let mut world = World::with([domain], [account], iter::empty::<AssetDefinition>());
            let validator_keys = (0..4)
                .map(|_| checked_keypair_with_algorithm(Algorithm::BlsNormal))
                .collect::<Vec<_>>();
            insert_active_consensus_keys(&mut world, &validator_keys);
            let mut state = State::new(world, Arc::clone(&kura), query);
            install_test_lane_manifests_for_keypairs(&state, &validator_keys);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);
            state.set_fraud_monitoring(FraudMonitoring {
                enabled: true,
                required_minimum_band: Some(FraudRiskBand::High),
                missing_assessment_grace: Duration::ZERO,
                ..Default::default()
            });
            let leader_private = validator_keys[0].private_key().clone();
            let topology = test_topology_with_keys(&validator_keys);
            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);
            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let tx = TransactionBuilder::new_with_time_source(
                state.network_id,
                authority,
                &tx_time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "fraud-check".to_owned())])
            .with_metadata(Metadata::default())
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder = BlockBuilder::new_with_time_source(vec![accepted], block_time_source);
            let builder = builder.chain(0, state.view().latest_block().as_deref());
            let new_block = with_current_state_da_sidecars(builder, &state)
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block = SignedBlock::from(new_block);
            let (valid_block, _) = validate_voting_test_block!(
                signed_block,
                &topology,
                &TimeSource::new_system(),
                &state,
                &validator_keys,
                Duration::from_millis(10)
            )
            .unpack(|_| {})
            .expect("block validation should complete and record transaction result");
            let committed_block: SignedBlock = valid_block.into();
            let rejection = committed_block
                .network_output_at(0)
                .expect("the signed input has its exact Network output")
                .1
                .result
                .as_ref()
                .err()
                .expect("fraud policy rejection should be recorded for missing assessment");
            match rejection {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("fraud monitoring requires an attached assessment"),
                        "unexpected rejection message: {msg}"
                    );
                }
                other => panic!("unexpected rejection reason: {other:?}"),
            }
        }
        // Direct fragment preserves canonical genesis validation test paths and source order.
        include!("block/canonical_genesis_validation_tests.rs");
        #[test]
        fn check_genesis_block_rejects_parent_hash() {
            use iroha_data_model::prelude::*;
            use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
            let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
            let tx = TransactionBuilder::new_genesis(
                genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            let mut block = SignedBlock::genesis(
                vec![tx],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                None,
                None,
            );
            let mut header = block.header();
            header.set_prev_block_hash(Some(HashOf::from_untyped_unchecked(Hash::new(
                b"not-a-genesis-parent",
            ))));
            block.replace_header_for_testing(header);
            let signature = BlockSignature::new(
                0,
                checked_block_signature(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(), block.hash()),
            );
            block
                .replace_signatures([signature].into_iter().collect())
                .expect("replace signature after changing test header");
            assert_eq!(
                check_genesis_block(&block, &genesis_account),
                Err(InvalidGenesisError::InvalidHeader)
            );
        }
        #[test]
        fn resultless_genesis_proposal_is_authenticated_before_execution() {
            use iroha_data_model::prelude::*;
            use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
            let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
            let transaction = TransactionBuilder::new_genesis(
                genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            let block = SignedBlock::genesis(
                vec![transaction],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                None,
                None,
            );
            let proposal = block.canonical_resultless_proposal();
            assert!(proposal.is_resultless_proposal());
            authenticate_genesis_block_intents(&proposal, &genesis_account)
                .expect("the configured genesis key must authenticate a resultless proposal");
            let mut noncanonical_index = proposal.clone();
            noncanonical_index
                .replace_signatures(
                    [BlockSignature::new(
                        1,
                        checked_block_signature(
                            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                            noncanonical_index.hash(),
                        ),
                    )]
                    .into_iter()
                    .collect(),
                )
                .expect("replace the proposal signature index for the adversarial fixture");
            assert_eq!(
                authenticate_genesis_block_intents(&noncanonical_index, &genesis_account)
                    .unwrap_err(),
                InvalidGenesisError::InvalidSignature
            );
            let unrelated = crate::block::checked_keypair();
            let mut forged = proposal;
            let forged_hash = forged.hash();
            forged
                .replace_signatures(
                    [BlockSignature::new(
                        0,
                        checked_block_signature(unrelated.private_key(), forged_hash),
                    )]
                    .into_iter()
                    .collect(),
                )
                .expect("replace the proposal signature for the adversarial fixture");
            assert_eq!(
                authenticate_genesis_block_intents(&forged, &genesis_account).unwrap_err(),
                InvalidGenesisError::InvalidSignature
            );
        }
        #[test]
        fn genesis_block_signature_does_not_replace_transaction_signature() {
            use iroha_data_model::prelude::*;
            let genesis_keypair = crate::block::checked_keypair();
            let unrelated = crate::block::checked_keypair();
            let genesis_account = AccountId::new(genesis_keypair.public_key().clone());
            let mut transaction = TransactionBuilder::new_genesis(
                genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
            .sign(genesis_keypair.private_key());
            transaction.set_signature(iroha_data_model::transaction::TransactionSignature(
                SignatureOf::try_new(unrelated.private_key(), transaction.payload())
                    .expect("unrelated key can sign the adversarial fixture payload"),
            ));
            let block =
                SignedBlock::genesis(vec![transaction], genesis_keypair.private_key(), None, None);
            assert_eq!(
                check_genesis_block(&block, &genesis_account),
                Err(InvalidGenesisError::InvalidTransactionSignature)
            );
        }
        include!("block/genesis_validation_regression_tests.rs");
        #[test]
        fn signed_genesis_validation_is_storage_side_effect_free() {
            use crate::{
                kura::Kura, query::store::LiveQueryStore, sumeragi::network_topology::Topology,
            };
            use iroha_data_model::{
                block::consensus_v2::{
                    ConsensusMode, SumeragiV2GenesisContextParameters, ValidatorPower,
                },
                parameter::{Parameter, system::SumeragiParameter},
                prelude::*,
            };
            use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
            use iroha_model_base::peer::PeerId;
            iroha_genesis::init_instruction_registry();
            let chain_id = ChainId::from("00000000-0000-0000-0000-000000000001");
            let genesis_keypair = crate::block::checked_keypair();
            let genesis_account = AccountId::new(genesis_keypair.public_key().clone());
            let mut topology = (0..4)
                .map(|_| {
                    let validator = crate::block::checked_keypair_with_algorithm(
                        iroha_crypto::Algorithm::BlsNormal,
                    );
                    let pop = iroha_crypto::bls_normal_pop_prove(validator.private_key())
                        .expect("derive genesis side-effect fixture validator PoP");
                    GenesisTopologyEntry::new(PeerId::new(validator.public_key().clone()), pop)
                })
                .collect::<Vec<_>>();
            topology.sort_by(|left, right| left.peer.cmp(&right.peer));
            let roster = topology
                .iter()
                .map(|entry| ValidatorPower {
                    validator: entry.peer.clone(),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let mint_finality =
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster);
            let manifest = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
                .with_sumeragi_v2_context_parameters(
                    SumeragiV2GenesisContextParameters::recommended(),
                )
                .with_kagemusha_mint_finality_genesis_parameters(mint_finality)
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)))
                .next_transaction()
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333)))
                .set_topology(topology)
                .build_raw()
                .expect("ordered genesis parameters form one valid raw transaction");
            let genesis = manifest
                .build_and_sign(&genesis_keypair)
                .expect("ordered genesis parameters should build");
            let topology = Topology::new(
                crate::sumeragi::startup::genesis_committee_peers(&genesis.0)
                    .expect("signed genesis must expose its exact voting roster"),
            );
            let genesis_domain =
                Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account);
            let genesis_account_model =
                Account::new(genesis_account.clone()).build(&genesis_account);
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(
                World::with([genesis_domain], [genesis_account_model], []),
                Arc::clone(&kura),
                query_handle,
            );
            install_test_lane_manifests_for_keypairs(
                &state,
                std::slice::from_ref(&genesis_keypair),
            );
            let genesis_block = with_current_state_confidential_features(
                genesis.0,
                &state,
                &[(0, genesis_keypair.private_key())],
            );
            let time_source = TimeSource::new_system();
            let result = ValidBlock::validate_signed_genesis(
                genesis_block,
                &topology,
                &genesis_account,
                &time_source,
                &state,
                ConsensusMode::Permissioned,
            )
            .unpack(|_| {});
            if let Err((failed_block, err)) = result {
                let results = failed_block
                    .output_results()
                    .map(|result| format!("{result:?}"))
                    .collect::<Vec<_>>();
                panic!(
                    "ordered genesis parameter transactions should validate: {err}; results={results:?}"
                );
            }
            assert_eq!(
                kura.pipeline_sidecar_queue_len_for_testing(),
                0,
                "disposable signed-genesis validation must not publish pipeline recovery metadata"
            );
        }
    }
    #[test]
    fn insufficient_commit_quorum_maps_to_a_rejection_reason() {
        let keypairs = (0..4)
            .map(|_| checked_keypair_with_algorithm(Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        let topology = Topology::new(
            keypairs
                .iter()
                .map(|key| PeerId::new(key.public_key().clone())),
        );
        let signed: SignedBlock = ValidBlock::new_dummy(keypairs[0].private_key()).into();
        let error = ValidBlock::is_commit(&signed, &topology)
            .expect_err("one valid BLS leader signature cannot satisfy the four-validator quorum");
        assert!(matches!(
            map_sig_err_to_reason(&error),
            iroha_data_model::block::error::BlockRejectionReason::InsufficientBlockSignatures
        ));
    }
}
mod commit {
    use super::*;
    /// Executed block lifecycle wrapper. Native consensus authentication is held by the
    /// original canonical carrier and `sumeragi::certified_chain::CommittedBlock`.
    #[derive(Debug, Clone)]
    pub struct CommittedBlock {
        block: ValidBlock,
    }
    impl CommittedBlock {
        pub(super) fn from_execution(block: ValidBlock) -> Self {
            Self { block }
        }
    }
    impl From<CommittedBlock> for ValidBlock {
        fn from(source: CommittedBlock) -> Self {
            source.block
        }
    }
    impl From<CommittedBlock> for SignedBlock {
        fn from(source: CommittedBlock) -> Self {
            source.block.into()
        }
    }
    impl AsRef<SignedBlock> for CommittedBlock {
        fn as_ref(&self) -> &SignedBlock {
            self.block.as_ref()
        }
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    impl AsMut<SignedBlock> for CommittedBlock {
        fn as_mut(&mut self) -> &mut SignedBlock {
            self.block.as_mut()
        }
    }
    #[cfg(all(test, feature = "app_api"))]
    mod axt_validation_tests {
        include!("block/axt_anchored_spend_admission_tests.rs");
    }
}
mod event {
    use super::*;
    use crate::state::StateBlock;
    use new::NewBlock;
    use std::collections::BTreeSet;
    pub trait EventProducer {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox>;
    }
    #[derive(Debug)]
    #[must_use]
    pub struct WithEvents<B>(B);
    impl<B> WithEvents<B> {
        pub(super) fn new(source: B) -> Self {
            Self(source)
        }
    }
    impl<B: EventProducer, U> WithEvents<Result<B, (U, Box<BlockValidationError>)>> {
        pub fn unpack<F: FnMut(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<B, (U, Box<BlockValidationError>)> {
            match self.0 {
                Ok(ok) => Ok(WithEvents(ok).unpack(f)),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl<'state, B: EventProducer, U>
        WithEvents<Result<(B, Box<StateBlock<'state>>), (U, Box<BlockValidationError>)>>
    {
        pub fn unpack<F: FnMut(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<(B, Box<StateBlock<'state>>), (U, Box<BlockValidationError>)> {
            match self.0 {
                Ok((ok, state)) => Ok((WithEvents(ok).unpack(f), state)),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl WithEvents<Result<BTreeSet<BlockSignature>, SignatureVerificationError>> {
        pub fn unpack<F: FnMut(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<BTreeSet<BlockSignature>, SignatureVerificationError> {
            match self.0 {
                Ok(ok) => Ok(ok),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl<B: EventProducer> WithEvents<B> {
        pub fn unpack<F: FnMut(PipelineEventBox)>(self, f: F) -> B {
            self.0.produce_events().for_each(f);
            self.0
        }
    }
    impl<B, E: EventProducer> WithEvents<(B, E)> {
        pub(crate) fn unpack<F: FnMut(PipelineEventBox)>(self, f: F) -> (B, E) {
            self.0.1.produce_events().for_each(f);
            self.0
        }
    }
    impl EventProducer for NewBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_event = BlockEvent {
                header: self.header,
                status: BlockStatus::Created,
            };
            core::iter::once(block_event.into())
        }
    }
    impl EventProducer for ValidBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block = self.as_ref();
            let block_height = block.header().height();
            let is_genesis = block.header().is_genesis();
            // Validate the complete body before exposing any status. A missing
            // or malformed output must never default to Approved.
            let valid_outputs = match block.validate_output_merkle_cache() {
                Ok(()) => true,
                Err(error) => {
                    iroha_logger::error!(
                        block_height = block_height.get(),
                        %error,
                        "validated block event source has invalid execution outputs"
                    );
                    false
                }
            };
            let committed_routes = block
                .execution_context()
                .map(|bundle| bundle.external.as_slice());
            let tx_events = block.network_entrypoints().enumerate().filter_map(
                move |(idx, entrypoint)| {
                    if !valid_outputs {
                        return None;
                    }
                    let entrypoint_hash = entrypoint.hash();
                    let tx = match entrypoint {
                        TransactionEntrypoint::External(tx) => tx,
                        TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
                        TransactionEntrypoint::SealedCommitment(_) => return None,
                    };
                    let input_index = u32::try_from(idx).ok()?;
                    let (_, output) = block.network_output_at(input_index)?;
                    let Some(context) = committed_routes.and_then(|routes| routes.get(idx)) else {
                        if !is_genesis {
                            iroha_logger::error!(
                                block_height = block_height.get(),
                                entrypoint_index = idx,
                                %entrypoint_hash,
                                "validated block transaction event is missing its committed execution route"
                            );
                        }
                        return None;
                    };
                    if context.entrypoint_hash != entrypoint_hash {
                        iroha_logger::error!(
                            block_height = block_height.get(),
                            entrypoint_index = idx,
                            %entrypoint_hash,
                            committed_entrypoint_hash = %context.entrypoint_hash,
                            "validated block transaction event route is bound to a different entrypoint"
                        );
                        return None;
                    }
                    let status = match output.result.as_ref() {
                        Ok(_) => TransactionStatus::Approved,
                        Err(error) => TransactionStatus::Rejected(Box::new(error.clone())),
                    };
                    Some(TransactionEvent {
                        hash: tx.hash(),
                        block_height: Some(block_height),
                        lane_id: context.lane_id,
                        dataspace_id: context.dataspace_id,
                        status,
                    })
                },
            );
            let block_event = valid_outputs.then(|| BlockEvent {
                header: block.header(),
                status: BlockStatus::Approved,
            });
            tx_events
                .map(PipelineEventBox::from)
                .chain(block_event.into_iter().map(Into::into))
        }
    }
    impl EventProducer for CommittedBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_event = core::iter::once(BlockEvent {
                header: self.as_ref().header(),
                status: BlockStatus::Committed,
            });
            block_event.map(Into::into)
        }
    }
    pub(super) fn map_sig_err_to_reason(
        err: &SignatureVerificationError,
    ) -> iroha_data_model::block::error::BlockRejectionReason {
        use iroha_data_model::block::error::BlockRejectionReason as Reason;
        match err {
            SignatureVerificationError::NotEnoughSignatures { .. } => {
                Reason::InsufficientBlockSignatures
            }
            SignatureVerificationError::DuplicateSignature { .. }
            | SignatureVerificationError::UnknownSignature
            | SignatureVerificationError::MissingPop => Reason::InvalidBlockSignature,
            SignatureVerificationError::UnknownSignatory => Reason::UnknownBlockSignatory,
            SignatureVerificationError::InactiveConsensusKey => Reason::InactiveConsensusKey,
            SignatureVerificationError::ProxyTailMissing => Reason::ProxyTailSignatureMissing,
            SignatureVerificationError::LeaderMissing => Reason::LeaderSignatureMissing,
            SignatureVerificationError::Other => Reason::OtherSignatureError,
        }
    }
    pub(super) fn map_block_err_to_reason(
        err: &BlockValidationError,
    ) -> Option<iroha_data_model::block::error::BlockRejectionReason> {
        use iroha_data_model::block::error::BlockRejectionReason as Reason;
        Some(match err {
            BlockValidationError::LocalStorageRecoveryRequired { .. }
            | BlockValidationError::StateStorageAdmission(_)
            | BlockValidationError::EvidencePreparation(_)
            | BlockValidationError::ExecutionDeferred(_)
            | BlockValidationError::BlockHashAdmission(_)
            | BlockValidationError::MembershipAdmission(_) => return None,
            BlockValidationError::HasCommittedTransactions => Reason::ContainsCommittedTransactions,
            BlockValidationError::EmptyBlock => Reason::EmptyBlock,
            BlockValidationError::DuplicateTransactions
            | BlockValidationError::TooManyTransactions { .. } => {
                Reason::TransactionValidationFailed
            }
            BlockValidationError::ExecutionContextInvalid(_)
            | BlockValidationError::MissingCertifiedMergeSidecar { .. }
            | BlockValidationError::CommittedFragmentCountMismatch { .. } => {
                Reason::TransactionValidationFailed
            }
            BlockValidationError::PrevBlockHashMismatch { .. } => Reason::PrevBlockHashMismatch,
            BlockValidationError::PrevBlockHeightMismatch { .. } => Reason::PrevBlockHeightMismatch,
            BlockValidationError::MerkleRootMismatch => Reason::MerkleRootMismatch,
            BlockValidationError::TransactionAccept(fail) => match fail {
                AcceptTransactionFail::TransactionLimit(_)
                | AcceptTransactionFail::SignatureVerification(_)
                | AcceptTransactionFail::UnexpectedGenesisAccountSignature
                | AcceptTransactionFail::TransactionDomainMismatch(_)
                | AcceptTransactionFail::TransactionInTheFuture { .. }
                | AcceptTransactionFail::TransactionExpired { .. }
                | AcceptTransactionFail::NetworkTimeUnhealthy { .. } => {
                    Reason::TransactionValidationFailed
                }
            },
            BlockValidationError::TopologyMismatch { .. } => Reason::TopologyMismatch,
            BlockValidationError::SignatureVerification(e) => map_sig_err_to_reason(e),
            BlockValidationError::InvalidGenesis(_)
            | BlockValidationError::GenesisPolicyMismatch { .. } => Reason::InvalidGenesis,
            BlockValidationError::BlockInThePast => Reason::BlockInThePast,
            BlockValidationError::BlockInTheFuture => Reason::BlockInTheFuture,
            BlockValidationError::NonCanonicalV2BlockTime { .. }
            | BlockValidationError::V2BlockTimeOverflow => Reason::BlockInTheFuture,
            BlockValidationError::SnapshotBootstrapParentInvalid(_) => {
                Reason::ConsensusBlockRejection
            }
            BlockValidationError::V2FinalityAuthorityInvalid(_) => Reason::ConsensusBlockRejection,
            BlockValidationError::TransactionInTheFuture => Reason::TransactionInTheFuture,
            BlockValidationError::ConfidentialFeaturesMismatch { .. } => {
                Reason::ConfidentialFeatureDigestMismatch
            }
            BlockValidationError::ProofPolicyHashMismatch { .. }
            | BlockValidationError::DaProofPolicySidecarHashMismatch { .. }
            | BlockValidationError::DaProofPolicyBundleMismatch => Reason::DaProofPolicyMismatch,
            BlockValidationError::DaCommitmentHashMismatch { .. }
            | BlockValidationError::NonCanonicalEmptyDaCommitmentBundle
            | BlockValidationError::DaPinIntentHashMismatch { .. }
            | BlockValidationError::NonCanonicalEmptyDaPinIntentBundle => {
                Reason::DaShardCursorViolation
            }
            BlockValidationError::DaCommitmentBundle(
                crate::da::DaCommitmentValidationError::ProofPolicy(_),
            ) => Reason::DaProofPolicyMismatch,
            BlockValidationError::DaCommitmentBundle(_) => Reason::DaShardCursorViolation,
            BlockValidationError::DaPinIntentBundle(_) => Reason::DaShardCursorViolation,
            BlockValidationError::DaIndexHydration(_) => Reason::DaShardCursorViolation,
            BlockValidationError::DaReceiptCursor(_) => Reason::DaShardCursorViolation,
            BlockValidationError::DaShardCursor(_) => Reason::DaShardCursorViolation,
            BlockValidationError::AxtEnvelopeValidationFailed(_) => {
                Reason::TransactionValidationFailed
            }
            BlockValidationError::NposEffectsInvalid(_) => Reason::NposEffectsMismatch,
        })
    }
    /// Emit a rejection only when validation produced a deterministic reason.
    pub(super) fn emit_block_rejection(
        header: BlockHeader,
        error: &BlockValidationError,
        mut send_events: impl FnMut(PipelineEventBox),
    ) {
        if let Some(reason) = map_block_err_to_reason(error) {
            send_events(PipelineEventBox::from(BlockEvent {
                header,
                status: BlockStatus::Rejected(reason),
            }));
        }
    }
    impl EventProducer for BlockValidationError {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            // Rejection events require a block header to construct `BlockEvent`.
            // These are emitted by consensus callers at sites where the offending
            // header and authenticated validation context are available.
            core::iter::empty()
        }
    }
    impl<T: EventProducer + ?Sized> EventProducer for Box<T> {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            (**self).produce_events()
        }
    }
    impl EventProducer for SignatureVerificationError {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            // Similar to `BlockValidationError`: emission is performed by the
            // caller at the site where the header is available.
            core::iter::empty()
        }
    }
    #[cfg(test)]
    mod tests {
        include!("block/output_event_tests.rs");
    }
}
#[cfg(all(test, feature = "simd"))]
mod simd_parent_dedup {
    const LANES: usize = 8;

    pub(super) fn dedup_sorted_slice(slice: &mut [usize]) -> Option<usize> {
        if slice.len() <= 1 {
            return Some(slice.len());
        }
        let mut write = 1usize;
        let mut prev = slice[0];
        let mut idx = 1usize;
        while idx + LANES <= slice.len() {
            // Snapshot each fixed-width chunk before compacting into the same
            // slice. This keeps the kernel safe, deterministic, and suitable
            // for stable-Rust auto-vectorization without aliasing unread data.
            let mut chunk = [0usize; LANES];
            chunk.copy_from_slice(&slice[idx..idx + LANES]);
            for value in chunk {
                if value != prev {
                    slice[write] = value;
                    write += 1;
                    prev = value;
                }
            }
            idx += LANES;
        }
        while idx < slice.len() {
            let value = slice[idx];
            if value != prev {
                slice[write] = value;
                write += 1;
                prev = value;
            }
            idx += 1;
        }
        Some(write)
    }

    #[cfg(test)]
    mod tests {
        use super::dedup_sorted_slice;

        #[test]
        fn fixed_width_dedup_handles_chunk_boundaries() {
            let mut values = [1, 1, 2, 3, 3, 3, 4, 5, 5, 6, 7, 7, 8, 9, 9, 10, 10];
            let len = dedup_sorted_slice(&mut values).expect("fixed-width path is available");
            assert_eq!(&values[..len], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        }
    }
}
#[cfg(test)]
/// Build a conflict graph from access sets using an incremental O(n + E) algorithm.
/// Returns adjacency list and indegree vector.
#[allow(clippy::disallowed_types)]
fn build_conflict_graph(
    access: &[crate::pipeline::access::AccessSet],
) -> (
    Vec<iroha_primitives::small::SmallVec<[usize; 8]>>,
    Vec<usize>,
) {
    use iroha_primitives::small::SmallVec;
    // Intern keys once per block to operate on compact integer IDs while
    // preserving deterministic ordering across peers.
    let (key_count, access_ids) = intern_access(access);
    let n = access.len();
    let mut adj: Vec<SmallVec<[usize; 8]>> = vec![SmallVec::new(); n];
    let mut indeg = vec![0usize; n];
    // Track the most recent writer per interned key and readers awaiting a write.
    let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
    let mut open_readers: Vec<SmallVec<[usize; 4]>> = (0..key_count)
        .map(|_| SmallVec::<[usize; 4]>::new())
        .collect();
    // Component partitioning via disjoint-set prepass is handled before scheduling.
    for (idx, aset) in access_ids.iter().enumerate() {
        // Collect parents in a small vec; sort+dedup to avoid the log factor of BTreeSet
        let mut parents: SmallVec<[usize; 8]> = SmallVec::new();
        // Read dependencies: last writer of each read key must precede this read
        for &key in aset.reads.iter() {
            let key_idx = key as usize;
            if let Some(writer) = last_writer[key_idx] {
                parents.push(writer);
            }
            open_readers[key_idx].push(idx);
        }
        // Write dependencies: last writer must precede; all open readers must precede
        for &key in aset.writes.iter() {
            let key_idx = key as usize;
            if let Some(writer) = last_writer[key_idx] {
                parents.push(writer);
            }
            let readers = &mut open_readers[key_idx];
            for &reader in readers.iter() {
                parents.push(reader);
            }
            readers.clear();
            last_writer[key_idx] = Some(idx);
        }
        if !parents.is_empty() {
            // Deterministic dedup without extra allocations
            parents.sort_unstable();
            let mut write = 0usize;
            let mut last: Option<usize> = None;
            for i in 0..parents.len() {
                let v = parents[i];
                if Some(v) != last {
                    parents[write] = v;
                    write += 1;
                    last = Some(v);
                }
            }
            while parents.len() > write {
                let _ = parents.remove(parents.len() - 1);
            }
            for p in parents {
                adj[p].push(idx);
                indeg[idx] += 1;
            }
        }
    }
    (adj, indeg)
}
#[cfg(test)]
mod dag_tests {
    use super::build_conflict_graph;
    use crate::pipeline::access::AccessSet;
    fn rw(reads: &[&str], writes: &[&str]) -> AccessSet {
        let mut s = AccessSet::new();
        for k in reads {
            s.add_read((*k).to_string());
        }
        for k in writes {
            s.add_write((*k).to_string());
        }
        s
    }
    #[test]
    fn ww_conflict_edge() {
        let a = rw(&[], &["k"]);
        let b = rw(&[], &["k"]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert!(adj[1].is_empty());
    }
    #[test]
    fn state_map_wildcard_conflicts_with_map_entries() {
        let a = rw(&[], &["state:Foo/1"]);
        let b = rw(&[], &["state:Foo/2"]);
        let c = rw(&[], &["state:Foo[*]"]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 0, 2]);
        assert_eq!(&adj[0][..], &[2]);
        assert_eq!(&adj[1][..], &[2]);
        assert!(adj[2].is_empty());
    }
    #[test]
    fn exact_state_map_keys_only_conflict_when_canonical_keys_match() {
        let first = rw(&[], &["state:Foo/01"]);
        let distinct = rw(&[], &["state:Foo/02"]);
        let repeated = rw(&[], &["state:Foo/01"]);
        let (adj, indeg) = build_conflict_graph(&[first, distinct, repeated]);
        assert_eq!(indeg, vec![0, 0, 1]);
        assert_eq!(&adj[0][..], &[2]);
        assert!(adj[1].is_empty());
        assert!(adj[2].is_empty());
    }
    #[test]
    fn global_wildcard_conflicts_with_all() {
        let a = rw(&[], &["k1"]);
        let b = rw(&[], &["*"]);
        let c = rw(&[], &["k2"]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 1, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert_eq!(&adj[1][..], &[2]);
        assert!(adj[2].is_empty());
    }
    #[test]
    fn state_global_wildcard_conflicts_with_state_entries() {
        let a = rw(&[], &["state:Foo/1"]);
        let b = rw(&[], &["state:*"]);
        let c = rw(&[], &["state:Foo/2"]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 1, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert_eq!(&adj[1][..], &[2]);
        assert!(adj[2].is_empty());
    }
    #[test]
    fn wr_conflict_edge() {
        let a = rw(&[], &["k"]);
        let b = rw(&["k"], &[]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert!(adj[1].is_empty());
    }
    #[test]
    fn rw_conflict_edge() {
        let a = rw(&["k"], &[]);
        let b = rw(&[], &["k"]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert!(adj[1].is_empty());
    }
    #[test]
    fn dedup_edges_for_multiple_keys() {
        let a = rw(&[], &["x", "y"]);
        let b = rw(&["x", "y"], &[]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]); // only one edge despite two overlapping keys
    }
    #[test]
    fn disjoint_transactions_remain_independent() {
        let a = rw(&["alpha"], &[]);
        let b = rw(&[], &["beta"]);
        let c = rw(&["gamma"], &[]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 0, 0]);
        assert!(adj.iter().all(|neighbors| neighbors.is_empty()));
    }
    #[test]
    fn chain_reads_and_writes() {
        // 0: R(A); 1: W(A); 2: R(A); 3: W(A)
        let a0 = rw(&["A"], &[]);
        let a1 = rw(&[], &["A"]);
        let a2 = rw(&["A"], &[]);
        let a3 = rw(&[], &["A"]);
        let (adj, indeg) = build_conflict_graph(&[a0, a1, a2, a3]);
        assert_eq!(indeg, vec![0, 1, 1, 2]);
        assert_eq!(&adj[0][..], &[1]);
        assert_eq!(&adj[1][..], &[2, 3]);
        assert_eq!(&adj[2][..], &[3]);
        assert!(adj[3].is_empty());
    }
}
#[cfg(test)]
mod dsu_tests {
    use super::{DisjointSet, intern_access};
    use crate::pipeline::access::AccessSet;
    use iroha_primitives::small::SmallVec;
    fn ids(reads: &[&str], writes: &[&str]) -> AccessSet {
        let mut s = AccessSet::new();
        for k in reads {
            s.add_read((*k).to_string());
        }
        for k in writes {
            s.add_write((*k).to_string());
        }
        s
    }
    #[test]
    fn dsu_partitions_independent_components() {
        // Two independent components: {0,1} share key "A"; {2,3} share key "B".
        let a0 = ids(&["A"], &[]);
        let a1 = ids(&[], &["A"]);
        let b0 = ids(&["B"], &[]);
        let b1 = ids(&[], &["B"]);
        let access = [a0, a1, b0, b1];
        let (key_count, access_ids) = intern_access(&access);
        let mut dsu = DisjointSet::new(access_ids.len());
        {
            let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
            let mut open_readers: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); key_count];
            for (idx, aset) in access_ids.iter().enumerate() {
                for &k in aset.reads.iter() {
                    if let Some(w) = last_writer[k as usize] {
                        dsu.union(idx, w);
                    }
                    open_readers[k as usize].push(idx);
                }
                for &k in aset.writes.iter() {
                    if let Some(w) = last_writer[k as usize] {
                        dsu.union(idx, w);
                    }
                    if let Some(readers) = {
                        if open_readers[k as usize].is_empty() {
                            None
                        } else {
                            Some(std::mem::take(&mut open_readers[k as usize]))
                        }
                    } {
                        for r in readers {
                            dsu.union(idx, r);
                        }
                    }
                    last_writer[k as usize] = Some(idx);
                }
            }
        }
        let mut roots: Vec<usize> = Vec::new();
        let mut dsu_copy = dsu.clone();
        for i in 0..4 {
            roots.push(dsu_copy.find(i));
        }
        // Expect two distinct roots among four items
        let mut uniq = roots.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), 2);
    }
}
include!("block/scheduler_variant_tests.rs");
/// Block validation tests and signed Native AMX fixtures shared within Core.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        block::event::map_sig_err_to_reason,
        governance::manifest::{LaneManifestRegistry, LaneManifestStatus},
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::{Execute, isi::triggers::set::SetReadOnly},
        state::{State, World},
        tx::AcceptedTransaction,
    };
    use core::time::Duration;
    use iroha_crypto::{Hash, HashOf, KeyPair, Signature, bls_normal_aggregate_signatures};
    use iroha_data_model::{
        errors::AmxStage,
        events::pipeline::{BlockEventFilter, TransactionEventFilter},
        prelude::*,
        transaction::{
            ExecutableBatchItem,
            signed::{
                SealedTransactionCommitmentPayload, SealedTransactionReveal,
                SignedSealedTransactionCommitment, SignedTransaction,
                compute_sealed_transaction_commitment,
            },
        },
    };
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_model_base::chain::ChainId;
    use iroha_model_base::domain::DomainId;
    use iroha_model_base::metadata::Metadata;
    use iroha_model_base::{name::Name, state_path::StatePath};
    use iroha_primitives::json::Json;
    use iroha_primitives::time::TimeSource;
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;
    use std::{borrow::Cow, num::NonZeroU64};
    #[test]
    fn merge_capable_validation_paths_source_bind_post_effect_authorization() {
        let source = include_str!("block.rs");
        let staged_reference_needle = ["Self::validate_staged_execution_controls", "("].concat();
        let post_effect_authorization_needle =
            ["Self::validate_staged_merge_execution_authorization", "("].concat();
        let staged_reference_calls = source.matches(&staged_reference_needle).count();
        let post_effect_authorization_calls =
            source.matches(&post_effect_authorization_needle).count();
        assert_eq!(
            post_effect_authorization_calls, 4,
            "both Sumeragi-v2 fixtures, authenticated keep-voting validation, and unchecked execution must gate merge execution after effects"
        );
        let native_finalizer = source
            .split_once("pub(crate) fn finalize_native_execution_contexts(")
            .expect("source-owned native finalizer")
            .1
            .split_once("fn state_block_for_execution<")
            .expect("ordinary execution boundary follows the native finalizer")
            .0;
        assert_eq!(
            native_finalizer.matches(&staged_reference_needle).count(),
            1,
            "source-owned native finalization must independently check its exact staged controls"
        );
        assert_eq!(
            staged_reference_calls,
            post_effect_authorization_calls + 1,
            "only the source-owned native finalizer omits the old merge voting gate"
        );
    }
    include!("block/validation_native_amx_test_support.rs");
    fn signed_native_amx_attestation_qc_with_signer_count(
        phase: NativeAmxPhase,
        source_id: [u8; iroha_crypto::Hash::LENGTH],
        tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
        plan_digest: iroha_crypto::Hash,
        coordinator_proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
        participant: crate::queue::RoutingDecision,
        keypairs: &[KeyPair],
        signer_count: usize,
    ) -> NativeAmxAttestationQcV2 {
        let mut ordered_keypairs = keypairs.iter().collect::<Vec<_>>();
        ordered_keypairs.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let validator_set = ordered_keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_set_pops = ordered_keypairs
            .iter()
            .map(|keypair| {
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("generate fixture BLS proof-of-possession")
            })
            .collect::<Vec<_>>();
        let descriptor = &coordinator_proposal.descriptor;
        let participant_is_coordinator = participant.lane_id == descriptor.lane_id
            && participant.dataspace_id == descriptor.dataspace_id;
        let participant_min_quorum = iroha_sumeragi::types::quorum(validator_set.len()).max(1);
        let (
            participant_lane_incarnation,
            participant_previous_block_height,
            participant_previous_block_descriptor_hash,
            participant_lane_block_height,
            participant_lane_block_view,
        ) = if participant_is_coordinator {
            (
                descriptor.lane_incarnation,
                descriptor.previous_lane_block_height,
                descriptor.previous_lane_block_descriptor_hash,
                descriptor.lane_block_height,
                descriptor.lane_block_view,
            )
        } else {
            (
                Hash::new(participant.lane_id.as_u32().to_be_bytes()),
                0,
                None,
                1,
                0,
            )
        };
        let mut body = NativeAmxAttestationBodyV2 {
            round: iroha_data_model::block::consensus_v2::ConsensusRound {
                context_id:
                    iroha_data_model::block::consensus_v2::HeightContextId(HashOf::<
                        iroha_data_model::block::consensus_v2::HeightContext,
                    >::from_untyped_unchecked(
                        Hash::new(b"native-amx-block-test-context"),
                    )),
                height: descriptor.proposal_height,
                view: 0,
            },
            epoch: 0,
            network_id: native_amx_test_network_id(),
            source_id,
            tx_entrypoint_hash,
            plan_digest,
            phase,
            coordinator_lane_id: descriptor.lane_id,
            coordinator_dataspace_id: descriptor.dataspace_id,
            coordinator_lane_incarnation: descriptor.lane_incarnation,
            participant_lane_id: participant.lane_id,
            participant_dataspace_id: participant.dataspace_id,
            participant_lane_incarnation,
            participant_previous_block_height,
            participant_previous_block_descriptor_hash,
            participant_lane_block_height,
            participant_lane_block_view,
            participant_proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
            participant_validator_set_hash: HashOf::new(&validator_set),
            participant_validator_count: u32::try_from(validator_set.len())
                .expect("fixture validator count"),
            participant_min_quorum: u32::try_from(participant_min_quorum)
                .expect("fixture participant quorum"),
            authority_context_height: descriptor.proposal_height,
            planned_coordinator_block_height: descriptor.lane_block_height,
            coordinator_lane_block_view: descriptor.lane_block_view,
            coordinator_proposal_hash: coordinator_proposal.proposal_hash,
        };
        let participant_proposal = native_amx_test_participant_proposal(
            &body,
            validator_set.clone(),
            coordinator_proposal,
        );
        body.participant_proposal_hash = participant_proposal.proposal_hash;
        body.participant_settlement_commitment = body
            .computed_grouped_participant_settlement_commitment(None, &[body.source_id])
            .expect("single-source test fixture settlement is valid");
        let preimage = body.signature_preimage();
        let signatures = ordered_keypairs
            .iter()
            .take(signer_count)
            .map(|keypair| {
                checked_signature(keypair.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let bls_aggregate_signature =
            bls_normal_aggregate_signatures(&signature_refs).expect("aggregate AMX signatures");
        let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
        for idx in 0..signer_count.min(validator_set.len()) {
            signers_bitmap[idx / 8] |= 1_u8 << (idx % 8);
        }
        let validator_set_hash = HashOf::new(&validator_set);
        NativeAmxAttestationQcV2::try_new(
            body,
            VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
            validator_set,
            validator_set_pops,
            signers_bitmap,
            bls_aggregate_signature,
        )
        .expect("fixture validator set and proofs must align")
    }
    /// Build a signed typed receipt for Core's Native AMX validation fixtures.
    pub(crate) fn signed_native_amx_receipt(
        source_id: [u8; iroha_crypto::Hash::LENGTH],
        tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
        routing_plan: &crate::queue::RoutingPlan,
        block_height: u64,
        keypairs: &[KeyPair],
    ) -> NativeAmxReceipt {
        let signer_count = iroha_sumeragi::types::quorum(keypairs.len()).max(1);
        signed_native_amx_receipt_with_signer_count(
            source_id,
            tx_entrypoint_hash,
            routing_plan,
            block_height,
            keypairs,
            signer_count,
        )
    }
    fn signed_native_amx_receipt_with_signer_count(
        source_id: [u8; iroha_crypto::Hash::LENGTH],
        tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
        routing_plan: &crate::queue::RoutingPlan,
        block_height: u64,
        keypairs: &[KeyPair],
        signer_count: usize,
    ) -> NativeAmxReceipt {
        let crate::queue::RoutingPlan::NativeAmx(plan) = routing_plan else {
            panic!("test expects native AMX plan");
        };
        let coordinator = plan.coordinator.route;
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            coordinator,
            tx_entrypoint_hash,
            block_height,
            keypairs,
        );
        signed_native_amx_receipt_for_coordinator(
            source_id,
            tx_entrypoint_hash,
            routing_plan,
            coordinator_proposal,
            keypairs,
            signer_count,
        )
    }
    fn signed_native_amx_receipt_for_coordinator(
        source_id: [u8; iroha_crypto::Hash::LENGTH],
        tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
        routing_plan: &crate::queue::RoutingPlan,
        coordinator_proposal: LaneBlockProposalV1,
        keypairs: &[KeyPair],
        signer_count: usize,
    ) -> NativeAmxReceipt {
        let crate::queue::RoutingPlan::NativeAmx(plan) = routing_plan else {
            panic!("test expects native AMX plan");
        };
        let coordinator = plan.coordinator.route;
        let legs = plan
            .participants
            .iter()
            .map(|leg| {
                let prepare_qc = signed_native_amx_attestation_qc_with_signer_count(
                    NativeAmxPhase::Prepare,
                    source_id,
                    tx_entrypoint_hash,
                    routing_plan.digest(),
                    &coordinator_proposal,
                    leg.route,
                    keypairs,
                    signer_count,
                );
                let commit_qc = signed_native_amx_attestation_qc_with_signer_count(
                    NativeAmxPhase::Commit,
                    source_id,
                    tx_entrypoint_hash,
                    routing_plan.digest(),
                    &coordinator_proposal,
                    leg.route,
                    keypairs,
                    signer_count,
                );
                let participant_proposal = native_amx_test_participant_proposal(
                    &prepare_qc.body,
                    prepare_qc.validator_set().to_vec(),
                    &coordinator_proposal,
                );
                let participant_settlement = prepare_qc
                    .body
                    .computed_grouped_participant_settlement(None, &[prepare_qc.body.source_id])
                    .expect("single-source test fixture settlement is valid");
                let participant_settlement_hash = participant_settlement
                    .computed_hash()
                    .expect("fixture participant settlement hashes");
                NativeAmxLegRecordV2 {
                    lane_id: leg.route.lane_id,
                    dataspace_id: leg.route.dataspace_id,
                    participant_proposal,
                    participant_settlement,
                    participant_settlement_hash,
                    prepare_qc,
                    commit_qc,
                }
            })
            .collect();
        NativeAmxReceipt {
            version: 2,
            source_id,
            network_id: native_amx_test_network_id(),
            plan_digest: routing_plan.digest(),
            lane_id: coordinator.lane_id,
            dataspace_id: coordinator.dataspace_id,
            lane_incarnation: coordinator_proposal.descriptor.lane_incarnation,
            authority_context_height: coordinator_proposal.descriptor.proposal_height,
            lane_block_height: coordinator_proposal.descriptor.lane_block_height,
            lane_block_view: coordinator_proposal.descriptor.lane_block_view,
            coordinator_proposal_hash: coordinator_proposal.proposal_hash,
            legs,
        }
    }
    struct HistoricalNativeAmxSourceBundleFixture {
        bundle: crate::kura::AutonomousLaneMergeBundleV1,
        source_bundle: Vec<u8>,
        active_lanes: Vec<MergeLaneBinding>,
        authority: NativeAmxTestAuthority,
        network_id: iroha_data_model::NetworkId,
        epoch: u64,
    }
    #[expect(
        clippy::too_many_lines,
        reason = "the fixture builds the complete autonomous and Native AMX certificate chain"
    )]
    fn historical_native_amx_source_bundle_fixture() -> HistoricalNativeAmxSourceBundleFixture {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, _tx_hash) = signed_domain_registration_tx_with_admission_intent(
            &[("merchant", "paynet"), ("treasury", "cbuae")],
            iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
        );
        let entrypoint = TransactionEntrypoint::External(tx);
        let entrypoint_hash = entrypoint.hash();
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
            vec![
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
                    crate::queue::RouteLegRole::Participant,
                ),
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(2), cbuae),
                    crate::queue::RouteLegRole::Participant,
                ),
            ],
        );
        let (world, keypairs) = native_amx_test_world_with_keys();
        let coordinator_proposal = native_amx_test_coordinator_proposal_at_view(
            routing_plan.coordinator_route(),
            entrypoint_hash,
            42,
            0,
            &keypairs,
        );
        let mut source_id = [0_u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(entrypoint_hash.as_ref());
        let signer_count = iroha_sumeragi::types::quorum(keypairs.len()).max(1);
        let receipt = signed_native_amx_receipt_for_coordinator(
            source_id,
            entrypoint_hash,
            &routing_plan,
            coordinator_proposal.clone(),
            &keypairs,
            signer_count,
        );
        let descriptor = &coordinator_proposal.descriptor;
        let reservation = crate::queue::LaneQueueReservationKeyV1 {
            version: crate::queue::LaneQueueReservationKeyV1::VERSION,
            entrypoint_hash,
            queue_plan_admission_binding_hash: Hash::new(
                b"historical-native-amx-queue-plan-admission",
            ),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            reservation_owner_hash: Hash::new(b"historical-native-amx-reservation-owner"),
            proposal_identity_hash: coordinator_proposal.proposal_hash,
        };
        let producer = crate::lane_consensus::deterministic_lane_author(
            &descriptor.validator_set,
            descriptor.lane_block_height,
        )
        .cloned()
        .expect("fixture has a deterministic producer");
        let producer_keypair = keypairs
            .iter()
            .find(|keypair| keypair.public_key() == producer.public_key())
            .expect("fixture retains its producer key");
        let network_id = native_amx_test_network_id();
        let epoch = 0;
        let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
            network_id,
            epoch,
            coordinator_proposal.clone(),
            vec![entrypoint],
            vec![reservation],
            vec![routing_plan],
            vec![Some(receipt.clone())],
            producer,
            producer_keypair.private_key(),
        )
        .expect("fixture autonomous Native AMX payload");
        let mut ordered_keypairs = keypairs.iter().collect::<Vec<_>>();
        ordered_keypairs.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let validator_pops = ordered_keypairs
            .iter()
            .map(|keypair| {
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("fixture lane-validator PoP")
            })
            .collect::<Vec<_>>();
        let quorum = iroha_sumeragi::types::quorum(descriptor.validator_set.len());
        let selected_keypairs = ordered_keypairs
            .into_iter()
            .take(quorum)
            .collect::<Vec<_>>();
        let availability_body = crate::lane_consensus::lane_payload_availability_body(
            &payload,
            &coordinator_proposal,
            network_id,
            epoch,
        )
        .expect("fixture availability body");
        let prepare_body = coordinator_proposal.vote_body(CertPhase::Prepare);
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
                    .expect("fixture availability vote");
                crate::lane_consensus::LaneBlockVoteV1 {
                    body: prepare_body.clone(),
                    signer: PeerId::new(keypair.public_key().clone()),
                    bls_signature: checked_signature(
                        keypair.private_key(),
                        &prepare_body.signature_preimage(),
                    )
                    .payload()
                    .to_vec(),
                    payload_availability_vote: Some(availability_vote),
                }
            })
            .collect::<Vec<_>>();
        let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            prepare_body,
            descriptor.validator_set.clone(),
            &prepare_votes,
        )
        .expect("fixture lane PrepareQC");
        let commit_body = coordinator_proposal.vote_body(CertPhase::Commit);
        let commit_votes = selected_keypairs
            .iter()
            .map(|keypair| crate::lane_consensus::LaneBlockVoteV1 {
                body: commit_body.clone(),
                signer: PeerId::new(keypair.public_key().clone()),
                bls_signature: checked_signature(
                    keypair.private_key(),
                    &commit_body.signature_preimage(),
                )
                .payload()
                .to_vec(),
                payload_availability_vote: None,
            })
            .collect::<Vec<_>>();
        let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            commit_body,
            descriptor.validator_set.clone(),
            &commit_votes,
        )
        .expect("fixture lane CommitQC");
        let signer_pops = selected_keypairs
            .iter()
            .map(|keypair| {
                (
                    keypair.public_key().clone(),
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("fixture selected lane-validator PoP"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let certified = crate::kura::CertifiedLaneBlockArtifact::new(
            crate::lane_consensus::CommittedLaneBlockSession {
                proposal: coordinator_proposal.clone(),
                prepare_qc: prepare_qc.clone(),
                commit_qc,
            },
            signer_pops,
        );
        let autonomous = crate::kura::AutonomousLaneBlockArtifact {
            format: crate::kura::AutonomousLaneBlockArtifactFormat::Current,
            executable_payload: payload,
            availability_certificate: Some(
                crate::lane_consensus::DurableLanePayloadAvailabilityCertificateV1 {
                    certificate: prepare_qc,
                },
            ),
            view_checkpoint: None,
            new_view_certificates: Vec::new(),
        };
        let bundle = crate::kura::AutonomousLaneMergeBundleV1 {
            version: crate::kura::AutonomousLaneMergeBundleV1::VERSION,
            autonomous,
            certified,
        };
        let source_bundle = bundle
            .encode_framed()
            .expect("fixture historical source bundle");
        let active_lanes = historical_native_amx_test_active_lanes(&coordinator_proposal, &receipt);
        let authority = native_amx_test_authority(world, &keypairs);
        HistoricalNativeAmxSourceBundleFixture {
            bundle,
            source_bundle,
            active_lanes,
            authority,
            network_id,
            epoch,
        }
    }
    fn signed_domain_registration_tx(
        domains: &[(&str, &str)],
    ) -> (SignedTransaction, HashOf<SignedTransaction>) {
        signed_domain_registration_tx_with_admission_intent(
            domains,
            iroha_data_model::transaction::TransactionAdmissionIntent::Ordinary,
        )
    }
    fn signed_domain_registration_tx_with_admission_intent(
        domains: &[(&str, &str)],
        admission_intent: iroha_data_model::transaction::TransactionAdmissionIntent,
    ) -> (SignedTransaction, HashOf<SignedTransaction>) {
        let (authority_id, keypair) = gen_account_in("wonderland");
        let instructions = domains
            .iter()
            .map(|(name, dataspace_alias)| {
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new(*name, *dataspace_alias).expect("domain id"),
                )))
            })
            .collect::<Vec<_>>();
        let tx = TransactionBuilder::new(
            deterministic_test_network_id(0x09),
            authority_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .with_admission_intent(admission_intent)
        .sign(keypair.private_key());
        let tx_hash = AcceptedTransaction::prepare_signed_metadata(&tx).signed_hash;
        (tx, tx_hash)
    }
    fn native_amx_shared_participant_group_bundle() -> BlockExecutionContextBundle {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let coordinator = crate::queue::RoutingDecision::new(LaneId::new(1), paynet);
        let participant = crate::queue::RoutingDecision::new(LaneId::new(2), cbuae);
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            coordinator,
            vec![
                crate::queue::RouteLeg::new(coordinator, crate::queue::RouteLegRole::Participant),
                crate::queue::RouteLeg::new(participant, crate::queue::RouteLegRole::Participant),
            ],
        );
        let (first_tx, first_hash) = signed_domain_registration_tx(&[("merchant", "paynet")]);
        let (second_tx, second_hash) = signed_domain_registration_tx(&[("treasury", "cbuae")]);
        let first_entrypoint_hash = first_tx.hash_as_entrypoint();
        let second_entrypoint_hash = second_tx.hash_as_entrypoint();
        let mut first_source = [0_u8; Hash::LENGTH];
        first_source.copy_from_slice(first_hash.as_ref());
        let mut second_source = [0_u8; Hash::LENGTH];
        second_source.copy_from_slice(second_hash.as_ref());
        let (_, keypairs) = native_amx_test_world_with_keys();
        let mut first_receipt = signed_native_amx_receipt(
            first_source,
            first_entrypoint_hash,
            &routing_plan,
            42,
            &keypairs,
        );
        let mut second_receipt = signed_native_amx_receipt(
            second_source,
            second_entrypoint_hash,
            &routing_plan,
            42,
            &keypairs,
        );
        let mut participant_proposal = first_receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == participant.lane_id)
            .expect("participant leg")
            .participant_proposal
            .clone();
        participant_proposal.descriptor.accepted_candidate_indices = vec![0, 1];
        participant_proposal.descriptor.accepted_transaction_hashes = vec![
            Hash::from(first_entrypoint_hash),
            Hash::from(second_entrypoint_hash),
        ];
        participant_proposal.descriptor.descriptor_hash =
            participant_proposal.descriptor.computed_descriptor_hash();
        participant_proposal.proposal_hash = participant_proposal.computed_proposal_hash();
        let participant_settlement =
            iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
                participant.lane_id,
                participant.dataspace_id,
                participant_proposal.descriptor.lane_incarnation,
                participant_proposal.descriptor.lane_block_height,
                42,
                None,
                vec![first_source, second_source],
            )
            .expect("valid Native participant control");
        let participant_settlement_hash = participant_settlement
            .computed_hash()
            .expect("shared participant settlement hash");
        for receipt in [&mut first_receipt, &mut second_receipt] {
            let leg = receipt
                .legs
                .iter_mut()
                .find(|leg| leg.lane_id == participant.lane_id)
                .expect("participant leg");
            leg.participant_proposal = participant_proposal.clone();
            leg.participant_settlement = participant_settlement.clone();
            leg.participant_settlement_hash = participant_settlement_hash;
            for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
                body.participant_proposal_hash = participant_proposal.proposal_hash;
                body.participant_settlement_commitment = Hash::from(participant_settlement_hash);
            }
        }
        BlockExecutionContextBundle::new(vec![
            crate::queue::execution_context_for_routing_plan(first_entrypoint_hash, &routing_plan)
                .with_native_amx_receipt(first_receipt),
            crate::queue::execution_context_for_routing_plan(second_entrypoint_hash, &routing_plan)
                .with_native_amx_receipt(second_receipt),
        ])
    }
    #[test]
    fn native_amx_group_validation_accepts_shared_members_and_skips_coordinator() {
        let bundle = native_amx_shared_participant_group_bundle();
        let receipt = bundle.external[0]
            .native_amx_receipt
            .as_ref()
            .expect("native AMX receipt");
        let coordinator_leg = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == receipt.lane_id && leg.dataspace_id == receipt.dataspace_id)
            .expect("coordinator-route leg");
        let participant_leg = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id != receipt.lane_id || leg.dataspace_id != receipt.dataspace_id)
            .expect("separate participant leg");
        assert!(
            crate::native_amx::native_amx_participant_application_role(receipt, coordinator_leg,)
                == Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator)
        );
        assert!(
            crate::native_amx::native_amx_participant_application_role(receipt, participant_leg,)
                == Ok(crate::native_amx::NativeAmxParticipantApplicationRole::SeparateParticipant)
        );
        ValidBlock::validate_native_amx_participant_groups(&bundle)
            .expect("one exact two-member participant control should validate");
    }
    #[test]
    fn native_amx_group_validation_rejects_stale_same_route_identity() {
        let mut bundle = native_amx_shared_participant_group_bundle();
        let receipt = bundle.external[0]
            .native_amx_receipt
            .as_mut()
            .expect("native AMX receipt");
        let coordinator_leg = receipt
            .legs
            .iter_mut()
            .find(|leg| leg.lane_id == receipt.lane_id && leg.dataspace_id == receipt.dataspace_id)
            .expect("coordinator-route leg");
        coordinator_leg
            .participant_proposal
            .descriptor
            .lane_incarnation = Hash::new(b"stale coordinator incarnation");
        coordinator_leg
            .participant_proposal
            .descriptor
            .descriptor_hash = coordinator_leg
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        coordinator_leg.participant_proposal.proposal_hash = coordinator_leg
            .participant_proposal
            .computed_proposal_hash();
        coordinator_leg.participant_settlement =
            iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
                coordinator_leg.participant_settlement.lane_id(),
                coordinator_leg.participant_settlement.dataspace_id(),
                coordinator_leg
                    .participant_proposal
                    .descriptor
                    .lane_incarnation,
                coordinator_leg
                    .participant_settlement
                    .participant_lane_block_height(),
                coordinator_leg
                    .participant_settlement
                    .authority_context_height(),
                coordinator_leg
                    .participant_settlement
                    .previous_native_settlement_hash(),
                coordinator_leg.participant_settlement.source_ids().to_vec(),
            )
            .expect("valid conflicting Native control identity");
        coordinator_leg.participant_settlement_hash = coordinator_leg
            .participant_settlement
            .computed_hash()
            .expect("stale same-route settlement hashes");
        for body in [
            &mut coordinator_leg.prepare_qc.body,
            &mut coordinator_leg.commit_qc.body,
        ] {
            body.participant_lane_incarnation = coordinator_leg
                .participant_proposal
                .descriptor
                .lane_incarnation;
            body.participant_proposal_hash = coordinator_leg.participant_proposal.proposal_hash;
            body.participant_settlement_commitment =
                Hash::from(coordinator_leg.participant_settlement_hash);
        }
        assert!(matches!(
            ValidBlock::validate_native_amx_participant_groups(&bundle),
            Err(BlockValidationError::ExecutionContextInvalid(message))
                if message.contains("same-route leg differs from the coordinator identity")
        ));
    }
    #[test]
    fn native_amx_group_validation_rejects_partial_membership() {
        let mut bundle = native_amx_shared_participant_group_bundle();
        bundle.external.pop();
        assert!(matches!(
            ValidBlock::validate_native_amx_participant_groups(&bundle),
            Err(BlockValidationError::ExecutionContextInvalid(message))
                if message.contains("does not exactly cover its grouped block members")
        ));
    }
    #[test]
    fn native_amx_group_validation_rejects_conflicting_control() {
        let mut bundle = native_amx_shared_participant_group_bundle();
        let receipt = bundle.external[1]
            .native_amx_receipt
            .as_mut()
            .expect("native AMX receipt");
        let leg = receipt
            .legs
            .iter_mut()
            .find(|leg| leg.lane_id == LaneId::new(2))
            .expect("participant leg");
        leg.participant_proposal
            .descriptor
            .qc_mode_tag
            .push_str("-conflicting-group");
        leg.participant_proposal.descriptor.descriptor_hash = leg
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
        for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
            body.participant_proposal_hash = leg.participant_proposal.proposal_hash;
        }
        assert!(matches!(
            ValidBlock::validate_native_amx_participant_groups(&bundle),
            Err(BlockValidationError::ExecutionContextInvalid(message))
                if message.contains("conflicting grouped control evidence")
        ));
    }
    #[test]
    fn native_amx_autoscale_pop_policy_requires_the_exact_incarnation_pin() {
        let lane_id = LaneId::new(1);
        let mut elastic_lane = LaneConfig {
            id: lane_id,
            alias: "elastic-lane-1".to_owned(),
            ..LaneConfig::default()
        };
        elastic_lane
            .metadata
            .insert("autoscale.managed".to_owned(), "true".to_owned());
        elastic_lane
            .metadata
            .insert("autoscale.created_height".to_owned(), "1".to_owned());
        crate::state::attach_synthetic_autoscale_committee_for_test(&mut elastic_lane);
        let pinned = crate::state::autoscale_lane_pinned_committee_with_pops(&elastic_lane)
            .expect("synthetic autoscale lane carries a pin");
        let (peer, exact_pop) = pinned[0].clone();
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![LaneConfig::default(), elastic_lane.clone()],
        )
        .expect("autoscale Native AMX policy fixture catalog");
        let world = World::new();
        assert!(consensus_pop_matches_lane_authority(
            &nexus,
            &world.view(),
            lane_id,
            &peer,
            42,
            &exact_pop,
        ));
        let wrong_pop = &pinned[1].1;
        assert!(!consensus_pop_matches_lane_authority(
            &nexus,
            &world.view(),
            lane_id,
            &peer,
            42,
            wrong_pop,
        ));
        elastic_lane.metadata.remove("autoscale.committee_v1");
        nexus.lane_catalog =
            LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), elastic_lane])
                .expect("missing-pin Native AMX policy fixture catalog");
        assert!(
            !consensus_pop_matches_lane_authority(
                &nexus,
                &world.view(),
                lane_id,
                &peer,
                42,
                &exact_pop,
            ),
            "an autoscale lane must never fall back to an absent live-key record"
        );
    }
    include!("block/native_amx_receipt_regression_tests.rs");
    #[test]
    fn historical_native_amx_validation_checks_later_forged_leg_after_stale_predecessor() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
            vec![
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
                    crate::queue::RouteLegRole::Participant,
                ),
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(2), cbuae),
                    crate::queue::RouteLegRole::Participant,
                ),
            ],
        );
        let (world, keypairs) = native_amx_test_world_with_keys();
        let entrypoint_hash = tx.hash_as_entrypoint();
        let mut source_id = [0_u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(tx_hash.as_ref());
        let receipt =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            routing_plan.coordinator_route(),
            entrypoint_hash,
            42,
            &keypairs,
        );
        let authority = native_amx_test_authority(world, &keypairs);
        let stale_first_predecessor = NativeAmxStalePredecessorTestAuthority {
            inner: &authority,
            stale_lane_id: receipt.legs[0].lane_id,
        };
        let validate_current = |candidate: &NativeAmxReceipt| {
            validate_native_amx_receipt_against_plan(
                candidate,
                &coordinator_proposal,
                entrypoint_hash,
                &routing_plan,
                source_id,
                native_amx_test_network_id(),
                &dataspace_catalog,
                &stale_first_predecessor,
                Some(expected_native_amx_test_context(42)),
            )
        };
        let validate_historical = |candidate: &NativeAmxReceipt| {
            validate_historical_native_amx_receipt_against_plan(
                candidate,
                &coordinator_proposal,
                entrypoint_hash,
                &routing_plan,
                source_id,
                native_amx_test_network_id(),
                None,
                Some(expected_native_amx_test_context(42)),
            )
        };
        let current_error = validate_current(&receipt)
            .expect_err("admission must reject the stale first participant predecessor");
        assert!(
            current_error.contains("does not extend the exact durable predecessor"),
            "unexpected stale-predecessor rejection: {current_error}"
        );
        validate_historical(&receipt)
            .expect("historical validation should relax only current-predecessor freshness");
        let mut forged_later_leg = receipt;
        forged_later_leg.legs[1].commit_qc.bls_aggregate_signature[0] ^= 0x80;
        let current_error = validate_current(&forged_later_leg)
            .expect_err("admission still stops at the stale first participant predecessor");
        assert!(
            current_error.contains("does not extend the exact durable predecessor"),
            "unexpected admission rejection for the forged receipt: {current_error}"
        );
        let historical_error = validate_historical(&forged_later_leg)
            .expect_err("historical validation must authenticate the later participant leg");
        assert!(
            historical_error.contains("aggregate signature invalid"),
            "unexpected historical rejection for the forged later leg: {historical_error}"
        );
    }
    #[test]
    fn historical_native_amx_validation_uses_frozen_merge_routes_across_participant_drift() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
            vec![
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
                    crate::queue::RouteLegRole::Participant,
                ),
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(2), cbuae),
                    crate::queue::RouteLegRole::Participant,
                ),
            ],
        );
        let (world, keypairs) = native_amx_test_world_with_keys();
        let entrypoint_hash = tx.hash_as_entrypoint();
        let mut source_id = [0_u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(tx_hash.as_ref());
        let receipt =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            routing_plan.coordinator_route(),
            entrypoint_hash,
            42,
            &keypairs,
        );
        let active_lanes = historical_native_amx_test_active_lanes(&coordinator_proposal, &receipt);
        let validate_historical = |candidate: &NativeAmxReceipt, bindings: &[MergeLaneBinding]| {
            validate_historical_native_amx_receipt_against_plan(
                candidate,
                &coordinator_proposal,
                entrypoint_hash,
                &routing_plan,
                source_id,
                native_amx_test_network_id(),
                Some(bindings),
                Some(expected_native_amx_test_context(42)),
            )
        };
        validate_historical(&receipt, &active_lanes)
            .expect("the merge-QC lane snapshot authenticates grouped A+B evidence");
        let current_authority = native_amx_test_authority(world, &keypairs);
        let original_b_incarnation = receipt.legs[1].prepare_qc.body.participant_lane_incarnation;
        for (label, participant_incarnation, predecessor_is_current) in [
            ("advanced", Some(original_b_incarnation), false),
            ("retired", None, false),
            (
                "recreated B2",
                Some(Hash::new(b"native-amx-recreated-b2")),
                true,
            ),
        ] {
            let drifted = NativeAmxDriftedParticipantTestAuthority {
                inner: &current_authority,
                participant_lane_id: LaneId::new(2),
                participant_incarnation,
                participant_predecessor_is_current: predecessor_is_current,
            };
            assert!(
                validate_native_amx_receipt_against_plan(
                    &receipt,
                    &coordinator_proposal,
                    entrypoint_hash,
                    &routing_plan,
                    source_id,
                    native_amx_test_network_id(),
                    &native_amx_test_catalog(paynet, cbuae),
                    &drifted,
                    Some(expected_native_amx_test_context(42)),
                )
                .is_err(),
                "live admission must reject historical B evidence after B is {label}",
            );
            validate_historical(&receipt, &active_lanes).unwrap_or_else(|error| {
                panic!("frozen merge evidence must survive B being {label}: {error}")
            });
        }
        let mut forged_participant_binding = active_lanes.clone();
        forged_participant_binding
            .iter_mut()
            .find(|binding| binding.lane_id == LaneId::new(2))
            .expect("fixture B binding")
            .incarnation = Hash::new(b"forged-merge-active-lane-b");
        assert!(
            validate_historical(&receipt, &forged_participant_binding)
                .expect_err("forged B active-lane binding must fail")
                .contains("participant route/incarnation differs")
        );
        let mut forged_coordinator_binding = active_lanes.clone();
        forged_coordinator_binding
            .iter_mut()
            .find(|binding| binding.lane_id == LaneId::new(1))
            .expect("fixture A binding")
            .incarnation = Hash::new(b"forged-merge-active-lane-a");
        assert!(
            validate_historical(&receipt, &forged_coordinator_binding)
                .expect_err("forged A active-lane binding must fail")
                .contains("coordinator route/incarnation differs")
        );
        let mut forged_participant_qc = receipt;
        forged_participant_qc.legs[1]
            .commit_qc
            .bls_aggregate_signature[0] ^= 0x80;
        assert!(
            validate_historical(&forged_participant_qc, &active_lanes)
                .expect_err("forged historical B commit QC must fail")
                .contains("aggregate signature invalid")
        );
    }
    #[test]
    fn historical_native_amx_source_bundle_authenticates_every_evidence_layer() {
        let fixture = historical_native_amx_source_bundle_fixture();
        let decoded = validate_historical_native_amx_source_bundle(
            &fixture.source_bundle,
            fixture.network_id,
            fixture.epoch,
            HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes(&fixture.active_lanes),
        )
        .expect("complete historical source bundle must validate");
        assert_eq!(decoded, fixture.bundle);
        validate_historical_native_amx_source_bundle(
            &fixture.source_bundle,
            fixture.network_id,
            fixture.epoch,
            HistoricalNativeAmxSourceAuthority::CertifiedCoordinator {
                committee: &fixture.authority.committee,
                key_authority: &fixture.authority,
            },
        )
        .expect("bundle-only diagnostics must authenticate the still-active coordinator");
        let foreign_coordinator_key =
            checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
        let foreign_coordinator_authority = NativeAmxTestAuthority {
            world: World::new(),
            committee: vec![PeerId::new(foreign_coordinator_key.public_key().clone())],
        };
        assert!(
            validate_historical_native_amx_source_bundle(
                &fixture.source_bundle,
                fixture.network_id,
                fixture.epoch,
                HistoricalNativeAmxSourceAuthority::CertifiedCoordinator {
                    committee: &foreign_coordinator_authority.committee,
                    key_authority: &foreign_coordinator_authority,
                },
            )
            .expect_err("self-selected coordinator committee must fail closed")
            .contains("committee is not authoritative")
        );
        let mut forged_producer_bundle = fixture.bundle.clone();
        forged_producer_bundle
            .autonomous
            .executable_payload
            .producer_signature[0] ^= 0x80;
        let forged_producer_bytes = forged_producer_bundle
            .encode_framed()
            .expect("encode forged producer fixture");
        assert!(
            validate_historical_native_amx_source_bundle(
                &forged_producer_bytes,
                fixture.network_id,
                fixture.epoch,
                HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes(&fixture.active_lanes),
            )
            .expect_err("forged producer-authenticated bundle must fail")
            .contains("invalid autonomous executable payload")
        );
        let mut forged_lane_qc_bundle = fixture.bundle.clone();
        forged_lane_qc_bundle
            .certified
            .commit_qc
            .bls_aggregate_signature[0] ^= 0x80;
        let forged_lane_qc_bytes = forged_lane_qc_bundle
            .encode_framed()
            .expect("encode forged lane-QC fixture");
        assert!(
            validate_historical_native_amx_source_bundle(
                &forged_lane_qc_bytes,
                fixture.network_id,
                fixture.epoch,
                HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes(&fixture.active_lanes),
            )
            .expect_err("forged lane CommitQC must fail")
            .contains("invalid commit lane block QC aggregate")
        );
        let mut forged_receipt_bundle = fixture.bundle.clone();
        forged_receipt_bundle
            .autonomous
            .executable_payload
            .native_amx_receipts[0]
            .as_mut()
            .expect("fixture Native AMX receipt")
            .legs[1]
            .commit_qc
            .bls_aggregate_signature[0] ^= 0x80;
        let forged_receipt_bytes = forged_receipt_bundle
            .encode_framed()
            .expect("encode forged participant-control fixture");
        assert!(
            validate_historical_native_amx_source_bundle(
                &forged_receipt_bytes,
                fixture.network_id,
                fixture.epoch,
                HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes(&fixture.active_lanes),
            )
            .is_err(),
            "a forged participant control must fail the exact source trust chain",
        );
    }
    #[test]
    fn native_amx_receipt_retains_pop_evidence_after_key_retirement() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
            vec![
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
                    crate::queue::RouteLegRole::Participant,
                ),
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(2), cbuae),
                    crate::queue::RouteLegRole::Participant,
                ),
            ],
        );
        let (_, keypairs) = native_amx_test_world_with_keys();
        let entrypoint_hash = tx.hash_as_entrypoint();
        let mut source_id = [0_u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(tx_hash.as_ref());
        let receipt =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            routing_plan.coordinator_route(),
            entrypoint_hash,
            42,
            &keypairs,
        );
        let historical_authority = NativeAmxTestAuthority {
            world: World::new(),
            committee: receipt.legs[0].prepare_qc.validator_set().to_vec(),
        };
        let validate = |candidate: &NativeAmxReceipt| {
            validate_native_amx_receipt_against_plan(
                candidate,
                &coordinator_proposal,
                entrypoint_hash,
                &routing_plan,
                source_id,
                native_amx_test_network_id(),
                &native_amx_test_catalog(paynet, cbuae),
                &historical_authority,
                Some(expected_native_amx_test_context(42)),
            )
        };
        validate(&receipt).expect("embedded historical PoPs survive live-key retirement");
        let mut tampered = receipt;
        let qc = &mut tampered.legs[0].prepare_qc;
        let mut validator_set_pops = qc.validator_set_pops().to_vec();
        validator_set_pops[0][0] ^= 0x80;
        *qc = NativeAmxAttestationQcV2::try_new(
            qc.body,
            qc.validator_set_hash_version,
            qc.validator_set_hash,
            qc.validator_set().to_vec(),
            validator_set_pops,
            qc.signers_bitmap.clone(),
            qc.bls_aggregate_signature.clone(),
        )
        .expect("tampered fixture validator set and proofs remain aligned");
        assert!(
            validate(&tampered).is_err(),
            "tampered historical PoP must fail closed"
        );
    }
    include!("block/native_amx_exact_quorum_cardinality_tests.rs");
    #[test]
    fn native_amx_receipt_validation_rejects_malformed_qcs() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
            vec![
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
                    crate::queue::RouteLegRole::Participant,
                ),
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(2), cbuae),
                    crate::queue::RouteLegRole::Participant,
                ),
            ],
        );
        let (world, keypairs) = native_amx_test_world_with_keys();
        let entrypoint_hash = tx.hash_as_entrypoint();
        let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(tx_hash.as_ref());
        let receipt =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            routing_plan.coordinator_route(),
            entrypoint_hash,
            42,
            &keypairs,
        );
        let authority = native_amx_test_authority(world, &keypairs);
        let validate = |receipt: &NativeAmxReceipt| {
            validate_native_amx_receipt_against_plan(
                receipt,
                &coordinator_proposal,
                entrypoint_hash,
                &routing_plan,
                source_id,
                native_amx_test_network_id(),
                &dataspace_catalog,
                &authority,
                Some(expected_native_amx_test_context(42)),
            )
        };
        let mut missing_leg = receipt.clone();
        missing_leg.legs.pop();
        assert!(
            validate(&missing_leg)
                .expect_err("missing leg must fail")
                .contains("missing or extra")
        );
        let mut wrong_phase = receipt.clone();
        wrong_phase.legs[0].prepare_qc = wrong_phase.legs[0].commit_qc.clone();
        let error = validate(&wrong_phase).expect_err("wrong phase must fail");
        assert!(
            error.contains("attestation phase mismatch"),
            "unexpected wrong-phase rejection: {error}"
        );
        let mut wrong_digest = receipt.clone();
        let mut digest = [0_u8; iroha_crypto::Hash::LENGTH];
        digest[0] = 0x42;
        digest[iroha_crypto::Hash::LENGTH - 1] = 0x01;
        let wrong_plan_digest = Hash::prehashed(digest);
        wrong_digest.legs[0].prepare_qc.body.plan_digest = wrong_plan_digest;
        wrong_digest.legs[0].commit_qc.body.plan_digest = wrong_plan_digest;
        let error = validate(&wrong_digest).expect_err("wrong digest must fail");
        assert!(
            error.contains("plan digest mismatch"),
            "unexpected wrong-digest rejection: {error}"
        );
        let mut excessive_participants = receipt.clone();
        excessive_participants.legs =
            vec![receipt.legs[0].clone(); crate::native_amx::MAX_NATIVE_AMX_PLAN_LEGS];
        assert!(
            validate(&excessive_participants)
                .expect_err("coordinator-inclusive leg cap must reject 256 participants")
                .contains("participant-leg cap")
        );
        let mut bad_bitmap = receipt;
        bad_bitmap.legs[0].prepare_qc.signers_bitmap.push(0);
        assert!(
            validate(&bad_bitmap)
                .expect_err("bad signer bitmap must fail")
                .contains("signer bitmap length mismatch")
        );
        let assert_context_replay_rejected = |replayed: NativeAmxReceipt, label: &str| {
            let error = validate(&replayed).expect_err(label);
            assert!(
                error.contains("attestation context mismatch"),
                "unexpected {label} rejection: {error}"
            );
        };
        let mut foreign_context =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        for leg in &mut foreign_context.legs {
            for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
                body.round.context_id = iroha_data_model::block::consensus_v2::HeightContextId(
                    HashOf::from_untyped_unchecked(Hash::new(b"foreign-native-amx-context")),
                );
            }
        }
        assert_context_replay_rejected(foreign_context, "foreign context must fail");
        let mut foreign_height =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        for leg in &mut foreign_height.legs {
            leg.prepare_qc.body.round.height = 41;
            leg.commit_qc.body.round.height = 41;
        }
        assert_context_replay_rejected(foreign_height, "foreign height must fail");
        let mut foreign_view =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        for leg in &mut foreign_view.legs {
            leg.prepare_qc.body.round.view = 1;
            leg.commit_qc.body.round.view = 1;
        }
        assert_context_replay_rejected(foreign_view, "foreign origin view must fail");
        let mut foreign_epoch =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        for leg in &mut foreign_epoch.legs {
            leg.prepare_qc.body.epoch = 1;
            leg.commit_qc.body.epoch = 1;
        }
        assert_context_replay_rejected(foreign_epoch, "foreign epoch must fail");
        let mut stale_participant_incarnation =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        let stale_leg = &mut stale_participant_incarnation.legs[1];
        let stale_incarnation = Hash::new(b"retired-native-amx-participant-incarnation");
        stale_leg.participant_proposal.descriptor.lane_incarnation = stale_incarnation;
        stale_leg.participant_proposal.descriptor.descriptor_hash = stale_leg
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        stale_leg.participant_proposal.proposal_hash =
            stale_leg.participant_proposal.computed_proposal_hash();
        stale_leg.participant_settlement =
            iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
                stale_leg.participant_settlement.lane_id(),
                stale_leg.participant_settlement.dataspace_id(),
                stale_incarnation,
                stale_leg
                    .participant_settlement
                    .participant_lane_block_height(),
                stale_leg.participant_settlement.authority_context_height(),
                stale_leg
                    .participant_settlement
                    .previous_native_settlement_hash(),
                stale_leg.participant_settlement.source_ids().to_vec(),
            )
            .expect("valid conflicting Native control identity");
        stale_leg.participant_settlement_hash = stale_leg
            .participant_settlement
            .computed_hash()
            .expect("stale participant settlement hashes");
        for body in [
            &mut stale_leg.prepare_qc.body,
            &mut stale_leg.commit_qc.body,
        ] {
            body.participant_lane_incarnation = stale_incarnation;
            body.participant_proposal_hash = stale_leg.participant_proposal.proposal_hash;
            body.participant_settlement_commitment =
                Hash::from(stale_leg.participant_settlement_hash);
        }
        let error = validate(&stale_participant_incarnation)
            .expect_err("retired participant incarnation must fail");
        assert!(
            error.contains("unexpected participant lane 2 dataspace 8"),
            "unexpected participant-incarnation rejection: {error}"
        );
        let mut foreign_committee =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        for leg in &mut foreign_committee.legs {
            for qc in [&mut leg.prepare_qc, &mut leg.commit_qc] {
                let mut validator_set = qc.validator_set().to_vec();
                let mut validator_set_pops = qc.validator_set_pops().to_vec();
                validator_set.pop();
                validator_set_pops.pop();
                let validator_set_hash = HashOf::new(&validator_set);
                let mut body = qc.body;
                body.participant_validator_set_hash = validator_set_hash;
                body.participant_validator_count =
                    u32::try_from(validator_set.len()).expect("fixture validator count");
                body.participant_min_quorum =
                    u32::try_from(iroha_sumeragi::types::quorum(validator_set.len()).max(1))
                        .expect("fixture participant quorum");
                *qc = NativeAmxAttestationQcV2::try_new(
                    body,
                    qc.validator_set_hash_version,
                    validator_set_hash,
                    validator_set,
                    validator_set_pops,
                    qc.signers_bitmap.clone(),
                    qc.bls_aggregate_signature.clone(),
                )
                .expect("foreign committee fixture validator set and proofs remain aligned");
            }
        }
        let error = validate(&foreign_committee).expect_err("foreign committee must fail");
        assert!(
            error.contains("not the authoritative height committee"),
            "unexpected committee rejection: {error}"
        );
        let old_origin_receipt =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        validate(&old_origin_receipt)
            .expect("the receipt must validate against the immutable block-origin view");
        // This helper deliberately receives only the immutable body-origin
        // round. Later proposal-view admission is exercised separately by the
        // body-store locked-reproposal tests; it must not be threaded into this
        // comparison or old locked bodies would be rejected.
    }
    #[test]
    fn native_amx_receipt_validation_rejects_participant_set_drift() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let routing_plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
            vec![
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
                    crate::queue::RouteLegRole::Participant,
                ),
                crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(LaneId::new(2), cbuae),
                    crate::queue::RouteLegRole::Participant,
                ),
            ],
        );
        let (world, keypairs) = native_amx_test_world_with_keys();
        let entrypoint_hash = tx.hash_as_entrypoint();
        let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
        source_id.copy_from_slice(tx_hash.as_ref());
        let receipt =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            routing_plan.coordinator_route(),
            entrypoint_hash,
            42,
            &keypairs,
        );
        let authority = native_amx_test_authority(world, &keypairs);
        let validate = |receipt: &NativeAmxReceipt| {
            validate_native_amx_receipt_against_plan(
                receipt,
                &coordinator_proposal,
                entrypoint_hash,
                &routing_plan,
                source_id,
                native_amx_test_network_id(),
                &dataspace_catalog,
                &authority,
                Some(expected_native_amx_test_context(42)),
            )
        };
        let mut duplicate_participant = receipt.clone();
        duplicate_participant.legs[1] = duplicate_participant.legs[0].clone();
        assert!(
            validate(&duplicate_participant)
                .expect_err("duplicate participant leg must fail")
                .contains("duplicates participant"),
            "duplicate native AMX participant legs must fail before QC material can be reused"
        );
        let mut reordered_participants = receipt.clone();
        reordered_participants.legs.swap(0, 1);
        assert!(
            validate(&reordered_participants)
                .expect_err("reordered participant legs must fail")
                .contains("reordered"),
            "native AMX participant legs must retain canonical routing-plan order"
        );
        let mut unexpected_participant = receipt;
        let unexpected_leg = &mut unexpected_participant.legs[1];
        unexpected_leg.lane_id = LaneId::new(99);
        unexpected_leg.dataspace_id = DataSpaceId::new(99);
        unexpected_leg.participant_proposal.descriptor.lane_id = unexpected_leg.lane_id;
        unexpected_leg.participant_proposal.descriptor.dataspace_id = unexpected_leg.dataspace_id;
        unexpected_leg
            .participant_proposal
            .descriptor
            .descriptor_hash = unexpected_leg
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        unexpected_leg.participant_proposal.proposal_hash =
            unexpected_leg.participant_proposal.computed_proposal_hash();
        unexpected_leg.participant_settlement =
            iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
                unexpected_leg.lane_id,
                unexpected_leg.dataspace_id,
                unexpected_leg.participant_settlement.lane_incarnation(),
                unexpected_leg
                    .participant_settlement
                    .participant_lane_block_height(),
                unexpected_leg
                    .participant_settlement
                    .authority_context_height(),
                None,
                unexpected_leg.participant_settlement.source_ids().to_vec(),
            )
            .expect("valid conflicting Native control identity");
        unexpected_leg.participant_settlement_hash = unexpected_leg
            .participant_settlement
            .computed_hash()
            .expect("unexpected participant settlement hashes");
        for body in [
            &mut unexpected_leg.prepare_qc.body,
            &mut unexpected_leg.commit_qc.body,
        ] {
            body.participant_lane_id = unexpected_leg.lane_id;
            body.participant_dataspace_id = unexpected_leg.dataspace_id;
            body.participant_proposal_hash = unexpected_leg.participant_proposal.proposal_hash;
            body.participant_settlement_commitment =
                Hash::from(unexpected_leg.participant_settlement_hash);
        }
        assert!(
            validate(&unexpected_participant)
                .expect_err("unexpected participant leg must fail")
                .contains("unexpected participant lane 99 dataspace 99"),
            "native AMX receipts must not add participant legs outside the canonical routing plan"
        );
    }
    include!("block/native_amx_and_dag_tests.rs");
    fn state_with_transaction_policy(
        chain_id: &ChainId,
        authority: &AccountId,
        require_height_ttl: bool,
        require_sequence: bool,
    ) -> State {
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(authority);
        let account = Account::new(authority.clone()).build(authority);
        let mut world = World::with([domain], [account], []);
        let mut params = iroha_data_model::parameter::system::Parameters::default();
        params.transaction = params
            .transaction
            .with_ingress_enforcement(require_height_ttl, require_sequence);
        world.parameters = mv::cell::Cell::new(params);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain_and_network_id_for_testing(
            world,
            kura,
            query_handle,
            chain_id.clone(),
            deterministic_test_network_id(0x0B),
        );
        install_test_lane_manifests(&state);
        state
    }
    #[test]
    fn rejected_prepared_overlay_carries_confidential_work_into_block_budget() {
        let chain_id = ChainId::from("rejected-overlay-confidential-budget");
        let (authority, keypair) = gen_account_in("wonderland");
        let mut state = state_with_transaction_policy(&chain_id, &authority, false, false);
        // Generic proof verification requires an explicitly installed executor.
        // This deterministic policy admits the instruction; Core still enforces
        // its production circuit registry, proof-work budgets, and registered VK.
        let verdict = norito::codec::Encode::encode(&Ok::<(), ValidationFail>(()));
        let executor_program = crate::executor::build_program_from_encoded_result(&verdict);
        let executor = crate::executor::LoadedExecutor::load(
            iroha_data_model::executor::Executor::new(IvmBytecode::from_compiled(executor_program)),
        )
        .expect("load proof-verification executor policy");
        state.world.executor =
            mv::cell::Cell::new(crate::executor::Executor::UserProvided(executor));
        let mut pipeline = state.pipeline.clone();
        pipeline.parallel_overlay = true;
        pipeline.workers = 2;
        state.set_pipeline(pipeline);
        let mut zk = state.zk.clone();
        zk.max_confidential_ops_per_block = 1;
        zk.max_verify_calls_per_block = 1;
        zk.max_verify_calls_per_tx = 2;
        zk.max_proof_bytes_block = 1_000_000;
        zk.max_proof_size_bytes = 1_000_000;
        state
            .set_zk(zk)
            .expect("empty state accepts focused confidential limits");

        let fixture = crate::zk::test_utils::halo2_fixture_envelope(
            "halo2/pasta/ipa/kaigi-usage-v1",
            [0_u8; 32],
        );
        let proof = fixture.proof_box("halo2/ipa");
        let proof_bytes = u64::try_from(proof.bytes.len()).expect("proof length fits u64");
        let instruction: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(
            iroha_data_model::proof::ProofAttachment::new_ref(
                "halo2/ipa".into(),
                proof,
                iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "missing-overlay-vk"),
            ),
        )
        .into();
        let expected_confidential_gas = crate::gas::confidential_gas_cost(&instruction);
        let transaction = TransactionBuilder::new(
            state.network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());
        state
            .seed_genesis_for_testing()
            .expect("authenticate ordinary fixture predecessor");
        let block = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(
            transaction,
        ))])
        .chain(0, state.view().latest_block().as_deref())
        .sign(keypair.private_key())
        .unpack(|_| {});
        let mut state_block = state.block(block.header());

        let valid = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results = valid
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .map(|(_, _, result)| result.0.clone())
            .collect::<Vec<_>>();
        assert!(
            matches!(
                results.as_slice(),
                [Err(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(message)
                )))] if message.as_ref() == "proof must reference a registered verifying key reference; inline verifying keys are not supported"
            ),
            "missing verifying key must reject after attempting its proof: {results:?}"
        );
        assert_eq!(state_block.zk_confidential_ops_in_block, 1);
        assert_eq!(state_block.zk_verify_calls_in_block, 1);
        assert_eq!(state_block.zk_proof_bytes_in_block, proof_bytes);
        assert_eq!(
            state_block.confidential_gas_used_in_block,
            expected_confidential_gas
        );

        let mut next = state_block.transaction();
        let error = next
            .register_confidential_proof(1)
            .expect_err("the rejected overlay must exhaust the one-operation block budget");
        assert!(
            matches!(
                error,
                iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
                    iroha_data_model::isi::error::InvalidParameterError::SmartContract(message)
                ) if message == "confidential verify calls per block exceeded"
            ),
            "the next proof must fail the exact exhausted block verification quota"
        );
    }
    #[test]
    fn ordinary_prepared_overlay_block_cap_prevents_post_cap_execution() {
        for parallel_apply in [false, true] {
            let chain_id =
                ChainId::try_from(format!("ordinary-overlay-parent-gas-{parallel_apply}"))
                    .expect("canonical test chain id");
            let (authority, keypair) = gen_account_in("wonderland");
            let mut state = state_with_transaction_policy(&chain_id, &authority, false, false);
            let mut pipeline = state.pipeline.clone();
            pipeline.parallel_apply = parallel_apply;
            pipeline.parallel_overlay = true;
            pipeline.workers = 2;
            state.set_pipeline(pipeline);
            let instruction =
                InstructionBox::from(Log::new(Level::INFO, "meter ordinary overlay".into()));
            let expected_gas = crate::gas::meter_instructions(core::slice::from_ref(&instruction));
            assert!(expected_gas > 0, "the fixture must consume gas");
            let accepted = [0_u64, 1_u64, 2_u64]
                .into_iter()
                .map(|creation_time_ms| {
                    let mut builder = TransactionBuilder::new(
                        state.network_id,
                        authority.clone(),
                        iroha_data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    );
                    builder.set_creation_time(Duration::from_millis(creation_time_ms));
                    let transaction = builder
                        .with_instructions([instruction.clone()])
                        .sign(keypair.private_key());
                    AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
                })
                .collect::<Vec<_>>();
            let previous = previous_block_at_height(1);
            finalize_test_genesis_assets(&state, &previous);
            let (_clock, time_source) = TimeSource::new_mock(Duration::from_millis(10));
            let block = BlockBuilder::new_with_time_source(accepted, time_source)
                .chain(1, Some(&previous))
                .sign(keypair.private_key())
                .unpack(|_| {});
            let mut state_block = state.block(block.header());
            state_block.gas_limit_per_block = expected_gas;

            let valid = block
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {});
            let results = valid
                .as_ref()
                .network_entrypoints()
                .enumerate()
                .map(|(index, entrypoint)| {
                    let (output_index, output) = valid
                        .as_ref()
                        .network_output_at(
                            u32::try_from(index).expect("fixture Network index fits u32"),
                        )
                        .expect("every queried input has its explicit Network output");
                    assert_eq!(usize::try_from(output_index).unwrap(), index);
                    (index, entrypoint, &output.result)
                })
                .map(|(_, _, result)| result.0.clone())
                .collect::<Vec<_>>();
            assert_eq!(
                results.iter().filter(|result| result.is_ok()).count(),
                1,
                "exactly one overlay must fit with parallel_apply={parallel_apply}: {results:?}"
            );
            assert_eq!(
                results
                    .iter()
                    .filter(|result| matches!(
                        result,
                        Err(TransactionRejectionReason::Validation(
                            ValidationFail::NotPermitted(message)
                        )) if message.contains("block gas limit exceeded")
                    ))
                    .count(),
                2,
                "every overlay after the cap must fail before execution with parallel_apply={parallel_apply}: {results:?}"
            );
            assert_eq!(
                state_block.gas_used_in_block, expected_gas,
                "the rejected overlay must fail its base-gas reservation before executing work"
            );
        }
    }
    #[test]
    fn rejected_prepared_overlay_accounts_full_gas_exactly_once() {
        let chain_id = ChainId::from("rejected-overlay-exact-gas");
        let (authority, keypair) = gen_account_in("wonderland");
        let (missing_account, _) = gen_account_in("missing");
        let mut state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let mut pipeline = state.pipeline.clone();
        pipeline.parallel_overlay = true;
        pipeline.workers = 2;
        state.set_pipeline(pipeline);
        let instruction = InstructionBox::from(SetKeyValue::account(
            missing_account,
            "rejected_overlay_marker".parse().expect("metadata key"),
            Json::new("must roll back"),
        ));
        let expected_gas = crate::gas::meter_instructions(core::slice::from_ref(&instruction));
        assert!(expected_gas > 0, "the fixture must consume gas");
        let transaction = TransactionBuilder::new(
            state.network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());
        let previous = previous_block_at_height(1);
        finalize_test_genesis_assets(&state, &previous);
        let block = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(
            transaction,
        ))])
        .chain(1, Some(&previous))
        .sign(keypair.private_key())
        .unpack(|_| {});
        let mut state_block = state.block(block.header());
        state_block.gas_limit_per_block = expected_gas.saturating_mul(4);

        let valid = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results = valid
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .map(|(_, _, result)| result.0.clone())
            .collect::<Vec<_>>();
        assert!(matches!(results.as_slice(), [Err(_)]), "{results:?}");
        assert_eq!(state_block.gas_used_in_block, expected_gas);
    }
    fn add_pipeline_metadata_trigger(
        world: &mut World,
        authority: &AccountId,
        trigger_id: &str,
        key: Name,
        filter: PipelineEventFilterBox,
    ) {
        let mut action = crate::smartcontracts::triggers::specialized::SpecializedAction::new(
            vec![InstructionBox::from(SetKeyValue::account(
                authority.clone(),
                key,
                Json::new("ok"),
            ))],
            Repeats::Exactly(1),
            authority.clone(),
            filter,
        )
        .expect("test pipeline-trigger action satisfies its authority invariant");
        // This helper bypasses `Register<Trigger>`, so seed the lifecycle marker
        // that normal registration would add. Height zero models an incarnation
        // created before every non-genesis block exercised by these tests.
        action.metadata.insert(
            crate::smartcontracts::isi::triggers::TRIGGER_REGISTERED_BLOCK_HEIGHT_METADATA_KEY
                .parse()
                .expect("valid trigger lifecycle metadata key"),
            Json::from(0_u64),
        );
        let mut trigger_block = world.triggers.block();
        let mut trigger_transaction = trigger_block.transaction();
        trigger_transaction
            .add_pipeline_trigger(
                crate::smartcontracts::triggers::specialized::SpecializedTrigger::new(
                    trigger_id.parse().expect("trigger id"),
                    action,
                ),
            )
            .expect("add pipeline trigger");
        trigger_transaction.apply();
        trigger_block.commit();
    }
    pub(super) fn previous_block_at_height(height: u64) -> SignedBlock {
        let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(NonZeroU64::new(height).expect("non-zero height"));
        });
        latest_valid.into()
    }
    fn validation_error_message(block: &SignedBlock) -> String {
        block
            .output_results()
            .enumerate()
            .filter_map(|(index, result)| result.as_ref().err().map(|reason| (index, reason)))
            .next()
            .map(|(_, err)| format!("{err:?}"))
            .expect("block must contain a transaction error")
    }
    pub(super) fn sealed_set_key_entrypoints(
        network_id: NetworkId,
        authority: &AccountId,
        keypair: &KeyPair,
        reveal_after_height: u64,
        reveal_deadline_height: u64,
        metadata_key: Name,
    ) -> (TransactionEntrypoint, TransactionEntrypoint) {
        let mut builder = TransactionBuilder::new(
            network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::ZERO);
        let signed = builder
            .with_instructions([SetKeyValue::account(
                authority.clone(),
                metadata_key,
                Json::new("revealed"),
            )])
            .sign(keypair.private_key());
        sealed_entrypoints_from_signed(
            network_id,
            authority,
            keypair,
            reveal_after_height,
            reveal_deadline_height,
            signed,
        )
    }
    fn sealed_entrypoints_from_signed(
        network_id: NetworkId,
        authority: &AccountId,
        keypair: &KeyPair,
        reveal_after_height: u64,
        reveal_deadline_height: u64,
        signed: SignedTransaction,
    ) -> (TransactionEntrypoint, TransactionEntrypoint) {
        let salt = [0x5A; 32];
        let commitment = compute_sealed_transaction_commitment(
            &network_id,
            &signed,
            salt,
            reveal_deadline_height,
        );
        let payload = SealedTransactionCommitmentPayload::new(
            network_id,
            authority.clone(),
            commitment,
            reveal_after_height,
            reveal_deadline_height,
            None,
        );
        let signed_commitment =
            SignedSealedTransactionCommitment::sign(payload, keypair.private_key());
        let reveal = SealedTransactionReveal::new(commitment, signed, salt);
        (
            TransactionEntrypoint::SealedCommitment(signed_commitment),
            TransactionEntrypoint::SealedReveal(reveal),
        )
    }
    #[test]
    fn block_validation_external_only_records_entrypoint_hash_without_fallback() {
        let chain_id = ChainId::from("external-only-borrowed-validation");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let signed = TransactionBuilder::new(
            state.network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "external-only".to_owned())])
        .sign(keypair.private_key());
        let entrypoint_hash = TransactionEntrypoint::External(signed.clone()).hash();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed));
        state
            .seed_genesis_for_testing()
            .expect("authenticate ordinary fixture predecessor");
        let block = BlockBuilder::new(vec![accepted])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());
        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results: Vec<_> = valid_block
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid_block
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.hash(), entrypoint_hash);
        assert!(
            results[0].2.0.is_ok(),
            "external-only transaction must execute successfully: {:?}",
            results[0].2
        );
    }
    #[test]
    fn external_executables_keep_one_canonical_network_source() {
        let network_id = deterministic_test_network_id(0x0C);
        let (authority, keypair) = gen_account_in("wonderland");
        let make_block = |tx: SignedTransaction| {
            BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(tx))])
                .chain(0, None)
                .sign(keypair.private_key())
                .unpack(|_| {})
                .into()
        };
        let instructions_tx = TransactionBuilder::new(
            network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "plain".to_owned())])
        .sign(keypair.private_key());
        let instructions_block: SignedBlock = make_block(instructions_tx);
        assert!(
            matches!(
                instructions_block.network_entrypoint_at(0),
                Some(TransactionEntrypoint::External(_))
            ) && instructions_block.network_entrypoint_count() == 1,
            "plain Instructions retain their actual signed Network input"
        );
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &network_id,
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let contract_tx = TransactionBuilder::new(
            network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::ContractCall(
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address,
                expected_code_hash: Hash::new(b"overlay-routing-contract-code"),
                entrypoint: "increment".to_owned(),
                arguments: None,
            },
        ))
        .sign(keypair.private_key());
        let contract_block: SignedBlock = make_block(contract_tx);
        assert!(
            matches!(
                contract_block.network_entrypoint_at(0),
                Some(TransactionEntrypoint::External(_))
            ) && contract_block.network_entrypoint_count() == 1,
            "ContractCall retains its actual signed Network input"
        );
        let ivm_tx = TransactionBuilder::new(
            network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![0x01])))
        .sign(keypair.private_key());
        let ivm_block: SignedBlock = make_block(ivm_tx);
        assert!(
            matches!(
                ivm_block.network_entrypoint_at(0),
                Some(TransactionEntrypoint::External(_))
            ) && ivm_block.network_entrypoint_count() == 1,
            "raw IVM retains its actual signed Network input"
        );
        let proved_tx = TransactionBuilder::new(
            network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
            overlay: Vec::<InstructionBox>::new().into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        }))
        .sign(keypair.private_key());
        let proved_block: SignedBlock = make_block(proved_tx);
        assert!(
            matches!(
                proved_block.network_entrypoint_at(0),
                Some(TransactionEntrypoint::External(_))
            ) && proved_block.network_entrypoint_count() == 1,
            "IvmProved retains its actual signed Network input; execution still verifies the proof"
        );
    }
    #[test]
    fn block_overlay_rejects_protected_contract_call_without_persisting_state() {
        let chain_id = ChainId::from("protected-contract-overlay");
        let (authority, keypair) = gen_account_in("wonderland");
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").expect("valid domain"))
                .build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let mut world = World::with([domain], [account], []);
        let source = r#"
seiyaku GuardedOverlay {
  state StateMap<int, int> Values;

  kotoage fn write(int value) authorize("CanWriteGuardedOverlay") {
    Values[0] = value;
  }
}
"#;
        let (program, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(source)
            .expect("compile protected overlay contract");
        let interface = ivm::ProgramMetadata::parse(&program)
            .expect("parse protected contract")
            .contract_interface
            .expect("compiled contract interface");
        let code_hash = ivm::contract_code_hash(&program);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        world.contract_code.insert(code_hash, program);
        world
            .contract_manifests
            .insert(code_hash, manifest.signed(&keypair));
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let contract_subject = contract_address.subject_id();
        world.accounts.insert(
            contract_subject.clone(),
            iroha_data_model::account::AccountValue::new(
                iroha_data_model::account::AccountDetails::default(),
            ),
        );
        world.contract_subject_bindings.insert(
            contract_address.clone(),
            crate::smartcontracts::code::ContractSubjectBinding::new_direct(
                &contract_address,
                authority.clone(),
            )
            .with_active_code_hash(code_hash),
        );
        world
            .contract_subject_addresses
            .insert(contract_subject, contract_address.clone());
        let state = State::new_with_chain_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        );
        install_test_lane_manifests(&state);
        let payload = Json::new(norito::json!({ "value": "9" }));
        let schema = interface
            .entrypoints
            .iter()
            .find(|descriptor| descriptor.name == "write")
            .and_then(|descriptor| descriptor.argument_schema.as_ref());
        let arguments = crate::executor::encode_contract_argument_record(schema, Some(&payload))
            .expect("encode guarded arguments")
            .map(iroha_data_model::transaction::executable::ContractArgumentRecord::try_new)
            .transpose()
            .expect("bounded guarded arguments");
        let transaction = TransactionBuilder::new(
            state.network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(
                Vec::new(),
                core::num::NonZeroU64::new(100_000),
            ),
        )
        .with_executable(Executable::ContractCall(
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address,
                expected_code_hash: code_hash,
                entrypoint: "write".to_owned(),
                arguments,
            },
        ))
        .sign(keypair.private_key());
        state
            .seed_genesis_for_testing()
            .expect("authenticate ordinary fixture predecessor");
        let block = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(
            transaction,
        ))])
        .chain(0, state.view().latest_block().as_deref())
        .sign(keypair.private_key())
        .unpack(|_| {});
        let mut state_block = state.block(block.header());
        let initial_smart_contract_state = state_block
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let valid = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results = valid
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .map(|(_, _, result)| result.0.clone())
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].as_ref().is_err_and(|error| matches!(
                error,
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message))
                    if message.contains("CanWriteGuardedOverlay")
            )),
            "protected overlay call must be rejected with its stable permission name: {results:?}"
        );
        let final_smart_contract_state = state_block
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            final_smart_contract_state, initial_smart_contract_state,
            "a denied overlay must not persist any contract state"
        );
    }
    #[test]
    fn block_validation_reprepares_stale_contract_state_read_modify_write() {
        let chain_id = ChainId::from("durable-state-read-validation");
        let (alice, alice_keypair) = gen_account_in("wonderland");
        let (bob, bob_keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(&alice);
        let alice_account = Account::new(alice.clone()).build(&alice);
        let bob_account = Account::new(bob.clone()).build(&alice);
        let mut world = World::with([domain], [alice_account, bob_account], []);
        for authority in [&alice, &bob] {
            let mut permissions = iroha_data_model::permission::Permissions::new();
            assert!(
                permissions.insert(iroha_data_model::permission::Permission::new(
                    "CanEnactGovernance".to_owned(),
                    Json::new(()),
                ))
            );
            world
                .account_permissions_mut_for_testing()
                .insert(authority.clone(), permissions);
        }
        let source = r#"
seiyaku DynamicAccessCounter {
  state StateMap<int, int> Counters;

  fn bump_hidden(int key, int delta) {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_direct(int key, int delta) authorize("CanEnactGovernance") {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_via_helper(int key, int delta) authorize("CanEnactGovernance") {
    bump_hidden(key: key, delta: delta);
  }
}
"#;
        let (program, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(source)
            .expect("compile dynamic StateMap counter");
        let contract_interface = ivm::ProgramMetadata::parse(&program)
            .expect("parse compiled contract")
            .contract_interface
            .expect("compiled contract interface");
        let code_hash = ivm::contract_code_hash(&program);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &alice,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        world.contract_code.insert(code_hash, program);
        world
            .contract_manifests
            .insert(code_hash, manifest.signed(&alice_keypair));
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let contract_subject = contract_address.subject_id();
        world.accounts.insert(
            contract_subject.clone(),
            iroha_data_model::account::AccountValue::new(
                iroha_data_model::account::AccountDetails::default(),
            ),
        );
        world.contract_subject_bindings.insert(
            contract_address.clone(),
            crate::smartcontracts::code::ContractSubjectBinding::new_direct(
                &contract_address,
                alice.clone(),
            )
            .with_active_code_hash(code_hash),
        );
        world
            .contract_subject_addresses
            .insert(contract_subject, contract_address.clone());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_with_chain_for_testing(world, kura, query, chain_id.clone());
        install_test_lane_manifests(&state);
        let mut pipeline = state.pipeline.clone();
        pipeline.dynamic_prepass = true;
        pipeline.parallel_overlay = true;
        pipeline.parallel_apply = true;
        pipeline.workers = 2;
        state.set_pipeline(pipeline);
        let make_call = |authority: AccountId, keypair: &KeyPair, entrypoint: &str, delta: i64| {
            let payload = Json::new(norito::json!({
                "key": "7",
                "delta": (delta.to_string()),
            }));
            let schema = contract_interface
                .entrypoints
                .iter()
                .find(|descriptor| descriptor.name == entrypoint)
                .and_then(|descriptor| descriptor.argument_schema.as_ref());
            let arguments =
                crate::executor::encode_contract_argument_record(schema, Some(&payload))
                    .expect("encode contract arguments")
                    .map(iroha_data_model::transaction::executable::ContractArgumentRecord::try_new)
                    .transpose()
                    .expect("bounded contract arguments");
            TransactionBuilder::new(
                state.network_id,
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(
                    Vec::new(),
                    core::num::NonZeroU64::new(1_000_000),
                ),
            )
            .with_executable(Executable::ContractCall(
                iroha_data_model::transaction::executable::ContractInvocation {
                    contract_address: contract_address.clone(),
                    expected_code_hash: code_hash,
                    entrypoint: entrypoint.to_owned(),
                    arguments,
                },
            ))
            .sign(keypair.private_key())
        };
        let direct = make_call(alice.clone(), &alice_keypair, "bump_direct", 3);
        let helper = make_call(bob, &bob_keypair, "bump_via_helper", 5);
        let accepted = [direct, helper]
            .into_iter()
            .map(|tx| AcceptedTransaction::new_unchecked(Cow::Owned(tx)))
            .collect();
        state
            .seed_genesis_for_testing()
            .expect("authenticate ordinary fixture predecessor");
        let block = BlockBuilder::new(accepted)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());
        let valid = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results = valid
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .map(|(_, _, result)| result.0.clone())
            .collect::<Vec<_>>();
        assert!(
            results.iter().all(Result::is_ok),
            "both co-batched contract calls must succeed: {results:?}"
        );
        let encoded_key =
            ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(7))
                .expect("encode canonical StateMap int key");
        let logical_path = format!("Counters/{}", hex::encode(encoded_key));
        let scope_id = contract_address.to_string();
        let scope_digest = hex::encode(Hash::new(scope_id.as_bytes()).as_ref());
        let scoped_path: StatePath = format!("sc/{scope_digest}/{logical_path}")
            .parse()
            .expect("valid scoped StateMap path");
        let stored = state_block
            .world
            .smart_contract_state
            .get(&scoped_path)
            .expect("counter state must be persisted");
        let counter = decode_stored_state_int(stored);
        assert_eq!(
            counter, 8,
            "the second overlay must be recomputed from the first call's committed value"
        );
    }
    #[test]
    fn block_validation_serializes_a_dynamic_target_that_changes_during_reprepare() {
        let chain_id = ChainId::from("dynamic-target-live-reprepare");
        let (alice, alice_keypair) = gen_account_in("wonderland");
        let (bob, bob_keypair) = gen_account_in("wonderland");
        let (charlie, charlie_keypair) = gen_account_in("wonderland");
        let (dave, dave_keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(&alice);
        let accounts = [
            Account::new(alice.clone()).build(&alice),
            Account::new(bob.clone()).build(&alice),
            Account::new(charlie.clone()).build(&alice),
            Account::new(dave.clone()).build(&alice),
        ];
        let mut world = World::with([domain], accounts, []);
        for authority in [&alice, &bob, &charlie, &dave] {
            let mut permissions = iroha_data_model::permission::Permissions::new();
            assert!(
                permissions.insert(iroha_data_model::permission::Permission::new(
                    "CanEnactGovernance".to_owned(),
                    Json::new(()),
                ))
            );
            world
                .account_permissions_mut_for_testing()
                .insert(authority.clone(), permissions);
        }
        let source = r#"
seiyaku DynamicTarget {
  error enum DynamicTargetError {
    SelectorClosed = 1,
  }

  state StateMap<int, int> Selector;
  state StateMap<int, int> Counters;

  kotoage fn choose(int key) authorize("CanEnactGovernance") {
    Selector[0] = key;
  }

  kotoage fn set_selected(int value) authorize("CanEnactGovernance") {
    let key = Selector.get(0).unwrap_or(1);
    Counters[key] = value;
  }

  kotoage fn set_direct(int key, int value) authorize("CanEnactGovernance") {
    Counters[key] = value;
  }

  kotoage fn guarded_set(int value) authorize("CanEnactGovernance") {
    require(Selector.get(0).unwrap_or(0) == 2, DynamicTargetError::SelectorClosed);
    Counters[3] = value;
  }
}
"#;
        let (program, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(source)
            .expect("compile dynamic-target contract");
        let contract_interface = ivm::ProgramMetadata::parse(&program)
            .expect("parse compiled contract")
            .contract_interface
            .expect("compiled contract interface");
        let code_hash = ivm::contract_code_hash(&program);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &alice,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        world.contract_code.insert(code_hash, program);
        world
            .contract_manifests
            .insert(code_hash, manifest.signed(&alice_keypair));
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let contract_subject = contract_address.subject_id();
        world.accounts.insert(
            contract_subject.clone(),
            iroha_data_model::account::AccountValue::new(
                iroha_data_model::account::AccountDetails::default(),
            ),
        );
        world.contract_subject_bindings.insert(
            contract_address.clone(),
            crate::smartcontracts::code::ContractSubjectBinding::new_direct(
                &contract_address,
                alice.clone(),
            )
            .with_active_code_hash(code_hash),
        );
        world
            .contract_subject_addresses
            .insert(contract_subject, contract_address.clone());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_with_chain_for_testing(world, kura, query, chain_id.clone());
        install_test_lane_manifests(&state);
        let mut pipeline = state.pipeline.clone();
        pipeline.dynamic_prepass = true;
        pipeline.parallel_overlay = true;
        pipeline.parallel_apply = true;
        pipeline.workers = 4;
        state.set_pipeline(pipeline);
        let make_call = |authority: AccountId,
                         keypair: &KeyPair,
                         entrypoint: &str,
                         payload: Json,
                         fixture_nonce: u64| {
            let mut metadata = Metadata::default();
            metadata.insert(
                "dynamic_target_fixture_nonce"
                    .parse()
                    .expect("fixture nonce name"),
                Json::new(fixture_nonce),
            );
            let schema = contract_interface
                .entrypoints
                .iter()
                .find(|descriptor| descriptor.name == entrypoint)
                .and_then(|descriptor| descriptor.argument_schema.as_ref());
            let arguments =
                crate::executor::encode_contract_argument_record(schema, Some(&payload))
                    .expect("encode contract arguments")
                    .map(iroha_data_model::transaction::executable::ContractArgumentRecord::try_new)
                    .transpose()
                    .expect("bounded contract arguments");
            TransactionBuilder::new(
                state.network_id,
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(
                    Vec::new(),
                    core::num::NonZeroU64::new(1_000_000),
                ),
            )
            .with_metadata(metadata)
            .with_executable(Executable::ContractCall(
                iroha_data_model::transaction::executable::ContractInvocation {
                    contract_address: contract_address.clone(),
                    expected_code_hash: code_hash,
                    entrypoint: entrypoint.to_owned(),
                    arguments,
                },
            ))
            .sign(keypair.private_key())
        };
        let choose = (0_u64..)
            .find_map(|nonce| {
                let tx = make_call(
                    alice.clone(),
                    &alice_keypair,
                    "choose",
                    Json::new(norito::json!({ "key": "2" })),
                    nonce,
                );
                (tx.hash_as_entrypoint().as_ref()[0] < 0x10).then_some(tx)
            })
            .expect("find selector update in the first canonical hash band");
        let selected = (0_u64..)
            .find_map(|nonce| {
                let tx = make_call(
                    bob.clone(),
                    &bob_keypair,
                    "set_selected",
                    Json::new(norito::json!({ "value": "5" })),
                    nonce,
                );
                (0x40..0x50)
                    .contains(&tx.hash_as_entrypoint().as_ref()[0])
                    .then_some(tx)
            })
            .expect("find selected write in the second canonical hash band");
        let direct = (0_u64..)
            .find_map(|nonce| {
                let tx = make_call(
                    charlie.clone(),
                    &charlie_keypair,
                    "set_direct",
                    Json::new(norito::json!({ "key": "2", "value": "7" })),
                    nonce,
                );
                (0x80..0x90)
                    .contains(&tx.hash_as_entrypoint().as_ref()[0])
                    .then_some(tx)
            })
            .expect("find direct write in the third canonical hash band");
        let guarded = (0_u64..)
            .find_map(|nonce| {
                let tx = make_call(
                    dave.clone(),
                    &dave_keypair,
                    "guarded_set",
                    Json::new(norito::json!({ "value": "11" })),
                    nonce,
                );
                (tx.hash_as_entrypoint().as_ref()[0] >= 0xF0).then_some(tx)
            })
            .expect("find guarded write in the final canonical hash band");
        let accepted = [choose, selected, direct, guarded]
            .into_iter()
            .map(|tx| AcceptedTransaction::new_unchecked(Cow::Owned(tx)))
            .collect();
        state
            .seed_genesis_for_testing()
            .expect("authenticate ordinary fixture predecessor");
        let block = BlockBuilder::new(accepted)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());
        let valid = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results = valid
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .map(|(_, _, result)| result.0.clone())
            .collect::<Vec<_>>();
        assert!(
            results.iter().all(Result::is_ok),
            "all dynamic-target calls must succeed: {results:?}"
        );
        let encoded_key =
            ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(2))
                .expect("encode canonical StateMap int key");
        let logical_path = format!("Counters/{}", hex::encode(encoded_key));
        let scope_digest = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
        let scoped_path: StatePath = format!("sc/{scope_digest}/{logical_path}")
            .parse()
            .expect("valid scoped StateMap path");
        let stored = state_block
            .world
            .smart_contract_state
            .get(&scoped_path)
            .expect("selected counter must be persisted");
        let counter = decode_stored_state_int(stored);
        assert_eq!(
            counter, 7,
            "a key selected during live re-execution must retain source-order conflict semantics"
        );
        let guarded_key =
            ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(3))
                .expect("encode canonical guarded StateMap int key");
        let guarded_path: StatePath =
            format!("sc/{scope_digest}/Counters/{}", hex::encode(guarded_key))
                .parse()
                .expect("valid guarded StateMap path");
        let guarded_stored = state_block
            .world
            .smart_contract_state
            .get(&guarded_path)
            .expect("an initially failing VM overlay must be retried against live state");
        let guarded_value = decode_stored_state_int(guarded_stored);
        assert_eq!(guarded_value, 11);
    }
    #[test]
    fn block_validation_sealed_commitment_and_time_keep_distinct_canonical_sources() {
        use crate::state::TransactionsReadOnly;
        use iroha_data_model::{
            events::time::{ExecutionTime, TimeEvent, TimeEventFilter, TimeInterval},
            fastpq::{FastpqSourceExecutionKindV1, FastpqSourceRouteV1},
            trigger::{
                Trigger,
                action::{Action, Repeats},
            },
        };

        let chain_id = ChainId::from("non-external-sequential-fallback");
        let (authority, keypair) = gen_account_in("wonderland");
        let mut state = state_with_transaction_policy(&chain_id, &authority, false, false);
        // A component predecessor establishes ordinary height; it makes no finality claim.
        let mut parent = iroha_data_model::block::builder::BlockBuilder::new(BlockHeader::new(
            nonzero!(1_u64),
            None,
            None,
            0,
            0,
        ))
        .build_with_signature(0, keypair.private_key());
        parent
            .set_execution_outputs(
                Vec::new(),
                0,
                Default::default(),
                Vec::new(),
                Default::default(),
                Default::default(),
                Vec::new(),
                &crate::execution_output_test_support::structural_output_limits(),
            )
            .unwrap();
        state
            .kura()
            .store_block(std::sync::Arc::new(parent.clone()))
            .unwrap();
        state.push_block_hash_for_testing(parent.hash());
        let time_trigger_id: iroha_data_model::trigger::TriggerId =
            "non_external_sequential_heartbeat"
                .parse()
                .expect("trigger id");
        let mut trigger_metadata = Metadata::default();
        trigger_metadata.insert(
            "__registered_block_height"
                .parse()
                .expect("registered-height key"),
            Json::new(0_u64),
        );
        trigger_metadata.insert(
            "__registered_at_ms".parse().expect("registered-time key"),
            Json::new(0_u64),
        );
        let time_trigger = Trigger::new(
            time_trigger_id.clone(),
            Action::new(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "sequential heartbeat".to_owned(),
                ))],
                Repeats::Exactly(1),
                authority.clone(),
                TimeEventFilter::new(ExecutionTime::PreCommit),
            )
            .expect("time-trigger action")
            .with_metadata(trigger_metadata),
        );
        {
            let mut triggers_block = state.world.triggers.block();
            let mut triggers_transaction = triggers_block.transaction();
            triggers_transaction
                .add_time_trigger(time_trigger.try_into().expect("specialized time trigger"))
                .expect("add time trigger");
            triggers_transaction.apply();
            triggers_block.commit();
        }
        let metadata_key = Name::from_str("sequential_fallback_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(state.network_id, &authority, &keypair, 3, 4, metadata_key);
        let commitment_entrypoint_hash = commitment_entrypoint.hash();
        let commitment_call_hash = Hash::from(commitment_entrypoint.execution_call_hash());
        let accepted =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let block = BlockBuilder::new(vec![accepted])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        let time_event = TimeEvent {
            interval: TimeInterval::new(
                parent.header().creation_time(),
                block.header().creation_time(),
            ),
        };
        let expected_invocation = iroha_data_model::block::execution_output::TimeInvocationV1 {
            schedule_index: 0,
            event: time_event,
            trigger:
                crate::smartcontracts::isi::triggers::set::invocation_identity::time_trigger_use_v1(
                    &state.world.triggers.view(),
                    &time_trigger_id,
                    block.header().height().get(),
                )
                .expect("bind actual stored Time action before execution"),
        };
        let expected_time_call_hash = expected_invocation
            .execution_call_hash(block.header().hash())
            .unwrap();
        let mut state_block = state.block(block.header());
        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results: Vec<_> = valid_block
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid_block
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(valid_block.as_ref().execution_outputs().len(), 2);
        assert_eq!(results[0].1.hash(), commitment_entrypoint_hash);
        assert!(
            results[0].2.0.is_ok(),
            "non-external entrypoint fallback must preserve execution: {:?}",
            results[0].2
        );
        let time_output = &valid_block.as_ref().execution_outputs()[1];
        let iroha_data_model::block::execution_output::ExecutionOutputV1::Time(time) = time_output
        else {
            panic!("actual Time invocation follows the single Network output");
        };
        assert_eq!(time.invocation, expected_invocation);
        assert!(
            time.result.is_ok(),
            "actual Time output must succeed: {:?}",
            time.result
        );
        let time_output_hash = HashOf::new(time_output);
        let time_non_network_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::from(time_output_hash));
        assert_eq!(
            state_block.transactions.get(&commitment_entrypoint_hash),
            Some(nonzero!(2_usize))
        );
        assert_eq!(
            state_block.transactions.get(&time_non_network_hash),
            None,
            "an internal output hash is not a signed canonical replay carrier"
        );
        let source_inventory = state_block
            .fastpq_source_inventory()
            .expect("valid finalized source inventory")
            .expect("execution retains its complete source inventory");
        let canonical_entrypoints = valid_block
            .as_ref()
            .external_entrypoints_cloned()
            .collect::<Vec<_>>();
        let expected_tx_set_hash: [u8; 32] =
            iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
                canonical_entrypoints.iter(),
            )
            .expect("canonical sequential transaction set")
            .into();
        assert_eq!(source_inventory.tx_set_hash(), expected_tx_set_hash);
        assert_eq!(source_inventory.entries().len(), 2);
        assert_eq!(
            source_inventory.entries()[0].entry_hash,
            commitment_call_hash
        );
        let time_source = &source_inventory.entries()[1];
        assert_eq!(time_source.entry_hash, expected_time_call_hash);
        assert_ne!(time_source.entry_hash, Hash::from(time_output_hash));
        assert_eq!(
            time_source.execution_kind,
            FastpqSourceExecutionKindV1::ExecutionCall
        );
        assert_eq!(time_source.route, FastpqSourceRouteV1::Unrouted);
        assert_eq!(time_source.dataspace_id, DataSpaceId::UNIVERSAL);
    }
    #[test]
    fn block_validation_sequential_entrypoints_execute_pipeline_triggers() {
        let _guard = crate::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus status test lock");
        crate::status::set_lane_settlement_commitments(Vec::new());
        crate::status::set_lane_relay_envelopes(Vec::new());
        let chain_id = ChainId::from("sequential-pipeline-triggers");
        let network_id = deterministic_test_network_id(0x0D);
        let (authority, keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let mut world = World::with([domain], [account], []);
        let block_key = Name::from_str("sequential_block_pipeline_trigger").expect("metadata key");
        let tx_key = Name::from_str("sequential_tx_pipeline_trigger").expect("metadata key");
        let external_signed = TransactionBuilder::new(
            network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "external".to_owned())])
        .sign(keypair.private_key());
        let external_hash = external_signed.hash();
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sequential_block_approved",
            block_key.clone(),
            PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sequential_external_approved",
            tx_key.clone(),
            PipelineEventFilterBox::from(
                TransactionEventFilter::new()
                    .for_hash(external_hash)
                    .for_status(TransactionStatus::Approved),
            ),
        );
        let fixture_triggers = std::mem::take(&mut world.triggers);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            kura,
            query_handle,
            chain_id.clone(),
            network_id,
        );
        install_test_lane_manifests(&state);
        state
            .seed_genesis_for_testing()
            .expect("authenticate Pipeline fixture predecessor");
        // Install component callback fixtures after the actual genesis owner has completed.
        state.world.triggers = fixture_triggers;
        // Settlement finality binds the exact dataspace proof policy as well as lane status.
        let manifest = iroha_data_model::nexus::AssetPermissionManifest {
            version: iroha_data_model::nexus::ManifestVersion::default(),
            uaid: iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(
                b"sequential-pipeline-trigger-manifest-owner",
            )),
            dataspace: DataSpaceId::UNIVERSAL,
            issued_ms: 0,
            activation_epoch: 1,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let manifest_record =
            crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(manifest);
        let mut manifest_root = [0_u8; 32];
        manifest_root.copy_from_slice(manifest_record.manifest_hash.as_ref());
        state.set_axt_policy(
            DataSpaceId::UNIVERSAL,
            iroha_data_model::nexus::AxtPolicyEntry {
                manifest_root,
                target_lane: LaneId::SINGLE,
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 0,
            },
        );
        let metadata_key = Name::from_str("sequential_commitment_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(state.network_id, &authority, &keypair, 3, 4, metadata_key);
        let commitment_entrypoint_hash = commitment_entrypoint.hash();
        let external_entrypoint_hash = external_signed.hash_as_entrypoint();
        let lane_incarnation = Hash::new(b"sequential-settlement-lane-incarnation");
        let validator_set = vec![PeerId::new(keypair.public_key().clone())];
        let mut ownership = iroha_data_model::block::consensus::SumeragiLanePayloadOwnership {
            proposal_height: 2,
            proposal_view: 0,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"sequential settlement subject placeholder"),
            qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                chain_id.as_str(),
            ),
            accepted_candidate_indices: vec![0, 1],
            accepted_transaction_hashes: vec![
                Hash::from(external_entrypoint_hash),
                Hash::from(commitment_entrypoint_hash),
            ],
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(Hash::new(
                b"sequential settlement descriptor placeholder",
            )),
            lane_block_descriptor_validator_set: validator_set,
            lane_block_descriptor_validator_count: 1,
            lane_block_descriptor_min_quorum: 1,
            payload_ownership_hash: Hash::new(b"sequential settlement ownership placeholder"),
            rbc_instance_hash: Hash::new(b"sequential settlement rbc placeholder"),
        };
        let replay_hashes = ownership
            .compute_replay_hashes()
            .expect("sequential settlement ownership replay hashes");
        ownership.subject_hash = replay_hashes.subject_hash;
        ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
        ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
        let execution_context = BlockExecutionContextBundle::new(vec![
            ExternalExecutionContext::new(
                external_entrypoint_hash,
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            ),
            ExternalExecutionContext::new(
                commitment_entrypoint_hash,
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            ),
        ])
        .with_lane_payload_ownerships(vec![ownership]);
        let accepted_external = AcceptedTransaction::new_unchecked(Cow::Owned(external_signed));
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let block = BlockBuilder::new(vec![accepted_external, accepted_commitment])
            .chain(0, state.view().latest_block().as_deref())
            .with_execution_context(Some(execution_context))
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());
        let mut source_id = [0; Hash::LENGTH];
        source_id.copy_from_slice(external_hash.as_ref());
        state_block.record_settlement_receipt(
            external_hash,
            crate::settlement::PendingSettlement {
                source_id,
                asset_definition_id: AssetDefinitionId::derive_from_components(
                    domain_id,
                    "settlement".parse().expect("asset name"),
                ),
                local_amount: crate::settlement::quantity_from_micro_units(11),
                xor_due: crate::settlement::quantity_from_micro_units(7),
                xor_after_haircut: crate::settlement::quantity_from_micro_units(6),
                xor_variance: crate::settlement::quantity_from_micro_units(1),
                timestamp_ms: 1,
                liquidity_profile: settlement_router::LiquidityProfile::Tier1,
                volatility_bucket: crate::settlement::VolatilityBucket::Stable,
                twap_local_per_xor: Numeric::one(),
                epsilon_bps: 25,
                twap_window_seconds: 60,
                oracle_timestamp_ms: 1,
            },
        );
        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        assert!(
            valid_block
                .as_ref()
                .network_entrypoints()
                .enumerate()
                .map(|(index, entrypoint)| {
                    let (output_index, output) = valid_block
                        .as_ref()
                        .network_output_at(
                            u32::try_from(index).expect("fixture Network index fits u32"),
                        )
                        .expect("every queried input has its explicit Network output");
                    assert_eq!(usize::try_from(output_index).unwrap(), index);
                    (index, entrypoint, &output.result)
                })
                .all(|(_, _, result)| result.0.is_ok()),
            "mixed sequential block should validate successfully"
        );
        let (block_value, tx_value) = state_block
            .world
            .map_account(&authority, |account| {
                (
                    account.value().metadata().get(&block_key).cloned(),
                    account.value().metadata().get(&tx_key).cloned(),
                )
            })
            .expect("authority account exists");
        assert_eq!(block_value, Some(Json::new("ok")));
        assert_eq!(tx_value, Some(Json::new("ok")));
        let statements = valid_block.as_ref().lane_finality_statements();
        assert_eq!(statements.len(), 1);
        let statement = &statements[0];
        assert_eq!(statement.manifest_root, manifest_root);
        assert_eq!(
            statement.block_header_hash,
            valid_block.as_ref().hash(),
            "lane finality must bind the header after result and trigger finalization"
        );
        let settlement = &statement.settlement_commitment;
        assert_eq!(settlement.lane_id, LaneId::SINGLE);
        assert_eq!(settlement.dataspace_id, DataSpaceId::UNIVERSAL);
        assert_eq!(settlement.lane_incarnation, lane_incarnation);
        assert_eq!(
            settlement.block_height, 1,
            "settlement binds the lane-local slot"
        );
        assert_eq!(settlement.tx_count, 1);
        assert_eq!(settlement.receipts.len(), 1);
        assert_eq!(settlement.receipts[0].source_id, source_id);
        let snapshot = crate::status::snapshot();
        assert!(
            snapshot.lane_settlement_commitments.is_empty()
                && snapshot.lane_relay_envelopes.is_empty(),
            "successful execution is still only a candidate and must not publish relay evidence"
        );
        crate::status::set_lane_settlement_commitments(Vec::new());
        crate::status::set_lane_relay_envelopes(Vec::new());
    }
    #[test]
    fn block_validation_sealed_only_entrypoint_executes_only_block_pipeline_trigger() {
        let chain_id = ChainId::from("sealed-only-pipeline-triggers");
        let network_id = deterministic_test_network_id(0x0E);
        let (authority, keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let mut world = World::with([domain], [account], []);
        let block_key = Name::from_str("sealed_only_block_pipeline_trigger").expect("metadata key");
        let wrong_block_height_key =
            Name::from_str("sealed_only_wrong_height_block_pipeline_trigger")
                .expect("metadata key");
        let tx_key = Name::from_str("sealed_only_tx_pipeline_trigger").expect("metadata key");
        let any_tx_key =
            Name::from_str("sealed_only_any_tx_pipeline_trigger").expect("metadata key");
        let dummy_signed = TransactionBuilder::new(
            network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
        .sign(keypair.private_key());
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sealed_only_block_approved",
            block_key.clone(),
            PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sealed_only_wrong_height_block_approved",
            wrong_block_height_key.clone(),
            PipelineEventFilterBox::from(
                BlockEventFilter::new()
                    .for_height(nonzero!(9999_u64))
                    .for_status(BlockStatus::Approved),
            ),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sealed_only_dummy_tx_approved",
            tx_key.clone(),
            PipelineEventFilterBox::from(
                TransactionEventFilter::new()
                    .for_hash(dummy_signed.hash())
                    .for_status(TransactionStatus::Approved),
            ),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sealed_only_any_tx_approved",
            any_tx_key.clone(),
            PipelineEventFilterBox::from(
                TransactionEventFilter::new().for_status(TransactionStatus::Approved),
            ),
        );
        let fixture_triggers = std::mem::take(&mut world.triggers);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            kura,
            query_handle,
            chain_id.clone(),
            network_id,
        );
        install_test_lane_manifests(&state);
        state
            .seed_genesis_for_testing()
            .expect("authenticate Pipeline fixture predecessor");
        // Install component callback fixtures after the actual genesis owner has completed.
        state.world.triggers = fixture_triggers;
        let metadata_key = Name::from_str("sealed_only_commitment_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(state.network_id, &authority, &keypair, 3, 4, metadata_key);
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let block = BlockBuilder::new(vec![accepted_commitment])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        assert_ne!(block.header().height(), nonzero!(9999_u64));
        let mut state_block = state.block(block.header());
        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        assert!(
            valid_block
                .as_ref()
                .network_entrypoints()
                .enumerate()
                .map(|(index, entrypoint)| {
                    let (output_index, output) = valid_block
                        .as_ref()
                        .network_output_at(
                            u32::try_from(index).expect("fixture Network index fits u32"),
                        )
                        .expect("every queried input has its explicit Network output");
                    assert_eq!(usize::try_from(output_index).unwrap(), index);
                    (index, entrypoint, &output.result)
                })
                .all(|(_, _, result)| result.0.is_ok()),
            "sealed-only block should validate successfully"
        );
        let (block_value, wrong_block_height_value, tx_value, any_tx_value) = state_block
            .world
            .map_account(&authority, |account| {
                (
                    account.value().metadata().get(&block_key).cloned(),
                    account
                        .value()
                        .metadata()
                        .get(&wrong_block_height_key)
                        .cloned(),
                    account.value().metadata().get(&tx_key).cloned(),
                    account.value().metadata().get(&any_tx_key).cloned(),
                )
            })
            .expect("authority account exists");
        assert_eq!(block_value, Some(Json::new("ok")));
        assert_eq!(
            wrong_block_height_value, None,
            "approved block trigger must not match a different block height"
        );
        assert_eq!(
            tx_value, None,
            "sealed-only entrypoints must not synthesize transaction pipeline events for arbitrary hashes"
        );
        assert_eq!(
            any_tx_value, None,
            "sealed-only entrypoints must not synthesize broad transaction pipeline events"
        );
    }
    include!("block/sequential_rejected_pipeline_trigger_tests.rs");
    #[test]
    fn block_pipeline_executes_sealed_reveal_and_records_entrypoint_hash() {
        let chain_id = ChainId::from("sealed-block-pipeline");
        let (authority, keypair) = gen_account_in("wonderland");
        let mut state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let (fee_sink, _) = gen_account_in("wonderland");
        let fee_asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "sealed_fee".parse().unwrap(),
        );
        {
            let mut genesis = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            let mut transaction = genesis.transaction();
            Register::account(Account::new(fee_sink.clone()))
                .execute(&authority, &mut transaction)
                .unwrap();
            Register::asset_definition(AssetDefinition::numeric(
                fee_asset.clone(),
                "sealed fee",
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            ))
            .execute(&authority, &mut transaction)
            .unwrap();
            Mint::asset_quantity(1_u32, AssetId::new(fee_asset.clone(), authority.clone()))
                .execute(&authority, &mut transaction)
                .unwrap();
            transaction.apply();
            genesis.commit_world_overlay_for_testing().unwrap();
        }
        state.nexus.get_mut().fees.fee_asset_id = fee_asset.to_string();
        state.nexus.get_mut().fees.fee_sink_account_id = fee_sink.to_string();
        let metadata_key = Name::from_str("sealed_reveal_executed").expect("metadata key");
        let mut builder = TransactionBuilder::new(
            state.network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(
                vec![iroha_data_model::transaction::FeeChargeLimit::new(
                    iroha_data_model::transaction::FeeChargeKind::Nexus,
                    fee_asset,
                    Quantity::from(1_u32),
                )],
                None,
            ),
        );
        builder.set_creation_time(Duration::ZERO);
        let signed = builder
            .with_instructions([SetKeyValue::account(
                authority.clone(),
                metadata_key.clone(),
                Json::new("revealed"),
            )])
            .sign(keypair.private_key());
        let (commitment_entrypoint, reveal_entrypoint) =
            sealed_entrypoints_from_signed(state.network_id, &authority, &keypair, 3, 4, signed);
        let commitment_entrypoint_hash = commitment_entrypoint.hash();
        let reveal_entrypoint_hash = reveal_entrypoint.hash();
        state
            .seed_genesis_for_testing()
            .expect("authenticate sealed commitment predecessor");
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let (_commit_clock, commit_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let commitment_block =
            BlockBuilder::new_with_time_source(vec![accepted_commitment], commit_time_source)
                .chain(0, state.view().latest_block().as_deref())
                .sign(keypair.private_key())
                .unpack(|_| {});
        let mut commitment_state_block = state.block(commitment_block.header);
        let initial_smart_contract_state_len =
            commitment_state_block.world.smart_contract_state.len();
        let valid_commitment_block = commitment_block
            .validate_and_record_transactions(&mut commitment_state_block)
            .unpack(|_| {});
        let commitment_result = valid_commitment_block
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid_commitment_block
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .next()
            .expect("commitment result");
        assert_eq!(commitment_result.1.hash(), commitment_entrypoint_hash);
        assert!(
            commitment_result.2.0.is_ok(),
            "sealed commitment must execute successfully: {:?}",
            commitment_result.2
        );
        assert_eq!(
            commitment_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len + 1,
            "commitment block should leave one pending sealed commitment"
        );
        state
            .commit_executed_block_for_testing(
                commitment_state_block,
                valid_commitment_block
                    .clone()
                    .commit_unchecked()
                    .unpack(|_| {}),
            )
            .expect("publish actual sealed commitment output");
        let commitment_signed_block: SignedBlock = valid_commitment_block.into();
        let accepted_reveal =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(reveal_entrypoint));
        let (_reveal_clock, reveal_time_source) = TimeSource::new_mock(Duration::from_millis(2));
        let reveal_block =
            BlockBuilder::new_with_time_source(vec![accepted_reveal], reveal_time_source)
                .chain(0, Some(&commitment_signed_block))
                .sign(keypair.private_key())
                .unpack(|_| {});
        let mut reveal_state_block = state.block(reveal_block.header);
        let valid_reveal_block = reveal_block
            .validate_and_record_transactions(&mut reveal_state_block)
            .unpack(|_| {});
        let reveal_result = valid_reveal_block
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid_reveal_block
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .next()
            .expect("reveal result");
        assert_eq!(reveal_result.1.hash(), reveal_entrypoint_hash);
        assert!(
            reveal_result.2.0.is_ok(),
            "sealed reveal must execute the inner transaction: {:?}",
            reveal_result.2
        );
        assert_eq!(
            reveal_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len,
            "successful reveal should consume the pending commitment"
        );
        let metadata_value = reveal_state_block
            .world
            .map_account(&authority, |account| {
                account.value().metadata().get(&metadata_key).cloned()
            })
            .expect("authority account exists");
        assert_eq!(metadata_value, Some(Json::new("revealed")));
    }
    #[test]
    fn block_pipeline_finalizes_sealed_reveal_batch_receipts_on_outer_result_once() {
        let (authority, keypair) = gen_account_in("wonderland");
        let (destination, _destination_keypair) = gen_account_in("wonderland");
        let sealed_dataspace = DataSpaceId::new(7);
        let sealed_dataspace_alias = "sealed-dataspace-7";
        let domain_id = DomainId::try_new("wonderland", sealed_dataspace_alias).expect("domain id");
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "sealed_coin".parse().expect("asset name"),
        );
        let source_asset_id = AssetId::with_scope(
            asset_definition_id.clone(),
            authority.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(sealed_dataspace),
        );
        let destination_asset_id = AssetId::with_scope(
            asset_definition_id.clone(),
            destination.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(sealed_dataspace),
        );
        let world = test_world_with_assets(
            [Domain::new(domain_id.clone()).build(&authority)],
            [
                Account::new(authority.clone()).build(&authority),
                Account::new(destination.clone()).build(&destination),
            ],
            [AssetDefinition::numeric(
                asset_definition_id.clone(),
                "sealed batch coin".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
                Some(domain_id.clone()),
            )
            .build(&authority)],
            [Asset::new(source_asset_id.clone(), Quantity::from(10_u32))],
            [],
        );
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::SINGLE,
                dataspace_id: sealed_dataspace,
                alias: "sealed-default".to_owned(),
                ..LaneConfig::default()
            }],
        )
        .expect("sealed test lane catalog");
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata {
                id: DataSpaceId::UNIVERSAL,
                alias: "universal".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: sealed_dataspace,
                alias: sealed_dataspace_alias.to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("sealed test dataspace catalog");
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.lane_catalog = lane_catalog;
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        nexus.dataspace_catalog = dataspace_catalog;
        nexus.routing_policy.default_lane = LaneId::SINGLE;
        nexus.routing_policy.default_dataspace = sealed_dataspace;
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        // Open the isolated test Kura at the authoritative pre-genesis geometry. Installing this
        // catalog through `set_nexus` would instead model a lifecycle relabel of the already-open
        // synthetic primary lane and correctly refuse to archive that active block store.
        let state = State::new_with_pre_genesis_nexus_for_testing(
            world,
            nexus,
            LiveQueryStore::start_test(),
        );
        assert_eq!(
            state
                .world
                .view()
                .asset_definition_domains()
                .get(&asset_definition_id),
            Some(&domain_id),
            "the fixture asset must be authoritative on the sealed execution dataspace"
        );
        install_test_lane_manifests(&state);

        let mut transaction_builder = TransactionBuilder::new(
            state.network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        transaction_builder.set_creation_time(Duration::ZERO);
        let signed = transaction_builder
            .with_instructions([TransferAssetBatch::independent(vec![
                TransferAssetBatchEntry::with_leg_id(
                    "sealed-batch-leg",
                    authority.clone(),
                    destination.clone(),
                    asset_definition_id,
                    3_u32,
                ),
            ])])
            .sign(keypair.private_key());
        let inner_call_hash = signed.hash_as_entrypoint();
        let salt = [0x5B; 32];
        let commitment = compute_sealed_transaction_commitment(&state.network_id, &signed, salt, 4);
        let commitment_entrypoint =
            TransactionEntrypoint::SealedCommitment(SignedSealedTransactionCommitment::sign(
                SealedTransactionCommitmentPayload::new(
                    state.network_id,
                    authority,
                    commitment,
                    3,
                    4,
                    None,
                ),
                keypair.private_key(),
            ));
        let reveal_entrypoint = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
            commitment, signed, salt,
        ));
        let outer_reveal_hash = reveal_entrypoint.hash();

        state
            .seed_genesis_for_testing()
            .expect("authenticate sealed batch predecessor");
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let (_commit_clock, commit_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let commitment_block =
            BlockBuilder::new_with_time_source(vec![accepted_commitment], commit_time_source)
                .chain(0, state.view().latest_block().as_deref())
                .sign(keypair.private_key())
                .unpack(|_| {});
        let mut commitment_state_block = state.block(commitment_block.header);
        let valid_commitment = commitment_block
            .validate_and_record_transactions(&mut commitment_state_block)
            .unpack(|_| {});
        assert!(
            valid_commitment
                .as_ref()
                .output_results()
                .enumerate()
                .filter_map(|(index, result)| result.as_ref().err().map(|reason| (index, reason)))
                .next()
                .is_none(),
            "the sealed commitment must finalize successfully"
        );
        state
            .commit_executed_block_for_testing(
                commitment_state_block,
                valid_commitment.clone().commit_unchecked().unpack(|_| {}),
            )
            .expect("publish exact sealed batch commitment output");
        let committed_parent: SignedBlock = valid_commitment.into();

        let accepted_reveal =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(reveal_entrypoint));
        let (_reveal_clock, reveal_time_source) = TimeSource::new_mock(Duration::from_millis(2));
        let reveal_plan = {
            let view = state.view();
            crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                &view.nexus,
                &accepted_reveal,
                view.world(),
                u64::try_from(reveal_time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
                3,
            )
            .expect("sealed batch route resolves from the installed Nexus catalog")
        };
        assert_eq!(
            reveal_plan.coordinator_route(),
            crate::queue::RoutingDecision::new(LaneId::SINGLE, sealed_dataspace),
            "the production router must select the authoritative sealed dataspace"
        );
        let reveal_builder =
            BlockBuilder::new_with_time_source(vec![accepted_reveal], reveal_time_source)
                .chain(0, Some(&committed_parent));
        // The state-free block-builder test fallback is intentionally UNIVERSAL. Replace that
        // scaffold with the exact policy-derived route so the ordinary finalizer is exercised
        // against the same non-universal execution identity that production Sumeragi embeds.
        let mut reveal_execution_context = default_test_execution_context(
            &reveal_builder.0.transactions,
            &reveal_builder.0.header,
            PeerId::new(keypair.public_key().clone()),
        );
        reveal_execution_context.external[0] =
            crate::queue::execution_context_for_routing_plan(outer_reveal_hash, &reveal_plan);
        let ownership = &mut reveal_execution_context.lane_payload_ownerships[0];
        ownership.dataspace_id = sealed_dataspace;
        ownership.lane_incarnation =
            crate::state::derive_static_lane_incarnations(&state.nexus_snapshot().lane_catalog)
                .get(&LaneId::SINGLE)
                .copied()
                .expect("sealed test lane has an incarnation");
        ownership.qc_mode_tag = LaneRelayEnvelope::lane_qc_mode_tag_for(
            LaneId::SINGLE,
            sealed_dataspace,
            "block-builder-test",
        );
        let replay_hashes = ownership
            .compute_replay_hashes()
            .expect("sealed test ownership replay hashes compute");
        ownership.subject_hash = replay_hashes.subject_hash;
        ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
        ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
        let reveal_block = reveal_builder
            .with_execution_context(Some(reveal_execution_context))
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut reveal_state_block = state.block(reveal_block.header);
        let valid_reveal = reveal_block
            .validate_and_record_transactions(&mut reveal_state_block)
            .unpack(|_| {});
        let (_, result_entrypoint, result) = valid_reveal
            .as_ref()
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, output) = valid_reveal
                    .as_ref()
                    .network_output_at(
                        u32::try_from(index).expect("fixture Network index fits u32"),
                    )
                    .expect("every queried input has its explicit Network output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (index, entrypoint, &output.result)
            })
            .next()
            .expect("one sealed reveal result");
        assert_eq!(result_entrypoint.hash(), outer_reveal_hash);
        assert!(
            result.0.is_ok(),
            "the sealed native batch must execute successfully: {result:?}"
        );
        let outcomes = result.batch_transfer_outcomes();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].leg_index, 0);
        assert_eq!(outcomes[0].leg_id, "sealed-batch-leg");
        assert!(
            matches!(outcomes[0].status, AssetBatchTransferLegStatus::Applied),
            "the sealed batch leg must apply: {:?}",
            outcomes[0]
        );
        assert_eq!(
            valid_reveal
                .as_ref()
                .batch_transfer_outcomes_for(&outer_reveal_hash),
            outcomes,
            "the canonical outer entrypoint must query the inner execution receipts"
        );
        assert!(
            valid_reveal
                .as_ref()
                .batch_transfer_outcomes_for(&inner_call_hash)
                .is_empty(),
            "committed receipt lookup remains keyed by the canonical outer entrypoint"
        );
        let inner_call_hash = Hash::from(inner_call_hash);
        let outer_reveal_hash = Hash::from(outer_reveal_hash);
        assert_eq!(
            valid_reveal
                .as_ref()
                .fastpq_transcripts()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![inner_call_hash],
            "sealed FASTPQ evidence must retain the inner signed execution identity"
        );
        let witness = reveal_state_block
            .exec_witness
            .as_ref()
            .expect("live sealed reveal captures its execution witness");
        assert_eq!(witness.fastpq_transcripts.len(), 1);
        assert_eq!(witness.fastpq_transcripts[0].entry_hash, inner_call_hash);
        let fastpq_context = reveal_state_block
            .take_fastpq_witness_context()
            .expect("sealed numeric transfer captures FASTPQ context");
        assert_eq!(
            fastpq_context.entry_dataspaces.get(&inner_call_hash),
            Some(&crate::fastpq::dataspace_id_bytes(sealed_dataspace))
        );
        assert!(
            fastpq_context
                .entry_dataspaces
                .get(&outer_reveal_hash)
                .is_none(),
            "the outer reveal envelope must not replace the signed execution-call identity"
        );
        let ordered_entrypoints = valid_reveal
            .as_ref()
            .external_entrypoints_cloned()
            .collect::<Vec<_>>();
        let expected_tx_set_hash: [u8; 32] =
            iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
                ordered_entrypoints.iter(),
            )
            .expect("canonical sealed-reveal transaction set")
            .into();
        assert_eq!(fastpq_context.tx_set_hash, Some(expected_tx_set_hash));
        assert_eq!(
            reveal_state_block
                .fastpq_source_inventory()
                .expect("valid finalized reveal inventory")
                .expect("reveal execution retains source ownership")
                .tx_set_hash(),
            expected_tx_set_hash,
        );
        assert_eq!(
            reveal_state_block
                .world
                .asset(&source_asset_id)
                .expect("source balance exists")
                .value()
                .clone()
                .into_inner(),
            Quantity::from(7_u32)
        );
        assert_eq!(
            reveal_state_block
                .world
                .asset(&destination_asset_id)
                .expect("destination balance exists")
                .value()
                .clone()
                .into_inner(),
            Quantity::from(3_u32)
        );
        state
            .commit_executed_block_for_testing(
                reveal_state_block,
                valid_reveal.commit_unchecked().unpack(|_| {}),
            )
            .expect("publish sealed batch reveal state exactly once");
        assert_eq!(
            state
                .world
                .view()
                .asset(&source_asset_id)
                .expect("committed source balance exists")
                .value()
                .clone()
                .into_inner(),
            Quantity::from(7_u32)
        );
        assert_eq!(
            state
                .world
                .view()
                .asset(&destination_asset_id)
                .expect("committed destination balance exists")
                .value()
                .clone()
                .into_inner(),
            Quantity::from(3_u32)
        );
    }
    #[test]
    fn prune_expired_sealed_commitments_removes_pending_state_after_deadline() {
        let chain_id = ChainId::from("sealed-prune-pipeline");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let metadata_key = Name::from_str("sealed_prune_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(state.network_id, &authority, &keypair, 3, 3, metadata_key);
        state
            .seed_genesis_for_testing()
            .expect("authenticate sealed commitment predecessor");
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let (_commit_clock, commit_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let commitment_block =
            BlockBuilder::new_with_time_source(vec![accepted_commitment], commit_time_source)
                .chain(0, state.view().latest_block().as_deref())
                .sign(keypair.private_key())
                .unpack(|_| {});
        let mut commitment_state_block = state.block(commitment_block.header);
        let initial_smart_contract_state_len =
            commitment_state_block.world.smart_contract_state.len();
        let valid_commitment_block = commitment_block
            .validate_and_record_transactions(&mut commitment_state_block)
            .unpack(|_| {});
        assert!(
            valid_commitment_block
                .as_ref()
                .network_entrypoints()
                .enumerate()
                .map(|(index, entrypoint)| {
                    let (output_index, output) = valid_commitment_block
                        .as_ref()
                        .network_output_at(
                            u32::try_from(index).expect("fixture Network index fits u32"),
                        )
                        .expect("every queried input has its explicit Network output");
                    assert_eq!(usize::try_from(output_index).unwrap(), index);
                    (index, entrypoint, &output.result)
                })
                .all(|(_, _, result)| result.0.is_ok()),
            "commitment block should not reject the sealed commitment"
        );
        assert_eq!(
            commitment_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len + 1
        );
        state
            .commit_executed_block_for_testing(
                commitment_state_block,
                valid_commitment_block
                    .clone()
                    .commit_unchecked()
                    .unpack(|_| {}),
            )
            .expect("publish actual sealed commitment output");
        let prune_header = BlockHeader::new(nonzero!(4_u64), None, None, 3, 0);
        let mut prune_state_block = state.block(prune_header);
        assert_eq!(
            prune_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len + 1,
            "pending commitment should still be visible before pruning"
        );
        let pruned = crate::tx::prune_expired_sealed_commitments(&mut prune_state_block);
        assert_eq!(pruned, 1);
        assert_eq!(
            prune_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len,
            "expired sealed commitment should be removed after its deadline"
        );
    }
    #[test]
    fn block_pipeline_rejects_expired_height_ttl() {
        let chain_id = ChainId::from("block-height-ttl-check");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, true, false);
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("expires_at_height").expect("metadata key"),
            Json::from(2_u64),
        );
        let mut builder = TransactionBuilder::new(
            state.network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::ZERO);
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "expired".to_owned())])
            .with_metadata(metadata)
            .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let previous = previous_block_at_height(1);
        finalize_test_genesis_assets(&state, &previous);
        let unverified_block = BlockBuilder::new_with_time_source(vec![accepted], time_source)
            .chain(0, Some(&previous))
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let error = validation_error_message(valid_block.as_ref());
        assert!(error.contains("expired"), "unexpected rejection: {error}");
    }
    #[test]
    fn block_pipeline_rejects_non_increasing_tx_sequence() {
        let chain_id = ChainId::from("block-sequence-check");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, true);
        {
            let mut world = state.world.block();
            world.tx_sequences.insert(authority.clone(), 5);
            world.commit();
        }
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("tx_sequence").expect("metadata key"),
            Json::from(5_u64),
        );
        let mut builder = TransactionBuilder::new(
            state.network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::ZERO);
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "sequence".to_owned())])
            .with_metadata(metadata)
            .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let previous = previous_block_at_height(1);
        finalize_test_genesis_assets(&state, &previous);
        let unverified_block = BlockBuilder::new_with_time_source(vec![accepted], time_source)
            .chain(0, Some(&previous))
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let error = validation_error_message(valid_block.as_ref());
        assert!(error.contains("sequence"), "unexpected rejection: {error}");
    }
    #[tokio::test]
    async fn should_reject_due_to_repetition() {
        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("Valid");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain = Domain::new(domain_id.clone()).build(&alice_id);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        install_test_lane_manifests(&state);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        // Creating an instruction
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        );
        // Make two distinct signed transactions that attempt the same state
        // transition. Exact duplicate signed hashes are rejected by the block
        // execution-context boundary before instruction evaluation.
        let make_transaction = |creation_time_ms| {
            let mut builder = TransactionBuilder::new(
                state.network_id,
                alice_id.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            );
            builder.set_creation_time(Duration::from_millis(creation_time_ms));
            builder
                .with_instructions([Register::asset_definition(AssetDefinition::numeric(
                    asset_definition_id.clone(),
                    "xor",
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    Some(domain_id.clone()),
                ))])
                .sign(alice_keypair.private_key())
        };
        let first_tx = make_transaction(0);
        let second_tx = make_transaction(1);
        let (_clock, time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let crypto_cfg = state.crypto();
        let first_tx = AcceptedTransaction::accept_with_time_source(
            first_tx,
            &state.network_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
            &time_source,
        )
        .expect("Valid");
        let second_tx = AcceptedTransaction::accept_with_time_source(
            second_tx,
            &state.network_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
            &time_source,
        )
        .expect("Valid");
        // Creating a block of two semantically repetitive transactions and validating it
        let transactions = vec![first_tx, second_tx];
        state
            .seed_genesis_for_testing()
            .expect("authenticate ordinary fixture predecessor");
        let unverified_block = BlockBuilder::new_with_time_source(transactions, time_source)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state
            .commit_executed_block_for_testing(
                state_block,
                valid_block.clone().commit_unchecked().unpack(|_| {}),
            )
            .expect("publish exact validated fixture outputs");
        // The 1st transaction should be confirmed and the 2nd rejected
        assert_eq!(
            valid_block
                .as_ref()
                .output_results()
                .enumerate()
                .filter_map(|(index, result)| result.as_ref().err().map(|reason| (index, reason)))
                .next()
                .unwrap()
                .0,
            1
        );
    }
    include!("block/tx_order_validation_revalidation_test.rs");
    #[tokio::test]
    async fn failed_transactions_revert() {
        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("Valid");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain = Domain::new(domain_id).build(&alice_id);
        let (created_account_id, _) = gen_account_in("wonderland");
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        install_test_lane_manifests(&state);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let create_account = Register::account(Account::new(created_account_id));
        let asset_definition_id =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "coin".parse().unwrap(),
            );
        let create_asset = Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id,
            "coin",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal").unwrap()),
        ));
        let (missing_account_id, _) = gen_account_in("wonderland");
        let fail_isi = Unregister::account(missing_account_id);
        let tx_fail = TransactionBuilder::new(
            state.network_id,
            alice_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions::<InstructionBox>([create_account.clone().into(), fail_isi.into()])
        .sign(alice_keypair.private_key());
        let crypto_cfg = state.crypto();
        let (_clock, time_source) = TimeSource::new_mock(tx_fail.creation_time());
        let tx_fail = AcceptedTransaction::accept_with_time_source(
            tx_fail,
            &state.network_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
            &time_source,
        )
        .expect("Valid");
        let tx_accept = TransactionBuilder::new(
            state.network_id,
            alice_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions::<InstructionBox>([create_account.into(), create_asset.into()])
        .sign(alice_keypair.private_key());
        let (_clock, time_source) = TimeSource::new_mock(tx_accept.creation_time());
        let tx_accept = AcceptedTransaction::accept_with_time_source(
            tx_accept,
            &state.network_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
            &time_source,
        )
        .expect("Valid");
        let fail_hash = tx_fail.as_ref().hash_as_entrypoint();
        let accept_hash = tx_accept.as_ref().hash_as_entrypoint();
        // Creating a block of where first transaction must fail and second one fully executed
        let transactions = vec![tx_fail, tx_accept];
        state
            .seed_genesis_for_testing()
            .expect("authenticate ordinary fixture predecessor");
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state
            .commit_executed_block_for_testing(
                state_block,
                valid_block.clone().commit_unchecked().unpack(|_| {}),
            )
            .expect("publish exact validated fixture outputs");
        let block_ref = valid_block.as_ref();
        let outcomes: Vec<_> = block_ref
            .network_entrypoints()
            .enumerate()
            .map(|(index, input)| {
                let (output_index, row) = block_ref
                    .network_output_at(u32::try_from(index).unwrap())
                    .expect("exact Network source output");
                assert_eq!(usize::try_from(output_index).unwrap(), index);
                (input.hash(), &row.result)
            })
            .collect();
        let lookup = |target: &_, msg: &str| {
            outcomes
                .iter()
                .find(|(hash, _)| hash == target)
                .unwrap_or_else(|| panic!("missing result for {msg}"))
                .1
                .as_ref()
        };
        let fail_result = lookup(&fail_hash, "fail tx");
        assert!(fail_result.is_err(), "Failing tx must be rejected");
        let accept_result = lookup(&accept_hash, "accept tx");
        assert!(
            accept_result.is_ok(),
            "Second tx must succeed, got {accept_result:?}"
        );
    }
    include!("block/rejected_live_batch_fee_tests.rs");
    include!("block/fee_admission_tests.rs");
    include!("block/public_contract_creation_fee_tests.rs");
    include!("block/bootstrap_and_genesis_tests.rs");
    #[test]
    fn sumeragi_parameters_are_accessible() {
        let params = iroha_data_model::parameter::Parameters::default();
        let _ = params.sumeragi().max_clock_drift();
    }
    #[cfg(feature = "bls")]
    #[test]
    fn verify_validator_signatures_accepts_bls_normal() {
        use crate::sumeragi::network_topology::Topology;
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_model_base::peer::PeerId;
        // 3 BLS peers
        let kp0 = KeyPair::try_from_seed(b"seed0".to_vec(), Algorithm::BlsNormal)
            .expect("test BLS validator keypair should be valid");
        let kp1 = KeyPair::try_from_seed(b"seed1".to_vec(), Algorithm::BlsNormal)
            .expect("test BLS validator keypair should be valid");
        let kp2 = KeyPair::try_from_seed(b"seed2".to_vec(), Algorithm::BlsNormal)
            .expect("test BLS validator keypair should be valid");
        let peers = vec![
            PeerId::new(kp0.public_key().clone()),
            PeerId::new(kp1.public_key().clone()),
            PeerId::new(kp2.public_key().clone()),
        ];
        let topology = Topology::new(peers);
        // Build SignedBlock signed by all
        let unverified_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .sign(kp0.private_key())
            .unpack(|_| {});
        let mut vb = ValidBlock::new_unverified_for_tests(unverified_block.into());
        vb.sign(&kp1, &topology);
        vb.sign(&kp2, &topology);
        // Commit succeeds under BLS-normal uniform validators
        assert!(vb.commit(&topology).unpack(|_| {}).is_ok());
    }
    #[test]
    fn signature_error_maps_inactive_consensus_key_reason() {
        assert_eq!(
            map_sig_err_to_reason(&SignatureVerificationError::InactiveConsensusKey),
            error::BlockRejectionReason::InactiveConsensusKey
        );
    }
}
#[cfg(test)]
#[path = "block/commit_signature_tally_tests.rs"]
mod commit_signature_tally_tests;
#[cfg(test)]
fn committed_teu_by_lane_from_routes(
    routes: impl IntoIterator<Item = impl core::borrow::Borrow<crate::queue::RoutingDecision>>,
    teus: impl IntoIterator<Item = u64>,
) -> BTreeMap<LaneId, u64> {
    routes
        .into_iter()
        .zip(teus)
        .fold(BTreeMap::new(), |mut committed, (route, teu)| {
            let route = route.borrow();
            committed
                .entry(route.lane_id)
                .and_modify(|total| *total = total.saturating_add(teu))
                .or_insert(teu);
            committed
        })
}
#[cfg(test)]
mod committed_teu_tests {
    use super::committed_teu_by_lane_from_routes;
    use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
    #[test]
    fn committed_teu_attribution_uses_supplied_routes_not_cached_hints() {
        let stale_hint_lane = LaneId::new(99);
        let fresh_lane = LaneId::new(1);
        let routes = [
            crate::queue::RoutingDecision::new(fresh_lane, DataSpaceId::UNIVERSAL),
            crate::queue::RoutingDecision::new(fresh_lane, DataSpaceId::UNIVERSAL),
            crate::queue::RoutingDecision::new(LaneId::new(2), DataSpaceId::UNIVERSAL),
        ];
        let committed = committed_teu_by_lane_from_routes(routes.iter(), [7, 11, 5]);
        assert_eq!(committed.get(&fresh_lane), Some(&18));
        assert_eq!(committed.get(&LaneId::new(2)), Some(&5));
        assert_eq!(
            committed.get(&stale_hint_lane),
            None,
            "committed telemetry must be derived from validated block routes"
        );
    }
    #[test]
    fn committed_teu_attribution_saturates_instead_of_wrapping() {
        let lane = LaneId::new(7);
        let routes = [
            crate::queue::RoutingDecision::new(lane, DataSpaceId::UNIVERSAL),
            crate::queue::RoutingDecision::new(lane, DataSpaceId::UNIVERSAL),
        ];
        let committed = committed_teu_by_lane_from_routes(routes.iter(), [u64::MAX, 1]);
        assert_eq!(committed.get(&lane), Some(&u64::MAX));
    }
}
