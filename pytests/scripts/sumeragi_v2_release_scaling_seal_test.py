"""Actual parent-record publication custody; receipt semantics are an explicit seam."""
from __future__ import annotations

import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest

from sumeragi_v2_release_scaling_cache_test import MAIN, definitions
from sumeragi_v2_release_scaling_selection_test import bootstrap_definitions


@pytest.fixture
def sealing(tmp_path, monkeypatch):
    """Build actual disjoint private trees and retain every production file owner."""
    m = definitions(MAIN)
    invocation = tmp_path / 'invocation'
    evidence = tmp_path / 'bootstrap'
    candidate = tmp_path / 'candidate'
    for path in (invocation, evidence, candidate, invocation/'source',
                 invocation/'output', invocation/'output/release',
                 invocation/'runtime', invocation/'target'):
        path.mkdir(mode=0o700)

    def write(path, raw):
        path.write_bytes(raw)
        path.chmod(0o400)
        return path

    record_raw = b'{"fixture":"semantic authentication is owned by the receipt validator"}\n'
    record = write(evidence/'scaling-execution.json', record_raw)
    write(invocation/'sealed-identity.json', b'{"fixture":"sealed identity"}\n')
    receipt_raw = b'{"fixture":"aggregate receipt"}\n'
    write(invocation/'output/release/RELEASE_COMPLETED.json', receipt_raw)
    ack_raw = b'{"fixture":"canonical acknowledgment tested separately"}\n'
    write(invocation/'receipt-validation-ack.json', ack_raw)
    write(invocation/'runtime/disposable', b'runtime only\n')
    write(invocation/'target/disposable', b'build only\n')
    # This is the sole semantic seam. All held reads, filesystem walks,
    # pruning, publication, repeated fences and rollback are production code.
    def acknowledged(ack, receipt, source, bootstrap, source_sha, candidate_root,
                     scaling_record, signer, scaling_sha):
        assert ack['data'] == ack_raw and receipt['data'] == receipt_raw
        assert source == invocation/'source' and bootstrap == evidence
        assert candidate_root == candidate and scaling_record == record
        assert scaling_sha == hashlib.sha256(record_raw).hexdigest()
        return hashlib.sha256(ack_raw).hexdigest(), len(ack_raw)
    monkeypatch.setattr(m, '_validation_ack', acknowledged)
    held = []
    original_hold = m._hold_regular
    def hold(path, *args, **kwargs):
        value = original_hold(path, *args, **kwargs)
        if path == record:
            held.append(value)
        return value
    monkeypatch.setattr(m, '_hold_regular', hold)

    def run():
        m.seal_release_result(invocation, evidence, 'a'*64, candidate,
            record, 'SHA256:'+'A'*43, hashlib.sha256(record_raw).hexdigest())
    return SimpleNamespace(m=m, invocation=invocation, evidence=evidence,
        record=record, raw=record_raw, held=held, run=run, write=write,
        receipt_raw=receipt_raw, tmp=tmp_path)


def assert_closed(descriptor):
    """Require an actual closed descriptor after completed production cleanup."""
    with pytest.raises(OSError) as raised:
        os.fstat(descriptor)
    assert raised.value.errno == errno.EBADF


def test_original_record_survives_actual_publication_and_disposable_pruning(sealing):
    f = sealing
    f.run()
    assert f.record.read_bytes() == f.raw
    assert (f.evidence/'RELEASE_COMPLETED.json').read_bytes() == f.receipt_raw
    result = json.loads((f.evidence/'release-runner-result.json').read_bytes())
    assert result['receipt']['sha256'] == hashlib.sha256(f.receipt_raw).hexdigest()
    assert (f.invocation/'source').is_dir()
    assert not (f.invocation/'runtime').exists()
    assert not (f.invocation/'target').exists()
    # _digest_regular takes its own short-lived hold; the last one is the
    # original record retained across all publication fences.
    assert f.held[-1]['data'] == f.raw
    assert_closed(f.held[-1]['descriptor'])
    assert_closed(f.held[-1]['parent_fd'])


