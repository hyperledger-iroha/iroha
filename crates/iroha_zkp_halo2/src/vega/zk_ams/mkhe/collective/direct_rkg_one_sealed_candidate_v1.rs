use super::super::super::{
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        PreparedDirectRkgOneStatementCoreV1, SealedDirectRkgOneProofOwnerV1,
        VerifiedPersistentWitnessBindingSetV1, prepare_direct_rkg_one_creator_h0_v1,
        prepare_direct_rkg_one_statement_permit_v1, seal_direct_rkg_one_proof_owner_v1,
    },
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    direct_object_transport::ZkAmsMkheDirectObjectCasPublicationV1,
};
use super::super::{ZkAmsMkheCollectivePartyStateV1, ZkAmsMkheErrorV1};
use super::{
    StateOwnedDirectRkgEphemeralMembershipPrecursorV1,
    direct_rkg_one_candidate_v1::take_ready_direct_rkg_one_owner_v1,
    direct_rkg_one_publication_v1::{
        DirectRkgOnePublicationOwnerV1, publish_direct_rkg_one_h0_h1_v1,
    },
};
use crate::{generalized_bulletproof::ProofRandomSource, vega::MaskedRelaxedRandomSourceV1};
/// Unverified authority-neutral candidate; owns no verifier or successor receipt.
/// Its proof owner retains the consumed opening, original wrapper, post-CPK
/// guard, and finalized capability; its publication owner retains both typed
/// stream and move-only CAS/readback receipts.
pub(in crate::vega::zk_ams::mkhe) struct SealedDirectRkgOneCandidateV1<'a> {
    proof_owner: SealedDirectRkgOneProofOwnerV1<'a>,
    _publication_owner: DirectRkgOnePublicationOwnerV1,
}
struct CompactedSealedDirectRkgOneCandidateV1<P> {
    _proof_owner: P,
    _publication_owner: DirectRkgOnePublicationOwnerV1,
}
impl<'a> SealedDirectRkgOneCandidateV1<'a> {
    pub(in crate::vega::zk_ams::mkhe) fn proof_bytes(&self) -> &[u8] {
        self.proof_owner.proof_bytes()
    }
    /// The byte figures are logical payload lower bounds, not heap, RSS,
    /// headroom, or certification: saves 42_074_112; candidate 26_308_918;
    /// prospective retained-wrapper peak 128_093_990.
    #[expect(dead_code, reason = "private unconnected compaction precursor")]
    fn into_compacted_sealed_candidate_v1(self) -> impl Sized + 'a {
        CompactedSealedDirectRkgOneCandidateV1 {
            _proof_owner: self.proof_owner.into_compacted_post_seal_v1(),
            _publication_owner: self._publication_owner,
        }
    }
}
/// Private construction corridor; deliberately unexported and uncalled.
#[expect(
    dead_code,
    reason = "current precursor has no production caller or gate"
)]
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
    let provenance = take_ready_direct_rkg_one_owner_v1(
        state,
        original_wrapper,
        roster,
        bindings,
        context,
        random,
    )?;
    let h0_ready = prepare_direct_rkg_one_creator_h0_v1(roster, bindings, context, provenance)?;
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
