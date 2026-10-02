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

use iroha_primitives::time::TimeSource;

use crate::{
    block::{BlockBuilder, ValidBlock},
    queue::Queue,
    state::{State, StateReadOnly, WorldReadOnly, compute_confidential_feature_digest},
    tx::AcceptedTransaction,
};
use iroha_data_model::{
    block::{BlockExecutionContextBundle, SignedBlock},
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
    /// Original parent-state staking preparation failed.
    #[error("staking effect preparation failed: {0}")]
    Staking(String),
    /// The versioned decoder stopped at a local resource ceiling or allocation refusal.
    /// Its public error preserves the category but does not carry the nested limit fields.
    #[error("payload decoding was refused by local resources")]
    DecodeResource,
    /// Original committed routing could not be read with the local resources.
    #[error("payload routing deferred: {0}")]
    RoutingDeferred(#[from] crate::execution_attempt::ExecutionDeferred),
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
    transactions: &[AcceptedTransaction<'static>],
) -> Result<SignedBlock, PayloadError> {
    assemble_with_merges(state, assembly, transactions, &MergeProposal::default())
}

/// The lane merges a leader proposes (`specs/sumeragi_lanes.md` §4.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeProposal {
    /// The merged ranges, lanes ascending.
    pub merges: Vec<SumeragiLaneMerge>,
    /// Transactions of the fresh merged blocks (the block capacity they reserve).
    pub transactions: usize,
    /// One millisecond after the latest creation time among those transactions.
    pub time_floor_ms: u64,
}

/// Assemble the block's own transactions and its lane merges (`specs/sumeragi_lanes.md` §4.2)
/// without beacon control, which is supplied separately in the native header. Merged lane
/// blocks are work: a block may carry merges alone.
///
/// # Errors
/// There are neither transactions nor merges, or the canonical block time overflows.
pub fn assemble_with_merges(
    state: &State,
    assembly: Assembly<'_>,
    transactions: &[AcceptedTransaction<'static>],
    merges: &MergeProposal,
) -> Result<SignedBlock, PayloadError> {
    if transactions.is_empty() && merges.merges.is_empty() {
        return Err(PayloadError::EmptyBlock);
    }
    let parent_time = assembly.parent.header().creation_time();
    let minimum = parent_time
        .checked_add(assembly.cadence)
        .ok_or(PayloadError::TimeOverflow)?;
    let build = |time: Duration| -> Result<SignedBlock, PayloadError> {
        build_at(state, assembly, transactions, merges, time)
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
    transactions: &[AcceptedTransaction<'static>],
    merges: &MergeProposal,
    time: Duration,
) -> Result<SignedBlock, PayloadError> {
    let height = assembly.parent.header().height().get().saturating_add(1);
    let (_, time_source) = TimeSource::new_mock(time);
    let accepted = transactions.iter().cloned().collect::<Vec<_>>();
    let nexus = state.nexus_snapshot();
    let view = state.view();
    let npos = view.world().sumeragi_npos_parameters().is_some();
    let confidential = compute_confidential_feature_digest(view.world(), view.zk(), height);
    let routing = super::lanes::routing::RoutingSnapshot::of(&view)?;
    let inputs = routing.inputs(view.world());
    let contexts = transactions
        .iter()
        .map(|tx| {
            let route = inputs.execution_route(tx, height)?.ok_or_else(|| {
                PayloadError::NotCanonical("committed execution route is unavailable".into())
            })?;
            Ok(iroha_data_model::block::ExternalExecutionContext::new(
                tx.hash_as_entrypoint(),
                route.lane_id,
                route.dataspace_id,
            ))
        })
        .collect::<Result<Vec<_>, PayloadError>>()?;
    drop(view);
    let mut execution_context = BlockExecutionContextBundle::new(contexts);
    if !merges.merges.is_empty() {
        execution_context.lane_merge = Some(SumeragiLaneMergeSection {
            merges: merges.merges.clone(),
            time_floor_ms: merges.time_floor_ms,
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
        .with_network_input_time_floor(time)
        .ok_or(PayloadError::TimeOverflow)?;
    let mut proposal = builder.into_unsigned_proposal();
    let effects = if npos {
        let header = proposal.header();
        Some(
            super::penalties::PenaltyApplier::new(state, None)
                .derive_npos_consensus_effects(&header)
                .map_err(|error| PayloadError::Staking(error.to_string()))?,
        )
    } else {
        None
    };
    proposal.set_npos_consensus_effects(effects);
    Ok(proposal)
}

/// Whether the canonical proposal carries transactions or certified lane merge work.
fn has_work(block: &SignedBlock) -> bool {
    block.has_consensus_work()
}

/// The payload bytes of `block`: its canonical resultless proposal wire.
///
/// # Errors
/// The block carries no work or cannot be encoded.
pub fn encode(block: &SignedBlock) -> Result<Vec<u8>, PayloadError> {
    if !has_work(block) {
        return Err(PayloadError::EmptyBlock);
    }
    let len = block
        .resultless_proposal_wire_len()
        .map_err(|error| PayloadError::Encode(error.to_string()))?;
    let mut wire = Vec::with_capacity(len);
    block
        .write_resultless_proposal_wire(&mut wire)
        .map_err(|error| PayloadError::Encode(error.to_string()))?;
    Ok(wire)
}

/// Decode a non-empty payload: the canonical wire of an unsigned, resultless proposal without
/// a certificate (re-encoding must reproduce the bytes exactly).
///
/// # Errors
/// The bytes do not decode, are not canonical, carry a result, a certificate or a signature,
/// or the decoded block has no transactions. Local decoder resource refusal remains
/// [`PayloadError::DecodeResource`], rather than a deterministic property of the bytes.
pub fn decode(payload: &[u8]) -> Result<SignedBlock, PayloadError> {
    let block =
        iroha_data_model::block::decode_versioned_signed_block(payload).map_err(|error| {
            if error.is_decode_resource_limit() {
                PayloadError::DecodeResource
            } else {
                PayloadError::NotCanonical(error.to_string())
            }
        })?;
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
/// Fresh inputs routed to a native lane are selected by that lane; the global
/// chain selects direct inputs and rescues inputs whose lane has stalled.
pub fn select(
    state: &State,
    queue: &std::sync::Arc<Queue>,
    max_bytes: usize,
    reserved: usize,
) -> Result<Vec<AcceptedTransaction<'static>>, crate::execution_attempt::ExecutionDeferred> {
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
    let Some(pending) = queue.bounded_pending_snapshot(&view, MAX_QUEUE_SCAN) else {
        return Ok(Vec::new());
    };
    // The global chain sequences lane 0; transactions routed to a lane reach it through that
    // lane's merged blocks (`specs/sumeragi_lanes.md` §5).
    let routing = super::lanes::routing::RoutingSnapshot::of(&view)?;
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
        let Some(lane) = inputs.route(&transaction, height)? else {
            continue;
        };
        if routing.has_lanes()
            && lane != super::lanes::routing::GLOBAL_LANE
            && u64::try_from(transaction.as_ref().creation_time().as_millis())
                .is_ok_and(|created| created >= rescue_before_ms)
        {
            continue;
        }
        let next = bytes.saturating_add(transaction.encoded_len());
        if next > max_bytes {
            continue;
        }
        bytes = next;
        selected.push(transaction);
    }
    Ok(selected)
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
    fn encode_rejects_a_merge_suffix_larger_than_the_input_sequence() {
        use iroha_data_model::{
            block::{BlockExecutionContextBundle, ExternalExecutionContext},
            sumeragi_finality::test_fixtures::NativeFinalityFixture,
            sumeragi_lanes::{SumeragiLaneMerge, SumeragiLaneMergeSection},
        };
        use iroha_model_base::topology::{DataSpaceId, LaneId};

        let fixture = NativeFinalityFixture::start("invalid-payload-projection");
        let source = fixture.block_with_submitted_work(fixture.next_header());
        let input = source.external_transactions().next().unwrap().clone();
        let mut context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            input.hash_as_entrypoint(),
            LaneId::new(0),
            DataSpaceId::new(0),
        )]);
        context.lane_merge = Some(SumeragiLaneMergeSection {
            merges: vec![SumeragiLaneMerge {
                lane: LaneId::new(16),
                incarnation: [1; 32],
                from: 1,
                to: 2,
                tip_hash: [2; 32],
                tip_result: [3; 32],
            }],
            time_floor_ms: 0,
            merged_count: 2,
        });
        let mut builder = WireBlockBuilder::new(source.header());
        builder.push_transaction(input);
        builder.set_execution_context(Some(context));
        let malformed = builder.build(BTreeSet::new());
        assert!(matches!(encode(&malformed), Err(PayloadError::Encode(_))));
    }

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
        assert_eq!(bytes, block.encode_wire().unwrap());
        let decoded = decode(&bytes).expect("canonical nonempty proposal");
        assert_eq!(decoded.network_entrypoint_count(), 1);
        assert_eq!(decoded.encode_wire().unwrap(), bytes);
    }
    #[test]
    fn signed_native_lane_policy_drives_direct_global_context_without_queue_override() {
        use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{
            account::{Account, AccountId},
            isi::Register,
            parameter::Parameter,
            sumeragi_lanes::{
                SumeragiFixedLane, SumeragiLaneMember, SumeragiLanePolicy, SumeragiLaneRoute,
            },
        };
        use iroha_model_base::{
            peer::PeerId,
            topology::{DataSpaceId, LaneId},
        };
        let user = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let account = AccountId::new(user.public_key().clone());
        let mut committee = (0x61..=0x64)
            .map(|seed| {
                let key = KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal);
                SumeragiLaneMember {
                    peer: PeerId::new(key.public_key().clone()),
                    pop: iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                }
            })
            .collect::<Vec<_>>();
        committee.sort();
        let policy = SumeragiLanePolicy {
            da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
            anchor_freshness: 4,
            max_merge_blocks: 8,
            stall_window: 1000,
            lane_params: Default::default(),
            fixed: vec![SumeragiFixedLane {
                lane: LaneId::new(2),
                dataspace: DataSpaceId::UNIVERSAL,
                committee,
            }],
            routes: vec![SumeragiLaneRoute {
                lane: LaneId::new(2),
                account: Some(account.to_string()),
                instruction: None,
            }],
            autoscale: None,
        };
        let mut config = TestChainConfig::new(crate::state::World::new(), 1000);
        config
            .genesis_instructions
            .push(Register::account(Account::new(account.clone())).into());
        config
            .genesis_parameters
            .push(Parameter::Custom(policy.into_custom_parameter()));
        let mut chain = CertifiedTestChain::start(config).unwrap();
        chain.commit_at(2000, Vec::new());
        chain.commit_at(3000, Vec::new());
        let view = chain.state().view();
        assert!(
            view.world()
                .sumeragi_lanes()
                .lane(LaneId::new(2))
                .unwrap()
                .admits_anchor(3)
        );
        let parent = view.latest_block().unwrap();
        let mut builder = TransactionBuilder::new(
            chain.network_id(),
            account,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "direct rescue".to_owned())]);
        builder.set_creation_time(Duration::from_millis(3000));
        let (_, clock) = TimeSource::new_mock(Duration::from_millis(3001));
        let accepted = AcceptedTransaction::accept_with_time_source(
            builder.sign(user.private_key()),
            &chain.network_id(),
            Duration::from_secs(1),
            view.world().parameters().transaction(),
            &iroha_config::parameters::actual::Crypto::default(),
            &clock,
        )
        .unwrap();
        drop(view);
        let proposal = assemble(
            chain.state(),
            Assembly {
                parent: &parent,
                view: 0,
                cadence: Duration::from_millis(1),
            },
            &[accepted],
        )
        .unwrap();
        let contexts = &proposal.execution_context().unwrap().external;
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].lane_id, LaneId::new(2));
        assert_eq!(contexts[0].dataspace_id, DataSpaceId::UNIVERSAL);
        assert_eq!(contexts[0].routing_plan_legs.len(), 1);
        assert!(
            proposal.lane_merge().is_none(),
            "a direct rescue carries no fabricated lane certificate"
        );
        assert_eq!(decode(&encode(&proposal).unwrap()).unwrap(), proposal);
    }
}

#[cfg(test)]
#[path = "payload/work_tests.rs"]
mod work_tests;
