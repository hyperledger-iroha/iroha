"""Independent inert npm archive fixtures; never execute or extract members."""
from __future__ import annotations

import gzip
import io
import tarfile
import zlib

BLOCK = 512


def header(name="package/a.js", *, size=1, mode=0o644, kind=b"0", link=""):
    info = tarfile.TarInfo(name)
    info.size, info.mode, info.type, info.linkname = size, mode, kind, link
    info.uid = info.gid = 0
    info.mtime = 499162500
    return info.tobuf(format=tarfile.USTAR_FORMAT, encoding="utf-8", errors="strict")


def checksum(block):
    value = bytearray(block)
    value[148:156] = b" " * 8
    value[148:156] = f"{sum(value):06o}\0 ".encode()
    return bytes(value)


def field(block, start, stop, value, *, repair=True):
    assert len(value) == stop - start
    changed = block[:start] + value + block[stop:]
    return checksum(changed) if repair else changed


def member(name="package/a.js", body=b"x", **kwargs):
    return header(name, size=len(body), **kwargs) + body + bytes(-len(body) % BLOCK)


def tar(*members, end_blocks=2, padding=0):
    return b"".join(members or (member(),)) + bytes(BLOCK * end_blocks + padding)


def compress(payload):
    return gzip.compress(payload, mtime=0)


def stdlib_archive(entries, *, format=tarfile.USTAR_FORMAT):
    sink = io.BytesIO()
    with tarfile.open(fileobj=sink, mode="w", format=format, encoding="utf-8") as output:
        for name, body, mode in entries:
            info = tarfile.TarInfo(name)
            info.size, info.mode, info.mtime = len(body), mode, 499162500
            output.addfile(info, io.BytesIO(body))
    return compress(sink.getvalue())


def pax_records(rows):
    result = bytearray()
    for key, value in rows:
        suffix = f" {key}={value}\n".encode("utf-8")
        length = len(suffix) + 1
        while len(str(length)) + len(suffix) != length:
            length = len(str(length)) + len(suffix)
        result.extend(str(length).encode() + suffix)
    return bytes(result)


def pax_member(records, following=None):
    return member("PaxHeader/a.js", records, kind=b"x") + (member() if following is None else following)


def compressed_chunks(*chunks):
    compressor = zlib.compressobj(level=1, wbits=31)
    return b"".join(compressor.compress(chunk) for chunk in chunks) + compressor.flush()


def compressed_iter(chunks):
    """Compress generated chunks without allocating a complete expanded tar."""
    compressor = zlib.compressobj(level=1, wbits=31)
    output = bytearray()
    for chunk in chunks:
        output.extend(compressor.compress(chunk))
    output.extend(compressor.flush())
    return bytes(output)
