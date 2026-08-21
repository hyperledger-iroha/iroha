//! Serialized advice-column binding for the review-blocked Kagemusha V7 audit join.
//!
//! This module does not weaken or replace the reciprocal dense-MSM identity.
//! It copy-binds the two frozen audit vectors of one physical proof into one
//! phase-zero advice column.  The column's ordinary Halo2 IPA commitment is
//! then the pre-challenge vector binding used by the atomic Eq/Ep pair.
//!
//! The V7 release gate remains closed.  In particular, callers must not accept
//! one proof half without validating the selected advice commitments and all
//! shared public join cells against its sibling proof.

use ff::PrimeField;
use halo2_base::{
    AssignedValue, Context,
    halo2_proofs::{
        circuit::{Cell, Layouter, Value},
        halo2curves::{CurveAffine, group::prime::PrimeCurveAffine},
        plonk::{Advice, Column, ConstraintSystem, Error},
    },
    utils::BigPrimeField,
    virtual_region::copy_constraints::SharedCopyConstraintManager,
};
use iroha_data_model::offline::KagemushaPastaCycleParityV1;
use sha2::{Digest as _, Sha256};

use super::kagemusha_sha256_v4::{
    KagemushaSha256BitV4, KagemushaSha256ByteV4, KagemushaSha256JobsV4,
};

const SERIALIZED_AUDIT_SCHEMA_V7: u64 = u64::from_le_bytes(*b"kgser007");
const NATIVE_SCALAR_ENCODING_V7: u64 = u64::from_le_bytes(*b"native01");
const RECIPROCAL_SCALAR_ENCODING_V7: u64 = u64::from_le_bytes(*b"2x128le1");
const ZERO_PADDING_ENCODING_V7: u64 = u64::from_le_bytes(*b"zero-pad");
const SERIALIZED_HEADER_CELLS_V7: usize = 10;
const SERIALIZED_AUDIT_PROFILE_VERSION_V7: u16 = 7;
const SERIALIZED_AUDIT_PROFILE_DOMAIN_V7: &[u8] = b"iroha:kagemusha:v7:serialized-advice-profile";
const AUDIT_PAIR_CHALLENGE_DOMAIN_V7: &[u8] = b"iroha:kagemusha:v7:serialized-audit-pair-challenge";
const AUDIT_PARENT_SLOTS_DOMAIN_V7: &[u8] = b"iroha:kagemusha:v7:serialized-audit-parent-slots";
const AUDIT_PARENT_SLOT_COUNT_V7: usize = 2;
const AUDIT_CURRENT_JOIN_CELLS_V7: usize = 10;
const AUDIT_PARENT_DIGEST_CELLS_V7: usize = 2;
const SERIALIZED_EXPECTED_FIXED_COLUMNS_V7: u32 = 339;
const SERIALIZED_EXPECTED_PERMUTATION_COLUMNS_V7: u32 = 298;
const SERIALIZED_EXPECTED_BLINDING_FACTORS_V7: u32 = 8;
pub(super) const SERIALIZED_EXPECTED_UNUSABLE_ROWS_V7: u32 = 9;
pub(super) const SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7: u32 = 93_184;

mod sealed_field_v7 {
    pub trait Sealed {}

    impl Sealed for halo2_base::halo2_proofs::halo2curves::pasta::Fp {}
    impl Sealed for halo2_base::halo2_proofs::halo2curves::pasta::Fq {}
}

/// Sealed physical-field and opposite-scalar mapping for one Pasta proof half.
pub(super) trait KagemushaSerializedAuditFieldV7:
    BigPrimeField + ff::WithSmallOrderMulGroup<3> + sealed_field_v7::Sealed
{
    /// Exact scalar field represented reciprocally in this physical field.
    type ReciprocalScalar: BigPrimeField;
    /// Field-specific serialization header tag.
    const PHYSICAL_FIELD_TAG: u64;
    /// Target vector represented by native scalar cells.
    const NATIVE_TARGET: KagemushaPastaCycleParityV1;
    /// Target vector represented by two canonical little-endian u128 chunks.
    const RECIPROCAL_TARGET: KagemushaPastaCycleParityV1;
    /// Low little-endian half of this field's canonical modulus.
    const MODULUS_LOW_U128: u128;
    /// High little-endian half of this field's canonical modulus.
    const MODULUS_HIGH_U128: u128;
}

impl KagemushaSerializedAuditFieldV7 for halo2_base::halo2_proofs::halo2curves::pasta::Fp {
    type ReciprocalScalar = halo2_base::halo2_proofs::halo2curves::pasta::Fq;
    const PHYSICAL_FIELD_TAG: u64 = u64::from_le_bytes(*b"serialfp");
    const NATIVE_TARGET: KagemushaPastaCycleParityV1 = KagemushaPastaCycleParityV1::StepEq;
    const RECIPROCAL_TARGET: KagemushaPastaCycleParityV1 = KagemushaPastaCycleParityV1::StepEp;
    const MODULUS_LOW_U128: u128 =
        (0x2246_98fc_u128 << 96) | (0x094c_f91b_u128 << 64) | (0x992d_30ed_u128 << 32) | 1;
    const MODULUS_HIGH_U128: u128 = 0x4000_0000_0000_0000_0000_0000_0000_0000;
}

impl KagemushaSerializedAuditFieldV7 for halo2_base::halo2_proofs::halo2curves::pasta::Fq {
    type ReciprocalScalar = halo2_base::halo2_proofs::halo2curves::pasta::Fp;
    const PHYSICAL_FIELD_TAG: u64 = u64::from_le_bytes(*b"serialfq");
    const NATIVE_TARGET: KagemushaPastaCycleParityV1 = KagemushaPastaCycleParityV1::StepEp;
    const RECIPROCAL_TARGET: KagemushaPastaCycleParityV1 = KagemushaPastaCycleParityV1::StepEq;
    const MODULUS_LOW_U128: u128 =
        (0x2246_98fc_u128 << 96) | (0x0994_a8dd_u128 << 64) | (0x8c46_eb21_u128 << 32) | 1;
    const MODULUS_HIGH_U128: u128 = 0x4000_0000_0000_0000_0000_0000_0000_0000;
}

fn parity_tag_v7(parity: KagemushaPastaCycleParityV1) -> u64 {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => 1,
        KagemushaPastaCycleParityV1::StepEp => 2,
    }
}

/// Exact authenticated coefficient count for one target audit vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaSerializedAuditProfileV7 {
    coefficient_count: usize,
}

/// Canonical profile hashed into every V7 commitment challenge and parent join.
///
/// This is deliberately more specific than the Base-circuit parameter carrier:
/// it authenticates the exact proving domains, both compiled protocols, the
/// phase-zero serialization-column positions, the one-proof RNG schedule, and
/// the new 70-cell recursive public layout. A caller cannot select any of
/// these values independently after the two vector commitments are known.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct KagemushaSerializedAuditManifestV7 {
    /// Halo2 domain exponent shared by both Pasta halves.
    pub(super) k: u32,
    /// SHA-256 of the exact Eq `ParamsIPA` encoding.
    pub(super) step_eq_params_sha256: [u8; 32],
    /// SHA-256 of the exact Ep `ParamsIPA` encoding.
    pub(super) step_ep_params_sha256: [u8; 32],
    /// SHA-256 of the exact compiled Eq protocol and verifying key.
    pub(super) step_eq_vk_sha256: [u8; 32],
    /// SHA-256 of the exact compiled Ep protocol and verifying key.
    pub(super) step_ep_vk_sha256: [u8; 32],
    /// Complete Eq advice-column phase vector in global-column order.
    pub(super) step_eq_advice_phases: Vec<u8>,
    /// Complete Ep advice-column phase vector in global-column order.
    pub(super) step_ep_advice_phases: Vec<u8>,
    /// Global Eq advice-column index of the serialization column.
    pub(super) step_eq_serialized_column: u32,
    /// Global Ep advice-column index of the serialization column.
    pub(super) step_ep_serialized_column: u32,
    /// Rank of that Eq column among phase-zero witness commitments.
    pub(super) step_eq_phase_zero_rank: u32,
    /// Rank of that Ep column among phase-zero witness commitments.
    pub(super) step_ep_phase_zero_rank: u32,
    /// Exact Eq constraint-system degree.
    pub(super) step_eq_constraint_degree: u32,
    /// Exact Ep constraint-system degree.
    pub(super) step_ep_constraint_degree: u32,
    /// Exact Eq fixed-polynomial count after selector compression.
    pub(super) step_eq_fixed_columns: u32,
    /// Exact Ep fixed-polynomial count after selector compression.
    pub(super) step_ep_fixed_columns: u32,
    /// Exact Eq copy-permutation column count.
    pub(super) step_eq_permutation_columns: u32,
    /// Exact Ep copy-permutation column count.
    pub(super) step_ep_permutation_columns: u32,
    /// Exact Eq blinding-factor count; the unusable tail is this plus one.
    pub(super) step_eq_blinding_factors: u32,
    /// Exact Ep blinding-factor count; the unusable tail is this plus one.
    pub(super) step_ep_blinding_factors: u32,
    /// Exact unusable-row tail shared by both proof halves.
    pub(super) minimum_unusable_rows: u32,
    /// Exact augmented StepEq proof transcript length.
    pub(super) step_eq_proof_bytes: u32,
    /// Exact augmented StepEp proof transcript length.
    pub(super) step_ep_proof_bytes: u32,
    /// Exact Eq target-vector coefficient count.
    pub(super) eq_coefficient_count: u32,
    /// Exact Ep target-vector coefficient count.
    pub(super) ep_coefficient_count: u32,
    /// Exact new-profile instance-column length.
    pub(super) public_instance_cells: u32,
    /// First cell of `Ceq/Cep/z/vEq/vEp` in either proof half.
    pub(super) current_join_offset: u32,
    /// First cell of the ordered-parent digest in either proof half.
    pub(super) parent_digest_offset: u32,
    /// Final live/bootstrap selector cell.
    pub(super) live_selector_offset: u32,
}

impl KagemushaSerializedAuditManifestV7 {
    fn validate_phase_column(
        phases: &[u8],
        selected_column: u32,
        expected_phase_zero_rank: u32,
    ) -> Result<(), String> {
        let selected_column = usize::try_from(selected_column)
            .map_err(|_| "Kagemusha V7 serialized-column index does not fit usize".to_owned())?;
        if phases.is_empty()
            || phases.iter().any(|phase| *phase > 2)
            || phases.get(selected_column) != Some(&0)
            || selected_column.checked_add(1) != Some(phases.len())
        {
            return Err(
                "Kagemusha V7 advice phase/serialization-column profile is invalid".to_owned(),
            );
        }
        let rank = phases[..selected_column]
            .iter()
            .filter(|phase| **phase == 0)
            .count();
        if u32::try_from(rank).ok() != Some(expected_phase_zero_rank) {
            return Err("Kagemusha V7 serialized phase-zero rank is invalid".to_owned());
        }
        Ok(())
    }

