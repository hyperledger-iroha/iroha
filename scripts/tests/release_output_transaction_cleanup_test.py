"""Actual inert descriptor/rollback controls for the sole output transaction."""
from contextlib import contextmanager
import errno
import os
from pathlib import Path

import pytest

from scripts import release_manifest_signing as signing


@contextmanager
def observed_descriptors(monkeypatch):
    """Close only still-matching test-owned leftovers after counterfactual failures."""
    real_open, real_close, real_fstat = os.open, os.close, os.fstat
    observed = {}
    def opened(*args, **kwargs):
        fd = real_open(*args, **kwargs)
        info = real_fstat(fd)
        observed[fd] = (info.st_dev, info.st_ino)
        return fd
    try:
        with monkeypatch.context() as patch:
            patch.setattr(os, "open", opened)
            yield observed, real_open, real_close, real_fstat
    finally:
        for fd, expected in observed.items():
            try:
                actual = real_fstat(fd)
                if (actual.st_dev, actual.st_ino) == expected:
                    real_close(fd)
            except OSError:
                pass


def transaction(tmp_path, count=1):
    parent = tmp_path / "outputs"
    parent.mkdir()
    paths = [parent / f"release-{index}.sig" for index in range(count)]
    return signing._ReleaseOutputTransaction([(path, "test output") for path in paths]), paths


def owned(transaction):
    return tuple(row[1] for row in transaction.created) + tuple(
        fd for _, _, descriptors in transaction.parents.values() for fd in descriptors
    )


def assert_closed(descriptors, fstat=os.fstat):
    assert descriptors
    for fd in descriptors:
        with pytest.raises(OSError) as failure:
            fstat(fd)
        assert failure.value.errno == errno.EBADF


@pytest.mark.parametrize("body_failure", (False, True))
def test_close_after_success_error_drains_all_once_and_preserves_reused_number(
    tmp_path, monkeypatch, body_failure
):
    tx, paths = transaction(tmp_path, 2)
    original = ValueError("original body failure")
    previous = LookupError("original cause")
    original.__cause__ = previous
    replacements, attempts = [], []
    sentinel = tmp_path / "sentinel"
    sentinel.write_bytes(b"unrelated replacement fd")
    with observed_descriptors(monkeypatch) as (_, real_open, real_close, real_fstat):
        tx.__enter__()
        for path in paths:
            tx.install(path, b"retained output", "test output")
        originals = owned(tx)
        def close(fd):
            if fd not in originals:
                return real_close(fd)
            attempts.append(fd)
            real_close(fd)
            if not replacements:
                replacement = real_open(sentinel, os.O_RDONLY)
                replacements.append(replacement)
                assert replacement == fd
                raise OSError(errno.EIO, "close succeeded then reported failure")
        try:
            with monkeypatch.context() as patch:
                patch.setattr(os, "close", close)
                if body_failure:
                    tx.__exit__(ValueError, original, None)
                    failure = original
                else:
                    with pytest.raises(signing.ReleaseManifestSignatureError, match="outputs retained") as result:
                        tx.__exit__(None, None, None)
                    failure = result.value
            assert set(attempts) == set(originals)
            assert len(attempts) == len(originals) == len(set(attempts))
            assert len(failure.transaction_cleanup_errors) == 1
            assert type(failure.transaction_cleanup_errors) is tuple
            assert original.__cause__ is previous
            assert tx.created == [] and tx.parents == {}
            assert real_fstat(replacements[0]).st_ino == sentinel.stat().st_ino
            with pytest.raises(signing.ReleaseManifestSignatureError, match="not open"):
                tx.__exit__(None, None, None)
            assert real_fstat(replacements[0]).st_ino == sentinel.stat().st_ino
            for path in paths:
                assert path.exists() is (not body_failure)
        finally:
            for fd in replacements:
                real_close(fd)


