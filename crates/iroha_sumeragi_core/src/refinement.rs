//! Executable refinement gate shared by production and Verus.
//!
//! The reducer projects each candidate transition into [`TransitionFacts`]
//! and must pass this gate before replacing its caller-visible state.  The
//! boolean expression is defined by a macro because the exact same executable
//! expression is instantiated in `verus_proofs.rs`; normal builds therefore
//! do not need to link `vstd`.

/// Maximum number of effects one reducer input can emit.
///
/// Retransmission is the largest branch: the seven canonical control-message
/// classes followed by at most one body-fetch or apply effect.  A future ninth
/// effect fails closed at the refinement boundary until this limit is
/// deliberately revised and re-verified.
pub const MAX_EFFECTS_PER_STEP: usize = 8;

pub const EFFECT_NONE: u8 = 0;
pub const EFFECT_PERSIST: u8 = 1;
pub const EFFECT_FETCH: u8 = 2;
pub const EFFECT_STORE: u8 = 3;
pub const EFFECT_VALIDATE: u8 = 4;
pub const EFFECT_SIGN: u8 = 5;
pub const EFFECT_BROADCAST: u8 = 6;
pub const EFFECT_APPLY: u8 = 7;
pub const EFFECT_ENTER_VIEW: u8 = 8;
pub const EFFECT_REPORT: u8 = 9;

pub const EVENT_BODY_AVAILABLE: u8 = 8;
pub const EVENT_BODY_STORED: u8 = 9;
pub const EVENT_PERSISTED: u8 = 11;
pub const EVENT_SIGNED: u8 = 13;

pub const CONTINUATION_NONE: u8 = 0;
pub const CONTINUATION_SIGN: u8 = 1;
pub const CONTINUATION_INSTALL_TIMEOUT: u8 = 2;
pub const CONTINUATION_DECIDE: u8 = 3;

/// Caller-visible reducer action classes checked at the commit boundary.
///
/// WAL begin/acknowledgement actions carry the exact [`WAL_RECORD_*`] class;
/// every other action must carry [`WAL_RECORD_NONE`].  The split is deliberately
/// small: it classifies the exact atomicity boundary implemented by
/// `Reducer::step`, including production macro-steps that combine ingress,
/// certificate formation, and creation of one pending WAL append.
pub const ACTION_STUTTER: u8 = 0;
pub const ACTION_BEGIN_WAL: u8 = 1;
pub const ACTION_ACKNOWLEDGE_WAL: u8 = 2;
pub const ACTION_BODY_PROGRESS: u8 = 3;
pub const ACTION_VOLATILE_PROTOCOL: u8 = 4;
pub const ACTION_COMPLETE_APPLICATION: u8 = 5;

/// No WAL record participates in this reducer action.
pub const WAL_RECORD_NONE: u8 = 0;
/// `WalRecord::ProposalIntent`.
pub const WAL_RECORD_PROPOSAL_INTENT: u8 = 1;
/// `WalRecord::PrepareIntent`.
pub const WAL_RECORD_PREPARE_INTENT: u8 = 2;
/// `WalRecord::ObservePrepare`.
pub const WAL_RECORD_OBSERVE_PREPARE: u8 = 3;
/// `WalRecord::LockAndCommit`.
pub const WAL_RECORD_LOCK_AND_COMMIT: u8 = 4;
/// `WalRecord::TimeoutIntent`.
pub const WAL_RECORD_TIMEOUT_INTENT: u8 = 5;
/// `WalRecord::InstallTimeout`.
pub const WAL_RECORD_INSTALL_TIMEOUT: u8 = 6;
/// `WalRecord::Decision`.
pub const WAL_RECORD_DECISION: u8 = 7;

/// No successfully completed signature participates in this action.
pub const SIGNED_MESSAGE_NONE: u8 = 0;
/// Completion of a proposal signature.
pub const SIGNED_MESSAGE_PROPOSAL: u8 = 1;
/// Completion of a Prepare-vote signature.
pub const SIGNED_MESSAGE_PREPARE: u8 = 2;
/// Completion of a Commit-vote signature.
pub const SIGNED_MESSAGE_COMMIT: u8 = 3;
/// Completion of a timeout-vote signature.
pub const SIGNED_MESSAGE_TIMEOUT: u8 = 4;

