#!/usr/bin/env python3
"""Normalize a pinned TLC tool-mode decision trace into replay actions.

TLA2Tools 1.7.4 predates ``-dumpTrace json``.  Its supported ``-tool`` mode
wraps every diagnostic in stable message delimiters, including one message
with code 2217 for each state in an invariant counterexample.  The trace-only
``witnessAction`` variable in ``SumeragiV2TraceWitness`` records the exact
bound arguments of the action that reached each state, so normalization does
not infer parameters from pretty-printed Core state.

Unknown messages relevant to the counterexample, malformed records,
non-contiguous states, unsupported actions, a different simulation seed, and
a trace that does not end at ``PersistDecision`` all fail closed.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import TypeAlias


MESSAGE_START = re.compile(r"^@!@!@STARTMSG ([0-9]+):([0-9]+) @!@!@$")
MESSAGE_END = re.compile(r"^@!@!@ENDMSG ([0-9]+) @!@!@$")
STATE_HEADER = re.compile(r"^([0-9]+): <.+>$")
SIMULATION_START = re.compile(r"^Running Random Simulation with seed (-?[0-9]+)\b")
WITNESS_ASSIGNMENT = "/\\ witnessAction = "

Scalar: TypeAlias = int | str

REPLAY_ACTIONS = frozenset(
    {
        "SetGST",
        "AssembleLocalBody",
        "BeginTimeout",
        "PersistTimeout",
        "CompleteTimeoutSignature",
        "DeliverTimeout",
        "PersistInstallTC",
        "DeliverTC",
        "BeginInstallTC",
        "BeginLocalProposal",
        "PersistProposal",
        "CompleteProposalSignature",
        "DeliverProposal",
        "FetchBody",
        "StoreBody",
        "ValidateBody",
        "BeginPrepare",
        "PersistPrepare",
        "CompleteVoteSignature",
        "DeliverVote",
        "FormPrepareQC",
        "DeliverQC",
        "BeginObservePrepare",
        "PersistObservePrepare",
        "BeginLockCommit",
        "PersistLockCommit",
        "FormCommitQC",
        "PersistDecision",
    }
)

RECORD_FIELDS = ("action", "node", "peer", "view", "phase", "subject")


@dataclass(frozen=True)
class ToolMessage:
    """One delimited TLC tool-mode message."""

    code: int
    severity: int
    payload: str


@dataclass(frozen=True)
class ReplayAction:
    """One validated action projected onto the production replay columns."""

    action: str
    node: Scalar
    peer: Scalar
    view: Scalar
    phase: str
    subject: str


def parse_tool_messages(source: str) -> list[ToolMessage]:
    """Extract TLC tool messages and reject broken delimiter structure."""

    messages: list[ToolMessage] = []
    current_code: int | None = None
    current_severity = 0
    payload: list[str] = []
    for line_number, line in enumerate(source.splitlines(), 1):
        start = MESSAGE_START.fullmatch(line)
        end = MESSAGE_END.fullmatch(line)
        if start is not None:
            if current_code is not None:
                raise ValueError(f"nested TLC tool message at line {line_number}")
            current_code = int(start.group(1))
            current_severity = int(start.group(2))
            payload = []
        elif end is not None:
            if current_code is None:
                raise ValueError(f"orphan TLC tool-message end at line {line_number}")
            end_code = int(end.group(1))
            if end_code != current_code:
                raise ValueError(
                    f"TLC tool-message code mismatch at line {line_number}: "
                    f"started {current_code}, ended {end_code}"
                )
            messages.append(
                ToolMessage(current_code, current_severity, "\n".join(payload))
            )
            current_code = None
            current_severity = 0
            payload = []
        elif current_code is not None:
            payload.append(line)
    if current_code is not None:
        raise ValueError(f"unterminated TLC tool message {current_code}")
    if not messages:
        raise ValueError("input has no TLC tool messages")
    return messages


class RecordParser:
    """Parser for the scalar-only TLA record used by ``witnessAction``."""

    def __init__(self, source: str) -> None:
        self.source = source
        self.offset = 0

    def skip_space(self) -> None:
        while self.offset < len(self.source) and self.source[self.offset].isspace():
            self.offset += 1

    def take(self, token: str) -> None:
        self.skip_space()
        if not self.source.startswith(token, self.offset):
            raise ValueError(
                f"malformed witnessAction record near {self.source[self.offset:self.offset + 24]!r}"
            )
        self.offset += len(token)

    def identifier(self) -> str:
        self.skip_space()
        match = re.match(r"[A-Za-z][A-Za-z0-9_]*", self.source[self.offset :])
        if match is None:
            raise ValueError("witnessAction record has an invalid field name")
        self.offset += len(match.group(0))
        return match.group(0)

    def scalar(self) -> Scalar:
        self.skip_space()
        if self.offset >= len(self.source):
            raise ValueError("witnessAction record is missing a field value")
        if self.source[self.offset] == '"':
            self.offset += 1
            start = self.offset
            while self.offset < len(self.source) and self.source[self.offset] != '"':
                if self.source[self.offset] == "\\":
                    raise ValueError("witnessAction strings must not contain escapes")
                self.offset += 1
            if self.offset == len(self.source):
                raise ValueError("witnessAction record has an unterminated string")
            value = self.source[start : self.offset]
            self.offset += 1
            return value
        match = re.match(r"-?[0-9]+", self.source[self.offset :])
        if match is None:
            raise ValueError("witnessAction fields must be strings or integers")
        self.offset += len(match.group(0))
        return int(match.group(0))

    def parse(self) -> dict[str, Scalar]:
        self.take("[")
        result: dict[str, Scalar] = {}
        while True:
            name = self.identifier()
            if name in result:
                raise ValueError(f"witnessAction record repeats field {name!r}")
            self.take("|->")
            result[name] = self.scalar()
            self.skip_space()
            if self.source.startswith("]", self.offset):
                self.offset += 1
                break
            self.take(",")
        self.skip_space()
        if self.offset != len(self.source):
            raise ValueError("witnessAction record has trailing content")
        if set(result) != set(RECORD_FIELDS):
            raise ValueError(
                "witnessAction record fields differ: "
                f"expected {list(RECORD_FIELDS)}, got {sorted(result)}"
            )
        return result


def witness_record(payload: str, state_number: int) -> dict[str, Scalar]:
    """Extract and parse the sole ``witnessAction`` assignment in a state."""

    lines = payload.splitlines()
    starts = [
        index for index, line in enumerate(lines) if line.startswith(WITNESS_ASSIGNMENT)
    ]
    if len(starts) != 1:
        raise ValueError(
            f"state {state_number} has {len(starts)} witnessAction assignments"
        )
    start = starts[0]
    value_lines = [lines[start][len(WITNESS_ASSIGNMENT) :]]
    for line in lines[start + 1 :]:
        if line.startswith("/\\ "):
            break
        value_lines.append(line)
    record = RecordParser("\n".join(value_lines)).parse()
    for field in ("node", "peer", "view"):
        value = record[field]
        if not isinstance(value, int) or isinstance(value, bool):
            raise ValueError(
                f"state {state_number} witnessAction {field} must be an integer"
            )
        if value == -1:
            record[field] = "-"
    return record


def require_integer(value: Scalar, field: str, action: str) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value < 4:
        raise ValueError(f"{action} has invalid four-validator {field} {value!r}")


def require_none(value: Scalar, field: str, action: str) -> None:
    if value != "-":
        raise ValueError(f"{action} requires {field}='-', got {value!r}")


def validate_action(record: dict[str, Scalar], state_number: int) -> ReplayAction:
    """Validate one trace marker against the closed reducer vocabulary."""

    action = record["action"]
    if not isinstance(action, str) or action not in REPLAY_ACTIONS:
        raise ValueError(f"state {state_number} uses unsupported action {action!r}")
    phase = record["phase"]
    subject = record["subject"]
    if phase not in {"-", "Prepare", "Commit"}:
        raise ValueError(f"{action} has invalid phase {phase!r}")
    if subject not in {"-", "A", "B"}:
        raise ValueError(f"{action} has invalid subject {subject!r}")

    nodes = {
        "AssembleLocalBody",
        "BeginTimeout",
        "PersistTimeout",
        "CompleteTimeoutSignature",
        "DeliverTimeout",
        "PersistInstallTC",
        "DeliverTC",
        "BeginInstallTC",
        "BeginLocalProposal",
        "PersistProposal",
        "CompleteProposalSignature",
        "DeliverProposal",
        "FetchBody",
        "StoreBody",
        "ValidateBody",
        "BeginPrepare",
        "PersistPrepare",
        "CompleteVoteSignature",
        "DeliverVote",
        "FormPrepareQC",
        "DeliverQC",
        "BeginObservePrepare",
        "PersistObservePrepare",
        "BeginLockCommit",
        "PersistLockCommit",
        "FormCommitQC",
        "PersistDecision",
    }
    peers = {"DeliverTimeout", "DeliverProposal", "DeliverVote"}
    views = REPLAY_ACTIONS - {"SetGST", "AssembleLocalBody", "FetchBody", "StoreBody"}
    subjects = REPLAY_ACTIONS - {
        "SetGST",
        "BeginTimeout",
        "PersistTimeout",
        "CompleteTimeoutSignature",
        "DeliverTimeout",
        "PersistInstallTC",
        "DeliverTC",
        "BeginInstallTC",
    }
    phases = {
        "BeginPrepare",
        "PersistPrepare",
        "CompleteVoteSignature",
        "DeliverVote",
        "FormPrepareQC",
        "DeliverQC",
        "BeginObservePrepare",
        "PersistObservePrepare",
        "BeginLockCommit",
        "PersistLockCommit",
        "FormCommitQC",
        "PersistDecision",
    }

    for field, present in (
        ("node", action in nodes),
        ("peer", action in peers),
        ("view", action in views),
    ):
        if present:
            require_integer(record[field], field, action)
        else:
            require_none(record[field], field, action)
    if action in subjects:
        if subject not in {"A", "B"}:
            raise ValueError(f"{action} requires a model subject, got {subject!r}")
    else:
        require_none(subject, "subject", action)
    if action in phases:
        if phase not in {"Prepare", "Commit"}:
            raise ValueError(f"{action} requires a vote phase, got {phase!r}")
    else:
        require_none(phase, "phase", action)

    prepare_only = {
        "BeginPrepare",
        "PersistPrepare",
        "FormPrepareQC",
        "BeginObservePrepare",
        "PersistObservePrepare",
        "BeginLockCommit",
        "PersistLockCommit",
    }
    commit_only = {"FormCommitQC", "PersistDecision"}
    if action in prepare_only and phase != "Prepare":
        raise ValueError(f"{action} requires phase='Prepare', got {phase!r}")
    if action in commit_only and phase != "Commit":
        raise ValueError(f"{action} requires phase='Commit', got {phase!r}")

    return ReplayAction(
        action=action,
        node=record["node"],
        peer=record["peer"],
        view=record["view"],
        phase=phase,
        subject=subject,
    )


def normalize(source: str, seed: int) -> list[ReplayAction]:
    """Normalize the exact ``NoDecision`` counterexample in TLC tool output."""

    messages = parse_tool_messages(source)
    starts = [
        message
        for message in messages
        if message.code == 2188 and SIMULATION_START.match(message.payload)
    ]
    if len(starts) != 1:
        raise ValueError(f"expected one TLC simulation-start message, got {len(starts)}")
    if starts[0].severity != 0:
        raise ValueError("TLC simulation-start message has an unexpected severity")
    seed_match = SIMULATION_START.match(starts[0].payload)
    if seed_match is None:
        raise ValueError("TLC simulation-start message lost its seed")
    actual_seed = int(seed_match.group(1))
    if actual_seed != seed:
        raise ValueError(f"TLC simulation seed differs: expected {seed}, got {actual_seed}")

    violations = [message for message in messages if message.code == 2110]
    exact_violation = (
        len(violations) == 1
        and violations[0].severity == 1
        and violations[0].payload.strip() == "Invariant NoDecision is violated."
    )
    if not exact_violation:
        raise ValueError(
            "expected the exact NoDecision invariant violation, "
            f"got {[message.payload.strip() for message in violations]!r}"
        )
    trace_intros = [message for message in messages if message.code == 2121]
    if len(trace_intros) != 1 or trace_intros[0] != ToolMessage(
        2121, 1, "The behavior up to this point is:"
    ):
        raise ValueError("expected the exact TLC counterexample introduction")

    state_messages = [message for message in messages if message.code == 2217]
    if len(state_messages) < 2:
        raise ValueError("TLC counterexample has fewer than two states")

    actions: list[ReplayAction] = []
    for expected_state, message in enumerate(state_messages, 1):
        if message.severity != 4:
            raise ValueError(
                f"state message {expected_state} has unexpected severity {message.severity}"
            )
        lines = message.payload.splitlines()
        if not lines:
            raise ValueError(f"state message {expected_state} is empty")
        header = STATE_HEADER.fullmatch(lines[0])
        if header is None:
            raise ValueError(f"state message {expected_state} has an invalid header")
        actual_state = int(header.group(1))
        if actual_state != expected_state:
            raise ValueError(
                f"non-contiguous TLC state {actual_state}; expected {expected_state}"
            )
        record = witness_record(message.payload, actual_state)
        if actual_state == 1:
            expected_initial: dict[str, Scalar] = {
                "action": "Initial",
                "node": "-",
                "peer": "-",
                "view": "-",
                "phase": "-",
                "subject": "-",
            }
            if record != expected_initial:
                raise ValueError(f"initial witnessAction differs: {record!r}")
        else:
            actions.append(validate_action(record, actual_state))
    if actions[-1].action != "PersistDecision":
        raise ValueError(
            "NoDecision counterexample does not end at PersistDecision: "
            f"last action is {actions[-1].action}"
        )
    return actions


def render(actions: list[ReplayAction], seed: int) -> str:
    """Render normalized replay actions as the checked-in TSV format."""

    lines = [
        "# sumeragi-v2-tlc-action-trace-v1",
        f"# seed={seed}",
        "# step\taction\tnode\tpeer\tview\tphase\tsubject",
    ]
    for index, action in enumerate(actions, 1):
        lines.append(
            "\t".join(
                str(value)
                for value in (
                    index,
                    action.action,
                    action.node,
                    action.peer,
                    action.view,
                    action.phase,
                    action.subject,
                )
            )
        )
    return "\n".join(lines) + "\n"


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, help="TLC 1.7.4 -tool output")
    parser.add_argument("--seed", required=True, type=int, help="TLC simulation seed")
    arguments = parser.parse_args()
    try:
        source = arguments.trace.read_text(encoding="utf-8")
        actions = normalize(source, arguments.seed)
    except (OSError, UnicodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    sys.stdout.write(render(actions, arguments.seed))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
