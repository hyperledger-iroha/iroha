//! Original-pool immutable control-flow arrays built without temporary vectors.

use crate::{VMError, cache_memory::SharedAllocation, instruction::wide, ivm_cache::DecodedOp};
use iroha_allocation::AllocationBudget;

#[derive(Clone, Copy, Debug)]
pub(super) struct PreparedControlFlowNode {
    pc: u64,
    successors: [u64; 2],
    successor_count: u8,
    pub(super) has_indirect_successor: bool,
}
impl PreparedControlFlowNode {
    fn new(pc: u64) -> Self {
        Self {
            pc,
            successors: [0; 2],
            successor_count: 0,
            has_indirect_successor: false,
        }
    }
    fn push_successor(&mut self, target: u64) -> Result<(), VMError> {
        let index = usize::from(self.successor_count);
        let Some(slot) = self.successors.get_mut(index) else {
            return Err(VMError::DecodeError);
        };
        *slot = target;
        self.successor_count = self.successor_count.saturating_add(1);
        Ok(())
    }
    pub(super) fn successors(&self) -> &[u64] {
        &self.successors[..usize::from(self.successor_count)]
    }
}
#[derive(Clone, Debug)]
pub(crate) struct PreparedControlFlow {
    pub(super) boundaries: SharedAllocation<u64>,
    pub(super) nodes: SharedAllocation<PreparedControlFlowNode>,
}
impl PreparedControlFlow {
    /// Build both immutable arrays directly in their original allocation owner.
    /// The decoder already established the ordered fixed-width instruction stream.
    pub(crate) fn from_decoded(
        decoded: &[DecodedOp],
        budget: Option<&AllocationBudget>,
    ) -> Result<Self, VMError> {
        let values = decoded.iter().map(|op| Ok::<_, VMError>(op.pc));
        let boundaries = match budget {
            Some(budget) => SharedAllocation::try_from_iter_with_memory_budget(values, budget)?,
            None => SharedAllocation::try_from_iter(values)?,
        };
        let is_boundary = |pc: u64| boundaries.binary_search(&pc).is_ok();
        // SharedAllocation admits backing and control before consuming the
        // iterator, so node construction never grows uncharged temporary storage.
        let values = decoded.iter().map(|op| {
            let opcode = wide::opcode(op.inst);
            let fallthrough = op.pc.checked_add(4);
            let mut node = PreparedControlFlowNode::new(op.pc);
            match opcode {
                wide::control::HALT => {}
                wide::control::BEQ
                | wide::control::BNE
                | wide::control::BLT
                | wide::control::BGE
                | wide::control::BLTU
                | wide::control::BGEU => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                    let next = fallthrough.ok_or(VMError::DecodeError)?;
                    if !is_boundary(next) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(next)?;
                }
                wide::control::JAL => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                    if wide::rd(op.inst) != 0 {
                        let next = fallthrough.ok_or(VMError::DecodeError)?;
                        if !is_boundary(next) {
                            return Err(VMError::DecodeError);
                        }
                        node.push_successor(next)?;
                    }
                }
                wide::control::JMP => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                }
                wide::control::JALS => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                    let next = fallthrough.ok_or(VMError::DecodeError)?;
                    if !is_boundary(next) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(next)?;
                }
                wide::control::JALR | wide::control::JR => {
                    node.has_indirect_successor = true;
                }
                _ => {
                    if let Some(next) = fallthrough.filter(|next| is_boundary(*next)) {
                        node.push_successor(next)?;
                    }
                }
            }
            Ok::<_, VMError>(node)
        });
        let nodes = match budget {
            Some(budget) => SharedAllocation::try_from_iter_with_memory_budget(values, budget)?,
            None => SharedAllocation::try_from_iter(values)?,
        };
        Ok(Self { boundaries, nodes })
    }
    pub(super) fn node(&self, pc: u64) -> Option<&PreparedControlFlowNode> {
        let index = self.boundaries.binary_search(&pc).ok()?;
        let node = self.nodes.get(index)?;
        debug_assert_eq!(node.pc, pc);
        Some(node)
    }
}
fn direct_target(op: &DecodedOp) -> Option<u64> {
    let offset_words = match wide::opcode(op.inst) {
        wide::control::BEQ
        | wide::control::BNE
        | wide::control::BLT
        | wide::control::BGE
        | wide::control::BLTU
        | wide::control::BGEU => i64::from(wide::imm8(op.inst)),
        wide::control::JAL => i64::from(wide::imm16(op.inst)),
        wide::control::JMP | wide::control::JALS => i64::from(wide::imm24(op.inst)),
        _ => return None,
    };
    let byte_offset = offset_words.checked_mul(4)?;
    i128::from(op.pc)
        .checked_add(i128::from(byte_offset))
        .and_then(|target| u64::try_from(target).ok())
}

#[cfg(test)]
mod tests;
