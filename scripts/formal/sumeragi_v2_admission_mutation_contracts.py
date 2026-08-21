"""Bounded Sumeragi v2 admission and recovery mutation source contracts.

This internal component owns immutable inventories and source-fidelity checks
used by ``check_sumeragi_v2_proof_ledger.py``.  It has no CLI and performs no
work at import time.  The proof-ledger checker binds its reviewed Rust parsing
helpers with :func:`bind_checker` before invoking any check.
"""

from __future__ import annotations

import re
import shlex
from collections.abc import Mapping
from pathlib import Path
from typing import Any


ROOT_DIR = Path(__file__).resolve().parents[2]
FORMAL_DIR = ROOT_DIR / "formal" / "sumeragi_v2"

# Exact source seal for the finite evidence-bearing post-apply storage
# admission matrix. The model checks only its bounded mutation corpus; the
# broader two-phase storage retry regression is pinned separately below.
APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2AppliedPhaseAdmissionMutation.tla",
    "applied_phase_admission_fixed.cfg",
    "applied_phase_conflicting_evidence_coalesced_bug.cfg",
    "applied_phase_malformed_callback_stale_tag_hidden_bug.cfg",
    "applied_phase_post_apply_ordinal_bug.cfg",
    "applied_phase_post_apply_owner_bug.cfg",
    "applied_phase_stale_tag_admitted_bug.cfg",
)
APPLIED_PHASE_ADMISSION_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh"
)
APPLIED_PHASE_ADMISSION_MUTATION_SHA256 = {
    "SumeragiV2AppliedPhaseAdmissionMutation.tla": (
        "8926d782609f76e98125283cec242639c7e311e612a2a5e0668a5d3b8161d4f5"
    ),
    "applied_phase_admission_fixed.cfg": (
        "956d1353a9adad2074e0cd3c75b25469fabd0c37516decd18f913f5366d80cf6"
    ),
    "applied_phase_conflicting_evidence_coalesced_bug.cfg": (
        "8d5ba75979f9b4e60f6da10cebd16f5e9fe493fc8e4a607fab8df3e4d46500f5"
    ),
    "applied_phase_malformed_callback_stale_tag_hidden_bug.cfg": (
        "c794391953f27a2257f8d6050c0c11c5aaa8f0f4984afdab0a99fcb18ef4fca7"
    ),
    "applied_phase_post_apply_ordinal_bug.cfg": (
        "5f3cb819809fbf8a7d8138ab8d3f98dee10ac4271b57298472782f4be8386fb3"
    ),
    "applied_phase_post_apply_owner_bug.cfg": (
        "fd9f55a0095c25b83399d71be8320e0755d6aa014d1a8854035de4151149ea8c"
    ),
    "applied_phase_stale_tag_admitted_bug.cfg": (
        "88ded7c0489d2296bb9b79c8b46668c3e91cdc18c8822649352f18df46a7c538"
    ),
    APPLIED_PHASE_ADMISSION_MUTATION_RUNNER: (
        "d163bc19b62ed9f23e4f08433a7bb8080407cb8d31b9666ccf5c5d9491034ca6"
    ),
}
APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2AppliedPhaseAdmission*.tla",
    "applied_phase_*.cfg",
)
_APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256 = {
    "preflight_runtime_command_admission": (
        "ebe763984c85e4a2a2ad793d026b62c197646a588cefe35afe35ae17c6d137f6"
    ),
    "command_admission_preflight": (
        "398986e713372c50c17f663d5cbf4b7ff12c194a3cf7bcb8850a7292f8dc529a"
    ),
    "serialized_runtime_enqueue": (
        "93cda9fc1a3560ffea3337b05540077c111bf60c4417879cae8dbbfeecad4407"
    ),
    "enqueue_with_lifecycle_owner": (
        "2f1bd74df1dd29f195bccd664d8b8a0b96155264cd787f9c924eeb5a9fc5f821"
    ),
    "applied_phase_test": (
        "ee0545b0d25db2ac8abcaeed803e11181b99c54fe8c6073a773477674ed8ba48"
    ),
    "busy_owner_test": (
        "1952e16907dcd43b199cb74f2c3775ffc7a58fb4d94a069b8f941cb4c35306a2"
    ),
}

# Exact source seal for the finite durable Validate scheduler/worker lifecycle
# matrix. The model and its five mutants are bounded TLC evidence only.
DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2DurableValidateLifecycleMutation.tla",
    "durable_validate_lifecycle_fixed.cfg",
    "durable_validate_lifecycle_post_fsync_continue_bug.cfg",
    "durable_validate_lifecycle_restart_replay_drop_bug.cfg",
    "durable_validate_lifecycle_sidecar_new_ordinal_bug.cfg",
    "durable_validate_lifecycle_unguarded_completion_bug.cfg",
    "durable_validate_lifecycle_unreserved_rejection_bug.cfg",
)
DURABLE_VALIDATE_LIFECYCLE_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_durable_validate_lifecycle_mutations.sh"
)
DURABLE_VALIDATE_LIFECYCLE_MUTATION_SHA256 = {
    "SumeragiV2DurableValidateLifecycleMutation.tla": (
        "83135350484686cddc36a9aec163e0f62eb9ced34172f75b681f28038474e041"
    ),
    "durable_validate_lifecycle_fixed.cfg": (
        "b057a2ff2bb132bd809c821ac1402aaec25ad647870b598f084ef54fa378916f"
    ),
    "durable_validate_lifecycle_post_fsync_continue_bug.cfg": (
        "da8ada43c120c581fd6159a559e5f6651ebce3751b5493380769d9b89f003701"
    ),
    "durable_validate_lifecycle_restart_replay_drop_bug.cfg": (
        "48afe10e12cb1018ace57806903b69793b6733f11f4ea00e47f0413168cc4165"
    ),
    "durable_validate_lifecycle_sidecar_new_ordinal_bug.cfg": (
        "5660782af6c437c7142df60e324db836b006358463db45f2d95b694718f1589a"
    ),
    "durable_validate_lifecycle_unguarded_completion_bug.cfg": (
        "255f1e1013f10576e5b29d53f1d9f0c1f0492383d114cae18ac627aba4f26477"
    ),
    "durable_validate_lifecycle_unreserved_rejection_bug.cfg": (
        "7d014c3d79b79c2adc9cffb64439ef4ceb44b4f2042ddbab0606a13e01fb2d21"
    ),
    DURABLE_VALIDATE_LIFECYCLE_MUTATION_RUNNER: (
        "43eeb61545f5c9caa5365ee97f61fed4f56cd574ac09ca70d848d41b5ca36f6e"
    ),
}
DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2DurableValidateLifecycle*.tla",
    "durable_validate_lifecycle_*.cfg",
)

