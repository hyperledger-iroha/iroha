//! Block payloads of the Sumeragi driver (`specs/sumeragi.md` §3.2, §6.10).
//!
//! A payload is the exact wire of an *unsigned, resultless* iroha block proposal: the certified
//! core header binds its bytes, so it needs no block signature. Every proposal must carry
//! at least one transaction. An empty builder result means there is no includable work;
//! it never authorizes a block, including at later views or during replay.
//!
//! The leader's builder peeks at the queue (it never removes transactions), keeps the queue's
//! FIFO order, and fills the block up to the payload byte cap and the on-chain transaction cap.

use std::{num::NonZeroUsize, time::Duration};

use iroha_data_model::{block::SignedBlock, transaction::TransactionAdmissionIntent};
use iroha_primitives::time::TimeSource;

use crate::{
    block::{BlockBuilder, ValidBlock},
    queue::{Queue, execution_context_for_routing_plan},
    state::{State, StateReadOnly, WorldReadOnly, compute_confidential_feature_digest},
    tx::AcceptedTransaction,
};
use iroha_data_model::{
    block::BlockExecutionContextBundle,
    sumeragi_lanes::{SumeragiLaneMerge, SumeragiLaneMergeSection},
};

/// How many queued transactions one build inspects at most.
pub const MAX_QUEUE_SCAN: NonZeroUsize = NonZeroUsize::new(4096).expect("non-zero");

/// Why a payload could not be built or decoded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PayloadError {
    /// No transaction can justify creating a block.
    #[error("block proposal must contain at least one transaction")]
    EmptyBlock,
    /// The canonical block time overflows.
    #[error("canonical block time overflows")]
    TimeOverflow,
    /// The block could not be encoded.
    #[error("block encoding failed: {0}")]
    Encode(String),
    /// The payload bytes are not a canonical block proposal.
    #[error("payload is not a canonical block proposal: {0}")]
    NotCanonical(String),
}

/// Inputs of one nonempty block assembly.
#[derive(Clone, Copy, Debug)]
pub struct Assembly<'a> {
    /// The committed parent block.
    pub parent: &'a SignedBlock,
    /// The view the block is (first) proposed in (`origin_view`).
    pub view: u64,
    /// The chain's block cadence (`ChainParams.block_time`).
    pub cadence: Duration,
}

