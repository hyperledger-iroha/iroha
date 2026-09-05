//! Early processed-key resource prediction for fixed Pasta circuits.
//!
//! Kagemusha helper keys use Halo2 selector compression. Configure-only preflight accounts for a
//! sound minimum or maximum number of compressed selector columns, and the consuming keygen path
//! checks the exact synthesized selector overlap before selector bitmaps become field
//! polynomials. The later serialization checks remain authoritative and defend against
//! backend-format drift.

use ff::{FromUniformBytes, PrimeField as _};
use halo2_proofs::{
    halo2curves::CurveAffine,
    plonk::{
        Circuit, ConstraintSystem, KeygenCircuitResourceProfile, KeygenWithExtractorError,
        ProvingKey, VerifyingKey, keygen_pk2_consuming_with_profile,
        keygen_vk_consuming_with_profile, keygen_vk_custom,
    },
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
// At k=16, 1,024 advice columns already require at least 4 GiB for just two 32-byte
// domain-sized field buffers, before assigned values, FFT/MSM scratch, fixed columns, or keys.
// The old full-audit regression configured 6,738 advice columns and could therefore enter a
// >160 GiB process before the serialized-key checks noticed nothing unusual. Keep an explicit
// configure-only ceiling so an advice-width regression fails before synthesis or allocation.
const KAGEMUSHA_HELPER_ADVICE_COLUMN_MAX_V1: u64 = 1_024;

/// Serialization limits applied after the independent configure-only width guard.
///
/// Release generation always uses [`Self::release`].  The explicitly ignored, externally
/// memory-guarded real-proof test may select a larger byte envelope so it can exercise keygen and
/// proving while the claim arithmetization is being compacted.  This type does not relax the
/// advice-column ceiling and is intentionally not exposed outside the recursion generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaProcessedKeyLimitsV1 {
    pub(super) proving_key_maximum: u64,
    pub(super) verifying_key_maximum: u64,
}

impl KagemushaProcessedKeyLimitsV1 {
    pub(super) const fn release() -> Self {
        Self {
            proving_key_maximum: KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
            verifying_key_maximum: KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
        }
    }

    #[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
    pub(super) const fn guarded_real_proof() -> Self {
        // These are serialization envelopes, not resident-memory allowances.  The sole caller is
        // additionally confined by scripts/run_kagemusha_real_proof_guarded.py's aggregate
        // process-group limit. Keeping this opt-in behind either `cfg(test)` or the non-shipping
        // dedicated harness feature prevents an oversized key from becoming release-eligible by
        // accident; the dedicated binary remains confined by the same guard.
        Self {
            proving_key_maximum: 1024 * 1024 * 1024,
            verifying_key_maximum: 2 * 1024 * 1024,
        }
    }
}

/// Column inventory and Processed-format byte prediction for one Pasta circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaProcessedKeyResourceProfileV1 {
    /// Advice columns configured by the circuit.
    pub(super) advice_columns: u64,
    /// Instance columns configured by the circuit.
    pub(super) instance_columns: u64,
    /// Fixed columns configured before selector materialization.
    pub(super) configured_fixed_columns: u64,
    /// Original virtual selectors (and compressed-selector bitmaps).
    pub(super) selector_columns: u64,
    /// Fixed polynomials produced by selector materialization.
    pub(super) materialized_selector_columns: u64,
    /// Fixed polynomials serialized after selector materialization.
    pub(super) processed_fixed_columns: u64,
    /// Columns participating in the permutation argument.
    pub(super) permutation_columns: u64,
    /// Bytes occupied by bit-packed original selector activations in the verifier key.
    pub(super) selector_bitmap_bytes: u64,
    /// Whether the prediction uses compressed selector serialization.
    pub(super) compress_selectors: bool,
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
    predict_processed_key_resources_with_selectors_v1(
        k,
        advice_columns,
        instance_columns,
        configured_fixed_columns,
        selector_columns,
        selector_columns,
        permutation_columns,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn predict_processed_key_resources_with_selectors_v1(
    k: usize,
    advice_columns: usize,
    instance_columns: usize,
    configured_fixed_columns: usize,
    selector_columns: usize,
    materialized_selector_columns: usize,
    permutation_columns: usize,
    compress_selectors: bool,
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
    let materialized_selector_columns = checked_count(materialized_selector_columns)?;
    let permutation_columns = checked_count(permutation_columns)?;
    let processed_fixed_columns = configured_fixed_columns
        .checked_add(materialized_selector_columns)
        .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?;
    if processed_fixed_columns > u64::from(u32::MAX) {
        return Err(ResourcePredictionErrorV1::ColumnCountDoesNotFitU32);
    }

    let polynomial_bytes = domain_rows
        .checked_mul(PASTA_PROCESSED_SCALAR_BYTES)
        .and_then(|bytes| bytes.checked_add(POLYNOMIAL_LENGTH_BYTES))
        .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?;
    let selector_bitmap_bytes = if compress_selectors {
        selector_columns
            .checked_mul(domain_rows.div_ceil(8))
            .ok_or(ResourcePredictionErrorV1::ArithmeticOverflow)?
    } else {
        0
    };
    let verifying_key_bytes = processed_fixed_columns
        .checked_add(permutation_columns)
        .and_then(|columns| columns.checked_mul(PASTA_PROCESSED_POINT_BYTES))
        .and_then(|bytes| bytes.checked_add(VERIFYING_KEY_HEADER_BYTES))
        .and_then(|bytes| bytes.checked_add(selector_bitmap_bytes))
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
        materialized_selector_columns,
        processed_fixed_columns,
        permutation_columns,
        selector_bitmap_bytes,
        compress_selectors,
        verifying_key_bytes,
        proving_key_bytes,
    })
}

