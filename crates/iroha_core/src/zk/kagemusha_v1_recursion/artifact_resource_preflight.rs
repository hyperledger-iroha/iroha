//! Early processed-key resource prediction for fixed Pasta circuits.
//!
//! Consuming Kagemusha helper key generation chooses compressed or direct selector encoding from
//! exact synthesized profiles, checking both unchanged key limits before polynomial expansion.
//! Configure-only preflight considers both alternatives; later serialization and authenticated
//! key checks remain authoritative and defend against backend-format drift.

use ff::{FromUniformBytes, PrimeField as _};
use halo2_proofs::{
    halo2curves::CurveAffine,
    plonk::{
        Circuit, ConstraintSystem, KeygenCircuitResourceProfile, KeygenSelectorProfiles,
        KeygenWithExtractorError, ProvingKey, VerifyingKey,
        keygen_pk2_consuming_with_selector_choice, keygen_vk_consuming_with_selector_choice,
        keygen_vk_custom,
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
/// Release generation checks both selector encodings against [`Self::release`].  The explicitly ignored, externally
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

#[cfg(test)]
fn configured_minimum_compressed_key_resources_for_params_v1<C, ConcreteCircuit>(
    k: usize,
    circuit_params: ConcreteCircuit::Params,
) -> Result<KagemushaProcessedKeyResourceProfileV1, ResourcePredictionErrorV1>
where
    C: CurveAffine,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    configured_key_encoding_profiles_for_params_v1::<C, ConcreteCircuit>(k, circuit_params)
        .map(|profiles| profiles[0])
}

fn configured_key_encoding_profiles_for_params_v1<C, ConcreteCircuit>(
    k: usize,
    circuit_params: ConcreteCircuit::Params,
) -> Result<[KagemushaProcessedKeyResourceProfileV1; 2], ResourcePredictionErrorV1>
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
    let compressed = predict_processed_key_resources_with_selectors_v1(
        k,
        constraint_system.num_advice_columns(),
        constraint_system.num_instance_columns(),
        constraint_system.num_fixed_columns(),
        constraint_system.num_selectors(),
        constraint_system.minimum_compressed_selector_columns(),
        constraint_system.permutation().get_columns().len(),
        true,
    )?;
    let direct = predict_processed_key_resources_with_selectors_v1(
        k,
        constraint_system.num_advice_columns(),
        constraint_system.num_instance_columns(),
        constraint_system.num_fixed_columns(),
        constraint_system.num_selectors(),
        constraint_system.num_selectors(),
        constraint_system.permutation().get_columns().len(),
        false,
    )?;
    Ok([compressed, direct])
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

/// Choose only among encodings satisfying both key limits and the independent advice ceiling.
///
/// Sorting is deterministic: smaller PK, then smaller VK, then the historical compressed mode
/// on an exact tie. A PK-feasible encoding and a different VK-feasible encoding do not combine
/// into a feasible choice. If neither fits, return the first concrete rejection in that order.
fn choose_helper_key_encoding_v1(
    mut profiles: [KagemushaProcessedKeyResourceProfileV1; 2],
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    limits: KagemushaProcessedKeyLimitsV1,
) -> Result<KagemushaProcessedKeyResourceProfileV1, KagemushaArtifactGenerationErrorV1> {
    profiles.sort_by_key(|profile| {
        (
            profile.proving_key_bytes,
            profile.verifying_key_bytes,
            !profile.compress_selectors,
        )
    });
    let mut first_error = None;
    for profile in profiles {
        match enforce_helper_key_limits_v1(
            parity,
            kind,
            profile,
            limits.proving_key_maximum,
            limits.verifying_key_maximum,
        ) {
            Ok(profile) => return Ok(profile),
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }
    Err(first_error.expect("both fixed encoding alternatives were rejected"))
}

fn choose_synthesized_helper_key_encoding_v1<C: CurveAffine>(
    profiles: KeygenSelectorProfiles,
    parity: KagemushaPastaParityV1,
    kind: &'static str,
    limits: KagemushaProcessedKeyLimitsV1,
) -> Result<(bool, KagemushaProcessedKeyResourceProfileV1), KagemushaArtifactGenerationErrorV1> {
    let predicted =
        [profiles.compressed, profiles.direct].map(exact_processed_key_resources_v1::<C>);
    let [compressed, direct] = predicted;
    let profiles = compressed
        .and_then(|compressed| direct.map(|direct| [compressed, direct]))
        .map_err(|error| {
            KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
                "{kind} exact processed-key resource prediction failed: {error}"
            ))
        })?;
    let selected = choose_helper_key_encoding_v1(profiles, parity, kind, limits)?;
    Ok((selected.compress_selectors, selected))
}

/// Reject a configured helper layout before constructing its witness graph.
///
/// This is intentionally parameter-only: either encoding may pass its optimistic bound.
/// The consuming guard selects from the exact synthesized alternatives. Large fixed auxiliary
/// gadgets configure independently
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

/// Check a configured helper against the caller's explicit key-size envelope.
///
/// Release generation supplies the unchanged release limits. The test-only guarded proof
/// corridor supplies its existing diagnostic limits and retains the same advice-width guard.
/// Exact synthesized sizing remains mandatory before key expansion in either corridor.
pub(super) fn preflight_key_configuration_with_limits_v1<C, ConcreteCircuit>(
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
    let profiles =
        configured_key_encoding_profiles_for_params_v1::<C, ConcreteCircuit>(k, circuit_params)
            .map_err(|error| {
                KagemushaArtifactGenerationErrorV1::CircuitBuild(format!(
                    "{kind} processed-key resource prediction failed: {error}"
                ))
            })?;
    choose_helper_key_encoding_v1(profiles, parity, kind, limits)
}

