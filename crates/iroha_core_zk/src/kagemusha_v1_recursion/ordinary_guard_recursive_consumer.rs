//! Exact five-original ordinary Guard proof and whole-history consumer.
//!
//! This helper is shared by the two terminal phases: retained purpose2 preparation and
//! purpose1 monetary approval. Both calls use genuine ordinary roles50–53 and the complete
//! current proof plus history. It supplies scalar equations only; every equation still needs
//! the opposite parity audit, final typed SHA claim, all merges and Native financial custody.

use super::{
    KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaPastaParityV1,
    deferred_parent::{
        DeferredAccumulator, DeferredLoader, accumulator_limb_count, bind_accumulator_limbs,
        load_native_accumulator, native_parent_protocol_digest_v1, verify_fold,
        verify_ordinary_proof_v1,
    },
    guard_bundle::digest_limbs_assigned,
    ordinary_guard_circuit::ORDINARY_GUARD_PUBLIC_INSTANCE_COUNT_V1,
    ordinary_guard_data_binding::KagemushaOrdinaryGuardDataBindingV1,
};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128},
    pasta_sha256::PastaSha256ByteV1,
};
use halo2_base::{
    AssignedValue, Context,
    gates::{RangeChip, RangeInstructions as _},
    utils::{BigPrimeField, CurveAffineExt},
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

/// Semantic commitments in their actual ordinary role order, never OEM audit offsets.
pub(super) struct KagemushaOrdinaryGuardCommitmentCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) normalized_statement: [PastaSha256ByteV1<F>; 32],
    pub(super) original_credential: [PastaSha256ByteV1<F>; 32],
    pub(super) original_authorization: [PastaSha256ByteV1<F>; 32],
    pub(super) complete_subject: [PastaSha256ByteV1<F>; 32],
    pub(super) provider_policy_root: [PastaSha256ByteV1<F>; 32],
}
impl<F: KagemushaPoseidonFieldV1> KagemushaOrdinaryGuardCommitmentCellsV1<F> {
    /// Retain the exact SHA outputs created from the same assigned C, W, PI and full S.
    pub(super) fn from_binding(binding: &KagemushaOrdinaryGuardDataBindingV1<F>) -> Self {
        Self {
            normalized_statement: binding.digests[0],
            original_credential: binding.digests[1],
            original_authorization: binding.digests[2],
            complete_subject: binding.digests[3],
            provider_policy_root: binding.digests[4],
        }
    }
}
/// Whole original proof and its exact carried history, never just an accepting verifier handle.
pub(super) struct KagemushaOrdinaryGuardCompleteProofV1<'a, C: CurveAffineExt> {
    pub(super) protocol: &'a PlonkProtocol<C>,
    pub(super) proof: &'a [u8],
    pub(super) history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) history_bytes: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    pub(super) history_fold_proof: &'a [u8],
}

/// Assign precisely ten digest limbs and the complete 34-limb original history.
/// Every byte belongs to the actual retained ordinary original, not a hardware audit claim.
pub(super) fn assigned_ordinary_guard_column_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    commitments: KagemushaOrdinaryGuardCommitmentCellsV1<F>,
    history: &[AssignedValue<F>],
) -> Result<Vec<AssignedValue<F>>, String> {
    if history.len() != accumulator_limb_count() {
        return Err("ordinary complete Guard history has wrong shape".into());
    }
    let mut column = Vec::with_capacity(ORDINARY_GUARD_PUBLIC_INSTANCE_COUNT_V1);
    for digest in [
        commitments.normalized_statement,
        commitments.original_credential,
        commitments.original_authorization,
        commitments.complete_subject,
        commitments.provider_policy_root,
    ] {
        // The originals are actual queued SHA outputs and assigned provider root bytes.
        // The enclosing mandatory typed SHA claim authenticates every output below.
        column.extend(digest_limbs_assigned(ctx, &digest));
    }
    column.extend_from_slice(history);
    if column.len() != ORDINARY_GUARD_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("ordinary complete Guard column has wrong shape".into());
    }
    Ok(column)
}

