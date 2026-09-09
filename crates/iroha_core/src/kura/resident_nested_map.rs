//! Incremental nested association counts under an existing resident owner lock.

use super::{
    resident_inventory::{AssociationCount, ResidentOwner},
    resource_inventory::{Family, Unavailable},
};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, btree_map::Entry},
    ops::{Deref, DerefMut},
};

/// A value's exact fixed-number-of-lengths weight, including its outer record.
pub(super) trait AssociationValue {
    /// Resource family of this index.
    const FAMILY: Family;
    /// Count the record and its actual nested associations without scanning it.
    fn association_weight(&self) -> Result<u64, Unavailable>;
}

/// A map that never exposes untracked mutable access to nested values.
#[derive(Clone, Debug)]
pub(super) struct NestedMap<K, V> {
    inner: BTreeMap<K, V>,
    count: AssociationCount,
}
impl<K, V> Default for NestedMap<K, V> {
    fn default() -> Self {
        Self {
            inner: BTreeMap::new(),
            count: AssociationCount::default(),
        }
    }
}
impl<K: PartialEq, V: PartialEq> PartialEq for NestedMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}
impl<K: Eq, V: Eq> Eq for NestedMap<K, V> {}
impl<K, V> Deref for NestedMap<K, V> {
    type Target = BTreeMap<K, V>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<K: Ord, V: AssociationValue> NestedMap<K, V> {
    /// Insert or replace one value and count only its exact changed memberships.
    pub(super) fn insert(&mut self, key: K, value: V) -> Option<V> {
        let mut candidate = self.count;
        self.count.replace(None, None);
        let after = value.association_weight().ok();
        let old = self.inner.insert(key, value);
        let before = old
            .as_ref()
            .map_or(Some(0), |v| v.association_weight().ok());
        candidate.replace(before, after);
        self.count = candidate;
        old
    }
    /// Remove one actual record, including every nested association it owned.
    pub(super) fn remove<Q: ?Sized + Ord>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        let mut candidate = self.count;
        self.count.replace(None, None);
        let old = self.inner.remove(key);
        if let Some(value) = &old {
            candidate.removed(value.association_weight().ok());
        }
        self.count = candidate;
        old
    }
    /// Clear a fixture map while retaining any earlier arithmetic failure.
    #[cfg(test)]
    pub(super) fn clear(&mut self) {
        let mut candidate = self.count;
        let before = candidate.get().ok();
        self.count.replace(None, None);
        self.inner.clear();
        candidate.removed(before);
        self.count = candidate;
    }
    /// Track both callback changes to retained values and actual removals.
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        let mut candidate = self.count;
        // A caught callback unwind cannot leave an old count looking complete.
        self.count.replace(None, None);
        self.inner.retain(|key, value| {
            let before = value.association_weight().ok();
            let retained = keep(key, value);
            let after = if retained {
                value.association_weight().ok()
            } else {
                Some(0)
            };
            candidate.replace(before, after);
            retained
        });
        self.count = candidate;
    }
    /// Mutate an existing nested value through a checked per-value guard.
    pub(super) fn get_mut<Q: ?Sized + Ord>(&mut self, key: &Q) -> Option<ValueGuard<'_, V>>
    where
        K: Borrow<Q>,
    {
        let previous = self.count;
        self.count.replace(None, None);
        let Some(value) = self.inner.get_mut(key) else {
            self.count = previous;
            return None;
        };
        let before = value.association_weight().ok();
        Some(ValueGuard {
            value,
            count: &mut self.count,
            previous,
            before,
        })
    }
    /// Return the existing entry operation used by replica admission.
    pub(super) fn entry(&mut self, key: K) -> NestedEntry<'_, K, V> {
        let previous = self.count;
        self.count.replace(None, None);
        NestedEntry {
            entry: self.inner.entry(key),
            count: &mut self.count,
            previous,
        }
    }
    /// Extend through the same duplicate-aware owner as individual insertion.
    pub(super) fn extend(&mut self, values: impl IntoIterator<Item = (K, V)>) {
        for (key, value) in values {
            self.insert(key, value);
        }
    }
}
impl<K: Ord, V: AssociationValue> From<BTreeMap<K, V>> for NestedMap<K, V> {
    fn from(inner: BTreeMap<K, V>) -> Self {
        let mut result = Self::default();
        result.extend(inner);
        result
    }
}
impl<'a, K, V> IntoIterator for &'a NestedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = std::collections::btree_map::Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}
impl<K, V: AssociationValue> ResidentOwner for NestedMap<K, V> {
    const FAMILY: Family = V::FAMILY;
    fn resident_associations(&self) -> Result<u64, Unavailable> {
        self.count.get()
    }
    fn resident_complete(&self) -> bool {
        self.count.get().is_ok()
    }
}
/// Entry access retains only the exact selected value and the cached counter.
pub(super) struct NestedEntry<'a, K, V> {
    entry: Entry<'a, K, V>,
    count: &'a mut AssociationCount,
    previous: AssociationCount,
}
impl<'a, K: Ord, V: AssociationValue + Default> NestedEntry<'a, K, V> {
    /// Count a vacant record plus all changes before releasing its value borrow.
    pub(super) fn or_default(self) -> ValueGuard<'a, V> {
        let (before, value) = match self.entry {
            Entry::Occupied(entry) => (entry.get().association_weight().ok(), entry.into_mut()),
            Entry::Vacant(entry) => (Some(0), entry.insert(V::default())),
        };
        ValueGuard {
            value,
            count: self.count,
            previous: self.previous,
            before,
        }
    }
}
/// Counts a nested mutation when its exclusive value borrow ends.
/// The live map remains poisoned until this guard commits. Forgetting the guard
/// or unwinding before its count commit can never certify stale associations.
pub(super) struct ValueGuard<'a, V: AssociationValue> {
    value: &'a mut V,
    count: &'a mut AssociationCount,
    previous: AssociationCount,
    before: Option<u64>,
}
impl<V: AssociationValue> Deref for ValueGuard<'_, V> {
    type Target = V;
    fn deref(&self) -> &V {
        self.value
    }
}
impl<V: AssociationValue> DerefMut for ValueGuard<'_, V> {
    fn deref_mut(&mut self) -> &mut V {
        self.value
    }
}
impl<V: AssociationValue> Drop for ValueGuard<'_, V> {
    fn drop(&mut self) {
        let mut candidate = self.previous;
        candidate.replace(self.before, self.value.association_weight().ok());
        *self.count = candidate;
    }
}
