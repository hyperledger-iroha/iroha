//! Deterministic parameter (SRS-like) generation for the IPA backend.
//!
//! We derive three sets of generators deterministically:
//! - `g` vector for committing to witness/coefficients (length `n`)
//! - `h` vector for binding the public vector in the IPA proof (length `n`)
//! - a single `u` element to bind the inner product term
//!
//! Production generators are mapped independently with the backend's hash-to-curve suite under a
//! fixed, framed domain. This keeps their discrete-log relationships unknown.
use crate::{
    backend::{IpaBackend, traits::IpaGroup},
    errors::Error,
    hash::sha3_256,
    norito_types::{IpaParams, ZkCurveId},
};
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::RwLock;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
pub(crate) const PARAMS_REGISTRY_MAX_ENTRIES: usize = 32;
/// IPA public parameters instantiated for backend `B`.
#[derive(Clone, Debug)]
pub struct Params<B: IpaBackend> {
    n: usize,
    g: Vec<B::Group>,
    h: Vec<B::Group>,
    u: B::Group,
    fingerprint: OnceCell<[u8; 32]>,
}
impl<B: IpaBackend> Params<B> {
    /// Creates parameters for vectors of length `n` (must be a power of two greater than one).
    pub fn new(n: usize) -> Result<Self, Error> {
        if n < 2 || (n & (n - 1)) != 0 {
            return Err(Error::InvalidN(n));
        }
        let mut g = Vec::with_capacity(n);
        let mut h = Vec::with_capacity(n);
        for i in 0..n {
            let gi = B::derive_group_elem(b"G", n as u64, i as u64);
            let hi = B::derive_group_elem(b"H", n as u64, i as u64);
            g.push(gi);
            h.push(hi);
        }
        let u = B::derive_group_elem(b"U", n as u64, 0);
        Ok(Self {
            n,
            g,
            h,
            u,
            fingerprint: OnceCell::new(),
        })
    }
    /// Return a deliberately different valid generator ordering for transcript tests.
    #[cfg(test)]
    pub(crate) fn with_rotated_generators_for_test(&self) -> Self {
        let mut g = self.g.clone();
        let mut h = self.h.clone();
        g.rotate_left(1);
        h.rotate_right(1);
        Self {
            n: self.n,
            g,
            h,
            u: self.u,
            fingerprint: OnceCell::new(),
        }
    }
    /// Returns the vector length `n`.
    pub fn n(&self) -> usize {
        self.n
    }
    /// Returns the `g` generator vector.
    pub fn g(&self) -> &[B::Group] {
        &self.g
    }
    /// Returns the `h` generator vector.
    pub fn h(&self) -> &[B::Group] {
        &self.h
    }
    /// Returns the `u` generator.
    pub fn u(&self) -> B::Group {
        self.u
    }
    /// Compute and memoize a fingerprint for these parameters.
    pub fn fingerprint(&self) -> [u8; 32] {
        *self.fingerprint.get_or_init(|| {
            let g_bytes: Vec<[u8; 32]> = self.g.iter().map(|g| g.to_bytes()).collect();
            let h_bytes: Vec<[u8; 32]> = self.h.iter().map(|h| h.to_bytes()).collect();
            let u_bytes = self.u.to_bytes();
            fingerprint_bytes(
                B::CURVE_ID.as_u16(),
                self.n as u32,
                &g_bytes,
                &h_bytes,
                &u_bytes,
            )
        })
    }
    fn validate_wire_header(w: &IpaParams) -> Result<usize, Error> {
        if w.version != 1 {
            return Err(Error::UnsupportedVersion {
                component: "IpaParams",
                version: w.version,
            });
        }
        let actual_curve = ZkCurveId::from_u16(w.curve_id);
        if actual_curve != B::CURVE_ID {
            return Err(Error::CurveMismatch {
                expected: B::CURVE_ID,
                actual: actual_curve,
            });
        }
        let n = w.n as usize;
        if n < 2 || (n & (n - 1)) != 0 {
            return Err(Error::InvalidN(n));
        }
        Ok(n)
    }
}
fn fingerprint_bytes(
    curve_id: u16,
    n: u32,
    g: &[[u8; 32]],
    h: &[[u8; 32]],
    u: &[u8; 32],
) -> [u8; 32] {
    let mut buf = Vec::with_capacity(2 + 4 + (g.len() + h.len() + 1) * 32);
    buf.extend_from_slice(&curve_id.to_le_bytes());
    buf.extend_from_slice(&n.to_le_bytes());
    for elem in g {
        buf.extend_from_slice(elem);
    }
    for elem in h {
        buf.extend_from_slice(elem);
    }
    buf.extend_from_slice(u);
    sha3_256(&buf)
}
pub(crate) fn params_from_wire_backend<B>(w: &IpaParams) -> Result<Arc<Params<B>>, Error>
where
    B: IpaBackend + 'static,
{
    // The wire selects only `(curve, n)`: it has no representation for
    // caller-chosen generators. Cache lookup can therefore safely happen before
    // the deterministic derivation.
    let n = Params::<B>::validate_wire_header(w)?;
    if let Some(existing) = PARAMS_REGISTRY.lookup::<B>(n) {
        return Ok(existing);
    }
    let params = Arc::new(Params::<B>::new(n)?);
    Ok(PARAMS_REGISTRY.insert::<B>(n, params))
}
type ParamsKey = (TypeId, ZkCurveId, usize);
type ParamsSlot = Arc<dyn Any + Send + Sync>;
struct ParamsEntry {
    slot: ParamsSlot,
    last_used: u64,
}
struct ParamsRegistry {
    map: RwLock<HashMap<ParamsKey, ParamsEntry>>,
    clock: AtomicU64,
}
impl ParamsRegistry {
    fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
            clock: AtomicU64::new(0),
        }
    }
    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }
    fn lookup<B>(&self, n: usize) -> Option<Arc<Params<B>>>
    where
        B: IpaBackend + 'static,
    {
        let key = (TypeId::of::<B>(), B::CURVE_ID, n);
        let mut guard = self.map.write();
        let entry = guard.get_mut(&key)?;
        entry.last_used = self.tick();
        entry.slot.clone().downcast::<Params<B>>().ok()
    }
    fn insert<B>(&self, n: usize, params: Arc<Params<B>>) -> Arc<Params<B>>
    where
        B: IpaBackend + 'static,
    {
        let key = (TypeId::of::<B>(), B::CURVE_ID, n);
        let mut guard = self.map.write();
        let now = self.tick();
        if let Some(entry) = guard.get_mut(&key) {
            entry.last_used = now;
            return entry
                .slot
                .clone()
                .downcast::<Params<B>>()
                .expect("registry type mismatch");
        }
        if guard.len() >= PARAMS_REGISTRY_MAX_ENTRIES
            && let Some(evict_key) = guard
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
        {
            guard.remove(&evict_key);
        }
        let slot = params.clone() as Arc<dyn Any + Send + Sync>;
        guard.insert(
            key,
            ParamsEntry {
                slot: slot.clone(),
                last_used: now,
            },
        );
        slot.clone()
            .downcast::<Params<B>>()
            .expect("registry type mismatch")
    }
    #[cfg(test)]
    fn clear(&self) {
        self.map.write().clear();
        self.clock.store(0, Ordering::Relaxed);
    }
    #[cfg(test)]
    fn contains<B>(&self, n: usize) -> bool
    where
        B: IpaBackend + 'static,
    {
        self.map
            .read()
            .contains_key(&(TypeId::of::<B>(), B::CURVE_ID, n))
    }
}
static PARAMS_REGISTRY: Lazy<ParamsRegistry> = Lazy::new(ParamsRegistry::new);
#[cfg(test)]
pub(crate) fn clear_params_registry_for_tests() {
    PARAMS_REGISTRY.clear();
}
#[cfg(test)]
pub(crate) fn params_registry_contains_for_tests<B>(n: usize) -> bool
where
    B: IpaBackend + 'static,
{
    PARAMS_REGISTRY.contains::<B>(n)
}
