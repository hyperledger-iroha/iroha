//! Executable refinement gate shared by production and Verus.
//!
//! The reducer projects each candidate transition into concrete
//! [`TransitionProjection`] primitives and must pass this gate before replacing
//! its caller-visible state.  The kernel privately derives all boolean facts
//! and effect/action authorization from those primitives.  Its expressions are
//! defined by macros because the exact same executable expressions are
//! instantiated in `verus_proofs.rs`; normal builds therefore do not need to
//! link `vstd`.

// The fixed-width proof projection is intentionally passed by value so the
// normal Rust and Verus instantiations share one branch-complete expression.
// Production inlines this private gate; changing it to borrowed wrappers would
// create a second, unverified calling relation solely to silence this lint.
#![allow(clippy::large_types_passed_by_value)]

use super::{
    ContextId, Digest, DurableState, HeightContext, Reducer, Subject, ValidatorId,
    reducer::PendingPersistence,
};

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
pub const EVENT_RESUME_AFTER_REPLAY: u8 = 15;

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
#[allow(dead_code)]
pub const ACTION_STUTTER: u8 = 0;
#[allow(dead_code)]
pub const ACTION_BEGIN_WAL: u8 = 1;
#[allow(dead_code)]
pub const ACTION_ACKNOWLEDGE_WAL: u8 = 2;
#[allow(dead_code)]
pub const ACTION_BODY_PROGRESS: u8 = 3;
#[allow(dead_code)]
pub const ACTION_VOLATILE_PROTOCOL: u8 = 4;
#[allow(dead_code)]
pub const ACTION_COMPLETE_APPLICATION: u8 = 5;
/// Consume the one recovery-pending transition created by successful replay.
#[allow(dead_code)]
pub const ACTION_RESUME_AFTER_REPLAY: u8 = 6;

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

/// Replay emitted no safety-relevant effect because the WAL was empty.
pub const REPLAY_EFFECT_NONE: u8 = 0;
/// Replay resumed an already-durable proposal intent.
pub const REPLAY_EFFECT_PROPOSAL: u8 = 1;
/// Replay resumed an already-durable Prepare intent.
pub const REPLAY_EFFECT_PREPARE: u8 = 2;
/// Replay resumed an already-durable Commit intent.
pub const REPLAY_EFFECT_COMMIT: u8 = 3;
/// Replay resumed an already-durable timeout intent.
pub const REPLAY_EFFECT_TIMEOUT: u8 = 4;
/// Replay resumed acquisition of a durably decided body.
pub const REPLAY_EFFECT_DECISION: u8 = 5;

/// No durable-boundary capability is claimed by a transition.
pub const BOUNDARY_NONE: u8 = 0;
/// Capability to create one pending WAL append.
pub const BOUNDARY_BEGIN_WAL: u8 = 1;
/// Capability to acknowledge and install one pending WAL append.
pub const BOUNDARY_ACKNOWLEDGE_WAL: u8 = 2;
/// Capability to acknowledge local application of a durable decision.
pub const BOUNDARY_COMPLETE_APPLICATION: u8 = 3;
/// Capability to consume the one recovery-resumption transition.
pub const BOUNDARY_RESUME_AFTER_REPLAY: u8 = 4;

/// Primitive `(height, view, generation)` projection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TagProjection {
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) generation: u64,
}

/// Primitive optional validator identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidatorProjection {
    pub(crate) present: bool,
    pub(crate) id: ValidatorId,
}

/// Primitive optional subject identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubjectProjection {
    pub(crate) present: bool,
    pub(crate) subject: Subject,
}

/// Concrete invariant violations extracted from one reducer state.
///
/// These are counts, not caller-provided truth values.  The verified kernel
/// accepts a state only when every independently computed class is empty.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SafetyProjection {
    pub(crate) durable_identity_mismatches: u64,
    pub(crate) asynchronous_fence_conflicts: u64,
    pub(crate) invalid_highest_prepare: u64,
    pub(crate) invalid_lock: u64,
    pub(crate) invalid_timeout: u64,
    pub(crate) invalid_decision: u64,
    pub(crate) invalid_pending_append: u64,
    pub(crate) unauthorized_signables: u64,
    pub(crate) invalid_application: u64,
}

/// Safety identity of one pending WAL append and its continuation.
///
/// `record_kind == WAL_RECORD_NONE` is the sole absent value.  The remaining
/// fields are canonical zeroes in that case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PendingProjection {
    pub(crate) record_kind: u8,
    pub(crate) continuation: u8,
    pub(crate) persistence_id: u64,
    pub(crate) context_id: ContextId,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) subject: Subject,
}

