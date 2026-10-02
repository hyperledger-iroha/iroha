//! Key-distinct compact wrapper for the complete ordinary cash Terminal family.
//!
//! The inner83-cell ordinary relation is recursively verified and its whole carried history
//! is folded. Both outer parities bind the same complete inner audit/protocol tuple before
//! consuming the opposite curve's equations. No hardware public-input type participates.
use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaPastaParityV1,
    base_packing::finalize_base_params_v1,
    composite::{assigned_digest_bytes, ep_succinct_vk, eq_succinct_vk},
    deferred_parent::{
        KagemushaDeferredParentOutputV1, bind_accumulator_limbs,
        constrain_reciprocal_output_with_u128_binding_serialized_v1, deferred_field_chips_v1,
        deferred_loader_v1, finalize_tagged_deferred_audit_with_u128_binding_v1,
        load_native_accumulator, native_parent_protocol_digest_v1, verify_fold,
        verify_ordinary_proof_v1,
    },
    ordinary_cash_terminal_verifier::{
        ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1, OrdinaryCashTerminalPublicV1,
    },
};
use crate::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128};
use halo2_base::{
    AssignedValue,
    gates::{
        GateInstructions as _, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::ipa::commitment::ParamsIPA,
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};
const AUDIT_EQ: usize = 41;
const AUDIT_EP: usize = 43;
const PROTOCOL_EQ: usize = 45;
const PROTOCOL_EP: usize = 47;
const HISTORY: usize = 49;
const INNER_TUPLE_CELLS: usize = 8;
const UNUSABLE_ROWS: usize = 9;
const ORDINARY_WRAPPER_EQUATION_TAG: u32 = 0x4f43_5701;

/// Complete original inner proof and history for one actual parity.
pub(super) struct OrdinaryCashCommitWrapperHalfWitnessV1<'a, C: CurveAffineExt> {
    pub(super) protocol: &'a PlonkProtocol<C>,
    pub(super) instances: &'a [Vec<C::ScalarExt>],
    pub(super) proof: &'a [u8],
    pub(super) history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) history_fold_proof: &'a [u8],
    pub(super) successor_history: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}
/// Paired mathematical proof inputs. Shipping construction is private to the Native producer.
pub(super) struct OrdinaryCashCommitWrapperWitnessV1<'a> {
    pub(super) public: OrdinaryCashTerminalPublicV1,
    pub(super) eq: OrdinaryCashCommitWrapperHalfWitnessV1<'a, EqAffine>,
    pub(super) ep: OrdinaryCashCommitWrapperHalfWitnessV1<'a, EpAffine>,
}
/// Full compact audits retained only after both discovery graphs are dropped.
pub(super) struct OrdinaryCashCommitWrapperAuditsV1 {
    eq: KagemushaDeferredParentOutputV1<EqAffine>,
    ep: KagemushaDeferredParentOutputV1<EpAffine>,
    pub(super) eq_digest: DigestV1,
    pub(super) ep_digest: DigestV1,
}

fn inner_protocol_pair(
    witness: &OrdinaryCashCommitWrapperWitnessV1<'_>,
) -> Result<[DigestV1; 2], String> {
    witness.public.validate()?;
    let eq = native_parent_protocol_digest_v1(witness.eq.protocol, KagemushaPastaParityV1::Eq)?;
    let ep = native_parent_protocol_digest_v1(witness.ep.protocol, KagemushaPastaParityV1::Ep)?;
    if eq == ep || eq == [0; 32] || ep == [0; 32] {
        return Err("ordinary Wrapper inner parity protocols are absent or aliased".into());
    }
    Ok([eq, ep])
}
/// Assign the explicit ordinary prefix and complete34-limb output history.
fn assign_public<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    public: &OrdinaryCashTerminalPublicV1,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<(Vec<AssignedValue<F>>, Vec<AssignedValue<F>>), String> {
    let values = public.public_prefix::<F>()?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let cells = values
        .into_iter()
        .map(|value| {
            let cell = ctx.load_witness(value);
            range.range_check(ctx, cell, 128);
            cell
        })
        .collect::<Vec<_>>();
    let history_cells = history
        .chunks_exact(16)
        .map(|chunk| {
            let value = from_u128::<F>(u128::from_le_bytes(
                chunk.try_into().expect("whole history limb"),
            ));
            let cell = ctx.load_witness(value);
            range.range_check(ctx, cell, 128);
            cell
        })
        .collect::<Vec<_>>();
    if cells.len() != HISTORY || history_cells.len() != 34 {
        return Err("ordinary Wrapper public/history width differs".into());
    }
    // This first-release ordinary relation has distinct fixed-column role material.
    let role = ctx.load_witness(F::from(0x4f43_5702_u64));
    range
        .gate()
        .assert_is_const(ctx, &role, &F::from(0x4f43_5702_u64));
    builder.assigned_instances = vec![cells.iter().chain(&history_cells).copied().collect()];
    Ok((cells, history_cells))
}

