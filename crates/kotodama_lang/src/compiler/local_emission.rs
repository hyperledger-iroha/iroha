//! Keep original local ABI sources through one verified consumer.
//!
//! This pass changes only moves and unused private frame writes. It retains
//! every source instruction, typed syscall, authenticated snapshot, allocation,
//! canonical serializer, checked failure, callable, schema and exact frame size.
//! A result stays in its original r10 only through zero-emission literal markers
//! to its sole original consumer in the same block. Clobbers, shared values,
//! spill homes, Phi destinations and unknown consumers keep ordinary lowering.

use super::*;

#[cfg(test)]
std::thread_local! {
    static RETAIN_SCALAR_LOCAL_EMISSION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Retain only the preceding emission sequence inside a test's exact scope.
#[cfg(test)]
pub(super) fn with_scalar_emission<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RETAIN_SCALAR_LOCAL_EMISSION.set(self.0);
        }
    }
    let _restore = Restore(RETAIN_SCALAR_LOCAL_EMISSION.replace(true));
    body()
}
fn retain_scalar() -> bool {
    #[cfg(test)]
    if RETAIN_SCALAR_LOCAL_EMISSION.get() {
        return true;
    }
    false
}
/// One original result's complete closed local lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RetainedResult {
    /// Unique original SSA result; no new value is constructed.
    pub(super) value: ir::Temp,
    /// First zero-emission marker or consumer after its original producer.
    pub(super) first_use_position: usize,
    /// Sole original local consumer, including a complete return or branch.
    pub(super) consumer_position: usize,
}
impl RetainedResult {
    /// Select the original return register only inside this exact lifetime.
    pub(super) fn register(self, value: ir::Temp, position: usize) -> Option<u8> {
        (self.value == value
            && self.first_use_position <= position
            && position <= self.consumer_position)
            .then_some(10)
    }
}
/// Emission-only plan joined to the original physical allocation plan.
pub(super) struct Plan {
    /// Consecutive original entry reads sharing the authenticated table base.
    pub(super) parameter_prefix_words: usize,
    /// Whether any original parameter read needs the private saved base.
    pub(super) stores_argument_base: bool,
    outputs: BTreeMap<usize, RetainedResult>,
}
impl Plan {
    /// Join exact source uses, original allocation homes and split reloads.
    pub(super) fn new(
        function: &ir::Function,
        allocation: &regalloc::AllocationPlan,
    ) -> Result<Self, String> {
        let mut plan = Self {
            parameter_prefix_words: 0,
            stores_argument_base: true,
            outputs: BTreeMap::new(),
        };
        if retain_scalar() {
            return Ok(plan);
        }
        let entry = function
            .blocks
            .iter()
            .find(|block| block.label == function.entry)
            .ok_or("missing local-emission entry block")?;
        let prefix = entry
            .instrs
            .iter()
            .take_while(|instruction| matches!(instruction, Instr::LoadVar { .. }))
            .count();
        let all_parameter_loads = function
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .filter(|instruction| matches!(instruction, Instr::LoadVar { .. }))
            .count();
        let entry_position = function
            .blocks
            .iter()
            .take_while(|block| block.label != function.entry)
            .try_fold(0usize, |position, block| {
                position
                    .checked_add(block.instrs.len())
                    .and_then(|position| position.checked_add(1))
                    .ok_or("entry parameter position overflow")
            })?;
        // A split reload may use r27 to address its original spill home. Keep
        // the saved-base lowering if even one reload precedes an entry read.
        let prefix_end = entry_position
            .checked_add(prefix)
            .ok_or("entry parameter lifetime overflow")?;
        let prefix_ready =
            (entry_position..prefix_end).all(|position| allocation.reloads_at(position).is_empty());
        // r27 remains reserved throughout this consecutive entry prefix. Every
        // original table-word read and possible spill write still executes.
        if prefix_ready && all_parameter_loads == prefix {
            plan.parameter_prefix_words = prefix;
            plan.stores_argument_base = false;
        } else if prefix_ready && prefix >= 2 {
            plan.parameter_prefix_words = prefix;
        }
        if plan.parameter_prefix_words > 0 && allocation.used_registers().contains(&27) {
            return Err("entry parameter base aliases allocated r27".to_owned());
        }
        let mut counts = BTreeMap::<usize, (usize, usize)>::new();
        for block in &function.blocks {
            for instruction in &block.instrs {
                regalloc::visit_instr_defs(instruction, |value| {
                    counts.entry(value.0).or_default().0 += 1
                });
                regalloc::visit_instr_uses(instruction, |value| {
                    counts.entry(value.0).or_default().1 += 1
                });
            }
            regalloc::visit_terminator_uses(&block.terminator, |value| {
                counts.entry(value.0).or_default().1 += 1
            });
        }
        let mut block_position = 0usize;
        for block in &function.blocks {
            for (index, instruction) in block.instrs.iter().enumerate() {
                let Some(value) = syscall_result(instruction) else {
                    continue;
                };
                if counts.get(&value.0) != Some(&(1, 1))
                    || !allocation.regs.contains_key(&value)
                    || allocation.stack.contains_key(&value)
                {
                    continue;
                }
                let mut next = index + 1;
                // DataRef emits no operation; its original canonical literal
                // validation still runs during the complete artifact build.
                while next < block.instrs.len()
                    && matches!(block.instrs[next], Instr::DataRef { .. })
                {
                    next += 1;
                }
                let owns_one_use = if let Some(consumer) = block.instrs.get(next) {
                    if !safe_consumer(consumer) {
                        false
                    } else {
                        let mut uses = 0;
                        regalloc::visit_instr_uses(consumer, |used| {
                            uses += usize::from(used == value)
                        });
                        uses == 1
                    }
                } else {
                    let mut uses = 0;
                    regalloc::visit_terminator_uses(&block.terminator, |used| {
                        uses += usize::from(used == value)
                    });
                    uses == 1
                };
                if !owns_one_use {
                    continue;
                }
                let position = block_position
                    .checked_add(index)
                    .ok_or("local result position overflow")?;
                let consumer_position = block_position
                    .checked_add(next)
                    .ok_or("local consumer position overflow")?;
                let first_use_position = position
                    .checked_add(1)
                    .ok_or("local result lifetime overflow")?;
                // Reloads happen before marker/consumer emission, so inspect
                // every position in the interval, including the terminator.
                if (first_use_position..=consumer_position)
                    .any(|at| reload_clobbers(value, allocation.reloads_at(at)))
                {
                    continue;
                }
                plan.outputs.insert(
                    position,
                    RetainedResult {
                        value,
                        first_use_position,
                        consumer_position,
                    },
                );
            }
            block_position = block_position
                .checked_add(block.instrs.len())
                .and_then(|position| position.checked_add(1))
                .ok_or("local block position overflow")?;
        }
        Ok(plan)
    }
    /// Return only the eligible original result at its exact producer position.
    pub(super) fn output(&self, value: ir::Temp, position: usize) -> Option<RetainedResult> {
        self.outputs
            .get(&position)
            .copied()
            .filter(|output| output.value == value)
    }
}
fn reload_clobbers(value: ir::Temp, reloads: &[regalloc::SplitReload]) -> bool {
    reloads
        .iter()
        .any(|reload| reload.register == 10 || reload.temp == value)
}
fn syscall_result(instruction: &Instr) -> Option<ir::Temp> {
    match instruction {
        Instr::NumericBinary { dest, .. }
        | Instr::NumericCompare { dest, .. }
        | Instr::NumericRound { dest, .. }
        | Instr::NumericNeg { dest, .. }
        | Instr::NumericConvert { dest, .. }
        | Instr::DecimalToInt { dest, .. }
        | Instr::StateValueEncode { dest, .. }
        | Instr::StateGet { dest, .. }
        | Instr::PointerToNorito { dest, .. }
        | Instr::PathMapKeyNorito { dest, .. } => Some(*dest),
        Instr::DirectHelperSyscall {
            dest,
            syscall,
            args,
        } if parallel_state_decode(*syscall, args.len())
            || matches!(
                *syscall,
                syscalls::SYSCALL_INT_ISQRT..=syscalls::SYSCALL_INT_MEAN
            ) =>
        {
            Some(*dest)
        }
        _ => None,
    }
}
fn safe_consumer(instruction: &Instr) -> bool {
    // Comparison emitters exist only in tests. A retained publication emitter
    // may stage sequentially, so it never receives an unmaterialized r10 value.
    #[cfg(test)]
    match instruction {
        Instr::NumericBinary { .. } | Instr::NumericCompare { .. }
            if numeric_operands::retain_publication() =>
        {
            return false;
        }
        Instr::StateSet { .. } | Instr::PathMapKeyNorito { .. }
            if state_operands::retain_publication() =>
        {
            return false;
        }
        Instr::NumericRound { .. } if compact_emission::retain_scalar() => {
            return false;
        }
        Instr::DirectHelperSyscall { syscall, .. }
            if compact_emission::retain_scalar()
                && matches!(
                    *syscall,
                    syscalls::SYSCALL_INT_ISQRT..=syscalls::SYSCALL_INT_MEAN
                ) =>
        {
            return false;
        }
        _ => {}
    }
    matches!(
        instruction,
        Instr::NumericBinary { .. }
            | Instr::NumericCompare { .. }
            | Instr::NumericRound { .. }
            | Instr::NumericNeg { .. }
            | Instr::StateValueEncode { .. }
            | Instr::StateSet { .. }
            | Instr::StateGet { .. }
            | Instr::PointerToNorito { .. }
            | Instr::PathMapKeyNorito { .. }
            | Instr::Unary { .. }
            | Instr::Copy { .. }
            | Instr::Load64Imm { .. }
            | Instr::Assert { .. }
            | Instr::AssertEq { .. }
            | Instr::AbortIf { .. }
    ) || matches!(instruction, Instr::DirectHelperSyscall { syscall, args, .. } if parallel_state_decode(*syscall, args.len()) || matches!(*syscall, syscalls::SYSCALL_INT_ISQRT..=syscalls::SYSCALL_INT_MEAN))
}
/// The original two-input schema/data decoder uses the existing parallel ABI mover.
pub(super) fn parallel_state_decode(syscall: u32, argument_count: usize) -> bool {
    syscall == syscalls::SYSCALL_STATE_VALUE_DECODE && argument_count == 2 && !retain_scalar()
}
/// A self-copy changes no architectural value or public tag.
pub(super) fn emit_move(code: &mut Vec<u8>, destination: u8, source: u8) -> Result<(), String> {
    if destination != source || retain_scalar() {
        push_word(code, encode_addi(destination, source, 0)?);
    }
    Ok(())
}

#[cfg(test)]
mod native_pairs;
#[cfg(test)]
mod tests;