/// Build the unsigned proposal carrying `transactions` (in order) over `assembly.parent`,
/// reading the DA policy and confidential-feature digest from the committed `state`.
///
/// Deterministic in its inputs: the block time is canonical (parent time plus the cadence,
/// strictly after every timed input), not the local clock.
///
/// # Errors
/// There are no transactions, or the canonical block time overflows.
pub fn assemble(
    state: &State,
    assembly: Assembly<'_>,
    transactions: &[(AcceptedTransaction<'static>, crate::queue::RoutingPlan)],
) -> Result<SignedBlock, PayloadError> {
    assemble_with_pulse(state, assembly, transactions, None)
}

/// Assemble actual transaction work together with an already finalized current pulse.
///
/// # Errors
/// See [`assemble`].
pub fn assemble_with_pulse(
    state: &State,
    assembly: Assembly<'_>,
    transactions: &[(AcceptedTransaction<'static>, crate::queue::RoutingPlan)],
    pulse: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
) -> Result<SignedBlock, PayloadError> {
    assemble_with_merges(state, assembly, transactions, &[], pulse)
}

/// Assemble the block's own transactions and its lane merges (`specs/sumeragi_lanes.md` §4.2)
/// with an already finalized current pulse. Merged lane blocks are work: a block may carry
/// merges alone.
///
/// # Errors
/// There are neither transactions nor merges, or the canonical block time overflows.
pub fn assemble_with_merges(
    state: &State,
    assembly: Assembly<'_>,
    transactions: &[(AcceptedTransaction<'static>, crate::queue::RoutingPlan)],
    merges: &[SumeragiLaneMerge],
    pulse: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
) -> Result<SignedBlock, PayloadError> {
    if transactions.is_empty() && merges.is_empty() {
        return Err(PayloadError::EmptyBlock);
    }
    let parent_time = assembly.parent.header().creation_time();
    let minimum = parent_time
        .checked_add(assembly.cadence)
        .ok_or(PayloadError::TimeOverflow)?;
    let build = |time: Duration| -> Result<SignedBlock, PayloadError> {
        build_at(state, assembly, transactions, merges, time, pulse)
    };
    let first = build(minimum)?;
    let canonical = ValidBlock::sumeragi_block_time(&first, parent_time, assembly.cadence)
        .map_err(|_| PayloadError::TimeOverflow)?;
    if canonical == first.header().creation_time() {
        Ok(first)
    } else {
        build(canonical)
    }
}

fn build_at(
    state: &State,
    assembly: Assembly<'_>,
    transactions: &[(AcceptedTransaction<'static>, crate::queue::RoutingPlan)],
    merges: &[SumeragiLaneMerge],
    time: Duration,
    pulse: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
) -> Result<SignedBlock, PayloadError> {
    let height = assembly.parent.header().height().get().saturating_add(1);
    let (_, time_source) = TimeSource::new_mock(time);
    let accepted = transactions
        .iter()
        .map(|(tx, _)| tx.clone())
        .collect::<Vec<_>>();
    let nexus = state.nexus_snapshot();
    let view = state.view();
    let confidential = compute_confidential_feature_digest(view.world(), view.zk(), height);
    drop(view);
    let contexts = transactions
        .iter()
        .map(|(tx, plan)| execution_context_for_routing_plan(tx.hash_as_entrypoint(), plan))
        .collect::<Vec<_>>();
    let mut execution_context = BlockExecutionContextBundle::new(contexts);
    if !merges.is_empty() {
        execution_context.lane_merge = Some(SumeragiLaneMergeSection {
            merges: merges.to_vec(),
            merged_count: 0,
        });
    }
    let builder = BlockBuilder::new_with_time_source(accepted, time_source)
        .chain(assembly.view, Some(assembly.parent))
        .with_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &nexus, height,
        )))
        .with_confidential_features((!confidential.is_empty()).then_some(confidential))
        .with_execution_context((!execution_context.is_empty()).then_some(execution_context))
        .with_global_beacon_pulse(pulse)
        .with_network_input_time_floor(time)
        .ok_or(PayloadError::TimeOverflow)?;
    Ok(builder.into_unsigned_proposal())
}

/// Whether a block carries work: its own transactions or lane merges.
fn has_work(block: &SignedBlock) -> bool {
    block.network_entrypoint_count() > 0
        || block
            .lane_merge()
            .is_some_and(|section| !section.merges.is_empty())
}

/// The payload bytes of `block`: its canonical resultless proposal wire.
///
/// # Errors
/// The block carries no work or cannot be encoded.
pub fn encode(block: &SignedBlock) -> Result<Vec<u8>, PayloadError> {
    if !has_work(block) {
        return Err(PayloadError::EmptyBlock);
    }
    block
        .canonical_resultless_proposal()
        .encode_wire()
        .map_err(|error| PayloadError::Encode(error.to_string()))
}

/// Decode a non-empty payload: the canonical wire of an unsigned, resultless proposal without
/// a certificate (re-encoding must reproduce the bytes exactly).
///
/// # Errors
/// The bytes do not decode, are not canonical, carry a result, a certificate or a signature,
/// or the decoded block has no transactions.
pub fn decode(payload: &[u8]) -> Result<SignedBlock, PayloadError> {
    let block = iroha_data_model::block::decode_versioned_signed_block(payload)
        .map_err(|error| PayloadError::NotCanonical(error.to_string()))?;
    if !has_work(&block) {
        return Err(PayloadError::EmptyBlock);
    }
    if !block.is_resultless_proposal() {
        return Err(PayloadError::NotCanonical(
            "carries a result or a commit certificate".into(),
        ));
    }
    if block.signatures().next().is_some() {
        return Err(PayloadError::NotCanonical(
            "carries a block signature".into(),
        ));
    }
    let reencoded = block
        .encode_wire()
        .map_err(|error| PayloadError::NotCanonical(error.to_string()))?;
    if reencoded != payload {
        return Err(PayloadError::NotCanonical("non-canonical encoding".into()));
    }
    Ok(block)
}

/// Select queued transactions for the block after `parent` (FIFO, peeked, never removed),
/// within `max_bytes` of transaction bytes, the on-chain transaction cap and the FASTPQ source
/// policy's Network input cap less `reserved` (the block's merged lane transactions).
/// Transactions the router cannot place, transactions routed to another lane and
/// `QueuePlanSynced` inputs are skipped.
pub fn select(
    state: &State,
    queue: &std::sync::Arc<Queue>,
    max_bytes: usize,
    reserved: usize,
) -> Vec<(AcceptedTransaction<'static>, crate::queue::RoutingPlan)> {
    let view = state.view();
    let block_parameters = view.world().parameters().block();
    // The next block executes under the FASTPQ source policy frozen at its start (the
    // committed one): proposal packing honours its Network input cap, as validation does.
    let fastpq_inputs = block_parameters
        .fastpq_source()
        .maximum_network_inputs(block_parameters.execution_output())
        .map_or(0, |inputs| usize::try_from(inputs).unwrap_or(usize::MAX));
    let max_transactions = usize::try_from(block_parameters.max_transactions().get())
        .unwrap_or(usize::MAX)
        .min(fastpq_inputs)
        .saturating_sub(reserved);
    let Some((pending, lease)) = queue.bounded_pending_snapshot(&view, MAX_QUEUE_SCAN) else {
        return Vec::new();
    };
    drop(lease);
    // The global chain sequences lane 0; transactions routed to a lane reach it through that
    // lane's merged blocks (`specs/sumeragi_lanes.md` §5).
    let routing = super::lanes::routing::RoutingSnapshot::of(&view);
    let height = u64::try_from(view.height())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let inputs = routing.inputs(view.world());
    // A transaction routed to a lane that has not carried it for `2A` global block times is
    // rescued by the global chain itself (§6.4): a stalled lane cannot hold transactions, and
    // the rescue is the committed load that closes it.
    let rescue_before_ms = routing.policy().map_or(0, |policy| {
        let cadence = view.world().parameters().sumeragi().block_cadence_ms.get();
        let parent_ms = view
            .latest_block()
            .and_then(|block| u64::try_from(block.header().creation_time().as_millis()).ok())
            .unwrap_or(0);
        parent_ms.saturating_sub(
            policy
                .anchor_freshness
                .saturating_mul(2)
                .saturating_mul(cadence),
        )
    });
    let mut selected = Vec::new();
    let mut bytes = 0usize;
    for transaction in pending {
        if selected.len() >= max_transactions {
            break;
        }
        if routing.has_lanes()
            && inputs.route(&transaction, height) != super::lanes::routing::GLOBAL_LANE
            && u64::try_from(transaction.as_ref().creation_time().as_millis())
                .is_ok_and(|created| created >= rescue_before_ms)
        {
            continue;
        }
        // TODO(WP8a): QueuePlanSynced is deleted with the lane machinery.
        if transaction.entrypoint().admission_intent()
            == TransactionAdmissionIntent::QueuePlanSynced
        {
            continue;
        }
        let Ok(plan) = queue.route_plan_with_state(&transaction, state) else {
            continue;
        };
        let next = bytes.saturating_add(transaction.encoded_len());
        if next > max_bytes {
            continue;
        }
        bytes = next;
        selected.push((transaction, plan));
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, num::NonZeroU64};

    use iroha_data_model::{
        block::{BlockHeader, builder::BlockBuilder as WireBlockBuilder},
        isi::Log,
        level::Level,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

    #[test]
    fn decode_rejects_garbage_and_empty() {
        assert!(matches!(decode(&[]), Err(PayloadError::NotCanonical(_))));
        assert!(matches!(
            decode(&[1, 2, 3, 4]),
            Err(PayloadError::NotCanonical(_))
        ));
    }

    #[test]
    fn canonical_wire_cannot_hide_a_zero_transaction_block() {
        let block = WireBlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            1,
            0,
        ))
        .build(BTreeSet::new());
        let bytes = block.encode_wire().expect("canonical empty proposal");
        assert!(
            !bytes.is_empty(),
            "the transport payload itself is nonempty"
        );
        assert!(matches!(decode(&bytes), Err(PayloadError::EmptyBlock)));
        assert_eq!(encode(&block), Err(PayloadError::EmptyBlock));
    }

    #[test]
    fn assembly_refuses_empty_work_before_reading_state() {
        let state = State::new_for_testing(
            crate::state::World::new(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let parent = WireBlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            1,
            0,
        ))
        .build(BTreeSet::new());
        assert!(matches!(
            assemble(
                &state,
                Assembly {
                    parent: &parent,
                    view: 10,
                    cadence: Duration::from_millis(100),
                },
                &[]
            ),
            Err(PayloadError::EmptyBlock)
        ));
        assert_eq!(state.view().height(), 0);
    }

    #[test]
    fn canonical_nonempty_proposal_roundtrips_without_synthesized_work() {
        let mut builder = WireBlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            1,
            0,
        ));
        let transaction = TransactionBuilder::new(
            iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                    b"payload-test-network",
                )),
            ),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "payload transaction".to_owned())])
        .sign(ALICE_KEYPAIR.private_key());
        builder.push_transaction(transaction);
        let block = builder.build(BTreeSet::new());
        let bytes = encode(&block).expect("nonempty proposal");
        let decoded = decode(&bytes).expect("canonical nonempty proposal");
        assert_eq!(decoded.network_entrypoint_count(), 1);
        assert_eq!(decoded.encode_wire().unwrap(), bytes);
    }
}
