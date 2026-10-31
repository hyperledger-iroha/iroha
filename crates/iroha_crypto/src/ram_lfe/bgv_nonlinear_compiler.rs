//! Owner-side fixed matrix compilation for admitted nonlinear RAM-LFE classes.
//!
//! A single clearing owner copies canonical operands and ordered snapshots.
//! Bounded uses sixteen rank layers, twenty state banks and ten slots per gate.
//! Refresh retains three state banks and updates only the public target bank
//! at each of 319 mixed gate ordinals. Nine physical steps use a condition square,
//! seven repeated squares and the canonical result. Three unique maps cover
//! those steps; prior local slots carry through C and future slots stay zero.
//! The redundant unit step is omitted uniformly on this closed reachable domain.
//!
//! Preparation alone may inspect private kinds or coefficients. Every map's
//! exact public support is checked before those private reads. The base bank
//! contains 64 input slots, 32 current lanes, one and zero padding. No request
//! memory is captured. Logical Refresh admission and stored/aliased forms remain
//! those of the original class-first normalizer, regardless of physical padding.
//!
//! The sibling nonlinear runtime consumes both class geometries with one closed
//! encrypted evaluator. TODO(R.9): complete end-to-end application proofs and
//! resource qualification. This compiler alone does not establish input
//! authentication, a committed lowering proof, circuit privacy or parameter
//! qualification; those production refusals remain unchanged.

use iroha_fhe::bgv::{
    SlotLinearError, SlotLinearLayout, SlotLinearMap, SlotLinearPlan, SlotLinearSegment,
};
use std::{
    fmt,
    mem::size_of,
    ops::{Deref, DerefMut},
};
use thiserror::Error;

use super::{
    RAM_LFE_V1_BOUNDED_MAX_RANK, RAM_LFE_V1_MAX_INSTRUCTIONS, RAM_LFE_V1_MAX_OUTPUTS,
    RAM_LFE_V1_NORMAL_BASE_COLUMNS, RAM_LFE_V1_PLAINTEXT_MODULUS, RAM_LFE_V1_REFRESH_MAX_REFRESHES,
    RamLfeClassV1, RamLfeNormalFormV1, RamLfeNormalGateKindV1, clearing::ClearingArray,
};

const SLOTS: usize = 128;
const GATES: usize = RAM_LFE_V1_MAX_INSTRUCTIONS - 1;
const PRIMITIVES_PER_GATE: usize = 10;
const STATE_BANKS: usize = 20;
const INPUT_BANKS: usize = STATE_BANKS + 1;
#[cfg(test)]
const OPERAND_BANKS: usize = STATE_BANKS * 3;
const PHYSICAL: usize = STATE_BANKS * SLOTS;
const BASE_BANK: usize = PHYSICAL;
const ONE: usize = BASE_BANK + 96;
const ROW_CELLS: usize = INPUT_BANKS * SLOTS;
const COLUMNS: usize = RAM_LFE_V1_NORMAL_BASE_COLUMNS + GATES;
const OPERAND_CELLS: usize = GATES * 3 * COLUMNS;
const OUTPUT_CELLS: usize = RAM_LFE_V1_MAX_OUTPUTS * COLUMNS;
const KINDS: usize = OPERAND_CELLS + OUTPUT_CELLS;
const READY: usize = KINDS + GATES;
const PRIVATE_CELLS: usize = READY + PHYSICAL;
const LAYER_CELLS: usize = 294_270;
const REFRESH_GATES: usize = GATES + RAM_LFE_V1_REFRESH_MAX_REFRESHES as usize;
const REFRESH_COLUMNS: usize = RAM_LFE_V1_NORMAL_BASE_COLUMNS + REFRESH_GATES;
const REFRESH_PHYSICAL: usize = 384;
const REFRESH_BASE: usize = REFRESH_PHYSICAL;
const REFRESH_ONE: usize = REFRESH_BASE + 96;
const REFRESH_OPERANDS: usize = REFRESH_GATES * 3 * REFRESH_COLUMNS;
const REFRESH_KINDS: usize = REFRESH_OPERANDS + RAM_LFE_V1_MAX_OUTPUTS * REFRESH_COLUMNS;
const REFRESH_PRIVATE_CELLS: usize = REFRESH_KINDS + REFRESH_GATES;
const MODULUS: u16 = RAM_LFE_V1_PLAINTEXT_MODULUS;

const _: () = {
    assert!(GATES == 255);
    assert!(REFRESH_GATES == 319 && REFRESH_COLUMNS == 416);
    assert!(RAM_LFE_V1_BOUNDED_MAX_RANK == 16);
    assert!(GATES * PRIMITIVES_PER_GATE == PHYSICAL - 10);
};

