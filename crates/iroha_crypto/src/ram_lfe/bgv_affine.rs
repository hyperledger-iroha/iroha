//! Exact encrypted evaluation of an affine normal form under one padded schedule.
//!
//! Owner-side preparation copies the canonical normalizer's reduced forms into
//! one padded packed matrix. Each request masks the original 64 input slots,
//! adds its 32 initialized lanes and a constant-one slot, then uses the shared
//! BGV routing owner at the input's top level. No ciphertext operation is selected by the tape,
//! coefficient values or output count. The same public BGV transfer rules plan
//! and admit the complete schedule before private coefficients are used.
//!
//! This supplies an algorithm, not a qualified RAM-LFE backend. It does not
//! authenticate keys or inputs, prove normalization, sanitize ciphertext noise,
//! establish circuit privacy or complete bounded/refresh evaluation. In
//! particular, a decoded ciphertext's claimed fresh bound is not an input proof.
//! Production RAM-LFE refusals remain unchanged. Preparation must occur outside
//! an observable request; fixed operation shapes do not prove constant time.

use super::{
    RAM_LFE_V1_INPUT_SLOTS, RAM_LFE_V1_MAX_OUTPUTS, RAM_LFE_V1_NORMAL_INPUT_COLUMN,
    RAM_LFE_V1_NORMAL_LANE_COLUMN, RAM_LFE_V1_PLAINTEXT_MODULUS, RAM_LFE_V1_STATE_LANES,
    RamLfeClassV1, RamLfeInitialMemoryV1, RamLfeNormalFormV1, clearing::ClearingArray,
};
use iroha_fhe::bgv::{
    BgvError, BgvProfile, Ciphertext, CiphertextShape, EvaluationKeys, PlaintextPrecision,
    SlotEncoding, SlotLinearBudget, SlotLinearError, SlotLinearLayout, SlotLinearMap,
    SlotLinearPlan, SwitchingKeyKind, cost::CostModel,
};
use std::fmt;
use thiserror::Error;

const SLOTS: usize = 128;
const TRACE_STEPS: usize = 7;
const LANE_SLOT: usize = RAM_LFE_V1_INPUT_SLOTS;
const CONSTANT_SLOT: usize = LANE_SLOT + RAM_LFE_V1_STATE_LANES;
const WORDS: usize = SLOTS * SLOTS + SLOTS;
// The optimal 1-input/1-output 8-baby/16-giant routing split has 8 cached images,
// four giant ancestors and three immutable-add temporaries. Tighter noise can
// select a smaller split inside this same public residue bound.
const ROUTING_SCRATCH_CIPHERTEXTS: usize = 15;

/// Refusal of the unqualified affine BGV evaluation algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RamLfeBgvAffineErrorV1 {
    /// Only the affine class has an encrypted normal-form implementation here.
    #[error("BGV affine evaluation requires affine.v1")]
    ClassNotAffine,
    /// The algorithm requires F257 with 128 scalar slots.
    #[error("BGV affine evaluation requires 128 F257 slots")]
    UnsupportedProfile,
    /// The input must have the public shape of a fresh top-level encryption.
    #[error("BGV affine input does not have the required fresh encryption shape")]
    InputShape,
    /// A shared routing allocation, geometry or arithmetic check failed.
    #[error(transparent)]
    Routing(#[from] SlotLinearError),
    /// An underlying profile, key, arithmetic or noise admission check failed.
    #[error(transparent)]
    Bgv(#[from] BgvError),
}

/// Public noise and operation shape of the complete padded affine evaluation.
///
/// The plan contains no program, initialized lanes, input plaintext or secret
/// coefficients. A successfully planned profile holds every affine normal form
/// on a well-formed fresh encryption under this arithmetic schedule. This does
/// not include a circuit-privacy sanitizer, proof or end-to-end resource claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RamLfeBgvAffinePlanV1 {
    identity: [u8; 32],
    input: CiphertextShape,
    output: CiphertextShape,
    galois: [u32; TRACE_STEPS],
    routing: SlotLinearPlan<1, 1>,
}

