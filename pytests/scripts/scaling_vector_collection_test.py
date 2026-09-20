"""Actual bounded-command and retained-file composition; no native processes."""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest

from scaling_readiness_fixture import ready_setup, compact
from scaling_native_outputs_test import budget, write
import scaling_command as command
import scaling_native_outputs as outputs
import scaling_proof_sequence as sequence
import scaling_vector_collection as vector


def allocation():
    return vector.CollectionLimits(65536, 65536, 4 * 65536, 4096, 1000, 100, 1024 * 1024)


def stopped(c):
    role = c.inputs.roles[3]
    return sequence.StoppedReader(role.primary_block_store, role.primary_merge_log, 1, 100, 100,
        8 * 1024 * 1024, 65536, 65536, 100, 65536, 65536, os.geteuid())


@pytest.fixture
def collection(ready_setup, monkeypatch):
    c = ready_setup
    native = outputs.NativeOutputs(c.root / 'native-vector-output', budget())
    p = SimpleNamespace(c=c, outputs=native, stopped=stopped(c), limits=allocation(),
        guard=c.inputs.validate, before_spawn=lambda argv: None, transform=lambda raw: raw,
        change_reply=lambda reply: None, after_files=lambda: None, owners=[], replies=[])
    def spawn(argv, **kwargs):
        p.before_spawn(argv)
        get = lambda flag: argv[argv.index(flag) + 1]
        context = next(a for a in c.inputs.generation.artifacts if a.path == 'genesis-context.nrt')
        reply = dict(version=1, operation='collect_scaling_inputs', invocation_id=get('--invocation-id'),
            client_config_sha256=c.inputs.roles[0].client_config_sha256,
            committed_height=100, finality_count=100, query_count=8,
            context_sha256=context.sha256, context_bytes=context.bytes)
        for role in ('finality', 'queries'):
            path = Path(get('--' + role + '-out'))
            stage = path.with_name(path.name + '.collecting')
            digest, length = write(stage, ('test-native-vector-' + role).encode())
            stage.rename(path)
            reply[role + '_sha256'], reply[role + '_bytes'] = digest, length
        p.after_files()
        p.change_reply(reply)
        p.replies.append(dict(reply))
        c.commands.raws[len(c.commands.calls)] = p.transform(compact(reply))
        return c.commands(argv, **kwargs)
    monkeypatch.setattr(command.subprocess, 'Popen', spawn)
    yield p
    # Test-owned fake children only; borrowed runtime/image lifetimes belong to ready_setup.
    for child in c.commands.children: child.returncode = 0
    for owner in p.owners: owner.cleanup(c.clock.end())
    native.close()


def owner(p):
    value = vector.NativeVectorCollection(p.c.inputs, p.outputs, p.stopped, p.limits,
        p.c.images[1], p.c.commands, p.c.clock.end(60), p.guard)
    p.owners.append(value)
    return value


