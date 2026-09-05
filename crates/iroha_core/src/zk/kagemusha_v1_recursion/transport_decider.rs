//! Compact paired-Pasta transport decider for Kagemusha V1.
//!
//! Aggregate-state recursion remains a private local carrier: constant-size
//! state and accumulators recursively authenticate the complete unbounded
//! transition history, while only the current carrier proof is stored locally.
//! Its ordinary Halo2 transcript is too wide for a payment envelope. The
//! circuits in this module verify exactly one private inner-state proof and
//! expose the same monetary statement under a compact, release-authenticated
//! outer key pair. Each parity records the
//! inner verifier's curve equation symbolically; the reciprocal parity binds
//! and evaluates that equation before either proof can carry authority.
//!
//! The inner protocol identities and inner deferred audits are intentionally
//! private.  They are absorbed into the outer deferred-audit Poseidon
//! transcript, and the reciprocal circuit equality-constrains the absorbed
//! tuple to its own inner proof.  This prevents independently valid Eq and Ep
//! inner proofs from being mixed while preserving the public hardware
//! credential-audit semantics and the existing wire fields.

#[cfg(test)]
use ff::PrimeField as _;
#[cfg(test)]
use halo2_base::utils::fe_to_biguint;
use halo2_base::{
    AssignedValue,
    gates::{
        RangeInstructions as _,
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

use super::{
    KAGEMUSHA_RECURSION_IPA_K_V1,
    composite::{assigned_digest_bytes, ep_succinct_vk, eq_succinct_vk},
    deferred_parent::{
        DeferredLoader, KagemushaDeferredParentOutputV1, bind_accumulator_limbs,
        constrain_reciprocal_output_with_u128_binding_serialized_v1, deferred_field_chips_v1,
        deferred_loader_v1, finalize_tagged_deferred_audit_with_u128_binding_v1,
        load_native_accumulator, verify_fold, verify_ordinary_proof_v1,
    },
    state_relation::{PUBLIC_INSTANCE_COUNT, public_instance},
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const TRANSPORT_DECIDER_EQUATION_TAG_V1: u32 = 6;

/// Public instance count of one compact outer parity.
///
/// The decider deliberately preserves the aggregate-state ABI: 85 semantic
/// cells followed by the 34-limb terminal history produced by folding the
/// private carrier's current opening claim into its complete prior history.
pub(super) const KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1: usize = 119;

/// Private inner cells which differ from the public outer interpretation.
///
/// The state protocol identities name the private carrier, while the outer
/// positions name the transported decider.  Likewise, the inner deferred
/// audits authenticate the carrier's recursive verifier and the outer
/// positions authenticate this decider.  Both tuples are bound recursively.
const INNER_BINDING_INDICES_V1: [usize; 8] = [
    public_instance::EQ_PROTOCOL_LO,
    public_instance::EQ_PROTOCOL_HI,
    public_instance::EP_PROTOCOL_LO,
    public_instance::EP_PROTOCOL_HI,
    public_instance::EQ_DEFERRED_AUDIT_LO,
    public_instance::EQ_DEFERRED_AUDIT_HI,
    public_instance::EP_DEFERRED_AUDIT_LO,
    public_instance::EP_DEFERRED_AUDIT_HI,
];

/// One parity's private wide carrier and public compact statement.
#[derive(Clone, Copy)]
pub(super) struct KagemushaTransportDeciderParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    /// Release-pinned private-carrier protocol compiled from its authenticated VK.
    pub(super) inner_protocol: &'a PlonkProtocol<C>,
    /// Exact private-carrier public instance column.
    pub(super) inner_instances: &'a [C::ScalarExt],
    /// Exact private-carrier proof bytes.
    pub(super) inner_proof: &'a [u8],
    /// Prior history authenticated by the private carrier's public tail.
    pub(super) inner_history: &'a IpaAccumulator<C, NativeLoader>,
    /// Exact two-input IPA-AS proof folding the current carrier claim into its prior history.
    pub(super) inner_history_fold_proof: &'a [u8],
    /// Exact public compact-decider instance column.
    ///
    /// Its history tail is the fold successor, not the private carrier's prior history.
    pub(super) outer_instances: &'a [C::ScalarExt],
}

/// Complete mutually audited outer-decider witness.
#[derive(Clone, Copy)]
pub(super) struct KagemushaTransportDeciderWitnessV1<'a> {
    /// Eq/Fp private carrier and public statement.
    pub(super) eq: KagemushaTransportDeciderParityWitnessV1<'a, EqAffine>,
    /// Ep/Fq private carrier and public statement.
    pub(super) ep: KagemushaTransportDeciderParityWitnessV1<'a, EpAffine>,
}

/// Base configuration of one compact parity.
#[derive(Clone, Debug)]
pub(super) struct KagemushaTransportDeciderConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
}