def test_temporary_lineage_drain_failure_poison_does_not_leak_or_hide_original_error(tmp_path, monkeypatch):
    tx, _ = transaction(tmp_path)
    with observed_descriptors(monkeypatch) as (observed, _, real_close, real_fstat):
        tx.__enter__()
        held = owned(tx)
        attempts = []
        def close(fd):
            if fd in held:
                return real_close(fd)
            attempts.append(fd)
            real_close(fd)
            raise OSError(errno.EIO, "temporary lineage close reported failure")
        with monkeypatch.context() as patch:
            patch.setattr(os, "close", close)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="cleanup failed") as failure:
                tx.assert_unchanged()
        assert len(attempts) == len(tx.parents[next(iter(tx.parents))][2])
        assert len(attempts) == len(set(attempts))
        assert len(failure.value.transaction_cleanup_errors) == len(attempts)
        assert_closed(attempts, real_fstat)
        with pytest.raises(signing.ReleaseManifestSignatureError, match="invalidated"):
            tx.__exit__(None, None, None)
        assert_closed(held, real_fstat)
        assert tx.parents == {} and tx.created == []


@pytest.mark.parametrize("persistent", (False, True))
def test_opened_leaf_is_owned_before_first_fstat_and_never_lost(tmp_path, monkeypatch, persistent):
    tx, paths = transaction(tmp_path)
    with observed_descriptors(monkeypatch) as (observed, _, _, real_fstat):
        tx.__enter__()
        directory_fds = owned(tx)
        candidate = []
        original = OSError(errno.EIO, "first leaf metadata unavailable")
        calls = 0
        # The first leaf fstat is identified by its real regular-file mode, not
        # a private implementation counter or alternate file-opening path.
        import stat
        def metadata(fd):
            nonlocal calls
            info = real_fstat(fd)
            if stat.S_ISREG(info.st_mode):
                candidate.append(fd)
                calls += 1
                if persistent or calls == 1:
                    raise original
            return info
        with monkeypatch.context() as patch:
            patch.setattr(os, "fstat", metadata)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="cannot publish") as failure:
                try:
                    tx.install(paths[0], b"must never be written", "test output")
                except BaseException as exc:
                    tx.__exit__(type(exc), exc, exc.__traceback__)
                    raise
        assert failure.value.__cause__ is original
        assert candidate
        assert_closed((*directory_fds, candidate[0]), real_fstat)
        assert tx.created == [] and tx.parents == {}
        if persistent:
            assert paths[0].read_bytes() == b""
            assert failure.value.transaction_cleanup_errors == (original,)
        else:
            assert not paths[0].exists()


@pytest.mark.parametrize("operation", ("ftruncate", "file_fsync", "stat", "unlink", "directory_fsync"))
def test_rollback_failures_remain_on_original_exception_and_do_not_abort_descriptor_drain(
    tmp_path, monkeypatch, operation
):
    tx, paths = transaction(tmp_path)
    primary = RuntimeError("original caller failure")
    previous = ValueError("original cause")
    primary.__cause__ = previous
    failure = OSError(errno.EIO, "injected " + operation + " refusal")
    with observed_descriptors(monkeypatch) as (_, _, _, real_fstat):
        tx.__enter__(); tx.install(paths[0], b"private failed output", "test output")
        original_fds = owned(tx)
        file_fd = tx.created[0][1]
        directory_fd = tx.parents[paths[0].parent][0]
        real_fsync, real_stat, real_unlink = os.fsync, os.stat, os.unlink
        def fsync(fd):
            if (operation == "file_fsync" and fd == file_fd) or (operation == "directory_fsync" and fd == directory_fd):
                raise failure
            return real_fsync(fd)
        def stat(path, *args, **kwargs):
            if kwargs.get("dir_fd") == directory_fd and path == paths[0].name:
                raise failure
            return real_stat(path, *args, **kwargs)
        def unlink(path, *args, **kwargs):
            if kwargs.get("dir_fd") == directory_fd and path == paths[0].name:
                raise failure
            return real_unlink(path, *args, **kwargs)
        with monkeypatch.context() as patch:
            if operation == "ftruncate":
                patch.setattr(os, "ftruncate", lambda *args: (_ for _ in ()).throw(failure))
            elif operation in ("file_fsync", "directory_fsync"):
                patch.setattr(os, "fsync", fsync)
            elif operation == "stat":
                patch.setattr(os, "stat", stat)
            else:
                patch.setattr(os, "unlink", unlink)
            tx.__exit__(RuntimeError, primary, None)
        assert primary.__cause__ is previous
        assert primary.transaction_cleanup_errors == (failure,)
        assert_closed(original_fds, real_fstat)
        assert tx.parents == {} and tx.created == []


