//! Test-only publication of one genuinely prepared governance carrier.
//!
//! The helper consumes the original preparation and actual signed quorum. It has
//! no raw StateBlock/ValidBlock converter, output-capacity setter or reexecution
//! path. Unit admissions below do not qualify complete production funding.

use super::*;
use crate::block::VerifiedV2FinalityArtifact;
use crate::state::PreparedCarrier;
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::block::consensus_v2 as wire;

/// Publish the same prepared test candidate through exact durable finality.
/// No replacement execution or caller-supplied checkpoint is accepted.
pub(crate) fn publish_governance_fixture(
    state: &State,
    prepared: PreparedCarrier<'_>,
    keys: &[KeyPair],
) -> wire::finality::V2FinalityArtifact {
    let context = prepared.context().clone();
    assert_eq!(context.height, state.committed_height() as u64 + 1);
    assert_eq!(context.network_id, *state.network_id_ref());
    if prepared.block().header().is_genesis() {
        // The immutable SignedBlock copy is inspected while its sole executed
        // State owner is still retained. It is never executed a second time.
        let bootstrap = crate::sumeragi::freeze_staged_genesis_v2(
            &iroha_genesis::GenesisBlock(prepared.block().clone()),
            prepared.state(),
            context.mode,
        )
        .expect("authenticate the same prepared signed-genesis context");
        assert_eq!(bootstrap.context(), &context);
    }
    let subject = wire::BlockSubject {
        parent_block_hash: prepared.block().header().prev_block_hash(),
        block_hash: prepared.block().hash(),
        payload_hash: prepared.block().canonical_proposal_wire_hash().unwrap(),
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: prepared.block().header().view_change_index(),
    };
    let execution_commitment = prepared.execution_prefix_commitment();
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    assert_eq!(context.roster.len(), 4);
    let keys = context
        .roster
        .iter()
        .map(|member| {
            keys.iter()
                .find(|key| key.public_key() == member.validator.public_key())
                .expect("exact signed fixture roster key")
        })
        .collect::<Vec<_>>();
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
            execution_commitment,
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
    let verified = VerifiedV2FinalityArtifact::verify(artifact)
        .expect("verify exact three-of-four fixture CommitQC");
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(()))
        .unwrap_or_else(|error| panic!("capture original governance journals: {error}"));
    let checkpoint_hash = journals.checkpoint;
    let decision = journals
        .bind_decision(verified, |_| Ok::<_, Infallible>(()))
        .unwrap_or_else(|refusal| panic!("bind exact governance decision: {:?}", refusal.error));
    let finality = decision.finality().clone();
    state
        .kura
        .store_block(decision.block().clone())
        .expect("persist actual governance block");
    let durable = state
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist exact governance finality");
    let checkpoint = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&durable, checkpoint_hash)
        .expect("persist original governance checkpoint");
    let decision = decision.attach_checkpoint(checkpoint);
    let generation = state.state_view_generation();
    let physical = decision
        .try_prepare_physical(state, None, |_, _| Ok::<_, Infallible>(()))
        .unwrap_or_else(|(_, error)| panic!("acquire original governance publication: {error:?}"));
    let published = physical
        .publish()
        .unwrap_or_else(|(_, error)| panic!("publish original governance carrier: {error:?}"));
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_eq!(
        state.latest_block_hash_fast(),
        Some(published.block().hash())
    );
    assert_eq!(
        checkpoint_hash,
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap()
    );
    drop(published);
    finality
}
