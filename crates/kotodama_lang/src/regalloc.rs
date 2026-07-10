//! Linear-scan register allocator and stack frame layout for Kotodama IR.
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet},
};

use super::ir::{BasicBlock, Function, Instr, Label, Program, Temp, Terminator};

/// Result of register allocation for a function.
#[derive(Debug, PartialEq)]
pub struct Allocation {
    /// Mapping from IR temporaries to physical registers.
    pub regs: HashMap<Temp, usize>,
    /// Mapping from spilled temporaries to stack offsets.
    pub stack: HashMap<Temp, usize>,
    /// Total frame size in bytes (16-byte aligned).
    pub frame_size: usize,
}

/// One register-resident slice of a temporary whose canonical home is a spill slot.
///
/// Split segments are read-only: code generation reloads the spill slot at
/// `start`, uses `register` through `end`, and keeps every definition writing
/// the canonical stack home. This makes control-flow joins and loop-carried
/// multi-definition temporaries safe without inserting edge copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SplitSegment {
    temp: Temp,
    register: usize,
    start: usize,
    end: usize,
    use_count: usize,
}

/// Reload scheduled at the beginning of a split live segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SplitReload {
    /// Spilled temporary whose canonical stack home is reloaded.
    pub(crate) temp: Temp,
    /// Physical register holding this segment.
    pub(crate) register: usize,
}

/// Internal code-generation plan combining stable stack homes with split ranges.
///
/// The public [`Allocation`] shape remains the canonical home-location view.
/// Code generation uses this plan to give frequently reused spilled values a
/// deterministic second chance at a register after peak pressure has passed.
#[derive(Debug, PartialEq)]
pub(crate) struct AllocationPlan {
    home: Allocation,
    segments: HashMap<Temp, Vec<SplitSegment>>,
    reloads: BTreeMap<usize, Vec<SplitReload>>,
}

impl std::ops::Deref for AllocationPlan {
    type Target = Allocation;

    fn deref(&self) -> &Self::Target {
        &self.home
    }
}

impl AllocationPlan {
    /// Return the register containing `temp` at a source-use position.
    pub(crate) fn register_for_use(&self, temp: Temp, position: usize) -> Option<usize> {
        if let Some(segments) = self.segments.get(&temp) {
            let insertion = segments.partition_point(|segment| segment.start <= position);
            if let Some(segment) = insertion
                .checked_sub(1)
                .and_then(|index| segments.get(index))
                && position <= segment.end
            {
                return Some(segment.register);
            }
        }
        self.home.regs.get(&temp).copied()
    }

    /// Return deterministic spill reloads required before `position` executes.
    pub(crate) fn reloads_at(&self, position: usize) -> &[SplitReload] {
        self.reloads.get(&position).map_or(&[], Vec::as_slice)
    }

    /// Return every physical register used by a home or split interval.
    pub(crate) fn used_registers(&self) -> Vec<usize> {
        let mut registers = self.home.regs.values().copied().collect::<BTreeSet<_>>();
        for segments in self.segments.values() {
            registers.extend(segments.iter().map(|segment| segment.register));
        }
        registers.into_iter().collect()
    }

    #[cfg(test)]
    fn split_segments(&self, temp: Temp) -> &[SplitSegment] {
        self.segments.get(&temp).map_or(&[], Vec::as_slice)
    }

    #[cfg(test)]
    pub(crate) fn first_split_register(&self, temp: Temp) -> Option<usize> {
        self.segments
            .get(&temp)
            .and_then(|segments| segments.first())
            .map(|segment| segment.register)
    }

    #[cfg(test)]
    fn saved_reload_count(&self) -> usize {
        self.segments
            .values()
            .flatten()
            .map(|segment| segment.use_count.saturating_sub(1))
            .sum()
    }
}

/// Registers r10-r22 are used for argument passing.
pub const ARG_REGS: [usize; 13] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22];
/// Maximum number of recursively flattened argument words in the V1 call ABI.
pub const MAX_ARGUMENT_VALUES: usize = ARG_REGS.len();
/// r10 also holds the first return value.
pub const RET_REG: usize = 10;
/// ABI limit for multi-value returns carried in r10..r22.
pub const MAX_RETURN_VALUES: usize = ARG_REGS.len();
/// r31 acts as the stack pointer.
pub const SP_REG: usize = 31;
/// r30 may be used as a frame pointer.
pub const FP_REG: usize = 30;

// Pool of allocatable registers (see policy above)
const ALLOC_POOL: &[usize] = &[2, 3, 4, 5, 6, 7, 8, 9, 23, 24];

#[derive(Clone, Copy, Debug)]
struct Interval {
    temp: Temp,
    start: usize,
    end: usize,
}

