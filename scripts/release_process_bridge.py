"""Synchronous Python supervision of the original shared process owner.

Every post-spawn refusal retains the entire caller stack in this live process
until original terminal observation. Windows requires the whole original Job;
Unix currently observes only the root. An unobservable original parks this call
indefinitely; it does not turn a normal CLI error into parent exit. External
termination, interpreter abort and machine failure are outside this guarantee.
"""
from contextlib import ExitStack
import ctypes as c
import json
import time

from sumeragi_prebuilt_bundle import NativeFailure

_OK = 0
_EOF = 1
_PENDING = 2
_MAX_ACK = 128 * 1024
_FAILED_CUSTODY = []


class _Custody:
    """Reserve all higher-level originals before any native child can exist."""
    def __init__(self, owners, bridge, observation):
        self.originals = owners.pop_all()
        self.bridge = bridge
        self.observation = observation
        self.attempted = False
        self.terminal = False
        self.first = None
        try:
            _FAILED_CUSTODY.append(self)
            owners.push(self.__exit__)
        except BaseException:
            self.originals.close()
            if self in _FAILED_CUSTODY:
                _FAILED_CUSTODY.remove(self)
            raise

    def refuse(self, error):
        if self.first is None:
            self.first = error

    def observed_terminal(self):
        # Completion truth precedes any fallible close/cleanup or error formatting.
        self.terminal = True

    def __exit__(self, kind, error, traceback):
        # This is an actual supervision fallback, not merely a global leak: a
        # one-shot interruption outside run's inner guard must not reach CLI exit.
        try:
            if error is not None:
                self.refuse(error)
        finally:
            self.bridge._retain_until_terminal(self, self.observation)
        self.bridge._close_terminal(self, self.observation)
        first = self.first
        try:
            try:
                suppressed = self.originals.__exit__(
                    type(first) if first is not None else kind,
                    first if first is not None else error,
                    first.__traceback__ if first is not None else traceback)
            except BaseException as cleanup:
                if first is not None and cleanup is not first:
                    # Raise the same first object. Python retains this actual
                    # secondary exception as its context; any explicit original
                    # cause remains unchanged rather than replaced by cleanup.
                    raise first
                raise
        finally:
            if self in _FAILED_CUSTODY:
                _FAILED_CUSTODY.remove(self)
        if first is not None and first is not error and not suppressed:
            raise first
        return suppressed


class _Observation:
    """Allocate every bounded native observation destination before spawn."""
    def __init__(self):
        self.token = c.c_uint64()
        self.buffer = c.create_string_buffer(64 * 1024)
        self.output = c.create_string_buffer(_MAX_ACK)
        self.count = c.c_size_t()
        self.exit_code = c.c_int64()
        self.length = 0
        self.eof = False
        self.closed = False