impl RamLfeBgvAffinePlanV1 {
    /// Plan all 64 rows and the final mask at explicit effective F257 precision.
    ///
    /// The supplied context must belong to this master profile. Encryption and
    /// key-switch error still use the master p^E scale, including when E > 1.
    ///
    /// Mask the 64 input slots and add the current initialized lanes/one. Route
    /// the complete padded affine matrix through the shared BGV owner, then
    /// apply the fixed 64-output prefix mask. No private data selects a split,
    /// level, buffer budget or operation.
    ///
    /// # Errors
    /// Rejects an incompatible encoding or a profile that cannot hold the
    /// complete schedule's existing worst-case BGV noise transfers.
    pub fn new(
        profile: &BgvProfile,
        precision: PlaintextPrecision,
    ) -> Result<Self, RamLfeBgvAffineErrorV1> {
        if precision.modulus() != u64::from(RAM_LFE_V1_PLAINTEXT_MODULUS) {
            return Err(RamLfeBgvAffineErrorV1::UnsupportedProfile);
        }
        let encoding = SlotEncoding::new(profile, precision)?;
        if encoding.slots() != SLOTS {
            return Err(RamLfeBgvAffineErrorV1::UnsupportedProfile);
        }
        let galois = encoding
            .broadcast_elements()
            .try_into()
            .map_err(|_| RamLfeBgvAffineErrorV1::UnsupportedProfile)?;
        let input = profile.plan_fresh(precision)?;
        let mask_bound = encoding.public_prefix_mask_bound(RAM_LFE_V1_INPUT_SLOTS)?;
        let masked = profile.plan_mul_plain(&input, mask_bound)?;
        let augmented = profile.plan_add_plain(&masked, encoding.vector_bound())?;
        let residue_bytes = usize::try_from(
            CostModel::new(profile).ciphertext_residue_bytes(profile.top_level(), 2)?,
        )
        .map_err(|_| SlotLinearError::GeometryOverflow)?;
        let routing = SlotLinearPlan::new(
            profile,
            &[augmented],
            SlotLinearBudget {
                coefficient_bytes: WORDS * std::mem::size_of::<u32>(),
                layout_metadata_bytes: SlotLinearLayout::<1, 1>::metadata_bytes_for(SLOTS, SLOTS)?,
                preparation_scratch_bytes: (SLOTS + 1) * std::mem::size_of::<u32>(),
                output_residue_bytes: residue_bytes,
                scratch_residue_bytes: residue_bytes
                    .checked_mul(ROUTING_SCRATCH_CIPHERTEXTS)
                    .ok_or(SlotLinearError::GeometryOverflow)?,
            },
        )?;
        let output = profile.plan_mul_plain(
            &routing.output_shapes()[0],
            encoding.public_prefix_mask_bound(RAM_LFE_V1_MAX_OUTPUTS)?,
        )?;
        Ok(Self {
            identity: *profile.identity(),
            input,
            output,
            galois,
            routing,
        })
    }

    /// Exact public shape of the output of every admitted evaluation.
    #[must_use]
    pub const fn output_shape(&self) -> &CiphertextShape {
        &self.output
    }

    /// Fixed Galois keys used by the shared packed routing schedule.
    #[must_use]
    pub const fn galois_elements(&self) -> &[u32; TRACE_STEPS] {
        &self.galois
    }

    /// Galois applications per request, including all padded matrix work.
    #[must_use]
    pub const fn galois_applications(&self) -> usize {
        self.routing.galois_applications()
    }

    fn check_inputs(
        &self,
        profile: &BgvProfile,
        input: &Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<(), RamLfeBgvAffineErrorV1> {
        if profile.identity() != &self.identity || input.profile_identity() != &self.identity {
            return Err(BgvError::ProfileMismatch.into());
        }
        if input.shape() != self.input {
            return Err(RamLfeBgvAffineErrorV1::InputShape);
        }
        // Validate the entire fixed key set before reading private coefficients.
        for &element in &self.galois {
            let key = keys.galois(element).ok_or(BgvError::WrongSwitchingKey)?;
            if key.profile_identity() != &self.identity {
                return Err(BgvError::ProfileMismatch.into());
            }
            if key.kind() != SwitchingKeyKind::Galois(element) {
                return Err(BgvError::WrongSwitchingKey.into());
            }
        }
        Ok(())
    }
}

/// Private padded coefficients prepared from the canonical affine normalizer.
///
/// The shared clearing owner contains 128 rows of 128 coefficients plus fixed
/// zero biases. The first 64 rows map canonical inputs, initialized lanes and
/// the constant; every other row is zero. Initial lanes are supplied anew on
/// each request, so preparation is independent of associated data. The owner
/// has no wire encoding or public function commitment: the tape remains the
/// committed function, and preparation proves nothing to a remote verifier.
pub struct RamLfeBgvAffineProgramV1 {
    identity: [u8; 32],
    routing: SlotLinearMap<1, 1>,
}

impl fmt::Debug for RamLfeBgvAffineProgramV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED RAM-LFE affine BGV program]")
    }
}