    /// Validate every field that affects commitment reconstruction or recursion.
    pub(super) fn validate(&self) -> Result<(), String> {
        const COMMON_HEADER_CELLS: u32 = 19;
        const CURRENT_JOIN_CELLS: u32 = AUDIT_CURRENT_JOIN_CELLS_V7 as u32;
        const PARENT_DIGEST_CELLS: u32 = AUDIT_PARENT_DIGEST_CELLS_V7 as u32;

        if self.k != iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4
            || self.k != iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4
            || self.eq_coefficient_count == 0
            || self.ep_coefficient_count == 0
            || self.step_eq_params_sha256 == [0; 32]
            || self.step_ep_params_sha256 == [0; 32]
            || self.step_eq_params_sha256 == self.step_ep_params_sha256
            || self.step_eq_vk_sha256 == [0; 32]
            || self.step_ep_vk_sha256 == [0; 32]
            || self.step_eq_vk_sha256 == self.step_ep_vk_sha256
            || self.step_eq_constraint_degree != 9
            || self.step_ep_constraint_degree != 9
            || self.step_eq_fixed_columns != SERIALIZED_EXPECTED_FIXED_COLUMNS_V7
            || self.step_ep_fixed_columns != SERIALIZED_EXPECTED_FIXED_COLUMNS_V7
            || self.step_eq_permutation_columns != SERIALIZED_EXPECTED_PERMUTATION_COLUMNS_V7
            || self.step_ep_permutation_columns != SERIALIZED_EXPECTED_PERMUTATION_COLUMNS_V7
            || self.step_eq_blinding_factors != SERIALIZED_EXPECTED_BLINDING_FACTORS_V7
            || self.step_ep_blinding_factors != SERIALIZED_EXPECTED_BLINDING_FACTORS_V7
            || self.minimum_unusable_rows != SERIALIZED_EXPECTED_UNUSABLE_ROWS_V7
            || self.step_eq_blinding_factors + 1 != self.minimum_unusable_rows
            || self.step_ep_blinding_factors + 1 != self.minimum_unusable_rows
            || self.step_eq_proof_bytes != SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7
            || self.step_ep_proof_bytes != SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7
        {
            return Err("Kagemusha V7 serialized manifest identity/geometry is invalid".to_owned());
        }
        Self::validate_phase_column(
            &self.step_eq_advice_phases,
            self.step_eq_serialized_column,
            self.step_eq_phase_zero_rank,
        )?;
        Self::validate_phase_column(
            &self.step_ep_advice_phases,
            self.step_ep_serialized_column,
            self.step_ep_phase_zero_rank,
        )?;
        let domain_rows = 1_u64
            .checked_shl(self.k)
            .ok_or_else(|| "Kagemusha V7 domain exponent overflowed".to_owned())?;
        let eq_usable = domain_rows
            .checked_sub(u64::from(self.step_eq_blinding_factors) + 1)
            .ok_or_else(|| "Kagemusha V7 Eq blinding tail exceeds the domain".to_owned())?;
        let ep_usable = domain_rows
            .checked_sub(u64::from(self.step_ep_blinding_factors) + 1)
            .ok_or_else(|| "Kagemusha V7 Ep blinding tail exceeds the domain".to_owned())?;
        let eq_serialized_rows = u64::try_from(SERIALIZED_HEADER_CELLS_V7)
            .expect("fixed header fits u64")
            .checked_add(u64::from(self.eq_coefficient_count))
            .and_then(|rows| rows.checked_add(2 * u64::from(self.ep_coefficient_count)))
            .ok_or_else(|| "Kagemusha V7 Eq serialization row count overflowed".to_owned())?;
        let ep_serialized_rows = u64::try_from(SERIALIZED_HEADER_CELLS_V7)
            .expect("fixed header fits u64")
            .checked_add(u64::from(self.ep_coefficient_count))
            .and_then(|rows| rows.checked_add(2 * u64::from(self.eq_coefficient_count)))
            .ok_or_else(|| "Kagemusha V7 Ep serialization row count overflowed".to_owned())?;
        let accumulator_cells = self
            .k
            .checked_mul(2)
            .and_then(|cells| cells.checked_add(4))
            .ok_or_else(|| "Kagemusha V7 accumulator length overflowed".to_owned())?;
        let expected_join_offset = COMMON_HEADER_CELLS
            .checked_add(accumulator_cells)
            .ok_or_else(|| "Kagemusha V7 public join offset overflowed".to_owned())?;
        let expected_parent_offset = expected_join_offset
            .checked_add(CURRENT_JOIN_CELLS)
            .ok_or_else(|| "Kagemusha V7 parent digest offset overflowed".to_owned())?;
        let expected_live_offset = expected_parent_offset
            .checked_add(PARENT_DIGEST_CELLS)
            .ok_or_else(|| "Kagemusha V7 live-selector offset overflowed".to_owned())?;
        if eq_serialized_rows > eq_usable
            || ep_serialized_rows > ep_usable
            || self.current_join_offset != expected_join_offset
            || self.parent_digest_offset != expected_parent_offset
            || self.live_selector_offset != expected_live_offset
            || self.public_instance_cells != expected_live_offset + 1
        {
            return Err("Kagemusha V7 serialized rows or public layout are invalid".to_owned());
        }
        Ok(())
    }

    /// Canonical typed profile digest used by both circuits and the pair API.
    pub(super) fn sha256(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let mut hasher = Sha256::new();
        absorb_len_prefixed_v7(&mut hasher, SERIALIZED_AUDIT_PROFILE_DOMAIN_V7);
        hasher.update(SERIALIZED_AUDIT_PROFILE_VERSION_V7.to_le_bytes());
        hasher.update(self.k.to_le_bytes());
        for digest in [
            self.step_eq_params_sha256,
            self.step_ep_params_sha256,
            self.step_eq_vk_sha256,
            self.step_ep_vk_sha256,
        ] {
            absorb_len_prefixed_v7(&mut hasher, &digest);
        }
        for phases in [&self.step_eq_advice_phases, &self.step_ep_advice_phases] {
            absorb_len_prefixed_v7(&mut hasher, phases);
        }
        for value in [
            self.step_eq_serialized_column,
            self.step_ep_serialized_column,
            self.step_eq_phase_zero_rank,
            self.step_ep_phase_zero_rank,
            self.step_eq_constraint_degree,
            self.step_ep_constraint_degree,
            self.step_eq_fixed_columns,
            self.step_ep_fixed_columns,
            self.step_eq_permutation_columns,
            self.step_ep_permutation_columns,
            self.step_eq_blinding_factors,
            self.step_ep_blinding_factors,
            self.minimum_unusable_rows,
            self.step_eq_proof_bytes,
            self.step_ep_proof_bytes,
            self.eq_coefficient_count,
            self.ep_coefficient_count,
            self.public_instance_cells,
            self.current_join_offset,
            self.parent_digest_offset,
            self.live_selector_offset,
        ] {
            hasher.update(value.to_le_bytes());
        }
        hasher.update((SERIALIZED_HEADER_CELLS_V7 as u32).to_le_bytes());
        hasher.update(NATIVE_SCALAR_ENCODING_V7.to_le_bytes());
        hasher.update(RECIPROCAL_SCALAR_ENCODING_V7.to_le_bytes());
        hasher.update(ZERO_PADDING_ENCODING_V7.to_le_bytes());
        hasher.update([AUDIT_PARENT_SLOT_COUNT_V7 as u8]);
        // The consuming prover admits exactly one circuit. This byte pins the
        // RNG replay schedule against future batched-proof APIs.
        hasher.update([1]);
        Ok(hasher.finalize().into())
    }
}

impl KagemushaSerializedAuditProfileV7 {
    /// Construct a lab profile from a freshly captured graph.
    ///
    /// Production V7 must instead compare this count with the typed manifest
    /// constant recaptured for the exact Eq/Ep bootstrap and live graphs.
    pub(super) fn for_captured_coefficient_count(coefficient_count: usize) -> Result<Self, String> {
        if coefficient_count == 0 || u32::try_from(coefficient_count).is_err() {
            return Err("Kagemusha V7 serialized audit count is invalid".to_owned());
        }
        Ok(Self { coefficient_count })
    }

    /// Exact number of canonical polynomial coefficients.
    pub(super) const fn coefficient_count(self) -> usize {
        self.coefficient_count
    }
}

/// Frozen native vector returned directly by the reviewed pre-audit builder.
#[derive(Clone, Debug)]
pub(super) struct KagemushaFrozenNativeAuditVectorV7<F: PrimeField> {
    coefficients: Vec<AssignedValue<F>>,
}

/// Source capability implemented only next to the reviewed native builder.
pub(super) trait KagemushaNativeAuditVectorSourceV7<F: PrimeField> {
    /// Consume the exact source after its source-order and count checks.
    fn into_reviewed_coefficients(self) -> Vec<AssignedValue<F>>;
}

impl<F: PrimeField> KagemushaFrozenNativeAuditVectorV7<F> {
    /// Freeze the opaque result returned by the reviewed pre-audit builder.
    pub(super) fn from_reviewed_source<S>(
        profile: KagemushaSerializedAuditProfileV7,
        source: S,
    ) -> Result<Self, String>
    where
        S: KagemushaNativeAuditVectorSourceV7<F>,
    {
        let coefficients = source.into_reviewed_coefficients();
        if coefficients.len() != profile.coefficient_count {
            return Err(format!(
                "Kagemusha V7 native vector has {} coefficients instead of {}",
                coefficients.len(),
                profile.coefficient_count
            ));
        }
        Ok(Self { coefficients })
    }

    /// Borrow the exact pre-audit coefficient cells.
    pub(super) fn coefficients(&self) -> &[AssignedValue<F>] {
        &self.coefficients
    }
}

/// Frozen reciprocal vector encoded injectively as two LE u128 chunks/scalar.
#[derive(Clone, Debug)]
pub(super) struct KagemushaFrozenReciprocalAuditVectorV7<F: PrimeField> {
    chunks: Vec<[AssignedValue<F>; 2]>,
}

impl<F: PrimeField> KagemushaFrozenReciprocalAuditVectorV7<F> {
    /// Borrow the exact canonical two-chunk encodings.
    pub(super) fn chunks(&self) -> &[[AssignedValue<F>; 2]] {
        &self.chunks
    }
}

