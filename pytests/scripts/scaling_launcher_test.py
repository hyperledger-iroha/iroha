"""Mocked direct-child lifetimes with real file and executable custody owners.

No test starts, observes or signals an OS child. Mach-O bytes are synthetic;
PinnedProcess is real and its kernel reader is replaced with an explicit fake.
"""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import struct
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
import scaling_launcher as launcher
import resource_process as process


class Clock:
    def __init__(self):
        self.now = 1_000_000_000

    def __call__(self):
        return self.now

    def end(self, seconds=60):
        return self.now + seconds * 1_000_000_000


class Child:
    def __init__(self, factory, index):
        self.factory, self.index = factory, index
        self.pid = 1000 + index
        self.returncode = None
        self.status = None
        self.drift = False
        self.nonzero = False
        self.stall = False
        self.wait_hook = lambda: None

    def poll(self):
        if self.status is not None:
            self.returncode = self.status
        return self.returncode

    def terminate(self):
        self.factory.events.append(('terminate', self.index))
        if not self.stall:
            self.status = 9 if self.nonzero else 0

    def wait(self, timeout):
        self.factory.events.append(('wait', self.index, timeout))
        self.wait_hook()
        if self.stall or self.status is None:
            raise launcher.subprocess.TimeoutExpired('redacted-test-command', timeout)
        self.returncode = self.status
        return self.returncode


class Factory:
    def __init__(self):
        self.children, self.calls, self.events = [], [], []
        self.fail_at = None
        self.after_spawn = lambda child: None

    def __call__(self, argv, **kwargs):
        index = len(self.calls)
        self.calls.append((argv, kwargs))
        if index == self.fail_at:
            raise OSError('PRIVATE CONFIG SECRET')
        child = Child(self, index)
        self.children.append(child)
        self.after_spawn(child)
        return child

    def sample(self, pid, image):
        child = next(child for child in self.children if child.pid == pid)
        if child.status is not None:
            raise process.ProcessObservationError('fake child exited')
        image.validate()
        identity = process.ProcessIdentity(pid, os.geteuid(), 50 + int(child.drift),
                                           1, 500, next(iter(image.uuids)).hex(), image.sha256)
        return process.ProcessSample(identity, 4096)


@pytest.fixture
def setup(tmp_path, monkeypatch):
    clock, factory = Clock(), Factory()
    monkeypatch.setattr(launcher.time, 'monotonic_ns', clock)
    monkeypatch.setattr(launcher.subprocess, 'Popen', factory)
    monkeypatch.setattr(os, 'kill', lambda *_: pytest.fail('no numeric PID signals'))
    monkeypatch.setattr(os, 'killpg', lambda *_: pytest.fail('no process group signals'))
    # Resolve only the test-owned temporary fixture root; production admission
    # receives an already absolute lexical path and never resolves symlinks.
    root = tmp_path.resolve()
    image_path = root / 'iroha3d'
    raw = (struct.pack('<8I', 0xFEEDFACF, 0x100000C, 0, 2, 1, 24, 0, 0)
           + struct.pack('<II', 0x1B, 24) + bytes(range(1, 17)))
    image_path.write_bytes(raw)
    image_path.chmod(0o700)
    peers = tuple(launcher.PeerLaunch(f'peer{i}', root / f'config{i}.toml',
                                      str(i) * 64, root / f'kura{i}') for i in range(4))
    for peer in peers:
        peer.config.write_text(f'private_key = "test-secret-{peer.peer_id}"\n'
                               f'[kura]\nstore_dir = {json.dumps(str(peer.block_store))}\n')
        peer.config.chmod(0o600)
        peer.block_store.mkdir(mode=0o700)
    peers = tuple(replace(peer, config_blake3=launcher.blake3.blake3(
        peer.config.read_bytes()).hexdigest()) for peer in peers)
    original = tuple(peer.config.read_bytes() for peer in peers)
    verifications = []

    def verify():
        verifications.append(clock.now)
        if tuple(peer.config.read_bytes() for peer in peers) != original:
            raise ValueError('PRIVATE CONFIG SECRET')

    with process.ExecutableImage(image_path, hashlib.sha256(raw).hexdigest()) as image:
        with launcher.PeerLaunchInputs(peers) as inputs:
            owner = launcher.FourPeerRun(peers, image, factory, inputs, verify, clock.end(600))
            # This pre-existing lifecycle suite isolates the new native readiness
            # boundary with typed receipts. scaling_readiness_test exercises the
            # actual fixed command/FD/pipe owner and mandatory phase transition.
            original_launch = owner.launch_owned
            def launch_then_readiness():
                pins = original_launch()
                step = object.__new__(launcher.FourPeerReadiness)
                step._end = owner._trial_deadline_ns
                step._inputs = SimpleNamespace(role_bindings=tuple(
                    (peer.peer_id, peer.config, peer.block_store) for peer in peers))
                def collect(original, verify):
                    verify()
                    return tuple(launcher.ReadyReceipt(pin.peer_id, pin.identity,
                        'test-node', 'test-network', 'test-genesis', 'test-context',
                        '1' * 64, image.sha256, pin.identity, '2' * 64, '3' * 64,
                        '4' * 64, b'test-native-receipt') for pin in original)
                step.collect = collect
                step.cleanup = lambda deadline: ()
                owner.await_genesis_ready(step)
                return pins
            owner.launch_owned = launch_then_readiness
            yield owner, factory, clock, peers, image, verifications


