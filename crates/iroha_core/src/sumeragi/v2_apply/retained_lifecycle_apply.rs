//! Authenticate the exact durable Apply body and CommitQC before publication.

use super::{ExactApplyTaskRef, V2ApplyError, V2ApplyService};
use crate::{block::VerifiedV2FinalityArtifact, sumeragi::v2_body_store::V2BodyStore};
use iroha_crypto::Hash;
use iroha_data_model::block::{SignedBlock, consensus_v2 as wire};

/// Read-only authentication shared by ordinary and lifecycle Apply.
pub(super) struct AuthenticatedApplyBody {
    pub(super) body: SignedBlock,
    pub(super) verified_artifact: VerifiedV2FinalityArtifact,
    pub(super) canonical_proposal_wire_hash: Hash,
}

impl V2ApplyService {
    /// Authenticate without modifying Kura, State, Queue or retained execution.
    pub(super) fn authenticate_exact_apply_body(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        task: ExactApplyTaskRef<'_>,
    ) -> Result<AuthenticatedApplyBody, V2ApplyError> {
        context.validate()?;
        let durable = task.validated_receipt().durable();
        if task.subject() != task.certificate().subject
            || task.certificate().phase != wire::GlobalPhase::Commit
            || task.certificate().round.context_id != context.id()
            || task.certificate().round.height != context.height
            || durable.context_id() != context.id()
            || durable.round() != task.certificate().proposal_round
            || durable.subject() != task.subject()
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        task.certificate().execution_commitment.validate()?;
        if task.certificate().execution_commitment
            != task.validated_receipt().execution_commitment()
        {
            return Err(V2ApplyError::ExecutionCommitmentMismatch);
        }
        let body = body_store.load(durable)?;
        let canonical_proposal_wire_hash = body
            .canonical_proposal_wire_hash()
            .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;
        if !body.is_resultless_proposal()
            || body.hash() != task.subject().block_hash
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != task.subject().parent_block_hash
            || canonical_proposal_wire_hash != task.subject().payload_hash
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        let verified_artifact =
            VerifiedV2FinalityArtifact::verify(wire::finality::V2FinalityArtifact::new(
                context.clone(),
                task.subject(),
                task.certificate().clone(),
                self.validator_set_pops.clone(),
            ))
            .map_err(V2ApplyError::FinalityCryptography)?;
        verified_artifact
            .artifact()
            .validate_for_header(&body.header())?;
        Ok(AuthenticatedApplyBody {
            body,
            verified_artifact,
            canonical_proposal_wire_hash,
        })
    }
}