fn build_scalar<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    public: &OrdinaryCashTerminalPublicV1,
    protocol_pair: [DigestV1; 2],
    witness: &OrdinaryCashCommitWrapperHalfWitnessV1<'_, C>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        KagemushaDeferredParentOutputV1<C>,
        Vec<AssignedValue<C::ScalarExt>>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    public.validate()?;
    if witness.protocol.num_instance != [ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]
        || witness.instances.len() != 1
        || witness.instances[0].len() != ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1
    {
        return Err("ordinary Wrapper requires the exact inner83-cell protocol".into());
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(super::KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
        .use_lookup_bits((super::KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize)
        .use_instance_columns(1);
    let (public_cells, history_cells) =
        assign_public(&mut builder, public, witness.successor_history)?;
    let range = builder.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let instances = witness
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .copied()
                .map(|value| loader.assign_scalar(value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let column = &instances[0];
    // Exact original semantics through the manifest slot. Outer audit/protocol slots belong
    // to this distinct Wrapper relation and therefore are bound separately below.
    for (actual, expected) in column[..AUDIT_EQ].iter().zip(&public_cells[..AUDIT_EQ]) {
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&actual.assigned(), expected);
    }
    for (offset, digest) in [
        (PROTOCOL_EQ, protocol_pair[0]),
        (PROTOCOL_EP, protocol_pair[1]),
    ] {
        for (actual, expected) in column[offset..offset + 2]
            .iter()
            .zip(digest_limbs::<C::ScalarExt>(digest))
        {
            let expected = loader.ctx_mut().main().load_constant(expected);
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&actual.assigned(), &expected);
        }
    }
    // All8inner audit/protocol limbs enter the scalar audit and opposite parity's same-value
    // equality constraints. Binding only the currently verified parity leaves substitution.
    let inner_tuple = column[AUDIT_EQ..HISTORY]
        .iter()
        .map(|value| {
            let value = *value.assigned();
            range.range_check(loader.ctx_mut().main(), value, 128);
            value
        })
        .collect::<Vec<_>>();
    if inner_tuple.len() != INNER_TUPLE_CELLS {
        return Err("ordinary Wrapper inner audit/protocol tuple differs".into());
    }
    let start = loader.ecc_chip().equation_count();
    let current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &witness.protocol.loaded(&loader),
        &instances,
        witness.proof,
    )
    .map_err(|e| format!("ordinary Wrapper full inner Terminal proof: {e:?}"))?;
    let prior = load_native_accumulator(&loader, witness.history)
        .map_err(|e| format!("ordinary Wrapper inner carried history: {e:?}"))?;
    let prior_limbs = column[HISTORY..]
        .iter()
        .map(|v| *v.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &prior, &prior_limbs)
        .map_err(|e| format!("ordinary Wrapper exact inner history: {e:?}"))?;
    let complete = verify_fold(
        &loader,
        succinct_vk,
        &[current, prior],
        witness.history_fold_proof,
    )
    .map_err(|e| format!("ordinary Wrapper complete inner history fold: {e:?}"))?;
    bind_accumulator_limbs(&loader, &complete, &history_cells)
        .map_err(|e| format!("ordinary Wrapper exposed successor history: {e:?}"))?;
    let end = loader.ecc_chip().equation_count();
    if end <= start {
        return Err("ordinary Wrapper verifier/history fold emitted no equations".into());
    }
    let output = finalize_tagged_deferred_audit_with_u128_binding_v1(
        &mut builder,
        loader,
        ORDINARY_WRAPPER_EQUATION_TAG,
        &inner_tuple,
    )
    .map_err(|e| format!("ordinary Wrapper complete scalar audit: {e:?}"))?;
    let offset = match parity {
        KagemushaPastaParityV1::Eq => AUDIT_EQ,
        KagemushaPastaParityV1::Ep => AUDIT_EP,
    };
    for (actual, expected) in output
        .audit_digest_limbs
        .iter()
        .zip(&public_cells[offset..offset + 2])
    {
        builder.main(0).constrain_equal(actual, expected);
    }
    Ok((builder, output, inner_tuple))
}

/// Discover complete paired audits sequentially, dropping each large graph immediately.
pub(super) fn collect_ordinary_cash_commit_wrapper_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: &OrdinaryCashCommitWrapperWitnessV1<'_>,
) -> Result<OrdinaryCashCommitWrapperAuditsV1, String> {
    let pair = inner_protocol_pair(witness)?;
    let (eq_builder, eq, _) = build_scalar(
        &eq_succinct_vk(eq_params),
        KagemushaPastaParityV1::Eq,
        &witness.public,
        pair,
        &witness.eq,
    )?;
    let eq_digest = assigned_digest_bytes(&eq.audit_digest_limbs)?;
    drop(eq_builder);
    let (ep_builder, ep, _) = build_scalar(
        &ep_succinct_vk(ep_params),
        KagemushaPastaParityV1::Ep,
        &witness.public,
        pair,
        &witness.ep,
    )?;
    let ep_digest = assigned_digest_bytes(&ep.audit_digest_limbs)?;
    drop(ep_builder);
    Ok(OrdinaryCashCommitWrapperAuditsV1 {
        eq,
        ep,
        eq_digest,
        ep_digest,
    })
}