/// A refusal during private nonlinear coefficient preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RamLfeBgvNonlinearCompileErrorV1 {
    /// The original tape must be admitted as bounded.v1 or refresh.v1.
    #[error("nonlinear matrix compilation requires bounded.v1 or refresh.v1")]
    ClassNotNonlinear,
    /// The public stage, routing dimensions or normal-form class disagree.
    #[error("nonlinear compilation class or public geometry mismatch")]
    ClassMismatch,
    /// The fixed private compiler allocation exceeds the caller's public ceiling.
    #[error("nonlinear compiler requires {required} private payload bytes, budget {budget}")]
    PrivateBudget {
        /// Fixed number of private scalar payload bytes.
        required: usize,
        /// Caller-supplied public byte ceiling.
        budget: usize,
    },
    /// Publicly sized compiler or row storage could not be reserved.
    #[error("nonlinear compiler storage reservation failed")]
    Allocation,
    /// The public layer or gate index is outside its fixed class schedule.
    #[error("nonlinear compiler stage index is outside its fixed schedule")]
    Layer,
    /// Coefficient arithmetic is defined only for 128 F257 scalar slots.
    #[error("nonlinear matrix compilation requires 128 F257 slots")]
    UnsupportedProfile,
    /// The normal-form object violates the already-admitted nonlinear shape.
    #[error("nonlinear normal form violates its fixed shape or depth")]
    InvalidNormalForm,
    /// An underlying public routing allocation or coefficient check failed.
    #[error(transparent)]
    Routing(#[from] SlotLinearError),
}

/// Public physical geometry of an originally admitted nonlinear class.
///
/// This descriptor supplies no class admission: the compiler binds it to the
/// original normal form. Bounded uses sixteen rank layers; Refresh uses all
/// 319 mixed canonical gate positions and nine physical steps at each position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamLfeBgvNonlinearShapeV1 {
    /// Twenty state banks and one base bank, with ten slots per nonlinear gate.
    Bounded,
    /// Three state banks and one base bank, with one slot per mixed gate.
    Refresh,
}

/// One of three unique maps at each public refresh-class gate position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamLfeBgvRefreshMapV1 {
    /// Step zero: Select condition square; other kinds start from zero.
    Initial,
    /// Steps one through seven reuse this same target-slot squaring map.
    Square,
    /// Step eight computes the canonical Mul, Select, Refresh or padding result.
    Final,
}

/// A public nonlinear layer position; it never depends on private gate kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamLfeBgvNonlinearStageV1 {
    /// One-based bounded rank layer, exactly 1 through 16.
    BoundedLayer(u16),
    /// Zero-based mixed gate position, exactly 0 through 318.
    RefreshGate {
        /// Public canonical ordinal including padded absent gates.
        gate: u16,
        /// One of the three fixed maps in the nine-step schedule.
        mode: RamLfeBgvRefreshMapV1,
    },
}

impl RamLfeBgvNonlinearStageV1 {
    fn shape(self) -> RamLfeBgvNonlinearShapeV1 {
        match self {
            Self::BoundedLayer(_) => RamLfeBgvNonlinearShapeV1::Bounded,
            Self::RefreshGate { .. } => RamLfeBgvNonlinearShapeV1::Refresh,
        }
    }

    fn validate(self) -> Result<(), RamLfeBgvNonlinearCompileErrorV1> {
        let valid = match self {
            Self::BoundedLayer(layer) => (1..=16).contains(&layer),
            Self::RefreshGate { gate, .. } => usize::from(gate) < REFRESH_GATES,
        };
        if valid {
            Ok(())
        } else {
            Err(RamLfeBgvNonlinearCompileErrorV1::Layer)
        }
    }
}

impl RamLfeBgvNonlinearShapeV1 {
    /// The original normal-form class represented by this public geometry.
    #[must_use]
    pub const fn class(self) -> RamLfeClassV1 {
        match self {
            Self::Bounded => RamLfeClassV1::Bounded,
            Self::Refresh => RamLfeClassV1::Refresh,
        }
    }

    /// Exact clearing compiler payload, excluding headers and the source form.
    #[must_use]
    pub const fn private_payload_bytes(self) -> usize {
        (match self {
            Self::Bounded => PRIVATE_CELLS,
            Self::Refresh => REFRESH_PRIVATE_CELLS,
        }) * size_of::<u16>()
    }

