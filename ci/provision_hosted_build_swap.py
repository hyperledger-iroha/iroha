#!/usr/bin/env python3
"""Add bounded, job-local swap before heavy builds on GitHub-hosted Linux.

The observed 16 GiB runner exhausted its RAM and existing 3 GiB swap. Add one
16 GiB file, retaining at least 24 GiB free for builds. Existing swap, compiler
policy and test limits are unchanged. The disposable runner owns cleanup.
"""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys

MIB = 1024**2
SWAP_BYTES = 16 * 1024**3
BUILD_HEADROOM_BYTES = 24 * 1024**3


def provision() -> Path:
    """Create and activate one private, fully written file on the hosted runner."""
    if (sys.platform != "linux" or os.environ.get("GITHUB_ACTIONS") != "true"
            or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or os.environ.get("RUNNER_OS") != "Linux"):
        raise RuntimeError("build swap requires GitHub-hosted Linux")
    temporary = Path(os.environ.get("RUNNER_TEMP", ""))
    if not temporary.is_absolute() or temporary.is_symlink() or not temporary.is_dir():
        raise RuntimeError("build swap requires an absolute runner temporary directory")
    if shutil.disk_usage(temporary).free < SWAP_BYTES + BUILD_HEADROOM_BYTES:
        raise RuntimeError("build swap requires 16 GiB plus 24 GiB build headroom")
    directory = temporary / "iroha-ci-build-swap"
    # Refuse a repeated invocation or any pre-existing entry, including symlinks.
    directory.mkdir(mode=0o700)
    path = directory / "swapfile"
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb", buffering=0) as output:
        os.fchmod(output.fileno(), 0o600)
        # Write actual blocks, not a sparse/truncated file or filesystem-dependent
        # fallocate extent. dd writes through the original exclusive descriptor.
        subprocess.run(
            ["dd", "if=/dev/zero", "bs=1M", f"count={SWAP_BYTES // MIB}",
             "conv=fsync", "status=none"], stdout=output, check=True,
        )
        if os.fstat(output.fileno()).st_size != SWAP_BYTES:
            raise RuntimeError("build swap file has an unexpected extent")
    if shutil.disk_usage(temporary).free < BUILD_HEADROOM_BYTES:
        raise RuntimeError("build swap allocation left insufficient build headroom")
    subprocess.run(["sudo", "-n", "chown", "0:0", "--", str(path)], check=True)
    subprocess.run(["sudo", "-n", "mkswap", "--", str(path)], check=True)
    subprocess.run(["sudo", "-n", "swapon", "--", str(path)], check=True)
    subprocess.run(["swapon", "--show", "--bytes"], check=True)
    print(f"Added {SWAP_BYTES} bytes of hosted build swap at {path}", flush=True)
    return path


if __name__ == "__main__":
    provision()
