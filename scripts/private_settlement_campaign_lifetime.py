"""Keep canonical campaign children alive through failure and owner draining.

The caller must execute on the main Python thread and retain the actual
CampaignExecution owner. This module does not grant evidence authority, extend
registered deadlines, signal native workers, or turn a late exit into success.
Only an existing packet owner may perform its documented tcpdump shutdown.
No environment variable or alternative execution mode changes this boundary.
"""
from contextlib import AbstractContextManager
import os
import signal
import subprocess
import threading
import time


class CampaignDrainRequested(RuntimeError):
    """The current owner may finish; no subsequent job may start."""


def wait_owned_process(process, *, group):
    """Reap an actual child, then retain its process group until it is absent.

    Physical cleanup deliberately outlives an expired qualification deadline.
    Callers retain the original failed/timed-out/incomplete outcome. This is
    not a replacement protocol terminal or source/image verification.
    """
    if process is None:
        return None
    while True:
        try:
            code = process.wait(timeout=0.2)
            break
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            continue
    if group:
        while True:
            try:
                os.killpg(process.pid, 0)
            except ProcessLookupError:
                break
            except PermissionError:
                # Unknown is not proof of absence. Keep the outer session alive.
                pass
            time.sleep(0.05)
    return {'pid': process.pid, 'exit_code': code,
            'physical_wait_completed': True, 'group_absence_observed': group}


def join_owned_thread(thread):
    """Wait for the real adapter/reader before inspecting its final child set."""
    if thread is None or thread.ident is None:
        return
    while thread.is_alive():
        try:
            thread.join(timeout=0.2)
        except KeyboardInterrupt:
            continue


def wait_native_owner(native):
    """Never re-probe a PID whose prior physical closure is already observed."""
    if native.physical_exit is not None:
        return {'prior_physical_exit_retained': True}
    observation = wait_owned_process(native.process, group=True)
    if observation is not None:
        native.physical_exit = observation
    return observation


def drain_session(owner):
    """Join one retained session's explicit physical owners after execution ends.

    Join the adapter first: it may still install a native verifier or capture
    owner. Its failure path closes worker control pipes, allowing native EOF
    cleanup. No State or protocol observations are invented by this final wait.
    """
    observations = {'worker': None, 'verifiers': [], 'captures': [], 'cleanup_errors': []}
    if owner.closed:
        try:
            owner.require_physical_owners_closed()
        except ValueError as error:
            # A contradictory flag cannot hide a live physical owner. Retain
            # the invariant error and drain the actual objects below.
            observations['cleanup_errors'].append(type(error).__name__)
        else:
            return {'canonical_session_closed': True, 'additional_pid_probes': 0}

    runtime = owner.runtime
    if runtime is not None:
        join_owned_thread(runtime.thread)
    # The adapter is now stable; no new capture/verifier can be installed.
    for resources in owner.resource_owners.values():
        # Only the private sampler stop event changes; no native process or
        # financial protocol is signalled and no metric is fabricated.
        resources._stop.set()
        join_owned_thread(resources._thread)
    for packet in owner.packet_owners.values():
        if not packet.closed:
            try:
                packet.abort('campaign_owner_draining')
            except BaseException as error:
                observations['cleanup_errors'].append(type(error).__name__)
        record = wait_owned_process(packet.child, group=False)
        join_owned_thread(packet.thread)
        observations['captures'].append(record)
    if owner.semantic_owner is not None:
        for invocation in owner.semantic_owner.native_invocations.values():
            observations['verifiers'].append(wait_native_owner(invocation))
    if owner.worker is not None:
        observations['worker'] = wait_native_owner(owner.worker)
    return observations


class CampaignLifetime(AbstractContextManager):
    """Keep the enclosing execution session alive until every retained child exits."""

    def __init__(self, owner):
        self.owner = owner
        self.drain_reason = None
        self.previous = {}
        self.observations = {}
        self.cleanup_errors = {}

    def handle_signal(self, signum, _frame):
        """Request drain without doing I/O or forwarding signals to children."""
        if self.drain_reason is None:
            self.drain_reason = {'kind': 'owner_signal', 'signal': signum}

    def require_launch(self):
        """Stop only at a registered job/session boundary after current work."""
        if self.drain_reason is not None:
            raise CampaignDrainRequested('campaign owner requested drain')

    def __enter__(self):
        if threading.current_thread() is not threading.main_thread():
            raise RuntimeError('canonical campaign lifetime requires the main Python thread')
        try:
            for signum in (signal.SIGINT, signal.SIGTERM):
                self.previous[signum] = signal.signal(signum, self.handle_signal)
        except BaseException:
            for signum, handler in self.previous.items():
                signal.signal(signum, handler)
            raise
        return self

    def __exit__(self, kind, error, traceback):
        try:
            # This callback runs even if writing the campaign failure receipt
            # failed. Strong owners remain held through the final physical wait.
            pending = dict(self.owner.session_owners)
            while pending:
                for session_id, session in list(pending.items()):
                    try:
                        self.observations[session_id] = drain_session(session)
                    except BaseException as cleanup_error:
                        # Preserve one bounded diagnostic per registered owner,
                        # then continue draining the other children. Never
                        # restore terminating handlers while one remains open.
                        self.cleanup_errors.setdefault(session_id, type(cleanup_error).__name__)
                    else:
                        del pending[session_id]
                if pending:
                    time.sleep(0.05)
        finally:
            for signum, handler in self.previous.items():
                signal.signal(signum, handler)
        return False