# Exact source seal for the finite post-Decision timeout/TC mutation matrix.
# The matrix guards production/formal refinement seams; it remains bounded TLC
# evidence and must never promote a deductive proof-ledger obligation.
POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2PostDecisionTimeoutMutation.tla",
    "post_decision_begin_install_tc_guard_bug.cfg",
    "post_decision_begin_timeout_guard_bug.cfg",
    "post_decision_complete_timeout_guard_bug.cfg",
    "post_decision_local_timeout_successor_bug.cfg",
    "post_decision_resume_timeout_guard_bug.cfg",
    "post_decision_tc_receive_bug.cfg",
    "post_decision_tc_successor_bug.cfg",
    "post_decision_timeout_fixed.cfg",
    "post_decision_timeout_receive_bug.cfg",
    "post_decision_timeout_successor_bug.cfg",
)
POST_DECISION_TIMEOUT_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh"
)
POST_DECISION_TIMEOUT_MUTATION_SHA256 = {
    "SumeragiV2PostDecisionTimeoutMutation.tla": (
        "384128c67519a351edd23b7fe01ba5e67439b1dfa2ab9b8b5dfa3db703f94b61"
    ),
    "post_decision_begin_install_tc_guard_bug.cfg": (
        "815e41cd50d0078be2590574eb9f044d8aff8f808d93d17df4f723f1d1ccf018"
    ),
    "post_decision_begin_timeout_guard_bug.cfg": (
        "1a640160d6eef841c7b9daf67bc19adec07b8119e256d320a10c65760d3bc971"
    ),
    "post_decision_complete_timeout_guard_bug.cfg": (
        "c89abe5f7002a75398ab629c29d44bf1ecaabdf88d89e699b09c390126350cf6"
    ),
    "post_decision_local_timeout_successor_bug.cfg": (
        "664cc1b0835249bf00c9c11850ca71ec1f44d535521b94c1255639e33c3d6f8d"
    ),
    "post_decision_resume_timeout_guard_bug.cfg": (
        "ca0829121c8670f62dde0e45c66d3f25a29823e11c2a043ad406ab436e3eb471"
    ),
    "post_decision_tc_receive_bug.cfg": (
        "617949a3724958bf59159bdc68c72a80ee8923cc05beccbe8d93001691ebfcdd"
    ),
    "post_decision_tc_successor_bug.cfg": (
        "ff012051c21e24f9cdddf0195aa8aaa04a77ba74dc3230bf940569c9fcbcd4cf"
    ),
    "post_decision_timeout_fixed.cfg": (
        "f609d628853ce714bea15f8a0311cccf4c01cc3984fb9f1ae2112ca2ed44ee73"
    ),
    "post_decision_timeout_receive_bug.cfg": (
        "5570219ab51b8e1fea28c45c18115466130c7c656f090f96df74f1fa5caf3c1d"
    ),
    "post_decision_timeout_successor_bug.cfg": (
        "a045606b2b34d755ab9e5fff0b486562a2193a2085ef24dc7252a55bfb2d9e3d"
    ),
    POST_DECISION_TIMEOUT_MUTATION_RUNNER: (
        "ff5d34f872f1250f98e11945781cc2cc25d17c3870d7fb415d0636dab7a76c1b"
    ),
}

# Exact source seal for the finite certified-response registration matrix.
# This corpus distinguishes requested certified-body responses from valid but
# unsolicited or replayed responses.  It is bounded regression evidence only.
CERTIFIED_RESPONSE_REGISTRATION_FORMAL_ARTIFACTS = (
    "SumeragiV2CertifiedResponseRegistrationMutation.tla",
    "certified_response_registration_duplicate_fixed.cfg",
    "certified_response_registration_commit_fanout_fixed.cfg",
    "certified_response_registration_duplicate_missing_guard.cfg",
    "certified_response_registration_historical_fixed.cfg",
    "certified_response_registration_restart_fixed.cfg",
    "certified_response_registration_commit_fanout_route_only_bug.cfg",
    "certified_response_registration_restart_missing_guard.cfg",
)
CERTIFIED_RESPONSE_REGISTRATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh"
)
CERTIFIED_RESPONSE_REGISTRATION_SHA256 = {
    "SumeragiV2CertifiedResponseRegistrationMutation.tla": (
        "748ba311a119e3c925f22474a16f04df554c4d5d91be57fa4fb6dbe1e01e923e"
    ),
    "certified_response_registration_duplicate_fixed.cfg": (
        "a8fb966b97bc7c4398a8ed49f23417a82369252802fe6b8ce1c4b28b886d3d71"
    ),
    "certified_response_registration_commit_fanout_fixed.cfg": (
        "761195e50a0d09b9ef88563f70f6ca44c130f420d061363b31766155aeb18bac"
    ),
    "certified_response_registration_duplicate_missing_guard.cfg": (
        "d3a76a8c91bf425d20d79eff29863d5e116a22c8fde79b98dc1f2c711d5b350d"
    ),
    "certified_response_registration_historical_fixed.cfg": (
        "edd47670a5e7badb402095ed668b41b4582523997a2a0d7d9cb826e4324963cc"
    ),
    "certified_response_registration_restart_fixed.cfg": (
        "5490a870ba6902d211b92f6bbc12258ace06239c09cd71b2581564554d7f7cdc"
    ),
    "certified_response_registration_commit_fanout_route_only_bug.cfg": (
        "364d236e0751f878704aafb32e72d09704a844061ba21ecbfa29bff5c277c1f6"
    ),
    "certified_response_registration_restart_missing_guard.cfg": (
        "bd71a1ba2684f95127d4244eb9b45ab63f1c982374c4b258318368f8611df0fe"
    ),
    CERTIFIED_RESPONSE_REGISTRATION_RUNNER: (
        "97f30ef35ce90e5a67193da0cdbdcc13c3de2f1c2535fd50ecdd081a697313a5"
    ),
}

