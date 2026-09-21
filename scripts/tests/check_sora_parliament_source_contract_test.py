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


STATE_PATH = "crates/iroha_core/src/state.rs"
EXPIRY_CALL = "Self::apply_block_start_private_settlement_expiry(&mut sb, now_h);"
ENACTMENT_CALL = "Self::apply_block_start_parliament_enactments(&mut sb, now_h);"


def test_block_start_phase_helpers_preserve_original_order_and_custody() -> None:
    """The extracted production phases use the same original block before execution."""
    guard.require_block_start_enactment_phases(guard.read(STATE_PATH))
    body = ast.parse(inspect.getsource(guard.main))
    calls = [node.func.id for node in ast.walk(body)
             if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)]
    assert calls.count("require_block_start_enactment_phases") == 1


@pytest.mark.parametrize("mutation", (
    "missing_expiry", "missing_enactment", "duplicate", "swapped",
    "wrong_height", "conditional", "late_before_flag", "late_after_execution",
))
def test_block_start_phase_calls_reject_disconnected_or_late_owners(mutation: str) -> None:
    """Valid Rust statement changes cannot disconnect helpers or defer them until after execution."""
    source = guard.read(STATE_PATH)
    guard.require_block_start_enactment_phases(source)
    if mutation == "missing_expiry":
        mutated = source.replace(EXPIRY_CALL, "", 1)
    elif mutation == "missing_enactment":
        mutated = source.replace(ENACTMENT_CALL, "", 1)
    elif mutation == "duplicate":
        mutated = source.replace(ENACTMENT_CALL, ENACTMENT_CALL * 2, 1)
    elif mutation == "swapped":
        mutated = source.replace(EXPIRY_CALL, "PHASE_SWAP", 1).replace(
            ENACTMENT_CALL, EXPIRY_CALL, 1).replace("PHASE_SWAP", ENACTMENT_CALL, 1)
    elif mutation == "wrong_height":
        mutated = source.replace(ENACTMENT_CALL, ENACTMENT_CALL.replace("now_h)", "now_h + 1)"), 1)
    elif mutation == "conditional":
        mutated = source.replace(ENACTMENT_CALL, "if false { " + ENACTMENT_CALL + " }", 1)
    else:
        anchor = ("        sb.start_of_block_effects_applied = true;" if mutation == "late_before_flag"
                  else "        let result = after_start(&mut sb, continuation).map_err(StateBlockStartError::Stage)?;")
        assert source.count(anchor) == 1
        mutated = source.replace(ENACTMENT_CALL, "", 1).replace(anchor, anchor + "\n        " + ENACTMENT_CALL, 1)
    assert mutated != source
    with pytest.raises(RuntimeError, match="start phases"):
        guard.require_block_start_enactment_phases(mutated)


@pytest.mark.parametrize("original,replacement", (
    ("barrier.manifest.expiry_height < now_h", "barrier.manifest.expiry_height <= now_h"),
    ("expiry.apply();", "drop(expiry);"),
    ("*enact_at_height < now_h", "*enact_at_height > now_h"),
    (".get(&now_h)", ".get(&(now_h + 1))"),
    ("drop(enactment);", "enactment.apply();"),
    ("failure.apply();", "drop(failure);"),
    ("let due_parliament_certificates = sb", "let _ = sb.world.parliament_attempts.iter();\n        let due_parliament_certificates = sb"),
))
def test_block_start_phase_bodies_reject_changed_height_or_rollback(
    original: str, replacement: str,
) -> None:
    """Both helpers retain exact due selection, actual rollback, and terminal recording."""
    source = guard.read(STATE_PATH)
    guard.require_block_start_enactment_phases(source)
    helpers = guard.section(source,
        "    /// Release expired private locks inside their original block transaction.",
        "    /// Apply scheduled world transitions within their shared transaction.", STATE_PATH)
    assert original in helpers
    mutated = source.replace(helpers, helpers.replace(original, replacement))
    with pytest.raises(RuntimeError, match=STATE_PATH):
        guard.require_block_start_enactment_phases(mutated)



