//! Prover-only membership and challenge contexts from retained authority axes.

use super::super::{DIRECT_RELATION_CODEC_VERSION_V1, RELATION_LINEAGE_DOMAIN_V1};
use crate::vega::{
    sponge::Keccak256,
    zk_ams::mkhe::{
        ZkAmsMkheErrorV1,
        active_exact_binding::{
            ExactBindingTranscriptContextV1, PersistentDirectRelationV1,
            PreparedDirectRkgOneCreatorPermitV1,
        },
        exact_eight_chunk_membership::{
            ExactEightChunkMembershipContextV1, ExactEightChunkMembershipRoleV1,
        },
    },
};

impl PreparedDirectRkgOneCreatorPermitV1<'_> {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn membership_context_v1<R>(
        &self,
        share_statement_digest: [u8; 32],
    ) -> Result<ExactEightChunkMembershipContextV1<R>, ZkAmsMkheErrorV1>
    where
        R: ExactEightChunkMembershipRoleV1,
    {
        let context = self.context();
        ExactEightChunkMembershipContextV1::new(
            context.profile_digest(),
            context.roster_digest(),
            context.key_material_digest(),
            context.epoch(),
            context.transcript_digest(),
            self.prover_axes_v1().party,
            share_statement_digest,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }

    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn transcript_context_v1(
        &self,
        statement_core_digest: [u8; 32],
        commitment_set_digest: [u8; 32],
        membership_proof_set_digest: [u8; 32],
    ) -> Result<ExactBindingTranscriptContextV1, ZkAmsMkheErrorV1> {
        let context = self.context();
        let axes = self.prover_axes_v1();
        let transcript = ExactBindingTranscriptContextV1 {
            profile_digest: context.profile_digest(),
            roster_digest: context.roster_digest(),
            key_material_digest: context.key_material_digest(),
            epoch: context.epoch(),
            protocol_transcript_digest: context.transcript_digest(),
            round_tag: PersistentDirectRelationV1::RkgRoundOne as u8,
            party_index: axes.party_index,
            party: axes.party.to_bytes(),
            record_index: axes.record_index,
            relation_index: u32::from(context.digit_index()),
            statement_digest: statement_core_digest,
            commitment_set_digest,
            membership_proof_set_digest,
            persistent_graph_digest: lineage_digest_v1(self)?,
        };
        transcript.validate()?;
        Ok(transcript)
    }
}

fn lineage_digest_v1(
    permit: &PreparedDirectRkgOneCreatorPermitV1<'_>,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let context = permit.context();
    let axes = permit.prover_axes_v1();
    let mut hash = Keccak256::new();
    hash.update(RELATION_LINEAGE_DOMAIN_V1);
    hash.update(&[
        DIRECT_RELATION_CODEC_VERSION_V1,
        PersistentDirectRelationV1::RkgRoundOne as u8,
    ]);
    hash.update(&context.secret_lineage_root());
    hash.update(&axes.secret_identity);
    hash.update(&axes.secret_commitments);
    hash.update(&[1]);
    hash.update(&axes.ephemeral_identity);
    hash.update(&axes.ephemeral_commitments);
    hash.update(&axes.source_context);
    hash.update(&axes.source_statement);
    hash.update(&axes.record_index.to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(digest)
}
