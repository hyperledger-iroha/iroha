"""Early directory admission for all original Python ZIP consumers; no extraction."""
from __future__ import annotations

from copy import copy
import hashlib
import io
from pathlib import Path
import stat
import struct
import sys
import zipfile
import zlib

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_archive as execution
import sorafs_python_dependency_archive as dependency
from sorafs_python_consumer_artifact import ArtifactError, _VERIFIER as verifier
from sorafs_python_dependency_inputs import DependencyWheel
from sorafs_sdk_artifact_index import FileReference
from sorafs_python_dependency_install_test import harness, original_wheel

END = struct.Struct("<4s4H2IH")
CENTRAL = struct.Struct("<4s6H3I5H2I")


def raw_zip(*, extra=b"", comment=b""):
    result = io.BytesIO()
    with zipfile.ZipFile(result, "w") as archive:
        for name in ("a" * 100, "b" * 100):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            entry.create_system = 3
            entry.external_attr = (stat.S_IFREG | 0o600) << 16
            entry.extra, entry.comment = extra, comment
            archive.writestr(entry, b"bounded original bytes")
    return result.getvalue()


def parse(consumer, raw):
    if consumer == "execution":
        return execution.archive_members(raw)
    if consumer == "dependency":
        owner = DependencyWheel("idna", "1.0.0", FileReference(
            "/recorded/idna.whl", hashlib.sha256(raw).hexdigest(), len(raw)))
        return dependency.parse_dependency_wheel(raw, wheel=owner)
    return verifier.parse_wheel_bytes(
        raw, owner=verifier.NATIVE_OWNER if consumer == "native" else verifier.SDK_OWNER,
        extension_suffixes=(".abi3.so",))


@pytest.mark.parametrize("consumer", ("native", "sdk", "dependency", "execution"))
@pytest.mark.parametrize("mutation", (
    "over_count", "underdeclared_count", "overdeclared_count", "oversized_name",
    "variable_extent", "central_signature", "central_extent", "entry_disk",
))
def test_every_consumer_admits_actual_directory_before_zipfile(consumer, mutation, monkeypatch):
    raw = bytearray(raw_zip())
    end = len(raw) - END.size
    central = END.unpack_from(raw, end)[6]
    maximum_count = execution.MAX_MEMBERS if consumer == "execution" else verifier.MAX_ARCHIVE_MEMBERS
    maximum_name = 1024 if consumer == "execution" else verifier.MAX_MEMBER_NAME_BYTES
    if mutation == "over_count": struct.pack_into("<HH", raw, end + 8, maximum_count + 1, maximum_count + 1)
    elif mutation == "underdeclared_count": struct.pack_into("<HH", raw, end + 8, 1, 1)
    elif mutation == "overdeclared_count": struct.pack_into("<HH", raw, end + 8, 3, 3)
    elif mutation == "oversized_name": struct.pack_into("<H", raw, central + 28, maximum_name + 1)
    elif mutation == "variable_extent": struct.pack_into("<HH", raw, central + 30, 0xFFFF, 0xFFFF)
    elif mutation == "central_signature": raw[central:central + 4] = b"NOPE"
    elif mutation == "central_extent": struct.pack_into("<I", raw, end + 16, central + 1)
    else: struct.pack_into("<H", raw, central + 34, 1)

    def forbidden(*_args, **_kwargs):
        raise AssertionError("original directory refusal must precede every ZipFile constructor")

    monkeypatch.setattr(zipfile, "ZipFile", forbidden)
    with pytest.raises((verifier.VerificationError, ArtifactError)):
        parse(consumer, bytes(raw))


def test_exact_raw_directory_bounds_preserve_allowed_wheel_extras_and_comments():
    extra, comment = b"\xfe\xca\x03\x00xyz", b"retained original comment"
    raw = raw_zip(extra=extra, comment=comment)
    verifier.preflight_zip_directory(raw, max_members=2, max_name_bytes=100,
                                    allow_member_extra=True, allow_member_comments=True)
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        assert len(archive.infolist()) == 2
        assert all(entry.extra == extra and entry.comment == comment for entry in archive.infolist())
    for count, name in ((1, 100), (2, 99)):
        with pytest.raises(verifier.VerificationError):
            verifier.preflight_zip_directory(raw, max_members=count, max_name_bytes=name,
                                            allow_member_extra=True, allow_member_comments=True)
    for extras, comments in ((False, True), (True, False)):
        with pytest.raises(verifier.VerificationError):
            verifier.preflight_zip_directory(raw, max_members=2, max_name_bytes=100,
                                            allow_member_extra=extras, allow_member_comments=comments)