impl RamLfeBgvAffineProgramV1 {
    /// Prepare the full padded coefficient owner before serving requests.
    ///
    /// # Errors
    /// Rejects a non-affine normal form before allocating private storage, or
    /// a shared routing allocation/validation error. The public plan binds
    /// the prepared map to its encryption profile.
    pub fn from_normal_form(
        normal: &RamLfeNormalFormV1,
        plan: &RamLfeBgvAffinePlanV1,
    ) -> Result<Self, RamLfeBgvAffineErrorV1> {
        if normal.class() != RamLfeClassV1::Affine {
            return Err(RamLfeBgvAffineErrorV1::ClassNotAffine);
        }
        // This temporary is cleared after transfer to the shared matrix owner;
        // there is no second persistent affine coefficient implementation.
        let mut coefficients = ClearingArray::<u32, WORDS>::zeroed();
        for row in 0..RAM_LFE_V1_MAX_OUTPUTS {
            if let Some(form) = normal.output_form(row) {
                let start = row * SLOTS;
                for slot in 0..RAM_LFE_V1_INPUT_SLOTS {
                    coefficients[start + slot] =
                        u32::from(form[RAM_LFE_V1_NORMAL_INPUT_COLUMN + slot]);
                }
                for lane in 0..RAM_LFE_V1_STATE_LANES {
                    coefficients[start + LANE_SLOT + lane] =
                        u32::from(form[RAM_LFE_V1_NORMAL_LANE_COLUMN + lane]);
                }
                coefficients[start + CONSTANT_SLOT] = u32::from(form[0]);
            }
        }
        Ok(Self {
            identity: plan.identity,
            routing: SlotLinearMap::new(&plan.routing, &coefficients[..])?,
        })
    }

    /// Evaluate the complete packed affine map through shared BGV routing.
    ///
    /// The result contains all 64 padded outputs, followed by 64 zero slots.
    /// The evaluator knows the initialized lanes, but never obtains an input
    /// plaintext or decryption key. Arithmetic automatically uses the selected
    /// shared SIMD backend with its identical scalar fallback.
    ///
    /// # Errors
    /// Rejects a mismatched profile, input shape or required key before private
    /// work, or an underlying BGV operation error. A caller must separately
    /// authenticate input/key well-formedness; passing these checks is not proof
    /// that a ciphertext's claimed noise or scalar-subalgebra membership holds.
    pub fn evaluate(
        &self,
        profile: &BgvProfile,
        plan: &RamLfeBgvAffinePlanV1,
        memory: &RamLfeInitialMemoryV1,
        input: &Ciphertext,
        keys: &EvaluationKeys,
    ) -> Result<Ciphertext, RamLfeBgvAffineErrorV1> {
        if self.identity != plan.identity {
            return Err(BgvError::ProfileMismatch.into());
        }
        plan.check_inputs(profile, input, keys)?;
        let encoding = SlotEncoding::new(profile, plan.input.plaintext_precision())?;
        // Upper input slots are ignored even before an input proof exists.
        let masked = profile.mul_plain(
            input,
            &encoding.public_prefix_mask(profile, RAM_LFE_V1_INPUT_SLOTS)?,
        )?;
        let mut initialization = ClearingArray::<u64, SLOTS>::zeroed();
        for (slot, &lane) in initialization[LANE_SLOT..CONSTANT_SLOT]
            .iter_mut()
            .zip(memory.lanes())
        {
            *slot = u64::from(lane);
        }
        initialization[CONSTANT_SLOT] = 1;
        let packed = encoding.encode(profile, &initialization[..])?;
        let augmented = profile.add_plain(&masked, &packed)?;
        drop(masked);
        drop(packed);
        drop(initialization);
        let mut routed = self
            .routing
            .evaluate(profile, &plan.routing, &[augmented], keys)?;
        let value = routed.pop().ok_or(BgvError::Kernel)?;
        debug_assert!(routed.is_empty());
        let output = profile.mul_plain(
            &value,
            &encoding.public_prefix_mask(profile, RAM_LFE_V1_MAX_OUTPUTS)?,
        )?;
        debug_assert_eq!(output.shape(), plan.output);
        Ok(output)
    }
}

#[cfg(test)]
#[path = "bgv_affine_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bgv_affine_maximum_tests.rs"]
mod maximum_tests;
