"""Short-write and crash recovery for native completion's public journals.

All effects use the existing isolated native nginx fixture. No live services,
private configuration reads or alternate publication-journal decoder are used.
"""
from __future__ import annotations

import errno
import json
import os
from pathlib import Path
import stat

import pytest

from taira_native_edge_completion_test import MODULE, OWNER, _complete, effect_admission
from taira_native_nginx_apply_test import _identity, native_apply


def append_phase(journal, phase):
    """Record one bounded public intent using the fixture's pinned publisher."""
    journal.append(phase, publication_identity=None, backup_identity=None,
        publisher_journal=journal.admission.plan["nginx"]["publication"]["prior"]["journal"])


def journal_stages(admission):
    return list(admission.path.glob(".native-completion-*.journal"))


def inode(fd):
    info = os.fstat(fd)
    return info.st_dev, info.st_ino


def publisher_path(admission):
    return Path(admission.plan["nginx"]["publication"]["prior"]["journal"]["path"])


def journal_child(admission, fd):
    info = os.stat("native-completion.ndjson", dir_fd=admission.directory, follow_symlinks=False)
    return inode(fd) == (info.st_dev, info.st_ino)


def test_short_stage_writes_publish_only_complete_canonical_chains(effect_admission, monkeypatch):
    admission, _ = effect_admission
    journal = MODULE.CompletionJournal(admission)
    original = os.write
    monkeypatch.setattr(MODULE.os, "write", lambda fd, data: original(fd, data[:13]))
    try:
        before = inode(journal.fd)
        append_phase(journal, "admitted")
        assert inode(journal.fd) != before
        append_phase(journal, "rollback_requested")
        journal.check()
        body = (admission.path / journal.name).read_bytes()
        assert body.endswith(b"\n")
        rows = [json.loads(line) for line in body.splitlines()]
        assert [row["sequence"] for row in rows] == [1, 2]
        assert rows == journal.records
        assert stat.S_IMODE(os.fstat(journal.fd).st_mode) == 0o600
        assert not journal_stages(admission)
    finally:
        journal.close()


def test_enospc_during_stage_preserves_visible_prefix_and_partial_evidence(effect_admission, monkeypatch):
    admission, _ = effect_admission
    journal = MODULE.CompletionJournal(admission)
    append_phase(journal, "admitted")
    path = admission.path / journal.name
    original_body, original_identity = path.read_bytes(), MODULE.identity(path.stat())
    original = os.write
    calls = 0
    def interrupted(fd, data):
        nonlocal calls
        calls += 1
        if calls == 1:
            return original(fd, data[:17])
        raise OSError(errno.ENOSPC, "fixture disk full")
    monkeypatch.setattr(MODULE.os, "write", interrupted)
    try:
        with pytest.raises(OSError):
            append_phase(journal, "rollback_requested")
        assert path.read_bytes() == original_body
        assert MODULE.identity(path.stat()) == original_identity
        journal.check()
        assert len(journal_stages(admission)) == 1
        assert journal_stages(admission)[0].read_bytes() == original_body[:17]
    finally:
        journal.close()
    monkeypatch.setattr(MODULE.os, "write", original)
    recovered = MODULE.CompletionJournal(admission)
    try:
        assert len(recovered.records) == 1
        append_phase(recovered, "rollback_requested")
        assert [row["sequence"] for row in recovered.records] == [1, 2]
    finally:
        recovered.close()


def test_crash_after_atomic_exchange_reopens_complete_record_without_duplicates(effect_admission, monkeypatch):
    admission, _ = effect_admission
    journal = MODULE.CompletionJournal(admission)
    append_phase(journal, "admitted")
    exchange = OWNER.native_exchange
    def interrupted(directory, first, second):
        exchange(directory, first, second)
        raise KeyboardInterrupt("fixture after atomic journal exchange")
    monkeypatch.setattr(OWNER, "native_exchange", interrupted)
    try:
        with pytest.raises(KeyboardInterrupt):
            append_phase(journal, "rollback_requested")
        journal.check()
        assert [row["sequence"] for row in journal.records] == [1, 2]
        assert len(journal_stages(admission)) == 1
    finally:
        journal.close()
    monkeypatch.setattr(OWNER, "native_exchange", exchange)
    reopened = MODULE.CompletionJournal(admission)
    try:
        assert [row["phase"] for row in reopened.records] == ["admitted", "rollback_requested"]
        append_phase(reopened, "source_restored")
        assert [row["sequence"] for row in reopened.records] == [1, 2, 3]
    finally:
        reopened.close()