def test_rollback_preserves_substituted_foreign_leaf_and_erases_original_inode(tmp_path, monkeypatch):
    tx, paths = transaction(tmp_path)
    with observed_descriptors(monkeypatch) as (_, _, _, real_fstat):
        tx.__enter__(); tx.install(paths[0], b"original secret output", "test output")
        original_fds = owned(tx)
        old = tmp_path / "moved-original"
        paths[0].rename(old)
        paths[0].write_bytes(b"foreign replacement must remain")
        primary = ValueError("rollback")
        tx.__exit__(ValueError, primary, None)
        assert paths[0].read_bytes() == b"foreign replacement must remain"
        assert old.read_bytes() == b""
        assert not getattr(primary, "transaction_cleanup_errors", ())
        assert_closed(original_fds, real_fstat)


@pytest.mark.parametrize("action", ("assert", "install", "enter", "exit"))
def test_reentry_during_final_retained_fstat_poison_cannot_be_swallowed(tmp_path, monkeypatch, action):
    tx, paths = transaction(tmp_path, 2)
    with observed_descriptors(monkeypatch) as (_, _, _, real_fstat):
        tx.__enter__(); tx.install(paths[0], b"output", "test output")
        original_fds = owned(tx)
        leaf = tx.created[0][1]
        nested = []
        invoke = {"assert": tx.assert_unchanged, "install": lambda: tx.install(paths[1], b"nested", "test output"),
                  "enter": tx.__enter__, "exit": lambda: tx.__exit__(None, None, None)}[action]
        armed = True
        def fstat(fd):
            nonlocal armed
            info = real_fstat(fd)
            if armed and fd == leaf:
                armed = False
                try:
                    invoke()
                except BaseException as failure:
                    nested.append(failure)
            return info
        with monkeypatch.context() as patch:
            patch.setattr(os, "fstat", fstat)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="invalidated"):
                tx.__exit__(None, None, None)
        assert len(nested) == 1 and isinstance(nested[0], signing.ReleaseManifestSignatureError)
        assert not any(path.exists() for path in paths)
        assert_closed(original_fds, real_fstat)


def test_swallowed_reentrant_exit_during_detached_cleanup_still_refuses_publication(tmp_path, monkeypatch):
    tx, paths = transaction(tmp_path)
    with observed_descriptors(monkeypatch) as (_, _, real_close, real_fstat):
        tx.__enter__(); tx.install(paths[0], b"validated output", "test output")
        original_fds = owned(tx)
        nested = []
        armed = True
        def close(fd):
            nonlocal armed
            real_close(fd)
            if armed and fd in original_fds:
                armed = False
                try:
                    tx.__exit__(None, None, None)
                except BaseException as failure:
                    nested.append(failure)
        with monkeypatch.context() as patch:
            patch.setattr(os, "close", close)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="outputs retained") as failure:
                tx.__exit__(None, None, None)
        assert failure.value.transaction_cleanup_errors == tuple(nested)
        assert failure.value.outputs_retained is True
        assert_closed(original_fds, real_fstat)
        assert paths[0].read_bytes() == b"validated output"


@pytest.mark.parametrize("action", ("assert", "install", "enter", "exit"))
def test_closed_owner_rejects_every_operation_before_any_new_io(tmp_path, monkeypatch, action):
    tx, paths = transaction(tmp_path)
    with tx:
        tx.install(paths[0], b"output", "test output")
    invoke = {"assert": tx.assert_unchanged, "install": lambda: tx.install(paths[0], b"again", "test output"),
              "enter": tx.__enter__, "exit": lambda: tx.__exit__(None, None, None)}[action]
    def forbidden(*args, **kwargs):
        pytest.fail("closed transaction attempted new descriptor acquisition")
    monkeypatch.setattr(os, "open", forbidden)
    with pytest.raises(signing.ReleaseManifestSignatureError):
        invoke()
    assert paths[0].read_bytes() == b"output"


