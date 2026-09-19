//! Iterators for the map.

// Iterators for the bptree
use super::node::{Branch, Leaf, Meta, Node};
use std::collections::VecDeque;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

pub(crate) struct LeafIter<'a, K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    length: Option<usize>,
    // idx: usize,
    stack: VecDeque<(*mut Node<K, V>, usize)>,
    phantom_k: PhantomData<&'a K>,
    phantom_v: PhantomData<&'a V>,
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> LeafIter<'_, K, V> {
    pub(crate) fn new(root: *mut Node<K, V>, size_hint: bool) -> Self {
        let length = if size_hint {
            Some(unsafe { (*root).leaf_count() })
        } else {
            None
        };

        // We probably need to position the VecDeque here.
        let mut stack = VecDeque::new();

        let mut work_node = root;
        loop {
            stack.push_back((work_node, 0));
            if self_meta!(work_node).is_leaf() {
                break;
            } else {
                work_node = branch_ref!(work_node, K, V).get_idx_unchecked(0);
            }
        }

        LeafIter {
            length,
            // idx: 0,
            stack,
            phantom_k: PhantomData,
            phantom_v: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_base() -> Self {
        LeafIter {
            length: None,
            // idx: 0,
            stack: VecDeque::new(),
            phantom_k: PhantomData,
            phantom_v: PhantomData,
        }
    }

    pub(crate) fn stack_position(&mut self, idx: usize) {
        // Get the current branch, it must the the back.
        if let Some((bref, bpidx)) = self.stack.back() {
            let wbranch = branch_ref!(*bref, K, V);
            if let Some(node) = wbranch.get_idx_checked(idx) {
                // Insert as much as possible now. First insert
                // our current idx, then all the 0, idxs.
                let mut work_node = node;
                let mut work_idx = idx;
                loop {
                    self.stack.push_back((work_node, work_idx));
                    if self_meta!(work_node).is_leaf() {
                        break;
                    } else {
                        work_idx = 0;
                        work_node = branch_ref!(work_node, K, V).get_idx_unchecked(work_idx);
                    }
                }
            } else {
                // Unwind further.
                let bpidx = *bpidx + 1;
                let _ = self.stack.pop_back();
                self.stack_position(bpidx)
            }
        }
        // Must have been none, so we are exhausted. This means
        // the stack is empty, so return.
    }

    /*
    fn peek(&mut self) -> Option<&Leaf<K, V>> {
        // I have no idea how peekable works, yolo.
        self.stack.back().map(|t| t.0.as_leaf())
    }
    */
}

impl<'a, K: Clone + Hash + Eq + Debug, V: Clone> Iterator for LeafIter<'a, K, V> {
    type Item = &'a Leaf<K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        // base case is the vecdeque is empty
        let (leafref, parent_idx) = self.stack.pop_back()?;

        // Setup the veqdeque for the next iteration.
        self.stack_position(parent_idx + 1);

        // Return the leaf as we found at the start, regardless of the
        // stack operations.
        Some(leaf_ref!(leafref, K, V))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.length {
            Some(l) => (l, Some(l)),
            // We aren't (shouldn't) be estimating
            None => (0, None),
        }
    }
}

/// Iterator over references to Key Value pairs stored in the map.
pub struct Iter<'a, K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    length: usize,
    slot_idx: usize,
    bk_idx: usize,
    curleaf: Option<&'a Leaf<K, V>>,
    leafiter: LeafIter<'a, K, V>,
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> Iter<'_, K, V> {
    pub(crate) fn new(root: *mut Node<K, V>, length: usize) -> Self {
        let mut liter = LeafIter::new(root, false);
        let leaf = liter.next();
        // We probably need to position the VecDeque here.
        Iter {
            length,
            slot_idx: 0,
            bk_idx: 0,
            curleaf: leaf,
            leafiter: liter,
        }
    }
}

impl<'a, K: Clone + Hash + Eq + Debug, V: Clone> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(leaf) = self.curleaf {
            if let Some(r) = leaf.get_kv_idx_checked(self.slot_idx, self.bk_idx) {
                self.bk_idx += 1;
                Some(r)
            } else {
                // Are we partway in a bucket?
                if self.bk_idx > 0 {
                    // It's probably ended, next slot.
                    self.slot_idx += 1;
                    self.bk_idx = 0;
                    self.next()
                } else {
                    // We've exhasuted the slots sink bk_idx == 0 was empty.
                    self.curleaf = self.leafiter.next();
                    self.slot_idx = 0;
                    self.bk_idx = 0;
                    self.next()
                }
            }
        } else {
            None
        }
    }

    /// Provide a hint as to the number of items this iterator will yield.
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.length, Some(self.length))
    }
}

/// Iterator over references to Keys stored in the map.
pub struct KeyIter<'a, K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    iter: Iter<'a, K, V>,
}

