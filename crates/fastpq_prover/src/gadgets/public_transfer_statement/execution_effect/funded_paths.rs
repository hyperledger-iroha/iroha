//! Prepaid AVL intervals for exact first-free wrapping public path allocation.

use std::alloc::Layout;

use iroha_allocation::{AllocationBudget, AllocationRefusal, AllocationReservation, ChargedBuffer};

use super::super::{check_limit, checked_add, invariant};
use crate::{Error, Result};

const NONE: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Interval {
    start: u32,
    end: u32,
    left: usize,
    right: usize,
    height: u8,
}

/// One original-pool arena for at most `key_count` live occupied intervals.
///
/// Every lookup/update is bounded by AVL height. Freed slots are reused through
/// an inline chain, never by shifting sorted storage or allocating more backing.
/// The preparing caller performs at most `key_count` successful allocations.
pub(super) struct FundedPaths {
    nodes: ChargedBuffer<Interval>,
    root: usize,
    free: usize,
    live: usize,
    key_count: usize,
}

impl FundedPaths {
    /// Exact one-arena backing demand, excluding the original pool/control owner.
    pub(super) fn allocation_bytes(key_count: usize) -> Result<usize> {
        Layout::array::<Interval>(key_count)
            .map(|layout| layout.size())
            .map_err(|_| AllocationRefusal::DemandOverflow.into())
    }

    /// Consume only the original reservation's exact fixed backing layout.
    /// No new pool or secondary admission is permitted.
    pub(super) fn from_reservation(
        key_count: usize,
        budget: &AllocationBudget,
        reservation: &mut AllocationReservation,
    ) -> Result<Self> {
        if !reservation.belongs_to(budget) {
            return Err(Error::AllocationForeignPool);
        }
        Ok(Self {
            nodes: ChargedBuffer::from_reservation(key_count, reservation)?,
            root: NONE,
            free: NONE,
            live: 0,
            key_count,
        })
    }

    /// Allocate the identical first-free wrapping path and logical lookup charge.
    ///
    /// All probe/count failures precede mutation. The four possible charge sites
    /// match the original interval-map kernel, independent of AVL rotations and
    /// physical tree comparisons. The extra arena guard can reject only a caller
    /// exceeding the preflight key-count contract; it never weakens path/work caps.
    pub(super) fn allocate(
        &mut self,
        base: u32,
        steps: &mut usize,
        max_steps: usize,
    ) -> Result<u32> {
        let mut charge = || {
            *steps = checked_add(*steps, 1)?;
            check_limit("max_public_transfer_allocation_steps", *steps, max_steps)
        };
        charge()?;
        let mut candidate = base;
        if let Some((_, end)) = self.predecessor(base, true).filter(|(_, end)| *end >= base) {
            if let Some(next) = end.checked_add(1) {
                candidate = next;
            } else {
                charge()?;
                candidate = match self.first().filter(|(start, _)| *start == 0) {
                    Some((_, end)) => end
                        .checked_add(1)
                        .ok_or_else(|| invariant("public path space is full"))?,
                    None => 0,
                };
            }
        }
        if u64::from(candidate.wrapping_sub(base))
            >= u64::try_from(self.key_count).unwrap_or(u64::MAX)
        {
            return Err(invariant("public path exceeds native bounded probe window"));
        }
        charge()?;
        let left = self
            .predecessor(candidate, false)
            .filter(|(_, end)| end.checked_add(1) == Some(candidate));
        charge()?;
        let right = self
            .successor(candidate)
            .filter(|(start, _)| candidate.checked_add(1) == Some(*start));
        // A prefix of at most key_count allocations cannot exceed this bound.
        // Check before removing either neighbour so even contract misuse cannot
        // partially rewrite the intervals on an arena-capacity refusal.
        let next_live = checked_add(
            self.live - usize::from(left.is_some()) - usize::from(right.is_some()),
            1,
        )?;
        check_limit(
            "max_public_transfer_path_intervals",
            next_live,
            self.key_count,
        )?;
        let start = left.map_or(candidate, |(start, _)| start);
        let end = right.map_or(candidate, |(_, end)| end);
        if let Some((start, _)) = left {
            self.root = self.remove(self.root, start);
        }
        if let Some((start, _)) = right {
            self.root = self.remove(self.root, start);
        }
        self.root = self.insert(self.root, start, end);
        Ok(candidate)
    }

    fn height(&self, index: usize) -> u8 {
        if index == NONE {
            0
        } else {
            self.nodes.as_slice()[index].height
        }
    }

    fn refresh(&mut self, index: usize) {
        let node = self.nodes.as_slice()[index];
        let height = 1 + self.height(node.left).max(self.height(node.right));
        self.nodes.as_mut_slice()[index].height = height;
    }

    fn balance(&self, index: usize) -> i16 {
        let node = self.nodes.as_slice()[index];
        i16::from(self.height(node.left)) - i16::from(self.height(node.right))
    }

