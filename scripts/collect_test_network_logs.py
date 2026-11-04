#!/usr/bin/env python3
"""Stage disposable peer process logs without copying runtime credentials or state."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import shutil


PROCESS_LOG = re.compile(r"run-[0-9]+-(?:stdout|stderr)\.log\Z")


def collect_logs(source: Path, destination: Path) -> int:
    """Copy regular peer stdout/stderr logs from an optionally hidden fixture root."""
    count = 0
    for network in sorted(source.glob("irohad_test_network_*")):
        if network.is_symlink() or not network.is_dir():
            continue
        for peer in sorted(network.iterdir()):
            if peer.is_symlink() or not peer.is_dir():
                continue
            for log in sorted(peer.iterdir()):
                if not PROCESS_LOG.fullmatch(log.name) or log.is_symlink() or not log.is_file():
                    continue
                target = destination / network.name / peer.name / log.name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(log, target)
                count += 1
    return count


def main() -> None:
    """Stage only the explicitly allowed process log files for CI upload."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    print(f"Staged {collect_logs(args.source, args.destination)} test-network process logs")


if __name__ == "__main__":
    main()
