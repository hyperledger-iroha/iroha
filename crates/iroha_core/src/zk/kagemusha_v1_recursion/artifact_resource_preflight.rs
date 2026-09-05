//! Early processed-key resource prediction for fixed Pasta circuits.
//!
//! Halo2's ordinary `keygen_vk` path does not compress selectors. Every configured selector is
//! therefore materialized as its own fixed polynomial before proving-key construction. This
//! module predicts that exact serialized layout before key generation; the later serialization
//! checks remain authoritative and defend against backend-format drift.

use ff::{FromUniformBytes, PrimeField as _};
use halo2_proofs::{
    halo2curves::CurveAffine,
    plonk::{Circuit, ConstraintSystem, VerifyingKey, keygen_vk},
    poly::{commitment::Params as _, ipa::commitment::ParamsIPA},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1, KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
};

use super::{KagemushaArtifactGenerationErrorV1, KagemushaPastaParityV1};

const PASTA_PROCESSED_SCALAR_BYTES: u64 = 32;
const PASTA_PROCESSED_POINT_BYTES: u64 = 32;
const VERIFYING_KEY_HEADER_BYTES: u64 = 1 + 4 + 1 + 4;
const POLYNOMIAL_LENGTH_BYTES: u64 = 4;
const POLYNOMIAL_VECTOR_LENGTH_BYTES: u64 = 4;
const PROVING_KEY_MASK_POLYNOMIALS: u64 = 3;
const PROVING_KEY_POLYNOMIAL_VECTORS: u64 = 4;

/// Exact column inventory and Processed-format byte prediction for one Pasta circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaProcessedKeyResourceProfileV1 {
    /// Advice columns configured by the circuit.
    pub(super) advice_columns: u64,
    /// Instance columns configured by the circuit.
    pub(super) instance_columns: u64,
    /// Fixed columns configured before selector materialization.
    pub(super) configured_fixed_columns: u64,
    /// Selectors converted one-for-one to fixed columns by uncompressed keygen.
    pub(super) selector_columns: u64,
    /// Fixed polynomials serialized after selector materialization.
    pub(super) processed_fixed_columns: u64,
    /// Columns participating in the permutation argument.
    pub(super) permutation_columns: u64,
    /// Exact predicted Processed verifier-key bytes.
    pub(super) verifying_key_bytes: u64,
    /// Exact predicted Processed proving-key bytes.
    pub(super) proving_key_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourcePredictionErrorV1 {
    UnsupportedProcessedEncodingWidth,
    DomainExponentDoesNotFitU32,
    PolynomialDomainDoesNotFitU32,
    ColumnCountDoesNotFitU32,
    ArithmeticOverflow,
}

impl core::fmt::Display for ResourcePredictionErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedProcessedEncodingWidth => {
                "Pasta resource prediction requires 32-byte processed scalars and points"
            }
            Self::DomainExponentDoesNotFitU32 => "domain exponent does not fit u32",
            Self::PolynomialDomainDoesNotFitU32 => {
                "polynomial domain length does not fit the serialized u32 prefix"
            }
            Self::ColumnCountDoesNotFitU32 => "column count does not fit the serialized u32 prefix",
            Self::ArithmeticOverflow => "processed key byte prediction overflowed u64",
        })
    }
}

fn checked_count(value: usize) -> Result<u64, ResourcePredictionErrorV1> {
    u32::try_from(value)
        .map(u64::from)
        .map_err(|_| ResourcePredictionErrorV1::ColumnCountDoesNotFitU32)
}

