#!/usr/bin/env python3
"""Durably publish a new release marker without following filesystem links."""

from __future__ import annotations

import argparse
import errno
import os
from pathlib import Path
import stat
import sys


class PublicationError(RuntimeError):
    """Raised when a release marker cannot be published safely."""


_DIRECTORY_FLAGS = (
    os.O_RDONLY
    | getattr(os, "O_DIRECTORY", 0)
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
_CREATE_FLAGS = (
    os.O_WRONLY
    | os.O_CREAT
    | os.O_EXCL
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
_MAXIMUM_ALLOWED_BYTES = 1024 * 1024


class _AnchoredParent:
    """An opened, owner-controlled parent reached without following symlinks."""

    def __init__(self, path: Path, *, label: str) -> None:
        if not path.is_absolute():
            raise PublicationError(f"{label} must be an absolute path")
        if path.name in {"", ".", ".."}:
            raise PublicationError(f"{label} must name a file")

        descriptors: list[int] = []
        try:
            current = os.open("/", _DIRECTORY_FLAGS)
            descriptors.append(current)
            for component in path.parent.parts[1:]:
                if component in {"", ".", ".."}:
                    raise PublicationError(
                        f"{label} contains an unsafe parent component"
                    )
                current = os.open(component, _DIRECTORY_FLAGS, dir_fd=current)
                descriptors.append(current)
        except OSError as error:
            for descriptor in reversed(descriptors):
                os.close(descriptor)
            if error.errno == errno.ELOOP:
                raise PublicationError(
                    f"{label} parent must not contain a symlink"
                ) from error
            raise PublicationError(
                f"cannot open authenticated parent for {label}: {error.strerror}"
            ) from error

        self._descriptors = descriptors
        self.descriptor = descriptors[-1]
        self.leaf = path.name
        self.label = label
        parent = os.fstat(self.descriptor)
        if not stat.S_ISDIR(parent.st_mode):
            self.close()
            raise PublicationError(f"{label} parent must be a directory")
        if parent.st_uid != os.geteuid():
            self.close()
            raise PublicationError(f"{label} parent must be owned by the current user")
        if stat.S_IMODE(parent.st_mode) & 0o022:
            self.close()
            raise PublicationError(
                f"{label} parent must not be group- or world-writable"
            )

    @property
    def temporary(self) -> str:
        return f".{self.leaf}.publish.tmp"

    def require_absent(self, name: str, *, item_label: str) -> None:
        try:
            metadata = os.stat(
                name,
                dir_fd=self.descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return
        kind = "symlink" if stat.S_ISLNK(metadata.st_mode) else "unexpected entry"
        raise PublicationError(f"{item_label} already exists as {kind}")

    def close(self) -> None:
        descriptors = getattr(self, "_descriptors", ())
        for descriptor in reversed(descriptors):
            os.close(descriptor)
        self._descriptors = []

    def __enter__(self) -> _AnchoredParent:
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()


def _read_bounded_payload(maximum_bytes: int) -> bytes:
    payload = sys.stdin.buffer.read(maximum_bytes + 1)
    if len(payload) > maximum_bytes:
        raise PublicationError(
            f"release marker exceeds the {maximum_bytes}-byte publication bound"
        )
    if not payload or not payload.endswith(b"\n") or b"\0" in payload:
        raise PublicationError(
            "release marker must be non-empty, NUL-free, and newline-terminated"
        )
    return payload


def _write_all(descriptor: int, payload: bytes) -> None:
    offset = 0
    while offset < len(payload):
        written = os.write(descriptor, payload[offset:])
        if written <= 0:
            raise PublicationError("release marker write made no progress")
        offset += written


def _remove_if_owned(
    parent: _AnchoredParent,
    name: str,
    identity: os.stat_result,
) -> None:
    try:
        observed = os.stat(
            name,
            dir_fd=parent.descriptor,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        return
    if observed.st_dev != identity.st_dev or observed.st_ino != identity.st_ino:
        raise PublicationError(
            f"{parent.label} changed while rolling back failed publication"
        )
    os.unlink(name, dir_fd=parent.descriptor)
    os.fsync(parent.descriptor)


def _publish_new(parent: _AnchoredParent, payload: bytes) -> os.stat_result:
    parent.require_absent(parent.leaf, item_label=parent.label)
    parent.require_absent(
        parent.temporary,
        item_label=f"{parent.label} temporary",
    )

    descriptor = -1
    temporary_created = False
    published: os.stat_result | None = None
    publication_complete = False
    try:
        descriptor = os.open(
            parent.temporary,
            _CREATE_FLAGS,
            0o600,
            dir_fd=parent.descriptor,
        )
        temporary_created = True
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
            raise PublicationError(
                f"{parent.label} temporary must be a singly-linked regular file"
            )
        _write_all(descriptor, payload)
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1

        # A hard-link publication is the portable no-replace equivalent of an
        # atomic same-directory rename. It cannot overwrite a raced target.
        os.link(
            parent.temporary,
            parent.leaf,
            src_dir_fd=parent.descriptor,
            dst_dir_fd=parent.descriptor,
            follow_symlinks=False,
        )
        published = opened
        observed = os.stat(
            parent.leaf,
            dir_fd=parent.descriptor,
            follow_symlinks=False,
        )
        if (
            not stat.S_ISREG(observed.st_mode)
            or observed.st_dev != opened.st_dev
            or observed.st_ino != opened.st_ino
            or observed.st_nlink != 2
        ):
            raise PublicationError(
                f"{parent.label} identity changed during atomic publication"
            )
        os.unlink(parent.temporary, dir_fd=parent.descriptor)
        temporary_created = False
        os.fsync(parent.descriptor)
        publication_complete = True
        return observed
    except OSError as error:
        raise PublicationError(
            f"cannot durably publish {parent.label}: {error.strerror}"
        ) from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if published is not None and not publication_complete:
            try:
                _remove_if_owned(parent, parent.leaf, published)
            except (OSError, PublicationError):
                pass
        if temporary_created:
            try:
                os.unlink(parent.temporary, dir_fd=parent.descriptor)
                os.fsync(parent.descriptor)
            except OSError:
                pass


def _remove_published(
    parent: _AnchoredParent,
    identity: os.stat_result | None,
) -> None:
    if identity is None:
        return
    _remove_if_owned(parent, parent.leaf, identity)


def publish_release_marker(
    *,
    output: Path,
    payload: bytes,
    pointer: Path | None,
) -> None:
    """Publish an optional durable pointer, then the terminal marker."""

    with _AnchoredParent(output, label="completion marker") as completion:
        pointer_parent: _AnchoredParent | None = None
        pointer_identity: os.stat_result | None = None
        try:
            completion.require_absent(
                completion.leaf,
                item_label=completion.label,
            )
            completion.require_absent(
                completion.temporary,
                item_label=f"{completion.label} temporary",
            )
            if pointer is not None:
                if pointer == output:
                    raise PublicationError(
                        "completion pointer and marker paths must be distinct"
                    )
                pointer_parent = _AnchoredParent(
                    pointer,
                    label="completion pointer",
                )
                pointer_parent.require_absent(
                    pointer_parent.leaf,
                    item_label=pointer_parent.label,
                )
                pointer_parent.require_absent(
                    pointer_parent.temporary,
                    item_label=f"{pointer_parent.label} temporary",
                )
                pointer_payload = f"{output}\n".encode("utf-8")
                if len(pointer_payload) > 8192:
                    raise PublicationError(
                        "completion pointer exceeds the 8192-byte publication bound"
                    )
                pointer_identity = _publish_new(pointer_parent, pointer_payload)
            _publish_new(completion, payload)
        except BaseException:
            if pointer_parent is not None:
                try:
                    _remove_published(pointer_parent, pointer_identity)
                except (OSError, PublicationError):
                    pass
            raise
        finally:
            if pointer_parent is not None:
                pointer_parent.close()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="durably publish a new bounded release completion marker"
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--pointer", type=Path)
    parser.add_argument("--maximum-bytes", type=int, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if not 1 <= args.maximum_bytes <= _MAXIMUM_ALLOWED_BYTES:
        print(
            f"--maximum-bytes must be between 1 and {_MAXIMUM_ALLOWED_BYTES}",
            file=sys.stderr,
        )
        return 2
    try:
        payload = _read_bounded_payload(args.maximum_bytes)
        publish_release_marker(
            output=args.output,
            payload=payload,
            pointer=args.pointer,
        )
    except PublicationError as error:
        print(f"release marker publication failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
