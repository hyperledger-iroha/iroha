//! Linear cold build from the original State owner's canonical sorted stream.
use super::*;

/// Stream a complete, sorted image into the same radix layout as incremental
/// mutation. Each leaf and internal node is allocated and hashed exactly once.
pub struct KeyedStateBuilderV1 {
    tree: KeyedStateTreeV1,
    previous: Option<Owner>,
    pending: Option<ChargedBuffer<(u32, Owner)>>,
}

impl KeyedStateBuilderV1 {
    /// Start a cold build retaining the original State allocation budget.
    pub fn new(schema: Digest, budget: &AllocationBudget) -> Self {
        Self {
            tree: KeyedStateTreeV1::new(schema, budget),
            previous: None,
            pending: None,
        }
    }

    /// Append the next distinct key and its canonical value. A failed append
    /// preserves the previously accepted prefix exactly.
    ///
    /// # Errors
    /// Invalid, duplicate, unsorted or unrepresentable input, or allocation refusal.
    pub fn push(&mut self, key: &StateKeyV1, value: &[u8]) -> Result<(), KeyedStateError> {
        key.validate()?;
        let Some(previous) = &self.previous else {
            self.previous = Some(Node::leaf(key, value, &self.tree.budget)?);
            return Ok(());
        };
        let Kind::Leaf(leaf) = &previous.kind else {
            unreachable!("pending tail is a leaf")
        };
        let Some(bit) = key.first_diff(leaf.encoded_key()) else {
            return Err(KeyedStateError::Order);
        };
        if !key.right(bit) {
            return Err(KeyedStateError::Order);
        }
        let pending = self
            .pending
            .as_ref()
            .map_or(&[][..], ChargedBuffer::as_slice);
        let keep = pending.partition_point(|(split, _)| *split < bit);
        let mut left = previous.clone();
        for (split, child) in pending[keep..].iter().rev() {
            left = Node::branch(*split, child.clone(), left, &self.tree.budget)?;
        }
        let next = Node::leaf(key, value, &self.tree.budget)?;
        if self
            .pending
            .as_ref()
            .is_none_or(|stack| keep == stack.capacity())
        {
            let capacity = keep
                .checked_add(1)
                .and_then(usize::checked_next_power_of_two)
                .ok_or(KeyedStateError::Capacity)?;
            let mut grown = ChargedBuffer::new(capacity, &self.tree.budget)?;
            for item in &pending[..keep] {
                assert!(grown.try_push(item.clone()).is_ok(), "funded cold frontier");
            }
            self.pending = Some(grown);
        }
        let stack = self.pending.as_mut().expect("frontier has capacity");
        stack.truncate(keep);
        assert!(stack.try_push((bit, left)).is_ok(), "funded cold frontier");
        self.previous = Some(next);
        Ok(())
    }

    /// Finish the rightmost pending branches. The result equals a tree built by
    /// applying the same rows in any insertion order.
    ///
    /// # Errors
    /// Original budget or physical allocator refuses a final branch.
    pub fn finish(mut self) -> Result<KeyedStateTreeV1, KeyedStateError> {
        if let Some(mut right) = self.previous {
            while let Some((bit, left)) = self.pending.as_mut().and_then(ChargedBuffer::pop) {
                right = Node::branch(bit, left, right, &self.tree.budget)?;
            }
            self.tree.top = Some(right);
        }
        Ok(self.tree)
    }
}