/// Exact cardinality projection of the reducer's volatile collections.
///
/// This summary is not a replacement for verifying the collection extraction
/// itself.  It makes the production gate fail closed if an accepted candidate
/// exceeds a protocol-derived bound, and gives Verus the same fixed-width
/// state shape used by the executable check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VolatileSummary {
    pub(crate) candidate_present: bool,
    pub(crate) body_work: u64,
    pub(crate) pending_prepare: u64,
    pub(crate) known_prepare: u64,
    pub(crate) vote_pools: u64,
    pub(crate) vote_entries: u64,
    pub(crate) timeout_vote_pools: u64,
    pub(crate) timeout_vote_entries: u64,
    pub(crate) formed_certificates: u64,
    pub(crate) formed_timeouts: u64,
    pub(crate) outbound_control: u64,
    pub(crate) signature_queue: u64,
    pub(crate) awaiting_signature: bool,
    /// Upper bound derived from durable proposal/Prepare/Commit/timeout
    /// intents that may be resumed for signing.
    pub(crate) durable_signable_limit: u64,
    pub(crate) replay_resumed: bool,
}

/// Fixed, exact projection of one reducer effect vector.
///
/// `kindN` is the effect discriminant at vector index `N`; `authorizedN` is
/// computed against the concrete pre-state, event, and candidate post-state.
/// Slots at or beyond `len` must be canonical zeroes.  A fixed representation
/// keeps the executable checker small and makes complete vector order visible
/// to Verus without trusting an iterator implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct EffectTrace {
    pub(crate) len: u8,
    pub(crate) kind0: u8,
    pub(crate) authorized0: bool,
    pub(crate) kind1: u8,
    pub(crate) authorized1: bool,
    pub(crate) kind2: u8,
    pub(crate) authorized2: bool,
    pub(crate) kind3: u8,
    pub(crate) authorized3: bool,
    pub(crate) kind4: u8,
    pub(crate) authorized4: bool,
    pub(crate) kind5: u8,
    pub(crate) authorized5: bool,
    pub(crate) kind6: u8,
    pub(crate) authorized6: bool,
    pub(crate) kind7: u8,
    pub(crate) authorized7: bool,
}

impl EffectTrace {
    /// Construct an empty canonical trace.
    pub(crate) const fn empty() -> Self {
        Self {
            len: 0,
            kind0: EFFECT_NONE,
            authorized0: false,
            kind1: EFFECT_NONE,
            authorized1: false,
            kind2: EFFECT_NONE,
            authorized2: false,
            kind3: EFFECT_NONE,
            authorized3: false,
            kind4: EFFECT_NONE,
            authorized4: false,
            kind5: EFFECT_NONE,
            authorized5: false,
            kind6: EFFECT_NONE,
            authorized6: false,
            kind7: EFFECT_NONE,
            authorized7: false,
        }
    }

    /// Append one exact effect projection.
    pub(crate) fn push(&mut self, kind: u8, authorized: bool) -> bool {
        let index = usize::from(self.len);
        if index >= MAX_EFFECTS_PER_STEP {
            return false;
        }
        match index {
            0 => {
                self.kind0 = kind;
                self.authorized0 = authorized;
            }
            1 => {
                self.kind1 = kind;
                self.authorized1 = authorized;
            }
            2 => {
                self.kind2 = kind;
                self.authorized2 = authorized;
            }
            3 => {
                self.kind3 = kind;
                self.authorized3 = authorized;
            }
            4 => {
                self.kind4 = kind;
                self.authorized4 = authorized;
            }
            5 => {
                self.kind5 = kind;
                self.authorized5 = authorized;
            }
            6 => {
                self.kind6 = kind;
                self.authorized6 = authorized;
            }
            7 => {
                self.kind7 = kind;
                self.authorized7 = authorized;
            }
            _ => return false,
        }
        self.len += 1;
        true
    }
}

/// Concrete facts extracted from one attempted production reducer step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct TransitionFacts {
    pub(crate) before_invariant: bool,
    pub(crate) after_invariant: bool,
    pub(crate) context_unchanged: bool,
    pub(crate) tag_matches: bool,
    pub(crate) busy_fence_open: bool,
    pub(crate) event_kind: u8,
    pub(crate) action_kind: u8,
    pub(crate) wal_record_kind: u8,
    pub(crate) signed_message_kind: u8,
    pub(crate) validator_count: u64,
    pub(crate) volatile_before: VolatileSummary,
    pub(crate) volatile_after: VolatileSummary,
    pub(crate) durable_unchanged: bool,
    pub(crate) pending_unchanged: bool,
    pub(crate) generation_unchanged: bool,
    pub(crate) application_unchanged: bool,
    pub(crate) begin_persist_exact: bool,
    pub(crate) acknowledge_persist_exact: bool,
    pub(crate) application_transition_exact: bool,
    pub(crate) acknowledgement_continuation: u8,
    pub(crate) effects: EffectTrace,
}

