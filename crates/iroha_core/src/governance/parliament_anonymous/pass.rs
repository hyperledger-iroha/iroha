//! Ordering and the single work cursor of the anonymous family's positioned pass.
//!
//! Each segment consumes a frozen occurrence list: work its callbacks create in the same
//! or an earlier segment waits for another block. G2a merges Parliament and SCCP by time
//! before rank. No continuation gets a fresh budget or restarts positions.
//!
//! `world_pass::run_governance_pass` is the only caller: it begins one cursor per block of
//! the global chain and lists Parliament's and SCCP's items into it. This cursor alone
//! grants no execution, SCCP availability or activation authority.

use iroha_data_model::{
    block::BlockHeader,
    bridge::SccpNetworkV1,
    governance::{
        GovernanceAnchorV1,
        types::{GovernanceAttemptId, ParliamentBody},
    },
    sccp::fast_pause::SccpPanelRoleV1,
};

use super::schedule::{
    AnonymousDueItemV1, CLOSE_BALLOT_COST_V1, DRAW_GENERATION_COST_V1, OPEN_BALLOT_COST_V1,
};
use crate::state::StateBlock;

/// The five budgeted segments, in execution order. Prelude, G0 and G4 are unpositioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GovernancePassSegmentV1 {
    /// Height progress followed by exact-slot generation draws.
    G1,
    /// Lifecycle, pause conclusions and reconciliation ends, merged by due time.
    G2a,
    /// Ballot closures.
    G2b,
    /// Ballot openings.
    G2c,
    /// Parliament enactments followed by fast-pause enactments.
    G3,
}

impl GovernancePassSegmentV1 {
    fn next(self) -> Option<Self> {
        match self {
            Self::G1 => Some(Self::G2a),
            Self::G2a => Some(Self::G2b),
            Self::G2b => Some(Self::G2c),
            Self::G2c => Some(Self::G3),
            Self::G3 => None,
        }
    }
}