    /// Exact maximum layer coefficient payload for this public class schedule.
    /// Bounded layers all have this size; refresh gate/mode maps may be smaller.
    #[must_use]
    pub const fn maximum_layer_payload_bytes(self) -> usize {
        (match self {
            Self::Bounded => LAYER_CELLS,
            Self::Refresh => 3 * (REFRESH_GATES - 1) + 292 + (REFRESH_GATES - 1) % SLOTS,
        }) * size_of::<u32>()
    }

    /// Exact coefficient payload of the complete ordered-output projection.
    #[must_use]
    pub const fn projection_payload_bytes(self) -> usize {
        RAM_LFE_V1_MAX_OUTPUTS * self.columns() * size_of::<u32>()
    }

    /// One clearing logical row, including its structural-zero bias coordinate.
    #[must_use]
    pub const fn preparation_scratch_bytes(self) -> usize {
        (self.input_banks() * SLOTS + 1) * size_of::<u32>()
    }

    const fn gates(self) -> usize {
        match self {
            Self::Bounded => GATES,
            Self::Refresh => REFRESH_GATES,
        }
    }
    const fn columns(self) -> usize {
        match self {
            Self::Bounded => COLUMNS,
            Self::Refresh => REFRESH_COLUMNS,
        }
    }
    const fn input_banks(self) -> usize {
        match self {
            Self::Bounded => INPUT_BANKS,
            Self::Refresh => 4,
        }
    }
    const fn operands(self) -> usize {
        self.gates() * 3 * self.columns()
    }
    const fn kinds(self) -> usize {
        self.operands() + RAM_LFE_V1_MAX_OUTPUTS * self.columns()
    }

    fn support(
        self,
        stage: Option<RamLfeBgvNonlinearStageV1>,
        row: usize,
    ) -> ([SlotLinearSegment; 3], usize) {
        match self {
            Self::Bounded => {
                if stage.is_some() {
                    RamLfeBgvNonlinearCompilerV1::row_support::<60>(row)
                } else {
                    RamLfeBgvNonlinearCompilerV1::row_support::<1>(row)
                }
            }
            Self::Refresh => refresh_support(stage, row),
        }
    }

    fn check_geometry<const I: usize, const O: usize>(
        self,
        stage: Option<RamLfeBgvNonlinearStageV1>,
    ) -> Result<(), RamLfeBgvNonlinearCompileErrorV1> {
        if let Some(stage) = stage {
            stage.validate()?;
            if stage.shape() != self {
                return Err(RamLfeBgvNonlinearCompileErrorV1::ClassMismatch);
            }
        }
        if I != self.input_banks()
            || O != if stage.is_some() {
                match self {
                    Self::Bounded => STATE_BANKS * 3,
                    Self::Refresh => 3,
                }
            } else {
                1
            }
        {
            return Err(RamLfeBgvNonlinearCompileErrorV1::ClassMismatch);
        }
        Ok(())
    }

    fn check_plan<const I: usize, const O: usize>(
        self,
        stage: Option<RamLfeBgvNonlinearStageV1>,
        plan: &SlotLinearPlan<I, O>,
    ) -> Result<(), RamLfeBgvNonlinearCompileErrorV1> {
        self.check_geometry::<I, O>(stage)?;
        if plan.slots() != SLOTS || plan.plaintext_modulus() != u64::from(MODULUS) {
            return Err(RamLfeBgvNonlinearCompileErrorV1::UnsupportedProfile);
        }
        for row in 0..O * SLOTS {
            let (expected, count) = self.support(stage, row);
            let actual = plan
                .layout()
                .row_segments(row)
                .ok_or(SlotLinearError::LayoutMismatch)?;
            if actual.len() != count
                || actual.iter().zip(&expected[..count]).any(|(a, b)| {
                    (a.start(), a.count(), a.stride()) != (b.start(), b.count(), b.stride())
                })
            {
                return Err(SlotLinearError::LayoutMismatch.into());
            }
        }
        Ok(())
    }

