#!/usr/bin/env python3
"""Normalize a TLC JSON counterexample into reducer-replay actions.

The Sumeragi model intentionally wraps post-GST actions in ``ReliableNext``.
TLC consequently records many action labels as ``ReliableNext`` even though
the state delta is one concrete action from ``SumeragiV2Core``.  This script
uses the closed production/model variable-delta vocabulary to recover that
action and emits a small, reviewable TSV fixture.

The input is produced with TLC's ``-dumpTrace json`` option.  Unknown deltas,
missing fields, non-contiguous state numbers, and ambiguous records fail
closed instead of silently creating a partial replay fixture.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def canonical(value: Any) -> str:
    """Return a stable identity for one TLC JSON value."""

    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def set_delta(before: dict[str, Any], after: dict[str, Any], key: str) -> tuple[list[Any], list[Any]]:
    """Return values added to and removed from a TLC set-valued variable."""

    old = {canonical(value): value for value in before[key]}
    new = {canonical(value): value for value in after[key]}
    added = [new[item] for item in sorted(new.keys() - old.keys())]
    removed = [old[item] for item in sorted(old.keys() - new.keys())]
    return added, removed


def one(values: list[Any], description: str) -> Any:
    """Require exactly one model record."""

    if len(values) != 1:
        raise ValueError(f"expected one {description}, got {len(values)}")
    return values[0]


DELTA_ACTIONS: dict[frozenset[str], str] = {
    frozenset({"pendingTimeout", "signTimeouts", "timeoutIntents"}): "PersistTimeout",
    frozenset({"timeoutNetwork", "signTimeouts"}): "CompleteTimeoutSignature",
    frozenset({"timeoutNetwork", "receivedTimeoutVotes"}): "DeliverTimeout",
    frozenset({"generation", "tcNetwork", "pendingInstallTC", "installedTCs", "nodeView"}): "PersistInstallTC",
    frozenset({"generation", "pendingInstallTC", "installedTCs", "nodeView"}): "PersistInstallTC",
    frozenset({"tcNetwork", "receivedTCs"}): "DeliverTC",
    frozenset({"pendingInstallTC"}): "BeginInstallTC",
    frozenset({"pendingProposal", "signProposals", "proposalIntents"}): "PersistProposal",
    frozenset({"proposalNetwork", "signProposals"}): "CompleteProposalSignature",
    frozenset({"proposalNetwork", "seenProposals"}): "DeliverProposal",
    frozenset({"availableBodies"}): "FetchBody",
    frozenset({"durableBodies", "availableBodies"}): "StoreBody",
    frozenset({"validatedBodies"}): "ValidateBody",
    frozenset({"pendingPrepare"}): "BeginPrepare",
    frozenset({"signVotes", "prepareIntents", "pendingPrepare"}): "PersistPrepare",
    frozenset({"signVotes", "voteNetwork"}): "CompleteVoteSignature",
    frozenset({"voteNetwork", "receivedVotes"}): "DeliverVote",
    frozenset({"qcNetwork", "receivedQCs"}): "DeliverQC",
    frozenset({"pendingObservePrepare"}): "BeginObservePrepare",
    frozenset({"highestSubject", "highestRank", "pendingObservePrepare"}): "PersistObservePrepare",
    frozenset({"pendingLockCommit"}): "BeginLockCommit",
    frozenset(
        {
            "highestSubject",
            "signVotes",
            "pendingLockCommit",
            "lockSubject",
            "highestRank",
            "commitIntents",
            "lockRank",
        }
    ): "PersistLockCommit",
    frozenset({"signVotes", "pendingLockCommit", "lockSubject", "commitIntents", "lockRank"}): "PersistLockCommit",
    frozenset({"decisions", "pendingDecision", "qcNetwork"}): "PersistDecision",
}


def changed_variables(before: dict[str, Any], after: dict[str, Any]) -> frozenset[str]:
    """Return the exact state-variable delta of one TLC step."""

    if before.keys() != after.keys():
        raise ValueError("TLC states expose different variable sets")
    return frozenset(key for key in before if before[key] != after[key])


def record_for_action(
    action: str, before: dict[str, Any], after: dict[str, Any], context: dict[str, Any]
) -> dict[str, Any]:
    """Extract the one record whose fields identify an action."""

    keys = {
        "AssembleLocalBody": ("durableBodies", "added"),
        "BeginTimeout": ("pendingTimeout", "added"),
        "PersistTimeout": ("timeoutIntents", "added"),
        "CompleteTimeoutSignature": ("signTimeouts", "removed"),
        "DeliverTimeout": ("receivedTimeoutVotes", "added"),
        "FormTC": ("pendingInstallTC", "added"),
        "PersistInstallTC": ("installedTCs", "added"),
        "DeliverTC": ("receivedTCs", "added"),
        "BeginInstallTC": ("pendingInstallTC", "added"),
        "BeginLocalProposal": ("pendingProposal", "added"),
        "PersistProposal": ("proposalIntents", "added"),
        "CompleteProposalSignature": ("signProposals", "removed"),
        "DeliverProposal": ("seenProposals", "added"),
        "FetchBody": ("availableBodies", "added"),
        "StoreBody": ("durableBodies", "added"),
        "ValidateBody": ("validatedBodies", "added"),
        "BeginPrepare": ("pendingPrepare", "added"),
        "PersistPrepare": ("prepareIntents", "added"),
        "CompleteVoteSignature": ("signVotes", "removed"),
        "DeliverVote": ("receivedVotes", "added"),
        "FormPrepareQC": ("prepareQCs", "added"),
        "DeliverQC": ("receivedQCs", "added"),
        "BeginObservePrepare": ("pendingObservePrepare", "added"),
        "PersistObservePrepare": ("pendingObservePrepare", "removed"),
        "BeginLockCommit": ("pendingLockCommit", "added"),
        "PersistLockCommit": ("pendingLockCommit", "removed"),
        "FormCommitQC": ("pendingDecision", "added"),
        "PersistDecision": ("decisions", "added"),
    }
    if action == "SetGST":
        return {}
    key, direction = keys[action]
    added, removed = set_delta(before, after, key)
    values = added if direction == "added" else removed
    if not values and context:
        return context
    return one(values, f"{action} record")


def nested(record: dict[str, Any], *names: str) -> dict[str, Any]:
    """Return the first nested record used by the model action."""

    for name in names:
        value = record.get(name)
        if isinstance(value, dict):
            return value
    return record


def fields(action: str, record: dict[str, Any], context: dict[str, Any]) -> tuple[str, str, str, str, str]:
    """Project one model action onto the production replay vocabulary."""

    node = context.get("node", record.get("node"))
    value = record
    if action in {"BeginTimeout", "PersistTimeout", "CompleteTimeoutSignature"}:
        value = nested(record, "vote")
        node = value.get("signer", node)
    elif action == "DeliverTimeout":
        value = nested(record, "vote")
    elif action in {"FormTC", "PersistInstallTC", "DeliverTC", "BeginInstallTC"}:
        value = nested(record, "tc")
    elif action in {"BeginLocalProposal", "CompleteProposalSignature", "DeliverProposal"}:
        value = nested(record, "proposal")
        node = record.get("node", value.get("proposer", node))
    elif action == "PersistProposal":
        node = record.get("proposer", node)
    elif action in {"BeginPrepare", "PersistPrepare", "CompleteVoteSignature", "DeliverVote"}:
        value = nested(record, "vote")
        node = record.get("node", value.get("signer", node))
    elif action in {
        "FormPrepareQC",
        "DeliverQC",
        "BeginObservePrepare",
        "PersistObservePrepare",
        "BeginLockCommit",
        "PersistLockCommit",
        "FormCommitQC",
        "PersistDecision",
    }:
        value = nested(record, "qc")

    if action in {"AssembleLocalBody", "FetchBody", "StoreBody", "ValidateBody"}:
        node = record.get("node", node)

    peer: Any = None
    if action in {"DeliverTimeout", "DeliverVote"}:
        peer = value.get("signer")
    elif action == "DeliverProposal":
        peer = value.get("proposer")

    view = context.get("roundView", value.get("view"))
    phase = value.get("phase")
    subject = context.get("subject", value.get("subject"))

    def render(item: Any) -> str:
        return "-" if item is None else str(item)

    return tuple(render(item) for item in (node, peer, view, phase, subject))


def normalize(document: dict[str, Any]) -> list[tuple[int, str, str, str, str, str, str]]:
    """Normalize every transition in one TLC counterexample."""

    try:
        transitions = document["counterexample"]["action"]
    except (KeyError, TypeError) as error:
        raise ValueError("input is not a TLC JSON counterexample") from error
    if not isinstance(transitions, list) or not transitions:
        raise ValueError("TLC counterexample has no transitions")

    output = []
    expected_state = 1
    for index, transition in enumerate(transitions, 1):
        if not isinstance(transition, list) or len(transition) != 3:
            raise ValueError(f"transition {index} has an invalid shape")
        before_frame, metadata, after_frame = transition
        if before_frame[0] != expected_state or after_frame[0] != expected_state + 1:
            raise ValueError(f"transition {index} has non-contiguous state numbers")
        expected_state += 1
        before, after = before_frame[1], after_frame[1]
        raw_name = metadata.get("name")
        context = metadata.get("context", {})
        delta = changed_variables(before, after)
        if raw_name == "ReliableNext":
            try:
                action = DELTA_ACTIONS[delta]
            except KeyError as error:
                raise ValueError(
                    f"transition {index} has unknown ReliableNext delta {sorted(delta)}"
                ) from error
        elif raw_name == "ReliableBeginTimeout":
            action = "BeginTimeout"
        else:
            action = raw_name
        if action not in {
            "SetGST",
            "AssembleLocalBody",
            "BeginTimeout",
            "PersistTimeout",
            "CompleteTimeoutSignature",
            "DeliverTimeout",
            "FormTC",
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
        }:
            raise ValueError(f"transition {index} uses unsupported action {action!r}")
        record = record_for_action(action, before, after, context)
        output.append((index, action, *fields(action, record, context)))
    return output


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, help="TLC -dumpTrace json output")
    parser.add_argument("--seed", required=True, type=int, help="TLC simulation seed")
    arguments = parser.parse_args()
    try:
        document = json.loads(arguments.trace.read_text(encoding="utf-8"))
        actions = normalize(document)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    print("# sumeragi-v2-tlc-action-trace-v1")
    print(f"# seed={arguments.seed}")
    print("# step\taction\tnode\tpeer\tview\tphase\tsubject")
    for row in actions:
        print("\t".join(str(value) for value in row))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
