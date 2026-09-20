"""Fixed native readiness invocation with real private FDs and fake OS children."""
import base64
import hashlib
import os
from dataclasses import replace

import pytest
from scaling_readiness_fixture import ready_setup, compact, command, inputs, launcher, readiness


def complete(case):
    pins = case.run.launch_owned()
    receipts = case.run.await_genesis_ready(case.step)
    return pins, receipts


def test_fixed_four_peer_commands_original_descriptors_receipts_and_required_load_barrier(ready_setup):
    c = ready_setup
    pins, receipts = complete(c)
    assert c.run.run_load(lambda received: received) == pins
    assert len(receipts) == len(c.commands.calls) == len(c.daemons.calls) == 4
    assert len({receipt.challenge for receipt in receipts}) == 4
    for index, (argv, kwargs) in enumerate(c.commands.calls):
        role, receipt = c.inputs.roles[index], receipts[index]
        fd = c.inputs.client_fd(index)
        assert argv == (str(c.images[1].path), '--machine', '--config-fd', str(fd),
            '--config-source-path', str(role.client_config), '--output-format', 'json',
            'bridge', 'genesis-readiness', '--challenge', receipt.challenge,
            '--node-public-key', role.node_public_key, '--genesis-hash', c.inputs.genesis_hash[5:69],
            '--context-id', c.inputs.context_id[5:69], '--request-timeout-ms', '60000')
        assert kwargs == dict(stdin=command.subprocess.DEVNULL, stdout=command.subprocess.PIPE,
            stderr=command.subprocess.PIPE, cwd='/', env={}, close_fds=True, pass_fds=(fd,),
            shell=False, start_new_session=False, bufsize=0)
        assert os.pread(fd, 1024, 0) == role.client_config.read_bytes()
        assert receipt.process == pins[index].identity and receipt.peer_id == role.peer_id
        assert receipt.cli_process.pid == 2000 + index and receipt.cli_sha256 == c.images[1].sha256
        assert receipt.client_config_sha256 == hashlib.sha256(role.client_config.read_bytes()).hexdigest()
        assert receipt.anchors_sha256 == c.inputs.anchors_sha256
        assert receipt.attestation == b'native-test-bytes'
        assert receipt.node_id == role.node_public_key and receipt.network_id == c.inputs.network_id
        assert c.commands.children[index].stdout.closed and c.commands.children[index].stderr.closed
    assert c.runtime_checks and c.run._phase == 'loaded' and c.step._phase == 'ready'


def test_load_before_readiness_fails_permanently_without_cli_or_load(ready_setup):
    c = ready_setup
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.run_load(lambda _: pytest.fail('early load'))
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert c.commands.calls == [] and c.daemons.events == []


@pytest.mark.parametrize('field', ['deadline', 'role', 'config', 'store', 'type'])
def test_readiness_step_must_match_original_trial_and_roles_before_spawn(ready_setup, field):
    c = ready_setup
    c.run.launch_owned()
    step = c.step
    if field == 'deadline': step._end += 1
    elif field == 'type': step = object()
    else:
        role = c.inputs.roles[0]
        key = {'role': 'peer_id', 'config': 'node_config', 'store': 'block_store'}[field]
        replacement = 'wrong-peer' if field == 'role' else c.root / 'wrong'
        c.inputs._anchors = replace(c.inputs._anchors, roles=(replace(role, **{key: replacement}), *c.inputs.roles[1:]))
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(step)
    assert c.commands.calls == [] and c.daemons.events == []


@pytest.mark.parametrize('ready_setup', [1], indirect=True)
@pytest.mark.parametrize('reason', ['consensus_uninitialized', 'genesis_uncommitted'])
def test_only_typed_pending_retries_with_fresh_challenge_original_deadline(ready_setup, reason):
    c = ready_setup
    c.commands.reports = [dict(state='pending', reason=reason, attestation_norito_base64=None)]
    _, receipts = complete(c)
    assert len(c.commands.calls) == 5 and len(receipts) == 4
    assert [argv[-1] for argv, _ in c.commands.calls] == ['1000', '900', '900', '900', '900']
    assert len({argv[argv.index('--challenge') + 1] for argv, _ in c.commands.calls}) == 5
    assert receipts[0].challenge == '0' * 63 + '2'
    assert c.clock.now == 1_100_000_000