    /// Prepare a tape-independent Square map for the Refresh class.
    ///
    /// This mode has entirely public coefficients. Class, stage,
    /// dimensions, effective F257 encoding and every support segment are
    /// checked before map or row allocation. No private compiler or normal
    /// form is constructed, and this method grants no tape-class admission.
    /// # Errors
    /// Refuses private modes, another class, invalid stage/profile/support or
    /// a shared map allocation failure.
    pub fn prepare_public_layer<const I: usize, const O: usize>(
        self,
        stage: RamLfeBgvNonlinearStageV1,
        plan: &SlotLinearPlan<I, O>,
    ) -> Result<SlotLinearMap<I, O>, RamLfeBgvNonlinearCompileErrorV1> {
        let RamLfeBgvNonlinearStageV1::RefreshGate { gate, mode } = stage else {
            return Err(RamLfeBgvNonlinearCompileErrorV1::ClassMismatch);
        };
        if self != Self::Refresh || mode != RamLfeBgvRefreshMapV1::Square {
            return Err(RamLfeBgvNonlinearCompileErrorV1::ClassMismatch);
        }
        self.check_plan(Some(stage), plan)?;
        Ok(SlotLinearMap::prepare_rows(plan, |row, values| {
            #[cfg(test)]
            tests::observe_row();
            let complete = emit_public_refresh_row(
                usize::from(gate),
                mode,
                row % SLOTS,
                row / SLOTS,
                &mut |column, coefficient| {
                    values[column] = (values[column] + u32::from(coefficient)) % u32::from(MODULUS);
                },
            );
            debug_assert!(complete);
            #[cfg(test)]
            for column in 0..values.len() {
                tests::observe_write(row * values.len() + column);
            }
            Ok(())
        })?)
    }

    /// Exact metadata bytes for a class-fixed layer support graph.
    /// # Errors
    /// Refuses stage/geometry mismatch or checked byte arithmetic overflow.
    pub fn layer_layout_metadata_bytes<const I: usize, const O: usize>(
        self,
        stage: RamLfeBgvNonlinearStageV1,
    ) -> Result<usize, RamLfeBgvNonlinearCompileErrorV1> {
        self.layout_metadata::<I, O>(Some(stage))
    }

    /// Exact metadata bytes for the ordered-output support graph.
    /// # Errors
    /// Refuses geometry mismatch or checked byte arithmetic overflow.
    pub fn projection_layout_metadata_bytes<const I: usize>(
        self,
    ) -> Result<usize, RamLfeBgvNonlinearCompileErrorV1> {
        self.layout_metadata::<I, 1>(None)
    }

    fn layout_metadata<const I: usize, const O: usize>(
        self,
        stage: Option<RamLfeBgvNonlinearStageV1>,
    ) -> Result<usize, RamLfeBgvNonlinearCompileErrorV1> {
        self.check_geometry::<I, O>(stage)?;
        let count: usize = (0..O * SLOTS).map(|row| self.support(stage, row).1).sum();
        Ok(SlotLinearLayout::<I, O>::metadata_bytes_for(SLOTS, count)?)
    }

    /// Build exact fixed support for a public stage, independent of the tape.
    /// # Errors
    /// Refuses invalid stage/geometry, metadata budget or allocation failure.
    pub fn layer_layout<const I: usize, const O: usize>(
        self,
        stage: RamLfeBgvNonlinearStageV1,
        maximum_metadata_bytes: usize,
    ) -> Result<SlotLinearLayout<I, O>, RamLfeBgvNonlinearCompileErrorV1> {
        self.build_layout(Some(stage), maximum_metadata_bytes)
    }

    /// Build 64 fixed ordered-output rows followed by 64 structural zeros.
    /// # Errors
    /// Refuses geometry, metadata budget or allocation failure.
    pub fn projection_layout<const I: usize>(
        self,
        maximum_metadata_bytes: usize,
    ) -> Result<SlotLinearLayout<I, 1>, RamLfeBgvNonlinearCompileErrorV1> {
        self.build_layout(None, maximum_metadata_bytes)
    }

    fn build_layout<const I: usize, const O: usize>(
        self,
        stage: Option<RamLfeBgvNonlinearStageV1>,
        budget: usize,
    ) -> Result<SlotLinearLayout<I, O>, RamLfeBgvNonlinearCompileErrorV1> {
        let required = self.layout_metadata::<I, O>(stage)?;
        if required > budget {
            return Err(SlotLinearError::LayoutBudget { required, budget }.into());
        }
        let count: usize = (0..O * SLOTS).map(|row| self.support(stage, row).1).sum();
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(O * SLOTS + 1)
            .map_err(|_| SlotLinearError::Allocation)?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(count)
            .map_err(|_| SlotLinearError::Allocation)?;
        offsets.push(0);
        for row in 0..O * SLOTS {
            let (support, length) = self.support(stage, row);
            segments.extend_from_slice(&support[..length]);
            offsets.push(segments.len());
        }
        Ok(SlotLinearLayout::new(SLOTS, offsets, segments, budget)?)
    }
}

