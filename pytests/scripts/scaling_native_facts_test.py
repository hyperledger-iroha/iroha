"""Real original-file/pipe custody with fake native children; no Norito or OS process claim."""
from dataclasses import fields, replace
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest
from scaling_readiness_fixture import ready_setup, write_inputs
from scaling_native_outputs_test import budget as output_budget, complete, write
import scaling_command as command
import scaling_native_outputs as outputs
import scaling_native_facts as native
import scaling_native_facts_inputs as inputs


def plan():
    return inputs.FactsJournalPlan('a' * 64, 1, 4, 1, 1_000_000_000, 20_000_000_000,
        1_000_000_000, 0, 2, 2, 1_000_000, 4, 8, 2, 1_000_000, 100,
        1_000_000_000, 100_000_000, 0)


def reader():
    return inputs.ReaderBudget(1000, 8 * 1024 * 1024, 65536, 1024 * 1024, 1000,
        1024 * 1024, 2 * 1024 * 1024, os.geteuid())


def budget():
    return inputs.FactsBudget(65536, 65536, 65536, 65536, 1024 * 1024, 65536,
        1024 * 1024 + 65536, 1024 * 1024, 2 * 1024 * 1024,
        1024 * 1024, 1024 * 1024, 1000, 128, 1024)


@pytest.fixture
def pipeline(ready_setup, monkeypatch):
    c = ready_setup
    out = outputs.NativeOutputs(c.root / 'native-outputs', output_budget())
    directory = c.root / 'journal'; directory.mkdir(mode=0o700)
    path = directory / 'collector.jsonl'
    raw = b'{"fake_journal_transport_only":true}\n'
    digest, size = write(path, raw)
    p = SimpleNamespace(c=c, outputs=out, journal=inputs.JournalInput(path, digest, size, 65536),
        budget=budget(), plan=plan(), reader=reader(), guard=c.inputs.validate,
        reply_transform=lambda operation, value: None,
        raw_transform=lambda operation, raw: raw,
        before_command=lambda operation, argv: None, height=100)
    def spawn(argv, **kwargs):
        operation = argv[6]; p.before_command(operation, argv)
        invocation = argv[argv.index('--invocation-id') + 1]
        if operation == 'stopped-tip':
            genesis = next(row for row in c.inputs.generation.artifacts if row.path == 'genesis.signed.nrt')
            value = dict(version=1, operation='stopped_tip', invocation_id=invocation,
                genesis_sha256=genesis.sha256, genesis_bytes=genesis.bytes, committed_height=p.height)
        else:
            assert operation == 'facts'
            destination = Path(argv[argv.index('--facts-output') + 1])
            stage = destination.with_name(destination.name + '.publishing')
            sha, count = write(stage, b'fake-native-facts-no-crypto-claim')
            stage.rename(destination)
            value = dict(version=1, operation='facts', invocation_id=invocation,
                facts_sha256=sha, facts_bytes=count)
        p.reply_transform(operation, value)
        raw = json.dumps(value, separators=(',', ':')).encode() + b'\n'
        c.commands.raws[len(c.commands.calls)] = p.raw_transform(operation, raw)
        return c.commands(argv, **kwargs)
    monkeypatch.setattr(command.subprocess, 'Popen', spawn)
    yield p
    out.close()


def owner(p):
    value = native.NativeFacts(p.c.inputs, p.outputs, p.journal, p.reader, p.plan, p.budget,
        p.c.images[1], p.c.commands, p.c.clock.end(60), p.guard)
    return value


def facts(p, value):
    tip = value.observe_tip()
    complete(p.outputs, 'collection')
    return tip, value.produce_facts()


def flags(argv):
    result, last = {}, None
    for value in argv[7:]:
        if value.startswith('--'):
            assert value not in result
            result[value] = []; last = value
        else:
            assert last is not None
            result[last].append(value)
    return result


