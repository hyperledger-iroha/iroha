//! Funded fixed-point traversal of prepared static-state access facts.
//!
//! The workspace is optional scheduler analysis, never execution authority.
//! Facts, queue, borrowed literal indexes, temporary NFC scratch, symbolic key
//! scratch and published key storage retain their respective original charges.

use super::{
    StaticStateAccessAnalysis, StaticStateFacts, direct_call_edges, is_protected_contract_return,
    static_state_keys::KeyScratch,
    static_state_literals::{LiteralSource, PreparedLiterals},
    static_state_workspace::{Roots, Workspace},
    transfer_static_state_facts,
};
use crate::{PreparedContract, VMError, instruction::wide};
use iroha_allocation::AllocationBudget;

pub(super) fn analyze(
    contract: &PreparedContract,
    entrypoint: Option<&str>,
    budget: &AllocationBudget,
) -> Result<Option<StaticStateAccessAnalysis>, VMError> {
    let Some(roots) = Roots::new(contract, entrypoint) else {
        return Ok(None);
    };
    if roots
        .iter()
        .any(|root| !contract.is_instruction_boundary(root))
    {
        return Ok(None);
    }
    let mut workspace = Workspace::new(contract.decoded().len(), budget)?;
    let mut keys = KeyScratch::new(contract.decoded(), budget)?;
    let literals = PreparedLiterals::new(contract, budget)?;
    let Some(mut result) = traverse(contract, &roots, &mut workspace, &mut keys, &literals)? else {
        return Ok(None);
    };
    (result.read_keys, result.write_keys) = keys.finish(&literals, budget)?;
    Ok(Some(result))
}

fn traverse(
    contract: &PreparedContract,
    roots: &Roots<'_>,
    workspace: &mut Workspace,
    keys: &mut KeyScratch,
    literals: &impl LiteralSource,
) -> Result<Option<StaticStateAccessAnalysis>, VMError> {
    let decoded = contract.decoded();
    for root in roots.iter() {
        let Ok(index) = decoded.binary_search_by_key(&root, |op| op.pc) else {
            return Ok(None);
        };
        if workspace
            .merge(index, &StaticStateFacts::entrypoint())
            .is_err()
        {
            return Ok(None);
        }
    }
    let mut result = StaticStateAccessAnalysis {
        complete: true,
        ..StaticStateAccessAnalysis::default()
    };
    while let Some((index, mut outgoing)) = workspace.pop() {
        let Some(op) = decoded.get(index) else {
            return Ok(None);
        };
        let pc = op.pc;
        if let Some(key) = transfer_static_state_facts(op, literals, &mut outgoing, &mut result) {
            keys.record(pc, key, literals)?;
        }
        if contract.has_indirect_control_flow(pc) && !is_protected_contract_return(op) {
            // A general indirect edge has no authenticated target in the
            // prepared control-flow graph. Treat the proof as incomplete even
            // when the visible instruction itself is not a durable-state
            // syscall: otherwise a JR/JALR target could hide an unaccounted
            // state access while the scheduler accepts an exact access set.
            //
            // Prepared contracts are always executed with strict return
            // integrity. Under that runtime policy the one canonical return
            // encoding below can only target the protected call stack (or the
            // validated outer-return sentinel), so it is a terminal edge rather
            // than an unauthenticated computed jump. This exception is enforced
            // by `IVM::load_prepared`'s protected return stack.
            result.complete = false;
        }
        let Some(successors) = contract.control_flow_successors(pc) else {
            return Ok(None);
        };
        let call_edges = direct_call_edges(op);
        // Production Kotodama entrypoints are authenticated two-instruction
        // thunks (`call body; halt`). Crossing that compiler-owned boundary is
        // still direct entrypoint code; calls made by the body remain
        // conservative helper edges.
        let entrypoint_wrapper_call = call_edges.is_some_and(|(_, return_pc)| {
            roots.contains(pc)
                && decoded
                    .binary_search_by_key(&return_pc, |candidate| candidate.pc)
                    .ok()
                    .and_then(|index| decoded.get(index))
                    .is_some_and(|candidate| wide::opcode(candidate.inst) == wide::control::HALT)
        });
        for successor in successors.iter().copied() {
            let mut successor_facts = outgoing.clone();
            if let Some((call_target, return_pc)) = call_edges {
                if successor == call_target && !entrypoint_wrapper_call {
                    // Even when a helper's path is literal, keep helper-hidden
                    // state access conservative.
                    successor_facts.direct = false;
                }
                if successor == return_pc {
                    // A callee may overwrite any caller-visible register, so a
                    // literal loaded before the call is not proof of a later state
                    // target. The caller can recover exactness by loading a fresh
                    // authenticated literal after the call.
                    successor_facts.clear();
                }
            }
            let Ok(next) = decoded.binary_search_by_key(&successor, |candidate| candidate.pc)
            else {
                return Ok(None);
            };
            if workspace.merge(next, &successor_facts).is_err() {
                return Ok(None);
            }
        }
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests;