/// Apply deterministic, semantics-preserving optimizations before register
/// allocation.
///
/// Constant evaluation uses Rust's checked integer operations. A checked
/// operation that would overflow or divide by zero is deliberately retained,
/// even when its result is dead, so the generated contract still traps. Host,
/// memory, and state operations are never speculated or discarded here.
pub(crate) fn optimize_program(program: &mut Program) {
    for function in &mut program.functions {
        loop {
            retain_reachable_blocks(function);
            let constants = infer_integer_constants(function);
            let mut changed = fold_integer_instructions(function, &constants);
            changed |= simplify_control_flow(function, &constants);
            retain_reachable_blocks(function);
            changed |= coalesce_local_copies(function);
            changed |= eliminate_dead_pure_instructions(function);
            if !changed {
                break;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConstantState {
    Unknown,
    Integer(i64),
    Overdefined,
}

fn merge_constant_state(left: ConstantState, right: ConstantState) -> ConstantState {
    match (left, right) {
        (ConstantState::Unknown, value) | (value, ConstantState::Unknown) => value,
        (ConstantState::Integer(left), ConstantState::Integer(right)) if left == right => {
            ConstantState::Integer(left)
        }
        (ConstantState::Integer(_), ConstantState::Integer(_))
        | (ConstantState::Overdefined, _)
        | (_, ConstantState::Overdefined) => ConstantState::Overdefined,
    }
}

fn constant_state(constants: &HashMap<Temp, ConstantState>, temp: Temp) -> ConstantState {
    constants
        .get(&temp)
        .copied()
        .unwrap_or(ConstantState::Unknown)
}

fn integer_constant(constants: &HashMap<Temp, ConstantState>, temp: Temp) -> Option<i64> {
    match constant_state(constants, temp) {
        ConstantState::Integer(value) => Some(value),
        ConstantState::Unknown | ConstantState::Overdefined => None,
    }
}

fn merge_definition(
    constants: &mut HashMap<Temp, ConstantState>,
    dest: Temp,
    state: ConstantState,
) -> bool {
    let previous = constant_state(constants, dest);
    let merged = merge_constant_state(previous, state);
    if merged == previous {
        return false;
    }
    constants.insert(dest, merged);
    true
}

fn checked_binary_constant(op: super::ast::BinaryOp, left: i64, right: i64) -> Option<i64> {
    use super::ast::BinaryOp;

    match op {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Sub => left.checked_sub(right),
        BinaryOp::Mul => left.checked_mul(right),
        BinaryOp::Div => left.checked_div(right),
        BinaryOp::Mod => left.checked_rem(right),
        BinaryOp::And => Some(left & right),
        BinaryOp::Or => Some(left | right),
        BinaryOp::Eq => Some(i64::from(left == right)),
        BinaryOp::Ne => Some(i64::from(left != right)),
        BinaryOp::Lt => Some(i64::from(left < right)),
        BinaryOp::Le => Some(i64::from(left <= right)),
        BinaryOp::Gt => Some(i64::from(left > right)),
        BinaryOp::Ge => Some(i64::from(left >= right)),
    }
}

fn wrapping_binary_constant(op: super::ast::BinaryOp, left: i64, right: i64) -> Option<i64> {
    use super::ast::BinaryOp;

    match op {
        BinaryOp::Add => Some(left.wrapping_add(right)),
        BinaryOp::Sub => Some(left.wrapping_sub(right)),
        BinaryOp::Mul => Some(left.wrapping_mul(right)),
        BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge => None,
    }
}

fn unary_constant(op: super::ast::UnaryOp, operand: i64) -> Option<i64> {
    match op {
        super::ast::UnaryOp::Neg => operand.checked_neg(),
        super::ast::UnaryOp::Not => Some(i64::from(operand == 0)),
    }
}

fn instruction_constant_state(
    instruction: &Instr,
    constants: &HashMap<Temp, ConstantState>,
) -> ConstantState {
    let binary_state = |left: Temp,
                        right: Temp,
                        evaluate: fn(super::ast::BinaryOp, i64, i64) -> Option<i64>,
                        op| {
        match (
            constant_state(constants, left),
            constant_state(constants, right),
        ) {
            (ConstantState::Integer(left), ConstantState::Integer(right)) => {
                evaluate(op, left, right).map_or(ConstantState::Overdefined, ConstantState::Integer)
            }
            (ConstantState::Overdefined, _) | (_, ConstantState::Overdefined) => {
                ConstantState::Overdefined
            }
            (ConstantState::Unknown, _) | (_, ConstantState::Unknown) => ConstantState::Unknown,
        }
    };

    match instruction {
        Instr::Const { value, .. } => ConstantState::Integer(*value),
        Instr::Copy { src, .. } => constant_state(constants, *src),
        Instr::Binary {
            op, left, right, ..
        } => binary_state(*left, *right, checked_binary_constant, *op),
        Instr::WrappingBinary {
            op, left, right, ..
        } => binary_state(*left, *right, wrapping_binary_constant, *op),
        Instr::Unary { op, operand, .. } => match constant_state(constants, *operand) {
            ConstantState::Integer(value) => unary_constant(*op, value)
                .map_or(ConstantState::Overdefined, ConstantState::Integer),
            ConstantState::Unknown => ConstantState::Unknown,
            ConstantState::Overdefined => ConstantState::Overdefined,
        },
        Instr::WrappingNeg { operand, .. } => match constant_state(constants, *operand) {
            ConstantState::Integer(value) => ConstantState::Integer(value.wrapping_neg()),
            ConstantState::Unknown => ConstantState::Unknown,
            ConstantState::Overdefined => ConstantState::Overdefined,
        },
        _ => ConstantState::Overdefined,
    }
}

/// Infer constants across SSA copies and control-flow joins. A temporary is a
/// constant only when every reachable definition converges to the same value.
/// Treating unknown definitions as the lattice bottom lets loop-carried copies
/// settle without making block traversal order observable.
fn infer_integer_constants(function: &Function) -> HashMap<Temp, ConstantState> {
    let mut constants = HashMap::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            for instruction in &block.instrs {
                let primary_dest = dest_temp(instruction);
                let primary_state = instruction_constant_state(instruction, &constants);
                visit_instr_defs(instruction, |dest| {
                    let state = if Some(dest) == primary_dest {
                        primary_state
                    } else {
                        ConstantState::Overdefined
                    };
                    changed |= merge_definition(&mut constants, dest, state);
                });
            }
        }
        if !changed {
            return constants;
        }
    }
}

fn simplify_binary_instruction(
    dest: Temp,
    op: super::ast::BinaryOp,
    left: Temp,
    right: Temp,
    constants: &HashMap<Temp, ConstantState>,
) -> Option<Instr> {
    use super::ast::BinaryOp;

    let left_constant = integer_constant(constants, left);
    let right_constant = integer_constant(constants, right);
    if let (Some(left), Some(right)) = (left_constant, right_constant) {
        return checked_binary_constant(op, left, right).map(|value| Instr::Const { dest, value });
    }

    match op {
        // Do not rewrite integer addition with zero: the existing IR also uses
        // `DataRef + 0` as a typed literal materialization marker.
        BinaryOp::Add => None,
        BinaryOp::Sub if right_constant == Some(0) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::Sub if left == right => Some(Instr::Const { dest, value: 0 }),
        BinaryOp::Mul if left_constant == Some(0) || right_constant == Some(0) => {
            Some(Instr::Const { dest, value: 0 })
        }
        BinaryOp::Mul if left_constant == Some(1) => Some(Instr::Copy { dest, src: right }),
        BinaryOp::Mul if right_constant == Some(1) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::Div if right_constant == Some(1) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::And if left_constant == Some(0) || right_constant == Some(0) => {
            Some(Instr::Const { dest, value: 0 })
        }
        BinaryOp::And if left_constant == Some(-1) => Some(Instr::Copy { dest, src: right }),
        BinaryOp::And if right_constant == Some(-1) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::Or if left_constant == Some(0) => Some(Instr::Copy { dest, src: right }),
        BinaryOp::Or if right_constant == Some(0) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::Or if left_constant == Some(-1) || right_constant == Some(-1) => {
            Some(Instr::Const { dest, value: -1 })
        }
        BinaryOp::Eq | BinaryOp::Le | BinaryOp::Ge if left == right => {
            Some(Instr::Const { dest, value: 1 })
        }
        BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt if left == right => {
            Some(Instr::Const { dest, value: 0 })
        }
        BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge => None,
    }
}

fn simplify_wrapping_binary_instruction(
    dest: Temp,
    op: super::ast::BinaryOp,
    left: Temp,
    right: Temp,
    constants: &HashMap<Temp, ConstantState>,
) -> Option<Instr> {
    use super::ast::BinaryOp;

    let left_constant = integer_constant(constants, left);
    let right_constant = integer_constant(constants, right);
    if let (Some(left), Some(right)) = (left_constant, right_constant) {
        return wrapping_binary_constant(op, left, right).map(|value| Instr::Const { dest, value });
    }

    match op {
        BinaryOp::Add if left_constant == Some(0) => Some(Instr::Copy { dest, src: right }),
        BinaryOp::Add if right_constant == Some(0) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::Sub if right_constant == Some(0) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::Sub if left == right => Some(Instr::Const { dest, value: 0 }),
        BinaryOp::Mul if left_constant == Some(0) || right_constant == Some(0) => {
            Some(Instr::Const { dest, value: 0 })
        }
        BinaryOp::Mul if left_constant == Some(1) => Some(Instr::Copy { dest, src: right }),
        BinaryOp::Mul if right_constant == Some(1) => Some(Instr::Copy { dest, src: left }),
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => None,
        BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge => None,
    }
}

fn fold_integer_instructions(
    function: &mut Function,
    constants: &HashMap<Temp, ConstantState>,
) -> bool {
    let mut changed = false;
    for block in &mut function.blocks {
        for instruction in &mut block.instrs {
            let replacement = match instruction {
                Instr::Copy { dest, src } => integer_constant(constants, *src)
                    .map(|value| Instr::Const { dest: *dest, value }),
                Instr::Binary {
                    dest,
                    op,
                    left,
                    right,
                } => simplify_binary_instruction(*dest, *op, *left, *right, constants),
                Instr::WrappingBinary {
                    dest,
                    op,
                    left,
                    right,
                } => simplify_wrapping_binary_instruction(*dest, *op, *left, *right, constants),
                Instr::Unary { dest, op, operand } => integer_constant(constants, *operand)
                    .and_then(|operand| unary_constant(*op, operand))
                    .map(|value| Instr::Const { dest: *dest, value }),
                Instr::WrappingNeg { dest, operand } => {
                    integer_constant(constants, *operand).map(|operand| Instr::Const {
                        dest: *dest,
                        value: operand.wrapping_neg(),
                    })
                }
                _ => None,
            };
            if let Some(replacement) = replacement {
                *instruction = replacement;
                changed = true;
            }
        }
    }
    changed
}

fn resolve_trampoline(mut label: Label, trampolines: &HashMap<Label, Label>) -> Label {
    let original = label;
    let mut visited = HashSet::new();
    while let Some(next) = trampolines.get(&label).copied() {
        if !visited.insert(label) {
            return original;
        }
        label = next;
    }
    label
}

fn simplify_control_flow(
    function: &mut Function,
    constants: &HashMap<Temp, ConstantState>,
) -> bool {
    let mut changed = false;
    for block in &mut function.blocks {
        if let Terminator::Branch {
            cond,
            then_bb,
            else_bb,
        } = block.terminator
        {
            let destination = if then_bb == else_bb {
                Some(then_bb)
            } else {
                integer_constant(constants, cond)
                    .map(|value| if value == 0 { else_bb } else { then_bb })
            };
            if let Some(destination) = destination {
                block.terminator = Terminator::Jump(destination);
                changed = true;
            }
        }
    }

    let trampolines: HashMap<Label, Label> = function
        .blocks
        .iter()
        .filter_map(|block| {
            if block.instrs.is_empty()
                && let Terminator::Jump(target) = block.terminator
                && target != block.label
            {
                return Some((block.label, target));
            }
            None
        })
        .collect();
    if trampolines.is_empty() {
        return changed;
    }

    let entry = resolve_trampoline(function.entry, &trampolines);
    changed |= entry != function.entry;
    function.entry = entry;
    for block in &mut function.blocks {
        match &mut block.terminator {
            Terminator::Jump(target) => {
                let resolved = resolve_trampoline(*target, &trampolines);
                changed |= resolved != *target;
                *target = resolved;
            }
            Terminator::Branch {
                then_bb, else_bb, ..
            } => {
                let resolved_then = resolve_trampoline(*then_bb, &trampolines);
                let resolved_else = resolve_trampoline(*else_bb, &trampolines);
                changed |= resolved_then != *then_bb || resolved_else != *else_bb;
                *then_bb = resolved_then;
                *else_bb = resolved_else;
                if resolved_then == resolved_else {
                    block.terminator = Terminator::Jump(resolved_then);
                    changed = true;
                }
            }
            Terminator::Return(_) | Terminator::Return2(_, _) | Terminator::ReturnN(_) => {}
        }
    }
    changed
}

fn retarget_simple_definition(instruction: &mut Instr, from: Temp, to: Temp) -> bool {
    let dest = match instruction {
        Instr::Const { dest, .. }
        | Instr::Copy { dest, .. }
        | Instr::StringConst { dest, .. }
        | Instr::DataRef { dest, .. }
        | Instr::LoadVar { dest, .. }
        | Instr::Binary { dest, .. }
        | Instr::WrappingBinary { dest, .. }
        | Instr::Unary { dest, .. }
        | Instr::WrappingNeg { dest, .. }
        | Instr::TuplePack { dest, .. }
        | Instr::TupleGet { dest, .. }
        | Instr::PointerFromString { dest, .. } => dest,
        _ => return false,
    };
    if *dest != from {
        return false;
    }
    *dest = to;
    true
}

fn is_simple_definition(instruction: &Instr, temp: Temp) -> bool {
    matches!(
        instruction,
        Instr::Const { dest, .. }
            | Instr::Copy { dest, .. }
            | Instr::StringConst { dest, .. }
            | Instr::DataRef { dest, .. }
            | Instr::LoadVar { dest, .. }
            | Instr::Binary { dest, .. }
            | Instr::WrappingBinary { dest, .. }
            | Instr::Unary { dest, .. }
            | Instr::WrappingNeg { dest, .. }
            | Instr::TuplePack { dest, .. }
            | Instr::TupleGet { dest, .. }
            | Instr::PointerFromString { dest, .. }
            if *dest == temp
    )
}

/// Coalesce a copy into its unique, same-block producer. Restricting the pass
/// to a single block and a source with exactly one use makes dominance and
/// lifetime preservation explicit; multi-definition join/loop copies remain
/// untouched.
fn coalesce_local_copies(function: &mut Function) -> bool {
    let mut any_changed = false;
    loop {
        let mut definition_counts = HashMap::<Temp, usize>::new();
        let mut use_counts = HashMap::<Temp, usize>::new();
        for block in &function.blocks {
            for instruction in &block.instrs {
                visit_instr_defs(instruction, |temp| {
                    *definition_counts.entry(temp).or_default() += 1;
                });
                visit_instr_uses(instruction, |temp| {
                    *use_counts.entry(temp).or_default() += 1;
                });
            }
            visit_terminator_uses(&block.terminator, |temp| {
                *use_counts.entry(temp).or_default() += 1;
            });
        }

        let mut candidate = None;
        'blocks: for (block_index, block) in function.blocks.iter().enumerate() {
            for (copy_index, instruction) in block.instrs.iter().enumerate() {
                let Instr::Copy { dest, src } = instruction else {
                    continue;
                };
                if dest == src
                    || definition_counts.get(dest).copied() != Some(1)
                    || definition_counts.get(src).copied() != Some(1)
                    || use_counts.get(src).copied() != Some(1)
                {
                    continue;
                }
                if let Some(producer_index) = block.instrs[..copy_index]
                    .iter()
                    .rposition(|producer| is_simple_definition(producer, *src))
                {
                    candidate = Some((block_index, producer_index, copy_index, *src, *dest));
                    break 'blocks;
                }
            }
        }

        let Some((block_index, producer_index, copy_index, src, dest)) = candidate else {
            return any_changed;
        };
        let block = &mut function.blocks[block_index];
        let retargeted = retarget_simple_definition(&mut block.instrs[producer_index], src, dest);
        debug_assert!(retargeted, "candidate was checked as a simple definition");
        if !retargeted {
            return any_changed;
        }
        block.instrs.remove(copy_index);
        any_changed = true;
    }
}

fn retain_reachable_blocks(function: &mut Function) {
    let label_to_idx: HashMap<Label, usize> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label, index))
        .collect();
    let Some(&entry_index) = label_to_idx.get(&function.entry) else {
        // Lowering owns this invariant. Leave malformed IR untouched so the
        // regular compiler validation reports it instead of hiding it here.
        return;
    };

    let mut reachable = HashSet::new();
    let mut pending = vec![entry_index];
    while let Some(index) = pending.pop() {
        if !reachable.insert(index) {
            continue;
        }
        let mut successors = block_successors(&function.blocks[index], &label_to_idx);
        // Push in reverse so the source-order successor is visited first. The
        // membership result is order-independent, but this keeps debugging
        // deterministic as the pass grows.
        successors.reverse();
        pending.extend(successors);
    }

    let mut index = 0usize;
    function.blocks.retain(|_| {
        let keep = reachable.contains(&index);
        index += 1;
        keep
    });
}

fn eliminate_dead_pure_instructions(function: &mut Function) -> bool {
    let label_to_idx: HashMap<Label, usize> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label, index))
        .collect();
    let uses_defs = function
        .blocks
        .iter()
        .map(block_uses_defs)
        .collect::<Vec<_>>();
    let block_uses = uses_defs
        .iter()
        .map(|(uses, _)| uses.clone())
        .collect::<Vec<_>>();
    let block_defs = uses_defs
        .iter()
        .map(|(_, defs)| defs.clone())
        .collect::<Vec<_>>();
    let block_succs = function
        .blocks
        .iter()
        .map(|block| block_successors(block, &label_to_idx))
        .collect::<Vec<_>>();
    let (_, live_out) = compute_liveness(&block_uses, &block_defs, &block_succs);

    let mut changed = false;
    for (block_index, block) in function.blocks.iter_mut().enumerate() {
        let mut live = live_out[block_index].clone();
        visit_terminator_uses(&block.terminator, |temp| {
            live.insert(temp);
        });

        let mut retained = Vec::with_capacity(block.instrs.len());
        for instruction in block.instrs.drain(..).rev() {
            let mut definitions = Vec::new();
            visit_instr_defs(&instruction, |temp| definitions.push(temp));
            let is_dead = is_dead_code_eliminable(&instruction)
                && !definitions.is_empty()
                && definitions.iter().all(|temp| !live.contains(temp));
            if is_dead {
                changed = true;
                continue;
            }

            for definition in definitions {
                live.remove(&definition);
            }
            visit_instr_uses(&instruction, |temp| {
                live.insert(temp);
            });
            retained.push(instruction);
        }
        retained.reverse();
        block.instrs = retained;
    }
    changed
}

