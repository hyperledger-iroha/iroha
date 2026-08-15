//! Candidate-only RKG1 proof assembly; grants no verification authority.

use super::super::{
    FinalizedDirectRkgOneCapabilityV1, PreparedDirectRkgOneCreatorPermitV1,
    direct_rkg_one_creator_adapter_v1::{
        finalize_direct_rkg_one_capability_v1,
        verify_finalized_direct_rkg_one_semantic_candidate_v1,
    },
};
use super::{
    BLIND_RESPONSE_BYTES_V1, BODY_BYTES_V1, CHALLENGE_SEED_BYTES_V1, DirectRelationPublicObjectsV1,
    HEADER_BYTES_V1, MEMBERSHIP_BYTES_V1, PersistentDirectRelationV1, RESPONSE_BYTES_V1,
    RKG_ONE_STATEMENT_BYTES_V1, canonical_header_fields_v1,
    rkg_one_creator_membership_v1::generate_direct_rkg_one_memberships_v1,
    rkg_one_creator_response_v1::create_direct_rkg_one_responses_v1,
    statement_v1::PreparedDirectRkgOneStatementCoreV1,
};
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{
        MaskedRelaxedRandomSourceV1,
        zk_ams::mkhe::{
            ZkAmsMkheErrorV1, direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
            direct_object_transport::ZkAmsMkheDirectObjectReadAtProviderV1,
        },
    },
};
#[path = "rkg_one_creator_prover_v1/transcript_v1.rs"]
mod transcript_v1;

const STATEMENT_START_V1: usize = HEADER_BYTES_V1;
const MEMBERSHIP_START_V1: usize = STATEMENT_START_V1 + RKG_ONE_STATEMENT_BYTES_V1;
const RESPONSE_START_V1: usize = MEMBERSHIP_START_V1 + MEMBERSHIP_BYTES_V1;
const BLIND_RESPONSE_START_V1: usize = RESPONSE_START_V1 + RESPONSE_BYTES_V1;
const SEED_START_V1: usize = BLIND_RESPONSE_START_V1 + BLIND_RESPONSE_BYTES_V1;
const DIRECT_RKG_ONE_PROOF_BYTES_V1: usize = SEED_START_V1 + CHALLENGE_SEED_BYTES_V1;

const _: () = {
    assert!(MEMBERSHIP_START_V1 == 908);
    assert!(RESPONSE_START_V1 == 76_766);
    assert!(DIRECT_RKG_ONE_PROOF_BYTES_V1 == 25_248_766);
    assert!(BODY_BYTES_V1 == 25_247_858);
};

/// Sealed proof byte owner. It is neither cloneable nor encodable.
pub(in crate::vega::zk_ams::mkhe) struct SealedDirectRkgOneProofBytesV1 {
    bytes: Vec<u8>,
}

impl SealedDirectRkgOneProofBytesV1 {
    pub(in crate::vega::zk_ams::mkhe) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl core::fmt::Debug for SealedDirectRkgOneProofBytesV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SealedDirectRkgOneProofBytesV1")
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// Composite final owner of local prover-session/opening ownership, single-use capability,
/// and exact proof bytes; not provenance, extractor evidence, or verifier completion.
pub(in crate::vega::zk_ams::mkhe) struct SealedDirectRkgOneProofOwnerV1<'a> {
    _finalized_capability: FinalizedDirectRkgOneCapabilityV1<'a>,
    proof: SealedDirectRkgOneProofBytesV1,
}

struct PostSemanticDirectRkgOneProofOwnerV1<S> {
    _semantic_owner: S,
    _proof: SealedDirectRkgOneProofBytesV1,
}

