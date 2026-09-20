"""Actual retained-file/pipe/row-join composition with mocked native children."""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest

from scaling_readiness_fixture import ready_setup
from scaling_canonical_proof_test import plan, projection, wire
from scaling_native_outputs_test import budget, complete, write
import scaling_command as command
import scaling_native_outputs as outputs
import scaling_proof_sequence as sequence


def stopped_reader(c):
    store = c.inputs.roles[3].block_store
    return sequence.StoppedReader(store, store / 'merge.log', 1, 100, 100,
        8 * 1024 * 1024, 65536, 65536, 100, 65536, 65536, os.geteuid())


@pytest.fixture
def pipeline(ready_setup, monkeypatch):
    c = ready_setup
    out = outputs.NativeOutputs(c.root / 'native-outputs', budget())
    complete(out, 'collection')
    complete(out, 'facts')
    run = plan()
    result = SimpleNamespace(c=c, outputs=out, plan=run, expected_plan=plan(),
        stopped=stopped_reader(c), reply_cap=128 * 1024, guard=c.inputs.validate,
        replies=[], raw_transform=lambda operation, raw: raw,
        reply_transform=lambda operation, reply: None,
        before_command=lambda operation, argv: None)

    def publish(role, argv, flag):
        path = Path(argv[argv.index(flag) + 1])
        # The fake child writes the real, exact native staging/final filenames.
        # Cryptographic generation is deliberately not simulated by these bytes.
        stage = path.with_name(path.name + '.publishing')
        identity = write(stage, ('native-test-' + role).encode())
        stage.rename(path)
        return identity

    def spawn(argv, **options):
        operation = argv[6]
        result.before_command(operation, argv)
        invocation = argv[argv.index('--invocation-id') + 1]
        reply = dict(version=1, operation=operation, invocation_id=invocation)
        if operation == 'prepare':
            facts = out.artifact('facts')
            reply.update(facts_sha256=facts.sha256, facts_bytes=facts.bytes)
            for role in ('request', 'bundle'):
                digest, size = publish(role, argv, '--' + role + '-output')
                reply.update({role + '_sha256': digest, role + '_bytes': size})
        elif operation == 'export':
            digest, size = publish('proof', argv, '--output')
            reply.update(request_sha256=out.artifact('request').sha256,
                input_sha256=out.artifact('bundle').sha256, proof_sha256=digest,
                proof_iroha_hash='7' * 64, proof_bytes=size)
        else:
            assert operation == 'replay'
            request, proof = out.artifact('request'), out.artifact('proof')
            binding = sequence.ReplayBindings(request.path, request.sha256, request.max_bytes,
                proof.path, proof.sha256, '7' * 64, proof.bytes, proof.max_bytes, result.reply_cap)
            reply, rows = projection(result.expected_plan, binding, invocation)
        result.reply_transform(operation, reply)
        result.replies.append(reply.copy())
        raw = wire(reply, rows) if operation == 'replay' else (
            json.dumps(reply, separators=(',', ':')).encode() + b'\n')
        c.commands.raws[len(c.commands.calls)] = result.raw_transform(operation, raw)
        return c.commands(argv, **options)

    monkeypatch.setattr(command.subprocess, 'Popen', spawn)
    yield result
    out.close()


def owner(p):
    return sequence.NativeProofSequence(p.outputs, p.stopped, p.c.images[1],
        p.c.commands, p.c.clock.end(60), p.reply_cap, p.guard)