def ready(owner):
    peers = owner.launch_owned()
    assert len(peers) == 4 and all(isinstance(peer, process.PinnedProcess) for peer in peers)
    assert owner.run_load(lambda same: same) == peers


def collected(owner, clock):
    ready(owner)
    stopped = owner.stop_peer3(clock.end())
    assert owner.collect_inputs(lambda store, peer0: (store, peer0.peer_id)) == (stopped, 'peer0')


def test_exact_direct_argv_environment_four_owners_and_ordered_clean_reap(setup):
    owner, factory, clock, peers, image, checks = setup
    collected(owner, clock)
    owner.stop_survivors(clock.end())
    owner.verify_stopped()
    for index, (argv, kwargs) in enumerate(factory.calls):
        assert argv == [str(image.path), '--config', str(peers[index].config),
                        '--config-blake3', peers[index].config_blake3]
        assert kwargs == dict(stdin=launcher.subprocess.DEVNULL,
                              stdout=launcher.subprocess.DEVNULL,
                              stderr=launcher.subprocess.DEVNULL, cwd='/', env={},
                              close_fds=True, shell=False, start_new_session=False)
    assert [event[1] for event in factory.events if event[0] == 'terminate'] == [3, 2, 1, 0]
    assert len([event for event in factory.events if event[0] == 'wait']) == 4
    assert checks and all(child.returncode == 0 for child in factory.children)
    assert all(peer.config.exists() and peer.block_store.is_dir() for peer in peers)


@pytest.mark.parametrize('at', range(4))
def test_partial_spawn_failure_keeps_only_created_handles_and_redacts_error(setup, at):
    owner, factory, clock, *_ = setup
    factory.fail_at = at
    with pytest.raises(launcher.LauncherError, match='^launch_stage_failed$'):
        owner.launch_owned()
    assert len(factory.children) == at
    assert owner.cleanup(clock.end()) == ()
    assert [event[1] for event in factory.events if event[0] == 'terminate'] == list(reversed(range(at)))
    with pytest.raises(launcher.LauncherError):
        owner.launch_owned()
    assert len(factory.calls) == at + 1