def test_output_namespace_inside_original_peer_storage_is_rejected_before_tip(pipeline):
    p = pipeline
    nested = outputs.NativeOutputs(p.c.inputs.roles[3].block_store / 'native-outputs', output_budget())
    try:
        p.c.inputs.validate()
        with pytest.raises(native.NativeFactsError, match='native_facts_output_namespace'):
            native.NativeFacts(p.c.inputs, nested, p.journal, p.reader, p.plan, p.budget,
                p.c.images[1], p.c.commands, p.c.clock.end(60), p.guard)
        assert p.c.commands.calls == [] and list(nested.directory.iterdir()) == []
    finally:
        nested.close()


def test_generated_input_root_below_output_namespace_is_rejected_before_tip(pipeline):
    p = pipeline
    directory = p.outputs.directory / 'generated-inputs'
    write_inputs(directory)
    anchor = hashlib.sha256((directory / 'genesis-anchors.json').read_bytes()).hexdigest()
    with native.ReadinessInputs(directory, anchor, 4) as original:
        with pytest.raises(native.NativeFactsError, match='native_facts_output_namespace'):
            native.NativeFacts(original, p.outputs, p.journal, p.reader, p.plan, p.budget,
                p.c.images[1], p.c.commands, p.c.clock.end(60), original.validate)
        assert p.c.commands.calls == []
        assert set(path.name for path in p.outputs.directory.iterdir()) == {'generated-inputs'}


def test_fixed_native_tip_and_facts_join_original_inputs_vectors_and_deadline(pipeline):
    p = pipeline
    value = owner(p)
    try:
        tip, receipt = facts(p, value)
        assert tip.reader.block_store == p.c.inputs.roles[3].primary_block_store
        assert tip.reader.merge_log == p.c.inputs.roles[3].primary_merge_log
        assert tip.reader.block_store != p.c.inputs.roles[3].block_store
        assert tip.reader.first_height == 1 and tip.reader.last_height == 100
        assert receipt.facts == p.outputs.artifact('facts') and receipt.journal_sha256 == p.journal.sha256
        assert receipt.original_anchor_sha256 == p.c.inputs.anchors_sha256
        assert receipt.finality_sha256 == p.outputs.artifact('finality').sha256
        assert receipt.queries_sha256 == p.outputs.artifact('queries').sha256
        assert value._phase == 'complete'
        assert len(p.c.commands.calls) == 2
        first, second = (flags(argv) for argv, _ in p.c.commands.calls)
        assert first['--first-height'] == first['--last-height'] == ['1']
        assert second['--last-height'] == ['100'] and second['--first-height'] == ['1']
        assert second['--genesis-public-key'] == [p.c.inputs.generation.genesis_public_key]
        assert second['--chain-discriminant'] == [str(p.c.inputs.generation.chain_discriminant)]
        assert second['--network-id'] == [p.c.inputs.network_id]
        assert second['--peer-config'] == [str(row.node_config) for row in p.c.inputs.roles]
        assert second['--validator'] == [row.node_public_key for row in p.c.inputs.roles]
        assert second['--account'] == [row.account_id for row in p.c.inputs.generation.accounts]
        assert set(second) == set(NATIVE_FACTS_FLAGS)
        for argv, options in p.c.commands.calls:
            assert argv[:6] == (str(p.c.images[1].path), '--ui-mode', 'plain', 'advanced', 'kura', 'scaling-evidence')
            assert options['env'] == {} and options['shell'] is False
            assert options['pass_fds'] == (value._journal.fd,)
            assert len(argv) <= command.MAX_ARGV_ITEMS
        assert first['--invocation-id'] != second['--invocation-id']
        assert value._commands.deadline_ns == value._end
        assert len(value._commands._children) == 2
        assert all(child.returncode == 0 and child.stdout.closed and child.stderr.closed for child in p.c.commands.children)
        value.validate()
        assert value.cleanup(p.c.clock.end()) == ()
        p.outputs.validate()
    finally: value.close()


