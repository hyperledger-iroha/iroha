//! Test-only structural lowering of the real tape; no encryption qualification.
//!
//! Logical ranks are not physical RNS levels. Counts exclude unknown algorithm
//! workspaces, noise transfers and sanitization, which remain mandatory blockers.

use super::{HiddenRamFheInstruction as Op, HiddenRamFheProgram};
use crate::fhe_bfv::plaintext_packing_candidate::GALOIS_EXPONENTS;
use std::fmt;
use zeroize::Zeroize;

const TAPE_CAPACITY: usize = 256;
const OUTPUT_CAPACITY: usize = 64;
const INITIAL_VALUES: usize = 34; // Input, 32 state embeddings, shared register zero.
// Select creates at most 20 nodes including every rank-alignment demand; input
// broadcast creates 15. Retired IDs are never reused during one private plan.
const VALUE_CAPACITY: usize = INITIAL_VALUES + TAPE_CAPACITY * 20;
const PROGRAM_KEY_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QualificationBlocker {
    SecurityParameters,
    PhysicalRnsLevelsAndBackendParity,
    NoiseTransfers,
    AuthenticatedKeyOwnership,
    BoundClientInputProof,
    CircuitPrivacySanitizer,
    AlgorithmAndProverScratch,
    CanonicalBoundedCodecs,
    CompleteUniversalRelationAndBudgets,
}

const BLOCKERS: [QualificationBlocker; 9] = [
    QualificationBlocker::SecurityParameters,
    QualificationBlocker::PhysicalRnsLevelsAndBackendParity,
    QualificationBlocker::NoiseTransfers,
    QualificationBlocker::AuthenticatedKeyOwnership,
    QualificationBlocker::BoundClientInputProof,
    QualificationBlocker::CircuitPrivacySanitizer,
    QualificationBlocker::AlgorithmAndProverScratch,
    QualificationBlocker::CanonicalBoundedCodecs,
    QualificationBlocker::CompleteUniversalRelationAndBudgets,
];

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
enum Count {
    Instructions,
    Embeddings,
    Add,
    Sub,
    Mul,
    Relinearize,
    AddPlain,
    SubPlain,
    MulPlain,
    Mask,
    Galois,
    RankAlignment,
    UniversalPositions,
    InactivePositions,
    Outputs,
}
const COUNT_LENGTH: usize = Count::Outputs as usize + 1;

#[derive(Debug, PartialEq, Eq)]
enum PlanError {
    RequestShape,
    Allocation,
    Capacity,
    ArithmeticOverflow,
    Interrupted,
}

/// Private structural metadata only: deliberately no executable/ready state.
struct UnqualifiedPlan {
    counts: [usize; COUNT_LENGTH],
    output_ranks: [usize; OUTPUT_CAPACITY],
    final_register_ranks: [usize; 4],
    final_memory_ranks: [usize; 32],
    required_galois: [u16; 7],
    // Peak unique two-component owners; peak residue-component buffers including
    // the raw three-component product while its two-component result is live.
    peaks: [usize; 2],
    allocated_value_ids: usize,
}

impl Default for UnqualifiedPlan {
    fn default() -> Self {
        Self {
            counts: [0; COUNT_LENGTH],
            output_ranks: [0; OUTPUT_CAPACITY],
            final_register_ranks: [0; 4],
            final_memory_ranks: [0; 32],
            required_galois: [0; 7],
            peaks: [0; 2],
            allocated_value_ids: 0,
        }
    }
}

impl UnqualifiedPlan {
    fn count(&self, counter: Count) -> usize {
        self.counts[counter as usize]
    }

    fn increment(&mut self, counter: Count) -> Result<(), PlanError> {
        let count = &mut self.counts[counter as usize];
        *count = count.checked_add(1).ok_or(PlanError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Symbolic base residue storage only. Neither N nor limb count selects or
    /// validates a cryptographic profile; all BLOCKERS still apply.
    fn base_residue_bytes(&self, ring_degree: usize, limbs: usize) -> Result<usize, PlanError> {
        if ring_degree == 0 || limbs == 0 {
            return Err(PlanError::RequestShape);
        }
        self.peaks[1]
            .checked_mul(ring_degree)
            .and_then(|size| size.checked_mul(limbs))
            .and_then(|size| size.checked_mul(8))
            .ok_or(PlanError::ArithmeticOverflow)
    }

    fn clear(&mut self) {
        self.counts.zeroize();
        self.output_ranks.zeroize();
        self.final_register_ranks.zeroize();
        self.final_memory_ranks.zeroize();
        self.required_galois.zeroize();
        self.peaks.zeroize();
        self.allocated_value_ids.zeroize();
    }
}

impl fmt::Debug for UnqualifiedPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED unqualified RAM structural plan]")
    }
}