@pytest.mark.parametrize('at', range(4))
def test_initial_identity_failure_retains_failed_child_for_owned_cleanup(setup, at):
    owner, factory, clock, *_ = setup
    original = factory.sample
    def sample(pid, image):
        if pid == 1000 + at:
            raise RuntimeError('PRIVATE READER DETAIL')
        return original(pid, image)
    factory.sample = sample
    with pytest.raises(launcher.LauncherError, match='^launch_stage_failed$'):
        owner.launch_owned()
    assert len(factory.children) == at + 1
    assert owner.cleanup(clock.end()) == ()
    assert all(child.returncode == 0 for child in factory.children)


@pytest.mark.parametrize('at', range(4))
@pytest.mark.parametrize('failure', ['exit', 'lifetime', 'owner_retarget'])
def test_each_original_child_is_required_at_load_and_restoration_cannot_revive_run(setup, at, failure):
    owner, factory, _, *_ = setup
    peers = owner.launch_owned()
    if failure == 'exit':
        factory.children[at].status = 0
    elif failure == 'lifetime':
        factory.children[at].drift = True
    else:
        peers[at].reader = object()
    called = []
    with pytest.raises(launcher.LauncherError):
        owner.run_load(lambda _: called.append(True))
    factory.children[at].status = factory.children[at].returncode = None
    factory.children[at].drift = False
    peers[at].reader = factory
    with pytest.raises(launcher.LauncherError):
        owner.run_load(lambda _: called.append(True))
    assert called == [] and factory.events == []


@pytest.mark.parametrize('method', ['stop_peer3', 'collect_inputs', 'stop_survivors', 'verify_stopped'])
def test_early_transition_fails_permanently_without_signals(setup, method):
    owner, factory, clock, *_ = setup
    call = getattr(owner, method)
    arg = () if method == 'verify_stopped' else ((lambda *_: pytest.fail('early collection')),) if method == 'collect_inputs' else (clock.end(),)
    with pytest.raises(launcher.LauncherError):
        call(*arg)
    with pytest.raises(launcher.LauncherError):
        owner.launch_owned()
    assert factory.calls == [] and factory.events == []


@pytest.mark.parametrize('stage', ['load', 'collection'])
@pytest.mark.parametrize('error', [RuntimeError('PRIVATE'), KeyboardInterrupt()])
def test_callback_error_or_unwind_prevents_all_later_success(setup, stage, error):
    owner, factory, clock, *_ = setup
    owner.launch_owned()
    if stage == 'collection':
        owner.run_load(lambda _: None)
        owner.stop_peer3(clock.end())
    def fail(*_):
        raise error
    with pytest.raises(KeyboardInterrupt if isinstance(error, KeyboardInterrupt) else launcher.LauncherError):
        (owner.run_load if stage == 'load' else owner.collect_inputs)(fail)
    with pytest.raises(launcher.LauncherError):
        owner.stop_survivors(clock.end())
    assert owner.cleanup(clock.end()) == ()
    with pytest.raises(launcher.LauncherError):
        owner.verify_stopped()


@pytest.mark.parametrize('at', range(3))
@pytest.mark.parametrize('when', ['before', 'during'])
def test_every_original_survivor_required_through_collection(setup, at, when):
    owner, factory, clock, *_ = setup
    ready(owner)
    owner.stop_peer3(clock.end())
    called = []
    if when == 'before':
        factory.children[at].status = 0
    def collect(*_):
        called.append(True)
        factory.children[at].status = 0
    with pytest.raises(launcher.LauncherError):
        owner.collect_inputs(collect)
    assert called == ([] if when == 'before' else [True])
    assert [event[1] for event in factory.events if event[0] == 'terminate'] == [3]


@pytest.mark.parametrize('failure', ['nonzero', 'timeout', 'deadline', 'survivor_exit'])
def test_peer3_shutdown_must_be_clean_reaped_and_preserve_proof_survivors(setup, failure):
    owner, factory, clock, *_ = setup
    ready(owner)
    if failure == 'nonzero': factory.children[3].nonzero = True
    if failure == 'timeout': factory.children[3].stall = True
    if failure == 'deadline': factory.children[3].wait_hook = lambda: setattr(clock, 'now', clock.end(61))
    if failure == 'survivor_exit': factory.children[3].wait_hook = lambda: setattr(factory.children[0], 'status', 0)
    with pytest.raises(launcher.LauncherError):
        owner.stop_peer3(clock.end())
    with pytest.raises(launcher.LauncherError):
        owner.collect_inputs(lambda *_: pytest.fail('failed stop reached proof'))
    assert [event[1] for event in factory.events if event[0] == 'terminate'] == [3]


