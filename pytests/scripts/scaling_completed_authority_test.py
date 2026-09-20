"""Actual completed trial/public owners with fake native bytes and real files."""
from dataclasses import replace
import hashlib
import os
from pathlib import Path
from types import SimpleNamespace

import pytest
from scaling_fixed_trial_fixture import trial_setup, ready_setup
import scaling_completed_authority as authority
from scaling_public_files import RunPublicFiles


def complete(setup):
    public = RunPublicFiles(setup.paths.public, setup.kwargs['allocation'])
    trial = setup.create()
    try:
        result = trial.run('1' * 64)
    except BaseException:
        public.close()
        raise
    return trial, result, public


def transfer(trial, result, public, *, guard=None):
    guard = trial.validate if guard is None else guard
    for role in ('collector_journal', 'transaction_trace'):
        artifact = getattr(result.load, role)
        public.adopt_existing(role, trial._load.public_descriptor(role), artifact.sha256,
                              artifact.bytes, native_parent=None, guard=guard)
    for role, name in authority._NATIVE_ROLES:
        artifact = trial._outputs.artifact(name)
        public.adopt_existing(role, trial._outputs.descriptor(name), artifact.sha256,
                              artifact.bytes, native_parent=trial._outputs._root, guard=guard)
    native = trial._generated.inputs
    artifacts = {item.path: item for item in result.generation.artifacts}
    for role, name in authority._GENESIS:
        digest, size = (result.generation.anchors_sha256, result.generation.anchors_bytes) if name == 'genesis-anchors.json' else (
            artifacts[name].sha256, artifacts[name].bytes)
        public.copy_genesis(role, native.public_descriptor(name), digest, size, guard=guard)
    public.seal_sources(guard=guard)
    return trial._captures.transfer(lambda: None)


@pytest.mark.parametrize('trial_setup', [1, 4], indirect=True)
def test_actual_trial_commits_to_actual_public_owners_and_survives_private_close(trial_setup):
    setup = trial_setup
    trial, result, public = complete(setup)
    token = object()
    owner = authority.CompletedRunAuthority.admit(trial, token)
    captures = None
    try:
        assert owner._phase == 'pending' and owner._trial is trial
        captures = transfer(trial, result, public)
        owner.commit(token, public, captures)
        assert owner._phase == 'committed' and owner._trial is None
        trial.close()
        setup.c.clock.now = result.original_deadline_ns + 1
        projection = owner.reconcile(token, result.resources)
        assert projection.variant == result.variant and len(projection.readiness) == 4
        assert projection.plan.load.seed == result.load.plan.seed
        assert projection.load.process == tuple(getattr(result.load.process, name)
                                                 for name in projection.load.process._fields)
        assert len(projection.requests) == projection.canonical.row_count == 12
        assert projection.requests[0].cohort == 'warmup'
        assert projection.requests[-1].local_block_height == projection.requests[-1].block_height
        assert projection.generation.accounts[0].account_id == result.generation.accounts[0].account_id
        assert len(projection.artifacts) == 13 and projection.resources.capture_bytes == result.resources.capture_bytes
        assert not hasattr(projection.requests[0], 'canonical_bytes')
        with pytest.raises(AttributeError): object.__setattr__(projection.requests[0], 'block_height', 0)
        assert owner.projection(token) == projection
        owner.close(token)
        with pytest.raises(authority.CompletedAuthorityError): owner.check_provenance(token)
        assert public.verify() and captures.verify() == result.capture_census
    finally:
        owner.close(token)
        if captures is not None: captures.close()
        public.close()


@pytest.mark.parametrize('value', [None, {}, (), object()])
def test_receipt_or_constructible_value_cannot_create_authority(value):
    with pytest.raises(authority.CompletedAuthorityError): authority.CompletedRunAuthority.admit(value, object())
    with pytest.raises(authority.CompletedAuthorityError): authority.CompletedRunAuthority(value, object())


