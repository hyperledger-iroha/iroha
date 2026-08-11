/// Provider of online peers.
#[derive(Clone)]
pub struct OnlinePeersProvider {
    rx: watch::Receiver<HashSet<Peer>>,
    response_limit: usize,
}

impl OnlinePeersProvider {
    /// Construct a provider with the core network profile's connection ceiling.
    pub fn new(rx: watch::Receiver<HashSet<Peer>>) -> Self {
        Self::new_with_response_limit(
            rx,
            iroha_config::parameters::defaults::network::lane_profile::CORE_MAX_TOTAL_CONNECTIONS,
        )
    }

    /// Construct a provider whose diagnostic response is bounded by the resolved
    /// P2P total-connection ceiling.
    pub fn new_with_response_limit(
        rx: watch::Receiver<HashSet<Peer>>,
        response_limit: usize,
    ) -> Self {
        Self {
            rx,
            response_limit: response_limit.max(1),
        }
    }

    /// Inspect the live peer set without cloning it.
    ///
    /// Keep the closure synchronous and short: the watch borrow prevents the
    /// producer from replacing its snapshot until the closure returns.
    pub(crate) fn with_snapshot<R>(&self, inspect: impl FnOnce(&HashSet<Peer>) -> R) -> R {
        let peers = self.rx.borrow();
        inspect(&peers)
    }

    pub(crate) fn bounded_response_snapshot(&self) -> HashSet<Peer> {
        let peers = self.rx.borrow();
        if peers.len() <= self.response_limit {
            return peers.clone();
        }

        // HashSet iteration order is process-randomized. Retain the smallest
        // peer identities so an invariant violation cannot make truncation
        // nondeterministic, while cloning at most the configured connection
        // ceiling instead of materializing the whole watch value.
        let mut selected = std::collections::BTreeSet::new();
        for peer in peers.iter() {
            if selected.len() < self.response_limit {
                selected.insert(peer.clone());
                continue;
            }
            if selected.last().is_some_and(|largest| peer < largest) {
                let _ = selected.pop_last();
                selected.insert(peer.clone());
            }
        }
        selected.into_iter().collect()
    }
}
