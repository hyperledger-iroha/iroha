"""Fixed generator selection, original output equality and no false readiness."""
from dataclasses import asdict, replace
import hashlib
import json
import os
from pathlib import Path
import stat

import pytest
from scaling_generator_fixture import generator_setup, generator, SEED, refresh
from scaling_readiness_fixture import command, inputs, ready_setup


def test_fixed_argv_public_receipt_and_original_inputs_remain_owned(generator_setup):
    c = generator_setup
    owner = c.create()
    generated = owner.generate(SEED)
    receipt, original = generated.receipt, generated.inputs
    argv, kwargs = c.factory.calls[0]
    assert argv == (str(c.image.path), '--ui-mode', 'plain', 'localnet', '--peers', '4',
        '--consensus-mode', 'npos', '--scaling-lanes', '4', '--scaling-accounts', '4', '--seed-fd', str(kwargs['pass_fds'][0]),
        '--chain-id', 'test-chain', '--bind-host', '127.0.0.1', '--public-host', '127.0.0.1',
        '--base-api-port', '8080', '--base-p2p-port', '1337', '--out-dir', str(original.input_directory))
    assert kwargs == dict(stdin=command.subprocess.DEVNULL, stdout=command.subprocess.PIPE,
        stderr=command.subprocess.PIPE, cwd='/', env={}, close_fds=True, pass_fds=(int(argv[argv.index('--seed-fd')+1]),), shell=False,
        start_new_session=False, bufsize=0)
    assert len(kwargs['pass_fds']) == 1 and '--seed' not in argv and SEED not in repr(argv)
    assert owner._seed_pipe is None
    assert receipt.plan == c.plan and receipt.process.pid == 2000 and receipt.generator_sha256 == c.image.sha256
    assert len(receipt.peers) == 4 and len(receipt.accounts) == 4 and len(receipt.artifacts) == 17
    assert receipt.anchors_bytes == len(original.anchor_bytes())
    assert receipt.anchors_sha256 == hashlib.sha256(original.anchor_bytes()).hexdigest()
    assert SEED not in repr(receipt) and 'PRIVATE' not in repr(receipt)
    assert stat.S_IMODE(original.input_directory.stat().st_mode) == 0o700 and c.checks
    for index, peer in enumerate(receipt.peers):
        assert peer.role.peer_id == f'peer{index}' and peer.role.block_store == original.input_directory / 'storage' / f'peer{index}' / 'kura'
        assert peer.state_root == original.input_directory / 'storage' / f'peer{index}' / 'state'
        assert peer.p2p_bind == f'addr:127.0.0.1:{1337 + index}#ABCD'
        assert peer.torii_bind == f'addr:127.0.0.1:{8080 + index}#ABCD'
    assert c.factory.children[0].returncode == 0 and c.factory.children[0].stdout.closed
    generated.validate()
    generated.close()
    assert c.image.fd >= 0
    with pytest.raises(generator.GeneratorError): generated.validate()


@pytest.mark.parametrize('lanes,accounts,host', [(1, 4, '127.0.0.1'), (4, 64, '127.0.0.1'), (4, 8, '::1'), (1, 12, 'localhost')])
def test_fixed_geometry_public_projection_and_upper_account_bound(generator_setup, lanes, accounts, host):
    c = generator_setup
    owner = c.create(replace(c.plan, lane_count=lanes, account_count=accounts, bind_host=host, public_host=host))
    result = owner.generate(SEED)
    assert result.receipt.plan.lane_count == lanes and len(result.receipt.accounts) == accounts
    assert len(result.receipt.artifacts) == 13 + accounts and len(c.factory.calls) == 1


@pytest.mark.parametrize('field,value', [('chain_id', ''), ('chain_id', 'bad\nchain'), ('chain_id', 'x' * 129),
    ('lane_count', True), ('lane_count', 2), ('account_count', 3), ('account_count', 5), ('account_count', 68),
    ('bind_host', ' 127.0.0.1'), ('bind_host', '127.0.0.1:1'), ('public_host', 'EXAMPLE.COM'),
    ('public_host', 'example.com/'), ('public_host', '[::1]'), ('public_host', '::1%lo0'),
    ('base_api_port', 0), ('base_api_port', 65533), ('base_api_port', True), ('base_p2p_port', 8083)])