def test_exact_command_and_complete_pair_keep_original_inputs_owned(collection):
    p = collection
    value = owner(p)
    receipt = value.run()
    assert receipt.stopped_height == receipt.finality_count == 100 and receipt.query_count == 8
    assert receipt.process.pid == 2000 and receipt.cli_sha256 == p.c.images[1].sha256
    assert receipt.anchors_sha256 == p.c.inputs.anchors_sha256
    argv, options = p.c.commands.calls[0]
    fd = p.c.inputs.client_fd(0)
    assert argv[:10] == (str(p.c.images[1].path), '--machine', '--config-fd', str(fd),
        '--config-source-path', str(p.c.inputs.roles[0].client_config), '--output-format', 'json',
        'tx', 'collect-scaling-inputs')
    flags = dict(zip(argv[10::2], argv[11::2], strict=True))
    assert len(flags) == 27 and len(argv) == 64
    assert flags == {'--invocation-id': receipt.invocation_id, '--network-id': p.c.inputs.network_id,
        '--client-config-sha256': receipt.client_config_sha256, '--client-config-max-bytes': '65536',
        '--deadline-monotonic-ns': str(value._original_end), '--block-store': str(p.stopped.block_store),
        '--merge-log': str(p.stopped.merge_log), '--context': str(p.c.inputs.input_directory / 'genesis-context.nrt'),
        '--context-sha256': receipt.context_sha256, '--context-max-bytes': '65536',
        '--finality-out': str(receipt.finality.path), '--queries-out': str(receipt.queries.path),
        '--finality-max-bytes': '65536', '--queries-max-bytes': '65536', '--total-max-bytes': str(4 * 65536),
        '--reply-max-bytes': '4096', '--last-height': '100', '--max-committed-blocks': '100',
        '--max-store-data-bytes': str(8 * 1024 * 1024), '--max-carrier-bytes': '65536',
        '--max-merge-log-bytes': '65536', '--max-merge-frames': '100', '--max-input-bytes': '65536',
        '--max-total-leaves': '1000', '--max-leaves-per-carrier': '100', '--max-decode-bytes': str(1024 * 1024),
        '--max-value-decode-bytes': '65536'}
    assert options == dict(stdin=command.subprocess.DEVNULL, stdout=command.subprocess.PIPE,
        stderr=command.subprocess.PIPE, cwd='/', env={}, close_fds=True, pass_fds=(fd,), shell=False,
        start_new_session=False, bufsize=0)
    assert len(value._commands._children) == 1 and value._phase == 'collected'
    assert p.c.commands.children[0].returncode == 0
    for artifact in (receipt.finality, receipt.queries):
        assert artifact == p.outputs.artifact(artifact.role)
        assert hashlib.sha256(os.pread(p.outputs.descriptor(artifact.role), artifact.bytes, 0)).hexdigest() == artifact.sha256
    assert os.fstat(fd).st_size > 0 and 'PRIVATE' not in repr(receipt)
    value.validate()
    assert value.cleanup(p.c.clock.end()) == ()
    p.c.inputs.validate(); p.outputs.validate()
    with pytest.raises(vector.VectorCollectionError): value.run()


@pytest.mark.parametrize('field,bad', [('client_config_max_bytes', 0), ('client_config_max_bytes', 1024 * 1024 + 1),
    ('context_max_bytes', 0), ('context_max_bytes', 8 * 1024 * 1024 + 1), ('total_max_bytes', 1),
    ('total_max_bytes', 256 * 1024 * 1024 + 1), ('reply_max_bytes', True), ('reply_max_bytes', 4097),
    ('max_total_leaves', 0), ('max_total_leaves', 1000001), ('max_leaves_per_carrier', 1001),
    ('max_decode_bytes', 65535), ('max_decode_bytes', 512 * 1024 * 1024 + 1)])
def test_independent_caps_refuse_before_any_child_or_output_slot(collection, field, bad):
    p = collection; p.limits = replace(p.limits, **{field: bad})
    with pytest.raises(vector.VectorCollectionError): owner(p)
    assert not p.c.commands.calls and list(p.outputs.directory.iterdir()) == []
    p.outputs.validate()


@pytest.mark.parametrize('field,bad', [('first_height', 2), ('last_height', 0), ('last_height', 101),
    ('max_committed_blocks', 1000001), ('max_store_data_bytes', 2 * 1024 ** 3 + 1),
    ('max_carrier_bytes', 32 * 1024 ** 2 + 1), ('max_merge_log_bytes', 256 * 1024 ** 2 + 1),
    ('max_merge_frames', 101), ('reader_max_output_bytes', 256 * 1024 ** 2 + 1),
    ('max_decode_allocation_bytes', 1024 ** 2 + 1), ('owner_uid', -1), ('merge_log', Path('relative'))])
def test_complete_original_reader_geometry_is_bounded_before_dispatch(collection, field, bad):
    p = collection; p.stopped = replace(p.stopped, **{field: bad})
    with pytest.raises(vector.VectorCollectionError): owner(p)
    assert not p.c.commands.calls and list(p.outputs.directory.iterdir()) == []


@pytest.mark.parametrize('which', ['kura_root', 'other_peer_store', 'other_peer_merge'])
def test_resolved_original_peer3_paths_cannot_be_replaced_by_config_root_or_other_role(collection, which):
    p = collection
    if which == 'kura_root': p.stopped = replace(p.stopped, block_store=p.c.inputs.roles[3].block_store)
    elif which == 'other_peer_store': p.stopped = replace(p.stopped, block_store=p.c.inputs.roles[0].primary_block_store)
    else: p.stopped = replace(p.stopped, merge_log=p.c.inputs.roles[0].primary_merge_log)
    with pytest.raises(vector.VectorCollectionError): owner(p)
    assert not p.c.commands.calls