@pytest.mark.parametrize('value', [True, None, {}, [], bytearray(b'x'), ('x',) * 129,
                                 'x' * 4097, b'x' * (authority.MAX_ATTESTATION_BYTES + 1),
                                 1 << 128, -(1 << 63) - 1, Path('relative')])
def test_public_freeze_rejects_unbounded_or_mutable_values(value):
    with pytest.raises(authority.CompletedAuthorityError): authority._Freeze(1, 1, 32)(value)


def test_public_freeze_owns_records_without_original_object_alias():
    original = authority.ProcessIdentity(123, 4, 5, 6, 7, 'B' * 32, 'a' * 64)
    frozen = authority._Freeze(1, 1, 32)(original)
    object.__setattr__(original, 'pid', 999)
    assert frozen.pid == 123 and frozen.start_abstime == 7 and frozen.executable_sha256 == 'a' * 64
    with pytest.raises(AttributeError): object.__setattr__(frozen, 'pid', 999)
    with pytest.raises(authority.CompletedAuthorityError): authority._Freeze(1, 1, 1)(b'ab')
    nested = 'leaf'
    for _ in range(17): nested = (nested,)
    with pytest.raises(authority.CompletedAuthorityError): authority._Freeze(1, 1, 32)(nested)


def test_actual_source_rejects_incomplete_wrong_type_deadline_and_result_alias(trial_setup):
    trial, result, public = complete(trial_setup)
    try:
        for value in (result, replace(result), {'result': result}, None):
            with pytest.raises(authority.CompletedAuthorityError): authority.CompletedRunAuthority.admit(value, object())
        for name, value in (('_phase', 'new'), ('_phase', 'failed'),
                            ('_result', replace(result, original_deadline_ns=result.original_deadline_ns + 1)),
                            ('_scheduled', True), ('_scheduled', 65537)):
            before = getattr(trial, name)
            setattr(trial, name, value)
            with pytest.raises(authority.CompletedAuthorityError): authority.CompletedRunAuthority.admit(trial, object())
            setattr(trial, name, before)
        for token in (None, 0, True, {}):
            with pytest.raises(authority.CompletedAuthorityError): authority.CompletedRunAuthority.admit(trial, token)
        trial_setup.c.clock.now = result.original_deadline_ns
        with pytest.raises(authority.CompletedAuthorityError): authority.CompletedRunAuthority.admit(trial, object())
    finally:
        public.close()


def _state_source(trial, source, monkeypatch):
    """Isolate state transitions using one actual captured source, no invented receipt.

    Full source validation executes in the two lane compositions. These focused
    lifecycle cases control its boundary to exercise callbacks without repeating
    native-fixture setup or broad original-file scans for every transition.
    """
    calls = []
    def check(value):
        assert value is trial
        calls.append(value)
        return source
    monkeypatch.setattr(authority, '_source', check)
    return calls


