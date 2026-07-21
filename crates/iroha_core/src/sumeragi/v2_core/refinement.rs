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

use std::collections::VecDeque;

use super::{
    ContextId, Digest, DurableState, HeightContext, Reducer, Subject, ValidatorId,
    reducer::PendingPersistence,
};

/// Maximum number of effects one reducer input can emit.
///
/// Retransmission is the largest branch: the seven canonical control-message
/// classes followed by at most one fetch, store, validation, or application
/// effect for the exact durable Decision stage. A future ninth effect fails
/// closed at the refinement boundary until this limit is deliberately revised
/// and re-verified.
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
pub const EVENT_PERSISTENCE_FAILED: u8 = 12;
pub const EVENT_SIGNED: u8 = 13;
pub const EVENT_RESUME_AFTER_REPLAY: u8 = 15;

pub const CONTINUATION_NONE: u8 = 0;
pub const CONTINUATION_SIGN: u8 = 1;
pub const CONTINUATION_INSTALL_TIMEOUT: u8 = 2;
pub const CONTINUATION_DECIDE: u8 = 3;

/// Prepend one persisted reducer continuation without reversing its order.
///
/// `V2Adapter::drive_effects` removes a `Persist` effect from the head of its
/// private work queue, acknowledges the WAL record synchronously, and obtains
/// the reducer effects causally enabled by that acknowledgement. Those effects
/// must run before the queue's old tail and in the exact order emitted by the
/// reducer. Iterating the continuation backwards while pushing each item to
/// the front implements precisely `continuation ++ old_tail`.
///
/// This helper is intentionally generic: the production call moves concrete
/// effects, while the refinement regression uses opaque tokens to pin the
/// queue transformation independently of effect identity or cloning.
#[allow(dead_code)]
pub fn prepend_causal_continuation<T>(pending: &mut VecDeque<T>, continuation: Vec<T>) {
    for item in continuation.into_iter().rev() {
        pending.push_front(item);
    }
}

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

/// Replay's first item is absent because no durable work needs reconstruction.
pub const REPLAY_EFFECT_NONE: u8 = 0;
/// Replay's first item resumes an already-durable proposal intent.
pub const REPLAY_EFFECT_PROPOSAL: u8 = 1;
/// Replay's first item resumes an already-durable Prepare intent.
pub const REPLAY_EFFECT_PREPARE: u8 = 2;
/// Replay's first item resumes an already-durable Commit intent.
pub const REPLAY_EFFECT_COMMIT: u8 = 3;
/// Replay's first item resumes an already-durable timeout intent.
pub const REPLAY_EFFECT_TIMEOUT: u8 = 4;
/// Replay's sole item resumes acquisition of a durably decided body.
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

/// No bounded runtime ingress class was selected.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_NONE: u8 = 0;
/// Trusted local completion ingress class.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_COMPLETION: u8 = 1;
/// Certified protocol progress ingress class.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_PROGRESS: u8 = 2;
/// Ordinary proposal and vote ingress class.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_NORMAL: u8 = 3;

/// A persisted view transition selected the effective lock and its fetch.
pub const EFFECTIVE_LOCK_TRACE_ENTER_VIEW: u8 = 1;
/// The body pipeline bound or monotonically enriched its exact owner.
pub const EFFECTIVE_LOCK_TRACE_OWNER: u8 = 2;
/// Supersession retired the exact body-capacity residuals.
pub const EFFECTIVE_LOCK_TRACE_RETIRE: u8 = 3;
/// One bounded ingress invocation selected an exact ready service class.
pub const EFFECTIVE_LOCK_TRACE_SERVICE: u8 = 4;

/// No quorum-certificate evidence is present at this projection position.
pub(crate) const CERTIFICATE_EVIDENCE_ABSENT: u8 = 0;
/// The certificate is byte-for-byte equal to the transition's local lock.
pub(crate) const CERTIFICATE_EVIDENCE_LOCAL: u8 = 1;
/// The certificate is byte-for-byte equal to the incoming timeout high-QC.
pub(crate) const CERTIFICATE_EVIDENCE_INCOMING: u8 = 2;
/// The certificate is not owned by either authenticated transition source.
pub(crate) const CERTIFICATE_EVIDENCE_FOREIGN: u8 = 3;

/// Domain for a frozen consensus context digest.
pub(crate) const IDENTITY_DOMAIN_CONTEXT: u8 = 1;
/// Domain for a canonical block or consensus subject component.
pub(crate) const IDENTITY_DOMAIN_SUBJECT: u8 = 2;
/// Domain for canonical body, request, or response bytes.
pub(crate) const IDENTITY_DOMAIN_PAYLOAD: u8 = 3;
/// Domain for an authenticated network source or semantic origin.
pub(crate) const IDENTITY_DOMAIN_PEER: u8 = 4;
/// Domain for a durable receipt or finality artifact.
pub(crate) const IDENTITY_DOMAIN_DURABLE_ARTIFACT: u8 = 5;
/// Canonical identity kind for one frozen consensus context.
pub(crate) const IDENTITY_KIND_CONSENSUS_CONTEXT: u8 = 1;
/// Canonical identity kind for one consensus subject.
pub(crate) const IDENTITY_KIND_CONSENSUS_SUBJECT: u8 = 1;
/// Canonical identity kind for one wire-level frozen height context.
pub(crate) const IDENTITY_KIND_WIRE_HEIGHT_CONTEXT: u8 = 2;
/// Canonical identity kind for one wire-level block subject.
pub(crate) const IDENTITY_KIND_WIRE_BLOCK_SUBJECT: u8 = 2;
/// Canonical identity kind for one block-header hash.
pub(crate) const IDENTITY_KIND_BLOCK_HEADER: u8 = 3;
/// Canonical identity kind for one canonical payload hash.
pub(crate) const IDENTITY_KIND_CANONICAL_PAYLOAD: u8 = 1;
/// Canonical identity kind for one execution commitment.
pub(crate) const IDENTITY_KIND_EXECUTION_COMMITMENT: u8 = 2;
/// Canonical identity kind for one complete quorum certificate.
pub(crate) const IDENTITY_KIND_QUORUM_CERTIFICATE: u8 = 3;
/// Canonical identity kind for one payload manifest.
pub(crate) const IDENTITY_KIND_PAYLOAD_MANIFEST: u8 = 4;
/// Canonical identity kind for one result-bearing block wire.
pub(crate) const IDENTITY_KIND_EXECUTED_BLOCK_WIRE: u8 = 5;
/// Canonical identity kind for one signed sidecar request.
pub(crate) const IDENTITY_KIND_SIDECAR_REQUEST: u8 = 6;
/// Canonical identity kind for one actor-admitted reply payload.
pub(crate) const IDENTITY_KIND_REPLY_PAYLOAD: u8 = 7;
/// Canonical identity kind for one merge-ledger entry.
pub(crate) const IDENTITY_KIND_MERGE_ENTRY: u8 = 8;
/// Canonical identity kind for one sidecar ledger-reference digest.
pub(crate) const IDENTITY_KIND_REFERENCE_DIGEST: u8 = 9;
/// Canonical identity kind for one complete network response.
pub(crate) const IDENTITY_KIND_NETWORK_RESPONSE: u8 = 10;
/// Canonical identity kind for one certified sidecar response.
pub(crate) const IDENTITY_KIND_SIDECAR_RESPONSE: u8 = 11;
/// Canonical identity kind for one certified sidecar chunk.
pub(crate) const IDENTITY_KIND_SIDECAR_CHUNK: u8 = 12;
/// Canonical identity kind for one sidecar payload digest.
pub(crate) const IDENTITY_KIND_SIDECAR_PAYLOAD: u8 = 13;
/// Canonical identity kind for one checksummed durable body frame.
pub(crate) const IDENTITY_KIND_DURABLE_BODY_FRAME: u8 = 1;
/// Canonical identity kind for one finality artifact.
pub(crate) const IDENTITY_KIND_FINALITY_ARTIFACT: u8 = 2;
/// Canonical identity kind for one authenticated peer.
pub(crate) const IDENTITY_KIND_PEER: u8 = 1;

/// Lossless fixed-width view of one existing canonical 256-bit identity.
///
/// `domain` and `kind` are kept outside the digest so projections of equal
/// bytes used for different protocol objects cannot be confused. The four
/// words preserve every input bit; this helper does not hash or truncate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CanonicalIdentityProjection {
    pub(crate) domain: u8,
    pub(crate) kind: u8,
    pub(crate) word0: u64,
    pub(crate) word1: u64,
    pub(crate) word2: u64,
    pub(crate) word3: u64,
}

impl CanonicalIdentityProjection {
    /// Canonical absent identity used by fixed-width optional projections.
    #[must_use]
    pub(crate) const fn zero() -> Self {
        Self {
            domain: 0,
            kind: 0,
            word0: 0,
            word1: 0,
            word2: 0,
            word3: 0,
        }
    }

    /// Project all 32 canonical bytes into four big-endian words.
    #[must_use]
    pub(crate) const fn from_bytes(domain: u8, kind: u8, bytes: [u8; 32]) -> Self {
        Self {
            domain,
            kind,
            word0: u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]),
            word1: u64::from_be_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ]),
            word2: u64::from_be_bytes([
                bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22],
                bytes[23],
            ]),
            word3: u64::from_be_bytes([
                bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30],
                bytes[31],
            ]),
        }
    }
}

macro_rules! canonical_identity_equal_body {
    ($left:expr, $right:expr) => {{
        $left.domain == $right.domain
            && $left.kind == $right.kind
            && $left.word0 == $right.word0
            && $left.word1 == $right.word1
            && $left.word2 == $right.word2
            && $left.word3 == $right.word3
    }};
}

macro_rules! canonical_identity_is_typed_body {
    ($identity:expr, $domain:expr, $kind:expr) => {{ $identity.domain == $domain && $identity.kind == $kind }};
}

macro_rules! canonical_identity_is_zero_body {
    ($identity:expr) => {{
        $identity.domain == 0u8
            && $identity.kind == 0u8
            && $identity.word0 == 0u64
            && $identity.word1 == 0u64
            && $identity.word2 == 0u64
            && $identity.word3 == 0u64
    }};
}

// One exact body-pipeline identity is carried unchanged through FetchBody,
// BodyAvailable, StoreBody, and ValidateBody. These macros are instantiated by
// typed production helpers below and by Verus over mathematical identities.
// Callers supply identities, never an authorization boolean.
macro_rules! exact_body_owner_equal_body {
    ($left:expr, $right:expr) => {{
        $left.tag.height == $right.tag.height
            && $left.tag.view == $right.tag.view
            && $left.tag.generation == $right.tag.generation
            && $left.key == $right.key
            && $left.manifest_hash == $right.manifest_hash
    }};
}

macro_rules! exact_body_owner_binding_body {
    ($current:expr, $incoming:expr, $owner_type:ident, $binding_type:ident) => {{
        match $current {
            None => Some($binding_type {
                owner: $incoming,
                already_owned: false,
            }),
            Some(current) => {
                if current.tag.height != $incoming.tag.height
                    || current.tag.view != $incoming.tag.view
                    || current.tag.generation != $incoming.tag.generation
                    || current.key != $incoming.key
                {
                    None
                } else {
                    match (current.manifest_hash, $incoming.manifest_hash) {
                        (Some(existing), Some(incoming)) if existing != incoming => None,
                        (Some(existing), _) => Some($binding_type {
                            owner: $owner_type {
                                tag: current.tag,
                                key: current.key,
                                manifest_hash: Some(existing),
                            },
                            already_owned: true,
                        }),
                        (None, incoming) => Some($binding_type {
                            owner: $owner_type {
                                tag: current.tag,
                                key: current.key,
                                manifest_hash: incoming,
                            },
                            already_owned: true,
                        }),
                    }
                }
            }
        }
    }};
}

macro_rules! exact_body_owner_rebind_body {
    ($current:expr, $previous:expr, $rebound_tag:expr, $owner_type:ident) => {{
        if !exact_body_owner_equal_body!($current, $previous)
            || $previous.tag.height != $rebound_tag.height
            || $previous.tag.view > $rebound_tag.view
            || $previous.tag.generation >= $rebound_tag.generation
        {
            None
        } else {
            Some($owner_type {
                tag: $rebound_tag,
                key: $previous.key,
                manifest_hash: $previous.manifest_hash,
            })
        }
    }};
}

// Runtime ingress and the Busy-deferred lane jointly own each logical
// completion slot. Exactly one lane may own one exact evidence value.
macro_rules! exact_body_completion_ownership_body {
    (
        $ingress_owners:expr,
        $ingress_exact:expr,
        $deferred_owners:expr,
        $deferred_exact:expr,
        $vacant:expr,
        $exact:expr,
        $invalid:expr $(,)?
    ) => {{
        if $ingress_owners == 0
            && $ingress_exact == 0
            && $deferred_owners == 0
            && $deferred_exact == 0
        {
            $vacant
        } else if ($ingress_owners == 1
            && $ingress_exact == 1
            && $deferred_owners == 0
            && $deferred_exact == 0)
            || ($ingress_owners == 0
                && $ingress_exact == 0
                && $deferred_owners == 1
                && $deferred_exact == 1)
        {
            $exact
        } else {
            $invalid
        }
    }};
}

// Supersession retires two independently bounded byte classes. Sequential
// subtraction makes both the no-underflow precondition and the exact residual
// explicit without relying on an overflowing `retained + ready` sum.
macro_rules! exact_body_retirement_accounting_body {
    (
        $ready_before:expr,
        $retained_bytes:expr,
        $ready_bytes:expr,
        $store_before:expr,
        $store_bytes:expr,
        $accounting_type:ident $(,)?
    ) => {{
        if $retained_bytes > $ready_before {
            None
        } else {
            let after_retained = $ready_before - $retained_bytes;
            if $ready_bytes > after_retained || $store_bytes > $store_before {
                None
            } else {
                Some($accounting_type {
                    ready_after: after_retained - $ready_bytes,
                    store_after: $store_before - $store_bytes,
                })
            }
        }
    }};
}

// Exact three-class round-robin branch relation used by runtime ingress.
// Every call examines all classes from the persistent cursor and advances the
// cursor past the selected class; an empty call makes one full rotation.
macro_rules! bounded_service_selection_body {
    (
        $cursor:expr,
        $completion_ready:expr,
        $progress_ready:expr,
        $normal_ready:expr,
        $selection_type:ident $(,)?
    ) => {{
        if $cursor == 1u8 {
            if $completion_ready {
                $selection_type {
                    selected: 1u8,
                    next: 2u8,
                }
            } else if $progress_ready {
                $selection_type {
                    selected: 2u8,
                    next: 3u8,
                }
            } else if $normal_ready {
                $selection_type {
                    selected: 3u8,
                    next: 1u8,
                }
            } else {
                $selection_type {
                    selected: 0u8,
                    next: 1u8,
                }
            }
        } else if $cursor == 2u8 {
            if $progress_ready {
                $selection_type {
                    selected: 2u8,
                    next: 3u8,
                }
            } else if $normal_ready {
                $selection_type {
                    selected: 3u8,
                    next: 1u8,
                }
            } else if $completion_ready {
                $selection_type {
                    selected: 1u8,
                    next: 2u8,
                }
            } else {
                $selection_type {
                    selected: 0u8,
                    next: 2u8,
                }
            }
        } else if $cursor == 3u8 {
            if $normal_ready {
                $selection_type {
                    selected: 3u8,
                    next: 1u8,
                }
            } else if $completion_ready {
                $selection_type {
                    selected: 1u8,
                    next: 2u8,
                }
            } else if $progress_ready {
                $selection_type {
                    selected: 2u8,
                    next: 3u8,
                }
            } else {
                $selection_type {
                    selected: 0u8,
                    next: 3u8,
                }
            }
        } else {
            $selection_type {
                selected: 0u8,
                next: 0u8,
            }
        }
    }};
}

// A compact discriminated trace ties the four production effective-lock seams
// to one executable relation. Each producer fills only its action branch and
// must use canonical zeroes for every unrelated field. Verus instantiates this
// exact macro over its mathematical mirror, while production invokes one of
// the four checked wrappers below with values derived from live state.
macro_rules! effective_lock_trace_step_body {
    ($projection:expr) => {{
        if $projection.kind == 1u8 {
            $projection.relation_exact
                && $projection.protected_before <= 1u64
                && $projection.protected_after == $projection.protected_before
                && $projection.owner_before == $projection.protected_before
                && $projection.owner_after == $projection.owner_before
                && !$projection.owner_reused
                && $projection.ready_before == 0u64
                && $projection.retired_retained == 0u64
                && $projection.retired_ready == 0u64
                && $projection.ready_after == 0u64
                && $projection.store_before == 0u64
                && $projection.retired_store == 0u64
                && $projection.store_after == 0u64
                && $projection.cursor_before == 0u8
                && !$projection.completion_ready
                && !$projection.progress_ready
                && !$projection.normal_ready
                && $projection.selected == 0u8
                && $projection.cursor_after == 0u8
        } else if $projection.kind == 2u8 {
            $projection.relation_exact
                && $projection.protected_before <= 1u64
                && $projection.protected_after <= 1u64
                && $projection.protected_before <= $projection.protected_after
                && $projection.owner_before <= 1u64
                && $projection.owner_after == 1u64
                && $projection.owner_reused == ($projection.owner_before == 1u64)
                && $projection.ready_before == 0u64
                && $projection.retired_retained == 0u64
                && $projection.retired_ready == 0u64
                && $projection.ready_after == 0u64
                && $projection.store_before == 0u64
                && $projection.retired_store == 0u64
                && $projection.store_after == 0u64
                && $projection.cursor_before == 0u8
                && !$projection.completion_ready
                && !$projection.progress_ready
                && !$projection.normal_ready
                && $projection.selected == 0u8
                && $projection.cursor_after == 0u8
        } else if $projection.kind == 3u8 {
            $projection.relation_exact
                && $projection.protected_before == 0u64
                && $projection.protected_after == 0u64
                && $projection.owner_before == 0u64
                && $projection.owner_after == 0u64
                && !$projection.owner_reused
                && $projection.retired_retained <= $projection.ready_before
                && $projection.retired_ready
                    <= $projection.ready_before - $projection.retired_retained
                && $projection.ready_after
                    == $projection.ready_before
                        - $projection.retired_retained
                        - $projection.retired_ready
                && $projection.retired_store <= $projection.store_before
                && $projection.store_after == $projection.store_before - $projection.retired_store
                && $projection.cursor_before == 0u8
                && !$projection.completion_ready
                && !$projection.progress_ready
                && !$projection.normal_ready
                && $projection.selected == 0u8
                && $projection.cursor_after == 0u8
        } else if $projection.kind == 4u8 {
            $projection.relation_exact
                && $projection.protected_before == 0u64
                && $projection.protected_after == 0u64
                && $projection.owner_before == 0u64
                && $projection.owner_after == 0u64
                && !$projection.owner_reused
                && $projection.ready_before == 0u64
                && $projection.retired_retained == 0u64
                && $projection.retired_ready == 0u64
                && $projection.ready_after == 0u64
                && $projection.store_before == 0u64
                && $projection.retired_store == 0u64
                && $projection.store_after == 0u64
                && $projection.cursor_before >= 1u8
                && $projection.cursor_before <= 3u8
                && (if $projection.selected == 0u8 {
                    !$projection.completion_ready
                        && !$projection.progress_ready
                        && !$projection.normal_ready
                        && $projection.cursor_after == $projection.cursor_before
                } else if $projection.selected == 1u8 {
                    $projection.completion_ready && $projection.cursor_after == 2u8
                } else if $projection.selected == 2u8 {
                    $projection.progress_ready && $projection.cursor_after == 3u8
                } else if $projection.selected == 3u8 {
                    $projection.normal_ready && $projection.cursor_after == 1u8
                } else {
                    false
                })
        } else {
            false
        }
    }};
}

