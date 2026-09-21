"""Original execution ZIP codec controls; no execution or approval substitutes."""
from __future__ import annotations

import io
from pathlib import Path
import stat
import struct
import sys
import warnings
import zipfile

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_archive as codec


def _zip(rows, *, mode=stat.S_IFREG | 0o600, compression=zipfile.ZIP_STORED,
         timestamp=(1980, 1, 1, 0, 0, 0), extra=b"", comment=b"", system=3):
    output = io.BytesIO()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)
        with zipfile.ZipFile(output, "w", compression=compression) as archive:
            archive.comment = comment
            for name, raw in rows:
                entry = zipfile.ZipInfo(name, timestamp)
                entry.create_system = system
                entry.external_attr = mode << 16
                entry.compress_type = compression
                entry.extra = extra
                archive.writestr(entry, raw)
    return output.getvalue()


def test_canonical_original_roundtrip_empty_metadata_unicode_and_order():
    members = {"tools/qualify.py": b"source\n", "environment/REQUESTED": b"",
               "snapshot/\u03bb.txt": b"fixture", "logs/execute.stdout": b"actual log\n"}
    raw = codec.execution_archive(members)
    assert codec.archive_members(raw) == members
    assert raw == codec.execution_archive(dict(reversed(tuple(members.items()))))
    assert raw == _zip(sorted(members.items()))


@pytest.mark.parametrize("alter", (
    {"compression": zipfile.ZIP_DEFLATED}, {"timestamp": (2026, 1, 1, 0, 0, 0)},
    {"mode": stat.S_IFLNK | 0o600}, {"mode": stat.S_IFREG | 0o644},
    {"mode": stat.S_IFREG | 0o700}, {"system": 0},
    {"extra": b"\xca\xfe\x00\x00"}, {"comment": b"unowned"},
))
def test_canonical_metadata_cannot_be_replaced(alter):
    with pytest.raises(codec.ArtifactError):
        codec.archive_members(_zip([("original", b"bytes")], **alter))


@pytest.mark.parametrize("name", ("../escape", "/absolute", "a//b", "a\\b", "a/./b",
                                      "a\x00suffix", "directory/", "a\nb", "a" * 1025))
