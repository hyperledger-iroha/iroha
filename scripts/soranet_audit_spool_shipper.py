#!/usr/bin/env python3
"""Securely batch and ship SoraNet relay compliance events.

The shipper intentionally requires pre-provisioned, owner-private directories.
It never follows event-file symlinks, never invokes a shell, and publishes only
fully written archives.
"""

import argparse
import contextlib
import fcntl
import json
import os
import re
import secrets
import signal
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, List, Sequence

DEFAULT_BATCH = 200
MAX_BATCH = 10_000
MAX_SCAN_ENTRIES = 100_000
MAX_EVENT_BYTES = 256 * 1024
MAX_ARCHIVE_BYTES = 32 * 1024 * 1024
SHIP_TIMEOUT_SECONDS = 300
DIRECTORY_MODE = 0o700
FILE_MODE = 0o600
ENVIRONMENT_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
FORBIDDEN_SHIP_ENVIRONMENT = {
    "BASH_ENV",
    "ENV",
    "IFS",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "PYTHONHOME",
    "PYTHONPATH",
    "SORANET_AUDIT_ARCHIVE",
}


@dataclass(frozen=True)
class SpoolEvent:
    """One event path bound to the exact file observed during spool scanning."""

    path: Path
    device: int
    inode: int
    size: int
    mode: int
    uid: int
    gid: int
    links: int
    modified_ns: int
    changed_ns: int

    @classmethod
    def capture(cls, path: Path, metadata: os.stat_result) -> "SpoolEvent":
        return cls(
            path=path,
            device=metadata.st_dev,
            inode=metadata.st_ino,
            size=metadata.st_size,
            mode=metadata.st_mode,
            uid=metadata.st_uid,
            gid=metadata.st_gid,
            links=metadata.st_nlink,
            modified_ns=metadata.st_mtime_ns,
            changed_ns=metadata.st_ctime_ns,
        )

    def matches(self, metadata: os.stat_result) -> bool:
        return (
            self.device == metadata.st_dev
            and self.inode == metadata.st_ino
            and self.size == metadata.st_size
            and self.mode == metadata.st_mode
            and self.uid == metadata.st_uid
            and self.gid == metadata.st_gid
            and self.links == metadata.st_nlink
            and self.modified_ns == metadata.st_mtime_ns
            and self.changed_ns == metadata.st_ctime_ns
        )

    def matches_payload_identity(self, metadata: os.stat_result) -> bool:
        """Match fields that remain stable across a hard-link move."""

        return (
            self.device == metadata.st_dev
            and self.inode == metadata.st_ino
            and self.size == metadata.st_size
            and self.mode == metadata.st_mode
            and self.uid == metadata.st_uid
            and self.gid == metadata.st_gid
            and self.modified_ns == metadata.st_mtime_ns
        )


def _effective_uid() -> int:
    if not hasattr(os, "geteuid"):
        raise RuntimeError("the SoraNet audit shipper requires a Unix host")
    return os.geteuid()


def _validate_trusted_ancestors(path: Path, purpose: str) -> None:
    effective_uid = _effective_uid()
    current = path
    while True:
        metadata = os.lstat(current)
        if not stat.S_ISDIR(metadata.st_mode):
            raise ValueError(f"{purpose} ancestor is not a directory: {current}")
        trusted_owner = metadata.st_uid in (0, effective_uid)
        root_sticky = metadata.st_uid == 0 and metadata.st_mode & stat.S_ISVTX
        if not trusted_owner or (metadata.st_mode & 0o022 and not root_sticky):
            raise ValueError(
                f"{purpose} ancestor is replaceable by another principal: {current}"
            )
        parent = current.parent
        if parent == current:
            return
        current = parent


def secure_directory(path: Path, purpose: str) -> Path:
    """Resolve and validate a pre-provisioned owner-private directory."""

    if not path.is_absolute():
        raise ValueError(f"{purpose} directory must be absolute: {path}")
    try:
        canonical = path.resolve(strict=True)
    except OSError as error:
        raise ValueError(f"cannot resolve {purpose} directory {path}: {error}") from error
    effective_uid = _effective_uid()
    _validate_trusted_ancestors(canonical, purpose)
    leaf = os.stat(canonical, follow_symlinks=False)
    if stat.S_IMODE(leaf.st_mode) != DIRECTORY_MODE or leaf.st_uid != effective_uid:
        raise ValueError(
            f"{purpose} directory must be owned by the effective user with mode 0700: "
            f"{canonical}"
        )
    return canonical