_REQUIRED_CHECKER_BINDINGS = (
    "_read_reviewed_rust_source",
    "_require_rust_item",
    "_require_rust_item_context",
    "_require_rust_item_token_sha256",
    "_require_rust_token_sequence",
    "_sha256_file",
    "_token_sequence_count",
    "rust_code_tokens",
    "rust_items",
)


def bind_checker(namespace: Mapping[str, Any]) -> None:
    """Bind the proof-ledger checker's reviewed Rust parsing helpers."""

    missing = [
        name for name in _REQUIRED_CHECKER_BINDINGS if name not in namespace
    ]
    if missing:
        raise RuntimeError(
            f"admission mutation checker bindings are incomplete: {missing}"
        )
    globals().update(
        {name: namespace[name] for name in _REQUIRED_CHECKER_BINDINGS}
    )


def _applied_phase_admission_mutation_runner_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Pin all bounded applied-phase admission outcomes and witnesses."""

    path = repo_root / APPLIED_PHASE_ADMISSION_MUTATION_RUNNER
    if not path.is_file() or path.is_symlink():
        return [
            f"{path}: applied-phase admission runner must be a regular file"
        ]

    source = path.read_text(encoding="utf-8")
    normalized_source = re.sub(r"[ \t]*\\\r?\n[ \t]*", " ", source)
    errors: list[str] = []
    for contract, description in (
        (
            'readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"',
            "repository-relative formal directory",
        ),
        (
            '-config "$config" SumeragiV2AppliedPhaseAdmissionMutation.tla',
            "sealed applied-phase model invocation",
        ),
        ("if (($#)); then", "argument rejection"),
        (
            "evidence-bearing BodyStored callbacks suppress exact applied retries before ordinal allocation",
            "evidence-bearing post-apply suppression summary",
        ),
        (
            "their Busy retries retain one owner; storage conflicts and malformed callbacks fail closed",
            "scoped Busy/conflict/callback summary",
        ),
        (
            "malformed-plus-stale callbacks reject before well-formed stale callbacks coalesce marker-free",
            "malformed/stale ordering summary",
        ),
    ):
        if source.count(contract) != 1:
            errors.append(
                f"{path}: runner must contain exactly one {description}"
            )

    expected = {
        "applied_phase_admission_fixed.cfg": (
            "applied-phase-admission-fixed",
            0,
            (
                "TLC2 Version 2.19",
                "Model checking completed. No error has been found.",
            ),
        ),
        "applied_phase_post_apply_ordinal_bug.cfg": (
            "post-apply-ordinal-allocation-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant AppliedExactRetryPreservesOrdinal is violated.",
                "<SuppressExactAppliedRetry",
            ),
        ),
        "applied_phase_post_apply_owner_bug.cfg": (
            "post-apply-physical-owner-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant AppliedPhaseHasNoPhysicalOwner is violated.",
                "<ApplyOwnedCallback",
            ),
        ),
        "applied_phase_conflicting_evidence_coalesced_bug.cfg": (
            "conflicting-storage-evidence-coalescing-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant ConflictingEvidenceFailsClosed is violated.",
                "<ObserveConflictingEvidence",
            ),
        ),
        "applied_phase_malformed_callback_stale_tag_hidden_bug.cfg": (
            "malformed-callback-hidden-by-stale-tag-coalescing-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant MalformedCallbackFailsClosed is violated.",
                "<ObserveMalformedCallback",
            ),
        ),
        "applied_phase_stale_tag_admitted_bug.cfg": (
            "stale-tag-admitted-as-current-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant WellFormedStaleTagIsMarkerFree is violated.",
                "<ObserveWellFormedStaleTag",
            ),
        ),
    }

    cases: list[tuple[str, str, int, tuple[str, ...]]] = []
    for line in normalized_source.splitlines():
        stripped = line.strip()
        if not stripped.startswith("run_case "):
            continue
        try:
            tokens = shlex.split(stripped, comments=False, posix=True)
        except ValueError as error:
            errors.append(f"{path}: cannot parse run_case invocation: {error}")
            continue
        if len(tokens) < 5:
            errors.append(
                f"{path}: each run_case must name label, config, status, "
                "and exact outcome markers"
            )
            continue
        label, config, status_token = tokens[1:4]
        try:
            status = int(status_token)
        except ValueError:
            errors.append(
                f"{path}: {label} has non-integer TLC status {status_token!r}"
            )
            continue
        cases.append((label, config, status, tuple(tokens[4:])))

    observed_configs = [case[1] for case in cases]
    duplicate_configs = sorted(
        config
        for config in set(observed_configs)
        if observed_configs.count(config) != 1
    )
    if (
        len(cases) != 6
        or set(observed_configs) != set(expected)
        or duplicate_configs
    ):
        errors.append(
            f"{path}: runner must execute each of the six sealed "
            "applied-phase configurations exactly once; "
            f"cases={len(cases)}, "
            f"missing={sorted(set(expected) - set(observed_configs))}, "
            f"extra={sorted(set(observed_configs) - set(expected))}, "
            f"duplicates={duplicate_configs}"
        )

    for label, config, status, markers in cases:
        contract = expected.get(config)
        if contract is None:
            continue
        expected_label, expected_status, expected_markers = contract
        if (
            label != expected_label
            or status != expected_status
            or markers != expected_markers
        ):
            errors.append(
                f"{path}: applied-phase role {config} must remain "
                f"{expected_label!r} at status {expected_status} with exact "
                f"markers {expected_markers!r}; found label={label!r}, "
                f"status={status}, markers={markers!r}"
            )

    repaired_count = sum(status == 0 for _, _, status, _ in cases)
    mutant_count = sum(status != 0 for _, _, status, _ in cases)
    if repaired_count != 1 or mutant_count != 5:
        errors.append(
            f"{path}: runner must contain exactly one repaired case and five "
            f"mutants; found repaired={repaired_count}, mutants={mutant_count}"
        )
    return errors


def _durable_validate_lifecycle_mutation_runner_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Pin the repaired durable Validate lifecycle run and five mutants."""

    path = repo_root / DURABLE_VALIDATE_LIFECYCLE_MUTATION_RUNNER
    if not path.is_file() or path.is_symlink():
        return [
            f"{path}: durable Validate lifecycle runner must be a regular file"
        ]

    source = path.read_text(encoding="utf-8")
    normalized_source = re.sub(r"[ \t]*\\\r?\n[ \t]*", " ", source)
    errors: list[str] = []
    for contract, description in (
        (
            'readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"',
            "repository-relative formal directory",
        ),
        (
            '-config "$config" SumeragiV2DurableValidateLifecycleMutation.tla',
            "sealed durable Validate lifecycle model invocation",
        ),
        ("if (($#)); then", "argument rejection"),
        (
            "durable Validate owns Ready-to-Waiting dispatch and exact guarded completion",
            "scheduler/worker/turn-driver summary",
        ),
        (
            "exact sidecar wake reuses one row and ordinal; rejected output is pre-reserved",
            "sidecar and rejection reservation summary",
        ),
        (
            "all replay origins reopen with mandatory authority; ambiguous post-fsync failure stops",
            "restart and fail-stop summary",
        ),
    ):
        if source.count(contract) != 1:
            errors.append(f"{path}: runner must contain exactly one {description}")

    expected = {
        "durable_validate_lifecycle_fixed.cfg": (
            "durable-validate-lifecycle-fixed",
            0,
            (
                "TLC2 Version 2.19",
                "Model checking completed. No error has been found.",
            ),
        ),
        "durable_validate_lifecycle_unguarded_completion_bug.cfg": (
            "unguarded-validate-completion-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant GuardedCompletionMatchesClaimedRow is violated.",
                "<WorkerReturnsGuardedCompletion",
            ),
        ),
        "durable_validate_lifecycle_sidecar_new_ordinal_bug.cfg": (
            "sidecar-new-ordinal-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant ExactSidecarWakeReusesWaitingRow is violated.",
                "<DeliverExactSidecar",
            ),
        ),
        "durable_validate_lifecycle_unreserved_rejection_bug.cfg": (
            "unreserved-rejection-output-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant RejectedResultClaimHasReservedOutput is violated.",
                "<ClaimRejectedResult",
            ),
        ),
        "durable_validate_lifecycle_restart_replay_drop_bug.cfg": (
            "restart-drops-replay-authority-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant RestartReopensMandatoryReplayAuthority is violated.",
                "<CrashAndReopenWaiting",
            ),
        ),
        "durable_validate_lifecycle_post_fsync_continue_bug.cfg": (
            "ambiguous-post-fsync-continues-mutant",
            12,
            (
                "TLC2 Version 2.19",
                "Invariant AmbiguousPostFsyncRequiresFailStop is violated.",
                "<ObserveAmbiguousPostFsync",
            ),
        ),
    }

    cases: list[tuple[str, str, int, tuple[str, ...]]] = []
    for line in normalized_source.splitlines():
        stripped = line.strip()
        if not stripped.startswith("run_case "):
            continue
        try:
            tokens = shlex.split(stripped, comments=False, posix=True)
        except ValueError as error:
            errors.append(f"{path}: cannot parse run_case invocation: {error}")
            continue
        if len(tokens) < 5:
            errors.append(
                f"{path}: each run_case must name label, config, status, "
                "and exact outcome markers"
            )
            continue
        label, config, status_token = tokens[1:4]
        try:
            status = int(status_token)
        except ValueError:
            errors.append(
                f"{path}: {label} has non-integer TLC status {status_token!r}"
            )
            continue
        cases.append((label, config, status, tuple(tokens[4:])))

    observed_configs = [case[1] for case in cases]
    duplicate_configs = sorted(
        config
        for config in set(observed_configs)
        if observed_configs.count(config) != 1
    )
    if (
        len(cases) != 6
        or set(observed_configs) != set(expected)
        or duplicate_configs
    ):
        errors.append(
            f"{path}: runner must execute each of the six sealed durable "
            "Validate lifecycle configurations exactly once; "
            f"cases={len(cases)}, "
            f"missing={sorted(set(expected) - set(observed_configs))}, "
            f"extra={sorted(set(observed_configs) - set(expected))}, "
            f"duplicates={duplicate_configs}"
        )

    for label, config, status, markers in cases:
        contract = expected.get(config)
        if contract is None:
            continue
        expected_label, expected_status, expected_markers = contract
        if (
            label != expected_label
            or status != expected_status
            or markers != expected_markers
        ):
            errors.append(
                f"{path}: durable Validate lifecycle role {config} must "
                f"remain {expected_label!r} at status {expected_status} with "
                f"exact markers {expected_markers!r}; found label={label!r}, "
                f"status={status}, markers={markers!r}"
            )

    repaired_count = sum(status == 0 for _, _, status, _ in cases)
    mutant_count = sum(status != 0 for _, _, status, _ in cases)
    if repaired_count != 1 or mutant_count != 5:
        errors.append(
            f"{path}: runner must contain exactly one repaired case and five "
            f"mutants; found repaired={repaired_count}, mutants={mutant_count}"
        )
    return errors