macro_rules! effective_lock_trace_claim_body {
    ($projection:expr, $kind:expr) => {{ $projection.kind == $kind && effective_lock_trace_step_is_valid($projection) }};
}

macro_rules! pending_projection_is_absent_body {
    ($pending:expr) => {{
        $pending.record_kind == WAL_RECORD_NONE
            && $pending.continuation == CONTINUATION_NONE
            && $pending.persistence_id == 0u64
            && canonical_identity_is_zero_body!($pending.context_id)
            && $pending.height == 0u64
            && $pending.view == 0u64
            && !$pending.proposal_present
            && $pending.proposal_height == 0u64
            && $pending.proposal_view == 0u64
            && canonical_identity_is_zero_body!($pending.subject)
    }};
}

macro_rules! pending_projection_equal_body {
    ($left:expr, $right:expr) => {{
        $left.record_kind == $right.record_kind
            && $left.continuation == $right.continuation
            && $left.persistence_id == $right.persistence_id
            && canonical_identity_equal_body!($left.context_id, $right.context_id)
            && $left.height == $right.height
            && $left.view == $right.view
            && $left.proposal_present == $right.proposal_present
            && $left.proposal_height == $right.proposal_height
            && $left.proposal_view == $right.proposal_view
            && canonical_identity_equal_body!($left.subject, $right.subject)
    }};
}

macro_rules! pending_projection_matches_boundary_body {
    ($pending:expr, $boundary:expr) => {{
        $boundary.subject.present
            && $pending.record_kind == $boundary.record_kind
            && $pending.continuation == $boundary.continuation
            && $pending.persistence_id == $boundary.persistence_id
            && canonical_identity_equal_body!($pending.context_id, $boundary.context_identity)
            && $pending.proposal_present == $boundary.proposal_present
            && $pending.proposal_height == $boundary.proposal_height
            && $pending.proposal_view == $boundary.proposal_view
            && canonical_identity_equal_body!($pending.subject, $boundary.subject_identity)
    }};
}

macro_rules! wal_record_proposal_round_is_exact_body {
    ($record_kind:expr, $pending:expr, $boundary:expr) => {{
        match $record_kind {
            WAL_RECORD_PROPOSAL_INTENT | WAL_RECORD_PREPARE_INTENT | WAL_RECORD_OBSERVE_PREPARE => {
                $pending.proposal_present
                    && $pending.proposal_height == $pending.height
                    && $pending.proposal_view == $pending.view
            }
            WAL_RECORD_LOCK_AND_COMMIT => {
                $pending.proposal_present
                    && $pending.proposal_height == $pending.height
                    && $pending.proposal_view <= $pending.view
                    && $boundary.auxiliary_present
                    && $boundary.auxiliary_context_id == $boundary.context_id
                    && $boundary.auxiliary_height == $pending.proposal_height
                    && $boundary.auxiliary_view == $pending.proposal_view
                    && $boundary.auxiliary_proposal_height == $pending.proposal_height
                    && $boundary.auxiliary_proposal_view == $pending.proposal_view
                    && $boundary.auxiliary_phase == 1u8
                    && $boundary.auxiliary_subject == $boundary.subject.subject
            }
            WAL_RECORD_DECISION => {
                $pending.proposal_present
                    && $pending.proposal_height == $pending.height
                    && $pending.proposal_view <= $pending.view
            }
            WAL_RECORD_TIMEOUT_INTENT | WAL_RECORD_INSTALL_TIMEOUT => {
                !$pending.proposal_present
                    && $pending.proposal_height == 0u64
                    && $pending.proposal_view == 0u64
            }
            _ => false,
        }
    }};
}

macro_rules! wal_record_continuation_is_exact_body {
    ($record_kind:expr, $continuation:expr) => {{
        match $record_kind {
            WAL_RECORD_PROPOSAL_INTENT
            | WAL_RECORD_PREPARE_INTENT
            | WAL_RECORD_LOCK_AND_COMMIT
            | WAL_RECORD_TIMEOUT_INTENT => $continuation == CONTINUATION_SIGN,
            WAL_RECORD_OBSERVE_PREPARE => $continuation == CONTINUATION_NONE,
            WAL_RECORD_INSTALL_TIMEOUT => $continuation == CONTINUATION_INSTALL_TIMEOUT,
            WAL_RECORD_DECISION => $continuation == CONTINUATION_DECIDE,
            _ => false,
        }
    }};
}

macro_rules! wal_record_round_matches_owner_body {
    ($record_kind:expr, $pending:expr, $owner:expr) => {{
        $pending.height == $owner.height
            && match $record_kind {
                WAL_RECORD_PROPOSAL_INTENT
                | WAL_RECORD_PREPARE_INTENT
                | WAL_RECORD_LOCK_AND_COMMIT
                | WAL_RECORD_TIMEOUT_INTENT => $pending.view == $owner.view,
                WAL_RECORD_OBSERVE_PREPARE => $pending.view <= $owner.view,
                WAL_RECORD_INSTALL_TIMEOUT => {
                    $pending.view < u64::MAX
                        && ($owner.view <= $pending.view || $pending.view + 1u64 == $owner.view)
                }
                WAL_RECORD_DECISION => true,
                _ => false,
            }
    }};
}

macro_rules! event_can_start_wal_record_body {
    ($event_kind:expr, $record_kind:expr) => {{
        match $event_kind {
            0u8 => $record_kind == WAL_RECORD_PROPOSAL_INTENT,
            2u8 | 3u8 => {
                $record_kind == WAL_RECORD_OBSERVE_PREPARE
                    || $record_kind == WAL_RECORD_LOCK_AND_COMMIT
                    || $record_kind == WAL_RECORD_DECISION
            }
            4u8 | 5u8 => $record_kind == WAL_RECORD_INSTALL_TIMEOUT,
            6u8 => $record_kind == WAL_RECORD_TIMEOUT_INTENT,
            10u8 => {
                $record_kind == WAL_RECORD_PREPARE_INTENT
                    || $record_kind == WAL_RECORD_LOCK_AND_COMMIT
            }
            EVENT_SIGNED => {
                $record_kind == WAL_RECORD_PREPARE_INTENT
                    || $record_kind == WAL_RECORD_OBSERVE_PREPARE
                    || $record_kind == WAL_RECORD_LOCK_AND_COMMIT
                    || $record_kind == WAL_RECORD_INSTALL_TIMEOUT
                    || $record_kind == WAL_RECORD_DECISION
            }
            _ => false,
        }
    }};
}

macro_rules! persist_slot_matches_boundary_body {
    ($slot:expr, $pending:expr, $boundary:expr) => {{
        if $slot.kind == EFFECT_PERSIST {
            $slot.requested.persistence_id == $boundary.persistence_id
                && $slot.requested.record_kind == $boundary.record_kind
                && $slot.requested.context_id == $boundary.context_id
                && $slot.requested.height == $pending.height
                && $slot.requested.view == $pending.view
                && $slot.requested.proposal_height == $boundary.proposal_height
                && $slot.requested.proposal_view == $boundary.proposal_view
                && $slot.requested.subject == $boundary.subject.subject
                && $slot.requested.tag.height == $boundary.tag.height
                && $slot.requested.tag.view == $boundary.tag.view
                && $slot.requested.tag.generation == $boundary.tag.generation
                && $slot.requested.auxiliary_present == $boundary.auxiliary_present
                && $slot.requested.auxiliary_context_id == $boundary.auxiliary_context_id
                && $slot.requested.auxiliary_height == $boundary.auxiliary_height
                && $slot.requested.auxiliary_view == $boundary.auxiliary_view
                && $slot.requested.auxiliary_proposal_height == $boundary.auxiliary_proposal_height
                && $slot.requested.auxiliary_proposal_view == $boundary.auxiliary_proposal_view
                && $slot.requested.auxiliary_phase == $boundary.auxiliary_phase
                && $slot.requested.auxiliary_subject == $boundary.auxiliary_subject
        } else {
            true
        }
    }};
}

macro_rules! tag_projection_equal_body {
    ($left:expr, $right:expr) => {{
        $left.height == $right.height
            && $left.view == $right.view
            && $left.generation == $right.generation
    }};
}

// A validated lock must retain exactly one durable reconstruction witness.
// The expression is shared verbatim with Verus and derives exactness from
// primitive round, context, subject, signer, and persistence observations.
// A durable timeout is deliberately admissible only for a historical lock:
// it witnesses that the current finality view closed before LockAndCommit
// could be appended and that the next certified view transition can retry the
// immutable proposal origin. It never authorizes a Commit in the closed view.
macro_rules! locked_commit_progress_witness_body {
    ($projection:expr) => {{
        let lock_is_active = $projection.locked_context_id == $projection.context_id
            && $projection.locked_height == $projection.current_height;
        let lock_is_historical =
            lock_is_active && $projection.locked_view < $projection.current_view;
        let commit_is_exact = lock_is_active
            && $projection.commit_intent_present
            && $projection.local_validator_present
            && $projection.commit_context_id == $projection.context_id
            && $projection.commit_height == $projection.current_height
            && $projection.commit_view >= $projection.locked_view
            && $projection.commit_proposal_height == $projection.locked_height
            && $projection.commit_proposal_view == $projection.locked_view
            && $projection.commit_phase == 2u8
            && $projection.commit_subject == $projection.locked_subject
            && $projection.commit_signer == $projection.local_validator
            && ($projection.commit_signature_pending || $projection.commit_pooled);
        let pending_lock_and_commit_is_exact = lock_is_active
            && !$projection.commit_intent_present
            && $projection.local_validator_present
            && $projection.pending.record_kind == WAL_RECORD_LOCK_AND_COMMIT
            && $projection.pending.continuation == CONTINUATION_SIGN
            && $projection.pending.persistence_id > 0u64
            && $projection.pending.context_id == $projection.context_id
            && $projection.pending.height == $projection.current_height
            && $projection.pending.view == $projection.current_view
            && $projection.pending.proposal_present
            && $projection.pending.proposal_height == $projection.locked_height
            && $projection.pending.proposal_view == $projection.locked_view
            && $projection.pending.subject == $projection.locked_subject;
        let durable_timeout_is_exact = !$projection.commit_intent_present
            && lock_is_historical
            && $projection.local_validator_present
            && $projection.timeout_intent_present
            && $projection.timeout_intent_durable
            && $projection.timeout_context_id == $projection.context_id
            && $projection.timeout_height == $projection.current_height
            && $projection.timeout_view == $projection.current_view
            && $projection.timeout_signer == $projection.local_validator;
        commit_is_exact || pending_lock_and_commit_is_exact || durable_timeout_is_exact
    }};
}

// These seven expressions are the source-shared production/Verus kernels for
// the progress-witness refinement.  Every field is a primitive observation at
// the enforcing production seam; in particular, callers cannot supply an
// already-computed "valid" or "owned" bit.
macro_rules! production_durable_intent_trace_body {
    ($projection:expr) => {{
        let boundary_exact = $projection.boundary_claimed.kind != BOUNDARY_NONE
            && boundary_capability_equal_body!(
                $projection.boundary_claimed,
                $projection.boundary_granted
            );
        let persist_effects = effect_count_body!($projection.effects, EFFECT_PERSIST);
        let tag_matches_owner =
            tag_projection_equal_body!($projection.event_tag, $projection.owner_tag_before);
        let persistence_completion = $projection.event_kind == EVENT_PERSISTED
            || $projection.event_kind == EVENT_PERSISTENCE_FAILED;
        effect_slots_authorized_body!($projection.effects)
            && persist_effects <= 1u64
            && $projection.durable_sequence_after >= $projection.durable_sequence_before
            && (if !tag_matches_owner {
                boundary_capability_is_absent_body!($projection.boundary_claimed)
                    && boundary_capability_is_absent_body!($projection.boundary_granted)
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && $projection.effects.len == 0u8
                    && persist_effects == 0u64
                    && (persistence_completion || $projection.event_persistence_id == 0u64)
            } else if boundary_exact && $projection.boundary_claimed.kind == BOUNDARY_BEGIN_WAL {
                pending_projection_is_absent_body!($projection.pending_before)
                    && boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && $projection.boundary_claimed.subject.present
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && pending_projection_matches_boundary_body!(
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && wal_record_round_matches_owner_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_after,
                        $projection.owner_tag_before
                    )
                    && wal_record_proposal_round_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && wal_record_continuation_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.boundary_claimed.continuation
                    )
                    && (if $projection.boundary_claimed.record_kind == WAL_RECORD_INSTALL_TIMEOUT {
                        if $projection.boundary_claimed.auxiliary_present {
                            $projection.boundary_claimed.auxiliary_phase == 1u8
                                && $projection.boundary_claimed.subject.subject
                                    == $projection.boundary_claimed.auxiliary_subject
                        } else {
                            true
                        }
                    } else {
                        true
                    })
                    && event_can_start_wal_record_body!(
                        $projection.event_kind,
                        $projection.boundary_claimed.record_kind
                    )
                    && $projection.durable_sequence_before < u64::MAX
                    && $projection.pending_after.persistence_id
                        == $projection.durable_sequence_before + 1u64
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && $projection.event_kind != EVENT_PERSISTED
                    && $projection.event_persistence_id == 0u64
                    && persist_effects == 1u64
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot0,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot1,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot2,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot3,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot4,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot5,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot6,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot7,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
            } else if boundary_exact
                && $projection.boundary_claimed.kind == BOUNDARY_ACKNOWLEDGE_WAL
            {
                pending_projection_matches_boundary_body!(
                    $projection.pending_before,
                    $projection.boundary_claimed
                ) && boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && $projection.boundary_claimed.subject.present
                    && wal_record_continuation_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.boundary_claimed.continuation
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && wal_record_round_matches_owner_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_before,
                        $projection.owner_tag_before
                    )
                    && wal_record_proposal_round_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_before,
                        $projection.boundary_claimed
                    )
                    && (if $projection.boundary_claimed.record_kind == WAL_RECORD_INSTALL_TIMEOUT {
                        $projection.owner_tag_after.height == $projection.owner_tag_before.height
                            && $projection.owner_tag_after.view
                                == $projection.pending_before.view + 1u64
                            && $projection.owner_tag_before.generation < u64::MAX
                            && $projection.owner_tag_after.generation
                                == $projection.owner_tag_before.generation + 1u64
                    } else {
                        tag_projection_equal_body!(
                            $projection.owner_tag_before,
                            $projection.owner_tag_after
                        )
                    })
                    && pending_projection_is_absent_body!($projection.pending_after)
                    && $projection.durable_sequence_before < u64::MAX
                    && $projection.durable_sequence_after
                        == $projection.durable_sequence_before + 1u64
                    && $projection.pending_before.persistence_id
                        == $projection.durable_sequence_after
                    && $projection.event_kind == EVENT_PERSISTED
                    && $projection.event_persistence_id
                        == $projection.boundary_claimed.persistence_id
                    && persist_effects == 0u64
            } else if boundary_exact
                && $projection.boundary_claimed.kind == BOUNDARY_COMPLETE_APPLICATION
            {
                boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && $projection.boundary_claimed.subject.present
                    && $projection.boundary_claimed.record_kind == WAL_RECORD_NONE
                    && $projection.boundary_claimed.continuation == CONTINUATION_NONE
                    && $projection.boundary_claimed.replay_effect_kind == REPLAY_EFFECT_NONE
                    && $projection.boundary_claimed.persistence_id == 0u64
                    && $projection.boundary_claimed.replay_plan.len == 0u8
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && $projection.event_kind == 14u8
                    && $projection.event_persistence_id == 0u64
                    && persist_effects == 0u64
            } else if boundary_exact
                && $projection.boundary_claimed.kind == BOUNDARY_RESUME_AFTER_REPLAY
            {
                boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && $projection.boundary_claimed.record_kind == WAL_RECORD_NONE
                    && $projection.boundary_claimed.continuation == CONTINUATION_NONE
                    && $projection.boundary_claimed.persistence_id
                        == $projection.durable_sequence_before
                    && replay_plan_well_formed_body!(
                        $projection.boundary_claimed.replay_plan,
                        $projection.boundary_claimed.replay_effect_kind
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && $projection.event_kind == EVENT_RESUME_AFTER_REPLAY
                    && $projection.event_persistence_id == 0u64
                    && persist_effects == 0u64
            } else {
                boundary_capability_is_absent_body!($projection.boundary_claimed)
                    && boundary_capability_is_absent_body!($projection.boundary_granted)
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && (if persistence_completion {
                        pending_projection_is_absent_body!($projection.pending_before)
                            && $projection.effects.len == 0u8
                    } else {
                        $projection.event_persistence_id == 0u64
                    })
                    && persist_effects == 0u64
            })
    }};
}