def test_complete_native_sequence_binds_original_inputs_and_retains_every_output(pipeline):
    p = pipeline
    value = owner(p)
    receipt = value.run(p.plan)
    assert receipt.row_count == 8
    assert receipt.proof_sha256 == p.outputs.artifact('proof').sha256
    assert receipt.request_sha256 == p.outputs.artifact('request').sha256
    assert receipt.proof_iroha_hash == '7' * 64
    assert receipt.verifier_process.pid == 2002
    assert value._phase == 'verified'
    assert len(p.outputs.finish()) == 6
    calls = p.c.commands.calls
    assert [argv[6] for argv, _ in calls] == ['prepare', 'export', 'replay']
    invocations = []
    for argv, options in calls:
        assert argv[:6] == (str(p.c.images[1].path), '--ui-mode', 'plain', 'advanced', 'kura', 'scaling-evidence')
        assert options['env'] == {} and options['shell'] is False and options['pass_fds'] == ()
        invocations.append(argv[argv.index('--invocation-id') + 1])
    assert len(set(invocations)) == 3
    prepare = dict(zip(calls[0][0][7::2], calls[0][0][8::2], strict=True))
    assert prepare['--facts-sha256'] == p.outputs.artifact('facts').sha256
    assert prepare['--total-max-bytes'] == str(3 * 65536)
    export = dict(zip(calls[1][0][7::2], calls[1][0][8::2], strict=True))
    assert export['--block-store'] == str(p.stopped.block_store)
    assert export['--merge-log'] == str(p.stopped.merge_log)
    assert export['--first-height'] == '1' and export['--last-height'] == '100'
    assert export['--owner-uid'] == str(os.geteuid())
    assert export['--request-sha256'] == p.outputs.artifact('request').sha256
    assert export['--input-sha256'] == p.outputs.artifact('bundle').sha256
    assert len(value._commands._children) == 2 and len(value._replay._commands._children) == 1
    assert all(child.returncode == 0 and child.stdout.closed and child.stderr.closed for child in p.c.commands.children)
    for artifact in p.outputs.finish():
        assert hashlib.sha256(os.pread(p.outputs.descriptor(artifact.role), artifact.bytes, 0)).hexdigest() == artifact.sha256
    assert value.cleanup(p.c.clock.end()) == ()
    p.outputs.validate()
    with pytest.raises(sequence.ProofSequenceError): value.run(p.plan)


@pytest.mark.parametrize('kind', ['missing_observation', 'late_tip', 'small_projection',
    'invalid_phase_size', 'mismatched_application', 'duplicate_transaction', 'invalid_account'])
def test_entire_plan_is_admitted_before_any_native_command(pipeline, kind):
    p = pipeline
    if kind == 'missing_observation': p.plan = replace(p.plan, observations=p.plan.observations[:-1])
    elif kind == 'late_tip': p.plan = replace(p.plan, last_height=101)
    elif kind == 'small_projection': p.reply_cap = 4096
    elif kind == 'invalid_phase_size': p.plan = replace(p.plan, warmup_requests=3)
    elif kind == 'mismatched_application':
        p.plan = replace(p.plan, observations=(replace(p.plan.observations[0], local_height=99), *p.plan.observations[1:]))
    elif kind == 'duplicate_transaction':
        p.plan = replace(p.plan, observations=(p.plan.observations[1], *p.plan.observations[1:]))
    else: p.plan = replace(p.plan, accounts=('', *p.plan.accounts[1:]))
    value = owner(p)
    with pytest.raises(sequence.ProofSequenceError): value.run(p.plan)
    assert p.c.commands.calls == [] and value._phase == 'failed'
    assert sorted(path.name for path in p.outputs.directory.iterdir()) == ['facts.nrt', 'finality.nrt', 'queries.nrt']
    with pytest.raises(sequence.ProofSequenceError): value.run(plan())


@pytest.mark.parametrize('when', ['guard', 'prepare', 'export'])
def test_original_plan_is_owned_before_callbacks_and_native_operations(pipeline, when):
    p = pipeline
    def mutate():
        object.__setattr__(p.plan, 'seed', 'f' * 64)
        object.__setattr__(p.plan, 'accounts', ('changed',) * 4)
        object.__setattr__(p.plan.observations[0], 'transaction_hash', 'f' * 64)
        object.__setattr__(p.plan.observations[0], 'global_height', 99)
    if when == 'guard':
        def guard():
            mutate()
            p.c.inputs.validate()
        p.guard = guard
    else: p.before_command = lambda operation, _: mutate() if operation == when else None
    receipt = owner(p).run(p.plan)
    assert receipt.row_count == 8 and p.plan.seed == 'f' * 64


@pytest.mark.parametrize('field,bad', [('first_height', 2), ('last_height', 0),
    ('last_height', 1000001), ('max_committed_blocks', 99), ('max_merge_frames', 0),
    ('max_carrier_bytes', True), ('max_store_data_bytes', 1 << 64), ('owner_uid', -1),
    ('owner_uid', 1 << 32), ('merge_log', Path('relative'))])
def test_stopped_reader_requires_original_bounded_interval_before_commands(pipeline, field, bad):
    p = pipeline
    p.stopped = replace(p.stopped, **{field: bad})
    with pytest.raises(ValueError): owner(p)
    assert p.c.commands.calls == []


def test_stopped_reader_snapshot_does_not_follow_later_caller_changes(pipeline):
    p = pipeline
    value = owner(p)
    original = str(p.stopped.block_store)
    object.__setattr__(p.stopped, 'last_height', 999)
    object.__setattr__(p.stopped, 'block_store', Path('/another/store'))
    assert value.run(p.plan).row_count == 8
    argv = p.c.commands.calls[1][0]
    assert argv[argv.index('--last-height') + 1] == '100'
    assert argv[argv.index('--block-store') + 1] == original


