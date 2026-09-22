"""Exact fixed and focused admission bounds using inert streaming fixtures."""
from __future__ import annotations

from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_archive as archive
from sorafs_javascript_archive_fixtures import (
    BLOCK, compress, compressed_iter, header, member, pax_member, pax_records, tar,
)


def reject(raw):
    with pytest.raises(archive.ArchiveError):
        archive.parse_npm_archive(raw)


def test_bound_contract_is_fixed():
    assert archive.MAX_ARCHIVE_BYTES == 64 * 1024 * 1024
    assert archive.MAX_TAR_BYTES == 256 * 1024 * 1024
    assert archive.MAX_MEMBER_BYTES == 32 * 1024 * 1024
    assert archive.MAX_MEMBERS == 20000
    assert archive.MAX_NAME_BYTES == 1024
    assert archive.MAX_PAX_BYTES == 16 * 1024
    assert archive.MAX_PATH_NODES == 80000
    assert archive.MAX_PATH_BYTES == 16 * 1024 * 1024


def test_exact_compressed_byte_admission_and_one_below(monkeypatch):
    raw = compress(tar())
    monkeypatch.setattr(archive, "MAX_ARCHIVE_BYTES", len(raw))
    assert archive.parse_npm_archive(raw).files() == {"a.js": b"x"}
    monkeypatch.setattr(archive, "MAX_ARCHIVE_BYTES", len(raw) - 1)
    reject(raw)