def test_foreign_displaced_stage_is_preserved_after_exchange(effect_admission, monkeypatch):
    admission, _ = effect_admission
    journal = MODULE.CompletionJournal(admission)
    append_phase(journal, "admitted")
    exchange = OWNER.native_exchange
    def substitute(directory, first, second):
        exchange(directory, first, second)
        os.rename(second, ".retained-displaced-journal", src_dir_fd=directory, dst_dir_fd=directory)
        foreign = os.open(second, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=directory)
        try:
            os.write(foreign, b"foreign complete public evidence\n")
        finally:
            os.close(foreign)
    monkeypatch.setattr(OWNER, "native_exchange", substitute)
    try:
        with pytest.raises(RuntimeError, match="completion_journal_exchange_ambiguous"):
            append_phase(journal, "rollback_requested")
        assert journal_stages(admission)[0].read_bytes() == b"foreign complete public evidence\n"
        assert (admission.path / ".retained-displaced-journal").exists()
        rows = [json.loads(line) for line in (admission.path / journal.name).read_bytes().splitlines()]
        assert [row["sequence"] for row in rows] == [1, 2]
    finally:
        journal.close()


def test_in_place_stage_mutation_before_exchange_preserves_both_files(effect_admission, monkeypatch):
    admission, _ = effect_admission
    journal = MODULE.CompletionJournal(admission)
    append_phase(journal, "admitted")
    original_body = (admission.path / journal.name).read_bytes()
    fsync = os.fsync
    def mutate(fd):
        fsync(fd)
        if stat.S_ISREG(os.fstat(fd).st_mode) and not journal_child(admission, fd):
            os.pwrite(fd, b"foreign stage", 0)
    monkeypatch.setattr(MODULE.os, "fsync", mutate)
    try:
        with pytest.raises(RuntimeError, match="completion_journal_stage_changed"):
            append_phase(journal, "rollback_requested")
        assert (admission.path / journal.name).read_bytes() == original_body
        assert journal_stages(admission)[0].read_bytes().startswith(b"foreign stage")
        journal.check()
    finally:
        journal.close()


def test_proof_arrival_before_journal_exchange_refuses_visibility_change(effect_admission, monkeypatch):
    admission, _ = effect_admission
    journal = MODULE.CompletionJournal(admission)
    append_phase(journal, "admitted")
    original_body = (admission.path / journal.name).read_bytes()
    fsync = os.fsync
    def arrive(fd):
        fsync(fd)
        if stat.S_ISREG(os.fstat(fd).st_mode) and not journal_child(admission, fd):
            (admission.path / "global-proof.json").write_bytes(b"fixture irreversible proof\n")
    monkeypatch.setattr(MODULE.os, "fsync", arrive)
    try:
        with pytest.raises(RuntimeError, match="rollback_after_global_proof"):
            append_phase(journal, "rollback_requested")
        assert (admission.path / journal.name).read_bytes() == original_body
        assert journal_stages(admission)
    finally:
        journal.close()


def publisher_stage_for_fd(path, fd):
    """Select only the actual new public stage for this fixture's owner operation."""
    operation = path.name.removeprefix(".taira-native-nginx-apply-").removesuffix(".receipt.ndjson")
    observed = inode(fd)
    for stage in path.parent.glob(".taira-nginx-journal-" + operation + "-*.stage"):
        current = stage.stat(follow_symlinks=False)
        if (current.st_dev, current.st_ino) == observed:
            return stage
    return None


@pytest.mark.parametrize("fault", ["enospc", "partial_stage_crash", "lost_exchange_ack"])
def test_atomic_terminal_append_resumes_exact_record_once(effect_admission, monkeypatch, fault):
    admission, root = effect_admission
    path = publisher_path(admission)
    prefix, original_identity = path.read_bytes(), MODULE.identity(path.stat())
    original_write, original_exchange = os.write, OWNER.native_exchange
    partial_stage = None
    calls = 0
    exchanged = False

    def interrupted_write(fd, data):
        nonlocal partial_stage, calls
        selected = publisher_stage_for_fd(path, fd)
        if selected is None or fault == "lost_exchange_ack":
            return original_write(fd, data)
        partial_stage = selected
        calls += 1
        if calls == 1:
            written = original_write(fd, data[:23])
            if fault == "partial_stage_crash":
                raise KeyboardInterrupt("fixture partial native stage before publication")
            return written
        raise OSError(errno.ENOSPC, "fixture short publisher stage")

    def interrupted_exchange(directory, first, second):
        nonlocal exchanged
        original_exchange(directory, first, second)
        if second == path.name and fault == "lost_exchange_ack" and not exchanged:
            exchanged = True
            raise KeyboardInterrupt("fixture lost acknowledgment after complete atomic terminal publication")

    monkeypatch.setattr(MODULE.os, "write", interrupted_write)
    monkeypatch.setattr(OWNER, "native_exchange", interrupted_exchange)
    if fault == "enospc":
        first, _ = _complete(admission)
        assert first["status"] == "recovery_pending", first
    else:
        with pytest.raises(KeyboardInterrupt):
            _complete(admission)
    if fault == "lost_exchange_ack":
        assert exchanged
        records = [json.loads(line) for line in path.read_bytes().splitlines()]
        assert records[-1]["phase"] == "rolled_back_unqualified"
        assert path.read_bytes().startswith(prefix)
    else:
        assert (path.read_bytes(), MODULE.identity(path.stat())) == (prefix, original_identity)
        assert partial_stage is not None and partial_stage.read_bytes() == prefix[:23]
        assert path.read_bytes().endswith(b"\n"), "canonical owner must never expose partial bytes"
    reloads = (root / "reload-count").read_bytes()
    monkeypatch.setattr(MODULE.os, "write", original_write)
    monkeypatch.setattr(OWNER, "native_exchange", original_exchange)
    result, rows = _complete(admission)
    assert result["status"] == "rolled_back" and result["error_code"] is None, result
    assert (root / "reload-count").read_bytes() == reloads
    records = [json.loads(line) for line in path.read_bytes().splitlines()]
    assert sum(row["phase"] == "rolled_back_unqualified" for row in records) == 1
    assert rows[-1]["phase"] == "rolled_back"
    assert result["restored_owned_publication"]["journal"]["sha256"]
    completion_path = admission.path / "native-completion.ndjson"
    durable_body = completion_path.read_bytes()
    again, repeated = _complete(admission)
    assert again["status"] == "rolled_back" and repeated[-1]["phase"] == "rolled_back"
    assert completion_path.read_bytes() == durable_body