def test_invalid_plan_fails_before_new_namespace_or_spawn(generator_setup, field, value):
    c = generator_setup
    output = c.root / 'never-created'
    with pytest.raises(generator.GeneratorError): c.create(replace(c.plan, **{field: value}), output)
    assert not output.exists() and c.factory.calls == []


@pytest.mark.parametrize('seed', [None, '', '0' * 64, 'A' * 64, '1' * 63, 'private-text'])
def test_seed_is_runtime_only_strict_and_invalid_seed_cannot_retry(generator_setup, seed):
    c = generator_setup
    owner = c.create()
    with pytest.raises(generator.GeneratorError) as caught: owner.generate(seed)
    assert str(caught.value) == 'fixed_generator_failed'
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert c.factory.calls == [] and owner._inputs is None


@pytest.mark.parametrize('kind', ['directory', 'file', 'symlink'])
def test_existing_output_namespace_is_never_adopted(generator_setup, kind):
    c = generator_setup
    output = c.root / 'preexisting'
    if kind == 'directory': output.mkdir(mode=0o700)
    if kind == 'file': output.write_bytes(b'PRIVATE')
    if kind == 'symlink': output.symlink_to(c.root / 'inputs')
    with pytest.raises(generator.GeneratorError): c.create(output=output)
    assert c.factory.calls == [] and output.exists()


@pytest.mark.parametrize('change', ['stdout', 'empty_stdout', 'partial_stdout', 'extra_stdout', 'anchor_replace',
    'anchor_symlink', 'anchor_mode', 'anchor_hardlink', 'anchor_missing', 'nonzero', 'spawn', 'pin', 'wait_timeout'])
def test_zero_exit_exact_stdout_and_original_anchor_are_all_required(generator_setup, change):
    c = generator_setup
    owner = c.create()
    if change == 'stdout': c.factory.transform_stdout = lambda _: b'PRIVATE\n'
    if change == 'empty_stdout': c.factory.transform_stdout = lambda _: b''
    if change == 'partial_stdout': c.factory.transform_stdout = lambda raw: raw[:-1]
    if change == 'extra_stdout': c.factory.transform_stdout = lambda raw: raw + b'{}\n'
    def alter(root, value):
        path = root / 'genesis-anchors.json'
        if change == 'anchor_replace':
            raw = path.read_bytes(); path.unlink(); path.write_bytes(raw + b' '); path.chmod(0o600)
        if change == 'anchor_symlink':
            saved = c.root / 'foreign-anchor'; path.rename(saved); path.symlink_to(saved)
        if change == 'anchor_mode': path.chmod(0o644)
        if change == 'anchor_hardlink': os.link(path, c.root / 'second-anchor')
        if change == 'anchor_missing': path.unlink()
    c.factory.after_files = alter
    if change == 'nonzero': c.factory.after_spawn = lambda child: setattr(child, 'status', 9)
    if change == 'spawn': c.factory.fail_spawn = 0
    if change == 'pin': c.factory.fail_pin = True
    if change == 'wait_timeout': c.factory.after_spawn = lambda child: setattr(child, 'stall', True)
    with pytest.raises(generator.GeneratorError) as caught: owner.generate(SEED)
    assert 'PRIVATE' not in str(caught.value) and owner._generated is None
    assert len(c.factory.calls) == 1
    assert owner.cleanup(c.clock.end()) == (('generator',) if change == 'wait_timeout' else ())
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)


@pytest.mark.parametrize('change', ['chain', 'lanes', 'accounts', 'peer_order', 'client_url', 'kura_path',
    'p2p_bind', 'p2p_public', 'torii_bind', 'transport_missing', 'unknown_static'])