@pytest.mark.parametrize('boundary', ['before_tip', 'before_facts'])
def test_duplicate_account_projection_cannot_reach_native_facts(pipeline, boundary):
    """A changed account cohort cannot replace the original native command input."""
    p = pipeline
    value = owner(p)
    try:
        if boundary == 'before_facts':
            value.observe_tip()
            complete(p.outputs, 'collection')
        original = p.c.inputs.generation
        duplicate = replace(original.accounts[1], account_id=original.accounts[0].account_id)
        p.c.inputs._generation = replace(
            original, accounts=(original.accounts[0], duplicate, *original.accounts[2:]),
        )
        with pytest.raises(native.NativeFactsError, match='native_facts_original_changed'):
            if boundary == 'before_tip':
                value.observe_tip()
            else:
                value.produce_facts()
        assert len(p.c.commands.calls) == (0 if boundary == 'before_tip' else 1)
        assert value._phase == 'failed'
        assert value.cleanup(p.c.clock.end()) == ()
    finally:
        value.close()


@pytest.mark.parametrize('field', [field.name for field in fields(inputs.FactsJournalPlan) if field.name != 'workload_seed'])
def test_journal_numeric_fields_reject_boolean_before_any_child(pipeline, field):
    p = pipeline; p.plan = replace(p.plan, **{field: True})
    with pytest.raises(native.NativeFactsError): owner(p)
    assert p.c.commands.calls == [] and list(p.outputs.directory.iterdir()) == []


@pytest.mark.parametrize('field,bad', [('workload_seed', 'secret'), ('pair_index', 6),
    ('rate_numerator', 0), ('rate_denominator', 1 << 128), ('warmup_ns', -1),
    ('measurement_ns', 1), ('drain_ns', 0), ('submission_lag_bound_ns', 100_000_000),
    ('preparation_lookahead', 4097), ('preparation_concurrency', 33),
    ('preparation_ahead_ns', 1), ('max_submissions', 4097), ('max_in_flight', 16385),
    ('max_status_requests', 257), ('poll_interval_ns', 1), ('journal_max_requests', 80),
    ('resource_interval_ns', 2_000_000_000), ('resource_response_deadline_ns', 600_000_000),
    ('resource_max_start_lag_ns', 300_000_000)])
def test_invalid_independent_schedule_or_sampling_geometry_is_admitted_early(pipeline, field, bad):
    p = pipeline; p.plan = replace(p.plan, **{field: bad})
    with pytest.raises(native.NativeFactsError): owner(p)
    assert p.c.commands.calls == []


@pytest.mark.parametrize('kind,field,bad', [
    ('reader', 'max_committed_blocks', 0), ('reader', 'max_store_data_bytes', 2 * 1024 ** 3 + 1),
    ('reader', 'max_carrier_bytes', 32 * 1024 ** 2 + 1), ('reader', 'max_merge_log_bytes', 0),
    ('reader', 'max_merge_frames', 1001), ('reader', 'reader_max_output_bytes', 0),
    ('reader', 'max_decode_allocation_bytes', 512 * 1024 ** 2 + 1), ('reader', 'owner_uid', -1),
    ('budget', 'source_max_bytes', 1), ('budget', 'facts_max_bytes', 65535),
    ('budget', 'total_max_bytes', 1), ('budget', 'proof_max_bytes', 1),
    ('budget', 'max_heights', 65537), ('budget', 'max_requests', 99),
    ('budget', 'context_max_bytes', 8 * 1024 ** 2 + 1), ('budget', 'peer_config_max_bytes', 1),
    ('journal', 'bytes', 0), ('journal', 'sha256', 'private'), ('journal', 'max_bytes', 1),
    ('journal', 'path', Path('relative'))])
def test_allocation_and_reader_failures_happen_before_commands(pipeline, kind, field, bad):
    p = pipeline; setattr(p, kind, replace(getattr(p, kind), **{field: bad}))
    with pytest.raises(native.NativeFactsError): owner(p)
    assert p.c.commands.calls == []