macro_rules! production_decision_identity_is_canonical_body {
    ($decision:expr) => {{
        canonical_identity_is_typed_body!(
            $decision.context_id,
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT
        ) && $decision.height > 0u64
            && $decision.proposal_height == $decision.height
            && $decision.proposal_view <= $decision.view
            && ($decision.phase == 1u8 || $decision.phase == 2u8)
            && ($decision.phase != 1u8 || $decision.proposal_view == $decision.view)
            && canonical_identity_is_typed_body!(
                $decision.subject,
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_WIRE_BLOCK_SUBJECT
            )
            && canonical_identity_is_typed_body!(
                $decision.block_hash,
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER
            )
            && canonical_identity_is_typed_body!(
                $decision.payload_hash,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_CANONICAL_PAYLOAD
            )
            && canonical_identity_is_typed_body!(
                $decision.execution_commitment,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_EXECUTION_COMMITMENT
            )
            && canonical_identity_is_typed_body!(
                $decision.executed_block_wire_hash,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_EXECUTED_BLOCK_WIRE
            )
    }};
}

macro_rules! production_decision_identity_equal_body {
    ($left:expr, $right:expr) => {{
        production_decision_identity_is_canonical_body!($left)
            && production_decision_identity_is_canonical_body!($right)
            && canonical_identity_equal_body!($left.context_id, $right.context_id)
            && $left.height == $right.height
            && $left.view == $right.view
            && $left.proposal_height == $right.proposal_height
            && $left.proposal_view == $right.proposal_view
            && $left.phase == $right.phase
            && canonical_identity_equal_body!($left.subject, $right.subject)
            && canonical_identity_equal_body!($left.block_hash, $right.block_hash)
            && canonical_identity_equal_body!($left.payload_hash, $right.payload_hash)
            && canonical_identity_equal_body!(
                $left.execution_commitment,
                $right.execution_commitment
            )
            && canonical_identity_equal_body!(
                $left.executed_block_wire_hash,
                $right.executed_block_wire_hash
            )
    }};
}

macro_rules! production_quorum_certificate_is_canonical_body {
    ($certificate:expr) => {{
        production_decision_identity_is_canonical_body!($certificate.decision)
            && canonical_identity_is_typed_body!(
                $certificate.certificate,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_QUORUM_CERTIFICATE
            )
            && $certificate.signer_count > 0u64
            && $certificate.aggregate_signature_len > 0u64
    }};
}

macro_rules! production_quorum_certificate_equal_body {
    ($left:expr, $right:expr) => {{
        production_quorum_certificate_is_canonical_body!($left)
            && production_quorum_certificate_is_canonical_body!($right)
            && production_decision_identity_equal_body!($left.decision, $right.decision)
            && canonical_identity_equal_body!($left.certificate, $right.certificate)
            && $left.signer_count == $right.signer_count
            && $left.aggregate_signature_len == $right.aggregate_signature_len
    }};
}

macro_rules! production_durable_body_is_canonical_body {
    ($body:expr) => {{
        canonical_identity_is_typed_body!(
            $body.context_id,
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT
        ) && $body.height > 0u64
            && canonical_identity_is_typed_body!(
                $body.subject,
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_WIRE_BLOCK_SUBJECT
            )
            && canonical_identity_is_typed_body!(
                $body.block_hash,
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER
            )
            && canonical_identity_is_typed_body!(
                $body.payload_hash,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_CANONICAL_PAYLOAD
            )
            && canonical_identity_is_typed_body!(
                $body.manifest,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_PAYLOAD_MANIFEST
            )
            && canonical_identity_is_typed_body!(
                $body.frame,
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_DURABLE_BODY_FRAME
            )
    }};
}

macro_rules! production_durable_body_equal_body {
    ($left:expr, $right:expr) => {{
        production_durable_body_is_canonical_body!($left)
            && production_durable_body_is_canonical_body!($right)
            && canonical_identity_equal_body!($left.context_id, $right.context_id)
            && $left.height == $right.height
            && $left.view == $right.view
            && canonical_identity_equal_body!($left.subject, $right.subject)
            && canonical_identity_equal_body!($left.block_hash, $right.block_hash)
            && canonical_identity_equal_body!($left.payload_hash, $right.payload_hash)
            && canonical_identity_equal_body!($left.manifest, $right.manifest)
            && canonical_identity_equal_body!($left.frame, $right.frame)
    }};
}

macro_rules! production_decision_recovery_trace_body {
    ($projection:expr) => {{
        $projection.expected_height > 0u64
            && $projection.state_height <= $projection.expected_height
            && $projection.expected_height - $projection.state_height <= 1u64
            && canonical_identity_is_typed_body!(
                $projection.expected_context_id,
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_WIRE_HEIGHT_CONTEXT
            )
            && canonical_identity_equal_body!(
                $projection.expected_context_id,
                $projection.frozen_context_id
            )
            && $projection.expected_height == $projection.frozen_height
            && $projection.replay_tag.height == $projection.frozen_height
            && tag_projection_equal_body!($projection.replay_tag, $projection.owner_tag)
            && $projection.replay_tag.generation == $projection.replay_generation
            && production_quorum_certificate_is_canonical_body!($projection.commit_qc)
            && $projection.commit_qc.decision.phase == 2u8
            && canonical_identity_equal_body!(
                $projection.commit_qc.decision.context_id,
                $projection.frozen_context_id
            )
            && $projection.commit_qc.decision.height == $projection.frozen_height
            && canonical_identity_equal_body!(
                $projection.commit_qc.decision.block_hash,
                $projection.expected_block_hash
            )
            && $projection.manifest_round.height == $projection.commit_qc.decision.proposal_height
            && $projection.manifest_round.view == $projection.commit_qc.decision.proposal_view
            && $projection.manifest_round.generation == 0u64
            && canonical_identity_equal_body!(
                $projection.manifest_subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_is_typed_body!(
                $projection.manifest,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_PAYLOAD_MANIFEST
            )
            && production_durable_body_equal_body!(
                $projection.durable_body,
                $projection.validated_body
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.context_id,
                $projection.frozen_context_id
            )
            && $projection.durable_body.height == $projection.frozen_height
            && $projection.durable_body.view == $projection.manifest_round.view
            && $projection.durable_body.view == $projection.commit_qc.decision.proposal_view
            && canonical_identity_equal_body!(
                $projection.durable_body.subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.block_hash,
                $projection.commit_qc.decision.block_hash
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.payload_hash,
                $projection.commit_qc.decision.payload_hash
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.manifest,
                $projection.manifest
            )
            && canonical_identity_equal_body!(
                $projection.validated_execution_commitment,
                $projection.commit_qc.decision.execution_commitment
            )
            && $projection.stage == 1u8
    }};
}

macro_rules! production_scheduler_trace_body {
    ($projection:expr) => {{
        if $projection.timeout_due {
            $projection.selected == 1u8 && $projection.fifo_owed_after == $projection.fifo_ready
        } else if $projection.fifo_ready && $projection.fifo_owed_before {
            $projection.selected == 3u8 && !$projection.fifo_owed_after
        } else if $projection.periodic_timer_due {
            $projection.selected == 2u8 && $projection.fifo_owed_after == $projection.fifo_ready
        } else if $projection.fifo_ready {
            $projection.selected == 3u8 && !$projection.fifo_owed_after
        } else {
            $projection.selected == 0u8 && !$projection.fifo_owed_after
        }
    }};
}

macro_rules! production_ingress_identity_and_class_trace_body {
    ($projection:expr) => {{
        $projection.incoming_height == $projection.stored_height
            && $projection.incoming_view == $projection.stored_view
            && $projection.incoming_generation == $projection.stored_generation
            && $projection.incoming_class == $projection.stored_class
            && $projection.incoming_class >= 1u8
            && $projection.incoming_class <= 3u8
            && $projection.queue_len_before < u64::MAX
            && $projection.queue_len_after == $projection.queue_len_before + 1u64
            && $projection.queue_len_after <= $projection.queue_capacity
    }};
}

macro_rules! production_reliable_flush_trace_body {
    ($projection:expr) => {{
        canonical_identity_is_typed_body!(
            $projection.semantic_target,
            IDENTITY_DOMAIN_PEER,
            IDENTITY_KIND_PEER
        ) && canonical_identity_is_typed_body!(
            $projection.authenticated_source,
            IDENTITY_DOMAIN_PEER,
            IDENTITY_KIND_PEER
        ) && canonical_identity_is_typed_body!(
            $projection.requester,
            IDENTITY_DOMAIN_PEER,
            IDENTITY_KIND_PEER
        ) && canonical_identity_is_typed_body!(
            $projection.responder,
            IDENTITY_DOMAIN_PEER,
            IDENTITY_KIND_PEER
        ) && canonical_identity_equal_body!($projection.semantic_target, $projection.requester)
            && $projection.ticket_rank > 0u64
            && $projection.ticket_topic == 3u8
            && canonical_identity_is_typed_body!(
                $projection.canonical_request_digest,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_REPLY_PAYLOAD
            )
            && $projection.stream_wire_bytes > 0u64
            && canonical_identity_is_typed_body!(
                $projection.request_id,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_SIDECAR_REQUEST
            )
            && canonical_identity_is_typed_body!(
                $projection.entry_hash,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_MERGE_ENTRY
            )
            && $projection.encoded_len > 0u64
            && canonical_identity_is_typed_body!(
                $projection.reference_digest,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_REFERENCE_DIGEST
            )
            && canonical_identity_is_typed_body!(
                $projection.canonical_response_hash,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_NETWORK_RESPONSE
            )
            && canonical_identity_is_typed_body!(
                $projection.sidecar_response_hash,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_SIDECAR_RESPONSE
            )
            && canonical_identity_is_typed_body!(
                $projection.chunk_hash,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_SIDECAR_CHUNK
            )
            && canonical_identity_is_typed_body!(
                $projection.payload_digest,
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_SIDECAR_PAYLOAD
            )
            && $projection.chunk_count > 0u64
            && $projection.chunk_index < $projection.chunk_count
            && $projection.message_cursor_before == 0u64
            && $projection.chunk_cursor_before == $projection.chunk_index
            && $projection.flushing_after <= $projection.capacity
            && $projection.admitted_after <= $projection.capacity - $projection.flushing_after
            && (if $projection.status == 1u8 {
                $projection.flushing_after == $projection.flushing_before
                    && $projection.admitted_after == $projection.admitted_before
                    && $projection.message_cursor_after == $projection.message_cursor_before
                    && $projection.chunk_cursor_after == $projection.chunk_cursor_before
            } else if $projection.status == 2u8 {
                $projection.flushing_after < u64::MAX
                    && $projection.flushing_before == $projection.flushing_after + 1u64
                    && $projection.admitted_before < u64::MAX
                    && $projection.admitted_after == $projection.admitted_before + 1u64
                    && $projection.message_cursor_before < u64::MAX
                    && $projection.message_cursor_after == $projection.message_cursor_before + 1u64
                    && $projection.chunk_cursor_before < u64::MAX
                    && $projection.chunk_cursor_after == $projection.chunk_cursor_before + 1u64
            } else if $projection.status == 3u8 {
                $projection.flushing_after < u64::MAX
                    && $projection.flushing_before == $projection.flushing_after + 1u64
                    && $projection.admitted_after == $projection.admitted_before
                    && $projection.message_cursor_after == $projection.message_cursor_before
                    && $projection.chunk_cursor_after == $projection.chunk_cursor_before
            } else {
                false
            })
    }};
}

macro_rules! production_application_trace_body {
    ($projection:expr) => {{
        $projection.context_height > 0u64
            && $projection.task_tag.height == $projection.context_height
            // The lifecycle owner and CommitQC round are distinct domains.
            // Finality may arrive from either side of the local view; only the
            // independently captured current owner must equal the task tag.
            && tag_projection_equal_body!(
                $projection.task_tag,
                $projection.owner_tag
            )
            && $projection.task_tag.generation == $projection.task_generation
            && production_quorum_certificate_is_canonical_body!($projection.commit_qc)
            && $projection.commit_qc.decision.phase == 2u8
            && canonical_identity_equal_body!(
                $projection.context_id,
                $projection.commit_qc.decision.context_id
            )
            && $projection.context_height == $projection.commit_qc.decision.height
            && production_durable_body_is_canonical_body!($projection.validated_body)
            && canonical_identity_equal_body!(
                $projection.validated_body.context_id,
                $projection.context_id
            )
            && $projection.validated_body.height == $projection.context_height
            // Proposal origin and finality round are distinct. The body must
            // match the CommitQC's authenticated immutable origin exactly.
            && $projection.validated_body.view
                == $projection.commit_qc.decision.proposal_view
            && canonical_identity_equal_body!(
                $projection.validated_body.subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_equal_body!(
                $projection.validated_body.block_hash,
                $projection.proposal_block_hash
            )
            && canonical_identity_equal_body!(
                $projection.validated_body.payload_hash,
                $projection.proposal_payload_hash
            )
            && canonical_identity_equal_body!(
                $projection.validated_execution_commitment,
                $projection.commit_qc.decision.execution_commitment
            )
            && canonical_identity_equal_body!(
                $projection.proposal_block_hash,
                $projection.commit_qc.decision.block_hash
            )
            && canonical_identity_equal_body!(
                $projection.proposal_payload_hash,
                $projection.commit_qc.decision.payload_hash
            )
            && canonical_identity_equal_body!(
                $projection.committed_block_hash,
                $projection.commit_qc.decision.block_hash
            )
            && canonical_identity_equal_body!(
                $projection.executed_block_wire_hash,
                $projection.commit_qc.decision.executed_block_wire_hash
            )
            && production_decision_identity_equal_body!(
                $projection.kura_decision,
                $projection.commit_qc.decision
            )
            && canonical_identity_equal_body!(
                $projection.kura_artifact_hash,
                $projection.artifact_hash
            )
            && canonical_identity_equal_body!(
                $projection.artifact_context_id,
                $projection.context_id
            )
            && $projection.artifact_height == $projection.context_height
            && canonical_identity_equal_body!(
                $projection.artifact_subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_equal_body!(
                $projection.artifact_block_hash,
                $projection.committed_block_hash
            )
            && production_quorum_certificate_equal_body!(
                $projection.artifact_commit_qc,
                $projection.commit_qc
            )
            && canonical_identity_is_typed_body!(
                $projection.artifact_hash,
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_FINALITY_ARTIFACT
            )
            && $projection.state_height_after == $projection.context_height
            && $projection.completion_work_id == $projection.task_work_id
    }};
}

// Keep the durability acknowledgement predicate shared between the ordinary
// Rust WAL lifecycle and its Verus proof.  An append receipt is minted only
// after all three adapter completions have happened in this order; the
// expression below states the safety half of that contract without treating a
// successful write or userspace flush as durable storage.
macro_rules! wal_append_acknowledged_body {
    ($write_complete:expr, $flush_complete:expr, $sync_complete:expr $(,)?) => {{ $write_complete && $flush_complete && $sync_complete }};
}

// Canonical header acceptance is also a shared production/Verus expression.
// The byte parser still returns field-specific errors, then crosses this final
// fail-closed gate before exposing any recovered frame.
macro_rules! wal_header_accepted_body {
    (
        $complete:expr,
        $magic_matches:expr,
        $format_matches:expr,
        $actual_protocol:expr,
        $expected_protocol:expr,
        $actual_chain:expr,
        $expected_chain:expr,
        $actual_key:expr,
        $expected_key:expr,
        $checksum_matches:expr $(,)?
    ) => {{
        $complete
            && $magic_matches
            && $format_matches
            && $actual_protocol == $expected_protocol
            && $actual_chain == $expected_chain
            && $actual_key == $expected_key
            && $checksum_matches
    }};
}

// Complete-frame acceptance is intentionally expressed only over parsed
// primitives.  Production and Verus therefore agree on the exact sequence,
// size, hash-link, and checksum corridor even though BLAKE3 itself stays in the
// cryptographic TCB.
macro_rules! wal_complete_frame_valid_body {
    (
        $before_failed_closed:expr,
        $complete:expr,
        $expected_sequence:expr,
        $maximum_sequence:expr,
        $actual_sequence:expr,
        $payload_len:expr,
        $maximum_payload_len:expr,
        $encoded_previous:expr,
        $expected_previous:expr,
        $encoded_hash:expr,
        $calculated_hash:expr $(,)?
    ) => {{
        !$before_failed_closed
            && $complete
            && $expected_sequence < $maximum_sequence
            && $actual_sequence == $expected_sequence
            && $payload_len <= $maximum_payload_len
            && $encoded_previous == $expected_previous
            && $encoded_hash == $calculated_hash
    }};
}

// This is the fixed-width retirement rule used by the executable lifecycle
// projection and Verus.  Presence bits are derived from typed production
// state: a closed FinalizedHeight and its trusted Kura durability receipt.
// Identity equality is deliberately expanded field by field so neither side
// can authorize pruning with a merely same-height or same-subject artifact.
macro_rules! wal_retirement_authorized_body {
    (
        $height_closed:expr,
        $block_durable:expr,
        $certificate_durable:expr,
        $decision_context:expr,
        $decision_height:expr,
        $decision_subject:expr,
        $decision_certificate_context:expr,
        $decision_certificate_height:expr,
        $decision_certificate_view:expr,
        $decision_proposal_height:expr,
        $decision_proposal_view:expr,
        $decision_certificate_phase:expr,
        $commit_phase:expr,
        $decision_certificate_subject:expr,
        $receipt_context:expr,
        $receipt_height:expr,
        $receipt_subject:expr,
        $receipt_certificate_context:expr,
        $receipt_certificate_height:expr,
        $receipt_certificate_view:expr,
        $receipt_proposal_height:expr,
        $receipt_proposal_view:expr,
        $receipt_certificate_phase:expr,
        $receipt_certificate_subject:expr $(,)?
    ) => {{
        $height_closed
            && $block_durable
            && $certificate_durable
            && $decision_context == $receipt_context
            && $decision_height == $receipt_height
            && $decision_subject == $receipt_subject
            && $decision_certificate_context == $receipt_certificate_context
            && $decision_certificate_height == $receipt_certificate_height
            && $decision_certificate_view == $receipt_certificate_view
            && $decision_proposal_height == $receipt_proposal_height
            && $decision_proposal_view == $receipt_proposal_view
            && $decision_certificate_phase == $commit_phase
            && $decision_certificate_phase == $receipt_certificate_phase
            && $decision_certificate_subject == $receipt_certificate_subject
            && $decision_context == $decision_certificate_context
            && $decision_height == $decision_certificate_height
            && $decision_height == $decision_proposal_height
            && $decision_proposal_view <= $decision_certificate_view
            && $decision_subject == $decision_certificate_subject
    }};
}

/// Primitive `(height, view, generation)` lifecycle projection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TagProjection {
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) generation: u64,
}

/// Typed identity of the sole reducer consumer for one exact body pipeline.
///
/// `K` is the complete `(round, subject)` key and `M` is the canonical
/// manifest identity. A missing manifest is permitted only while a certified
/// fetch has not yet acquired the body metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactBodyOwnerProjection<K, M> {
    pub(crate) tag: TagProjection,
    pub(crate) key: K,
    pub(crate) manifest_hash: Option<M>,
}