fn is_dead_code_eliminable(instruction: &Instr) -> bool {
    match instruction {
        Instr::Const { .. }
        | Instr::StringConst { .. }
        | Instr::DataRef { .. }
        | Instr::LoadVar { .. }
        | Instr::Copy { .. }
        | Instr::TuplePack { .. }
        | Instr::TupleGet { .. }
        | Instr::PointerFromString { .. }
        | Instr::WrappingBinary { .. }
        | Instr::WrappingNeg { .. } => true,
        Instr::Binary { op, .. } => matches!(
            op,
            super::ast::BinaryOp::And
                | super::ast::BinaryOp::Or
                | super::ast::BinaryOp::Eq
                | super::ast::BinaryOp::Ne
                | super::ast::BinaryOp::Lt
                | super::ast::BinaryOp::Le
                | super::ast::BinaryOp::Gt
                | super::ast::BinaryOp::Ge
        ),
        Instr::Unary { op, .. } => matches!(op, super::ast::UnaryOp::Not),
        _ => false,
    }
}

/// Whether this function can allocate exclusively from caller-saved argument
/// registers without crossing an instruction that uses the host-call ABI.
pub(crate) fn uses_caller_saved_pool(function: &Function) -> bool {
    function.blocks.iter().all(|block| {
        block
            .instrs
            .iter()
            .all(instruction_preserves_argument_registers)
    })
}

fn instruction_preserves_argument_registers(instruction: &Instr) -> bool {
    matches!(
        instruction,
        Instr::Const { .. }
            | Instr::StringConst { .. }
            | Instr::DataRef { .. }
            | Instr::LoadVar { .. }
            | Instr::Copy { .. }
            | Instr::TuplePack { .. }
            | Instr::TupleGet { .. }
            | Instr::Binary { .. }
            | Instr::WrappingBinary { .. }
            | Instr::Unary { .. }
            | Instr::WrappingNeg { .. }
            | Instr::Min { .. }
            | Instr::Max { .. }
            | Instr::Abs { .. }
            | Instr::DivCeil { .. }
            | Instr::Gcd { .. }
            | Instr::Mean { .. }
            | Instr::Isqrt { .. }
            | Instr::Poseidon2 { .. }
            | Instr::Pubkgen { .. }
            | Instr::Valcom { .. }
            | Instr::PointerFromString { .. }
            | Instr::Load64Imm { .. }
            | Instr::MapLoadPair { .. }
            | Instr::MapGet { .. }
            | Instr::MapSet { .. }
    )
}

/// Whether a direct function call can overwrite the return-address register.
pub(crate) fn has_internal_calls(function: &Function) -> bool {
    function.blocks.iter().any(|block| {
        block
            .instrs
            .iter()
            .any(|instruction| matches!(instruction, Instr::Call { .. } | Instr::CallMulti { .. }))
    })
}

fn precolored_argument_temps(function: &Function, enabled: bool) -> HashMap<Temp, usize> {
    if !enabled {
        return HashMap::new();
    }
    let Some(entry) = function
        .blocks
        .iter()
        .find(|block| block.label == function.entry)
    else {
        return HashMap::new();
    };

    entry
        .instrs
        .iter()
        .take_while(|instruction| matches!(instruction, Instr::LoadVar { .. }))
        .filter_map(|instruction| {
            let Instr::LoadVar { dest, name } = instruction else {
                return None;
            };
            let index = function.params.iter().position(|param| param == name)?;
            ARG_REGS
                .get(index)
                .copied()
                .map(|register| (*dest, register))
        })
        .collect()
}

/// Allocate registers for a function using a single-pass linear scan.
pub fn allocate(func: &Function) -> Allocation {
    let intervals = collect_live_intervals(func);
    allocate_intervals(func, &intervals)
}

/// Allocate canonical homes, then fill post-pressure register holes with
/// read-only slices of frequently reused spills.
pub(crate) fn allocate_with_splitting(func: &Function) -> AllocationPlan {
    let intervals = collect_live_intervals(func);
    let home = allocate_intervals(func, &intervals);
    let split_segments = build_split_segments(func, &intervals, &home);
    let mut segments: HashMap<Temp, Vec<SplitSegment>> = HashMap::new();
    let mut reloads: BTreeMap<usize, Vec<SplitReload>> = BTreeMap::new();
    for segment in split_segments {
        segments.entry(segment.temp).or_default().push(segment);
        reloads.entry(segment.start).or_default().push(SplitReload {
            temp: segment.temp,
            register: segment.register,
        });
    }
    for temp_segments in segments.values_mut() {
        temp_segments.sort_unstable_by_key(|segment| (segment.start, segment.end));
    }
    for position_reloads in reloads.values_mut() {
        position_reloads.sort_unstable_by_key(|reload| (reload.register, reload.temp.0));
    }

    if crate::dev_env::debug_regalloc_enabled() {
        let mut ordered = segments.values().flatten().copied().collect::<Vec<_>>();
        ordered.sort_unstable_by_key(|segment| (segment.start, segment.temp.0, segment.register));
        for segment in ordered {
            eprintln!(
                "  split {:?} start {} end {} uses {} => reg {}",
                segment.temp, segment.start, segment.end, segment.use_count, segment.register
            );
        }
    }

    AllocationPlan {
        home,
        segments,
        reloads,
    }
}

fn collect_live_intervals(func: &Function) -> Vec<Interval> {
    let mut intervals: HashMap<Temp, Interval> = HashMap::new();
    let mut tuple_defs: HashMap<Temp, Vec<Temp>> = HashMap::new();
    let mut position: usize = 0;
    let block_count = func.blocks.len();
    let mut label_to_idx: HashMap<Label, usize> = HashMap::new();
    for (idx, block) in func.blocks.iter().enumerate() {
        label_to_idx.insert(block.label, idx);
    }
    let mut block_uses: Vec<HashSet<Temp>> = Vec::with_capacity(block_count);
    let mut block_defs: Vec<HashSet<Temp>> = Vec::with_capacity(block_count);
    let mut block_succs: Vec<Vec<usize>> = Vec::with_capacity(block_count);
    for block in &func.blocks {
        let (uses, defs) = block_uses_defs(block);
        block_uses.push(uses);
        block_defs.push(defs);
        block_succs.push(block_successors(block, &label_to_idx));
    }
    let (live_in, _live_out) = compute_liveness(&block_uses, &block_defs, &block_succs);
    let mut block_end_pos: Vec<usize> = Vec::with_capacity(block_count);

    for block in &func.blocks {
        for instr in &block.instrs {
            visit_instr_uses(instr, |temp| add_use(&mut intervals, temp, position));
            visit_instr_defs(instr, |dest| add_def(&mut intervals, dest, position));
            if let Instr::TuplePack { dest, items } = instr {
                tuple_defs.insert(*dest, items.clone());
            }
            position = position.saturating_add(1);
        }
        visit_terminator_uses(&block.terminator, |temp| {
            add_use(&mut intervals, temp, position)
        });
        block_end_pos.push(position);
        position = position.saturating_add(1);
    }

    for (block_idx, succs) in block_succs.iter().enumerate() {
        for &succ in succs {
            if succ <= block_idx {
                let end_pos = block_end_pos[block_idx];
                for temp in &live_in[succ] {
                    intervals
                        .entry(*temp)
                        .and_modify(|iv| {
                            if iv.end < end_pos {
                                iv.end = end_pos;
                            }
                        })
                        .or_insert(Interval {
                            temp: *temp,
                            start: end_pos,
                            end: end_pos,
                        });
                }
            }
        }
    }

    extend_tuple_intervals(&mut intervals, &tuple_defs);

    let mut interval_list: Vec<Interval> = intervals.values().copied().collect();
    interval_list.sort_by_key(|iv| (iv.start, iv.temp.0));
    interval_list
}

fn allocate_intervals(func: &Function, interval_list: &[Interval]) -> Allocation {
    let mut allocation = Allocation {
        regs: HashMap::new(),
        stack: HashMap::new(),
        frame_size: 0,
    };
    let use_caller_saved_pool = uses_caller_saved_pool(func);
    let precolored = precolored_argument_temps(func, use_caller_saved_pool);
    let mut active: Vec<(usize, Temp, usize)> = Vec::new();
    let mut free_regs: Vec<usize> = if use_caller_saved_pool {
        ARG_REGS.to_vec()
    } else {
        ALLOC_POOL.to_vec()
    };
    let mut spilled = HashSet::new();

    for interval in interval_list.iter().copied() {
        expire_old_intervals(interval.start, &mut active, &mut free_regs);

        if let Some(&register) = precolored.get(&interval.temp) {
            if let Some(index) = free_regs.iter().position(|free| *free == register) {
                free_regs.swap_remove(index);
            } else if let Some(index) = active
                .iter()
                .position(|(_, _, active_register)| *active_register == register)
            {
                let (_, displaced, _) = active.remove(index);
                allocation.regs.remove(&displaced);
                spilled.insert(displaced);
            }
            allocation.regs.insert(interval.temp, register);
            active.push((interval.end, interval.temp, register));
            active.sort_by_key(|(end, _, _)| *end);
            continue;
        }

        if let Some(reg) = free_regs.pop() {
            allocation.regs.insert(interval.temp, reg);
            active.push((interval.end, interval.temp, reg));
            active.sort_by_key(|(end, _, _)| *end);
            continue;
        }

        if let Some((idx, _)) = active
            .iter()
            .enumerate()
            .max_by_key(|(_, (end, _, _))| *end)
        {
            let (spill_end, spill_temp, spill_reg) = active[idx];
            if spill_end > interval.end {
                spilled.insert(spill_temp);
                allocation.regs.remove(&spill_temp);
                active.remove(idx);
                allocation.regs.insert(interval.temp, spill_reg);
                active.push((interval.end, interval.temp, spill_reg));
                active.sort_by_key(|(end, _, _)| *end);
                continue;
            }
        }

        spilled.insert(interval.temp);
    }

    let (stack, mut next_slot) = assign_spill_slots(interval_list, &spilled);
    allocation.stack = stack;
    if !next_slot.is_multiple_of(16) {
        next_slot += 16 - (next_slot % 16);
    }
    allocation.frame_size = next_slot;
    if crate::dev_env::debug_regalloc_enabled() {
        eprintln!(
            "[regalloc] function {} frame {}",
            func.name, allocation.frame_size
        );
        for interval in interval_list {
            let reg = allocation.regs.get(&interval.temp).copied();
            let stack = allocation.stack.get(&interval.temp).copied();
            eprintln!(
                "  temp {:?} start {} end {} => reg {:?} stack {:?}",
                interval.temp, interval.start, interval.end, reg, stack
            );
        }
    }
    allocation
}

const MAX_SPLIT_USE_GAP: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SplitCandidate {
    temp: Temp,
    start: usize,
    end: usize,
    use_count: usize,
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start <= right_end && right_start <= left_end
}

fn split_candidate_uses<F: FnMut(Temp)>(instruction: &Instr, mut visit: F) {
    match instruction {
        Instr::Binary { left, right, .. }
        | Instr::WrappingBinary { left, right, .. }
        | Instr::NumericBinary { left, right, .. }
        | Instr::NumericCompare { left, right, .. } => {
            visit(*left);
            visit(*right);
        }
        Instr::Unary { operand, .. } | Instr::WrappingNeg { operand, .. } => visit(*operand),
        Instr::NumericFromInt { value, .. }
        | Instr::NumericToInt { value, .. }
        | Instr::NumericNeg { value, .. } => visit(*value),
        Instr::Min { a, b, .. }
        | Instr::Max { a, b, .. }
        | Instr::Gcd { a, b, .. }
        | Instr::Mean { a, b, .. }
        | Instr::Poseidon2 { a, b, .. } => {
            visit(*a);
            visit(*b);
        }
        Instr::DivCeil { num, denom, .. } => {
            visit(*num);
            visit(*denom);
        }
        Instr::Abs { src, .. }
        | Instr::Isqrt { src, .. }
        | Instr::Pubkgen { src, .. }
        | Instr::Load64Imm { base: src, .. } => visit(*src),
        Instr::Poseidon6 { args, .. } => {
            for temp in args {
                visit(*temp);
            }
        }
        Instr::Valcom { value, blind, .. } => {
            visit(*value);
            visit(*blind);
        }
        Instr::DirectHelperSyscall { args, .. }
        | Instr::Call { args, .. }
        | Instr::CallMulti { args, .. } => {
            for temp in args {
                visit(*temp);
            }
        }
        Instr::MapGet { map, key, .. } => {
            visit(*map);
            visit(*key);
        }
        Instr::MapLoadPair { map, .. } => visit(*map),
        Instr::MapSet { map, key, value } => {
            visit(*map);
            visit(*key);
            visit(*value);
        }
        _ => {}
    }
}