/// Concrete capability key used for one reducer effect.
///
/// The key contains only fixed-width, safety-relevant primitives.  Signature
/// bytes are deliberately outside the reducer proof boundary; their checked
/// message identity is represented by context, round, phase, subject, actor,
/// WAL id, and auxiliary certificate/manifest identity below.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectCapabilityKey {
    pub(crate) kind: u8,
    pub(crate) tag: TagProjection,
    pub(crate) context_id: ContextId,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) phase: u8,
    pub(crate) subject: Subject,
    pub(crate) actor: ValidatorId,
    pub(crate) persistence_id: u64,
    pub(crate) record_kind: u8,
    pub(crate) auxiliary_context_id: ContextId,
    pub(crate) auxiliary_height: u64,
    pub(crate) auxiliary_view: u64,
    pub(crate) auxiliary_phase: u8,
    pub(crate) auxiliary_subject: Subject,
    pub(crate) manifest_payload: Digest,
    pub(crate) manifest_chunks: Digest,
    pub(crate) manifest_len: u64,
    pub(crate) manifest_count: u64,
}

impl EffectCapabilityKey {
    /// Canonical absent key.
    pub(crate) const fn none() -> Self {
        Self {
            kind: EFFECT_NONE,
            tag: TagProjection {
                height: 0,
                view: 0,
                generation: 0,
            },
            context_id: ContextId::repeat(0),
            height: 0,
            view: 0,
            phase: 0,
            subject: Subject::repeat(0),
            actor: ValidatorId::repeat(0),
            persistence_id: 0,
            record_kind: WAL_RECORD_NONE,
            auxiliary_context_id: ContextId::repeat(0),
            auxiliary_height: 0,
            auxiliary_view: 0,
            auxiliary_phase: 0,
            auxiliary_subject: Subject::repeat(0),
            manifest_payload: Digest::repeat(0),
            manifest_chunks: Digest::repeat(0),
            manifest_len: 0,
            manifest_count: 0,
        }
    }
}

/// Exact requested/granted capability pair for one effect vector slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectSlotProjection {
    pub(crate) kind: u8,
    pub(crate) requested: EffectCapabilityKey,
    pub(crate) granted: EffectCapabilityKey,
}

/// Primitive identity of a durable-boundary action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoundaryCapabilityKey {
    pub(crate) kind: u8,
    pub(crate) record_kind: u8,
    pub(crate) continuation: u8,
    pub(crate) replay_effect_kind: u8,
    pub(crate) persistence_id: u64,
    pub(crate) context_id: ContextId,
    pub(crate) tag: TagProjection,
    pub(crate) subject: SubjectProjection,
}

impl BoundaryCapabilityKey {
    /// Canonical absent boundary capability.
    pub(crate) const fn none() -> Self {
        Self {
            kind: BOUNDARY_NONE,
            record_kind: WAL_RECORD_NONE,
            continuation: CONTINUATION_NONE,
            replay_effect_kind: REPLAY_EFFECT_NONE,
            persistence_id: 0,
            context_id: ContextId::repeat(0),
            tag: TagProjection {
                height: 0,
                view: 0,
                generation: 0,
            },
            subject: SubjectProjection {
                present: false,
                subject: Subject::repeat(0),
            },
        }
    }
}

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
/// Each active slot carries a capability requested by the concrete effect and
/// an independently reconstructed capability granted by the candidate state.
/// The kernel, rather than the reducer extractor, decides authorization by
/// requiring the complete fixed-width keys to match.  Slots at or beyond
/// `len` must be canonical zeroes.  A fixed representation keeps complete
/// vector order visible to Verus without trusting an iterator implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectTrace {
    pub(crate) len: u8,
    pub(crate) slot0: EffectSlotProjection,
    pub(crate) slot1: EffectSlotProjection,
    pub(crate) slot2: EffectSlotProjection,
    pub(crate) slot3: EffectSlotProjection,
    pub(crate) slot4: EffectSlotProjection,
    pub(crate) slot5: EffectSlotProjection,
    pub(crate) slot6: EffectSlotProjection,
    pub(crate) slot7: EffectSlotProjection,
}

impl EffectTrace {
    /// Construct an empty canonical trace.
    pub(crate) const fn empty() -> Self {
        Self {
            len: 0,
            slot0: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot1: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot2: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot3: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot4: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot5: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot6: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot7: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
        }
    }

    /// Append one exact effect projection.
    pub(crate) fn push(
        &mut self,
        requested: EffectCapabilityKey,
        granted: EffectCapabilityKey,
    ) -> bool {
        let index = usize::from(self.len);
        if index >= MAX_EFFECTS_PER_STEP {
            return false;
        }
        let slot = EffectSlotProjection {
            kind: requested.kind,
            requested,
            granted,
        };
        match index {
            0 => self.slot0 = slot,
            1 => self.slot1 = slot,
            2 => self.slot2 = slot,
            3 => self.slot3 = slot,
            4 => self.slot4 = slot,
            5 => self.slot5 = slot,
            6 => self.slot6 = slot,
            7 => self.slot7 = slot,
            _ => return false,
        }
        self.len += 1;
        true
    }
}