// Keep this expression free of calls into production code.  It is expanded
// both below and inside `verus!`, so Verus proves the actual decision logic
// used by the production reducer rather than a separately transcribed checker.
macro_rules! effect_count_body {
    ($trace:expr, $kind:expr) => {{
        (if $trace.len > 0 && $trace.kind0 == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 1 && $trace.kind1 == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 2 && $trace.kind2 == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 3 && $trace.kind3 == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 4 && $trace.kind4 == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 5 && $trace.kind5 == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 6 && $trace.kind6 == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 7 && $trace.kind7 == $kind {
            1u64
        } else {
            0u64
        })
    }};
}

macro_rules! active_effect_slot_body {
    ($kind:expr, $authorized:expr) => {{ $kind >= 1u8 && $kind <= 9u8 && $authorized }};
}

macro_rules! inactive_effect_slot_body {
    ($kind:expr, $authorized:expr) => {{ $kind == 0u8 && !$authorized }};
}

macro_rules! effect_slots_authorized_body {
    ($trace:expr) => {{
        $trace.len <= 8u8
            && (if $trace.len > 0 {
                active_effect_slot_body!($trace.kind0, $trace.authorized0)
            } else {
                inactive_effect_slot_body!($trace.kind0, $trace.authorized0)
            })
            && (if $trace.len > 1 {
                active_effect_slot_body!($trace.kind1, $trace.authorized1)
            } else {
                inactive_effect_slot_body!($trace.kind1, $trace.authorized1)
            })
            && (if $trace.len > 2 {
                active_effect_slot_body!($trace.kind2, $trace.authorized2)
            } else {
                inactive_effect_slot_body!($trace.kind2, $trace.authorized2)
            })
            && (if $trace.len > 3 {
                active_effect_slot_body!($trace.kind3, $trace.authorized3)
            } else {
                inactive_effect_slot_body!($trace.kind3, $trace.authorized3)
            })
            && (if $trace.len > 4 {
                active_effect_slot_body!($trace.kind4, $trace.authorized4)
            } else {
                inactive_effect_slot_body!($trace.kind4, $trace.authorized4)
            })
            && (if $trace.len > 5 {
                active_effect_slot_body!($trace.kind5, $trace.authorized5)
            } else {
                inactive_effect_slot_body!($trace.kind5, $trace.authorized5)
            })
            && (if $trace.len > 6 {
                active_effect_slot_body!($trace.kind6, $trace.authorized6)
            } else {
                inactive_effect_slot_body!($trace.kind6, $trace.authorized6)
            })
            && (if $trace.len > 7 {
                active_effect_slot_body!($trace.kind7, $trace.authorized7)
            } else {
                inactive_effect_slot_body!($trace.kind7, $trace.authorized7)
            })
    }};
}

macro_rules! effect_order_constraints_body {
    (
        $trace:expr,
        $event_kind:expr,
        $persist_count:expr,
        $fetch_count:expr,
        $store_count:expr,
        $validate_count:expr,
        $sign_count:expr,
        $apply_count:expr,
        $enter_count:expr $(,)?
    ) => {{
        $persist_count <= 1u64
            && $store_count <= 1u64
            && $validate_count <= 1u64
            && $sign_count <= 1u64
            && $apply_count <= 1u64
            && $enter_count <= 1u64
            // A signing request is always the final effect.  This makes any
            // preceding view-entry, decision handling, fetch, or broadcast
            // observable before the next asynchronous signing completion.
            && ($sign_count == 0u64
                || match $trace.len {
                    1 => $trace.kind0 == 5u8,
                    2 => $trace.kind1 == 5u8,
                    3 => $trace.kind2 == 5u8,
                    4 => $trace.kind3 == 5u8,
                    5 => $trace.kind4 == 5u8,
                    6 => $trace.kind5 == 5u8,
                    7 => $trace.kind6 == 5u8,
                    8 => $trace.kind7 == 5u8,
                    _ => false,
                })
            // A persistence request is a fence: the same transition cannot
            // sign, apply, enter a view, or advance the body pipeline.
            && ($persist_count == 0u64
                || ($sign_count == 0u64
                    && $apply_count == 0u64
                    && $enter_count == 0u64
                    && $store_count == 0u64
                    && $validate_count == 0u64))
            // Body storage and validation acknowledgements are serialized
            // one-effect transitions in their exact pipeline order.
            && ($store_count == 0u64
                || ($trace.len == 1u8 && $event_kind == 8u8))
            && ($validate_count == 0u64
                || ($trace.len == 1u8 && $event_kind == 9u8))
            && ($enter_count == 0u64
                || ($persist_count == 0u64
                    && $apply_count == 0u64
                    && $store_count == 0u64
                    && $validate_count == 0u64))
            && ($apply_count == 0u64
                || ($persist_count == 0u64
                    && $enter_count == 0u64
                    && $store_count == 0u64
                    && $validate_count == 0u64))
            // `fetch_count` is intentionally not bounded: retransmission may
            // evolve to request several equivalent certified sources, but all
            // exact slots still require concrete authorization above.
            && $fetch_count <= 8u64
    }};
}

