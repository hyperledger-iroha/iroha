//! Checked typed payload accounting for prepared public arithmetic plans.
//!
//! Requirements bound requested Rust object/element storage, not allocator
//! headers, compiler stack frames, allocation over-reservation or process RSS.
//! They are architecture-local engineering limits, not consensus policy. The
//! separate coefficient/layout/upper-DAG budgets retain their existing scopes.

use core::mem::size_of;
use thiserror::Error;

use super::{BgvError, BgvProfile, CiphertextShape, PlaintextPrecision, SlotEncoding};
use crate::natural::Natural;

/// A checked public accounting failure; no arithmetic authority is created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PlanMetadataError {
    /// A public count or byte sum cannot be represented.
    #[error("prepared plan metadata geometry overflow")]
    Overflow,
    /// The public program and effective context do not describe one bootstrap.
    #[error("prepared plan metadata context mismatch")]
    Context,
    /// An existing profile or encoding validation failed.
    #[error(transparent)]
    Bgv(#[from] BgvError),
}

/// Typed retained storage and additional sequential preparation scratch.
///
/// An enclosing owner counts its own inline struct once and adds nested owners'
/// `retained_heap_bytes`; independently owned vector elements count both fields.
/// `preparation_scratch_bytes` is additional to all retained bytes, including a
/// routing search's simultaneous best/candidate/cache and rejected transfers.
/// Matrix coefficients, shared layout graphs and upper-DAG element storage are
/// separately charged by their existing budgets. Every inline handle is counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanMetadataRequirement {
    /// Rust owning object, including its inline nested fields and handles.
    pub inline_bytes: usize,
    /// Dynamic metadata outside separately charged allocation graphs.
    pub retained_heap_bytes: usize,
    /// Additional typed/element storage during sequential public preparation.
    pub preparation_scratch_bytes: usize,
}

impl PlanMetadataRequirement {
    /// Complete retained typed storage in this scope.
    ///
    /// # Errors
    /// Refuses byte overflow.
    pub fn retained_bytes(self) -> Result<usize, PlanMetadataError> {
        add(self.inline_bytes, self.retained_heap_bytes)
    }

    /// Retained storage plus the additional preparation scratch bound.
    ///
    /// # Errors
    /// Refuses byte overflow.
    pub fn peak_bytes(self) -> Result<usize, PlanMetadataError> {
        add(self.retained_bytes()?, self.preparation_scratch_bytes)
    }

    /// Bound sequential preparation of `count` independent retained owners.
    ///
    /// Only one scratch set is live at a time. A zero count needs no storage.
    /// The enclosing container's header is charged by its own owner.
    ///
    /// # Errors
    /// Refuses count/byte overflow.
    pub fn repeated(self, count: usize) -> Result<Self, PlanMetadataError> {
        Ok(Self {
            inline_bytes: mul(self.inline_bytes, count)?,
            retained_heap_bytes: mul(self.retained_heap_bytes, count)?,
            preparation_scratch_bytes: if count == 0 {
                0
            } else {
                self.preparation_scratch_bytes
            },
        })
    }
}

pub(super) fn add(a: usize, b: usize) -> Result<usize, PlanMetadataError> {
    a.checked_add(b).ok_or(PlanMetadataError::Overflow)
}
pub(super) fn mul(a: usize, b: usize) -> Result<usize, PlanMetadataError> {
    a.checked_mul(b).ok_or(PlanMetadataError::Overflow)
}
pub(super) fn sum(values: &[usize]) -> Result<usize, PlanMetadataError> {
    values.iter().try_fold(0, |total, &value| add(total, value))
}
pub(super) fn words(count: usize) -> Result<usize, PlanMetadataError> {
    mul(count, size_of::<u64>())
}
pub(super) fn shape_heap_bound(profile: &BgvProfile) -> Result<usize, PlanMetadataError> {
    // Admitted significant length <=W. Tensor requests2W then degree scaling
    // requests at most2W+1; its result is admitted BEFORE key-switch addition.
    // Other retained transfers request <=W+1; clone uses significant length.
    words(add(mul(2, profile.noise_bound_words())?, 1)?)
}
pub(super) fn shape_heap(shape: &CiphertextShape) -> Result<usize, PlanMetadataError> {
    words(shape.bound().allocated_words())
}
pub(super) fn shape_value_bound(profile: &BgvProfile) -> Result<usize, PlanMetadataError> {
    add(size_of::<CiphertextShape>(), shape_heap_bound(profile)?)
}
pub(super) fn geometry(
    profile: &BgvProfile,
    precision: PlaintextPrecision,
) -> Result<(usize, usize, usize, usize), PlanMetadataError> {
    let slots = SlotEncoding::slot_count(profile, precision)?;
    let banks = profile.degree() / slots;
    Ok((slots, banks, slots.ilog2() as usize, banks.ilog2() as usize))
}

