"""Pure npm archive controls; all payloads are inert synthetic bytes."""
from __future__ import annotations

from dataclasses import FrozenInstanceError
import gzip
import os
from pathlib import Path
import sys
import tarfile

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_archive as archive
from sorafs_javascript_archive_fixtures import (
    BLOCK, checksum, compress, field, header, member, pax_member, pax_records,
    stdlib_archive, tar,
)


def reject(payload):
    with pytest.raises(archive.ArchiveError):
        archive.parse_npm_archive(payload)


def test_stdlib_regular_files_empty_executable_and_ustar_prefix():
    long_name = "lib/" + "a" * 90 + "/entry.js"
    entries = [("package/empty", b"", 0o644), ("package/bin/run", b"inert", 0o755),
               ("package/" + long_name, b"export {};", 0o644)]
    raw = stdlib_archive(entries)
    parsed = archive.parse_npm_archive(raw)
    assert parsed.raw == raw
    assert parsed.files() == {name.removeprefix("package/"): body for name, body, _ in entries}
    assert [(x.name, x.mode) for x in parsed.members] == [(n.removeprefix("package/"), m) for n, _, m in entries]
    assert all(type(x.content) is bytes for x in parsed.members)
    with pytest.raises(FrozenInstanceError):
        parsed.raw = b"replacement"
    with pytest.raises(FrozenInstanceError):
        parsed.members[0].name = "replacement"
    result = parsed.files()
    result.clear()
    assert len(parsed.files()) == 3


def test_parser_performs_no_filesystem_io(monkeypatch):
    raw = compress(tar(member("package/a", b"inert")))
    def forbidden(*args, **kwargs):
        raise AssertionError("pure archive parser touched filesystem")
    monkeypatch.setattr("builtins.open", forbidden)
    monkeypatch.setattr(os, "open", forbidden)
    monkeypatch.setattr(Path, "open", forbidden)
    assert archive.parse_npm_archive(raw).files() == {"a": b"inert"}


@pytest.mark.parametrize("raw", [b"", b"not gzip", b"\x1f\x8b", bytearray(b"x"), memoryview(b"x"), None])
def test_invalid_input_is_archive_error(raw):
    reject(raw)


@pytest.mark.parametrize("operation", [
    lambda x: x + b"trailing", lambda x: x + bytes(32),
    lambda x: x + compress(tar()), lambda x: x[:-1],
    lambda x: x[:3] + bytes([x[3] | 0xe0]) + x[4:],
    lambda x: x[:2] + b"\x00" + x[3:],
    lambda x: x[:-8] + bytes([x[-8] ^ 1]) + x[-7:],
    lambda x: x[:-4] + bytes([x[-4] ^ 1]) + x[-3:],
])
def test_gzip_envelope_rejects_corruption_and_second_stream(operation):
    reject(operation(compress(tar())))


@pytest.mark.parametrize("name", [
    "a.js", "/package/a", "package/", "package//a", "package/./a", "package/../a",
    "package/a/../b", "package/a/./b", "package/a/", "package/a\\b", "package/C:a",
    "package/a\n.js", "package/a\r.js", "package/a\t.js", "Package/a", "package/e\u0301.js",
])
def test_noncanonical_paths_rejected(name):
    reject(compress(tar(member(name))))


@pytest.mark.parametrize("names", [
    ("a", "a"), ("A", "a"), ("Straße", "STRASSE"),
    ("a", "a/b"), ("a/b", "a"), ("A/b", "a/c"),
])
def test_duplicate_case_unicode_and_ancestor_aliases_rejected(names):
    reject(compress(tar(*(member("package/" + name) for name in names))))


@pytest.mark.parametrize("kind", [b"1", b"2", b"3", b"4", b"5", b"6", b"7", b"S", b"L", b"K", b"g", b"?", b"\0"])
def test_non_regular_header_types_rejected(kind):
    reject(compress(tar(member(kind=kind))))


@pytest.mark.parametrize("mode", [0, 0o600, 0o664, 0o700, 0o777, 0o1644, 0o2644, 0o4644])
def test_noncanonical_modes_rejected(mode):
    reject(compress(tar(member(mode=mode))))


@pytest.mark.parametrize("start,stop,value", [
    (257, 263, b"ustar "), (263, 265, b"01"),
    (100, 108, b"-000644\0"), (108, 116, b"0000008\0"),
    (124, 136, b"80000000001\0"), (124, 136, b"\x80" + bytes(10) + b"\x01"),
    (136, 148, b"-0000000001\0"), (148, 156, b"000000\0 "),
    (157, 257, b"hidden-link" + bytes(89)),
    (500, 512, b"unexpected!!"),
])
def test_bad_header_fields_rejected(start, stop, value):
    block = field(header(), start, stop, value, repair=start != 148)
    reject(compress(tar(block + b"x" + bytes(511))))


def test_header_checksum_not_signed_or_ignored():
    payload = bytearray(tar())
    payload[0] ^= 1
    reject(compress(bytes(payload)))


def test_embedded_nul_name_cannot_hide_suffix():
    block = field(header(), 0, 100, b"package/a\0hidden" + bytes(84))
    reject(compress(tar(block + b"x" + bytes(511))))


@pytest.mark.parametrize("payload", [
    tar(end_blocks=0), tar(end_blocks=1), tar(padding=1),
    tar() + member("package/hidden"), tar() + b"x" + bytes(511),
    member() + bytes(512) + member("package/after-one-zero") + bytes(1024),
    header(size=2) + b"x", header(size=1024) + bytes(512),
    header() + b"x" + b"notzero" + bytes(504) + bytes(1024),
])
def test_padding_truncation_and_end_markers_rejected(payload):
    reject(compress(payload))