impl Drop for UnqualifiedPlan {
    fn drop(&mut self) {
        self.clear();
        observe_clear(
            1,
            self.counts
                .iter()
                .chain(&self.output_ranks)
                .chain(&self.final_register_ranks)
                .chain(&self.final_memory_ranks)
                .chain(&self.peaks)
                .all(|&word| word == 0)
                && self.required_galois == [0; 7]
                && self.allocated_value_ids == 0,
        );
    }
}

struct Work {
    // [references, logical rank]. Full initialized storage clears on every exit.
    values: Vec<[usize; 2]>,
    registers: [usize; 4],
    memory: [usize; 32],
    output: Option<usize>,
    next: usize,
    live: usize,
    plan: UnqualifiedPlan,
}

impl Drop for Work {
    fn drop(&mut self) {
        for row in &mut self.values {
            for word in row {
                word.zeroize();
            }
        }
        self.registers.zeroize();
        self.memory.zeroize();
        if let Some(id) = &mut self.output {
            id.zeroize();
        }
        self.next.zeroize();
        self.live.zeroize();
        observe_clear(
            0,
            self.values.iter().flatten().all(|&word| word == 0)
                && self.registers == [0; 4]
                && self.memory == [0; 32]
                && self.output.is_none_or(|id| id == 0)
                && self.next == 0
                && self.live == 0,
        );
    }
}