macro_rules! effect_ordering_gate_body {
    ($trace:expr, $event_kind:expr, $count_gate:ident, $constraints_gate:ident $(,)?) => {{
        let persist_count = $count_gate($trace, 1u8);
        let fetch_count = $count_gate($trace, 2u8);
        let store_count = $count_gate($trace, 3u8);
        let validate_count = $count_gate($trace, 4u8);
        let sign_count = $count_gate($trace, 5u8);
        let apply_count = $count_gate($trace, 7u8);
        let enter_count = $count_gate($trace, 8u8);
        $constraints_gate(
            $trace,
            $event_kind,
            persist_count,
            fetch_count,
            store_count,
            validate_count,
            sign_count,
            apply_count,
            enter_count,
        )
    }};
}

macro_rules! effect_trace_gate_body {
    ($trace:expr, $event_kind:expr, $slot_gate:ident, $order_gate:ident $(,)?) => {{ $slot_gate($trace) && $order_gate($trace, $event_kind) }};
}

macro_rules! volatile_summary_well_formed_body {
    ($summary:expr, $validator_count:expr) => {{
        $validator_count > 0u64
            && $validator_count <= u64::MAX / 2u64
            // Only Prepare and Commit pools for the current view are kept.
            && $summary.vote_pools <= 2u64
            && $summary.vote_entries >= $summary.vote_pools
            && $summary.vote_entries <= $validator_count * 2u64
            // Exactly one current-view timeout pool can exist.
            && $summary.timeout_vote_pools <= 1u64
            && $summary.timeout_vote_entries >= $summary.timeout_vote_pools
            && $summary.timeout_vote_entries <= $validator_count
            // At most one locally formed certificate per phase and one TC.
            && $summary.formed_certificates <= 2u64
            && $summary.formed_timeouts <= 1u64
            // `OutboundControlClass` has seven exhaustive variants.
            && $summary.outbound_control <= 7u64
            // Every pending PrepareQC is also known.  Recovery and a view
            // reset may additionally retain highest and locked (at most two).
            && $summary.pending_prepare <= $summary.known_prepare
            && $summary.known_prepare - $summary.pending_prepare <= 2u64
            // Body work is sourced by a candidate, a pending certified body,
            // or the sole durable decision.  Two spare identities cover the
            // candidate/decision cases without trusting subject equality.
            && ($summary.body_work <= $summary.pending_prepare
                || $summary.body_work - $summary.pending_prepare <= 2u64)
            // The FIFO plus its sole in-flight element is bounded by durable
            // intents eligible for replay; no unsigned item may be invented.
            && $summary.signature_queue <= $summary.durable_signable_limit
            && (!$summary.awaiting_signature
                || $summary.signature_queue < $summary.durable_signable_limit)
    }};
}

macro_rules! volatile_summaries_equal_body {
    ($before:expr, $after:expr) => {{
        $before.candidate_present == $after.candidate_present
            && $before.body_work == $after.body_work
            && $before.pending_prepare == $after.pending_prepare
            && $before.known_prepare == $after.known_prepare
            && $before.vote_pools == $after.vote_pools
            && $before.vote_entries == $after.vote_entries
            && $before.timeout_vote_pools == $after.timeout_vote_pools
            && $before.timeout_vote_entries == $after.timeout_vote_entries
            && $before.formed_certificates == $after.formed_certificates
            && $before.formed_timeouts == $after.formed_timeouts
            && $before.outbound_control == $after.outbound_control
            && $before.signature_queue == $after.signature_queue
            && $before.awaiting_signature == $after.awaiting_signature
            && $before.durable_signable_limit == $after.durable_signable_limit
            && $before.replay_resumed == $after.replay_resumed
    }};
}