@pytest.mark.parametrize("original,replacement", (
    ("self.acquire_canonical_runtime_block(false)?", "self.acquire_canonical_runtime_block(false).unwrap()"),
    ("self.acquire_canonical_runtime_block(false)?", "self.acquire_canonical_runtime_block(true)?"),
    ("before_start(&mut sb).map_err(StateBlockStartError::Stage)?", "before_start(&mut sb).unwrap()"),
    ("after_start(&mut sb, continuation).map_err(StateBlockStartError::Stage)?", "after_start(&mut sb, continuation).unwrap()"),
))
def test_block_start_admission_and_stage_refusals_remain_typed(
    original: str, replacement: str,
) -> None:
    """Local history admission precedes original effects; callback errors retain their class."""
    source = guard.read(STATE_PATH)
    guard.require_block_start_enactment_phases(source)
    constructor = guard.section(source,
        "    fn block_with_owned_start_stages<'state, E: std::fmt::Debug, T, R>(",
        "    /// Release expired private locks inside their original block transaction.", STATE_PATH)
    assert constructor.count(original) == 1
    mutated = source.replace(constructor, constructor.replace(original, replacement, 1), 1)
    with pytest.raises(RuntimeError, match="start phases"):
        guard.require_block_start_enactment_phases(mutated)