@pytest.mark.parametrize('operation,field,bad', [
    ('prepare', 'version', True), ('prepare', 'operation', 'export'),
    ('prepare', 'invocation_id', '0' * 64), ('prepare', 'facts_sha256', '0' * 64),
    ('prepare', 'facts_bytes', True), ('prepare', 'facts_bytes', 1),
    ('prepare', 'request_sha256', '0' * 64), ('prepare', 'request_bytes', 1),
    ('prepare', 'request_bytes', 65537), ('prepare', 'bundle_sha256', '0' * 64),
    ('prepare', 'bundle_bytes', 0), ('prepare', 'extra', 1),
    ('export', 'version', 2), ('export', 'invocation_id', '0' * 64),
    ('export', 'request_sha256', '0' * 64), ('export', 'input_sha256', '0' * 64),
    ('export', 'proof_sha256', '0' * 64), ('export', 'proof_iroha_hash', '8' * 64),
    ('export', 'proof_bytes', True), ('export', 'proof_bytes', 1),
    ('export', 'proof_bytes', 65537), ('export', 'extra', 1),
    ('replay', 'request_sha256', '0' * 64), ('replay', 'proof_sha256', '0' * 64),
])
def test_wrong_native_reply_never_exposes_a_verified_prefix(pipeline, operation, field, bad):
    p = pipeline
    def change(stage, reply):
        if stage == operation: reply[field] = bad
    p.reply_transform = change
    value, published = owner(p), []
    with pytest.raises(sequence.ProofSequenceError): published.append(value.run(p.plan))
    assert published == [] and value._phase == 'failed'
    assert len(p.c.commands.calls) == ('prepare', 'export', 'replay').index(operation) + 1
    assert all(child.stdout.closed and child.stderr.closed for child in p.c.commands.children)
    assert value.cleanup(p.c.clock.end()) == ()


@pytest.mark.parametrize('operation', ['prepare', 'export', 'replay'])
@pytest.mark.parametrize('failure', ['truncated', 'extra_newline', 'nonzero', 'late', 'original_input', 'sealed_output'])
def test_transport_deadline_or_custody_failure_at_each_stage_is_terminal(pipeline, operation, failure):
    p = pipeline
    stage = ('prepare', 'export', 'replay').index(operation)
    if failure in ('truncated', 'extra_newline'):
        p.raw_transform = lambda op, raw: (raw[:-1] if failure == 'truncated' else raw + b'\n') if op == operation else raw
    value = owner(p)
    def setup(child):
        if child.index != stage: return
        if failure == 'nonzero': child.status = 7
        elif failure == 'late': child.wait_hook = lambda: setattr(p.c.clock, 'now', value._end)
        elif failure == 'original_input':
            child.wait_hook = lambda: p.c.inputs.roles[0].client_config.write_bytes(b'changed private input')
        elif failure == 'sealed_output':
            child.wait_hook = lambda: (p.outputs.directory / 'facts.nrt').write_bytes(b'changed facts')
    p.c.commands.after_spawn = setup
    published = []
    with pytest.raises(sequence.ProofSequenceError): published.append(value.run(p.plan))
    assert published == [] and value._phase == 'failed'
    assert len(p.c.commands.calls) == stage + 1
    assert all(child.stdout.closed and child.stderr.closed for child in p.c.commands.children)
    assert value.cleanup(p.c.clock.end()) == ()


def test_reentrant_run_cannot_be_swallowed_by_original_input_callback(pipeline):
    p = pipeline
    value = None
    def reenter():
        with pytest.raises(sequence.ProofSequenceError): value.run(p.plan)
    p.guard = reenter
    value = owner(p)
    with pytest.raises(sequence.ProofSequenceError): value.run(p.plan)
    assert p.c.commands.calls == [] and value._phase == 'failed'


def test_cleanup_retains_original_pending_child_and_never_adopts_another(pipeline):
    p = pipeline
    def setup(child): child.stall = True
    p.c.commands.after_spawn = setup
    value = owner(p)
    with pytest.raises(sequence.ProofSequenceError): value.run(p.plan)
    assert len(p.c.commands.children) == 1
    assert value.cleanup(p.c.clock.end()) == ('proof-prepare',)
    assert len(p.c.commands.children) == 1
    p.c.commands.children[0].stall = False
    assert value.cleanup(p.c.clock.end()) == ()
    p.outputs.validate()
