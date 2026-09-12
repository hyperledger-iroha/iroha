"""Exact executable and process identity tests for the local resource adapter."""
from dataclasses import replace
import ctypes
import hashlib
import importlib.util
import os
from pathlib import Path
import struct
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("resource_process_under_test", ROOT / "scripts/nexus/resource_process.py")
process = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = process
SPEC.loader.exec_module(process)
UUID = bytes(range(1, 17))


def thin(uuid=UUID, order="<"):
    return struct.pack(order + "8I", 0xFEEDFACF, 0x100000C, 0, 2, 1, 24, 0, 0) + struct.pack(order + "II", 0x1B, 24) + uuid


def image_path(tmp_path, raw=None):
    raw = thin() if raw is None else raw
    path = tmp_path / "iroha3d"
    path.write_bytes(raw)
    return path, hashlib.sha256(raw).hexdigest()


@pytest.mark.parametrize("order", ["<", ">"])
def test_exact_image_pin_and_close(tmp_path, order):
    path, digest = image_path(tmp_path, thin(order=order))
    with process.ExecutableImage(path, digest) as image:
        assert image.uuids == frozenset({UUID})
        assert image.sha256 == digest
        image.validate()
        fd = image.fd
    assert image.fd == -1
    with pytest.raises(OSError):
        os.fstat(fd)
    image.close()
    with pytest.raises(process.ProcessObservationError, match="closed"):
        image.validate()


def test_universal_image_pins_each_nonoverlapping_slice(tmp_path):
    second = bytes(reversed(UUID))
    raw = (struct.pack(">II", 0xCAFEBABE, 2)
           + struct.pack(">5I", 0x100000C, 0, 64, 56, 3)
           + struct.pack(">5I", 0x1000007, 0, 128, 56, 3))
    raw += bytes(64 - len(raw)) + thin() + bytes(8) + thin(second)
    path, digest = image_path(tmp_path, raw)
    with process.ExecutableImage(path, digest) as image:
        assert image.uuids == frozenset({UUID, second})


@pytest.mark.parametrize("mutation", ["empty", "short", "magic", "commands", "bytes", "size", "uuid_missing", "uuid_zero", "duplicate", "tail", "fat_count", "fat_overlap", "fat_alignment"])
def test_malformed_images_fail_before_pin(tmp_path, mutation):
    raw = bytearray(thin())
    if mutation == "empty": raw = bytearray()
    if mutation == "short": raw = raw[:12]
    if mutation == "magic": raw[:4] = b"ELF!"
    if mutation == "commands": struct.pack_into("<I", raw, 16, 65537)
    if mutation == "bytes": struct.pack_into("<I", raw, 20, process.MAX_LOAD_COMMAND_BYTES + 1)
    if mutation == "size": struct.pack_into("<I", raw, 36, 16)
    if mutation == "uuid_missing": struct.pack_into("<I", raw, 32, 0x10)
    if mutation == "uuid_zero": raw[-16:] = bytes(16)
    if mutation == "duplicate":
        raw.extend(raw[32:]); struct.pack_into("<II", raw, 16, 2, 48)
    if mutation == "tail":
        raw.extend(bytes(8)); struct.pack_into("<I", raw, 20, 32)
    if mutation == "fat_count": raw = bytearray(struct.pack(">II", 0xCAFEBABE, 33))
    if mutation in {"fat_overlap", "fat_alignment"}:
        raw = bytearray(struct.pack(">II", 0xCAFEBABE, 2)
                        + struct.pack(">5I", 0, 0, 64, 56, 3)
                        + struct.pack(">5I", 0, 0, 64, 56, 32 if mutation == "fat_alignment" else 3))
        raw.extend(bytes(64 - len(raw)) + thin())
    path, digest = image_path(tmp_path, raw)
    with pytest.raises(process.ProcessObservationError):
        process.ExecutableImage(path, digest)