def test_exact_stdout_with_wrong_native_public_projection_cannot_mint_inputs(generator_setup, change):
    c = generator_setup
    owner = c.create()
    def alter(root, value):
        if change == 'chain': value['chain_id'] = 'other-chain'
        if change == 'lanes': value['lane_count'] = 1
        if change == 'accounts': value['accounts'].pop(); value['artifacts'] = [row for row in value['artifacts'] if row['path'] != 'workload-account-03.toml']; (root/'workload-account-03.toml').unlink()
        if change == 'peer_order': value['peers'].reverse()
        if change == 'client_url': value['peers'][0]['torii_url'] = 'http://127.0.0.1:9090/'
        if change == 'unknown_static': (root / 'start.sh').write_text('PRIVATE retired scaffold')
        node = root / 'peer0.toml'; raw = node.read_text()
        if change == 'kura_path': raw = raw.replace(str(root / 'storage/peer0/kura'), str(root / 'storage/peer1/kura'))
        if change == 'p2p_bind': raw = raw.replace('address = "addr:127.0.0.1:1337', 'address = "addr:127.0.0.1:2337', 1)
        if change == 'p2p_public': raw = raw.replace('public_address = "addr:127.0.0.1:1337', 'public_address = "addr:127.0.0.1:2337')
        if change == 'torii_bind': raw = raw.replace('addr:127.0.0.1:8080', 'addr:127.0.0.1:9090')
        if change == 'transport_missing': raw = raw.replace('[network]', '[retired_network]')
        node.write_text(raw)
    c.factory.change_files = alter
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert owner._generated is None and len(c.factory.calls) == 1


@pytest.mark.parametrize('change', ['missing_peer', 'extra_peer', 'missing_state', 'extra_leaf', 'nonempty_kura',
    'nonempty_state', 'state_symlink', 'state_mode'])
def test_exact_initial_mutable_namespace_census_is_required(generator_setup, change):
    c = generator_setup
    owner = c.create()
    def alter(root, value):
        storage, peer = root / 'storage', root / 'storage/peer0'
        if change == 'missing_peer': (storage/'peer3/kura').rmdir(); (storage/'peer3/state').rmdir(); (storage/'peer3').rmdir()
        if change == 'extra_peer': (storage/'peer4').mkdir(mode=0o700)
        if change == 'missing_state': (peer/'state').rmdir()
        if change == 'extra_leaf': (peer/'extra').mkdir(mode=0o700)
        if change == 'nonempty_kura': (peer/'kura/block').write_bytes(b'old-state')
        if change == 'nonempty_state': (peer/'state/old').write_bytes(b'old-state')
        if change == 'state_symlink': (peer/'state').rmdir(); (peer/'state').symlink_to(storage/'peer1/state')
        if change == 'state_mode': (peer/'state').chmod(0o755)
    c.factory.change_files = alter
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert owner._generated is None


@pytest.mark.parametrize('change', ['plan', 'deadline', 'nested', 'cleanup', 'runtime'])
def test_runtime_callback_cannot_change_declared_admission_or_revive_generation(generator_setup, change):
    c = generator_setup
    mutated = []
    def guard():
        if mutated: return
        mutated.append(True)
        if change == 'plan': object.__setattr__(owner._plan, 'lane_count', 1)
        if change == 'deadline': owner._end += 1
        if change == 'nested':
            with pytest.raises(generator.GeneratorError): owner.generate(SEED)
        if change == 'cleanup': owner.cleanup(c.clock.end())
        if change == 'runtime': raise generator.GeneratorError('PRIVATE RUNTIME INPUT')
    owner = c.create(guard=guard)
    with pytest.raises(generator.GeneratorError) as caught: owner.generate(SEED)
    assert 'PRIVATE' not in str(caught.value) and c.factory.calls == []


def test_original_deadline_expires_during_generation_without_later_admission(generator_setup):
    c = generator_setup
    owner = c.create()
    c.factory.after_spawn = lambda _: setattr(c.clock, 'now', owner.trial_deadline_ns)
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert owner._inputs is None and len(c.factory.calls) == 1