def test_unsafe_names_are_refused_without_extraction(name, tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    # ZipInfo sanitizes a NUL before writing, so mutate both original headers
    # to exercise a genuinely malformed archive rather than a safe renamed one.
    raw = (_zip([(name.replace("\x00", "X"), b"bytes")]).replace(
        name.replace("\x00", "X").encode(), name.encode()) if "\x00" in name
        else _zip([(name, b"bytes")]))
    with pytest.raises(codec.ArtifactError):
        codec.archive_members(raw)
    assert not list(tmp_path.iterdir())


@pytest.mark.parametrize("rows", ([("a", b"first"), ("a", b"second")],
                                    [("z", b"last"), ("a", b"first")], []))
def test_duplicate_reordered_and_empty_inventories_reject(rows):
    with pytest.raises(codec.ArtifactError): codec.archive_members(_zip(rows))


@pytest.mark.parametrize("mutation", ("prepend", "append", "truncate", "crc", "member_comment"))
def test_original_envelope_and_payload_integrity(mutation):
    raw = codec.execution_archive({"original": b"payload"})
    if mutation == "prepend": raw = b"prefix" + raw
    elif mutation == "append": raw += b"tail"
    elif mutation == "truncate": raw = raw[:-1]
    elif mutation == "crc": raw = raw.replace(b"payload", b"PAYLOAD", 1)
    else:
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as archive:
            entry = zipfile.ZipInfo("original", (1980, 1, 1, 0, 0, 0))
            entry.create_system = 3; entry.external_attr = (stat.S_IFREG | 0o600) << 16
            entry.comment = b"extra comment"
            archive.writestr(entry, b"payload")
        raw = output.getvalue()
    with pytest.raises(codec.ArtifactError): codec.archive_members(raw)


@pytest.mark.parametrize("bound", ("MAX_MEMBERS", "MAX_MEMBER_BYTES", "MAX_PAYLOAD_BYTES", "MAX_ARCHIVE_BYTES"))
def test_reader_refuses_exact_limit_overflow_before_unbounded_reads(monkeypatch, bound):
    members = {"a": b"1234", "b": b"56"}
    raw = codec.execution_archive(members)
    limit = {"MAX_MEMBERS": 2, "MAX_MEMBER_BYTES": 4, "MAX_PAYLOAD_BYTES": 6,
             "MAX_ARCHIVE_BYTES": len(raw)}[bound]
    monkeypatch.setattr(codec, bound, limit)
    assert codec.archive_members(raw) == members
    monkeypatch.setattr(codec, bound, limit - 1)
    with pytest.raises(codec.ArtifactError): codec.archive_members(raw)
    with pytest.raises(codec.ArtifactError): codec.execution_archive(members)


@pytest.mark.parametrize("raw", (b"", bytearray(b"not immutable"), None, b"not a ZIP"))
def test_reader_requires_original_nonempty_immutable_bytes(raw):
    with pytest.raises(codec.ArtifactError): codec.archive_members(raw)


@pytest.mark.parametrize("mutation", (
    "short_end", "end_signature", "split_disk", "central_disk", "unequal_counts",
    "zero_count", "over_count", "underdeclared_count", "overdeclared_count",
    "end_comment", "central_extent", "central_signature", "oversized_name",
    "empty_name", "central_extra", "central_comment", "entry_disk", "central_tail",
))
def test_directory_refusal_precedes_any_zipfile_inventory(monkeypatch, mutation):
    # Long valid names leave enough bytes to test an overdeclared count without
    # merely failing the minimum fixed-header byte check.
    raw = bytearray(_zip([("a" * 100, b"first"), ("b" * 100, b"second")]))
    end = len(raw) - 22
    central = struct.unpack_from("<I", raw, end + 16)[0]
    if mutation == "short_end": raw = raw[:10]
    elif mutation == "end_signature": raw[end:end + 4] = b"NOPE"
    elif mutation == "split_disk": struct.pack_into("<H", raw, end + 4, 1)
    elif mutation == "central_disk": struct.pack_into("<H", raw, end + 6, 1)
    elif mutation == "unequal_counts": struct.pack_into("<H", raw, end + 8, 1)
    elif mutation == "zero_count": struct.pack_into("<HH", raw, end + 8, 0, 0)
    elif mutation == "over_count":
        struct.pack_into("<HH", raw, end + 8, codec.MAX_MEMBERS + 1, codec.MAX_MEMBERS + 1)
    elif mutation == "underdeclared_count": struct.pack_into("<HH", raw, end + 8, 1, 1)
    elif mutation == "overdeclared_count": struct.pack_into("<HH", raw, end + 8, 3, 3)
    elif mutation == "end_comment": struct.pack_into("<H", raw, end + 20, 1)
    elif mutation == "central_extent": struct.pack_into("<I", raw, end + 16, central + 1)
    elif mutation == "central_signature": raw[central:central + 4] = b"NOPE"
    elif mutation == "oversized_name": struct.pack_into("<H", raw, central + 28, 1025)
    elif mutation == "empty_name": struct.pack_into("<H", raw, central + 28, 0)
    elif mutation == "central_extra": struct.pack_into("<H", raw, central + 30, 1)
    elif mutation == "central_comment": struct.pack_into("<H", raw, central + 32, 1)
    elif mutation == "entry_disk": struct.pack_into("<H", raw, central + 34, 1)
    else:
        raw[end:end] = b"unowned"
        struct.pack_into("<I", raw, end + 7 + 12, len(raw) - 22 - central)

    def forbidden(*_args, **_kwargs):
        raise AssertionError("directory refusal must precede ZipFile allocation")

    monkeypatch.setattr(codec.zipfile, "ZipFile", forbidden)
    with pytest.raises(codec.ArtifactError): codec.archive_members(bytes(raw))


def test_directory_aggregate_byte_bound_precedes_zipfile(monkeypatch):
    monkeypatch.setattr(codec, "MAX_MEMBERS", 2)
    oversized = b"x" * (2 * (46 + 1024) + 1)
    raw = oversized + struct.pack("<4s4H2IH", b"PK\x05\x06", 0, 0, 1, 1, len(oversized), 0, 0)

    def forbidden(*_args, **_kwargs):
        raise AssertionError("directory byte refusal must precede ZipFile allocation")

    monkeypatch.setattr(codec.zipfile, "ZipFile", forbidden)
    with pytest.raises(codec.ArtifactError) as refused:
        codec.archive_members(raw)
    assert "central-directory byte bound" in str(refused.value.__cause__)


def test_exact_directory_count_and_name_byte_limits_reach_original_parser(monkeypatch):
    monkeypatch.setattr(codec, "MAX_MEMBERS", 2)
    suffix = "/".join(["a" * 250] * 4 + ["z" * 20])
    assert len(suffix.encode()) == 1024
    members = {suffix: b"first", "b" + suffix[1:]: b"second"}
    raw = codec.execution_archive(members)
    calls = []
    original = codec.zipfile.ZipFile

    def observed(*args, **kwargs):
        calls.append(True)
        return original(*args, **kwargs)

    monkeypatch.setattr(codec.zipfile, "ZipFile", observed)
    assert codec.archive_members(raw) == members
    assert calls, "accepted directory must still reach the one existing ZIP parser"