@pytest.mark.parametrize('field,bad', [('version', True), ('version', 2), ('operation', 'pending'),
    ('invocation_id', 'a' * 64), ('client_config_sha256', 'a' * 64), ('context_sha256', 'a' * 64),
    ('context_bytes', True), ('context_bytes', 999), ('committed_height', True), ('committed_height', 99),
    ('finality_count', True), ('finality_count', 99), ('query_count', -1), ('query_count', 1001),
    ('finality_sha256', 'A' * 64), ('queries_sha256', 'a' * 64), ('finality_bytes', 0),
    ('finality_bytes', 65537), ('queries_bytes', True), ('queries_bytes', 65537)])
def test_exact_terminal_reply_cannot_mint_a_partial_pair(collection, field, bad):
    p = collection; p.change_reply = lambda value: value.__setitem__(field, bad)
    value = owner(p)
    with pytest.raises(vector.VectorCollectionError): value.run()
    assert value._receipt is None and value._phase == 'failed' and len(p.c.commands.calls) == 1
    with pytest.raises(vector.VectorCollectionError): value.run()


@pytest.mark.parametrize('kind', ['missing', 'extra', 'duplicate', 'float', 'nan', 'huge_int',
    'missing_newline', 'second_line', 'prefix', 'nonascii', 'nested', 'oversize', 'truncated'])
def test_reply_is_one_exact_bounded_closed_json_object(collection, kind):
    p = collection
    def transform(raw):
        if kind == 'missing':
            value = json.loads(raw); del value['query_count']; return compact(value)
        if kind == 'extra':
            value = json.loads(raw); value['private'] = 'PRIVATE'; return compact(value)
        if kind == 'duplicate': return raw.replace(b'{', b'{"version":1,', 1)
        if kind == 'float': return raw.replace(b'"query_count":8', b'"query_count":8.0')
        if kind == 'nan': return raw.replace(b'"query_count":8', b'"query_count":NaN')
        if kind == 'huge_int': return raw.replace(b'"query_count":8', b'"query_count":' + b'1' * 21)
        if kind == 'missing_newline': return raw[:-1]
        if kind == 'second_line': return raw + b'{}\n'
        if kind == 'prefix': return b' ' + raw
        if kind == 'nonascii': return raw.replace(b'collect_scaling_inputs', 'priv\u00e9'.encode())
        if kind == 'nested': return raw.replace(b'"query_count":8', b'"query_count":{}')
        if kind == 'oversize': return b' ' * 4097
        return b'{'
    p.transform = transform; value = owner(p)
    with pytest.raises(vector.VectorCollectionError) as error: value.run()
    assert 'PRIVATE' not in str(error.value) and value._receipt is None


@pytest.mark.parametrize('kind', ['nonzero', 'pin', 'spawn', 'timeout', 'runtime', 'nested', 'cleanup', 'deadline'])
def test_child_or_runtime_failure_never_publishes_a_receipt_and_cannot_retry(collection, kind):
    p = collection
    if kind == 'nonzero': p.c.commands.after_spawn = lambda child: setattr(child, 'status', 9)
    if kind == 'pin': p.c.commands.fail_pin = True
    if kind == 'spawn': p.c.commands.fail_spawn = 0
    if kind == 'timeout': p.c.commands.after_spawn = lambda child: setattr(child, 'stall', True)
    def guard():
        if kind == 'runtime': raise ValueError('PRIVATE RUNTIME')
        if kind == 'nested':
            with pytest.raises(vector.VectorCollectionError): value.run()
        if kind == 'cleanup': value.cleanup(p.c.clock.end())
        if kind == 'deadline': p.c.clock.now = value._original_end
        p.c.inputs.validate()
    if kind in ('runtime', 'nested', 'cleanup', 'deadline'): p.guard = guard
    value = owner(p)
    with pytest.raises(vector.VectorCollectionError) as error: value.run()
    assert str(error.value) == 'vector_collection_failed' and value._receipt is None
    assert value.cleanup(p.c.clock.end()) == (('vector-collection',) if kind == 'timeout' else ())
    with pytest.raises(vector.VectorCollectionError): value.run()