/// Canonicalize and injectively split every reciprocal scalar into two u128s.
pub(super) fn constrain_reciprocal_audit_vector_at_seam_v7<F>(
    ctx: &mut Context<F>,
    scalar: &halo2_ecc::fields::fp::FpChip<'_, F, F::ReciprocalScalar>,
    profile: KagemushaSerializedAuditProfileV7,
    coefficients: &[halo2_ecc::bigint::ProperCrtUint<F>],
) -> Result<KagemushaFrozenReciprocalAuditVectorV7<F>, String>
where
    F: KagemushaSerializedAuditFieldV7,
{
    use halo2_base::{
        QuantumCell::{Constant, Existing},
        gates::{GateInstructions as _, RangeInstructions as _},
        utils::fe_to_biguint,
    };
    use halo2_ecc::fields::FieldChip as _;

    if scalar.limb_bits != 86 || scalar.num_limbs != 3 {
        return Err(
            "Kagemusha V7 reciprocal encoding requires exactly three 86-bit limbs".to_owned(),
        );
    }
    if coefficients.len() != profile.coefficient_count {
        return Err("Kagemusha V7 reciprocal seam count mismatch".to_owned());
    }
    let mut chunks = Vec::with_capacity(coefficients.len());
    let two_to_42 = F::from_u128(1_u128 << 42);
    let two_to_44 = F::from_u128(1_u128 << 44);
    let two_to_86 = F::from_u128(1_u128 << 86);
    for coefficient in coefficients {
        if coefficient.limbs().len() != 3 {
            return Err(
                "Kagemusha V7 reciprocal coefficient does not have three source limbs".to_owned(),
            );
        }
        let canonical: halo2_ecc::bigint::ProperCrtUint<F> =
            scalar.enforce_less_than(ctx, coefficient.clone()).into();
        let limb_one = u128::try_from(fe_to_biguint(canonical.limbs()[1].value()))
            .map_err(|_| "Kagemusha V7 reciprocal middle limb does not fit u128".to_owned())?;
        let low_42 = ctx.load_witness(F::from_u128(limb_one & ((1_u128 << 42) - 1)));
        let high_44 = ctx.load_witness(F::from_u128(limb_one >> 42));
        scalar.range.range_check(ctx, low_42, 42);
        scalar.range.range_check(ctx, high_44, 44);
        let recomposed_middle = scalar.range.gate().mul_add(
            ctx,
            Existing(high_44),
            Constant(two_to_42),
            Existing(low_42),
        );
        ctx.constrain_equal(&recomposed_middle, &canonical.limbs()[1]);
        let low = scalar.range.gate().mul_add(
            ctx,
            Existing(low_42),
            Constant(two_to_86),
            Existing(canonical.limbs()[0]),
        );
        let high = scalar.range.gate().mul_add(
            ctx,
            Existing(canonical.limbs()[2]),
            Constant(two_to_44),
            Existing(high_44),
        );
        scalar.range.range_check(ctx, low, 128);
        scalar.range.range_check(ctx, high, 128);
        chunks.push([low, high]);
    }
    Ok(KagemushaFrozenReciprocalAuditVectorV7 { chunks })
}

/// Exact copy-bound serialization queued for one physical proof.
#[derive(Clone, Debug)]
pub(super) struct KagemushaSerializedAuditJobsV7<F: PrimeField> {
    values: Vec<AssignedValue<F>>,
    padding_zero: Option<AssignedValue<F>>,
    native_count: usize,
    reciprocal_count: usize,
    authenticated_usable_rows: Option<usize>,
    queued: bool,
    use_unknown: bool,
}

impl<F: PrimeField> Default for KagemushaSerializedAuditJobsV7<F> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            padding_zero: None,
            native_count: 0,
            reciprocal_count: 0,
            authenticated_usable_rows: None,
            queued: false,
            use_unknown: false,
        }
    }
}

impl<F> KagemushaSerializedAuditJobsV7<F>
where
    F: KagemushaSerializedAuditFieldV7,
{
    /// Queue exactly one schema-tagged physical-proof serialization.
    pub(super) fn queue_physical_proof(
        &mut self,
        ctx: &mut Context<F>,
        native_profile: KagemushaSerializedAuditProfileV7,
        native: &KagemushaFrozenNativeAuditVectorV7<F>,
        reciprocal_profile: KagemushaSerializedAuditProfileV7,
        reciprocal: &KagemushaFrozenReciprocalAuditVectorV7<F>,
        authenticated_usable_rows: usize,
    ) -> Result<(), String> {
        if self.queued {
            return Err("Kagemusha V7 serialized audit was queued more than once".to_owned());
        }
        if native.coefficients.len() != native_profile.coefficient_count
            || reciprocal.chunks.len() != reciprocal_profile.coefficient_count
        {
            return Err("Kagemusha V7 serialized audit vector/profile mismatch".to_owned());
        }
        let reciprocal_cells = reciprocal_profile
            .coefficient_count
            .checked_mul(2)
            .ok_or_else(|| "Kagemusha V7 reciprocal serialization overflowed".to_owned())?;
        let serialized_len = SERIALIZED_HEADER_CELLS_V7
            .checked_add(native_profile.coefficient_count)
            .and_then(|len| len.checked_add(reciprocal_cells))
            .ok_or_else(|| "Kagemusha V7 serialization length overflowed".to_owned())?;
        self.values
            .try_reserve_exact(serialized_len)
            .map_err(|_| "Kagemusha V7 serialization allocation failed".to_owned())?;
        self.values.extend([
            ctx.load_constant(F::from(SERIALIZED_AUDIT_SCHEMA_V7)),
            ctx.load_constant(F::from(F::PHYSICAL_FIELD_TAG)),
            ctx.load_constant(F::from(parity_tag_v7(F::NATIVE_TARGET))),
            ctx.load_constant(F::from_u128(native_profile.coefficient_count as u128)),
            ctx.load_constant(F::from(NATIVE_SCALAR_ENCODING_V7)),
            ctx.load_constant(F::from(parity_tag_v7(F::RECIPROCAL_TARGET))),
            ctx.load_constant(F::from_u128(reciprocal_profile.coefficient_count as u128)),
            ctx.load_constant(F::from(RECIPROCAL_SCALAR_ENCODING_V7)),
            ctx.load_constant(F::from(ZERO_PADDING_ENCODING_V7)),
            ctx.load_constant(F::from_u128(serialized_len as u128)),
        ]);
        self.values.extend_from_slice(&native.coefficients);
        self.values
            .extend(reciprocal.chunks.iter().flatten().copied());
        if self.values.len() != serialized_len {
            return Err("Kagemusha V7 serialization length drifted".to_owned());
        }
        if serialized_len > authenticated_usable_rows {
            return Err("Kagemusha V7 serialization exceeds authenticated usable rows".to_owned());
        }
        self.padding_zero = Some(ctx.load_zero());
        self.native_count = native_profile.coefficient_count;
        self.reciprocal_count = reciprocal_profile.coefficient_count;
        self.authenticated_usable_rows = Some(authenticated_usable_rows);
        self.queued = true;
        Ok(())
    }

    /// Preserve the exact source cells and shape while hiding lane witnesses.
    pub(super) fn unknown(&self) -> Self {
        let mut clone = self.clone();
        clone.use_unknown = true;
        clone
    }

    /// Return `(jobs, native coefficients, reciprocal coefficients, rows)`.
    pub(super) fn capacity_profile(&self) -> Result<(usize, usize, usize, usize), String> {
        if !self.queued || self.values.is_empty() {
            return Err("Kagemusha V7 serialized audit queue is incomplete".to_owned());
        }
        Ok((
            1,
            self.native_count,
            self.reciprocal_count,
            self.values.len(),
        ))
    }

    /// Reproduce the exact selected phase-zero advice commitment before proof
    /// construction, using the same fresh deterministic RNG stream later
    /// passed to `create_proof_consuming`.
    pub(super) fn precommitment<C, R>(
        &self,
        params: &halo2_base::halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
        verifying_key: &halo2_base::halo2_proofs::plonk::VerifyingKey<C>,
        selected_advice_column: usize,
        mut rng: R,
    ) -> Result<C, String>
    where
        C: CurveAffine<ScalarExt = F>,
        R: rand_core_06::RngCore + rand_core_06::CryptoRng,
    {
        use halo2_base::halo2_proofs::{
            halo2curves::group::Curve as _,
            poly::commitment::{Blind, Params as _},
        };

        let (_, _, _, serialized_rows) = self.capacity_profile()?;
        let phases = verifying_key.cs().advice_column_phase();
        if phases.len() != verifying_key.cs().num_advice_columns()
            || phases.get(selected_advice_column) != Some(&0)
        {
            return Err("Kagemusha V7 selected serialization column is not phase zero".to_owned());
        }
        let n = usize::try_from(params.n())
            .map_err(|_| "Kagemusha V7 parameter row count does not fit usize".to_owned())?;
        if 1_u64
            .checked_shl(verifying_key.get_domain().k())
            .filter(|domain_n| *domain_n == params.n())
            .is_none()
        {
            return Err("Kagemusha V7 parameter and proving-key domains differ".to_owned());
        }
        let unusable = verifying_key
            .cs()
            .blinding_factors()
            .checked_add(1)
            .ok_or_else(|| "Kagemusha V7 blinding row count overflowed".to_owned())?;
        let usable_rows = n
            .checked_sub(unusable)
            .ok_or_else(|| "Kagemusha V7 blinding rows exceed the domain".to_owned())?;
        if self.authenticated_usable_rows != Some(usable_rows) {
            return Err("Kagemusha V7 proving-key usable rows differ from the profile".to_owned());
        }
        if serialized_rows > usable_rows {
            return Err("Kagemusha V7 serialization exceeds usable rows".to_owned());
        }
        let phase_zero_columns = phases
            .iter()
            .enumerate()
            .filter_map(|(index, phase)| (*phase == 0).then_some(index))
            .collect::<Vec<_>>();
        let selected_position = phase_zero_columns
            .iter()
            .position(|index| *index == selected_advice_column)
            .ok_or_else(|| "Kagemusha V7 selected serialization column is absent".to_owned())?;

        // `create_proof_consuming` first draws every unusable-row value for
        // every phase-zero column in column order, then one blind per column.
        let mut selected_unusable = Vec::with_capacity(unusable);
        for position in 0..phase_zero_columns.len() {
            for _ in 0..unusable {
                let value = F::random(&mut rng);
                if position == selected_position {
                    selected_unusable.push(value);
                }
            }
        }
        let mut selected_blind = None;
        for position in 0..phase_zero_columns.len() {
            let blind = F::random(&mut rng);
            if position == selected_position {
                selected_blind = Some(blind);
            }
        }

        let mut values = vec![F::ZERO; n];
        for (target, source) in values.iter_mut().zip(&self.values) {
            // Base values may still be represented as `Assigned::Rational`
            // before batch inversion.  Normalize them exactly as the Halo2
            // witness collector does; `AssignedValue::value()` would panic
            // for that valid representation.
            *target = source.value.evaluate();
        }
        values[usable_rows..].copy_from_slice(&selected_unusable);
        let polynomial = verifying_key.get_domain().lagrange_from_vec(values);
        let commitment = params
            .commit_lagrange(
                &polynomial,
                Blind(selected_blind.expect("selected phase-zero blind exists")),
            )
            .to_affine();
        if bool::from(commitment.is_identity()) {
            return Err("Kagemusha V7 serialized advice commitment is the identity".to_owned());
        }
        Ok(commitment)
    }

    /// Realize the serialization after Base synthesis and bind every row.
    pub(super) fn synthesize(
        &self,
        config: &KagemushaSerializedAuditConfigV7,
        layouter: &mut impl Layouter<F>,
        copy_manager: &SharedCopyConstraintManager<F>,
        usable_rows: usize,
    ) -> Result<(), Error> {
        let (_, _, _, serialized_rows) = self.capacity_profile().map_err(|_| Error::Synthesis)?;
        if serialized_rows > usable_rows || self.authenticated_usable_rows != Some(usable_rows) {
            return Err(Error::Synthesis);
        }
        let padding_zero = self.padding_zero.ok_or(Error::Synthesis)?;
        let physical_cells = copy_manager.lock().map_err(|_| Error::Synthesis)?;
        layouter.assign_region(
            || "Kagemusha V7 serialized audit column",
            |mut region| {
                for row in 0..usable_rows {
                    let source = self.values.get(row).copied();
                    let value = match (self.use_unknown, source) {
                        (true, Some(_)) => Value::unknown(),
                        (false, Some(source)) => Value::known(source.value.evaluate()),
                        (_, None) => Value::known(F::ZERO),
                    };
                    let raw = region.assign_advice(config.column, row, value).cell();
                    if let Some(source) = source {
                        bind_virtual_v7(
                            &mut region,
                            raw,
                            source,
                            &physical_cells.assigned_advices,
                        )?;
                    } else {
                        // Every active tail row is part of the committed
                        // polynomial.  Copy it to one reviewed Base zero so a
                        // prover cannot smuggle unconstrained padding into the
                        // pre-challenge binding.
                        bind_virtual_v7(
                            &mut region,
                            raw,
                            padding_zero,
                            &physical_cells.assigned_advices,
                        )?;
                    }
                }
                Ok(())
            },
        )
    }
}

