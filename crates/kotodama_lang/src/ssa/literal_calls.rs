//! Exact private numeric-literal calls folded within the canonical SSA program.

use super::{Function, Program};
use crate::ir::{DataRefKind, Instr, Terminator};
use std::collections::{BTreeMap, BTreeSet};

// This is an optimization eligibility bound, not a source or protocol limit.
// Do not replicate an arbitrarily long internal literal string at every call.
// A skipped helper follows the unchanged ordinary typed-call lowering.
const MAX_FOLDED_LITERAL_TEXT_BYTES: usize = 256;

impl Program {
    /// Substitute one already typed numeric literal at a private leaf call.
    ///
    /// The exact DataRef kind and payload still pass through ordinary canonical
    /// literal validation and charged LDLIT emission. Public roots, argument
    /// evaluation, host work, traps and nonliteral private helpers are untouched.
    /// The eligibility map comes from validated typed declarations, retaining
    /// exclusions that are no longer represented in SSA (including secret types
    /// and function attributes). This is one bounded pass, not recursive inlining.
    pub(super) fn fold_private_literal_calls(
        &mut self,
        roots: &BTreeSet<String>,
        private_literals: &BTreeMap<String, DataRefKind>,
    ) {
        let literals = self
            .functions
            .iter()
            .filter(|function| !roots.contains(&function.name))
            .filter_map(|function| {
                let expected_kind = private_literals.get(&function.name)?;
                let (kind, value) = returned_numeric_literal(function)?;
                (kind == *expected_kind).then(|| (function.name.clone(), (kind, value.to_owned())))
            })
            .collect::<BTreeMap<_, _>>();
        for function in &mut self.functions {
            for block in &mut function.blocks {
                for instruction in &mut block.instructions {
                    let Instr::Call {
                        callee,
                        args,
                        dest: Some(destination),
                    } = instruction.as_ir()
                    else {
                        continue;
                    };
                    if !args.is_empty() {
                        continue;
                    }
                    let Some((kind, value)) = literals.get(callee) else {
                        continue;
                    };
                    let replacement = Instr::DataRef {
                        dest: *destination,
                        kind: *kind,
                        value: value.clone(),
                    };
                    *instruction.as_ir_mut() = replacement;
                }
            }
        }
        // Do not run a second dead-value pass here. Even an unused call result
        // was previously emitted and validated; its replacement literal must
        // still reach the same canonical literal validator. Whole-program DCE
        // may now remove private bodies whose last actual call disappeared.
    }
}

fn returned_numeric_literal(function: &Function) -> Option<(DataRefKind, &str)> {
    if !function.params.is_empty() {
        return None;
    }
    let [block] = function.blocks.as_slice() else {
        return None;
    };
    if block.label != function.entry || !block.phis.is_empty() {
        return None;
    }
    let [instruction] = block.instructions.as_slice() else {
        return None;
    };
    let Instr::DataRef { dest, kind, value } = instruction.as_ir() else {
        return None;
    };
    if !matches!(
        kind,
        DataRefKind::Int | DataRefKind::Decimal | DataRefKind::Quantity
    ) || value.len() > MAX_FOLDED_LITERAL_TEXT_BYTES
        || block.terminator.as_ir() != &Terminator::Return(Some(*dest))
    {
        return None;
    }
    Some((*kind, value))
}

#[cfg(test)]
mod tests;
