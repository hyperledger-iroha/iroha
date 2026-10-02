//! Physical ordinary cash Terminal circuits with complete reciprocal Pasta audits.
//!
//! Both constructors use the identical full scalar relation and typed SHA queue. Discovery
//! drops one graph before allocating the next. Neither planning nor circuit construction
//! installs a Native owner or treats carried proof originals as monetary authority.
use super::{
    DigestV1, KagemushaPastaParityV1,
    base_packing::finalize_base_params_v1,
    composite::{
        assigned_digest_bytes, ep_succinct_vk, eq_succinct_vk,
        ordinary_cash_terminal_math::{
            OrdinaryCashTerminalHalfWitnessV1, OrdinaryCashTerminalSemanticWitnessV1,
            assign_ordinary_cash_terminal_semantics_v1, build_ordinary_cash_terminal_scalar_v1,
        },
    },
    deferred_parent::{
        KagemushaDeferredParentOutputV1, constrain_reciprocal_output_with_u128_binding_v1,
    },
    mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1,
    ordinary_cash_terminal_verifier::OrdinaryCashTerminalPublicV1,
};
use crate::pasta_dense_msm::{
    PastaDenseMsmConfigV1, PastaDenseMsmJobsV1, preflight_k16_dense_single_job_source_count_v1,
};
use halo2_base::{
    gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::ipa::commitment::ParamsIPA,
};
const UNUSABLE_ROWS: usize = 9;
const AUDIT_EQ: usize = 41;
const AUDIT_EP: usize = 43;
/// Pair of actual candidate/Guard/fold/SHA originals and the same Native-selected semantics.
pub(super) struct OrdinaryCashTerminalCircuitWitnessV1<'a> {
    pub(super) public: OrdinaryCashTerminalPublicV1,
    pub(super) semantic: &'a OrdinaryCashTerminalSemanticWitnessV1<'a>,
    pub(super) eq: OrdinaryCashTerminalHalfWitnessV1<'a, EqAffine>,
    pub(super) ep: OrdinaryCashTerminalHalfWitnessV1<'a, EpAffine>,
}
/// Complete compact audit data retained after the two discovery graphs are dropped.
pub(super) struct OrdinaryCashTerminalAuditsV1 {
    eq: KagemushaDeferredParentOutputV1<EqAffine>,
    ep: KagemushaDeferredParentOutputV1<EpAffine>,
    pub(super) eq_digest: DigestV1,
    pub(super) ep_digest: DigestV1,
}
/// Plan the exact complete ordered queue. These messages cannot create a proof/owner grant.
/// Any later State/protocol/original change requires running this same planner again.
pub(super) fn plan_ordinary_cash_terminal_sha_v1(
    public: &OrdinaryCashTerminalPublicV1,
    source: &OrdinaryCashTerminalSemanticWitnessV1<'_>,
    eq_candidate_instances: &[Vec<Fp>],
    ep_candidate_instances: &[Vec<Fq>],
) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>), String> {
    let empty_history = [0; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
    let eq = assign_ordinary_cash_terminal_semantics_v1(
        KagemushaPastaParityV1::Eq,
        public,
        source,
        eq_candidate_instances,
        &empty_history,
    )?;
    let eq_messages = eq.jobs.bounded_claim_messages()?;
    drop(eq);
    let ep = assign_ordinary_cash_terminal_semantics_v1(
        KagemushaPastaParityV1::Ep,
        public,
        source,
        ep_candidate_instances,
        &empty_history,
    )?;
    let ep_messages = ep.jobs.bounded_claim_messages()?;
    drop(ep);
    if eq_messages.is_empty() || eq_messages.len() != ep_messages.len() {
        return Err("ordinary Terminal paired ordered SHA queue differs".into());
    }
    // These are exact active messages, exported only for the maintained claim planner.
    // The consuming scalar relation separately binds every fixed-capacity padded job.
    if eq_messages
        .iter()
        .zip(&ep_messages)
        .any(|(a, b)| a.len() != b.len())
    {
        return Err("ordinary Terminal paired fixed SHA capacities differ".into());
    }
    Ok((eq_messages, ep_messages))
}

/// Discover both genuine scalar audits, retaining no full Base graph across parity builds.
pub(super) fn collect_ordinary_cash_terminal_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: &OrdinaryCashTerminalCircuitWitnessV1<'_>,
) -> Result<OrdinaryCashTerminalAuditsV1, String> {
    let (eq_builder, eq, _) = build_ordinary_cash_terminal_scalar_v1(
        &eq_succinct_vk(eq_params),
        KagemushaPastaParityV1::Eq,
        &witness.public,
        witness.semantic,
        witness.eq.reborrow(),
    )?;
    let eq_digest = assigned_digest_bytes(&eq.audit_digest_limbs)?;
    drop(eq_builder);
    let (ep_builder, ep, _) = build_ordinary_cash_terminal_scalar_v1(
        &ep_succinct_vk(ep_params),
        KagemushaPastaParityV1::Ep,
        &witness.public,
        witness.semantic,
        witness.ep.reborrow(),
    )?;
    let ep_digest = assigned_digest_bytes(&ep.audit_digest_limbs)?;
    drop(ep_builder);
    if eq.bound_u128_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1
        || eq.bound_u128_values != ep.bound_u128_values
    {
        return Err("ordinary Terminal paired complete SHA carrier tuples differ".into());
    }
    Ok(OrdinaryCashTerminalAuditsV1 {
        eq,
        ep,
        eq_digest,
        ep_digest,
    })
}
fn preflight_reciprocal_sources<C>(audit: &KagemushaDeferredParentOutputV1<C>) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let mut used = std::collections::BTreeSet::new();
    for equation in &audit.audit.equations {
        for (index, _) in equation {
            if *index >= audit.audit.sources.len() {
                return Err("ordinary Terminal reciprocal source index invalid".into());
            }
            used.insert(*index);
        }
    }
    preflight_k16_dense_single_job_source_count_v1(used.len())
        .map_err(|e| format!("ordinary Terminal reciprocal geometry: {e}"))
}
/// Genuine ordinary Terminal's Base and opposite-curve dense MSM configuration.
#[derive(Clone, Debug)]
pub(super) struct OrdinaryCashTerminalConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    dense: PastaDenseMsmConfigV1,
}
/// Actual Eq ordinary Terminal circuit.
#[derive(Clone)]
pub(super) struct OrdinaryCashTerminalEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}
/// Actual Ep ordinary Terminal circuit.
#[derive(Clone)]
pub(super) struct OrdinaryCashTerminalEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}
macro_rules! impl_terminal {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = OrdinaryCashTerminalConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;
            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }
            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                    dense_jobs: self.dense_jobs.unknown(),
                }
            }
            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable = (1_usize << params.k) - UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable);
                OrdinaryCashTerminalConfigV1 {
                    base,
                    dense: PastaDenseMsmConfigV1::configure::<$opposite>(meta),
                }
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
                let usable = (1_usize << self.builder.config_params.k) - UNUSABLE_ROWS;
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| concat!($label, " Base")),
                )?;
                self.dense_jobs.synthesize(
                    &config.dense,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                    usable,
                )
            }
        }
    };
}
impl_terminal!(
    OrdinaryCashTerminalEqCircuitV1,
    Fp,
    EpAffine,
    "ordinary Eq cash Terminal"
);
impl_terminal!(
    OrdinaryCashTerminalEpCircuitV1,
    Fq,
    EqAffine,
    "ordinary Ep cash Terminal"
);