    fn rotate_left(&mut self, index: usize) -> usize {
        let root = self.nodes.as_slice()[index].right;
        let middle = self.nodes.as_slice()[root].left;
        self.nodes.as_mut_slice()[index].right = middle;
        self.refresh(index);
        self.nodes.as_mut_slice()[root].left = index;
        self.refresh(root);
        root
    }

    fn rotate_right(&mut self, index: usize) -> usize {
        let root = self.nodes.as_slice()[index].left;
        let middle = self.nodes.as_slice()[root].right;
        self.nodes.as_mut_slice()[index].left = middle;
        self.refresh(index);
        self.nodes.as_mut_slice()[root].right = index;
        self.refresh(root);
        root
    }

    fn rebalance(&mut self, index: usize) -> usize {
        self.refresh(index);
        if self.balance(index) > 1 {
            let left = self.nodes.as_slice()[index].left;
            if self.balance(left) < 0 {
                let replacement = self.rotate_left(left);
                self.nodes.as_mut_slice()[index].left = replacement;
            }
            self.rotate_right(index)
        } else if self.balance(index) < -1 {
            let right = self.nodes.as_slice()[index].right;
            if self.balance(right) > 0 {
                let replacement = self.rotate_right(right);
                self.nodes.as_mut_slice()[index].right = replacement;
            }
            self.rotate_left(index)
        } else {
            index
        }
    }

    fn acquire(&mut self, start: u32, end: u32) -> usize {
        let value = Interval {
            start,
            end,
            left: NONE,
            right: NONE,
            height: 1,
        };
        let index = if self.free == NONE {
            let index = self.nodes.as_slice().len();
            self.nodes.push_reserved(value);
            index
        } else {
            let index = self.free;
            self.free = self.nodes.as_slice()[index].left;
            self.nodes.as_mut_slice()[index] = value;
            index
        };
        self.live += 1;
        index
    }

    fn release(&mut self, index: usize) {
        self.nodes.as_mut_slice()[index] = Interval {
            start: 0,
            end: 0,
            left: self.free,
            right: NONE,
            height: 0,
        };
        self.free = index;
        self.live -= 1;
    }

    fn insert(&mut self, index: usize, start: u32, end: u32) -> usize {
        if index == NONE {
            return self.acquire(start, end);
        }
        let node = self.nodes.as_slice()[index];
        if start < node.start {
            let replacement = self.insert(node.left, start, end);
            self.nodes.as_mut_slice()[index].left = replacement;
        } else {
            assert_ne!(
                start, node.start,
                "coalescing removes the existing interval first"
            );
            let replacement = self.insert(node.right, start, end);
            self.nodes.as_mut_slice()[index].right = replacement;
        }
        self.rebalance(index)
    }

    fn minimum(&self, mut index: usize) -> usize {
        while self.nodes.as_slice()[index].left != NONE {
            index = self.nodes.as_slice()[index].left;
        }
        index
    }

    fn remove(&mut self, index: usize, start: u32) -> usize {
        assert_ne!(index, NONE, "an observed neighbour remains in the tree");
        let node = self.nodes.as_slice()[index];
        if start < node.start {
            let replacement = self.remove(node.left, start);
            self.nodes.as_mut_slice()[index].left = replacement;
        } else if start > node.start {
            let replacement = self.remove(node.right, start);
            self.nodes.as_mut_slice()[index].right = replacement;
        } else if node.left == NONE || node.right == NONE {
            let replacement = if node.left == NONE {
                node.right
            } else {
                node.left
            };
            self.release(index);
            return replacement;
        } else {
            let successor = self.nodes.as_slice()[self.minimum(node.right)];
            self.nodes.as_mut_slice()[index].start = successor.start;
            self.nodes.as_mut_slice()[index].end = successor.end;
            let replacement = self.remove(node.right, successor.start);
            self.nodes.as_mut_slice()[index].right = replacement;
        }
        self.rebalance(index)
    }

    fn first(&self) -> Option<(u32, u32)> {
        if self.root == NONE {
            return None;
        }
        let node = self.nodes.as_slice()[self.minimum(self.root)];
        Some((node.start, node.end))
    }

    fn predecessor(&self, path: u32, inclusive: bool) -> Option<(u32, u32)> {
        let mut index = self.root;
        let mut found = None;
        while index != NONE {
            let node = self.nodes.as_slice()[index];
            if node.start < path || (inclusive && node.start == path) {
                found = Some((node.start, node.end));
                index = node.right;
            } else {
                index = node.left;
            }
        }
        found
    }

    fn successor(&self, path: u32) -> Option<(u32, u32)> {
        let mut index = self.root;
        let mut found = None;
        while index != NONE {
            let node = self.nodes.as_slice()[index];
            if node.start >= path {
                found = Some((node.start, node.end));
                index = node.left;
            } else {
                index = node.right;
            }
        }
        found
    }
}

#[cfg(test)]
mod tests;