@contextlib.contextmanager
def exclusive_spool_lock(spool: Path) -> Iterable[None]:
    """Serialize shipping against the exact validated spool directory inode."""

    nofollow = getattr(os, "O_NOFOLLOW", None)
    directory = getattr(os, "O_DIRECTORY", None)
    if nofollow is None or directory is None:
        raise RuntimeError("O_NOFOLLOW and O_DIRECTORY are required for SoraNet audit custody")
    flags = os.O_RDONLY | nofollow | directory | getattr(os, "O_CLOEXEC", 0)
    descriptor = os.open(spool, flags)
    try:
        named = os.stat(spool, follow_symlinks=False)
        opened = os.fstat(descriptor)
        if (
            (named.st_dev, named.st_ino) != (opened.st_dev, opened.st_ino)
            or not stat.S_ISDIR(opened.st_mode)
            or stat.S_IMODE(opened.st_mode) != DIRECTORY_MODE
            or opened.st_uid != _effective_uid()
        ):
            raise ValueError("audit spool changed identity or custody while locking")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("another SoraNet audit shipper owns the spool lock") from error
        yield
    finally:
        os.close(descriptor)


def _validate_event_metadata(path: Path, metadata: os.stat_result) -> None:
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != _effective_uid()
        or stat.S_IMODE(metadata.st_mode) != FILE_MODE
        or metadata.st_nlink != 1
    ):
        raise ValueError(
            f"audit event must be an owner-owned, single-link regular file with mode 0600: "
            f"{path}"
        )
    if metadata.st_size > MAX_EVENT_BYTES:
        raise ValueError(
            f"audit event exceeds the {MAX_EVENT_BYTES}-byte limit: {path}"
        )


def iter_spool_files(spool: Path) -> Iterable[SpoolEvent]:
    """Return a bounded, oldest-first snapshot of direct event files."""

    candidates = []
    with os.scandir(spool) as entries:
        for count, entry in enumerate(entries, start=1):
            if count > MAX_SCAN_ENTRIES:
                raise ValueError(
                    f"audit spool exceeds the {MAX_SCAN_ENTRIES}-entry scan limit"
                )
            path = Path(entry.path)
            if path.suffix != ".json":
                continue
            metadata = entry.stat(follow_symlinks=False)
            _validate_event_metadata(path, metadata)
            candidates.append(
                (metadata.st_mtime_ns, path.name, SpoolEvent.capture(path, metadata))
            )
    candidates.sort()
    return [candidate[2] for candidate in candidates]


