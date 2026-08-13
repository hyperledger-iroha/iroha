#!/usr/bin/env python3
"""Functionally authenticate the closed Sumeragi v2 release tool set.

The release bootstrap and runtime copier authenticate executable bytes before
they copy them.  That alone is insufficient on hosts which permit a system
binary at its installed location but kill an inode-independent copy at exec
time.  This component gives both callers one fixed, harmless functional probe
for every first-release command.  Probe arguments are code-owned, execution is
closed, tool bytes are authenticated before and after use, and the returned
record contains no filesystem paths.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import selectors
import signal
import stat
import subprocess
import sys
import time
from types import MappingProxyType
from typing import Any, Mapping


PROBE_FORMAT = "iroha-sumeragi-v2-release-tool-functional-probes"
PROBE_SCHEMA_VERSION = 1
MAXIMUM_MANIFEST_BYTES = 1024 * 1024
MAXIMUM_TOOL_BYTES = 512 * 1024 * 1024
MAXIMUM_OUTPUT_BYTES = 64 * 1024
COMMAND_TIMEOUT_SECONDS = 10

_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_ARCHIVE_ID_RE = re.compile(r"[a-z0-9][a-z0-9._:-]{0,191}")
_TEMPORARY_NAME_RE = re.compile(r"temporary\.[A-Za-z0-9]{6}")
_EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
_CARGO_VERSION = b"cargo 1.93.1 (083ac5135 2025-12-15)\n"
_RUSTC_VERSION = b"rustc 1.93.1 (01f6ddf75 2026-02-11)\n"
_TLAPM_VERSION = b"3ab43c7"
_VERUS_VERSION = b"0.2026.05.31.5dd6d83"

_SHELL_TOOL_NAMES = (
    "awk",
    "basename",
    "cat",
    "chmod",
    "cmp",
    "cp",
    "cut",
    "diff",
    "dirname",
    "env",
    "find",
    "grep",
    "ln",
    "ls",
    "mkdir",
    "mkfifo",
    "mktemp",
    "mv",
    "openssl",
    "rm",
    "rmdir",
    "sed",
    "sh",
    "sleep",
    "tail",
    "tee",
    "tr",
    "uname",
    "wc",
    "xargs",
    "shasum" if sys.platform == "darwin" else "sha256sum",
)
_LANGUAGE_TOOL_NAMES = (
    "cargo",
    "cargo-verus",
    "git-index-pack",
    "git-upload-pack",
    "java",
    "node",
    "rustc",
    "swift",
    "tlapm",
    "verus",
)
REQUIRED_TOOL_NAMES = tuple(sorted((*_SHELL_TOOL_NAMES, *_LANGUAGE_TOOL_NAMES)))

# Keep this literal table reviewable.  A producer cannot add an executable or
# select a weaker probe by changing its manifest.
PROBE_OPERATION_IDS = MappingProxyType(
    {
        "awk": "release-tool.awk-program.v1",
        "basename": "release-tool.basename-path.v1",
        "cargo": "release-tool.cargo-version.v1",
        "cargo-verus": "release-tool.cargo-verus-help.v1",
        "cat": "release-tool.cat-file.v1",
        "chmod": "release-tool.chmod-mode.v1",
        "cmp": "release-tool.cmp-different-quiet.v1",
        "cp": "release-tool.cp-file.v1",
        "cut": "release-tool.cut-byte.v1",
        "diff": "release-tool.diff-different-brief.v1",
        "dirname": "release-tool.dirname-path.v1",
        "env": "release-tool.env-closed.v1",
        "find": "release-tool.find-file.v1",
        "git-index-pack": "release-tool.git-index-pack-empty.v1",
        "git-upload-pack": "release-tool.git-upload-pack-missing.v1",
        "grep": "release-tool.grep-exact.v1",
        "java": "release-tool.java-version.v1",
        "ln": "release-tool.ln-hardlink.v1",
        "ls": "release-tool.ls-entry.v1",
        "mkdir": "release-tool.mkdir-directory.v1",
        "mkfifo": "release-tool.mkfifo-fifo.v1",
        "mktemp": "release-tool.mktemp-file.v1",
        "mv": "release-tool.mv-file.v1",
        "node": "release-tool.node-exec-path.v1",
        "openssl": "release-tool.openssl-sha256.v1",
        "rm": "release-tool.rm-file.v1",
        "rmdir": "release-tool.rmdir-directory.v1",
        "rustc": "release-tool.rustc-version.v1",
        "sed": "release-tool.sed-first-line.v1",
        "sh": "release-tool.sh-builtin-output.v1",
        ("shasum" if sys.platform == "darwin" else "sha256sum"): (
            "release-tool.shasum-empty.v1"
            if sys.platform == "darwin"
            else "release-tool.sha256sum-empty.v1"
        ),
        "sleep": "release-tool.sleep-duration.v1",
        "swift": "release-tool.swift-version.v1",
        "tail": "release-tool.tail-last-line.v1",
        "tee": "release-tool.tee-file.v1",
        "tlapm": "release-tool.tlapm-version.v1",
        "tr": "release-tool.tr-byte.v1",
        "uname": "release-tool.uname-system.v1",
        "verus": "release-tool.verus-version.v1",
        "wc": "release-tool.wc-empty.v1",
        "xargs": "release-tool.xargs-protected-shell.v1",
    }
)

_EXPECTED_EXIT_STATUSES = MappingProxyType(
    {
        name: (
            (128,)
            if name in {"git-index-pack", "git-upload-pack"}
            else (1,)
            if name in {"cmp", "diff"}
            else (0,)
        )
        for name in REQUIRED_TOOL_NAMES
    }
)


class ToolProbeError(RuntimeError):
    """One protected release tool failed its closed functional contract."""


@dataclass(frozen=True)
class ToolSnapshot:
    """Stable identity and content of one protected regular executable."""

    path: Path
    device: int
    inode: int
    mode: int
    owner: int
    nlink: int
    size: int
    mtime_ns: int
    ctime_ns: int
    sha256: str
    prefix: bytes


@dataclass(frozen=True)
class ProbeInvocation:
    """One code-owned invocation and its accepted process status."""

    arguments: tuple[str, ...]
    stdin: bytes = b""


@dataclass(frozen=True)
class ProbeExecution:
    """Bounded raw process result used only inside the private probe."""

    status: int
    stdout: bytes
    stderr: bytes
    duration_ns: int


@dataclass(frozen=True)
class ProbeContext:
    """Private paths and authenticated tools for one functional probe."""

    root: Path
    work: Path
    tools: Mapping[str, ToolSnapshot]
    environment: Mapping[str, str]


def canonical_json(value: Any) -> bytes:
    """Render the canonical ASCII JSON representation used by probe receipts."""

    return (
        json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        )
        + "\n"
    ).encode("ascii")


def _host_family() -> str:
    if sys.platform == "darwin":
        return "darwin"
    if sys.platform.startswith("linux"):
        return "linux"
    raise ToolProbeError("release tool probes support only Darwin and Linux hosts")


def _same_file(left: os.stat_result, right: os.stat_result) -> bool:
    fields = (
        "st_dev",
        "st_ino",
        "st_mode",
        "st_uid",
        "st_nlink",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
    )
    return all(getattr(left, field) == getattr(right, field) for field in fields)


def _snapshot_tool(path: Path, name: str) -> ToolSnapshot:
    if (
        not path.is_absolute()
        or Path(os.path.abspath(path)) != path
        or path.name in {"", ".", ".."}
    ):
        raise ToolProbeError(f"release tool {name} path is not absolute and normalized")
    try:
        parent = path.parent.lstat()
        before = path.lstat()
        resolved_parent = path.parent.resolve(strict=True)
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise ToolProbeError(f"release tool {name} is unavailable") from error
    parent_before = parent
    if (
        resolved_parent != path.parent
        or stat.S_ISLNK(parent.st_mode)
        or not stat.S_ISDIR(parent.st_mode)
        or parent.st_uid != os.geteuid()
        or stat.S_IMODE(parent.st_mode) != 0o700
        or resolved != path
        or stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != 0o500
        or before.st_size > MAXIMUM_TOOL_BYTES
    ):
        raise ToolProbeError(f"release tool {name} metadata is unsafe")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ToolProbeError(f"release tool {name} could not be opened safely") from error
    digest = hashlib.sha256()
    total = 0
    prefix = bytearray()
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or not _same_file(before, opened):
            raise ToolProbeError(f"release tool {name} changed while opened")
        while True:
            block = os.read(descriptor, 1024 * 1024)
            if not block:
                break
            if len(prefix) < 4096:
                prefix.extend(block[: 4096 - len(prefix)])
            total += len(block)
            if total > MAXIMUM_TOOL_BYTES:
                raise ToolProbeError(f"release tool {name} exceeds its byte bound")
            digest.update(block)
        after = os.fstat(descriptor)
        if total != opened.st_size or not _same_file(opened, after):
            raise ToolProbeError(f"release tool {name} changed while authenticated")
    finally:
        os.close(descriptor)
    try:
        path_after = path.lstat()
        parent_after = path.parent.lstat()
    except OSError as error:
        raise ToolProbeError(f"release tool {name} disappeared after authentication") from error
    if not _same_file(after, path_after) or not _same_file(
        parent_before, parent_after
    ):
        raise ToolProbeError(f"release tool {name} pathname changed during authentication")
    return ToolSnapshot(
        path=path,
        device=after.st_dev,
        inode=after.st_ino,
        mode=stat.S_IMODE(after.st_mode),
        owner=after.st_uid,
        nlink=after.st_nlink,
        size=after.st_size,
        mtime_ns=after.st_mtime_ns,
        ctime_ns=after.st_ctime_ns,
        sha256=digest.hexdigest(),
        prefix=bytes(prefix),
    )


def _reject_script(snapshot: ToolSnapshot, name: str) -> None:
    if not snapshot.prefix.startswith(b"#!"):
        return
    first_line = snapshot.prefix.split(b"\n", 1)[0][2:].strip()
    try:
        interpreter = first_line.split(None, 1)[0].decode("ascii")
    except (IndexError, UnicodeDecodeError) as error:
        raise ToolProbeError(f"release tool {name} has a malformed shebang") from error
    if interpreter.startswith("/"):
        raise ToolProbeError(
            f"release tool {name} has a forbidden absolute shebang interpreter"
        )
    raise ToolProbeError(
        f"release tool {name} has an unlisted shebang interpreter"
    )


def _require_same(snapshot: ToolSnapshot, name: str) -> None:
    try:
        current = _snapshot_tool(snapshot.path, name)
    except ToolProbeError as error:
        raise ToolProbeError(
            f"release tool {name} changed across its functional probe"
        ) from error
    if current != snapshot:
        raise ToolProbeError(f"release tool {name} changed across its functional probe")


def _prepare_probe_root(path: Path) -> None:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ToolProbeError("release tool probe root is not absolute and normalized")
    try:
        parent = path.parent.lstat()
    except OSError as error:
        raise ToolProbeError("release tool probe parent is unavailable") from error
    if (
        path.parent.resolve(strict=True) != path.parent
        or stat.S_ISLNK(parent.st_mode)
        or not stat.S_ISDIR(parent.st_mode)
        or parent.st_uid != os.geteuid()
        or stat.S_IMODE(parent.st_mode) != 0o700
        or path.exists()
        or path.is_symlink()
    ):
        raise ToolProbeError("release tool probe root or parent is unsafe")
    try:
        path.mkdir(mode=0o700)
    except OSError as error:
        raise ToolProbeError("release tool probe root could not be created") from error
    try:
        metadata = path.lstat()
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != 0o700
        ):
            raise ToolProbeError(
                "release tool probe root was not created owner-private"
            )
    except BaseException:
        try:
            path.rmdir()
        except OSError as cleanup_error:
            raise ToolProbeError(
                "unsafe release tool probe root could not be reclaimed"
            ) from cleanup_error
        raise


def _remove_probe_root(path: Path) -> None:
    """Remove the exact quiescent owner-private root without external tools."""

    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        parent_flags |= os.O_NOFOLLOW
    parent_fd = os.open(path.parent, parent_flags)
    try:
        expected = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
        if (
            stat.S_ISLNK(expected.st_mode)
            or not stat.S_ISDIR(expected.st_mode)
            or expected.st_uid != os.geteuid()
        ):
            raise ToolProbeError("release tool probe cleanup root is unsafe")
        root_fd = os.open(path.name, parent_flags, dir_fd=parent_fd)

        def remove_children(directory_fd: int, depth: int = 0) -> None:
            for name in sorted(os.listdir(directory_fd)):
                metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if metadata.st_uid != os.geteuid():
                    raise ToolProbeError("release tool probe cleanup entry has the wrong owner")
                if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                    child = os.open(name, parent_flags, dir_fd=directory_fd)
                    try:
                        opened = os.fstat(child)
                        if (opened.st_dev, opened.st_ino) != (
                            metadata.st_dev,
                            metadata.st_ino,
                        ):
                            raise ToolProbeError("release tool probe cleanup entry changed")
                        if stat.S_IMODE(opened.st_mode) & 0o700 != 0o700:
                            os.fchmod(child, stat.S_IMODE(opened.st_mode) | 0o700)
                        remove_children(child, depth + 1)
                    finally:
                        os.close(child)
                    current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                    if (current.st_dev, current.st_ino) != (
                        metadata.st_dev,
                        metadata.st_ino,
                    ):
                        raise ToolProbeError("release tool probe cleanup directory changed")
                    os.rmdir(name, dir_fd=directory_fd)
                else:
                    current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                    if (current.st_dev, current.st_ino) != (
                        metadata.st_dev,
                        metadata.st_ino,
                    ):
                        raise ToolProbeError("release tool probe cleanup entry changed")
                    os.unlink(name, dir_fd=directory_fd)

        try:
            opened_root = os.fstat(root_fd)
            if (opened_root.st_dev, opened_root.st_ino) != (
                expected.st_dev,
                expected.st_ino,
            ):
                raise ToolProbeError("release tool probe cleanup root changed")
            if stat.S_IMODE(opened_root.st_mode) & 0o700 != 0o700:
                os.fchmod(root_fd, stat.S_IMODE(opened_root.st_mode) | 0o700)
            remove_children(root_fd)
        finally:
            os.close(root_fd)
        current_root = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
        if (current_root.st_dev, current_root.st_ino) != (
            expected.st_dev,
            expected.st_ino,
        ):
            raise ToolProbeError("release tool probe cleanup root was replaced")
        os.rmdir(path.name, dir_fd=parent_fd)
    finally:
        os.close(parent_fd)


def _write_probe_file(path: Path, data: bytes, mode: int = 0o600) -> Path:
    descriptor = os.open(
        path,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        mode,
    )
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise ToolProbeError("release tool probe file write did not progress")
            view = view[written:]
        os.fchmod(descriptor, mode)
    finally:
        os.close(descriptor)
    return path


def _prepare_invocation(name: str, context: ProbeContext) -> ProbeInvocation:
    work = context.work
    if name == "awk":
        return ProbeInvocation(("BEGIN { print \"probe-ok\"; exit 0 }",))
    if name == "basename":
        return ProbeInvocation(("alpha/beta",))
    if name == "cat":
        _write_probe_file(work / "input", b"probe\n")
        return ProbeInvocation(("input",))
    if name == "chmod":
        _write_probe_file(work / "mode-target", b"mode\n")
        return ProbeInvocation(("0400", "mode-target"))
    if name in {"cmp", "diff"}:
        _write_probe_file(work / "left", b"equal\n")
        _write_probe_file(work / "right", b"different\n")
        return ProbeInvocation(
            ("-s", "left", "right")
            if name == "cmp"
            else ("--brief", "left", "right")
        )
    if name == "cp":
        _write_probe_file(work / "copy-source", b"copied\n")
        return ProbeInvocation(("copy-source", "copy-target"))
    if name == "cut":
        return ProbeInvocation(("-b", "2"), b"abc\n")
    if name == "dirname":
        return ProbeInvocation(("alpha/beta",))
    if name == "env":
        return ProbeInvocation(("-i", "RELEASE_PROBE=closed"))
    if name == "find":
        (work / "find-root").mkdir(mode=0o700)
        _write_probe_file(work / "find-root" / "probe-file", b"found\n")
        return ProbeInvocation(
            ("find-root", "-type", "f", "-name", "probe-file", "-print")
        )
    if name == "grep":
        _write_probe_file(work / "grep-input", b"probe\n")
        return ProbeInvocation(("-F", "-x", "probe", "grep-input"))
    if name == "ln":
        _write_probe_file(work / "link-source", b"linked\n")
        return ProbeInvocation(("link-source", "link-target"))
    if name == "ls":
        _write_probe_file(work / "listed", b"")
        return ProbeInvocation(("-d", "listed"))
    if name == "mkdir":
        return ProbeInvocation(("made",))
    if name == "mkfifo":
        return ProbeInvocation(("fifo",))
    if name == "mktemp":
        return ProbeInvocation(("temporary.XXXXXX",))
    if name == "mv":
        _write_probe_file(work / "move-source", b"moved\n")
        return ProbeInvocation(("move-source", "move-target"))
    if name == "openssl":
        return ProbeInvocation(("dgst", "-sha256", "-binary"))
    if name == "rm":
        _write_probe_file(work / "removed", b"remove\n")
        return ProbeInvocation(("-f", "removed"))
    if name == "rmdir":
        (work / "removed-directory").mkdir(mode=0o700)
        return ProbeInvocation(("removed-directory",))
    if name == "sed":
        _write_probe_file(work / "sed-input", b"first\nsecond\n")
        return ProbeInvocation(("-n", "1p", "sed-input"))
    if name == "sh":
        return ProbeInvocation(("-c", "printf '%s\\n' probe-ok"))
    if name == "sleep":
        return ProbeInvocation(("1",))
    if name == "tail":
        _write_probe_file(work / "tail-input", b"first\nlast\n")
        return ProbeInvocation(("-n", "1", "tail-input"))
    if name == "tee":
        return ProbeInvocation(("tee-output",), b"tee-probe\n")
    if name == "tr":
        return ProbeInvocation(("a", "b"), b"a\n")
    if name == "uname":
        return ProbeInvocation(("-s",))
    if name == "wc":
        return ProbeInvocation(("-c",))
    if name == "xargs":
        return ProbeInvocation(
            (
                "-0",
                str(context.tools["sh"].path),
                "-c",
                "test \"$1\" = probe && printf '%s\\n' xargs-ok",
                "probe-sh",
            ),
            b"probe\0",
        )
    if name == "shasum":
        return ProbeInvocation(("-a", "256"))
    if name == "sha256sum":
        return ProbeInvocation(())
    if name in {"cargo", "rustc", "swift", "tlapm", "verus"}:
        return ProbeInvocation(("--version",))
    if name == "cargo-verus":
        return ProbeInvocation(("--help",))
    if name == "git-index-pack":
        _write_probe_file(work / "empty.pack", b"")
        return ProbeInvocation(("empty.pack",))
    if name == "git-upload-pack":
        return ProbeInvocation(("--strict", "missing.git"))
    if name == "java":
        return ProbeInvocation(("-version",))
    if name == "node":
        return ProbeInvocation(("-e", "process.stdout.write(process.execPath+'\\n')"))
    raise ToolProbeError("release tool probe table is incomplete")


def _run_bounded(
    executable: Path,
    invocation: ProbeInvocation,
    *,
    work: Path,
    environment: Mapping[str, str],
) -> ProbeExecution:
    if len(invocation.stdin) > 4096:
        raise ToolProbeError("release tool probe stdin exceeds its bound")
    selector: selectors.BaseSelector | None = None
    started_ns = time.monotonic_ns()
    try:
        process = subprocess.Popen(
            [str(executable), *invocation.arguments],
            cwd=work,
            env=dict(environment),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            start_new_session=True,
            umask=0o077,
        )
    except OSError as error:
        raise ToolProbeError("release tool could not be executed from its archive") from error
    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None
    try:
        try:
            process.stdin.write(invocation.stdin)
            process.stdin.close()
        except BrokenPipeError:
            try:
                process.stdin.close()
            except BrokenPipeError:
                pass
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ, "stdout")
        selector.register(process.stderr, selectors.EVENT_READ, "stderr")
        buffers = {"stdout": bytearray(), "stderr": bytearray()}
        totals = {"stdout": 0, "stderr": 0}
        oversized = False
        deadline = time.monotonic() + COMMAND_TIMEOUT_SECONDS
        while selector.get_map() or process.poll() is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except (PermissionError, ProcessLookupError):
                    try:
                        process.kill()
                    except ProcessLookupError:
                        pass
                process.wait()
                raise ToolProbeError("release tool functional probe exceeded its runtime bound")
            for key, _ in selector.select(min(remaining, 0.1)):
                available = MAXIMUM_OUTPUT_BYTES - sum(totals.values())
                block = os.read(key.fd, min(65536, max(1, available + 1)))
                if not block:
                    selector.unregister(key.fileobj)
                    continue
                label = key.data
                totals[label] += len(block)
                if sum(totals.values()) > MAXIMUM_OUTPUT_BYTES:
                    oversized = True
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except (PermissionError, ProcessLookupError):
                        try:
                            process.kill()
                        except ProcessLookupError:
                            pass
                    process.wait()
                    raise ToolProbeError(
                        "release tool functional probe exceeded its output bound"
                    )
                else:
                    buffers[label].extend(block)
        status = process.wait()
        if oversized:
            raise ToolProbeError(
                "release tool functional probe exceeded its output bound"
            )
        return ProbeExecution(
            status,
            bytes(buffers["stdout"]),
            bytes(buffers["stderr"]),
            time.monotonic_ns() - started_ns,
        )
    finally:
        if selector is not None:
            selector.close()
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except (PermissionError, ProcessLookupError):
                try:
                    process.kill()
                except ProcessLookupError:
                    pass
            process.wait()
        if not process.stdin.closed:
            try:
                process.stdin.close()
            except BrokenPipeError:
                pass
        process.stdout.close()
        process.stderr.close()


def _regular_file(
    path: Path,
    *,
    data: bytes | None = None,
    expected_mode: int = 0o600,
) -> dict[str, Any]:
    try:
        before = path.lstat()
    except OSError as error:
        raise ToolProbeError("release tool probe postcondition file is absent") from error
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ToolProbeError("release tool probe postcondition file is unsafe") from error
    digest = hashlib.sha256()
    contents = bytearray()
    try:
        opened = os.fstat(descriptor)
        if not _same_file(before, opened):
            raise ToolProbeError("release tool probe postcondition file changed")
        while True:
            block = os.read(descriptor, 65536)
            if not block:
                break
            contents.extend(block)
            if len(contents) > 4096:
                raise ToolProbeError(
                    "release tool probe postcondition exceeds its byte bound"
                )
            digest.update(block)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        path_after = path.lstat()
    except OSError as error:
        raise ToolProbeError("release tool probe postcondition file disappeared") from error
    if (
        stat.S_ISLNK(after.st_mode)
        or not stat.S_ISREG(after.st_mode)
        or after.st_uid != os.geteuid()
        or after.st_nlink != 1
        or stat.S_IMODE(after.st_mode) != expected_mode
        or len(contents) != after.st_size
        or not _same_file(opened, after)
        or not _same_file(after, path_after)
        or (data is not None and bytes(contents) != data)
    ):
        raise ToolProbeError("release tool probe postcondition file is unsafe")
    return {
        "kind": "regular-file",
        "mode": f"{stat.S_IMODE(after.st_mode):04o}",
        "sha256": digest.hexdigest(),
        "size_bytes": after.st_size,
    }


def _no_output(execution: ProbeExecution) -> None:
    if execution.stdout or execution.stderr:
        raise ToolProbeError("release tool probe produced unexpected output")


def _validate_execution(
    name: str,
    context: ProbeContext,
    execution: ProbeExecution,
) -> tuple[bytes, bytes, dict[str, Any]]:
    if execution.status not in _EXPECTED_EXIT_STATUSES[name]:
        if execution.status < 0:
            raise ToolProbeError(
                f"release tool {name} was terminated by signal {-execution.status}"
            )
        raise ToolProbeError(
            f"release tool {name} returned unexpected status {execution.status}"
        )
    stdout = execution.stdout
    stderr = execution.stderr
    postcondition: dict[str, Any] = {"kind": "status", "status": execution.status}
    if name == "awk" and (stdout, stderr) != (b"probe-ok\n", b""):
        raise ToolProbeError("release tool awk failed its program probe")
    elif name == "basename" and (stdout, stderr) != (b"beta\n", b""):
        raise ToolProbeError("release tool basename failed its path probe")
    elif name == "cat" and (stdout, stderr) != (b"probe\n", b""):
        raise ToolProbeError("release tool cat failed its file probe")
    elif name == "chmod":
        _no_output(execution)
        metadata = (context.work / "mode-target").lstat()
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o400
        ):
            raise ToolProbeError("release tool chmod did not apply the exact mode")
        postcondition = {"kind": "file-mode", "mode": "0400"}
    elif name == "cmp":
        _no_output(execution)
        postcondition = {"kind": "different-files", "status": 1}
    elif name == "diff":
        if stdout != b"Files left and right differ\n" or stderr:
            raise ToolProbeError("release tool diff failed its difference probe")
        postcondition = {"kind": "different-files", "status": 1}
    elif name == "env" and (stdout, stderr) != (b"RELEASE_PROBE=closed\n", b""):
        raise ToolProbeError("release tool env failed its empty-environment probe")
    elif name == "find" and (stdout, stderr) != (b"find-root/probe-file\n", b""):
        raise ToolProbeError("release tool find failed its traversal probe")
    elif name == "grep" and (stdout, stderr) != (b"probe\n", b""):
        raise ToolProbeError("release tool grep failed its match probe")
    elif name == "sh" and (stdout, stderr) != (b"probe-ok\n", b""):
        raise ToolProbeError("release tool sh failed its builtin probe")
    elif name == "sleep":
        _no_output(execution)
        if execution.duration_ns < 900_000_000:
            raise ToolProbeError("release tool sleep returned before its minimum duration")
        postcondition = {"kind": "minimum-duration", "minimum_milliseconds": 900}
    elif name == "tail" and (stdout, stderr) != (b"last\n", b""):
        raise ToolProbeError("release tool tail failed its line probe")
    elif name == "xargs" and (stdout, stderr) != (b"xargs-ok\n", b""):
        raise ToolProbeError("release tool xargs failed its argument probe")
    elif name == "cp":
        _no_output(execution)
        postcondition = _regular_file(
            context.work / "copy-target", data=b"copied\n"
        )
    elif name == "cut" and (stdout, stderr) != (b"b\n", b""):
        raise ToolProbeError("release tool cut failed its byte probe")
    elif name == "dirname" and (stdout, stderr) != (b"alpha\n", b""):
        raise ToolProbeError("release tool dirname failed its path probe")
    elif name == "ln":
        _no_output(execution)
        source = (context.work / "link-source").lstat()
        target = (context.work / "link-target").lstat()
        if (
            not stat.S_ISREG(target.st_mode)
            or source.st_uid != os.geteuid()
            or target.st_uid != os.geteuid()
            or stat.S_IMODE(source.st_mode) != 0o600
            or stat.S_IMODE(target.st_mode) != 0o600
            or (source.st_dev, source.st_ino) != (target.st_dev, target.st_ino)
            or source.st_nlink != 2
            or target.st_nlink != 2
        ):
            raise ToolProbeError("release tool ln did not create one exact hard link")
        postcondition = {"kind": "hard-link", "nlink": 2, "same_inode": True}
    elif name == "ls" and (stdout, stderr) != (b"listed\n", b""):
        raise ToolProbeError("release tool ls failed its entry probe")
    elif name == "mkdir":
        _no_output(execution)
        metadata = (context.work / "made").lstat()
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != 0o700
        ):
            raise ToolProbeError("release tool mkdir did not create an owner-private directory")
        postcondition = {"kind": "directory", "mode": "0700"}
    elif name == "mkfifo":
        _no_output(execution)
        metadata = (context.work / "fifo").lstat()
        if (
            not stat.S_ISFIFO(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise ToolProbeError("release tool mkfifo did not create an owner-private FIFO")
        postcondition = {"kind": "fifo", "mode": "0600"}
    elif name == "mktemp":
        if stderr:
            raise ToolProbeError("release tool mktemp wrote diagnostics")
        if not stdout.endswith(b"\n") or stdout.count(b"\n") != 1:
            raise ToolProbeError("release tool mktemp returned an unsafe line")
        try:
            rendered = stdout[:-1].decode("ascii")
        except UnicodeDecodeError as error:
            raise ToolProbeError("release tool mktemp returned non-ASCII output") from error
        if _TEMPORARY_NAME_RE.fullmatch(rendered) is None:
            raise ToolProbeError("release tool mktemp returned an unsafe name")
        postcondition = _regular_file(context.work / rendered, data=b"")
        stdout = b"temporary.<TOKEN>\n"
    elif name == "mv":
        _no_output(execution)
        if (context.work / "move-source").exists() or (context.work / "move-source").is_symlink():
            raise ToolProbeError("release tool mv retained its source")
        postcondition = _regular_file(
            context.work / "move-target", data=b"moved\n"
        )
    elif name == "openssl":
        if stdout != hashlib.sha256(b"").digest() or stderr:
            raise ToolProbeError("release tool openssl failed its SHA-256 probe")
    elif name == "rm":
        _no_output(execution)
        if (context.work / "removed").exists() or (context.work / "removed").is_symlink():
            raise ToolProbeError("release tool rm retained its target")
        postcondition = {"kind": "absent", "entry": "removed"}
    elif name == "rmdir":
        _no_output(execution)
        removed_directory = context.work / "removed-directory"
        if removed_directory.exists() or removed_directory.is_symlink():
            raise ToolProbeError("release tool rmdir retained its target")
        postcondition = {"kind": "absent", "entry": "removed-directory"}
    elif name == "sed" and (stdout, stderr) != (b"first\n", b""):
        raise ToolProbeError("release tool sed failed its line probe")
    elif name == "tee":
        if (stdout, stderr) != (b"tee-probe\n", b""):
            raise ToolProbeError("release tool tee failed its stream probe")
        postcondition = _regular_file(
            context.work / "tee-output", data=b"tee-probe\n"
        )
    elif name == "tr" and (stdout, stderr) != (b"b\n", b""):
        raise ToolProbeError("release tool tr failed its translation probe")
    elif name == "uname":
        expected = (platform.system() + "\n").encode("ascii")
        if (stdout, stderr) != (expected, b""):
            raise ToolProbeError("release tool uname disagrees with the admitted host")
        stdout = b"<ADMITTED-HOST>\n"
    elif name == "wc":
        if stdout.strip() != b"0" or stderr:
            raise ToolProbeError("release tool wc failed its empty-input probe")
        stdout = b"0\n"
    elif name in {"shasum", "sha256sum"}:
        if stdout != (_EMPTY_SHA256 + "  -\n").encode("ascii") or stderr:
            raise ToolProbeError(f"release tool {name} failed its SHA-256 probe")
        stdout = (_EMPTY_SHA256 + "\n").encode("ascii")
    elif name == "cargo":
        if stdout != _CARGO_VERSION or stderr:
            raise ToolProbeError("release tool cargo failed its version probe")
    elif name == "rustc":
        if stdout != _RUSTC_VERSION or stderr:
            raise ToolProbeError("release tool rustc failed its version probe")
    elif name == "cargo-verus":
        combined = (stdout + stderr).lower()
        if not combined or not (b"cargo" in combined and b"verus" in combined):
            raise ToolProbeError("release tool cargo-verus failed its help probe")
    elif name == "git-index-pack":
        if stdout or b"early eof" not in stderr.lower():
            raise ToolProbeError("release tool git-index-pack failed its empty-pack probe")
    elif name == "git-upload-pack":
        lowered = stderr.lower()
        if stdout or not (
            b"not appear to be a git repository" in lowered
            or b"not a git repository" in lowered
        ):
            raise ToolProbeError(
                "release tool git-upload-pack failed its missing-repository probe"
            )
    elif name == "java":
        combined = (stdout + stderr).lower()
        if b"openjdk" not in combined or b"version" not in combined:
            raise ToolProbeError("release tool java failed its version probe")
    elif name == "node":
        expected = os.fsencode(context.tools[name].path) + b"\n"
        if stdout != expected or stderr:
            raise ToolProbeError("release tool node did not report its archived executable")
    elif name == "swift":
        combined = (stdout + stderr).lower()
        if b"swift" not in combined or b"version" not in combined:
            raise ToolProbeError("release tool swift failed its version probe")
    elif name == "tlapm":
        if (stdout, stderr) not in {
            (_TLAPM_VERSION, b""),
            (_TLAPM_VERSION + b"\n", b""),
            (b"", _TLAPM_VERSION),
            (b"", _TLAPM_VERSION + b"\n"),
        }:
            raise ToolProbeError("release tool tlapm failed its version probe")
    elif name == "verus":
        combined = (stdout + stderr).lower()
        if b"verus" not in combined or _VERUS_VERSION not in combined:
            raise ToolProbeError("release tool verus failed its version probe")
    return stdout, stderr, postcondition


def _sanitize_output(data: bytes, context: ProbeContext) -> bytes:
    replacements = [(os.fsencode(context.root), b"<PROBE-ROOT>")]
    replacements.extend(
        (os.fsencode(snapshot.path), f"<TOOL:{name}>".encode("ascii"))
        for name, snapshot in context.tools.items()
    )
    for original, replacement in sorted(replacements, key=lambda item: -len(item[0])):
        data = data.replace(original, replacement)
    return data


def _normalized_argument(argument: str, context: ProbeContext) -> str:
    for name, snapshot in context.tools.items():
        if argument == str(snapshot.path):
            return f"<TOOL:{name}>"
    candidate = Path(argument)
    if candidate.is_absolute():
        try:
            return "<PROBE-ROOT>/" + candidate.relative_to(context.root).as_posix()
        except ValueError:
            return "<ABSOLUTE-ARGUMENT>"
    return argument


def _normalized_environment(context: ProbeContext) -> dict[str, str]:
    normalized: dict[str, str] = {}
    for name, value in context.environment.items():
        if value == os.devnull:
            normalized[name] = "<NULL-DEVICE>"
            continue
        if value == str(context.root):
            normalized[name] = "<PROBE-ROOT>"
            continue
        prefix = str(context.root) + os.sep
        if value.startswith(prefix):
            normalized[name] = (
                "<PROBE-ROOT>/" + Path(value).relative_to(context.root).as_posix()
            )
            continue
        normalized[name] = value
    return normalized


def _probe_contract_sha256(
    invocation_records: Mapping[str, Mapping[str, Any]],
) -> str:
    return hashlib.sha256(
        canonical_json(
            {
                "closed_environment": True,
                "native_executables_only": True,
                "postcondition_contract": "release-tool-specific-v1",
                "probe_umask": "0077",
                "reauthentication": "before-after-and-final",
                "schema_version": PROBE_SCHEMA_VERSION,
                "tools": {
                    name: invocation_records[name]
                    for name in REQUIRED_TOOL_NAMES
                },
            }
        )
    ).hexdigest()


def probe_release_tool_closure(
    tool_paths: Mapping[str, Path],
    probe_root: Path,
    *,
    expected_sha256: Mapping[str, str] | None = None,
    archive_ids: Mapping[str, str] | None = None,
) -> dict[str, Any]:
    """Execute the exact protected 41-tool closure and return a path-free record.

    ``tool_paths`` must name the protected regular archives, not PATH aliases.
    ``probe_root`` must be a new child of an exact 0700 owner-owned directory;
    it is reclaimed before this function returns or raises.
    """

    host_family = _host_family()
    if set(PROBE_OPERATION_IDS) != set(REQUIRED_TOOL_NAMES):
        raise ToolProbeError("release tool probe operation table is not exact")
    if set(tool_paths) != set(REQUIRED_TOOL_NAMES) or len(tool_paths) != 41:
        raise ToolProbeError("release tool closure does not contain exactly 41 commands")
    if expected_sha256 is not None and set(expected_sha256) != set(REQUIRED_TOOL_NAMES):
        raise ToolProbeError("release tool expected-digest inventory is not exact")
    if archive_ids is None:
        effective_archive_ids = {
            name: f"release-tool.{name}.v1" for name in REQUIRED_TOOL_NAMES
        }
    else:
        if set(archive_ids) != set(REQUIRED_TOOL_NAMES):
            raise ToolProbeError("release tool archive-ID inventory is not exact")
        effective_archive_ids = dict(archive_ids)
    if any(
        not isinstance(value, str) or _ARCHIVE_ID_RE.fullmatch(value) is None
        for value in effective_archive_ids.values()
    ):
        raise ToolProbeError("release tool archive ID is invalid")
    if len(set(effective_archive_ids.values())) != len(REQUIRED_TOOL_NAMES):
        raise ToolProbeError("release tool archive IDs are not unique")

    snapshots: dict[str, ToolSnapshot] = {}
    inodes: set[tuple[int, int]] = set()
    for name in REQUIRED_TOOL_NAMES:
        raw_path = tool_paths[name]
        if not isinstance(raw_path, Path):
            raise ToolProbeError(f"release tool {name} path is not a Path")
        snapshot = _snapshot_tool(raw_path, name)
        _reject_script(snapshot, name)
        if expected_sha256 is not None:
            expected = expected_sha256[name]
            if not isinstance(expected, str) or _DIGEST_RE.fullmatch(expected) is None:
                raise ToolProbeError(f"release tool {name} expected digest is invalid")
            if snapshot.sha256 != expected:
                raise ToolProbeError(f"release tool {name} digest does not match policy")
        inode = (snapshot.device, snapshot.inode)
        if inode in inodes:
            raise ToolProbeError("release tool closure contains an executable inode alias")
        inodes.add(inode)
        snapshots[name] = snapshot

    _prepare_probe_root(probe_root)
    result: dict[str, Any] | None = None
    try:
        alias_root = probe_root / "bin"
        home = probe_root / "home"
        temporary = probe_root / "tmp"
        work_root = probe_root / "work"
        for directory in (alias_root, home, temporary, work_root):
            directory.mkdir(mode=0o700)
        for name in REQUIRED_TOOL_NAMES:
            target = os.path.relpath(snapshots[name].path, alias_root)
            os.symlink(target, alias_root / name)
        environment = {
            "GIT_CONFIG_COUNT": "2",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_KEY_0": "core.hooksPath",
            "GIT_CONFIG_KEY_1": "core.fsmonitor",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_VALUE_0": os.devnull,
            "GIT_CONFIG_VALUE_1": "false",
            "GIT_TERMINAL_PROMPT": "0",
            "HOME": str(home),
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": str(alias_root),
            "TMPDIR": str(temporary),
            "TZ": "UTC",
        }
        tool_results: dict[str, dict[str, Any]] = {}
        invocation_records: dict[str, dict[str, Any]] = {}
        for name in REQUIRED_TOOL_NAMES:
            work = work_root / name
            work.mkdir(mode=0o700)
            context = ProbeContext(probe_root, work, snapshots, environment)
            _require_same(snapshots[name], name)
            dependencies = ("sh",) if name == "xargs" else ()
            for dependency in dependencies:
                _require_same(snapshots[dependency], dependency)
            invocation = _prepare_invocation(name, context)
            invocation_record = {
                "argv": [
                    f"<TOOL:{name}>",
                    *(
                        _normalized_argument(argument, context)
                        for argument in invocation.arguments
                    ),
                ],
                "closed_environment": _normalized_environment(context),
                "expected_exit_statuses": list(_EXPECTED_EXIT_STATUSES[name]),
                "maximum_output_bytes": MAXIMUM_OUTPUT_BYTES,
                "operation_id": PROBE_OPERATION_IDS[name],
                "stdin_sha256": hashlib.sha256(invocation.stdin).hexdigest(),
                "stdin_size_bytes": len(invocation.stdin),
                "timeout_seconds": COMMAND_TIMEOUT_SECONDS,
            }
            invocation_records[name] = invocation_record
            try:
                execution = _run_bounded(
                    snapshots[name].path,
                    invocation,
                    work=work,
                    environment=environment,
                )
                stdout, stderr, postcondition = _validate_execution(
                    name, context, execution
                )
            finally:
                _require_same(snapshots[name], name)
                for dependency in dependencies:
                    _require_same(snapshots[dependency], dependency)
            stdout = _sanitize_output(stdout, context)
            stderr = _sanitize_output(stderr, context)
            tool_results[name] = {
                "archive_id": effective_archive_ids[name],
                "exit_status": execution.status,
                "invocation_sha256": hashlib.sha256(
                    canonical_json(invocation_record)
                ).hexdigest(),
                "mode": "0500",
                "operation_id": PROBE_OPERATION_IDS[name],
                "postcondition_sha256": hashlib.sha256(
                    canonical_json(postcondition)
                ).hexdigest(),
                "sha256": snapshots[name].sha256,
                "size_bytes": snapshots[name].size,
                "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
                "stderr_size_bytes": len(stderr),
                "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
                "stdout_size_bytes": len(stdout),
            }
        for name in REQUIRED_TOOL_NAMES:
            _require_same(snapshots[name], name)
            alias = alias_root / name
            metadata = alias.lstat()
            expected_target = os.path.relpath(snapshots[name].path, alias_root)
            if (
                not stat.S_ISLNK(metadata.st_mode)
                or metadata.st_uid != os.geteuid()
                or metadata.st_nlink != 1
                or os.readlink(alias) != expected_target
                or alias.resolve(strict=True) != snapshots[name].path
            ):
                raise ToolProbeError(f"release tool {name} probe alias changed")
        result = {
            "format": PROBE_FORMAT,
            "host_family": host_family,
            "probe_contract_sha256": _probe_contract_sha256(invocation_records),
            "schema_version": PROBE_SCHEMA_VERSION,
            "tool_count": len(tool_results),
            "tools": tool_results,
        }
    finally:
        _remove_probe_root(probe_root)
    assert result is not None
    return result


def _read_manifest(
    path: Path, expected_sha256: str
) -> tuple[dict[str, Path], dict[str, str], dict[str, str]]:
    if _DIGEST_RE.fullmatch(expected_sha256) is None:
        raise ToolProbeError("release tool manifest expected digest is invalid")
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ToolProbeError("release tool manifest path is not absolute and normalized")
    try:
        parent_before = path.parent.lstat()
        metadata = path.lstat()
        resolved_parent = path.parent.resolve(strict=True)
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise ToolProbeError("release tool manifest is unavailable") from error
    if (
        resolved_parent != path.parent
        or resolved != path
        or stat.S_ISLNK(parent_before.st_mode)
        or not stat.S_ISDIR(parent_before.st_mode)
        or parent_before.st_uid != os.geteuid()
        or stat.S_IMODE(parent_before.st_mode) != 0o700
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != 0o400
    ):
        raise ToolProbeError("release tool manifest metadata or digest is unsafe")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ToolProbeError("release tool manifest could not be opened safely") from error
    data = bytearray()
    try:
        opened = os.fstat(descriptor)
        if not _same_file(metadata, opened):
            raise ToolProbeError("release tool manifest changed while opened")
        while True:
            block = os.read(descriptor, min(65536, MAXIMUM_MANIFEST_BYTES + 1 - len(data)))
            if not block:
                break
            data.extend(block)
            if len(data) > MAXIMUM_MANIFEST_BYTES:
                raise ToolProbeError("release tool manifest exceeds its byte bound")
        after = os.fstat(descriptor)
        if not _same_file(opened, after) or len(data) != opened.st_size:
            raise ToolProbeError("release tool manifest changed while authenticated")
    finally:
        os.close(descriptor)
    try:
        path_after = path.lstat()
        parent_after = path.parent.lstat()
    except OSError as error:
        raise ToolProbeError("release tool manifest disappeared") from error
    data_bytes = bytes(data)
    if (
        not _same_file(after, path_after)
        or not _same_file(parent_before, parent_after)
        or hashlib.sha256(data_bytes).hexdigest() != expected_sha256
    ):
        raise ToolProbeError("release tool manifest changed or has the wrong digest")
    try:
        value = json.loads(data_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ToolProbeError("release tool manifest is malformed") from error
    if (
        not isinstance(value, dict)
        or set(value) != {"schema_version", "tools"}
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
        or not isinstance(value["tools"], dict)
        or set(value["tools"]) != set(REQUIRED_TOOL_NAMES)
        or canonical_json(value) != data_bytes
    ):
        raise ToolProbeError("release tool manifest contract is not exact")
    paths: dict[str, Path] = {}
    digests: dict[str, str] = {}
    archive_ids: dict[str, str] = {}
    for name in REQUIRED_TOOL_NAMES:
        record = value["tools"][name]
        if (
            not isinstance(record, dict)
            or set(record) != {"archive_id", "path", "sha256"}
            or not isinstance(record["archive_id"], str)
            or not isinstance(record["path"], str)
            or not isinstance(record["sha256"], str)
        ):
            raise ToolProbeError(f"release tool manifest record {name} is not exact")
        if not record["path"] or "\0" in record["path"]:
            raise ToolProbeError(f"release tool manifest record {name} has an unsafe path")
        paths[name] = Path(record["path"])
        digests[name] = record["sha256"]
        archive_ids[name] = record["archive_id"]
    return paths, digests, archive_ids


def main() -> int:
    if not sys.flags.isolated or not sys.flags.no_site:
        print(
            "Sumeragi v2 release tool probe error: protected Python requires -I -S",
            file=sys.stderr,
        )
        return 1
    parser = argparse.ArgumentParser()
    parser.add_argument("--tool-manifest", type=Path, required=True)
    parser.add_argument("--expected-tool-manifest-sha256", required=True)
    parser.add_argument("--probe-root", type=Path, required=True)
    args = parser.parse_args()
    try:
        paths, digests, archive_ids = _read_manifest(
            args.tool_manifest, args.expected_tool_manifest_sha256
        )
        result = probe_release_tool_closure(
            paths,
            args.probe_root,
            expected_sha256=digests,
            archive_ids=archive_ids,
        )
    except (OSError, ToolProbeError) as error:
        print(f"Sumeragi v2 release tool probe error: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(canonical_json(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