/// Simultaneous Natural temporaries of every canonical transfer used here.
/// Sum all expression temporaries, even when Rust could drop them earlier.
pub(super) fn transfer_scratch(profile: &BgvProfile) -> Result<usize, PlanMetadataError> {
    let w = profile.noise_bound_words();
    let mut key_words = 0;
    for level in 0..=profile.top_level() {
        key_words = key_words.max(profile.key_switch_noise(level)?.words_le().len());
    }
    let objects = |count, payload| add(mul(count, size_of::<Natural>())?, words(payload)?);
    // Rejected tensor: 2W-word raw product and 2W+1 degree-scaled product.
    let product = objects(2, add(mul(4, w)?, 1)?)?;
    // A key error need not itself fit capacity. Read its public significant
    // length rather than assuming W on a refusal path.
    let key = objects(1, add(w.max(key_words), 1)?)?;
    let plain = objects(2, add(w, 2)?)?; // add_u64's one-word argument + sum.
    // fresh_noise: nine constructor/intermediate Naturals; master<2^31,
    // degree<=2^17 and even a u64 error bound leave product length<=2 words.
    let fresh = objects(9, 16)?;
    // switch_down_noise: q/2*T*(N+1) has <=2 significant words. Seven
    // rounding-expression allocations request11 words in total; the sum
    // and quotient each request at most max(W,2)+1, also on refusal.
    let switching = objects(9, add(11, mul(2, add(w.max(2), 1)?)?)?)?;
    // Q may have one more significant word than capacity=(Q-1)/2.
    // level_modulus constructs it through product_of: the last mul_u64 can
    // reserve one additional word. Strict carry then owns Q, V*2a,
    // Q*(N+1), their sum and Q*a simultaneously: at most5W+10 words.
    let carry = objects(5, add(mul(5, w)?, 10)?)?;
    // Q construction also owns a public main-prime Vec while its fold keeps
    // an old/new product. This is not borrowed profile storage.
    let modulus = add(
        objects(2, add(mul(2, w)?, 3)?)?,
        add(size_of::<Vec<u64>>(), words(profile.top_level() + 1)?)?,
    )?;
    Ok(product
        .max(key)
        .max(plain)
        .max(fresh)
        .max(switching)
        .max(carry)
        .max(modulus))
}