macro_rules! acknowledgement_record_matches_body {
    ($record_kind:expr, $continuation:expr) => {{
        match $continuation {
            0 => $record_kind == 3u8,
            1 => matches!($record_kind, 1u8 | 2u8 | 4u8 | 5u8),
            2 => $record_kind == 6u8,
            3 => $record_kind == 7u8,
            _ => false,
        }
    }};
}

macro_rules! signed_message_class_body {
    ($facts:expr) => {{
        ($facts.signed_message_kind == 0u8
            && ($facts.event_kind != 13u8 || $facts.action_kind == 0u8))
            || ($facts.event_kind == 13u8
                && $facts.action_kind != 0u8
                && $facts.signed_message_kind >= 1u8
                && $facts.signed_message_kind <= 4u8)
    }};
}

macro_rules! stutter_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && volatile_summaries_equal_body!($facts.volatile_before, $facts.volatile_after)
            && $facts.effects.len == 0u8
    }};
}

macro_rules! begin_wal_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind >= 1u8
            && $facts.wal_record_kind <= 7u8
            && $facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
    }};
}

macro_rules! acknowledge_wal_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind >= 1u8
            && $facts.wal_record_kind <= 7u8
            && !$facts.begin_persist_exact
            && $facts.acknowledge_persist_exact
            && $facts.event_kind == 11u8
            && acknowledgement_record_matches_body!(
                $facts.wal_record_kind,
                $facts.acknowledgement_continuation
            )
            && effect_count_body!($facts.effects, 1u8) == 0u64
            && effect_count_body!($facts.effects, 3u8) == 0u64
            && effect_count_body!($facts.effects, 4u8) == 0u64
    }};
}

macro_rules! validation_completed_action_body {
    ($facts:expr, $count_gate:ident $(,)?) => {{
        $count_gate($facts.effects, 1u8) == 0u64
            && $count_gate($facts.effects, 2u8) == 0u64
            && $count_gate($facts.effects, 3u8) == 0u64
            && $count_gate($facts.effects, 4u8) == 0u64
            && $count_gate($facts.effects, 5u8) == 0u64
            && $count_gate($facts.effects, 6u8) == 0u64
            && $count_gate($facts.effects, 8u8) == 0u64
    }};
}

macro_rules! body_progress_action_body {
    ($facts:expr, $validation_gate:ident $(,)?) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.event_kind >= 8u8
            && $facts.event_kind <= 10u8
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && (match $facts.event_kind {
                8 => $facts.effects.len == 1u8 && $facts.effects.kind0 == 3u8,
                9 => $facts.effects.len == 1u8 && $facts.effects.kind0 == 4u8,
                10 => $validation_gate($facts),
                _ => false,
            })
    }};
}

macro_rules! volatile_protocol_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && effect_count_body!($facts.effects, 1u8) == 0u64
            && effect_count_body!($facts.effects, 3u8) == 0u64
            && effect_count_body!($facts.effects, 4u8) == 0u64
            && effect_count_body!($facts.effects, 8u8) == 0u64
    }};
}

macro_rules! complete_application_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.event_kind == 14u8
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && !$facts.application_unchanged
            && $facts.application_transition_exact
            && $facts.effects.len == 0u8
    }};
}

macro_rules! action_kind_relation_body {
    (
        $facts:expr,
        $stutter_gate:ident,
        $begin_wal_gate:ident,
        $acknowledge_wal_gate:ident,
        $body_progress_gate:ident,
        $volatile_protocol_gate:ident,
        $complete_application_gate:ident $(,)?
    ) => {{
        match $facts.action_kind {
            0 => $stutter_gate($facts),
            1 => $begin_wal_gate($facts),
            2 => $acknowledge_wal_gate($facts),
            3 => $body_progress_gate($facts),
            4 => $volatile_protocol_gate($facts),
            5 => $complete_application_gate($facts),
            _ => false,
        }
    }};
}

macro_rules! production_action_relation_body {
    ($facts:expr, $signed_gate:ident, $action_gate:ident $(,)?) => {{ $signed_gate($facts) && $action_gate($facts) }};
}