/// Eq/Fp half of the transported compact state proof.
#[derive(Clone)]
pub(super) struct KagemushaTransportDeciderEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
}

/// Ep/Fq half of the transported compact state proof.
#[derive(Clone)]
pub(super) struct KagemushaTransportDeciderEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
}

/// Measured row/cell inventory for one compact transport-decider parity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KagemushaTransportDeciderCapacityProfileV1 {
    pub(super) k: usize,
    pub(super) domain_rows: usize,
    pub(super) usable_rows: usize,
    pub(super) gate_advice_cells: usize,
    pub(super) gate_advice_columns: usize,
    pub(super) gate_packed_rows: usize,
    pub(super) lookup_advice_cells: usize,
    pub(super) lookup_advice_columns: usize,
    pub(super) lookup_packed_rows: usize,
    pub(super) dense_jobs: usize,
    pub(super) dense_sources: usize,
    pub(super) dense_rows: usize,
    pub(super) max_component_rows: usize,
}

impl KagemushaTransportDeciderEqCircuitV1 {
    pub(super) fn capacity_profile(
        &self,
    ) -> Result<KagemushaTransportDeciderCapacityProfileV1, String> {
        transport_capacity_profile_v1(&self.builder)
    }
}

impl KagemushaTransportDeciderEpCircuitV1 {
    pub(super) fn capacity_profile(
        &self,
    ) -> Result<KagemushaTransportDeciderCapacityProfileV1, String> {
        transport_capacity_profile_v1(&self.builder)
    }
}

fn transport_capacity_profile_v1<F>(
    builder: &BaseCircuitBuilder<F>,
) -> Result<KagemushaTransportDeciderCapacityProfileV1, String>
where
    F: halo2_base::utils::ScalarField + BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    let stats = builder.statistics();
    let gate_advice_cells = stats.gate.total_advice_per_phase.iter().sum();
    let gate_advice_columns = builder.config_params.num_advice_per_phase.iter().sum();
    let gate_packed_rows = packed_rows_v1(
        &stats.gate.total_advice_per_phase,
        &builder.config_params.num_advice_per_phase,
        "gate",
    )?;
    let lookup_advice_cells = stats.total_lookup_advice_per_phase.iter().sum();
    let lookup_advice_columns = builder
        .config_params
        .num_lookup_advice_per_phase
        .iter()
        .sum();
    let lookup_packed_rows = packed_rows_v1(
        &stats.total_lookup_advice_per_phase,
        &builder.config_params.num_lookup_advice_per_phase,
        "lookup",
    )?;
    let (dense_jobs, dense_sources, dense_rows) = (0, 0, 0);
    let k = builder.config_params.k;
    let domain_rows = 1_usize
        .checked_shl(u32::try_from(k).map_err(|_| "transport k exceeds u32".to_owned())?)
        .ok_or_else(|| "transport domain row count overflow".to_owned())?;
    let usable_rows = domain_rows
        .checked_sub(MINIMUM_UNUSABLE_ROWS)
        .ok_or_else(|| "transport unusable-row reserve exceeds domain".to_owned())?;
    let max_component_rows = gate_packed_rows.max(lookup_packed_rows).max(dense_rows);
    if max_component_rows > usable_rows {
        return Err(format!(
            "transport decider requires {max_component_rows} rows, exceeding {usable_rows}"
        ));
    }
    Ok(KagemushaTransportDeciderCapacityProfileV1 {
        k,
        domain_rows,
        usable_rows,
        gate_advice_cells,
        gate_advice_columns,
        gate_packed_rows,
        lookup_advice_cells,
        lookup_advice_columns,
        lookup_packed_rows,
        dense_jobs,
        dense_sources,
        dense_rows,
        max_component_rows,
    })
}

