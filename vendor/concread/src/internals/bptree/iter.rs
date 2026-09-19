//! Iterators for the map.

// Iterators for the bptree
use super::node::{Branch, Leaf, Meta, Node};
use std::borrow::Borrow;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Bound, RangeBounds};

pub(crate) struct LeafIter<'a, K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    stack: VecDeque<(*mut Node<K, V>, usize)>,
    phantom_k: PhantomData<&'a K>,
    phantom_v: PhantomData<&'a V>,
}

impl<K, V> LeafIter<'_, K, V>
where
    K: Clone + Ord + Debug,
    V: Clone,
{
    pub(crate) fn new<T>(root: *mut Node<K, V>, bound: Bound<&T>) -> Self
    where
        T: Ord + ?Sized,
        K: Borrow<T>,
    {
        // We need to position the VecDeque here.
        let mut stack = VecDeque::new();

        let mut work_node = root;
        loop {
            if self_meta!(work_node).is_leaf() {
                stack.push_back((work_node, 0));
                break;
            } else {
                match bound {
                    Bound::Excluded(q) | Bound::Included(q) => {
                        let bref = branch_ref!(work_node, K, V);
                        let idx = bref.locate_node(q);
                        // This is the index we are currently chasing from
                        // within this node.
                        stack.push_back((work_node, idx));
                        work_node = bref.get_idx_unchecked(idx);
                    }
                    Bound::Unbounded => {
                        stack.push_back((work_node, 0));
                        work_node = branch_ref!(work_node, K, V).get_idx_unchecked(0);
                    }
                }
            }
        }

        // eprintln!("{:?}", stack);

        LeafIter {
            stack,
            phantom_k: PhantomData,
            phantom_v: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_base() -> Self {
        LeafIter {
            stack: VecDeque::new(),
            phantom_k: PhantomData,
            phantom_v: PhantomData,
        }
    }

    pub(crate) fn stack_position(&mut self) {
        debug_assert!(match self.stack.back() {
            Some((node, _)) => {
                self_meta!(*node).is_branch()
            }
            None => true,
        });

        'outer: loop {
            // Get the current branch, it must the the back.
            if let Some((bref, bpidx)) = self.stack.back_mut() {
                let wbranch = branch_ref!(*bref, K, V);
                // We were currently looking at bpidx in bref. Increment and
                // check what's next.
                *bpidx += 1;

                if let Some(node) = wbranch.get_idx_checked(*bpidx) {
                    // Got the new node, continue down.
                    let mut work_node = node;
                    loop {
                        self.stack.push_back((work_node, 0));
                        if self_meta!(work_node).is_leaf() {
                            break 'outer;
                        } else {
                            work_node = branch_ref!(work_node, K, V).get_idx_unchecked(0);
                        }
                    }
                } else {
                    let _ = self.stack.pop_back();
                    continue 'outer;
                }
            } else {
                // Must have been none, so we are exhausted. This means
                // the stack is empty, so return.
                break 'outer;
            }
        }
        // Done!
    }

    pub(crate) fn get_mut(&mut self) -> Option<&mut (*mut Node<K, V>, usize)> {
        self.stack.back_mut()
    }

    pub(crate) fn clear(&mut self) {
        self.stack.clear()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl<'a, K: Clone + Ord + Debug, V: Clone> Iterator for LeafIter<'a, K, V> {
    type Item = &'a Leaf<K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        // base case is the vecdeque is empty
        let (leafref, _) = self.stack.pop_back()?;

        // Setup the veqdeque for the next iteration.
        self.stack_position();

        // Return the leaf as we found at the start, regardless of the
        // stack operations.
        Some(leaf_ref_shared!(leafref, K, V))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

pub(crate) struct RevLeafIter<'a, K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    stack: VecDeque<(*mut Node<K, V>, usize)>,
    phantom_k: PhantomData<&'a K>,
    phantom_v: PhantomData<&'a V>,
}

impl<K, V> RevLeafIter<'_, K, V>
where
    K: Clone + Ord + Debug,
    V: Clone,
{
    pub(crate) fn new<T>(root: *mut Node<K, V>, bound: Bound<&T>) -> Self
    where
        T: Ord + ?Sized,
        K: Borrow<T>,
    {
        // We need to position the VecDeque here.
        let mut stack = VecDeque::new();

        let mut work_node = root;
        loop {
            if self_meta!(work_node).is_leaf() {
                // Put in the max len here ...
                let lref = leaf_ref!(work_node, K, V);
                if lref.count() > 0 {
                    stack.push_back((work_node, lref.count() - 1));
                }
                break;
            } else {
                let bref = branch_ref_shared!(work_node, K, V);
                let bref_count = bref.count();
                match bound {
                    Bound::Excluded(q) | Bound::Included(q) => {
                        let idx = bref.locate_node(q);
                        // This is the index we are currently chasing from
                        // within this node.
                        stack.push_back((work_node, idx));
                        work_node = bref.get_idx_unchecked(idx);
                    }
                    Bound::Unbounded => {
                        // count shows the most right node.
                        stack.push_back((work_node, bref_count));
                        work_node = branch_ref!(work_node, K, V).get_idx_unchecked(bref_count);
                    }
                }
            }
        }

        // eprintln!("{:?}", stack);

        RevLeafIter {
            stack,
            phantom_k: PhantomData,
            phantom_v: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_base() -> Self {
        RevLeafIter {
            stack: VecDeque::new(),
            phantom_k: PhantomData,
            phantom_v: PhantomData,
        }
    }

    pub(crate) fn stack_position(&mut self) {
        debug_assert!(match self.stack.back() {
            Some((node, _)) => {
                self_meta!(*node).is_branch()
            }
            None => true,
        });

        'outer: loop {
            // Get the current branch, it must the the back.
            if let Some((bref, bpidx)) = self.stack.back_mut() {
                let wbranch = branch_ref!(*bref, K, V);
                // We were currently looking at bpidx in bref. Increment and
                // check what's next.
                // NOTE: If this underflows, it's okay because idx_checked won't
                // return the Some case!
                let (nidx, oflow) = (*bpidx).overflowing_sub(1);

                if oflow {
                    let _ = self.stack.pop_back();
                    continue 'outer;
                }

                *bpidx = nidx;

                if let Some(node) = wbranch.get_idx_checked(*bpidx) {
                    // Got the new node, continue down.
                    let mut work_node = node;
                    loop {
                        if self_meta!(work_node).is_leaf() {
                            let lref = leaf_ref!(work_node, K, V);
                            self.stack.push_back((work_node, lref.count() - 1));
                            break 'outer;
                        } else {
                            let bref = branch_ref!(work_node, K, V);
                            let idx = bref.count();
                            self.stack.push_back((work_node, idx));
                            work_node = bref.get_idx_unchecked(idx);
                        }
                    }
                } else {
                    let _ = self.stack.pop_back();
                    continue 'outer;
                }
            } else {
                // Must have been none, so we are exhausted. This means
                // the stack is empty, so return.
                break 'outer;
            }
        }
    }

    pub(crate) fn get_mut(&mut self) -> Option<&mut (*mut Node<K, V>, usize)> {
        self.stack.back_mut()
    }

    pub(crate) fn clear(&mut self) {
        self.stack.clear()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl<'a, K: Clone + Ord + Debug, V: Clone> Iterator for RevLeafIter<'a, K, V> {
    type Item = &'a Leaf<K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        // base case is the vecdeque is empty
        let (leafref, _) = self.stack.pop_back()?;

        // Setup the veqdeque for the next iteration.
        self.stack_position();

        // Return the leaf as we found at the start, regardless of the
        // stack operations.
        Some(leaf_ref_shared!(leafref, K, V))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

// Wrappers

/// Iterator over references to Key Value pairs stored in the map.
pub struct Iter<'n, K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    iter: RangeIter<'n, K, V>,
}

impl<K: Clone + Ord + Debug, V: Clone> Iter<'_, K, V> {
    pub(crate) fn new(root: *mut Node<K, V>, length: usize) -> Self {
        let bounds: (Bound<K>, Bound<K>) = (Bound::Unbounded, Bound::Unbounded);
        let iter = RangeIter::new(root, bounds, length);
        Iter { iter }
    }
}

impl<'n, K: Clone + Ord + Debug, V: Clone> Iterator for Iter<'n, K, V> {
    type Item = (&'n K, &'n V);

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    /// Provide a hint as to the number of items this iterator will yield.
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.iter.size_hint() {
            // Transpose the x through as a lower bound.
            (_, Some(x)) => (x, Some(x)),
            (_, None) => (0, None),
        }
    }
}

impl<K: Clone + Ord + Debug, V: Clone> DoubleEndedIterator for Iter<'_, K, V> {
    /// Yield the next key value reference, or `None` if exhausted.
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

/// Iterator over references to Keys stored in the map.
pub struct KeyIter<'n, K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    iter: Iter<'n, K, V>,
}

impl<K: Clone + Ord + Debug, V: Clone> KeyIter<'_, K, V> {
    pub(crate) fn new(root: *mut Node<K, V>, length: usize) -> Self {
        KeyIter {
            iter: Iter::new(root, length),
        }
    }
}

impl<'n, K: Clone + Ord + Debug, V: Clone> Iterator for KeyIter<'n, K, V> {
    type Item = &'n K;

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<K: Clone + Ord + Debug, V: Clone> DoubleEndedIterator for KeyIter<'_, K, V> {
    /// Yield the next key value reference, or `None` if exhausted.
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(k, _)| k)
    }
}