fn refresh_support(
    stage: Option<RamLfeBgvNonlinearStageV1>,
    row: usize,
) -> ([SlotLinearSegment; 3], usize) {
    let mut support = [SlotLinearSegment::new(0, 0, 1); 3];
    let mut count = 0;
    let mut push = |start, length| {
        if length != 0 {
            support[count] = SlotLinearSegment::new(start, length, 1);
            count += 1;
        }
    };
    if let Some(RamLfeBgvNonlinearStageV1::RefreshGate { gate, mode }) = stage {
        let gate = usize::from(gate);
        let slot = row % SLOTS;
        let operand = row / SLOTS;
        let target = gate % SLOTS;
        if slot < target {
            if operand == 2 {
                push(gate / SLOTS * SLOTS + slot, 1);
            }
        } else if slot == target {
            match (mode, operand) {
                (RamLfeBgvRefreshMapV1::Initial, 0 | 1) => {
                    push(0, gate);
                    push(REFRESH_BASE, 97);
                }
                (RamLfeBgvRefreshMapV1::Square, 0 | 1) => push(gate, 1),
                (RamLfeBgvRefreshMapV1::Final, 0..=2) => {
                    push(0, gate + usize::from(operand == 0));
                    push(REFRESH_BASE, 97);
                }
                _ => {}
            }
        }
    } else if row < RAM_LFE_V1_MAX_OUTPUTS {
        push(0, REFRESH_GATES);
        push(REFRESH_BASE, 97);
    }
    (support, count)
}

/// Emit a complete row when it depends only on public stage geometry.
/// Returns false only for the current Initial/Final target, whose coefficients
/// must be supplied by the private compiler after the public support preflight.
fn emit_public_refresh_row(
    gate: usize,
    mode: RamLfeBgvRefreshMapV1,
    slot: usize,
    operand: usize,
    emit: &mut impl FnMut(usize, u16),
) -> bool {
    let target = gate % SLOTS;
    if slot < target {
        if operand == 2 {
            emit(gate / SLOTS * SLOTS + slot, 1);
        }
        return true;
    }
    if slot != target {
        return true;
    }
    match mode {
        RamLfeBgvRefreshMapV1::Square => {
            if operand <= 1 {
                emit(gate, 1);
            }
        }
        RamLfeBgvRefreshMapV1::Initial | RamLfeBgvRefreshMapV1::Final => return false,
    }
    true
}

enum PrivateForms {
    Bounded(ClearingArray<u16, PRIVATE_CELLS>),
    Refresh(ClearingArray<u16, REFRESH_PRIVATE_CELLS>),
}
impl Deref for PrivateForms {
    type Target = [u16];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Bounded(c) => &c[..],
            Self::Refresh(c) => &c[..],
        }
    }
}
impl DerefMut for PrivateForms {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Bounded(c) => &mut c[..],
            Self::Refresh(c) => &mut c[..],
        }
    }
}

/// One immutable compiler bound to the original admitted nonlinear class.
///
/// Canonical operands, stored aliases and ordered output snapshots are copied
/// once into class-sized clearing storage. Every gate keeps its mixed ordinal,
/// including Refresh, dead and zero gates. Coefficients may be inspected only
/// during owner-side preparation outside observable encrypted requests.
pub struct RamLfeBgvNonlinearCompilerV1 {
    shape: RamLfeBgvNonlinearShapeV1,
    private: PrivateForms,
}

impl fmt::Debug for RamLfeBgvNonlinearCompilerV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED RAM-LFE nonlinear compiler]")
    }
}

impl RamLfeBgvNonlinearCompilerV1 {
    /// Public geometry bound to the original admitted normal-form class.
    #[must_use]
    pub const fn shape(&self) -> RamLfeBgvNonlinearShapeV1 {
        self.shape
    }

