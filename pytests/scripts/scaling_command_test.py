"""Bounded reusable command owner tested with private descriptors and fake children."""
import os
import selectors
from dataclasses import replace

import pytest
from scaling_readiness_fixture import ready_setup, command


def new_owner(c, guard=lambda: None, end=None):
    return command.BoundedCommand(c.images[1], c.commands, c.clock.end(60) if end is None else end, guard)


def invoke(c, owner, stdout_limit=1024, stderr_limit=1024):
    return owner.run('peer0', (str(c.images[1].path), 'fixed-proof-replay'),
                     (c.inputs.client_fd(0),), stdout_limit, stderr_limit)


def test_bounded_command_terminal_receipt_and_original_file_lifetime(ready_setup):
    c = ready_setup
    owner = new_owner(c)
    result = invoke(c, owner)
    assert result.stdout == b'{}\n' and result.process.pid == 2000
    assert owner.deadline_ns == c.clock.end(60) and owner._phase == 'idle'
    assert len(owner._children) == 1 and c.commands.children[0].returncode == 0
    assert os.pread(c.inputs.client_fd(0), 1, 0)
    assert owner.cleanup(c.clock.end(60)) == ()
    with pytest.raises(command.CommandError): invoke(c, owner)


@pytest.mark.parametrize('stream', ['stdout', 'stderr'])
@pytest.mark.parametrize('extra', [0, 1])
def test_stream_caps_accept_exact_bound_and_fail_on_first_extra_byte(ready_setup, stream, extra):
    c = ready_setup
    (c.commands.raws if stream == 'stdout' else c.commands.diagnostics)[0] = b'x' * (32 + extra)
    owner = new_owner(c)
    if extra:
        with pytest.raises(command.CommandError): invoke(c, owner, 32, 32)
        assert owner._phase == 'failed'
    else:
        result = invoke(c, owner, 32, 32)
        assert result.stdout == (b'x' * 32 if stream == 'stdout' else b'{}\n')
    assert len(c.commands.calls) == 1 and len(owner._children) == 1
    assert c.commands.children[0].stdout.closed and c.commands.children[0].stderr.closed


def test_non_eof_pipe_is_bounded_by_original_deadline_and_retains_child(ready_setup, monkeypatch):
    c = ready_setup
    c.commands.raws[0] = None
    owner = new_owner(c, end=c.clock.end(1))
    native_selector = selectors.DefaultSelector
    class AdvancingSelector:
        def __init__(self): self.inner = native_selector()
        def register(self, *args): return self.inner.register(*args)
        def unregister(self, *args): return self.inner.unregister(*args)
        def get_map(self): return self.inner.get_map()
        def select(self, timeout):
            assert 0 < timeout <= 0.05
            c.clock.now += int(timeout * 1e9)
            return self.inner.select(0)
        def close(self): self.inner.close()
    monkeypatch.setattr(command.selectors, 'DefaultSelector', AdvancingSelector)
    with pytest.raises(command.CommandError): invoke(c, owner)
    assert c.clock.now == 2_000_000_000 and len(owner._children) == 1
    assert c.commands.children[0].returncode is None
    assert c.commands.children[0].stdout.closed and c.commands.children[0].stderr.closed
    assert owner.cleanup(c.clock.end()) == ()
    assert ('terminate', 0) in c.commands.events


@pytest.mark.parametrize('failure', ['args_list', 'argv_image', 'argv_count', 'argv_nul', 'argv_bytes',
    'fd_list', 'fd_bool', 'fd_repeat', 'fd_write', 'stdout_bool', 'stdout_zero', 'stdout_huge',
    'stderr_bool', 'role', 'command_count'])
def test_admission_bounds_refuse_before_any_spawn(ready_setup, monkeypatch, failure):
    c = ready_setup
    owner = new_owner(c)
    role, argv, fds, out, err = 'proof', (str(c.images[1].path), 'fixed'), (c.inputs.client_fd(0),), 1024, 1024
    added_fd = None
    if failure == 'args_list': argv = list(argv)
    if failure == 'argv_image': argv = (str(c.images[0].path), 'fixed')
    if failure == 'argv_count': argv = (argv[0],) + ('fixed',) * command.MAX_ARGV_ITEMS
    if failure == 'argv_nul': argv = (argv[0], 'fixed\x00')
    if failure == 'argv_bytes': argv = (argv[0], 'x' * 8193)
    if failure == 'fd_list': fds = list(fds)
    if failure == 'fd_bool': fds = (True,)
    if failure == 'fd_repeat': fds = fds * 2
    if failure == 'fd_write': added_fd = os.open(c.inputs.roles[0].client_config, os.O_RDWR); fds = (added_fd,)
    if failure == 'stdout_bool': out = True
    if failure == 'stdout_zero': out = 0
    if failure == 'stdout_huge': out = command.MAX_OUTPUT_BYTES + 1
    if failure == 'stderr_bool': err = True
    if failure == 'role': role = 'PRIVATE INVALID ROLE'
    if failure == 'command_count': monkeypatch.setattr(command, 'MAX_COMMANDS', 0)
    try:
        with pytest.raises(command.CommandError): owner.run(role, argv, fds, out, err)
    finally:
        if added_fd is not None: os.close(added_fd)
    assert c.commands.calls == [] and owner._phase == 'failed'