/// Rebuild Eq with the exact opposite audit and complete14-limb original SHA carrier tail.
pub(super) fn build_ordinary_cash_terminal_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &OrdinaryCashTerminalCircuitWitnessV1<'_>,
    audits: &OrdinaryCashTerminalAuditsV1,
) -> Result<(OrdinaryCashTerminalEqCircuitV1, Vec<Fp>), String> {
    preflight_reciprocal_sources(&audits.ep)?;
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err("ordinary Terminal public audits differ from actual discovery".into());
    }
    let values = witness
        .public
        .public_column::<Fp>(witness.eq.successor_history)?;
    let (mut builder, output, claim_tail) = build_ordinary_cash_terminal_scalar_v1(
        &eq_succinct_vk(eq_params),
        KagemushaPastaParityV1::Eq,
        &witness.public,
        witness.semantic,
        witness.eq.reborrow(),
    )?;
    let expected = [
        builder.assigned_instances[0][AUDIT_EP],
        builder.assigned_instances[0][AUDIT_EP + 1],
    ];
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_output_with_u128_binding_v1::<EpAffine>(
        &mut builder,
        &audits.ep,
        &expected,
        &claim_tail,
        &mut dense_jobs,
    )?;
    finalize_base_params_v1(&mut builder, UNUSABLE_ROWS)?;
    dense_jobs.validate_capacity((1_usize << builder.config_params.k) - UNUSABLE_ROWS)?;
    if assigned_digest_bytes(&output.audit_digest_limbs)? != audits.eq_digest {
        return Err("ordinary Eq Terminal scalar audit changed after rebinding".into());
    }
    Ok((
        OrdinaryCashTerminalEqCircuitV1 {
            builder,
            dense_jobs,
        },
        values,
    ))
}
/// Rebuild Ep with the exact opposite audit and complete14-limb original SHA carrier tail.
pub(super) fn build_ordinary_cash_terminal_ep_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &OrdinaryCashTerminalCircuitWitnessV1<'_>,
    audits: &OrdinaryCashTerminalAuditsV1,
) -> Result<(OrdinaryCashTerminalEpCircuitV1, Vec<Fq>), String> {
    preflight_reciprocal_sources(&audits.eq)?;
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err("ordinary Terminal public audits differ from actual discovery".into());
    }
    let values = witness
        .public
        .public_column::<Fq>(witness.ep.successor_history)?;
    let (mut builder, output, claim_tail) = build_ordinary_cash_terminal_scalar_v1(
        &ep_succinct_vk(ep_params),
        KagemushaPastaParityV1::Ep,
        &witness.public,
        witness.semantic,
        witness.ep.reborrow(),
    )?;
    let expected = [
        builder.assigned_instances[0][AUDIT_EQ],
        builder.assigned_instances[0][AUDIT_EQ + 1],
    ];
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_output_with_u128_binding_v1::<EqAffine>(
        &mut builder,
        &audits.eq,
        &expected,
        &claim_tail,
        &mut dense_jobs,
    )?;
    finalize_base_params_v1(&mut builder, UNUSABLE_ROWS)?;
    dense_jobs.validate_capacity((1_usize << builder.config_params.k) - UNUSABLE_ROWS)?;
    if assigned_digest_bytes(&output.audit_digest_limbs)? != audits.ep_digest {
        return Err("ordinary Ep Terminal scalar audit changed after rebinding".into());
    }
    Ok((
        OrdinaryCashTerminalEpCircuitV1 {
            builder,
            dense_jobs,
        },
        values,
    ))
}