/// Ordinary Wrapper's Base-only compact transport configuration.
#[derive(Clone, Debug)]
pub(super) struct OrdinaryCashCommitWrapperConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
}
/// Actual Eq ordinary Wrapper circuit, independent of OEM transport types.
#[derive(Clone)]
pub(super) struct OrdinaryCashCommitWrapperEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
}
/// Actual Ep ordinary Wrapper circuit, independent of OEM transport types.
#[derive(Clone)]
pub(super) struct OrdinaryCashCommitWrapperEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
}
macro_rules! impl_wrapper {
    ($circuit:ty, $field:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = OrdinaryCashCommitWrapperConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;
            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }
            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                }
            }
            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable = (1_usize << params.k) - UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable);
                OrdinaryCashCommitWrapperConfigV1 { base }
            }
            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }
            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
            }
            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| $label),
                )
            }
        }
    };
}
impl_wrapper!(
    OrdinaryCashCommitWrapperEqCircuitV1,
    Fp,
    "ordinary Eq cash Wrapper"
);
impl_wrapper!(
    OrdinaryCashCommitWrapperEpCircuitV1,
    Fq,
    "ordinary Ep cash Wrapper"
);

/// Rebuild one Eq half and constrain every opposite-curve audit equation through Base MSM.
pub(super) fn build_ordinary_cash_commit_wrapper_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &OrdinaryCashCommitWrapperWitnessV1<'_>,
    audits: &OrdinaryCashCommitWrapperAuditsV1,
) -> Result<(OrdinaryCashCommitWrapperEqCircuitV1, Vec<Fp>), String> {
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err("ordinary Wrapper public audit pair differs from full discovery".into());
    }
    let pair = inner_protocol_pair(witness)?;
    let values = witness
        .public
        .public_column::<Fp>(witness.eq.successor_history)?;
    let (mut builder, output, local) = build_scalar(
        &eq_succinct_vk(eq_params),
        KagemushaPastaParityV1::Eq,
        &witness.public,
        pair,
        &witness.eq,
    )?;
    let expected = [
        builder.assigned_instances[0][AUDIT_EP],
        builder.assigned_instances[0][AUDIT_EP + 1],
    ];
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EpAffine>(
        &mut builder,
        &audits.ep,
        &expected,
        &local,
    )?;
    finalize_base_params_v1(&mut builder, UNUSABLE_ROWS)?;
    if assigned_digest_bytes(&output.audit_digest_limbs)? != audits.eq_digest {
        return Err("ordinary Eq Wrapper audit changed after rebinding".into());
    }
    Ok((OrdinaryCashCommitWrapperEqCircuitV1 { builder }, values))
}
/// Rebuild one Ep half and constrain every opposite-curve audit equation through Base MSM.
pub(super) fn build_ordinary_cash_commit_wrapper_ep_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &OrdinaryCashCommitWrapperWitnessV1<'_>,
    audits: &OrdinaryCashCommitWrapperAuditsV1,
) -> Result<(OrdinaryCashCommitWrapperEpCircuitV1, Vec<Fq>), String> {
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err("ordinary Wrapper public audit pair differs from full discovery".into());
    }
    let pair = inner_protocol_pair(witness)?;
    let values = witness
        .public
        .public_column::<Fq>(witness.ep.successor_history)?;
    let (mut builder, output, local) = build_scalar(
        &ep_succinct_vk(ep_params),
        KagemushaPastaParityV1::Ep,
        &witness.public,
        pair,
        &witness.ep,
    )?;
    let expected = [
        builder.assigned_instances[0][AUDIT_EQ],
        builder.assigned_instances[0][AUDIT_EQ + 1],
    ];
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EqAffine>(
        &mut builder,
        &audits.eq,
        &expected,
        &local,
    )?;
    finalize_base_params_v1(&mut builder, UNUSABLE_ROWS)?;
    if assigned_digest_bytes(&output.audit_digest_limbs)? != audits.ep_digest {
        return Err("ordinary Ep Wrapper audit changed after rebinding".into());
    }
    Ok((OrdinaryCashCommitWrapperEpCircuitV1 { builder }, values))
}