    /// Copy a class-admitted normal form into the immutable private compiler.
    ///
    /// Class membership belongs to the original tape: use the canonical
    /// normalizer with Bounded or Refresh. Cancellation may reduce bounded
    /// ready depths but cannot remove the original logical Refresh schedule.
    /// Every boundary, including zero/dead/equal gates, keeps its original index.
    ///
    /// # Errors
    /// Refuses another class and an insufficient private payload budget before
    /// allocation. Also reports reservation failure or an inconsistent form.
    /// The fixed-array conversion may use the infallible allocator to shrink;
    /// this is not complete allocation-failure recovery.
    pub fn from_normal_form(
        normal: &RamLfeNormalFormV1,
        maximum_private_payload_bytes: usize,
    ) -> Result<Self, RamLfeBgvNonlinearCompileErrorV1> {
        let shape = match normal.class() {
            RamLfeClassV1::Bounded => RamLfeBgvNonlinearShapeV1::Bounded,
            RamLfeClassV1::Refresh => RamLfeBgvNonlinearShapeV1::Refresh,
            _ => return Err(RamLfeBgvNonlinearCompileErrorV1::ClassNotNonlinear),
        };
        let required = shape.private_payload_bytes();
        if required > maximum_private_payload_bytes {
            return Err(RamLfeBgvNonlinearCompileErrorV1::PrivateBudget {
                required,
                budget: maximum_private_payload_bytes,
            });
        }
        if normal.gate_count() > shape.gates() || normal.depth() > shape.class().max_rank() {
            return Err(RamLfeBgvNonlinearCompileErrorV1::InvalidNormalForm);
        }
        let private = match shape {
            RamLfeBgvNonlinearShapeV1::Bounded => PrivateForms::Bounded(
                ClearingArray::try_zeroed()
                    .map_err(|_| RamLfeBgvNonlinearCompileErrorV1::Allocation)?,
            ),
            RamLfeBgvNonlinearShapeV1::Refresh => PrivateForms::Refresh(
                ClearingArray::try_zeroed()
                    .map_err(|_| RamLfeBgvNonlinearCompileErrorV1::Allocation)?,
            ),
        };
        let mut compiler = Self { shape, private };
        for gate in 0..shape.gates() {
            let Some(kind) = normal.gate_kind(gate) else {
                continue;
            };
            compiler.private[shape.kinds() + gate] = match kind {
                RamLfeNormalGateKindV1::Mul => 1,
                RamLfeNormalGateKindV1::SelectEqZero => 2,
                RamLfeNormalGateKindV1::Refresh if shape == RamLfeBgvNonlinearShapeV1::Refresh => 3,
                RamLfeNormalGateKindV1::Refresh => {
                    return Err(RamLfeBgvNonlinearCompileErrorV1::InvalidNormalForm);
                }
            };
            for operand in 0..kind.operand_count() {
                let form = normal
                    .gate_operand(gate, operand)
                    .ok_or(RamLfeBgvNonlinearCompileErrorV1::InvalidNormalForm)?;
                if form.len() > RAM_LFE_V1_NORMAL_BASE_COLUMNS + gate {
                    return Err(RamLfeBgvNonlinearCompileErrorV1::InvalidNormalForm);
                }
                let start = (gate * 3 + operand) * shape.columns();
                compiler.private[start..start + form.len()].copy_from_slice(form);
            }
            if shape == RamLfeBgvNonlinearShapeV1::Refresh {
                continue;
            }
            let start = gate * PRIMITIVES_PER_GATE;
            let left = compiler.form_depth(compiler.operand(gate, 0));
            let right = compiler.form_depth(compiler.operand(gate, 1));
            let final_depth = if kind == RamLfeNormalGateKindV1::Mul {
                left.max(right) + 1
            } else {
                // Eight squarings followed by a retained product by one.
                for step in 0_u16..9 {
                    compiler.private[READY + start + usize::from(step)] = left + step + 1;
                }
                (left + 10)
                    .max(right + 1)
                    .max(compiler.form_depth(compiler.operand(gate, 2)) + 1)
            };
            if final_depth > RAM_LFE_V1_BOUNDED_MAX_RANK {
                return Err(RamLfeBgvNonlinearCompileErrorV1::InvalidNormalForm);
            }
            compiler.private[READY + start + 9] = final_depth;
        }
        for row in 0..RAM_LFE_V1_MAX_OUTPUTS {
            if let Some(form) = normal.output_form(row) {
                if form.len() > shape.columns() {
                    return Err(RamLfeBgvNonlinearCompileErrorV1::InvalidNormalForm);
                }
                let start = shape.operands() + row * shape.columns();
                compiler.private[start..start + form.len()].copy_from_slice(form);
            }
        }
        Ok(compiler)
    }

    // Conservative compiler-case union; all bias columns are structural zero.
    // F_g consists of 97 base coordinates and only prior canonical gate finals.
    // A private zero/dead gate does not remove anything from this public union.
    fn row_support<const O: usize>(row: usize) -> ([SlotLinearSegment; 3], usize) {
        let mut support = [SlotLinearSegment::new(0, 0, 1); 3];
        let mut count = 0;
        let mut push = |start, length, stride| {
            if length != 0 {
                support[count] = SlotLinearSegment::new(start, length, stride);
                count += 1;
            }
        };
        if O == 1 {
            if row < RAM_LFE_V1_MAX_OUTPUTS {
                push(9, GATES, 10);
                push(BASE_BANK, RAM_LFE_V1_NORMAL_BASE_COLUMNS, 1);
            }
        } else {
            let operand = row / PHYSICAL;
            let physical = row % PHYSICAL;
            let gate = physical / PRIMITIVES_PER_GATE;
            let step = physical % PRIMITIVES_PER_GATE;
            if gate < GATES {
                match (step, operand) {
                    (0, 0 | 1) | (9, 0..=2) => {
                        push(9, gate, 10);
                        if operand == 0 {
                            push(
                                if step == 0 { physical } else { physical - 1 },
                                if step == 0 { 1 } else { 2 },
                                1,
                            );
                        }
                        push(BASE_BANK, RAM_LFE_V1_NORMAL_BASE_COLUMNS, 1);
                    }
                    (1..=8, 0) => push(physical - 1, 2, 1),
                    (1..=7, 1) => {
                        push(physical - 1, 1, 1);
                        push(ONE, 1, 1);
                    }
                    (8, 1) => push(ONE, 1, 1),
                    _ => {}
                }
            }
        }
        (support, count)
    }