/// SlotEncoding::new's public interpolation buffer; its inline value is Copy.
pub(super) fn encoding_scratch(slots: usize) -> Result<usize, PlanMetadataError> {
    sum(&[
        size_of::<SlotEncoding>(),
        size_of::<Vec<u64>>(),
        words(slots)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bgv::BgvParameters;

    #[test]
    fn checked_composition_counts_sequential_scratch_and_refuses_overflow() {
        let one = PlanMetadataRequirement {
            inline_bytes: 17,
            retained_heap_bytes: 31,
            preparation_scratch_bytes: 43,
        };
        assert_eq!(one.retained_bytes().unwrap(), 48);
        assert_eq!(one.peak_bytes().unwrap(), 91);
        assert_eq!(
            one.repeated(21).unwrap().peak_bytes().unwrap(),
            21 * 48 + 43
        );
        assert_eq!(one.repeated(0).unwrap(), PlanMetadataRequirement::default());
        assert_eq!(one.repeated(usize::MAX), Err(PlanMetadataError::Overflow));
        assert_eq!(
            PlanMetadataRequirement {
                inline_bytes: usize::MAX,
                ..one
            }
            .retained_bytes(),
            Err(PlanMetadataError::Overflow)
        );
    }

    #[test]
    fn refused_tensor_and_carry_temporaries_have_distinct_public_capacity_bounds() {
        let profile =
            BgvProfile::new(BgvParameters::derive(4, 5, 3, (50, 5), (60, 2), 2).unwrap()).unwrap();
        let w = profile.noise_bound_words();
        let a = Natural::from_words_le(&vec![u64::MAX; w]);
        let product = a.mul(&a);
        let scaled = product.mul_u64(profile.degree() as u64);
        assert!(product.allocated_words() <= 2 * w);
        assert!(scaled.allocated_words() <= 2 * w + 1);
        let observed =
            2 * size_of::<Natural>() + 8 * (product.allocated_words() + scaled.allocated_words());
        assert!(observed <= transfer_scratch(&profile).unwrap());
        let precision = profile.plaintext_precision(1).unwrap();
        let admitted = profile
            .plan_claimed(
                precision,
                profile.top_level(),
                2,
                profile.capacity(profile.top_level()).unwrap().clone(),
            )
            .unwrap();
        assert!(profile.plan_tensor(&admitted, &admitted).is_err());
        assert!(shape_heap(&admitted).unwrap() <= shape_heap_bound(&profile).unwrap());
        assert_eq!(geometry(&profile, precision).unwrap(), (2, 8, 1, 3));
        // A caller can supply an admitted value with excessive spare capacity.
        // Inspection must report that capacity rather than normalized length;
        // aggregate admission can then refuse it before private execution.
        let mut padded = vec![0_u64; 2 * w + 99];
        padded[0] = 1;
        let oversized = profile
            .plan_claimed(
                precision,
                profile.top_level(),
                2,
                Natural::from_words_le(&padded),
            )
            .unwrap();
        assert_eq!(oversized.bound().words_le(), &[1]);
        assert!(
            oversized.retained_metadata().unwrap().retained_heap_bytes
                > CiphertextShape::metadata_requirement(&profile)
                    .unwrap()
                    .retained_heap_bytes
        );
    }
    #[test]
    fn public_f257_single_and_512_bank_geometry_is_bounded_without_prepared_maps() {
        use crate::prime_power::{PrimePowerContext, UpperDigitsBudget, UpperDigitsPlan};
        let context = PrimePowerContext::new(257, 3).unwrap();
        let g = UpperDigitsPlan::required(context, 1).unwrap();
        let blueprint = UpperDigitsPlan::new(
            context,
            1,
            UpperDigitsBudget {
                coefficient_bytes: g.coefficient_bytes,
                preparation_scratch_bytes: g.preparation_scratch_bytes,
                program_bytes: g.program_bytes,
                preparation_operations: g.preparation_operations,
                live_values: g.live_values,
            },
        )
        .unwrap();
        let profiles: Vec<_> = [7, 16]
            .into_iter()
            .map(|log| {
                BgvProfile::new(BgvParameters::derive(log, 257, 3, (60, 4), (60, 2), 2).unwrap())
                    .unwrap()
            })
            .collect();
        let requirements: Vec<_> = profiles
            .iter()
            .map(|profile| {
                super::super::BootstrapPlan::metadata_requirement(
                    profile,
                    profile.plaintext_precision(1).unwrap(),
                    &blueprint,
                )
                .unwrap()
            })
            .collect();
        assert_eq!(
            geometry(&profiles[0], profiles[0].master_plaintext()).unwrap(),
            (128, 1, 7, 0)
        );
        assert_eq!(
            geometry(&profiles[1], profiles[1].master_plaintext()).unwrap(),
            (128, 512, 7, 9)
        );
        assert_eq!(requirements[0].inline_bytes, requirements[1].inline_bytes);
        assert!(requirements[1].retained_heap_bytes > requirements[0].retained_heap_bytes);
        assert!(requirements[1].peak_bytes().unwrap() > requirements[0].peak_bytes().unwrap());
        for (profile, requirement) in profiles.iter().zip(requirements) {
            assert!(
                requirement.repeated(21).unwrap().peak_bytes().unwrap()
                    > requirement.peak_bytes().unwrap()
            );
            let class = super::super::SlotLinearPlan::<21, 60>::metadata_requirement(
                profile,
                profile.plaintext_precision(1).unwrap(),
            )
            .unwrap();
            assert!(
                class.preparation_scratch_bytes >= (21 * 128) * shape_value_bound(profile).unwrap()
            );
        }
    }
}