/// Concrete primitive projection consumed by the verified transition kernel.
///
/// Exact reducer and durable-state references are compared directly by the
/// executable kernel.  The Verus instantiation represents those identities as
/// mathematical integers, while sharing the same derivation expression.
#[derive(Clone, Copy, Debug)]
pub struct TransitionProjection<'a> {
    pub(crate) before_state: &'a Reducer,
    pub(crate) after_state: &'a Reducer,
    pub(crate) durable_before: &'a DurableState,
    pub(crate) durable_after: &'a DurableState,
    pub(crate) safety_before: SafetyProjection,
    pub(crate) safety_after: SafetyProjection,
    pub(crate) context_before: &'a HeightContext,
    pub(crate) context_after: &'a HeightContext,
    pub(crate) local_before: ValidatorProjection,
    pub(crate) local_after: ValidatorProjection,
    pub(crate) event_tag: TagProjection,
    pub(crate) height_before: u64,
    pub(crate) view_before: u64,
    pub(crate) generation_before: u64,
    pub(crate) generation_after: u64,
    pub(crate) pending_state_before: Option<&'a PendingPersistence>,
    pub(crate) pending_state_after: Option<&'a PendingPersistence>,
    pub(crate) pending_before: PendingProjection,
    pub(crate) awaiting_before: bool,
    pub(crate) replay_before: bool,
    pub(crate) application_before: SubjectProjection,
    pub(crate) application_after: SubjectProjection,
    pub(crate) event_kind: u8,
    pub(crate) awaiting_message_kind: u8,
    pub(crate) validator_count: u64,
    pub(crate) volatile_before: VolatileSummary,
    pub(crate) volatile_after: VolatileSummary,
    pub(crate) boundary_claimed: BoundaryCapabilityKey,
    pub(crate) boundary_granted: BoundaryCapabilityKey,
    pub(crate) effects: EffectTrace,
}

/// Internal facts derived by the kernel from one primitive transition.
///
/// This type never crosses the module boundary; in particular, the reducer
/// cannot supply any of its boolean fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct TransitionFacts {
    before_invariant: bool,
    after_invariant: bool,
    context_unchanged: bool,
    whole_state_unchanged: bool,
    tag_matches: bool,
    busy_fence_open: bool,
    event_kind: u8,
    action_kind: u8,
    wal_record_kind: u8,
    signed_message_kind: u8,
    replay_effect_kind: u8,
    validator_count: u64,
    volatile_before: VolatileSummary,
    volatile_after: VolatileSummary,
    durable_unchanged: bool,
    pending_unchanged: bool,
    generation_unchanged: bool,
    application_unchanged: bool,
    begin_persist_exact: bool,
    acknowledge_persist_exact: bool,
    application_transition_exact: bool,
    acknowledgement_continuation: u8,
    effects: EffectTrace,
}

// Keep this expression free of calls into production code.  It is expanded
// both below and inside `verus!`, so Verus proves the actual decision logic
// used by the production reducer rather than a separately transcribed checker.
macro_rules! effect_count_body {
    ($trace:expr, $kind:expr) => {{
        (if $trace.len > 0 && $trace.slot0.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 1 && $trace.slot1.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 2 && $trace.slot2.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 3 && $trace.slot3.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 4 && $trace.slot4.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 5 && $trace.slot5.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 6 && $trace.slot6.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 7 && $trace.slot7.kind == $kind {
            1u64
        } else {
            0u64
        })
    }};
}

macro_rules! capability_key_equal_body {
    ($left:expr, $right:expr) => {{
        $left.kind == $right.kind
            && $left.tag.height == $right.tag.height
            && $left.tag.view == $right.tag.view
            && $left.tag.generation == $right.tag.generation
            && $left.context_id == $right.context_id
            && $left.height == $right.height
            && $left.view == $right.view
            && $left.phase == $right.phase
            && $left.subject == $right.subject
            && $left.actor == $right.actor
            && $left.persistence_id == $right.persistence_id
            && $left.record_kind == $right.record_kind
            && $left.auxiliary_context_id == $right.auxiliary_context_id
            && $left.auxiliary_height == $right.auxiliary_height
            && $left.auxiliary_view == $right.auxiliary_view
            && $left.auxiliary_phase == $right.auxiliary_phase
            && $left.auxiliary_subject == $right.auxiliary_subject
            && $left.manifest_payload == $right.manifest_payload
            && $left.manifest_chunks == $right.manifest_chunks
            && $left.manifest_len == $right.manifest_len
            && $left.manifest_count == $right.manifest_count
    }};
}