fn configured_compressed_key_resources_v1<C, ConcreteCircuit>(
    k: usize,
    circuit: &ConcreteCircuit,
    minimum: bool,
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
    let materialized_selector_columns = if minimum {
        constraint_system.minimum_compressed_selector_columns()
    } else {
        // Compression can never produce more fixed columns than direct one-for-one selector
        // materialization, independent of synthesized activations.
        constraint_system.num_selectors()
    };
    predict_processed_key_resources_with_selectors_v1(
        k,
        constraint_system.num_advice_columns(),
        constraint_system.num_instance_columns(),
        constraint_system.num_fixed_columns(),
        constraint_system.num_selectors(),
        materialized_selector_columns,
        constraint_system.permutation().get_columns().len(),
        true,
    )
}

fn configured_minimum_compressed_key_resources_for_params_v1<C, ConcreteCircuit>(
    k: usize,
    circuit_params: ConcreteCircuit::Params,
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
    let _ = ConcreteCircuit::configure_with_params(&mut constraint_system, circuit_params);
    predict_processed_key_resources_with_selectors_v1(
        k,
        constraint_system.num_advice_columns(),
        constraint_system.num_instance_columns(),
        constraint_system.num_fixed_columns(),
        constraint_system.num_selectors(),
        constraint_system.minimum_compressed_selector_columns(),
        constraint_system.permutation().get_columns().len(),
        true,
    )
}

fn exact_processed_key_resources_v1<C: CurveAffine>(
    profile: KeygenCircuitResourceProfile,
) -> Result<KagemushaProcessedKeyResourceProfileV1, ResourcePredictionErrorV1> {
    if C::Scalar::default().to_repr().as_ref().len() != 32
        || C::default().to_bytes().as_ref().len() != 32
    {
        return Err(ResourcePredictionErrorV1::UnsupportedProcessedEncodingWidth);
    }
    if !profile.domain_rows.is_power_of_two() {
        return Err(ResourcePredictionErrorV1::PolynomialDomainDoesNotFitU32);
    }
    let k = usize::try_from(profile.domain_rows.ilog2())
        .map_err(|_| ResourcePredictionErrorV1::DomainExponentDoesNotFitU32)?;
    predict_processed_key_resources_with_selectors_v1(
        k,
        profile.advice_columns,
        profile.instance_columns,
        profile.configured_fixed_columns,
        profile.selector_columns,
        profile.materialized_selector_columns,
        profile.permutation_columns,
        profile.compress_selectors,
    )
}

fn enforce_helper_key_limits_v1(
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    profile: KagemushaProcessedKeyResourceProfileV1,
    proving_key_maximum: u64,
    verifying_key_maximum: u64,
) -> Result<KagemushaProcessedKeyResourceProfileV1, KagemushaArtifactGenerationErrorV1> {
    if profile.advice_columns > KAGEMUSHA_HELPER_ADVICE_COLUMN_MAX_V1 {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
            "{kind} configures {} advice columns, exceeding the hard pre-synthesis maximum of {KAGEMUSHA_HELPER_ADVICE_COLUMN_MAX_V1}",
            profile.advice_columns,
        )));
    }
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
                materialized_selector_columns: profile.materialized_selector_columns,
                selector_bitmap_bytes: profile.selector_bitmap_bytes,
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

