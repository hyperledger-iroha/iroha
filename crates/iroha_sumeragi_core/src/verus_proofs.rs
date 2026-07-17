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
//! `VERIFICATION.md` for pinned execution evidence and the residual
//! collection-extraction, adapter-contract, WAL-byte, cross-tool, and liveness
//! boundaries that remain before a complete production-correctness claim.

use vstd::prelude::*;

// These expressions are instantiated both as specifications and as executable
// Verus functions.  The PrepareIntent and TimeoutIntent WAL guards below are
// derived directly from primitive vote and frozen-context fields.  The
// remaining projected WAL certificate/proposal predicates are called out
// explicitly in `VERIFICATION.md` until they are migrated to the same
// representation.
// Cargo Verus consumes these macros inside `verus!`; the trailing ordinary
// Rust metadata pass sees the ghost module erased and would otherwise report
// them as unused.
#[allow(unused_macros)]
macro_rules! same_certificate_body {
    ($left:expr, $right:expr) => {{
        $left.present == $right.present
            && (!$left.present
                || ($left.context == $right.context
                    && $left.height == $right.height
                    && $left.prepare == $right.prepare
                    && $left.view == $right.view
                    && $left.subject == $right.subject))
    }};
}

#[allow(unused_macros)]
macro_rules! same_certificate_evidence_body {
    ($left:expr, $right:expr) => {{
        same_certificate_body!($left, $right)
            && (!$left.present || $left.evidence == $right.evidence)
    }};
}

// Compose the source-linked EnterView gate and the exact effect-count checks
// once for both the closed specification and its branch-factored executable
// Verus instances.
#[allow(unused_macros)]
macro_rules! production_enter_view_exact_body {
    ($projection:expr) => {{
        enter_view_projection_gate_body!($projection.enter_view)
            && $projection.enter_view.enter_count == effect_count_body!($projection.effects, 8u8)
            && $projection.enter_view.fetch_count == effect_count_body!($projection.effects, 2u8)
    }};
}