/// One occurrence in the common pass. Owners still authenticate its source and liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GovernancePassItemV1 {
    /// Recurring height-progress ticket, before every draw.
    HeightProgress,
    /// One complete generation draw.
    Draw {
        slot: u64,
        attempt: GovernanceAttemptId,
    },
    /// A Parliament rank-zero, one or two lifecycle occurrence.
    Lifecycle(AnonymousDueItemV1),
    /// A rank-three emergency-panel conclusion.
    PauseConclusion {
        due_ms: u64,
        network: SccpNetworkV1,
        role: SccpPanelRoleV1,
    },
    /// A rank-three reconciliation end.
    Reconciliation {
        due_ms: u64,
        network: SccpNetworkV1,
        revision: u32,
    },
    /// One binding ballot's closure.
    Close {
        due_ms: u64,
        attempt: GovernanceAttemptId,
        body: ParliamentBody,
    },
    /// One binding ballot's opening.
    Open {
        due_ms: u64,
        attempt: GovernanceAttemptId,
        body: ParliamentBody,
    },
    /// One production Parliament certificate's effect.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "G3 lists production enactments from slice 5; this release certifies no production attempt"
        )
    )]
    EnactParliament {
        due_ms: u64,
        attempt: GovernanceAttemptId,
    },
    /// One same-pass certified emergency pause.
    EnactPause {
        network: SccpNetworkV1,
        role: SccpPanelRoleV1,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ItemIdentity {
    Progress,
    Parliament(GovernanceAttemptId, u8, Option<ParliamentBody>),
    Sccp(SccpNetworkV1, u8, u64),
    Enactment(u64, GovernanceAttemptId),
}

fn role_index(role: SccpPanelRoleV1) -> u64 {
    match role {
        SccpPanelRoleV1::Primary => 0,
        SccpPanelRoleV1::Backup => 1,
    }
}

impl GovernancePassItemV1 {
    fn key(self) -> (GovernancePassSegmentV1, u64, u8, ItemIdentity) {
        use GovernancePassSegmentV1 as Segment;
        use ItemIdentity as Key;
        match self {
            Self::HeightProgress => (Segment::G1, 0, 0, Key::Progress),
            Self::Draw { slot, attempt } => {
                (Segment::G1, slot, 1, Key::Parliament(attempt, 0, None))
            }
            Self::Lifecycle(due) => (
                Segment::G2a,
                due.due_ms,
                due.item.rank(),
                Key::Parliament(due.governance_attempt_id, due.item.kind(), due.item.body()),
            ),
            Self::PauseConclusion {
                due_ms,
                network,
                role,
            } => (
                Segment::G2a,
                due_ms,
                3,
                Key::Sccp(network, 0, role_index(role)),
            ),
            Self::Reconciliation {
                due_ms,
                network,
                revision,
            } => (
                Segment::G2a,
                due_ms,
                3,
                Key::Sccp(network, 1, u64::from(revision)),
            ),
            Self::Close {
                due_ms,
                attempt,
                body,
            } => (
                Segment::G2b,
                due_ms,
                0,
                Key::Parliament(attempt, 0, Some(body)),
            ),
            Self::Open {
                due_ms,
                attempt,
                body,
            } => (
                Segment::G2c,
                due_ms,
                0,
                Key::Parliament(attempt, 0, Some(body)),
            ),
            Self::EnactParliament { due_ms, attempt } => {
                (Segment::G3, 0, 0, Key::Enactment(due_ms, attempt))
            }
            Self::EnactPause { network, role } => {
                (Segment::G3, 1, 0, Key::Sccp(network, 0, role_index(role)))
            }
        }
    }

    fn is_due(self, header: &BlockHeader) -> bool {
        match self {
            Self::HeightProgress | Self::EnactPause { .. } => true,
            Self::Draw { slot, .. } => slot <= header.height().get(),
            Self::Lifecycle(due) => due.due_ms <= header.creation_time_ms,
            Self::PauseConclusion { due_ms, .. }
            | Self::Reconciliation { due_ms, .. }
            | Self::Close { due_ms, .. }
            | Self::Open { due_ms, .. }
            | Self::EnactParliament { due_ms, .. } => due_ms <= header.creation_time_ms,
        }
    }

    /// The fixed owner-defined charge, never selected by an executor callback.
    pub(crate) const fn cost(self) -> u32 {
        match self {
            Self::HeightProgress | Self::PauseConclusion { .. } | Self::Reconciliation { .. } => 1,
            Self::Draw { .. } => DRAW_GENERATION_COST_V1,
            Self::Lifecycle(due) => due.item.cost(),
            Self::Close { .. } => CLOSE_BALLOT_COST_V1,
            Self::Open { .. } => OPEN_BALLOT_COST_V1,
            Self::EnactParliament { .. } | Self::EnactPause { .. } => 4,
        }
    }
}

/// The owner's observation immediately before charging an occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GovernanceOccurrenceV1 {
    /// The exact occurrence still exists and can be processed.
    Live,
    /// An earlier item ended the occurrence or its attempt; no cost or position is taken.
    Lapsed,
    /// All casting slots are occupied. Only an opening may return this disposition.
    NoCastingSlot,
    /// The next item cannot fit the fixed committed carrier byte or row partition.
    /// The owner checks this before mutation against its retained reservation. This never
    /// represents a local allocator, host-memory, verifier or storage refusal: those must
    /// return the original owner error and abandon the block.
    NoOutputCapacity,
}

/// Why the cursor refuses to continue. A callback failure must discard the original block.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GovernancePassErrorV1<E> {
    /// A committed parameter failed decoding or validation.
    Policy(&'static str),
    /// The block or original World publication differs from the one that started the pass.
    Source,
    /// A segment was repeated, omitted, reordered, or resumed after failure.
    Segment,
    /// An item belongs to another segment or repeats a live order key.
    Items,
    /// An owner returned an invalid occurrence disposition.
    Occurrence,
    /// The original owner failed; its error is preserved, never turned into a verdict.
    Owner(E),
    /// The next position cannot be expressed.
    Position,
}

