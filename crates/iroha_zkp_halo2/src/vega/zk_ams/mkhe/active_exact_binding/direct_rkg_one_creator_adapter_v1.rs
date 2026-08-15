//! Owner-derived, single-use authority seam for one direct RKG1 candidate.

use super::super::{
    MKHE_VERSION_V1, ZkAmsMkheErrorV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    collective::DirectRkgOneProverSessionV1,
    direct_collective_eval_ceremony::{
        ZkAmsMkheDirectCeremonyContextV1, ZkAmsMkheDirectPolynomialStreamReceiptV1,
        direct_rkg_one_creator_contribution_statement_v1,
    },
    direct_object_transport::ZkAmsMkheDirectObjectReadAtProviderV1,
    exact_eight_chunk_membership::{
        DirectRelationBoundOneMembershipRoleV1, DirectRelationBoundTwoMembershipRoleV1,
        ExactEightChunkMembershipContextV1, ExactEightChunkMembershipEvidenceV1,
    },
};
use super::{
    PERSISTENT_IDENTITY_DOMAIN_V1, PersistentDirectRelationUseSelectorV1,
    PersistentWitnessConsumerV1, PersistentWitnessRoleV1, VerifiedPersistentWitnessBindingSetV1,
    VerifiedPersistentWitnessDirectRelationUseV1,
    direct_common_a_v1::{
        CompletedDirectCommonACreatorAuthorityV1, DirectCommonACreatorH0ReadyV1,
        DirectCommonACreatorH0ReplayV1, DirectCommonACreatorH1ReadyV1,
        DirectCommonACreatorH1ReplayV1, consume_completed_creator_authority_v1,
        prepare_direct_common_a_creator_h0_v1,
    },
    direct_relation_wire_v1::{
        DirectRelationPublicObjectsV1,
        verify_direct_rkg_one_semantic_candidate_v1 as verify_semantic_candidate_v1,
    },
    persistent_commitment_set_digest, persistent_direct_relation_use_digest,
};
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, sponge::Keccak256},
};
const STATEMENT_AUTHORITY_END_V1: usize = 544;
struct DirectRkgOneCreatorAuthorityV1<'a> {
    context: ZkAmsMkheDirectCeremonyContextV1,
    party_index: u8,
    party: super::super::ZkAmsMkhePartyIdV1,
    secret_identity_digest: [u8; 32],
    secret_generator_basis_digest: [u8; 32],
    secret_commitment_set_digest: [u8; 32],
    secret_commitments: [Point; 8],
    ephemeral_identity_digest: [u8; 32],
    ephemeral_commitment_set_digest: [u8; 32],
    ephemeral_source_context_digest: [u8; 32],
    ephemeral_source_statement_digest: [u8; 32],
    ephemeral_record_index: u32,
    prover_session: DirectRkgOneProverSessionV1<'a>,
}
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneCreatorH0ReadyV1<'a> {
    authority: DirectRkgOneCreatorAuthorityV1<'a>,
    common: DirectCommonACreatorH0ReadyV1,
}
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneCreatorH0ReplayV1<'a> {
    authority: DirectRkgOneCreatorAuthorityV1<'a>,
    common: DirectCommonACreatorH0ReplayV1,
}
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneCreatorH1ReadyV1<'a> {
    authority: DirectRkgOneCreatorAuthorityV1<'a>,
    common: DirectCommonACreatorH1ReadyV1,
}
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneCreatorH1ReplayV1<'a> {
    authority: DirectRkgOneCreatorAuthorityV1<'a>,
    common: DirectCommonACreatorH1ReplayV1,
}
pub(in crate::vega::zk_ams::mkhe) struct CompletedDirectRkgOneCreatorV1<'a> {
    authority: DirectRkgOneCreatorAuthorityV1<'a>,
    common: CompletedDirectCommonACreatorAuthorityV1,
}
pub(in crate::vega::zk_ams::mkhe) struct PreparedDirectRkgOneCreatorPermitV1<'a> {
    completed: CompletedDirectRkgOneCreatorV1<'a>,
    contribution_statement_digest: [u8; 32],
}
pub(in crate::vega::zk_ams::mkhe::active_exact_binding) struct DirectRkgOneProverAxesV1 {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) party_index: u8,
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) party: super::super::ZkAmsMkhePartyIdV1,
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) record_index: u32,
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) secret_identity: [u8; 32],
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) secret_commitments: [u8; 32],
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) ephemeral_identity: [u8; 32],
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) ephemeral_commitments: [u8; 32],
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) source_context: [u8; 32],
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) source_statement: [u8; 32],
}
pub(in crate::vega::zk_ams::mkhe::active_exact_binding) struct DirectRkgOneFinalizeRequestV1 {
    context_digest: [u8; 32],
    contribution_statement_digest: [u8; 32],
    proof_commitment_transcript_digest: [u8; 32],
}
pub(in crate::vega::zk_ams::mkhe) struct FinalizedDirectRkgOneCapabilityV1<'a> {
    _prover_session: DirectRkgOneProverSessionV1<'a>,
    capability: VerifiedPersistentWitnessDirectRelationUseV1,
}
pub(in crate::vega::zk_ams::mkhe) fn prepare_direct_rkg_one_creator_h0_v1<'a>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    prover_session: DirectRkgOneProverSessionV1<'a>,
) -> Result<DirectRkgOneCreatorH0ReadyV1<'a>, ZkAmsMkheErrorV1> {
    bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::RkgRoundOne)?;
    context.validate_rkg_ephemeral_membership_axes(roster, bindings)?;
    let ephemeral_context = prover_session.context();
    let party_index = ephemeral_context.party_index();
    let party_index_u8 =
        u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
    let (secret_identity_digest, generator_basis_digest, secret_commitment_set_digest, points) =
        bindings.decryption_party_material(party_index)?;
    if ephemeral_context.direct_context_digest() != context.digest()
        || context.direct_secret_lineage_digest(party_index_u8)? != secret_identity_digest
        || prover_session.persistent_commitments() != &points
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let ephemeral_commitment_set_digest = persistent_commitment_set_digest(
        generator_basis_digest,
        prover_session.ephemeral_commitments(),
    )?;
    if ephemeral_commitment_set_digest == secret_commitment_set_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let ephemeral_identity_digest = prospective_ephemeral_identity_v1(
        roster,
        bindings,
        ephemeral_context,
        generator_basis_digest,
        ephemeral_commitment_set_digest,
    )?;
    if ephemeral_identity_digest == secret_identity_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let common = prepare_direct_common_a_creator_h0_v1(roster, bindings, context)?;
    Ok(DirectRkgOneCreatorH0ReadyV1 {
        authority: DirectRkgOneCreatorAuthorityV1 {
            context,
            party_index: party_index_u8,
            party: roster.participants()[party_index].party(),
            secret_identity_digest,
            secret_generator_basis_digest: generator_basis_digest,
            secret_commitment_set_digest,
            secret_commitments: points,
            ephemeral_identity_digest,
            ephemeral_commitment_set_digest,
            ephemeral_source_context_digest: ephemeral_context.direct_context_digest(),
            ephemeral_source_statement_digest: ephemeral_context.statement_digest(),
            ephemeral_record_index: ephemeral_context.record_index(),
            prover_session,
        },
        common,
    })
}
impl<'a> DirectRkgOneCreatorH0ReadyV1<'a> {
    pub(in crate::vega::zk_ams::mkhe) const fn stream_axes_v1(
        &self,
    ) -> (ZkAmsMkheDirectCeremonyContextV1, usize) {
        (self.authority.context, self.authority.party_index as usize)
    }
    pub(in crate::vega::zk_ams::mkhe) fn begin_h0_v1(
        self,
    ) -> Result<DirectRkgOneCreatorH0ReplayV1<'a>, ZkAmsMkheErrorV1> {
        Ok(DirectRkgOneCreatorH0ReplayV1 {
            authority: self.authority,
            common: self.common.begin_h0_v1()?,
        })
    }
}
impl<'a> DirectRkgOneCreatorH0ReplayV1<'a> {
    pub(in crate::vega::zk_ams::mkhe) fn derive_next_limb_v1(
        &mut self,
        limb: usize,
        common_a: &mut [u64],
    ) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
        self.common.derive_next_limb_into(common_a)?;
        self.authority
            .prover_session
            .retain_common_a_limb_v1(limb, common_a)?;
        self.authority
            .prover_session
            .relation_limb_v1(0, limb, common_a)
    }
    pub(in crate::vega::zk_ams::mkhe) fn finish_h0_v1(
        self,
    ) -> Result<DirectRkgOneCreatorH1ReadyV1<'a>, ZkAmsMkheErrorV1> {
        Ok(DirectRkgOneCreatorH1ReadyV1 {
            authority: self.authority,
            common: self.common.finish_h0_v1()?,
        })
    }
}
impl<'a> DirectRkgOneCreatorH1ReadyV1<'a> {
    pub(in crate::vega::zk_ams::mkhe) const fn stream_axes_v1(
        &self,
    ) -> (ZkAmsMkheDirectCeremonyContextV1, usize) {
        (self.authority.context, self.authority.party_index as usize)
    }
    pub(in crate::vega::zk_ams::mkhe) fn begin_h1_v1(
        self,
    ) -> Result<DirectRkgOneCreatorH1ReplayV1<'a>, ZkAmsMkheErrorV1> {
        Ok(DirectRkgOneCreatorH1ReplayV1 {
            authority: self.authority,
            common: self.common.begin_h1_v1()?,
        })
    }
}
impl<'a> DirectRkgOneCreatorH1ReplayV1<'a> {
    pub(in crate::vega::zk_ams::mkhe) fn derive_next_limb_v1(
        &mut self,
        limb: usize,
        common_a: &mut [u64],
    ) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
        self.common.derive_next_limb_into(common_a)?;
        if self.authority.prover_session.common_a_limb_v1(limb)? != common_a {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.authority
            .prover_session
            .relation_limb_v1(1, limb, common_a)
    }
    pub(in crate::vega::zk_ams::mkhe) fn finish_h1_v1(
        self,
    ) -> Result<CompletedDirectRkgOneCreatorV1<'a>, ZkAmsMkheErrorV1> {
        Ok(CompletedDirectRkgOneCreatorV1 {
            authority: self.authority,
            common: self.common.finish_h1_v1()?,
        })
    }
}
pub(in crate::vega::zk_ams::mkhe) fn prepare_direct_rkg_one_statement_permit_v1<'a>(
    completed: CompletedDirectRkgOneCreatorV1<'a>,
    h0: &ZkAmsMkheDirectPolynomialStreamReceiptV1,
    h1: &ZkAmsMkheDirectPolynomialStreamReceiptV1,
) -> Result<PreparedDirectRkgOneCreatorPermitV1<'a>, ZkAmsMkheErrorV1> {
    let contribution_statement_digest = direct_rkg_one_creator_contribution_statement_v1(
        completed.authority.context,
        usize::from(completed.authority.party_index),
        h0,
        h1,
    )?;
    Ok(PreparedDirectRkgOneCreatorPermitV1 {
        completed,
        contribution_statement_digest,
    })
}
impl PreparedDirectRkgOneCreatorPermitV1<'_> {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) const fn context(
        &self,
    ) -> ZkAmsMkheDirectCeremonyContextV1 {
        self.completed.authority.context
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) const fn prover_axes_v1(
        &self,
    ) -> DirectRkgOneProverAxesV1 {
        let authority = &self.completed.authority;
        DirectRkgOneProverAxesV1 {
            party_index: authority.party_index,
            party: authority.party,
            record_index: authority.ephemeral_record_index,
            secret_identity: authority.secret_identity_digest,
            secret_commitments: authority.secret_commitment_set_digest,
            ephemeral_identity: authority.ephemeral_identity_digest,
            ephemeral_commitments: authority.ephemeral_commitment_set_digest,
            source_context: authority.ephemeral_source_context_digest,
            source_statement: authority.ephemeral_source_statement_digest,
        }
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn write_statement_authority_v1(
        &self,
        bytes: &mut [u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if bytes.len() < STATEMENT_AUTHORITY_END_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let authority = &self.completed.authority;
        put(bytes, 212, &authority.context.secret_lineage_root());
        put(
            bytes,
            244,
            &authority.context.collective_public_key_digest(),
        );
        bytes[276] = authority.party_index;
        bytes[277] = authority.context.evaluated_key_ordinal();
        bytes[278] = authority.context.digit_index();
        put(
            bytes,
            280,
            &authority.context.galois_exponent().to_be_bytes(),
        );
        put(bytes, 284, &authority.ephemeral_record_index.to_be_bytes());
        put(bytes, 288, &authority.party.to_bytes());
        put(bytes, 320, &authority.secret_identity_digest);
        put(bytes, 352, &authority.ephemeral_identity_digest);
        put(bytes, 384, &authority.ephemeral_source_context_digest);
        put(bytes, 416, &authority.ephemeral_source_statement_digest);
        let common_destination: &mut [u8; 32] = (&mut bytes[448..480])
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        self.completed
            .common
            .write_statement_digest_v1(authority.context, common_destination)?;
        put(bytes, 512, &self.contribution_statement_digest);
        Ok(())
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn prove_bound_one_v1<
        R: ProofRandomSource,
    >(
        &self,
        slot: usize,
        context: ExactEightChunkMembershipContextV1<DirectRelationBoundOneMembershipRoleV1>,
        random: &mut R,
    ) -> Result<
        ExactEightChunkMembershipEvidenceV1<DirectRelationBoundOneMembershipRoleV1>,
        ZkAmsMkheErrorV1,
    > {
        self.completed
            .authority
            .prover_session
            .prove_bound_one_v1(slot, context, random)
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn prove_bound_two_v1<
        R: ProofRandomSource,
    >(
        &self,
        slot: usize,
        context: ExactEightChunkMembershipContextV1<DirectRelationBoundTwoMembershipRoleV1>,
        random: &mut R,
    ) -> Result<
        ExactEightChunkMembershipEvidenceV1<DirectRelationBoundTwoMembershipRoleV1>,
        ZkAmsMkheErrorV1,
    > {
        self.completed
            .authority
            .prover_session
            .prove_bound_two_v1(slot, context, random)
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn response_coefficient_v1(
        &self,
        slot: usize,
        coefficient: usize,
        mask: i64,
        challenge: u32,
    ) -> Result<i64, ZkAmsMkheErrorV1> {
        self.completed
            .authority
            .prover_session
            .response_coefficient_v1(slot, coefficient, mask, challenge)
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn response_blinding_v1(
        &self,
        slot: usize,
        chunk: usize,
        mask_blinding: &Scalar,
        challenge: u32,
    ) -> Result<Scalar, ZkAmsMkheErrorV1> {
        self.completed
            .authority
            .prover_session
            .response_blinding_v1(slot, chunk, mask_blinding, challenge)
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn common_a_limb_v1(
        &self,
        limb: usize,
    ) -> Result<&[u64], ZkAmsMkheErrorV1> {
        self.completed
            .authority
            .prover_session
            .common_a_limb_v1(limb)
    }
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn finalize_request_v1(
        &self,
        proof_commitment_transcript_digest: [u8; 32],
    ) -> Result<DirectRkgOneFinalizeRequestV1, ZkAmsMkheErrorV1> {
        if proof_commitment_transcript_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(DirectRkgOneFinalizeRequestV1 {
            context_digest: self.context().digest(),
            contribution_statement_digest: self.contribution_statement_digest,
            proof_commitment_transcript_digest,
        })
    }
}
pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn finalize_direct_rkg_one_capability_v1<
    'a,
>(
    permit: PreparedDirectRkgOneCreatorPermitV1<'a>,
    request: DirectRkgOneFinalizeRequestV1,
) -> Result<FinalizedDirectRkgOneCapabilityV1<'a>, ZkAmsMkheErrorV1> {
    let authority = permit.completed.authority;
    if request.context_digest != authority.context.digest()
        || request.contribution_statement_digest != permit.contribution_statement_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let selector: PersistentDirectRelationUseSelectorV1 = consume_completed_creator_authority_v1(
        permit.completed.common,
        authority.context.initial_round_digest(),
        request.contribution_statement_digest,
        request.proof_commitment_transcript_digest,
    )?;
    let mut capability = VerifiedPersistentWitnessDirectRelationUseV1 {
        binding_set_root: authority.context.secret_lineage_root(),
        collective_public_key_digest: authority.context.collective_public_key_digest(),
        party_index: authority.party_index,
        party: authority.party,
        secret_identity_digest: authority.secret_identity_digest,
        secret_generator_basis_digest: authority.secret_generator_basis_digest,
        secret_commitment_set_digest: authority.secret_commitment_set_digest,
        secret_commitments: authority.secret_commitments,
        ephemeral_identity_digest: authority.ephemeral_identity_digest,
        ephemeral_commitment_set_digest: authority.ephemeral_commitment_set_digest,
        ephemeral_source_context_digest: authority.ephemeral_source_context_digest,
        ephemeral_source_statement_digest: authority.ephemeral_source_statement_digest,
        ephemeral_record_index: authority.ephemeral_record_index,
        ephemeral_commitments: Some(*authority.prover_session.ephemeral_commitments()),
        selector,
        use_digest: [0; 32],
    };
    capability.use_digest = persistent_direct_relation_use_digest(&capability)?;
    capability.validate()?;
    Ok(FinalizedDirectRkgOneCapabilityV1 {
        _prover_session: authority.prover_session,
        capability,
    })
}

impl FinalizedDirectRkgOneCapabilityV1<'_> {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn write_statement_trailer_v1(
        &self,
        destination: &mut [u8; 64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.capability.validate()?;
        destination[..32].copy_from_slice(&self.capability.use_digest);
        destination[32..]
            .copy_from_slice(&self.capability.selector.proof_commitment_transcript_digest);
        Ok(())
    }
}

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn verify_finalized_direct_rkg_one_semantic_candidate_v1<
    'a,
    P,
>(
    finalized: FinalizedDirectRkgOneCapabilityV1<'a>,
    context: ZkAmsMkheDirectCeremonyContextV1,
    objects: DirectRelationPublicObjectsV1,
    proof_bytes: &[u8],
    provider: &mut P,
) -> Result<impl Sized + use<'a, P>, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let FinalizedDirectRkgOneCapabilityV1 {
        _prover_session,
        capability,
    } = finalized;
    let semantic =
        verify_semantic_candidate_v1(context, capability, objects, proof_bytes, provider)?;
    Ok((_prover_session.into_compacted_post_seal_v1(), semantic))
}

fn prospective_ephemeral_identity_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: super::super::direct_rkg_ephemeral_membership::ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    generator_basis_digest: [u8; 32],
    commitment_set_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let party_index = context.party_index();
    if party_index >= super::super::manifest::ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || generator_basis_digest == [0; 32]
        || commitment_set_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(PERSISTENT_IDENTITY_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&roster.profile_digest());
    hash.update(&bindings.security_certificate_digest);
    hash.update(&roster.roster_digest());
    hash.update(&roster.key_material_digest());
    hash.update(&roster.epoch().to_be_bytes());
    hash.update(&bindings.cpk_transcript_digest);
    hash.update(&[u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?]);
    hash.update(&roster.participants()[party_index].party().to_bytes());
    hash.update(&bindings.cpk_share_digests[party_index]);
    hash.update(&[PersistentWitnessRoleV1::RkgEphemeral as u8]);
    hash.update(&context.record_index().to_be_bytes());
    hash.update(&context.direct_context_digest());
    hash.update(&context.statement_digest());
    hash.update(&generator_basis_digest);
    hash.update(&commitment_set_digest);
    Ok(hash.finalize())
}

fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
    bytes[offset..offset + N].copy_from_slice(value);
}