def test_survivor_shutdown_uses_one_absolute_deadline_without_per_peer_reset(setup):
    owner, factory, clock, *_ = setup
    collected(owner, clock)
    for child in factory.children[:3]:
        child.wait_hook = lambda: setattr(clock, 'now', clock.now + 2_000_000_000)
    owner.stop_survivors(clock.end(7))
    waits = [event[2] for event in factory.events if event[0] == 'wait']
    assert waits[-3:] == [7.0, 5.0, 3.0]


def test_cleanup_timeout_leaves_original_handles_and_never_becomes_success(setup):
    owner, factory, clock, peers, *_ = setup
    ready(owner)
    factory.children[2].stall = True
    assert owner.cleanup(clock.end()) == ('peer2',)
    assert all(peer.config.exists() for peer in peers)
    factory.children[2].stall = False
    assert owner.cleanup(clock.end()) == ()
    with pytest.raises(launcher.LauncherError):
        owner.verify_stopped()


@pytest.mark.parametrize('at', ['load', 'stop', 'collection', 'final'])
def test_original_config_change_fails_every_barrier_and_remains_failed(setup, at):
    owner, _, clock, peers, *_ = setup
    if at == 'final':
        collected(owner, clock)
        owner.stop_survivors(clock.end())
    else:
        owner.launch_owned()
        if at in {'stop', 'collection'}: owner.run_load(lambda _: None)
        if at == 'collection': owner.stop_peer3(clock.end())
    raw = peers[0].config.read_bytes()
    peers[0].config.write_bytes(b'PRIVATE CHANGED CONFIG')
    calls = {'load': lambda: owner.run_load(lambda _: None),
             'stop': lambda: owner.stop_peer3(clock.end()),
             'collection': lambda: owner.collect_inputs(lambda *_: None),
             'final': owner.verify_stopped}
    with pytest.raises(launcher.LauncherError, match='^launch_stage_failed$'):
        calls[at]()
    peers[0].config.write_bytes(raw)
    with pytest.raises(launcher.LauncherError):
        calls[at]()


@pytest.mark.parametrize('change', ['count', 'list', 'label', 'config', 'digest', 'store', 'nested', 'relative', 'inside_store', 'deadline', 'boolean'])
def test_independent_admission_rejects_invalid_roles_and_bounds_without_spawn(setup, change):
    owner, factory, clock, peers, image, _ = setup
    args = peers
    end = clock.end(600)
    if change == 'count': args = peers[:3]
    if change == 'list': args = list(peers)
    if change == 'label': args = (replace(peers[0], peer_id=peers[1].peer_id),) + peers[1:]
    if change == 'config': args = (replace(peers[0], config=peers[1].config),) + peers[1:]
    if change == 'digest': args = (replace(peers[0], config_blake3='A' * 64),) + peers[1:]
    if change == 'store': args = (replace(peers[0], block_store=peers[1].block_store),) + peers[1:]
    if change == 'nested': args = (replace(peers[0], block_store=peers[1].block_store / 'nested'),) + peers[1:]
    if change == 'relative': args = (replace(peers[0], config=Path('relative')), ) + peers[1:]
    if change == 'inside_store': args = (replace(peers[0], config=peers[1].block_store / 'config'),) + peers[1:]
    if change == 'deadline': end = clock.end(7201)
    if change == 'boolean': end = True
    with pytest.raises(launcher.LauncherError):
        launcher.FourPeerRun(args, image, factory, owner._inputs, lambda: None, end)
    assert factory.calls == []


