use super::node::{Leaf, Node};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug)]
pub(crate) enum LeafInsertState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    Ok(Option<V>),
    // Split(K, V),
    Split(*mut Leaf<K, V>),
    // We split in the reverse direction.
    RevSplit(*mut Leaf<K, V>),
}

#[derive(Debug)]
pub(crate) enum LeafRemoveState<V>
where
    V: Clone,
{
    Ok(Option<V>),
    // Indicate that we found the associated value, but this
    // removal means we no longer exist so should be removed.
    Shrink(Option<V>),
}

#[derive(Debug)]
pub(crate) enum BranchInsertState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    Ok,
    // Two nodes that need addition to a new branch?
    Split(*mut Node<K, V>, *mut Node<K, V>),
}

#[derive(Debug)]
pub(crate) enum BranchShrinkState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    Balanced,
    Merge(*mut Node<K, V>),
    Shrink(*mut Node<K, V>),
}

/*
#[derive(Debug)]
pub(crate) enum BranchTrimState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    Complete,
    Promote(*mut Node<K, V>),
}
*/

/*
pub(crate) enum CRTrimState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    Complete,
    Clone(*mut Node<K, V>),
    Promote(*mut Node<K, V>),
}
*/

#[derive(Debug)]
pub(crate) enum CRInsertState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    // We did not need to clone, here is the result.
    NoClone(Option<V>),
    // We had to clone the referenced node provided.
    Clone(Option<V>, *mut Node<K, V>),
    // We had to split, but did not need a clone.
    // REMEMBER: In all split cases it means the key MUST NOT have
    // previously existed, so it implies return none to the
    // caller.
    Split(*mut Node<K, V>),
    RevSplit(*mut Node<K, V>),
    // We had to clone and split.
    CloneSplit(*mut Node<K, V>, *mut Node<K, V>),
    CloneRevSplit(*mut Node<K, V>, *mut Node<K, V>),
}

#[derive(Debug)]
pub(crate) enum CRCloneState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    Clone(*mut Node<K, V>),
    NoClone,
}

#[derive(Debug)]
pub(crate) enum CRRemoveState<K, V>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    // We did not need to clone, here is the result.
    NoClone(Option<V>),
    // We had to clone the referenced node provided.
    Clone(Option<V>, *mut Node<K, V>),
    //
    Shrink(Option<V>),
    //
    CloneShrink(Option<V>, *mut Node<K, V>),
}