fn predict_processed_key_resources_v1(
    k: usize,
    advice_columns: usize,
    instance_columns: usize,
    configured_fixed_columns: usize,
    selector_columns: usize,
    permutation_columns: usize,
) -> Result<KagemushaProcessedKeyResourceProfileV1, ResourcePredictionErrorV1> {
    let k = u32::try_from(k).map_err(|_| ResourcePredictionErrorV1::DomainExponentDoesNotFitU32)?;
    let domain_rows = 1_u64
        .checked_shl(k)
        .ok_or(ResourcePredictionErrorV1::PolynomialDomainDoesNotFitU32)?;
    if domain_rows > u64::from(u32::MAX) {
        return Err(ResourcePredictionErrorV1::PolynomialDomainDoesNotFitU32);
    }

    let advice_columns = checked_count(advice_columns)?;
    let instance_columns = checked_count(instance_columns)?;
    let configured_fixed_columns = checked_count(configured_fixed_columns)?;
    let selector_columns = checked_count(selector_columns)?;
    let permutation_columns = checked_count(permutation_columns)?;
    let processed_fixed_columns = configured_fixed_columns
        .checked_add(selector_columns)
        .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?;
    if processed_fixed_columns > u64::from(u32::MAX) {
        return Err(ResourcePredictionErrorV1::ColumnCountDoesNotFitU32);
    }

    let polynomial_bytes = domain_rows
        .checked_mul(PASTA_PROCESSED_SCALAR_BYTES)
        .and_then(|bytes| bytes.checked_add(POLYNOMIAL_LENGTH_BYTES))
        .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?;
    let verifying_key_bytes = processed_fixed_columns
        .checked_add(permutation_columns)
        .and_then(|columns| columns.checked_mul(PASTA_PROCESSED_POINT_BYTES))
        .and_then(|bytes| bytes.checked_add(VERIFYING_KEY_HEADER_BYTES))
        .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?;
    let proving_key_polynomials = processed_fixed_columns
        .checked_mul(2)
        .and_then(|columns| {
            permutation_columns
                .checked_mul(2)
                .and_then(|permutations| columns.checked_add(permutations))
        })
        .and_then(|columns| columns.checked_add(PROVING_KEY_MASK_POLYNOMIALS))
        .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?;
    let proving_key_bytes = proving_key_polynomials
        .checked_mul(polynomial_bytes)
        .and_then(|bytes| {
            PROVING_KEY_POLYNOMIAL_VECTORS
                .checked_mul(POLYNOMIAL_VECTOR_LENGTH_BYTES)
                .and_then(|headers| bytes.checked_add(headers))
        })
        .and_then(|bytes| bytes.checked_add(verifying_key_bytes))
        .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?;

    Ok(KagemushaProcessedKeyResourceProfileV1 {
        advice_columns,
        instance_columns,
        configured_fixed_columns,
        selector_columns,
        processed_fixed_columns,
        permutation_columns,
        verifying_key_bytes,
        proving_key_bytes,
    })
}

fn configured_processed_key_resources_v1<C, ConcreteCircuit>(
    k: usize,
    circuit: &ConcreteCircuit,
) -> Result<KagemushaProcessedKeyResourceProfileV1, ResourcePredictionErrorV1>
where
    C: CurveAffine,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    if C::Scalar::default().to_repr().as_ref().len() != 32
        || C::default().to_bytes().as_ref().len() != 32
    {
        return Err(ResourcePredictionErrorV1::UnsupportedProcessedEncodingWidth);
    }
    let mut constraint_system = ConstraintSystem::<C::Scalar>::default();
    let _ = ConcreteCircuit::configure_with_params(&mut constraint_system, circuit.params());
    predict_processed_key_resources_v1(
        k,
        constraint_system.num_advice_columns(),
        constraint_system.num_instance_columns(),
        constraint_system.num_fixed_columns(),
        constraint_system.num_selectors(),
        constraint_system.permutation().get_columns().len(),
    )
}