/// Preflighted exact owner binding derived from typed identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Constructed by the production executor and refinement tests.
pub struct ExactBodyOwnerBindingProjection<K, M> {
    pub(crate) owner: ExactBodyOwnerProjection<K, M>,
    pub(crate) already_owned: bool,
}

/// Bind or monotonically enrich one exact body-pipeline owner.
///
/// The only permitted enrichment changes an absent manifest identity to a
/// present one. The reducer tag, round/subject key, and any existing manifest
/// identity are immutable.
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn plan_exact_body_owner_binding<K, M>(
    current: Option<ExactBodyOwnerProjection<K, M>>,
    incoming: ExactBodyOwnerProjection<K, M>,
) -> Option<ExactBodyOwnerBindingProjection<K, M>>
where
    K: Copy + Eq,
    M: Copy + Eq,
{
    exact_body_owner_binding_body!(
        current,
        incoming,
        ExactBodyOwnerProjection,
        ExactBodyOwnerBindingProjection
    )
}

/// Return whether a pipeline stage carries the exact immutable owner identity.
#[must_use]
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn exact_body_stage_is_owned<K, M>(
    owner: ExactBodyOwnerProjection<K, M>,
    stage: ExactBodyOwnerProjection<K, M>,
) -> bool
where
    K: Copy + Eq,
    M: Copy + Eq,
{
    exact_body_owner_equal_body!(owner, stage)
}

/// Rebind one exact body-pipeline consumer to a strictly newer incarnation.
///
/// The height, round/subject key, and manifest identity remain immutable;
/// the view cannot regress and the generation must strictly advance. This is
/// only a safety transition. It does not claim the asynchronous rebind will be
/// scheduled.
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn plan_exact_body_owner_rebind<K, M>(
    current: ExactBodyOwnerProjection<K, M>,
    previous: ExactBodyOwnerProjection<K, M>,
    rebound_tag: TagProjection,
) -> Option<ExactBodyOwnerProjection<K, M>>
where
    K: Copy + Eq,
    M: Copy + Eq,
{
    exact_body_owner_rebind_body!(current, previous, rebound_tag, ExactBodyOwnerProjection)
}

/// Classification of one logical completion stage across serialized owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Consumed by the production runtime and refinement tests.
pub enum ExactBodyCompletionOwnership {
    /// Neither runtime ingress nor the Busy-deferred lane owns the stage.
    Vacant,
    /// Exactly one lane owns exactly matching trusted evidence.
    Exact,
    /// Evidence conflicts, is duplicated, or its owner count is inconsistent.
    Invalid,
}

/// Classify exact completion ownership across runtime and deferred lanes.
#[must_use]
#[allow(dead_code)] // Called by the production runtime, outside the pure harness crate.
pub fn classify_exact_body_completion_ownership(
    ingress_owners: usize,
    ingress_exact: usize,
    deferred_owners: usize,
    deferred_exact: usize,
) -> ExactBodyCompletionOwnership {
    exact_body_completion_ownership_body!(
        ingress_owners,
        ingress_exact,
        deferred_owners,
        deferred_exact,
        ExactBodyCompletionOwnership::Vacant,
        ExactBodyCompletionOwnership::Exact,
        ExactBodyCompletionOwnership::Invalid,
    )
}

/// Exact residual counters after superseding body-pipeline ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Constructed by the production executor and refinement tests.
pub struct ExactBodyRetirementAccounting {
    pub(crate) ready_after: u64,
    pub(crate) store_after: u64,
}

/// Compute exact body-byte residuals, rejecting any underflow or leakage.
#[must_use]
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn plan_exact_body_retirement_accounting(
    ready_before: u64,
    retained_bytes: u64,
    ready_bytes: u64,
    store_before: u64,
    store_bytes: u64,
) -> Option<ExactBodyRetirementAccounting> {
    exact_body_retirement_accounting_body!(
        ready_before,
        retained_bytes,
        ready_bytes,
        store_before,
        store_bytes,
        ExactBodyRetirementAccounting,
    )
}

/// One exact selection made by the bounded three-class ingress kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Constructed by the production runtime and refinement tests.
pub struct BoundedServiceSelection {
    pub(crate) selected: u8,
    pub(crate) next: u8,
}

/// Select one ready runtime class from the persistent round-robin cursor.
///
/// Invalid cursors fail closed as `(NONE, NONE)`. This function proves only the
/// bounded arbitration decision made when production invokes it; repeated
/// invocation remains an explicit scheduler/host-service premise.
#[must_use]
#[allow(dead_code)] // Called by the production runtime, outside the pure harness crate.
pub fn select_bounded_service_class(
    cursor: u8,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
) -> BoundedServiceSelection {
    bounded_service_selection_body!(
        cursor,
        completion_ready,
        progress_ready,
        normal_ready,
        BoundedServiceSelection,
    )
}

/// Primitive production trace spanning the effective-lock acquisition seams.
///
/// Each action uses one discriminant and canonical zeroes for unrelated
/// fields. The trace contains only facts derived from the live reducer,
/// executor, or runtime state at its enforcement point; no authorization bit
/// is accepted from an external caller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectiveLockTraceProjection {
    pub(crate) kind: u8,
    pub(crate) relation_exact: bool,
    pub(crate) protected_before: u64,
    pub(crate) protected_after: u64,
    pub(crate) owner_before: u64,
    pub(crate) owner_after: u64,
    pub(crate) owner_reused: bool,
    pub(crate) ready_before: u64,
    pub(crate) retired_retained: u64,
    pub(crate) retired_ready: u64,
    pub(crate) ready_after: u64,
    pub(crate) store_before: u64,
    pub(crate) retired_store: u64,
    pub(crate) store_after: u64,
    pub(crate) cursor_before: u8,
    pub(crate) completion_ready: bool,
    pub(crate) progress_ready: bool,
    pub(crate) normal_ready: bool,
    pub(crate) selected: u8,
    pub(crate) cursor_after: u8,
}

/// Primitive durable-intent ownership observed around one reducer step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDurableIntentTraceProjection {
    pub(crate) event_tag: TagProjection,
    pub(crate) owner_tag_before: TagProjection,
    pub(crate) owner_tag_after: TagProjection,
    pub(crate) event_kind: u8,
    pub(crate) event_persistence_id: u64,
    pub(crate) pending_before: PendingProjection,
    pub(crate) pending_after: PendingProjection,
    pub(crate) boundary_claimed: BoundaryCapabilityKey,
    pub(crate) boundary_granted: BoundaryCapabilityKey,
    pub(crate) effects: EffectTrace,
    pub(crate) durable_sequence_before: u64,
    pub(crate) durable_sequence_after: u64,
}

/// Primitive ownership facts for one validated, undecided durable lock.
///
/// The kernel accepts an active exact Commit, an exact pending
/// `LockAndCommit`, or an exact acknowledged timeout for the current view.
/// The last form is a recovery witness only: the reducer still refuses to
/// append or sign a Commit after that timeout closes the view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockedCommitProgressWitnessProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) current_height: u64,
    pub(crate) current_view: u64,
    pub(crate) local_validator_present: bool,
    pub(crate) local_validator: ValidatorId,
    pub(crate) locked_context_id: CanonicalIdentityProjection,
    pub(crate) locked_height: u64,
    pub(crate) locked_view: u64,
    pub(crate) locked_subject: CanonicalIdentityProjection,
    pub(crate) commit_intent_present: bool,
    pub(crate) commit_context_id: CanonicalIdentityProjection,
    pub(crate) commit_height: u64,
    pub(crate) commit_view: u64,
    pub(crate) commit_proposal_height: u64,
    pub(crate) commit_proposal_view: u64,
    pub(crate) commit_phase: u8,
    pub(crate) commit_subject: CanonicalIdentityProjection,
    pub(crate) commit_signer: ValidatorId,
    pub(crate) commit_signature_pending: bool,
    pub(crate) commit_pooled: bool,
    pub(crate) pending: PendingProjection,
    pub(crate) timeout_intent_present: bool,
    pub(crate) timeout_intent_durable: bool,
    pub(crate) timeout_context_id: CanonicalIdentityProjection,
    pub(crate) timeout_height: u64,
    pub(crate) timeout_view: u64,
    pub(crate) timeout_signer: ValidatorId,
}

/// Check that a validated lock retains one exact durable progress witness.
#[must_use]
pub(crate) fn locked_commit_progress_witness_is_valid(
    projection: LockedCommitProgressWitnessProjection,
) -> bool {
    locked_commit_progress_witness_body!(projection)
}

/// Complete semantic decision identity repeated by a certificate or receipt.
///
/// Every digest is a lossless four-word projection of an existing canonical
/// 256-bit identity. The projection never hashes or truncates inside the pure
/// kernel; production supplies identities derived at the typed protocol seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDecisionIdentityProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) proposal_height: u64,
    pub(crate) proposal_view: u64,
    pub(crate) phase: u8,
    pub(crate) subject: CanonicalIdentityProjection,
    pub(crate) block_hash: CanonicalIdentityProjection,
    pub(crate) payload_hash: CanonicalIdentityProjection,
    pub(crate) execution_commitment: CanonicalIdentityProjection,
    pub(crate) executed_block_wire_hash: CanonicalIdentityProjection,
}

/// Complete quorum-certificate identity at a production refinement seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionQuorumCertificateIdentityProjection {
    pub(crate) decision: ProductionDecisionIdentityProjection,
    pub(crate) certificate: CanonicalIdentityProjection,
    pub(crate) signer_count: u64,
    pub(crate) aggregate_signature_len: u64,
}

/// Exact durable-body identity shared by recovery and application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDurableBodyIdentityProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) subject: CanonicalIdentityProjection,
    pub(crate) block_hash: CanonicalIdentityProjection,
    pub(crate) payload_hash: CanonicalIdentityProjection,
    pub(crate) manifest: CanonicalIdentityProjection,
    pub(crate) frame: CanonicalIdentityProjection,
}

/// Primitive pending-Decision recovery boundary selected during startup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDecisionRecoveryTraceProjection {
    pub(crate) state_height: u64,
    pub(crate) expected_context_id: CanonicalIdentityProjection,
    pub(crate) expected_height: u64,
    pub(crate) expected_block_hash: CanonicalIdentityProjection,
    pub(crate) frozen_context_id: CanonicalIdentityProjection,
    pub(crate) frozen_height: u64,
    pub(crate) replay_tag: TagProjection,
    pub(crate) owner_tag: TagProjection,
    pub(crate) replay_generation: u64,
    pub(crate) commit_qc: ProductionQuorumCertificateIdentityProjection,
    pub(crate) manifest_round: TagProjection,
    pub(crate) manifest_subject: CanonicalIdentityProjection,
    pub(crate) manifest: CanonicalIdentityProjection,
    pub(crate) durable_body: ProductionDurableBodyIdentityProjection,
    pub(crate) validated_body: ProductionDurableBodyIdentityProjection,
    pub(crate) validated_execution_commitment: CanonicalIdentityProjection,
    pub(crate) stage: u8,
}

/// Primitive scheduler input and its concrete selected owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionSchedulerTraceProjection {
    pub(crate) fifo_owed_before: bool,
    pub(crate) timeout_due: bool,
    pub(crate) periodic_timer_due: bool,
    pub(crate) fifo_ready: bool,
    pub(crate) selected: u8,
    pub(crate) fifo_owed_after: bool,
}

/// Primitive identity and queue-class observation for one admitted command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionIngressIdentityAndClassTraceProjection {
    pub(crate) incoming_height: u64,
    pub(crate) incoming_view: u64,
    pub(crate) incoming_generation: u64,
    pub(crate) incoming_class: u8,
    pub(crate) stored_height: u64,
    pub(crate) stored_view: u64,
    pub(crate) stored_generation: u64,
    pub(crate) stored_class: u8,
    pub(crate) queue_len_before: u64,
    pub(crate) queue_len_after: u64,
    pub(crate) queue_capacity: u64,
}

/// Primitive per-source sidecar-flush cursor movement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionReliableFlushTraceProjection {
    pub(crate) status: u8,
    pub(crate) semantic_target: CanonicalIdentityProjection,
    pub(crate) authenticated_source: CanonicalIdentityProjection,
    pub(crate) requester: CanonicalIdentityProjection,
    pub(crate) responder: CanonicalIdentityProjection,
    pub(crate) connection_tenure_ordinal_high: u64,
    pub(crate) connection_tenure_ordinal_low: u64,
    pub(crate) delivery_ordinal_high: u64,
    pub(crate) delivery_ordinal_low: u64,
    pub(crate) ticket_id: u64,
    pub(crate) ticket_rank: u64,
    pub(crate) ticket_topic: u8,
    pub(crate) canonical_request_digest: CanonicalIdentityProjection,
    pub(crate) stream_wire_bytes: u64,
    pub(crate) request_id: CanonicalIdentityProjection,
    pub(crate) entry_hash: CanonicalIdentityProjection,
    pub(crate) encoded_len: u64,
    pub(crate) epoch_id: u64,
    pub(crate) reference_digest: CanonicalIdentityProjection,
    pub(crate) canonical_response_hash: CanonicalIdentityProjection,
    pub(crate) sidecar_response_hash: CanonicalIdentityProjection,
    pub(crate) chunk_hash: CanonicalIdentityProjection,
    pub(crate) payload_digest: CanonicalIdentityProjection,
    pub(crate) chunk_index: u64,
    pub(crate) chunk_count: u64,
    pub(crate) message_cursor_before: u64,
    pub(crate) message_cursor_after: u64,
    pub(crate) chunk_cursor_before: u64,
    pub(crate) chunk_cursor_after: u64,
    pub(crate) flushing_before: u64,
    pub(crate) flushing_after: u64,
    pub(crate) admitted_before: u64,
    pub(crate) admitted_after: u64,
    pub(crate) capacity: u64,
}

/// Primitive durable application completion returned to the reducer owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionApplicationTraceProjection {
    pub(crate) task_tag: TagProjection,
    pub(crate) owner_tag: TagProjection,
    pub(crate) task_generation: u64,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) context_height: u64,
    pub(crate) commit_qc: ProductionQuorumCertificateIdentityProjection,
    pub(crate) validated_body: ProductionDurableBodyIdentityProjection,
    pub(crate) validated_execution_commitment: CanonicalIdentityProjection,
    pub(crate) proposal_block_hash: CanonicalIdentityProjection,
    pub(crate) proposal_payload_hash: CanonicalIdentityProjection,
    pub(crate) committed_block_hash: CanonicalIdentityProjection,
    pub(crate) executed_block_wire_hash: CanonicalIdentityProjection,
    pub(crate) kura_decision: ProductionDecisionIdentityProjection,
    pub(crate) kura_artifact_hash: CanonicalIdentityProjection,
    pub(crate) artifact_context_id: CanonicalIdentityProjection,
    pub(crate) artifact_height: u64,
    pub(crate) artifact_subject: CanonicalIdentityProjection,
    pub(crate) artifact_block_hash: CanonicalIdentityProjection,
    pub(crate) artifact_commit_qc: ProductionQuorumCertificateIdentityProjection,
    pub(crate) artifact_hash: CanonicalIdentityProjection,
    pub(crate) state_height_after: u64,
    pub(crate) task_work_id: u64,
    pub(crate) completion_work_id: u64,
}

const fn effective_lock_trace_step_is_valid(projection: EffectiveLockTraceProjection) -> bool {
    effective_lock_trace_step_body!(projection)
}

/// Primitive identity of one optional quorum certificate.
///
/// The signer bitmap is indexed by the frozen, canonically ordered voting
/// roster. `evidence_class` is assigned by full concrete
/// [`super::QuorumCertificate`] equality against the local and incoming
/// transition anchors, so signature/evidence changes cannot hide behind an
/// equal stable certificate reference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CertificateIdentityProjection {
    pub(crate) present: bool,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) phase: u8,
    pub(crate) subject: CanonicalIdentityProjection,
    pub(crate) signer_bitmap: u128,
    pub(crate) signer_bitmap_count: u64,
    pub(crate) signer_count: u64,
    pub(crate) voting_power: u64,
    pub(crate) evidence_class: u8,
}

/// Primitive identity of one optional timeout certificate and its selected
/// highest `PrepareQC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeoutIdentityProjection {
    pub(crate) present: bool,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) highest_prepare: CertificateIdentityProjection,
}

/// Exact production projection of a persisted-TC `EnterView` macro-step.
///
/// This relation is deliberately limited to the reducer-owned transition. It
/// proves which lock crosses the serialized view boundary and which immediate
/// recovery fetch names it; asynchronous ownership and scheduler fairness stay
/// separate obligations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnterViewProjection {
    pub(crate) active: bool,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) before_tag: TagProjection,
    pub(crate) after_tag: TagProjection,
    pub(crate) pending_record_kind: u8,
    pub(crate) pending_continuation: u8,
    pub(crate) pending_record_timeout: TimeoutIdentityProjection,
    pub(crate) pending_continuation_timeout: TimeoutIdentityProjection,
    pub(crate) durable_timeout_after: TimeoutIdentityProjection,
    pub(crate) effect_timeout: TimeoutIdentityProjection,
    pub(crate) local_lock_before: CertificateIdentityProjection,
    pub(crate) durable_lock_after: CertificateIdentityProjection,
    pub(crate) effect_protected_lock: CertificateIdentityProjection,
    pub(crate) following_fetch_lock: CertificateIdentityProjection,
    pub(crate) enter_count: u64,
    pub(crate) fetch_count: u64,
    pub(crate) enter_index: u8,
    pub(crate) following_fetch_index: u8,
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
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    /// Whether the WAL record authenticates a proposal-body origin.
    pub(crate) proposal_present: bool,
    /// Immutable proposal-origin height, or zero when absent.
    pub(crate) proposal_height: u64,
    /// Immutable proposal-origin view, or zero when absent.
    pub(crate) proposal_view: u64,
    pub(crate) subject: CanonicalIdentityProjection,
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
    pub(crate) proposal_height: u64,
    pub(crate) proposal_view: u64,
    pub(crate) phase: u8,
    pub(crate) subject: Subject,
    pub(crate) actor: ValidatorId,
    pub(crate) persistence_id: u64,
    pub(crate) record_kind: u8,
    pub(crate) auxiliary_present: bool,
    pub(crate) auxiliary_context_id: ContextId,
    pub(crate) auxiliary_height: u64,
    pub(crate) auxiliary_view: u64,
    pub(crate) auxiliary_proposal_height: u64,
    pub(crate) auxiliary_proposal_view: u64,
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
            proposal_height: 0,
            proposal_view: 0,
            phase: 0,
            subject: Subject::repeat(0),
            actor: ValidatorId::repeat(0),
            persistence_id: 0,
            record_kind: WAL_RECORD_NONE,
            auxiliary_present: false,
            auxiliary_context_id: ContextId::repeat(0),
            auxiliary_height: 0,
            auxiliary_view: 0,
            auxiliary_proposal_height: 0,
            auxiliary_proposal_view: 0,
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