/// Reject a Pasta circuit if even the conservative compressed-selector upper bound cannot fit.
///
/// This borrowed-circuit guard cannot observe synthesized selector overlap. Production helper
/// generation uses the consuming guards below, which replace this bound with an exact
/// post-synthesis check before key expansion.
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
    let profile = configured_compressed_key_resources_v1::<C, ConcreteCircuit>(k, circuit, false)
        .map_err(|error| {
        KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
            "{kind} processed-key resource prediction failed: {error}"
        ))
    })?;
    enforce_helper_key_limits_v1(
        parity,
        kind,
        profile,
        KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
        KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
    )
}

/// Reject a configured helper layout before constructing its witness graph.
///
/// This is intentionally parameter-only. Large fixed auxiliary gadgets configure independently
/// of the Base witness, so release generation can reject an accidental column explosion before
/// allocating either parity's circuit graph. The ordinary circuit-specific preflight remains the
/// authoritative full-layout check immediately before key generation.
pub(super) fn preflight_helper_key_configuration_v1<C, ConcreteCircuit>(
    k: usize,
    circuit_params: ConcreteCircuit::Params,
    parity: KagemushaPastaParityV1,
    kind: &'static str,
) -> Result<KagemushaProcessedKeyResourceProfileV1, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    preflight_key_configuration_with_limits_v1::<C, ConcreteCircuit>(
        k,
        circuit_params,
        parity,
        kind,
        KagemushaProcessedKeyLimitsV1::release(),
    )
}

fn preflight_key_configuration_with_limits_v1<C, ConcreteCircuit>(
    k: usize,
    circuit_params: ConcreteCircuit::Params,
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    limits: KagemushaProcessedKeyLimitsV1,
) -> Result<KagemushaProcessedKeyResourceProfileV1, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let profile = configured_minimum_compressed_key_resources_for_params_v1::<C, ConcreteCircuit>(
        k,
        circuit_params,
    )
    .map_err(|error| {
        KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
            "{kind} processed-key resource prediction failed: {error}"
        ))
    })?;
    enforce_helper_key_limits_v1(
        parity,
        kind,
        profile,
        limits.proving_key_maximum,
        limits.verifying_key_maximum,
    )
}

/// Generate a verifier key only after the actual configured helper layout passes preflight.
///
/// Callers retain their circuit-family profile validation before this shared guard. The ordinary
/// verifier-key backend uses compressed selectors. Returning only after the conservative upper
/// bound passes makes it executable-testable that an obviously over-limit layout never reaches
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
    keygen_vk_custom(params, circuit, true).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity,
            kind: key_kind,
            reason: error.to_string(),
        }
    })
}

/// Generate a verifier key after preflight while releasing the owned circuit before key assembly.
pub(super) fn keygen_vk_with_helper_resource_preflight_consuming_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    circuit: ConcreteCircuit,
    parity: KagemushaPastaParityV1,
    resource_kind: &'static str,
    key_kind: &'static str,
) -> Result<VerifyingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    keygen_vk_with_key_resource_limits_consuming_v1(
        params,
        circuit,
        parity,
        resource_kind,
        key_kind,
        KagemushaProcessedKeyLimitsV1::release(),
    )
}

/// Generate a verifier key under an explicit serialization envelope.
///
/// The hard configure-only advice-width ceiling remains active for every envelope.
pub(super) fn keygen_vk_with_key_resource_limits_consuming_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    circuit: ConcreteCircuit,
    parity: KagemushaPastaParityV1,
    resource_kind: &'static str,
    key_kind: &'static str,
    limits: KagemushaProcessedKeyLimitsV1,
) -> Result<VerifyingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let k = usize::try_from(params.k()).map_err(|_| {
        KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
            "{resource_kind} processed-key resource prediction failed: domain exponent does not fit usize"
        ))
    })?;
    preflight_key_configuration_with_limits_v1::<C, ConcreteCircuit>(
        k,
        circuit.params(),
        parity,
        resource_kind,
        limits,
    )?;
    match keygen_vk_consuming_with_profile(params, circuit, true, |_circuit, keygen_profile| {
        let profile = exact_processed_key_resources_v1::<C>(keygen_profile).map_err(|error| {
            KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
                "{resource_kind} exact processed-key resource prediction failed: {error}"
            ))
        })?;
        enforce_helper_key_limits_v1(
            parity,
            resource_kind,
            profile,
            limits.proving_key_maximum,
            limits.verifying_key_maximum,
        )
    }) {
        Ok((key, _profile)) => Ok(key),
        Err(KeygenWithExtractorError::Keygen(error)) => {
            Err(KagemushaArtifactGenerationErrorV1::KeyGeneration {
                parity,
                kind: key_kind,
                reason: error.to_string(),
            })
        }
        Err(KeygenWithExtractorError::Extractor(error)) => Err(error),
    }
}

