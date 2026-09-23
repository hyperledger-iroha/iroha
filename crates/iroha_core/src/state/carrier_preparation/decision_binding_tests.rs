//! Actual execution plus exact three-of-four signed decisions, without publication.

use super::*;
use crate::{state::State, sumeragi::network_topology::Topology};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::block::consensus_v2 as wire;
use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Reservation(Arc<AtomicUsize>);
impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn captured(
    state: &State,
    proposal: SignedBlock,
    topology: &Topology,
    context: &HeightContext,
    released: &Arc<AtomicUsize>,
) -> PreparedCarrierJournals<Reservation> {
    super::super::super::tests::prepare(state, proposal, topology, context)
        .unwrap_or_else(|(_, error)| panic!("actual carrier execution: {error}"))
        .prepare_journals(None, None, |_| {
            Ok::<_, Infallible>(Reservation(Arc::clone(released)))
        })
        .unwrap_or_else(|error| panic!("capture original journals: {error}"))
}

pub(super) fn subject(block: &SignedBlock) -> wire::BlockSubject {
    wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block.canonical_proposal_wire_hash().unwrap(),
    }
}

/// Sign the exact supplied test statement with the actual fixture's four keys.
/// Mutant statements still need a valid quorum and are rejected by binding, not
/// a forged VerifiedV2FinalityArtifact or unchecked finality constructor.
pub(super) fn signed_finality(
    context: HeightContext,
    subject: wire::BlockSubject,
    execution: ExecutionCommitment,
    view: u64,
) -> VerifiedV2FinalityArtifact {
    let keys = (0_u8..4)
        .map(|index| KeyPair::try_from_seed(vec![0xB0 + index; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(context.roster.len(), 4);
    let keys = context
        .roster
        .iter()
        .map(|member| {
            keys.iter()
                .find(|key| key.public_key() == member.validator.public_key())
                .expect("actual fixture roster key")
        })
        .collect::<Vec<_>>();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: execution,
        signer: 0,
        signature: Vec::new(),
    };
    let shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &vote.signature_preimage())
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let artifact = wire::finality::V2FinalityArtifact::new(
        context,
        subject,
        wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: execution,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .unwrap(),
        },
        keys.iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect(),
    );
    VerifiedV2FinalityArtifact::verify(artifact).expect("exact three-of-four signed test decision")
}

fn exact_finality<A>(journals: &PreparedCarrierJournals<A>) -> VerifiedV2FinalityArtifact {
    signed_finality(
        (*journals.context).clone(),
        subject(journals.valid.as_ref()),
        journals.execution_prefix,
        journals.valid.as_ref().header().view_change_index(),
    )
}

#[test]
fn exact_decision_retains_original_journals_and_survives_static_handoff_without_apply() {
    fn assert_static_send<T: Send + 'static>() {}
    assert_static_send::<DecisionBoundCarrierJournals<Reservation>>();
    let (state, proposal, topology, context) = super::super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let capture_released = Arc::new(AtomicUsize::new(0));
    let journals = captured(&state, proposal, &topology, &context, &capture_released);
    let events_allocation = journals.publication_events.as_ptr();
    let checkpoint = journals.checkpoint;
    let wire = journals.valid.as_ref().encode_wire().unwrap();
    let finality = exact_finality(&journals);
    let finality_bytes = finality.artifact().clone();
    assert_eq!(journals.context.as_ref(), &context);
    assert_eq!(
        journals.execution_prefix,
        finality.artifact().commit_qc.execution_commitment
    );
    let decided = journals
        .bind_decision(finality)
        .unwrap_or_else(|refusal| panic!("bind actual execution: {}", refusal.error));
    assert_eq!(decided.block().encode_wire().unwrap(), wire);
    assert_eq!(decided.finality(), &finality_bytes);
    assert_eq!(decided.journals.checkpoint, checkpoint);
    assert_eq!(
        decided.journals.publication_events.as_ptr(),
        events_allocation
    );
    assert_eq!(
        decided.journals.valid.verified_v2_finality_artifact(),
        Some(decided.finality())
    );
    assert_eq!(
        decided.journals.source_prefix.sources().proposal(),
        decided.block().hash()
    );
    decided
        .journals
        .source_prefix
        .inventory()
        .verify_ordinary_witness_bundles(
            &decided.journals.source_prefix.witness().fastpq_transcripts,
        )
        .unwrap();
    assert_eq!(decided.committed_event.status, BlockStatus::Committed);
    assert_eq!(
        decided.publication_authorization(),
        PendingCarrierPublicationAuthorization::SourceAndDurability
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.kura.blocks_count(), 0);
    let kura = Arc::clone(&state.kura);
    drop(state);
    let decided = std::thread::spawn(move || decided).join().unwrap();
    assert_eq!(capture_released.load(Ordering::SeqCst), 0);
    drop(decided);
    assert_eq!(capture_released.load(Ordering::SeqCst), 1);
    assert_eq!(kura.blocks_count(), 0);
}