/// One exact item in the durable replay FIFO.
///
/// `kind` uses the [`REPLAY_EFFECT_*`] discriminants.  Proposal, Prepare,
/// Commit, and timeout items carry an [`EFFECT_SIGN`] capability; a Decision
/// carries the one [`EFFECT_FETCH`] frontier reconstructed on recovery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayPlanSlotProjection {
    pub(crate) kind: u8,
    pub(crate) capability: EffectCapabilityKey,
}

impl ReplayPlanSlotProjection {
    /// Canonical absent replay-plan slot.
    pub(crate) const fn none() -> Self {
        Self {
            kind: REPLAY_EFFECT_NONE,
            capability: EffectCapabilityKey::none(),
        }
    }
}

/// Fixed projection of the complete reducer-owned replay FIFO.
///
/// Recovery can reconstruct at most three signable intents, in this order:
/// current Timeout-or-Proposal, current Prepare, and the exact locked Commit.
/// A durable Decision is instead represented by one body-fetch frontier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayPlanProjection {
    pub(crate) len: u8,
    pub(crate) slot0: ReplayPlanSlotProjection,
    pub(crate) slot1: ReplayPlanSlotProjection,
    pub(crate) slot2: ReplayPlanSlotProjection,
}

impl ReplayPlanProjection {
    /// Construct an empty canonical replay plan.
    pub(crate) const fn empty() -> Self {
        Self {
            len: 0,
            slot0: ReplayPlanSlotProjection::none(),
            slot1: ReplayPlanSlotProjection::none(),
            slot2: ReplayPlanSlotProjection::none(),
        }
    }

    /// Append one exact replay item, failing closed beyond the protocol bound.
    pub(crate) fn push(&mut self, kind: u8, capability: EffectCapabilityKey) -> bool {
        let slot = ReplayPlanSlotProjection { kind, capability };
        match self.len {
            0 => self.slot0 = slot,
            1 => self.slot1 = slot,
            2 => self.slot2 = slot,
            _ => return false,
        }
        self.len += 1;
        true
    }
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
    pub(crate) context_identity: CanonicalIdentityProjection,
    pub(crate) tag: TagProjection,
    pub(crate) subject: SubjectProjection,
    pub(crate) subject_identity: CanonicalIdentityProjection,
    /// Whether the primary WAL identity authenticates a proposal-body origin.
    pub(crate) proposal_present: bool,
    /// Immutable primary proposal-origin height, or zero when absent.
    pub(crate) proposal_height: u64,
    /// Immutable primary proposal-origin view, or zero when absent.
    pub(crate) proposal_view: u64,
    /// Whether the WAL record carries an auxiliary certificate reference.
    pub(crate) auxiliary_present: bool,
    pub(crate) auxiliary_context_id: ContextId,
    pub(crate) auxiliary_height: u64,
    pub(crate) auxiliary_view: u64,
    pub(crate) auxiliary_proposal_height: u64,
    pub(crate) auxiliary_proposal_view: u64,
    pub(crate) auxiliary_phase: u8,
    pub(crate) auxiliary_subject: Subject,
    pub(crate) replay_plan: ReplayPlanProjection,
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
            context_identity: CanonicalIdentityProjection::zero(),
            tag: TagProjection {
                height: 0,
                view: 0,
                generation: 0,
            },
            subject: SubjectProjection {
                present: false,
                subject: Subject::repeat(0),
            },
            subject_identity: CanonicalIdentityProjection::zero(),
            proposal_present: false,
            proposal_height: 0,
            proposal_view: 0,
            auxiliary_present: false,
            auxiliary_context_id: ContextId::repeat(0),
            auxiliary_height: 0,
            auxiliary_view: 0,
            auxiliary_proposal_height: 0,
            auxiliary_proposal_view: 0,
            auxiliary_phase: 0,
            auxiliary_subject: Subject::repeat(0),
            replay_plan: ReplayPlanProjection::empty(),
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
    pub(crate) enter_view: EnterViewProjection,
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
    enter_view_exact: bool,
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
            && $left.proposal_height == $right.proposal_height
            && $left.proposal_view == $right.proposal_view
            && $left.phase == $right.phase
            && $left.subject == $right.subject
            && $left.actor == $right.actor
            && $left.persistence_id == $right.persistence_id
            && $left.record_kind == $right.record_kind
            && $left.auxiliary_present == $right.auxiliary_present
            && $left.auxiliary_context_id == $right.auxiliary_context_id
            && $left.auxiliary_height == $right.auxiliary_height
            && $left.auxiliary_view == $right.auxiliary_view
            && $left.auxiliary_proposal_height == $right.auxiliary_proposal_height
            && $left.auxiliary_proposal_view == $right.auxiliary_proposal_view
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

macro_rules! final_effect_kind_body {
    ($trace:expr, $kind:expr) => {{
        match $trace.len {
            1 => $trace.slot0.kind == $kind,
            2 => $trace.slot1.kind == $kind,
            3 => $trace.slot2.kind == $kind,
            4 => $trace.slot3.kind == $kind,
            5 => $trace.slot4.kind == $kind,
            6 => $trace.slot5.kind == $kind,
            7 => $trace.slot6.kind == $kind,
            8 => $trace.slot7.kind == $kind,
            _ => false,
        }
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
            // one-effect transitions in their exact pipeline order. A
            // retransmission tick may reconstruct the one lost Decision-stage
            // owner after retransmitting retained control messages; in that
            // case the reconstructed body effect is last and cannot coexist
            // with another body-stage, fetch, or report effect.
            && ($store_count == 0u64
                || (($trace.len == 1u8 && $event_kind == 8u8)
                    || $event_kind == 7u8))
            && ($validate_count == 0u64
                || (($trace.len == 1u8 && $event_kind == 9u8)
                    || $event_kind == 7u8))
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
            // evolve to request several equivalent certified sources in other
            // transitions, but all exact slots still require concrete
            // authorization above. The current Decision retransmission path is
            // deliberately narrower: retained broadcasts followed by at most
            // one exact pipeline-stage owner.
            && $fetch_count <= 8u64
            && ($event_kind != 7u8
                || (effect_count_body!($trace, 9u8) == 0u64
                    && (($fetch_count == 0u64
                        && $store_count == 0u64
                        && $validate_count == 0u64
                        && $apply_count == 0u64)
                        || ($fetch_count == 1u64
                            && $store_count == 0u64
                            && $validate_count == 0u64
                            && $apply_count == 0u64
                            && final_effect_kind_body!($trace, 2u8))
                        || ($fetch_count == 0u64
                            && $store_count == 1u64
                            && $validate_count == 0u64
                            && $apply_count == 0u64
                            && final_effect_kind_body!($trace, 3u8))
                        || ($fetch_count == 0u64
                            && $store_count == 0u64
                            && $validate_count == 1u64
                            && $apply_count == 0u64
                            && final_effect_kind_body!($trace, 4u8))
                        || ($fetch_count == 0u64
                            && $store_count == 0u64
                            && $validate_count == 0u64
                            && $apply_count == 1u64
                            && final_effect_kind_body!($trace, 7u8)))))
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
            // At most two active phase pools are kept: current Prepare plus
            // either current Commit or the exact historical locked Commit.
            // A newly durable lock retires the superseded historical pool
            // before its current-round Commit signature can complete.
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

macro_rules! replay_plan_slot_well_formed_body {
    ($slot:expr, $active:expr) => {{
        if $active {
            $slot.kind >= 1u8
                && $slot.kind <= 5u8
                && $slot.capability.kind == if $slot.kind == 5u8 { 2u8 } else { 5u8 }
        } else {
            $slot.kind == 0u8 && $slot.capability.kind == 0u8
        }
    }};
}

macro_rules! replay_plan_well_formed_body {
    ($plan:expr, $effect_kind:expr) => {{
        $plan.len <= 3u8
            && replay_plan_slot_well_formed_body!($plan.slot0, $plan.len > 0u8)
            && replay_plan_slot_well_formed_body!($plan.slot1, $plan.len > 1u8)
            && replay_plan_slot_well_formed_body!($plan.slot2, $plan.len > 2u8)
            && (if $plan.len == 0u8 {
                $effect_kind == 0u8
            } else {
                $effect_kind == $plan.slot0.kind
            })
    }};
}

macro_rules! replay_plan_equal_body {
    ($left:expr, $right:expr) => {{
        $left.len == $right.len
            && $left.slot0.kind == $right.slot0.kind
            && capability_key_equal_body!($left.slot0.capability, $right.slot0.capability)
            && $left.slot1.kind == $right.slot1.kind
            && capability_key_equal_body!($left.slot1.capability, $right.slot1.capability)
            && $left.slot2.kind == $right.slot2.kind
            && capability_key_equal_body!($left.slot2.capability, $right.slot2.capability)
    }};
}

macro_rules! boundary_capability_equal_body {
    ($left:expr, $right:expr) => {{
        $left.kind == $right.kind
            && $left.record_kind == $right.record_kind
            && $left.continuation == $right.continuation
            && $left.replay_effect_kind == $right.replay_effect_kind
            && $left.persistence_id == $right.persistence_id
            && $left.context_id == $right.context_id
            && canonical_identity_equal_body!($left.context_identity, $right.context_identity)
            && $left.tag.height == $right.tag.height
            && $left.tag.view == $right.tag.view
            && $left.tag.generation == $right.tag.generation
            && subject_projection_equal_body!($left.subject, $right.subject)
            && canonical_identity_equal_body!($left.subject_identity, $right.subject_identity)
            && $left.proposal_present == $right.proposal_present
            && $left.proposal_height == $right.proposal_height
            && $left.proposal_view == $right.proposal_view
            && $left.auxiliary_present == $right.auxiliary_present
            && $left.auxiliary_context_id == $right.auxiliary_context_id
            && $left.auxiliary_height == $right.auxiliary_height
            && $left.auxiliary_view == $right.auxiliary_view
            && $left.auxiliary_proposal_height == $right.auxiliary_proposal_height
            && $left.auxiliary_proposal_view == $right.auxiliary_proposal_view
            && $left.auxiliary_phase == $right.auxiliary_phase
            && $left.auxiliary_subject == $right.auxiliary_subject
            && replay_plan_well_formed_body!($left.replay_plan, $left.replay_effect_kind)
            && replay_plan_well_formed_body!($right.replay_plan, $right.replay_effect_kind)
            && replay_plan_equal_body!($left.replay_plan, $right.replay_plan)
    }};
}

macro_rules! boundary_capability_is_absent_body {
    ($boundary:expr) => {{
        $boundary.kind == BOUNDARY_NONE
            && $boundary.record_kind == WAL_RECORD_NONE
            && $boundary.continuation == CONTINUATION_NONE
            && $boundary.replay_effect_kind == REPLAY_EFFECT_NONE
            && $boundary.persistence_id == 0u64
            && canonical_identity_is_zero_body!($boundary.context_identity)
            && $boundary.tag.height == 0u64
            && $boundary.tag.view == 0u64
            && $boundary.tag.generation == 0u64
            && !$boundary.subject.present
            && canonical_identity_is_zero_body!($boundary.subject_identity)
            && !$boundary.proposal_present
            && $boundary.proposal_height == 0u64
            && $boundary.proposal_view == 0u64
            && !$boundary.auxiliary_present
            && $boundary.replay_plan.len == 0u8
            && replay_plan_well_formed_body!($boundary.replay_plan, REPLAY_EFFECT_NONE)
    }};
}

macro_rules! boundary_identity_is_canonical_body {
    ($boundary:expr) => {{
        canonical_identity_is_typed_body!(
            $boundary.context_identity,
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_CONSENSUS_CONTEXT
        ) && (if $boundary.subject.present {
            canonical_identity_is_typed_body!(
                $boundary.subject_identity,
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_CONSENSUS_SUBJECT
            )
        } else {
            canonical_identity_is_zero_body!($boundary.subject_identity)
        }) && (if $boundary.proposal_present {
            $boundary.proposal_height > 0u64
        } else {
            $boundary.proposal_height == 0u64 && $boundary.proposal_view == 0u64
        }) && (if $boundary.auxiliary_present {
            $boundary.auxiliary_height > 0u64
                && ($boundary.auxiliary_phase == 1u8 || $boundary.auxiliary_phase == 2u8)
        } else {
            true
        })
    }};
}

macro_rules! certificate_identity_is_canonical_body {
    ($certificate:expr) => {{
        if !$certificate.present {
            canonical_identity_is_zero_body!($certificate.context_id)
                && $certificate.height == 0u64
                && $certificate.view == 0u64
                && $certificate.phase == 0u8
                && canonical_identity_is_zero_body!($certificate.subject)
                && $certificate.signer_bitmap == 0u128
                && $certificate.signer_bitmap_count == 0u64
                && $certificate.signer_count == 0u64
                && $certificate.voting_power == 0u64
                && $certificate.evidence_class == CERTIFICATE_EVIDENCE_ABSENT
        } else {
            canonical_identity_is_typed_body!(
                $certificate.context_id,
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_CONSENSUS_CONTEXT
            ) && canonical_identity_is_typed_body!(
                $certificate.subject,
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_CONSENSUS_SUBJECT
            ) && $certificate.signer_bitmap != 0u128
                && $certificate.signer_bitmap_count > 0u64
                && $certificate.signer_bitmap_count == $certificate.signer_count
                && $certificate.signer_count > 0u64
                && $certificate.signer_count <= 128u64
                && $certificate.voting_power > 0u64
                && ($certificate.evidence_class == CERTIFICATE_EVIDENCE_LOCAL
                    || $certificate.evidence_class == CERTIFICATE_EVIDENCE_INCOMING)
        }
    }};
}

macro_rules! certificate_identity_equal_body {
    ($left:expr, $right:expr) => {{
        certificate_identity_is_canonical_body!($left)
            && certificate_identity_is_canonical_body!($right)
            && $left.present == $right.present
            && (!$left.present
                || (canonical_identity_equal_body!($left.context_id, $right.context_id)
                    && $left.height == $right.height
                    && $left.view == $right.view
                    && $left.phase == $right.phase
                    && canonical_identity_equal_body!($left.subject, $right.subject)
                    && $left.signer_bitmap == $right.signer_bitmap
                    && $left.signer_bitmap_count == $right.signer_bitmap_count
                    && $left.signer_count == $right.signer_count
                    && $left.voting_power == $right.voting_power
                    && $left.evidence_class == $right.evidence_class))
    }};
}

macro_rules! timeout_identity_equal_body {
    ($left:expr, $right:expr) => {{
        certificate_identity_is_canonical_body!($left.highest_prepare)
            && certificate_identity_is_canonical_body!($right.highest_prepare)
            && $left.present == $right.present
            && ($left.present || !$left.highest_prepare.present)
            && ($right.present || !$right.highest_prepare.present)
            && (!$left.present
                || (canonical_identity_equal_body!($left.context_id, $right.context_id)
                    && $left.height == $right.height
                    && $left.view == $right.view
                    && certificate_identity_equal_body!(
                        $left.highest_prepare,
                        $right.highest_prepare
                    )))
    }};
}

macro_rules! prepare_identity_in_context_body {
    ($certificate:expr, $context_id:expr, $height:expr, $maximum_view:expr) => {{
        certificate_identity_is_canonical_body!($certificate)
            && (!$certificate.present
                || (canonical_identity_equal_body!($certificate.context_id, $context_id)
                    && $certificate.height == $height
                    && $certificate.phase == 1u8
                    && $certificate.view <= $maximum_view))
    }};
}

// Preserve every PrepareQC position through the post-WAL EnterView seam. The
// local and incoming anchors are independently authenticated by the concrete
// reducer. Their evidence classes are then copied through pending,
// continuation, durable, effect, and immediate-fetch positions. In
// particular, no FOREIGN class is canonical and no equality can omit signer
// set, quorum totals, phase, or full concrete evidence identity.
macro_rules! enter_view_locked_prepare_qc_identity_body {
    ($projection:expr) => {{
        let timeout = $projection.pending_record_timeout;
        let local = $projection.local_lock_before;
        let incoming = timeout.highest_prepare;
        let selected = if !local.present {
            incoming
        } else if !incoming.present || incoming.view <= local.view {
            local
        } else {
            incoming
        };
        certificate_identity_is_canonical_body!(local)
            && (!local.present || local.evidence_class == CERTIFICATE_EVIDENCE_LOCAL)
            && certificate_identity_is_canonical_body!(incoming)
            && timeout_identity_equal_body!(timeout, $projection.pending_continuation_timeout)
            && timeout_identity_equal_body!(timeout, $projection.durable_timeout_after)
            && timeout_identity_equal_body!(timeout, $projection.effect_timeout)
            && certificate_identity_equal_body!($projection.durable_lock_after, selected)
            && certificate_identity_equal_body!(
                $projection.effect_protected_lock,
                $projection.durable_lock_after
            )
            && (if selected.present {
                certificate_identity_equal_body!($projection.following_fetch_lock, selected)
            } else {
                certificate_identity_is_canonical_body!($projection.following_fetch_lock)
                    && !$projection.following_fetch_lock.present
            })
    }};
}

// This expression is instantiated both by the concrete reducer and Verus. It
// describes the exact post-install lock choice, without claiming anything
// about asynchronous executor ownership or temporal fairness.
macro_rules! enter_view_projection_gate_body {
    ($projection:expr) => {{
        if !$projection.active {
            $projection.enter_count == 0u64
        } else {
            let timeout = $projection.pending_record_timeout;
            let local = $projection.local_lock_before;
            let incoming = timeout.highest_prepare;
            let selected = if !local.present {
                incoming
            } else if !incoming.present || incoming.view <= local.view {
                local
            } else {
                incoming
            };
            let strict_same_round_upgrade = timeout.view < u64::MAX
                && timeout.view + 1u64 == $projection.before_tag.view
                && incoming.present
                && (!local.present || incoming.view > local.view);
            $projection.enter_count == 1u64
                && enter_view_locked_prepare_qc_identity_body!($projection)
                && $projection.pending_record_kind == 6u8
                && $projection.pending_continuation == 2u8
                && timeout.present
                && canonical_identity_equal_body!(timeout.context_id, $projection.context_id)
                && timeout.height == $projection.before_tag.height
                && $projection.after_tag.height == $projection.before_tag.height
                && ($projection.before_tag.view <= timeout.view || strict_same_round_upgrade)
                && timeout.view < u64::MAX
                && $projection.after_tag.view == timeout.view + 1u64
                && $projection.before_tag.generation < u64::MAX
                && $projection.after_tag.generation == $projection.before_tag.generation + 1u64
                && prepare_identity_in_context_body!(
                    local,
                    $projection.context_id,
                    $projection.before_tag.height,
                    $projection.before_tag.view
                )
                && prepare_identity_in_context_body!(
                    incoming,
                    $projection.context_id,
                    $projection.before_tag.height,
                    timeout.view
                )
                && (!(local.present && incoming.present && local.view == incoming.view)
                    || canonical_identity_equal_body!(local.subject, incoming.subject))
                && (if selected.present {
                    $projection.fetch_count == 1u64
                        && $projection.enter_index < 254u8
                        && $projection.following_fetch_index == $projection.enter_index + 1u8
                } else {
                    $projection.fetch_count == 0u64 && !$projection.following_fetch_lock.present
                })
        }
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
        let enter_view_exact = enter_view_projection_gate_body!($projection.enter_view)
            && $projection.enter_view.enter_count == effect_count_body!($projection.effects, 8u8)
            && $projection.enter_view.fetch_count == effect_count_body!($projection.effects, 2u8);
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
            enter_view_exact,
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
            && ($facts.event_kind == 7u8 || effect_count_body!($facts.effects, 3u8) == 0u64)
            && ($facts.event_kind == 7u8 || effect_count_body!($facts.effects, 4u8) == 0u64)
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
            // A current, serviceable first ResumeAfterReplay event may not be
            // reclassified as an ordinary volatile action when its exact
            // boundary grant fails. Stale tags and duplicate resume events
            // remain ordinary empty stutters.
            && ($facts.event_kind != 15u8
                || !$facts.tag_matches
                || !$facts.busy_fence_open
                || (if $facts.volatile_before.replay_resumed {
                    $facts.action_kind == 0u8
                } else {
                    $facts.action_kind == 6u8
                }))
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
                // LocalProposalReady may recover Apply only when the concrete
                // capability grant binds its exact trusted manifest to the
                // durable decision.  The branch gate admits the event class;
                // requested/granted key equality rejects every non-exact use.
                && ($apply_count == 0u64
                    || $facts.event_kind == 0u8
                    || $facts.event_kind == 7u8
                    || $facts.event_kind == 10u8)
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
            && $facts.enter_view_exact
            && $branch_gate($facts)
    }};
}

/// Validate the exact post-WAL effective-lock selection trace.
pub(crate) const fn production_enter_view_uses_post_install_effective_lock_kernel(
    trace: EffectiveLockTraceProjection,
    enter_view: EnterViewProjection,
) -> bool {
    effective_lock_trace_claim_body!(trace, 1u8)
        && enter_view_locked_prepare_qc_identity_body!(enter_view)
}

/// Validate monotonic exact body-pipeline ownership.
pub(crate) const fn production_body_ownership_preserves_effective_lock_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 2u8)
}

/// Validate exact byte/capacity retirement for superseded body ownership.
pub(crate) const fn production_body_capacity_retirement_preserves_effective_lock_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 3u8)
}

