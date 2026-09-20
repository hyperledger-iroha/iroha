//! Mutable iterators over bptree.

use std::borrow::Borrow;
// use std::collections::VecDeque;
use std::fmt::Debug;
use std::ops::RangeBounds;

use super::cursor::{CursorReadOps, CursorWrite};
use super::iter::RangeIter;

/// Mutable Iterator over references to Key Value pairs stored, bounded by a range.
pub struct RangeMutIter<'n, K, V>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    cursor: &'n mut CursorWrite<K, V>,
    inner_range_iter: RangeIter<'n, K, V>,
}

impl<'n, K, V> RangeMutIter<'n, K, V>
where
    K: Clone + Ord + Debug,
    V: Clone,
{
    pub(crate) fn new<R, T>(cursor: &'n mut CursorWrite<K, V>, range: R) -> Self
    where
        T: Ord + ?Sized,
        K: Borrow<T>,
        R: RangeBounds<T>,
    {
        // For now I'm doing this in the "stupidest way possible".
        //
        // The reason is we could do this with a more advanced iterator that determines
        // clones as we go, but that's quite a bit more work. For now if we just use
        // an existing iterator and get_mut_ref as we go, we get the same effect.
        //
        // This relies on the fact that we take &mut over cursor, so the keys can't
        // change during this, so even if we iterator over the ro-snapshot, the keys
        // still sync to the rw version.

        let length = cursor.len();
        let root = cursor.get_root();

        let inner_range_iter = RangeIter::new(root, range, length);

        RangeMutIter {
            cursor,
            inner_range_iter,
        }
    }
}

impl<'n, K: Clone + Ord + Debug, V: Clone> Iterator for RangeMutIter<'n, K, V> {
    type Item = (&'n K, &'n mut V);

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        if let Some((k, _)) = self.inner_range_iter.next() {
            self.cursor.get_mut_ref(k).map(|v| {
                // Rust's lifetime constraints aren't working here, and this is
                // yielding 'n when we need '_ which is shorter. So we force strip
                // and apply the lifetime to constrain it to this iterator.
                let v = v as *mut V;
                let v = unsafe { &mut *v as &mut V };
                (k, v)
            })
        } else {
            None
        }
    }

    /// Provide a hint as to the number of items this iterator will yield.
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.cursor.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::super::cursor::SuperBlock;
    use super::super::node::{Leaf, Node, L_CAPACITY};
    use super::RangeMutIter;
    use std::ops::Bound;
    use std::ops::Bound::*;

    use crate::internals::lincowcell::LinCowCellCapable;

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
    fn test_bptree2_iter_mutrangeiter_1() {
        let node = create_leaf_node_full(10);

        let sb = SuperBlock::new_test(1, node);
        let mut wcurs = sb.create_writer();

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let range_mut_iter = RangeMutIter::new(&mut wcurs, bounds);
        for (k, v_mut) in range_mut_iter {
            assert_eq!(*k, *v_mut);
        }

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let range_mut_iter = RangeMutIter::new(&mut wcurs, bounds);
        for (_k, v_mut) in range_mut_iter {
            *v_mut += 1;
        }

        let bounds: (Bound<usize>, Bound<usize>) = (Unbounded, Unbounded);
        let range_mut_iter = RangeMutIter::new(&mut wcurs, bounds);
        for (k, v_mut) in range_mut_iter {
            assert_eq!(*k + 1, *v_mut);
        }
    }
}
