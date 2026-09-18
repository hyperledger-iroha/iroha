//! Mandatory semantic reservations before an outbound post enters a peer FIFO.
//! All sublimits charge the existing H/L/P physical owners. Private per-PeerId
//! partitions survive reconnect while any accepted post/flush owner is retained.
use super::*;
use crate::TransportAdmissionClass as Class;
const N: usize = Class::COUNT;

#[derive(Debug)]
pub struct Pool {
    budgets: OutboundPostByteBudgets,
    maximum: [usize; N],
    shared: [Arc<SharedByteBudget>; N],
    private: [usize; N],
    sources: Mutex<HashMap<PeerId, Weak<Source>>>,
}
#[derive(Debug)]
pub(super) struct Source {
    pool: Arc<Pool>,
    physical: Arc<SharedByteBudget>,
    private: [Arc<SharedByteBudget>; N],
}
#[derive(Debug)]
pub(super) struct Lease {
    #[cfg(test)]
    pub(super) class: Class,
    _source: Arc<Source>,
    _class: SharedByteLease,
    _physical: SharedByteLease,
}
fn share(total: usize, index: usize, count: usize) -> usize {
    total / count + usize::from(index < total % count)
}
impl Pool {
    pub(super) fn new(
        budgets: OutboundPostByteBudgets,
        maximum: [usize; N],
    ) -> Result<Arc<Self>, Error> {
        let mut caps = [0; N];
        let mut private = [0; N];
        for class in Class::ALL {
            let i = class.index();
            let bytes = maximum[i]
                .checked_add(receive_credit::ENVELOPE_BYTES)
                .ok_or(Error::FrameTooLarge)?;
            if maximum[i] == 0 {
                return Err(Error::Format);
            }
            caps[i] = if class.is_low() {
                share(
                    budgets.low.max_bytes,
                    i - Class::HIGH.len(),
                    Class::LOW.len(),
                )
            } else {
                share(budgets.high.max_bytes, i, Class::HIGH.len())
            };
            if bytes > caps[i] {
                return Err(Error::FrameTooLarge);
            }
            if !class.is_low() {
                private[i] = if matches!(
                    class,
                    Class::Safety
                        | Class::Availability
                        | Class::RecoveryControl
                        | Class::RecoveryData
                ) {
                    bytes
                } else {
                    1 + receive_credit::ENVELOPE_BYTES
                };
            }
        }
        let required = private
            .iter()
            .try_fold(0usize, |sum, n| sum.checked_add(*n))
            .ok_or(Error::FrameTooLarge)?;
        let remainder = budgets
            .progress_reserve_bytes_per_peer
            .checked_sub(required)
            .ok_or(Error::FrameTooLarge)?;
        for class in Class::HIGH {
            let i = class.index();
            private[i] += share(remainder, i, Class::HIGH.len());
        }
        Ok(Arc::new(Self {
            budgets,
            maximum,
            shared: caps
                .map(|bytes| SharedByteBudget::new(bytes, 0).expect("positive admitted class cap")),
            private,
            sources: Mutex::new(HashMap::new()),
        }))
    }
    pub(super) fn bind(self: &Arc<Self>, peer: &PeerId) -> Result<Arc<Source>, Error> {
        let mut sources = self
            .sources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sources.retain(|_, source| source.strong_count() != 0);
        if let Some(source) = sources.get(peer).and_then(Weak::upgrade) {
            return Ok(source);
        }
        // This original strong physical owner also participates in the shared
        // authenticated source census, so a second map cannot admit extra peers.
        let physical = self
            .budgets
            .source_geometry
            .outbound_progress_owner(peer, self.budgets.progress_reserve_bytes_per_peer)
            .ok_or(Error::Format)?;
        let source = Arc::new(Source {
            pool: Arc::clone(self),
            physical,
            private: self.private.map(|bytes| {
                SharedByteBudget::new(bytes.max(1), 0).expect("positive private sublimit")
            }),
        });
        sources.insert(peer.clone(), Arc::downgrade(&source));
        Ok(source)
    }
}
impl Source {
    pub(super) fn reserve(self: &Arc<Self>, class: Class, plaintext: usize) -> Option<Lease> {
        let i = class.index();
        if plaintext == 0 || plaintext > self.pool.maximum[i] {
            return None;
        }
        let bytes = plaintext.checked_add(receive_credit::ENVELOPE_BYTES)?;
        let physical = if class.is_low() {
            &self.pool.budgets.low
        } else {
            &self.pool.budgets.high
        };
        let primary = (|| {
            Some((
                self.pool.shared[i].try_reserve(bytes, false)?,
                physical.try_reserve(bytes, false)?,
            ))
        })();
        let (class_lease, physical) = match primary {
            Some(pair) => pair,
            None if !class.is_low() => (
                self.private[i].try_reserve(bytes, false)?,
                self.physical.try_reserve(bytes, false)?,
            ),
            None => return None,
        };
        Some(Lease {
            #[cfg(test)]
            class,
            _source: Arc::clone(self),
            _class: class_lease,
            _physical: physical,
        })
    }
}