def test_actual_member_byte_limit_and_declared_overflow_before_payload(monkeypatch):
    size = archive.MAX_MEMBER_BYTES
    chunk = b"x" * (64 * 1024)
    raw = compressed_iter(iter([header(size=size)] + [chunk] * (size // len(chunk)) + [bytes(1024)]))
    result = archive.parse_npm_archive(raw)
    assert len(result.members[0].content) == size
    assert result.members[0].content[:1] == result.members[0].content[-1:] == b"x"
    calls = []
    original = archive._GzipReader.exact
    def observed(self, count):
        calls.append(count)
        return original(self, count)
    monkeypatch.setattr(archive._GzipReader, "exact", observed)
    reject(compress(header(size=size + 1)))
    assert calls == [512]


def test_padded_payload_extent_admitted_before_payload_read(monkeypatch):
    # Header consumes 512; 513 data bytes require 1024 more incl padding.
    monkeypatch.setattr(archive, "MAX_TAR_BYTES", 512 + 1023)
    calls = []
    original = archive._GzipReader.exact
    def observed(self, count):
        calls.append(count)
        return original(self, count)
    monkeypatch.setattr(archive._GzipReader, "exact", observed)
    reject(compress(header(size=513)))
    assert calls == [512]


def padded_stream(size):
    first = member()
    assert size >= len(first) + 1024 and size % 512 == 0
    def chunks():
        yield first
        remaining = size - len(first)
        zero = bytes(64 * 1024)
        while remaining:
            length = min(len(zero), remaining)
            yield zero[:length]
            remaining -= length
    return compressed_iter(chunks())


def test_actual_total_tar_byte_limit_counts_headers_and_zero_padding():
    assert archive.parse_npm_archive(padded_stream(archive.MAX_TAR_BYTES)).files() == {"a.js": b"x"}
    reject(padded_stream(archive.MAX_TAR_BYTES + BLOCK))


def test_actual_member_count_limit_and_one_over():
    def members(count):
        yield from (member(f"package/f{i:05d}", b"") for i in range(count))
        yield bytes(1024)
    parsed = archive.parse_npm_archive(compressed_iter(members(archive.MAX_MEMBERS)))
    assert len(parsed.members) == archive.MAX_MEMBERS
    reject(compressed_iter(members(archive.MAX_MEMBERS + 1)))


def test_actual_utf8_name_byte_limit_and_one_over():
    # Header name remains safe and short; effective PAX name owns exact bytes.
    name = "package/" + "/".join(["a" * 250, "b" * 250, "c" * 250, "d" * 250, "é" * 6])
    assert len(name.encode()) == archive.MAX_NAME_BYTES
    raw = compress(tar(pax_member(pax_records([("path", name)]))))
    assert archive.parse_npm_archive(raw).members[0].name == name.removeprefix("package/")
    reject(compress(tar(pax_member(pax_records([("path", name + "a")])))))


def test_exact_pax_read_bound_and_declared_overflow_before_read(monkeypatch):
    payload = pax_records([("path", "package/a")])
    monkeypatch.setattr(archive, "MAX_PAX_BYTES", len(payload))
    assert archive.parse_npm_archive(compress(tar(pax_member(payload)))).files() == {"a": b"x"}
    calls = []
    original = archive._GzipReader.exact
    def observed(self, count):
        calls.append(count)
        return original(self, count)
    monkeypatch.setattr(archive._GzipReader, "exact", observed)
    reject(compress(header("PaxHeader/a", size=len(payload) + 1, kind=b"x")))
    assert calls == [512]


def test_actual_retained_path_node_limit_and_one_over():
    def content(over):
        for i in range(archive.MAX_MEMBERS):
            extra = "/extra" if over and i == archive.MAX_MEMBERS - 1 else ""
            yield member(f"package/f{i:05d}/a/b{extra}/file", b"")
        yield bytes(1024)
    parsed = archive.parse_npm_archive(compressed_iter(content(False)))
    assert len(parsed.members) * 4 == archive.MAX_PATH_NODES
    reject(compressed_iter(content(True)))


def test_actual_retained_path_byte_limit_and_one_over():
    # Unique six-byte root plus three 136-byte components: ancestor sum846.
    count, remainder = divmod(archive.MAX_PATH_BYTES, 846)
    assert count < archive.MAX_MEMBERS and 0 < remainder < 255
    def content(over):
        for i in range(count):
            name = "package/" + "/".join([f"f{i:05d}", "a" * 136, "b" * 136, "c" * 136])
            yield pax_member(pax_records([("path", name)]), member("package/placeholder", b""))
        yield pax_member(pax_records([("path", "package/" + "z" * (remainder + int(over)))]), member("package/placeholder", b""))
        yield bytes(1024)
    parsed = archive.parse_npm_archive(compressed_iter(content(False)))
    assert len(parsed.members) == count + 1
    reject(compressed_iter(content(True)))


def test_shared_path_ancestors_are_charged_once(monkeypatch):
    # a=1, a/x=3, a/y=3 => three nodes/seven bytes, not four/eight.
    monkeypatch.setattr(archive, "MAX_PATH_NODES", 3)
    monkeypatch.setattr(archive, "MAX_PATH_BYTES", 7)
    raw = compress(tar(member("package/a/x"), member("package/a/y")))
    assert len(archive.parse_npm_archive(raw).members) == 2
    monkeypatch.setattr(archive, "MAX_PATH_BYTES", 6)
    reject(raw)


def test_tighter_aggregate_tar_budget_exact_and_one_under():
    expanded = tar()
    raw = compress(expanded)
    parsed = archive.parse_npm_archive(raw, tar_byte_limit=len(expanded))
    assert parsed.tar_size == len(expanded)
    with pytest.raises(archive.ArchiveError):
        archive.parse_npm_archive(raw, tar_byte_limit=len(expanded) - 1)


@pytest.mark.parametrize("limit", [True, False, 0, -1, 1.5, "2048", 256 * 1024 * 1024 + 1])
def test_tighter_budget_cannot_raise_ceiling_or_coerce_types(limit):
    with pytest.raises(archive.ArchiveError):
        archive.parse_npm_archive(compress(tar()), tar_byte_limit=limit)


def test_invalid_member_extent_respects_original_tighter_budget_before_read(monkeypatch):
    calls = []
    original = archive._GzipReader.exact
    def observed(self, count):
        calls.append(count)
        return original(self, count)
    monkeypatch.setattr(archive._GzipReader, "exact", observed)
    with pytest.raises(archive.ArchiveError):
        archive.parse_npm_archive(compress(header(size=513)), tar_byte_limit=512 + 1023)
    assert calls == [512]
