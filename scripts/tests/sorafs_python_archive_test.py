"""Original execution ZIP codec controls; no execution or approval substitutes."""
from __future__ import annotations

import io
from pathlib import Path
import stat
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
