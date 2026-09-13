"""Compose one native benchmark worker, in-process adapter, and runner bridge.

The caller retains this owner on every failure. An adapter thread finishing is
not process quiescence; a mandatory evidence validator authenticates the final
worker lifecycle before another registered campaign job can start.
The session execution owner supplies the admitted worker, semantic validators
and retained closure; protocol fixtures do not qualify native measurements.
"""
from __future__ import annotations

import os
import threading

import private_settlement_session_adapter as adapter
import private_settlement_session_bridge as bridge
import private_settlement_session_control as control
import private_settlement_session_semantics as semantics
import private_settlement_release_runner as runner


class SessionRuntimeIncomplete(RuntimeError):
    """Keep the actual process, pump, and observation owners recoverable."""

    def __init__(self, runtime):
        self.runtime = runtime
        super().__init__('retained session incomplete; actual native owners remain available')


def spawn_bound_worker(prepared, started, *, records, cwd, images, runtime_root, outer_timeout_ms):
    """Refuse a substituted native command/image before creating a process."""
    image = images['worker']
    image.validate()
    start = control.decode(records.read(started))
    command = [str(image.path), adapter.WORKER_TEST, '--exact', '--ignored', '--nocapture', '--test-threads=1']
    binding = {'sha256': image.sha256, 'bytes': image.path.stat().st_size}
    control.require(control.canonical(start['command']) == control.canonical(command)
                    and control.canonical(start['harness']) == control.canonical(binding),
                    'durable session owner differs from its admitted native executable')
    return adapter.spawn_native_session(prepared, started, records=records, cwd=cwd,
        images=images, runtime_root=runtime_root, outer_timeout_ms=outer_timeout_ms)


class SessionRuntime:
    """Own the complete local pipe composition without introducing another process.

``worker`` must be the actual NativeWorker, including after a failed startup.
No method signals it, invents an exit, or restarts a failed registered session.
The adapter and bridge have distinct record-directory owners over the same
immutable evidence tree. Each consumes deadlines from the same durable starts;
the runner cannot extend an adapter's preceding ACK-validation deadline.
"""

    def __init__(self, prepared, started, *, records, worker, observations,
                 packet_owner, semantic_owner, outer_timeout_ms):
        self.records, self.worker = records, worker
        self.prepared, self.started = prepared, started
        self.observations, self.semantic_owner = observations, semantic_owner
        self.used, self.exchange, self.lifecycle, self.adapter_error = False, None, None, None
        self.thread = self.pipes = self.adapter_records = None
        self.adapter_reader = self.adapter_writer = self.owner = None
        try:
            self.adapter_records = control.RecordDirectory(records.path)
            self.pipes = control.ChildControlPipes()
            read_fd, write_fd = self.pipes.child_fds
            self.adapter_reader = os.fdopen(os.dup(read_fd), 'rb', buffering=0)
            self.adapter_writer = os.fdopen(os.dup(write_fd), 'wb', buffering=0)
            self.pipes.child_spawn_finished()
            self.owner = adapter.SessionAdapter(prepared, started, records=self.adapter_records,
                runner_reader=self.adapter_reader, runner_writer=self.adapter_writer,
                worker=worker, observations=observations, packets=packet_owner,
                semantics=semantic_owner, outer_timeout_ms=outer_timeout_ms)
            self.current_deadline = self.owner.current_deadline
            semantic_owner.deadline = lambda: self.owner.current_deadline
            self.controller = bridge.RunnerSessionBridge(prepared, started, records=records,
                reader=adapter.DeadlinePipe(self.pipes.reader, lambda: self.current_deadline),
                writer=adapter.DeadlinePipe(self.pipes.writer, lambda: self.current_deadline))
        except BaseException as error:
            self.close_inactive_descriptors()
            raise SessionRuntimeIncomplete(self) from error

    def _pump(self):
        try:
            self.lifecycle = self.owner.run()
        except BaseException as error:
            self.adapter_error = error
        finally:
            for stream in (self.adapter_reader, self.adapter_writer):
                if stream is not None and not stream.closed:
                    stream.close()

    def close_inactive_descriptors(self):
        """Release local ends only; retain resources still used by a live pump."""
        if self.pipes is not None:
            self.pipes.close()
        if self.thread is not None and self.thread.is_alive():
            return
        for stream in (self.adapter_reader, self.adapter_writer):
            if stream is not None and not stream.closed:
                stream.close()
        if self.adapter_records is not None:
            self.adapter_records.close()

    def observation(self):
        """Return current facts without treating an exit code as a cleanup proof."""
        process = self.worker.process
        return {'worker_pid': None if process is None else process.pid,
                'worker_poll_exit_code': None if process is None else process.poll(),
                'adapter_thread_alive': self.thread is not None and self.thread.is_alive(),
                'adapter_error_type': None if self.adapter_error is None else type(self.adapter_error).__name__,
                'lifecycle': self.lifecycle,
                'exchange_complete': self.exchange is not None and self.exchange.all_attempts_accepted}

    def run(self, *, publish_start, validate_ready, validate_completion,
            validate_acceptance, validate_terminal):
        """Run exact mandatory validators and retain an incomplete owner on error."""
        control.require(not self.used, 'retained session runtime cannot be reused')
        self.used = True
        self.thread = threading.Thread(target=self._pump,
            name='benchmark-session-'+self.prepared['identity']['session_id'][:12], daemon=False)
        try:
            self.thread.start()

            def durable_start(index, preceding):
                reference = publish_start(index, preceding)
                # Read the durable record before setting this attempt's sealed
                # deadline; computation cannot extend time already consumed.
                value = control.decode(self.records.read(reference))
                self.current_deadline = adapter.start_deadline(value, self.owner.outer_timeout_ms)
                return reference

            def joined_terminal(bound, accepted, complete):
                # The complete terminal frame has been consumed. The adapter
                # has no further read dependency on this callback; join it
                # before the collector publishes any quiescent session cut.
                self.thread.join(timeout=adapter.remaining_seconds(min(self.current_deadline, self.owner.current_deadline)))
                control.require(not self.thread.is_alive() and self.adapter_error is None
                                and self.lifecycle is not None,
                                'adapter did not finish its authenticated worker lifecycle')
                return validate_terminal(bound, accepted, complete)

            self.exchange = self.controller.run(publish_start=durable_start,
                validate_ready=validate_ready, validate_completion=validate_completion,
                validate_acceptance=validate_acceptance, validate_terminal=joined_terminal)
            # The terminal callback has already authenticated real process
            # absence. Only now can the campaign consume this exchange.
            return self.exchange
        except BaseException as error:
            # Closing our pipe endpoints permits EOF, never process signalling.
            # A still-active pump and every native verifier remain reachable.
            if self.pipes is not None:
                self.pipes.close()
            raise SessionRuntimeIncomplete(self) from error
        finally:
            self.close_inactive_descriptors()


