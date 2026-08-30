#!/usr/bin/env python3
"""Check that the bounded Parliament model remains bound to implementation guards.

This deterministic structural check is intentionally narrower than parsing or
compiling Rust. It fails when a modeled guard disappears, when authenticated
registration or reducer-derived corpus boundaries regress, when a plaintext or
fallback transition enters the closed Parliament instruction enum, when global
timed-OVN reservation admission or restore loses fail-atomic capacity checks, when a
persisted attempt can exceed the authoritative framed-state bound, when a signed
draft can claim a consensus-owned certificate outcome, or when the
current specifications regress to the retired proposal-time JIT description. It
also pins the modeled no-pulse hidden-electorate capacity failure and atomic
Policy-to-Confirmation capacity handoff. The model and implementation must also
share one proposal-wide fresh-randomness redraw ceiling across successor
attempts, sortition/Confirmation generations, and timed-OVN ballot retries;
committed transport replay must remain state-idempotent.
It also keeps the PR model run bound to archived copies of its exact inputs and
to stable, source-identified result metadata.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def read(relative: str) -> str:
    path = ROOT / relative
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise RuntimeError(f"cannot read {relative}: {error}") from error
    marker = re.search(r"^(?:<<<<<<< .+|=======|>>>>>>> .+)$", text, re.M)
    if marker is not None:
        raise RuntimeError(
            f"{relative}: unresolved merge marker {marker.group(0)!r}"
        )
    return text


RUST_INCLUDE_LINE = re.compile(
    r'^(?P<indent>[ \t]*)include!\(\s*"(?P<path>[^"]+)"\s*\);[ \t]*$', re.M
)


def read_rust_with_includes(relative: str, stack: tuple[str, ...] = ()) -> str:
    """Read one Rust source and recursively expand its textual item includes."""
    if relative in stack:
        chain = " -> ".join((*stack, relative))
        raise RuntimeError(f"cyclic Rust include chain: {chain}")
    text = read(relative)
    parent = Path(relative).parent

    def expand(match: re.Match[str]) -> str:
        include_path = Path(match.group("path"))
        if include_path.is_absolute() or ".." in include_path.parts:
            raise RuntimeError(
                f"{relative}: non-local Rust include path {include_path.as_posix()!r}"
            )
        child = (parent / include_path).as_posix()
        expanded = read_rust_with_includes(child, (*stack, relative))
        indent = match.group("indent")
        if not indent:
            return expanded
        return indent + expanded.replace("\n", f"\n{indent}")

    return RUST_INCLUDE_LINE.sub(expand, text)


def require_all(relative: str, text: str, needles: tuple[str, ...]) -> None:
    missing = [needle for needle in needles if needle not in text]
    if missing:
        rendered = ", ".join(repr(item) for item in missing)
        raise RuntimeError(f"{relative}: missing modeled source binding(s): {rendered}")


def section(text: str, start: str, end: str, relative: str) -> str:
    pattern = re.compile(re.escape(start) + r"(?P<body>.*?)" + re.escape(end), re.S)
    match = pattern.search(text)
    if match is None:
        raise RuntimeError(f"{relative}: cannot locate section beginning {start!r}")
    return match.group("body")


def public_field_names(text: str) -> tuple[str, ...]:
    """Return public named fields from one Rust DTO source section."""
    return tuple(re.findall(r"\bpub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", text))


def main() -> int:
    types_path = "crates/iroha_data_model/src/governance/types.rs"
    types = read(types_path)
    require_all(
        types_path,
        types,
        (
            "if self.pulse_height <= self.request_height",
            "MAX_PARLIAMENT_BALLOT_RETRIES_V1: u32 = 16",
            "MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1: u32 = 16",
            "MAX_PARLIAMENT_SORTITION_RETRIES_V1: u32 = 16",
            "MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1: u32 = 1_000",
            "PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1: usize = 32",
            "MAX_PARLIAMENT_ATTEMPT_STATE_BYTES_V1: usize = 16 * 1024 * 1024",
            "pub fn parliament_timed_ovn_required_chunk_blocks_v1",
            "parliament_ballot_failure_root_v1",
            "parliament_ballot_result_root_v1",
            "OpeningDeadlineExpired",
            "pub enum ParliamentNoResultKindV1",
            "PublicFindingQuorumUnreachable",
            "PublicFindingDeadlineExpired",
            "BallotOpeningDeadlineExpired",
            "SortitionRetriesExhausted",
            "ConfirmationJuryCapacityUnavailable",
            "RandomnessRedrawBudgetExhausted",
            "impl From<ParliamentBallotFailureKindV1> for ParliamentNoResultKindV1",
            "ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted => {",
            "Self::RandomnessRedrawBudgetExhausted",
            "pub opening_deadline_height: u64",
            "ExecutionFailed",
            "parliament_execution_failure_root_v1",
            "pub const fn parliament_quorum_seats_v1",
            "parliament_public_finding_endorsement_root_v1",
            "pub struct ParliamentPublicFindingCertificateBindingV1",
            "pub endorsement_root: [u8; 32]",
            "pub endorsing_assignments: Vec<AssignmentId>",
            "pub endorsements: u32",
            "pub quorum: u32",
            "let endorsements = u32::try_from(public_finding.endorsing_assignments.len())",
            "public_finding.endorsements != quorum",
            ".endorsing_assignments\n                        .windows(2)",
            ".all(|pair| pair[0] < pair[1])",
            "ballot.commitment_closed_at_height <= ballot.survivor_freeze_height",
            "ballot.commitment_closed_at_height > ballot.commitment_close_height",
            ".saturating_sub(ballot.survivor_freeze_height)",
            "parliament_timed_ovn_required_chunk_blocks_v1(",
        ),
    )
    if "AggregateOpeningFailed" in types:
        raise RuntimeError(
            f"{types_path}: unverifiable caller-triggered aggregate-opening failure remains"
        )

    reducer_path = "crates/iroha_core/src/governance/parliament.rs"
    reducer = read_rust_with_includes(reducer_path)
    require_all(
        reducer_path,
        reducer,
        (
            "first consumed pulse must cover every initially required body in one",
            "sortition_pulse_delay_blocks: u64",
            ".checked_add(self.sortition_pulse_delay_blocks)",
            "InvalidSortitionPulseSchedule",
            "SortitionPulseAvailable",
            "SortitionRetryLimitExceeded",
            "GovernanceAttemptRetryLimitExceeded",
            "MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "RandomnessRedrawLimitExceeded",
            "RandomnessRedrawLineageMismatch",
            "randomness_redraws_before_attempt: u32",
            "pub(crate) fn randomness_redraws_used_v1(",
            "fn ensure_sortition_generation_redraw_available_v1(",
            "fn ensure_ballot_redraw_available_v1(",
            "validate_parliament_randomness_redraw_lineage_v1",
            "AttemptStateSizeLimitExceeded",
            "TimedOvnResourceScheduleConflict",
            "TooManyConcurrentCastingContexts",
            "pub fn register_sortition_request_batch(",
            "MAX_PARLIAMENT_SORTITION_RETRIES_V1",
            "ParliamentElectionFailureKindV1::PulseUnavailable",
            "ParliamentElectionFailureKindV1::EmptyAcceptedRoster",
            "ParliamentElectionFailureKindV1::InsufficientHiddenBallotRoster",
            "pub struct ParliamentSortitionCapacityFailureV1 {",
            "pub fn record_hidden_sortition_capacity_failure_batch(",
            "candidate_snapshot.len() >= 2",
            "failure.failure_height != failure.request_height",
            "active_sortition_capacity_failures",
            "BodyElectionAttemptStatusV1::AwaitingPulse\n                    | BodyElectionAttemptStatusV1::Drawing\n                    | BodyElectionAttemptStatusV1::AcceptingInvitations",
            "request.request_height < failure_height",
            "election.attempt.sequence == MAX_PARLIAMENT_SORTITION_RETRIES_V1",
            "election_awaiting_pulse_shape_is_empty(election)",
            "pulse_missing_terminal",
            "pub(crate) fn precheck_close_ballot_registration(",
            "current_height == ballot.registration_close_height",
            "pub(crate) fn precheck_freeze_ballot_survivors(",
            "current_height == ballot.survivor_freeze_height",
            "pub(crate) fn precheck_freeze_timed_ovn_corpus(",
            "timed_commitment_height_is_in_window(ballot, current_height)",
            "height > ballot.survivor_freeze_height",
            "height <= ballot.commitment_close_height",
            "timed_commitment_completed_in_window(ballot)",
            "ParliamentBallotFailureKindV1::CommitmentDeadlineExpired",
            "minimum_registration_phase_blocks = u64::from(policy.max_corpus_entries)",
            "policy.registration_phase_blocks < minimum_registration_phase_blocks",
            "policy.survivor_freeze_phase_blocks < minimum_survivor_freeze_phase_blocks",
            "parliament_timed_ovn_required_chunk_blocks_v1(policy.max_corpus_entries)",
            "timed_ovn_policy.max_corpus_entries < original_seats",
            "policy.opening_phase_blocks == 0",
            ".checked_add(policy.opening_phase_blocks)",
            "at_height > ballot.opening_deadline_height",
            "result_height > opening_deadline_height",
            "self.used_tle_sessions.contains_key(&tle_session_id)",
            "self.used_tle_sessions\n            .insert(tle_session_id, ballot_attempt_id)",
            "previous.attempt.status != BallotAttemptStatusV1::NoResult",
            "registered_at_height < failure_height",
            "release_pulse_available: bool",
            "classify_ballot_failure(ballot, release_pulse_available, current_height)",
            "current_height > ballot.opening_deadline_height",
            "ParliamentBallotFailureKindV1::ReleasePulseUnavailable",
            "ParliamentBallotFailureKindV1::OpeningDeadlineExpired",
            "ParliamentBallotFailureKindV1::ConfirmationJuryCapacityUnavailable",
            "ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted",
            "eligible_confirmation_candidates: Option<u32>",
            "if requires_confirmation && eligible_confirmation_candidates < 2",
            "request.target_seats < 2",
            "candidate_snapshot.len() < 2",
            "request.request_height < policy_result_height",
            "request.request_height != policy_result_height",
            "sequences.get(&0)",
            "fn policy_margin_is_strict_and_atomic_confirmation_roster_is_fresh()",
            "fn hidden_sortition_capacity_failure_is_typed_bounded_and_consumes_no_pulse()",
            "fn hidden_sortition_capacity_restore_rejects_mutated_evidence()",
            "fn live_sortition_candidates_retain_bonds_until_terminal_or_superseded()",
            "fn retryable_singleton_capacity_failure_retains_only_its_live_candidate_bond()",
            "restore must reject a narrow Policy approval without its atomic Confirmation request",
            "restore must reject a Confirmation snapshot backdated before the Policy result",
            "restore must reject a sequence-zero Confirmation snapshot delayed past the Policy result",
            "parliament_ballot_failure_root_v1(",
            "if at_height != certificate.enact_at_height",
            "if observed_head == certificate.expected_head",
            "pub fn mark_execution_failed(",
            "parliament_execution_failure_root_v1(",
            "GovernanceAttemptStatusV1::ExecutionFailed",
            "pub fn record_attempt_absence(",
            "if &assignment.member != member",
            "!body.public_finding_endorsements.is_empty()",
            "pub fn endorse_public_finding(",
            "find(|assignment| &assignment.member == member)",
            ".contains_key(&assignment.assignment_id)",
            "let quorum = parliament_quorum_seats_v1(body.instance.original_seats);",
            ".insert(assignment_id, result_root);",
            "if endorsements < quorum",
            "fn public_finding_quorum_is_unreachable(",
            ".checked_sub(body.excluded_assignments.len())",
            ".checked_sub(body.public_finding_endorsements.len())",
            "strongest_existing_root.saturating_add(remaining) < quorum",
            "pub fn fail_public_finding_no_result(",
            "if current_height <= deadline_height",
            "ParliamentNoResultKindV1::PublicFindingDeadlineExpired",
            "let retry_budget_exhausted = ballot.attempt.sequence == ballot.max_ballot_retries;",
            "proposal_wide_redraw_budget_composes_sortition_and_timed_ovn_retries",
            "successor_attempt_inherits_exact_proposal_redraw_prefix",
            "narrow_policy_at_randomness_redraw_ceiling_persists_terminal_no_result",
            "an exact transport retry must not spend a redraw unit",
            "parliament_public_finding_endorsement_root_v1(",
            "body.public_finding_binding = Some(ParliamentPublicFindingCertificateBindingV1",
            "endorsing_assignments,\n                endorsements,\n                quorum,",
        ),
    )
    attempt_size_validation = section(
        reducer,
        "pub(crate) fn validate_encoded_size_v1(",
        "fn expected_completed_body_count_v1(",
        reducer_path,
    )
    require_all(
        reducer_path,
        attempt_size_validation,
        (
            "norito::core::encoded_frame_len(self)",
            "MAX_PARLIAMENT_ATTEMPT_STATE_BYTES_V1",
            "ParliamentReducerErrorV1::AttemptStateSizeLimitExceeded",
        ),
    )
    redraw_accounting = section(
        reducer,
        "pub(crate) fn randomness_redraws_used_v1(",
        "fn ensure_sortition_generation_redraw_available_v1(",
        reducer_path,
    )
    require_all(
        reducer_path,
        redraw_accounting,
        (
            "self.attempt.sequence == 0 && sortition_generations > 0",
            ".checked_sub(baseline_generations)",
            ".filter(|ballot| ballot.attempt.sequence > 0)",
            ".randomness_redraws_before_attempt",
            ".checked_add(sortition_redraws)",
            ".and_then(|used| used.checked_add(ballot_redraws))",
            "used > MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
        ),
    )
    sortition_redraw_guard = section(
        reducer,
        "fn ensure_sortition_generation_redraw_available_v1(",
        "fn ensure_ballot_redraw_available_v1(",
        reducer_path,
    )
    require_all(
        reducer_path,
        sortition_redraw_guard,
        (
            "generations.contains(&slot)",
            "self.attempt.sequence == 0 && generations.is_empty()",
            "self.randomness_redraws_used_v1()? < MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "ParliamentReducerErrorV1::RandomnessRedrawLimitExceeded",
        ),
    )
    ballot_redraw_guard = section(
        reducer,
        "fn ensure_ballot_redraw_available_v1(",
        "/// Return whether this immutable attempt currently retains",
        reducer_path,
    )
    require_all(
        reducer_path,
        ballot_redraw_guard,
        (
            "sequence == 0",
            "self.randomness_redraws_used_v1()? < MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "ParliamentReducerErrorV1::RandomnessRedrawLimitExceeded",
        ),
    )
    confirmation_redraw_terminalization = section(
        reducer,
        "pub fn finalize_opened_ballot(",
        "/// Construct and freeze the complete automatic governance certificate.",
        reducer_path,
    )
    require_all(
        reducer_path,
        confirmation_redraw_terminalization,
        (
            "eligible_confirmation_candidates < 2",
            "self.randomness_redraws_used_v1()? == MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "Some(ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted)",
            "if let Some(failure_kind) = confirmation_failure_kind",
            "ballot.failure_kind = Some(failure_kind)",
            "ballot.attempt.status = BallotAttemptStatusV1::NoResult",
            "BodyInstanceStatusV1::NoResult",
            "self.attempt.status = GovernanceAttemptStatusV1::Rejected",
            "return Ok(ParliamentAggregateOutcomeV1::NoResult)",
        ),
    )
    sortition_failure_terminalization = section(
        reducer,
        "pub fn fail_body_election_no_roster(",
        "/// Seal a canonical roster into a new body instance.",
        reducer_path,
    )
    require_all(
        reducer_path,
        sortition_failure_terminalization,
        (
            "let proposal_redraw_budget_exhausted =",
            "self.randomness_redraws_used_v1()? == MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "retry_budget_exhausted || proposal_redraw_budget_exhausted",
            "election.attempt.sequence == MAX_PARLIAMENT_SORTITION_RETRIES_V1",
            "self.attempt.status = GovernanceAttemptStatusV1::Rejected",
        ),
    )
    ballot_failure_terminalization = section(
        reducer,
        "pub fn fail_ballot_no_result(",
        "/// Finalize a cryptographically opened aggregate and its body result.",
        reducer_path,
    )
    require_all(
        reducer_path,
        ballot_failure_terminalization,
        (
            "let proposal_redraw_budget_exhausted =",
            "self.randomness_redraws_used_v1()? == MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "retry_budget_exhausted || proposal_redraw_budget_exhausted",
            "self.attempt.status = GovernanceAttemptStatusV1::Rejected",
        ),
    )
    confirmation_redraw_terminal_test = section(
        reducer,
        "fn narrow_policy_at_randomness_redraw_ceiling_persists_terminal_no_result()",
        "fn sealed_and_released_cross_store_bindings_fail_closed_on_substitution()",
        reducer_path,
    )
    require_all(
        reducer_path,
        confirmation_redraw_terminal_test,
        (
            "MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "ParliamentAggregateOutcomeV1::NoResult",
            "GovernanceAttemptStatusV1::Rejected",
            '"the unaffordable Confirmation draw must never enter the pipeline"',
            '"the narrow Policy result must remain uncommitted"',
            "BodyInstanceStatusV1::NoResult",
            "BallotAttemptStatusV1::NoResult",
            "ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted",
            '"redraw-exhausted opening must restore canonically"',
            '"the redraw-exhaustion classification requires the exact shared ceiling"',
        ),
    )
    redraw_lineage = section(
        reducer,
        "pub(crate) fn validate_parliament_randomness_redraw_lineage_v1",
        "#[cfg(test)]\npub(crate) mod tests {",
        reducer_path,
    )
    require_all(
        reducer_path,
        redraw_lineage,
        (
            "attempts.sort_unstable_by_key(|attempt| attempt.borrow().attempt.sequence)",
            "attempt.randomness_redraws_before_attempt != expected_prefix",
            "attempt.randomness_redraws_before_attempt >= MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "expected_prefix = attempt.randomness_redraws_used_v1()?",
        ),
    )
    full_attempt_validation = section(
        reducer,
        "pub fn validate(&self) -> Result<(), ParliamentReducerErrorV1> {",
        "#[cfg(any(test, feature = \"iroha-core-tests\"))]",
        reducer_path,
    )
    if not full_attempt_validation.lstrip().startswith(
        "self.validate_encoded_size_v1()?;"
    ):
        raise RuntimeError(
            f"{reducer_path}: full attempt validation must begin with the exact size-only guard"
        )
    execution_failure_signature = re.search(
        r"pub fn mark_execution_failed\((?P<params>.*?)\)\s*->", reducer, re.S
    )
    if execution_failure_signature is None:
        raise RuntimeError(f"{reducer_path}: cannot locate execution-failure reducer")
    for caller_field in ("failure_root", "effect_preimage_hash"):
        if caller_field in execution_failure_signature.group("params"):
            raise RuntimeError(
                f"{reducer_path}: execution-failure reducer accepts caller field {caller_field!r}"
            )
    if "AggregateOpeningFailed" in reducer:
        raise RuntimeError(
            f"{reducer_path}: unverifiable caller-triggered aggregate-opening failure remains"
        )
    absence_reducer = section(
        reducer,
        "pub fn record_attempt_absence(",
        "fn build_ballot_binding(",
        reducer_path,
    )
    require_all(
        reducer_path,
        absence_reducer,
        (
            "public_finding_deadline_height",
            ".is_some_and(|deadline| current_height > deadline)",
            "ParliamentReducerErrorV1::PublicFindingWindowClosed",
        ),
    )

    instruction_path = "crates/iroha_data_model/src/isi/governance/parliament.rs"
    instructions = read(instruction_path)
    require_all(
        instruction_path,
        instructions,
        (
            "MAX_PARLIAMENT_SORTITION_REQUESTS_PER_BATCH_V1: usize = 10",
            "pub struct ParliamentSortitionRequestRegistrationV1",
            "pub requests: Vec<ParliamentSortitionRequestRegistrationV1>",
            "next contiguous corpus chunk is appended",
            "complete exact survivor coverage and causes automatic corpus sealing",
        ),
    )
    for misplaced_policy_definition in (
        "pub const PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1",
        "pub fn parliament_timed_ovn_required_chunk_blocks_v1",
    ):
        if misplaced_policy_definition in instructions:
            raise RuntimeError(
                f"{instruction_path}: timed-OVN chunk policy must be owned by {types_path}"
            )
    transition_enum = section(
        instructions,
        "pub enum ParliamentLifecycleTransitionV1 {",
        "/// Bounded audit classification",
        instruction_path,
    )
    for forbidden in (
        "Plain",
        "Plaintext",
        "Fallback",
        "ManualOpen",
        "ConstructCertificate",
        "MarkEnacted",
        "MarkSuperseded",
        "MarkExecutionFailed",
        "FinalizePublicFinding",
    ):
        if forbidden in transition_enum:
            raise RuntimeError(
                f"{instruction_path}: closed Parliament transition enum contains {forbidden!r}"
            )
    automatic_outcome = section(
        instructions,
        "pub enum ParliamentAutomaticExecutionOutcomeV1 {",
        "/// One closed, versioned transition accepted",
        instruction_path,
    )
    require_all(
        instruction_path,
        automatic_outcome,
        (
            "Enacted",
            "Superseded(ParliamentAutomaticSupersededV1)",
            "ExecutionFailed(ParliamentAutomaticExecutionFailedV1)",
        ),
    )
    require_all(
        instruction_path,
        transition_enum,
        (
            "RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1)",
            "EndorsePublicFinding(ParliamentEndorsePublicFindingV1)",
            "FailPublicFindingNoResult(ParliamentFailPublicFindingNoResultV1)",
        ),
    )
    require_all(
        instruction_path,
        instructions,
        (
            "PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOME_DIGEST_V1",
            "impl ParliamentAutomaticExecutionOutcomeV1",
            "pub fn digest_v1(self) -> [u8; 32]",
        ),
    )
    failure_payload = section(
        instructions,
        "pub struct ParliamentFailBallotNoResultV1 {",
        "/// Canonical public final threshold release record",
        instruction_path,
    )
    require_all(
        instruction_path,
        failure_payload,
        ("pub ballot_attempt_id: BallotAttemptId",),
    )
    if "failure_kind" in failure_payload or "failure_root" in failure_payload:
        raise RuntimeError(
            f"{instruction_path}: caller can select reducer-derived ballot failure evidence"
        )
    objective_election_progress_payloads = (
        (
            "pub struct ParliamentConsumeSortitionPulseBatchV1 {",
            "/// Payload beginning invitation acceptance after a deterministic draw.",
            ("request_ids", "beacon_session_id", "pulse_height", "pulse_id"),
        ),
        (
            "pub struct ParliamentBeginInvitationAcceptanceV1 {",
            "/// Payload terminally recording a missing sortition pulse or empty roster.",
            ("election_attempt_id",),
        ),
        (
            "pub struct ParliamentFailBodyElectionNoRosterV1 {",
            "/// A candidate's response to one canonical Parliament invitation.",
            ("election_attempt_id",),
        ),
        (
            "pub struct ParliamentSealBodyRosterV1 {",
            "/// Payload advancing one sealed body by exactly one deliberation phase.",
            ("election_attempt_id",),
        ),
    )
    for start, end, expected_fields in objective_election_progress_payloads:
        payload = section(instructions, start, end, instruction_path)
        actual_fields = public_field_names(payload)
        if actual_fields != expected_fields:
            raise RuntimeError(
                f"{instruction_path}: objective election-progress payload {start!r} "
                f"must expose exactly {expected_fields!r}, found {actual_fields!r}"
            )
    absence_payload = section(
        instructions,
        "pub struct ParliamentRecordAttemptAbsenceV1 {",
        "/// Payload endorsing one public nonbinding Parliament finding",
        instruction_path,
    )
    require_all(
        instruction_path,
        absence_payload,
        ("pub body_instance_id: BodyInstanceId", "pub assignment_id: AssignmentId"),
    )
    for caller_field in ("pub member:", "pub authority:"):
        if caller_field in absence_payload:
            raise RuntimeError(
                f"{instruction_path}: self-absence accepts caller field {caller_field!r}"
            )
    endorsement_payload = section(
        instructions,
        "pub struct ParliamentEndorsePublicFindingV1 {",
        "/// Payload triggering objective expiry of one public-finding endorsement window.",
        instruction_path,
    )
    require_all(
        instruction_path,
        endorsement_payload,
        ("pub body_instance_id: BodyInstanceId", "pub result_root: [u8; 32]"),
    )
    for caller_field in (
        "pub assignment_id:",
        "pub member:",
        "pub authority:",
        "pub endorsement_root:",
        "pub endorsements:",
        "pub quorum:",
    ):
        if caller_field in endorsement_payload:
            raise RuntimeError(
                f"{instruction_path}: public-finding endorsement accepts caller field {caller_field!r}"
            )
    public_failure_payload = section(
        instructions,
        "pub struct ParliamentFailPublicFindingNoResultV1 {",
        "/// Payload registering a fresh private timed-OVN ballot attempt.",
        instruction_path,
    )
    require_all(
        instruction_path,
        public_failure_payload,
        ("pub body_instance_id: BodyInstanceId",),
    )
    for caller_field in (
        "result_root",
        "failure_kind",
        "failure_height",
        "deadline_height",
    ):
        if caller_field in public_failure_payload:
            raise RuntimeError(
                f"{instruction_path}: public-finding expiry accepts caller field {caller_field!r}"
            )
    for start, end, forbidden in (
        (
            "pub struct ParliamentCloseBallotRegistrationV1 {",
            "/// Payload recording one registered seated member's authenticated dropout.",
            ("registration_records", "roster_root", "registered_voters"),
        ),
        (
            "pub struct ParliamentRecordBallotDropoutV1 {",
            "/// Payload freezing the exact nonempty survivor subset derived by Core.",
            ("participant_hash", "member_id"),
        ),
        (
            "pub struct ParliamentFreezeBallotSurvivorsV1 {",
            "/// Payload appending the exact next timed-OVN ciphertext and one-hot-proof chunk.",
            ("survivor_participant_hashes", "dropout_root", "survivor_corpus_root"),
        ),
    ):
        payload = section(instructions, start, end, instruction_path)
        for field in forbidden:
            if field in payload:
                raise RuntimeError(
                    f"{instruction_path}: caller-selected {field!r} re-entered {start}"
                )

    world_path = "crates/iroha_core/src/smartcontracts/isi/world.rs"
    world = read(world_path)
    require_all(
        world_path,
        world,
        (
            ".checked_add(configured_delay)",
            "parliament_certificate_enactment_height_v1(",
            "observed_head != certificate.expected_head",
            "apply_parliament_proposal_effect_v1",
            "parliament_finalized_pulse_seed_v1",
            "parliament_verified_pulse_available_v1",
            "entry.request.target_seats != configured_target",
            "let first = payload.requests.first()",
            "for entry in &payload.requests",
            ".register_sortition_request_batch(",
            "request.beacon_session_id",
            "request.pulse_height",
            "pulse_available,\n                            current_height",
            ".global_beacon_pulses",
            "ballot.release_beacon_session_id()",
            "release_pulse_available",
            "validate_ballot_registration_member",
            "parliament_ballot_participant_hash_v1",
            "lifecycle.registration_records().len()",
            ".close_registration(&tle_key_session)",
            "validate_ballot_dropout_member",
            ".record_dropout(participant_hash, &tle_key_session)",
            ".freeze_survivors(&tle_key_session)",
            ".seal_ballots(payload.ballot_records, &tle_key_session)",
            "PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1",
            "DueParliamentCertificateExecutionV1",
            "execute_due_parliament_certificate_v1",
            "record_due_parliament_execution_failure_v1",
            "parliament_execution_failure_root_v1(",
            "ParliamentAutomaticExecutionOutcomeV1::Enacted",
            "ParliamentAutomaticExecutionOutcomeV1::Superseded",
            "ParliamentAutomaticSupersededV1 { observed_head }",
            "ParliamentAutomaticExecutionOutcomeV1::ExecutionFailed",
            "ParliamentAutomaticExecutionFailedV1 {",
            "ParliamentLifecycleTransitionV1::RecordAttemptAbsence(payload)",
            ".record_attempt_absence(",
            "payload.assignment_id,\n                            authority,\n                            current_height,",
            "ParliamentLifecycleTransitionV1::EndorsePublicFinding(payload)",
            ".endorse_public_finding(",
            "payload.result_root,\n                            authority,\n                            current_height,",
            "ParliamentLifecycleTransitionV1::FailPublicFindingNoResult(payload)",
            ".fail_public_finding_no_result(",
            "state_transaction.gov.parliament_public_finding_phase_blocks",
            "no_result_kind",
            "fn validated_active_parliament_tle_key_session_for_new_ballot_v1(",
            ".tle_key_session_eligible_for_new_ballots(",
            "key_session_id,\n                state_transaction.block_height(),",
            ".tle_key_session_rosters()",
            "frozen_ordered_roster != Some(ordered_roster)",
            '"active Parliament TLE key session is not bound to the current commit topology"',
            "let randomness_redraws_before_attempt = previous",
            "ParliamentAttemptStateV1::randomness_redraws_used_v1",
            "randomness_redraws_before_attempt\n                    >= crate::governance::parliament::MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1",
            "ParliamentAttemptStateV1::try_new_with_randomness_redraws_before_attempt(",
        ),
    )
    manager_partition = section(
        world,
        "const fn parliament_transition_requires_manager_v1(",
        "fn parliament_certificate_enactment_height_v1(",
        world_path,
    )
    require_all(
        world_path,
        manager_partition,
        (
            "ParliamentLifecycleTransitionKindV1::ConsumeSortitionPulseBatch",
            "ParliamentLifecycleTransitionKindV1::BeginInvitationAcceptance",
            "ParliamentLifecycleTransitionKindV1::FailBodyElectionNoRoster",
            "ParliamentLifecycleTransitionKindV1::SealBodyRoster",
            "ParliamentLifecycleTransitionKindV1::FreezeTimedOvnCorpus",
        ),
    )
    for manager_only in (
        "ParliamentLifecycleTransitionKindV1::EscalateRisk",
        "ParliamentLifecycleTransitionKindV1::CompleteQualification",
        "ParliamentLifecycleTransitionKindV1::RegisterSortitionRequest",
        "ParliamentLifecycleTransitionKindV1::AdvanceBodyPhase",
        "ParliamentLifecycleTransitionKindV1::RegisterBallotAttempt",
    ):
        if manager_only in manager_partition:
            raise RuntimeError(
                f"{world_path}: manager-only intent transition became permissionless: "
                f"{manager_only!r}"
            )
    create_attempt_execution = section(
        world,
        "impl Execute for gov::CreateParliamentGovernanceAttemptV1 {",
        "fn confirmation_candidate_snapshot_v1(",
        world_path,
    )
    require_all(
        world_path,
        create_attempt_execution,
        ("require_parliament_manager(authority, state_transaction)?;",),
    )
    require_all(
        world_path,
        world,
        (
            "fn parliament_progress_authority_partition_is_exact()",
            "fn parliament_hidden_sortition_capacity_is_objective_for_zero_and_one_candidate()",
            "fn parliament_sortition_pulse_consumption_is_permissionless_and_exactly_bound()",
            "fn parliament_invitation_start_is_permissionless_and_election_bound()",
            "fn parliament_no_roster_failure_is_permissionless_and_reducer_derived()",
            "fn parliament_roster_sealing_is_permissionless_and_transcript_derived()",
            "fn parliament_proof_heavy_ballot_corpus_is_permissionless_but_shape_checked()",
            "fn parliament_non_manager_can_append_the_exact_next_timed_ovn_chunks()",
        ),
    )
    for branch_start, branch_end, bindings in (
        (
            "gov::ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::BeginInvitationAcceptance(payload) => {",
            (
                "parliament_finalized_pulse_seed_v1(",
                "payload.request_ids",
                "payload.beacon_session_id",
                "payload.pulse_height",
                "payload.pulse_id",
                "pulse_output",
                "&state_transaction.network_id",
                "&state_transaction.gov",
            ),
        ),
        (
            "gov::ParliamentLifecycleTransitionV1::BeginInvitationAcceptance(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::FailBodyElectionNoRoster(payload) => {",
            (
                "payload.election_attempt_id",
                "current_height",
                "state_transaction.gov.parliament_invitation_phase_blocks",
            ),
        ),
        (
            "gov::ParliamentLifecycleTransitionV1::FailBodyElectionNoRoster(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::SealBodyRoster(payload) => {",
            (
                "parliament_verified_pulse_available_v1(",
                "request.beacon_session_id",
                "request.pulse_height",
                "pulse_available",
                "current_height",
            ),
        ),
        (
            "gov::ParliamentLifecycleTransitionV1::SealBodyRoster(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::AdvanceBodyPhase(payload) =>",
            ("payload.election_attempt_id", "current_height"),
        ),
    ):
        branch = section(world, branch_start, branch_end, world_path)
        require_all(world_path, branch, bindings)
    register_sortition_branch = section(
        world,
        "gov::ParliamentLifecycleTransitionV1::RegisterSortitionRequest(payload) => {",
        "gov::ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(payload) => {",
        world_path,
    )
    require_all(
        world_path,
        register_sortition_branch,
        (
            "canonical_parliament_candidate_snapshot_v1(",
            "expected_candidates.len() < 2 && hidden_body_requested",
            ".record_hidden_sortition_capacity_failure_batch(",
            "ParliamentNoResultKindV1::SortitionRetriesExhausted",
            ".register_sortition_request_batch(",
        ),
    )
    for forbidden in (
        "ParliamentLifecycleTransitionV1::ConstructCertificate",
        "ParliamentLifecycleTransitionV1::MarkEnacted",
        "ParliamentLifecycleTransitionV1::MarkSuperseded",
        "ParliamentLifecycleTransitionV1::MarkExecutionFailed",
        "ParliamentLifecycleTransitionV1::FinalizePublicFinding",
    ):
        if forbidden in world:
            raise RuntimeError(
                f"{world_path}: consensus-owned action remains a public transition: {forbidden!r}"
            )
    finalize_branch = section(
        world,
        "gov::ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(payload) => {",
        "gov::ParliamentLifecycleTransitionV1::BeginBallotOpeningBatch(payload) => {",
        world_path,
    )
    require_all(
        world_path,
        finalize_branch,
        (
            "canonical_parliament_eligible_candidates_v1(",
            "confirmation_candidate_snapshot_v1(",
            "eligible_confirmation_candidates",
            ".finalize_opened_ballot(",
            "canonical_confirmation_sortition_request_v1(",
            ".register_sortition_request(",
            ".construct_certificate(",
        ),
    )
    require_all(
        world_path,
        world,
        (
            "fn narrow_confirmation_request_freezes_exact_current_snapshot_and_schedule()",
            "request_height + attempt.sortition_pulse_delay_blocks()",
            "request.id, request.canonical_id()",
        ),
    )
    for branch_start, branch_end, precheck, phase_guard, expensive_call in (
        (
            "gov::ParliamentLifecycleTransitionV1::CloseBallotRegistration(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::RecordBallotDropout(payload) => {",
            ".precheck_close_ballot_registration(",
            "TimedOvnLifecycleStateV1::Registered(_)",
            ".close_registration(&tle_key_session)",
        ),
        (
            "gov::ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(payload) => {",
            ".precheck_freeze_ballot_survivors(",
            "TimedOvnLifecycleStateV1::RegistrationClosed(_)",
            ".freeze_survivors(&tle_key_session)",
        ),
        (
            "gov::ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(payload) => {",
            ".precheck_freeze_timed_ovn_corpus(",
            "TimedOvnLifecycleStateV1::SurvivorsFrozen(_)",
            ".seal_ballots(payload.ballot_records, &tle_key_session)",
        ),
    ):
        branch = section(world, branch_start, branch_end, world_path)
        require_all(world_path, branch, (precheck, phase_guard, expensive_call))
        if branch.find(precheck) > branch.find(expensive_call):
            raise RuntimeError(
                f"{world_path}: {precheck!r} must precede proof-heavy {expensive_call!r}"
            )
        if branch.find(phase_guard) > branch.find(expensive_call):
            raise RuntimeError(
                f"{world_path}: {phase_guard!r} must precede proof-heavy {expensive_call!r}"
            )

    corpus_branch = section(
        world,
        "gov::ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(payload) => {",
        "gov::ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(payload) => {",
        world_path,
    )
    require_all(
        world_path,
        corpus_branch,
        (
            "TimedOvnLifecycleStateV1::CorpusOpen(_)",
            "if let TimedOvnLifecycleStateV1::Sealed(sealed) = &lifecycle",
            ".freeze_timed_ovn_corpus(",
        ),
    )
    api_path = "crates/iroha_torii_shared/src/parliament_api.rs"
    api = read(api_path)
    require_all(api_path, api, ("impl ParliamentTransitionDraftRequestV1 {",))
    for forbidden in (
        "Transition::ConstructCertificate",
        "Transition::MarkEnacted",
        "Transition::MarkSuperseded",
        "Transition::MarkExecutionFailed",
        "Transition::FinalizePublicFinding",
    ):
        if forbidden in api:
            raise RuntimeError(
                f"{api_path}: public draft code references retired transition {forbidden!r}"
            )
    require_all(
        api_path,
        api,
        (
            "pub execution_failure_root: Option<[u8; 32]>",
            "MAX_PARLIAMENT_ATTEMPT_STATE_BYTES_V1 as PARLIAMENT_ATTEMPT_READ_MAX_STATE_BYTES_V1",
        ),
    )

    torii_gov_path = "crates/iroha_torii/src/gov.rs"
    torii_gov = read(torii_gov_path)
    attempt_read = section(
        torii_gov,
        "pub async fn handle_gov_parliament_attempt_read(",
        "/// GET `/v1/gov/parliament/ballots/{ballot_attempt_id}/release-context`",
        torii_gov_path,
    )
    require_all(
        torii_gov_path,
        attempt_read,
        (
            "norito::core::to_bytes_bounded(",
            "PARLIAMENT_ATTEMPT_READ_MAX_STATE_BYTES_V1",
        ),
    )

    state_path = "crates/iroha_core/src/state.rs"
    state = read(state_path)
    require_all(
        state_path,
        state,
        (
            "due_parliament_certificates",
            "let mut enactment = sb.transaction();",
            "execute_due_parliament_certificate_v1(",
            "DueParliamentCertificateExecutionV1::EffectFailed",
            "drop(enactment);",
            "let mut failure = sb.transaction();",
            "record_due_parliament_execution_failure_v1(",
            "failure.apply();",
            "crate::telemetry::parliament_lifecycle_metric_projection(event)",
            "events.push(projection);",
            "telemetry.record_committed_parliament_transition(transition, no_result_kind);",
            "telemetry.seed_parliament_attempts(snapshot);",
            "telemetry_seed.seed_parliament_attempts(parliament_view.iter()",
            "validate_parliament_randomness_redraw_lineage_v1(",
            ".filter_map(|(persisted_id, persisted)|",
            ".chain(std::iter::once(&attempt))",
        ),
    )

    events_path = "crates/iroha_data_model/src/events/data/governance.rs"
    events = read(events_path)
    require_all(
        events_path,
        events,
        (
            "pub struct GovernanceParliamentLifecycleTransitionApplied",
            "pub no_result_kind: Option<ParliamentNoResultKindV1>",
            "pub automatic_outcome: Option<ParliamentAutomaticExecutionOutcomeV1>",
        ),
    )

    telemetry_path = "crates/iroha_core/src/telemetry.rs"
    telemetry = read(telemetry_path)
    require_all(
        telemetry_path,
        telemetry,
        (
            "fn parliament_transition_label(",
            "fn parliament_no_result_label(",
            "fn parliament_no_result_matches_transition(",
            "pub(crate) fn parliament_lifecycle_metric_projection(",
            "Some((payload.transition_kind, payload.no_result_kind))",
            "record_committed_parliament_transition(",
            "Transition::FailPublicFindingNoResult",
            "Transition::FailBallotNoResult",
            "Transition::FailBodyElectionNoRoster",
            'Kind::SortitionRetriesExhausted => "sortition_retries_exhausted"',
            "NoResult::SortitionRetriesExhausted => transition == Transition::FailBodyElectionNoRoster",
            '"confirmation_jury_capacity_unavailable"',
            "NoResult::ConfirmationJuryCapacityUnavailable",
            "transition == Transition::FinalizeOpenedBallot",
            "pub(crate) fn seed_parliament_attempts(",
            "governance_parliament_attempts_by_status",
            "governance_parliament_attempts_by_stage",
        ),
    )

    metrics_path = "crates/iroha_telemetry/src/metrics.rs"
    metrics = read(metrics_path)
    require_all(
        metrics_path,
        metrics,
        (
            "pub governance_parliament_transitions_total: int_counter_vec(&[\"transition\"])",
            "pub governance_parliament_no_result_total: int_counter_vec(&[\"class\"])",
            "pub governance_parliament_attempts_by_status: gauge_vec(&[\"status\"])",
            "pub governance_parliament_attempts_by_stage: gauge_vec(&[\"stage\"])",
            '"fail_public_finding_no_result"',
            '"public_finding_quorum_unreachable"',
            '"public_finding_deadline_expired"',
            '"ballot_opening_deadline_expired"',
            '"sortition_retries_exhausted"',
            '"confirmation_jury_capacity_unavailable"',
            "fn confirmation_capacity_no_result_metric_is_pre_registered()",
        ),
    )
    commitment_precheck = section(
        reducer,
        "pub(crate) fn precheck_freeze_timed_ovn_corpus(",
        "fn precheck_ballot_checkpoint(",
        reducer_path,
    )
    require_all(
        reducer_path,
        commitment_precheck,
        (
            "timed_commitment_height_is_in_window(ballot, current_height)",
            "ballot.survivors_frozen_at_height == Some(ballot.survivor_freeze_height)",
        ),
    )
    ballot_failure_classifier = section(
        reducer,
        "fn classify_ballot_failure(",
        "fn ballot_failure_matches_state(",
        reducer_path,
    )
    require_all(
        reducer_path,
        ballot_failure_classifier,
        (
            "BallotAttemptStatusV1::TimedCommitment",
            "current_height > ballot.commitment_close_height",
            "ParliamentBallotFailureKindV1::CommitmentDeadlineExpired",
        ),
    )

    defaults_path = "crates/iroha_config/src/parameters/defaults.rs"
    defaults = read(defaults_path)
    require_all(
        defaults_path,
        defaults,
        (
            "pub const PARLIAMENT_SORTITION_PULSE_DELAY_BLOCKS: u64 = 4",
            "pub const PARLIAMENT_PUBLIC_FINDING_PHASE_BLOCKS: u64 = 3_600",
            "pub const SURVIVOR_FREEZE_PHASE_BLOCKS: u64 = 1_000",
            "pub const OPENING_PHASE_BLOCKS: u64 = 600",
        ),
    )
    actual_config_path = "crates/iroha_config/src/parameters/actual.rs"
    actual_config = read(actual_config_path)
    require_all(
        actual_config_path,
        actual_config,
        (
            "pub parliament_sortition_pulse_delay_blocks: u64",
            '"governance.parliament_sortition_pulse_delay_blocks"',
            "pub parliament_public_finding_phase_blocks: u64",
            '"governance.parliament_public_finding_phase_blocks"',
            "pub opening_phase_blocks: u64",
            ".checked_add(self.opening_phase_blocks)",
            '("opening_phase_blocks", self.opening_phase_blocks)',
            '"governance.parliament_timed_ovn.opening_phase_blocks"',
            "parliament_timed_ovn_required_chunk_blocks_v1(",
            "self.registration_phase_blocks >= required_registration_blocks",
            "self.survivor_freeze_phase_blocks >= required_single_record_blocks",
            "self.commitment_phase_blocks >= required_chunk_blocks",
        ),
    )
    user_config_path = "crates/iroha_config/src/parameters/user.rs"
    user_config = read(user_config_path)
    require_all(
        user_config_path,
        user_config,
        (
            "pub parliament_sortition_pulse_delay_blocks: u64",
            "PARLIAMENT_SORTITION_PULSE_DELAY_BLOCKS",
            "parliament_sortition_pulse_delay_blocks: self",
            '"parliament_sortition_pulse_delay_blocks must be non-zero"',
            "pub parliament_public_finding_phase_blocks: u64",
            "PARLIAMENT_PUBLIC_FINDING_PHASE_BLOCKS",
            "parliament_public_finding_phase_blocks: self",
            '"parliament_public_finding_phase_blocks must be non-zero"',
            "pub opening_phase_blocks: u64",
            "OPENING_PHASE_BLOCKS",
            "opening_phase_blocks: self.opening_phase_blocks",
        ),
    )

    timed_path = "crates/iroha_crypto/src/timed_ovn.rs"
    timed = read(timed_path)
    require_all(
        timed_path,
        timed,
        (
            "no plaintext, manual-opening, or\n//! post-freeze recovery API",
            "validate_timed_ovn_official_release_audit_manifest_bytes_v1",
        ),
    )

    threshold_path = "crates/iroha_crypto/src/threshold_bls.rs"
    threshold = read(threshold_path)
    require_all(
        threshold_path,
        threshold,
        (
            "V1 fixes `n = 3f + 1`, signing threshold `f + 1`",
            "It implements no proactive/mobile-adversary refresh",
            "representation proof on every\n//! partial is mandatory",
            "scalar_bytes: Zeroizing<[[u8; 32]; 3]>",
        ),
    )

    evidence_path = "crates/iroha_core/src/governance/timed_ovn.rs"
    evidence = read(evidence_path)
    require_all(
        evidence_path,
        evidence,
        (
            "Replayable, public-only evidence",
            "no secret fields, private-share codec, individual opening",
            "aggregate-only tally operation",
            "fn verify_final_release_pregate(",
            "CorpusOpen(TimedOvnCorpusOpenStateV1)",
            "fn is_bounded_ballot_prefix_extension(",
            "pub(crate) fn validate_committed_cache(",
        ),
    )
    corpus_open_impl = section(
        evidence,
        "impl TimedOvnCorpusOpenStateV1 {",
        "/// Public-only persisted evidence for a complete sealed timed-OVN ballot corpus.",
        evidence_path,
    )
    require_all(
        evidence_path,
        corpus_open_impl,
        (
            "fn validate_committed_cache(",
            "self.frozen.verification_common(tle_key_session)",
            "self.accumulator.validate_shape()",
        ),
    )
    if "self.frozen.validate_committed_cache" in corpus_open_impl:
        raise RuntimeError(
            f"{evidence_path}: a corpus append must not rederive the predecessor's frozen cache"
        )
    lifecycle_finalize = section(
        evidence,
        "/// Verify the unique threshold release and persist the aggregate-only tally.",
        "/// Replay and validate all public evidence required by the current phase.",
        evidence_path,
    )
    require_all(
        evidence_path,
        lifecycle_finalize,
        ("sealed.finalize_release_committed_cache(",),
    )
    cached_finalize = section(
        evidence,
        "fn finalize_release_committed_cache(",
        "/// Public tally derived only after a valid threshold release opens the aggregate.",
        evidence_path,
    )
    require_all(
        evidence_path,
        cached_finalize,
        ("self.verify_final_release_pregate(", "self.validate_committed_cache("),
    )
    if cached_finalize.find("verify_final_release_pregate") > cached_finalize.find(
        "validate_committed_cache"
    ):
        raise RuntimeError(
            f"{evidence_path}: fixed-size final-release verification must precede committed-cache validation"
        )

    restore_path = "crates/iroha_core/src/state/deserialize_world.rs"
    restore = read(restore_path)
    require_all(
        restore_path,
        restore,
        (
            "TimedOvnLifecycleStateV1::CorpusOpen(_)",
            ".validated_parliament_reducer_binding(key_session)",
            "timed_ovn_reducer_binding_matches(ballot_attempt_id, &lifecycle_binding)",
            "FailureKind::ConfirmationJuryCapacityUnavailable",
            "FailureKind::RandomnessRedrawBudgetExhausted",
            "phase == PersistedTimedOvnPhaseV1::Released",
            "post-opening NoResult must retain its released timed-OVN evidence",
            "let tle_key_session_rosters = world.tle_key_session_rosters.view();",
            "validate_tle_key_session_roster_binding_v1(public_state, ordered_roster)",
            '"TLE key session {key_session_id} is missing its frozen ordered roster"',
            '"frozen ordered roster references missing TLE key session {key_session_id}"',
            "proposal_attempts.sort_unstable_by_key(|attempt| attempt.attempt().sequence)",
            "validate_parliament_randomness_redraw_lineage_v1(",
            '"governance Parliament randomness-redraw lineage is invalid: {error}"',
        ),
    )
    restore_size_validation = section(
        restore,
        "fn validate_parliament_attempt_encoded_size_bounds_v1(",
        "#[cfg(test)]\nmod timed_ovn_persistence_phase_tests",
        restore_path,
    )
    require_all(
        restore_path,
        restore_size_validation,
        (".validate_encoded_size_v1()", 'field: "parliament_attempts".into()'),
    )
    restore_attempt_prefix = section(
        restore,
        "world\n        .rebuild_global_beacon_pulse_slots()",
        "fn build_state(",
        restore_path,
    )
    require_all(
        restore_path,
        restore_attempt_prefix,
        ("validate_parliament_attempt_encoded_size_bounds_v1(&world)?;",),
    )
    restored_reservations = section(
        restore,
        "    let mut active_resource_reservations = Vec::new();",
        "    let concurrent_casting_contexts = timed_ovn_evidence",
        restore_path,
    )
    require_all(
        restore_path,
        restored_reservations,
        (
            "for (governance_attempt_id, governance_attempt) in parliament_attempts.iter()",
            "for left_index in 0..active_resource_reservations.len()",
            "for right_index in left_index + 1..active_resource_reservations.len()",
            "parliament_timed_ovn_resource_windows_overlap_v1(left_windows, right_windows)",
            '"active timed-OVN resource reservations overlap',
        ),
    )
    restored_capacity = section(
        restore,
        "    let concurrent_casting_contexts = timed_ovn_evidence",
        "    for (governance_attempt_id, governance_attempt) in parliament_attempts.iter()",
        restore_path,
    )
    require_all(
        restore_path,
        restored_capacity,
        (
            "MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1",
            "if concurrent_casting_contexts > maximum_casting_contexts",
            '"concurrent cast-capable timed-OVN contexts exceed the protocol maximum"',
        ),
    )

    state_path = "crates/iroha_core/src/state.rs"
    state = read(state_path)
    tle_roster_binding = section(
        state,
        "pub(crate) fn validate_tle_key_session_roster_binding_v1(",
        "/// Closed failures for the committed Parliament TLE key-session lifecycle.",
        state_path,
    )
    require_all(
        state_path,
        tle_roster_binding,
        (
            "let unique_peers = ordered_roster.iter().collect::<BTreeSet<_>>()",
            "ordered_roster.is_empty()",
            "unique_peers.len() != ordered_roster.len()",
            "usize::from(public_state.committee_size) != ordered_roster.len()",
            "global_threshold_beacon_roster_hash_v1(ordered_roster)",
        ),
    )
    checked_reservation_insert = section(
        state,
        "fn insert_parliament_timed_ovn_resource_reservation_v1(",
        "fn parliament_timed_ovn_reservation_reducer_error_v1(",
        state_path,
    )
    require_all(
        state_path,
        checked_reservation_insert,
        (
            "reservations.contains_key(&ballot_attempt_id)",
            "parliament_timed_ovn_resource_windows_overlap_v1(",
            "parliament_timed_ovn_casting_capacity_allows_new_v1(",
            "reservations.insert(ballot_attempt_id, reservation)",
        ),
    )
    insert_guards = (
        checked_reservation_insert.find("reservations.contains_key(&ballot_attempt_id)"),
        checked_reservation_insert.find("parliament_timed_ovn_resource_windows_overlap_v1("),
        checked_reservation_insert.find("parliament_timed_ovn_casting_capacity_allows_new_v1("),
        checked_reservation_insert.find("reservations.insert(ballot_attempt_id, reservation)"),
    )
    if tuple(sorted(insert_guards)) != insert_guards:
        raise RuntimeError(
            f"{state_path}: reservation duplicate/overlap/capacity guards must precede insertion"
        )
    attempt_admission = section(
        state,
        "    pub(crate) fn put_parliament_attempt(",
        "    /// Validate and persist one immutable public-only adaptive TLE key session.",
        state_path,
    )
    require_all(
        state_path,
        attempt_admission,
        (
            "attempt.validate()?",
            "let mut next_reservations = BTreeMap::new()",
            "insert_parliament_timed_ovn_resource_reservation_v1(",
            "for ballot_attempt_id in stale_reservations",
            "self.parliament_attempts.insert(id, attempt)",
        ),
    )
    if attempt_admission.find("insert_parliament_timed_ovn_resource_reservation_v1(") > attempt_admission.find(
        "for ballot_attempt_id in stale_reservations"
    ):
        raise RuntimeError(
            f"{state_path}: attempt admission mutates the live reservation index before validation"
        )
    tle_session_admission = section(
        state,
        "    pub(crate) fn put_tle_key_session(",
        "    /// Schedule one committed public TLE key session for next-height activation.",
        state_path,
    )
    require_all(
        state_path,
        tle_session_admission,
        (
            "ordered_roster: Vec<PeerId>",
            "validate_tle_key_session_roster_binding_v1(&state, &ordered_roster)?",
            "self.tle_key_sessions.get(&key_session_id)",
            "self.tle_key_session_rosters.get(&key_session_id)",
            "(None, None)",
            "self.tle_key_sessions.insert(key_session_id, state)",
            ".insert(key_session_id, ordered_roster)",
            "_ => Err(TleReleaseAdapterError::TranscriptMismatch)",
        ),
    )
    required_tle_custody = section(
        state,
        "    fn tle_key_sessions_required_for_runtime_custody_v1(",
        "    /// Return the single ABI version accepted by the first release runtime.",
        state_path,
    )
    require_all(
        state_path,
        required_tle_custody,
        (
            "let next_height = committed_height.checked_add(1).unwrap_or(committed_height);",
            "self.selectable_tle_key_session_for_fresh_ballot_at(next_height)",
            "attempt.validate()?",
            "for (_, ballot) in attempt.ballot_attempts()",
            "let opening_deadline = ballot.opening_deadline_height()",
            "(*deadline).max(opening_deadline)",
            ".or_insert(opening_deadline)",
            "committed_height <= deadline",
        ),
    )
    rebuilt_reservations = section(
        state,
        "    fn rebuild_governance_read_indexes(",
        "    /// Rebuild the unique `(network, height)` lookup",
        state_path,
    )
    require_all(
        state_path,
        rebuilt_reservations,
        (
            "-> Result<(), String>",
            "let mut timed_ovn_resource_reservations = BTreeMap::new()",
            "insert_parliament_timed_ovn_resource_reservation_v1(",
            "self.parliament_timed_ovn_resource_reservations =",
            "Ok(())",
        ),
    )
    if rebuilt_reservations.find("insert_parliament_timed_ovn_resource_reservation_v1(") > rebuilt_reservations.find(
        "self.parliament_timed_ovn_resource_reservations ="
    ):
        raise RuntimeError(
            f"{state_path}: restore publishes the reservation index before complete validation"
        )

    tle_release_path = "crates/iroha_core/src/tle_release.rs"
    tle_release = read(tle_release_path)
    require_all(
        tle_release_path,
        tle_release,
        (
            "pub struct AuthorizedTleReleaseContextV1",
            "pub fn authorize_parliament_tle_release_v1(",
            "BallotAttemptStatusV1::Opening",
            "finalized_height > opening_deadline_height",
            "pub trait TlePartialReleaseSignerV1: Send + Sync",
            "fn attest_partial_release_capability(",
            "expected_participant_index: u16,\n    ) -> Result<TlePartialReleaseCapabilityAttestationV1, TlePartialReleaseCapabilityErrorV1>;",
            "pub struct TlePartialReleaseCapabilityAttestationV1",
            "pub enum TlePartialReleaseCapabilityErrorV1",
            "impl TlePartialReleaseSignerV1 for InMemoryTlePartialReleaseSignerV1",
            "expected_participant_index != self.share.index()",
            "TlePartialReleaseCapabilityAttestationV1::for_validated_session(",
            "context: &AuthorizedTleReleaseContextV1",
            "pub struct AuthorizedTleReleaseProjectionV1",
            "pub struct ValidatedTleReleaseProjectionV1",
            "pub trait TleProjectedPartialReleaseSignerV1: Send + Sync",
            "projection: &ValidatedTleReleaseProjectionV1",
            "pub fn broker_projection_v1(",
            "pub const TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1: usize = 243",
            "pub identity_payload: [u8; TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1]",
            "let session = self.key_session.clone().validate()?;",
            ".validate_release_identity(&identity, self.finalized_height)?",
            "impl TleProjectedPartialReleaseSignerV1 for InMemoryTlePartialReleaseSignerV1",
            "pub struct InMemoryTlePartialReleaseSignerV1",
            "pub use custody::{RuntimeTleReleaseShareCustodyV1",
        ),
    )
    authorized_context = section(
        tle_release,
        "pub struct AuthorizedTleReleaseContextV1 {",
        "/// Authorize a TLE release from one point-in-time committed state view.",
        tle_release_path,
    )
    for forbidden in ("NoritoSerialize", "JsonSerialize", "Encode", "pub identity:"):
        if forbidden in authorized_context:
            raise RuntimeError(
                f"{tle_release_path}: opaque release authorization exposes {forbidden!r}"
            )
    validated_projection = section(
        tle_release,
        "pub struct ValidatedTleReleaseProjectionV1 {",
        "/// Closed failures while validating a public authenticated-broker projection.",
        tle_release_path,
    )
    for forbidden in ("NoritoSerialize", "NoritoDeserialize", "JsonSerialize", "JsonDeserialize"):
        if forbidden in validated_projection:
            raise RuntimeError(
                f"{tle_release_path}: validated broker projection exposes {forbidden!r}"
            )
    if re.search(
        r"impl\s+(?:Try)?From<[^>]*ValidatedTleReleaseProjectionV1[^>]*>\s+for\s+AuthorizedTleReleaseContextV1",
        tle_release,
    ):
        raise RuntimeError(
            f"{tle_release_path}: public broker projection can mint opaque Core authorization"
        )

    release_runtime_path = "crates/iroha_core/src/tle_release/runtime.rs"
    release_runtime = read(release_runtime_path)
    require_all(
        release_runtime_path,
        release_runtime,
        (
            "authorize_parliament_tle_release_v1(state, ballot_attempt_id)?",
            ".sign_partial_release(context)",
            ".verify_partial_release(context.identity(), context.finalized_height(), &partial)",
            "canonical.sort_by_key(|partial| partial.participant_index)",
            ".combine_partial_releases(context.identity(), context.finalized_height(), &canonical)",
            ".verify_final_release(",
            "ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(",
        ),
    )

    custody_path = "crates/iroha_core/src/tle_release/custody.rs"
    custody = read(custody_path)
    require_all(
        custody_path,
        custody,
        (
            "pub struct RuntimeTleReleaseShareCustodyV1",
            "RwLock<BTreeMap<TleKeySessionId, InMemoryTlePartialReleaseSignerV1>>",
            "pub fn insert_validated_share(",
            "pub fn import_components(",
            "pub fn import_committed_components(",
            ".tle_key_sessions()",
            "pub fn retire_session(",
            "let next_height = committed_height.checked_add(1).unwrap_or(committed_height);",
            ".tle_key_session_eligible_for_new_ballots(key_session_id, next_height)",
            "attempt.tle_key_session_retention_deadline(key_session_id)",
            "deadline == u64::MAX || committed_height <= deadline",
            ".remove(&key_session_id)",
            "drop(retired);",
            ".get(&context.session().public_state().key_session_id)",
            "fn attest_partial_release_capability(",
            ".get(&session.public_state().key_session_id)",
            "signer.attest_partial_release_capability(session, expected_participant_index)",
            "impl TleProjectedPartialReleaseSignerV1 for RuntimeTleReleaseShareCustodyV1",
            ".get(&projection.session().public_state().key_session_id)",
            "signer.sign_projected_partial_release(projection)",
        ),
    )
    retirement = section(
        custody,
        "pub fn retire_session(",
        "impl Default for RuntimeTleReleaseShareCustodyV1",
        custody_path,
    )
    if retirement.find("tle_key_session_eligible_for_new_ballots") > retirement.find(
        "for (_, attempt)"
    ):
        raise RuntimeError(
            f"{custody_path}: active-session retirement guard must precede ballot deadline scan"
        )
    custody_type = section(
        custody,
        "pub struct RuntimeTleReleaseShareCustodyV1 {",
        "impl RuntimeTleReleaseShareCustodyV1 {",
        custody_path,
    )
    for forbidden in ("derive(", "pub sessions", "Vec<TleKeySessionId>"):
        if forbidden in custody_type:
            raise RuntimeError(
                f"{custody_path}: runtime custody exposes forbidden inventory surface {forbidden!r}"
            )

    casting_path = "crates/iroha_core/src/tle_release/casting.rs"
    casting = read(casting_path)
    require_all(
        casting_path,
        casting,
        (
            "pub fn authorize_parliament_timed_ovn_casting_context_v1(",
            "pub struct AuthorizedTimedOvnCastingContextV1",
            "pub struct ParliamentTimedOvnCastingContextArchiveV1",
            "pub struct ValidatedParliamentTimedOvnCastingContextArchiveV1",
            "pub enum ParliamentTimedOvnCastingPhaseV1",
            "PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_VERSION_V1: u16 = 1",
            "PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1: usize = 4 * 1024 * 1024",
            "norito::core::to_bytes_bounded(",
            "pub fn try_from_parts_v1(",
            "pub fn validate_v1(",
            "rebuild_casting_registration_context_v1(",
            "PreparedTimedOvnAttemptV1::from_records(",
            "lifecycle.validate(&tle_key_session)?",
            "fn validate_casting_phase_window_v1(",
            "registered_at_height < registration_close_height",
            "registration_close_height < survivor_freeze_height",
            "survivor_freeze_height < commitment_close_height",
            "commitment_close_height < release_height",
            "current_height >= registered_at_height",
            "current_height < registration_close_height",
            "current_height >= registration_close_height",
            "current_height < survivor_freeze_height",
            "current_height >= survivor_freeze_height",
            "current_height < commitment_close_height",
            "TimedOvnCastingAuthorizationErrorV1::InvalidPhaseSchedule",
            "TimedOvnCastingAuthorizationErrorV1::PhaseWindowInactive",
            "GovernanceAttemptStatusV1::Active",
            "BodyInstanceStatusV1::Balloting",
            "ParliamentDecisionModeV1::HiddenBindingBallot",
            ".active_ballot_for_body(&body_instance_id)",
            ".registration_opened_at_finalized_height()",
            "TimedOvnLifecycleStateV1::Sealed(_) | TimedOvnLifecycleStateV1::Released(_)",
        ),
    )
    casting_authorization = section(
        casting,
        "pub fn authorize_parliament_timed_ovn_casting_context_v1(",
        "/// Closed failures while authorizing a public timed-OVN casting context.",
        casting_path,
    )
    if casting_authorization.find("validate_casting_phase_window_v1(") > casting_authorization.find(
        "lifecycle.validate(&tle_key_session)?"
    ):
        raise RuntimeError(
            f"{casting_path}: exact phase window must be checked before proof-heavy lifecycle replay"
        )
    authorized_casting = section(
        casting,
        "/// Constructor-authenticated, replay-validated timed-OVN casting context.",
        "/// Authorize and replay-validate one public timed-OVN casting context.",
        casting_path,
    )
    for forbidden in (
        "NoritoSerialize",
        "NoritoDeserialize",
        "JsonSerialize",
        "JsonDeserialize",
        "ballot_records:",
        "dropout_participant_hashes:",
        "partial_release:",
        "opening_root:",
    ):
        if forbidden in authorized_casting:
            raise RuntimeError(
                f"{casting_path}: opaque casting authorization exposes {forbidden!r}"
            )
    casting_archive = section(
        casting,
        "/// Canonical public-only archive for restarting a timed-OVN wallet operation.",
        "/// Constructor-authenticated, replay-validated timed-OVN casting context.",
        casting_path,
    )
    casting_archive_declaration = section(
        casting,
        "/// Canonical public-only archive for restarting a timed-OVN wallet operation.",
        "impl ParliamentTimedOvnCastingContextArchiveV1 {",
        casting_path,
    )
    require_all(
        casting_path,
        casting_archive,
        (
            "NoritoSerialize",
            "NoritoDeserialize",
            "registration_records: Vec<Vec<u8>>",
            "registration_opened_at_finalized_height: u64",
            "survivor_participant_hashes: Option<Vec<[u8; 32]>>",
            "release_identity: Option<TimedOvnReleaseIdentityPublicV1>",
        ),
    )
    for forbidden in (
        "JsonSerialize",
        "JsonDeserialize",
        "ballot_records:",
        "dropout_participant_hashes:",
        "partial_release:",
        "opening_root:",
        "AccountId:",
        "registration_close_height: u64",
        "survivor_freeze_height: u64",
        "commitment_close_height: u64",
    ):
        if forbidden in casting_archive_declaration:
            raise RuntimeError(
                f"{casting_path}: casting archive exposes forbidden material {forbidden!r}"
            )

    local_release_path = "crates/iroha_torii/src/parliament_tle_release.rs"
    local_release = read(local_release_path)
    require_all(
        local_release_path,
        local_release,
        (
            "pub(crate) async fn request_local_partial_release_v1(",
            "ballot_attempt_id: String",
            "signer_admission: crate::QueryAdmissionPermit",
            "crate::panic_recovery::join_recoverable(",
            "crate::panic_recovery::spawn_blocking_recoverable(",
            "let _signer_admission = signer_admission;",
            "coordinator.request_partial_release(&view, ballot_attempt_id)",
            "TleReleaseCoordinatorErrorV1::SignerUnavailable",
            "TleReleaseCoordinatorErrorV1::InvalidSignerOutput",
        ),
    )

    torii_path = "crates/iroha_torii/src/lib.rs"
    torii = read(torii_path)
    partial_handler = section(
        torii,
        "async fn handler_gov_parliament_tle_partial_release(",
        "async fn handler_gov_citizen_status(",
        torii_path,
    )
    require_all(
        torii_path,
        partial_handler,
        (
            '"v1/gov/parliament/ballots/{ballot_attempt_id}/partial-release"',
            "let signer_admission = acquire_query_admission(app.as_ref(), true).await?;",
            "ballot_attempt_id,\n        signer_admission,",
        ),
    )

    route_catalog_path = "crates/iroha_torii_shared/src/route_catalog.rs"
    route_catalog = read(route_catalog_path)
    partial_route = section(
        route_catalog,
        "pub const GOV_PARLIAMENT_TLE_PARTIAL_RELEASE: RouteDescriptor",
        "pub const GOV_PARLIAMENT_TRANSITION_DRAFT: RouteDescriptor",
        route_catalog_path,
    )
    require_all(
        route_catalog_path,
        partial_route,
        (
            "app_signed_post(",
            '"/v1/gov/parliament/ballots/{ballot_attempt_id}/partial-release"',
        ),
    )

    runtime_deps_path = "crates/irohad/src/main/runtime_deps.rs"
    runtime_deps = read(runtime_deps_path)
    require_all(
        runtime_deps_path,
        runtime_deps,
        (
            "parliament_tle_partial_release_signer:",
            "Option<Arc<dyn iroha_core::tle_release::TlePartialReleaseSignerV1>>",
            "with_parliament_tle_partial_release_signer(",
            "pub(crate) fn parliament_tle_release_coordinator(",
            "TleReleaseCoordinatorV1::without_signer",
            "TleReleaseCoordinatorV1::from_signer",
            "tle_key_sessions_required_for_runtime_custody_v1(committed_height)",
            ".tle_key_session_rosters()",
            "parliament_tle_local_participant_index_v1(frozen_roster, local_peer)",
            "require_parliament_tle_capability_for_local_seat_v1(",
            ".attest_partial_release_capability(session, participant_index)",
            "attestation.matches(session, participant_index)",
        ),
    )
    readiness = section(
        runtime_deps,
        "fn validate_threshold_signer_startup_readiness_v1(",
        "macro_rules! define_runtime_dep_setters_v1",
        runtime_deps_path,
    )
    if ".sign_partial_release(" in readiness:
        raise RuntimeError(
            f"{runtime_deps_path}: startup readiness must attest custody without signing"
        )
    readiness_fixture = section(
        runtime_deps,
        "fn threshold_signer_readiness_fixture_v1(",
        "fn parliament_tle_coordinator_is_fail_closed_or_runtime_injected() {",
        runtime_deps_path,
    )
    require_all(
        runtime_deps_path,
        readiness_fixture,
        (
            "const RETAINED_SESSION_BYTE: u8 = 0xD1",
            "const ACTIVE_SESSION_BYTE: u8 = 0xE1",
            "const RETENTION_DEADLINE_HEIGHT: u64 = 13",
            "active_validator_keys.reverse()",
            "let retained_participant_index = 2",
            "let active_participant_index = 3",
            "TleKeySessionId::new([RETAINED_SESSION_BYTE; 32])",
            "put_parliament_attempt_for_testing(attempt_id, attempt)",
            "while u64::try_from(block_hashes.len()).unwrap_or(u64::MAX) < committed_height",
        ),
    )
    retained_readiness_test = section(
        runtime_deps,
        "fn threshold_signer_startup_readiness_scans_active_and_deadline_retained_frozen_rosters() {",
        "fn threshold_signer_startup_readiness_skips_expired_history_and_rejects_mismatch() {",
        runtime_deps_path,
    )
    require_all(
        runtime_deps_path,
        retained_readiness_test,
        (
            "threshold_signer_readiness_fixture_v1(13)",
            "validate_threshold_signer_startup_readiness_v1(",
            "fixture.retained_key_session_id",
            "fixture.retained_participant_index",
            "fixture.active_key_session_id",
            "fixture.active_participant_index",
            "assert_eq!(calls, expected)",
            "signer.sign_calls.load(Ordering::Acquire), 0",
        ),
    )
    expired_readiness_test = section(
        runtime_deps,
        "fn threshold_signer_startup_readiness_skips_expired_history_and_rejects_mismatch() {",
        "fn threshold_signer_preflight_rejects_before_consensus_startup() {",
        runtime_deps_path,
    )
    require_all(
        runtime_deps_path,
        expired_readiness_test,
        (
            "threshold_signer_readiness_fixture_v1(14)",
            "validate_threshold_signer_startup_readiness_v1(",
            "exact_signer.attestation_calls()",
            "fixture.active_key_session_id",
            "fixture.active_participant_index",
            "CapabilityMode::MismatchedSeat",
            "mismatched_signer.attestation_calls()",
            "returned a mismatched runtime custody attestation",
        ),
    )
    if "fixture.retained_key_session_id" in expired_readiness_test:
        raise RuntimeError(
            f"{runtime_deps_path}: expired startup-readiness call set still includes the historical session"
        )

    broker_primitives_path = (
        "crates/irohad/src/runtime_provider_broker/protocol_primitives.rs"
    )
    broker_primitives = read(broker_primitives_path)
    require_all(
        broker_primitives_path,
        broker_primitives,
        (
            "OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1: u16 = 125",
            "ParliamentTleCapabilityAttestRequestWireV1",
            "ParliamentTleCapabilityAttestResultWireV1",
            "assert!(!super::super::operation_is_known(126))",
        ),
    )
    broker_attestation_request_wire = section(
        broker_primitives,
        "define_broker_wire_struct!(owned pub(super) ParliamentTleCapabilityAttestRequestWireV1 {",
        "define_broker_wire_struct!(owned pub(super) ParliamentTleCapabilityAttestResultWireV1 {",
        broker_primitives_path,
    )
    require_all(
        broker_primitives_path,
        broker_attestation_request_wire,
        (
            "pub(super) key_session: iroha_core::tle_release::TleKeySessionPublicStateV1",
            "pub(super) participant_index: u16",
        ),
    )
    broker_attestation_result_wire = section(
        broker_primitives,
        "define_broker_wire_struct!(owned pub(super) ParliamentTleCapabilityAttestResultWireV1 {",
        "pub(super) fn governance_signing_purpose_from_wire(",
        broker_primitives_path,
    )
    require_all(
        broker_primitives_path,
        broker_attestation_result_wire,
        (
            "pub(super) key_session_id: iroha_data_model::governance::types::TleKeySessionId",
            "pub(super) transcript_hash: [u8; 32]",
            "pub(super) participant_index: u16",
        ),
    )
    broker_validation_path = (
        "crates/irohad/src/runtime_provider_broker/protocol_operation_validation.rs"
    )
    broker_validation = read(broker_validation_path)
    broker_semantic_limits = section(
        broker_validation,
        "const fn operation_semantic_frame_limit(operation: u16) -> usize {",
        "const fn operation_frame_limit(operation: u16) -> usize {",
        broker_validation_path,
    )
    require_all(
        broker_validation_path,
        broker_semantic_limits,
        (
            "OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1",
            "MAX_CONSENSUS_SIGNER_FRAME_BYTES_V1",
        ),
    )
    broker_frame_limits = section(
        broker_validation,
        "const fn operation_frame_limit(operation: u16) -> usize {",
        "const fn operation_is_known(operation: u16) -> bool {",
        broker_validation_path,
    )
    require_all(
        broker_validation_path,
        broker_frame_limits,
        (
            "OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1",
            "MAX_CONSENSUS_SIGNER_FRAME_BYTES_V1",
        ),
    )
    broker_known_operations = section(
        broker_validation,
        "const fn operation_is_known(operation: u16) -> bool {",
        "fn provider_ingest_signer_context_from_wire(",
        broker_validation_path,
    )
    require_all(
        broker_validation_path,
        broker_known_operations,
        ("OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1",),
    )
    broker_attestation_request = section(
        broker_validation,
        "fn decode_parliament_tle_capability_attest_request(",
        "fn verify_parliament_tle_capability_attest_result(",
        broker_validation_path,
    )
    require_all(
        broker_validation_path,
        broker_attestation_request,
        (
            "request.key_session.network_id != *session_network_id.as_bytes()",
            ".key_session\n        .clone()\n        .validate()",
            "TlePartialReleaseCapabilityAttestationV1::for_validated_session(",
            "request.participant_index",
        ),
    )
    broker_attestation_result = section(
        broker_validation,
        "fn verify_parliament_tle_capability_attest_result(",
        "fn verify_parliament_tle_partial_release_result(",
        broker_validation_path,
    )
    require_all(
        broker_validation_path,
        broker_attestation_result,
        (
            "result.key_session_id != expected.key_session_id()",
            "result.transcript_hash != expected.transcript_hash()",
            "result.participant_index != expected.participant_index()",
            "return Err(BrokerError::Rejected)",
        ),
    )
    broker_typed_response = section(
        broker_validation,
        "fn validate_operation_response_for_client(",
        "fn validate_operation_response_envelope(",
        broker_validation_path,
    )
    require_all(
        broker_validation_path,
        broker_typed_response,
        (
            "OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1",
            "IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner.wire_id()",
            "response.status == STATUS_OK_V1",
            "return Ok(())",
        ),
    )
    broker_result_matrix = section(
        broker_validation,
        "fn validate_operation_result(",
        "fn sealed_slot_to_wire(",
        broker_validation_path,
    )
    broker_attestation_result_branch = section(
        broker_result_matrix,
        "            OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1 => {",
        "            OPERATION_MODERATION_HANDOFF_DELIVER_ONCE_V1 => {",
        broker_validation_path,
    )
    require_all(
        broker_validation_path,
        broker_attestation_result_branch,
        (
            "IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner.wire_id()",
            "decode_parliament_tle_capability_attest_request(",
            "decode_canonical::<ParliamentTleCapabilityAttestResultWireV1>(",
            "verify_parliament_tle_capability_attest_result(",
            ".map_err(|_| BrokerError::Protocol)?;",
        ),
    )
    broker_payload_path = (
        "crates/irohad/src/runtime_provider_broker/validate_operation_payload.rs"
    )
    broker_payload = read(broker_payload_path)
    broker_attestation_payload_branch = section(
        broker_payload,
        "        (slot, OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1)",
        "        (slot, OPERATION_BOOTLE_LANTERN_ISSUANCE_AUTHENTICATE_V1)",
        broker_payload_path,
    )
    require_all(
        broker_payload_path,
        broker_attestation_payload_branch,
        (
            "if slot == parliament_tle_partial_release_signer_slot",
            "decode_parliament_tle_capability_attest_request(",
            "&request.payload",
            "session_network_id",
        ),
    )
    broker_dispatch_path = (
        "crates/irohad/src/runtime_provider_broker/platform_operation_dispatch.rs"
    )
    broker_dispatch = read(broker_dispatch_path)
    require_all(
        broker_dispatch_path,
        broker_dispatch,
        (
            "OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1",
            "decode_parliament_tle_capability_attest_request(",
            ".attest_partial_release_capability(&session, request.participant_index)",
            "if !attestation.matches(&session, request.participant_index)",
            "requalify()?;",
            "ParliamentTleCapabilityAttestResultWireV1",
        ),
    )
    broker_api_path = "crates/irohad/src/runtime_provider_broker/api.rs"
    broker_api = read(broker_api_path)
    tle_broker_backend = section(
        broker_api,
        "pub trait ParliamentTlePartialReleaseSignerBrokerBackendV1: Send + Sync {",
        "/// One-shot lifecycle control shared by a broker launcher and serving thread.",
        broker_api_path,
    )
    require_all(
        broker_api_path,
        tle_broker_backend,
        (
            "fn attest_partial_release_capability(",
            "session: &iroha_core::tle_release::ValidatedTleKeySessionV1",
            "expected_participant_index: u16",
            "ParliamentTlePartialReleaseSignerBrokerBackendErrorV1,\n    >;",
        ),
    )
    broker_client_path = (
        "crates/irohad/src/runtime_provider_broker/platform_provider_clients_03.rs"
    )
    broker_client = read(broker_client_path)
    broker_attestation_client = section(
        broker_client,
        "    fn attest_projected_capability(",
        "    fn sign_projected_partial_release(",
        broker_client_path,
    )
    require_all(
        broker_client_path,
        broker_attestation_client,
        (
            "retry_consensus_signer_once_after_unavailable(",
            "live_exact_qualification(",
            "OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1",
            "ParliamentTleCapabilityAttestResultWireV1",
            "if attested.key_session_id != expected.key_session_id()",
            "attested.transcript_hash != expected.transcript_hash()",
            "attested.participant_index != expected.participant_index()",
            "self.session.poison();",
        ),
    )
    require_all(
        broker_client_path,
        broker_client,
        ("fn attest_partial_release_capability(",),
    )
    software_tle_path = (
        "crates/irohad/src/external_software_signer/consensus_threshold.rs"
    )
    software_tle = read(software_tle_path)
    require_all(
        software_tle_path,
        software_tle,
        (
            "fn attest_partial_release_capability(",
            "self.custody",
            ".attest_partial_release_capability(session, expected_participant_index)",
            "TlePartialReleaseCapabilityErrorV1::Unavailable",
            "ParliamentTlePartialReleaseSignerBrokerBackendErrorV1::Unavailable",
        ),
    )
    broker_tests_path = "crates/irohad/src/runtime_provider_broker/server_tests_04.rs"
    broker_tests = read(broker_tests_path)
    require_all(
        broker_tests_path,
        broker_tests,
        (
            "fn parliament_tle_capability_attestation_round_trips_over_authenticated_broker()",
            "wrong_seat_payload",
            "Err(BrokerError::Rejected)",
        ),
    )
    broker_typed_capability_success = section(
        broker_tests,
        "fn parliament_tle_capability_typed_proxy_requalifies_before_and_after_lookup()",
        "fn parliament_tle_partial_release_round_trips_over_authenticated_broker()",
        broker_tests_path,
    )
    require_all(
        broker_tests_path,
        broker_typed_capability_success,
        (
            "expect_and_answer_consensus_signer_qualification(",
            "&mut stream, 1, revision, policy_digest",
            "assert_eq!(attest.request_id, 2)",
            "OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1",
            "ParliamentTleCapabilityAttestResultWireV1",
            "&mut stream, 3, revision, policy_digest",
            ".attest_projected_capability(&session, 1)",
            "attestation.matches(&session, 1)",
        ),
    )
    first_qualification = broker_typed_capability_success.find(
        "expect_and_answer_consensus_signer_qualification("
    )
    capability_lookup = broker_typed_capability_success.find(
        "let attest = read_consensus_signer_operation("
    )
    second_qualification = broker_typed_capability_success.rfind(
        "expect_and_answer_consensus_signer_qualification("
    )
    if not first_qualification < capability_lookup < second_qualification:
        raise RuntimeError(
            f"{broker_tests_path}: typed TLE capability lookup is not surrounded by live qualification"
        )
    broker_typed_capability_failures = section(
        broker_tests,
        "enum ParliamentTleCapabilityResultFault {",
        "fn parliament_tle_partial_release_reconnects_after_broker_restart()",
        broker_tests_path,
    )
    require_all(
        broker_tests_path,
        broker_typed_capability_failures,
        (
            "WrongKeySessionId",
            "WrongTranscriptHash",
            "WrongParticipantIndex",
            "Truncated",
            "result.key_session_id = if candidate == result.key_session_id",
            "result.transcript_hash[0] ^= 1",
            "result.participant_index = 2",
            ".pop()",
            "fn correlated_wrong_tle_key_session_id_is_rejected_by_typed_proxy()",
            "fn correlated_wrong_tle_transcript_hash_is_rejected_by_typed_proxy()",
            "fn correlated_wrong_tle_participant_index_is_rejected_by_typed_proxy()",
            "fn correlated_truncated_tle_capability_is_rejected_by_typed_proxy()",
            "an invalid capability must permanently poison the TLE session without replay",
        ),
    )
    daemon_path = "crates/irohad/src/main.rs"
    daemon = read(daemon_path)
    require_all(
        daemon_path,
        daemon,
        (
            "runtime_deps.parliament_tle_release_coordinator()",
            ".with_parliament_tle_release_coordinator(parliament_tle_release_coordinator)",
        ),
    )

    model_path = "formal/sora_parliament/SoraParliamentV1.tla"
    model = read(model_path)
    require_all(
        model_path,
        model,
        (
            "FuturePulseSortition ==",
            "SortitionPulseDelayBlocks",
            "MaxSortitionRetries",
            "MaxRandomnessRedraws",
            "governanceAttemptSequence",
            "randomnessRedrawsBeforeAttempt",
            "InitialSortitionRedrawCost ==",
            "ProposalRandomnessRedrawsUsed ==",
            "ProposalWideRandomnessRedrawBudget ==",
            "sortitionPulseHeight' = height + SortitionPulseDelayBlocks",
            "FailSortitionPulseUnavailable ==",
            "RetryInitialSortitionBatch ==",
            "ReplayCommittedTransportIdempotently ==",
            "RecordInitialHiddenSortitionCapacityFailure(candidateCount) ==",
            "RecordRetryHiddenSortitionCapacityFailure(candidateCount) ==",
            '"HiddenElectorateCapacityUnavailable"',
            "sortitionCandidateCount",
            "sortitionPulseConsumed",
            "HiddenElectorateCapacityConsumesNoPulse ==",
            'sortitionFailureKind\' = "PulseUnavailable"',
            "sortitionSequence < MaxSortitionRetries",
            "supersededSortitionAttempts' = supersededSortitionAttempts + 1",
            "ObjectiveBoundedSortitionRetries ==",
            "AdmitTimedOvnResourceReservation(candidate) ==",
            "RejectTimedOvnResourceReservation(candidate) ==",
            "ReleaseTimedOvnResourceReservation(candidate) ==",
            "TimedOvnReservationSafety ==",
            "RejectedReservationDoesNotLeak ==",
            "TimedOvnReservationAuditShape ==",
            "reservationAuditStep = 8",
            "SimultaneousInitialDraw ==",
            "RecordSelfAbsence(assignment) ==",
            'IF findingState = "AwaitingReflection"',
            "ELSE height <= findingDeadlineHeight",
            "EndorsePublicFinding(assignment, root) ==",
            "EnterPublicFindingReflection ==",
            "FailPublicFindingNoResult ==",
            "AuthorityBoundImmutableMemberRecords ==",
            "PublicFindingQuorumBinding ==",
            "FindingQuorumUnreachable(absent, endorsements) ==",
            'findingFailureKind\' = "QuorumUnreachable"',
            'findingFailureKind\' = "DeadlineExpired"',
            "PublicFindingQuorum",
            "AssignmentOrder",
            "CanonicalEndorserSequence",
            "certificateFindingEndorsementRoot",
            "certificateFindingEndorsingAssignments",
            "certificateFindingEndorsementCount",
            "certificateFindingQuorum",
            "ExactPhaseBoundaries ==",
            "PhaseCapacity ==",
            "RegistrationBlocks >= MaxCorpusEntries + 1",
            "SurvivorBlocks >= MaxCorpusEntries",
            "CommitmentBlocks * 32 >= MaxCorpusEntries",
            "FreezeCommitmentInWindow ==",
            "height > survivorFreezeHeight",
            "height <= commitmentCloseHeight",
            "commitmentClosedAt > survivorFreezeHeight",
            "commitmentClosedAt <= commitmentCloseHeight",
            "ExactPublicFindingDeadline ==",
            "ObjectiveReleaseAvailability ==",
            "OpeningBlocks",
            "BoundedOpeningWindow ==",
            "FreshRetrySessions ==",
            "NoResultTerminalization ==",
            "NoPlaintextOrFallback ==",
            "CertificateBindsApprovedResult ==",
            "ExactHeightCasEnactment ==",
            "CertifiedCannotPassDueHeight ==",
            '"ExecutionFailed"',
            "FinalizeAggregateApprovedAndCertify ==",
            "FinalizeNarrowPolicyCapacityNoResult(eligibleCount) ==",
            "FinalizeNarrowPolicyRandomnessRedrawBudgetExhausted ==",
            "ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted",
            "FinalizeNarrowPolicyAndRegisterConfirmationRequest ==",
            "AtomicPolicyConfirmationCapacity ==",
            "RecordInternalExecutionFailureAtExactHeight ==",
            "~releasePulseKnown",
        ),
    )
    redraw_accounting_model = section(
        model,
        "InitialSortitionRedrawCost ==",
        "PublicFindingQuorum ==",
        model_path,
    )
    require_all(
        model_path,
        redraw_accounting_model,
        (
            "BoolToNat(governanceAttemptSequence > 0)",
            'IF sortitionState = "None"',
            "sortitionSequence + InitialSortitionRedrawCost",
            'IF ballotState = "None" THEN 0 ELSE ballotSequence',
            "randomnessRedrawsBeforeAttempt +",
            "BoolToNat(confirmationRequestCommitted)",
        ),
    )
    redraw_init_model = section(model, "Init ==", "FindingLifecycleFrame ==", model_path)
    require_all(
        model_path,
        redraw_init_model,
        (
            "governanceAttemptSequence = 0",
            "randomnessRedrawsBeforeAttempt = 0",
            "governanceAttemptSequence = 1",
            "0..(MaxRandomnessRedraws - 1)",
        ),
    )
    redraw_budget_invariant = section(
        model,
        "ProposalWideRandomnessRedrawBudget ==",
        "FuturePulseSortition ==",
        model_path,
    )
    require_all(
        model_path,
        redraw_budget_invariant,
        (
            "ProposalRandomnessRedrawsUsed \\in 0..MaxRandomnessRedraws",
            "randomnessRedrawsBeforeAttempt < MaxRandomnessRedraws",
            'sortitionState = "NoRoster"',
            'ballotState = "NoResult"',
            'attemptStatus = "Rejected"',
        ),
    )
    initial_sortition_action = section(
        model,
        "CommitInitialSortitionBatch ==",
        "RecordInitialHiddenSortitionCapacityFailure(candidateCount) ==",
        model_path,
    )
    require_all(
        model_path,
        initial_sortition_action,
        (
            'sortitionState = "None"',
            "ProposalRandomnessRedrawsUsed + InitialSortitionRedrawCost <=",
            "MaxRandomnessRedraws",
        ),
    )
    sortition_retry_action = section(
        model,
        "RetryInitialSortitionBatch ==",
        "RecordRetryHiddenSortitionCapacityFailure(candidateCount) ==",
        model_path,
    )
    require_all(
        model_path,
        sortition_retry_action,
        (
            "ProposalRandomnessRedrawsUsed < MaxRandomnessRedraws",
            "sortitionSequence' = sortitionSequence + 1",
        ),
    )
    ballot_registration_action = section(
        model,
        "RegisterPrivateBallot ==",
        "CloseRegistrationAtBoundary ==",
        model_path,
    )
    require_all(
        model_path,
        ballot_registration_action,
        (
            'ballotState \\in {"None", "NoResult"}',
            'ballotState = "None" \\/',
            "ProposalRandomnessRedrawsUsed < MaxRandomnessRedraws",
            'ballotSequence\' = IF ballotState = "None" THEN 0 ELSE ballotSequence + 1',
        ),
    )
    transport_replay_action = section(
        model,
        "ReplayCommittedTransportIdempotently ==",
        "ReducerNext ==",
        model_path,
    )
    require_all(
        model_path,
        transport_replay_action,
        (
            "sortitionState",
            "ballotState",
            "confirmationRequestCommitted",
            "UNCHANGED vars",
        ),
    )
    for forbidden_update in (
        "sortitionSequence'",
        "ballotSequence'",
        "confirmationRequestCommitted'",
        "randomnessRedrawsBeforeAttempt'",
    ):
        if forbidden_update in transport_replay_action:
            raise RuntimeError(
                f"{model_path}: committed transport replay mutates {forbidden_update!r}"
            )
    for start, end in (
        (
            "RecordInitialHiddenSortitionCapacityFailure(candidateCount) ==",
            "RevealSortitionPulse ==",
        ),
        (
            "RecordRetryHiddenSortitionCapacityFailure(candidateCount) ==",
            "SealInvitationRosters ==",
        ),
    ):
        capacity_action = section(model, start, end, model_path)
        require_all(
            model_path,
            capacity_action,
            (
                "candidateCount \\in 0..1",
                'sortitionFailureKind\' = "HiddenElectorateCapacityUnavailable"',
                "sortitionFailureHeight' = height",
                "requestHeight' = height",
                "sortitionPulseKnown' = FALSE",
                "sortitionPulseConsumed' = FALSE",
                "sortitionCandidateCount' = candidateCount",
            ),
        )
        if "sortitionPulseConsumed' = TRUE" in capacity_action:
            raise RuntimeError(
                f"{model_path}: hidden-electorate capacity failure consumes a pulse"
            )
    confirmation_capacity_action = section(
        model,
        "FinalizeNarrowPolicyCapacityNoResult(eligibleCount) ==",
        "FinalizeNarrowPolicyRandomnessRedrawBudgetExhausted ==",
        model_path,
    )
    confirmation_redraw_exhaustion_action = section(
        model,
        "FinalizeNarrowPolicyRandomnessRedrawBudgetExhausted ==",
        "FinalizeNarrowPolicyAndRegisterConfirmationRequest ==",
        model_path,
    )
    require_all(
        model_path,
        confirmation_redraw_exhaustion_action,
        (
            "ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws",
            'ballotState\' = "NoResult"',
            'attemptStatus\' = "Rejected"',
            "eligibleConfirmationCandidates' = 2",
            "policyBindingCommitted' = FALSE",
            "confirmationRequirementCommitted' = FALSE",
            "confirmationRequestCommitted' = FALSE",
            "confirmationRequestHeight' = None",
            "confirmationPulseHeight' = None",
        ),
    )
    require_all(
        model_path,
        confirmation_capacity_action,
        (
            "eligibleCount \\in 0..1",
            'ballotState\' = "NoResult"',
            'attemptStatus\' = "Rejected"',
            "policyBindingCommitted' = FALSE",
            "confirmationRequirementCommitted' = FALSE",
            "confirmationRequestCommitted' = FALSE",
            "confirmationRequestHeight' = None",
            "confirmationPulseHeight' = None",
        ),
    )
    confirmation_handoff_action = section(
        model,
        "FinalizeNarrowPolicyAndRegisterConfirmationRequest ==",
        "FinalizeAggregateRejected ==",
        model_path,
    )
    require_all(
        model_path,
        confirmation_handoff_action,
        (
            "eligibleConfirmationCandidates' = 2",
            "policyResultHeight' = height",
            "policyBindingCommitted' = TRUE",
            "confirmationRequirementCommitted' = TRUE",
            "confirmationRequestCommitted' = TRUE",
            "confirmationRequestHeight' = height",
            "confirmationPulseHeight' = height + SortitionPulseDelayBlocks",
            "ProposalRandomnessRedrawsUsed < MaxRandomnessRedraws",
        ),
    )
    if "ConstructCertificate ==" in model:
        raise RuntimeError(
            f"{model_path}: certificate construction remains a separately schedulable action"
        )
    model_config_path = "formal/sora_parliament/SoraParliamentV1.cfg"
    model_config = read(model_config_path)
    require_all(
        model_config_path,
        model_config,
        (
            "SortitionPulseDelayBlocks = 1",
            "MaxSortitionRetries = 2",
            "MaxRandomnessRedraws = 2",
            "MaxConcurrentReservations = 2",
            "ReservationIds = {Reservation0, Reservation1, Reservation2}",
            "FirstConflictingReservation = Reservation0",
            "SecondConflictingReservation = Reservation1",
            "OpeningBlocks = 2",
            "RegistrationBlocks = 3",
            "SurvivorBlocks = 2",
            "CommitmentBlocks = 2",
            "MaxCorpusEntries = 2",
            "FindingBlocks = 2",
            "SeatedAssignments = {Seat0, Seat1}",
            "FirstAssignment = Seat0",
            "SecondAssignment = Seat1",
            "FindingRoots = {Finding0, Finding1}",
            "ObjectiveBoundedSortitionRetries",
            "ProposalWideRandomnessRedrawBudget",
            "HiddenElectorateCapacityConsumesNoPulse",
            "TimedOvnReservationSafety",
            "RejectedReservationDoesNotLeak",
            "TimedOvnReservationAuditShape",
            "AuthorityBoundImmutableMemberRecords",
            "PublicFindingQuorumBinding",
            "ExactPublicFindingDeadline",
            "PhaseCapacity",
            "BoundedOpeningWindow",
            "AtomicPolicyConfirmationCapacity",
            "CertificateBindsApprovedResult",
            "ExactHeightCasEnactment",
            "NoResultTerminalization",
        ),
    )
    expected_invariants = (
        "TypeOK",
        "ProposalWideRandomnessRedrawBudget",
        "FuturePulseSortition",
        "ObjectiveBoundedSortitionRetries",
        "HiddenElectorateCapacityConsumesNoPulse",
        "TimedOvnReservationSafety",
        "RejectedReservationDoesNotLeak",
        "TimedOvnReservationAuditShape",
        "SimultaneousInitialDraw",
        "AuthorityBoundImmutableMemberRecords",
        "PublicFindingQuorumBinding",
        "ExactPublicFindingDeadline",
        "ExactBallotSchedule",
        "PhaseCapacity",
        "ExactPhaseBoundaries",
        "ObjectiveReleaseAvailability",
        "BoundedOpeningWindow",
        "FreshRetrySessions",
        "AtomicPolicyConfirmationCapacity",
        "NoPlaintextOrFallback",
        "CertificateBindsApprovedResult",
        "ExactHeightCasEnactment",
        "CertifiedCannotPassDueHeight",
        "NoResultTerminalization",
    )
    configured_invariants = tuple(
        line.strip()
        for line in section(
            model_config,
            "INVARIANTS\n",
            "\nCHECK_DEADLOCK FALSE",
            model_config_path,
        ).splitlines()
        if line.strip()
    )
    if configured_invariants != expected_invariants:
        raise RuntimeError(
            f"{model_config_path}: invariant block mismatch: "
            f"expected {expected_invariants!r}, found {configured_invariants!r}"
        )
    for invariant in expected_invariants:
        declaration_count = len(
            re.findall(rf"(?m)^{re.escape(invariant)}[ \t]*==", model)
        )
        if declaration_count != 1:
            raise RuntimeError(
                f"{model_path}: invariant {invariant!r} must be declared exactly once; "
                f"found {declaration_count}"
            )

    for declaration in ("SPECIFICATION Spec", "INVARIANTS", "CHECK_DEADLOCK FALSE"):
        declaration_count = model_config.count(declaration)
        if declaration_count != 1:
            raise RuntimeError(
                f"{model_config_path}: {declaration!r} must appear exactly once; "
                f"found {declaration_count}"
            )

    workflow_path = ".github/workflows/pr.yml"
    workflow = read(workflow_path)
    formal_job = section(
        workflow,
        "  sumeragi_formal:\n",
        "\n  nexus_cross_dataspace_localnet:\n",
        workflow_path,
    )
    require_all(
        workflow_path,
        formal_job,
        (
            '"$invocation_root/artifacts/formal/sora_parliament/inputs"',
            "SORA_PARLIAMENT_FORMAL_EVIDENCE_DIR=%s",
            'install -m 600 -- "$model_source" "$model_input"',
            'install -m 600 -- "$config_source" "$config_input"',
            '"schema": "iroha.sora_parliament.formal_run.v2"',
            '"source_commit": source_commit',
            '"successful": source_status == 0 and model_status == 0',
            '"jar_sha256": digest(jar_name)',
            '**evidence(model_name, "inputs/SoraParliamentV1.tla")',
            '**evidence(config_name, "inputs/SoraParliamentV1.cfg")',
            '"size_bytes": item.stat().st_size',
            'source_status_evidence["exit_status"] = source_status',
            'model_status_evidence["exit_status"] = model_status',
            '2>&1 | tee "$source_contract_log"',
            '-config "$config_input"',
            '"$model_input" 2>&1 | tee "$tlc_log"',
            'printf \'%s\\n\' "$tlc_status" > "$tlc_status_path"',
            "Validate SORA Parliament formal evidence closure",
            'if document.get("source_commit") != expected_commit:',
            'if entry.get("sha256") != hashlib.sha256(payload).hexdigest():',
            "name: sora-parliament-formal-pr",
            "path: ${{ steps.formal_layout.outputs.artifact_root }}/formal/sora_parliament",
            "if-no-files-found: error",
        ),
    )
    for status_capture in (
        'source_contract_status="${PIPESTATUS[0]}"',
        'tlc_status="${PIPESTATUS[0]}"',
    ):
        if formal_job.count(status_capture) != 1:
            raise RuntimeError(
                f"{workflow_path}: Parliament formal job must contain exactly one "
                f"{status_capture!r}"
            )
    if formal_job.count("name: sora-parliament-formal-pr") != 1:
        raise RuntimeError(
            f"{workflow_path}: Parliament formal artifact name must appear exactly once"
        )

    for spec_path in ("specs/governance_pipeline.md", "specs/governance_api.md"):
        spec = read(spec_path)
        folded = " ".join(spec.casefold().split())
        for retired in (
            "proposal-time JIT",
            "proposal-time Parliament snapshot",
            "Proposal-backed PLAIN",
        ):
            if retired.casefold() in folded:
                raise RuntimeError(f"{spec_path}: retired Parliament wording {retired!r}")
        for required in (
            "rollback-isolated",
            "ExecutionFailed",
            "ParliamentAutomaticExecutionOutcomeV1",
            "opening_phase_blocks",
            "parliament_sortition_pulse_delay_blocks",
            "ConsumeSortitionPulseBatch",
            "BeginInvitationAcceptance",
            "FailBodyElectionNoRoster",
            "SealBodyRoster",
            "RecordAttemptAbsence",
            "EndorsePublicFinding",
            "ceil(2 * original_seats / 3)",
            "endorsing_assignments",
            "parliament_public_finding_phase_blocks",
            "FailPublicFindingNoResult",
            "ParliamentNoResultKindV1",
            "not yet an operationally automatic",
        ):
            if required.casefold() not in folded:
                raise RuntimeError(
                    f"{spec_path}: missing automatic execution contract wording {required!r}"
                )

    telemetry_spec_path = "specs/telemetry.md"
    telemetry_spec = read(telemetry_spec_path)
    require_all(
        telemetry_spec_path,
        telemetry_spec,
        (
            "governance_parliament_transitions_total{transition}",
            "governance_parliament_no_result_total{class}",
            "governance_parliament_attempts_by_status{status}",
            "governance_parliament_attempts_by_stage{stage}",
            "ParliamentNoResultKindV1",
            "public_finding_quorum_unreachable",
            "public_finding_deadline_expired",
            "sortition_retries_exhausted",
            "confirmation_jury_capacity_unavailable",
            "never use proposal, governance-attempt, body, ballot, assignment, pulse,",
        ),
    )

    model_readme_path = "formal/sora_parliament/README.md"
    model_readme = " ".join(read(model_readme_path).split())
    require_all(
        model_readme_path,
        model_readme,
        (
            "mathematically irreversible splits",
            "permissionless caller eventually submitting the deadline trigger",
            "post-deadline non-response rejection",
            "empty or singleton live electorate",
            "without revealing or consuming a pulse",
            "Policy binding, Confirmation requirement, and Confirmation request all",
            "same transition commits the Policy binding and Confirmation requirement",
            "one proposal-wide redraw budget",
            "successor governance attempt's first sortition",
            "Exact request/session transport replays are state-idempotent",
            "required Confirmation draw at an already exhausted ceiling fails closed",
        ),
    )
    pipeline_spec = read("specs/governance_pipeline.md")
    require_all(
        "specs/governance_pipeline.md",
        pipeline_spec,
        (
            "Coercion-Resistant Voting via Anamorphic",
            "10.1145/3750555.3811888",
            "Timed OVN neither implements nor",
        ),
    )

    print("SORA Parliament source/model contract: ok")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
