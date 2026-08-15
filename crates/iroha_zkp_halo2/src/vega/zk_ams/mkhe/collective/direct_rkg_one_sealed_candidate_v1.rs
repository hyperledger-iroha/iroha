use super::super::super::{
    active_exact_binding::DirectRelationPublicObjectsV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    direct_object_transport::ZkAmsMkheDirectObjectReadAtProviderV1,
};
use super::{
    ZkAmsMkheErrorV1,
    direct_rkg_one_publication_v1::DirectRkgOneProofPublishedUnverifiedOwnerV2,
};

/// Unverified authority-neutral candidate retaining the complete durable publication/proof owner.
pub(in crate::vega::zk_ams::mkhe) struct SealedDirectRkgOneCandidateV1<'a> {
    lifecycle_owner: DirectRkgOneProofPublishedUnverifiedOwnerV2<'a>,
}

struct PostSemanticDirectRkgOneCandidateV1<S> {
    _lifecycle_owner: S,
}

impl<'a> SealedDirectRkgOneCandidateV1<'a> {
    /// Sole private constructor; only the V2 creator can supply the completed durable owner.
    pub(super) const fn from_durable_parts_v2(
        lifecycle_owner: DirectRkgOneProofPublishedUnverifiedOwnerV2<'a>,
    ) -> Self {
        Self { lifecycle_owner }
    }

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
        if self.lifecycle_owner.statement_objects_v2()? != objects {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let lifecycle_owner = self
            .lifecycle_owner
            .verify_semantic_candidate_v2(context, objects, provider)?;
        Ok(PostSemanticDirectRkgOneCandidateV1 {
            _lifecycle_owner: lifecycle_owner,
        })
    }
}