def test_pending_token_close_and_commit_callback_failures_are_permanent(trial_setup, monkeypatch):
    trial, result, public = complete(trial_setup)
    source = authority._source(trial)
    token = object()
    captures = transfer(trial, result, public)
    calls = _state_source(trial, source, monkeypatch)
    owners = []
    try:
        for operation in ('validate', 'projection', 'reconcile', 'check_provenance'):
            owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
            args = (token, result.resources) if operation == 'reconcile' else (token,)
            with pytest.raises(authority.CompletedAuthorityError): getattr(owner, operation)(*args)
            assert owner._phase == 'failed'
            with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, public, captures)
        for wrong in (object(), None, 0, type('Equal', (), {'__eq__': lambda *_: True})()):
            owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
            before = len(calls)
            with pytest.raises(authority.CompletedAuthorityError): owner.commit(wrong, public, captures)
            assert owner._phase == 'failed' and len(calls) == before
        for bad_files, bad_captures in ((None, captures), (public, None), (SimpleNamespace(), captures)):
            owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
            before = len(calls)
            with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, bad_files, bad_captures)
            assert len(calls) == before and owner._phase == 'failed'
        for boundary in ('deadline_before', 'deadline_during', 'close', 'reentry'):
            owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
            saved = trial_setup.c.clock.now
            physical = owner._physical
            calls_at_entry = len(calls)
            def check(files, captures, committing=False):
                physical(files, captures, committing)
                if boundary == 'deadline_during': trial_setup.c.clock.now = result.original_deadline_ns
                elif boundary == 'close': owner.close(token)
                elif boundary == 'reentry':
                    with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, files, captures)
            monkeypatch.setattr(owner, '_physical', check)
            if boundary == 'deadline_before': trial_setup.c.clock.now = result.original_deadline_ns
            with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, public, captures)
            assert owner._phase == 'failed'
            if boundary != 'close': assert owner._trial is trial
            with pytest.raises(authority.CompletedAuthorityError): owner.check_provenance(token)
            trial_setup.c.clock.now = saved
            assert len(calls) >= calls_at_entry
        owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
        # Invalidating pending provenance invokes no source/physical callback.
        before = len(calls)
        owner.close(token)
        assert owner._trial is None and len(calls) == before
        with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, public, captures)
    finally:
        for owner in owners: owner.close(token)
        captures.close(); public.close()


def test_original_replay_reconciliation_rejects_all_join_and_schedule_changes(trial_setup, monkeypatch):
    trial, result, public = complete(trial_setup)
    source = authority._source(trial)
    token = object()
    captures = transfer(trial, result, public)
    _state_source(trial, source, monkeypatch)
    owners = []
    replay = result.resources
    request, app = replay.signed_requests[0], replay.applied_requests[0]
    def signed(**changes):
        return replace(replay, signed_requests=(replace(request, **changes), *replay.signed_requests[1:]))
    def applied(**changes):
        return replace(replay, applied_requests=(replace(app, **changes), *replay.applied_requests[1:]))
    bad = [replace(replay, signed_requests=replay.signed_requests[:-1]),
           replace(replay, applied_requests=replay.applied_requests[:-1]),
           replace(replay, signed_requests=(replay.signed_requests[1], *replay.signed_requests[1:])),
           signed(index=True), signed(index=1), signed(hash='b' * 63 + '1'),
           signed(canonical_bytes=request.canonical_bytes + b'x'), signed(canonical_sha256='b' * 64),
           signed(canonical_bytes=bytearray(request.canonical_bytes)),
           applied(index=1), applied(index=True), applied(hash='b' * 63 + '1'),
           replace(replay, capture_bytes=replay.capture_bytes + 1),
           replace(replay, journal_sha256='b' * 64),
           replace(replay, samples=tuple(reversed(replay.samples))),
           replace(replay, geometry=replace(replay.geometry, warmup_ns=replay.geometry.warmup_ns + 1))]
    for name in ('cohort', 'sequence', 'logical_id', 'scheduled_offset_ns', 'account_index'):
        old = getattr(request.plan, name)
        value = 'measurement' if name == 'cohort' else 'b' * 64 if name == 'logical_id' else old + 1
        bad.append(signed(plan=replace(request.plan, **{name: value})))
    for name in ('offer_offset_ns', 'acknowledgment_offset_ns', 'applied_offset_ns', 'block_height',
                 'status_attempts', 'local_applied_offset_ns', 'local_block_height', 'local_status_attempts'):
        bad.append(applied(**{name: getattr(app, name) + 1}))
        bad.append(applied(**{name: True}))
    changed_budget = authority.parse_run_budget(authority.run_budget_inputs(replay.allocation))
    object.__setattr__(changed_budget.run, 'pair_index', 2)
    bad.append(replace(replay, allocation=changed_budget))
    try:
        for changed in bad:
            owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
            owner.commit(token, public, captures)
            with pytest.raises(authority.CompletedAuthorityError): owner.reconcile(token, changed)
            assert owner._phase == 'failed'
            with pytest.raises(authority.CompletedAuthorityError): owner.reconcile(token, replay)
        owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
        owner.commit(token, public, captures)
        assert owner.reconcile(token, replay).requests == source[0].requests
        with pytest.raises(authority.CompletedAuthorityError): owner.reconcile(token, replay)
        assert owner._phase == 'failed'
    finally:
        for owner in owners: owner.close(token)
        captures.close(); public.close()


