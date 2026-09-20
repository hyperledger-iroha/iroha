use super::node::{Leaf, Node, Untracked};
use std::fmt::Debug;

#[derive(Debug)]
pub(crate) enum LeafInsertState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    Ok(Option<V>),
    // Split(K, V),
    Split(*mut Leaf<K, V, C>),
    // We split in the reverse direction.
    RevSplit(*mut Leaf<K, V, C>),
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
pub(crate) enum BranchInsertState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    Ok,
    // Two nodes that need addition to a new branch?
    Split(*mut Node<K, V, C>, *mut Node<K, V, C>),
}

#[derive(Debug)]
pub(crate) enum BranchShrinkState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    Balanced,
    Merge(*mut Node<K, V, C>),
    Shrink(*mut Node<K, V, C>),
}

/*
#[derive(Debug)]
pub(crate) enum BranchTrimState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    Complete,
    Promote(*mut Node<K, V, C>),
}

pub(crate) enum CRTrimState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    Complete,
    Clone(*mut Node<K, V, C>),
    Promote(*mut Node<K, V, C>),
}
*/

#[derive(Debug)]
pub(crate) enum CRInsertState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    // We did not need to clone, here is the result.
    NoClone(Option<V>),
    // We had to clone the referenced node provided.
    Clone(Option<V>, *mut Node<K, V, C>),
    // We had to split, but did not need a clone.
    // REMEMBER: In all split cases it means the key MUST NOT have
    // previously existed, so it implies return none to the
    // caller.
    Split(*mut Node<K, V, C>),
    RevSplit(*mut Node<K, V, C>),
    // We had to clone and split.
    CloneSplit(*mut Node<K, V, C>, *mut Node<K, V, C>),
    CloneRevSplit(*mut Node<K, V, C>, *mut Node<K, V, C>),
}

#[derive(Debug)]
pub(crate) enum CRCloneState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    Clone(*mut Node<K, V, C>),
    NoClone,
}

#[derive(Debug)]
pub(crate) enum CRRemoveState<K, V, C = Untracked>
where
    K: Ord + Clone + Debug,
    V: Clone,
{
    // We did not need to clone, here is the result.
    NoClone(Option<V>),
    // We had to clone the referenced node provided.
    Clone(Option<V>, *mut Node<K, V, C>),
    //
    Shrink(Option<V>),
    //
    CloneShrink(Option<V>, *mut Node<K, V, C>),
}
