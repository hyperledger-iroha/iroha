//! Test-only structural lowering of the real tape; no encryption qualification.
//!
//! Logical ranks are not physical RNS levels. Counts exclude unknown algorithm
//! workspaces, noise transfers and sanitization, which remain mandatory blockers.

use super::{HiddenRamFheInstruction as Op, HiddenRamFheProgram};
use std::fmt;
use zeroize::Zeroize;

const TAPE_CAPACITY: usize = 256;
const OUTPUT_CAPACITY: usize = 64;
const INITIAL_VALUES: usize = 34; // Input, 32 state embeddings, shared register zero.
// Select creates at most 20 nodes including every rank-alignment demand; input
// broadcast creates 15. Retired IDs are never reused during one private plan.
const VALUE_CAPACITY: usize = INITIAL_VALUES + TAPE_CAPACITY * 20;
const GALOIS_EXPONENTS: [u16; 7] = [5, 25, 625, 5601, 4033, 3969, 8191];
const BLOCKERS: [&str; 9] = [
    "reviewed encryption parameters and security qualification",
    "genuine RNS levels, exact scale-round and deterministic backend parity",
    "typed noise transfers for the complete operation schedule",
    "authenticated key generation, distribution and ownership",
    "client input well-formedness proof bound to the execution statement",
    "reviewed output sanitization and circuit privacy",
    "key, NTT, basis, decomposition, sanitizer and prover scratch ownership",
    "bounded canonical ciphertext and proof codecs",
    "complete private 256-position relation within the existing budgets",
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

    fn increment(&mut self, counter: Count) {
        self.counts[counter as usize] += 1;
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
        self.values.as_mut_slice().zeroize();
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
        if self.next == self.values.len() {
            return Err(PlanError::Capacity);
        }
        let id = self.next;
        self.values[id] = [1, rank];
        self.next += 1;
        self.live += 1;
        self.plan.peaks[0] = self.plan.peaks[0].max(self.live);
        self.plan.peaks[1] = self.plan.peaks[1].max(self.live * 2);
        Ok(id)
    }

    fn retain(&mut self, id: usize) {
        assert_ne!(self.values[id][0], 0, "live symbolic owner");
        self.values[id][0] += 1;
    }

    fn release(&mut self, id: usize) {
        assert_ne!(self.values[id][0], 0, "live symbolic owner");
        self.values[id][0] -= 1;
        if self.values[id][0] == 0 {
            self.live -= 1;
        }
    }

    fn rank(&self, id: usize) -> usize {
        assert_ne!(self.values[id][0], 0, "live symbolic owner");
        self.values[id][1]
    }

    fn replace_register(&mut self, dst: u16, owned: usize) {
        let old = std::mem::replace(&mut self.registers[usize::from(dst)], owned);
        self.release(old);
    }

    fn unary(&mut self, src: usize, counter: Count) -> Result<usize, PlanError> {
        self.plan.increment(counter);
        self.new_value(self.rank(src))
    }

    fn embed(&mut self) -> Result<usize, PlanError> {
        self.plan.increment(Count::Embeddings);
        self.new_value(0)
    }

    fn binary(&mut self, lhs: usize, rhs: usize, counter: Count) -> Result<usize, PlanError> {
        let rank = self.rank(lhs).max(self.rank(rhs));
        let mut alignment = [None; 2];
        for (slot, id) in alignment.iter_mut().zip([lhs, rhs]) {
            if self.rank(id) < rank {
                // Demand for an immutable aligned copy. Its physical transfer
                // and noise are unresolved, even when logical ranks agree.
                self.plan.increment(Count::RankAlignment);
                *slot = Some(self.new_value(rank)?);
            }
        }
        self.plan.increment(counter);
        let multiply = matches!(counter, Count::Mul);
        let result = self.new_value(rank + usize::from(multiply))?;
        if multiply {
            self.plan.increment(Count::Relinearize);
            self.plan.peaks[1] = self.plan.peaks[1].max(self.live * 2 + 3);
        }
        for id in alignment.into_iter().flatten() {
            self.release(id);
        }
        Ok(result)
    }

    fn broadcast_input(&mut self) -> Result<usize, PlanError> {
        let mut result = self.unary(0, Count::Mask)?;
        for _ in GALOIS_EXPONENTS {
            let rotated = self.unary(result, Count::Galois)?;
            let next = self.binary(result, rotated, Count::Add)?;
            self.release(rotated);
            self.release(result);
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
        self.retain(condition);
        let mut base = condition;
        let one_result = self.embed()?;
        for _ in 0..8 {
            let squared = self.binary(base, base, Count::Mul)?;
            self.release(base);
            base = squared;
        }
        let powered = self.binary(one_result, base, Count::Mul)?;
        self.release(one_result);
        self.release(base);
        let one_indicator = self.embed()?;
        let indicator = self.binary(one_indicator, powered, Count::Sub)?;
        self.release(one_indicator);
        self.release(powered);
        let delta = self.binary(zero, nonzero, Count::Sub)?;
        let selected = self.binary(indicator, delta, Count::Mul)?;
        self.release(indicator);
        self.release(delta);
        let result = self.binary(nonzero, selected, Count::Add)?;
        self.release(selected);
        Ok(result)
    }

    fn instruction(&mut self, op: Op) -> Result<(), PlanError> {
        self.plan.increment(Count::Instructions);
        match op {
            Op::LoadInput(dst, _) => {
                let result = self.broadcast_input()?;
                self.replace_register(dst, result);
            }
            Op::LoadState(dst, lane) => {
                let id = self.memory[usize::from(lane)];
                self.retain(id);
                self.replace_register(dst, id);
            }
            Op::StoreState(lane, src) => {
                let id = self.registers[usize::from(src)];
                self.retain(id);
                let old = std::mem::replace(&mut self.memory[usize::from(lane)], id);
                self.release(old);
            }
            Op::LoadConst(dst, _) => {
                let id = self.embed()?;
                self.replace_register(dst, id);
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
                self.replace_register(dst, id);
            }
            Op::AddPlain(dst, src, _) | Op::SubPlain(dst, src, _) | Op::MulPlain(dst, src, _) => {
                let counter = match op {
                    Op::AddPlain(..) => Count::AddPlain,
                    Op::SubPlain(..) => Count::SubPlain,
                    _ => Count::MulPlain,
                };
                let id = self.unary(self.registers[usize::from(src)], counter)?;
                self.replace_register(dst, id);
            }
            Op::SelectEqZero(dst, condition, zero, nonzero) => {
                let id = self.select(
                    self.registers[usize::from(condition)],
                    self.registers[usize::from(zero)],
                    self.registers[usize::from(nonzero)],
                )?;
                self.replace_register(dst, id);
            }
            Op::Output(src) => {
                let id = self.registers[usize::from(src)];
                let output_index = self.plan.count(Count::Outputs);
                self.plan.output_ranks[output_index] = self.rank(id);
                self.plan.increment(Count::Outputs);
                let masked = self.unary(id, Count::Mask)?;
                self.output = Some(if let Some(old) = self.output {
                    let sum = self.binary(old, masked, Count::Add)?;
                    self.release(old);
                    self.release(masked);
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
    for (pc, op) in program.instructions().enumerate() {
        work.instruction(op)?;
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
        assert!(
            BLOCKERS
                .iter()
                .any(|blocker| blocker.contains("sanitization"))
        );
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
            assert!(CLEARED.with_borrow(|events| events.is_empty()));
        }
        assert!(plan(&program, 1_048_576, 512).is_ok());
        assert!(BLOCKERS.iter().any(|blocker| blocker.contains("noise")));
        assert!(
            BLOCKERS
                .iter()
                .any(|blocker| blocker.contains("input well-formedness"))
        );
        assert!(
            BLOCKERS
                .iter()
                .any(|blocker| blocker.contains("256-position"))
        );
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
}