@pytest.mark.parametrize('error', [ValueError('PRIVATE'), command.CommandError('PRIVATE'),
    KeyboardInterrupt('PRIVATE'), SystemExit('PRIVATE'), GeneratorExit('PRIVATE'), BaseException('PRIVATE')])
def test_guard_error_and_unwind_cannot_leak_private_text_or_revive_owner(ready_setup, error):
    c = ready_setup
    def fail(): raise error
    owner = new_owner(c, fail)
    expected = type(error) if isinstance(error, (KeyboardInterrupt, SystemExit, GeneratorExit)) else command.CommandError
    with pytest.raises(expected) as caught: invoke(c, owner)
    assert 'PRIVATE' not in str(caught.value)
    if isinstance(error, SystemExit): assert caught.value.code == 1
    assert c.commands.calls == [] and owner._phase == 'failed'


@pytest.mark.parametrize('failure', ['reentrant', 'cleanup', 'closed_image', 'image_retarget', 'descriptor_retarget', 'deadline'])
def test_last_guard_cannot_poison_or_retarget_then_allow_spawn(ready_setup, failure):
    c = ready_setup
    changed = []
    def guard():
        if changed: return
        changed.append(True)
        if failure == 'reentrant':
            with pytest.raises(command.CommandError): invoke(c, owner)
        if failure == 'cleanup': owner.cleanup(c.clock.end())
        if failure == 'closed_image': c.images[1].close()
        if failure == 'image_retarget': c.images[1].path = c.images[0].path
        if failure == 'descriptor_retarget':
            fd = c.inputs.client_fd(0)
            other = c.inputs.client_fd(1)
            os.dup2(other, fd)
        if failure == 'deadline': c.clock.now = c.clock.end(61)
    original_fd = c.inputs.client_fd(0)
    saved = os.dup(original_fd) if failure == 'descriptor_retarget' else None
    try:
        owner = new_owner(c, guard)
        with pytest.raises(command.CommandError): invoke(c, owner)
        assert c.commands.calls == [] and owner._phase == 'failed'
    finally:
        # The negative owns its deliberate retarget; restore it before the
        # stricter original ReadinessInputs owner tears down its descriptors.
        if saved is not None:
            os.dup2(saved, original_fd, inheritable=False)
            os.close(saved)


@pytest.mark.parametrize('field', ['identity', 'pid', 'reader', 'image', 'peer_id'])
def test_sampling_cannot_retarget_original_child_before_another_io_read(ready_setup, field):
    c = ready_setup
    owner = new_owner(c)
    original = c.commands.sample
    def sample(pid, image):
        value = original(pid, image)
        child = owner._children[0]
        if child.pinned is not None:
            replacement = {'identity': replace(value.identity, start_seconds=99), 'pid': 9999,
                'reader': object(), 'image': c.images[0], 'peer_id': 'peer1'}[field]
            setattr(child.pinned, field, replacement)
        return value
    c.commands.sample = sample
    with pytest.raises(command.CommandError): invoke(c, owner)
    assert owner._phase == 'failed' and len(owner._children) == 1


@pytest.mark.parametrize('stage', ['initial_pin', 'poll', 'wait'])
def test_cancellation_after_spawn_keeps_original_handle_and_closes_both_pipes(ready_setup, stage):
    c = ready_setup
    owner = new_owner(c)
    def cancel(*_): raise KeyboardInterrupt('PRIVATE')
    if stage == 'initial_pin': c.commands.sample = cancel
    if stage == 'poll': c.commands.after_spawn = lambda child: setattr(child, 'poll', cancel)
    if stage == 'wait': c.commands.after_spawn = lambda child: setattr(child, 'wait_hook', cancel)
    with pytest.raises(KeyboardInterrupt) as caught: invoke(c, owner)
    assert str(caught.value) == '' and len(owner._children) == 1
    assert c.commands.children[0].stdout.closed and c.commands.children[0].stderr.closed


def test_cleanup_bound_can_expire_without_forced_signal_or_false_success(ready_setup):
    c = ready_setup
    owner = new_owner(c)
    c.commands.after_spawn = lambda child: setattr(child, 'stall', True)
    with pytest.raises(command.CommandError): invoke(c, owner)
    assert owner.cleanup(c.clock.end()) == ('peer0',)
    assert [event for event in c.commands.events if event[0] == 'terminate'] == [('terminate', 0)]
    assert len(owner._children) == 1 and owner._phase == 'failed'
    c.commands.children[0].stall = False
    assert owner.cleanup(c.clock.end()) == ()
    with pytest.raises(command.CommandError): invoke(c, owner)


def test_deadline_expiring_during_selector_wait_refuses_even_first_late_read(ready_setup, monkeypatch):
    c = ready_setup
    owner = new_owner(c)
    native_selector = selectors.DefaultSelector
    class LateSelector:
        def __init__(self): self.inner = native_selector()
        def register(self, *args): return self.inner.register(*args)
        def unregister(self, *args): return self.inner.unregister(*args)
        def get_map(self): return self.inner.get_map()
        def select(self, timeout):
            events = self.inner.select(0)
            c.clock.now = owner.deadline_ns
            return events
        def close(self): self.inner.close()
    reads = []
    original_read = os.read
    def read(*args): reads.append(args); return original_read(*args)
    monkeypatch.setattr(command.selectors, 'DefaultSelector', LateSelector)
    monkeypatch.setattr(os, 'read', read)
    with pytest.raises(command.CommandError): invoke(c, owner)
    assert reads == [] and len(owner._children) == 1