/// Iterator over references to Values stored in the map.
pub struct ValueIter<'n, K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    iter: Iter<'n, K, V>,
}

impl<K: Clone + Ord + Debug, V: Clone> ValueIter<'_, K, V> {
    pub(crate) fn new(root: *mut Node<K, V>, length: usize) -> Self {
        ValueIter {
            iter: Iter::new(root, length),
        }
    }
}

impl<'n, K: Clone + Ord + Debug, V: Clone> Iterator for ValueIter<'n, K, V> {
    type Item = &'n V;

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<K: Clone + Ord + Debug, V: Clone> DoubleEndedIterator for ValueIter<'_, K, V> {
    /// Yield the next key value reference, or `None` if exhausted.
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back().map(|(_, v)| v)
    }
}

/// Iterator over references to Key Value pairs stored, bounded by a range.
pub struct RangeIter<'n, K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    length: Option<usize>,
    left_iter: LeafIter<'n, K, V>,
    right_iter: RevLeafIter<'n, K, V>,
    phantom_root: PhantomData<&'n ()>,
}

impl<K, V> RangeIter<'_, K, V>
where
    K: Clone + Ord + Debug,
    V: Clone,
{
    pub(crate) fn new<R, T>(root: *mut Node<K, V>, range: R, length: usize) -> Self
    where
        T: Ord + ?Sized,
        K: Borrow<T>,
        R: RangeBounds<T>,
    {
        let length = Some(length);
        // We need to position the VecDeque here. This requires us
        // to know the bounds that we have. We do this similar to the main
        // rust library tree by locating our "edges", and maintaining stacks to their paths.

        // Setup and position the two iters.
        let mut left_iter = LeafIter::new(root, range.start_bound());
        let mut right_iter = RevLeafIter::new(root, range.end_bound());

        //If needed, advanced the left / right iter depending on the situation.

        match range.start_bound() {
            Bound::Unbounded => {
                // Do nothing!
            }
            Bound::Included(k) => {
                if let Some((node, idx)) = left_iter.get_mut() {
                    let leaf = leaf_ref!(*node, K, V);
                    // eprintln!("Positioning Included with ... {:?} {:?}", leaf, idx);
                    match leaf.locate(k) {
                        Ok(fidx) | Err(fidx) => {
                            // eprintln!("Using, {}", fidx);
                            *idx = fidx;
                            // Done!
                        }
                    }
                } else {
                    // Do nothing, it's empty.
                }
            }
            Bound::Excluded(k) => {
                if let Some((node, idx)) = left_iter.get_mut() {
                    let leaf = leaf_ref!(*node, K, V);
                    // eprintln!("Positioning Excluded with ... {:?} {:?}", leaf, idx);
                    match leaf.locate(k) {
                        Ok(fidx) => {
                            // eprintln!("Excluding Using, {}", fidx);
                            *idx = fidx + 1;
                            if *idx >= leaf.count() {
                                if let Some((rnode, _)) = right_iter.get_mut() {
                                    // If the leaf iterators were in the same node before advancing left iterator
                                    // means that left iterator would be ahead of right iter so no elements left
                                    if rnode == node {
                                        left_iter.clear();
                                        right_iter.clear();
                                    }
                                }
                                // Okay, this means we overflowed to the next leaf, so just
                                // advanced the leaf iter to the start of the next
                                left_iter.next();
                            }
                            // Done
                        }
                        Err(fidx) => {
                            // eprintln!("Using, {}", fidx);
                            *idx = fidx;
                            // Done!
                        }
                    }
                } else {
                    // Do nothing, the leaf iter is empty.
                }
            }
        }

        match range.end_bound() {
            Bound::Unbounded => {
                // Do nothing!
            }
            Bound::Included(k) => {
                if let Some((node, idx)) = right_iter.get_mut() {
                    let leaf = leaf_ref!(*node, K, V);
                    // eprintln!("Positioning Included with ... {:?} {:?}", leaf, idx);
                    match leaf.locate(k) {
                        Ok(fidx) => {
                            *idx = fidx;
                        }
                        Err(fidx) => {
                            // eprintln!("Using, {}", fidx);
                            let (nidx, oflow) = fidx.overflowing_sub(1);
                            if oflow {
                                if let Some((lnode, _)) = left_iter.get_mut() {
                                    // If the leaf iterators were in the same node before advancing right iterator
                                    // means that left iterator would be ahead of right iter so no elements left
                                    if lnode == node {
                                        left_iter.clear();
                                        right_iter.clear();
                                    }
                                }
                                right_iter.next();
                            } else {
                                *idx = nidx;
                            }
                            // Done!
                        }
                    }
                } else {
                    // Do nothing, it's empty.
                }
            }
            Bound::Excluded(k) => {
                if let Some((node, idx)) = right_iter.get_mut() {
                    let leaf = leaf_ref!(*node, K, V);
                    // eprintln!("Positioning Included with ... {:?} {:?}", leaf, idx);
                    match leaf.locate(k) {
                        Ok(fidx) | Err(fidx) => {
                            // eprintln!("Using, {}", fidx);
                            let (nidx, oflow) = fidx.overflowing_sub(1);
                            if oflow {
                                if let Some((lnode, _)) = left_iter.get_mut() {
                                    // If the leaf iterators were in the same node before advancing right iterator
                                    // means that left iterator would be ahead of right iter so no elements left
                                    if lnode == node {
                                        left_iter.clear();
                                        right_iter.clear();
                                    }
                                }
                                right_iter.next();
                            } else {
                                *idx = nidx;
                            }
                            // Done!
                        }
                    }
                } else {
                    // Do nothing, it's empty.
                }
            }
        }

        // If either side is empty, it indicates a bound hit the end of the tree
        // and we can't proceed
        if left_iter.is_empty() || right_iter.is_empty() {
            left_iter.clear();
            right_iter.clear();
        }

        // If both iterators end up in the same leaf and left index is larger,
        // it indicates that there is nothing to return
        if let Some((lnode, lidx)) = left_iter.get_mut() {
            if let Some((rnode, ridx)) = right_iter.get_mut() {
                if rnode == lnode && lidx > ridx {
                    right_iter.clear();
                    left_iter.clear();
                }
            }
        }

        RangeIter {
            length,
            left_iter,
            right_iter,
            phantom_root: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_base() -> Self {
        RangeIter {
            length: None,
            left_iter: LeafIter::new_base(),
            right_iter: RevLeafIter::new_base(),
            phantom_root: PhantomData,
        }
    }
}

impl<'n, K: Clone + Ord + Debug, V: Clone> Iterator for RangeIter<'n, K, V> {
    type Item = (&'n K, &'n V);

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((node, idx)) = self.left_iter.get_mut() {
                // eprintln!("Next with ... {:?} {:?}", node, idx);
                let leaf = leaf_ref!(*node, K, V);
                // Get idx checked.
                if let Some(r) = leaf.get_kv_idx_checked(*idx) {
                    if let Some((rnode, ridx)) = self.right_iter.get_mut() {
                        if rnode == node && idx == ridx {
                            // eprintln!("Clearing lists, end condition reached");
                            // Was the node + index the same as right?
                            // It means we just exhausted the list.
                            self.right_iter.clear();
                            self.left_iter.clear();
                            return Some(r);
                        }
                    }

                    let nidx = *idx + 1;

                    if nidx >= leaf.count() {
                        self.left_iter.next();
                    } else {
                        *idx = nidx;
                    }
                    return Some(r);
                } else {
                    // Go to the next leaf.
                    self.left_iter.next();
                    continue;
                }
            } else {
                break None;
            }
        }
    }

    /// Provide a hint as to the number of items this iterator will yield.
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.length)
    }
}