    /// Prepare one class-fixed public stage in the shared clearing map owner.
    ///
    /// The exact support graph, stage, class, dimensions and effective F257
    /// encoding are checked before private coefficients or row allocation.
    /// Refresh callers prepare three maps per gate and reuse Square for the
    /// seven public middle steps; each of those seven squares still executes.
    /// # Errors
    /// Refuses an invalid stage/class/profile/support or shared-owner failure.
    pub fn prepare_layer<const I: usize, const O: usize>(
        &self,
        stage: RamLfeBgvNonlinearStageV1,
        plan: &SlotLinearPlan<I, O>,
    ) -> Result<SlotLinearMap<I, O>, RamLfeBgvNonlinearCompileErrorV1> {
        self.shape.check_plan(Some(stage), plan)?;
        Ok(SlotLinearMap::prepare_rows(plan, |row, values| {
            match stage {
                RamLfeBgvNonlinearStageV1::BoundedLayer(layer) => {
                    self.fill_layer_row(layer, row, values)
                }
                RamLfeBgvNonlinearStageV1::RefreshGate { gate, mode } => {
                    self.fill_refresh_row(usize::from(gate), mode, row, values)
                }
            }
            Ok(())
        })?)
    }

    /// Prepare all ordered output snapshots with fixed zero output padding.
    /// # Errors
    /// Refuses class geometry, effective profile, support or owner allocation.
    pub fn prepare_projection<const I: usize>(
        &self,
        plan: &SlotLinearPlan<I, 1>,
    ) -> Result<SlotLinearMap<I, 1>, RamLfeBgvNonlinearCompileErrorV1> {
        self.shape.check_plan(None, plan)?;
        Ok(SlotLinearMap::prepare_rows(plan, |row, values| {
            self.fill_projection_row(row, values);
            Ok(())
        })?)
    }

    fn operand(&self, gate: usize, operand: usize) -> &[u16] {
        let start = (gate * 3 + operand) * self.shape.columns();
        &self.private[start..start + self.shape.columns()]
    }

    fn form_depth(&self, form: &[u16]) -> u16 {
        form.iter()
            .enumerate()
            .skip(RAM_LFE_V1_NORMAL_BASE_COLUMNS)
            .filter(|(_, coefficient)| **coefficient != 0)
            .map(|(column, _)| self.private[READY + self.column_slot(column)])
            .max()
            .unwrap_or(0)
    }

    fn column_slot(&self, column: usize) -> usize {
        if self.shape == RamLfeBgvNonlinearShapeV1::Refresh {
            return match column {
                0 => REFRESH_ONE,
                1..RAM_LFE_V1_NORMAL_BASE_COLUMNS => REFRESH_BASE + column - 1,
                _ => column - RAM_LFE_V1_NORMAL_BASE_COLUMNS,
            };
        }
        match column {
            0 => ONE,
            1..RAM_LFE_V1_NORMAL_BASE_COLUMNS => BASE_BANK + column - 1,
            _ => (column - RAM_LFE_V1_NORMAL_BASE_COLUMNS) * PRIMITIVES_PER_GATE + 9,
        }
    }

    fn emit_form(&self, form: &[u16], negative: bool, emit: &mut impl FnMut(usize, u16)) {
        for (column, &coefficient) in form.iter().enumerate() {
            let coefficient = if negative && coefficient != 0 {
                MODULUS - coefficient
            } else {
                coefficient
            };
            emit(self.column_slot(column), coefficient);
        }
    }