@pytest.mark.parametrize("mutation", ["replace", "write", "symlink", "unlink"])
def test_pinned_descriptor_and_name_cannot_change(tmp_path, mutation):
    path, digest = image_path(tmp_path)
    with process.ExecutableImage(path, digest) as image:
        if mutation == "replace":
            alternate = tmp_path / "replacement"
            alternate.write_bytes(thin())
            alternate.replace(path)
        if mutation == "write": path.write_bytes(thin(bytes(reversed(UUID))))
        if mutation == "symlink":
            target = tmp_path / "same"
            path.rename(target)
            path.symlink_to(target)
        if mutation == "unlink": path.unlink()
        with pytest.raises((process.ProcessObservationError, FileNotFoundError)):
            image.validate()


@pytest.mark.parametrize("digest", ["0" * 64, "A" * 64, "0" * 63, "0" * 65])
def test_digest_must_match_pinned_image(tmp_path, digest):
    path, _ = image_path(tmp_path)
    with pytest.raises(process.ProcessObservationError):
        process.ExecutableImage(path, digest)


def identity(pid=123):
    return process.ProcessIdentity(pid, os.getuid(), 100, 5, 500, UUID.hex(), "1" * 64)


@pytest.mark.parametrize("field,value", [("pid", 124), ("uid", 99), ("start_seconds", 101), ("start_microseconds", 6), ("start_abstime", 501), ("image_uuid", "0" * 32), ("executable_sha256", "2" * 64)])
def test_lifetime_or_image_change_never_rebinds(field, value):
    baseline = identity()
    rows = iter([process.ProcessSample(baseline, 10), process.ProcessSample(replace(baseline, **{field: value}), 20)])
    reader = SimpleNamespace(sample=lambda *_: next(rows))
    pinned = process.PinnedProcess("validator-0", 123, None, reader)
    with pytest.raises(process.ProcessObservationError, match="restarted or changed"):
        pinned.sample()
    assert pinned.identity == baseline


def test_peer_scope_requires_every_unique_validator_and_checked_total():
    def peer(index, rss=100):
        return SimpleNamespace(peer_id=str(index), pid=index + 2, sample=lambda: process.ProcessSample(identity(index + 2), rss))
    peers = tuple(peer(i) for i in range(4))
    assert len(process.sample_peers(peers)) == 4
    assert sum(row.rss_bytes for row in process.sample_peers(peers)) == 400
    for invalid in [peers[:3], (peers[0],) * 4, tuple(peer(i) for i in range(65)), tuple(peer(i, process.MAX_EXACT_INTEGER) for i in range(4))]:
        with pytest.raises(process.ProcessObservationError):
            process.sample_peers(invalid)
    peers[3].sample = lambda: (_ for _ in ()).throw(process.ProcessObservationError("exited"))
    with pytest.raises(process.ProcessObservationError, match="exited"):
        process.sample_peers(peers)


def test_unsupported_platform_is_explicit(monkeypatch):
    monkeypatch.setattr(process.sys, "platform", "unsupported")
    with pytest.raises(process.ProcessObservationError, match="requires Darwin"):
        process.DarwinProcessReader()


@pytest.mark.parametrize("label", ["", "x" * 129, "validator/0", "validator\n0", None])
def test_peer_label_is_bounded_public_identity(label):
    reader = SimpleNamespace(sample=lambda *_: pytest.fail("invalid label reached process reader"))
    with pytest.raises(process.ProcessObservationError):
        process.PinnedProcess(label, 123, None, reader)


