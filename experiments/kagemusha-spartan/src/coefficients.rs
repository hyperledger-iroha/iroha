//! Public per-slot coefficients in a uniform matrix template.
use std::collections::HashSet;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use super::UniformCircuit;

/// Builders own their buffers. Once placed, equal coefficients of the primary
/// matrix can share one allocation while its prover waits for the secondary.
#[derive(Clone, Debug)]
pub enum Coefficients<F> {
    Owned(Vec<F>),
    Shared(Arc<[F]>),
}

impl<F> From<Vec<F>> for Coefficients<F> {
    fn from(values: Vec<F>) -> Self {
        Self::Owned(values)
    }
}
impl<F> FromIterator<F> for Coefficients<F> {
    fn from_iter<T: IntoIterator<Item = F>>(values: T) -> Self {
        Self::Owned(values.into_iter().collect())
    }
}

impl<F> Deref for Coefficients<F> {
    type Target = [F];

    fn deref(&self) -> &[F] {
        match self {
            Self::Owned(values) => values,
            Self::Shared(values) => values,
        }
    }
}

impl<F: Clone> DerefMut for Coefficients<F> {
    fn deref_mut(&mut self) -> &mut [F] {
        match self {
            Self::Owned(values) => values,
            Self::Shared(values) => Arc::make_mut(values),
        }
    }
}

impl<F: PartialEq> PartialEq for Coefficients<F> {
    fn eq(&self, other: &Self) -> bool {
        self[..] == other[..]
    }
}

impl<F> Coefficients<F> {
    /// Capacity in bytes, counting a shared allocation once per census.
    pub(super) fn heap_bytes(&self, seen: &mut HashSet<*const F>) -> u128 {
        let field_bytes = std::mem::size_of::<F>() as u128;
        match self {
            Self::Owned(values) => values.capacity() as u128 * field_bytes,
            Self::Shared(values) if seen.insert(values.as_ptr()) => {
                values.len() as u128 * field_bytes + (2 * size_of::<usize>()) as u128
            }
            Self::Shared(_) => 0,
        }
    }
}

impl<F: Clone + Eq + Hash> Coefficients<F> {
    fn share(&mut self, pool: &mut HashSet<Arc<[F]>>) {
        // Bound temporary table storage for matrices with many distinct
        // coefficients. Remaining buffers keep their ordinary representation.
        const MAX_ENTRIES: usize = 4096;
        if let Some(existing) = pool.get(&self[..]) {
            *self = Self::Shared(Arc::clone(existing));
        } else if pool.len() < MAX_ENTRIES
            && let Self::Owned(values) = self
        {
            let shared: Arc<[F]> = std::mem::take(values).into();
            pool.insert(Arc::clone(&shared));
            *self = Self::Shared(shared);
        }
    }
}

impl<F: Clone + Eq + Hash> UniformCircuit<F> {
    pub(crate) fn share_template_coefficients(&mut self) {
        let mut pool = HashSet::new();
        for a in &mut self.arrays {
            let rows = a.rows.iter_mut().flatten();
            let lookups = a.lookups.iter_mut().flat_map(|(_, tuple)| tuple.iter_mut());
            for term in rows.chain(lookups).flatten() {
                term.coef.share(&mut pool);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooling_preserves_coefficients_on_collision_and_after_mutation() {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct CollidingWord(u64);

        impl Hash for CollidingWord {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                0u64.hash(state);
            }
        }
        let mut pool = HashSet::new();
        let mut a: Coefficients<_> = vec![CollidingWord(1), CollidingWord(2)].into();
        let mut b = a.clone();
        let mut c: Coefficients<_> = vec![CollidingWord(1), CollidingWord(3)].into();
        a.share(&mut pool);
        b.share(&mut pool);
        c.share(&mut pool);
        drop(pool);
        assert_eq!(a.as_ptr(), b.as_ptr());
        assert_ne!(a.as_ptr(), c.as_ptr());
        let mut seen = HashSet::new();
        assert!(a.heap_bytes(&mut seen) > 0);
        assert_eq!(b.heap_bytes(&mut seen), 0);
        b[1] = CollidingWord(7);
        assert_eq!(a[1], CollidingWord(2));
        assert_eq!(b[1], CollidingWord(7));
        assert_eq!(c[1], CollidingWord(3));
        assert!(b.heap_bytes(&mut seen) > 0);
    }
}