@pytest.mark.parametrize('target', ['root', 'peer', 'state', 'kura', 'file'])
def test_generated_inputs_keep_original_ancestor_and_file_custody_until_closed(generator_setup, target):
    c = generator_setup
    owner = c.create(); result = owner.generate(SEED)
    root = result.inputs.input_directory
    path = {'root': root, 'peer': root/'storage/peer0', 'state': root/'storage/peer0/state',
            'kura': root/'storage/peer0/kura', 'file': root/'peer0-client.toml'}[target]
    if target == 'file':
        raw = path.read_bytes(); path.unlink(); path.write_bytes(raw); path.chmod(0o600)
    else:
        path.rename(path.with_name(path.name + '-original')); path.mkdir(mode=0o700)
    with pytest.raises(generator.GeneratorError): result.validate()
    with pytest.raises(generator.GeneratorError): result.receipt
    assert owner._phase == 'failed'


def test_cleanup_reaps_only_generator_and_preserves_borrowed_inputs_and_external_image(generator_setup):
    c = generator_setup
    owner = c.create(); result = owner.generate(SEED)
    borrowed = result.inputs
    fd = borrowed.client_fd(0)
    assert owner.cleanup(c.clock.end()) == ()
    assert os.fstat(fd).st_size > 0 and c.image.fd >= 0
    with pytest.raises(generator.GeneratorError): result.validate()
    owner.close()
    with pytest.raises(OSError): os.fstat(fd)
    assert c.image.fd >= 0


def test_close_requires_original_child_reap_and_never_signals_as_a_side_effect(generator_setup):
    c = generator_setup
    owner = c.create()
    c.factory.after_spawn = lambda child: setattr(child, 'stall', True)
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    with pytest.raises(generator.GeneratorError): owner.close()
    assert not [event for event in c.factory.events if event[0] == 'terminate']
    assert owner.cleanup(c.clock.end()) == ('generator',)


@pytest.mark.parametrize('field,relative', generator._RUNTIME_PATHS)
@pytest.mark.parametrize('mutation', ['foreign_role', 'relative', 'missing'])
def test_every_original_runtime_path_is_explicit_and_belongs_to_exact_role(generator_setup, field, relative, mutation):
    c = generator_setup
    owner = c.create()
    def alter(root, value):
        node = root / 'peer0.toml'
        text = node.read_text()
        old = str(root / 'storage/peer0' / relative)
        if mutation == 'foreign_role': text = text.replace(old, str(root / 'storage/peer1' / relative))
        if mutation == 'relative': text = text.replace(old, relative)
        if mutation == 'missing':
            key = field.rsplit('.', 1)[-1]
            text = text.replace(f'{key} = {json.dumps(old)}', f'retired_{key} = {json.dumps(old)}')
        node.write_text(text)
    c.factory.change_files = alter
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert owner._generated is None and len(c.factory.calls) == 1


def test_retained_runtime_projection_names_match_fixed_native_owner(generator_setup):
    c = generator_setup
    result = c.create().generate(SEED)
    for index, peer in enumerate(result.receipt.peers):
        root = result.inputs.input_directory / 'storage' / f'peer{index}'
        assert peer.runtime_paths == tuple((field, str(root / relative)) for field, relative in generator._RUNTIME_PATHS)
        assert not any('vrf_state_path' in field or 'drand.state_path' in field for field, _ in peer.runtime_paths)
        assert result.inputs.node_runtime_paths(index) == peer.runtime_paths


def test_output_cap_refuses_overrun_and_never_parses_or_publishes_inputs(generator_setup):
    c = generator_setup
    owner = c.create()
    c.factory.transform_stdout = lambda _: b'x' * (inputs.MAX_ANCHORS_BYTES + 1)
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert owner._inputs is None and owner._generated is None and len(c.factory.calls) == 1
    assert c.factory.children[0].stdout.closed and c.factory.children[0].stderr.closed


def test_error_from_original_child_poll_is_redacted_without_closing_any_input(generator_setup):
    c = generator_setup
    owner = c.create(); result = owner.generate(SEED)
    child = c.factory.children[0]
    fd = result.inputs.client_fd(0)
    original = child.poll
    def fail(): raise OSError('PRIVATE RUNTIME SEED')
    child.poll = fail
    with pytest.raises(generator.GeneratorError) as caught: owner.close()
    assert str(caught.value) == 'fixed_generator_failed' and os.fstat(fd).st_size > 0
    child.poll = original
    owner.close()