def _read_event(event: SpoolEvent) -> object:
    path = event.path
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise RuntimeError("O_NOFOLLOW is required for SoraNet audit custody")
    flags = os.O_RDONLY | nofollow | getattr(os, "O_CLOEXEC", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        _validate_event_metadata(path, opened)
        named = os.stat(path, follow_symlinks=False)
        _validate_event_metadata(path, named)
        if not event.matches(opened) or not event.matches(named):
            raise ValueError(f"audit event changed after spool scanning: {path}")
        with os.fdopen(descriptor, "rb", closefd=False) as source:
            encoded = source.read(MAX_EVENT_BYTES + 1)
        if len(encoded) > MAX_EVENT_BYTES:
            raise ValueError(
                f"audit event exceeds the {MAX_EVENT_BYTES}-byte limit: {path}"
            )
        opened_after = os.fstat(descriptor)
        named_after = os.stat(path, follow_symlinks=False)
        _validate_event_metadata(path, opened_after)
        _validate_event_metadata(path, named_after)
        if not event.matches(opened_after) or not event.matches(named_after):
            raise ValueError(f"audit event changed while being read: {path}")
        event = json.loads(encoded)
        if not isinstance(event, dict):
            raise ValueError(f"audit event must be a JSON object: {path}")
        return event
    finally:
        os.close(descriptor)


def _sync_directory(path: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def write_archive(batch: Sequence[SpoolEvent], archive_dir: Path, dry_run: bool) -> Path:
    """Write a bounded archive and publish it atomically without clobbering."""

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    archive_path = archive_dir / (
        f"compliance-{timestamp}-{os.getpid()}-{secrets.token_hex(8)}.jsonl"
    )
    if dry_run:
        return archive_path

    descriptor, temporary_name = tempfile.mkstemp(
        prefix=".compliance-archive-", dir=archive_dir
    )
    temporary = Path(temporary_name)
    try:
        os.fchmod(descriptor, FILE_MODE)
        total = 0
        with os.fdopen(descriptor, "wb", closefd=False) as archive:
            for event in batch:
                rendered = json.dumps(
                    _read_event(event),
                    ensure_ascii=False,
                    separators=(",", ":"),
                    sort_keys=True,
                ).encode("utf-8") + b"\n"
                total += len(rendered)
                if total > MAX_ARCHIVE_BYTES:
                    raise ValueError(
                        f"audit archive exceeds the {MAX_ARCHIVE_BYTES}-byte limit"
                    )
                archive.write(rendered)
            archive.flush()
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        os.link(temporary, archive_path, follow_symlinks=False)
        os.unlink(temporary)
        _sync_directory(archive_dir)
        return archive_path
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def _validate_ship_program(program: str) -> str:
    path = Path(program)
    if not path.is_absolute():
        raise ValueError("the shipping program path must be absolute")
    canonical_parent = path.parent.resolve(strict=True)
    _validate_trusted_ancestors(canonical_parent, "shipping program")
    canonical = canonical_parent / path.name
    metadata = os.stat(canonical, follow_symlinks=False)
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid not in (0, _effective_uid())
        or metadata.st_mode & 0o022
        or metadata.st_nlink != 1
        or not metadata.st_mode & 0o111
    ):
        raise ValueError(
            "the shipping program must be a trusted, single-link executable regular file"
        )
    return str(canonical)


def _shipping_environment(archive_path: Path, allowed_names: Sequence[str]) -> dict:
    environment = {
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "SORANET_AUDIT_ARCHIVE": str(archive_path),
    }
    for name in allowed_names:
        if (
            not ENVIRONMENT_NAME.fullmatch(name)
            or name in FORBIDDEN_SHIP_ENVIRONMENT
            or name.startswith("DYLD_")
        ):
            raise ValueError(f"unsafe shipping environment name: {name!r}")
        if name not in os.environ:
            raise ValueError(f"requested shipping environment variable is unset: {name}")
        environment[name] = os.environ[name]
    return environment


def ship_archive(
    archive_path: Path,
    command: Sequence[str],
    dry_run: bool,
    allowed_environment: Sequence[str] = (),
) -> None:
    """Invoke an explicitly selected executable without a shell."""

    if dry_run or not command:
        return
    if command.count("{archive}") != 1:
        raise ValueError("the shipping command must contain exactly one {archive} argument")
    program = _validate_ship_program(command[0])
    arguments = [str(archive_path) if value == "{archive}" else value for value in command[1:]]
    process = subprocess.Popen(
        [program, *arguments],
        env=_shipping_environment(archive_path, allowed_environment),
        stdin=subprocess.DEVNULL,
        cwd="/",
        close_fds=True,
        start_new_session=True,
    )
    try:
        return_code = process.wait(timeout=SHIP_TIMEOUT_SECONDS)
    except BaseException:
        _terminate_shipping_process_group(process)
        raise
    # A trusted shipping adapter is still not allowed to detach background work that retains
    # inherited resources or continues using credentials after the requested shipment completes.
    _signal_shipping_process_group(process.pid, signal.SIGKILL)
    if return_code != 0:
        raise subprocess.CalledProcessError(return_code, [program, *arguments])


def _signal_shipping_process_group(process_group: int, requested_signal: int) -> None:
    try:
        os.killpg(process_group, requested_signal)
    except ProcessLookupError:
        pass


def _terminate_shipping_process_group(process: subprocess.Popen) -> None:
    """Bounded cleanup for a failed or interrupted shipping process tree."""

    _signal_shipping_process_group(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=1)
    except subprocess.TimeoutExpired:
        pass
    _signal_shipping_process_group(process.pid, signal.SIGKILL)
    if process.poll() is None:
        process.wait()


def _unlink_if_identity(path: Path, expected: os.stat_result) -> None:
    """Remove `path` only while it still names the exact expected inode."""

    try:
        current = os.stat(path, follow_symlinks=False)
    except FileNotFoundError:
        return
    if (current.st_dev, current.st_ino) == (expected.st_dev, expected.st_ino):
        os.unlink(path)


def cleanup_batch(
    batch: Sequence[SpoolEvent], processed_dir: Path, dry_run: bool
) -> None:
    """Hard-link shipped records into processed custody without clobbering."""

    if dry_run:
        return
    publications = []
    try:
        # Publish every no-clobber hard link before removing any source. A collision or transient
        # failure on a later record therefore cannot leave an earlier record half-moved after the
        # archive has already been shipped.
        for event in batch:
            file_path = event.path
            named = os.stat(file_path, follow_symlinks=False)
            _validate_event_metadata(file_path, named)
            if not event.matches(named):
                raise ValueError(
                    f"audit event changed before processed publication: {file_path}"
                )
            destination = processed_dir / file_path.name
            os.link(file_path, destination, follow_symlinks=False)
            linked = os.stat(destination, follow_symlinks=False)
            source_after = os.stat(file_path, follow_symlinks=False)
            if not event.matches_payload_identity(linked) or (
                linked.st_dev,
                linked.st_ino,
            ) != (source_after.st_dev, source_after.st_ino):
                _unlink_if_identity(destination, linked)
                raise ValueError(
                    f"audit event changed while publishing processed evidence: {file_path}"
                )
            publications.append((event, destination, linked))
    except BaseException:
        for _event, destination, linked in reversed(publications):
            _unlink_if_identity(destination, linked)
        _sync_directory(processed_dir)
        raise

    try:
        for event, destination, linked in publications:
            source = os.stat(event.path, follow_symlinks=False)
            current_destination = os.stat(destination, follow_symlinks=False)
            if (
                not event.matches_payload_identity(source)
                or not event.matches_payload_identity(current_destination)
                or (source.st_dev, source.st_ino) != (linked.st_dev, linked.st_ino)
                or (current_destination.st_dev, current_destination.st_ino)
                != (linked.st_dev, linked.st_ino)
            ):
                raise ValueError(
                    f"audit event changed before source retirement: {event.path}"
                )
    except BaseException:
        for _event, destination, linked in reversed(publications):
            _unlink_if_identity(destination, linked)
        _sync_directory(processed_dir)
        raise

    for event, destination, _linked in publications:
        os.unlink(event.path)
        final = os.stat(destination, follow_symlinks=False)
        _validate_event_metadata(destination, final)
        if not event.matches_payload_identity(final):
            raise ValueError(
                f"processed audit event changed after publication: {destination}"
            )
    _sync_directory(batch[0].path.parent)
    _sync_directory(processed_dir)


def _batched(
    paths: Iterable[SpoolEvent], batch_size: int
) -> Iterable[List[SpoolEvent]]:
    batch: List[SpoolEvent] = []
    for path in paths:
        batch.append(path)
        if len(batch) == batch_size:
            yield batch
            batch = []
    if batch:
        yield batch


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--spool-dir",
        default="/var/spool/soranet/audit",
        type=Path,
        help="Pre-provisioned 0700 directory containing 0600 compliance JSON files",
    )
    parser.add_argument(
        "--archive-dir",
        default="/var/lib/soranet/audit-archives",
        type=Path,
        help="Pre-provisioned 0700 directory for immutable JSONL archives",
    )
    parser.add_argument(
        "--processed-dir",
        default="/var/lib/soranet/audit-processed",
        type=Path,
        help="Pre-provisioned 0700 directory for shipped source records",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=DEFAULT_BATCH,
        help=f"Events per archive (1..{MAX_BATCH})",
    )
    parser.add_argument(
        "--ship-env",
        action="append",
        default=[],
        metavar="NAME",
        help="Explicitly forward one runtime environment variable to the shipping program",
    )
    parser.add_argument(
        "--ship-command",
        nargs=argparse.REMAINDER,
        default=[],
        help=(
            "Absolute shipping executable followed by literal arguments including one "
            "{archive} placeholder; must be the final option because no shell parsing is "
            "performed"
        ),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate and report work without modifying files",
    )
    args = parser.parse_args()
    if not 1 <= args.batch_size <= MAX_BATCH:
        parser.error(f"--batch-size must be between 1 and {MAX_BATCH}")

    spool_dir = secure_directory(args.spool_dir, "audit spool")
    archive_dir = secure_directory(args.archive_dir, "audit archive")
    processed_dir = secure_directory(args.processed_dir, "processed audit")
    if len({spool_dir, archive_dir, processed_dir}) != 3:
        parser.error("spool, archive, and processed directories must be distinct")

    archives_created = 0
    shipped = 0
    with exclusive_spool_lock(spool_dir):
        for batch in _batched(iter_spool_files(spool_dir), args.batch_size):
            archive_path = write_archive(batch, archive_dir, args.dry_run)
            ship_archive(archive_path, args.ship_command, args.dry_run, args.ship_env)
            cleanup_batch(batch, processed_dir, args.dry_run)
            archives_created += 1
            if args.ship_command:
                shipped += 1

    print(f"Processed {archives_created} archive(s); shipped {shipped} via command")
    return 0


if __name__ == "__main__":
    sys.exit(main())