macro_rules! transition_branch_constraints_body {
    (
        $facts:expr,
        $persist_count:expr,
        $fetch_count:expr,
        $sign_count:expr,
        $apply_count:expr,
        $enter_count:expr $(,)?
    ) => {{
        if !$facts.tag_matches || !$facts.busy_fence_open {
            $facts.durable_unchanged
                && $facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_unchanged
                && volatile_summaries_equal_body!($facts.volatile_before, $facts.volatile_after)
                && $facts.effects.len == 0u8
                && !$facts.begin_persist_exact
                && !$facts.acknowledge_persist_exact
        } else if $facts.begin_persist_exact {
            !$facts.acknowledge_persist_exact
                && $facts.durable_unchanged
                && !$facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_unchanged
                && $persist_count == 1u64
                && $sign_count == 0u64
                && $apply_count == 0u64
                && $enter_count == 0u64
        } else if $facts.acknowledge_persist_exact {
            !$facts.begin_persist_exact
                && !$facts.pending_unchanged
                && $facts.application_unchanged
                && $persist_count == 0u64
                && (match $facts.acknowledgement_continuation {
                    0 => {
                        $facts.generation_unchanged && $apply_count == 0u64 && $enter_count == 0u64
                    }
                    1 => {
                        $facts.generation_unchanged
                            && $sign_count == 1u64
                            && $apply_count == 0u64
                            && $enter_count == 0u64
                    }
                    2 => {
                        !$facts.generation_unchanged
                            && $enter_count == 1u64
                            && $apply_count == 0u64
                            && !$facts.volatile_after.candidate_present
                            && $facts.volatile_after.body_work == 0u64
                            && $facts.volatile_after.pending_prepare == 0u64
                            && $facts.volatile_after.vote_pools == 0u64
                            && $facts.volatile_after.vote_entries == 0u64
                            && $facts.volatile_after.timeout_vote_pools == 0u64
                            && $facts.volatile_after.timeout_vote_entries == 0u64
                            && $facts.volatile_after.formed_certificates == 0u64
                            && $facts.volatile_after.formed_timeouts == 0u64
                            && $facts.volatile_after.known_prepare <= 2u64
                            && $facts.volatile_after.outbound_control <= 3u64
                    }
                    3 => {
                        $facts.generation_unchanged
                            && $enter_count == 0u64
                            && (($apply_count == 1u64 && $fetch_count == 0u64)
                                || ($apply_count == 0u64 && $fetch_count == 1u64))
                    }
                    _ => false,
                })
        } else {
            $facts.durable_unchanged
                && $facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_transition_exact
                && $persist_count == 0u64
                && $enter_count == 0u64
                && ($sign_count == 0u64 || $facts.event_kind == 13u8)
                && ($apply_count == 0u64 || $facts.event_kind == 7u8 || $facts.event_kind == 10u8)
        }
    }};
}

macro_rules! transition_branch_gate_body {
    ($facts:expr, $count_gate:ident, $constraints_gate:ident $(,)?) => {{
        let persist_count = $count_gate($facts.effects, 1u8);
        let fetch_count = $count_gate($facts.effects, 2u8);
        let sign_count = $count_gate($facts.effects, 5u8);
        let apply_count = $count_gate($facts.effects, 7u8);
        let enter_count = $count_gate($facts.effects, 8u8);
        $constraints_gate(
            $facts,
            persist_count,
            fetch_count,
            sign_count,
            apply_count,
            enter_count,
        )
    }};
}

macro_rules! production_transition_gate_body {
    (
        $facts:expr,
        $volatile_gate:ident,
        $action_gate:ident,
        $trace_gate:ident,
        $branch_gate:ident $(,)?
    ) => {{
        $facts.before_invariant
            && $facts.after_invariant
            && $facts.context_unchanged
            && $volatile_gate($facts.volatile_before, $facts.validator_count)
            && $volatile_gate($facts.volatile_after, $facts.validator_count)
            && $action_gate($facts)
            && $trace_gate($facts.effects, $facts.event_kind)
            && $branch_gate($facts)
    }};
}

fn volatile_summary_is_well_formed(summary: VolatileSummary, validator_count: u64) -> bool {
    volatile_summary_well_formed_body!(summary, validator_count)
}

fn signed_message_class_is_valid(facts: TransitionFacts) -> bool {
    signed_message_class_body!(facts)
}

fn stutter_action_is_valid(facts: TransitionFacts) -> bool {
    stutter_action_body!(facts)
}

fn begin_wal_action_is_valid(facts: TransitionFacts) -> bool {
    begin_wal_action_body!(facts)
}

fn acknowledge_wal_action_is_valid(facts: TransitionFacts) -> bool {
    acknowledge_wal_action_body!(facts)
}

fn validation_completed_action_is_valid(facts: TransitionFacts) -> bool {
    validation_completed_action_body!(facts, effect_count)
}