@pytest.mark.parametrize("timing", ("before", "after"))
def test_every_owned_close_failure_is_retained_and_no_number_is_retried(tmp_path, monkeypatch, timing):
    tx, paths = transaction(tmp_path, 2)
    with observed_descriptors(monkeypatch) as (_, _, real_close, _):
        tx.__enter__()
        for path in paths:
            tx.install(path, b"output", "test output")
        original_fds = owned(tx)
        attempts, failures = [], []
        def close(fd):
            if fd not in original_fds:
                return real_close(fd)
            attempts.append(fd)
            if timing == "after":
                real_close(fd)
            failure = OSError(errno.EIO, f"injected {timing} close refusal {fd}")
            failures.append(failure)
            raise failure
        with monkeypatch.context() as patch:
            patch.setattr(os, "close", close)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="outputs retained") as raised:
                tx.__exit__(None, None, None)
        assert len(attempts) == len(original_fds)
        assert set(attempts) == set(original_fds)
        assert raised.value.transaction_cleanup_errors == tuple(failures)
        with pytest.raises(signing.ReleaseManifestSignatureError):
            tx.__exit__(None, None, None)
        assert len(attempts) == len(original_fds)
        assert tx.parents == {} and tx.created == []


def test_failed_entry_preserves_existing_output_and_original_error_despite_drain_failures(tmp_path, monkeypatch):
    tx, paths = transaction(tmp_path, 2)
    paths[1].write_bytes(b"preexisting foreign output")
    with observed_descriptors(monkeypatch) as (observed, _, real_close, real_fstat):
        attempts = []
        def close(fd):
            attempts.append(fd)
            real_close(fd)
            raise OSError(errno.EIO, "failed entry close ambiguity")
        with monkeypatch.context() as patch:
            patch.setattr(os, "close", close)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="already exists") as raised:
                tx.__enter__()
        assert raised.value.transaction_cleanup_errors
        assert len(attempts) == len(observed)
        assert len(attempts) == len(set(attempts))
        assert_closed(attempts, real_fstat)
        assert paths[1].read_bytes() == b"preexisting foreign output"
        assert not paths[0].exists()
        assert tx.parents == {} and tx.created == []


def test_parent_replacement_error_survives_temporary_cleanup_failure_and_foreign_output(tmp_path, monkeypatch):
    tx, paths = transaction(tmp_path)
    with observed_descriptors(monkeypatch) as (_, _, real_close, real_fstat):
        tx.__enter__(); tx.install(paths[0], b"original private output", "test output")
        held = owned(tx)
        old_parent = tmp_path / "original-parent"
        paths[0].parent.rename(old_parent)
        paths[0].parent.mkdir()
        paths[0].write_bytes(b"foreign replacement")
        attempts = []
        def close(fd):
            real_close(fd)
            if fd not in held:
                attempts.append(fd)
                raise OSError(errno.EIO, "temporary lineage close failed")
        with monkeypatch.context() as patch:
            patch.setattr(os, "close", close)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="parent directory was replaced") as raised:
                tx.assert_unchanged()
        assert len(raised.value.transaction_cleanup_errors) == len(attempts)
        tx.__exit__(type(raised.value), raised.value, raised.value.__traceback__)
        assert paths[0].read_bytes() == b"foreign replacement"
        assert not (old_parent / paths[0].name).exists()
        assert_closed(held, real_fstat)


def test_swallowed_install_failure_cannot_commit_an_empty_partial_output(tmp_path, monkeypatch):
    tx, paths = transaction(tmp_path)
    with observed_descriptors(monkeypatch) as (_, _, _, real_fstat):
        tx.__enter__()
        with pytest.raises(TypeError):
            tx.install(paths[0], object(), "unsupported buffer")
        original_fds = owned(tx)
        with pytest.raises(signing.ReleaseManifestSignatureError, match="invalidated"):
            tx.__exit__(None, None, None)
        assert not paths[0].exists()
        assert_closed(original_fds, real_fstat)


@pytest.mark.parametrize("error_type", (RuntimeError, KeyboardInterrupt, SystemExit))
def test_first_metadata_control_flow_failure_keeps_exact_original_and_rolls_back(tmp_path, monkeypatch, error_type):
    import stat
    tx, paths = transaction(tmp_path)
    original = error_type("control-flow interrupted first leaf metadata")
    cause = ValueError("previous cause")
    original.__cause__ = cause
    with observed_descriptors(monkeypatch) as (_, _, _, real_fstat):
        tx.__enter__()
        candidate = []
        def metadata(fd):
            result = real_fstat(fd)
            if stat.S_ISREG(result.st_mode) and not candidate:
                candidate.append(fd)
                raise original
            return result
        with monkeypatch.context() as patch:
            patch.setattr(os, "fstat", metadata)
            with pytest.raises(error_type) as raised:
                try:
                    tx.install(paths[0], b"never published", "test output")
                except BaseException as exc:
                    tx.__exit__(type(exc), exc, exc.__traceback__)
                    raise
        assert raised.value is original and original.__cause__ is cause
        assert candidate
        assert_closed(candidate, real_fstat)
        assert not paths[0].exists()