impl<'a, K: Clone + Hash + Eq + Debug, V: Clone> KeyIter<'a, K, V> {
    pub(crate) fn new(root: *mut Node<K, V>, length: usize) -> Self {
        KeyIter {
            iter: Iter::new(root, length),
        }
    }
}

impl<'a, K: Clone + Hash + Eq + Debug, V: Clone> Iterator for KeyIter<'a, K, V> {
    type Item = &'a K;

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Iterator over references to Values stored in the map.
pub struct ValueIter<'a, K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    iter: Iter<'a, K, V>,
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> ValueIter<'_, K, V> {
    pub(crate) fn new(root: *mut Node<K, V>, length: usize) -> Self {
        ValueIter {
            iter: Iter::new(root, length),
        }
    }
}

impl<'a, K: Clone + Hash + Eq + Debug, V: Clone> Iterator for ValueIter<'a, K, V> {
    type Item = &'a V;

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::super::cursor::SuperBlock;
    use super::super::node::{Branch, Leaf, Node, H_CAPACITY};
    use super::{Iter, LeafIter};

    fn create_leaf_node_full(vbase: usize) -> *mut Node<usize, usize> {
        assert!(vbase.is_multiple_of(10));
        let node = Node::new_leaf(0);
        {
            let nmut = leaf_ref!(node, usize, usize);
            for idx in 0..H_CAPACITY {
                let v = vbase + idx;
                nmut.insert_or_update(v as u64, v, v);
            }
        }
        node as *mut _
    }

    #[test]
    fn test_hashmap2_iter_leafiter_1() {
        let test_iter: LeafIter<usize, usize> = LeafIter::new_base();
        assert!(test_iter.count() == 0);
    }

    #[test]
    fn test_hashmap2_iter_leafiter_2() {
        let lnode = create_leaf_node_full(10);
        let mut test_iter = LeafIter::new(lnode, true);

        assert!(test_iter.size_hint() == (1, Some(1)));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == 10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_hashmap2_iter_leafiter_3() {
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, true);

        assert!(test_iter.size_hint() == (2, Some(2)));
        let lref = test_iter.next().unwrap();
        let rref = test_iter.next().unwrap();
        assert!(lref.min() == 10);
        assert!(rref.min() == 20);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_hashmap2_iter_leafiter_4() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let mut test_iter: LeafIter<usize, usize> = LeafIter::new(root as *mut _, true);

        assert!(test_iter.size_hint() == (4, Some(4)));
        let l1ref = test_iter.next().unwrap();
        let r1ref = test_iter.next().unwrap();
        let l2ref = test_iter.next().unwrap();
        let r2ref = test_iter.next().unwrap();
        assert!(l1ref.min() == 10);
        assert!(r1ref.min() == 20);
        assert!(l2ref.min() == 30);
        assert!(r2ref.min() == 40);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_hashmap2_iter_leafiter_5() {
        let lnode = create_leaf_node_full(10);
        let mut test_iter = LeafIter::new(lnode, true);

        assert!(test_iter.size_hint() == (1, Some(1)));

        let lref = test_iter.next().unwrap();
        assert!(lref.min() == 10);
        assert!(test_iter.next().is_none());
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, lnode as *mut _);
    }

    #[test]
    fn test_hashmap2_iter_iter_1() {
        // Make a tree
        let lnode = create_leaf_node_full(10);
        let rnode = create_leaf_node_full(20);
        let root = Node::new_branch(0, lnode, rnode);
        let test_iter: Iter<usize, usize> = Iter::new(root as *mut _, H_CAPACITY * 2);

        assert!(test_iter.size_hint() == (H_CAPACITY * 2, Some(H_CAPACITY * 2)));
        assert!(test_iter.count() == H_CAPACITY * 2);
        // Iterate!
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }

    #[test]
    fn test_hashmap2_iter_iter_2() {
        let l1node = create_leaf_node_full(10);
        let r1node = create_leaf_node_full(20);
        let l2node = create_leaf_node_full(30);
        let r2node = create_leaf_node_full(40);
        let b1node = Node::new_branch(0, l1node, r1node);
        let b2node = Node::new_branch(0, l2node, r2node);
        let root: *mut Branch<usize, usize> =
            Node::new_branch(0, b1node as *mut _, b2node as *mut _);
        let test_iter: Iter<usize, usize> = Iter::new(root as *mut _, H_CAPACITY * 4);

        // println!("{:?}", test_iter.size_hint());

        assert!(test_iter.size_hint() == (H_CAPACITY * 4, Some(H_CAPACITY * 4)));
        assert!(test_iter.count() == H_CAPACITY * 4);
        // This drops everything.
        let _sb: SuperBlock<usize, usize> = SuperBlock::new_test(1, root as *mut _);
    }
}