fn packed_rows_v1(cells: &[usize], columns: &[usize], label: &str) -> Result<usize, String> {
    if cells.len() != columns.len() {
        return Err(format!("transport {label} phase inventory mismatch"));
    }
    cells.iter().copied().zip(columns.iter().copied()).try_fold(
        0_usize,
        |maximum, (cell_count, column_count)| {
            if cell_count == 0 {
                return Ok(maximum);
            }
            if column_count == 0 {
                return Err(format!(
                    "transport {label} has cells without a physical column"
                ));
            }
            Ok(maximum.max(cell_count.div_ceil(column_count)))
        },
    )
}

macro_rules! impl_transport_decider_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaTransportDeciderConfigV1<$field>;
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
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                KagemushaTransportDeciderConfigV1 { base }
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
                    layouter.namespace(|| concat!($label, " Base")),
                )
            }
        }
    };
}

impl_transport_decider_circuit!(
    KagemushaTransportDeciderEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq transport decider"
);
impl_transport_decider_circuit!(
    KagemushaTransportDeciderEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep transport decider"
);

struct TransportScalarHalfV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    builder: BaseCircuitBuilder<C::ScalarExt>,
    output: KagemushaDeferredParentOutputV1<C>,
    inner_binding_cells: Vec<AssignedValue<C::ScalarExt>>,
}

/// Build both compact parities and return the exact circuit-derived outer
/// deferred-audit digests.
pub(super) fn build_kagemusha_transport_decider_pair_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaTransportDeciderWitnessV1<'_>,
) -> Result<
    (
        KagemushaTransportDeciderEqCircuitV1,
        KagemushaTransportDeciderEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let eq_svk = eq_succinct_vk(eq_params);
    let ep_svk = ep_succinct_vk(ep_params);
    let TransportScalarHalfV1 {
        builder: mut eq_builder,
        output: eq_output,
        inner_binding_cells: eq_inner_binding_cells,
    } = build_transport_scalar_half_v1(&eq_svk, witness.eq)?;
    let TransportScalarHalfV1 {
        builder: mut ep_builder,
        output: ep_output,
        inner_binding_cells: ep_inner_binding_cells,
    } = build_transport_scalar_half_v1(&ep_svk, witness.ep)?;

    bind_own_audit_v1(
        &mut eq_builder,
        public_instance::EQ_DEFERRED_AUDIT_LO,
        &eq_output,
    )?;
    bind_own_audit_v1(
        &mut ep_builder,
        public_instance::EP_DEFERRED_AUDIT_LO,
        &ep_output,
    )?;

    let eq_expected_ep_audit = public_digest_cells_v1(
        &eq_builder,
        public_instance::EP_DEFERRED_AUDIT_LO,
        "Eq decider Ep audit",
    )?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EpAffine>(
        &mut eq_builder,
        &ep_output,
        &eq_expected_ep_audit,
        &eq_inner_binding_cells,
    )?;

    let ep_expected_eq_audit = public_digest_cells_v1(
        &ep_builder,
        public_instance::EQ_DEFERRED_AUDIT_LO,
        "Ep decider Eq audit",
    )?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EqAffine>(
        &mut ep_builder,
        &eq_output,
        &ep_expected_eq_audit,
        &ep_inner_binding_cells,
    )?;

    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let eq_audit = assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    let ep_audit = assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    Ok((
        KagemushaTransportDeciderEqCircuitV1 {
            builder: eq_builder,
        },
        KagemushaTransportDeciderEpCircuitV1 {
            builder: ep_builder,
        },
        eq_audit,
        ep_audit,
    ))
}