macro_rules! capability_key_is_none_body {
    ($key:expr) => {{ $key.kind == 0u8 }};
}

macro_rules! active_effect_slot_body {
    ($slot:expr) => {{
        $slot.kind >= 1u8
            && $slot.kind <= 9u8
            && $slot.requested.kind == $slot.kind
            && $slot.granted.kind == $slot.kind
            && capability_key_equal_body!($slot.requested, $slot.granted)
    }};
}

macro_rules! inactive_effect_slot_body {
    ($slot:expr) => {{
        $slot.kind == 0u8
            && capability_key_is_none_body!($slot.requested)
            && capability_key_is_none_body!($slot.granted)
    }};
}

macro_rules! effect_slots_authorized_body {
    ($trace:expr) => {{
        $trace.len <= 8u8
            && (if $trace.len > 0 {
                active_effect_slot_body!($trace.slot0)
            } else {
                inactive_effect_slot_body!($trace.slot0)
            })
            && (if $trace.len > 1 {
                active_effect_slot_body!($trace.slot1)
            } else {
                inactive_effect_slot_body!($trace.slot1)
            })
            && (if $trace.len > 2 {
                active_effect_slot_body!($trace.slot2)
            } else {
                inactive_effect_slot_body!($trace.slot2)
            })
            && (if $trace.len > 3 {
                active_effect_slot_body!($trace.slot3)
            } else {
                inactive_effect_slot_body!($trace.slot3)
            })
            && (if $trace.len > 4 {
                active_effect_slot_body!($trace.slot4)
            } else {
                inactive_effect_slot_body!($trace.slot4)
            })
            && (if $trace.len > 5 {
                active_effect_slot_body!($trace.slot5)
            } else {
                inactive_effect_slot_body!($trace.slot5)
            })
            && (if $trace.len > 6 {
                active_effect_slot_body!($trace.slot6)
            } else {
                inactive_effect_slot_body!($trace.slot6)
            })
            && (if $trace.len > 7 {
                active_effect_slot_body!($trace.slot7)
            } else {
                inactive_effect_slot_body!($trace.slot7)
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
                    1 => $trace.slot0.kind == 5u8,
                    2 => $trace.slot1.kind == 5u8,
                    3 => $trace.slot2.kind == 5u8,
                    4 => $trace.slot3.kind == 5u8,
                    5 => $trace.slot4.kind == 5u8,
                    6 => $trace.slot5.kind == 5u8,
                    7 => $trace.slot6.kind == 5u8,
                    8 => $trace.slot7.kind == 5u8,
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

macro_rules! safety_projection_accepts_body {
    ($safety:expr) => {{
        $safety.durable_identity_mismatches == 0u64
            && $safety.asynchronous_fence_conflicts == 0u64
            && $safety.invalid_highest_prepare == 0u64
            && $safety.invalid_lock == 0u64
            && $safety.invalid_timeout == 0u64
            && $safety.invalid_decision == 0u64
            && $safety.invalid_pending_append == 0u64
            && $safety.unauthorized_signables == 0u64
            && $safety.invalid_application == 0u64
    }};
}

macro_rules! validator_projection_equal_body {
    ($left:expr, $right:expr) => {{ $left.present == $right.present && (!$left.present || $left.id == $right.id) }};
}

macro_rules! subject_projection_equal_body {
    ($left:expr, $right:expr) => {{ $left.present == $right.present && (!$left.present || $left.subject == $right.subject) }};
}

macro_rules! boundary_capability_equal_body {
    ($left:expr, $right:expr) => {{
        $left.kind == $right.kind
            && $left.record_kind == $right.record_kind
            && $left.continuation == $right.continuation
            && $left.replay_effect_kind == $right.replay_effect_kind
            && $left.persistence_id == $right.persistence_id
            && $left.context_id == $right.context_id
            && $left.tag.height == $right.tag.height
            && $left.tag.view == $right.tag.view
            && $left.tag.generation == $right.tag.generation
            && subject_projection_equal_body!($left.subject, $right.subject)
    }};
}

// Derive every safety/action boolean consumed by the legacy relation from
// concrete primitive state, boundary, and capability projections.  Production
// and Verus instantiate this exact expression with different identity types.
macro_rules! transition_facts_from_projection_body {
    ($projection:expr, $facts_type:ident) => {{
        let boundary_exact = $projection.boundary_claimed.kind != 0u8
            && boundary_capability_equal_body!(
                $projection.boundary_claimed,
                $projection.boundary_granted
            );
        let begin_persist_exact = boundary_exact && $projection.boundary_claimed.kind == 1u8;
        let acknowledge_persist_exact = boundary_exact && $projection.boundary_claimed.kind == 2u8;
        let application_boundary_exact = boundary_exact && $projection.boundary_claimed.kind == 3u8;
        let replay_boundary_exact = boundary_exact && $projection.boundary_claimed.kind == 4u8;
        let durable_unchanged = $projection.durable_before == $projection.durable_after;
        let pending_unchanged = $projection.pending_state_before == $projection.pending_state_after;
        let generation_unchanged = $projection.generation_before == $projection.generation_after;
        let application_unchanged = subject_projection_equal_body!(
            $projection.application_before,
            $projection.application_after
        );
        let state_unchanged = $projection.before_state == $projection.after_state;
        let action_kind = if begin_persist_exact {
            1u8
        } else if acknowledge_persist_exact {
            2u8
        } else if state_unchanged && $projection.effects.len == 0u8 {
            0u8
        } else if !application_unchanged {
            5u8
        } else if replay_boundary_exact {
            6u8
        } else if $projection.event_kind >= 8u8 && $projection.event_kind <= 10u8 {
            3u8
        } else {
            4u8
        };
        let replay_duplicate = $projection.replay_before && $projection.event_kind == 15u8;
        let recovery_fence_open = $projection.replay_before || $projection.event_kind == 15u8;
        let pending_completion = $projection.event_kind == 11u8 || $projection.event_kind == 12u8;
        let signing_completion = $projection.event_kind == 13u8;
        let pending_present = $projection.pending_before.record_kind != 0u8;
        let busy_fence_open = recovery_fence_open
            && (!pending_present || pending_completion || replay_duplicate)
            && (!$projection.awaiting_before || signing_completion || replay_duplicate);
        let tag_matches = $projection.event_tag.height == $projection.height_before
            && $projection.event_tag.view == $projection.view_before
            && $projection.event_tag.generation == $projection.generation_before;
        let wal_record_kind = if begin_persist_exact || acknowledge_persist_exact {
            $projection.boundary_claimed.record_kind
        } else {
            0u8
        };
        let acknowledgement_continuation = if acknowledge_persist_exact {
            $projection.boundary_claimed.continuation
        } else {
            0u8
        };
        let signed_message_kind = if action_kind == 0u8 || $projection.event_kind != 13u8 {
            0u8
        } else {
            $projection.awaiting_message_kind
        };
        let replay_effect_kind = if replay_boundary_exact {
            $projection.boundary_claimed.replay_effect_kind
        } else {
            0u8
        };
        $facts_type {
            before_invariant: safety_projection_accepts_body!($projection.safety_before),
            after_invariant: safety_projection_accepts_body!($projection.safety_after),
            context_unchanged: $projection.context_before == $projection.context_after
                && validator_projection_equal_body!(
                    $projection.local_before,
                    $projection.local_after
                ),
            whole_state_unchanged: state_unchanged,
            tag_matches,
            busy_fence_open,
            event_kind: $projection.event_kind,
            action_kind,
            wal_record_kind,
            signed_message_kind,
            replay_effect_kind,
            validator_count: $projection.validator_count,
            volatile_before: $projection.volatile_before,
            volatile_after: $projection.volatile_after,
            durable_unchanged,
            pending_unchanged,
            generation_unchanged,
            application_unchanged,
            begin_persist_exact,
            acknowledge_persist_exact,
            application_transition_exact: application_unchanged || application_boundary_exact,
            acknowledgement_continuation,
            effects: $projection.effects,
        }
    }};
}

macro_rules! volatile_replay_unchanged_body {
    ($before:expr, $after:expr) => {{
        $before.candidate_present == $after.candidate_present
            && $before.pending_prepare == $after.pending_prepare
            && $before.known_prepare == $after.known_prepare
            && $before.vote_pools == $after.vote_pools
            && $before.vote_entries == $after.vote_entries
            && $before.timeout_vote_pools == $after.timeout_vote_pools
            && $before.timeout_vote_entries == $after.timeout_vote_entries
            && $before.formed_certificates == $after.formed_certificates
            && $before.formed_timeouts == $after.formed_timeouts
            && $before.outbound_control == $after.outbound_control
            && $before.durable_signable_limit == $after.durable_signable_limit
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
            && $facts.whole_state_unchanged
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
                8 => $facts.effects.len == 1u8 && $facts.effects.slot0.kind == 3u8,
                9 => $facts.effects.len == 1u8 && $facts.effects.slot0.kind == 4u8,
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

macro_rules! resume_after_replay_action_body {
    ($facts:expr, $count_gate:ident $(,)?) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.event_kind == 15u8
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && !$facts.volatile_before.replay_resumed
            && $facts.volatile_after.replay_resumed
            && !$facts.volatile_before.awaiting_signature
            && $facts.volatile_before.signature_queue == 0u64
            && volatile_replay_unchanged_body!($facts.volatile_before, $facts.volatile_after)
            && $count_gate($facts.effects, 1u8) == 0u64
            && $count_gate($facts.effects, 3u8) == 0u64
            && $count_gate($facts.effects, 4u8) == 0u64
            && $count_gate($facts.effects, 6u8) == 0u64
            && $count_gate($facts.effects, 8u8) == 0u64
            && $count_gate($facts.effects, 9u8) == 0u64
            && match $facts.replay_effect_kind {
                0 => {
                    $facts.effects.len == 0u8
                        && $facts.volatile_after.body_work == $facts.volatile_before.body_work
                        && !$facts.volatile_after.awaiting_signature
                        && $facts.volatile_after.signature_queue == 0u64
                }
                1..=4 => {
                    $facts.effects.len == 1u8
                        && $facts.effects.slot0.kind == 5u8
                        && $facts.volatile_after.body_work == $facts.volatile_before.body_work
                        && $facts.volatile_after.awaiting_signature
                }
                5 => {
                    $facts.effects.len == 1u8
                        && $facts.effects.slot0.kind == 2u8
                        && $facts.volatile_before.body_work < u64::MAX
                        && $facts.volatile_after.body_work
                            == $facts.volatile_before.body_work + 1u64
                        && !$facts.volatile_after.awaiting_signature
                        && $facts.volatile_after.signature_queue == 0u64
                }
                _ => false,
            }
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
        $complete_application_gate:ident,
        $resume_after_replay_gate:ident $(,)?
    ) => {{
        match $facts.action_kind {
            0 => $stutter_gate($facts),
            1 => $begin_wal_gate($facts),
            2 => $acknowledge_wal_gate($facts),
            3 => $body_progress_gate($facts),
            4 => $volatile_protocol_gate($facts),
            5 => $complete_application_gate($facts),
            6 => $resume_after_replay_gate($facts),
            _ => false,
        }
    }};
}

macro_rules! production_action_relation_body {
    ($facts:expr, $signed_gate:ident, $action_gate:ident $(,)?) => {{
        $signed_gate($facts)
            && ($facts.action_kind == 6u8 || $facts.replay_effect_kind == 0u8)
            && $action_gate($facts)
    }};
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
            $facts.whole_state_unchanged
                && $facts.durable_unchanged
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
                            // A TC-selected lock carries its full PrepareQC.
                            // Installation therefore starts at most one
                            // certified body fetch in the successor view.
                            && $facts.volatile_after.body_work <= 1u64
                            && $fetch_count == $facts.volatile_after.body_work
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
                                || ($apply_count == 0u64 && $fetch_count == 1u64)
                                // A CommitQC may arrive after the exact body
                                // has entered StoreBody or validation.  That
                                // generation-tagged pipeline remains the sole
                                // continuation, so issuing a second fetch here
                                // would race useful work and can exhaust the
                                // bounded adapter.  A zero-effect Decision ack
                                // is accepted only while body work already
                                // exists and its cardinality is unchanged.
                                || ($apply_count == 0u64
                                    && $fetch_count == 0u64
                                    && $facts.volatile_before.body_work > 0u64
                                    && $facts.volatile_after.body_work
                                        == $facts.volatile_before.body_work))
                    }
                    _ => false,
                })
        } else if $facts.action_kind == 6u8 {
            $facts.event_kind == 15u8
                && $facts.durable_unchanged
                && $facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_unchanged
                && $persist_count == 0u64
                && $apply_count == 0u64
                && $enter_count == 0u64
                && ($sign_count == 0u64 || $sign_count == 1u64)
                && ($fetch_count == 0u64 || $fetch_count == 1u64)
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

fn resume_after_replay_action_is_valid(facts: TransitionFacts) -> bool {
    resume_after_replay_action_body!(facts, effect_count)
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
        resume_after_replay_action_is_valid,
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

fn accepts_facts(facts: TransitionFacts) -> bool {
    production_transition_gate_body!(
        facts,
        volatile_summary_is_well_formed,
        named_action_is_valid,
        effect_trace_accepts,
        transition_branch_accepts,
    )
}

/// Derive the complete checked relation from concrete primitive projections.
fn transition_facts(projection: TransitionProjection<'_>) -> TransitionFacts {
    transition_facts_from_projection_body!(projection, TransitionFacts)
}

/// Execute the verified transition kernel used as the production commit gate.
///
/// No caller-provided authorization or action-exactness boolean crosses this
/// boundary.  The kernel derives them from requested/granted capability keys,
/// exact pre/post state identities, event tags, and invariant violation counts.
#[must_use]
pub fn accepts(projection: TransitionProjection<'_>) -> bool {
    accepts_facts(transition_facts(projection))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(kind: u8, nonce: u64) -> EffectCapabilityKey {
        EffectCapabilityKey {
            kind,
            persistence_id: nonce,
            ..EffectCapabilityKey::default()
        }
    }

    fn push_authorized(trace: &mut EffectTrace, kind: u8) -> bool {
        let key = capability(kind, u64::from(trace.len) + 1);
        trace.push(key, key)
    }

    fn base_facts() -> TransitionFacts {
        let volatile = VolatileSummary {
            durable_signable_limit: 1,
            ..VolatileSummary::default()
        };
        TransitionFacts {
            before_invariant: true,
            after_invariant: true,
            context_unchanged: true,
            whole_state_unchanged: true,
            tag_matches: true,
            busy_fence_open: true,
            event_kind: 7,
            action_kind: ACTION_STUTTER,
            wal_record_kind: WAL_RECORD_NONE,
            signed_message_kind: SIGNED_MESSAGE_NONE,
            replay_effect_kind: REPLAY_EFFECT_NONE,
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
        assert!(accepts_facts(base_facts()));

        let mut facts = base_facts();
        facts.action_kind = ACTION_BEGIN_WAL;
        facts.wal_record_kind = WAL_RECORD_PROPOSAL_INTENT;
        facts.pending_unchanged = false;
        facts.begin_persist_exact = true;
        assert!(push_authorized(&mut facts.effects, EFFECT_PERSIST));
        assert!(accepts_facts(facts));
    }

    #[test]
    fn unauthorized_or_misordered_effects_fail_closed() {
        let mut unauthorized = base_facts();
        assert!(unauthorized.effects.push(
            capability(EFFECT_BROADCAST, 1),
            capability(EFFECT_BROADCAST, 2),
        ));
        assert!(!accepts_facts(unauthorized));

        let mut signing_not_last = base_facts();
        signing_not_last.event_kind = EVENT_SIGNED;
        assert!(push_authorized(&mut signing_not_last.effects, EFFECT_SIGN));
        assert!(push_authorized(
            &mut signing_not_last.effects,
            EFFECT_BROADCAST
        ));
        assert!(!accepts_facts(signing_not_last));

        let mut persist_and_sign = base_facts();
        persist_and_sign.action_kind = ACTION_BEGIN_WAL;
        persist_and_sign.wal_record_kind = WAL_RECORD_PROPOSAL_INTENT;
        persist_and_sign.pending_unchanged = false;
        persist_and_sign.begin_persist_exact = true;
        assert!(push_authorized(
            &mut persist_and_sign.effects,
            EFFECT_PERSIST
        ));
        assert!(push_authorized(&mut persist_and_sign.effects, EFFECT_SIGN));
        assert!(!accepts_facts(persist_and_sign));
    }

    #[test]
    fn stale_or_busy_input_must_be_an_exact_empty_stutter() {
        let mut stale = base_facts();
        stale.tag_matches = false;
        assert!(accepts_facts(stale));

        stale.application_transition_exact = false;
        stale.application_unchanged = false;
        assert!(!accepts_facts(stale));

        let mut busy = base_facts();
        busy.busy_fence_open = false;
        assert!(push_authorized(&mut busy.effects, EFFECT_FETCH));
        assert!(!accepts_facts(busy));
    }

    #[test]
    fn trace_capacity_is_fail_closed() {
        let mut trace = EffectTrace::empty();
        for _ in 0..MAX_EFFECTS_PER_STEP {
            assert!(push_authorized(&mut trace, EFFECT_BROADCAST));
        }
        assert!(!push_authorized(&mut trace, EFFECT_BROADCAST));
    }

    #[test]
    fn volatile_bounds_and_action_record_pairs_fail_closed() {
        let mut too_many_vote_pools = base_facts();
        too_many_vote_pools.volatile_after.vote_pools = 3;
        assert!(!accepts_facts(too_many_vote_pools));

        let mut invented_signature = base_facts();
        invented_signature.volatile_before.awaiting_signature = true;
        invented_signature.volatile_after.awaiting_signature = true;
        invented_signature.volatile_before.durable_signable_limit = 0;
        invented_signature.volatile_after.durable_signable_limit = 0;
        assert!(!accepts_facts(invented_signature));

        let mut bad_ack = base_facts();
        bad_ack.action_kind = ACTION_ACKNOWLEDGE_WAL;
        bad_ack.wal_record_kind = WAL_RECORD_DECISION;
        bad_ack.event_kind = EVENT_PERSISTED;
        bad_ack.pending_unchanged = false;
        bad_ack.acknowledge_persist_exact = true;
        bad_ack.acknowledgement_continuation = CONTINUATION_INSTALL_TIMEOUT;
        assert!(!accepts_facts(bad_ack));
    }

    #[test]
    fn decision_ack_may_retain_only_an_existing_body_pipeline() {
        let mut retained = base_facts();
        retained.action_kind = ACTION_ACKNOWLEDGE_WAL;
        retained.wal_record_kind = WAL_RECORD_DECISION;
        retained.event_kind = EVENT_PERSISTED;
        retained.pending_unchanged = false;
        retained.acknowledge_persist_exact = true;
        retained.acknowledgement_continuation = CONTINUATION_DECIDE;
        retained.volatile_before.body_work = 1;
        retained.volatile_after.body_work = 1;
        assert!(accepts_facts(retained));

        let mut missing_pipeline = retained;
        missing_pipeline.volatile_before.body_work = 0;
        missing_pipeline.volatile_after.body_work = 0;
        assert!(!accepts_facts(missing_pipeline));

        let mut dropped_pipeline = retained;
        dropped_pipeline.volatile_after.body_work = 0;
        assert!(!accepts_facts(dropped_pipeline));
    }

    #[test]
    fn body_pipeline_classifier_rejects_non_pipeline_effects() {
        let mut stored = base_facts();
        stored.action_kind = ACTION_BODY_PROGRESS;
        stored.event_kind = EVENT_BODY_AVAILABLE;
        assert!(push_authorized(&mut stored.effects, EFFECT_STORE));
        assert!(accepts_facts(stored));

        let mut validated = base_facts();
        validated.action_kind = ACTION_BODY_PROGRESS;
        validated.event_kind = 10;
        assert!(push_authorized(&mut validated.effects, EFFECT_REPORT));
        assert!(accepts_facts(validated));

        let mut invented_broadcast = validated;
        invented_broadcast.effects = EffectTrace::empty();
        assert!(push_authorized(
            &mut invented_broadcast.effects,
            EFFECT_BROADCAST
        ));
        assert!(!accepts_facts(invented_broadcast));

        let mut invented_fetch = validated;
        invented_fetch.effects = EffectTrace::empty();
        assert!(push_authorized(&mut invented_fetch.effects, EFFECT_FETCH));
        assert!(!accepts_facts(invented_fetch));
    }

    #[test]
    fn signed_classifier_and_inactive_slots_are_canonical() {
        let mut invented_signed_transition = base_facts();
        invented_signed_transition.event_kind = EVENT_SIGNED;
        invented_signed_transition.action_kind = ACTION_VOLATILE_PROTOCOL;
        assert!(!accepts_facts(invented_signed_transition));

        let mut noncanonical_empty = base_facts();
        noncanonical_empty.effects.slot0 = EffectSlotProjection {
            kind: EFFECT_BROADCAST,
            requested: EffectCapabilityKey::none(),
            granted: EffectCapabilityKey::none(),
        };
        assert!(!accepts_facts(noncanonical_empty));

        let mut impossible_roster = base_facts();
        impossible_roster.validator_count = u64::MAX / 2 + 1;
        assert!(!accepts_facts(impossible_roster));
    }

    #[test]
    fn replay_resume_has_a_distinct_one_shot_effect_relation() {
        let mut resumed = base_facts();
        resumed.event_kind = EVENT_RESUME_AFTER_REPLAY;
        resumed.action_kind = ACTION_RESUME_AFTER_REPLAY;
        resumed.replay_effect_kind = REPLAY_EFFECT_PREPARE;
        resumed.volatile_after.replay_resumed = true;
        resumed.volatile_after.awaiting_signature = true;
        assert!(push_authorized(&mut resumed.effects, EFFECT_SIGN));
        assert!(accepts_facts(resumed));

        let mut stale_did_work = resumed;
        stale_did_work.tag_matches = false;
        assert!(!accepts_facts(stale_did_work));

        let mut replayed_twice = resumed;
        replayed_twice.volatile_before.replay_resumed = true;
        assert!(!accepts_facts(replayed_twice));

        let mut decision_fetch = base_facts();
        decision_fetch.event_kind = EVENT_RESUME_AFTER_REPLAY;
        decision_fetch.action_kind = ACTION_RESUME_AFTER_REPLAY;
        decision_fetch.replay_effect_kind = REPLAY_EFFECT_DECISION;
        decision_fetch.volatile_after.replay_resumed = true;
        decision_fetch.volatile_after.body_work = 1;
        assert!(push_authorized(&mut decision_fetch.effects, EFFECT_FETCH));
        assert!(accepts_facts(decision_fetch));
    }
}
