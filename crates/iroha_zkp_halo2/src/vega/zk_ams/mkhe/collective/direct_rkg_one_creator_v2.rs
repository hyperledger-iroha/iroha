//! Private, inert V2 direct-RKG1 creator corridor.

use super::super::super::{
    ZkAmsMkheDirectRkgOneLifecycleStoreV2,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        PreparedDirectRkgOneStatementCoreV1, VerifiedPersistentWitnessBindingSetV1,
        prepare_direct_rkg_one_creator_h0_v1, prepare_direct_rkg_one_statement_permit_v1,
        seal_direct_rkg_one_proof_owner_v1,
    },
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    direct_object_transport::ZkAmsMkheDirectObjectCasPublicationV1,
};
use super::{
    StateOwnedDirectRkgEphemeralMembershipPrecursorV1, ZkAmsMkheCollectivePartyStateV1,
    ZkAmsMkheErrorV1,
    direct_rkg_one_candidate_v1::take_ready_direct_rkg_one_prover_session_v1,
    direct_rkg_one_publication_v1::{
        DirectRkgOneFreshReservationOutcomeV2,
        persist_direct_rkg_one_proof_published_unverified_v2, publish_direct_rkg_one_h0_h1_v1,
        reserve_direct_rkg_one_fresh_v2,
    },
    direct_rkg_one_sealed_candidate_v1::SealedDirectRkgOneCandidateV1,
};
use crate::{generalized_bulletproof::ProofRandomSource, vega::MaskedRelaxedRandomSourceV1};

/// Private construction corridor; no production caller or release reexport exists.
#[expect(
    dead_code,
    reason = "V2 lifecycle backend and release corridor remain unavailable"
)]
fn create_direct_rkg_one_sealed_candidate_v2<'a, P, R>(
    state: &'a mut ZkAmsMkheCollectivePartyStateV1,
    original_wrapper: StateOwnedDirectRkgEphemeralMembershipPrecursorV1,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    provider: &mut P,
    random: &mut R,
) -> Result<impl Sized + use<'a, P, R>, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ZkAmsMkheDirectRkgOneLifecycleStoreV2 + ?Sized,
    R: ProofRandomSource + MaskedRelaxedRandomSourceV1,
{
    let party_index = usize::from(state.party_index());
    let fresh = match reserve_direct_rkg_one_fresh_v2(roster, context, party_index, provider)? {
        DirectRkgOneFreshReservationOutcomeV2::Reserved(permit) => permit,
        DirectRkgOneFreshReservationOutcomeV2::Quarantined(_) => {
            return Err(ZkAmsMkheErrorV1::ReleaseUnavailable);
        }
    };
    let prover_session = take_ready_direct_rkg_one_prover_session_v1(
        state,
        original_wrapper,
        roster,
        bindings,
        context,
        random,
    )?;
    let h0_ready = prepare_direct_rkg_one_creator_h0_v1(roster, bindings, context, prover_session)?;
    let (completed, publication_owner, published_lifecycle) =
        publish_direct_rkg_one_h0_h1_v1(roster, h0_ready, fresh, provider)?;
    let permit = prepare_direct_rkg_one_statement_permit_v1(
        completed,
        publication_owner.h0_stream(),
        publication_owner.h1_stream(),
    )?;
    let statement_objects = publication_owner.statement_objects_v1()?;
    let statement_core =
        PreparedDirectRkgOneStatementCoreV1::new(context, &permit, &publication_owner)?;
    let proof_owner = seal_direct_rkg_one_proof_owner_v1(permit, statement_core, random)?;
    let proof_owner = proof_owner.publish_unverified_v2(provider)?;
    let lifecycle_owner = persist_direct_rkg_one_proof_published_unverified_v2(
        published_lifecycle,
        publication_owner,
        proof_owner,
        provider,
    )?;
    let candidate = SealedDirectRkgOneCandidateV1::from_durable_parts_v2(lifecycle_owner);
    candidate.verify_semantic_candidate_v1(context, statement_objects, provider)
}