fn build_transport_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    witness: KagemushaTransportDeciderParityWitnessV1<'_, C>,
) -> Result<TransportScalarHalfV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if witness.inner_protocol.num_instance != [KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1]
        || witness.inner_instances.len() != KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1
        || witness.outer_instances.len() != KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1
        || PUBLIC_INSTANCE_COUNT + 34 != KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("Kagemusha transport decider public instance ABI mismatch".to_owned());
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(
            usize::try_from(KAGEMUSHA_RECURSION_IPA_K_V1)
                .expect("Kagemusha recursion k fits usize"),
        )
        .use_lookup_bits(
            usize::try_from(KAGEMUSHA_RECURSION_IPA_K_V1 - 1)
                .expect("Kagemusha lookup bits fit usize"),
        )
        .use_instance_columns(1);
    let range = builder.range_chip();
    let outer_cells = witness
        .outer_instances
        .iter()
        .copied()
        .map(|value| builder.main(0).load_witness(value))
        .collect::<Vec<_>>();
    let inner_cells = witness
        .inner_instances
        .iter()
        .copied()
        .map(|value| builder.main(0).load_witness(value))
        .collect::<Vec<_>>();
    builder.assigned_instances = vec![outer_cells.clone()];

    for index in 0..KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1 {
        if index < PUBLIC_INSTANCE_COUNT && !INNER_BINDING_INDICES_V1.contains(&index) {
            builder
                .main(0)
                .constrain_equal(&inner_cells[index], &outer_cells[index]);
        }
    }
    let inner_binding_cells = INNER_BINDING_INDICES_V1
        .into_iter()
        .map(|index| {
            range.range_check(builder.main(0), inner_cells[index], 128);
            inner_cells[index]
        })
        .collect::<Vec<_>>();
    for index in INNER_BINDING_INDICES_V1 {
        range.range_check(builder.main(0), outer_cells[index], 128);
    }

    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader: DeferredLoader<'_, C> =
        deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let loaded_protocol = witness.inner_protocol.loaded(&loader);
    let loaded_instances = vec![
        inner_cells
            .iter()
            .copied()
            .map(|cell| loader.scalar_from_assigned(cell))
            .collect::<Vec<_>>(),
    ];
    let current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &loaded_protocol,
        &loaded_instances,
        witness.inner_proof,
    )
    .map_err(|error| format!("failed to verify private carrier proof: {error:?}"))?;
    let inner_history = load_native_accumulator(&loader, witness.inner_history)
        .map_err(|error| format!("failed to load private carrier history: {error:?}"))?;
    bind_accumulator_limbs(
        &loader,
        &inner_history,
        inner_cells
            .get(PUBLIC_INSTANCE_COUNT..)
            .ok_or_else(|| "private carrier history tail is absent".to_owned())?,
    )
    .map_err(|error| format!("failed to bind private carrier history: {error:?}"))?;
    let transported_history = verify_fold(
        &loader,
        succinct_vk,
        &[current, inner_history],
        witness.inner_history_fold_proof,
    )
    .map_err(|error| format!("failed to fold current carrier into history: {error:?}"))?;
    bind_accumulator_limbs(
        &loader,
        &transported_history,
        outer_cells
            .get(PUBLIC_INSTANCE_COUNT..)
            .ok_or_else(|| "transported history tail is absent".to_owned())?,
    )
    .map_err(|error| format!("failed to bind transported carrier history: {error:?}"))?;
    let output = finalize_tagged_deferred_audit_with_u128_binding_v1(
        &mut builder,
        loader,
        TRANSPORT_DECIDER_EQUATION_TAG_V1,
        &inner_binding_cells,
    )
    .map_err(|error| format!("failed to finalize transport-decider audit: {error:?}"))?;
    Ok(TransportScalarHalfV1 {
        builder,
        output,
        inner_binding_cells,
    })
}