@pytest.mark.parametrize('inner', ['stop', 'load', 'cleanup'])
def test_caught_nested_transition_or_cleanup_never_revives_outer_scope(setup, inner):
    owner, factory, clock, *_ = setup
    owner.launch_owned()
    def operation(_):
        try:
            if inner == 'stop': owner.stop_survivors(clock.end())
            if inner == 'load': owner.run_load(lambda _: pytest.fail('reentrant load'))
            if inner == 'cleanup': owner.cleanup(clock.end())
        except launcher.LauncherError:
            pass
    with pytest.raises(launcher.LauncherError):
        owner.run_load(operation)
    with pytest.raises(launcher.LauncherError):
        owner.stop_peer3(clock.end())
    if inner != 'cleanup': assert factory.events == []


def test_callback_cannot_smuggle_private_error_through_public_exception_type(setup):
    owner, _, _, *_ = setup
    owner.launch_owned()
    def operation(_):
        raise launcher.LauncherError('PRIVATE CONFIG SECRET')
    with pytest.raises(launcher.LauncherError, match='^launch_stage_failed$'):
        owner.run_load(operation)


@pytest.mark.parametrize('at', ['spawn', 'load', 'collection', 'final'])
def test_absolute_trial_deadline_applies_at_each_stage(setup, at):
    owner, factory, clock, *_ = setup
    if at == 'load': owner.launch_owned()
    if at == 'collection':
        ready(owner)
        owner.stop_peer3(clock.end())
    if at == 'final':
        collected(owner, clock)
        owner.stop_survivors(clock.end())
    clock.now = clock.end(601)
    calls = {'spawn': owner.launch_owned,
             'load': lambda: owner.run_load(lambda _: pytest.fail('late load')),
             'collection': lambda: owner.collect_inputs(lambda *_: pytest.fail('late collection')),
             'final': owner.verify_stopped}
    with pytest.raises(launcher.LauncherError): calls[at]()
    if at == 'spawn': assert factory.calls == []


def test_executable_replacement_after_admission_fails_before_first_spawn(setup):
    owner, factory, _, _, image, _ = setup
    raw = image.path.read_bytes()
    image.path.rename(image.path.with_suffix('.old'))
    image.path.write_bytes(raw)
    image.path.chmod(0o700)
    with pytest.raises(launcher.LauncherError):
        owner.launch_owned()
    assert factory.calls == []


@pytest.mark.parametrize('before_peer', range(4))
@pytest.mark.parametrize('failure', ['caught_poison', 'deadline', 'image'])
def test_last_input_check_cannot_allow_any_later_spawn_after_failure(setup, before_peer, failure):
    owner, factory, clock, _, image, _ = setup
    original = owner._verify_inputs
    calls = 0
    def verify():
        nonlocal calls
        original()
        calls += 1
        # Last custody callback immediately before this role's Popen call.
        # Every earlier role has completed both pre/post-spawn identity checks.
        if calls == 3 + 4 * before_peer:
            if failure == 'caught_poison':
                try:
                    owner.stop_survivors(clock.end())
                except launcher.LauncherError:
                    pass
            elif failure == 'deadline':
                clock.now = clock.end(601)
            else:
                with image.path.open('ab') as output:
                    output.write(b'changed-main-image')
    owner._verify_inputs = verify
    with pytest.raises(launcher.LauncherError):
        owner.launch_owned()
    assert len(factory.calls) == len(factory.children) == before_peer
    assert factory.events == []