def test_all_thirteen_physical_bindings_and_capture_census_are_required(trial_setup, monkeypatch):
    trial, result, public = complete(trial_setup)
    source = authority._source(trial)
    token = object()
    captures = transfer(trial, result, public)
    _state_source(trial, source, monkeypatch)
    owners = []
    verify_public, verify_captures = public.verify, captures.verify
    original = verify_public()
    cases = [original[:-1], tuple(reversed(original))]
    for index in range(13):
        row = original[index]
        changed = replace(row, binding=replace(row.binding, sha256='b' * 64))
        cases.append((*original[:index], changed, *original[index + 1:]))
    first = original[0]
    for changes in ({'role': 'unknown'}, {'bytes': first.bytes + 1}, {'max_bytes': first.max_bytes + 1},
                    {'binding': replace(first.binding, label='foreign')},
                    {'binding': replace(first.binding, path='runs/pair-02/four_lane/collector.jsonl')}):
        cases.append((replace(first, **changes), *original[1:]))
    try:
        for rows in cases:
            with monkeypatch.context() as patch:
                def incorrect():
                    verify_public()
                    return rows
                patch.setattr(public, 'verify', incorrect)
                owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
                with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, public, captures)
                assert owner._phase == 'failed'
        for changes in ({'files': result.capture_census.files - 1}, {'bytes': result.capture_census.bytes + 1},
                        {'census_sha256': 'b' * 64}, {'metadata_sha256': 'b' * 64}):
            with monkeypatch.context() as patch:
                def incorrect():
                    verify_captures()
                    return replace(result.capture_census, **changes)
                patch.setattr(captures, 'verify', incorrect)
                owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
                with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, public, captures)
                assert owner._phase == 'failed'
        for cls, attribute, value in ((type(public), 'directory', public.directory.parent),
                                     (type(captures), 'directory', captures.directory.parent)):
            with monkeypatch.context() as patch:
                patch.setattr(cls, attribute, property(lambda _: value))
                owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
                with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, public, captures)
                assert owner._phase == 'failed'
        owner = authority.CompletedRunAuthority.admit(trial, token); owners.append(owner)
        owner.commit(token, public, captures)
        with pytest.raises(authority.CompletedAuthorityError): owner.commit(token, public, captures)
        assert owner._phase == 'failed'
        assert public.verify() == original and captures.verify() == result.capture_census
    finally:
        for owner in owners: owner.close(token)
        captures.close(); public.close()


def test_earlier_public_file_mutation_during_capture_scan_poisoned(trial_setup, monkeypatch):
    trial, result, public = complete(trial_setup)
    source = authority._source(trial)
    token = object()
    captures = transfer(trial, result, public)
    _state_source(trial, source, monkeypatch)
    owner = authority.CompletedRunAuthority.admit(trial, token)
    try:
        owner.commit(token, public, captures)
        trial.close()
        trial_setup.c.clock.now = result.original_deadline_ns + 1
        path = trial_setup.paths.load.collector_journal
        raw = path.read_bytes()
        calls = []
        def mutate():
            if not calls:
                calls.append(True)
                # A same-size in-place write leaves the parent namespace unchanged.
                with path.open('r+b') as stream:
                    stream.write(b'!' + raw[1:]); stream.flush(); os.fsync(stream.fileno())
        monkeypatch.setattr(captures, '_guard', mutate)
        with pytest.raises(authority.CompletedAuthorityError): owner.validate(token)
        assert calls == [True] and owner._phase == 'failed'
        with pytest.raises(authority.CompletedAuthorityError): owner.check_provenance(token)
    finally:
        owner.close(token); captures.close(); public.close()
