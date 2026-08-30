#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OuterIngressTurn {
    Completion,
    Runtime,
    Ingress,
}

/// Closed outer-runner target named by one lifecycle rank observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum LifecycleRunnerRankTarget {
    /// The next effect/I/O completion service turn.
    Completion,
    /// The next serialized reducer-runtime service turn.
    Runtime,
    /// The next authenticated fair-ingress service turn.
    Ingress,
}

impl LifecycleRunnerRankTarget {
    #[cfg(test)]
    const fn turn(self) -> OuterIngressTurn {
        match self {
            Self::Completion => OuterIngressTurn::Completion,
            Self::Runtime => OuterIngressTurn::Runtime,
            Self::Ingress => OuterIngressTurn::Ingress,
        }
    }
}

impl From<OuterIngressTurn> for LifecycleRunnerRankTarget {
    fn from(turn: OuterIngressTurn) -> Self {
        match turn {
            OuterIngressTurn::Completion => Self::Completion,
            OuterIngressTurn::Runtime => Self::Runtime,
            OuterIngressTurn::Ingress => Self::Ingress,
        }
    }
}

/// Borrow-bound proof of the outer runner cursor's exact current turn.
///
/// Construction is private to [`OuterIngressTurns::next_current`]. While this
/// value exists, its mutable cursor borrow prevents another turn from being
/// observed or advanced. Dropping it advances exactly the represented turn,
/// so a retained same-context value can never be reused after the live cursor
/// moves.
#[derive(Debug)]
#[must_use = "the current runner turn must be serviced before the cursor advances"]
pub(crate) struct LifecycleCurrentRunnerTurn<'cursor> {
    cursor: &'cursor mut OuterIngressTurns,
    turn: OuterIngressTurn,
}

impl LifecycleCurrentRunnerTurn<'_> {
    /// Frozen height-context identity owned by the borrowed cursor.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.cursor.context_id
    }

    /// Frozen height owned by the borrowed cursor.
    pub(crate) const fn height(&self) -> wire::Height {
        self.cursor.height
    }

    /// Exact current outer-runner target.
    pub(crate) fn target(&self) -> LifecycleRunnerRankTarget {
        self.turn.into()
    }

    /// Current-turn reach debt. A borrow can represent only the turn presently
    /// at the cursor, so its debt is necessarily zero.
    pub(crate) const fn debt(&self) -> u64 {
        0
    }

    #[cfg(test)]
    const fn turn(&self) -> OuterIngressTurn {
        self.turn
    }
}

impl Drop for LifecycleCurrentRunnerTurn<'_> {
    fn drop(&mut self) {
        self.cursor.advance_current(self.turn);
    }
}

/// Test-only runner reach-debt observation for one outer turn.
///
/// Production cannot mint or consume this free-standing shape; its planner
/// accepts only [`LifecycleCurrentRunnerTurn`].
#[derive(Debug, PartialEq, Eq)]
#[must_use = "the runner observation must be consumed by the composite planner snapshot"]
#[cfg(test)]
pub(crate) struct LifecycleRunnerRankSnapshot {
    context_id: wire::HeightContextId,
    height: wire::Height,
    target: LifecycleRunnerRankTarget,
    debt: u64,
    _linearity: LifecycleRunnerRankSnapshotLinearity,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct LifecycleRunnerRankSnapshotLinearity;

#[cfg(test)]
impl Drop for LifecycleRunnerRankSnapshotLinearity {
    fn drop(&mut self) {}
}

#[cfg(test)]
impl LifecycleRunnerRankSnapshot {
    /// Frozen height-context identity owning this cursor observation.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }

    /// Frozen height owning this cursor observation.
    pub(crate) const fn height(&self) -> wire::Height {
        self.height
    }

    /// Closed outer turn whose reach was measured.
    pub(crate) const fn target(&self) -> LifecycleRunnerRankTarget {
        self.target
    }

