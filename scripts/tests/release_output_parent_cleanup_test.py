"""Real descriptor controls for partial release-output lineage acquisition."""
from __future__ import annotations

import errno
import os

import pytest

from scripts import release_manifest_signing as signing


@pytest.mark.parametrize("operation", ("open", "fstat"))
@pytest.mark.parametrize("error_type", (OSError, NotImplementedError, RuntimeError, KeyboardInterrupt))
@pytest.mark.parametrize("cleanup", ("clean", "before", "after_reuse", "interrupt"))
def test_partial_acquisition_drains_once_preserving_both_failures(
    tmp_path, monkeypatch, operation, error_type, cleanup
):
    root = tmp_path / "output"
    root.mkdir()
    real_open, real_close, real_fstat = os.open, os.close, os.fstat
    acquisition = error_type("injected partial ancestry acquisition failure")
    previous_cause = ValueError("original explicit acquisition cause")
    acquisition.__cause__ = previous_cause
    cleanup_failure = (KeyboardInterrupt("injected cleanup interruption")
                       if cleanup == "interrupt" else OSError("injected ambiguous close"))
    acquired, attempts, replacements, cleanup_errors = [], [], [], []
    live_originals = set()
    calls = {"open": 0, "fstat": 0}

    def fail_if_selected(name):
        calls[name] += 1
        if name == operation and calls[name] == 3:
            raise acquisition

    def observed_open(*args, **kwargs):
        fail_if_selected("open")
        descriptor = real_open(*args, **kwargs)
        acquired.append(descriptor)
        live_originals.add(descriptor)
        return descriptor

    def observed_fstat(descriptor):
        fail_if_selected("fstat")
        return real_fstat(descriptor)

    def observed_close(descriptor):
        attempts.append(descriptor)
        # Fault every attempt: draining must not stop at the first or second.
        if cleanup in ("before", "interrupt"):
            cleanup_errors.append(cleanup_failure)
            raise cleanup_failure
        real_close(descriptor)
        live_originals.remove(descriptor)
        if cleanup == "after_reuse":
            replacement = real_open(os.devnull, os.O_RDONLY)
            replacements.append(replacement)
            assert replacement == descriptor
            cleanup_errors.append(cleanup_failure)
            raise cleanup_failure

    try:
        with monkeypatch.context() as patch:
            patch.setattr(signing.os, "open", observed_open)
            patch.setattr(signing.os, "fstat", observed_fstat)
            patch.setattr(signing.os, "close", observed_close)
            with pytest.raises(BaseException) as raised:
                signing._open_release_output_parent(root)
        assert attempts == list(reversed(acquired))
        assert len(set(attempts)) == len(attempts)
        assert len(acquired) == (2 if operation == "open" else 3)
        if error_type in (OSError, NotImplementedError):
            assert type(raised.value) is signing.ReleaseManifestSignatureError
            assert raised.value.__cause__ is acquisition
            assert getattr(raised.value, "cleanup_errors", ()) == tuple(cleanup_errors)
            assert ("cleanup attempt(s) failed" in str(raised.value)) == bool(cleanup_errors)
        else:
            assert raised.value is acquisition
            assert getattr(raised.value, "cleanup_errors", ()) == tuple(cleanup_errors)
        assert acquisition.__cause__ is previous_cause
        for descriptor in replacements:
            real_fstat(descriptor)  # A failed close's reused descriptor was never retried.
        if cleanup == "clean":
            for descriptor in acquired:
                with pytest.raises(OSError) as closed:
                    real_fstat(descriptor)
                assert closed.value.errno == errno.EBADF
        else:
            assert len(cleanup_errors) == len(acquired)
    finally:
        # Test-owned handles left by intentionally ambiguous failures only.
        for descriptor in live_originals | set(replacements):
            real_close(descriptor)


@pytest.mark.parametrize("operation", ("open", "fstat"))
def test_first_acquisition_failure_closes_only_handles_actually_acquired(tmp_path, monkeypatch, operation):
    real_open, real_close, real_fstat = os.open, os.close, os.fstat
    acquired, attempts = [], []
    original = OSError("first acquisition failed")
    def observed_open(*args, **kwargs):
        if operation == "open":
            raise original
        descriptor = real_open(*args, **kwargs)
        acquired.append(descriptor)
        return descriptor
    def observed_fstat(descriptor):
        raise original
    def observed_close(descriptor):
        attempts.append(descriptor)
        real_close(descriptor)
    with monkeypatch.context() as patch:
        patch.setattr(signing.os, "open", observed_open)
        patch.setattr(signing.os, "fstat", observed_fstat)
        patch.setattr(signing.os, "close", observed_close)
        with pytest.raises(signing.ReleaseManifestSignatureError) as raised:
            signing._open_release_output_parent(tmp_path)
    assert raised.value.__cause__ is original
    assert attempts == acquired
    assert len(acquired) == (0 if operation == "open" else 1)
    for descriptor in acquired:
        with pytest.raises(OSError):
            real_fstat(descriptor)


def test_success_transfers_complete_open_lineage_to_caller(tmp_path, monkeypatch):
    root = tmp_path / "a" / "b"
    root.mkdir(parents=True)
    attempts = []
    real_close = os.close
    with monkeypatch.context() as patch:
        patch.setattr(signing.os, "close", lambda fd: attempts.append(fd))
        descriptor, lineage, descriptors = signing._open_release_output_parent(root)
    try:
        assert not attempts
        assert descriptor == descriptors[-1]
        assert len(descriptors) == len(root.parts)
        assert lineage == tuple((os.fstat(fd).st_dev, os.fstat(fd).st_ino) for fd in descriptors)
        assert lineage[-1] == (root.stat().st_dev, root.stat().st_ino)
    finally:
        for fd in reversed(descriptors):
            real_close(fd)


def test_symlink_refusal_drains_real_partial_lineage(tmp_path, monkeypatch):
    destination = tmp_path / "destination"
    destination.mkdir()
    link = tmp_path / "link"
    link.symlink_to(destination, target_is_directory=True)
    real_open, real_close = os.open, os.close
    acquired, attempts = [], []
    def observed_open(*args, **kwargs):
        descriptor = real_open(*args, **kwargs)
        acquired.append(descriptor)
        return descriptor
    def observed_close(descriptor):
        attempts.append(descriptor)
        real_close(descriptor)
    with monkeypatch.context() as patch:
        patch.setattr(signing.os, "open", observed_open)
        patch.setattr(signing.os, "close", observed_close)
        with pytest.raises(signing.ReleaseManifestSignatureError) as raised:
            signing._open_release_output_parent(link)
    assert isinstance(raised.value.__cause__, OSError)
    assert attempts == list(reversed(acquired))
    for descriptor in acquired:
        with pytest.raises(OSError):
            os.fstat(descriptor)


def test_unsupported_host_refuses_before_any_acquisition(tmp_path, monkeypatch):
    monkeypatch.setattr(signing, "RELEASE_OUTPUT_DIR_FD_SUPPORTED", False)
    def forbidden(*args, **kwargs):
        pytest.fail("unsupported host attempted descriptor acquisition")
    monkeypatch.setattr(signing.os, "open", forbidden)
    with pytest.raises(signing.ReleaseManifestSignatureError, match="unavailable"):
        signing._open_release_output_parent(tmp_path)