def test_constructor_failure_releases_every_directory_descriptor_without_adopting_output(generator_setup, monkeypatch):
    c = generator_setup
    existing = c.root / 'original-empty'; existing.mkdir(mode=0o700)
    opened, closed = [], []
    original_open, original_close = os.open, os.close
    def open(*args, **kwargs):
        fd = original_open(*args, **kwargs); opened.append(fd); return fd
    def close(fd): closed.append(fd); original_close(fd)
    monkeypatch.setattr(os, 'open', open); monkeypatch.setattr(os, 'close', close)
    with pytest.raises(generator.GeneratorError): c.create(output=existing)
    assert sorted(opened) == sorted(closed) and len(opened) > 1
    assert not list(existing.iterdir()) and c.factory.calls == []


def test_generated_context_manager_closes_only_owned_input_lifetime(generator_setup):
    c = generator_setup
    owner = c.create()
    with owner.generate(SEED) as result:
        fd = result.inputs.client_fd(0)
        assert result.receipt.plan == c.plan and os.fstat(fd).st_size > 0
    with pytest.raises(OSError): os.fstat(fd)
    assert c.image.fd >= 0 and owner._phase == 'closed'


@pytest.mark.parametrize('error', [KeyboardInterrupt('PRIVATE'), SystemExit('PRIVATE'), GeneratorExit('PRIVATE')])
def test_runtime_cancellation_is_redacted_and_cannot_resume_generation(generator_setup, error):
    c = generator_setup
    def guard(): raise error
    owner = c.create(guard=guard)
    with pytest.raises(type(error)) as caught: owner.generate(SEED)
    assert 'PRIVATE' not in str(caught.value) and c.factory.calls == [] and owner._phase == 'failed'
    if isinstance(error, SystemExit): assert caught.value.code == 1
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)


@pytest.mark.parametrize('target', ['storage', 'peer0', 'peer0/kura', 'peer0/state'])
@pytest.mark.parametrize('transient', [False, True])
def test_initial_census_rejects_earlier_directory_mutation_during_later_scan(generator_setup, monkeypatch, target, transient):
    c = generator_setup
    owner = c.create()
    namespace = owner._namespace
    original = namespace.census
    changed = []
    def scan(path, expected):
        original(path, expected)
        if path == namespace.root / 'storage/peer3/state':
            directory = namespace.root / 'storage' if target == 'storage' else namespace.root / 'storage' / target
            before = directory.stat()
            intruder = directory / 'unexpected'
            intruder.write_bytes(b'unadmitted-state')
            if transient: intruder.unlink()
            # Force deterministic metadata drift even on coarse timestamp filesystems.
            os.utime(directory, ns=(before.st_atime_ns, before.st_mtime_ns + 1_000_000_000))
            changed.append(directory)
    monkeypatch.setattr(namespace, 'census', scan)
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert len(changed) == 1 and owner._generated is None
    assert owner.cleanup(c.clock.end()) == ()


def test_initial_census_remains_guarded_through_last_runtime_callback(generator_setup):
    c = generator_setup
    changes = []
    def guard():
        if owner._inputs is not None and len(owner._namespace.entries) >= 13 and not changes:
            leaf = owner._namespace.root / 'storage/peer0/kura'
            if leaf in owner._namespace.entries:
                (leaf / 'late-unadmitted-block').write_bytes(b'unadmitted-state')
                changes.append(leaf)
    owner = c.create(guard=guard)
    with pytest.raises(generator.GeneratorError): owner.generate(SEED)
    assert len(changes) == 1 and owner._generated is None


def test_live_generation_validation_allows_writes_after_initial_admission(generator_setup):
    c = generator_setup
    result = c.create().generate(SEED)
    for peer in result.receipt.peers:
        (peer.role.block_store / 'block').write_bytes(b'live-block')
        (peer.state_root / 'snapshot').mkdir(mode=0o700)
    result.validate()
    assert len(result.receipt.peers) == 4
