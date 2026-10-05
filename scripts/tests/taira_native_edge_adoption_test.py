"""Native ownership effects against pinned opaque public incident inodes.

The isolated fixture supplies authority; real signature/OS-parent admission is
covered by the native receiver gate and the separate malformed child tests.
No live services or private configuration bodies are read here.
"""
from __future__ import annotations

import base64
import copy
import hashlib
import os
from pathlib import Path

import pytest

from taira_native_edge_completion_test import MODULE, OWNER, _plan, native_admission
from taira_native_nginx_apply_test import native_apply


def pin(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    return dict(fd=fd, reference=dict(file=dict(path=str(path), identity=MODULE.identity(os.fstat(fd))),
        sha256=hashlib.sha256(os.pread(fd, MODULE.MAX_PUBLIC_BYTES + 1, 0)).hexdigest()))


class IsolatedAdoption:
    def __init__(self, request, root):
        self.path = root / 'adoption'
        self.path.mkdir(mode=0o700)
        self.directory = os.open(self.path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        self.bindings = dict(operation_id='c'*32, request_sha256='d'*64, authorization_sha256='e'*64,
            authorization_nonce='f'*32, host_pair_sha256='1'*64, host_identity_sha256='2'*64,
            custody_root=str(root), helper_source_closure_sha256='3'*64)
        self.request = copy.deepcopy(request)
        self.request.update(operation_id=self.bindings['operation_id'], publication=dict(kind='create'))
        nginx = _plan(self.request, root)
        renderer = Path(nginx['renderer_source']['path'])
        nginx['renderer_source']['sha256'] = hashlib.sha256(renderer.read_bytes()).hexdigest()
        include = root / 'conf.d/new-scoped.conf'
        publication = pin(include)
        opaque_paths = sorted(root.glob('.taira-native-nginx-apply-*.receipt.ndjson'))
        for index, path in enumerate(opaque_paths):
            path.write_bytes(b'opaque incident evidence, never a JSON owner '+str(index).encode()+b'\n')
            path.chmod(0o600)
        extra = root / ('.taira-native-nginx-apply-' + 'b'*32 + '.receipt.ndjson')
        extra.write_bytes(b'opaque second incident\n')
        extra.chmod(0o600)
        opaque = [pin(path) for path in sorted([*opaque_paths, extra])]
        self.plan = dict(schema=MODULE.ADOPTION_PREFIX+'plan.v1', nginx=nginx,
            publication=publication['reference'], opaque_journals=[copy.deepcopy(p['reference']) for p in opaque])
        self.packet = dict(self.bindings, publication=publication, opaque_journals=opaque,
            progress=dict(reference=dict(sha256='4'*64)))
        self.incident_refs = None
        self.fd_handles = [publication['fd'], *(item['fd'] for item in opaque)]
        self.refresh_directory()

    def verify(self):
        MODULE.retained_public(self.packet['publication'])
        if self.incident_refs is not None:
            for item in self.incident_refs:
                MODULE.retained_public(item)
        return OWNER

    def refresh_directory(self):
        self.directory_identity = MODULE.identity(os.fstat(self.directory))

    def reopen_packet(self):
        self.incident_refs = None
        for ordinal, original in enumerate(self.plan['opaque_journals']):
            path = Path(original['file']['path'])
            if not path.exists():
                path = Path(MODULE.adoption_archive_path(self, ordinal))
            retained = pin(path)
            self.fd_handles.append(retained['fd'])
            self.packet['opaque_journals'][ordinal] = retained
        self.refresh_directory()

    def close(self):
        for fd in self.fd_handles:
            os.close(fd)
        os.close(self.directory)


@pytest.fixture
def adoption(native_apply):
    request, root, _ = native_apply
    original = OWNER.remote_apply(request)
    assert original['exit_code'] == 0
    admission = IsolatedAdoption(request, root)
    try:
        yield admission, root
    finally:
        admission.close()


def run(admission):
    journal = MODULE.AdoptionJournal(admission)
    try:
        return MODULE.adopt_owned(admission, journal)
    finally:
        journal.close()


def test_opaque_adoption_preserves_serving_include_and_replays_without_effect(adoption, monkeypatch):
    admission, root = adoption
    before = copy.deepcopy(admission.plan['publication'])
    reload_count = (root / 'reload-count').read_text()
    opaque = [Path(item['file']['path']).read_bytes() for item in admission.plan['opaque_journals']]
    loads = OWNER.json.loads
    def forbid_opaque(body, *args, **kwargs):
        assert body not in opaque, 'opaque incident journals must never enter an owner decoder'
        return loads(body, *args, **kwargs)
    monkeypatch.setattr(OWNER.json, 'loads', forbid_opaque)
    result = run(admission)
    assert result['status'] == 'adopted_unqualified'
    assert result['owned_publication']['publication'] == before
    assert result['error_code'] is None
    assert len(result['archived_journals']) == len(opaque)
    for ordinal, reference in enumerate(result['archived_journals']):
        assert Path(reference['file']['path']).read_bytes() == opaque[ordinal]
        assert reference['file']['identity']['inode'] == admission.plan['opaque_journals'][ordinal]['file']['identity']['inode']
        assert not Path(admission.plan['opaque_journals'][ordinal]['file']['path']).exists()
    assert (root / 'reload-count').read_text() == reload_count
    authority = (admission.path / 'native-adoption.ndjson').read_bytes()
    owner = Path(result['owned_publication']['journal']['file']['path']).read_bytes()
    admission.reopen_packet()
    again = run(admission)
    assert again['owned_publication'] == result['owned_publication']
    assert (admission.path / 'native-adoption.ndjson').read_bytes() == authority
    assert Path(result['owned_publication']['journal']['file']['path']).read_bytes() == owner
    assert (root / 'reload-count').read_text() == reload_count


@pytest.mark.parametrize('boundary', ['archive_after', 'owner_before', 'owner_after'])
def test_lost_native_ack_reuses_only_recorded_exact_incident_and_stage(adoption, monkeypatch, boundary):
    admission, root = adoption
    publish = OWNER.native_publish_no_replace
    triggered = False
    def interrupt(directory, source, target):
        nonlocal triggered
        selected = (boundary == 'archive_after' and target.startswith('.taira-native-nginx-adoption-')
            or boundary.startswith('owner_') and target == '.taira-native-nginx-apply-'+'c'*32+'.receipt.ndjson')
        if selected and not triggered:
            triggered = True
            if boundary != 'owner_before':
                publish(directory, source, target)
            raise KeyboardInterrupt('isolated lost native acknowledgement')
        return publish(directory, source, target)
    monkeypatch.setattr(OWNER, 'native_publish_no_replace', interrupt)
    with pytest.raises(KeyboardInterrupt):
        run(admission)
    assert triggered
    monkeypatch.setattr(OWNER, 'native_publish_no_replace', publish)
    admission.reopen_packet()
    result = run(admission)
    assert result['status'] == 'adopted_unqualified'
    assert result['owned_publication']['publication'] == admission.plan['publication']
    assert (root / 'reload-count').read_text() == '1'
    journal = MODULE.AdoptionJournal(admission)
    try:
        assert len([row for row in journal.records if row['write_intent'] is not None]) == 1
    finally:
        journal.close()


def test_unknown_canonical_owner_refuses_before_archiving_any_incident(adoption):
    admission, root = adoption
    foreign = root / ('.taira-native-nginx-apply-'+'9'*32+'.receipt.ndjson')
    foreign.write_bytes(b'foreign owner preserved\n')
    foreign.chmod(0o600)
    result = run(admission)
    assert result['status'] == 'recovery_pending'
    assert result['error_code'] == 'adoption_unadmitted_journal'
    assert result['owned_publication'] is None
    assert result['archived_journals'] == []
    assert foreign.read_bytes() == b'foreign owner preserved\n'
    assert all(Path(item['file']['path']).exists() for item in admission.plan['opaque_journals'])
    assert (root / 'reload-count').read_text() == '1'


def test_substituted_archive_is_not_adopted_from_matching_bytes(adoption, monkeypatch):
    admission, root = adoption
    publish = OWNER.native_publish_no_replace
    def interrupt(directory, source, target):
        publish(directory, source, target)
        if target.startswith('.taira-native-nginx-adoption-'):
            raise KeyboardInterrupt('isolated after archive')
    monkeypatch.setattr(OWNER, 'native_publish_no_replace', interrupt)
    with pytest.raises(KeyboardInterrupt):
        run(admission)
    monkeypatch.setattr(OWNER, 'native_publish_no_replace', publish)
    path = Path(MODULE.adoption_archive_path(admission, 0))
    body = path.read_bytes()
    path.rename(root / 'retained-genuine-incident')
    path.write_bytes(body)
    path.chmod(0o600)
    admission.reopen_packet()
    result = run(admission)
    assert result['status'] == 'recovery_pending'
    assert result['owned_publication'] is None
    assert path.read_bytes() == body
    assert (root / 'retained-genuine-incident').read_bytes() == body
    assert (root / 'reload-count').read_text() == '1'


def test_adoption_refuses_self_selected_custodian_before_opening_inputs(native_admission, monkeypatch):
    packet = copy.deepcopy(native_admission['packet'])
    packet['schema'] = MODULE.ADOPTION_PREFIX + 'admission.v1'
    packet['request'] = packet.pop('inventory')
    packet['request_sha256'] = packet.pop('inventory_sha256')
    for field in ('action', 'checkpoints', 'fence'):
        packet.pop(field)
    packet.update(trusted_key=packet['request'], source_plan=packet['plan'],
        lease=packet['request'], publication=packet['request'], opaque_journals=[packet['plan']])
    opened = []
    anchored = MODULE.open_anchored
    def record_open(path, **kwargs):
        opened.append(str(path))
        return anchored(path, **kwargs)
    monkeypatch.setattr(MODULE, 'open_anchored', record_open)
    with pytest.raises(RuntimeError, match='independent_native_anchor_required'):
        MODULE.AdoptionAdmission(packet)
    assert opened == []


def test_adoption_requires_its_closed_independent_authority_packet():
    with pytest.raises(RuntimeError, match='adoption_admission_fields'):
        MODULE.AdoptionAdmission(dict(schema=MODULE.ADOPTION_PREFIX + 'admission.v1',
            operation_id='c'*32, invented_operator_approval=True))