class RunnerSemanticCallbacks:
    """Publish the canonical sample and validation between completion and ACK.

The same mandatory semantic owner independently replays its native economic
verification and process/packet windows before acceptance. Failed attempts
publish typed validation without a sample; no process exit is inferred here.
"""

    def __init__(self, prepared, started, *, records, semantic_owner, first_ordinal,
                 outer_timeout_ms, validate_lifecycle):
        control.require(callable(validate_lifecycle), 'session lifecycle validator is mandatory')
        self.prepared, self.started, self.records = prepared, started, records
        self.semantics, self.first_ordinal = semantic_owner, first_ordinal
        self.outer_timeout_ms, self.validate_lifecycle = outer_timeout_ms, validate_lifecycle

    def publish_start(self, index, preceding):
        """Use the planner-owned full-plan ordinal and exact predecessor ACK."""
        return runner.publish_benchmark_attempt_start(self.prepared, index, records=self.records,
            session_started=self.started, ordinal=self.first_ordinal+index,
            outer_timeout_ms=self.outer_timeout_ms, preceding_acceptance=preceding)

    def validate_ready(self, bound):
        """Join the adapter's retained native readiness to its semantic check."""
        control.exact(bound, {'ready', 'process_observation'}, 'runner readiness inputs')
        ready = control.decode(bound['ready'])
        self.semantics.validate_ready(ready, self.prepared['request'])
        prefix = 'sessions/'+self.prepared['identity']['session_id']
        observed = control.decode(bound['process_observation'])
        control.require(bound['process_observation'] == self.records.read(self.records.locate(prefix+'/process-ready.json'))
                        and observed['ready'] == self.records.locate(prefix+'/ready.json')
                        and control.canonical(observed['process_scope']) == control.canonical(self.semantics.observations.scope.initial),
                        'runner readiness differs from the actual observed worker scope')

    def validate_completion(self, index, bound):
        """Retain every genuine terminal and publish metrics only after replay."""
        row = self.prepared['request']['attempts'][index]
        request = control.decode(self.records.read(row['request']))
        terminal = adapter.terminal_envelope(bound['rust_terminal'], request, row['request']['sha256'])
        status = terminal['outcome']['kind']
        fields = {**self.prepared['identity'], **{key: row[key] for key in control.ATTEMPT_FIELDS}}
        sample_ref = response_ref = None
        if status == 'succeeded':
            completed = self.semantics.completed[row['attempt_id']]
            control.require(bound['rust_terminal'] == completed['native_raw']
                            and bound['adapter_outcome'] == self.records.read(completed['adapter_outcome'])
                            and bound['response'] == self.records.read(completed['response']),
                            'runner completion differs from its independently verified native terminal')
            response_ref = completed['response']
            sample_ref = self.records.publish(row['output_directory']+'/benchmark-sample.json',
                                             semantics.measurement_bytes(completed['sample']))
        else:
            control.require(bound.get('response') is None, 'unsuccessful attempt contains a successful response')
            outcome = control.decode(bound['adapter_outcome'])
            control.require(outcome['status'] == status and outcome['response'] is None
                            and all(outcome[key] == value for key, value in fields.items())
                            and outcome['rust_terminal'] == self.records.locate(row['output_directory']+'/evidence/benchmark-protocol/rust-result.json'),
                            'failed runner completion differs from its native terminal')
        validation = {**fields, 'passed': status == 'succeeded',
                      'validation_kind': 'accepted' if status == 'succeeded' else status,
                      'response': response_ref, 'sample': sample_ref}
        reference = self.records.publish(row['output_directory']+'/validation-outcome.json', control.canonical(validation))
        return bridge.CompletionDecision(status, reference, sample_ref)

    def validate_acceptance(self, index, bound):
        """Replay exact semantic inputs after publication, before any ACK byte."""
        row = self.prepared['request']['attempts'][index]
        completed = self.semantics.completed[row['attempt_id']]
        self.semantics.validate_acceptance(bound, row, completed['window'])

    def validate_terminal(self, bound, accepted, complete):
        """Require the collector-owned actual lifecycle and full chain replay."""
        return self.validate_lifecycle(bound, accepted, complete)
