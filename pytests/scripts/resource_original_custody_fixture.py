"""Actual original file/replay owners with an explicit native-authority seam.

The fixture never creates or completes a FixedTrial. Exact-class native authority
methods are controlled test seams; synthetic request projections have no signed
native proof provenance. Every directory, control, capture, raw replay, borrower,
measurement and report owner exercised after that boundary is production code.
"""
from dataclasses import replace
import copy
import hashlib
import os
from pathlib import Path
import shutil
from types import SimpleNamespace

import pytest

import resource_experiment as experiment
import resource_evidence_budget as budget
import resource_replay as replay
import resource_replay_test as frozen
import scaling_experiment_custody as custody
import scaling_native_outputs as outputs
import scaling_public_files as public
from scaling_completed_authority import CompletedRunAuthority, _PUBLIC_RECORDS
from scaling_experiment_custody_test import experiment_setup, trial_setup, ready_setup
from scaling_measurements_test import run as synthetic_projection
from scaling_trial_captures import TrialCaptures
from resource_originating_replay_fixture import ScopedPublishedRuns, RUNS


def write_readonly(path, raw):
    """Create one private test input and retain its original read-only slot."""
    with path.open('xb') as stream:
        os.fchmod(stream.fileno(), 0o600)
        stream.write(raw)
    return os.open(path, public._READ)


@pytest.fixture(scope='module')
def original_raw(tmp_path_factory):
    value = ScopedPublishedRuns(tmp_path_factory.mktemp('custody-raw') / 'evidence')
    # Current admitted fixed plan has zero warmup; capture brackets are identical.
    value.geometry = replace(value.geometry, warmup_ns=0)
    value.runs = tuple(replace(row, geometry=value.geometry) for row in value.runs)
    for index, rows in enumerate(value.events):
        rows[0]['warmup_ns'] = 0
        next(row for row in rows if row['event'] == 'clock_started')['initial_offset_ns'] = -(
            value.geometry.drain_ns + value.geometry.preparation_ahead_ns)
        value.save_journal(index)
    return value


def native_authority_seam(owner, projection, files, captures):
    """Controlled native proof boundary; no native completion can be claimed."""
    authority = object.__new__(CompletedRunAuthority)
    state = {'closed': False, 'reconciled': False}

    def check(token):
        assert token is owner._original_token and not state['closed']
        files._check()
        captures.check_namespace()

    def validate(token):
        check(token)
        files.verify()
        captures.check_metadata()

    def read(token):
        check(token)
        return projection

    def reconcile(token, reduced):
        validate(token)
        assert not state['reconciled'] and type(reduced) is replay.ReplayResult
        assert experiment._resource_snapshot(reduced) == projection.resources
        state['reconciled'] = True
        return projection

    def close(token):
        assert token is owner._original_token
        state['closed'] = True

    authority.check_provenance = check
    authority.validate = validate
    authority.projection = read
    authority.reconcile = reconcile
    authority.close = close
    return authority


@pytest.fixture
def original_custody(experiment_setup, original_raw):
    setup = experiment_setup
    owner = setup.create()
    handles = []
    early = []
    try:
        for index, (pair, variant) in enumerate(RUNS):
            owner._directories.create_run(pair, variant)
            run_root = owner._directories.evidence / 'runs' / f'pair-{pair:02}' / variant
            allocation = budget.select_run_budget(owner._budget, pair, variant)
            files = public.RunPublicFiles(run_root, allocation)
            capture_root = owner._directories.evidence / 'resources' / f'pair-{pair:02}' / variant
            shutil.copytree(original_raw.captures(index), capture_root)
            captures = TrialCaptures(capture_root, allocation, lambda: None)
            early.append((files, captures))
            descriptors = []
            native = outputs.NativeOutputs(run_root / 'native',
                outputs.NativeOutputBudget(*(65536,) * 6, 65536 * 6))
            try:
                for step, roles in outputs._STEPS:
                    native.begin(step)
                    claims = []
                    for role in roles:
                        raw = ('controlled-native-seam:' + role).encode()
                        fd = write_readonly(native.path(role), raw)
                        os.close(fd)
                        claims.append(outputs.PublishedIdentity(role, hashlib.sha256(raw).hexdigest(), len(raw)))
                    native.complete(tuple(claims))
                for role in public._EXISTING:
                    if role.startswith('native_') or role == 'canonical_proof':
                        name = 'proof' if role == 'canonical_proof' else role.removeprefix('native_')
                        item = native.artifact(name)
                        files.adopt_existing(role, native.descriptor(name), item.sha256, item.bytes,
                            native_parent=native.directory_descriptor(), guard=lambda: None)
                    else:
                        raw = original_raw.journal(index).read_bytes() if role == 'collector_journal' else b'{}'
                        fd = write_readonly(run_root / public._EXISTING[role], raw)
                        descriptors.append(fd)
                        files.adopt_existing(role, fd, hashlib.sha256(raw).hexdigest(), len(raw),
                            native_parent=None, guard=lambda: None)
                private = setup.setup.c.root / f'controlled-genesis-{index}'
                private.mkdir(mode=0o700)
                for role, relative in public._GENESIS.items():
                    raw = ('controlled-genesis-seam:' + role).encode()
                    fd = write_readonly(private / Path(relative).name, raw)
                    descriptors.append(fd)
                    files.copy_genesis(role, fd, hashlib.sha256(raw).hexdigest(), len(raw), guard=lambda: None)
                files.seal_sources(guard=lambda: None)
                files.publish_summary('raw_run', b'{}', guard=lambda: None)
                files.publish_summary('run_receipt', b'{}', guard=lambda: None)
            finally:
                native.close()
                for fd in descriptors:
                    os.close(fd)
            owner._directories.finish_run()
            row = original_raw.runs[index]
            journal = next(item for item in files.verify() if item.role == 'collector_journal')
            reduced = replay.replay(capture_root, run_root / 'collector.jsonl', journal.binding.sha256,
                row.peers, row.geometry, expected_policy=allocation.policy, allocation=allocation)
            value = synthetic_projection(owner._plan.trials[index])._replace(
                resources=experiment._resource_snapshot(reduced),
                peers=tuple(_PUBLIC_RECORDS[type(peer.identity)](*(
                    getattr(peer.identity, name) for name in peer.identity.__dataclass_fields__)) for peer in row.peers),
                budget=budget.canonical_run_budget_bytes(allocation),
                original_deadline_ns=owner._original_end - owner._plan.experiment_timeout_ns
                    + owner._plan.trial_timeout_ns + (index + 1) * 1000)
            authority = native_authority_seam(owner, value, files, captures)
            handle = object.__new__(custody.CompletedRunHandle)
            handle._owner = owner
            handle._index = index
            owner._runs.append(custody._Run(handle, authority, files, captures, published=True))
            owner._run_pins = (*owner._run_pins, (handle, authority, files, captures))
            handles.append(handle)
            early.pop()
        owner._phase = 'runs_complete'
        owner.publish_manifest()
        borrower = experiment.ResourceExperiment.from_completed(owner)
        yield SimpleNamespace(owner=owner, borrower=borrower, setup=setup,
            handles=tuple(handles), manifest=owner._manifest[0], raw=original_raw,
            root=owner._directories.evidence,
            member=lambda: owner._directories.evidence / 'resources/pair-01/one_lane/sample-0000000001-peer-0000-status.body')
    finally:
        for files, captures in early:
            captures.close()
            files.close()