impl<K: Clone + Ord + Debug, V: Clone> DoubleEndedIterator for RangeIter<'_, K, V> {
    /// Yield the next key value reference, or `None` if exhausted.
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((node, idx)) = self.right_iter.get_mut() {
                let leaf = leaf_ref_shared!(*node, K, V);
                // Get idx checked.
                if let Some(r) = leaf.get_kv_idx_checked(*idx) {
                    if let Some((lnode, lidx)) = self.left_iter.get_mut() {
                        if lnode == node && idx == lidx {
                            // eprintln!("Clearing lists, end condition reached");
                            // Was the node + index the same as right?
                            // It means we just exhausted the list.
                            self.right_iter.clear();
                            self.left_iter.clear();
                            return Some(r);
                        }
                    }

                    let (nidx, oflow) = (*idx).overflowing_sub(1);

                    if oflow {
                        self.right_iter.next();
                    } else {
                        *idx = nidx;
                    }
                    return Some(r);
                } else {
                    // Go to the next leaf.
                    self.right_iter.next();
                    continue;
                }
            } else {
                break None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::cursor::SuperBlock;
    use super::super::node::{Branch, Leaf, Node, L_CAPACITY, L_CAPACITY_N1};
    use super::{Iter, LeafIter, RangeIter, RevLeafIter};
    use std::ops::Bound;
    use std::ops::Bound::*;

    fn create_leaf_node_full(vbase: usize) -> *mut Node<usize, usize> {
        assert!(vbase.is_multiple_of(10));
        let node = Node::new_leaf(0);
        {
            let nmut = leaf_ref!(node, usize, usize);
            for idx in 0..L_CAPACITY {
                let v = vbase + idx;
                nmut.insert_or_update(v, v);
            }
        }
        node as *mut _
    }

    #[test]
    fn test_bptree2_iter_leafiter_1() {
        let test_iter: LeafIter<usize, usize> = LeafIter::new_base();
        assert!(test_iter.count() == 0);
    }

    #[test]
    fn test_bptree2_iter_leafiter_2() {
        let lnode = create_leaf_node_full(10);
        let mut test_iter = LeafIter::new(lnode, Unbounded);

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_3() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Unbounded);

        let lref = test_iter.next().unwrap();
        let rref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(rref.min() == &20);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_4() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Unbounded);

        let l1ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(l1ref.min() == &10);
        assert!(r1ref.min() == &20);
        assert!(l2ref.min() == &30);
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_5() {
        let lnode = create_leaf_node_full(10);
        let mut test_iter = LeafIter::new(lnode, Unbounded);

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_1() {
        // Test a lower bound that is *minimum*.
        let lnode = create_leaf_node_full(10);
        let mut test_iter = LeafIter::new(lnode, Included(&0));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_2() {
        // Test a lower bound that is *within*.
        let lnode = create_leaf_node_full(10);
        let mut test_iter = LeafIter::new(lnode, Included(&10));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_3() {
        // Test a lower bound that is *greater*.
        let lnode = create_leaf_node_full(10);
        let mut test_iter = LeafIter::new(lnode, Included(&100));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_4() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&0));
        // Cursor should be positioned on the node with "10"

        let lref = test_iter.next().unwrap();
        let rref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(rref.min() == &20);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_5() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&10));
        // Cursor should be positioned on the node with "10"

        let lref = test_iter.next().unwrap();
        let rref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(rref.min() == &20);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_6() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&19));
        // Cursor should be positioned on the node with "10"

        let lref = test_iter.next().unwrap();
        let rref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(rref.min() == &20);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_7() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        eprintln!("{:?}, {:?}", lnode, rnode);

        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&20));
        // Cursor should be positioned on the node with "20"

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &20);
        let x = test_iter.next();
        eprintln!("{:?}", x);
        assert!(x.is_none());
        // assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_8() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&100));
        // Cursor should be positioned on the node with "20"

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &20);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_9() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&0));
        // Should be on the 10

        let l1ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(l1ref.min() == &10);
        assert!(r1ref.min() == &20);
        assert!(l2ref.min() == &30);
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_10() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&15));
        // Should be on the 10

        let l1ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(l1ref.min() == &10);
        assert!(r1ref.min() == &20);
        assert!(l2ref.min() == &30);
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_11() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&20));
        // Should be on the 20

        let r1ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(r1ref.min() == &20);
        assert!(l2ref.min() == &30);
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_12() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&25));
        // Should be on the 20

        let r1ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(r1ref.min() == &20);
        assert!(l2ref.min() == &30);
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_13() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&30));
        // Should be on the 30

        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(l2ref.min() == &30);
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_14() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&35));
        // Should be on the 30

        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(l2ref.min() == &30);
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_15() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&40));
        // Should be on the 40

        let r2ref = test_iter.next().unwrap();
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_leafiter_bound_16() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, Included(&100));
        // Should be on the 40

        let r2ref = test_iter.next().unwrap();
        assert!(r2ref.min() == &40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    // == Reverse Leaf Iter ==
    #[test]
    fn test_bptree2_iter_revleafiter_1() {
        let test_iter: RevLeafIter<usize, usize> = RevLeafIter::new_base();
        assert!(test_iter.count() == 0);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_2() {
        let lnode = create_leaf_node_full(10);
        let mut test_iter = RevLeafIter::new(lnode, Unbounded);

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_3() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: RevLeafIter<usize, usize> = RevLeafIter::new(root as *mut _, Unbounded);

        let rref = test_iter.next().unwrap();
        let lref = test_iter.next().unwrap();
        assert!(rref.min() == &20);
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_4() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> = RevLeafIter::new(root as *mut _, Unbounded);

        let r2ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l1ref = test_iter.next().unwrap();
        assert!(r2ref.min() == &40);
        assert!(l2ref.min() == &30);
        assert!(r1ref.min() == &20);
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_1() {
        // Test an upper bound that is *maximum*.
        let lnode = create_leaf_node_full(10);
        let mut test_iter = RevLeafIter::new(lnode, Included(&100));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_2() {
        // Test a lower bound that is *within*.
        let lnode = create_leaf_node_full(10);
        let mut test_iter = RevLeafIter::new(lnode, Included(&10));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_3() {
        // Test a lower bound that is *minimum*.
        let lnode = create_leaf_node_full(10);
        let mut test_iter = RevLeafIter::new(lnode, Included(&0));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_5() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&10));
        // Cursor should be positioned on the node with "10"

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_6() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&19));
        // Cursor should be positioned on the node with "10"

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_7() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        eprintln!("{:?}, {:?}", lnode, rnode);

        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&20));
        // Cursor should be positioned on the node with "20"

        let rref = test_iter.next().unwrap();
        let lref = test_iter.next().unwrap();
        assert!(rref.min() == &20);
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_8() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&100));
        // Cursor should be positioned on the node with "20"

        let rref = test_iter.next().unwrap();
        let lref = test_iter.next().unwrap();
        assert!(rref.min() == &20);
        assert!(lref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_9() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&100));
        // Should be on the 40

        let r2ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l1ref = test_iter.next().unwrap();
        assert!(r2ref.min() == &40);
        assert!(l2ref.min() == &30);
        assert!(r1ref.min() == &20);
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_10() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&45));
        // Should be on the 40

        let r2ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l1ref = test_iter.next().unwrap();
        assert!(r2ref.min() == &40);
        assert!(l2ref.min() == &30);
        assert!(r1ref.min() == &20);
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_11() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&35));
        // Should be on the 30

        let l2ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l1ref = test_iter.next().unwrap();
        assert!(l2ref.min() == &30);
        assert!(r1ref.min() == &20);
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_12() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&30));
        // Should be on the 30

        let l2ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l1ref = test_iter.next().unwrap();
        assert!(l2ref.min() == &30);
        assert!(r1ref.min() == &20);
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_13() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&25));
        // Should be on the 20

        let r1ref = test_iter.next().unwrap();
        let l1ref = test_iter.next().unwrap();
        assert!(r1ref.min() == &20);
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_14() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&20));
        // Should be on the 20

        let r1ref = test_iter.next().unwrap();
        let l1ref = test_iter.next().unwrap();
        assert!(r1ref.min() == &20);
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_15() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&15));
        // Should be on the 10

        let l1ref = test_iter.next().unwrap();
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_revleafiter_bound_16() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: RevLeafIter<usize, usize> =
            RevLeafIter::new(root as *mut _, Included(&0));
        // Should be on the 10

        let l1ref = test_iter.next().unwrap();
        assert!(l1ref.min() == &10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_iter_1() {
        // Make a tree
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let test_iter: Iter<usize, usize> = Iter::new(root as *mut _, L_CAPACITY * 2);

        assert!(test_iter.size_hint() == (L_CAPACITY * 2, Some(L_CAPACITY * 2)));
        assert!(test_iter.count() == L_CAPACITY * 2);
        // Iterate!
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_iter_2() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let test_iter: Iter<usize, usize> = Iter::new(root as *mut _, L_CAPACITY * 4);

        // println!("{:?}", test_iter.size_hint());

        assert!(test_iter.size_hint() == (L_CAPACITY * 4, Some(L_CAPACITY * 4)));
        assert!(test_iter.count() == L_CAPACITY * 4);
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_rangeiter_1() {
        let test_iter: RangeIter<usize, usize> = RangeIter::new_base();
        assert!(test_iter.count() == 0);
    }

    #[test]
    fn test_bptree2_iter_rangeiter_2() {
        let lnode = create_leaf_node_full(10);

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let test_iter = RangeIter::new(lnode, bounds, L_CAPACITY);
        assert!(test_iter.count() == L_CAPACITY);

        for i in 0..L_CAPACITY {
            let l_bound = 10 + i;
            let bounds: (Bound<usize>, Bound<usize>) = (Included(l_bound), Unbounded);
            let test_iter = RangeIter::new(lnode, bounds, L_CAPACITY);
            let i_count = test_iter.count();
            let x_count = L_CAPACITY - i;
            eprintln!("ex {} == {}", i_count, x_count);
            assert!(i_count == x_count);
        }

        for i in 0..L_CAPACITY {
            let l_bound = 10 + i;
            let bounds: (Bound<usize>, Bound<usize>) = (Excluded(l_bound), Unbounded);
            let test_iter = RangeIter::new(lnode, bounds, L_CAPACITY);
            let i_count = test_iter.count();
            let x_count = L_CAPACITY_N1 - i;
            eprintln!("ex {} == {}", i_count, x_count);
            assert!(i_count == x_count);
        }

        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_bptree2_iter_rangeiter_3() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let test_iter: RangeIter<usize, usize> =
            RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
        assert!(test_iter.count() == (L_CAPACITY * 4));

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let l_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Included(l_bound), Unbounded);
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.count();
                let x_count = ((5 - j) * L_CAPACITY) - i;
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let l_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Excluded(l_bound), Unbounded);
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.count();
                let x_count = ((5 - j) * L_CAPACITY) - (i + 1);
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_rangeiter_4() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let test_iter: RangeIter<usize, usize> =
            RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);

        assert!(test_iter.count() == (L_CAPACITY * 4));

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let r_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Included(r_bound));
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.count();
                let x_count = ((L_CAPACITY * 4) - (((4 - j) * L_CAPACITY) + (L_CAPACITY - i))) + 1;
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let r_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Excluded(r_bound));
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.count();
                let x_count = (L_CAPACITY * 4) - (((4 - j) * L_CAPACITY) + (L_CAPACITY - i));
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_rangeiter_5() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let test_iter: RangeIter<usize, usize> =
            RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
        assert!(test_iter.rev().count() == (L_CAPACITY * 4));

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let l_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Included(l_bound), Unbounded);
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.rev().count();
                let x_count = ((5 - j) * L_CAPACITY) - i;
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let l_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Excluded(l_bound), Unbounded);
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.rev().count();
                let x_count = ((5 - j) * L_CAPACITY) - (i + 1);
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_bptree2_iter_rangeiter_6() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let test_iter: RangeIter<usize, usize> =
            RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);

        assert!(test_iter.rev().count() == (L_CAPACITY * 4));

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let r_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Included(r_bound));
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.rev().count();
                let x_count = ((L_CAPACITY * 4) - (((4 - j) * L_CAPACITY) + (L_CAPACITY - i))) + 1;
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        for j in 1..5 {
            for i in 0..L_CAPACITY {
                let r_bound = (j * 10) + i;
                let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Excluded(r_bound));
                let test_iter: RangeIter<usize, usize> =
                    RangeIter::new(root as *mut _, bounds, L_CAPACITY * 4);
                let i_count = test_iter.rev().count();
                let x_count = (L_CAPACITY * 4) - (((4 - j) * L_CAPACITY) + (L_CAPACITY - i));
                eprintln!("ex {} == {}", i_count, x_count);
                assert!(i_count == x_count);
            }
        }

        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }
}
