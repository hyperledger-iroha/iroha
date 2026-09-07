//! Shared consumer of complete authenticated typed SHA claims.
//!
//! The caller supplies its original assigned SHA queue and release-pinned helper protocols.
//! Acceptance verifies the hybrid proof, binds the entire queue, and folds both the claim's
//! carried history and the caller's predecessor. The returned carrier tail must also be bound
//! into both scalar and reciprocal deferred audits; native validation alone grants no authority.

use halo2_base::{
    AssignedValue,
    utils::{BigPrimeField, CurveAffineExt},
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

use super::{
    DigestV1, KagemushaPastaParityV1,
    deferred_parent::{
        DeferredLoader, bind_accumulator_limbs, kagemusha_protocol_structure_digest_v1,
        load_and_constrain_parent_protocol_v1, load_native_accumulator,
        native_parent_protocol_digest_v1, verify_fold,
        verify_two_carrier_hybrid_ordinary_proof_and_stream_v1,
    },
    mint_hash_claim_fold::{
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1,
        canonical_claim_carrier_binding_tail_v1, constrain_complete_claim_against_sha_jobs_v1,
        public_instance as hash_claim_public,
    },
};
use crate::zk::{kagemusha_v1_poseidon::KagemushaPoseidonFieldV1, pasta_sha256::PastaSha256JobsV1};

/// Exact terminal ordered claim for the SHA jobs emitted by one consumer parity.
pub(super) struct KagemushaRecursiveHashClaimParityWitnessV1<'a, C: CurveAffineExt> {
    /// Release-authenticated Eq/Ep claim and Eq/Ep shard protocol identities, in that order.
    pub(super) protocol_digests: [DigestV1; 4],
    /// Native claim protocol whose exact identity is pinned by the consumer key.
    pub(super) protocol: &'a PlonkProtocol<C>,
    /// Exact semantic and two-carrier instance columns of the hybrid claim.
    pub(super) instances: &'a [Vec<C::ScalarExt>],
    /// Ordinary hybrid proof bytes for the pinned claim protocol.
    pub(super) proof: &'a [u8],
    /// Claim history authenticated by its semantic public column.
    pub(super) history: &'a IpaAccumulator<C, NativeLoader>,
    /// Fold proof combining the current claim accumulator and its carried history.
    pub(super) history_fold_proof: &'a [u8],
    /// Fold proof combining the caller predecessor and the complete claim history.
    pub(super) merge_fold_proof: &'a [u8],
}

/// Validate paired claim identities, exact public shape, histories, and carrier binding.
///
/// This is an early native diagnostic; callers must still constrain the proof and its audits.
pub(super) fn validate_recursive_hash_claim_v1(
    claim: &super::generation::KagemushaMintHashClaimGenerationWitnessV1<'_>,
) -> Result<(), String> {
    let digests = [
        claim.eq_claim_protocol_digest,
        claim.ep_claim_protocol_digest,
        claim.eq_shard_protocol_digest,
        claim.ep_shard_protocol_digest,
    ];
    if digests.iter().any(|digest| *digest == [0; 32])
        || digests[0] == digests[1]
        || digests[2] == digests[3]
    {
        return Err(
            "recursive state hash suite has absent or parity-aliased identities".to_owned(),
        );
    }
    if native_parent_protocol_digest_v1(claim.eq_protocol, KagemushaPastaParityV1::Eq)?
        != digests[0]
        || native_parent_protocol_digest_v1(claim.ep_protocol, KagemushaPastaParityV1::Ep)?
            != digests[1]
    {
        return Err(
            "recursive state hash claim differs from its authenticated protocol".to_owned(),
        );
    }
    validate_recursive_hash_claim_history_v1(claim.eq_instances, claim.eq_history.as_bytes())?;
    validate_recursive_hash_claim_history_v1(claim.ep_instances, claim.ep_history.as_bytes())?;
    if canonical_claim_carrier_binding_tail_v1(claim.eq_instances)?
        != canonical_claim_carrier_binding_tail_v1(claim.ep_instances)?
    {
        return Err(
            "recursive state paired hash claims have different carrier bindings".to_owned(),
        );
    }
    Ok(())
}