def test_parent_record_publication_refusal_still_drains_locally_acquired_lineage(tmp_path, monkeypatch):
    tx, _ = transaction(tmp_path)
    original = MemoryError("synthetic refusal before parent-map publication")
    class RefusingMap(dict):
        def __setitem__(self, key, value):
            raise original
    tx.parents = RefusingMap()
    with observed_descriptors(monkeypatch) as (observed, _, _, real_fstat):
        with pytest.raises(MemoryError) as raised:
            tx.__enter__()
        assert raised.value is original
        assert_closed(tuple(observed), real_fstat)
        assert tx.parents == {} and tx.created == []


def test_leaf_record_publication_refusal_rolls_back_same_locally_acquired_inode(tmp_path, monkeypatch):
    tx, paths = transaction(tmp_path)
    original = MemoryError("synthetic refusal before leaf-list publication")
    class RefusingList(list):
        def append(self, value):
            raise original
    with observed_descriptors(monkeypatch) as (observed, _, _, real_fstat):
        tx.__enter__()
        tx.created = RefusingList()
        with pytest.raises(MemoryError) as raised:
            try:
                tx.install(paths[0], b"never published", "test output")
            except BaseException as exc:
                tx.__exit__(type(exc), exc, exc.__traceback__)
                raise
        assert raised.value is original
        assert_closed(tuple(observed), real_fstat)
        assert not paths[0].exists()
        assert tx.parents == {} and tx.created == []


def test_cleanup_failure_without_created_outputs_does_not_claim_outputs_were_retained(tmp_path, monkeypatch):
    tx, paths = transaction(tmp_path)
    with observed_descriptors(monkeypatch) as (_, _, real_close, real_fstat):
        tx.__enter__()
        originals = owned(tx)
        armed = True
        def close(fd):
            nonlocal armed
            real_close(fd)
            if armed and fd in originals:
                armed = False
                raise OSError(errno.EIO, "empty transaction close ambiguity")
        with monkeypatch.context() as patch:
            patch.setattr(os, "close", close)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="cleanup failed") as raised:
                tx.__exit__(None, None, None)
        assert raised.value.outputs_retained is False
        assert "outputs retained" not in str(raised.value)
        assert not paths[0].exists()
        assert_closed(originals, real_fstat)


@pytest.mark.parametrize("role", ("parent", "leaf"))
@pytest.mark.parametrize("published", (False, True))
@pytest.mark.parametrize("ambiguous_close", (False, True))
def test_interrupted_record_publication_has_exactly_one_cleanup_owner(
    tmp_path, monkeypatch, role, published, ambiguous_close
):
    tx, paths = transaction(tmp_path)
    original = RuntimeError("interrupted bookkeeping publication")
    previous = LookupError("preexisting acquisition cause")
    original.__cause__ = previous
    sentinel = tmp_path / "sentinel"
    sentinel.write_bytes(b"another task owns this descriptor and its bytes")
    candidates, replacements, attempts = [], [], []
    class InterruptedMap(dict):
        def __setitem__(self, key, value):
            candidates.append(value[2][-1])
            if published:
                super().__setitem__(key, value)
            raise original
    class InterruptedList(list):
        def append(self, value):
            candidates.append(value[1])
            if published:
                super().append(value)
            raise original
    with observed_descriptors(monkeypatch) as (observed, real_open, real_close, real_fstat):
        if role == "parent":
            tx.parents = InterruptedMap()
        else:
            tx.__enter__()
            tx.created = InterruptedList()
        def close(fd):
            # Number reuse during earlier path inspection is unrelated: only
            # count attempts once this original descriptor has been acquired.
            if not candidates or fd != candidates[0]:
                return real_close(fd)
            attempts.append(fd)
            real_close(fd)
            if not replacements:
                replacement = real_open(sentinel, os.O_RDWR)
                replacements.append(replacement)
                assert replacement == fd
                if ambiguous_close:
                    raise OSError(errno.EIO, "original descriptor closed before error")
        try:
            with monkeypatch.context() as patch:
                patch.setattr(os, "close", close)
                with pytest.raises(RuntimeError) as raised:
                    if role == "parent":
                        tx.__enter__()
                    else:
                        try:
                            tx.install(paths[0], b"not published", "test output")
                        except BaseException as exc:
                            tx.__exit__(type(exc), exc, exc.__traceback__)
                            raise
            assert raised.value is original and original.__cause__ is previous
            assert len(candidates) == 1 and attempts == candidates
            assert len(replacements) == 1
            assert real_fstat(replacements[0]).st_ino == sentinel.stat().st_ino
            assert sentinel.read_bytes() == b"another task owns this descriptor and its bytes"
            assert len(getattr(original, "transaction_cleanup_errors", ())) == int(ambiguous_close)
            assert tx.parents == {} and tx.created == []
            assert not paths[0].exists()
            assert_closed(tuple(fd for fd in observed if fd not in replacements), real_fstat)
            with pytest.raises(signing.ReleaseManifestSignatureError, match="not open"):
                tx.__exit__(None, None, None)
            assert real_fstat(replacements[0]).st_ino == sentinel.stat().st_ino
        finally:
            for fd in replacements:
                try:
                    info = real_fstat(fd)
                except OSError:
                    continue
                if info.st_ino == sentinel.stat().st_ino:
                    real_close(fd)