fn bind_own_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    offset: usize,
    output: &KagemushaDeferredParentOutputV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let expected = public_digest_cells_v1(builder, offset, "decider own audit")?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok(())
}

fn public_digest_cells_v1<F: halo2_base::utils::ScalarField>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
    label: &str,
) -> Result<[AssignedValue<F>; 2], String> {
    builder
        .assigned_instances
        .first()
        .and_then(|public| public.get(offset..offset + 2))
        .ok_or_else(|| format!("Kagemusha {label} public digest is absent"))?
        .try_into()
        .map_err(|_| format!("Kagemusha {label} public digest has wrong shape"))
}

/// Convert a range-checked binding cell into its exact host `u128` value.
///
/// This helper is used only to construct the reciprocal witness.  Monetary
/// authority comes from the reciprocal circuit's range and equality
/// constraints, not from this extraction.
#[cfg(test)]
fn assigned_u128_v1<F: halo2_base::utils::ScalarField>(
    value: AssignedValue<F>,
) -> Result<u128, String> {
    let integer = fe_to_biguint(value.value());
    if integer.bits() > 128 {
        return Err("Kagemusha decider binding value exceeds u128".to_owned());
    }
    let digits = integer.to_u64_digits();
    Ok(u128::from(digits.first().copied().unwrap_or(0))
        | (u128::from(digits.get(1).copied().unwrap_or(0)) << 64))
}

const _: () = {
    assert!(PUBLIC_INSTANCE_COUNT == 85);
    assert!(KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1 == 119);
    assert!(INNER_BINDING_INDICES_V1.len() == 8);
};

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field as _;

    #[test]
    fn private_inner_binding_indices_are_unique_and_nonsemantic() {
        let mut indices = INNER_BINDING_INDICES_V1.to_vec();
        indices.sort_unstable();
        indices.dedup();
        assert_eq!(indices.len(), INNER_BINDING_INDICES_V1.len());
        assert!(indices.iter().all(|index| *index < PUBLIC_INSTANCE_COUNT));
        assert_eq!(indices[0], public_instance::EQ_PROTOCOL_LO);
        assert_eq!(indices[4], public_instance::EQ_DEFERRED_AUDIT_LO);
    }

    #[test]
    fn only_protocol_audits_and_folded_history_may_differ_from_inner_carrier() {
        let differing = (0..KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1)
            .filter(|index| {
                *index >= PUBLIC_INSTANCE_COUNT || INNER_BINDING_INDICES_V1.contains(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(differing.len(), INNER_BINDING_INDICES_V1.len() + 34);
        assert_eq!(
            &differing[..INNER_BINDING_INDICES_V1.len()],
            &INNER_BINDING_INDICES_V1
        );
        assert_eq!(
            differing[INNER_BINDING_INDICES_V1.len()],
            PUBLIC_INSTANCE_COUNT
        );
        assert_eq!(
            *differing.last().expect("nonempty differing-index set"),
            KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1 - 1
        );
    }

    #[test]
    fn assigned_u128_rejects_noncanonical_large_value() {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(16)
            .use_lookup_bits(15);
        let small = builder.main(0).load_witness(Fp::from_u128(u128::MAX));
        assert_eq!(assigned_u128_v1(small).unwrap(), u128::MAX);
        let large = builder.main(0).load_witness(-Fp::ONE);
        assert!(assigned_u128_v1(large).is_err());
    }
}