/// One equality-enabled phase-zero advice column and no gate/fixed polynomial.
#[derive(Clone, Copy, Debug)]
pub(super) struct KagemushaSerializedAuditConfigV7 {
    column: Column<Advice>,
}

impl KagemushaSerializedAuditConfigV7 {
    /// Allocate the sole serialization column.
    pub(super) fn configure<F: PrimeField>(meta: &mut ConstraintSystem<F>) -> Self {
        let column = meta.advice_column();
        meta.enable_equality(column);
        Self { column }
    }

    /// Stable advice-column index authenticated by the V7 typed profile.
    pub(super) fn advice_column_index(self) -> usize {
        self.column.index()
    }

    /// Exact custom polynomial contribution `(advice, fixed, permutation)`.
    pub(super) const fn polynomial_profile() -> (usize, usize, usize) {
        (1, 0, 1)
    }
}

fn bind_virtual_v7<F: BigPrimeField>(
    region: &mut halo2_base::halo2_proofs::circuit::Region<'_, F>,
    raw: Cell,
    virtual_value: AssignedValue<F>,
    physical_cells: &std::collections::HashMap<halo2_base::ContextCell, Cell>,
) -> Result<(), Error> {
    let virtual_cell = virtual_value.cell.ok_or(Error::Synthesis)?;
    let physical = *physical_cells.get(&virtual_cell).ok_or(Error::Synthesis)?;
    region.constrain_equal(raw, physical);
    Ok(())
}

/// Frozen context hashed before the shared pair evaluation challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaSerializedAuditChallengeContextV7 {
    /// Typed V7 profile/manifest digest.
    pub(super) profile_sha256: [u8; 32],
    /// Exact StepEq verifying-key digest.
    pub(super) step_eq_vk_sha256: [u8; 32],
    /// Exact StepEp verifying-key digest.
    pub(super) step_ep_vk_sha256: [u8; 32],
    /// Frozen core statement excluding all V7 join cells.
    pub(super) frozen_core_statement_sha256: [u8; 32],
    /// Exact Eq target coefficient count.
    pub(super) eq_coefficient_count: u32,
    /// Exact Ep target coefficient count.
    pub(super) ep_coefficient_count: u32,
}

impl KagemushaSerializedAuditChallengeContextV7 {
    /// Build the only authenticated challenge context admitted by V7.
    pub(super) fn from_manifest(
        manifest: &KagemushaSerializedAuditManifestV7,
        frozen_core_statement_sha256: [u8; 32],
    ) -> Result<Self, String> {
        if frozen_core_statement_sha256 == [0; 32] {
            return Err("Kagemusha V7 frozen core statement is zero".to_owned());
        }
        Ok(Self {
            profile_sha256: manifest.sha256()?,
            step_eq_vk_sha256: manifest.step_eq_vk_sha256,
            step_ep_vk_sha256: manifest.step_ep_vk_sha256,
            frozen_core_statement_sha256,
            eq_coefficient_count: manifest.eq_coefficient_count,
            ep_coefficient_count: manifest.ep_coefficient_count,
        })
    }
}

/// Circuit cells for the same field-neutral challenge context.
///
/// The three 32-byte identity values are loaded from the already-constrained
/// public core rather than hard-coded, avoiding a self-referential final-VK
/// constant.  The authenticated manifest behind `profile_sha256` binds Params,
/// layout, phase vector, selected column, and padding policy.
#[derive(Clone, Copy, Debug)]
pub(super) struct KagemushaAssignedAuditChallengeContextV7<F: PrimeField> {
    /// Authenticated V7 manifest/profile digest as exact little-endian chunks.
    pub(super) profile_sha256_exact_chunks: [AssignedValue<F>; 2],
    /// StepEq digest as four packed big-endian SHA words per chunk.
    pub(super) step_eq_vk_sha256_word_chunks: [AssignedValue<F>; 2],
    /// StepEp digest as four packed big-endian SHA words per chunk.
    pub(super) step_ep_vk_sha256_word_chunks: [AssignedValue<F>; 2],
    /// Frozen statement digest as exact little-endian chunks.
    pub(super) frozen_core_statement_sha256_exact_chunks: [AssignedValue<F>; 2],
    /// Exact Eq target coefficient count.
    pub(super) eq_coefficient_count: u32,
    /// Exact Ep target coefficient count.
    pub(super) ep_coefficient_count: u32,
}

/// Circuit form of the parent-pair digest identity.
///
/// Profile and VK identities are assigned from the authenticated current
/// public core.  They are deliberately not circuit constants: placing a final
/// VK digest in a fixed column would make the V7 VK identity self-referential.
#[derive(Clone, Copy, Debug)]
pub(super) struct KagemushaAssignedParentDigestContextV7<F: PrimeField> {
    /// Typed profile digest as exact little-endian chunks.
    pub(super) profile_sha256_exact_chunks: [AssignedValue<F>; 2],
    /// StepEq digest as four packed big-endian SHA words per chunk.
    pub(super) step_eq_vk_sha256_word_chunks: [AssignedValue<F>; 2],
    /// StepEp digest as four packed big-endian SHA words per chunk.
    pub(super) step_ep_vk_sha256_word_chunks: [AssignedValue<F>; 2],
    /// Exact Eq target coefficient count.
    pub(super) eq_coefficient_count: u32,
    /// Exact Ep target coefficient count.
    pub(super) ep_coefficient_count: u32,
}

/// Ten shared u128 cells carried identically by both proof halves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaSerializedAuditPublicJoinV7 {
    /// Canonical compressed selected StepEq advice commitment.
    pub(super) step_eq_commitment: [u128; 2],
    /// Canonical compressed selected StepEp advice commitment.
    pub(super) step_ep_commitment: [u128; 2],
    /// Shared masked-254-bit challenge.
    pub(super) challenge: [u128; 2],
    /// Full canonical Eq-field evaluation.
    pub(super) eq_evaluation: [u128; 2],
    /// Full canonical Ep-field evaluation.
    pub(super) ep_evaluation: [u128; 2],
}

impl KagemushaSerializedAuditPublicJoinV7 {
    /// Return the canonical ten-cell order used by both instance columns.
    pub(super) const fn cells(self) -> [u128; 10] {
        [
            self.step_eq_commitment[0],
            self.step_eq_commitment[1],
            self.step_ep_commitment[0],
            self.step_ep_commitment[1],
            self.challenge[0],
            self.challenge[1],
            self.eq_evaluation[0],
            self.eq_evaluation[1],
            self.ep_evaluation[0],
            self.ep_evaluation[1],
        ]
    }
}

/// Fixed context shared by both ordered parent-slot digest transcripts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KagemushaSerializedParentDigestContextV7 {
    /// Typed V7 profile/manifest digest.
    pub(super) profile_sha256: [u8; 32],
    /// Exact StepEq verifying-key digest.
    pub(super) step_eq_vk_sha256: [u8; 32],
    /// Exact StepEp verifying-key digest.
    pub(super) step_ep_vk_sha256: [u8; 32],
    /// Exact Eq target coefficient count.
    pub(super) eq_coefficient_count: u32,
    /// Exact Ep target coefficient count.
    pub(super) ep_coefficient_count: u32,
}

impl From<&KagemushaSerializedAuditChallengeContextV7>
    for KagemushaSerializedParentDigestContextV7
{
    fn from(context: &KagemushaSerializedAuditChallengeContextV7) -> Self {
        Self {
            profile_sha256: context.profile_sha256,
            step_eq_vk_sha256: context.step_eq_vk_sha256,
            step_ep_vk_sha256: context.step_ep_vk_sha256,
            eq_coefficient_count: context.eq_coefficient_count,
            ep_coefficient_count: context.ep_coefficient_count,
        }
    }
}

/// Host form of one ordered parent-pair tuple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct KagemushaSerializedParentSlotV7 {
    /// Whether this logical parent slot is present.
    pub(super) present: bool,
    /// Parent's typed public profile cell.
    pub(super) profile: u128,
    /// Parent's recursive step count.
    pub(super) proof_step_count: u128,
    /// Parent's own bounded parent count.
    pub(super) parent_count: u8,
    /// Parent proof's own live/bootstrap bit, distinct from slot presence.
    pub(super) parent_live: bool,
    /// Frozen parent core-statement digest, excluding every V7 audit cell.
    pub(super) frozen_core_statement: [u128; 2],
    /// Parent's exact current `Ceq/Cep/z/vEq/vEp` tuple.
    pub(super) current_join: [u128; AUDIT_CURRENT_JOIN_CELLS_V7],
    /// Parent's own ordered-parent digest, preserving induction ancestry.
    pub(super) parent_slots_digest: [u128; AUDIT_PARENT_DIGEST_CELLS_V7],
}

/// Circuit cells for one ordered parsed parent-pair tuple.
#[derive(Clone, Copy, Debug)]
pub(super) struct KagemushaAssignedParentSlotV7<F: PrimeField> {
    /// Circuit-derived current-step presence selector for this parent slot.
    pub(super) present: AssignedValue<F>,
    /// Exact parent profile cell parsed from the verified instance column.
    pub(super) profile: AssignedValue<F>,
    /// Exact parent step count parsed from the verified instance column.
    pub(super) proof_step_count: AssignedValue<F>,
    /// Exact parent count parsed from that same column.
    pub(super) parent_count: AssignedValue<F>,
    /// Exact parent live/bootstrap bit parsed from that same column.
    pub(super) parent_live: AssignedValue<F>,
    /// Frozen parent core-statement chunks parsed from that same column.
    pub(super) frozen_core_statement: [AssignedValue<F>; 2],
    /// Exact parent `Ceq/Cep/z/vEq/vEp` cells parsed from that same column.
    pub(super) current_join: [AssignedValue<F>; AUDIT_CURRENT_JOIN_CELLS_V7],
    /// Exact ancestry digest parsed from that same column.
    pub(super) parent_slots_digest: [AssignedValue<F>; AUDIT_PARENT_DIGEST_CELLS_V7],
}

fn absorb_len_prefixed_v7(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(
        u32::try_from(bytes.len())
            .expect("fixed V7 transcript item length")
            .to_le_bytes(),
    );
    hasher.update(bytes);
}

