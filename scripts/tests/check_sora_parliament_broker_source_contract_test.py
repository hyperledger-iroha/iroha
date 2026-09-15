"""Mutation coverage for Parliament broker operation, framing, and dispatch guards."""

from __future__ import annotations

import ast
import inspect
import re

import pytest

from scripts.formal import check_sora_parliament_source_contract as guard


BROKER_ROOT = "crates/irohad/src/runtime_provider_broker/"
PRIMITIVES = BROKER_ROOT + "protocol_primitives.rs"
DISPATCH = BROKER_ROOT + "platform_operation_dispatch.rs"
CONSENSUS = BROKER_ROOT + "protocol/platform/operation_dispatch/consensus.rs"


def test_broker_source_baseline() -> None:
    """The actual framed DTOs and delegated handler pass their source contracts."""
    guard.require_parliament_broker_primitives(guard.read(PRIMITIVES))
    guard.require_parliament_broker_dispatch(guard.read(DISPATCH), guard.read(CONSENSUS))


@pytest.mark.parametrize(
    ("operation", "ordinal"),
    (
        ("OPERATION_PARLIAMENT_TLE_PARTIAL_RELEASE_SIGN_V1", 124),
        ("OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1", 125),
    ),
)
@pytest.mark.parametrize("binding", ("constant", "golden_entry"))
def test_broker_rejects_changed_parliament_operation_bindings(
    operation: str, ordinal: int, binding: str
) -> None:
    """Both the protocol ID and its tested ordinal remain fixed for each operation."""
    source = guard.read(PRIMITIVES)
    target = (
        f"{operation}: u16 = {ordinal}"
        if binding == "constant"
        else f"({operation}, {ordinal})"
    )
    assert source.count(target) == 1
    mutated = source.replace(target, target.replace(str(ordinal), "999"), 1)
    with pytest.raises(RuntimeError, match="missing modeled source binding"):
        guard.require_parliament_broker_primitives(mutated)


@pytest.mark.parametrize("kind", ("Request", "Result"))
@pytest.mark.parametrize("mutation", ("unframed", "wrong_schema", "wrong_field_type"))
def test_broker_rejects_malformed_attestation_wire(kind: str, mutation: str) -> None:
    """Framing, stable schema identity, and participant width are wire requirements."""
    source = guard.read(PRIMITIVES)
    name = f"ParliamentTleCapabilityAttest{kind}WireV1"
    frame = f'frame "irohad::runtime_provider_broker::protocol::primitives::{name}"; '
    if mutation == "unframed":
        mutated = source.replace(frame, "", 1)
    elif mutation == "wrong_schema":
        mutated = source.replace(frame, frame.replace(name, "UnrelatedWireV1"), 1)
    else:
        start = source.index(f"pub(super) {name} {{")
        prefix, declaration = source[:start], source[start:]
        mutated = prefix + declaration.replace(
            "pub(super) participant_index: u16", "pub(super) participant_index: u32", 1
        )
    assert mutated != source
    with pytest.raises(RuntimeError):
        guard.require_parliament_broker_primitives(mutated)


def test_broker_allows_additional_unrelated_operations() -> None:
    """The next unused operation ID is independent of Parliament's fixed IDs."""
    source = guard.read(PRIMITIVES)
    unknown_operation = re.search(
        r"assert!\(!super::super::operation_is_known\((\d+)\)\);", source
    )
    assert unknown_operation is not None
    current = int(unknown_operation.group(1))
    source = source.replace(
        f"operation_is_known({current})", f"operation_is_known({current + 1})"
    )
    guard.require_parliament_broker_primitives(source)


@pytest.mark.parametrize(
    ("path", "before", "after"),
    (
        (DISPATCH, "protocol/platform/operation_dispatch/consensus.rs", "unrelated.rs"),
        (
            DISPATCH,
            "    requalify()?;\n    let moderation_quarantine_slot",
            "    let moderation_quarantine_slot",
        ),
        (
            DISPATCH,
            "(slot, OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1)\n"
            "            if slot == parliament_tle_partial_release_signer_slot",
            "(slot, OPERATION_PARLIAMENT_TLE_CAPABILITY_ATTEST_V1)\n"
            "            if slot == global_beacon_partial_signer_slot",
        ),
        (
            DISPATCH,
            "consensus_operations::parliament_tle_capability_attest(state, request)",
            "consensus_operations::qualify_consensus_signer(state, request)",
        ),
        (
            CONSENSUS,
            "decode_parliament_tle_capability_attest_request(&request.payload, &state.network_id)?;",
            "decode_canonical(&request.payload)?;",
        ),
        (
            CONSENSUS,
            ".attest_partial_release_capability(&session, request.participant_index)",
            ".attest_partial_release_capability(&session, 0)",
        ),
        (
            CONSENSUS,
            "if !attestation.matches(&session, request.participant_index)",
            "if false",
        ),
        (
            CONSENSUS,
            "transcript_hash: attestation.transcript_hash()",
            "transcript_hash: [0; 32]",
        ),
    ),
)
def test_broker_rejects_disconnected_or_unvalidated_attestation(
    path: str, before: str, after: str
) -> None:
    """Routing and each typed input, authority, and result binding stay connected."""
    sources = {DISPATCH: guard.read(DISPATCH), CONSENSUS: guard.read(CONSENSUS)}
    assert sources[path].count(before) == 1
    sources[path] = sources[path].replace(before, after, 1)
    with pytest.raises(RuntimeError):
        guard.require_parliament_broker_dispatch(sources[DISPATCH], sources[CONSENSUS])


def test_broker_rejects_requalification_before_attestation() -> None:
    """Requalification must follow the backend call before its result is encoded."""
    consensus = guard.read(CONSENSUS)
    prefix, handler = consensus.split("pub(super) fn parliament_tle_capability_attest(", 1)
    assert handler.count("requalify()?;") == 1
    handler = handler.replace("    requalify()?;\n", "", 1)
    handler = handler.replace(
        "    let attestation = backend", "    requalify()?;\n    let attestation = backend", 1
    )
    mutated = prefix + "pub(super) fn parliament_tle_capability_attest(" + handler
    with pytest.raises(RuntimeError, match="validate and requalify before encoding"):
        guard.require_parliament_broker_dispatch(guard.read(DISPATCH), mutated)


def test_broker_checks_are_connected_to_production_entrypoint() -> None:
    """The production checker runs both independently mutation-tested broker gates."""
    body = ast.parse(inspect.getsource(guard.main))
    calls = [
        node.func.id
        for node in ast.walk(body)
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
    ]
    assert calls.count("require_parliament_broker_primitives") == 1
    assert calls.count("require_parliament_broker_dispatch") == 1