fn body_progress_action_is_valid(facts: TransitionFacts) -> bool {
    body_progress_action_body!(facts, validation_completed_action_is_valid)
}

fn volatile_protocol_action_is_valid(facts: TransitionFacts) -> bool {
    volatile_protocol_action_body!(facts)
}

fn complete_application_action_is_valid(facts: TransitionFacts) -> bool {
    complete_application_action_body!(facts)
}

fn action_kind_is_valid(facts: TransitionFacts) -> bool {
    action_kind_relation_body!(
        facts,
        stutter_action_is_valid,
        begin_wal_action_is_valid,
        acknowledge_wal_action_is_valid,
        body_progress_action_is_valid,
        volatile_protocol_action_is_valid,
        complete_application_action_is_valid,
    )
}

fn named_action_is_valid(facts: TransitionFacts) -> bool {
    production_action_relation_body!(facts, signed_message_class_is_valid, action_kind_is_valid,)
}

fn effect_slots_are_authorized(trace: EffectTrace) -> bool {
    effect_slots_authorized_body!(trace)
}

fn effect_count(trace: EffectTrace, kind: u8) -> u64 {
    effect_count_body!(trace, kind)
}

#[allow(clippy::too_many_arguments)]
fn effect_order_constraints(
    trace: EffectTrace,
    event_kind: u8,
    persist_count: u64,
    fetch_count: u64,
    store_count: u64,
    validate_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> bool {
    effect_order_constraints_body!(
        trace,
        event_kind,
        persist_count,
        fetch_count,
        store_count,
        validate_count,
        sign_count,
        apply_count,
        enter_count,
    )
}

fn effect_order_is_valid(trace: EffectTrace, event_kind: u8) -> bool {
    effect_ordering_gate_body!(trace, event_kind, effect_count, effect_order_constraints)
}

fn effect_trace_accepts(trace: EffectTrace, event_kind: u8) -> bool {
    effect_trace_gate_body!(
        trace,
        event_kind,
        effect_slots_are_authorized,
        effect_order_is_valid,
    )
}

#[allow(clippy::too_many_arguments)]
fn transition_branch_constraints(
    facts: TransitionFacts,
    persist_count: u64,
    fetch_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> bool {
    transition_branch_constraints_body!(
        facts,
        persist_count,
        fetch_count,
        sign_count,
        apply_count,
        enter_count,
    )
}

fn transition_branch_accepts(facts: TransitionFacts) -> bool {
    transition_branch_gate_body!(facts, effect_count, transition_branch_constraints)
}

/// Execute the transition relation used as the production commit gate.
#[must_use]
pub fn accepts(facts: TransitionFacts) -> bool {
    production_transition_gate_body!(
        facts,
        volatile_summary_is_well_formed,
        named_action_is_valid,
        effect_trace_accepts,
        transition_branch_accepts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_facts() -> TransitionFacts {
        let volatile = VolatileSummary {
            durable_signable_limit: 1,
            ..VolatileSummary::default()
        };
        TransitionFacts {
            before_invariant: true,
            after_invariant: true,
            context_unchanged: true,
            tag_matches: true,
            busy_fence_open: true,
            event_kind: 7,
            action_kind: ACTION_STUTTER,
            wal_record_kind: WAL_RECORD_NONE,
            signed_message_kind: SIGNED_MESSAGE_NONE,
            validator_count: 4,
            volatile_before: volatile,
            volatile_after: volatile,
            durable_unchanged: true,
            pending_unchanged: true,
            generation_unchanged: true,
            application_unchanged: true,
            begin_persist_exact: false,
            acknowledge_persist_exact: false,
            application_transition_exact: true,
            acknowledgement_continuation: CONTINUATION_NONE,
            effects: EffectTrace::empty(),
        }
    }

    #[test]
    fn stutter_and_exact_begin_are_accepted() {
        assert!(accepts(base_facts()));

        let mut facts = base_facts();
        facts.action_kind = ACTION_BEGIN_WAL;
        facts.wal_record_kind = WAL_RECORD_PROPOSAL_INTENT;
        facts.pending_unchanged = false;
        facts.begin_persist_exact = true;
        assert!(facts.effects.push(EFFECT_PERSIST, true));
        assert!(accepts(facts));
    }

    #[test]
    fn unauthorized_or_misordered_effects_fail_closed() {
        let mut unauthorized = base_facts();
        assert!(unauthorized.effects.push(EFFECT_BROADCAST, false));
        assert!(!accepts(unauthorized));

        let mut signing_not_last = base_facts();
        signing_not_last.event_kind = EVENT_SIGNED;
        assert!(signing_not_last.effects.push(EFFECT_SIGN, true));
        assert!(signing_not_last.effects.push(EFFECT_BROADCAST, true));
        assert!(!accepts(signing_not_last));

        let mut persist_and_sign = base_facts();
        persist_and_sign.action_kind = ACTION_BEGIN_WAL;
        persist_and_sign.wal_record_kind = WAL_RECORD_PROPOSAL_INTENT;
        persist_and_sign.pending_unchanged = false;
        persist_and_sign.begin_persist_exact = true;
        assert!(persist_and_sign.effects.push(EFFECT_PERSIST, true));
        assert!(persist_and_sign.effects.push(EFFECT_SIGN, true));
        assert!(!accepts(persist_and_sign));
    }

    #[test]
    fn stale_or_busy_input_must_be_an_exact_empty_stutter() {
        let mut stale = base_facts();
        stale.tag_matches = false;
        assert!(accepts(stale));

        stale.application_transition_exact = false;
        stale.application_unchanged = false;
        assert!(!accepts(stale));

        let mut busy = base_facts();
        busy.busy_fence_open = false;
        assert!(busy.effects.push(EFFECT_FETCH, true));
        assert!(!accepts(busy));
    }

    #[test]
    fn trace_capacity_is_fail_closed() {
        let mut trace = EffectTrace::empty();
        for _ in 0..MAX_EFFECTS_PER_STEP {
            assert!(trace.push(EFFECT_BROADCAST, true));
        }
        assert!(!trace.push(EFFECT_BROADCAST, true));
    }

    #[test]
    fn volatile_bounds_and_action_record_pairs_fail_closed() {
        let mut too_many_vote_pools = base_facts();
        too_many_vote_pools.volatile_after.vote_pools = 3;
        assert!(!accepts(too_many_vote_pools));

        let mut invented_signature = base_facts();
        invented_signature.volatile_before.awaiting_signature = true;
        invented_signature.volatile_after.awaiting_signature = true;
        invented_signature.volatile_before.durable_signable_limit = 0;
        invented_signature.volatile_after.durable_signable_limit = 0;
        assert!(!accepts(invented_signature));

        let mut bad_ack = base_facts();
        bad_ack.action_kind = ACTION_ACKNOWLEDGE_WAL;
        bad_ack.wal_record_kind = WAL_RECORD_DECISION;
        bad_ack.event_kind = EVENT_PERSISTED;
        bad_ack.pending_unchanged = false;
        bad_ack.acknowledge_persist_exact = true;
        bad_ack.acknowledgement_continuation = CONTINUATION_INSTALL_TIMEOUT;
        assert!(!accepts(bad_ack));
    }

    #[test]
    fn body_pipeline_classifier_rejects_non_pipeline_effects() {
        let mut stored = base_facts();
        stored.action_kind = ACTION_BODY_PROGRESS;
        stored.event_kind = EVENT_BODY_AVAILABLE;
        assert!(stored.effects.push(EFFECT_STORE, true));
        assert!(accepts(stored));

        let mut validated = base_facts();
        validated.action_kind = ACTION_BODY_PROGRESS;
        validated.event_kind = 10;
        assert!(validated.effects.push(EFFECT_REPORT, true));
        assert!(accepts(validated));

        let mut invented_broadcast = validated;
        invented_broadcast.effects = EffectTrace::empty();
        assert!(invented_broadcast.effects.push(EFFECT_BROADCAST, true));
        assert!(!accepts(invented_broadcast));

        let mut invented_fetch = validated;
        invented_fetch.effects = EffectTrace::empty();
        assert!(invented_fetch.effects.push(EFFECT_FETCH, true));
        assert!(!accepts(invented_fetch));
    }

    #[test]
    fn signed_classifier_and_inactive_slots_are_canonical() {
        let mut invented_signed_transition = base_facts();
        invented_signed_transition.event_kind = EVENT_SIGNED;
        invented_signed_transition.action_kind = ACTION_VOLATILE_PROTOCOL;
        assert!(!accepts(invented_signed_transition));

        let mut noncanonical_empty = base_facts();
        noncanonical_empty.effects.kind0 = EFFECT_BROADCAST;
        assert!(!accepts(noncanonical_empty));

        let mut impossible_roster = base_facts();
        impossible_roster.validator_count = u64::MAX / 2 + 1;
        assert!(!accepts(impossible_roster));
    }
}