def _durable_validate_lifecycle_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind the bounded lifecycle cuts to the production scheduler seams."""

    errors: list[str] = []

    def require_sequence(relative: str, fragment: str, description: str) -> None:
        path = repo_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: {description} requires a regular production source file"
            )
            return
        observed = _token_sequence_count(
            rust_code_tokens(path.read_text(encoding="utf-8")),
            rust_code_tokens(fragment),
        )
        if observed != 1:
            errors.append(
                f"{path}: {description} must occur exactly once in real Rust "
                f"tokens; found {observed}"
            )

    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        """
let reservation = census
    .select_validate(ordinal)
    .map_err(|_| ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch)?;
let dispatch = self
    .coordinator
    .begin_durable_validate_dispatch(&mut self.registry, lease, &self.verified)
    .map_err(|_| ProductionCompletionDispatchErrorV1::DispatchProjection)?;
if !reservation.preflight(&dispatch) {
    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
}
reservation.commit(dispatch);
""",
        "Ready Validate must reserve the worker slot before its exact Waiting dispatch",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_worker_completion.rs",
        """
V2IoCommand::LifecycleValidate(task) => {
    if !task.matches_exact() {
        Err("lifecycle Validate command changed after queue publication".to_owned())
    } else {
        let key = task.key;
        task.dispatch
            .execute(
                body_store.as_mut().expect(
                    "body store remains live before Retire",
                ),
                |body| {
                    apply_service.validate_candidate(&context, body)
                },
            )
            .map(|result| {
                V2IoCompletion::LifecycleValidate(Box::new(
                    GuardedLifecycleValidateWorkerResultV1::new(
                        key,
                        result,
                        Arc::clone(&output_guard),
                    ),
                ))
            })
            .map_err(|(error, _dispatch)| error.to_string())
    }
}
""",
        "the worker must execute and return one exact guarded Validate completion",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        """
