#!/usr/bin/env python3
"""Render a Taira validator systemd unit without reading runtime signing inputs.

Requires Python 3.10+ and explicit absolute Linux paths to retained owner-private
signers. Output is a new mode-0600 unit, never installed or overwritten here.
Validate it with systemd-analyze verify on the deployment host and bind its exact
bytes into the native public-reset inventory before installation.

Type=exec waits for systemd to execute the inline custody launcher. Native
process attestation and HTTP readiness still wait for the actual daemon.
"""
import argparse
import os
from pathlib import Path, PurePosixPath

CUSTODY = '''reserved_fds = (198, 199)
for reserved_fd in reserved_fds:
    try:
        os.fstat(reserved_fd)
    except OSError as error:
        if error.errno != errno.EBADF: raise
    else:
        raise RuntimeError("Taira signer descriptor is already occupied")
staged = []

def stage_signer(source_path, target_fd, size, label):
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    source_fd = os.open(source_path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | nofollow)
    if source_fd in reserved_fds:
        os.close(source_fd)
        raise RuntimeError("source open reached a reserved Taira signer descriptor")
    launch_path = source_path + ".fd" + str(target_fd)
    launch_fd = None
    launch_created = False
    launch_ready = False
    secret = bytearray(size)
    secret_view = memoryview(secret)
    try:
        source_before = os.fstat(source_fd)
        if not stat.S_ISREG(source_before.st_mode) or source_before.st_uid != os.geteuid() or source_before.st_mode & 0o7777 != 0o600 or source_before.st_nlink != 1 or source_before.st_size != size:
            raise RuntimeError("untrusted persistent Taira " + label + " file")
        try:
            stale = os.lstat(launch_path)
        except FileNotFoundError:
            stale = None
        if stale is not None:
            if not stat.S_ISREG(stale.st_mode) or stale.st_uid != os.geteuid() or stale.st_mode & 0o7777 != 0o600 or stale.st_nlink != 1 or stale.st_size not in (0, size):
                raise RuntimeError("untrusted stale Taira FD" + str(target_fd) + " launch file")
            os.unlink(launch_path)
        launch_fd = os.open(launch_path, os.O_RDWR | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) | nofollow, 0o600)
        launch_created = True
        if launch_fd in reserved_fds:
            raise RuntimeError("launch open reached a reserved Taira signer descriptor")
        offset = 0
        while offset < len(secret):
            count = os.readv(source_fd, [secret_view[offset:]])
            if count == 0: raise RuntimeError("short Taira " + label + " source")
            offset += count
        source_after = os.fstat(source_fd)
        stable_fields = ("st_dev", "st_ino", "st_uid", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
        if any(getattr(source_before, field) != getattr(source_after, field) for field in stable_fields):
            raise RuntimeError("Taira " + label + " source changed while staging")
        offset = 0
        while offset < len(secret):
            count = os.write(launch_fd, secret_view[offset:])
            if count == 0: raise RuntimeError("short Taira FD" + str(target_fd) + " launch write")
            offset += count
        os.fsync(launch_fd)
        os.lseek(launch_fd, 0, os.SEEK_SET)
        launch_stat = os.fstat(launch_fd)
        if not stat.S_ISREG(launch_stat.st_mode) or launch_stat.st_uid != os.geteuid() or launch_stat.st_mode & 0o7777 != 0o600 or launch_stat.st_nlink != 1 or launch_stat.st_size != size:
            raise RuntimeError("untrusted Taira FD" + str(target_fd) + " launch file")
        os.dup2(launch_fd, target_fd, inheritable=True)
        staged.append((target_fd, launch_path))
        launch_ready = True
    finally:
        for index in range(len(secret)): secret[index] = 0
        secret_view.release()
        # A write/fsync/dup2 failure can leave a full copy before it joins
        # staged. Erase that owned copy while its descriptor is still open.
        if launch_created and not launch_ready and launch_fd is not None:
            try:
                os.ftruncate(launch_fd, 0)
                os.fsync(launch_fd)
            except OSError: pass
        try: os.close(source_fd)
        except OSError: pass
        if launch_fd is not None:
            try: os.close(launch_fd)
            except OSError: pass
        if launch_created and not launch_ready:
            try: os.unlink(launch_path)
            except OSError: pass

try:
    stage_signer(runtime_key, 198, 71, "runtime signer")
    stage_signer(mint_finality_seed, 199, 32, "mint-finality seed")
    os.execv(cmd[0], cmd)
finally:
    # Successful foreground exec never returns. Failed staging/exec must not
    # leave an extra full seed copy or close an unrelated inherited descriptor.
    for staged_fd, staged_path in reversed(staged):
        try:
            os.ftruncate(staged_fd, 0)
            os.fsync(staged_fd)
        except OSError:
            pass
        try: os.close(staged_fd)
        except OSError: pass
        try: os.unlink(staged_path)
        except OSError: pass
'''

