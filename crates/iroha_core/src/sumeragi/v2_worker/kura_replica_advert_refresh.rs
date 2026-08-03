/// Maximum number of exact durable heights inspected by one Sumeragi owner
/// turn.  A turn may transfer at most one fanout regardless of this bound.
pub(crate) const KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN: usize = 8;

/// Arithmetic proactive-refresh window at one exact durable tip.
///
/// The newest `protected_tail` bodies are visited after the immediately
/// preceding `evictable_window`, so bodies which can already consume the
/// eviction budget receive refresh priority.  No height older than these two
/// configured partitions is ever probed by this owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KuraReplicaAdvertRefreshWindow {
    durable_tip: u64,
    evictable_start: u64,
    evictable_len: u64,
    tail_start: u64,
    tail_len: u64,
}

impl KuraReplicaAdvertRefreshWindow {
    fn new(durable_tip: u64, protected_tail: u64, evictable_window: u64) -> Self {
        let tail_len = durable_tip.min(protected_tail);
        let before_tail = durable_tip.saturating_sub(tail_len);
        let evictable_len = before_tail.min(evictable_window);
        let evictable_start = before_tail
            .saturating_sub(evictable_len)
            .saturating_add(u64::from(evictable_len != 0));
        let tail_start = durable_tip
            .saturating_sub(tail_len)
            .saturating_add(u64::from(tail_len != 0));
        Self {
            durable_tip,
            evictable_start,
            evictable_len,
            tail_start,
            tail_len,
        }
    }

    fn len(self) -> u64 {
        self.evictable_len.saturating_add(self.tail_len)
    }

    fn contains(self, height: u64) -> bool {
        if height == 0 || height > self.durable_tip || self.len() == 0 {
            return false;
        }
        let first = if self.evictable_len == 0 {
            self.tail_start
        } else {
            self.evictable_start
        };
        height >= first
    }

