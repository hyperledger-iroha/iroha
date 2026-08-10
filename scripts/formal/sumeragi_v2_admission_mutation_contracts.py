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

# Exact source seal for the finite evidence-bearing post-apply admission
# matrix. The model checks only its bounded mutation corpus; the broader
# four-phase exact-retry regression is pinned separately below.
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
        "8aa8f9cc994eb4f443838fffe53e99605b7d11794392f3858a284fe65e638626"
    ),
    "applied_phase_admission_fixed.cfg": (
        "255788a309416ee6addfc9e638966eb801713a46551d6e11e5216e4abc127749"
    ),
    "applied_phase_conflicting_evidence_coalesced_bug.cfg": (
        "ccbb3f471c69d4d70e119393bcfabae5edb65f28512ebd58fcc08c1d43a96f36"
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
        "381c2beba6ec3130ad1102de87dd09739022a5c36c1ead3bd6589cf45be0f7f9"
    ),
}
APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2AppliedPhaseAdmission*.tla",
    "applied_phase_*.cfg",
)
_APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256 = {
    "preflight_runtime_command_admission": (
        "a68f6905eae6cad7eb139072bd9def8fde68a67ec8e5ab7c9b001124ebc263b2"
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
        "c44468d94b67f58e5bbd8b97bd2271c030f6f337fdc71dbbd9c9b00d3f17598a"
    ),
    "busy_owner_test": (
        "6e279f42322b64360ebc6b67fe1d0a3d19e075d70db995fb708cf3396551b420"
    ),
}

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
        "9e8a9f07e50230929712a84d01abf4a75f134727a254a0fd21b57e19c93a4e8b"
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
        "166fbeb948d2e05015c6deed7c4d45e512bf9f29b8825f5feccb7608c0ef2eae"
    ),
}

_REQUIRED_CHECKER_BINDINGS = (
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
            "evidence-bearing BodyStored/ValidationSucceeded callbacks suppress exact applied retries before ordinal allocation",
            "evidence-bearing post-apply suppression summary",
        ),
        (
            "their Busy retries retain one owner; validation conflicts and malformed callbacks fail closed",
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
            "conflicting-validation-evidence-coalescing-mutant",
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

    adapter_source = adapter_path.read_text(encoding="utf-8")
    runtime_source = runtime_path.read_text(encoding="utf-8")
    errors: list[str] = []
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
        (
            """
(reducer::BodyState::Validated, true)
| (reducer::BodyState::Invalid, false)
| (reducer::BodyState::Available, _) => Some(Preflight::Coalesce),
(reducer::BodyState::Validated, false) | (reducer::BodyState::Invalid, true) => {
    Some(Preflight::Reject)
}
""",
            "validation polarity suppression and conflict rejection",
        ),
        (
            """
reducer::Event::Signed { .. } => self
    .reducer
    .awaiting_signature()
    .is_none()
    .then_some(Preflight::Coalesce)
""",
            "SignatureCompleted monotone suppression",
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
        "owned preflight must coalesce or select exact dormant reuse before "
        "tagged owner construction",
        errors,
    )

    applied_test = _require_rust_item(
        runtime_path,
        runtime_source,
        "applied_body_pipeline_phases_suppress_retries_before_ordinal_allocation",
        errors,
    )
    _require_rust_item_context(
        runtime_path,
        applied_test,
        test_context,
        "four-phase post-apply suppression regression",
        errors,
        expected_attributes=("#[test]",),
    )
    _require_rust_item_token_sha256(
        runtime_path,
        applied_test,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256["applied_phase_test"],
        "four-phase post-apply suppression regression",
        errors,
    )
    if applied_test is not None:
        test_tokens = rust_code_tokens(applied_test.source)
        for fragment, expected_count, description in (
            (
                "assert_eq!(runtime.queued_commands(), 0);",
                4,
                "one zero-physical-owner assertion per phase",
            ),
            (
                "assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);",
                4,
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
            '"validation_succeeded"',
            '"signature_completed"',
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
        "completion_retries_coalesce_across_ingress_and_busy_deferred_ownership",
        errors,
    )
    _require_rust_item_context(
        runtime_path,
        busy_test,
        test_context,
        "Busy exact-owner coalescing regression",
        errors,
        expected_attributes=("#[test]",),
    )
    _require_rust_item_token_sha256(
        runtime_path,
        busy_test,
        _APPLIED_PHASE_ADMISSION_RUST_ITEM_SHA256["busy_owner_test"],
        "Busy exact-owner coalescing regression",
        errors,
    )
    for fragment, description in (
        (
            """
DeferredBodyPipelineStageForTest::BodyStored,
""",
            "BodyStored Busy-owner witness",
        ),
        (
            """
DeferredBodyPipelineStageForTest::ValidationSucceeded,
""",
            "ValidationSucceeded Busy-owner witness",
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
