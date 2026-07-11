//! Verus refinement model for the executable Sumeragi v2 reducer.
//!
//! This module is ghost-only and is erased by normal Rust compilation.  It
//! gives every production `Event` and `WalRecord` variant an explicit abstract
//! action.  The WAL relation is transition complete at the safety projection:
//! sequence, context, view, local intent, highest-PrepareQC, lock, timeout, and
//! decision changes are represented branch by branch.  The reducer relation
//! additionally models the pending-append fence and historical body-store
//! tokens needed to justify signing and application effects.
//!
//! The definitions and proof bodies contain no proof escape hatches.  See
//! `VERIFICATION.md` for the mechanical source-link and tool-execution work
//! that remains before this can be called a verification of production code.

use vstd::prelude::*;

verus! {

/// Largest value representable by every production height/view/generation/WAL
/// counter (`u64`).
pub open spec fn machine_u64_max() -> int {
    18_446_744_073_709_551_615
}

// ---------------------------------------------------------------------------
// Common certificate and quorum facts
// ---------------------------------------------------------------------------

/// Safety projection of a quorum certificate.
pub struct CertificateProjection {
    /// Whether a certificate is present.
    pub present: bool,
    /// Prepare when true and Commit when false.
    pub prepare: bool,
    /// Certificate view at the frozen height.
    pub view: int,
    /// Certified subject identity.
    pub subject: int,
}

/// The absent certificate value.  Its remaining fields are canonicalized.
pub open spec fn absent_certificate() -> CertificateProjection {
    CertificateProjection {
        present: false,
        prepare: true,
        view: 0,
        subject: 0,
    }
}

/// Exact equality of the safety-relevant certificate reference.
pub open spec fn same_certificate(
    left: CertificateProjection,
    right: CertificateProjection,
) -> bool {
    left.present == right.present
        && (!left.present
            || (left.prepare == right.prepare
                && left.view == right.view
                && left.subject == right.subject))
}

/// A well-formed projected PrepareQC.
pub open spec fn valid_prepare(certificate: CertificateProjection) -> bool {
    certificate.present
        && certificate.prepare
        && 0 <= certificate.view <= machine_u64_max()
}

/// A well-formed projected CommitQC.
pub open spec fn valid_commit(certificate: CertificateProjection) -> bool {
    certificate.present
        && !certificate.prepare
        && 0 <= certificate.view <= machine_u64_max()
}

/// Equal-view certificates do not conflict and a higher one may replace one.
pub open spec fn compatible_highest_update(
    current: CertificateProjection,
    incoming: CertificateProjection,
) -> bool {
    valid_prepare(incoming)
        && (!current.present
            || incoming.view != current.view
            || incoming.subject == current.subject)
}

/// Production `update_highest`: install only a strictly higher PrepareQC.
pub open spec fn highest_after_update(
    current: CertificateProjection,
    incoming: CertificateProjection,
) -> CertificateProjection {
    if !current.present || incoming.view > current.view {
        incoming
    } else {
        current
    }
}

/// Production TC installation: never lower a lock and change its subject only
/// at a strictly greater view.
pub open spec fn lock_after_timeout(
    current: CertificateProjection,
    selected: CertificateProjection,
) -> CertificateProjection {
    if selected.present && (!current.present || selected.view > current.view) {
        selected
    } else {
        current
    }
}

/// A later state extends, rather than regresses, an earlier lock.
pub open spec fn lock_extends(
    before: CertificateProjection,
    after: CertificateProjection,
) -> bool {
    !before.present
        || (after.present
            && (after.view > before.view
                || (after.view == before.view && after.subject == before.subject)))
}

/// Arithmetic core of strict voting-power quorum intersection.
pub proof fn strict_power_quorums_cannot_be_disjoint(
    total_power: int,
    left_power: int,
    right_power: int,
)
    requires
        0 < total_power,
        0 <= left_power <= total_power,
        0 <= right_power <= total_power,
        left_power * 3 > total_power * 2,
        right_power * 3 > total_power * 2,
    ensures
        left_power + right_power > total_power,
{
    assert((left_power + right_power) * 3 > total_power * 4);
    assert(total_power * 4 >= total_power * 3);
}

/// Arithmetic core of the floor(2n/3)+1 count quorum rule.
pub proof fn strict_count_quorums_cannot_be_disjoint(
    validator_count: int,
    left_count: int,
    right_count: int,
)
    requires
        0 < validator_count,
        0 <= left_count <= validator_count,
        0 <= right_count <= validator_count,
        left_count * 3 > validator_count * 2,
        right_count * 3 > validator_count * 2,
    ensures
        left_count + right_count > validator_count,
{
    assert((left_count + right_count) * 3 > validator_count * 4);
    assert(validator_count * 4 >= validator_count * 3);
}

// ---------------------------------------------------------------------------
// Exact WAL safety projection
// ---------------------------------------------------------------------------

/// Every production `WalRecord` variant, projected onto checked predicates.
pub enum WalRecordProjection {
    /// `WalRecord::ProposalIntent`.
    ProposalIntent {
        /// Proposal view.
        view: int,
        /// Proposal subject.
        subject: int,
        /// Projection of the local-leader/context/round checks.
        local_leader_valid: bool,
        /// Projection of parent/TC justification and safe-unlock checks.
        justification_safe: bool,
    },
    /// `WalRecord::PrepareIntent`.
    PrepareIntent {
        /// Vote view.
        view: int,
        /// Vote subject.
        subject: int,
        /// Projection of context, height, phase, signer, and roster checks.
        local_vote_valid: bool,
    },
    /// `WalRecord::ObservePrepare`.
    ObservePrepare {
        /// Observed PrepareQC.
        certificate: CertificateProjection,
        /// Projection of full QC validation against the frozen context.
        certificate_valid: bool,
    },
    /// `WalRecord::LockAndCommit`.
    LockAndCommit {
        /// PrepareQC stored atomically with the Commit intent.
        prepare: CertificateProjection,
        /// Commit vote view.
        vote_view: int,
        /// Commit vote subject.
        vote_subject: int,
        /// Projection of local Commit vote validation.
        local_vote_valid: bool,
        /// Projection of full PrepareQC validation.
        certificate_valid: bool,
    },
    /// `WalRecord::TimeoutIntent`.
    TimeoutIntent {
        /// Timed-out view.
        view: int,
        /// Canonical identity of the carried highest PrepareQC reference.
        high_reference: int,
        /// Projection of local timeout vote validation.
        local_vote_valid: bool,
        /// The carried high reference equals durable `highest_prepare`.
        high_reference_matches: bool,
    },
    /// `WalRecord::InstallTimeout`.
    InstallTimeout {
        /// View certified by the TC; installation enters `tc_view + 1`.
        tc_view: int,
        /// Projection of grouped-signature and dual-quorum TC validation.
        certificate_valid: bool,
        /// Highest PrepareQC selected from all TC groups, or absent.
        selected_prepare: CertificateProjection,
    },
    /// `WalRecord::Decision`.
    Decision {
        /// CommitQC decision.
        certificate: CertificateProjection,
        /// Projection of full CommitQC validation.
        certificate_valid: bool,
    },
}

/// One complete projected WAL frame.
pub struct WalFrameProjection {
    /// Monotonic frame number.
    pub id: nat,
    /// Frozen height-context identity.
    pub context: int,
    /// Projected record.
    pub record: WalRecordProjection,
}

/// Safety-relevant `DurableState` fields reconstructed by WAL replay.
pub struct WalStateProjection {
    /// Frozen context identity.
    pub context: int,
    /// Frozen height.
    pub height: int,
    /// Persisted current view.
    pub view: int,
    /// Last complete frame number.
    pub last_id: nat,
    /// Unique local proposal subject per view.
    pub proposal_intents: Map<int, int>,
    /// Unique local Prepare subject per view.
    pub prepare_intents: Map<int, int>,
    /// Unique local Commit subject per view.
    pub commit_intents: Map<int, int>,
    /// Unique local timeout high-reference per view.
    pub timeout_intents: Map<int, int>,
    /// Highest observed PrepareQC.
    pub highest_prepare: CertificateProjection,
    /// Durable lock.
    pub locked: CertificateProjection,
    /// Last installed TC view, or -1 when absent.
    pub last_timeout_view: int,
    /// Durable CommitQC decision.
    pub decision: CertificateProjection,
}

/// Structural equality of the complete durable safety projection.
pub open spec fn wal_states_equivalent(
    left: WalStateProjection,
    right: WalStateProjection,
) -> bool {
    left.context == right.context
        && left.height == right.height
        && left.view == right.view
        && left.last_id == right.last_id
        && left.proposal_intents =~= right.proposal_intents
        && left.prepare_intents =~= right.prepare_intents
        && left.commit_intents =~= right.commit_intents
        && left.timeout_intents =~= right.timeout_intents
        && same_certificate(left.highest_prepare, right.highest_prepare)
        && same_certificate(left.locked, right.locked)
        && left.last_timeout_view == right.last_timeout_view
        && same_certificate(left.decision, right.decision)
}

/// The invariant reconstructed from every accepted complete WAL prefix.
pub open spec fn wal_invariant(state: WalStateProjection) -> bool {
    0 <= state.height <= machine_u64_max()
        && 0 <= state.view <= machine_u64_max()
        && state.last_id <= machine_u64_max()
        && state.last_timeout_view < state.view
        && (!state.highest_prepare.present
            || (valid_prepare(state.highest_prepare)
                && state.highest_prepare.view <= state.view))
        && (!state.locked.present
            || (valid_prepare(state.locked)
                && state.locked.view <= state.view
                && state.highest_prepare.present
                && state.highest_prepare.view >= state.locked.view
                && (state.highest_prepare.view != state.locked.view
                    || state.highest_prepare.subject == state.locked.subject)))
        && (!state.decision.present || valid_commit(state.decision))
}

/// A map insertion is permitted only when it does not change an existing
/// local intent for the same view.
pub open spec fn unique_insert_allowed(
    intents: Map<int, int>,
    view: int,
    subject: int,
) -> bool {
    !intents.dom().contains(view) || intents[view] == subject
}

/// A projected frame passes every production pre-state check, but has not yet
/// changed the state.  This is the guard half of `DurableState::apply`.
pub open spec fn wal_frame_admissible(
    before: WalStateProjection,
    frame: WalFrameProjection,
) -> bool {
    before.last_id < machine_u64_max()
        && frame.id == before.last_id + 1
        && frame.context == before.context
        && match frame.record {
            WalRecordProjection::ProposalIntent {
                view,
                subject,
                local_leader_valid,
                justification_safe,
            } => {
                local_leader_valid
                    && justification_safe
                    && view == before.view
                    && !before.timeout_intents.dom().contains(view)
                    && unique_insert_allowed(before.proposal_intents, view, subject)
            }
            WalRecordProjection::PrepareIntent {
                view,
                subject,
                local_vote_valid,
            } => {
                local_vote_valid
                    && view == before.view
                    && !before.timeout_intents.dom().contains(view)
                    && unique_insert_allowed(before.prepare_intents, view, subject)
            }
            WalRecordProjection::ObservePrepare {
                certificate,
                certificate_valid,
            } => {
                certificate_valid
                    && valid_prepare(certificate)
                    && certificate.view <= before.view
                    && compatible_highest_update(before.highest_prepare, certificate)
            }
            WalRecordProjection::LockAndCommit {
                prepare,
                vote_view,
                vote_subject,
                local_vote_valid,
                certificate_valid,
            } => {
                local_vote_valid
                    && certificate_valid
                    && valid_prepare(prepare)
                    && vote_view == before.view
                    && vote_view == prepare.view
                    && vote_subject == prepare.subject
                    && !before.timeout_intents.dom().contains(vote_view)
                    && unique_insert_allowed(before.commit_intents, vote_view, vote_subject)
                    && compatible_highest_update(before.highest_prepare, prepare)
                    && (!before.locked.present
                        || prepare.view > before.locked.view
                        || (prepare.view == before.locked.view
                            && prepare.subject == before.locked.subject))
            }
            WalRecordProjection::TimeoutIntent {
                view,
                high_reference,
                local_vote_valid,
                high_reference_matches,
            } => {
                local_vote_valid
                    && high_reference_matches
                    && view == before.view
                    && unique_insert_allowed(before.timeout_intents, view, high_reference)
            }
            WalRecordProjection::InstallTimeout {
                tc_view,
                certificate_valid,
                selected_prepare,
            } => {
                certificate_valid
                    && tc_view >= before.view
                    && 0 <= tc_view < machine_u64_max()
                    && (!selected_prepare.present || valid_prepare(selected_prepare))
                    && (!selected_prepare.present || selected_prepare.view <= tc_view)
                    && (!selected_prepare.present
                        || compatible_highest_update(before.highest_prepare, selected_prepare))
                    && (!selected_prepare.present
                        || !before.locked.present
                        || selected_prepare.view != before.locked.view
                        || selected_prepare.subject == before.locked.subject)
            }
            WalRecordProjection::Decision {
                certificate,
                certificate_valid,
            } => {
                certificate_valid
                    && valid_commit(certificate)
                    && (!before.decision.present
                        || same_certificate(before.decision, certificate))
            }
        }
}

/// Fields unaffected by a WAL record remain identical.
pub open spec fn same_wal_identity_and_intents(
    before: WalStateProjection,
    after: WalStateProjection,
) -> bool {
    after.context == before.context
        && after.height == before.height
}

/// Exact transition relation for the safety projection of
/// `DurableState::apply_in_place`.
pub open spec fn wal_apply(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
) -> bool {
    wal_frame_admissible(before, frame)
        && same_wal_identity_and_intents(before, after)
        && after.last_id == frame.id
        && match frame.record {
            WalRecordProjection::ProposalIntent { view, subject, .. } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents.insert(view, subject)
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate(after.highest_prepare, before.highest_prepare)
                    && same_certificate(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate(after.decision, before.decision)
            }
            WalRecordProjection::PrepareIntent { view, subject, .. } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents.insert(view, subject)
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate(after.highest_prepare, before.highest_prepare)
                    && same_certificate(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate(after.decision, before.decision)
            }
            WalRecordProjection::ObservePrepare { certificate, .. } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate(
                        after.highest_prepare,
                        highest_after_update(before.highest_prepare, certificate),
                    )
                    && same_certificate(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate(after.decision, before.decision)
            }
            WalRecordProjection::LockAndCommit {
                prepare,
                vote_view,
                vote_subject,
                ..
            } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents
                        =~= before.commit_intents.insert(vote_view, vote_subject)
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate(
                        after.highest_prepare,
                        highest_after_update(before.highest_prepare, prepare),
                    )
                    && same_certificate(after.locked, prepare)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate(after.decision, before.decision)
            }
            WalRecordProjection::TimeoutIntent {
                view,
                high_reference,
                ..
            } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents
                        =~= before.timeout_intents.insert(view, high_reference)
                    && same_certificate(after.highest_prepare, before.highest_prepare)
                    && same_certificate(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate(after.decision, before.decision)
            }
            WalRecordProjection::InstallTimeout {
                tc_view,
                selected_prepare,
                ..
            } => {
                after.view == tc_view + 1
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate(
                        after.highest_prepare,
                        if selected_prepare.present {
                            highest_after_update(before.highest_prepare, selected_prepare)
                        } else {
                            before.highest_prepare
                        },
                    )
                    && same_certificate(
                        after.locked,
                        lock_after_timeout(before.locked, selected_prepare),
                    )
                    && after.last_timeout_view == tc_view
                    && same_certificate(after.decision, before.decision)
            }
            WalRecordProjection::Decision { certificate, .. } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate(after.highest_prepare, before.highest_prepare)
                    && same_certificate(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate(after.decision, certificate)
            }
        }
}