@pytest.mark.parametrize('stage', ['initial', 'load', 'stop'])
def test_sample_caught_poison_stops_before_next_observation_or_signal(setup, stage):
    owner, factory, clock, *_ = setup
    if stage != 'initial': owner.launch_owned()
    if stage == 'stop': owner.run_load(lambda _: None)
    original = factory.sample
    observed_pids = []
    def sample(pid, image):
        result = original(pid, image)
        observed_pids.append(pid)
        try:
            owner.stop_survivors(clock.end())
        except launcher.LauncherError:
            pass
        return result
    factory.sample = sample
    calls = {'initial': owner.launch_owned,
             'load': lambda: owner.run_load(lambda _: pytest.fail('poisoned sample entered load')),
             'stop': lambda: owner.stop_peer3(clock.end())}
    with pytest.raises(launcher.LauncherError): calls[stage]()
    assert observed_pids == [1000]
    assert factory.events == []
    assert len(factory.children) == (1 if stage == 'initial' else 4)


@pytest.mark.parametrize('field', ['peer_id', 'pid', 'image', 'reader', 'identity'])
def test_sample_cannot_retarget_public_pin_before_passing_it_to_load(setup, field):
    owner, factory, _, *_ = setup
    pins = owner.launch_owned()
    original = factory.sample
    called = []
    def sample(pid, image):
        observed = original(pid, image)
        if pid == 1000:
            value = {'peer_id': 'other-peer', 'pid': 12345, 'image': object(),
                     'reader': object(), 'identity': replace(pins[0].identity, start_seconds=99)}[field]
            setattr(pins[0], field, value)
        return observed
    factory.sample = sample
    with pytest.raises(launcher.LauncherError):
        owner.run_load(lambda _: called.append(True))
    assert called == [] and factory.events == []


@pytest.mark.parametrize('error', [KeyboardInterrupt('PRIVATE'), SystemExit('PRIVATE'),
                                 GeneratorExit('PRIVATE'), BaseException('PRIVATE')])
def test_unwind_preserves_failure_semantics_without_exposing_private_text(setup, error):
    owner, _, _, *_ = setup
    owner.launch_owned()
    def operation(_): raise error
    expected = type(error) if type(error) is not BaseException else launcher.LauncherError
    with pytest.raises(expected) as raised:
        owner.run_load(operation)
    assert 'PRIVATE' not in str(raised.value)
    if type(error) is SystemExit: assert raised.value.code == 1
    with pytest.raises(launcher.LauncherError): owner.run_load(lambda _: None)


def test_closed_image_invalidates_admitted_binding_before_any_input_callback_or_spawn(setup):
    owner, factory, _, _, image, checks = setup
    image.close()
    with pytest.raises(launcher.LauncherError): owner.launch_owned()
    assert factory.calls == [] and checks == []


@pytest.mark.parametrize('failure', ['deadline', 'image_close'])
def test_sample_must_preserve_deadline_and_image_before_next_observation(setup, failure):
    owner, factory, clock, _, image, _ = setup
    owner.launch_owned()
    original = factory.sample
    observed_pids = []
    def sample(pid, source):
        observed = original(pid, source)
        observed_pids.append(pid)
        if failure == 'deadline': clock.now = clock.end(601)
        else: image.close()
        return observed
    factory.sample = sample
    with pytest.raises(launcher.LauncherError):
        owner.run_load(lambda _: pytest.fail('invalid sample entered load'))
    assert observed_pids == [1000] and factory.events == []


@pytest.mark.parametrize('where', ['poll', 'terminate', 'wait'])
@pytest.mark.parametrize('error', [KeyboardInterrupt('PRIVATE'), SystemExit('PRIVATE'),
                                 GeneratorExit('PRIVATE'), BaseException('PRIVATE')])
def test_cleanup_also_redacts_control_flow_errors_and_keeps_original_handles(setup, where, error):
    owner, factory, clock, *_ = setup
    owner.launch_owned()
    def fail(*_, **__): raise error
    setattr(factory.children[3], where, fail)
    expected = type(error) if type(error) is not BaseException else launcher.LauncherError
    with pytest.raises(expected) as raised:
        owner.cleanup(clock.end())
    assert 'PRIVATE' not in str(raised.value)
    if type(error) is SystemExit: assert raised.value.code == 1
    assert len(owner._children) == 4
    with pytest.raises(launcher.LauncherError): owner.verify_stopped()