def test_production_checker_requires_the_extracted_start_phase_call(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A disconnected intact helper fails the real full checker entrypoint."""
    original_read = guard.read
    source = original_read(STATE_PATH)
    mutated = source.replace(ENACTMENT_CALL, "", 1)
    monkeypatch.setattr(guard, "read", lambda path: mutated if path == STATE_PATH else original_read(path))
    with pytest.raises(RuntimeError, match="start phases"):
        guard.main()



def test_parliament_direct_event_return_preserves_projection_before_drain() -> None:
    """The actual direct-return boundary still owns the sole event drain."""
    guard.require_parliament_event_capture(guard.read(STATE_PATH))
    body = ast.parse(inspect.getsource(guard.main))
    calls = [node.func.id for node in ast.walk(body)
             if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)]
    assert calls.count("require_parliament_event_capture") == 1


@pytest.mark.parametrize("mutation", ("early_drain", "substituted_return", "missing_projection"))
def test_parliament_event_capture_rejects_early_drain_or_lost_projection(mutation: str) -> None:
    """No second drain, replacement result or omitted retained projection is accepted."""
    source = guard.read(STATE_PATH)
    guard.require_parliament_event_capture(source)
    capture = guard.section(source, "    fn apply_without_execution_inner(",
                            "    fn pin_new_autoscale_lane_committee(", STATE_PATH)
    if mutation == "early_drain":
        changed = capture.replace("let parliament_transitions = self",
                                  "let _ = self.world.take_external_events();\n            let parliament_transitions = self", 1)
    elif mutation == "substituted_return":
        changed = capture.replace("Ok(self.world.take_external_events())", "Ok(Vec::new())", 1)
    else:
        changed = capture.replace(".extend(parliament_transitions);", ".extend(Vec::new());", 1)
    assert changed != capture
    with pytest.raises(RuntimeError, match=STATE_PATH):
        guard.require_parliament_event_capture(source.replace(capture, changed))


def test_prepared_parliament_commit_publication_baseline_and_entrypoint() -> None:
    """The actual prepared commit path and production checker share the same gate."""
    guard.require_parliament_commit_publication(guard.read(STATE_PATH))
    body = ast.parse(inspect.getsource(guard.main))
    calls = [node.func.id for node in ast.walk(body)
             if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)]
    assert calls.count("require_parliament_commit_publication") == 1


@pytest.mark.parametrize("mutation", (
    "world_refusal", "geometry_refusal", "world_drop", "hash_drop", "swapped_commits",
    "writer_removed", "generation_removed", "writer_early_drop", "conditional_world",
    "replay_prevalidation", "authenticated_replay", "transitions_outside_replay_guard",
    "gauges_inside_replay_guard", "duplicate_publisher", "early_telemetry", "missing_cfg",
    "hash_prepare_refusal", "hash_cleanup_before_commit_unlock", "hash_cleanup_declared_after_commit_lock",
))
def test_prepared_commit_rejects_refusal_publication_or_replay_regressions(mutation: str) -> None:
    """Complete Rust statements cannot bypass refusals, publish early, or recount replay."""
    source = guard.read(STATE_PATH)
    guard.require_parliament_commit_publication(source)
    commit = guard.section(source, "    fn commit_inner(",
                           "    fn mint_canonical_carrier_commit_metadata_authorization(", STATE_PATH)
    telemetry_start = commit.index('        #[cfg(feature = "telemetry")]\n        if !replay_prevalidation {',
                                   commit.index("drop(autoscale_lifecycle_guard);"))
    telemetry_end = commit.index("        if !verified_lane_relay_records.is_empty()", telemetry_start)
    telemetry = commit[telemetry_start:telemetry_end]
    changed = commit
    if mutation == "world_refusal":
        anchor = "TransactionsBlockError::WorldCommitPreparation\n        })?;"
        assert commit.count(anchor) == 1
        changed = commit.replace(anchor, anchor.replace("})?;", "}).unwrap();"), 1)
    elif mutation == "geometry_refusal":
        changed = commit.replace("return Err(TransactionsBlockError::from(err));", "", 1)
    elif mutation == "hash_prepare_refusal":
        changed = commit.replace(".map_err(|(_, _)| TransactionsBlockError::SnapshotObservationChanged)?;", ".unwrap();", 1)
    elif mutation == "hash_cleanup_before_commit_unlock":
        changed = commit.replace("        drop(hash_retirement);", "", 1).replace(
            "        drop(_state_commit_lock);", "        drop(hash_retirement);\n        drop(_state_commit_lock);", 1)
    elif mutation == "hash_cleanup_declared_after_commit_lock":
        changed = commit.replace("        let hash_retirement;", "", 1).replace(
            "let _state_commit_lock = state_ref.state_commit_lock.lock();",
            "let _state_commit_lock = state_ref.state_commit_lock.lock();\n        let hash_retirement;", 1)
    elif mutation == "world_drop":
        changed = commit.replace("world.commit();", "drop(world);", 1)
    elif mutation == "hash_drop":
        changed = commit.replace("hash_retirement = block_hashes.publish();", "drop(block_hashes);", 1)
    elif mutation == "swapped_commits":
        changed = commit.replace("world.commit();", "SWAP_COMMIT", 1).replace(
            "hash_retirement = block_hashes.publish();", "world.commit();", 1).replace("SWAP_COMMIT", "hash_retirement = block_hashes.publish();", 1)
    elif mutation in ("writer_removed", "generation_removed", "writer_early_drop", "conditional_world"):
        publication = guard.section(commit, "        let mut lifecycle_post_publication = None;",
                                    "        if let Some(post) = lifecycle_post_publication", STATE_PATH)
        if mutation == "writer_removed":
            replacement = publication.replace("let _state_write_lock = state_write_lock.lock();", "", 1)
        elif mutation == "generation_removed":
            replacement = publication.replace("let _view_generation = state_ref.begin_state_view_write();", "", 1)
        elif mutation == "writer_early_drop":
            replacement = publication.replace("transactions.publish();",
                "drop(_state_write_lock);\n            transactions.publish();", 1)
        else:
            replacement = publication.replace("world.commit();", "if false { world.commit(); }", 1)
        changed = commit.replace(publication, replacement, 1)
    elif mutation in ("replay_prevalidation", "authenticated_replay", "missing_cfg", "duplicate_publisher"):
        old, new = {
            "replay_prevalidation": ("if !replay_prevalidation {", "if true {"),
            "authenticated_replay": ("if !authenticated_replay_commit {", "if true {"),
            "missing_cfg": ('#[cfg(feature = "telemetry")]', ""),
            "duplicate_publisher": (
                ".record_committed_parliament_transition(transition, no_result_kind);",
                ".record_committed_parliament_transition(transition, no_result_kind);\n"
                "                    state_ref.telemetry.record_committed_parliament_transition(transition, no_result_kind);"),
        }[mutation]
        changed = commit.replace(telemetry, telemetry.replace(old, new, 1), 1)
    elif mutation == "transitions_outside_replay_guard":
        old = "            if !authenticated_replay_commit {\n"
        moved = telemetry.replace(old, "", 1).replace(
            "                }\n            }\n            if let Some(counts)",
            "                }\n            if !authenticated_replay_commit {}\n            if let Some(counts)", 1)
        changed = commit.replace(telemetry, moved, 1)
    elif mutation == "gauges_inside_replay_guard":
        moved = telemetry.replace("                }\n            }\n            if let Some(counts)",
                                  "                }\n            if let Some(counts)", 1).replace(
            "            if let Some(citizens_total)", "            }\n            if let Some(citizens_total)", 1)
        changed = commit.replace(telemetry, moved, 1)
    else:
        changed = commit.replace(telemetry, "", 1).replace(
            "        let mut lifecycle_post_publication = None;", telemetry + "        let mut lifecycle_post_publication = None;", 1)
    assert changed != commit
    with pytest.raises(RuntimeError, match=STATE_PATH):
        guard.require_parliament_commit_publication(source.replace(commit, changed, 1))


BEACON_PATH = "crates/iroha_core/src/sumeragi/v2_beacon.rs"


def test_indexed_beacon_requirement_survives_deferred_activation() -> None:
    """Both constructors, activation, and candidate attachment retain the original demand."""
    guard.require_parliament_beacon_requirement(guard.read(BEACON_PATH))
    body = ast.parse(inspect.getsource(guard.main))
    calls = [node.func.id for node in ast.walk(body)
             if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)]
    assert calls.count("require_parliament_beacon_requirement") == 1


@pytest.mark.parametrize("old,new", (
    ("npos_boundary_requested || parliament_requested", "npos_boundary_requested && parliament_requested"),
    ("context.height.checked_add(1)", "context.height.checked_add(2)"),
    (".get(&(logical_beacon_id, context.height))", ".get(&(logical_beacon_id, context.height + 1))"),
    (".is_some_and(|attempts| !attempts.is_empty())", ".is_some_and(|attempts| attempts.is_empty())"),
    ("let required_for_consensus = Self::required_for_height(context, state);",
     "let required_for_consensus = false;"),
    ("local_validator.is_some() && Self::required_for_height(context, state.as_ref())", "false"),
    ("deferred_state: required_for_consensus.then_some(state)", "deferred_state: None"),
    ("Err(error) => return Err(error),", "Err(_) => None,"),
    ("if self.active.is_some() || !self.required_for_consensus", "if true"),
    ("&self.context,\n            state,", "&self.context,\n            &replacement_state,"),
    ("self.signer.clone(),\n        )?;", "self.signer.clone(),\n        ).unwrap();"),
    ("        *self = activated;", "        drop(activated);"),
    ("pub(crate) const fn pulse_required_for_consensus(&self) -> bool {\n        self.required_for_consensus",
     "pub(crate) const fn pulse_required_for_consensus(&self) -> bool {\n        false"),
    ("if self.pulse_required_for_consensus() && pulse.is_none()", "if false"),
    ("effects.finalized_global_beacon_pulse = pulse;", "effects.finalized_global_beacon_pulse = None;"),
))
def test_beacon_requirement_rejects_lost_demand_or_unauthenticated_activation(old: str, new: str) -> None:
    """Demand, original State activation, and missing-pulse refusal cannot be bypassed."""
    source = guard.read(BEACON_PATH)
    guard.require_parliament_beacon_requirement(source)
    assert source.count(old) == 1
    changed = source.replace(old, new, 1)
    with pytest.raises(RuntimeError, match=BEACON_PATH):
        guard.require_parliament_beacon_requirement(changed)


STORAGE_PATH = "crates/mv/src/storage.rs"


def _storage_iterator_implementation(source: str, owner: str) -> tuple[int, int]:
    """Select the concrete owner, so another implementation cannot be a decoy."""
    marker = f"StorageReadOnly<K, V> for {owner}<'_, K, V, M>"
    assert source.count(marker) == 1
    start = source.rfind("impl<", 0, source.index(marker))
    opening = source.index("{", source.index(marker))
    depth = 1
    for end in range(opening + 1, len(source)):
        depth += (source[end] == "{") - (source[end] == "}")
        if depth == 0:
            return start, end + 1
    raise AssertionError("missing concrete implementation end")


def test_borrowed_storage_iterators_accept_current_complete_owner_families() -> None:
    """All three owners retain lifetime, mode charge, reverse order and exact length."""
    guard.require_storage_borrowed_iterators(guard.read(STORAGE_PATH))


@pytest.mark.parametrize("mutation", (
    "iter_forward_only", "iter_inexact", "range_forward_only",
    "range_owner_lifetime", "iter_static_items", "range_sized_query",
))
def test_borrowed_storage_iterator_trait_rejects_weakened_contract(mutation: str) -> None:
    source = guard.read(STORAGE_PATH)
    start = source.index("pub trait StorageReadOnly")
    end = source.index("mod view {", start)
    trait = source[start:end]
    if mutation == "range_sized_query":
        range_start = trait.index("fn range<Q>")
        changed = trait[:range_start] + trait[range_start:].replace(
            "Q: Ord + ?Sized;", "Q: Ord + Sized;", 1
        )
    else:
        family = "RangeIter" if mutation.startswith("range_") else "Iter"
        match = re.search(rf"type {family}<'a>.*?;", trait, re.S)
        assert match
        declaration = match.group()
        if mutation.endswith("forward_only"):
            replacement = declaration.replace("DoubleEndedIterator", "Iterator")
        elif mutation == "iter_inexact":
            replacement = declaration.replace(" + ExactSizeIterator", "")
        elif mutation == "range_owner_lifetime":
            replacement = re.sub(r"\s+where\s+Self: 'a", "", declaration)
        else:
            replacement = declaration.replace("&'a", "&'static")
        changed = trait[:match.start()] + replacement + trait[match.end():]
    assert changed != trait
    mutated = source[:start] + changed + source[end:] + "\n/* " + trait + " */\n"
    with pytest.raises(RuntimeError, match="borrowed iterator trait contract changed"):
        guard.require_storage_borrowed_iterators(mutated)


@pytest.mark.parametrize("owner", ("View", "Block", "Transaction"))
@pytest.mark.parametrize("family", ("Iter", "RangeIter"))
@pytest.mark.parametrize("mutation", ("erase_charge", "remove_owner_lifetime", "boxed_delegate"))
def test_borrowed_storage_iterators_reject_family_and_allocation_escape(
    owner: str, family: str, mutation: str,
) -> None:
    source = guard.read(STORAGE_PATH)
    start, end = _storage_iterator_implementation(source, owner)
    implementation = source[start:end]
    if mutation == "boxed_delegate":
        receiver = {"View": "txn", "Block": "self.blocks", "Transaction": "self.current()"}[owner]
        call = f"{receiver}.iter()" if family == "Iter" else f"{receiver}.range(bounds)"
        assert implementation.count(call) == 1
        changed = implementation.replace(call, f"Box::new({call})", 1)
        message = f"{owner} borrowed iterator must use its original native owner"
    else:
        match = re.search(rf"type {family}<'a>.*?;", implementation, re.S)
        assert match
        declaration = match.group()
        replacement = (declaration.replace("M::Charge", "Untracked")
                       if mutation == "erase_charge"
                       else re.sub(r"\s+where\s+Self: 'a", "", declaration))
        changed = implementation[:match.start()] + replacement + implementation[match.end():]
        message = f"{owner} borrowed iterator family changed"
    assert changed != implementation
    mutated = source[:start] + changed + source[end:] + "\n/* " + implementation + " */\n"
    with pytest.raises(RuntimeError, match=re.escape(message)):
        guard.require_storage_borrowed_iterators(mutated)


@pytest.mark.parametrize("owner", ("View", "Block", "Transaction"))
def test_borrowed_storage_iterators_reject_other_generation_or_order(owner: str) -> None:
    source = guard.read(STORAGE_PATH)
    start, end = _storage_iterator_implementation(source, owner)
    implementation = source[start:end]
    call = {"View": "snapshot.range(bounds)", "Block": "self.blocks.range(bounds)",
            "Transaction": "self.current().range(bounds)"}[owner]
    assert implementation.count(call) == 1
    changed = implementation.replace(call, f"{call}.rev()", 1)
    with pytest.raises(RuntimeError, match=f"{owner} borrowed iterator must use its original native owner"):
        guard.require_storage_borrowed_iterators(source[:start] + changed + source[end:])


def test_borrowed_storage_iterator_gate_is_connected_to_main() -> None:
    calls = [node.func.id for node in ast.walk(ast.parse(inspect.getsource(guard.main)))
             if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)]
    assert calls.count("require_storage_borrowed_iterators") == 1