/// Consume one genuine ordinary Guard and its whole history under the exact candidate role.
///
/// `candidate_protocol_digest` must be the original cells of the verified candidate State,
/// never assigned afresh from this witness's protocol. The installed State suite authenticates
/// those roles. The returned fold must join every other consumed history and SHA claim, while
/// the opposite-parity point audit must consume the entire returned equation interval.
pub(super) fn constrain_ordinary_guard_complete_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    range: &RangeChip<C::ScalarExt>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    candidate_protocol_digest: [AssignedValue<C::ScalarExt>; 2],
    commitments: KagemushaOrdinaryGuardCommitmentCellsV1<C::ScalarExt>,
    witness: KagemushaOrdinaryGuardCompleteProofV1<'_, C>,
) -> Result<(DeferredAccumulator<'chip, C>, core::ops::Range<usize>), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if witness.protocol.num_instance != [ORDINARY_GUARD_PUBLIC_INSTANCE_COUNT_V1] {
        return Err(
            "ordinary Guard consumer requires the distinct five-original protocol shape".into(),
        );
    }
    let protocol_digest = native_parent_protocol_digest_v1(witness.protocol, parity)?;
    let expected = digest_limbs::<C::ScalarExt>(protocol_digest);
    for (actual, expected) in candidate_protocol_digest.into_iter().zip(expected) {
        let constant = loader.ctx_mut().main().load_constant(expected);
        loader.ctx_mut().main().constrain_equal(&actual, &constant);
    }
    let history_cells = {
        let mut pool = loader.ctx_mut();
        let ctx = pool.main();
        witness
            .history_bytes
            .chunks_exact(16)
            .map(|chunk| {
                let value = from_u128::<C::ScalarExt>(u128::from_le_bytes(
                    chunk.try_into().expect("whole history chunk"),
                ));
                let assigned = ctx.load_witness(value);
                range.range_check(ctx, assigned, 128);
                assigned
            })
            .collect::<Vec<_>>()
    };
    let column =
        assigned_ordinary_guard_column_v1(loader.ctx_mut().main(), commitments, &history_cells)?;
    let instances = vec![
        column
            .into_iter()
            .map(|cell| loader.scalar_from_assigned(cell))
            .collect::<Vec<_>>(),
    ];
    let start = loader.ecc_chip().equation_count();
    let current = verify_ordinary_proof_v1(
        loader,
        succinct_vk,
        &witness.protocol.loaded(loader),
        &instances,
        witness.proof,
    )
    .map_err(|error| format!("ordinary exact-original Guard proof: {error:?}"))?;
    let history = load_native_accumulator(loader, witness.history)
        .map_err(|error| format!("ordinary Guard carried history: {error:?}"))?;
    bind_accumulator_limbs(loader, &history, &history_cells)
        .map_err(|error| format!("ordinary Guard original history binding: {error:?}"))?;
    let complete = verify_fold(
        loader,
        succinct_vk,
        &[current, history],
        witness.history_fold_proof,
    )
    .map_err(|error| format!("ordinary Guard complete history fold: {error:?}"))?;
    let end = loader.ecc_chip().equation_count();
    if end <= start {
        return Err("ordinary Guard consumer emitted no proof/history equations".into());
    }
    Ok((complete, start..end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_proofs::halo2curves::pasta::{Fp, Fq};
    fn check<F: KagemushaPoseidonFieldV1>() {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(iroha_data_model::kagemusha::KAGEMUSHA_HALO2_K_V1 as usize)
            .use_lookup_bits((iroha_data_model::kagemusha::KAGEMUSHA_HALO2_K_V1 - 1) as usize);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let digest = |ctx: &mut Context<F>, tag: u8| {
            core::array::from_fn(|_| {
                let value = ctx.load_witness(F::from(u64::from(tag)));
                PastaSha256ByteV1::range_checked(ctx, &range, value)
            })
        };
        let values = KagemushaOrdinaryGuardCommitmentCellsV1 {
            normalized_statement: digest(ctx, 1),
            original_credential: digest(ctx, 2),
            original_authorization: digest(ctx, 3),
            complete_subject: digest(ctx, 4),
            provider_policy_root: digest(ctx, 5),
        };
        let history = (0..accumulator_limb_count())
            .map(|index| ctx.load_witness(F::from(100 + index as u64)))
            .collect::<Vec<_>>();
        let column = assigned_ordinary_guard_column_v1(ctx, values, &history).unwrap();
        assert_eq!(column.len(), 44);
        for (index, tag) in (1..=5).enumerate() {
            for (actual, expected) in column[index * 2..index * 2 + 2]
                .iter()
                .zip(digest_limbs::<F>([tag; 32]))
            {
                assert_eq!(*actual.value(), expected);
            }
        }
        assert!(
            column[10..]
                .iter()
                .zip(history)
                .all(|(actual, expected)| actual.value() == expected.value())
        );
    }
    #[test]
    fn ordinary_guard_column_both_fields_retains_exact_five_original_roles_and_whole_history() {
        check::<Fp>();
        check::<Fq>();
    }
}
