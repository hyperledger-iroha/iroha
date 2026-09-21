"""Canonical bounded Python execution ZIP codec; content grants no process authority.

Both the producer and original-index adapter use this one representation. The
reader consumes immutable original bytes, never extracts files or reopens paths
reported by a producer. Candidate/source/input and execution joins are separate.
"""
from __future__ import annotations

import io
import stat
import zipfile

from sorafs_python_consumer_artifact import ArtifactError, _path, _VERIFIER as verifier

MAX_MEMBERS = 20000
MAX_MEMBER_BYTES = verifier.MAX_MEMBER_BYTES
MAX_PAYLOAD_BYTES = 768 * 1024 * 1024
MAX_ARCHIVE_BYTES = 1024 * 1024 * 1024
_MAX_NAME_BYTES = 1024


def _name(value: object) -> str:
    name = _path(value, absolute=False)
    if len(name.encode("utf-8")) > _MAX_NAME_BYTES:
        raise ArtifactError("Python execution member name exceeds its bound")
    return name


def execution_archive(members: dict[str, bytes]) -> bytes:
    """Write the one sorted uncompressed representation of captured observations."""
    if (type(members) is not dict or not 0 < len(members) <= MAX_MEMBERS
            or any(type(raw) is not bytes or len(raw) > MAX_MEMBER_BYTES for raw in members.values())
            or sum(map(len, members.values())) > MAX_PAYLOAD_BYTES):
        raise ArtifactError("Python execution archive exceeds its bounds")
    for name in members:
        _name(name)
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as archive:
        for name, raw in sorted(members.items()):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            entry.create_system = 3
            entry.external_attr = (stat.S_IFREG | 0o600) << 16
            archive.writestr(entry, raw)
    result = output.getvalue()
    if len(result) > MAX_ARCHIVE_BYTES:
        raise ArtifactError("Python execution archive exceeds index original-file bound")
    with zipfile.ZipFile(io.BytesIO(result)) as archive:
        if {name: archive.read(name) for name in archive.namelist()} != members:
            raise ArtifactError("Python execution archive did not replay exactly")
    return result


def archive_members(raw: bytes) -> dict[str, bytes]:
    """Read bounded regular members and require byte-for-byte canonical encoding.

    This authenticates the codec relation only. Even an exact archive must still
    be joined to original indexed inputs, reviewed source and producer approval.
    """
    if type(raw) is not bytes or not 0 < len(raw) <= MAX_ARCHIVE_BYTES:
        raise ArtifactError("Python execution ZIP byte bound")
    try:
        verifier.preflight_zip_directory(
            raw, max_members=MAX_MEMBERS, max_name_bytes=_MAX_NAME_BYTES,
            allow_member_extra=False, allow_member_comments=False,
        )
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            entries = archive.infolist()
            if not 0 < len(entries) <= MAX_MEMBERS:
                raise ArtifactError("Python execution ZIP member bound")
            verifier._assert_canonical_zip_envelope(raw, archive, entries)
            names, total, result = [], 0, {}
            for entry in entries:
                name = _name(entry.filename)
                if (entry.orig_filename != name or name in result or entry.is_dir()
                        or entry.create_system != 3
                        or entry.external_attr != (stat.S_IFREG | 0o600) << 16
                        or entry.compress_type != zipfile.ZIP_STORED
                        or entry.file_size != entry.compress_size
                        or entry.file_size > MAX_MEMBER_BYTES):
                    raise ArtifactError("Python execution ZIP member differs from its canonical layout")
                total += entry.file_size
                if total > MAX_PAYLOAD_BYTES:
                    raise ArtifactError("Python execution ZIP payload bound")
                body = verifier._read_member_bytes(archive, entry, label="Python execution member")
                if len(body) != entry.file_size:
                    raise ArtifactError("Python execution ZIP member size differs")
                names.append(name)
                result[name] = body
            if names != sorted(names) or execution_archive(result) != raw:
                raise ArtifactError("Python execution ZIP is not its exact canonical encoding")
            return result
    except (zipfile.BadZipFile, RuntimeError, NotImplementedError, OSError) as error:
        raise ArtifactError("Python execution ZIP could not be replayed") from error