impl Work {
    fn new() -> Result<Self, PlanError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(VALUE_CAPACITY)
            .map_err(|_| PlanError::Allocation)?;
        values.resize(VALUE_CAPACITY, [0; 2]);
        let mut work = Self {
            values,
            registers: [33; 4],
            memory: std::array::from_fn(|i| i + 1),
            output: None,
            next: 0,
            live: 0,
            plan: UnqualifiedPlan::default(),
        };
        for _ in 0..INITIAL_VALUES {
            work.new_value(0)?;
        }
        work.values[33][0] = 4;
        work.plan.counts[Count::Embeddings as usize] = 33;
        Ok(work)
    }

    fn new_value(&mut self, rank: usize) -> Result<usize, PlanError> {
        if self.next >= self.values.len() {
            return Err(PlanError::Capacity);
        }
        let id = self.next;
        let next = self
            .next
            .checked_add(1)
            .ok_or(PlanError::ArithmeticOverflow)?;
        let live = self
            .live
            .checked_add(1)
            .ok_or(PlanError::ArithmeticOverflow)?;
        let components = live.checked_mul(2).ok_or(PlanError::ArithmeticOverflow)?;
        self.values[id] = [1, rank];
        self.next = next;
        self.live = live;
        self.plan.peaks[0] = self.plan.peaks[0].max(live);
        self.plan.peaks[1] = self.plan.peaks[1].max(components);
        Ok(id)
    }

    fn retain(&mut self, id: usize) -> Result<(), PlanError> {
        assert_ne!(self.values[id][0], 0, "live symbolic owner");
        self.values[id][0] = self.values[id][0]
            .checked_add(1)
            .ok_or(PlanError::ArithmeticOverflow)?;
        Ok(())
    }

    fn release(&mut self, id: usize) -> Result<(), PlanError> {
        let references = self.values[id][0]
            .checked_sub(1)
            .ok_or(PlanError::ArithmeticOverflow)?;
        let live = if references == 0 {
            self.live
                .checked_sub(1)
                .ok_or(PlanError::ArithmeticOverflow)?
        } else {
            self.live
        };
        self.values[id][0] = references;
        self.live = live;
        Ok(())
    }

    fn rank(&self, id: usize) -> usize {
        assert_ne!(self.values[id][0], 0, "live symbolic owner");
        self.values[id][1]
    }

    fn replace_register(&mut self, dst: u16, owned: usize) -> Result<(), PlanError> {
        let old = std::mem::replace(&mut self.registers[usize::from(dst)], owned);
        self.release(old)
    }

    fn unary(&mut self, src: usize, counter: Count) -> Result<usize, PlanError> {
        self.plan.increment(counter)?;
        self.new_value(self.rank(src))
    }

    fn embed(&mut self) -> Result<usize, PlanError> {
        self.plan.increment(Count::Embeddings)?;
        self.new_value(0)
    }

    fn binary(&mut self, lhs: usize, rhs: usize, counter: Count) -> Result<usize, PlanError> {
        let rank = self.rank(lhs).max(self.rank(rhs));
        let mut alignment = [None; 2];
        for (slot, id) in alignment.iter_mut().zip([lhs, rhs]) {
            if self.rank(id) < rank {
                // Demand for an immutable aligned copy. Its physical transfer
                // and noise are unresolved, even when logical ranks agree.
                self.plan.increment(Count::RankAlignment)?;
                *slot = Some(self.new_value(rank)?);
            }
        }
        self.plan.increment(counter)?;
        let multiply = matches!(counter, Count::Mul);
        let result_rank = rank
            .checked_add(usize::from(multiply))
            .ok_or(PlanError::ArithmeticOverflow)?;
        let result = self.new_value(result_rank)?;
        if multiply {
            self.plan.increment(Count::Relinearize)?;
            let components = self
                .live
                .checked_mul(2)
                .and_then(|live| live.checked_add(3))
                .ok_or(PlanError::ArithmeticOverflow)?;
            self.plan.peaks[1] = self.plan.peaks[1].max(components);
        }
        for id in alignment.into_iter().flatten() {
            self.release(id)?;
        }
        Ok(result)
    }

    fn broadcast_input(&mut self) -> Result<usize, PlanError> {
        let mut result = self.unary(0, Count::Mask)?;
        for _ in GALOIS_EXPONENTS {
            let rotated = self.unary(result, Count::Galois)?;
            let next = self.binary(result, rotated, Count::Add)?;
            self.release(rotated)?;
            self.release(result)?;
            result = next;
        }
        self.plan.required_galois = GALOIS_EXPONENTS;
        Ok(result)
    }

    fn select(
        &mut self,
        condition: usize,
        zero: usize,
        nonzero: usize,
    ) -> Result<usize, PlanError> {
        self.retain(condition)?;
        let mut base = condition;
        let one_result = self.embed()?;
        for _ in 0..8 {
            let squared = self.binary(base, base, Count::Mul)?;
            self.release(base)?;
            base = squared;
        }
        let powered = self.binary(one_result, base, Count::Mul)?;
        self.release(one_result)?;
        self.release(base)?;
        let one_indicator = self.embed()?;
        let indicator = self.binary(one_indicator, powered, Count::Sub)?;
        self.release(one_indicator)?;
        self.release(powered)?;
        let delta = self.binary(zero, nonzero, Count::Sub)?;
        let selected = self.binary(indicator, delta, Count::Mul)?;
        self.release(indicator)?;
        self.release(delta)?;
        let result = self.binary(nonzero, selected, Count::Add)?;
        self.release(selected)?;
        Ok(result)
    }

    fn instruction(&mut self, op: Op) -> Result<(), PlanError> {
        self.plan.increment(Count::Instructions)?;
        match op {
            Op::LoadInput(dst, _) => {
                let result = self.broadcast_input()?;
                self.replace_register(dst, result)?;
            }
            Op::LoadState(dst, lane) => {
                let id = self.memory[usize::from(lane)];
                self.retain(id)?;
                self.replace_register(dst, id)?;
            }
            Op::StoreState(lane, src) => {
                let id = self.registers[usize::from(src)];
                self.retain(id)?;
                let old = std::mem::replace(&mut self.memory[usize::from(lane)], id);
                self.release(old)?;
            }
            Op::LoadConst(dst, _) => {
                let id = self.embed()?;
                self.replace_register(dst, id)?;
            }
            Op::Add(dst, lhs, rhs) | Op::Mul(dst, lhs, rhs) => {
                let counter = if matches!(op, Op::Mul(..)) {
                    Count::Mul
                } else {
                    Count::Add
                };
                let id = self.binary(
                    self.registers[usize::from(lhs)],
                    self.registers[usize::from(rhs)],
                    counter,
                )?;
                self.replace_register(dst, id)?;
            }
            Op::AddPlain(dst, src, _) | Op::SubPlain(dst, src, _) | Op::MulPlain(dst, src, _) => {
                let counter = match op {
                    Op::AddPlain(..) => Count::AddPlain,
                    Op::SubPlain(..) => Count::SubPlain,
                    _ => Count::MulPlain,
                };
                let id = self.unary(self.registers[usize::from(src)], counter)?;
                self.replace_register(dst, id)?;
            }
            Op::SelectEqZero(dst, condition, zero, nonzero) => {
                let id = self.select(
                    self.registers[usize::from(condition)],
                    self.registers[usize::from(zero)],
                    self.registers[usize::from(nonzero)],
                )?;
                self.replace_register(dst, id)?;
            }
            Op::Output(src) => {
                let id = self.registers[usize::from(src)];
                let output_index = self.plan.count(Count::Outputs);
                self.plan.output_ranks[output_index] = self.rank(id);
                self.plan.increment(Count::Outputs)?;
                let masked = self.unary(id, Count::Mask)?;
                self.output = Some(if let Some(old) = self.output {
                    let sum = self.binary(old, masked, Count::Add)?;
                    self.release(old)?;
                    self.release(masked)?;
                    sum
                } else {
                    masked
                });
            }
        }
        Ok(())
    }
}