/// Generate one combined proving/verifying key after preflight, consuming the circuit graph.
///
/// Unlike separate borrowed `keygen_vk` and `keygen_pk` calls, this synthesizes once and releases
/// the owned witness/configuration graph before Halo2 expands the proving-key polynomials. This is
/// the required path for the large fixed Kagemusha helper circuits.
pub(in crate::zk::kagemusha_v1_recursion) fn keygen_pk_with_helper_resource_preflight_consuming_v1<
    C,
    ConcreteCircuit,
>(
    params: &ParamsIPA<C>,
    circuit: ConcreteCircuit,
    parity: KagemushaPastaParityV1,
    resource_kind: &'static str,
    key_kind: &'static str,
) -> Result<ProvingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    keygen_pk_with_key_resource_limits_consuming_v1(
        params,
        circuit,
        parity,
        resource_kind,
        key_kind,
        KagemushaProcessedKeyLimitsV1::release(),
    )
}

/// Generate a combined proving/verifying key under an explicit serialization envelope.
///
/// The hard configure-only advice-width ceiling remains active for every envelope.
pub(super) fn keygen_pk_with_key_resource_limits_consuming_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    circuit: ConcreteCircuit,
    parity: KagemushaPastaParityV1,
    resource_kind: &'static str,
    key_kind: &'static str,
    limits: KagemushaProcessedKeyLimitsV1,
) -> Result<ProvingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let k = usize::try_from(params.k()).map_err(|_| {
        KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
            "{resource_kind} processed-key resource prediction failed: domain exponent does not fit usize"
        ))
    })?;
    preflight_key_configuration_with_limits_v1::<C, ConcreteCircuit>(
        k,
        circuit.params(),
        parity,
        resource_kind,
        limits,
    )?;
    match keygen_pk2_consuming_with_profile(params, circuit, true, |_circuit, keygen_profile| {
        let profile = exact_processed_key_resources_v1::<C>(keygen_profile).map_err(|error| {
            KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
                "{resource_kind} exact processed-key resource prediction failed: {error}"
            ))
        })?;
        enforce_helper_key_limits_v1(
            parity,
            resource_kind,
            profile,
            limits.proving_key_maximum,
            limits.verifying_key_maximum,
        )
    }) {
        Ok((key, _profile)) => Ok(key),
        Err(KeygenWithExtractorError::Keygen(error)) => {
            Err(KagemushaArtifactGenerationErrorV1::KeyGeneration {
                parity,
                kind: key_kind,
                reason: error.to_string(),
            })
        }
        Err(KeygenWithExtractorError::Extractor(error)) => Err(error),
    }
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
    use crate::zk::kagemusha_v1_recursion::{
        guard_bundle::KagemushaPlatformCredentialRelationCircuitV1,
        mint_authority::{KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1},
        mint_authorization::{
            KagemushaMintAuthorizationEpCircuitV1, KagemushaMintAuthorizationEqCircuitV1,
        },
        mint_hash_claim_fold::{
            KagemushaMintHashClaimEpCircuitV1, KagemushaMintHashClaimEqCircuitV1,
        },
    };

    #[derive(Clone, Default)]
    struct SmallProcessedKeyCircuit<F>(PhantomData<F>);

    struct DropTrackedSmallProcessedKeyCircuit<F> {
        owner_dropped: Option<Arc<AtomicBool>>,
        marker: PhantomData<F>,
    }

    impl<F> Drop for DropTrackedSmallProcessedKeyCircuit<F> {
        fn drop(&mut self) {
            if let Some(dropped) = self.owner_dropped.as_ref() {
                dropped.store(true, Ordering::SeqCst);
            }
        }
    }

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
        type Config = (
            Column<Advice>,
            Column<Instance>,
            Column<Fixed>,
            Selector,
            Selector,
        );
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
            let first_selector = meta.selector();
            let second_selector = meta.selector();
            meta.create_gate("small processed-key first gate", |meta| {
                let enabled = meta.query_selector(first_selector);
                let value = meta.query_advice(advice, Rotation::cur());
                vec![enabled * value]
            });
            meta.create_gate("small processed-key second gate", |meta| {
                let enabled = meta.query_selector(second_selector);
                let value = meta.query_advice(advice, Rotation::cur());
                vec![enabled * value]
            });
            (advice, instance, fixed, first_selector, second_selector)
        }

        fn synthesize(
            &self,
            (advice, instance, _, first_selector, second_selector): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            let cell = layouter.assign_region(
                || "small processed-key row",
                |mut region| {
                    first_selector.enable(&mut region, 0)?;
                    second_selector.enable(&mut region, 1)?;
                    let cell = region
                        .assign_advice(advice, 0, Value::known(F::ZERO))
                        .cell();
                    let _ = region.assign_advice(advice, 1, Value::known(F::ZERO));
                    Ok(cell)
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    impl<F: PrimeField> Circuit<F> for DropTrackedSmallProcessedKeyCircuit<F> {
        type Config = <SmallProcessedKeyCircuit<F> as Circuit<F>>::Config;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self {
                owner_dropped: None,
                marker: PhantomData,
            }
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            SmallProcessedKeyCircuit::<F>::configure(meta)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            SmallProcessedKeyCircuit::<F>::default().synthesize(config, layouter)
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
    fn helper_limits_reject_advice_width_before_key_size_can_hide_it() {
        let profile = predict_processed_key_resources_v1(
            16,
            usize::try_from(KAGEMUSHA_HELPER_ADVICE_COLUMN_MAX_V1 + 1)
                .expect("advice limit fits usize"),
            1,
            0,
            0,
            0,
        )
        .expect("advice-only resource profile");
        assert!(profile.proving_key_bytes < KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1);
        let error = enforce_helper_key_limits_v1(
            KagemushaPastaParityV1::Eq,
            "advice-width regression",
            profile,
            KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
            KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
        )
        .expect_err("advice width must be bounded independently of serialized key size");
        assert!(
            matches!(error, KagemushaArtifactGenerationErrorV1::CircuitBuild(reason) if reason.contains("1025 advice columns")),
        );
    }

    #[test]
    fn compressed_prediction_accounts_for_exact_original_selector_bitmaps() {
        let profile = predict_processed_key_resources_with_selectors_v1(6, 1, 1, 1, 3, 1, 2, true)
            .expect("compressed resource profile");
        assert!(profile.compress_selectors);
        assert_eq!(profile.materialized_selector_columns, 1);
        assert_eq!(profile.processed_fixed_columns, 2);
        assert_eq!(profile.selector_bitmap_bytes, 3 * (64 / 8));
        assert_eq!(profile.verifying_key_bytes, 162);
        assert_eq!(profile.proving_key_bytes, 22_750);
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
        let eq = configured_minimum_compressed_key_resources_for_params_v1::<
            EqAffine,
            KagemushaMintAuthorityEqCircuitV1,
        >(16, params.clone())
        .expect("Eq inner auxiliary profile");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaMintAuthorityEpCircuitV1,
        >(16, params)
        .expect("Ep inner auxiliary profile");
        assert_eq!(eq, ep);
        assert!(eq.compress_selectors);
        assert_eq!(eq.advice_columns, 148);
        assert_eq!(eq.instance_columns, 0);
        assert_eq!(eq.configured_fixed_columns, 1);
        assert_eq!(eq.selector_columns, 0);
        assert_eq!(eq.processed_fixed_columns, 1);
        assert_eq!(eq.permutation_columns, 4);
        assert_eq!(eq.verifying_key_bytes, 170);
        assert_eq!(eq.proving_key_bytes, 27_263_214);
        for (parity, profile) in [
            (KagemushaPastaParityV1::Eq, eq),
            (KagemushaPastaParityV1::Ep, ep),
        ] {
            enforce_helper_key_limits_v1(
                parity,
                "inner mint authority auxiliary lower bound",
                profile,
                KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
                KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
            )
            .expect("inner mint-authority auxiliary geometry fits helper key limits");
        }
    }

    #[test]
    fn platform_credential_claim_consumer_has_no_k16_sha_auxiliary_columns() {
        // Empty Base parameters isolate the reciprocal dense-MSM machine. PlatformCredential SHA
        // is proved by the ordered k=12 shard/claim path, so no Table8 selector bitmap or columns
        // may return to this k=16 monetary producer.
        let params = BaseCircuitParams {
            k: 16,
            num_advice_per_phase: Vec::new(),
            num_fixed: 0,
            num_lookup_advice_per_phase: Vec::new(),
            lookup_bits: None,
            num_instance_columns: 0,
        };
        let eq = configured_minimum_compressed_key_resources_for_params_v1::<
            EqAffine,
            KagemushaPlatformCredentialRelationCircuitV1<Fp>,
        >(16, params.clone())
        .expect("Eq PlatformCredential claim-consumer auxiliary profile");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaPlatformCredentialRelationCircuitV1<Fq>,
        >(16, params)
        .expect("Ep PlatformCredential claim-consumer auxiliary profile");
        assert_eq!(eq, ep);
        assert_eq!(eq.advice_columns, 148);
        assert_eq!(eq.instance_columns, 0);
        assert_eq!(eq.configured_fixed_columns, 1);
        assert_eq!(eq.selector_columns, 0);
        assert_eq!(eq.permutation_columns, 4);
        assert!(eq.compress_selectors);
        assert_eq!(eq.materialized_selector_columns, 0);
        assert_eq!(eq.selector_bitmap_bytes, 0);
        assert_eq!(eq.processed_fixed_columns, 1);
        assert_eq!(eq.verifying_key_bytes, 170);
        assert_eq!(eq.proving_key_bytes, 27_263_214);
        for (parity, profile) in [
            (KagemushaPastaParityV1::Eq, eq),
            (KagemushaPastaParityV1::Ep, ep),
        ] {
            enforce_helper_key_limits_v1(
                parity,
                "PlatformCredential claim-consumer auxiliary lower bound",
                profile,
                KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
                KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
            )
            .expect("PlatformCredential claim-consumer auxiliary geometry fits helper limits");
        }
    }

    #[test]
    fn mint_authorization_dense_only_auxiliary_geometry_fits_helper_limits() {
        // Empty Base parameters isolate the unconditional reciprocal dense-MSM machine. The exact
        // MintAuthorization SHA queue is authenticated by the ordered shard/claim path, so no
        // Table8 advice, fixed columns, selectors, permutation columns, or bitmap may return here.
        let params = BaseCircuitParams {
            k: 16,
            num_advice_per_phase: Vec::new(),
            num_fixed: 0,
            num_lookup_advice_per_phase: Vec::new(),
            lookup_bits: None,
            num_instance_columns: 0,
        };
        let eq = configured_minimum_compressed_key_resources_for_params_v1::<
            EqAffine,
            KagemushaMintAuthorizationEqCircuitV1,
        >(16, params.clone())
        .expect("Eq MintAuthorization dense-only auxiliary profile");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaMintAuthorizationEpCircuitV1,
        >(16, params)
        .expect("Ep MintAuthorization dense-only auxiliary profile");
        assert_eq!(eq, ep);
        assert!(eq.compress_selectors);
        assert_eq!(eq.advice_columns, 148);
        assert_eq!(eq.instance_columns, 0);
        assert_eq!(eq.configured_fixed_columns, 1);
        assert_eq!(eq.selector_columns, 0);
        assert_eq!(eq.materialized_selector_columns, 0);
        assert_eq!(eq.processed_fixed_columns, 1);
        assert_eq!(eq.permutation_columns, 4);
        assert_eq!(eq.selector_bitmap_bytes, 0);
        assert_eq!(eq.verifying_key_bytes, 170);
        assert_eq!(eq.proving_key_bytes, 27_263_214);
        assert!(eq.verifying_key_bytes <= KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1);
        assert!(eq.proving_key_bytes <= KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1);
        for (parity, profile) in [
            (KagemushaPastaParityV1::Eq, eq),
            (KagemushaPastaParityV1::Ep, ep),
        ] {
            enforce_helper_key_limits_v1(
                parity,
                "inner MintAuthorization dense-only auxiliary lower bound",
                profile,
                KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
                KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
            )
            .expect("MintAuthorization dense-only auxiliary geometry fits helper limits");
        }
    }

    #[test]
    fn mint_hash_claim_dense_only_auxiliary_geometry_is_frozen() {
        // This lower bound catches regressions in the compact reciprocal verifier before an
        // exact claim witness is built. The consuming preflight still measures the complete Base
        // layout and enforces the immutable key limits immediately before key expansion.
        let params = BaseCircuitParams {
            k: 16,
            num_advice_per_phase: Vec::new(),
            num_fixed: 0,
            num_lookup_advice_per_phase: Vec::new(),
            lookup_bits: None,
            num_instance_columns: 0,
        };
        let eq = configured_minimum_compressed_key_resources_for_params_v1::<
            EqAffine,
            KagemushaMintHashClaimEqCircuitV1,
        >(16, params.clone())
        .expect("Eq MintHashClaim dense-only auxiliary profile");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaMintHashClaimEpCircuitV1,
        >(16, params)
        .expect("Ep MintHashClaim dense-only auxiliary profile");
        assert_eq!(eq, ep);
        assert!(eq.compress_selectors);
        assert_eq!(eq.advice_columns, 74);
        assert_eq!(eq.instance_columns, 0);
        assert_eq!(eq.configured_fixed_columns, 1);
        assert_eq!(eq.selector_columns, 0);
        assert_eq!(eq.materialized_selector_columns, 0);
        assert_eq!(eq.processed_fixed_columns, 1);
        assert_eq!(eq.permutation_columns, 2);
        assert_eq!(eq.selector_bitmap_bytes, 0);
        assert_eq!(eq.verifying_key_bytes, 106);
        assert_eq!(eq.proving_key_bytes, 18_874_526);
    }

    #[test]
    fn small_k6_prediction_matches_real_processed_keys_in_both_parities() {
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $parity:expr) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let circuit = SmallProcessedKeyCircuit::<$scalar>::default();
                let profile =
                    configured_compressed_key_resources_v1::<$curve, _>(6, &circuit, false)
                        .expect("small compressed resource profile");
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
                assert!(profile.compress_selectors);
                assert_eq!(profile.selector_columns, 2);
                assert_eq!(profile.materialized_selector_columns, 1);
                assert_eq!(profile.selector_bitmap_bytes, 16);
                assert_eq!(verifying_key.to_bytes(SerdeFormat::Processed)[5], 1);
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
                let proving_key_bytes = proving_key.to_bytes(SerdeFormat::Processed);
                let restored =
                    ProvingKey::<$curve>::read_checked::<_, SmallProcessedKeyCircuit<$scalar>>(
                        &mut proving_key_bytes.as_slice(),
                        SerdeFormat::Processed,
                        6,
                        (),
                    )
                    .expect("compressed checked Processed PK roundtrip");
                assert_eq!(restored.to_bytes(SerdeFormat::Processed), proving_key_bytes);
            }};
        }
        check_parity!(EqAffine, Fp, KagemushaPastaParityV1::Eq);
        check_parity!(EpAffine, Fq, KagemushaPastaParityV1::Ep);
    }

    #[test]
    fn consuming_combined_keygen_preserves_keys_and_drops_owned_circuit_in_both_parities() {
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $parity:expr) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let reference = SmallProcessedKeyCircuit::<$scalar>::default();
                let reference_pk = halo2_proofs::plonk::keygen_pk2(&params, &reference, true)
                    .expect("reference compressed PK");
                let owner_dropped = Arc::new(AtomicBool::new(false));
                let consuming_pk = keygen_pk_with_helper_resource_preflight_consuming_v1(
                    &params,
                    DropTrackedSmallProcessedKeyCircuit::<$scalar> {
                        owner_dropped: Some(Arc::clone(&owner_dropped)),
                        marker: PhantomData,
                    },
                    $parity,
                    "small compressed consuming test circuit",
                    "small compressed consuming test proving key",
                )
                .expect("consuming combined PK");
                assert!(
                    owner_dropped.load(Ordering::SeqCst),
                    "owned circuit must be dropped before keygen returns"
                );
                assert_eq!(
                    consuming_pk.to_bytes(SerdeFormat::Processed),
                    reference_pk.to_bytes(SerdeFormat::Processed),
                    "consuming keygen must preserve canonical PK/VK bytes"
                );
            }};
        }
        check_parity!(EqAffine, Fp, KagemushaPastaParityV1::Eq);
        check_parity!(EpAffine, Fq, KagemushaPastaParityV1::Ep);
    }
}
