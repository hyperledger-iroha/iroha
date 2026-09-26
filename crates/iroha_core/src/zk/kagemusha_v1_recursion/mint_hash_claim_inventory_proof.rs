//! Release-pinned scalar verification of one complete Claim inventory proof.
//!
//! This consumes the actual three-column hybrid proof, including both proof-read carrier
//! commitments. The returned graph remains deferred: it is neither a Claim closure nor a
//! monetary admission token. Its IPA opening and every emitted curve equation still require
//! reciprocal discharge, alongside the opposite inventory and the slice/join relations.

use std::ops::Range;

use super::*;
use crate::zk::kagemusha_v1_recursion::deferred_parent::DeferredEcPoint;

/// Independently authenticated verifier inputs for one inventory parity.
///
/// These pins must come from the containing circuit's authenticated configuration. In
/// particular, neither digest may be derived from the submitted proof, its semantic column,
/// or its submitted protocol. The protocol identity binds the actual VK preprocessing and
/// transcript state; its fixed structure also binds the exact instance committing key.
pub(super) struct KagemushaClaimInventoryVerifierPinsV1<'key, C>
where
    C: CurveAffineExt,
{
    pub(super) parity: KagemushaPastaParityV1,
    pub(super) succinct_vk: &'key IpaSuccinctVerifyingKey<C>,
    pub(super) structure_digest: DigestV1,
    pub(super) protocol_digest: DigestV1,
}

/// An exact proof-read inventory statement and its still-unresolved curve graph.
///
/// The original semantic cells and carrier points are retained for the reciprocal root.
/// `equations` indexes the containing loader's graph, including protocol identity and
/// semantic commitment reconstruction. Dropping this result or deciding only its IPA opening
/// cannot substantiate the inventory's curve equations or its cross-parity carrier equality.
#[must_use]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "inventory root is not yet selected by the release"
    )
)]
pub(super) struct KagemushaClaimInventoryDeferredProofV1<'chip, C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    pub(super) semantic: Vec<DeferredScalar<'chip, C>>,
    pub(super) carrier_commitments: [DeferredEcPoint<'chip, C>; 2],
    pub(super) accumulator: DeferredAccumulator<'chip, C>,
    pub(super) transcript_binding: AssignedValue<C::ScalarExt>,
    pub(super) equations: Range<usize>,
}

/// Verify exactly `[113, 4090, 4090]` against independent protocol/VK pins.
///
/// The caller supplies the original semantic cells of its containing circuit. The two wide
/// instance columns are represented only by their actual proof-read commitments: Eq binds
/// slots 97..100 and Ep binds slots 101..104. No host digest substitutes for either point.
/// The protocol identity always enters the mandatory native Poseidon queue. The ordinary
/// transcript uses the existing Base implementation; a future combined root may select its
/// native schedule from the independently fixed protocol after a whole-root capacity check.
///
/// TODO: Join both verified inventories, discharge the complete reciprocal equation graph and
/// the returned IPA openings, and authenticate every slice/join before selecting a release.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "inventory root is not yet selected by the release"
    )
)]
pub(super) fn verify_kagemusha_claim_inventory_proof_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    pins: &KagemushaClaimInventoryVerifierPinsV1<'_, C>,
    submitted_protocol: &PlonkProtocol<C>,
    semantic: &[DeferredScalar<'chip, C>],
    proof_bytes: &[u8],
    native_poseidon_jobs: &mut PastaNativePoseidonJobsV1<C::ScalarExt>,
) -> Result<KagemushaClaimInventoryDeferredProofV1<'chip, C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if semantic.len() != INVENTORY_SEMANTIC_COUNT
        || (pins.parity == KagemushaPastaParityV1::Eq) != C::ScalarExt::IS_EQ_PARITY
        || submitted_protocol.num_instance
            != [
                INVENTORY_SEMANTIC_COUNT,
                KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
                KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            ]
        || pins.succinct_vk.domain.k != KAGEMUSHA_RECURSION_IPA_K_V1 as usize
        || submitted_protocol.domain.k != KAGEMUSHA_RECURSION_IPA_K_V1 as usize
        || pins.structure_digest == [0; 32]
        || pins.protocol_digest == [0; 32]
    {
        return Err("Claim inventory proof differs from its exact pinned shape".to_owned());
    }
    let equation_start = loader.ecc_chip().equation_count();
    let expected_identity = {
        let mut ctx = loader.ctx_mut();
        std::array::from_fn(|half| {
            ctx.main()
                .load_constant(C::ScalarExt::from_u128(u128::from_le_bytes(
                    pins.protocol_digest[half * 16..(half + 1) * 16]
                        .try_into()
                        .expect("protocol identity has two exact u128 limbs"),
                )))
        })
    };
    let loaded = load_and_constrain_claim_protocol_native_v1(
        loader,
        submitted_protocol,
        pins.parity,
        pins.structure_digest,
        &expected_identity,
        native_poseidon_jobs,
    )
    .map_err(|error| format!("Claim inventory protocol pin rejected: {error:?}"))?;
    let offsets = match pins.parity {
        KagemushaPastaParityV1::Eq => [
            public_instance::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO,
            public_instance::EQ_PROOF_EP_CARRIER_COMMITMENT_LO,
        ],
        KagemushaPastaParityV1::Ep => [
            public_instance::EP_PROOF_EQ_CARRIER_COMMITMENT_LO,
            public_instance::EP_PROOF_EP_CARRIER_COMMITMENT_LO,
        ],
    };
    let proof = verify_two_carrier_hybrid_ordinary_proof_with_native_v1(
        loader,
        pins.succinct_vk,
        &loaded.protocol,
        semantic,
        offsets.map(|offset| [offset, offset + 1]),
        proof_bytes,
        None,
        native_poseidon_jobs,
    )
    .map_err(|error| format!("Claim inventory hybrid proof rejected: {error:?}"))?;
    let equations = equation_start..loader.ecc_chip().equation_count();
    if equations.is_empty() {
        return Err("Claim inventory verifier emitted no deferred equations".to_owned());
    }
    Ok(KagemushaClaimInventoryDeferredProofV1 {
        semantic: semantic.to_vec(),
        carrier_commitments: proof.carrier_commitments,
        accumulator: proof.accumulator,
        transcript_binding: proof.transcript_binding,
        equations,
    })
}