def test_builtin_leaf_publication_interruption_retains_one_descriptor_owner(tmp_path, monkeypatch):
    import dis
    import sys
    tx, paths = transaction(tmp_path)
    original = KeyboardInterrupt("interruption after real builtin append")
    previous = ValueError("previous control-flow cause")
    original.__cause__ = previous
    sentinel = tmp_path / "sentinel"
    sentinel.write_bytes(b"unrelated descriptor must remain intact")
    candidates, replacements, attempts, boundary = [], [], [], []
    with observed_descriptors(monkeypatch) as (observed, real_open, real_close, real_fstat):
        tx.__enter__()
        assert type(tx.created) is list
        code = tx._install.__func__.__code__
        instructions = {row.offset: row.opname for row in dis.get_instructions(code)}
        def trace(frame, event, _arg):
            if frame.f_code is code:
                frame.f_trace_opcodes = True
                if event == "opcode" and not boundary and tx.created:
                    candidates.append(tx.created[0][1])
                    boundary.append(instructions[frame.f_lasti])
                    raise original
            return trace
        def close(fd):
            if not candidates or fd != candidates[0]:
                return real_close(fd)
            attempts.append(fd)
            real_close(fd)
            if not replacements:
                replacement = real_open(sentinel, os.O_RDWR)
                assert replacement == fd
                replacements.append(replacement)
        prior_trace = sys.gettrace()
        # CPython 3.12 enables opcode instrumentation only on the next tracing
        # activation after observing f_trace_opcodes. Prime tracing on an inert
        # function, not on the transaction or a substitute ownership algorithm.
        def prime():
            return None
        def prime_trace(frame, _event, _arg):
            if frame.f_code is prime.__code__:
                frame.f_trace_opcodes = True
            return prime_trace
        try:
            sys.settrace(prime_trace)
            prime()
        finally:
            sys.settrace(prior_trace)
        try:
            with monkeypatch.context() as patch:
                patch.setattr(os, "close", close)
                try:
                    sys.settrace(trace)
                    with pytest.raises(KeyboardInterrupt) as raised:
                        tx.install(paths[0], b"never written", "test output")
                finally:
                    sys.settrace(prior_trace)
                tx.__exit__(type(raised.value), raised.value, raised.value.__traceback__)
            assert raised.value is original and original.__cause__ is previous
            # CPython has completed builtin append but remains within its local
            # exception region at this actual opcode; no replacement list is used.
            assert boundary == ["POP_TOP"] and len(candidates) == 1
            assert attempts == candidates and len(replacements) == 1
            assert real_fstat(replacements[0]).st_ino == sentinel.stat().st_ino
            assert sentinel.read_bytes() == b"unrelated descriptor must remain intact"
            assert not paths[0].exists()
            assert tx.created == [] and tx.parents == {}
            assert_closed(tuple(fd for fd in observed if fd not in replacements), real_fstat)
        finally:
            sys.settrace(prior_trace)
            for fd in replacements:
                try:
                    info = real_fstat(fd)
                except OSError:
                    continue
                if info.st_ino == sentinel.stat().st_ino:
                    real_close(fd)