@pytest.mark.parametrize('kind', ['config_bytes', 'context_bytes', 'config_inode', 'context_inode', 'output_inode'])
def test_original_input_and_output_custody_survives_success(collection, kind):
    p = collection; value = owner(p); receipt = value.run()
    path = (p.c.inputs.roles[0].client_config if kind.startswith('config') else
            p.c.inputs.input_directory / 'genesis-context.nrt' if kind.startswith('context') else receipt.finality.path)
    raw = path.read_bytes()
    if kind.endswith('inode'):
        replacement = path.with_name(path.name + '.foreign'); replacement.write_bytes(raw)
        replacement.chmod(0o600); replacement.replace(path)
    else: path.write_bytes(raw + b'PRIVATE')
    with pytest.raises(vector.VectorCollectionError): value.validate()
    assert os.fstat(p.c.inputs.client_fd(0) if not kind.startswith(('config', 'context')) else value._client[3]).st_size > 0


def test_caller_dataclass_mutation_cannot_expand_owned_caps_or_retarget_store(collection):
    p = collection; value = owner(p)
    def guard():
        object.__setattr__(p.stopped, 'block_store', Path('/PRIVATE'))
        object.__setattr__(p.stopped, 'last_height', 1000000)
        object.__setattr__(p.limits, 'context_max_bytes', 8 * 1024 * 1024)
        object.__setattr__(p.limits, 'max_total_leaves', 1000000)
        p.c.inputs.validate()
    value._guard = guard
    receipt = value.run()
    argv = p.c.commands.calls[0][0]
    assert argv[argv.index('--block-store') + 1] != '/PRIVATE'
    assert argv[argv.index('--last-height') + 1] == '100'
    assert argv[argv.index('--context-max-bytes') + 1] == '65536'
    assert receipt.finality_count == 100


def test_later_pair_digest_failure_keeps_first_fd_but_makes_entire_pair_unavailable(collection):
    p = collection
    p.change_reply = lambda reply: reply.__setitem__('queries_sha256', 'a' * 64)
    value = owner(p)
    with pytest.raises(vector.VectorCollectionError): value.run()
    assert 'finality' in p.outputs._files and os.fstat(p.outputs._files['finality'].fd).st_size > 0
    assert value._receipt is None and p.outputs._failed
    with pytest.raises(outputs.NativeOutputError): p.outputs.artifact('finality')
    assert value.cleanup(p.c.clock.end()) == ()
    assert os.fstat(value._client[3]).st_size > 0


@pytest.mark.parametrize('phase', ['before_child', 'after_native_files', 'after_pair_capture'])
def test_original_runtime_guard_is_required_across_every_publication_boundary(collection, phase):
    p = collection
    value = owner(p)
    captured = False
    original_complete = p.outputs.complete
    def complete(replies):
        nonlocal captured
        result = original_complete(replies); captured = True; return result
    p.outputs.complete = complete
    def guard():
        if (phase == 'before_child' or
            phase == 'after_native_files' and (p.outputs.directory / 'finality.nrt').exists() or
            phase == 'after_pair_capture' and captured):
            raise RuntimeError('PRIVATE original runtime lost')
        p.c.inputs.validate()
    value._guard = guard
    with pytest.raises(vector.VectorCollectionError): value.run()
    assert value._receipt is None and value._phase == 'failed'
    assert len(p.c.commands.calls) == (0 if phase == 'before_child' else 1)


@pytest.mark.parametrize('change', ['zero', 'bad_type', 'uppercase'])
def test_invocation_is_generated_internally_and_must_be_canonical(collection, monkeypatch, change):
    value = {'zero': '0' * 64, 'bad_type': None, 'uppercase': 'A' * 64}[change]
    monkeypatch.setattr(vector.secrets, 'token_hex', lambda _: value)
    p = collection; owned = owner(p)
    with pytest.raises(vector.VectorCollectionError): owned.run()
    assert not p.c.commands.calls and list(p.outputs.directory.iterdir()) == []