    fn height_at(self, offset: u64) -> Option<u64> {
        let height = if offset < self.evictable_len {
            self.evictable_start.saturating_add(offset)
        } else {
            let tail_offset = offset.saturating_sub(self.evictable_len);
            if tail_offset >= self.tail_len {
                return None;
            }
            self.tail_start.saturating_add(tail_offset)
        };
        if height == 0 || height > self.durable_tip {
            return None;
        }
        Some(height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KuraReplicaAdvertRefreshCursor {
    anchor: Option<KuraReplicaAdvertRefreshTip>,
    window: KuraReplicaAdvertRefreshWindow,
    next_offset: u64,
}

impl KuraReplicaAdvertRefreshCursor {
    fn new(
        anchor: Option<KuraReplicaAdvertRefreshTip>,
        window: KuraReplicaAdvertRefreshWindow,
    ) -> Self {
        Self {
            anchor,
            window,
            next_offset: 0,
        }
    }

    fn next_height(&mut self) -> Option<u64> {
        let height = self.window.height_at(self.next_offset)?;
        self.next_offset = self.next_offset.saturating_add(1);
        Some(height)
    }

    fn is_exhausted(self) -> bool {
        self.next_offset >= self.window.len()
    }
}

/// Exact canonical tip identity anchoring one arithmetic scan cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KuraReplicaAdvertRefreshTip {
    height: u64,
    block_hash: HashOf<BlockHeader>,
}

impl KuraReplicaAdvertRefreshTip {
    fn new(height: u64, block_hash: HashOf<BlockHeader>) -> Self {
        Self { height, block_hash }
    }
}

#[derive(Debug)]
struct KuraReplicaAdvertRefreshState {
    observed_durable_tip: Option<KuraReplicaAdvertRefreshTip>,
    cursor: Option<KuraReplicaAdvertRefreshCursor>,
    retained_source: Option<KuraReplicaAdvertSourceV1>,
    /// Body-free height hints whose accepted advert was retired by exact
    /// output handoff before every target admitted it.
    ///
    /// The active arithmetic window bounds this set, and every retry performs
    /// a fresh authenticated Kura source probe.
    urgent_heights: BTreeSet<u64>,
    next_cycle_at: Instant,
    /// A tip which advanced during an in-progress scan requests a follow-up
    /// cycle; it never resets the current cursor and therefore cannot starve
    /// its older evictable prefix.
    follow_up_cycle_requested: bool,
}

impl KuraReplicaAdvertRefreshState {
    fn new(observed_durable_tip: Option<KuraReplicaAdvertRefreshTip>, now: Instant) -> Self {
        Self {
            observed_durable_tip,
            cursor: None,
            retained_source: None,
            urgent_heights: BTreeSet::new(),
            next_cycle_at: now,
            follow_up_cycle_requested: false,
        }
    }

    fn note_durable_tip(&mut self, tip: Option<KuraReplicaAdvertRefreshTip>, now: Instant) {
        let unchanged = match (self.observed_durable_tip, tip) {
            (Some(previous), Some(current)) => {
                previous.height == current.height && previous.block_hash == current.block_hash
            }
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return;
        }
        let invalidates_retained_source = match (self.observed_durable_tip, tip) {
            (Some(previous), Some(current)) => current.height <= previous.height,
            (Some(_), None) => true,
            (None, _) => false,
        };
        self.observed_durable_tip = tip;
        if invalidates_retained_source {
            // Same-height replacement and prune invalidate the exact old-chain
            // token. The current cursor finishes without reset, then an
            // immediate exact-tip-anchored cycle discovers replacement history.
            self.retained_source = None;
        }
        if self.cursor.is_some() || self.retained_source.is_some() {
            self.follow_up_cycle_requested = true;
        } else {
            self.next_cycle_at = now;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KuraReplicaAdvertRefreshTurnOutcome {
    pub(crate) probes: usize,
    pub(crate) fanout_attempted: bool,
    pub(crate) retained_source: bool,
    pub(crate) scan_active: bool,
}

/// Single process-lifetime owner of proactive Kura replica-advert refresh.
///
/// The runner creates this outside its height loop and lends the same owner to
/// each `ProductionV2Services`.  Only a fanout accepted by that height's exact
/// output corridor gains an `ExactOutputRolloverClaim`; this owner's retained
/// opaque Kura source is neither pending exact output nor a finality-seal
/// blocker.
#[derive(Debug)]
pub(crate) struct KuraReplicaAdvertRefreshOwner {
    protected_tail: u64,
    evictable_window: u64,
    refresh_interval: Duration,
    state: Mutex<KuraReplicaAdvertRefreshState>,
}

impl KuraReplicaAdvertRefreshOwner {
    pub(crate) fn from_kura(kura: &Kura, now: Instant) -> Result<Self, String> {
        let protected_tail = u64::try_from(kura.blocks_in_memory().get())
            .map_err(|_| "Kura replica advert protected tail is not representable".to_owned())?;
        let evictable_window = u64::try_from(kura.replica_advert_evictable_window().get())
            .map_err(|_| "Kura replica advert evictable window is not representable".to_owned())?;
        let initial_durable_tip = kura
            .exact_kura_replica_advert_tip()
            .map_err(|error| error.to_string())?
            .map(|(height, block_hash)| KuraReplicaAdvertRefreshTip::new(height, block_hash));
        Self::new(
            protected_tail,
            evictable_window,
            kura.replica_advert_refresh_interval(),
            initial_durable_tip,
            now,
        )
    }

    fn new(
        protected_tail: u64,
        evictable_window: u64,
        refresh_interval: Duration,
        initial_durable_tip: Option<KuraReplicaAdvertRefreshTip>,
        now: Instant,
    ) -> Result<Self, String> {
        if protected_tail == 0
            || evictable_window == 0
            || refresh_interval < KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MIN
        {
            return Err(
                "Kura replica advert refresh geometry must be non-zero and the interval must be at least 1 ms"
                    .to_owned(),
            );
        }
        now.checked_add(refresh_interval).ok_or_else(|| {
            "Kura replica advert refresh interval exceeds the monotonic clock".to_owned()
        })?;
        Ok(Self {
            protected_tail,
            evictable_window,
            refresh_interval,
            state: Mutex::new(KuraReplicaAdvertRefreshState::new(initial_durable_tip, now)),
        })
    }

    pub(crate) fn note_durable_tip(
        &self,
        tip: Option<(u64, HashOf<BlockHeader>)>,
        now: Instant,
    ) -> Result<(), String> {
        if tip.is_some_and(|(height, _)| height == 0) {
            return Err("Kura replica advert refresh observed a zero durable tip".to_owned());
        }
        let mut state = self.lock_state()?;
        state.note_durable_tip(
            tip.map(|(height, block_hash)| KuraReplicaAdvertRefreshTip::new(height, block_hash)),
            now,
        );
        let active_window = KuraReplicaAdvertRefreshWindow::new(
            state.observed_durable_tip.map_or(0, |tip| tip.height),
            self.protected_tail,
            self.evictable_window,
        );
        state
            .urgent_heights
            .retain(|height| active_window.contains(*height));
        Ok(())
    }

    /// Schedule exact body-free height hints retired by finality rollover.
    ///
    /// The current durable-tip window bounds and filters the hints. A later
    /// turn probes each height again, so rollover never promotes queued wire
    /// bytes into durable refresh authority.
    fn schedule_retired_exact_output_heights(
        &self,
        heights: impl IntoIterator<Item = u64>,
        now: Instant,
    ) -> Result<usize, String> {
        let mut state = self.lock_state()?;
        let active_window = KuraReplicaAdvertRefreshWindow::new(
            state.observed_durable_tip.map_or(0, |tip| tip.height),
            self.protected_tail,
            self.evictable_window,
        );
        let mut scheduled = 0usize;
        for height in heights {
            if active_window.contains(height) && state.urgent_heights.insert(height) {
                scheduled = scheduled.checked_add(1).ok_or_else(|| {
                    "Kura replica advert urgent-height count overflowed".to_owned()
                })?;
            }
        }
        if scheduled != 0 {
            state.next_cycle_at = now;
        }
        debug_assert!(
            u64::try_from(state.urgent_heights.len())
                .is_ok_and(|count| count <= active_window.len())
        );
        Ok(scheduled)
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, KuraReplicaAdvertRefreshState>, String> {
        self.state
            .lock()
            .map_err(|_| "Kura replica advert refresh owner lock was poisoned".to_owned())
    }

    /// Advance one bounded owner turn.
    ///
    /// `probe` must authenticate keeper authority without reading the block
    /// body. `transfer` revalidates the exact token, signs, and attempts one
    /// exact fanout. A retained source is always retried before further cursor
    /// probes.
    fn drive_turn(
        &self,
        now: Instant,
        mut probe: impl FnMut(u64) -> Result<Option<KuraReplicaAdvertSourceV1>, String>,
        mut transfer: impl FnMut(&KuraReplicaAdvertSourceV1) -> Result<ExactFanoutOwnership, String>,
    ) -> Result<KuraReplicaAdvertRefreshTurnOutcome, String> {
        let mut state = self.lock_state()?;
        let mut outcome = KuraReplicaAdvertRefreshTurnOutcome::default();

        if let Some(source) = state.retained_source.as_ref() {
            outcome.fanout_attempted = true;
            if transfer(source)? == ExactFanoutOwnership::Owned {
                state.retained_source = None;
            }
            outcome.retained_source = state.retained_source.is_some();
            outcome.scan_active = state.cursor.is_some() || !state.urgent_heights.is_empty();
            return Ok(outcome);
        }

        while outcome.probes < KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN {
            let Some(height) = state.urgent_heights.pop_first() else {
                break;
            };
            outcome.probes = outcome.probes.saturating_add(1);
            let Some(source) = probe(height)? else {
                continue;
            };

            outcome.fanout_attempted = true;
            if transfer(&source)? == ExactFanoutOwnership::SourceRetained {
                state.retained_source = Some(source);
            }
            outcome.retained_source = state.retained_source.is_some();
            outcome.scan_active = state.cursor.is_some() || !state.urgent_heights.is_empty();
            return Ok(outcome);
        }

        if state.cursor.is_none() {
            if now < state.next_cycle_at {
                outcome.scan_active = !state.urgent_heights.is_empty();
                return Ok(outcome);
            }
            let anchor = state.observed_durable_tip;
            let next_cycle_at = now.checked_add(self.refresh_interval).ok_or_else(|| {
                "Kura replica advert refresh deadline exceeds the monotonic clock".to_owned()
            })?;
            state.cursor = Some(KuraReplicaAdvertRefreshCursor::new(
                anchor,
                KuraReplicaAdvertRefreshWindow::new(
                    anchor.map_or(0, |tip| tip.height),
                    self.protected_tail,
                    self.evictable_window,
                ),
            ));
            // Anchor the next cycle to this cycle's start. A slow bounded scan
            // therefore starts its successor immediately after completion
            // instead of adding another complete refresh interval.
            state.next_cycle_at = next_cycle_at;
            state.follow_up_cycle_requested = false;
        }

        while outcome.probes < KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN {
            let Some(height) = state
                .cursor
                .as_mut()
                .and_then(KuraReplicaAdvertRefreshCursor::next_height)
            else {
                break;
            };
            outcome.probes = outcome.probes.saturating_add(1);
            let Some(source) = probe(height)? else {
                continue;
            };

            outcome.fanout_attempted = true;
            if transfer(&source)? == ExactFanoutOwnership::SourceRetained {
                state.retained_source = Some(source);
            }
            outcome.retained_source = state.retained_source.is_some();
            outcome.scan_active = true;
            return Ok(outcome);
        }

        if state.cursor.is_some_and(|cursor| cursor.is_exhausted()) {
            if state
                .cursor
                .is_some_and(|cursor| cursor.anchor != state.observed_durable_tip)
            {
                state.follow_up_cycle_requested = true;
            }
            state.cursor = None;
            if state.follow_up_cycle_requested {
                state.follow_up_cycle_requested = false;
                state.next_cycle_at = now;
            }
        }
        outcome.retained_source = state.retained_source.is_some();
        outcome.scan_active = state.cursor.is_some() || !state.urgent_heights.is_empty();
        Ok(outcome)
    }
}

#[cfg(test)]
mod kura_replica_advert_refresh_tests {
    use super::*;
    use iroha_crypto::Algorithm;

    fn heights(window: KuraReplicaAdvertRefreshWindow) -> Vec<u64> {
        (0..window.len())
            .filter_map(|offset| window.height_at(offset))
            .collect()
    }

    fn tip(height: u64, domain: &[u8]) -> Option<KuraReplicaAdvertRefreshTip> {
        Some(KuraReplicaAdvertRefreshTip::new(
            height,
            HashOf::from_untyped_unchecked(Hash::new(domain)),
        ))
    }

    fn source(height: u64) -> KuraReplicaAdvertSourceV1 {
        let key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::BlsNormal)
            .expect("deterministic refresh test keeper");
        KuraReplicaAdvertSourceV1::for_refresh_owner_test(
            height,
            PeerId::new(key.public_key().clone()),
        )
    }

    #[test]
    fn refresh_window_is_evictable_first_and_overflow_safe() {
        let now = Instant::now();
        assert!(
            KuraReplicaAdvertRefreshOwner::new(
                1,
                1,
                Duration::from_nanos(999_999),
                tip(1, b"sub-millisecond-refresh-owner"),
                now,
            )
            .is_err(),
            "direct construction must enforce the configured 1 ms floor"
        );
        assert!(
            KuraReplicaAdvertRefreshOwner::new(
                1,
                1,
                KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MIN,
                tip(1, b"minimum-refresh-owner"),
                now,
            )
            .is_ok(),
            "the exact configured minimum must remain valid"
        );

        let window = KuraReplicaAdvertRefreshWindow::new(10, 3, 4);
        assert_eq!(heights(window), [4, 5, 6, 7, 8, 9, 10]);
        assert!(!window.contains(0));
        assert!(!window.contains(3));
        assert!(window.contains(4));
        assert!(window.contains(10));
        assert!(!window.contains(11));
        assert_eq!(
            heights(KuraReplicaAdvertRefreshWindow::new(2, 3, 4)),
            [1, 2]
        );
        assert_eq!(
            heights(KuraReplicaAdvertRefreshWindow::new(u64::MAX, 2, 3)),
            [
                u64::MAX - 4,
                u64::MAX - 3,
                u64::MAX - 2,
                u64::MAX - 1,
                u64::MAX
            ]
        );
    }

    #[test]
    fn refresh_turn_retains_one_source_and_attempts_at_most_one_fanout() {
        let now = Instant::now();
        let owner = KuraReplicaAdvertRefreshOwner::new(
            2,
            3,
            Duration::from_secs(60),
            tip(5, b"retained-source-tip"),
            now,
        )
        .expect("valid refresh owner");
        let mut probes = Vec::new();
        let mut fanouts = Vec::new();
        let first = owner
            .drive_turn(
                now,
                |height| {
                    probes.push(height);
                    Ok(Some(source(height)))
                },
                |source| {
                    fanouts.push(source.clone());
                    Ok(ExactFanoutOwnership::SourceRetained)
                },
            )
            .expect("retain first selected source");
        assert_eq!(first.probes, 1);
        assert_eq!(fanouts, [source(1)]);
        assert!(first.retained_source);

        let second = owner
            .drive_turn(
                now,
                |_| panic!("retained source must precede another probe"),
                |source| {
                    fanouts.push(source.clone());
                    Ok(ExactFanoutOwnership::Owned)
                },
            )
            .expect("transfer retained source after service rollover");
        assert_eq!(second.probes, 0);
        assert_eq!(fanouts, [source(1), source(1)]);
        assert!(!second.retained_source);
        assert!(fanouts.len() <= 2, "each of two turns attempted one fanout");

        assert_eq!(
            owner
                .schedule_retired_exact_output_heights([0, 2, 5, 6], now)
                .expect("schedule only active-window rollover heights"),
            2
        );
        let third = owner
            .drive_turn(
                now,
                |height| {
                    probes.push(height);
                    Ok(Some(source(height)))
                },
                |source| {
                    fanouts.push(source.clone());
                    Ok(ExactFanoutOwnership::Owned)
                },
            )
            .expect("retry the oldest rolled-over advert before cursor work");
        assert_eq!(third.probes, 1);
        assert_eq!(probes, [1, 2]);
        assert_eq!(fanouts.last(), Some(&source(2)));
        assert!(
            owner
                .lock_state()
                .expect("inspect remaining urgent rollover height")
                .urgent_heights
                .contains(&5)
        );
    }

    #[test]
    fn tip_advance_never_resets_an_in_progress_scan() {
        let now = Instant::now();
        let owner = KuraReplicaAdvertRefreshOwner::new(
            2,
            2,
            Duration::from_secs(60),
            tip(5, b"short-scan-tip"),
            now,
        )
        .expect("valid refresh owner");
        let mut visited = Vec::new();
        let first = owner
            .drive_turn(
                now,
                |height| {
                    visited.push(height);
                    Ok(None)
                },
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("scan initial window");
        assert_eq!(first.probes, 4);
        assert_eq!(visited, [2, 3, 4, 5]);

        // Use a larger window than the per-turn probe bound to expose the
        // non-reset property across turns.
        let owner = KuraReplicaAdvertRefreshOwner::new(
            5,
            6,
            Duration::from_secs(60),
            tip(20, b"long-scan-tip"),
            now,
        )
        .expect("valid long refresh owner");
        visited.clear();
        let first = owner
            .drive_turn(
                now,
                |height| {
                    visited.push(height);
                    Ok(None)
                },
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("scan bounded prefix");
        assert_eq!(first.probes, KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN);
        assert_eq!(visited, [10, 11, 12, 13, 14, 15, 16, 17]);
        owner
            .note_durable_tip(
                Some((
                    23,
                    HashOf::from_untyped_unchecked(Hash::new(b"advanced-scan-tip")),
                )),
                now,
            )
            .expect("record advanced durable tip");
        owner
            .drive_turn(
                now,
                |height| {
                    visited.push(height);
                    Ok(None)
                },
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("finish original window before following new tip");
        assert_eq!(visited, [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);

        visited.clear();
        owner
            .drive_turn(
                now,
                |height| {
                    visited.push(height);
                    Ok(None)
                },
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("start immediate follow-up window");
        assert_eq!(visited, [13, 14, 15, 16, 17, 18, 19, 20]);

        let slow_owner = KuraReplicaAdvertRefreshOwner::new(
            5,
            6,
            Duration::from_secs(1),
            tip(20, b"slow-cycle-tip"),
            now,
        )
        .expect("valid slow-cycle refresh owner");
        let after_deadline = now + Duration::from_secs(2);
        slow_owner
            .drive_turn(
                now,
                |_| Ok(None),
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("start scan and anchor its next-cycle deadline");
        slow_owner
            .drive_turn(
                after_deadline,
                |_| Ok(None),
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("finish scan after its anchored deadline");
        let immediate = slow_owner
            .drive_turn(
                after_deadline,
                |_| Ok(None),
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("overdue successor cycle starts without another interval");
        assert_eq!(
            immediate.probes,
            KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN
        );
    }

    #[test]
    fn same_height_tip_rewrite_requests_follow_up_without_starving_current_cursor() {
        let now = Instant::now();
        let owner = KuraReplicaAdvertRefreshOwner::new(
            5,
            6,
            Duration::from_secs(60),
            tip(20, b"canonical-tip-a"),
            now,
        )
        .expect("valid rewrite refresh owner");
        let mut visited = Vec::new();
        owner
            .drive_turn(
                now,
                |height| {
                    visited.push(height);
                    Ok(None)
                },
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("scan bounded original prefix");
        assert_eq!(visited, [10, 11, 12, 13, 14, 15, 16, 17]);

        owner
            .note_durable_tip(
                Some((
                    20,
                    HashOf::from_untyped_unchecked(Hash::new(b"canonical-tip-b")),
                )),
                now,
            )
            .expect("observe same-height replacement tip");
        owner
            .drive_turn(
                now,
                |height| {
                    visited.push(height);
                    Ok(None)
                },
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("finish the original cursor without reset");
        assert_eq!(visited, [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);

        visited.clear();
        owner
            .drive_turn(
                now,
                |height| {
                    visited.push(height);
                    Ok(None)
                },
                |_| panic!("nonkeeper scan cannot fan out"),
            )
            .expect("immediately scan replacement history");
        assert_eq!(visited, [10, 11, 12, 13, 14, 15, 16, 17]);
        assert_eq!(
            owner
                .lock_state()
                .expect("inspect rewrite cursor")
                .cursor
                .expect("replacement cursor remains active")
                .anchor,
            tip(20, b"canonical-tip-b")
        );
    }
}