/// Validate one exact bounded fair-service selection.
pub(crate) const fn production_body_service_refines_async_fairness_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 4u8)
}

/// Validate one reducer step's durable intent owner and WAL cursor movement.
pub(crate) fn production_durable_intent_trace_refines_progress_witness_kernel(
    projection: ProductionDurableIntentTraceProjection,
) -> bool {
    production_durable_intent_trace_body!(projection)
}

/// Validate startup reconstruction of one durable pending Decision.
pub(crate) const fn production_decision_trace_refines_recovery_witness_kernel(
    projection: ProductionDecisionRecoveryTraceProjection,
) -> bool {
    production_decision_recovery_trace_body!(projection)
}

/// Validate exact scheduler selection and the resulting FIFO debt owner.
pub(crate) const fn production_scheduler_trace_refines_protected_ownership_kernel(
    projection: ProductionSchedulerTraceProjection,
) -> bool {
    production_scheduler_trace_body!(projection)
}

/// Validate ingress identity preservation and its admitted service class.
pub(crate) const fn production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
    projection: ProductionIngressIdentityAndClassTraceProjection,
) -> bool {
    production_ingress_identity_and_class_trace_body!(projection)
}

/// Validate that sidecar delivery ownership moves only after exact flush.
pub(crate) const fn production_reliable_flush_trace_refines_outbound_ownership_kernel(
    projection: ProductionReliableFlushTraceProjection,
) -> bool {
    production_reliable_flush_trace_body!(projection)
}

