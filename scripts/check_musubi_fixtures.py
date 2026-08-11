#!/usr/bin/env python3
"""Read-only two-pass check for the signed Musubi V1 fixture owner.

The checker invokes the argument-free typed Rust owner twice, requires the two
envelopes to be byte-identical, validates their exact closed output set, and
compares the emitted bytes with a descriptor-relative read of the repository.
It never creates, replaces, or removes a fixture path.  Both Cargo invocations
share the caller-supplied private external ``CARGO_TARGET_DIR`` and never write
build state beneath the repository.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Iterable, Sequence

import write_musubi_fixtures as fixture_writer

REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUTS = fixture_writer.OUTPUTS


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    return parser.parse_args(argv)


def compare_outputs(
    expected: Sequence[fixture_writer.RenderedOutput],
    actual: Sequence[fixture_writer.RenderedOutput],
    *,
    description: str,
) -> None:
    """Require identical ordered paths and bytes for the closed fixture pair."""

    if tuple(output.relative_path for output in expected) != OUTPUTS:
        raise RuntimeError("expected Musubi fixture set is not the closed V1 pair")
    if tuple(output.relative_path for output in actual) != OUTPUTS:
        raise RuntimeError(f"{description} is not the closed Musubi V1 pair")
    for left, right in zip(expected, actual):
        if left.contents != right.contents:
            raise RuntimeError(
                f"Musubi fixture drift for {left.relative_path}: {description}"
            )


def check() -> None:
    """Perform the deterministic read-only owner and repository check."""

    cargo_target_dir = fixture_writer.resolve_owner_cargo_target_dir()
    first_envelope = fixture_writer.run_owner(cargo_target_dir)
    second_envelope = fixture_writer.run_owner(cargo_target_dir)
    if first_envelope != second_envelope:
        raise RuntimeError("Musubi fixture owner emitted nondeterministic envelopes")
    first = fixture_writer.parse_owner_envelope(first_envelope)
    second = fixture_writer.parse_owner_envelope(second_envelope)
    compare_outputs(first, second, description="second typed-owner pass")
    checked_in = fixture_writer.read_closed_outputs(REPO_ROOT)
    compare_outputs(first, checked_in, description="checked-in repository bytes")


def main(argv: Iterable[str] | None = None) -> int:
    parse_args(argv)
    check()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
