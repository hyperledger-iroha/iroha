"""Acknowledge an already restored publisher without another publication effect."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

import pytest

from taira_native_edge_completion_test import MODULE, OWNER, EffectAdmission, _complete, effect_admission
from taira_native_nginx_apply_test import _prior, native_apply


def public_reference(path):
    return dict(file=dict(path=str(path), identity=MODULE.identity(path.stat())),
                sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def publisher_acknowledgement(admission, root):
    """Use a genuine isolated owner restore, then a fresh terminal receiver."""
    nginx = copy.deepcopy(admission.plan["nginx"])
    publisher = Path(nginx["publication"]["prior"]["journal"]["path"])
    initial = [json.loads(line) for line in publisher.read_bytes().splitlines()]
    original = initial[0].get("prior")
    restored, _ = _complete(admission)
    assert restored["status"] == "rolled_back", restored
    request = {key: nginx[key] for key in ("host_kind", "native", "master", "destination", "operation_id")}
    request["candidate_base64"] = ""
    request["candidate_sha256"] = nginx["candidate"]["sha256"]
    request["renderer_source_sha256"] = nginx["renderer_source"]["sha256"]
    import base64
    request["candidate_base64"] = base64.b64encode(Path(nginx["candidate"]["path"]).read_bytes()).decode()
    request["publication"] = dict(kind="create") if original is None else dict(kind="replace", prior=original)
    receiver = root / "publisher-terminal-acknowledgement"
    receiver.mkdir(mode=0o700)
    acknowledged = EffectAdmission(request, receiver)
    acknowledged.plan["publication_effect"] = dict(kind="publisher_rolled_back", value=dict(
        journal=public_reference(publisher), restored_owned_publication=restored["restored_owned_publication"]))
    return acknowledged, publisher


def test_publisher_restored_ack_is_idempotent_and_never_reloads_or_changes_owner(effect_admission, monkeypatch):
    admission, root = effect_admission
    acknowledged, publisher = publisher_acknowledgement(admission, root)
    publication = root / "conf.d/new-scoped.conf"
    predecessor_journal = Path(acknowledged.plan["publication_effect"]["value"]
                               ["restored_owned_publication"]["journal"]["file"]["path"])
    before = {path: (path.read_bytes(), MODULE.identity(path.stat()))
              for path in (publication, publisher, predecessor_journal)}
    reloads = (root / "reload-count").read_bytes()
    monkeypatch.setattr(OWNER, "signal_master", lambda *args: pytest.fail("acknowledgement cannot reload"))
    try:
        receipt, records = _complete(acknowledged)
        assert receipt["status"] == "rolled_back" and receipt["error_code"] is None, receipt
        assert receipt["restored_owned_publication"] == acknowledged.plan["publication_effect"]["value"]\
            ["restored_owned_publication"]
        assert [row["phase"] for row in records] == ["admitted", "rollback_requested", "rolled_back"]
        completion = acknowledged.path / "native-completion.ndjson"
        journal_body = completion.read_bytes()
        second, _ = _complete(acknowledged)
        assert second["status"] == "rolled_back" and completion.read_bytes() == journal_body
        assert (root / "reload-count").read_bytes() == reloads
        assert all((path.read_bytes(), MODULE.identity(path.stat())) == observed
                   for path, observed in before.items())
    finally:
        acknowledged.close()


@pytest.mark.parametrize("fault", ["unrestored_owner", "foreign_publication", "wrong_operation",
                                   "foreign_publication_path", "remaining_backup", "unknown_terminal"])
def test_publisher_ack_refuses_unknown_or_changed_ownership_without_effects(effect_admission, monkeypatch, fault):
    admission, root = effect_admission
    acknowledged, publisher = publisher_acknowledgement(admission, root)
    effect = acknowledged.plan["publication_effect"]["value"]
    publication = root / "conf.d/new-scoped.conf"
    if fault == "unrestored_owner":
        effect["restored_owned_publication"]["publication"]["file"]["identity"]["ctime_ns"] += 1
    elif fault == "foreign_publication":
        foreign = root / "foreign-publication"
        foreign.write_bytes(publication.read_bytes())
        foreign.chmod(0o600)
        foreign.replace(publication)
    elif fault == "wrong_operation":
        effect["restored_owned_publication"]["operation_id"] = "f" * 32
    elif fault == "foreign_publication_path":
        effect["restored_owned_publication"]["publication"]["file"]["path"] = str(root / "nginx.conf")
    elif fault == "remaining_backup":
        backup = publication.parent / (".taira-nginx-backup-" + acknowledged.plan["nginx"]["operation_id"] + ".public")
        backup.write_bytes(b"unowned retained backup")
        backup.chmod(0o600)
    else:
        rows = [json.loads(line) for line in publisher.read_bytes().splitlines()]
        rows[-1]["phase"] = "rollback_ambiguous"
        publisher.write_bytes(b"".join((json.dumps(row, sort_keys=True) + "\n").encode() for row in rows))
        effect["journal"] = public_reference(publisher)
    before = (publication.read_bytes(), MODULE.identity(publication.stat()), publisher.read_bytes(),
              (root / "reload-count").read_bytes())
    monkeypatch.setattr(OWNER, "signal_master", lambda *args: pytest.fail("refusal cannot reload"))
    try:
        receipt, _ = _complete(acknowledged)
        assert receipt["status"] == "recovery_pending" and receipt["error_code"], receipt
        assert (publication.read_bytes(), MODULE.identity(publication.stat()), publisher.read_bytes(),
                (root / "reload-count").read_bytes()) == before
    finally:
        acknowledged.close()


def test_first_publication_restore_ack_proves_absence_without_another_effect(native_apply, monkeypatch):
    request, root, _ = native_apply
    published = OWNER.remote_apply(request)
    assert published["exit_code"] == 0
    publication = root / "conf.d/new-scoped.conf"
    reference = _prior(root, request["operation_id"], request["candidate_sha256"])
    owned = dict(request, publication=dict(kind="reconcile", prior=reference))
    admission = EffectAdmission(owned, root)
    try:
        acknowledged, publisher = publisher_acknowledgement(admission, root)
    finally:
        admission.close()
    assert not publication.exists()
    before, reloads = publisher.read_bytes(), (root / "reload-count").read_bytes()
    monkeypatch.setattr(OWNER, "signal_master", lambda *args: pytest.fail("absence acknowledgement cannot reload"))
    try:
        receipt, rows = _complete(acknowledged)
        assert receipt["status"] == "rolled_back" and receipt["restored_owned_publication"] is None, receipt
        assert rows[-1]["phase"] == "rolled_back" and not publication.exists()
        assert publisher.read_bytes() == before and (root / "reload-count").read_bytes() == reloads
    finally:
        acknowledged.close()
