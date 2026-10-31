#!/usr/bin/env python3
"""Regenerate the Cargo.lock pin chain of the Taira privacy rollout plan.

Kagami refuses to render a Taira release unless the checked-in rollout plan names
the exact Cargo.lock it was compiled with, and the plan's own SHA-256 is pinned in
two further places. Every dependency change therefore moves three values together.
This tool moves only those values. A changed protocol matrix or wire contract stays
a reviewed edit, and Kagami's own validation remains the authority.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
LOCK = Path("Cargo.lock")
ROLLOUT_PLAN = Path("configs/soranexus/taira/privacy_rollout_plan_v1.json")
BOOTSTRAP_PLAN = Path("configs/soranexus/taira/privacy_bootstrap_plan.json")
KAGAMI_RELEASE = Path("crates/iroha_kagami/src/privacy_bootstrap/release.rs")


def replace_exactly_one(pattern: str, digest: str, source: str, label: str) -> str:
    """Replace one pinned digest, refusing a pin that moved or was duplicated."""
    updated, count = re.subn(pattern, rf'\g<1>"{digest}"', source)
    if count != 1:
        raise ValueError(f"expected exactly one {label} pin, found {count}")
    return updated


def update_pins(lock: bytes, rollout: str, bootstrap: str, release: str) -> tuple[str, str, str]:
    """Derive the whole chain from the lock bytes, keeping every other byte unchanged."""
    rollout = replace_exactly_one(
        r'("cargo_lock_sha256": )"[0-9a-f]{64}"',
        hashlib.sha256(lock).hexdigest(),
        rollout,
        "rollout plan Cargo.lock",
    )
    rollout_digest = hashlib.sha256(rollout.encode()).hexdigest()
    bootstrap = replace_exactly_one(
        r'("rollout_plan_sha256": )"[0-9a-f]{64}"',
        rollout_digest,
        bootstrap,
        "bootstrap plan rollout",
    )
    release = replace_exactly_one(
        r'(const ROLLOUT_PLAN_SHA256_V1: &str =\s*)"[0-9a-f]{64}"',
        rollout_digest,
        release,
        "Kagami rollout",
    )
    return rollout, bootstrap, release


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail on drift without changing any file")
    args = parser.parse_args()
    paths = (ROLLOUT_PLAN, BOOTSTRAP_PLAN, KAGAMI_RELEASE)
    current = tuple((ROOT / path).read_bytes().decode() for path in paths)
    updated = update_pins((ROOT / LOCK).read_bytes(), *current)
    if args.check:
        if updated != current:
            raise SystemExit(
                "Taira privacy rollout pins differ from Cargo.lock; "
                "run scripts/update_taira_privacy_rollout_pins.py"
            )
    else:
        for path, before, after in zip(paths, current, updated):
            if after != before:
                (ROOT / path).write_bytes(after.encode())
    print("Taira privacy rollout pins match the exact Cargo.lock.")


if __name__ == "__main__":
    main()