fn split_candidate_terminator_uses<F: FnMut(Temp)>(terminator: &Terminator, mut visit: F) {
    match terminator {
        Terminator::Return(Some(temp)) => visit(*temp),
        Terminator::Return(None) | Terminator::Jump(_) => {}
        Terminator::Return2(left, right) => {
            visit(*left);
            visit(*right);
        }
        Terminator::ReturnN(values) => {
            for temp in values {
                visit(*temp);
            }
        }
        Terminator::Branch { cond, .. } => visit(*cond),
    }
}

fn fused_relational_operands(block: &BasicBlock) -> Option<(Temp, Temp)> {
    let Terminator::Branch { cond, .. } = &block.terminator else {
        return None;
    };
    let Some(Instr::Binary {
        dest,
        op,
        left,
        right,
    }) = block.instrs.last()
    else {
        return None;
    };
    if *dest == *cond
        && matches!(
            op,
            super::ast::BinaryOp::Lt
                | super::ast::BinaryOp::Le
                | super::ast::BinaryOp::Gt
                | super::ast::BinaryOp::Ge
        )
    {
        Some((*left, *right))
    } else {
        None
    }
}

fn push_split_clusters(candidates: &mut Vec<SplitCandidate>, temp: Temp, positions: &[usize]) {
    let mut cluster_start = 0usize;
    while cluster_start < positions.len() {
        let mut cluster_end = cluster_start + 1;
        while cluster_end < positions.len()
            && positions[cluster_end].saturating_sub(positions[cluster_end - 1])
                <= MAX_SPLIT_USE_GAP
        {
            cluster_end += 1;
        }
        let cluster = &positions[cluster_start..cluster_end];
        if cluster.len() >= 2 {
            candidates.push(SplitCandidate {
                temp,
                start: cluster[0],
                end: cluster[cluster.len() - 1],
                use_count: cluster.len(),
            });
        }
        cluster_start = cluster_end;
    }
}