fn enforce_helper_key_limits_v1(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    profile: KagemushaProcessedKeyResourceProfileV1,
    proving_key_maximum: u64,
    verifying_key_maximum: u64,
) -> Result<KagemushaProcessedKeyResourceProfileV1, KagemushaArtifactGenerationErrorV1> {
    if profile.proving_key_bytes > proving_key_maximum
        || profile.verifying_key_bytes > verifying_key_maximum
    {
        return Err(
            KagemushaArtifactGenerationErrorV1::PredictedKeyResourceLimit {
                parity,
                kind,
                advice_columns: profile.advice_columns,
                instance_columns: profile.instance_columns,
                configured_fixed_columns: profile.configured_fixed_columns,
                selector_columns: profile.selector_columns,
                permutation_columns: profile.permutation_columns,
                predicted_proving_key_bytes: profile.proving_key_bytes,
                proving_key_maximum,
                predicted_verifying_key_bytes: profile.verifying_key_bytes,
                verifying_key_maximum,
            },
        );
    }
    Ok(profile)
}

/// Reject an actual Pasta circuit whose uncompressed Processed keys cannot fit helper limits.
pub(super) fn preflight_helper_key_resources_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    circuit: &ConcreteCircuit,
    parity: KagemushaPastaParityV1,
    kind: &'static str,
) -> Result<KagemushaProcessedKeyResourceProfileV1, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let k = usize::try_from(params.k()).map_err(|_| {
        KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
            "{kind} processed-key resource prediction failed: domain exponent does not fit usize"
        ))
    })?;
    let profile = configured_processed_key_resources_v1::<C, ConcreteCircuit>(k, circuit).map_err(
        |error| {
            KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
                "{kind} processed-key resource prediction failed: {error}"
            ))
        },
    )?;
    enforce_helper_key_limits_v1(
        parity,
        kind,
        profile,
        KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
        KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
    )
}