/// Reconstruct the unchanged complete scalar audits from the exact submitted inner originals.
/// Admission first verifies both current inner/Wrapper proofs and terminally decides all histories.
/// The actual fixed BGH19 transcripts below come from the same real Wrapper producer; decoding
/// them alone creates no grant. No claimed raw-inner SHA replaces these reciprocal audit joins.
pub(super) fn require_exact_ordinary_cash_wrapper_inner_v1(
    material: &super::ordinary_cash_terminal_verifier::OrdinaryCashTerminalMaterialV1<'_>,
    inner_public: &OrdinaryCashTerminalPublicV1,
    wrapper_public: &OrdinaryCashTerminalPublicV1,
    inner: &super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1,
    wrapper: &super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1,
    folds: [&[u8]; 2],
) -> Result<(), String> {
    if inner.relation != 1
        || wrapper.relation != 2
        || [inner.eq_protocol_digest, inner.ep_protocol_digest]
            != material.terminal_protocol_digests
        || [wrapper.eq_protocol_digest, wrapper.ep_protocol_digest]
            != material.wrapper_protocol_digests
        || folds
            .iter()
            .any(|raw| raw.len() != super::KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1)
    {
        return Err("ordinary Wrapper exact inner original/fold profile differs".into());
    }
    let eq_instances = vec![inner_public.public_column::<Fp>(&inner.eq_history)?];
    let ep_instances = vec![inner_public.public_column::<Fq>(&inner.ep_history)?];
    let eq_history = super::KagemushaEqAccumulatorV1::try_from_bytes(&inner.eq_history)
        .and_then(|h| h.to_native())
        .map_err(|e| e.to_string())?;
    let ep_history = super::KagemushaEpAccumulatorV1::try_from_bytes(&inner.ep_history)
        .and_then(|h| h.to_native())
        .map_err(|e| e.to_string())?;
    let witness = OrdinaryCashCommitWrapperWitnessV1 {
        public: wrapper_public.clone(),
        eq: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: material.terminal_eq_protocol,
            instances: &eq_instances,
            proof: &inner.eq_proof,
            history: &eq_history,
            history_fold_proof: folds[0],
            successor_history: &wrapper.eq_history,
        },
        ep: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: material.terminal_ep_protocol,
            instances: &ep_instances,
            proof: &inner.ep_proof,
            history: &ep_history,
            history_fold_proof: folds[1],
            successor_history: &wrapper.ep_history,
        },
    };
    let audits = collect_ordinary_cash_commit_wrapper_audits_v1(
        material.eq_parameters,
        material.ep_parameters,
        &witness,
    )?;
    if audits.eq_digest != wrapper.eq_deferred_audit
        || audits.ep_digest != wrapper.ep_deferred_audit
        || audits.eq.bound_u128_values != audits.ep.bound_u128_values
    {
        return Err(
            "ordinary Wrapper audits do not bind the submitted exact inner proofs/history".into(),
        );
    }
    Ok(())
}
