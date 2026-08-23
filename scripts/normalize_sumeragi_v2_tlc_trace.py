#!/usr/bin/env python3
"""Normalize the one pinned TLC 1.7.4 replay witness, failing closed.

The input is the complete stdout of the replay TLC invocation in ``-tool``
mode. This parser validates the complete message order, the SANY module
census, every state heading, the simulation seed/aril and the terminal
statistics before projecting ``witnessAction`` records to the checked-in TSV.
It intentionally accepts no legacy or heuristic transcript form.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Union


MESSAGE_START = re.compile(r"^@!@!@STARTMSG ([0-9]+):([0-9]+) @!@!@$", re.ASCII)
MESSAGE_END = re.compile(r"^@!@!@ENDMSG ([0-9]+) @!@!@$", re.ASCII)
ASSIGNMENT_START = re.compile(r"^/\\ ([A-Za-z][A-Za-z0-9_]*) = ", re.ASCII)
PARSE_LINE = re.compile(r"^Parsing file (.+[/\\])?([A-Za-z][A-Za-z0-9_]*)\.tla$", re.ASCII)
SEMANTIC_LINE = re.compile(
    r"^Semantic processing of module ([A-Za-z][A-Za-z0-9_]*)$", re.ASCII
)
TIMESTAMP = r"[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}"
STARTING = re.compile(rf"^Starting\.\.\. \({TIMESTAMP}\)$", re.ASCII)
FINISHED = re.compile(
    rf"^Finished in (([0-9]+d )?([0-9]+h )?([0-9]+min )?[0-9]+(ms|s)|"
    rf"([0-9]+d )?([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|[0-9]+d) "
    rf"at \({TIMESTAMP}\)$",
    re.ASCII,
)
SIMULATION = re.compile(
    r"^Running Random Simulation with seed (-?[0-9]+) with 1 worker on "
    r"[1-9][0-9]* cores with [0-9]+MB heap and [0-9]+MB offheap memory"
    r"(?: \[pid: [1-9][0-9]*\])? \(.+\)\.$",
    re.ASCII,
)
PROGRESS = re.compile(
    rf"^Progress\(-1\) at {TIMESTAMP}: ([1-9][0-9,]*) states generated, "
    r"-1 distinct states found, -1 states left on queue\.$",
    re.ASCII,
)
STATISTICS = re.compile(
    r"^The number of states generated: ([1-9][0-9,]*)\n"
    r"Simulation using seed (-?[0-9]+) and aril (-?[0-9]+)$",
    re.ASCII,
)
DIAGNOSTIC = re.compile(
    r"(?im)^[ \t]*(?:error:|warning:|fatal(?: error)?:|exception(?: in thread)?\b|"
    r"caused by:|suppressed:|deadlock reached(?:\.|$)|temporal properties were "
    r"violated\.|[A-Za-z_$][A-Za-z0-9_.$]*(?:Exception|Error)(?::|$))"
)
WITNESS_ASSIGNMENT = "/\\ witnessAction = "
TLC_VERSION = "TLC2 Version 2.19 of 08 August 2024 (rev: 5a47802)"
EXPECTED_STATES = 101
EXPECTED_ACTIONS = 100

# DFS parse order and dependency-order semantic phase for this pinned closure.
SANY_PARSE_ORDER = (
    "SumeragiV2TraceWitness",
    "SumeragiV2Inductive",
    "SumeragiV2Reconfiguration",
    "SumeragiV2SafetyDefinitions",
    "SumeragiV2CrashRecovery",
    "SumeragiV2Core",
    "SumeragiV2Availability",
    "Sequences",
    "SumeragiV2Quorums",
    "Naturals",
    "Integers",
    "FiniteSets",
)
SANY_SEMANTIC_ORDER = (
    "Naturals",
    "Integers",
    "Sequences",
    "FiniteSets",
    "SumeragiV2Quorums",
    "SumeragiV2Availability",
    "SumeragiV2Core",
    "SumeragiV2CrashRecovery",
    "SumeragiV2Reconfiguration",
    "SumeragiV2SafetyDefinitions",
    "SumeragiV2Inductive",
    "SumeragiV2TraceWitness",
)

# Python 3.9 does not provide ``typing.TypeAlias`` or runtime PEP 604 unions.
Scalar = Union[int, str]

REPLAY_ACTIONS = frozenset(
    {
        "SetGST", "AssembleLocalBody", "BeginTimeout", "PersistTimeout",
        "CompleteTimeoutSignature", "DeliverTimeout", "PersistInstallTC",
        "DeliverTC", "BeginInstallTC", "BeginLocalProposal", "PersistProposal",
        "CompleteProposalSignature", "DeliverProposal", "FetchBody", "StoreBody",
        "ValidateBody", "BeginPrepare", "PersistPrepare", "CompleteVoteSignature",
        "DeliverVote", "FormPrepareQC", "DeliverQC", "BeginObservePrepare",
        "PersistObservePrepare", "BeginLockCommit", "PersistLockCommit",
        "FormCommitQC", "PersistDecision",
    }
)
RECORD_FIELDS = ("action", "node", "peer", "view", "phase", "subject")


@dataclass(frozen=True)
class ToolMessage:
    """One complete delimited TLC tool message."""

    code: int
    severity: int
    payload: str
    line: int


@dataclass(frozen=True)
class RawLine:
    """One nonblank SANY line outside TLC message delimiters."""

    payload: str
    line: int


@dataclass(frozen=True)
class ReplayAction:
    """One validated action projected onto the production replay columns."""

    action: str
    node: Scalar
    peer: Scalar
    view: Scalar
    phase: str
    subject: str


TranscriptItem = Union[ToolMessage, RawLine]


def _validate_characters(source: str) -> None:
    """Reject characters which can conceal framing or diagnostics."""

    for offset, character in enumerate(source):
        codepoint = ord(character)
        if (codepoint < 32 and character not in "\t\n") or 127 <= codepoint <= 159:
            raise ValueError(
                f"input contains forbidden control U+{codepoint:04X} at offset {offset}"
            )
        if character.isspace() and character not in " \t\n":
            raise ValueError(
                f"input contains non-ASCII whitespace U+{codepoint:04X} at offset {offset}"
            )


def parse_transcript(source: str) -> list[TranscriptItem]:
    """Parse all tool frames while preserving every raw nonblank line."""

    _validate_characters(source)
    items: list[TranscriptItem] = []
    current_code: Union[int, None] = None
    current_severity = 0
    current_line = 0
    payload: list[str] = []
    for line_number, line in enumerate(source.split("\n"), 1):
        start = MESSAGE_START.fullmatch(line)
        end = MESSAGE_END.fullmatch(line)
        if "@!@!@" in line and start is None and end is None:
            raise ValueError(f"malformed TLC tool-message framing at line {line_number}")
        if start is not None:
            if current_code is not None:
                raise ValueError(f"nested TLC tool message at line {line_number}")
            current_code = int(start.group(1))
            current_severity = int(start.group(2))
            current_line = line_number
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
            joined = "\n".join(payload)
            if DIAGNOSTIC.search(joined):
                raise ValueError(
                    f"hidden diagnostic in TLC message {current_code} at line {current_line}"
                )
            items.append(ToolMessage(current_code, current_severity, joined, current_line))
            current_code = None
            current_severity = 0
            current_line = 0
            payload = []
        elif current_code is not None:
            payload.append(line)
        elif line:
            if DIAGNOSTIC.search(line):
                raise ValueError(f"hidden diagnostic outside tool framing at line {line_number}")
            items.append(RawLine(line, line_number))
    if current_code is not None:
        raise ValueError(f"unterminated TLC tool message {current_code}")
    if not any(isinstance(item, ToolMessage) for item in items):
        raise ValueError("input has no TLC tool messages")
    return items


def parse_tool_messages(source: str) -> list[ToolMessage]:
    """Return tool frames after validating all raw and framed content."""

    return [item for item in parse_transcript(source) if isinstance(item, ToolMessage)]


class RecordParser:
    """Parser for the scalar-only TLA record used by ``witnessAction``."""

    def __init__(self, source: str) -> None:
        self.source = source
        self.offset = 0

    def skip_space(self) -> None:
        while self.offset < len(self.source) and self.source[self.offset] in " \t\n":
            self.offset += 1

    def take(self, token: str) -> None:
        self.skip_space()
        if not self.source.startswith(token, self.offset):
            raise ValueError(
                "malformed witnessAction record near "
                f"{self.source[self.offset:self.offset + 24]!r}"
            )
        self.offset += len(token)

    def identifier(self) -> str:
        self.skip_space()
        match = re.match(r"[A-Za-z][A-Za-z0-9_]*", self.source[self.offset :], re.ASCII)
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
        match = re.match(r"-?[0-9]+", self.source[self.offset :], re.ASCII)
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


def _validate_state_body(payload: str, state_number: int) -> list[str]:
    lines = payload.split("\n")
    if not lines or not lines[0]:
        raise ValueError(f"state {state_number} has no heading")
    if state_number == 1:
        heading_matches = lines[0] == "1: <Initial predicate>"
    else:
        heading_matches = re.fullmatch(
            rf"{state_number}: <(?:WitnessMark|WitnessNext) line [1-9][0-9]*, "
            r"col [1-9][0-9]* to line [1-9][0-9]*, col [1-9][0-9]* of "
            r"module SumeragiV2TraceWitness>",
            lines[0],
            re.ASCII,
        ) is not None
    if not heading_matches:
        raise ValueError(f"state {state_number} heading differs: got {lines[0]!r}")
    # TLC 1.7.4 emits one empty payload line before each state-frame end.
    if len(lines) < 3 or lines[-1] != "":
        raise ValueError(f"state {state_number} lacks its terminal payload line")
    lines.pop()
    if not lines[1:]:
        raise ValueError(f"state {state_number} has no assignments")
    assignment_names: set[str] = set()
    saw_assignment = False
    for line_number, line in enumerate(lines[1:], 2):
        if re.match(r"^[0-9]+: <.*>$", line, re.ASCII):
            raise ValueError(f"state {state_number} contains an extra state heading")
        match = ASSIGNMENT_START.match(line)
        if match is not None:
            saw_assignment = True
            name = match.group(1)
            if name in assignment_names:
                raise ValueError(f"state {state_number} repeats assignment {name!r}")
            assignment_names.add(name)
        elif not saw_assignment or not line.startswith((" ", "\t")):
            raise ValueError(
                f"state {state_number} has malformed assignment framing at payload line "
                f"{line_number}"
            )
    return lines


def witness_record(payload: str, state_number: int) -> dict[str, Scalar]:
    """Extract and parse the sole ``witnessAction`` assignment in a state."""

    lines = _validate_state_body(payload, state_number)
    starts = [index for index, line in enumerate(lines) if line.startswith(WITNESS_ASSIGNMENT)]
    if len(starts) != 1:
        raise ValueError(f"state {state_number} has {len(starts)} witnessAction assignments")
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
            raise ValueError(f"state {state_number} witnessAction {field} must be an integer")
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

    nodes = REPLAY_ACTIONS - {"SetGST"}
    peers = {"DeliverTimeout", "DeliverProposal", "DeliverVote"}
    views = REPLAY_ACTIONS - {"SetGST", "AssembleLocalBody", "FetchBody", "StoreBody"}
    subjects = REPLAY_ACTIONS - {
        "SetGST", "BeginTimeout", "PersistTimeout", "CompleteTimeoutSignature",
        "DeliverTimeout", "PersistInstallTC", "DeliverTC", "BeginInstallTC",
    }
    phases = {
        "BeginPrepare", "PersistPrepare", "CompleteVoteSignature", "DeliverVote",
        "FormPrepareQC", "DeliverQC", "BeginObservePrepare", "PersistObservePrepare",
        "BeginLockCommit", "PersistLockCommit", "FormCommitQC", "PersistDecision",
    }

    for field, present in (
        ("node", action in nodes), ("peer", action in peers), ("view", action in views)
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
        "BeginPrepare", "PersistPrepare", "FormPrepareQC", "BeginObservePrepare",
        "PersistObservePrepare", "BeginLockCommit", "PersistLockCommit",
    }
    if action in prepare_only and phase != "Prepare":
        raise ValueError(f"{action} requires phase='Prepare', got {phase!r}")
    if action in {"FormCommitQC", "PersistDecision"} and phase != "Commit":
        raise ValueError(f"{action} requires phase='Commit', got {phase!r}")
    return ReplayAction(action, record["node"], record["peer"], record["view"], phase, subject)


def _require_message(
    items: list[TranscriptItem], cursor: int, code: int, severity: int, payload: str
) -> int:
    if cursor >= len(items):
        raise ValueError(f"transcript ended before TLC message {code}")
    item = items[cursor]
    if not isinstance(item, ToolMessage) or (
        item.code, item.severity, item.payload
    ) != (code, severity, payload):
        raise ValueError(
            f"transcript item {cursor + 1} differs; expected message "
            f"{code}:{severity} {payload!r}, got {item!r}"
        )
    return cursor + 1


def _validate_sany(items: list[TranscriptItem], cursor: int) -> int:
    for expected_module in SANY_PARSE_ORDER:
        if cursor >= len(items) or not isinstance(items[cursor], RawLine):
            raise ValueError(f"SANY parse census ended before {expected_module}")
        match = PARSE_LINE.fullmatch(items[cursor].payload)
        actual = match.group(2) if match is not None else None
        if actual != expected_module:
            raise ValueError(
                f"SANY parse order differs: expected {expected_module}, got "
                f"{items[cursor].payload!r}"
            )
        cursor += 1
    for expected_module in SANY_SEMANTIC_ORDER:
        if cursor >= len(items) or not isinstance(items[cursor], RawLine):
            raise ValueError(f"SANY semantic census ended before {expected_module}")
        match = SEMANTIC_LINE.fullmatch(items[cursor].payload)
        actual = match.group(1) if match is not None else None
        if actual != expected_module:
            raise ValueError(
                f"SANY semantic order differs: expected {expected_module}, got "
                f"{items[cursor].payload!r}"
            )
        cursor += 1
    return cursor


def normalize(source: str, seed: int, aril: int) -> list[ReplayAction]:
    """Validate and normalize the exact 101-state ``NoDecision`` transcript."""

    items = parse_transcript(source)
    cursor = _require_message(items, 0, 2262, 0, TLC_VERSION)

    if cursor >= len(items) or not isinstance(items[cursor], ToolMessage):
        raise ValueError("transcript is missing the simulation banner")
    banner = items[cursor]
    banner_match = SIMULATION.fullmatch(banner.payload)
    if banner.code != 2188 or banner.severity != 0 or banner_match is None:
        raise ValueError(f"unexpected TLC simulation banner: {banner!r}")
    if int(banner_match.group(1)) != seed:
        raise ValueError(
            f"TLC simulation seed differs: expected {seed}, got {banner_match.group(1)}"
        )
    cursor += 1

    cursor = _require_message(items, cursor, 2220, 0, "Starting SANY...")
    cursor = _validate_sany(items, cursor)
    cursor = _require_message(items, cursor, 2219, 0, "SANY finished.")

    if cursor >= len(items) or not isinstance(items[cursor], ToolMessage):
        raise ValueError("transcript is missing the TLC starting message")
    starting = items[cursor]
    if starting.code != 2185 or starting.severity != 0 or not STARTING.fullmatch(starting.payload):
        raise ValueError(f"unexpected TLC starting message: {starting!r}")
    cursor += 1
    cursor = _require_message(items, cursor, 2269, 0, "Computed 1 initial states...")
    cursor = _require_message(items, cursor, 2110, 1, "Invariant NoDecision is violated.")
    cursor = _require_message(items, cursor, 2121, 1, "The behavior up to this point is:")

    actions: list[ReplayAction] = []
    for state_number in range(1, EXPECTED_STATES + 1):
        if cursor >= len(items) or not isinstance(items[cursor], ToolMessage):
            raise ValueError(f"transcript ended before state {state_number}")
        state_message = items[cursor]
        if state_message.code != 2217 or state_message.severity != 4:
            raise ValueError(f"state {state_number} message differs: got {state_message!r}")
        record = witness_record(state_message.payload, state_number)
        if state_number == 1:
            expected_initial: dict[str, Scalar] = {
                "action": "Initial", "node": "-", "peer": "-", "view": "-",
                "phase": "-", "subject": "-",
            }
            if record != expected_initial:
                raise ValueError(f"initial witnessAction differs: {record!r}")
        else:
            actions.append(validate_action(record, state_number))
        cursor += 1

    if len(actions) != EXPECTED_ACTIONS:
        raise ValueError(f"replay action count differs: expected {EXPECTED_ACTIONS}, got {len(actions)}")
    if actions[-1].action != "PersistDecision":
        raise ValueError(
            "NoDecision counterexample does not end at PersistDecision: "
            f"last action is {actions[-1].action}"
        )

    if cursor >= len(items) or not isinstance(items[cursor], ToolMessage):
        raise ValueError("transcript is missing simulation progress")
    progress = items[cursor]
    progress_match = PROGRESS.fullmatch(progress.payload)
    if progress.code != 2209 or progress.severity != 0 or progress_match is None:
        raise ValueError(f"unexpected simulation progress message: {progress!r}")
    generated = int(progress_match.group(1).replace(",", ""))
    cursor += 1

    if cursor >= len(items) or not isinstance(items[cursor], ToolMessage):
        raise ValueError("transcript is missing simulation statistics")
    statistics = items[cursor]
    stats_match = STATISTICS.fullmatch(statistics.payload)
    if statistics.code != 2210 or statistics.severity != 0 or stats_match is None:
        raise ValueError(f"unexpected simulation statistics message: {statistics!r}")
    stats_generated = int(stats_match.group(1).replace(",", ""))
    if stats_generated < generated:
        raise ValueError(
            "simulation generated-state count regressed before statistics: "
            f"{generated} vs {stats_generated}"
        )
    if int(stats_match.group(2)) != seed:
        raise ValueError(
            f"simulation statistics seed differs: expected {seed}, got {stats_match.group(2)}"
        )
    if int(stats_match.group(3)) != aril:
        raise ValueError(f"simulation aril differs: expected {aril}, got {stats_match.group(3)}")
    cursor += 1

    if cursor >= len(items) or not isinstance(items[cursor], ToolMessage):
        raise ValueError("transcript is missing final simulation progress")
    final_progress = items[cursor]
    final_progress_match = PROGRESS.fullmatch(final_progress.payload)
    if (
        final_progress.code != 2209
        or final_progress.severity != 0
        or final_progress_match is None
    ):
        raise ValueError(f"unexpected final simulation progress message: {final_progress!r}")
    final_generated = int(final_progress_match.group(1).replace(",", ""))
    if final_generated < stats_generated:
        raise ValueError(
            "simulation generated-state count regressed after statistics: "
            f"{stats_generated} vs {final_generated}"
        )
    cursor += 1

    if cursor >= len(items) or not isinstance(items[cursor], ToolMessage):
        raise ValueError("transcript is missing the terminal TLC message")
    terminal = items[cursor]
    if terminal.code != 2186 or terminal.severity != 0 or not FINISHED.fullmatch(terminal.payload):
        raise ValueError(f"unexpected terminal TLC message: {terminal!r}")
    cursor += 1
    if cursor != len(items):
        raise ValueError(f"transcript has {len(items) - cursor} item(s) after termination")
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
                    index, action.action, action.node, action.peer, action.view,
                    action.phase, action.subject,
                )
            )
        )
    return "\n".join(lines) + "\n"


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(
        description="Validate and normalize the pinned Sumeragi V2 TLC witness"
    )
    parser.add_argument("trace", type=Path, help="complete TLC 1.7.4 -tool stdout")
    parser.add_argument("--seed", required=True, type=int, help="exact TLC seed")
    parser.add_argument("--aril", required=True, type=int, help="exact TLC aril")
    arguments = parser.parse_args()
    try:
        source = arguments.trace.read_text(encoding="utf-8")
        actions = normalize(source, arguments.seed, arguments.aril)
    except (OSError, UnicodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    sys.stdout.write(render(actions, arguments.seed))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