@pytest.mark.parametrize("fault", [None, "unknown_suffix", "changed_prefix", "proof_fence"])
def test_recorded_terminal_stage_refuses_mutation_or_late_proof(effect_admission, monkeypatch, fault):
    admission, root = effect_admission
    path = publisher_path(admission)
    prefix = path.read_bytes()
    original_exchange = OWNER.native_exchange
    selected_stage = None

    def interrupt_before_exchange(directory, first, second):
        nonlocal selected_stage
        if second == path.name and selected_stage is None:
            selected_stage = path.parent / first
            raise KeyboardInterrupt("fixture durable witness before canonical terminal exchange")
        return original_exchange(directory, first, second)

    monkeypatch.setattr(OWNER, "native_exchange", interrupt_before_exchange)
    with pytest.raises(KeyboardInterrupt):
        _complete(admission)
    assert selected_stage is not None and path.read_bytes() == prefix
    completion_rows = [json.loads(line) for line in (admission.path / "native-completion.ndjson").read_bytes().splitlines()]
    prepared = [row["publisher_write_intent"] for row in completion_rows
                if row["publisher_write_intent"] is not None]
    assert prepared and prepared[-1]["stage"] == _identity(selected_stage)
    assert selected_stage.read_bytes().endswith(b"\n")
    monkeypatch.setattr(OWNER, "native_exchange", original_exchange)
    if fault is None:
        reloads = (root / "reload-count").read_bytes()
        result, rows = _complete(admission)
        assert result["status"] == "rolled_back" and result["error_code"] is None, result
        records = [json.loads(line) for line in path.read_bytes().splitlines()]
        assert records[-1]["journal_write_intent"] == prepared[-1]["intent"]
        assert records[-1]["journal_publication_identity"] == {
            key: prepared[-1]["stage"][key] for key in ("device", "inode", "uid", "gid", "mode")}
        assert (path.stat().st_dev, path.stat().st_ino) == (
            int(prepared[-1]["stage"]["device"]), int(prepared[-1]["stage"]["inode"]))
        assert not selected_stage.exists(), "the exact recorded stage became the canonical owner"
        assert (root / "reload-count").read_bytes() == reloads
        assert rows[-1]["phase"] == "rolled_back"
        return
    if fault == "unknown_suffix":
        with selected_stage.open("ab") as target:
            target.write(b"foreign complete record\n")
    elif fault == "changed_prefix":
        with selected_stage.open("r+b") as target:
            target.write(b"foreign")
    else:
        (admission.path / "global-proof.json").write_bytes(b"fixture irreversible proof\n")
    before = selected_stage.read_bytes(), MODULE.identity(selected_stage.stat())
    reloads = (root / "reload-count").read_bytes()
    if fault == "proof_fence":
        with pytest.raises(RuntimeError, match="rollback_after_global_proof"):
            _complete(admission)
    else:
        refused, _ = _complete(admission)
        assert refused["status"] == "recovery_pending" and refused["error_code"] is not None, refused
    assert (selected_stage.read_bytes(), MODULE.identity(selected_stage.stat())) == before
    assert path.read_bytes() == prefix
    assert (root / "reload-count").read_bytes() == reloads


def test_many_short_terminal_stage_writes_keep_complete_owner_and_finish(effect_admission, monkeypatch):
    admission, _ = effect_admission
    path = publisher_path(admission)
    original = os.write
    calls = 0
    def short(fd, data):
        nonlocal calls
        if publisher_stage_for_fd(path, fd) is not None:
            calls += 1
            return original(fd, data[:19])
        return original(fd, data)
    monkeypatch.setattr(MODULE.os, "write", short)
    result, _ = _complete(admission)
    assert result["status"] == "rolled_back" and result["error_code"] is None, result
    assert calls > 1
    records = [json.loads(line) for line in path.read_bytes().splitlines()]
    assert records[-1]["phase"] == "rolled_back_unqualified"
    assert sum(row["phase"] == "rolled_back_unqualified" for row in records) == 1