class Bridge:
    """One independently admitted library and one synchronous original session."""
    def __init__(self, admitted_library, *, owners=None, validate=None, validate_originals=None):
        self.library = admitted_library
        self.owners = owners
        self.validate = validate
        # Optional process-free original check; never a replacement for full admission.
        self.validate_originals = validate_originals
        self.used = False
        self.spawn = self._symbol('iroha_process_spawn_v1', [c.c_void_p, c.c_size_t, c.POINTER(c.c_uint64)])
        self.read = self._symbol('iroha_process_read_v1', [c.c_uint64, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)])
        self.wait = self._symbol('iroha_process_try_wait_v1', [c.c_uint64, c.POINTER(c.c_int64)])
        self.close = self._symbol('iroha_process_close_v1', [c.c_uint64])

    def _symbol(self, name, arguments):
        value = getattr(self.library, name)
        value.argtypes = arguments
        value.restype = c.c_int32
        return value

    def _read(self, observation):
        observation.count.value = 0
        result = self.read(observation.token.value, observation.buffer,
                           len(observation.buffer), c.byref(observation.count))
        count = observation.count.value
        if result == _EOF and count == 0:
            observation.eof = True
            return 0
        if result != _OK or not 0 < count <= len(observation.buffer):
            raise RuntimeError('original native stdout observation failed')
        return count

    def _pause(self, custody):
        try:
            time.sleep(0.01)
        except BaseException as error:
            custody.refuse(error)

    def _supervise_refusal(self, custody, observation):
        """Remain in the initiating process until this same original is complete.

        No status, exception, EOF or retry count is completion. Only successful
        original wait or the ABI's terminal/spawn-refused close authorizes return.
        A truly unknown token or poisoned observer therefore remains parked. Each
        usable pipe is drained to let owned compilers finish without a signal.
        """
        while not custody.terminal:
            if observation.token.value:
                try:
                    if self.wait(observation.token.value, c.byref(observation.exit_code)) == _OK:
                        custody.observed_terminal()
                except BaseException as error:
                    custody.refuse(error)
                if not custody.terminal:
                    try:
                        if self.close(observation.token.value) == _OK:
                            # The ABI admits close only for a genuinely terminal
                            # child/whole Job or explicit pre-child spawn refusal.
                            observation.closed = True
                            custody.observed_terminal()
                    except BaseException as error:
                        custody.refuse(error)
                if not custody.terminal and not observation.eof:
                    try:
                        self._read(observation)
                    except BaseException as error:
                        custody.refuse(error)
            if not custody.terminal:
                self._pause(custody)

    def _close_terminal(self, custody, observation):
        if observation.token.value and not observation.closed:
            try:
                if self.close(observation.token.value) != _OK:
                    custody.refuse(RuntimeError('native session close was refused after terminal observation'))
                else:
                    observation.closed = True
            except BaseException as error:
                custody.refuse(error)

    def _retain_until_terminal(self, custody, observation):
        while True:
            try:
                if not custody.attempted or custody.terminal:
                    return
                self._supervise_refusal(custody, observation)
            except BaseException as error:
                custody.refuse(error)
                self._pause(custody)

    def _session(self, request, length, custody, observation):
        custody.attempted = True
        status = self.spawn(request, length, c.byref(observation.token))
        if status != _OK:
            if not observation.token.value:
                # A returned ABI refusal without a token proves no child creation.
                custody.observed_terminal()
            raise RuntimeError('native original process creation was refused')
        if not observation.token.value:
            raise RuntimeError('native session token was absent after creation')
        while not observation.eof:
            count = self._read(observation)
            if custody.first is None and count:
                if observation.length + count > _MAX_ACK:
                    custody.refuse(RuntimeError('native acknowledgement exceeds its bound'))
                else:
                    c.memmove(c.addressof(observation.output) + observation.length,
                              observation.buffer, count)
                    observation.length += count
        while True:
            status = self.wait(observation.token.value, c.byref(observation.exit_code))
            if status == _OK:
                custody.observed_terminal()
                return
            if status != _PENDING:
                raise RuntimeError('original native whole-Job observation failed')
            self._pause(custody)

    def run(self, settings, owners):
        """Return acknowledgement or the first refusal only after native completion.

        The complete caller ExitStack moves before spawn. There is no timeout,
        process signal, PID lookup, or normal-error escape while completion is
        unknown. Callers must not forcibly terminate this supervising interpreter
        if they require its original process and source custody to remain live.
        """
        if (self.used or not isinstance(owners, ExitStack) or _FAILED_CUSTODY
                or (self.owners is not None and self.owners is not owners)):
            raise RuntimeError('process bridge cannot start another retained operation')
        self.used = True
        if self.validate is not None:
            self.validate()
        raw = json.dumps(settings, separators=(',', ':'), ensure_ascii=False).encode('utf-8')
        if not 0 < len(raw) <= 512 * 1024:
            raise RuntimeError('native entry settings exceed their bound')
        request = c.create_string_buffer(raw)
        observation = _Observation()
        custody = _Custody(owners, self, observation)
        try:
            self._session(request, len(raw), custody, observation)
        except BaseException as error:
            custody.refuse(error)
        # Supervision itself is guarded: interrupted sleep, an unexpected Python
        # allocation error, or a fallible observer cannot escape into CLI shutdown.
        self._retain_until_terminal(custody, observation)
        self._close_terminal(custody, observation)
        if custody.first is not None:
            raise custody.first
        if observation.exit_code.value != 0:
            raise NativeFailure(observation.exit_code.value)
        if self.validate is not None:
            self.validate()
        return observation.output.raw[:observation.length]