/// Generate a verifier key only after the actual configured helper layout passes preflight.
///
/// Callers retain their circuit-family profile validation before this shared guard. The ordinary
/// borrowed verifier-key backend retains compressed selectors; production uses the adaptive
/// consuming wrappers below. Returning only after the conservative upper
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

/// Generate a verifier key using the smallest feasible exact selector encoding.
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
    match keygen_vk_consuming_with_selector_choice(params, circuit, |_circuit, profiles| {
        choose_synthesized_helper_key_encoding_v1::<C>(profiles, parity, resource_kind, limits)
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

/// Generate combined keys using the smallest feasible exact selector encoding.
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
    match keygen_pk2_consuming_with_selector_choice(params, circuit, |_circuit, profiles| {
        choose_synthesized_helper_key_encoding_v1::<C>(profiles, parity, resource_kind, limits)
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

    use ff::{Field, PrimeField};
    use halo2_base::gates::circuit::BaseCircuitParams;
    use halo2_proofs::{
        SerdeFormat,
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
        plonk::{
            Advice, Circuit, Column, ConstraintSystem, Error, Fixed, Instance, Selector,
            keygen_pk2_consuming_with_profile,
        },
        poly::{
            Rotation, VerificationStrategy,
            commitment::ParamsProver as _,
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::{ProverIPA, VerifierIPA},
                strategy::SingleStrategy,
            },
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };

    use super::*;
    use crate::zk::kagemusha_v1_recursion::{
        composite::{KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1},
        guard_bundle::KagemushaPlatformCredentialRelationCircuitV1,
        mint_authority::{KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1},
        mint_authorization::{
            KagemushaMintAuthorizationEpCircuitV1, KagemushaMintAuthorizationEqCircuitV1,
        },
        mint_hash_claim_fold::{
            KagemushaMintHashClaimEpCircuitV1, KagemushaMintHashClaimEqCircuitV1,
        },
        terminal_authorization::{
            KagemushaTerminalAuthorizationEpCircuitV1, KagemushaTerminalAuthorizationEqCircuitV1,
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
    fn recursive_state_claim_consumer_has_no_k16_sha_auxiliary_columns() {
        // The full typed canonical SHA queue is now authenticated by the ordered claim proof.
        // Empty Base parameters isolate the exact unchanged reciprocal dense-MSM machine; a
        // reintroduced inline Table8 machine would violate this configuration equality.
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
            KagemushaRecursiveStateEqCircuitV1,
        >(16, params.clone())
        .expect("Eq recursive-state auxiliary profile");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaRecursiveStateEpCircuitV1,
        >(16, params.clone())
        .expect("Ep recursive-state auxiliary profile");
        let dense_only = configured_minimum_compressed_key_resources_for_params_v1::<
            EqAffine,
            KagemushaMintAuthorityEqCircuitV1,
        >(16, params)
        .expect("existing dense-only authority auxiliary profile");
        assert_eq!(eq, dense_only);
        assert_eq!(ep, dense_only);
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
        let params = crate::zk::kagemusha_v1_recursion::KagemushaProviderRootCircuitParamsV1::new(
            params, [1; 32],
        )
        .expect("non-authorizing setup root");
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
    fn mint_hash_claim_dense_fixed_rlc_and_native_poseidon_auxiliary_geometry_is_frozen() {
        // The native Poseidon queue adds seven advice, five fixed, and one shared BUS copy
        // column to the existing dense/RLC machines. This auxiliary floor now fits the release
        // PK cap; the separate minimum legal Base test still rejects a complete Claim.
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
        .expect("Eq MintHashClaim dense, fixed-RLC and native Poseidon auxiliary profile");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaMintHashClaimEpCircuitV1,
        >(16, params.clone())
        .expect("Ep MintHashClaim dense, fixed-RLC and native Poseidon auxiliary profile");
        assert_eq!(eq, ep);
        assert!(eq.compress_selectors);
        assert_eq!(eq.advice_columns, 115);
        assert_eq!(eq.instance_columns, 0);
        assert_eq!(eq.configured_fixed_columns, 10);
        assert_eq!(eq.selector_columns, 0);
        assert_eq!(eq.materialized_selector_columns, 0);
        assert_eq!(eq.processed_fixed_columns, 10);
        assert_eq!(eq.permutation_columns, 4);
        assert_eq!(eq.selector_bitmap_bytes, 0);
        assert_eq!(eq.verifying_key_bytes, 458);
        assert_eq!(eq.proving_key_bytes, 65_012_310);
        macro_rules! check_preflight {
            ($curve:ty, $field:ty, $circuit:ty, $parity:expr) => {{
                let mut configured = ConstraintSystem::<$field>::default();
                <$circuit>::configure_with_params(&mut configured, params.clone());
                assert_eq!(configured.degree(), 7);
                assert_eq!(configured.blinding_factors(), 6);
                assert_eq!(configured.minimum_rows(), 9);
                assert_eq!(
                    preflight_helper_key_configuration_v1::<$curve, $circuit>(
                        16,
                        params.clone(),
                        $parity,
                        "claim native Poseidon auxiliary release floor",
                    )
                    .expect("shared BUS auxiliary geometry fits both unchanged release caps"),
                    eq
                );
                assert_eq!(
                    preflight_key_configuration_with_limits_v1::<$curve, $circuit>(
                        16,
                        params.clone(),
                        $parity,
                        "claim native Poseidon auxiliary guarded floor",
                        KagemushaProcessedKeyLimitsV1::guarded_real_proof(),
                    )
                    .expect("existing guarded envelope permits full-graph measurement"),
                    eq
                );
            }};
        }
        check_preflight!(
            EqAffine,
            Fp,
            KagemushaMintHashClaimEqCircuitV1,
            KagemushaPastaParityV1::Eq
        );
        check_preflight!(
            EpAffine,
            Fq,
            KagemushaMintHashClaimEpCircuitV1,
            KagemushaPastaParityV1::Ep
        );
    }

    #[test]
    fn mint_hash_claim_minimum_legal_base_configuration_exceeds_release_key_budget() {
        // The claim always has native arithmetic, range checks, constants, and three public
        // columns. Configure only one gate and one lookup request as a strict lower bound.
        // Base's one-gate optimization reuses its gate advice for the lookup, so counting a
        // separate lookup advice/copy column here would overstate that lower bound.
        let params = BaseCircuitParams {
            k: 16,
            num_advice_per_phase: vec![1],
            num_fixed: 1,
            num_lookup_advice_per_phase: vec![1],
            lookup_bits: Some(15),
            num_instance_columns: 3,
        };
        let eq = configured_minimum_compressed_key_resources_for_params_v1::<
            EqAffine,
            KagemushaMintHashClaimEqCircuitV1,
        >(16, params.clone())
        .expect("configure actual Eq claim with minimum legal Base");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaMintHashClaimEpCircuitV1,
        >(16, params)
        .expect("configure actual Ep claim with minimum legal Base");
        assert_eq!(eq, ep);
        assert_eq!(eq.advice_columns, 116);
        assert_eq!(eq.instance_columns, 3);
        assert_eq!(eq.configured_fixed_columns, 11);
        assert_eq!(eq.selector_columns, 2);
        assert_eq!(eq.materialized_selector_columns, 2);
        assert_eq!(eq.processed_fixed_columns, 13);
        assert_eq!(eq.permutation_columns, 9);
        assert_eq!(eq.proving_key_bytes, 98_583_446);
        assert_eq!(eq.verifying_key_bytes, 17_098);
        assert!(eq.proving_key_bytes > KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1);
        assert!(eq.verifying_key_bytes <= KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1);

        // The shared Base/RLC range table remains shared. Even an impossible removal of both
        // selectors leaves the native Poseidon and minimum Base fixed/copy inventory above the
        // release budget; direct selector encoding cannot close that independent floor.
        let without_any_selectors =
            10_u64 + 32 * (11 + 9) + 16 + (2 * (11 + 9) + 3) * (32 * (1 << 16) + 4);
        assert_eq!(without_any_selectors, 90_178_374);
        assert!(without_any_selectors > KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1);
        for (parity, profile) in [
            (KagemushaPastaParityV1::Eq, eq),
            (KagemushaPastaParityV1::Ep, ep),
        ] {
            assert!(matches!(
                enforce_helper_key_limits_v1(
                    parity,
                    "mint-hash claim minimum legal Base",
                    profile,
                    KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
                    KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
                ),
                Err(KagemushaArtifactGenerationErrorV1::PredictedKeyResourceLimit { .. })
            ));
            let diagnostic_limits = KagemushaProcessedKeyLimitsV1::guarded_real_proof();
            enforce_helper_key_limits_v1(
                parity,
                "diagnostic mint-hash claim minimum legal Base",
                profile,
                diagnostic_limits.proving_key_maximum,
                diagnostic_limits.verifying_key_maximum,
            )
            .expect("minimum configuration does not exclude guarded diagnostic measurement");
        }
        eprintln!("KAGEMUSHA minimum legal claim configuration: {eq:?}");
    }

    #[test]
    fn terminal_auxiliary_configuration_has_only_dense_geometry_in_both_parities() {
        // Configure the actual Terminal types without a witness or scalar graph. Its entire
        // original SHA queue is now consumed by the mandatory complete-claim verifier.
        let params = super::super::auxiliary_only_k16_base_params_v1();
        let eq = configured_minimum_compressed_key_resources_for_params_v1::<
            EqAffine,
            KagemushaTerminalAuthorizationEqCircuitV1,
        >(16, params.clone())
        .expect("Eq terminal auxiliary profile");
        let ep = configured_minimum_compressed_key_resources_for_params_v1::<
            EpAffine,
            KagemushaTerminalAuthorizationEpCircuitV1,
        >(16, params.clone())
        .expect("Ep terminal auxiliary profile");
        assert_eq!(eq, ep);
        // Four dense lanes remain, with one shared fixed schedule and four BUS copies.
        // No Table8 advice, selectors, selector bitmaps, fixed tables or copies remain.
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
        assert_eq!(KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1, 64 * 1024 * 1024);
        assert_eq!(KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1, 64 * 1024);
        assert!(eq.proving_key_bytes <= KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1);
        assert!(eq.verifying_key_bytes <= KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1);
        macro_rules! check_parity {
            ($curve:ty, $circuit:ty, $parity:expr, $profile:expr) => {{
                let checked = preflight_helper_key_configuration_v1::<$curve, $circuit>(
                    16,
                    params.clone(),
                    $parity,
                    "terminal authorization auxiliary geometry",
                )
                .expect("fixed dense auxiliary floor fits the unchanged key envelope");
                assert_eq!(checked, $profile);

                // Passing the auxiliary floor never authorizes an oversized final Base graph.
                // This valid Base configuration already needs too many permutation columns,
                // even before selector materialization; the same release caps must reject it.
                let oversized = BaseCircuitParams {
                    k: 16,
                    num_advice_per_phase: vec![32],
                    num_fixed: 1,
                    num_lookup_advice_per_phase: vec![1],
                    lookup_bits: Some(15),
                    num_instance_columns: 1,
                };
                assert!(matches!(
                    preflight_helper_key_configuration_v1::<$curve, $circuit>(
                        16, oversized, $parity, "terminal authorization Base geometry",
                    ),
                    Err(KagemushaArtifactGenerationErrorV1::PredictedKeyResourceLimit {
                        parity,
                        predicted_proving_key_bytes,
                        proving_key_maximum: KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1,
                        verifying_key_maximum: KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
                        ..
                    }) if parity == $parity
                        && predicted_proving_key_bytes > KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1
                ));
            }};
        }
        check_parity!(
            EqAffine,
            KagemushaTerminalAuthorizationEqCircuitV1,
            KagemushaPastaParityV1::Eq,
            eq
        );
        check_parity!(
            EpAffine,
            KagemushaTerminalAuthorizationEpCircuitV1,
            KagemushaPastaParityV1::Ep,
            ep
        );
    }

    #[test]
    fn terminal_generator_auxiliary_preflight_requires_no_scalar_graph() {
        // Production still checks the actual fixed auxiliary geometry before graph allocation.
        // This floor fits; it does not establish complete graph, witness, or mobile RSS capacity.
        super::super::preflight_kagemusha_terminal_authorization_key_configuration_v1()
            .expect("actual dense-only terminal auxiliary floor fits unchanged release limits");
    }

    #[test]
    fn small_k6_prediction_matches_real_processed_keys_in_both_parities() {
        macro_rules! check_parity {
            ($curve:ty, $scalar:ty, $parity:expr) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let circuit = SmallProcessedKeyCircuit::<$scalar>::default();
                let upper_bound =
                    configured_compressed_key_resources_v1::<$curve, _>(6, &circuit, false)
                        .expect("small compressed resource upper bound");
                let lower_bound =
                    configured_compressed_key_resources_v1::<$curve, _>(6, &circuit, true)
                        .expect("small compressed resource lower bound");
                assert_eq!(
                    preflight_helper_key_configuration_v1::<
                        $curve,
                        SmallProcessedKeyCircuit<$scalar>,
                    >(6, (), $parity, "small test auxiliary configuration",),
                    Ok(lower_bound),
                    "parameter-only preflight must match the actual small circuit configuration"
                );
                assert_eq!(
                    preflight_helper_key_resources_v1(
                        &params,
                        &circuit,
                        $parity,
                        "small test circuit",
                    ),
                    Ok(upper_bound)
                );
                let verifying_key = keygen_vk_with_helper_resource_preflight_v1(
                    &params,
                    &circuit,
                    $parity,
                    "small test circuit",
                    "small test verifying key",
                )
                .expect("guarded small VK");
                let (proving_key, profile) = keygen_pk2_consuming_with_profile(
                    &params,
                    circuit,
                    true,
                    |_circuit, synthesized| exact_processed_key_resources_v1::<$curve>(synthesized),
                )
                .expect("small PK and exact synthesized resource profile");
                assert_eq!(
                    proving_key.get_vk().to_bytes(SerdeFormat::Processed),
                    verifying_key.to_bytes(SerdeFormat::Processed),
                    "profile extraction must preserve the guarded verifier key"
                );
                // Configure-only preflight cannot see these two selectors' disjoint rows. Its
                // conservative two-column estimate must bound the actual one-column compression.
                assert_eq!(upper_bound.materialized_selector_columns, 2);
                assert_eq!(lower_bound.materialized_selector_columns, 1);
                assert!(lower_bound.verifying_key_bytes <= profile.verifying_key_bytes);
                assert!(profile.verifying_key_bytes < upper_bound.verifying_key_bytes);
                assert!(lower_bound.proving_key_bytes <= profile.proving_key_bytes);
                assert!(profile.proving_key_bytes < upper_bound.proving_key_bytes);
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
    #[derive(Clone)]
    struct SelectorEncodingCircuit<F, const OVERLAP: bool> {
        syntheses: Arc<std::sync::atomic::AtomicUsize>,
        marker: PhantomData<F>,
    }

    impl<F: PrimeField, const OVERLAP: bool> Circuit<F> for SelectorEncodingCircuit<F, OVERLAP> {
        type Config = <SmallProcessedKeyCircuit<F> as Circuit<F>>::Config;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            self.clone()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            SmallProcessedKeyCircuit::<F>::configure(meta)
        }

        fn synthesize(
            &self,
            (advice, instance, _, first, second): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            self.syntheses.fetch_add(1, Ordering::SeqCst);
            let cell = layouter.assign_region(
                || "adaptive selector rows",
                |mut region| {
                    first.enable(&mut region, 0)?;
                    second.enable(&mut region, if OVERLAP { 0 } else { 1 })?;
                    let cell = region
                        .assign_advice(advice, 0, Value::known(F::ZERO))
                        .cell();
                    region.assign_advice(advice, 1, Value::known(F::ZERO));
                    Ok(cell)
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct MixedDegreeSelectorCircuit<F>(PhantomData<F>);

    const MIXED_DEGREE_SELECTOR_ROWS: [&[usize]; 6] = [&[0, 1], &[0], &[1], &[2], &[3], &[4]];

    impl<F: PrimeField> Circuit<F> for MixedDegreeSelectorCircuit<F> {
        type Config = (Column<Advice>, [Selector; 6]);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            self.clone()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            meta.set_minimum_degree(5);
            let advice = meta.advice_column();
            let selectors = std::array::from_fn(|_| meta.selector());
            for (selector, degree) in selectors.into_iter().zip([4, 2, 2, 4, 2, 2]) {
                meta.create_gate("mixed-degree selector sizing", |meta| {
                    let value = meta.query_advice(advice, Rotation::cur());
                    let mut constraint = meta.query_selector(selector);
                    for _ in 1..degree {
                        constraint = constraint * value.clone();
                    }
                    vec![constraint]
                });
            }
            (advice, selectors)
        }

        fn synthesize(
            &self,
            (advice, selectors): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            layouter.assign_region(
                || "mixed-degree selector overlap",
                |mut region| {
                    for row in 0..5 {
                        region.assign_advice(advice, row, Value::known(F::ZERO));
                    }
                    for (selector, rows) in selectors.iter().zip(MIXED_DEGREE_SELECTOR_ROWS) {
                        for &row in rows {
                            selector.enable(&mut region, row)?;
                        }
                    }
                    Ok(())
                },
            )
        }
    }

    #[test]
    fn adaptive_selector_lower_bound_handles_independent_and_degree_thresholds() {
        fn check<F: PrimeField>() {
            let empty = ConstraintSystem::<F>::default();
            assert_eq!(empty.minimum_compressed_selector_columns(), 0);
            assert_eq!(empty.compress_selectors(vec![]).1.len(), 0);

            let mut independent = ConstraintSystem::<F>::default();
            let _ = independent.complex_selector();
            let _ = independent.selector(); // Unused simple selectors also retain degree zero.
            assert_eq!(independent.minimum_compressed_selector_columns(), 2);
            assert_eq!(
                independent.compress_selectors(vec![vec![false]; 2]).1.len(),
                2
            );

            // The highest-degree threshold must retain its stronger bound even when lower-degree
            // selectors would permit larger groups. Degree-zero columns add independently.
            for (degrees, expected, actual) in [(vec![5, 5, 5], 4, 4), (vec![4, 4, 4, 2, 2], 3, 4)]
            {
                let mut meta = ConstraintSystem::<F>::default();
                meta.set_minimum_degree(5);
                let advice = meta.advice_column();
                let complex = meta.complex_selector();
                meta.create_gate("independent complex selector", |meta| {
                    vec![meta.query_selector(complex) * meta.query_advice(advice, Rotation::cur())]
                });
                for degree in degrees {
                    let selector = meta.selector();
                    meta.create_gate("selector degree threshold", |meta| {
                        let value = meta.query_advice(advice, Rotation::cur());
                        let mut constraint = meta.query_selector(selector);
                        for _ in 1..degree {
                            constraint = constraint * value.clone();
                        }
                        vec![constraint]
                    });
                }
                assert_eq!(meta.degree(), 5);
                assert_eq!(meta.minimum_compressed_selector_columns(), expected);
                let activations = vec![vec![false]; meta.num_selectors()];
                assert_eq!(meta.compress_selectors(activations).1.len(), actual);
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn adaptive_mixed_degree_overlap_does_not_false_reject_feasible_keys() {
        macro_rules! check {
            ($curve:ty, $field:ty, $parity:expr) => {{
                let mut meta = ConstraintSystem::<$field>::default();
                let _ = MixedDegreeSelectorCircuit::<$field>::configure(&mut meta);
                assert_eq!(meta.degree(), 5);
                assert_eq!(meta.minimum_compressed_selector_columns(), 2);
                // Greedy packing is not monotone in overlap: no overlaps gives [0,1], [2,3],
                // [4,5], while only overlaps (0,1) and (0,2) give [0,3], [1,2,4,5].
                let inactive_count = meta
                    .clone()
                    .compress_selectors(vec![vec![false]; 6])
                    .1
                    .len();
                assert_eq!(inactive_count, 3);
                let activations = MIXED_DEGREE_SELECTOR_ROWS
                    .iter()
                    .map(|rows| (0..5).map(|row| rows.contains(&row)).collect())
                    .collect();
                assert_eq!(meta.clone().compress_selectors(activations).1.len(), 2);

                let configured = configured_key_encoding_profiles_for_params_v1::<
                    $curve,
                    MixedDegreeSelectorCircuit<$field>,
                >(6, ())
                .unwrap();
                assert_eq!(configured[0].materialized_selector_columns, 2);
                assert_eq!(configured[1].materialized_selector_columns, 6);
                let limits = KagemushaProcessedKeyLimitsV1 {
                    proving_key_maximum: configured[0].proving_key_bytes,
                    verifying_key_maximum: configured[0].verifying_key_bytes,
                };
                let inactive_profile = predict_processed_key_resources_with_selectors_v1(
                    6,
                    meta.num_advice_columns(),
                    meta.num_instance_columns(),
                    meta.num_fixed_columns(),
                    meta.num_selectors(),
                    inactive_count,
                    meta.permutation().get_columns().len(),
                    true,
                )
                .unwrap();
                for rejected in [inactive_profile, configured[1]] {
                    assert!(
                        enforce_helper_key_limits_v1(
                            $parity,
                            "mixed-degree infeasible encoding",
                            rejected,
                            limits.proving_key_maximum,
                            limits.verifying_key_maximum,
                        )
                        .is_err()
                    );
                }
                assert_eq!(
                    preflight_key_configuration_with_limits_v1::<
                        $curve,
                        MixedDegreeSelectorCircuit<$field>,
                    >(6, (), $parity, "mixed-degree feasible preflight", limits)
                    .unwrap(),
                    configured[0]
                );
                let params = ParamsIPA::<$curve>::new(6);
                let circuit = MixedDegreeSelectorCircuit::<$field>::default();
                let pk = keygen_pk_with_key_resource_limits_consuming_v1(
                    &params,
                    circuit.clone(),
                    $parity,
                    "mixed-degree feasible preflight",
                    "small mixed-degree PK",
                    limits,
                )
                .unwrap();
                let vk = keygen_vk_with_key_resource_limits_consuming_v1(
                    &params,
                    circuit,
                    $parity,
                    "mixed-degree feasible preflight",
                    "small mixed-degree VK",
                    limits,
                )
                .unwrap();
                assert_eq!(
                    pk.to_bytes(SerdeFormat::Processed).len() as u64,
                    limits.proving_key_maximum
                );
                let vk_bytes = vk.to_bytes(SerdeFormat::Processed);
                assert_eq!(vk_bytes.len() as u64, limits.verifying_key_maximum);
                assert_eq!(vk_bytes[5], 1);
                assert_eq!(pk.get_vk().to_bytes(SerdeFormat::Processed), vk_bytes);
            }};
        }
        check!(EqAffine, Fp, KagemushaPastaParityV1::Eq);
        check!(EpAffine, Fq, KagemushaPastaParityV1::Ep);
    }

    #[test]
    fn adaptive_key_encoding_checks_both_limits_and_stable_ties() {
        // Sixteen selectors compressed pairwise save eight fixed polynomials, but at k16 their
        // bitmaps alone exceed the VK limit. Direct conversion instead exceeds the PK limit.
        let compressed =
            predict_processed_key_resources_with_selectors_v1(16, 1, 1, 1, 16, 8, 2, true).unwrap();
        let direct =
            predict_processed_key_resources_with_selectors_v1(16, 1, 1, 1, 16, 16, 2, false)
                .unwrap();
        let release = KagemushaProcessedKeyLimitsV1::release();
        assert!(compressed.proving_key_bytes < release.proving_key_maximum);
        assert!(compressed.verifying_key_bytes > release.verifying_key_maximum);
        assert!(direct.proving_key_bytes > release.proving_key_maximum);
        assert!(direct.verifying_key_bytes < release.verifying_key_maximum);
        for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
            assert!(
                choose_helper_key_encoding_v1(
                    [compressed, direct],
                    parity,
                    "conflicting encoding limits",
                    release,
                )
                .is_err()
            );
            // Each alternative is selected when it alone fits both exact boundaries.
            for expected in [compressed, direct] {
                let limits = KagemushaProcessedKeyLimitsV1 {
                    proving_key_maximum: expected.proving_key_bytes,
                    verifying_key_maximum: expected.verifying_key_bytes,
                };
                assert_eq!(
                    choose_helper_key_encoding_v1(
                        [compressed, direct],
                        parity,
                        "exact encoding boundary",
                        limits,
                    )
                    .unwrap(),
                    expected
                );
                for rejected in [
                    KagemushaProcessedKeyLimitsV1 {
                        proving_key_maximum: limits.proving_key_maximum - 1,
                        ..limits
                    },
                    KagemushaProcessedKeyLimitsV1 {
                        verifying_key_maximum: limits.verifying_key_maximum - 1,
                        ..limits
                    },
                ] {
                    assert!(
                        choose_helper_key_encoding_v1(
                            [compressed, direct],
                            parity,
                            "encoding boundary minus one",
                            rejected,
                        )
                        .is_err()
                    );
                }
            }
            let compressed =
                predict_processed_key_resources_with_selectors_v1(6, 1, 1, 1, 0, 0, 2, true)
                    .unwrap();
            let direct =
                predict_processed_key_resources_with_selectors_v1(6, 1, 1, 1, 0, 0, 2, false)
                    .unwrap();
            assert_eq!(
                choose_helper_key_encoding_v1(
                    [direct, compressed],
                    parity,
                    "stable no-selector tie",
                    release,
                )
                .unwrap(),
                compressed
            );
        }
    }

    #[test]
    fn adaptive_configuration_preflight_keeps_a_direct_encoding_candidate() {
        macro_rules! check {
            ($curve:ty, $field:ty, $parity:expr) => {{
                let profiles = configured_key_encoding_profiles_for_params_v1::<
                    $curve,
                    SelectorEncodingCircuit<$field, true>,
                >(9, ())
                .unwrap();
                let limits = KagemushaProcessedKeyLimitsV1 {
                    proving_key_maximum: profiles[1].proving_key_bytes,
                    verifying_key_maximum: profiles[1].verifying_key_bytes,
                };
                assert!(profiles[0].verifying_key_bytes > limits.verifying_key_maximum);
                assert_eq!(
                    preflight_key_configuration_with_limits_v1::<
                        $curve,
                        SelectorEncodingCircuit<$field, true>,
                    >(9, (), $parity, "direct-only configure preflight", limits)
                    .unwrap(),
                    profiles[1]
                );
            }};
        }
        check!(EqAffine, Fp, KagemushaPastaParityV1::Eq);
        check!(EpAffine, Fq, KagemushaPastaParityV1::Ep);
    }

    #[test]
    fn adaptive_consuming_keys_match_actual_serialization_in_both_fields() {
        macro_rules! check {
            ($curve:ty, $field:ty, $parity:expr, $overlap:literal) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let syntheses = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let circuit = SelectorEncodingCircuit::<$field, $overlap> {
                    syntheses: Arc::clone(&syntheses),
                    marker: PhantomData,
                };
                let (pk, profile) = keygen_pk2_consuming_with_selector_choice(
                    &params,
                    circuit.clone(),
                    |_circuit, profiles| {
                        assert_eq!(
                            profiles.compressed.materialized_selector_columns,
                            if $overlap { 2 } else { 1 }
                        );
                        assert_eq!(profiles.direct.materialized_selector_columns, 2);
                        choose_synthesized_helper_key_encoding_v1::<$curve>(
                            profiles,
                            $parity,
                            "exact adaptive small key",
                            KagemushaProcessedKeyLimitsV1::release(),
                        )
                    },
                )
                .expect("adaptive combined key from one synthesis");
                assert_eq!(syntheses.load(Ordering::SeqCst), 1);
                assert_eq!(profile.compress_selectors, !$overlap);
                let pk_bytes = pk.to_bytes(SerdeFormat::Processed);
                let vk_bytes = pk.get_vk().to_bytes(SerdeFormat::Processed);
                assert_eq!(pk_bytes.len() as u64, profile.proving_key_bytes);
                assert_eq!(vk_bytes.len() as u64, profile.verifying_key_bytes);
                assert_eq!(vk_bytes[5], u8::from(profile.compress_selectors));
                let (vk, vk_profile) = keygen_vk_consuming_with_selector_choice(
                    &params,
                    circuit.clone(),
                    |_circuit, profiles| {
                        choose_synthesized_helper_key_encoding_v1::<$curve>(
                            profiles,
                            $parity,
                            "exact adaptive small VK",
                            KagemushaProcessedKeyLimitsV1::release(),
                        )
                    },
                )
                .expect("adaptive VK from one additional synthesis");
                assert_eq!(syntheses.load(Ordering::SeqCst), 2);
                assert_eq!(vk_profile, profile);
                assert_eq!(vk.to_bytes(SerdeFormat::Processed), vk_bytes);
                // Both legacy explicit encodings remain available and byte-identical to their
                // selected adaptive result; no caller default was changed in Halo2.
                let reference =
                    halo2_proofs::plonk::keygen_pk2(&params, &circuit, profile.compress_selectors)
                        .unwrap();
                assert_eq!(reference.to_bytes(SerdeFormat::Processed), pk_bytes);
                let generated = keygen_pk_with_helper_resource_preflight_consuming_v1(
                    &params,
                    circuit.clone(),
                    $parity,
                    "adaptive production wrapper",
                    "small PK",
                )
                .unwrap();
                assert_eq!(generated.to_bytes(SerdeFormat::Processed), pk_bytes);
                let rebuilt = keygen_vk_with_helper_resource_preflight_consuming_v1(
                    &params,
                    circuit.clone(),
                    $parity,
                    "adaptive production wrapper",
                    "small VK",
                )
                .unwrap();
                assert_eq!(rebuilt.to_bytes(SerdeFormat::Processed), vk_bytes);
                let mut input = pk_bytes.as_slice();
                let restored_pk = ProvingKey::<$curve>::read_checked::<
                    _,
                    SelectorEncodingCircuit<$field, $overlap>,
                >(&mut input, SerdeFormat::Processed, 6, ())
                .unwrap();
                assert!(input.is_empty());
                assert_eq!(restored_pk.to_bytes(SerdeFormat::Processed), pk_bytes);
                let mut input = vk_bytes.as_slice();
                let restored_vk = VerifyingKey::<$curve>::read_checked::<
                    _,
                    SelectorEncodingCircuit<$field, $overlap>,
                >(&mut input, SerdeFormat::Processed, 6, ())
                .unwrap();
                assert!(input.is_empty());
                assert_eq!(restored_vk.to_bytes(SerdeFormat::Processed), vk_bytes);
                // Exercise both checked reloaded keys through an actual small IPA proof.
                let public = [<$field>::ZERO];
                let columns: [&[$field]; 1] = [&public];
                let mut transcript =
                    Blake2bWrite::<_, $curve, Challenge255<$curve>>::init(Vec::new());
                halo2_proofs::plonk::create_proof::<
                    IPACommitmentScheme<$curve>,
                    ProverIPA<'_, $curve>,
                    _,
                    _,
                    _,
                    _,
                >(
                    &params,
                    &restored_pk,
                    &[circuit],
                    &[&columns],
                    rand_core_06::OsRng,
                    &mut transcript,
                )
                .expect("genuine proof with checked adaptive PK");
                let proof = transcript.finalize();
                let verify = |public: &[$field]| {
                    let columns: [&[$field]; 1] = [public];
                    let mut transcript =
                        Blake2bRead::<_, $curve, Challenge255<$curve>>::init(proof.as_slice());
                    halo2_proofs::plonk::verify_proof::<
                        IPACommitmentScheme<$curve>,
                        VerifierIPA<'_, $curve>,
                        _,
                        _,
                        _,
                    >(
                        &params,
                        &restored_vk,
                        SingleStrategy::<$curve>::new(&params),
                        &[&columns],
                        &mut transcript,
                    )
                };
                verify(&public).expect("genuine proof with checked adaptive VK");
                assert!(
                    verify(&[<$field>::ONE]).is_err(),
                    "wrong public value must fail"
                );
                let mut bad_flag = vk_bytes.clone();
                bad_flag[5] ^= 1;
                assert!(
                    VerifyingKey::<$curve>::read_checked::<
                        _,
                        SelectorEncodingCircuit<$field, $overlap>,
                    >(&mut bad_flag.as_slice(), SerdeFormat::Processed, 6, ())
                    .is_err()
                );
            }};
        }
        check!(EqAffine, Fp, KagemushaPastaParityV1::Eq, false);
        check!(EqAffine, Fp, KagemushaPastaParityV1::Eq, true);
        check!(EpAffine, Fq, KagemushaPastaParityV1::Ep, false);
        check!(EpAffine, Fq, KagemushaPastaParityV1::Ep, true);
    }

    #[test]
    fn adaptive_consuming_keygen_rejects_exact_overlap_before_expansion() {
        macro_rules! check {
            ($curve:ty, $field:ty, $parity:expr) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let profiles = configured_key_encoding_profiles_for_params_v1::<
                    $curve,
                    SelectorEncodingCircuit<$field, true>,
                >(6, ())
                .unwrap();
                let limits = KagemushaProcessedKeyLimitsV1 {
                    proving_key_maximum: profiles[0].proving_key_bytes,
                    verifying_key_maximum: KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
                };
                assert!(
                    preflight_key_configuration_with_limits_v1::<
                        $curve,
                        SelectorEncodingCircuit<$field, true>,
                    >(6, (), $parity, "optimistic overlap preflight", limits)
                    .is_ok()
                );
                let syntheses = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let circuit = SelectorEncodingCircuit::<$field, true> {
                    syntheses: Arc::clone(&syntheses),
                    marker: PhantomData,
                };
                for vk_only in [false, true] {
                    let failed = if vk_only {
                        keygen_vk_with_key_resource_limits_consuming_v1(
                            &params,
                            circuit.clone(),
                            $parity,
                            "exact overlap",
                            "small VK",
                            limits,
                        )
                        .map(|_| ())
                    } else {
                        keygen_pk_with_key_resource_limits_consuming_v1(
                            &params,
                            circuit.clone(),
                            $parity,
                            "exact overlap",
                            "small PK",
                            limits,
                        )
                        .map(|_| ())
                    };
                    assert!(matches!(
                        failed,
                        Err(KagemushaArtifactGenerationErrorV1::PredictedKeyResourceLimit { .. })
                    ));
                }
                assert_eq!(syntheses.load(Ordering::SeqCst), 2);
            }};
        }
        check!(EqAffine, Fp, KagemushaPastaParityV1::Eq);
        check!(EpAffine, Fq, KagemushaPastaParityV1::Ep);
    }
    #[test]
    fn adaptive_phase12_geometry_removes_only_bitmaps_and_remains_over_limit() {
        let compressed =
            predict_processed_key_resources_with_selectors_v1(16, 234, 3, 6, 118, 118, 133, true)
                .unwrap();
        let direct =
            predict_processed_key_resources_with_selectors_v1(16, 234, 3, 6, 118, 118, 133, false)
                .unwrap();
        assert_eq!(
            compressed.processed_fixed_columns,
            direct.processed_fixed_columns
        );
        assert_eq!(compressed.permutation_columns, direct.permutation_columns);
        assert_eq!(compressed.selector_bitmap_bytes, 966_656);
        assert_eq!(direct.selector_bitmap_bytes, 0);
        assert_eq!(compressed.verifying_key_bytes, 974_890);
        assert_eq!(direct.verifying_key_bytes, 8_234);
        assert_eq!(compressed.proving_key_bytes, 1_085_204_558);
        assert_eq!(direct.proving_key_bytes, 1_084_237_902);
        assert_eq!(
            compressed.proving_key_bytes - direct.proving_key_bytes,
            966_656
        );
        for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
            for limits in [
                KagemushaProcessedKeyLimitsV1::release(),
                KagemushaProcessedKeyLimitsV1::guarded_real_proof(),
            ] {
                assert!(
                    choose_helper_key_encoding_v1(
                        [compressed, direct],
                        parity,
                        "actual phase12 claim geometry",
                        limits,
                    )
                    .is_err(),
                    "changing serialization cannot close the full claim PK budget"
                );
            }
        }
    }
}