def test_two_end_blocks_and_standard_zero_record_padding_valid():
    for payload in (tar(), tar(padding=512), tar(padding=8192)):
        assert archive.parse_npm_archive(compress(payload)).files() == {"a.js": b"x"}


def test_stdlib_per_file_pax_nfc_utf8_and_long_path():
    path = "package/" + "é" * 70 + "/file.js"
    raw = stdlib_archive([(path, b"ok", 0o644)], format=tarfile.PAX_FORMAT)
    assert archive.parse_npm_archive(raw).files() == {path.removeprefix("package/"): b"ok"}


@pytest.mark.parametrize("records", [
    b"path=package/a\n", b"0 path=package/a\n", b"01 path=package/a\n", b"999 path=package/a\n",
    b"20 path=package/a", b"20 path=package/a\nJUNK", b"18 path=package/\xff\n",
    pax_records([("path", "package/a"), ("path", "package/b")]),
    pax_records([("linkpath", "package/a")]), pax_records([("GNU.sparse.name", "package/a")]),
    pax_records([("path", "../escaped")]), pax_records([("path", "package/a\nextra")]),
    pax_records([("unknown", "ignored")]), b"",
])
def test_malformed_or_unsupported_pax_rejected(records):
    reject(compress(tar(pax_member(records))))


def test_pax_cannot_dangle_or_stack():
    extended = member("PaxHeader/a", pax_records([("path", "package/a")]), kind=b"x")
    reject(compress(tar(extended)))
    reject(compress(tar(extended + extended + member())))


def test_safe_pax_effective_path_may_override_safe_ustar_name():
    raw = compress(tar(pax_member(pax_records([("path", "package/different")]))))
    assert archive.parse_npm_archive(raw).files() == {"different": b"x"}


def test_actual_npm_pack_fixture_uses_nonexecuted_pax_metadata():
    # Actual offline npm pack --ignore-scripts output; its prepack exits 99.
    # These bytes are parsed only, never installed or loaded.
    import hashlib
    raw = (Path(__file__).parent / "fixtures/sorafs_npm_archive_v1.tgz").read_bytes()
    assert len(raw) == 445
    assert hashlib.sha256(raw).hexdigest() == "160d7888da2f1df4c9624f063de6dc8d9848b92834449f125fe0d43b93a8ce06"
    parsed = archive.parse_npm_archive(raw)
    assert parsed.files()["empty.txt"] == b""
    assert parsed.files()["long/" + "a" * 110 + "/é.js"] == b"export default 2;\n"
    assert parsed.files()["index.js"] == b"export const value = 1;\n"
    assert {item.name: item.mode for item in parsed.members}["bin/run.js"] == 0o755
    assert len(parsed.members) == 5


@pytest.mark.parametrize("rows", [
    [("path", "package/a"), ("size", "1"), ("mtime", "499162500")],
    [("mtime", "499162500"), ("path", "package/a"), ("size", "1")],
])
def test_pax_numeric_metadata_agrees_exactly_with_regular_header(rows):
    assert archive.parse_npm_archive(compress(tar(pax_member(pax_records(rows))))).files() == {"a": b"x"}


@pytest.mark.parametrize("extra", [
    ("size", "2"), ("mtime", "499162501"), ("size", "01"), ("mtime", "0499162500"),
    ("size", "+1"), ("size", "-1"), ("size", "1.0"), ("mtime", "499162500.0"),
    ("mtime", "499162500.1"), ("mtime", "-1"), ("atime", "499162500"),
])
def test_pax_numeric_override_or_noncanonical_value_rejected(extra):
    reject(compress(tar(pax_member(pax_records([("path", "package/a"), extra])))))


@pytest.mark.parametrize("key,value", [("size", "1"), ("mtime", "499162500")])
def test_duplicate_pax_numeric_record_rejected(key, value):
    reject(compress(tar(pax_member(pax_records([("path", "package/a"), (key, value), (key, value)])))))


def test_pax_without_path_or_with_unsafe_original_header_rejected():
    reject(compress(tar(pax_member(pax_records([("size", "1")])))))
    reject(compress(tar(pax_member(pax_records([("path", "package/a")]), member("../unsafe")))))


@pytest.mark.parametrize("start,stop,value", [
    (108, 116, b"-000001\0"), (116, 124, b"\x80" + bytes(6) + b"\x01"),
    (329, 337, b"0000001\0"), (337, 345, b"0000001\0"),
    (265, 297, b"owner\0hidden" + bytes(20)),
])
def test_inert_header_metadata_cannot_hide_noncanonical_numbers_or_suffixes(start, stop, value):
    block = field(header(), start, stop, value)
    reject(compress(tar(block + b"x" + bytes(511))))


def test_pax_zero_size_matches_actual_empty_file():
    records = pax_records([("path", "package/empty"), ("size", "0"), ("mtime", "499162500")])
    raw = compress(tar(pax_member(records, member("package/placeholder", b""))))
    assert archive.parse_npm_archive(raw).files() == {"empty": b""}


def test_whole_archive_must_have_at_least_one_regular_member():
    reject(compress(bytes(1024)))


def test_mode_zero_belongs_only_to_inert_pax_header():
    records = pax_records([("path", "package/a")])
    extended = member("PaxHeader/a", records, kind=b"x", mode=0)
    assert archive.parse_npm_archive(compress(tar(extended + member()))).files() == {"a": b"x"}
    reject(compress(tar(member(mode=0))))
