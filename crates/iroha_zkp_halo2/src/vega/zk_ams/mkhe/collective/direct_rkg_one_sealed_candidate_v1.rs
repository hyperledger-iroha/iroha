use super::super::super::{
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        DirectRelationPublicObjectsV1, PreparedDirectRkgOneStatementCoreV1,
        SealedDirectRkgOneProofOwnerV1, VerifiedPersistentWitnessBindingSetV1,
        prepare_direct_rkg_one_creator_h0_v1, prepare_direct_rkg_one_statement_permit_v1,
        seal_direct_rkg_one_proof_owner_v1,
    },
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    direct_object_transport::{
        ZkAmsMkheDirectObjectCasPublicationV1, ZkAmsMkheDirectObjectReadAtProviderV1,
    },
};
use super::super::{ZkAmsMkheCollectivePartyStateV1, ZkAmsMkheErrorV1};
use super::{
    StateOwnedDirectRkgEphemeralMembershipPrecursorV1,
    direct_rkg_one_candidate_v1::take_ready_direct_rkg_one_prover_session_v1,
    direct_rkg_one_publication_v1::{
        DirectRkgOnePublicationOwnerV1, publish_direct_rkg_one_h0_h1_v1,
    },
};
use crate::{generalized_bulletproof::ProofRandomSource, vega::MaskedRelaxedRandomSourceV1};
/// Unverified authority-neutral candidate; owns no verifier or successor receipt.
pub(in crate::vega::zk_ams::mkhe) struct SealedDirectRkgOneCandidateV1<'a> {
    proof_owner: SealedDirectRkgOneProofOwnerV1<'a>,
    _publication_owner: DirectRkgOnePublicationOwnerV1,
}
struct PostSemanticDirectRkgOneCandidateV1<S> {
    _proof_owner: S,
    _publication_owner: DirectRkgOnePublicationOwnerV1,
}
impl<'a> SealedDirectRkgOneCandidateV1<'a> {
    /// Named logical payload lower bounds, not heap/RSS, headroom, or certification: verification 170_096_534; post-success 128_022_422.
    #[expect(dead_code, reason = "private unconnected semantic handoff")]
    fn verify_semantic_candidate_v1<P>(
        self,
        context: ZkAmsMkheDirectCeremonyContextV1,
        objects: DirectRelationPublicObjectsV1,
        provider: &mut P,
    ) -> Result<impl Sized + use<'a, P>, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self._publication_owner.statement_objects_v1()? != objects {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let proof_owner = self.proof_owner;
        let publication_owner = self._publication_owner;
        let proof_owner = proof_owner.verify_semantic_candidate_v1(context, objects, provider)?;
        Ok(PostSemanticDirectRkgOneCandidateV1 {
            _proof_owner: proof_owner,
            _publication_owner: publication_owner,
        })
    }
}
/// Private construction corridor; deliberately unexported and uncalled.
// TODO: Wire verifier/admission consumption and durable state reinsertion before any release gate.
#[expect(dead_code, reason = "unconnected semantic precursor")]
fn create_direct_rkg_one_sealed_candidate_v1<'a, P, R>(
    state: &'a mut ZkAmsMkheCollectivePartyStateV1,
    original_wrapper: StateOwnedDirectRkgEphemeralMembershipPrecursorV1,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    publisher: &mut P,
    random: &mut R,
) -> Result<SealedDirectRkgOneCandidateV1<'a>, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    R: ProofRandomSource + MaskedRelaxedRandomSourceV1,
{
    let prover_session = take_ready_direct_rkg_one_prover_session_v1(
        state,
        original_wrapper,
        roster,
        bindings,
        context,
        random,
    )?;
    let h0_ready = prepare_direct_rkg_one_creator_h0_v1(roster, bindings, context, prover_session)?;
    let (completed, publication_owner) =
        publish_direct_rkg_one_h0_h1_v1(roster, h0_ready, publisher)?;
    let permit = prepare_direct_rkg_one_statement_permit_v1(
        completed,
        publication_owner.h0_stream(),
        publication_owner.h1_stream(),
    )?;
    let statement_core =
        PreparedDirectRkgOneStatementCoreV1::new(context, &permit, &publication_owner)?;
    let proof_owner = seal_direct_rkg_one_proof_owner_v1(permit, statement_core, random)?;
    Ok(SealedDirectRkgOneCandidateV1 {
        proof_owner,
        _publication_owner: publication_owner,
    })
}
