"""Mutation coverage for constructor-authenticated Parliament source boundaries."""

from __future__ import annotations

import ast
import inspect
import re
import subprocess
import sys

import pytest

from scripts.formal import check_sora_parliament_source_contract as guard


OPAQUE_OWNERS = (
    (
        "crates/iroha_core/src/tle_release.rs",
        "AuthorizedTleReleaseContextV1",
        "/// Authorize a TLE release from one point-in-time committed state view.",
        "opaque release authorization",
        guard.require_opaque_release_authorizations,
    ),
    (
        "crates/iroha_core/src/tle_release.rs",
        "ValidatedTleReleaseProjectionV1",
        "/// Closed failures while validating a public authenticated-broker projection.",
        "validated broker projection",
        guard.require_opaque_release_authorizations,
    ),
    (
        "crates/iroha_core/src/tle_release/casting.rs",
        "AuthorizedTimedOvnCastingContextV1",
        "/// Authorize and replay-validate one public timed-OVN casting context.",
        "opaque casting authorization",
        guard.require_opaque_casting_authorization,
    ),
)


@pytest.mark.parametrize("owner", OPAQUE_OWNERS, ids=lambda owner: owner[1])
def test_opaque_authorization_source_baseline(owner: tuple) -> None:
    """The actual production owner passes before any mutation is considered."""
    path, _, _, _, check = owner
    check(guard.read(path))


@pytest.mark.parametrize("owner", OPAQUE_OWNERS, ids=lambda owner: owner[1])
@pytest.mark.parametrize("trait", ("SerializePayload", "DeserializePayload"))
@pytest.mark.parametrize("implementation", ("derive", "manual"))
def test_opaque_authorizations_reject_payload_codecs(
    owner: tuple, trait: str, implementation: str
) -> None:
    """Neither a derive attribute nor an impl may make authorized state decodable."""
    path, name, end, label, check = owner
    source = guard.read(path)
    check(source)
    if implementation == "derive":
        declaration = f"pub struct {name} {{"
        assert source.count(declaration) == 1
        mutated = source.replace(
            declaration, f"#[derive(norito::{trait})]\n{declaration}", 1
        )
    else:
        assert source.count(end) == 1
        lifetime = "<'_>" if trait == "DeserializePayload" else ""
        mutated = source.replace(
            end, f"impl norito::{trait}{lifetime} for {name} {{}}\n\n{end}", 1
        )
    assert mutated != source
    with pytest.raises(RuntimeError, match=re.escape(f"{path}: {label} exposes {trait!r}")):
        check(mutated)


def test_opaque_checks_are_connected_to_production_entrypoint() -> None:
    """The production check invokes both independently tested authorization gates."""
    body = ast.parse(inspect.getsource(guard.main))
    calls = [node.func.id for node in ast.walk(body) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name)]
    assert calls.count("require_opaque_release_authorizations") == 1
    assert calls.count("require_opaque_casting_authorization") == 1


TYPES_PATH = "crates/iroha_data_model/src/governance/types.rs"
WORLD_PATH = "crates/iroha_core/src/smartcontracts/isi/world.rs"
ENDORSEMENT_ORDER = """!public_finding
                    .endorsing_assignments
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])"""


def test_complete_source_contract_baseline() -> None:
    """Exercise every production binding so drift outside the focused gates is visible."""
    assert guard.main() == 0


def test_source_contract_cli_help() -> None:
    """The read-only command documents its prerequisites without checking sources."""
    result = subprocess.run(
        [sys.executable, guard.__file__, "--help"], capture_output=True, text=True
    )
    assert result.returncode == 0
    assert "read-only" in result.stdout
    assert "no third-party dependencies" in result.stdout
    assert result.stderr == ""


@pytest.mark.parametrize(
    "order",
    (
        "!public_finding.endorsing_assignments.windows(2).all(|pair| pair[0] < pair[1])",
        "!public_finding\n\t.endorsing_assignments\n\t.windows(2)"
        "\n\t.all(|pair| pair[0] < pair[1])",
        "! public_finding . endorsing_assignments . windows ( 2 )"
        " . all ( | pair | pair [ 0 ] < pair [ 1 ] )",
    ),
)
def test_endorsement_order_accepts_rust_whitespace_changes(order: str) -> None:
    """Line wrapping and indentation do not alter strict supporter ordering."""
    source = guard.read(TYPES_PATH)
    guard.require_public_finding_endorsement_order(source)
    assert source.count(ENDORSEMENT_ORDER) == 1
    guard.require_public_finding_endorsement_order(source.replace(ENDORSEMENT_ORDER, order))