@pytest.mark.parametrize("failure", ["short", "pid", "uid", "ruid", "seconds", "microseconds"])
def test_native_identity_unavailable_or_foreign_is_rejected(failure):
    reader = process.DarwinProcessReader.__new__(process.DarwinProcessReader)
    def identity_info(_pid, _flavor, _argument, pointer, size):
        info = pointer._obj
        info.pid = 124 if failure == "pid" else 123
        info.uid = os.geteuid() + (1 if failure == "uid" else 0)
        info.ruid = os.getuid() + (1 if failure == "ruid" else 0)
        info.start_sec = 0 if failure == "seconds" else 50
        info.start_usec = 1_000_000 if failure == "microseconds" else 1
        return size - 1 if failure == "short" else size
    reader.lib = SimpleNamespace(proc_pidinfo=identity_info)
    with pytest.raises(process.ProcessObservationError):
        reader._identity(123)


@pytest.mark.skipif(sys.platform != "darwin", reason="actual Darwin kernel observation")
def test_actual_current_process_rss_and_loaded_uuid_are_bound():
    reader = process.DarwinProcessReader()
    # Homebrew's sys.executable is a launcher which execs a different image.
    # This native smoke pins the kernel-reported test image; real trials supply
    # their reviewed daemon artifact path and digest before launching validators.
    path_buffer = ctypes.create_string_buffer(4096)
    count = reader.lib.proc_pidpath(os.getpid(), path_buffer, len(path_buffer))
    assert 0 < count < len(path_buffer)
    path = Path(os.fsdecode(path_buffer.raw[:count])).resolve(strict=True)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    with process.ExecutableImage(path, digest) as image:
        pinned = process.PinnedProcess("test-process", os.getpid(), image, reader)
        first = pinned.sample()
        second = pinned.sample()
        assert first.identity == second.identity == pinned.identity
        assert first.identity.pid == os.getpid()
        assert bytes.fromhex(first.identity.image_uuid) in image.uuids
        assert 0 < first.rss_bytes <= process.MAX_EXACT_INTEGER
        assert 0 < second.rss_bytes <= process.MAX_EXACT_INTEGER
    launcher = Path(sys.executable).resolve(strict=True)
    if launcher != path:
        with process.ExecutableImage(launcher, hashlib.sha256(launcher.read_bytes()).hexdigest()) as wrong_image:
            with pytest.raises(process.ProcessObservationError, match="differs from the pinned"):
                reader.sample(os.getpid(), wrong_image)


@pytest.mark.parametrize("failure", ["none", "pid_reuse", "uuid", "path", "exited", "zero_rss", "large_rss", "missing_usage", "truncated_path"])
def test_native_reader_rechecks_each_identity_and_memory_boundary(tmp_path, failure):
    path, digest = image_path(tmp_path)
    reader = process.DarwinProcessReader.__new__(process.DarwinProcessReader)
    before = SimpleNamespace(pid=123, uid=os.geteuid(), ruid=os.getuid(), start_sec=50, start_usec=1)
    after = SimpleNamespace(**vars(before))
    if failure == "pid_reuse": after.start_usec = 2
    identities = iter([before, after])
    reader._identity = lambda _: next(identities)

    def rusage(_pid, _version, pointer):
        usage = pointer._obj
        usage.uuid[:] = bytes(reversed(UUID)) if failure == "uuid" else UUID
        usage.rss = 0 if failure == "zero_rss" else process.MAX_EXACT_INTEGER + 1 if failure == "large_rss" else 4096
        usage.start_abstime = 4
        usage.exit_abstime = 1 if failure == "exited" else 0
        return -1 if failure == "missing_usage" else 0

    def pidpath(_pid, buffer, _size):
        target = tmp_path if failure == "path" else path
        data = os.fsencode(target)
        buffer.value = data
        return len(buffer) if failure == "truncated_path" else len(data)

    reader.lib = SimpleNamespace(proc_pid_rusage=rusage, proc_pidpath=pidpath)
    with process.ExecutableImage(path, digest) as image:
        if failure == "none":
            sample = reader.sample(123, image)
            assert sample.rss_bytes == 4096
            assert sample.identity.start_microseconds == 1
        else:
            with pytest.raises(process.ProcessObservationError):
                reader.sample(123, image)