/// The one source-bound budget and position cursor; neither clonable nor serializable.
#[derive(Debug)]
pub(crate) struct SharedGovernancePassV1 {
    source: mv::BlockPublicationIdentity,
    header: BlockHeader,
    remaining: u32,
    position: u32,
    next: Option<GovernancePassSegmentV1>,
    stopped: bool,
}

impl SharedGovernancePassV1 {
    /// Begin once for the original block, reading the committed shared clock policy.
    /// Only the Parliament owner can construct a cursor. Its selected service must call
    /// this once and retain the original cursor through all SCCP continuations.
    pub(super) fn begin<E>(state: &StateBlock<'_>) -> Result<Self, GovernancePassErrorV1<E>> {
        let policy = crate::smartcontracts::isi::sccp::keepalive::clock_parameters(&state.world)
            .map_err(GovernancePassErrorV1::Policy)?;
        Ok(Self {
            source: state
                .world
                .parliament_anonymous_attempts
                .publication_identity(),
            header: state._curr_block,
            remaining: policy.governance_block_start_work_units,
            position: 0,
            next: Some(GovernancePassSegmentV1::G1),
            stopped: false,
        })
    }

    /// How many G1–G3 items this pass has processed; also the next item's position.
    pub(crate) const fn processed(&self) -> u32 {
        self.position
    }

    /// The work units left to the items after the first.
    pub(crate) const fn remaining(&self) -> u32 {
        self.remaining
    }

    /// Run one frozen segment snapshot. `inspect` reads the current original State, and
    /// `apply` must atomically apply that occurrence at its supplied anchor. Any error poisons
    /// the cursor: the caller must abandon this block, including previously applied items.
    ///
    /// Work-budget exhaustion or deterministic committed output-capacity exhaustion
    /// blocks every later segment. An output-capacity stop consumes neither work nor a
    /// position and leaves the occurrence due. The owner must establish mandatory prelude
    /// capacity and capacity for the first maximal item before starting the pass; this cursor
    /// does not acquire that reservation. A full casting set stops only G2c, allowing G3 to
    /// spend the original remaining budget. Local resource refusals are errors, never capacity
    /// dispositions, and poison the pass instead of producing a committed deferral.
    pub(crate) fn run_segment<E>(
        &mut self,
        state: &mut StateBlock<'_>,
        segment: GovernancePassSegmentV1,
        mut items: Vec<GovernancePassItemV1>,
        mut inspect: impl FnMut(
            &StateBlock<'_>,
            GovernancePassItemV1,
        ) -> Result<GovernanceOccurrenceV1, E>,
        mut apply: impl FnMut(
            &mut StateBlock<'_>,
            GovernancePassItemV1,
            GovernanceAnchorV1,
        ) -> Result<(), E>,
    ) -> Result<(), GovernancePassErrorV1<E>> {
        // Poison before anything fallible, so an owner error cannot be swallowed and retried.
        if self.next.take() != Some(segment) {
            return Err(GovernancePassErrorV1::Segment);
        }
        if self.source
            != state
                .world
                .parliament_anonymous_attempts
                .publication_identity()
            || self.header != state._curr_block
        {
            return Err(GovernancePassErrorV1::Source);
        }
        items.sort_unstable_by_key(|item| item.key());
        if items
            .iter()
            .any(|item| item.key().0 != segment || !item.is_due(&self.header))
            || items.windows(2).any(|pair| pair[0].key() == pair[1].key())
        {
            return Err(GovernancePassErrorV1::Items);
        }
        if !self.stopped {
            for item in items {
                if self.source
                    != state
                        .world
                        .parliament_anonymous_attempts
                        .publication_identity()
                    || self.header != state._curr_block
                {
                    return Err(GovernancePassErrorV1::Source);
                }
                match inspect(state, item).map_err(GovernancePassErrorV1::Owner)? {
                    GovernanceOccurrenceV1::Lapsed => continue,
                    GovernanceOccurrenceV1::NoOutputCapacity => {
                        self.stopped = true;
                        break;
                    }
                    GovernanceOccurrenceV1::NoCastingSlot
                        if segment == GovernancePassSegmentV1::G2c =>
                    {
                        break;
                    }
                    GovernanceOccurrenceV1::NoCastingSlot => {
                        return Err(GovernancePassErrorV1::Occurrence);
                    }
                    GovernanceOccurrenceV1::Live => (),
                }
                if self.position != 0 && item.cost() > self.remaining {
                    self.stopped = true;
                    break;
                }
                let next_position = self
                    .position
                    .checked_add(1)
                    .ok_or(GovernancePassErrorV1::Position)?;
                let at = GovernanceAnchorV1 {
                    height: self.header.height().get(),
                    position: self.position,
                    timestamp_ms: self.header.creation_time_ms,
                };
                apply(state, item, at).map_err(GovernancePassErrorV1::Owner)?;
                if self.source
                    != state
                        .world
                        .parliament_anonymous_attempts
                        .publication_identity()
                    || self.header != state._curr_block
                {
                    return Err(GovernancePassErrorV1::Source);
                }
                self.position = next_position;
                self.remaining = self.remaining.saturating_sub(item.cost());
            }
        }
        self.next = segment.next();
        Ok(())
    }
}

