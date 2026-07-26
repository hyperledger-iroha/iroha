//! Deterministic parameter (SRS-like) generation for the IPA backend.
//!
//! We derive three sets of generators deterministically:
//! - `g` vector for committing to witness/coefficients (length `n`)
//! - `h` vector for binding the public vector in the IPA proof (length `n`)
//! - a single `u` element to bind the inner product term
//!
//! All generators are derived using SHA3-256 under a fixed DST.

use std::{
    any::Any,
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use once_cell::sync::{Lazy, OnceCell};
use parking_lot::RwLock;

use crate::{
    backend::{IpaBackend, traits::IpaGroup},
    errors::Error,
    norito_types::{IpaParams, ZkCurveId, fingerprint_bytes},
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
    /// Creates parameters for vectors of length `n` (must be a non-zero power of two).
    pub fn new(n: usize) -> Result<Self, Error> {
        if n == 0 || (n & (n - 1)) != 0 {
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

    /// Construct parameters directly from provided generator vectors.
    pub(crate) fn from_generators(
        n: usize,
        g: Vec<B::Group>,
        h: Vec<B::Group>,
        u: B::Group,
    ) -> Result<Self, Error> {
        use std::collections::HashSet;

        if n == 0 || (n & (n - 1)) != 0 {
            return Err(Error::InvalidN(n));
        }
        if g.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: g.len(),
            });
        }
        if h.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: h.len(),
            });
        }

        let identity = B::Group::identity();

        let mut seen_g = HashSet::with_capacity(n);
        for (idx, point) in g.iter().enumerate() {
            if *point == identity {
                return Err(Error::InvalidGenerator {
                    kind: "G",
                    index: idx,
                    reason: "identity element is not a valid generator",
                });
            }
            let inserted = seen_g.insert(point.to_bytes());
            if !inserted {
                return Err(Error::InvalidGenerator {
                    kind: "G",
                    index: idx,
                    reason: "duplicate generator",
                });
            }
        }

        let mut seen_h = HashSet::with_capacity(n);
        for (idx, point) in h.iter().enumerate() {
            if *point == identity {
                return Err(Error::InvalidGenerator {
                    kind: "H",
                    index: idx,
                    reason: "identity element is not a valid generator",
                });
            }
            let inserted = seen_h.insert(point.to_bytes());
            if !inserted {
                return Err(Error::InvalidGenerator {
                    kind: "H",
                    index: idx,
                    reason: "duplicate generator",
                });
            }
        }

        if u == identity {
            return Err(Error::InvalidGenerator {
                kind: "U",
                index: 0,
                reason: "identity element is not a valid generator",
            });
        }

        Ok(Self {
            n,
            g,
            h,
            u,
            fingerprint: OnceCell::new(),
        })
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
        if n == 0 || (n & (n - 1)) != 0 {
            return Err(Error::InvalidN(n));
        }
        if w.g.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: w.g.len(),
            });
        }
        if w.h.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: w.h.len(),
            });
        }
        Ok(n)
    }

    pub(crate) fn from_wire(w: &IpaParams) -> Result<Self, Error> {
        let n = Self::validate_wire_header(w)?;
        let g =
            w.g.iter()
                .map(B::Group::from_bytes)
                .collect::<Result<Vec<_>, _>>()?;
        let h =
            w.h.iter()
                .map(B::Group::from_bytes)
                .collect::<Result<Vec<_>, _>>()?;
        let u = B::Group::from_bytes(&w.u)?;
        Self::from_generators(n, g, h, u)
    }
}

pub(crate) fn params_from_wire_backend<B>(w: &IpaParams) -> Result<Arc<Params<B>>, Error>
where
    B: IpaBackend + 'static,
{
    Params::<B>::validate_wire_header(w)?;
    let advertised_fp = w.fingerprint();
    if let Some(existing) = PARAMS_REGISTRY.lookup::<B>(&advertised_fp) {
        return Ok(existing);
    }

    // Parse generator set directly from the wire payload so callers can supply
    // pre-generated structured references (e.g., larger SRS snapshots) instead
    // of relying on the deterministic DST derivation.  The decoded parameter
    // set is cached under its fingerprint to amortise future decodes.
    let params = Arc::new(Params::<B>::from_wire(w)?);
    let fp = params.fingerprint();
    Ok(PARAMS_REGISTRY.insert::<B>(fp, params))
}

pub(crate) fn register_params_from_wire<B>(w: &IpaParams) -> Result<Arc<Params<B>>, Error>
where
    B: IpaBackend + 'static,
{
    let params = Arc::new(Params::<B>::from_wire(w)?);
    let fp = params.fingerprint();
    Ok(PARAMS_REGISTRY.insert::<B>(fp, params))
}

type ParamsKey = (ZkCurveId, [u8; 32]);
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

    fn lookup<B>(&self, fp: &[u8; 32]) -> Option<Arc<Params<B>>>
    where
        B: IpaBackend + 'static,
    {
        let key = (B::CURVE_ID, *fp);
        let mut guard = self.map.write();
        let entry = guard.get_mut(&key)?;
        entry.last_used = self.tick();
        entry.slot.clone().downcast::<Params<B>>().ok()
    }

    fn insert<B>(&self, fp: [u8; 32], params: Arc<Params<B>>) -> Arc<Params<B>>
    where
        B: IpaBackend + 'static,
    {
        let key = (B::CURVE_ID, fp);
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
    fn len(&self) -> usize {
        self.map.read().len()
    }

    #[cfg(test)]
    fn contains<B>(&self, fp: &[u8; 32]) -> bool
    where
        B: IpaBackend + 'static,
    {
        self.map.read().contains_key(&(B::CURVE_ID, *fp))
    }
}

static PARAMS_REGISTRY: Lazy<ParamsRegistry> = Lazy::new(ParamsRegistry::new);

#[cfg(test)]
pub(crate) fn clear_params_registry_for_tests() {
    PARAMS_REGISTRY.clear();
}

#[cfg(test)]
pub(crate) fn params_registry_len_for_tests() -> usize {
    PARAMS_REGISTRY.len()
}

#[cfg(test)]
pub(crate) fn params_registry_contains_for_tests<B>(fp: &[u8; 32]) -> bool
where
    B: IpaBackend + 'static,
{
    PARAMS_REGISTRY.contains::<B>(fp)
}