verus! {

/// Largest value representable by every production height/view/generation/WAL
/// counter (`u64`).
pub open spec fn machine_u64_max() -> int {
    18_446_744_073_709_551_615
}

// ---------------------------------------------------------------------------
// Production timer/FIFO scheduling kernel
// ---------------------------------------------------------------------------

/// Fixed-width projection of one production scheduling decision.
pub struct ScheduleDecisionProjection {
    /// `1 = Timeout`, `2 = PeriodicTimer`, `3 = Fifo`, `4 = Idle`.
    pub work: u8,
    /// Whether an admitted FIFO command is owed the next non-timeout slot.
    pub fifo_owed: bool,
}

/// Exact branch relation instantiated by `ScheduleState::select` in
/// production. The macro body is owned by the source-linked scheduler module,
/// so the executable runtime and this proof cannot drift independently.
pub open spec fn schedule_decision(
    fifo_owed: bool,
    timeout_due: bool,
    periodic_timer_due: bool,
    fifo_ready: bool,
) -> ScheduleDecisionProjection {
    schedule_select_body!(
        fifo_owed,
        timeout_due,
        periodic_timer_due,
        fifo_ready,
        ScheduleDecisionProjection { work: 1, fifo_owed: fifo_ready },
        ScheduleDecisionProjection { work: 2, fifo_owed: fifo_ready },
        ScheduleDecisionProjection { work: 3, fifo_owed: false },
        ScheduleDecisionProjection { work: 4, fifo_owed: false },
    )
}

/// Executable Verus instance of the exact production arbitration branches.
pub fn verified_schedule_decision(
    fifo_owed: bool,
    timeout_due: bool,
    periodic_timer_due: bool,
    fifo_ready: bool,
) -> (decision: ScheduleDecisionProjection)
    ensures
        decision == schedule_decision(
            fifo_owed,
            timeout_due,
            periodic_timer_due,
            fifo_ready,
        ),
{
    let decision = schedule_select_body!(
        fifo_owed,
        timeout_due,
        periodic_timer_due,
        fifo_ready,
        ScheduleDecisionProjection { work: 1, fifo_owed: fifo_ready },
        ScheduleDecisionProjection { work: 2, fifo_owed: fifo_ready },
        ScheduleDecisionProjection { work: 3, fifo_owed: false },
        ScheduleDecisionProjection { work: 4, fifo_owed: false },
    );
    proof {
        reveal(schedule_decision);
    }
    decision
}

/// The absolute timeout always preempts periodic work and FIFO debt.
pub proof fn schedule_timeout_has_absolute_priority(
    fifo_owed: bool,
    periodic_timer_due: bool,
    fifo_ready: bool,
)
    ensures
        schedule_decision(
            fifo_owed,
            true,
            periodic_timer_due,
            fifo_ready,
        ).work == 1,
        schedule_decision(
            fifo_owed,
            true,
            periodic_timer_due,
            fifo_ready,
        ).fifo_owed == fifo_ready,
{
    reveal(schedule_decision);
}

/// Once periodic service incurs FIFO debt, the next non-timeout slot drains
/// the FIFO even when the periodic timer remains due.
pub proof fn schedule_fifo_debt_prevents_periodic_starvation(
    periodic_timer_due: bool,
)
    ensures
        schedule_decision(true, false, periodic_timer_due, true).work == 3,
        !schedule_decision(true, false, periodic_timer_due, true).fifo_owed,
{
    reveal(schedule_decision);
}

/// A periodic tick which precedes ready FIFO work records the exact debt used
/// by the previous theorem, giving an admitted command a two-invocation rank.
pub proof fn schedule_periodic_delay_is_bounded()
    ensures
        schedule_decision(false, false, true, true).work == 2,
        schedule_decision(false, false, true, true).fifo_owed,
        schedule_decision(
            schedule_decision(false, false, true, true).fifo_owed,
            false,
            true,
            true,
        ).work == 3,
{
    reveal(schedule_decision);
}



// ---------------------------------------------------------------------------
// Common certificate and quorum facts
// ---------------------------------------------------------------------------

/// Safety projection of a quorum certificate.
pub struct CertificateProjection {
    /// Whether a certificate is present.
    pub present: bool,
    /// Frozen height-context identity carried by the certificate.
    pub context: int,
    /// Frozen block height carried by the certificate.
    pub height: int,
    /// Prepare when true and Commit when false.
    pub prepare: bool,
    /// Certificate view at the frozen height.
    pub view: int,
    /// Certified subject identity.
    pub subject: int,
    /// Number of canonical distinct voting-validator signers.
    pub signer_count: int,
    /// Voting power represented by those signers.
    pub signer_power: int,
    /// Identity of the complete certificate evidence (signer set and
    /// aggregate/signature bytes), distinct from its stable decision
    /// reference.
    pub evidence: int,
}

/// The absent certificate value.  Its remaining fields are canonicalized.
pub open spec fn absent_certificate() -> CertificateProjection {
    CertificateProjection {
        present: false,
        context: 0,
        height: 0,
        prepare: true,
        view: 0,
        subject: 0,
        signer_count: 0,
        signer_power: 0,
        evidence: 0,
    }
}

/// Exact equality of the safety-relevant certificate reference.
pub open spec fn same_certificate(
    left: CertificateProjection,
    right: CertificateProjection,
) -> bool {
    same_certificate_body!(left, right)
}

/// Equality of the complete carried certificate, including its signer and
/// signature evidence identity.  Production timeout intents compare the full
/// `QuorumCertificate`, not only its stable semantic reference.
pub open spec fn same_certificate_evidence(
    left: CertificateProjection,
    right: CertificateProjection,
) -> bool {
    same_certificate_evidence_body!(left, right)
}

/// Canonical fixed-width identity stored for one timeout intent's optional
/// full high QC.  Absence has no hidden evidence bytes.
pub open spec fn certificate_evidence_identity(certificate: CertificateProjection) -> int {
    if certificate.present { certificate.evidence } else { 0 }
}

/// A well-formed projected PrepareQC.
pub open spec fn valid_prepare(certificate: CertificateProjection) -> bool {
    certificate.present
        && certificate.prepare
        && 0 <= certificate.height <= machine_u64_max()
        && 0 <= certificate.view <= machine_u64_max()
}

/// A well-formed projected CommitQC.
pub open spec fn valid_commit(certificate: CertificateProjection) -> bool {
    certificate.present
        && !certificate.prepare
        && 0 <= certificate.height <= machine_u64_max()
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

/// Production Decision replay keeps the first full certificate evidence for
/// a semantic decision and accepts later certificates only as equivalent
/// witnesses for the same reference.
pub open spec fn decision_after_update(
    current: CertificateProjection,
    incoming: CertificateProjection,
) -> CertificateProjection {
    if current.present { current } else { incoming }
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
// Exact physical WAL lifecycle projection
// ---------------------------------------------------------------------------

/// Fixed-width projection of the canonical WAL file header.
pub struct WalHeaderProjection {
    /// Whether all canonical header bytes are present.
    pub complete: bool,
    /// Exact file magic comparison.
    pub magic_matches: bool,
    /// Exact layout-version comparison.
    pub format_matches: bool,
    /// Persisted protocol identity.
    pub protocol: int,
    /// Persisted chain identity.
    pub chain: int,
    /// Persisted local consensus-key identity.
    pub consensus_key: int,
    /// Recomputed checksum equals the stored checksum.
    pub checksum_matches: bool,
}

/// Header identity expected by the running validator.
pub struct WalExpectedIdentityProjection {
    /// Configured consensus protocol version.
    pub protocol: int,
    /// Configured chain hash.
    pub chain: int,
    /// Configured local consensus-key hash.
    pub consensus_key: int,
}

/// The executable parser accepts a header only after every structural,
/// identity, and checksum comparison succeeds.
pub open spec fn wal_header_accepted(
    header: WalHeaderProjection,
    expected: WalExpectedIdentityProjection,
) -> bool {
    wal_header_accepted_body!(
        header.complete,
        header.magic_matches,
        header.format_matches,
        header.protocol,
        expected.protocol,
        header.chain,
        expected.chain,
        header.consensus_key,
        expected.consensus_key,
        header.checksum_matches,
    )
}

/// Accepted header bytes are bound to the exact protocol, chain, and local
/// consensus key; a mismatch cannot be interpreted as an empty WAL.
pub proof fn accepted_wal_header_has_exact_identity(
    header: WalHeaderProjection,
    expected: WalExpectedIdentityProjection,
)
    requires
        wal_header_accepted(header, expected),
    ensures
        header.complete,
        header.magic_matches,
        header.format_matches,
        header.protocol == expected.protocol,
        header.chain == expected.chain,
        header.consensus_key == expected.consensus_key,
        header.checksum_matches,
{
}

/// A malformed header or any protocol/chain/key/checksum mismatch cannot be
/// accepted as an empty log.
pub proof fn invalid_or_foreign_wal_header_is_rejected(
    header: WalHeaderProjection,
    expected: WalExpectedIdentityProjection,
)
    requires
        !header.complete
            || !header.magic_matches
            || !header.format_matches
            || header.protocol != expected.protocol
            || header.chain != expected.chain
            || header.consensus_key != expected.consensus_key
            || !header.checksum_matches,
    ensures
        !wal_header_accepted(header, expected),
{
}

/// Verified complete-prefix state used by physical WAL recovery.
pub struct PhysicalWalStateProjection {
    /// Required physical sequence of the next complete frame.
    pub next_sequence: int,
    /// Hash of the preceding complete frame, or zero initially.
    pub last_hash: int,
    /// Number of complete records exposed to decoded replay.
    pub recovered_records: int,
    /// Whether recovery has failed closed.
    pub failed_closed: bool,
}

/// One observed frame boundary in canonical file order.
pub struct PhysicalWalFrameProjection {
    /// Whether the complete header, payload, and checksum are present.
    pub complete: bool,
    /// Whether this observation consumes the physical end of file.
    pub final_frame: bool,
    /// Physical frame sequence.
    pub sequence: int,
    /// Declared payload length.
    pub payload_len: int,
    /// Stored previous-frame hash.
    pub previous_hash: int,
    /// Stored checksum/hash of this frame.
    pub stored_hash: int,
    /// Hash recomputed over canonical frame bytes.
    pub calculated_hash: int,
    /// Whether the append call returned a durability acknowledgement.
    pub acknowledged: bool,
}

/// Fixed proof value of the production 16 MiB frame-payload limit.
pub open spec fn wal_max_record_bytes() -> int {
    16_777_216
}

/// A complete frame extends the verified prefix exactly when every production
/// sequence, length, hash-chain, and checksum check succeeds.
pub open spec fn complete_physical_frame_valid(
    before: PhysicalWalStateProjection,
    frame: PhysicalWalFrameProjection,
) -> bool {
    0 <= before.next_sequence
        && 0 <= frame.payload_len
        && wal_complete_frame_valid_body!(
            before.failed_closed,
            frame.complete,
            before.next_sequence,
            machine_u64_max(),
            frame.sequence,
            frame.payload_len,
            wal_max_record_bytes(),
            frame.previous_hash,
            before.last_hash,
            frame.stored_hash,
            frame.calculated_hash,
        )
}

/// Physical recovery has exactly three outcomes at one observed boundary.
pub enum PhysicalWalRecoveryPath {
    /// One complete valid frame extends the recovered prefix.
    Complete,
    /// An incomplete final frame is discarded as unacknowledged.
    IncompleteFinal,
    /// Complete corruption, or a non-final incomplete frame, fails closed.
    Reject,
}

/// One executable physical-recovery step.
pub open spec fn physical_wal_recovery_step(
    before: PhysicalWalStateProjection,
    frame: PhysicalWalFrameProjection,
    path: PhysicalWalRecoveryPath,
    after: PhysicalWalStateProjection,
) -> bool {
    match path {
        PhysicalWalRecoveryPath::Complete => {
            complete_physical_frame_valid(before, frame)
                && after.next_sequence == before.next_sequence + 1
                && after.last_hash == frame.stored_hash
                && after.recovered_records == before.recovered_records + 1
                && !after.failed_closed
        }
        PhysicalWalRecoveryPath::IncompleteFinal => {
            !before.failed_closed
                && !frame.complete
                && frame.final_frame
                && !frame.acknowledged
                && after.next_sequence == before.next_sequence
                && after.last_hash == before.last_hash
                && after.recovered_records == before.recovered_records
                && !after.failed_closed
        }
        PhysicalWalRecoveryPath::Reject => {
            !before.failed_closed
                && (if frame.complete {
                    !complete_physical_frame_valid(before, frame)
                } else {
                    !frame.final_frame
                })
                && after.next_sequence == before.next_sequence
                && after.last_hash == before.last_hash
                && after.recovered_records == before.recovered_records
                && after.failed_closed
        }
    }
}

/// A complete accepted frame advances one sequence and one hash-chain link.
pub proof fn complete_physical_frame_extends_verified_prefix(
    before: PhysicalWalStateProjection,
    frame: PhysicalWalFrameProjection,
    after: PhysicalWalStateProjection,
)
    requires
        physical_wal_recovery_step(
            before,
            frame,
            PhysicalWalRecoveryPath::Complete,
            after,
        ),
    ensures
        after.next_sequence == before.next_sequence + 1,
        after.last_hash == frame.stored_hash,
        after.recovered_records == before.recovered_records + 1,
        !after.failed_closed,
{
}

/// A complete frame may have reached disk before `sync_data` returned. Replay
/// accepts it only through the same full checksum/hash-chain corridor and
/// installs it atomically, which is conservative for safety.
pub proof fn complete_unacknowledged_frame_replays_atomically(
    before: PhysicalWalStateProjection,
    frame: PhysicalWalFrameProjection,
    after: PhysicalWalStateProjection,
)
    requires
        !frame.acknowledged,
        physical_wal_recovery_step(
            before,
            frame,
            PhysicalWalRecoveryPath::Complete,
            after,
        ),
    ensures
        complete_physical_frame_valid(before, frame),
        after.next_sequence == before.next_sequence + 1,
        after.last_hash == frame.stored_hash,
        after.recovered_records == before.recovered_records + 1,
        !after.failed_closed,
{
}

/// An incomplete final frame is never acknowledged or exposed to decoded
/// replay and leaves the complete-prefix sequence/hash unchanged.
pub proof fn incomplete_final_frame_is_unacknowledged_stutter(
    before: PhysicalWalStateProjection,
    frame: PhysicalWalFrameProjection,
    after: PhysicalWalStateProjection,
)
    requires
        physical_wal_recovery_step(
            before,
            frame,
            PhysicalWalRecoveryPath::IncompleteFinal,
            after,
        ),
    ensures
        !frame.complete,
        frame.final_frame,
        !frame.acknowledged,
        after.next_sequence == before.next_sequence,
        after.last_hash == before.last_hash,
        after.recovered_records == before.recovered_records,
        !after.failed_closed,
{
}

/// A complete sequence, length, previous-hash, or checksum failure cannot be
/// truncated as an unacknowledged tail; it preserves the prefix and closes.
pub proof fn corrupt_complete_frame_fails_closed(
    before: PhysicalWalStateProjection,
    frame: PhysicalWalFrameProjection,
    after: PhysicalWalStateProjection,
)
    requires
        frame.complete,
        physical_wal_recovery_step(
            before,
            frame,
            PhysicalWalRecoveryPath::Reject,
            after,
        ),
    ensures
        !complete_physical_frame_valid(before, frame),
        after.next_sequence == before.next_sequence,
        after.last_hash == before.last_hash,
        after.recovered_records == before.recovered_records,
        after.failed_closed,
{
}

/// Ordered adapter completions for one physical append attempt.
pub struct WalAppendLifecycleProjection {
    /// `write_all` returned success.
    pub write_complete: bool,
    /// `flush` returned success.
    pub flush_complete: bool,
    /// `sync_data` returned success.
    pub sync_complete: bool,
    /// Monotonic operation index of `write_all`, or zero when absent.
    pub write_order: int,
    /// Monotonic operation index of `flush`, or zero when absent.
    pub flush_order: int,
    /// Monotonic operation index of `sync_data`, or zero when absent.
    pub sync_order: int,
    /// Whether the append state advanced to the successor hash/sequence.
    pub state_advanced: bool,
    /// Whether a durable append receipt was returned to the reducer adapter.
    pub receipt_minted: bool,
    /// Whether an I/O failure poisoned this append instance.
    pub failed_closed: bool,
}

/// Exact append lifecycle implemented by `WalAppendState::append`.
pub open spec fn wal_append_lifecycle_valid(step: WalAppendLifecycleProjection) -> bool {
    (!step.write_complete || 0 < step.write_order)
        && (!step.flush_complete
            || (step.write_complete
                && step.write_order < step.flush_order))
        && (!step.sync_complete
            || (step.flush_complete
                && step.flush_order < step.sync_order))
        && step.receipt_minted
            == wal_append_acknowledged_body!(
                step.write_complete,
                step.flush_complete,
                step.sync_complete,
            )
        && step.state_advanced == step.receipt_minted
        && step.failed_closed == (!step.write_complete
            || (step.write_complete && !step.receipt_minted))
}

/// A returned append receipt implies the complete ordered
/// write/flush/sync-data corridor and atomic hash-chain-state advance.
pub proof fn append_receipt_requires_ordered_durability(
    step: WalAppendLifecycleProjection,
)
    requires
        wal_append_lifecycle_valid(step),
        step.receipt_minted,
    ensures
        step.write_complete,
        step.flush_complete,
        step.sync_complete,
        0 < step.write_order < step.flush_order < step.sync_order,
        step.state_advanced,
        !step.failed_closed,
{
}

/// Any incomplete append corridor mints no receipt and does not advance the
/// in-memory sequence/hash state.
pub proof fn incomplete_append_corridor_has_no_acknowledgement(
    step: WalAppendLifecycleProjection,
)
    requires
        wal_append_lifecycle_valid(step),
        !step.write_complete || !step.flush_complete || !step.sync_complete,
    ensures
        !step.receipt_minted,
        !step.state_advanced,
        step.failed_closed,
{
}

/// Fixed-width projection of the typed WAL retirement corridor.
pub struct WalRetirementProjection {
    /// A `FinalizedHeight` was produced by consuming the reducer.
    pub height_closed: bool,
    /// The exact block bytes are durable in Kura.
    pub block_durable: bool,
    /// The exact CommitQC/finality artifact is durable in Kura.
    pub certificate_durable: bool,
    /// Durable reducer decision context.
    pub decision_context: int,
    /// Durable reducer decision height.
    pub decision_height: int,
    /// Durable reducer decision subject.
    pub decision_subject: int,
    /// CommitQC context carried by the reducer decision.
    pub decision_certificate_context: int,
    /// CommitQC height carried by the reducer decision.
    pub decision_certificate_height: int,
    /// CommitQC view carried by the reducer decision.
    pub decision_certificate_view: int,
    /// Commit phase discriminator carried by the reducer decision.
    pub decision_certificate_phase: int,
    /// CommitQC subject carried by the reducer decision.
    pub decision_certificate_subject: int,
    /// Trusted Kura receipt context.
    pub receipt_context: int,
    /// Trusted Kura receipt height.
    pub receipt_height: int,
    /// Trusted Kura receipt subject.
    pub receipt_subject: int,
    /// Trusted Kura certificate context.
    pub receipt_certificate_context: int,
    /// Trusted Kura certificate height.
    pub receipt_certificate_height: int,
    /// Trusted Kura certificate view.
    pub receipt_certificate_view: int,
    /// Trusted Kura certificate phase.
    pub receipt_certificate_phase: int,
    /// Trusted Kura certificate subject.
    pub receipt_certificate_subject: int,
}

/// Fixed proof discriminator for the production Commit phase.
pub open spec fn wal_commit_phase_code() -> int {
    0
}

/// Exact production rule for minting WAL retirement authority.
pub open spec fn wal_retirement_authorized(step: WalRetirementProjection) -> bool {
    wal_retirement_authorized_body!(
        step.height_closed,
        step.block_durable,
        step.certificate_durable,
        step.decision_context,
        step.decision_height,
        step.decision_subject,
        step.decision_certificate_context,
        step.decision_certificate_height,
        step.decision_certificate_view,
        step.decision_certificate_phase,
        wal_commit_phase_code(),
        step.decision_certificate_subject,
        step.receipt_context,
        step.receipt_height,
        step.receipt_subject,
        step.receipt_certificate_context,
        step.receipt_certificate_height,
        step.receipt_certificate_view,
        step.receipt_certificate_phase,
        step.receipt_certificate_subject,
    )
}

/// Retirement authority proves both Kura durability facts and exact identity
/// equality with the reducer's durable CommitQC decision.
pub proof fn wal_retirement_requires_exact_durable_kura_receipt(
    step: WalRetirementProjection,
)
    requires
        wal_retirement_authorized(step),
    ensures
        step.height_closed,
        step.block_durable,
        step.certificate_durable,
        step.decision_context == step.receipt_context,
        step.decision_height == step.receipt_height,
        step.decision_subject == step.receipt_subject,
        step.decision_certificate_context == step.receipt_certificate_context,
        step.decision_certificate_height == step.receipt_certificate_height,
        step.decision_certificate_view == step.receipt_certificate_view,
        step.decision_certificate_phase == wal_commit_phase_code(),
        step.decision_certificate_phase == step.receipt_certificate_phase,
        step.decision_certificate_subject == step.receipt_certificate_subject,
{
}

/// Missing block durability, missing certificate durability, or an unclosed
/// reducer height cannot authorize pruning regardless of matching identities.
pub proof fn incomplete_kura_durability_cannot_authorize_wal_retirement(
    step: WalRetirementProjection,
)
    requires
        !step.height_closed || !step.block_durable || !step.certificate_durable,
    ensures
        !wal_retirement_authorized(step),
{
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
        /// Frozen height-context identity carried by the vote.
        context: int,
        /// Vote height.
        height: int,
        /// Vote view.
        view: int,
        /// Vote subject.
        subject: int,
        /// Frozen-roster index of the vote signer.
        signer: int,
        /// Prepare when true and Commit when false.
        is_prepare: bool,
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
        /// Frozen height-context identity carried by the timeout vote.
        context: int,
        /// Timed-out block height.
        height: int,
        /// Timed-out view.
        view: int,
        /// Frozen-roster index of the timeout signer.
        signer: int,
        /// Complete highest PrepareQC carried by the timeout vote, including
        /// its evidence identity, or the canonical absent value.
        highest_prepare: CertificateProjection,
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

/// Safety-relevant durable fields plus frozen inputs supplied to WAL replay.
pub struct WalStateProjection {
    /// Frozen context identity.
    pub context: int,
    /// Frozen height.
    pub height: int,
    /// Number of validators in the frozen ordered voting roster.
    pub validator_count: int,
    /// Frozen-roster index of the local validator, or `-1` for an observer.
    pub local_validator: int,
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
    /// Unique local timeout high-QC evidence identity per view.
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
        && left.validator_count == right.validator_count
        && left.local_validator == right.local_validator
        && left.view == right.view
        && left.last_id == right.last_id
        && left.proposal_intents =~= right.proposal_intents
        && left.prepare_intents =~= right.prepare_intents
        && left.commit_intents =~= right.commit_intents
        && left.timeout_intents =~= right.timeout_intents
        && same_certificate_evidence(left.highest_prepare, right.highest_prepare)
        && same_certificate_evidence(left.locked, right.locked)
        && left.last_timeout_view == right.last_timeout_view
        && same_certificate_evidence(left.decision, right.decision)
}

/// The invariant reconstructed from every accepted complete WAL prefix.
pub open spec fn wal_invariant(state: WalStateProjection) -> bool {
    0 <= state.height <= machine_u64_max()
        && 0 <= state.view <= machine_u64_max()
        && 0 < state.validator_count
        && -1 <= state.local_validator < state.validator_count
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
                context,
                height,
                view,
                subject,
                signer,
                is_prepare,
            } => {
                context == before.context
                    && height == before.height
                    && is_prepare
                    && signer == before.local_validator
                    && 0 <= signer
                    && signer < before.validator_count
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
                context,
                height,
                view,
                signer,
                highest_prepare,
            } => {
                context == before.context
                    && height == before.height
                    && view == before.view
                    && signer == before.local_validator
                    && 0 <= signer
                    && signer < before.validator_count
                    && same_certificate_evidence(highest_prepare, before.highest_prepare)
                    && unique_insert_allowed(
                        before.timeout_intents,
                        view,
                        certificate_evidence_identity(highest_prepare),
                    )
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
        && after.validator_count == before.validator_count
        && after.local_validator == before.local_validator
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
                    && same_certificate_evidence(
                        after.highest_prepare,
                        before.highest_prepare,
                    )
                    && same_certificate_evidence(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate_evidence(after.decision, before.decision)
            }
            WalRecordProjection::PrepareIntent { view, subject, .. } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents.insert(view, subject)
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate_evidence(
                        after.highest_prepare,
                        before.highest_prepare,
                    )
                    && same_certificate_evidence(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate_evidence(after.decision, before.decision)
            }
            WalRecordProjection::ObservePrepare { certificate, .. } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate_evidence(
                        after.highest_prepare,
                        highest_after_update(before.highest_prepare, certificate),
                    )
                    && same_certificate_evidence(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate_evidence(after.decision, before.decision)
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
                    && same_certificate_evidence(
                        after.highest_prepare,
                        highest_after_update(before.highest_prepare, prepare),
                    )
                    && same_certificate_evidence(after.locked, prepare)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate_evidence(after.decision, before.decision)
            }
            WalRecordProjection::TimeoutIntent {
                view,
                highest_prepare,
                ..
            } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents
                        =~= before.timeout_intents.insert(
                            view,
                            certificate_evidence_identity(highest_prepare),
                        )
                    && same_certificate_evidence(
                        after.highest_prepare,
                        before.highest_prepare,
                    )
                    && same_certificate_evidence(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate_evidence(after.decision, before.decision)
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
                    && same_certificate_evidence(
                        after.highest_prepare,
                        if selected_prepare.present {
                            highest_after_update(before.highest_prepare, selected_prepare)
                        } else {
                            before.highest_prepare
                        },
                    )
                    && same_certificate_evidence(
                        after.locked,
                        lock_after_timeout(before.locked, selected_prepare),
                    )
                    && after.last_timeout_view == tc_view
                    && same_certificate_evidence(after.decision, before.decision)
            }
            WalRecordProjection::Decision { certificate, .. } => {
                after.view == before.view
                    && after.proposal_intents =~= before.proposal_intents
                    && after.prepare_intents =~= before.prepare_intents
                    && after.commit_intents =~= before.commit_intents
                    && after.timeout_intents =~= before.timeout_intents
                    && same_certificate_evidence(
                        after.highest_prepare,
                        before.highest_prepare,
                    )
                    && same_certificate_evidence(after.locked, before.locked)
                    && after.last_timeout_view == before.last_timeout_view
                    && same_certificate_evidence(
                        after.decision,
                        decision_after_update(before.decision, certificate),
                    )
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
            highest_prepare,
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
                        certificate_evidence_identity(highest_prepare),
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

/// PrepareIntent admissibility is computed from vote primitives and frozen
/// replay inputs; no caller-supplied validity bit can authorize the record.
pub proof fn prepare_intent_guard_is_derived_from_vote_and_frozen_context(
    before: WalStateProjection,
    frame: WalFrameProjection,
)
    requires
        wal_frame_admissible(before, frame),
    ensures
        match frame.record {
            WalRecordProjection::PrepareIntent {
                context,
                height,
                signer,
                is_prepare,
                ..
            } => {
                context == before.context
                    && height == before.height
                    && is_prepare
                    && signer == before.local_validator
                    && 0 <= signer
                    && signer < before.validator_count
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::PrepareIntent { .. } => {},
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

/// TimeoutIntent admissibility is derived from timeout-vote primitives and the
/// frozen replay context; no caller-supplied validity or high-QC-match bit can
/// authorize the record.
pub proof fn timeout_intent_guard_is_derived_from_vote_and_frozen_context(
    before: WalStateProjection,
    frame: WalFrameProjection,
)
    requires
        wal_frame_admissible(before, frame),
    ensures
        match frame.record {
            WalRecordProjection::TimeoutIntent {
                context,
                height,
                view,
                signer,
                highest_prepare,
            } => {
                context == before.context
                    && height == before.height
                    && view == before.view
                    && signer == before.local_validator
                    && 0 <= signer
                    && signer < before.validator_count
                    && same_certificate_evidence(highest_prepare, before.highest_prepare)
            }
            _ => true,
        },
{
    match frame.record {
        WalRecordProjection::TimeoutIntent { .. } => {},
        _ => {},
    }
}

/// TimeoutIntent durably closes exactly the current view with the expected
/// full high PrepareQC evidence identity.
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
                highest_prepare,
                ..
            } => {
                view == before.view
                    && after.timeout_intents.dom().contains(view)
                    && after.timeout_intents[view]
                        == certificate_evidence_identity(highest_prepare)
                    && same_certificate_evidence(
                        highest_prepare,
                        before.highest_prepare,
                    )
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
    /// `Event::ResumeAfterReplay`.
    ResumeAfterReplay,
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
    /// Whether the sole recovery-resumption transition was consumed.
    pub replay_resumed: bool,
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
    /// `on_resume_after_replay` consumed the recovery-pending transition.
    ResumeAfterReplay,
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

/// The `NoDurableChange` branch enforces the recovery and pending-write fences,
/// may retransmit Apply only on the timer path, and may request the next
/// already-authorized signature only after Signed. This safety projection
/// intentionally erases Fetch/Store/Validate; their exact Decision identity,
/// stage, mutual exclusion, and ordering are checked by the complete production
/// effect trace below.
pub open spec fn no_change_effects_match_input(
    before: ReducerProjection,
    input: ReducerInputProjection,
    after: ReducerProjection,
    effects: EffectProjection,
) -> bool {
    if !input_tag_matches(before, input)
        || (before.pending && !is_persistence_completion(input.event))
        || (!before.replay_resumed && input.event != EventProjection::ResumeAfterReplay)
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
        && before.replay_resumed == after.replay_resumed
}

/// Exact safety projection of the one replay-resumption event.  The complete
/// production effect vector is checked separately by the executable gate;
/// this projection retains the persistence/sign/application fences.
pub open spec fn replay_resume_transition(
    before: ReducerProjection,
    input: ReducerInputProjection,
    effects: EffectProjection,
    after: ReducerProjection,
) -> bool {
    input_tag_matches(before, input)
        && input.event == EventProjection::ResumeAfterReplay
        && !before.replay_resumed
        && after.replay_resumed
        && same_wal_state(before.durable, after.durable)
        && before.generation == after.generation
        && !before.pending
        && !after.pending
        && after.durable_bodies =~= before.durable_bodies
        && after.validated_bodies =~= before.validated_bodies
        && after.application_ready =~= before.application_ready
        && before.applied == after.applied
        && (!before.applied || before.applied_subject == after.applied_subject)
        && non_persist_effects_safe(after, effects)
        && !effects.apply
        && !effects.enter_view
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
                && before.replay_resumed == after.replay_resumed
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
                && before.replay_resumed == after.replay_resumed
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
                && before.replay_resumed == after.replay_resumed
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
                && before.replay_resumed == after.replay_resumed
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
        ReducerPathProjection::ResumeAfterReplay => {
            replay_resume_transition(before, input, effects, after)
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
        ReducerPathProjection::ResumeAfterReplay => {},
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
        | ReducerPathProjection::CompleteApplication
        | ReducerPathProjection::ResumeAfterReplay => {},
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
        | ReducerPathProjection::CompleteApplication
        | ReducerPathProjection::ResumeAfterReplay => {},
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
        | ReducerPathProjection::CompleteApplication
        | ReducerPathProjection::ResumeAfterReplay => {},
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
        && !after.replay_resumed
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
        !after.replay_resumed,
{
}

// ---------------------------------------------------------------------------
// Exact executable production commit gate
// ---------------------------------------------------------------------------

/// Verus-side shape of one fixed-width effect capability key.
#[derive(Copy, Clone)]
pub struct ProductionTagProjection {
    pub height: u64,
    pub view: u64,
    pub generation: u64,
}


/// Verus-side complete safety identity of one optional quorum certificate.
///
/// Signature bytes remain outside this fixed-width projection. The executable
/// reducer separately compares the complete effect-carried certificate object
/// with the durable certificate before committing the transition.
#[derive(Copy, Clone)]
pub struct CertificateIdentityProjection {
    pub present: bool,
    pub context_id: int,
    pub height: u64,
    pub view: u64,
    pub phase: u8,
    pub subject: int,
}

/// Verus-side identity of one optional timeout certificate and its selected
/// highest `PrepareQC`.
#[derive(Copy, Clone)]
pub struct TimeoutIdentityProjection {
    pub present: bool,
    pub context_id: int,
    pub height: u64,
    pub view: u64,
    pub highest_prepare: CertificateIdentityProjection,
}

/// Exact persisted-TC `EnterView` macro-step projected by the production
/// reducer.
///
/// This shape deliberately stops at the serialized reducer boundary: it names
/// the selected lock and immediate recovery fetch, but makes no asynchronous
/// ownership, scheduling, or temporal-fairness claim.
#[derive(Copy, Clone)]
pub struct EnterViewProjection {
    pub active: bool,
    pub context_id: int,
    pub before_tag: ProductionTagProjection,
    pub after_tag: ProductionTagProjection,
    pub pending_record_kind: u8,
    pub pending_continuation: u8,
    pub pending_record_timeout: TimeoutIdentityProjection,
    pub pending_continuation_timeout: TimeoutIdentityProjection,
    pub durable_timeout_after: TimeoutIdentityProjection,
    pub effect_timeout: TimeoutIdentityProjection,
    pub local_lock_before: CertificateIdentityProjection,
    pub durable_lock_after: CertificateIdentityProjection,
    pub effect_protected_lock: CertificateIdentityProjection,
    pub following_fetch_lock: CertificateIdentityProjection,
    pub enter_count: u64,
    pub fetch_count: u64,
    pub enter_index: u8,
    pub following_fetch_index: u8,
}

/// Verus-side shape of a concrete requested/granted effect capability.
#[derive(Copy, Clone)]
pub struct ProductionEffectCapabilityKeyProjection {
    pub kind: u8,
    pub tag: ProductionTagProjection,
    pub context_id: int,
    pub height: u64,
    pub view: u64,
    pub phase: u8,
    pub subject: int,
    pub actor: int,
    pub persistence_id: u64,
    pub record_kind: u8,
    pub auxiliary_context_id: int,
    pub auxiliary_height: u64,
    pub auxiliary_view: u64,
    pub auxiliary_phase: u8,
    pub auxiliary_subject: int,
    pub manifest_payload: int,
    pub manifest_chunks: int,
    pub manifest_len: u64,
    pub manifest_count: u64,
}

/// Verus-side shape of one effect vector slot.
#[derive(Copy, Clone)]
pub struct ProductionEffectSlotProjection {
    pub kind: u8,
    pub requested: ProductionEffectCapabilityKeyProjection,
    pub granted: ProductionEffectCapabilityKeyProjection,
}

/// Verus-side shape of the fixed production effect trace.  This mirrors the
/// private `refinement::EffectTrace`; the decision expression below is shared
/// textually with production through `production_transition_gate_body!`.
#[derive(Copy, Clone)]
pub struct ProductionEffectTraceProjection {
    pub len: u8,
    pub slot0: ProductionEffectSlotProjection,
    pub slot1: ProductionEffectSlotProjection,
    pub slot2: ProductionEffectSlotProjection,
    pub slot3: ProductionEffectSlotProjection,
    pub slot4: ProductionEffectSlotProjection,
    pub slot5: ProductionEffectSlotProjection,
    pub slot6: ProductionEffectSlotProjection,
    pub slot7: ProductionEffectSlotProjection,
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

/// Verus-side optional validator identity.
#[derive(Copy, Clone)]
pub struct ProductionValidatorProjection {
    pub present: bool,
    pub id: int,
}

/// Verus-side optional subject identity.
#[derive(Copy, Clone)]
pub struct ProductionSubjectProjection {
    pub present: bool,
    pub subject: int,
}

/// Verus-side concrete invariant violation counters.
#[derive(Copy, Clone)]
pub struct ProductionSafetyProjection {
    pub durable_identity_mismatches: u64,
    pub asynchronous_fence_conflicts: u64,
    pub invalid_highest_prepare: u64,
    pub invalid_lock: u64,
    pub invalid_timeout: u64,
    pub invalid_decision: u64,
    pub invalid_pending_append: u64,
    pub unauthorized_signables: u64,
    pub invalid_application: u64,
}

/// Verus-side safety identity of one pending WAL append.
#[derive(Copy, Clone)]
pub struct ProductionPendingProjection {
    pub record_kind: u8,
    pub continuation: u8,
    pub persistence_id: u64,
    pub context_id: int,
    pub height: u64,
    pub view: u64,
    pub subject: int,
}

/// Verus-side durable-boundary capability key.
#[derive(Copy, Clone)]
pub struct ProductionBoundaryCapabilityKeyProjection {
    pub kind: u8,
    pub record_kind: u8,
    pub continuation: u8,
    pub replay_effect_kind: u8,
    pub persistence_id: u64,
    pub context_id: int,
    pub tag: ProductionTagProjection,
    pub subject: ProductionSubjectProjection,
}

/// Verus-side primitive projection supplied to the exact production kernel.
///
/// Whole reducer and durable-state identities are mathematical values here;
/// production supplies direct references and the shared derivation compares
/// their concrete `Eq` identities before committing the transition.
#[derive(Copy, Clone)]
pub struct ProductionTransitionProjection {
    pub before_state: int,
    pub after_state: int,
    pub durable_before: int,
    pub durable_after: int,
    pub safety_before: ProductionSafetyProjection,
    pub safety_after: ProductionSafetyProjection,
    pub context_before: int,
    pub context_after: int,
    pub local_before: ProductionValidatorProjection,
    pub local_after: ProductionValidatorProjection,
    pub event_tag: ProductionTagProjection,
    pub height_before: u64,
    pub view_before: u64,
    pub generation_before: u64,
    pub generation_after: u64,
    pub pending_state_before: int,
    pub pending_state_after: int,
    pub pending_before: ProductionPendingProjection,
    pub awaiting_before: bool,
    pub replay_before: bool,
    pub application_before: ProductionSubjectProjection,
    pub application_after: ProductionSubjectProjection,
    pub event_kind: u8,
    pub awaiting_message_kind: u8,
    pub validator_count: u64,
    pub volatile_before: ProductionVolatileSummaryProjection,
    pub volatile_after: ProductionVolatileSummaryProjection,
    pub boundary_claimed: ProductionBoundaryCapabilityKeyProjection,
    pub boundary_granted: ProductionBoundaryCapabilityKeyProjection,
    pub enter_view: EnterViewProjection,
    pub effects: ProductionEffectTraceProjection,
}

/// Verus-side shape of the facts extracted by the production reducer before
/// committing a candidate transition.  These booleans are internal derived
/// values; production callers cannot provide them to the kernel.
#[derive(Copy, Clone)]
pub struct ProductionTransitionFactsProjection {
    pub before_invariant: bool,
    pub after_invariant: bool,
    pub context_unchanged: bool,
    pub whole_state_unchanged: bool,
    pub tag_matches: bool,
    pub busy_fence_open: bool,
    pub event_kind: u8,
    pub action_kind: u8,
    pub wal_record_kind: u8,
    pub signed_message_kind: u8,
    pub replay_effect_kind: u8,
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
    pub enter_view_exact: bool,
    pub effects: ProductionEffectTraceProjection,
}

/// Lock selected by the persisted-TC view transition: the incoming highest
/// `PrepareQC` replaces the local lock only when it has a strictly higher view.
pub open spec fn production_enter_view_selected_lock(
    projection: EnterViewProjection,
) -> CertificateIdentityProjection {
    let local = projection.local_lock_before;
    let incoming = projection.pending_record_timeout.highest_prepare;
    if !local.present {
        incoming
    } else if !incoming.present || incoming.view <= local.view {
        local
    } else {
        incoming
    }
}

/// Whether the effect immediately following `EnterView` is the one exact
/// durable-lock recovery fetch.
pub open spec fn production_enter_view_has_exact_following_fetch(
    projection: EnterViewProjection,
) -> bool {
    projection.fetch_count == 1
        && certificate_identity_equal_body!(
            projection.following_fetch_lock,
            projection.durable_lock_after
        )
        && projection.enter_index < 254
        && projection.following_fetch_index == projection.enter_index + 1
}

/// Exact source-linked reducer relation for the persisted-TC `EnterView`
/// macro-step.
pub open spec fn production_enter_view_projection_relation(
    projection: EnterViewProjection,
) -> bool {
    enter_view_projection_gate_body!(projection)
}

/// Exact fact derivation shared with the executable production kernel.
pub closed spec fn production_facts_from_projection(
    projection: ProductionTransitionProjection,
) -> ProductionTransitionFactsProjection {
    transition_facts_from_projection_body!(projection, ProductionTransitionFactsProjection)
}

/// The EnterView component of the source-linked fact constructor, isolated so
/// its certificate-identity cases can be discharged independently of the
/// remaining transition fields.
pub closed spec fn production_enter_view_exact_fact(
    projection: ProductionTransitionProjection,
) -> bool {
    production_enter_view_exact_body!(projection)
}

/// The full source-linked constructor projects the isolated exact EnterView
/// fact. Case splitting keeps this definitional bridge within the normal root
/// verifier budget without weakening either side of the equality.
pub proof fn production_enter_view_fact_projection_is_exact(
    projection: ProductionTransitionProjection,
)
    ensures
        production_facts_from_projection(projection).enter_view_exact
            == production_enter_view_exact_fact(projection),
{
    let enter_view = projection.enter_view;
    let local = enter_view.local_lock_before;
    let incoming = enter_view.pending_record_timeout.highest_prepare;
    if !enter_view.active {
        reveal(production_facts_from_projection);
        reveal(production_enter_view_exact_fact);
    } else if !local.present {
        if !incoming.present {
            reveal(production_facts_from_projection);
            reveal(production_enter_view_exact_fact);
        } else {
            reveal(production_facts_from_projection);
            reveal(production_enter_view_exact_fact);
        }
    } else if !incoming.present {
        reveal(production_facts_from_projection);
        reveal(production_enter_view_exact_fact);
    } else if incoming.view <= local.view {
        reveal(production_facts_from_projection);
        reveal(production_enter_view_exact_fact);
    } else {
        reveal(production_facts_from_projection);
        reveal(production_enter_view_exact_fact);
    }
}

/// Equality of invariant, fence, and action-classification facts.
pub open spec fn production_classification_facts_equal(
    left: ProductionTransitionFactsProjection,
    right: ProductionTransitionFactsProjection,
) -> bool {
    left.before_invariant == right.before_invariant
        && left.after_invariant == right.after_invariant
        && left.context_unchanged == right.context_unchanged
        && left.whole_state_unchanged == right.whole_state_unchanged
        && left.tag_matches == right.tag_matches
        && left.busy_fence_open == right.busy_fence_open
        && left.event_kind == right.event_kind
        && left.action_kind == right.action_kind
        && left.wal_record_kind == right.wal_record_kind
        && left.signed_message_kind == right.signed_message_kind
        && left.replay_effect_kind == right.replay_effect_kind
        && left.validator_count == right.validator_count
}

/// Equality of volatile, durable-delta, capability, and effect facts.
pub open spec fn production_delta_facts_equal(
    left: ProductionTransitionFactsProjection,
    right: ProductionTransitionFactsProjection,
) -> bool {
    left.volatile_before == right.volatile_before
        && left.volatile_after == right.volatile_after
        && left.durable_unchanged == right.durable_unchanged
        && left.pending_unchanged == right.pending_unchanged
        && left.generation_unchanged == right.generation_unchanged
        && left.application_unchanged == right.application_unchanged
        && left.begin_persist_exact == right.begin_persist_exact
        && left.acknowledge_persist_exact == right.acknowledge_persist_exact
        && left.application_transition_exact == right.application_transition_exact
        && left.acknowledgement_continuation == right.acknowledgement_continuation
        && left.effects == right.effects
}

/// Field equality for every production transition fact except the separately
/// factored EnterView certificate relation.
pub open spec fn production_non_enter_view_facts_equal(
    left: ProductionTransitionFactsProjection,
    right: ProductionTransitionFactsProjection,
) -> bool {
    production_classification_facts_equal(left, right)
        && production_delta_facts_equal(left, right)
}

/// Equality of the factored EnterView field plus all remaining fields is
/// extensional equality of the complete production fact projection.
pub proof fn production_transition_fact_extensionality(
    left: ProductionTransitionFactsProjection,
    right: ProductionTransitionFactsProjection,
)
    requires
        left.enter_view_exact == right.enter_view_exact,
        production_non_enter_view_facts_equal(left, right),
    ensures
        left == right,
{
    reveal(production_non_enter_view_facts_equal);
    reveal(production_classification_facts_equal);
    reveal(production_delta_facts_equal);
}

/// Executable projection of invariant, fence, and classification facts from
/// the exact shared constructor.
pub fn verified_classification_facts_from_projection(
    projection: ProductionTransitionProjection,
) -> (facts: ProductionTransitionFactsProjection)
    ensures
        production_classification_facts_equal(
            facts,
            production_facts_from_projection(projection),
        ),
{
    let facts = transition_facts_from_projection_body!(
        projection,
        ProductionTransitionFactsProjection
    );
    proof {
        reveal(production_classification_facts_equal);
        reveal(production_facts_from_projection);
    }
    facts
}

/// Executable projection of volatile, durable-delta, capability, and effect
/// facts from the exact shared constructor.
pub fn verified_delta_facts_from_projection(
    projection: ProductionTransitionProjection,
) -> (facts: ProductionTransitionFactsProjection)
    ensures
        production_delta_facts_equal(
            facts,
            production_facts_from_projection(projection),
        ),
{
    let facts = transition_facts_from_projection_body!(
        projection,
        ProductionTransitionFactsProjection
    );
    proof {
        reveal(production_delta_facts_equal);
        reveal(production_facts_from_projection);
    }
    facts
}

/// Exact EnterView fact when the transition is inactive.
pub fn verified_inactive_enter_view_fact(
    projection: ProductionTransitionProjection,
) -> (enter_view_exact: bool)
    requires
        !projection.enter_view.active,
    ensures
        enter_view_exact == production_enter_view_exact_fact(projection),
{
    let enter_view_exact = production_enter_view_exact_body!(projection);
    proof {
        reveal(production_enter_view_exact_fact);
    }
    enter_view_exact
}

/// Exact active EnterView fact when neither a local nor incoming lock exists.
pub fn verified_empty_enter_view_lock_fact(
    projection: ProductionTransitionProjection,
) -> (enter_view_exact: bool)
    requires
        projection.enter_view.active,
        !projection.enter_view.local_lock_before.present,
        !projection.enter_view.pending_record_timeout.highest_prepare.present,
    ensures
        enter_view_exact == production_enter_view_exact_fact(projection),
{
    let enter_view_exact = production_enter_view_exact_body!(projection);
    proof {
        reveal(production_enter_view_exact_fact);
    }
    enter_view_exact
}

/// Exact active EnterView fact when only the incoming highest `PrepareQC`
/// exists.
pub fn verified_incoming_only_enter_view_lock_fact(
    projection: ProductionTransitionProjection,
) -> (enter_view_exact: bool)
    requires
        projection.enter_view.active,
        !projection.enter_view.local_lock_before.present,
        projection.enter_view.pending_record_timeout.highest_prepare.present,
    ensures
        enter_view_exact == production_enter_view_exact_fact(projection),
{
    let enter_view_exact = production_enter_view_exact_body!(projection);
    proof {
        reveal(production_enter_view_exact_fact);
    }
    enter_view_exact
}

/// Exact active EnterView fact when only the pre-transition local lock exists.
pub fn verified_local_only_enter_view_lock_fact(
    projection: ProductionTransitionProjection,
) -> (enter_view_exact: bool)
    requires
        projection.enter_view.active,
        projection.enter_view.local_lock_before.present,
        !projection.enter_view.pending_record_timeout.highest_prepare.present,
    ensures
        enter_view_exact == production_enter_view_exact_fact(projection),
{
    let enter_view_exact = production_enter_view_exact_body!(projection);
    proof {
        reveal(production_enter_view_exact_fact);
    }
    enter_view_exact
}

/// Exact active EnterView fact when the local lock is at least as high as the
/// incoming highest `PrepareQC`.
pub fn verified_local_max_enter_view_lock_fact(
    projection: ProductionTransitionProjection,
) -> (enter_view_exact: bool)
    requires
        projection.enter_view.active,
        projection.enter_view.local_lock_before.present,
        projection.enter_view.pending_record_timeout.highest_prepare.present,
        projection.enter_view.pending_record_timeout.highest_prepare.view
            <= projection.enter_view.local_lock_before.view,
    ensures
        enter_view_exact == production_enter_view_exact_fact(projection),
{
    let enter_view_exact = production_enter_view_exact_body!(projection);
    proof {
        reveal(production_enter_view_exact_fact);
    }
    enter_view_exact
}

/// Exact active EnterView fact when the incoming highest `PrepareQC` is
/// strictly higher than the local lock.
pub fn verified_incoming_max_enter_view_lock_fact(
    projection: ProductionTransitionProjection,
) -> (enter_view_exact: bool)
    requires
        projection.enter_view.active,
        projection.enter_view.local_lock_before.present,
        projection.enter_view.pending_record_timeout.highest_prepare.present,
        projection.enter_view.pending_record_timeout.highest_prepare.view
            > projection.enter_view.local_lock_before.view,
    ensures
        enter_view_exact == production_enter_view_exact_fact(projection),
{
    let enter_view_exact = production_enter_view_exact_body!(projection);
    proof {
        reveal(production_enter_view_exact_fact);
    }
    enter_view_exact
}

/// Executable projection of only the exact EnterView fact from the same shared
/// constructor. The selected-certificate cases are discharged separately from
/// the remaining transition structure.
pub fn verified_enter_view_fact_from_projection(
    projection: ProductionTransitionProjection,
) -> (enter_view_exact: bool)
    ensures
        enter_view_exact == production_enter_view_exact_fact(projection),
{
    let enter_view = projection.enter_view;
    let local = enter_view.local_lock_before;
    let incoming = enter_view.pending_record_timeout.highest_prepare;
    if !enter_view.active {
        verified_inactive_enter_view_fact(projection)
    } else if !local.present {
        if incoming.present {
            verified_incoming_only_enter_view_lock_fact(projection)
        } else {
            verified_empty_enter_view_lock_fact(projection)
        }
    } else if !incoming.present {
        verified_local_only_enter_view_lock_fact(projection)
    } else if incoming.view <= local.view {
        verified_local_max_enter_view_lock_fact(projection)
    } else {
        verified_incoming_max_enter_view_lock_fact(projection)
    }
}

/// Executable fact derivation used to prove that action and authorization
/// booleans cannot be supplied independently of their primitive witnesses.
pub fn verified_facts_from_projection(
    projection: ProductionTransitionProjection,
) -> (facts: ProductionTransitionFactsProjection)
    ensures
        facts == production_facts_from_projection(projection),
{
    let classification_facts = verified_classification_facts_from_projection(projection);
    let delta_facts = verified_delta_facts_from_projection(projection);
    let enter_view_exact = verified_enter_view_fact_from_projection(projection);
    let facts = ProductionTransitionFactsProjection {
        before_invariant: classification_facts.before_invariant,
        after_invariant: classification_facts.after_invariant,
        context_unchanged: classification_facts.context_unchanged,
        whole_state_unchanged: classification_facts.whole_state_unchanged,
        tag_matches: classification_facts.tag_matches,
        busy_fence_open: classification_facts.busy_fence_open,
        event_kind: classification_facts.event_kind,
        action_kind: classification_facts.action_kind,
        wal_record_kind: classification_facts.wal_record_kind,
        signed_message_kind: classification_facts.signed_message_kind,
        replay_effect_kind: classification_facts.replay_effect_kind,
        validator_count: classification_facts.validator_count,
        volatile_before: delta_facts.volatile_before,
        volatile_after: delta_facts.volatile_after,
        durable_unchanged: delta_facts.durable_unchanged,
        pending_unchanged: delta_facts.pending_unchanged,
        generation_unchanged: delta_facts.generation_unchanged,
        application_unchanged: delta_facts.application_unchanged,
        begin_persist_exact: delta_facts.begin_persist_exact,
        acknowledge_persist_exact: delta_facts.acknowledge_persist_exact,
        application_transition_exact: delta_facts.application_transition_exact,
        acknowledgement_continuation: delta_facts.acknowledgement_continuation,
        enter_view_exact,
        effects: delta_facts.effects,
    };
    proof {
        let expected = production_facts_from_projection(projection);
        production_enter_view_fact_projection_is_exact(projection);
        assert(enter_view_exact == expected.enter_view_exact);
        assert(production_non_enter_view_facts_equal(facts, expected)) by {
            reveal(production_non_enter_view_facts_equal);
            reveal(production_classification_facts_equal);
            reveal(production_delta_facts_equal);
        }
        production_transition_fact_extensionality(facts, expected);
    }
    facts
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
    ResumeProposal,
    ResumeVote,
    ResumeTimeout,
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
        15 => match facts.replay_effect_kind {
            1 => TlaActionNameProjection::ResumeProposal,
            2 | 3 => TlaActionNameProjection::ResumeVote,
            4 => TlaActionNameProjection::ResumeTimeout,
            5 => TlaActionNameProjection::FetchBody,
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
        | TlaActionNameProjection::FormTC
        | TlaActionNameProjection::ResumeProposal
        | TlaActionNameProjection::ResumeVote
        | TlaActionNameProjection::ResumeTimeout => true,
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

/// One active slot is authorized only by equality of its complete requested
/// and independently granted capability keys.
pub open spec fn production_effect_slot_authorized(
    slot: ProductionEffectSlotProjection,
) -> bool {
    active_effect_slot_body!(slot)
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

/// Exact replay-resumption action relation enforced by production.
pub closed spec fn production_resume_after_replay_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    resume_after_replay_action_body!(facts, production_effect_count)
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
        production_resume_after_replay_action_relation,
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
    production_transition_gate_body!(
        facts,
        production_volatile_summary_well_formed,
        production_named_action_relation,
        production_effect_trace_relation,
        production_transition_branch_relation,
    )
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
            ==> facts.action_kind == 3 || facts.action_kind == 4 || facts.action_kind == 6,
        facts.action_kind == 6 && facts.replay_effect_kind != 0
            ==> production_tla_macro_step(facts).source
                != TlaActionNameProjection::NoAction,
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
    reveal(production_resume_after_replay_action_relation);
    match facts.action_kind {
        0 => {},
        1 | 2 => {
            match facts.wal_record_kind {
                1 | 2 | 3 | 4 | 5 | 6 | 7 => {},
                _ => {},
            }
        },
        3 | 4 | 5 => {},
        6 => {
            match facts.replay_effect_kind {
                0 | 1 | 2 | 3 | 4 | 5 => {},
                _ => {},
            }
        },
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
                && facts.volatile_after.body_work <= 1
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

/// Exact executable replay-resumption checker used by production.
pub fn verified_resume_after_replay_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_resume_after_replay_action_relation(facts),
{
    let accepted = resume_after_replay_action_body!(facts, verified_effect_count_gate);
    proof {
        reveal(production_resume_after_replay_action_relation);
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
        verified_resume_after_replay_action_gate,
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

/// Safety relation of the exact primitive production kernel.
pub closed spec fn production_kernel_relation(
    projection: ProductionTransitionProjection,
) -> bool {
    production_transition_action_relation(production_facts_from_projection(projection))
}

/// An accepted active persisted-TC transition selects the maximum of the
/// pre-transition lock and the TC's highest `PrepareQC`, carries that exact
/// durable lock in `EnterView`, and emits one immediately following recovery
/// fetch exactly when the selected lock is present.
///
/// This is a serialized transition theorem only. It intentionally makes no
/// claim that asynchronous transport or executor work is eventually serviced.
pub proof fn accepted_core_enter_view_projection_selects_post_install_lock(
    projection: ProductionTransitionProjection,
)
    requires
        production_kernel_relation(projection),
        projection.enter_view.active,
    ensures
        production_enter_view_projection_relation(projection.enter_view),
        certificate_identity_equal_body!(
            projection.enter_view.durable_lock_after,
            production_enter_view_selected_lock(projection.enter_view)
        ),
        certificate_identity_equal_body!(
            projection.enter_view.effect_protected_lock,
            projection.enter_view.durable_lock_after
        ),
        projection.enter_view.durable_lock_after.present
            <==> production_enter_view_has_exact_following_fetch(projection.enter_view),
        !projection.enter_view.durable_lock_after.present
            ==> projection.enter_view.fetch_count == 0
                && !projection.enter_view.following_fetch_lock.present,
{
    reveal(production_kernel_relation);
    reveal(production_transition_action_relation);
    reveal(production_facts_from_projection);
    reveal(production_enter_view_projection_relation);
    reveal(production_enter_view_selected_lock);
    reveal(production_enter_view_has_exact_following_fetch);
}

/// Exact executable kernel called conceptually by `refinement::accepts`:
/// derive facts from concrete primitives first, then evaluate the established
/// transition gate.  No authorization or action-exactness boolean is an input.
pub fn verified_production_kernel(
    projection: ProductionTransitionProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_kernel_relation(projection),
{
    let facts = verified_facts_from_projection(projection);
    let accepted = verified_production_transition_gate(facts);
    proof {
        reveal(production_kernel_relation);
    }
    accepted
}

/// A nonzero invariant-violation counter cannot be hidden by the fact
/// derivation and therefore cannot pass the production kernel.
pub proof fn production_kernel_rejects_invalid_before_state(
    projection: ProductionTransitionProjection,
)
    requires
        projection.safety_before.durable_identity_mismatches > 0
            || projection.safety_before.asynchronous_fence_conflicts > 0
            || projection.safety_before.invalid_highest_prepare > 0
            || projection.safety_before.invalid_lock > 0
            || projection.safety_before.invalid_timeout > 0
            || projection.safety_before.invalid_decision > 0
            || projection.safety_before.invalid_pending_append > 0
            || projection.safety_before.unauthorized_signables > 0
            || projection.safety_before.invalid_application > 0,
    ensures
        !production_kernel_relation(projection),
{
    reveal(production_kernel_relation);
    reveal(production_transition_action_relation);
}

/// A counterfeit grant whose primitive key differs from the requested key in
/// any active slot cannot authorize the effect vector.
pub proof fn production_effect_slot_rejects_counterfeit_grant(
    slot: ProductionEffectSlotProjection,
)
    requires
        1 <= slot.kind <= 9,
        slot.requested.kind == slot.kind,
        slot.granted.kind == slot.kind,
        !capability_key_equal_body!(slot.requested, slot.granted),
    ensures
        !production_effect_slot_authorized(slot),
{
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
        facts.enter_view_exact,
        production_effect_trace_relation(facts.effects, facts.event_kind),
        production_transition_branch_relation(facts),
        (!facts.tag_matches || !facts.busy_fence_open)
            ==> facts.whole_state_unchanged,
        facts.effects.len <= 8,
        facts.effects.len > 0 ==> production_effect_slot_authorized(facts.effects.slot0),
        facts.effects.len > 1 ==> production_effect_slot_authorized(facts.effects.slot1),
        facts.effects.len > 2 ==> production_effect_slot_authorized(facts.effects.slot2),
        facts.effects.len > 3 ==> production_effect_slot_authorized(facts.effects.slot3),
        facts.effects.len > 4 ==> production_effect_slot_authorized(facts.effects.slot4),
        facts.effects.len > 5 ==> production_effect_slot_authorized(facts.effects.slot5),
        facts.effects.len > 6 ==> production_effect_slot_authorized(facts.effects.slot6),
        facts.effects.len > 7 ==> production_effect_slot_authorized(facts.effects.slot7),
        effect_count_body!(facts.effects, 1u8) > 0
            ==> effect_count_body!(facts.effects, 5u8) == 0
                && effect_count_body!(facts.effects, 7u8) == 0
                && effect_count_body!(facts.effects, 8u8) == 0,
        effect_count_body!(facts.effects, 5u8) > 0
            ==> match facts.effects.len {
                1 => facts.effects.slot0.kind == 5,
                2 => facts.effects.slot1.kind == 5,
                3 => facts.effects.slot2.kind == 5,
                4 => facts.effects.slot3.kind == 5,
                5 => facts.effects.slot4.kind == 5,
                6 => facts.effects.slot5.kind == 5,
                7 => facts.effects.slot6.kind == 5,
                8 => facts.effects.slot7.kind == 5,
                _ => false,
            },
        effect_count_body!(facts.effects, 8u8) > 0
            ==> facts.acknowledge_persist_exact
                && facts.acknowledgement_continuation == 2,
{
    reveal(production_transition_action_relation);
}



} // verus!