@pytest.mark.parametrize(
    "order",
    (
        ENDORSEMENT_ORDER.replace("<", "<="),
        ENDORSEMENT_ORDER.replace("<", ">"),
        ENDORSEMENT_ORDER.replace("windows(2)", "windows(3)"),
        ENDORSEMENT_ORDER.removeprefix("!"),
        ENDORSEMENT_ORDER.replace("pair[0] < pair[1]", "true"),
        "false",
    ),
)
def test_endorsement_order_rejects_weakened_certificate_guard(order: str) -> None:
    """Duplicates, reversed comparisons and removed checks cannot satisfy the contract."""
    source = guard.read(TYPES_PATH)
    guard.require_public_finding_endorsement_order(source)
    assert source.count(ENDORSEMENT_ORDER) == 1
    mutated = source.replace(ENDORSEMENT_ORDER, order)
    # An intact snippet elsewhere cannot stand in for certificate validation.
    mutated += "\n/* " + ENDORSEMENT_ORDER + " */\n"
    with pytest.raises(RuntimeError, match="certificate validation must reject supporters"):
        guard.require_public_finding_endorsement_order(mutated)


@pytest.mark.parametrize(
    "removed,replacement",
    (
        ("entry.request.request_height != current_height", "false"),
        ("ensure_parliament_logical_beacon_v1(", "unchecked_logical_beacon("),
        ("entry.request.target_seats != configured_target", "false"),
        (
            "ParliamentDecisionModeV1::HiddenBindingBallot",
            "ParliamentDecisionModeV1::PublicFinding",
        ),
        (
            "if !crate::governance::parliament::hidden_ballot_population",
            "if crate::governance::parliament::hidden_ballot_population",
        ),
        ("expected_candidates.len()", "0"),
        ("&& hidden_body_requested", "|| hidden_body_requested"),
        (".record_hidden_sortition_capacity_failure_batch(", ".register_sortition_request_batch("),
        (".register_sortition_request_batch(", ".record_hidden_sortition_capacity_failure_batch("),
        (
            "ParliamentNoResultKindV1::SortitionRetriesExhausted",
            "ParliamentNoResultKindV1::RandomnessRedrawBudgetExhausted",
        ),
    ),
)
def test_sortition_registration_rejects_missing_shared_guards(
    removed: str, replacement: str
) -> None:
    """Both registration paths retain the helper's height, authority and capacity checks."""
    source = guard.read_rust_with_includes(WORLD_PATH)
    guard.require_sortition_registration_guards(source)
    helper = guard.section(
        source,
        "    fn apply_parliament_sortition_request_batch_v1(",
        "    fn ensure_parliament_logical_beacon_v1(",
        WORLD_PATH,
    )
    assert helper.count(removed) == 1
    mutated = source.replace(helper, helper.replace(removed, replacement))
    with pytest.raises(RuntimeError, match=WORLD_PATH):
        guard.require_sortition_registration_guards(mutated)


@pytest.mark.parametrize("initial", (False, True))
@pytest.mark.parametrize("bypass", (False, True))
def test_sortition_registration_rejects_disconnected_admission(
    initial: bool, bypass: bool
) -> None:
    """An unused correct helper cannot authenticate a transition or substituted candidates."""
    source = guard.read_rust_with_includes(WORLD_PATH)
    guard.require_sortition_registration_guards(source)
    start, end, candidates = (
        (
            "gov::ParliamentLifecycleTransitionV1::RegisterInitialSortition => {",
            "gov::ParliamentLifecycleTransitionV1::RegisterSortitionRequest(payload) => {",
            "candidates,",
        )
        if initial
        else (
            "gov::ParliamentLifecycleTransitionV1::RegisterSortitionRequest(payload) => {",
            "gov::ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(payload) => {",
            "expected_candidates,",
        )
    )
    branch = guard.section(source, start, end, WORLD_PATH)
    removed = "apply_parliament_sortition_request_batch_v1(" if bypass else candidates
    replacement = "unchecked_sortition_registration(" if bypass else "Vec::new(),"
    assert branch.count(removed) == 1
    mutated = source.replace(branch, branch.replace(removed, replacement))
    with pytest.raises(RuntimeError, match="must use shared sortition admission"):
        guard.require_sortition_registration_guards(mutated)


@pytest.mark.parametrize("path", (TYPES_PATH, WORLD_PATH))
def test_production_checker_enforces_repaired_guards(
    path: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Run actual negative source mutations through the complete production entrypoint."""
    original_read = guard.read
    source = original_read(path)
    if path == TYPES_PATH:
        assert source.count(ENDORSEMENT_ORDER) == 1
        mutated = source.replace(ENDORSEMENT_ORDER, ENDORSEMENT_ORDER.replace("<", "<="))
        message = "certificate validation must reject supporters"
    else:
        original_call = "no_result_kind = apply_parliament_sortition_request_batch_v1("
        assert original_call in source
        mutated = source.replace(
            original_call, "no_result_kind = unchecked_sortition_registration(", 1
        )
        message = "must use shared sortition admission"
    monkeypatch.setattr(
        guard, "read", lambda relative: mutated if relative == path else original_read(relative)
    )
    with pytest.raises(RuntimeError, match=message):
        guard.main()