/// Borrow an already validated tape. Public sizes are checked before private
/// allocation; this function accepts no keys, input plaintext or random coins.
fn plan(
    program: &HiddenRamFheProgram,
    envelope_bytes: usize,
    associated_data_bytes: usize,
) -> Result<UnqualifiedPlan, PlanError> {
    plan_with_hook(program, envelope_bytes, associated_data_bytes, |_| Ok(()))
}

fn plan_with_hook(
    program: &HiddenRamFheProgram,
    envelope_bytes: usize,
    associated_data_bytes: usize,
    mut hook: impl FnMut(usize) -> Result<(), PlanError>,
) -> Result<UnqualifiedPlan, PlanError> {
    if envelope_bytes == 0 || envelope_bytes > 1_048_576 || associated_data_bytes > 512 {
        return Err(PlanError::RequestShape);
    }
    let mut work = Work::new()?;
    // Every universal position is accounted even for a short private tape.
    // Inactive positions preserve every owner and emit nothing. This is neither
    // an implemented proof relation nor a claim of constant-time planning.
    for pc in 0..TAPE_CAPACITY {
        work.plan.increment(Count::UniversalPositions)?;
        if let Some(op) = program.instruction(pc) {
            work.instruction(op)?;
        } else {
            work.plan.increment(Count::InactivePositions)?;
        }
        hook(pc)?;
    }
    for (dst, id) in work
        .plan
        .final_register_ranks
        .iter_mut()
        .zip(work.registers)
    {
        *dst = work.values[id][1];
    }
    for (dst, id) in work.plan.final_memory_ranks.iter_mut().zip(work.memory) {
        *dst = work.values[id][1];
    }
    work.plan.allocated_value_ids = work.next;
    Ok(std::mem::take(&mut work.plan))
}