/// Transactional success/failure split of public `DurableState::apply`.
pub enum WalApplyPathProjection {
    /// The frame passed every guard and was committed to the clone.
    Accepted,
    /// The frame failed and the original state was retained exactly.
    Rejected,
}

/// Public `DurableState::apply` either applies one admissible frame or leaves
/// every projected field unchanged on error.
pub open spec fn durable_apply_refines(
    before: WalStateProjection,
    frame: WalFrameProjection,
    path: WalApplyPathProjection,
    after: WalStateProjection,
) -> bool {
    match path {
        WalApplyPathProjection::Accepted => wal_apply(before, frame, after),
        WalApplyPathProjection::Rejected => {
            !wal_frame_admissible(before, frame)
                && wal_states_equivalent(before, after)
        }
    }
}

/// A rejected frame cannot partially change a vote, lock, view, or decision.
pub proof fn rejected_wal_frame_is_transactional(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        durable_apply_refines(before, frame, WalApplyPathProjection::Rejected, after),
    ensures
        wal_states_equivalent(before, after),
        before.last_id == after.last_id,
        before.view == after.view,
        same_certificate(before.locked, after.locked),
        same_certificate(before.decision, after.decision),
{
}

/// Every accepted WAL transition preserves the durable invariant.
pub proof fn wal_apply_preserves_invariant(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_invariant(before),
        wal_apply(before, frame, after),
    ensures
        wal_invariant(after),
{
    match frame.record {
        WalRecordProjection::ProposalIntent { .. } => {},
        WalRecordProjection::PrepareIntent { .. } => {},
        WalRecordProjection::ObservePrepare { .. } => {},
        WalRecordProjection::LockAndCommit { .. } => {},
        WalRecordProjection::TimeoutIntent { .. } => {},
        WalRecordProjection::InstallTimeout { .. } => {},
        WalRecordProjection::Decision { .. } => {},
    }
}

/// Existing local proposal, Prepare, Commit, and timeout intents never change.
pub proof fn wal_apply_preserves_all_existing_intents(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        forall |view: int|
            #![trigger before.proposal_intents[view], after.proposal_intents[view]]
            before.proposal_intents.dom().contains(view)
            ==> after.proposal_intents.dom().contains(view)
                && after.proposal_intents[view] == before.proposal_intents[view],
        forall |view: int|
            #![trigger before.prepare_intents[view], after.prepare_intents[view]]
            before.prepare_intents.dom().contains(view)
            ==> after.prepare_intents.dom().contains(view)
                && after.prepare_intents[view] == before.prepare_intents[view],
        forall |view: int|
            #![trigger before.commit_intents[view], after.commit_intents[view]]
            before.commit_intents.dom().contains(view)
            ==> after.commit_intents.dom().contains(view)
                && after.commit_intents[view] == before.commit_intents[view],
        forall |view: int|
            #![trigger before.timeout_intents[view], after.timeout_intents[view]]
            before.timeout_intents.dom().contains(view)
            ==> after.timeout_intents.dom().contains(view)
                && after.timeout_intents[view] == before.timeout_intents[view],
{
    match frame.record {
        WalRecordProjection::ProposalIntent { view, subject, .. } => {
            assert forall |old_view: int|
                #![trigger before.proposal_intents[old_view], after.proposal_intents[old_view]]
                before.proposal_intents.dom().contains(old_view)
                implies after.proposal_intents.dom().contains(old_view)
                    && after.proposal_intents[old_view]
                        == before.proposal_intents[old_view] by {
                if old_view == view {
                    assert(unique_insert_allowed(before.proposal_intents, view, subject));
                }
            }
        },
        WalRecordProjection::PrepareIntent { view, subject, .. } => {
            assert forall |old_view: int|
                #![trigger before.prepare_intents[old_view], after.prepare_intents[old_view]]
                before.prepare_intents.dom().contains(old_view)
                implies after.prepare_intents.dom().contains(old_view)
                    && after.prepare_intents[old_view]
                        == before.prepare_intents[old_view] by {
                if old_view == view {
                    assert(unique_insert_allowed(before.prepare_intents, view, subject));
                }
            }
        },
        WalRecordProjection::LockAndCommit {
            vote_view,
            vote_subject,
            ..
        } => {
            assert forall |old_view: int|
                #![trigger before.commit_intents[old_view], after.commit_intents[old_view]]
                before.commit_intents.dom().contains(old_view)
                implies after.commit_intents.dom().contains(old_view)
                    && after.commit_intents[old_view]
                        == before.commit_intents[old_view] by {
                if old_view == vote_view {
                    assert(unique_insert_allowed(
                        before.commit_intents,
                        vote_view,
                        vote_subject,
                    ));
                }
            }
        },
        WalRecordProjection::TimeoutIntent {
            view,
            high_reference,
            ..
        } => {
            assert forall |old_view: int|
                #![trigger before.timeout_intents[old_view], after.timeout_intents[old_view]]
                before.timeout_intents.dom().contains(old_view)
                implies after.timeout_intents.dom().contains(old_view)
                    && after.timeout_intents[old_view]
                        == before.timeout_intents[old_view] by {
                if old_view == view {
                    assert(unique_insert_allowed(
                        before.timeout_intents,
                        view,
                        high_reference,
                    ));
                }
            }
        },
        WalRecordProjection::ObservePrepare { .. }
        | WalRecordProjection::InstallTimeout { .. }
        | WalRecordProjection::Decision { .. } => {},
    }
}

/// WAL application never lowers a lock or changes its subject at equal view.
pub proof fn wal_apply_preserves_lock_monotonicity(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        lock_extends(before.locked, after.locked),
{
    match frame.record {
        WalRecordProjection::ProposalIntent { .. } => {},
        WalRecordProjection::PrepareIntent { .. } => {},
        WalRecordProjection::ObservePrepare { .. } => {},
        WalRecordProjection::LockAndCommit { .. } => {},
        WalRecordProjection::TimeoutIntent { .. } => {},
        WalRecordProjection::InstallTimeout { .. } => {},
        WalRecordProjection::Decision { .. } => {},
    }
}

/// Installed TCs are the only WAL action that advances view, and never regress it.
pub proof fn wal_apply_preserves_view_monotonicity(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        after.view >= before.view,
{
    match frame.record {
        WalRecordProjection::InstallTimeout { .. } => {},
        WalRecordProjection::ProposalIntent { .. }
        | WalRecordProjection::PrepareIntent { .. }
        | WalRecordProjection::ObservePrepare { .. }
        | WalRecordProjection::LockAndCommit { .. }
        | WalRecordProjection::TimeoutIntent { .. }
        | WalRecordProjection::Decision { .. } => {},
    }
}

/// A durable decision is immutable across all later complete frames.
pub proof fn wal_apply_preserves_decision_uniqueness(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
        before.decision.present,
    ensures
        same_certificate(after.decision, before.decision),
{
    match frame.record {
        WalRecordProjection::Decision { .. } => {},
        WalRecordProjection::ProposalIntent { .. }
        | WalRecordProjection::PrepareIntent { .. }
        | WalRecordProjection::ObservePrepare { .. }
        | WalRecordProjection::LockAndCommit { .. }
        | WalRecordProjection::TimeoutIntent { .. }
        | WalRecordProjection::InstallTimeout { .. } => {},
    }
}

/// ProposalIntent installs exactly one proposal subject at its projected view.
pub proof fn proposal_intent_branch_postcondition(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        match frame.record {
            WalRecordProjection::ProposalIntent { view, subject, .. } => {
                after.proposal_intents.dom().contains(view)
                    && after.proposal_intents[view] == subject
                    && after.view == before.view
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::ProposalIntent { .. } => {},
        _ => {},
    }
}

/// PrepareIntent installs exactly one Prepare subject while its view is open.
pub proof fn prepare_intent_branch_postcondition(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        match frame.record {
            WalRecordProjection::PrepareIntent { view, subject, .. } => {
                after.prepare_intents.dom().contains(view)
                    && after.prepare_intents[view] == subject
                    && !before.timeout_intents.dom().contains(view)
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::PrepareIntent { .. } => {},
        _ => {},
    }
}

/// ObservePrepare changes only the highest-PrepareQC projection and cannot
/// select a lower certificate.
pub proof fn observe_prepare_branch_postcondition(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        match frame.record {
            WalRecordProjection::ObservePrepare { certificate, .. } => {
                same_certificate(
                    after.highest_prepare,
                    highest_after_update(before.highest_prepare, certificate),
                )
                    && (!before.highest_prepare.present
                        || after.highest_prepare.view >= before.highest_prepare.view)
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::ObservePrepare { .. } => {},
        _ => {},
    }
}

/// LockAndCommit atomically installs the exact lock and matching unique Commit
/// intent in the same acknowledged frame.
pub proof fn lock_and_commit_branch_is_atomic(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        match frame.record {
            WalRecordProjection::LockAndCommit {
                prepare,
                vote_view,
                vote_subject,
                ..
            } => {
                same_certificate(after.locked, prepare)
                    && after.commit_intents.dom().contains(vote_view)
                    && after.commit_intents[vote_view] == vote_subject
                    && prepare.view == vote_view
                    && prepare.subject == vote_subject
                    && lock_extends(before.locked, after.locked)
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::LockAndCommit { .. } => {},
        _ => {},
    }
}

/// TimeoutIntent durably closes exactly the current view with the expected high
/// PrepareQC reference.
pub proof fn timeout_intent_branch_postcondition(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        match frame.record {
            WalRecordProjection::TimeoutIntent {
                view,
                high_reference,
                ..
            } => {
                view == before.view
                    && after.timeout_intents.dom().contains(view)
                    && after.timeout_intents[view] == high_reference
                    && after.view == before.view
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::TimeoutIntent { .. } => {},
        _ => {},
    }
}

/// InstallTimeout is the only durable branch that advances view; its selected
/// PrepareQC cannot regress the lock.
pub proof fn install_timeout_branch_postcondition(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        match frame.record {
            WalRecordProjection::InstallTimeout { tc_view, .. } => {
                after.last_timeout_view == tc_view
                    && after.view == tc_view + 1
                    && after.view > before.view
                    && lock_extends(before.locked, after.locked)
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::InstallTimeout { .. } => {},
        _ => {},
    }
}

/// Decision installs the exact CommitQC reference and leaves application to a
/// later reducer effect.
pub proof fn decision_branch_postcondition(
    before: WalStateProjection,
    frame: WalFrameProjection,
    after: WalStateProjection,
)
    requires
        wal_apply(before, frame, after),
    ensures
        match frame.record {
            WalRecordProjection::Decision { certificate, .. } => {
                valid_commit(after.decision)
                    && same_certificate(after.decision, certificate)
                    && after.view == before.view
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::Decision { .. } => {},
        _ => {},
    }
}

// ---------------------------------------------------------------------------
// Branch-complete reducer safety refinement
// ---------------------------------------------------------------------------

/// The four production `SignableMessage` classes.
pub enum SignKindProjection {
    /// Leader proposal signature.
    Proposal,
    /// Prepare vote signature.
    Prepare,
    /// Commit vote signature.
    Commit,
    /// Timeout vote signature.
    Timeout,
}

/// Every production `Event` variant with safety-relevant payload projected.
pub enum EventProjection {
    /// `Event::LocalProposalReady`.
    LocalProposalReady { view: int, subject: int },
    /// `Event::ProposalReceived`.
    ProposalReceived { view: int, subject: int },
    /// `Event::VoteReceived` (`prepare` distinguishes phase).
    VoteReceived { prepare: bool, view: int, subject: int },
    /// `Event::QuorumCertificateReceived`.
    QuorumCertificateReceived { prepare: bool, view: int, subject: int },
    /// `Event::TimeoutVoteReceived`.
    TimeoutVoteReceived { view: int },
    /// `Event::TimeoutCertificateReceived`.
    TimeoutCertificateReceived { view: int },
    /// `Event::TimeoutElapsed`.
    TimeoutElapsed { view: int },
    /// `Event::RetransmitElapsed`.
    RetransmitElapsed,
    /// `Event::BodyAvailable`.
    BodyAvailable { view: int, subject: int },
    /// `Event::BodyStored`.
    BodyStored { view: int, subject: int },
    /// `Event::ValidationCompleted`.
    ValidationCompleted { view: int, subject: int, valid: bool },
    /// `Event::Persisted`.
    Persisted { id: nat },
    /// `Event::PersistenceFailed`.
    PersistenceFailed { id: nat },
    /// `Event::Signed`.
    Signed { kind: SignKindProjection, view: int, subject: int },
    /// `Event::ApplicationCompleted`.
    ApplicationCompleted { subject: int },
}

/// Production event envelope.  Every asynchronous completion and authenticated
/// ingress carries the active height, persisted view, and local generation.
pub struct ReducerInputProjection {
    /// Event-tag height.
    pub tag_height: int,
    /// Event-tag view.
    pub tag_view: int,
    /// Event-tag generation.
    pub tag_generation: int,
    /// Projected event payload.
    pub event: EventProjection,
}

/// Safety-relevant continuation attached to one pending WAL frame.
pub enum ContinuationProjection {
    /// No direct signing/view/decision continuation.
    None,
    /// Sign a leader proposal after ProposalIntent acknowledgement.
    SignProposal { view: int, subject: int },
    /// Sign Prepare after PrepareIntent acknowledgement.
    SignPrepare { view: int, subject: int },
    /// Sign Commit after LockAndCommit acknowledgement.
    SignCommit { view: int, subject: int },
    /// Sign timeout after TimeoutIntent acknowledgement.
    SignTimeout { view: int },
    /// Enter the successor view after TC acknowledgement.
    InstallTimeout { tc_view: int },
    /// Handle a CommitQC only after Decision acknowledgement.
    Decide { view: int, subject: int },
}

/// One signing request emitted by the reducer.
pub struct SignEffectProjection {
    /// Whether signing was requested.
    pub present: bool,
    /// Signed message class.
    pub kind: SignKindProjection,
    /// Signed view.
    pub view: int,
    /// Signed subject; ignored for timeout.
    pub subject: int,
}

/// Safety-relevant subset of the reducer's effect list.
pub struct EffectProjection {
    /// A complete WAL frame append was requested.
    pub persist: bool,
    /// A signing request was issued.
    pub sign: SignEffectProjection,
    /// A decided body application was requested.
    pub apply: bool,
    /// Subject passed to Apply.
    pub apply_subject: int,
    /// A persisted TC caused an EnterView notification.
    pub enter_view: bool,
}

/// Safety projection of the executable reducer plus adapter-held historical
/// body-store tokens.  The two sets are monotone for the active height even
/// when production `body_work` is cleared on a view change.
pub struct ReducerProjection {
    /// WAL-derived durable state.
    pub durable: WalStateProjection,
    /// Local asynchronous completion generation.
    pub generation: int,
    /// Whether exactly one WAL append is outstanding.
    pub pending: bool,
    /// Outstanding frame; ignored when `pending` is false.
    pub pending_frame: WalFrameProjection,
    /// Continuation; ignored when `pending` is false.
    pub continuation: ContinuationProjection,
    /// Subjects acknowledged by the exact-body durable store.
    pub durable_bodies: Set<int>,
    /// Subjects accepted by deterministic validation after durable storage.
    pub validated_bodies: Set<int>,
    /// Subjects whose exact body is currently present as `BodyState::Validated`.
    /// Unlike historical validation, this set is cleared when a TC installs a
    /// new reducer generation and on crash recovery.
    pub application_ready: Set<int>,
    /// Whether local application completed.
    pub applied: bool,
    /// Applied subject when `applied` is true.
    pub applied_subject: int,
}

/// Reducer source branches at the safety projection boundary.
pub enum ReducerPathProjection {
    /// Ignored, rejected transactionally, or volatile-only processing.
    NoDurableChange,
    /// Accepted body store/validation progress without a WAL append.
    BodyProgress,
    /// `start_persistence` installed the sole pending frame.
    StartPersistence {
        /// Requested frame.
        frame: WalFrameProjection,
        /// Continuation stored with it.
        continuation: ContinuationProjection,
    },
    /// `on_persisted` applied the sole matching pending frame.
    AcknowledgePersistence,
    /// `on_application_completed` accepted the exact decided subject.
    CompleteApplication,
}

/// Durable-state equality used by all non-acknowledgement reducer branches.
pub open spec fn same_wal_state(
    left: WalStateProjection,
    right: WalStateProjection,
) -> bool {
    wal_states_equivalent(left, right)
}

/// Continuation kind must correspond exactly to its production WAL record.
pub open spec fn continuation_matches(
    record: WalRecordProjection,
    continuation: ContinuationProjection,
) -> bool {
    match (record, continuation) {
        (
            WalRecordProjection::ProposalIntent { view: rv, subject: rs, .. },
            ContinuationProjection::SignProposal { view: cv, subject: cs },
        ) => rv == cv && rs == cs,
        (
            WalRecordProjection::PrepareIntent { view: rv, subject: rs, .. },
            ContinuationProjection::SignPrepare { view: cv, subject: cs },
        ) => rv == cv && rs == cs,
        (
            WalRecordProjection::LockAndCommit {
                vote_view: rv,
                vote_subject: rs,
                ..
            },
            ContinuationProjection::SignCommit { view: cv, subject: cs },
        ) => rv == cv && rs == cs,
        (
            WalRecordProjection::TimeoutIntent { view: rv, .. },
            ContinuationProjection::SignTimeout { view: cv },
        ) => rv == cv,
        (
            WalRecordProjection::InstallTimeout { tc_view: rv, .. },
            ContinuationProjection::InstallTimeout { tc_view: cv },
        ) => rv == cv,
        (
            WalRecordProjection::Decision { certificate, .. },
            ContinuationProjection::Decide { view, subject },
        ) => certificate.view == view && certificate.subject == subject,
        (WalRecordProjection::ObservePrepare { .. }, ContinuationProjection::None) => true,
        _ => false,
    }
}

/// Events permitted to reach each `start_persistence` call site in
/// `Reducer::step` and the synchronous handlers it invokes.
pub open spec fn event_may_start_record(
    event: EventProjection,
    record: WalRecordProjection,
) -> bool {
    match (event, record) {
        (
            EventProjection::LocalProposalReady { view: ev, subject: es },
            WalRecordProjection::ProposalIntent { view: rv, subject: rs, .. },
        ) => ev == rv && es == rs,
        (
            EventProjection::ValidationCompleted { view: ev, subject: es, valid: true },
            WalRecordProjection::PrepareIntent { view: rv, subject: rs, .. },
        ) => ev == rv && es == rs,
        (
            EventProjection::ValidationCompleted { view: ev, subject: es, valid: true },
            WalRecordProjection::LockAndCommit { vote_view: rv, vote_subject: rs, .. },
        ) => ev == rv && es == rs,
        (EventProjection::TimeoutElapsed { view: ev }, WalRecordProjection::TimeoutIntent { view: rv, .. }) => ev == rv,
        (
            EventProjection::VoteReceived { prepare: true, view: ev, subject: es },
            WalRecordProjection::ObservePrepare { certificate, .. },
        ) => certificate.view == ev && certificate.subject == es,
        (
            EventProjection::VoteReceived { prepare: true, view: ev, subject: es },
            WalRecordProjection::LockAndCommit { prepare, .. },
        ) => prepare.view == ev && prepare.subject == es,
        (
            EventProjection::VoteReceived { prepare: false, view: ev, subject: es },
            WalRecordProjection::Decision { certificate, .. },
        ) => certificate.view == ev && certificate.subject == es,
        (
            EventProjection::QuorumCertificateReceived { prepare: true, view: ev, subject: es },
            WalRecordProjection::ObservePrepare { certificate, .. },
        ) => certificate.view == ev && certificate.subject == es,
        (
            EventProjection::QuorumCertificateReceived { prepare: true, view: ev, subject: es },
            WalRecordProjection::LockAndCommit { prepare, .. },
        ) => prepare.view == ev && prepare.subject == es,
        (
            EventProjection::QuorumCertificateReceived { prepare: false, view: ev, subject: es },
            WalRecordProjection::Decision { certificate, .. },
        ) => certificate.view == ev && certificate.subject == es,
        (
            EventProjection::TimeoutVoteReceived { view: ev },
            WalRecordProjection::InstallTimeout { tc_view: rv, .. },
        ) => ev == rv,
        (
            EventProjection::TimeoutCertificateReceived { view: ev },
            WalRecordProjection::InstallTimeout { tc_view: rv, .. },
        ) => ev == rv,
        (
            EventProjection::Signed { kind: SignKindProjection::Proposal, view: ev, subject: es },
            WalRecordProjection::PrepareIntent { view: rv, subject: rs, .. },
        ) => ev == rv && es == rs,
        (
            EventProjection::Signed { kind: SignKindProjection::Prepare, view: ev, subject: es },
            WalRecordProjection::ObservePrepare { certificate, .. },
        ) => certificate.view == ev && certificate.subject == es,
        (
            EventProjection::Signed { kind: SignKindProjection::Prepare, view: ev, subject: es },
            WalRecordProjection::LockAndCommit { prepare, .. },
        ) => prepare.view == ev && prepare.subject == es,
        (
            EventProjection::Signed { kind: SignKindProjection::Commit, view: ev, subject: es },
            WalRecordProjection::Decision { certificate, .. },
        ) => certificate.view == ev && certificate.subject == es,
        (
            EventProjection::Signed { kind: SignKindProjection::Timeout, view: ev, .. },
            WalRecordProjection::InstallTimeout { tc_view: rv, .. },
        ) => ev == rv,
        _ => false,
    }
}

/// Historical body facts supplied by accepted body-adapter completions.
pub open spec fn body_history_transition(
    before: ReducerProjection,
    event: EventProjection,
    after: ReducerProjection,
) -> bool {
    match event {
        EventProjection::LocalProposalReady { subject, .. } => {
            after.durable_bodies =~= before.durable_bodies.insert(subject)
                && after.validated_bodies =~= before.validated_bodies.insert(subject)
                && after.application_ready =~= before.application_ready.insert(subject)
        }
        EventProjection::BodyStored { subject, .. } => {
            after.durable_bodies =~= before.durable_bodies.insert(subject)
                && after.validated_bodies =~= before.validated_bodies
                && after.application_ready =~= before.application_ready
        }
        EventProjection::ValidationCompleted { subject, valid: true, .. } => {
            before.durable_bodies.contains(subject)
                && after.durable_bodies =~= before.durable_bodies
                && after.validated_bodies =~= before.validated_bodies.insert(subject)
                && after.application_ready =~= before.application_ready.insert(subject)
        }
        EventProjection::ValidationCompleted { subject, valid: false, .. } => {
            before.durable_bodies.contains(subject)
                && after.durable_bodies =~= before.durable_bodies
                && after.validated_bodies =~= before.validated_bodies
                && after.application_ready =~= before.application_ready
        }
        EventProjection::BodyAvailable { .. } => {
            after.durable_bodies =~= before.durable_bodies
                && after.validated_bodies =~= before.validated_bodies
                && after.application_ready =~= before.application_ready
        }
        _ => false,
    }
}

/// Accepted body progress never creates a validation token before its durable
/// exact-body token.
pub proof fn body_history_preserves_storage_before_validation(
    before: ReducerProjection,
    event: EventProjection,
    after: ReducerProjection,
)
    requires
        before.validated_bodies.subset_of(before.durable_bodies),
        before.application_ready.subset_of(before.validated_bodies),
        body_history_transition(before, event, after),
    ensures
        after.validated_bodies.subset_of(after.durable_bodies),
        after.application_ready.subset_of(after.validated_bodies),
{
    match event {
        EventProjection::LocalProposalReady { .. } => {},
        EventProjection::BodyStored { .. } => {},
        EventProjection::ValidationCompleted { valid: true, .. } => {},
        EventProjection::ValidationCompleted { valid: false, .. } => {},
        EventProjection::BodyAvailable { .. } => {},
        _ => {},
    }
}

/// A signing effect is authorized by a complete durable intent and, where
/// required, by the retained exact-body history.
pub open spec fn signing_effect_is_safe(
    state: ReducerProjection,
    sign: SignEffectProjection,
) -> bool {
    !sign.present
        || match sign.kind {
            SignKindProjection::Proposal => {
                state.durable.proposal_intents.dom().contains(sign.view)
                    && state.durable.proposal_intents[sign.view] == sign.subject
                    && state.durable_bodies.contains(sign.subject)
                    && state.validated_bodies.contains(sign.subject)
            }
            SignKindProjection::Prepare => {
                state.durable.prepare_intents.dom().contains(sign.view)
                    && state.durable.prepare_intents[sign.view] == sign.subject
                    && state.durable_bodies.contains(sign.subject)
                    && state.validated_bodies.contains(sign.subject)
            }
            SignKindProjection::Commit => {
                state.durable.commit_intents.dom().contains(sign.view)
                    && state.durable.commit_intents[sign.view] == sign.subject
            }
            SignKindProjection::Timeout => {
                state.durable.timeout_intents.dom().contains(sign.view)
            }
        }
}

/// Apply is authorized only by a durable CommitQC for a validated durable body.
pub open spec fn apply_effect_is_safe(
    state: ReducerProjection,
    effects: EffectProjection,
) -> bool {
    !effects.apply
        || (state.durable.decision.present
            && state.durable.decision.subject == effects.apply_subject
            && state.durable_bodies.contains(effects.apply_subject)
            && state.validated_bodies.contains(effects.apply_subject)
            && state.application_ready.contains(effects.apply_subject))
}

/// Reducer invariant spanning WAL state and the body adapter's monotone tokens.
pub open spec fn reducer_invariant(state: ReducerProjection) -> bool {
    wal_invariant(state.durable)
        && 0 <= state.generation <= machine_u64_max()
        && state.validated_bodies.subset_of(state.durable_bodies)
        && state.application_ready.subset_of(state.validated_bodies)
        && (!state.pending
            || (wal_frame_admissible(state.durable, state.pending_frame)
                && continuation_matches(
                    state.pending_frame.record,
                    state.continuation,
                )))
        && (!state.applied
            || (state.durable.decision.present
                && state.applied_subject == state.durable.decision.subject
                && state.validated_bodies.contains(state.applied_subject)))
}

/// Effects that do not append a WAL frame still obey signing/application fences.
pub open spec fn non_persist_effects_safe(
    state: ReducerProjection,
    effects: EffectProjection,
) -> bool {
    !effects.persist
        && signing_effect_is_safe(state, effects.sign)
        && apply_effect_is_safe(state, effects)
}

/// Exact `reject_tag` predicate for the safety projection.
pub open spec fn input_tag_matches(
    state: ReducerProjection,
    input: ReducerInputProjection,
) -> bool {
    input.tag_height == state.durable.height
        && input.tag_view == state.durable.view
        && input.tag_generation == state.generation
}

/// No persistence, signing, application, or view effect was emitted.
pub open spec fn no_safety_effect(effects: EffectProjection) -> bool {
    !effects.persist && !effects.sign.present && !effects.apply && !effects.enter_view
}

/// The two events allowed through the pending-persistence busy fence.
pub open spec fn is_persistence_completion(event: EventProjection) -> bool {
    match event {
        EventProjection::Persisted { .. } | EventProjection::PersistenceFailed { .. } => true,
        _ => false,
    }
}

/// The `NoDurableChange` branch may retransmit Apply only on the timer path and
/// may request the next already-authorized signature only after Signed.
pub open spec fn no_change_effects_match_input(
    before: ReducerProjection,
    input: ReducerInputProjection,
    after: ReducerProjection,
    effects: EffectProjection,
) -> bool {
    if !input_tag_matches(before, input)
        || (before.pending && !is_persistence_completion(input.event))
    {
        no_safety_effect(effects)
    } else {
        non_persist_effects_safe(after, effects)
            && !effects.enter_view
            && (effects.apply ==> match input.event {
                EventProjection::RetransmitElapsed => true,
                _ => false,
            })
            && (effects.sign.present ==> match input.event {
                EventProjection::Signed { .. } => true,
                _ => false,
            })
    }
}

/// Safety-state equality for ignored, rejected, and volatile-only branches.
pub open spec fn same_reducer_projection(
    before: ReducerProjection,
    after: ReducerProjection,
) -> bool {
    same_wal_state(before.durable, after.durable)
        && before.generation == after.generation
        && before.pending == after.pending
        && (!before.pending
            || (before.pending_frame.id == after.pending_frame.id
                && before.pending_frame.context == after.pending_frame.context
                && before.pending_frame.record == after.pending_frame.record
                && before.continuation == after.continuation))
        && before.durable_bodies =~= after.durable_bodies
        && before.validated_bodies =~= after.validated_bodies
        && before.application_ready =~= after.application_ready
        && before.applied == after.applied
        && (!before.applied || before.applied_subject == after.applied_subject)
}

/// Exact safety-effect behavior after acknowledgement of each continuation.
/// A queued replay signature may be selected ahead of the just-acknowledged
/// continuation, but any such selection must satisfy the durable sign fence.
pub open spec fn acknowledgement_effects_match(
    before: ReducerProjection,
    after: ReducerProjection,
    effects: EffectProjection,
) -> bool {
    non_persist_effects_safe(after, effects)
        && match before.continuation {
            ContinuationProjection::None => {
                after.generation == before.generation
                    && !effects.enter_view
                    && !effects.apply
            }
            ContinuationProjection::SignProposal { .. }
            | ContinuationProjection::SignPrepare { .. }
            | ContinuationProjection::SignCommit { .. }
            | ContinuationProjection::SignTimeout { .. } => {
                after.generation == before.generation
                    && !effects.enter_view
                    && !effects.apply
                    && effects.sign.present
            }
            ContinuationProjection::InstallTimeout { .. } => {
                before.generation < machine_u64_max()
                    && after.generation == before.generation + 1
                    && effects.enter_view
                    && !effects.apply
            }
            ContinuationProjection::Decide { subject, .. } => {
                after.generation == before.generation
                    && !effects.enter_view
                    && (effects.apply
                        <==> (after.application_ready.contains(subject)
                            && !after.applied))
                    && (!effects.apply || effects.apply_subject == subject)
            }
        }
}

/// The single branch-complete safety refinement relation for `Reducer::step`.
pub open spec fn reducer_step_refines(
    before: ReducerProjection,
    input: ReducerInputProjection,
    path: ReducerPathProjection,
    effects: EffectProjection,
    after: ReducerProjection,
) -> bool {
    match path {
        ReducerPathProjection::NoDurableChange => {
            same_reducer_projection(before, after)
                && no_change_effects_match_input(before, input, after, effects)
        }
        ReducerPathProjection::BodyProgress => {
            input_tag_matches(before, input)
                && match input.event {
                EventProjection::BodyAvailable { .. }
                | EventProjection::BodyStored { .. }
                | EventProjection::ValidationCompleted { .. } => true,
                _ => false,
            }
                && same_wal_state(before.durable, after.durable)
                && before.generation == after.generation
                && !before.pending
                && !after.pending
                && body_history_transition(before, input.event, after)
                && before.applied == after.applied
                && (!before.applied || before.applied_subject == after.applied_subject)
                && non_persist_effects_safe(after, effects)
                && !effects.enter_view
                && !effects.sign.present
                && (effects.apply ==> match input.event {
                    EventProjection::ValidationCompleted { valid: true, .. } => true,
                    _ => false,
                })
        }
        ReducerPathProjection::StartPersistence {
            frame,
            continuation,
        } => {
            !before.pending
                && input_tag_matches(before, input)
                && event_may_start_record(input.event, frame.record)
                && wal_frame_admissible(before.durable, frame)
                && continuation_matches(frame.record, continuation)
                && same_wal_state(before.durable, after.durable)
                && before.generation == after.generation
                && after.pending
                && after.pending_frame.id == frame.id
                && after.pending_frame.context == frame.context
                && after.pending_frame.record == frame.record
                && after.continuation == continuation
                && (match input.event {
                    EventProjection::LocalProposalReady { .. }
                    | EventProjection::ValidationCompleted { valid: true, .. } => {
                        body_history_transition(before, input.event, after)
                    }
                    _ => {
                        after.durable_bodies =~= before.durable_bodies
                            && after.validated_bodies =~= before.validated_bodies
                            && after.application_ready =~= before.application_ready
                    }
                })
                && before.applied == after.applied
                && (!before.applied || before.applied_subject == after.applied_subject)
                && effects.persist
                && !effects.sign.present
                && !effects.apply
                && !effects.enter_view
        }
        ReducerPathProjection::AcknowledgePersistence => {
            input_tag_matches(before, input)
                && match input.event {
                EventProjection::Persisted { id } => before.pending && id == before.pending_frame.id,
                _ => false,
            }
                && wal_apply(before.durable, before.pending_frame, after.durable)
                && !after.pending
                && after.durable_bodies =~= before.durable_bodies
                && after.validated_bodies =~= before.validated_bodies
                && (match before.continuation {
                    ContinuationProjection::InstallTimeout { .. } => {
                        after.application_ready =~= Set::empty()
                    }
                    _ => after.application_ready =~= before.application_ready,
                })
                && before.applied == after.applied
                && (!before.applied || before.applied_subject == after.applied_subject)
                && acknowledgement_effects_match(before, after, effects)
        }
        ReducerPathProjection::CompleteApplication => {
            input_tag_matches(before, input)
                && match input.event {
                EventProjection::ApplicationCompleted { subject } => {
                    before.durable.decision.present
                        && before.durable.decision.subject == subject
                        && before.validated_bodies.contains(subject)
                        && before.application_ready.contains(subject)
                        && after.applied
                        && after.applied_subject == subject
                }
                _ => false,
            }
                && same_wal_state(before.durable, after.durable)
                && before.generation == after.generation
                && !before.pending
                && !after.pending
                && after.durable_bodies =~= before.durable_bodies
                && after.validated_bodies =~= before.validated_bodies
                && after.application_ready =~= before.application_ready
                && !effects.persist
                && !effects.sign.present
                && !effects.apply
                && !effects.enter_view
        }
    }
}

/// Every reducer branch preserves the combined WAL/body/application invariant.
pub proof fn reducer_step_preserves_invariant(
    before: ReducerProjection,
    input: ReducerInputProjection,
    path: ReducerPathProjection,
    effects: EffectProjection,
    after: ReducerProjection,
)
    requires
        reducer_invariant(before),
        reducer_step_refines(before, input, path, effects, after),
    ensures
        reducer_invariant(after),
{
    match path {
        ReducerPathProjection::NoDurableChange => {},
        ReducerPathProjection::BodyProgress => {
            body_history_preserves_storage_before_validation(
                before,
                input.event,
                after,
            );
        },
        ReducerPathProjection::StartPersistence { .. } => {
            match input.event {
                EventProjection::LocalProposalReady { .. }
                | EventProjection::ValidationCompleted { valid: true, .. } => {
                    body_history_preserves_storage_before_validation(
                        before,
                        input.event,
                        after,
                    );
                },
                _ => {},
            }
        },
        ReducerPathProjection::AcknowledgePersistence => {
            wal_apply_preserves_invariant(
                before.durable,
                before.pending_frame,
                after.durable,
            );
        },
        ReducerPathProjection::CompleteApplication => {},
    }
}

/// Every emitted signing, persistence, view, and application effect is on the
/// far side of its corresponding durable fence.
pub proof fn reducer_step_preserves_effect_ordering(
    before: ReducerProjection,
    input: ReducerInputProjection,
    path: ReducerPathProjection,
    effects: EffectProjection,
    after: ReducerProjection,
)
    requires
        reducer_invariant(before),
        reducer_step_refines(before, input, path, effects, after),
    ensures
        effects.persist ==> after.pending,
        signing_effect_is_safe(after, effects.sign),
        apply_effect_is_safe(after, effects),
        effects.enter_view ==> after.durable.view > before.durable.view,
{
    match path {
        ReducerPathProjection::AcknowledgePersistence => {
            wal_apply_preserves_view_monotonicity(
                before.durable,
                before.pending_frame,
                after.durable,
            );
        },
        ReducerPathProjection::NoDurableChange
        | ReducerPathProjection::BodyProgress
        | ReducerPathProjection::StartPersistence { .. }
        | ReducerPathProjection::CompleteApplication => {},
    }
}

/// Reducer steps inherit durable vote uniqueness from the sole WAL-ack branch.
pub proof fn reducer_step_preserves_vote_uniqueness(
    before: ReducerProjection,
    input: ReducerInputProjection,
    path: ReducerPathProjection,
    effects: EffectProjection,
    after: ReducerProjection,
)
    requires
        reducer_step_refines(before, input, path, effects, after),
    ensures
        forall |view: int|
            #![trigger before.durable.prepare_intents[view], after.durable.prepare_intents[view]]
            before.durable.prepare_intents.dom().contains(view)
            ==> after.durable.prepare_intents.dom().contains(view)
                && after.durable.prepare_intents[view]
                    == before.durable.prepare_intents[view],
        forall |view: int|
            #![trigger before.durable.commit_intents[view], after.durable.commit_intents[view]]
            before.durable.commit_intents.dom().contains(view)
            ==> after.durable.commit_intents.dom().contains(view)
                && after.durable.commit_intents[view]
                    == before.durable.commit_intents[view],
{
    match path {
        ReducerPathProjection::AcknowledgePersistence => {
            wal_apply_preserves_all_existing_intents(
                before.durable,
                before.pending_frame,
                after.durable,
            );
        },
        ReducerPathProjection::NoDurableChange
        | ReducerPathProjection::BodyProgress
        | ReducerPathProjection::StartPersistence { .. }
        | ReducerPathProjection::CompleteApplication => {},
    }
}

/// Reducer steps inherit lock and decision monotonicity from the sole WAL-ack
/// branch; every other event leaves durable state unchanged.
pub proof fn reducer_step_preserves_lock_and_decision(
    before: ReducerProjection,
    input: ReducerInputProjection,
    path: ReducerPathProjection,
    effects: EffectProjection,
    after: ReducerProjection,
)
    requires
        reducer_step_refines(before, input, path, effects, after),
    ensures
        lock_extends(before.durable.locked, after.durable.locked),
        before.durable.decision.present
            ==> same_certificate(after.durable.decision, before.durable.decision),
{
    match path {
        ReducerPathProjection::AcknowledgePersistence => {
            wal_apply_preserves_lock_monotonicity(
                before.durable,
                before.pending_frame,
                after.durable,
            );
            if before.durable.decision.present {
                wal_apply_preserves_decision_uniqueness(
                    before.durable,
                    before.pending_frame,
                    after.durable,
                );
            }
        },
        ReducerPathProjection::NoDurableChange
        | ReducerPathProjection::BodyProgress
        | ReducerPathProjection::StartPersistence { .. }
        | ReducerPathProjection::CompleteApplication => {},
    }
}

/// Crash recovery projects to replaying a complete WAL prefix.  Volatile
/// pending work and body reconstruction may be lost, while durable intents,
/// lock, view, and decision remain identical.
pub open spec fn crash_recovery(
    before: ReducerProjection,
    after: ReducerProjection,
) -> bool {
    same_wal_state(before.durable, after.durable)
        && !after.pending
        && 0 <= after.generation <= machine_u64_max()
        && after.durable_bodies =~= before.durable_bodies
        && after.validated_bodies =~= before.validated_bodies
        && after.application_ready =~= Set::empty()
        && !after.applied
}

/// Replay preserves all safety invariants; application acknowledgement is
/// deliberately volatile and must be reacquired from Kura after restart.
pub proof fn crash_recovery_preserves_safety(
    before: ReducerProjection,
    after: ReducerProjection,
)
    requires
        reducer_invariant(before),
        crash_recovery(before, after),
    ensures
        reducer_invariant(after),
        wal_invariant(after.durable),
        after.validated_bodies.subset_of(after.durable_bodies),
        after.application_ready.subset_of(after.validated_bodies),
        !after.applied,
{
}

// ---------------------------------------------------------------------------
// Exact executable production commit gate
// ---------------------------------------------------------------------------

/// Verus-side shape of the fixed production effect trace.  This mirrors the
/// private `refinement::EffectTrace`; the decision expression below is shared
/// textually with production through `production_transition_gate_body!`.
#[derive(Copy, Clone)]
pub struct ProductionEffectTraceProjection {
    pub len: u8,
    pub kind0: u8,
    pub authorized0: bool,
    pub kind1: u8,
    pub authorized1: bool,
    pub kind2: u8,
    pub authorized2: bool,
    pub kind3: u8,
    pub authorized3: bool,
    pub kind4: u8,
    pub authorized4: bool,
    pub kind5: u8,
    pub authorized5: bool,
    pub kind6: u8,
    pub authorized6: bool,
    pub kind7: u8,
    pub authorized7: bool,
}

/// Verus-side shape of `refinement::VolatileSummary`.
#[derive(Copy, Clone)]
pub struct ProductionVolatileSummaryProjection {
    pub candidate_present: bool,
    pub body_work: u64,
    pub pending_prepare: u64,
    pub known_prepare: u64,
    pub vote_pools: u64,
    pub vote_entries: u64,
    pub timeout_vote_pools: u64,
    pub timeout_vote_entries: u64,
    pub formed_certificates: u64,
    pub formed_timeouts: u64,
    pub outbound_control: u64,
    pub signature_queue: u64,
    pub awaiting_signature: bool,
    pub durable_signable_limit: u64,
    pub replay_resumed: bool,
}

/// Verus-side shape of the facts extracted by the production reducer before
/// committing a candidate transition.
#[derive(Copy, Clone)]
pub struct ProductionTransitionFactsProjection {
    pub before_invariant: bool,
    pub after_invariant: bool,
    pub context_unchanged: bool,
    pub tag_matches: bool,
    pub busy_fence_open: bool,
    pub event_kind: u8,
    pub action_kind: u8,
    pub wal_record_kind: u8,
    pub signed_message_kind: u8,
    pub validator_count: u64,
    pub volatile_before: ProductionVolatileSummaryProjection,
    pub volatile_after: ProductionVolatileSummaryProjection,
    pub durable_unchanged: bool,
    pub pending_unchanged: bool,
    pub generation_unchanged: bool,
    pub application_unchanged: bool,
    pub begin_persist_exact: bool,
    pub acknowledge_persist_exact: bool,
    pub application_transition_exact: bool,
    pub acknowledgement_continuation: u8,
    pub effects: ProductionEffectTraceProjection,
}

/// Names of the TLA+ `SumeragiV2Core` actions represented at the reducer's
/// safety projection.  `SpecStutter` is the stuttering step admitted by
/// `[Next]_vars`; `NoAction` is an empty slot in a production macro-step.
///
/// This enum is an explicit, mechanically checked name/state-delta map.  It is
/// not by itself a cross-tool proof that the separately parsed TLA+ operator
/// bodies equal these Verus definitions; that remaining obligation is stated
/// in `VERIFICATION.md`.
pub enum TlaActionNameProjection {
    NoAction,
    SpecStutter,
    BeginLocalProposal,
    BeginPrepare,
    BeginObservePrepare,
    BeginLockCommit,
    BeginTimeout,
    BeginInstallTC,
    BeginDecision,
    PersistProposal,
    PersistPrepare,
    PersistObservePrepare,
    PersistLockCommit,
    PersistTimeout,
    PersistInstallTC,
    PersistDecision,
    DeliverProposal,
    DeliverVote,
    DeliverQC,
    DeliverTimeout,
    DeliverTC,
    FetchBody,
    StoreBody,
    ValidateBody,
    CompleteProposalSignature,
    CompleteVoteSignature,
    CompleteTimeoutSignature,
    FormPrepareQC,
    FormCommitQC,
    FormTC,
    ApplyDecision,
}

/// At most three TLA+ actions represented by one serialized production step:
/// authenticated/completion ingress, optional local certificate formation,
/// and the reducer's durable boundary action.
pub struct TlaMacroStepProjection {
    pub source: TlaActionNameProjection,
    pub formation: TlaActionNameProjection,
    pub boundary: TlaActionNameProjection,
}

/// TLA+ source/completion action named by the exact production event class.
pub open spec fn production_source_tla_action(
    facts: ProductionTransitionFactsProjection,
) -> TlaActionNameProjection {
    match facts.event_kind {
        1 => TlaActionNameProjection::DeliverProposal,
        2 => TlaActionNameProjection::DeliverVote,
        3 => TlaActionNameProjection::DeliverQC,
        4 => TlaActionNameProjection::DeliverTimeout,
        5 => TlaActionNameProjection::DeliverTC,
        8 => TlaActionNameProjection::FetchBody,
        9 => TlaActionNameProjection::StoreBody,
        10 => TlaActionNameProjection::ValidateBody,
        13 => match facts.signed_message_kind {
            1 => TlaActionNameProjection::CompleteProposalSignature,
            2 | 3 => TlaActionNameProjection::CompleteVoteSignature,
            4 => TlaActionNameProjection::CompleteTimeoutSignature,
            _ => TlaActionNameProjection::NoAction,
        },
        _ => TlaActionNameProjection::NoAction,
    }
}

/// Optional TLA+ certificate-formation action coalesced into an ingress/sign
/// macro-step by the executable reducer.
pub open spec fn production_formation_tla_action(
    facts: ProductionTransitionFactsProjection,
) -> TlaActionNameProjection {
    if facts.action_kind != 1 {
        TlaActionNameProjection::NoAction
    } else {
        match (facts.event_kind, facts.signed_message_kind, facts.wal_record_kind) {
            (2, _, 3 | 4) | (13, 2, 3 | 4) => TlaActionNameProjection::FormPrepareQC,
            (2, _, 7) | (13, 3, 7) => TlaActionNameProjection::FormCommitQC,
            (4, _, 6) | (13, 4, 6) => TlaActionNameProjection::FormTC,
            _ => TlaActionNameProjection::NoAction,
        }
    }
}

/// TLA+ action corresponding to the durable/body/application boundary of the
/// classified production transition.
pub open spec fn production_boundary_tla_action(
    facts: ProductionTransitionFactsProjection,
) -> TlaActionNameProjection {
    match facts.action_kind {
        0 => TlaActionNameProjection::SpecStutter,
        1 => match facts.wal_record_kind {
            1 => TlaActionNameProjection::BeginLocalProposal,
            2 => TlaActionNameProjection::BeginPrepare,
            3 => TlaActionNameProjection::BeginObservePrepare,
            4 => TlaActionNameProjection::BeginLockCommit,
            5 => TlaActionNameProjection::BeginTimeout,
            6 => TlaActionNameProjection::BeginInstallTC,
            7 => TlaActionNameProjection::BeginDecision,
            _ => TlaActionNameProjection::NoAction,
        },
        2 => match facts.wal_record_kind {
            1 => TlaActionNameProjection::PersistProposal,
            2 => TlaActionNameProjection::PersistPrepare,
            3 => TlaActionNameProjection::PersistObservePrepare,
            4 => TlaActionNameProjection::PersistLockCommit,
            5 => TlaActionNameProjection::PersistTimeout,
            6 => TlaActionNameProjection::PersistInstallTC,
            7 => TlaActionNameProjection::PersistDecision,
            _ => TlaActionNameProjection::NoAction,
        },
        5 => TlaActionNameProjection::ApplyDecision,
        _ => TlaActionNameProjection::NoAction,
    }
}

/// Canonical named TLA+ macro-step for one production transition.
pub open spec fn production_tla_macro_step(
    facts: ProductionTransitionFactsProjection,
) -> TlaMacroStepProjection {
    TlaMacroStepProjection {
        source: production_source_tla_action(facts),
        formation: production_formation_tla_action(facts),
        boundary: production_boundary_tla_action(facts),
    }
}

/// State-delta compatibility at the shared safety projection.  This names the
/// exact pending-WAL/durable/view/application boundary represented by each TLA+
/// action family; transport queues and certificate payloads remain outside
/// this compact production gate.
pub open spec fn production_tla_boundary_delta(
    facts: ProductionTransitionFactsProjection,
    action: TlaActionNameProjection,
) -> bool {
    match action {
        TlaActionNameProjection::SpecStutter => {
            facts.action_kind == 0
                && facts.durable_unchanged
                && facts.pending_unchanged
                && facts.generation_unchanged
                && facts.application_unchanged
                && production_volatile_summaries_equal(
                    facts.volatile_before,
                    facts.volatile_after,
                )
                && facts.effects.len == 0
        }
        TlaActionNameProjection::BeginLocalProposal
        | TlaActionNameProjection::BeginPrepare
        | TlaActionNameProjection::BeginObservePrepare
        | TlaActionNameProjection::BeginLockCommit
        | TlaActionNameProjection::BeginTimeout
        | TlaActionNameProjection::BeginInstallTC
        | TlaActionNameProjection::BeginDecision => {
            facts.action_kind == 1
                && facts.begin_persist_exact
                && facts.durable_unchanged
                && !facts.pending_unchanged
                && facts.generation_unchanged
                && facts.application_unchanged
        }
        TlaActionNameProjection::PersistProposal
        | TlaActionNameProjection::PersistPrepare
        | TlaActionNameProjection::PersistObservePrepare
        | TlaActionNameProjection::PersistLockCommit
        | TlaActionNameProjection::PersistTimeout
        | TlaActionNameProjection::PersistInstallTC
        | TlaActionNameProjection::PersistDecision => {
            facts.action_kind == 2
                && facts.acknowledge_persist_exact
                && !facts.pending_unchanged
                && facts.application_unchanged
        }
        TlaActionNameProjection::ApplyDecision => {
            facts.action_kind == 5
                && facts.application_transition_exact
                && !facts.application_unchanged
                && facts.durable_unchanged
                && facts.pending_unchanged
        }
        TlaActionNameProjection::NoAction
        | TlaActionNameProjection::DeliverProposal
        | TlaActionNameProjection::DeliverVote
        | TlaActionNameProjection::DeliverQC
        | TlaActionNameProjection::DeliverTimeout
        | TlaActionNameProjection::DeliverTC
        | TlaActionNameProjection::FetchBody
        | TlaActionNameProjection::StoreBody
        | TlaActionNameProjection::ValidateBody
        | TlaActionNameProjection::CompleteProposalSignature
        | TlaActionNameProjection::CompleteVoteSignature
        | TlaActionNameProjection::CompleteTimeoutSignature
        | TlaActionNameProjection::FormPrepareQC
        | TlaActionNameProjection::FormCommitQC
        | TlaActionNameProjection::FormTC => true,
    }
}

/// The action relation enforced at the caller-visible production commit
/// boundary.  Effect kinds are the exhaustive production discriminants:
/// Persist=1, Fetch=2, Store=3, Validate=4, Sign=5, Broadcast=6, Apply=7,
/// EnterView=8, and report=9.
pub open spec fn production_effect_slots_authorized(
    trace: ProductionEffectTraceProjection,
) -> bool {
    effect_slots_authorized_body!(trace)
}

/// Exact boundedness relation enforced for each concrete volatile summary.
pub open spec fn production_volatile_summary_well_formed(
    summary: ProductionVolatileSummaryProjection,
    validator_count: u64,
) -> bool {
    volatile_summary_well_formed_body!(summary, validator_count)
}

/// Exact equality relation used for stale, busy, and rejected stutters.
pub open spec fn production_volatile_summaries_equal(
    before: ProductionVolatileSummaryProjection,
    after: ProductionVolatileSummaryProjection,
) -> bool {
    volatile_summaries_equal_body!(before, after)
}

/// Signed-completion discriminator relation enforced by production.
pub closed spec fn production_signed_message_class_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    signed_message_class_body!(facts)
}

/// Exact stutter-action relation enforced by production.
pub closed spec fn production_stutter_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    stutter_action_body!(facts)
}

/// Exact begin-WAL action relation enforced by production.
pub closed spec fn production_begin_wal_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    begin_wal_action_body!(facts)
}

/// Exact acknowledge-WAL action relation enforced by production.
pub closed spec fn production_acknowledge_wal_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    acknowledge_wal_action_body!(facts)
}

/// Exact successful-validation effect relation enforced by production.
pub closed spec fn production_validation_completed_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    validation_completed_action_body!(facts, production_effect_count)
}

/// Exact body-pipeline action relation enforced by production.
pub closed spec fn production_body_progress_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    body_progress_action_body!(facts, production_validation_completed_action_relation)
}

/// Exact volatile-protocol action relation enforced by production.
pub closed spec fn production_volatile_protocol_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    volatile_protocol_action_body!(facts)
}

/// Exact application-completion action relation enforced by production.
pub closed spec fn production_complete_application_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    complete_application_action_body!(facts)
}

/// Exact action-discriminant relation enforced by production.
pub closed spec fn production_action_kind_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    action_kind_relation_body!(
        facts,
        production_stutter_action_relation,
        production_begin_wal_action_relation,
        production_acknowledge_wal_action_relation,
        production_body_progress_action_relation,
        production_volatile_protocol_action_relation,
        production_complete_application_action_relation,
    )
}

/// Named atomic action/WAL-class relation enforced by the production gate.
pub closed spec fn production_named_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    production_action_relation_body!(
        facts,
        production_signed_message_class_relation,
        production_action_kind_relation,
    )
}

/// Exact order and mutual-exclusion rules for safety-bearing effects.
pub open spec fn production_effect_count(
    trace: ProductionEffectTraceProjection,
    kind: u8,
) -> u64 {
    effect_count_body!(trace, kind)
}

/// Order rules once exact discriminant counts have been computed.
pub open spec fn production_effect_order_constraints(
    trace: ProductionEffectTraceProjection,
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

/// Exact order and mutual-exclusion rules for safety-bearing effects.
pub open spec fn production_effect_order_relation(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
) -> bool {
    production_effect_order_constraints(
        trace,
        event_kind,
        production_effect_count(trace, 1),
        production_effect_count(trace, 2),
        production_effect_count(trace, 3),
        production_effect_count(trace, 4),
        production_effect_count(trace, 5),
        production_effect_count(trace, 7),
        production_effect_count(trace, 8),
    )
}

/// Complete fixed-vector effect relation.
pub open spec fn production_effect_trace_relation(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
) -> bool {
    production_effect_slots_authorized(trace)
        && production_effect_order_relation(trace, event_kind)
}

/// Branch relation after the exact effect trace has passed its independent
/// authorization and ordering check.
pub open spec fn production_transition_branch_constraints(
    facts: ProductionTransitionFactsProjection,
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

/// Branch relation after the exact effect trace has passed its independent
/// authorization and ordering check.
pub open spec fn production_transition_branch_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    production_transition_branch_constraints(
        facts,
        production_effect_count(facts.effects, 1),
        production_effect_count(facts.effects, 2),
        production_effect_count(facts.effects, 5),
        production_effect_count(facts.effects, 7),
        production_effect_count(facts.effects, 8),
    )
}

pub closed spec fn production_transition_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    facts.before_invariant
        && facts.after_invariant
        && facts.context_unchanged
        && production_volatile_summary_well_formed(
            facts.volatile_before,
            facts.validator_count,
        )
        && production_volatile_summary_well_formed(
            facts.volatile_after,
            facts.validator_count,
        )
        && production_named_action_relation(facts)
        && production_effect_trace_relation(facts.effects, facts.event_kind)
        && production_transition_branch_relation(facts)
}

/// Every accepted production action has an explicit TLA+ macro-step name and
/// its durable-boundary name permits exactly the projected state delta.
pub proof fn production_action_has_named_tla_mapping(
    facts: ProductionTransitionFactsProjection,
)
    requires
        production_transition_action_relation(facts),
    ensures
        production_tla_boundary_delta(
            facts,
            production_tla_macro_step(facts).boundary,
        ),
        production_tla_macro_step(facts).boundary
                == TlaActionNameProjection::NoAction
            ==> facts.action_kind == 3 || facts.action_kind == 4,
{
    reveal(production_transition_action_relation);
    reveal(production_named_action_relation);
    reveal(production_action_kind_relation);
    reveal(production_stutter_action_relation);
    reveal(production_begin_wal_action_relation);
    reveal(production_acknowledge_wal_action_relation);
    reveal(production_body_progress_action_relation);
    reveal(production_volatile_protocol_action_relation);
    reveal(production_complete_application_action_relation);
    match facts.action_kind {
        0 => {},
        1 | 2 => {
            match facts.wal_record_kind {
                1 | 2 | 3 | 4 | 5 | 6 | 7 => {},
                _ => {},
            }
        },
        3 | 4 | 5 => {},
        _ => {},
    }
}

/// The executable gate bounds the safety-relevant volatile structures on both
/// sides of every committed step and makes the persisted-TC reset explicit.
pub proof fn production_action_preserves_volatile_bounds(
    facts: ProductionTransitionFactsProjection,
)
    requires
        production_transition_action_relation(facts),
    ensures
        facts.volatile_after.vote_pools <= 2,
        facts.volatile_after.vote_entries <= facts.validator_count * 2,
        facts.volatile_after.timeout_vote_pools <= 1,
        facts.volatile_after.timeout_vote_entries <= facts.validator_count,
        facts.volatile_after.formed_certificates <= 2,
        facts.volatile_after.formed_timeouts <= 1,
        facts.volatile_after.outbound_control <= 7,
        facts.volatile_after.pending_prepare <= facts.volatile_after.known_prepare,
        facts.volatile_after.known_prepare - facts.volatile_after.pending_prepare <= 2,
        facts.volatile_after.signature_queue
            <= facts.volatile_after.durable_signable_limit,
        facts.volatile_after.awaiting_signature
            ==> facts.volatile_after.signature_queue
                < facts.volatile_after.durable_signable_limit,
        facts.action_kind == 2 && facts.wal_record_kind == 6
            ==> !facts.volatile_after.candidate_present
                && facts.volatile_after.body_work == 0
                && facts.volatile_after.pending_prepare == 0
                && facts.volatile_after.vote_pools == 0
                && facts.volatile_after.vote_entries == 0
                && facts.volatile_after.timeout_vote_pools == 0
                && facts.volatile_after.timeout_vote_entries == 0
                && facts.volatile_after.formed_certificates == 0
                && facts.volatile_after.formed_timeouts == 0
                && facts.volatile_after.known_prepare <= 2
                && facts.volatile_after.outbound_control <= 3,
{
    reveal(production_transition_action_relation);
}

/// Exact executable volatile-cardinality checker used by production.
pub fn verified_volatile_summary_gate(
    summary: ProductionVolatileSummaryProjection,
    validator_count: u64,
) -> (accepted: bool)
    ensures
        accepted ==> production_volatile_summary_well_formed(summary, validator_count),
{
    volatile_summary_well_formed_body!(summary, validator_count)
}

/// Exact executable signed-completion classifier used by production.
pub fn verified_signed_message_class_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_signed_message_class_relation(facts),
{
    let accepted = signed_message_class_body!(facts);
    proof {
        reveal(production_signed_message_class_relation);
    }
    accepted
}

/// Exact executable stutter-action checker used by production.
pub fn verified_stutter_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_stutter_action_relation(facts),
{
    let accepted = stutter_action_body!(facts);
    proof {
        reveal(production_stutter_action_relation);
    }
    accepted
}

/// Exact executable begin-WAL action checker used by production.
pub fn verified_begin_wal_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_begin_wal_action_relation(facts),
{
    let accepted = begin_wal_action_body!(facts);
    proof {
        reveal(production_begin_wal_action_relation);
    }
    accepted
}

/// Exact executable acknowledge-WAL action checker used by production.
pub fn verified_acknowledge_wal_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_acknowledge_wal_action_relation(facts),
{
    let accepted = acknowledge_wal_action_body!(facts);
    proof {
        reveal(production_acknowledge_wal_action_relation);
    }
    accepted
}

/// Exact executable successful-validation effect checker used by production.
pub fn verified_validation_completed_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_validation_completed_action_relation(facts),
{
    let accepted = validation_completed_action_body!(facts, verified_effect_count_gate);
    proof {
        reveal(production_validation_completed_action_relation);
    }
    accepted
}

/// Exact executable body-progress action checker used by production.
pub fn verified_body_progress_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_body_progress_action_relation(facts),
{
    let accepted = body_progress_action_body!(facts, verified_validation_completed_action_gate);
    proof {
        reveal(production_body_progress_action_relation);
    }
    accepted
}

/// Exact executable volatile-protocol action checker used by production.
pub fn verified_volatile_protocol_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_volatile_protocol_action_relation(facts),
{
    let accepted = volatile_protocol_action_body!(facts);
    proof {
        reveal(production_volatile_protocol_action_relation);
    }
    accepted
}

/// Exact executable application-completion action checker used by production.
pub fn verified_complete_application_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_complete_application_action_relation(facts),
{
    let accepted = complete_application_action_body!(facts);
    proof {
        reveal(production_complete_application_action_relation);
    }
    accepted
}

/// Exact executable action-discriminant checker used by production.
pub fn verified_action_kind_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_action_kind_relation(facts),
{
    let accepted = action_kind_relation_body!(
        facts,
        verified_stutter_action_gate,
        verified_begin_wal_action_gate,
        verified_acknowledge_wal_action_gate,
        verified_body_progress_action_gate,
        verified_volatile_protocol_action_gate,
        verified_complete_application_action_gate,
    );
    proof {
        reveal(production_action_kind_relation);
    }
    accepted
}

/// Exact executable action/WAL/signature-class checker used by production.
pub fn verified_named_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_named_action_relation(facts),
{
    let accepted = production_action_relation_body!(
        facts,
        verified_signed_message_class_gate,
        verified_action_kind_gate,
    );
    proof {
        reveal(production_named_action_relation);
    }
    accepted
}

/// Exact executable per-slot authorization checker used by production.
pub fn verified_effect_slots_gate(
    trace: ProductionEffectTraceProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_slots_authorized(trace),
{
    effect_slots_authorized_body!(trace)
}

/// Exact executable effect-discriminant counter used by production.
pub fn verified_effect_count_gate(
    trace: ProductionEffectTraceProjection,
    kind: u8,
) -> (count: u64)
    ensures
        count == production_effect_count(trace, kind),
{
    effect_count_body!(trace, kind)
}

/// Exact executable order constraints used by production.
pub fn verified_effect_order_constraints_gate(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
    persist_count: u64,
    fetch_count: u64,
    store_count: u64,
    validate_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_order_constraints(
            trace,
            event_kind,
            persist_count,
            fetch_count,
            store_count,
            validate_count,
            sign_count,
            apply_count,
            enter_count,
        ),
{
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

/// Exact executable vector-order checker used by production.
pub fn verified_effect_order_gate(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_order_relation(trace, event_kind),
{
    effect_ordering_gate_body!(
        trace,
        event_kind,
        verified_effect_count_gate,
        verified_effect_order_constraints_gate,
    )
}

/// Exact executable combined effect checker used by the production gate.
pub fn verified_effect_trace_gate(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_trace_relation(trace, event_kind),
{
    effect_trace_gate_body!(
        trace,
        event_kind,
        verified_effect_slots_gate,
        verified_effect_order_gate,
    )
}

/// Exact executable branch constraints used by the production gate.
pub fn verified_transition_branch_constraints_gate(
    facts: ProductionTransitionFactsProjection,
    persist_count: u64,
    fetch_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> (accepted: bool)
    ensures
        accepted ==> production_transition_branch_constraints(
            facts,
            persist_count,
            fetch_count,
            sign_count,
            apply_count,
            enter_count,
        ),
{
    transition_branch_constraints_body!(
        facts,
        persist_count,
        fetch_count,
        sign_count,
        apply_count,
        enter_count,
    )
}

/// Exact executable branch checker used by the production gate.
pub fn verified_transition_branch_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_transition_branch_relation(facts),
{
    transition_branch_gate_body!(
        facts,
        verified_effect_count_gate,
        verified_transition_branch_constraints_gate,
    )
}

/// Exact executable decision procedure used by `refinement::accepts` in a
/// normal build.  The body is one shared macro expansion, not a transcription.
pub fn verified_production_transition_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_transition_action_relation(facts),
{
    let accepted = production_transition_gate_body!(
        facts,
        verified_volatile_summary_gate,
        verified_named_action_gate,
        verified_effect_trace_gate,
        verified_transition_branch_gate,
    );
    proof {
        reveal(production_transition_action_relation);
    }
    accepted
}

/// Acceptance refines to the production action relation and exposes the
/// durable invariants and complete per-slot effect authorization on which the
/// reducer's commit depends.
pub proof fn accepted_production_transition_refines_action(
    facts: ProductionTransitionFactsProjection,
)
    requires
        production_transition_action_relation(facts),
    ensures
        facts.before_invariant,
        facts.after_invariant,
        facts.context_unchanged,
        production_volatile_summary_well_formed(
            facts.volatile_before,
            facts.validator_count,
        ),
        production_volatile_summary_well_formed(
            facts.volatile_after,
            facts.validator_count,
        ),
        production_named_action_relation(facts),
        facts.effects.len <= 8,
        facts.effects.len > 0 ==> facts.effects.authorized0,
        facts.effects.len > 1 ==> facts.effects.authorized1,
        facts.effects.len > 2 ==> facts.effects.authorized2,
        facts.effects.len > 3 ==> facts.effects.authorized3,
        facts.effects.len > 4 ==> facts.effects.authorized4,
        facts.effects.len > 5 ==> facts.effects.authorized5,
        facts.effects.len > 6 ==> facts.effects.authorized6,
        facts.effects.len > 7 ==> facts.effects.authorized7,
        effect_count_body!(facts.effects, 1u8) > 0
            ==> effect_count_body!(facts.effects, 5u8) == 0
                && effect_count_body!(facts.effects, 7u8) == 0
                && effect_count_body!(facts.effects, 8u8) == 0,
        effect_count_body!(facts.effects, 5u8) > 0
            ==> match facts.effects.len {
                1 => facts.effects.kind0 == 5,
                2 => facts.effects.kind1 == 5,
                3 => facts.effects.kind2 == 5,
                4 => facts.effects.kind3 == 5,
                5 => facts.effects.kind4 == 5,
                6 => facts.effects.kind5 == 5,
                7 => facts.effects.kind6 == 5,
                8 => facts.effects.kind7 == 5,
                _ => false,
            },
        effect_count_body!(facts.effects, 8u8) > 0
            ==> facts.acknowledge_persist_exact
                && facts.acknowledgement_continuation == 2,
{
    reveal(production_transition_action_relation);
}

} // verus!