fn collect_split_candidates(func: &Function, home: &Allocation) -> Vec<SplitCandidate> {
    let mut candidates = Vec::new();
    let mut position = 0usize;
    // Literal-like values may be materialized directly by code generation, so
    // their nominal IR use count is not proof of repeated spill traffic. Leave
    // them to the rematerialization paths instead of scheduling speculative
    // stack reads.
    let instructions = func
        .blocks
        .iter()
        .flat_map(|block| &block.instrs)
        .collect::<Vec<_>>();
    let mut rematerializable = instructions
        .iter()
        .filter_map(|instruction| match *instruction {
            Instr::Const { dest, .. }
            | Instr::StringConst { dest, .. }
            | Instr::DataRef { dest, .. }
            | Instr::TuplePack { dest, .. } => Some(*dest),
            _ => None,
        })
        .collect::<HashSet<_>>();
    loop {
        let mut changed = false;
        for instruction in &instructions {
            match *instruction {
                Instr::Copy { dest, src } | Instr::PointerFromString { dest, src, .. }
                    if rematerializable.contains(src) =>
                {
                    changed |= rematerializable.insert(*dest);
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }
    for block in &func.blocks {
        // A generation boundary after every definition prevents a read-only
        // segment from observing a stale register value after its stack home
        // has been updated. Generations reset at block boundaries so every
        // control-flow path reloads independently.
        let mut generation = HashMap::<Temp, usize>::new();
        let mut uses = HashMap::<(Temp, usize), Vec<usize>>::new();
        let fused = fused_relational_operands(block);
        for (instruction_index, instruction) in block.instrs.iter().enumerate() {
            let is_fused_compare = fused.is_some() && instruction_index + 1 == block.instrs.len();
            if !is_fused_compare {
                split_candidate_uses(instruction, |temp| {
                    if home.stack.contains_key(&temp) && !rematerializable.contains(&temp) {
                        let epoch = generation.get(&temp).copied().unwrap_or(0);
                        uses.entry((temp, epoch)).or_default().push(position);
                    }
                });
            }
            visit_instr_defs(instruction, |temp| {
                let epoch = generation.entry(temp).or_default();
                *epoch = epoch.saturating_add(1);
            });
            position = position.saturating_add(1);
        }

        if let Some((left, right)) = fused {
            for temp in [left, right] {
                if home.stack.contains_key(&temp) && !rematerializable.contains(&temp) {
                    let epoch = generation.get(&temp).copied().unwrap_or(0);
                    uses.entry((temp, epoch)).or_default().push(position);
                }
            }
        } else {
            split_candidate_terminator_uses(&block.terminator, |temp| {
                if home.stack.contains_key(&temp) && !rematerializable.contains(&temp) {
                    let epoch = generation.get(&temp).copied().unwrap_or(0);
                    uses.entry((temp, epoch)).or_default().push(position);
                }
            });
        }
        position = position.saturating_add(1);

        let mut block_uses = uses.into_iter().collect::<Vec<_>>();
        block_uses.sort_unstable_by_key(|((temp, epoch), _)| (temp.0, *epoch));
        for ((temp, _), positions) in block_uses {
            push_split_clusters(&mut candidates, temp, &positions);
        }
    }
    candidates
}

fn build_split_segments(
    func: &Function,
    intervals: &[Interval],
    home: &Allocation,
) -> Vec<SplitSegment> {
    if home.stack.is_empty() {
        return Vec::new();
    }

    let caller_saved_pool = uses_caller_saved_pool(func);
    let home_registers = home.regs.values().copied().collect::<BTreeSet<_>>();
    let register_pool = if caller_saved_pool {
        ARG_REGS.to_vec()
    } else {
        // Reusing a callee-saved register that already has a home interval
        // avoids adding prologue/epilogue traffic merely to save reloads.
        ALLOC_POOL
            .iter()
            .copied()
            .filter(|register| home_registers.contains(register))
            .collect()
    };
    if register_pool.is_empty() {
        return Vec::new();
    }

    let mut occupied: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for interval in intervals {
        if let Some(register) = home.regs.get(&interval.temp).copied() {
            occupied
                .entry(register)
                .or_default()
                .push((interval.start, interval.end));
        }
    }

    let mut candidates = collect_split_candidates(func, home);
    candidates.sort_unstable_by(|left, right| {
        right
            .use_count
            .cmp(&left.use_count)
            .then_with(|| {
                left.end
                    .saturating_sub(left.start)
                    .cmp(&right.end.saturating_sub(right.start))
            })
            .then_with(|| left.start.cmp(&right.start))
            .then_with(|| left.temp.0.cmp(&right.temp.0))
    });

    let mut segments = Vec::new();
    for candidate in candidates {
        let register = register_pool.iter().rev().copied().find(|register| {
            occupied.get(register).is_none_or(|ranges| {
                ranges.iter().all(|(start, end)| {
                    !ranges_overlap(candidate.start, candidate.end, *start, *end)
                })
            })
        });
        let Some(register) = register else {
            continue;
        };
        occupied
            .entry(register)
            .or_default()
            .push((candidate.start, candidate.end));
        segments.push(SplitSegment {
            temp: candidate.temp,
            register,
            start: candidate.start,
            end: candidate.end,
            use_count: candidate.use_count,
        });
    }
    segments
}

fn expire_stack_intervals(
    current_start: usize,
    active: &mut Vec<(usize, usize)>,
    free_slots: &mut BinaryHeap<Reverse<usize>>,
) {
    // `active` is kept in descending end-position order, so expired slots are
    // removed from the end without shifting the remaining entries.
    while active.last().is_some_and(|(end, _)| *end < current_start) {
        let (_, slot) = active.pop().expect("active stack entry exists");
        free_slots.push(Reverse(slot));
    }
}

fn assign_spill_slots(
    intervals: &[Interval],
    spilled: &HashSet<Temp>,
) -> (HashMap<Temp, usize>, usize) {
    let mut slots = HashMap::with_capacity(spilled.len());
    let mut active: Vec<(usize, usize)> = Vec::new();
    let mut free_slots: BinaryHeap<Reverse<usize>> = BinaryHeap::new();
    let mut next_slot = 0usize;

    for interval in intervals
        .iter()
        .filter(|interval| spilled.contains(&interval.temp))
    {
        expire_stack_intervals(interval.start, &mut active, &mut free_slots);
        let slot = free_slots.pop().map_or_else(
            || {
                let slot = next_slot;
                next_slot = next_slot.saturating_add(8);
                slot
            },
            |Reverse(slot)| slot,
        );
        slots.insert(interval.temp, slot);
        active.push((interval.end, slot));
        active.sort_unstable_by(|left, right| {
            right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1))
        });
    }

    (slots, next_slot)
}

fn extend_tuple_intervals(
    intervals: &mut HashMap<Temp, Interval>,
    tuple_defs: &HashMap<Temp, Vec<Temp>>,
) {
    fn extend_tuple_items(
        tuple: Temp,
        tuple_end: usize,
        intervals: &mut HashMap<Temp, Interval>,
        tuple_defs: &HashMap<Temp, Vec<Temp>>,
        visiting: &mut HashSet<Temp>,
    ) {
        if !visiting.insert(tuple) {
            return;
        }
        if let Some(items) = tuple_defs.get(&tuple) {
            for item in items {
                intervals
                    .entry(*item)
                    .and_modify(|iv| {
                        if iv.end < tuple_end {
                            iv.end = tuple_end;
                        }
                    })
                    .or_insert(Interval {
                        temp: *item,
                        start: tuple_end,
                        end: tuple_end,
                    });
                extend_tuple_items(*item, tuple_end, intervals, tuple_defs, visiting);
            }
        }
        visiting.remove(&tuple);
    }

    let mut visiting: HashSet<Temp> = HashSet::new();
    let tuples: Vec<(Temp, usize)> = tuple_defs
        .keys()
        .filter_map(|tuple| intervals.get(tuple).map(|iv| (*tuple, iv.end)))
        .collect();
    for (tuple, tuple_end) in tuples {
        extend_tuple_items(tuple, tuple_end, intervals, tuple_defs, &mut visiting);
    }
}

fn add_def(intervals: &mut HashMap<Temp, Interval>, temp: Temp, pos: usize) {
    intervals
        .entry(temp)
        .and_modify(|iv| {
            iv.start = iv.start.min(pos);
            iv.end = iv.end.max(pos);
        })
        .or_insert(Interval {
            temp,
            start: pos,
            end: pos,
        });
}

fn add_use(intervals: &mut HashMap<Temp, Interval>, temp: Temp, pos: usize) {
    intervals
        .entry(temp)
        .and_modify(|iv| iv.end = iv.end.max(pos))
        .or_insert(Interval {
            temp,
            start: pos,
            end: pos,
        });
}

fn expire_old_intervals(
    current_start: usize,
    active: &mut Vec<(usize, Temp, usize)>,
    free_regs: &mut Vec<usize>,
) {
    let mut idx = 0;
    while idx < active.len() {
        if active[idx].0 < current_start {
            free_regs.push(active[idx].2);
            active.remove(idx);
        } else {
            idx += 1;
        }
    }
}

fn block_uses_defs(block: &BasicBlock) -> (HashSet<Temp>, HashSet<Temp>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();
    for instr in &block.instrs {
        visit_instr_uses(instr, |temp| {
            if !defs.contains(&temp) {
                uses.insert(temp);
            }
        });
        visit_instr_defs(instr, |temp| {
            defs.insert(temp);
        });
    }
    visit_terminator_uses(&block.terminator, |temp| {
        if !defs.contains(&temp) {
            uses.insert(temp);
        }
    });
    (uses, defs)
}

fn block_successors(block: &BasicBlock, label_to_idx: &HashMap<Label, usize>) -> Vec<usize> {
    match block.terminator {
        Terminator::Jump(label) => label_to_idx.get(&label).copied().into_iter().collect(),
        Terminator::Branch {
            then_bb, else_bb, ..
        } => {
            let mut out = Vec::with_capacity(2);
            if let Some(idx) = label_to_idx.get(&then_bb).copied() {
                out.push(idx);
            }
            if let Some(idx) = label_to_idx.get(&else_bb).copied() {
                out.push(idx);
            }
            out
        }
        Terminator::Return(_) | Terminator::Return2(_, _) | Terminator::ReturnN(_) => Vec::new(),
    }
}

fn compute_liveness(
    block_uses: &[HashSet<Temp>],
    block_defs: &[HashSet<Temp>],
    block_succs: &[Vec<usize>],
) -> (Vec<HashSet<Temp>>, Vec<HashSet<Temp>>) {
    let block_count = block_uses.len();
    let mut live_in: Vec<HashSet<Temp>> = vec![HashSet::new(); block_count];
    let mut live_out: Vec<HashSet<Temp>> = vec![HashSet::new(); block_count];
    let mut changed = true;
    while changed {
        changed = false;
        for idx in (0..block_count).rev() {
            let mut out = HashSet::new();
            for &succ in &block_succs[idx] {
                out.extend(live_in[succ].iter().copied());
            }
            let mut in_set = block_uses[idx].clone();
            for temp in out.iter() {
                if !block_defs[idx].contains(temp) {
                    in_set.insert(*temp);
                }
            }
            if out != live_out[idx] || in_set != live_in[idx] {
                live_out[idx] = out;
                live_in[idx] = in_set;
                changed = true;
            }
        }
    }
    (live_in, live_out)
}

fn visit_instr_uses<F: FnMut(Temp)>(instr: &Instr, mut f: F) {
    use Instr::*;
    match instr {
        Const { .. }
        | StringConst { .. }
        | LoadVar { .. }
        | MapNew { .. }
        | JsonObject { .. }
        | CreateNftsForAllUsers
        | SubscriptionBill
        | SubscriptionRecordUsage
        | DataRef { .. }
        | GetAuthority { .. }
        | SysvarAuthority { .. }
        | CurrentTimeMs { .. }
        | BlockHeight { .. }
        | BlockTimeMs { .. }
        | ChainId { .. }
        | ContractAddress { .. }
        | Entrypoint { .. }
        | GetTriggerEvent { .. }
        | ProveExecution { .. }
        | TransferBatchBegin
        | TransferBatchEnd
        | CommitOutput => {}
        TransferBatchApply { payload } => f(*payload),
        Binary { left, right, .. } | WrappingBinary { left, right, .. } => {
            f(*left);
            f(*right);
        }
        Unary { operand, .. } | WrappingNeg { operand, .. } => f(*operand),
        NumericFromInt { value, .. } | NumericToInt { value, .. } | NumericNeg { value, .. } => {
            f(*value)
        }
        NumericBinary { left, right, .. } | NumericCompare { left, right, .. } => {
            f(*left);
            f(*right);
        }
        DirectHelperSyscall { args, .. } => {
            for arg in args {
                f(*arg);
            }
        }
        Min { a, b, .. } | Max { a, b, .. } | Gcd { a, b, .. } | Mean { a, b, .. } => {
            f(*a);
            f(*b);
        }
        DivCeil { num, denom, .. } => {
            f(*num);
            f(*denom);
        }
        CallContract {
            contract,
            entrypoint,
            payload,
            ..
        } => {
            f(*contract);
            f(*entrypoint);
            f(*payload);
        }
        InvokeEntrypointAs {
            actor,
            entrypoint,
            payload,
            ..
        }
        | InvokeEntrypointAsMulti {
            actor,
            entrypoint,
            payload,
            ..
        }
        | ExpectRejectAs {
            actor,
            entrypoint,
            payload,
        } => {
            f(*actor);
            f(*entrypoint);
            f(*payload);
        }
        ActorAccount { actor, .. } | ActorPublicKey { actor, .. } => f(*actor),
        ActorSign { actor, message, .. } => {
            f(*actor);
            f(*message);
        }
        ResolveAccountAlias { alias, .. } => f(*alias),
        Abs { src, .. } => f(*src),
        Isqrt { src, .. } => f(*src),
        Poseidon2 { a, b, .. } => {
            f(*a);
            f(*b);
        }
        Poseidon6 { args, .. } => {
            for temp in args {
                f(*temp);
            }
        }
        Pubkgen { src, .. } => f(*src),
        Valcom { value, blind, .. } => {
            f(*value);
            f(*blind);
        }
        RegisterAsset {
            asset,
            symbol,
            quantity,
            mintable,
        } => {
            f(*asset);
            f(*symbol);
            f(*quantity);
            f(*mintable);
        }
        CreateNewAsset {
            asset,
            symbol,
            quantity,
            account,
            mintable,
        } => {
            f(*asset);
            f(*symbol);
            f(*quantity);
            f(*account);
            f(*mintable);
        }
        TransferAsset {
            from,
            to,
            asset,
            amount,
            dataspace,
        } => {
            f(*from);
            f(*to);
            f(*asset);
            f(*amount);
            f(*dataspace);
        }
        TransferBatchAsset {
            from,
            to,
            asset,
            amount,
        } => {
            f(*from);
            f(*to);
            f(*asset);
            f(*amount);
        }
        EscrowOpenOffer {
            escrow,
            asset,
            amount,
            evidence_hashes,
        } => {
            f(*escrow);
            f(*asset);
            f(*amount);
            if let Some(evidence_hashes) = evidence_hashes {
                f(*evidence_hashes);
            }
        }
        EscrowResolveDispute {
            escrow,
            buyer_amount,
            seller_amount,
            evidence_hashes,
        } => {
            f(*escrow);
            f(*buyer_amount);
            f(*seller_amount);
            if let Some(evidence_hashes) = evidence_hashes {
                f(*evidence_hashes);
            }
        }
        EscrowAccept { escrow }
        | EscrowMarkPaymentSent { escrow }
        | EscrowRelease { escrow }
        | EscrowCancel { escrow } => f(*escrow),
        AnonymousEscrowOpenOffer { request }
        | AnonymousEscrowRelease { request }
        | AnonymousEscrowCancel { request }
        | AnonymousEscrowResolveDispute { request } => f(*request),
        AnonymousEscrowAccept { escrow } | AnonymousEscrowMarkPaymentSent { escrow } => f(*escrow),
        EscrowOpenDispute {
            escrow,
            evidence_hashes,
        } => {
            f(*escrow);
            if let Some(evidence_hashes) = evidence_hashes {
                f(*evidence_hashes);
            }
        }
        AnonymousEscrowOpenDispute {
            escrow,
            evidence_hashes,
        } => {
            f(*escrow);
            if let Some(evidence_hashes) = evidence_hashes {
                f(*evidence_hashes);
            }
        }
        MintAsset {
            account,
            asset,
            amount,
        }
        | BurnAsset {
            account,
            asset,
            amount,
        } => {
            f(*account);
            f(*asset);
            f(*amount);
        }
        AssertEq { left, right } => {
            f(*left);
            f(*right);
        }
        Assert { cond } => f(*cond),
        AbortIf { cond, code } => {
            f(*cond);
            f(*code);
        }
        Info { msg } => f(*msg),
        DebugPrint { value } => f(*value),
        DebugLog { payload } => f(*payload),
        PointerFromString { src, .. } => f(*src),
        MapGet { map, key, .. } => {
            f(*map);
            f(*key);
        }
        MapLoadPair { map, .. } => f(*map),
        MapSet { map, key, value } => {
            f(*map);
            f(*key);
            f(*value);
        }
        Load64Imm { base, .. } => f(*base),
        TuplePack { items, .. } => {
            for temp in items {
                f(*temp);
            }
        }
        TupleGet { tuple, .. } => f(*tuple),
        Copy { src, .. } => f(*src),
        SetExecutionDepth { value } => f(*value),
        SetVl { value } => f(*value),
        Call { args, .. } | CallMulti { args, .. } => {
            for arg in args {
                f(*arg);
            }
        }
        SetAccountDetail {
            account,
            key,
            value,
        } => {
            f(*account);
            f(*key);
            f(*value);
        }
        CreateNft { nft, owner } => {
            f(*nft);
            f(*owner);
        }
        SetNftData { nft, key, json } => {
            f(*nft);
            f(*key);
            f(*json);
        }
        BurnNft { nft } => f(*nft),
        TransferNft { from, nft, to } => {
            f(*from);
            f(*nft);
            f(*to);
        }
        RegisterDomain { domain } | UnregisterDomain { domain } => f(*domain),
        TransferDomain { domain, to } => {
            f(*domain);
            f(*to);
        }
        RegisterAccount { account } | UnregisterAccount { account } => f(*account),
        AddSignatory { account, signatory } | RemoveSignatory { account, signatory } => {
            f(*account);
            f(*signatory);
        }
        SetAccountQuorum { account, quorum } => {
            f(*account);
            f(*quorum);
        }
        GrantPermission { account, token } | RevokePermission { account, token } => {
            f(*account);
            f(*token);
        }
        GrantRole { account, name } | RevokeRole { account, name } => {
            f(*account);
            f(*name);
        }
        UnregisterAsset { asset } => f(*asset),
        RegisterPeer { json } | UnregisterPeer { json } | CreateTrigger { json } => f(*json),
        CreateRole { name, json } => {
            f(*name);
            f(*json);
        }
        RemoveTrigger { name } | DeleteRole { name } => f(*name),
        SetTriggerEnabled { name, enabled } => {
            f(*name);
            f(*enabled);
        }
        Instr::Sm3Hash { message, .. }
        | Instr::Sha256Hash { message, .. }
        | Instr::Sha3Hash { message, .. }
        | Instr::Blake2b256Hash { message, .. }
        | Instr::Keccak256Hash { message, .. }
        | Instr::IrohaHash { message, .. } => f(*message),
        Instr::Sm2Verify {
            message,
            signature,
            public_key,
            distid,
            ..
        } => {
            f(*message);
            f(*signature);
            f(*public_key);
            if let Some(d) = distid {
                f(*d);
            }
        }
        Instr::VerifySignature {
            message,
            signature,
            public_key,
            scheme,
            ..
        } => {
            f(*message);
            f(*signature);
            f(*public_key);
            f(*scheme);
        }
        Instr::Sm4GcmSeal {
            key,
            nonce,
            aad,
            plaintext,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*plaintext);
        }
        Instr::Sm4GcmOpen {
            key,
            nonce,
            aad,
            ciphertext_and_tag,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*ciphertext_and_tag);
        }
        Instr::Sm4CcmSeal {
            key,
            nonce,
            aad,
            plaintext,
            tag_len,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*plaintext);
            if let Some(t) = tag_len {
                f(*t);
            }
        }
        Instr::Sm4CcmOpen {
            key,
            nonce,
            aad,
            ciphertext_and_tag,
            tag_len,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*ciphertext_and_tag);
            if let Some(t) = tag_len {
                f(*t);
            }
        }
        Instr::ZkVerify { payload, .. }
        | Instr::VerifyProof { payload, .. }
        | Instr::VendorExecuteInstruction { payload, .. }
        | Instr::VendorExecuteQuery { payload, .. }
        | Instr::QueryExecuteNorito { payload, .. }
        | Instr::QueryGet { key: payload, .. }
        | Instr::SmartContractLifecycle { payload, .. }
        | Instr::ZkRootsGet { payload, .. }
        | Instr::ZkVoteGetTally { payload, .. }
        | Instr::VrfEpochSeed { payload, .. }
        | Instr::SoracloudHostCall {
            request: payload, ..
        } => f(*payload),
        Instr::GetAccountBalance { account, asset, .. } => {
            f(*account);
            f(*asset);
        }
        Instr::Alloc { bytes, .. } => f(*bytes),
        Instr::GrowHeap { bytes, .. } => f(*bytes),
        Instr::GetMerklePath {
            address,
            output,
            root_output,
            ..
        } => {
            f(*address);
            f(*output);
            if let Some(root_output) = root_output {
                f(*root_output);
            }
        }
        Instr::GetMerkleCompact {
            address,
            output,
            max_depth,
            root_output,
            ..
        } => {
            f(*address);
            f(*output);
            if let Some(max_depth) = max_depth {
                f(*max_depth);
            }
            if let Some(root_output) = root_output {
                f(*root_output);
            }
        }
        Instr::GetRegisterMerkleCompact {
            register_index,
            output,
            max_depth,
            root_output,
            ..
        } => {
            f(*register_index);
            f(*output);
            if let Some(max_depth) = max_depth {
                f(*max_depth);
            }
            if let Some(root_output) = root_output {
                f(*root_output);
            }
        }
        Instr::GetPrivateInput { index, .. } => f(*index),
        Instr::GetPublicInput { key, .. } => f(*key),
        Instr::UseNullifier { nullifier } => f(*nullifier),
        StateGet { path, .. } => f(*path),
        StateSet { path, value } => {
            f(*path);
            f(*value);
        }
        StateDel { path } => f(*path),
        StateKeys {
            prefix,
            offset,
            limit,
            ..
        } => {
            f(*prefix);
            f(*offset);
            f(*limit);
        }
        StateMapKeyAt {
            page, base, index, ..
        } => {
            f(*page);
            f(*base);
            f(*index);
        }
        StateValueEncode { schema, words, .. } => {
            f(*schema);
            for word in words {
                f(*word);
            }
        }
        StateHas { path, .. } | StateLen { path, .. } => f(*path),
        StateCount { prefix, .. } => f(*prefix),
        DecodeInt { blob, .. } | JsonDecode { blob, .. } | NameDecode { blob, .. } => f(*blob),
        TlvLen { value, .. } => f(*value),
        JsonSetInt {
            json, key, value, ..
        }
        | JsonSetAccountId {
            json, key, value, ..
        } => {
            f(*json);
            f(*key);
            f(*value);
        }
        JsonGetInt { json, key, .. }
        | JsonGetNumeric { json, key, .. }
        | JsonGetJson { json, key, .. }
        | JsonGetName { json, key, .. }
        | JsonGetAccountId { json, key, .. }
        | JsonGetAssetDefinitionId { json, key, .. }
        | JsonGetNftId { json, key, .. }
        | JsonGetBlobHex { json, key, .. } => {
            f(*json);
            f(*key);
        }
        SchemaDecode { schema, blob, .. } => {
            f(*schema);
            f(*blob);
        }
        EncodeInt { value, .. } | PointerToNorito { value, .. } => f(*value),
        PointerFromNorito { blob, .. } => f(*blob),
        PathMapKey { base, key, .. } => {
            f(*base);
            f(*key);
        }
        PathMapKeyNorito { base, key_blob, .. } => {
            f(*base);
            f(*key_blob);
        }
        JsonEncode { json, .. } => f(*json),
        SchemaEncode { schema, json, .. } => {
            f(*schema);
            f(*json);
        }
        SchemaInfo { schema, .. } => f(*schema),
        BuildSubmitBallotInline {
            election_id,
            ciphertext,
            nullifier,
            backend,
            proof,
            vk,
            ..
        } => {
            f(*election_id);
            f(*ciphertext);
            f(*nullifier);
            f(*backend);
            f(*proof);
            f(*vk);
        }
        BuildUnshieldInline {
            asset,
            to,
            amount,
            inputs,
            outputs,
            backend,
            proof,
            vk,
            ..
        } => {
            f(*asset);
            f(*to);
            f(*amount);
            f(*inputs);
            if let Some(outputs) = outputs {
                f(*outputs);
            }
            f(*backend);
            f(*proof);
            f(*vk);
        }
        PointerEq { left, right, .. } => {
            f(*left);
            f(*right);
        }
        VrfVerify {
            input,
            public_key,
            proof,
            variant,
            ..
        } => {
            f(*input);
            f(*public_key);
            f(*proof);
            f(*variant);
        }
        VrfVerifyBatch { batch, .. } => f(*batch),
        AxtBegin { descriptor } => f(*descriptor),
        AxtTouch { dsid, manifest } => {
            f(*dsid);
            if let Some(m) = manifest {
                f(*m);
            }
        }
        VerifyDsProof { dsid, proof } => {
            f(*dsid);
            if let Some(p) = proof {
                f(*p);
            }
        }
        UseAssetHandle {
            handle,
            intent,
            proof,
        } => {
            f(*handle);
            f(*intent);
            if let Some(p) = proof {
                f(*p);
            }
        }
        AxtCommit => {}
    }
}

