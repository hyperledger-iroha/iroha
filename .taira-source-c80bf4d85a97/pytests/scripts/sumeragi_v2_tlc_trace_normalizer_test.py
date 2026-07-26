"""Fail-closed tests for the pinned TLC replay-trace normalizer."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
NORMALIZER = ROOT_DIR / "scripts" / "normalize_sumeragi_v2_tlc_trace.py"


def load_normalizer():
    """Load the normalizer as a module without requiring a scripts package."""

    spec = importlib.util.spec_from_file_location("sumeragi_tlc_normalizer", NORMALIZER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


MODULE = load_normalizer()


def message(code: int, payload: str, severity: int = 0) -> str:
    """Return one TLC tool-mode message block."""

    return (
        f"@!@!@STARTMSG {code}:{severity} @!@!@\n"
        f"{payload}\n"
        f"@!@!@ENDMSG {code} @!@!@"
    )


def marker(
    action: str,
    node: int | str = -1,
    peer: int | str = -1,
    view: int | str = -1,
    phase: str = "-",
    subject: str = "-",
) -> str:
    """Render a witnessAction value in TLC's field-sorted presentation."""

    def scalar(value: int | str) -> str:
        return str(value) if isinstance(value, int) else f'"{value}"'

    return (
        f"[ node |-> {scalar(node)},\n"
        f"  peer |-> {scalar(peer)},\n"
        f'  phase |-> "{phase}",\n'
        f'  subject |-> "{subject}",\n'
        f'  action |-> "{action}",\n'
        f"  view |-> {scalar(view)} ]"
    )


def state(number: int, value: str) -> str:
    """Return one minimal state payload with a trace marker."""

    return message(
        2217,
        f"{number}: <Action from test>\n"
        f"/\\ witnessAction = {value}\n"
        "/\\ unrelatedCoreVariable = {}",
        severity=4,
    )


def valid_log() -> str:
    """Return the smallest valid NoDecision counterexample log."""

    return "\n".join(
        (
            "unstructured SANY output is permitted outside tool messages",
            message(2188, "Running Random Simulation with seed 19349663 with 1 worker"),
            message(2110, "Invariant NoDecision is violated.", severity=1),
            message(2121, "The behavior up to this point is:", severity=1),
            state(1, marker("Initial")),
            state(2, marker("SetGST")),
            state(
                3,
                marker(
                    "PersistDecision",
                    node=1,
                    view=1,
                    phase="Commit",
                    subject="A",
                ),
            ),
        )
    )


def test_normalizes_tool_messages_and_numeric_sentinels() -> None:
    actions = MODULE.normalize(valid_log(), 19349663)

    assert [action.action for action in actions] == ["SetGST", "PersistDecision"]
    assert actions[0].node == "-"
    assert actions[1].node == 1
    assert MODULE.render(actions, 19349663).endswith(
        "1\tSetGST\t-\t-\t-\t-\t-\n"
        "2\tPersistDecision\t1\t-\t1\tCommit\tA\n"
    )


def test_rejects_string_sentinel_in_numeric_marker_field() -> None:
    source = valid_log().replace(
        state(1, marker("Initial")),
        state(1, marker("Initial", node="-")),
        1,
    )

    with pytest.raises(ValueError, match="node must be an integer"):
        MODULE.normalize(source, 19349663)


@pytest.mark.parametrize(
    ("source", "error"),
    (
        (valid_log().replace("seed 19349663", "seed 9", 1), "seed differs"),
        (
            valid_log().replace(
                "Invariant NoDecision is violated.",
                "Invariant TypeOK is violated.",
                1,
            ),
            "exact NoDecision",
        ),
        (
            valid_log().replace(
                "The behavior up to this point is:", "A different trace follows:", 1
            ),
            "exact TLC counterexample introduction",
        ),
        (
            valid_log().replace("STARTMSG 2217:4", "STARTMSG 2217:3", 1),
            "unexpected severity",
        ),
        (valid_log().replace("3: <Action", "4: <Action", 1), "non-contiguous"),
        (
            valid_log().replace('action |-> "SetGST"', 'action |-> "BeginDecision"', 1),
            "unsupported action",
        ),
        (
            valid_log().replace(
                'action |-> "PersistDecision"', 'action |-> "FormCommitQC"', 1
            ),
            "last action is FormCommitQC",
        ),
        (
            valid_log().replace(
                'phase |-> "Commit"', 'phase |-> "Prepare"', 1
            ),
            "requires phase='Commit'",
        ),
    ),
)
def test_counterexample_contract_fails_closed(source: str, error: str) -> None:
    with pytest.raises(ValueError, match=error):
        MODULE.normalize(source, 19349663)


def test_rejects_duplicate_record_field() -> None:
    source = valid_log().replace(
        'action |-> "SetGST",',
        'action |-> "SetGST",\n  action |-> "SetGST",',
        1,
    )

    with pytest.raises(ValueError, match="repeats field 'action'"):
        MODULE.normalize(source, 19349663)


@pytest.mark.parametrize(
    ("source", "error"),
    (
        (
            valid_log().replace("@!@!@ENDMSG 2217 @!@!@", "@!@!@ENDMSG 999 @!@!@", 1),
            "code mismatch",
        ),
        (
            valid_log() + "\n@!@!@STARTMSG 2000:0 @!@!@\nunterminated",
            "unterminated TLC tool message",
        ),
        (
            "@!@!@ENDMSG 2217 @!@!@\n" + valid_log(),
            "orphan TLC tool-message end",
        ),
    ),
)
def test_tool_message_delimiters_fail_closed(source: str, error: str) -> None:
    with pytest.raises(ValueError, match=error):
        MODULE.normalize(source, 19349663)


def test_rejects_multiple_witness_assignments_in_one_state() -> None:
    source = valid_log().replace(
        "/\\ unrelatedCoreVariable = {}",
        f"/\\ witnessAction = {marker('Initial')}",
        1,
    )

    with pytest.raises(ValueError, match="2 witnessAction assignments"):
        MODULE.normalize(source, 19349663)