let Some(completion) =
    PendingLifecycleCompletionV1::take_validate(pending_lifecycle_completion)
else {
    services
        .lifecycle_output_guard()
        .close_admission_for_restart();
    return ProductionLifecycleCompletionSelectionV1::RestartRequired;
};
let (dispatch, ack) = completion.into_publication_parts();
match owner.coordinator.complete_durable_validate_dispatch(
    &mut owner.registry,
    dispatch,
)
""",
        "the turn driver must rejoin the guarded completion to its coordinator row",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        """
DurableValidateCompletionPublication::PublishedValidated(
    _,
)
| crate::sumeragi::v2_lifecycle_coordinator::DurableValidateCompletionPublication::PublishedRejected(
    _,
),
) => {
    ack.acknowledge_after_publication();
    ProductionLifecycleCompletionSelectionV1::LifecycleValidatePublished
}
""",
        "the exact guarded Validate owner must retire only after publication",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        """
DurableValidateCompletionPublication::DeferredMergeSidecar(
    deferred,
),
) => {
    assert!(pending_lifecycle_completion.is_none());
    *pending_lifecycle_completion = Some(
        PendingLifecycleCompletionV1::DeferredValidate(ack.bind_deferred(deferred)),
    );
""",
        "a missing sidecar must retain the exact guarded completion while Waiting",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_validate_sidecar.rs",
        """
next.publish_ready(ReadyEvent::new(
    identity.dispatch_key.lifecycle_ordinal(),
    identity.dispatch_key.owner(),
    identity.wait_token,
    None,
));
if !sidecar_wake_transition_is_exact(self, &next, identity) {
    return Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity);
}
""",
        "exact sidecar availability must wake the same Waiting row",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_validate_sidecar.rs",
        """
&& next.high_water == current.high_water
&& next.next_lease == current.next_lease
&& next.durable_records == current.durable_records
""",
        "sidecar wake must allocate neither a new row ordinal nor a new lease",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_validate_sidecar.rs",
        """
let Some(identity) = coordinator.load_validate_sidecar_registration()? else {
    return Ok(None);
};
coordinator.restore_validate_sidecar_wait(&identity, registry)?;
""",
        "restart must restore an fsynced sidecar wait before Ready selection",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        """
let expected_reservation = match outcome_kind {
    ReadyDurableValidateOutcomeKind::Validated => None,
    ReadyDurableValidateOutcomeKind::Rejected => Some(CapacityClass::Consensus),
};
if lease
    .output_reservation()
    .map(|reservation| reservation.class())
    != expected_reservation
{
    return Err(ReadyDurableValidateExecutionError::InvalidLeaseShape);
}
""",
        "rejected Validate must carry its pre-reserved report output before execution",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        """
coordinator
    .durable_records
    .get(&ordinal)
    .is_none_or(|metadata| {
        !metadata.replay_authority.structurally_matches_record(
            coordinator.active_context,
            record.key,
            record.work_class,
            record.stage,
            metadata.payload,
        )
    })
""",
        "restart coverage must reject any row without exact durable replay authority",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_pre_admission.rs",
        """
pub(super) enum PreparedLifecycleAdmissionOwnerV1 {
    LiveWal(PreparedLiveWalAdmissionV1),
    LocalBody(PreparedLocalBodyValidateReplayPreAdmission),
    RemoteProposal(PreparedRemoteProposalValidateReplayPreAdmission),
    InvalidBodyReport(BoundAdapterEffectV1),
    DirectSigned(BoundAdapterEffectV1),
}
""",
        "all five replay-authorized origins must share one closed prepared admission owner",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority_live_wal.rs",
        """
let authority = canonical_replay_authority(
    context,
    LifecycleReplaySourceV1::Wal(source),
    stage,
    ReplayPayloadBindingV1::None,
)?;
LiveWalPersistedReplayStateV1::Canonical { stage, authority }
""",
        "live-WAL replay must seal a mandatory canonical authority",
    )
    require_sequence(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
        """
