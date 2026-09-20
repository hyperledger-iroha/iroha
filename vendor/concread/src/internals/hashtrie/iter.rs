//! Iterators for the hashtrie

use super::cursor::{Ptr, HT_CAPACITY, MAX_HEIGHT};
use std::collections::VecDeque;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

/// Iterator over references to Key Value pairs stored in the map.
pub struct Iter<'a, K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    length: usize,
    stack: VecDeque<(usize, Ptr)>,
    k: PhantomData<&'a K>,
    v: PhantomData<&'a V>,
}

impl<K: Clone + Hash + Eq + Debug, V: Clone> Iter<'_, K, V> {
    pub(crate) fn new(root: Ptr, length: usize) -> Self {
        let mut stack = VecDeque::with_capacity(MAX_HEIGHT as usize);
        stack.push_back((0, root));
        Iter {
            length,
            stack,
            k: PhantomData,
            v: PhantomData,
        }
    }
}

impl<'a, K: Clone + Hash + Eq + Debug, V: Clone> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    /// Yield the next key value reference, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        if self.stack.is_empty() {
            return None;
        }
        'outer: loop {
            if let Some((mut idx, tgt_ptr)) = self.stack.pop_back() {
                if tgt_ptr.is_bucket() {
                    // Look next at idx.
                    let v = &(tgt_ptr.as_bucket::<K, V>()[idx].v) as *const V;
                    let k = &(tgt_ptr.as_bucket::<K, V>()[idx].k) as *const K;
                    // Inc idx
                    idx += 1;
                    // push back if there is more to examine
                    if idx < tgt_ptr.as_bucket::<K, V>().len() {
                        // Still more to go
                        self.stack.push_back((idx, tgt_ptr));
                    }
                    return Some(unsafe { (&*k as &K, &*v as &V) });
                } else {
                    debug_assert!(tgt_ptr.is_branch());
                    let brch = tgt_ptr.as_branch::<K, V>();
                    while idx < HT_CAPACITY {
                        let interest_ptr = brch.nodes[idx];
                        idx += 1;
                        if !interest_ptr.is_null() {
                            // Push our current loc
                            // to the stack, and our ptr,
                            // as well as the new one.
                            self.stack.push_back((idx, tgt_ptr));
                            self.stack.push_back((0, interest_ptr));
                            continue 'outer;
                        }
                    }
                }
            } else {
                // stack is depleted!
                return None;
            }
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

impl<K: Clone + Hash + Eq + Debug, V: Clone> KeyIter<'_, K, V> {
    pub(crate) fn new(root: Ptr, length: usize) -> Self {
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
    pub(crate) fn new(root: Ptr, length: usize) -> Self {
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
