"""Runner-side durable control bridge for one retained benchmark session.

Prerequisites are a canonical prepared session, an already durable session
start, owner-only RecordDirectory, and the canonical semantic validators.
This library never spawns networks, synthesizes measurements, or asserts process
exit. The adapter owns deadlines, native measurements and worker cleanup.

The session runtime composes this bridge with native measurement semantics and
the joined closure reducer before the campaign may dispatch another owner.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
from typing import Any, BinaryIO, Callable, Mapping

import private_settlement_session_control as control


@dataclass(frozen=True)
class CompletionDecision:
    """Semantic validator result; no classification is inferred from exit/text."""

    kind: str
    validation: dict[str, Any]
    sample: dict[str, Any] | None

    def validate(self) -> None:
        control.require(self.kind in {"succeeded", "failed", "timed_out", "incomplete"},
                        "unknown validated attempt outcome")
        control.reference(self.validation)
        control.require((self.sample is not None) == (self.kind == "succeeded"),
                        "only successful validation has a sample")
        if self.sample is not None:
            control.reference(self.sample)


@dataclass(frozen=True)
class SessionExchange:
    """Retained exchange facts, never precomputed accounting or quiescence.

    Even all_attempts_accepted does not authorize another campaign job: the
    lifecycle owner must first authenticate adapter/worker exit and cleanup."""

    started: tuple[dict[str, Any], ...]
    acknowledgements: tuple[dict[str, Any], ...]
    worker_terminal: dict[str, Any]
    all_attempts_accepted: bool


class SessionBridgeFailure(RuntimeError):
    """Continuation stopped; durable files remain for incomplete accounting."""

    def __init__(self, started: list[dict[str, Any]], written: list[dict[str, Any]]):
        super().__init__("session exchange did not complete; inspect retained typed evidence")
        self.started = tuple(dict(row) for row in started)
        self.acknowledgements = tuple(dict(row) for row in written)


class _BeforeWrite:
    """Run one barrier after journal publication and before any pipe byte."""

    def __init__(self, stream: BinaryIO, before: Callable[[bytes], None]):
        self.stream, self.before = stream, before
        self.checked = False

    def write(self, raw: bytes | memoryview) -> int:
        if not self.checked:
            self.before(bytes(raw))
            self.checked = True
        return self.stream.write(raw)

    def flush(self) -> None:
        self.stream.flush()


class RunnerSessionBridge:
    """Own ordered dispatch/acceptance for one adapter pipe pair.

The supplied streams must enforce the existing sealed deadlines. A bridge
failure poisons continuation; the lifecycle owner closes its pipes, retains
actual cleanup/exit facts and does not start another campaign job.
"""

    def __init__(self, prepared: Mapping[str, Any], session_started: dict[str, Any], *,
                 records: control.RecordDirectory, reader: BinaryIO, writer: BinaryIO):
        self.records, self.reader, self.writer = records, reader, writer
        self.prepared = control.decode(control.canonical(dict(prepared)))
        self.identity = control.identity(self.prepared["identity"])
        self.request_raw = records.read(self.prepared["reference"])
        control.require(self.request_raw == control.canonical(self.prepared["request"])
                        and self.prepared["reference"]["sha256"] == self.identity["session_request_sha256"],
                        "session request differs from its immutable identity")
        self.started_ref = dict(control.reference(session_started))
        self.started_raw = records.read(self.started_ref)
        started = control.decode(self.started_raw)
        control.require(all(started.get(key) == value for key, value in self.identity.items())
                        and started.get("request") == self.prepared["reference"],
                        "session start belongs to a different request")
        self.attempts = self.prepared["request"]["attempts"]
        control.require(type(self.attempts) is list and 0 < len(self.attempts) <= 1_000_000,
                        "session has no bounded attempt inventory")
        for index, row in enumerate(self.attempts):
            control.attempt({key: row[key] for key in control.ATTEMPT_FIELDS})
            control.require(row["session_attempt_index"] == index, "session attempt order differs")
            records.read(row["request"])
        for key in ("request_id", "invocation_nonce", "attempt_id"):
            control.require(len({row[key] for row in self.attempts}) == len(self.attempts),
                            "session repeats an attempt identity")
        self.prefix = f"sessions/{self.identity['session_id']}/control"
        self.incoming = control.ControlChain(self.identity, self.started_ref["sha256"],
            "runner_adapter", "child_to_owner", records, observer="runner", journal_prefix=self.prefix)
        self.outgoing = control.ControlChain(self.identity, self.started_ref["sha256"],
            "runner_adapter", "owner_to_child", records, observer="runner", journal_prefix=self.prefix)
        self.gate = control.AttemptGate(self.identity, len(self.attempts))
        self.started: list[dict[str, Any]] = []
        self.written: list[dict[str, Any]] = []
        self.used = False

    def _stable(self) -> None:
        control.require(self.records.read(self.prepared["reference"]) == self.request_raw
                        and self.records.read(self.started_ref) == self.started_raw,
                        "session request or start changed during execution")

    def _receive(self) -> control.RetainedMessage:
        message = self.incoming.receive(self.reader)
        ref = message.decoded()["forwarded_from"]
        upstream = control.RetainedMessage(self.records.read(ref), ref)
        control.verify_forwarded(message, upstream)
        return message

    def _bound_validate(self, refs: dict[str, Any], validate: Callable[[dict[str, bytes]], Any]) -> Any:
        bound = {key: self.records.read(ref) for key, ref in refs.items() if ref is not None}
        result = validate(bound)
        control.require(all(self.records.read(refs[key]) == raw for key, raw in bound.items()),
                        "semantic evidence changed while validating")
        self._stable()
        return result

    def _retained_outgoing(self, raw: bytes) -> control.RetainedMessage:
        name = (f"{self.prefix}/runner.runner_adapter.owner_to_child."
                f"{self.outgoing.sequence:020d}.frame")
        ref = {"path": name, "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
        control.require(self.records.read(ref) == raw, "outgoing frame was not retained exactly")
        return control.RetainedMessage(raw, ref)

    def _ack_phase(self, frame: control.RetainedMessage, phase: str) -> dict[str, Any]:
        payload = frame.decoded()["payload"]
        row = {**self.identity, **{key: payload[key] for key in control.ATTEMPT_FIELDS},
               "acknowledgement": frame.binding, "phase": phase}
        return self.records.publish(
            f"{self.prefix}/ack-{payload['session_attempt_index']:06d}-{phase}.json", control.canonical(row))

    def _finish_terminal(self, terminal: control.RetainedMessage, complete: bool,
                         validate: Callable[[dict[str, bytes], tuple[str, ...], bool], None]) -> SessionExchange:
        control.require(terminal.decoded()["kind"] == "session_completed",
                        "session lacks its terminal closure message")
        refs = terminal.decoded()["payload"]
        accepted = tuple(row["request_id"] for row in self.attempts[:len(self.written)])
        self._bound_validate(refs, lambda bound: validate(bound, accepted, complete))
        return SessionExchange(tuple(self.started), tuple(self.written),
                               dict(refs["worker_terminal"]), complete)

    def run(self, *,
            publish_start: Callable[[int, dict[str, Any] | None], dict[str, Any]],
            validate_ready: Callable[[dict[str, bytes]], None],
            validate_completion: Callable[[int, dict[str, bytes]], CompletionDecision],
            validate_acceptance: Callable[[int, dict[str, bytes]], None],
            validate_terminal: Callable[[dict[str, bytes], tuple[str, ...], bool], None]) -> SessionExchange:
        """Drive all planned attempts, stopping at the first unsuccessful result.