impl<'a> SealedDirectRkgOneProofOwnerV1<'a> {
    pub(in crate::vega::zk_ams::mkhe) fn verify_semantic_candidate_v1<P>(
        self,
        context: ZkAmsMkheDirectCeremonyContextV1,
        objects: DirectRelationPublicObjectsV1,
        provider: &mut P,
    ) -> Result<impl Sized + use<'a, P>, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let Self {
            _finalized_capability,
            proof,
        } = self;
        let semantic_owner = verify_finalized_direct_rkg_one_semantic_candidate_v1(
            _finalized_capability,
            context,
            objects,
            proof.as_bytes(),
            provider,
        )?;
        Ok(PostSemanticDirectRkgOneProofOwnerV1 {
            _semantic_owner: semantic_owner,
            _proof: proof,
        })
    }
}

pub(in crate::vega::zk_ams::mkhe) fn seal_direct_rkg_one_proof_owner_v1<'a, R>(
    permit: PreparedDirectRkgOneCreatorPermitV1<'a>,
    statement_core: PreparedDirectRkgOneStatementCoreV1,
    random: &mut R,
) -> Result<SealedDirectRkgOneProofOwnerV1<'a>, ZkAmsMkheErrorV1>
where
    R: ProofRandomSource + MaskedRelaxedRandomSourceV1,
{
    let mut builder = WipingDirectRkgOneProofBuilderV1::new()?;
    let memberships = generate_direct_rkg_one_memberships_v1(
        &permit,
        &statement_core,
        random,
        &mut builder.bytes,
    )?;
    if builder.bytes.len() != RESPONSE_START_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let transcript_context = permit.transcript_context_v1(
        statement_core.core_digest(),
        memberships.commitment_set_digest,
        memberships.membership_proof_set_digest,
    )?;

    builder
        .bytes
        .try_reserve_exact(DIRECT_RKG_ONE_PROOF_BYTES_V1 - RESPONSE_START_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    builder.bytes.resize(DIRECT_RKG_ONE_PROOF_BYTES_V1, 0);
    let seed = {
        let response_and_tail = &mut builder.bytes[RESPONSE_START_V1..];
        let (responses, blind_and_seed) = response_and_tail.split_at_mut(RESPONSE_BYTES_V1);
        let (blind_responses, _) = blind_and_seed.split_at_mut(BLIND_RESPONSE_BYTES_V1);
        create_direct_rkg_one_responses_v1(
            &permit,
            transcript_context,
            responses,
            blind_responses,
            random,
        )?
    };
    builder.bytes[SEED_START_V1..].copy_from_slice(&seed);

    let request = permit.finalize_request_v1(seed)?;
    let finalized_capability = finalize_direct_rkg_one_capability_v1(permit, request)?;
    let statement = statement_core.finalize(&finalized_capability)?;
    if statement.core_digest() != transcript_context.statement_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    builder.bytes[STATEMENT_START_V1..MEMBERSHIP_START_V1].copy_from_slice(statement.bytes());
    let header = canonical_header_fields_v1(
        PersistentDirectRelationV1::RkgRoundOne,
        RKG_ONE_STATEMENT_BYTES_V1,
        statement.statement_digest(),
    );
    builder.bytes[..HEADER_BYTES_V1].copy_from_slice(&header);
    let proof = builder.into_sealed()?;
    Ok(SealedDirectRkgOneProofOwnerV1 {
        _finalized_capability: finalized_capability,
        proof,
    })
}

struct WipingDirectRkgOneProofBuilderV1 {
    bytes: Vec<u8>,
}

impl WipingDirectRkgOneProofBuilderV1 {
    fn new() -> Result<Self, ZkAmsMkheErrorV1> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(RESPONSE_START_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.resize(MEMBERSHIP_START_V1, 0);
        Ok(Self { bytes })
    }

    fn into_sealed(mut self) -> Result<SealedDirectRkgOneProofBytesV1, ZkAmsMkheErrorV1> {
        if self.bytes.len() != DIRECT_RKG_ONE_PROOF_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let bytes = core::mem::take(&mut self.bytes);
        Ok(SealedDirectRkgOneProofBytesV1 { bytes })
    }
}

impl Drop for WipingDirectRkgOneProofBuilderV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(self.bytes.as_mut_slice());
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
    }
}
