#!/usr/bin/env python3
"""Create a private native CI fixture root and append its paths to GITHUB_ENV.

Requires Python 3.11+, Git and a Unix service account with a trusted home from
the system account database. HOME and temporary-directory environment variables
never select custody storage. The root is fresh, outside Git, and retained for
the job; existing failure-artifact globs select network evidence beneath it.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import pwd
import secrets
import stat
import subprocess
import unicodedata

ROOT_PREFIX = ".i-"
ROOT_RANDOM_BYTES = 4
# Reserve the native broker's complete child name and portable Unix socket bound.
BROKER_CHILD = ".iroha-b-" + "x" * 6
BROKER_SOCKET = "runtime-provider-broker-v1.sock"
SOCKET_MAX_BYTES = 103


def service_home() -> Path:
    """Resolve the effective service account without trusting HOME."""
    raw = pwd.getpwuid(os.geteuid()).pw_dir
    path = Path(raw)
    if not path.is_absolute() or str(path) != raw:
        raise ValueError("service account home must be a canonical absolute path")
    return path


def validate_directory(path: Path, *, private: bool) -> None:
    """Require the native broker and beacon fixture's joint ancestry policy."""
    raw = str(path)
    raw.encode("utf-8", errors="strict")
    if (
        not path.is_absolute()
        or "\\" in raw
        or any(unicodedata.category(c) == "Cc" for c in raw)
        or any(part in (".", "..") for part in path.parts)
        or path.resolve(strict=True) != path
    ):
        raise ValueError("fixture path must be absolute, canonical and direct")
    owner = os.geteuid()
    for index, ancestor in enumerate((path, *path.parents)):
        info = ancestor.lstat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or info.st_mode & 0o022
            or info.st_uid not in ({owner} if index == 0 else {0, owner})
        ):
            raise ValueError(f"untrusted fixture directory ancestor: {ancestor}")
        if os.path.lexists(ancestor / ".git"):
            raise ValueError("fixture root must remain outside Git")
        if index == 0 and private and stat.S_IMODE(info.st_mode) != 0o700:
            raise ValueError("fixture root must be owner-private mode 0700")
    # Also reject bare repositories. Ignore caller-selected Git worktree/config
    # environment so this check examines the directory's real ancestry.
    environment = {
        key: value for key, value in os.environ.items() if not key.startswith("GIT_")
    }
    environment.update(LC_ALL="C", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
    result = subprocess.run(
        ["git", "-C", raw, "rev-parse", "--git-dir"],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        env=environment,
        check=False,
    )
    if result.returncode != 128 or "not a git repository" not in result.stderr:
        raise ValueError("fixture root must remain outside Git")


def validate_socket_budget(path: Path) -> None:
    """Reserve the full native broker child/socket path before creating a root."""
    endpoint = path / BROKER_CHILD / BROKER_SOCKET
    if len(str(endpoint).encode("utf-8")) > SOCKET_MAX_BYTES:
        raise ValueError("service home makes the native broker socket path too long")


def create_fixture_root() -> Path:
    """Create a fresh 0700 child of the actual service user's trusted home."""
    home = service_home()
    validate_directory(home, private=False)
    validate_socket_budget(home / (ROOT_PREFIX + "x" * (2 * ROOT_RANDOM_BYTES)))
    for _ in range(16):
        root = home / (ROOT_PREFIX + secrets.token_hex(ROOT_RANDOM_BYTES))
        try:
            root.mkdir(mode=0o700)
        except FileExistsError:
            continue
        try:
            validate_directory(root, private=True)
            validate_socket_budget(root)
        except Exception:
            root.rmdir()  # Only this new, empty directory; never recursive cleanup.
            raise
        return root
    raise ValueError("could not allocate a fresh private fixture root")


def export_fixture_root(environment_file: Path, root: Path) -> None:
    """Append both native fixture selectors without altering HOME or TMPDIR."""
    validate_directory(root, private=True)
    validate_socket_budget(root)
    descriptor = os.open(environment_file, os.O_WRONLY | os.O_APPEND | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "w", encoding="utf-8") as destination:
        info = os.fstat(destination.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid():
            raise ValueError("GITHUB_ENV must be a regular file owned by the service user")
        destination.write(f"TEST_NETWORK_TMP_DIR={root}\n")
        destination.write(f"TAIRA_TESTNET_BEACON_FIXTURE_DIR={root}\n")


def main() -> int:
    """Create and export one job-owned fixture root or fail without fallback."""
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--github-env", required=True, type=Path)
    args = parser.parse_args()
    root = create_fixture_root()
    try:
        export_fixture_root(args.github_env, root)
    except Exception:
        root.rmdir()
        raise
    print(f"Native CI fixtures: {root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