@pytest.mark.parametrize('operation,field,bad', [
    ('stopped-tip', 'version', True), ('stopped-tip', 'operation', 'stopped-tip'),
    ('stopped-tip', 'invocation_id', '0' * 64), ('stopped-tip', 'genesis_sha256', '0' * 64),
    ('stopped-tip', 'genesis_bytes', True), ('stopped-tip', 'genesis_bytes', 1),
    ('stopped-tip', 'committed_height', True), ('stopped-tip', 'committed_height', 0),
    ('stopped-tip', 'committed_height', 1001), ('stopped-tip', 'extra', 1),
    ('facts', 'version', 2), ('facts', 'operation', 'export'), ('facts', 'invocation_id', '0' * 64),
    ('facts', 'facts_sha256', '0' * 64), ('facts', 'facts_bytes', True),
    ('facts', 'facts_bytes', 1), ('facts', 'facts_bytes', 65537), ('facts', 'extra', 1)])
def test_native_reply_identity_or_shape_failure_never_publishes_receipt(pipeline, operation, field, bad):
    p = pipeline
    p.reply_transform = lambda stage, reply: reply.update({field: bad}) if stage == operation else None
    value = owner(p); published = []
    try:
        with pytest.raises(native.NativeFactsError): published.append(facts(p, value))
        assert published == [] and value._phase == 'failed'
        assert len(p.c.commands.calls) == (1 if operation == 'stopped-tip' else 2)
        assert value.cleanup(p.c.clock.end()) == ()
    finally: value.close()


@pytest.mark.parametrize('operation', ['stopped-tip', 'facts'])
@pytest.mark.parametrize('failure', ['truncated', 'extra_newline', 'nonzero', 'late', 'journal', 'input', 'vectors'])
def test_every_transport_deadline_and_original_custody_failure_is_terminal(pipeline, operation, failure):
    p = pipeline; value = owner(p)
    stage = 0 if operation == 'stopped-tip' else 1
    p.raw_transform = lambda op, raw: ((raw[:-1] if failure == 'truncated' else raw + b'\n')
        if op == operation and failure in ('truncated', 'extra_newline') else raw)
    def setup(child):
        if child.index != stage: return
        if failure == 'nonzero': child.status = 7
        elif failure == 'late': child.wait_hook = lambda: setattr(p.c.clock, 'now', value._end)
        elif failure == 'journal': child.wait_hook = lambda: p.journal.path.write_bytes(b'changed journal')
        elif failure == 'input': child.wait_hook = lambda: p.c.inputs.roles[0].node_config.write_bytes(b'changed private')
        elif failure == 'vectors':
            if operation == 'facts':
                path = p.outputs.artifact('finality').path
                child.wait_hook = lambda: path.write_bytes(b'changed')
            else: child.wait_hook = lambda: (p.outputs.directory / 'unallocated').write_bytes(b'changed')
    p.c.commands.after_spawn = setup
    try:
        with pytest.raises(native.NativeFactsError): facts(p, value)
        assert value._phase == 'failed' and len(p.c.commands.calls) == stage + 1
        assert all(child.stdout.closed and child.stderr.closed for child in p.c.commands.children)
        assert value.cleanup(p.c.clock.end()) == ()
    finally: value.close()


def test_phase_order_and_returned_reader_mutation_do_not_refresh_original_height(pipeline):
    p = pipeline; value = owner(p)
    try:
        tip = value.observe_tip()
        object.__setattr__(tip.reader, 'last_height', 999)
        object.__setattr__(tip.reader, 'block_store', Path('/foreign'))
        complete(p.outputs, 'collection')
        receipt = value.produce_facts()
        assert receipt.stopped_height == 100
        call = flags(p.c.commands.calls[1][0])
        assert call['--last-height'] == ['100']
        assert call['--block-store'] == [str(p.c.inputs.roles[3].primary_block_store)]
        with pytest.raises(native.NativeFactsError): value.produce_facts()
        assert len(p.c.commands.calls) == 2
    finally: value.close()


@pytest.mark.parametrize('phase', ['before_tip', 'repeat_tip', 'without_vectors'])
def test_missing_phase_never_runs_facts(pipeline, phase):
    p = pipeline; value = owner(p)
    try:
        if phase != 'before_tip': value.observe_tip()
        with pytest.raises(native.NativeFactsError):
            if phase == 'repeat_tip': value.observe_tip()
            else: value.produce_facts()
        assert len(p.c.commands.calls) == (0 if phase == 'before_tip' else 1)
    finally: value.close()