fn visit_terminator_uses<F: FnMut(Temp)>(term: &Terminator, mut f: F) {
    match term {
        Terminator::Return(Some(temp)) => f(*temp),
        Terminator::Return2(t0, t1) => {
            f(*t0);
            f(*t1);
        }
        Terminator::ReturnN(vals) => {
            for temp in vals {
                f(*temp);
            }
        }
        Terminator::Branch { cond, .. } => f(*cond),
        Terminator::Return(None) | Terminator::Jump(_) => {}
    }
}

fn dest_temp(instr: &Instr) -> Option<Temp> {
    match instr {
        Instr::PointerEq { dest, .. }
        | Instr::Const { dest, .. }
        | Instr::StringConst { dest, .. }
        | Instr::DataRef { dest, .. }
        | Instr::Binary { dest, .. }
        | Instr::WrappingBinary { dest, .. }
        | Instr::Unary { dest, .. }
        | Instr::WrappingNeg { dest, .. }
        | Instr::Min { dest, .. }
        | Instr::Max { dest, .. }
        | Instr::Abs { dest, .. }
        | Instr::DivCeil { dest, .. }
        | Instr::Gcd { dest, .. }
        | Instr::Mean { dest, .. }
        | Instr::Isqrt { dest, .. }
        | Instr::LoadVar { dest, .. }
        | Instr::Poseidon2 { dest, .. }
        | Instr::Poseidon6 { dest, .. }
        | Instr::Pubkgen { dest, .. }
        | Instr::Valcom { dest, .. }
        | Instr::MapNew { dest }
        | Instr::GetAuthority { dest }
        | Instr::SysvarAuthority { dest }
        | Instr::CurrentTimeMs { dest }
        | Instr::BlockHeight { dest }
        | Instr::BlockTimeMs { dest }
        | Instr::ChainId { dest }
        | Instr::ContractAddress { dest }
        | Instr::Entrypoint { dest }
        | Instr::CallContract { dest, .. }
        | Instr::QueryExecuteNorito { dest, .. }
        | Instr::QueryGet { dest, .. }
        | Instr::GetAccountBalance { dest, .. }
        | Instr::Alloc { dest, .. }
        | Instr::GetPublicInput { dest, .. }
        | Instr::GetPrivateInput { dest, .. }
        | Instr::ZkRootsGet { dest, .. }
        | Instr::ZkVoteGetTally { dest, .. }
        | Instr::VrfEpochSeed { dest, .. }
        | Instr::SoracloudHostCall { dest, .. }
        | Instr::ResolveAccountAlias { dest, .. }
        | Instr::GetTriggerEvent { dest }
        | Instr::ActorAccount { dest, .. }
        | Instr::ActorPublicKey { dest, .. }
        | Instr::ActorSign { dest, .. }
        | Instr::Copy { dest, .. }
        | Instr::PointerFromString { dest, .. }
        | Instr::PointerToNorito { dest, .. }
        | Instr::PointerFromNorito { dest, .. }
        | Instr::Load64Imm { dest, .. }
        | Instr::StateGet { dest, .. }
        | Instr::StateKeys { dest, .. }
        | Instr::StateMapKeyAt { dest, .. }
        | Instr::StateValueEncode { dest, .. }
        | Instr::StateHas { dest, .. }
        | Instr::StateLen { dest, .. }
        | Instr::StateCount { dest, .. }
        | Instr::NumericFromInt { dest, .. }
        | Instr::NumericToInt { dest, .. }
        | Instr::NumericNeg { dest, .. }
        | Instr::NumericBinary { dest, .. }
        | Instr::NumericCompare { dest, .. }
        | Instr::DirectHelperSyscall { dest, .. } => Some(*dest),
        Instr::SchemaInfo { dest, .. } => Some(*dest),
        Instr::Sm3Hash { dest, .. }
        | Instr::Sha256Hash { dest, .. }
        | Instr::Sha3Hash { dest, .. }
        | Instr::Blake2b256Hash { dest, .. }
        | Instr::Keccak256Hash { dest, .. }
        | Instr::IrohaHash { dest, .. } => Some(*dest),
        Instr::Sm2Verify { dest, .. } => Some(*dest),
        Instr::VerifySignature { dest, .. } => Some(*dest),
        Instr::ProveExecution { dest } => Some(*dest),
        Instr::GrowHeap { dest, .. } => Some(*dest),
        Instr::GetMerklePath { dest, .. }
        | Instr::GetMerkleCompact { dest, .. }
        | Instr::GetRegisterMerkleCompact { dest, .. } => Some(*dest),
        Instr::VerifyProof { dest, .. } => Some(*dest),
        Instr::Sm4GcmSeal { dest, .. } => Some(*dest),
        Instr::Sm4GcmOpen { dest, .. } => Some(*dest),
        Instr::Sm4CcmSeal { dest, .. } => Some(*dest),
        Instr::Sm4CcmOpen { dest, .. } => Some(*dest),
        Instr::VrfVerify { dest, .. } => Some(*dest),
        Instr::VrfVerifyBatch { dest, .. } => Some(*dest),
        Instr::MapGet { dest, .. } => Some(*dest),
        Instr::DecodeInt { dest, .. } => Some(*dest),
        Instr::TlvLen { dest, .. } => Some(*dest),
        Instr::EncodeInt { dest, .. } => Some(*dest),
        Instr::JsonObject { dest, .. } => Some(*dest),
        Instr::JsonSetInt { dest, .. } => Some(*dest),
        Instr::JsonSetAccountId { dest, .. } => Some(*dest),
        Instr::PathMapKey { dest, .. } => Some(*dest),
        Instr::PathMapKeyNorito { dest, .. } => Some(*dest),
        Instr::JsonEncode { dest, .. } => Some(*dest),
        Instr::JsonDecode { dest, .. } => Some(*dest),
        Instr::JsonGetInt { dest, .. }
        | Instr::JsonGetNumeric { dest, .. }
        | Instr::JsonGetJson { dest, .. }
        | Instr::JsonGetName { dest, .. }
        | Instr::JsonGetAccountId { dest, .. }
        | Instr::JsonGetAssetDefinitionId { dest, .. }
        | Instr::JsonGetNftId { dest, .. }
        | Instr::JsonGetBlobHex { dest, .. } => Some(*dest),
        Instr::NameDecode { dest, .. } => Some(*dest),
        Instr::SchemaEncode { dest, .. } => Some(*dest),
        Instr::SchemaDecode { dest, .. } => Some(*dest),
        Instr::TuplePack { dest, .. } => Some(*dest),
        Instr::TupleGet { dest, .. } => Some(*dest),
        Instr::BuildSubmitBallotInline { dest, .. } => Some(*dest),
        Instr::BuildUnshieldInline { dest, .. } => Some(*dest),
        Instr::VendorExecuteQuery { dest, .. } => Some(*dest),
        Instr::Call { dest, .. } | Instr::InvokeEntrypointAs { dest, .. } => dest.as_ref().copied(),
        Instr::GrantPermission { .. }
        | Instr::RevokePermission { .. }
        | Instr::RegisterAsset { .. }
        | Instr::CreateNewAsset { .. }
        | Instr::TransferAsset { .. }
        | Instr::TransferBatchAsset { .. }
        | Instr::EscrowOpenOffer { .. }
        | Instr::EscrowAccept { .. }
        | Instr::EscrowMarkPaymentSent { .. }
        | Instr::EscrowRelease { .. }
        | Instr::EscrowCancel { .. }
        | Instr::EscrowOpenDispute { .. }
        | Instr::EscrowResolveDispute { .. }
        | Instr::AnonymousEscrowOpenOffer { .. }
        | Instr::AnonymousEscrowAccept { .. }
        | Instr::AnonymousEscrowMarkPaymentSent { .. }
        | Instr::AnonymousEscrowRelease { .. }
        | Instr::AnonymousEscrowCancel { .. }
        | Instr::AnonymousEscrowOpenDispute { .. }
        | Instr::AnonymousEscrowResolveDispute { .. }
        | Instr::MintAsset { .. }
        | Instr::BurnAsset { .. }
        | Instr::CreateNft { .. }
        | Instr::TransferNft { .. }
        | Instr::CreateNftsForAllUsers
        | Instr::SetExecutionDepth { .. }
        | Instr::SetVl { .. }
        | Instr::SetAccountDetail { .. }
        | Instr::RegisterDomain { .. }
        | Instr::RegisterAccount { .. }
        | Instr::AddSignatory { .. }
        | Instr::RemoveSignatory { .. }
        | Instr::SetAccountQuorum { .. }
        | Instr::UnregisterDomain { .. }
        | Instr::UnregisterAsset { .. }
        | Instr::UnregisterAccount { .. }
        | Instr::RegisterPeer { .. }
        | Instr::UnregisterPeer { .. }
        | Instr::CreateTrigger { .. }
        | Instr::RemoveTrigger { .. }
        | Instr::SetTriggerEnabled { .. }
        | Instr::CreateRole { .. }
        | Instr::DeleteRole { .. }
        | Instr::GrantRole { .. }
        | Instr::RevokeRole { .. }
        | Instr::ZkVerify { .. }
        | Instr::VendorExecuteInstruction { .. }
        | Instr::SubscriptionBill
        | Instr::SubscriptionRecordUsage
        | Instr::AssertEq { .. }
        | Instr::Assert { .. }
        | Instr::AbortIf { .. }
        | Instr::Info { .. }
        | Instr::DebugPrint { .. }
        | Instr::DebugLog { .. }
        | Instr::MapSet { .. }
        | Instr::SetNftData { .. }
        | Instr::BurnNft { .. }
        | Instr::TransferDomain { .. }
        | Instr::StateSet { .. }
        | Instr::StateDel { .. }
        | Instr::AxtBegin { .. }
        | Instr::AxtTouch { .. }
        | Instr::VerifyDsProof { .. }
        | Instr::UseAssetHandle { .. }
        | Instr::AxtCommit
        | Instr::TransferBatchBegin
        | Instr::TransferBatchEnd
        | Instr::TransferBatchApply { .. }
        | Instr::UseNullifier { .. }
        | Instr::CommitOutput
        | Instr::SmartContractLifecycle { .. }
        | Instr::ExpectRejectAs { .. } => None,
        Instr::CallMulti { .. }
        | Instr::InvokeEntrypointAsMulti { .. }
        | Instr::MapLoadPair { .. } => None,
    }
}