@pytest.mark.parametrize('changes', [
    {'state': 'restart_required', 'reason': 'restart_required', 'attestation_norito_base64': None},
    {'state': 'conflict', 'reason': 'tip_changed', 'attestation_norito_base64': None},
    {'state': 'unavailable', 'reason': 'finality_unavailable', 'attestation_norito_base64': None},
    {'state': 'pending', 'reason': 'uninitialized', 'attestation_norito_base64': None},
    {'state': 'pending', 'reason': 'internal_failure', 'attestation_norito_base64': None},
    {'state': 'pending', 'reason': 'genesis_uncommitted'},
    {'state': 'ready', 'reason': 'genesis_uncommitted'},
    {'state': 'ready', 'attestation_norito_base64': None},
    {'state': 'ready', 'attestation_norito_base64': ''},
    {'state': 'Ready'}, {'state': True}, {'version': True}, {'version': 2},
    {'challenge': 'a' * 64}, {'node_id': 'wrong-node'}, {'network_id': 'wrong-network'},
    {'genesis_hash': 'wrong-genesis'}, {'context_id': 'wrong-context'}, {'extra': 'PRIVATE'},
    {'attestation_norito_base64': '***'}, {'attestation_norito_base64': 'Zh=='},
    {'attestation_norito_base64': 'bmF0aXZlLXRlc3QtYnl0ZXM=\n'},
])
def test_unsigned_failure_or_wrong_report_never_ready_or_retried(ready_setup, changes):
    c = ready_setup
    c.commands.reports = [changes]
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    with pytest.raises(launcher.LauncherError): c.run.run_load(lambda _: pytest.fail('failed readiness entered load'))
    assert len(c.commands.calls) == 1 and c.step._receipts == []
    assert c.run._readiness is c.step and c.step._phase == 'failed'


@pytest.mark.parametrize('raw', [b'', b'{}', b'{}\n{}\n', b' {"version":1}\n',
    b'{\n"version":1}\n', b'{"version":1,"version":1}\n', b'{"version":1.0}\n',
    b'{"version":NaN}\n', b'\xff\n', '{"version":1}\n'.encode('utf-16')])
def test_report_framing_duplicate_numbers_and_encoding_fail_closed(ready_setup, raw):
    c = ready_setup
    c.commands.raws[0] = raw
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert len(c.commands.calls) == 1 and c.step._receipts == []


@pytest.mark.parametrize('nonce', ['0' * 64, 'A' * 64, '1' * 63, None])
def test_bad_challenge_fails_before_cli_spawn(ready_setup, monkeypatch, nonce):
    c = ready_setup
    monkeypatch.setattr(readiness.secrets, 'token_hex', lambda _: nonce)
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert c.commands.calls == []


def test_reused_pending_challenge_fails_before_second_cli_spawn(ready_setup, monkeypatch):
    c = ready_setup
    monkeypatch.setattr(readiness.secrets, 'token_hex', lambda _: '1' * 64)
    c.commands.reports = [dict(state='pending', reason='genesis_uncommitted', attestation_norito_base64=None)]
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert len(c.commands.calls) == 1 and c.step._receipts == []