def test_selected_plan_budgets_and_journal_binding_are_owned_before_callbacks(pipeline):
    p = pipeline; value = owner(p)
    try:
        object.__setattr__(p.plan, 'rate_numerator', 999)
        object.__setattr__(p.budget, 'source_max_bytes', 1)
        object.__setattr__(p.reader, 'max_committed_blocks', 1)
        object.__setattr__(p.journal, 'sha256', 'f' * 64)
        _, receipt = facts(p, value)
        call = flags(p.c.commands.calls[1][0])
        assert call['--rate-numerator'] == ['4'] and call['--max-committed-blocks'] == ['1000']
        assert call['--source-max-bytes'] == [str(1024 * 1024)]
        assert receipt.journal_sha256 != p.journal.sha256
    finally: value.close()


@pytest.mark.parametrize('change', ['symlink', 'hardlink', 'mode', 'replace', 'ancestor'])
def test_journal_descriptor_and_complete_named_ancestry_are_retained(pipeline, change):
    p = pipeline; value = owner(p); path = p.journal.path
    try:
        if change == 'symlink':
            other = path.with_name('other'); path.rename(other); path.symlink_to(other)
        elif change == 'hardlink': os.link(path, path.with_name('other'))
        elif change == 'mode': path.chmod(0o644)
        elif change == 'replace':
            raw = path.read_bytes(); path.unlink(); path.write_bytes(raw); path.chmod(0o600)
        else:
            moved = path.parent.with_name('held-journal'); path.parent.rename(moved); path.parent.mkdir(mode=0o700)
        with pytest.raises(native.NativeFactsError): value.observe_tip()
        assert p.c.commands.calls == []
    finally: value.close()


def test_closed_journal_owner_is_not_readopted(pipeline):
    value = owner(pipeline); fd = value._journal.fd
    value.close(); value.close()
    with pytest.raises(OSError): os.fstat(fd)
    with pytest.raises(native.NativeFactsError): value.validate()
    assert pipeline.c.commands.calls == []


def test_deadline_is_never_extended_after_admission(pipeline):
    p = pipeline; value = owner(p)
    try:
        value._end += 1
        with pytest.raises(native.NativeFactsError): value.observe_tip()
        assert p.c.commands.calls == []
    finally: value.close()


def test_close_keeps_original_journal_until_owned_child_is_reaped(pipeline):
    p = pipeline; value = owner(p)
    p.c.commands.after_spawn = lambda child: setattr(child, 'stall', True)
    fd = value._journal.fd
    try:
        with pytest.raises(native.NativeFactsError): value.observe_tip()
        with pytest.raises(native.NativeFactsError): value.close()
        assert os.fstat(fd).st_size == p.journal.bytes
        assert value.cleanup(p.c.clock.end()) == ('native-stopped-tip',)
        p.c.commands.children[0].stall = False
        assert value.cleanup(p.c.clock.end()) == ()
        value.close()
        with pytest.raises(OSError): os.fstat(fd)
    finally:
        p.c.commands.children[0].stall = False
        value.cleanup(p.c.clock.end())
        value.close()