fn validate_recursive_hash_claim_history_v1<F: KagemushaPoseidonFieldV1>(
    instances: &[Vec<F>],
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<(), String> {
    let shape = [
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
    ];
    if !instances.iter().map(Vec::len).eq(shape) {
        return Err(
            "recursive state hash claim requires the exact two-carrier hybrid shape".to_owned(),
        );
    }
    let expected = history
        .chunks_exact(16)
        .map(|bytes| {
            F::from_u128(u128::from_le_bytes(
                bytes.try_into().expect("history limb width"),
            ))
        })
        .collect::<Vec<_>>();
    if instances[0]
        .get(hash_claim_public::HISTORY_START..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1)
        != Some(expected.as_slice())
    {
        return Err(
            "recursive state hash-claim history is detached from its public column".to_owned(),
        );
    }
    Ok(())
}

/// Verify the exact ordered SHA queue and fold its complete authenticated history.
///
/// The returned carrier binding must be absorbed in both deferred audits and equality-bound in
/// the reciprocal passes. The hybrid proof alone does not authenticate its opposite-field
/// deferred equations, and a host comparison of the binding is only an early diagnostic.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_recursive_hash_claim_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    claim: KagemushaRecursiveHashClaimParityWitnessV1<'_, C>,
    jobs: &PastaSha256JobsV1<C::ScalarExt>,
    release: [AssignedValue<C::ScalarExt>; 2],
    predecessor: super::deferred_parent::DeferredAccumulator<'chip, C>,
) -> Result<
    (
        super::deferred_parent::DeferredAccumulator<'chip, C>,
        Vec<AssignedValue<C::ScalarExt>>,
        usize,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    // These four identities are fixed-column values owned by the release-authenticated consumer
    // key. They are not prover-selected public claims or unconstrained digest witnesses.
    let protocols: [[AssignedValue<C::ScalarExt>; 2]; 4] = claim.protocol_digests.map(|digest| {
        crate::zk::kagemusha_v1_poseidon::digest_limbs::<C::ScalarExt>(digest)
            .map(|value| loader.ctx_mut().main().load_constant(value))
    });
    let structure = kagemusha_protocol_structure_digest_v1(claim.protocol, parity)?;
    let loaded = load_and_constrain_parent_protocol_v1(
        loader,
        claim.protocol,
        parity,
        structure,
        &protocols[match parity {
            KagemushaPastaParityV1::Eq => 0,
            KagemushaPastaParityV1::Ep => 1,
        }],
    )
    .map_err(|error| format!("recursive state hash-claim protocol binding failed: {error:?}"))?;
    let shape = [
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
    ];
    if loaded.protocol.num_instance != shape || !claim.instances.iter().map(Vec::len).eq(shape) {
        return Err("recursive state terminal hash-claim protocol shape changed".to_owned());
    }
    let semantic = claim.instances[0]
        .iter()
        .map(|value| loader.assign_scalar(*value))
        .collect::<Vec<_>>();
    let equation_start = loader.ecc_chip().equation_count();
    let current = verify_two_carrier_hybrid_ordinary_proof_and_stream_v1(
        loader,
        succinct_vk,
        &loaded.protocol,
        &semantic,
        match parity {
            KagemushaPastaParityV1::Eq => [
                [
                    hash_claim_public::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO + 1,
                ],
                [
                    hash_claim_public::EQ_PROOF_EP_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EQ_PROOF_EP_CARRIER_COMMITMENT_LO + 1,
                ],
            ],
            KagemushaPastaParityV1::Ep => [
                [
                    hash_claim_public::EP_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EP_PROOF_EQ_CARRIER_COMMITMENT_LO + 1,
                ],
                [
                    hash_claim_public::EP_PROOF_EP_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EP_PROOF_EP_CARRIER_COMMITMENT_LO + 1,
                ],
            ],
        },
        claim.proof,
    )
    .map_err(|error| format!("recursive state hash-claim verifier failed: {error:?}"))?;
    let current_end = loader.ecc_chip().equation_count();
    if current_end <= equation_start {
        return Err("recursive state hash-claim current verifier emitted no equation".to_owned());
    }
    let column = &semantic[..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1];
    let history = load_native_accumulator(loader, claim.history)
        .map_err(|error| format!("recursive state hash history load failed: {error:?}"))?;
    let history_cells = column[hash_claim_public::HISTORY_START..]
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(loader, &history, &history_cells)
        .map_err(|error| format!("recursive state hash history binding failed: {error:?}"))?;
    let binding = semantic[KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
        ..hash_claim_public::CARRIER_BINDING_END]
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    if binding.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1 {
        return Err("recursive state hash carrier-binding width changed".to_owned());
    }
    {
        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let assigned = column
            .iter()
            .map(|value| *value.assigned())
            .collect::<Vec<_>>();
        constrain_complete_claim_against_sha_jobs_v1(
            ctx.main(),
            chip.range(),
            jobs,
            &assigned,
            parity,
            release,
            protocols[0],
            protocols[1],
            protocols[2],
            protocols[3],
        )?;
    }
    let complete = verify_fold(
        loader,
        succinct_vk,
        &[current.accumulator, history],
        claim.history_fold_proof,
    )
    .map_err(|error| format!("recursive state hash-claim history fold failed: {error:?}"))?;
    let complete_end = loader.ecc_chip().equation_count();
    if complete_end <= current_end {
        return Err("recursive state hash-claim history fold emitted no equation".to_owned());
    }
    let successor = verify_fold(
        loader,
        succinct_vk,
        &[predecessor, complete],
        claim.merge_fold_proof,
    )
    .map_err(|error| format!("recursive state hash-claim merge fold failed: {error:?}"))?;
    if loader.ecc_chip().equation_count() <= complete_end {
        return Err("recursive state hash-claim merge fold emitted no equation".to_owned());
    }
    Ok((successor, binding, current_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::halo2curves::pasta::{Fp, Fq};

    #[test]
    fn recursive_hash_claim_history_rejects_truncation_extra_columns_and_detached_history() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            let history = [0x42; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
            let mut instances = vec![
                vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1],
                vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
                vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
            ];
            for (cell, bytes) in instances[0][hash_claim_public::HISTORY_START
                ..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1]
                .iter_mut()
                .zip(history.chunks_exact(16))
            {
                *cell = F::from_u128(u128::from_le_bytes(bytes.try_into().unwrap()));
            }
            validate_recursive_hash_claim_history_v1(&instances, &history)
                .expect("exact bound history");
            let mut changed = instances.clone();
            changed[0][hash_claim_public::HISTORY_START] += F::ONE;
            assert!(validate_recursive_hash_claim_history_v1(&changed, &history).is_err());
            for index in 0..3 {
                let mut changed = instances.clone();
                changed[index].pop();
                assert!(validate_recursive_hash_claim_history_v1(&changed, &history).is_err());
            }
            instances.push(Vec::new());
            assert!(validate_recursive_hash_claim_history_v1(&instances, &history).is_err());
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn recursive_hash_claim_history_rejects_every_changed_limb_and_nonexact_column_shape() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            let history: [u8; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] =
                core::array::from_fn(|index| (index / 16 + 1) as u8);
            let mut instances = vec![
                vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1],
                vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
                vec![F::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
            ];
            for (cell, bytes) in instances[0][hash_claim_public::HISTORY_START
                ..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1]
                .iter_mut()
                .zip(history.chunks_exact(16))
            {
                *cell = F::from_u128(u128::from_le_bytes(bytes.try_into().unwrap()));
            }
            validate_recursive_hash_claim_history_v1(&instances, &history)
                .expect("every distinct history limb is bound in order");
            for index in
                hash_claim_public::HISTORY_START..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
            {
                let mut changed = instances.clone();
                changed[0][index] += F::ONE;
                assert!(
                    validate_recursive_hash_claim_history_v1(&changed, &history).is_err(),
                    "detached history limb {index} must fail"
                );
            }
            let mut changed = instances.clone();
            changed[0].swap(
                hash_claim_public::HISTORY_START,
                hash_claim_public::HISTORY_START + 1,
            );
            assert!(validate_recursive_hash_claim_history_v1(&changed, &history).is_err());
            for index in 0..3 {
                let mut changed = instances.clone();
                changed[index].push(F::ZERO);
                assert!(validate_recursive_hash_claim_history_v1(&changed, &history).is_err());
                let mut changed = instances.clone();
                changed[index].clear();
                assert!(validate_recursive_hash_claim_history_v1(&changed, &history).is_err());
                let mut changed = instances.clone();
                changed.remove(index);
                assert!(validate_recursive_hash_claim_history_v1(&changed, &history).is_err());
            }
            assert!(validate_recursive_hash_claim_history_v1::<F>(&[], &history).is_err());
        }
        check::<Fp>();
        check::<Fq>();
    }
}