ROLES = tuple(f"taira-validator-{index}" for index in range(1, 5))


def checked_key_path(value):
    path = PurePosixPath(value)
    if (not value.startswith("/") or value.startswith("//") or str(path) != value
            or ".." in path.parts or any(ord(char) < 32 or ord(char) == 127 for char in value)
            or path.name in ("", ".", "..")):
        raise ValueError("signer input must be an explicit canonical absolute Linux path")
    return value


def launcher(role, runtime_key, mint_finality_seed):
    if role not in ROLES:
        raise ValueError("unknown validator role")
    runtime_key = checked_key_path(runtime_key)
    mint_finality_seed = checked_key_path(mint_finality_seed)
    paths = (runtime_key, runtime_key + ".fd198", mint_finality_seed, mint_finality_seed + ".fd199")
    if len(set(paths)) != len(paths):
        raise ValueError("retained signer and launch-copy paths must all be distinct")
    current = f"/srv/taira/{role}/current"
    cmd = [current + "/bin/iroha3d_taira", "--config", current + "/config/config.toml", "--sora"]
    # Native consumers truncate only their independent owner-private, single-link
    # RW launch copies. Both retained native sources remain intact for restart.
    return ("import errno\nimport os\nimport stat\n"
            + f"runtime_key = {runtime_key!r}\nmint_finality_seed = {mint_finality_seed!r}\n"
            + f"cmd = {cmd!r}\n" + CUSTODY)


def systemd_argument(value):
    # systemd.syntax C escaping, then systemd.exec specifier/environment escaping.
    # This is one quoted argv element, without a shell or external launcher file.
    out = value.replace("\\", "\\\\").replace('"', '\\"')
    out = out.replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")
    out = out.replace("%", "%%").replace("$", "$$")
    return '"' + out + '"'


def render(role, runtime_key, mint_finality_seed):
    code = launcher(role, runtime_key, mint_finality_seed)
    compile(code, "<signed-unit-inline-python>", "exec")
    return (f"[Unit]\nDescription=Taira {role}\nAfter=network.target\n\n"
            "[Service]\nType=exec\nUser=root\nGroup=root\nUMask=0077\n"
            f"WorkingDirectory=/var/lib/taira/{role}\n"
            "CPUQuota=100%\nMemoryMax=2147483648\nRestart=on-failure\nRestartSec=5s\n"
            f"ExecStart=/usr/bin/python3 -c {systemd_argument(code)}\n"
            "\n[Install]\nWantedBy=multi-user.target\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--role", choices=ROLES, required=True)
    parser.add_argument("--runtime-key", required=True, help="Actual retained owner-0600, single-link, 71-byte signer path on the approved guest; never read here")
    parser.add_argument("--mint-finality-seed", required=True, help="Actual retained owner-0600, single-link, 32-byte raw mint-finality seed path on the approved guest; never read here")
    parser.add_argument("--output", type=Path, required=True, help="Fresh iroha3d-ROLE.service file; never overwritten")
    args = parser.parse_args()
    if args.output.name != f"iroha3d-{args.role}.service":
        parser.error("output filename must match the canonical role unit name")
    try:
        content = render(args.role, args.runtime_key, args.mint_finality_seed).encode()
    except ValueError as error:
        parser.error(str(error))
    fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(content)
        output.flush()
        os.fsync(output.fileno())
    print(args.output)


if __name__ == "__main__":
    main()