/// Generate a verifier key only after the actual configured helper layout passes preflight.
///
/// Callers retain their circuit-family profile validation before this shared guard. The ordinary
/// verifier-key backend uses uncompressed selectors, matching the prediction above. Returning
/// only after preflight makes it executable-testable that an over-limit layout never reaches
/// Halo2 synthesis or key allocation.
pub(super) fn keygen_vk_with_helper_resource_preflight_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    circuit: &ConcreteCircuit,
    parity: KagemushaPastaParityV1,
    resource_kind: &'static str,
    key_kind: &'static str,
) -> Result<VerifyingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    preflight_helper_key_resources_v1(params, circuit, parity, resource_kind)?;
    keygen_vk(params, circuit).map_err(|error| KagemushaArtifactGenerationErrorV1::KeyGeneration {
        parity,
        kind: key_kind,
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use core::marker::PhantomData;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use ff::PrimeField;
    use halo2_base::gates::circuit::BaseCircuitParams;
    use halo2_proofs::{
        SerdeFormat,
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Fixed, Instance, Selector},
        poly::{Rotation, commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };

    use super::*;
    use crate::zk::kagemusha_v1_recursion::mint_authority::{
        KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1,
    };

    #[derive(Clone, Default)]
    struct SmallProcessedKeyCircuit<F>(PhantomData<F>);

    #[derive(Clone)]
    struct OverLimitProcessedKeyCircuit<F> {
        synthesized: Arc<AtomicBool>,
        marker: PhantomData<F>,
    }

    impl<F: PrimeField> Circuit<F> for OverLimitProcessedKeyCircuit<F> {
        type Config = ();
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            self.clone()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            // At k=6, 2,048 Processed fixed commitments make the VK 65,546 bytes: ten
            // bytes above the immutable limit without allocating any domain polynomials.
            for _ in 0..2_048 {
                let _ = meta.fixed_column();
            }
        }

        fn synthesize(&self, (): Self::Config, _: impl Layouter<F>) -> Result<(), Error> {
            self.synthesized.store(true, Ordering::SeqCst);
            Err(Error::Synthesis)
        }
    }

    impl<F: PrimeField> Circuit<F> for SmallProcessedKeyCircuit<F> {
        type Config = (Column<Advice>, Column<Instance>, Column<Fixed>, Selector);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            let fixed = meta.fixed_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            meta.enable_constant(fixed);
            let selector = meta.selector();
            meta.create_gate("small processed-key gate", |meta| {
                let enabled = meta.query_selector(selector);
                let value = meta.query_advice(advice, Rotation::cur());
                vec![enabled * value]
            });
            (advice, instance, fixed, selector)
        }

        fn synthesize(
            &self,
            (advice, instance, _, selector): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            let cell = layouter.assign_region(
                || "small processed-key row",
                |mut region| {
                    selector.enable(&mut region, 0)?;
                    Ok(region
                        .assign_advice(advice, 0, Value::known(F::ZERO))
                        .cell())
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    #[test]
    fn arithmetic_and_serialized_domain_bounds_fail_closed() {
        assert_eq!(
            predict_processed_key_resources_v1(32, 0, 0, 0, 0, 0),
            Err(ResourcePredictionErrorV1::PolynomialDomainDoesNotFitU32)
        );
        assert_eq!(
            predict_processed_key_resources_v1(
                31,
                0,
                0,
                usize::try_from(u32::MAX).expect("u32 fits usize"),
                0,
                usize::try_from(u32::MAX).expect("u32 fits usize"),
            ),
            Err(ResourcePredictionErrorV1::ArithmeticOverflow)
        );
        if usize::BITS > 32 {
            assert_eq!(
                predict_processed_key_resources_v1(
                    6,
                    0,
                    0,
                    usize::try_from(u64::from(u32::MAX) + 1).expect("64-bit usize"),
                    0,
                    0,
                ),
                Err(ResourcePredictionErrorV1::ColumnCountDoesNotFitU32)
            );
        }
    }

    #[test]
    fn helper_limits_accept_the_boundary_and_reject_each_excess() {
        let profile =
            predict_processed_key_resources_v1(6, 1, 1, 1, 1, 2).expect("small resource profile");
        assert_eq!(
            enforce_helper_key_limits_v1(
                KagemushaPastaParityV1::Eq,
                "boundary",
                profile,
                profile.proving_key_bytes,
                profile.verifying_key_bytes,
            ),
            Ok(profile)
        );
        for (proving_maximum, verifying_maximum) in [
            (profile.proving_key_bytes - 1, profile.verifying_key_bytes),
            (profile.proving_key_bytes, profile.verifying_key_bytes - 1),
        ] {
            assert!(matches!(
                enforce_helper_key_limits_v1(
                    KagemushaPastaParityV1::Ep,
                    "boundary",
                    profile,
                    proving_maximum,
                    verifying_maximum,
                ),
                Err(KagemushaArtifactGenerationErrorV1::PredictedKeyResourceLimit { .. })
            ));
        }
    }

    #[test]
    fn guarded_vk_keygen_rejects_over_limit_config_before_synthesis_in_both_parities() {
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $parity:expr) => {{
                let synthesized = Arc::new(AtomicBool::new(false));
                let circuit = OverLimitProcessedKeyCircuit::<$scalar> {
                    synthesized: Arc::clone(&synthesized),
                    marker: PhantomData,
                };
                let params = ParamsIPA::<$curve>::new(6);
                assert!(matches!(
                    keygen_vk_with_helper_resource_preflight_v1(
                        &params,
                        &circuit,
                        $parity,
                        "over-limit test circuit",
                        "over-limit test verifying key",
                    ),
                    Err(
                        KagemushaArtifactGenerationErrorV1::PredictedKeyResourceLimit {
                            advice_columns: 0,
                            instance_columns: 0,
                            configured_fixed_columns: 2_048,
                            selector_columns: 0,
                            permutation_columns: 0,
                            predicted_verifying_key_bytes: 65_546,
                            verifying_key_maximum: KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
                            ..
                        }
                    )
                ));
                assert!(!synthesized.load(Ordering::SeqCst));
            }};
        }
        check_parity!(EqAffine, Fp, KagemushaPastaParityV1::Eq);
        check_parity!(EpAffine, Fq, KagemushaPastaParityV1::Ep);
    }

    #[test]
    fn current_inner_mint_authority_auxiliary_geometry_is_frozen_in_both_parities() {
        // Empty Base configuration isolates the unconditional auxiliary columns; this is
        // configure-only inventory, not a valid authority witness or a K16 key generation.
        // BaseConfig selects FlexGate here, whose empty column loops are supported.
        let params = BaseCircuitParams {
            k: 16,
            num_advice_per_phase: Vec::new(),
            num_fixed: 0,
            num_lookup_advice_per_phase: Vec::new(),
            lookup_bits: None,
            num_instance_columns: 0,
        };
        macro_rules! profile {
            ($scalar:ty, $circuit:ty) => {{
                let mut constraint_system = ConstraintSystem::<$scalar>::default();
                let _ = <$circuit as Circuit<$scalar>>::configure_with_params(
                    &mut constraint_system,
                    params.clone(),
                );
                predict_processed_key_resources_v1(
                    16,
                    constraint_system.num_advice_columns(),
                    constraint_system.num_instance_columns(),
                    constraint_system.num_fixed_columns(),
                    constraint_system.num_selectors(),
                    constraint_system.permutation().get_columns().len(),
                )
                .expect("inner auxiliary profile")
            }};
        }
        let eq = profile!(Fp, KagemushaMintAuthorityEqCircuitV1);
        let ep = profile!(Fq, KagemushaMintAuthorityEpCircuitV1);
        assert_eq!(eq, ep);
        assert_eq!(eq.advice_columns, 166);
        assert_eq!(eq.instance_columns, 0);
        assert_eq!(eq.configured_fixed_columns, 7);
        assert_eq!(eq.selector_columns, 110);
        assert_eq!(eq.processed_fixed_columns, 117);
        assert_eq!(eq.permutation_columns, 50);
        assert_eq!(eq.verifying_key_bytes, 5_354);
        assert_eq!(eq.proving_key_bytes, 706_746_942);
        for (parity, profile) in [
            (KagemushaPastaParityV1::Eq, eq),
            (KagemushaPastaParityV1::Ep, ep),
        ] {
            assert!(matches!(
                enforce_helper_key_limits_v1(
                    parity,
                    "inner mint authority auxiliary lower bound",
                    profile,
                    KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
                    KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
                ),
                Err(
                    KagemushaArtifactGenerationErrorV1::PredictedKeyResourceLimit {
                        predicted_proving_key_bytes: 706_746_942,
                        proving_key_maximum: KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
                        ..
                    }
                )
            ));
        }
    }

    #[test]
    fn small_k6_prediction_matches_real_processed_keys_in_both_parities() {
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $parity:expr) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let circuit = SmallProcessedKeyCircuit::<$scalar>::default();
                let profile = configured_processed_key_resources_v1::<$curve, _>(6, &circuit)
                    .expect("small configured resource profile");
                assert_eq!(
                    preflight_helper_key_resources_v1(
                        &params,
                        &circuit,
                        $parity,
                        "small test circuit",
                    ),
                    Ok(profile)
                );
                let verifying_key = keygen_vk_with_helper_resource_preflight_v1(
                    &params,
                    &circuit,
                    $parity,
                    "small test circuit",
                    "small test verifying key",
                )
                .expect("guarded small VK");
                let proving_key =
                    halo2_proofs::plonk::keygen_pk(&params, verifying_key.clone(), &circuit)
                        .expect("small PK");
                assert_eq!(
                    u64::try_from(verifying_key.to_bytes(SerdeFormat::Processed).len())
                        .expect("VK length"),
                    profile.verifying_key_bytes
                );
                assert_eq!(
                    u64::try_from(proving_key.to_bytes(SerdeFormat::Processed).len())
                        .expect("PK length"),
                    profile.proving_key_bytes
                );
            }};
        }
        check_parity!(EqAffine, Fp, KagemushaPastaParityV1::Eq);
        check_parity!(EpAffine, Fq, KagemushaPastaParityV1::Ep);
    }
}