#[test]
fn other_signed_context_retains_original_capture_owner_for_retry() {
    let (state, proposal, topology, context) = super::super::super::tests::fixture();
    let released = Arc::new(AtomicUsize::new(0));
    let journals = captured(&state, proposal, &topology, &context, &released);
    let mut foreign_context = context.clone();
    foreign_context.execution_policy_hash = Hash::new(b"different authenticated policy");
    let foreign = signed_finality(
        foreign_context,
        subject(journals.valid.as_ref()),
        journals.execution_prefix,
        journals.valid.as_ref().header().view_change_index(),
    );
    let wire = journals.valid.as_ref().encode_wire().unwrap();
    let refusal = match journals.bind_decision(foreign) {
        Err(refusal) => refusal,
        Ok(_) => panic!("foreign context accepted"),
    };
    assert!(matches!(
        refusal.error,
        CarrierDecisionBindingError::Context
    ));
    assert_eq!(refusal.journals.valid.as_ref().encode_wire().unwrap(), wire);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    let exact = exact_finality(&refusal.journals);
    let decided = refusal
        .journals
        .bind_decision(exact)
        .unwrap_or_else(|refusal| panic!("retry original owner: {}", refusal.error));
    drop(decided);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn same_header_signed_foreign_proposal_returns_every_original_journal_and_guard() {
    let (state, proposal, topology, context) = super::super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let released = Arc::new(AtomicUsize::new(0));
    let journals = captured(&state, proposal, &topology, &context, &released);
    let events = journals.publication_events.as_ptr();
    let checkpoint = journals.checkpoint;
    let mut foreign_subject = subject(journals.valid.as_ref());
    foreign_subject.payload_hash = Hash::new(b"another canonical signed proposal");
    let foreign = signed_finality(
        context,
        foreign_subject,
        journals.execution_prefix,
        journals.valid.as_ref().header().view_change_index(),
    );
    let expected_artifact = foreign.artifact().clone();
    let refusal = match journals.bind_decision(foreign) {
        Err(refusal) => refusal,
        Ok(_) => panic!("foreign signed payload accepted"),
    };
    assert!(matches!(
        refusal.error,
        CarrierDecisionBindingError::Block(_)
    ));
    assert_eq!(refusal.finality.artifact(), &expected_artifact);
    assert_eq!(refusal.journals.publication_events.as_ptr(), events);
    assert_eq!(refusal.journals.checkpoint, checkpoint);
    assert!(
        refusal
            .journals
            .components
            .world
            .matches_current(&state.world)
    );
    assert!(refusal.journals.components.runtime.matches_current(&state));
    assert!(
        refusal
            .journals
            .components
            .block_hashes
            .matches_current(&state.block_hashes)
    );
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    let exact = exact_finality(&refusal.journals);
    drop(
        refusal
            .journals
            .bind_decision(exact)
            .unwrap_or_else(|refusal| panic!("retry exact finality: {}", refusal.error)),
    );
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

#[test]
fn verified_decision_for_different_execution_cannot_replace_the_retained_prefix() {
    let (state, proposal, topology, context) = super::super::super::tests::fixture();
    let released = Arc::new(AtomicUsize::new(0));
    let journals = captured(&state, proposal, &topology, &context, &released);
    let actual = journals.execution_prefix;
    let mut changed = actual;
    changed.ordinary_writes_root = Hash::new(b"other execution writes");
    let foreign = signed_finality(
        context,
        subject(journals.valid.as_ref()),
        changed,
        journals.valid.as_ref().header().view_change_index(),
    );
    let refusal = match journals.bind_decision(foreign) {
        Err(refusal) => refusal,
        Ok(_) => panic!("foreign execution accepted"),
    };
    assert!(matches!(
        refusal.error,
        CarrierDecisionBindingError::Execution
    ));
    assert_eq!(refusal.journals.execution_prefix, actual);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(refusal);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

#[test]
fn exact_wire_signature_substitution_is_rejected_by_the_canonical_block_owner() {
    let (state, proposal, topology, context) = super::super::super::tests::fixture();
    let released = Arc::new(AtomicUsize::new(0));
    let mut journals = captured(&state, proposal, &topology, &context, &released);
    let exact = exact_finality(&journals);
    let original_signatures = journals.valid.as_ref().signatures().cloned().collect();
    // Only the test module can mutate this retained ValidBlock. Add a real
    // header signature without changing its header, inputs or execution rows.
    let extra = KeyPair::try_from_seed(vec![0xFE; 32], Algorithm::BlsNormal).unwrap();
    journals.valid.as_mut().sign(extra.private_key(), 99);
    let substituted_wire = journals.valid.as_ref().encode_wire().unwrap();
    let refusal = match journals.bind_decision(exact.clone()) {
        Err(refusal) => refusal,
        Ok(_) => panic!("changed exact signed wire accepted"),
    };
    assert!(matches!(
        refusal.error,
        CarrierDecisionBindingError::Block(_)
    ));
    let mut journals = refusal.journals;
    assert_eq!(
        journals.valid.as_ref().encode_wire().unwrap(),
        substituted_wire
    );
    journals
        .valid
        .as_mut()
        .replace_signatures(original_signatures)
        .unwrap();
    drop(
        journals
            .bind_decision(exact)
            .unwrap_or_else(|refusal| panic!("original exact wire: {}", refusal.error)),
    );
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn original_capture_pool_remains_reserved_through_decision_binding_and_handoff() {
    use mv::allocation::{AllocationBudget, AllocationRefusal};
    let (state, proposal, topology, context) = super::super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let prepared = super::super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("actual carrier execution: {error}"));
    let budget = AllocationBudget::new(64 << 20);
    // This real finite pool funds the exact World wrapper and effects layouts.
    // It deliberately makes no claim to cover nested execution payloads.
    let journals = prepared
        .prepare_journals(None, None, |inputs| {
            let bytes = inputs
                .world_journal_shell_bytes()?
                .checked_add(inputs.retained_effects_layout.size())
                .ok_or(AllocationRefusal::DemandOverflow)?;
            budget.try_reserve_bytes(bytes)
        })
        .unwrap();
    let reserved = budget.reserved_bytes();
    assert!(reserved > 0);
    let remaining = budget
        .try_reserve_bytes(budget.limit_bytes() - reserved)
        .unwrap();
    let refusal = budget
        .try_reserve_bytes(1)
        .err()
        .expect("original owner consumes the pool");
    assert!(matches!(refusal, AllocationRefusal::Capacity { .. }));
    let finality = exact_finality(&journals);
    let wire = journals.valid.as_ref().encode_wire().unwrap();
    let events = journals.publication_events.as_ptr().addr();
    let effects = std::ptr::from_ref(journals.effects.as_ref()).addr();
    let decided = journals
        .bind_decision(finality)
        .unwrap_or_else(|refusal| panic!("exact original decision: {}", refusal.error));
    let decided = std::thread::spawn(move || decided).join().unwrap();
    assert_eq!(decided.block().encode_wire().unwrap(), wire);
    assert_eq!(decided.journals.publication_events.as_ptr().addr(), events);
    assert_eq!(
        std::ptr::from_ref(decided.journals.effects.as_ref()).addr(),
        effects
    );
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    drop(remaining);
    assert_eq!(budget.reserved_bytes(), reserved);
    drop(decided);
    assert_eq!(budget.reserved_bytes(), 0);
    drop(budget.try_reserve_bytes(budget.limit_bytes()).unwrap());
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn equivalent_parent_commit_witnesses_share_the_verified_decision_context_join() {
    let (state, proposal, topology, context) = super::super::super::tests::fixture();
    let released = Arc::new(AtomicUsize::new(0));
    let journals = captured(&state, proposal, &topology, &context, &released);
    let parent = exact_finality(&journals);
    let redecided = signed_finality(
        context.clone(),
        parent.artifact().subject,
        journals.execution_prefix,
        parent.artifact().commit_qc.round.view + 1,
    );
    let mut left = context;
    left.height += 1;
    left.parent_commit_qc = Some(parent.artifact().commit_qc.clone());
    left.validate()
        .expect("authenticated parent decision context");
    let mut right = left.clone();
    right.parent_commit_qc = Some(redecided.artifact().commit_qc.clone());
    right
        .validate()
        .expect("equivalent parent decision context");
    assert_ne!(left, right, "different valid parent decision witnesses");
    assert_eq!(left.id(), right.id(), "one canonical consensus identity");
    let child_subject = wire::BlockSubject {
        parent_block_hash: Some(parent.artifact().block_hash),
        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            b"context-join child header",
        )),
        payload_hash: Hash::new(b"context-join child proposal"),
    };
    // This isolates the authenticated context join. It neither claims an
    // executed child body nor permits its publication or execution reuse.
    let child = signed_finality(right.clone(), child_subject, journals.execution_prefix, 0);
    assert!(decision_has_retained_context_identity(&left, &child));
    assert!(decision_has_retained_context_identity(&right, &child));
    right.execution_policy_hash = Hash::new(b"different child execution policy");
    let other_policy = signed_finality(right, child_subject, journals.execution_prefix, 0);
    assert!(!decision_has_retained_context_identity(
        &left,
        &other_policy
    ));
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
    drop(journals);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}
