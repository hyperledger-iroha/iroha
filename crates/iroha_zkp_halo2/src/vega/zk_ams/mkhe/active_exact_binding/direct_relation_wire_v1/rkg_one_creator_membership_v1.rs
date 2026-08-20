//! Sequential six-slot membership generation for a direct RKG1 candidate.

use super::super::PreparedDirectRkgOneCreatorPermitV1;
use super::statement_v1::PreparedDirectRkgOneStatementCoreV1;
use super::{
    DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1, DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1,
    DIRECT_RELATION_CODEC_VERSION_V1, MEMBERSHIP_BYTES_V1, ORDERED_COMMITMENT_ROOT_DOMAIN_V1,
    ORDERED_MEMBERSHIP_ROOT_DOMAIN_V1, PersistentDirectRelationV1,
    membership_share_statement_digest,
};
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{
        sponge::Keccak256,
        zk_ams::mkhe::{
            ZkAmsMkheErrorV1,
            exact_eight_chunk_membership::{
                DirectRelationBoundOneMembershipRoleV1, DirectRelationBoundTwoMembershipRoleV1,
                ExactEightChunkMembershipEvidenceV1,
            },
        },
    },
};

pub(super) struct DirectRkgOneMembershipOutputV1 {
    pub(super) commitment_set_digest: [u8; 32],
    pub(super) membership_proof_set_digest: [u8; 32],
}

pub(super) fn generate_direct_rkg_one_memberships_v1<R: ProofRandomSource>(
    permit: &PreparedDirectRkgOneCreatorPermitV1<'_>,
    core: &PreparedDirectRkgOneStatementCoreV1,
    random: &mut R,
    output: &mut Vec<u8>,
) -> Result<DirectRkgOneMembershipOutputV1, ZkAmsMkheErrorV1> {
    let start = output.len();
    output
        .try_reserve_exact(MEMBERSHIP_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let relation = PersistentDirectRelationV1::RkgRoundOne;
    let mut commitment_root = Keccak256::new();
    commitment_root.update(ORDERED_COMMITMENT_ROOT_DOMAIN_V1);
    commitment_root.update(&[DIRECT_RELATION_CODEC_VERSION_V1, relation as u8, 6]);
    commitment_root.update(&core.core_digest());
    let mut membership_root = Keccak256::new();
    membership_root.update(ORDERED_MEMBERSHIP_ROOT_DOMAIN_V1);
    membership_root.update(&[DIRECT_RELATION_CODEC_VERSION_V1, relation as u8, 6]);
    membership_root.update(&core.core_digest());
    for slot in 0..6 {
        let share_statement_digest =
            membership_share_statement_digest(relation, core.core_digest(), slot);
        if slot < 2 {
            let context = permit.membership_context_v1::<DirectRelationBoundOneMembershipRoleV1>(
                share_statement_digest,
            )?;
            let evidence = permit.prove_bound_one_v1(slot, context, random)?;
            absorb_and_encode_v1(
                slot,
                &evidence,
                DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1,
                &mut commitment_root,
                &mut membership_root,
                output,
            )?;
        } else {
            let context = permit.membership_context_v1::<DirectRelationBoundTwoMembershipRoleV1>(
                share_statement_digest,
            )?;
            let evidence = permit.prove_bound_two_v1(slot, context, random)?;
            absorb_and_encode_v1(
                slot,
                &evidence,
                DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1,
                &mut commitment_root,
                &mut membership_root,
                output,
            )?;
        }
    }
    if output.len().checked_sub(start) != Some(MEMBERSHIP_BYTES_V1) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let commitment_set_digest = commitment_root.finalize();
    let membership_proof_set_digest = membership_root.finalize();
    if commitment_set_digest == [0; 32] || membership_proof_set_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(DirectRkgOneMembershipOutputV1 {
        commitment_set_digest,
        membership_proof_set_digest,
    })
}

fn absorb_and_encode_v1<R>(
    slot: usize,
    evidence: &ExactEightChunkMembershipEvidenceV1<R>,
    expected_wire_bytes: usize,
    commitment_root: &mut Keccak256,
    membership_root: &mut Keccak256,
    output: &mut Vec<u8>,
) -> Result<(), ZkAmsMkheErrorV1>
where
    R: crate::vega::zk_ams::mkhe::exact_eight_chunk_membership::ExactEightChunkMembershipRoleV1,
{
    commitment_root.update(&[slot as u8]);
    commitment_root.update(&evidence.commitment_set_digest());
    membership_root.update(&[slot as u8]);
    membership_root.update(&evidence.proof_set_digest());
    membership_root.update(&evidence.verifier_transcript_digest());
    let wire = evidence
        .to_wire_bytes()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if wire.len() != expected_wire_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    output.extend_from_slice(&wire);
    Ok(())
}