/// Derive the shared field-neutral challenge after both serialized commitments.
pub(super) fn kagemusha_serialized_audit_challenge_v7(
    context: &KagemushaSerializedAuditChallengeContextV7,
    step_eq_commitment: [u8; 32],
    step_ep_commitment: [u8; 32],
) -> Result<[u8; 32], String> {
    let mut hasher = Sha256::new();
    absorb_len_prefixed_v7(&mut hasher, AUDIT_PAIR_CHALLENGE_DOMAIN_V7);
    absorb_len_prefixed_v7(&mut hasher, &context.profile_sha256);
    absorb_len_prefixed_v7(&mut hasher, &context.step_eq_vk_sha256);
    absorb_len_prefixed_v7(&mut hasher, &context.step_ep_vk_sha256);
    absorb_len_prefixed_v7(&mut hasher, &context.frozen_core_statement_sha256);
    hasher.update(context.eq_coefficient_count.to_le_bytes());
    hasher.update(context.ep_coefficient_count.to_le_bytes());
    absorb_len_prefixed_v7(&mut hasher, &step_eq_commitment);
    absorb_len_prefixed_v7(&mut hasher, &step_ep_commitment);
    let mut digest: [u8; 32] = hasher.finalize().into();
    // Both Pasta moduli exceed every 254-bit integer.  Discard, rather than
    // reject, raw digest bits 254 and 255 so both circuits get the same scalar.
    digest[31] &= 0x3f;
    let low = u128::from_le_bytes(digest[..16].try_into().expect("digest low half"));
    let high = u128::from_le_bytes(digest[16..].try_into().expect("digest high half"));
    if high == 0 && low <= 1 {
        return Err("Kagemusha V7 serialized audit challenge is zero or one".to_owned());
    }
    Ok(digest)
}

fn push_len_prefixed_assigned_chunks_v7<F: BigPrimeField>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    output: &mut Vec<KagemushaSha256ByteV4<F>>,
    chunks: [AssignedValue<F>; 2],
) {
    push_constant_bytes_v7(output, &32_u32.to_le_bytes());
    for chunk in chunks {
        push_assigned_u128_le_v7(ctx, gate, output, chunk);
    }
}

fn push_len_prefixed_assigned_sha256_word_chunks_v7<F: BigPrimeField>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    output: &mut Vec<KagemushaSha256ByteV4<F>>,
    chunks: [AssignedValue<F>; 2],
) {
    push_constant_bytes_v7(output, &32_u32.to_le_bytes());
    for chunk in chunks {
        let bits = KagemushaSha256BitV4::decompose(ctx, gate, chunk, 128);
        for word_bits in bits.chunks_exact(32) {
            output.extend(
                word_bits
                    .chunks_exact(8)
                    .rev()
                    .map(|byte_bits| KagemushaSha256ByteV4::from_bits_le(ctx, gate, byte_bits)),
            );
        }
    }
}

fn sha_digest_raw_bytes_v7<F: BigPrimeField>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    words: [AssignedValue<F>; 8],
) -> Vec<KagemushaSha256ByteV4<F>> {
    let mut bytes = Vec::with_capacity(32);
    for word in words {
        let bits = KagemushaSha256BitV4::decompose(ctx, gate, word, 32);
        let little_endian = bits
            .chunks_exact(8)
            .map(|bits| KagemushaSha256ByteV4::from_bits_le(ctx, gate, bits))
            .collect::<Vec<_>>();
        bytes.extend(little_endian.into_iter().rev());
    }
    bytes
}

pub(super) fn pack_assigned_le_bytes_v7<F: BigPrimeField>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    bytes: &[KagemushaSha256ByteV4<F>],
) -> AssignedValue<F> {
    use halo2_base::{QuantumCell::Constant, gates::GateInstructions as _};

    assert!(bytes.len() <= 16, "u128 chunk has at most sixteen bytes");
    gate.inner_product(
        ctx,
        bytes
            .iter()
            .copied()
            .map(KagemushaSha256ByteV4::quantum_cell),
        (0..bytes.len()).map(|index| Constant(F::from_u128(1_u128 << (8 * index)))),
    )
}