/// Validate the exact durable application completion exposed to the reducer.
pub(crate) const fn production_application_trace_refines_decision_completion_kernel(
    projection: ProductionApplicationTraceProjection,
) -> bool {
    production_application_trace_body!(projection)
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
    if projection.enter_view.active {
        let enter_view = projection.enter_view;
        let protected_after = u64::from(enter_view.durable_lock_after.present);
        let ownership_after = u64::from(enter_view.following_fetch_lock.present);
        let trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
            relation_exact: enter_view_projection_gate_body!(enter_view)
                && enter_view.enter_count == effect_count_body!(projection.effects, 8u8)
                && enter_view.fetch_count == effect_count_body!(projection.effects, 2u8),
            protected_before: protected_after,
            protected_after: u64::from(enter_view.effect_protected_lock.present),
            owner_before: enter_view.fetch_count,
            owner_after: ownership_after,
            owner_reused: false,
            ready_before: 0,
            retired_retained: 0,
            retired_ready: 0,
            ready_after: 0,
            store_before: 0,
            retired_store: 0,
            store_after: 0,
            cursor_before: 0,
            completion_ready: false,
            progress_ready: false,
            normal_ready: false,
            selected: 0,
            cursor_after: 0,
        };
        if !production_enter_view_uses_post_install_effective_lock_kernel(trace, enter_view) {
            return false;
        }
    }
    accepts_facts(transition_facts(projection))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress_identity(byte: u64) -> CanonicalIdentityProjection {
        CanonicalIdentityProjection {
            domain: 1,
            kind: 1,
            word0: byte,
            word1: byte,
            word2: byte,
            word3: byte,
        }
    }

    fn durable_timeout_progress_witness() -> LockedCommitProgressWitnessProjection {
        LockedCommitProgressWitnessProjection {
            context_id: progress_identity(1),
            current_height: 7,
            current_view: 3,
            local_validator_present: true,
            local_validator: ValidatorId::repeat(4),
            locked_context_id: progress_identity(1),
            locked_height: 7,
            locked_view: 1,
            locked_subject: progress_identity(2),
            timeout_intent_present: true,
            timeout_intent_durable: true,
            timeout_context_id: progress_identity(1),
            timeout_height: 7,
            timeout_view: 3,
            timeout_signer: ValidatorId::repeat(4),
            ..LockedCommitProgressWitnessProjection::default()
        }
    }

    #[test]
    fn locked_commit_progress_witness_accepts_exact_owners_and_rejects_mutations() {
        let timeout = durable_timeout_progress_witness();
        assert!(locked_commit_progress_witness_is_valid(timeout));

        let mut stale_timeout = timeout;
        stale_timeout.timeout_view -= 1;
        assert!(!locked_commit_progress_witness_is_valid(stale_timeout));

        let mut wrong_timeout = timeout;
        wrong_timeout.timeout_signer = ValidatorId::repeat(5);
        assert!(!locked_commit_progress_witness_is_valid(wrong_timeout));

        let mut wrong_lock_context = timeout;
        wrong_lock_context.locked_context_id = progress_identity(9);
        assert!(!locked_commit_progress_witness_is_valid(wrong_lock_context));

        let mut wrong_lock_height = timeout;
        wrong_lock_height.locked_height += 1;
        assert!(!locked_commit_progress_witness_is_valid(wrong_lock_height));

        let mut volatile_timeout = timeout;
        volatile_timeout.timeout_intent_durable = false;
        assert!(!locked_commit_progress_witness_is_valid(volatile_timeout));

        let mut pending = timeout;
        pending.timeout_intent_present = false;
        pending.timeout_intent_durable = false;
        pending.pending = PendingProjection {
            record_kind: WAL_RECORD_LOCK_AND_COMMIT,
            continuation: CONTINUATION_SIGN,
            persistence_id: 9,
            context_id: pending.context_id,
            height: pending.current_height,
            view: pending.current_view,
            proposal_present: true,
            proposal_height: pending.locked_height,
            proposal_view: pending.locked_view,
            subject: pending.locked_subject,
        };
        assert!(locked_commit_progress_witness_is_valid(pending));

        let mut nonexact_pending = pending;
        nonexact_pending.pending.proposal_view += 1;
        assert!(!locked_commit_progress_witness_is_valid(nonexact_pending));

        let mut foreign_height_pending = pending;
        foreign_height_pending.locked_height += 1;
        foreign_height_pending.pending.proposal_height = foreign_height_pending.locked_height;
        assert!(!locked_commit_progress_witness_is_valid(
            foreign_height_pending
        ));

        let mut commit = timeout;
        commit.timeout_intent_present = false;
        commit.timeout_intent_durable = false;
        commit.commit_intent_present = true;
        commit.commit_context_id = commit.context_id;
        commit.commit_height = commit.current_height;
        commit.commit_view = commit.current_view;
        commit.commit_proposal_height = commit.locked_height;
        commit.commit_proposal_view = commit.locked_view;
        commit.commit_phase = 2;
        commit.commit_subject = commit.locked_subject;
        commit.commit_signer = commit.local_validator;
        commit.commit_signature_pending = true;
        assert!(locked_commit_progress_witness_is_valid(commit));

        let mut nonexact_commit = commit;
        nonexact_commit.commit_subject.word0 ^= 1;
        assert!(!locked_commit_progress_witness_is_valid(nonexact_commit));

        let mut foreign_height_commit = commit;
        foreign_height_commit.locked_height += 1;
        foreign_height_commit.commit_proposal_height = foreign_height_commit.locked_height;
        assert!(!locked_commit_progress_witness_is_valid(
            foreign_height_commit
        ));
    }

    #[test]
    fn durable_intent_refinement_accepts_exact_stutters_and_rejects_mutations() {
        let durable_intent = durable_begin_trace();
        assert!(production_durable_intent_trace_refines_progress_witness_kernel(durable_intent));

        let mut wrong_event = durable_intent;
        wrong_event.event_kind = EVENT_PERSISTED;
        assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_event));

        let mut wrong_context = durable_intent;
        wrong_context.pending_after.context_id.word3 ^= 1;
        assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_context));

        let mut wrong_subject = durable_intent;
        wrong_subject.pending_after.subject.word0 ^= 1;
        assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_subject));

        let mut wrong_continuation = durable_intent;
        wrong_continuation.boundary_granted.continuation = CONTINUATION_NONE;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(wrong_continuation)
        );

        let mut wrong_wal_id = durable_intent;
        wrong_wal_id.boundary_granted.persistence_id += 1;
        assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_wal_id));

        let mut consistently_skipped_wal_id = durable_intent;
        consistently_skipped_wal_id.pending_after.persistence_id += 1;
        consistently_skipped_wal_id.boundary_claimed.persistence_id += 1;
        consistently_skipped_wal_id.boundary_granted.persistence_id += 1;
        consistently_skipped_wal_id
            .effects
            .slot0
            .requested
            .persistence_id += 1;
        consistently_skipped_wal_id
            .effects
            .slot0
            .granted
            .persistence_id += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                consistently_skipped_wal_id
            ),
            "a mutually consistent projection must not skip the next durable WAL id"
        );

        let mut wrong_effect = durable_intent;
        wrong_effect.effects.slot0.granted.persistence_id += 1;
        assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_effect));

        let mut timeout_with_high_qc = durable_intent;
        timeout_with_high_qc.event_kind = 5;
        timeout_with_high_qc.boundary_claimed.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
        timeout_with_high_qc.boundary_claimed.continuation = CONTINUATION_INSTALL_TIMEOUT;
        timeout_with_high_qc.boundary_claimed.proposal_present = false;
        timeout_with_high_qc.boundary_claimed.proposal_height = 0;
        timeout_with_high_qc.boundary_claimed.proposal_view = 0;
        timeout_with_high_qc.boundary_granted = timeout_with_high_qc.boundary_claimed;
        timeout_with_high_qc.pending_after.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
        timeout_with_high_qc.pending_after.continuation = CONTINUATION_INSTALL_TIMEOUT;
        timeout_with_high_qc.pending_after.view += 4;
        timeout_with_high_qc.pending_after.proposal_present = false;
        timeout_with_high_qc.pending_after.proposal_height = 0;
        timeout_with_high_qc.pending_after.proposal_view = 0;
        timeout_with_high_qc.boundary_claimed.auxiliary_present = true;
        timeout_with_high_qc.boundary_claimed.auxiliary_context_id =
            timeout_with_high_qc.boundary_claimed.context_id;
        timeout_with_high_qc.boundary_claimed.auxiliary_height =
            timeout_with_high_qc.owner_tag_before.height;
        timeout_with_high_qc.boundary_claimed.auxiliary_view =
            timeout_with_high_qc.owner_tag_before.view;
        timeout_with_high_qc
            .boundary_claimed
            .auxiliary_proposal_height = timeout_with_high_qc.owner_tag_before.height;
        timeout_with_high_qc
            .boundary_claimed
            .auxiliary_proposal_view = timeout_with_high_qc.owner_tag_before.view;
        timeout_with_high_qc.boundary_claimed.auxiliary_phase = 1;
        timeout_with_high_qc.boundary_claimed.auxiliary_subject =
            timeout_with_high_qc.boundary_claimed.subject.subject;
        timeout_with_high_qc.boundary_granted = timeout_with_high_qc.boundary_claimed;
        timeout_with_high_qc.effects.slot0.requested.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
        timeout_with_high_qc.effects.slot0.requested.view = timeout_with_high_qc.pending_after.view;
        timeout_with_high_qc.effects.slot0.requested.proposal_height = 0;
        timeout_with_high_qc.effects.slot0.requested.proposal_view = 0;
        timeout_with_high_qc.effects.slot0.requested.subject =
            timeout_with_high_qc.boundary_claimed.subject.subject;
        timeout_with_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_present = true;
        timeout_with_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_context_id = timeout_with_high_qc.boundary_claimed.auxiliary_context_id;
        timeout_with_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_height = timeout_with_high_qc.boundary_claimed.auxiliary_height;
        timeout_with_high_qc.effects.slot0.requested.auxiliary_view =
            timeout_with_high_qc.boundary_claimed.auxiliary_view;
        timeout_with_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_proposal_height = timeout_with_high_qc
            .boundary_claimed
            .auxiliary_proposal_height;
        timeout_with_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_proposal_view = timeout_with_high_qc
            .boundary_claimed
            .auxiliary_proposal_view;
        timeout_with_high_qc.effects.slot0.requested.auxiliary_phase =
            timeout_with_high_qc.boundary_claimed.auxiliary_phase;
        timeout_with_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_subject = timeout_with_high_qc.boundary_claimed.subject.subject;
        timeout_with_high_qc.effects.slot0.granted = timeout_with_high_qc.effects.slot0.requested;
        assert!(
            production_durable_intent_trace_refines_progress_witness_kernel(timeout_with_high_qc)
        );

        let mut wrong_timeout_high_qc = timeout_with_high_qc;
        wrong_timeout_high_qc.effects.slot0.requested.subject = Subject::repeat(9);
        wrong_timeout_high_qc.effects.slot0.granted.subject = Subject::repeat(9);
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(wrong_timeout_high_qc)
        );

        let mut substituted_timeout_evidence = timeout_with_high_qc;
        substituted_timeout_evidence
            .effects
            .slot0
            .requested
            .auxiliary_subject = Subject::repeat(9);
        substituted_timeout_evidence
            .effects
            .slot0
            .granted
            .auxiliary_subject = Subject::repeat(9);
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                substituted_timeout_evidence
            )
        );

        let mut timeout_without_high_qc = timeout_with_high_qc;
        let absent_subject = Subject::default();
        let absent_subject_identity = CanonicalIdentityProjection::from_bytes(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_CONSENSUS_SUBJECT,
            *absent_subject.as_bytes(),
        );
        timeout_without_high_qc.boundary_claimed.subject.subject = absent_subject;
        timeout_without_high_qc.boundary_claimed.subject_identity = absent_subject_identity;
        timeout_without_high_qc.boundary_claimed.auxiliary_present = false;
        timeout_without_high_qc
            .boundary_claimed
            .auxiliary_context_id = ContextId::repeat(0);
        timeout_without_high_qc.boundary_claimed.auxiliary_height = 0;
        timeout_without_high_qc.boundary_claimed.auxiliary_view = 0;
        timeout_without_high_qc
            .boundary_claimed
            .auxiliary_proposal_height = 0;
        timeout_without_high_qc
            .boundary_claimed
            .auxiliary_proposal_view = 0;
        timeout_without_high_qc.boundary_claimed.auxiliary_phase = 0;
        timeout_without_high_qc.boundary_claimed.auxiliary_subject = Subject::repeat(0);
        timeout_without_high_qc.boundary_granted = timeout_without_high_qc.boundary_claimed;
        timeout_without_high_qc.pending_after.subject = absent_subject_identity;
        timeout_without_high_qc.effects.slot0.requested.subject = absent_subject;
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_present = false;
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_context_id = ContextId::repeat(0);
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_height = 0;
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_view = 0;
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_proposal_height = 0;
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_proposal_view = 0;
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_phase = 0;
        timeout_without_high_qc
            .effects
            .slot0
            .requested
            .auxiliary_subject = Subject::repeat(0);
        timeout_without_high_qc.effects.slot0.granted =
            timeout_without_high_qc.effects.slot0.requested;
        assert!(
            production_durable_intent_trace_refines_progress_witness_kernel(
                timeout_without_high_qc
            )
        );

        let mut regressive_timeout = timeout_with_high_qc;
        regressive_timeout.pending_after.view = regressive_timeout.owner_tag_before.view - 1;
        regressive_timeout.effects.slot0.requested.view = regressive_timeout.pending_after.view;
        regressive_timeout.effects.slot0.granted.view = regressive_timeout.pending_after.view;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(regressive_timeout)
        );

        let mut overflowing_timeout = timeout_with_high_qc;
        overflowing_timeout.pending_after.view = u64::MAX;
        overflowing_timeout.effects.slot0.requested.view = u64::MAX;
        overflowing_timeout.effects.slot0.granted.view = u64::MAX;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(overflowing_timeout)
        );

        let mut wrong_record_height = durable_intent;
        wrong_record_height.pending_after.height += 1;
        wrong_record_height.effects.slot0.requested.height += 1;
        wrong_record_height.effects.slot0.granted.height += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(wrong_record_height)
        );

        let mut stale_generation = durable_intent;
        stale_generation.event_tag.generation += 1;
        stale_generation.pending_after = stale_generation.pending_before;
        stale_generation.boundary_claimed = BoundaryCapabilityKey::none();
        stale_generation.boundary_granted = BoundaryCapabilityKey::none();
        stale_generation.effects = EffectTrace::empty();
        assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_generation));

        let mut stale_height = stale_generation;
        stale_height.event_tag = stale_height.owner_tag_before;
        stale_height.event_tag.height += 1;
        assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_height));

        let mut stale_view = stale_generation;
        stale_view.event_tag = stale_view.owner_tag_before;
        stale_view.event_tag.view += 1;
        assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_view));

        let mut stale_while_pending = stale_generation;
        stale_while_pending.pending_before = durable_intent.pending_after;
        stale_while_pending.pending_after = durable_intent.pending_after;
        assert!(
            production_durable_intent_trace_refines_progress_witness_kernel(stale_while_pending)
        );

        let mut stale_owner_mutation = stale_generation;
        stale_owner_mutation.owner_tag_after.generation += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(stale_owner_mutation)
        );

        let mut stale_pending_mutation = stale_generation;
        stale_pending_mutation.pending_after.persistence_id += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                stale_pending_mutation
            )
        );

        let mut stale_sequence_mutation = stale_generation;
        stale_sequence_mutation.durable_sequence_after += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                stale_sequence_mutation
            )
        );

        let mut stale_boundary = stale_generation;
        stale_boundary.boundary_claimed = durable_intent.boundary_claimed;
        stale_boundary.boundary_granted = durable_intent.boundary_granted;
        assert!(!production_durable_intent_trace_refines_progress_witness_kernel(stale_boundary));

        let mut stale_effect = stale_generation;
        assert!(push_authorized(&mut stale_effect.effects, EFFECT_REPORT));
        assert!(!production_durable_intent_trace_refines_progress_witness_kernel(stale_effect));

        let mut stale_non_completion_id = stale_generation;
        stale_non_completion_id.event_persistence_id = 91;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                stale_non_completion_id
            )
        );

        let mut stale_persisted = stale_generation;
        stale_persisted.event_kind = EVENT_PERSISTED;
        stale_persisted.event_persistence_id = 91;
        assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_persisted));

        let mut stale_persistence_failed = stale_persisted;
        stale_persistence_failed.event_kind = EVENT_PERSISTENCE_FAILED;
        assert!(
            production_durable_intent_trace_refines_progress_witness_kernel(
                stale_persistence_failed
            )
        );

        let mut unmatched_persisted = stale_persisted;
        unmatched_persisted.event_tag = unmatched_persisted.owner_tag_before;
        assert!(
            production_durable_intent_trace_refines_progress_witness_kernel(unmatched_persisted)
        );

        let mut unmatched_persistence_failed = unmatched_persisted;
        unmatched_persistence_failed.event_kind = EVENT_PERSISTENCE_FAILED;
        assert!(
            production_durable_intent_trace_refines_progress_witness_kernel(
                unmatched_persistence_failed
            )
        );

        let mut completion_with_effect = unmatched_persisted;
        assert!(push_authorized(
            &mut completion_with_effect.effects,
            EFFECT_REPORT
        ));
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                completion_with_effect
            )
        );

        let mut completion_while_pending = unmatched_persisted;
        completion_while_pending.pending_before = durable_intent.pending_after;
        completion_while_pending.pending_after = durable_intent.pending_after;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                completion_while_pending
            )
        );

        let mut matching_non_completion_id = unmatched_persisted;
        matching_non_completion_id.event_kind = 0;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                matching_non_completion_id
            )
        );
    }

    #[test]
    fn later_view_lock_and_commit_binds_finality_owner_and_proposal_origin() {
        let begin = lock_and_commit_begin_trace();
        assert!(production_durable_intent_trace_refines_progress_witness_kernel(begin));

        let mut origin_as_lifecycle_round = begin;
        origin_as_lifecycle_round.pending_after.view = begin.pending_after.proposal_view;
        origin_as_lifecycle_round.effects.slot0.requested.view = begin.pending_after.proposal_view;
        origin_as_lifecycle_round.effects.slot0.granted.view = begin.pending_after.proposal_view;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                origin_as_lifecycle_round
            ),
            "the pending WAL owner is the Commit finality round, not its proposal origin"
        );

        let mut substituted_primary_origin = begin;
        substituted_primary_origin
            .effects
            .slot0
            .requested
            .proposal_view += 1;
        substituted_primary_origin
            .effects
            .slot0
            .granted
            .proposal_view += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                substituted_primary_origin
            )
        );

        let mut substituted_auxiliary_origin = begin;
        substituted_auxiliary_origin
            .effects
            .slot0
            .requested
            .auxiliary_proposal_view += 1;
        substituted_auxiliary_origin
            .effects
            .slot0
            .granted
            .auxiliary_proposal_view += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                substituted_auxiliary_origin
            )
        );

        let mut acknowledge = begin;
        acknowledge.event_kind = EVENT_PERSISTED;
        acknowledge.event_persistence_id = begin.pending_after.persistence_id;
        acknowledge.pending_before = begin.pending_after;
        acknowledge.pending_after = PendingProjection::default();
        acknowledge.boundary_claimed.kind = BOUNDARY_ACKNOWLEDGE_WAL;
        acknowledge.boundary_granted = acknowledge.boundary_claimed;
        acknowledge.effects = EffectTrace::empty();
        acknowledge.durable_sequence_after = acknowledge.durable_sequence_before + 1;
        assert!(production_durable_intent_trace_refines_progress_witness_kernel(acknowledge));

        let mut substituted_ack_origin = acknowledge;
        substituted_ack_origin.boundary_claimed.proposal_view += 1;
        substituted_ack_origin.boundary_granted.proposal_view += 1;
        assert!(
            !production_durable_intent_trace_refines_progress_witness_kernel(
                substituted_ack_origin
            ),
            "acknowledgement must retain the proposal origin from the pending record"
        );
    }

    #[test]
    fn remaining_progress_witness_kernels_reject_primitive_trace_mutations() {
        let identity =
            |domain, kind, byte| CanonicalIdentityProjection::from_bytes(domain, kind, [byte; 32]);
        let context = identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            1,
        );
        let subject = identity(IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_WIRE_BLOCK_SUBJECT, 2);
        let block_hash = identity(IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_BLOCK_HEADER, 3);
        let payload_hash = identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_CANONICAL_PAYLOAD, 4);
        let execution = identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            5,
        );
        let executed_wire = identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
            6,
        );
        let decision = ProductionDecisionIdentityProjection {
            context_id: context,
            height: 9,
            view: 4,
            proposal_height: 9,
            proposal_view: 4,
            phase: 2,
            subject,
            block_hash,
            payload_hash,
            execution_commitment: execution,
            executed_block_wire_hash: executed_wire,
        };
        let commit_qc = ProductionQuorumCertificateIdentityProjection {
            decision,
            certificate: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_QUORUM_CERTIFICATE, 7),
            signer_count: 3,
            aggregate_signature_len: 96,
        };
        let manifest = identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_PAYLOAD_MANIFEST, 8);
        let durable_body = ProductionDurableBodyIdentityProjection {
            context_id: context,
            height: 9,
            view: 4,
            subject,
            block_hash,
            payload_hash,
            manifest,
            frame: identity(
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_DURABLE_BODY_FRAME,
                9,
            ),
        };
        let recovery = ProductionDecisionRecoveryTraceProjection {
            state_height: 8,
            expected_context_id: context,
            expected_height: 9,
            expected_block_hash: block_hash,
            frozen_context_id: context,
            frozen_height: 9,
            replay_tag: TagProjection {
                height: 9,
                view: 4,
                generation: 12,
            },
            owner_tag: TagProjection {
                height: 9,
                view: 4,
                generation: 12,
            },
            replay_generation: 12,
            commit_qc,
            manifest_round: TagProjection {
                height: 9,
                view: 4,
                generation: 0,
            },
            manifest_subject: subject,
            manifest,
            durable_body,
            validated_body: durable_body,
            validated_execution_commitment: execution,
            stage: 1,
        };
        assert!(production_decision_trace_refines_recovery_witness_kernel(
            recovery
        ));
        let historical_body = ProductionDurableBodyIdentityProjection {
            view: 2,
            ..durable_body
        };
        let historical_commit_qc = ProductionQuorumCertificateIdentityProjection {
            decision: ProductionDecisionIdentityProjection {
                proposal_view: 2,
                ..decision
            },
            ..commit_qc
        };
        assert!(production_decision_trace_refines_recovery_witness_kernel(
            ProductionDecisionRecoveryTraceProjection {
                commit_qc: historical_commit_qc,
                manifest_round: TagProjection {
                    view: 2,
                    ..recovery.manifest_round
                },
                durable_body: historical_body,
                validated_body: historical_body,
                ..recovery
            }
        ));
        assert!(!production_decision_trace_refines_recovery_witness_kernel(
            ProductionDecisionRecoveryTraceProjection {
                commit_qc: historical_commit_qc,
                manifest_round: TagProjection {
                    view: 3,
                    ..recovery.manifest_round
                },
                durable_body: historical_body,
                validated_body: historical_body,
                ..recovery
            }
        ));
        assert!(!production_decision_trace_refines_recovery_witness_kernel(
            ProductionDecisionRecoveryTraceProjection {
                commit_qc: historical_commit_qc,
                manifest_round: TagProjection {
                    view: 5,
                    ..recovery.manifest_round
                },
                durable_body: ProductionDurableBodyIdentityProjection {
                    view: 5,
                    ..durable_body
                },
                validated_body: ProductionDurableBodyIdentityProjection {
                    view: 5,
                    ..durable_body
                },
                ..recovery
            }
        ));
        for view in [3, 7] {
            let owner_tag = TagProjection {
                view,
                ..recovery.owner_tag
            };
            assert!(production_decision_trace_refines_recovery_witness_kernel(
                ProductionDecisionRecoveryTraceProjection {
                    replay_tag: owner_tag,
                    owner_tag,
                    ..recovery
                }
            ));
        }
        assert!(!production_decision_trace_refines_recovery_witness_kernel(
            ProductionDecisionRecoveryTraceProjection {
                owner_tag: TagProjection {
                    view: 5,
                    ..recovery.owner_tag
                },
                ..recovery
            }
        ));

        let scheduler = ProductionSchedulerTraceProjection {
            fifo_owed_before: false,
            timeout_due: false,
            periodic_timer_due: true,
            fifo_ready: true,
            selected: 2,
            fifo_owed_after: true,
        };
        assert!(production_scheduler_trace_refines_protected_ownership_kernel(scheduler));
        assert!(
            !production_scheduler_trace_refines_protected_ownership_kernel(
                ProductionSchedulerTraceProjection {
                    selected: 3,
                    ..scheduler
                }
            )
        );

        let ingress = ProductionIngressIdentityAndClassTraceProjection {
            incoming_height: 4,
            incoming_view: 2,
            incoming_generation: 3,
            incoming_class: SERVICE_CLASS_PROGRESS,
            stored_height: 4,
            stored_view: 2,
            stored_generation: 3,
            stored_class: SERVICE_CLASS_PROGRESS,
            queue_len_before: 1,
            queue_len_after: 2,
            queue_capacity: 4,
        };
        assert!(
            production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(ingress)
        );
        assert!(
            !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
                ProductionIngressIdentityAndClassTraceProjection {
                    stored_generation: 4,
                    ..ingress
                }
            )
        );

        let flush = ProductionReliableFlushTraceProjection {
            status: 2,
            semantic_target: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 20),
            authenticated_source: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 21),
            requester: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 20),
            responder: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 22),
            connection_tenure_ordinal_high: 0,
            connection_tenure_ordinal_low: 1,
            delivery_ordinal_high: 0,
            delivery_ordinal_low: 2,
            ticket_id: 3,
            ticket_rank: 1,
            ticket_topic: 3,
            canonical_request_digest: identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_REPLY_PAYLOAD,
                23,
            ),
            stream_wire_bytes: 512,
            request_id: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_SIDECAR_REQUEST, 24),
            entry_hash: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_MERGE_ENTRY, 25),
            encoded_len: 256,
            epoch_id: 4,
            reference_digest: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_REFERENCE_DIGEST, 26),
            canonical_response_hash: identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_NETWORK_RESPONSE,
                27,
            ),
            sidecar_response_hash: identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_SIDECAR_RESPONSE,
                28,
            ),
            chunk_hash: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_SIDECAR_CHUNK, 29),
            payload_digest: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_SIDECAR_PAYLOAD, 30),
            chunk_index: 0,
            chunk_count: 2,
            message_cursor_before: 0,
            message_cursor_after: 1,
            chunk_cursor_before: 0,
            chunk_cursor_after: 1,
            flushing_before: 1,
            flushing_after: 0,
            admitted_before: 0,
            admitted_after: 1,
            capacity: 2,
        };
        assert!(production_reliable_flush_trace_refines_outbound_ownership_kernel(flush));
        assert!(
            !production_reliable_flush_trace_refines_outbound_ownership_kernel(
                ProductionReliableFlushTraceProjection {
                    admitted_after: 0,
                    ..flush
                }
            )
        );

        let artifact_hash = identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_FINALITY_ARTIFACT,
            11,
        );
        let application = ProductionApplicationTraceProjection {
            task_tag: TagProjection {
                height: 9,
                view: 4,
                generation: 12,
            },
            owner_tag: TagProjection {
                height: 9,
                view: 4,
                generation: 12,
            },
            task_generation: 12,
            context_id: context,
            context_height: 9,
            commit_qc,
            validated_body: durable_body,
            validated_execution_commitment: execution,
            proposal_block_hash: block_hash,
            proposal_payload_hash: payload_hash,
            committed_block_hash: block_hash,
            executed_block_wire_hash: executed_wire,
            kura_decision: decision,
            kura_artifact_hash: artifact_hash,
            artifact_context_id: context,
            artifact_height: 9,
            artifact_subject: subject,
            artifact_block_hash: block_hash,
            artifact_commit_qc: commit_qc,
            artifact_hash,
            state_height_after: 9,
            task_work_id: 11,
            completion_work_id: 11,
        };
        assert!(production_application_trace_refines_decision_completion_kernel(application));
        let historical_application = ProductionApplicationTraceProjection {
            commit_qc: historical_commit_qc,
            validated_body: ProductionDurableBodyIdentityProjection {
                view: 2,
                ..application.validated_body
            },
            kura_decision: historical_commit_qc.decision,
            artifact_commit_qc: historical_commit_qc,
            ..application
        };
        assert!(
            production_application_trace_refines_decision_completion_kernel(historical_application)
        );
        assert!(
            !production_application_trace_refines_decision_completion_kernel(
                ProductionApplicationTraceProjection {
                    validated_body: ProductionDurableBodyIdentityProjection {
                        view: 5,
                        ..application.validated_body
                    },
                    ..application
                }
            )
        );
        for view in [3, 7] {
            let owner_tag = TagProjection {
                view,
                generation: 15,
                ..application.owner_tag
            };
            assert!(
                production_application_trace_refines_decision_completion_kernel(
                    ProductionApplicationTraceProjection {
                        task_tag: owner_tag,
                        owner_tag,
                        task_generation: 15,
                        ..application
                    }
                )
            );
        }
        assert!(
            !production_application_trace_refines_decision_completion_kernel(
                ProductionApplicationTraceProjection {
                    owner_tag: TagProjection {
                        view: 5,
                        ..application.owner_tag
                    },
                    ..application
                }
            )
        );
        assert!(
            !production_application_trace_refines_decision_completion_kernel(
                ProductionApplicationTraceProjection {
                    completion_work_id: 12,
                    ..application
                }
            )
        );
    }

    #[test]
    fn effective_lock_trace_wrappers_accept_only_their_exact_live_projection() {
        let enter_view = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
            relation_exact: true,
            protected_before: 1,
            protected_after: 1,
            owner_before: 1,
            owner_after: 1,
            ..EffectiveLockTraceProjection::default()
        };
        let enter_view_identity = EnterViewProjection::default();
        assert!(
            production_enter_view_uses_post_install_effective_lock_kernel(
                enter_view,
                enter_view_identity,
            )
        );
        assert!(
            !production_enter_view_uses_post_install_effective_lock_kernel(
                EffectiveLockTraceProjection {
                    owner_after: 0,
                    ..enter_view
                },
                enter_view_identity,
            )
        );

        let ownership = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_OWNER,
            relation_exact: true,
            protected_after: 1,
            owner_after: 1,
            ..EffectiveLockTraceProjection::default()
        };
        assert!(production_body_ownership_preserves_effective_lock_kernel(
            ownership
        ));
        assert!(!production_body_ownership_preserves_effective_lock_kernel(
            EffectiveLockTraceProjection {
                owner_reused: true,
                ..ownership
            }
        ));

        let retirement = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_RETIRE,
            relation_exact: true,
            ready_before: 13,
            retired_retained: 3,
            retired_ready: 4,
            ready_after: 6,
            store_before: 11,
            retired_store: 5,
            store_after: 6,
            ..EffectiveLockTraceProjection::default()
        };
        assert!(production_body_capacity_retirement_preserves_effective_lock_kernel(retirement));
        assert!(
            !production_body_capacity_retirement_preserves_effective_lock_kernel(
                EffectiveLockTraceProjection {
                    ready_after: 7,
                    ..retirement
                }
            )
        );

        let service = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_SERVICE,
            relation_exact: true,
            cursor_before: SERVICE_CLASS_COMPLETION,
            completion_ready: true,
            progress_ready: true,
            selected: SERVICE_CLASS_COMPLETION,
            cursor_after: SERVICE_CLASS_PROGRESS,
            ..EffectiveLockTraceProjection::default()
        };
        assert!(production_body_service_refines_async_fairness_kernel(
            service
        ));
        assert!(!production_body_service_refines_async_fairness_kernel(
            EffectiveLockTraceProjection {
                selected: SERVICE_CLASS_PROGRESS,
                ..service
            }
        ));
    }

    fn capability(kind: u8, nonce: u64) -> EffectCapabilityKey {
        EffectCapabilityKey {
            kind,
            persistence_id: nonce,
            ..EffectCapabilityKey::default()
        }
    }

    fn durable_begin_trace() -> ProductionDurableIntentTraceProjection {
        let context = ContextId::repeat(1);
        let subject = Subject::repeat(2);
        let context_identity = CanonicalIdentityProjection::from_bytes(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_CONSENSUS_CONTEXT,
            *context.as_bytes(),
        );
        let subject_identity = CanonicalIdentityProjection::from_bytes(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_CONSENSUS_SUBJECT,
            *subject.as_bytes(),
        );
        let tag = TagProjection {
            height: 4,
            view: 2,
            generation: 3,
        };
        let boundary = BoundaryCapabilityKey {
            kind: BOUNDARY_BEGIN_WAL,
            record_kind: WAL_RECORD_PROPOSAL_INTENT,
            continuation: CONTINUATION_SIGN,
            persistence_id: 8,
            context_id: context,
            context_identity,
            tag,
            subject: SubjectProjection {
                present: true,
                subject,
            },
            subject_identity,
            proposal_present: true,
            proposal_height: tag.height,
            proposal_view: tag.view,
            ..BoundaryCapabilityKey::none()
        };
        let persist = EffectCapabilityKey {
            kind: EFFECT_PERSIST,
            tag,
            context_id: context,
            height: tag.height,
            view: tag.view,
            proposal_height: tag.height,
            proposal_view: tag.view,
            subject,
            persistence_id: boundary.persistence_id,
            record_kind: boundary.record_kind,
            ..EffectCapabilityKey::default()
        };
        let mut effects = EffectTrace::empty();
        assert!(effects.push(persist, persist));
        ProductionDurableIntentTraceProjection {
            event_tag: tag,
            owner_tag_before: tag,
            owner_tag_after: tag,
            event_kind: 0,
            event_persistence_id: 0,
            pending_before: PendingProjection::default(),
            pending_after: PendingProjection {
                record_kind: boundary.record_kind,
                continuation: boundary.continuation,
                persistence_id: boundary.persistence_id,
                context_id: context_identity,
                height: tag.height,
                view: tag.view,
                proposal_present: true,
                proposal_height: tag.height,
                proposal_view: tag.view,
                subject: subject_identity,
            },
            boundary_claimed: boundary,
            boundary_granted: boundary,
            effects,
            durable_sequence_before: 7,
            durable_sequence_after: 7,
        }
    }

    fn lock_and_commit_begin_trace() -> ProductionDurableIntentTraceProjection {
        let mut trace = durable_begin_trace();
        let origin_view = trace.owner_tag_before.view - 1;
        trace.event_kind = 10;
        trace.pending_after.record_kind = WAL_RECORD_LOCK_AND_COMMIT;
        trace.pending_after.proposal_present = true;
        trace.pending_after.proposal_height = trace.owner_tag_before.height;
        trace.pending_after.proposal_view = origin_view;
        trace.boundary_claimed.record_kind = WAL_RECORD_LOCK_AND_COMMIT;
        trace.boundary_claimed.proposal_present = true;
        trace.boundary_claimed.proposal_height = trace.owner_tag_before.height;
        trace.boundary_claimed.proposal_view = origin_view;
        trace.boundary_claimed.auxiliary_present = true;
        trace.boundary_claimed.auxiliary_context_id = trace.boundary_claimed.context_id;
        trace.boundary_claimed.auxiliary_height = trace.owner_tag_before.height;
        trace.boundary_claimed.auxiliary_view = origin_view;
        trace.boundary_claimed.auxiliary_proposal_height = trace.owner_tag_before.height;
        trace.boundary_claimed.auxiliary_proposal_view = origin_view;
        trace.boundary_claimed.auxiliary_phase = 1;
        trace.boundary_claimed.auxiliary_subject = trace.boundary_claimed.subject.subject;
        trace.boundary_granted = trace.boundary_claimed;

        let persist = {
            let persist = &mut trace.effects.slot0.requested;
            persist.record_kind = WAL_RECORD_LOCK_AND_COMMIT;
            persist.proposal_height = trace.owner_tag_before.height;
            persist.proposal_view = origin_view;
            persist.phase = 2;
            persist.auxiliary_present = true;
            persist.auxiliary_context_id = trace.boundary_claimed.auxiliary_context_id;
            persist.auxiliary_height = trace.boundary_claimed.auxiliary_height;
            persist.auxiliary_view = trace.boundary_claimed.auxiliary_view;
            persist.auxiliary_proposal_height = trace.boundary_claimed.auxiliary_proposal_height;
            persist.auxiliary_proposal_view = trace.boundary_claimed.auxiliary_proposal_view;
            persist.auxiliary_phase = trace.boundary_claimed.auxiliary_phase;
            persist.auxiliary_subject = trace.boundary_claimed.auxiliary_subject;
            *persist
        };
        trace.effects.slot0.granted = persist;
        trace
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
            enter_view_exact: true,
            effects: EffectTrace::empty(),
        }
    }

    fn owner(
        height: u64,
        view: u64,
        generation: u64,
        key: u64,
        manifest_hash: Option<u64>,
    ) -> ExactBodyOwnerProjection<u64, u64> {
        ExactBodyOwnerProjection {
            tag: TagProjection {
                height,
                view,
                generation,
            },
            key,
            manifest_hash,
        }
    }

    #[test]
    fn exact_body_owner_binding_rejects_stale_generation_and_conflicting_evidence() {
        let current = owner(9, 4, 7, 11, Some(23));
        for conflicting in [
            owner(10, 4, 7, 11, Some(23)),
            owner(9, 5, 7, 11, Some(23)),
            owner(9, 4, 7, 12, Some(23)),
        ] {
            assert!(
                plan_exact_body_owner_binding(Some(current), conflicting).is_none(),
                "height, view, and round/subject identity are immutable"
            );
        }
        assert!(
            plan_exact_body_owner_binding(Some(current), owner(9, 4, 8, 11, Some(23))).is_none(),
            "a different generation cannot overwrite an exact owner"
        );
        assert!(
            plan_exact_body_owner_binding(Some(current), owner(9, 4, 7, 11, Some(24))).is_none(),
            "a different manifest identity cannot overwrite an exact owner"
        );

        let enriched = plan_exact_body_owner_binding(
            Some(owner(9, 4, 7, 11, None)),
            owner(9, 4, 7, 11, Some(23)),
        )
        .expect("one certified fetch may acquire its exact manifest identity");
        assert!(enriched.already_owned);
        assert_eq!(enriched.owner, current);
    }

    #[test]
    fn exact_body_owner_rebind_preserves_key_and_evidence_and_advances_incarnation() {
        let previous = owner(9, 4, 7, 11, Some(23));
        let rebound = plan_exact_body_owner_rebind(
            previous,
            previous,
            TagProjection {
                height: 9,
                view: 5,
                generation: 8,
            },
        )
        .expect("strict later-view rebind is accepted");
        assert_eq!(rebound.key, previous.key);
        assert_eq!(rebound.manifest_hash, previous.manifest_hash);

        let same_view = plan_exact_body_owner_rebind(
            previous,
            previous,
            TagProjection {
                height: 9,
                view: 4,
                generation: 8,
            },
        )
        .expect("same-view higher-generation rebind is accepted");
        assert_eq!(same_view.key, previous.key);
        assert_eq!(same_view.manifest_hash, previous.manifest_hash);

        for wrong in [
            TagProjection {
                height: 10,
                view: 5,
                generation: 8,
            },
            TagProjection {
                height: 9,
                view: 3,
                generation: 8,
            },
            TagProjection {
                height: 9,
                view: 5,
                generation: 7,
            },
        ] {
            assert!(plan_exact_body_owner_rebind(previous, previous, wrong).is_none());
        }
        assert!(
            plan_exact_body_owner_rebind(
                previous,
                owner(9, 4, 7, 12, Some(23)),
                TagProjection {
                    height: 9,
                    view: 5,
                    generation: 8,
                },
            )
            .is_none(),
            "a wrong round/subject owner cannot be rebound"
        );
        assert!(
            plan_exact_body_owner_rebind(
                previous,
                owner(9, 4, 7, 11, Some(24)),
                TagProjection {
                    height: 9,
                    view: 5,
                    generation: 8,
                },
            )
            .is_none(),
            "a conflicting manifest identity cannot be rebound"
        );
        assert!(
            plan_exact_body_owner_rebind(
                previous,
                owner(9, 3, 6, 11, Some(23)),
                TagProjection {
                    height: 9,
                    view: 5,
                    generation: 8,
                },
            )
            .is_none(),
            "the previous stage tag must be the exact installed owner"
        );
    }

    #[test]
    fn exact_body_completion_classifier_rejects_duplicate_or_conflicting_owners() {
        for ingress_owners in 0..=2 {
            for ingress_exact in 0..=2 {
                for deferred_owners in 0..=2 {
                    for deferred_exact in 0..=2 {
                        let expected = match (
                            ingress_owners,
                            ingress_exact,
                            deferred_owners,
                            deferred_exact,
                        ) {
                            (0, 0, 0, 0) => ExactBodyCompletionOwnership::Vacant,
                            (1, 1, 0, 0) | (0, 0, 1, 1) => ExactBodyCompletionOwnership::Exact,
                            _ => ExactBodyCompletionOwnership::Invalid,
                        };
                        assert_eq!(
                            classify_exact_body_completion_ownership(
                                ingress_owners,
                                ingress_exact,
                                deferred_owners,
                                deferred_exact,
                            ),
                            expected,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn exact_body_retirement_accounting_rejects_capacity_leakage() {
        let accounting = plan_exact_body_retirement_accounting(100, 20, 30, 80, 35)
            .expect("exact owned bytes fit both counters");
        assert_eq!(accounting.ready_after, 50);
        assert_eq!(accounting.store_after, 45);
        assert!(plan_exact_body_retirement_accounting(49, 20, 30, 80, 35).is_none());
        assert!(plan_exact_body_retirement_accounting(100, 20, 30, 34, 35).is_none());
        assert_eq!(
            plan_exact_body_retirement_accounting(u64::MAX, 0, 0, u64::MAX, 0),
            Some(ExactBodyRetirementAccounting {
                ready_after: u64::MAX,
                store_after: u64::MAX,
            })
        );
        assert_eq!(
            plan_exact_body_retirement_accounting(u64::MAX, u64::MAX, 0, 0, 0),
            Some(ExactBodyRetirementAccounting {
                ready_after: 0,
                store_after: 0,
            })
        );
        assert!(
            plan_exact_body_retirement_accounting(u64::MAX, u64::MAX, u64::MAX, 0, 0).is_none(),
            "sequential retirement rejects an overflowing combined claim"
        );
    }

    #[test]
    fn bounded_service_kernel_exhaustively_selects_each_readiness_combination() {
        let classes = [
            SERVICE_CLASS_COMPLETION,
            SERVICE_CLASS_PROGRESS,
            SERVICE_CLASS_NORMAL,
        ];
        for cursor in classes {
            for ready_mask in 0u8..8 {
                let completion_ready = ready_mask & 0b001 != 0;
                let progress_ready = ready_mask & 0b010 != 0;
                let normal_ready = ready_mask & 0b100 != 0;
                let ready = |class| match class {
                    SERVICE_CLASS_COMPLETION => completion_ready,
                    SERVICE_CLASS_PROGRESS => progress_ready,
                    SERVICE_CLASS_NORMAL => normal_ready,
                    _ => false,
                };
                let cursor_index = classes
                    .iter()
                    .position(|class| *class == cursor)
                    .expect("cursor is one of the three classes");
                let expected = (0..3)
                    .map(|offset| classes[(cursor_index + offset) % 3])
                    .find(|class| ready(*class));
                let selection = select_bounded_service_class(
                    cursor,
                    completion_ready,
                    progress_ready,
                    normal_ready,
                );
                assert_eq!(selection.selected, expected.unwrap_or(SERVICE_CLASS_NONE));
                let expected_next = expected.map_or(cursor, |selected| match selected {
                    SERVICE_CLASS_COMPLETION => SERVICE_CLASS_PROGRESS,
                    SERVICE_CLASS_PROGRESS => SERVICE_CLASS_NORMAL,
                    SERVICE_CLASS_NORMAL => SERVICE_CLASS_COMPLETION,
                    _ => unreachable!("selected class came from the canonical set"),
                });
                assert_eq!(selection.next, expected_next);
            }
        }

        let first = select_bounded_service_class(SERVICE_CLASS_COMPLETION, true, true, true);
        let second = select_bounded_service_class(first.next, true, true, true);
        let third = select_bounded_service_class(second.next, true, true, true);
        assert_eq!(
            [first.selected, second.selected, third.selected],
            [
                SERVICE_CLASS_COMPLETION,
                SERVICE_CLASS_PROGRESS,
                SERVICE_CLASS_NORMAL,
            ]
        );
        assert_eq!(third.next, SERVICE_CLASS_COMPLETION);

        for invalid_cursor in [0, 4, 99, u8::MAX] {
            let invalid = select_bounded_service_class(invalid_cursor, true, true, true);
            assert_eq!(invalid.selected, SERVICE_CLASS_NONE);
            assert_eq!(invalid.next, SERVICE_CLASS_NONE);
        }
    }

    #[test]
    fn source_linked_effective_lock_body_kernels_reject_adversarial_inputs() {
        exact_body_owner_binding_rejects_stale_generation_and_conflicting_evidence();
        exact_body_owner_rebind_preserves_key_and_evidence_and_advances_incarnation();
        exact_body_completion_classifier_rejects_duplicate_or_conflicting_owners();
        exact_body_retirement_accounting_rejects_capacity_leakage();
        bounded_service_kernel_exhaustively_selects_each_readiness_combination();
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

        struct OpaqueOrderToken(&'static str);

        let mut pending = VecDeque::from([
            OpaqueOrderToken("old-tail-0"),
            OpaqueOrderToken("old-tail-1"),
        ]);
        prepend_causal_continuation(
            &mut pending,
            vec![
                OpaqueOrderToken("continuation-0"),
                OpaqueOrderToken("continuation-1"),
                OpaqueOrderToken("continuation-2"),
            ],
        );
        assert_eq!(
            pending.into_iter().map(|token| token.0).collect::<Vec<_>>(),
            [
                "continuation-0",
                "continuation-1",
                "continuation-2",
                "old-tail-0",
                "old-tail-1",
            ],
            "persisted continuation order is causal FIFO order"
        );

        let mut forward_iteration_mutant = VecDeque::from([
            OpaqueOrderToken("old-tail-0"),
            OpaqueOrderToken("old-tail-1"),
        ]);
        for item in [
            OpaqueOrderToken("continuation-0"),
            OpaqueOrderToken("continuation-1"),
            OpaqueOrderToken("continuation-2"),
        ] {
            forward_iteration_mutant.push_front(item);
        }
        assert_eq!(
            forward_iteration_mutant
                .into_iter()
                .map(|token| token.0)
                .collect::<Vec<_>>(),
            [
                "continuation-2",
                "continuation-1",
                "continuation-0",
                "old-tail-0",
                "old-tail-1",
            ],
            "the compact forward-iteration mutant reverses the continuation"
        );
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
    fn retransmit_may_reconstruct_one_final_decision_body_stage() {
        let mut store_retry = base_facts();
        store_retry.action_kind = ACTION_VOLATILE_PROTOCOL;
        store_retry.event_kind = 7;
        for _ in 0..7 {
            assert!(push_authorized(&mut store_retry.effects, EFFECT_BROADCAST));
        }
        assert!(push_authorized(&mut store_retry.effects, EFFECT_STORE));
        assert!(accepts_facts(store_retry));

        let mut validate_retry = base_facts();
        validate_retry.action_kind = ACTION_VOLATILE_PROTOCOL;
        validate_retry.event_kind = 7;
        assert!(push_authorized(
            &mut validate_retry.effects,
            EFFECT_BROADCAST
        ));
        assert!(push_authorized(
            &mut validate_retry.effects,
            EFFECT_VALIDATE
        ));
        assert!(accepts_facts(validate_retry));

        let mut not_final = validate_retry;
        not_final.effects = EffectTrace::empty();
        assert!(push_authorized(&mut not_final.effects, EFFECT_VALIDATE));
        assert!(push_authorized(&mut not_final.effects, EFFECT_BROADCAST));
        assert!(!accepts_facts(not_final));

        let mut mixed_stages = validate_retry;
        mixed_stages.effects = EffectTrace::empty();
        assert!(push_authorized(&mut mixed_stages.effects, EFFECT_STORE));
        assert!(push_authorized(&mut mixed_stages.effects, EFFECT_VALIDATE));
        assert!(!accepts_facts(mixed_stages));

        let mut fetch_and_store = validate_retry;
        fetch_and_store.effects = EffectTrace::empty();
        assert!(push_authorized(&mut fetch_and_store.effects, EFFECT_FETCH));
        assert!(push_authorized(&mut fetch_and_store.effects, EFFECT_STORE));
        assert!(!accepts_facts(fetch_and_store));

        let mut report_and_store = validate_retry;
        report_and_store.effects = EffectTrace::empty();
        assert!(push_authorized(
            &mut report_and_store.effects,
            EFFECT_REPORT
        ));
        assert!(push_authorized(&mut report_and_store.effects, EFFECT_STORE));
        assert!(!accepts_facts(report_and_store));

        let mut apply_and_fetch = validate_retry;
        apply_and_fetch.effects = EffectTrace::empty();
        assert!(push_authorized(&mut apply_and_fetch.effects, EFFECT_APPLY));
        assert!(push_authorized(&mut apply_and_fetch.effects, EFFECT_FETCH));
        assert!(!accepts_facts(apply_and_fetch));

        let mut fetch_not_final = validate_retry;
        fetch_not_final.effects = EffectTrace::empty();
        assert!(push_authorized(&mut fetch_not_final.effects, EFFECT_FETCH));
        assert!(push_authorized(
            &mut fetch_not_final.effects,
            EFFECT_BROADCAST
        ));
        assert!(!accepts_facts(fetch_not_final));

        let mut wrong_event = validate_retry;
        wrong_event.event_kind = 6;
        assert!(!accepts_facts(wrong_event));
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
