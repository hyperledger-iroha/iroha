//! Bounded traversal of exact callable types and active aggregate payloads.

use super::IVM;
use crate::{Memory, VMError};
use ivm_abi::{
    call::{CallSchemaV1, CallTypeNodeV1, MAX_CALL_SCHEMA_DEPTH_V1},
    list::ListLayoutV1,
    sum::SumLayoutV1,
};

#[derive(Clone, Copy, Default)]
struct Sequence {
    first: usize,
    next: usize,
    end: usize,
    address: u64,
    repetitions: u64,
    returned: bool,
}

impl IVM {
    pub(super) fn validate_call_values(
        &mut self,
        schema: &CallSchemaV1,
        layout_offset: usize,
        base: u64,
        returned: bool,
    ) -> Result<(), VMError> {
        // One continuation per schema level, never one per field/element. Repeated
        // List elements reuse their continuation and only visit logical contents.
        let mut stack = [Sequence::default(); MAX_CALL_SCHEMA_DEPTH_V1];
        let mut depth = 1;
        stack[0] = Sequence {
            end: schema.nodes.len(),
            address: base,
            repetitions: 1,
            returned,
            ..Sequence::default()
        };
        while depth != 0 {
            let current = &mut stack[depth - 1];
            if current.next == current.end {
                current.repetitions -= 1;
                if current.repetitions == 0 {
                    depth -= 1;
                } else {
                    current.next = current.first;
                }
                continue;
            }
            let index = current.next;
            let address = current.address;
            let is_returned = current.returned;
            let layout = self
                .call_layouts
                .as_ref()
                .ok_or(VMError::DecodeError)?
                .node(layout_offset, index)?;
            let role = schema.nodes.get(index).ok_or(VMError::DecodeError)?;
            self.debit_gas(crate::call_gas::NODE)?;
            current.next = layout.subtree_end;
            current.address = address
                .checked_add(
                    (layout.words as u64)
                        .checked_mul(8)
                        .ok_or(VMError::DecodeError)?,
                )
                .ok_or(VMError::DecodeError)?;
            let mut child = Sequence {
                first: index + 1,
                next: index + 1,
                end: layout.subtree_end,
                address,
                repetitions: 1,
                returned: is_returned,
            };
            match role {
                CallTypeNodeV1::Struct { fields, .. } if !fields.is_empty() => {}
                CallTypeNodeV1::Tuple(_) => {}
                _ => {
                    self.debit_gas(crate::call_gas::WORD)?;
                    let word = if is_returned {
                        let slot = address.checked_sub(base).ok_or(VMError::DecodeError)? / 8;
                        let (owned, word) = self.memory.active_call_result(
                            usize::try_from(slot).map_err(|_| VMError::DecodeError)?,
                        )?;
                        if owned != address {
                            return Err(VMError::AssertionFailed);
                        }
                        word
                    } else {
                        self.memory.load_u64(address)?
                    };
                    if self.memory_load_privacy_tag(address, 8)? != role.is_private() {
                        return Err(VMError::PrivacyViolation);
                    }
                    match role {
                        CallTypeNodeV1::Option | CallTypeNodeV1::Result => {
                            let first = self
                                .call_layouts
                                .as_ref()
                                .ok_or(VMError::DecodeError)?
                                .node(layout_offset, index + 1)?;
                            let is_option = matches!(role, CallTypeNodeV1::Option);
                            let sum = if is_option {
                                SumLayoutV1::option(first.words as u64)
                            } else {
                                let second = self
                                    .call_layouts
                                    .as_ref()
                                    .ok_or(VMError::DecodeError)?
                                    .node(layout_offset, first.subtree_end)?;
                                SumLayoutV1::try_new(second.words as u64, first.words as u64)
                            }
                            .map_err(|_| VMError::DecodeError)?;
                            self.debit_gas(crate::call_gas::WORD)?;
                            self.ensure_call_heap_footprint(
                                word,
                                sum.allocation_bytes().map_err(|_| VMError::DecodeError)?,
                            )?;
                            let tag = self.load_u64(word)?;
                            sum.active_words(tag).map_err(|_| VMError::DecodeError)?;
                            if is_option && tag == 0 {
                                continue;
                            }
                            child.first = if tag == 1 {
                                index + 1
                            } else {
                                first.subtree_end
                            };
                            child.next = child.first;
                            child.end = self
                                .call_layouts
                                .as_ref()
                                .ok_or(VMError::DecodeError)?
                                .node(layout_offset, child.first)?
                                .subtree_end;
                            child.address = word.checked_add(8).ok_or(VMError::DecodeError)?;
                            child.returned = false;
                        }
                        CallTypeNodeV1::List { capacity } => {
                            let element = self
                                .call_layouts
                                .as_ref()
                                .ok_or(VMError::DecodeError)?
                                .node(layout_offset, index + 1)?;
                            let list =
                                ListLayoutV1::try_new(u64::from(*capacity), element.words as u64)
                                    .map_err(|_| VMError::DecodeError)?;
                            self.debit_gas(2 * crate::call_gas::WORD)?;
                            self.ensure_call_heap_footprint(
                                word,
                                list.allocation_bytes().map_err(|_| VMError::DecodeError)?,
                            )?;
                            let length = self.load_u64(word)?;
                            let encoded_capacity = self.load_u64(word + 8)?;
                            if encoded_capacity != u64::from(*capacity) || length > encoded_capacity
                            {
                                return Err(VMError::DecodeError);
                            }
                            if length == 0 {
                                continue;
                            }
                            child.address = word.checked_add(16).ok_or(VMError::DecodeError)?;
                            child.repetitions = length;
                            child.returned = false;
                        }
                        _ => {
                            self.validate_call_word(address, word, role)?;
                            continue;
                        }
                    }
                }
            }
            if depth == stack.len() {
                return Err(VMError::DecodeError);
            }
            stack[depth] = child;
            depth += 1;
        }
        Ok(())
    }

    fn ensure_call_heap_footprint(&self, address: u64, bytes: u64) -> Result<(), VMError> {
        // Check the entire reserved shape without reading inactive Sum/List slots.
        // Active words and public headers get their own privacy checks after gas.
        let end = address.checked_add(bytes).ok_or(VMError::DecodeError)?;
        let heap_end = Memory::HEAP_START
            .checked_add(self.memory.heap_allocated_len())
            .ok_or(VMError::DecodeError)?;
        if !address.is_multiple_of(8) || address < Memory::HEAP_START || end > heap_end {
            return Err(VMError::DecodeError);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "values/tests.rs"]
mod tests;