/// Run `segment` of a fresh cursor on `state` with every item live, after the earlier
/// segments ran empty, and record the order the items are applied in.
#[cfg(test)]
pub(crate) fn order_for_test(
    state: &mut StateBlock<'_>,
    segment: GovernancePassSegmentV1,
    items: Vec<GovernancePassItemV1>,
    seen: &mut Vec<GovernancePassItemV1>,
) {
    let mut pass = SharedGovernancePassV1::begin::<()>(state).expect("a cursor");
    let mut earlier = Some(GovernancePassSegmentV1::G1);
    while let Some(current) = earlier.filter(|current| *current != segment) {
        pass.run_segment::<()>(
            state,
            current,
            Vec::new(),
            |_, _| Ok(GovernanceOccurrenceV1::Live),
            |_, _, _| Ok(()),
        )
        .expect("an empty segment");
        earlier = current.next();
    }
    pass.run_segment::<()>(
        state,
        segment,
        items,
        |_, _| Ok(GovernanceOccurrenceV1::Live),
        |_, item, _| {
            seen.push(item);
            Ok(())
        },
    )
    .expect("the ordered segment");
}

#[cfg(test)]
mod tests {
    use super::super::schedule::AnonymousLifecycleItemV1;
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};

    fn attempt(byte: u8) -> GovernanceAttemptId {
        GovernanceAttemptId::new([byte; 32])
    }
    fn lifecycle(time: u64, id: u8) -> GovernancePassItemV1 {
        GovernancePassItemV1::Lifecycle(AnonymousDueItemV1 {
            due_ms: time,
            governance_attempt_id: attempt(id),
            item: AnonymousLifecycleItemV1::StartQualification,
        })
    }
    fn live(_: &StateBlock<'_>, _: GovernancePassItemV1) -> Result<GovernanceOccurrenceV1, ()> {
        Ok(GovernanceOccurrenceV1::Live)
    }
    fn noop(
        _: &mut StateBlock<'_>,
        _: GovernancePassItemV1,
        _: GovernanceAnchorV1,
    ) -> Result<(), ()> {
        Ok(())
    }

    #[test]
    fn shared_order_compares_due_time_before_rank_and_preserves_all_segment_keys() {
        let network = SccpNetworkV1::EthereumMainnet;
        let reconciliation = GovernancePassItemV1::Reconciliation {
            due_ms: 5,
            network,
            revision: 3,
        };
        let pause = GovernancePassItemV1::PauseConclusion {
            due_ms: 7,
            network,
            role: SccpPanelRoleV1::Primary,
        };
        assert!(reconciliation.key() < lifecycle(6, 1).key());
        assert!(lifecycle(7, 1).key() < pause.key());
        assert!(
            pause.key()
                < GovernancePassItemV1::Reconciliation {
                    due_ms: 7,
                    network,
                    revision: 0
                }
                .key()
        );
        assert!(
            GovernancePassItemV1::HeightProgress.key()
                < GovernancePassItemV1::Draw {
                    slot: 0,
                    attempt: attempt(1)
                }
                .key()
        );
        assert!(
            GovernancePassItemV1::EnactParliament {
                due_ms: u64::MAX,
                attempt: attempt(1)
            }
            .key()
                < GovernancePassItemV1::EnactPause {
                    network,
                    role: SccpPanelRoleV1::Primary
                }
                .key()
        );
    }

    #[test]
    fn one_budget_stops_every_later_segment_without_skipping_a_large_item() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        pass.remaining = 43;
        let draws = vec![
            GovernancePassItemV1::Draw {
                slot: 1,
                attempt: attempt(2),
            },
            GovernancePassItemV1::HeightProgress,
            GovernancePassItemV1::Draw {
                slot: 1,
                attempt: attempt(1),
            },
        ];
        let mut positions = Vec::new();
        pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G1,
            draws,
            live,
            |_, item, at| {
                positions.push((item, at.position));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(pass.remaining, 10);
        assert_eq!(pass.processed(), 2);
        pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G2a,
            vec![lifecycle(0, 1)],
            |_, _| panic!("budget stop must not inspect later work"),
            noop,
        )
        .unwrap();
        assert_eq!(pass.processed(), 2);
    }

    #[test]
    fn first_live_item_always_runs_and_lapsed_items_take_no_position() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        pass.remaining = 0;
        pass.run_segment::<()>(
            &mut block,
            GovernancePassSegmentV1::G1,
            vec![
                GovernancePassItemV1::HeightProgress,
                GovernancePassItemV1::Draw {
                    slot: 1,
                    attempt: attempt(1),
                },
            ],
            |_, item| {
                Ok(if item == GovernancePassItemV1::HeightProgress {
                    GovernanceOccurrenceV1::Lapsed
                } else {
                    GovernanceOccurrenceV1::Live
                })
            },
            |_, _, at| {
                assert_eq!(at.position, 0);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(pass.processed(), 1);
        assert_eq!(pass.remaining, 0);
    }

    #[test]
    fn owner_failure_poisoning_prevents_retry_and_preserves_the_original_error() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        assert_eq!(
            pass.run_segment(
                &mut block,
                GovernancePassSegmentV1::G1,
                vec![GovernancePassItemV1::HeightProgress],
                live,
                |_, _, _| Err(())
            ),
            Err(GovernancePassErrorV1::Owner(()))
        );
        assert_eq!(pass.processed(), 0);
        assert_eq!(
            pass.run_segment(&mut block, GovernancePassSegmentV1::G1, vec![], live, noop),
            Err(GovernancePassErrorV1::Segment)
        );
    }

    #[test]
    fn source_substitution_and_duplicate_order_keys_are_refused_before_callbacks() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let other = blank_state();
        let mut foreign = other.block(header(2));
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        assert_eq!(
            pass.run_segment(
                &mut foreign,
                GovernancePassSegmentV1::G1,
                vec![],
                live,
                noop
            ),
            Err(GovernancePassErrorV1::Source)
        );
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        assert_eq!(
            pass.run_segment(
                &mut block,
                GovernancePassSegmentV1::G1,
                vec![GovernancePassItemV1::HeightProgress; 2],
                live,
                noop
            ),
            Err(GovernancePassErrorV1::Items)
        );
    }

    #[test]
    fn full_casting_capacity_stops_openings_but_keeps_enactment_budget() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        for segment in [
            GovernancePassSegmentV1::G1,
            GovernancePassSegmentV1::G2a,
            GovernancePassSegmentV1::G2b,
        ] {
            pass.run_segment(&mut block, segment, vec![], live, noop)
                .unwrap();
        }
        pass.run_segment::<()>(
            &mut block,
            GovernancePassSegmentV1::G2c,
            vec![GovernancePassItemV1::Open {
                due_ms: 0,
                attempt: attempt(1),
                body: ParliamentBody::PolicyJury,
            }],
            |_, _| Ok(GovernanceOccurrenceV1::NoCastingSlot),
            |_, _, _| panic!("a full casting set cannot open"),
        )
        .unwrap();
        pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G3,
            vec![GovernancePassItemV1::EnactParliament {
                due_ms: 0,
                attempt: attempt(1),
            }],
            live,
            noop,
        )
        .unwrap();
        assert_eq!(pass.processed(), 1);
        assert_eq!(pass.remaining, 60);
    }

    #[test]
    fn g2a_executes_the_union_by_due_time_with_shared_positions() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G1,
            vec![GovernancePassItemV1::HeightProgress],
            live,
            noop,
        )
        .unwrap();
        let old_reconciliation = GovernancePassItemV1::Reconciliation {
            due_ms: 1,
            network: SccpNetworkV1::EthereumMainnet,
            revision: 1,
        };
        let same_time_pause = GovernancePassItemV1::PauseConclusion {
            due_ms: 2,
            network: SccpNetworkV1::EthereumMainnet,
            role: SccpPanelRoleV1::Primary,
        };
        let mut seen = Vec::new();
        pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G2a,
            vec![same_time_pause, lifecycle(2, 1), old_reconciliation],
            live,
            |_, item, anchor| {
                seen.push((item, anchor.position));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            seen,
            vec![
                (old_reconciliation, 1),
                (lifecycle(2, 1), 2),
                (same_time_pause, 3)
            ]
        );
        assert_eq!(pass.remaining, 60);
    }

    #[test]
    fn future_work_wrong_segments_and_reordering_never_reach_an_owner() {
        let state = blank_state();
        let mut block = state.block(header(2));
        for (segment, items, expected) in [
            (
                GovernancePassSegmentV1::G2a,
                vec![],
                GovernancePassErrorV1::Segment,
            ),
            (
                GovernancePassSegmentV1::G1,
                vec![lifecycle(0, 1)],
                GovernancePassErrorV1::Items,
            ),
            (
                GovernancePassSegmentV1::G1,
                vec![GovernancePassItemV1::Draw {
                    slot: 3,
                    attempt: attempt(1),
                }],
                GovernancePassErrorV1::Items,
            ),
        ] {
            let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
            assert_eq!(
                pass.run_segment(
                    &mut block,
                    segment,
                    items,
                    |_, _| panic!("invalid schedule must not inspect State"),
                    noop
                ),
                Err(expected)
            );
            assert_eq!(
                pass.run_segment(&mut block, GovernancePassSegmentV1::G1, vec![], live, noop),
                Err(GovernancePassErrorV1::Segment)
            );
        }
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        pass.run_segment(&mut block, GovernancePassSegmentV1::G1, vec![], live, noop)
            .unwrap();
        assert_eq!(
            pass.run_segment(
                &mut block,
                GovernancePassSegmentV1::G2a,
                vec![lifecycle(u64::MAX, 1)],
                live,
                noop
            ),
            Err(GovernancePassErrorV1::Items)
        );
    }

    #[test]
    fn actual_reducer_consumes_only_the_segment_snapshot() {
        use super::super::test_driver::{CREATED_HEIGHT, CREATED_MS, Driver, WINDOW_MS, network};
        use iroha_data_model::governance::types::RiskTierV1;
        let state = blank_state();
        let mut block_header = header(CREATED_HEIGHT + 1);
        block_header.creation_time_ms = CREATED_MS + 1;
        let mut block = state.block(block_header);
        let mut driver = Driver::new(RiskTierV1::Standard, 3);
        let snapshot = driver
            .state
            .due_lifecycle_items(CREATED_MS + 1)
            .unwrap()
            .into_iter()
            .map(GovernancePassItemV1::Lifecycle)
            .collect();
        let mut pass =
            SharedGovernancePassV1::begin::<super::super::AnonymousParliamentErrorV1>(&block)
                .unwrap();
        pass.run_segment::<super::super::AnonymousParliamentErrorV1>(
            &mut block,
            GovernancePassSegmentV1::G1,
            vec![],
            |_, _| Ok(GovernanceOccurrenceV1::Live),
            |_, _, _| Ok(()),
        )
        .unwrap();
        pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G2a,
            snapshot,
            |_, _| Ok(GovernanceOccurrenceV1::Live),
            |_, item, anchor| {
                let GovernancePassItemV1::Lifecycle(due) = item else {
                    panic!("lifecycle snapshot")
                };
                driver
                    .state
                    .apply_item_with_credentials(&network(), due.item, anchor, |_, _| Vec::new())
                    .map(|_| ())
            },
        )
        .unwrap();
        assert_eq!(pass.processed(), 1);
        assert_eq!(
            driver.state.qualification_due_ms(),
            Some(CREATED_MS + 1 + WINDOW_MS)
        );
        assert!(
            driver
                .state
                .due_lifecycle_items(CREATED_MS + 1)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            driver
                .state
                .due_lifecycle_items(CREATED_MS + 1 + WINDOW_MS)
                .unwrap()[0]
                .item,
            AnonymousLifecycleItemV1::EndQualification
        );
    }

    #[test]
    fn committed_output_capacity_defers_the_item_and_every_later_segment() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut pass = SharedGovernancePassV1::begin::<()>(&block).unwrap();
        pass.run_segment(
            &mut block,
            GovernancePassSegmentV1::G1,
            vec![GovernancePassItemV1::HeightProgress],
            live,
            noop,
        )
        .unwrap();
        let mut inspected = Vec::new();
        pass.run_segment::<()>(
            &mut block,
            GovernancePassSegmentV1::G2a,
            vec![lifecycle(1, 1), lifecycle(2, 2)],
            |_, item| {
                inspected.push(item);
                Ok(GovernanceOccurrenceV1::NoOutputCapacity)
            },
            |_, _, _| panic!("unfunded output must not mutate State"),
        )
        .unwrap();
        assert_eq!(inspected, vec![lifecycle(1, 1)]);
        assert_eq!(pass.processed(), 1);
        assert_eq!(pass.remaining, 63);
        for (segment, item) in [
            (
                GovernancePassSegmentV1::G2b,
                GovernancePassItemV1::Close {
                    due_ms: 0,
                    attempt: attempt(1),
                    body: ParliamentBody::PolicyJury,
                },
            ),
            (
                GovernancePassSegmentV1::G2c,
                GovernancePassItemV1::Open {
                    due_ms: 0,
                    attempt: attempt(2),
                    body: ParliamentBody::PolicyJury,
                },
            ),
            (
                GovernancePassSegmentV1::G3,
                GovernancePassItemV1::EnactParliament {
                    due_ms: 0,
                    attempt: attempt(3),
                },
            ),
        ] {
            pass.run_segment(
                &mut block,
                segment,
                vec![item],
                |_, _| panic!("capacity deferral blocks all later work"),
                noop,
            )
            .unwrap();
        }
        assert_eq!(pass.processed(), 1);
        assert_eq!(pass.remaining, 63);
    }
}