@pytest.mark.parametrize('failure', ['stdout', 'stderr', 'nonzero', 'pin', 'spawn', 'late', 'wait_timeout', 'drift'])
def test_command_failures_are_bounded_original_children_retained_and_cleanup_cannot_revive(ready_setup, monkeypatch, failure):
    c = ready_setup
    if failure == 'stdout': monkeypatch.setattr(readiness, 'MAX_REPORT_BYTES', 8)
    if failure == 'stderr': c.commands.diagnostics[0] = b'x' * 10; monkeypatch.setattr(command, 'MAX_OUTPUT_BYTES', 8)
    if failure == 'nonzero': c.commands.after_spawn = lambda child: setattr(child, 'status', 7)
    if failure == 'pin': c.commands.fail_pin = True
    if failure == 'spawn': c.commands.fail_spawn = 0
    if failure == 'late': c.commands.after_spawn = lambda child: setattr(c.clock, 'now', c.clock.end(601))
    if failure == 'wait_timeout': c.commands.after_spawn = lambda child: setattr(child, 'stall', True)
    if failure == 'drift':
        original = c.commands.sample
        def sample(pid, image):
            observed = original(pid, image)
            c.commands.children[0].drift = True
            return observed
        c.commands.sample = sample
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError) as caught: c.run.await_genesis_ready(c.step)
    assert 'PRIVATE' not in str(caught.value) and c.run._readiness is c.step
    expected = 0 if failure in ('spawn', 'stderr') else 1
    assert len(c.step._commands._children) == expected
    for child in c.commands.children: assert child.stdout.closed and child.stderr.closed
    pending = c.run.cleanup(c.clock.end(20))
    assert pending == (('readiness-peer0',) if failure == 'wait_timeout' else ())
    assert len(c.commands.calls) == (0 if failure == 'stderr' else 1)
    with pytest.raises(launcher.LauncherError): c.run.run_load(lambda _: pytest.fail('cleanup entered load'))


@pytest.mark.parametrize('peer', range(4))
def test_daemon_exit_during_cli_prevents_all_later_receipts(ready_setup, peer):
    c = ready_setup
    c.commands.after_spawn = lambda _: setattr(c.daemons.children[peer], 'status', 0)
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert len(c.commands.calls) == 1 and c.step._receipts == []


@pytest.mark.parametrize('when', ['before', 'during', 'after'])
def test_original_client_replacement_at_every_command_boundary_fails(ready_setup, when):
    c = ready_setup
    c.run.launch_owned()
    def change():
        path = c.inputs.roles[0].client_config
        raw = path.read_bytes()
        path.unlink(); path.write_bytes(raw); path.chmod(0o600)
    if when == 'before': change()
    if when == 'during': c.commands.after_spawn = lambda _: change()
    if when == 'after': c.commands.after_spawn = lambda child: setattr(child, 'wait_hook', change)
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert len(c.commands.calls) == (0 if when == 'before' else 1)
    assert c.step._receipts == []


def test_original_runtime_closure_guard_is_mandatory_and_postchecked(ready_setup):
    c = ready_setup
    live = [True]
    def verify():
        if not live[0]: raise ValueError('PRIVATE RUNTIME PATH')
    c.step._verify_runtime = verify
    c.commands.after_spawn = lambda child: setattr(child, 'wait_hook', lambda: live.__setitem__(0, False))
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert len(c.commands.calls) == 1 and c.step._receipts == []


def test_all_ready_step_cannot_be_reused(ready_setup):
    c = ready_setup
    pins, _ = complete(c)
    with pytest.raises(readiness.ReadinessError): c.step.collect(pins, lambda: None)
    assert len(c.commands.calls) == 4


def test_attestation_decoded_cap_is_enforced_after_fixed_native_command(ready_setup, monkeypatch):
    c = ready_setup
    monkeypatch.setattr(readiness, 'MAX_ATTESTATION_BYTES', 2)
    c.commands.reports = [dict(attestation_norito_base64=base64.b64encode(b'abc').decode())]
    c.run.launch_owned()
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert c.step._receipts == []


@pytest.mark.parametrize('field', ['image', 'anchors', 'input'])
def test_runtime_callback_cannot_refresh_original_readiness_binding(ready_setup, field):
    c = ready_setup
    c.run.launch_owned()
    changed = []
    def retarget():
        if changed: return
        changed.append(True)
        if field == 'image': c.images[1].path = c.images[0].path
        if field == 'anchors': c.inputs._anchors = replace(c.inputs._anchors, context_id=c.inputs.genesis_hash)
        if field == 'input': c.step._inputs = object()
    c.step._verify_runtime = retarget
    with pytest.raises(launcher.LauncherError): c.run.await_genesis_ready(c.step)
    assert c.commands.calls == [] and c.step._receipts == []