@pytest.mark.parametrize('mode', (0o400, 0o600))
def test_sealer_preserves_original_handoff_artifact_handles_and_metadata(sealing, mode):
    f = sealing
    bootstrap = bootstrap_definitions()
    root = f.invocation/'output/scaling'
    root.mkdir(mode=0o700)
    for name in ('manifest.json', 'report.json'):
        f.write(root/name, ('{"fixture":"'+name+'"}\n').encode()).chmod(mode)
    # Construct only the artifact-custody part of a handoff. No synthetic
    # command is presented as an original parent execution observation.
    handoff = object.__new__(bootstrap.FixedScalingHandoff)
    handoff._artifact_handles = []
    try:
        artifacts = tuple(handoff._retain_artifact(SimpleNamespace(evidence_root=root), name, 1024)
                          for name in ('manifest.json', 'report.json'))
        original = SimpleNamespace(artifacts=artifacts)
        handoff._observation = original
        metadata = [(root/name).stat() for name in ('manifest.json', 'report.json')]
        assert handoff.revalidate_observation() is original
        f.run()
        assert handoff.revalidate_observation() is original
        for name, before in zip(('manifest.json', 'report.json'), metadata, strict=True):
            after = (root/name).stat()
            assert (after.st_dev, after.st_ino, after.st_mode, after.st_ctime_ns) == (
                before.st_dev, before.st_ino, before.st_mode, before.st_ctime_ns)
        assert root.stat().st_mode & 0o7777 == 0o700
    finally:
        for parent, descriptor, *_ in handoff._artifact_handles:
            os.close(descriptor)
            os.close(parent)


@pytest.mark.parametrize('boundary', ('retained-evidence-inventory.json', 'release-runner-result.json'))
@pytest.mark.parametrize('change', ('bytes', 'same_bytes_new_inode', 'mode',
                                  'descriptor_reuse', 'inheritance', 'nonblocking'))
def test_late_parent_record_substitution_rejects_and_rolls_back(sealing, monkeypatch, boundary, change):
    f = sealing
    original_publish = f.m._publish_inventory
    injected = []
    foreign = None
    reused = None
    drifted = None
    if change == 'descriptor_reuse':
        foreign = os.open(f.write(f.tmp/'foreign', b'foreign owner\n'), os.O_RDONLY)

    def publish(path, data):
        nonlocal reused, drifted
        value = original_publish(path, data)
        if path.name == boundary:
            assert not injected
            injected.append(path)
            if change == 'bytes':
                f.record.chmod(0o600)
                f.record.write_bytes(b'changed parent record\n')
                f.record.chmod(0o400)
            elif change == 'same_bytes_new_inode':
                f.record.unlink()
                f.write(f.record, f.raw)
            elif change == 'mode':
                f.record.chmod(0o600)
                drifted = f.held[-1]['descriptor']
            elif change == 'descriptor_reuse':
                reused = f.held[-1]['descriptor']
                os.close(reused)
                os.dup2(foreign, reused, inheritable=False)
            else:
                drifted = f.held[-1]['descriptor']
                if change == 'inheritance':
                    os.set_inheritable(drifted, True)
                else:
                    fcntl.fcntl(drifted, fcntl.F_SETFL,
                                fcntl.fcntl(drifted, fcntl.F_GETFL) ^ os.O_NONBLOCK)
        return value
    monkeypatch.setattr(f.m, '_publish_inventory', publish)
    try:
        with pytest.raises(f.m.CacheCopyError, match='parent scaling execution'):
            f.run()
        assert len(injected) == 1
        assert f.record.exists()
        for name in ('RELEASE_COMPLETED.json', 'sealed-identity.json',
                     'release-retained-inventory.json', 'receipt-validation-ack.json',
                     'release-runner-private-provenance.json', 'release-runner-result.json'):
            assert not (f.evidence/name).exists(), name
        assert not (f.invocation/'retained-evidence-inventory.json').exists()
        assert_closed(f.held[-1]['parent_fd'])
        if reused is None and drifted is None:
            assert_closed(f.held[-1]['descriptor'])
        elif reused is not None:
            assert os.fstat(reused).st_ino == os.fstat(foreign).st_ino
            assert os.pread(reused, 64, 0) == b'foreign owner\n'
        else:
            # Production cleanup deliberately preserves every slot whose
            # original complete pin changed. The test owns this injected
            # descriptor and closes it itself; it must not demand an unsafe
            # close based only on inode equality.
            assert f.m._retained_descriptor_pin(drifted) != f.held[-1]['descriptor_pin']
            assert os.fstat(drifted).st_ino == f.record.stat().st_ino
            assert os.pread(drifted, len(f.raw), 0) == f.raw
    finally:
        if reused is not None:
            os.close(reused)
        if foreign is not None:
            os.close(foreign)
        if drifted is not None:
            os.close(drifted)