/// Visit every temporary defined by one IR instruction.
pub(crate) fn visit_instr_defs<F: FnMut(Temp)>(instruction: &Instr, mut visit: F) {
    if let Some(dest) = dest_temp(instruction) {
        visit(dest);
    }
    match instruction {
        Instr::MapLoadPair {
            dest_key, dest_val, ..
        } => {
            visit(*dest_key);
            visit(*dest_val);
        }
        Instr::CallMulti { dests, .. } | Instr::InvokeEntrypointAsMulti { dests, .. } => {
            for dest in dests {
                visit(*dest);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{self, BasicBlock, Instr, Terminator};

    #[test]
    fn reuse_registers_when_intervals_do_not_overlap() {
        let mut blocks = Vec::new();
        let mut instrs = Vec::new();
        for i in 0..35 {
            instrs.push(Instr::Const {
                dest: Temp(i),
                value: i as i64,
            });
        }
        blocks.push(BasicBlock {
            label: ir::Label(0),
            instrs,
            terminator: Terminator::Return(None),
        });
        let func = Function {
            name: "f".into(),
            params: vec![],
            blocks,
            entry: ir::Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };
        let alloc = allocate(&func);
        assert!(alloc.stack.is_empty());
        assert_eq!(alloc.frame_size, 0);
        for &reg in alloc.regs.values() {
            assert!(ARG_REGS.contains(&reg));
        }
    }

    #[test]
    fn precolors_leaf_parameters_in_abi_argument_registers() {
        let parameter = Temp(0);
        let func = Function {
            name: "identity".into(),
            params: vec!["value".into()],
            blocks: vec![BasicBlock {
                label: Label(0),
                instrs: vec![Instr::LoadVar {
                    dest: parameter,
                    name: "value".into(),
                }],
                terminator: Terminator::Return(Some(parameter)),
            }],
            entry: Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };

        let alloc = allocate(&func);
        assert_eq!(alloc.regs.get(&parameter), Some(&RET_REG));
        assert!(alloc.stack.is_empty());
        assert_eq!(alloc.frame_size, 0);
        assert!(uses_caller_saved_pool(&func));
        assert!(!has_internal_calls(&func));
    }

    #[test]
    fn optimizer_removes_unreachable_blocks_and_dead_ssa_chains() {
        let dead_source = Temp(0);
        let dead_copy = Temp(1);
        let live = Temp(2);
        let unreachable = Temp(3);
        let mut program = Program {
            functions: vec![Function {
                name: "optimized".into(),
                params: vec![],
                blocks: vec![
                    BasicBlock {
                        label: Label(0),
                        instrs: vec![
                            Instr::Const {
                                dest: dead_source,
                                value: 1,
                            },
                            Instr::Copy {
                                dest: dead_copy,
                                src: dead_source,
                            },
                            Instr::Const {
                                dest: live,
                                value: 2,
                            },
                        ],
                        terminator: Terminator::Return(Some(live)),
                    },
                    BasicBlock {
                        label: Label(1),
                        instrs: vec![Instr::StateDel { path: unreachable }],
                        terminator: Terminator::Return(None),
                    },
                ],
                entry: Label(0),
                location: crate::ast::SourceLocation { line: 1, column: 1 },
            }],
        };

        optimize_program(&mut program);
        let function = &program.functions[0];
        assert_eq!(function.blocks.len(), 1);
        assert_eq!(
            function.blocks[0].instrs,
            vec![Instr::Const {
                dest: live,
                value: 2,
            }]
        );
    }

    #[test]
    fn optimizer_keeps_checked_arithmetic_that_can_trap() {
        let left = Temp(0);
        let right = Temp(1);
        let result = Temp(2);
        let mut program = Program {
            functions: vec![Function {
                name: "checked".into(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: Label(0),
                    instrs: vec![
                        Instr::Const {
                            dest: left,
                            value: i64::MAX,
                        },
                        Instr::Const {
                            dest: right,
                            value: 1,
                        },
                        Instr::Binary {
                            dest: result,
                            op: crate::ast::BinaryOp::Add,
                            left,
                            right,
                        },
                    ],
                    terminator: Terminator::Return(None),
                }],
                entry: Label(0),
                location: crate::ast::SourceLocation { line: 1, column: 1 },
            }],
        };

        optimize_program(&mut program);
        assert!(
            program.functions[0].blocks[0]
                .instrs
                .iter()
                .any(|instruction| matches!(instruction, Instr::Binary { .. }))
        );
    }

    #[test]
    fn optimizer_folds_constant_branch_and_removes_unreachable_effects() {
        let left = Temp(0);
        let right = Temp(1);
        let cond = Temp(2);
        let result = Temp(3);
        let mut program = Program {
            functions: vec![Function {
                name: "constant_branch".into(),
                params: vec![],
                blocks: vec![
                    BasicBlock {
                        label: Label(0),
                        instrs: vec![
                            Instr::Const {
                                dest: left,
                                value: 3,
                            },
                            Instr::Const {
                                dest: right,
                                value: 4,
                            },
                            Instr::Binary {
                                dest: cond,
                                op: crate::ast::BinaryOp::Lt,
                                left,
                                right,
                            },
                        ],
                        terminator: Terminator::Branch {
                            cond,
                            then_bb: Label(1),
                            else_bb: Label(2),
                        },
                    },
                    BasicBlock {
                        label: Label(1),
                        instrs: vec![Instr::Const {
                            dest: result,
                            value: 9,
                        }],
                        terminator: Terminator::Return(Some(result)),
                    },
                    BasicBlock {
                        label: Label(2),
                        instrs: vec![Instr::StateDel { path: Temp(99) }],
                        terminator: Terminator::Return(None),
                    },
                ],
                entry: Label(0),
                location: crate::ast::SourceLocation { line: 1, column: 1 },
            }],
        };

        optimize_program(&mut program);

        let function = &program.functions[0];
        assert_eq!(function.entry, Label(1));
        assert_eq!(function.blocks.len(), 1);
        assert_eq!(
            function.blocks[0].instrs,
            vec![Instr::Const {
                dest: result,
                value: 9,
            }]
        );
    }

    #[test]
    fn optimizer_does_not_fold_checked_trap_boundaries() {
        use crate::ast::{BinaryOp, UnaryOp};

        let max = Temp(0);
        let one = Temp(1);
        let min = Temp(2);
        let minus_one = Temp(3);
        let zero = Temp(4);
        let mut program = Program {
            functions: vec![Function {
                name: "checked_boundaries".into(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: Label(0),
                    instrs: vec![
                        Instr::Const {
                            dest: max,
                            value: i64::MAX,
                        },
                        Instr::Const {
                            dest: one,
                            value: 1,
                        },
                        Instr::Const {
                            dest: min,
                            value: i64::MIN,
                        },
                        Instr::Const {
                            dest: minus_one,
                            value: -1,
                        },
                        Instr::Const {
                            dest: zero,
                            value: 0,
                        },
                        Instr::Binary {
                            dest: Temp(5),
                            op: BinaryOp::Add,
                            left: max,
                            right: one,
                        },
                        Instr::Binary {
                            dest: Temp(6),
                            op: BinaryOp::Div,
                            left: min,
                            right: minus_one,
                        },
                        Instr::Binary {
                            dest: Temp(7),
                            op: BinaryOp::Div,
                            left: one,
                            right: zero,
                        },
                        Instr::Unary {
                            dest: Temp(8),
                            op: UnaryOp::Neg,
                            operand: min,
                        },
                    ],
                    terminator: Terminator::Return(None),
                }],
                entry: Label(0),
                location: crate::ast::SourceLocation { line: 1, column: 1 },
            }],
        };

        optimize_program(&mut program);

        let instructions = &program.functions[0].blocks[0].instrs;
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Binary { dest: Temp(5), .. }))
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Binary { dest: Temp(6), .. }))
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Binary { dest: Temp(7), .. }))
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction, Instr::Unary { dest: Temp(8), .. }))
        );
    }

    #[test]
    fn optimizer_coalesces_unknown_copy_chains() {
        let input = Temp(0);
        let first_copy = Temp(1);
        let result = Temp(2);
        let mut program = Program {
            functions: vec![Function {
                name: "copy_chain".into(),
                params: vec!["value".into()],
                blocks: vec![BasicBlock {
                    label: Label(0),
                    instrs: vec![
                        Instr::LoadVar {
                            dest: input,
                            name: "value".into(),
                        },
                        Instr::Copy {
                            dest: first_copy,
                            src: input,
                        },
                        Instr::Copy {
                            dest: result,
                            src: first_copy,
                        },
                    ],
                    terminator: Terminator::Return(Some(result)),
                }],
                entry: Label(0),
                location: crate::ast::SourceLocation { line: 1, column: 1 },
            }],
        };

        optimize_program(&mut program);

        assert_eq!(
            program.functions[0].blocks[0].instrs,
            vec![Instr::LoadVar {
                dest: result,
                name: "value".into(),
            }]
        );
    }

    #[test]
    fn optimizer_eliminates_folded_dead_work() {
        let left = Temp(0);
        let right = Temp(1);
        let result = Temp(2);
        let mut program = Program {
            functions: vec![Function {
                name: "dead_constants".into(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: Label(0),
                    instrs: vec![
                        Instr::Const {
                            dest: left,
                            value: 20,
                        },
                        Instr::Const {
                            dest: right,
                            value: 22,
                        },
                        Instr::Binary {
                            dest: result,
                            op: crate::ast::BinaryOp::Add,
                            left,
                            right,
                        },
                    ],
                    terminator: Terminator::Return(None),
                }],
                entry: Label(0),
                location: crate::ast::SourceLocation { line: 1, column: 1 },
            }],
        };

        optimize_program(&mut program);

        assert!(program.functions[0].blocks[0].instrs.is_empty());
    }

    #[test]
    fn optimizer_simplifies_safe_algebra_without_dropping_input() {
        let input = Temp(0);
        let one = Temp(1);
        let result = Temp(2);
        let mut program = Program {
            functions: vec![Function {
                name: "identity_multiply".into(),
                params: vec!["value".into()],
                blocks: vec![BasicBlock {
                    label: Label(0),
                    instrs: vec![
                        Instr::LoadVar {
                            dest: input,
                            name: "value".into(),
                        },
                        Instr::Const {
                            dest: one,
                            value: 1,
                        },
                        Instr::Binary {
                            dest: result,
                            op: crate::ast::BinaryOp::Mul,
                            left: input,
                            right: one,
                        },
                    ],
                    terminator: Terminator::Return(Some(result)),
                }],
                entry: Label(0),
                location: crate::ast::SourceLocation { line: 1, column: 1 },
            }],
        };

        optimize_program(&mut program);

        assert_eq!(
            program.functions[0].blocks[0].instrs,
            vec![Instr::LoadVar {
                dest: result,
                name: "value".into(),
            }]
        );
    }

    #[test]
    fn spills_when_live_set_exceeds_pool() {
        let live = ALLOC_POOL.len() + 4;
        let mut blocks = Vec::new();
        let mut instrs = Vec::new();
        for i in 0..live {
            instrs.push(Instr::Const {
                dest: Temp(i),
                value: i as i64,
            });
        }
        instrs.push(Instr::TuplePack {
            dest: Temp(live),
            items: (0..live).map(Temp).collect(),
        });
        blocks.push(BasicBlock {
            label: ir::Label(0),
            instrs,
            terminator: Terminator::Return(None),
        });
        let func = Function {
            name: "g".into(),
            params: vec![],
            blocks,
            entry: ir::Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };
        let alloc = allocate(&func);
        assert!(
            !alloc.stack.is_empty(),
            "expected spills when live set exceeds pool"
        );
        assert!(alloc.frame_size > 0);
        assert_eq!(alloc.frame_size % 16, 0);
    }

    #[test]
    fn splits_a_long_spill_into_one_reload_for_a_reuse_cluster() {
        let reused = Temp(0);
        let mut instructions = vec![Instr::LoadVar {
            dest: reused,
            name: "reused".into(),
        }];
        for index in 1..=ALLOC_POOL.len() {
            instructions.push(Instr::Const {
                dest: Temp(index),
                value: index as i64,
            });
        }
        // These effectful uses keep every short-lived value live during the
        // definition-pressure prefix, then release all registers before the
        // repeated uses of `reused`.
        for index in 1..=ALLOC_POOL.len() {
            instructions.push(Instr::DebugPrint { value: Temp(index) });
        }
        let first_result = Temp(ALLOC_POOL.len() + 1);
        let second_result = Temp(ALLOC_POOL.len() + 2);
        let first_use_position = instructions.len();
        instructions.push(Instr::Binary {
            dest: first_result,
            op: crate::ast::BinaryOp::Add,
            left: reused,
            right: reused,
        });
        instructions.push(Instr::Binary {
            dest: second_result,
            op: crate::ast::BinaryOp::Add,
            left: reused,
            right: reused,
        });
        let function = Function {
            name: "split_reuse".into(),
            params: vec!["reused".into()],
            blocks: vec![BasicBlock {
                label: Label(0),
                instrs: instructions,
                terminator: Terminator::Return(Some(second_result)),
            }],
            entry: Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };

        let baseline = allocate(&function);
        assert!(
            baseline.stack.contains_key(&reused),
            "the long-lived value must be the initial spill: {baseline:#?}"
        );
        let plan = allocate_with_splitting(&function);
        let segments = plan.split_segments(reused);
        assert_eq!(segments.len(), 1, "expected one local reuse segment");
        assert_eq!(segments[0].start, first_use_position);
        assert_eq!(segments[0].end, first_use_position + 1);
        assert_eq!(segments[0].use_count, 4);
        assert!(
            baseline
                .regs
                .values()
                .any(|register| *register == segments[0].register),
            "a non-leaf split must reuse an already-preserved register"
        );
        assert_eq!(plan.frame_size, baseline.frame_size);
        assert_eq!(plan.reloads_at(first_use_position).len(), 1);
        assert_eq!(plan.saved_reload_count(), 3);
        assert_eq!(
            plan.register_for_use(reused, first_use_position),
            plan.register_for_use(reused, first_use_position + 1)
        );
        assert_eq!(
            plan,
            allocate_with_splitting(&function),
            "split allocation must be deterministic"
        );
    }

    #[test]
    fn split_candidates_reload_again_after_a_definition_epoch() {
        let value = Temp(0);
        let replacement = Temp(1);
        let function = Function {
            name: "split_definition_epoch".into(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: Label(0),
                instrs: vec![
                    Instr::Binary {
                        dest: Temp(2),
                        op: crate::ast::BinaryOp::Add,
                        left: value,
                        right: value,
                    },
                    Instr::Copy {
                        dest: value,
                        src: replacement,
                    },
                    Instr::Binary {
                        dest: Temp(3),
                        op: crate::ast::BinaryOp::Add,
                        left: value,
                        right: value,
                    },
                ],
                terminator: Terminator::Return(None),
            }],
            entry: Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };
        let home = Allocation {
            regs: HashMap::new(),
            stack: [(value, 0)].into_iter().collect(),
            frame_size: 16,
        };

        let mut candidates = collect_split_candidates(&function, &home);
        candidates.sort_unstable_by_key(|candidate| candidate.start);
        assert_eq!(
            candidates,
            vec![
                SplitCandidate {
                    temp: value,
                    start: 0,
                    end: 0,
                    use_count: 2,
                },
                SplitCandidate {
                    temp: value,
                    start: 2,
                    end: 2,
                    use_count: 2,
                },
            ],
            "a definition must terminate the prior read-only split segment"
        );
    }

    #[test]
    fn split_candidates_exclude_virtual_and_rematerialized_stack_homes() {
        let literal = Temp(0);
        let rematerialized_copy = Temp(1);
        let virtual_tuple = Temp(2);
        let function = Function {
            name: "split_rematerialization".into(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: Label(0),
                instrs: vec![
                    Instr::Const {
                        dest: literal,
                        value: 7,
                    },
                    Instr::Copy {
                        dest: rematerialized_copy,
                        src: literal,
                    },
                    Instr::TuplePack {
                        dest: virtual_tuple,
                        items: vec![rematerialized_copy],
                    },
                    Instr::Binary {
                        dest: Temp(3),
                        op: crate::ast::BinaryOp::Add,
                        left: rematerialized_copy,
                        right: rematerialized_copy,
                    },
                    Instr::Binary {
                        dest: Temp(4),
                        op: crate::ast::BinaryOp::Add,
                        left: rematerialized_copy,
                        right: rematerialized_copy,
                    },
                ],
                terminator: Terminator::Return2(virtual_tuple, virtual_tuple),
            }],
            entry: Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };
        let home = Allocation {
            regs: HashMap::new(),
            stack: [(rematerialized_copy, 0), (virtual_tuple, 8)]
                .into_iter()
                .collect(),
            frame_size: 16,
        };

        assert!(
            collect_split_candidates(&function, &home).is_empty(),
            "literal copies and metadata-only tuples must never trigger stack reloads"
        );
    }

    #[test]
    fn deterministic_allocation_for_equal_start_intervals() {
        let dest0 = Temp(0);
        let dest1 = Temp(1);
        let instrs = vec![Instr::CallMulti {
            callee: "f".into(),
            args: Vec::new(),
            dests: vec![dest0, dest1],
        }];
        let block = BasicBlock {
            label: ir::Label(0),
            instrs,
            terminator: Terminator::Return(None),
        };
        let func = Function {
            name: "f".into(),
            params: vec![],
            blocks: vec![block],
            entry: ir::Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };
        let alloc = allocate(&func);
        let expected_first = *ALLOC_POOL.last().expect("alloc pool");
        let expected_second = ALLOC_POOL[ALLOC_POOL.len() - 2];
        assert_eq!(alloc.regs.get(&dest0), Some(&expected_first));
        assert_eq!(alloc.regs.get(&dest1), Some(&expected_second));
    }

    #[test]
    fn reuses_spill_slots_for_non_overlapping_pressure_phases() {
        fn pressure_phase(instrs: &mut Vec<Instr>, next_temp: &mut usize, live: usize) {
            let first = *next_temp;
            for _ in 0..live {
                let dest = Temp(*next_temp);
                *next_temp += 1;
                instrs.push(Instr::Const {
                    dest,
                    value: dest.0 as i64,
                });
            }
            let tuple = Temp(*next_temp);
            *next_temp += 1;
            instrs.push(Instr::TuplePack {
                dest: tuple,
                items: (first..first + live).map(Temp).collect(),
            });
        }

        let live = ALLOC_POOL.len() + 4;
        let mut one_phase = Vec::new();
        let mut next_temp = 0;
        pressure_phase(&mut one_phase, &mut next_temp, live);
        let one_phase = Function {
            name: "one_phase".into(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: Label(0),
                instrs: one_phase,
                terminator: Terminator::Return(None),
            }],
            entry: Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };

        let mut two_phases = Vec::new();
        let mut next_temp = 0;
        pressure_phase(&mut two_phases, &mut next_temp, live);
        pressure_phase(&mut two_phases, &mut next_temp, live);
        let two_phases = Function {
            name: "two_phases".into(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: Label(0),
                instrs: two_phases,
                terminator: Terminator::Return(None),
            }],
            entry: Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };

        let one = allocate(&one_phase);
        let two = allocate(&two_phases);
        assert!(!one.stack.is_empty());
        assert_eq!(
            two.frame_size, one.frame_size,
            "disjoint pressure phases must share the same stack-slot high-water mark"
        );
        assert!(two.stack.len() > one.stack.len());
        assert!(
            two.stack.values().any(|slot| two
                .stack
                .values()
                .filter(|other| *other == slot)
                .count()
                > 1),
            "at least one physical spill slot should be shared by disjoint intervals"
        );
    }

    #[test]
    fn spill_slot_coloring_preserves_full_interval_overlap() {
        let long = Temp(0);
        let early = Temp(1);
        let late = Temp(2);
        let intervals = [
            Interval {
                temp: long,
                start: 0,
                end: 10,
            },
            Interval {
                temp: early,
                start: 1,
                end: 4,
            },
            Interval {
                temp: late,
                start: 5,
                end: 6,
            },
        ];
        let spilled = [long, early, late].into_iter().collect();
        let (slots, high_water) = assign_spill_slots(&intervals, &spilled);

        assert_ne!(slots[&long], slots[&early]);
        assert_eq!(slots[&early], slots[&late]);
        assert_eq!(high_water, 16);
    }
}