def with_member_metadata(entries):
    result = [(copy(info), payload) for info, payload in entries]
    result[0][0].extra = b"\xfe\xca\x03\x00xyz"
    result[0][0].comment = b"original allowed member comment"
    return result


@pytest.mark.parametrize("consumer", ("native", "sdk", "dependency"))
def test_original_wheel_parsers_still_accept_member_extras_and_comments(harness, tmp_path, consumer):
    if consumer == "dependency":
        parsed, _ = original_wheel(harness, tmp_path, module="idna",
                                   mutate=lambda entries, _dist: with_member_metadata(entries))
        raw = parsed.raw
        assert dependency.parse_dependency_wheel(raw, wheel=parsed.wheel) == parsed
    else:
        entries = harness["valid_entries"]() if consumer == "native" else harness["sdk_entries"]
        path = harness["write_wheel"](tmp_path / (consumer + ".whl"), with_member_metadata(entries))
        raw = path.read_bytes()
        parsed = parse(consumer, raw)
        assert parsed.owner == (verifier.NATIVE_OWNER if consumer == "native" else verifier.SDK_OWNER)
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        assert archive.infolist()[0].extra == b"\xfe\xca\x03\x00xyz"
        assert archive.infolist()[0].comment == b"original allowed member comment"


def redirected_zip64_comment():
    """A valid classic one-member directory whose comment redirects ZipFile."""
    local = struct.Struct("<4s5H3I2H")
    end64 = struct.Struct("<4sQ2H2I4Q")
    locator = struct.Struct("<4sIQI")

    def row(name, compressed=0, size=0, crc=0, comment=0):
        return CENTRAL.pack(b"PK\x01\x02", 20, 20, 0, 0, 0, 0, crc, compressed, size,
                            len(name), 0, comment, 0, 0, 0, 0) + name

    hidden = b"".join(row(("hidden-" + str(index)).encode()) for index in range(65))
    name = b"original"
    crc = zlib.crc32(hidden)
    head = local.pack(b"PK\x03\x04", 20, 0, 0, 0, 0, crc, len(hidden), len(hidden), len(name), 0) + name
    classic_offset = len(head) + len(hidden)
    classic = row(name, len(hidden), len(hidden), crc, end64.size + locator.size)
    zip64_offset = classic_offset + len(classic)
    alternate_offset = len(head)
    alternate_size = zip64_offset - alternate_offset
    comment = (end64.pack(b"PK\x06\x06", 44, 45, 45, 0, 0, 66, 66, alternate_size, alternate_offset)
               + locator.pack(b"PK\x06\x07", 0, zip64_offset, 1))
    raw = head + hidden + classic + comment + END.pack(
        b"PK\x05\x06", 0, 0, 1, 1, len(classic) + len(comment), classic_offset, 0)
    assert classic_offset + len(classic) + len(comment) == len(raw) - END.size
    assert CENTRAL.unpack_from(raw, classic_offset)[12] == len(comment)
    return raw


@pytest.mark.parametrize("consumer", ("native", "sdk", "dependency", "execution"))
def test_zip64_comment_cannot_redirect_the_admitted_directory(consumer, monkeypatch):
    raw = redirected_zip64_comment()
    # Prove the bounded attack is real on the pinned interpreter: classic EOCD
    # has one member, but ZipFile follows its comment to 66 original headers.
    assert END.unpack_from(raw, len(raw) - END.size)[4] == 1
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        assert len(archive.infolist()) == 66

    def forbidden(*_args, **_kwargs):
        raise AssertionError("ZIP64 alias must refuse before redirected inventory allocation")

    monkeypatch.setattr(zipfile, "ZipFile", forbidden)
    with pytest.raises((verifier.VerificationError, ArtifactError)):
        parse(consumer, raw)