/// Recompute the shared masked challenge from the exact public core and both
/// selected advice commitments.  Raw SHA bits 254/255 are discarded, never
/// required to be zero, and the resulting scalar is constrained away from
/// zero and one.
pub(super) fn constrain_kagemusha_serialized_audit_challenge_v7<F>(
    ctx: &mut Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    sha_jobs: &mut KagemushaSha256JobsV4<F>,
    context: KagemushaAssignedAuditChallengeContextV7<F>,
    step_eq_commitment: [AssignedValue<F>; 2],
    step_ep_commitment: [AssignedValue<F>; 2],
    expected_challenge: [AssignedValue<F>; 2],
) -> Result<AssignedValue<F>, String>
where
    F: KagemushaSerializedAuditFieldV7,
{
    use halo2_base::{
        QuantumCell::{Constant, Existing},
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    let gate = range.gate();
    let mut message = Vec::new();
    let mut domain_prefix = Vec::new();
    extend_len_prefixed_v7(&mut domain_prefix, AUDIT_PAIR_CHALLENGE_DOMAIN_V7);
    push_constant_bytes_v7(&mut message, &domain_prefix);
    let profile = context.profile_sha256_exact_chunks;
    for chunk in profile {
        range.range_check(ctx, chunk, 128);
    }
    push_len_prefixed_assigned_chunks_v7(ctx, gate, &mut message, profile);
    // The compact V5/V7 header preserves statement/profile bytes as exact LE
    // limbs, but protocol identities are the SHA gadget's big-endian u32
    // words packed four-at-a-time into each LE u128 public cell.
    for chunks in [
        context.step_eq_vk_sha256_word_chunks,
        context.step_ep_vk_sha256_word_chunks,
    ] {
        for chunk in chunks {
            range.range_check(ctx, chunk, 128);
        }
        push_len_prefixed_assigned_sha256_word_chunks_v7(ctx, gate, &mut message, chunks);
    }
    let statement = context.frozen_core_statement_sha256_exact_chunks;
    for chunk in statement {
        range.range_check(ctx, chunk, 128);
    }
    push_len_prefixed_assigned_chunks_v7(ctx, gate, &mut message, statement);
    push_constant_bytes_v7(&mut message, &context.eq_coefficient_count.to_le_bytes());
    push_constant_bytes_v7(&mut message, &context.ep_coefficient_count.to_le_bytes());
    for chunks in [step_eq_commitment, step_ep_commitment] {
        for chunk in chunks {
            range.range_check(ctx, chunk, 128);
        }
        push_len_prefixed_assigned_chunks_v7(ctx, gate, &mut message, chunks);
    }
    let digest: [AssignedValue<F>; 8] = sha_jobs
        .digest_constrained(ctx, &message)?
        .try_into()
        .expect("SHA-256 has eight words");
    let mut raw_bytes = sha_digest_raw_bytes_v7(ctx, gate, digest);
    let raw_last_bits = raw_bytes[31].decompose_bits_le(ctx, gate);
    let zero = ctx.load_zero();
    let zero_bits = KagemushaSha256BitV4::decompose(ctx, gate, zero, 2);
    let masked_last_bits = raw_last_bits[..6]
        .iter()
        .copied()
        .chain(zero_bits)
        .collect::<Vec<_>>();
    raw_bytes[31] = KagemushaSha256ByteV4::from_bits_le(ctx, gate, &masked_last_bits);
    let derived = [
        pack_assigned_le_bytes_v7(ctx, gate, &raw_bytes[..16]),
        pack_assigned_le_bytes_v7(ctx, gate, &raw_bytes[16..]),
    ];
    range.range_check(ctx, expected_challenge[0], 128);
    range.range_check(ctx, expected_challenge[1], 126);
    for (actual, expected) in derived.into_iter().zip(expected_challenge) {
        ctx.constrain_equal(&actual, &expected);
    }
    let challenge = gate.inner_product(
        ctx,
        expected_challenge,
        [
            Constant(F::ONE),
            Constant(F::from_u128(1_u128 << 127).double()),
        ],
    );
    let is_zero = gate.is_zero(ctx, challenge);
    let is_one = gate.is_equal(ctx, challenge, Constant(F::ONE));
    gate.assert_is_const(ctx, &is_zero, &F::ZERO);
    gate.assert_is_const(ctx, &is_one, &F::ZERO);
    // Make the exact source relation explicit for source-level audits.
    let recomposed = gate.mul_add(
        ctx,
        Existing(expected_challenge[1]),
        Constant(F::from_u128(1_u128 << 127).double()),
        Existing(expected_challenge[0]),
    );
    ctx.constrain_equal(&challenge, &recomposed);
    Ok(challenge)
}

fn extend_len_prefixed_v7(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend(
        u32::try_from(bytes.len())
            .expect("fixed V7 transcript item length")
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
}

fn kagemusha_serialized_parent_slots_prefix_v7(
    context: &KagemushaSerializedParentDigestContextV7,
) -> Vec<u8> {
    let mut output = Vec::new();
    extend_len_prefixed_v7(&mut output, AUDIT_PARENT_SLOTS_DOMAIN_V7);
    output.extend_from_slice(&SERIALIZED_AUDIT_SCHEMA_V7.to_le_bytes());
    extend_len_prefixed_v7(&mut output, &context.profile_sha256);
    extend_len_prefixed_v7(&mut output, &context.step_eq_vk_sha256);
    extend_len_prefixed_v7(&mut output, &context.step_ep_vk_sha256);
    output.extend_from_slice(&context.eq_coefficient_count.to_le_bytes());
    output.extend_from_slice(&context.ep_coefficient_count.to_le_bytes());
    output.push(u8::try_from(AUDIT_PARENT_SLOT_COUNT_V7).expect("two parent slots fit u8"));
    output
}

fn append_parent_slot_host_v7(
    output: &mut Vec<u8>,
    index: usize,
    slot: KagemushaSerializedParentSlotV7,
) -> Result<(), String> {
    output.push(u8::try_from(index).expect("parent slot index fits u8"));
    output.push(u8::from(slot.present));
    if slot.parent_count > 2 {
        return Err("Kagemusha V7 parent tuple count exceeds two".to_owned());
    }
    if !slot.present && (slot.parent_count != 0 || slot.parent_live) {
        return Err("Kagemusha V7 absent parent tuple metadata is not canonical zero".to_owned());
    }
    output.push(slot.parent_count);
    output.push(u8::from(slot.parent_live));
    let values = [slot.profile, slot.proof_step_count]
        .into_iter()
        .chain(slot.frozen_core_statement)
        .chain(slot.current_join)
        .chain(slot.parent_slots_digest);
    for value in values {
        if !slot.present && value != 0 {
            return Err("Kagemusha V7 absent parent tuple is not canonical zero".to_owned());
        }
        output.extend_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

/// Hash both ordered parent tuples, including presence and ancestry.
pub(super) fn kagemusha_serialized_parent_slots_digest_v7(
    context: &KagemushaSerializedParentDigestContextV7,
    slots: [KagemushaSerializedParentSlotV7; AUDIT_PARENT_SLOT_COUNT_V7],
) -> Result<[u8; 32], String> {
    let mut message = kagemusha_serialized_parent_slots_prefix_v7(context);
    for (index, slot) in slots.into_iter().enumerate() {
        append_parent_slot_host_v7(&mut message, index, slot)?;
    }
    Ok(Sha256::digest(message).into())
}

/// Domain-separated bootstrap value for two absent canonical parent slots.
pub(super) fn kagemusha_serialized_null_parent_digest_v7(
    context: &KagemushaSerializedParentDigestContextV7,
) -> [u8; 32] {
    kagemusha_serialized_parent_slots_digest_v7(
        context,
        [KagemushaSerializedParentSlotV7::default(); AUDIT_PARENT_SLOT_COUNT_V7],
    )
    .expect("two absent V7 parent slots are canonical zero")
}

/// Encode SHA-256 as two chunks of four big-endian digest words.
///
/// This is deliberately distinct from little-endian compressed-point chunks.
pub(super) fn kagemusha_serialized_digest_word_chunks_v7(digest: [u8; 32]) -> [u128; 2] {
    let words = std::array::from_fn::<_, 8, _>(|index| {
        u32::from_be_bytes(
            digest[index * 4..index * 4 + 4]
                .try_into()
                .expect("SHA-256 word has four bytes"),
        )
    });
    std::array::from_fn(|chunk| {
        words[chunk * 4..chunk * 4 + 4]
            .iter()
            .enumerate()
            .fold(0_u128, |value, (index, word)| {
                value | (u128::from(*word) << (index * 32))
            })
    })
}

fn push_constant_bytes_v7<F: BigPrimeField>(
    output: &mut Vec<KagemushaSha256ByteV4<F>>,
    bytes: &[u8],
) {
    output.extend(bytes.iter().copied().map(KagemushaSha256ByteV4::constant));
}

fn push_assigned_u128_le_v7<F: BigPrimeField>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    output: &mut Vec<KagemushaSha256ByteV4<F>>,
    value: AssignedValue<F>,
) {
    let bits = KagemushaSha256BitV4::decompose(ctx, gate, value, 128);
    output.extend(
        bits.chunks_exact(8)
            .map(|bits| KagemushaSha256ByteV4::from_bits_le(ctx, gate, bits)),
    );
}

/// Constrain the ordered parent tuple digest from cells loaded by the verified
/// parent proofs, then bind its exact two public chunks.
pub(super) fn constrain_kagemusha_serialized_parent_slots_digest_v7<F>(
    ctx: &mut Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    sha_jobs: &mut KagemushaSha256JobsV4<F>,
    context: KagemushaAssignedParentDigestContextV7<F>,
    slots: [KagemushaAssignedParentSlotV7<F>; AUDIT_PARENT_SLOT_COUNT_V7],
    expected_digest: [AssignedValue<F>; AUDIT_PARENT_DIGEST_CELLS_V7],
) -> Result<[AssignedValue<F>; 8], String>
where
    F: KagemushaSerializedAuditFieldV7,
{
    use halo2_base::{
        QuantumCell::{Constant, Existing},
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    let gate = range.gate();
    let mut message = Vec::new();
    let mut prefix = Vec::new();
    extend_len_prefixed_v7(&mut prefix, AUDIT_PARENT_SLOTS_DOMAIN_V7);
    prefix.extend_from_slice(&SERIALIZED_AUDIT_SCHEMA_V7.to_le_bytes());
    push_constant_bytes_v7(&mut message, &prefix);
    let profile = context.profile_sha256_exact_chunks;
    for chunk in profile {
        range.range_check(ctx, chunk, 128);
    }
    push_len_prefixed_assigned_chunks_v7(ctx, gate, &mut message, profile);
    for chunks in [
        context.step_eq_vk_sha256_word_chunks,
        context.step_ep_vk_sha256_word_chunks,
    ] {
        for chunk in chunks {
            range.range_check(ctx, chunk, 128);
        }
        push_len_prefixed_assigned_sha256_word_chunks_v7(ctx, gate, &mut message, chunks);
    }
    push_constant_bytes_v7(&mut message, &context.eq_coefficient_count.to_le_bytes());
    push_constant_bytes_v7(&mut message, &context.ep_coefficient_count.to_le_bytes());
    push_constant_bytes_v7(
        &mut message,
        &[u8::try_from(AUDIT_PARENT_SLOT_COUNT_V7).expect("two parent slots fit u8")],
    );
    for (index, slot) in slots.into_iter().enumerate() {
        gate.assert_bit(ctx, slot.present);
        push_constant_bytes_v7(
            &mut message,
            &[u8::try_from(index).expect("parent slot index fits u8")],
        );
        let presence_bits = KagemushaSha256BitV4::decompose(ctx, gate, slot.present, 8);
        message.push(KagemushaSha256ByteV4::from_bits_le(
            ctx,
            gate,
            &presence_bits,
        ));
        range.range_check(ctx, slot.parent_count, 2);
        let count_is_three = gate.is_equal(ctx, slot.parent_count, Constant(F::from(3)));
        gate.assert_is_const(ctx, &count_is_three, &F::ZERO);
        gate.assert_bit(ctx, slot.parent_live);
        for value in [slot.parent_count, slot.parent_live] {
            let selected = gate.mul(ctx, Existing(slot.present), Existing(value));
            ctx.constrain_equal(&selected, &value);
            let bits = KagemushaSha256BitV4::decompose(ctx, gate, selected, 8);
            message.push(KagemushaSha256ByteV4::from_bits_le(ctx, gate, &bits));
        }
        for value in [slot.profile, slot.proof_step_count]
            .into_iter()
            .chain(slot.frozen_core_statement)
            .chain(slot.current_join)
            .chain(slot.parent_slots_digest)
        {
            range.range_check(ctx, value, 128);
            let selected = gate.mul(ctx, Existing(slot.present), Existing(value));
            // Canonical absence is literal zero in the verified parent tuple,
            // not merely a zero selected into the hash.  This removes hidden
            // malleable witness data from disabled/bootstrap slots.
            ctx.constrain_equal(&selected, &value);
            push_assigned_u128_le_v7(ctx, gate, &mut message, selected);
        }
    }
    let digest = sha_jobs.digest_constrained(ctx, &message)?;
    for (words, expected) in digest.chunks_exact(4).zip(expected_digest) {
        range.range_check(ctx, expected, 128);
        let packed = gate.inner_product(
            ctx,
            words.iter().copied(),
            (0..4).map(|index| Constant(F::from_u128(1_u128 << (32 * index)))),
        );
        ctx.constrain_equal(&packed, &expected);
    }
    Ok(digest)
}

/// Recover bytes preserved as two exact little-endian u128 public chunks.
pub(super) fn kagemusha_serialized_exact_chunks_to_bytes_v7(chunks: [u128; 2]) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..16].copy_from_slice(&chunks[0].to_le_bytes());
    bytes[16..].copy_from_slice(&chunks[1].to_le_bytes());
    bytes
}

/// Recover raw digest bytes from four packed big-endian SHA words per chunk.
pub(super) fn kagemusha_serialized_sha256_word_chunks_to_bytes_v7(chunks: [u128; 2]) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (chunk_index, chunk) in chunks.into_iter().enumerate() {
        for word_index in 0..4 {
            let word = (chunk >> (word_index * 32)) as u32;
            let byte_index = (chunk_index * 4 + word_index) * 4;
            bytes[byte_index..byte_index + 4].copy_from_slice(&word.to_be_bytes());
        }
    }
    bytes
}

/// Split a canonical 32-byte encoding into two exact little-endian u128 cells.
pub(super) fn kagemusha_serialized_bytes_to_chunks_v7(bytes: [u8; 32]) -> [u128; 2] {
    [
        u128::from_le_bytes(bytes[..16].try_into().expect("low 16 bytes")),
        u128::from_le_bytes(bytes[16..].try_into().expect("high 16 bytes")),
    ]
}

/// Recover one selected commitment from the structurally parsed PLONK witness
/// list.  The caller must obtain this slice from a fully consumed and verified
/// `PlonkProof::witnesses`; raw transcript offsets are never accepted here.
pub(super) fn kagemusha_selected_advice_commitment_v7<C>(
    witness_commitments: &[C],
    advice_column_phases: &[u8],
    selected_advice_column: usize,
) -> Result<(C, [u8; 32]), String>
where
    C: CurveAffine + PrimeCurveAffine,
{
    if witness_commitments.len() != advice_column_phases.len() {
        return Err("Kagemusha V7 parsed witness/phase vector length mismatch".to_owned());
    }
    let selected_phase = *advice_column_phases
        .get(selected_advice_column)
        .ok_or_else(|| "Kagemusha V7 selected advice column is out of range".to_owned())?;
    if selected_phase != 0 {
        return Err("Kagemusha V7 serialized advice column is not phase zero".to_owned());
    }
    let position = advice_column_phases
        .iter()
        .enumerate()
        .filter(|(index, phase)| {
            **phase < selected_phase
                || (**phase == selected_phase && *index < selected_advice_column)
        })
        .count();
    let point = *witness_commitments
        .get(position)
        .ok_or_else(|| "Kagemusha V7 selected parsed witness is absent".to_owned())?;
    if bool::from(point.is_identity()) {
        return Err("Kagemusha V7 selected advice commitment is the identity".to_owned());
    }
    let repr = point.to_bytes();
    if repr.as_ref().len() != 32 {
        return Err("Kagemusha V7 selected commitment encoding is not 32 bytes".to_owned());
    }
    let bytes: [u8; 32] = repr
        .as_ref()
        .try_into()
        .expect("validated 32-byte commitment encoding");
    let roundtrip = Option::<C>::from(C::from_bytes(&repr))
        .ok_or_else(|| "Kagemusha V7 selected advice commitment is noncanonical".to_owned())?;
    if roundtrip != point {
        return Err("Kagemusha V7 selected advice commitment did not round trip".to_owned());
    }
    Ok((point, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_sha256_v4::KagemushaSha256ConfigV4;
    use halo2_base::gates::circuit::{BaseConfig, builder::BaseCircuitBuilder};
    use halo2_base::halo2_proofs::halo2curves::{
        group::{Curve as _, GroupEncoding as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    };
    use halo2_base::halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        plonk::{Circuit, Error},
    };

    const TEST_K: u32 = 17;
    const TEST_UNUSABLE_ROWS: usize = 9;

    #[derive(Clone, Debug)]
    struct ParentDigestConfig<F: halo2_base::utils::ScalarField> {
        base: BaseConfig<F>,
        sha: KagemushaSha256ConfigV4,
    }

    #[derive(Clone)]
    struct ParentDigestCircuit<F: KagemushaSerializedAuditFieldV7> {
        builder: BaseCircuitBuilder<F>,
        jobs: KagemushaSha256JobsV4<F>,
    }

    impl<F> Circuit<F> for ParentDigestCircuit<F>
    where
        F: KagemushaSerializedAuditFieldV7,
    {
        type Config = ParentDigestConfig<F>;
        type FloorPlanner = V1;
        type Params = halo2_base::gates::circuit::BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }

        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows((1_usize << TEST_K) - TEST_UNUSABLE_ROWS);
            ParentDigestConfig {
                base,
                sha: KagemushaSha256ConfigV4::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("parent digest test uses parameterized Base config")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "Kagemusha V7 parent digest Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << TEST_K) - TEST_UNUSABLE_ROWS,
            )
        }
    }

    fn serialized_manifest() -> KagemushaSerializedAuditManifestV7 {
        KagemushaSerializedAuditManifestV7 {
            k: TEST_K,
            step_eq_params_sha256: std::array::from_fn(|index| 0x11 ^ index as u8),
            step_ep_params_sha256: std::array::from_fn(|index| 0x61 ^ index as u8),
            step_eq_vk_sha256: std::array::from_fn(|index| 0x21_u8.wrapping_add(index as u8)),
            step_ep_vk_sha256: std::array::from_fn(|index| 0x91_u8.wrapping_sub(index as u8)),
            step_eq_advice_phases: vec![0, 0, 0],
            step_ep_advice_phases: vec![0, 0, 0],
            step_eq_serialized_column: 2,
            step_ep_serialized_column: 2,
            step_eq_phase_zero_rank: 2,
            step_ep_phase_zero_rank: 2,
            step_eq_constraint_degree: 9,
            step_ep_constraint_degree: 9,
            step_eq_fixed_columns: 339,
            step_ep_fixed_columns: 339,
            step_eq_permutation_columns: 298,
            step_ep_permutation_columns: 298,
            step_eq_blinding_factors: 8,
            step_ep_blinding_factors: 8,
            minimum_unusable_rows: SERIALIZED_EXPECTED_UNUSABLE_ROWS_V7,
            step_eq_proof_bytes: SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7,
            step_ep_proof_bytes: SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7,
            eq_coefficient_count: 10_111,
            ep_coefficient_count: 10_111,
            public_instance_cells: 70,
            current_join_offset: 57,
            parent_digest_offset: 67,
            live_selector_offset: 69,
        }
    }

    fn challenge_context() -> KagemushaSerializedAuditChallengeContextV7 {
        KagemushaSerializedAuditChallengeContextV7::from_manifest(
            &serialized_manifest(),
            std::array::from_fn(|index| 0x41_u8.wrapping_add(3 * index as u8)),
        )
        .expect("valid serialized manifest")
    }

    fn parent_slots() -> [KagemushaSerializedParentSlotV7; 2] {
        [
            KagemushaSerializedParentSlotV7 {
                present: true,
                profile: 7,
                proof_step_count: 11,
                parent_count: 1,
                parent_live: true,
                frozen_core_statement: [13, 17],
                current_join: std::array::from_fn(|index| 19 + index as u128),
                parent_slots_digest: [31, 37],
            },
            KagemushaSerializedParentSlotV7 {
                present: true,
                profile: 41,
                proof_step_count: 43,
                parent_count: 2,
                parent_live: false,
                frozen_core_statement: [47, 53],
                current_join: std::array::from_fn(|index| 59 + index as u128),
                parent_slots_digest: [71, 73],
            },
        ]
    }

    fn parent_digest_circuit<F>(
        assigned_slots: [KagemushaSerializedParentSlotV7; 2],
        expected_slots: [KagemushaSerializedParentSlotV7; 2],
    ) -> ParentDigestCircuit<F>
    where
        F: KagemushaSerializedAuditFieldV7,
    {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let mut jobs = KagemushaSha256JobsV4::default();
        let assigned_slots = assigned_slots.map(|slot| KagemushaAssignedParentSlotV7 {
            present: builder
                .main(0)
                .load_witness(F::from(u64::from(slot.present))),
            profile: builder.main(0).load_witness(F::from_u128(slot.profile)),
            proof_step_count: builder
                .main(0)
                .load_witness(F::from_u128(slot.proof_step_count)),
            parent_count: builder
                .main(0)
                .load_witness(F::from(u64::from(slot.parent_count))),
            parent_live: builder
                .main(0)
                .load_witness(F::from(u64::from(slot.parent_live))),
            frozen_core_statement: slot
                .frozen_core_statement
                .map(|value| builder.main(0).load_witness(F::from_u128(value))),
            current_join: slot
                .current_join
                .map(|value| builder.main(0).load_witness(F::from_u128(value))),
            parent_slots_digest: slot
                .parent_slots_digest
                .map(|value| builder.main(0).load_witness(F::from_u128(value))),
        });
        let context = KagemushaSerializedParentDigestContextV7::from(&challenge_context());
        let assigned_context = KagemushaAssignedParentDigestContextV7 {
            profile_sha256_exact_chunks: kagemusha_serialized_bytes_to_chunks_v7(
                context.profile_sha256,
            )
            .map(|value| builder.main(0).load_witness(F::from_u128(value))),
            step_eq_vk_sha256_word_chunks: kagemusha_serialized_digest_word_chunks_v7(
                context.step_eq_vk_sha256,
            )
            .map(|value| builder.main(0).load_witness(F::from_u128(value))),
            step_ep_vk_sha256_word_chunks: kagemusha_serialized_digest_word_chunks_v7(
                context.step_ep_vk_sha256,
            )
            .map(|value| builder.main(0).load_witness(F::from_u128(value))),
            eq_coefficient_count: context.eq_coefficient_count,
            ep_coefficient_count: context.ep_coefficient_count,
        };
        let expected = kagemusha_serialized_digest_word_chunks_v7(
            kagemusha_serialized_parent_slots_digest_v7(&context, expected_slots)
                .expect("canonical expected parent slots"),
        )
        .map(|value| builder.main(0).load_witness(F::from_u128(value)));
        constrain_kagemusha_serialized_parent_slots_digest_v7(
            builder.main(0),
            &range,
            &mut jobs,
            assigned_context,
            assigned_slots,
            expected,
        )
        .expect("parent digest relation");
        builder.calculate_params(Some(TEST_UNUSABLE_ROWS));
        ParentDigestCircuit { builder, jobs }
    }

    fn verify_parent_digest<F>(
        assigned_slots: [KagemushaSerializedParentSlotV7; 2],
        expected_slots: [KagemushaSerializedParentSlotV7; 2],
    ) -> Result<(), Vec<halo2_base::halo2_proofs::dev::VerifyFailure>>
    where
        F: KagemushaSerializedAuditFieldV7,
    {
        MockProver::run(
            TEST_K,
            &parent_digest_circuit::<F>(assigned_slots, expected_slots),
            vec![],
        )
        .expect("parent digest circuit synthesis")
        .verify()
    }

    fn challenge_circuit<F>(
        mutate_commitment: bool,
        mutate_challenge: bool,
    ) -> ParentDigestCircuit<F>
    where
        F: KagemushaSerializedAuditFieldV7,
    {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let mut jobs = KagemushaSha256JobsV4::default();
        let host = challenge_context();
        let eq_bytes: [u8; 32] = EqAffine::generator()
            .to_bytes()
            .as_ref()
            .try_into()
            .expect("Eq commitment bytes");
        let ep_bytes: [u8; 32] = EpAffine::generator()
            .to_bytes()
            .as_ref()
            .try_into()
            .expect("Ep commitment bytes");
        let mut eq_chunks = kagemusha_serialized_bytes_to_chunks_v7(eq_bytes);
        if mutate_commitment {
            eq_chunks[0] ^= 1;
        }
        let ep_chunks = kagemusha_serialized_bytes_to_chunks_v7(ep_bytes);
        let assign_exact = |builder: &mut BaseCircuitBuilder<F>, bytes: [u8; 32]| {
            kagemusha_serialized_bytes_to_chunks_v7(bytes)
                .map(|value| builder.main(0).load_witness(F::from_u128(value)))
        };
        let assign_sha_words = |builder: &mut BaseCircuitBuilder<F>, bytes: [u8; 32]| {
            kagemusha_serialized_digest_word_chunks_v7(bytes)
                .map(|value| builder.main(0).load_witness(F::from_u128(value)))
        };
        let assigned_context = KagemushaAssignedAuditChallengeContextV7 {
            profile_sha256_exact_chunks: assign_exact(&mut builder, host.profile_sha256),
            step_eq_vk_sha256_word_chunks: assign_sha_words(&mut builder, host.step_eq_vk_sha256),
            step_ep_vk_sha256_word_chunks: assign_sha_words(&mut builder, host.step_ep_vk_sha256),
            frozen_core_statement_sha256_exact_chunks: assign_exact(
                &mut builder,
                host.frozen_core_statement_sha256,
            ),
            eq_coefficient_count: host.eq_coefficient_count,
            ep_coefficient_count: host.ep_coefficient_count,
        };
        let eq_chunks = eq_chunks.map(|value| builder.main(0).load_witness(F::from_u128(value)));
        let ep_chunks = ep_chunks.map(|value| builder.main(0).load_witness(F::from_u128(value)));
        let mut challenge = kagemusha_serialized_bytes_to_chunks_v7(
            kagemusha_serialized_audit_challenge_v7(&host, eq_bytes, ep_bytes)
                .expect("host challenge"),
        );
        if mutate_challenge {
            challenge[0] ^= 1;
        }
        let challenge = challenge.map(|value| builder.main(0).load_witness(F::from_u128(value)));
        constrain_kagemusha_serialized_audit_challenge_v7(
            builder.main(0),
            &range,
            &mut jobs,
            assigned_context,
            eq_chunks,
            ep_chunks,
            challenge,
        )
        .expect("constrained shared challenge");
        builder.calculate_params(Some(TEST_UNUSABLE_ROWS));
        ParentDigestCircuit { builder, jobs }
    }

    #[test]
    fn masked_challenge_is_ordered_and_unique_in_both_pasta_fields() {
        let eq = EqAffine::generator().to_bytes();
        let ep = EpAffine::generator().to_bytes();
        let eq: [u8; 32] = eq.as_ref().try_into().expect("Eq encoding");
        let ep: [u8; 32] = ep.as_ref().try_into().expect("Ep encoding");
        let challenge = kagemusha_serialized_audit_challenge_v7(&challenge_context(), eq, ep)
            .expect("nondegenerate challenge");
        assert_eq!(challenge[31] & 0xc0, 0);
        let mut fp_repr = <Fp as PrimeField>::Repr::default();
        fp_repr.as_mut().copy_from_slice(&challenge);
        let mut fq_repr = <Fq as PrimeField>::Repr::default();
        fq_repr.as_mut().copy_from_slice(&challenge);
        assert!(Option::<Fp>::from(Fp::from_repr(fp_repr)).is_some());
        assert!(Option::<Fq>::from(Fq::from_repr(fq_repr)).is_some());
        assert_ne!(
            challenge,
            kagemusha_serialized_audit_challenge_v7(&challenge_context(), ep, eq)
                .expect("swapped nondegenerate challenge")
        );
    }

    #[test]
    fn exact_chunks_and_sha_word_chunks_preserve_distinct_public_encodings() {
        let digest = std::array::from_fn(|index| {
            0x13_u8
                .wrapping_add((index as u8).wrapping_mul(7))
                .rotate_left((index % 7) as u32)
        });
        let exact = kagemusha_serialized_bytes_to_chunks_v7(digest);
        let words = kagemusha_serialized_digest_word_chunks_v7(digest);
        assert_ne!(
            exact, words,
            "non-palindromic digest must expose the endian distinction"
        );
        assert_eq!(kagemusha_serialized_exact_chunks_to_bytes_v7(exact), digest);
        assert_eq!(
            kagemusha_serialized_sha256_word_chunks_to_bytes_v7(words),
            digest
        );
        assert_ne!(
            kagemusha_serialized_exact_chunks_to_bytes_v7(words),
            digest,
            "raw LE decoding of SHA-word public chunks is the rejected mapping"
        );
    }

    #[test]
    fn manifest_binds_rng_schedule_geometry_layout_and_both_identities() {
        let manifest = serialized_manifest();
        let digest = manifest.sha256().expect("valid serialized manifest");
        assert_ne!(digest, [0; 32]);

        let mut mutations = Vec::new();
        let mut changed = manifest.clone();
        changed.step_eq_params_sha256[0] ^= 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_ep_params_sha256[0] ^= 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_eq_vk_sha256[0] ^= 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_ep_vk_sha256[0] ^= 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_eq_advice_phases.push(0);
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_ep_advice_phases.push(0);
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_eq_fixed_columns += 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_ep_fixed_columns += 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_eq_permutation_columns += 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.step_ep_permutation_columns += 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.eq_coefficient_count += 1;
        mutations.push(changed);
        let mut changed = manifest.clone();
        changed.ep_coefficient_count += 1;
        mutations.push(changed);
        for changed in mutations {
            assert_ne!(
                digest,
                changed.sha256().expect("still-valid manifest mutation")
            );
        }

        let mut wrong_rank = manifest.clone();
        wrong_rank.step_eq_phase_zero_rank -= 1;
        assert!(wrong_rank.validate().is_err());
        let mut wrong_phase = manifest.clone();
        wrong_phase.step_ep_advice_phases[2] = 1;
        assert!(wrong_phase.validate().is_err());
        let mut wrong_layout = manifest.clone();
        wrong_layout.current_join_offset += 1;
        assert!(wrong_layout.validate().is_err());
        let mut wrong_unusable = manifest.clone();
        wrong_unusable.minimum_unusable_rows += 1;
        assert!(wrong_unusable.validate().is_err());
        let mut wrong_eq_proof = manifest.clone();
        wrong_eq_proof.step_eq_proof_bytes -= 1;
        assert!(wrong_eq_proof.validate().is_err());
        let mut wrong_ep_proof = manifest.clone();
        wrong_ep_proof.step_ep_proof_bytes += 1;
        assert!(wrong_ep_proof.validate().is_err());
        let mut wrong_rows = manifest;
        wrong_rows.eq_coefficient_count = 131_063;
        assert!(wrong_rows.validate().is_err());
    }

    #[test]
    fn challenge_binds_every_frozen_context_item() {
        let eq = [0x51; 32];
        let ep = [0x62; 32];
        let original = kagemusha_serialized_audit_challenge_v7(&challenge_context(), eq, ep)
            .expect("challenge");
        let mut mutations = Vec::new();
        let mut context = challenge_context();
        context.profile_sha256[0] ^= 1;
        mutations.push(context);
        let mut context = challenge_context();
        context.step_eq_vk_sha256[0] ^= 1;
        mutations.push(context);
        let mut context = challenge_context();
        context.step_ep_vk_sha256[0] ^= 1;
        mutations.push(context);
        let mut context = challenge_context();
        context.frozen_core_statement_sha256[0] ^= 1;
        mutations.push(context);
        let mut context = challenge_context();
        context.eq_coefficient_count += 1;
        mutations.push(context);
        let mut context = challenge_context();
        context.ep_coefficient_count += 1;
        mutations.push(context);
        for context in mutations {
            assert_ne!(
                original,
                kagemusha_serialized_audit_challenge_v7(&context, eq, ep)
                    .expect("mutated challenge")
            );
        }
        let mut changed_eq = eq;
        changed_eq[0] ^= 1;
        assert_ne!(
            original,
            kagemusha_serialized_audit_challenge_v7(&challenge_context(), changed_eq, ep)
                .expect("Eq-mutated challenge")
        );
        let mut changed_ep = ep;
        changed_ep[0] ^= 1;
        assert_ne!(
            original,
            kagemusha_serialized_audit_challenge_v7(&challenge_context(), eq, changed_ep)
                .expect("Ep-mutated challenge")
        );
    }

    #[test]
    fn constrained_challenge_matches_host_in_both_pasta_fields() {
        for fp_ok in [
            MockProver::run(TEST_K, &challenge_circuit::<Fp>(false, false), vec![])
                .expect("Fp challenge synthesis")
                .verify()
                .is_ok(),
            MockProver::run(TEST_K, &challenge_circuit::<Fq>(false, false), vec![])
                .expect("Fq challenge synthesis")
                .verify()
                .is_ok(),
        ] {
            assert!(fp_ok);
        }
    }

    #[test]
    fn constrained_challenge_rejects_commitment_and_public_z_mutations() {
        for circuit in [
            challenge_circuit::<Fp>(true, false),
            challenge_circuit::<Fp>(false, true),
        ] {
            assert!(
                MockProver::run(TEST_K, &circuit, vec![])
                    .expect("mutated challenge synthesis")
                    .verify()
                    .is_err()
            );
        }
    }

    #[test]
    fn selected_commitment_parser_uses_exact_phase_zero_column() {
        let points = [
            (EqAffine::generator() * Fp::from(3)).to_affine(),
            (EqAffine::generator() * Fp::from(5)).to_affine(),
            (EqAffine::generator() * Fp::from(7)).to_affine(),
        ];
        let (parsed, bytes) =
            kagemusha_selected_advice_commitment_v7::<EqAffine>(&points, &[0, 0, 0], 1)
                .expect("selected commitment");
        assert_eq!(parsed, points[1]);
        assert_eq!(bytes.as_slice(), points[1].to_bytes().as_ref());
        assert!(
            kagemusha_selected_advice_commitment_v7::<EqAffine>(&points, &[0, 1, 0], 1).is_err()
        );
        assert!(
            kagemusha_selected_advice_commitment_v7::<EqAffine>(&points[..2], &[0], 0).is_err()
        );
    }

    #[test]
    fn public_and_ordered_parent_digest_cell_orders_are_fixed() {
        let join = KagemushaSerializedAuditPublicJoinV7 {
            step_eq_commitment: [1, 2],
            step_ep_commitment: [3, 4],
            challenge: [5, 6],
            eq_evaluation: [7, 8],
            ep_evaluation: [9, 10],
        };
        assert_eq!(join.cells(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let context = KagemushaSerializedParentDigestContextV7::from(&challenge_context());
        let slots = [
            KagemushaSerializedParentSlotV7 {
                present: true,
                profile: 7,
                proof_step_count: 11,
                parent_count: 1,
                parent_live: false,
                frozen_core_statement: [13, 17],
                current_join: join.cells(),
                parent_slots_digest: [19, 23],
            },
            KagemushaSerializedParentSlotV7 {
                present: true,
                profile: 29,
                proof_step_count: 31,
                parent_count: 2,
                parent_live: true,
                frozen_core_statement: [37, 41],
                current_join: std::array::from_fn(|index| 43 + index as u128),
                parent_slots_digest: [59, 61],
            },
        ];
        let digest = kagemusha_serialized_parent_slots_digest_v7(&context, slots)
            .expect("canonical parent slots");
        assert_ne!(digest, kagemusha_serialized_null_parent_digest_v7(&context));
        let chunks = kagemusha_serialized_digest_word_chunks_v7(digest);
        assert_ne!(chunks, [0; 2]);

        let mut mutations = Vec::new();
        for slot_index in 0..2 {
            let mut changed = slots;
            changed[slot_index].profile ^= 1;
            mutations.push(changed);
            let mut changed = slots;
            changed[slot_index].proof_step_count ^= 1;
            mutations.push(changed);
            let mut changed = slots;
            changed[slot_index].parent_count = (changed[slot_index].parent_count + 1) % 3;
            mutations.push(changed);
            let mut changed = slots;
            changed[slot_index].parent_live = !changed[slot_index].parent_live;
            mutations.push(changed);
            for index in 0..2 {
                let mut changed = slots;
                changed[slot_index].frozen_core_statement[index] ^= 1;
                mutations.push(changed);
            }
            for index in 0..AUDIT_CURRENT_JOIN_CELLS_V7 {
                let mut changed = slots;
                changed[slot_index].current_join[index] ^= 1;
                mutations.push(changed);
            }
            for index in 0..AUDIT_PARENT_DIGEST_CELLS_V7 {
                let mut changed = slots;
                changed[slot_index].parent_slots_digest[index] ^= 1;
                mutations.push(changed);
            }
        }
        mutations.push([slots[1], slots[0]]);
        for changed in mutations {
            assert_ne!(
                digest,
                kagemusha_serialized_parent_slots_digest_v7(&context, changed)
                    .expect("present mutated parent slots")
            );
        }
        let mut noncanonical_absent = slots;
        noncanonical_absent[0].present = false;
        assert!(
            kagemusha_serialized_parent_slots_digest_v7(&context, noncanonical_absent).is_err()
        );
        let mut canonical_absent = slots;
        canonical_absent[0] = KagemushaSerializedParentSlotV7::default();
        assert_ne!(
            digest,
            kagemusha_serialized_parent_slots_digest_v7(&context, canonical_absent)
                .expect("canonical absent parent slot")
        );

        let mut contexts = Vec::new();
        let mut changed = context;
        changed.profile_sha256[0] ^= 1;
        contexts.push(changed);
        let mut changed = context;
        changed.step_eq_vk_sha256[0] ^= 1;
        contexts.push(changed);
        let mut changed = context;
        changed.step_ep_vk_sha256[0] ^= 1;
        contexts.push(changed);
        let mut changed = context;
        changed.eq_coefficient_count += 1;
        contexts.push(changed);
        let mut changed = context;
        changed.ep_coefficient_count += 1;
        contexts.push(changed);
        for changed in contexts {
            assert_ne!(
                digest,
                kagemusha_serialized_parent_slots_digest_v7(&changed, slots)
                    .expect("present parent slots")
            );
        }
    }

    #[test]
    fn constrained_parent_digest_matches_host_in_both_pasta_fields() {
        let slots = parent_slots();
        assert!(slots[1].present && !slots[1].parent_live);
        assert!(verify_parent_digest::<Fp>(slots, slots).is_ok());
        assert!(verify_parent_digest::<Fq>(slots, slots).is_ok());
        let mut one_parent = slots;
        one_parent[1] = KagemushaSerializedParentSlotV7::default();
        assert!(verify_parent_digest::<Fp>(one_parent, one_parent).is_ok());
    }

    #[test]
    fn constrained_parent_digest_rejects_tuple_presence_and_order_mutations() {
        let expected = parent_slots();
        let mut changed_join = expected;
        changed_join[0].current_join[6] ^= 1;
        assert!(verify_parent_digest::<Fp>(changed_join, expected).is_err());

        let mut changed_core = expected;
        changed_core[1].frozen_core_statement[1] ^= 1;
        assert!(verify_parent_digest::<Fp>(changed_core, expected).is_err());

        let mut changed_count = expected;
        changed_count[0].parent_count ^= 1;
        assert!(verify_parent_digest::<Fp>(changed_count, expected).is_err());

        let mut changed_live = expected;
        changed_live[1].parent_live = true;
        assert!(verify_parent_digest::<Fp>(changed_live, expected).is_err());

        let mut changed_presence = expected;
        changed_presence[0].present = false;
        assert!(verify_parent_digest::<Fp>(changed_presence, expected).is_err());

        assert!(verify_parent_digest::<Fp>([expected[1], expected[0]], expected).is_err());
    }

    #[test]
    fn custom_polynomial_profile_is_one_permutation_column_only() {
        let mut constraints = ConstraintSystem::<Fp>::default();
        let config = KagemushaSerializedAuditConfigV7::configure(&mut constraints);
        assert_eq!(
            KagemushaSerializedAuditConfigV7::polynomial_profile(),
            (1, 0, 1)
        );
        assert_eq!(constraints.num_advice_columns(), 1);
        assert_eq!(constraints.num_fixed_columns(), 0);
        assert_eq!(constraints.num_selectors(), 0);
        assert_eq!(constraints.permutation().get_columns().len(), 1);
        assert_eq!(config.advice_column_index(), 0);
    }
}