    /// Shared by public-support preparation and the differential scalar test oracle.
    fn emit_layer_row(
        &self,
        layer: u16,
        physical: usize,
        operand: usize,
        mut emit: impl FnMut(usize, u16),
    ) {
        #[cfg(test)]
        tests::observe_row();
        let ready = self.private[READY + physical];
        if ready == 0 || ready > layer {
            return;
        }
        if ready < layer {
            match operand {
                0 => emit(physical, 1),
                1 => emit(ONE, 1),
                _ => {}
            }
            return;
        }
        let gate = physical / PRIMITIVES_PER_GATE;
        let step = physical % PRIMITIVES_PER_GATE;
        match (self.private[KINDS + gate], step, operand) {
            (1, 9, 0 | 1) => self.emit_form(self.operand(gate, operand), false, &mut emit),
            (2, 0, 0 | 1) => self.emit_form(self.operand(gate, 0), false, &mut emit),
            (2, 1..=7, 0 | 1) | (2, 8, 0) => emit(physical - 1, 1),
            (2, 8, 1) => emit(ONE, 1),
            (2, 9, 0) => {
                emit(ONE, 1);
                emit(physical - 1, MODULUS - 1);
            }
            (2, 9, 1) => {
                self.emit_form(self.operand(gate, 1), false, &mut emit);
                self.emit_form(self.operand(gate, 2), true, &mut emit);
            }
            (2, 9, 2) => self.emit_form(self.operand(gate, 2), false, &mut emit),
            _ => {}
        }
    }

    fn emit_refresh_row(
        &self,
        gate: usize,
        mode: RamLfeBgvRefreshMapV1,
        slot: usize,
        operand: usize,
        mut emit: impl FnMut(usize, u16),
    ) {
        #[cfg(test)]
        tests::observe_row();
        if emit_public_refresh_row(gate, mode, slot, operand, &mut emit) {
            return;
        }
        match (mode, self.private[REFRESH_KINDS + gate], operand) {
            (RamLfeBgvRefreshMapV1::Initial, 2, 0 | 1) | (RamLfeBgvRefreshMapV1::Final, 3, 2) => {
                self.emit_form(self.operand(gate, 0), false, &mut emit)
            }
            (RamLfeBgvRefreshMapV1::Final, 1, 0 | 1) => {
                self.emit_form(self.operand(gate, operand), false, &mut emit)
            }
            (RamLfeBgvRefreshMapV1::Final, 2, 0) => {
                emit(REFRESH_ONE, 1);
                emit(gate, MODULUS - 1);
            }
            (RamLfeBgvRefreshMapV1::Final, 2, 1) => {
                self.emit_form(self.operand(gate, 1), false, &mut emit);
                self.emit_form(self.operand(gate, 2), true, &mut emit);
            }
            (RamLfeBgvRefreshMapV1::Final, 2, 2) => {
                self.emit_form(self.operand(gate, 2), false, &mut emit)
            }
            _ => {}
        }
    }

    fn fill_refresh_row(
        &self,
        gate: usize,
        mode: RamLfeBgvRefreshMapV1,
        row: usize,
        values: &mut [u32],
    ) {
        debug_assert_eq!(values.len(), 4 * SLOTS + 1);
        self.emit_refresh_row(
            gate,
            mode,
            row % SLOTS,
            row / SLOTS,
            |column, coefficient| {
                values[column] = (values[column] + u32::from(coefficient)) % u32::from(MODULUS);
            },
        );
        #[cfg(test)]
        for column in 0..values.len() {
            tests::observe_write(row * values.len() + column);
        }
    }

    fn emit_projection_row(&self, row: usize, mut emit: impl FnMut(usize, u16)) {
        if row < RAM_LFE_V1_MAX_OUTPUTS {
            let start = self.shape.operands() + row * self.shape.columns();
            self.emit_form(
                &self.private[start..start + self.shape.columns()],
                false,
                &mut emit,
            );
        }
    }

    fn fill_layer_row(&self, layer: u16, row: usize, values: &mut [u32]) {
        debug_assert_eq!(values.len(), ROW_CELLS + 1);
        self.emit_layer_row(
            layer,
            row % PHYSICAL,
            row / PHYSICAL,
            |column, coefficient| {
                values[column] = (values[column] + u32::from(coefficient)) % u32::from(MODULUS);
            },
        );
        #[cfg(test)]
        for column in 0..values.len() {
            tests::observe_write(row * values.len() + column);
        }
    }

    fn fill_projection_row(&self, row: usize, values: &mut [u32]) {
        debug_assert_eq!(values.len(), self.shape.input_banks() * SLOTS + 1);
        self.emit_projection_row(row, |column, coefficient| {
            values[column] = u32::from(coefficient);
        });
        #[cfg(test)]
        for column in 0..values.len() {
            tests::observe_write(row * values.len() + column);
        }
    }
}

#[cfg(test)]
#[path = "bgv_nonlinear_compiler_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bgv_nonlinear_refresh_tests.rs"]
mod refresh_tests;