if let Err(error) = coordinator.persist_exact_staged_successor(&staged) {
    coordinator.fault = Some(CoordinatorFault::DurabilityFailure);
    return Err(LiveValidateReportPublicationError {
        _coordinator: coordinator,
        _staged: staged,
        _failure: LiveValidateReportPublicationFailure::Ledger {
            _error: error,
            _publication: publication,
        },
    });
}
*coordinator = staged;
publication.publish_after_ledger_fsync();
Ok(())
""",
        "report publication must have only an infallible tail after LedgerV1 fsync",
    )
    return errors


def _applied_phase_admission_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind post-apply suppression to production before ordinal allocation."""

    adapter_path = (
        repo_root / "crates/iroha_core/src/sumeragi/v2.rs"
    )
    runtime_path = (
        repo_root / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    )
    for path, description in (
        (adapter_path, "production adapter"),
        (runtime_path, "serialized production runtime"),
    ):
        if not path.is_file() or path.is_symlink():
            return [
                f"{path}: {description} must be a regular source file for "
                "applied-phase admission refinement"
            ]

    errors: list[str] = []
    _, adapter_source = _read_reviewed_rust_source(
        repo_root,
        adapter_path.relative_to(repo_root).as_posix(),
        errors,
        "production adapter applied-phase admission source",
    )
    _, runtime_source = _read_reviewed_rust_source(
        repo_root,
        runtime_path.relative_to(repo_root).as_posix(),
        errors,
        "serialized runtime applied-phase admission source",
    )
    if errors:
        return errors
    adapter_context = (("impl", "SumeragiV2Adapter"),)
    runtime_context = (
        (
            "impl",
            "<",
            "D",
            ":",
            "RuntimeDriver",
            ">",
            "SerializedV2Runtime",
            "<",
            "D",
            ">",
        ),
    )
    test_context = (
        ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
    )

    preflight = _require_rust_item(
        adapter_path,
        adapter_source,
        "preflight_runtime_command_admission",
        errors,
    )
    _require_rust_item_context(
        adapter_path,
        preflight,
        adapter_context,
        "phase-aware callback preflight",
        errors,
    )
    _require_rust_item_token_sha256(
        adapter_path,
        preflight,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256[
            "preflight_runtime_command_admission"
        ],
        "phase-aware callback preflight",
        errors,
    )
    _require_rust_token_sequence(
        adapter_path,
        preflight,
        """
let Ok((event, completion_evidence)) = projected else {
    return Preflight::Reject;
};
if tag != self.reducer.current_tag() {
    return Preflight::Coalesce;
}
""",
        "malformed callbacks must reject before stale tags coalesce",
        errors,
    )
    for fragment, description in (
        (
            """
reducer::Event::BodyAvailable { round, subject, .. } => {
    (self.reducer.body_state(*round, *subject) != reducer::BodyState::Missing)
        .then_some(Preflight::Coalesce)
}
""",
            "BodyAvailable monotone suppression",
        ),
        (
            """
reducer::Event::BodyStored { round, subject, .. } => {
    match self.reducer.body_state(*round, *subject) {
        reducer::BodyState::Missing | reducer::BodyState::Available => None,
        reducer::BodyState::Durable
        | reducer::BodyState::Validated
        | reducer::BodyState::Invalid => Some(Preflight::Coalesce),
    }
}
""",
            "BodyStored monotone suppression",
        ),
    ):
        _require_rust_token_sequence(
            adapter_path,
            preflight,
            fragment,
            description,
            errors,
        )

    runtime_preflight = _require_rust_item(
        runtime_path,
        runtime_source,
        "command_admission_preflight",
        errors,
    )
    _require_rust_item_context(
        runtime_path,
        runtime_preflight,
        runtime_context,
        "checked command-admission preflight",
        errors,
    )
    _require_rust_item_token_sha256(
        runtime_path,
        runtime_preflight,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256[
            "command_admission_preflight"
        ],
        "checked command-admission preflight",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_preflight,
        """
RuntimeCommandAdmissionPreflight::ReuseDormant { .. }
    if class != CommandClass::Completion =>
{
    self.latch_fail_closed(
        "restart-dormant producer changed its frozen completion service class",
    );
    Err(EnqueueError::FailClosed)
}
preflight @ (RuntimeCommandAdmissionPreflight::Admit
| RuntimeCommandAdmissionPreflight::ReuseDormant { .. }
| RuntimeCommandAdmissionPreflight::Coalesce
| RuntimeCommandAdmissionPreflight::CoalesceOwned { .. }) => Ok(preflight),
RuntimeCommandAdmissionPreflight::Reject => {
    self.latch_fail_closed(
        "runtime command admission conflicted with frozen reducer authority",
    );
    Err(EnqueueError::FailClosed)
}
""",
        "admit, dormant reuse, coalesce, and reject outcomes must remain distinct",
        errors,
    )

    runtime_enqueues = [
        item
        for item in rust_items(runtime_source, "enqueue")
        if item.brace_context == runtime_context
    ]
    if len(runtime_enqueues) != 1:
        errors.append(
            f"{runtime_path}: require exactly one serialized-runtime enqueue "
            f"method; found {len(runtime_enqueues)}"
        )
        runtime_enqueue = None
    else:
        runtime_enqueue = runtime_enqueues[0]
        _require_rust_item_context(
            runtime_path,
            runtime_enqueue,
            runtime_context,
            "serialized runtime enqueue",
            errors,
        )
    _require_rust_item_token_sha256(
        runtime_path,
        runtime_enqueue,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256[
            "serialized_runtime_enqueue"
        ],
        "serialized runtime enqueue",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_enqueue,
        """
let preflight = self.command_admission_preflight(tag, class, &command)?;
let tagged = match preflight {
    RuntimeCommandAdmissionPreflight::Coalesce
    | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => return Ok(()),
    RuntimeCommandAdmissionPreflight::Admit => {
        TaggedCommand::new(tag, class, command, Instant::now())
    }
    RuntimeCommandAdmissionPreflight::ReuseDormant {
        causal_lifecycle_key,
        admission_ordinal,
        producer_stage,
    } => self.restored_tagged_command(
        tag,
        class,
        command,
        Instant::now(),
        causal_lifecycle_key,
        admission_ordinal,
        producer_stage,
    )?,
    RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
};
let result = self.enqueue_after_clock_reservation(tagged);
""",
        "preflight must coalesce or restore the exact dormant owner before "
        "physical enqueue and fresh ordinal allocation",
        errors,
    )

    owned_enqueue = _require_rust_item(
        runtime_path,
        runtime_source,
        "enqueue_with_lifecycle_owner",
        errors,
    )
    _require_rust_item_context(
        runtime_path,
        owned_enqueue,
        runtime_context,
        "causal-successor runtime enqueue",
        errors,
    )
    _require_rust_item_token_sha256(
        runtime_path,
        owned_enqueue,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256[
            "enqueue_with_lifecycle_owner"
        ],
        "causal-successor runtime enqueue",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        owned_enqueue,
        """
let preflight = self.command_admission_preflight(tag, class, &command)?;
if self.owned_preflight_is_coalesced(tag, &command, preflight, ownership)? {
    return Ok(());
}
let mut tagged = match preflight {
    RuntimeCommandAdmissionPreflight::Admit => TaggedCommand::with_causal_origin(
""",
        "owned preflight must validate coalescence or select exact dormant "
        "reuse before tagged owner construction",
        errors,
    )

    applied_test = _require_rust_item(
        runtime_path,
        runtime_source,
        "applied_body_storage_phases_suppress_retries_before_ordinal_allocation",
        errors,
    )
    _require_rust_item_context(
        runtime_path,
        applied_test,
        test_context,
        "two-phase post-apply storage suppression regression",
        errors,
        expected_attributes=("#[test]",),
    )
    _require_rust_item_token_sha256(
        runtime_path,
        applied_test,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256["applied_phase_test"],
        "two-phase post-apply storage suppression regression",
        errors,
    )
    if applied_test is not None:
        test_tokens = rust_code_tokens(applied_test.source)
        for fragment, expected_count, description in (
            (
                "assert_eq!(runtime.queued_commands(), 0);",
                2,
                "one zero-physical-owner assertion per phase",
            ),
            (
                "assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);",
                2,
                "one no-new-ordinal assertion per phase",
            ),
        ):
            observed = _token_sequence_count(
                test_tokens,
                rust_code_tokens(fragment),
            )
            if observed != expected_count:
                errors.append(
                    f"{runtime_path}:{applied_test.line}: {description} must "
                    f"appear exactly {expected_count} times; found {observed}"
                )
        for phase in (
            '"body_available"',
            '"body_stored"',
        ):
            if applied_test.source.count(phase) != 2:
                errors.append(
                    f"{runtime_path}:{applied_test.line}: phase inventory "
                    f"{phase} must appear exactly in the inventory and "
                    "observed suppression vector"
                )

    busy_test = _require_rust_item(
        runtime_path,
        runtime_source,
        "store_completion_retries_coalesce_across_ingress_and_busy_deferred_ownership",
        errors,
    )
    _require_rust_item_context(
        runtime_path,
        busy_test,
        test_context,
        "BodyStored Busy exact-owner coalescing regression",
        errors,
        expected_attributes=("#[test]",),
    )
    _require_rust_item_token_sha256(
        runtime_path,
        busy_test,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256["busy_owner_test"],
        "BodyStored Busy exact-owner coalescing regression",
        errors,
    )
    for fragment, description in (
        (
            """
DeferredBodyPipelineStageForTest::BodyStored,
""",
            "BodyStored Busy-owner witness",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            busy_test,
            fragment,
            description,
            errors,
        )
    return errors


def _applied_phase_admission_mutation_source_fidelity_errors(
    formal_dir: Path = FORMAL_DIR,
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the applied-phase model, runner, and production refinement seam."""

    errors: list[str] = []
    expected_formal = set(
        APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS
    )
    expected_all = expected_formal | {
        APPLIED_PHASE_ADMISSION_MUTATION_RUNNER
    }
    digest_names = set(APPLIED_PHASE_ADMISSION_MUTATION_SHA256)
    model_count = sum(name.endswith(".tla") for name in expected_formal)
    config_count = sum(name.endswith(".cfg") for name in expected_formal)
    if (
        len(APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS) != 7
        or model_count != 1
        or config_count != 6
    ):
        errors.append(
            "applied-phase admission source seal must name exactly one model "
            "and six configurations; found "
            f"models={model_count}, configurations={config_count}, "
            f"total={len(APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS)}"
        )
    if digest_names != expected_all:
        errors.append(
            "applied-phase admission digest inventory must equal the exact "
            f"eight-artifact corpus; missing={sorted(expected_all - digest_names)}, "
            f"extra={sorted(digest_names - expected_all)}"
        )

    observed_formal: set[str] = set()
    for pattern in APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_GLOBS:
        observed_formal.update(
            path.name for path in formal_dir.glob(pattern)
        )
    for name in sorted(expected_formal - observed_formal):
        errors.append(
            f"{formal_dir / name}: missing applied-phase admission artifact"
        )
    for name in sorted(observed_formal - expected_formal):
        errors.append(
            f"{formal_dir / name}: extra applied-phase admission artifact"
        )

    runner_dir = repo_root / "scripts" / "formal"
    observed_runners = {
        path.relative_to(repo_root).as_posix()
        for path in runner_dir.glob(
            "run_sumeragi_v2_applied_phase_admission*.sh"
        )
    }
    expected_runners = {APPLIED_PHASE_ADMISSION_MUTATION_RUNNER}
    for name in sorted(expected_runners - observed_runners):
        errors.append(
            f"{repo_root / name}: missing applied-phase admission runner"
        )
    for name in sorted(observed_runners - expected_runners):
        errors.append(
            f"{repo_root / name}: extra applied-phase admission runner"
        )

    for name, expected_sha256 in (
        APPLIED_PHASE_ADMISSION_MUTATION_SHA256.items()
    ):
        path = repo_root / name if "/" in name else formal_dir / name
        if not path.is_file() or path.is_symlink():
            if path.exists() or path.is_symlink():
                errors.append(
                    f"{path}: applied-phase admission artifact must be a "
                    "regular file"
                )
            continue
        observed_sha256 = _sha256_file(path)
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: applied-phase admission artifact must match exact "
                f"reviewed SHA-256 {expected_sha256}; found {observed_sha256}"
            )
    errors.extend(
        _applied_phase_admission_mutation_runner_errors(repo_root)
    )
    errors.extend(
        _applied_phase_admission_production_source_fidelity_errors(repo_root)
    )
    ci_path = repo_root / "ci" / "check_sumeragi_formal.sh"
    if not ci_path.is_file() or ci_path.is_symlink():
        errors.append(
            f"{ci_path}: formal CI gate must be a regular file for the "
            "applied-phase admission runner"
        )
    else:
        ci_source = ci_path.read_text(encoding="utf-8")
        invocation = (
            "run_formal_script scripts/formal/"
            "run_sumeragi_v2_applied_phase_admission_mutations.sh"
        )
        if ci_source.count(invocation) != 1:
            errors.append(
                f"{ci_path}: formal CI gate must invoke the sealed "
                "applied-phase admission runner exactly once"
            )
        effect_capacity_offset = ci_source.find(
            "run_formal_script scripts/formal/"
            "run_sumeragi_v2_effect_capacity_ownership_mutation.sh"
        )
        invocation_offset = ci_source.find(invocation)
        aggregate_tlc_offset = ci_source.find(
            "run_formal_script scripts/formal/run_sumeragi_v2_tlc.sh"
        )
        if not (
            0 <= effect_capacity_offset
            < invocation_offset
            < aggregate_tlc_offset
        ):
            errors.append(
                f"{ci_path}: applied-phase admission mutation must run after "
                "effect-capacity ownership and before aggregate TLC"
            )
    return errors


def _durable_validate_lifecycle_mutation_source_fidelity_errors(
    formal_dir: Path = FORMAL_DIR,
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the durable Validate model, runner, and production refinement."""

    errors: list[str] = []
    expected_formal = set(
        DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_ARTIFACTS
    )
    expected_all = expected_formal | {
        DURABLE_VALIDATE_LIFECYCLE_MUTATION_RUNNER
    }
    digest_names = set(DURABLE_VALIDATE_LIFECYCLE_MUTATION_SHA256)
    model_count = sum(name.endswith(".tla") for name in expected_formal)
    config_count = sum(name.endswith(".cfg") for name in expected_formal)
    if (
        len(DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_ARTIFACTS) != 7
        or model_count != 1
        or config_count != 6
    ):
        errors.append(
            "durable Validate lifecycle source seal must name exactly one "
            "model and six configurations; found "
            f"models={model_count}, configurations={config_count}, "
            "total="
            f"{len(DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_ARTIFACTS)}"
        )
    if digest_names != expected_all:
        errors.append(
            "durable Validate lifecycle digest inventory must equal the exact "
            f"eight-artifact corpus; missing={sorted(expected_all - digest_names)}, "
            f"extra={sorted(digest_names - expected_all)}"
        )

    observed_formal: set[str] = set()
    for pattern in DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_GLOBS:
        observed_formal.update(path.name for path in formal_dir.glob(pattern))
    for name in sorted(expected_formal - observed_formal):
        errors.append(
            f"{formal_dir / name}: missing durable Validate lifecycle artifact"
        )
    for name in sorted(observed_formal - expected_formal):
        errors.append(
            f"{formal_dir / name}: extra durable Validate lifecycle artifact"
        )

    runner_dir = repo_root / "scripts" / "formal"
    observed_runners = {
        path.relative_to(repo_root).as_posix()
        for path in runner_dir.glob(
            "run_sumeragi_v2_durable_validate_lifecycle*.sh"
        )
    }
    expected_runners = {DURABLE_VALIDATE_LIFECYCLE_MUTATION_RUNNER}
    for name in sorted(expected_runners - observed_runners):
        errors.append(
            f"{repo_root / name}: missing durable Validate lifecycle runner"
        )
    for name in sorted(observed_runners - expected_runners):
        errors.append(
            f"{repo_root / name}: extra durable Validate lifecycle runner"
        )

    for name, expected_sha256 in (
        DURABLE_VALIDATE_LIFECYCLE_MUTATION_SHA256.items()
    ):
        path = repo_root / name if "/" in name else formal_dir / name
        if not path.is_file() or path.is_symlink():
            if path.exists() or path.is_symlink():
                errors.append(
                    f"{path}: durable Validate lifecycle artifact must be a "
                    "regular file"
                )
            continue
        observed_sha256 = _sha256_file(path)
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: durable Validate lifecycle artifact must match exact "
                f"reviewed SHA-256 {expected_sha256}; found {observed_sha256}"
            )
    errors.extend(
        _durable_validate_lifecycle_mutation_runner_errors(repo_root)
    )
    errors.extend(
        _durable_validate_lifecycle_production_source_fidelity_errors(
            repo_root
        )
    )

    ci_path = repo_root / "ci" / "check_sumeragi_formal.sh"
    if not ci_path.is_file() or ci_path.is_symlink():
        errors.append(
            f"{ci_path}: formal CI gate must be a regular file for the durable "
            "Validate lifecycle runner"
        )
    else:
        ci_source = ci_path.read_text(encoding="utf-8")
        invocation = (
            "run_formal_script scripts/formal/"
            "run_sumeragi_v2_durable_validate_lifecycle_mutations.sh"
        )
        if ci_source.count(invocation) != 1:
            errors.append(
                f"{ci_path}: formal CI gate must invoke the sealed durable "
                "Validate lifecycle runner exactly once"
            )
        applied_offset = ci_source.find(
            "run_formal_script scripts/formal/"
            "run_sumeragi_v2_applied_phase_admission_mutations.sh"
        )
        invocation_offset = ci_source.find(invocation)
        aggregate_tlc_offset = ci_source.find(
            "run_formal_script scripts/formal/run_sumeragi_v2_tlc.sh"
        )
        if not (
            0 <= applied_offset < invocation_offset < aggregate_tlc_offset
        ):
            errors.append(
                f"{ci_path}: durable Validate lifecycle mutation must run "
                "after applied-phase admission and before aggregate TLC"
            )
    return errors


def _post_decision_timeout_mutation_source_fidelity_errors(
    formal_dir: Path = FORMAL_DIR,
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the complete bounded post-Decision timeout mutation corpus."""

    errors: list[str] = []
    expected_formal = set(POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS)
    expected_all = expected_formal | {POST_DECISION_TIMEOUT_MUTATION_RUNNER}
    digest_names = set(POST_DECISION_TIMEOUT_MUTATION_SHA256)
    model_count = sum(name.endswith(".tla") for name in expected_formal)
    config_count = sum(name.endswith(".cfg") for name in expected_formal)
    if (
        len(POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS) != 11
        or model_count != 1
        or config_count != 10
    ):
        errors.append(
            "post-Decision timeout mutation source seal must name exactly one "
            "model and ten configurations; found "
            f"models={model_count}, configurations={config_count}, "
            f"total={len(POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS)}"
        )
    if digest_names != expected_all:
        errors.append(
            "post-Decision timeout mutation digest inventory must equal the "
            f"exact 12-artifact corpus; missing={sorted(expected_all - digest_names)}, "
            f"extra={sorted(digest_names - expected_all)}"
        )

    observed_formal = {
        path.name
        for pattern in (
            "SumeragiV2PostDecision*.tla",
            "post_decision_*.cfg",
        )
        for path in formal_dir.glob(pattern)
    }
    for name in sorted(expected_formal - observed_formal):
        errors.append(
            f"{formal_dir / name}: missing post-Decision timeout mutation artifact"
        )
    for name in sorted(observed_formal - expected_formal):
        errors.append(
            f"{formal_dir / name}: extra post-Decision timeout mutation artifact"
        )

    runner_dir = repo_root / "scripts" / "formal"
    observed_runners = {
        path.relative_to(repo_root).as_posix()
        for path in runner_dir.glob(
            "run_sumeragi_v2_post_decision*_mutation.sh"
        )
    }
    expected_runners = {POST_DECISION_TIMEOUT_MUTATION_RUNNER}
    for name in sorted(expected_runners - observed_runners):
        errors.append(
            f"{repo_root / name}: missing post-Decision timeout mutation runner"
        )
    for name in sorted(observed_runners - expected_runners):
        errors.append(
            f"{repo_root / name}: extra post-Decision timeout mutation runner"
        )

    for name, expected_sha256 in POST_DECISION_TIMEOUT_MUTATION_SHA256.items():
        path = repo_root / name if "/" in name else formal_dir / name
        if not path.is_file() or path.is_symlink():
            if path.exists() or path.is_symlink():
                errors.append(
                    f"{path}: post-Decision timeout mutation artifact must be a "
                    "regular file"
                )
            continue
        observed_sha256 = _sha256_file(path)
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: post-Decision timeout mutation artifact must match "
                f"exact reviewed SHA-256 {expected_sha256}; found {observed_sha256}"
            )
    return errors