Validators are mandatory and receive authentic record bytes, including floats
in successful sample records. Control JSON is never used to parse metrics.
No successful-result omission or implicit zero-valued counters are produced.
"""
        control.require(not self.used, "session bridge cannot be reused or retried")
        self.used = True
        try:
            ready = self._receive()
            if ready.decoded()["kind"] == "session_completed":
                # An authenticated setup terminal precedes every attempt start.
                # Its typed reason and process cleanup still require validation.
                self.gate.stop()
                return self._finish_terminal(ready, False, validate_terminal)
            control.require(ready.decoded()["kind"] == "ready", "session did not establish readiness")
            self._bound_validate(ready.decoded()["payload"], validate_ready)
            self.gate.ready()
            predecessor = None
            all_accepted = True
            for index, attempt in enumerate(self.attempts):
                self._stable()
                start = publish_start(index, predecessor)
                self.started.append(dict(start))
                request_raw = self.records.read(attempt["request"])
                start_raw = self.records.read(start)
                payload = {key: attempt[key] for key in control.ATTEMPT_FIELDS}
                payload.update(request=attempt["request"], attempt_started=start)
                def dispatch(raw: bytes) -> None:
                    self.gate.dispatch(self._retained_outgoing(raw), expected_request=request_raw,
                                       expected_start=start_raw, records=self.records)
                self.outgoing.send(_BeforeWrite(self.writer, dispatch), "dispatch", payload)
                completed = self._receive()
                if completed.decoded()["kind"] == "session_completed":
                    # A durable start with no attempt terminal is not silently
                    # converted to failed or not-started by the bridge.
                    self.gate.stop()
                    return self._finish_terminal(completed, False, validate_terminal)
                self.gate.complete(completed)
                refs = {key: ref for key, ref in completed.decoded()["payload"].items()
                        if key not in control.ATTEMPT_FIELDS}
                decision = self._bound_validate(refs, lambda bound: validate_completion(index, bound))
                control.require(type(decision) is CompletionDecision, "semantic validator returned no typed decision")
                decision.validate()
                if decision.kind != "succeeded":
                    self.records.read(decision.validation)
                    self.outgoing.send(self.writer, "stop", {
                        "active_attempt_id": attempt["attempt_id"],
                        "reason": f"attempt_{decision.kind}", "validation": decision.validation})
                    self.gate.stop()
                    all_accepted = False
                    break
                control.require(refs.get("adapter_outcome") is not None and refs.get("response") is not None,
                                "successful completion omitted required records")
                accept_payload = {**completed.decoded()["payload"],
                                  "validation": decision.validation, "sample": decision.sample}
                def validate_before_write(raw: bytes) -> None:
                    frame = self._retained_outgoing(raw)
                    self._ack_phase(frame, "proposed")
                    self.gate.accept(frame, records=self.records,
                                     validate=lambda bound: validate_acceptance(index, bound))
                    self._stable()
                    self._ack_phase(frame, "validated")
                accepted = self.outgoing.send(_BeforeWrite(self.writer, validate_before_write),
                                              "accept", accept_payload)
                # This means every byte reached the pipe and flush returned. It
                # does not assert the worker consumed the ACK or exited.
                written = self._ack_phase(accepted, "pipe-written")
                self.written.append(written)
                predecessor = accepted.binding
            terminal = self._receive()
            return self._finish_terminal(terminal, all_accepted, validate_terminal)
        except BaseException as error:
            self.gate.phase = "stopped"
            self.incoming.poisoned = self.outgoing.poisoned = True
            raise SessionBridgeFailure(self.started, self.written) from error