/// No runtime alternative exists: live peer startup receives only `Granted`.
/// The synthetic branch is compiled solely for explicit legacy mailbox fixtures.
#[derive(Clone)]
pub(super) enum Admission {
    Granted(Arc<Source>),
    #[cfg(test)]
    Synthetic {
        high: OutboundHighByteBudget,
        low: Arc<SharedByteBudget>,
        overhead: usize,
    },
}
impl Admission {
    pub(super) fn reserve(
        &self,
        class: Class,
        plaintext: usize,
        _progress: bool,
    ) -> Option<OutboundPostOwnership> {
        match self {
            Self::Granted(source) => Some(OutboundPostOwnership::granted(
                source.reserve(class, plaintext)?,
            )),
            #[cfg(test)]
            Self::Synthetic {
                high,
                low,
                overhead,
            } => {
                let bytes = plaintext.checked_add(*overhead)?;
                let lease = if _progress || !class.is_low() {
                    high.try_reserve(bytes, _progress)?
                } else {
                    low.try_reserve(bytes, false)?
                };
                Some(OutboundPostOwnership::new(lease, None))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn peer(seed: u8) -> PeerId {
        iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal)
            .unwrap()
            .public_key()
            .clone()
            .into()
    }
    fn pool() -> Arc<Pool> {
        let budgets = OutboundPostByteBudgets::new(128 * 1024, 64 * 1024, 32 * 1024, 2).unwrap();
        assert!(
            budgets
                .source_geometry
                .install_protected_sources(HashSet::new())
        );
        Pool::new(budgets, [1024; N]).unwrap()
    }
    #[test]
    fn shared_saturation_preserves_each_peers_protected_post_and_reconnect_owner() {
        let pool = pool();
        let a = pool.bind(&peer(71)).unwrap();
        let mut b = pool.bind(&peer(72)).unwrap();
        let global = pool
            .budgets
            .high
            .try_reserve(pool.budgets.high.max_bytes, false)
            .unwrap();
        let mut held = Vec::new();
        for class in [
            Class::Safety,
            Class::Availability,
            Class::RecoveryControl,
            Class::RecoveryData,
        ] {
            let quota = a.private[class.index()].max_bytes;
            held.push(a.private[class.index()].try_reserve(quota, false).unwrap());
            assert!(a.reserve(class, 1024).is_none());
            let lease = b
                .reserve(class, 1024)
                .expect("other peer has its own protected full maximum");
            let weak = Arc::downgrade(&b);
            drop(b);
            assert!(
                weak.upgrade().is_some(),
                "only the post lease retains the peer owner"
            );
            b = pool.bind(&peer(72)).unwrap();
            assert!(
                Arc::ptr_eq(&b, &weak.upgrade().unwrap()),
                "reconnect reuses the retained owner"
            );
            assert!(
                pool.bind(&peer(73)).is_err(),
                "a retained post consumes the original source slot"
            );
            drop(lease);
        }
        assert!(Arc::ptr_eq(&a, &pool.bind(&peer(71)).unwrap()));
        assert!(pool.bind(&peer(73)).is_err());
        assert_eq!(pool.private.iter().sum::<usize>(), 32 * 1024);
        drop((global, held, a, b));
        assert_eq!(pool.budgets.retained_high_total(), 0);
    }
    #[test]
    fn blocked_payload_cannot_consume_availability_or_low_block_sync_post_bytes() {
        let pool = pool();
        let source = pool.bind(&peer(74)).unwrap();
        let i = Class::Payload.index();
        let shared = pool.shared[i]
            .try_reserve(pool.shared[i].max_bytes, false)
            .unwrap();
        let private = source.private[i]
            .try_reserve(source.private[i].max_bytes, false)
            .unwrap();
        assert!(source.reserve(Class::Payload, 1024).is_none());
        let availability = source.reserve(Class::Availability, 1024).unwrap();
        let sync = source.reserve(Class::BlockSync, 1024).unwrap();
        assert_eq!(
            pool.budgets.retained_low_total(),
            1024 + receive_credit::ENVELOPE_BYTES
        );
        assert_eq!(availability.class, Class::Availability);
        assert_eq!(sync.class, Class::BlockSync);
        drop((shared, private, availability, sync));
        assert!(source.reserve(Class::Payload, 1024).is_some());
    }
    #[test]
    fn mandatory_post_geometry_rejects_unfunded_protected_maximum_without_extra_p() {
        let budgets = OutboundPostByteBudgets::new(128 * 1024, 64 * 1024, 1024, 2).unwrap();
        assert!(Pool::new(budgets, [1024; N]).is_err());
    }
}
