"""Canonical atomic owner journal custody and interrupted-write regressions.

These isolated native fixtures cover partial writes, durable witness reuse,
lost publication acknowledgments and foreign inode/content substitution.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys

import pytest

from taira_native_nginx_apply_test import (
    MODULE,
    _identity,
    _interrupted_journal,
    _interrupted_local_plan,
    _journal,
    native_apply,
)


def _journal_file_for_fd(root, fd):
    """Classify only fixture public journal paths by native device/inode."""
    observed = os.fstat(fd)
    for entry in root.iterdir():
        if not (entry.name.endswith('.receipt.ndjson') or entry.name.startswith('.taira-nginx-journal-')):
            continue
        metadata = entry.stat(follow_symlinks=False)
        if (metadata.st_dev, metadata.st_ino) == (observed.st_dev, observed.st_ino):
            return entry
    return None


def test_native_short_writes_and_interruptions_never_commit_incomplete_owner_or_candidate(native_apply, monkeypatch):
    request, root, candidate = native_apply
    write = MODULE.os.write
    calls = 0
    def interrupted_short_write(fd, body):
        nonlocal calls
        observed = os.fstat(fd)
        if stat.S_ISREG(observed.st_mode):
            calls += 1
            if calls % 5 == 1:
                raise InterruptedError('owned native write interruption')
            return write(fd, body[:7])
        return write(fd, body)
    monkeypatch.setattr(MODULE.os, 'write', interrupted_short_write)
    result = MODULE.remote_apply(request)
    assert result['exit_code'] == 0 and result['phase'] == 'awaiting_readiness', result
    assert calls > 100
    assert (root/'conf.d/new-scoped.conf').read_bytes() == candidate
    assert _journal(root)[-1]['phase'] == 'awaiting_readiness'
    assert result['qualified'] is False


@pytest.mark.parametrize('canonical_exists', [False, True])
def test_process_loss_during_staged_journal_write_leaves_canonical_complete_or_absent(native_apply, monkeypatch, canonical_exists):
    request, root, _ = native_apply
    write = MODULE.os.write
    canonical = root/('.taira-native-nginx-apply-'+request['operation_id']+'.receipt.ndjson')
    previous = None
    interrupted = False
    def killed_writer(fd, body):
        nonlocal previous, interrupted
        public = _journal_file_for_fd(root, fd)
        if public is not None and canonical.exists() == canonical_exists and not interrupted:
            previous = canonical.read_bytes() if canonical.exists() else None
            write(fd, body[:11])
            interrupted = True
            raise KeyboardInterrupt('owned process loss after partial stage write')
        return write(fd, body)
    monkeypatch.setattr(MODULE.os, 'write', killed_writer)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    assert interrupted
    if previous is None:
        assert not canonical.exists(), 'an empty/partial canonical owner journal became visible'
    else:
        assert canonical.read_bytes() == previous
        assert previous.endswith(b'\n')
        assert [json.loads(line)['sequence'] for line in previous.splitlines()] == list(range(1,len(previous.splitlines())+1))
    assert not (root/'conf.d/new-scoped.conf').exists()
    assert not (root/'reload-count').exists()


@pytest.mark.parametrize('restamped', [False, True])
def test_foreign_journal_after_exchange_is_not_adopted_on_retry(native_apply, monkeypatch, restamped):
    request, root, _ = native_apply
    plan = _interrupted_local_plan(request, root)
    exchange = MODULE.native_exchange
    canonical = root/('.taira-native-nginx-apply-'+request['operation_id']+'.receipt.ndjson')
    substituted = False
    unlink = MODULE.os.unlink
    def killed_owner_retains_candidate(name, *args, **kwargs):
        if substituted and str(name).startswith('.taira-nginx-publish-'):
            raise OSError('killed owner retains the recorded candidate stage')
        return unlink(name, *args, **kwargs)
    monkeypatch.setattr(MODULE.os, 'unlink', killed_owner_retains_candidate)
    def foreign_journal(directory, first, second):
        nonlocal substituted
        exchange(directory, first, second)
        if canonical.name not in {first, second} or substituted:
            return
        substituted = True
        content = canonical.read_bytes()
        foreign = root/'foreign-same-content-journal'
        foreign.write_bytes(content)
        foreign.chmod(0o600)
        if restamped:
            lines = content.splitlines(keepends=True)
            row = json.loads(lines[-1])
            observed = _identity(foreign)
            row['journal_publication_identity'] = {key:observed[key] for key in ('device','inode','uid','gid','mode')}
            foreign.write_bytes(b''.join(lines[:-1])+(json.dumps(row,sort_keys=True)+'\n').encode())
        os.replace(foreign, canonical)
        raise KeyboardInterrupt('lost acknowledgment after same-content foreign replacement')
    monkeypatch.setattr(MODULE, 'native_exchange', foreign_journal)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    assert substituted
    before = canonical.read_bytes(), _identity(canonical)
    monkeypatch.setattr(MODULE, 'native_exchange', exchange)
    monkeypatch.setattr(MODULE.os, 'unlink', unlink)
    assert list((root/'conf.d').glob('.taira-nginx-publish-*')), 'fixture must preserve the otherwise admissible stage'
    result = MODULE.reconcile_interrupted_publication(plan, _interrupted_journal(root,request['operation_id']), 'resume')
    assert result['exit_code'] == 1 and result['recovery_pending'] is True, result
    assert (canonical.read_bytes(), _identity(canonical)) == before
    assert not (root/'conf.d/new-scoped.conf').exists()
    assert not (root/'reload-count').exists()
    # A restamped semantic copy still contradicts the independently retained
    # immutable intent's actual created stage inode.
    assert hashlib.sha256(canonical.read_bytes()).hexdigest() == hashlib.sha256(before[0]).hexdigest()


def test_lost_atomic_journal_ack_resumes_only_durably_bound_created_inode(native_apply, monkeypatch):
    request, root, candidate = native_apply
    plan = _interrupted_local_plan(request, root)
    exchange, unlink = MODULE.native_exchange, MODULE.os.unlink
    canonical = root/('.taira-native-nginx-apply-'+request['operation_id']+'.receipt.ndjson')
    interrupted = False
    created = None
    def lose_ack(directory, first, second):
        nonlocal interrupted, created
        exchange(directory, first, second)
        if canonical.name in {first,second} and not interrupted:
            interrupted = True
            created = _identity(canonical)
            raise KeyboardInterrupt('owned loss after atomic canonical journal exchange')
    def retain_candidate(name, *args, **kwargs):
        if interrupted and str(name).startswith('.taira-nginx-publish-'):
            raise OSError('killed owner retains its recorded candidate')
        return unlink(name, *args, **kwargs)
    monkeypatch.setattr(MODULE, 'native_exchange', lose_ack)
    monkeypatch.setattr(MODULE.os, 'unlink', retain_candidate)
    with pytest.raises(KeyboardInterrupt):
        MODULE.remote_apply(request)
    assert interrupted and created == _identity(canonical)
    assert list((root/'conf.d').glob('.taira-nginx-publish-*'))
    monkeypatch.setattr(MODULE, 'native_exchange', exchange)
    monkeypatch.setattr(MODULE.os, 'unlink', unlink)
    result = MODULE.reconcile_interrupted_publication(plan, _interrupted_journal(root,request['operation_id']), 'resume')
    assert result['exit_code'] == 0 and result['phase'] == 'awaiting_readiness', result
    assert (root/'conf.d/new-scoped.conf').read_bytes() == candidate
    assert _journal(root)[-1]['phase'] == 'awaiting_readiness'


@pytest.mark.parametrize('corrupt_intent', [False, True])
def test_full_created_stage_and_independent_witness_reach_guard_before_canonical_visibility(native_apply, monkeypatch, corrupt_intent):
    request, root, _ = native_apply
    writer = MODULE.append_owned_publication_record
    canonical = root/('.taira-native-nginx-apply-'+request['operation_id']+'.receipt.ndjson')
    witnessed = []
    def guarded_writer(context, request, reference, opened, rows, record, guard, prepared_write):
        def checkpoint(witness, stage, projection):
            guard(witness, stage, projection)
            intent_path = Path(witness['path'])
            assert _identity(intent_path) == witness['identity']
            intent_body = intent_path.read_bytes()
            assert hashlib.sha256(intent_body).hexdigest() == witness['sha256']
            intent = json.loads(intent_body)
            stage_path = root/intent['staging_basename']
            assert _identity(stage_path) == stage
            assert intent['created_identity'] == {key:stage[key] for key in ('device','inode','uid','gid','mode')}
            assert intent['record_sha256'] == hashlib.sha256(json.dumps(projection,sort_keys=True).encode()).hexdigest()
            assert canonical.read_bytes() == os.pread(opened,1_048_577,0) if reference else not canonical.exists()
            witnessed.append(witness)
            if corrupt_intent:
                intent_path.write_bytes(b'foreign intent mutation\n')
            else:
                raise KeyboardInterrupt('durable external witness admission before canonical effect')
        return writer(context, request, reference, opened, rows, record, checkpoint, prepared_write)
    monkeypatch.setattr(MODULE,'append_owned_publication_record',guarded_writer)
    if corrupt_intent:
        result = MODULE.remote_apply(request)
        assert result['exit_code'] == 1 and result['error_code'] == 'native_identity_changed', result
    else:
        with pytest.raises(KeyboardInterrupt): MODULE.remote_apply(request)
    assert len(witnessed) == 1
    assert not canonical.exists() and not (root/'conf.d/new-scoped.conf').exists()
    assert not (root/'reload-count').exists()


@pytest.mark.parametrize('fault', [None, 'foreign_stage', 'changed_stage', 'missing_stage', 'foreign_intent'])
def test_recorded_complete_prepublication_write_is_reused_or_preserved_on_substitution(native_apply, monkeypatch, fault):
    request, root, candidate = native_apply
    writer = MODULE.append_owned_publication_record
    saved = None
    interrupted = False
    def interrupted_writer(context, request, reference, opened, rows, record, guard, prepared_write):
        nonlocal saved, interrupted
        if saved is not None and MODULE.public_journal_record_projection(record) == saved['record']:
            return writer(context,request,reference,opened,rows,record,guard,saved)
        def checkpoint(intent, stage, projection):
            nonlocal saved, interrupted
            guard(intent,stage,projection)
            saved = dict(intent=intent,stage=stage,record=projection)
            interrupted = True
            raise KeyboardInterrupt('durable completed write intent before canonical publication')
        return writer(context,request,reference,opened,rows,record,checkpoint,prepared_write)
    monkeypatch.setattr(MODULE,'append_owned_publication_record',interrupted_writer)
    with pytest.raises(KeyboardInterrupt): MODULE.remote_apply(request)
    assert interrupted and saved is not None
    witness = Path(saved['intent']['path'])
    stage = root/json.loads(witness.read_bytes())['staging_basename']
    if fault in {'foreign_stage','foreign_intent'}:
        selected = stage if fault == 'foreign_stage' else witness
        foreign = root/'foreign-native-stage'
        foreign.write_bytes(selected.read_bytes()); foreign.chmod(0o600)
        os.replace(foreign,selected)
    elif fault == 'changed_stage':
        stage.write_bytes(b'changed complete stage\n')
    elif fault == 'missing_stage':
        stage.unlink()
    preserved = {path:(path.read_bytes(),_identity(path)) for path in (stage,witness) if path.exists()}
    # Later phases use fresh canonical atomic writes; only this exact durable
    # predecessor checkpoint can reuse the recorded first stage.
    def resumed_writer(context, request, reference, opened, rows, record, guard, prepared_write):
        retained = saved if MODULE.public_journal_record_projection(record) == saved['record'] else prepared_write
        return writer(context,request,reference,opened,rows,record,guard,retained)
    monkeypatch.setattr(MODULE,'append_owned_publication_record',resumed_writer)
    result = MODULE.remote_apply(request)
    if fault is None:
        assert result['exit_code'] == 0 and result['phase'] == 'awaiting_readiness', result
        assert (root/'conf.d/new-scoped.conf').read_bytes() == candidate
        assert _journal(root)[0]['journal_write_intent'] == saved['intent']
        assert _journal(root)[0]['journal_publication_identity'] == {
            key:saved['stage'][key] for key in ('device','inode','uid','gid','mode')}
    else:
        assert result['exit_code'] == 1, result
        assert not (root/'conf.d/new-scoped.conf').exists() and not (root/'reload-count').exists()
        assert all((path.read_bytes(),_identity(path)) == previous for path,previous in preserved.items())


@pytest.mark.parametrize('malformed', [None, [], ['untrusted host response']])
def test_directional_receipt_rejects_non_object_wire_with_closed_error(native_apply, malformed):
    request, root, _ = native_apply
    plan = _interrupted_local_plan(request,root)
    with pytest.raises(MODULE.checked.CheckError, match='apply_receipt_fields'):
        MODULE.admit_receipt(malformed,1,plan,recovery_direction='rollback')


def test_native_apply_and_exact_rollback_fit_an_ordinary_256_descriptor_process():
    directory = Path(__file__).resolve().parent
    # Lower only this owned subprocess; never alter pytest/controller limits.
    program = """import os, resource, sys
resource.setrlimit(resource.RLIMIT_NOFILE, (256, resource.getrlimit(resource.RLIMIT_NOFILE)[1]))
sys.path.insert(0, sys.argv[1])
os.environ['PYTEST_DISABLE_PLUGIN_AUTOLOAD'] = '1'
import pytest
raise SystemExit(pytest.main(['-q', '--tb=short',
    sys.argv[1]+'/taira_native_nginx_check_test.py::test_repeated_native_custody_guards_release_only_their_path_check_duplicates',
    sys.argv[1]+'/taira_native_nginx_apply_test.py::test_publication_is_complete_included_once_durable_and_unqualified',
    sys.argv[1]+'/taira_native_nginx_apply_test.py::test_failed_context_source_proof_or_reload_rolls_back_only_new_file[reject-context]',
    sys.argv[1]+'/taira_native_nginx_apply_test.py::test_direct_partial_rollback_restores_without_candidate_context_or_hup[True-replace]']))
"""
    result = subprocess.run([sys.executable,'-I','-c',program,str(directory)],
        stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=90)
    assert result.returncode == 0, result.stdout.decode()[:8192]
    assert b'4 passed' in result.stdout