# Exact current native Args + flattened ReaderArgs flag contract, independently captured.
NATIVE_FACTS_FLAGS = ('--account', '--assembly-decode-max-bytes', '--block-store', '--chain-discriminant', '--chain-id', '--context', '--context-max-bytes', '--context-sha256', '--drain-ns', '--facts-max-bytes', '--facts-output', '--finality', '--finality-max-bytes', '--finality-sha256', '--first-height', '--genesis-public-key', '--invocation-id', '--journal', '--journal-max-bytes', '--journal-max-requests', '--journal-sha256', '--lanes', '--last-height', '--manifest', '--manifest-max-bytes', '--manifest-sha256', '--max-carrier-bytes', '--max-committed-blocks', '--max-decode-allocation-bytes', '--max-heights', '--max-in-flight', '--max-leaves-per-carrier', '--max-merge-frames', '--max-merge-log-bytes', '--max-requests', '--max-status-requests', '--max-store-data-bytes', '--max-submissions', '--measurement-ns', '--merge-log', '--network-id', '--owner-uid', '--pair-index', '--peer-config', '--peer-config-max-bytes', '--peer-config-sha256', '--poll-interval-ns', '--preparation-ahead-ns', '--preparation-concurrency', '--preparation-lookahead', '--proof-max-bytes', '--queries', '--queries-max-bytes', '--queries-sha256', '--rate-denominator', '--rate-numerator', '--reader-max-output-bytes', '--reply-max-bytes', '--resource-interval-ns', '--resource-max-start-lag-ns', '--resource-response-deadline-ns', '--signed-genesis', '--signed-genesis-max-bytes', '--signed-genesis-sha256', '--source-max-bytes', '--submission-lag-bound-ns', '--total-max-bytes', '--validator', '--verification-input-max-bytes', '--verification-output-max-bytes', '--warmup-ns', '--workload-seed')


@pytest.mark.parametrize('wrong_vector_height', [False, True])
def test_actual_typed_tip_vector_and_facts_owners_share_original_store_and_clock(pipeline, monkeypatch, wrong_vector_height):
    import scaling_vector_collection as vectors
    p = pipeline; value = owner(p)
    original_spawn = command.subprocess.Popen
    def spawn(argv, **options):
        if 'collect-scaling-inputs' not in argv:
            return original_spawn(argv, **options)
        invocation = argv[argv.index('--invocation-id') + 1]
        context = next(row for row in p.c.inputs.generation.artifacts if row.path == 'genesis-context.nrt')
        reply = dict(version=1, operation='collect_scaling_inputs', invocation_id=invocation,
            client_config_sha256=p.c.inputs.roles[0].client_config_sha256,
            committed_height=101 if wrong_vector_height else 100, finality_count=100, query_count=84,
            context_sha256=context.sha256, context_bytes=context.bytes)
        for role in ('finality', 'queries'):
            path = Path(argv[argv.index('--' + role + '-out') + 1])
            stage = path.with_name(path.name + '.collecting')
            sha, size = write(stage, ('fake-vector-' + role).encode()); stage.rename(path)
            reply.update({role + '_sha256': sha, role + '_bytes': size})
        p.c.commands.raws[len(p.c.commands.calls)] = json.dumps(reply, separators=(',', ':')).encode() + b'\n'
        return p.c.commands(argv, **options)
    monkeypatch.setattr(command.subprocess, 'Popen', spawn)
    collection = None
    try:
        tip = value.observe_tip()
        collection = vectors.NativeVectorCollection(p.c.inputs, p.outputs, tip.reader,
            vectors.CollectionLimits(65536, 65536, 4 * 65536, 1024, 128, 128, 2 * 1024 * 1024),
            p.c.images[1], p.c.commands, value._end, value.validate)
        if wrong_vector_height:
            with pytest.raises(vectors.VectorCollectionError): collection.run()
            assert len(p.c.commands.calls) == 2 and not (p.outputs.directory / 'facts.nrt').exists()
        else:
            receipt = collection.run()
            result = value.produce_facts()
            assert result.finality_sha256 == receipt.finality.sha256
            assert result.queries_sha256 == receipt.queries.sha256
            assert result.stopped_height == receipt.stopped_height == tip.reader.last_height
            assert value._commands.deadline_ns == collection._commands.deadline_ns
            assert len(p.c.commands.calls) == 3
            for argv, _ in p.c.commands.calls:
                assert argv[argv.index('--block-store') + 1] == str(p.c.inputs.roles[3].primary_block_store)
                assert argv[argv.index('--merge-log') + 1] == str(p.c.inputs.roles[3].primary_merge_log)
            value.validate(); collection.validate()
    finally:
        if collection is not None: assert collection.cleanup(p.c.clock.end()) == ()
        assert value.cleanup(p.c.clock.end()) == ()
        value.close()