    /// Number of cursor turns strictly before the target.
    pub(crate) const fn debt(&self) -> u64 {
        self.debt
    }
}

/// Move-only cursor for the exact outer Completion/Runtime/Ingress cycle.
///
/// Reifying the cursor preserves the existing iterator order while giving the
/// guarded lifecycle planner a real runner-reach debt instead of a
/// caller-supplied zero. It remains private and never mints SchedulerInputs by
/// itself. Ordinary Ingress selection transfers the live borrow into its
/// opaque owner transaction, which releases the cursor only after the
/// owner-to-worker handoff has been consumed.
#[derive(Debug)]
struct OuterIngressTurns {
    context_id: wire::HeightContextId,
    height: wire::Height,
    cycles_remaining: usize,
    next_turn: OuterIngressTurn,
}

impl OuterIngressTurns {
    fn new(limit: usize, context_id: wire::HeightContextId, height: wire::Height) -> Self {
        Self {
            context_id,
            height,
            cycles_remaining: limit.max(1),
            next_turn: OuterIngressTurn::Completion,
        }
    }

    #[cfg(test)]
    fn reach_debt(&self, target: OuterIngressTurn) -> Option<u64> {
        if self.cycles_remaining == 0 {
            return None;
        }
        let next = outer_ingress_turn_index(self.next_turn);
        let target = outer_ingress_turn_index(target);
        if target >= next {
            return Some(u64::from(target - next));
        }
        (self.cycles_remaining > 1).then(|| u64::from(3 - next + target))
    }

    #[cfg(test)]
    fn lifecycle_rank_snapshot(
        &self,
        target: LifecycleRunnerRankTarget,
    ) -> Option<LifecycleRunnerRankSnapshot> {
        Some(LifecycleRunnerRankSnapshot {
            context_id: self.context_id,
            height: self.height,
            target,
            debt: self.reach_debt(target.turn())?,
            _linearity: LifecycleRunnerRankSnapshotLinearity,
        })
    }

    /// Borrow the exact current turn without advancing the cursor early.
    fn next_current(&mut self) -> Option<LifecycleCurrentRunnerTurn<'_>> {
        if self.cycles_remaining == 0 {
            return None;
        }
        Some(LifecycleCurrentRunnerTurn {
            turn: self.next_turn,
            cursor: self,
        })
    }

    fn advance_current(&mut self, turn: OuterIngressTurn) {
        assert_eq!(
            self.next_turn, turn,
            "borrow-bound outer runner turn must remain current until drop"
        );
        self.next_turn = match turn {
            OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
            OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,
            OuterIngressTurn::Ingress => {
                self.cycles_remaining -= 1;
                OuterIngressTurn::Completion
            }
        };
    }
}

/// Mint the exact Ingress reach observation after Completion and Runtime for
/// the production-owner cross-module transaction regression.
#[cfg(test)]
pub(in crate::sumeragi) fn lifecycle_ingress_rank_snapshot_for_test(
    context: &wire::HeightContext,
) -> LifecycleRunnerRankSnapshot {
    let mut turns = OuterIngressTurns::new(1, context.id(), context.height);
    {
        let turn = turns
            .next_current()
            .expect("the outer cursor starts at Completion");
        assert_eq!(turn.turn(), OuterIngressTurn::Completion);
    }
    {
        let turn = turns
            .next_current()
            .expect("the outer cursor continues at Runtime");
        assert_eq!(turn.turn(), OuterIngressTurn::Runtime);
    }
    let turn = turns
        .next_current()
        .expect("the current outer cursor owns its immediate Ingress turn");
    assert_eq!(turn.turn(), OuterIngressTurn::Ingress);
    LifecycleRunnerRankSnapshot {
        context_id: turn.context_id(),
        height: turn.height(),
        target: turn.target(),
        debt: turn.debt(),
        _linearity: LifecycleRunnerRankSnapshotLinearity,
    }
}

#[cfg(test)]
const fn outer_ingress_turn_index(turn: OuterIngressTurn) -> u8 {
    match turn {
        OuterIngressTurn::Completion => 0,
        OuterIngressTurn::Runtime => 1,
        OuterIngressTurn::Ingress => 2,
    }
}

fn outer_ingress_turns(
    limit: usize,
    context_id: wire::HeightContextId,
    height: wire::Height,
) -> OuterIngressTurns {
    OuterIngressTurns::new(limit, context_id, height)
}