thread_local! {
    static CLEARED: std::cell::RefCell<Vec<(u8, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn observe_clear(kind: u8, cleared: bool) {
    CLEARED.with_borrow_mut(|events| events.push((kind, cleared)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tape(ops: impl IntoIterator<Item = Op>) -> HiddenRamFheProgram {
        let mut builder = HiddenRamFheProgram::builder().expect("bounded owner");
        for op in ops {
            builder.push(op).expect("valid instruction");
        }
        builder.finish().expect("valid tape")
    }

    fn lower(ops: impl IntoIterator<Item = Op>) -> UnqualifiedPlan {
        plan(&tape(ops), 1, 0).expect("structural plan")
    }

    #[test]
    fn all_eleven_operations_have_explicit_costs_and_unresolved_requirements() {
        let plan = lower([
            Op::LoadInput(0, 63),
            Op::LoadState(1, 31),
            Op::StoreState(0, 1),
            Op::LoadConst(2, 256),
            Op::Add(0, 0, 1),
            Op::AddPlain(0, 0, 2),
            Op::SubPlain(0, 0, 1),
            Op::MulPlain(0, 0, 256),
            Op::Mul(0, 0, 2),
            Op::SelectEqZero(3, 0, 1, 2),
            Op::Output(3),
        ]);
        assert_eq!(plan.count(Count::Instructions), 11);
        assert_eq!(plan.count(Count::Embeddings), 36);
        assert_eq!(plan.count(Count::Mul), 11);
        assert_eq!(plan.count(Count::Relinearize), 11);
        assert_eq!(plan.count(Count::Add), 9);
        assert_eq!(plan.count(Count::Sub), 2);
        for count in [
            Count::AddPlain,
            Count::SubPlain,
            Count::MulPlain,
            Count::Outputs,
        ] {
            assert_eq!(plan.count(count), 1);
        }
        assert_eq!(plan.count(Count::Galois), 7);
        assert_eq!(plan.count(Count::Mask), 2);
        assert_eq!(plan.required_galois, GALOIS_EXPONENTS);
        assert_eq!(plan.output_ranks[0], 11);
        assert_eq!(BLOCKERS.len(), 9);
        assert!(BLOCKERS.contains(&QualificationBlocker::CircuitPrivacySanitizer));
        assert_eq!(
            format!("{plan:?}"),
            "[REDACTED unqualified RAM structural plan]"
        );
    }

    #[test]
    fn zero_reads_aliases_stores_and_output_snapshots_are_preserved() {
        let plan = lower([
            Op::Output(0),
            Op::Mul(0, 0, 0),
            Op::StoreState(31, 0),
            Op::LoadState(1, 31),
            Op::LoadConst(0, 5),
            Op::Output(1),
            Op::Mul(1, 1, 1),
            Op::LoadState(2, 31),
            Op::Output(2),
            Op::Output(1),
        ]);
        assert_eq!(&plan.output_ranks[..4], &[0, 1, 1, 2]);
        assert_eq!(plan.final_register_ranks, [0, 2, 1, 0]);
        assert_eq!(plan.final_memory_ranks[31], 1);
        assert_eq!(plan.count(Count::Mask), 4);
        assert_eq!(plan.count(Count::Add), 3);
        assert!(plan.count(Count::RankAlignment) >= 2);
        assert_eq!(plan.required_galois, [0; 7]);
    }

    #[test]
    fn distinct_structural_extrema_do_not_shrink_the_universal_tape() {
        let selects =
            lower(std::iter::repeat_n(Op::SelectEqZero(3, 0, 1, 2), 255).chain([Op::Output(3)]));
        assert_eq!(selects.count(Count::Instructions), TAPE_CAPACITY);
        assert_eq!(selects.count(Count::Mul), 2550);
        assert_eq!(selects.count(Count::Relinearize), 2550);
        assert_eq!(selects.count(Count::Sub), 510);
        assert_eq!(selects.count(Count::Add), 255);
        assert_eq!(selects.output_ranks[0], 10);
        assert!(selects.allocated_value_ids <= VALUE_CAPACITY);
        let inputs = lower(std::iter::repeat_n(Op::LoadInput(0, 63), 255).chain([Op::Output(0)]));
        assert_eq!(inputs.count(Count::Galois), 1785);
        assert_eq!(inputs.count(Count::Add), 1785);
        assert_eq!(inputs.count(Count::Mask), 256);
        let outputs = lower(std::iter::repeat_n(Op::Output(0), 64));
        assert_eq!(outputs.count(Count::Mask), 64);
        assert_eq!(outputs.count(Count::Add), 63);
        assert_eq!(outputs.count(Count::Mul), 0);
        assert_eq!(outputs.output_ranks, [0; 64]);
    }

    #[test]
    fn select_destination_alias_and_branch_rank_follow_the_real_validator() {
        let condition_alias = lower(
            std::iter::repeat_n(Op::Mul(0, 0, 0), 6)
                .chain([Op::SelectEqZero(0, 0, 1, 2), Op::Output(0)]),
        );
        assert_eq!(condition_alias.output_ranks[0], 16);
        assert_eq!(condition_alias.count(Count::Mul), 16);
        let branch_alias = lower(
            std::iter::repeat_n(Op::Mul(1, 1, 1), 15)
                .chain([Op::SelectEqZero(1, 0, 1, 1), Op::Output(1)]),
        );
        assert_eq!(branch_alias.output_ranks[0], 16);
        assert_eq!(branch_alias.count(Count::Mul), 25);
        let mut rejected = HiddenRamFheProgram::builder().unwrap();
        for op in std::iter::repeat_n(Op::Mul(0, 0, 0), 7)
            .chain([Op::SelectEqZero(0, 0, 1, 2), Op::Output(0)])
        {
            rejected.push(op).unwrap();
        }
        assert!(rejected.finish().is_err());
    }

    #[test]
    fn actual_tape_owner_enforces_instruction_output_rank_and_operand_caps() {
        let max = lower(std::iter::repeat_n(Op::Mul(0, 0, 0), 16).chain([Op::Output(0)]));
        assert_eq!(max.output_ranks[0], 16);
        for ops in [
            vec![Op::Output(0); 65],
            std::iter::repeat_n(Op::Mul(0, 0, 0), 17)
                .chain([Op::Output(0)])
                .collect(),
            vec![Op::LoadInput(0, 64), Op::Output(0)],
            vec![Op::LoadState(0, 32), Op::Output(0)],
            vec![Op::LoadConst(0, 257), Op::Output(0)],
            vec![Op::Output(4)],
        ] {
            let mut builder = HiddenRamFheProgram::builder().unwrap();
            for op in ops {
                builder.push(op).unwrap();
            }
            assert!(builder.finish().is_err());
        }
        let mut builder = HiddenRamFheProgram::builder().unwrap();
        for _ in 0..256 {
            builder.push(Op::LoadConst(0, 1)).unwrap();
        }
        assert!(builder.push(Op::Output(0)).is_err());
    }

    #[test]
    fn peaks_count_immutable_aliases_and_raw_tensor_without_qualifying_noise() {
        let empty_register = lower([Op::Output(0)]);
        assert_eq!(empty_register.peaks, [35, 70]);
        let multiply = lower([Op::Mul(0, 0, 0), Op::Output(0)]);
        assert_eq!(multiply.peaks, [36, 73]);
        assert_eq!(multiply.base_residue_bytes(4096, 3), Ok(73 * 4096 * 3 * 8));
        assert_eq!(
            multiply.base_residue_bytes(usize::MAX, 1),
            Err(PlanError::ArithmeticOverflow)
        );
        assert_eq!(
            multiply.base_residue_bytes(1, usize::MAX),
            Err(PlanError::ArithmeticOverflow)
        );
        assert_eq!(
            multiply.base_residue_bytes(0, 1),
            Err(PlanError::RequestShape)
        );
        let mut work = Work::new().unwrap();
        work.next = VALUE_CAPACITY;
        assert_eq!(work.new_value(0), Err(PlanError::Capacity));
    }

    #[test]
    fn request_caps_precede_private_allocation_and_plan_always_remains_unqualified() {
        let program = tape([Op::Output(0)]);
        for (bytes, ad) in [(0, 0), (1_048_577, 0), (1, 513)] {
            CLEARED.with_borrow_mut(Vec::clear);
            assert_eq!(
                plan(&program, bytes, ad).unwrap_err(),
                PlanError::RequestShape
            );
            assert!(CLEARED.with_borrow(Vec::is_empty));
        }
        assert!(plan(&program, 1_048_576, 512).is_ok());
        assert!(BLOCKERS.contains(&QualificationBlocker::NoiseTransfers));
        assert_eq!(PROGRAM_KEY_BYTES, 32);
        assert_eq!(TAPE_CAPACITY, 256);
        assert!(BLOCKERS.contains(&QualificationBlocker::BoundClientInputProof));
        assert!(BLOCKERS.contains(&QualificationBlocker::CompleteUniversalRelationAndBudgets));
    }

    #[test]
    fn initialized_owned_cells_clear_after_success_error_and_unwind() {
        let program = tape([
            Op::LoadInput(0, 1),
            Op::SelectEqZero(3, 0, 1, 2),
            Op::Output(3),
        ]);
        for mode in 0..3 {
            CLEARED.with_borrow_mut(Vec::clear);
            let result = std::panic::catch_unwind(|| {
                let output = plan_with_hook(&program, 1, 0, |pc| {
                    if pc == 1 {
                        if mode == 1 {
                            return Err(PlanError::Interrupted);
                        }
                        assert_ne!(mode, 2, "test post-assignment unwind");
                    }
                    Ok(())
                });
                if mode == 1 {
                    assert_eq!(output.unwrap_err(), PlanError::Interrupted);
                } else {
                    drop(output.unwrap());
                }
            });
            assert_eq!(result.is_err(), mode == 2);
            CLEARED.with_borrow(|events| {
                assert!(events.iter().any(|&(kind, _)| kind == 0));
                assert!(events.iter().any(|&(kind, _)| kind == 1));
                assert!(events.iter().all(|&(_, cleared)| cleared));
            });
        }
    }

    #[test]
    fn every_universal_position_is_accounted_without_inactive_effects() {
        for active in [1, 64, 255, 256] {
            let program =
                tape(std::iter::repeat_n(Op::LoadConst(0, 256), active - 1).chain([Op::Output(0)]));
            let mut observed = Vec::new();
            let plan = plan_with_hook(&program, 1, 0, |pc| {
                observed.push(pc);
                Ok(())
            })
            .unwrap();
            assert_eq!(observed, (0..TAPE_CAPACITY).collect::<Vec<_>>());
            assert_eq!(plan.count(Count::UniversalPositions), TAPE_CAPACITY);
            assert_eq!(plan.count(Count::InactivePositions), TAPE_CAPACITY - active);
            assert_eq!(plan.count(Count::Instructions), active);
            assert_eq!(plan.count(Count::Outputs), 1);
            assert_eq!(plan.count(Count::Embeddings), 33 + active - 1);
            assert_eq!(plan.count(Count::Mask), 1);
            assert_eq!(plan.final_register_ranks, [0; 4]);
            assert_eq!(plan.final_memory_ranks, [0; 32]);
        }
        let program = tape([Op::Output(0)]);
        CLEARED.with_borrow_mut(Vec::clear);
        assert_eq!(
            plan_with_hook(&program, 1, 0, |pc| {
                if pc == TAPE_CAPACITY - 1 {
                    Err(PlanError::Interrupted)
                } else {
                    Ok(())
                }
            })
            .unwrap_err(),
            PlanError::Interrupted
        );
        CLEARED.with_borrow(|events| {
            assert!(events.iter().any(|&(kind, _)| kind == 0));
            assert!(events.iter().any(|&(kind, _)| kind == 1));
            assert!(events.iter().all(|&(_, cleared)| cleared));
        });
    }

    #[test]
    fn counter_reference_rank_and_tensor_arithmetic_fail_closed() {
        let mut plan = UnqualifiedPlan::default();
        plan.counts[Count::Mul as usize] = usize::MAX;
        assert_eq!(
            plan.increment(Count::Mul),
            Err(PlanError::ArithmeticOverflow)
        );
        assert_eq!(plan.count(Count::Mul), usize::MAX);
        let mut work = Work::new().unwrap();
        work.values[33][0] = usize::MAX;
        assert_eq!(work.retain(33), Err(PlanError::ArithmeticOverflow));
        assert_eq!(work.values[33][0], usize::MAX);
        work.values[0][0] = 0;
        assert_eq!(work.release(0), Err(PlanError::ArithmeticOverflow));
        work.values[0][0] = 1;
        work.live = 0;
        assert_eq!(work.release(0), Err(PlanError::ArithmeticOverflow));
        assert_eq!(work.values[0][0], 1);
        work.live = usize::MAX;
        let next = work.next;
        assert_eq!(work.new_value(0), Err(PlanError::ArithmeticOverflow));
        assert_eq!(work.next, next);
        let mut rank = Work::new().unwrap();
        rank.values[33][1] = usize::MAX;
        assert_eq!(
            rank.binary(33, 33, Count::Mul),
            Err(PlanError::ArithmeticOverflow)
        );
        let mut tensor = Work::new().unwrap();
        tensor.live = usize::MAX / 2 - 1;
        assert_eq!(
            tensor.binary(33, 33, Count::Mul),
            Err(PlanError::ArithmeticOverflow)
        );
    }

    #[test]
    fn independent_raii_lifetimes_match_each_aliased_operation_peak() {
        use std::{cell::Cell, rc::Rc};

        #[derive(Default)]
        struct Heap {
            owners: Cell<usize>,
            components: Cell<usize>,
            peaks: Cell<[usize; 2]>,
        }
        struct Value {
            heap: Rc<Heap>,
            rank: usize,
            components: usize,
        }
        impl Drop for Value {
            fn drop(&mut self) {
                self.heap
                    .components
                    .set(self.heap.components.get() - self.components);
                if self.components == 2 {
                    self.heap.owners.set(self.heap.owners.get() - 1);
                }
            }
        }
        // Independent reference: actual Rc ownership and destructor timing, no
        // value IDs, manual retain/release, planner counters or reused peak code.
        fn allocate(heap: &Rc<Heap>, rank: usize, components: usize) -> Rc<Value> {
            heap.components.set(heap.components.get() + components);
            if components == 2 {
                heap.owners.set(heap.owners.get() + 1);
            }
            let old = heap.peaks.get();
            heap.peaks.set([
                old[0].max(heap.owners.get()),
                old[1].max(heap.components.get()),
            ]);
            Rc::new(Value {
                heap: Rc::clone(heap),
                rank,
                components,
            })
        }
        fn binary(heap: &Rc<Heap>, a: &Rc<Value>, b: &Rc<Value>, multiply: bool) -> Rc<Value> {
            let rank = a.rank.max(b.rank);
            let aligned = (a.rank != b.rank).then(|| allocate(heap, rank, 2));
            let raw = multiply.then(|| allocate(heap, rank, 3));
            let result = allocate(heap, rank + usize::from(multiply), 2);
            drop(raw);
            drop(aligned);
            result
        }
        fn select(heap: &Rc<Heap>, c: &Rc<Value>, z: &Rc<Value>, n: &Rc<Value>) -> Rc<Value> {
            let mut base = Rc::clone(c);
            let one = allocate(heap, 0, 2);
            for _ in 0..8 {
                base = binary(heap, &base, &base, true);
            }
            let powered = binary(heap, &one, &base, true);
            drop(one);
            drop(base);
            let one = allocate(heap, 0, 2);
            let indicator = binary(heap, &one, &powered, false);
            drop(one);
            drop(powered);
            let delta = binary(heap, z, n, false);
            let selected = binary(heap, &indicator, &delta, true);
            drop(indicator);
            drop(delta);
            binary(heap, n, &selected, false)
        }
        let heap = Rc::new(Heap::default());
        let input = allocate(&heap, 0, 2);
        let mut memory: [_; 32] = std::array::from_fn(|_| allocate(&heap, 0, 2));
        let mut registers: [_; 4] = {
            let zero = allocate(&heap, 0, 2);
            std::array::from_fn(|_| Rc::clone(&zero))
        };
        let mut output: Option<Rc<Value>> = None;
        let mut work = Work::new().unwrap();
        let program = tape([
            Op::LoadState(0, 0),
            Op::StoreState(31, 0),
            Op::Mul(0, 0, 0),
            Op::Output(0),
            Op::LoadState(1, 31),
            Op::Add(1, 1, 0),
            Op::Output(1),
            Op::StoreState(0, 1),
            Op::SelectEqZero(0, 2, 0, 1),
            Op::Output(0),
            Op::LoadInput(0, 63),
            Op::SelectEqZero(1, 0, 1, 1),
            Op::Output(1),
        ]);
        for op in program.instructions() {
            match op {
                Op::LoadState(dst, lane) => {
                    registers[usize::from(dst)] = Rc::clone(&memory[usize::from(lane)])
                }
                Op::StoreState(lane, src) => {
                    memory[usize::from(lane)] = Rc::clone(&registers[usize::from(src)])
                }
                Op::Mul(dst, a, b) | Op::Add(dst, a, b) => {
                    registers[usize::from(dst)] = binary(
                        &heap,
                        &registers[usize::from(a)],
                        &registers[usize::from(b)],
                        matches!(op, Op::Mul(..)),
                    );
                }
                Op::Output(src) => {
                    let masked = allocate(&heap, registers[usize::from(src)].rank, 2);
                    output = Some(match output.take() {
                        Some(old) => binary(&heap, &old, &masked, false),
                        None => masked,
                    });
                }
                Op::SelectEqZero(dst, c, z, n) => {
                    registers[usize::from(dst)] = select(
                        &heap,
                        &registers[usize::from(c)],
                        &registers[usize::from(z)],
                        &registers[usize::from(n)],
                    );
                }
                Op::LoadInput(dst, _) => {
                    let mut running = allocate(&heap, input.rank, 2);
                    for _ in 0..7 {
                        let rotated = allocate(&heap, running.rank, 2);
                        running = binary(&heap, &running, &rotated, false);
                    }
                    registers[usize::from(dst)] = running;
                }
                _ => unreachable!("closed independent ownership schedule"),
            }
            work.instruction(op).unwrap();
            assert_eq!(work.live, heap.owners.get(), "live values after {op:?}");
            assert_eq!(work.plan.peaks, heap.peaks.get(), "peak after {op:?}");
            assert_eq!(
                work.registers.map(|id| work.rank(id)),
                registers.each_ref().map(|value| value.rank)
            );
            assert_eq!(
                work.memory.map(|id| work.rank(id)),
                memory.each_ref().map(|value| value.rank)
            );
        }
        drop((input, memory, registers, output));
        assert_eq!(heap.owners.get(), 0);
        assert_eq!(heap.components.get(), 0);
    }
}
